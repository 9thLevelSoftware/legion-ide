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
Start: 2026-07-04  
End: 2026-07-04  
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
Start: 2026-07-04  
End: 2026-07-04  
Exit code: 0

All tests passed (unit tests in terminal_policy module + all integration tests).

### Command 3 — platform shell smoke tests (Windows cmd + pwsh)

```
cargo test -p legion-terminal --test platform_shell_smoke
```

CWD: `C:/Users/dasbl/RustroverProjects/legion-ide-term`  
Start: 2026-07-04  
End: 2026-07-04  
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
Start: 2026-07-04  
End: 2026-07-04  
Exit code: 0

All tests passed (including taxonomy classification tests for `pwsh` and `zsh`).

### Command 5 — legion-platform PTY tests

```
cargo test -p legion-platform
```

CWD: `C:/Users/dasbl/RustroverProjects/legion-ide-term`  
Start: 2026-07-04  
End: 2026-07-04  
Exit code: 0

All tests passed.

## Design decisions

### Product gate placement (Task 1)

The trust check lives at the top of `TerminalWorkflow::launch()`, before any capability broker evaluation. This means `enable_runtime_for_tests()` cannot override it for untrusted workspaces: the deny is unconditional and happens before the security broker is consulted.

### Env policy enforcement at PTY spawn (Task 5 — FIX ROUND)

Initial implementation only audited the env deny-list; it did not enforce it because `PtyRequest` lacked an `env` field. The fix round added `env: Option<Vec<(String, String)>>` to `PtyRequest` in `legion-platform`, plumbed it through `TerminalRuntimeLaunchRequest` in `legion-terminal`, wired both Windows ConPTY (`lpEnvironment` block) and Unix exec paths, and updated `TerminalWorkflow::launch()` to pass `env_policy.effective_env()`. A TDD smoke test (`windows_env_deny_list_stripped_at_pty_spawn`) verifies the secret var is absent from PTY output while the control var is present.

### Orphan cleanup (Task 6)

`TerminalRuntime::cleanup_orphans()` is the runtime-level hook. `AppComposition::cleanup_terminal_orphans()` wraps it and returns metadata-only `TerminalAuditRecord` values. No raw command output is included in the records; redaction stays.

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
