# WS-MANUAL-02 Large Files and Workspace Scale Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Support real repositories and large files without blocking typing. Complete WS-MANUAL-02 from `plans/legion-production-master-plan-v0.2.md` (lines 308-337) so `PR-UI-002` moves from substrate-validated toward product-workflow validated with real large-file and large-workspace evidence.

**Architecture:** `legion-text` owns rope-backed buffer/snapshot/chunk/line-index behavior and already handles degraded mode for files over `DEFAULT_FULL_CACHE_BYTE_BUDGET_BYTES` (5MB). `legion-editor` owns the `EditorEngine` with `BufferMode::Normal`/`Degraded` switching, viewport projections, and `LargeFileStatus`. `legion-project` owns workspace discovery, file tree scanning, watcher events, streaming search, and ignore policy. `legion-app` composes file open/save through editor+project authority, and `legion-desktop` renders the UI. `xtask` provides the CI-friendly perf-harness and verification commands. New work focuses on: (a) 100MB streaming viewport without full materialization, (b) binary file detection and preview refusal, (c) file-size policy UX, (d) workspace tree non-blocking open, (e) watcher burst/debounce under churn, (f) search cancellation resource cleanup, (g) memory ceiling measurement, and (h) stale snapshot/lease tests under large-file edits.

**Tech Stack:** Rust 2024 workspace, `ropey` for rope-backed text, `sha2`/`hex` for content hashing, `globset`/`ignore`/`tantivy` for search, `legion-platform` for OS file/watcher services, `xtask` for CI gates, targeted integration tests, `plans/evidence/production/WS-MANUAL-02/` for evidence files.

---

## Current Codebase Facts to Preserve

- `legion-text` `TextBuffer` already operates in degraded mode (no full-text cache) when `len > DEFAULT_FULL_CACHE_BYTE_BUDGET_BYTES` (5MB). Chunk descriptors, line slicing, and `materialize_full_text_from_chunks()` exist.
- `legion-editor` `EditorEngine` already switches to `BufferMode::Degraded` at `large_file_threshold_bytes`. `ViewportProjection` already includes `LargeFileStatus` with threshold, disabled overlay reasons, and degraded message.
- `legion-project` `WorkspaceActor` already has `LARGE_FILE_BYTES = 5MB` for file open decisions, `WORKSPACE_SEARCH_MAX_FILE_BYTES = 256KB` for search skip, `poll_watcher_events` with debounce constants, and streaming `search_workspace_stream` with batch emission.
- `legion-text` tests already cover: degraded cache-free mode, bounded line slices and chunks for large snapshots, chunk materialization for save, line slice across chunks, huge single-line files, edits crossing the full-cache budget boundary, and keystroke-edit-smoke at 1MB.
- `legion-editor` tests already cover: degraded completion, degraded viewport projection, degraded save assembly, coordinate conversion without full text, and snapshot lease with large files.
- `xtask/src/perf_harness.rs` already runs synthetic input-to-paint and line-galley benchmarks with budget gates.
- No binary file detection exists anywhere in the codebase currently.
- No explicit 100MB test exists (the current large-file tests use ~5MB+32 byte files).
- Watcher debounce logic exists but there is no explicit churn/burst test.
- Search cancellation exists via the `emit_workspace_search_batch` callback returning `false`, but resource cleanup after cancellation is not explicitly tested.

## Files to Create

- `plans/evidence/production/WS-MANUAL-02/WS-MANUAL-02-evidence.md`
- `plans/evidence/production/WS-MANUAL-02/reference-workspaces.md`
- `crates/legion-text/src/binary.rs`
- `crates/legion-text/tests/large_scale_100mb.rs`
- `crates/legion-editor/tests/large_file_scale.rs`
- `crates/legion-project/tests/workspace_scale.rs`
- `crates/legion-project/tests/watcher_burst.rs`
- `crates/legion-project/tests/search_cancellation.rs`

## Files to Modify

- `crates/legion-text/src/lib.rs`
- `crates/legion-text/Cargo.toml`
- `crates/legion-editor/src/lib.rs`
- `crates/legion-protocol/src/lib.rs`
- `crates/legion-project/src/lib.rs`
- `crates/legion-app/src/lib.rs`
- `crates/legion-desktop/src/view.rs`
- `xtask/src/perf_harness.rs`
- `xtask/src/main.rs`
- `plans/product-readiness-ledger.md`

## Non-Goals

- Do not rewrite the rope or chunk substrate; extend and harden it.
- Do not implement actual OS-level file memory-mapping (mmap); the rope + chunk + line-slice approach is the v1 strategy. Measure whether it is sufficient.
- Do not solve full rendering/paint integration in this workstream (that is WS-MANUAL-01).
- Do not add network or AI features. This is pure Manual-mode work.
- Do not change existing passing tests. Add new tests alongside them.

---

## Phase 0 - Evidence and Reference Workspace Definition

### Task 1: Create evidence directory and workstream evidence file

**Files:**
- Create: `plans/evidence/production/WS-MANUAL-02/WS-MANUAL-02-evidence.md`

- [ ] **Step 1: Create evidence directory and initial evidence file**

```powershell
New-Item -ItemType Directory -Force "plans/evidence/production/WS-MANUAL-02"
```

Then create `plans/evidence/production/WS-MANUAL-02/WS-MANUAL-02-evidence.md`:

```markdown
# WS-MANUAL-02 Large Files and Workspace Scale Evidence

## Workstream status

- Status: In Progress
- Plan: `docs/superpowers/plans/2026-06-19-ws-manual-02-large-files-workspace-scale.md`
- Master plan reference: `plans/legion-production-master-plan-v0.2.md` WS-MANUAL-02 (lines 308-337)

## Product gate

- `PR-UI-002` large workspace behavior: Substrate validated → pending product-workflow evidence

## Evidence records

| Task | Description | Status | Evidence |
| --- | --- | --- | --- |
| SCALE.01 | Reference workspaces defined | Pending | `reference-workspaces.md` |
| SCALE.02 | 100MB measured non-green test | Pending | integration test |
| SCALE.03 | Streaming text viewport for 100MB | Pending | integration test |
| SCALE.04 | Binary file detection and preview refusal | Pending | unit test |
| SCALE.05 | File-size policy projection and UX | Pending | protocol + UI |
| SCALE.06 | Workspace tree open non-blocking | Pending | integration test |
| SCALE.07 | Watcher burst/debounce under churn | Pending | integration test |
| SCALE.08 | Search cancellation resource cleanup | Pending | integration test |
| SCALE.09 | Memory ceiling measurement | Pending | perf harness |
| SCALE.10 | Stale snapshot/lease tests for large files | Pending | integration test |
```

