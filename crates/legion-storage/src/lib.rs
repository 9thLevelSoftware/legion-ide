//! Storage service interfaces for workspace metadata, trust decisions, file metadata cache, and sessions.

#![warn(missing_docs)]

/// Plan revision ledger and audit persistence helpers.
pub mod plan;

/// Local file history metadata store.
pub mod local_history;

/// OS keyring-backed and in-memory secret stores for BYOK provider credentials.
pub mod secrets;

pub use secrets::{
    InMemorySecretStore, OsKeyringSecretStore, PROVIDER_SECRET_SERVICE, SecretReference,
    SecretStore, SecretStoreError, load_provider_api_key, provider_api_key_reference,
    provider_api_key_secret_name, provider_api_key_secret_names, provider_secret_reference,
};

/// Durable checkpoint store for workspace-level file-mutation rollback.
pub mod checkpoint;

use std::collections::{HashMap, HashSet};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{
    Mutex,
    atomic::{AtomicBool, AtomicUsize, Ordering},
};

use legion_observability::{
    EventSinkConfig, SharedEventSink, event_metadata_record, prepare_event_batch, validate_envelope,
};
use legion_protocol::{
    AgentReplayManifest, AgentRunId, AssistedAiAuditRecord, CanonicalPath, CausalityId,
    CheckpointRollbackLimitation, CheckpointRollbackProjection, CollaborationAuditRecord,
    CollaborationSessionId, CorrelationId, DebugAdapterAuditRecord, DebugBreakpointRecord,
    DebugSessionId, DelegatedTaskAuditLinkageRecord, EditablePlanRevisionArtifact, EventEnvelope,
    EventId, EventMetadataRecord, EventSequence, EventSinkPort, EventSinkRequest, FileFingerprint,
    FileId, FileMetadata, HostedTelemetrySpoolRecord, Phase4RuntimeAuditRecord, PluginDenialReason,
    PluginStorageOperation, PluginStorageRecord, PrincipalId, ProposalAuditRecord, ProposalId,
    ProposalLifecycleState, ProtocolError, ProtocolResult, RawSourceRetentionAccessAudit,
    RemoteAuditRecord, RemoteTransportAuditSummary, RemoteWorkspaceSessionId,
    SemanticMetadataBatch, SemanticMetadataFreshnessKey, SemanticMetadataQuery,
    SemanticMetadataReadResult, SemanticMetadataRecord, SemanticMetadataTombstone,
    SemanticMetadataTombstoneReason, SnapshotId, StorageBackupMarker, StorageChecksum,
    StorageMigrationDryRunReport, StorageMigrationStep, StorageRecoveryOutcome,
    StorageRepairRequest, StorageRepositoryPort, StorageRepositoryRequest,
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
use legion_security::TrustState;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use plan::PlanRevisionLedger;

const STORAGE_CHECKSUM_ALGORITHM: &str = "legion-storage-sha256-v1";
/// Legacy non-cryptographic checksum algorithm retained only for verifying backups
/// written before the SHA-256 upgrade.
const LEGACY_STORAGE_CHECKSUM_ALGORITHM: &str = "legion-storage-stable-sum-v1";

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

/// Metadata-only record of how many times a palette item (file path or command
/// id) was confirmed in a given workspace.  No raw query text, no AI context,
/// no telemetry — only an opaque item key and a use counter.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaletteUsageRecord {
    /// Workspace that owns this usage record.
    pub workspace_id: WorkspaceId,
    /// Opaque item key; typically a canonical path or command id string.
    pub item_key: String,
    /// Number of times the item was confirmed in this workspace.
    pub usage_count: u32,
}

/// Metadata-only palette usage counter API.
///
/// Stores per-workspace, per-item confirmation counts so the fuzzy scorer can
/// blend a local frequency bonus into palette ranking.  No query text, AI
/// context, or telemetry leaves the machine through this interface.
pub trait PaletteUsageRepository {
    /// Increment the usage counter for `item_key` in `workspace_id`.
    fn record_usage(&mut self, workspace_id: WorkspaceId, item_key: &str);
    /// Return the usage count for `item_key`, or 0 if not yet recorded.
    fn usage_count(&self, workspace_id: WorkspaceId, item_key: &str) -> u32;
    /// Return all records for the workspace, ordered by descending usage count.
    fn top_items(&self, workspace_id: WorkspaceId) -> Vec<PaletteUsageRecord>;
    /// Clear all usage records for a workspace (e.g. on workspace close).
    fn clear_workspace(&mut self, workspace_id: WorkspaceId);
}

/// In-memory `PaletteUsageRepository` implementation.  Satisfies the
/// metadata-only requirement: no raw query text, no AI context, no network I/O.
#[derive(Debug, Default)]
pub struct InMemoryPaletteUsageRepository {
    counts: HashMap<(WorkspaceId, String), u32>,
}

impl InMemoryPaletteUsageRepository {
    /// Create an empty repository.
    pub fn new() -> Self {
        Self::default()
    }
}

impl PaletteUsageRepository for InMemoryPaletteUsageRepository {
    fn record_usage(&mut self, workspace_id: WorkspaceId, item_key: &str) {
        let entry = self
            .counts
            .entry((workspace_id, item_key.to_string()))
            .or_insert(0);
        *entry = entry.saturating_add(1);
    }

    fn usage_count(&self, workspace_id: WorkspaceId, item_key: &str) -> u32 {
        self.counts
            .get(&(workspace_id, item_key.to_string()))
            .copied()
            .unwrap_or(0)
    }

    fn top_items(&self, workspace_id: WorkspaceId) -> Vec<PaletteUsageRecord> {
        let mut records: Vec<PaletteUsageRecord> = self
            .counts
            .iter()
            .filter(|((ws, _), _)| *ws == workspace_id)
            .map(|((_, key), &count)| PaletteUsageRecord {
                workspace_id,
                item_key: key.clone(),
                usage_count: count,
            })
            .collect();
        records.sort_by_key(|record| std::cmp::Reverse(record.usage_count));
        records
    }

    fn clear_workspace(&mut self, workspace_id: WorkspaceId) {
        self.counts.retain(|(ws, _), _| *ws != workspace_id);
    }
}

// ── FilePaletteUsageRepository ────────────────────────────────────────────────

/// Maximum number of (workspace, item) pairs retained across all workspaces.
/// When the cap is exceeded the entries with the lowest usage counts are evicted.
const PALETTE_USAGE_MAX_ENTRIES: usize = 500;

/// Serialization entry for one (workspace, item) pair.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct PaletteUsagePersistedEntry {
    workspace_id: u128,
    item_key: String,
    usage_count: u32,
}

/// Top-level structure written to the palette-usage JSON file.
#[derive(Debug, Default, Serialize, Deserialize)]
struct PaletteUsageFile {
    /// Schema version for forward-compatible migration.
    #[serde(default)]
    schema_version: u16,
    /// All recorded (workspace, item) usage pairs.
    #[serde(default)]
    entries: Vec<PaletteUsagePersistedEntry>,
}

/// Disk-backed `PaletteUsageRepository`.
///
/// Persists per-workspace palette usage counts to a JSON file using an
/// atomic-rename write pattern (identical to `FileBackedStorage::flush`).
/// On `record_usage`, the in-memory map is updated, the cap is applied, and
/// the file is flushed synchronously.
///
/// **Metadata only**: stores opaque item keys and integer counts — no raw
/// query text, no AI context, no file content.
pub struct FilePaletteUsageRepository {
    path: PathBuf,
    /// In-memory cache of all (workspace_id, item_key) → count pairs.
    counts: HashMap<(WorkspaceId, String), u32>,
}

impl FilePaletteUsageRepository {
    /// Open (or create) the palette-usage file at `path`.
    ///
    /// If the file does not exist an empty repository is returned.  If the
    /// file exists but is corrupt a warning is emitted to stderr and an empty
    /// repository is returned — the file will be overwritten on the next
    /// `record_usage` call.
    pub fn open(path: &Path) -> Self {
        let counts = (|| -> Option<HashMap<(WorkspaceId, String), u32>> {
            let bytes = fs::read(path).ok()?;
            let state: PaletteUsageFile = serde_json::from_slice(&bytes).ok()?;
            Some(
                state
                    .entries
                    .into_iter()
                    .map(|e| ((WorkspaceId(e.workspace_id), e.item_key), e.usage_count))
                    .collect(),
            )
        })()
        .unwrap_or_default();
        Self {
            path: path.to_owned(),
            counts,
        }
    }

    /// Flush the in-memory state to disk using an atomic rename.
    fn flush(&self) -> std::io::Result<()> {
        let file_state = PaletteUsageFile {
            schema_version: 1,
            entries: self
                .counts
                .iter()
                .map(|((ws, key), &count)| PaletteUsagePersistedEntry {
                    workspace_id: ws.0,
                    item_key: key.clone(),
                    usage_count: count,
                })
                .collect(),
        };
        let body = serde_json::to_vec_pretty(&file_state).map_err(std::io::Error::other)?;

        // Atomic rename: write to a temp file then rename.
        let parent = self.path.parent().unwrap_or_else(|| Path::new("."));
        fs::create_dir_all(parent)?;
        let suffix = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let temp_path = parent.join(format!(
            ".palette_usage.{}.{}.tmp",
            std::process::id(),
            suffix
        ));
        let write_result = (|| -> std::io::Result<()> {
            let mut file = OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&temp_path)?;
            file.write_all(&body)?;
            file.flush()?;
            drop(file);
            fs::rename(&temp_path, &self.path)?;
            Ok(())
        })();
        if write_result.is_err() {
            let _ = fs::remove_file(&temp_path);
        }
        write_result
    }

    /// Enforce the entry cap by evicting entries with the lowest usage counts.
    fn apply_cap(&mut self) {
        if self.counts.len() <= PALETTE_USAGE_MAX_ENTRIES {
            return;
        }
        // Collect all entries sorted ascending by count; evict the lowest ones.
        let excess = self.counts.len() - PALETTE_USAGE_MAX_ENTRIES;
        let mut by_count: Vec<_> = self.counts.iter().map(|(k, &v)| (k.clone(), v)).collect();
        by_count.sort_by_key(|(_, count)| *count);
        for (key, _) in by_count.into_iter().take(excess) {
            self.counts.remove(&key);
        }
    }
}

impl PaletteUsageRepository for FilePaletteUsageRepository {
    fn record_usage(&mut self, workspace_id: WorkspaceId, item_key: &str) {
        let entry = self
            .counts
            .entry((workspace_id, item_key.to_string()))
            .or_insert(0);
        *entry = entry.saturating_add(1);
        self.apply_cap();
        // Best-effort flush; silently ignore I/O errors at the repository
        // level — usage history is advisory and a failed write should not
        // surface as a user-facing error.
        let _ = self.flush();
    }

    fn usage_count(&self, workspace_id: WorkspaceId, item_key: &str) -> u32 {
        self.counts
            .get(&(workspace_id, item_key.to_string()))
            .copied()
            .unwrap_or(0)
    }

    fn top_items(&self, workspace_id: WorkspaceId) -> Vec<PaletteUsageRecord> {
        let mut records: Vec<PaletteUsageRecord> = self
            .counts
            .iter()
            .filter(|((ws, _), _)| *ws == workspace_id)
            .map(|((_, key), &count)| PaletteUsageRecord {
                workspace_id,
                item_key: key.clone(),
                usage_count: count,
            })
            .collect();
        records.sort_by_key(|record| std::cmp::Reverse(record.usage_count));
        records
    }

    fn clear_workspace(&mut self, workspace_id: WorkspaceId) {
        self.counts.retain(|(ws, _), _| *ws != workspace_id);
        let _ = self.flush();
    }
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

/// Repository for metadata-only editable plan revision artifacts.
pub trait PlanRevisionRepository {
    /// Persist one audited plan revision.
    fn record_plan_revision(&mut self, revision: EditablePlanRevisionArtifact)
    -> StorageResult<()>;
    /// Read all revisions for a plan in ledger order.
    fn plan_revisions(&self, plan_artifact_id: &str) -> Vec<EditablePlanRevisionArtifact>;
    /// Read the latest revision for a plan.
    fn latest_plan_revision(&self, plan_artifact_id: &str) -> Option<EditablePlanRevisionArtifact>;
}

/// Atomic proposal-created observation payload stored in the transactional outbox.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProposalObservationBatch {
    /// Stable idempotency key for this batch.
    pub batch_id: String,
    /// Proposal-created events delivered atomically to the event sink.
    pub events: Vec<EventEnvelope>,
    /// Durable metadata corresponding one-to-one with `events`.
    pub event_metadata: Vec<EventMetadataRecord>,
    /// Proposal lifecycle audit records committed with the event metadata.
    pub proposal_audits: Vec<ProposalAuditRecord>,
    /// Batch schema version.
    pub schema_version: u16,
}

/// Strict proposal-observation schema written by current producers.
pub const PROPOSAL_OBSERVATION_BATCH_SCHEMA_VERSION: u16 = 2;

// Schema 1 was briefly written before full event/audit field binding shipped.
// It remains readable through a bounded compatibility validator because a
// Pending record may already have been accepted by a sink under its EventId.
const LEGACY_PROPOSAL_OBSERVATION_BATCH_SCHEMA_VERSION: u16 = 1;

/// Delivery state for a proposal observation outbox record.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProposalObservationDeliveryState {
    /// Storage committed, but the atomic sink batch is not acknowledged yet.
    Pending,
    /// The atomic sink batch completed and delivery was durably acknowledged.
    Delivered,
}

/// Durable transactional-outbox record for one proposal observation batch.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProposalObservationOutboxRecord {
    /// Validated observation batch.
    pub batch: ProposalObservationBatch,
    /// Current sink-delivery state.
    pub delivery_state: ProposalObservationDeliveryState,
}

/// Stability classification for a failed Pending outbox delivery attempt.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProposalObservationRetryErrorKind {
    /// A later attempt may succeed without changing the stored batch.
    Transient,
    /// The sink or stored record must be repaired before retry can succeed.
    Permanent,
}

/// Result of retrying one Pending proposal observation batch.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProposalObservationRetryAttempt {
    /// Stable outbox batch id.
    pub batch_id: String,
    /// Resulting delivery state.
    pub delivery_state: ProposalObservationDeliveryState,
    /// Original sink/storage error code, when delivery remains Pending.
    pub error_code: Option<String>,
    /// Metadata-only error classification, when delivery remains Pending.
    pub error_kind: Option<ProposalObservationRetryErrorKind>,
    /// Retry attempt schema version.
    pub schema_version: u16,
}

/// Complete no-starvation report for one Pending-outbox retry pass.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProposalObservationRetryReport {
    /// Per-batch attempts in stable batch-id order.
    pub attempts: Vec<ProposalObservationRetryAttempt>,
    /// Number of batches acknowledged Delivered during this pass.
    pub delivered_count: usize,
    /// Number of batches that remain Pending after this pass.
    pub pending_count: usize,
    /// Retry report schema version.
    pub schema_version: u16,
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
    protocol_proposal_observation_events: HashMap<EventId, EventEnvelope>,
    protocol_proposal_observation_outbox: HashMap<String, ProposalObservationOutboxRecord>,
    protocol_semantic_metadata: HashMap<String, SemanticMetadataRecord>,
    protocol_semantic_tombstones: Vec<SemanticMetadataTombstone>,
    protocol_plugin_storage: HashMap<String, PluginStorageRecord>,
    protocol_plan_revision_ledger: PlanRevisionLedger,
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
            redaction_hints: vec![legion_protocol::RedactionHint::MetadataOnly],
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
                value: storage_checksum(&bytes),
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
        let bytes = fs::read(&backup.location_label).map_err(|err| StorageError::Failed {
            message: format!("read storage backup: {err}"),
        })?;
        let computed = if backup.checksum.algorithm == STORAGE_CHECKSUM_ALGORITHM {
            storage_checksum(&bytes)
        } else if backup.checksum.algorithm == LEGACY_STORAGE_CHECKSUM_ALGORITHM {
            stable_storage_sum(&bytes)
        } else {
            return Err(StorageError::Failed {
                message: "storage backup checksum algorithm mismatch".to_string(),
            });
        };
        if computed != backup.checksum.value {
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
            redaction_hints: vec![legion_protocol::RedactionHint::MetadataOnly],
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

/// Legacy non-cryptographic checksum. Retained only to verify backups written before
/// the SHA-256 upgrade; it is order-insensitive and MUST NOT be used for new markers.
fn stable_storage_sum(bytes: &[u8]) -> String {
    let sum = bytes
        .iter()
        .fold(0u64, |acc, byte| acc.wrapping_add(*byte as u64));
    format!("sum:{sum}:len:{}", bytes.len())
}

/// Compute the integrity checksum for a storage payload as a lowercase SHA-256 hex digest.
///
/// Unlike [`stable_storage_sum`], this is collision-resistant and order-sensitive, so byte
/// permutations or swaps that preserve the naive sum and length are detected.
fn storage_checksum(bytes: &[u8]) -> String {
    let digest = sha256_digest(bytes);
    let mut hex = String::with_capacity(digest.len() * 2);
    for byte in digest {
        hex.push_str(&format!("{byte:02x}"));
    }
    hex
}

/// Pure-Rust SHA-256 (FIPS 180-4) over an in-memory byte slice, returning the 32-byte digest.
///
/// Implemented inline to avoid adding a new crate dependency to `legion-storage`.
fn sha256_digest(message: &[u8]) -> [u8; 32] {
    const K: [u32; 64] = [
        0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4,
        0xab1c5ed5, 0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe,
        0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f,
        0x4a7484aa, 0x5cb0a9dc, 0x76f988da, 0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7,
        0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967, 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc,
        0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85, 0xa2bfe8a1, 0xa81a664b,
        0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070, 0x19a4c116,
        0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
        0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7,
        0xc67178f2,
    ];

    let mut hash: [u32; 8] = [
        0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab,
        0x5be0cd19,
    ];

    let bit_len = (message.len() as u64).wrapping_mul(8);
    let mut padded = message.to_vec();
    padded.push(0x80);
    while padded.len() % 64 != 56 {
        padded.push(0);
    }
    padded.extend_from_slice(&bit_len.to_be_bytes());

    for chunk in padded.chunks_exact(64) {
        let mut w = [0u32; 64];
        for (index, word) in w.iter_mut().enumerate().take(16) {
            let base = index * 4;
            *word = u32::from_be_bytes([
                chunk[base],
                chunk[base + 1],
                chunk[base + 2],
                chunk[base + 3],
            ]);
        }
        for index in 16..64 {
            let s0 = w[index - 15].rotate_right(7)
                ^ w[index - 15].rotate_right(18)
                ^ (w[index - 15] >> 3);
            let s1 = w[index - 2].rotate_right(17)
                ^ w[index - 2].rotate_right(19)
                ^ (w[index - 2] >> 10);
            w[index] = w[index - 16]
                .wrapping_add(s0)
                .wrapping_add(w[index - 7])
                .wrapping_add(s1);
        }

        let mut a = hash[0];
        let mut b = hash[1];
        let mut c = hash[2];
        let mut d = hash[3];
        let mut e = hash[4];
        let mut f = hash[5];
        let mut g = hash[6];
        let mut h = hash[7];

        for index in 0..64 {
            let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let ch = (e & f) ^ ((!e) & g);
            let temp1 = h
                .wrapping_add(s1)
                .wrapping_add(ch)
                .wrapping_add(K[index])
                .wrapping_add(w[index]);
            let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let maj = (a & b) ^ (a & c) ^ (b & c);
            let temp2 = s0.wrapping_add(maj);

            h = g;
            g = f;
            f = e;
            e = d.wrapping_add(temp1);
            d = c;
            c = b;
            b = a;
            a = temp1.wrapping_add(temp2);
        }

        hash[0] = hash[0].wrapping_add(a);
        hash[1] = hash[1].wrapping_add(b);
        hash[2] = hash[2].wrapping_add(c);
        hash[3] = hash[3].wrapping_add(d);
        hash[4] = hash[4].wrapping_add(e);
        hash[5] = hash[5].wrapping_add(f);
        hash[6] = hash[6].wrapping_add(g);
        hash[7] = hash[7].wrapping_add(h);
    }

    let mut digest = [0u8; 32];
    for (index, word) in hash.iter().enumerate() {
        digest[index * 4..index * 4 + 4].copy_from_slice(&word.to_be_bytes());
    }
    digest
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
    protocol_workspace_configs: HashMap<WorkspaceId, WorkspaceConfigSnapshot>,
    #[serde(default)]
    protocol_file_metadata: HashMap<FileId, FileMetadata>,
    #[serde(default)]
    protocol_sessions: HashMap<String, WorkspaceSessionRecord>,
    // Persisted as a flat list because `protocol_trust` is keyed by a `(WorkspaceId, PrincipalId)`
    // tuple, which serde_json cannot encode as a JSON map key. The map is rebuilt on load.
    #[serde(default)]
    protocol_trust: Vec<TrustRecord>,
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
    #[serde(default)]
    protocol_proposal_observation_events: HashMap<EventId, EventEnvelope>,
    #[serde(default)]
    protocol_proposal_observation_outbox: HashMap<String, ProposalObservationOutboxRecord>,
    #[serde(default)]
    protocol_plan_revisions: Vec<EditablePlanRevisionArtifact>,
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
            protocol_workspace_configs: value.protocol_workspace_configs.clone(),
            protocol_file_metadata: value.protocol_file_metadata.clone(),
            protocol_sessions: value.protocol_sessions.clone(),
            protocol_trust: value.protocol_trust.values().cloned().collect(),
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
            protocol_proposal_observation_events: value
                .protocol_proposal_observation_events
                .clone(),
            protocol_proposal_observation_outbox: value
                .protocol_proposal_observation_outbox
                .clone(),
            protocol_plan_revisions: value.protocol_plan_revision_ledger.all_revisions(),
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
            protocol_proposal_observation_events: self.protocol_proposal_observation_events.clone(),
            protocol_proposal_observation_outbox: self.protocol_proposal_observation_outbox.clone(),
            protocol_semantic_metadata: self.protocol_semantic_metadata.clone(),
            protocol_semantic_tombstones: self.protocol_semantic_tombstones.clone(),
            protocol_plugin_storage: self.protocol_plugin_storage.clone(),
            protocol_plan_revision_ledger: self.protocol_plan_revision_ledger.clone(),
        }
    }
}

