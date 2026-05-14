//! Storage service interfaces for workspace metadata, trust decisions, file metadata cache, and sessions.

#![warn(missing_docs)]

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use devil_observability::{SharedEventSink, event_metadata_record};
use devil_protocol::{
    CanonicalPath, CorrelationId, EventEnvelope, EventId, EventMetadataRecord, EventSinkPort,
    EventSinkRequest, FileId, FileMetadata, PrincipalId, ProposalAuditRecord, ProposalId,
    ProtocolError, ProtocolResult, SnapshotId, StorageRepositoryPort, StorageRepositoryRequest,
    StorageRepositoryResponse, TrustRecord, WorkspaceConfigSnapshot, WorkspaceId,
    WorkspaceSessionRecord, WorkspaceTrustState,
};
use devil_security::TrustState;
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Clone, Serialize, Deserialize)]
/// Lightweight record for persisted workspace configuration snapshots.
pub struct WorkspaceConfigRecord {
    /// Serialized configuration payload.
    pub serialized: String,
    /// Current snapshot identifier for this configuration.
    pub snapshot_id: SnapshotId,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
/// Stored trust decision metadata for a workspace principal.
pub struct TrustDecisionRecord {
    /// Last known trust state.
    pub trust_state: WorkspaceTrustState,
    /// Correlation tracking this decision.
    pub correlation_id: CorrelationId,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
/// Cached file metadata used by shallow-discovery reconciliation.
pub struct FileMetadataRecord {
    /// Fingerprint hash or digest string.
    pub fingerprint: String,
    /// Stable workspace-local file identifier.
    pub file_id: FileId,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
/// Session metadata persisted for recovery and restore.
pub struct SessionRecord {
    /// Owning workspace identifier.
    pub workspace_id: WorkspaceId,
    /// Canonical workspace root path.
    pub workspace_path: CanonicalPath,
    /// Persisted trust state.
    pub trust_state: WorkspaceTrustState,
}

#[derive(Debug, Error)]
/// Storage error conditions.
pub enum StorageError {
    /// Requested record was not found.
    #[error("not found: {key}")]
    NotFound {
        /// Lookup key used for this lookup.
        key: String,
    },
    /// Low-level failure.
    #[error("storage operation failed: {message}")]
    Failed {
        /// Detailed failure text.
        message: String,
    },
    /// Persisted storage file was corrupt and got quarantined.
    #[error("storage corruption detected at `{path}`; quarantined to `{quarantine_path}`")]
    Corrupt {
        /// Original corrupt storage file path.
        path: String,
        /// Quarantine destination path.
        quarantine_path: String,
    },
}

type StorageResult<T> = Result<T, StorageError>;

/// Persistent workspace config persistence API.
pub trait WorkspaceConfigRepository {
    /// Store workspace configuration data.
    fn save(
        &mut self,
        workspace_id: WorkspaceId,
        config: WorkspaceConfigRecord,
    ) -> StorageResult<()>;
    /// Load workspace configuration data.
    fn load(&self, workspace_id: WorkspaceId) -> StorageResult<WorkspaceConfigRecord>;
    /// Remove workspace configuration data.
    fn remove(&mut self, workspace_id: WorkspaceId) -> StorageResult<()>;
}

/// Persistent trust decision API.
pub trait WorkspaceTrustRepository {
    /// Persist trust decision for principal in workspace.
    fn persist(
        &mut self,
        workspace_id: WorkspaceId,
        principal_id: &str,
        decision: TrustDecisionRecord,
    ) -> StorageResult<()>;
    /// Resolve trust decision for principal/workspace pair.
    fn resolve(
        &self,
        workspace_id: WorkspaceId,
        principal_id: &str,
    ) -> StorageResult<TrustDecisionRecord>;
}

/// File metadata cache API.
pub trait FileMetadataCache {
    /// Save fingerprint metadata for a path.
    fn put_fingerprint(
        &mut self,
        workspace_id: WorkspaceId,
        canonical_path: &str,
        metadata: FileMetadataRecord,
    ) -> StorageResult<()>;
    /// Load fingerprint metadata for a path.
    fn get_fingerprint(
        &self,
        workspace_id: WorkspaceId,
        canonical_path: &str,
    ) -> StorageResult<FileMetadataRecord>;
    /// Clear cache for workspace.
    fn clear_workspace(&mut self, workspace_id: WorkspaceId) -> StorageResult<()>;
}

/// Session persistence API.
pub trait WorkspaceSessionRepository {
    /// Persist session metadata.
    fn save_session(&mut self, session_id: &str, session: SessionRecord) -> StorageResult<()>;
    /// Restore session metadata.
    fn load_session(&self, session_id: &str) -> StorageResult<SessionRecord>;
    /// Delete session metadata.
    fn delete_session(&mut self, session_id: &str) -> StorageResult<()>;
}

#[derive(Debug, Default)]
/// Test-oriented, in-memory storage implementation.
pub struct InMemoryStorage {
    workspace_configs: HashMap<WorkspaceId, WorkspaceConfigRecord>,
    trust: HashMap<(WorkspaceId, String), TrustDecisionRecord>,
    metadata: HashMap<(WorkspaceId, String), FileMetadataRecord>,
    sessions: HashMap<String, SessionRecord>,
    protocol_workspace_configs: HashMap<WorkspaceId, WorkspaceConfigSnapshot>,
    protocol_file_metadata: HashMap<FileId, FileMetadata>,
    protocol_sessions: HashMap<String, WorkspaceSessionRecord>,
    protocol_trust: HashMap<(WorkspaceId, PrincipalId), TrustRecord>,
    protocol_proposal_audit: HashMap<ProposalId, ProposalAuditRecord>,
    protocol_event_metadata: HashMap<EventId, EventMetadataRecord>,
}

#[derive(Debug)]
/// JSON file-backed storage implementation with corruption quarantine behavior.
pub struct FileBackedStorage {
    path: PathBuf,
    state: InMemoryStorage,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PersistedState {
    schema_version: u16,
    workspace_configs: HashMap<WorkspaceId, WorkspaceConfigRecord>,
    trust: HashMap<(WorkspaceId, String), TrustDecisionRecord>,
    metadata: HashMap<(WorkspaceId, String), FileMetadataRecord>,
    sessions: HashMap<String, SessionRecord>,
}

impl From<&InMemoryStorage> for PersistedState {
    fn from(value: &InMemoryStorage) -> Self {
        Self {
            schema_version: 1,
            workspace_configs: value.workspace_configs.clone(),
            trust: value.trust.clone(),
            metadata: value.metadata.clone(),
            sessions: value.sessions.clone(),
        }
    }
}

impl Clone for InMemoryStorage {
    fn clone(&self) -> Self {
        Self {
            workspace_configs: self.workspace_configs.clone(),
            trust: self.trust.clone(),
            metadata: self.metadata.clone(),
            sessions: self.sessions.clone(),
            protocol_workspace_configs: self.protocol_workspace_configs.clone(),
            protocol_file_metadata: self.protocol_file_metadata.clone(),
            protocol_sessions: self.protocol_sessions.clone(),
            protocol_trust: self.protocol_trust.clone(),
            protocol_proposal_audit: self.protocol_proposal_audit.clone(),
            protocol_event_metadata: self.protocol_event_metadata.clone(),
        }
    }
}

#[derive(Debug, Default)]
/// Mutex-backed protocol repository adapter for [`InMemoryStorage`].
pub struct InMemoryStorageRepositoryPort {
    storage: Mutex<InMemoryStorage>,
    event_sink: SharedEventSink,
}

impl InMemoryStorageRepositoryPort {
    /// Construct a protocol storage repository port around a fresh in-memory store.
    pub fn new() -> Self {
        Self::default()
    }

