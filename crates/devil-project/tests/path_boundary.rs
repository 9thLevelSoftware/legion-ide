use std::path::{Path, PathBuf};
use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};

static TEMP_ROOT_COUNTER: AtomicU64 = AtomicU64::new(0);

use devil_platform::{
    FileSystemFingerprint, FileSystemMetadata, FileSystemService, NativeFileSystem,
    NativeWatcherService, PathNormalizationService, PlatformError,
};
use devil_project::{WorkspaceActor, WorkspaceError, WorkspaceSaveRequest};
use devil_protocol::{
    BufferVersion, CanonicalPath, CapabilityId, CapabilityNamespace, CausalityId, CorrelationId,
    EventSequence, PrincipalId, ProposalId, ProposalResponse, ProposalStaleReason, SnapshotId,
    WatcherEvent, WatcherEventKind, WorkspaceDiscoveryChangeKind, WorkspaceDiscoveryDecision,
    WorkspaceDiscoverySkipReason, WorkspaceOpenRequest, WorkspacePort, WorkspaceRequest,
    WorkspaceResponse, WorkspaceTrustState,
};
use devil_security::{DenyByDefaultBroker, SecurityPolicy};
use uuid::Uuid;

#[derive(Debug)]
struct CountingFileSystem {
    inner: NativeFileSystem,
    write_attempts: Arc<AtomicU64>,
}

impl CountingFileSystem {
    fn new(write_attempts: Arc<AtomicU64>) -> Self {
        Self {
            inner: NativeFileSystem,
            write_attempts,
        }
    }
}

impl PathNormalizationService for CountingFileSystem {
    fn normalize_path(&self, path: &Path) -> Result<PathBuf, PlatformError> {
        self.inner.normalize_path(path)
    }

    fn canonicalize_path(&self, path: &Path) -> Result<PathBuf, PlatformError> {
        self.inner.canonicalize_path(path)
    }

    fn is_within_base(&self, base: &Path, candidate: &Path) -> Result<bool, PlatformError> {
        self.inner.is_within_base(base, candidate)
    }
}

impl FileSystemService for CountingFileSystem {
    fn read_text_file(&self, path: &Path) -> Result<String, PlatformError> {
        self.inner.read_text_file(path)
    }

    fn write_text_file(&self, path: &Path, text: &str) -> Result<(), PlatformError> {
        self.write_attempts.fetch_add(1, Ordering::SeqCst);
        self.inner.write_text_file(path, text)
    }

    fn write_text_file_atomic(&self, path: &Path, text: &str) -> Result<(), PlatformError> {
        self.write_attempts.fetch_add(1, Ordering::SeqCst);
        self.inner.write_text_file_atomic(path, text)
    }

    fn read_metadata(&self, path: &Path) -> Result<FileSystemMetadata, PlatformError> {
        self.inner.read_metadata(path)
    }

    fn read_fingerprint(&self, path: &Path) -> Result<FileSystemFingerprint, PlatformError> {
        self.inner.read_fingerprint(path)
    }

    fn stable_hash(&self, bytes: &[u8]) -> String {
        self.inner.stable_hash(bytes)
    }

    fn stable_hash_file(&self, path: &Path) -> Result<String, PlatformError> {
        self.inner.stable_hash_file(path)
    }

    fn modified_timestamp(&self, path: &Path) -> Result<Option<u64>, PlatformError> {
        self.inner.modified_timestamp(path)
    }

    fn file_length(&self, path: &Path) -> Result<u64, PlatformError> {
        self.inner.file_length(path)
    }

    fn list_directory(&self, path: &Path) -> Result<Vec<PathBuf>, PlatformError> {
        self.inner.list_directory(path)
    }
}

fn security_policy_for_root(root: &Path) -> SecurityPolicy {
    let mut policy = SecurityPolicy::default();
    policy.path_policy.readable_roots = vec![root.to_string_lossy().into_owned()];
    policy.path_policy.writable_roots = vec![root.to_string_lossy().into_owned()];
    policy
}

fn security_policy_for_root_with_limits(
    root: &Path,
    path_max_write_bytes: usize,
    file_max_bytes_per_write: usize,
) -> SecurityPolicy {
    let mut policy = security_policy_for_root(root);
    policy.path_policy.max_write_bytes = path_max_write_bytes;
    policy.file_write_policy.max_bytes_per_write = file_max_bytes_per_write;
    policy
}