/// Mutex-backed protocol repository adapter for [`InMemoryStorage`].
///
/// When constructed with [`Self::with_base_dir`] / [`Self::with_event_sink_and_base_dir`],
/// proposal audit records are also persisted under
/// `<base_dir>/proposal-audit/<proposal_id>.json` (atomic rename) and reloaded on open.
/// Omitting `base_dir` keeps purely in-memory behavior for tests.
#[derive(Debug, Default)]
pub struct InMemoryStorageRepositoryPort {
    storage: Mutex<InMemoryStorage>,
    event_sink: SharedEventSink,
    /// Optional workspace-local `.legion/` root for durable proposal audit blobs.
    base_dir: Option<PathBuf>,
    fail_next_proposal_audit_write: AtomicBool,
    fail_next_event_metadata_write: AtomicBool,
    fail_next_plan_revision_write: AtomicBool,
    fail_proposal_observation_batch_at_item: AtomicUsize,
    fail_next_proposal_observation_delivery_persist: AtomicBool,
    proposal_observation_delivery: Mutex<()>,
    proposal_observation_startup_error: Option<ProtocolError>,
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
            base_dir: None,
            fail_next_proposal_audit_write: AtomicBool::new(false),
            fail_next_event_metadata_write: AtomicBool::new(false),
            fail_next_plan_revision_write: AtomicBool::new(false),
            fail_proposal_observation_batch_at_item: AtomicUsize::new(0),
            fail_next_proposal_observation_delivery_persist: AtomicBool::new(false),
            proposal_observation_delivery: Mutex::new(()),
            proposal_observation_startup_error: None,
        }
    }

    /// Construct a protocol storage repository port with an injected audit event sink.
    pub fn with_event_sink(event_sink: SharedEventSink) -> Self {
        Self {
            storage: Mutex::new(InMemoryStorage::new()),
            event_sink,
            base_dir: None,
            fail_next_proposal_audit_write: AtomicBool::new(false),
            fail_next_event_metadata_write: AtomicBool::new(false),
            fail_next_plan_revision_write: AtomicBool::new(false),
            fail_proposal_observation_batch_at_item: AtomicUsize::new(0),
            fail_next_proposal_observation_delivery_persist: AtomicBool::new(false),
            proposal_observation_delivery: Mutex::new(()),
            proposal_observation_startup_error: None,
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
            base_dir: None,
            fail_next_proposal_audit_write: AtomicBool::new(false),
            fail_next_event_metadata_write: AtomicBool::new(false),
            fail_next_plan_revision_write: AtomicBool::new(false),
            fail_proposal_observation_batch_at_item: AtomicUsize::new(0),
            fail_next_proposal_observation_delivery_persist: AtomicBool::new(false),
            proposal_observation_delivery: Mutex::new(()),
            proposal_observation_startup_error: None,
        }
    }

    /// Construct with workspace-local durability under `base_dir` (typically `.legion/`).
    ///
    /// Loads any existing proposal audit JSON blobs from
    /// `base_dir/proposal-audit/` into the in-memory map.
    pub fn with_base_dir(base_dir: impl AsRef<Path>) -> Self {
        Self::with_event_sink_and_base_dir(SharedEventSink::default(), base_dir)
    }

    /// Construct with an event sink and workspace-local durability directory.
    pub fn with_event_sink_and_base_dir(
        event_sink: SharedEventSink,
        base_dir: impl AsRef<Path>,
    ) -> Self {
        let base_dir = base_dir.as_ref().to_path_buf();
        let mut port = Self {
            storage: Mutex::new(InMemoryStorage::new()),
            event_sink,
            base_dir: Some(base_dir),
            fail_next_proposal_audit_write: AtomicBool::new(false),
            fail_next_event_metadata_write: AtomicBool::new(false),
            fail_next_plan_revision_write: AtomicBool::new(false),
            fail_proposal_observation_batch_at_item: AtomicUsize::new(0),
            fail_next_proposal_observation_delivery_persist: AtomicBool::new(false),
            proposal_observation_delivery: Mutex::new(()),
            proposal_observation_startup_error: None,
        };
        if let Err(error) = port
            .load_proposal_observation_outbox_from_disk()
            .and_then(|()| port.load_proposal_audit_from_disk())
        {
            port.proposal_observation_startup_error = Some(error);
        }
        port
    }

    /// Enable durability under `base_dir` on an existing port (loads existing blobs).
    ///
    /// A live port is bound to at most one durability root. Switching roots
    /// would merge independently-owned proposal identities in the in-memory
    /// maps, so it is rejected fail-closed.
    pub fn enable_base_dir(&mut self, base_dir: impl AsRef<Path>) -> ProtocolResult<()> {
        let base_dir = base_dir.as_ref().to_path_buf();
        if self
            .base_dir
            .as_ref()
            .is_some_and(|existing| existing != &base_dir)
        {
            return Err(ProtocolError {
                code: "proposal_observation_workspace_switch_unsupported".to_string(),
                message: "proposal observation storage is already bound to another workspace root"
                    .to_string(),
            });
        }
        self.base_dir = Some(base_dir);
        self.proposal_observation_startup_error = None;
        if let Err(error) = self
            .load_proposal_observation_outbox_from_disk()
            .and_then(|()| self.load_proposal_audit_from_disk())
        {
            self.proposal_observation_startup_error = Some(error.clone());
            return Err(error);
        }
        Ok(())
    }

    fn proposal_audit_dir(&self) -> Option<PathBuf> {
        self.base_dir
            .as_ref()
            .map(|base| base.join("proposal-audit"))
    }

    fn proposal_observation_outbox_dir(&self) -> Option<PathBuf> {
        self.base_dir
            .as_ref()
            .map(|base| base.join("proposal-observation-outbox"))
    }

    fn load_proposal_audit_from_disk(&mut self) -> ProtocolResult<()> {
        let Some(dir) = self.proposal_audit_dir() else {
            return Ok(());
        };
        let entries = match fs::read_dir(&dir) {
            Ok(entries) => entries,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(error) => {
                return Err(Self::outbox_startup_error(format!(
                    "read proposal audit directory failed: {error}"
                )));
            }
        };
        let mut paths = entries
            .map(|entry| {
                entry.map(|entry| entry.path()).map_err(|error| {
                    Self::outbox_startup_error(format!(
                        "read proposal audit directory entry failed: {error}"
                    ))
                })
            })
            .collect::<ProtocolResult<Vec<_>>>()?;
        paths.sort();

        let mut loaded = HashMap::new();
        for path in paths {
            if path.extension().and_then(|e| e.to_str()) != Some("json") {
                continue;
            }
            let bytes = fs::read(&path).map_err(|error| {
                Self::outbox_startup_error(format!(
                    "read proposal audit {} failed: {error}",
                    path.display()
                ))
            })?;
            let record =
                serde_json::from_slice::<ProposalAuditRecord>(&bytes).map_err(|error| {
                    Self::outbox_startup_error(format!(
                        "decode proposal audit {} failed: {error}",
                        path.display()
                    ))
                })?;
            Self::validate_persisted_proposal_audit(&record).map_err(|error| {
                Self::outbox_startup_error(format!(
                    "invalid proposal audit {}: {}",
                    path.display(),
                    error.message
                ))
            })?;
            let expected_stem = record.proposal_id.0.to_string();
            if path.file_stem().and_then(|stem| stem.to_str()) != Some(expected_stem.as_str())
                || loaded.insert(record.proposal_id, record).is_some()
            {
                return Err(Self::outbox_startup_error(format!(
                    "proposal audit identity collision at {}",
                    path.display()
                )));
            }
        }
        let mut storage = self.storage.lock().map_err(Self::poisoned_error)?;
        let mut audits = storage.protocol_proposal_audit.clone();
        for (proposal_id, audit) in loaded {
            if let Some(created) = audits.get(&proposal_id)
                && !Self::proposal_audit_identity_matches(created, &audit)
            {
                return Err(Self::outbox_startup_error(format!(
                    "proposal audit {proposal_id:?} conflicts with outbox identity"
                )));
            }
            audits.insert(proposal_id, audit);
        }
        storage.protocol_proposal_audit = audits;
        Ok(())
    }

    fn persist_proposal_audit(&self, record: &ProposalAuditRecord) -> Result<(), ProtocolError> {
        let Some(dir) = self.proposal_audit_dir() else {
            return Ok(());
        };
        fs::create_dir_all(&dir).map_err(|err| ProtocolError {
            code: "storage_failed".to_string(),
            message: format!("create proposal-audit directory failed: {err}"),
        })?;
        let path = dir.join(format!("{}.json", record.proposal_id.0));
        let body = serde_json::to_vec_pretty(record).map_err(|err| ProtocolError {
            code: "storage_failed".to_string(),
            message: format!("serialize proposal audit failed: {err}"),
        })?;
        atomic_write_bytes(&path, &body).map_err(|err| ProtocolError {
            code: "storage_failed".to_string(),
            message: format!("write proposal audit failed: {err}"),
        })
    }

    fn persist_proposal_observation_outbox(
        &self,
        record: &ProposalObservationOutboxRecord,
    ) -> Result<(), ProtocolError> {
        let Some(dir) = self.proposal_observation_outbox_dir() else {
            return Ok(());
        };
        fs::create_dir_all(&dir).map_err(|error| ProtocolError {
            code: "storage_failed".to_string(),
            message: format!("create proposal observation outbox failed: {error}"),
        })?;
        let path = dir.join(format!("{}.json", record.batch.batch_id));
        let body = serde_json::to_vec_pretty(record).map_err(|error| ProtocolError {
            code: "storage_failed".to_string(),
            message: format!("serialize proposal observation outbox failed: {error}"),
        })?;
        write_file_atomically(&path, &body).map_err(|error| ProtocolError {
            code: "storage_failed".to_string(),
            message: format!("write proposal observation outbox failed: {error}"),
        })
    }

    fn load_proposal_observation_outbox_from_disk(&mut self) -> ProtocolResult<()> {
        let Some(dir) = self.proposal_observation_outbox_dir() else {
            return Ok(());
        };
        let entries = match fs::read_dir(&dir) {
            Ok(entries) => entries,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(error) => {
                return Err(Self::outbox_startup_error(format!(
                    "read proposal observation outbox directory failed: {error}"
                )));
            }
        };
        let mut paths = entries
            .map(|entry| {
                entry.map(|entry| entry.path()).map_err(|error| {
                    Self::outbox_startup_error(format!(
                        "read proposal observation outbox entry failed: {error}"
                    ))
                })
            })
            .collect::<ProtocolResult<Vec<_>>>()?;
        paths.sort();

        let mut records = Vec::new();
        let mut legacy_rewrites = Vec::new();
        let mut loaded_event_ids = HashSet::new();
        let mut loaded_proposal_ids = HashSet::new();
        let mut loaded_batch_ids = HashSet::new();
        for path in paths {
            if path.extension().and_then(|extension| extension.to_str()) != Some("json") {
                continue;
            }
            let bytes = fs::read(&path).map_err(|error| {
                Self::outbox_startup_error(format!(
                    "read proposal observation outbox {} failed: {error}",
                    path.display()
                ))
            })?;
            let mut record = serde_json::from_slice::<ProposalObservationOutboxRecord>(&bytes)
                .map_err(|error| {
                    Self::outbox_startup_error(format!(
                        "decode proposal observation outbox {} failed: {error}",
                        path.display()
                    ))
                })?;
            match record.batch.schema_version {
                PROPOSAL_OBSERVATION_BATCH_SCHEMA_VERSION => {
                    Self::validate_proposal_observation_batch(&record.batch).map_err(|error| {
                        Self::outbox_startup_error(format!(
                            "invalid proposal observation outbox {}: {}",
                            path.display(),
                            error.message
                        ))
                    })?;
                }
                LEGACY_PROPOSAL_OBSERVATION_BATCH_SCHEMA_VERSION => {
                    record.batch.proposal_audits =
                        std::mem::take(&mut record.batch.proposal_audits)
                            .into_iter()
                            .map(Self::sanitize_proposal_observation_audit)
                            .collect::<ProtocolResult<Vec<_>>>()
                            .map_err(|error| {
                                Self::outbox_startup_error(format!(
                                    "sanitize legacy proposal observation outbox {} failed: {}",
                                    path.display(),
                                    error.message
                                ))
                            })?;
                    Self::validate_legacy_proposal_observation_batch(&record.batch).map_err(
                        |error| {
                            Self::outbox_startup_error(format!(
                                "invalid legacy proposal observation outbox {}: {}",
                                path.display(),
                                error.message
                            ))
                        },
                    )?;
                    legacy_rewrites.push(record.clone());
                }
                schema_version => {
                    return Err(Self::outbox_startup_error(format!(
                        "proposal observation outbox {} has unsupported schema {schema_version}",
                        path.display()
                    )));
                }
            }
            let file_stem = path.file_stem().and_then(|stem| stem.to_str());
            if file_stem != Some(record.batch.batch_id.as_str())
                || !loaded_batch_ids.insert(record.batch.batch_id.clone())
                || record
                    .batch
                    .events
                    .iter()
                    .any(|event| !loaded_event_ids.insert(event.event_id))
                || record
                    .batch
                    .proposal_audits
                    .iter()
                    .any(|audit| !loaded_proposal_ids.insert(audit.proposal_id))
            {
                return Err(Self::outbox_startup_error(format!(
                    "proposal observation outbox identity collision at {}",
                    path.display()
                )));
            }
            records.push(record);
        }
        // Canonicalize only audit-at-rest fields in legacy records. Event and
        // metadata identities remain unchanged because a Pending event may
        // already have been accepted by the sink under its EventId.
        for record in &legacy_rewrites {
            self.persist_proposal_observation_outbox(record)
                .map_err(|error| {
                    Self::outbox_startup_error(format!(
                        "rewrite legacy proposal observation outbox {} failed: {}",
                        record.batch.batch_id, error.message
                    ))
                })?;
        }
        let mut storage = self.storage.lock().map_err(Self::poisoned_error)?;
        let mut events = storage.protocol_proposal_observation_events.clone();
        let mut metadata = storage.protocol_event_metadata.clone();
        let mut audits = storage.protocol_proposal_audit.clone();
        let mut outbox = storage.protocol_proposal_observation_outbox.clone();
        for record in records {
            if let Some(existing) = outbox.get(&record.batch.batch_id) {
                if Self::serialized_records_equal(existing, &record)? {
                    continue;
                }
                return Err(Self::outbox_startup_error(format!(
                    "proposal observation batch {} conflicts with in-memory state",
                    record.batch.batch_id
                )));
            }
            if record.batch.events.iter().any(|event| {
                events.contains_key(&event.event_id) || metadata.contains_key(&event.event_id)
            }) || record
                .batch
                .proposal_audits
                .iter()
                .any(|audit| audits.contains_key(&audit.proposal_id))
            {
                return Err(Self::outbox_startup_error(format!(
                    "proposal observation batch {} reuses an existing event or proposal id",
                    record.batch.batch_id
                )));
            }
            for event in &record.batch.events {
                events.insert(event.event_id, event.clone());
            }
            for event_metadata in &record.batch.event_metadata {
                metadata.insert(event_metadata.event_id, event_metadata.clone());
            }
            for audit in &record.batch.proposal_audits {
                audits.insert(audit.proposal_id, audit.clone());
            }
            outbox.insert(record.batch.batch_id.clone(), record);
        }
        storage.protocol_proposal_observation_events = events;
        storage.protocol_event_metadata = metadata;
        storage.protocol_proposal_audit = audits;
        storage.protocol_proposal_observation_outbox = outbox;
        Ok(())
    }

    fn outbox_startup_error(message: String) -> ProtocolError {
        ProtocolError {
            code: "proposal_observation_outbox_corrupt".to_string(),
            message,
        }
    }

    fn ensure_proposal_observation_outbox_healthy(&self) -> ProtocolResult<()> {
        match self.proposal_observation_startup_error.as_ref() {
            Some(error) => Err(error.clone()),
            None => Ok(()),
        }
    }

    /// Return the fail-closed startup error recorded while loading the outbox.
    pub fn proposal_observation_startup_error(&self) -> Option<ProtocolError> {
        self.proposal_observation_startup_error.clone()
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

    /// Cause the next plan revision write to fail for fail-closed integration tests.
    pub fn fail_next_plan_revision_write(&self) {
        self.fail_next_plan_revision_write
            .store(true, Ordering::SeqCst);
    }

    /// Inject a deterministic zero-based item failure for the next proposal
    /// observation batch. Validation still runs, but no storage state mutates.
    pub fn fail_proposal_observation_batch_at_item_for_test(&self, item_index: usize) {
        self.fail_proposal_observation_batch_at_item
            .store(item_index.saturating_add(1), Ordering::SeqCst);
    }

    /// Cause the next Delivered-marker persistence to fail after sink acknowledgement.
    pub fn fail_next_proposal_observation_delivery_persist_for_test(&self) {
        self.fail_next_proposal_observation_delivery_persist
            .store(true, Ordering::SeqCst);
    }

    /// Validate and atomically commit a proposal-created observation as Pending.
    ///
    /// Event metadata, proposal audits, and the outbox record are inserted under
    /// one storage lock. With `base_dir` enabled, the complete record is first
    /// persisted as one atomically-renamed JSON document.
    pub fn store_proposal_observation_batch(
        &self,
        mut batch: ProposalObservationBatch,
    ) -> ProtocolResult<ProposalObservationOutboxRecord> {
        self.ensure_proposal_observation_outbox_healthy()?;
        batch.events = prepare_event_batch(
            batch
                .events
                .into_iter()
                .map(|envelope| EventSinkRequest { envelope })
                .collect(),
            EventSinkConfig::default(),
        )
        .map_err(|error| ProtocolError {
            code: "proposal_observation_batch_invalid".to_string(),
            message: format!("proposal observation event preparation failed: {error}"),
        })?;
        batch.proposal_audits = batch
            .proposal_audits
            .into_iter()
            .map(Self::sanitize_proposal_observation_audit)
            .collect::<ProtocolResult<Vec<_>>>()?;
        Self::validate_proposal_observation_batch(&batch)?;
        let item_count = batch
            .events
            .len()
            .saturating_add(batch.event_metadata.len())
            .saturating_add(batch.proposal_audits.len());
        let fail_item = self
            .fail_proposal_observation_batch_at_item
            .swap(0, Ordering::SeqCst);
        if fail_item != 0 && fail_item <= item_count {
            return Err(ProtocolError {
                code: "storage_failed".to_string(),
                message: format!(
                    "injected proposal observation batch failure at item {}",
                    fail_item - 1
                ),
            });
        }

        let mut storage = self.storage.lock().map_err(Self::poisoned_error)?;
        if let Some(existing) = storage
            .protocol_proposal_observation_outbox
            .get(&batch.batch_id)
        {
            if Self::proposal_observation_batches_equal(&existing.batch, &batch)? {
                return Ok(existing.clone());
            }
            return Err(ProtocolError {
                code: "proposal_observation_batch_conflict".to_string(),
                message: format!(
                    "proposal observation batch {} was retried with different content",
                    batch.batch_id
                ),
            });
        }
        for (event, metadata) in batch.events.iter().zip(&batch.event_metadata) {
            if let Some(existing) = storage
                .protocol_proposal_observation_events
                .get(&event.event_id)
            {
                let relation = if Self::serialized_records_equal(existing, event)? {
                    "identical"
                } else {
                    "different"
                };
                return Err(ProtocolError {
                    code: "proposal_observation_record_conflict".to_string(),
                    message: format!(
                        "event {:?} is reused by another batch with {relation} content",
                        event.event_id
                    ),
                });
            }
            if storage
                .protocol_event_metadata
                .contains_key(&metadata.event_id)
            {
                return Err(ProtocolError {
                    code: "proposal_observation_record_conflict".to_string(),
                    message: format!(
                        "event metadata {:?} is already owned by another record",
                        metadata.event_id
                    ),
                });
            }
        }
        for audit in &batch.proposal_audits {
            if storage
                .protocol_proposal_audit
                .contains_key(&audit.proposal_id)
            {
                return Err(ProtocolError {
                    code: "proposal_observation_record_conflict".to_string(),
                    message: format!(
                        "proposal audit {:?} is already owned by another batch",
                        audit.proposal_id
                    ),
                });
            }
        }

        let record = ProposalObservationOutboxRecord {
            batch,
            delivery_state: ProposalObservationDeliveryState::Pending,
        };
        self.persist_proposal_observation_outbox(&record)?;
        for event in &record.batch.events {
            storage
                .protocol_proposal_observation_events
                .insert(event.event_id, event.clone());
        }
        for metadata in &record.batch.event_metadata {
            storage
                .protocol_event_metadata
                .insert(metadata.event_id, metadata.clone());
        }
        for audit in &record.batch.proposal_audits {
            storage
                .protocol_proposal_audit
                .insert(audit.proposal_id, audit.clone());
        }
        storage
            .protocol_proposal_observation_outbox
            .insert(record.batch.batch_id.clone(), record.clone());
        Ok(record)
    }

    fn sanitize_proposal_observation_audit(
        audit: ProposalAuditRecord,
    ) -> ProtocolResult<ProposalAuditRecord> {
        let audit = Self::sanitize_proposal_audit(audit)?;
        if audit.lifecycle_state != ProposalLifecycleState::Created
            || audit.checkpoint_rollback_projection.is_some()
        {
            return Err(ProtocolError {
                code: "proposal_observation_batch_invalid".to_string(),
                message:
                    "Created proposal observation audits cannot contain checkpoint projections"
                        .to_string(),
            });
        }
        Ok(audit)
    }

    fn sanitize_proposal_audit(
        mut audit: ProposalAuditRecord,
    ) -> ProtocolResult<ProposalAuditRecord> {
        if !audit
            .redaction_hints
            .contains(&legion_protocol::RedactionHint::MetadataOnly)
        {
            return Err(ProtocolError {
                code: "proposal_observation_batch_invalid".to_string(),
                message: "proposal observation audit requires MetadataOnly redaction".to_string(),
            });
        }
        if audit
            .risk_rule_ids
            .iter()
            .any(|rule_id| !Self::is_safe_audit_identifier(rule_id))
        {
            return Err(ProtocolError {
                code: "proposal_observation_batch_invalid".to_string(),
                message: "proposal observation audit contains an unsafe risk rule id".to_string(),
            });
        }
        audit.payload_summary.title = audit
            .payload_summary
            .title
            .as_deref()
            .map(Self::metadata_only_storage_summary);
        for diagnostic in &mut audit.diagnostics {
            diagnostic.message = Self::metadata_only_storage_summary(&diagnostic.message);
            diagnostic.path = diagnostic
                .path
                .as_ref()
                .map(|path| CanonicalPath(Self::metadata_only_storage_summary(path.0.as_str())));
        }
        if let Some(projection) = &mut audit.checkpoint_rollback_projection {
            Self::sanitize_checkpoint_rollback_projection(projection)?;
        }
        audit.redaction_hints = vec![legion_protocol::RedactionHint::MetadataOnly];
        Ok(audit)
    }

    fn sanitize_checkpoint_rollback_projection(
        projection: &mut CheckpointRollbackProjection,
    ) -> ProtocolResult<()> {
        let declares_metadata_only = |hints: &[legion_protocol::RedactionHint]| {
            hints.contains(&legion_protocol::RedactionHint::MetadataOnly)
        };
        if !declares_metadata_only(&projection.redaction_hints)
            || !declares_metadata_only(&projection.checkpoint.redaction_hints)
            || !declares_metadata_only(&projection.rollback.redaction_hints)
            || projection
                .targets
                .iter()
                .any(|target| !declares_metadata_only(&target.redaction_hints))
            || projection
                .checkpoint
                .limitations
                .iter()
                .chain(&projection.rollback.limitations)
                .any(|limitation| !declares_metadata_only(&limitation.redaction_hints))
        {
            return Err(ProtocolError {
                code: "proposal_audit_invalid".to_string(),
                message: "checkpoint/rollback projection requires MetadataOnly at every level"
                    .to_string(),
            });
        }
        projection.checkpoint.labels = projection
            .checkpoint
            .labels
            .iter()
            .map(|label| Self::metadata_only_storage_summary(label))
            .collect();
        projection.rollback.labels = projection
            .rollback
            .labels
            .iter()
            .map(|label| Self::metadata_only_storage_summary(label))
            .collect();
        for target in &mut projection.targets {
            target.labels = target
                .labels
                .iter()
                .map(|label| Self::metadata_only_storage_summary(label))
                .collect();
            target.redaction_hints = vec![legion_protocol::RedactionHint::MetadataOnly];
        }
        for limitation in projection
            .checkpoint
            .limitations
            .iter_mut()
            .chain(projection.rollback.limitations.iter_mut())
        {
            limitation.label = Self::metadata_only_storage_summary(&limitation.label);
            limitation.redaction_hints = vec![legion_protocol::RedactionHint::MetadataOnly];
        }
        projection.checkpoint.redaction_hints = vec![legion_protocol::RedactionHint::MetadataOnly];
        projection.rollback.redaction_hints = vec![legion_protocol::RedactionHint::MetadataOnly];
        projection.redaction_hints = vec![legion_protocol::RedactionHint::MetadataOnly];
        Self::validate_checkpoint_rollback_projection_structure(projection)
    }

    fn validate_checkpoint_rollback_projection_structure(
        projection: &CheckpointRollbackProjection,
    ) -> ProtocolResult<()> {
        let metadata_only = vec![legion_protocol::RedactionHint::MetadataOnly];
        let preconditions = &projection.checkpoint.expected_preconditions;
        let mut target_ids = HashSet::new();
        let invalid_target = projection.targets.iter().any(|target| {
            !Self::is_safe_audit_identifier(&target.target_id)
                || !target_ids.insert(target.target_id.as_str())
                || target.schema_version == 0
                || target.redaction_hints != metadata_only
                || target
                    .labels
                    .iter()
                    .any(|label| !Self::is_storage_redaction_marker(label))
                || target
                    .terminal_session_id
                    .is_some_and(|session_id| session_id.0 == 0)
                || target.plugin_id.is_some_and(|plugin_id| plugin_id.0 == 0)
                || target.ranges.iter().any(|range| range.start > range.end)
                || target
                    .hashes
                    .iter()
                    .any(|fingerprint| !Self::is_safe_fingerprint(fingerprint))
        });
        let invalid_limitation = projection
            .checkpoint
            .limitations
            .iter()
            .chain(&projection.rollback.limitations)
            .any(|limitation| {
                !Self::is_safe_checkpoint_limitation(limitation)
                    || limitation
                        .target_id
                        .as_deref()
                        .is_some_and(|target_id| !target_ids.contains(target_id))
            });
        let target_count = projection.targets.len() as u64;
        let rollback_classified_count = u64::from(projection.rollback.reversible_target_count)
            .saturating_add(u64::from(projection.rollback.irreversible_target_count));
        if projection.schema_version == 0
            || projection.generated_at.0 == 0
            || !Self::is_safe_audit_identifier(&projection.projection_id)
            || projection.redaction_hints != metadata_only
            || projection.checkpoint.schema_version == 0
            || !Self::is_safe_audit_identifier(&projection.checkpoint.checkpoint_id)
            || projection.checkpoint.redaction_hints != metadata_only
            || projection.checkpoint.target_count as u64 != target_count
            || projection
                .checkpoint
                .labels
                .iter()
                .any(|label| !Self::is_storage_redaction_marker(label))
            || projection
                .checkpoint
                .hashes
                .iter()
                .any(|fingerprint| !Self::is_safe_fingerprint(fingerprint))
            || preconditions.schema_version == 0
            || preconditions
                .risk_reasons
                .iter()
                .any(|reason| !Self::is_safe_audit_identifier(reason))
            || preconditions
                .expected_fingerprint
                .as_ref()
                .is_some_and(|fingerprint| !Self::is_safe_fingerprint(fingerprint))
            || projection.rollback.schema_version == 0
            || projection.rollback.redaction_hints != metadata_only
            || rollback_classified_count != target_count
            || projection
                .rollback
                .labels
                .iter()
                .any(|label| !Self::is_storage_redaction_marker(label))
            || invalid_target
            || invalid_limitation
        {
            return Err(ProtocolError {
                code: "proposal_audit_invalid".to_string(),
                message: "checkpoint/rollback projection must be complete structural metadata only"
                    .to_string(),
            });
        }
        Ok(())
    }

    fn is_safe_checkpoint_limitation(limitation: &CheckpointRollbackLimitation) -> bool {
        limitation.schema_version != 0
            && Self::is_safe_audit_identifier(&limitation.reason_code)
            && Self::is_storage_redaction_marker(&limitation.label)
            && limitation
                .target_id
                .as_deref()
                .is_none_or(Self::is_safe_audit_identifier)
            && limitation.redaction_hints == vec![legion_protocol::RedactionHint::MetadataOnly]
    }

    fn is_safe_fingerprint(fingerprint: &FileFingerprint) -> bool {
        Self::is_safe_audit_identifier(&fingerprint.algorithm)
            && !fingerprint.value.is_empty()
            && fingerprint.value.len() <= 256
            && fingerprint.value.bytes().all(|byte| {
                byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b':' | b'=')
            })
    }

    fn validate_persisted_proposal_audit(audit: &ProposalAuditRecord) -> ProtocolResult<()> {
        InMemoryStorage::validate_audit_record(audit).map_err(InMemoryStorage::protocol_error)?;
        if audit.proposal_id.0 == 0
            || !Self::is_safe_audit_identifier(&audit.principal.0)
            || !Self::is_safe_audit_identifier(&audit.capability.0)
            || audit.redaction_hints != vec![legion_protocol::RedactionHint::MetadataOnly]
            || audit
                .payload_summary
                .title
                .as_deref()
                .is_some_and(|title| !Self::is_storage_redaction_marker(title))
            || audit.diagnostics.iter().any(|diagnostic| {
                !Self::is_safe_audit_identifier(&diagnostic.code)
                    || !Self::is_storage_redaction_marker(&diagnostic.message)
                    || diagnostic
                        .path
                        .as_ref()
                        .is_some_and(|path| !Self::is_storage_redaction_marker(path.0.as_str()))
            })
            || audit
                .risk_rule_ids
                .iter()
                .any(|rule_id| !Self::is_safe_audit_identifier(rule_id))
        {
            return Err(ProtocolError {
                code: "proposal_audit_invalid".to_string(),
                message: "proposal audit must contain only canonical metadata-only fields"
                    .to_string(),
            });
        }
        if let Some(projection) = &audit.checkpoint_rollback_projection {
            Self::validate_checkpoint_rollback_projection_structure(projection)?;
            if projection.proposal_id != audit.proposal_id
                || projection.payload_kind != audit.payload_summary.kind
                || projection.lifecycle_state != audit.lifecycle_state
                || projection.correlation_id != audit.correlation_id
                || projection.causality_id != Some(audit.causality_id)
            {
                return Err(ProtocolError {
                    code: "proposal_audit_invalid".to_string(),
                    message: "checkpoint/rollback projection identity must match its audit"
                        .to_string(),
                });
            }
        }
        Ok(())
    }

    fn proposal_audit_identity_matches(
        created: &ProposalAuditRecord,
        candidate: &ProposalAuditRecord,
    ) -> bool {
        created.proposal_id == candidate.proposal_id
            && created.principal == candidate.principal
            && created.capability == candidate.capability
            && created.correlation_id == candidate.correlation_id
            && created.causality_id == candidate.causality_id
            && created.payload_summary.kind == candidate.payload_summary.kind
            && created.payload_summary.affected_files == candidate.payload_summary.affected_files
            && created.payload_summary.title == candidate.payload_summary.title
            && created.payload_summary.byte_count == candidate.payload_summary.byte_count
    }

    fn metadata_only_storage_summary(value: &str) -> String {
        if Self::is_storage_redaction_marker(value) {
            return value.to_string();
        }
        format!(
            "hash={};len={}",
            storage_checksum(value.as_bytes()),
            value.len()
        )
    }

    fn is_storage_redaction_marker(value: &str) -> bool {
        if matches!(value, "<metadata-only>" | "<redacted>") {
            return true;
        }
        let Some((digest, length)) = value
            .strip_prefix("hash=")
            .and_then(|value| value.split_once(";len="))
        else {
            return false;
        };
        digest.len() == 64
            && digest.bytes().all(|byte| byte.is_ascii_hexdigit())
            && digest.bytes().all(|byte| !byte.is_ascii_uppercase())
            && !length.is_empty()
            && length.bytes().all(|byte| byte.is_ascii_digit())
            && length.parse::<u64>().is_ok()
    }

    fn is_safe_audit_identifier(value: &str) -> bool {
        !value.is_empty()
            && value.len() <= 128
            && value.bytes().all(|byte| {
                byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b':')
            })
    }

    /// Return every proposal observation outbox record in stable batch-id order.
    ///
    /// This read-only view includes both Pending and Delivered records so an
    /// application can reserve durable identities and reconcile its live
    /// proposal ledger after a restart without re-persisting proposal content.
    pub fn proposal_observation_batches(
        &self,
    ) -> ProtocolResult<Vec<ProposalObservationOutboxRecord>> {
        self.ensure_proposal_observation_outbox_healthy()?;
        let storage = self.storage.lock().map_err(Self::poisoned_error)?;
        let mut records = storage
            .protocol_proposal_observation_outbox
            .values()
            .cloned()
            .collect::<Vec<_>>();
        records.sort_by(|left, right| left.batch.batch_id.cmp(&right.batch.batch_id));
        Ok(records)
    }

    /// Compare a replay candidate with its stored outbox record after applying
    /// the same event preparation and audit sanitization as the write path.
    ///
    /// This never mutates storage or delivers events. It lets an application
    /// prove that trusted replay inputs reproduce the exact durable
    /// EventIds, timestamps, metadata, and redacted audits before publishing
    /// recovered live proposal state.
    pub fn proposal_observation_batch_matches_stored(
        &self,
        mut candidate: ProposalObservationBatch,
    ) -> ProtocolResult<bool> {
        self.ensure_proposal_observation_outbox_healthy()?;
        candidate.events = prepare_event_batch(
            candidate
                .events
                .into_iter()
                .map(|envelope| EventSinkRequest { envelope })
                .collect(),
            EventSinkConfig::default(),
        )
        .map_err(|error| ProtocolError {
            code: "proposal_observation_batch_invalid".to_string(),
            message: format!("proposal observation replay event preparation failed: {error}"),
        })?;
        candidate.proposal_audits = candidate
            .proposal_audits
            .into_iter()
            .map(Self::sanitize_proposal_observation_audit)
            .collect::<ProtocolResult<Vec<_>>>()?;
        Self::validate_proposal_observation_batch(&candidate)?;

        let storage = self.storage.lock().map_err(Self::poisoned_error)?;
        let Some(stored) = storage
            .protocol_proposal_observation_outbox
            .get(&candidate.batch_id)
        else {
            return Ok(false);
        };
        Self::proposal_observation_batches_equal(&stored.batch, &candidate)
    }

    /// Return all proposal observation batches still awaiting sink delivery.
    pub fn pending_proposal_observation_batches(
        &self,
    ) -> ProtocolResult<Vec<ProposalObservationOutboxRecord>> {
        Ok(self
            .proposal_observation_batches()?
            .into_iter()
            .filter(|record| record.delivery_state == ProposalObservationDeliveryState::Pending)
            .collect())
    }

    /// Mark a previously committed proposal observation batch as delivered.
    fn mark_proposal_observation_batch_delivered(
        &self,
        batch_id: &str,
    ) -> ProtocolResult<ProposalObservationOutboxRecord> {
        self.ensure_proposal_observation_outbox_healthy()?;
        let mut storage = self.storage.lock().map_err(Self::poisoned_error)?;
        let existing = storage
            .protocol_proposal_observation_outbox
            .get(batch_id)
            .cloned()
            .ok_or_else(|| ProtocolError {
                code: "proposal_observation_batch_missing".to_string(),
                message: format!("proposal observation batch {batch_id} is not stored"),
            })?;
        if existing.delivery_state == ProposalObservationDeliveryState::Delivered {
            return Ok(existing);
        }
        let delivered = ProposalObservationOutboxRecord {
            delivery_state: ProposalObservationDeliveryState::Delivered,
            ..existing
        };
        if self
            .fail_next_proposal_observation_delivery_persist
            .swap(false, Ordering::SeqCst)
        {
            return Err(ProtocolError {
                code: "storage_failed".to_string(),
                message: "injected Delivered-marker persistence failure".to_string(),
            });
        }
        self.persist_proposal_observation_outbox(&delivered)?;
        storage
            .protocol_proposal_observation_outbox
            .insert(batch_id.to_string(), delivered.clone());
        Ok(delivered)
    }

    /// Deliver one Pending outbox batch through the sink's atomic batch API.
    pub fn deliver_proposal_observation_batch(
        &self,
        batch_id: &str,
    ) -> ProtocolResult<ProposalObservationOutboxRecord> {
        self.ensure_proposal_observation_outbox_healthy()?;
        let _delivery = self
            .proposal_observation_delivery
            .lock()
            .map_err(|_| ProtocolError {
                code: "storage_lock_poisoned".to_string(),
                message: "proposal observation delivery lock poisoned".to_string(),
            })?;
        let record = {
            let storage = self.storage.lock().map_err(Self::poisoned_error)?;
            storage
                .protocol_proposal_observation_outbox
                .get(batch_id)
                .cloned()
                .ok_or_else(|| ProtocolError {
                    code: "proposal_observation_batch_missing".to_string(),
                    message: format!("proposal observation batch {batch_id} is not stored"),
                })?
        };
        if record.delivery_state == ProposalObservationDeliveryState::Delivered {
            return Ok(record);
        }
        self.event_sink
            .emit_batch(
                record
                    .batch
                    .events
                    .iter()
                    .cloned()
                    .map(|envelope| EventSinkRequest { envelope })
                    .collect(),
            )
            .map_err(|error| ProtocolError {
                code: error.code,
                message: format!(
                    "proposal observation batch {batch_id} remains pending: {}",
                    error.message
                ),
            })?;
        self.mark_proposal_observation_batch_delivered(batch_id)
    }

    /// Retry every Pending proposal observation batch in stable batch-id order.
    ///
    /// Each record remains Pending unless its complete event batch is accepted
    /// by the sink and the Delivered marker is durably persisted.
    pub fn retry_pending_proposal_observations(
        &self,
    ) -> ProtocolResult<ProposalObservationRetryReport> {
        let batch_ids = self
            .pending_proposal_observation_batches()?
            .into_iter()
            .map(|record| record.batch.batch_id)
            .collect::<Vec<_>>();
        let mut attempts = Vec::with_capacity(batch_ids.len());
        let mut delivered_count = 0usize;
        let mut pending_count = 0usize;
        for batch_id in batch_ids {
            match self.deliver_proposal_observation_batch(&batch_id) {
                Ok(record) => {
                    delivered_count = delivered_count.saturating_add(1);
                    attempts.push(ProposalObservationRetryAttempt {
                        batch_id,
                        delivery_state: record.delivery_state,
                        error_code: None,
                        error_kind: None,
                        schema_version: 1,
                    });
                }
                Err(error) => {
                    pending_count = pending_count.saturating_add(1);
                    attempts.push(ProposalObservationRetryAttempt {
                        batch_id,
                        delivery_state: ProposalObservationDeliveryState::Pending,
                        error_kind: Some(Self::proposal_observation_retry_error_kind(&error.code)),
                        error_code: Some(error.code),
                        schema_version: 1,
                    });
                }
            }
        }
        Ok(ProposalObservationRetryReport {
            attempts,
            delivered_count,
            pending_count,
            schema_version: 1,
        })
    }

    fn proposal_observation_retry_error_kind(code: &str) -> ProposalObservationRetryErrorKind {
        if matches!(
            code,
            "event_batch_unsupported"
                | "event_id_conflict"
                | "event_batch_invalid"
                | "proposal_observation_batch_invalid"
                | "proposal_observation_batch_conflict"
                | "proposal_observation_record_conflict"
                | "proposal_observation_outbox_corrupt"
                | "storage_lock_poisoned"
        ) {
            ProposalObservationRetryErrorKind::Permanent
        } else {
            ProposalObservationRetryErrorKind::Transient
        }
    }

    fn proposal_observation_batches_equal(
        left: &ProposalObservationBatch,
        right: &ProposalObservationBatch,
    ) -> ProtocolResult<bool> {
        let left = serde_json::to_vec(left).map_err(|error| ProtocolError {
            code: "storage_failed".to_string(),
            message: format!("serialize stored proposal observation batch failed: {error}"),
        })?;
        let right = serde_json::to_vec(right).map_err(|error| ProtocolError {
            code: "storage_failed".to_string(),
            message: format!("serialize retried proposal observation batch failed: {error}"),
        })?;
        Ok(left == right)
    }

    fn serialized_records_equal<T: Serialize>(left: &T, right: &T) -> ProtocolResult<bool> {
        let left = serde_json::to_vec(left).map_err(|error| ProtocolError {
            code: "storage_failed".to_string(),
            message: format!("serialize stored proposal observation record failed: {error}"),
        })?;
        let right = serde_json::to_vec(right).map_err(|error| ProtocolError {
            code: "storage_failed".to_string(),
            message: format!("serialize retried proposal observation record failed: {error}"),
        })?;
        Ok(left == right)
    }

    fn validate_legacy_proposal_observation_batch(
        batch: &ProposalObservationBatch,
    ) -> ProtocolResult<()> {
        if batch.schema_version != LEGACY_PROPOSAL_OBSERVATION_BATCH_SCHEMA_VERSION
            || !Self::is_safe_proposal_observation_batch_id(&batch.batch_id)
            || batch.events.is_empty()
            || batch.events.len() != batch.event_metadata.len()
            || batch.events.len() != batch.proposal_audits.len()
        {
            return Err(ProtocolError {
                code: "proposal_observation_batch_invalid".to_string(),
                message: "legacy proposal observation batch shape is invalid".to_string(),
            });
        }

        let mut event_ids = HashSet::new();
        let mut proposal_ids = HashSet::new();
        for ((event, metadata), audit) in batch
            .events
            .iter()
            .zip(&batch.event_metadata)
            .zip(&batch.proposal_audits)
        {
            validate_envelope(event, EventSinkConfig::default()).map_err(|error| {
                ProtocolError {
                    code: "proposal_observation_batch_invalid".to_string(),
                    message: format!("legacy proposal observation event is invalid: {error}"),
                }
            })?;
            let prepared = prepare_event_batch(
                vec![EventSinkRequest {
                    envelope: event.clone(),
                }],
                EventSinkConfig::default(),
            )
            .map_err(|error| ProtocolError {
                code: "proposal_observation_batch_invalid".to_string(),
                message: format!("legacy proposal event preparation failed: {error}"),
            })?;
            if !Self::serialized_records_equal(event, &prepared[0])?
                || event.event_id.0.is_nil()
                || event.event != "proposal.created"
                || event.severity != legion_protocol::EventSeverity::Info
                || event.retention != legion_protocol::RetentionLabel::Audit
                || event.redaction != legion_protocol::RedactionHint::MetadataOnly
                || audit.proposal_id.0 == 0
                || audit.lifecycle_state != ProposalLifecycleState::Created
                || audit.checkpoint_rollback_projection.is_some()
                || event.payload["proposal_id"].as_u64() != Some(audit.proposal_id.0)
                || event.payload["lifecycle_state"].as_str() != Some("Created")
                || event.payload["capability"].as_str() != Some(audit.capability.0.as_str())
                || event.principal_id.as_ref() != Some(&audit.principal)
                || event.correlation_id != audit.correlation_id
                || event.causality_id != audit.causality_id
                || !event_ids.insert(event.event_id)
                || !proposal_ids.insert(audit.proposal_id)
            {
                return Err(ProtocolError {
                    code: "proposal_observation_batch_invalid".to_string(),
                    message: "legacy proposal observation identities are invalid".to_string(),
                });
            }
            Self::validate_persisted_proposal_audit(audit)?;
            InMemoryStorage::validate_event_metadata(metadata)
                .map_err(InMemoryStorage::protocol_error)?;
            if !Self::serialized_records_equal(&event_metadata_record(event), metadata)? {
                return Err(ProtocolError {
                    code: "proposal_observation_batch_invalid".to_string(),
                    message: "legacy proposal event metadata does not match".to_string(),
                });
            }
        }
        Ok(())
    }

    fn validate_proposal_observation_batch(batch: &ProposalObservationBatch) -> ProtocolResult<()> {
        if !Self::is_safe_proposal_observation_batch_id(&batch.batch_id)
            || batch.schema_version != PROPOSAL_OBSERVATION_BATCH_SCHEMA_VERSION
        {
            return Err(ProtocolError {
                code: "proposal_observation_batch_invalid".to_string(),
                message: format!(
                    "proposal observation batch requires a safe id and schema {}",
                    PROPOSAL_OBSERVATION_BATCH_SCHEMA_VERSION
                ),
            });
        }
        let item_count = batch.events.len();
        if item_count == 0
            || batch.event_metadata.len() != item_count
            || batch.proposal_audits.len() != item_count
        {
            return Err(ProtocolError {
                code: "proposal_observation_batch_invalid".to_string(),
                message: "proposal observation batch requires equal non-empty event, metadata, and audit lists"
                    .to_string(),
            });
        }

        let mut event_ids = HashSet::new();
        let mut proposal_ids = HashSet::new();
        for ((event, metadata), audit) in batch
            .events
            .iter()
            .zip(&batch.event_metadata)
            .zip(&batch.proposal_audits)
        {
            validate_envelope(event, EventSinkConfig::default()).map_err(|error| {
                ProtocolError {
                    code: "proposal_observation_batch_invalid".to_string(),
                    message: format!("proposal observation event is invalid: {error}"),
                }
            })?;
            let prepared = prepare_event_batch(
                vec![EventSinkRequest {
                    envelope: event.clone(),
                }],
                EventSinkConfig::default(),
            )
            .map_err(|error| ProtocolError {
                code: "proposal_observation_batch_invalid".to_string(),
                message: format!("proposal observation event preparation failed: {error}"),
            })?;
            let expected_payload_kind = format!("{:?}", audit.payload_summary.kind);
            let byte_count_matches = match (
                event.payload.get("payload_byte_count"),
                audit.payload_summary.byte_count,
            ) {
                (None, None) => true,
                (Some(serde_json::Value::Number(value)), Some(expected)) => {
                    value.as_u64() == Some(expected)
                }
                _ => false,
            };
            let title_matches = match (
                event.payload.get("title"),
                audit.payload_summary.title.as_deref(),
            ) {
                (None, None) => true,
                (Some(serde_json::Value::String(event_title)), Some(audit_title)) => {
                    event_title == audit_title && Self::is_storage_redaction_marker(event_title)
                }
                _ => false,
            };
            if !Self::serialized_records_equal(event, &prepared[0])?
                || event.event_id.0.is_nil()
                || event.event != "proposal.created"
                || event.severity != legion_protocol::EventSeverity::Info
                || event.retention != legion_protocol::RetentionLabel::Audit
                || event.redaction != legion_protocol::RedactionHint::MetadataOnly
                || audit.proposal_id.0 == 0
                || audit.lifecycle_state != ProposalLifecycleState::Created
                || audit.principal.0.trim().is_empty()
                || audit.capability.0.trim().is_empty()
                || !Self::is_safe_audit_identifier(&audit.principal.0)
                || !Self::is_safe_audit_identifier(&audit.capability.0)
                || audit.checkpoint_rollback_projection.is_some()
                || audit.redaction_hints != vec![legion_protocol::RedactionHint::MetadataOnly]
                || audit
                    .payload_summary
                    .title
                    .as_deref()
                    .is_some_and(|title| !Self::is_storage_redaction_marker(title))
                || audit.diagnostics.iter().any(|diagnostic| {
                    !Self::is_safe_audit_identifier(&diagnostic.code)
                        || !Self::is_storage_redaction_marker(&diagnostic.message)
                        || diagnostic
                            .path
                            .as_ref()
                            .is_some_and(|path| !Self::is_storage_redaction_marker(path.0.as_str()))
                })
                || audit
                    .risk_rule_ids
                    .iter()
                    .any(|rule_id| !Self::is_safe_audit_identifier(rule_id))
                || event.payload["proposal_id"].as_u64() != Some(audit.proposal_id.0)
                || event.payload["lifecycle_state"].as_str() != Some("Created")
                || event.payload["capability"].as_str() != Some(audit.capability.0.as_str())
                || event.payload["payload_kind"].as_str() != Some(expected_payload_kind.as_str())
                || event.payload["affected_file_count"].as_u64()
                    != Some(audit.payload_summary.affected_files.len() as u64)
                || !byte_count_matches
                || !title_matches
                || event.principal_id.as_ref() != Some(&audit.principal)
                || event.occurred_at != audit.timestamp
                || !event_ids.insert(event.event_id)
                || !proposal_ids.insert(audit.proposal_id)
            {
                return Err(ProtocolError {
                    code: "proposal_observation_batch_invalid".to_string(),
                    message: "proposal observation batch requires unique metadata-only Created events and audits"
                        .to_string(),
                });
            }
            InMemoryStorage::validate_event_metadata(metadata)
                .map_err(InMemoryStorage::protocol_error)?;
            InMemoryStorage::validate_audit_record(audit)
                .map_err(InMemoryStorage::protocol_error)?;
            let derived = event_metadata_record(event);
            if !Self::serialized_records_equal(&derived, metadata)?
                || event.correlation_id != audit.correlation_id
                || event.causality_id != audit.causality_id
            {
                return Err(ProtocolError {
                    code: "proposal_observation_batch_invalid".to_string(),
                    message:
                        "proposal observation event, metadata, and audit identities do not match"
                            .to_string(),
                });
            }
        }
        Ok(())
    }

    fn is_safe_proposal_observation_batch_id(batch_id: &str) -> bool {
        if batch_id.is_empty()
            || batch_id.len() > 120
            || batch_id == "."
            || batch_id == ".."
            || batch_id.starts_with('.')
            || batch_id.ends_with('.')
            || !batch_id.chars().all(|character| {
                character.is_ascii_lowercase()
                    || character.is_ascii_digit()
                    || matches!(character, '-' | '_' | '.')
            })
        {
            return false;
        }
        let stem = batch_id.split('.').next().unwrap_or(batch_id);
        !matches!(stem, "con" | "prn" | "aux" | "nul")
            && !(stem.len() == 4
                && (stem.starts_with("com") || stem.starts_with("lpt"))
                && matches!(stem.as_bytes()[3], b'1'..=b'9'))
    }

    /// Persist redacted event metadata and emit through the injected sink.
    pub fn record_event(
        &self,
        envelope: EventEnvelope,
    ) -> ProtocolResult<StorageRepositoryResponse> {
        let metadata = event_metadata_record(&envelope);
        // Persist and validate the audit metadata first so we never emit an event that lacks a
        // durable audit record. If the store fails, propagate the error and emit nothing.
        let stored = self.handle(StorageRepositoryRequest::SaveEventMetadata(metadata))?;
        // The record is now persisted. If emit fails, surface an explicit partial-failure error
        // rather than discarding the persisted record or silently succeeding.
        self.event_sink
            .emit(EventSinkRequest { envelope })
            .map_err(|err| ProtocolError {
                code: "storage_event_emit_failed_after_store".to_string(),
                message: format!(
                    "event metadata persisted but sink emit failed: {}",
                    err.message
                ),
            })?;
        Ok(stored)
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

    /// Persist one metadata-only plan revision in the wrapped store.
    pub fn record_plan_revision(
        &self,
        revision: EditablePlanRevisionArtifact,
    ) -> ProtocolResult<()> {
        if self
            .fail_next_plan_revision_write
            .swap(false, Ordering::SeqCst)
        {
            return Err(ProtocolError {
                code: "storage_failed".to_string(),
                message: "injected plan revision write failure".to_string(),
            });
        }
        let mut storage = self.storage.lock().map_err(Self::poisoned_error)?;
        storage
            .record_plan_revision(revision)
            .map_err(InMemoryStorage::protocol_error)
    }

    /// Read all plan revisions for a plan from the wrapped store.
    pub fn plan_revisions(
        &self,
        plan_artifact_id: &str,
    ) -> ProtocolResult<Vec<EditablePlanRevisionArtifact>> {
        let storage = self.storage.lock().map_err(Self::poisoned_error)?;
        Ok(storage.plan_revisions(plan_artifact_id))
    }

    /// Read the latest plan revision for a plan from the wrapped store.
    pub fn latest_plan_revision(
        &self,
        plan_artifact_id: &str,
    ) -> ProtocolResult<Option<EditablePlanRevisionArtifact>> {
        let storage = self.storage.lock().map_err(Self::poisoned_error)?;
        Ok(storage.latest_plan_revision(plan_artifact_id))
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
        self.ensure_proposal_observation_outbox_healthy()?;
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

        match request {
            StorageRepositoryRequest::SaveProposalAuditRecord(record) => {
                let record = Self::sanitize_proposal_audit(record)?;
                Self::validate_persisted_proposal_audit(&record)?;
                let mut storage = self.storage.lock().map_err(Self::poisoned_error)?;
                if let Some(created) = storage
                    .protocol_proposal_observation_outbox
                    .values()
                    .flat_map(|outbox| &outbox.batch.proposal_audits)
                    .find(|created| created.proposal_id == record.proposal_id)
                    && !Self::proposal_audit_identity_matches(created, &record)
                {
                    return Err(ProtocolError {
                        code: "proposal_observation_record_conflict".to_string(),
                        message: format!(
                            "proposal audit {:?} conflicts with its outbox-owned identity",
                            record.proposal_id
                        ),
                    });
                }
                // Validate → durable write → memory while holding the repository
                // lock, so concurrent lifecycle writes cannot race the identity check.
                self.persist_proposal_audit(&record)?;
                storage
                    .handle_protocol_request(StorageRepositoryRequest::SaveProposalAuditRecord(
                        record,
                    ))
                    .map_err(InMemoryStorage::protocol_error)
            }
            StorageRepositoryRequest::SaveEventMetadata(metadata) => {
                InMemoryStorage::validate_event_metadata(&metadata)
                    .map_err(InMemoryStorage::protocol_error)?;
                let mut storage = self.storage.lock().map_err(Self::poisoned_error)?;
                if let Some(event) = storage
                    .protocol_proposal_observation_events
                    .get(&metadata.event_id)
                    && !Self::serialized_records_equal(&event_metadata_record(event), &metadata)?
                {
                    return Err(ProtocolError {
                        code: "proposal_observation_record_conflict".to_string(),
                        message: format!(
                            "event metadata {:?} conflicts with its outbox-owned event",
                            metadata.event_id
                        ),
                    });
                }
                storage
                    .handle_protocol_request(StorageRepositoryRequest::SaveEventMetadata(metadata))
                    .map_err(InMemoryStorage::protocol_error)
            }
            request => {
                let mut storage = self.storage.lock().map_err(Self::poisoned_error)?;
                storage
                    .handle_protocol_request(request)
                    .map_err(InMemoryStorage::protocol_error)
            }
        }
    }
}

