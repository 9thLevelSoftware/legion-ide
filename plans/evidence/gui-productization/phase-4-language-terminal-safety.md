# Phase 4 Language And Terminal Safety Evidence

Acceptance status: Accepted

## Scope

Phase 4 adds the minimum local IDE language and terminal loop while preserving the existing boundaries:

- `legion-ui` remains projection-only and owns no editor text, editor sessions, terminal runtime, workspace actor, or disk mutation path.
- `legion-app` composes the activated language and terminal workflows.
- Edit-producing language actions are converted into workspace proposals before any mutation can occur.
- Terminal launch and lifecycle operations are policy-gated, bounded, metadata-redacted, and denied by default.

## Boundary Proof

- Dependency policy explicitly authorizes GUI Phase 4 `legion-app -> legion-index` and `legion-app -> legion-terminal` edges only.
- `crates/legion-ui/src/ui.rs` only emits `CommandDispatchIntent` variants and consumes `LanguageToolingProjection` / `TerminalPanelProjection`.
- `crates/legion-app/src/lib.rs` owns `LanguageToolingWorkflow` and `TerminalWorkflow`.
- Language edit actions call `convert_lsp_edit_to_workspace_proposal`, register proposal lifecycle context, and advance proposal rows through Created -> Validated -> Previewed before reporting a preview.
- Rename proposals target the identifier byte range at the requested position. Formatting, organize-imports, and code-action paths are safe no-op previews with warning diagnostics until a live LSP edit provider exists.
- Language projections clear prior buffer-specific rows when the active buffer/file identity changes, preventing stale hover/completion/outline data from surviving a buffer switch.
- Terminal workflow uses `DenyByDefaultBroker`, protocol terminal validators, and metadata-only `TerminalAuditRecord` storage/event emission for launch/input/resize/poll/search/close/kill. The deterministic fixture is only enabled in tests; default launch returns a visible denial.
- Desktop bridge/view render language and terminal rows from projection snapshots and route actions back to app authority.

## Regression Coverage

- `crates/legion-protocol/tests/dto_contracts.rs`
  - `language_terminal_projection_roundtrips_language_surface`
  - `language_terminal_projection_roundtrips_terminal_surface`
  - `language_terminal_projection_default_surfaces_are_inert`
- `crates/legion-app/tests/language_tooling_workflow.rs`
  - language projection refresh uses semantic/index data without UI text ownership
  - formatting/rename actions create proposal previews without editor or disk mutation
- `crates/legion-app/tests/terminal_workflow.rs`
  - terminal launch is denied by default
  - terminal launch is denied for untrusted workspaces even when fixture is enabled
  - fixture lifecycle supports launch, input, resize, poll, search, close, and preserves editor/disk text
  - terminal lifecycle operations cannot mutate editor buffers or disk content
- `crates/legion-app/tests/language_terminal_integration.rs`
  - language and terminal workflows share the app boundary without direct editor or disk mutation
  - language cancellation and default-denied terminal launch project fail-closed states
  - language proposal ledger rows reach Previewed and stale rows are dropped on buffer switches
- `crates/legion-desktop/tests/language_terminal_view.rs`
  - desktop view renders language and terminal rows from projections
  - desktop bridge maps language and terminal actions to projection-selected intents
- `crates/legion-desktop/tests/language_terminal_workflow.rs`
  - desktop runtime projects language cancellation and terminal denial rows
  - desktop bridge rejects missing active buffer/session state and routes projected language/terminal commands

## Verification Snapshot

Passed before final gate:

- `cargo run -p xtask -- check-deps`
- `cargo test -p legion-protocol --test dto_contracts language_terminal_projection -- --nocapture`
- `cargo check -p legion-ui --all-targets`
- `cargo check -p legion-app --all-targets`
- `cargo test -p legion-app --test language_terminal_integration -- --nocapture`
- `cargo test -p legion-app --test language_tooling_workflow -- --nocapture`
- `cargo test -p legion-app --test terminal_workflow -- --nocapture`
- `cargo test -p legion-desktop --test language_terminal_workflow -- --nocapture`
- `cargo test -p legion-desktop --test language_terminal_view -- --nocapture`
- `cargo check -p legion-desktop --all-targets`
- `cargo test -p legion-terminal --all-targets`
- `cargo test -p legion-security --all-targets`
- `cargo check --workspace --all-targets`
- `cargo test --workspace --all-targets`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo deny check` passed with duplicate-version warnings under the existing warning-level policy

Final full gate results are recorded in `plans/evidence/gui-productization/phase-4-language-terminal-ide-loop.md`.