    /// Construct a protocol storage repository port around an existing in-memory store.
    pub fn from_storage(storage: InMemoryStorage) -> Self {
        Self {
            storage: Mutex::new(storage),
            event_sink: SharedEventSink::default(),
        }
    }

    /// Construct a protocol storage repository port with an injected audit event sink.
    pub fn with_event_sink(event_sink: SharedEventSink) -> Self {
        Self {
            storage: Mutex::new(InMemoryStorage::new()),
            event_sink,
        }
    }

    /// Construct a protocol storage repository port around an existing store and event sink.
    pub fn from_storage_with_event_sink(
        storage: InMemoryStorage,
        event_sink: SharedEventSink,
    ) -> Self {
        Self {
            storage: Mutex::new(storage),
            event_sink,
        }
    }

    /// Persist redacted event metadata and emit through the injected sink.
    pub fn record_event(
        &self,
        envelope: EventEnvelope,
    ) -> ProtocolResult<StorageRepositoryResponse> {
        let metadata = event_metadata_record(&envelope);
        let emitted = self.event_sink.emit(EventSinkRequest { envelope });
        let stored = self.handle(StorageRepositoryRequest::SaveEventMetadata(metadata));
        emitted?;
        stored
    }

    /// Consume the adapter and return the wrapped in-memory store.
    pub fn into_inner(self) -> ProtocolResult<InMemoryStorage> {
        self.storage.into_inner().map_err(|_| ProtocolError {
            code: "storage_lock_poisoned".to_string(),
            message: "in-memory storage lock poisoned".to_string(),
        })
    }