fn atomic_write_bytes(dest: &Path, body: &[u8]) -> Result<(), String> {
    let parent = dest.parent().unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent).map_err(|err| format!("create parent failed: {err}"))?;
    let suffix = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let temp = parent.join(format!(
        ".proposal-audit-tmp-{}-{}.tmp",
        std::process::id(),
        suffix
    ));
    let backup = parent.join(format!(
        ".proposal-audit-bak-{}-{}.tmp",
        std::process::id(),
        suffix
    ));
    let write_result = (|| -> Result<(), String> {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp)
            .map_err(|err| format!("create temp failed: {err}"))?;
        file.write_all(body)
            .map_err(|err| format!("write temp failed: {err}"))?;
        file.flush()
            .map_err(|err| format!("flush temp failed: {err}"))?;
        drop(file);
        // Portable replace that keeps the prior durable blob until commit:
        // 1) move dest → backup (Windows-safe; no overwrite)
        // 2) move temp → dest
        // 3) remove backup
        // On any failure after step 1, restore backup → dest.
        let had_existing = dest.exists();
        if had_existing {
            let _ = fs::remove_file(&backup);
            fs::rename(dest, &backup).map_err(|err| format!("backup dest failed: {err}"))?;
        }
        match fs::rename(&temp, dest) {
            Ok(()) => {
                if had_existing {
                    let _ = fs::remove_file(&backup);
                }
                Ok(())
            }
            Err(err) => {
                if had_existing {
                    let _ = fs::rename(&backup, dest);
                }
                Err(format!("rename temp failed: {err}"))
            }
        }
    })();
    if write_result.is_err() {
        let _ = fs::remove_file(&temp);
        // Leave backup in place only if dest is missing (restore may have failed).
        if dest.exists() {
            let _ = fs::remove_file(&backup);
        }
    }
    write_result
}

