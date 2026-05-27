# GUI Phase 5 Control, Trust, And Assisted AI Evidence

## Acceptance Status

- Phase 5 acceptance: Accepted.
- Runtime surface status: GUI control, trust, and assisted-AI surfaces are implemented, safety-tested, and accepted for Phase 5.
- Legacy compatibility: `plans/evidence/phase-5/plugin-architecture-map.md` remains the accepted historical plugin Phase 5 evidence. This document is the active GUI productization Phase 5 evidence path.

## Scope

This evidence path covers the GUI Phase 5 roadmap goal: proposal control surfaces, selected proposal details, context manifest, privacy inspector, permission/risk/cost budgets, approval checklist, rollback/checkpoint metadata, and assisted-AI explain/propose flows.

The GUI phase does not authorize UI or desktop ownership of proposal lifecycle state, provider routing, editor text, workspace mutation, storage authority, raw-source retention, hosted-provider activation, or autonomous apply behavior.

## Required Artifacts

- `.planning/phases/05-control-trust-and-assisted-ai-surfaces/05-01-RESULT.md`
- `.planning/phases/05-control-trust-and-assisted-ai-surfaces/05-02-RESULT.md`
- `.planning/phases/05-control-trust-and-assisted-ai-surfaces/05-03-RESULT.md`
- `.planning/phases/05-control-trust-and-assisted-ai-surfaces/05-04-RESULT.md`
- `.planning/phases/05-control-trust-and-assisted-ai-surfaces/05-05-RESULT.md`
- `.planning/phases/05-control-trust-and-assisted-ai-surfaces/05-06-RESULT.md`
- `plans/evidence/gui-productization/phase-5-control-trust-safety.md`

## Required Commands

- `cargo run -p xtask -- check-deps`
- `cargo fmt --all --check`
- `cargo check --workspace --all-targets`
- `cargo test --workspace --all-targets`
- `cargo clippy --workspace --all-targets -- -D warnings`

## Requirement traceability

| Requirement | Evidence |
|---|---|
| R-010 Control and trust surfaces | `crates/devil-protocol/tests/dto_contracts.rs`, `crates/devil-ui/src/ui.rs`, `crates/devil-app/tests/control_trust_surfaces.rs`, `crates/devil-desktop/tests/control_trust_view.rs`, and result files `05-02` through `05-06` prove proposal ledger rows, selected proposal details, diff/target summaries, approval checklist, rollback/checkpoint, context manifest, privacy inspector, and permission/risk budget projections are populated and rendered as bounded metadata. |
| R-011 Assisted AI GUI | `crates/devil-app/tests/control_trust_surfaces.rs`, `crates/devil-desktop/tests/control_trust_view.rs`, `crates/devil-desktop/tests/control_trust_bridge.rs`, and `plans/evidence/gui-productization/phase-5-control-trust-safety.md` prove explain/propose flows are local-first/default-deny, refusal-visible, and proposal-only for generated edits. |
| R-013 Performance and reliability evidence | The final full gates below, existing Phase 2-4 evidence, and Phase 5 safety evidence prove the GUI track preserves dependency policy, formatting, compile health, workspace tests, clippy policy, save conflict/dirty text preservation, projection-only UI/desktop boundaries, and metadata-only trust surfaces. |

## Success-criterion traceability

| Phase 5 success criterion | Evidence |
|---|---|
| Proposal ledger, proposal details, diff/target summary, approval checklist, rollback/checkpoint, context manifest, privacy inspector, and permission/risk/cost budget panels are usable. | Plans `05-02`, `05-03`, and `05-05`; tests `control_trust_surfaces`, `control_trust_view`; safety evidence sections "Proposal Lifecycle Visibility" and "Metadata-Only Trust Data". |
| Assisted AI explain/propose flows use local-first/default-deny provider routing. | Plan `05-04`; tests `assisted_ai_explain_routes_metadata_only_without_proposal`, `assisted_ai_propose_is_proposal_only`, and `assisted_ai_refusals_visible_for_untrusted_workspace`; safety evidence section "Local And Default-Deny Routing". |
| AI-generated edits are proposals only and never self-applied. | `assisted_ai_propose_is_proposal_only` asserts proposal creation while editor buffer and disk remain unchanged; desktop bridge emits intents only. |
| Users can see what context was used, what was redacted or denied, and what risk labels apply. | Selected proposal trust projections in `05-03`, detailed desktop rows in `05-05`, and safety evidence show context, privacy, permission, approval, rollback, refusal, redaction, and risk rows. |
| Approval, rejection, cancellation, stale, conflict, failed, applied, and rolled-back states are visible. | `proposal_lifecycle_ui_intents_route_through_app_authority`, `proposal_states_visible_after_ui_apply_rejections_and_rollback`, and `dirty_text_preserved_on_rejected_stale_and_conflict_outcomes`. |

## Full Gate Results

| Command | Result |
|---|---|
| Prior result scan for unresolved stop markers across 05-01 through 05-06 | Pass |
| `cargo run -p xtask -- check-deps` | Pass |
| `cargo fmt --all --check` | Pass |
| `cargo check --workspace --all-targets` | Pass |
| `cargo test --workspace --all-targets` | Pass on rerun after updating stale Phase 4 AI shell-projection assertion to the Phase 5 selected-proposal detail contract |
| `cargo clippy --workspace --all-targets -- -D warnings` | Pass |

## Gate Adjustment Note

The first full workspace test run exposed a stale assertion in `workspace_vfs_integration_phase4_ai_run_is_context_inspectable_and_proposal_only`: the test expected the active shell context manifest to remain the Phase 4 run manifest after `start_ai_run`, while Phase 5 now intentionally prefers selected proposal detail projections in the shell. The test was updated to assert both contracts separately: shell trust details point at the selected proposal, and `inspect_ai_run` still returns the Phase 4 run context manifest. The targeted test and full workspace test gate passed after that update.

## Artifact Inventory

- Governance: `plans/dependency-policy.md`, `xtask/src/main.rs`, and this evidence file.
- Protocol/UI contracts: `crates/devil-protocol/src/lib.rs`, `crates/devil-protocol/tests/dto_contracts.rs`, and `crates/devil-ui/src/ui.rs`.
- App proposal routing: `crates/devil-app/src/lib.rs` and `crates/devil-app/tests/control_trust_surfaces.rs`.
- Assisted-AI routing: `crates/devil-app/src/lib.rs`, `crates/devil-ai/src/lib.rs`, `crates/devil-ai-providers/src/lib.rs`, `crates/devil-ui/src/ui.rs`, and focused app tests.
- Desktop controls: `crates/devil-desktop/src/view.rs`, `crates/devil-desktop/src/bridge.rs`, `crates/devil-desktop/src/workflow.rs`, and focused desktop tests.
- Safety evidence: `plans/evidence/gui-productization/phase-5-control-trust-safety.md`.
- Full gates: cargo gate commands listed above.

## Final Validation Checklist

- [x] Governance distinguishes GUI Phase 5 from historical plugin Phase 5 evidence.
- [x] Protocol and UI projection contracts carry metadata-only proposal, trust, privacy, budget, approval, rollback, and assisted-AI details.
- [x] App-owned proposal lifecycle controls route through proposal authority and preserve proposal-mediated mutation.
- [x] Assisted-AI explain/propose flows are local-first/default-deny, refusal-visible, and proposal-only for generated edits.
- [x] Desktop renders bounded control/trust/assistant rows and emits intents without owning product state.
- [x] Safety evidence proves no UI or desktop authority creep.
- [x] Full repository gates pass and are summarized here.
