# Plan 05-06 Result: Control Surface Safety Regression Suite

Status: Complete

## Files Changed

- `crates/devil-app/tests/control_trust_surfaces.rs`
- `plans/evidence/gui-productization/phase-5-control-trust-safety.md`

## Decisions

- Added a focused app regression named `dirty_text_preserved_on_rejected_stale_and_conflict_outcomes` so the safety suite has a direct filter proving rejected, stale, and conflict proposal outcomes preserve dirty editor text.
- Reused the 05-05 desktop view and bridge tests as safety proof because they already exercise detailed control/trust/assistant rendering, proposal/AI action translation, typed bridge errors, and projection-only boundaries.
- Created safety evidence as a pre-acceptance artifact; final full-gate acceptance remains owned by 05-07.

## Verification

| Command | Result |
|---|---|
| Prior result scan for unresolved stop markers across 05-01 through 05-05 | Pass: no unresolved markers found |
| `cargo test -p devil-app --test control_trust_surfaces proposal_states_visible -- --nocapture` | Pass: 1 test passed |
| `cargo test -p devil-app --test control_trust_surfaces dirty_text_preserved -- --nocapture` | Pass: 1 test passed |
| `cargo test -p devil-app --test control_trust_surfaces proposal_details -- --nocapture` | Pass: 1 test passed |
| `cargo test -p devil-app --test control_trust_surfaces assisted_ai_propose_is_proposal_only -- --nocapture` | Pass: 1 test passed |
| `cargo test -p devil-app --test control_trust_surfaces assisted_ai -- --nocapture` | Pass: 3 tests passed |
| `cargo test -p devil-app --test control_trust_surfaces -- --nocapture` | Pass: 7 tests passed |
| `cargo test -p devil-desktop --test control_trust_view -- --nocapture` | Pass: 3 tests passed |
| `cargo test -p devil-desktop --test control_trust_bridge -- --nocapture` | Pass: 5 tests passed |
| `cargo test -p devil-ui control_trust -- --nocapture` | Pass: 2 tests passed |
| `rg -q "No UI or desktop authority creep" plans/evidence/gui-productization/phase-5-control-trust-safety.md` | Pass |

## Open Issues

None.