    /// Execute a closure with read-only access to the wrapped in-memory store.
    pub fn with_storage<T>(&self, read: impl FnOnce(&InMemoryStorage) -> T) -> ProtocolResult<T> {
        let storage = self.storage.lock().map_err(Self::poisoned_error)?;
        Ok(read(&storage))
    }

    fn poisoned_error(
        _: std::sync::PoisonError<std::sync::MutexGuard<'_, InMemoryStorage>>,
    ) -> ProtocolError {
        ProtocolError {
            code: "storage_lock_poisoned".to_string(),
            message: "in-memory storage lock poisoned".to_string(),
        }
    }
}

impl StorageRepositoryPort for InMemoryStorageRepositoryPort {
    fn handle(
        &self,
        request: StorageRepositoryRequest,
    ) -> ProtocolResult<StorageRepositoryResponse> {
        let mut storage = self.storage.lock().map_err(Self::poisoned_error)?;
        storage
            .handle_protocol_request(request)
            .map_err(InMemoryStorage::protocol_error)
    }
}

impl From<PersistedState> for InMemoryStorage {
    fn from(value: PersistedState) -> Self {
        Self {
            workspace_configs: value.workspace_configs,
            trust: value.trust,
            metadata: value.metadata,
            sessions: value.sessions,
            protocol_workspace_configs: HashMap::new(),
            protocol_file_metadata: HashMap::new(),
            protocol_sessions: HashMap::new(),
            protocol_trust: HashMap::new(),
            protocol_proposal_audit: HashMap::new(),
            protocol_event_metadata: HashMap::new(),
        }
    }
}

impl FileBackedStorage {
    /// Open file-backed storage from path, creating defaults when missing.
    pub fn open(path: impl AsRef<Path>) -> StorageResult<Self> {
        let path = path.as_ref().to_path_buf();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|err| StorageError::Failed {
                message: format!("create storage directory failed: {err}"),
            })?;
        }

        let state = match fs::read_to_string(&path) {
            Ok(contents) => {
                let persisted: PersistedState = serde_json::from_str(&contents).map_err(|_| {
                    let quarantine = Self::quarantine_path(&path);
                    let _ = fs::rename(&path, &quarantine);
                    StorageError::Corrupt {
                        path: path.to_string_lossy().into_owned(),
                        quarantine_path: quarantine.to_string_lossy().into_owned(),
                    }
                })?;
                InMemoryStorage::from(persisted)
            }
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => InMemoryStorage::new(),
            Err(err) => {
                return Err(StorageError::Failed {
                    message: format!("read storage file failed: {err}"),
                });
            }
        };

        let mut storage = Self { path, state };
        storage.flush()?;
        Ok(storage)
    }

    fn quarantine_path(path: &Path) -> PathBuf {
        let mut extension = path
            .extension()
            .map(|value| value.to_string_lossy().to_string())
            .unwrap_or_else(|| "json".to_string());
        extension.push_str(".corrupt");
        path.with_extension(extension)
    }

