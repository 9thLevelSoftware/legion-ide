# M11 PKT-LANES Evidence

Date: 2026-07-07
Branch: `m11/parallel-conflict-merge`

## Scope

PKT-LANES completes the M11 workflow lane execution slice for `P6.F2.T2`, `P6.F2.T3`, and `P6.F2.T4`.

Implemented behavior:

- Deterministic scheduler lanes drive workflow execution instead of a flat `next_ready_workers()` loop.
- Same-lane workers run concurrently with owned `Box<dyn ToolCallingProvider + Send>` providers.
- Sandbox allocation, proposal registration, coordinator mutation, dependency satisfaction, artifact persistence, and cleanup remain serialized through `AppComposition`.
- Later dependency lanes only dispatch after current session dependency state is satisfied by completed predecessors.
- One shared `SharedCancellationFlag` is installed per session run; cancellation observed by workers marks them `Cancelled` and records a kill-switch decision-feed row.
- Unresolved conflicts that name or intersect a worker target pause dispatch, mark the worker `Blocked` with `legion_workflow.conflict_pause:{conflict_id}`, and place the session in `WaitingOnHuman`.
- `resolve_legion_workflow_conflict` restores paused worker states and is the only conflict-resolution path.
- `AppComposition::legion_workflow_merge_readiness_report` exposes verification-evidence citations over app session metadata.
- `AppComposition::export_legion_workflow_evidence_bundle` returns a serializable metadata-only DTO with session snapshot, packets, worker results, evidence records, decision feed, merge-readiness report, and projection replay inputs.
- `LegionWorkflowEvidenceBundle::replay_projection` deterministically rebuilds the live workflow projection from metadata.
- Verification evidence readiness now emits a single `MissingVerificationEvidence` blocker for missing, empty, or pending verification evidence.

## RED Evidence

Initial RED command:

```powershell
$env:CARGO_BUILD_JOBS='4'; cargo test -p legion-app --test legion_workflow_integration
```

Initial failure:

- `crates/legion-app/src/lib.rs` failed to compile because `LegionTaskPacket`, `LegionWorkerResult`, and `LegionEvidenceRecord` were not imported.
- `LegionWorkflowEvidenceBundle` derived serde traits while `legion_agent::merge_readiness::LegionWorkflowMergeReadinessReport` was not serde.

After the compile fix and first implementation pass, the same test binary ran and exposed behavior/test-contract failures:

- `legion_workflow_merge_readiness_report_blocks_ready_without_verification_evidence` seeded a protocol-invalid passed gate with missing evidence and was adjusted to use a valid pending gate with missing evidence.
- `legion_workflow_dependency_chain_resumes_without_rerunning_completed_worker` still expected one worker per execution pass; PKT-LANES now requires later lanes to dispatch after predecessors complete, so the test now asserts both workers complete in the first pass and a second pass does not rerun completed work.
- Controller review found the cancellation regression only proved one in-flight worker; the test was strengthened so a same-lane sibling also ends as `Cancelled` when the shared flag trips.
- Task review found that conflict pause still allowed later same-lane workers to dispatch and that target-intersection pauses without `worker_ids` could fail to restore. The regression was expanded to cover both cases, observed failing, and the scheduler/restore logic was fixed.

## Verification

Passed:

```powershell
$env:CARGO_BUILD_JOBS='4'; cargo test -p legion-app --test legion_workflow_integration legion_workflow_shared_kill_switch_cancels_inflight_worker_with_fast_ack
$env:CARGO_BUILD_JOBS='4'; cargo test -p legion-app --test legion_workflow_integration legion_workflow_unresolved_conflict_pauses_dispatch_until_resolved
$env:CARGO_BUILD_JOBS='4'; cargo test -p legion-app --test legion_workflow_integration
$env:CARGO_BUILD_JOBS='4'; cargo test -p legion-app --test delegated_task_integration
$env:CARGO_BUILD_JOBS='4'; cargo test -p legion-app --test manual_zero_egress
$env:CARGO_BUILD_JOBS='4'; cargo test -p legion-agent --test merge_readiness
$env:CARGO_BUILD_JOBS='4'; cargo test -p legion-ui --test legion_workflow_board_projection
$env:CARGO_BUILD_JOBS='4'; cargo check -p legion-app --no-default-features --features offline
$env:CARGO_BUILD_JOBS='4'; cargo run -p xtask -- verify-kanban-backlog
cargo fmt --all --check
git diff --check
$env:CARGO_BUILD_JOBS='4'; cargo clippy -p legion-app --all-targets -- -D warnings
```

The parallel verification batch printed Cargo lock waits, but all commands exited 0. The ignored SDD report at `.superpowers/sdd/task-PKT-LANES-M11-report.md` mirrors this history for local recovery.

Full standing gates:

```powershell
$freeGb = [math]::Round((Get-PSDrive -Name C).Free / 1GB, 2)
$env:CARGO_BUILD_JOBS='4'
# sequential 19-gate chain logged to target/m11-pkt-lanes-full-gates.log
```

Result: PASS at code commit `31b8321`; initial disk check reported `C_FREE_GB=80.54`. All 19 standing gates exited 0: `check-deps`, `docs-hygiene`, `claim-audit`, `no-egui-textedit`, `verify-kanban-backlog`, release dry-run, release verify, `cargo fmt --all --check`, workspace check, workspace tests, workspace Clippy, `cargo deny check`, rust-analyzer smoke, GP-1, GP-2, GP-3, perf harness, perf verify, and update drill.

## Caveats

- Live-provider cancellation remains cooperative at model/tool-loop boundaries. In-flight HTTP calls cannot be force-interrupted by this packet.
- Execution replay is intentionally not implemented. The export bundle supports deterministic metadata projection replay only.
- P6.F4 / ACP external-agent interop, PKT-CONSOLE, and PKT-GP4 remain deferred/out of scope.
