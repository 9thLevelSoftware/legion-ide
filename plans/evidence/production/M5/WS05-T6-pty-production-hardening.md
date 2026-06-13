# M5 — WS05.T6 PTY Production Hardening Evidence

## Status

Accepted.

## Acceptance target

- Orphan reaping and crash cleanup leave no lingering PTY sessions.
- Secret-bearing environment variables are stripped before child process launch.
- Windows ConPTY branch uses the same sanitized environment launch path.
- Evidence package records the hardening checks and the verification commands.

## What landed

- `crates/legion-platform/src/lib.rs`
  - Added secret-aware environment sanitization helpers for child launches.
  - `NativeProcessService::execute` now clears the inherited environment, then repopulates only sanitized variables.
  - Unix PTY launch now clears inherited env before repopulating sanitized variables.
  - Windows ConPTY launch now builds an explicit sanitized environment block instead of inheriting the full parent environment.
  - Added coverage for secret filtering in normalized environment maps and process execution.
- `crates/legion-terminal/src/lib.rs`
  - Existing lifecycle coverage already exercises session removal on close/kill, orphan cleanup, and exit-drain behavior; no extra edits were required for this card.

## Verification

- `cargo test -p legion-platform --lib -- --nocapture` ✅
  - 13 tests passed, including:
    - `environment_service_strips_secret_like_vars_from_normalized_map`
    - `process_execution_strips_secret_like_env_vars`
- `cargo test -p legion-terminal --lib -- --nocapture` ✅
  - 13 tests passed, including:
    - `terminal_runtime_kill_and_orphan_cleanup_remove_sessions`
    - `terminal_runtime_poll_output_removes_exited_native_session`
    - `terminal_runtime_keeps_exited_native_session_until_truncated_output_is_drained`
- `cargo clippy -p legion-platform -p legion-terminal --all-targets -- -D warnings` ✅
- `cargo fmt --all --check` ⚠️
  - Fails on pre-existing formatting drift in unrelated files elsewhere in the workspace; not introduced by this task.

## Findings

- Secret-like environment keys are now removed from both metadata-normalized and child-launch environment paths.
- PTY/process launch paths no longer inherit the full parent environment wholesale.
- The current workspace already had strong lifecycle cleanup tests; this card extended the hardening surface with env hygiene and retained the orphan-reaping coverage.
- Windows ConPTY parity was audited in-source: the cfg(windows) branch now uses the same sanitized env block construction as the Unix/process paths.
