//! Storage service interfaces for workspace metadata, trust decisions, file metadata cache, and sessions.

#![warn(missing_docs)]

use std::collections::HashMap;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{
    Mutex,
    atomic::{AtomicBool, Ordering},
};

use devil_observability::{SharedEventSink, event_metadata_record};
use devil_protocol::{
    AgentReplayManifest, AgentRunId, AssistedAiAuditRecord, CanonicalPath, CausalityId,
    CollaborationAuditRecord, CollaborationSessionId, CorrelationId, DebugAdapterAuditRecord,
    DebugBreakpointRecord, DebugSessionId, DelegatedTaskAuditLinkageRecord, EventEnvelope, EventId,
    EventMetadataRecord, EventSequence, EventSinkPort, EventSinkRequest, FileId, FileMetadata,
    HostedTelemetrySpoolRecord, Phase4RuntimeAuditRecord, PluginDenialReason,
    PluginStorageOperation, PluginStorageRecord, PrincipalId, ProposalAuditRecord, ProposalId,
    ProtocolError, ProtocolResult, RawSourceRetentionAccessAudit, RemoteAuditRecord,
    RemoteTransportAuditSummary, RemoteWorkspaceSessionId, SemanticMetadataBatch,
    SemanticMetadataFreshnessKey, SemanticMetadataQuery, SemanticMetadataReadResult,
    SemanticMetadataRecord, SemanticMetadataTombstone, SemanticMetadataTombstoneReason, SnapshotId,
    StorageBackupMarker, StorageChecksum, StorageMigrationDryRunReport, StorageMigrationStep,
    StorageRecoveryOutcome, StorageRepairRequest, StorageRepositoryPort, StorageRepositoryRequest,
    StorageRepositoryResponse, StorageSchemaManifest, TerminalAuditRecord, TerminalSessionId,
    TrustRecord, WorkspaceConfigSnapshot, WorkspaceId, WorkspaceSessionRecord, WorkspaceTrustState,
    validate_agent_replay_manifest, validate_assisted_ai_audit_record,
    validate_collaboration_audit_record, validate_debug_adapter_audit_record,
    validate_debug_breakpoint_identity, validate_debug_breakpoint_record,
    validate_delegated_task_audit_linkage_record, validate_hosted_telemetry_spool_record,
    validate_phase4_runtime_audit_record, validate_plugin_storage_record,
    validate_raw_source_retention_access_audit, validate_remote_audit_record,
    validate_remote_transport_audit_summary, validate_storage_backup_marker,
    validate_storage_migration_dry_run_report, validate_storage_recovery_outcome,
    validate_storage_repair_request, validate_storage_schema_manifest,
    validate_terminal_audit_record,
};
use devil_security::TrustState;
use serde::{Deserialize, Serialize};
use thiserror::Error;

const STORAGE_CHECKSUM_ALGORITHM: &str = "devil-storage-stable-sum-v1";

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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
/// Persisted dock layout state for one dock side in one product mode.
pub struct DockLayoutStorageRecord {
    /// Owning workspace identifier.
    pub workspace_id: WorkspaceId,
    /// Stable product mode label, for example `Manual`, `Assist`, `Delegate`, or `Automate`.
    pub mode: String,
    /// Stable dock side label, for example `Left`, `Right`, or `Bottom`.
    pub side: String,
    /// Stable panel id pinned as the side default.
    pub pinned_default_panel_id: String,
    /// Stable panel ids in the custom toolkit region.
    pub custom_toolkit_panel_ids: Vec<String>,
    /// Persisted splitter fraction for the side.
    pub splitter_fraction: f32,
    /// Whether this dock side is collapsed.
    pub collapsed: bool,
    /// Storage record schema version.
    pub schema_version: u16,
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

impl StorageError {
    fn from_protocol(error: ProtocolError) -> Self {
        Self::Failed {
            message: error.message,
        }
    }
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

/// Mode-scoped dock layout persistence API.
pub trait DockLayoutRepository {
    /// Persist one dock side layout record.
    fn save_dock_side_layout(&mut self, record: DockLayoutStorageRecord) -> StorageResult<()>;
    /// Load one dock side layout record.
    fn load_dock_side_layout(
        &self,
        workspace_id: WorkspaceId,
        mode: &str,
        side: &str,
    ) -> StorageResult<DockLayoutStorageRecord>;
    /// Load all dock side layout records for a workspace.
    fn load_dock_layouts(
        &self,
        workspace_id: WorkspaceId,
    ) -> StorageResult<Vec<DockLayoutStorageRecord>>;
    /// Delete one dock side layout record.
    fn delete_dock_side_layout(
        &mut self,
        workspace_id: WorkspaceId,
        mode: &str,
        side: &str,
    ) -> StorageResult<()>;
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
    dock_layouts: HashMap<String, DockLayoutStorageRecord>,
    protocol_workspace_configs: HashMap<WorkspaceId, WorkspaceConfigSnapshot>,
    protocol_file_metadata: HashMap<FileId, FileMetadata>,
    protocol_sessions: HashMap<String, WorkspaceSessionRecord>,
    protocol_trust: HashMap<(WorkspaceId, PrincipalId), TrustRecord>,
    protocol_proposal_audit: HashMap<ProposalId, ProposalAuditRecord>,
    protocol_assisted_ai_audit: HashMap<String, AssistedAiAuditRecord>,
    protocol_delegated_task_audit_linkage: HashMap<String, DelegatedTaskAuditLinkageRecord>,
    protocol_phase4_runtime_audit: HashMap<String, Phase4RuntimeAuditRecord>,
    protocol_agent_replay_manifests: HashMap<AgentRunId, AgentReplayManifest>,
    protocol_collaboration_audit: HashMap<String, CollaborationAuditRecord>,
    protocol_remote_audit: HashMap<String, RemoteAuditRecord>,
    protocol_remote_transport_audit: HashMap<String, RemoteTransportAuditSummary>,
    protocol_terminal_audit: HashMap<String, TerminalAuditRecord>,
    protocol_debug_breakpoints: HashMap<String, DebugBreakpointRecord>,
    protocol_debug_adapter_audit: HashMap<String, DebugAdapterAuditRecord>,
    protocol_hosted_telemetry_spool: HashMap<String, HostedTelemetrySpoolRecord>,
    protocol_raw_source_retention_access_audit: HashMap<String, RawSourceRetentionAccessAudit>,
    protocol_event_metadata: HashMap<EventId, EventMetadataRecord>,
    protocol_semantic_metadata: HashMap<String, SemanticMetadataRecord>,
    protocol_semantic_tombstones: Vec<SemanticMetadataTombstone>,
    protocol_plugin_storage: HashMap<String, PluginStorageRecord>,
}

#[derive(Debug)]
/// JSON file-backed storage implementation with corruption quarantine behavior.
pub struct FileBackedStorage {
    path: PathBuf,
    state: InMemoryStorage,
}

/// Explicit metadata-only storage migration registry.
#[derive(Debug, Clone)]
pub struct StorageMigrationRegistry {
    active_schema_version: u16,
    steps: Vec<StorageMigrationStep>,
}

/// Outcome of applying or recovering storage migration files.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StorageMigrationApplyOutcome {
    /// Backup marker written before mutation.
    pub backup: StorageBackupMarker,
    /// Recovery outcome metadata after apply or recovery.
    pub recovery: StorageRecoveryOutcome,
}

impl StorageMigrationRegistry {
    /// Create a migration registry for the current active schema version.
    pub fn new(active_schema_version: u16) -> Self {
        Self {
            active_schema_version,
            steps: Vec::new(),
        }
    }

    /// Register one explicit forward migration step.
    pub fn register(&mut self, step: StorageMigrationStep) -> StorageResult<()> {
        if step.from_schema_version == 0
            || step.to_schema_version <= step.from_schema_version
            || step.migration_id.trim().is_empty()
            || step.subsystem_id.trim().is_empty()
            || step.schema_version == 0
        {
            return Err(StorageError::Failed {
                message: "storage migration step must be explicit and forward-only".to_string(),
            });
        }
        self.steps.push(step);
        Ok(())
    }

    /// Produce a metadata-only dry-run report for a manifest and registered step.
    pub fn dry_run(
        &self,
        manifest: StorageSchemaManifest,
        target_schema_version: u16,
        correlation_id: CorrelationId,
        causality_id: CausalityId,
    ) -> StorageResult<StorageMigrationDryRunReport> {
        validate_storage_schema_manifest(&manifest).map_err(StorageError::from_protocol)?;
        let step = self
            .steps
            .iter()
            .find(|step| {
                step.subsystem_id == manifest.subsystem_id
                    && step.from_schema_version == manifest.active_schema_version
                    && step.to_schema_version == target_schema_version
            })
            .cloned()
            .ok_or_else(|| StorageError::Failed {
                message: "no registered storage migration step matches manifest".to_string(),
            })?;
        let report = StorageMigrationDryRunReport {
            step,
            compatible: target_schema_version >= self.active_schema_version,
            estimated_record_count: 1,
            metadata_summary: format!(
                "subsystem={} from={} to={} dry_run=true",
                manifest.subsystem_id, manifest.active_schema_version, target_schema_version
            ),
            event_sequence: EventSequence(correlation_id.0.max(1)),
            correlation_id,
            causality_id,
            redaction_hints: vec![devil_protocol::RedactionHint::MetadataOnly],
            schema_version: 1,
        };
        validate_storage_migration_dry_run_report(&report).map_err(StorageError::from_protocol)?;
        Ok(report)
    }

    /// Backup a storage file and return a checksum-bearing marker.
    pub fn backup_file(
        &self,
        path: &Path,
        backup_dir: &Path,
        subsystem_id: impl Into<String>,
        correlation_id: CorrelationId,
        causality_id: CausalityId,
    ) -> StorageResult<StorageBackupMarker> {
        fs::create_dir_all(backup_dir).map_err(|err| StorageError::Failed {
            message: format!("create backup directory: {err}"),
        })?;
        let bytes = fs::read(path).map_err(|err| StorageError::Failed {
            message: format!("read storage before backup: {err}"),
        })?;
        let backup_id = format!("backup-{}-{}", correlation_id.0.max(1), causality_id.0);
        let backup_path = backup_dir.join(format!("{backup_id}.json"));
        write_file_atomically(&backup_path, &bytes)?;
        let marker = StorageBackupMarker {
            backup_id,
            subsystem_id: subsystem_id.into(),
            location_label: backup_path.to_string_lossy().into_owned(),
            checksum: StorageChecksum {
                algorithm: STORAGE_CHECKSUM_ALGORITHM.to_string(),
                value: stable_storage_sum(&bytes),
                schema_version: 1,
            },
            event_sequence: EventSequence(correlation_id.0.max(1)),
            correlation_id,
            causality_id,
            schema_version: 1,
        };
        validate_storage_backup_marker(&marker).map_err(StorageError::from_protocol)?;
        Ok(marker)
    }

