# PKT-WORKERS Evidence - M11 Real Workflow Workers

Branch: `m11/real-workers`
Date: 2026-07-07
Packet: PKT-WORKERS / P6.F2.T1 only

## Summary

Implemented the P6.F2.T1 workflow worker activation slice:
- `legion-app` now exposes `LegionWorkerProviderResolver` behind the default `ai` feature.
- `execute_legion_workflow_with_providers(session_id, resolver)` runs workflow workers through the real delegated task loop with a resolver-provided `ToolCallingProvider`.
- `execute_legion_workflow(session_id)` remains compiled in both default and offline cfgs and is now the honest no-provider path.
- Local and provider-backed no-provider workers block with `legion_workflow.worker_provider_unavailable`; provider-backed workers still emit `ProviderRouteRequired` and route metadata before blocking.
- Workflow local workers use the real sandboxed delegated loop, app-side proposal id reassignment, proposal lifecycle registration, coordinator `record_proposal_output`, worker completion/dependency satisfaction, evidence outputs, and sandbox cleanup.
- The old mock delegated-task execution path and ACP host hook are gated behind `test-helpers`/test cfg so existing delegated-task integration tests still compile without keeping the mock in the normal product path.

This packet did not start PKT-LANES threading/parallelism, PKT-CONSOLE, GP-4, or P6.F4/ACP external-agent work.

## Changed Files

- `crates/legion-app/Cargo.toml`
- `crates/legion-app/src/lib.rs`
- `crates/legion-app/tests/legion_workflow_integration.rs`
- `plans/kanban/legion-ga-backlog.toml`
- `.superpowers/sdd/progress-m11-campaign.md`
- `plans/evidence/production/M11/PKT-WORKERS-evidence.md`

## TDD Evidence

```powershell
cargo test -p legion-app --test legion_workflow_integration
```

Result: RED before implementation. The test failed to compile because `legion_app::LegionWorkerProviderResolver` did not exist.

## Green Evidence

```powershell
cargo test -p legion-app --test legion_workflow_integration
```

Result: PASS. 21 tests passed, including resolver-backed local worker proposal registration, no-provider blocking, resolver-`None` blocking, provider-backed route metadata plus provider-unavailable blocking, and `ToolCallRejected` evidence on rejected tool calls.

```powershell
cargo test -p legion-app --test delegated_task_integration
```

Result: PASS. 15 tests passed, proving the old delegated-task mock execution path still compiles through `test-helpers` while `start_delegated_task` real-loop tests remain green.

```powershell
cargo check -p legion-app --no-default-features --features offline
```

Result: PASS. The offline no-provider app build compiled. Cargo emitted offline-only unused-code warnings for AI-path helpers, but there were no errors.

```powershell
cargo test -p legion-app --test manual_zero_egress
```

Result: PASS. 1 zero-egress manual-mode test passed.

```powershell
cargo run -p xtask -- check-deps
```

Result: PASS. Dependency policy accepted the packet's `Cargo.toml`/feature-gating changes.

```powershell
cargo fmt --all --check
```

Result: PASS after formatting.

```powershell
cargo clippy -p legion-app --all-targets -- -D warnings
```

Result: PASS.

```powershell
cargo run -p xtask -- verify-kanban-backlog
```

Result: PASS. Output: `kanban backlog ok: 10 epic(s), 38 feature(s), 146 task(s)`.

## Standing Gate Evidence

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
```

Result: PASS at commit `3a6ed30`, recorded in `target/m11-pkt-workers-gates-prefix.log`.

```powershell
cargo run -p xtask -- golden-path-1
cargo run -p xtask -- golden-path-2
cargo run -p xtask -- golden-path-3
cargo run -p xtask -- perf-harness
cargo run -p xtask -- verify-perf-harness
cargo run -p xtask -- update-drill
```

Result: PASS at commit `3a6ed30`, recorded in `target/m11-pkt-workers-gates-tail.log`.

## Scope Notes

- `execute_legion_workflow_with_providers` is sequential. It does not spawn threads or introduce lane parallelism.
- Provider-backed workers can use the resolver path, but still emit route metadata before any provider-backed execution attempt. Without a provider they block instead of pretending route metadata is completion.
- The workflow runner records only the first loop proposal for a worker because the current coordinator result/evidence ids are worker-scoped. Multi-proposal worker aggregation remains outside P6.F2.T1.
