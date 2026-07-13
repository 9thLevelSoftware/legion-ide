//! Text primitives: rope-backed buffers, UTF index conversion, edits, and immutable snapshots.

#![warn(missing_docs)]

use std::cmp::Ordering;
use std::fmt;
use std::sync::Arc;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicU64, Ordering as AtomicOrdering};

use legion_protocol::{BufferVersion, SnapshotId};
use memchr::memchr;
use ropey::Rope;
use sha2::{Digest, Sha256};
use thiserror::Error;

pub mod binary;
pub use binary::{BinaryDetectionResult, detect_binary, detect_binary_with_window};

static NEXT_SNAPSHOT_ID: AtomicU64 = AtomicU64::new(1);

const DEFAULT_LEAF_TARGET_BYTES: usize = 1024;
const DEFAULT_CHUNK_TARGET_BYTES: usize = 64 * 1024;
const DEFAULT_CHUNK_BOUNDARY_WINDOW_BYTES: usize = 8 * 1024;
const DEFAULT_CHUNK_FORCE_MAX_BYTES: usize = 96 * 1024;
const DEFAULT_LINE_SLICE_MAX_BYTES: usize = DEFAULT_CHUNK_FORCE_MAX_BYTES;

/// Maximum UTF-8 bytes allowed in the optional full text cache used by the text model.
///
/// Buffers larger than this threshold remain rope-backed, chunk-indexed, and sliceable, but they
/// do not retain a compatibility full-source cache.
pub const DEFAULT_FULL_CACHE_BYTE_BUDGET_BYTES: usize = 5 * 1024 * 1024;

/// A position in text expressed as zero-indexed line and UTF-8 column within that line.
///
/// `line` is zero-based. `column` is a UTF-8 byte offset from the start of the logical line,
/// excluding the line ending. This preserves compatibility with the spike API while the richer
/// [`LineIndex`] APIs expose explicit UTF-16 and byte-coordinate conversions for LSP use.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TextPosition {
    /// Zero-indexed line number.
    pub line: usize,
    /// Zero-indexed UTF-8 byte column within the line.
    pub column: usize,
}

impl TextPosition {
    /// Create a new position at the given line and column.
    pub const fn new(line: usize, column: usize) -> Self {
        Self { line, column }
    }

    /// Return a position at the start of the document (`0,0`).
    pub const fn zero() -> Self {
        Self { line: 0, column: 0 }
    }
}

impl fmt::Display for TextPosition {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.line, self.column)
    }
}

/// A range between two [`TextPosition`]s.
///
/// `start` is inclusive and `end` is exclusive. It is valid when `start` is less than or equal to
/// `end` within the same buffer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TextRange {
    /// Inclusive start position.
    pub start: TextPosition,
    /// Exclusive end position.
    pub end: TextPosition,
}

impl TextRange {
    /// Create a new range from `start` to `end`.
    ///
    /// # Panics
    ///
    /// Panics in debug mode if `start` is after `end`.
    pub fn new(start: TextPosition, end: TextPosition) -> Self {
        debug_assert!(
            start.line < end.line || (start.line == end.line && start.column <= end.column),
            "TextRange start must be <= end"
        );
        Self { start, end }
    }

    /// Create a zero-length (collapsed) range at `pos`.
    pub const fn empty(pos: TextPosition) -> Self {
        Self {
            start: pos,
            end: pos,
        }
    }

    /// Returns `true` if the range contains no characters.
    pub fn is_empty(&self) -> bool {
        self.start == self.end
    }
}

impl fmt::Display for TextRange {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}..{}", self.start, self.end)
    }
}

/// A single text edit operation: replace the contents of [`TextRange`] with `new_text`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextEdit {
    /// The range to replace.
    pub range: TextRange,
    /// The text to insert in place of the range.
    pub new_text: String,
}

impl TextEdit {
    /// Create a new edit.
    pub fn new(range: TextRange, new_text: impl Into<String>) -> Self {
        Self {
            range,
            new_text: new_text.into(),
        }
    }

    /// Create an insertion edit (empty range).
    pub fn insert(pos: TextPosition, text: impl Into<String>) -> Self {
        Self {
            range: TextRange::empty(pos),
            new_text: text.into(),
        }
    }

    /// Create a deletion edit (empty replacement text).
    pub fn delete(range: TextRange) -> Self {
        Self {
            range,
            new_text: String::new(),
        }
    }
}

/// Reason that keeps a snapshot retained by the editor engine.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum RetentionPinReason {
    /// Current live buffer state.
    CurrentBuffer,
    /// Referenced by undo history.
    UndoHistory,
    /// Referenced by redo history.
    RedoHistory,
    /// Referenced by an in-flight background save.
    BackgroundSave,
    /// Referenced by an LSP synchronization operation.
    LspSync,
    /// Referenced by indexing or analysis.
    Indexing,
    /// Explicit caller-specified reason.
    Explicit(String),
}

impl fmt::Display for RetentionPinReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RetentionPinReason::CurrentBuffer => write!(f, "current-buffer"),
            RetentionPinReason::UndoHistory => write!(f, "undo-history"),
            RetentionPinReason::RedoHistory => write!(f, "redo-history"),
            RetentionPinReason::BackgroundSave => write!(f, "background-save"),
            RetentionPinReason::LspSync => write!(f, "lsp-sync"),
            RetentionPinReason::Indexing => write!(f, "indexing"),
            RetentionPinReason::Explicit(reason) => write!(f, "{reason}"),
        }
    }
}

/// Immutable metadata describing a text snapshot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextSnapshotDescriptor {
    /// Globally unique snapshot identifier.
    pub snapshot_id: SnapshotId,
    /// Buffer version captured by the snapshot.
    pub buffer_version: BufferVersion,
    /// SHA-256 hash of snapshot chunk metadata, hex encoded.
    pub content_hash: String,
    /// Total byte length.
    pub byte_len: usize,
    /// Total logical line count.
    pub line_count: usize,
    /// Estimated memory footprint in bytes.
    pub memory_footprint_bytes: usize,
    /// Reason this snapshot is pinned for retention.
    pub retention_pin_reason: RetentionPinReason,
}

/// Metadata describing a stable UTF-8 chunk within a buffer or snapshot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextChunkDescriptor {
    /// Zero-based chunk ordinal within the snapshot or buffer.
    pub ordinal: usize,
    /// Inclusive absolute start byte.
    pub start_byte: usize,
    /// Exclusive absolute end byte.
    pub end_byte: usize,
    /// Chunk byte length.
    pub byte_len: usize,
    /// Logical line containing the first byte of the chunk.
    pub start_line: usize,
    /// Logical line containing the last byte of the chunk.
    pub end_line: usize,
    /// Domain-separated SHA-256 hash of the exact chunk bytes.
    pub hash: String,
}

/// A bounded line slice suitable for viewport-style rendering.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextLineSlice {
    /// Zero-based logical line number.
    pub line: usize,
    /// Absolute byte offset for the start of the logical line.
    pub line_start_byte: usize,
    /// Exclusive absolute byte offset for the returned slice text.
    pub slice_end_byte: usize,
    /// Full logical line byte length excluding the line ending.
    pub line_content_byte_len: usize,
    /// Returned slice UTF-8 byte length.
    pub utf8_byte_len: usize,
    /// Returned slice UTF-16 code-unit length.
    pub utf16_len: usize,
    /// Logical line ending byte width: `0`, `1`, or `2`.
    pub line_ending_bytes: usize,
    /// `true` when the logical line exceeded the slice budget and was clipped.
    pub truncated: bool,
    /// Returned bounded line text.
    pub text: String,
}

/// Immutable snapshot of buffer contents.
///
/// Snapshots are cheap to clone because rope nodes are shared through [`Arc`]. Full-source text is
/// retained only when the snapshot remains within [`DEFAULT_FULL_CACHE_BYTE_BUDGET_BYTES`].
#[derive(Debug, Clone)]
pub struct TextSnapshot {
    rope: Arc<Rope>,
    full_text_cache: Option<Arc<String>>,
    /// Lazily materialized full text used as a fallback for [`TextSnapshot::text`] when the bounded
    /// compatibility cache is absent. Computed at most once per snapshot.
    materialized_full_text: OnceLock<Arc<String>>,
    line_index: Arc<LineIndex>,
    descriptor: TextSnapshotDescriptor,
}

impl PartialEq for TextSnapshot {
    fn eq(&self, other: &Self) -> bool {
        self.descriptor == other.descriptor
    }
}

impl Eq for TextSnapshot {}

impl TextSnapshot {
    /// Create a snapshot from a string with an automatically assigned ID and version 0.
    pub fn new(text: impl Into<String>) -> Self {
        Self::try_new(text).expect("snapshot construction should not fail")
    }

    /// Try to create a snapshot from a string with an automatically assigned ID and version 0.
    pub fn try_new(text: impl Into<String>) -> TextResult<Self> {
        let text = text.into();
        let rope = Rope::from_str(&text);
        let full_text_cache = if text.len() <= DEFAULT_FULL_CACHE_BYTE_BUDGET_BYTES {
            Some(Arc::new(text))
        } else {
            None
        };
        Self::from_rope_parts(
            Arc::new(rope),
            BufferVersion(0),
            RetentionPinReason::Explicit("standalone snapshot".to_string()),
            full_text_cache,
        )
    }

    /// Create a snapshot from a rope, version, and retention reason.
    pub fn from_rope(
        rope: Rope,
        buffer_version: BufferVersion,
        retention_pin_reason: RetentionPinReason,
    ) -> Self {
        Self::try_from_rope(rope, buffer_version, retention_pin_reason)
            .expect("snapshot construction should not fail")
    }

    /// Try to create a snapshot from a rope, version, and retention reason.
    pub fn try_from_rope(
        rope: Rope,
        buffer_version: BufferVersion,
        retention_pin_reason: RetentionPinReason,
    ) -> TextResult<Self> {
        let rope = Arc::new(rope);
        let full_text_cache = maybe_full_text_cache(rope.as_ref())?.map(Arc::new);
        Self::from_rope_parts(rope, buffer_version, retention_pin_reason, full_text_cache)
    }