impl TryFrom<PersistedState> for InMemoryStorage {
    type Error = StorageError;

    fn try_from(value: PersistedState) -> StorageResult<Self> {
        let protocol_plan_revision_ledger =
            PlanRevisionLedger::from_revisions(value.protocol_plan_revisions)?;
        Ok(Self {
            workspace_configs: value.workspace_configs,
            trust: value.trust,
            metadata: value.metadata,
            sessions: value.sessions,
            dock_layouts: value.dock_layouts,
            protocol_workspace_configs: value.protocol_workspace_configs,
            protocol_file_metadata: value.protocol_file_metadata,
            protocol_sessions: value.protocol_sessions,
            protocol_trust: value
                .protocol_trust
                .into_iter()
                .map(|record| {
                    let key = (record.workspace_id, record.principal_id.clone());
                    (key, record)
                })
                .collect(),
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
            protocol_proposal_observation_events: value.protocol_proposal_observation_events,
            protocol_proposal_observation_outbox: value.protocol_proposal_observation_outbox,
            protocol_plan_revision_ledger,
            protocol_semantic_metadata: value.semantic_metadata,
            protocol_semantic_tombstones: value.semantic_tombstones,
            protocol_plugin_storage: value.plugin_storage,
        })
    }
}

impl PlanRevisionRepository for InMemoryStorage {
    fn record_plan_revision(
        &mut self,
        revision: EditablePlanRevisionArtifact,
    ) -> StorageResult<()> {
        self.protocol_plan_revision_ledger.record_revision(revision)
    }

