# Phase 3 Context — Live LSP Integration

## Goal
Wire the existing LSP client to the editor so diagnostics, completions, hover, and go-to-definition work visually. The LSP client, protocol types, worker threading, and app-layer composition are already built. The gap is purely in desktop rendering.

## Success Criteria
- Open a Rust project, see red squiggles on errors, get completions, hover shows types
- Diagnostics appear as inline underlines with severity-colored indicators
- Completion popup shows on typing / Ctrl+Space, insert on accept
- Hover tooltip shows type information on mouse hover
- Go-to-definition works on Ctrl+Click / F12

## Current State — What's Already Built

### legion-lsp (4,601 source lines)
- **Full LSP client**: `LspClient` with JSON-RPC correlation, `prepare_request`, `correlate_response`
- **Document sync**: `did_open_notification`, `did_change_notification` — NO `did_close_notification`
- **Request builders**: completion, hover, definition, references, rename, formatting, code actions, document symbols, workspace symbols, inlay hints, code lens, semantic tokens
- **Response projectors**: `project_completion_response`, `project_hover_response`, `project_location_response`, `project_document_symbol_response`, `project_inlay_hint_response`
- **Diagnostics module**: `severity_from_lsp_value`, `protocol_range_from_lsp_json`, `diagnostic_code_label`
- **Supervisor**: lifecycle/restart/circuit-breaker management
- **Stdio launcher**: Content-Length framing, process spawn

### legion-app — App Layer Composition (already wired)
- `LspSessionHandle` (`crates/legion-app/src/language/app_lsp.rs`): background worker thread, MPSC channels, `issue_request`, `send_did_change`, `send_did_open`, `try_drain_results` with DiagnosticBatch/ReadResult/TransportDead routing
- `LanguageToolingWorkflow` (lib.rs line 6245): holds `LanguageToolingProjection`, ingests diagnostics and read results into projection state, wired into `AppComposition` with frame-tick polling

### legion-protocol — All Projection Types Defined
- `LanguageToolingProjection` (line 16470): `problems: Vec<LanguageProblemProjection>`, `completions`, `hover`, `definitions`, `references`, `outline`, `inlay_hints`, `code_lenses`, `lsp_session_status`
- Each row type has range, label, severity/kind, redaction hints, schema version

### legion-ui — Intents Exist
- `request_hover`, `request_completion`, `request_definition` intents exported
- `LspServerHealthProjection` and `project_lsp_health` available

### legion-desktop — THE GAP
- `code_canvas_painter.rs` has NO squiggly-underline, completion popup, or hover tooltip painting
- The view layer consumes projections but has no rendering paths for LSP-driven decorations

## What's Missing
1. **No diagnostic rendering** — no squiggly underlines, no error markers in the editor gutter
2. **No completion popup** — no dropdown menu for completions
3. **No hover tooltip** — no popup showing type info on hover
4. **No go-to-definition action** — no Ctrl+Click / F12 handler
5. **No `textDocument/didClose`** — missing from LSP client (minor)
6. **No inlay hints rendering** — type annotations inline (stretch goal)

## Architecture Decision
All LSP data flows through existing projections. The rendering layer reads `LanguageToolingProjection` fields and paints visual decorations. No new protocol types needed — everything is already defined.

## Plan Structure
| Plan | Wave | Description | Agent |
|------|------|-------------|-------|
| 03-01 | 1 | Diagnostic rendering + LSP client completion (didClose) | engineering-senior-developer |
| 03-02 | 2 | Completion popup + hover tooltip + go-to-definition | engineering-senior-developer |
