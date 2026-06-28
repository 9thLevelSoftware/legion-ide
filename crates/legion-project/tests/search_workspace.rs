use std::{
    fs,
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    thread::sleep,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
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
                search_text: "needle".to_string(),
                filters: WorkspaceSearchFilters::default(),
                result_limit: 10,
                batch_size: 2,
                use_indexed_backend: false,
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

#[test]
fn indexed_workspace_search_matches_live_scan() {
    let root = create_temp_workspace();
    fs::write(root.join("indexed.txt"), "needle one\nneedle two\n").expect("write file");
    fs::write(root.join("other.txt"), "irrelevant\n").expect("write file");

    let (actor, opened) = open_workspace(&root, WorkspaceTrustState::Trusted);
    let query = WorkspaceSearchQuery {
        workspace_id: opened.workspace_id,
        pattern: SearchPattern::literal("needle", true, false).expect("literal search"),
        search_text: "needle".to_string(),
        filters: WorkspaceSearchFilters::default(),
        result_limit: 10,
        batch_size: 2,
        use_indexed_backend: true,
    };

    let live = actor
        .search_workspace_stream(query.clone(), |_| true)
        .expect("live search should succeed");
    let indexed = actor
        .search_workspace_stream(query, |_| true)
        .expect("indexed search should succeed");

    assert_eq!(indexed.hit_count, live.hit_count);
    assert_eq!(indexed.omitted_hit_count, live.omitted_hit_count);
    assert_eq!(indexed.omitted_file_count, live.omitted_file_count);
    let _ = fs::remove_dir_all(root);
}

#[test]
fn indexed_workspace_search_refreshes_after_file_changes() {
    let root = create_temp_workspace();
    let file = root.join("refresh.txt");
    fs::write(&file, "needle one\n").expect("write file");

    let (actor, opened) = open_workspace(&root, WorkspaceTrustState::Trusted);
    let query = WorkspaceSearchQuery {
        workspace_id: opened.workspace_id,
        pattern: SearchPattern::literal("needle", true, false).expect("literal search"),
        search_text: "needle".to_string(),
        filters: WorkspaceSearchFilters::default(),
        result_limit: 10,
        batch_size: 2,
        use_indexed_backend: true,
    };

    let first = actor
        .search_workspace_stream(query.clone(), |_| true)
        .expect("first search");
    assert_eq!(first.hit_count, 1);

    fs::write(&file, "nothing to see here\n").expect("overwrite file");

    // Bounded poll loop instead of a single fixed sleep: drain watcher events
    // and re-run the indexed search until the modification is observed (index
    // invalidated to zero hits) or the overall deadline elapses.
    let deadline = Instant::now() + Duration::from_secs(5);
    let second = loop {
        let _ = actor
            .poll_watcher_events(opened.workspace_id)
            .expect("poll watcher events");
        let result = actor
            .search_workspace_stream(query.clone(), |_| true)
            .expect("second search");
        if result.hit_count == 0 || Instant::now() >= deadline {
            break result;
        }
        sleep(Duration::from_millis(20));
    };
    assert_eq!(second.hit_count, 0);
    let _ = fs::remove_dir_all(root);
}

#[test]
fn indexed_workspace_search_matches_live_scan_for_dozens_of_files() {
    // Non-ignored coverage for a larger (multi-file) fixture, complementing the
    // #[ignore]d 500-file benchmark below so default runs still validate indexed
    // vs live equivalence beyond a single file.
    let root = create_temp_workspace();
    for index in 0..32 {
        let body = if index % 3 == 0 {
            "needle here\nfiller\n"
        } else {
            "filler only\n"
        };
        fs::write(root.join(format!("file-{index:03}.txt")), body).expect("write file");
    }

    let (actor, opened) = open_workspace(&root, WorkspaceTrustState::Trusted);
    let live_query = WorkspaceSearchQuery {
        workspace_id: opened.workspace_id,
        pattern: SearchPattern::literal("needle", true, false).expect("literal search"),
        search_text: "needle".to_string(),
        filters: WorkspaceSearchFilters::default(),
        result_limit: 1000,
        batch_size: 8,
        use_indexed_backend: false,
    };
    let indexed_query = WorkspaceSearchQuery {
        use_indexed_backend: true,
        ..live_query.clone()
    };

    let live = actor
        .search_workspace_stream(live_query, |_| true)
        .expect("live search");
    let indexed = actor
        .search_workspace_stream(indexed_query, |_| true)
        .expect("indexed search");

    assert!(live.hit_count > 0, "fixture should produce matches");
    assert_eq!(indexed.hit_count, live.hit_count);
    assert_eq!(indexed.omitted_hit_count, live.omitted_hit_count);
    assert_eq!(indexed.omitted_file_count, live.omitted_file_count);
    let _ = fs::remove_dir_all(root);
}

#[test]
#[ignore]
fn indexed_workspace_search_benchmark_large_fixture() {
    let root = create_temp_workspace();
    let needle = "needle needle needle\n";
    for index in 0..500 {
        fs::write(root.join(format!("file-{index:04}.txt")), needle).expect("write file");
    }

    let (actor, opened) = open_workspace(&root, WorkspaceTrustState::Trusted);
    let live_query = WorkspaceSearchQuery {
        workspace_id: opened.workspace_id,
        pattern: SearchPattern::literal("needle", true, false).expect("literal search"),
        search_text: "needle".to_string(),
        filters: WorkspaceSearchFilters::default(),
        result_limit: 10,
        batch_size: 2,
        use_indexed_backend: false,
    };
    let indexed_query = WorkspaceSearchQuery {
        use_indexed_backend: true,
        ..live_query.clone()
    };

    let _warmup = actor
        .search_workspace_stream(indexed_query.clone(), |_| true)
        .expect("warm up indexed search");

    let live_start = Instant::now();
    let live = actor
        .search_workspace_stream(live_query, |_| true)
        .expect("live search");
    let live_elapsed = live_start.elapsed();

    let indexed_start = Instant::now();
    let indexed = actor
        .search_workspace_stream(indexed_query, |_| true)
        .expect("indexed search");
    let indexed_elapsed = indexed_start.elapsed();

    println!("live={:?} indexed={:?}", live_elapsed, indexed_elapsed);
    assert_eq!(indexed.hit_count, live.hit_count);
    let _ = fs::remove_dir_all(root);
}
