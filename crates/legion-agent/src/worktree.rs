//! Delegated-task sandbox orchestration, containment validation, and proposal generation.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;

use legion_platform::resolve_existing_prefix;
use legion_protocol::{
    AssistedAiContractError, AssistedAiEditProposalOutput, AssistedAiTrustProjectionReference,
    CanonicalPath, CapabilityId, CausalityId, CorrelationId, DelegatedTaskToolPermissionProfile,
    DelegatedTaskToolPermissionRequest, FileFingerprint, PermissionBudgetActionClass,
    PreviewSummary, PrincipalId, ProposalId, ProposalPayload, ProposalVersionPreconditions,
    RedactionHint, TimestampMillis,
};

use crate::AgentError;

/// How the sandbox was actually initialized.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SandboxIsolationMode {
    /// Full git worktree isolation.
    GitWorktree,
    /// Plain directory copy (git worktree unavailable).
    DirectoryCopy,
    /// Not yet initialized.
    NotInitialized,
}

/// Orchestrator for isolating agent tasks under `target/delegated-tasks/task-{run_id}`.
/// Uses git worktrees with standard directory fallback if git is unavailable.
#[derive(Debug, Clone)]
pub struct DelegatedTaskSandboxOrchestrator {
    sandbox_path: PathBuf,
    source_root: Option<PathBuf>,
    is_worktree: bool,
    /// How the sandbox was initialized; `NotInitialized` until `initialize()` completes.
    isolation_mode: SandboxIsolationMode,
    /// Exclusive lock handle over the sandbox's `.lock` lease file. `Arc`
    /// keeps this orchestrator `Clone`: every clone shares the same lease,
    /// so the lease outlives any single clone and is only released when the
    /// last clone drops it (or `cleanup` clears it explicitly). Best-effort:
    /// `None` if the lease could not be acquired (see `initialize`).
    lease: Option<Arc<std::fs::File>>,
}

impl DelegatedTaskSandboxOrchestrator {
    /// Creates a new orchestrator.
    pub fn new(run_id: &str) -> Self {
        let sandbox_path = PathBuf::from("target/delegated-tasks").join(format!("task-{}", run_id));
        Self {
            sandbox_path,
            source_root: None,
            is_worktree: false,
            isolation_mode: SandboxIsolationMode::NotInitialized,
            lease: None,
        }
    }

    /// Creates a new orchestrator that isolates a specific workspace root.
    ///
    /// The sandbox directory is placed under `source_root/target/delegated-tasks/`
    /// so the sandbox path is always derived from the workspace root, not CWD.
    pub fn with_workspace_root(source_root: &Path, run_id: &str) -> Self {
        let sandbox_path = source_root
            .join("target/delegated-tasks")
            .join(format!("task-{}", run_id));
        Self {
            sandbox_path,
            source_root: Some(source_root.to_path_buf()),
            is_worktree: false,
            isolation_mode: SandboxIsolationMode::NotInitialized,
            lease: None,
        }
    }

    /// Creates a new orchestrator that places the sandbox under an explicit
    /// root directory instead of the shared `target/delegated-tasks` default.
    ///
    /// Intended for test callers that need hermetic, per-test sandbox
    /// isolation: pass a unique temporary directory as `sandbox_root` so
    /// concurrent test threads cannot see or reap each other's lease files.
    /// The `source_root` argument has the same semantics as in
    /// `with_workspace_root`: `Some(path)` enables the fallback workspace-copy
    /// path when `git worktree add` is unavailable, `None` creates an empty
    /// sandbox directory.
    ///
    /// Product callers should continue to use `new` or `with_workspace_root`
    /// so that the canonical `target/delegated-tasks` root is preserved.
    pub fn with_sandbox_root(
        sandbox_root: &Path,
        run_id: &str,
        source_root: Option<&Path>,
    ) -> Self {
        let sandbox_path = sandbox_root.join(format!("task-{run_id}"));
        Self {
            sandbox_path,
            source_root: source_root.map(|p| p.to_path_buf()),
            is_worktree: false,
            isolation_mode: SandboxIsolationMode::NotInitialized,
            lease: None,
        }
    }

    /// Returns the sandbox path.
    pub fn sandbox_path(&self) -> &Path {
        &self.sandbox_path
    }

    /// Returns how the sandbox was actually initialized.
    /// Returns `NotInitialized` until `initialize()` completes successfully.
    pub fn isolation_mode(&self) -> SandboxIsolationMode {
        self.isolation_mode
    }

    /// Returns whether an exclusive lease was acquired on the sandbox's lock file.
    /// A `false` return means the sandbox is unprotected against concurrent reaper runs.
    pub fn lease_acquired(&self) -> bool {
        self.lease.is_some()
    }

    /// Returns the sibling lease file path for this sandbox
    /// (`task-<run_id>.lock` next to `task-<run_id>/`).
    fn lease_path(&self) -> PathBuf {
        lease_path_for_sandbox(&self.sandbox_path)
    }

