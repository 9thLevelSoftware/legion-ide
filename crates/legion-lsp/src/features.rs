//! LSP feature projections for Legion IDE.
//!
//! This module provides the dedicated feature projection path from LSP
//! responses to Legion metadata rows. Each feature follows the same pattern:
//!
//! 1. Build a JSON-RPC request (e.g., `textDocument/completion`).
//! 2. Send it through the LSP client.
//! 3. Project the response into Legion metadata rows.
//!
//! # Supported Features
//!
//! | Feature | LSP Method | Projection |
//! |---------|-----------|------------|
//! | Completion | `textDocument/completion` | `CompletionProjection` |
//! | Hover | `textDocument/hover` | `HoverProjection` |
//! | Definition | `textDocument/definition` | `DefinitionProjection` |
//! | References | `textDocument/references` | `ReferencesProjection` |
//! | Document Symbols | `textDocument/documentSymbol` | `SymbolProjection` |
//! | Inlay Hints | `textDocument/inlayHint` | `InlayHintProjection` |
//! | Code Lenses | `textDocument/codeLens` | `CodeLensProjection` |
//!
//! # Design Constraints
//!
//! * All projections are metadata-only — no raw source retention.
//! * Write-side actions (rename, format, code actions) go through the
//!   proposal lifecycle, not directly through this module.
//! * Features are only projected when the LSP server advertises the
//!   corresponding capability.

use serde_json::{Value, json};

use legion_protocol::{
    BufferId, BufferVersion, CanonicalPath, FileId, LanguageId, SnapshotId, TextCoordinate,
    WorkspaceId,
};

/// Build a JSON-RPC `textDocument/completion` request.
pub fn completion_request(id: u64, text_document_uri: &str, position: &TextCoordinate) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": "textDocument/completion",
        "params": {
            "textDocument": { "uri": text_document_uri },
            "position": { "line": position.line, "character": position.character }
        }
    })
}

/// Build a JSON-RPC `textDocument/hover` request.
pub fn hover_request(id: u64, text_document_uri: &str, position: &TextCoordinate) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": "textDocument/hover",
        "params": {
            "textDocument": { "uri": text_document_uri },
            "position": { "line": position.line, "character": position.character }
        }
    })
}

/// Build a JSON-RPC `textDocument/definition` request.
pub fn definition_request(id: u64, text_document_uri: &str, position: &TextCoordinate) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": "textDocument/definition",
        "params": {
            "textDocument": { "uri": text_document_uri },
            "position": { "line": position.line, "character": position.character }
        }
    })
}

/// Build a JSON-RPC `textDocument/references` request.
pub fn references_request(id: u64, text_document_uri: &str, position: &TextCoordinate) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": "textDocument/references",
        "params": {
            "textDocument": { "uri": text_document_uri },
            "position": { "line": position.line, "character": position.character },
            "context": { "includeDeclaration": true }
        }
    })
}

/// Build a JSON-RPC `textDocument/documentSymbol` request.
pub fn document_symbol_request(id: u64, text_document_uri: &str) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": "textDocument/documentSymbol",
        "params": {
            "textDocument": { "uri": text_document_uri }
        }
    })
}

/// Build a JSON-RPC `textDocument/inlayHint` request.
pub fn inlay_hint_request(
    id: u64,
    text_document_uri: &str,
    start_line: u32,
    end_line: u32,
) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": "textDocument/inlayHint",
        "params": {
            "textDocument": { "uri": text_document_uri },
            "range": {
                "start": { "line": start_line, "character": 0 },
                "end": { "line": end_line, "character": 0 }
            }
        }
    })
}

