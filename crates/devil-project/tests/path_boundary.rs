use std::path::{Path, PathBuf};
use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};

static TEMP_ROOT_COUNTER: AtomicU64 = AtomicU64::new(0);

use devil_platform::{NativeFileSystem, NativeWatcherService};
use devil_project::{WorkspaceActor, WorkspaceError, WorkspaceSaveRequest};
use devil_protocol::{
    BufferVersion, CanonicalPath, CapabilityId, CapabilityNamespace, CausalityId, CorrelationId,
    PrincipalId, ProposalId, ProposalResponse, ProposalStaleReason, SnapshotId,
    WorkspaceOpenRequest, WorkspaceTrustState,
};
use devil_security::{DenyByDefaultBroker, SecurityPolicy};
use uuid::Uuid;

fn open_workspace(
    root: &Path,
    trust: WorkspaceTrustState,
) -> (WorkspaceActor, devil_protocol::WorkspaceOpened) {
    let mut policy = SecurityPolicy::default();
    policy.path_policy.readable_roots = vec![root.to_string_lossy().into_owned()];
    policy.path_policy.writable_roots = vec![root.to_string_lossy().into_owned()];

    let actor = WorkspaceActor::new(
        Arc::new(NativeFileSystem),
        Arc::new(NativeWatcherService),
        DenyByDefaultBroker::new(policy, CapabilityNamespace("test".to_string())),
    );

    let opened = actor
        .open_workspace(WorkspaceOpenRequest {
            correlation_id: CorrelationId(1),
            principal_id: PrincipalId("tester".to_string()),
            root_path: CanonicalPath(root.to_string_lossy().into_owned()),
            trust: Some(trust),
        })
        .expect("open workspace");

    (actor, opened)
}