    /// Initializes the sandbox using `git worktree add` with fallback to copy-based isolation.
    ///
    /// Ordering is deliberate and load-bearing: the lease is acquired
    /// *before* the sandbox directory is created (published), not after. A
    /// concurrent `reap_orphaned_sandboxes` call only treats a `task-<id>`
    /// dir as protected if its sibling `.lock` is present and held; if the
    /// dir were published first, a reaper scan landing in the gap between
    /// dir creation and lease acquisition would see a lease-less directory
    /// and could delete it mid-initialization. Acquiring the lease first
    /// closes that window: by the time the sandbox dir exists at all, its
    /// protection (if acquired) is already in place.
    ///
    /// Lease-unlink ownership rule applies here too: on publication
    /// failure, the now-orphaned lease file is removed while
    /// `acquired_lease` is still held, then the handle is dropped — never
    /// the reverse. See the ownership-rule note on `reap_orphaned_sandboxes`
    /// for why a lease path must only ever be unlinked while its lock is
    /// held.
    pub fn initialize(
        &mut self,
        permission: &DelegatedTaskToolPermissionRequest,
    ) -> Result<(), std::io::Error> {
        validate_sandbox_permission(permission, "initialize")?;

        // Main-workspace protection: reject any configuration that would point the
        // sandbox at the workspace root itself or any of its ancestor directories.
        // This check must happen BEFORE any directory is created so there is no
        // partial-creation window to clean up on failure.
        if let Some(source_root) = &self.source_root {
            validate_not_main_workspace(&self.sandbox_path, source_root)
                .map_err(|e| std::io::Error::other(e.to_string()))?;
        }

        // Create the delegated-tasks root (the sandbox dir's parent) first —
        // this is shared, pre-existing infrastructure, not the sandbox
        // itself, so publishing it early is not a TOCTOU concern.
        if let Some(parent) = self.sandbox_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        // Acquire a best-effort exclusive lease on the sandbox's sibling
        // `.lock` file BEFORE the sandbox directory is created, so a
        // concurrent reaper can never observe a published sandbox without
        // its protection already in place. Lease acquisition is not a
        // correctness gate: if it fails for any reason (permissions,
        // unsupported filesystem, contention on an extremely unlikely stale
        // lock), the sandbox still initializes successfully without
        // protection — but the directory is only created after this
        // attempt, regardless of whether it succeeded.
        let lease_path = self.lease_path();
        let acquired_lease = match std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(&lease_path)
        {
            Ok(lock_file) if lock_file.try_lock().is_ok() => Some(Arc::new(lock_file)),
            _ => None,
        };

        // Try git worktree first. Pass the path as an OsStr so a non-UTF-8
        // PathBuf cannot panic the process.
        let mut command = Command::new("git");
        if let Some(source_root) = &self.source_root {
            command.arg("-C").arg(source_root);
        }
        let output = command
            .arg("worktree")
            .arg("add")
            .arg(&self.sandbox_path)
            .arg("HEAD")
            .output();

        let publish_result: Result<(), std::io::Error> = match output {
            Ok(output) if output.status.success() => {
                self.is_worktree = true;
                self.isolation_mode = SandboxIsolationMode::GitWorktree;
                Ok(())
            }
            _ => {
                self.is_worktree = false;
                self.isolation_mode = SandboxIsolationMode::DirectoryCopy;
                std::fs::create_dir_all(&self.sandbox_path).and_then(|()| {
                    if let Some(source_root) = &self.source_root {
                        copy_workspace_tree(source_root, &self.sandbox_path)
                    } else {
                        Ok(())
                    }
                })
            }
        };

        match publish_result {
            Ok(()) => {
                self.lease = acquired_lease;
                Ok(())
            }
            Err(error) => {
                // Sandbox publication failed after a lease was acquired:
                // best-effort remove the now-orphaned lease file so it is
                // not mistaken for a live lease by a later reap pass (there
                // is no sandbox dir left to protect). This follows the same
                // hold-through-remove ownership rule as the reaper: a lease
                // path is only ever unlinked while its lock is still held,
                // so `remove_file` runs BEFORE `acquired_lease` is dropped,
                // not after. On Windows, std opens files with
                // FILE_SHARE_DELETE, so `remove_file` on a path we hold an
                // open (locked) handle to succeeds by marking the file
                // delete-on-close; the directory entry disappears once
                // `acquired_lease` is dropped below. On Unix, unlinking a
                // file while `flock`ed is unconditionally fine. Safe here
                // for an additional, simpler reason too — no sandbox
                // directory was ever published, so there is nothing another
                // process could be protecting — but following the same
                // sequencing everywhere keeps the invariant uniform rather
                // than relying on a case-by-case justification.
                self.isolation_mode = SandboxIsolationMode::NotInitialized;
                let _ = std::fs::remove_file(&lease_path);
                drop(acquired_lease);
                Err(error)
            }
        }
    }

    /// Cleans up the sandbox.
    pub fn cleanup(
        &mut self,
        permission: &DelegatedTaskToolPermissionRequest,
    ) -> Result<(), std::io::Error> {
        validate_sandbox_permission(permission, "cleanup")?;
        if self.sandbox_path.exists() {
            if self.is_worktree {
                let output = Command::new("git")
                    .arg("worktree")
                    .arg("remove")
                    .arg("--force")
                    .arg(&self.sandbox_path)
                    .output()?;
                if !output.status.success() {
                    let message = String::from_utf8_lossy(&output.stderr).trim().to_string();
                    return Err(std::io::Error::other(format!(
                        "git worktree remove failed for {}: {}",
                        self.sandbox_path.display(),
                        message
                    )));
                }
            } else {
                std::fs::remove_dir_all(&self.sandbox_path)?;
            }
        }
        // Release the lease, but do NOT unlink the lock file here. Ownership
        // rule: an orchestrator never removes its lease path after releasing
        // the lock; only the reaper does, and only while re-acquiring the
        // lock immediately beforehand (see `remove_stale_lease_files`). If
        // cleanup dropped the lock and then called `remove_file`, a
        // restarted same-run-id lane could acquire the now-unlocked lease
        // in the gap between the drop and the unlink — cleanup's
        // `remove_file` would then delete the NEW owner's lock file out
        // from under it. Leaving the (now-unlocked) lock file in place is
        // safe: it is indistinguishable from any other stale lease and will
        // be cleaned up by the next `reap_orphaned_sandboxes` call, which
        // removes it race-free by holding the lock across the removal.
        self.lease = None;
        Ok(())
    }
}