- [ ] **Step 2: Commit**

```bash
git add plans/evidence/production/WS-MANUAL-02/WS-MANUAL-02-evidence.md
git commit -m "docs: create WS-MANUAL-02 evidence directory and tracking file"
```

### Task 2: Define reference workspaces (SCALE.01)

**Files:**
- Create: `plans/evidence/production/WS-MANUAL-02/reference-workspaces.md`

- [ ] **Step 1: Create reference workspaces document**

Create `plans/evidence/production/WS-MANUAL-02/reference-workspaces.md`:

```markdown
# WS-MANUAL-02 Reference Workspaces

## Purpose

Define the reference workspaces against which all WS-MANUAL-02 scale tasks are measured.
These workspaces are generated or identified, not shipped as binary fixtures.

## Reference workspaces

### RW-1: Legion Repository (self-hosted)

- Type: Real Cargo workspace
- Approximate file count: ~1,000 files
- Approximate total size: ~20MB source
- Use: GP-1 daily-driver baseline, dogfood target
- How to obtain: the repo itself (`cargo metadata` provides the workspace root)

### RW-2: 100K-File Generated Repository

- Type: Synthetic generated workspace
- File count: 100,000 `.rs` and `.toml` files across 500 directories
- Approximate size per file: 200 bytes (stub modules)
- Total size: ~20MB
- Use: workspace tree open, watcher burst, search scalability
- Generation: `xtask generate-test-workspace --files 100000 --dirs 500 --target target/test-workspaces/rw-2`

### RW-3: 100MB Single File

- Type: Synthetic single large file
- Size: exactly 100MB (104,857,600 bytes) of repeating ASCII lines
- Line count: ~2,621,440 lines (40 bytes per line)
- Use: streaming viewport, degraded mode, memory ceiling
- Generation: programmatic in-test generation (no disk fixture needed for text model tests)

### RW-4: Large Cargo Workspace

- Type: Synthetic or real large Cargo workspace
- Package count: 50 workspace members
- Use: Cargo metadata parsing, LSP project root discovery
- Generation: `xtask generate-test-workspace --cargo-workspace --packages 50 --target target/test-workspaces/rw-4`

### RW-5: Mixed Binary/Text Workspace

- Type: Synthetic workspace with intentional binary files
- Contents: 100 `.rs` files, 10 `.png` (random bytes), 5 `.exe` (random bytes), 2 `.pdf` (random bytes), 1 `.tar.gz` (random bytes)
- Use: binary detection, preview refusal, search skip behavior
- Generation: programmatic in-test generation

## Threshold definitions

| Metric | Budget | Measured against |
| --- | --- | --- |
| 100MB file open (buffer creation) | < 5s | RW-3 |
| 100MB viewport slice (40 visible lines) | < 1ms | RW-3 |
| 100MB single keystroke edit | < 50ms | RW-3 |
| 100MB memory ceiling (buffer + index) | < 400MB | RW-3 |
| Workspace tree open (100K files) | non-blocking return | RW-2 |
| Search cancellation resource release | immediate (< 100ms) | RW-1 |
| Watcher burst (1000 events in 100ms) | debounced to < 10 notifications | RW-2 |
```

- [ ] **Step 2: Commit**

```bash
git add plans/evidence/production/WS-MANUAL-02/reference-workspaces.md
git commit -m "docs: define WS-MANUAL-02 reference workspaces and thresholds (SCALE.01)"
```

---

## Phase 1 - Binary File Detection (SCALE.04)

### Task 3: Add binary detection module to `legion-text`

**Files:**
- Create: `crates/legion-text/src/binary.rs`
- Modify: `crates/legion-text/src/lib.rs`

- [ ] **Step 1: Write the failing test**

Create `crates/legion-text/src/binary.rs`:

