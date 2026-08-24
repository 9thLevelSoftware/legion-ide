# PR-LANG-002 Terminal Trusted-PTY Workflow Evidence

| Field | Value |
| --- | --- |
| Packet ID | `PR-LANG-002-TERMINAL-PTY-LOCAL-001` |
| Scope | Local product-workflow evidence for trusted PTY launch, input/output, kill, and bounded scrollback |
| Readiness effect | None; `PR-LANG-002` remains **Substrate validated** until the full acceptance bar is met |
| Trust boundary | Trusted Manual workspace launches; untrusted workspace is denied before session creation |
| Runtime | `TerminalRuntime<NativePtyService>` with workspace trust and `terminal.launch` policy |

## Claim boundary

This packet names executable evidence already present in the repository. It distinguishes real native-PTY checks from deterministic fixture checks used to make the app and projection contract repeatable. It does **not** claim a completed windowed capture, hosted 3-OS dogfood, screen-reader evidence, or readiness-ledger promotion.

## Evidence map

| Workflow leg | Evidence | What it proves |
| --- | --- | --- |
| Trusted launch | `crates/legion-app/tests/terminal_workflow.rs::terminal_product_gate_trusted_workspace_launches_without_test_helper` | An explicit launch in a trusted Manual workspace auto-enables the runtime, returns `Running`, and creates an active session without a test-only enable helper. |
| Untrusted denial | `crates/legion-app/tests/terminal_workflow.rs::terminal_product_gate_trusted_workspace_launches_without_test_helper`; `terminal_denial_is_visible_and_fail_closed` | An untrusted workspace returns `Denied`, surfaces an `untrusted` reason, and does not create a session; the denial remains fail-closed even when a test override is attempted. |
| Native PTY launch | `crates/legion-desktop/tests/terminal_reachability.rs::a_terminal_launch_from_the_ui_reaches_a_real_session`; `crates/legion-terminal/tests/platform_shell_smoke.rs::windows_cmd_launch_smoke` / `unix_bash_launch_smoke` | The desktop launch path reaches a real session, while platform smoke covers the native shell backend where the target shell is available. |
| Input and output | `crates/legion-desktop/tests/terminal_reachability.rs::a_launched_command_actually_runs_in_the_terminal`; `crates/legion-app/tests/terminal_workflow.rs::terminal_fixture_lifecycle_projects_status` | Input is dispatched after launch and command output becomes visible in the projection. The app test uses the deterministic fixture; the desktop test exercises the real PTY path. |
| Kill and cleanup | `crates/legion-app/tests/terminal_workflow.rs::terminal_orphan_cleanup_kills_and_records_evidence`; `crates/legion-terminal/src/lib.rs::terminal_runtime_kill_and_orphan_cleanup_remove_sessions` | Exited/orphaned sessions are reaped, produce an audit record, and are removed; kill leaves a non-running state rather than a zombie `Running` projection. |
| Bounded scrollback | `crates/legion-app/tests/terminal_workflow.rs::terminal_scrollback_limit_enforced_and_eviction_counted`; `crates/legion-terminal/tests/terminal_grid.rs::terminal_grid_projects_rows_badges_and_scrollback_summary` | Visible rows stay within the configured limit, omitted rows are counted, and the projection/grid exposes truncation metadata. |

## Repeatable local run

Run the app-level workflow packet:

```text
cargo test -p legion-app --test terminal_workflow
```

Run the native desktop reachability packet:

```text
cargo test -p legion-desktop --test terminal_reachability
```

Run the terminal runtime and grid contracts:

```text
cargo test -p legion-terminal --lib --tests
```

The platform shell smoke test is target-gated. On Windows, `cmd.exe` is required and `pwsh` is skipped when it is not on `PATH`; on Unix, `bash` is required and `zsh` is skipped when unavailable. A skipped optional shell is not evidence of failure, and a passing fixture test is not evidence of a native shell capture.

## Recorded validation

Local Windows run on **August 24, 2026**:

- `cargo test -p legion-app --test terminal_workflow` — **9 passed**.
- `cargo test -p legion-desktop --test terminal_reachability` — **5 passed**.
- `cargo test -p legion-terminal --lib --tests` — **128 passed** across the runtime unit tests, ConPTY contract tests, platform shell smoke, OSC tracking, and terminal grid suites. `windows_cmd_launch_smoke`, `windows_powershell_core_launch_smoke`, and both Windows environment-isolation checks passed in this run.

These are local automated results from the current worktree. They are not a hosted multi-OS run and do not change the readiness status.

## Manual capture checklist

When a renderer-backed local transcript is available, attach it to this packet and record the exact OS, build revision, trust state, shell, and command. The minimum sequence is:

1. Open a workspace as **Trusted** in Manual mode.
2. Launch the terminal and record the transition to `Running` plus the session identifier.
3. Send a harmless command (for example `ver` on Windows or `uname` on Unix) and record the output row visible in the terminal projection.
4. Send terminal input, then kill the session; record `Exited` or the documented fail-closed `Failed` result and the audit/session outcome.
5. Generate more output than the configured scrollback limit and record `visible_row_count`, `omitted_row_count`, and `truncated`.
6. Repeat launch from an **Untrusted** workspace and record the visible denial and absence of an active session.

Do not attach raw secrets or full workspace transcripts. The existing PTY environment deny-list applies to the child launch path, and packet attachments should remain metadata-only where possible.
