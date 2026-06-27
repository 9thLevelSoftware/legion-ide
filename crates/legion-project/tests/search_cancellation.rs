//! SCALE.08: Search cancellation resource-cleanup tests.
//!
//! Verifies that:
//!   1. Cancellation stops iteration and leaves the workspace usable.
//!   2. Returning `false` on the very first batch produces cancelled==true.
//!   3. A search that runs to completion sets cancelled==false.

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

fn open_workspace(root: &Path) -> (WorkspaceActor, legion_protocol::WorkspaceOpened) {
    let policy = security_policy_for_root(root);
    let actor = WorkspaceActor::new(
        Arc::new(NativeFileSystem),
        Arc::new(NativeWatcherService),
        DenyByDefaultBroker::new(policy, CapabilityNamespace("cancel-tests".to_string())),
    );
    let opened = actor
        .open_workspace(WorkspaceOpenRequest {
            correlation_id: CorrelationId(1),
            principal_id: PrincipalId("cancel-test".to_string()),
            root_path: CanonicalPath(root.to_string_lossy().into_owned()),
            trust: Some(WorkspaceTrustState::Trusted),
        })
        .expect("open workspace");
    (actor, opened)
}

/// SCALE.08 – cancellation stops iteration and subsequent operations succeed.
///
/// Scenario:
///   • 50 files, each containing "needle"; batch_size=5 so iteration spans many batches.
///   • The callback returns `false` after the first batch → search is cancelled.
///   • After cancellation:
///     - A fresh search (callback always returns `true`) completes normally.
///     - `read_file_text` on a known file works without error.
#[test]
fn search_cancellation_stops_iteration_and_allows_subsequent_operations() {
    let root = create_temp_workspace("cancel-cleanup");

    // Create 50 files each with two "needle" matches so batches fill up.
    for i in 0..50 {
        fs::write(
            root.join(format!("file-{:04}.txt", i)),
            "needle line one\nneedle line two\n",
        )
        .expect("write file");
    }

    let (actor, opened) = open_workspace(&root);
    let workspace_id = opened.workspace_id;

    // --- First search: cancel after first batch ---
    let mut batches_received: usize = 0;
    let report = actor
        .search_workspace_stream(
            WorkspaceSearchQuery {
                workspace_id,
                pattern: SearchPattern::literal("needle", true, false)
                    .expect("literal search pattern"),
                search_text: "needle".to_string(),
                filters: WorkspaceSearchFilters::default(),
                result_limit: 100,
                batch_size: 5,
                use_indexed_backend: false,
            },
            |_batch| {
                batches_received += 1;
                false // cancel after the very first batch
            },
        )
        .expect("search should complete even when cancelled");

    assert!(
        report.cancelled,
        "report.cancelled should be true after early callback return"
    );
    assert_eq!(
        batches_received, 1,
        "only one batch should have been delivered before cancellation"
    );

    eprintln!(
        "Cancelled search: hit_count={} batches_received={}",
        report.hit_count, batches_received
    );

    // --- Subsequent search: must work normally after cancellation ---
    let mut all_hits: usize = 0;
    let report2 = actor
        .search_workspace_stream(
            WorkspaceSearchQuery {
                workspace_id,
                pattern: SearchPattern::literal("needle", true, false)
                    .expect("literal search pattern"),
                search_text: "needle".to_string(),
                filters: WorkspaceSearchFilters::default(),
                result_limit: 200,
                batch_size: 5,
                use_indexed_backend: false,
            },
            |batch| {
                all_hits += batch.hits.len();
                true // keep going – let it complete
            },
        )
        .expect("subsequent search after cancellation should succeed");

    assert!(
        !report2.cancelled,
        "subsequent search should complete without cancellation"
    );
    assert!(
        report2.hit_count > 0,
        "subsequent search should find hits (got {})",
        report2.hit_count
    );
    assert_eq!(
        all_hits, report2.hit_count,
        "accumulated hit count should match report"
    );

    eprintln!(
        "Subsequent search: hit_count={} cancelled={}",
        report2.hit_count, report2.cancelled
    );

    // --- read_file_text: must work after cancellation ---
    let first_file = root.join("file-0000.txt");
    let content = actor
        .read_file_text(workspace_id, first_file.to_string_lossy().as_ref())
        .expect("read_file_text should work after cancellation");
    assert!(
        content.contains("needle"),
        "file content should contain 'needle', got: {:?}",
        content
    );

    let _ = fs::remove_dir_all(&root);
}

/// SCALE.08 – returning `false` immediately produces cancelled==true.
///
/// When the very first callback invocation returns `false` the search must
/// set cancelled==true and return a valid (possibly empty) report.
#[test]
fn search_cancellation_immediate_returns_cancelled() {
    let root = create_temp_workspace("cancel-immediate");

    for i in 0..10 {
        fs::write(root.join(format!("file-{:04}.txt", i)), "needle here\n").expect("write file");
    }

    let (actor, opened) = open_workspace(&root);

    let report = actor
        .search_workspace_stream(
            WorkspaceSearchQuery {
                workspace_id: opened.workspace_id,
                pattern: SearchPattern::literal("needle", true, false)
                    .expect("literal search pattern"),
                search_text: "needle".to_string(),
                filters: WorkspaceSearchFilters::default(),
                result_limit: 100,
                batch_size: 5,
                use_indexed_backend: false,
            },
            |_batch| false, // cancel immediately
        )
        .expect("search should return a report even when immediately cancelled");

    assert!(
        report.cancelled,
        "report.cancelled must be true when callback returns false on first batch"
    );

    eprintln!(
        "Immediate-cancel search: hit_count={} cancelled={}",
        report.hit_count, report.cancelled
    );

    let _ = fs::remove_dir_all(&root);
}

/// SCALE.08 – a fully-completed search sets cancelled==false.
///
/// Baseline sanity check: when the callback always returns `true`, the
/// search runs to completion and cancelled==false with all hits reported.
#[test]
fn search_full_completion_sets_cancelled_false() {
    let root = create_temp_workspace("cancel-full");

    let file_count: usize = 10;
    for i in 0..file_count {
        fs::write(root.join(format!("file-{:04}.txt", i)), "needle content\n").expect("write file");
    }

    let (actor, opened) = open_workspace(&root);

    let mut total_hits: usize = 0;
    let report = actor
        .search_workspace_stream(
            WorkspaceSearchQuery {
                workspace_id: opened.workspace_id,
                pattern: SearchPattern::literal("needle", true, false)
                    .expect("literal search pattern"),
                search_text: "needle".to_string(),
                filters: WorkspaceSearchFilters::default(),
                result_limit: 100,
                batch_size: 5,
                use_indexed_backend: false,
            },
            |batch| {
                total_hits += batch.hits.len();
                true
            },
        )
        .expect("full search should succeed");

    assert!(
        !report.cancelled,
        "report.cancelled must be false when search completes normally"
    );
    assert_eq!(
        report.hit_count, file_count,
        "expected one hit per file (got {}), report: {:?}",
        report.hit_count, report.cancelled
    );
    assert_eq!(
        total_hits, file_count,
        "accumulated hits from batches should match file count"
    );

    eprintln!(
        "Full-completion search: hit_count={} cancelled={}",
        report.hit_count, report.cancelled
    );

    let _ = fs::remove_dir_all(&root);
}