/// Returns the sibling lease file path for a `task-<run_id>` sandbox
/// directory: `task-<run_id>.lock` next to it.
fn lease_path_for_sandbox(sandbox_path: &Path) -> PathBuf {
    let mut lease_path = sandbox_path.to_path_buf();
    let file_name = sandbox_path
        .file_name()
        .map(|name| {
            let mut name = name.to_os_string();
            name.push(".lock");
            name
        })
        .unwrap_or_else(|| std::ffi::OsString::from("task.lock"));
    lease_path.set_file_name(file_name);
    lease_path
}

/// NOTE: `crates/legion-app/src/offline_ai.rs::reap_orphaned_sandboxes`
/// mirrors this logic for offline builds — apply any change to both.
///
/// Removes orphaned sandbox directories under `delegated_tasks_root`.
///
/// A directory is an orphan when its name starts with `task-` and its
/// run-id suffix is not in `active_run_ids`. Attempts `git worktree
/// remove --force` first (mirroring `initialize`'s worktree-first
/// strategy) and falls back to plain directory removal. Returns the
/// paths that were removed. A missing root is a successful no-op.
///
/// Lock-file lease protocol: each sandbox may have a sibling
/// `task-<run_id>.lock` file (see `DelegatedTaskSandboxOrchestrator`). A
/// sandbox is only reaped if its lease file is absent (legacy sandbox from
/// before this protocol existed, or one whose owner already released the
/// lease) or its lease can be acquired here (owner is gone). If the lease
/// is currently held elsewhere, `try_lock` fails and the reaper treats the
/// owner as alive and skips that sandbox entirely — fail-closed toward NOT
/// deleting. This makes the empty `active_run_ids` list used at startup
/// safe even when another process instance has live sandboxes under the
/// same relative root. Stale `.lock` files whose sandbox directory no
/// longer exists are removed as housekeeping when they can be locked.
///
/// TOCTOU note: `acquire_reapable_lease` returns the still-locked file
/// handle rather than a bool, and every caller here holds that handle for
/// the entire duration of the delete (`remove_dir_all`/`remove_file`) before
/// dropping it. This closes the window between "lease looked reapable" and
/// "sandbox actually deleted" during which a restarted orchestrator could
/// otherwise re-acquire the lease and start using a sandbox the reaper is
/// mid-delete on.
///
/// Ordering guarantee: within a single `reap_orphaned_sandboxes` call, the
/// main loop below removes each reaped sandbox's `.lock` file itself
/// (while still holding its lease) before `remove_stale_lease_files` runs
/// afterward, so the two passes never double-process the same lock file.
///
/// Publish-side TOCTOU note: this reaper's fail-closed check only protects
/// against deleting a sandbox once it is published. The other half of the
/// guarantee lives in `DelegatedTaskSandboxOrchestrator::initialize`, which
/// acquires the lease BEFORE creating the sandbox directory — so a reaper
/// scan can never observe a freshly-published `task-<id>` dir that does not
/// yet have its lease in place.
///
/// Lease-unlink ownership rule (uniform across this file): a lease path is
/// only ever unlinked while its lock is still held — never drop-then-unlink.
/// Dropping the lock first and unlinking after leaves a window in which a
/// restarted same-run-id lane can acquire the now-unlocked lease before the
/// unlink runs, so the unlink would then delete that NEW owner's lock file
/// instead of the stale one it meant to remove. Every removal site in this
/// module follows hold-through-remove-then-drop:
/// - This reaper's main loop above and `remove_stale_lease_files` below
///   both remove a lock file only while still holding the lock they just
///   (re-)acquired for that purpose.
/// - `DelegatedTaskSandboxOrchestrator::initialize`'s publish-failure path
///   removes the now-orphaned lease file while still holding
///   `acquired_lease`, then drops it.
/// - `DelegatedTaskSandboxOrchestrator::cleanup`, by contrast, never
///   unlinks the lease file at all — it only releases the lock. Lock-file
///   removal for a *successfully cleaned-up* sandbox is exclusively this
///   reaper's job (a leftover unlocked lock file is swept up by the next
///   `reap_orphaned_sandboxes` call), since `cleanup` has no way to hold
///   the lock through a remove without reintroducing the same race for
///   *its own* case (the sandbox may be reused by a restarted same-run-id
///   lane immediately after cleanup, before any reap runs).
pub fn reap_orphaned_sandboxes(
    delegated_tasks_root: &Path,
    active_run_ids: &[&str],
) -> Result<Vec<PathBuf>, std::io::Error> {
    let mut removed = Vec::new();
    if !delegated_tasks_root.exists() {
        return Ok(removed);
    }
    for entry in std::fs::read_dir(delegated_tasks_root)? {
        let entry = entry?;
        if !entry.file_type()?.is_dir() {
            continue;
        }
        let name = entry.file_name();
        let Some(name) = name.to_str() else { continue };
        let Some(run_id) = name.strip_prefix("task-") else {
            continue;
        };
        if active_run_ids.contains(&run_id) {
            continue;
        }
        let path = entry.path();
        let lease_path = lease_path_for_sandbox(&path);
        let Some(held_lease) = acquire_reapable_lease(&lease_path) else {
            // Owner process is alive and holding the lease: skip, do not delete.
            continue;
        };
        let worktree_removed = Command::new("git")
            .arg("worktree")
            .arg("remove")
            .arg("--force")
            .arg(&path)
            .output()
            .map(|output| output.status.success())
            .unwrap_or(false);
        if !worktree_removed {
            std::fs::remove_dir_all(&path)?;
        }
        // Remove the lock file while still holding `held_lease`. On
        // Windows, std opens files with FILE_SHARE_DELETE, so calling
        // `remove_file` on a path we hold an open (locked) handle to
        // succeeds by marking the file delete-on-close; the directory
        // entry disappears once `held_lease` drops below. On Unix,
        // unlinking a file while `flock`ed is unconditionally fine.
        let _ = std::fs::remove_file(&lease_path);
        drop(held_lease);
        removed.push(path);
    }
    remove_stale_lease_files(delegated_tasks_root)?;
    Ok(removed)
}

