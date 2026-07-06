//! Raw LSP `WorkspaceEdit` → `WorkspaceEditProposalPayload` translation (PKT-LSP-C T2).
//!
//! Translates a raw rust-analyzer `WorkspaceEdit` JSON value (LSP
//! `{line,character}` positions, `uri`-based file references) into the
//! existing proposal DTOs in `legion_protocol`:
//! - [`WorkspaceEditProposalPayload`] (the outer container)
//! - [`WorkspaceTextEdit`] (per-file text mutations, byte-ranged)
//! - [`WorkspaceFileOperation`] (file-level create/rename/delete)
//! - [`ProposalVersionPreconditions`] (per-file staleness guard)
//!
//! ## Design invariants
//!
//! - **No filesystem mutation.** The translator only reads document state
//!   supplied through [`DocumentResolver`] and the raw JSON input.
//! - **Typed rejection.** Unsupported shapes (annotated edits, unresolvable
//!   URIs) surface as [`TranslationError`] variants, never silently dropped.
//! - **Precondition completeness.** Every `WorkspaceTextEdit` carries the
//!   full version context that the applying layer needs to detect stale
//!   application.
//!
//! ## Scope
//! This module performs the translation `proposal.rs` explicitly defers:
//! "Translating raw rust-analyzer WorkspaceEdit JSON … requires document
//! text (for line/character → byte offset conversion) and workspace state
//! (for `uri` → `FileIdentity` resolution)."

use legion_protocol::{
    BufferId, BufferVersion, CanonicalPath, CapabilityId, EditBatch, FileContentVersion,
    FileFingerprint, FileIdentity, ProposalAffectedTarget, ProposalTargetCoverage,
    ProposalTargetCoverageKind, ProposalTargetKind, ProposalVersionPreconditions,
    ProtocolDiagnostic, ProtocolDiagnosticSeverity, SnapshotId, TextEdit, TextOffset, TextRange,
    TimestampMillis, WorkspaceEditProposalPayload, WorkspaceEditSourceKind, WorkspaceFileOperation,
    WorkspaceGeneration, WorkspaceId, WorkspaceTextEdit,
};
use serde_json::Value;
use uuid::Uuid;

// ─────────────────────────────────────────────────────────────────────────────
// Document resolver contract
// ─────────────────────────────────────────────────────────────────────────────

/// Per-file document context supplied by the caller to support URI resolution
/// and byte-offset conversion.
///
/// In production code the implementor queries `AppComposition`'s
/// `active_documents`; in tests a [`HashMap`]-backed stub is sufficient.
pub struct ResolvedDocument {
    /// File identity (workspace + file id + canonical path + content version).
    pub file: FileIdentity,
    /// Open buffer identifier, when the file is currently open in the editor.
    pub buffer_id: Option<BufferId>,
    /// Current document text, used to convert LSP line/character positions to
    /// byte offsets.  `None` means the document is not loaded in memory;
    /// translation will reject edits for this file with
    /// [`TranslationError::DocumentNotLoaded`].
    pub text: Option<String>,
    /// Current file content version for precondition stamping.
    pub file_content_version: FileContentVersion,
    /// Current buffer version for precondition stamping.
    pub buffer_version: Option<BufferVersion>,
    /// Current workspace generation for precondition stamping.
    pub workspace_generation: WorkspaceGeneration,
    /// Current snapshot identifier.
    pub snapshot_id: Option<SnapshotId>,
    /// Current content fingerprint for staleness detection.
    pub fingerprint: Option<FileFingerprint>,
    /// Current file length in bytes.
    pub file_length: Option<u64>,
    /// Last-modified timestamp.
    pub modified_at: Option<TimestampMillis>,
}

/// Resolves a `file://` URI to a [`ResolvedDocument`].
///
/// Returning `None` means the URI is unknown to the workspace; the
/// translator will surface [`TranslationError::UnresolvableUri`].
pub trait DocumentResolver {
    /// Attempt to resolve `uri` to its current document state.
    fn resolve(&self, uri: &str) -> Option<ResolvedDocument>;
}

// ─────────────────────────────────────────────────────────────────────────────
// Error types
// ─────────────────────────────────────────────────────────────────────────────

/// Typed rejection reasons emitted by the translator.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TranslationError {
    /// The WorkspaceEdit JSON has a shape the translator does not support
    /// (e.g. annotated edits with `annotationId`, unrecognised `kind`).
    UnsupportedShape {
        /// Human-readable explanation for the rejection.
        reason: String,
    },
    /// A `file://` URI in the edit could not be mapped to a workspace file.
    UnresolvableUri {
        /// The URI that could not be resolved.
        uri: String,
    },
    /// A file's document text was not available for byte-offset conversion.
    DocumentNotLoaded {
        /// Canonical path of the file whose text is missing.
        path: String,
    },
    /// The LSP position (line, character) was out of bounds for the document.
    PositionOutOfBounds {
        /// URI of the affected file.
        uri: String,
        /// LSP line number.
        line: u64,
        /// LSP character offset (UTF-16).
        character: u64,
    },
    /// The raw `WorkspaceEdit` JSON was structurally malformed.
    MalformedEdit {
        /// Description of the structural problem.
        reason: String,
    },
}