fn open_workspace(
    root: &Path,
    trust: WorkspaceTrustState,
) -> (WorkspaceActor, devil_protocol::WorkspaceOpened) {
    let policy = security_policy_for_root(root);

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

fn open_workspace_with_counting_fs(
    root: &Path,
    trust: WorkspaceTrustState,
    policy: SecurityPolicy,
    write_attempts: Arc<AtomicU64>,
) -> (WorkspaceActor, devil_protocol::WorkspaceOpened) {
    let actor = WorkspaceActor::new(
        Arc::new(CountingFileSystem::new(write_attempts)),
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

#[allow(clippy::result_large_err)]
fn save_new_file(
    actor: &WorkspaceActor,
    workspace_id: devil_protocol::WorkspaceId,
    path: impl AsRef<str>,
    text: impl Into<String>,
) -> Result<devil_project::WorkspaceSaveApplied, ProposalResponse> {
    let opened = actor
        .open_new_file_text(workspace_id, path.as_ref())
        .expect("open safe new-file precondition");
    let text = text.into();
    actor.save_file_with_proposal(WorkspaceSaveRequest {
        workspace_id,
        proposal_id: ProposalId(1),
        principal: PrincipalId("tester".to_string()),
        required_capability: CapabilityId("fs.write".to_string()),
        file_id: opened.identity.file_id,
        path: opened.identity.canonical_path,
        expected_fingerprint: opened.fingerprint,
        expected_file_content_version: opened.file_content_version,
        expected_workspace_generation: opened.workspace_generation,
        buffer_version: BufferVersion(1),
        snapshot_id: SnapshotId(1),
        payload_byte_len: text.len() as u64,
        correlation_id: CorrelationId(1),
        causality_id: CausalityId(Uuid::now_v7()),
        text,
    })
}

#[test]
fn semantic_discovery_snapshot_reports_safe_workspace_policy_outcomes() {
    let root = create_temp_workspace();
    std::fs::create_dir_all(root.join("src")).expect("create src");
    std::fs::create_dir_all(root.join("target")).expect("create target");
    std::fs::create_dir_all(root.join("node_modules")).expect("create vendor dir");
    std::fs::write(root.join("src/lib.rs"), "pub fn accepted() {}\n").expect("accepted file");
    std::fs::write(root.join("target/generated.rs"), "pub fn generated() {}\n")
        .expect("generated file");
    std::fs::write(
        root.join("node_modules/vendor.rs"),
        "pub fn vendored() {}\n",
    )
    .expect("vendored file");
    std::fs::write(root.join("image.png"), [0_u8, 1, 2, 3]).expect("binary file");
    std::fs::write(root.join(".gitignore"), "target\n").expect("ignore file");

    let (actor, opened) = open_workspace(&root, WorkspaceTrustState::Trusted);
    let snapshot = match actor
        .handle(WorkspaceRequest::ReadSemanticDiscoverySnapshot(
            opened.workspace_id,
        ))
        .expect("workspace port should expose workspace-authored discovery snapshot")
    {
        WorkspaceResponse::SemanticDiscoverySnapshot(snapshot) => snapshot,
        other => panic!("unexpected workspace response: {other:?}"),
    };

    assert!(snapshot.records.iter().any(|record| {
        record.display_path.as_deref() == Some("src/lib.rs")
            && record.policy.decision == WorkspaceDiscoveryDecision::ContentAllowed
            && record.identity.is_some()
    }));
    assert!(snapshot.records.iter().any(|record| {
        record.display_path.as_deref() == Some("target")
            && record.policy.skip_reason == Some(WorkspaceDiscoverySkipReason::Generated)
            && record.policy.metadata_only
    }));
    assert!(snapshot.records.iter().any(|record| {
        record.display_path.as_deref() == Some("node_modules")
            && record.policy.skip_reason == Some(WorkspaceDiscoverySkipReason::Vendored)
            && record.policy.metadata_only
    }));
    assert!(snapshot.records.iter().any(|record| {
        record.display_path.as_deref() == Some("image.png")
            && record.policy.skip_reason == Some(WorkspaceDiscoverySkipReason::Binary)
            && record.policy.metadata_only
    }));
    assert!(snapshot.records.iter().any(|record| {
        record.display_path.as_deref() == Some(".gitignore")
            && record.policy.skip_reason == Some(WorkspaceDiscoverySkipReason::Ignored)
            && record.policy.metadata_only
    }));

    let (untrusted_actor, untrusted_opened) = open_workspace(&root, WorkspaceTrustState::Untrusted);
    let denied_snapshot = match untrusted_actor
        .handle(WorkspaceRequest::ReadSemanticDiscoverySnapshot(
            untrusted_opened.workspace_id,
        ))
        .expect("workspace port should expose untrusted discovery snapshot")
    {
        WorkspaceResponse::SemanticDiscoverySnapshot(snapshot) => snapshot,
        other => panic!("unexpected workspace response: {other:?}"),
    };
    assert!(denied_snapshot.records.iter().any(|record| {
        record.display_path.as_deref() == Some("src/lib.rs")
            && record.policy.skip_reason == Some(WorkspaceDiscoverySkipReason::PolicyDenied)
            && record.policy.metadata_only
    }));

    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn semantic_discovery_delta_reports_external_deleted_and_changed_outcomes() {
    let root = create_temp_workspace();
    std::fs::create_dir_all(root.join("src")).expect("create src");
    let changed = root.join("src/lib.rs");
    let deleted = root.join("src/deleted.rs");
    std::fs::write(&changed, "pub fn changed() {}\n").expect("changed seed");
    std::fs::write(&deleted, "pub fn deleted() {}\n").expect("deleted seed");

    let (actor, opened) = open_workspace(&root, WorkspaceTrustState::Trusted);
    std::fs::remove_file(&deleted).expect("delete after open");
    std::fs::write(&changed, "pub fn changed_again() {}\n").expect("change after open");
    let outside = root
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join("outside-discovery.rs");

    let events = vec![
        WatcherEvent {
            workspace_id: opened.workspace_id,
            kind: WatcherEventKind::Modified,
            path: CanonicalPath(changed.to_string_lossy().into_owned()),
            old_path: None,
            sequence: EventSequence(1),
        },
        WatcherEvent {
            workspace_id: opened.workspace_id,
            kind: WatcherEventKind::Deleted,
            path: CanonicalPath(deleted.to_string_lossy().into_owned()),
            old_path: None,
            sequence: EventSequence(2),
        },
        WatcherEvent {
            workspace_id: opened.workspace_id,
            kind: WatcherEventKind::Created,
            path: CanonicalPath(outside.to_string_lossy().into_owned()),
            old_path: None,
            sequence: EventSequence(3),
        },
    ];
    let delta = match actor
        .handle(WorkspaceRequest::BuildSemanticDiscoveryDelta {
            workspace_id: opened.workspace_id,
            events,
        })
        .expect("workspace port should expose workspace-authored discovery delta")
    {
        WorkspaceResponse::SemanticDiscoveryDelta(delta) => delta,
        other => panic!("unexpected workspace response: {other:?}"),
    };

    assert!(delta.records.iter().any(|record| {
        record.display_path.as_deref() == Some("src/lib.rs")
            && record.change_kind == Some(WorkspaceDiscoveryChangeKind::Changed)
            && record.policy.decision == WorkspaceDiscoveryDecision::ContentAllowed
    }));
    assert!(delta.records.iter().any(|record| {
        record.display_path.as_deref() == Some("src/deleted.rs")
            && record.change_kind == Some(WorkspaceDiscoveryChangeKind::Deleted)
            && record.policy.skip_reason == Some(WorkspaceDiscoverySkipReason::Deleted)
    }));
    assert!(delta.records.iter().any(|record| {
        record.policy.skip_reason == Some(WorkspaceDiscoverySkipReason::External)
            && record.policy.decision == WorkspaceDiscoveryDecision::Excluded
    }));

    let _ = std::fs::remove_dir_all(root);
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
        .open_new_file_text(opened.workspace_id, escape)
        .expect_err("new-file open should fail");
    assert!(matches!(write_err, WorkspaceError::PathOutsideRoot { .. }));

    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn nonexistent_child_path_inside_root_is_allowed() {
    let root = create_temp_workspace();
    let (actor, opened) = open_workspace(&root, WorkspaceTrustState::Trusted);

    save_new_file(&actor, opened.workspace_id, "new/deep/file.txt", "ok")
        .expect("write nested child");

    let text = actor
        .read_file_text(opened.workspace_id, "new/deep/file.txt")
        .expect("read nested child");
    assert_eq!(text, "ok");

    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn stale_save_preserves_disk_content_and_returns_typed_response() {
    let root = create_temp_workspace();
    let (actor, opened) = open_workspace(&root, WorkspaceTrustState::Trusted);
    let target = root.join("stale-save.txt");
    std::fs::write(&target, "seed").expect("seed file");

    let opened_file = actor
        .open_existing_file_text(opened.workspace_id, target.to_string_lossy())
        .expect("open existing file");
    std::fs::write(&target, "external").expect("external overwrite");

    let response = actor
        .save_file_with_proposal(WorkspaceSaveRequest {
            workspace_id: opened.workspace_id,
            proposal_id: ProposalId(2),
            principal: PrincipalId("tester".to_string()),
            required_capability: CapabilityId("fs.write".to_string()),
            file_id: opened_file.identity.file_id,
            path: opened_file.identity.canonical_path,
            expected_fingerprint: opened_file.fingerprint,
            expected_file_content_version: opened_file.file_content_version,
            expected_workspace_generation: opened_file.workspace_generation,
            buffer_version: BufferVersion(2),
            snapshot_id: SnapshotId(2),
            payload_byte_len: "buffer".len() as u64,
            correlation_id: CorrelationId(2),
            causality_id: CausalityId(Uuid::now_v7()),
            text: "buffer".to_string(),
        })
        .expect_err("external overwrite should reject stale proposal");

    assert_eq!(
        std::fs::read_to_string(&target).expect("disk content"),
        "external"
    );
    match response {
        ProposalResponse::Stale { stale, .. } => {
            assert_eq!(
                stale.reason,
                ProposalStaleReason::FileContentVersionMismatch
            );
        }
        ProposalResponse::Conflict { .. } => {}
        other => panic!("expected stale or conflict response, got {other:?}"),
    }

    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn oversized_save_denied_by_path_limit_before_atomic_write() {
    let root = create_temp_workspace();
    let target = root.join("path-limit.txt");
    let write_attempts = Arc::new(AtomicU64::new(0));
    let policy = security_policy_for_root_with_limits(&root, 4, 64);
    let (actor, opened) = open_workspace_with_counting_fs(
        &root,
        WorkspaceTrustState::Trusted,
        policy,
        write_attempts.clone(),
    );

    let response = save_new_file(
        &actor,
        opened.workspace_id,
        target.to_string_lossy(),
        "12345",
    )
    .expect_err("oversized save should be denied");

    assert_eq!(write_attempts.load(Ordering::SeqCst), 0);
    assert!(!target.exists());
    match response {
        ProposalResponse::Denied { transition, .. } => {
            let diagnostic = transition
                .diagnostics
                .first()
                .expect("write-size denial diagnostic");
            assert!(diagnostic.message.contains("write-size limit 4 bytes"));
        }
        other => panic!("expected denied response, got {other:?}"),
    }

    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn oversized_save_denied_by_file_write_limit_before_atomic_write() {
    let root = create_temp_workspace();
    let target = root.join("file-limit.txt");
    let write_attempts = Arc::new(AtomicU64::new(0));
    let policy = security_policy_for_root_with_limits(&root, 64, 4);
    let (actor, opened) = open_workspace_with_counting_fs(
        &root,
        WorkspaceTrustState::Trusted,
        policy,
        write_attempts.clone(),
    );

    let response = save_new_file(
        &actor,
        opened.workspace_id,
        target.to_string_lossy(),
        "12345",
    )
    .expect_err("oversized save should be denied by file write limit");

    assert_eq!(write_attempts.load(Ordering::SeqCst), 0);
    assert!(!target.exists());
    match response {
        ProposalResponse::Denied { transition, .. } => {
            let diagnostic = transition
                .diagnostics
                .first()
                .expect("write-size denial diagnostic");
            assert!(diagnostic.message.contains("write-size limit 4 bytes"));
        }
        other => panic!("expected denied response, got {other:?}"),
    }

    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn in_limit_save_uses_stricter_limit_and_writes_once() {
    let root = create_temp_workspace();
    let target = root.join("in-limit.txt");
    let write_attempts = Arc::new(AtomicU64::new(0));
    let policy = security_policy_for_root_with_limits(&root, 64, 4);
    let (actor, opened) = open_workspace_with_counting_fs(
        &root,
        WorkspaceTrustState::Trusted,
        policy,
        write_attempts.clone(),
    );

    save_new_file(
        &actor,
        opened.workspace_id,
        target.to_string_lossy(),
        "1234",
    )
    .expect("payload at effective write limit should save");

    assert_eq!(write_attempts.load(Ordering::SeqCst), 1);
    assert_eq!(
        std::fs::read_to_string(&target).expect("saved content"),
        "1234"
    );

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
    save_new_file(&actor, opened.workspace_id, &candidate, "case-ok")
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

    save_new_file(&actor, opened.workspace_id, &long_prefixed, "long")
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