/// Build a JSON-RPC `textDocument/codeLens` request.
pub fn code_lens_request(id: u64, text_document_uri: &str) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": "textDocument/codeLens",
        "params": {
            "textDocument": { "uri": text_document_uri }
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn coord(line: u32, character: u32) -> TextCoordinate {
        TextCoordinate {
            line,
            character,
            byte_offset: None,
            utf16_offset: None,
        }
    }

    #[test]
    fn completion_request_has_correct_method_and_params() {
        let request = completion_request(1, "file:///test.rs", &coord(10, 5));
        assert_eq!(request["method"], "textDocument/completion");
        assert_eq!(request["id"], 1);
        assert_eq!(request["params"]["textDocument"]["uri"], "file:///test.rs");
        assert_eq!(request["params"]["position"]["line"], 10);
        assert_eq!(request["params"]["position"]["character"], 5);
    }

    #[test]
    fn hover_request_has_correct_method() {
        let request = hover_request(2, "file:///test.rs", &coord(0, 0));
        assert_eq!(request["method"], "textDocument/hover");
        assert_eq!(request["id"], 2);
    }

    #[test]
    fn definition_request_has_correct_method() {
        let request = definition_request(3, "file:///test.rs", &coord(5, 10));
        assert_eq!(request["method"], "textDocument/definition");
        assert_eq!(request["id"], 3);
    }

    #[test]
    fn references_request_includes_declaration_context() {
        let request = references_request(4, "file:///test.rs", &coord(0, 0));
        assert_eq!(request["method"], "textDocument/references");
        assert_eq!(request["params"]["context"]["includeDeclaration"], true);
    }

    #[test]
    fn document_symbol_request_has_correct_method() {
        let request = document_symbol_request(5, "file:///test.rs");
        assert_eq!(request["method"], "textDocument/documentSymbol");
        assert_eq!(request["id"], 5);
    }

    #[test]
    fn inlay_hint_request_has_range() {
        let request = inlay_hint_request(6, "file:///test.rs", 0, 100);
        assert_eq!(request["method"], "textDocument/inlayHint");
        assert_eq!(request["params"]["range"]["start"]["line"], 0);
        assert_eq!(request["params"]["range"]["end"]["line"], 100);
    }

    #[test]
    fn code_lens_request_has_correct_method() {
        let request = code_lens_request(7, "file:///test.rs");
        assert_eq!(request["method"], "textDocument/codeLens");
        assert_eq!(request["id"], 7);
    }
}

/// Build a JSON-RPC `textDocument/rename` request.
pub fn rename_request(
    id: u64,
    text_document_uri: &str,
    position: &TextCoordinate,
    new_name: &str,
) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": "textDocument/rename",
        "params": {
            "textDocument": { "uri": text_document_uri },
            "position": { "line": position.line, "character": position.character },
            "newName": new_name
        }
    })
}

/// Build a JSON-RPC `textDocument/formatting` request.
pub fn formatting_request(
    id: u64,
    text_document_uri: &str,
    tab_size: u32,
    insert_spaces: bool,
) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": "textDocument/formatting",
        "params": {
            "textDocument": { "uri": text_document_uri },
            "options": { "tabSize": tab_size, "insertSpaces": insert_spaces }
        }
    })
}

/// Build a JSON-RPC `textDocument/rangeFormatting` request.
pub fn range_formatting_request(
    id: u64,
    text_document_uri: &str,
    start: &TextCoordinate,
    end: &TextCoordinate,
    tab_size: u32,
    insert_spaces: bool,
) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": "textDocument/rangeFormatting",
        "params": {
            "textDocument": { "uri": text_document_uri },
            "range": {
                "start": { "line": start.line, "character": start.character },
                "end": { "line": end.line, "character": end.character }
            },
            "options": { "tabSize": tab_size, "insertSpaces": insert_spaces }
        }
    })
}

/// Build a JSON-RPC `textDocument/codeAction` request.
pub fn code_action_request(
    id: u64,
    text_document_uri: &str,
    start: &TextCoordinate,
    end: &TextCoordinate,
    diagnostics: &[Value],
) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": "textDocument/codeAction",
        "params": {
            "textDocument": { "uri": text_document_uri },
            "range": {
                "start": { "line": start.line, "character": start.character },
                "end": { "line": end.line, "character": end.character }
            },
            "context": { "diagnostics": diagnostics }
        }
    })
}

/// Build a JSON-RPC `workspace/executeCommand` request for organize imports.
pub fn organize_imports_request(id: u64, text_document_uri: &str) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": "workspace/executeCommand",
        "params": {
            "command": "organizeImports",
            "arguments": [{ "uri": text_document_uri }]
        }
    })
}

/// Represents a workspace edit from an LSP response that should become a proposal.
///
/// This is the bridge between LSP write-side responses and Legion's proposal
/// lifecycle. The LSP server returns a `WorkspaceEdit` with text changes per
/// file; this struct converts those into a proposal-ready form.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LspWorkspaceEditProposal {
    /// Human-readable label for the proposal (e.g., "Rename foo to bar").
    pub label: String,
    /// Per-file text edits that make up the proposal.
    pub file_edits: Vec<LspFileEdit>,
}

/// A set of text edits for a single file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LspFileEdit {
    /// URI of the file being edited.
    pub uri: String,
    /// Text edits to apply (in document order).
    pub edits: Vec<LspTextEdit>,
}