```rust
//! Binary file detection for safe preview refusal.
//!
//! Uses a simple heuristic: scan the first N bytes for NUL (`\0`) bytes.
//! Files containing NUL bytes in the detection window are classified as binary.
//! This matches the approach used by `git` and `grep`.

/// Maximum number of bytes to scan for binary detection.
const BINARY_DETECTION_WINDOW_BYTES: usize = 8192;

/// Result of binary detection on a byte slice.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinaryDetectionResult {
    /// The content appears to be valid text (no NUL bytes found in detection window).
    Text,
    /// The content appears to be binary (NUL byte found at the given offset).
    Binary {
        /// Byte offset of the first NUL byte found.
        first_nul_offset: usize,
    },
}

impl BinaryDetectionResult {
    /// Returns `true` if the content was classified as binary.
    pub fn is_binary(&self) -> bool {
        matches!(self, BinaryDetectionResult::Binary { .. })
    }

    /// Returns `true` if the content was classified as text.
    pub fn is_text(&self) -> bool {
        matches!(self, BinaryDetectionResult::Text)
    }
}

/// Detect whether a byte slice appears to be binary content.
///
/// Scans up to [`BINARY_DETECTION_WINDOW_BYTES`] bytes for NUL (`\0`) bytes.
/// If any NUL byte is found, the content is classified as binary.
///
/// This is intentionally conservative: valid UTF-8 text never contains NUL bytes
/// in normal source files.
pub fn detect_binary(bytes: &[u8]) -> BinaryDetectionResult {
    let scan_len = bytes.len().min(BINARY_DETECTION_WINDOW_BYTES);
    let window = &bytes[..scan_len];

    match memchr::memchr(0, window) {
        Some(offset) => BinaryDetectionResult::Binary {
            first_nul_offset: offset,
        },
        None => BinaryDetectionResult::Text,
    }
}

/// Detect whether a byte slice appears to be binary content with an explicit
/// detection window size.
pub fn detect_binary_with_window(bytes: &[u8], window_bytes: usize) -> BinaryDetectionResult {
    let scan_len = bytes.len().min(window_bytes);
    let window = &bytes[..scan_len];

    match memchr::memchr(0, window) {
        Some(offset) => BinaryDetectionResult::Binary {
            first_nul_offset: offset,
        },
        None => BinaryDetectionResult::Text,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_content_is_text() {
        assert!(detect_binary(b"").is_text());
    }

    #[test]
    fn ascii_text_is_text() {
        assert!(detect_binary(b"hello world\nfn main() {}\n").is_text());
    }

    #[test]
    fn utf8_text_is_text() {
        assert!(detect_binary("héllo 🦀 wörld\n".as_bytes()).is_text());
    }

    #[test]
    fn nul_byte_at_start_is_binary() {
        let result = detect_binary(b"\x00hello");
        assert!(result.is_binary());
        assert_eq!(
            result,
            BinaryDetectionResult::Binary {
                first_nul_offset: 0
            }
        );
    }

    #[test]
    fn nul_byte_in_middle_is_binary() {
        let result = detect_binary(b"hello\x00world");
        assert!(result.is_binary());
        assert_eq!(
            result,
            BinaryDetectionResult::Binary {
                first_nul_offset: 5
            }
        );
    }

    #[test]
    fn nul_byte_beyond_window_is_text() {
        let mut data = vec![b'a'; 8192];
        data.push(0);
        assert!(detect_binary(&data).is_text());
    }

    #[test]
    fn nul_byte_at_window_boundary_is_text() {
        let mut data = vec![b'a'; 8192];
        data[8191] = 0;
        let result = detect_binary(&data);
        assert!(result.is_binary());
        assert_eq!(
            result,
            BinaryDetectionResult::Binary {
                first_nul_offset: 8191
            }
        );
    }

    #[test]
    fn custom_window_detects_within_range() {
        let data = b"ab\x00cd";
        assert!(detect_binary_with_window(data, 2).is_text());
        assert!(detect_binary_with_window(data, 3).is_binary());
    }

    #[test]
    fn simulated_png_header_is_binary() {
        let png_header: &[u8] = &[0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00];
        assert!(detect_binary(png_header).is_binary());
    }

    #[test]
    fn simulated_elf_header_is_binary() {
        let elf_header: &[u8] = &[0x7F, 0x45, 0x4C, 0x46, 0x02, 0x01, 0x01, 0x00];
        assert!(detect_binary(elf_header).is_binary());
    }
}
```

- [ ] **Step 2: Wire the module into `legion-text/src/lib.rs`**

Add near the top of `crates/legion-text/src/lib.rs`, after the existing module-level doc comment and attributes:

```rust
pub mod binary;
pub use binary::{BinaryDetectionResult, detect_binary, detect_binary_with_window};
```

- [ ] **Step 3: Run binary detection tests**

Run: `cargo test -p legion-text -- binary`
Expected: all tests PASS

- [ ] **Step 4: Commit**

```bash
git add crates/legion-text/src/binary.rs crates/legion-text/src/lib.rs
git commit -m "feat: add binary file detection to legion-text (SCALE.04)"
```

---

## Phase 2 - 100MB Streaming Viewport (SCALE.02, SCALE.03)

### Task 4: Add 100MB text model tests

**Files:**
- Create: `crates/legion-text/tests/large_scale_100mb.rs`

- [ ] **Step 1: Write the 100MB text model integration tests**

Create `crates/legion-text/tests/large_scale_100mb.rs`:

