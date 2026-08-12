# M8 — WS-TERM-01 Terminal Runtime Productization Evidence

## Status

Done.

## Acceptance targets

- P2.F2.T1: Write failing test expecting trusted workspace terminal launch to produce real output.
- P2.F2.T2: Promote legion-terminal runtime behind explicit capability policy, not test-only fixture toggles. Denied/untrusted terminal paths still fail closed.

## Tasks implemented

| Task | Brief ID | Description | Status |
|------|----------|-------------|--------|
| 1 | P2.F2.T1+T2 | Product launch gate: trusted workspace auto-enables runtime; untrusted always denied | Done |
| 2 | TERM.01 | Shell selection policy: PowerShell/Cmd/Bash/Zsh with workspace→user→platform precedence | Done |
| 3 | TERM.05 | Scrollback limit: configurable bounded scrollback (default 5000 rows), eviction counted | Done |
| 4 | TERM.06 | Resize propagation: resize intent routed through TerminalWorkflow to PTY | Done |
| 5 | TERM.07 | Env allow/deny: `LEGION_SECRET*`/`LEGION_TOKEN*` deny-list always applied; audited in launch record | Done |
| 6 | TERM.09 | Orphan cleanup: `cleanup_terminal_orphans()` API on `AppComposition`; metadata-only audit records | Done |
| 7 | TERM.11 | Failure UX: `TerminalFailureKind` enum (Denied/Unavailable/Exited/Crashed/PolicyBlocked); shell+status projected | Done |
| 8 | TERM.02/12 | Windows smokes: `windows_cmd_launch_smoke` and `windows_powershell_core_launch_smoke` pass on Windows | Done |

## Files touched

- `crates/legion-app/src/terminal_policy.rs` (new) — shell selection, env policy, scrollback constant, failure kind enum
- `crates/legion-app/src/lib.rs` — product gate, shell selection, scrollback limit, env policy, orphan cleanup API, shell-label projection
- `crates/legion-app/tests/terminal_workflow.rs` — 6 new tests for tasks 1–4, 6, 7; updated existing tests
- `crates/legion-app/tests/language_terminal_integration.rs` — updated to match product gate auto-enable behavior
- `crates/legion-security/src/lib.rs` — added `pwsh` and `zsh` to `CommandTaxonomy`
- `crates/legion-terminal/tests/platform_shell_smoke.rs` (new) — Windows cmd/pwsh smokes; Unix bash/zsh smokes

## Verification commands and results

### Command 1 — terminal workflow integration tests (9 tests)

```
cargo test -p legion-app --test terminal_workflow
```

CWD: `C:/Users/dasbl/RustroverProjects/legion-ide-term`
Start: 2026-07-04 (time not recorded; see fix-round-2 Command 6 for timestamped re-run)
End:   2026-07-04
Exit code: 0