/// A single text edit within a file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LspTextEdit {
    /// Range to replace.
    pub range: legion_protocol::ProtocolTextRange,
    /// New text to insert (empty string = deletion).
    pub new_text: String,
}

impl LspWorkspaceEditProposal {
    /// Parse an LSP `WorkspaceEdit` JSON response into a proposal.
    ///
    /// Returns `None` if the response doesn't contain a valid workspace edit.
    pub fn from_workspace_edit_json(edit: &Value, label: String) -> Option<Self> {
        let changes = edit.get("changes")?;
        let mut file_edits = Vec::new();
        for (uri, edits_json) in changes.as_object()? {
            let edits = edits_json
                .as_array()?
                .iter()
                .filter_map(|edit_json| {
                    let range = edit_json.get("range")?;
                    let new_text = edit_json.get("newText")?.as_str()?;
                    Some(LspTextEdit {
                        range: super::diagnostics::protocol_range_from_lsp_json(range)?,
                        new_text: new_text.to_string(),
                    })
                })
                .collect::<Vec<_>>();
            file_edits.push(LspFileEdit {
                uri: uri.clone(),
                edits,
            });
        }
        Some(LspWorkspaceEditProposal { label, file_edits })
    }

    /// Returns the total number of edits across all files.
    pub fn total_edit_count(&self) -> usize {
        self.file_edits.iter().map(|f| f.edits.len()).sum()
    }

    /// Returns the number of files affected.
    pub fn file_count(&self) -> usize {
        self.file_edits.len()
    }
}

#[cfg(test)]
mod write_side_tests {
    use super::*;
    use serde_json::json;

    fn coord(line: u32, character: u32) -> TextCoordinate {
        TextCoordinate {
            line,
            character,
            byte_offset: None,
            utf16_offset: None,
        }
    }

    #[test]
    fn rename_request_has_correct_method_and_params() {
        let request = rename_request(10, "file:///test.rs", &coord(5, 10), "new_name");
        assert_eq!(request["method"], "textDocument/rename");
        assert_eq!(request["id"], 10);
        assert_eq!(request["params"]["newName"], "new_name");
        assert_eq!(request["params"]["position"]["line"], 5);
    }

    #[test]
    fn formatting_request_has_options() {
        let request = formatting_request(11, "file:///test.rs", 4, true);
        assert_eq!(request["method"], "textDocument/formatting");
        assert_eq!(request["params"]["options"]["tabSize"], 4);
        assert_eq!(request["params"]["options"]["insertSpaces"], true);
    }

    #[test]
    fn code_action_request_includes_diagnostics() {
        let diags = vec![json!({"severity": 1, "message": "error"})];
        let request =
            code_action_request(12, "file:///test.rs", &coord(0, 0), &coord(0, 10), &diags);
        assert_eq!(request["method"], "textDocument/codeAction");
        assert_eq!(
            request["params"]["context"]["diagnostics"]
                .as_array()
                .unwrap()
                .len(),
            1
        );
    }

    #[test]
    fn organize_imports_request_uses_execute_command() {
        let request = organize_imports_request(13, "file:///test.rs");
        assert_eq!(request["method"], "workspace/executeCommand");
        assert_eq!(request["params"]["command"], "organizeImports");
    }

    #[test]
    fn workspace_edit_proposal_parses_changes() {
        let edit = json!({
            "changes": {
                "file:///test.rs": [
                    {
                        "range": {
                            "start": {"line": 0, "character": 0},
                            "end": {"line": 0, "character": 5}
                        },
                        "newText": "hello"
                    }
                ]
            }
        });
        let proposal =
            LspWorkspaceEditProposal::from_workspace_edit_json(&edit, "rename".to_string());
        assert!(proposal.is_some());
        let proposal = proposal.unwrap();
        assert_eq!(proposal.label, "rename");
        assert_eq!(proposal.file_count(), 1);
        assert_eq!(proposal.total_edit_count(), 1);
        assert_eq!(proposal.file_edits[0].uri, "file:///test.rs");
        assert_eq!(proposal.file_edits[0].edits[0].new_text, "hello");
    }

    #[test]
    fn workspace_edit_proposal_returns_none_for_invalid_json() {
        assert!(
            LspWorkspaceEditProposal::from_workspace_edit_json(&json!({}), "test".to_string())
                .is_none()
        );
        assert!(
            LspWorkspaceEditProposal::from_workspace_edit_json(&json!(null), "test".to_string())
                .is_none()
        );
    }
}
