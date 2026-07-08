# M11 PKT-GP4 Evidence

Date: 2026-07-08
Branch: `m11/gp4-harness`

## Scope

PKT-GP4 completes `P6.F3.T4` and adds the twentieth standing gate:
`cargo run -p xtask -- golden-path-4`.

Implemented behavior:

- `xtask golden-path-4` spawns the app-owned `golden_path_4` binary and writes `target/golden-path/gp4_report.toml`.
- GP-4 exercises an approved editable plan that becomes a three-worker workflow session with two parallel lane starters and one dependent worker.
- GP-4 executes deterministic proposal-only workers and proves dependency ordering, policy denial, retry-budget exhaustion, failed verification blocking, conflict pause/resume, kill-switch cancellation under two seconds, command-center projections, and evidence bundle replay.
- Approved-plan session construction now propagates workspace ids into generated worker target summaries so task-packet validation accepts plan-built sessions as workspace-scoped.
- Workflow sign-off and merge approval now emit tagged comm rows (`REVIEW` and `APPROVAL`) through the app-owned command-center stream.
- Evidence bundle export now filters session-scoped kill-switch, risk, and tool-permission rows instead of replaying unrelated workflow state.
- `golden_path_2` is explicitly marked `required-features = ["ai"]`, matching its documented default-feature requirement and allowing no-default package checks to skip the AI-only GP-2 binary.
- The parallel-lane regression test now uses the same two-second lane barrier budget as the GP-4 harness, so it verifies concurrent lane dispatch without depending on sub-500ms thread scheduling under full-suite load.
- `.github/workflows/legion-smoke.yml` includes an independent GP-4 smoke job and uploads `gp4_report.toml` artifacts.
- Gate docs now list 20 standing gates including GP-4.
- Local phase-gate helper scripts now run the same 20 standing gates listed in `AGENTS.md`.

## Verification

Passed:

```powershell
cargo fmt --all --check
cargo check -p legion-app --no-default-features
cargo run -p xtask -- golden-path-4
cargo run -p xtask -- golden-path-1
cargo test -p legion-agent --test coordinator
cargo test -p legion-app --test legion_workflow_plan_lifecycle
cargo check -p xtask
cargo test -p legion-app --test legion_workflow_integration
```

GP-4 step result: PASS, 13/13 steps.

Full standing-gate verification was split to avoid losing long-running output:

- `target/m11-pkt-gp4-gates-prefix-r4.log`: gates 1-10 passed; start disk headroom `C_FREE_GB=79.39`.
- `target/m11-pkt-gp4-gates-tail-r5.log`: gates 11-20 passed, including clippy, cargo-deny, rust-analyzer smoke, GP-1/2/3/4, perf harness, verify-perf-harness, and update-drill.

`cargo deny check` exited 0 with existing unmatched-skip warnings for macOS-only objc2 crates.

## Review

Independent re-review found no remaining critical or important findings after these fixes:

- Evidence-bundle replay no longer leaks unrelated tool-permission rows.
- P6.F3.T4 acceptance text now describes the app-owned projection harness instead of overstating direct UI coverage.
- Local phase-gate scripts now invoke the full 20-gate set.

## Caveats

- GP-4 is a deterministic local harness using scripted providers; it does not make hosted provider calls.
- P6.F4 / ACP external-agent interoperability remains explicitly out of scope and deferred.