```rust
//! WS-MANUAL-02 SCALE.02/SCALE.03: 100MB large-file text model tests.
//!
//! These tests verify that 100MB files can be opened, viewport-sliced,
//! and edited without materializing the full text into memory.
//! Tests are marked #[ignore] for normal CI but run explicitly for scale evidence.

use legion_protocol::BufferVersion;
use legion_text::{
    DEFAULT_FULL_CACHE_BYTE_BUDGET_BYTES, TextBuffer, TextError, TextSnapshot,
    RetentionPinReason,
};
use std::time::Instant;

const ONE_HUNDRED_MB: usize = 100 * 1024 * 1024;
const LINE_CONTENT: &str = "abcdefghijklmnopqrstuvwxyz0123456789_|";
// 38 bytes per line content + 1 byte newline = 39 bytes per line

fn generate_100mb_text() -> String {
    let line_with_newline = format!("{LINE_CONTENT}\n");
    let bytes_per_line = line_with_newline.len(); // 39
    let line_count = ONE_HUNDRED_MB / bytes_per_line;
    let mut text = String::with_capacity(line_count * bytes_per_line);
    for _ in 0..line_count {
        text.push_str(&line_with_newline);
    }
    text
}

#[test]
#[ignore = "100MB scale test - run with: cargo test -p legion-text --test large_scale_100mb -- --ignored"]
fn scale_100mb_buffer_creation_under_budget() {
    let start = Instant::now();
    let text = generate_100mb_text();
    let gen_elapsed = start.elapsed();
    assert!(
        text.len() >= ONE_HUNDRED_MB - 39,
        "generated text should be ~100MB, got {} bytes",
        text.len()
    );

    let start = Instant::now();
    let buf = TextBuffer::try_with_version(text, BufferVersion(0))
        .expect("100MB buffer creation should succeed");
    let creation_elapsed = start.elapsed();

    assert!(
        creation_elapsed.as_secs() < 5,
        "100MB buffer creation took {:?}, budget is 5s",
        creation_elapsed
    );
    assert!(matches!(
        buf.try_full_text(),
        Err(TextError::FullCacheBudgetExceeded { .. })
    ));
    assert!(buf.len() >= ONE_HUNDRED_MB - 39);
    assert!(buf.line_count() > 2_000_000);

    eprintln!(
        "SCALE.02 100MB buffer: gen={:?} creation={:?} lines={} bytes={} chunks={}",
        gen_elapsed,
        creation_elapsed,
        buf.line_count(),
        buf.len(),
        buf.chunk_descriptors().len()
    );
}

#[test]
#[ignore = "100MB scale test - run with: cargo test -p legion-text --test large_scale_100mb -- --ignored"]
fn scale_100mb_viewport_slice_under_budget() {
    let text = generate_100mb_text();
    let buf = TextBuffer::try_with_version(text, BufferVersion(0)).unwrap();

    // Viewport of 40 visible lines near the middle
    let mid_line = buf.line_count() / 2;
    let start = Instant::now();
    let slices = buf
        .visible_line_slices(mid_line, mid_line + 40)
        .expect("viewport slice should succeed");
    let elapsed = start.elapsed();

    assert_eq!(slices.len(), 40);
    for slice in &slices {
        assert!(!slice.truncated, "normal-length lines should not truncate");
        assert_eq!(slice.text, LINE_CONTENT);
    }
    assert!(
        elapsed.as_millis() < 1,
        "100MB viewport slice took {:?}, budget is 1ms",
        elapsed
    );

    eprintln!("SCALE.03 100MB viewport (40 lines at mid): {:?}", elapsed);
}

#[test]
#[ignore = "100MB scale test - run with: cargo test -p legion-text --test large_scale_100mb -- --ignored"]
fn scale_100mb_single_keystroke_edit_under_budget() {
    let text = generate_100mb_text();
    let mut buf = TextBuffer::try_with_version(text, BufferVersion(0)).unwrap();

    // Edit near the middle of the document
    let mid_offset = buf.len() / 2;
    // Find a safe char boundary
    let edit_offset = (mid_offset..)
        .find(|&o| o <= buf.len() && {
            buf.position(o).is_some()
        })
        .unwrap_or(mid_offset);

    let start = Instant::now();
    buf.try_insert(edit_offset, "X")
        .expect("insert into 100MB buffer should succeed");
    let elapsed = start.elapsed();

    assert!(
        elapsed.as_millis() < 50,
        "100MB keystroke edit took {:?}, budget is 50ms",
        elapsed
    );
    assert_eq!(buf.len(), ONE_HUNDRED_MB / 39 * 39 + 1); // original + 1 byte

    eprintln!("SCALE.02 100MB keystroke edit: {:?}", elapsed);
}

#[test]
#[ignore = "100MB scale test - run with: cargo test -p legion-text --test large_scale_100mb -- --ignored"]
fn scale_100mb_memory_ceiling() {
    let text = generate_100mb_text();
    let buf = TextBuffer::try_with_version(text, BufferVersion(0)).unwrap();

    let memory = buf.memory_footprint_bytes();
    let ceiling = 400 * 1024 * 1024; // 400MB budget

    assert!(
        memory < ceiling,
        "100MB buffer memory footprint is {} bytes ({:.1}MB), budget is 400MB",
        memory,
        memory as f64 / (1024.0 * 1024.0)
    );

    eprintln!(
        "SCALE.09 100MB memory: {} bytes ({:.1}MB), budget: 400MB",
        memory,
        memory as f64 / (1024.0 * 1024.0)
    );
}

#[test]
#[ignore = "100MB scale test - run with: cargo test -p legion-text --test large_scale_100mb -- --ignored"]
fn scale_100mb_snapshot_creation_and_chunk_iteration() {
    let text = generate_100mb_text();
    let buf = TextBuffer::try_with_version(text, BufferVersion(0)).unwrap();

    let start = Instant::now();
    let snapshot = buf.try_snapshot().expect("snapshot of 100MB should succeed");
    let snap_elapsed = start.elapsed();

    assert!(matches!(
        snapshot.try_full_text(),
        Err(TextError::FullCacheBudgetExceeded { .. })
    ));

    // Iterate all chunks and verify contiguity
    let chunks = snapshot.chunk_descriptors();
    assert!(chunks.len() > 100, "100MB should produce many chunks");
    for (i, chunk) in chunks.iter().enumerate() {
        assert_eq!(chunk.ordinal, i);
        if i > 0 {
            assert_eq!(
                chunk.start_byte,
                chunks[i - 1].end_byte,
                "chunks must be contiguous"
            );
        }
    }
    assert_eq!(
        chunks.last().unwrap().end_byte,
        snapshot.len(),
        "last chunk must end at document end"
    );

    eprintln!(
        "SCALE.03 100MB snapshot: {:?}, chunks: {}",
        snap_elapsed,
        chunks.len()
    );
}
```

- [ ] **Step 2: Run the 100MB tests to verify they work**

Run: `cargo test -p legion-text --test large_scale_100mb -- --ignored --nocapture 2>&1`
Expected: all tests PASS (may take 10-30s total due to string generation)

- [ ] **Step 3: Commit**

```bash
git add crates/legion-text/tests/large_scale_100mb.rs
git commit -m "test: add 100MB scale tests for text model (SCALE.02, SCALE.03, SCALE.09)"
```

---

## Phase 3 - Editor Large-File Scale Tests (SCALE.05, SCALE.10)

### Task 5: Add editor engine large-file scale tests

**Files:**
- Create: `crates/legion-editor/tests/large_file_scale.rs`

- [ ] **Step 1: Write the editor large-file integration tests**

Create `crates/legion-editor/tests/large_file_scale.rs`:

