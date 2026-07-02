use legion_agent::{DelegatedTaskSandboxOrchestrator, reap_orphaned_sandboxes};
use legion_protocol::{
    CapabilityId, DelegatedTaskToolPermissionDecision, DelegatedTaskToolPermissionProfile,
    DelegatedTaskToolPermissionRequestInput, PermissionBudgetActionClass,
    delegated_task_tool_permission_request,
};
use std::fs;
use std::path::{Path, PathBuf};

#[test]
fn initialize_copies_workspace_contents_when_git_worktree_is_unavailable() {
    let source_root = unique_temp_dir("legion-agent-worktree-source");
    fs::create_dir_all(source_root.join("src")).expect("source tree");
    fs::write(source_root.join("README.md"), "workspace root\n").expect("write root file");
    fs::write(source_root.join("src/lib.rs"), "pub fn copied() {}\n").expect("write source file");

    let mut orchestrator =
        DelegatedTaskSandboxOrchestrator::with_workspace_root(&source_root, "fallback-copy");
    let permission = approved_sandbox_permission("sandbox:init");

    orchestrator
        .initialize(&permission)
        .expect("initialize sandbox");

    let sandbox_path = orchestrator.sandbox_path().to_path_buf();
    assert!(sandbox_path.exists(), "sandbox path should be created");
    assert_eq!(
        fs::read_to_string(sandbox_path.join("README.md")).expect("copied root file"),
        "workspace root\n"
    );
    assert_eq!(
        fs::read_to_string(sandbox_path.join("src/lib.rs")).expect("copied nested file"),
        "pub fn copied() {}\n"
    );
    assert_ne!(
        sandbox_path, source_root,
        "sandbox must be disposable, not the main workspace"
    );

    orchestrator.cleanup(&permission).expect("cleanup sandbox");
    assert!(
        !sandbox_path.exists(),
        "sandbox should be removed after cleanup"
    );

    fs::remove_dir_all(&source_root).expect("remove temp source root");
}

/// Derives the sibling `.lock` lease path for a sandbox dir, mirroring the
/// private `lease_path_for_sandbox` convention inside `legion-agent`
/// (`task-<run_id>.lock` next to `task-<run_id>/`), since that helper is not
/// part of the public API.
fn lease_path_for(sandbox_path: &Path) -> PathBuf {
    let mut lease_path = sandbox_path.to_path_buf();
    let mut file_name = sandbox_path
        .file_name()
        .expect("sandbox path has a file name")
        .to_os_string();
    file_name.push(".lock");
    lease_path.set_file_name(file_name);
    lease_path
}

#[test]
fn initialize_holds_the_sandbox_lease_immediately_on_return() {
    let source_root = unique_temp_dir("legion-agent-worktree-source-lease");
    fs::write(source_root.join("README.md"), "workspace root\n").expect("write root file");

    let mut orchestrator =
        DelegatedTaskSandboxOrchestrator::with_workspace_root(&source_root, "lease-held-on-return");
    let permission = approved_sandbox_permission("sandbox:init-lease");

    orchestrator
        .initialize(&permission)
        .expect("initialize sandbox");

    let sandbox_path = orchestrator.sandbox_path().to_path_buf();
    let lease_path = lease_path_for(&sandbox_path);
    assert!(
        lease_path.exists(),
        "lease file should exist immediately after initialize() returns"
    );

    // The orchestrator itself must still be holding the lock: a fresh,
    // independent handle attempting to lock the same file must fail.
    let probe = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(&lease_path)
        .expect("open lease file for probing");
    assert!(
        probe.try_lock().is_err(),
        "a second handle must not be able to lock the sandbox's lease while \
         the orchestrator still holds it"
    );
    drop(probe);

    orchestrator.cleanup(&permission).expect("cleanup sandbox");
    assert!(
        !sandbox_path.exists(),
        "sandbox directory should be removed by cleanup"
    );
    // Ownership rule: cleanup() releases the lock but must NOT unlink the
    // lease file itself — only the reaper removes lock files, and only
    // while holding the lock it just re-acquired (race-free). A leftover,
    // now-unlocked lock file after cleanup is expected and safe: it will
    // be swept up by the next `reap_orphaned_sandboxes` call.
    assert!(
        lease_path.exists(),
        "cleanup must leave the (now-unlocked) lease file in place; \
         unlinking it is exclusively the reaper's job"
    );
    let probe = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(&lease_path)
        .expect("open leftover lease file for probing");
    assert!(
        probe.try_lock().is_ok(),
        "the leftover lease file must be unlocked after cleanup releases it"
    );
    drop(probe);

    fs::remove_dir_all(&source_root).expect("remove temp source root");
    let _ = fs::remove_file(&lease_path);
}

