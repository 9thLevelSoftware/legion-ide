use legion_agent::DelegatedTaskSandboxOrchestrator;
use legion_protocol::{
    CapabilityId, DelegatedTaskToolPermissionDecision, DelegatedTaskToolPermissionProfile,
    DelegatedTaskToolPermissionRequestInput, PermissionBudgetActionClass,
    delegated_task_tool_permission_request,
};
use std::fs;
use std::path::PathBuf;

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
