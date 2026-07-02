use legion_agent::reap_orphaned_sandboxes;
use std::fs;
use std::path::PathBuf;

fn temp_root(tag: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!("legion-reap-{tag}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).expect("create temp root");
    root
}

#[test]
fn reap_removes_orphans_and_preserves_active_and_unrelated() {
    let root = temp_root("basic");
    fs::create_dir_all(root.join("task-orphan-1")).unwrap();
    fs::write(root.join("task-orphan-1/marker.txt"), "stale").unwrap();
    fs::create_dir_all(root.join("task-active-1")).unwrap();
    fs::create_dir_all(root.join("not-a-task-dir")).unwrap();

    let removed = reap_orphaned_sandboxes(&root, &["active-1"]).expect("reap succeeds");

    assert_eq!(removed.len(), 1);
    assert!(removed[0].ends_with("task-orphan-1"));
    assert!(!root.join("task-orphan-1").exists(), "orphan removed");
    assert!(root.join("task-active-1").exists(), "active lane preserved");
    assert!(
        root.join("not-a-task-dir").exists(),
        "non-task dirs untouched"
    );

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn reap_on_missing_root_is_a_noop() {
    let root = temp_root("missing").join("does-not-exist");
    let removed = reap_orphaned_sandboxes(&root, &[]).expect("noop on missing root");
    assert!(removed.is_empty());
}

#[test]
fn reap_skips_sandbox_with_locked_lease_and_removes_it_once_released() {
    let root = temp_root("leased");
    fs::create_dir_all(root.join("task-live-1")).unwrap();
    fs::write(root.join("task-live-1/marker.txt"), "live").unwrap();
    let lease_path = root.join("task-live-1.lock");

    // Simulate the owning process/orchestrator holding an exclusive lease.
    // A same-process second `File::open` yields a distinct file
    // description, so the reaper's `try_lock` on its own handle genuinely
    // contends with this one (this mirrors real cross-process contention).
    let holder = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(&lease_path)
        .expect("create lease file");
    holder.try_lock().expect("test holds the lease");

    let removed = reap_orphaned_sandboxes(&root, &[]).expect("reap succeeds");
    assert!(
        removed.is_empty(),
        "locked lease must protect its sandbox from reaping"
    );
    assert!(
        root.join("task-live-1").exists(),
        "live sandbox must survive while its lease is held"
    );
    assert!(lease_path.exists(), "lock file must survive too");

    // Release the lease and re-run: the now-orphaned sandbox must be reaped
    // along with its lock file.
    drop(holder);
    let removed = reap_orphaned_sandboxes(&root, &[]).expect("reap succeeds after release");
    assert_eq!(removed.len(), 1);
    assert!(removed[0].ends_with("task-live-1"));
    assert!(
        !root.join("task-live-1").exists(),
        "sandbox removed once lease is released"
    );
    assert!(
        !lease_path.exists(),
        "lock file removed alongside its sandbox"
    );

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn reap_still_removes_legacy_sandboxes_without_a_lease_file() {
    let root = temp_root("legacy");
    fs::create_dir_all(root.join("task-legacy-1")).unwrap();
    fs::write(root.join("task-legacy-1/marker.txt"), "no lease").unwrap();

    let removed = reap_orphaned_sandboxes(&root, &[]).expect("reap succeeds");

    assert_eq!(removed.len(), 1);
    assert!(removed[0].ends_with("task-legacy-1"));
    assert!(
        !root.join("task-legacy-1").exists(),
        "legacy sandbox without a lease file must still be reaped"
    );

    let _ = fs::remove_dir_all(&root);
}
