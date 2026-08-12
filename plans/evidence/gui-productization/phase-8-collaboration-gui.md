# GUI Phase 8 collaboration GUI evidence

## Status

- Collaboration GUI: supported.
- Phase 8 acceptance: not final; this artifact covers Plan 08-03 only.

## Scope

The desktop GUI now exposes collaboration session, reconnect/offline, conflict, presence, and shared proposal review metadata through app-owned projections and actions.

Covered behavior:

- `CollaborationGuiProjection` summarizes runtime availability, presence availability, session rows, reconnecting sessions, conflict sessions, offline sessions, and shared proposal rows.
- Desktop rows show session state, presence count, reconnecting participant count, conflict count, operation/acknowledgement/gap counts, and metadata-only redaction notes.
- Shared proposal rows show proposal id, required/authorized approver counts, approval/denial/pending counts, linked operation count, stale status, and proposal-mediated review wording.
- Desktop collaboration actions validate runtime/session/proposal projection state before dispatch.
- Join, leave, and presence actions route through existing app-owned collaboration intents.
- Shared proposal review routes through existing proposal details instead of duplicating proposal lifecycle authority.

## Preserved Boundaries

- Collaboration runtime state remains app-owned. `legion-ui` stores projections only and owns no collaboration sessions, editor text, proposal state, or transport state.
- `legion-desktop` validates against projection metadata and emits app/proposal intents only; it does not apply collaboration operations to editor text.
- Collaboration operation application remains in `AppComposition::receive_collaboration_transport_envelope`, where accepted operations go through editor authority.
- Shared collaboration proposal application remains proposal-mediated and requires app-owned approval evidence before apply.
- Evidence and GUI rows contain metadata only. They do not include raw collaboration transport payloads, operation bodies, editor text, prompt text, or file contents.

## Verification

| Command | Result |
|---|---|
| `rg -q "Collaboration" crates/legion-protocol/src/lib.rs` | passed |
| `rg -q "collaboration" crates/legion-ui/src/ui.rs` | passed |
| `rg -q "collaboration" crates/legion-app/src/lib.rs` | passed |
| `rg -q "Collaboration" crates/legion-desktop/src/bridge.rs` | passed |
| `rg -q "shared proposal" crates/legion-desktop/src/view.rs` | passed |
| `cargo test -p legion-desktop collaboration_gui -- --nocapture` | passed, 3 matching tests |
| `cargo test -p legion-app collaboration -- --nocapture` | passed, 4 matching tests |
| `cargo run -p xtask -- check-deps` | passed |
| `cargo fmt --all --check` | passed |

## Evidence Notes

- `collaboration_gui_bridge_routes_actions_with_projection_validation` proves runtime-disabled join denial, unknown-session denial, invalid participant denial, and shared proposal review routing through `OpenProposalDetails`.
- `collaboration_gui_rows_show_reconnect_conflict_and_shared_proposal_metadata` proves reconnect, conflict, shared proposal, and metadata-only row wording.
- `collaboration_gui_workflow_reports_join_and_presence_outcomes` proves desktop workflow outcomes for app-owned join and metadata-only presence publication while preserving the active buffer text.
- Existing app collaboration tests still prove collaboration runtime is disabled by default, presence is app-owned projection data, collaboration operations use editor authority, and disk is not mutated by collaboration operation receipt.

## Residual Risk

- This evidence does not mark final GUI Phase 8 accepted. Remote workspace GUI, delegated task command center, GA operations, platform parity, final evidence checks, and repository-wide gates still have to pass in later Phase 8 plans.
