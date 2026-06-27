# Review: Tests - Remaining Crates

Scope: 57 assigned integration-test files under protocol, security, plugin, AI, LSP, UI, editor, storage, sandbox, project, terminal, index, vscode-compat, remote, and xtask.

Verification performed:
- Read the assigned file set with static scans for stubs, ignored tests, sleeps, temp-dir handling, process/network fixtures, env mutation, and unchecked commands.
- Ran `cargo test --workspace --tests --no-run` from `/Users/christopherwilloughby/legion-ide`; it completed successfully and compiled the assigned test targets.

Findings count: 21
Severity breakdown: critical 0, high 2, medium 10, low 9

## crates/legion-ai-providers/tests/smoke.rs

### Finding 1
- Category: failure-point
- Severity: medium
- Line numbers: 72-77
- Description: The hosted Anthropic smoke path silently returns success when credentials are unavailable. In normal CI this means the only live provider validation can pass without exercising token counting or completion at all.
- Suggested fix direction: Split this into an explicitly ignored/live-only test or gate it behind a required CI feature/job that fails when the live smoke is expected but credentials are missing. Keep the recorded local fixture as the default deterministic test.

## crates/legion-ai-providers/tests/mcp_ga_conformance.rs

### Finding 2
- Category: failure-point
- Severity: high
- Line numbers: 230-237, 444-446
- Description: `spawn_http_fixture` accepts exactly four HTTP requests and the test unconditionally joins the fixture thread. If the client sends fewer than four requests, fails before the tool call, or blocks while reading, the fixture thread remains stuck in `accept()` and the join can hang the test binary instead of producing a useful failure.
- Suggested fix direction: Add listener/read timeouts, return the join handle plus a shutdown channel, or accept until the client closes with bounded timeouts. Avoid fixed request counts for conformance fixtures unless every early-failure path can still unblock the listener.

### Finding 3
- Category: failure-point
- Severity: low
- Line numbers: 166-175, 567-605
- Description: The stdio MCP fixtures hard-code `python3` as an external command. Environments that can compile the Rust workspace but do not have `python3` on PATH will fail these runtime tests for reasons unrelated to MCP behavior.
- Suggested fix direction: Use a Rust fixture binary built by Cargo, discover Python through a workspace toolchain variable, or skip with an explicit diagnostic only in a clearly marked optional smoke test.

## crates/legion-lsp/tests/stdio_transport_contract.rs

### Finding 4
- Category: failure-point
- Severity: medium
- Line numbers: 399-409
- Description: The real `rust-analyzer` smoke test is skipped by default and also returns success when the binary is unavailable. This leaves the real server launch path untested in default CI.
- Suggested fix direction: Move the test to an explicitly ignored/live smoke target or add a CI job that sets `LEGION_RUN_RUST_ANALYZER_SMOKE=1` and fails if `rust-analyzer` is absent.

### Finding 5
- Category: stub
- Severity: medium
- Line numbers: 533-705
- Description: Even when the rust-analyzer smoke is enabled, many projected responses are assigned to underscore variables without assertions (`_completion_rows`, `_hover`, `_definitions`, `_references`, `_locations`, `_signature_help`, `_outline`, `_workspace_symbols`, `_hints`, `_lenses`, `_folding_ranges`, `_semantic_tokens`). The test mostly proves requests do not error, not that the product projections are correct or non-empty.
- Suggested fix direction: Assert expected non-empty projected rows and key fields for each LSP surface, or narrow the test to the few surfaces it can validate deterministically.

## crates/legion-plugin/tests/hostile.rs

### Finding 6
- Category: failure-point
- Severity: low
- Line numbers: 74-84
- Description: `compile_fixture` writes generated wasm files into the system temp directory and never removes them. Repeated hostile-plugin runs leave stale artifacts behind.
- Suggested fix direction: Use a `tempfile::TempDir`/RAII fixture object that owns the wasm path for the duration of the test and deletes it on drop.

## crates/legion-plugin/tests/quotas.rs

### Finding 7
- Category: failure-point
- Severity: low
- Line numbers: 65-74
- Description: `write_fixture_wasm` writes generated wasm files into the system temp directory and never removes them. This leaks artifacts across repeated quota test runs.
- Suggested fix direction: Use an RAII temp directory or named temp file that is retained only while `WasmPluginHost::load_fixture` needs the path.

## crates/legion-editor/tests/large_file_streaming.rs

### Finding 8
- Category: failure-point
- Severity: medium
- Line numbers: 22-28
- Description: A default, non-ignored test allocates and opens a 100 MiB string. This can make ordinary test runs slow or memory-sensitive, especially when the harness runs tests in parallel with other large fixtures.
- Suggested fix direction: Gate the 100 MiB case behind an explicit large/performance test profile, or keep a smaller deterministic default test and run the 100 MiB case in a dedicated CI job with resource expectations.

