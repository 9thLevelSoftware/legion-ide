//! Project model: workspace, file tree, file watcher, and trust-aware VFS resolution.

#![warn(missing_docs)]

use std::collections::{HashMap, HashSet, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use devil_observability::{
    NoopEventSink, conflict_created_event, fallback_denied_event, open_file_read_failure_event,
    security_denial_event, stale_proposal_rejected_event, watcher_recovery_event,
};
use devil_platform::{
    FileSystemEntryKind, FileSystemMetadata, FileSystemService, PathNormalizationService,
    PlatformError, WatcherService,
};
use devil_protocol::{
    BufferVersion, CanonicalPath, CapabilityId, CausalityId, CorrelationId, EventSequence,
    EventSinkPort, EventSinkRequest, FileConflictContext, FileConflictLifecycleState,
    FileConflictReason, FileConflictState, FileContentVersion,
    FileFingerprint as ProtocolFileFingerprint, FileId, FileIdentity, FileKind, FileMetadata,
    FileTreeDelta, FileTreeDeltaOp, FileTreeNode, PrincipalId, ProjectId, ProposalDenialReason,
    ProposalFailureReason, ProposalId, ProposalLifecycleState, ProposalLifecycleTransition,
    ProposalResponse, ProposalStaleContext, ProposalStaleReason, ProposalVersionPreconditions,
    ProtocolDiagnostic, ProtocolDiagnosticSeverity, ProtocolError, ProtocolResult, SnapshotId,
    TimestampMillis, WatcherEvent, WatcherEventKind, WorkspaceCloseRequest, WorkspaceClosed,
    WorkspaceConfigSnapshot, WorkspaceGeneration, WorkspaceId, WorkspaceOpenRequest,
    WorkspaceOpened, WorkspaceRequest, WorkspaceResponse, WorkspaceRootId, WorkspaceTrustState,
};
use devil_security::{DenyByDefaultBroker, TrustState};
use thiserror::Error;
use uuid::Uuid;

/// Internal filesystem trait alias used by [`WorkspaceActor`] for path-normalization and file-system operations.
pub trait ProjectFilesystemService:
    PathNormalizationService + FileSystemService + Send + Sync
{
}

impl<T> ProjectFilesystemService for T where
    T: PathNormalizationService + FileSystemService + Send + Sync
{
}

type ProjectFilesystem = dyn ProjectFilesystemService;

const LARGE_FILE_BYTES: u64 = 5 * 1024 * 1024;
const MAX_TREE_CHILDREN_DEPTH: usize = 2;
const WATCHER_EVENT_BUFFER: usize = 1_024;
const WATCHER_RENAME_DEBOUNCE_MILLIS: u64 = 64;
const WATCHER_RECOVERY_MAX_RESCANS: usize = 2;

fn now_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |dur| dur.as_millis() as u64)
}

fn stable_hash(value: &str) -> u128 {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let mut hasher = DefaultHasher::new();
    value.hash(&mut hasher);
    hasher.finish() as u128
}

fn trust_to_protocol(state: TrustState) -> WorkspaceTrustState {
    match state {
        TrustState::Trusted => WorkspaceTrustState::Trusted,
        TrustState::Untrusted => WorkspaceTrustState::Untrusted,
        TrustState::Unknown => WorkspaceTrustState::Unknown,
    }
}

#[derive(Debug, Error)]
/// Errors emitted by the workspace VFS.
pub enum WorkspaceError {
    /// Workspace has not been opened in this actor instance.
    #[error("workspace {workspace_id:?} has not been opened")]
    WorkspaceMissing {
        /// Workspace id.
        workspace_id: WorkspaceId,
    },
    /// Candidate path is outside the workspace root boundary.
    #[error("path `{path}` is outside workspace root boundary")]
    PathOutsideRoot {
        /// Requested canonical path.
        path: String,
    },
    /// Security policy denied the operation.
    #[error("security denied operation for `{path}`: {reason}")]
    SecurityDenied {
        /// Requested path.
        path: String,
        /// Human-readable reason.
        reason: String,
    },
    /// Platform-level error propagated as protocol-facing error.
    #[error("platform error: {0}")]
    Platform(#[from] PlatformError),
    /// Internal data inconsistency.
    #[error("internal error: {0}")]
    Internal(&'static str),
}

type WorkspaceResult<T> = Result<T, WorkspaceError>;

/// Metadata returned with a successful workspace text open.
#[derive(Debug, Clone)]
pub struct OpenedFileText {
    /// File identity captured at open time.
    pub identity: FileIdentity,
    /// UTF-8 text loaded from disk, or an explicit safe-new-file empty payload.
    pub text: String,
    /// Protocol fingerprint captured for save preconditions.
    pub fingerprint: ProtocolFileFingerprint,
    /// File content version captured at open time.
    pub file_content_version: FileContentVersion,
    /// Workspace generation captured at open time.
    pub workspace_generation: WorkspaceGeneration,
    /// Modified timestamp captured at open time if available.
    pub modified_at: Option<TimestampMillis>,
    /// File length captured at open time if available.
    pub file_length: Option<u64>,
    /// Whether this open represented explicit create intent for a new file.
    pub is_new_file: bool,
}

/// Proposal-context save request accepted by the workspace write pipeline.
#[derive(Debug, Clone)]
pub struct WorkspaceSaveRequest {
    /// Workspace being mutated.
    pub workspace_id: WorkspaceId,
    /// Proposal authorizing this write.
    pub proposal_id: ProposalId,
    /// Principal requesting the save.
    pub principal: PrincipalId,
    /// Required capability.
    pub required_capability: CapabilityId,
    /// Expected file identity.
    pub file_id: FileId,
    /// Target path.
    pub path: CanonicalPath,
    /// Expected disk fingerprint.
    pub expected_fingerprint: ProtocolFileFingerprint,
    /// Expected file content version.
    pub expected_file_content_version: FileContentVersion,
    /// Expected workspace generation.
    pub expected_workspace_generation: WorkspaceGeneration,
    /// Buffer version being saved.
    pub buffer_version: BufferVersion,
    /// Snapshot being saved.
    pub snapshot_id: SnapshotId,
    /// Payload byte length.
    pub payload_byte_len: u64,
    /// Correlation id.
    pub correlation_id: CorrelationId,
    /// Causality id linking save/proposal/workspace events.
    pub causality_id: CausalityId,
    /// UTF-8 text payload to write.
    pub text: String,
}

/// Non-atomic fallback policy for workspace saves.
///
/// Track 3 intentionally exposes only the fail-closed policy. Any future fallback variant must add
/// explicit security approval, immediate fingerprint re-verification, visible fallback response,
/// and event/audit hook placeholders before use.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NonAtomicSaveFallbackPolicy {
    /// Fail closed when atomic replacement fails.
    Disabled,
}

/// Successful workspace save metadata.
#[derive(Debug, Clone)]
pub struct WorkspaceSaveApplied {
    /// Updated file identity.
    pub identity: FileIdentity,
    /// New disk fingerprint.
    pub fingerprint: ProtocolFileFingerprint,
    /// Updated file content version.
    pub file_content_version: FileContentVersion,
    /// Updated workspace generation.
    pub workspace_generation: WorkspaceGeneration,
    /// Updated modified timestamp if available.
    pub modified_at: Option<TimestampMillis>,
    /// Updated file length if available.
    pub file_length: Option<u64>,
    /// Whether a non-atomic fallback path was used.
    pub used_non_atomic_fallback: bool,
    /// Visible fallback status for audit/UI surfaces.
    pub fallback_status: Option<String>,
    /// Proposal response for applied lifecycle.
    pub response: ProposalResponse,
}

/// Workspace save result preserving typed stale/conflict/denial/failure responses.
pub type WorkspaceSaveResult = Result<WorkspaceSaveApplied, ProposalResponse>;

#[derive(Debug, Clone)]
struct FileFingerprint {
    size: Option<u64>,
    modified: Option<TimestampMillis>,
    hash: Option<String>,
    read_only: bool,
}

impl FileFingerprint {
    fn from_path(path: &Path, fs: &ProjectFilesystem) -> Result<Self, WorkspaceError> {
        let metadata = fs.read_metadata(path).map_err(WorkspaceError::Platform)?;
        Self::from_metadata(path, fs, &metadata)
    }

    fn from_metadata(
        path: &Path,
        fs: &ProjectFilesystem,
        metadata: &FileSystemMetadata,
    ) -> Result<Self, WorkspaceError> {
        let size = metadata.length;
        let modified = metadata.modified_at.map(TimestampMillis);
        let hash = if metadata.is_file() && size <= LARGE_FILE_BYTES {
            let fingerprint = fs
                .read_fingerprint(path)
                .map_err(WorkspaceError::Platform)?;
            if fingerprint.length != Some(size) || fingerprint.modified_at != metadata.modified_at {
                return Err(WorkspaceError::Platform(
                    PlatformError::MetadataInconsistent {
                        operation: "workspace fingerprint read".to_string(),
                        path: path.to_path_buf(),
                        details: format!(
                            "metadata={metadata:?}, fingerprint_length={:?}, fingerprint_modified={:?}",
                            fingerprint.length, fingerprint.modified_at
                        ),
                    },
                ));
            }
            Some(fingerprint.stable_hash.ok_or_else(|| {
                WorkspaceError::Platform(PlatformError::MetadataInconsistent {
                    operation: "workspace fingerprint read".to_string(),
                    path: path.to_path_buf(),
                    details: "regular file fingerprint did not include stable hash".to_string(),
                })
            })?)
        } else {
            None
        };

        Ok(Self {
            size: Some(size),
            modified,
            hash,
            read_only: metadata.read_only,
        })
    }

    fn from_dir() -> Self {
        Self {
            size: None,
            modified: None,
            hash: None,
            read_only: false,
        }
    }

    fn for_new_file(path: &Path) -> Self {
        let mut value = path.to_string_lossy().into_owned();
        value.push_str("|new-file|0");
        Self {
            size: Some(0),
            modified: None,
            hash: Some(format!("new:{:016x}", stable_hash(&value))),
            read_only: false,
        }
    }