    fn flush(&mut self) -> StorageResult<()> {
        let persisted = PersistedState::from(&self.state);
        let body =
            serde_json::to_string_pretty(&persisted).map_err(|err| StorageError::Failed {
                message: format!("serialize storage state failed: {err}"),
            })?;

        fs::write(&self.path, body).map_err(|err| StorageError::Failed {
            message: format!("write storage file failed: {err}"),
        })
    }
}

impl WorkspaceConfigRepository for FileBackedStorage {
    fn save(
        &mut self,
        workspace_id: WorkspaceId,
        config: WorkspaceConfigRecord,
    ) -> StorageResult<()> {
        self.state.save(workspace_id, config)?;
        self.flush()
    }

    fn load(&self, workspace_id: WorkspaceId) -> StorageResult<WorkspaceConfigRecord> {
        self.state.load(workspace_id)
    }

    fn remove(&mut self, workspace_id: WorkspaceId) -> StorageResult<()> {
        self.state.remove(workspace_id)?;
        self.flush()
    }
}

impl WorkspaceTrustRepository for FileBackedStorage {
    fn persist(
        &mut self,
        workspace_id: WorkspaceId,
        principal_id: &str,
        decision: TrustDecisionRecord,
    ) -> StorageResult<()> {
        self.state.persist(workspace_id, principal_id, decision)?;
        self.flush()
    }

    fn resolve(
        &self,
        workspace_id: WorkspaceId,
        principal_id: &str,
    ) -> StorageResult<TrustDecisionRecord> {
        self.state.resolve(workspace_id, principal_id)
    }
}

impl FileMetadataCache for FileBackedStorage {
    fn put_fingerprint(
        &mut self,
        workspace_id: WorkspaceId,
        canonical_path: &str,
        metadata: FileMetadataRecord,
    ) -> StorageResult<()> {
        self.state
            .put_fingerprint(workspace_id, canonical_path, metadata)?;
        self.flush()
    }

    fn get_fingerprint(
        &self,
        workspace_id: WorkspaceId,
        canonical_path: &str,
    ) -> StorageResult<FileMetadataRecord> {
        self.state.get_fingerprint(workspace_id, canonical_path)
    }

    fn clear_workspace(&mut self, workspace_id: WorkspaceId) -> StorageResult<()> {
        self.state.clear_workspace(workspace_id)?;
        self.flush()
    }
}

impl WorkspaceSessionRepository for FileBackedStorage {
    fn save_session(&mut self, session_id: &str, session: SessionRecord) -> StorageResult<()> {
        self.state.save_session(session_id, session)?;
        self.flush()
    }

    fn load_session(&self, session_id: &str) -> StorageResult<SessionRecord> {
        self.state.load_session(session_id)
    }

    fn delete_session(&mut self, session_id: &str) -> StorageResult<()> {
        self.state.delete_session(session_id)?;
        self.flush()
    }
}

impl InMemoryStorage {
    /// Construct a new in-memory store.
    pub fn new() -> Self {
        Self::default()
    }

    fn protocol_saved(key: impl Into<String>) -> StorageRepositoryResponse {
        StorageRepositoryResponse::Saved { key: key.into() }
    }

    fn protocol_error(error: StorageError) -> ProtocolError {
        match error {
            StorageError::NotFound { key } => ProtocolError {
                code: "storage_not_found".to_string(),
                message: key,
            },
            StorageError::Failed { message } => ProtocolError {
                code: "storage_failed".to_string(),
                message,
            },
            StorageError::Corrupt {
                path,
                quarantine_path,
            } => ProtocolError {
                code: "storage_corrupt".to_string(),
                message: format!("{path} quarantined to {quarantine_path}"),
            },
        }
    }

