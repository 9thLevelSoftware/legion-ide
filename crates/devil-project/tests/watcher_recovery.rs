use std::path::{Path, PathBuf};
use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};

static TEMP_ROOT_COUNTER: AtomicU64 = AtomicU64::new(0);

use devil_observability::{InMemoryEventSink, SharedEventSink};
use devil_platform::{NativeFileSystem, PlatformError, WatcherService};
use devil_project::WorkspaceActor;
use devil_protocol::{
    CanonicalPath, CapabilityNamespace, CorrelationId, EventEnvelope, PrincipalId, WatcherEvent,
    WatcherEventKind, WorkspaceId, WorkspaceOpenRequest, WorkspaceTrustState,
};
use devil_security::{DenyByDefaultBroker, SecurityPolicy};
use uuid::Uuid;

fn create_temp_workspace() -> PathBuf {
    let root = std::env::temp_dir().join(format!(
        "devil-project-watcher-recovery-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |value| value.as_millis() as u64)
            + TEMP_ROOT_COUNTER.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&root).expect("create temp workspace");
    std::fs::canonicalize(root).expect("canonicalize temp workspace")
}

struct OverflowThenOkWatcher {
    calls: std::sync::Mutex<u32>,
}

impl OverflowThenOkWatcher {
    fn new() -> Self {
        Self {
            calls: std::sync::Mutex::new(0),
        }
    }
}

impl WatcherService for OverflowThenOkWatcher {
    fn snapshot(
        &self,
        workspace_id: WorkspaceId,
        path: &Path,
    ) -> Result<Vec<WatcherEvent>, PlatformError> {
        let mut calls = self.calls.lock().expect("lock calls");
        *calls += 1;

        if *calls == 1 {
            return Err(PlatformError::WatcherOverflow {
                path: path.to_path_buf(),
                context: "synthetic overflow".to_string(),
            });
        }

        Ok(vec![WatcherEvent {
            workspace_id,
            kind: WatcherEventKind::Modified,
            path: CanonicalPath(path.to_string_lossy().into_owned()),
            old_path: None,
            sequence: devil_protocol::EventSequence(*calls as u64),
        }])
    }
}

fn assert_non_zero_core_ids(event: &EventEnvelope) {
    assert_ne!(event.correlation_id.0, 0, "correlation id must be non-zero");
    assert_ne!(
        event.causality_id.0,
        Uuid::nil(),
        "causality id must be non-zero"
    );
    assert_ne!(event.sequence.0, 0, "event sequence must be non-zero");
}

#[test]
fn watcher_recovery_overflow_then_rescan_emits_recovery_event() {
    let root = create_temp_workspace();
    let file = root.join("seed.txt");
    std::fs::write(&file, "seed").expect("seed file");

    let mut policy = SecurityPolicy::default();
    policy.path_policy.readable_roots = vec![root.to_string_lossy().into_owned()];
    policy.path_policy.writable_roots = vec![root.to_string_lossy().into_owned()];

    let sink = InMemoryEventSink::new();
    let actor = WorkspaceActor::with_event_sink(
        Arc::new(NativeFileSystem),
        Arc::new(OverflowThenOkWatcher::new()),
        DenyByDefaultBroker::new(
            policy,
            CapabilityNamespace("watcher-recovery-test".to_string()),
        ),
        Box::new(SharedEventSink::new(sink.clone())),
    );

    let opened = actor
        .open_workspace(WorkspaceOpenRequest {
            correlation_id: CorrelationId(1),
            principal_id: PrincipalId("tester".to_string()),
            root_path: CanonicalPath(root.to_string_lossy().into_owned()),
            trust: Some(WorkspaceTrustState::Trusted),
        })
        .expect("open workspace");

    let first = actor
        .poll_watcher_events(opened.workspace_id)
        .expect("first watcher poll");
    assert!(
        first
            .iter()
            .any(|event| matches!(event.kind, WatcherEventKind::Overflow)),
        "expected overflow event"
    );

    std::thread::sleep(std::time::Duration::from_millis(80));

    let second = actor
        .poll_watcher_events(opened.workspace_id)
        .expect("second watcher poll");
    assert!(
        second
            .iter()
            .any(|event| matches!(event.kind, WatcherEventKind::Modified)),
        "expected recovery completion event"
    );

    let events = sink.events().expect("watcher observability events");
    let names = events
        .iter()
        .map(|event| event.event.as_str())
        .collect::<Vec<_>>();
    assert_eq!(
        names,
        vec!["workspace.watcher_overflow", "workspace.watcher_recovery"]
    );
    for event in &events {
        assert_non_zero_core_ids(event);
    }

    let _ = std::fs::remove_dir_all(root);
}
