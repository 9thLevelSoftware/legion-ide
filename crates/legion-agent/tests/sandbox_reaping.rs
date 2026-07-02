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