    fn handle_protocol_request(
        &mut self,
        request: StorageRepositoryRequest,
    ) -> StorageResult<StorageRepositoryResponse> {
        match request {
            StorageRepositoryRequest::SaveWorkspaceConfig(config) => {
                let key = format!("workspace_config:{:?}", config.workspace_id);
                self.protocol_workspace_configs
                    .insert(config.workspace_id, config);
                Ok(Self::protocol_saved(key))
            }
            StorageRepositoryRequest::SaveFileMetadata(metadata) => {
                let file_id = self
                    .protocol_file_metadata
                    .iter()
                    .find(|(_, existing)| existing.canonical_path == metadata.canonical_path)
                    .map(|(id, _)| *id);
                let file_id = file_id.unwrap_or(FileId(devil_protocol_stable_hash(
                    &metadata.canonical_path.0,
                )));
                self.protocol_file_metadata.insert(file_id, metadata);
                Ok(Self::protocol_saved(format!("file_metadata:{file_id:?}")))
            }
            StorageRepositoryRequest::SaveSessionRecord(record) => {
                let key = record.session_id.clone();
                self.protocol_sessions.insert(key.clone(), record);
                Ok(Self::protocol_saved(format!("session:{key}")))
            }
            StorageRepositoryRequest::SaveTrustRecord(record) => {
                let key = (record.workspace_id, record.principal_id.clone());
                self.protocol_trust.insert(key.clone(), record);
                Ok(Self::protocol_saved(format!(
                    "trust:{:?}:{}",
                    key.0,
                    (key.1).0
                )))
            }
            StorageRepositoryRequest::SaveProposalAuditRecord(record) => {
                let key = record.proposal_id;
                self.protocol_proposal_audit.insert(key, record);
                Ok(Self::protocol_saved(format!("proposal_audit:{key:?}")))
            }
            StorageRepositoryRequest::SaveEventMetadata(record) => {
                let key = record.event_id;
                self.protocol_event_metadata.insert(key, record);
                Ok(Self::protocol_saved(format!("event_metadata:{key:?}")))
            }
            StorageRepositoryRequest::ReadWorkspaceConfig(workspace_id) => {
                Ok(StorageRepositoryResponse::WorkspaceConfig(
                    self.protocol_workspace_configs.get(&workspace_id).cloned(),
                ))
            }
            StorageRepositoryRequest::ReadFileMetadata(file_id) => {
                Ok(StorageRepositoryResponse::FileMetadata(
                    self.protocol_file_metadata.get(&file_id).cloned(),
                ))
            }
            StorageRepositoryRequest::ReadSessionRecord { session_id } => {
                Ok(StorageRepositoryResponse::SessionRecord(
                    self.protocol_sessions.get(&session_id).cloned(),
                ))
            }
            StorageRepositoryRequest::ReadTrustRecord {
                workspace_id,
                principal_id,
            } => Ok(StorageRepositoryResponse::TrustRecord(
                self.protocol_trust
                    .get(&(workspace_id, principal_id))
                    .cloned(),
            )),
            StorageRepositoryRequest::ReadProposalAuditRecord(proposal_id) => {
                Ok(StorageRepositoryResponse::ProposalAuditRecord(
                    self.protocol_proposal_audit.get(&proposal_id).cloned(),
                ))
            }
            StorageRepositoryRequest::ReadEventMetadata(event_id) => {
                Ok(StorageRepositoryResponse::EventMetadata(
                    self.protocol_event_metadata.get(&event_id).cloned(),
                ))
            }
        }
    }
}

fn devil_protocol_stable_hash(value: &str) -> u128 {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let mut hasher = DefaultHasher::new();
    value.hash(&mut hasher);
    hasher.finish() as u128
}

impl WorkspaceConfigRepository for InMemoryStorage {
    fn save(
        &mut self,
        workspace_id: WorkspaceId,
        config: WorkspaceConfigRecord,
    ) -> StorageResult<()> {
        self.workspace_configs.insert(workspace_id, config);
        Ok(())
    }

    fn load(&self, workspace_id: WorkspaceId) -> StorageResult<WorkspaceConfigRecord> {
        self.workspace_configs
            .get(&workspace_id)
            .cloned()
            .ok_or_else(|| StorageError::NotFound {
                key: format!("workspace_config:{workspace_id:?}"),
            })
    }

    fn remove(&mut self, workspace_id: WorkspaceId) -> StorageResult<()> {
        self.workspace_configs
            .remove(&workspace_id)
            .map(|_| ())
            .ok_or_else(|| StorageError::NotFound {
                key: format!("workspace_config:{workspace_id:?}"),
            })
    }
}

impl WorkspaceTrustRepository for InMemoryStorage {
    fn persist(
        &mut self,
        workspace_id: WorkspaceId,
        principal_id: &str,
        decision: TrustDecisionRecord,
    ) -> StorageResult<()> {
        self.trust
            .insert((workspace_id, principal_id.to_string()), decision);
        Ok(())
    }

