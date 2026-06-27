# Review: Tests - App & Agent

Scope: legion-app and legion-agent integration tests listed in kanban task t_924140b9.

Summary:
- Findings: 19
- Critical: 0
- High: 0
- Medium: 10
- Low: 9

## crates/legion-app/tests/apply_activation.rs

### Finding 1
- Category: stub
- Severity: low
- Line numbers: 30-44
- Description: `open_workspace` is kept behind `#[allow(dead_code)]` and is not used by either test. This is leftover helper code in a small smoke-test file and can hide stale setup behavior because it creates an extra temp root contract nobody exercises.
- Suggested fix direction: Remove the unused helper or rewrite the tests to use it consistently so workspace setup lives in one exercised path.

## crates/legion-app/tests/assist_inline_prediction_workflow.rs

No findings.

## crates/legion-app/tests/checkpoint_restore.rs

No findings.

## crates/legion-app/tests/control_trust_surfaces.rs

No findings.

## crates/legion-app/tests/daily_editing_contracts.rs

### Finding 2
- Category: bug
- Severity: medium
- Line numbers: 264-274, 286-296
- Description: The assertions intended to prove fallback comment detection ignores delimiters inside strings can pass even when a bogus comment token starts immediately after the delimiter. For example, a token starting at the second slash in `https://` has `start_col > rust_slashes`, satisfying the predicate even though it still marks string content as a comment.
- Suggested fix direction: Assert that no comment token overlaps the full delimiter span inside the string, e.g. reject any comment token where `token.start_col < delimiter_end && token.end_col > delimiter_start`, and include the two-character `//` span for Rust.

### Finding 3
- Category: failure-point
- Severity: low
- Line numbers: 17-27, 116, 173, 241, 301, 350, 401, 467, 481, 525, 559, 598
- Description: The file manually removes temp roots at the end of each test instead of using a `Drop` guard. Any panic before the cleanup statement leaves temp workspaces behind, which is likely in this assertion-heavy file.
- Suggested fix direction: Introduce a `TempWorkspace` guard with a scoped prefix check and `Drop` cleanup, matching the safer pattern used in `daily_editing_search.rs` and `palette.rs`.

## crates/legion-app/tests/daily_editing_search.rs

### Finding 4
- Category: failure-point
- Severity: medium
- Line numbers: 231-238
- Description: `daily_editing_search_workspace_honors_gitignore` only checks that `git init` could be spawned. It does not assert that the exit status succeeded. If `git init -q` returns a non-zero status because of environment/configuration issues, the test continues in a non-repository workspace and the `.gitignore` assertion can fail for the wrong reason or mask setup problems.
- Suggested fix direction: Capture the `ExitStatus`, assert `status.success()`, and include stderr/stdout in the failure message. If git is optional for the test environment, skip with an explicit precondition instead of continuing.

## crates/legion-app/tests/debug_workflow.rs

No findings.

## crates/legion-app/tests/delegated_task_integration.rs

### Finding 5
- Category: failure-point
- Severity: medium
- Line numbers: 373-379
- Description: The ACP host integration test hard-codes `/bin/sh`. That path is Unix-specific and makes the test fail on Windows or sandboxed environments where `/bin/sh` is unavailable, despite the surrounding Rust crates otherwise being cross-platform.
- Suggested fix direction: Gate this test with a Unix cfg, or resolve a shell through a test helper that uses `cmd /C` on Windows and skips with a clear message when no shell is available.

### Finding 6
- Category: failure-point
- Severity: low
- Line numbers: 63-69, 204, 298, 365, 459, 454
- Description: Several delegated-task tests create temp workspaces with `temp_workspace` but only the ACP-host path explicitly removes its workspace. Other test paths rely on OS temp cleanup and leak directories on success as well as on panic.
- Suggested fix direction: Return a temp-workspace guard with `Drop`, or use `tempfile::TempDir`, so all paths are cleaned consistently.

## crates/legion-app/tests/git_workflow.rs

### Finding 7
- Category: failure-point
- Severity: medium
- Line numbers: 106-118
- Description: All git workflow tests require the external `git` binary and panic if it is missing or if repository setup fails. There is no precondition check, skip path, or feature gate, so environments without git report these as product regressions instead of test-environment failures.
- Suggested fix direction: Add a shared helper that verifies `git --version` once and either skips/marks the suite ignored with a clear diagnostic or fails with an explicit setup error before creating repositories.

### Finding 8
- Category: bug
- Severity: medium
- Line numbers: 530-534, 634-638, 784-788, 896-900, 1020-1024
- Description: The conflict tests intentionally run `git merge feature` to create a conflicted state, but they ignore the merge exit status and stdout/stderr. If the merge unexpectedly succeeds, fails before writing conflicts, or changes git behavior, the test proceeds and later assertions fail with misleading application-level messages.
- Suggested fix direction: Assert that the merge command exits non-zero for the expected reason and that the target file/status is unmerged before opening the app. Include merge stdout/stderr in the assertion message.

## crates/legion-app/tests/language_terminal_integration.rs

### Finding 9
- Category: failure-point
- Severity: medium
- Line numbers: 219-237
- Description: `terminal_actions_cannot_mutate_editor_or_disk` sends terminal input, polls once, immediately searches, and then asserts `output_rows` is not empty. Terminal output is asynchronous; a slow CI host can legitimately have no visible rows after one poll, making this test flaky.
- Suggested fix direction: Use a bounded polling helper that waits until the expected output/search row appears or the terminal exits, and emit the final projection on timeout.

## crates/legion-app/tests/language_tooling_workflow.rs