    fn to_protocol(&self) -> ProtocolFileFingerprint {
        let hash = self.hash.clone().unwrap_or_else(|| "nohash".to_string());
        let modified = self
            .modified
            .map(|value| value.0.to_string())
            .unwrap_or_else(|| "nomtime".to_string());
        let size = self
            .size
            .map(|value| value.to_string())
            .unwrap_or_else(|| "nosize".to_string());
        ProtocolFileFingerprint {
            algorithm: "devil-fingerprint-v1".to_string(),
            value: format!("size={size};modified={modified};hash={hash}"),
        }
    }
}

impl PartialEq for FileFingerprint {
    fn eq(&self, other: &Self) -> bool {
        self.size == other.size && self.modified == other.modified && self.hash == other.hash
    }
}

#[derive(Debug)]
struct WorkspaceState {
    workspace_id: WorkspaceId,
    workspace_root_id: WorkspaceRootId,
    principal_id: PrincipalId,
    root_path: PathBuf,
    trust: TrustState,
    generation: WorkspaceGeneration,
    config: WorkspaceConfigSnapshot,
    config_snapshot_id: SnapshotId,
    next_file_id: u128,
    file_id_by_path: HashMap<String, FileId>,
    file_metadata: HashMap<FileId, FileMetadata>,
    file_path_by_id: HashMap<FileId, String>,
    tree: Vec<FileTreeNode>,
    last_scan: HashMap<String, FileFingerprint>,
    active_sessions: HashSet<FileId>,
    watcher_sequence: u64,
    watcher_queue: VecDeque<WatcherEvent>,
    last_watcher_poll: u64,
    last_watcher_signature: HashSet<String>,
    in_recovery: bool,
}

struct WorkspaceStateInit {
    workspace_id: WorkspaceId,
    workspace_root_id: WorkspaceRootId,
    principal_id: PrincipalId,
    root_path: PathBuf,
    trust: TrustState,
    snapshot_id: SnapshotId,
    tree: Vec<FileTreeNode>,
    scan: HashMap<String, FileFingerprint>,
}

impl WorkspaceState {
    fn new(init: WorkspaceStateInit) -> Self {
        let WorkspaceStateInit {
            workspace_id,
            workspace_root_id,
            principal_id,
            root_path,
            trust,
            snapshot_id,
            tree,
            scan,
        } = init;
        let canonical_root = CanonicalPath(root_path.to_string_lossy().into_owned());

        Self {
            workspace_id,
            workspace_root_id,
            principal_id,
            root_path,
            trust,
            generation: WorkspaceGeneration(1),
            config: WorkspaceConfigSnapshot {
                workspace_id,
                root_path: canonical_root,
                merged: HashMap::new(),
                trust_state: trust_to_protocol(trust),
                captured_at: TimestampMillis(now_millis()),
                schema_version: "1.0".to_string(),
            },
            config_snapshot_id: snapshot_id,
            next_file_id: 1,
            file_id_by_path: HashMap::new(),
            file_metadata: HashMap::new(),
            file_path_by_id: HashMap::new(),
            tree,
            last_scan: scan,
            active_sessions: HashSet::new(),
            watcher_sequence: 0,
            watcher_queue: VecDeque::new(),
            last_watcher_poll: 0,
            last_watcher_signature: HashSet::new(),
            in_recovery: false,
        }
    }

    fn next_file_id(&mut self) -> FileId {
        let id = FileId(self.next_file_id);
        self.next_file_id = self.next_file_id.saturating_add(1);
        id
    }

    fn enqueue_watcher_event(&mut self, event: WatcherEvent) {
        if self.watcher_queue.len() >= WATCHER_EVENT_BUFFER {
            let _ = self.watcher_queue.pop_front();
        }
        let signature = format!("{}::{:?}", event.path.0, event.kind);
        if self.last_watcher_signature.contains(&signature) {
            return;
        }
        self.last_watcher_signature.insert(signature);
        self.watcher_queue.push_back(event);
    }
}

#[derive(Debug, Default)]
struct DiscoveryConfig {
    skip_hidden: bool,
    skip_generated: bool,
    skip_binary: bool,
    skip_large: bool,
}

/// Actor-like workspace service with typed project state and shallow tree ownership.
pub struct WorkspaceActor {
    fs: Arc<ProjectFilesystem>,
    watcher: Arc<dyn WatcherService + Send + Sync>,
    security: Mutex<DenyByDefaultBroker>,
    state: Mutex<Option<WorkspaceState>>,
    discovery: DiscoveryConfig,
    event_sink: Box<dyn EventSinkPort + Send + Sync>,
}

impl WorkspaceActor {
    /// Creates a new workspace actor.
    pub fn new(
        fs: Arc<ProjectFilesystem>,
        watcher: Arc<dyn WatcherService + Send + Sync>,
        security: DenyByDefaultBroker,
    ) -> Self {
        Self::with_event_sink(fs, watcher, security, Box::new(NoopEventSink))
    }

    /// Creates a new workspace actor with an injected event sink.
    pub fn with_event_sink(
        fs: Arc<ProjectFilesystem>,
        watcher: Arc<dyn WatcherService + Send + Sync>,
        security: DenyByDefaultBroker,
        event_sink: Box<dyn EventSinkPort + Send + Sync>,
    ) -> Self {
        Self {
            fs,
            watcher,
            security: Mutex::new(security),
            state: Mutex::new(None),
            discovery: DiscoveryConfig {
                skip_hidden: true,
                skip_generated: true,
                skip_binary: true,
                skip_large: true,
            },
            event_sink,
        }
    }

    fn now_sequence(state: &mut WorkspaceState) -> EventSequence {
        state.watcher_sequence = state.watcher_sequence.saturating_add(1);
        EventSequence(state.watcher_sequence)
    }

    fn causality() -> CausalityId {
        CausalityId(Uuid::now_v7())
    }

    fn emit(&self, envelope: devil_protocol::EventEnvelope) {
        let _ = self.event_sink.emit(EventSinkRequest { envelope });
    }

    fn canonicalize_root_path(&self, state: &WorkspaceState) -> WorkspaceResult<PathBuf> {
        self.fs
            .canonicalize_path(&state.root_path)
            .or_else(|_| self.fs.normalize_path(&state.root_path))
            .map_err(WorkspaceError::Platform)
    }

    fn canonicalize_with_parent_fallback(&self, path: &Path) -> WorkspaceResult<PathBuf> {
        match self.fs.canonicalize_path(path) {
            Ok(path) => Ok(path),
            Err(PlatformError::NotFound { .. }) => {
                let mut suffix = Vec::new();
                let mut cursor = path.to_path_buf();

                while let Err(PlatformError::NotFound { .. }) = self.fs.canonicalize_path(&cursor) {
                    let Some(name) = cursor.file_name() else {
                        break;
                    };
                    suffix.push(name.to_os_string());

                    let Some(parent) = cursor.parent() else {
                        break;
                    };
                    cursor = parent.to_path_buf();
                }

                let mut rebuilt = self
                    .fs
                    .canonicalize_path(&cursor)
                    .or_else(|_| self.fs.normalize_path(&cursor))
                    .map_err(WorkspaceError::Platform)?;

                for part in suffix.iter().rev() {
                    rebuilt.push(part);
                }

                self.fs
                    .normalize_path(&rebuilt)
                    .map_err(WorkspaceError::Platform)
            }
            Err(err) => Err(WorkspaceError::Platform(err)),
        }
    }

    fn path_components_for_compare(path: &Path) -> Vec<String> {
        let mut normalized = path.to_string_lossy().replace('\\', "/");

        if normalized.starts_with("//?/UNC/") {
            normalized = format!("//{}", &normalized[8..]);
        } else if normalized.starts_with("//?/") || normalized.starts_with("//./") {
            normalized = normalized[4..].to_string();
        }

        #[cfg(windows)]
        {
            normalized = normalized.to_ascii_lowercase();
        }

        let mut components = Vec::new();
        for part in normalized.split('/') {
            if part.is_empty() || part == "." {
                continue;
            }
            if part == ".." {
                components.pop();
                continue;
            }
            components.push(part.to_string());
        }
        components
    }

    fn path_is_within_root(root: &Path, candidate: &Path) -> bool {
        let root_parts = Self::path_components_for_compare(root);
        let candidate_parts = Self::path_components_for_compare(candidate);

        if root_parts.len() > candidate_parts.len() {
            return false;
        }

        root_parts
            .iter()
            .zip(candidate_parts.iter())
            .all(|(left, right)| left == right)
    }

    fn check_path_within_root(&self, state: &WorkspaceState, path: &Path) -> WorkspaceResult<()> {
        let root = self.canonicalize_root_path(state)?;
        let candidate = self.canonicalize_with_parent_fallback(path)?;

        if Self::path_is_within_root(&root, &candidate) {
            Ok(())
        } else {
            Err(WorkspaceError::PathOutsideRoot {
                path: candidate.to_string_lossy().into_owned(),
            })
        }
    }

    fn canonicalize_candidate(
        &self,
        state: &WorkspaceState,
        path: &str,
    ) -> WorkspaceResult<PathBuf> {
        let path = Path::new(path);
        let absolute = if path.is_absolute() {
            path.to_path_buf()
        } else {
            state.root_path.join(path)
        };
        let normalized = self
            .fs
            .normalize_path(&absolute)
            .map_err(WorkspaceError::Platform)?;
        self.check_path_within_root(state, &normalized)?;
        Ok(normalized)
    }

    fn should_skip_entry(&self, entry_name: &str, metadata: Option<&FileSystemMetadata>) -> bool {
        if self.discovery.skip_hidden && entry_name.starts_with('.') {
            return true;
        }

        let generated = [
            ".git",
            "target",
            "node_modules",
            ".idea",
            ".vscode",
            "out",
            "dist",
            "build",
        ];
        if self.discovery.skip_generated && generated.contains(&entry_name) {
            return true;
        }

        let binaries = [
            ".exe", ".dll", ".so", ".png", ".jpg", ".jpeg", ".gif", ".pdf", ".zip", ".class",
            ".jar", ".ico", ".bin", ".mp4", ".mp3",
        ];
        if self.discovery.skip_binary
            && let Some(ext) = Path::new(entry_name)
                .extension()
                .and_then(|value| value.to_str())
        {
            let suffix = format!(".{ext}").to_ascii_lowercase();
            if binaries.iter().any(|value| *value == suffix) {
                return true;
            }
        }

        if self.discovery.skip_large
            && let Some(meta) = metadata
            && meta.is_file()
            && meta.length > LARGE_FILE_BYTES
        {
            return true;
        }

        false
    }

    fn kind_for_platform_metadata(&self, metadata: &FileSystemMetadata) -> FileKind {
        match metadata.kind {
            FileSystemEntryKind::Directory => FileKind::Directory,
            FileSystemEntryKind::Symlink => FileKind::Symlink,
            FileSystemEntryKind::File => FileKind::File,
            FileSystemEntryKind::Other => FileKind::Other("other".to_string()),
        }
    }

    fn file_identity_from_platform_metadata(
        &self,
        state: &mut WorkspaceState,
        canonical_path: &Path,
        fingerprint: &FileFingerprint,
        metadata: &FileSystemMetadata,
    ) -> FileIdentity {
        let key = canonical_path.to_string_lossy().into_owned();
        let file_id = if let Some(id) = state.file_id_by_path.get(&key) {
            *id
        } else {
            let id = state.next_file_id();
            state.file_id_by_path.insert(key.clone(), id);
            state.file_path_by_id.insert(id, key.clone());
            id
        };

        let content_version = match (
            fingerprint.size,
            fingerprint.modified,
            fingerprint.hash.as_ref(),
        ) {
            (Some(size), Some(ts), Some(hash)) => {
                let digest = (size ^ ts.0).wrapping_add(stable_hash(hash) as u64);
                FileContentVersion(digest)
            }
            (Some(size), Some(ts), None) => FileContentVersion(size.saturating_add(ts.0)),
            (Some(size), None, _) => FileContentVersion(size),
            _ => FileContentVersion(0),
        };

        let protocol_fingerprint = fingerprint.to_protocol();
        let canonical_path = CanonicalPath(canonical_path.to_string_lossy().into_owned());
        let file_metadata = FileMetadata {
            canonical_path: canonical_path.clone(),
            file_id: Some(file_id),
            workspace_id: Some(state.workspace_id),
            kind: self.kind_for_platform_metadata(metadata),
            size_bytes: fingerprint.size,
            modified_at: fingerprint.modified,
            read_only: fingerprint.read_only,
            permissions: None,
            hash: fingerprint.hash.clone(),
            fingerprint: Some(protocol_fingerprint),
            content_version: Some(content_version),
            workspace_generation: Some(state.generation),
            schema_version: 1,
        };

        state.file_metadata.insert(file_id, file_metadata);

        FileIdentity {
            file_id,
            workspace_id: state.workspace_id,
            canonical_path,
            content_version,
            content_hash: fingerprint.hash.clone(),
        }
    }