    fn resolve(
        &self,
        workspace_id: WorkspaceId,
        principal_id: &str,
    ) -> StorageResult<TrustDecisionRecord> {
        self.trust
            .get(&(workspace_id, principal_id.to_string()))
            .cloned()
            .ok_or_else(|| StorageError::NotFound {
                key: format!("workspace_trust:{workspace_id:?}:{principal_id}"),
            })
    }
}

impl FileMetadataCache for InMemoryStorage {
    fn put_fingerprint(
        &mut self,
        workspace_id: WorkspaceId,
        canonical_path: &str,
        metadata: FileMetadataRecord,
    ) -> StorageResult<()> {
        self.metadata
            .insert((workspace_id, canonical_path.to_string()), metadata);
        Ok(())
    }

    fn get_fingerprint(
        &self,
        workspace_id: WorkspaceId,
        canonical_path: &str,
    ) -> StorageResult<FileMetadataRecord> {
        self.metadata
            .get(&(workspace_id, canonical_path.to_string()))
            .cloned()
            .ok_or_else(|| StorageError::NotFound {
                key: format!("file_metadata:{workspace_id:?}:{canonical_path}"),
            })
    }

    fn clear_workspace(&mut self, workspace_id: WorkspaceId) -> StorageResult<()> {
        let before = self.metadata.len();
        self.metadata.retain(|(id, _), _| *id != workspace_id);

        if self.metadata.len() == before {
            return Err(StorageError::NotFound {
                key: format!("file_metadata:{workspace_id:?}"),
            });
        }

        Ok(())
    }
}

impl WorkspaceSessionRepository for InMemoryStorage {
    fn save_session(&mut self, session_id: &str, session: SessionRecord) -> StorageResult<()> {
        self.sessions.insert(session_id.to_string(), session);
        Ok(())
    }

    fn load_session(&self, session_id: &str) -> StorageResult<SessionRecord> {
        self.sessions
            .get(session_id)
            .cloned()
            .ok_or_else(|| StorageError::NotFound {
                key: format!("session:{session_id}"),
            })
    }

    fn delete_session(&mut self, session_id: &str) -> StorageResult<()> {
        self.sessions
            .remove(session_id)
            .map(|_| ())
            .ok_or_else(|| StorageError::NotFound {
                key: format!("session:{session_id}"),
            })
    }
}

/// Convert security trust model to protocol trust model.
pub fn security_trust_to_protocol(state: TrustState) -> WorkspaceTrustState {
    match state {
        TrustState::Trusted => WorkspaceTrustState::Trusted,
        TrustState::Untrusted => WorkspaceTrustState::Untrusted,
        TrustState::Unknown => WorkspaceTrustState::Unknown,
    }
}

/// Convert protocol trust model to security trust model.
pub fn protocol_trust_to_security(state: WorkspaceTrustState) -> TrustState {
    match state {
        WorkspaceTrustState::Trusted => TrustState::Trusted,
        WorkspaceTrustState::Untrusted => TrustState::Untrusted,
        WorkspaceTrustState::Unknown => TrustState::Unknown,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_storage_path(tag: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "devil-storage-{tag}-{}-{}.json",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |value| value.as_millis() as u64)
        ))
    }

    #[test]
    fn in_memory_storage_roundtrip_config() {
        let mut storage = InMemoryStorage::new();
        let id = WorkspaceId(10);
        let record = WorkspaceConfigRecord {
            serialized: r#"{"name":"demo"}"#.to_string(),
            snapshot_id: SnapshotId(99),
        };

        storage
            .save(id, record.clone())
            .expect("save workspace config");
        let loaded = storage.load(id).expect("load workspace config");
        assert_eq!(loaded.serialized, record.serialized);
        assert_eq!(loaded.snapshot_id, record.snapshot_id);
    }

    #[test]
    fn in_memory_storage_roundtrip_trust() {
        let mut storage = InMemoryStorage::new();
        let record = TrustDecisionRecord {
            trust_state: WorkspaceTrustState::Trusted,
            correlation_id: CorrelationId(3),
        };

        storage
            .persist(WorkspaceId(20), "principal", record.clone())
            .expect("persist trust decision");
        let loaded = storage
            .resolve(WorkspaceId(20), "principal")
            .expect("load trust decision");
        assert_eq!(
            loaded.trust_state as u8, record.trust_state as u8,
            "stored and loaded trust state must match"
        );
    }