/// A lease held for the duration of a reap delete. `Locked` means this call
/// acquired and now holds the file's exclusive lock; `Absent` means there
/// was no lease file to lock in the first place (legacy sandbox, or no
/// lease was ever acquired). Either way the sandbox is safe to reap. Kept
/// alive (not `_`-discarded) by callers until after the delete completes:
/// the `File` in `Locked` is a pure RAII guard, held only for its `Drop`
/// side effect (releasing the OS lock), and is never read otherwise.
enum ReapableLease {
    Locked(#[allow(dead_code)] std::fs::File),
    Absent,
}

/// Classifies a lease-file `open()` error for `acquire_reapable_lease`.
/// Only `NotFound` means "no lease file exists" (legacy sandbox, or a lease
/// that was never created) — every other error (permission denied, sharing
/// violation, I/O error, etc.) is ambiguous and must be treated the same as
/// "lease is held elsewhere": fail-closed toward NOT reaping, since we
/// cannot distinguish a transient/permission failure to open an
/// owner-locked file from genuine absence. Extracted as a standalone
/// function so the classification itself is unit-testable without needing
/// to simulate real OS-level permission or sharing-violation errors, which
/// is awkward to do portably across Windows and Unix in an integration
/// test.
pub(crate) fn open_error_means_no_lease_file(error: &std::io::Error) -> bool {
    error.kind() == std::io::ErrorKind::NotFound
}

/// Attempts to acquire the lease for a sandbox slated for reaping, and
/// returns the still-locked handle on success so the caller can hold it
/// through the delete (closing the TOCTOU window between "reapable" and
/// "reaped"). Returns `None` when the lease is currently held elsewhere
/// (owner alive: `try_lock` fails) — fail-closed toward NOT deleting.
/// Returns `None` on any lease-file open error other than `NotFound` too
/// (see `open_error_means_no_lease_file`): a permission or sharing-violation
/// error opening the lease file is ambiguous and must not be treated as
/// "no lease" — an owner could plausibly hold the file in a way that also
/// prevents this call from opening it.
fn acquire_reapable_lease(lease_path: &Path) -> Option<ReapableLease> {
    match std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(lease_path)
    {
        Ok(lock_file) => match lock_file.try_lock() {
            Ok(()) => Some(ReapableLease::Locked(lock_file)),
            Err(_) => None,
        },
        Err(error) if open_error_means_no_lease_file(&error) => Some(ReapableLease::Absent),
        Err(_) => None,
    }
}

/// Housekeeping: removes `task-<id>.lock` files whose corresponding
/// `task-<id>` sandbox directory no longer exists, when the lock can be
/// acquired (i.e. is not held by a live process). This can happen if a
/// sandbox directory was removed by means other than `cleanup`/`reap`. Holds
/// the acquired lease through the `remove_file` call for the same TOCTOU
/// reasons as the main reap loop above.
fn remove_stale_lease_files(delegated_tasks_root: &Path) -> Result<(), std::io::Error> {
    for entry in std::fs::read_dir(delegated_tasks_root)? {
        let entry = entry?;
        if !entry.file_type()?.is_file() {
            continue;
        }
        let path = entry.path();
        let Some(name) = entry.file_name().to_str().map(str::to_string) else {
            continue;
        };
        let Some(stem) = name.strip_suffix(".lock") else {
            continue;
        };
        if !stem.starts_with("task-") {
            continue;
        }
        if delegated_tasks_root.join(stem).exists() {
            continue;
        }
        if let Some(held_lease) = acquire_reapable_lease(&path) {
            let _ = std::fs::remove_file(&path);
            drop(held_lease);
        }
    }
    Ok(())
}

fn copy_workspace_tree(source: &Path, destination: &Path) -> Result<(), std::io::Error> {
    // The root sandbox path is passed through all recursive calls so we can avoid
    // copying any source entry that equals or contains the sandbox directory. This
    // prevents the infinite-recursion / stack-overflow that would occur when the
    // sandbox is nested inside the workspace root (the canonical `with_workspace_root`
    // layout places it at `workspace_root/target/delegated-tasks/task-<id>`).
    copy_workspace_tree_impl(source, destination, destination)
}

fn copy_workspace_tree_impl(
    source: &Path,
    destination: &Path,
    root_sandbox: &Path,
) -> Result<(), std::io::Error> {
    for entry in std::fs::read_dir(source)? {
        let entry = entry?;
        let source_path = entry.path();
        let destination_path = destination.join(entry.file_name());
        // Skip entries that are, or are ancestors of, the sandbox root. Without
        // this guard, copying `workspace_root` into `workspace_root/target/…/sandbox`
        // would recurse into `target`, then into `delegated-tasks`, then into
        // `sandbox` itself — the copy-into-itself cycle — causing a stack overflow.
        if source_path == root_sandbox || root_sandbox.starts_with(&source_path) {
            continue;
        }
        let file_type = entry.file_type()?;
        if file_type.is_dir() {
            std::fs::create_dir_all(&destination_path)?;
            copy_workspace_tree_impl(&source_path, &destination_path, root_sandbox)?;
        } else if file_type.is_file() {
            if let Some(parent) = destination_path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::copy(&source_path, &destination_path)?;
        }
    }
    Ok(())
}

/// Ensures `sandbox_path` is not the workspace root or any ancestor of it.
///
/// This prevents any configuration from accidentally targeting the main workspace
/// as the sandbox write target, which would allow agent-generated writes to land
/// directly in the source tree rather than an isolated copy.
///
/// Both paths are canonicalized (or lexically normalized when the path does not
/// yet exist) before comparison so that symlink aliases and Windows UNC prefixes
/// (`\\?\`) do not produce false negatives.
///
/// Returns `Err` when:
/// - `sandbox_path` is the same directory as `workspace_root`, or
/// - `sandbox_path` is an ancestor of `workspace_root` (i.e. the workspace root
///   is *inside* the proposed sandbox directory).
pub fn validate_not_main_workspace(
    sandbox_path: &Path,
    workspace_root: &Path,
) -> Result<(), AgentError> {
    let strip_unc = |p: PathBuf| -> PathBuf {
        if let Some(s) = p.to_str().and_then(|s| s.strip_prefix(r"\\?\")) {
            PathBuf::from(s)
        } else {
            p
        }
    };

    let canonical_workspace = std::fs::canonicalize(workspace_root)
        .map(strip_unc)
        .unwrap_or_else(|_| strip_unc(workspace_root.to_path_buf()));

    let canonical_sandbox = std::fs::canonicalize(sandbox_path)
        .map(strip_unc)
        .unwrap_or_else(|_| strip_unc(sandbox_path.to_path_buf()));

    if canonical_sandbox == canonical_workspace {
        return Err(AgentError::InvalidMetadata(
            AssistedAiContractError::InvalidProposalMetadata {
                reason: format!(
                    "sandbox path is identical to workspace root: {}",
                    workspace_root.display()
                ),
            },
        ));
    }

    // Also reject when the sandbox is a parent of the workspace root — placing
    // the workspace inside the sandbox write boundary would expose it to writes.
    if canonical_workspace.starts_with(&canonical_sandbox) {
        return Err(AgentError::InvalidMetadata(
            AssistedAiContractError::InvalidProposalMetadata {
                reason: format!(
                    "sandbox path '{}' is an ancestor of workspace root '{}' — \
                     this would expose the workspace to sandbox writes",
                    sandbox_path.display(),
                    workspace_root.display()
                ),
            },
        ));
    }

    Ok(())
}

fn validate_sandbox_permission(
    permission: &DelegatedTaskToolPermissionRequest,
    operation: &str,
) -> Result<(), std::io::Error> {
    let write_profile = permission.profile == DelegatedTaskToolPermissionProfile::Write;
    let sandbox_action = matches!(
        permission.action_class,
        PermissionBudgetActionClass::AccessWorkspaceFiles
            | PermissionBudgetActionClass::InvokeLocalTool
    );
    let delegated_runtime_capability = permission
        .capability
        .as_ref()
        .is_some_and(|capability| capability.0 == "delegated.runtime.allocate");
    if write_profile
        && sandbox_action
        && delegated_runtime_capability
        && permission.runtime_allowed
        && permission.human_approval_recorded
        && !permission.deny_overrides
    {
        return Ok(());
    }
    Err(std::io::Error::new(
        std::io::ErrorKind::PermissionDenied,
        format!("delegated sandbox {operation} requires approved Write tool permission"),
    ))
}

/// Validate that `path` is contained within the base directory.
///
/// Returns the normalized path *relative to* `base` (with any `.`/`..`
/// segments already collapsed) so callers emit a genuinely canonical path
/// rather than one that still embeds traversal segments. Any path that
/// escapes the base or retains residual traversal/root/prefix components
/// after normalization is rejected.
///
/// Symlink discipline: both sides of the check are resolved through
/// `std::fs::canonicalize` so the comparison happens in real-filesystem
/// space, not lexical space. For the target path (which may name a file
/// that does not exist yet), the deepest EXISTING ancestor is canonicalized
/// and the non-existent remainder is re-appended after lexical cleaning.
/// This simultaneously fixes a false REJECT (symlink-aliased bases, e.g.
/// macOS `/var` -> `/private/var` temp dirs) and a false ALLOW (an existing
/// in-sandbox symlink pointing outside passed the previous lexical check
/// while real I/O would follow it out). A component whose symlink cannot be
/// resolved (dangling) fails closed.
pub fn validate_containment(base: &Path, path: &Path) -> Result<PathBuf, AgentError> {
    let base_absolute =
        std::fs::canonicalize(base).unwrap_or_else(|_| std::env::current_dir().unwrap().join(base));

    let path_absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir().unwrap().join(path)
    };

    let mut clean_components = Vec::new();
    for component in path_absolute.components() {
        match component {
            std::path::Component::ParentDir => {
                clean_components.pop();
            }
            std::path::Component::CurDir => {}
            c => {
                clean_components.push(c);
            }
        }
    }

    let clean_lexical: PathBuf = clean_components.into_iter().collect();
    let clean_path = resolve_existing_prefix(&clean_lexical).ok_or_else(|| {
        AgentError::InvalidMetadata(AssistedAiContractError::InvalidProposalMetadata {
            reason: "Sandbox path contains an unresolvable (dangling) symlink component"
                .to_string(),
        })
    })?;

    // Strip Windows UNC prefix if present to prevent starts_with discrepancies
    let strip_unc = |p: &Path| -> PathBuf {
        let p_str = p.to_str().unwrap_or("");
        if let Some(stripped) = p_str.strip_prefix(r"\\?\") {
            PathBuf::from(stripped)
        } else {
            p.to_path_buf()
        }
    };

    let clean_stripped = strip_unc(&clean_path);
    let base_stripped = strip_unc(&base_absolute);

    let relative = clean_stripped.strip_prefix(&base_stripped).map_err(|_| {
        AgentError::InvalidMetadata(AssistedAiContractError::InvalidProposalMetadata {
            reason: "Path traversal escaped sandbox".to_string(),
        })
    })?;

    // Defense in depth: a normalized, contained path must not retain any
    // traversal, root, or prefix components.
    if relative.components().any(|component| {
        matches!(
            component,
            std::path::Component::ParentDir
                | std::path::Component::RootDir
                | std::path::Component::Prefix(_)
        )
    }) {
        return Err(AgentError::InvalidMetadata(
            AssistedAiContractError::InvalidProposalMetadata {
                reason: "Normalized sandbox path retained traversal components".to_string(),
            },
        ));
    }

    Ok(relative.to_path_buf())
}

/// Proposal generator inside `legion-agent`.
#[derive(Debug, Clone)]
pub struct DelegatedTaskProposalGenerator {
    sandbox_base: PathBuf,
}

/// Request-scoped inputs for delegated task proposal generation.
#[derive(Debug, Clone)]
pub struct DelegatedTaskProposalInput<'a> {
    /// Target path inside the delegated task sandbox.
    pub target_path: &'a Path,
    /// Provider-produced file content for create-file proposals.
    pub modified_content: &'a str,
    /// Output identifier assigned by the caller.
    pub output_id: String,
    /// Provider request identifier associated with the proposal.
    pub request_id: String,
    /// Provider identifier that produced the proposed content.
    pub provider_id: String,
    /// Proposal identifier assigned by the caller.
    pub proposal_id: ProposalId,
    /// Principal on whose behalf the proposal was generated.
    pub principal: PrincipalId,
    /// Capability authorizing proposal generation.
    pub capability: CapabilityId,
    /// Correlation identifier for observability.
    pub correlation_id: CorrelationId,
    /// Causality identifier for observability.
    pub causality_id: CausalityId,
    /// Creation timestamp assigned by the caller.
    pub created_at: TimestampMillis,
    /// Metadata-only context manifest reference used to generate the proposal.
    pub context_manifest: AssistedAiTrustProjectionReference,
    /// Metadata-only approval checklist reference gating the proposal.
    pub approval_checklist: AssistedAiTrustProjectionReference,
}