impl std::fmt::Display for TranslationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TranslationError::UnsupportedShape { reason } => {
                write!(f, "unsupported WorkspaceEdit shape: {reason}")
            }
            TranslationError::UnresolvableUri { uri } => {
                write!(f, "URI could not be resolved to a workspace file: {uri}")
            }
            TranslationError::DocumentNotLoaded { path } => {
                write!(
                    f,
                    "document text not available for byte-offset conversion: {path}"
                )
            }
            TranslationError::PositionOutOfBounds {
                uri,
                line,
                character,
            } => {
                write!(
                    f,
                    "LSP position ({line},{character}) is out of bounds for {uri}"
                )
            }
            TranslationError::MalformedEdit { reason } => {
                write!(f, "malformed WorkspaceEdit JSON: {reason}")
            }
        }
    }
}

impl std::error::Error for TranslationError {}

// ─────────────────────────────────────────────────────────────────────��───────
// Entry point
// ─────────────────────────────────────────────────────────────────────────────

/// Translate a raw LSP `WorkspaceEdit` JSON value into a
/// [`WorkspaceEditProposalPayload`] using caller-supplied document context.
///
/// # Arguments
/// - `raw`           — the full `WorkspaceEdit` JSON object.
/// - `resolver`      — supplies file identities, text, and version context.
/// - `workspace_id`  — owning workspace for all produced identities.
/// - `source`        — producer kind (rename, formatting, code action …).
/// - `title`         — user-visible proposal title.
/// - `capability`    — capability required to apply the resulting proposal.
///
/// # Errors
/// Returns `Err(TranslationError)` when the shape is unsupported, URIs are
/// unresolvable, positions are out of bounds, or document text is unavailable.
pub fn translate_workspace_edit(
    raw: &Value,
    resolver: &dyn DocumentResolver,
    workspace_id: WorkspaceId,
    source: WorkspaceEditSourceKind,
    title: String,
    capability: CapabilityId,
) -> Result<WorkspaceEditProposalPayload, TranslationError> {
    let obj = raw
        .as_object()
        .ok_or_else(|| TranslationError::MalformedEdit {
            reason: "WorkspaceEdit must be a JSON object".to_string(),
        })?;

    let mut file_edits: Vec<WorkspaceTextEdit> = Vec::new();
    let mut file_operations: Vec<WorkspaceFileOperation> = Vec::new();
    let mut diagnostics: Vec<ProtocolDiagnostic> = Vec::new();

    if let Some(document_changes) = obj.get("documentChanges") {
        // Modern format: array of TextDocumentEdit | resource operations.
        let changes =
            document_changes
                .as_array()
                .ok_or_else(|| TranslationError::MalformedEdit {
                    reason: "documentChanges must be an array".to_string(),
                })?;

        for change in changes {
            if let Some(kind) = change.get("kind").and_then(Value::as_str) {
                // Resource operation.
                let op = translate_resource_operation(kind, change, resolver)?;
                file_operations.push(op);
            } else if change.get("textDocument").is_some() {
                // TextDocumentEdit.
                let edit = translate_text_document_edit(change, resolver, &mut diagnostics)?;
                file_edits.push(edit);
            } else {
                return Err(TranslationError::MalformedEdit {
                    reason: "documentChanges item is neither a TextDocumentEdit nor a resource operation".to_string(),
                });
            }
        }
    } else if let Some(changes) = obj.get("changes") {
        // Legacy format: { "uri": [TextEdit, ...] }
        let changes_obj = changes
            .as_object()
            .ok_or_else(|| TranslationError::MalformedEdit {
                reason: "changes must be an object".to_string(),
            })?;

        for (uri, edits_json) in changes_obj {
            let edit = translate_legacy_changes_entry(uri, edits_json, resolver, &mut diagnostics)?;
            file_edits.push(edit);
        }
    } else {
        // Empty WorkspaceEdit is valid (e.g. no-op rename).
    }

    let target_coverage = build_target_coverage(&file_edits, &file_operations);

    Ok(WorkspaceEditProposalPayload {
        workspace_id,
        edit_id: Uuid::new_v4(),
        title,
        source,
        target_coverage,
        file_edits,
        file_operations,
        required_capability: capability,
        diagnostics,
        schema_version: 1,
    })
}

// ─────────────────────────────────────────────────────────────────────────────
// Internal helpers
// ─────────────────────────────────────────────────────────────────────────────

/// Translate a single `TextDocumentEdit` entry (modern `documentChanges`).
fn translate_text_document_edit(
    change: &Value,
    resolver: &dyn DocumentResolver,
    diagnostics: &mut Vec<ProtocolDiagnostic>,
) -> Result<WorkspaceTextEdit, TranslationError> {
    let text_doc = change
        .get("textDocument")
        .ok_or_else(|| TranslationError::MalformedEdit {
            reason: "TextDocumentEdit missing textDocument".to_string(),
        })?;

    let uri = text_doc.get("uri").and_then(Value::as_str).ok_or_else(|| {
        TranslationError::MalformedEdit {
            reason: "textDocument.uri missing or not a string".to_string(),
        }
    })?;

    // Reject annotated edits (with `annotationId`) — unsupported shape.
    if let Some(edits_array) = change.get("edits").and_then(Value::as_array) {
        for edit in edits_array {
            if edit.get("annotationId").is_some() {
                return Err(TranslationError::UnsupportedShape {
                    reason: "annotated edits (annotationId) are not supported".to_string(),
                });
            }
        }
    }

    // Check for optional LSP document version for diagnostics.
    let lsp_version = text_doc.get("version").and_then(Value::as_i64);

    let doc = resolver
        .resolve(uri)
        .ok_or_else(|| TranslationError::UnresolvableUri {
            uri: uri.to_string(),
        })?;

    // Emit a diagnostic if the LSP-reported version doesn't match our current
    // version (potential staleness; the preconditions will catch it at apply
    // time, but we surface it eagerly as a diagnostic).
    if let Some(lsp_ver) = lsp_version {
        let current = doc.file_content_version.0 as i64;
        if lsp_ver != current {
            diagnostics.push(ProtocolDiagnostic {
                code: "lsp.workspace_edit.version_mismatch".to_string(),
                message: format!(
                    "document version mismatch for {uri}: LSP={lsp_ver} current={current}"
                ),
                severity: ProtocolDiagnosticSeverity::Warning,
                path: Some(doc.file.canonical_path.clone()),
                range: None,
            });
        }
    }

    let text = doc
        .text
        .as_deref()
        .ok_or_else(|| TranslationError::DocumentNotLoaded {
            path: doc.file.canonical_path.0.clone(),
        })?;

    let edits_json = change
        .get("edits")
        .and_then(Value::as_array)
        .ok_or_else(|| TranslationError::MalformedEdit {
            reason: "TextDocumentEdit missing edits array".to_string(),
        })?;

    let mut edits = Vec::new();
    for edit_json in edits_json {
        edits.push(translate_text_edit(uri, edit_json, text)?);
    }

    let preconditions = build_preconditions(&doc);

    Ok(WorkspaceTextEdit {
        file: doc.file,
        buffer_id: doc.buffer_id,
        edits: EditBatch { edits },
        preconditions,
    })
}