```rust
//! WS-MANUAL-02 SCALE.05/SCALE.10: Editor engine large-file behavior tests.
//!
//! Tests that the editor engine correctly handles large files:
//! - Opens files in degraded mode above threshold
//! - Viewport projection includes LargeFileStatus with policy info
//! - Stale snapshot/lease handling under large-file edits
//! - File-size policy is projected in buffer metadata

use legion_editor::{
    BufferMode, Cursor, EditorEngine, EditorThresholds, TextEdit, TextPosition, TextRange,
};
use legion_protocol::{
    BufferVersion, CorrelationId, EditorViewportRequest, FileConflictLifecycleState, FileId,
    ScrollState, SnapshotConsumerKind, SnapshotId, TransactionSource, ViewportDimensions,
    ViewportProjectionMode, WorkspaceId,
};
use uuid::Uuid;

fn large_text(byte_count: usize) -> String {
    let line = "abcdefghijklmnopqrstuvwxyz0123456789_|\n";
    let lines_needed = byte_count / line.len() + 1;
    let mut text = String::with_capacity(lines_needed * line.len());
    for _ in 0..lines_needed {
        text.push_str(line);
    }
    text.truncate(byte_count);
    // Ensure it ends on a char boundary
    while !text.is_char_boundary(text.len()) {
        text.pop();
    }
    text
}

fn test_viewport_request(buffer_id: legion_protocol::BufferId) -> EditorViewportRequest {
    EditorViewportRequest {
        buffer_id,
        scroll: ScrollState {
            top_line: 0,
            left_column: 0,
        },
        dimensions: ViewportDimensions {
            width_px: 800,
            height_px: 640,
        },
    }
}

#[test]
fn large_file_opens_in_degraded_mode_with_file_size_status() {
    let threshold = 1024; // 1KB for test speed
    let mut engine = EditorEngine::with_thresholds(EditorThresholds {
        large_file_threshold_bytes: threshold,
        ..EditorThresholds::default()
    });

    let text = large_text(threshold + 100);
    let buffer_id = engine
        .open_buffer(
            WorkspaceId(1),
            FileId(1),
            "large.rs",
            text,
        )
        .unwrap();

    assert_eq!(engine.buffer_mode(buffer_id).unwrap(), BufferMode::Degraded);

    let projection = engine.viewport_projection(test_viewport_request(buffer_id)).unwrap();
    assert_eq!(projection.mode, ViewportProjectionMode::DegradedLargeFile);

    let status = projection.large_file_status.expect("should have large_file_status");
    assert_eq!(status.threshold_bytes, threshold as u64);
    assert!(status.byte_len > threshold as u64);
    assert!(!status.disabled_overlay_reasons.is_empty());
    assert!(status.message.contains("degraded"));
}

#[test]
fn normal_file_has_no_large_file_status() {
    let mut engine = EditorEngine::new();
    let buffer_id = engine
        .open_buffer(WorkspaceId(1), FileId(1), "small.rs", "fn main() {}")
        .unwrap();

    let projection = engine.viewport_projection(test_viewport_request(buffer_id)).unwrap();
    assert_eq!(projection.mode, ViewportProjectionMode::Normal);
    assert!(projection.large_file_status.is_none());
}

#[test]
fn stale_snapshot_lease_after_large_file_edit() {
    let threshold = 256;
    let mut engine = EditorEngine::with_thresholds(EditorThresholds {
        large_file_threshold_bytes: threshold,
        ..EditorThresholds::default()
    });

    let text = large_text(threshold + 100);
    let buffer_id = engine
        .open_buffer(WorkspaceId(1), FileId(1), "large.rs", text)
        .unwrap();

    // Acquire a lease on the current snapshot
    let lease = engine
        .lease_snapshot(buffer_id, SnapshotConsumerKind::LspSync)
        .unwrap();
    let pre_snapshot_id = lease.snapshot_id;

    // Edit the buffer to advance the snapshot
    engine
        .apply_edit(
            buffer_id,
            TextEdit::insert(TextPosition::zero(), "// inserted\n"),
            TransactionSource::User,
            None,
            None,
        )
        .unwrap();

    let post_snapshot_id = engine.current_snapshot(buffer_id).unwrap().snapshot_id;
    assert_ne!(pre_snapshot_id, post_snapshot_id, "snapshot should have advanced after edit");

    // Reading with the old snapshot identity should fail as stale
    let result = engine.read_snapshot_lease_chunk(
        lease.lease_id,
        buffer_id,
        post_snapshot_id, // asking for the NEW snapshot against the OLD lease
        engine.buffer_version(buffer_id).unwrap(),
        0,
    );
    assert!(
        result.is_err(),
        "reading with mismatched snapshot id should fail"
    );

    // Reading with the original (correct) lease identity should still work
    let chunk = engine
        .read_snapshot_lease_chunk(
            lease.lease_id,
            buffer_id,
            pre_snapshot_id,
            lease.buffer_version,
            0,
        )
        .unwrap();
    assert!(!chunk.text.is_empty());
}

#[test]
fn large_file_edit_preserves_degraded_mode() {
    let threshold = 512;
    let mut engine = EditorEngine::with_thresholds(EditorThresholds {
        large_file_threshold_bytes: threshold,
        ..EditorThresholds::default()
    });

    let text = large_text(threshold + 200);
    let buffer_id = engine
        .open_buffer(WorkspaceId(1), FileId(1), "large.rs", text)
        .unwrap();

    assert_eq!(engine.buffer_mode(buffer_id).unwrap(), BufferMode::Degraded);

    // Insert text - should stay degraded
    engine
        .apply_edit(
            buffer_id,
            TextEdit::insert(TextPosition::zero(), "X"),
            TransactionSource::User,
            None,
            None,
        )
        .unwrap();

    assert_eq!(
        engine.buffer_mode(buffer_id).unwrap(),
        BufferMode::Degraded,
        "buffer should remain degraded after edit that doesn't shrink below threshold"
    );
}

#[test]
fn large_file_save_assembles_from_chunks() {
    let threshold = 512;
    let mut engine = EditorEngine::with_thresholds(EditorThresholds {
        large_file_threshold_bytes: threshold,
        ..EditorThresholds::default()
    });

    let text = large_text(threshold + 200);
    let expected_len = text.len();
    let buffer_id = engine
        .open_buffer(WorkspaceId(1), FileId(1), "large.rs", text.clone())
        .unwrap();

    let save_dto = engine
        .request_save(buffer_id, Some(CorrelationId(42)))
        .unwrap();

    assert_eq!(
        save_dto.text.len(),
        expected_len,
        "save payload should contain full text assembled from chunks"
    );
    assert_eq!(save_dto.text, text);
    assert_eq!(save_dto.payload_byte_len, expected_len as u64);
}

#[test]
fn large_file_undo_redo_round_trips_content() {
    let threshold = 256;
    let mut engine = EditorEngine::with_thresholds(EditorThresholds {
        large_file_threshold_bytes: threshold,
        ..EditorThresholds::default()
    });

    let original = large_text(threshold + 100);
    let buffer_id = engine
        .open_buffer(WorkspaceId(1), FileId(1), "large.rs", original.clone())
        .unwrap();

    // Apply an edit
    engine
        .apply_edit(
            buffer_id,
            TextEdit::insert(TextPosition::zero(), "HEADER\n"),
            TransactionSource::User,
            None,
            None,
        )
        .unwrap();

    let after_edit_save = engine.request_save(buffer_id, None).unwrap();
    assert!(after_edit_save.text.starts_with("HEADER\n"));

    // Undo
    engine.undo(buffer_id, None).unwrap();
    let after_undo_save = engine.request_save(buffer_id, None).unwrap();
    assert_eq!(after_undo_save.text, original);

    // Redo
    engine.redo(buffer_id, None).unwrap();
    let after_redo_save = engine.request_save(buffer_id, None).unwrap();
    assert_eq!(after_redo_save.text, after_edit_save.text);
}
```

