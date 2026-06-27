//! WS-MANUAL-02 SCALE.06: Workspace tree open returns without blocking editor input.

use std::{
    fs,
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use legion_platform::{NativeFileSystem, NativeWatcherService};
use legion_project::WorkspaceActor;
use legion_protocol::{
    CanonicalPath, CapabilityNamespace, CorrelationId, PrincipalId, WorkspaceOpenRequest,
    WorkspaceTrustState,
};
use legion_security::{DenyByDefaultBroker, SecurityPolicy};

static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

fn create_temp_workspace(prefix: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!(
        "legion-project-{}-{}-{}",
        prefix,
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |value| value.as_millis() as u64)
            + TEMP_COUNTER.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir_all(&root).expect("create temp workspace");
    fs::canonicalize(root).expect("canonicalize temp workspace")
}

fn security_policy_for_root(root: &Path) -> SecurityPolicy {
    let mut policy = SecurityPolicy::default();
    policy.path_policy.readable_roots = vec![root.to_string_lossy().into_owned()];
    policy.path_policy.writable_roots = vec![root.to_string_lossy().into_owned()];
    policy
}

fn open_workspace_timed(
    root: &Path,
) -> (WorkspaceActor, legion_protocol::WorkspaceOpened, Duration) {
    let policy = security_policy_for_root(root);
    let actor = WorkspaceActor::new(
        Arc::new(NativeFileSystem),
        Arc::new(NativeWatcherService),
        DenyByDefaultBroker::new(policy, CapabilityNamespace("scale-tests".to_string())),
    );
    let t0 = Instant::now();
    let opened = actor
        .open_workspace(WorkspaceOpenRequest {
            correlation_id: CorrelationId(1),
            principal_id: PrincipalId("scale-test".to_string()),
            root_path: CanonicalPath(root.to_string_lossy().into_owned()),
            trust: Some(WorkspaceTrustState::Trusted),
        })
        .expect("open workspace");
    let elapsed = t0.elapsed();
    (actor, opened, elapsed)
}

#[test]
fn workspace_open_500_files_completes_within_budget() {
    let root = create_temp_workspace("scale-tree");

    // Create 10 directories with 50 files each = 500 files
    for dir_idx in 0..10 {
        let dir = root.join(format!("dir-{:03}", dir_idx));
        fs::create_dir_all(&dir).expect("create subdir");
        for file_idx in 0..50 {
            let file = dir.join(format!("file-{:04}.rs", file_idx));
            fs::write(&file, format!("// stub module {}\n", file_idx)).expect("write stub");
        }
    }

    let (_actor, opened, elapsed) = open_workspace_timed(&root);

    eprintln!(
        "Workspace open for 500 files took {:?} (workspace_id={:?})",
        elapsed, opened.workspace_id
    );

    assert!(
        elapsed.as_secs() < 10,
        "workspace open took {:?}, expected < 10s for 500 files",
        elapsed
    );

    let _ = fs::remove_dir_all(&root);
}

#[test]
#[ignore = "1000-file scale test - run explicitly"]
fn workspace_open_1000_files_completes_within_budget() {
    let root = create_temp_workspace("scale-tree-1k");

    // Create 20 directories with 50 files each = 1000 files
    for dir_idx in 0..20 {
        let dir = root.join(format!("dir-{:03}", dir_idx));
        fs::create_dir_all(&dir).expect("create subdir");
        for file_idx in 0..50 {
            let file = dir.join(format!("file-{:04}.rs", file_idx));
            fs::write(&file, format!("// stub module {}\n", file_idx)).expect("write stub");
        }
    }

    let (_actor, opened, elapsed) = open_workspace_timed(&root);

    eprintln!(
        "Workspace open for 1000 files took {:?} (workspace_id={:?})",
        elapsed, opened.workspace_id
    );

    assert!(
        elapsed.as_secs() < 10,
        "workspace open took {:?}, expected < 10s for 1000 files",
        elapsed
    );

    let _ = fs::remove_dir_all(&root);
}