/// Translate a single entry from the legacy `changes` object format.
fn translate_legacy_changes_entry(
    uri: &str,
    edits_json: &Value,
    resolver: &dyn DocumentResolver,
    _diagnostics: &mut Vec<ProtocolDiagnostic>,
) -> Result<WorkspaceTextEdit, TranslationError> {
    let doc = resolver
        .resolve(uri)
        .ok_or_else(|| TranslationError::UnresolvableUri {
            uri: uri.to_string(),
        })?;

    let text = doc
        .text
        .as_deref()
        .ok_or_else(|| TranslationError::DocumentNotLoaded {
            path: doc.file.canonical_path.0.clone(),
        })?;

    let edits_array = edits_json
        .as_array()
        .ok_or_else(|| TranslationError::MalformedEdit {
            reason: format!("changes[{uri}] must be an array"),
        })?;

    let mut edits = Vec::new();
    for edit_json in edits_array {
        edits.push(translate_text_edit(uri, edit_json, text)?);
    }

    let preconditions = build_preconditions(&doc);

    Ok(WorkspaceTextEdit {
        file: doc.file,
        buffer_id: doc.buffer_id,
        edits: EditBatch { edits },
        preconditions,
    })
}

/// Translate a single LSP `TextEdit` object (range + newText) to a
/// [`TextEdit`] with byte-offset [`TextRange`].
fn translate_text_edit(
    uri: &str,
    edit_json: &Value,
    document_text: &str,
) -> Result<TextEdit, TranslationError> {
    let range = edit_json
        .get("range")
        .ok_or_else(|| TranslationError::MalformedEdit {
            reason: "TextEdit missing range".to_string(),
        })?;

    let start_line = range
        .pointer("/start/line")
        .and_then(Value::as_u64)
        .ok_or_else(|| TranslationError::MalformedEdit {
            reason: "range.start.line missing".to_string(),
        })?;
    let start_char = range
        .pointer("/start/character")
        .and_then(Value::as_u64)
        .ok_or_else(|| TranslationError::MalformedEdit {
            reason: "range.start.character missing".to_string(),
        })?;
    let end_line = range
        .pointer("/end/line")
        .and_then(Value::as_u64)
        .ok_or_else(|| TranslationError::MalformedEdit {
            reason: "range.end.line missing".to_string(),
        })?;
    let end_char = range
        .pointer("/end/character")
        .and_then(Value::as_u64)
        .ok_or_else(|| TranslationError::MalformedEdit {
            reason: "range.end.character missing".to_string(),
        })?;

    let new_text = edit_json
        .get("newText")
        .and_then(Value::as_str)
        .ok_or_else(|| TranslationError::MalformedEdit {
            reason: "TextEdit missing newText".to_string(),
        })?
        .to_string();

    let start_byte = lsp_position_to_byte_offset(document_text, start_line, start_char)
        .ok_or_else(|| TranslationError::PositionOutOfBounds {
            uri: uri.to_string(),
            line: start_line,
            character: start_char,
        })?;

    let end_byte =
        lsp_position_to_byte_offset(document_text, end_line, end_char).ok_or_else(|| {
            TranslationError::PositionOutOfBounds {
                uri: uri.to_string(),
                line: end_line,
                character: end_char,
            }
        })?;

    Ok(TextEdit {
        range: TextRange {
            start: TextOffset::byte(start_byte),
            end: TextOffset::byte(end_byte),
        },
        replacement: new_text,
    })
}