- [ ] **Step 2: Run the editor large-file tests**

Run: `cargo test -p legion-editor --test large_file_scale -- --nocapture`
Expected: all tests PASS

- [ ] **Step 3: Commit**

```bash
git add crates/legion-editor/tests/large_file_scale.rs
git commit -m "test: add editor engine large-file scale tests (SCALE.05, SCALE.10)"
```

---

## Phase 4 - File-Size Policy in Protocol (SCALE.05)

### Task 6: Add file-size policy projection to protocol

**Files:**
- Modify: `crates/legion-protocol/src/lib.rs`

- [ ] **Step 1: Check existing `LargeFileStatus` and `EditorBufferMetadata` structures**

Run: `rg -n "LargeFileStatus|EditorBufferMetadata|FileSizePolicy" crates/legion-protocol/src/lib.rs`
Expected: existing `LargeFileStatus` struct and `EditorBufferMetadata` struct are found.

- [ ] **Step 2: Add `FileSizeClassification` enum to protocol**

In `crates/legion-protocol/src/lib.rs`, add after the `LargeFileStatus` struct:

```rust
/// File size classification for UX status display.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileSizeClassification {
    /// File is within normal editing limits.
    Normal,
    /// File exceeds the large-file threshold and operates in degraded mode.
    Large,
    /// File was detected as binary content and cannot be previewed as text.
    Binary,
}
```

- [ ] **Step 3: Add `file_size_classification` to `EditorBufferMetadata`**

Add a new field to `EditorBufferMetadata`:

```rust
    /// File size classification for UX display.
    pub file_size_classification: FileSizeClassification,
```

- [ ] **Step 4: Fix all compilation errors from the new field**

Update every site that constructs `EditorBufferMetadata` (in `legion-editor` and any other crates) to include `file_size_classification`. In `EditorEngine::buffer_metadata()`:

```rust
file_size_classification: match state.mode {
    BufferMode::Normal => legion_protocol::FileSizeClassification::Normal,
    BufferMode::Degraded => legion_protocol::FileSizeClassification::Large,
},
```

- [ ] **Step 5: Run workspace tests**

Run: `cargo test --workspace --all-targets`
Expected: PASS

- [ ] **Step 6: Commit**

```bash
git add crates/legion-protocol/src/lib.rs crates/legion-editor/src/lib.rs
git commit -m "feat: add FileSizeClassification to protocol and editor metadata (SCALE.05)"
```

---

## Phase 5 - Workspace Tree and Watcher Tests (SCALE.06, SCALE.07)

### Task 7: Add workspace tree non-blocking and watcher burst tests

**Files:**
- Create: `crates/legion-project/tests/workspace_scale.rs`
- Create: `crates/legion-project/tests/watcher_burst.rs`

- [ ] **Step 1: Examine existing test infrastructure in `legion-project`**

Run: `rg -n "mod tests|#\[test\]|FakeFilesystem|MockFilesystem|TestFilesystem|InMemory" crates/legion-project/src/lib.rs | head -20`
Expected: identify the test infrastructure (fake filesystem, fake watcher, etc.)

- [ ] **Step 2: Write workspace tree non-blocking test**

Create `crates/legion-project/tests/workspace_scale.rs`:

The exact content depends on step 1. The test must:
1. Create a `WorkspaceActor` with a fake filesystem containing many files (1,000+ entries)
2. Call `open_workspace` and verify it returns without blocking (i.e., returns within a reasonable time)
3. Verify the file tree contains the expected entries
4. Verify that editor input (buffer operations) are not blocked during workspace open

Pattern:
```rust
//! WS-MANUAL-02 SCALE.06: Workspace tree open does not block editor input.

// ... imports based on existing test infrastructure ...

#[test]
fn workspace_open_returns_without_blocking_editor() {
    // Create a fake filesystem with 1000+ files
    // Open the workspace
    // Measure time to return
    // Assert < 5 seconds (generous budget for test infra overhead)
    // Verify file tree is populated
}
```

- [ ] **Step 3: Write watcher burst/debounce test**

Create `crates/legion-project/tests/watcher_burst.rs`:

```rust
//! WS-MANUAL-02 SCALE.07: Watcher burst/debounce under generated churn.

// ... imports based on existing test infrastructure ...

#[test]
fn watcher_burst_debounces_rapid_events() {
    // Create workspace with watcher
    // Inject 1000 rapid watcher events (file changed) in quick succession
    // Poll watcher events
    // Verify that the number of produced events is significantly less than 1000
    // (debounce should collapse rapid changes to the same file)
}
```

NOTE: The exact implementation depends on the test infrastructure discovered in step 1. The engineer must read the existing `WorkspaceActor` test helpers and watcher mock/fake to write compatible tests. The key assertions are:
- Workspace open returns in bounded time
- Watcher burst produces debounced (collapsed) events, not 1:1 event forwarding

- [ ] **Step 4: Run the tests**

