# Plan 05-03 Result: Proposal Lifecycle App Routing And Details Population

Status: Complete

## Files Changed

- `crates/devil-app/src/lib.rs`
- `crates/devil-app/tests/control_trust_surfaces.rs`

## Decisions

- Added an app-owned proposal lookup and selected-proposal id in `AppProposalCoordinator`; UI and desktop still pass only projection ids.
- Routed preview, approve, reject, apply, rollback, cancel, and details intents before generic command dispatch so they no longer collapse to `Noop`.
- Kept proposal mutation authority inside `AppComposition::handle_proposal_request`; UI-originated apply and rollback reuse the existing proposal preconditions and audit flow.
- Built selected-proposal context, privacy, permission budget, approval checklist, and checkpoint/rollback projections from existing protocol helpers, preferring selected proposal details over stale Phase 4 trust state.

## Verification

| Command | Result |
|---|---|
| `rg -q "proposal_for_id" crates/devil-app/src/lib.rs` | Pass |
| `rg -q "route_proposal_intent" crates/devil-app/src/lib.rs` | Pass |
| `rg -q "approval_checklist_from_trust_projections" crates/devil-app/src/lib.rs` | Pass |
| `cargo test -p devil-app --test control_trust_surfaces proposal_lifecycle -- --nocapture` | Pass: 1 test passed |
| `cargo test -p devil-app --test control_trust_surfaces proposal_details -- --nocapture` | Pass: 1 test passed |
| `cargo test -p devil-app --test control_trust_surfaces proposal_states_visible -- --nocapture` | Pass: 1 test passed |
| `cargo test -p devil-app --test workspace_vfs_integration rollback -- --nocapture` | Pass: 4 tests passed |
| `cargo check -p devil-app --all-targets` | Pass |

## Open Issues

None.