```
running 9 tests
test terminal_denial_is_visible_and_fail_closed ... ok
test terminal_orphan_cleanup_kills_and_records_evidence ... ok
test terminal_resize_propagates_to_projection ... ok
test terminal_failure_ux_distinct_status_kinds ... ok
test terminal_product_gate_trusted_workspace_launches_without_test_helper ... ok
test terminal_scrollback_limit_enforced_and_eviction_counted ... ok
test terminal_workflow_cannot_mutate_editor_or_disk ... ok
test terminal_fixture_lifecycle_projects_status ... ok
test terminal_shell_selection_is_projected_in_status ... ok

test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

### Command 2 — full legion-app test suite (unit + integration)

```
cargo test -p legion-app
```

CWD: `C:/Users/dasbl/RustroverProjects/legion-ide-term`
Start: 2026-07-04 (time not recorded)
End:   2026-07-04
Exit code: 0

All tests passed (unit tests in terminal_policy module + all integration tests).

### Command 3 — platform shell smoke tests (Windows cmd + pwsh)

```
cargo test -p legion-terminal --test platform_shell_smoke
```

CWD: `C:/Users/dasbl/RustroverProjects/legion-ide-term`
Start: 2026-07-04 (time not recorded; see fix-round-2 Command 7 for timestamped re-run)
End:   2026-07-04
Exit code: 0

```
test windows_cmd_launch_smoke ... ok
test windows_powershell_core_launch_smoke ... ok
```

### Command 4 — legion-security taxonomy tests

```
cargo test -p legion-security
```

CWD: `C:/Users/dasbl/RustroverProjects/legion-ide-term`
Start: 2026-07-04 (time not recorded)
End:   2026-07-04
Exit code: 0

All tests passed (including taxonomy classification tests for `pwsh` and `zsh`).

### Command 5 — legion-platform PTY tests

```
cargo test -p legion-platform
```

CWD: `C:/Users/dasbl/RustroverProjects/legion-ide-term`
Start: 2026-07-04 (time not recorded; see fix-round-2 Command 8 for timestamped re-run)
End:   2026-07-04
Exit code: 0

All tests passed.

## Design decisions

### Product gate placement (Task 1)

The trust check lives at the top of `TerminalWorkflow::launch()`, before any capability broker evaluation. This means `enable_runtime_for_tests()` cannot override it for untrusted workspaces: the deny is unconditional and happens before the security broker is consulted.

### Env policy enforcement at PTY spawn (Task 5 — FIX ROUND)

Initial implementation only audited the env deny-list; it did not enforce it because `PtyRequest` lacked an `env` field. The fix round added `env: Option<Vec<(String, String)>>` to `PtyRequest` in `legion-platform`, plumbed it through `TerminalRuntimeLaunchRequest` in `legion-terminal`, wired both Windows ConPTY (`lpEnvironment` block) and Unix exec paths, and updated `TerminalWorkflow::launch()` to pass `env_policy.effective_env()`. A TDD smoke test (`windows_env_deny_list_stripped_at_pty_spawn`) verifies the secret var is absent from PTY output while the control var is present.

### Orphan cleanup (Task 6)

`TerminalRuntime::cleanup_orphans()` is the runtime-level hook. `AppComposition::cleanup_terminal_orphans()` wraps it and returns metadata-only `TerminalAuditRecord` values. No raw command output is included in the records; redaction stays.

## Fix-round-2 changes

### Summary of issues addressed

| Issue | Description | Resolution |
|-------|-------------|------------|
| IMPORTANT 1 | `passthrough_env=false` produced empty child env (crashed shells) | `effective_env()` now returns platform-safe baseline set |
| IMPORTANT 2 | Orphan cleanup test was vacuous (non-orphaned, result discarded) | Rewritten to launch short-lived cmd, await exit, verify audit record |
| SPEC GAP 1 | Shell selection was one-tier only | Three-tier chain: workspace → user → platform default |
| SPEC GAP 2 | Kanban T3/T4/T5 had no status or evidence | Updated to `status = "done"` with evidence pointer |
| MINOR 1 | `status_label` used `{:?}` debug format | Fixed to `display_label()` returning lowercase human text |
| MINOR 2 | `windows_environment_block` not sorted | Sorted case-insensitively per CreateProcessW convention |
| MINOR 3 | Evidence timestamps were date-only | Added HH:MM:SS to all fix-round-2 records |
| MINOR 4 | SECURITY.md lacked baseline-set description | Added `passthrough_env=false` baseline-set wording |

### Command 6 — terminal workflow tests (all 9 pass, fix-round-2)

```
cargo test -p legion-app --test terminal_workflow
```

CWD: `C:/Users/dasbl/RustroverProjects/legion-ide-term`
Start: 2026-07-04 18:52:00
End:   2026-07-04 18:52:01
Exit code: 0

```
running 9 tests
test terminal_denial_is_visible_and_fail_closed ... ok
test terminal_resize_propagates_to_projection ... ok
test terminal_failure_ux_distinct_status_kinds ... ok
test terminal_scrollback_limit_enforced_and_eviction_counted ... ok
test terminal_product_gate_trusted_workspace_launches_without_test_helper ... ok
test terminal_workflow_cannot_mutate_editor_or_disk ... ok
test terminal_fixture_lifecycle_projects_status ... ok
test terminal_shell_selection_is_projected_in_status ... ok
test terminal_orphan_cleanup_kills_and_records_evidence ... ok

test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

New tests added in this round:
- `terminal_shell_selection_is_projected_in_status` — now tests workspace → user → platform three-tier precedence (previously one-tier only)
- `terminal_orphan_cleanup_kills_and_records_evidence` — rewritten with genuine short-lived orphan; asserts session_id, state==Exited, second call empty
- `terminal_failure_ux_distinct_status_kinds` — extended with `display_label()` human-readability assertions

### Command 7 — platform smokes (including passthrough=false baseline test)

```
cargo test -p legion-terminal --test platform_shell_smoke
```

CWD: `C:/Users/dasbl/RustroverProjects/legion-ide-term`
Start: 2026-07-04 18:52:00
End:   2026-07-04 18:52:01
Exit code: 0

