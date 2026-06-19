use std::{
    fs,
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::{SystemTime, UNIX_EPOCH},
};

use legion_platform::{NativeFileSystem, NativeWatcherService};
use legion_project::{SearchPattern, WorkspaceActor, WorkspaceSearchFilters, WorkspaceSearchQuery};
use legion_protocol::{
    CanonicalPath, CapabilityNamespace, CorrelationId, PrincipalId, WorkspaceOpenRequest,
    WorkspaceTrustState,
};
use legion_security::{DenyByDefaultBroker, SecurityPolicy};

static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

fn create_temp_workspace() -> PathBuf {
    let root = std::env::temp_dir().join(format!(
        "legion-project-harness-tools-{}-{}",
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

fn open_workspace(
    root: &Path,
    trust: WorkspaceTrustState,
) -> (WorkspaceActor, legion_protocol::WorkspaceOpened) {
    let policy = security_policy_for_root(root);
    let actor = WorkspaceActor::new(
        Arc::new(NativeFileSystem),
        Arc::new(NativeWatcherService),
        DenyByDefaultBroker::new(policy, CapabilityNamespace("harness-tools".to_string())),
    );
    let opened = actor
        .open_workspace(WorkspaceOpenRequest {
            correlation_id: CorrelationId(1),
            principal_id: PrincipalId("harness-test".to_string()),
            root_path: CanonicalPath(root.to_string_lossy().into_owned()),
            trust: Some(trust),
        })
        .expect("open workspace");
    (actor, opened)
}

fn path_ends_with(path: &str, suffix: &str) -> bool {
    path.replace('\\', "/").ends_with(suffix)
}

#[test]
fn grep_alias_matches_workspace_search() {
    let root = create_temp_workspace();
    fs::write(root.join("needle.txt"), "needle one\nneedle two\n").expect("write file");

    let (actor, opened) = open_workspace(&root, WorkspaceTrustState::Trusted);
    let query = WorkspaceSearchQuery {
        workspace_id: opened.workspace_id,
        pattern: SearchPattern::literal("needle", true, false).expect("literal search"),
        search_text: "needle".to_string(),
        filters: WorkspaceSearchFilters::default(),
        result_limit: 10,
        batch_size: 2,
        use_indexed_backend: false,
    };

    let search_report = actor
        .search_workspace_stream(query.clone(), |_| false)
        .expect("search should succeed");
    let grep_report = actor
        .grep_workspace_stream(query, |_| false)
        .expect("grep should succeed");

    assert_eq!(grep_report.hit_count, search_report.hit_count);
    assert_eq!(
        grep_report.omitted_hit_count,
        search_report.omitted_hit_count
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn glob_tool_returns_matching_paths() {
    let root = create_temp_workspace();
    fs::create_dir_all(root.join("src/nested")).expect("create nested dirs");
    fs::write(root.join("src/lib.rs"), "pub fn outer() {}\n").expect("write lib");
    fs::write(root.join("src/nested/mod.rs"), "pub fn inner() {}\n").expect("write mod");
    fs::write(root.join("notes.txt"), "not rust\n").expect("write notes");

    let (actor, opened) = open_workspace(&root, WorkspaceTrustState::Trusted);
    let matches = actor
        .glob_workspace_files(opened.workspace_id, "**/*.rs")
        .expect("glob should succeed");

    let paths: Vec<_> = matches
        .iter()
        .map(|identity| identity.canonical_path.0.clone())
        .collect();
    assert!(paths.iter().any(|path| path_ends_with(path, "src/lib.rs")));
    assert!(
        paths
            .iter()
            .any(|path| path_ends_with(path, "src/nested/mod.rs"))
    );
    assert!(!paths.iter().any(|path| path.ends_with("notes.txt")));
    let _ = fs::remove_dir_all(root);
}

#[test]
fn outline_tool_returns_rust_symbols() {
    let root = create_temp_workspace();
    fs::create_dir_all(root.join("src")).expect("create src dir");
    fs::write(
        root.join("src/lib.rs"),
        "mod inner {\n    pub fn nested() {}\n}\n\npub fn outer() {}\n",
    )
    .expect("write rust file");

    let (actor, opened) = open_workspace(&root, WorkspaceTrustState::Trusted);
    let outline = actor
        .outline_workspace_file(opened.workspace_id, "src/lib.rs")
        .expect("outline should succeed");

    assert!(!outline.is_empty());
    assert!(outline.iter().any(|row| row.label == "outer"));
    assert!(outline.iter().all(|row| row.schema_version == 1));
    let _ = fs::remove_dir_all(root);
}

#[test]
fn read_alias_returns_file_text() {
    let root = create_temp_workspace();
    fs::write(root.join("hello.txt"), "hello world\n").expect("write file");

    let (actor, opened) = open_workspace(&root, WorkspaceTrustState::Trusted);
    let text = actor
        .read_workspace_text(opened.workspace_id, "hello.txt")
        .expect("read should succeed");

    assert_eq!(text, "hello world\n");
    let _ = fs::remove_dir_all(root);
}