impl DelegatedTaskProposalGenerator {
    /// Creates a new proposal generator.
    pub fn new(sandbox_base: PathBuf) -> Self {
        Self { sandbox_base }
    }

    /// Builds an `AssistedAiEditProposalOutput` for the sandbox target.
    ///
    /// Stats and reads the base target (the HEAD checkout / sandbox-base copy at
    /// `target_path`) and compares it with the provider-produced
    /// `modified_content` to decide the payload shape:
    ///
    /// * When the base target does **not** exist this is a genuine creation and a
    ///   [`ProposalPayload::CreateFile`] is emitted with no preconditions.
    /// * When the base target **already exists** a path-based
    ///   [`ProposalPayload::CreateFile`] (create/overwrite) is emitted carrying
    ///   the full modified content plus a content-level concurrency guard. The
    ///   generator has no workspace `FileId` or open buffer, so it cannot emit a
    ///   buffer-addressable `TextEdit` (the apply path resolves edits via
    ///   `buffer_for_file`); a path-based proposal with preconditions is the
    ///   appliable representation.
    ///
    /// For an existing base the size, last-modified timestamp, content
    /// fingerprint, and a content-derived [`legion_protocol::FileContentVersion`]
    /// are recorded in [`ProposalVersionPreconditions`] so an apply step can
    /// detect concurrent modification. The proposal target path is the
    /// normalized, sandbox-relative path returned by [`validate_containment`].
    pub fn generate_proposal(
        &self,
        input: DelegatedTaskProposalInput<'_>,
    ) -> Result<AssistedAiEditProposalOutput, AgentError> {
        let target_relative = validate_containment(&self.sandbox_base, input.target_path)?;
        // Emit a canonical, forward-slash relative path regardless of host OS so
        // the `CanonicalPath` payload is portable and free of `..` segments.
        let target_relative = target_relative
            .components()
            .map(|component| {
                component.as_os_str().to_str().ok_or_else(|| {
                    AgentError::InvalidMetadata(AssistedAiContractError::InvalidProposalMetadata {
                        reason: "Proposal target path is not valid UTF-8".to_string(),
                    })
                })
            })
            .collect::<Result<Vec<_>, _>>()?
            .join("/");

        // Stat/read the base target so we can both populate a concurrency guard
        // and decide whether this is a create (no base) or an edit (base exists).
        let base_state = BaseTargetState::read(input.target_path);

        let create_payload = || {
            ProposalPayload::CreateFile(legion_protocol::CreateFileProposal {
                path: CanonicalPath(target_relative.clone()),
                initial_content: Some(input.modified_content.to_string()),
            })
        };

        // The delegated generator works from the sandbox output and has no
        // workspace `FileId` or open editor buffer. A `TextEdit` proposal would
        // need a real buffer-addressable file id (the apply path resolves edits
        // via `buffer_for_file`), so a synthetic id would make existing-file
        // proposals reject at apply time. Always emit a path-based create/
        // overwrite proposal; when a base already exists, attach a content-level
        // concurrency guard derived from its snapshot.
        let (payload, preconditions) = match &base_state {
            Some(base) => (create_payload(), base.preconditions()),
            None => (create_payload(), empty_preconditions()),
        };
        let preview_summary = "Create file proposal".to_string();

        Ok(AssistedAiEditProposalOutput {
            output_id: input.output_id,
            request_id: input.request_id,
            provider_id: input.provider_id,
            proposal_id: input.proposal_id,
            principal: input.principal,
            capability: input.capability,
            correlation_id: input.correlation_id,
            causality_id: input.causality_id,
            payload,
            preconditions,
            preview: PreviewSummary {
                summary: preview_summary,
                details: vec![],
            },
            expires_at: None,
            created_at: input.created_at,
            context_manifest: input.context_manifest,
            approval_checklist: input.approval_checklist,
            redaction_hints: vec![RedactionHint::MetadataOnly],
            schema_version: 1,
        })
    }
}