    fn file_identity_for_new_path(
        &self,
        state: &mut WorkspaceState,
        canonical_path: &Path,
        fingerprint: &FileFingerprint,
    ) -> FileIdentity {
        let key = canonical_path.to_string_lossy().into_owned();
        let file_id = if let Some(id) = state.file_id_by_path.get(&key) {
            *id
        } else {
            let id = state.next_file_id();
            state.file_id_by_path.insert(key.clone(), id);
            state.file_path_by_id.insert(id, key.clone());
            id
        };
        let protocol_fingerprint = fingerprint.to_protocol();
        state.file_metadata.insert(
            file_id,
            FileMetadata {
                canonical_path: CanonicalPath(key.clone()),
                file_id: Some(file_id),
                workspace_id: Some(state.workspace_id),
                kind: FileKind::File,
                size_bytes: fingerprint.size,
                modified_at: fingerprint.modified,
                read_only: fingerprint.read_only,
                permissions: Some("new-file-precondition".to_string()),
                hash: fingerprint.hash.clone(),
                fingerprint: Some(protocol_fingerprint),
                content_version: Some(FileContentVersion(0)),
                workspace_generation: Some(state.generation),
                schema_version: 1,
            },
        );
        FileIdentity {
            file_id,
            workspace_id: state.workspace_id,
            canonical_path: CanonicalPath(key),
            content_version: FileContentVersion(0),
            content_hash: fingerprint.hash.clone(),
        }
    }

    fn metadata_for_identity(
        &self,
        state: &WorkspaceState,
        file_id: FileId,
    ) -> Option<FileMetadata> {
        state.file_metadata.get(&file_id).cloned()
    }

    fn open_existing_file_text_internal(
        &self,
        state: &mut WorkspaceState,
        path: &str,
        correlation_id: Option<CorrelationId>,
        causality_id: Option<CausalityId>,
    ) -> WorkspaceResult<OpenedFileText> {
        let workspace_id = state.workspace_id;
        let canonical = self.canonicalize_candidate(state, path)?;
        let target_path = canonical.to_string_lossy().into_owned();
        if let Err(err) = self.decision_for_workspace(state, "fs.read", Some(&target_path)) {
            if let (Some(correlation_id), Some(causality_id)) = (correlation_id, causality_id) {
                let sequence = Self::now_sequence(state);
                self.emit(security_denial_event(
                    workspace_id,
                    None,
                    Some(state.principal_id.clone()),
                    &CapabilityId("fs.read".to_string()),
                    correlation_id,
                    causality_id,
                    sequence,
                    Some(&target_path),
                    err.to_string(),
                ));
            }
            return Err(err);
        }

        let metadata = match self.fs.read_metadata(&canonical) {
            Ok(metadata) => metadata,
            Err(err) => {
                if let (Some(correlation_id), Some(causality_id)) = (correlation_id, causality_id) {
                    let sequence = Self::now_sequence(state);
                    self.emit(open_file_read_failure_event(
                        workspace_id,
                        correlation_id,
                        causality_id,
                        sequence,
                        &target_path,
                        err.to_string(),
                    ));
                }
                return Err(WorkspaceError::Platform(err));
            }
        };
        let fingerprint = if metadata.is_file() {
            FileFingerprint::from_metadata(&canonical, self.fs.as_ref(), &metadata)?
        } else {
            FileFingerprint::from_dir()
        };
        let identity =
            self.file_identity_from_platform_metadata(state, &canonical, &fingerprint, &metadata);
        let metadata =
            self.metadata_for_identity(state, identity.file_id)
                .ok_or(WorkspaceError::Internal(
                    "file metadata missing after identity capture",
                ))?;
        let text = match self.fs.read_text_file(&canonical) {
            Ok(text) => text,
            Err(err) => {
                if let (Some(correlation_id), Some(causality_id)) = (correlation_id, causality_id) {
                    let sequence = Self::now_sequence(state);
                    self.emit(open_file_read_failure_event(
                        workspace_id,
                        correlation_id,
                        causality_id,
                        sequence,
                        canonical.to_string_lossy(),
                        err.to_string(),
                    ));
                }
                return Err(WorkspaceError::Platform(err));
            }
        };
        if text.contains('\0') {
            if let (Some(correlation_id), Some(causality_id)) = (correlation_id, causality_id) {
                let sequence = Self::now_sequence(state);
                self.emit(open_file_read_failure_event(
                    workspace_id,
                    correlation_id,
                    causality_id,
                    sequence,
                    canonical.to_string_lossy(),
                    "binary content rejected for text buffer",
                ));
            }
            return Err(WorkspaceError::Platform(PlatformError::Encoding {
                operation: "read".to_string(),
                path: canonical,
                source: std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "binary content rejected for text buffer",
                ),
            }));
        }

        state.active_sessions.insert(identity.file_id);
        Ok(OpenedFileText {
            identity: identity.clone(),
            text,
            fingerprint: fingerprint.to_protocol(),
            file_content_version: identity.content_version,
            workspace_generation: state.generation,
            modified_at: metadata.modified_at,
            file_length: metadata.size_bytes,
            is_new_file: false,
        })
    }

    fn open_new_file_text_internal(
        &self,
        state: &mut WorkspaceState,
        path: &str,
    ) -> WorkspaceResult<OpenedFileText> {
        let canonical = self.canonicalize_candidate(state, path)?;
        self.decision_for_workspace(state, "fs.write", Some(&canonical.to_string_lossy()))?;
        match self.fs.read_metadata(&canonical) {
            Ok(_) => return self.open_existing_file_text_internal(state, path, None, None),
            Err(PlatformError::NotFound { .. }) => {}
            Err(err) => return Err(WorkspaceError::Platform(err)),
        }

        let fingerprint = FileFingerprint::for_new_file(&canonical);
        let identity = self.file_identity_for_new_path(state, &canonical, &fingerprint);
        state.active_sessions.insert(identity.file_id);
        Ok(OpenedFileText {
            identity,
            text: String::new(),
            fingerprint: fingerprint.to_protocol(),
            file_content_version: FileContentVersion(0),
            workspace_generation: state.generation,
            modified_at: None,
            file_length: Some(0),
            is_new_file: true,
        })
    }

    fn scan_shallow(
        &self,
        state: &mut WorkspaceState,
    ) -> WorkspaceResult<(Vec<FileTreeNode>, HashMap<String, FileFingerprint>)> {
        let mut nodes = Vec::new();
        let mut fingerprints = HashMap::new();
        let root_path = state.root_path.clone();

        self.collect_tree_nodes(
            &root_path,
            &PathBuf::new(),
            0,
            state,
            &mut nodes,
            &mut fingerprints,
        )?;

        Ok((nodes, fingerprints))
    }

    fn collect_tree_nodes(
        &self,
        root: &Path,
        relative: &Path,
        depth: usize,
        state: &mut WorkspaceState,
        nodes: &mut Vec<FileTreeNode>,
        fingerprints: &mut HashMap<String, FileFingerprint>,
    ) -> WorkspaceResult<()> {
        if depth > MAX_TREE_CHILDREN_DEPTH {
            return Ok(());
        }

        let target = if relative.as_os_str().is_empty() {
            root.to_path_buf()
        } else {
            root.join(relative)
        };

        let entries = self
            .fs
            .list_directory(&target)
            .map_err(WorkspaceError::Platform)?;

        for child in entries {
            let entry_name: String = child
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_default();

            let meta = self.fs.read_metadata(&child).ok();
            let meta_ref = meta.as_ref();

            let entry_skip = if let Some(meta) = meta.as_ref() {
                self.should_skip_entry(&entry_name, Some(meta))
            } else {
                false
            };

            if entry_skip {
                continue;
            }

            let canonical = self
                .fs
                .normalize_path(&child)
                .map_err(WorkspaceError::Platform)?;
            self.check_path_within_root(state, &canonical)?;

            let metadata = match meta_ref {
                Some(meta) => {
                    if meta.is_file() {
                        FileFingerprint::from_metadata(&canonical, self.fs.as_ref(), meta)?
                    } else {
                        FileFingerprint::from_dir()
                    }
                }
                None => FileFingerprint {
                    size: None,
                    modified: None,
                    hash: None,
                    read_only: true,
                },
            };
            let identity = if let Some(meta) = meta_ref {
                self.file_identity_from_platform_metadata(state, &canonical, &metadata, meta)
            } else {
                let key = canonical.to_string_lossy().into_owned();
                let file_id = state.next_file_id();
                state.file_id_by_path.insert(key.clone(), file_id);
                state.file_path_by_id.insert(file_id, key.clone());
                state.file_metadata.insert(
                    file_id,
                    FileMetadata {
                        canonical_path: CanonicalPath(key.clone()),
                        file_id: Some(file_id),
                        workspace_id: Some(state.workspace_id),
                        kind: FileKind::Other("unreadable".to_string()),
                        size_bytes: metadata.size,
                        modified_at: metadata.modified,
                        read_only: metadata.read_only,
                        permissions: Some("unreadable".to_string()),
                        hash: metadata.hash.clone(),
                        fingerprint: Some(metadata.to_protocol()),
                        content_version: Some(FileContentVersion(0)),
                        workspace_generation: Some(state.generation),
                        schema_version: 1,
                    },
                );
                FileIdentity {
                    file_id,
                    workspace_id: state.workspace_id,
                    canonical_path: CanonicalPath(key),
                    content_version: FileContentVersion(0),
                    content_hash: None,
                }
            };

            let mut child_ids = Vec::new();
            let is_dir = meta.as_ref().map(|meta| meta.is_dir()).unwrap_or(false);
            if is_dir && depth < MAX_TREE_CHILDREN_DEPTH {
                let mut grandchildren: Vec<FileTreeNode> = Vec::new();
                let mut extra_meta = HashMap::new();
                self.collect_tree_nodes(
                    root,
                    &relative.join(&entry_name),
                    depth + 1,
                    state,
                    &mut grandchildren,
                    &mut extra_meta,
                )?;
                for child_node in &grandchildren {
                    child_ids.push(child_node.identity.file_id);
                }
                for (k, v) in extra_meta {
                    fingerprints.insert(k, v);
                }
            }

            let metadata = state
                .file_metadata
                .get(&identity.file_id)
                .cloned()
                .unwrap_or_else(|| FileMetadata {
                    canonical_path: identity.canonical_path.clone(),
                    file_id: Some(identity.file_id),
                    workspace_id: Some(identity.workspace_id),
                    kind: FileKind::Other("unknown".to_string()),
                    size_bytes: None,
                    modified_at: None,
                    read_only: false,
                    permissions: None,
                    hash: None,
                    fingerprint: None,
                    content_version: Some(identity.content_version),
                    workspace_generation: Some(state.generation),
                    schema_version: 1,
                });

            fingerprints.insert(
                identity.canonical_path.0.clone(),
                metadata
                    .size_bytes
                    .zip(metadata.modified_at)
                    .map(|(size, modified)| FileFingerprint {
                        size: Some(size),
                        modified: Some(modified),
                        hash: metadata.hash.clone(),
                        read_only: metadata.read_only,
                    })
                    .unwrap_or_else(|| {
                        let mut f = FileFingerprint::from_dir();
                        f.read_only = metadata.read_only;
                        f
                    }),
            );

            nodes.push(FileTreeNode {
                identity,
                name: entry_name,
                children: child_ids,
                metadata: Some(metadata),
            });
        }

        Ok(())
    }