    fn plan_revisions(&self, plan_artifact_id: &str) -> Vec<EditablePlanRevisionArtifact> {
        self.protocol_plan_revision_ledger
            .revisions(plan_artifact_id)
    }

    fn latest_plan_revision(&self, plan_artifact_id: &str) -> Option<EditablePlanRevisionArtifact> {
        self.protocol_plan_revision_ledger
            .latest_revision(plan_artifact_id)
    }
}

impl PlanRevisionRepository for FileBackedStorage {
    fn record_plan_revision(
        &mut self,
        revision: EditablePlanRevisionArtifact,
    ) -> StorageResult<()> {
        self.state.record_plan_revision(revision)?;
        self.flush()
    }

    fn plan_revisions(&self, plan_artifact_id: &str) -> Vec<EditablePlanRevisionArtifact> {
        self.state.plan_revisions(plan_artifact_id)
    }

    fn latest_plan_revision(&self, plan_artifact_id: &str) -> Option<EditablePlanRevisionArtifact> {
        self.state.latest_plan_revision(plan_artifact_id)
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
            Ok(contents) => match serde_json::from_str::<PersistedState>(&contents) {
                Ok(persisted) => match InMemoryStorage::try_from(persisted) {
                    Ok(storage) => storage,
                    Err(_) => return Err(Self::quarantine_corrupt(&path)),
                },
                Err(_) => return Err(Self::quarantine_corrupt(&path)),
            },
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

    fn quarantine_corrupt(path: &Path) -> StorageError {
        let quarantine = Self::quarantine_path(path);
        // Only report the file as quarantined if the move actually succeeded. A failed
        // rename must not leave the corrupt primary in place while claiming otherwise.
        match fs::rename(path, &quarantine) {
            Ok(()) => StorageError::Corrupt {
                path: path.to_string_lossy().into_owned(),
                quarantine_path: quarantine.to_string_lossy().into_owned(),
            },
            Err(rename_err) => StorageError::Failed {
                message: format!(
                    "storage corruption detected at `{}` but quarantine to `{}` failed: {rename_err}",
                    path.display(),
                    quarantine.display()
                ),
            },
        }
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
                let file_id = file_id.unwrap_or(FileId(legion_protocol_stable_hash(
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
                // Durability is handled by the port wrapper after this method returns
                // when `base_dir` is configured (see `InMemoryStorageRepositoryPort::handle`).
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
                Ok(StorageRepositoryResponse::SessionRecord(Box::new(
                    self.protocol_sessions.get(&session_id).cloned(),
                )))
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
        request: legion_protocol::PluginStorageRequest,
    ) -> StorageResult<StorageRepositoryResponse> {
        if request.plugin_id.0 == 0 || request.namespace.plugin_id != request.plugin_id {
            return Ok(StorageRepositoryResponse::PluginStorage(
                legion_protocol::PluginStorageResponse::Denied {
                    reason: PluginDenialReason::InvalidMetadata,
                    message: "plugin storage namespace escape denied".to_string(),
                },
            ));
        }

        match request.operation {
            PluginStorageOperation::Put => {
                let Some(record) = request.record else {
                    return Ok(StorageRepositoryResponse::PluginStorage(
                        legion_protocol::PluginStorageResponse::Denied {
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
                        legion_protocol::PluginStorageResponse::Denied {
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
                    legion_protocol::PluginStorageResponse::Stored {
                        key: record_key,
                        used_bytes: projected,
                    },
                ))
            }
            PluginStorageOperation::Get => {
                let Some(key) = request.key else {
                    return Ok(StorageRepositoryResponse::PluginStorage(
                        legion_protocol::PluginStorageResponse::Record(None),
                    ));
                };
                let storage_key = Self::plugin_storage_key(
                    request.workspace_id,
                    request.plugin_id,
                    &request.namespace.namespace,
                    &key,
                );
                Ok(StorageRepositoryResponse::PluginStorage(
                    legion_protocol::PluginStorageResponse::Record(
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
                    legion_protocol::PluginStorageResponse::QuotaUsage {
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
                    legion_protocol::PluginStorageResponse::Keys(keys),
                ))
            }
            PluginStorageOperation::QuotaUsage => Ok(StorageRepositoryResponse::PluginStorage(
                legion_protocol::PluginStorageResponse::QuotaUsage {
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
        plugin_id: legion_protocol::PluginId,
        namespace: &str,
        key: &str,
    ) -> String {
        format!("{}:{}:{namespace}:{key}", workspace_id.0, plugin_id.0)
    }

    fn plugin_storage_used_bytes(
        &self,
        workspace_id: WorkspaceId,
        plugin_id: legion_protocol::PluginId,
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
    breakpoint_id: &legion_protocol::DebugBreakpointId,
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

fn legion_protocol_stable_hash(value: &str) -> u128 {
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
                    observed_at: legion_protocol::TimestampMillis::now(),
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
    use std::sync::Arc;

    use legion_observability::{EventEnvelopeBuilder, InMemoryEventSink};
    use legion_protocol::{
        AgentReplayManifest, AgentRunId, AgentStateTransitionRecord,
        AssistedAiAuditOutcomeCategory, AssistedAiAuditPrivacyDisposition,
        AssistedAiAuditRedactionState, AssistedAiProviderInvocationState, ByteRange, CapabilityId,
        DebugBreakpointId, DebugBreakpointRecord, EditBatch, EventId, FileContentVersion,
        FileFingerprint, LanguageId, LineIndexRange, PermissionBudgetEvaluationDisposition,
        Phase4RuntimeAuditRecord, PreviewSummary, ProposalLifecycleState, ProposalPayload,
        ProposalPayloadKind, ProposalPayloadSummary, ProposalPrivacyLabel, ProposalRiskLabel,
        ProposalVersionPreconditions, ProtocolDiagnostic, ProtocolDiagnosticSeverity,
        ProtocolTextRange, RedactionHint, RetentionLabel, SemanticFileFingerprintIdentity,
        SemanticFreshnessState, SemanticGrammarVersion, SemanticMetadataChunkReference,
        SemanticMetadataDescriptorIdentity, SemanticMetadataDiagnosticSummary,
        SemanticMetadataFreshnessKey, SemanticMetadataSourceKind, SemanticMetadataSymbolRecord,
        SemanticModelVersion, SemanticRecordId, SemanticRecordProvenance, SemanticRecordSource,
        SemanticSymbolId, SnapshotId, TextCoordinate, TextEditProposal, WorkspaceGeneration,
        WorkspaceProposal, checkpoint_rollback_projection_from_proposal,
    };
    use serde_json::json;

    fn temp_storage_path(tag: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "legion-storage-{tag}-{}-{}.json",
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
            capability_decision: legion_protocol::CapabilityDecision {
                decision_id: legion_protocol::CapabilityDecisionId(99),
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
            timestamp: legion_protocol::TimestampMillis(1),
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
            checkpoint_rollback_projection: None,
            risk_rule_ids: Vec::new(),
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
            occurred_at: legion_protocol::TimestampMillis(1),
            schema_version: 1,
        }
    }

    fn proposal_observation_batch(batch_id: &str, item_count: usize) -> ProposalObservationBatch {
        proposal_observation_batch_from(batch_id, item_count, 100)
    }

    fn proposal_observation_batch_from(
        batch_id: &str,
        item_count: usize,
        first_proposal_id: u64,
    ) -> ProposalObservationBatch {
        let mut events = Vec::with_capacity(item_count);
        let mut event_metadata = Vec::with_capacity(item_count);
        let mut proposal_audits = Vec::with_capacity(item_count);
        for index in 0..item_count {
            let proposal_id = ProposalId(first_proposal_id + index as u64);
            let correlation_id = CorrelationId(700 + index as u64);
            let causality_id = non_nil_causality_id();
            let principal = PrincipalId(format!("proposal-author-{index}"));
            let capability = CapabilityId("workspace.edit".to_string());
            let event = EventEnvelopeBuilder::new("proposal.created", causality_id)
                .correlation_id(correlation_id)
                .sequence(EventSequence(index as u64 + 1))
                .principal_id(principal.clone())
                .retention(RetentionLabel::Audit)
                .metadata("proposal_id", proposal_id.0)
                .metadata("lifecycle_state", "Created")
                .metadata("capability", capability.0.clone())
                .metadata("payload_kind", "TextEdit")
                .metadata("affected_file_count", 1)
                .metadata("payload_byte_count", 4)
                .metadata("title", format!("proposal-{index}"))
                .metadata("source_text", "do not persist this source")
                .build();
            event_metadata.push(legion_observability::event_metadata_record(&event));
            proposal_audits.push(ProposalAuditRecord {
                proposal_id,
                lifecycle_state: ProposalLifecycleState::Created,
                timestamp: event.occurred_at,
                principal,
                capability,
                correlation_id,
                causality_id,
                payload_summary: ProposalPayloadSummary {
                    kind: ProposalPayloadKind::TextEdit,
                    affected_files: vec![FileId(3 + index as u128)],
                    title: Some(format!("proposal-{index}")),
                    byte_count: Some(4),
                },
                checkpoint_rollback_projection: None,
                risk_rule_ids: Vec::new(),
                diagnostics: Vec::new(),
                redaction_hints: vec![RedactionHint::MetadataOnly],
                schema_version: 1,
            });
            events.push(event);
        }
        ProposalObservationBatch {
            batch_id: batch_id.to_string(),
            events,
            event_metadata,
            proposal_audits,
            schema_version: PROPOSAL_OBSERVATION_BATCH_SCHEMA_VERSION,
        }
    }

    #[derive(Clone)]
    struct FailBatchAtItemSink {
        inner: InMemoryEventSink,
        fail_at_item: Arc<AtomicUsize>,
    }

    impl EventSinkPort for FailBatchAtItemSink {
        fn emit(&self, request: EventSinkRequest) -> ProtocolResult<()> {
            self.inner.emit(request)
        }

        fn emit_batch(&self, requests: Vec<EventSinkRequest>) -> ProtocolResult<()> {
            let fail_at_item = self.fail_at_item.load(Ordering::SeqCst);
            for (index, request) in requests.iter().enumerate() {
                validate_envelope(&request.envelope, EventSinkConfig::default()).map_err(
                    |error| ProtocolError {
                        code: "test_sink_invalid".to_string(),
                        message: error.to_string(),
                    },
                )?;
                if fail_at_item == index.saturating_add(1) {
                    return Err(ProtocolError {
                        code: "test_sink_failure".to_string(),
                        message: format!("injected sink failure at item {index}"),
                    });
                }
            }
            self.inner.emit_batch(requests)
        }
    }

    #[derive(Clone)]
    struct ClassifiedRetrySink {
        inner: InMemoryEventSink,
        transient_failure: Arc<AtomicBool>,
    }

    impl EventSinkPort for ClassifiedRetrySink {
        fn emit(&self, request: EventSinkRequest) -> ProtocolResult<()> {
            self.inner.emit(request)
        }

        fn emit_batch(&self, requests: Vec<EventSinkRequest>) -> ProtocolResult<()> {
            let proposal_id = requests
                .first()
                .and_then(|request| request.envelope.payload["proposal_id"].as_u64())
                .unwrap_or(0);
            if proposal_id == 100 {
                return Err(ProtocolError {
                    code: "event_batch_unsupported".to_string(),
                    message: "permanent test rejection".to_string(),
                });
            }
            if proposal_id == 200 && self.transient_failure.load(Ordering::SeqCst) {
                return Err(ProtocolError {
                    code: "event_sink_unavailable".to_string(),
                    message: "transient test rejection".to_string(),
                });
            }
            self.inner.emit_batch(requests)
        }
    }

    #[test]
    fn proposal_observation_invalid_second_item_leaves_zero_storage_mutation() {
        let port = InMemoryStorageRepositoryPort::new();
        let mut batch = proposal_observation_batch("invalid-second", 2);
        batch.proposal_audits[1].correlation_id = CorrelationId(999);

        port.store_proposal_observation_batch(batch)
            .expect_err("invalid second item must reject the complete batch");

        let counts = port
            .with_storage(|storage| {
                (
                    storage.protocol_proposal_observation_events.len(),
                    storage.protocol_event_metadata.len(),
                    storage.protocol_proposal_audit.len(),
                    storage.protocol_proposal_observation_outbox.len(),
                )
            })
            .expect("inspect storage");
        assert_eq!(counts, (0, 0, 0, 0));
    }

    #[test]
    fn proposal_observation_injected_second_item_failure_leaves_zero_disk_or_memory() {
        let base_dir =
            temp_storage_path("proposal-observation-fail-second").with_extension("outbox-dir");
        let port = InMemoryStorageRepositoryPort::with_base_dir(&base_dir);
        port.fail_proposal_observation_batch_at_item_for_test(1);

        port.store_proposal_observation_batch(proposal_observation_batch("fail-second", 2))
            .expect_err("injected second item failure");

        let counts = port
            .with_storage(|storage| {
                (
                    storage.protocol_proposal_observation_events.len(),
                    storage.protocol_event_metadata.len(),
                    storage.protocol_proposal_audit.len(),
                    storage.protocol_proposal_observation_outbox.len(),
                )
            })
            .expect("inspect storage");
        assert_eq!(counts, (0, 0, 0, 0));
        assert!(!base_dir.join("proposal-observation-outbox").exists());
        let _ = fs::remove_dir_all(base_dir);
    }

    #[test]
    fn proposal_observation_sink_second_item_failure_stays_pending_and_retries_once() {
        let recorder = InMemoryEventSink::new();
        let fail_at_item = Arc::new(AtomicUsize::new(2));
        let sink = FailBatchAtItemSink {
            inner: recorder.clone(),
            fail_at_item: Arc::clone(&fail_at_item),
        };
        let port = InMemoryStorageRepositoryPort::with_event_sink(SharedEventSink::new(sink));
        let batch = proposal_observation_batch("retry-batch", 2);

        port.store_proposal_observation_batch(batch.clone())
            .expect("store Pending batch");
        let error = port
            .deliver_proposal_observation_batch("retry-batch")
            .expect_err("sink failure must keep durable work pending");
        assert_eq!(error.code, "test_sink_failure");
        assert!(recorder.events().expect("event snapshot").is_empty());
        assert_eq!(
            port.pending_proposal_observation_batches()
                .expect("pending records")
                .len(),
            1
        );

        fail_at_item.store(0, Ordering::SeqCst);
        let retried = port
            .retry_pending_proposal_observations()
            .expect("retry pending batch");
        assert_eq!(retried.delivered_count, 1);
        assert_eq!(retried.pending_count, 0);
        assert_eq!(retried.attempts.len(), 1);
        assert_eq!(
            retried.attempts[0].delivery_state,
            ProposalObservationDeliveryState::Delivered
        );
        assert_eq!(recorder.events().expect("event snapshot").len(), 2);

        port.store_proposal_observation_batch(batch.clone())
            .expect("identical retry is idempotent");
        port.deliver_proposal_observation_batch("retry-batch")
            .expect("Delivered retry is idempotent");
        assert_eq!(recorder.events().expect("event snapshot").len(), 2);

        let mut divergent = batch;
        divergent.events[0].payload["title"] = json!("different");
        divergent.proposal_audits[0].payload_summary.title = Some("different".to_string());
        let error = port
            .store_proposal_observation_batch(divergent)
            .expect_err("same batch id with different content must conflict");
        assert_eq!(error.code, "proposal_observation_batch_conflict");
    }

    #[test]
    fn proposal_observation_sink_ack_with_delivery_persist_failure_retries_idempotently() {
        let recorder = InMemoryEventSink::new();
        let port =
            InMemoryStorageRepositoryPort::with_event_sink(SharedEventSink::new(recorder.clone()));
        port.store_proposal_observation_batch(proposal_observation_batch("ack-persist", 1))
            .expect("store pending batch");
        port.fail_next_proposal_observation_delivery_persist_for_test();

        let error = port
            .deliver_proposal_observation_batch("ack-persist")
            .expect_err("delivery marker failure leaves batch pending");
        assert_eq!(error.code, "storage_failed");
        assert_eq!(recorder.events().expect("sink events").len(), 1);
        assert_eq!(
            port.pending_proposal_observation_batches()
                .expect("pending after marker failure")
                .len(),
            1
        );

        let report = port
            .retry_pending_proposal_observations()
            .expect("retry acknowledged event idempotently");
        assert_eq!(report.delivered_count, 1);
        assert_eq!(report.pending_count, 0);
        assert_eq!(
            recorder.events().expect("deduplicated sink events").len(),
            1
        );
        assert!(
            port.pending_proposal_observation_batches()
                .expect("no pending batches")
                .is_empty()
        );
    }

    #[test]
    fn proposal_observation_pending_and_delivered_states_survive_restart() {
        let base_dir =
            temp_storage_path("proposal-observation-restart").with_extension("outbox-dir");
        let first_recorder = InMemoryEventSink::new();
        let fail_at_item = Arc::new(AtomicUsize::new(2));
        let batch = proposal_observation_batch("restart-batch", 2);
        {
            let sink = FailBatchAtItemSink {
                inner: first_recorder.clone(),
                fail_at_item: Arc::clone(&fail_at_item),
            };
            let port = InMemoryStorageRepositoryPort::with_event_sink_and_base_dir(
                SharedEventSink::new(sink),
                &base_dir,
            );
            port.store_proposal_observation_batch(batch.clone())
                .expect("store Pending batch");
            port.deliver_proposal_observation_batch("restart-batch")
                .expect_err("initial delivery fails");
        }

        let outbox_path = base_dir
            .join("proposal-observation-outbox")
            .join("restart-batch.json");
        let persisted = fs::read_to_string(&outbox_path).expect("read pending outbox");
        assert!(!persisted.contains("do not persist this source"));
        assert!(persisted.contains("<redacted>"));

        fail_at_item.store(0, Ordering::SeqCst);
        let second_recorder = InMemoryEventSink::new();
        {
            let sink = FailBatchAtItemSink {
                inner: second_recorder.clone(),
                fail_at_item,
            };
            let port = InMemoryStorageRepositoryPort::with_event_sink_and_base_dir(
                SharedEventSink::new(sink),
                &base_dir,
            );
            assert_eq!(
                port.pending_proposal_observation_batches()
                    .expect("restored pending records")
                    .len(),
                1
            );
            let report = port
                .retry_pending_proposal_observations()
                .expect("deliver restored pending record");
            assert_eq!(report.delivered_count, 1);
            assert_eq!(second_recorder.events().expect("event snapshot").len(), 2);
        }

        let mut reopened = InMemoryStorageRepositoryPort::with_base_dir(&base_dir);
        assert!(
            reopened
                .pending_proposal_observation_batches()
                .expect("reloaded delivered records")
                .is_empty()
        );
        let all_records = reopened
            .proposal_observation_batches()
            .expect("read delivered outbox records");
        assert_eq!(all_records.len(), 1);
        assert_eq!(
            all_records[0].delivery_state,
            ProposalObservationDeliveryState::Delivered
        );
        assert!(
            reopened
                .proposal_observation_batch_matches_stored(batch.clone())
                .expect("compare exact replay batch")
        );
        let mut divergent = batch;
        divergent.events[0].payload["payload_byte_count"] = json!(999);
        divergent.proposal_audits[0].payload_summary.byte_count = Some(999);
        assert!(
            !reopened
                .proposal_observation_batch_matches_stored(divergent)
                .expect("compare divergent replay batch")
        );
        let switch_error = reopened
            .enable_base_dir(base_dir.with_extension("different-root"))
            .expect_err("a live storage port must not mix workspace roots");
        assert_eq!(
            switch_error.code,
            "proposal_observation_workspace_switch_unsupported"
        );
        let _ = fs::remove_dir_all(base_dir);
    }

    #[test]
    fn proposal_observation_restart_preserves_later_audit_lifecycle_state() {
        let base_dir =
            temp_storage_path("proposal-observation-audit-order").with_extension("outbox-dir");
        let batch = proposal_observation_batch("audit-order", 1);
        let proposal_id = batch.proposal_audits[0].proposal_id;
        {
            let port = InMemoryStorageRepositoryPort::with_base_dir(&base_dir);
            port.store_proposal_observation_batch(batch.clone())
                .expect("store created observation");
            let mut applied = batch.proposal_audits[0].clone();
            applied.lifecycle_state = ProposalLifecycleState::Applied;
            port.handle(StorageRepositoryRequest::SaveProposalAuditRecord(applied))
                .expect("persist later applied audit");
        }

        let reopened = InMemoryStorageRepositoryPort::with_base_dir(&base_dir);
        let response = reopened
            .handle(StorageRepositoryRequest::ReadProposalAuditRecord(
                proposal_id,
            ))
            .expect("read reloaded audit");
        match response {
            StorageRepositoryResponse::ProposalAuditRecord(Some(record)) => {
                assert_eq!(record.lifecycle_state, ProposalLifecycleState::Applied);
            }
            other => panic!("unexpected proposal audit response: {other:?}"),
        }
        let _ = fs::remove_dir_all(base_dir);
    }

    #[test]
    fn proposal_observation_batch_ids_are_lowercase_bounded_and_windows_safe() {
        for invalid in [
            "",
            ".",
            "..",
            ".hidden",
            "trailing.",
            "Upper",
            "bad/name",
            "bad name",
            "ümlaut",
            "con",
            "con.json",
            "prn",
            "aux",
            "nul",
            "com1",
            "lpt9.txt",
        ] {
            let port = InMemoryStorageRepositoryPort::new();
            let error = port
                .store_proposal_observation_batch(proposal_observation_batch(invalid, 1))
                .expect_err("unsafe batch id must be rejected");
            assert_eq!(
                error.code, "proposal_observation_batch_invalid",
                "{invalid}"
            );
        }
        let port = InMemoryStorageRepositoryPort::new();
        let too_long = "a".repeat(121);
        assert!(
            port.store_proposal_observation_batch(proposal_observation_batch(&too_long, 1))
                .is_err()
        );
        port.store_proposal_observation_batch(proposal_observation_batch("safe-1_2.3", 1))
            .expect("lowercase safe batch id");
    }

    #[test]
    fn proposal_observation_cross_batch_identity_reuse_is_rejected_after_reopen() {
        let base_dir =
            temp_storage_path("proposal-observation-cross-batch").with_extension("outbox-dir");
        let original = proposal_observation_batch("batch-a", 1);
        {
            let port = InMemoryStorageRepositoryPort::with_base_dir(&base_dir);
            port.store_proposal_observation_batch(original.clone())
                .expect("store first batch");
        }
        let port = InMemoryStorageRepositoryPort::with_base_dir(&base_dir);

        let mut exact_event_reuse = original.clone();
        exact_event_reuse.batch_id = "batch-b".to_string();
        assert_eq!(
            port.store_proposal_observation_batch(exact_event_reuse)
                .expect_err("exact EventId reuse across batches")
                .code,
            "proposal_observation_record_conflict"
        );

        let mut divergent_event_reuse = original.clone();
        divergent_event_reuse.batch_id = "batch-c".to_string();
        divergent_event_reuse.proposal_audits[0]
            .payload_summary
            .affected_files
            .push(FileId(999));
        divergent_event_reuse.events[0].payload["affected_file_count"] = json!(2);
        assert_eq!(
            port.store_proposal_observation_batch(divergent_event_reuse)
                .expect_err("divergent EventId reuse across batches")
                .code,
            "proposal_observation_record_conflict"
        );

        let mut proposal_reuse = proposal_observation_batch_from("batch-d", 1, 300);
        proposal_reuse.proposal_audits[0].proposal_id = original.proposal_audits[0].proposal_id;
        proposal_reuse.events[0].payload["proposal_id"] =
            json!(original.proposal_audits[0].proposal_id.0);
        assert_eq!(
            port.store_proposal_observation_batch(proposal_reuse)
                .expect_err("ProposalId reuse across batches")
                .code,
            "proposal_observation_record_conflict"
        );

        let counts = port
            .with_storage(|storage| {
                (
                    storage.protocol_proposal_observation_events.len(),
                    storage.protocol_event_metadata.len(),
                    storage.protocol_proposal_audit.len(),
                    storage.protocol_proposal_observation_outbox.len(),
                )
            })
            .expect("storage counts");
        assert_eq!(counts, (1, 1, 1, 1));
        assert!(
            !base_dir
                .join("proposal-observation-outbox")
                .join("batch-b.json")
                .exists()
        );
        let _ = fs::remove_dir_all(base_dir);
    }

    #[test]
    fn proposal_observation_loader_records_corruption_without_partial_load() {
        let base_dir =
            temp_storage_path("proposal-observation-corrupt").with_extension("outbox-dir");
        {
            let port = InMemoryStorageRepositoryPort::with_base_dir(&base_dir);
            port.store_proposal_observation_batch(proposal_observation_batch("aaa-valid", 1))
                .expect("persist valid outbox record");
        }
        let outbox_dir = base_dir.join("proposal-observation-outbox");
        fs::write(outbox_dir.join("zzz-corrupt.json"), b"{not-json")
            .expect("write corrupt outbox record");

        let reopened = InMemoryStorageRepositoryPort::with_base_dir(&base_dir);
        assert_eq!(
            reopened
                .proposal_observation_startup_error()
                .expect("recorded startup error")
                .code,
            "proposal_observation_outbox_corrupt"
        );
        assert_eq!(
            reopened
                .pending_proposal_observation_batches()
                .expect_err("pending read fails closed")
                .code,
            "proposal_observation_outbox_corrupt"
        );
        assert_eq!(
            reopened
                .store_proposal_observation_batch(proposal_observation_batch("new-batch", 1))
                .expect_err("store fails closed after corrupt startup")
                .code,
            "proposal_observation_outbox_corrupt"
        );
        let counts = reopened
            .with_storage(|storage| {
                (
                    storage.protocol_proposal_observation_events.len(),
                    storage.protocol_event_metadata.len(),
                    storage.protocol_proposal_audit.len(),
                    storage.protocol_proposal_observation_outbox.len(),
                )
            })
            .expect("storage counts");
        assert_eq!(counts, (0, 0, 0, 0));
        let _ = fs::remove_dir_all(base_dir);
    }

    #[test]
    fn proposal_observation_loader_rejects_well_formed_invalid_record_without_partial_load() {
        let base_dir =
            temp_storage_path("proposal-observation-invalid-json").with_extension("outbox-dir");
        {
            let port = InMemoryStorageRepositoryPort::with_base_dir(&base_dir);
            port.store_proposal_observation_batch(proposal_observation_batch("aaa-valid", 1))
                .expect("persist valid outbox record");
        }
        let outbox_dir = base_dir.join("proposal-observation-outbox");
        let mut invalid: ProposalObservationOutboxRecord = serde_json::from_slice(
            &fs::read(outbox_dir.join("aaa-valid.json")).expect("read valid record"),
        )
        .expect("decode valid record");
        invalid.batch.batch_id = "zzz-invalid".to_string();
        invalid.batch.events[0].retention = RetentionLabel::Hot;
        fs::write(
            outbox_dir.join("zzz-invalid.json"),
            serde_json::to_vec_pretty(&invalid).expect("serialize invalid record"),
        )
        .expect("write structurally invalid record");

        let reopened = InMemoryStorageRepositoryPort::with_base_dir(&base_dir);
        assert_eq!(
            reopened
                .proposal_observation_startup_error()
                .expect("recorded startup error")
                .code,
            "proposal_observation_outbox_corrupt"
        );
        let counts = reopened
            .with_storage(|storage| {
                (
                    storage.protocol_proposal_observation_events.len(),
                    storage.protocol_event_metadata.len(),
                    storage.protocol_proposal_audit.len(),
                    storage.protocol_proposal_observation_outbox.len(),
                )
            })
            .expect("storage counts");
        assert_eq!(counts, (0, 0, 0, 0));
        let _ = fs::remove_dir_all(base_dir);
    }

    #[test]
    fn proposal_observation_loader_sanitizes_legacy_v1_without_mutating_event_identity() {
        let base_dir =
            temp_storage_path("proposal-observation-legacy-v1").with_extension("outbox-dir");
        let mut legacy = proposal_observation_batch("legacy-v1", 1);
        legacy.schema_version = LEGACY_PROPOSAL_OBSERVATION_BATCH_SCHEMA_VERSION;
        legacy.events[0].payload["title"] = json!("legacy raw title");
        legacy.proposal_audits[0].payload_summary.title = Some("legacy raw title".to_string());
        legacy.events = prepare_event_batch(
            legacy
                .events
                .into_iter()
                .map(|envelope| EventSinkRequest { envelope })
                .collect(),
            EventSinkConfig::default(),
        )
        .expect("prepare legacy envelope");
        // Schema 1's redaction order collapsed these structural payload fields,
        // and its producer stamped the event independently of the audit.
        legacy.events[0].payload["payload_kind"] = json!("<redacted>");
        legacy.events[0].payload["payload_byte_count"] = json!("<redacted>");
        legacy.events[0].occurred_at = legion_protocol::TimestampMillis(
            legacy.proposal_audits[0].timestamp.0.saturating_add(1),
        );
        legacy.event_metadata[0] = legion_observability::event_metadata_record(&legacy.events[0]);
        let original_event = serde_json::to_value(&legacy.events[0]).expect("serialize event");
        let original_metadata =
            serde_json::to_value(&legacy.event_metadata[0]).expect("serialize metadata");
        let record = ProposalObservationOutboxRecord {
            batch: legacy,
            delivery_state: ProposalObservationDeliveryState::Pending,
        };
        let outbox_dir = base_dir.join("proposal-observation-outbox");
        fs::create_dir_all(&outbox_dir).expect("create outbox directory");
        fs::write(
            outbox_dir.join("legacy-v1.json"),
            serde_json::to_vec_pretty(&record).expect("serialize legacy record"),
        )
        .expect("write legacy record");

        let reopened = InMemoryStorageRepositoryPort::with_base_dir(&base_dir);
        assert!(reopened.proposal_observation_startup_error().is_none());
        let pending = reopened
            .pending_proposal_observation_batches()
            .expect("read migrated legacy record");
        assert_eq!(pending.len(), 1);
        assert_eq!(
            pending[0].batch.schema_version,
            LEGACY_PROPOSAL_OBSERVATION_BATCH_SCHEMA_VERSION
        );
        assert_eq!(
            serde_json::to_value(&pending[0].batch.events[0]).expect("serialize loaded event"),
            original_event
        );
        assert_eq!(
            serde_json::to_value(&pending[0].batch.event_metadata[0])
                .expect("serialize loaded metadata"),
            original_metadata
        );
        assert!(
            pending[0].batch.proposal_audits[0]
                .payload_summary
                .title
                .as_deref()
                .is_some_and(InMemoryStorageRepositoryPort::is_storage_redaction_marker)
        );

        let disk = fs::read_to_string(outbox_dir.join("legacy-v1.json"))
            .expect("read rewritten legacy record");
        assert!(!disk.contains("legacy raw title"));
        let rewritten: ProposalObservationOutboxRecord =
            serde_json::from_str(&disk).expect("decode rewritten legacy record");
        assert_eq!(
            rewritten.batch.schema_version,
            LEGACY_PROPOSAL_OBSERVATION_BATCH_SCHEMA_VERSION
        );
        assert_eq!(
            serde_json::to_value(&rewritten.batch.events[0]).expect("serialize rewritten event"),
            original_event
        );
        let _ = fs::remove_dir_all(base_dir);
    }

    #[test]
    fn proposal_observation_audit_is_sanitized_before_memory_and_disk_commit() {
        let base_dir =
            temp_storage_path("proposal-observation-sanitize").with_extension("outbox-dir");
        let port = InMemoryStorageRepositoryPort::with_base_dir(&base_dir);
        let mut batch = proposal_observation_batch("sanitize-audit", 1);
        batch.events[0].payload["title"] = json!("raw proposal title");
        batch.proposal_audits[0].payload_summary.title = Some("raw proposal title".to_string());
        batch.proposal_audits[0].risk_rule_ids = vec!["risk.safe-1".to_string()];
        batch.proposal_audits[0].diagnostics = vec![ProtocolDiagnostic {
            code: "proposal.safe".to_string(),
            message: "raw diagnostic message".to_string(),
            severity: ProtocolDiagnosticSeverity::Warning,
            path: Some(CanonicalPath("C:/secret/source.rs".to_string())),
            range: None,
        }];

        let stored = port
            .store_proposal_observation_batch(batch.clone())
            .expect("sanitize audit");
        let audit = &stored.batch.proposal_audits[0];
        assert_eq!(audit.payload_summary.kind, ProposalPayloadKind::TextEdit);
        assert_eq!(audit.payload_summary.affected_files, vec![FileId(3)]);
        assert_eq!(audit.payload_summary.byte_count, Some(4));
        assert_eq!(audit.risk_rule_ids, vec!["risk.safe-1"]);
        assert_eq!(
            audit.diagnostics[0].severity,
            ProtocolDiagnosticSeverity::Warning
        );
        assert!(
            audit
                .payload_summary
                .title
                .as_deref()
                .is_some_and(InMemoryStorageRepositoryPort::is_storage_redaction_marker)
        );
        assert!(InMemoryStorageRepositoryPort::is_storage_redaction_marker(
            &audit.diagnostics[0].message
        ));
        assert!(audit.diagnostics[0].path.as_ref().is_some_and(|path| {
            InMemoryStorageRepositoryPort::is_storage_redaction_marker(&path.0)
        }));
        port.store_proposal_observation_batch(batch)
            .expect("raw retry sanitizes idempotently");

        let disk = fs::read_to_string(
            base_dir
                .join("proposal-observation-outbox")
                .join("sanitize-audit.json"),
        )
        .expect("read outbox");
        for raw in [
            "raw proposal title",
            "raw diagnostic message",
            "C:/secret/source.rs",
        ] {
            assert!(!disk.contains(raw));
        }

        let bad_port = InMemoryStorageRepositoryPort::new();
        let mut unsafe_rule = proposal_observation_batch("unsafe-rule", 1);
        unsafe_rule.proposal_audits[0].risk_rule_ids = vec!["/secret/source.rs".to_string()];
        assert_eq!(
            bad_port
                .store_proposal_observation_batch(unsafe_rule)
                .expect_err("path-like risk rule ids are unsafe")
                .code,
            "proposal_observation_batch_invalid"
        );
        let _ = fs::remove_dir_all(base_dir);
    }

    #[test]
    fn outbox_owned_metadata_and_audit_identity_cannot_be_overwritten() {
        let base_dir = temp_storage_path("proposal-observation-owned").with_extension("outbox-dir");
        let port = InMemoryStorageRepositoryPort::with_base_dir(&base_dir);
        let stored = port
            .store_proposal_observation_batch(proposal_observation_batch("owned", 1))
            .expect("store outbox batch");
        let expected_metadata = stored.batch.event_metadata[0].clone();
        let created_audit = stored.batch.proposal_audits[0].clone();

        port.handle(StorageRepositoryRequest::SaveEventMetadata(
            expected_metadata.clone(),
        ))
        .expect("byte-identical metadata retry");
        let mut conflicting_metadata = expected_metadata.clone();
        conflicting_metadata.event = "proposal.applied".to_string();
        assert_eq!(
            port.handle(StorageRepositoryRequest::SaveEventMetadata(
                conflicting_metadata
            ))
            .expect_err("outbox-owned metadata cannot diverge")
            .code,
            "proposal_observation_record_conflict"
        );

        let mut applied = created_audit.clone();
        applied.lifecycle_state = ProposalLifecycleState::Applied;
        applied.timestamp = legion_protocol::TimestampMillis(applied.timestamp.0.saturating_add(1));
        applied.diagnostics = vec![ProtocolDiagnostic {
            code: "proposal.applied".to_string(),
            message: "raw lifecycle diagnostic".to_string(),
            severity: ProtocolDiagnosticSeverity::Info,
            path: Some(CanonicalPath("C:/private/source.rs".to_string())),
            range: None,
        }];
        port.handle(StorageRepositoryRequest::SaveProposalAuditRecord(
            applied.clone(),
        ))
        .expect("identity-preserving lifecycle audit");

        let mut raw_title_overwrite = applied.clone();
        raw_title_overwrite.payload_summary.title = Some("different raw title".to_string());
        assert_eq!(
            port.handle(StorageRepositoryRequest::SaveProposalAuditRecord(
                raw_title_overwrite
            ))
            .expect_err("bound title cannot be replaced")
            .code,
            "proposal_observation_record_conflict"
        );
        let mut principal_overwrite = applied;
        principal_overwrite.principal = PrincipalId("different-principal".to_string());
        assert_eq!(
            port.handle(StorageRepositoryRequest::SaveProposalAuditRecord(
                principal_overwrite
            ))
            .expect_err("bound principal cannot be replaced")
            .code,
            "proposal_observation_record_conflict"
        );

        match port
            .handle(StorageRepositoryRequest::ReadEventMetadata(
                expected_metadata.event_id,
            ))
            .expect("read owned metadata")
        {
            StorageRepositoryResponse::EventMetadata(Some(actual)) => {
                assert!(
                    InMemoryStorageRepositoryPort::serialized_records_equal(
                        &actual,
                        &expected_metadata
                    )
                    .expect("compare metadata")
                );
            }
            other => panic!("unexpected metadata response: {other:?}"),
        }
        match port
            .handle(StorageRepositoryRequest::ReadProposalAuditRecord(
                created_audit.proposal_id,
            ))
            .expect("read lifecycle audit")
        {
            StorageRepositoryResponse::ProposalAuditRecord(Some(actual)) => {
                assert_eq!(actual.lifecycle_state, ProposalLifecycleState::Applied);
                assert!(InMemoryStorageRepositoryPort::is_storage_redaction_marker(
                    &actual.diagnostics[0].message
                ));
                assert!(actual.diagnostics[0].path.as_ref().is_some_and(|path| {
                    InMemoryStorageRepositoryPort::is_storage_redaction_marker(&path.0)
                }));
            }
            other => panic!("unexpected audit response: {other:?}"),
        }
        drop(port);

        let audit_disk = fs::read_to_string(
            base_dir
                .join("proposal-audit")
                .join(format!("{}.json", created_audit.proposal_id.0)),
        )
        .expect("read persisted lifecycle audit");
        assert!(!audit_disk.contains("raw lifecycle diagnostic"));
        assert!(!audit_disk.contains("C:/private/source.rs"));
        let reopened = InMemoryStorageRepositoryPort::with_base_dir(&base_dir);
        assert!(reopened.proposal_observation_startup_error().is_none());
        assert!(matches!(
            reopened
                .handle(StorageRepositoryRequest::ReadProposalAuditRecord(
                    created_audit.proposal_id
                ))
                .expect("read sanitized audit after restart"),
            StorageRepositoryResponse::ProposalAuditRecord(Some(record))
                if record.lifecycle_state == ProposalLifecycleState::Applied
                    && InMemoryStorageRepositoryPort::is_storage_redaction_marker(
                        &record.diagnostics[0].message
                    )
        ));
        let _ = fs::remove_dir_all(base_dir);
    }

    #[test]
    fn proposal_observation_noop_sink_cannot_mark_pending_batch_delivered() {
        let port = InMemoryStorageRepositoryPort::new();
        port.store_proposal_observation_batch(proposal_observation_batch("noop-pending", 1))
            .expect("store Pending batch");

        assert_eq!(
            port.deliver_proposal_observation_batch("noop-pending")
                .expect_err("Noop sink cannot acknowledge batch")
                .code,
            "event_batch_unsupported"
        );
        assert_eq!(
            port.pending_proposal_observation_batches()
                .expect("Pending batch")
                .len(),
            1
        );
    }

    #[test]
    fn proposal_observation_retry_report_does_not_starve_later_batches() {
        let recorder = InMemoryEventSink::new();
        let transient_failure = Arc::new(AtomicBool::new(true));
        let sink = ClassifiedRetrySink {
            inner: recorder.clone(),
            transient_failure: Arc::clone(&transient_failure),
        };
        let port = InMemoryStorageRepositoryPort::with_event_sink(SharedEventSink::new(sink));
        port.store_proposal_observation_batch(proposal_observation_batch_from("a", 1, 100))
            .expect("store permanent failure batch");
        port.store_proposal_observation_batch(proposal_observation_batch_from("b", 1, 200))
            .expect("store transient failure batch");
        port.store_proposal_observation_batch(proposal_observation_batch_from("c", 1, 300))
            .expect("store successful batch");

        let first = port
            .retry_pending_proposal_observations()
            .expect("complete retry report");
        assert_eq!(first.delivered_count, 1);
        assert_eq!(first.pending_count, 2);
        assert_eq!(
            first
                .attempts
                .iter()
                .map(|attempt| attempt.batch_id.as_str())
                .collect::<Vec<_>>(),
            vec!["a", "b", "c"]
        );
        assert_eq!(
            first.attempts[0].error_code.as_deref(),
            Some("event_batch_unsupported")
        );
        assert_eq!(
            first.attempts[0].error_kind,
            Some(ProposalObservationRetryErrorKind::Permanent)
        );
        assert_eq!(
            first.attempts[1].error_code.as_deref(),
            Some("event_sink_unavailable")
        );
        assert_eq!(
            first.attempts[1].error_kind,
            Some(ProposalObservationRetryErrorKind::Transient)
        );
        assert_eq!(recorder.events().expect("successful later event").len(), 1);

        transient_failure.store(false, Ordering::SeqCst);
        let second = port
            .retry_pending_proposal_observations()
            .expect("second retry report");
        assert_eq!(second.delivered_count, 1);
        assert_eq!(second.pending_count, 1);
        assert_eq!(
            second
                .attempts
                .iter()
                .map(|attempt| attempt.batch_id.as_str())
                .collect::<Vec<_>>(),
            vec!["a", "b"]
        );
        assert_eq!(recorder.events().expect("event snapshot").len(), 2);
    }

    #[test]
    fn proposal_observation_event_and_audit_bindings_fail_closed() {
        for case in [
            "proposal_id",
            "principal",
            "capability",
            "correlation",
            "causality",
            "occurred_at",
            "payload_kind",
            "affected_file_count",
            "byte_count",
            "byte_count_type",
            "title_value",
            "title_missing_event",
            "title_missing_audit",
            "retention",
            "severity",
            "schema",
            "lifecycle",
        ] {
            let port = InMemoryStorageRepositoryPort::new();
            let mut batch = proposal_observation_batch(&format!("binding-{case}"), 1);
            let mut regenerate_metadata = false;
            match case {
                "proposal_id" => batch.events[0].payload["proposal_id"] = json!(999),
                "principal" => {
                    batch.events[0].principal_id =
                        Some(PrincipalId("different-principal".to_string()));
                    regenerate_metadata = true;
                }
                "capability" => {
                    batch.events[0].payload["capability"] = json!("different.capability")
                }
                "correlation" => {
                    batch.events[0].correlation_id = CorrelationId(999);
                    regenerate_metadata = true;
                }
                "causality" => {
                    batch.events[0].causality_id =
                        serde_json::from_value(json!("018f0000-0000-7000-8000-000000000099"))
                            .expect("alternate causality id");
                    regenerate_metadata = true;
                }
                "occurred_at" => {
                    batch.events[0].occurred_at = legion_protocol::TimestampMillis(
                        batch.events[0].occurred_at.0.saturating_add(1),
                    );
                    regenerate_metadata = true;
                }
                "payload_kind" => batch.events[0].payload["payload_kind"] = json!("CreateFile"),
                "affected_file_count" => batch.events[0].payload["affected_file_count"] = json!(2),
                "byte_count" => batch.events[0].payload["payload_byte_count"] = json!(99),
                "byte_count_type" => {
                    batch.proposal_audits[0].payload_summary.byte_count = None;
                    batch.events[0].payload["payload_byte_count"] = json!("4");
                }
                "title_value" => batch.events[0].payload["title"] = json!("different title"),
                "title_missing_event" => {
                    batch.events[0]
                        .payload
                        .as_object_mut()
                        .expect("object payload")
                        .remove("title");
                }
                "title_missing_audit" => batch.proposal_audits[0].payload_summary.title = None,
                "retention" => batch.events[0].retention = RetentionLabel::Hot,
                "severity" => batch.events[0].severity = legion_protocol::EventSeverity::Warning,
                "schema" => batch.schema_version = LEGACY_PROPOSAL_OBSERVATION_BATCH_SCHEMA_VERSION,
                "lifecycle" => batch.events[0].payload["lifecycle_state"] = json!("Applied"),
                _ => unreachable!(),
            }
            if regenerate_metadata {
                batch.event_metadata[0] =
                    legion_observability::event_metadata_record(&batch.events[0]);
            }
            assert_eq!(
                port.store_proposal_observation_batch(batch)
                    .expect_err("binding mismatch must reject atomically")
                    .code,
                "proposal_observation_batch_invalid",
                "case={case}"
            );
            let counts = port
                .with_storage(|storage| {
                    (
                        storage.protocol_proposal_observation_events.len(),
                        storage.protocol_event_metadata.len(),
                        storage.protocol_proposal_audit.len(),
                        storage.protocol_proposal_observation_outbox.len(),
                    )
                })
                .expect("storage counts");
            assert_eq!(counts, (0, 0, 0, 0), "case={case}");
        }

        let port = InMemoryStorageRepositoryPort::new();
        let mut absent_optional_byte_count = proposal_observation_batch("binding-positive", 1);
        absent_optional_byte_count.proposal_audits[0]
            .payload_summary
            .byte_count = None;
        absent_optional_byte_count.events[0]
            .payload
            .as_object_mut()
            .expect("object payload")
            .remove("payload_byte_count");
        port.store_proposal_observation_batch(absent_optional_byte_count)
            .expect("absent optional byte count binds successfully");
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
            operation_id: Some(legion_protocol::CollaborationOperationId(3001)),
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
            operation_id: Some(legion_protocol::RemoteOperationId(8001)),
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
            state: legion_protocol::TerminalRuntimeState::Exited,
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
            category: legion_protocol::HostedTelemetryCategory::Diagnostics,
            classification: legion_protocol::PrivacyClassification::Metadata,
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
            consent_disposition: Some(legion_protocol::AssistedAiConsentState::Granted),
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

    fn delegated_task_audit_linkage_record() -> legion_protocol::DelegatedTaskAuditLinkageRecord {
        legion_protocol::DelegatedTaskAuditLinkageRecord {
            linkage_id: "delegated-task:audit-linkage:plan-1:88".to_string(),
            plan_id: legion_protocol::DelegatedTaskPlanId("plan:p7-2:storage".to_string()),
            plan_hash: semantic_fingerprint("delegated-plan-hash"),
            step_ids: vec![legion_protocol::DelegatedTaskStepId(
                "step:preview".to_string(),
            )],
            proposal_preview_links: Vec::new(),
            trust_projection_references: Vec::new(),
            lineage: None,
            assisted_ai_audit_references: vec![
                legion_protocol::DelegatedTaskAssistedAiAuditReference {
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
                legion_protocol::DelegatedTaskReadinessClassification::WaitingForApproval,
            correlation_id: CorrelationId(901),
            causality_id: non_nil_causality_id(),
            event_sequence: EventSequence(88),
            risk_labels: vec![ProposalRiskLabel::Medium],
            privacy_labels: vec![ProposalPrivacyLabel::WorkspaceMetadata],
            runtime_activation: legion_protocol::DelegatedTaskRuntimeActivationState::NotEncoded,
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
        privacy_scope: legion_protocol::SemanticPrivacyScope,
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
        privacy_scope: legion_protocol::SemanticPrivacyScope,
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
            persisted_at: legion_protocol::TimestampMillis(1),
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
    fn generic_proposal_audit_save_sanitizes_raw_fields_before_restart() {
        let base_dir = temp_storage_path("generic-proposal-audit-sanitize")
            .with_extension("proposal-audit-dir");
        let mut audit = audit_record();
        audit.payload_summary.title = Some("raw generic audit title".to_string());
        audit.diagnostics = vec![ProtocolDiagnostic {
            code: "proposal.generic".to_string(),
            message: "raw generic diagnostic".to_string(),
            severity: ProtocolDiagnosticSeverity::Warning,
            path: Some(CanonicalPath("C:/private/generic.rs".to_string())),
            range: None,
        }];
        {
            let port = InMemoryStorageRepositoryPort::with_base_dir(&base_dir);
            port.handle(StorageRepositoryRequest::SaveProposalAuditRecord(
                audit.clone(),
            ))
            .expect("save sanitized generic audit");
            let disk = fs::read_to_string(
                base_dir
                    .join("proposal-audit")
                    .join(format!("{}.json", audit.proposal_id.0)),
            )
            .expect("read generic audit disk record");
            assert!(!disk.contains("raw generic audit title"));
            assert!(!disk.contains("raw generic diagnostic"));
            assert!(!disk.contains("C:/private/generic.rs"));
        }

        let reopened = InMemoryStorageRepositoryPort::with_base_dir(&base_dir);
        assert!(reopened.proposal_observation_startup_error().is_none());
        match reopened
            .handle(StorageRepositoryRequest::ReadProposalAuditRecord(
                audit.proposal_id,
            ))
            .expect("read generic audit after restart")
        {
            StorageRepositoryResponse::ProposalAuditRecord(Some(record)) => {
                assert!(
                    record
                        .payload_summary
                        .title
                        .as_deref()
                        .is_some_and(InMemoryStorageRepositoryPort::is_storage_redaction_marker)
                );
                assert!(InMemoryStorageRepositoryPort::is_storage_redaction_marker(
                    &record.diagnostics[0].message
                ));
                assert!(record.diagnostics[0].path.as_ref().is_some_and(|path| {
                    InMemoryStorageRepositoryPort::is_storage_redaction_marker(&path.0)
                }));
            }
            other => panic!("unexpected proposal audit response: {other:?}"),
        }
        let _ = fs::remove_dir_all(base_dir);
    }

    fn audit_with_official_checkpoint_projection() -> ProposalAuditRecord {
        let mut audit = audit_record();
        let proposal = WorkspaceProposal {
            proposal_id: audit.proposal_id,
            principal: audit.principal.clone(),
            capability: audit.capability.clone(),
            correlation_id: audit.correlation_id,
            payload: ProposalPayload::TextEdit(TextEditProposal {
                file_id: FileId(3),
                edits: EditBatch { edits: Vec::new() },
            }),
            preconditions: ProposalVersionPreconditions {
                file_version: None,
                buffer_version: None,
                snapshot_id: None,
                generation: None,
                file_content_version: Some(FileContentVersion(4)),
                workspace_generation: Some(WorkspaceGeneration(5)),
                expected_fingerprint: Some(FileFingerprint {
                    algorithm: "sha256".to_string(),
                    value: "abc123".to_string(),
                }),
                expected_file_length: Some(4),
                expected_modified_at: Some(legion_protocol::TimestampMillis(1)),
            },
            preview: PreviewSummary {
                summary: "not persisted by audit".to_string(),
                details: Vec::new(),
            },
            expires_at: None,
            created_at: legion_protocol::TimestampMillis(1),
        };
        audit.checkpoint_rollback_projection = Some(checkpoint_rollback_projection_from_proposal(
            "checkpoint-rollback:audit-1",
            &proposal,
            audit.lifecycle_state,
            None,
            legion_protocol::CheckpointRollbackAuditStatus::Available,
            Some(audit.causality_id),
            audit.timestamp,
            1,
        ));
        audit
    }

    #[test]
    fn generic_proposal_audit_recursively_sanitizes_checkpoint_projection_on_restart() {
        let base_dir = temp_storage_path("proposal-audit-checkpoint-sanitize")
            .with_extension("proposal-audit-dir");
        let mut audit = audit_with_official_checkpoint_projection();
        let projection = audit
            .checkpoint_rollback_projection
            .as_mut()
            .expect("checkpoint projection");
        let projection_id = projection.projection_id.clone();
        let checkpoint_id = projection.checkpoint.checkpoint_id.clone();
        let target_id = projection.targets[0].target_id.clone();
        let expected_hashes = projection.targets[0].hashes.clone();
        projection.checkpoint.labels = vec!["raw checkpoint label /private".to_string()];
        projection.rollback.labels = vec!["raw rollback label /private".to_string()];
        projection.targets[0].labels = vec!["raw target label /private".to_string()];
        projection.checkpoint.limitations[0].label =
            "raw checkpoint limitation /private".to_string();
        projection.rollback.limitations[0].label = "raw rollback limitation /private".to_string();
        let raw_values = [
            "raw checkpoint label /private",
            "raw rollback label /private",
            "raw target label /private",
            "raw checkpoint limitation /private",
            "raw rollback limitation /private",
        ];

        {
            let port = InMemoryStorageRepositoryPort::with_base_dir(&base_dir);
            port.handle(StorageRepositoryRequest::SaveProposalAuditRecord(
                audit.clone(),
            ))
            .expect("save recursively sanitized audit");
            let stored = match port
                .handle(StorageRepositoryRequest::ReadProposalAuditRecord(
                    audit.proposal_id,
                ))
                .expect("read sanitized checkpoint audit")
            {
                StorageRepositoryResponse::ProposalAuditRecord(Some(record)) => record,
                other => panic!("unexpected proposal audit response: {other:?}"),
            };
            let stored_projection = stored
                .checkpoint_rollback_projection
                .expect("stored checkpoint projection");
            assert_eq!(stored_projection.projection_id, projection_id);
            assert_eq!(stored_projection.checkpoint.checkpoint_id, checkpoint_id);
            assert_eq!(stored_projection.targets[0].target_id, target_id);
            assert_eq!(stored_projection.targets[0].hashes, expected_hashes);
            assert_eq!(
                stored_projection.checkpoint.target_count as usize,
                stored_projection.targets.len()
            );
            assert!(
                stored_projection
                    .checkpoint
                    .labels
                    .iter()
                    .chain(&stored_projection.rollback.labels)
                    .chain(&stored_projection.targets[0].labels)
                    .chain(
                        stored_projection
                            .checkpoint
                            .limitations
                            .iter()
                            .map(|limitation| &limitation.label)
                    )
                    .chain(
                        stored_projection
                            .rollback
                            .limitations
                            .iter()
                            .map(|limitation| &limitation.label)
                    )
                    .all(|label| InMemoryStorageRepositoryPort::is_storage_redaction_marker(label))
            );
        }

        let audit_path = base_dir
            .join("proposal-audit")
            .join(format!("{}.json", audit.proposal_id.0));
        let disk = fs::read_to_string(&audit_path).expect("read sanitized projection audit");
        for raw in raw_values {
            assert!(!disk.contains(raw), "raw nested value leaked: {raw}");
        }
        let reopened = InMemoryStorageRepositoryPort::with_base_dir(&base_dir);
        assert!(reopened.proposal_observation_startup_error().is_none());
        let reopened_record = match reopened
            .handle(StorageRepositoryRequest::ReadProposalAuditRecord(
                audit.proposal_id,
            ))
            .expect("read checkpoint audit after restart")
        {
            StorageRepositoryResponse::ProposalAuditRecord(Some(record)) => record,
            other => panic!("unexpected proposal audit response: {other:?}"),
        };
        let reopened_projection = reopened_record
            .checkpoint_rollback_projection
            .expect("reopened checkpoint projection");
        assert_eq!(reopened_projection.projection_id, projection_id);
        assert_eq!(reopened_projection.checkpoint.checkpoint_id, checkpoint_id);
        assert_eq!(reopened_projection.targets[0].target_id, target_id);
        assert_eq!(reopened_projection.targets[0].hashes, expected_hashes);
        let _ = fs::remove_dir_all(base_dir);
    }

    #[test]
    fn generic_proposal_audit_rejects_invalid_nested_checkpoint_structure() {
        for case in [
            "projection_id",
            "fingerprint",
            "schema",
            "redaction",
            "target",
        ] {
            let port = InMemoryStorageRepositoryPort::new();
            let mut audit = audit_with_official_checkpoint_projection();
            let projection = audit
                .checkpoint_rollback_projection
                .as_mut()
                .expect("checkpoint projection");
            match case {
                "projection_id" => projection.projection_id = "../private/path".to_string(),
                "fingerprint" => {
                    projection.targets[0].hashes[0].value = "C:/private/source.rs".to_string()
                }
                "schema" => projection.checkpoint.schema_version = 0,
                "redaction" => projection.targets[0].redaction_hints = vec![RedactionHint::Full],
                "target" => {
                    projection.rollback.limitations[0].target_id =
                        Some("missing-target".to_string())
                }
                _ => unreachable!(),
            }
            assert_eq!(
                port.handle(StorageRepositoryRequest::SaveProposalAuditRecord(audit))
                    .expect_err("invalid nested checkpoint projection")
                    .code,
                "proposal_audit_invalid",
                "case={case}"
            );
        }
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
                    legion_protocol::DelegatedTaskRuntimeActivationState::NotEncoded
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
                from_state: legion_protocol::AgentRunState::Observing,
                to_state: legion_protocol::AgentRunState::Planning,
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
        let namespace = legion_protocol::PluginStateNamespace {
            plugin_id: legion_protocol::PluginId(7),
            namespace: "state".to_string(),
        };
        let record = PluginStorageRecord {
            workspace_id: WorkspaceId(1),
            plugin_id: legion_protocol::PluginId(7),
            namespace: namespace.clone(),
            key: "settings".to_string(),
            value: "metadata-only".to_string(),
            schema_version: 1,
            retention: legion_protocol::RetentionLabel::Warm,
            redaction: RedactionHint::MetadataOnly,
            byte_count: 13,
        };

        let put = storage
            .handle(StorageRepositoryRequest::PluginStorage(
                legion_protocol::PluginStorageRequest {
                    operation: PluginStorageOperation::Put,
                    workspace_id: WorkspaceId(1),
                    plugin_id: legion_protocol::PluginId(7),
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
                legion_protocol::PluginStorageResponse::Stored { used_bytes: 13, .. }
            )
        ));

        let get = storage
            .handle(StorageRepositoryRequest::PluginStorage(
                legion_protocol::PluginStorageRequest {
                    operation: PluginStorageOperation::Get,
                    workspace_id: WorkspaceId(1),
                    plugin_id: legion_protocol::PluginId(7),
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
            StorageRepositoryResponse::PluginStorage(legion_protocol::PluginStorageResponse::Record(
                Some(stored)
            )) if stored.key == "settings"
        ));

        let escape = storage
            .handle(StorageRepositoryRequest::PluginStorage(
                legion_protocol::PluginStorageRequest {
                    operation: PluginStorageOperation::Get,
                    workspace_id: WorkspaceId(1),
                    plugin_id: legion_protocol::PluginId(8),
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
                legion_protocol::PluginStorageResponse::Denied {
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
                legion_protocol::PluginStorageRequest {
                    operation: PluginStorageOperation::Put,
                    workspace_id: WorkspaceId(1),
                    plugin_id: legion_protocol::PluginId(7),
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
                legion_protocol::PluginStorageResponse::Denied {
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
            legion_protocol::SemanticPrivacyScope::Workspace,
            WorkspaceGeneration(5),
        );
        let query = SemanticMetadataQuery {
            workspace_id: WorkspaceId(77),
            file_ids: vec![FileId(88)],
            language_ids: vec![LanguageId("rust".to_string())],
            privacy_scope: legion_protocol::SemanticPrivacyScope::Workspace,
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
            legion_protocol::SemanticPrivacyScope::Workspace,
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
                legion_protocol::SemanticPrivacyScope::MetadataOnly,
                WorkspaceGeneration(5),
            )),
            reason: SemanticMetadataTombstoneReason::PrivacyScopeRevoked,
            observed_at: legion_protocol::TimestampMillis(2),
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
            legion_protocol::SemanticPrivacyScope::Workspace,
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
            privacy_scope: legion_protocol::SemanticPrivacyScope::Workspace,
            freshness_key: Some(semantic_freshness_key(
                legion_protocol::SemanticPrivacyScope::Workspace,
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

    #[test]
    fn file_backed_storage_roundtrips_protocol_workspace_metadata_session_and_trust() {
        let path = temp_storage_path("protocol-core-roundtrip");
        let workspace_id = WorkspaceId(4242);
        let principal_id = PrincipalId("trust-principal".to_string());

        let config = WorkspaceConfigSnapshot {
            workspace_id,
            root_path: CanonicalPath("C:/repo".to_string()),
            merged: HashMap::from([("theme".to_string(), "dark".to_string())]),
            trust_state: WorkspaceTrustState::Trusted,
            captured_at: legion_protocol::TimestampMillis(10),
            schema_version: "1".to_string(),
        };
        let metadata = FileMetadata {
            canonical_path: CanonicalPath("C:/repo/src/main.rs".to_string()),
            file_id: None,
            workspace_id: Some(workspace_id),
            kind: legion_protocol::FileKind::File,
            size_bytes: Some(128),
            modified_at: Some(legion_protocol::TimestampMillis(11)),
            read_only: false,
            permissions: None,
            hash: Some("abc123".to_string()),
            fingerprint: None,
            content_version: None,
            workspace_generation: None,
            schema_version: 1,
        };
        let session = WorkspaceSessionRecord {
            session_id: "session-protocol".to_string(),
            last_workspace: Some(workspace_id),
            last_workspace_path: Some(CanonicalPath("C:/repo".to_string())),
            open_tabs: Vec::new(),
            active_tab: None,
            active_buffer: None,
            tab_groups: Vec::new(),
            layout_splits: Vec::new(),
            explorer_expansion: Vec::new(),
            panel_state: legion_protocol::SessionPanelState {
                bottom_visible: true,
                side_visible: true,
                active_panel: Some("explorer".to_string()),
                bottom_height_px: Some(200),
                side_width_px: Some(320),
            },
            dock_layouts: Vec::new(),
            workbench_settings: legion_protocol::WorkbenchSettingsRecord::default(),
            memory_snapshot_json: None,
            dirty_indicators: Vec::new(),
            saved_at: legion_protocol::TimestampMillis(12),
            schema_version: 1,
        };
        let trust = TrustRecord {
            workspace_id,
            principal_id: principal_id.clone(),
            trust_state: WorkspaceTrustState::Trusted,
            decision_id: None,
            correlation_id: CorrelationId(5),
            recorded_at: legion_protocol::TimestampMillis(13),
            schema_version: 1,
        };

        {
            let mut storage = FileBackedStorage::open(&path).expect("open storage");
            storage
                .state
                .handle_protocol_request(StorageRepositoryRequest::SaveWorkspaceConfig(
                    config.clone(),
                ))
                .expect("save workspace config");
            storage
                .state
                .handle_protocol_request(StorageRepositoryRequest::SaveFileMetadata(
                    metadata.clone(),
                ))
                .expect("save file metadata");
            storage
                .state
                .handle_protocol_request(StorageRepositoryRequest::SaveSessionRecord(
                    session.clone(),
                ))
                .expect("save session record");
            storage
                .state
                .handle_protocol_request(StorageRepositoryRequest::SaveTrustRecord(trust.clone()))
                .expect("save trust record");
            storage.flush().expect("flush storage");
        }

        let mut reopened = FileBackedStorage::open(&path).expect("reopen storage");

        match reopened
            .state
            .handle_protocol_request(StorageRepositoryRequest::ReadWorkspaceConfig(workspace_id))
            .expect("read workspace config")
        {
            StorageRepositoryResponse::WorkspaceConfig(Some(loaded)) => {
                assert_eq!(loaded.workspace_id, workspace_id);
                assert_eq!(loaded.merged.get("theme").map(String::as_str), Some("dark"));
            }
            other => panic!("unexpected workspace config response: {other:?}"),
        }

        let file_id = FileId(legion_protocol_stable_hash(&metadata.canonical_path.0));
        match reopened
            .state
            .handle_protocol_request(StorageRepositoryRequest::ReadFileMetadata(file_id))
            .expect("read file metadata")
        {
            StorageRepositoryResponse::FileMetadata(Some(loaded)) => {
                assert_eq!(loaded.canonical_path, metadata.canonical_path);
                assert_eq!(loaded.hash.as_deref(), Some("abc123"));
            }
            other => panic!("unexpected file metadata response: {other:?}"),
        }

        match reopened
            .state
            .handle_protocol_request(StorageRepositoryRequest::ReadSessionRecord {
                session_id: "session-protocol".to_string(),
            })
            .expect("read session record")
        {
            StorageRepositoryResponse::SessionRecord(loaded) => {
                assert_eq!(
                    loaded.expect("session record").session_id,
                    "session-protocol"
                );
            }
            other => panic!("unexpected session record response: {other:?}"),
        }

        match reopened
            .state
            .handle_protocol_request(StorageRepositoryRequest::ReadTrustRecord {
                workspace_id,
                principal_id: principal_id.clone(),
            })
            .expect("read trust record")
        {
            StorageRepositoryResponse::TrustRecord(Some(loaded)) => {
                assert_eq!(loaded.principal_id, principal_id);
                assert_eq!(loaded.trust_state, WorkspaceTrustState::Trusted);
            }
            other => panic!("unexpected trust record response: {other:?}"),
        }

        let _ = fs::remove_file(path);
    }

    #[test]
    fn storage_checksum_is_collision_resistant_and_order_sensitive() {
        // Known SHA-256 vectors confirm the digest is computed correctly.
        assert_eq!(
            storage_checksum(b""),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
        assert_eq!(
            storage_checksum(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );

        // Byte permutations that preserve sum+len (and therefore fool the legacy checksum)
        // produce different SHA-256 digests.
        let original = [1u8, 2, 3, 4];
        let swapped = [4u8, 3, 2, 1];
        assert_eq!(stable_storage_sum(&original), stable_storage_sum(&swapped));
        assert_ne!(storage_checksum(&original), storage_checksum(&swapped));
    }

    #[test]
    fn record_event_persists_before_emitting_and_fails_closed_on_store_error() {
        fn envelope() -> EventEnvelope {
            EventEnvelope {
                schema_version: 1,
                event_id: event_id(),
                parent_event_id: None,
                causality_id: non_nil_causality_id(),
                event: "proposal.audit_recorded".to_string(),
                severity: legion_protocol::EventSeverity::Info,
                retention: RetentionLabel::Audit,
                redaction: RedactionHint::MetadataOnly,
                correlation_id: CorrelationId(7),
                workspace_id: Some(WorkspaceId(1)),
                sequence: EventSequence(1),
                principal_id: Some(PrincipalId("tester".to_string())),
                occurred_at: legion_protocol::TimestampMillis(1),
                payload: serde_json::json!({}),
            }
        }

        let recorder = legion_observability::InMemoryEventSink::new();
        let port =
            InMemoryStorageRepositoryPort::with_event_sink(SharedEventSink::new(recorder.clone()));

        // Store failure must fail closed: no audit metadata persisted AND no event emitted.
        port.fail_next_event_metadata_write();
        port.record_event(envelope())
            .expect_err("record_event must fail when the store fails");
        assert!(
            recorder.events().expect("read sink").is_empty(),
            "no event may be emitted when the audit store write fails"
        );

        // Success path: metadata persisted and exactly one event emitted.
        port.record_event(envelope())
            .expect("record_event succeeds");
        let emitted = recorder.events().expect("read sink");
        assert_eq!(emitted.len(), 1);
        match port
            .handle(StorageRepositoryRequest::ReadEventMetadata(event_id()))
            .expect("read event metadata")
        {
            StorageRepositoryResponse::EventMetadata(Some(loaded)) => {
                assert_eq!(loaded.event_id, event_id());
            }
            other => panic!("unexpected event metadata response: {other:?}"),
        }
    }

    // ── IMP-2: FilePaletteUsageRepository persistence ────────────────────────

    fn palette_usage_test_path(suffix: &str) -> std::path::PathBuf {
        let ts = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        std::env::temp_dir().join(format!(
            "legion_palette_usage_test_{}_{}_{}.json",
            std::process::id(),
            ts,
            suffix
        ))
    }

    /// Write-read round-trip: counts survive a `FilePaletteUsageRepository`
    /// re-open from the same file (simulating a process restart).
    #[test]
    fn file_palette_usage_round_trip() {
        let path = palette_usage_test_path("roundtrip");
        let ws = WorkspaceId(1);

        {
            let mut repo = FilePaletteUsageRepository::open(&path);
            repo.record_usage(ws, "command:save");
            repo.record_usage(ws, "command:save");
            repo.record_usage(ws, "file:main.rs");
        }

        // Re-open from disk — simulates a process restart.
        let repo2 = FilePaletteUsageRepository::open(&path);
        assert_eq!(repo2.usage_count(ws, "command:save"), 2);
        assert_eq!(repo2.usage_count(ws, "file:main.rs"), 1);
        assert_eq!(repo2.usage_count(ws, "command:never-used"), 0);

        let _ = fs::remove_file(&path);
    }

    /// Restart simulation: a heavily-used command retains its ranking boost
    /// after a simulated restart (re-open from disk).
    #[test]
    fn file_palette_usage_restart_retains_ranking_boost() {
        let path = palette_usage_test_path("restart");
        let ws = WorkspaceId(99);

        {
            let mut repo = FilePaletteUsageRepository::open(&path);
            for _ in 0..20 {
                repo.record_usage(ws, "command:heavy");
            }
            repo.record_usage(ws, "command:light");
        }

        // After restart, "heavy" must still rank first.
        let repo2 = FilePaletteUsageRepository::open(&path);
        let top = repo2.top_items(ws);
        assert_eq!(
            top.first().map(|r| r.item_key.as_str()),
            Some("command:heavy"),
            "command:heavy must be top-ranked after restart"
        );
        assert_eq!(top[0].usage_count, 20);
        assert_eq!(top[1].usage_count, 1);

        let _ = fs::remove_file(&path);
    }

    /// Cap eviction: when total entries exceed 500 the lowest-count entries
    /// are evicted so the map never grows unbounded.
    #[test]
    fn file_palette_usage_cap_eviction() {
        let path = palette_usage_test_path("cap");
        let ws = WorkspaceId(7);

        let mut repo = FilePaletteUsageRepository::open(&path);
        // Record 501 distinct items so the cap kicks in.
        for i in 0..=500usize {
            repo.record_usage(ws, &format!("item:{i}"));
        }
        // After capping, at most PALETTE_USAGE_MAX_ENTRIES entries remain.
        assert!(
            repo.top_items(ws).len() <= PALETTE_USAGE_MAX_ENTRIES,
            "entries must not exceed cap after eviction"
        );

        let _ = fs::remove_file(&path);
    }
}