/// On-disk state of a base proposal target captured once so that both the
/// concurrency guard and the create-vs-edit decision use a consistent snapshot.
struct BaseTargetState {
    /// Length of the base file in bytes.
    len: u64,
    /// Last-modified timestamp when the platform exposes one.
    modified_at: Option<TimestampMillis>,
    /// Stable FNV-1a digest of the base bytes when readable.
    content_hash: Option<u64>,
}

impl BaseTargetState {
    /// Reads the base target at `path`. Returns `None` when the target does not
    /// exist or is not a regular file (a genuine create). Any read error after a
    /// successful stat still yields a state (with `content_hash == None`) so the
    /// generator treats an existing file as an overwrite rather than
    /// misclassifying it as a create.
    fn read(path: &Path) -> Option<Self> {
        let metadata = std::fs::metadata(path).ok()?;
        if !metadata.is_file() {
            return None;
        }
        let modified_at = metadata
            .modified()
            .ok()
            .and_then(|modified| modified.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|elapsed| TimestampMillis(u64::try_from(elapsed.as_millis()).unwrap_or(u64::MAX)));
        let content_hash = std::fs::read(path)
            .ok()
            .map(|bytes| stable_hash_128(&bytes) as u64);
        Some(Self {
            len: metadata.len(),
            modified_at,
            content_hash,
        })
    }