#[test]
fn reap_removes_a_leftover_unlocked_lease_file_left_by_cleanup() {
    let source_root = unique_temp_dir("legion-agent-worktree-source-lease-reap");
    fs::write(source_root.join("README.md"), "workspace root\n").expect("write root file");

    let mut orchestrator = DelegatedTaskSandboxOrchestrator::with_workspace_root(
        &source_root,
        "lease-leftover-reaped",
    );
    let permission = approved_sandbox_permission("sandbox:init-lease-reap");

    orchestrator
        .initialize(&permission)
        .expect("initialize sandbox");
    let sandbox_path = orchestrator.sandbox_path().to_path_buf();
    let lease_path = lease_path_for(&sandbox_path);

    orchestrator.cleanup(&permission).expect("cleanup sandbox");
    assert!(
        !sandbox_path.exists(),
        "sandbox directory should be gone after cleanup"
    );
    assert!(
        lease_path.exists(),
        "cleanup leaves the unlocked lease file behind for the reaper"
    );

    // The reaper only scans `task-*` directories directly under a given
    // root, so point it at the lease file's parent directory (the shared
    // `target/delegated-tasks` root under this crate).
    let delegated_tasks_root = lease_path
        .parent()
        .expect("lease file has a parent directory")
        .to_path_buf();
    reap_orphaned_sandboxes(&delegated_tasks_root, &[]).expect("reap succeeds");

    assert!(
        !lease_path.exists(),
        "the next reap pass must remove the leftover lease file left by cleanup"
    );

    fs::remove_dir_all(&source_root).expect("remove temp source root");
}

#[test]
fn failed_initialize_leaves_no_stale_lease_file() {
    // Force the fallback (non-worktree) path to fail by pointing
    // `source_root` at a path that does not exist: `copy_workspace_tree`
    // will fail to read it, causing `initialize` to return an error after a
    // lease was already acquired. The lease file must not survive that
    // failure.
    let missing_source_root =
        unique_temp_dir("legion-agent-worktree-missing-source").join("does-not-exist");

    let mut orchestrator = DelegatedTaskSandboxOrchestrator::with_workspace_root(
        &missing_source_root,
        "failed-init-no-stale-lock",
    );
    let permission = approved_sandbox_permission("sandbox:init-failure");

    let result = orchestrator.initialize(&permission);
    assert!(
        result.is_err(),
        "initialize should fail when the workspace root does not exist"
    );

    let sandbox_path = orchestrator.sandbox_path().to_path_buf();
    let lease_path = lease_path_for(&sandbox_path);
    assert!(
        !lease_path.exists(),
        "a failed initialize must not leave a stale lease file behind"
    );

    let _ = fs::remove_dir_all(&sandbox_path);
}

fn approved_sandbox_permission(
    request_id: &str,
) -> legion_protocol::DelegatedTaskToolPermissionRequest {
    delegated_task_tool_permission_request(DelegatedTaskToolPermissionRequestInput {
        request_id: request_id.to_string(),
        profile: DelegatedTaskToolPermissionProfile::Write,
        action_class: PermissionBudgetActionClass::AccessWorkspaceFiles,
        capability: Some(CapabilityId("delegated.runtime.allocate".to_string())),
        target_id: Some("target/delegated-tasks".to_string()),
        decision: DelegatedTaskToolPermissionDecision::Allow,
        labels: vec!["test".to_string()],
        schema_version: 1,
    })
}

fn unique_temp_dir(prefix: &str) -> PathBuf {
    let unique = format!("{prefix}-{}-{}", std::process::id(), uuid_like());
    let path = std::env::temp_dir().join(unique);
    if path.exists() {
        fs::remove_dir_all(&path).expect("remove stale temp dir");
    }
    fs::create_dir_all(&path).expect("create temp dir");
    path
}

fn uuid_like() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    format!("{nanos:x}")
}