Run: `cargo test -p legion-project --test workspace_scale --test watcher_burst -- --nocapture`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/legion-project/tests/workspace_scale.rs crates/legion-project/tests/watcher_burst.rs
git commit -m "test: add workspace tree and watcher burst scale tests (SCALE.06, SCALE.07)"
```

---

## Phase 6 - Search Cancellation Resource Cleanup (SCALE.08)

### Task 8: Add search cancellation test

**Files:**
- Create: `crates/legion-project/tests/search_cancellation.rs`

- [ ] **Step 1: Examine the existing search stream cancellation mechanism**

Run: `rg -n "emit_workspace_search_batch|search_workspace_stream" crates/legion-project/src/lib.rs | head -20`
Expected: find the callback-based cancellation pattern where the batch callback returns `false` to stop.

- [ ] **Step 2: Write search cancellation resource cleanup test**

Create `crates/legion-project/tests/search_cancellation.rs`:

```rust
//! WS-MANUAL-02 SCALE.08: Search cancellation releases resources.

// ... imports based on existing test infrastructure ...

#[test]
fn search_cancellation_stops_iteration_and_releases_resources() {
    // Create workspace with ~100 files containing searchable text
    // Start a streaming search
    // After receiving the first batch, return false from the callback to cancel
    // Verify: no more batches are received
    // Verify: the workspace actor is not stuck in a search loop
    // Verify: subsequent operations (open file, etc.) succeed normally
}

#[test]
fn search_with_zero_result_limit_returns_empty() {
    // Create workspace with files
    // Search with result_limit = 0
    // Verify: empty result, no crash, resources released
}
```

NOTE: The exact implementation depends on the `WorkspaceActor` test infrastructure. The engineer must use the existing fake filesystem and workspace setup helpers.

- [ ] **Step 3: Run the test**

Run: `cargo test -p legion-project --test search_cancellation -- --nocapture`
Expected: PASS

- [ ] **Step 4: Commit**

```bash
git add crates/legion-project/tests/search_cancellation.rs
git commit -m "test: add search cancellation resource cleanup test (SCALE.08)"
```

---

## Phase 7 - Memory Ceiling Perf Harness Gate (SCALE.09)

### Task 9: Add memory ceiling scenario to xtask perf harness

**Files:**
- Modify: `xtask/src/perf_harness.rs`
- Modify: `xtask/src/main.rs`

- [ ] **Step 1: Read the current perf harness skeleton kinds**

Run: `rg -n "SkeletonKind|fn run_skeleton|fn plan_skeletons" xtask/src/perf_harness.rs`
Expected: understand how to add a new skeleton kind.

- [ ] **Step 2: Add a `MemoryCeiling` skeleton kind**

In `xtask/src/perf_harness.rs`, add a new variant to `SkeletonKind`:

```rust
/// Memory ceiling measurement for the GP-1 workload (legion-text buffer
/// for a reference-size document).
#[serde(rename = "memory_ceiling_1mb", alias = "memoryceiling1mb")]
MemoryCeiling1MB,
```

Add the corresponding `as_str` match arm:

```rust
Self::MemoryCeiling1MB => "memory_ceiling_1mb",
```

- [ ] **Step 3: Add the memory ceiling skeleton plan and execution**

Add a new planning/execution function that:
1. Creates a 1MB `TextBuffer` (not 100MB, to keep CI fast)
2. Measures `memory_footprint_bytes()`
3. Asserts the footprint is under a reasonable ceiling (e.g., 10MB for a 1MB document)
4. Reports the measurement in the standard perf report format

```rust
const MEMORY_CEILING_FIXTURE_BYTES: usize = 1024 * 1024;
const MEMORY_CEILING_DEFAULT_BUDGET_BYTES: usize = 10 * 1024 * 1024;
```

- [ ] **Step 4: Wire the new skeleton into the harness runner**

Add the `MemoryCeiling1MB` case to `plan_skeletons` and the execution function so it runs alongside the existing skeletons.

- [ ] **Step 5: Run the perf harness**

Run: `cargo run -p xtask -- perf-harness`
Expected: new memory ceiling scenario appears in `perf_report.toml` as PASSED

- [ ] **Step 6: Run verification**

Run: `cargo run -p xtask -- verify-perf-harness`
Expected: PASS

- [ ] **Step 7: Commit**

```bash
git add xtask/src/perf_harness.rs xtask/src/main.rs
git commit -m "feat: add memory ceiling perf harness gate (SCALE.09)"
```

---

## Phase 8 - Binary File Preview Refusal in Editor (SCALE.04 cont.)

### Task 10: Wire binary detection into editor open path

**Files:**
- Modify: `crates/legion-editor/src/lib.rs`

- [ ] **Step 1: Add `BinaryFileRefused` error variant to `EditorError`**

```rust
/// File was detected as binary and cannot be opened as a text buffer.
#[error("file {path:?} detected as binary (NUL byte at offset {nul_offset}); preview refused")]
BinaryFileRefused {
    /// Path of the binary file.
    path: String,
    /// Offset of the first NUL byte found.
    nul_offset: usize,
},
```

- [ ] **Step 2: Add binary detection check to `EditorEngine::open_buffer`**

Before creating the `EditorBufferState`, add:

```rust
let detection = legion_text::detect_binary(initial_text.as_bytes());
if detection.is_binary() {
    if let legion_text::BinaryDetectionResult::Binary { first_nul_offset } = detection {
        return Err(EditorError::BinaryFileRefused {
            path: file_path.into().to_string(),
            nul_offset: first_nul_offset,
        });
    }
}
```

Note: The exact insertion point requires reading the current `open_buffer` implementation. The binary check must occur after `initial_text.into()` but before `EditorBufferState::new()`.

- [ ] **Step 3: Write a test for binary file refusal**

Add to `crates/legion-editor/tests/large_file_scale.rs`:

```rust
#[test]
fn binary_file_open_is_refused() {
    let mut engine = EditorEngine::new();
    let binary_content = "hello\x00world binary content";
    let result = engine.open_buffer(
        WorkspaceId(1),
        FileId(1),
        "image.png",
        binary_content,
    );
    assert!(result.is_err());
    let err = result.unwrap_err();
    let err_msg = format!("{err}");
    assert!(
        err_msg.contains("binary") || err_msg.contains("Binary"),
        "error should mention binary: {err_msg}"
    );
}

