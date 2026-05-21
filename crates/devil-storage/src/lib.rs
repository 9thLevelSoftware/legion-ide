//! Storage service interfaces for workspace metadata, trust decisions, file metadata cache, and sessions.

#![warn(missing_docs)]

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{
    Mutex,
    atomic::{AtomicBool, Ordering},
};

use devil_observability::{SharedEventSink, event_metadata_record};
use devil_protocol::{
    AssistedAiAuditRecord, CanonicalPath, CausalityId, CorrelationId,
    DelegatedTaskAuditLinkageRecord, EventEnvelope, EventId, EventMetadataRecord, EventSequence,
    EventSinkPort, EventSinkRequest, FileId, FileMetadata, PrincipalId, ProposalAuditRecord,
    ProposalId, ProtocolError, ProtocolResult, SemanticMetadataBatch, SemanticMetadataFreshnessKey,
    SemanticMetadataQuery, SemanticMetadataReadResult, SemanticMetadataRecord,
    SemanticMetadataTombstone, SemanticMetadataTombstoneReason, SnapshotId, StorageRepositoryPort,
    StorageRepositoryRequest, StorageRepositoryResponse, TrustRecord, WorkspaceConfigSnapshot,
    WorkspaceId, WorkspaceSessionRecord, WorkspaceTrustState, validate_assisted_ai_audit_record,
    validate_delegated_task_audit_linkage_record,
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

/// Metadata-only semantic persistence API.
pub trait SemanticMetadataRepository {
    /// Persist metadata-only semantic records and tombstones.
    fn save_semantic_metadata_batch(&mut self, batch: SemanticMetadataBatch) -> StorageResult<()>;
    /// Read freshness-gated metadata-only semantic records.
    fn read_semantic_metadata(
        &self,
        query: &SemanticMetadataQuery,
    ) -> StorageResult<SemanticMetadataReadResult>;
    /// Tombstone matching metadata-only semantic records.
    fn tombstone_semantic_metadata(
        &mut self,
        tombstone: SemanticMetadataTombstone,
    ) -> StorageResult<usize>;
    /// Read recorded semantic metadata tombstones.
    fn semantic_metadata_tombstones(
        &self,
        workspace_id: WorkspaceId,
        file_id: Option<FileId>,
    ) -> StorageResult<Vec<SemanticMetadataTombstone>>;
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
    protocol_assisted_ai_audit: HashMap<String, AssistedAiAuditRecord>,
    protocol_delegated_task_audit_linkage: HashMap<String, DelegatedTaskAuditLinkageRecord>,
    protocol_event_metadata: HashMap<EventId, EventMetadataRecord>,
    protocol_semantic_metadata: HashMap<String, SemanticMetadataRecord>,
    protocol_semantic_tombstones: Vec<SemanticMetadataTombstone>,
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
    semantic_metadata: HashMap<String, SemanticMetadataRecord>,
    semantic_tombstones: Vec<SemanticMetadataTombstone>,
}

impl From<&InMemoryStorage> for PersistedState {
    fn from(value: &InMemoryStorage) -> Self {
        Self {
            schema_version: 1,
            workspace_configs: value.workspace_configs.clone(),
            trust: value.trust.clone(),
            metadata: value.metadata.clone(),
            sessions: value.sessions.clone(),
            semantic_metadata: value.protocol_semantic_metadata.clone(),
            semantic_tombstones: value.protocol_semantic_tombstones.clone(),
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
            protocol_assisted_ai_audit: self.protocol_assisted_ai_audit.clone(),
            protocol_delegated_task_audit_linkage: self
                .protocol_delegated_task_audit_linkage
                .clone(),
            protocol_event_metadata: self.protocol_event_metadata.clone(),
            protocol_semantic_metadata: self.protocol_semantic_metadata.clone(),
            protocol_semantic_tombstones: self.protocol_semantic_tombstones.clone(),
        }
    }
}

#[derive(Debug, Default)]
/// Mutex-backed protocol repository adapter for [`InMemoryStorage`].
pub struct InMemoryStorageRepositoryPort {
    storage: Mutex<InMemoryStorage>,
    event_sink: SharedEventSink,
    fail_next_proposal_audit_write: AtomicBool,
    fail_next_event_metadata_write: AtomicBool,
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
            fail_next_proposal_audit_write: AtomicBool::new(false),
            fail_next_event_metadata_write: AtomicBool::new(false),
        }
    }

    /// Construct a protocol storage repository port with an injected audit event sink.
    pub fn with_event_sink(event_sink: SharedEventSink) -> Self {
        Self {
            storage: Mutex::new(InMemoryStorage::new()),
            event_sink,
            fail_next_proposal_audit_write: AtomicBool::new(false),
            fail_next_event_metadata_write: AtomicBool::new(false),
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
            fail_next_proposal_audit_write: AtomicBool::new(false),
            fail_next_event_metadata_write: AtomicBool::new(false),
        }
    }

    /// Cause the next proposal-audit write to fail for fail-closed integration tests.
    pub fn fail_next_proposal_audit_write(&self) {
        self.fail_next_proposal_audit_write
            .store(true, Ordering::SeqCst);
    }

    /// Cause the next event-metadata write to fail for fail-closed integration tests.
    pub fn fail_next_event_metadata_write(&self) {
        self.fail_next_event_metadata_write
            .store(true, Ordering::SeqCst);
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
        if matches!(
            request,
            StorageRepositoryRequest::SaveProposalAuditRecord(_)
        ) && self
            .fail_next_proposal_audit_write
            .swap(false, Ordering::SeqCst)
        {
            return Err(ProtocolError {
                code: "storage_failed".to_string(),
                message: "injected proposal audit write failure".to_string(),
            });
        }
        if matches!(request, StorageRepositoryRequest::SaveEventMetadata(_))
            && self
                .fail_next_event_metadata_write
                .swap(false, Ordering::SeqCst)
        {
            return Err(ProtocolError {
                code: "storage_failed".to_string(),
                message: "injected event metadata write failure".to_string(),
            });
        }
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
            protocol_assisted_ai_audit: HashMap::new(),
            protocol_delegated_task_audit_linkage: HashMap::new(),
            protocol_event_metadata: HashMap::new(),
            protocol_semantic_metadata: value.semantic_metadata,
            protocol_semantic_tombstones: value.semantic_tombstones,
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

impl SemanticMetadataRepository for FileBackedStorage {
    fn save_semantic_metadata_batch(&mut self, batch: SemanticMetadataBatch) -> StorageResult<()> {
        self.state.save_semantic_metadata_batch(batch)?;
        self.flush()
    }

    fn read_semantic_metadata(
        &self,
        query: &SemanticMetadataQuery,
    ) -> StorageResult<SemanticMetadataReadResult> {
        self.state.read_semantic_metadata(query)
    }

    fn tombstone_semantic_metadata(
        &mut self,
        tombstone: SemanticMetadataTombstone,
    ) -> StorageResult<usize> {
        let removed = self.state.tombstone_semantic_metadata(tombstone)?;
        self.flush()?;
        Ok(removed)
    }

    fn semantic_metadata_tombstones(
        &self,
        workspace_id: WorkspaceId,
        file_id: Option<FileId>,
    ) -> StorageResult<Vec<SemanticMetadataTombstone>> {
        self.state
            .semantic_metadata_tombstones(workspace_id, file_id)
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
                Self::validate_audit_record(&record)?;
                let key = record.proposal_id;
                self.protocol_proposal_audit.insert(key, record);
                Ok(Self::protocol_saved(format!("proposal_audit:{key:?}")))
            }
            StorageRepositoryRequest::SaveAssistedAiAuditRecord(record) => {
                Self::validate_assisted_ai_audit_record(&record)?;
                let key = record.audit_id.clone();
                self.protocol_assisted_ai_audit.insert(key.clone(), record);
                Ok(Self::protocol_saved(format!("assisted_ai_audit:{key}")))
            }
            StorageRepositoryRequest::SaveDelegatedTaskAuditLinkageRecord(record) => {
                Self::validate_delegated_task_audit_linkage_record(&record)?;
                let key = record.linkage_id.clone();
                self.protocol_delegated_task_audit_linkage
                    .insert(key.clone(), record);
                Ok(Self::protocol_saved(format!(
                    "delegated_task_audit_linkage:{key}"
                )))
            }
            StorageRepositoryRequest::SaveEventMetadata(record) => {
                Self::validate_event_metadata(&record)?;
                let key = record.event_id;
                self.protocol_event_metadata.insert(key, record);
                Ok(Self::protocol_saved(format!("event_metadata:{key:?}")))
            }
            StorageRepositoryRequest::SaveSemanticMetadata(batch) => {
                let count = batch.records.len();
                self.save_semantic_metadata_batch(batch)?;
                Ok(Self::protocol_saved(format!("semantic_metadata:{count}")))
            }
            StorageRepositoryRequest::TombstoneSemanticMetadata(tombstone) => {
                let removed = self.tombstone_semantic_metadata(tombstone)?;
                Ok(Self::protocol_saved(format!(
                    "semantic_metadata_tombstone:{removed}"
                )))
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
            StorageRepositoryRequest::ReadAssistedAiAuditRecord(audit_id) => {
                Ok(StorageRepositoryResponse::AssistedAiAuditRecord(Box::new(
                    self.protocol_assisted_ai_audit.get(&audit_id).cloned(),
                )))
            }
            StorageRepositoryRequest::ReadDelegatedTaskAuditLinkageRecord(linkage_id) => Ok(
                StorageRepositoryResponse::DelegatedTaskAuditLinkageRecord(Box::new(
                    self.protocol_delegated_task_audit_linkage
                        .get(&linkage_id)
                        .cloned(),
                )),
            ),
            StorageRepositoryRequest::ReadEventMetadata(event_id) => {
                Ok(StorageRepositoryResponse::EventMetadata(
                    self.protocol_event_metadata.get(&event_id).cloned(),
                ))
            }
            StorageRepositoryRequest::ReadSemanticMetadata(query) => Ok(
                StorageRepositoryResponse::SemanticMetadata(self.read_semantic_metadata(&query)?),
            ),
            StorageRepositoryRequest::ReadSemanticMetadataTombstones {
                workspace_id,
                file_id,
            } => Ok(StorageRepositoryResponse::SemanticMetadataTombstones(
                self.semantic_metadata_tombstones(workspace_id, file_id)?,
            )),
        }
    }

    fn validate_semantic_batch(batch: &SemanticMetadataBatch) -> StorageResult<()> {
        Self::validate_core_ids(batch.correlation_id, batch.causality_id, None)?;
        if batch.schema_version == 0 {
            return Err(StorageError::Failed {
                message: "semantic metadata batch schema version must be non-zero".to_string(),
            });
        }
        for record in &batch.records {
            Self::validate_semantic_record(record)?;
        }
        for tombstone in &batch.tombstones {
            Self::validate_semantic_tombstone(tombstone)?;
        }
        Ok(())
    }

    fn validate_semantic_record(record: &SemanticMetadataRecord) -> StorageResult<()> {
        if record.schema_version == 0
            || record.freshness_key.schema_version == 0
            || record.freshness_key.descriptor.schema_version == 0
        {
            return Err(StorageError::Failed {
                message: "semantic metadata schema versions must be non-zero".to_string(),
            });
        }
        if record.workspace_id != record.freshness_key.workspace_id
            || record.file_id != record.freshness_key.file_id
            || record.language_id != record.freshness_key.language_id
            || record.file_identity.workspace_id != record.workspace_id
            || record.file_identity.file_id != record.file_id
            || record.file_identity.privacy_scope != record.freshness_key.privacy_scope
        {
            return Err(StorageError::Failed {
                message: "semantic metadata record identity must match freshness key".to_string(),
            });
        }
        if record
            .freshness_key
            .descriptor
            .chunks
            .iter()
            .any(|chunk| chunk.chunk_hash.value.is_empty() || chunk.schema_version == 0)
        {
            return Err(StorageError::Failed {
                message: "semantic metadata chunk references require hashes and schema versions"
                    .to_string(),
            });
        }
        Ok(())
    }

    fn validate_semantic_tombstone(tombstone: &SemanticMetadataTombstone) -> StorageResult<()> {
        if tombstone.schema_version == 0 {
            return Err(StorageError::Failed {
                message: "semantic metadata tombstone schema version must be non-zero".to_string(),
            });
        }
        Ok(())
    }

    fn validate_audit_record(record: &ProposalAuditRecord) -> StorageResult<()> {
        Self::validate_core_ids(record.correlation_id, record.causality_id, None)?;
        if record.schema_version == 0 {
            return Err(StorageError::Failed {
                message: "proposal audit record schema version must be non-zero".to_string(),
            });
        }
        Ok(())
    }

    fn validate_event_metadata(record: &EventMetadataRecord) -> StorageResult<()> {
        Self::validate_core_ids(
            record.correlation_id,
            record.causality_id,
            Some(record.sequence),
        )?;
        if record.schema_version == 0 {
            return Err(StorageError::Failed {
                message: "event metadata schema version must be non-zero".to_string(),
            });
        }
        Ok(())
    }

    fn validate_assisted_ai_audit_record(record: &AssistedAiAuditRecord) -> StorageResult<()> {
        validate_assisted_ai_audit_record(record).map_err(|error| StorageError::Failed {
            message: error.to_string(),
        })
    }

    fn validate_delegated_task_audit_linkage_record(
        record: &DelegatedTaskAuditLinkageRecord,
    ) -> StorageResult<()> {
        validate_delegated_task_audit_linkage_record(record).map_err(|error| StorageError::Failed {
            message: error.to_string(),
        })
    }

    fn validate_core_ids(
        correlation_id: CorrelationId,
        causality_id: CausalityId,
        sequence: Option<EventSequence>,
    ) -> StorageResult<()> {
        if correlation_id.0 == 0 {
            return Err(StorageError::Failed {
                message: "audit metadata correlation id must be non-zero".to_string(),
            });
        }
        if causality_id.0.is_nil() {
            return Err(StorageError::Failed {
                message: "audit metadata causality id must be non-nil".to_string(),
            });
        }
        if sequence.is_some_and(|sequence| sequence.0 == 0) {
            return Err(StorageError::Failed {
                message: "event metadata sequence must be non-zero".to_string(),
            });
        }
        Ok(())
    }
}