    /// Derives a concurrency guard from this base snapshot. The length and
    /// modified timestamp always populate; the fingerprint and content version
    /// populate only when the base bytes were readable.
    fn preconditions(&self) -> ProposalVersionPreconditions {
        ProposalVersionPreconditions {
            file_version: None,
            buffer_version: None,
            snapshot_id: None,
            generation: None,
            file_content_version: self.content_hash.map(legion_protocol::FileContentVersion),
            workspace_generation: None,
            expected_fingerprint: self.content_hash.map(|hash| FileFingerprint {
                algorithm: "fnv1a-64-v1".to_string(),
                value: format!("{hash:016x}"),
            }),
            expected_file_length: Some(self.len),
            expected_modified_at: self.modified_at,
        }
    }
}

/// Preconditions for a genuine create: nothing on disk to guard against.
fn empty_preconditions() -> ProposalVersionPreconditions {
    ProposalVersionPreconditions {
        file_version: None,
        buffer_version: None,
        snapshot_id: None,
        generation: None,
        file_content_version: None,
        workspace_generation: None,
        expected_fingerprint: None,
        expected_file_length: None,
        expected_modified_at: None,
    }
}

/// Deterministic, cross-version-stable 128-bit FNV-1a hash.
///
/// Unlike `std::collections::hash_map::DefaultHasher`, FNV-1a is a fixed
/// published specification, so identifiers and fingerprints derived from it stay
/// stable across compiler versions, platforms, and runs. The unit-separator
/// (`\u{1f}`) between domain prefix and payload prevents prefix-collision
/// ambiguity between distinct id namespaces.
pub(crate) fn stable_hash_128(bytes: &[u8]) -> u128 {
    // FNV-1a (128-bit) constants.
    const FNV_OFFSET_BASIS: u128 = 0x6c62_272e_07bb_0142_62b8_2175_6295_c58d;
    const FNV_PRIME: u128 = 0x0000_0000_0100_0000_0000_0000_0000_013b;

    let mut hash = FNV_OFFSET_BASIS;
    for &byte in bytes {
        hash ^= byte as u128;
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    hash
}

#[cfg(test)]
mod tests {
    use super::*;
    use legion_protocol::{
        CapabilityId, DelegatedTaskToolPermissionDecision, DelegatedTaskToolPermissionProfile,
        DelegatedTaskToolPermissionRequestInput, PermissionBudgetActionClass,
        delegated_task_tool_permission_request,
    };

    /// Constructs an approved Write tool permission for sandbox operations.
    fn approved_sandbox_permission() -> DelegatedTaskToolPermissionRequest {
        delegated_task_tool_permission_request(DelegatedTaskToolPermissionRequestInput {
            request_id: "test-sandbox-permission".to_string(),
            profile: DelegatedTaskToolPermissionProfile::Write,
            action_class: PermissionBudgetActionClass::AccessWorkspaceFiles,
            capability: Some(CapabilityId("delegated.runtime.allocate".to_string())),
            target_id: None,
            decision: DelegatedTaskToolPermissionDecision::Allow,
            labels: vec![],
            schema_version: 1,
        })
    }

    #[test]
    fn open_error_means_no_lease_file_is_true_only_for_not_found() {
        // NotFound (no lease file exists yet, or a legacy sandbox predating
        // the lease protocol) is the only error classified as "safe to
        // treat as an absent lease" — everything else must fail closed
        // toward "possibly protected, do not reap", since a permission or
        // sharing-violation error opening the lease file cannot be
        // distinguished from an owner holding it in a way that also blocks
        // this call from opening it. A real OS-level permission-denied or
        // sharing-violation error is awkward to simulate portably across
        // Windows and Unix in an integration test, so the classification
        // itself is exercised directly here via synthetic `io::Error`
        // values of each kind instead.
        assert!(open_error_means_no_lease_file(&std::io::Error::from(
            std::io::ErrorKind::NotFound
        )));

        let non_absent_kinds = [
            std::io::ErrorKind::PermissionDenied,
            std::io::ErrorKind::AlreadyExists,
            std::io::ErrorKind::InvalidInput,
            std::io::ErrorKind::Other,
            std::io::ErrorKind::Interrupted,
            std::io::ErrorKind::TimedOut,
        ];
        for kind in non_absent_kinds {
            assert!(
                !open_error_means_no_lease_file(&std::io::Error::from(kind)),
                "error kind {kind:?} must fail closed (not be treated as an absent lease)"
            );
        }
    }

    // -----------------------------------------------------------------------
    // D1: isolation mode and lease status
    // -----------------------------------------------------------------------

    #[test]
    fn isolation_mode_starts_as_not_initialized() {
        let orchestrator = DelegatedTaskSandboxOrchestrator::new("test-run");
        assert_eq!(
            orchestrator.isolation_mode(),
            SandboxIsolationMode::NotInitialized
        );
        assert!(!orchestrator.lease_acquired());
    }

    /// Initializes an orchestrator with a temp dir that has no git repo and verifies
    /// that (a) the sandbox is created as a directory copy, and (b) `isolation_mode()`
    /// returns `DirectoryCopy`.
    #[test]
    fn isolation_mode_is_directory_copy_when_git_not_available() {
        let tmp = tempfile::tempdir().expect("tmp dir");
        let source_root = tmp.path().join("source");
        std::fs::create_dir_all(&source_root).expect("source root");
        std::fs::write(source_root.join("hello.txt"), "hello").expect("fixture file");

        let sandbox_root = tmp.path().join("sandboxes");
        std::fs::create_dir_all(&sandbox_root).expect("sandbox root");

        let mut orchestrator = DelegatedTaskSandboxOrchestrator::with_sandbox_root(
            &sandbox_root,
            "test-no-git",
            Some(&source_root),
        );

        assert_eq!(
            orchestrator.isolation_mode(),
            SandboxIsolationMode::NotInitialized,
            "mode must be NotInitialized before initialize()"
        );

        let permission = approved_sandbox_permission();
        orchestrator
            .initialize(&permission)
            .expect("initialize should succeed via directory-copy fallback");

        // Without a real git repo the worktree command fails → fallback to copy.
        assert_eq!(
            orchestrator.isolation_mode(),
            SandboxIsolationMode::DirectoryCopy,
            "expected DirectoryCopy when git worktree is unavailable"
        );

        assert!(
            orchestrator.sandbox_path().exists(),
            "sandbox directory must be created"
        );
        assert!(
            orchestrator.sandbox_path().join("hello.txt").exists(),
            "source files must be copied into the sandbox"
        );
    }

    // -----------------------------------------------------------------------
    // D3: main-workspace protection
    // -----------------------------------------------------------------------

    #[test]
    fn validate_not_main_workspace_rejects_identical_paths() {
        let tmp = tempfile::tempdir().expect("tmp dir");
        let workspace_root = tmp.path().join("workspace");
        std::fs::create_dir_all(&workspace_root).expect("workspace dir");

        let result = validate_not_main_workspace(&workspace_root, &workspace_root);
        assert!(
            result.is_err(),
            "sandbox == workspace_root must be rejected"
        );
        let msg = result.unwrap_err().to_string();
        assert!(
            msg.contains("identical") || msg.contains("same"),
            "error message should mention identity: {msg}"
        );
    }

    #[test]
    fn validate_not_main_workspace_rejects_when_sandbox_is_parent_of_workspace() {
        let tmp = tempfile::tempdir().expect("tmp dir");
        let workspace_root = tmp.path().join("projects").join("my-project");
        std::fs::create_dir_all(&workspace_root).expect("workspace dir");

        // sandbox_path = parent of workspace_root
        let sandbox_path = tmp.path().join("projects");

        let result = validate_not_main_workspace(&sandbox_path, &workspace_root);
        assert!(
            result.is_err(),
            "sandbox being a parent of workspace_root must be rejected"
        );
        let msg = result.unwrap_err().to_string();
        assert!(
            msg.contains("ancestor") || msg.contains("parent"),
            "error message should mention ancestry: {msg}"
        );
    }

    #[test]
    fn validate_not_main_workspace_accepts_sibling_directory() {
        let tmp = tempfile::tempdir().expect("tmp dir");
        let workspace_root = tmp.path().join("workspace");
        std::fs::create_dir_all(&workspace_root).expect("workspace dir");

        let sandbox_path = tmp.path().join("sandbox");
        std::fs::create_dir_all(&sandbox_path).expect("sandbox dir");

        let result = validate_not_main_workspace(&sandbox_path, &workspace_root);
        assert!(
            result.is_ok(),
            "sibling sandbox path must be accepted, got: {:?}",
            result.err()
        );
    }

    #[test]
    fn initialize_fails_when_sandbox_path_equals_workspace_root() {
        // Craft sandbox_path == workspace_root by naming the workspace dir
        // "task-<run_id>" and placing sandbox_root at its parent directory.
        // with_sandbox_root(&parent, "run_id", ...) → sandbox_path = parent/task-run_id
        // which equals workspace_root = parent/task-run_id.
        let tmp = tempfile::tempdir().expect("tmp dir");
        let workspace_root = tmp.path().join("task-run-protected");
        std::fs::create_dir_all(&workspace_root).expect("workspace dir");

        let mut orchestrator = DelegatedTaskSandboxOrchestrator::with_sandbox_root(
            tmp.path(),
            "run-protected",
            Some(&workspace_root),
        );
        // sandbox_path is tmp/task-run-protected == workspace_root

        let permission = approved_sandbox_permission();
        let result = orchestrator.initialize(&permission);
        assert!(
            result.is_err(),
            "initialize must fail when sandbox_path == workspace_root"
        );
        let msg = result.unwrap_err().to_string();
        assert!(
            msg.contains("identical") || msg.contains("same") || msg.contains("workspace"),
            "error message should reference workspace protection: {msg}"
        );
    }
}