    fn decision_for_workspace(
        &self,
        state: &WorkspaceState,
        capability: &str,
        path: Option<&str>,
    ) -> WorkspaceResult<()> {
        let mut security = self
            .security
            .lock()
            .map_err(|_| WorkspaceError::Internal("security lock poisoned"))?;

        let decision = security.decide(
            state.trust,
            state.principal_id.clone(),
            devil_protocol::CapabilityId(capability.to_string()),
            path,
        );
        match decision {
            devil_security::SecurityDecision::Allow => Ok(()),
            devil_security::SecurityDecision::Deny(reason) => Err(WorkspaceError::SecurityDenied {
                path: path.unwrap_or("").to_string(),
                reason,
            }),
        }
    }

    fn diagnostic(
        code: impl Into<String>,
        message: impl Into<String>,
        path: Option<CanonicalPath>,
    ) -> ProtocolDiagnostic {
        ProtocolDiagnostic {
            code: code.into(),
            message: message.into(),
            severity: ProtocolDiagnosticSeverity::Error,
            path,
            range: None,
        }
    }

    fn save_transition(
        request: &WorkspaceSaveRequest,
        state: ProposalLifecycleState,
        diagnostics: Vec<ProtocolDiagnostic>,
    ) -> ProposalLifecycleTransition {
        ProposalLifecycleTransition {
            proposal_id: request.proposal_id,
            lifecycle_state: state,
            timestamp: TimestampMillis::now(),
            principal: request.principal.clone(),
            capability: request.required_capability.clone(),
            correlation_id: request.correlation_id,
            causality_id: request.causality_id,
            diagnostics,
        }
    }

    fn denied_save_response(
        request: &WorkspaceSaveRequest,
        reason: ProposalDenialReason,
        message: impl Into<String>,
    ) -> ProposalResponse {
        let diagnostic = Self::diagnostic("proposal.denied", message, Some(request.path.clone()));
        ProposalResponse::Denied {
            transition: Self::save_transition(
                request,
                ProposalLifecycleState::Denied,
                vec![diagnostic],
            ),
            reason,
        }
    }

    fn failed_save_response(
        request: &WorkspaceSaveRequest,
        message: impl Into<String>,
    ) -> ProposalResponse {
        let diagnostic = Self::diagnostic("proposal.failed", message, Some(request.path.clone()));
        ProposalResponse::Failed {
            transition: Self::save_transition(
                request,
                ProposalLifecycleState::Failed,
                vec![diagnostic],
            ),
            reason: ProposalFailureReason::ApplyFailed,
        }
    }

    fn stale_save_response(
        &self,
        request: &WorkspaceSaveRequest,
        reason: ProposalStaleReason,
        actual: Option<devil_protocol::VersionContext>,
        message: impl Into<String>,
    ) -> ProposalResponse {
        let diagnostic = Self::diagnostic("proposal.stale", message, Some(request.path.clone()));
        ProposalResponse::Stale {
            transition: Self::save_transition(
                request,
                ProposalLifecycleState::Stale,
                vec![diagnostic],
            ),
            stale: ProposalStaleContext {
                reason,
                expected: ProposalVersionPreconditions {
                    file_version: Some(request.expected_file_content_version),
                    buffer_version: Some(request.buffer_version),
                    snapshot_id: Some(request.snapshot_id),
                    generation: Some(request.expected_workspace_generation),
                    file_content_version: Some(request.expected_file_content_version),
                    workspace_generation: Some(request.expected_workspace_generation),
                    expected_fingerprint: Some(request.expected_fingerprint.clone()),
                    expected_file_length: None,
                    expected_modified_at: None,
                },
                actual,
            },
        }
    }

    fn conflict_save_response(
        &self,
        state: &mut WorkspaceState,
        request: &WorkspaceSaveRequest,
        identity: FileIdentity,
        actual_fingerprint: Option<ProtocolFileFingerprint>,
        message: impl Into<String>,
    ) -> ProposalResponse {
        let diagnostic = Self::diagnostic("proposal.conflict", message, Some(request.path.clone()));
        let conflict = FileConflictState {
            state: FileConflictLifecycleState::ConflictDirty,
            context: FileConflictContext {
                workspace_id: request.workspace_id,
                file_identity: identity,
                buffer_version: request.buffer_version,
                file_content_version: request.expected_file_content_version,
                snapshot_id: request.snapshot_id,
                disk_fingerprint: actual_fingerprint,
                expected_fingerprint: Some(request.expected_fingerprint.clone()),
                reason: FileConflictReason::DiskFingerprintChanged,
                diagnostics: vec![diagnostic.clone()],
            },
            diagnostics: vec![diagnostic],
            schema_version: 1,
        };
        let sequence = Self::now_sequence(state);
        self.emit(conflict_created_event(
            &conflict,
            request.correlation_id,
            request.causality_id,
            sequence,
        ));
        ProposalResponse::Conflict {
            transition: Self::save_transition(
                request,
                ProposalLifecycleState::Conflict,
                conflict.diagnostics.clone(),
            ),
            conflict,
        }
    }

    fn resolve_identity_internal(
        &self,
        state: &mut WorkspaceState,
        path: &str,
    ) -> WorkspaceResult<FileIdentity> {
        let canonical = self.canonicalize_candidate(state, path)?;
        self.decision_for_workspace(state, "fs.read", Some(&canonical.to_string_lossy()))?;

        let metadata = self
            .fs
            .read_metadata(&canonical)
            .map_err(WorkspaceError::Platform)?;
        let fingerprint = if metadata.is_file() {
            FileFingerprint::from_metadata(&canonical, self.fs.as_ref(), &metadata)?
        } else {
            FileFingerprint::from_dir()
        };

        let identity =
            self.file_identity_from_platform_metadata(state, &canonical, &fingerprint, &metadata);
        state.active_sessions.insert(identity.file_id);
        Ok(identity)
    }

    fn apply_tree_delta_internal(
        &self,
        state: &mut WorkspaceState,
        delta: FileTreeDelta,
    ) -> WorkspaceResult<()> {
        let identity = delta.identity.clone();
        let canonical_name = identity
            .canonical_path
            .0
            .rsplit('/')
            .next()
            .unwrap_or("unknown")
            .to_string();
        match delta.op {
            FileTreeDeltaOp::Add => {
                state.tree.push(FileTreeNode {
                    identity,
                    name: canonical_name,
                    children: Vec::new(),
                    metadata: None,
                });
            }
            FileTreeDeltaOp::Remove => {
                state
                    .tree
                    .retain(|node| node.identity.file_id != delta.identity.file_id);
            }
            FileTreeDeltaOp::Rename | FileTreeDeltaOp::Update => {
                if let Some(node) = state
                    .tree
                    .iter_mut()
                    .find(|node| node.identity.file_id == identity.file_id)
                {
                    node.identity = identity;
                    if let Some(target) = delta.target_path {
                        node.name = target.0.rsplit('/').next().unwrap_or("unknown").to_string();
                    }
                }
            }
        }

        Ok(())
    }

    fn rebuild_tree_from_scan(&self, state: &mut WorkspaceState) -> WorkspaceResult<()> {
        let (nodes, fingerprints) = self.scan_shallow(state)?;
        state.tree = nodes;
        state.last_scan = fingerprints;
        Ok(())
    }

    fn rebuild_tree_from_scan_bounded(&self, state: &mut WorkspaceState) -> WorkspaceResult<bool> {
        for _ in 0..WATCHER_RECOVERY_MAX_RESCANS {
            if self.rebuild_tree_from_scan(state).is_ok() {
                return Ok(true);
            }
        }
        Ok(false)
    }

    fn collect_watcher_events(
        &self,
        state: &mut WorkspaceState,
    ) -> WorkspaceResult<Vec<WatcherEvent>> {
        let now = now_millis();
        if now.saturating_sub(state.last_watcher_poll) < WATCHER_RENAME_DEBOUNCE_MILLIS {
            return Ok(Vec::new());
        }
        state.last_watcher_poll = now;

        let root = state.root_path.clone();
        let workspace_id = state.workspace_id;

        if state.in_recovery {
            let recovered = self.rebuild_tree_from_scan_bounded(state)?;
            if recovered {
                state.in_recovery = false;
                let sequence = Self::now_sequence(state);
                self.emit(watcher_recovery_event(
                    workspace_id,
                    CorrelationId(sequence.0),
                    Self::causality(),
                    sequence,
                    true,
                ));
                let event = WatcherEvent {
                    workspace_id,
                    kind: WatcherEventKind::Modified,
                    path: CanonicalPath(root.to_string_lossy().into_owned()),
                    old_path: None,
                    sequence,
                };
                state.enqueue_watcher_event(event.clone());
                return Ok(vec![event]);
            }
        }

        let snapshot = self.watcher.snapshot(workspace_id, &root);
        let new_entries: Vec<PathBuf> = match snapshot {
            Ok(events) => events
                .into_iter()
                .map(|event| PathBuf::from(event.path.0))
                .collect(),
            Err(PlatformError::WatcherOverflow { .. }) => {
                state.in_recovery = true;
                let sequence = Self::now_sequence(state);
                self.emit(watcher_recovery_event(
                    workspace_id,
                    CorrelationId(sequence.0),
                    Self::causality(),
                    sequence,
                    false,
                ));
                let overflow = WatcherEvent {
                    workspace_id,
                    kind: WatcherEventKind::Overflow,
                    path: CanonicalPath(root.to_string_lossy().into_owned()),
                    old_path: None,
                    sequence,
                };
                state.enqueue_watcher_event(overflow.clone());
                return Ok(vec![overflow]);
            }
            Err(err) => return Err(WorkspaceError::Platform(err)),
        };

        let current = new_entries
            .into_iter()
            .filter_map(|path| {
                let normalized = self.fs.normalize_path(&path).ok()?;
                FileFingerprint::from_path(&normalized, self.fs.as_ref())
                    .ok()
                    .map(|fingerprint| (normalized.to_string_lossy().into_owned(), fingerprint))
            })
            .collect::<HashMap<String, FileFingerprint>>();

        let mut produced = Vec::new();
        let previous: HashSet<String> = state.last_scan.keys().cloned().collect();
        let current_paths: HashSet<String> = current.keys().cloned().collect();

        let removed: Vec<String> = previous.difference(&current_paths).cloned().collect();
        let added: Vec<String> = current_paths.difference(&previous).cloned().collect();

        let mut modified = Vec::new();
        for path in current_paths.intersection(&previous) {
            if state.last_scan.get(path) != current.get(path) {
                modified.push(path.clone());
            }
        }

        if removed.len() == 1 && added.len() == 1 {
            let old_path = removed[0].clone();
            let new_path = added[0].clone();
            let event = WatcherEvent {
                workspace_id,
                kind: WatcherEventKind::Renamed,
                path: CanonicalPath(new_path.clone()),
                old_path: Some(CanonicalPath(old_path)),
                sequence: Self::now_sequence(state),
            };
            state.enqueue_watcher_event(event.clone());
            produced.push(event);
        } else {
            for removed_path in removed {
                let event = WatcherEvent {
                    workspace_id,
                    kind: WatcherEventKind::Deleted,
                    path: CanonicalPath(removed_path),
                    old_path: None,
                    sequence: Self::now_sequence(state),
                };
                state.enqueue_watcher_event(event.clone());
                produced.push(event);
            }
            for added_path in added {
                let event = WatcherEvent {
                    workspace_id,
                    kind: WatcherEventKind::Created,
                    path: CanonicalPath(added_path),
                    old_path: None,
                    sequence: Self::now_sequence(state),
                };
                state.enqueue_watcher_event(event.clone());
                produced.push(event);
            }
        }

        for modified_path in modified {
            let event = WatcherEvent {
                workspace_id,
                kind: WatcherEventKind::Modified,
                path: CanonicalPath(modified_path),
                old_path: None,
                sequence: Self::now_sequence(state),
            };
            state.enqueue_watcher_event(event.clone());
            produced.push(event);
        }

        state.last_scan = current;
        Ok(produced)
    }