### Finding 10
- Category: failure-point
- Severity: medium
- Line numbers: 121-124, 248-249
- Description: The cross-file rename traversal test writes `traversal_target` outside the workspace root and only removes it at the end of the happy path. Any panic before line 248 leaves an off-workspace file in the temp parent, which is exactly the area this test is trying to prove cannot be mutated by the app.
- Suggested fix direction: Manage both the workspace root and traversal target with a cleanup guard so off-workspace fixtures are removed on panic. Prefer constructing the outside path under a dedicated guarded parent directory.

## crates/legion-app/tests/legion_workflow_integration.rs

### Finding 11
- Category: failure-point
- Severity: low
- Line numbers: 265-274, 768-800
- Description: `temp_workspace` creates directories under `current_dir()/target/legion-workflow-integration` and only some tests clean them up manually. Panics or additional callers can leave state under the crate target directory, and the helper is tied to the process current directory.
- Suggested fix direction: Use a guard or `tempfile::TempDir` rooted in the OS temp directory, or make the target-directory fixture return a guard that always removes the specific generated workspace.

## crates/legion-app/tests/palette.rs

### Finding 12
- Category: failure-point
- Severity: medium
- Line numbers: 374-453, 523-527
- Description: The command catalog coverage check uses a manually maintained case list and a hard-coded denominator of `13`. If the registered command catalog grows, the test can still report 100% coverage because it never compares against the real catalog size.
- Suggested fix direction: Derive the expected denominator from the registered command catalog or assert that every registered command has a corresponding case. At minimum, use `cases.len()` instead of a literal and add a catalog-vs-cases diff to the failure output.

## crates/legion-app/tests/plugin_grammar.rs

### Finding 13
- Category: failure-point
- Severity: low
- Line numbers: 37-42
- Description: The plugin manifest advertises a grammar artifact at `file:///tmp/rust-plugin-grammar.wasm`, but the test never creates or validates that artifact. The app can therefore pass this test by registering grammar metadata without proving artifact existence or loadability.
- Suggested fix direction: Either create a real fixture artifact and assert the loader validates it, or rename/scope the test to make clear it is only checking metadata registration and add a separate artifact validation test.

## crates/legion-app/tests/settings.rs

No findings.

## crates/legion-app/tests/structural_search_workflow.rs

No findings.

## crates/legion-app/tests/terminal_workflow.rs

### Finding 14
- Category: failure-point
- Severity: medium
- Line numbers: 108-124, 143-155, 231-247
- Description: The terminal workflow tests use fixed `20 * 25ms` sleep/poll loops. Real terminal startup, command execution, or CI load can exceed 500ms, creating flakes unrelated to the terminal contract.
- Suggested fix direction: Replace fixed sleeps with a reusable `poll_until` helper that uses a larger deadline, checks the exact desired condition, and reports the last terminal projection on timeout.

### Finding 15
- Category: stub
- Severity: low
- Line numbers: 258-264
- Description: A diagnostic `eprintln!` remains in `terminal_fixture_lifecycle_projects_status`. This creates noisy test output and can leak internal projection details in CI logs.
- Suggested fix direction: Remove the debug print or only emit it inside the assertion failure message.

### Finding 16
- Category: failure-point
- Severity: low
- Line numbers: 9-16, 70-71, 163-164, 319-320, 382, 431
- Description: This file uses manual `remove_dir_all` cleanup rather than a temp guard. Panics in the terminal tests are relatively likely while debugging async behavior, so temp roots can accumulate.
- Suggested fix direction: Wrap the root in a guard with `Drop`, and keep the prefix/process-id safety check before deletion.

## crates/legion-app/tests/workspace_vfs_integration.rs

### Finding 17
- Category: failure-point
- Severity: medium
- Line numbers: 2391-2394, 3044-3048
- Description: Off-workspace escape fixture names are derived from `TEMP_ROOT_COUNTER.load(Ordering::Relaxed)` without a process id, timestamp, or increment. Parallel test processes, reruns after panic, or two tests observing the same counter value can collide in the shared temp parent and delete or assert against another test's file.
- Suggested fix direction: Reuse the unique root suffix, include process id plus an increment/fresh UUID in off-workspace file names, and manage the outside fixture with a cleanup guard.

## crates/legion-agent/tests/comm.rs

No findings.

## crates/legion-agent/tests/dag.rs

No findings.

## crates/legion-agent/tests/merge_readiness.rs

No findings.

## crates/legion-agent/tests/plan_artifact.rs

No findings.

## crates/legion-agent/tests/scheduler.rs

No findings.

## crates/legion-agent/tests/scope_enforcement.rs

No findings.

## crates/legion-agent/tests/tools_schema.rs

### Finding 18
- Category: failure-point
- Severity: low
- Line numbers: 8-20, 32-43
- Description: The schema tests hard-code the exact registry length, order, and required field list. That catches accidental changes, but it also makes intentional tool additions fail without showing which catalog entry lacks schema validation or case coverage.
- Suggested fix direction: Keep an explicit expected set if stability is desired, but compare as sets and emit added/removed tool kinds. Consider deriving the required-field checks from a table keyed by tool kind and separately asserting that all registry entries are covered.

## crates/legion-agent/tests/worktree_sandbox.rs

### Finding 19
- Category: failure-point
- Severity: low
- Line numbers: 12-16, 40-46, 64-71
- Description: The sandbox test manually removes the source temp root only after all assertions pass. A failure before line 46 leaves both the source fixture and possibly the sandbox directory behind.
- Suggested fix direction: Use a temp directory guard for `source_root` and ensure `orchestrator.cleanup` is attempted from a guard or finally-style scope even when assertions fail.