    /// Recover a storage file from an explicit backup marker and repair request.
    pub fn recover_from_backup(
        &self,
        destination: &Path,
        backup: &StorageBackupMarker,
        repair: &StorageRepairRequest,
    ) -> StorageResult<StorageRecoveryOutcome> {
        validate_storage_backup_marker(backup).map_err(StorageError::from_protocol)?;
        validate_storage_repair_request(repair).map_err(StorageError::from_protocol)?;
        if backup.checksum.algorithm != STORAGE_CHECKSUM_ALGORITHM {
            return Err(StorageError::Failed {
                message: "storage backup checksum algorithm mismatch".to_string(),
            });
        }
        let bytes = fs::read(&backup.location_label).map_err(|err| StorageError::Failed {
            message: format!("read storage backup: {err}"),
        })?;
        if stable_storage_sum(&bytes) != backup.checksum.value {
            return Err(StorageError::Failed {
                message: "storage backup checksum mismatch".to_string(),
            });
        }
        write_file_atomically(destination, &bytes)?;
        let outcome = StorageRecoveryOutcome {
            recovery_id: format!("recovery-{}", repair.correlation_id.0.max(1)),
            subsystem_id: backup.subsystem_id.clone(),
            recovered: true,
            quarantined: false,
            backup_id: Some(backup.backup_id.clone()),
            metadata_summary: "recovered=true source=backup checksum=verified".to_string(),
            event_sequence: repair.event_sequence,
            correlation_id: repair.correlation_id,
            causality_id: repair.causality_id,
            redaction_hints: vec![devil_protocol::RedactionHint::MetadataOnly],
            schema_version: 1,
        };
        validate_storage_recovery_outcome(&outcome).map_err(StorageError::from_protocol)?;
        Ok(outcome)
    }
}

fn write_file_atomically(path: &Path, body: &[u8]) -> StorageResult<()> {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent).map_err(|err| StorageError::Failed {
        message: format!("create storage directory failed: {err}"),
    })?;
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("storage-file");
    let temp = parent.join(format!(
        ".{file_name}.{}.{}.tmp",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or(0)
    ));
    let result = (|| -> StorageResult<()> {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp)
            .map_err(|err| StorageError::Failed {
                message: format!("create storage temp file failed: {err}"),
            })?;
        file.write_all(body).map_err(|err| StorageError::Failed {
            message: format!("write storage temp file failed: {err}"),
        })?;
        file.flush().map_err(|err| StorageError::Failed {
            message: format!("flush storage temp file failed: {err}"),
        })?;
        file.sync_all().map_err(|err| StorageError::Failed {
            message: format!("sync storage temp file failed: {err}"),
        })?;
        drop(file);
        atomic_replace(&temp, path).map_err(|err| StorageError::Failed {
            message: format!("replace storage file failed: {err}"),
        })?;
        sync_parent_directory_when_supported(parent).map_err(|err| StorageError::Failed {
            message: format!("sync storage directory failed: {err}"),
        })
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temp);
    }
    result
}

fn stable_storage_sum(bytes: &[u8]) -> String {
    let sum = bytes
        .iter()
        .fold(0u64, |acc, byte| acc.wrapping_add(*byte as u64));
    format!("sum:{sum}:len:{}", bytes.len())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PersistedState {
    schema_version: u16,
    workspace_configs: HashMap<WorkspaceId, WorkspaceConfigRecord>,
    trust: HashMap<(WorkspaceId, String), TrustDecisionRecord>,
    metadata: HashMap<(WorkspaceId, String), FileMetadataRecord>,
    sessions: HashMap<String, SessionRecord>,
    #[serde(default)]
    dock_layouts: HashMap<String, DockLayoutStorageRecord>,
    #[serde(default)]
    protocol_proposal_audit: HashMap<ProposalId, ProposalAuditRecord>,
    #[serde(default)]
    protocol_assisted_ai_audit: HashMap<String, AssistedAiAuditRecord>,
    #[serde(default)]
    protocol_delegated_task_audit_linkage: HashMap<String, DelegatedTaskAuditLinkageRecord>,
    #[serde(default)]
    protocol_phase4_runtime_audit: HashMap<String, Phase4RuntimeAuditRecord>,
    #[serde(default)]
    protocol_agent_replay_manifests: HashMap<AgentRunId, AgentReplayManifest>,
    #[serde(default)]
    protocol_collaboration_audit: HashMap<String, CollaborationAuditRecord>,
    #[serde(default)]
    protocol_remote_audit: HashMap<String, RemoteAuditRecord>,
    #[serde(default)]
    protocol_remote_transport_audit: HashMap<String, RemoteTransportAuditSummary>,
    #[serde(default)]
    protocol_terminal_audit: HashMap<String, TerminalAuditRecord>,
    #[serde(default)]
    protocol_debug_breakpoints: HashMap<String, DebugBreakpointRecord>,
    #[serde(default)]
    protocol_debug_adapter_audit: HashMap<String, DebugAdapterAuditRecord>,
    #[serde(default)]
    protocol_hosted_telemetry_spool: HashMap<String, HostedTelemetrySpoolRecord>,
    #[serde(default)]
    protocol_raw_source_retention_access_audit: HashMap<String, RawSourceRetentionAccessAudit>,
    #[serde(default)]
    protocol_event_metadata: HashMap<EventId, EventMetadataRecord>,
    semantic_metadata: HashMap<String, SemanticMetadataRecord>,
    semantic_tombstones: Vec<SemanticMetadataTombstone>,
    #[serde(default)]
    plugin_storage: HashMap<String, PluginStorageRecord>,
}

impl From<&InMemoryStorage> for PersistedState {
    fn from(value: &InMemoryStorage) -> Self {
        Self {
            schema_version: 3,
            workspace_configs: value.workspace_configs.clone(),
            trust: value.trust.clone(),
            metadata: value.metadata.clone(),
            sessions: value.sessions.clone(),
            dock_layouts: value.dock_layouts.clone(),
            protocol_proposal_audit: value.protocol_proposal_audit.clone(),
            protocol_assisted_ai_audit: value.protocol_assisted_ai_audit.clone(),
            protocol_delegated_task_audit_linkage: value
                .protocol_delegated_task_audit_linkage
                .clone(),
            protocol_phase4_runtime_audit: value.protocol_phase4_runtime_audit.clone(),
            protocol_agent_replay_manifests: value.protocol_agent_replay_manifests.clone(),
            protocol_collaboration_audit: value.protocol_collaboration_audit.clone(),
            protocol_remote_audit: value.protocol_remote_audit.clone(),
            protocol_remote_transport_audit: value.protocol_remote_transport_audit.clone(),
            protocol_terminal_audit: value.protocol_terminal_audit.clone(),
            protocol_debug_breakpoints: value.protocol_debug_breakpoints.clone(),
            protocol_debug_adapter_audit: value.protocol_debug_adapter_audit.clone(),
            protocol_hosted_telemetry_spool: value.protocol_hosted_telemetry_spool.clone(),
            protocol_raw_source_retention_access_audit: value
                .protocol_raw_source_retention_access_audit
                .clone(),
            protocol_event_metadata: value.protocol_event_metadata.clone(),
            semantic_metadata: value.protocol_semantic_metadata.clone(),
            semantic_tombstones: value.protocol_semantic_tombstones.clone(),
            plugin_storage: value.protocol_plugin_storage.clone(),
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
            dock_layouts: self.dock_layouts.clone(),
            protocol_workspace_configs: self.protocol_workspace_configs.clone(),
            protocol_file_metadata: self.protocol_file_metadata.clone(),
            protocol_sessions: self.protocol_sessions.clone(),
            protocol_trust: self.protocol_trust.clone(),
            protocol_proposal_audit: self.protocol_proposal_audit.clone(),
            protocol_assisted_ai_audit: self.protocol_assisted_ai_audit.clone(),
            protocol_delegated_task_audit_linkage: self
                .protocol_delegated_task_audit_linkage
                .clone(),
            protocol_phase4_runtime_audit: self.protocol_phase4_runtime_audit.clone(),
            protocol_agent_replay_manifests: self.protocol_agent_replay_manifests.clone(),
            protocol_collaboration_audit: self.protocol_collaboration_audit.clone(),
            protocol_remote_audit: self.protocol_remote_audit.clone(),
            protocol_remote_transport_audit: self.protocol_remote_transport_audit.clone(),
            protocol_terminal_audit: self.protocol_terminal_audit.clone(),
            protocol_debug_breakpoints: self.protocol_debug_breakpoints.clone(),
            protocol_debug_adapter_audit: self.protocol_debug_adapter_audit.clone(),
            protocol_hosted_telemetry_spool: self.protocol_hosted_telemetry_spool.clone(),
            protocol_raw_source_retention_access_audit: self
                .protocol_raw_source_retention_access_audit
                .clone(),
            protocol_event_metadata: self.protocol_event_metadata.clone(),
            protocol_semantic_metadata: self.protocol_semantic_metadata.clone(),
            protocol_semantic_tombstones: self.protocol_semantic_tombstones.clone(),
            protocol_plugin_storage: self.protocol_plugin_storage.clone(),
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
            dock_layouts: value.dock_layouts,
            protocol_workspace_configs: HashMap::new(),
            protocol_file_metadata: HashMap::new(),
            protocol_sessions: HashMap::new(),
            protocol_trust: HashMap::new(),
            protocol_proposal_audit: value.protocol_proposal_audit,
            protocol_assisted_ai_audit: value.protocol_assisted_ai_audit,
            protocol_delegated_task_audit_linkage: value.protocol_delegated_task_audit_linkage,
            protocol_phase4_runtime_audit: value.protocol_phase4_runtime_audit,
            protocol_agent_replay_manifests: value.protocol_agent_replay_manifests,
            protocol_collaboration_audit: value.protocol_collaboration_audit,
            protocol_remote_audit: value.protocol_remote_audit,
            protocol_remote_transport_audit: value.protocol_remote_transport_audit,
            protocol_terminal_audit: value.protocol_terminal_audit,
            protocol_debug_breakpoints: value.protocol_debug_breakpoints,
            protocol_debug_adapter_audit: value.protocol_debug_adapter_audit,
            protocol_hosted_telemetry_spool: value.protocol_hosted_telemetry_spool,
            protocol_raw_source_retention_access_audit: value
                .protocol_raw_source_retention_access_audit,
            protocol_event_metadata: value.protocol_event_metadata,
            protocol_semantic_metadata: value.semantic_metadata,
            protocol_semantic_tombstones: value.semantic_tombstones,
            protocol_plugin_storage: value.plugin_storage,
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

        self.write_atomically(body.as_bytes())
    }

    fn atomic_temp_path(&self) -> StorageResult<PathBuf> {
        let parent = self.path.parent().unwrap_or_else(|| Path::new("."));
        let file_name = self
            .path
            .file_name()
            .map(|value| value.to_string_lossy())
            .unwrap_or_else(|| "storage.json".into());
        let suffix = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|err| StorageError::Failed {
                message: format!("create atomic storage temp timestamp failed: {err}"),
            })?
            .as_nanos();
        Ok(parent.join(format!(
            ".{file_name}.{}.{}.tmp",
            std::process::id(),
            suffix
        )))
    }

    fn write_atomically(&self, body: &[u8]) -> StorageResult<()> {
        let parent = self.path.parent().unwrap_or_else(|| Path::new("."));
        fs::create_dir_all(parent).map_err(|err| StorageError::Failed {
            message: format!("create storage directory failed: {err}"),
        })?;

        let temp = self.atomic_temp_path()?;
        let write_result = (|| -> StorageResult<()> {
            let mut file = OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&temp)
                .map_err(|err| StorageError::Failed {
                    message: format!("create storage temp file failed: {err}"),
                })?;
            file.write_all(body).map_err(|err| StorageError::Failed {
                message: format!("write storage temp file failed: {err}"),
            })?;
            file.flush().map_err(|err| StorageError::Failed {
                message: format!("flush storage temp file failed: {err}"),
            })?;
            file.sync_all().map_err(|err| StorageError::Failed {
                message: format!("sync storage temp file failed: {err}"),
            })?;
            drop(file);
            atomic_replace(&temp, &self.path).map_err(|err| StorageError::Failed {
                message: format!("replace storage file failed: {err}"),
            })?;
            sync_parent_directory_when_supported(parent).map_err(|err| StorageError::Failed {
                message: format!("sync storage directory failed: {err}"),
            })
        })();

        if write_result.is_err() {
            let _ = fs::remove_file(&temp);
        }
        write_result
    }
}