fn create_temp_workspace() -> PathBuf {
    let root = std::env::temp_dir().join(format!(
        "devil-project-path-boundary-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |value| value.as_millis() as u64)
            + TEMP_ROOT_COUNTER.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&root).expect("create temp workspace");
    std::fs::canonicalize(root).expect("canonicalize temp workspace")
}

fn save_new_file(
    actor: &WorkspaceActor,
    workspace_id: devil_protocol::WorkspaceId,
    path: impl AsRef<str>,
    text: impl Into<String>,
) -> Result<devil_project::WorkspaceSaveApplied, ProposalResponse> {
    let opened = actor
        .open_new_file_text(workspace_id, path.as_ref())
        .expect("open safe new-file precondition");
    let text = text.into();
    actor.save_file_with_proposal(WorkspaceSaveRequest {
        workspace_id,
        proposal_id: ProposalId(1),
        principal: PrincipalId("tester".to_string()),
        required_capability: CapabilityId("fs.write".to_string()),
        file_id: opened.identity.file_id,
        path: opened.identity.canonical_path,
        expected_fingerprint: opened.fingerprint,
        expected_file_content_version: opened.file_content_version,
        expected_workspace_generation: opened.workspace_generation,
        buffer_version: BufferVersion(1),
        snapshot_id: SnapshotId(1),
        payload_byte_len: text.len() as u64,
        correlation_id: CorrelationId(1),
        causality_id: CausalityId(Uuid::now_v7()),
        text,
    })
}

#[test]
fn resolve_and_write_reject_parent_escape() {
    let root = create_temp_workspace();
    let (actor, opened) = open_workspace(&root, WorkspaceTrustState::Trusted);

    let escape = "../escape.txt";
    let resolve_err = actor
        .resolve_file(opened.workspace_id, escape)
        .expect_err("resolve should fail");
    assert!(matches!(
        resolve_err,
        WorkspaceError::PathOutsideRoot { .. }
    ));

    let write_err = actor
        .open_new_file_text(opened.workspace_id, escape)
        .expect_err("new-file open should fail");
    assert!(matches!(write_err, WorkspaceError::PathOutsideRoot { .. }));

    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn nonexistent_child_path_inside_root_is_allowed() {
    let root = create_temp_workspace();
    let (actor, opened) = open_workspace(&root, WorkspaceTrustState::Trusted);

    save_new_file(&actor, opened.workspace_id, "new/deep/file.txt", "ok")
        .expect("write nested child");

    let text = actor
        .read_file_text(opened.workspace_id, "new/deep/file.txt")
        .expect("read nested child");
    assert_eq!(text, "ok");

    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn stale_save_preserves_disk_content_and_returns_typed_response() {
    let root = create_temp_workspace();
    let (actor, opened) = open_workspace(&root, WorkspaceTrustState::Trusted);
    let target = root.join("stale-save.txt");
    std::fs::write(&target, "seed").expect("seed file");

    let opened_file = actor
        .open_existing_file_text(opened.workspace_id, target.to_string_lossy())
        .expect("open existing file");
    std::fs::write(&target, "external").expect("external overwrite");

    let response = actor
        .save_file_with_proposal(WorkspaceSaveRequest {
            workspace_id: opened.workspace_id,
            proposal_id: ProposalId(2),
            principal: PrincipalId("tester".to_string()),
            required_capability: CapabilityId("fs.write".to_string()),
            file_id: opened_file.identity.file_id,
            path: opened_file.identity.canonical_path,
            expected_fingerprint: opened_file.fingerprint,
            expected_file_content_version: opened_file.file_content_version,
            expected_workspace_generation: opened_file.workspace_generation,
            buffer_version: BufferVersion(2),
            snapshot_id: SnapshotId(2),
            payload_byte_len: "buffer".len() as u64,
            correlation_id: CorrelationId(2),
            causality_id: CausalityId(Uuid::now_v7()),
            text: "buffer".to_string(),
        })
        .expect_err("external overwrite should reject stale proposal");

    assert_eq!(
        std::fs::read_to_string(&target).expect("disk content"),
        "external"
    );
    match response {
        ProposalResponse::Stale { stale, .. } => {
            assert_eq!(
                stale.reason,
                ProposalStaleReason::FileContentVersionMismatch
            );
        }
        ProposalResponse::Conflict { .. } => {}
        other => panic!("expected stale or conflict response, got {other:?}"),
    }

    let _ = std::fs::remove_dir_all(root);
}

#[cfg(windows)]
#[test]
fn windows_drive_letter_case_is_accepted_inside_root() {
    let root = create_temp_workspace();
    let (actor, opened) = open_workspace(&root, WorkspaceTrustState::Trusted);

    let mut root_text = root.to_string_lossy().to_string();
    if let Some(first) = root_text.chars().next() {
        let flipped = if first.is_ascii_lowercase() {
            first.to_ascii_uppercase()
        } else {
            first.to_ascii_lowercase()
        };
        root_text.replace_range(0..1, &flipped.to_string());
    }

    let candidate = format!(
        "{}\\case-check.txt",
        root_text.trim_end_matches(['\\', '/'])
    );
    save_new_file(&actor, opened.workspace_id, &candidate, "case-ok")
        .expect("case-insensitive write should succeed");

    let _ = std::fs::remove_dir_all(root);
}

#[cfg(windows)]
#[test]
fn windows_long_path_prefix_inside_root_is_accepted() {
    let root = create_temp_workspace();
    let (actor, opened) = open_workspace(&root, WorkspaceTrustState::Trusted);

    let root_text = root.to_string_lossy().to_string();
    let long_prefixed = format!(
        "\\\\?\\{}\\long-path.txt",
        root_text
            .trim_start_matches("\\\\?\\")
            .trim_end_matches(['\\', '/'])
    );

    save_new_file(&actor, opened.workspace_id, &long_prefixed, "long")
        .expect("long-prefix write should succeed");

    let _ = std::fs::remove_dir_all(root);
}

#[cfg(unix)]
#[test]
fn symlink_escape_outside_root_is_rejected() {
    use std::os::unix::fs as unix_fs;

    let root = create_temp_workspace();
    let (actor, opened) = open_workspace(&root, WorkspaceTrustState::Trusted);

    let outside = root
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join("devil-project-path-boundary-outside.txt");
    std::fs::write(&outside, "outside").expect("write outside file");

    let link_path = root.join("link-outside.txt");
    unix_fs::symlink(&outside, &link_path).expect("create symlink");

    let err = actor
        .read_file_text(opened.workspace_id, link_path.to_string_lossy())
        .err()
        .expect("symlink escape should fail");
    assert!(matches!(err, WorkspaceError::PathOutsideRoot { .. }));

    let _ = std::fs::remove_file(outside);
    let _ = std::fs::remove_dir_all(root);
}
