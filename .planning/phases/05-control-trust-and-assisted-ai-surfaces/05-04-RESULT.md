# Plan 05-04 Result: Assisted AI Explain And Propose Routing

Status: Complete

## Files Changed

- `crates/devil-app/src/lib.rs`
- `crates/devil-ui/src/ui.rs`
- `crates/devil-app/tests/control_trust_surfaces.rs`
- `crates/devil-app/tests/workspace_vfs_integration.rs`

## Decisions

- Kept `start_ai_run` as a backward-compatible proposal wrapper and added app-owned explain/propose routing through `run_assisted_ai_operation`.
- Updated `AppAiRunOutcome` so explain and refusal paths can return `proposal_id: None`, `proposal_created: None`, and visible refusal metadata instead of opaque runtime errors.
- Added projection-only UI intents and command parsing for `:ai-explain` and `:ai-propose`; `:ai-start` remains a proposal-generation alias.
- Reused the existing local deterministic provider/router/security behavior. No AI provider or security production changes were needed after verification.
- Preserved the agent state machine by recording metadata-only explain readiness as a legal non-mutating `Proposing` transition, not a direct Planning-to-Completed jump.

## Verification

| Command | Result |
|---|---|
| `rg -q "AssistedAiOperationClass::Explain" crates/devil-app/src/lib.rs` | Pass |
| `rg -q "proposal_id: Option<ProposalId>" crates/devil-app/src/lib.rs` | Pass |
| `rg -q "StartAiExplain" crates/devil-ui/src/ui.rs` | Pass |
| `rg -q "StartAiProposal" crates/devil-ui/src/ui.rs` | Pass |
| `cargo test -p devil-ai router -- --nocapture` | Pass: 6 tests passed |
| `cargo test -p devil-ai-providers --all-targets` | Pass: 4 tests passed |
| `cargo test -p devil-security provider -- --nocapture` | Pass: 4 tests passed; 4 filtered tests in path-policy target |
| `cargo test -p devil-ui assisted_ai -- --nocapture` | Pass: 2 tests passed |
| `cargo test -p devil-app --test control_trust_surfaces assisted_ai -- --nocapture` | Pass: 3 tests passed |
| `cargo check -p devil-app --all-targets` | Pass |

## Open Issues

None.