    fn pop_watcher_events(&self, state: &mut WorkspaceState) -> Vec<WatcherEvent> {
        let drained: Vec<_> = state.watcher_queue.drain(..).collect();
        state.last_watcher_signature.clear();
        drained
    }

    /// Open or re-open a workspace and populate shallow tree state.
    pub fn open_workspace(
        &self,
        request: WorkspaceOpenRequest,
    ) -> WorkspaceResult<WorkspaceOpened> {
        let requested_root = Path::new(&request.root_path.0);
        let root = self
            .fs
            .canonicalize_path(requested_root)
            .or_else(|_| self.fs.normalize_path(requested_root))
            .map_err(WorkspaceError::Platform)?;
        let principal_id = request.principal_id.clone();
        let workspace_id = WorkspaceId(stable_hash(&root.to_string_lossy()));
        let root_id = WorkspaceRootId(stable_hash(
            &(root.to_string_lossy().into_owned() + "-root"),
        ));
        let trust = request.trust.unwrap_or(WorkspaceTrustState::Unknown).into();

        let mut state_guard = self
            .state
            .lock()
            .map_err(|_| WorkspaceError::Internal("workspace state lock poisoned"))?;

        if let Some(existing) = state_guard.as_ref()
            && existing.workspace_id == workspace_id
        {
            return Ok(WorkspaceOpened {
                workspace_id,
                root_id: existing.workspace_root_id,
                generation: existing.generation,
                snapshot_id: existing.config_snapshot_id,
                correlation_id: request.correlation_id,
            });
        }

        let mut state = WorkspaceState::new(WorkspaceStateInit {
            workspace_id,
            workspace_root_id: root_id,
            principal_id,
            root_path: root.clone(),
            trust,
            snapshot_id: SnapshotId(stable_hash(
                &(root.to_string_lossy().into_owned() + "snapshot"),
            )),
            tree: Vec::new(),
            scan: HashMap::new(),
        });
        self.rebuild_tree_from_scan(&mut state)?;
        let snapshot_id = state.config_snapshot_id;

        let generated = WorkspaceOpened {
            workspace_id,
            root_id: state.workspace_root_id,
            generation: state.generation,
            snapshot_id,
            correlation_id: request.correlation_id,
        };

        *state_guard = Some(state);
        Ok(generated)
    }

    /// Resolve a file path inside the workspace and allocate a stable `FileIdentity`.
    pub fn resolve_file(
        &self,
        workspace_id: WorkspaceId,
        path: impl AsRef<str>,
    ) -> WorkspaceResult<FileIdentity> {
        let mut state_guard = self
            .state
            .lock()
            .map_err(|_| WorkspaceError::Internal("workspace state lock poisoned"))?;

        let state = state_guard
            .as_mut()
            .ok_or(WorkspaceError::WorkspaceMissing { workspace_id })?;
        if state.workspace_id != workspace_id {
            return Err(WorkspaceError::WorkspaceMissing { workspace_id });
        }

        self.resolve_identity_internal(state, path.as_ref())
    }

    /// Read file text via the workspace's filesystem service.
    pub fn read_file_text(
        &self,
        workspace_id: WorkspaceId,
        path: impl AsRef<str>,
    ) -> WorkspaceResult<String> {
        let state_guard = self
            .state
            .lock()
            .map_err(|_| WorkspaceError::Internal("workspace state lock poisoned"))?;
        let state = state_guard
            .as_ref()
            .ok_or(WorkspaceError::WorkspaceMissing { workspace_id })?;
        let path = self.canonicalize_candidate(state, path.as_ref())?;
        self.decision_for_workspace(state, "fs.read", Some(&path.to_string_lossy()))?;
        self.fs
            .read_text_file(&path)
            .map_err(WorkspaceError::Platform)
    }

    /// Open an existing text file and return mandatory save-precondition metadata.
    pub fn open_existing_file_text(
        &self,
        workspace_id: WorkspaceId,
        path: impl AsRef<str>,
    ) -> WorkspaceResult<OpenedFileText> {
        self.open_existing_file_text_with_causality(workspace_id, path, None, None)
    }

    /// Open an existing text file and emit read-failure metadata when causality is supplied.
    pub fn open_existing_file_text_with_causality(
        &self,
        workspace_id: WorkspaceId,
        path: impl AsRef<str>,
        correlation_id: Option<CorrelationId>,
        causality_id: Option<CausalityId>,
    ) -> WorkspaceResult<OpenedFileText> {
        let mut state_guard = self
            .state
            .lock()
            .map_err(|_| WorkspaceError::Internal("workspace state lock poisoned"))?;
        let state = state_guard
            .as_mut()
            .ok_or(WorkspaceError::WorkspaceMissing { workspace_id })?;
        if state.workspace_id != workspace_id {
            return Err(WorkspaceError::WorkspaceMissing { workspace_id });
        }
        self.open_existing_file_text_internal(state, path.as_ref(), correlation_id, causality_id)
    }

    /// Open a safe new-file buffer only when the caller explicitly requested create intent.
    pub fn open_new_file_text(
        &self,
        workspace_id: WorkspaceId,
        path: impl AsRef<str>,
    ) -> WorkspaceResult<OpenedFileText> {
        let mut state_guard = self
            .state
            .lock()
            .map_err(|_| WorkspaceError::Internal("workspace state lock poisoned"))?;
        let state = state_guard
            .as_mut()
            .ok_or(WorkspaceError::WorkspaceMissing { workspace_id })?;
        if state.workspace_id != workspace_id {
            return Err(WorkspaceError::WorkspaceMissing { workspace_id });
        }
        self.open_new_file_text_internal(state, path.as_ref())
    }

