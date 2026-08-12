# Plan 08-03 Result: Collaboration Presence And Shared Proposal GUI Workflow

Status: Complete
Date: 2026-05-27

## Summary

Implemented collaboration GUI projection data, desktop collaboration action validation, reconnect/conflict/shared proposal rows, and explicit collaboration workflow outcomes while preserving app/editor/proposal authority boundaries.

## Files Changed

- `crates/devil-protocol/src/lib.rs`
- `crates/devil-ui/src/ui.rs`
- `crates/devil-app/src/lib.rs`
- `crates/devil-desktop/src/bridge.rs`
- `crates/devil-desktop/src/workflow.rs`
- `crates/devil-desktop/src/view.rs`
- `crates/devil-desktop/tests/collaboration_gui.rs`
- `plans/evidence/gui-productization/phase-8-collaboration-gui.md`
- `.planning/phases/08-advanced-platform-gui-ga/08-03-RESULT.md`

## Verification

| Command | Result |
|---|---|
| `rg -q "Collaboration" crates/devil-protocol/src/lib.rs` | passed |
| `rg -q "collaboration" crates/devil-ui/src/ui.rs` | passed |
| `rg -q "collaboration" crates/devil-app/src/lib.rs` | passed |
| `rg -q "Collaboration" crates/devil-desktop/src/bridge.rs` | passed |
| `rg -q "shared proposal" crates/devil-desktop/src/view.rs` | passed |
| `cargo test -p devil-desktop collaboration_gui -- --nocapture` | passed, 3 matching tests |
| `cargo test -p devil-app collaboration -- --nocapture` | passed, 4 matching tests |
| `cargo run -p xtask -- check-deps` | passed |
| `cargo fmt --all --check` | passed |
| `rg -q "Collaboration GUI: supported" plans/evidence/gui-productization/phase-8-collaboration-gui.md` | passed |
| `rg -q "proposal-mediated" plans/evidence/gui-productization/phase-8-collaboration-gui.md` | passed |
| `rg -q "cargo test -p devil-desktop collaboration_gui" .planning/phases/08-advanced-platform-gui-ga/08-03-RESULT.md` | passed |

## Decisions

- Added narrow metadata-only collaboration GUI DTOs in `devil-protocol` because presence projections alone did not cover session state, reconnect/offline, conflict, or shared proposal review summaries.
- Threaded `CollaborationGuiProjection` through `ShellProjectionSnapshot` without adding collaboration runtime ownership to `devil-ui`.
- Composed collaboration GUI projection in `AppComposition` from app-owned runtime/session/shared proposal state.
- Desktop join is allowed only when the projection says runtime sessions are enabled. Leave, presence, and shared review actions require projected sessions or shared proposal rows.
- Shared proposal review opens existing proposal details instead of creating a separate proposal lifecycle path.

## Boundary Evidence

- `devil-ui` remains projection-only.
- `devil-desktop` emits collaboration intents or proposal details intents only; it does not apply collaboration operations.
- Collaboration operation application remains app/editor-authority code.
- Shared proposal application remains proposal-mediated and app-gated.
- Evidence records no raw collaboration transport payloads, operation bodies, editor text, prompt text, or file contents.

## Blockers

None.