fn semantic_metadata_storage_key(record: &SemanticMetadataRecord) -> String {
    let key = &record.freshness_key;
    format!(
        "{}:{}:{}:{}:{}:{}:{}:{}:{}:{}:{}:{:?}",
        key.workspace_id.0,
        key.file_id.0,
        key.language_id.0,
        key.snapshot_id.map_or(0, |value| value.0),
        key.file_content_version.0,
        key.workspace_generation.0,
        key.content_hash.algorithm,
        key.content_hash.value,
        key.grammar_version
            .as_ref()
            .map_or("".to_string(), |value| value.0.clone()),
        key.model_version
            .as_ref()
            .map_or("".to_string(), |value| value.0.clone()),
        key.parser_version,
        key.privacy_scope,
    )
}

fn semantic_metadata_matches_query(
    record: &SemanticMetadataRecord,
    query: &SemanticMetadataQuery,
) -> bool {
    record.workspace_id == query.workspace_id
        && (query.file_ids.is_empty() || query.file_ids.contains(&record.file_id))
        && (query.language_ids.is_empty() || query.language_ids.contains(&record.language_id))
        && record.freshness_key.privacy_scope == query.privacy_scope
}

fn semantic_metadata_rejection_reason(
    record: &SemanticMetadataRecord,
    expected: &SemanticMetadataFreshnessKey,
) -> SemanticMetadataTombstoneReason {
    if record.freshness_key.privacy_scope != expected.privacy_scope {
        SemanticMetadataTombstoneReason::PrivacyScopeRevoked
    } else if record.freshness_key.workspace_generation != expected.workspace_generation {
        SemanticMetadataTombstoneReason::WorkspaceGenerationChanged
    } else if record.freshness_key.schema_version != expected.schema_version {
        SemanticMetadataTombstoneReason::SchemaVersionChanged
    } else if record.freshness_key.parser_version != expected.parser_version {
        SemanticMetadataTombstoneReason::ParserVersionChanged
    } else if record.freshness_key.grammar_version != expected.grammar_version {
        SemanticMetadataTombstoneReason::GrammarVersionChanged
    } else if record.freshness_key.model_version != expected.model_version {
        SemanticMetadataTombstoneReason::ModelVersionChanged
    } else if record.freshness_key.language_id != expected.language_id {
        SemanticMetadataTombstoneReason::LanguageChanged
    } else if record.freshness_key.descriptor != expected.descriptor {
        SemanticMetadataTombstoneReason::DescriptorIdentityChanged
    } else {
        SemanticMetadataTombstoneReason::ContentHashMismatch
    }
}

