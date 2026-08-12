# M11 PKT-CONSOLE Evidence

Date: 2026-07-08
Branch: `m11/fleet-console`

## Scope

PKT-CONSOLE completes the M11 workflow command-center surface slice for `P6.F3.T1`, `P6.F3.T2`, and `P6.F3.T3`.

Implemented behavior:

- `legion-ui` projects workflow coordinator rows into five stable fleet board columns: Assigned, In Progress, Waiting on Human, Testing, and Done.
- `legion-desktop` renders the fleet board from `LegionWorkflowBoardColumnProjection` instead of computing column state in the renderer.
- `legion-ui` projects proposal-ledger rows into structured fleet cards with owner, model, lifecycle status, progress, files/context, risk, aggregate test status, mini diff, and last activity fields.
- `legion-desktop` renders fleet cards from `LegionWorkflowFleetCardProjection` without parsing logs or freeform row text.
- Fleet-card test status is card-linked when verification target labels match proposal targets and explicitly marked `unlinked` when production delegated-plan verification rows cannot be associated with one proposal.
- `legion-agent` exposes the documented tagged comm stream contract: PLAN, WRITE, TEST, REVIEW, ERROR, APPROVAL, and COMPLETE.
- `legion-app` records metadata-only workflow comm rows for session acceptance, worker scheduling, worker output/error/cancellation, verification updates, and kill-switch acknowledgement.
- `legion-ui` carries comm rows and per-worker budget rows through `ShellProjectionSnapshot` without granting UI authority to mutate workflow state.
- `legion-desktop` renders the command-center Agent Comm Stream from tagged comm rows and Budget Meter from structured budget rows.
- `legion-agent` derives metadata-only delegated-loop budget usage from audit steps; raw tool output payloads are not retained.

## RED Evidence

Initial RED tests were added before implementation:

```powershell
$env:CARGO_BUILD_JOBS='4'; cargo test -p legion-agent comm::tests
$env:CARGO_BUILD_JOBS='4'; cargo test -p legion-ui legion_workflow_command_center_fields_roundtrip_without_ui_authority
```

Initial failure:

- `legion-agent::comm` did not expose `format_agent_comm_line`.
- `ShellProjectionSnapshot` and `Shell` did not carry command-center board/card/comm/budget projections.

Implementation then added the projection DTOs, desktop view-model adapters, app-owned comm/budget rows, and focused regression tests.

Task review findings and fixes:

- Review found fleet-card test status was initially a global aggregate copied into every card. Fixed by deriving linked status from structured proposal target labels, with an explicit `unlinked` fallback for current production delegated-plan verification labels.
- Re-review found strict target matching would hide current app-produced `delegated_task.plan_row.metadata_only` verification rows. Fixed by surfacing those rows as `unlinked` aggregate test status and adding a production-shape regression.
- Review found retry-limit budget pressure was not represented unless an explicit `BudgetExhausted` audit row existed. Fixed by including retry-limit exhaustion in `DelegatedTaskBudgetUsage::status_label` and adding a regression test.

## Verification

Passed:

```powershell
$env:CARGO_BUILD_JOBS='4'; cargo test -p legion-agent comm::tests
$env:CARGO_BUILD_JOBS='4'; cargo test -p legion-agent budget::tests
$env:CARGO_BUILD_JOBS='4'; cargo check -p legion-ui
$env:CARGO_BUILD_JOBS='4'; cargo test -p legion-ui projection::tests
$env:CARGO_BUILD_JOBS='4'; cargo test -p legion-ui legion_workflow_command_center_fields_roundtrip_without_ui_authority
$env:CARGO_BUILD_JOBS='4'; cargo check -p legion-app
$env:CARGO_BUILD_JOBS='4'; cargo test -p legion-desktop --test legion_workflow_command_center
$env:CARGO_BUILD_JOBS='4'; cargo test -p legion-app --test legion_workflow_integration
$env:CARGO_BUILD_JOBS='4'; cargo check -p legion-app --no-default-features --features offline
$env:CARGO_BUILD_JOBS='4'; cargo check -p legion-desktop --all-targets
$env:CARGO_BUILD_JOBS='4'; cargo clippy -p legion-agent --all-targets -- -D warnings
$env:CARGO_BUILD_JOBS='4'; cargo clippy -p legion-ui --all-targets -- -D warnings
$env:CARGO_BUILD_JOBS='4'; cargo clippy -p legion-desktop --all-targets -- -D warnings
cargo fmt --all --check
git diff --check
```

Full standing gates:

```powershell
$freeGb = [math]::Round((Get-PSDrive -Name C).Free / 1GB, 2)
$env:CARGO_BUILD_JOBS='4'
# sequential 19-gate chain logged to target/m11-pkt-console-full-gates.log
```

Result: PASS. The final disk check reported `C_FREE_GB=82.27`. All 19 standing gates exited 0: `check-deps`, `docs-hygiene`, `claim-audit`, `no-egui-textedit`, `verify-kanban-backlog`, release dry-run, release verify, `cargo fmt --all --check`, workspace check, workspace tests, workspace Clippy, `cargo deny check`, rust-analyzer smoke, GP-1, GP-2, GP-3, perf harness, perf verify, and update drill.

## Caveats

- P6.F3.T4 remains open. This packet surfaces existing risk monitor, approval, kill-switch, and budget metadata in the command center, but GP-4 end-to-end UI driving is the next packet scope.
- P6.F4 / ACP external-agent interoperability remains explicitly out of scope.
