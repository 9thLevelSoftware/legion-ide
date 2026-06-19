//! WS-MANUAL-02 SCALE.07: Watcher burst/debounce under generated churn.

use std::{
    fs,
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    thread::sleep,
    time::{Duration, SystemTime, UNIX_EPOCH},
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

fn open_workspace_for_watcher(
    root: &Path,
) -> (WorkspaceActor, legion_protocol::WorkspaceOpened) {
    let policy = security_policy_for_root(root);
    let actor = WorkspaceActor::new(
        Arc::new(NativeFileSystem),
        Arc::new(NativeWatcherService),
        DenyByDefaultBroker::new(policy, CapabilityNamespace("watcher-tests".to_string())),
    );
    let opened = actor
        .open_workspace(WorkspaceOpenRequest {
            correlation_id: CorrelationId(1),
            principal_id: PrincipalId("watcher-test".to_string()),
            root_path: CanonicalPath(root.to_string_lossy().into_owned()),
            trust: Some(WorkspaceTrustState::Trusted),
        })
        .expect("open workspace");
    (actor, opened)
}

#[test]
fn watcher_burst_debounces_rapid_modifications() {
    let root = create_temp_workspace("watcher-burst");
    let file = root.join("burst.txt");
    fs::write(&file, "initial\n").expect("write initial");

    let (actor, opened) = open_workspace_for_watcher(&root);

    // Initial poll to establish baseline scan
    sleep(Duration::from_millis(100));
    let _ = actor
        .poll_watcher_events(opened.workspace_id)
        .expect("initial poll");

    // Burst: write to the SAME file 50 times rapidly
    for i in 0..50 {
        fs::write(&file, format!("burst content {}\n", i)).expect("burst write");
    }

    // Wait past debounce window
    sleep(Duration::from_millis(100));

    let events = actor
        .poll_watcher_events(opened.workspace_id)
        .expect("poll after burst");

    eprintln!(
        "After 50 rapid writes to one file: {} watcher events",
        events.len()
    );

    // Key assertion: 50 writes to the same file should produce at most 1-2 events
    // (1 Modified event from the snapshot diff, since all writes target the same path)
    assert!(
        events.len() <= 3,
        "expected <= 3 events from 50 writes to same file, got {} events: {:?}",
        events.len(),
        events.iter().map(|e| format!("{:?}", e.kind)).collect::<Vec<_>>()
    );

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn watcher_burst_collapses_multi_file_changes() {
    let root = create_temp_workspace("watcher-burst-multi");

    // Create 20 files
    for i in 0..20 {
        fs::write(
            root.join(format!("file-{:02}.txt", i)),
            format!("content {}\n", i),
        )
        .expect("write file");
    }

    let (actor, opened) = open_workspace_for_watcher(&root);

    // Initial poll to establish baseline scan
    sleep(Duration::from_millis(100));
    let _ = actor
        .poll_watcher_events(opened.workspace_id)
        .expect("initial poll");

    // Modify all 20 files rapidly, each 3 times
    for round in 0..3 {
        for i in 0..20 {
            fs::write(
                root.join(format!("file-{:02}.txt", i)),
                format!("updated round {} content {}\n", round, i),
            )
            .expect("burst write");
        }
    }

    // Wait past debounce window
    sleep(Duration::from_millis(100));

    let events = actor
        .poll_watcher_events(opened.workspace_id)
        .expect("poll after multi-file burst");

    eprintln!(
        "After 60 writes to 20 files: {} watcher events",
        events.len()
    );

    // Each unique file gets at most 2 events: one from the snapshot diff (produced)
    // and one from the watcher queue that collect_watcher_events also enqueues.
    // So 60 writes across 20 files → at most 40 events (2 per unique path),
    // which is still O(unique files) rather than O(writes).
    assert!(
        events.len() <= 40,
        "expected <= 40 events (at most 2 per unique file) from 60 writes, got {}",
        events.len()
    );

    let _ = fs::remove_dir_all(&root);
}