## crates/legion-editor/tests/performance_suite.rs

### Finding 9
- Category: stub
- Severity: medium
- Line numbers: 333-395
- Description: `large_file_100mb_degraded_mode_measurement` records open, viewport, and edit latencies but never asserts latency budgets for the measured values. The test logs `open_elapsed`, `viewport_elapsed`, `p50`, and `p95`, then only asserts mode/payload/chunk-count properties, so performance regressions can pass.
- Suggested fix direction: Add explicit budget assertions for open, viewport, and edit percentiles, or rename/split this as a report-only measurement outside the pass/fail test suite.

### Finding 10
- Category: stub
- Severity: low
- Line numbers: 663-728
- Description: Two performance gate tests are permanently ignored (`undo_redo_latency_under_edit_burst` and `snapshot_retention_and_release`). Default CI does not exercise these latency/retention checks.
- Suggested fix direction: Move them to a dedicated performance suite that is scheduled in CI, or replace them with smaller deterministic default checks plus optional heavy benchmarks.

## crates/legion-project/tests/search_workspace.rs

### Finding 11
- Category: failure-point
- Severity: medium
- Line numbers: 155-164
- Description: `indexed_workspace_search_refreshes_after_file_changes` relies on a fixed 120 ms sleep before polling watcher events. This is timing-sensitive and can flake on slow or heavily loaded runners, or pass by accident on fast machines.
- Suggested fix direction: Poll with a bounded retry loop until the expected invalidation/change is observed, or expose a deterministic watcher flush/synchronization point in the workspace actor.

### Finding 12
- Category: stub
- Severity: low
- Line numbers: 168-210
- Description: The indexed-vs-live large fixture benchmark is ignored, so default test runs do not validate the intended large-workspace search behavior.
- Suggested fix direction: Keep the benchmark ignored only if it is run by a named performance job; otherwise add a smaller non-ignored coverage test for large-fixture indexing behavior.

## crates/legion-project/tests/watcher_recovery.rs

### Finding 13
- Category: failure-point
- Severity: low
- Line numbers: 120-124
- Description: The watcher recovery test sleeps for 80 ms even though the watcher is a deterministic mock. Fixed sleeps slow the suite and can hide missing synchronization in the actor.
- Suggested fix direction: Remove the sleep if the recovery is synchronous, or replace it with a bounded poll for the expected recovery event.

## crates/legion-project/tests/path_boundary.rs

### Finding 14
- Category: failure-point
- Severity: low
- Line numbers: 163-174, 276, 345, 367, 383, 433, 470, 507, 537, 563, 583, 608-609
- Description: Temp workspaces are manually removed at the end of each test instead of being owned by a drop guard. Any panic before the cleanup line leaks temp directories/files, including the outside symlink target in the Unix escape test.
- Suggested fix direction: Replace `create_temp_workspace() -> PathBuf` with an RAII temp workspace struct whose `Drop` removes the directory and any outside fixtures.

## crates/legion-remote/tests/cloud_lane_http_transport.rs

### Finding 15
- Category: bug
- Severity: medium
- Line numbers: 129-145, 137-139
- Description: `serve_one` reads the HTTP request with a single 4096-byte `read`. TCP does not guarantee the entire request arrives in one read, and larger or segmented requests will be truncated before the handler assertions inspect headers/body.
- Suggested fix direction: Read until `\r\n\r\n`, parse `Content-Length`, then read the full body (similar to the MCP fixture) with a timeout.

### Finding 16
- Category: failure-point
- Severity: medium
- Line numbers: 129-145, 293-343
- Description: Negative tests that assert policy rejection still call `serve_one`, spawning a listener thread whose handler should never run. Because `serve_one` returns only a URL and no join/shutdown handle, those listener threads block in `accept()` until process exit.
- Suggested fix direction: Do not spawn a listener for before-network rejection tests; use an invalid base URL or a transport mock that records whether it was called. If a listener is needed, return a handle and shut it down deterministically.

## xtask/tests/docs_hygiene.rs

### Finding 17
- Category: stub
- Severity: low
- Line numbers: 44-49
- Description: `placeholder_docs_hygiene_test_file_compiles` only writes and checks a README exists. The name and behavior indicate leftover placeholder coverage that does not exercise docs hygiene logic.
- Suggested fix direction: Remove the placeholder or replace it with a real assertion against `run_docs_hygiene`.