fn tombstone_matches_record(
    tombstone: &SemanticMetadataTombstone,
    record: &SemanticMetadataRecord,
) -> bool {
    if tombstone.workspace_id != record.workspace_id {
        return false;
    }
    if tombstone
        .file_id
        .is_some_and(|file_id| file_id != record.file_id)
    {
        return false;
    }
    let Some(freshness_key) = tombstone.freshness_key.as_ref() else {
        return true;
    };
    match tombstone.reason {
        SemanticMetadataTombstoneReason::PrivacyScopeRevoked => {
            record.freshness_key.privacy_scope != freshness_key.privacy_scope
        }
        SemanticMetadataTombstoneReason::WorkspaceGenerationChanged => {
            record.freshness_key.workspace_generation != freshness_key.workspace_generation
        }
        _ => record.freshness_key != *freshness_key,
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

impl SemanticMetadataRepository for InMemoryStorage {
    fn save_semantic_metadata_batch(&mut self, batch: SemanticMetadataBatch) -> StorageResult<()> {
        Self::validate_semantic_batch(&batch)?;
        for tombstone in batch.tombstones {
            self.tombstone_semantic_metadata(tombstone)?;
        }
        for record in batch.records {
            let key = semantic_metadata_storage_key(&record);
            self.protocol_semantic_metadata.insert(key, record);
        }
        Ok(())
    }

    fn read_semantic_metadata(
        &self,
        query: &SemanticMetadataQuery,
    ) -> StorageResult<SemanticMetadataReadResult> {
        if query.schema_version == 0 {
            return Err(StorageError::Failed {
                message: "semantic metadata query schema version must be non-zero".to_string(),
            });
        }

        let mut records = Vec::new();
        let mut rejected = Vec::new();
        for record in self.protocol_semantic_metadata.values() {
            if !semantic_metadata_matches_query(record, query) {
                continue;
            }
            if let Some(expected) = query.freshness_key.as_ref()
                && record.freshness_key != *expected
            {
                let reason = semantic_metadata_rejection_reason(record, expected);
                rejected.push(SemanticMetadataTombstone {
                    workspace_id: record.workspace_id,
                    file_id: Some(record.file_id),
                    freshness_key: Some(expected.clone()),
                    reason,
                    observed_at: devil_protocol::TimestampMillis::now(),
                    schema_version: query.schema_version,
                });
                if !query.include_stale {
                    continue;
                }
            }
            records.push(record.clone());
        }
        records.sort_by(|left, right| left.record_id.0.cmp(&right.record_id.0));
        rejected.sort_by_key(|tombstone| {
            (
                tombstone.workspace_id.0,
                tombstone.file_id.map_or(0, |file_id| file_id.0),
                format!("{:?}", tombstone.reason),
            )
        });
        Ok(SemanticMetadataReadResult {
            records,
            rejected,
            schema_version: query.schema_version,
        })
    }

    fn tombstone_semantic_metadata(
        &mut self,
        tombstone: SemanticMetadataTombstone,
    ) -> StorageResult<usize> {
        Self::validate_semantic_tombstone(&tombstone)?;
        let before = self.protocol_semantic_metadata.len();
        self.protocol_semantic_metadata
            .retain(|_, record| !tombstone_matches_record(&tombstone, record));
        let removed = before.saturating_sub(self.protocol_semantic_metadata.len());
        self.protocol_semantic_tombstones.push(tombstone);
        Ok(removed)
    }

    fn semantic_metadata_tombstones(
        &self,
        workspace_id: WorkspaceId,
        file_id: Option<FileId>,
    ) -> StorageResult<Vec<SemanticMetadataTombstone>> {
        let mut tombstones = self
            .protocol_semantic_tombstones
            .iter()
            .filter(|tombstone| tombstone.workspace_id == workspace_id)
            .filter(|tombstone| file_id.is_none_or(|file_id| tombstone.file_id == Some(file_id)))
            .cloned()
            .collect::<Vec<_>>();
        tombstones.sort_by_key(|tombstone| {
            (
                tombstone.workspace_id.0,
                tombstone.file_id.map_or(0, |file_id| file_id.0),
                tombstone.observed_at.0,
            )
        });
        Ok(tombstones)
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
    use devil_protocol::{
        AssistedAiAuditOutcomeCategory, AssistedAiAuditPrivacyDisposition,
        AssistedAiAuditRedactionState, AssistedAiProviderInvocationState, ByteRange, CapabilityId,
        EventId, FileContentVersion, FileFingerprint, LanguageId, LineIndexRange,
        PermissionBudgetEvaluationDisposition, ProposalLifecycleState, ProposalPayloadKind,
        ProposalPayloadSummary, ProposalPrivacyLabel, ProposalRiskLabel,
        ProtocolDiagnosticSeverity, RedactionHint, RetentionLabel, SemanticFileFingerprintIdentity,
        SemanticFreshnessState, SemanticGrammarVersion, SemanticMetadataChunkReference,
        SemanticMetadataDescriptorIdentity, SemanticMetadataDiagnosticSummary,
        SemanticMetadataFreshnessKey, SemanticMetadataSourceKind, SemanticMetadataSymbolRecord,
        SemanticModelVersion, SemanticRecordId, SemanticRecordProvenance, SemanticRecordSource,
        SemanticSymbolId, SnapshotId, WorkspaceGeneration,
    };
    use serde_json::json;

    fn temp_storage_path(tag: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "devil-storage-{tag}-{}-{}.json",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |value| value.as_millis() as u64)
        ))
    }

    fn non_nil_causality_id() -> CausalityId {
        serde_json::from_value(json!("018f0000-0000-7000-8000-000000000001"))
            .expect("valid causality id")
    }

    fn nil_causality_id() -> CausalityId {
        serde_json::from_value(json!("00000000-0000-0000-0000-000000000000"))
            .expect("valid nil causality id")
    }

    fn event_id() -> EventId {
        serde_json::from_value(json!("018f0000-0000-7000-8000-000000000002"))
            .expect("valid event id")
    }

    fn audit_record() -> ProposalAuditRecord {
        ProposalAuditRecord {
            proposal_id: ProposalId(1),
            lifecycle_state: ProposalLifecycleState::Applied,
            timestamp: devil_protocol::TimestampMillis(1),
            principal: PrincipalId("tester".to_string()),
            capability: CapabilityId("fs.write".to_string()),
            correlation_id: CorrelationId(7),
            causality_id: non_nil_causality_id(),
            payload_summary: ProposalPayloadSummary {
                kind: ProposalPayloadKind::TextEdit,
                affected_files: vec![FileId(3)],
                title: Some("text-edit".to_string()),
                byte_count: Some(4),
            },
            diagnostics: Vec::new(),
            redaction_hints: vec![RedactionHint::MetadataOnly],
            schema_version: 1,
        }
    }

    fn event_metadata_record() -> EventMetadataRecord {
        EventMetadataRecord {
            event_id: event_id(),
            parent_event_id: None,
            causality_id: non_nil_causality_id(),
            correlation_id: CorrelationId(7),
            event: "proposal.audit_recorded".to_string(),
            workspace_id: Some(WorkspaceId(1)),
            sequence: EventSequence(1),
            principal_id: Some(PrincipalId("tester".to_string())),
            retention: RetentionLabel::Audit,
            redaction: RedactionHint::MetadataOnly,
            occurred_at: devil_protocol::TimestampMillis(1),
            schema_version: 1,
        }
    }

    fn assisted_ai_audit_record() -> AssistedAiAuditRecord {
        AssistedAiAuditRecord {
            audit_id: "assist:audit:req-1:1".to_string(),
            provider_capability_id: "provider:local-redacted".to_string(),
            provider_capability_hash: semantic_fingerprint("provider-hash"),
            route_decision_id: "assist:route:req-1".to_string(),
            route_decision_hash: semantic_fingerprint("route-hash"),
            consent_disposition: Some(devil_protocol::AssistedAiConsentState::Granted),
            budget_dispositions: vec![PermissionBudgetEvaluationDisposition::Allowed],
            privacy_disposition: AssistedAiAuditPrivacyDisposition::Allowed,
            request_contract_id: "assist:req:1".to_string(),
            request_contract_hash: semantic_fingerprint("request-hash"),
            projection_id: Some("assisted-ai:p6-3".to_string()),
            projection_hash: Some(semantic_fingerprint("projection-hash")),
            preview_id: Some("assist:preview:701".to_string()),
            preview_hash: Some(semantic_fingerprint("preview-hash")),
            proposal_id: Some(ProposalId(701)),
            outcome_category: AssistedAiAuditOutcomeCategory::ProposalPreviewReady,
            refusal_error_category: None,
            correlation_id: CorrelationId(901),
            causality_id: non_nil_causality_id(),
            event_sequence: EventSequence(77),
            risk_labels: vec![ProposalRiskLabel::Medium],
            privacy_labels: vec![ProposalPrivacyLabel::WorkspaceMetadata],
            redaction_state: AssistedAiAuditRedactionState::MetadataOnly,
            runtime_invocation_state: AssistedAiProviderInvocationState::NotEncoded,
            runtime_activation_labels: vec![
                "provider.invocation.not_encoded".to_string(),
                "network.not_encoded".to_string(),
                "tool.disabled".to_string(),
            ],
            redaction_hints: vec![RedactionHint::MetadataOnly],
            schema_version: 1,
        }
    }

    fn delegated_task_audit_linkage_record() -> devil_protocol::DelegatedTaskAuditLinkageRecord {
        devil_protocol::DelegatedTaskAuditLinkageRecord {
            linkage_id: "delegated-task:audit-linkage:plan-1:88".to_string(),
            plan_id: devil_protocol::DelegatedTaskPlanId("plan:p7-2:storage".to_string()),
            plan_hash: semantic_fingerprint("delegated-plan-hash"),
            step_ids: vec![devil_protocol::DelegatedTaskStepId(
                "step:preview".to_string(),
            )],
            proposal_preview_links: Vec::new(),
            trust_projection_references: Vec::new(),
            assisted_ai_audit_references: vec![
                devil_protocol::DelegatedTaskAssistedAiAuditReference {
                    audit_id: "assist:audit:req-1:77".to_string(),
                    audit_hash: semantic_fingerprint("assist-audit-hash"),
                    request_contract_id: "assist:req:1".to_string(),
                    request_contract_hash: semantic_fingerprint("assist-request-hash"),
                    projection_id: Some("assisted-ai:p6-3".to_string()),
                    projection_hash: Some(semantic_fingerprint("assisted-projection-hash")),
                    preview_id: Some("assist:preview:701".to_string()),
                    preview_hash: Some(semantic_fingerprint("assist-preview-hash")),
                    proposal_id: Some(ProposalId(701)),
                    outcome_category: AssistedAiAuditOutcomeCategory::ProposalPreviewReady,
                    event_sequence: EventSequence(77),
                    redaction_state: AssistedAiAuditRedactionState::MetadataOnly,
                    runtime_invocation_state: AssistedAiProviderInvocationState::NotEncoded,
                    schema_version: 1,
                },
            ],
            proposal_ids: vec![ProposalId(701)],
            blockers: Vec::new(),
            refusals: Vec::new(),
            readiness_classification:
                devil_protocol::DelegatedTaskReadinessClassification::WaitingForApproval,
            correlation_id: CorrelationId(901),
            causality_id: non_nil_causality_id(),
            event_sequence: EventSequence(88),
            risk_labels: vec![ProposalRiskLabel::Medium],
            privacy_labels: vec![ProposalPrivacyLabel::WorkspaceMetadata],
            runtime_activation: devil_protocol::DelegatedTaskRuntimeActivationState::NotEncoded,
            runtime_activation_labels: vec![
                "agent.runtime.not_encoded".to_string(),
                "provider.invocation.not_encoded".to_string(),
                "proposal.apply.not_encoded".to_string(),
            ],
            redaction_hints: vec![RedactionHint::MetadataOnly],
            schema_version: 1,
        }
    }

    fn semantic_fingerprint(value: &str) -> FileFingerprint {
        FileFingerprint {
            algorithm: "semantic-test-hash".to_string(),
            value: value.to_string(),
        }
    }

    fn semantic_freshness_key(
        privacy_scope: devil_protocol::SemanticPrivacyScope,
        workspace_generation: WorkspaceGeneration,
    ) -> SemanticMetadataFreshnessKey {
        SemanticMetadataFreshnessKey {
            workspace_id: WorkspaceId(77),
            file_id: FileId(88),
            language_id: LanguageId("rust".to_string()),
            snapshot_id: Some(SnapshotId(99)),
            file_content_version: FileContentVersion(3),
            workspace_generation,
            content_hash: semantic_fingerprint("content-hash"),
            grammar_version: Some(SemanticGrammarVersion("grammar-v1".to_string())),
            model_version: Some(SemanticModelVersion("model-v1".to_string())),
            parser_version: "parser-v1".to_string(),
            privacy_scope,
            descriptor: SemanticMetadataDescriptorIdentity {
                source_kind: SemanticMetadataSourceKind::DescriptorOnly,
                snapshot_id: Some(SnapshotId(99)),
                content_hash: semantic_fingerprint("content-hash"),
                byte_len: Some(4096),
                ranges: vec![ByteRange::new(0, 512)],
                chunks: vec![SemanticMetadataChunkReference {
                    snapshot_id: SnapshotId(99),
                    chunk_index: 0,
                    byte_range: ByteRange::new(0, 512),
                    line_range: LineIndexRange { start: 0, end: 32 },
                    byte_len: 512,
                    chunk_hash: semantic_fingerprint("chunk-hash"),
                    lease_present: false,
                    schema_version: 1,
                }],
                schema_version: 1,
            },
            schema_version: 1,
        }
    }

    fn semantic_record(
        privacy_scope: devil_protocol::SemanticPrivacyScope,
        workspace_generation: WorkspaceGeneration,
    ) -> SemanticMetadataRecord {
        let freshness_key = semantic_freshness_key(privacy_scope, workspace_generation);
        SemanticMetadataRecord {
            record_id: SemanticRecordId("semantic-record-1".to_string()),
            workspace_id: WorkspaceId(77),
            file_id: FileId(88),
            language_id: LanguageId("rust".to_string()),
            file_identity: SemanticFileFingerprintIdentity {
                workspace_id: WorkspaceId(77),
                file_id: FileId(88),
                canonical_path: CanonicalPath("C:/repo/src/lib.rs".to_string()),
                file_content_version: FileContentVersion(3),
                workspace_generation,
                content_hash: semantic_fingerprint("content-hash"),
                disk_fingerprint: Some(semantic_fingerprint("disk-hash")),
                byte_len: Some(4096),
                modified_at: None,
                privacy_scope,
                schema_version: 1,
            },
            freshness_key,
            provenance: SemanticRecordProvenance {
                source: SemanticRecordSource::Lexical,
                server_id: None,
                extraction_version: "parser-v1".to_string(),
                confidence_basis_points: 10_000,
            },
            symbols: vec![SemanticMetadataSymbolRecord {
                symbol_id: SemanticSymbolId("symbol-1".to_string()),
                symbol_name_hash: semantic_fingerprint("symbol-name-hash"),
                kind_hash: semantic_fingerprint("symbol-kind-hash"),
                declaration_range: None,
                reference_ranges: Vec::new(),
                schema_version: 1,
            }],
            graph_records: Vec::new(),
            diagnostic_summaries: vec![SemanticMetadataDiagnosticSummary {
                code_hash: semantic_fingerprint("diagnostic-code"),
                severity: ProtocolDiagnosticSeverity::Hint,
                range: None,
                count: 1,
            }],
            freshness_state: SemanticFreshnessState::Fresh,
            persisted_at: devil_protocol::TimestampMillis(1),
            schema_version: 1,
        }
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
    fn proposal_audit_storage_rejects_zero_correlation_and_nil_causality() {
        let storage = InMemoryStorageRepositoryPort::new();
        let mut zero_correlation = audit_record();
        zero_correlation.correlation_id = CorrelationId(0);
        assert!(
            storage
                .handle(StorageRepositoryRequest::SaveProposalAuditRecord(
                    zero_correlation
                ))
                .is_err()
        );

        let mut nil_causality = audit_record();
        nil_causality.causality_id = nil_causality_id();
        assert!(
            storage
                .handle(StorageRepositoryRequest::SaveProposalAuditRecord(
                    nil_causality
                ))
                .is_err()
        );
    }

    #[test]
    fn event_metadata_storage_rejects_zero_sequence_and_invalid_core_ids() {
        let storage = InMemoryStorageRepositoryPort::new();
        let mut zero_sequence = event_metadata_record();
        zero_sequence.sequence = EventSequence(0);
        assert!(
            storage
                .handle(StorageRepositoryRequest::SaveEventMetadata(zero_sequence))
                .is_err()
        );

        let mut zero_correlation = event_metadata_record();
        zero_correlation.correlation_id = CorrelationId(0);
        assert!(
            storage
                .handle(StorageRepositoryRequest::SaveEventMetadata(
                    zero_correlation
                ))
                .is_err()
        );

        let mut nil_causality = event_metadata_record();
        nil_causality.causality_id = nil_causality_id();
        assert!(
            storage
                .handle(StorageRepositoryRequest::SaveEventMetadata(nil_causality))
                .is_err()
        );
    }

    #[test]
    fn assisted_ai_audit_storage_roundtrips_metadata_only_and_rejects_invalid_records() {
        let storage = InMemoryStorageRepositoryPort::new();
        let record = assisted_ai_audit_record();
        storage
            .handle(StorageRepositoryRequest::SaveAssistedAiAuditRecord(
                record.clone(),
            ))
            .expect("save assisted AI audit record");
        let loaded = storage
            .handle(StorageRepositoryRequest::ReadAssistedAiAuditRecord(
                record.audit_id.clone(),
            ))
            .expect("read assisted AI audit record");
        match loaded {
            StorageRepositoryResponse::AssistedAiAuditRecord(loaded) => {
                let loaded = loaded.expect("assisted AI audit record should be present");
                assert_eq!(loaded.proposal_id, Some(ProposalId(701)));
                assert_eq!(
                    loaded.runtime_invocation_state,
                    AssistedAiProviderInvocationState::NotEncoded
                );
                let serialized = serde_json::to_string(&loaded).expect("serialize loaded audit");
                assert!(!serialized.contains("raw prompt"));
                assert!(!serialized.contains("source_body"));
                assert!(!serialized.contains("provider_payload"));
                assert!(!serialized.contains("terminal output"));
                assert!(!serialized.contains("network_request"));
                assert!(!serialized.contains("tool_call"));
                assert!(!serialized.contains("runtime_started"));
            }
            _ => panic!("expected assisted AI audit record"),
        }

        let mut zero_sequence = assisted_ai_audit_record();
        zero_sequence.event_sequence = EventSequence(0);
        assert!(
            storage
                .handle(StorageRepositoryRequest::SaveAssistedAiAuditRecord(
                    zero_sequence
                ))
                .is_err()
        );

        let mut raw_marker = assisted_ai_audit_record();
        raw_marker.refusal_error_category = Some("provider_payload raw prompt".to_string());
        assert!(
            storage
                .handle(StorageRepositoryRequest::SaveAssistedAiAuditRecord(
                    raw_marker
                ))
                .is_err()
        );
    }

    #[test]
    fn delegated_task_audit_linkage_storage_roundtrips_metadata_only_and_rejects_invalid_records() {
        let storage = InMemoryStorageRepositoryPort::new();
        let record = delegated_task_audit_linkage_record();
        storage
            .handle(StorageRepositoryRequest::SaveDelegatedTaskAuditLinkageRecord(record.clone()))
            .expect("save delegated task audit linkage");
        let loaded = storage
            .handle(
                StorageRepositoryRequest::ReadDelegatedTaskAuditLinkageRecord(
                    record.linkage_id.clone(),
                ),
            )
            .expect("read delegated task audit linkage");
        match loaded {
            StorageRepositoryResponse::DelegatedTaskAuditLinkageRecord(loaded) => {
                let loaded = loaded.expect("delegated task audit linkage should be present");
                assert_eq!(loaded.proposal_ids, vec![ProposalId(701)]);
                assert_eq!(
                    loaded.runtime_activation,
                    devil_protocol::DelegatedTaskRuntimeActivationState::NotEncoded
                );
                let serialized =
                    serde_json::to_string(&loaded).expect("serialize loaded delegated linkage");
                assert!(serialized.contains("WaitingForApproval"));
                assert!(!serialized.contains("raw prompt"));
                assert!(!serialized.contains("source_body"));
                assert!(!serialized.contains("provider_payload"));
                assert!(!serialized.contains("terminal output"));
                assert!(!serialized.contains("network_request"));
                assert!(!serialized.contains("tool_call"));
                assert!(!serialized.contains("agent_runtime"));
                assert!(!serialized.contains("runtime_started"));
            }
            _ => panic!("expected delegated task audit linkage record"),
        }

        let mut zero_sequence = delegated_task_audit_linkage_record();
        zero_sequence.event_sequence = EventSequence(0);
        assert!(
            storage
                .handle(
                    StorageRepositoryRequest::SaveDelegatedTaskAuditLinkageRecord(zero_sequence)
                )
                .is_err()
        );

        let mut raw_marker = delegated_task_audit_linkage_record();
        raw_marker
            .runtime_activation_labels
            .push("agent_runtime runtime_started".to_string());
        assert!(
            storage
                .handle(StorageRepositoryRequest::SaveDelegatedTaskAuditLinkageRecord(raw_marker))
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
    fn semantic_metadata_roundtrips_without_source_bodies() {
        let mut storage = InMemoryStorage::new();
        let source_body_marker = "fn should_not_persist_source_body() {}";
        let record = semantic_record(
            devil_protocol::SemanticPrivacyScope::Workspace,
            WorkspaceGeneration(5),
        );
        let query = SemanticMetadataQuery {
            workspace_id: WorkspaceId(77),
            file_ids: vec![FileId(88)],
            language_ids: vec![LanguageId("rust".to_string())],
            privacy_scope: devil_protocol::SemanticPrivacyScope::Workspace,
            freshness_key: Some(record.freshness_key.clone()),
            include_stale: false,
            schema_version: 1,
        };

        storage
            .save_semantic_metadata_batch(SemanticMetadataBatch {
                records: vec![record.clone()],
                tombstones: Vec::new(),
                correlation_id: CorrelationId(11),
                causality_id: non_nil_causality_id(),
                schema_version: 1,
            })
            .expect("save semantic metadata");

        let loaded = storage
            .read_semantic_metadata(&query)
            .expect("read semantic metadata");
        assert_eq!(loaded.records.len(), 1);
        assert_eq!(
            loaded.records[0].symbols[0].symbol_name_hash.value,
            "symbol-name-hash"
        );
        let serialized =
            serde_json::to_string(&loaded).expect("serialize loaded semantic metadata");
        assert!(!serialized.contains(source_body_marker));
        assert!(!serialized.contains("should_not_persist_source_body"));
    }

    #[test]
    fn semantic_metadata_privacy_revocation_tombstones_records() {
        let mut storage = InMemoryStorage::new();
        let record = semantic_record(
            devil_protocol::SemanticPrivacyScope::Workspace,
            WorkspaceGeneration(5),
        );
        storage
            .save_semantic_metadata_batch(SemanticMetadataBatch {
                records: vec![record],
                tombstones: Vec::new(),
                correlation_id: CorrelationId(12),
                causality_id: non_nil_causality_id(),
                schema_version: 1,
            })
            .expect("save semantic metadata");
        let tombstone = SemanticMetadataTombstone {
            workspace_id: WorkspaceId(77),
            file_id: Some(FileId(88)),
            freshness_key: Some(semantic_freshness_key(
                devil_protocol::SemanticPrivacyScope::MetadataOnly,
                WorkspaceGeneration(5),
            )),
            reason: SemanticMetadataTombstoneReason::PrivacyScopeRevoked,
            observed_at: devil_protocol::TimestampMillis(2),
            schema_version: 1,
        };

        let removed = storage
            .tombstone_semantic_metadata(tombstone)
            .expect("tombstone privacy-revoked semantic metadata");
        assert_eq!(removed, 1);
        let tombstones = storage
            .semantic_metadata_tombstones(WorkspaceId(77), Some(FileId(88)))
            .expect("read tombstones");
        assert!(matches!(
            tombstones[0].reason,
            SemanticMetadataTombstoneReason::PrivacyScopeRevoked
        ));
    }

    #[test]
    fn semantic_metadata_workspace_generation_mismatch_is_rejected() {
        let mut storage = InMemoryStorage::new();
        let record = semantic_record(
            devil_protocol::SemanticPrivacyScope::Workspace,
            WorkspaceGeneration(5),
        );
        storage
            .save_semantic_metadata_batch(SemanticMetadataBatch {
                records: vec![record],
                tombstones: Vec::new(),
                correlation_id: CorrelationId(13),
                causality_id: non_nil_causality_id(),
                schema_version: 1,
            })
            .expect("save semantic metadata");
        let query = SemanticMetadataQuery {
            workspace_id: WorkspaceId(77),
            file_ids: vec![FileId(88)],
            language_ids: vec![LanguageId("rust".to_string())],
            privacy_scope: devil_protocol::SemanticPrivacyScope::Workspace,
            freshness_key: Some(semantic_freshness_key(
                devil_protocol::SemanticPrivacyScope::Workspace,
                WorkspaceGeneration(6),
            )),
            include_stale: false,
            schema_version: 1,
        };

        let loaded = storage
            .read_semantic_metadata(&query)
            .expect("read generation-gated semantic metadata");
        assert!(loaded.records.is_empty());
        assert_eq!(loaded.rejected.len(), 1);
        assert!(matches!(
            loaded.rejected[0].reason,
            SemanticMetadataTombstoneReason::WorkspaceGenerationChanged
        ));
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