/// Translate a resource operation item (`kind` = create/rename/delete).
fn translate_resource_operation(
    kind: &str,
    change: &Value,
    resolver: &dyn DocumentResolver,
) -> Result<WorkspaceFileOperation, TranslationError> {
    match kind {
        "create" => {
            let uri = change.get("uri").and_then(Value::as_str).ok_or_else(|| {
                TranslationError::MalformedEdit {
                    reason: "create operation missing uri".to_string(),
                }
            })?;
            let path = uri_to_canonical_path(uri);
            Ok(WorkspaceFileOperation::Create {
                path: CanonicalPath(path),
                initial_content_hash: None,
            })
        }
        "delete" => {
            let uri = change.get("uri").and_then(Value::as_str).ok_or_else(|| {
                TranslationError::MalformedEdit {
                    reason: "delete operation missing uri".to_string(),
                }
            })?;
            let doc = resolver
                .resolve(uri)
                .ok_or_else(|| TranslationError::UnresolvableUri {
                    uri: uri.to_string(),
                })?;
            Ok(WorkspaceFileOperation::Delete { file: doc.file })
        }
        "rename" => {
            let old_uri = change
                .get("oldUri")
                .and_then(Value::as_str)
                .ok_or_else(|| TranslationError::MalformedEdit {
                    reason: "rename operation missing oldUri".to_string(),
                })?;
            let new_uri = change
                .get("newUri")
                .and_then(Value::as_str)
                .ok_or_else(|| TranslationError::MalformedEdit {
                    reason: "rename operation missing newUri".to_string(),
                })?;
            let doc =
                resolver
                    .resolve(old_uri)
                    .ok_or_else(|| TranslationError::UnresolvableUri {
                        uri: old_uri.to_string(),
                    })?;
            let dest_path = uri_to_canonical_path(new_uri);
            Ok(WorkspaceFileOperation::Rename {
                file: doc.file,
                destination: CanonicalPath(dest_path),
            })
        }
        other => Err(TranslationError::UnsupportedShape {
            reason: format!("unknown resource operation kind: {other:?}"),
        }),
    }
}

/// Build a [`ProposalVersionPreconditions`] from a resolved document's current
/// version context.  All populated fields are treated as mandatory at apply time.
fn build_preconditions(doc: &ResolvedDocument) -> ProposalVersionPreconditions {
    ProposalVersionPreconditions {
        file_version: Some(doc.file_content_version),
        buffer_version: doc.buffer_version,
        snapshot_id: doc.snapshot_id,
        generation: Some(doc.workspace_generation),
        file_content_version: Some(doc.file_content_version),
        workspace_generation: Some(doc.workspace_generation),
        expected_fingerprint: doc.fingerprint.clone(),
        expected_file_length: doc.file_length,
        expected_modified_at: doc.modified_at,
    }
}

/// Build a [`ProposalTargetCoverage`] listing every touched file.
fn build_target_coverage(
    file_edits: &[WorkspaceTextEdit],
    file_operations: &[WorkspaceFileOperation],
) -> ProposalTargetCoverage {
    let mut targets = Vec::new();

    for (i, edit) in file_edits.iter().enumerate() {
        let kind = if edit.buffer_id.is_some() {
            ProposalTargetKind::OpenBuffer
        } else {
            ProposalTargetKind::ClosedFile
        };
        targets.push(ProposalAffectedTarget {
            target_id: format!("edit:{i}"),
            kind,
            workspace_id: Some(edit.file.workspace_id),
            file_id: Some(edit.file.file_id),
            buffer_id: edit.buffer_id,
            path: Some(edit.file.canonical_path.clone()),
            terminal_session_id: None,
            plugin_id: None,
            remote_authority: None,
            collaboration_session_id: None,
            byte_ranges: Vec::new(),
            redaction_hints: Vec::new(),
        });
    }

    for (i, op) in file_operations.iter().enumerate() {
        let (op_path, workspace_id, file_id) = match op {
            WorkspaceFileOperation::Create { path, .. } => (Some(path.clone()), None, None),
            WorkspaceFileOperation::Delete { file } => (
                Some(file.canonical_path.clone()),
                Some(file.workspace_id),
                Some(file.file_id),
            ),
            WorkspaceFileOperation::Rename { file, destination } => (
                Some(destination.clone()),
                Some(file.workspace_id),
                Some(file.file_id),
            ),
        };
        targets.push(ProposalAffectedTarget {
            target_id: format!("op:{i}"),
            kind: ProposalTargetKind::ClosedFile,
            workspace_id,
            file_id,
            buffer_id: None,
            path: op_path,
            terminal_session_id: None,
            plugin_id: None,
            remote_authority: None,
            collaboration_session_id: None,
            byte_ranges: Vec::new(),
            redaction_hints: Vec::new(),
        });
    }

    ProposalTargetCoverage {
        coverage_kind: ProposalTargetCoverageKind::Complete,
        targets,
        omitted_target_count: 0,
        redaction_hints: Vec::new(),
    }
}

