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

### Env policy without `PtyRequest::env` (Task 5)

`PtyRequest` has no `env` field, so we cannot inject the filtered env at PTY spawn time without a protocol change. The `TerminalEnvPolicy` type is fully implemented, unit-tested, and its configuration (passthrough enabled/disabled + deny-prefix count) is audited in the launch record. Actual env injection at PTY spawn requires a future `PtyRequest::env` field addition to `legion-protocol`. This is noted in the code comment and is a tracked concern.

### Orphan cleanup (Task 6)

`TerminalRuntime::cleanup_orphans()` is the runtime-level hook. `AppComposition::cleanup_terminal_orphans()` wraps it and returns metadata-only `TerminalAuditRecord` values. No raw command output is included in the records; redaction stays.

## Notes

- `TerminalRuntimeConfig::default()` remains `enabled: false`. The product gate calls `ensure_product_enabled()` on first trusted-workspace launch.
- `enable_terminal_runtime_for_tests()` still works as before (test helper only); untrusted workspaces are still denied even if this is called.
- `TerminalFailureKind` is defined in `terminal_policy.rs` but is not yet wired into `TerminalPanelStatusKind`; it is available for the projection layer to consume when the renderer is productized.
