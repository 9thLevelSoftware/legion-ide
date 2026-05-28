# GUI Phase 8 delegated task command-center evidence

## Status

- Delegated task command center: approval-gated.
- Autonomous apply: unsupported.
- Phase 8 acceptance: not final; this artifact covers Plan 08-05 only.

## Scope

The desktop GUI now exposes delegated task plan contracts as command-center metadata without activating agent execution, provider calls, terminal commands, tools, editor mutation, or workspace writes.

Covered behavior:

- `AppComposition` composes delegated task projections from app-owned `DelegatedTaskPlanContract` records using `delegated_task_projection_from_plan_contracts`.
- Runtime activation remains `NotEncoded`.
- Desktop rows show plan state, readiness, step summaries, blockers, refusals, trust gates, proposal-preview links, audit readiness, and plan-only disclaimers.
- Desktop delegated actions validate projected plan ids and delegated proposal-preview links before routing.
- Delegated proposal preview/details actions route through existing proposal authority.
- Plan inspection produces a metadata-only workflow outcome that explicitly says approval-gated and autonomous apply unsupported.

## Preserved Boundaries

- `devil-agent`, `devil-memory`, and `devil-tracker` were not activated.
- Delegated task command-center rows are projection-only and metadata-only.
- `devil-desktop` does not execute delegated tasks, apply plan output, write files, mutate editor buffers, invoke providers, run tools, or launch terminals.
- Proposal-preview links remain proposal-mediated and require projected proposal ids.
- Health text explicitly preserves the unsupported autonomous apply boundary.
- Evidence and GUI rows contain no raw prompts, raw context manifests, raw generated diffs, source text, dirty buffer text, provider payloads, terminal output bodies, secrets, or private keys.

## Verification

| Command | Result |
|---|---|
| `rg -q "DelegatedTask" crates/devil-app/src/lib.rs` | passed |
| `rg -q "delegated" crates/devil-desktop/src/view.rs` | passed |
| `rg -q "autonomous" crates/devil-desktop/src/health.rs` | passed |
| `rg -q "delegated task command center" crates/devil-desktop/src/view.rs` | passed |
| `rg -q "Delegated" crates/devil-desktop/src/bridge.rs` | passed |
| `rg -q "NotEncoded" crates/devil-app/src/lib.rs` | passed |
| `cargo test -p devil-desktop delegated_task_command_center -- --nocapture` | passed, 3 matching tests |
| `cargo test -p devil-ui delegated -- --nocapture` | passed, 1 matching test |
| `cargo test -p devil-desktop operational_health -- --nocapture` | passed, 2 matching tests |
| `cargo run -p xtask -- check-deps` | passed |
| `cargo fmt --all --check` | passed |

## Evidence Notes

- `delegated_task_command_center_rows_show_gates_blockers_refusals_and_audit` proves plan count, blocked/refused counts, approval gate rows, blocker/refusal rows, proposal-mediated preview rows, audit readiness, `NotEncoded`, and autonomous-apply unsupported wording.
- `delegated_task_command_center_bridge_routes_review_actions_and_denies_unknown_links` proves delegated plan inspection routing, proposal preview/details routing, unknown plan denial, unknown delegated proposal-preview denial, and missing proposal ledger denial.
- `delegated_task_command_center_app_projection_and_workflow_remain_plan_only` proves app-owned plan-contract projection composition, `NotEncoded` runtime activation, workflow plan inspection outcome, and no local disk mutation.
- `operational_health` tests prove the health unsupported-surface labels remain metadata-only after making the lowercase autonomous verification label explicit.

## Residual Risk

- This evidence does not mark final GUI Phase 8 accepted. GA operations evidence, platform parity, final evidence checks, and repository-wide gates still have to pass in later Phase 8 plans.