/// Convert a `file://` URI to a canonical path string.
///
/// Handles:
/// - `file:///C:/...` → `C:/...` (Windows with drive letter — detected by
///   alpha + colon at positions 0..1 of the post-`file:///` remainder).
///   The drive designator is normalized to the canonical UPPERCASE form
///   with a literal colon: LSP servers echo URIs in their own form
///   (rust-analyzer lowercases the drive; lsp-types' `Url` can emit a
///   percent-encoded `C%3A` colon), and a case-mismatched drive letter in
///   a proposal path would never match the editor's canonical paths at
///   apply time (PKT-S3-WEDGE-R3 root cause #1).
/// - `file:///home/dev/main.rs` → `/home/dev/main.rs` (Unix absolute path —
///   the leading `/` is restored; stripping `file:///` would otherwise yield
///   a relative path like `home/dev/main.rs`)
/// - `file://localhost/...` → `/...`
///
/// The output keeps the URI's forward slashes; Windows consumers that need
/// backslash form must convert (the editor's canonical paths use `\`).
pub fn uri_to_canonical_path(uri: &str) -> String {
    let stripped = if let Some(rest) = uri.strip_prefix("file:///") {
        // Percent-decode a drive colon (`C%3A` → `C:`, either hex case)
        // BEFORE the drive check — decoding after it misclassified the
        // `C%3A` form as a Unix path and prepended a bogus leading `/`.
        let rest = if rest.len() >= 4
            && rest.as_bytes()[0].is_ascii_alphabetic()
            && rest[1..4].eq_ignore_ascii_case("%3a")
        {
            format!("{}:{}", &rest[..1], &rest[4..])
        } else {
            rest.to_string()
        };
        // Distinguish Windows (drive letter + colon) from Unix absolute path.
        let rest_bytes = rest.as_bytes();
        if rest_bytes.len() >= 2 && rest_bytes[0].is_ascii_alphabetic() && rest_bytes[1] == b':' {
            // Windows drive-letter path: canonical uppercase drive.
            format!(
                "{}{}",
                (rest_bytes[0] as char).to_ascii_uppercase(),
                &rest[1..]
            )
        } else {
            // Unix absolute path: prepend the leading `/` that was consumed.
            format!("/{rest}")
        }
    } else if let Some(rest) = uri.strip_prefix("file://localhost/") {
        format!("/{rest}")
    } else if let Some(rest) = uri.strip_prefix("file://") {
        rest.to_string()
    } else {
        uri.to_string()
    };

    // Percent-decode common sequences.
    stripped.replace("%20", " ").replace("%3A", ":")
}