    /// Apply a save through mandatory proposal context and fail-closed fingerprint preconditions.
    pub fn save_file_with_proposal(&self, request: WorkspaceSaveRequest) -> WorkspaceSaveResult {
        let mut state_guard = match self.state.lock() {
            Ok(guard) => guard,
            Err(_) => {
                return Err(Self::failed_save_response(
                    &request,
                    "workspace state lock poisoned",
                ));
            }
        };
        let Some(state) = state_guard.as_mut() else {
            return Err(Self::failed_save_response(
                &request,
                "workspace is not open",
            ));
        };
        if state.workspace_id != request.workspace_id {
            return Err(Self::failed_save_response(
                &request,
                "workspace id does not match opened workspace",
            ));
        }

        let canonical = match self.canonicalize_candidate(state, &request.path.0) {
            Ok(path) => path,
            Err(err) => {
                return Err(Self::denied_save_response(
                    &request,
                    ProposalDenialReason::PolicyDenied,
                    err.to_string(),
                ));
            }
        };

        if request.payload_byte_len != request.text.len() as u64 {
            return Err(Self::failed_save_response(
                &request,
                "payload byte length does not match text payload",
            ));
        }

        if let Err(err) = self.decision_for_workspace(
            state,
            &request.required_capability.0,
            Some(&canonical.to_string_lossy()),
        ) {
            let sequence = Self::now_sequence(state);
            self.emit(security_denial_event(
                request.workspace_id,
                Some(request.file_id),
                Some(request.principal.clone()),
                &request.required_capability,
                request.correlation_id,
                request.causality_id,
                sequence,
                Some(&canonical.to_string_lossy()),
                err.to_string(),
            ));
            return Err(Self::denied_save_response(
                &request,
                ProposalDenialReason::CapabilityDenied,
                err.to_string(),
            ));
        }

        if state.generation != request.expected_workspace_generation {
            let sequence = Self::now_sequence(state);
            self.emit(stale_proposal_rejected_event(
                request.workspace_id,
                request.file_id,
                request.correlation_id,
                request.causality_id,
                sequence,
                request.proposal_id,
                ProposalStaleReason::WorkspaceGenerationMismatch,
            ));
            return Err(self.stale_save_response(
                &request,
                ProposalStaleReason::WorkspaceGenerationMismatch,
                None,
                "workspace generation changed before save",
            ));
        }

        let fallback_policy = NonAtomicSaveFallbackPolicy::Disabled;

        let actual_metadata = match self.fs.read_metadata(&canonical) {
            Ok(metadata) => Some(metadata),
            Err(PlatformError::NotFound { .. }) => {
                if request.expected_file_content_version == FileContentVersion(0)
                    && request.expected_fingerprint.value.contains("hash=new:")
                {
                    None
                } else {
                    let identity = FileIdentity {
                        file_id: request.file_id,
                        workspace_id: request.workspace_id,
                        canonical_path: request.path.clone(),
                        content_version: FileContentVersion(0),
                        content_hash: None,
                    };
                    return Err(self.conflict_save_response(
                        state,
                        &request,
                        identity,
                        None,
                        "file disappeared from disk before save",
                    ));
                }
            }
            Err(err) => {
                return Err(Self::failed_save_response(&request, err.to_string()));
            }
        };
        let actual_fingerprint = match actual_metadata.as_ref() {
            Some(metadata) if metadata.is_file() => {
                match FileFingerprint::from_metadata(&canonical, self.fs.as_ref(), metadata) {
                    Ok(fingerprint) => fingerprint,
                    Err(err) => {
                        return Err(Self::failed_save_response(&request, err.to_string()));
                    }
                }
            }
            Some(_) => FileFingerprint::from_dir(),
            None => FileFingerprint::for_new_file(&canonical),
        };
        let actual_protocol_fingerprint = actual_fingerprint.to_protocol();

        let actual_identity = if let Some(metadata) = actual_metadata.as_ref() {
            self.file_identity_from_platform_metadata(
                state,
                &canonical,
                &actual_fingerprint,
                metadata,
            )
        } else {
            self.file_identity_for_new_path(state, &canonical, &actual_fingerprint)
        };

        if actual_identity.file_id != request.file_id {
            return Err(self.conflict_save_response(
                state,
                &request,
                actual_identity,
                Some(actual_protocol_fingerprint),
                "file identity changed before save",
            ));
        }

        if actual_identity.content_version != request.expected_file_content_version {
            let actual_context = devil_protocol::VersionContext {
                file_version: actual_identity.content_version,
                buffer_version: request.buffer_version,
                snapshot_id: request.snapshot_id,
                generation: state.generation,
                file_content_version: actual_identity.content_version,
                workspace_generation: state.generation,
                fingerprint: Some(actual_protocol_fingerprint.clone()),
                file_length: actual_fingerprint.size,
                modified_at: actual_fingerprint.modified,
            };
            let sequence = Self::now_sequence(state);
            self.emit(stale_proposal_rejected_event(
                request.workspace_id,
                request.file_id,
                request.correlation_id,
                request.causality_id,
                sequence,
                request.proposal_id,
                ProposalStaleReason::FileContentVersionMismatch,
            ));
            return Err(self.stale_save_response(
                &request,
                ProposalStaleReason::FileContentVersionMismatch,
                Some(actual_context),
                "file content version changed before save",
            ));
        }

        if actual_protocol_fingerprint != request.expected_fingerprint {
            return Err(self.conflict_save_response(
                state,
                &request,
                actual_identity,
                Some(actual_protocol_fingerprint),
                "disk fingerprint changed before save",
            ));
        }

        if let Err(err) = self.fs.write_text_file_atomic(&canonical, &request.text) {
            let fallback_status = match fallback_policy {
                NonAtomicSaveFallbackPolicy::Disabled => {
                    "non-atomic fallback disabled; failing closed"
                }
            };
            let sequence = Self::now_sequence(state);
            self.emit(fallback_denied_event(
                request.workspace_id,
                request.file_id,
                request.correlation_id,
                request.causality_id,
                sequence,
                fallback_status,
            ));
            return Err(Self::failed_save_response(
                &request,
                format!("{err}; {fallback_status}"),
            ));
        }

        let metadata = match self.fs.read_metadata(&canonical) {
            Ok(metadata) => metadata,
            Err(err) => {
                return Err(Self::failed_save_response(&request, err.to_string()));
            }
        };
        let new_fingerprint =
            match FileFingerprint::from_metadata(&canonical, self.fs.as_ref(), &metadata) {
                Ok(fingerprint) => fingerprint,
                Err(err) => return Err(Self::failed_save_response(&request, err.to_string())),
            };
        let new_identity = self.file_identity_from_platform_metadata(
            state,
            &canonical,
            &new_fingerprint,
            &metadata,
        );
        let metadata = self
            .metadata_for_identity(state, new_identity.file_id)
            .unwrap_or_else(|| FileMetadata {
                canonical_path: new_identity.canonical_path.clone(),
                file_id: Some(new_identity.file_id),
                workspace_id: Some(new_identity.workspace_id),
                kind: FileKind::File,
                size_bytes: new_fingerprint.size,
                modified_at: new_fingerprint.modified,
                read_only: new_fingerprint.read_only,
                permissions: None,
                hash: new_fingerprint.hash.clone(),
                fingerprint: Some(new_fingerprint.to_protocol()),
                content_version: Some(new_identity.content_version),
                workspace_generation: Some(state.generation),
                schema_version: 1,
            });
        let key = new_identity.canonical_path.0.clone();
        state.last_scan.insert(key.clone(), new_fingerprint.clone());
        if !state
            .tree
            .iter()
            .any(|node| node.identity.file_id == new_identity.file_id)
        {
            state.tree.push(FileTreeNode {
                identity: new_identity.clone(),
                name: key
                    .rsplit(['/', '\\'])
                    .next()
                    .unwrap_or("unknown")
                    .to_string(),
                children: Vec::new(),
                metadata: Some(metadata.clone()),
            });
        } else if let Some(node) = state
            .tree
            .iter_mut()
            .find(|node| node.identity.file_id == new_identity.file_id)
        {
            node.identity = new_identity.clone();
            node.metadata = Some(metadata.clone());
        }
        state.generation = WorkspaceGeneration(state.generation.0.saturating_add(1));
        state.config.captured_at = TimestampMillis(now_millis());

        let transition =
            Self::save_transition(&request, ProposalLifecycleState::Applied, Vec::new());
        Ok(WorkspaceSaveApplied {
            identity: new_identity.clone(),
            fingerprint: new_fingerprint.to_protocol(),
            file_content_version: new_identity.content_version,
            workspace_generation: state.generation,
            modified_at: metadata.modified_at,
            file_length: metadata.size_bytes,
            used_non_atomic_fallback: false,
            fallback_status: Some("atomic-write-only; non-atomic fallback disabled".to_string()),
            response: ProposalResponse::Applied(transition),
        })
    }

    /// Read current cached shallow tree.
    pub fn tree_snapshot(&self) -> WorkspaceResult<Vec<FileTreeNode>> {
        let state = self
            .state
            .lock()
            .map_err(|_| WorkspaceError::Internal("workspace state lock poisoned"))?;
        Ok(state
            .as_ref()
            .map(|state| state.tree.clone())
            .unwrap_or_default())
    }

    /// Set workspace trust state and bump config snapshot generation.
    pub fn set_trust(&self, workspace_id: WorkspaceId, trust: TrustState) -> WorkspaceResult<()> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| WorkspaceError::Internal("workspace state lock poisoned"))?;
        let state = state
            .as_mut()
            .ok_or(WorkspaceError::WorkspaceMissing { workspace_id })?;
        if state.workspace_id != workspace_id {
            return Err(WorkspaceError::WorkspaceMissing { workspace_id });
        }

        state.trust = trust;
        state.config.trust_state = trust_to_protocol(trust);
        state.config.captured_at = TimestampMillis(now_millis());
        Ok(())
    }

    /// Returns the current workspace root path if a workspace is loaded.
    pub fn current_workspace_root(&self) -> WorkspaceResult<PathBuf> {
        let state = self
            .state
            .lock()
            .map_err(|_| WorkspaceError::Internal("workspace state lock poisoned"))?;
        let state = state.as_ref().ok_or(WorkspaceError::WorkspaceMissing {
            workspace_id: WorkspaceId(0),
        })?;

        Ok(state.root_path.clone())
    }

    /// Drain debounced watcher events.
    pub fn poll_watcher_events(
        &self,
        workspace_id: WorkspaceId,
    ) -> WorkspaceResult<Vec<WatcherEvent>> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| WorkspaceError::Internal("workspace state lock poisoned"))?;
        let state = state
            .as_mut()
            .ok_or(WorkspaceError::WorkspaceMissing { workspace_id })?;

        if state.workspace_id != workspace_id {
            return Err(WorkspaceError::WorkspaceMissing { workspace_id });
        }

        let mut produced = self.collect_watcher_events(state)?;
        let queued = self.pop_watcher_events(state);
        produced.extend(queued);
        Ok(produced)
    }

    fn protocol_error(error: WorkspaceError) -> ProtocolError {
        ProtocolError {
            code: "workspace_error".to_string(),
            message: error.to_string(),
        }
    }
}

impl devil_protocol::WorkspacePort for WorkspaceActor {
    /// Handle protocol workspace request messages.
    fn handle(&self, request: WorkspaceRequest) -> ProtocolResult<WorkspaceResponse> {
        let response = match request {
            WorkspaceRequest::Open(request) => {
                let opened = self.open_workspace(request).map_err(Self::protocol_error)?;

                WorkspaceResponse::Opened(opened)
            }
            WorkspaceRequest::Close(WorkspaceCloseRequest {
                workspace_id,
                correlation_id: _,
                principal_id: _,
            }) => {
                let mut guard = self.state.lock().map_err(|_| {
                    Self::protocol_error(WorkspaceError::Internal("workspace state lock poisoned"))
                })?;
                let existing = guard.take();
                if let Some(state) = existing {
                    if state.workspace_id == workspace_id {
                        WorkspaceResponse::Closed(WorkspaceClosed {
                            workspace_id,
                            correlation_id: CorrelationId(0),
                            success: true,
                        })
                    } else {
                        *guard = Some(state);
                        WorkspaceResponse::Closed(WorkspaceClosed {
                            workspace_id,
                            correlation_id: CorrelationId(0),
                            success: false,
                        })
                    }
                } else {
                    return Err(ProtocolError::unsupported("workspace not open"));
                }
            }
            WorkspaceRequest::ResolveFile { workspace_id, path } => {
                let identity = self
                    .resolve_file(workspace_id, path.0)
                    .map_err(Self::protocol_error)?;
                WorkspaceResponse::ResolvedFile(identity)
            }
            WorkspaceRequest::ReadConfig(workspace_id) => {
                let guard = self.state.lock().map_err(|_| {
                    Self::protocol_error(WorkspaceError::Internal("workspace state lock poisoned"))
                })?;
                let state = guard.as_ref().ok_or_else(|| {
                    Self::protocol_error(WorkspaceError::WorkspaceMissing { workspace_id })
                })?;
                if state.workspace_id != workspace_id {
                    return Err(Self::protocol_error(WorkspaceError::WorkspaceMissing {
                        workspace_id,
                    }));
                }
                WorkspaceResponse::Config(state.config.clone())
            }
            WorkspaceRequest::ReadTree(workspace_id) => {
                let guard = self.state.lock().map_err(|_| {
                    Self::protocol_error(WorkspaceError::Internal("workspace state lock poisoned"))
                })?;
                let state = guard.as_ref().ok_or_else(|| {
                    Self::protocol_error(WorkspaceError::WorkspaceMissing { workspace_id })
                })?;
                if state.workspace_id != workspace_id {
                    return Err(Self::protocol_error(WorkspaceError::WorkspaceMissing {
                        workspace_id,
                    }));
                }

                WorkspaceResponse::Tree(state.tree.clone())
            }
            WorkspaceRequest::ApplyTreeDelta(delta) => {
                let mut guard = self.state.lock().map_err(|_| {
                    Self::protocol_error(WorkspaceError::Internal("workspace state lock poisoned"))
                })?;
                let state = guard.as_mut().ok_or_else(|| {
                    Self::protocol_error(WorkspaceError::WorkspaceMissing {
                        workspace_id: delta.workspace_id,
                    })
                })?;
                if state.workspace_id != delta.workspace_id {
                    return Err(Self::protocol_error(WorkspaceError::WorkspaceMissing {
                        workspace_id: delta.workspace_id,
                    }));
                }

                self.apply_tree_delta_internal(state, delta)
                    .map_err(Self::protocol_error)?;
                WorkspaceResponse::Tree(state.tree.clone())
            }
        };

        Ok(response)
    }
}