    fn from_rope_parts(
        rope: Arc<Rope>,
        buffer_version: BufferVersion,
        retention_pin_reason: RetentionPinReason,
        full_text_cache: Option<Arc<String>>,
    ) -> TextResult<Self> {
        let line_index = Arc::new(LineIndex::from_rope(rope.clone()));
        Self::from_rope_parts_with_line_index(
            rope,
            buffer_version,
            retention_pin_reason,
            full_text_cache,
            line_index,
        )
    }

    fn from_rope_parts_with_line_index(
        rope: Arc<Rope>,
        buffer_version: BufferVersion,
        retention_pin_reason: RetentionPinReason,
        full_text_cache: Option<Arc<String>>,
        line_index: Arc<LineIndex>,
    ) -> TextResult<Self> {
        let descriptor = TextSnapshotDescriptor {
            snapshot_id: SnapshotId(NEXT_SNAPSHOT_ID.fetch_add(1, AtomicOrdering::Relaxed) as u128),
            buffer_version,
            content_hash: snapshot_content_hash(line_index.chunk_descriptors()),
            byte_len: rope.len_bytes(),
            line_count: line_index.line_count(),
            memory_footprint_bytes: estimate_rope_memory(
                rope.as_ref(),
                full_text_cache.as_ref().map_or(0, |text| text.len()),
                line_index.as_ref(),
            ),
            retention_pin_reason,
        };

        Ok(Self {
            rope,
            full_text_cache,
            materialized_full_text: OnceLock::new(),
            line_index,
            descriptor,
        })
    }

    /// Return the full text.
    ///
    /// When the snapshot retains a bounded compatibility full-text cache the cached string is
    /// returned directly. For large or degraded snapshots that dropped the bounded cache, the full
    /// text is materialized from the rope on first use and retained for the snapshot's lifetime, so
    /// this method never panics. Prefer [`TextSnapshot::try_full_text`] (or the bounded
    /// [`TextSnapshot::line_slice`] / [`TextSnapshot::chunk_text`] APIs) when the additional
    /// materialized retention is undesirable.
    pub fn text(&self) -> &str {
        if let Some(text) = self.full_text_cache.as_ref() {
            return text.as_str();
        }
        self.materialized_full_text
            .get_or_init(|| Arc::new(self.rope.to_string()))
            .as_str()
    }

    /// Return the cached full text when the snapshot remains within the full-cache budget.
    pub fn try_full_text(&self) -> TextResult<&str> {
        self.full_text_cache
            .as_ref()
            .map(|text| text.as_str())
            .ok_or(TextError::FullCacheBudgetExceeded {
                byte_len: self.len(),
                budget: DEFAULT_FULL_CACHE_BYTE_BUDGET_BYTES,
            })
    }

    /// Return the chunk descriptors backing this snapshot.
    pub fn chunk_descriptors(&self) -> &[TextChunkDescriptor] {
        self.line_index.chunk_descriptors()
    }

    /// Materialize a single bounded chunk by ordinal.
    pub fn chunk_text(&self, ordinal: usize) -> TextResult<String> {
        self.line_index.chunk_text(ordinal)
    }

    /// Materialize the full snapshot by concatenating bounded chunks.
    ///
    /// Plan Phase 1: this is the save-payload escape hatch for degraded buffers. It preserves the
    /// normal projection rule that UI callers must not rely on [`TextSnapshot::try_full_text`] for
    /// large snapshots, while still allowing editor-owned save assembly to stream through chunk
    /// boundaries under proposal-mediated workspace writes.
    pub fn materialize_full_text_from_chunks(&self) -> TextResult<String> {
        let mut text = String::with_capacity(self.len());
        for chunk in self.chunk_descriptors() {
            text.push_str(&self.chunk_text(chunk.ordinal)?);
        }
        if text.len() != self.len() {
            return Err(TextError::MaterializedLengthMismatch {
                expected: self.len(),
                actual: text.len(),
            });
        }
        Ok(text)
    }

    /// Return a bounded slice for a single logical line.
    pub fn line_slice(&self, line: usize) -> TextResult<TextLineSlice> {
        self.line_slice_with_limit(line, DEFAULT_LINE_SLICE_MAX_BYTES)
    }

    /// Return a bounded slice for a single logical line with an explicit byte budget.
    pub fn line_slice_with_limit(
        &self,
        line: usize,
        max_bytes: usize,
    ) -> TextResult<TextLineSlice> {
        self.line_index.line_slice(line, max_bytes)
    }

    /// Return the exact logical line range requested by a viewport using the default per-line
    /// slice budget.
    pub fn visible_line_slices(
        &self,
        start_line: usize,
        end_line_exclusive: usize,
    ) -> TextResult<Vec<TextLineSlice>> {
        self.visible_line_slices_with_limit(
            start_line,
            end_line_exclusive,
            DEFAULT_LINE_SLICE_MAX_BYTES,
        )
    }

    /// Return the exact logical line range requested by a viewport with an explicit per-line
    /// slice budget.
    pub fn visible_line_slices_with_limit(
        &self,
        start_line: usize,
        end_line_exclusive: usize,
        max_bytes_per_line: usize,
    ) -> TextResult<Vec<TextLineSlice>> {
        self.line_index
            .visible_line_slices(start_line, end_line_exclusive, max_bytes_per_line)
    }

    /// Return the immutable descriptor.
    pub fn descriptor(&self) -> &TextSnapshotDescriptor {
        &self.descriptor
    }

    /// Return the snapshot ID.
    pub fn snapshot_id(&self) -> SnapshotId {
        self.descriptor.snapshot_id
    }

    /// Return the buffer version captured by this snapshot.
    pub fn buffer_version(&self) -> BufferVersion {
        self.descriptor.buffer_version
    }

    /// Return the SHA-256 content hash.
    pub fn content_hash(&self) -> &str {
        &self.descriptor.content_hash
    }

    /// Return the line index for offset conversion and chunk metadata.
    pub fn line_index(&self) -> &LineIndex {
        &self.line_index
    }

    /// Return the number of lines in the snapshot.
    pub fn line_count(&self) -> usize {
        self.descriptor.line_count
    }

    /// Return the byte length of the text.
    pub fn len(&self) -> usize {
        self.descriptor.byte_len
    }

    /// Returns `true` if the snapshot contains no text.
    pub fn is_empty(&self) -> bool {
        self.descriptor.byte_len == 0
    }

    /// Return an estimated memory footprint in bytes.
    pub fn memory_footprint_bytes(&self) -> usize {
        self.descriptor.memory_footprint_bytes
    }

    /// Convert this snapshot back into a rope clone.
    pub fn rope(&self) -> Rope {
        self.rope.as_ref().clone()
    }
}

/// Errors returned by text model offset and edit operations.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum TextError {
    /// A byte offset was outside the document.
    #[error("byte offset {offset} is outside document length {len}")]
    ByteOffsetOutOfBounds {
        /// Requested offset.
        offset: usize,
        /// Document length.
        len: usize,
    },
    /// A byte offset was not on a UTF-8 character boundary.
    #[error("byte offset {offset} is not a UTF-8 character boundary")]
    NotUtf8Boundary {
        /// Requested offset.
        offset: usize,
    },
    /// A line index was outside the line table.
    #[error("line {line} is outside line count {line_count}")]
    LineOutOfBounds {
        /// Requested line.
        line: usize,
        /// Total line count.
        line_count: usize,
    },
    /// A chunk index was outside the chunk table.
    #[error("chunk {chunk} is outside chunk count {chunk_count}")]
    ChunkOutOfBounds {
        /// Requested chunk ordinal.
        chunk: usize,
        /// Total chunk count.
        chunk_count: usize,
    },
    /// A column was outside its line.
    #[error("{unit} column {column} is outside line {line} length {line_len}")]
    ColumnOutOfBounds {
        /// Requested line.
        line: usize,
        /// Requested column.
        column: usize,
        /// Line length in the requested unit.
        line_len: usize,
        /// Unit name.
        unit: &'static str,
    },
    /// A UTF-16 offset landed inside a surrogate pair.
    #[error("UTF-16 column {column} on line {line} splits a surrogate pair")]
    Utf16InsideSurrogatePair {
        /// Requested line.
        line: usize,
        /// Requested UTF-16 column.
        column: usize,
    },
    /// A range had start greater than end.
    #[error("invalid range {start}..{end}")]
    InvalidRange {
        /// Start offset.
        start: usize,
        /// End offset.
        end: usize,
    },
    /// A full-text compatibility operation exceeded the text-model byte budget.
    #[error(
        "text byte length {byte_len} exceeds full-cache budget {budget}; degraded large-file mode required"
    )]
    FullCacheBudgetExceeded {
        /// Requested text length in bytes.
        byte_len: usize,
        /// Maximum allowed full-cache text length in bytes.
        budget: usize,
    },
    /// Chunk materialization did not recreate the expected snapshot byte length.
    #[error("chunk materialization produced {actual} bytes but snapshot expects {expected} bytes")]
    MaterializedLengthMismatch {
        /// Expected snapshot length in bytes.
        expected: usize,
        /// Actual materialized text length in bytes.
        actual: usize,
    },
}

/// Result type for text model operations.
pub type TextResult<T> = Result<T, TextError>;

/// LSP-compatible UTF-16 position.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Utf16Position {
    /// Zero-based line number.
    pub line: usize,
    /// Zero-based UTF-16 code-unit column.
    pub character: usize,
}

impl Utf16Position {
    /// Construct a UTF-16 position.
    pub const fn new(line: usize, character: usize) -> Self {
        Self { line, character }
    }
}

/// LSP-compatible UTF-16 range.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Utf16Range {
    /// Inclusive start position.
    pub start: Utf16Position,
    /// Exclusive end position.
    pub end: Utf16Position,
}

