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
        "legion-project-search-workspace-{}-{}",
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
        DenyByDefaultBroker::new(policy, CapabilityNamespace("search-tests".to_string())),
    );
    let opened = actor
        .open_workspace(WorkspaceOpenRequest {
            correlation_id: CorrelationId(1),
            principal_id: PrincipalId("search-test".to_string()),
            root_path: CanonicalPath(root.to_string_lossy().into_owned()),
            trust: Some(trust),
        })
        .expect("open workspace");
    (actor, opened)
}

#[test]
fn regex_search_pattern_matches_expected_line() {
    let pattern = SearchPattern::regex("^alpha$", true, false).expect("regex search");
    assert_eq!(pattern.find_ranges("alpha").len(), 1);
}

#[test]
fn workspace_search_stream_emits_batches_and_cancels() {
    let root = create_temp_workspace();
    fs::write(root.join("batch.txt"), "needle one\nneedle two\n").expect("write file");

    let (actor, opened) = open_workspace(&root, WorkspaceTrustState::Trusted);
    let mut batches = Vec::new();
    let report = actor
        .search_workspace_stream(
            WorkspaceSearchQuery {
                workspace_id: opened.workspace_id,
                pattern: SearchPattern::literal("needle", true, false).expect("literal search"),
                filters: WorkspaceSearchFilters::default(),
                result_limit: 10,
                batch_size: 2,
            },
            |batch| {
                batches.push(batch);
                false
            },
        )
        .expect("search should complete");

    assert!(report.cancelled);
    assert_eq!(report.hit_count, 2);
    assert_eq!(batches.len(), 1);
    assert_eq!(batches[0].hits.len(), 2);
    assert_eq!(batches[0].omitted_hit_count, 0);
    assert_eq!(report.omitted_hit_count, 0);
    let _ = fs::remove_dir_all(root);
}
