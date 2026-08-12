# Plan 05-05 Result: Desktop Control Trust Panels And Actions

Status: Complete

## Files Changed

- `crates/devil-desktop/src/view.rs`
- `crates/devil-desktop/src/bridge.rs`
- `crates/devil-desktop/src/workflow.rs`
- `crates/devil-desktop/tests/control_trust_view.rs`
- `crates/devil-desktop/tests/control_trust_bridge.rs`

## Decisions

- Expanded desktop view-model rows from summary-only proposal, trust, and assistant rows into bounded metadata detail rows for proposal diffs, targets, context summaries, manifest items, permissions, privacy records/refusals, budgets/evaluations, approval gates/blockers, checkpoint/rollback metadata, providers, routes, requests, refusals, and proposal previews.
- Added adapter-local proposal lifecycle and assisted-AI actions that translate only projected ids and display-safe labels into `CommandDispatchIntent`; desktop still does not construct proposal payloads, provider routes, editor mutations, or workspace writes.
- Added typed bridge errors for unknown proposal ids, unknown assisted-AI run ids, and empty assisted-AI instruction labels.
- Mapped new app-owned proposal and assisted-AI outcomes into desktop workflow statuses that distinguish detail-opened, previewed, refused, metadata-only explain, proposal-producing AI, replay, inspect, and cancellation results.

## Verification

| Command | Result |
|---|---|
| `rg -q "ApproveProposal" crates/devil-desktop/src/bridge.rs` | Pass |
| `rg -q "StartAiExplain" crates/devil-desktop/src/bridge.rs` | Pass |
| `rg -q "StartAiProposal" crates/devil-desktop/src/bridge.rs` | Pass |
| `rg -q "ProposalLifecycleUpdated" crates/devil-desktop/src/workflow.rs` | Pass |
| `cargo test -p devil-desktop --test control_trust_view -- --nocapture` | Pass: 3 tests passed |
| `cargo test -p devil-desktop --test control_trust_bridge -- --nocapture` | Pass: 5 tests passed |
| `cargo test -p devil-desktop --test projection_rendering control -- --nocapture` | Non-evidence: command completed with 0 tests matched and 4 filtered out; unfiltered projection rendering target passed below |
| `cargo test -p devil-desktop --test projection_rendering -- --nocapture` | Pass: 4 tests passed |
| `cargo check -p devil-desktop --all-targets` | Pass |

## Open Issues

None.
