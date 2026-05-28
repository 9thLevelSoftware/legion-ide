# Plan 08-04 Result: Remote Workspace Manager And Remote Status GUI Workflow

Status: Complete
Date: 2026-05-27

## Summary

Implemented remote workspace GUI projection data, desktop remote action validation, remote session/status/proposal rows, and explicit remote workflow outcomes while preserving app-owned remote runtime, proposal, editor, terminal, LSP, and transport authority boundaries.

## Files Changed

- `crates/devil-protocol/src/lib.rs`
- `crates/devil-ui/src/ui.rs`
- `crates/devil-app/src/lib.rs`
- `crates/devil-desktop/src/bridge.rs`
- `crates/devil-desktop/src/workflow.rs`
- `crates/devil-desktop/src/view.rs`
- `crates/devil-desktop/tests/remote_workspace_gui.rs`
- `plans/evidence/gui-productization/phase-8-remote-workspace-gui.md`
- `.planning/phases/08-advanced-platform-gui-ga/08-04-RESULT.md`

## Verification

| Command | Result |
|---|---|
| `rg -q "Remote" crates/devil-protocol/src/lib.rs` | passed |
| `rg -q "remote" crates/devil-ui/src/ui.rs` | passed |
| `rg -q "remote_session" crates/devil-app/src/lib.rs` | passed |
| `rg -q "RemoteWorkspaceSession" crates/devil-protocol/src/lib.rs` | passed |
| `rg -q "Remote" crates/devil-desktop/src/bridge.rs` | passed |
| `rg -q "remote workspace" crates/devil-desktop/src/view.rs` | passed |
| `cargo test -p devil-desktop remote_workspace_gui -- --nocapture` | passed, 3 matching tests |
| `cargo test -p devil-app remote -- --nocapture` | passed, 2 matching tests |
| `cargo test -p devil-ui -- --nocapture` | passed, 15 tests |
| `cargo run -p xtask -- check-deps` | passed |
| `cargo fmt --all --check` | passed |
| `rg -q "Remote workspace GUI: supported" plans/evidence/gui-productization/phase-8-remote-workspace-gui.md` | passed |
| `rg -q "proposal-mediated" plans/evidence/gui-productization/phase-8-remote-workspace-gui.md` | passed |
| `rg -q "cargo test -p devil-desktop remote_workspace_gui" .planning/phases/08-advanced-platform-gui-ga/08-04-RESULT.md` | passed |

## Decisions

- Added narrow metadata-only remote GUI DTOs in `devil-protocol` because remote session descriptors alone did not cover proposal review summaries or terminal/LSP/filesystem display labels.
- Threaded `RemoteGuiProjection` through `ShellProjectionSnapshot` without adding remote runtime ownership to `devil-ui`.
- Composed remote GUI projection in `AppComposition` from app-owned remote session descriptors and proposal ledger rows.
- Desktop connect is allowed only when the projection says remote runtime sessions are enabled. Remote proposal review requires projected session and proposal review rows.
- Remote proposal review opens existing proposal details instead of creating a separate proposal lifecycle path.

## Boundary Evidence

- `devil-ui` remains projection-only.
- `devil-desktop` emits app requests or proposal details intents only; it does not parse remote frames, launch terminals, execute LSP requests, apply remote operations, or write local disk.
- Remote runtime connection remains in `AppComposition::connect_remote_workspace_session`.
- Remote operation receipt remains in `AppComposition::receive_remote_transport_envelope`, where app-owned audit persistence stays metadata-only.
- Remote mutation remains proposal-mediated and app-gated.
- Evidence records no raw remote transport frames, shell output bodies, PTY transcripts, LSP payload bodies, source text, file contents, prompt text, secrets, or private keys.

## Blockers

None.
