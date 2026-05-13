//! Project model: workspace, file tree, file watcher, and trust-aware VFS resolution.

#![warn(missing_docs)]

use std::collections::{HashMap, HashSet, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use devil_platform::{FileSystemService, PathNormalizationService, PlatformError, WatcherService};
use devil_protocol::{
    CanonicalPath, CorrelationId, EventSequence, FileContentVersion, FileId, FileIdentity,
    FileKind, FileMetadata, FileTreeDelta, FileTreeDeltaOp, FileTreeNode, PrincipalId, ProjectId,
    ProtocolError, ProtocolResult, SnapshotId, TimestampMillis, WatcherEvent, WatcherEventKind,
    WorkspaceCloseRequest, WorkspaceClosed, WorkspaceConfigSnapshot, WorkspaceGeneration,
    WorkspaceId, WorkspaceOpenRequest, WorkspaceOpened, WorkspaceRequest, WorkspaceResponse,
    WorkspaceRootId, WorkspaceTrustState,
};
use devil_security::{DenyByDefaultBroker, TrustState};
use thiserror::Error;

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

fn platform_error_from_io(
    operation: impl Into<String>,
    path: impl Into<PathBuf>,
    source: std::io::Error,
) -> PlatformError {
    let operation = operation.into();
    let path = path.into();

    match source.kind() {
        std::io::ErrorKind::PermissionDenied => PlatformError::PermissionDenied { operation, path },
        std::io::ErrorKind::NotFound => PlatformError::NotFound { operation, path },
        std::io::ErrorKind::InvalidData => PlatformError::Encoding {
            operation,
            path,
            source,
        },
        _ => {
            let message = source.to_string().to_ascii_lowercase();
            if message.contains("too many links")
                || message.contains("symlink")
                || message.contains("symbolic link")
                || message.contains("circular")
                || message.contains("loop")
            {
                PlatformError::SymlinkLoop { operation, path }
            } else if message.contains("filename too long")
                || message.contains("name too long")
                || message.contains("path too long")
                || message.contains("too long")
            {
                PlatformError::PathTooLong { operation, path }
            } else {
                PlatformError::Io {
                    operation,
                    path,
                    source,
                }
            }
        }
    }
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

#[derive(Debug, Clone)]
struct FileFingerprint {
    size: Option<u64>,
    modified: Option<TimestampMillis>,
    hash: Option<String>,
    read_only: bool,
}

impl FileFingerprint {
    fn from_path(path: &Path, fs: &ProjectFilesystem) -> Result<Self, WorkspaceError> {
        let metadata = std::fs::metadata(path).map_err(|err| {
            WorkspaceError::Platform(platform_error_from_io("metadata", path, err))
        })?;

        let size = metadata.len();
        let read_only = metadata.permissions().readonly();
        let modified = metadata.modified().ok().map(|value| {
            TimestampMillis(
                value
                    .duration_since(UNIX_EPOCH)
                    .map_or(0, |d| d.as_millis() as u64),
            )
        });
        let hash = if metadata.is_file() && size <= LARGE_FILE_BYTES {
            fs.hash_file(path).ok()
        } else {
            None
        };

        Ok(Self {
            size: Some(size),
            modified,
            hash,
            read_only,
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

impl WorkspaceState {
    fn new(
        workspace_id: WorkspaceId,
        workspace_root_id: WorkspaceRootId,
        principal_id: PrincipalId,
        root_path: PathBuf,
        trust: TrustState,
        snapshot_id: SnapshotId,
        tree: Vec<FileTreeNode>,
        scan: HashMap<String, FileFingerprint>,
    ) -> Self {
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
}

impl WorkspaceActor {
    /// Creates a new workspace actor.
    pub fn new(
        fs: Arc<ProjectFilesystem>,
        watcher: Arc<dyn WatcherService + Send + Sync>,
        security: DenyByDefaultBroker,
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
        }
    }

    fn now_sequence(state: &mut WorkspaceState) -> EventSequence {
        state.watcher_sequence = state.watcher_sequence.saturating_add(1);
        EventSequence(state.watcher_sequence)
    }

    fn check_path_within_root(&self, state: &WorkspaceState, path: &Path) -> WorkspaceResult<()> {
        let root = self
            .fs
            .canonicalize_path(&state.root_path)
            .or_else(|_| self.fs.normalize_path(&state.root_path))
            .map_err(WorkspaceError::Platform)?;
        let normalized = self
            .fs
            .canonicalize_path(path)
            .or_else(|_| self.fs.normalize_path(path))
            .map_err(WorkspaceError::Platform)?;

        if normalized.starts_with(&root) {
            Ok(())
        } else {
            Err(WorkspaceError::PathOutsideRoot {
                path: normalized.to_string_lossy().into_owned(),
            })
        }

        /*self.fs
        .is_within_base(&state.root_path, path)
        .map_err(WorkspaceError::Platform)
        .and_then(|inside| {
            if inside {
                Ok(())
            } else {
                Err(WorkspaceError::PathOutsideRoot {
                    path: path.to_string_lossy().into_owned(),
                })
            }
        })*/
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

    fn should_skip_entry(&self, entry_name: &str, metadata: Option<&std::fs::Metadata>) -> bool {
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
        if self.discovery.skip_binary {
            if let Some(ext) = Path::new(entry_name)
                .extension()
                .and_then(|value| value.to_str())
            {
                let suffix = format!(".{ext}").to_ascii_lowercase();
                if binaries.iter().any(|value| *value == suffix) {
                    return true;
                }
            }
        }

        if self.discovery.skip_large {
            if let Some(meta) = metadata {
                if meta.is_file() && meta.len() > LARGE_FILE_BYTES {
                    return true;
                }
            }
        }

        false
    }

    fn kind_for_metadata(&self, metadata: &std::fs::Metadata) -> FileKind {
        if metadata.is_dir() {
            FileKind::Directory
        } else if metadata.file_type().is_symlink() {
            FileKind::Symlink
        } else if metadata.is_file() {
            FileKind::File
        } else {
            FileKind::Other("other".to_string())
        }
    }

    fn file_identity(
        &self,
        state: &mut WorkspaceState,
        canonical_path: &Path,
        fingerprint: &FileFingerprint,
        metadata: &std::fs::Metadata,
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
                let digest = (size ^ ts.0) + (hash.len() as u64);
                FileContentVersion(digest)
            }
            (Some(size), Some(ts), None) => FileContentVersion(size.saturating_add(ts.0)),
            (Some(size), None, _) => FileContentVersion(size),
            _ => FileContentVersion(0),
        };

        let kind = self.kind_for_metadata(metadata);
        let file_metadata = FileMetadata {
            canonical_path: CanonicalPath(canonical_path.to_string_lossy().into_owned()),
            kind,
            size_bytes: fingerprint.size,
            modified_at: fingerprint.modified,
            read_only: fingerprint.read_only,
            permissions: None,
            hash: fingerprint.hash.clone(),
        };

        state.file_metadata.insert(file_id, file_metadata);

        FileIdentity {
            file_id,
            workspace_id: state.workspace_id,
            canonical_path: CanonicalPath(canonical_path.to_string_lossy().into_owned()),
            content_version,
            content_hash: fingerprint.hash.clone(),
        }
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

            let meta = match std::fs::metadata(&child) {
                Ok(meta) => Some(meta),
                Err(_) => None,
            };
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
                        FileFingerprint::from_path(&canonical, self.fs.as_ref())?
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
                self.file_identity(state, &canonical, &metadata, meta)
            } else {
                let key = canonical.to_string_lossy().into_owned();
                let file_id = state.next_file_id();
                state.file_id_by_path.insert(key.clone(), file_id);
                state.file_path_by_id.insert(file_id, key.clone());
                state.file_metadata.insert(
                    file_id,
                    FileMetadata {
                        canonical_path: CanonicalPath(key.clone()),
                        kind: FileKind::Other("unreadable".to_string()),
                        size_bytes: metadata.size,
                        modified_at: metadata.modified,
                        read_only: metadata.read_only,
                        permissions: Some("unreadable".to_string()),
                        hash: metadata.hash,
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
            if is_dir {
                if depth < MAX_TREE_CHILDREN_DEPTH {
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
            }

            let metadata = state
                .file_metadata
                .get(&identity.file_id)
                .cloned()
                .unwrap_or_else(|| FileMetadata {
                    canonical_path: identity.canonical_path.clone(),
                    kind: FileKind::Other("unknown".to_string()),
                    size_bytes: None,
                    modified_at: None,
                    read_only: false,
                    permissions: None,
                    hash: None,
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

    fn resolve_identity_internal(
        &self,
        state: &mut WorkspaceState,
        path: &str,
    ) -> WorkspaceResult<FileIdentity> {
        let canonical = self.canonicalize_candidate(state, path)?;
        self.decision_for_workspace(state, "fs.read", Some(&canonical.to_string_lossy()))?;

        let metadata = std::fs::metadata(&canonical).map_err(|err| {
            WorkspaceError::Platform(platform_error_from_io("metadata", &canonical, err))
        })?;
        let fingerprint = if metadata.is_file() {
            FileFingerprint::from_path(&canonical, self.fs.as_ref())?
        } else {
            FileFingerprint::from_dir()
        };

        let identity = self.file_identity(state, &canonical, &fingerprint, &metadata);
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
        let snapshot = self.watcher.snapshot(workspace_id, &root);
        let new_entries: Vec<PathBuf> = match snapshot {
            Ok(events) => events
                .into_iter()
                .map(|event| PathBuf::from(event.path.0))
                .collect(),
            Err(PlatformError::WatcherOverflow { .. }) => {
                state.in_recovery = true;
                let overflow = WatcherEvent {
                    workspace_id,
                    kind: WatcherEventKind::Overflow,
                    path: CanonicalPath(root.to_string_lossy().into_owned()),
                    old_path: None,
                    sequence: Self::now_sequence(state),
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

        if let Some(existing) = state_guard.as_ref() {
            if existing.workspace_id == workspace_id {
                return Ok(WorkspaceOpened {
                    workspace_id,
                    root_id: existing.workspace_root_id,
                    generation: existing.generation,
                    snapshot_id: existing.config_snapshot_id,
                    correlation_id: request.correlation_id,
                });
            }
        }

        let mut state = WorkspaceState::new(
            workspace_id,
            root_id,
            principal_id,
            root.clone(),
            trust,
            SnapshotId(stable_hash(
                &(root.to_string_lossy().into_owned() + "snapshot"),
            )),
            Vec::new(),
            HashMap::new(),
        );
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
        Ok(self
            .fs
            .read_text_file(&path)
            .map_err(WorkspaceError::Platform)?)
    }

    /// Write file text with trust checks and atomic write fallback.
    pub fn write_file_text(
        &self,
        workspace_id: WorkspaceId,
        path: impl AsRef<str>,
        text: impl AsRef<str>,
    ) -> WorkspaceResult<()> {
        let state_guard = self
            .state
            .lock()
            .map_err(|_| WorkspaceError::Internal("workspace state lock poisoned"))?;
        let state = state_guard
            .as_ref()
            .ok_or(WorkspaceError::WorkspaceMissing { workspace_id })?;
        let path = self.canonicalize_candidate(state, path.as_ref())?;
        self.decision_for_workspace(state, "fs.write", Some(&path.to_string_lossy()))?;

        self.fs
            .write_text_file_atomic(&path, text.as_ref())
            .or_else(|_| {
                self.fs
                    .write_text_file(&path, text.as_ref())
                    .map_err(WorkspaceError::Platform)
            })?;

        Ok(())
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
    use devil_platform::{NativeFileSystem, NativeWatcherService};
    use devil_protocol::{
        WorkspaceOpenRequest, WorkspaceOpened, WorkspacePort, WorkspaceRequest, WorkspaceTrustState,
    };
    use devil_security::{DenyByDefaultBroker, SecurityPolicy};

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
            "devil-project-test-{}-{}",
            std::process::id(),
            now_millis()
        );
        let root = base.join(unique);
        std::fs::create_dir_all(&root).expect("create temporary workspace directory");
        let canonical_root = std::fs::canonicalize(&root).expect("canonicalize temp workspace root");
        let canonical_root = canonical_root.to_string_lossy().into_owned();

        let mut policy = SecurityPolicy::default();
        policy.path_policy.readable_roots = vec![canonical_root.clone()];
        policy.path_policy.writable_roots = vec![canonical_root];

        let actor = WorkspaceActor::new(
            Arc::new(NativeFileSystem),
            Arc::new(NativeWatcherService),
            DenyByDefaultBroker::new(policy, devil_protocol::CapabilityNamespace("test".to_string())),
        );
        let req = WorkspaceOpenRequest {
            correlation_id: CorrelationId(3),
            principal_id: PrincipalId("temp-principal".to_string()),
            root_path: CanonicalPath(root.to_string_lossy().into_owned()),
            trust: Some(trust),
        };

        let opened = actor.open_workspace(req).expect("open temporary workspace");
        (actor, opened, PrincipalId("temp-principal".to_string()), root)
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
            .err()
            .expect("read config should fail when workspace id mismatch");
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
            .err()
            .expect("resolve should fail for outside root");
        assert!(matches!(read_err, WorkspaceError::PathOutsideRoot { .. }));

        let write_err = actor
            .write_file_text(opened.workspace_id, outside.to_string_lossy(), "ignored")
            .err()
            .expect("write should fail for outside root");
        assert!(matches!(write_err, WorkspaceError::PathOutsideRoot { .. }));

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn write_is_blocked_for_untrusted_workspace() {
        let (actor, opened, _, root) = temporary_workspace(WorkspaceTrustState::Untrusted);
        let file_path = "blocked.txt";

        let write_err = actor
            .write_file_text(opened.workspace_id, file_path.to_string(), "value")
            .err()
            .expect("untrusted workspace should not be able to write");

        assert!(matches!(
            write_err,
            WorkspaceError::SecurityDenied { .. }
        ));

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn read_write_roundtrip_from_workspace_apis() {
        let (actor, opened, _, root) = temporary_workspace(WorkspaceTrustState::Trusted);
        let file_path = "integration.txt";

        actor
            .write_file_text(opened.workspace_id, file_path, "hello\n")
            .expect("write via actor should succeed");

        let actual = actor
            .read_file_text(opened.workspace_id, file_path)
            .expect("read via actor should succeed");
        assert_eq!(actual, "hello\n");

        let _ = std::fs::remove_dir_all(&root);
    }

    impl WorkspaceActor {
        fn set_watchers_for_tests(&self) {
            let mut state = self.state.lock().expect("lock");
            if let Some(state) = state.as_mut() {
                state.watcher_sequence = state.watcher_sequence + 1;
            }
        }
    }
}