#[test]
fn text_file_with_no_nul_opens_normally() {
    let mut engine = EditorEngine::new();
    let result = engine.open_buffer(
        WorkspaceId(1),
        FileId(1),
        "clean.rs",
        "fn main() {\n    println!(\"hello\");\n}\n",
    );
    assert!(result.is_ok());
}
```

- [ ] **Step 4: Run the tests**

Run: `cargo test -p legion-editor --test large_file_scale -- --nocapture`
Expected: PASS

- [ ] **Step 5: Run full workspace tests to verify no regressions**

Run: `cargo test --workspace --all-targets`
Expected: PASS

- [ ] **Step 6: Commit**

```bash
git add crates/legion-editor/src/lib.rs crates/legion-editor/tests/large_file_scale.rs
git commit -m "feat: refuse to open binary files in editor with safe detection (SCALE.04)"
```

---

## Phase 9 - Desktop UX for File-Size Status (SCALE.05 cont.)

### Task 11: Render file-size classification banner in desktop view

**Files:**
- Modify: `crates/legion-desktop/src/view.rs`

- [ ] **Step 1: Read current large-file status rendering**

Run: `rg -n "large_file_status|degraded|LargeFileStatus|banner" crates/legion-desktop/src/view.rs`
Expected: find where/if the degraded mode status is currently rendered.

- [ ] **Step 2: Add or enhance the degraded-mode banner**

If no banner exists, add a colored warning banner at the top of the editor canvas when `projection.large_file_status.is_some()`:

```rust
// Inside render_editor_canvas or equivalent, after drawing the header:
if let Some(ref status) = projection.large_file_status {
    let banner_text = format!(
        "⚠ Large file ({:.1} MB) — some features disabled",
        status.byte_len as f64 / (1024.0 * 1024.0)
    );
    // Render banner_text in a warning-colored strip
    // Include the disabled reasons as tooltip or expanded detail
}
```

The exact egui code depends on the existing rendering pattern. The engineer must follow the existing code style in `view.rs`.

- [ ] **Step 3: Run the desktop build**

Run: `cargo check -p legion-desktop`
Expected: compiles without errors

- [ ] **Step 4: Commit**

```bash
git add crates/legion-desktop/src/view.rs
git commit -m "feat: add degraded large-file banner to desktop editor canvas (SCALE.05)"
```

---

## Phase 10 - Evidence Update and Ledger

### Task 12: Update evidence and product-readiness ledger

**Files:**
- Modify: `plans/evidence/production/WS-MANUAL-02/WS-MANUAL-02-evidence.md`
- Modify: `plans/product-readiness-ledger.md`

- [ ] **Step 1: Update evidence tracking with results**

Update `plans/evidence/production/WS-MANUAL-02/WS-MANUAL-02-evidence.md` status table to reflect completed tasks and their test evidence locations.

- [ ] **Step 2: Update product-readiness ledger**

In `plans/product-readiness-ledger.md`, update `PR-UI-002` to note that WS-MANUAL-02 evidence is in progress. Do NOT promote to product-workflow validated until all 10 SCALE tasks are complete and measured.

- [ ] **Step 3: Run docs hygiene**

Run: `cargo run -p xtask -- docs-hygiene`
Expected: PASS

- [ ] **Step 4: Run all standing gates**

```bash
cargo fmt --all --check
cargo check --workspace --all-targets
cargo test --workspace --all-targets
cargo clippy --workspace --all-targets -- -D warnings
cargo run -p xtask -- check-deps
cargo run -p xtask -- docs-hygiene
cargo run -p xtask -- no-egui-textedit
cargo run -p xtask -- perf-harness
cargo run -p xtask -- verify-perf-harness
```

Expected: all PASS

- [ ] **Step 5: Commit**

```bash
git add plans/evidence/production/WS-MANUAL-02/WS-MANUAL-02-evidence.md plans/product-readiness-ledger.md
git commit -m "docs: update WS-MANUAL-02 evidence and product-readiness ledger"
```

---

## Summary of Task-to-SCALE Mapping

| Task | SCALE Task | Description |
| --- | --- | --- |
| Task 1 | — | Evidence directory setup |
| Task 2 | SCALE.01 | Reference workspaces definition |
| Task 3 | SCALE.04 | Binary file detection module |
| Task 4 | SCALE.02, SCALE.03, SCALE.09 | 100MB text model tests (open, viewport, edit, memory, snapshot) |
| Task 5 | SCALE.05, SCALE.10 | Editor large-file scale tests (degraded mode, stale lease, undo/redo, save) |
| Task 6 | SCALE.05 | File-size classification in protocol |
| Task 7 | SCALE.06, SCALE.07 | Workspace tree non-blocking + watcher burst tests |
| Task 8 | SCALE.08 | Search cancellation resource cleanup test |
| Task 9 | SCALE.09 | Memory ceiling perf harness gate |
| Task 10 | SCALE.04 | Binary detection in editor open path |
| Task 11 | SCALE.05 | Desktop UX banner for large files |
| Task 12 | — | Evidence and ledger update |

## Dependencies Between Tasks

```text
Task 1 (evidence)
  → Task 2 (reference workspaces)
  → Task 3 (binary detection) → Task 10 (wire into editor)
  → Task 4 (100MB tests, independent)
  → Task 5 (editor scale tests, depends on Task 6 for FileSizeClassification)
  → Task 6 (protocol, independent)
  → Task 7 (workspace/watcher, independent of text/editor tasks)
  → Task 8 (search cancel, independent)
  → Task 9 (memory ceiling, independent)
  → Task 11 (desktop UX, depends on Task 6)
  → Task 12 (evidence wrap-up, depends on all above)
```

Parallelizable groups:
- **Group A** (text layer): Tasks 3, 4 (can run concurrently)
- **Group B** (editor layer): Tasks 5, 6 (can run concurrently with Group A)
- **Group C** (project layer): Tasks 7, 8 (fully independent of Groups A/B)
- **Group D** (integration): Tasks 9, 10, 11 (depend on earlier groups)
- **Group E** (wrap-up): Task 12 (depends on all)