### Finding 18
- Category: error
- Severity: medium
- Line numbers: 239-251
- Description: `docs_hygiene_checks_untracked_markdown_in_git_repo` checks that `git init` and `git add` spawn, but it never checks `output.status.success()`. If either command fails, the test may proceed in a non-git directory and no longer validate the intended untracked-file behavior.
- Suggested fix direction: Capture each command output, assert success, and print stdout/stderr on failure as in other git fixture helpers.

## xtask/tests/perf_harness.rs

### Finding 19
- Category: stub
- Severity: medium
- Line numbers: 157-190
- Description: The tests named `perf_harness_unreachable_budget_marks_measurement_failed` and `perf_harness_tight_budget_classifies_measurement_failed` do not actually assert a failed measurement. They set budget 0, assert `Skipped`/not `Failed`, and only reference the `SkeletonStatus::Failed` enum variant, leaving the failure classification path untested.
- Suggested fix direction: Add a deterministic helper or fixture that constructs a measurement over budget and assert `SkeletonStatus::Failed` plus the expected failure message.

### Finding 20
- Category: failure-point
- Severity: high
- Line numbers: 196-215
- Description: `perf_harness_fail_on_budget_env_overrides_descriptor_budget` mutates process-global environment with `set_var`/`remove_var`. Rust integration tests can run concurrently within the same process, so another test reading `FAIL_ON_BUDGET_ENV` can race this mutation despite the comment saying there are no spawned threads.
- Suggested fix direction: Serialize env-mutating tests with a global mutex/serial test attribute, or refactor `apply_fail_on_budget_override` to accept an injected environment value for tests.

## xtask/tests/release_pipeline.rs

### Finding 21
- Category: failure-point
- Severity: low
- Line numbers: 19-25
- Description: `TempRepo::new` uses only `name` plus current nanoseconds for its temp directory and no process/thread-local counter. Parallel or rapid repeated construction with the same name can collide if the clock granularity is lower than nanoseconds.
- Suggested fix direction: Include `std::process::id()` and an atomic counter, as some other test fixtures in the workspace already do.

## Reviewed with no findings

The following assigned files were reviewed with no reportable bugs/stubs/errors/failure-points found in this pass:

- `crates/legion-protocol/tests/context_manifest.rs`
- `crates/legion-protocol/tests/dto_contracts.rs`
- `crates/legion-protocol/tests/plan_artifact.rs`
- `crates/legion-protocol/tests/scope_contracts.rs`
- `crates/legion-security/tests/org_policy_bundle.rs`
- `crates/legion-security/tests/path_policy_windows.rs`
- `crates/legion-security/tests/proposal_apply_gate.rs`
- `crates/legion-security/tests/proposal_auto_approval_policy.rs`
- `crates/legion-security/tests/risk_rules.rs`
- `crates/legion-security/tests/secrets.rs`
- `crates/legion-plugin/tests/tampered.rs`
- `crates/legion-plugin/tests/wit_abi.rs`
- `crates/legion-ai/tests/advisory_classifier.rs`
- `crates/legion-ai/tests/context_manifest.rs`
- `crates/legion-ai/tests/egress_equality.rs`
- `crates/legion-ai/tests/redaction.rs`
- `crates/legion-ai/tests/streaming.rs`
- `crates/legion-ai-providers/tests/prompt_stability.rs`
- `crates/legion-lsp/tests/document_sync_contract.rs`
- `crates/legion-lsp/tests/lifecycle_contract.rs`
- `crates/legion-lsp/tests/read_side_contract.rs`
- `crates/legion-lsp/tests/registry_contract.rs`
- `crates/legion-lsp/tests/rust_analyzer_launch.rs`
- `crates/legion-lsp/tests/write_side_contract.rs`
- `crates/legion-ui/tests/assist_inline_prediction.rs`
- `crates/legion-ui/tests/debug_projection.rs`
- `crates/legion-ui/tests/legion_workflow_board_projection.rs`
- `crates/legion-editor/tests/atomicity_and_retention.rs`
- `crates/legion-storage/tests/debug_breakpoints.rs`
- `crates/legion-storage/tests/plan_revisions.rs`
- `crates/legion-sandbox/tests/compile_profiles.rs`
- `crates/legion-sandbox/tests/escape_attempts.rs`
- `crates/legion-project/tests/debug_locator.rs`
- `crates/legion-project/tests/git_workflow.rs`
- `crates/legion-project/tests/harness_tools.rs`
- `crates/legion-terminal/tests/dap_adapter_fixture.rs`
- `crates/legion-terminal/tests/dap_client_state_machine.rs`
- `crates/legion-index/tests/index_workflows.rs`
- `crates/legion-index/tests/plugin_grammar.rs`
- `crates/legion-vscode-compat/tests/compat_report.rs`
- `xtask/tests/kanban_backlog.rs`
- `xtask/tests/legion_bench.rs`
- `xtask/tests/no_egui_textedit.rs`