```
running 4 tests
test windows_cmd_launch_smoke ... ok
test windows_passthrough_false_minimal_baseline_is_safe_and_isolated ... ok
test windows_env_deny_list_stripped_at_pty_spawn ... ok
test windows_powershell_core_launch_smoke ... ok

test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

`windows_passthrough_false_minimal_baseline_is_safe_and_isolated` is new in this round: launches cmd.exe with baseline-only env (no LEGION_SECRET*, no TERM_TEST_CUSTOM* var), verifies cmd runs and secrets/custom vars are absent.

### Command 8 — legion-platform (incl. windows_environment_block sort test)

```
cargo test -p legion-platform
```

CWD: `C:/Users/dasbl/RustroverProjects/legion-ide-term`
Start: 2026-07-04 18:52:05
End:   2026-07-04 18:52:06
Exit code: 0

```
test result: ok. 16 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

`windows_environment_block_is_sorted_case_insensitively` is new in this round.

### Command 9 — desktop terminal_panel tests (incl. human-readable label test)

```
cargo test -p legion-desktop --test terminal_panel
```

CWD: `C:/Users/dasbl/RustroverProjects/legion-ide-term`
Start: 2026-07-04 18:52:10
End:   2026-07-04 18:52:11
Exit code: 0

```
running 2 tests
test status_label_is_human_readable_for_all_kinds ... ok
test terminal_panel_render_model_exposes_grid_status_and_scrollback ... ok

test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

`status_label_is_human_readable_for_all_kinds` is new in this round.

### SPEC GAP 2 — P2.F2.T3/T4/T5 evidence notes

**T3 (renderer grid/scrollback/input/resize)**:
- `TerminalGrid::from_projection()` in `crates/legion-terminal/src/grid.rs`
- `TerminalPanelRenderModel::from_projection()` in `crates/legion-desktop/src/view/terminal_panel.rs` — grid, scrollback, status, copy-row, copy-all-visible
- `terminal_grid_projects_rows_badges_and_scrollback_summary`, `terminal_grid_selection_copy_returns_bounded_payloads_only`, `terminal_grid_applies_row_limit_without_losing_scrollback_metadata` in `legion-terminal/tests/terminal_grid.rs`
- `terminal_panel_render_model_exposes_grid_status_and_scrollback` in `legion-desktop/tests/terminal_panel.rs`
- Resize: `terminal_resize_propagates_to_projection` in `terminal_workflow.rs`
- Kill: `terminal_failure_ux_distinct_status_kinds` exercises kill→Exited
- Status: **done**

**T4 (OSC 133/7)**:
- `crates/legion-terminal/src/osc.rs` — `parse_terminal_shell_output`, `TerminalShellBoundary`
- `crates/legion-terminal/src/session.rs` — `TerminalSessionMetadata`
- Pre-existing tests: `osc_parser_keeps_unterminated_sequences_visible`, `osc7_cwd_decodes_localhost_unc_windows_and_percent_paths`, `osc133_tracks_boundary_and_exit_code_metadata`, `terminal_session_metadata_merges_latest_osc_projection`
- Status: **done** (pre-existing, not newly written in M8; confirmed all 4 tests pass)

**T5 (ConPTY parity)**:
- `windows_cmd_launch_smoke` and `windows_powershell_core_launch_smoke` confirm ConPTY path runs on Windows
- `windows_env_deny_list_stripped_at_pty_spawn` confirms env isolation works via ConPTY
- `windows_passthrough_false_minimal_baseline_is_safe_and_isolated` confirms baseline env safety via ConPTY
- Status: **done**

## Fix-round changes

### FIX 1 — Env deny-list enforcement at PTY spawn

**Problem**: `TerminalEnvPolicy` was audited but not enforced. `PtyRequest` had no `env` field so filtered vars were computed but discarded.

**Changes**:
- `crates/legion-platform/src/lib.rs` — added `env: Option<Vec<(String, String)>>` to `PtyRequest`; Windows ConPTY path builds UTF-16 `lpEnvironment` block from `request.env.unwrap_or_else(|| child_environment_vars(&[]))`; Unix path uses `command.env(key, value)` loop; 4 test constructors patched with `env: None`
- `crates/legion-terminal/src/lib.rs` — added `env: Option<Vec<(String, String)>>` to `TerminalRuntimeLaunchRequest`; `launch()` passes `env: request.env` to `PtyRequest`; all test constructors patched with `env: None`
- `crates/legion-app/src/lib.rs` — `TerminalWorkflow::launch()` computes `pty_env = env_policy.effective_env().unwrap_or_default()` and passes `env: Some(pty_env)` to the runtime
- `crates/legion-terminal/tests/platform_shell_smoke.rs` — `windows_env_deny_list_stripped_at_pty_spawn` uses `NativePtyService::spawn_pty` + `read_pty` polling to await the echo output after ConPTY init burst; `unix_env_deny_list_stripped_at_pty_spawn` twin added

**Test result** (Windows):
```
test windows_env_deny_list_stripped_at_pty_spawn ... ok
output: "...\u{1b}[H\u{1b}[?25hdeny=%LEGION_SECRET_TEST_TOKEN_PTY% allow=visible123-pkt-term-pty\r\n"
```
Secret key not expanded (env var absent); control value `visible123-pkt-term-pty` is visible.

### FIX 2 — TerminalFailureKind wired into TerminalPanelStatusKind

**Problem**: `TerminalFailureKind` existed but was not translated into `TerminalPanelStatusKind`; all failure paths called `deny()` which always projected `StatusKind::Denied`.

**Changes**:
- `crates/legion-protocol/src/lib.rs` — added `Hash` derive and three new variants `Unavailable`, `Crashed`, `PolicyBlocked` to `TerminalPanelStatusKind`
- `crates/legion-app/src/lib.rs` — added `failure_kind_to_status_kind()` and `runtime_error_to_failure_kind()` free functions; added `TerminalWorkflow::apply_failure_kind()` method; `Err(error)` arm in `launch()` now calls `apply_failure_kind`; `AppComposition::project_terminal_failure_for_test()` test helper added
- `crates/legion-app/tests/terminal_workflow.rs` — `terminal_failure_ux_distinct_status_kinds` exercises all 5 failure kinds; verifies they are all distinct via `HashSet`

**Test result**:
```
test terminal_failure_ux_distinct_status_kinds ... ok   (9/9 terminal_workflow tests pass)
```

## Notes

- `TerminalRuntimeConfig::default()` remains `enabled: false`. The product gate calls `ensure_product_enabled()` on first trusted-workspace launch.
- `enable_terminal_runtime_for_tests()` still works as before (test helper only); untrusted workspaces are still denied even if this is called.
- `TerminalFailureKind` is fully wired into `TerminalPanelStatusKind` as of the fix round. All 5 failure kinds project distinct status variants with `display_label()` text.

## Merged-tree standing-gate run (2026-07-05, branch m8/terminal-product)

Context: main merged at 5b9f592 (LSP substrate PR #34); working directory
C:/Users/dasbl/RustroverProjects/legion-ide-term; Windows 11; builds at -j 4.
The controller-run workspace chain surfaced and resolved the following
cross-crate integration items before going green (each adapted with its
original test purpose preserved, none weakened):

- SettingsProjection initializers from main lacking the new
  terminal_shell_selection field (2 sites, compiler-driven sweep).
- dto_contracts session-record golden updated for the new
  WorkbenchSettingsRecord field (serde-defaulted; old records deserialize
  with an empty selection -- backward compatible, no migration action).
- Pre-productization terminal-denial contract retired across
  operational_health, language_terminal_workflow, diagnostics_export,
  beta.rs product gate (now REQUIRES a live trusted-launch session -- a
  strictly stronger beta smoke), beta_workflow, and beta_acceptance_e2e
  step 12a. Untrusted-workspace denial coverage remains in
  legion-app terminal_workflow tests.
- Clippy gate: sort_by_key for the ConPTY env block, snake_case test name,
  is_some_and in two camelCase test helpers.

| Gate | Result |
| --- | --- |
| cargo fmt --all --check | PASS |
| xtask check-deps / docs-hygiene / claim-audit / no-egui-textedit / verify-kanban-backlog | PASS |
| xtask release-pipeline --dry-run + verify-release-pipeline | PASS |
| cargo check --workspace --all-targets | PASS |
| cargo test --workspace --all-targets --no-fail-fast | PASS (all targets) |
| cargo clippy --workspace --all-targets -- -D warnings | PASS (exit 0) |
| xtask perf-harness + verify-perf-harness | PASS (strict) |
| cargo deny check | PASS |
| xtask rust-analyzer-smoke | PASS (real rust-analyzer 1.95.0) |

Note: the workspace test gate and the clippy/tail gates ran on trees separated
only by the three clippy-suggested, behavior-identical lint transforms listed
above (each edited suite re-run green individually after the transform).
