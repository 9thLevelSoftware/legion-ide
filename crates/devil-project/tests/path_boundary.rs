use std::path::{Path, PathBuf};
use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};

static TEMP_ROOT_COUNTER: AtomicU64 = AtomicU64::new(0);

use devil_platform::{NativeFileSystem, NativeWatcherService};
use devil_project::{WorkspaceActor, WorkspaceError};
use devil_protocol::{
    CanonicalPath, CapabilityNamespace, CorrelationId, PrincipalId, WorkspaceOpenRequest,
    WorkspaceTrustState,
};
use devil_security::{DenyByDefaultBroker, SecurityPolicy};

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
        .write_file_text(opened.workspace_id, escape, "blocked")
        .expect_err("write should fail");
    assert!(matches!(write_err, WorkspaceError::PathOutsideRoot { .. }));

    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn nonexistent_child_path_inside_root_is_allowed() {
    let root = create_temp_workspace();
    let (actor, opened) = open_workspace(&root, WorkspaceTrustState::Trusted);

    actor
        .write_file_text(opened.workspace_id, "new/deep/file.txt", "ok")
        .expect("write nested child");

    let text = actor
        .read_file_text(opened.workspace_id, "new/deep/file.txt")
        .expect("read nested child");
    assert_eq!(text, "ok");

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
    actor
        .write_file_text(opened.workspace_id, &candidate, "case-ok")
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

    actor
        .write_file_text(opened.workspace_id, &long_prefixed, "long")
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
