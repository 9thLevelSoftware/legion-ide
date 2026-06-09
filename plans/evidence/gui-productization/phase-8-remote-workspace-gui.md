# GUI Phase 8 remote workspace GUI evidence

## Status

- Remote workspace GUI: supported.
- Phase 8 acceptance: not final; this artifact covers Plan 08-04 only.

## Scope

The desktop GUI now exposes remote workspace runtime availability, session state, terminal/LSP/filesystem descriptor status, reconnect/offline indicators, and remote proposal review metadata through app-owned projections and actions.

Covered behavior:

- `RemoteGuiProjection` summarizes runtime availability, connected/reconnecting/offline session counts, session rows, and proposal review rows.
- Remote session rows show session id, redacted authority label, agent version, lifecycle state, filesystem descriptor status, terminal descriptor status, LSP descriptor status, reconnect support, offline state, and proposal review counts.
- Remote proposal rows show proposal id, authority label, payload kind, lifecycle state, and explicit proposal-mediated review wording.
- Desktop remote actions validate runtime/session/proposal projection state before dispatch.
- Remote connect and reconnect route through `AppComposition::connect_remote_workspace_session`.
- Remote proposal review routes through existing proposal details instead of duplicating proposal lifecycle authority.

## Preserved Boundaries

- Remote runtime state remains app-owned. `legion-ui` stores projections only and owns no remote sessions, editor text, proposal state, terminal execution, LSP authority, or transport state.
- `legion-desktop` validates against projection metadata and emits app/proposal requests only; it does not parse raw transport frames or apply remote operations.
- Remote writes remain proposal-mediated. Existing app remote tests still prove denied remote writes and accepted remote fixture writes do not mutate local disk directly.
- Terminal and LSP rows are descriptor/status metadata only. They do not expose shell output bodies, PTY transcripts, LSP payloads, source text, file contents, secrets, or private keys.
- Evidence and GUI rows contain metadata only. They do not include raw remote transport frames, remote command output bodies, LSP payload bodies, dirty buffer text, prompt text, or file contents.

## Verification

| Command | Result |
|---|---|
| `rg -q "Remote" crates/legion-protocol/src/lib.rs` | passed |
| `rg -q "remote" crates/legion-ui/src/ui.rs` | passed |
| `rg -q "remote_session" crates/legion-app/src/lib.rs` | passed |
| `rg -q "RemoteWorkspaceSession" crates/legion-protocol/src/lib.rs` | passed |
| `rg -q "Remote" crates/legion-desktop/src/bridge.rs` | passed |
| `rg -q "remote workspace" crates/legion-desktop/src/view.rs` | passed |
| `cargo test -p legion-desktop remote_workspace_gui -- --nocapture` | passed, 3 matching tests |
| `cargo test -p legion-app remote -- --nocapture` | passed, 2 matching tests |
| `cargo test -p legion-ui -- --nocapture` | passed, 15 tests |
| `cargo run -p xtask -- check-deps` | passed |
| `cargo fmt --all --check` | passed |

## Evidence Notes

- `remote_workspace_gui_bridge_routes_actions_with_projection_validation` proves runtime-disabled connect denial, invalid session denial, unknown-session denial, unknown-proposal denial, connect routing through an app request, and remote proposal review routing through `OpenProposalDetails`.
- `remote_workspace_gui_rows_show_reconnect_offline_terminal_lsp_and_proposals` proves reconnect/offline visibility, terminal descriptor status, LSP descriptor status, filesystem status, metadata-only redaction wording, and proposal-mediated remote review wording.
- `remote_workspace_gui_workflow_reports_connect_without_local_mutation` proves desktop connect uses app-owned remote authority, projects a connected remote session row, and does not mutate local disk or dirty editor text.
- `cargo test -p legion-ui -- --nocapture` proves the added remote projection field is carried by shell snapshots without breaking existing projection-only UI contracts.
- Existing app remote tests still prove remote runtime is disabled by default, session descriptors are app-owned projection data, remote audit records are metadata-only, remote writes require proposal linkage, and local disk is not mutated by remote operation receipt.

## Residual Risk

- This evidence does not mark final GUI Phase 8 accepted. Delegated task command center, GA operations evidence, platform parity, final evidence checks, and repository-wide gates still have to pass in later Phase 8 plans.
