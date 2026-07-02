//! LSP diagnostics projection for Legion IDE.
//!
//! This module provides the dedicated diagnostics projection path from
//! LSP `textDocument/publishDiagnostics` notifications through to Legion's
//! metadata-only problem rows. It exists so that:
//!
//! * Diagnostics are projected as metadata-only rows (no raw source retention).
//! * The desktop harness can ingest diagnostics through the product path.
//! * Privacy scope and redaction are applied at projection time.
//!
//! # Flow
//!
//! 1. LSP server sends `publishDiagnostics` notification.
//! 2. `project_publish_diagnostics` converts the JSON-RPC params into
//!    `LspProjectedDiagnostics` (problem rows + summary).
//! 3. `AppComposition::ingest_lsp_publish_diagnostics_for_buffer` merges
//!    the projected rows into the language tooling state.
//! 4. `DesktopRuntime::ingest_lsp_publish_diagnostics_for_buffer` refreshes
//!    the projection snapshot so the desktop harness sees the updated rows.

use legion_protocol::{
    BufferId, BufferVersion, FileFingerprint, FileId, LanguageProblemProjection,
    LspDiagnosticSummary, ProtocolDiagnosticSeverity, SemanticPrivacyScope, SnapshotId,
    WorkspaceId,
};
use serde_json::Value;

use crate::{
    LspDiagnosticProjectionContext, LspProjectedDiagnostics, LspRuntimeError, LspRuntimeResult,
};

/// Metadata-only fingerprint for a diagnostic message.
///
/// Used for deduplication and freshness checks without retaining raw source.
pub fn metadata_fingerprint(scope: &str, input: &str) -> FileFingerprint {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut hasher = DefaultHasher::new();
    scope.hash(&mut hasher);
    input.hash(&mut hasher);
    FileFingerprint {
        algorithm: "default-hash".to_string(),
        value: hasher.finish().to_string(),
    }
}

/// Severity mapping from LSP severity integer to Legion protocol severity.
pub fn severity_from_lsp_value(value: Option<&Value>) -> ProtocolDiagnosticSeverity {
    match value.and_then(Value::as_u64) {
        Some(1) => ProtocolDiagnosticSeverity::Error,
        Some(2) => ProtocolDiagnosticSeverity::Warning,
        Some(3) => ProtocolDiagnosticSeverity::Info,
        Some(4) => ProtocolDiagnosticSeverity::Hint,
        _ => ProtocolDiagnosticSeverity::Warning,
    }
}

/// Extract a protocol range from an LSP range JSON object.
pub fn protocol_range_from_lsp_json(range: &Value) -> Option<legion_protocol::ProtocolTextRange> {
    let start = range.get("start")?;
    let end = range.get("end")?;
    Some(legion_protocol::ProtocolTextRange {
        start: legion_protocol::TextCoordinate {
            line: start.get("line")?.as_u64()? as u32,
            character: start.get("character")?.as_u64()? as u32,
            byte_offset: None,
            utf16_offset: None,
        },
        end: legion_protocol::TextCoordinate {
            line: end.get("line")?.as_u64()? as u32,
            character: end.get("character")?.as_u64()? as u32,
            byte_offset: None,
            utf16_offset: None,
        },
    })
}

/// Extract a human-readable code label from an LSP diagnostic.
pub fn diagnostic_code_label(diagnostic: &Value) -> Option<String> {
    match diagnostic.get("code")? {
        Value::Number(n) => Some(n.to_string()),
        Value::String(s) => Some(s.clone()),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn severity_from_lsp_value_maps_all_known_severities() {
        assert_eq!(
            severity_from_lsp_value(Some(&json!(1))),
            ProtocolDiagnosticSeverity::Error
        );
        assert_eq!(
            severity_from_lsp_value(Some(&json!(2))),
            ProtocolDiagnosticSeverity::Warning
        );
        assert_eq!(
            severity_from_lsp_value(Some(&json!(3))),
            ProtocolDiagnosticSeverity::Info
        );
        assert_eq!(
            severity_from_lsp_value(Some(&json!(4))),
            ProtocolDiagnosticSeverity::Hint
        );
        // Unknown severity defaults to Warning.
        assert_eq!(
            severity_from_lsp_value(Some(&json!(99))),
            ProtocolDiagnosticSeverity::Warning
        );
        assert_eq!(
            severity_from_lsp_value(None),
            ProtocolDiagnosticSeverity::Warning
        );
    }

    #[test]
    fn protocol_range_from_lsp_json_extracts_line_and_character() {
        let range = json!({
            "start": {"line": 0, "character": 3},
            "end": {"line": 0, "character": 7}
        });
        let result = protocol_range_from_lsp_json(&range).expect("range should parse");
        assert_eq!(result.start.line, 0);
        assert_eq!(result.start.character, 3);
        assert_eq!(result.end.line, 0);
        assert_eq!(result.end.character, 7);
    }

    #[test]
    fn protocol_range_from_lsp_json_returns_none_for_missing_fields() {
        assert!(protocol_range_from_lsp_json(&json!({})).is_none());
        assert!(protocol_range_from_lsp_json(&json!({"start": {}})).is_none());
    }

    #[test]
    fn diagnostic_code_label_handles_number_and_string_codes() {
        assert_eq!(
            diagnostic_code_label(&json!({"code": 2580})),
            Some("2580".to_string())
        );
        assert_eq!(
            diagnostic_code_label(&json!({"code": "E0425"})),
            Some("E0425".to_string())
        );
        assert_eq!(diagnostic_code_label(&json!({})), None);
    }

    #[test]
    fn metadata_fingerprint_is_deterministic() {
        let a = metadata_fingerprint("lsp.diagnostic", "expected diagnostic text");
        let b = metadata_fingerprint("lsp.diagnostic", "expected diagnostic text");
        assert_eq!(a, b);

        let c = metadata_fingerprint("lsp.diagnostic", "different text");
        assert_ne!(a, c);
    }
}
