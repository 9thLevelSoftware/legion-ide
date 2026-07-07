# PKT-WORKTREE Evidence

**Branch:** `m10/worktree-scope`
**Status:** DONE

## D1 — Isolation mode reporting (GitWorktree vs DirectoryCopy)

`DelegatedTaskSandboxOrchestrator` gains a `SandboxIsolationMode` field (init: `NotInitialized`). After `initialize()` completes, `isolation_mode()` returns `GitWorktree` when `git worktree add` succeeded, `DirectoryCopy` when it fell back. On publish failure the mode resets to `NotInitialized`.

A `lease_acquired()` accessor returns `self.lease.is_some()` so callers can detect unprotected sandboxes.

Tests added:
- `isolation_mode_starts_as_not_initialized`
- `isolation_mode_is_directory_copy_when_git_not_available` — creates a temp dir with no git repo, calls `initialize()`, asserts `DirectoryCopy` and that source files are present in the sandbox.

## D2 — Workspace-root-derived sandbox paths

`with_workspace_root` was updated: sandbox path is now `source_root.join("target/delegated-tasks/task-{run_id}")`, not CWD-relative. The production caller (`execute_delegated_task` in `legion-app/src/lib.rs`) was changed from `DelegatedTaskSandboxOrchestrator::new(&plan_id.0)` to `with_workspace_root(&workspace_root, &plan_id.0)` where `workspace_root` comes from a new `workspace_root_path()` helper (falls back to CWD when no workspace is open).

`copy_workspace_tree` was also fixed: it now threads the root sandbox path through recursive calls to avoid circular copies when the sandbox is nested inside the workspace tree (the new canonical layout puts it at `workspace_root/target/delegated-tasks/task-<id>`).

Integration test helpers updated: `sandbox_path()` replaced with `sandbox_path_in(workspace_root, plan_id)` and `sandbox_path_cwd(plan_id)` to reference the correct location based on whether a workspace was opened.

## D3 — Main-workspace protection

`validate_not_main_workspace(sandbox_path, workspace_root) -> Result<(), AgentError>` is a public function that:
- Canonicalizes both paths (strips Windows `\\?\` UNC prefix for comparison)
- Rejects if `sandbox_path == workspace_root`
- Rejects if `sandbox_path` is an ancestor of `workspace_root` (workspace inside sandbox)

`initialize()` calls it before any directory creation when `source_root` is set.

Tests added:
- `validate_not_main_workspace_rejects_identical_paths`
- `validate_not_main_workspace_rejects_when_sandbox_is_parent_of_workspace`
- `validate_not_main_workspace_accepts_sibling_directory`
- `initialize_fails_when_sandbox_path_equals_workspace_root` — crafts `sandbox_path == workspace_root` via `with_sandbox_root` by naming the workspace dir `task-<run_id>` with `sandbox_root` as its parent.

## D4 — Sandbox panel productization with honest labels

`SandboxPanelState` enum added:
- `NoSandbox` — activation is `NotEncoded` or `Planned`
- `Active { isolation_mode_label, backend_label, strength_label, caveats, lease_held }` — all other activations

`rows(snapshot, state)` now accepts `SandboxPanelState`. `NoSandbox` emits a single "no sandbox/worktree allocated yet" row. `Active` emits backend, isolation, lease, and caveat rows.

`sandbox_strength_label` updated with honest post-PKT-SANDBOX labels:
- `Seatbelt` → `"os-enforced"`
- `BubblewrapLandlock` → `"os-enforced"`
- `RestrictedToken` → `"process-isolated"`
- `AppContainer` → `"os-enforced"`
- `DocumentedFallback` → `"fallback"`

`view.rs` call site updated: derives `SandboxPanelState::from_snapshot(snapshot)` then passes it to `rows`.

Tests updated and extended in both the module-internal test block and `crates/legion-desktop/tests/sandbox_panel.rs`:
- Existing `sandbox_panel_surfaces_active_backend_and_caveats` still passes
- New: `sandbox_panel_shows_honest_strength_label_not_descriptor_only`
- New: `sandbox_panel_no_sandbox_state_shows_not_allocated`
- New: `sandbox_panel_active_state_shows_isolation_and_lease`

## Test results

```
cargo fmt --check        → PASS (clean)
cargo clippy --all-targets -- -D warnings → PASS (no warnings)
cargo test --all-targets -j 4 → PASS (all tests green)
cargo test -p legion-app --test manual_zero_egress → PASS
```

## Concerns

None. All deliverables implemented, all tests green.
