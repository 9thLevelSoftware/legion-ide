# PKT-PLAN Evidence - M11 Plan Artifact

Branch: `m11/plan-artifact`
Date: 2026-07-07
Packet: PKT-PLAN / P6.F1 plan wiring

## Summary

Implemented the P6.F1 metadata-only plan lifecycle:
- directive/spec/task graph now creates an editable plan and records revision 1,
- revisions, approvals, and rejections append audited durable plan revision rows,
- approved plans produce a workflow DAG while unapproved or rejected plans do not,
- approved plans can build Legion workflow sessions through a new coordinator builder,
- desktop bridge actions map plan submit/approve/reject requests into app requests,
- storage persists plan revisions additively without bumping `PersistedState` schema version 3.

This packet did not start PKT-WORKERS and did not alter worker execution.

## Changed Files

- `crates/legion-agent/src/coordinator.rs`
- `crates/legion-agent/src/lib.rs`
- `crates/legion-agent/tests/coordinator.rs`
- `crates/legion-app/src/lib.rs`
- `crates/legion-app/src/offline_ai.rs`
- `crates/legion-app/tests/legion_workflow_plan_lifecycle.rs`
- `crates/legion-desktop/src/bridge.rs`
- `crates/legion-desktop/src/view.rs`
- `crates/legion-desktop/src/view/plan_editor.rs`
- `crates/legion-desktop/src/workflow.rs`
- `crates/legion-desktop/tests/plan_editor.rs`
- `crates/legion-storage/src/lib.rs`
- `crates/legion-storage/src/plan.rs`
- `crates/legion-storage/tests/plan_revisions.rs`
- `plans/kanban/legion-ga-backlog.toml`
- `.superpowers/sdd/progress-m11-campaign.md`
- `.superpowers/sdd/task-PKT-PLAN-M11-report.md`

## TDD RED Evidence

Failing tests were added before implementation:

```powershell
cargo test -p legion-agent --test coordinator
```

Result: RED. The test failed to compile because `legion_agent::coordinator` did not exist.

```powershell
cargo test -p legion-storage --test plan_revisions
```

Result: RED. The test failed to compile because the plan revision repository API and persistence hooks did not exist.

```powershell
cargo test -p legion-app --test legion_workflow_plan_lifecycle
```

Result: RED. The test failed to compile because the app lifecycle APIs and approved-plan session builder path did not exist.

```powershell
cargo test -p legion-desktop --test plan_editor
```

Result: RED. The first run timed out during concurrent red compilation; after the bridge variants were introduced, the desktop compile path also exposed missing app-request handling until workflow wiring was added.

## GREEN Evidence

```powershell
CARGO_PROFILE_DEV_DEBUG=0 CARGO_INCREMENTAL=0 cargo test -j 1 -p legion-agent --test coordinator
```

Result: PASS. 3 coordinator tests passed: worker per task with stable IDs, `depends_on` dependency edges, and unknown dependency rejection.

```powershell
CARGO_PROFILE_DEV_DEBUG=0 CARGO_INCREMENTAL=0 cargo test -j 1 -p legion-storage --test plan_revisions
```

Result: PASS. 3 storage plan revision tests passed, including file-backed save/load round trip.

```powershell
CARGO_PROFILE_DEV_DEBUG=0 CARGO_INCREMENTAL=0 cargo test -j 1 -p legion-app --test legion_workflow_plan_lifecycle
```

Result: PASS. 4 app lifecycle tests passed: happy path, rejection/no-DAG, revision ledger reload, and non-zero audit IDs.

```powershell
CARGO_PROFILE_DEV_DEBUG=0 CARGO_INCREMENTAL=0 cargo test -j 1 -p legion-desktop --test plan_editor
```

Result: PASS. 3 desktop plan editor tests passed, including submit/approve/reject bridge mappings.

```powershell
cargo check -p legion-app --lib --no-default-features
```

Result: PASS. The app lifecycle APIs compile through the library-only no-default cfg path.

```powershell
cargo run -p xtask -- verify-kanban-backlog
```

Result: PASS. Output: `kanban backlog ok: 10 epic(s), 38 feature(s), 146 task(s)`.

```powershell
git diff --check
```

Result: PASS. No whitespace errors were reported. Git emitted line-ending warnings for touched files on Windows.

## Standing Gate Evidence

```powershell
cargo fmt --all --check
```

Result: PASS after the rustfmt gate cleanup commit.

```powershell
cargo run -p xtask -- check-deps
cargo run -p xtask -- docs-hygiene
cargo run -p xtask -- claim-audit
cargo run -p xtask -- no-egui-textedit
cargo run -p xtask -- verify-kanban-backlog
cargo run -p xtask -- release-pipeline --dry-run
cargo run -p xtask -- verify-release-pipeline
cargo fmt --all --check
cargo check --workspace --all-targets
cargo test --workspace --all-targets
cargo clippy --workspace --all-targets -- -D warnings
cargo deny check
cargo run -p xtask -- rust-analyzer-smoke
cargo run -p xtask -- golden-path-1
```

Result: PASS at commit `0a87a72`, recorded in `target/m11-pkt-plan-full-gates-r5.log`. The command wrapper later timed out during the GP-2 portion, so the tail gates were rerun with explicit exit codes.

```powershell
cargo run -p xtask -- golden-path-2
cargo run -p xtask -- golden-path-3
cargo run -p xtask -- perf-harness
cargo run -p xtask -- verify-perf-harness
cargo run -p xtask -- update-drill
```

Result: PASS at commit `0a87a72`, recorded in `target/m11-pkt-plan-tail-gates-r5b.log`.

Gate cleanup performed before the final pass:
- formatted the pre-existing rustfmt drift that blocked `cargo fmt --all --check`,
- aligned dependency policy with active `legion-agent -> legion-debug` and `legion-app -> legion-sandbox` edges,
- fixed Clippy warnings in observability, AI provider parsing, updater sizing, diagnostics test helpers, and the plan editor bridge tests.

## Environment Notes

- During focused desktop verification, MSVC linking first hit a PDB/file-system limit and then the workspace `target/debug` tree exhausted drive space. The generated `target/debug` directory was verified to resolve inside `C:\Users\dasbl\RustroverProjects\legion-ide\target\debug` and removed. Re-running with `CARGO_PROFILE_DEV_DEBUG=0`, `CARGO_INCREMENTAL=0`, and `-j 1` passed.
- `cargo check -p legion-app --lib --no-default-features` passed for this packet's app lifecycle surface. The package-wide no-default binary path is not a standing gate and was not used as PKT-PLAN closure evidence.

## Kanban

Marked these tasks done with this evidence file:
- `P6.F1.T1`
- `P6.F1.T2`
- `P6.F1.T3`