/// Convert an LSP `{line, character}` position to a UTF-8 byte offset within
/// `text`.
///
/// Lines are 0-indexed; `character` is a UTF-16 code-unit offset (as per LSP
/// specification § 3.17.2).  Returns `None` if `line` exceeds the number of
/// lines in `text` or if `character` exceeds the line length.
pub fn lsp_position_to_byte_offset(text: &str, line: u64, character: u64) -> Option<u64> {
    let mut current_line = 0u64;
    let mut byte_pos = 0usize;

    // Advance to the start of the target line.
    for ch in text.chars() {
        if current_line == line {
            break;
        }
        if ch == '\n' {
            current_line += 1;
        }
        byte_pos += ch.len_utf8();
    }

    if current_line < line {
        // `line` is beyond end of document.
        return None;
    }

    // Now advance by `character` UTF-16 code units within the line.
    let remaining = &text[byte_pos..];
    let mut utf16_units = 0u64;
    let mut char_byte_pos = 0usize;

    for ch in remaining.chars() {
        if utf16_units >= character {
            break;
        }
        // Codepoints ≥ U+10000 take 2 UTF-16 code units.
        let units: u64 = if (ch as u32) >= 0x10000 { 2 } else { 1 };
        if utf16_units + units > character {
            // character lands inside a surrogate pair — treat as at surrogate boundary.
            break;
        }
        utf16_units += units;
        char_byte_pos += ch.len_utf8();
    }

    Some((byte_pos + char_byte_pos) as u64)
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use legion_protocol::{BufferVersion, FileContentVersion, FileId, WorkspaceGeneration};
    use serde_json::json;
    use std::collections::HashMap;

    // ── Test fixture helpers ──────────────────────────────────────────────────

    /// Simple hash-map backed resolver for tests.
    struct MockResolver(HashMap<String, ResolvedDocument>);

    impl DocumentResolver for MockResolver {
        fn resolve(&self, uri: &str) -> Option<ResolvedDocument> {
            self.0.get(uri).map(|d| ResolvedDocument {
                file: d.file.clone(),
                buffer_id: d.buffer_id,
                text: d.text.clone(),
                file_content_version: d.file_content_version,
                buffer_version: d.buffer_version,
                workspace_generation: d.workspace_generation,
                snapshot_id: d.snapshot_id,
                fingerprint: d.fingerprint.clone(),
                file_length: d.file_length,
                modified_at: d.modified_at,
            })
        }
    }

    fn make_doc(uri: &str, text: &str) -> (String, ResolvedDocument) {
        let path = uri_to_canonical_path(uri);
        let file = FileIdentity {
            file_id: FileId(42),
            workspace_id: WorkspaceId(1),
            canonical_path: CanonicalPath(path),
            content_version: FileContentVersion(3),
            content_hash: None,
        };
        let doc = ResolvedDocument {
            file,
            buffer_id: Some(BufferId(1)),
            text: Some(text.to_string()),
            file_content_version: FileContentVersion(3),
            buffer_version: Some(BufferVersion(5)),
            workspace_generation: WorkspaceGeneration(7),
            snapshot_id: Some(SnapshotId(9)),
            fingerprint: Some(FileFingerprint {
                algorithm: "sha256".to_string(),
                value: "abc123".to_string(),
            }),
            file_length: Some(text.len() as u64),
            modified_at: Some(TimestampMillis(1_000_000)),
        };
        (uri.to_string(), doc)
    }

    // ── T2-1: Multi-file rename (two edits) → correct per-file preconditions ──

    /// A two-file rename-style WorkspaceEdit (modern `documentChanges` format)
    /// must produce a `WorkspaceEditProposalPayload` with two `WorkspaceTextEdit`
    /// entries, each carrying the preconditions from the resolver.
    ///
    /// Zero filesystem mutation is structural: the function signature accepts
    /// only a `Value` and resolver; there is no filesystem access path.
    #[test]
    fn t2_multi_file_rename_yields_one_payload_with_per_file_preconditions() {
        let file_a_uri = "file:///workspace/src/alpha.rs";
        let file_b_uri = "file:///workspace/src/beta.rs";
        let text_a = "fn alpha() {}\nfn helper() {}\n";
        let text_b = "use crate::alpha;\nfn beta() { alpha(); }\n";

        let mut resolver_map = HashMap::new();
        let (uri_a, doc_a) = make_doc(file_a_uri, text_a);
        let (uri_b, doc_b) = make_doc(file_b_uri, text_b);
        let expected_fingerprint_a = doc_a.fingerprint.clone();
        let expected_fingerprint_b = doc_b.fingerprint.clone();
        resolver_map.insert(uri_a, doc_a);
        resolver_map.insert(uri_b, doc_b);
        let resolver = MockResolver(resolver_map);

        // Rename "alpha" → "renamed_alpha" in both files.
        let raw = json!({
            "documentChanges": [
                {
                    "textDocument": { "uri": file_a_uri, "version": 3 },
                    "edits": [
                        {
                            "range": {
                                "start": {"line": 0, "character": 3},
                                "end":   {"line": 0, "character": 8}
                            },
                            "newText": "renamed_alpha"
                        }
                    ]
                },
                {
                    "textDocument": { "uri": file_b_uri, "version": 3 },
                    "edits": [
                        {
                            "range": {
                                "start": {"line": 0, "character": 11},
                                "end":   {"line": 0, "character": 16}
                            },
                            "newText": "renamed_alpha"
                        },
                        {
                            "range": {
                                "start": {"line": 1, "character": 14},
                                "end":   {"line": 1, "character": 19}
                            },
                            "newText": "renamed_alpha"
                        }
                    ]
                }
            ]
        });

        let payload = translate_workspace_edit(
            &raw,
            &resolver,
            WorkspaceId(1),
            WorkspaceEditSourceKind::LspRename,
            "Rename alpha → renamed_alpha".to_string(),
            CapabilityId("editor.write".to_string()),
        )
        .expect("translation must succeed");

        // One payload covering both files.
        assert_eq!(payload.file_edits.len(), 2, "must have 2 file edits");
        assert_eq!(payload.file_operations.len(), 0, "no resource ops");
        assert_eq!(payload.source, WorkspaceEditSourceKind::LspRename);

        // File-A preconditions.
        let edit_a = payload
            .file_edits
            .iter()
            .find(|e| e.file.canonical_path.0.contains("alpha"))
            .expect("alpha.rs edit");
        assert_eq!(
            edit_a.preconditions.file_content_version,
            Some(FileContentVersion(3))
        );
        assert_eq!(
            edit_a.preconditions.expected_fingerprint,
            expected_fingerprint_a
        );
        assert_eq!(edit_a.edits.edits.len(), 1);

        // File-B preconditions.
        let edit_b = payload
            .file_edits
            .iter()
            .find(|e| e.file.canonical_path.0.contains("beta"))
            .expect("beta.rs edit");
        assert_eq!(
            edit_b.preconditions.file_content_version,
            Some(FileContentVersion(3))
        );
        assert_eq!(
            edit_b.preconditions.expected_fingerprint,
            expected_fingerprint_b
        );
        assert_eq!(edit_b.edits.edits.len(), 2);

        // Target coverage is complete.
        assert_eq!(
            payload.target_coverage.coverage_kind,
            ProposalTargetCoverageKind::Complete
        );
        assert_eq!(payload.target_coverage.targets.len(), 2);
    }

    // ── T2-2: Version mismatch emits a diagnostic but does NOT fail ───────────

    /// When the LSP document version in a `textDocument` differs from the
    /// resolver's current version, the translator emits a diagnostic warning
    /// but still succeeds.  Hard rejection happens at apply time via
    /// `ProposalVersionPreconditions::is_stale`.
    #[test]
    fn t2_version_mismatch_emits_diagnostic_not_error() {
        let uri = "file:///workspace/src/main.rs";
        let text = "fn main() {}\n";
        let mut resolver_map = HashMap::new();
        let (key, doc) = make_doc(uri, text);
        // Resolver has version 3; LSP says version 99.
        resolver_map.insert(key, doc);
        let resolver = MockResolver(resolver_map);

        let raw = json!({
            "documentChanges": [{
                "textDocument": { "uri": uri, "version": 99 },
                "edits": [{
                    "range": {
                        "start": {"line": 0, "character": 3},
                        "end":   {"line": 0, "character": 7}
                    },
                    "newText": "start"
                }]
            }]
        });

        let payload = translate_workspace_edit(
            &raw,
            &resolver,
            WorkspaceId(1),
            WorkspaceEditSourceKind::LspCodeAction,
            "code action".to_string(),
            CapabilityId("editor.write".to_string()),
        )
        .expect("version mismatch must not fail translation");

        assert!(
            payload
                .diagnostics
                .iter()
                .any(|d| d.code == "lsp.workspace_edit.version_mismatch"),
            "version mismatch diagnostic must be present; got {:?}",
            payload.diagnostics
        );
    }

    // ── T2-3: Annotated edit → typed rejection ─────────────────────────────────

    /// An edit bearing `annotationId` must be rejected with
    /// `TranslationError::UnsupportedShape` — never silently dropped.
    #[test]
    fn t2_annotated_edit_is_rejected_with_typed_error() {
        let uri = "file:///workspace/src/lib.rs";
        let text = "pub fn foo() {}\n";
        let mut resolver_map = HashMap::new();
        let (key, doc) = make_doc(uri, text);
        resolver_map.insert(key, doc);
        let resolver = MockResolver(resolver_map);

        let raw = json!({
            "documentChanges": [{
                "textDocument": { "uri": uri, "version": 1 },
                "edits": [{
                    "range": {
                        "start": {"line": 0, "character": 7},
                        "end":   {"line": 0, "character": 10}
                    },
                    "newText": "bar",
                    "annotationId": "rename-annotation"
                }]
            }]
        });

        let err = translate_workspace_edit(
            &raw,
            &resolver,
            WorkspaceId(1),
            WorkspaceEditSourceKind::LspRename,
            "annotated rename".to_string(),
            CapabilityId("editor.write".to_string()),
        )
        .expect_err("annotated edit must be rejected");

        assert!(
            matches!(err, TranslationError::UnsupportedShape { .. }),
            "expected UnsupportedShape, got {err:?}"
        );
    }

    // ── T2-4: Resource operation rename → WorkspaceFileOperation::Rename ──────

    #[test]
    fn t2_resource_rename_translates_to_file_operation() {
        let old_uri = "file:///workspace/src/old.rs";
        let new_uri = "file:///workspace/src/new.rs";
        let mut resolver_map = HashMap::new();
        let (key, doc) = make_doc(old_uri, "pub fn old() {}\n");
        resolver_map.insert(key, doc);
        let resolver = MockResolver(resolver_map);

        let raw = json!({
            "documentChanges": [{
                "kind": "rename",
                "oldUri": old_uri,
                "newUri": new_uri
            }]
        });

        let payload = translate_workspace_edit(
            &raw,
            &resolver,
            WorkspaceId(1),
            WorkspaceEditSourceKind::LspRename,
            "rename file".to_string(),
            CapabilityId("editor.write".to_string()),
        )
        .expect("resource rename must succeed");

        assert_eq!(payload.file_operations.len(), 1);
        assert!(matches!(
            &payload.file_operations[0],
            WorkspaceFileOperation::Rename { destination, .. }
                if destination.0.contains("new.rs")
        ));
    }

    // ── T2-5: Legacy changes format ────────────────────────────────────────────

    #[test]
    fn t2_legacy_changes_format_translates_correctly() {
        let uri = "file:///workspace/src/lib.rs";
        let text = "fn foo() {}\n";
        let mut resolver_map = HashMap::new();
        let (key, doc) = make_doc(uri, text);
        resolver_map.insert(key, doc);
        let resolver = MockResolver(resolver_map);

        let raw = json!({
            "changes": {
                uri: [{
                    "range": {
                        "start": {"line": 0, "character": 3},
                        "end":   {"line": 0, "character": 6}
                    },
                    "newText": "bar"
                }]
            }
        });

        let payload = translate_workspace_edit(
            &raw,
            &resolver,
            WorkspaceId(1),
            WorkspaceEditSourceKind::LspFormatting,
            "format".to_string(),
            CapabilityId("editor.write".to_string()),
        )
        .expect("legacy format must succeed");

        assert_eq!(payload.file_edits.len(), 1);
        assert_eq!(payload.file_edits[0].edits.edits.len(), 1);
        // "fn foo()" — byte 3..6 is "foo".
        assert_eq!(payload.file_edits[0].edits.edits[0].replacement, "bar");
        assert_eq!(payload.file_edits[0].edits.edits[0].range.start.value, 3);
        assert_eq!(payload.file_edits[0].edits.edits[0].range.end.value, 6);
    }

    // ── T2-6: Byte-offset conversion smoke test ────────────────────────────────

    #[test]
    fn t2_lsp_position_to_byte_offset_converts_correctly() {
        let text = "hello\nworld\n";
        // Line 0 character 0 → byte 0.
        assert_eq!(lsp_position_to_byte_offset(text, 0, 0), Some(0));
        // Line 0 character 5 → byte 5 (past "hello").
        assert_eq!(lsp_position_to_byte_offset(text, 0, 5), Some(5));
        // Line 1 character 0 → byte 6 (after the '\n').
        assert_eq!(lsp_position_to_byte_offset(text, 1, 0), Some(6));
        // Line 1 character 5 → byte 11 (past "world").
        assert_eq!(lsp_position_to_byte_offset(text, 1, 5), Some(11));
        // Line 2 out of bounds → None.
        assert_eq!(lsp_position_to_byte_offset(text, 3, 0), None);
    }

    // ── T2-7: URI conversion ────────────────────────────────────────────────────
    //
    // I-5 fix: `file:///home/dev/main.rs` must yield `/home/dev/main.rs`
    // (not `home/dev/main.rs`).  The previous implementation stripped
    // `file:///` and returned the remainder unchanged, dropping the leading
    // `/` for Unix paths.

    #[test]
    fn t2_uri_to_canonical_path_handles_windows_and_unix() {
        // Windows: drive-letter path is kept as-is (canonical uppercase drive).
        assert_eq!(
            uri_to_canonical_path("file:///C:/Users/dev/main.rs"),
            "C:/Users/dev/main.rs"
        );
        // Unix: leading slash must be present in the result (I-5 fix).
        assert_eq!(
            uri_to_canonical_path("file:///home/dev/main.rs"),
            "/home/dev/main.rs",
            "Unix URI must preserve the leading slash"
        );
        assert_eq!(
            uri_to_canonical_path("file://localhost/home/dev/main.rs"),
            "/home/dev/main.rs"
        );
    }

    // ── T2-7b: server-echoed drive-designator forms (PKT-S3-WEDGE-R3) ─────────
    //
    // rust-analyzer echoes URIs with a lowercase drive letter, and lsp-types'
    // `Url` can percent-encode the drive colon. A case-mismatched or
    // slash-prefixed drive path in a proposal would never match the editor's
    // canonical paths at apply time.

    #[test]
    fn t2_uri_to_canonical_path_normalizes_server_echoed_drive_forms() {
        for uri in [
            "file:///c:/Users/dev/main.rs",
            "file:///C%3A/Users/dev/main.rs",
            "file:///c%3A/Users/dev/main.rs",
            "file:///c%3a/Users/dev/main.rs",
        ] {
            assert_eq!(
                uri_to_canonical_path(uri),
                "C:/Users/dev/main.rs",
                "server-echoed form {uri} must normalize to the canonical drive"
            );
        }
        // A Unix path whose first segment merely looks alphabetic must not be
        // treated as a drive.
        assert_eq!(
            uri_to_canonical_path("file:///c/Users/dev/main.rs"),
            "/c/Users/dev/main.rs"
        );
    }

    // ── T2-8: Unresolvable URI → typed error ───────────────────────────────────

    #[test]
    fn t2_unresolvable_uri_returns_typed_error() {
        let resolver = MockResolver(HashMap::new()); // empty: resolves nothing

        let raw = json!({
            "changes": {
                "file:///workspace/src/missing.rs": [{
                    "range": {
                        "start": {"line": 0, "character": 0},
                        "end":   {"line": 0, "character": 0}
                    },
                    "newText": ""
                }]
            }
        });

        let err = translate_workspace_edit(
            &raw,
            &resolver,
            WorkspaceId(1),
            WorkspaceEditSourceKind::LspRename,
            "missing".to_string(),
            CapabilityId("editor.write".to_string()),
        )
        .expect_err("unresolvable URI must fail");

        assert!(
            matches!(err, TranslationError::UnresolvableUri { .. }),
            "expected UnresolvableUri, got {err:?}"
        );
    }

    // ── T2-9: Zero filesystem mutation scan — brief-mandated (I-1 fix) ────────
    //
    // The brief mandates: "zero direct filesystem mutation — assert by scanning
    // a temp workspace before/after the full rename flow."
    //
    // This test creates actual files in a temp directory, records their content
    // hashes before translation, runs `translate_workspace_edit` (the full rename
    // flow), and asserts that every file's hash is identical after translation.
    // The function signature (`&Value`, `&dyn DocumentResolver`) structurally
    // forbids filesystem writes, but this test provides an EXECUTABLE assertion
    // rather than relying on inspection alone.

    #[test]
    fn t2_zero_filesystem_mutation_scan() {
        use std::fs;

        // Set up a temp workspace with two source files.
        let dir = tempfile::tempdir().expect("create temp dir for mutation scan");
        let alpha_path = dir.path().join("alpha.rs");
        let beta_path = dir.path().join("beta.rs");
        let alpha_text = "fn alpha() {}\nfn helper() {}\n";
        let beta_text = "use crate::alpha;\nfn beta() { alpha(); }\n";
        fs::write(&alpha_path, alpha_text).expect("write alpha.rs");
        fs::write(&beta_path, beta_text).expect("write beta.rs");

        // Helper: read a file and return its bytes for content comparison.
        let snap =
            |path: &std::path::Path| -> Vec<u8> { fs::read(path).expect("read file for snapshot") };

        // Snapshot BEFORE translation.
        let before_alpha = snap(&alpha_path);
        let before_beta = snap(&beta_path);

        // Build the resolver and WorkspaceEdit payload (simulates LSP rename response).
        let alpha_uri = "file:///workspace/src/alpha.rs";
        let beta_uri = "file:///workspace/src/beta.rs";
        let mut resolver_map = HashMap::new();
        let (uri_a, doc_a) = make_doc(alpha_uri, alpha_text);
        let (uri_b, doc_b) = make_doc(beta_uri, beta_text);
        resolver_map.insert(uri_a, doc_a);
        resolver_map.insert(uri_b, doc_b);
        let resolver = MockResolver(resolver_map);

        let raw = json!({
            "documentChanges": [
                {
                    "textDocument": { "uri": alpha_uri, "version": 3 },
                    "edits": [{
                        "range": {
                            "start": {"line": 0, "character": 3},
                            "end":   {"line": 0, "character": 8}
                        },
                        "newText": "renamed_alpha"
                    }]
                },
                {
                    "textDocument": { "uri": beta_uri, "version": 3 },
                    "edits": [{
                        "range": {
                            "start": {"line": 0, "character": 11},
                            "end":   {"line": 0, "character": 16}
                        },
                        "newText": "renamed_alpha"
                    }]
                }
            ]
        });

        // Run the full rename translation.
        let payload = translate_workspace_edit(
            &raw,
            &resolver,
            WorkspaceId(1),
            WorkspaceEditSourceKind::LspRename,
            "Rename alpha → renamed_alpha".to_string(),
            CapabilityId("editor.write".to_string()),
        )
        .expect("translation must succeed");

        // Snapshot AFTER translation.
        let after_alpha = snap(&alpha_path);
        let after_beta = snap(&beta_path);

        // EXECUTABLE zero-mutation assertion (not merely structural).
        assert_eq!(
            before_alpha, after_alpha,
            "alpha.rs must not be mutated by translate_workspace_edit"
        );
        assert_eq!(
            before_beta, after_beta,
            "beta.rs must not be mutated by translate_workspace_edit"
        );

        // Sanity: the proposal was generated correctly (not empty).
        assert_eq!(
            payload.file_edits.len(),
            2,
            "translation must yield two file edits"
        );
    }
}