impl devil_protocol::ProjectInfoPort for WorkspaceActor {
    fn resolve_project_for_file(
        &self,
        query: devil_protocol::ProjectInfoQuery,
    ) -> Result<devil_protocol::ProjectInfo, devil_protocol::ProjectServiceError> {
        let mut state_guard =
            self.state
                .lock()
                .map_err(|_| devil_protocol::ProjectServiceError {
                    code: "workspace_lock_poisoned".to_string(),
                    message: "project service is unavailable".to_string(),
                })?;

        let state = state_guard
            .as_mut()
            .ok_or_else(|| devil_protocol::ProjectServiceError {
                code: "workspace_not_open".to_string(),
                message: "no workspace opened in service".to_string(),
            })?;

        let identity = self
            .resolve_identity_internal(state, &query.file_path)
            .map_err(|err| devil_protocol::ProjectServiceError {
                code: "resolve_error".to_string(),
                message: err.to_string(),
            })?;

        Ok(devil_protocol::ProjectInfo {
            project_id: ProjectId(state.workspace_id.0),
            root_path: state.root_path.to_string_lossy().into_owned(),
            language_id: None,
            file_id: identity.file_id,
        })
    }

    fn notify_editor_transaction(
        &self,
        event: devil_protocol::EditorTransactionEvent,
    ) -> Result<(), devil_protocol::ProjectServiceError> {
        let _ = &event;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    use devil_platform::{
        FileSystemFingerprint, FileSystemMetadata, NativeFileSystem, NativeWatcherService,
    };
    use devil_protocol::{
        WorkspaceOpenRequest, WorkspaceOpened, WorkspacePort, WorkspaceRequest, WorkspaceTrustState,
    };
    use devil_security::{DenyByDefaultBroker, SecurityPolicy};

    static TEST_TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

    fn next_test_temp_suffix() -> u64 {
        TEST_TEMP_COUNTER.fetch_add(1, Ordering::Relaxed)
    }

    #[derive(Debug)]
    struct FailingAtomicFs {
        root: PathBuf,
        atomic_error: PlatformError,
    }

    impl FailingAtomicFs {
        fn new(root: PathBuf, atomic_error: PlatformError) -> Self {
            Self { root, atomic_error }
        }
    }

    impl PathNormalizationService for FailingAtomicFs {
        fn normalize_path(&self, path: &Path) -> Result<PathBuf, PlatformError> {
            NativeFileSystem.normalize_path(path)
        }

        fn canonicalize_path(&self, path: &Path) -> Result<PathBuf, PlatformError> {
            NativeFileSystem.canonicalize_path(path)
        }

        fn is_within_base(&self, base: &Path, candidate: &Path) -> Result<bool, PlatformError> {
            NativeFileSystem.is_within_base(base, candidate)
        }
    }

    impl FileSystemService for FailingAtomicFs {
        fn read_text_file(&self, path: &Path) -> Result<String, PlatformError> {
            NativeFileSystem.read_text_file(path)
        }

        fn write_text_file(&self, path: &Path, text: &str) -> Result<(), PlatformError> {
            NativeFileSystem.write_text_file(path, text)
        }

        fn write_text_file_atomic(&self, _path: &Path, _text: &str) -> Result<(), PlatformError> {
            Err(match &self.atomic_error {
                PlatformError::UnsupportedOperation {
                    operation,
                    path,
                    reason,
                } => PlatformError::UnsupportedOperation {
                    operation: operation.clone(),
                    path: path.clone(),
                    reason: reason.clone(),
                },
                PlatformError::PermissionDenied { operation, path } => {
                    PlatformError::PermissionDenied {
                        operation: operation.clone(),
                        path: path.clone(),
                    }
                }
                _ => PlatformError::UnsupportedOperation {
                    operation: "atomic write".to_string(),
                    path: self.root.join("unknown"),
                    reason: self.atomic_error.to_string(),
                },
            })
        }

        fn read_metadata(&self, path: &Path) -> Result<FileSystemMetadata, PlatformError> {
            NativeFileSystem.read_metadata(path)
        }

        fn read_fingerprint(&self, path: &Path) -> Result<FileSystemFingerprint, PlatformError> {
            NativeFileSystem.read_fingerprint(path)
        }

        fn stable_hash(&self, bytes: &[u8]) -> String {
            NativeFileSystem.stable_hash(bytes)
        }

        fn stable_hash_file(&self, path: &Path) -> Result<String, PlatformError> {
            NativeFileSystem.stable_hash_file(path)
        }

        fn modified_timestamp(&self, path: &Path) -> Result<Option<u64>, PlatformError> {
            NativeFileSystem.modified_timestamp(path)
        }

        fn file_length(&self, path: &Path) -> Result<u64, PlatformError> {
            NativeFileSystem.file_length(path)
        }

        fn list_directory(&self, path: &Path) -> Result<Vec<PathBuf>, PlatformError> {
            NativeFileSystem.list_directory(path)
        }
    }

    #[derive(Debug)]
    struct InconsistentMetadataFs {
        metadata_calls: Mutex<u64>,
    }

    impl InconsistentMetadataFs {
        fn new(_root: PathBuf) -> Self {
            Self {
                metadata_calls: Mutex::new(0),
            }
        }
    }

    impl PathNormalizationService for InconsistentMetadataFs {
        fn normalize_path(&self, path: &Path) -> Result<PathBuf, PlatformError> {
            NativeFileSystem.normalize_path(path)
        }

        fn canonicalize_path(&self, path: &Path) -> Result<PathBuf, PlatformError> {
            NativeFileSystem.canonicalize_path(path)
        }

        fn is_within_base(&self, base: &Path, candidate: &Path) -> Result<bool, PlatformError> {
            NativeFileSystem.is_within_base(base, candidate)
        }
    }

    impl FileSystemService for InconsistentMetadataFs {
        fn read_text_file(&self, path: &Path) -> Result<String, PlatformError> {
            NativeFileSystem.read_text_file(path)
        }

        fn write_text_file(&self, path: &Path, text: &str) -> Result<(), PlatformError> {
            NativeFileSystem.write_text_file(path, text)
        }

        fn write_text_file_atomic(&self, path: &Path, text: &str) -> Result<(), PlatformError> {
            NativeFileSystem.write_text_file_atomic(path, text)
        }

        fn read_metadata(&self, path: &Path) -> Result<FileSystemMetadata, PlatformError> {
            let mut calls = self.metadata_calls.lock().expect("metadata calls lock");
            *calls = calls.saturating_add(1);
            let mut metadata = NativeFileSystem.read_metadata(path)?;
            if *calls > 1 && metadata.is_file() {
                metadata.length = metadata.length.saturating_add(1);
            }
            Ok(metadata)
        }

        fn read_fingerprint(&self, path: &Path) -> Result<FileSystemFingerprint, PlatformError> {
            let metadata = self.read_metadata(path)?;
            let stable_hash = if metadata.is_file() {
                Some(NativeFileSystem.stable_hash_file(path)?)
            } else {
                None
            };
            Ok(FileSystemFingerprint {
                path: path.to_path_buf(),
                algorithm: "inconsistent-test".to_string(),
                kind: metadata.kind,
                length: metadata
                    .is_file()
                    .then_some(metadata.length.saturating_add(1)),
                modified_at: metadata.modified_at,
                stable_hash,
                read_only: metadata.read_only,
            })
        }

        fn stable_hash(&self, bytes: &[u8]) -> String {
            NativeFileSystem.stable_hash(bytes)
        }

        fn stable_hash_file(&self, path: &Path) -> Result<String, PlatformError> {
            NativeFileSystem.stable_hash_file(path)
        }

        fn modified_timestamp(&self, path: &Path) -> Result<Option<u64>, PlatformError> {
            NativeFileSystem.modified_timestamp(path)
        }

        fn file_length(&self, path: &Path) -> Result<u64, PlatformError> {
            NativeFileSystem.file_length(path)
        }

        fn list_directory(&self, path: &Path) -> Result<Vec<PathBuf>, PlatformError> {
            NativeFileSystem.list_directory(path)
        }
    }

    struct FakeWatcher;
    impl WatcherService for FakeWatcher {
        fn snapshot(
            &self,
            _workspace_id: WorkspaceId,
            _path: &Path,
        ) -> Result<Vec<WatcherEvent>, PlatformError> {
            Err(PlatformError::WatcherOverflow {
                path: std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
                context: "fake overflow".to_string(),
            })
        }
    }

    fn root_workspace() -> (WorkspaceActor, WorkspaceOpened, PrincipalId) {
        let fs: Arc<ProjectFilesystem> = Arc::new(NativeFileSystem);
        let actor = WorkspaceActor::new(
            fs,
            Arc::new(NativeWatcherService),
            DenyByDefaultBroker::default(),
        );
        let req = WorkspaceOpenRequest {
            correlation_id: CorrelationId(1),
            principal_id: PrincipalId("main".to_string()),
            root_path: CanonicalPath(
                std::env::current_dir()
                    .expect("cwd")
                    .to_string_lossy()
                    .into_owned(),
            ),
            trust: Some(WorkspaceTrustState::Trusted),
        };
        let opened = actor.open_workspace(req).expect("open");
        (actor, opened, PrincipalId("main".to_string()))
    }

    fn temporary_workspace(
        trust: WorkspaceTrustState,
    ) -> (WorkspaceActor, WorkspaceOpened, PrincipalId, PathBuf) {
        let base = std::env::temp_dir();
        let unique = format!(
            "devil-project-test-{}-{}-{}",
            std::process::id(),
            now_millis(),
            next_test_temp_suffix()
        );
        let root = base.join(unique);
        std::fs::create_dir_all(&root).expect("create temporary workspace directory");
        let canonical_root =
            std::fs::canonicalize(&root).expect("canonicalize temp workspace root");
        let canonical_root = canonical_root.to_string_lossy().into_owned();

        let mut policy = SecurityPolicy::default();
        policy.path_policy.readable_roots = vec![canonical_root.clone()];
        policy.path_policy.writable_roots = vec![canonical_root];

        let actor = WorkspaceActor::new(
            Arc::new(NativeFileSystem),
            Arc::new(NativeWatcherService),
            DenyByDefaultBroker::new(
                policy,
                devil_protocol::CapabilityNamespace("test".to_string()),
            ),
        );
        let req = WorkspaceOpenRequest {
            correlation_id: CorrelationId(3),
            principal_id: PrincipalId("temp-principal".to_string()),
            root_path: CanonicalPath(root.to_string_lossy().into_owned()),
            trust: Some(trust),
        };

        let opened = actor.open_workspace(req).expect("open temporary workspace");
        (
            actor,
            opened,
            PrincipalId("temp-principal".to_string()),
            root,
        )
    }

    fn save_new_file_for_tests(
        actor: &WorkspaceActor,
        workspace_id: WorkspaceId,
        path: &str,
        text: &str,
    ) -> WorkspaceSaveResult {
        let opened = actor
            .open_new_file_text(workspace_id, path)
            .expect("open new file for proposal save");
        actor.save_file_with_proposal(WorkspaceSaveRequest {
            workspace_id,
            proposal_id: ProposalId(1),
            principal: PrincipalId("temp-principal".to_string()),
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
            text: text.to_string(),
        })
    }

    #[test]
    fn open_and_resolve_path_stays_inside_root() {
        let (actor, opened, _) = root_workspace();
        let root = std::env::current_dir().expect("cwd");
        let child = root.join("Cargo.toml");
        let identity = actor
            .resolve_file(opened.workspace_id, child.to_string_lossy())
            .expect("resolved");
        assert_eq!(identity.workspace_id, opened.workspace_id);
    }

    #[test]
    fn request_port_open_roundtrip_works() {
        let (actor, _, principal) = root_workspace();
        let response = actor
            .handle(WorkspaceRequest::ReadConfig(WorkspaceId(0)))
            .expect_err("read config should fail when workspace id mismatch");
        assert_eq!(response.code, "workspace_error");
        let _ = principal;
    }

    #[test]
    fn watcher_overflow_marks_recovery() {
        let fs: Arc<ProjectFilesystem> = Arc::new(NativeFileSystem);
        let actor = WorkspaceActor::new(fs, Arc::new(FakeWatcher), DenyByDefaultBroker::default());
        let req = WorkspaceOpenRequest {
            correlation_id: CorrelationId(1),
            principal_id: PrincipalId("u".to_string()),
            root_path: CanonicalPath(
                std::env::current_dir()
                    .expect("cwd")
                    .to_string_lossy()
                    .into_owned(),
            ),
            trust: Some(WorkspaceTrustState::Trusted),
        };
        let opened = actor.open_workspace(req).expect("open");
        actor.set_watchers_for_tests();
        let events = actor
            .poll_watcher_events(opened.workspace_id)
            .expect("poll");
        assert!(!events.is_empty());
        assert!(matches!(
            events[0].kind,
            devil_protocol::WatcherEventKind::Overflow
        ));
    }

    #[test]
    fn path_policy_rejects_outside_root_reads_and_writes() {
        let (actor, opened, _, root) = temporary_workspace(WorkspaceTrustState::Trusted);
        let outside = root
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join("outside-policy-check.txt");

        let read_err = actor
            .resolve_file(opened.workspace_id, outside.to_string_lossy())
            .expect_err("resolve should fail for outside root");
        assert!(matches!(read_err, WorkspaceError::PathOutsideRoot { .. }));

        let write_err = actor
            .open_new_file_text(opened.workspace_id, outside.to_string_lossy())
            .expect_err("new-file open should fail for outside root");
        assert!(matches!(write_err, WorkspaceError::PathOutsideRoot { .. }));

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn write_is_blocked_for_untrusted_workspace() {
        let (actor, opened, _, root) = temporary_workspace(WorkspaceTrustState::Untrusted);
        let file_path = "blocked.txt";

        let write_err = actor
            .open_new_file_text(opened.workspace_id, file_path)
            .expect_err("untrusted workspace should not be able to create a new-file buffer");

        assert!(matches!(write_err, WorkspaceError::SecurityDenied { .. }));

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn read_write_roundtrip_from_workspace_apis() {
        let (actor, opened, _, root) = temporary_workspace(WorkspaceTrustState::Trusted);
        let file_path = "integration.txt";

        let applied = save_new_file_for_tests(&actor, opened.workspace_id, file_path, "hello\n")
            .expect("proposal save via actor should succeed");
        assert!(!applied.used_non_atomic_fallback);
        assert_eq!(
            applied.fallback_status.as_deref(),
            Some("atomic-write-only; non-atomic fallback disabled")
        );

        let actual = actor
            .read_file_text(opened.workspace_id, file_path)
            .expect("read via actor should succeed");
        assert_eq!(actual, "hello\n");

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn atomic_write_failure_fails_closed_without_plain_write() {
        let base = std::env::temp_dir();
        let unique = format!(
            "devil-project-atomic-failure-{}-{}-{}",
            std::process::id(),
            now_millis(),
            next_test_temp_suffix()
        );
        let root = base.join(unique);
        std::fs::create_dir_all(&root).expect("create atomic failure workspace");
        let canonical_root = std::fs::canonicalize(&root).expect("canonicalize root");
        let target = canonical_root.join("atomic-failure.txt");
        std::fs::write(&target, "seed").expect("seed target");

        let mut policy = SecurityPolicy::default();
        policy.path_policy.readable_roots = vec![canonical_root.to_string_lossy().into_owned()];
        policy.path_policy.writable_roots = vec![canonical_root.to_string_lossy().into_owned()];

        let fs: Arc<ProjectFilesystem> = Arc::new(FailingAtomicFs::new(
            canonical_root.clone(),
            PlatformError::UnsupportedOperation {
                operation: "atomic replace".to_string(),
                path: target.clone(),
                reason: "synthetic unsupported atomic replacement".to_string(),
            },
        ));
        let actor = WorkspaceActor::new(
            fs,
            Arc::new(NativeWatcherService),
            DenyByDefaultBroker::new(
                policy,
                devil_protocol::CapabilityNamespace("test".to_string()),
            ),
        );
        let opened = actor
            .open_workspace(WorkspaceOpenRequest {
                correlation_id: CorrelationId(11),
                principal_id: PrincipalId("temp-principal".to_string()),
                root_path: CanonicalPath(canonical_root.to_string_lossy().into_owned()),
                trust: Some(WorkspaceTrustState::Trusted),
            })
            .expect("open workspace");
        let opened_file = actor
            .open_existing_file_text(opened.workspace_id, target.to_string_lossy())
            .expect("open target");

        let response = actor
            .save_file_with_proposal(WorkspaceSaveRequest {
                workspace_id: opened.workspace_id,
                proposal_id: ProposalId(99),
                principal: PrincipalId("temp-principal".to_string()),
                required_capability: CapabilityId("fs.write".to_string()),
                file_id: opened_file.identity.file_id,
                path: opened_file.identity.canonical_path,
                expected_fingerprint: opened_file.fingerprint,
                expected_file_content_version: opened_file.file_content_version,
                expected_workspace_generation: opened_file.workspace_generation,
                buffer_version: BufferVersion(2),
                snapshot_id: SnapshotId(2),
                payload_byte_len: "replacement".len() as u64,
                correlation_id: CorrelationId(99),
                causality_id: CausalityId(Uuid::now_v7()),
                text: "replacement".to_string(),
            })
            .expect_err("atomic write failure should fail closed");

        assert_eq!(
            std::fs::read_to_string(&target).expect("target content"),
            "seed"
        );
        match response {
            ProposalResponse::Failed { transition, .. } => {
                assert!(transition.diagnostics.iter().any(|diagnostic| {
                    diagnostic
                        .message
                        .contains("non-atomic fallback disabled; failing closed")
                }));
            }
            other => panic!("expected failed response, got {other:?}"),
        }

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn permission_failure_from_platform_is_reported_without_plain_write() {
        let base = std::env::temp_dir();
        let unique = format!(
            "devil-project-permission-failure-{}-{}-{}",
            std::process::id(),
            now_millis(),
            next_test_temp_suffix()
        );
        let root = base.join(unique);
        std::fs::create_dir_all(&root).expect("create permission failure workspace");
        let canonical_root = std::fs::canonicalize(&root).expect("canonicalize root");
        let target = canonical_root.join("permission-failure.txt");
        std::fs::write(&target, "seed").expect("seed target");

        let mut policy = SecurityPolicy::default();
        policy.path_policy.readable_roots = vec![canonical_root.to_string_lossy().into_owned()];
        policy.path_policy.writable_roots = vec![canonical_root.to_string_lossy().into_owned()];

        let fs: Arc<ProjectFilesystem> = Arc::new(FailingAtomicFs::new(
            canonical_root.clone(),
            PlatformError::PermissionDenied {
                operation: "atomic write".to_string(),
                path: target.clone(),
            },
        ));
        let actor = WorkspaceActor::new(
            fs,
            Arc::new(NativeWatcherService),
            DenyByDefaultBroker::new(
                policy,
                devil_protocol::CapabilityNamespace("test".to_string()),
            ),
        );
        let opened = actor
            .open_workspace(WorkspaceOpenRequest {
                correlation_id: CorrelationId(12),
                principal_id: PrincipalId("temp-principal".to_string()),
                root_path: CanonicalPath(canonical_root.to_string_lossy().into_owned()),
                trust: Some(WorkspaceTrustState::Trusted),
            })
            .expect("open workspace");
        let opened_file = actor
            .open_existing_file_text(opened.workspace_id, target.to_string_lossy())
            .expect("open target");

        let response = actor
            .save_file_with_proposal(WorkspaceSaveRequest {
                workspace_id: opened.workspace_id,
                proposal_id: ProposalId(100),
                principal: PrincipalId("temp-principal".to_string()),
                required_capability: CapabilityId("fs.write".to_string()),
                file_id: opened_file.identity.file_id,
                path: opened_file.identity.canonical_path,
                expected_fingerprint: opened_file.fingerprint,
                expected_file_content_version: opened_file.file_content_version,
                expected_workspace_generation: opened_file.workspace_generation,
                buffer_version: BufferVersion(2),
                snapshot_id: SnapshotId(2),
                payload_byte_len: "replacement".len() as u64,
                correlation_id: CorrelationId(100),
                causality_id: CausalityId(Uuid::now_v7()),
                text: "replacement".to_string(),
            })
            .expect_err("permission failure should fail closed");

        assert_eq!(
            std::fs::read_to_string(&target).expect("target content"),
            "seed"
        );
        match response {
            ProposalResponse::Failed { transition, .. } => {
                assert!(
                    transition
                        .diagnostics
                        .iter()
                        .any(|diagnostic| diagnostic.message.contains("permission denied"))
                );
            }
            other => panic!("expected failed response, got {other:?}"),
        }

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn metadata_inconsistency_blocks_open_before_save_preconditions() {
        let base = std::env::temp_dir();
        let unique = format!(
            "devil-project-metadata-inconsistent-{}-{}-{}",
            std::process::id(),
            now_millis(),
            next_test_temp_suffix()
        );
        let root = base.join(unique);
        std::fs::create_dir_all(&root).expect("create metadata workspace");
        let canonical_root = std::fs::canonicalize(&root).expect("canonicalize root");
        let target = canonical_root.join("metadata.txt");
        std::fs::write(&target, "seed").expect("seed target");

        let mut policy = SecurityPolicy::default();
        policy.path_policy.readable_roots = vec![canonical_root.to_string_lossy().into_owned()];
        policy.path_policy.writable_roots = vec![canonical_root.to_string_lossy().into_owned()];

        let actor = WorkspaceActor::new(
            Arc::new(InconsistentMetadataFs::new(canonical_root.clone())),
            Arc::new(NativeWatcherService),
            DenyByDefaultBroker::new(
                policy,
                devil_protocol::CapabilityNamespace("test".to_string()),
            ),
        );
        let err = actor
            .open_workspace(WorkspaceOpenRequest {
                correlation_id: CorrelationId(13),
                principal_id: PrincipalId("temp-principal".to_string()),
                root_path: CanonicalPath(canonical_root.to_string_lossy().into_owned()),
                trust: Some(WorkspaceTrustState::Trusted),
            })
            .expect_err("metadata inconsistency should block workspace open");
        assert!(matches!(
            err,
            WorkspaceError::Platform(PlatformError::MetadataInconsistent { .. })
        ));

        let _ = std::fs::remove_dir_all(&root);
    }

    impl WorkspaceActor {
        fn set_watchers_for_tests(&self) {
            let mut state = self.state.lock().expect("lock");
            if let Some(state) = state.as_mut() {
                state.watcher_sequence += 1;
            }
        }
    }
}