#[cfg(windows)]
fn atomic_replace(temp: &Path, target: &Path) -> std::io::Result<()> {
    use std::os::windows::ffi::OsStrExt;

    const MOVEFILE_REPLACE_EXISTING: u32 = 0x0000_0001;
    const MOVEFILE_WRITE_THROUGH: u32 = 0x0000_0008;

    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn MoveFileExW(existing: *const u16, new_name: *const u16, flags: u32) -> i32;
    }

    fn wide(path: &Path) -> Vec<u16> {
        path.as_os_str().encode_wide().chain(Some(0)).collect()
    }

    let ok = unsafe {
        MoveFileExW(
            wide(temp).as_ptr(),
            wide(target).as_ptr(),
            MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
        )
    };
    if ok == 0 {
        Err(std::io::Error::last_os_error())
    } else {
        Ok(())
    }
}

#[cfg(not(windows))]
fn atomic_replace(temp: &Path, target: &Path) -> std::io::Result<()> {
    fs::rename(temp, target)
}

#[cfg(unix)]
fn sync_parent_directory_when_supported(parent: &Path) -> std::io::Result<()> {
    OpenOptions::new().read(true).open(parent)?.sync_all()
}

#[cfg(not(unix))]
fn sync_parent_directory_when_supported(_parent: &Path) -> std::io::Result<()> {
    Ok(())
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

impl DockLayoutRepository for FileBackedStorage {
    fn save_dock_side_layout(&mut self, record: DockLayoutStorageRecord) -> StorageResult<()> {
        self.state.save_dock_side_layout(record)?;
        self.flush()
    }

    fn load_dock_side_layout(
        &self,
        workspace_id: WorkspaceId,
        mode: &str,
        side: &str,
    ) -> StorageResult<DockLayoutStorageRecord> {
        self.state.load_dock_side_layout(workspace_id, mode, side)
    }

    fn load_dock_layouts(
        &self,
        workspace_id: WorkspaceId,
    ) -> StorageResult<Vec<DockLayoutStorageRecord>> {
        self.state.load_dock_layouts(workspace_id)
    }

    fn delete_dock_side_layout(
        &mut self,
        workspace_id: WorkspaceId,
        mode: &str,
        side: &str,
    ) -> StorageResult<()> {
        self.state
            .delete_dock_side_layout(workspace_id, mode, side)?;
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
            StorageRepositoryRequest::SavePhase4RuntimeAuditRecord(record) => {
                Self::validate_phase4_runtime_audit_record(&record)?;
                let key = record.audit_id.clone();
                self.protocol_phase4_runtime_audit
                    .insert(key.clone(), record);
                Ok(Self::protocol_saved(format!("phase4_runtime_audit:{key}")))
            }
            StorageRepositoryRequest::SaveAgentReplayManifest(manifest) => {
                Self::validate_agent_replay_manifest(&manifest)?;
                let key = manifest.run_id.clone();
                self.protocol_agent_replay_manifests
                    .insert(key.clone(), manifest);
                Ok(Self::protocol_saved(format!(
                    "agent_replay_manifest:{}",
                    key.0
                )))
            }
            StorageRepositoryRequest::SaveCollaborationAuditRecord(record) => {
                Self::validate_collaboration_audit_record(&record)?;
                let key = collaboration_audit_storage_key(record.session_id, record.event_sequence);
                self.protocol_collaboration_audit
                    .insert(key.clone(), record);
                Ok(Self::protocol_saved(format!("collaboration_audit:{key}")))
            }
            StorageRepositoryRequest::SaveRemoteAuditRecord(record) => {
                Self::validate_remote_audit_record(&record)?;
                let key = remote_audit_storage_key(record.session_id, record.event_sequence);
                self.protocol_remote_audit.insert(key.clone(), record);
                Ok(Self::protocol_saved(format!("remote_audit:{key}")))
            }
            StorageRepositoryRequest::SaveRemoteTransportAuditSummary(summary) => {
                Self::validate_remote_transport_audit_summary(&summary)?;
                let key =
                    remote_transport_audit_storage_key(summary.session_id, summary.event_sequence);
                self.protocol_remote_transport_audit
                    .insert(key.clone(), summary);
                Ok(Self::protocol_saved(format!(
                    "remote_transport_audit:{key}"
                )))
            }
            StorageRepositoryRequest::SaveTerminalAuditRecord(record) => {
                Self::validate_terminal_audit_record(&record)?;
                let key = terminal_audit_storage_key(record.session_id, record.event_sequence);
                self.protocol_terminal_audit.insert(key.clone(), record);
                Ok(Self::protocol_saved(format!("terminal_audit:{key}")))
            }
            StorageRepositoryRequest::SaveDebugBreakpointRecord(record) => {
                Self::validate_debug_breakpoint_record(&record)?;
                let key = debug_breakpoint_storage_key(record.workspace_id, &record.breakpoint_id);
                self.protocol_debug_breakpoints.insert(key.clone(), record);
                Ok(Self::protocol_saved(format!("debug_breakpoint:{key}")))
            }
            StorageRepositoryRequest::DeleteDebugBreakpointRecord {
                workspace_id,
                breakpoint_id,
            } => {
                validate_debug_breakpoint_identity(workspace_id, &breakpoint_id).map_err(
                    |error| StorageError::Failed {
                        message: error.message,
                    },
                )?;
                let key = debug_breakpoint_storage_key(workspace_id, &breakpoint_id);
                self.protocol_debug_breakpoints.remove(&key);
                Ok(Self::protocol_saved(format!(
                    "debug_breakpoint_deleted:{key}"
                )))
            }
            StorageRepositoryRequest::SaveDebugAdapterAuditRecord(record) => {
                Self::validate_debug_adapter_audit_record(&record)?;
                let key =
                    debug_adapter_audit_storage_key(&record.session_id, record.event_sequence);
                self.protocol_debug_adapter_audit
                    .insert(key.clone(), record);
                Ok(Self::protocol_saved(format!("debug_adapter_audit:{key}")))
            }
            StorageRepositoryRequest::SaveHostedTelemetrySpoolRecord(record) => {
                Self::validate_hosted_telemetry_spool_record(&record)?;
                let key = record.record_id.clone();
                self.protocol_hosted_telemetry_spool
                    .insert(key.clone(), record);
                Ok(Self::protocol_saved(format!(
                    "hosted_telemetry_spool:{key}"
                )))
            }
            StorageRepositoryRequest::SaveRawSourceRetentionAccessAudit(audit) => {
                Self::validate_raw_source_retention_access_audit(&audit)?;
                let key = raw_source_retention_access_audit_storage_key(
                    &audit.bundle_id,
                    audit.event_sequence,
                );
                self.protocol_raw_source_retention_access_audit
                    .insert(key.clone(), audit);
                Ok(Self::protocol_saved(format!(
                    "raw_source_retention_access_audit:{key}"
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
            StorageRepositoryRequest::PluginStorage(request) => self.handle_plugin_storage(request),
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
            StorageRepositoryRequest::ReadPhase4RuntimeAuditRecord(audit_id) => {
                Ok(StorageRepositoryResponse::Phase4RuntimeAuditRecord(
                    Box::new(self.protocol_phase4_runtime_audit.get(&audit_id).cloned()),
                ))
            }
            StorageRepositoryRequest::ReadAgentReplayManifest(run_id) => {
                Ok(StorageRepositoryResponse::AgentReplayManifest(Box::new(
                    self.protocol_agent_replay_manifests.get(&run_id).cloned(),
                )))
            }
            StorageRepositoryRequest::ReadCollaborationAuditRecord {
                session_id,
                event_sequence,
            } => Ok(StorageRepositoryResponse::CollaborationAuditRecord(
                Box::new(
                    self.protocol_collaboration_audit
                        .get(&collaboration_audit_storage_key(session_id, event_sequence))
                        .cloned(),
                ),
            )),
            StorageRepositoryRequest::ReadRemoteAuditRecord {
                session_id,
                event_sequence,
            } => Ok(StorageRepositoryResponse::RemoteAuditRecord(Box::new(
                self.protocol_remote_audit
                    .get(&remote_audit_storage_key(session_id, event_sequence))
                    .cloned(),
            ))),
            StorageRepositoryRequest::ReadRemoteTransportAuditSummary {
                session_id,
                event_sequence,
            } => Ok(StorageRepositoryResponse::RemoteTransportAuditSummary(
                Box::new(
                    self.protocol_remote_transport_audit
                        .get(&remote_transport_audit_storage_key(
                            session_id,
                            event_sequence,
                        ))
                        .cloned(),
                ),
            )),
            StorageRepositoryRequest::ReadTerminalAuditRecord {
                session_id,
                event_sequence,
            } => Ok(StorageRepositoryResponse::TerminalAuditRecord(Box::new(
                self.protocol_terminal_audit
                    .get(&terminal_audit_storage_key(session_id, event_sequence))
                    .cloned(),
            ))),
            StorageRepositoryRequest::ReadDebugBreakpointRecords { workspace_id } => {
                let mut records = self
                    .protocol_debug_breakpoints
                    .values()
                    .filter(|record| record.workspace_id == workspace_id)
                    .cloned()
                    .collect::<Vec<_>>();
                records.sort_by(|left, right| left.breakpoint_id.0.cmp(&right.breakpoint_id.0));
                Ok(StorageRepositoryResponse::DebugBreakpointRecords(records))
            }
            StorageRepositoryRequest::ReadDebugAdapterAuditRecord {
                session_id,
                event_sequence,
            } => Ok(StorageRepositoryResponse::DebugAdapterAuditRecord(
                Box::new(
                    self.protocol_debug_adapter_audit
                        .get(&debug_adapter_audit_storage_key(
                            &session_id,
                            event_sequence,
                        ))
                        .cloned(),
                ),
            )),
            StorageRepositoryRequest::ReadHostedTelemetrySpoolRecord(record_id) => Ok(
                StorageRepositoryResponse::HostedTelemetrySpoolRecord(Box::new(
                    self.protocol_hosted_telemetry_spool
                        .get(&record_id)
                        .cloned(),
                )),
            ),
            StorageRepositoryRequest::ReadRawSourceRetentionAccessAudit {
                bundle_id,
                event_sequence,
            } => Ok(StorageRepositoryResponse::RawSourceRetentionAccessAudit(
                Box::new(
                    self.protocol_raw_source_retention_access_audit
                        .get(&raw_source_retention_access_audit_storage_key(
                            &bundle_id,
                            event_sequence,
                        ))
                        .cloned(),
                ),
            )),
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

    fn handle_plugin_storage(
        &mut self,
        request: devil_protocol::PluginStorageRequest,
    ) -> StorageResult<StorageRepositoryResponse> {
        if request.plugin_id.0 == 0 || request.namespace.plugin_id != request.plugin_id {
            return Ok(StorageRepositoryResponse::PluginStorage(
                devil_protocol::PluginStorageResponse::Denied {
                    reason: PluginDenialReason::InvalidMetadata,
                    message: "plugin storage namespace escape denied".to_string(),
                },
            ));
        }

        match request.operation {
            PluginStorageOperation::Put => {
                let Some(record) = request.record else {
                    return Ok(StorageRepositoryResponse::PluginStorage(
                        devil_protocol::PluginStorageResponse::Denied {
                            reason: PluginDenialReason::InvalidMetadata,
                            message: "plugin storage put requires a record".to_string(),
                        },
                    ));
                };
                validate_plugin_storage_record(&record).map_err(|err| StorageError::Failed {
                    message: err.message,
                })?;
                let used_without_existing = self.plugin_storage_used_bytes(
                    request.workspace_id,
                    request.plugin_id,
                    Some(&record.key),
                );
                let projected = used_without_existing.saturating_add(record.byte_count);
                if projected > request.quota_bytes {
                    return Ok(StorageRepositoryResponse::PluginStorage(
                        devil_protocol::PluginStorageResponse::Denied {
                            reason: PluginDenialReason::QuotaExceeded,
                            message: "plugin storage quota exceeded".to_string(),
                        },
                    ));
                }
                let key = Self::plugin_storage_key(
                    record.workspace_id,
                    record.plugin_id,
                    &record.namespace.namespace,
                    &record.key,
                );
                let record_key = record.key.clone();
                self.protocol_plugin_storage.insert(key, record);
                Ok(StorageRepositoryResponse::PluginStorage(
                    devil_protocol::PluginStorageResponse::Stored {
                        key: record_key,
                        used_bytes: projected,
                    },
                ))
            }
            PluginStorageOperation::Get => {
                let Some(key) = request.key else {
                    return Ok(StorageRepositoryResponse::PluginStorage(
                        devil_protocol::PluginStorageResponse::Record(None),
                    ));
                };
                let storage_key = Self::plugin_storage_key(
                    request.workspace_id,
                    request.plugin_id,
                    &request.namespace.namespace,
                    &key,
                );
                Ok(StorageRepositoryResponse::PluginStorage(
                    devil_protocol::PluginStorageResponse::Record(
                        self.protocol_plugin_storage.get(&storage_key).cloned(),
                    ),
                ))
            }
            PluginStorageOperation::Delete => {
                if let Some(key) = request.key {
                    let storage_key = Self::plugin_storage_key(
                        request.workspace_id,
                        request.plugin_id,
                        &request.namespace.namespace,
                        &key,
                    );
                    self.protocol_plugin_storage.remove(&storage_key);
                }
                Ok(StorageRepositoryResponse::PluginStorage(
                    devil_protocol::PluginStorageResponse::QuotaUsage {
                        used_bytes: self.plugin_storage_used_bytes(
                            request.workspace_id,
                            request.plugin_id,
                            None,
                        ),
                        quota_bytes: request.quota_bytes,
                    },
                ))
            }
            PluginStorageOperation::List => {
                let mut keys = self
                    .protocol_plugin_storage
                    .values()
                    .filter(|record| {
                        record.workspace_id == request.workspace_id
                            && record.plugin_id == request.plugin_id
                            && record.namespace.namespace == request.namespace.namespace
                    })
                    .map(|record| record.key.clone())
                    .collect::<Vec<_>>();
                keys.sort();
                Ok(StorageRepositoryResponse::PluginStorage(
                    devil_protocol::PluginStorageResponse::Keys(keys),
                ))
            }
            PluginStorageOperation::QuotaUsage => Ok(StorageRepositoryResponse::PluginStorage(
                devil_protocol::PluginStorageResponse::QuotaUsage {
                    used_bytes: self.plugin_storage_used_bytes(
                        request.workspace_id,
                        request.plugin_id,
                        None,
                    ),
                    quota_bytes: request.quota_bytes,
                },
            )),
        }
    }

    fn plugin_storage_key(
        workspace_id: WorkspaceId,
        plugin_id: devil_protocol::PluginId,
        namespace: &str,
        key: &str,
    ) -> String {
        format!("{}:{}:{namespace}:{key}", workspace_id.0, plugin_id.0)
    }

    fn plugin_storage_used_bytes(
        &self,
        workspace_id: WorkspaceId,
        plugin_id: devil_protocol::PluginId,
        excluding_key: Option<&str>,
    ) -> u64 {
        self.protocol_plugin_storage
            .values()
            .filter(|record| {
                record.workspace_id == workspace_id
                    && record.plugin_id == plugin_id
                    && excluding_key != Some(record.key.as_str())
            })
            .map(|record| record.byte_count)
            .sum()
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

    fn validate_phase4_runtime_audit_record(
        record: &Phase4RuntimeAuditRecord,
    ) -> StorageResult<()> {
        validate_phase4_runtime_audit_record(record).map_err(|error| StorageError::Failed {
            message: error.to_string(),
        })
    }

    fn validate_agent_replay_manifest(manifest: &AgentReplayManifest) -> StorageResult<()> {
        validate_agent_replay_manifest(manifest).map_err(|error| StorageError::Failed {
            message: error.to_string(),
        })
    }

    fn validate_collaboration_audit_record(record: &CollaborationAuditRecord) -> StorageResult<()> {
        validate_collaboration_audit_record(record).map_err(|error| StorageError::Failed {
            message: error.message,
        })
    }

    fn validate_remote_audit_record(record: &RemoteAuditRecord) -> StorageResult<()> {
        validate_remote_audit_record(record).map_err(|error| StorageError::Failed {
            message: error.message,
        })
    }

    fn validate_remote_transport_audit_summary(
        summary: &RemoteTransportAuditSummary,
    ) -> StorageResult<()> {
        validate_remote_transport_audit_summary(summary).map_err(|error| StorageError::Failed {
            message: error.message,
        })
    }

    fn validate_terminal_audit_record(record: &TerminalAuditRecord) -> StorageResult<()> {
        validate_terminal_audit_record(record).map_err(|error| StorageError::Failed {
            message: error.message,
        })
    }

    fn validate_debug_breakpoint_record(record: &DebugBreakpointRecord) -> StorageResult<()> {
        validate_debug_breakpoint_record(record).map_err(|error| StorageError::Failed {
            message: error.message,
        })
    }

    fn validate_debug_adapter_audit_record(record: &DebugAdapterAuditRecord) -> StorageResult<()> {
        validate_debug_adapter_audit_record(record).map_err(|error| StorageError::Failed {
            message: error.message,
        })
    }

    fn validate_hosted_telemetry_spool_record(
        record: &HostedTelemetrySpoolRecord,
    ) -> StorageResult<()> {
        validate_hosted_telemetry_spool_record(record).map_err(|error| StorageError::Failed {
            message: error.message,
        })
    }

    fn validate_raw_source_retention_access_audit(
        audit: &RawSourceRetentionAccessAudit,
    ) -> StorageResult<()> {
        validate_raw_source_retention_access_audit(audit).map_err(|error| StorageError::Failed {
            message: error.message,
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

fn collaboration_audit_storage_key(
    session_id: CollaborationSessionId,
    event_sequence: EventSequence,
) -> String {
    format!("{}:{}", session_id.0, event_sequence.0)
}

fn remote_audit_storage_key(
    session_id: RemoteWorkspaceSessionId,
    event_sequence: EventSequence,
) -> String {
    format!("{}:{}", session_id.0, event_sequence.0)
}

fn remote_transport_audit_storage_key(
    session_id: RemoteWorkspaceSessionId,
    event_sequence: EventSequence,
) -> String {
    format!("{}:{}", session_id.0, event_sequence.0)
}

fn terminal_audit_storage_key(
    session_id: TerminalSessionId,
    event_sequence: EventSequence,
) -> String {
    format!("{}:{}", session_id.0, event_sequence.0)
}

fn debug_breakpoint_storage_key(
    workspace_id: WorkspaceId,
    breakpoint_id: &devil_protocol::DebugBreakpointId,
) -> String {
    format!("{}:{}", workspace_id.0, breakpoint_id.0)
}

fn debug_adapter_audit_storage_key(
    session_id: &DebugSessionId,
    event_sequence: EventSequence,
) -> String {
    format!("{}:{}", session_id.0, event_sequence.0)
}

fn raw_source_retention_access_audit_storage_key(
    bundle_id: &str,
    event_sequence: EventSequence,
) -> String {
    format!("{}:{}", bundle_id, event_sequence.0)
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

fn dock_layout_storage_key(workspace_id: WorkspaceId, mode: &str, side: &str) -> String {
    format!("dock_layout:{}:{mode}:{side}", workspace_id.0)
}

fn validate_dock_layout_record(record: &DockLayoutStorageRecord) -> StorageResult<()> {
    validate_dock_layout_key(&record.mode, &record.side)?;
    if record.schema_version == 0 {
        return Err(StorageError::Failed {
            message: "dock layout schema version must be non-zero".to_string(),
        });
    }
    if record.pinned_default_panel_id.trim().is_empty() {
        return Err(StorageError::Failed {
            message: "dock layout pinned default panel id must not be empty".to_string(),
        });
    }
    if record
        .custom_toolkit_panel_ids
        .iter()
        .any(|panel_id| panel_id.trim().is_empty())
    {
        return Err(StorageError::Failed {
            message: "dock layout custom toolkit panel ids must not be empty".to_string(),
        });
    }
    if !record.splitter_fraction.is_finite() || !(0.05..=0.95).contains(&record.splitter_fraction) {
        return Err(StorageError::Failed {
            message: "dock layout splitter fraction must be finite and between 0.05 and 0.95"
                .to_string(),
        });
    }
    Ok(())
}

fn validate_dock_layout_key(mode: &str, side: &str) -> StorageResult<()> {
    if dock_mode_order(mode) == u8::MAX {
        return Err(StorageError::Failed {
            message: format!("unknown dock layout mode `{mode}`"),
        });
    }
    if dock_side_order(side) == u8::MAX {
        return Err(StorageError::Failed {
            message: format!("unknown dock layout side `{side}`"),
        });
    }
    Ok(())
}

fn dock_mode_order(mode: &str) -> u8 {
    match mode {
        "Manual" => 0,
        "Assist" => 1,
        "Delegate" => 2,
        "Automate" => 3,
        _ => u8::MAX,
    }
}

fn dock_side_order(side: &str) -> u8 {
    match side {
        "Left" => 0,
        "Right" => 1,
        "Bottom" => 2,
        _ => u8::MAX,
    }
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

impl DockLayoutRepository for InMemoryStorage {
    fn save_dock_side_layout(&mut self, record: DockLayoutStorageRecord) -> StorageResult<()> {
        validate_dock_layout_record(&record)?;
        let key = dock_layout_storage_key(record.workspace_id, &record.mode, &record.side);
        self.dock_layouts.insert(key, record);
        Ok(())
    }

    fn load_dock_side_layout(
        &self,
        workspace_id: WorkspaceId,
        mode: &str,
        side: &str,
    ) -> StorageResult<DockLayoutStorageRecord> {
        validate_dock_layout_key(mode, side)?;
        let key = dock_layout_storage_key(workspace_id, mode, side);
        self.dock_layouts
            .get(&key)
            .cloned()
            .ok_or(StorageError::NotFound { key })
    }

    fn load_dock_layouts(
        &self,
        workspace_id: WorkspaceId,
    ) -> StorageResult<Vec<DockLayoutStorageRecord>> {
        let mut layouts = self
            .dock_layouts
            .values()
            .filter(|record| record.workspace_id == workspace_id)
            .cloned()
            .collect::<Vec<_>>();
        layouts.sort_by_key(|record| {
            (
                dock_mode_order(&record.mode),
                dock_side_order(&record.side),
                record.pinned_default_panel_id.clone(),
            )
        });
        Ok(layouts)
    }

    fn delete_dock_side_layout(
        &mut self,
        workspace_id: WorkspaceId,
        mode: &str,
        side: &str,
    ) -> StorageResult<()> {
        validate_dock_layout_key(mode, side)?;
        let key = dock_layout_storage_key(workspace_id, mode, side);
        self.dock_layouts
            .remove(&key)
            .map(|_| ())
            .ok_or(StorageError::NotFound { key })
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
        AgentReplayManifest, AgentRunId, AgentStateTransitionRecord,
        AssistedAiAuditOutcomeCategory, AssistedAiAuditPrivacyDisposition,
        AssistedAiAuditRedactionState, AssistedAiProviderInvocationState, ByteRange, CapabilityId,
        DebugBreakpointId, DebugBreakpointRecord, EventId, FileContentVersion, FileFingerprint,
        LanguageId, LineIndexRange, PermissionBudgetEvaluationDisposition,
        Phase4RuntimeAuditRecord, ProposalLifecycleState, ProposalPayloadKind,
        ProposalPayloadSummary, ProposalPrivacyLabel, ProposalRiskLabel,
        ProtocolDiagnosticSeverity, ProtocolTextRange, RedactionHint, RetentionLabel,
        SemanticFileFingerprintIdentity, SemanticFreshnessState, SemanticGrammarVersion,
        SemanticMetadataChunkReference, SemanticMetadataDescriptorIdentity,
        SemanticMetadataDiagnosticSummary, SemanticMetadataFreshnessKey,
        SemanticMetadataSourceKind, SemanticMetadataSymbolRecord, SemanticModelVersion,
        SemanticRecordId, SemanticRecordProvenance, SemanticRecordSource, SemanticSymbolId,
        SnapshotId, TextCoordinate, WorkspaceGeneration,
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

    fn debug_breakpoint_record(
        workspace_id: WorkspaceId,
        breakpoint_id: &str,
    ) -> DebugBreakpointRecord {
        DebugBreakpointRecord {
            breakpoint_id: DebugBreakpointId(breakpoint_id.to_string()),
            workspace_id,
            session_id: None,
            path: CanonicalPath("C:/repo/src/main.rs".to_string()),
            range: ProtocolTextRange {
                start: TextCoordinate {
                    line: 4,
                    character: 0,
                    byte_offset: Some(48),
                    utf16_offset: Some(48),
                },
                end: TextCoordinate {
                    line: 4,
                    character: 0,
                    byte_offset: Some(48),
                    utf16_offset: Some(48),
                },
            },
            enabled: true,
            condition: Some("count > 2".to_string()),
            hit_condition: Some("3".to_string()),
            log_message: Some("count changed".to_string()),
            verified: false,
            message: Some("pending adapter verification".to_string()),
            correlation_id: CorrelationId(900),
            causality_id: non_nil_causality_id(),
            sequence: EventSequence(1),
            schema_version: 1,
        }
    }

    fn storage_repair_request() -> StorageRepairRequest {
        StorageRepairRequest {
            subsystem_id: "file-backed-storage".to_string(),
            principal_id: PrincipalId("storage-owner".to_string()),
            capability_decision: devil_protocol::CapabilityDecision {
                decision_id: devil_protocol::CapabilityDecisionId(99),
                granted: true,
                capability: CapabilityId("storage.migration.repair".to_string()),
                reason: Some("repair approved".to_string()),
            },
            explicit_repair_flag: true,
            metadata_summary: "repair=restore_backup".to_string(),
            event_sequence: EventSequence(99),
            correlation_id: CorrelationId(99),
            causality_id: non_nil_causality_id(),
            schema_version: 1,
        }
    }

    #[test]
    fn migration_registry_dry_run_backup_and_recovery_are_metadata_only() {
        let mut registry = StorageMigrationRegistry::new(2);
        registry
            .register(StorageMigrationStep {
                migration_id: "file-backed-v1-to-v2".to_string(),
                subsystem_id: "file-backed-storage".to_string(),
                from_schema_version: 1,
                to_schema_version: 2,
                destructive: false,
                requires_backup: true,
                schema_version: 1,
            })
            .expect("register step");
        let manifest = StorageSchemaManifest {
            subsystem_id: "file-backed-storage".to_string(),
            store_id: "primary".to_string(),
            active_schema_version: 1,
            min_supported_schema_version: 1,
            max_supported_schema_version: 2,
            metadata_summary: "records=1".to_string(),
            redaction_hints: vec![RedactionHint::MetadataOnly],
            schema_version: 1,
        };
        let report = registry
            .dry_run(manifest, 2, CorrelationId(42), non_nil_causality_id())
            .expect("dry run");
        assert!(report.compatible);
        assert!(!report.metadata_summary.contains("raw_source"));

        let path = temp_storage_path("migration-source");
        let backup_dir = path.with_extension("backup");
        fs::write(&path, "{\"schema_version\":1}").expect("write source");
        let backup = registry
            .backup_file(
                &path,
                &backup_dir,
                "file-backed-storage",
                CorrelationId(43),
                non_nil_causality_id(),
            )
            .expect("backup");
        fs::write(&path, "{\"schema_version\":2}").expect("mutate source");
        let outcome = registry
            .recover_from_backup(&path, &backup, &storage_repair_request())
            .expect("recover");
        assert!(outcome.recovered);
        assert_eq!(
            fs::read_to_string(&path).expect("read recovered"),
            "{\"schema_version\":1}"
        );
        let _ = fs::remove_file(path);
        let _ = fs::remove_dir_all(backup_dir);
    }

    #[test]
    fn migration_backup_uses_collision_safe_backup_ids() {
        let registry = StorageMigrationRegistry::new(2);
        let path = temp_storage_path("migration-collision-source");
        let backup_dir = path.with_extension("backup");
        fs::write(&path, "{\"schema_version\":1}").expect("write source");

        let first = registry
            .backup_file(
                &path,
                &backup_dir,
                "file-backed-storage",
                CorrelationId(43),
                non_nil_causality_id(),
            )
            .expect("first backup");
        let second = registry
            .backup_file(
                &path,
                &backup_dir,
                "file-backed-storage",
                CorrelationId(43),
                serde_json::from_value(json!("018f0000-0000-7000-8000-000000000002"))
                    .expect("valid causality id"),
            )
            .expect("second backup");

        assert_ne!(first.backup_id, second.backup_id);
        assert_ne!(first.location_label, second.location_label);
        let _ = fs::remove_file(path);
        let _ = fs::remove_dir_all(backup_dir);
    }

    #[test]
    fn migration_recovery_rejects_invalid_backup_marker() {
        let registry = StorageMigrationRegistry::new(2);
        let path = temp_storage_path("migration-invalid-marker");
        let backup_dir = path.with_extension("backup");
        fs::write(&path, "{\"schema_version\":1}").expect("write source");
        let backup = registry
            .backup_file(
                &path,
                &backup_dir,
                "file-backed-storage",
                CorrelationId(44),
                non_nil_causality_id(),
            )
            .expect("backup");
        let invalid = StorageBackupMarker {
            subsystem_id: String::new(),
            ..backup
        };
        assert!(matches!(
            registry.recover_from_backup(&path, &invalid, &storage_repair_request()),
            Err(StorageError::Failed { .. })
        ));
        let _ = fs::remove_file(path);
        let _ = fs::remove_dir_all(backup_dir);
    }

    #[test]
    fn migration_recovery_rejects_checksum_algorithm_mismatch() {
        let registry = StorageMigrationRegistry::new(2);
        let path = temp_storage_path("migration-algorithm-mismatch");
        let backup_dir = path.with_extension("backup");
        fs::write(&path, "{\"schema_version\":1}").expect("write source");
        let backup = registry
            .backup_file(
                &path,
                &backup_dir,
                "file-backed-storage",
                CorrelationId(45),
                non_nil_causality_id(),
            )
            .expect("backup");
        let mismatch = StorageBackupMarker {
            checksum: StorageChecksum {
                algorithm: "sha256".to_string(),
                ..backup.checksum.clone()
            },
            ..backup
        };
        assert!(matches!(
            registry.recover_from_backup(&path, &mismatch, &storage_repair_request()),
            Err(StorageError::Failed { message }) if message.contains("algorithm mismatch")
        ));
        let _ = fs::remove_file(path);
        let _ = fs::remove_dir_all(backup_dir);
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

    fn dock_layout_record(
        workspace_id: WorkspaceId,
        mode: &str,
        side: &str,
        pinned_default_panel_id: &str,
    ) -> DockLayoutStorageRecord {
        DockLayoutStorageRecord {
            workspace_id,
            mode: mode.to_string(),
            side: side.to_string(),
            pinned_default_panel_id: pinned_default_panel_id.to_string(),
            custom_toolkit_panel_ids: vec!["symbol_outline".to_string()],
            splitter_fraction: 0.42,
            collapsed: false,
            schema_version: 1,
        }
    }

    fn collaboration_audit_record() -> CollaborationAuditRecord {
        CollaborationAuditRecord {
            session_id: CollaborationSessionId(1001),
            operation_id: Some(devil_protocol::CollaborationOperationId(3001)),
            proposal_id: Some(ProposalId(700)),
            event_sequence: EventSequence(9),
            correlation_id: CorrelationId(7),
            causality_id: non_nil_causality_id(),
            retention_label: RetentionLabel::Audit,
            redaction_hints: vec![RedactionHint::MetadataOnly],
            metadata_summary: "operations=1 participants=2 bytes=42".to_string(),
            schema_version: 1,
        }
    }

    fn remote_audit_record() -> RemoteAuditRecord {
        RemoteAuditRecord {
            session_id: RemoteWorkspaceSessionId(7001),
            operation_id: Some(devil_protocol::RemoteOperationId(8001)),
            proposal_id: Some(ProposalId(700)),
            event_sequence: EventSequence(10),
            correlation_id: CorrelationId(7),
            causality_id: non_nil_causality_id(),
            retention_label: RetentionLabel::Audit,
            redaction_hints: vec![RedactionHint::MetadataOnly],
            metadata_summary: "state=Active files=1 checkpoints=0".to_string(),
            schema_version: 1,
        }
    }

    fn remote_transport_audit_summary() -> RemoteTransportAuditSummary {
        RemoteTransportAuditSummary {
            session_id: RemoteWorkspaceSessionId(7001),
            event_sequence: EventSequence(11),
            correlation_id: CorrelationId(7),
            causality_id: non_nil_causality_id(),
            metadata_summary: "handshake=accepted frames=3".to_string(),
            redaction_hints: vec![RedactionHint::MetadataOnly],
            schema_version: 1,
        }
    }

    fn terminal_audit_record() -> TerminalAuditRecord {
        TerminalAuditRecord {
            session_id: TerminalSessionId(42),
            state: devil_protocol::TerminalRuntimeState::Exited,
            event_sequence: EventSequence(12),
            correlation_id: CorrelationId(7),
            causality_id: non_nil_causality_id(),
            metadata_summary: "exit_code=0 output_bytes=128 truncated=false".to_string(),
            redaction_hints: vec![RedactionHint::MetadataOnly],
            schema_version: 1,
        }
    }

    fn hosted_telemetry_spool_record() -> HostedTelemetrySpoolRecord {
        HostedTelemetrySpoolRecord {
            record_id: "spool-1".to_string(),
            workspace_id: WorkspaceId(1),
            category: devil_protocol::HostedTelemetryCategory::Diagnostics,
            classification: devil_protocol::PrivacyClassification::Metadata,
            metadata_summary: "event_count=1 drop_count=0".to_string(),
            event_sequence: EventSequence(13),
            correlation_id: CorrelationId(7),
            causality_id: non_nil_causality_id(),
            redaction_hints: vec![RedactionHint::MetadataOnly],
            schema_version: 1,
        }
    }

    fn raw_source_retention_access_audit() -> RawSourceRetentionAccessAudit {
        RawSourceRetentionAccessAudit {
            bundle_id: "bundle-1".to_string(),
            principal_id: PrincipalId("tester".to_string()),
            action: "read_descriptor".to_string(),
            event_sequence: EventSequence(14),
            correlation_id: CorrelationId(7),
            causality_id: non_nil_causality_id(),
            redaction_hints: vec![RedactionHint::MetadataOnly],
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
    fn collaboration_audit_storage_roundtrips_metadata_only_and_rejects_raw_source() {
        let storage = InMemoryStorageRepositoryPort::new();
        let record = collaboration_audit_record();
        storage
            .handle(StorageRepositoryRequest::SaveCollaborationAuditRecord(
                record.clone(),
            ))
            .expect("save collaboration audit record");

        let loaded = storage
            .handle(StorageRepositoryRequest::ReadCollaborationAuditRecord {
                session_id: record.session_id,
                event_sequence: record.event_sequence,
            })
            .expect("read collaboration audit record");
        match loaded {
            StorageRepositoryResponse::CollaborationAuditRecord(loaded) => {
                let loaded = loaded.expect("collaboration audit should exist");
                assert_eq!(loaded.session_id, record.session_id);
                assert!(
                    loaded
                        .redaction_hints
                        .contains(&RedactionHint::MetadataOnly)
                );
                assert!(!loaded.metadata_summary.contains("source_text"));
            }
            other => panic!("unexpected collaboration audit response: {other:?}"),
        }

        let mut invalid = collaboration_audit_record();
        invalid.metadata_summary = "raw_transcript=secret source_text".to_string();
        assert!(
            storage
                .handle(StorageRepositoryRequest::SaveCollaborationAuditRecord(
                    invalid
                ))
                .is_err()
        );

        let mut zero_sequence = collaboration_audit_record();
        zero_sequence.event_sequence = EventSequence(0);
        assert!(
            storage
                .handle(StorageRepositoryRequest::SaveCollaborationAuditRecord(
                    zero_sequence
                ))
                .is_err()
        );
    }

    #[test]
    fn remote_audit_storage_roundtrips_metadata_only_and_rejects_raw_payloads() {
        let storage = InMemoryStorageRepositoryPort::new();
        let record = remote_audit_record();
        storage
            .handle(StorageRepositoryRequest::SaveRemoteAuditRecord(
                record.clone(),
            ))
            .expect("save remote audit record");

        let loaded = storage
            .handle(StorageRepositoryRequest::ReadRemoteAuditRecord {
                session_id: record.session_id,
                event_sequence: record.event_sequence,
            })
            .expect("read remote audit record");
        match loaded {
            StorageRepositoryResponse::RemoteAuditRecord(loaded) => {
                let loaded = loaded.expect("remote audit should exist");
                assert_eq!(loaded.session_id, record.session_id);
                assert!(
                    loaded
                        .redaction_hints
                        .contains(&RedactionHint::MetadataOnly)
                );
                assert!(!loaded.metadata_summary.contains("raw_source"));
                assert!(!loaded.metadata_summary.contains("raw_transcript"));
                assert!(!loaded.metadata_summary.contains("process_output"));
            }
            other => panic!("unexpected remote audit response: {other:?}"),
        }

        let mut invalid = remote_audit_record();
        invalid.metadata_summary = "transport_payload=secret process_output".to_string();
        assert!(
            storage
                .handle(StorageRepositoryRequest::SaveRemoteAuditRecord(invalid))
                .is_err()
        );

        let mut zero_sequence = remote_audit_record();
        zero_sequence.event_sequence = EventSequence(0);
        assert!(
            storage
                .handle(StorageRepositoryRequest::SaveRemoteAuditRecord(
                    zero_sequence
                ))
                .is_err()
        );
    }

    #[test]
    fn phase8_metadata_storage_roundtrips_and_rejects_raw_markers() {
        let storage = InMemoryStorageRepositoryPort::new();

        let transport = remote_transport_audit_summary();
        storage
            .handle(StorageRepositoryRequest::SaveRemoteTransportAuditSummary(
                transport.clone(),
            ))
            .expect("save transport audit summary");
        let loaded = storage
            .handle(StorageRepositoryRequest::ReadRemoteTransportAuditSummary {
                session_id: transport.session_id,
                event_sequence: transport.event_sequence,
            })
            .expect("read transport audit summary");
        match loaded {
            StorageRepositoryResponse::RemoteTransportAuditSummary(loaded) => {
                let loaded = loaded.expect("transport audit should exist");
                assert_eq!(loaded.metadata_summary, transport.metadata_summary);
            }
            other => panic!("unexpected transport audit response: {other:?}"),
        }

        let terminal = terminal_audit_record();
        storage
            .handle(StorageRepositoryRequest::SaveTerminalAuditRecord(
                terminal.clone(),
            ))
            .expect("save terminal audit record");
        let loaded = storage
            .handle(StorageRepositoryRequest::ReadTerminalAuditRecord {
                session_id: terminal.session_id,
                event_sequence: terminal.event_sequence,
            })
            .expect("read terminal audit record");
        match loaded {
            StorageRepositoryResponse::TerminalAuditRecord(loaded) => {
                let loaded = loaded.expect("terminal audit should exist");
                assert_eq!(loaded.session_id, terminal.session_id);
            }
            other => panic!("unexpected terminal audit response: {other:?}"),
        }

        let spool = hosted_telemetry_spool_record();
        storage
            .handle(StorageRepositoryRequest::SaveHostedTelemetrySpoolRecord(
                spool.clone(),
            ))
            .expect("save hosted telemetry spool record");
        let loaded = storage
            .handle(StorageRepositoryRequest::ReadHostedTelemetrySpoolRecord(
                spool.record_id.clone(),
            ))
            .expect("read hosted telemetry spool record");
        match loaded {
            StorageRepositoryResponse::HostedTelemetrySpoolRecord(loaded) => {
                let loaded = loaded.expect("telemetry spool record should exist");
                assert_eq!(loaded.record_id, spool.record_id);
            }
            other => panic!("unexpected telemetry spool response: {other:?}"),
        }

        let access = raw_source_retention_access_audit();
        storage
            .handle(StorageRepositoryRequest::SaveRawSourceRetentionAccessAudit(
                access.clone(),
            ))
            .expect("save retention access audit");
        let loaded = storage
            .handle(
                StorageRepositoryRequest::ReadRawSourceRetentionAccessAudit {
                    bundle_id: access.bundle_id.clone(),
                    event_sequence: access.event_sequence,
                },
            )
            .expect("read retention access audit");
        match loaded {
            StorageRepositoryResponse::RawSourceRetentionAccessAudit(loaded) => {
                let loaded = loaded.expect("retention access audit should exist");
                assert_eq!(loaded.bundle_id, access.bundle_id);
            }
            other => panic!("unexpected retention access audit response: {other:?}"),
        }

        let mut invalid_transport = remote_transport_audit_summary();
        invalid_transport.metadata_summary = "transport_payload=raw bytes".to_string();
        assert!(
            storage
                .handle(StorageRepositoryRequest::SaveRemoteTransportAuditSummary(
                    invalid_transport
                ))
                .is_err()
        );

        let mut invalid_terminal = terminal_audit_record();
        invalid_terminal.metadata_summary = "terminal_output=secret".to_string();
        assert!(
            storage
                .handle(StorageRepositoryRequest::SaveTerminalAuditRecord(
                    invalid_terminal
                ))
                .is_err()
        );

        let mut invalid_spool = hosted_telemetry_spool_record();
        invalid_spool.metadata_summary = "raw_source=fn main".to_string();
        assert!(
            storage
                .handle(StorageRepositoryRequest::SaveHostedTelemetrySpoolRecord(
                    invalid_spool
                ))
                .is_err()
        );

        let mut invalid_access = raw_source_retention_access_audit();
        invalid_access.action = "raw_source=fn main".to_string();
        assert!(
            storage
                .handle(StorageRepositoryRequest::SaveRawSourceRetentionAccessAudit(
                    invalid_access
                ))
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
    fn phase4_runtime_audit_and_replay_manifest_roundtrip_metadata_only() {
        let storage = InMemoryStorageRepositoryPort::new();
        let run_id = AgentRunId("phase4-run-storage".to_string());
        let audit = Phase4RuntimeAuditRecord {
            audit_id: "phase4-audit-storage".to_string(),
            run_id: Some(run_id.clone()),
            step_id: None,
            provider_route_id: Some("route-storage".to_string()),
            invocation_state: AssistedAiProviderInvocationState::Completed,
            outcome_label: "phase4.provider.completed".to_string(),
            labels: vec!["metadata-only".to_string()],
            correlation_id: CorrelationId(44),
            causality_id: non_nil_causality_id(),
            event_sequence: EventSequence(55),
            redaction_hints: vec![RedactionHint::MetadataOnly],
            schema_version: 1,
        };
        storage
            .handle(StorageRepositoryRequest::SavePhase4RuntimeAuditRecord(
                audit.clone(),
            ))
            .expect("save phase4 audit");
        match storage
            .handle(StorageRepositoryRequest::ReadPhase4RuntimeAuditRecord(
                audit.audit_id.clone(),
            ))
            .expect("read phase4 audit")
        {
            StorageRepositoryResponse::Phase4RuntimeAuditRecord(stored) => {
                assert_eq!(stored.as_ref(), &Some(audit.clone()));
            }
            other => panic!("unexpected response: {other:?}"),
        }

        let replay = AgentReplayManifest {
            run_id: run_id.clone(),
            transitions: vec![AgentStateTransitionRecord {
                run_id: run_id.clone(),
                step_id: None,
                from_state: devil_protocol::AgentRunState::Observing,
                to_state: devil_protocol::AgentRunState::Planning,
                reason_code: "phase4.replay.storage".to_string(),
                proposal_id: Some(ProposalId(9)),
                correlation_id: CorrelationId(44),
                causality_id: non_nil_causality_id(),
                event_sequence: EventSequence(56),
                redaction_hints: vec![RedactionHint::MetadataOnly],
                schema_version: 1,
            }],
            context_manifests: Vec::new(),
            provider_route_ids: vec!["route-storage".to_string()],
            proposal_ids: vec![ProposalId(9)],
            correlation_id: CorrelationId(44),
            causality_id: non_nil_causality_id(),
            event_sequence: EventSequence(57),
            redaction_hints: vec![RedactionHint::MetadataOnly],
            schema_version: 1,
        };
        storage
            .handle(StorageRepositoryRequest::SaveAgentReplayManifest(
                replay.clone(),
            ))
            .expect("save replay");
        match storage
            .handle(StorageRepositoryRequest::ReadAgentReplayManifest(run_id))
            .expect("read replay")
        {
            StorageRepositoryResponse::AgentReplayManifest(stored) => {
                assert_eq!(stored.as_ref(), &Some(replay));
            }
            other => panic!("unexpected response: {other:?}"),
        }

        let mut raw_marker = audit;
        raw_marker.labels.push("raw prompt".to_string());
        assert!(
            storage
                .handle(StorageRepositoryRequest::SavePhase4RuntimeAuditRecord(
                    raw_marker
                ))
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
    fn in_memory_storage_roundtrips_mode_scoped_dock_layouts() {
        let mut storage = InMemoryStorage::new();
        let manual_left = dock_layout_record(WorkspaceId(55), "Manual", "Left", "project_explorer");
        let assist_right = DockLayoutStorageRecord {
            custom_toolkit_panel_ids: vec!["assistant".to_string(), "context".to_string()],
            collapsed: true,
            ..dock_layout_record(WorkspaceId(55), "Assist", "Right", "assistant")
        };

        storage
            .save_dock_side_layout(manual_left.clone())
            .expect("save manual left layout");
        storage
            .save_dock_side_layout(assist_right.clone())
            .expect("save assist right layout");

        let loaded = storage
            .load_dock_side_layout(WorkspaceId(55), "Manual", "Left")
            .expect("load manual left layout");
        assert_eq!(loaded, manual_left);

        let layouts = storage
            .load_dock_layouts(WorkspaceId(55))
            .expect("load all dock layouts");
        assert_eq!(layouts.len(), 2);
        assert_eq!(layouts[0].mode, "Manual");
        assert_eq!(layouts[1].mode, "Assist");
        assert!(layouts[1].collapsed);

        storage
            .delete_dock_side_layout(WorkspaceId(55), "Manual", "Left")
            .expect("delete manual left layout");
        assert!(
            storage
                .load_dock_side_layout(WorkspaceId(55), "Manual", "Left")
                .is_err()
        );
    }

    #[test]
    fn dock_layout_storage_rejects_invalid_records() {
        let mut storage = InMemoryStorage::new();
        let invalid_mode = dock_layout_record(WorkspaceId(55), "Agents", "Left", "assistant");
        assert!(matches!(
            storage.save_dock_side_layout(invalid_mode),
            Err(StorageError::Failed { message }) if message.contains("unknown dock layout mode")
        ));

        let invalid_side = dock_layout_record(WorkspaceId(55), "Manual", "Center", "assistant");
        assert!(matches!(
            storage.save_dock_side_layout(invalid_side),
            Err(StorageError::Failed { message }) if message.contains("unknown dock layout side")
        ));

        let invalid_splitter = DockLayoutStorageRecord {
            splitter_fraction: f32::NAN,
            ..dock_layout_record(WorkspaceId(55), "Manual", "Left", "project_explorer")
        };
        assert!(matches!(
            storage.save_dock_side_layout(invalid_splitter),
            Err(StorageError::Failed { message }) if message.contains("splitter fraction")
        ));

        let zero_schema = DockLayoutStorageRecord {
            schema_version: 0,
            ..dock_layout_record(WorkspaceId(55), "Manual", "Left", "project_explorer")
        };
        assert!(matches!(
            storage.save_dock_side_layout(zero_schema),
            Err(StorageError::Failed { message }) if message.contains("schema version")
        ));
    }

    #[test]
    fn file_backed_storage_roundtrips_protocol_audit_and_event_metadata() {
        let path = temp_storage_path("protocol-roundtrip");
        let audit = audit_record();
        let event = event_metadata_record();
        let assisted = assisted_ai_audit_record();
        let delegated = delegated_task_audit_linkage_record();
        let collaboration = collaboration_audit_record();

        {
            let mut storage = FileBackedStorage::open(&path).expect("open storage");
            storage
                .state
                .handle_protocol_request(StorageRepositoryRequest::SaveProposalAuditRecord(
                    audit.clone(),
                ))
                .expect("save proposal audit");
            storage
                .state
                .handle_protocol_request(StorageRepositoryRequest::SaveEventMetadata(event.clone()))
                .expect("save event metadata");
            storage
                .state
                .handle_protocol_request(StorageRepositoryRequest::SaveAssistedAiAuditRecord(
                    assisted.clone(),
                ))
                .expect("save assisted AI audit");
            storage
                .state
                .handle_protocol_request(
                    StorageRepositoryRequest::SaveDelegatedTaskAuditLinkageRecord(
                        delegated.clone(),
                    ),
                )
                .expect("save delegated task linkage");
            storage
                .state
                .handle_protocol_request(StorageRepositoryRequest::SaveCollaborationAuditRecord(
                    collaboration.clone(),
                ))
                .expect("save collaboration audit");
            storage.flush().expect("flush storage");
        }

        let mut reopened = FileBackedStorage::open(&path).expect("reopen storage");
        assert!(
            !fs::read_to_string(&path)
                .expect("read persisted state")
                .contains("raw prompt"),
            "persisted state must stay metadata-only"
        );

        match reopened
            .state
            .handle_protocol_request(StorageRepositoryRequest::ReadProposalAuditRecord(
                audit.proposal_id,
            ))
            .expect("read proposal audit")
        {
            StorageRepositoryResponse::ProposalAuditRecord(Some(loaded)) => {
                assert_eq!(loaded.proposal_id, audit.proposal_id);
                assert_eq!(loaded.schema_version, 1);
            }
            other => panic!("unexpected proposal audit response: {other:?}"),
        }
        match reopened
            .state
            .handle_protocol_request(StorageRepositoryRequest::ReadEventMetadata(event.event_id))
            .expect("read event metadata")
        {
            StorageRepositoryResponse::EventMetadata(Some(loaded)) => {
                assert_eq!(loaded.event_id, event.event_id);
                assert_eq!(loaded.sequence, EventSequence(1));
            }
            other => panic!("unexpected event metadata response: {other:?}"),
        }
        match reopened
            .state
            .handle_protocol_request(StorageRepositoryRequest::ReadAssistedAiAuditRecord(
                assisted.audit_id.clone(),
            ))
            .expect("read assisted audit")
        {
            StorageRepositoryResponse::AssistedAiAuditRecord(loaded) => {
                assert_eq!(loaded.expect("assisted audit").audit_id, assisted.audit_id);
            }
            other => panic!("unexpected assisted audit response: {other:?}"),
        }
        match reopened
            .state
            .handle_protocol_request(
                StorageRepositoryRequest::ReadDelegatedTaskAuditLinkageRecord(
                    delegated.linkage_id.clone(),
                ),
            )
            .expect("read delegated linkage")
        {
            StorageRepositoryResponse::DelegatedTaskAuditLinkageRecord(loaded) => {
                assert_eq!(
                    loaded.expect("delegated linkage").linkage_id,
                    delegated.linkage_id
                );
            }
            other => panic!("unexpected delegated linkage response: {other:?}"),
        }
        match reopened
            .state
            .handle_protocol_request(StorageRepositoryRequest::ReadCollaborationAuditRecord {
                session_id: collaboration.session_id,
                event_sequence: collaboration.event_sequence,
            })
            .expect("read collaboration audit")
        {
            StorageRepositoryResponse::CollaborationAuditRecord(loaded) => {
                assert_eq!(
                    loaded.expect("collaboration audit").metadata_summary,
                    collaboration.metadata_summary
                );
            }
            other => panic!("unexpected collaboration audit response: {other:?}"),
        }

        let _ = fs::remove_file(path);
    }

    #[test]
    fn file_backed_protocol_debug_breakpoint_delete_survives_restore() {
        let path = temp_storage_path("debug-breakpoint-delete");
        let workspace_id = WorkspaceId(91);
        let breakpoint_id = DebugBreakpointId("bp-delete".to_string());
        let record = debug_breakpoint_record(workspace_id, &breakpoint_id.0);

        {
            let mut storage = FileBackedStorage::open(&path).expect("open storage");
            storage
                .state
                .handle_protocol_request(StorageRepositoryRequest::SaveDebugBreakpointRecord(
                    record,
                ))
                .expect("save breakpoint");
            storage.flush().expect("flush saved breakpoint");
        }

        {
            let mut storage = FileBackedStorage::open(&path).expect("reopen saved breakpoint");
            match storage
                .state
                .handle_protocol_request(StorageRepositoryRequest::ReadDebugBreakpointRecords {
                    workspace_id,
                })
                .expect("read saved breakpoint")
            {
                StorageRepositoryResponse::DebugBreakpointRecords(records) => {
                    assert_eq!(records.len(), 1);
                }
                other => panic!("unexpected breakpoint read response: {other:?}"),
            }
            storage
                .state
                .handle_protocol_request(StorageRepositoryRequest::DeleteDebugBreakpointRecord {
                    workspace_id,
                    breakpoint_id: breakpoint_id.clone(),
                })
                .expect("delete breakpoint");
            storage.flush().expect("flush deleted breakpoint");
        }

        let mut reopened = FileBackedStorage::open(&path).expect("reopen deleted breakpoint");
        match reopened
            .state
            .handle_protocol_request(StorageRepositoryRequest::ReadDebugBreakpointRecords {
                workspace_id,
            })
            .expect("read after delete")
        {
            StorageRepositoryResponse::DebugBreakpointRecords(records) => {
                assert!(records.is_empty());
            }
            other => panic!("unexpected breakpoint read response after delete: {other:?}"),
        }

        let _ = fs::remove_file(path);
    }

    #[test]
    fn file_backed_storage_opens_schema_one_without_protocol_metadata() {
        let path = temp_storage_path("schema-one-migration");
        fs::write(
            &path,
            r#"{
                "schema_version": 1,
                "workspace_configs": {},
                "trust": {},
                "metadata": {},
                "sessions": {},
                "semantic_metadata": {},
                "semantic_tombstones": []
            }"#,
        )
        .expect("write schema one state");

        let storage = FileBackedStorage::open(&path).expect("open schema one storage");
        assert!(storage.state.protocol_proposal_audit.is_empty());
        assert!(storage.state.protocol_event_metadata.is_empty());
        assert!(
            fs::read_to_string(&path)
                .expect("read migrated state")
                .contains("\"schema_version\": 3")
        );

        let _ = fs::remove_file(path);
    }

    #[test]
    fn plugin_storage_namespace_isolation_and_quota_fail_closed() {
        let storage = InMemoryStorageRepositoryPort::new();
        let namespace = devil_protocol::PluginStateNamespace {
            plugin_id: devil_protocol::PluginId(7),
            namespace: "state".to_string(),
        };
        let record = PluginStorageRecord {
            workspace_id: WorkspaceId(1),
            plugin_id: devil_protocol::PluginId(7),
            namespace: namespace.clone(),
            key: "settings".to_string(),
            value: "metadata-only".to_string(),
            schema_version: 1,
            retention: devil_protocol::RetentionLabel::Warm,
            redaction: RedactionHint::MetadataOnly,
            byte_count: 13,
        };

        let put = storage
            .handle(StorageRepositoryRequest::PluginStorage(
                devil_protocol::PluginStorageRequest {
                    operation: PluginStorageOperation::Put,
                    workspace_id: WorkspaceId(1),
                    plugin_id: devil_protocol::PluginId(7),
                    namespace: namespace.clone(),
                    key: Some("settings".to_string()),
                    record: Some(record.clone()),
                    quota_bytes: 32,
                    correlation_id: CorrelationId(9),
                },
            ))
            .expect("put plugin storage");
        assert!(matches!(
            put,
            StorageRepositoryResponse::PluginStorage(
                devil_protocol::PluginStorageResponse::Stored { used_bytes: 13, .. }
            )
        ));

        let get = storage
            .handle(StorageRepositoryRequest::PluginStorage(
                devil_protocol::PluginStorageRequest {
                    operation: PluginStorageOperation::Get,
                    workspace_id: WorkspaceId(1),
                    plugin_id: devil_protocol::PluginId(7),
                    namespace: namespace.clone(),
                    key: Some("settings".to_string()),
                    record: None,
                    quota_bytes: 32,
                    correlation_id: CorrelationId(10),
                },
            ))
            .expect("get plugin storage");
        assert!(matches!(
            get,
            StorageRepositoryResponse::PluginStorage(devil_protocol::PluginStorageResponse::Record(
                Some(stored)
            )) if stored.key == "settings"
        ));

        let escape = storage
            .handle(StorageRepositoryRequest::PluginStorage(
                devil_protocol::PluginStorageRequest {
                    operation: PluginStorageOperation::Get,
                    workspace_id: WorkspaceId(1),
                    plugin_id: devil_protocol::PluginId(8),
                    namespace: namespace.clone(),
                    key: Some("settings".to_string()),
                    record: None,
                    quota_bytes: 32,
                    correlation_id: CorrelationId(11),
                },
            ))
            .expect("namespace escape returns typed denial");
        assert!(matches!(
            escape,
            StorageRepositoryResponse::PluginStorage(
                devil_protocol::PluginStorageResponse::Denied {
                    reason: PluginDenialReason::InvalidMetadata,
                    ..
                }
            )
        ));

        let mut over_quota = record;
        over_quota.key = "large".to_string();
        over_quota.byte_count = 64;
        let denied = storage
            .handle(StorageRepositoryRequest::PluginStorage(
                devil_protocol::PluginStorageRequest {
                    operation: PluginStorageOperation::Put,
                    workspace_id: WorkspaceId(1),
                    plugin_id: devil_protocol::PluginId(7),
                    namespace,
                    key: Some("large".to_string()),
                    record: Some(over_quota),
                    quota_bytes: 32,
                    correlation_id: CorrelationId(12),
                },
            ))
            .expect("quota returns typed denial");
        assert!(matches!(
            denied,
            StorageRepositoryResponse::PluginStorage(
                devil_protocol::PluginStorageResponse::Denied {
                    reason: PluginDenialReason::QuotaExceeded,
                    ..
                }
            )
        ));
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
    fn file_backed_storage_roundtrips_dock_layouts() {
        let path = temp_storage_path("dock-layout-roundtrip");
        let record = DockLayoutStorageRecord {
            custom_toolkit_panel_ids: vec![
                "diagnostics".to_string(),
                "quick_fixes".to_string(),
                "terminal".to_string(),
            ],
            splitter_fraction: 0.64,
            collapsed: true,
            ..dock_layout_record(WorkspaceId(90), "Delegate", "Bottom", "terminal")
        };

        {
            let mut storage = FileBackedStorage::open(&path).expect("open file-backed storage");
            storage
                .save_dock_side_layout(record.clone())
                .expect("save dock layout");
        }

        let persisted = fs::read_to_string(&path).expect("read persisted storage");
        assert!(persisted.contains("dock_layouts"));
        assert!(persisted.contains("Delegate"));
        assert!(persisted.contains("quick_fixes"));

        let reopened = FileBackedStorage::open(&path).expect("reopen storage");
        let loaded = reopened
            .load_dock_side_layout(WorkspaceId(90), "Delegate", "Bottom")
            .expect("load dock layout");
        assert_eq!(loaded, record);

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