    #[test]
    fn in_memory_storage_roundtrip_file_metadata() {
        let mut storage = InMemoryStorage::new();
        let rec = FileMetadataRecord {
            fingerprint: "abc123".to_string(),
            file_id: FileId(5),
        };

        storage
            .put_fingerprint(WorkspaceId(30), "/tmp/a.txt", rec.clone())
            .expect("store file metadata");
        let loaded = storage
            .get_fingerprint(WorkspaceId(30), "/tmp/a.txt")
            .expect("load file metadata");
        assert_eq!(loaded.fingerprint, rec.fingerprint);

        storage
            .clear_workspace(WorkspaceId(30))
            .expect("clear workspace");
        assert!(
            storage
                .get_fingerprint(WorkspaceId(30), "/tmp/a.txt")
                .is_err()
        );
    }

    #[test]
    fn in_memory_storage_roundtrip_session() {
        let mut storage = InMemoryStorage::new();
        let rec = SessionRecord {
            workspace_id: WorkspaceId(40),
            workspace_path: CanonicalPath("/tmp/ws".to_string()),
            trust_state: WorkspaceTrustState::Trusted,
        };

        storage
            .save_session("session-1", rec.clone())
            .expect("save session");
        let loaded = storage.load_session("session-1").expect("load session");
        assert_eq!(loaded.workspace_id, rec.workspace_id);

        storage.delete_session("session-1").expect("delete session");
        assert!(storage.load_session("session-1").is_err());
    }

    #[test]
    fn trust_conversion_roundtrips() {
        let security_from_protocol = protocol_trust_to_security(WorkspaceTrustState::Trusted);
        let protocol_from_security = security_trust_to_protocol(TrustState::Untrusted);
        let protocol_from_security_roundtrip = protocol_from_security.clone();

        assert!(matches!(security_from_protocol, TrustState::Trusted));
        assert!(matches!(
            protocol_trust_to_security(protocol_from_security_roundtrip),
            TrustState::Untrusted
        ));
        assert!(matches!(
            protocol_from_security,
            WorkspaceTrustState::Untrusted
        ));
    }

    #[test]
    fn file_backed_storage_roundtrip_config_and_session() {
        let path = temp_storage_path("roundtrip");
        let mut storage = FileBackedStorage::open(&path).expect("open file-backed storage");

        storage
            .save(
                WorkspaceId(88),
                WorkspaceConfigRecord {
                    serialized: "{\"theme\":\"dark\"}".to_string(),
                    snapshot_id: SnapshotId(123),
                },
            )
            .expect("save config");
        storage
            .save_session(
                "session-a",
                SessionRecord {
                    workspace_id: WorkspaceId(88),
                    workspace_path: CanonicalPath("C:/repo".to_string()),
                    trust_state: WorkspaceTrustState::Trusted,
                },
            )
            .expect("save session");

        let storage_reloaded = FileBackedStorage::open(&path).expect("reopen storage");
        let loaded_config = storage_reloaded
            .load(WorkspaceId(88))
            .expect("load saved config");
        let loaded_session = storage_reloaded
            .load_session("session-a")
            .expect("load saved session");

        assert_eq!(loaded_config.snapshot_id, SnapshotId(123));
        assert_eq!(loaded_session.workspace_id, WorkspaceId(88));

        let _ = fs::remove_file(path);
    }

    #[test]
    fn file_backed_storage_corrupt_file_is_quarantined() {
        let path = temp_storage_path("corrupt");
        fs::write(&path, "{ invalid json").expect("write corrupt content");

        let err = FileBackedStorage::open(&path).expect_err("opening corrupt file should fail");
        match err {
            StorageError::Corrupt {
                path: original,
                quarantine_path,
            } => {
                assert!(original.ends_with(".json"));
                assert!(quarantine_path.ends_with(".json.corrupt"));
                assert!(Path::new(&quarantine_path).exists());
            }
            other => panic!("unexpected error: {other:?}"),
        }

        let quarantine = FileBackedStorage::quarantine_path(&path);
        let _ = fs::remove_file(quarantine);
    }
}