impl Utf16Range {
    /// Construct a UTF-16 range.
    pub const fn new(start: Utf16Position, end: Utf16Position) -> Self {
        Self { start, end }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct LineMetric {
    start_byte: usize,
    content_end_byte: usize,
    end_byte: usize,
    byte_len: usize,
    utf16_len: usize,
    line_ending_bytes: usize,
}

impl LineMetric {
    fn contains_offset(&self, offset: usize, is_last_line: bool) -> bool {
        if is_last_line {
            self.start_byte <= offset && offset <= self.end_byte
        } else {
            self.start_byte <= offset && offset < self.end_byte
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ChunkedLineIndex {
    rope: Arc<Rope>,
    lines: Vec<LineMetric>,
    chunks: Vec<TextChunkDescriptor>,
}

/// Line index supporting byte, UTF-8, UTF-16, and chunk-aware coordinate conversion.
///
/// The index treats CRLF (`\r\n`) as a single line ending for LSP mapping and excludes line-ending
/// bytes from column lengths. UTF-16 conversions reject offsets inside surrogate pairs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LineIndex {
    inner: ChunkedLineIndex,
}

impl LineIndex {
    /// Build a line index for the provided UTF-8 text.
    pub fn new(text: &str) -> Self {
        Self::from_rope(Arc::new(Rope::from_str(text)))
    }

    fn from_rope(rope: Arc<Rope>) -> Self {
        let lines = scan_line_metrics_from_byte(rope.as_ref(), 0);
        let chunks = build_chunk_descriptors(rope.as_ref(), &lines, 0, 0);
        Self {
            inner: ChunkedLineIndex {
                rope,
                lines,
                chunks,
            },
        }
    }

    fn rebuild_from_chunk(&self, rope: Arc<Rope>, start_chunk_index: usize) -> Self {
        if self.inner.chunks.is_empty() || start_chunk_index == 0 {
            return Self::from_rope(rope);
        }

        let rebuild_chunk = self
            .inner
            .chunks
            .get(start_chunk_index)
            .unwrap_or_else(|| self.inner.chunks.last().expect("chunk list is non-empty"));
        let rebuild_start_line = self.line_for_offset_context(rebuild_chunk.start_byte);
        let rebuild_line_start_byte = self.inner.lines[rebuild_start_line].start_byte;

        let mut lines = self.inner.lines[..rebuild_start_line].to_vec();
        lines.extend(scan_line_metrics_from_byte(
            rope.as_ref(),
            rebuild_line_start_byte,
        ));

        let mut chunks = self.inner.chunks[..start_chunk_index].to_vec();
        chunks.extend(build_chunk_descriptors(
            rope.as_ref(),
            &lines,
            rebuild_chunk.start_byte,
            start_chunk_index,
        ));

        Self {
            inner: ChunkedLineIndex {
                rope,
                lines,
                chunks,
            },
        }
    }

    fn rebuild_from_simple_edit(
        &self,
        rope: Arc<Rope>,
        edit_start: usize,
        edit_end: usize,
        replacement_text: &str,
        removed_utf16_len: usize,
    ) -> Option<Self> {
        if replacement_text.contains('\n') || replacement_text.contains('\r') {
            return None;
        }

        let edit_line_index = self.line_for_offset_context(edit_start);
        let edit_chunk_index = self.rebuild_start_chunk_index(edit_start);
        let byte_delta =
            replacement_text.len() as isize - (edit_end as isize - edit_start as isize);
        let replacement_utf16_len = replacement_text.chars().map(char::len_utf16).sum::<usize>();
        let utf16_delta = replacement_utf16_len as isize - removed_utf16_len as isize;

        let mut lines = self.inner.lines.clone();
        let line = lines.get_mut(edit_line_index)?;
        line.content_end_byte = shift_usize(line.content_end_byte, byte_delta);
        line.end_byte = shift_usize(line.end_byte, byte_delta);
        line.byte_len = shift_usize(line.byte_len, byte_delta);
        line.utf16_len = shift_usize(line.utf16_len, utf16_delta);

        for line in lines.iter_mut().skip(edit_line_index + 1) {
            line.start_byte = shift_usize(line.start_byte, byte_delta);
            line.content_end_byte = shift_usize(line.content_end_byte, byte_delta);
            line.end_byte = shift_usize(line.end_byte, byte_delta);
        }

        let mut chunks = self.inner.chunks.clone();
        let chunk = chunks.get_mut(edit_chunk_index)?;
        chunk.end_byte = shift_usize(chunk.end_byte, byte_delta);
        chunk.byte_len = shift_usize(chunk.byte_len, byte_delta);
        // Repeated same-line edits can grow the edited chunk past the advertised bound while
        // staying on the simple-edit fast path. Bail out so the caller falls back to a full
        // chunk rebuild, restoring the bounded-chunk invariant.
        if chunk.byte_len > DEFAULT_CHUNK_FORCE_MAX_BYTES {
            return None;
        }
        chunk.hash = chunk_hash_for_byte_range(rope.as_ref(), chunk.start_byte, chunk.end_byte);

        for chunk in chunks.iter_mut().skip(edit_chunk_index + 1) {
            chunk.start_byte = shift_usize(chunk.start_byte, byte_delta);
            chunk.end_byte = shift_usize(chunk.end_byte, byte_delta);
        }

        Some(Self {
            inner: ChunkedLineIndex {
                rope,
                lines,
                chunks,
            },
        })
    }

    /// Return the number of logical lines. Empty text has one line.
    pub fn line_count(&self) -> usize {
        self.inner.lines.len()
    }

    /// Return the full document byte length.
    pub fn len(&self) -> usize {
        self.inner.rope.len_bytes()
    }

    /// Returns `true` when the indexed text is empty.
    pub fn is_empty(&self) -> bool {
        self.inner.rope.len_bytes() == 0
    }

    /// Return the byte length of a logical line excluding CRLF/LF line endings.
    pub fn line_byte_len(&self, line: usize) -> TextResult<usize> {
        Ok(self.line(line)?.byte_len)
    }

    /// Return the UTF-16 length of a logical line excluding CRLF/LF line endings.
    pub fn line_utf16_len(&self, line: usize) -> TextResult<usize> {
        Ok(self.line(line)?.utf16_len)
    }

    /// Return the line-ending byte length for a line: `0`, `1`, or `2`.
    pub fn line_ending_bytes(&self, line: usize) -> TextResult<usize> {
        Ok(self.line(line)?.line_ending_bytes)
    }

    /// Return the chunk descriptors derived from the rope-backed text substrate.
    pub fn chunk_descriptors(&self) -> &[TextChunkDescriptor] {
        &self.inner.chunks
    }

    /// Materialize a bounded chunk by ordinal.
    pub fn chunk_text(&self, ordinal: usize) -> TextResult<String> {
        let chunk = self.chunk(ordinal)?;
        Ok(rope_string_from_byte_range(
            self.inner.rope.as_ref(),
            chunk.start_byte,
            chunk.end_byte,
        ))
    }

    /// Return a bounded slice for a logical line using an explicit byte budget.
    pub fn line_slice(&self, line: usize, max_bytes: usize) -> TextResult<TextLineSlice> {
        let metric = self.line(line)?;
        build_line_slice(self.inner.rope.as_ref(), metric, line, max_bytes)
    }

    /// Return the exact logical line range requested by a viewport using an explicit per-line
    /// byte budget.
    pub fn visible_line_slices(
        &self,
        start_line: usize,
        end_line_exclusive: usize,
        max_bytes_per_line: usize,
    ) -> TextResult<Vec<TextLineSlice>> {
        if start_line > end_line_exclusive {
            return Err(TextError::InvalidRange {
                start: start_line,
                end: end_line_exclusive,
            });
        }

        // Validate the requested range against the line table *before* allocating so a huge
        // `end_line_exclusive` cannot force a massive `Vec::with_capacity` ahead of the per-line
        // bounds check inside the loop.
        let line_count = self.line_count();
        if end_line_exclusive > line_count {
            return Err(TextError::LineOutOfBounds {
                line: end_line_exclusive,
                line_count,
            });
        }

        let mut slices = Vec::with_capacity(end_line_exclusive.saturating_sub(start_line));
        for line in start_line..end_line_exclusive {
            slices.push(self.line_slice(line, max_bytes_per_line)?);
        }
        Ok(slices)
    }

    /// Convert line + UTF-8 byte column to absolute byte offset.
    pub fn byte_offset(&self, pos: TextPosition) -> TextResult<usize> {
        let line = self.line(pos.line)?;
        if pos.column > line.byte_len {
            return Err(TextError::ColumnOutOfBounds {
                line: pos.line,
                column: pos.column,
                line_len: line.byte_len,
                unit: "byte",
            });
        }
        let offset = line.start_byte + pos.column;
        self.ensure_char_boundary(offset)?;
        Ok(offset)
    }

    /// Convert an absolute byte offset to line + UTF-8 byte column.
    pub fn position(&self, offset: usize) -> TextResult<TextPosition> {
        self.ensure_valid_offset(offset)?;
        self.ensure_char_boundary(offset)?;
        let line_index = self.line_for_offset(offset)?;
        let line = self.line(line_index)?;
        let column = offset.saturating_sub(line.start_byte).min(line.byte_len);
        Ok(TextPosition::new(line_index, column))
    }

    /// Convert an absolute byte offset to an LSP UTF-16 position.
    pub fn utf16_position(&self, offset: usize) -> TextResult<Utf16Position> {
        self.ensure_valid_offset(offset)?;
        self.ensure_char_boundary(offset)?;
        let line_index = self.line_for_offset(offset)?;
        let line = self.line(line_index)?;
        let column_end = offset.min(line.content_end_byte);
        Ok(Utf16Position::new(
            line_index,
            utf16_len_for_byte_range(self.inner.rope.as_ref(), line.start_byte, column_end),
        ))
    }

    /// Convert an LSP UTF-16 position to an absolute byte offset.
    pub fn byte_offset_from_utf16(&self, pos: Utf16Position) -> TextResult<usize> {
        let line = self.line(pos.line)?;
        if pos.character > line.utf16_len {
            return Err(TextError::ColumnOutOfBounds {
                line: pos.line,
                column: pos.character,
                line_len: line.utf16_len,
                unit: "utf16",
            });
        }

        let start_char = self.inner.rope.byte_to_char(line.start_byte);
        let end_char = self.inner.rope.byte_to_char(line.content_end_byte);
        let slice = self.inner.rope.slice(start_char..end_char);

        let mut utf16 = 0usize;
        let mut byte_offset = line.start_byte;
        for ch in slice.chars() {
            if utf16 == pos.character {
                return Ok(byte_offset);
            }

            let next_utf16 = utf16 + ch.len_utf16();
            if next_utf16 > pos.character {
                return Err(TextError::Utf16InsideSurrogatePair {
                    line: pos.line,
                    column: pos.character,
                });
            }

            utf16 = next_utf16;
            byte_offset += ch.len_utf8();
        }

        if utf16 == pos.character {
            Ok(line.content_end_byte)
        } else {
            Err(TextError::ColumnOutOfBounds {
                line: pos.line,
                column: pos.character,
                line_len: line.utf16_len,
                unit: "utf16",
            })
        }
    }

    /// Convert an absolute byte range to an LSP UTF-16 range.
    pub fn utf16_range(&self, start: usize, end: usize) -> TextResult<Utf16Range> {
        if start > end {
            return Err(TextError::InvalidRange { start, end });
        }
        Ok(Utf16Range::new(
            self.utf16_position(start)?,
            self.utf16_position(end)?,
        ))
    }

    /// Return an estimated memory footprint in bytes excluding rope storage.
    pub fn memory_footprint_bytes(&self) -> usize {
        self.inner.lines.len() * std::mem::size_of::<LineMetric>()
            + self.inner.chunks.len() * std::mem::size_of::<TextChunkDescriptor>()
            + self
                .inner
                .chunks
                .iter()
                .map(|chunk| chunk.hash.len())
                .sum::<usize>()
    }

    fn line(&self, line: usize) -> TextResult<&LineMetric> {
        self.inner
            .lines
            .get(line)
            .ok_or(TextError::LineOutOfBounds {
                line,
                line_count: self.inner.lines.len(),
            })
    }

    fn chunk(&self, chunk: usize) -> TextResult<&TextChunkDescriptor> {
        self.inner
            .chunks
            .get(chunk)
            .ok_or(TextError::ChunkOutOfBounds {
                chunk,
                chunk_count: self.inner.chunks.len(),
            })
    }

    fn line_for_offset(&self, offset: usize) -> TextResult<usize> {
        self.ensure_valid_offset(offset)?;
        line_index_for_offset(&self.inner.lines, offset).ok_or(TextError::LineOutOfBounds {
            line: 0,
            line_count: 0,
        })
    }

    fn line_for_offset_context(&self, offset: usize) -> usize {
        let len = self.len();
        if len == 0 {
            return 0;
        }
        let clamped = offset.min(len.saturating_sub(1));
        line_index_for_offset(&self.inner.lines, clamped).unwrap_or(0)
    }

    fn rebuild_start_chunk_index(&self, edit_start: usize) -> usize {
        if self.inner.chunks.is_empty() {
            return 0;
        }

        let context_offset = if self.is_empty() {
            0
        } else {
            edit_start
                .saturating_sub(1)
                .min(self.len().saturating_sub(1))
        };

        chunk_index_for_offset(&self.inner.chunks, context_offset).unwrap_or(0)
    }

    fn ensure_valid_offset(&self, offset: usize) -> TextResult<()> {
        let len = self.len();
        if offset > len {
            Err(TextError::ByteOffsetOutOfBounds { offset, len })
        } else {
            Ok(())
        }
    }

    fn ensure_char_boundary(&self, offset: usize) -> TextResult<()> {
        if is_char_boundary(self.inner.rope.as_ref(), offset) {
            Ok(())
        } else {
            Err(TextError::NotUtf8Boundary { offset })
        }
    }
}

/// A mutable text buffer backed by a [`Rope`].
///
/// Rope-backed edits are logarithmic in document size and avoid whole-buffer copies for typical
/// insertions and deletions. Full-source access remains available only while the buffer stays under
/// [`DEFAULT_FULL_CACHE_BYTE_BUDGET_BYTES`]; large buffers remain rope-backed, chunked, and
/// line-sliceable without materializing the entire document.
#[derive(Debug, Clone)]
pub struct TextBuffer {
    rope: Rope,
    line_index: Arc<LineIndex>,
    version: BufferVersion,
    full_text_cache: Option<String>,
    /// Lazily materialized full text used as a fallback for [`TextBuffer::text`] when the bounded
    /// compatibility cache is absent. Computed at most once per buffer state and invalidated
    /// whenever the buffer is mutated.
    materialized_full_text: OnceLock<Arc<String>>,
    allow_full_cache: bool,
}

impl PartialEq for TextBuffer {
    fn eq(&self, other: &Self) -> bool {
        // The lazily materialized full-text fallback is a pure cache derived from the rope; it is
        // intentionally excluded so two buffers with identical content compare equal regardless of
        // whether either has materialized its degraded-path text.
        self.rope == other.rope
            && self.line_index == other.line_index
            && self.version == other.version
            && self.full_text_cache == other.full_text_cache
            && self.allow_full_cache == other.allow_full_cache
    }
}

impl Eq for TextBuffer {}

impl TextBuffer {
    /// Create a new buffer from a string.
    pub fn new(text: impl Into<String>) -> Self {
        Self::with_version(text, BufferVersion(0))
    }

    /// Create a new buffer with an explicit buffer version.
    pub fn with_version(text: impl Into<String>, version: BufferVersion) -> Self {
        Self::try_with_version(text, version).expect("text buffer construction should not fail")
    }

    /// Try to create a new buffer with an explicit buffer version.
    pub fn try_with_version(text: impl Into<String>, version: BufferVersion) -> TextResult<Self> {
        Self::try_with_version_and_cache_policy(text, version, true)
    }

    /// Try to create a new buffer with an explicit buffer version and full-cache policy.
    pub fn try_with_version_and_cache_policy(
        text: impl Into<String>,
        version: BufferVersion,
        allow_full_cache: bool,
    ) -> TextResult<Self> {
        let text = text.into();
        let rope = Rope::from_str(&text);
        Self::try_from_rope_with_cache_policy_and_text(rope, version, allow_full_cache, Some(text))
    }

    /// Try to create a new buffer from an existing rope with budget-aware full-cache retention.
    pub fn try_from_rope(rope: Rope, version: BufferVersion) -> TextResult<Self> {
        Self::try_from_rope_with_cache_policy(rope, version, true)
    }

    /// Try to create a new buffer from an existing rope with an explicit full-cache policy.
    pub fn try_from_rope_with_cache_policy(
        rope: Rope,
        version: BufferVersion,
        allow_full_cache: bool,
    ) -> TextResult<Self> {
        Self::try_from_rope_with_cache_policy_and_text(rope, version, allow_full_cache, None)
    }

    fn try_from_rope_with_cache_policy_and_text(
        rope: Rope,
        version: BufferVersion,
        allow_full_cache: bool,
        source_text: Option<String>,
    ) -> TextResult<Self> {
        let line_index = Arc::new(LineIndex::from_rope(Arc::new(rope.clone())));
        let full_text_cache = if allow_full_cache {
            match source_text {
                Some(text) if text.len() <= DEFAULT_FULL_CACHE_BYTE_BUDGET_BYTES => Some(text),
                Some(_) => None,
                None => maybe_full_text_cache(&rope)?,
            }
        } else {
            None
        };

        Ok(Self {
            rope,
            line_index,
            version,
            full_text_cache,
            materialized_full_text: OnceLock::new(),
            allow_full_cache,
        })
    }

    /// Update whether this buffer may retain a bounded full-text compatibility cache.
    pub fn set_full_cache_policy(&mut self, allow_full_cache: bool) -> TextResult<()> {
        self.allow_full_cache = allow_full_cache;
        self.full_text_cache = if allow_full_cache {
            maybe_full_text_cache(&self.rope)?
        } else {
            None
        };
        // Policy changes do not alter content, but keep the fallback in lockstep with the bounded
        // cache so a freshly enabled bounded cache is always preferred by `text()`.
        self.materialized_full_text = OnceLock::new();
        Ok(())
    }

    /// Return the full text.
    ///
    /// When the buffer retains a bounded compatibility full-text cache the cached string is returned
    /// directly. For large or degraded buffers that dropped the bounded cache, the full text is
    /// materialized from the rope on first use and retained until the next mutation, so this method
    /// never panics. Prefer [`TextBuffer::try_full_text`] (or the bounded [`TextBuffer::line_slice`]
    /// / [`TextBuffer::chunk_text`] APIs) when the additional materialized retention is undesirable.
    pub fn text(&self) -> &str {
        if let Some(text) = self.full_text_cache.as_deref() {
            return text;
        }
        self.materialized_full_text
            .get_or_init(|| Arc::new(self.rope.to_string()))
            .as_str()
    }

    /// Return the cached full text when the buffer remains within the full-cache budget.
    pub fn try_full_text(&self) -> TextResult<&str> {
        self.full_text_cache
            .as_deref()
            .ok_or(TextError::FullCacheBudgetExceeded {
                byte_len: self.len(),
                budget: DEFAULT_FULL_CACHE_BYTE_BUDGET_BYTES,
            })
    }

    /// Return the chunk descriptors backing the current rope state.
    pub fn chunk_descriptors(&self) -> &[TextChunkDescriptor] {
        self.line_index.chunk_descriptors()
    }

    /// Materialize a single bounded chunk by ordinal.
    pub fn chunk_text(&self, ordinal: usize) -> TextResult<String> {
        self.line_index.chunk_text(ordinal)
    }

    /// Return a bounded slice for a single logical line.
    pub fn line_slice(&self, line: usize) -> TextResult<TextLineSlice> {
        self.line_slice_with_limit(line, DEFAULT_LINE_SLICE_MAX_BYTES)
    }

    /// Return a bounded slice for a single logical line with an explicit byte budget.
    pub fn line_slice_with_limit(
        &self,
        line: usize,
        max_bytes: usize,
    ) -> TextResult<TextLineSlice> {
        self.line_index.line_slice(line, max_bytes)
    }

    /// Return the exact logical line range requested by a viewport using the default per-line
    /// slice budget.
    pub fn visible_line_slices(
        &self,
        start_line: usize,
        end_line_exclusive: usize,
    ) -> TextResult<Vec<TextLineSlice>> {
        self.visible_line_slices_with_limit(
            start_line,
            end_line_exclusive,
            DEFAULT_LINE_SLICE_MAX_BYTES,
        )
    }

    /// Return the exact logical line range requested by a viewport with an explicit per-line
    /// slice budget.
    pub fn visible_line_slices_with_limit(
        &self,
        start_line: usize,
        end_line_exclusive: usize,
        max_bytes_per_line: usize,
    ) -> TextResult<Vec<TextLineSlice>> {
        self.line_index
            .visible_line_slices(start_line, end_line_exclusive, max_bytes_per_line)
    }

    /// Return the current buffer version.
    pub fn version(&self) -> BufferVersion {
        self.version
    }

    /// Set the current buffer version.
    pub fn set_version(&mut self, version: BufferVersion) {
        self.version = version;
    }

    /// Return the byte length of the text.
    pub fn len(&self) -> usize {
        self.rope.len_bytes()
    }

    /// Returns `true` if the buffer contains no text.
    pub fn is_empty(&self) -> bool {
        self.rope.len_bytes() == 0
    }

    /// Return the number of logical lines. Empty buffers have one line.
    pub fn line_count(&self) -> usize {
        self.line_index.line_count()
    }

    /// Return the current line index.
    pub fn line_index(&self) -> &LineIndex {
        &self.line_index
    }

    /// Convert a [`TextPosition`] to a byte offset.
    pub fn byte_offset(&self, pos: TextPosition) -> Option<usize> {
        self.try_byte_offset(pos).ok()
    }

    /// Convert a [`TextPosition`] to a byte offset with detailed error reporting.
    pub fn try_byte_offset(&self, pos: TextPosition) -> TextResult<usize> {
        self.line_index.byte_offset(pos)
    }

    /// Convert a byte offset to a [`TextPosition`].
    pub fn position(&self, offset: usize) -> Option<TextPosition> {
        self.try_position(offset).ok()
    }

    /// Convert a byte offset to a [`TextPosition`] with detailed error reporting.
    pub fn try_position(&self, offset: usize) -> TextResult<TextPosition> {
        self.line_index.position(offset)
    }

    /// Convert a byte offset to an LSP UTF-16 position.
    pub fn utf16_position(&self, offset: usize) -> TextResult<Utf16Position> {
        self.line_index.utf16_position(offset)
    }

    /// Convert an LSP UTF-16 position to a byte offset.
    pub fn byte_offset_from_utf16(&self, pos: Utf16Position) -> TextResult<usize> {
        self.line_index.byte_offset_from_utf16(pos)
    }

    /// Insert `text` at the given byte offset.
    pub fn insert(&mut self, offset: usize, text: &str) -> Option<()> {
        self.try_insert(offset, text).ok()
    }

    /// Insert `text` at the given byte offset with detailed error reporting.
    pub fn try_insert(&mut self, offset: usize, text: &str) -> TextResult<()> {
        self.try_replace_range(offset, offset, text)
    }

    /// Delete the byte range `[start, end)`.
    pub fn delete_range(&mut self, start: usize, end: usize) -> Option<()> {
        self.try_delete_range(start, end).ok()
    }

    /// Delete the byte range `[start, end)` with detailed error reporting.
    pub fn try_delete_range(&mut self, start: usize, end: usize) -> TextResult<()> {
        self.try_replace_range(start, end, "")
    }

    /// Replace the byte range `[start, end)` with `text`.
    pub fn replace_range(&mut self, start: usize, end: usize, text: &str) -> Option<()> {
        self.try_replace_range(start, end, text).ok()
    }

    /// Replace the byte range `[start, end)` with `text` and return detailed errors.
    pub fn try_replace_range(&mut self, start: usize, end: usize, text: &str) -> TextResult<()> {
        self.validate_range(start, end)?;
        let rebuild_chunk_index = self.line_index.rebuild_start_chunk_index(start);
        let new_len = self
            .len()
            .saturating_sub(end.saturating_sub(start))
            .saturating_add(text.len());
        let removed_utf16_len = utf16_len_for_byte_range(&self.rope, start, end);
        let simple_edit = !text.contains('\n')
            && !text.contains('\r')
            && !self
                .rope
                .slice(self.rope.byte_to_char(start)..self.rope.byte_to_char(end))
                .chars()
                .any(|ch| ch == '\n' || ch == '\r');

        let start_char = self.rope.byte_to_char(start);
        let end_char = self.rope.byte_to_char(end);
        self.rope.remove(start_char..end_char);
        self.rope.insert(start_char, text);

        if self.allow_full_cache && new_len <= DEFAULT_FULL_CACHE_BYTE_BUDGET_BYTES {
            match self.full_text_cache.as_mut() {
                Some(cache) => cache.replace_range(start..end, text),
                None => self.full_text_cache = Some(self.rope.to_string()),
            }
        } else {
            self.full_text_cache = None;
        }

        // The lazily materialized degraded-path fallback is now stale; drop it so the next
        // `text()` call rematerializes from the mutated rope.
        self.materialized_full_text = OnceLock::new();

        if simple_edit {
            let rope = Arc::new(self.rope.clone());
            if let Some(line_index) =
                self.line_index
                    .rebuild_from_simple_edit(rope, start, end, text, removed_utf16_len)
            {
                self.line_index = Arc::new(line_index);
                return Ok(());
            }
        }

        self.refresh_cache_and_index(rebuild_chunk_index)?;
        Ok(())
    }

    /// Apply a [`TextEdit`] to the buffer.
    pub fn apply_edit(&mut self, edit: &TextEdit) -> Option<()> {
        self.try_apply_edit(edit).ok()
    }

    /// Apply a [`TextEdit`] to the buffer with detailed error reporting.
    pub fn try_apply_edit(&mut self, edit: &TextEdit) -> TextResult<()> {
        let start = self.try_byte_offset(edit.range.start)?;
        let end = self.try_byte_offset(edit.range.end)?;
        self.try_replace_range(start, end, &edit.new_text)
    }

    /// Create an immutable [`TextSnapshot`] of the current contents.
    pub fn snapshot(&self) -> TextSnapshot {
        self.snapshot_with_retention(RetentionPinReason::CurrentBuffer)
    }

    /// Try to create an immutable [`TextSnapshot`] of the current contents.
    pub fn try_snapshot(&self) -> TextResult<TextSnapshot> {
        self.try_snapshot_with_retention(RetentionPinReason::CurrentBuffer)
    }

    /// Create an immutable [`TextSnapshot`] with an explicit retention reason.
    pub fn snapshot_with_retention(&self, reason: RetentionPinReason) -> TextSnapshot {
        self.try_snapshot_with_retention(reason)
            .expect("text buffer snapshot construction should not fail")
    }

    /// Try to create an immutable [`TextSnapshot`] with an explicit retention reason.
    pub fn try_snapshot_with_retention(
        &self,
        reason: RetentionPinReason,
    ) -> TextResult<TextSnapshot> {
        let full_text_cache = self.full_text_cache.clone().map(Arc::new);
        let line_index = Arc::clone(&self.line_index);
        TextSnapshot::from_rope_parts_with_line_index(
            Arc::new(self.rope.clone()),
            self.version,
            reason,
            full_text_cache,
            line_index,
        )
    }

    /// Return the estimated memory footprint of the buffer.
    pub fn memory_footprint_bytes(&self) -> usize {
        estimate_rope_memory(
            &self.rope,
            self.full_text_cache.as_ref().map_or(0, String::len),
            &self.line_index,
        )
    }

    fn validate_range(&self, start: usize, end: usize) -> TextResult<()> {
        if start > end {
            return Err(TextError::InvalidRange { start, end });
        }
        self.line_index.ensure_valid_offset(start)?;
        self.line_index.ensure_valid_offset(end)?;
        self.line_index.ensure_char_boundary(start)?;
        self.line_index.ensure_char_boundary(end)?;
        Ok(())
    }

    fn refresh_cache_and_index(&mut self, rebuild_chunk_index: usize) -> TextResult<()> {
        self.line_index = Arc::new(
            self.line_index
                .rebuild_from_chunk(Arc::new(self.rope.clone()), rebuild_chunk_index),
        );
        Ok(())
    }
}

impl Default for TextBuffer {
    fn default() -> Self {
        Self::new("")
    }
}

impl From<String> for TextBuffer {
    fn from(text: String) -> Self {
        Self::new(text)
    }
}

impl From<&str> for TextBuffer {
    fn from(text: &str) -> Self {
        Self::new(text)
    }
}

fn build_line_slice(
    rope: &Rope,
    metric: &LineMetric,
    line: usize,
    max_bytes: usize,
) -> TextResult<TextLineSlice> {
    let slice_end_byte = bounded_slice_end_byte(rope, metric, max_bytes);
    let text = rope_string_from_byte_range(rope, metric.start_byte, slice_end_byte);
    Ok(TextLineSlice {
        line,
        line_start_byte: metric.start_byte,
        slice_end_byte,
        line_content_byte_len: metric.byte_len,
        utf8_byte_len: text.len(),
        utf16_len: text.encode_utf16().count(),
        line_ending_bytes: metric.line_ending_bytes,
        truncated: slice_end_byte < metric.content_end_byte,
        text,
    })
}

fn bounded_slice_end_byte(rope: &Rope, metric: &LineMetric, max_bytes: usize) -> usize {
    if max_bytes >= metric.byte_len {
        return metric.content_end_byte;
    }

    let candidate = floor_char_boundary(rope, metric.start_byte.saturating_add(max_bytes))
        .min(metric.content_end_byte)
        .max(metric.start_byte);

    if candidate > metric.start_byte {
        candidate
    } else if metric.content_end_byte > metric.start_byte && max_bytes > 0 {
        next_char_boundary_after(rope, metric.start_byte).min(metric.content_end_byte)
    } else {
        metric.start_byte
    }
}

fn scan_line_metrics_from_byte(rope: &Rope, start_byte: usize) -> Vec<LineMetric> {
    let total_bytes = rope.len_bytes();
    let start_char = rope.byte_to_char(start_byte.min(total_bytes));
    let slice = rope.slice(start_char..rope.len_chars());

    let mut lines = Vec::new();
    let mut line_start = start_byte.min(total_bytes);
    let mut current_utf16 = 0usize;
    let mut absolute = start_byte.min(total_bytes);
    let mut pending_cr: Option<usize> = None;

    for chunk in slice.chunks() {
        if chunk.is_ascii() {
            let bytes = chunk.as_bytes();

            if pending_cr.is_none() && memchr(b'\r', bytes).is_none() {
                let mut start = 0usize;
                while let Some(newline_rel) = memchr(b'\n', &bytes[start..]) {
                    let newline = start + newline_rel;
                    let offset = absolute + newline;
                    current_utf16 += newline.saturating_sub(start);
                    push_line_metric(&mut lines, line_start, offset, offset + 1, 1, current_utf16);
                    line_start = offset + 1;
                    current_utf16 = 0;
                    start = newline + 1;
                }

                current_utf16 += bytes.len().saturating_sub(start);
                absolute += bytes.len();
                continue;
            }

            let mut i = 0usize;
            while i < bytes.len() {
                let byte = bytes[i];
                let offset = absolute + i;

                if let Some(cr_offset) = pending_cr {
                    if byte == b'\n' {
                        push_line_metric(
                            &mut lines,
                            line_start,
                            cr_offset,
                            offset + 1,
                            2,
                            current_utf16,
                        );
                        line_start = offset + 1;
                        current_utf16 = 0;
                        pending_cr = None;
                        i += 1;
                        continue;
                    }

                    push_line_metric(
                        &mut lines,
                        line_start,
                        cr_offset,
                        cr_offset + 1,
                        1,
                        current_utf16,
                    );
                    line_start = cr_offset + 1;
                    current_utf16 = 0;
                    pending_cr = None;
                }

                match byte {
                    b'\r' => {
                        pending_cr = Some(offset);
                        i += 1;
                    }
                    b'\n' => {
                        push_line_metric(
                            &mut lines,
                            line_start,
                            offset,
                            offset + 1,
                            1,
                            current_utf16,
                        );
                        line_start = offset + 1;
                        current_utf16 = 0;
                        i += 1;
                    }
                    _ => {
                        current_utf16 += 1;
                        i += 1;
                    }
                }
            }

            absolute += bytes.len();
            continue;
        }

        for (rel_offset, ch) in chunk.char_indices() {
            let offset = absolute + rel_offset;

            if let Some(cr_offset) = pending_cr {
                if ch == '\n' {
                    push_line_metric(
                        &mut lines,
                        line_start,
                        cr_offset,
                        offset + ch.len_utf8(),
                        2,
                        current_utf16,
                    );
                    line_start = offset + ch.len_utf8();
                    current_utf16 = 0;
                    pending_cr = None;
                    continue;
                }

                push_line_metric(
                    &mut lines,
                    line_start,
                    cr_offset,
                    cr_offset + 1,
                    1,
                    current_utf16,
                );
                line_start = cr_offset + 1;
                current_utf16 = 0;
                pending_cr = None;
            }

            match ch {
                '\r' => {
                    pending_cr = Some(offset);
                }
                '\n' => {
                    push_line_metric(
                        &mut lines,
                        line_start,
                        offset,
                        offset + ch.len_utf8(),
                        1,
                        current_utf16,
                    );
                    line_start = offset + ch.len_utf8();
                    current_utf16 = 0;
                }
                _ => {
                    current_utf16 += ch.len_utf16();
                }
            }
        }

        absolute += chunk.len();
    }

    if let Some(cr_offset) = pending_cr {
        push_line_metric(
            &mut lines,
            line_start,
            cr_offset,
            cr_offset + 1,
            1,
            current_utf16,
        );
        line_start = cr_offset + 1;
        current_utf16 = 0;
    }

    push_line_metric(
        &mut lines,
        line_start,
        total_bytes,
        total_bytes,
        0,
        current_utf16,
    );
    lines
}

fn push_line_metric(
    lines: &mut Vec<LineMetric>,
    start_byte: usize,
    content_end_byte: usize,
    end_byte: usize,
    line_ending_bytes: usize,
    utf16_len: usize,
) {
    lines.push(LineMetric {
        start_byte,
        content_end_byte,
        end_byte,
        byte_len: content_end_byte.saturating_sub(start_byte),
        utf16_len,
        line_ending_bytes,
    });
}

fn build_chunk_descriptors(
    rope: &Rope,
    lines: &[LineMetric],
    start_byte: usize,
    start_ordinal: usize,
) -> Vec<TextChunkDescriptor> {
    let total_bytes = rope.len_bytes();
    if total_bytes == 0 || lines.is_empty() || start_byte >= total_bytes {
        return Vec::new();
    }

    let mut descriptors = Vec::new();
    let mut chunk_start = start_byte;
    let mut ordinal = start_ordinal;

    while chunk_start < total_bytes {
        let start_line =
            line_index_for_offset(lines, chunk_start).unwrap_or_else(|| lines.len() - 1);
        let min_preferred = chunk_start.saturating_add(
            DEFAULT_CHUNK_TARGET_BYTES.saturating_sub(DEFAULT_CHUNK_BOUNDARY_WINDOW_BYTES),
        );
        let target = chunk_start.saturating_add(DEFAULT_CHUNK_TARGET_BYTES);
        let force_max = chunk_start
            .saturating_add(DEFAULT_CHUNK_FORCE_MAX_BYTES)
            .min(total_bytes);

        let mut preferred_boundary: Option<(usize, usize)> = None;
        let mut fallback_boundary: Option<(usize, usize)> = None;
        let mut line_idx = start_line;

        while line_idx < lines.len() {
            let boundary = lines[line_idx].end_byte;
            if boundary <= chunk_start {
                line_idx += 1;
                continue;
            }
            if boundary > force_max {
                break;
            }

            if boundary >= min_preferred && boundary <= target {
                preferred_boundary = Some((boundary, line_idx));
            } else if boundary > target {
                fallback_boundary = Some((boundary, line_idx));
                break;
            }

            line_idx += 1;
        }

        let (chunk_end, end_line) = if let Some(boundary) = preferred_boundary {
            boundary
        } else if let Some(boundary) = fallback_boundary {
            boundary
        } else {
            let mut forced_end = floor_char_boundary(rope, force_max);
            if forced_end <= chunk_start {
                forced_end = next_char_boundary_after(rope, chunk_start).min(total_bytes);
            }
            let context = forced_end
                .saturating_sub(1)
                .min(total_bytes.saturating_sub(1));
            let end_line = line_index_for_offset(lines, context).unwrap_or(start_line);
            (forced_end, end_line)
        };

        descriptors.push(TextChunkDescriptor {
            ordinal,
            start_byte: chunk_start,
            end_byte: chunk_end,
            byte_len: chunk_end.saturating_sub(chunk_start),
            start_line,
            end_line,
            hash: chunk_hash_for_byte_range(rope, chunk_start, chunk_end),
        });

        chunk_start = chunk_end;
        ordinal += 1;
    }

    descriptors
}

fn line_index_for_offset(lines: &[LineMetric], offset: usize) -> Option<usize> {
    if lines.is_empty() {
        return None;
    }

    match lines.binary_search_by(|line| {
        if offset < line.start_byte {
            Ordering::Greater
        } else if offset > line.end_byte {
            Ordering::Less
        } else {
            Ordering::Equal
        }
    }) {
        Ok(mut idx) => {
            while idx + 1 < lines.len()
                && !lines[idx].contains_offset(offset, idx + 1 == lines.len())
            {
                idx += 1;
            }
            Some(idx)
        }
        Err(idx) if idx > 0 => Some(idx - 1),
        Err(_) => Some(0),
    }
}

fn chunk_index_for_offset(chunks: &[TextChunkDescriptor], offset: usize) -> Option<usize> {
    if chunks.is_empty() {
        return None;
    }

    match chunks.binary_search_by(|chunk| {
        if offset < chunk.start_byte {
            Ordering::Greater
        } else if offset >= chunk.end_byte {
            Ordering::Less
        } else {
            Ordering::Equal
        }
    }) {
        Ok(mut idx) => {
            while idx + 1 < chunks.len() && offset >= chunks[idx].end_byte {
                idx += 1;
            }
            Some(idx)
        }
        Err(idx) if idx > 0 => Some(idx - 1),
        Err(_) => Some(0),
    }
}

fn shift_usize(base: usize, delta: isize) -> usize {
    if delta >= 0 {
        base.saturating_add(delta as usize)
    } else {
        base.saturating_sub((-delta) as usize)
    }
}

fn utf16_len_for_byte_range(rope: &Rope, start_byte: usize, end_byte: usize) -> usize {
    if start_byte >= end_byte {
        return 0;
    }

    let start_char = rope.byte_to_char(start_byte);
    let end_char = rope.byte_to_char(end_byte);
    rope.slice(start_char..end_char)
        .chars()
        .map(char::len_utf16)
        .sum()
}

fn rope_string_from_byte_range(rope: &Rope, start_byte: usize, end_byte: usize) -> String {
    if start_byte >= end_byte {
        return String::new();
    }

    let start_char = rope.byte_to_char(start_byte);
    let end_char = rope.byte_to_char(end_byte);
    rope.slice(start_char..end_char).to_string()
}

fn floor_char_boundary(rope: &Rope, offset: usize) -> usize {
    if offset >= rope.len_bytes() {
        return rope.len_bytes();
    }

    let char_idx = rope.byte_to_char(offset);
    let boundary = rope.char_to_byte(char_idx);
    if boundary == offset { offset } else { boundary }
}

fn next_char_boundary_after(rope: &Rope, offset: usize) -> usize {
    let char_idx = rope.byte_to_char(offset);
    rope.char_to_byte((char_idx + 1).min(rope.len_chars()))
}

fn is_char_boundary(rope: &Rope, offset: usize) -> bool {
    if offset > rope.len_bytes() {
        return false;
    }
    let char_idx = rope.byte_to_char(offset);
    rope.char_to_byte(char_idx) == offset
}

#[allow(dead_code)]
fn content_hash(text: &str) -> String {
    hash_with_domain(b"legion-text:content:v1\0", text.as_bytes())
}

fn chunk_hash_for_byte_range(rope: &Rope, start_byte: usize, end_byte: usize) -> String {
    if start_byte >= end_byte {
        return hash_with_domain(b"legion-text:chunk:v1\0", b"");
    }

    let start_char = rope.byte_to_char(start_byte);
    let end_char = rope.byte_to_char(end_byte);
    let mut hasher = Sha256::new();
    hasher.update(b"legion-text:chunk:v1\0");
    for chunk in rope.slice(start_char..end_char).chunks() {
        hasher.update(chunk.as_bytes());
    }
    format!("sha256:{}", hex::encode(hasher.finalize()))
}

fn snapshot_content_hash(chunks: &[TextChunkDescriptor]) -> String {
    use std::fmt::Write as _;

    let mut serialized = String::new();
    for chunk in chunks {
        let _ = write!(
            serialized,
            "{}:{}:{}:{}:{}:{};",
            chunk.ordinal,
            chunk.start_byte,
            chunk.end_byte,
            chunk.start_line,
            chunk.end_line,
            chunk.hash,
        );
    }
    hash_with_domain(b"legion-text:snapshot:v1\0", serialized.as_bytes())
}

fn hash_with_domain(domain: &[u8], bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(domain);
    hasher.update(bytes);
    format!("sha256:{}", hex::encode(hasher.finalize()))
}

fn estimate_rope_memory(
    rope: &Rope,
    full_cache_bytes: usize,
    line_index_bytes: &LineIndex,
) -> usize {
    rope.len_bytes()
        + full_cache_bytes
        + line_index_bytes.memory_footprint_bytes()
        + rope.len_lines() * std::mem::size_of::<usize>() * 4
        + DEFAULT_LEAF_TARGET_BYTES
}

fn maybe_full_text_cache(rope: &Rope) -> TextResult<Option<String>> {
    if rope.len_bytes() > DEFAULT_FULL_CACHE_BYTE_BUDGET_BYTES {
        Ok(None)
    } else {
        let text = rope.to_string();
        if text.len() > DEFAULT_FULL_CACHE_BYTE_BUDGET_BYTES {
            Err(TextError::FullCacheBudgetExceeded {
                byte_len: text.len(),
                budget: DEFAULT_FULL_CACHE_BYTE_BUDGET_BYTES,
            })
        } else {
            Ok(Some(text))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use quickcheck::quickcheck;

    #[test]
    fn text_position_display() {
        assert_eq!(TextPosition::new(1, 5).to_string(), "1:5");
    }

    #[test]
    fn text_range_empty() {
        let r = TextRange::empty(TextPosition::new(2, 4));
        assert!(r.is_empty());
    }

    #[test]
    fn snapshot_descriptor_has_required_metadata() {
        let snap = TextSnapshot::new("hello\nworld");
        assert_eq!(snap.len(), 11);
        assert_eq!(snap.line_count(), 2);
        assert!(snap.content_hash().starts_with("sha256:"));
        assert!(snap.memory_footprint_bytes() >= snap.len());
        assert_eq!(snap.descriptor().buffer_version, BufferVersion(0));
    }

    #[test]
    fn snapshot_clone_is_cheap_and_immutable() {
        let mut buf = TextBuffer::new("original");
        let snap = buf.snapshot();
        let clone = snap.clone();
        assert_eq!(clone.text(), "original");
        assert!(Arc::ptr_eq(
            snap.full_text_cache.as_ref().expect("small snapshot cache"),
            clone
                .full_text_cache
                .as_ref()
                .expect("small snapshot cache"),
        ));
        buf.insert(8, " changed").unwrap();
        assert_eq!(snap.text(), "original");
        assert_eq!(buf.text(), "original changed");
    }

    #[test]
    fn buffer_insert_delete_replace() {
        let mut buf = TextBuffer::new("hello world");
        assert!(buf.insert(5, ",").is_some());
        assert_eq!(buf.text(), "hello, world");
        assert!(buf.delete_range(5, 6).is_some());
        assert_eq!(buf.text(), "hello world");
        assert!(buf.replace_range(6, 11, "Rust").is_some());
        assert_eq!(buf.text(), "hello Rust");
    }

    #[test]
    fn buffer_rejects_non_boundary_edits() {
        let mut buf = TextBuffer::new("é");
        assert!(buf.insert(1, "x").is_none());
        assert!(matches!(
            buf.try_insert(1, "x"),
            Err(TextError::NotUtf8Boundary { offset: 1 })
        ));
    }

    #[test]
    fn position_roundtrip_multibyte_columns_are_bytes() {
        let buf = TextBuffer::new("aé\n🦀z");
        assert_eq!(buf.byte_offset(TextPosition::new(0, 0)), Some(0));
        assert_eq!(buf.byte_offset(TextPosition::new(0, 1)), Some(1));
        assert_eq!(buf.byte_offset(TextPosition::new(0, 3)), Some(3));
        assert_eq!(buf.position(4), Some(TextPosition::new(1, 0)));
        assert_eq!(buf.position(8), Some(TextPosition::new(1, 4)));
    }

    #[test]
    fn crlf_is_single_line_ending_for_lsp() {
        let idx = LineIndex::new("ab\r\ncd");
        assert_eq!(idx.line_count(), 2);
        assert_eq!(idx.line_byte_len(0).unwrap(), 2);
        assert_eq!(idx.line_ending_bytes(0).unwrap(), 2);
        assert_eq!(idx.byte_offset(TextPosition::new(1, 0)).unwrap(), 4);
        assert_eq!(idx.utf16_position(4).unwrap(), Utf16Position::new(1, 0));
    }

    #[test]
    fn utf16_golden_surrogate_pairs() {
        let idx = LineIndex::new("a🦀b");
        assert_eq!(idx.utf16_position(0).unwrap(), Utf16Position::new(0, 0));
        assert_eq!(idx.utf16_position(1).unwrap(), Utf16Position::new(0, 1));
        assert_eq!(idx.utf16_position(5).unwrap(), Utf16Position::new(0, 3));
        assert_eq!(idx.utf16_position(6).unwrap(), Utf16Position::new(0, 4));
        assert_eq!(
            idx.byte_offset_from_utf16(Utf16Position::new(0, 3))
                .unwrap(),
            5
        );
        assert!(matches!(
            idx.byte_offset_from_utf16(Utf16Position::new(0, 2)),
            Err(TextError::Utf16InsideSurrogatePair { .. })
        ));
    }

    #[test]
    fn utf16_range_golden() {
        let idx = LineIndex::new("a🦀\r\nb");
        let range = idx.utf16_range(1, 7).unwrap();
        assert_eq!(range.start, Utf16Position::new(0, 1));
        assert_eq!(range.end, Utf16Position::new(1, 0));
    }

    #[test]
    fn property_edits_match_string_model_for_ascii() {
        fn prop(seed: String, replacement: String, a: u8, b: u8) -> bool {
            let mut model: String = seed.chars().filter(|c| c.is_ascii()).take(128).collect();
            let replacement: String = replacement
                .chars()
                .filter(|c| c.is_ascii() && *c != '\0')
                .take(32)
                .collect();
            let mut rope = TextBuffer::new(model.clone());
            let len = model.len();
            let start = if len == 0 {
                0
            } else {
                (a as usize) % (len + 1)
            };
            let end = if len == 0 {
                0
            } else {
                (b as usize) % (len + 1)
            };
            let (start, end) = if start <= end {
                (start, end)
            } else {
                (end, start)
            };

            model.replace_range(start..end, &replacement);
            rope.replace_range(start, end, &replacement).unwrap();
            rope.text() == model
        }

        quickcheck(prop as fn(String, String, u8, u8) -> bool);
    }

    #[test]
    fn opening_larger_than_budget_uses_degraded_cache_free_mode() {
        let text = "x".repeat(DEFAULT_FULL_CACHE_BYTE_BUDGET_BYTES + 1);
        let buf = TextBuffer::try_with_version(text, BufferVersion(7)).unwrap();
        assert_eq!(buf.version(), BufferVersion(7));
        assert!(matches!(
            buf.try_full_text(),
            Err(TextError::FullCacheBudgetExceeded { .. })
        ));
        let chunks = buf.chunk_descriptors();
        assert!(chunks.len() > 1);
        assert_eq!(chunks[0].start_byte, 0);
        assert_eq!(chunks.last().unwrap().end_byte, buf.len());
        for (ordinal, chunk) in chunks.iter().enumerate() {
            assert_eq!(chunk.ordinal, ordinal);
            assert_eq!(chunk.byte_len, chunk.end_byte - chunk.start_byte);
            assert!(chunk.byte_len <= DEFAULT_CHUNK_FORCE_MAX_BYTES);
            assert!(chunk.hash.starts_with("sha256:"));
            if ordinal > 0 {
                assert_eq!(chunk.start_byte, chunks[ordinal - 1].end_byte);
            }
        }

        let snapshot = buf.try_snapshot().unwrap();
        assert!(matches!(
            snapshot.try_full_text(),
            Err(TextError::FullCacheBudgetExceeded { .. })
        ));
        assert_eq!(snapshot.len(), DEFAULT_FULL_CACHE_BYTE_BUDGET_BYTES + 1);
        assert_eq!(snapshot.chunk_descriptors().len(), chunks.len());
    }

    #[test]
    fn large_snapshot_line_slices_and_chunks_are_bounded_by_default() {
        let long_line = "m".repeat(DEFAULT_FULL_CACHE_BYTE_BUDGET_BYTES + 1024);
        let text = format!("head\n{long_line}\ntail\n");
        let snapshot = TextSnapshot::try_new(text).unwrap();

        assert!(matches!(
            snapshot.try_full_text(),
            Err(TextError::FullCacheBudgetExceeded { .. })
        ));
        assert!(snapshot.chunk_descriptors().len() > 1);

        let first_chunk = &snapshot.chunk_descriptors()[0];
        let first_chunk_text = snapshot.chunk_text(first_chunk.ordinal).unwrap();
        assert_eq!(first_chunk_text.len(), first_chunk.byte_len);
        assert!(first_chunk_text.len() <= DEFAULT_CHUNK_FORCE_MAX_BYTES);

        let slices = snapshot.visible_line_slices_with_limit(0, 3, 32).unwrap();
        let payload_bytes = slices.iter().map(|slice| slice.text.len()).sum::<usize>();

        assert_eq!(slices.len(), 3);
        assert_eq!(slices[0].text, "head");
        assert_eq!(slices[1].line, 1);
        assert!(slices[1].truncated);
        assert!(slices[1].utf8_byte_len <= 32);
        assert_eq!(slices[2].text, "tail");
        assert!(payload_bytes < 128);
        assert!(payload_bytes < snapshot.len() / 1024);
    }

    #[test]
    fn large_snapshot_can_materialize_save_payload_from_chunks() {
        let text = format!(
            "head\n{}\ntail\n",
            "x".repeat(DEFAULT_FULL_CACHE_BYTE_BUDGET_BYTES + 1024)
        );
        let snapshot = TextSnapshot::try_new(text.clone()).unwrap();

        assert!(matches!(
            snapshot.try_full_text(),
            Err(TextError::FullCacheBudgetExceeded { .. })
        ));

        let materialized = snapshot
            .materialize_full_text_from_chunks()
            .expect("chunk materialization should reconstruct text exactly");
        assert_eq!(materialized, text);
        assert_eq!(materialized.len(), snapshot.len());
    }

    #[test]
    fn line_slice_can_span_multiple_chunks() {
        let long_line = format!("{}🦀tail", "a".repeat(DEFAULT_CHUNK_TARGET_BYTES + 128));
        let buf = TextBuffer::new(format!("head\n{long_line}\nend"));
        let slice = buf.line_slice_with_limit(1, usize::MAX).unwrap();
        assert_eq!(slice.text, long_line);
        assert!(!slice.truncated);
        assert!(buf.chunk_descriptors().len() >= 2);
    }

    #[test]
    fn huge_single_line_files_are_bounded_without_full_text_materialization() {
        let text = "é".repeat(DEFAULT_FULL_CACHE_BYTE_BUDGET_BYTES / 2 + 1024);
        let buf = TextBuffer::try_with_version(text, BufferVersion(0)).unwrap();
        assert_eq!(buf.line_count(), 1);
        let slice = buf.line_slice(0).unwrap();
        assert!(slice.truncated);
        assert!(slice.utf8_byte_len <= DEFAULT_LINE_SLICE_MAX_BYTES + 4);
        assert!(matches!(
            buf.try_full_text(),
            Err(TextError::FullCacheBudgetExceeded { .. })
        ));
    }

    #[test]
    fn crlf_pair_is_not_split_when_near_chunk_boundary() {
        let first = "a".repeat(DEFAULT_CHUNK_TARGET_BYTES - 1);
        let second = "b".repeat(DEFAULT_CHUNK_TARGET_BYTES);
        let buf = TextBuffer::new(format!("{first}\r\n{second}\nend"));
        let cr_offset = first.len();
        assert!(
            buf.chunk_descriptors()
                .iter()
                .all(|chunk| chunk.end_byte != cr_offset + 1)
        );
    }

    #[test]
    fn utf8_and_utf16_conversion_work_across_chunk_boundaries() {
        let crab_count = DEFAULT_CHUNK_TARGET_BYTES / 4 + 32;
        let text = format!("{}\nend", "🦀".repeat(crab_count));
        let buf = TextBuffer::new(text);

        let crab_index = DEFAULT_CHUNK_TARGET_BYTES / 4 + 1;
        let byte_offset = crab_index * 4;
        let utf16 = buf.utf16_position(byte_offset).unwrap();
        assert_eq!(utf16, Utf16Position::new(0, crab_index * 2));
        assert_eq!(
            buf.byte_offset_from_utf16(Utf16Position::new(0, crab_index * 2))
                .unwrap(),
            byte_offset
        );
        assert_eq!(
            buf.position(byte_offset).unwrap(),
            TextPosition::new(0, byte_offset)
        );
    }

    #[test]
    fn chunk_hashes_change_only_for_edited_chunk_when_boundaries_stay_stable() {
        let line_a = "a".repeat(60_000);
        let line_b = "b".repeat(60_000);
        let line_c = "c".repeat(60_000);
        let mut buf = TextBuffer::new(format!("{line_a}\n{line_b}\n{line_c}\n"));

        let before = buf.chunk_descriptors().to_vec();
        assert!(before.len() >= 3);

        let edit_start = before[1].start_byte + 128;
        buf.try_replace_range(edit_start, edit_start + 5, "BBBBB")
            .unwrap();

        let after = buf.chunk_descriptors();
        assert_eq!(before.len(), after.len());
        assert_eq!(before[0].hash, after[0].hash);
        assert_ne!(before[1].hash, after[1].hash);
        assert_eq!(before[2].hash, after[2].hash);
    }

    #[test]
    fn explicit_full_text_access_fails_for_uncached_snapshot_and_buffer() {
        let text = "z".repeat(DEFAULT_FULL_CACHE_BYTE_BUDGET_BYTES + 32);
        let buf = TextBuffer::new(text);
        let snap = buf.snapshot();

        assert!(matches!(
            buf.try_full_text(),
            Err(TextError::FullCacheBudgetExceeded { .. })
        ));
        assert!(matches!(
            snap.try_full_text(),
            Err(TextError::FullCacheBudgetExceeded { .. })
        ));
    }

    #[test]
    fn edits_can_shrink_below_and_grow_above_full_cache_budget() {
        let mut buf = TextBuffer::new("x".repeat(DEFAULT_FULL_CACHE_BYTE_BUDGET_BYTES + 10));
        assert!(buf.try_full_text().is_err());

        buf.try_delete_range(
            DEFAULT_FULL_CACHE_BYTE_BUDGET_BYTES,
            DEFAULT_FULL_CACHE_BYTE_BUDGET_BYTES + 10,
        )
        .unwrap();
        assert_eq!(
            buf.try_full_text().unwrap().len(),
            DEFAULT_FULL_CACHE_BYTE_BUDGET_BYTES
        );

        buf.try_insert(buf.len(), "overflow!").unwrap();
        assert!(buf.try_full_text().is_err());
    }

    #[test]
    fn visible_line_slices_return_exact_requested_range() {
        let buf = TextBuffer::new("zero\none\ntwo\nthree\nfour");
        let slices = buf.visible_line_slices(1, 4).unwrap();
        assert_eq!(slices.len(), 3);
        assert_eq!(slices[0].line, 1);
        assert_eq!(slices[0].text, "one");
        assert_eq!(slices[2].line, 3);
        assert_eq!(slices[2].text, "three");
    }

    #[test]
    fn visible_line_slices_reject_out_of_bounds_range_before_allocating() {
        let buf = TextBuffer::new("a\nb\nc");
        assert_eq!(buf.line_count(), 3);
        // A huge `end_line_exclusive` must be rejected up front instead of forcing a massive
        // `Vec::with_capacity` ahead of the per-line bounds check.
        let err = buf.visible_line_slices(0, usize::MAX).unwrap_err();
        assert!(matches!(
            err,
            TextError::LineOutOfBounds {
                line,
                line_count: 3
            } if line == usize::MAX
        ));
    }

    #[test]
    fn simple_edit_growing_chunk_past_bound_falls_back_to_bounded_rebuild() {
        let mut buf = TextBuffer::new("abc");
        // A no-newline (simple-edit fast-path) insertion large enough to push the edited chunk past
        // the advertised force-max bound must fall back to a full chunk rebuild rather than leaving
        // an oversized chunk on the fast path.
        let big = "x".repeat(DEFAULT_CHUNK_FORCE_MAX_BYTES * 2);
        buf.try_replace_range(1, 1, &big).unwrap();
        let chunks = buf.chunk_descriptors();
        assert!(chunks.len() > 1);
        for chunk in chunks {
            assert!(chunk.byte_len <= DEFAULT_CHUNK_FORCE_MAX_BYTES);
        }
    }

    #[test]
    fn snapshot_text_does_not_panic_for_degraded_cache() {
        let text = "z".repeat(DEFAULT_FULL_CACHE_BYTE_BUDGET_BYTES + 1024);
        let snapshot = TextSnapshot::try_new(text.clone()).unwrap();
        assert!(matches!(
            snapshot.try_full_text(),
            Err(TextError::FullCacheBudgetExceeded { .. })
        ));
        // `text()` must materialize the full content instead of panicking.
        assert_eq!(snapshot.text(), text);
        assert_eq!(snapshot.text().len(), snapshot.len());
    }

    #[test]
    fn buffer_text_does_not_panic_for_degraded_cache() {
        let text = "z".repeat(DEFAULT_FULL_CACHE_BYTE_BUDGET_BYTES + 1024);
        let buf = TextBuffer::new(text.clone());
        // The bounded compatibility cache is dropped for over-budget buffers.
        assert!(matches!(
            buf.try_full_text(),
            Err(TextError::FullCacheBudgetExceeded { .. })
        ));
        // `text()` must materialize the full content instead of panicking.
        assert_eq!(buf.text(), text);
        assert_eq!(buf.text().len(), buf.len());
    }

    #[test]
    fn buffer_text_materialized_fallback_invalidated_after_mutation() {
        let mut buf = TextBuffer::new("a".repeat(DEFAULT_FULL_CACHE_BYTE_BUDGET_BYTES + 8));
        assert!(buf.try_full_text().is_err());
        // Materialize the degraded-path fallback once.
        assert_eq!(buf.text().len(), DEFAULT_FULL_CACHE_BYTE_BUDGET_BYTES + 8);

        // A mutation that keeps the buffer over budget must invalidate the cached fallback so the
        // next `text()` reflects the edit rather than returning stale content.
        buf.try_insert(0, "PREFIX").unwrap();
        assert!(buf.try_full_text().is_err());
        assert!(buf.text().starts_with("PREFIXa"));
        assert_eq!(buf.text().len(), DEFAULT_FULL_CACHE_BYTE_BUDGET_BYTES + 14);

        // Shrinking back under budget must prefer the freshly restored bounded cache.
        buf.try_delete_range(6, buf.len()).unwrap();
        assert_eq!(buf.text(), "PREFIX");
        assert_eq!(buf.try_full_text().unwrap(), "PREFIX");
    }

    #[test]
    fn large_file_typical_keystroke_edit_smoke() {
        let text = "a".repeat(1024 * 1024);
        let mut buf = TextBuffer::new(text);
        let before = buf.memory_footprint_bytes();
        buf.insert(512 * 1024, "x").unwrap();
        assert_eq!(buf.len(), 1024 * 1024 + 1);
        assert!(buf.memory_footprint_bytes() >= before);
    }

    #[test]
    fn content_hash_has_expected_prefix() {
        assert!(content_hash("hello").starts_with("sha256:"));
    }
}
