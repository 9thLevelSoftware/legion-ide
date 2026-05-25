//! Indexing Engine: actor-owned semantic scheduling, repository discovery,
//! lexical symbol maps, deterministic parser-cache fallbacks, and pure query DTOs.

#![warn(missing_docs)]

use std::collections::{HashMap, VecDeque};

use devil_protocol::{
    ByteRange, CancellationTokenId, CanonicalPath, CapabilityId, EditBatch, FileContentVersion,
    FileFingerprint, FileId, FileIdentity, LanguageId, LineIndexRange, LspDiagnosticSummary,
    ProposalAffectedTarget, ProposalPayloadKind, ProposalPayloadSummary, ProposalTargetCoverage,
    ProposalTargetCoverageKind, ProposalTargetKind, ProposalVersionPreconditions,
    ProtocolDiagnostic, ProtocolDiagnosticSeverity, ProtocolResult, ProtocolTextRange,
    RedactionHint, SemanticCancellationReason, SemanticCancellationToken,
    SemanticFabricDescriptorReference, SemanticFabricInvalidationCause, SemanticFabricJobRequest,
    SemanticFabricPriority, SemanticFabricPrivacyLabel, SemanticFabricSchedulePlan,
    SemanticFabricSchedulingAction, SemanticFabricSchedulingDecision,
    SemanticFabricSchedulingTrigger, SemanticFabricWorkSourceKind, SemanticFileFingerprintIdentity,
    SemanticFreshness, SemanticFreshnessState, SemanticGrammarVersion, SemanticGraphEndpoint,
    SemanticGraphRecord, SemanticGraphRecordKind, SemanticInvalidationKey,
    SemanticMetadataChunkReference, SemanticMetadataDescriptorIdentity,
    SemanticMetadataDiagnosticSummary, SemanticMetadataFreshnessKey, SemanticMetadataGraphRecord,
    SemanticMetadataRecord, SemanticMetadataSourceKind, SemanticMetadataSymbolRecord,
    SemanticModelVersion, SemanticPort, SemanticPrivacyScope, SemanticProperty,
    SemanticQueryFreshnessPolicy, SemanticQueryKind, SemanticQueryRequest, SemanticQueryResponse,
    SemanticQueryResult, SemanticQueryResultKind, SemanticQueryStatus, SemanticRecordId,
    SemanticRecordProvenance, SemanticRecordSource, SemanticRequest, SemanticResponse,
    SemanticSymbolId, SnapshotChunkDescriptor, SnapshotDescriptor, SnapshotId, SnapshotLeaseChunk,
    SnapshotLeaseDescriptor, SymbolFileMapRecord, TextCoordinate, TextEdit, TextOffset, TextRange,
    TimestampMillis, WorkspaceDiscoveryChangeKind, WorkspaceDiscoveryDecision,
    WorkspaceDiscoveryDelta, WorkspaceDiscoveryRecord, WorkspaceDiscoverySkipReason,
    WorkspaceDiscoverySnapshot, WorkspaceGeneration, WorkspaceId, WorkspaceTextEdit,
};
use devil_text::{TextChunkDescriptor, TextSnapshot};
use thiserror::Error;

/// Schema version emitted by the activated indexing crate DTOs.
pub const INDEX_SCHEMA_VERSION: u16 = 1;

/// Deterministic extraction contract version for lexical semantic records.
pub const LEXICAL_EXTRACTION_VERSION: &str = "devil-index-lexical-v1";

/// Default grammar version used by the lexical parser fallback.
pub const DEFAULT_GRAMMAR_VERSION: &str = "lexical-fallback-grammar-v1";

/// Default non-vector model metadata version used for deterministic ranking records.
pub const DEFAULT_MODEL_VERSION: &str = "semantic-ranking-metadata-v1";

const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

/// Result alias for indexing operations.
pub type IndexResult<T> = Result<T, IndexError>;

/// Errors produced by repository discovery, scheduling, parsing, and query helpers.
#[derive(Debug, Error)]
pub enum IndexError {
    /// Filesystem I/O failed while reading repository state.
    #[error("I/O error at `{path}`: {message}")]
    Io {
        /// Path associated with the failed operation.
        path: String,
        /// Metadata-only failure message.
        message: String,
    },
    /// A text snapshot did not expose bounded full text to the indexing crate.
    #[error("snapshot text unavailable: {message}")]
    TextSnapshotUnavailable {
        /// Metadata-only failure message.
        message: String,
    },
    /// The actor queue rejected work because no lower-priority work could be displaced.
    #[error(
        "index queue backpressure: capacity {capacity}, pending {pending_len}, priority {priority:?}"
    )]
    QueueBackpressure {
        /// Configured queue capacity.
        capacity: usize,
        /// Pending work count at rejection time.
        pending_len: usize,
        /// Priority of the rejected work.
        priority: WorkPriority,
    },
    /// A caller attempted to complete work that is not currently in flight.
    #[error("index work `{work_id}` is not in flight")]
    WorkNotInFlight {
        /// Work identifier that was not in flight.
        work_id: u64,
    },
    /// Repository scan configuration was invalid.
    #[error("invalid repository scan config: {message}")]
    InvalidConfig {
        /// Metadata-only validation message.
        message: String,
    },
}

/// Scheduling priority for actor-owned semantic work.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum WorkPriority {
    /// Background repository scan work that must yield to interactive work.
    BackgroundScan,
    /// Normal semantic indexing work.
    Normal,
    /// Foreground navigation or completion-support work.
    Foreground,
    /// Live editor snapshot work that supersedes slower background scans.
    LiveSnapshot,
}

/// Kind of semantic work owned by the in-process indexing actor.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum IndexWorkKind {
    /// Repository-wide discovery or scan work.
    RepositoryScan,
    /// Background file indexing work from disk discovery.
    BackgroundFile,
    /// Live snapshot indexing work from an editor-owned snapshot copy.
    LiveSnapshot,
    /// Query-support work that warms low-latency semantic records.
    SemanticQuery,
    /// Cache maintenance work.
    Maintenance,
}

/// Terminal state for a work item after actor processing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum WorkCompletionState {
    /// Work was queued and accepted for future processing.
    Queued,
    /// Work was started by the actor.
    InFlight,
    /// Work produced records that were applied to the semantic index.
    Applied,
    /// Work was cancelled and acknowledged.
    Cancelled,
    /// Work completed after being superseded and was intentionally ignored.
    IgnoredObsolete,
    /// Work was rejected by explicit queue backpressure.
    Rejected,
}

/// Acknowledgement emitted when semantic cancellation is observed locally.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SemanticCancellationAck {
    /// Token that was acknowledged.
    pub token_id: CancellationTokenId,
    /// Reason associated with the cancellation.
    pub reason: SemanticCancellationReason,
    /// Acknowledgement timestamp.
    pub acknowledged_at: TimestampMillis,
    /// Whether the token belonged to in-flight work.
    pub was_in_flight: bool,
    /// Whether queued work was removed by this acknowledgement.
    pub removed_from_queue: bool,
}

/// Actor-owned work item carrying the immutable data needed for semantic indexing.
#[derive(Debug, Clone)]
pub struct IndexWorkItem {
    /// Actor-assigned monotonic work identifier; callers may pass `0` before submission.
    pub work_id: u64,
    /// Work kind used for diagnostics and scheduling policy.
    pub kind: IndexWorkKind,
    /// Scheduling priority.
    pub priority: WorkPriority,
    /// Cancellation token descriptor bound to this work item.
    pub cancellation: SemanticCancellationToken,
    /// Optional source document to parse and index.
    pub document: Option<SourceDocument>,
    /// Submission timestamp.
    pub submitted_at: TimestampMillis,
}

impl IndexWorkItem {
    /// Constructs an index work item with caller-provided cancellation metadata.
    pub fn new(
        kind: IndexWorkKind,
        priority: WorkPriority,
        cancellation: SemanticCancellationToken,
        document: Option<SourceDocument>,
    ) -> Self {
        Self {
            work_id: 0,
            kind,
            priority,
            cancellation,
            document,
            submitted_at: TimestampMillis::now(),
        }
    }
}

/// Outcome returned by successful queue submission.
#[derive(Debug, Clone)]
pub struct IndexSubmitOutcome {
    /// Work identifier assigned by the actor.
    pub accepted_work_id: u64,
    /// Cancellation acknowledgements caused by pressure or supersession.
    pub cancellations: Vec<SemanticCancellationAck>,
    /// Pending queue length after acceptance.
    pub pending_len: usize,
}

/// Handle returned when work is moved from the pending queue into in-flight state.
#[derive(Debug, Clone)]
pub struct StartedIndexWork {
    /// Work item that is now in flight.
    pub item: IndexWorkItem,
    /// Timestamp at which the actor started the work.
    pub started_at: TimestampMillis,
}

/// Processing report emitted when a started work item is completed.
#[derive(Debug, Clone)]
pub struct IndexWorkReport {
    /// Work identifier that completed.
    pub work_id: u64,
    /// Completion state.
    pub state: WorkCompletionState,
    /// Cancellation acknowledgement when completion observed cancellation.
    pub cancellation_ack: Option<SemanticCancellationAck>,
    /// Upsert outcome when semantic records were considered for storage.
    pub upsert: Option<SemanticUpsertOutcome>,
    /// Diagnostics produced while processing the work item.
    pub diagnostics: Vec<ProtocolDiagnostic>,
}

#[derive(Debug, Clone)]
struct LatestWorkIdentity {
    priority: WorkPriority,
    workspace_generation: WorkspaceGeneration,
    file_content_version: FileContentVersion,
    content_hash: FileFingerprint,
}

fn version_tuple(
    workspace_generation: WorkspaceGeneration,
    file_content_version: FileContentVersion,
) -> (u64, u64) {
    (workspace_generation.0, file_content_version.0)
}

fn latest_work_version_tuple(identity: &LatestWorkIdentity) -> (u64, u64) {
    version_tuple(identity.workspace_generation, identity.file_content_version)
}

fn source_document_version_tuple(document: &SourceDocument) -> (u64, u64) {
    version_tuple(
        document.identity.workspace_generation,
        document.identity.file_content_version,
    )
}

fn file_identity_version_tuple(identity: &SemanticFileFingerprintIdentity) -> (u64, u64) {
    version_tuple(identity.workspace_generation, identity.file_content_version)
}

/// In-process actor/state machine for bounded semantic work scheduling.
#[derive(Debug)]
pub struct IndexingActor {
    capacity: usize,
    next_work_id: u64,
    pending: VecDeque<IndexWorkItem>,
    in_flight: HashMap<u64, IndexWorkItem>,
    cancelled_tokens: HashMap<CancellationTokenId, SemanticCancellationReason>,
    latest_by_file: HashMap<(WorkspaceId, FileId), LatestWorkIdentity>,
    parser_cache: SyntaxTreeCache,
    parser: LexicalFallbackParser,
    index: SemanticIndex,
}

impl IndexingActor {
    /// Constructs an actor with a bounded pending queue.
    pub fn new(capacity: usize) -> Self {
        Self {
            capacity,
            next_work_id: 1,
            pending: VecDeque::new(),
            in_flight: HashMap::new(),
            cancelled_tokens: HashMap::new(),
            latest_by_file: HashMap::new(),
            parser_cache: SyntaxTreeCache::new(),
            parser: LexicalFallbackParser::new(),
            index: SemanticIndex::new(),
        }
    }

    /// Returns the configured queue capacity.
    pub const fn capacity(&self) -> usize {
        self.capacity
    }

    /// Returns the number of queued work items.
    pub fn pending_len(&self) -> usize {
        self.pending.len()
    }

    /// Returns the number of in-flight work items.
    pub fn in_flight_len(&self) -> usize {
        self.in_flight.len()
    }

    /// Returns an immutable view of the actor-owned semantic index.
    pub const fn index(&self) -> &SemanticIndex {
        &self.index
    }

    /// Returns a mutable view of the actor-owned semantic index for fixtures and adapters.
    pub fn index_mut(&mut self) -> &mut SemanticIndex {
        &mut self.index
    }

    /// Submits work into the bounded actor queue, applying pressure and supersession policy first.
    pub fn submit(&mut self, mut item: IndexWorkItem) -> IndexResult<IndexSubmitOutcome> {
        let mut cancellations = self.cancel_obsolete_for_new_work(&item);

        if self.capacity == 0 {
            return Err(IndexError::QueueBackpressure {
                capacity: self.capacity,
                pending_len: self.pending.len(),
                priority: item.priority,
            });
        }

        if self.pending.len() >= self.capacity {
            let Some(lowest_index) = self.lowest_priority_pending_index() else {
                return Err(IndexError::QueueBackpressure {
                    capacity: self.capacity,
                    pending_len: self.pending.len(),
                    priority: item.priority,
                });
            };

            let lowest_priority = self.pending[lowest_index].priority;
            if item.priority <= lowest_priority {
                return Err(IndexError::QueueBackpressure {
                    capacity: self.capacity,
                    pending_len: self.pending.len(),
                    priority: item.priority,
                });
            }

            if let Some(removed) = self.pending.remove(lowest_index) {
                cancellations.push(self.ack_for(
                    &removed,
                    SemanticCancellationReason::QueuePressure,
                    false,
                    true,
                ));
            }
        }

        let work_id = self.next_work_id;
        self.next_work_id = self.next_work_id.saturating_add(1);
        item.work_id = work_id;

        self.record_latest(&item);
        self.pending.push_back(item);

        Ok(IndexSubmitOutcome {
            accepted_work_id: work_id,
            cancellations,
            pending_len: self.pending.len(),
        })
    }

    /// Cancels queued and in-flight work by token identifier.
    pub fn cancel(
        &mut self,
        token_id: CancellationTokenId,
        reason: SemanticCancellationReason,
    ) -> Option<SemanticCancellationAck> {
        let mut removed_from_queue = false;
        self.pending.retain(|item| {
            if item.cancellation.token_id == token_id {
                removed_from_queue = true;
                false
            } else {
                true
            }
        });

        let was_in_flight = self
            .in_flight
            .values()
            .any(|item| item.cancellation.token_id == token_id);

        if removed_from_queue || was_in_flight {
            self.cancelled_tokens.insert(token_id, reason);
            Some(SemanticCancellationAck {
                token_id,
                reason,
                acknowledged_at: TimestampMillis::now(),
                was_in_flight,
                removed_from_queue,
            })
        } else {
            None
        }
    }

    /// Starts the highest-priority queued item without completing it.
    pub fn start_next(&mut self) -> Option<StartedIndexWork> {
        let best_index = self.highest_priority_pending_index()?;
        let item = self.pending.remove(best_index)?;
        let started = StartedIndexWork {
            item: item.clone(),
            started_at: TimestampMillis::now(),
        };
        self.in_flight.insert(item.work_id, item);
        Some(started)
    }

    /// Completes previously started work, ignoring obsolete results by generation/hash semantics.
    pub fn complete_started(&mut self, started: StartedIndexWork) -> IndexResult<IndexWorkReport> {
        let Some(item) = self.in_flight.remove(&started.item.work_id) else {
            return Err(IndexError::WorkNotInFlight {
                work_id: started.item.work_id,
            });
        };

        if let Some(reason) = self.cancelled_tokens.remove(&item.cancellation.token_id) {
            let ack = SemanticCancellationAck {
                token_id: item.cancellation.token_id,
                reason,
                acknowledged_at: TimestampMillis::now(),
                was_in_flight: true,
                removed_from_queue: false,
            };
            return Ok(IndexWorkReport {
                work_id: item.work_id,
                state: WorkCompletionState::Cancelled,
                cancellation_ack: Some(ack),
                upsert: None,
                diagnostics: vec![diagnostic(
                    "index.work.cancelled",
                    "semantic work observed cancellation before applying records",
                    ProtocolDiagnosticSeverity::Info,
                    None,
                    None,
                )],
            });
        }

        if self.is_item_obsolete(&item) {
            return Ok(IndexWorkReport {
                work_id: item.work_id,
                state: WorkCompletionState::IgnoredObsolete,
                cancellation_ack: None,
                upsert: None,
                diagnostics: vec![diagnostic(
                    "index.work.obsolete",
                    "semantic work completed after a newer identity superseded it",
                    ProtocolDiagnosticSeverity::Info,
                    item.document
                        .as_ref()
                        .map(|document| document.identity.canonical_path.clone()),
                    None,
                )],
            });
        }

        let Some(document) = item.document.clone() else {
            return Ok(IndexWorkReport {
                work_id: item.work_id,
                state: WorkCompletionState::Applied,
                cancellation_ack: None,
                upsert: None,
                diagnostics: vec![diagnostic(
                    "index.work.no_document",
                    "semantic work did not carry a source document",
                    ProtocolDiagnosticSeverity::Info,
                    None,
                    None,
                )],
            });
        };

        let request = ParseRequest {
            document,
            grammar_version: SemanticGrammarVersion(DEFAULT_GRAMMAR_VERSION.to_string()),
            model_version: SemanticModelVersion(DEFAULT_MODEL_VERSION.to_string()),
        };
        let outcome = self.parser_cache.get_or_parse(&self.parser, request)?;
        let upsert = self.index.upsert(outcome.file_index.clone());
        let mut diagnostics = outcome.diagnostics.clone();
        diagnostics.push(diagnostic(
            "index.work.applied",
            "semantic work produced proposal-free index records",
            ProtocolDiagnosticSeverity::Info,
            Some(outcome.file_index.identity.canonical_path.clone()),
            None,
        ));

        Ok(IndexWorkReport {
            work_id: item.work_id,
            state: match upsert {
                SemanticUpsertOutcome::Applied | SemanticUpsertOutcome::Replaced => {
                    WorkCompletionState::Applied
                }
                SemanticUpsertOutcome::IgnoredStale => WorkCompletionState::IgnoredObsolete,
            },
            cancellation_ack: None,
            upsert: Some(upsert),
            diagnostics,
        })
    }

    /// Starts and completes the next queued item in a single non-threaded actor step.
    pub fn execute_next(&mut self) -> IndexResult<Option<IndexWorkReport>> {
        let Some(started) = self.start_next() else {
            return Ok(None);
        };
        self.complete_started(started).map(Some)
    }

    /// Returns queued cancellation tokens in the actor's current pending order.
    pub fn pending_tokens(&self) -> Vec<CancellationTokenId> {
        self.pending
            .iter()
            .map(|item| item.cancellation.token_id)
            .collect()
    }

    fn highest_priority_pending_index(&self) -> Option<usize> {
        self.pending
            .iter()
            .enumerate()
            .max_by(|(_, left), (_, right)| {
                left.priority
                    .cmp(&right.priority)
                    .then_with(|| right.work_id.cmp(&left.work_id))
            })
            .map(|(index, _)| index)
    }

    fn lowest_priority_pending_index(&self) -> Option<usize> {
        self.pending
            .iter()
            .enumerate()
            .min_by(|(_, left), (_, right)| {
                left.priority
                    .cmp(&right.priority)
                    .then_with(|| right.work_id.cmp(&left.work_id))
            })
            .map(|(index, _)| index)
    }

    fn ack_for(
        &mut self,
        item: &IndexWorkItem,
        reason: SemanticCancellationReason,
        was_in_flight: bool,
        removed_from_queue: bool,
    ) -> SemanticCancellationAck {
        self.cancelled_tokens
            .insert(item.cancellation.token_id, reason);
        SemanticCancellationAck {
            token_id: item.cancellation.token_id,
            reason,
            acknowledged_at: TimestampMillis::now(),
            was_in_flight,
            removed_from_queue,
        }
    }

    fn cancel_obsolete_for_new_work(
        &mut self,
        new_item: &IndexWorkItem,
    ) -> Vec<SemanticCancellationAck> {
        let mut acknowledgements = Vec::new();
        let Some(new_document) = new_item.document.as_ref() else {
            return acknowledgements;
        };
        let key = (
            new_document.identity.workspace_id,
            new_document.identity.file_id,
        );

        let mut index = 0;
        while index < self.pending.len() {
            let should_remove = self.pending.get(index).is_some_and(|old_item| {
                old_item.document.as_ref().is_some_and(|old_document| {
                    (
                        old_document.identity.workspace_id,
                        old_document.identity.file_id,
                    ) == key
                        && document_supersedes(
                            new_document,
                            new_item.priority,
                            old_document,
                            old_item.priority,
                        )
                })
            });

            if should_remove {
                if let Some(removed) = self.pending.remove(index) {
                    acknowledgements.push(self.ack_for(
                        &removed,
                        SemanticCancellationReason::SnapshotSuperseded,
                        false,
                        true,
                    ));
                }
            } else {
                index += 1;
            }
        }

        let in_flight_to_cancel = self
            .in_flight
            .values()
            .filter(|old_item| {
                old_item.document.as_ref().is_some_and(|old_document| {
                    (
                        old_document.identity.workspace_id,
                        old_document.identity.file_id,
                    ) == key
                        && document_supersedes(
                            new_document,
                            new_item.priority,
                            old_document,
                            old_item.priority,
                        )
                })
            })
            .cloned()
            .collect::<Vec<_>>();

        for old_item in in_flight_to_cancel {
            acknowledgements.push(self.ack_for(
                &old_item,
                SemanticCancellationReason::SnapshotSuperseded,
                true,
                false,
            ));
        }

        acknowledgements
    }

    fn record_latest(&mut self, item: &IndexWorkItem) {
        let Some(document) = item.document.as_ref() else {
            return;
        };
        let key = (document.identity.workspace_id, document.identity.file_id);
        let identity = LatestWorkIdentity {
            priority: item.priority,
            workspace_generation: document.identity.workspace_generation,
            file_content_version: document.identity.file_content_version,
            content_hash: document.identity.content_hash.clone(),
        };

        let should_replace = self.latest_by_file.get(&key).is_none_or(|current| {
            let incoming_version = latest_work_version_tuple(&identity);
            let current_version = latest_work_version_tuple(current);

            identity.priority > current.priority
                || incoming_version > current_version
                || (incoming_version == current_version
                    && identity.content_hash != current.content_hash)
        });

        if should_replace {
            self.latest_by_file.insert(key, identity);
        }
    }

    fn is_item_obsolete(&self, item: &IndexWorkItem) -> bool {
        let Some(document) = item.document.as_ref() else {
            return false;
        };
        let key = (document.identity.workspace_id, document.identity.file_id);
        self.latest_by_file.get(&key).is_some_and(|latest| {
            let latest_version = latest_work_version_tuple(latest);
            let document_version = source_document_version_tuple(document);

            latest_version > document_version
                || (latest_version == document_version
                    && latest.content_hash != document.identity.content_hash)
        })
    }
}

impl SemanticPort for IndexingActor {
    fn handle(&self, request: SemanticRequest) -> ProtocolResult<SemanticResponse> {
        match request {
            SemanticRequest::PlanJobs {
                requests,
                correlation_id,
                causality_id,
            } => {
                let workspace_generation = requests
                    .first()
                    .map(|request| request.file_identity.workspace_generation)
                    .unwrap_or(WorkspaceGeneration(0));
                let privacy_scope = requests
                    .first()
                    .map(|request| request.privacy.privacy_scope)
                    .unwrap_or(SemanticPrivacyScope::Workspace);
                let scheduler = SemanticFabricScheduler::new(SemanticFabricSchedulingPolicy::new(
                    workspace_generation,
                    privacy_scope,
                    self.capacity as u32,
                ));

                Ok(SemanticResponse::SchedulePlan(scheduler.plan(
                    requests,
                    correlation_id,
                    causality_id,
                )))
            }
            SemanticRequest::Query(request) => {
                Ok(SemanticResponse::Query(self.index.query(&request)))
            }
            SemanticRequest::Cancel(token) => Ok(SemanticResponse::Cancelled(token)),
        }
    }
}

fn document_supersedes(
    new_document: &SourceDocument,
    new_priority: WorkPriority,
    old_document: &SourceDocument,
    old_priority: WorkPriority,
) -> bool {
    let new_version = source_document_version_tuple(new_document);
    let old_version = source_document_version_tuple(old_document);

    new_priority > old_priority
        || new_version > old_version
        || (new_version == old_version
            && new_document.identity.content_hash != old_document.identity.content_hash)
}

/// Index-facing record imported from workspace-authored discovery metadata.
#[derive(Debug, Clone)]
pub struct RepositoryDiscoveryImportRecord {
    /// Semantic identity when content indexing is allowed.
    pub identity: Option<SemanticFileFingerprintIdentity>,
    /// Source workspace discovery record.
    pub discovery: WorkspaceDiscoveryRecord,
}

/// Outcome of importing workspace-authored discovery metadata.
#[derive(Debug, Clone, Default)]
pub struct RepositoryDiscoveryImportOutcome {
    /// Records eligible for later descriptor or lease-based content processing.
    pub content_records: Vec<RepositoryDiscoveryImportRecord>,
    /// Records retained only as safe metadata.
    pub metadata_only_records: Vec<RepositoryDiscoveryImportRecord>,
    /// Records excluded from indexing.
    pub excluded_records: Vec<RepositoryDiscoveryImportRecord>,
    /// File identities invalidated by deleted or excluded discovery records.
    pub invalidated_file_ids: Vec<FileId>,
    /// Metadata-only diagnostics forwarded from workspace discovery.
    pub diagnostics: Vec<ProtocolDiagnostic>,
}

/// Importer for workspace-authored semantic discovery DTOs.
#[derive(Debug, Default, Clone, Copy)]
pub struct RepositoryDiscoveryImporter;

impl RepositoryDiscoveryImporter {
    /// Constructs an importer that accepts only protocol discovery DTOs.
    pub const fn new() -> Self {
        Self
    }

    /// Ingests a workspace-authored discovery snapshot.
    pub fn ingest_snapshot(
        &self,
        snapshot: &WorkspaceDiscoverySnapshot,
    ) -> RepositoryDiscoveryImportOutcome {
        let mut outcome = RepositoryDiscoveryImportOutcome {
            diagnostics: snapshot.diagnostics.clone(),
            ..RepositoryDiscoveryImportOutcome::default()
        };
        for record in &snapshot.records {
            self.ingest_record(record, &mut outcome);
        }
        outcome
    }

    /// Ingests a workspace-authored discovery delta.
    pub fn ingest_delta(
        &self,
        delta: &WorkspaceDiscoveryDelta,
    ) -> RepositoryDiscoveryImportOutcome {
        let mut outcome = RepositoryDiscoveryImportOutcome {
            diagnostics: delta.diagnostics.clone(),
            ..RepositoryDiscoveryImportOutcome::default()
        };
        for record in &delta.records {
            self.ingest_record(record, &mut outcome);
        }
        outcome
    }

    fn ingest_record(
        &self,
        record: &WorkspaceDiscoveryRecord,
        outcome: &mut RepositoryDiscoveryImportOutcome,
    ) {
        let imported = RepositoryDiscoveryImportRecord {
            identity: semantic_identity_from_discovery(record),
            discovery: record.clone(),
        };
        if record.change_kind == Some(WorkspaceDiscoveryChangeKind::Deleted)
            || record.policy.skip_reason == Some(WorkspaceDiscoverySkipReason::Deleted)
        {
            if let Some(file_id) = discovery_file_id(record) {
                outcome.invalidated_file_ids.push(file_id);
            }
            outcome.excluded_records.push(imported);
            return;
        }

        match record.policy.decision {
            WorkspaceDiscoveryDecision::ContentAllowed if imported.identity.is_some() => {
                outcome.content_records.push(imported)
            }
            WorkspaceDiscoveryDecision::ContentAllowed
            | WorkspaceDiscoveryDecision::MetadataOnly => {
                outcome.metadata_only_records.push(imported)
            }
            WorkspaceDiscoveryDecision::Excluded => {
                if let Some(file_id) = discovery_file_id(record) {
                    outcome.invalidated_file_ids.push(file_id);
                }
                outcome.excluded_records.push(imported);
            }
        }
    }
}

/// Pure metadata-only policy used by the actor-owned semantic fabric scheduler.
#[derive(Debug, Clone)]
pub struct SemanticFabricSchedulingPolicy {
    /// Current workspace generation accepted by the actor.
    pub workspace_generation: WorkspaceGeneration,
    /// Current maximum privacy scope accepted by the actor.
    pub privacy_scope: SemanticPrivacyScope,
    /// Current grammar version for parser-derived work.
    pub grammar_version: SemanticGrammarVersion,
    /// Current model metadata version for ranked or enriched work.
    pub model_version: SemanticModelVersion,
    /// Current parser or extraction contract version.
    pub parser_version: String,
    /// Current scheduling and persistence schema version.
    pub schema_version: u16,
    /// Bounded queue capacity applied during planning.
    pub queue_capacity: u32,
}

impl SemanticFabricSchedulingPolicy {
    /// Constructs a scheduling policy for the current workspace and semantic versions.
    pub fn new(
        workspace_generation: WorkspaceGeneration,
        privacy_scope: SemanticPrivacyScope,
        queue_capacity: u32,
    ) -> Self {
        Self {
            workspace_generation,
            privacy_scope,
            grammar_version: SemanticGrammarVersion(DEFAULT_GRAMMAR_VERSION.to_string()),
            model_version: SemanticModelVersion(DEFAULT_MODEL_VERSION.to_string()),
            parser_version: LEXICAL_EXTRACTION_VERSION.to_string(),
            schema_version: INDEX_SCHEMA_VERSION,
            queue_capacity,
        }
    }
}

/// Actor-owned deterministic semantic fabric scheduler contract.
#[derive(Debug, Clone)]
pub struct SemanticFabricScheduler {
    policy: SemanticFabricSchedulingPolicy,
}

impl SemanticFabricScheduler {
    /// Constructs a pure scheduler that produces metadata-only decisions without runtime activation.
    pub const fn new(policy: SemanticFabricSchedulingPolicy) -> Self {
        Self { policy }
    }

    /// Returns the scheduling policy used by this scheduler.
    pub const fn policy(&self) -> &SemanticFabricSchedulingPolicy {
        &self.policy
    }

    /// Builds a metadata-only scheduling request from workspace-authored discovery metadata.
    pub fn request_from_discovery_record(
        &self,
        record: &RepositoryDiscoveryImportRecord,
        trigger: SemanticFabricSchedulingTrigger,
        persisted: Option<&SemanticMetadataRecord>,
        cancellation: SemanticCancellationToken,
        correlation_id: devil_protocol::CorrelationId,
        causality_id: devil_protocol::CausalityId,
    ) -> Option<SemanticFabricJobRequest> {
        let identity = record.identity.clone()?;
        let language_id = record.discovery.language_hint.clone()?;
        let descriptor = empty_descriptor_reference(
            metadata_source_kind(SemanticSourceInputKind::DescriptorOnly),
            None,
            &identity,
        );
        let expected_freshness_key = freshness_key_from_parts(
            &identity,
            &language_id,
            None,
            descriptor_identity_from_reference(&descriptor),
            &self.policy.grammar_version,
            &self.policy.model_version,
            &self.policy.parser_version,
            self.policy.schema_version,
        );
        Some(SemanticFabricJobRequest {
            job_id: semantic_fabric_job_id(
                SemanticFabricWorkSourceKind::WorkspaceDiscovery,
                trigger,
                identity.workspace_id,
                identity.file_id,
                &identity.content_hash,
            ),
            source_kind: SemanticFabricWorkSourceKind::WorkspaceDiscovery,
            trigger,
            workspace_id: identity.workspace_id,
            file_id: identity.file_id,
            language_id,
            file_identity: identity.clone(),
            expected_freshness_key,
            persisted_freshness_key: persisted.map(|record| record.freshness_key.clone()),
            descriptor,
            privacy: privacy_label(identity.privacy_scope, true),
            dependency_hints: Vec::new(),
            cancellation,
            correlation_id,
            causality_id,
            schema_version: self.policy.schema_version,
        })
    }

    /// Builds a metadata-only scheduling request from descriptor-first source metadata.
    pub fn request_from_source_document(
        &self,
        document: &SourceDocument,
        trigger: SemanticFabricSchedulingTrigger,
        persisted: Option<&SemanticMetadataRecord>,
        cancellation: SemanticCancellationToken,
        correlation_id: devil_protocol::CorrelationId,
        causality_id: devil_protocol::CausalityId,
    ) -> SemanticFabricJobRequest {
        let descriptor = descriptor_reference_from_document(document);
        let expected_freshness_key = freshness_key_from_parts(
            &document.identity,
            &document.language_id,
            document.snapshot_id,
            descriptor_identity_from_reference(&descriptor),
            &self.policy.grammar_version,
            &self.policy.model_version,
            &self.policy.parser_version,
            self.policy.schema_version,
        );
        SemanticFabricJobRequest {
            job_id: semantic_fabric_job_id(
                source_kind_from_document(document),
                trigger,
                document.identity.workspace_id,
                document.identity.file_id,
                &document.identity.content_hash,
            ),
            source_kind: source_kind_from_document(document),
            trigger,
            workspace_id: document.identity.workspace_id,
            file_id: document.identity.file_id,
            language_id: document.language_id.clone(),
            file_identity: document.identity.clone(),
            expected_freshness_key,
            persisted_freshness_key: persisted.map(|record| record.freshness_key.clone()),
            descriptor,
            privacy: privacy_label(document.identity.privacy_scope, true),
            dependency_hints: Vec::new(),
            cancellation,
            correlation_id,
            causality_id,
            schema_version: self.policy.schema_version,
        }
    }

    /// Builds a metadata-only scheduling request from an existing persisted semantic record.
    pub fn request_from_metadata_record(
        &self,
        record: &SemanticMetadataRecord,
        trigger: SemanticFabricSchedulingTrigger,
        cancellation: SemanticCancellationToken,
        correlation_id: devil_protocol::CorrelationId,
        causality_id: devil_protocol::CausalityId,
    ) -> SemanticFabricJobRequest {
        let descriptor = descriptor_reference_from_identity(&record.freshness_key.descriptor);
        SemanticFabricJobRequest {
            job_id: semantic_fabric_job_id(
                SemanticFabricWorkSourceKind::SemanticPersistence,
                trigger,
                record.workspace_id,
                record.file_id,
                &record.freshness_key.content_hash,
            ),
            source_kind: SemanticFabricWorkSourceKind::SemanticPersistence,
            trigger,
            workspace_id: record.workspace_id,
            file_id: record.file_id,
            language_id: record.language_id.clone(),
            file_identity: record.file_identity.clone(),
            expected_freshness_key: record.freshness_key.clone(),
            persisted_freshness_key: Some(record.freshness_key.clone()),
            descriptor,
            privacy: privacy_label(record.freshness_key.privacy_scope, true),
            dependency_hints: Vec::new(),
            cancellation,
            correlation_id,
            causality_id,
            schema_version: self.policy.schema_version,
        }
    }

    /// Builds a metadata-only semantic refresh request from normalized LSP diagnostic metadata.
    ///
    /// This helper connects the future LSP supervision DTO boundary to semantic fabric scheduling
    /// without authorizing any LSP runtime activation, process launch, I/O loop, or mutation path.
    /// The returned request stores only the workspace-authored file identity, snapshot identity,
    /// content hash, ranges, diagnostic hashes already present in the summary, and freshness keys.
    #[allow(clippy::too_many_arguments)]
    pub fn request_from_lsp_diagnostic_summary(
        &self,
        summary: &LspDiagnosticSummary,
        language_id: LanguageId,
        file_identity: &SemanticFileFingerprintIdentity,
        persisted: Option<&SemanticMetadataRecord>,
        cancellation: SemanticCancellationToken,
        correlation_id: devil_protocol::CorrelationId,
        causality_id: devil_protocol::CausalityId,
    ) -> Option<SemanticFabricJobRequest> {
        if summary.workspace_id != file_identity.workspace_id
            || summary.file_id != file_identity.file_id
        {
            return None;
        }
        if let Some(content_hash) = &summary.content_hash
            && content_hash != &file_identity.content_hash
        {
            return None;
        }

        let mut identity = file_identity.clone();
        identity.privacy_scope = summary.privacy_scope;
        let descriptor = empty_descriptor_reference(
            metadata_source_kind(SemanticSourceInputKind::DescriptorOnly),
            Some(summary.snapshot_id),
            &identity,
        );
        let expected_freshness_key = freshness_key_from_parts(
            &identity,
            &language_id,
            Some(summary.snapshot_id),
            descriptor_identity_from_reference(&descriptor),
            &self.policy.grammar_version,
            &self.policy.model_version,
            &self.policy.parser_version,
            self.policy.schema_version,
        );

        Some(SemanticFabricJobRequest {
            job_id: semantic_fabric_job_id(
                SemanticFabricWorkSourceKind::LspDtoMetadata,
                SemanticFabricSchedulingTrigger::LspEnrichment,
                identity.workspace_id,
                identity.file_id,
                &identity.content_hash,
            ),
            source_kind: SemanticFabricWorkSourceKind::LspDtoMetadata,
            trigger: SemanticFabricSchedulingTrigger::LspEnrichment,
            workspace_id: identity.workspace_id,
            file_id: identity.file_id,
            language_id,
            file_identity: identity,
            expected_freshness_key,
            persisted_freshness_key: persisted.map(|record| record.freshness_key.clone()),
            descriptor,
            privacy: privacy_label(summary.privacy_scope, true),
            dependency_hints: Vec::new(),
            cancellation,
            correlation_id,
            causality_id,
            schema_version: self.policy.schema_version,
        })
    }

    /// Plans a batch of semantic jobs without starting workers, threads, LSP processes, or providers.
    pub fn plan(
        &self,
        requests: impl IntoIterator<Item = SemanticFabricJobRequest>,
        correlation_id: devil_protocol::CorrelationId,
        causality_id: devil_protocol::CausalityId,
    ) -> SemanticFabricSchedulePlan {
        let mut decisions = requests
            .into_iter()
            .map(|request| self.classify(request))
            .collect::<Vec<_>>();

        decisions.sort_by(decision_order);

        let mut admitted_count = 0_u32;
        for decision in &mut decisions {
            if admits_queue_slot(decision.action) {
                if admitted_count < self.policy.queue_capacity {
                    admitted_count = admitted_count.saturating_add(1);
                    decision.queue_depth = admitted_count;
                } else {
                    decision.action = SemanticFabricSchedulingAction::Reject;
                    decision.freshness_state = SemanticFreshnessState::Unavailable;
                    decision
                        .invalidation_causes
                        .push(SemanticFabricInvalidationCause::QueuePressure);
                    decision.cancellation_reason = Some(SemanticCancellationReason::QueuePressure);
                    decision.queue_depth = admitted_count;
                    decision.diagnostics.push(diagnostic(
                        "semantic.fabric.queue_pressure",
                        "semantic scheduling rejected lower-priority metadata-only work under bounded capacity",
                        ProtocolDiagnosticSeverity::Info,
                        None,
                        None,
                    ));
                }
            } else {
                decision.queue_depth = admitted_count;
            }
        }

        SemanticFabricSchedulePlan {
            decisions,
            admitted_count,
            capacity: self.policy.queue_capacity,
            correlation_id,
            causality_id,
            schema_version: self.policy.schema_version,
        }
    }

    fn classify(&self, request: SemanticFabricJobRequest) -> SemanticFabricSchedulingDecision {
        let (priority, priority_score) = priority_for(&request);
        let mut invalidation_causes = freshness_mismatches(
            request.persisted_freshness_key.as_ref(),
            &request.expected_freshness_key,
            &self.policy,
        );
        if request.file_identity.workspace_generation != self.policy.workspace_generation
            && !invalidation_causes
                .contains(&SemanticFabricInvalidationCause::WorkspaceGenerationChanged)
        {
            invalidation_causes.push(SemanticFabricInvalidationCause::WorkspaceGenerationChanged);
        }
        if !privacy_scope_admitted(request.privacy.privacy_scope, self.policy.privacy_scope)
            && !invalidation_causes.contains(&SemanticFabricInvalidationCause::PrivacyScopeChanged)
        {
            invalidation_causes.push(SemanticFabricInvalidationCause::PrivacyScopeChanged);
        }

        let deleted = request.source_kind == SemanticFabricWorkSourceKind::WorkspaceDiscovery
            && request.expected_freshness_key.content_hash.value.is_empty();
        if deleted {
            invalidation_causes.push(SemanticFabricInvalidationCause::DiscoveryDeleted);
        }

        let action = action_for_causes(&invalidation_causes);
        let cancellation_reason = cancellation_for_action(action, &invalidation_causes);
        let freshness_state = freshness_for_action(action);
        let diagnostics = decision_diagnostics(action, &invalidation_causes);

        SemanticFabricSchedulingDecision {
            job_id: request.job_id,
            action,
            priority,
            priority_score,
            freshness_state,
            invalidation_causes,
            cancellation_reason,
            metadata_only: request.privacy.metadata_only,
            queue_depth: 0,
            diagnostics,
            schema_version: self.policy.schema_version,
        }
    }
}

fn descriptor_reference_from_identity(
    descriptor: &SemanticMetadataDescriptorIdentity,
) -> SemanticFabricDescriptorReference {
    SemanticFabricDescriptorReference {
        source_kind: descriptor.source_kind,
        snapshot_id: descriptor.snapshot_id,
        content_hash: descriptor.content_hash.clone(),
        byte_len: descriptor.byte_len,
        ranges: descriptor.ranges.clone(),
        chunks: descriptor.chunks.clone(),
        schema_version: descriptor.schema_version,
    }
}

fn descriptor_identity_from_reference(
    descriptor: &SemanticFabricDescriptorReference,
) -> SemanticMetadataDescriptorIdentity {
    SemanticMetadataDescriptorIdentity {
        source_kind: descriptor.source_kind,
        snapshot_id: descriptor.snapshot_id,
        content_hash: descriptor.content_hash.clone(),
        byte_len: descriptor.byte_len,
        ranges: descriptor.ranges.clone(),
        chunks: descriptor.chunks.clone(),
        schema_version: descriptor.schema_version,
    }
}

fn descriptor_reference_from_document(
    document: &SourceDocument,
) -> SemanticFabricDescriptorReference {
    let descriptor = document.source_descriptor();
    SemanticFabricDescriptorReference {
        source_kind: metadata_source_kind(document.source_kind()),
        snapshot_id: document.snapshot_id,
        content_hash: document.identity.content_hash.clone(),
        byte_len: document.identity.byte_len,
        ranges: descriptor.ranges.clone(),
        chunks: descriptor
            .chunks
            .iter()
            .map(metadata_chunk_reference)
            .collect(),
        schema_version: descriptor
            .chunks
            .iter()
            .map(|chunk| chunk.schema_version)
            .chain(descriptor.leases.iter().map(|lease| lease.schema_version))
            .max()
            .unwrap_or(INDEX_SCHEMA_VERSION),
    }
}

fn empty_descriptor_reference(
    source_kind: SemanticMetadataSourceKind,
    snapshot_id: Option<SnapshotId>,
    identity: &SemanticFileFingerprintIdentity,
) -> SemanticFabricDescriptorReference {
    SemanticFabricDescriptorReference {
        source_kind,
        snapshot_id,
        content_hash: identity.content_hash.clone(),
        byte_len: identity.byte_len,
        ranges: Vec::new(),
        chunks: Vec::new(),
        schema_version: identity.schema_version,
    }
}

fn metadata_chunk_reference(
    chunk: &SemanticSourceChunkReference,
) -> SemanticMetadataChunkReference {
    SemanticMetadataChunkReference {
        snapshot_id: chunk.snapshot_id,
        chunk_index: chunk.chunk_index,
        byte_range: chunk.byte_range,
        line_range: chunk.line_range,
        byte_len: chunk.byte_len,
        chunk_hash: chunk.chunk_hash.clone(),
        lease_present: chunk.lease_id.is_some(),
        schema_version: chunk.schema_version,
    }
}

#[allow(clippy::too_many_arguments)]
fn freshness_key_from_parts(
    identity: &SemanticFileFingerprintIdentity,
    language_id: &LanguageId,
    snapshot_id: Option<SnapshotId>,
    descriptor: SemanticMetadataDescriptorIdentity,
    grammar_version: &SemanticGrammarVersion,
    model_version: &SemanticModelVersion,
    parser_version: &str,
    schema_version: u16,
) -> SemanticMetadataFreshnessKey {
    SemanticMetadataFreshnessKey {
        workspace_id: identity.workspace_id,
        file_id: identity.file_id,
        language_id: language_id.clone(),
        snapshot_id,
        file_content_version: identity.file_content_version,
        workspace_generation: identity.workspace_generation,
        content_hash: identity.content_hash.clone(),
        grammar_version: Some(grammar_version.clone()),
        model_version: Some(model_version.clone()),
        parser_version: parser_version.to_string(),
        privacy_scope: identity.privacy_scope,
        descriptor,
        schema_version,
    }
}

fn privacy_label(
    privacy_scope: SemanticPrivacyScope,
    metadata_only: bool,
) -> SemanticFabricPrivacyLabel {
    SemanticFabricPrivacyLabel {
        privacy_scope,
        metadata_only,
        redaction: RedactionHint::MetadataOnly,
        schema_version: INDEX_SCHEMA_VERSION,
    }
}

fn source_kind_from_document(document: &SourceDocument) -> SemanticFabricWorkSourceKind {
    match document.source_kind() {
        SemanticSourceInputKind::DescriptorOnly | SemanticSourceInputKind::ChangedRanges => {
            SemanticFabricWorkSourceKind::SourceDescriptor
        }
        SemanticSourceInputKind::LeaseChunks => SemanticFabricWorkSourceKind::SnapshotLeaseMetadata,
        SemanticSourceInputKind::BoundedFullText => SemanticFabricWorkSourceKind::SourceDescriptor,
    }
}

fn semantic_fabric_job_id(
    source_kind: SemanticFabricWorkSourceKind,
    trigger: SemanticFabricSchedulingTrigger,
    workspace_id: WorkspaceId,
    file_id: FileId,
    content_hash: &FileFingerprint,
) -> String {
    format!(
        "semantic-fabric:{source_kind:?}:{trigger:?}:{}:{}:{}",
        workspace_id.0, file_id.0, content_hash.value
    )
}

fn priority_for(request: &SemanticFabricJobRequest) -> (SemanticFabricPriority, u16) {
    let (priority, base_score) = match request.trigger {
        SemanticFabricSchedulingTrigger::RecentEdit => (SemanticFabricPriority::RecentEdit, 1_000),
        SemanticFabricSchedulingTrigger::ForegroundViewport => {
            (SemanticFabricPriority::ForegroundViewport, 900)
        }
        SemanticFabricSchedulingTrigger::SaveAdjacent => {
            (SemanticFabricPriority::SaveAdjacent, 800)
        }
        SemanticFabricSchedulingTrigger::DependencyHint => {
            (SemanticFabricPriority::DependencyHint, 700)
        }
        SemanticFabricSchedulingTrigger::LspEnrichment => {
            (SemanticFabricPriority::LspEnrichment, 600)
        }
        SemanticFabricSchedulingTrigger::WorkspaceDiscovery => {
            (SemanticFabricPriority::WorkspaceDiscovery, 500)
        }
        SemanticFabricSchedulingTrigger::BackgroundCrawl => {
            (SemanticFabricPriority::BackgroundCrawl, 100)
        }
        SemanticFabricSchedulingTrigger::Maintenance => (SemanticFabricPriority::Maintenance, 50),
    };
    let dependency_bonus = if request.dependency_hints.is_empty() {
        0
    } else {
        25
    };
    (priority, base_score + dependency_bonus)
}

fn freshness_mismatches(
    persisted: Option<&SemanticMetadataFreshnessKey>,
    expected: &SemanticMetadataFreshnessKey,
    policy: &SemanticFabricSchedulingPolicy,
) -> Vec<SemanticFabricInvalidationCause> {
    let Some(persisted) = persisted else {
        return vec![SemanticFabricInvalidationCause::MetadataMissing];
    };
    let mut causes = Vec::new();
    if persisted.privacy_scope != expected.privacy_scope
        || expected.privacy_scope != policy.privacy_scope
    {
        causes.push(SemanticFabricInvalidationCause::PrivacyScopeChanged);
    }
    if persisted.workspace_generation != expected.workspace_generation
        || expected.workspace_generation != policy.workspace_generation
    {
        causes.push(SemanticFabricInvalidationCause::WorkspaceGenerationChanged);
    }
    if persisted.schema_version != expected.schema_version
        || expected.schema_version != policy.schema_version
    {
        causes.push(SemanticFabricInvalidationCause::SchemaVersionChanged);
    }
    if persisted.parser_version != expected.parser_version
        || expected.parser_version != policy.parser_version
    {
        causes.push(SemanticFabricInvalidationCause::ParserVersionChanged);
    }
    if persisted.grammar_version != expected.grammar_version
        || expected.grammar_version.as_ref() != Some(&policy.grammar_version)
    {
        causes.push(SemanticFabricInvalidationCause::GrammarVersionChanged);
    }
    if persisted.model_version != expected.model_version
        || expected.model_version.as_ref() != Some(&policy.model_version)
    {
        causes.push(SemanticFabricInvalidationCause::ModelVersionChanged);
    }
    if persisted.language_id != expected.language_id {
        causes.push(SemanticFabricInvalidationCause::LanguageChanged);
    }
    if persisted.descriptor != expected.descriptor {
        causes.push(SemanticFabricInvalidationCause::DescriptorIdentityChanged);
    }
    if persisted.content_hash != expected.content_hash {
        causes.push(SemanticFabricInvalidationCause::ContentHashChanged);
    }
    causes
}

fn privacy_scope_admitted(requested: SemanticPrivacyScope, policy: SemanticPrivacyScope) -> bool {
    requested == policy || requested == SemanticPrivacyScope::MetadataOnly
}

fn action_for_causes(causes: &[SemanticFabricInvalidationCause]) -> SemanticFabricSchedulingAction {
    if causes.is_empty() {
        return SemanticFabricSchedulingAction::Coalesce;
    }
    if causes == [SemanticFabricInvalidationCause::MetadataMissing] {
        return SemanticFabricSchedulingAction::Schedule;
    }
    if causes.iter().any(|cause| {
        matches!(
            cause,
            SemanticFabricInvalidationCause::PrivacyScopeChanged
                | SemanticFabricInvalidationCause::DiscoveryDeleted
                | SemanticFabricInvalidationCause::QueuePressure
        )
    }) {
        return SemanticFabricSchedulingAction::Reject;
    }
    if causes.iter().any(|cause| {
        matches!(
            cause,
            SemanticFabricInvalidationCause::WorkspaceGenerationChanged
                | SemanticFabricInvalidationCause::LanguageChanged
                | SemanticFabricInvalidationCause::DescriptorIdentityChanged
                | SemanticFabricInvalidationCause::ContentHashChanged
                | SemanticFabricInvalidationCause::MetadataMissing
                | SemanticFabricInvalidationCause::SnapshotSuperseded
        )
    }) {
        return SemanticFabricSchedulingAction::Reindex;
    }
    SemanticFabricSchedulingAction::Refresh
}

fn cancellation_for_action(
    action: SemanticFabricSchedulingAction,
    causes: &[SemanticFabricInvalidationCause],
) -> Option<SemanticCancellationReason> {
    if action != SemanticFabricSchedulingAction::Reject {
        return None;
    }
    if causes.contains(&SemanticFabricInvalidationCause::QueuePressure) {
        Some(SemanticCancellationReason::QueuePressure)
    } else if causes.contains(&SemanticFabricInvalidationCause::PrivacyScopeChanged) {
        Some(SemanticCancellationReason::PrivacyScopeReduced)
    } else if causes.contains(&SemanticFabricInvalidationCause::SnapshotSuperseded) {
        Some(SemanticCancellationReason::SnapshotSuperseded)
    } else {
        Some(SemanticCancellationReason::ContentHashMismatch)
    }
}

fn freshness_for_action(action: SemanticFabricSchedulingAction) -> SemanticFreshnessState {
    match action {
        SemanticFabricSchedulingAction::Coalesce => SemanticFreshnessState::Fresh,
        SemanticFabricSchedulingAction::Reject => SemanticFreshnessState::Unavailable,
        SemanticFabricSchedulingAction::Schedule
        | SemanticFabricSchedulingAction::Refresh
        | SemanticFabricSchedulingAction::Reindex => SemanticFreshnessState::Stale,
    }
}

fn decision_diagnostics(
    action: SemanticFabricSchedulingAction,
    causes: &[SemanticFabricInvalidationCause],
) -> Vec<ProtocolDiagnostic> {
    let code = match action {
        SemanticFabricSchedulingAction::Schedule => "semantic.fabric.schedule",
        SemanticFabricSchedulingAction::Refresh => "semantic.fabric.refresh",
        SemanticFabricSchedulingAction::Reindex => "semantic.fabric.reindex",
        SemanticFabricSchedulingAction::Coalesce => "semantic.fabric.coalesce",
        SemanticFabricSchedulingAction::Reject => "semantic.fabric.reject",
    };
    let message = if causes.is_empty() {
        "semantic fabric work coalesced with fresh metadata".to_string()
    } else {
        format!("semantic fabric scheduling action {action:?} from causes {causes:?}")
    };
    vec![diagnostic(
        code,
        &message,
        ProtocolDiagnosticSeverity::Info,
        None,
        None,
    )]
}

fn admits_queue_slot(action: SemanticFabricSchedulingAction) -> bool {
    matches!(
        action,
        SemanticFabricSchedulingAction::Schedule
            | SemanticFabricSchedulingAction::Refresh
            | SemanticFabricSchedulingAction::Reindex
    )
}

fn decision_order(
    left: &SemanticFabricSchedulingDecision,
    right: &SemanticFabricSchedulingDecision,
) -> std::cmp::Ordering {
    right
        .priority_score
        .cmp(&left.priority_score)
        .then_with(|| action_rank(left.action).cmp(&action_rank(right.action)))
        .then_with(|| left.job_id.cmp(&right.job_id))
}

fn action_rank(action: SemanticFabricSchedulingAction) -> u8 {
    match action {
        SemanticFabricSchedulingAction::Reindex => 0,
        SemanticFabricSchedulingAction::Schedule => 1,
        SemanticFabricSchedulingAction::Refresh => 2,
        SemanticFabricSchedulingAction::Coalesce => 3,
        SemanticFabricSchedulingAction::Reject => 4,
    }
}

/// Descriptor-first source input category used by semantic work.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SemanticSourceInputKind {
    /// Metadata and chunk descriptors only; no source payload is owned by the index.
    DescriptorOnly,
    /// Bounded chunk payloads read through snapshot leases.
    LeaseChunks,
    /// Changed ranges with descriptors and optional bounded chunk payloads.
    ChangedRanges,
    /// Explicit small-buffer full-text optimization guarded by size and policy caps.
    BoundedFullText,
}

/// Metadata-only reference to a source chunk used by semantic records.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SemanticSourceChunkReference {
    /// Snapshot identifier owning the chunk.
    pub snapshot_id: SnapshotId,
    /// Chunk ordinal in the snapshot.
    pub chunk_index: u32,
    /// Byte range covered by this chunk.
    pub byte_range: ByteRange,
    /// Line range covered by this chunk.
    pub line_range: LineIndexRange,
    /// Chunk byte length.
    pub byte_len: u64,
    /// Hash of the chunk contents.
    pub chunk_hash: FileFingerprint,
    /// Lease identifier when the chunk was read through a lease.
    pub lease_id: Option<uuid::Uuid>,
    /// Source descriptor schema version.
    pub schema_version: u16,
}

/// Descriptor-first source metadata shared by all semantic source input forms.
#[derive(Debug, Clone)]
pub struct SemanticSourceDescriptor {
    /// Optional whole-snapshot descriptor.
    pub snapshot: Option<SnapshotDescriptor>,
    /// Chunk references available for invalidation and provenance.
    pub chunks: Vec<SemanticSourceChunkReference>,
    /// Byte ranges represented by this input.
    pub ranges: Vec<ByteRange>,
    /// Lease descriptors authorizing any bounded chunk payloads.
    pub leases: Vec<SnapshotLeaseDescriptor>,
    /// Freshness state implied by the input coverage.
    pub freshness_state: SemanticFreshnessState,
    /// Metadata-only reasons for degraded or partial indexing.
    pub degraded_reasons: Vec<String>,
}

/// Bounded text for a single leased chunk. This is transient parser input, not durable source state.
#[derive(Debug, Clone)]
pub struct SemanticSourceChunkText {
    /// Lease descriptor authorizing the chunk read.
    pub lease: SnapshotLeaseDescriptor,
    /// Chunk descriptor for the bounded text.
    pub chunk: SnapshotChunkDescriptor,
    /// Bounded chunk text payload.
    pub text: String,
}

/// Explicit small-buffer full-text compatibility input.
#[derive(Debug, Clone)]
pub struct BoundedFullTextSource {
    /// Bounded text payload retained only by the work item.
    pub text: String,
    /// Maximum byte budget that allowed this optimization.
    pub byte_budget: usize,
    /// Metadata-only policy reason documenting why full text was allowed.
    pub policy_reason: String,
}

/// Descriptor-first semantic source input. Full text is only the explicit bounded optimization.
#[derive(Debug, Clone)]
pub enum SemanticSourceInput {
    /// Descriptor-only source metadata with no text payload.
    DescriptorOnly(SemanticSourceDescriptor),
    /// Bounded chunks read through snapshot leases.
    LeaseChunks {
        /// Descriptor metadata for the chunk batch.
        descriptor: SemanticSourceDescriptor,
        /// Transient bounded chunk payloads.
        chunks: Vec<SemanticSourceChunkText>,
    },
    /// Changed range input with descriptors and optional bounded chunks.
    ChangedRanges {
        /// Descriptor metadata for the changed ranges.
        descriptor: SemanticSourceDescriptor,
        /// Changed text ranges represented by this input.
        changed_ranges: Vec<ProtocolTextRange>,
        /// Transient bounded chunk payloads covering changed ranges.
        chunks: Vec<SemanticSourceChunkText>,
    },
    /// Explicit small-buffer full-text optimization.
    BoundedFullText {
        /// Descriptor metadata for the bounded source.
        descriptor: SemanticSourceDescriptor,
        /// Bounded full-text payload.
        text: BoundedFullTextSource,
    },
}

/// Immutable descriptor-first source document owned by indexing work.
#[derive(Debug, Clone)]
pub struct SourceDocument {
    /// Semantic file identity for invalidation.
    pub identity: SemanticFileFingerprintIdentity,
    /// Optional live snapshot id when sourced from editor-owned state.
    pub snapshot_id: Option<SnapshotId>,
    /// Language identifier.
    pub language_id: LanguageId,
    /// Descriptor-first source input.
    pub source: SemanticSourceInput,
}

impl SourceDocument {
    /// Constructs a fixture document using the explicit bounded full-text optimization.
    pub fn new(
        workspace_id: WorkspaceId,
        file_id: FileId,
        canonical_path: CanonicalPath,
        language_id: LanguageId,
        text: impl Into<String>,
    ) -> Self {
        Self::with_versions(
            workspace_id,
            file_id,
            canonical_path,
            language_id,
            FileContentVersion(1),
            WorkspaceGeneration(1),
            None,
            SemanticPrivacyScope::Workspace,
            text,
        )
    }

    /// Constructs a fixture or small-buffer document with explicit version and privacy metadata.
    #[allow(clippy::too_many_arguments)]
    pub fn with_versions(
        workspace_id: WorkspaceId,
        file_id: FileId,
        canonical_path: CanonicalPath,
        language_id: LanguageId,
        file_content_version: FileContentVersion,
        workspace_generation: WorkspaceGeneration,
        snapshot_id: Option<SnapshotId>,
        privacy_scope: SemanticPrivacyScope,
        text: impl Into<String>,
    ) -> Self {
        let text = text.into();
        let content_hash = content_fingerprint(text.as_bytes());
        let identity = SemanticFileFingerprintIdentity {
            workspace_id,
            file_id,
            canonical_path,
            file_content_version,
            workspace_generation,
            content_hash: content_hash.clone(),
            disk_fingerprint: Some(content_hash),
            byte_len: Some(text.len() as u64),
            modified_at: None,
            privacy_scope,
            schema_version: INDEX_SCHEMA_VERSION,
        };
        let source = SemanticSourceInput::BoundedFullText {
            descriptor: descriptor_from_parts(None, Vec::new(), Vec::new(), Vec::new(), true),
            text: BoundedFullTextSource {
                byte_budget: text.len(),
                policy_reason: "explicit small-buffer semantic fixture/full-text optimization"
                    .to_string(),
                text,
            },
        };
        Self {
            identity,
            snapshot_id,
            language_id,
            source,
        }
    }

    /// Builds descriptor-only input from a text snapshot without calling full-text accessors.
    #[allow(clippy::too_many_arguments)]
    pub fn from_text_snapshot_descriptors(
        workspace_id: WorkspaceId,
        file_id: FileId,
        canonical_path: CanonicalPath,
        language_id: LanguageId,
        file_content_version: FileContentVersion,
        workspace_generation: WorkspaceGeneration,
        privacy_scope: SemanticPrivacyScope,
        snapshot: &TextSnapshot,
    ) -> Self {
        let snapshot_descriptor = snapshot_descriptor_from_text_snapshot(file_id, snapshot);
        let chunks = chunk_refs_from_text_snapshot(snapshot);
        let ranges = chunks
            .iter()
            .map(|chunk| chunk.byte_range)
            .collect::<Vec<_>>();
        let identity = SemanticFileFingerprintIdentity {
            workspace_id,
            file_id,
            canonical_path,
            file_content_version,
            workspace_generation,
            content_hash: FileFingerprint {
                algorithm: "devil-text-snapshot-content-hash-v1".to_string(),
                value: snapshot.content_hash().to_string(),
            },
            disk_fingerprint: None,
            byte_len: Some(snapshot.len() as u64),
            modified_at: None,
            privacy_scope,
            schema_version: INDEX_SCHEMA_VERSION,
        };
        Self {
            identity,
            snapshot_id: Some(snapshot.snapshot_id()),
            language_id,
            source: SemanticSourceInput::DescriptorOnly(descriptor_from_parts(
                Some(snapshot_descriptor),
                chunks,
                ranges,
                Vec::new(),
                false,
            )),
        }
    }

    /// Copies bounded full text from a small text snapshot as an explicit compatibility optimization.
    #[allow(clippy::too_many_arguments)]
    pub fn from_text_snapshot(
        workspace_id: WorkspaceId,
        file_id: FileId,
        canonical_path: CanonicalPath,
        language_id: LanguageId,
        file_content_version: FileContentVersion,
        workspace_generation: WorkspaceGeneration,
        privacy_scope: SemanticPrivacyScope,
        snapshot: &TextSnapshot,
    ) -> IndexResult<Self> {
        let text = snapshot
            .try_full_text()
            .map_err(|err| IndexError::TextSnapshotUnavailable {
                message: err.to_string(),
            })?
            .to_string();
        let snapshot_descriptor = snapshot_descriptor_from_text_snapshot(file_id, snapshot);
        let chunks = chunk_refs_from_text_snapshot(snapshot);
        let ranges = chunks
            .iter()
            .map(|chunk| chunk.byte_range)
            .collect::<Vec<_>>();
        let identity = SemanticFileFingerprintIdentity {
            workspace_id,
            file_id,
            canonical_path,
            file_content_version,
            workspace_generation,
            content_hash: FileFingerprint {
                algorithm: "devil-text-snapshot-content-hash-v1".to_string(),
                value: snapshot.content_hash().to_string(),
            },
            disk_fingerprint: None,
            byte_len: Some(snapshot.len() as u64),
            modified_at: None,
            privacy_scope,
            schema_version: INDEX_SCHEMA_VERSION,
        };
        Ok(Self {
            identity,
            snapshot_id: Some(snapshot.snapshot_id()),
            language_id,
            source: SemanticSourceInput::BoundedFullText {
                descriptor: descriptor_from_parts(
                    Some(snapshot_descriptor),
                    chunks,
                    ranges,
                    Vec::new(),
                    true,
                ),
                text: BoundedFullTextSource {
                    byte_budget: snapshot.len(),
                    policy_reason: "explicit small-buffer snapshot full-text optimization"
                        .to_string(),
                    text,
                },
            },
        })
    }

    /// Builds a source document from bounded chunks read through snapshot leases.
    pub fn from_snapshot_lease_chunks(
        identity: SemanticFileFingerprintIdentity,
        language_id: LanguageId,
        chunks: Vec<SnapshotLeaseChunk>,
    ) -> Self {
        let snapshot_id = chunks.first().map(|chunk| chunk.lease.snapshot_id);
        let leases = unique_leases(&chunks);
        let chunk_refs = chunks
            .iter()
            .map(|chunk| chunk_ref_from_snapshot_chunk(&chunk.chunk, Some(chunk.lease.lease_id)))
            .collect::<Vec<_>>();
        let ranges = chunk_refs
            .iter()
            .map(|chunk| chunk.byte_range)
            .collect::<Vec<_>>();
        let descriptor = descriptor_from_parts(None, chunk_refs, ranges, leases, true);
        let chunk_text = chunks
            .into_iter()
            .map(|chunk| SemanticSourceChunkText {
                lease: chunk.lease,
                chunk: chunk.chunk,
                text: chunk.text,
            })
            .collect();
        Self {
            identity,
            snapshot_id,
            language_id,
            source: SemanticSourceInput::LeaseChunks {
                descriptor,
                chunks: chunk_text,
            },
        }
    }

    /// Returns the source input kind.
    pub const fn source_kind(&self) -> SemanticSourceInputKind {
        match &self.source {
            SemanticSourceInput::DescriptorOnly(_) => SemanticSourceInputKind::DescriptorOnly,
            SemanticSourceInput::LeaseChunks { .. } => SemanticSourceInputKind::LeaseChunks,
            SemanticSourceInput::ChangedRanges { .. } => SemanticSourceInputKind::ChangedRanges,
            SemanticSourceInput::BoundedFullText { .. } => SemanticSourceInputKind::BoundedFullText,
        }
    }

    /// Returns descriptor metadata for this source input.
    pub const fn source_descriptor(&self) -> &SemanticSourceDescriptor {
        match &self.source {
            SemanticSourceInput::DescriptorOnly(descriptor)
            | SemanticSourceInput::LeaseChunks { descriptor, .. }
            | SemanticSourceInput::ChangedRanges { descriptor, .. }
            | SemanticSourceInput::BoundedFullText { descriptor, .. } => descriptor,
        }
    }

    /// Returns true when this work item uses the explicit bounded full-text optimization.
    pub const fn uses_bounded_full_text(&self) -> bool {
        matches!(self.source, SemanticSourceInput::BoundedFullText { .. })
    }

    /// Returns the invalidation key for parser and model-derived records.
    pub fn invalidation_key(
        &self,
        grammar_version: Option<SemanticGrammarVersion>,
        model_version: Option<SemanticModelVersion>,
    ) -> SemanticInvalidationKey {
        SemanticInvalidationKey {
            workspace_id: self.identity.workspace_id,
            file_id: self.identity.file_id,
            snapshot_id: self.snapshot_id,
            file_content_version: self.identity.file_content_version,
            workspace_generation: self.identity.workspace_generation,
            content_hash: self.identity.content_hash.clone(),
            grammar_version,
            model_version,
            privacy_scope: self.identity.privacy_scope,
            schema_version: self.identity.schema_version,
        }
    }
}

fn descriptor_from_parts(
    snapshot: Option<SnapshotDescriptor>,
    chunks: Vec<SemanticSourceChunkReference>,
    ranges: Vec<ByteRange>,
    leases: Vec<SnapshotLeaseDescriptor>,
    complete_text_coverage: bool,
) -> SemanticSourceDescriptor {
    let freshness_state = if complete_text_coverage {
        SemanticFreshnessState::Fresh
    } else {
        SemanticFreshnessState::Partial
    };
    let degraded_reasons = if complete_text_coverage {
        Vec::new()
    } else {
        vec!["descriptor-only source input; bounded text payload not owned".to_string()]
    };

    SemanticSourceDescriptor {
        snapshot,
        chunks,
        ranges,
        leases,
        freshness_state,
        degraded_reasons,
    }
}

fn snapshot_descriptor_from_text_snapshot(
    file_id: FileId,
    snapshot: &TextSnapshot,
) -> SnapshotDescriptor {
    SnapshotDescriptor {
        snapshot_id: snapshot.snapshot_id(),
        file_id: Some(file_id),
        buffer_version: snapshot.buffer_version(),
        byte_len: snapshot.len() as u64,
        content_hash: Some(snapshot.content_hash().to_string()),
        created_at: TimestampMillis::now(),
    }
}

fn chunk_refs_from_text_snapshot(snapshot: &TextSnapshot) -> Vec<SemanticSourceChunkReference> {
    snapshot
        .chunk_descriptors()
        .iter()
        .map(|chunk| chunk_ref_from_text_chunk(snapshot.snapshot_id(), chunk))
        .collect()
}

fn chunk_ref_from_text_chunk(
    snapshot_id: SnapshotId,
    chunk: &TextChunkDescriptor,
) -> SemanticSourceChunkReference {
    SemanticSourceChunkReference {
        snapshot_id,
        chunk_index: chunk.ordinal as u32,
        byte_range: ByteRange::new(chunk.start_byte as u64, chunk.end_byte as u64),
        line_range: LineIndexRange {
            start: chunk.start_line as u32,
            end: chunk.end_line.saturating_add(1) as u32,
        },
        byte_len: chunk.byte_len as u64,
        chunk_hash: FileFingerprint {
            algorithm: "devil-text-chunk-sha256-v1".to_string(),
            value: chunk.hash.clone(),
        },
        lease_id: None,
        schema_version: INDEX_SCHEMA_VERSION,
    }
}

fn chunk_ref_from_snapshot_chunk(
    chunk: &SnapshotChunkDescriptor,
    lease_id: Option<uuid::Uuid>,
) -> SemanticSourceChunkReference {
    SemanticSourceChunkReference {
        snapshot_id: chunk.snapshot_id,
        chunk_index: chunk.chunk_index,
        byte_range: chunk.byte_range,
        line_range: chunk.line_range,
        byte_len: chunk.byte_len,
        chunk_hash: chunk.chunk_hash.clone(),
        lease_id,
        schema_version: INDEX_SCHEMA_VERSION,
    }
}

fn unique_leases(chunks: &[SnapshotLeaseChunk]) -> Vec<SnapshotLeaseDescriptor> {
    let mut leases = Vec::new();
    for chunk in chunks {
        if !leases
            .iter()
            .any(|lease: &SnapshotLeaseDescriptor| lease.lease_id == chunk.lease.lease_id)
        {
            leases.push(chunk.lease.clone());
        }
    }
    leases
}

/// Request passed to a parser worker.
#[derive(Debug, Clone)]
pub struct ParseRequest {
    /// Document to parse.
    pub document: SourceDocument,
    /// Grammar version that invalidates parser-derived records.
    pub grammar_version: SemanticGrammarVersion,
    /// Model metadata version used by deterministic ranking records.
    pub model_version: SemanticModelVersion,
}

/// Cache key for parser-derived syntax records.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SyntaxCacheKey {
    /// Workspace identifier from the workspace-authored source identity.
    pub workspace_id: WorkspaceId,
    /// File identifier from the workspace-authored source identity.
    pub file_id: FileId,
    /// Snapshot identifier when the parse is snapshot-bound.
    pub snapshot_id: Option<SnapshotId>,
    /// File content version observed by workspace authority.
    pub file_content_version: FileContentVersion,
    /// Workspace generation observed by workspace authority.
    pub workspace_generation: WorkspaceGeneration,
    /// Content hash of the parsed document.
    pub content_hash: FileFingerprint,
    /// Language identifier.
    pub language_id: LanguageId,
    /// Grammar version.
    pub grammar_version: SemanticGrammarVersion,
    /// Deterministic parser or extraction version.
    pub parser_version: String,
    /// Model metadata version for deterministic ranking or learned-enrichment invalidation.
    pub model_version: SemanticModelVersion,
    /// Privacy scope attached to the parser-derived record.
    pub privacy_scope: SemanticPrivacyScope,
    /// Cache schema version.
    pub schema_version: u16,
    /// Metadata-only fingerprint of descriptor freshness inputs.
    pub descriptor: SyntaxSourceDescriptorFingerprint,
}

impl SyntaxCacheKey {
    /// Builds a syntax cache key from a parse request without embedding source text.
    pub fn from_request(request: &ParseRequest) -> Self {
        Self::from_document(
            &request.document,
            &request.grammar_version,
            &request.model_version,
        )
    }

    /// Builds a syntax cache key from source identity, versions, privacy scope, and descriptors.
    pub fn from_document(
        document: &SourceDocument,
        grammar_version: &SemanticGrammarVersion,
        model_version: &SemanticModelVersion,
    ) -> Self {
        Self {
            workspace_id: document.identity.workspace_id,
            file_id: document.identity.file_id,
            snapshot_id: document.snapshot_id,
            file_content_version: document.identity.file_content_version,
            workspace_generation: document.identity.workspace_generation,
            content_hash: document.identity.content_hash.clone(),
            language_id: document.language_id.clone(),
            grammar_version: grammar_version.clone(),
            parser_version: LEXICAL_EXTRACTION_VERSION.to_string(),
            model_version: model_version.clone(),
            privacy_scope: document.identity.privacy_scope,
            schema_version: document.identity.schema_version,
            descriptor: SyntaxSourceDescriptorFingerprint::from_document(document),
        }
    }
}

/// Metadata-only descriptor fingerprint embedded in syntax cache keys.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SyntaxSourceDescriptorFingerprint {
    /// Source input kind represented by this descriptor.
    pub source_kind: SemanticSourceInputKind,
    /// Snapshot identifier from the descriptor when available.
    pub snapshot_id: Option<SnapshotId>,
    /// Snapshot content hash represented as metadata, not source.
    pub snapshot_content_hash: Option<FileFingerprint>,
    /// Snapshot byte length when known.
    pub snapshot_byte_len: Option<u64>,
    /// Descriptor freshness state label.
    pub freshness_state: String,
    /// Chunk metadata used for freshness and range invalidation.
    pub chunks: Vec<SyntaxSourceChunkFingerprint>,
    /// Byte ranges represented by the source descriptor.
    pub ranges: Vec<ByteRange>,
    /// Number of lease descriptors observed for bounded chunk input.
    pub lease_count: u32,
    /// Highest schema version observed among descriptor components.
    pub schema_version: u16,
}

impl SyntaxSourceDescriptorFingerprint {
    fn from_document(document: &SourceDocument) -> Self {
        let descriptor = document.source_descriptor();
        let snapshot_id = descriptor
            .snapshot
            .as_ref()
            .map(|snapshot| snapshot.snapshot_id);
        let snapshot_content_hash = descriptor.snapshot.as_ref().and_then(|snapshot| {
            snapshot.content_hash.as_ref().map(|value| FileFingerprint {
                algorithm: "devil-text-snapshot-content-hash-v1".to_string(),
                value: value.clone(),
            })
        });
        let snapshot_byte_len = descriptor
            .snapshot
            .as_ref()
            .map(|snapshot| snapshot.byte_len);
        let chunks = descriptor
            .chunks
            .iter()
            .map(SyntaxSourceChunkFingerprint::from_chunk_reference)
            .collect::<Vec<_>>();
        let schema_version = descriptor
            .chunks
            .iter()
            .map(|chunk| chunk.schema_version)
            .chain(descriptor.leases.iter().map(|lease| lease.schema_version))
            .max()
            .unwrap_or(INDEX_SCHEMA_VERSION);

        Self {
            source_kind: document.source_kind(),
            snapshot_id,
            snapshot_content_hash,
            snapshot_byte_len,
            freshness_state: freshness_state_label(descriptor.freshness_state).to_string(),
            chunks,
            ranges: descriptor.ranges.clone(),
            lease_count: descriptor.leases.len() as u32,
            schema_version,
        }
    }
}

/// Metadata-only chunk fingerprint embedded in syntax descriptor keys.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SyntaxSourceChunkFingerprint {
    /// Snapshot identifier owning the chunk.
    pub snapshot_id: SnapshotId,
    /// Chunk ordinal in the snapshot.
    pub chunk_index: u32,
    /// Byte range covered by the chunk.
    pub byte_range: ByteRange,
    /// Line range covered by the chunk.
    pub line_range: LineIndexRange,
    /// Chunk byte length.
    pub byte_len: u64,
    /// Hash of the chunk contents.
    pub chunk_hash: FileFingerprint,
    /// Whether a bounded lease id was present without recording source text.
    pub lease_present: bool,
    /// Chunk descriptor schema version.
    pub schema_version: u16,
}

impl SyntaxSourceChunkFingerprint {
    fn from_chunk_reference(chunk: &SemanticSourceChunkReference) -> Self {
        Self {
            snapshot_id: chunk.snapshot_id,
            chunk_index: chunk.chunk_index,
            byte_range: chunk.byte_range,
            line_range: chunk.line_range,
            byte_len: chunk.byte_len,
            chunk_hash: chunk.chunk_hash.clone(),
            lease_present: chunk.lease_id.is_some(),
            schema_version: chunk.schema_version,
        }
    }
}

fn freshness_state_label(state: SemanticFreshnessState) -> &'static str {
    match state {
        SemanticFreshnessState::Fresh => "fresh",
        SemanticFreshnessState::Stale => "stale",
        SemanticFreshnessState::Partial => "partial",
        SemanticFreshnessState::Unavailable => "unavailable",
    }
}

/// Metadata-only syntax cache event kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyntaxCacheEventKind {
    /// A cached parser outcome was reused after exact freshness-key matching.
    Hit,
    /// A parser outcome was inserted after a cache miss.
    MissInserted,
    /// A caller explicitly inserted a parser outcome.
    Inserted,
    /// Entries were removed by grammar-version invalidation.
    InvalidatedGrammar,
}

/// Metadata-only syntax cache event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyntaxCacheEvent {
    /// Event kind.
    pub kind: SyntaxCacheEventKind,
    /// Cache key associated with the event.
    pub cache_key: SyntaxCacheKey,
    /// Event observation timestamp.
    pub observed_at: TimestampMillis,
}

/// Deterministic syntax-tree/cache record produced by parser workers.
#[derive(Debug, Clone)]
pub struct SyntaxTreeRecord {
    /// Cache key used to retrieve this syntax record.
    pub cache_key: SyntaxCacheKey,
    /// File identity used for invalidation.
    pub identity: SemanticFileFingerprintIdentity,
    /// Count of lexical syntax nodes used by the fallback parser.
    pub node_count: usize,
    /// Count of declaration candidates detected by the parser.
    pub declaration_count: usize,
    /// Freshness metadata for the syntax record.
    pub freshness: SemanticFreshnessState,
    /// Parser provenance.
    pub provenance: SemanticRecordProvenance,
}

/// Complete parser outcome used by the syntax cache and semantic actor.
#[derive(Debug, Clone)]
pub struct ParseOutcome {
    /// Syntax record for the parsed document.
    pub syntax_tree: SyntaxTreeRecord,
    /// File-level semantic index extracted from the document.
    pub file_index: FileSemanticIndex,
    /// Diagnostics emitted during parsing.
    pub diagnostics: Vec<ProtocolDiagnostic>,
}

/// Parser worker abstraction; runtime wiring is intentionally owned by callers, not this crate.
pub trait ParserWorker {
    /// Parses a source document into deterministic semantic records.
    fn parse(&self, request: ParseRequest) -> IndexResult<ParseOutcome>;
}

/// Parser cache keyed by source identity, content hash, language, grammar, privacy, and freshness metadata.
#[derive(Debug, Default, Clone)]
pub struct SyntaxTreeCache {
    entries: HashMap<SyntaxCacheKey, ParseOutcome>,
    events: Vec<SyntaxCacheEvent>,
}

impl SyntaxTreeCache {
    /// Constructs an empty syntax cache.
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns a cached syntax tree by key.
    pub fn get(&self, key: &SyntaxCacheKey) -> Option<&SyntaxTreeRecord> {
        self.entries.get(key).map(|outcome| &outcome.syntax_tree)
    }

    /// Returns true when a key is present in the syntax cache.
    pub fn contains_key(&self, key: &SyntaxCacheKey) -> bool {
        self.entries.contains_key(key)
    }

    /// Returns the number of cached parser outcomes.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Returns metadata-only cache events emitted by this cache.
    pub fn events(&self) -> &[SyntaxCacheEvent] {
        &self.events
    }

    /// Returns `true` when the syntax cache is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Inserts a parser outcome into the cache.
    pub fn insert(&mut self, outcome: ParseOutcome) {
        let cache_key = outcome.syntax_tree.cache_key.clone();
        self.entries.insert(cache_key.clone(), outcome);
        self.record_event(SyntaxCacheEventKind::Inserted, cache_key);
    }

    /// Returns a cached parse outcome or invokes the provided worker.
    pub fn get_or_parse<W: ParserWorker>(
        &mut self,
        worker: &W,
        request: ParseRequest,
    ) -> IndexResult<ParseOutcome> {
        let key = SyntaxCacheKey::from_request(&request);

        if let Some(outcome) = self.entries.get(&key) {
            let outcome = outcome.clone();
            self.record_event(SyntaxCacheEventKind::Hit, key);
            return Ok(outcome.clone());
        }

        let outcome = worker.parse(request)?;
        self.entries.insert(key.clone(), outcome.clone());
        self.record_event(SyntaxCacheEventKind::MissInserted, key);
        Ok(outcome)
    }

    /// Removes all cache entries for a grammar version.
    pub fn invalidate_grammar(&mut self, grammar_version: &SemanticGrammarVersion) -> usize {
        let invalidated_keys = self
            .entries
            .keys()
            .filter(|key| &key.grammar_version == grammar_version)
            .cloned()
            .collect::<Vec<_>>();
        let before = self.entries.len();
        self.entries
            .retain(|key, _| &key.grammar_version != grammar_version);
        for key in invalidated_keys {
            self.record_event(SyntaxCacheEventKind::InvalidatedGrammar, key);
        }
        before.saturating_sub(self.entries.len())
    }

    fn record_event(&mut self, kind: SyntaxCacheEventKind, cache_key: SyntaxCacheKey) {
        self.events.push(SyntaxCacheEvent {
            kind,
            cache_key,
            observed_at: TimestampMillis::now(),
        });
    }
}

/// Production-ready deterministic parser worker backed by lexical analysis.
#[derive(Debug, Default, Clone, Copy)]
pub struct LexicalFallbackParser;

impl LexicalFallbackParser {
    /// Constructs a lexical fallback parser.
    pub const fn new() -> Self {
        Self
    }
}

impl ParserWorker for LexicalFallbackParser {
    fn parse(&self, request: ParseRequest) -> IndexResult<ParseOutcome> {
        let indexer = LexicalIndexer::new();
        let file_index = indexer.index_document(
            &request.document,
            request.grammar_version.clone(),
            request.model_version,
        );
        let syntax_tree = file_index.syntax_tree.clone();
        let diagnostics = file_index.diagnostics.clone();
        Ok(ParseOutcome {
            syntax_tree,
            file_index,
            diagnostics,
        })
    }
}

/// File-level semantic records extracted from a source document.
#[derive(Debug, Clone)]
pub struct FileSemanticIndex {
    /// File identity used for invalidation.
    pub identity: SemanticFileFingerprintIdentity,
    /// Optional live snapshot id represented by this file index.
    pub snapshot_id: Option<SnapshotId>,
    /// Language identifier.
    pub language_id: LanguageId,
    /// Source input kind used to produce these records.
    pub source_kind: SemanticSourceInputKind,
    /// Chunk references backing this semantic record set.
    pub source_chunks: Vec<SemanticSourceChunkReference>,
    /// Byte ranges represented by this semantic record set.
    pub source_ranges: Vec<ByteRange>,
    /// Source freshness and degradation metadata.
    pub source_freshness: SemanticFreshness,
    /// Syntax tree/cache record.
    pub syntax_tree: SyntaxTreeRecord,
    /// Lexical symbol-to-file map records.
    pub symbols: Vec<SymbolFileMapRecord>,
    /// Normalized semantic graph records.
    pub graph_records: Vec<SemanticGraphRecord>,
    /// Diagnostics emitted during extraction.
    pub diagnostics: Vec<ProtocolDiagnostic>,
}

impl FileSemanticIndex {
    /// Converts this in-memory file index into a metadata-only persistence DTO.
    pub fn to_semantic_metadata_record(&self) -> SemanticMetadataRecord {
        let freshness_key = self.semantic_metadata_freshness_key();
        SemanticMetadataRecord {
            record_id: SemanticRecordId(format!(
                "semantic-metadata:{}:{}:{}:{}:{}",
                self.identity.workspace_id.0,
                self.identity.file_id.0,
                self.language_id.0,
                freshness_key.workspace_generation.0,
                freshness_key.content_hash.value
            )),
            workspace_id: self.identity.workspace_id,
            file_id: self.identity.file_id,
            language_id: self.language_id.clone(),
            freshness_key,
            file_identity: self.identity.clone(),
            provenance: self.syntax_tree.provenance.clone(),
            symbols: self
                .symbols
                .iter()
                .map(|symbol| SemanticMetadataSymbolRecord {
                    symbol_id: symbol.symbol_id.clone(),
                    symbol_name_hash: symbol.symbol_name_hash.clone(),
                    kind_hash: content_fingerprint(symbol.kind.as_bytes()),
                    declaration_range: symbol.declaration_range,
                    reference_ranges: symbol.reference_ranges.clone(),
                    schema_version: symbol.schema_version,
                })
                .collect(),
            graph_records: self
                .graph_records
                .iter()
                .map(|record| SemanticMetadataGraphRecord {
                    record_id: record.record_id.clone(),
                    kind: record.kind,
                    source: record.source.clone(),
                    target: record.target.clone(),
                    label_hash: content_fingerprint(record.label.as_bytes()),
                    property_hashes: record
                        .properties
                        .iter()
                        .map(|property| {
                            let metadata = format!(
                                "{}:{}:{:?}",
                                property.key, property.value, property.redaction
                            );
                            content_fingerprint(metadata.as_bytes())
                        })
                        .collect(),
                    freshness: record.freshness,
                    schema_version: record.schema_version,
                })
                .collect(),
            diagnostic_summaries: self
                .diagnostics
                .iter()
                .map(|diagnostic| SemanticMetadataDiagnosticSummary {
                    code_hash: content_fingerprint(diagnostic.code.as_bytes()),
                    severity: diagnostic.severity,
                    range: diagnostic.range,
                    count: 1,
                })
                .collect(),
            freshness_state: self.source_freshness.state,
            persisted_at: TimestampMillis::now(),
            schema_version: INDEX_SCHEMA_VERSION,
        }
    }

    fn semantic_metadata_freshness_key(&self) -> SemanticMetadataFreshnessKey {
        SemanticMetadataFreshnessKey {
            workspace_id: self.identity.workspace_id,
            file_id: self.identity.file_id,
            language_id: self.language_id.clone(),
            snapshot_id: self.snapshot_id,
            file_content_version: self.identity.file_content_version,
            workspace_generation: self.identity.workspace_generation,
            content_hash: self.identity.content_hash.clone(),
            grammar_version: Some(self.syntax_tree.cache_key.grammar_version.clone()),
            model_version: Some(self.syntax_tree.cache_key.model_version.clone()),
            parser_version: self.syntax_tree.cache_key.parser_version.clone(),
            privacy_scope: self.identity.privacy_scope,
            descriptor: SemanticMetadataDescriptorIdentity {
                source_kind: metadata_source_kind(self.source_kind),
                snapshot_id: self.snapshot_id,
                content_hash: self.identity.content_hash.clone(),
                byte_len: self.identity.byte_len,
                ranges: self.source_ranges.clone(),
                chunks: self
                    .source_chunks
                    .iter()
                    .map(|chunk| SemanticMetadataChunkReference {
                        snapshot_id: chunk.snapshot_id,
                        chunk_index: chunk.chunk_index,
                        byte_range: chunk.byte_range,
                        line_range: chunk.line_range,
                        byte_len: chunk.byte_len,
                        chunk_hash: chunk.chunk_hash.clone(),
                        lease_present: chunk.lease_id.is_some(),
                        schema_version: chunk.schema_version,
                    })
                    .collect(),
                schema_version: self
                    .source_chunks
                    .iter()
                    .map(|chunk| chunk.schema_version)
                    .max()
                    .unwrap_or(INDEX_SCHEMA_VERSION),
            },
            schema_version: INDEX_SCHEMA_VERSION,
        }
    }
}

fn metadata_source_kind(kind: SemanticSourceInputKind) -> SemanticMetadataSourceKind {
    match kind {
        SemanticSourceInputKind::DescriptorOnly => SemanticMetadataSourceKind::DescriptorOnly,
        SemanticSourceInputKind::LeaseChunks => SemanticMetadataSourceKind::LeaseChunks,
        SemanticSourceInputKind::ChangedRanges => SemanticMetadataSourceKind::ChangedRanges,
        SemanticSourceInputKind::BoundedFullText => SemanticMetadataSourceKind::BoundedFullText,
    }
}

/// Upsert outcome for a file-level semantic index.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SemanticUpsertOutcome {
    /// New records were inserted.
    Applied,
    /// Existing file records were replaced by a newer identity.
    Replaced,
    /// Incoming records were stale and ignored.
    IgnoredStale,
}

/// Low-latency in-memory semantic index over protocol DTO records.
#[derive(Debug, Default, Clone)]
pub struct SemanticIndex {
    files: HashMap<(WorkspaceId, FileId), FileSemanticIndex>,
    symbol_records: Vec<SymbolFileMapRecord>,
    graph_records: Vec<SemanticGraphRecord>,
}

impl SemanticIndex {
    /// Constructs an empty semantic index.
    pub fn new() -> Self {
        Self::default()
    }

    /// Inserts or replaces records for a file using generation and content-version semantics.
    pub fn upsert(&mut self, file_index: FileSemanticIndex) -> SemanticUpsertOutcome {
        let key = (
            file_index.identity.workspace_id,
            file_index.identity.file_id,
        );
        let outcome = if let Some(existing) = self.files.get(&key) {
            if file_identity_version_tuple(&existing.identity)
                > file_identity_version_tuple(&file_index.identity)
            {
                return SemanticUpsertOutcome::IgnoredStale;
            }
            SemanticUpsertOutcome::Replaced
        } else {
            SemanticUpsertOutcome::Applied
        };

        self.files.insert(key, file_index);
        self.rebuild_views();
        outcome
    }

    /// Returns all file indexes in deterministic path order.
    pub fn files(&self) -> Vec<&FileSemanticIndex> {
        let mut files = self.files.values().collect::<Vec<_>>();
        files.sort_by_key(|file| file.identity.canonical_path.0.clone());
        files
    }

    /// Returns all symbol map records in deterministic display order.
    pub fn symbols(&self) -> &[SymbolFileMapRecord] {
        &self.symbol_records
    }

    /// Returns all semantic graph records in deterministic order.
    pub fn graph_records(&self) -> &[SemanticGraphRecord] {
        &self.graph_records
    }

    /// Serves a pure semantic query without mutating buffers, files, or workspace state.
    pub fn query(&self, request: &SemanticQueryRequest) -> SemanticQueryResponse {
        let mut results = match request.kind {
            SemanticQueryKind::SymbolLookup
            | SemanticQueryKind::CompletionRanking
            | SemanticQueryKind::AiContextSelection
            | SemanticQueryKind::AgentPlanning => self.query_symbols(request),
            SemanticQueryKind::Definition | SemanticQueryKind::HoverEnrichment => {
                self.query_definition_like(request)
            }
            SemanticQueryKind::References => self.query_references(request),
            SemanticQueryKind::TestImpact => {
                self.query_graph_kind(request, SemanticGraphRecordKind::TestLink)
            }
            SemanticQueryKind::RefactoringPreview => self.query_refactoring_preview(request),
        };

        let total = results.len();
        let limit = request.limit as usize;
        if limit > 0 && results.len() > limit {
            results.truncate(limit);
        }

        let status = if request.freshness_policy == SemanticQueryFreshnessPolicy::RequireFresh
            && results
                .iter()
                .any(|result| result.freshness.state != SemanticFreshnessState::Fresh)
        {
            SemanticQueryStatus::Stale
        } else if limit > 0 && total > limit {
            SemanticQueryStatus::Partial
        } else {
            SemanticQueryStatus::Fresh
        };

        SemanticQueryResponse {
            query_id: request.query_id,
            workspace_id: request.scope.workspace_id,
            status,
            results,
            diagnostics: Vec::new(),
            next_page_token: if limit > 0 && total > limit {
                Some(format!("offset:{limit}"))
            } else {
                None
            },
            correlation_id: request.correlation_id,
            causality_id: request.causality_id,
            schema_version: INDEX_SCHEMA_VERSION,
        }
    }

    fn rebuild_views(&mut self) {
        let mut symbol_records = self
            .files
            .values()
            .flat_map(|file| file.symbols.clone())
            .collect::<Vec<_>>();
        symbol_records.sort_by(|left, right| {
            left.display_name
                .cmp(&right.display_name)
                .then_with(|| left.path.0.cmp(&right.path.0))
                .then_with(|| left.symbol_id.0.cmp(&right.symbol_id.0))
        });

        let mut graph_records = self
            .files
            .values()
            .flat_map(|file| file.graph_records.clone())
            .collect::<Vec<_>>();
        graph_records.sort_by_key(|record| record.record_id.0.clone());

        self.symbol_records = symbol_records;
        self.graph_records = graph_records;
    }

    fn query_symbols(&self, request: &SemanticQueryRequest) -> Vec<SemanticQueryResult> {
        self.symbol_records
            .iter()
            .filter(|symbol| symbol_in_scope(symbol, request))
            .filter(|symbol| {
                request
                    .text_query_hash
                    .as_ref()
                    .is_none_or(|hash| hash == &symbol.symbol_name_hash)
            })
            .enumerate()
            .map(|(ordinal, symbol)| {
                result_from_symbol(
                    symbol,
                    request,
                    ordinal,
                    SemanticQueryResultKind::Symbol,
                    None,
                )
            })
            .collect()
    }

    fn query_definition_like(&self, request: &SemanticQueryRequest) -> Vec<SemanticQueryResult> {
        let Some(symbol) = self.find_symbol_at_position_or_hash(request) else {
            return Vec::new();
        };
        vec![result_from_symbol(
            symbol,
            request,
            0,
            SemanticQueryResultKind::Location,
            symbol.declaration_range,
        )]
    }

    fn query_references(&self, request: &SemanticQueryRequest) -> Vec<SemanticQueryResult> {
        let Some(symbol) = self.find_symbol_at_position_or_hash(request) else {
            return Vec::new();
        };
        symbol
            .reference_ranges
            .iter()
            .enumerate()
            .map(|(ordinal, range)| {
                result_from_symbol(
                    symbol,
                    request,
                    ordinal,
                    SemanticQueryResultKind::Location,
                    Some(*range),
                )
            })
            .collect()
    }

    fn query_graph_kind(
        &self,
        request: &SemanticQueryRequest,
        kind: SemanticGraphRecordKind,
    ) -> Vec<SemanticQueryResult> {
        self.graph_records
            .iter()
            .filter(|record| record.kind == kind)
            .filter(|record| graph_in_scope(record, request))
            .enumerate()
            .map(|(ordinal, record)| result_from_graph(record, request, ordinal))
            .collect()
    }

    fn query_refactoring_preview(
        &self,
        request: &SemanticQueryRequest,
    ) -> Vec<SemanticQueryResult> {
        self.symbol_records
            .iter()
            .filter(|symbol| symbol_in_scope(symbol, request))
            .filter(|symbol| {
                request
                    .text_query_hash
                    .as_ref()
                    .is_none_or(|hash| hash == &symbol.symbol_name_hash)
            })
            .enumerate()
            .map(|(ordinal, symbol)| {
                let preview = ProposalPayloadSummary {
                    kind: ProposalPayloadKind::WorkspaceEdit,
                    affected_files: vec![symbol.file_id],
                    title: Some(format!(
                        "semantic refactoring preview for {}",
                        symbol
                            .display_name
                            .as_deref()
                            .unwrap_or("metadata-only symbol")
                    )),
                    byte_count: None,
                };
                let mut result = result_from_symbol(
                    symbol,
                    request,
                    ordinal,
                    SemanticQueryResultKind::ProposalPreview,
                    symbol.declaration_range,
                );
                result.proposal_preview = Some(preview);
                result
            })
            .collect()
    }

    fn find_symbol_at_position_or_hash(
        &self,
        request: &SemanticQueryRequest,
    ) -> Option<&SymbolFileMapRecord> {
        if let Some(hash) = request.text_query_hash.as_ref() {
            return self.symbol_records.iter().find(|symbol| {
                symbol_in_scope(symbol, request) && &symbol.symbol_name_hash == hash
            });
        }

        let position = request.position?;
        self.symbol_records.iter().find(|symbol| {
            symbol_in_scope(symbol, request)
                && (symbol
                    .declaration_range
                    .is_some_and(|range| range_contains(range, position))
                    || symbol
                        .reference_ranges
                        .iter()
                        .any(|range| range_contains(*range, position)))
        })
    }
}

/// Stateless shallow lexical indexer producing protocol semantic DTOs.
#[derive(Debug, Default, Clone, Copy)]
pub struct LexicalIndexer;

impl LexicalIndexer {
    /// Constructs a lexical indexer.
    pub const fn new() -> Self {
        Self
    }

    /// Indexes a source document into symbol maps, graph records, and parser-cache metadata.
    pub fn index_document(
        &self,
        document: &SourceDocument,
        grammar_version: SemanticGrammarVersion,
        model_version: SemanticModelVersion,
    ) -> FileSemanticIndex {
        let invalidation_key =
            document.invalidation_key(Some(grammar_version.clone()), Some(model_version.clone()));
        let source_descriptor = document.source_descriptor();
        let source_freshness = SemanticFreshness {
            state: source_descriptor.freshness_state,
            key: invalidation_key.clone(),
            degraded_reasons: source_descriptor.degraded_reasons.clone(),
            observed_at: TimestampMillis::now(),
        };
        let provenance = provenance(SemanticRecordSource::Lexical);
        let lexical = extract_lexical_facts(document);
        let syntax_tree = SyntaxTreeRecord {
            cache_key: SyntaxCacheKey::from_document(document, &grammar_version, &model_version),
            identity: document.identity.clone(),
            node_count: lexical.token_count,
            declaration_count: lexical.declarations.len(),
            freshness: source_descriptor.freshness_state,
            provenance: provenance.clone(),
        };

        let mut symbols = lexical
            .declarations
            .iter()
            .map(|candidate| SymbolFileMapRecord {
                symbol_id: symbol_id(document, candidate),
                symbol_name_hash: symbol_name_fingerprint(&candidate.name),
                display_name: display_name_for_scope(
                    &candidate.name,
                    document.identity.privacy_scope,
                ),
                kind: candidate.kind.clone(),
                workspace_id: document.identity.workspace_id,
                file_id: document.identity.file_id,
                path: document.identity.canonical_path.clone(),
                language_id: document.language_id.clone(),
                declaration_range: Some(candidate.range),
                reference_ranges: candidate.reference_ranges.clone(),
                invalidation_key: invalidation_key.clone(),
                provenance: provenance.clone(),
                schema_version: INDEX_SCHEMA_VERSION,
            })
            .collect::<Vec<_>>();
        symbols.sort_by(|left, right| {
            left.display_name
                .cmp(&right.display_name)
                .then_with(|| left.symbol_id.0.cmp(&right.symbol_id.0))
        });

        let graph_records = build_graph_records(document, &lexical, &symbols, &invalidation_key);
        let diagnostics = lexical.diagnostics;

        FileSemanticIndex {
            identity: document.identity.clone(),
            snapshot_id: document.snapshot_id,
            language_id: document.language_id.clone(),
            source_kind: document.source_kind(),
            source_chunks: source_descriptor.chunks.clone(),
            source_ranges: source_descriptor.ranges.clone(),
            source_freshness,
            syntax_tree,
            symbols,
            graph_records,
            diagnostics,
        }
    }
}

#[derive(Debug, Clone)]
struct LexicalFacts {
    declarations: Vec<SymbolCandidate>,
    imports: Vec<LineFact>,
    exports: Vec<LineFact>,
    calls: Vec<CallFact>,
    diagnostics: Vec<ProtocolDiagnostic>,
    owner_facts: Vec<LineFact>,
    todo_facts: Vec<LineFact>,
    token_count: usize,
}

#[derive(Debug, Clone)]
struct SymbolCandidate {
    name: String,
    kind: String,
    range: ProtocolTextRange,
    has_type_hint: bool,
    reference_ranges: Vec<ProtocolTextRange>,
}

#[derive(Debug, Clone)]
struct LineFact {
    label: String,
    value_hash: FileFingerprint,
    range: ProtocolTextRange,
    line: u32,
}

#[derive(Debug, Clone)]
struct CallFact {
    caller: Option<String>,
    callee: String,
    range: ProtocolTextRange,
}

#[derive(Debug, Clone)]
struct TokenFact {
    text: String,
    range: ProtocolTextRange,
}

struct LexicalTextSegment<'a> {
    text: &'a str,
    start_line: u32,
    start_byte: usize,
}

fn extract_lexical_facts(document: &SourceDocument) -> LexicalFacts {
    let mut declarations = Vec::new();
    let mut imports = Vec::new();
    let mut exports = Vec::new();
    let mut diagnostics = Vec::new();
    let mut owner_facts = Vec::new();
    let mut todo_facts = Vec::new();
    let mut tokens = Vec::new();
    let mut calls = Vec::new();
    let mut current_scope: Option<String> = None;

    for segment in lexical_text_segments(document) {
        let mut byte_cursor = segment.start_byte;
        for (line_index, line) in segment.text.lines().enumerate() {
            let line_number = segment.start_line.saturating_add(line_index as u32);
            let trimmed = line.trim();
            let leading = line.len().saturating_sub(line.trim_start().len());

            if !trimmed.is_empty() {
                tokens.extend(tokenize_line(line, line_number, byte_cursor));
            }

            if is_import_line(trimmed) {
                imports.push(line_fact("import", line, line_number, byte_cursor));
            }
            if is_export_line(trimmed) {
                exports.push(line_fact("export", line, line_number, byte_cursor));
            }
            if let Some(owner_col) = line.find("owner:").or_else(|| line.find("@owner")) {
                owner_facts.push(LineFact {
                    label: "owner".to_string(),
                    value_hash: content_fingerprint(line.as_bytes()),
                    range: range_for_cols(line_number, owner_col, line.len(), byte_cursor),
                    line: line_number,
                });
            }
            if let Some(todo_col) = find_case_insensitive(line, "TODO")
                .or_else(|| find_case_insensitive(line, "FIXME"))
                .or_else(|| find_case_insensitive(line, "BUG"))
            {
                let fact = LineFact {
                    label: "diagnostic".to_string(),
                    value_hash: content_fingerprint(line.as_bytes()),
                    range: range_for_cols(line_number, todo_col, line.len(), byte_cursor),
                    line: line_number,
                };
                diagnostics.push(diagnostic(
                    "index.lexical.todo",
                    "lexical diagnostic marker linked into semantic graph",
                    ProtocolDiagnosticSeverity::Hint,
                    Some(document.identity.canonical_path.clone()),
                    Some(fact.range),
                ));
                todo_facts.push(fact);
            }

            if let Some((kind, name, col)) = declaration_from_line(trimmed, leading) {
                let range = range_for_cols(line_number, col, col + name.len(), byte_cursor);
                current_scope = Some(name.clone());
                declarations.push(SymbolCandidate {
                    name,
                    kind,
                    range,
                    has_type_hint: trimmed.contains(" -> ") || trimmed.contains(':'),
                    reference_ranges: Vec::new(),
                });
            }

            calls.extend(call_facts_from_line(
                line,
                line_number,
                byte_cursor,
                current_scope.as_deref(),
            ));

            byte_cursor = byte_cursor.saturating_add(line.len()).saturating_add(1);
        }
    }

    let declaration_ranges = declarations
        .iter()
        .map(|candidate| (candidate.name.clone(), candidate.range))
        .collect::<Vec<_>>();
    for declaration in &mut declarations {
        declaration.reference_ranges = tokens
            .iter()
            .filter(|token| token.text == declaration.name)
            .filter(|token| {
                !declaration_ranges.iter().any(|(name, range)| {
                    name == &declaration.name && ranges_equal(*range, token.range)
                })
            })
            .map(|token| token.range)
            .collect();
    }

    LexicalFacts {
        declarations,
        imports,
        exports,
        calls,
        diagnostics,
        owner_facts,
        todo_facts,
        token_count: tokens.len(),
    }
}

fn lexical_text_segments(document: &SourceDocument) -> Vec<LexicalTextSegment<'_>> {
    match &document.source {
        SemanticSourceInput::DescriptorOnly(_) => Vec::new(),
        SemanticSourceInput::LeaseChunks { chunks, .. }
        | SemanticSourceInput::ChangedRanges { chunks, .. } => chunks
            .iter()
            .map(|chunk| LexicalTextSegment {
                text: chunk.text.as_str(),
                start_line: chunk.chunk.line_range.start,
                start_byte: chunk.chunk.byte_range.start as usize,
            })
            .collect(),
        SemanticSourceInput::BoundedFullText { text, .. } => vec![LexicalTextSegment {
            text: text.text.as_str(),
            start_line: 0,
            start_byte: 0,
        }],
    }
}

fn build_graph_records(
    document: &SourceDocument,
    lexical: &LexicalFacts,
    symbols: &[SymbolFileMapRecord],
    invalidation_key: &SemanticInvalidationKey,
) -> Vec<SemanticGraphRecord> {
    let mut records = Vec::new();
    let mut ordinal = 0usize;
    let symbol_by_name = symbols
        .iter()
        .filter_map(|symbol| {
            symbol
                .display_name
                .as_ref()
                .map(|name| (name.clone(), symbol))
        })
        .collect::<HashMap<_, _>>();

    for symbol in symbols {
        records.push(graph_record(
            document,
            GraphRecordSpec {
                kind: SemanticGraphRecordKind::Symbol,
                source_range: symbol.declaration_range,
                source_symbol: Some(symbol.symbol_id.clone()),
                target: None,
                label: "declares",
                properties: vec![
                    property("kind", &symbol.kind),
                    property("privacy", &format!("{:?}", document.identity.privacy_scope)),
                ],
            },
            invalidation_key,
            ordinal,
        ));
        ordinal += 1;

        for reference_range in &symbol.reference_ranges {
            records.push(graph_record(
                document,
                GraphRecordSpec {
                    kind: SemanticGraphRecordKind::Reference,
                    source_range: Some(*reference_range),
                    source_symbol: None,
                    target: Some(SemanticGraphEndpoint {
                        record_id: None,
                        symbol_id: Some(symbol.symbol_id.clone()),
                        file_id: Some(symbol.file_id),
                        range: symbol.declaration_range,
                    }),
                    label: "references",
                    properties: vec![property("symbol", &symbol.symbol_name_hash.value)],
                },
                invalidation_key,
                ordinal,
            ));
            ordinal += 1;
        }

        if is_type_like(&symbol.kind) || declaration_line_has_type(&symbol.display_name, lexical) {
            records.push(graph_record(
                document,
                GraphRecordSpec {
                    kind: SemanticGraphRecordKind::TypeRelation,
                    source_range: symbol.declaration_range,
                    source_symbol: Some(symbol.symbol_id.clone()),
                    target: None,
                    label: "type-context",
                    properties: vec![property("kind", &symbol.kind)],
                },
                invalidation_key,
                ordinal,
            ));
            ordinal += 1;
        }

        if is_test_symbol(symbol.display_name.as_deref().unwrap_or_default()) {
            records.push(graph_record(
                document,
                GraphRecordSpec {
                    kind: SemanticGraphRecordKind::TestLink,
                    source_range: symbol.declaration_range,
                    source_symbol: Some(symbol.symbol_id.clone()),
                    target: None,
                    label: "test-impact-source",
                    properties: vec![property("test", "true")],
                },
                invalidation_key,
                ordinal,
            ));
            ordinal += 1;
        }
    }

    for import in &lexical.imports {
        records.push(line_graph_record(
            document,
            SemanticGraphRecordKind::Import,
            import,
            "imports",
            invalidation_key,
            ordinal,
        ));
        ordinal += 1;
    }
    for export in &lexical.exports {
        records.push(line_graph_record(
            document,
            SemanticGraphRecordKind::Export,
            export,
            "exports",
            invalidation_key,
            ordinal,
        ));
        ordinal += 1;
    }
    for call in &lexical.calls {
        let target = symbol_by_name
            .get(&call.callee)
            .map(|symbol| SemanticGraphEndpoint {
                record_id: None,
                symbol_id: Some(symbol.symbol_id.clone()),
                file_id: Some(symbol.file_id),
                range: symbol.declaration_range,
            });
        records.push(graph_record(
            document,
            GraphRecordSpec {
                kind: SemanticGraphRecordKind::CallEdge,
                source_range: Some(call.range),
                source_symbol: call
                    .caller
                    .as_ref()
                    .and_then(|caller| symbol_by_name.get(caller))
                    .map(|symbol| symbol.symbol_id.clone()),
                target,
                label: "calls",
                properties: vec![property(
                    "callee_hash",
                    &symbol_name_fingerprint(&call.callee).value,
                )],
            },
            invalidation_key,
            ordinal,
        ));
        ordinal += 1;
    }
    for todo in &lexical.todo_facts {
        records.push(line_graph_record(
            document,
            SemanticGraphRecordKind::DiagnosticLink,
            todo,
            "diagnostic-link",
            invalidation_key,
            ordinal,
        ));
        ordinal += 1;
    }
    for owner in &lexical.owner_facts {
        records.push(line_graph_record(
            document,
            SemanticGraphRecordKind::OwnershipMetadata,
            owner,
            "ownership",
            invalidation_key,
            ordinal,
        ));
        ordinal += 1;
    }

    records.sort_by_key(|record| record.record_id.0.clone());
    records
}

struct GraphRecordSpec<'a> {
    kind: SemanticGraphRecordKind,
    source_range: Option<ProtocolTextRange>,
    source_symbol: Option<SemanticSymbolId>,
    target: Option<SemanticGraphEndpoint>,
    label: &'a str,
    properties: Vec<SemanticProperty>,
}

fn graph_record(
    document: &SourceDocument,
    spec: GraphRecordSpec<'_>,
    invalidation_key: &SemanticInvalidationKey,
    ordinal: usize,
) -> SemanticGraphRecord {
    let GraphRecordSpec {
        kind,
        source_range,
        source_symbol,
        target,
        label,
        properties,
    } = spec;
    let source = SemanticGraphEndpoint {
        record_id: None,
        symbol_id: source_symbol,
        file_id: Some(document.identity.file_id),
        range: source_range,
    };
    SemanticGraphRecord {
        record_id: SemanticRecordId(format!(
            "graph:{}:{}:{}:{}",
            document.identity.workspace_id.0,
            document.identity.file_id.0,
            graph_kind_label(kind),
            ordinal
        )),
        kind,
        workspace_id: document.identity.workspace_id,
        source,
        target,
        label: label.to_string(),
        properties,
        invalidation_key: invalidation_key.clone(),
        provenance: provenance(SemanticRecordSource::Lexical),
        freshness: SemanticFreshnessState::Fresh,
        schema_version: INDEX_SCHEMA_VERSION,
    }
}

fn line_graph_record(
    document: &SourceDocument,
    kind: SemanticGraphRecordKind,
    fact: &LineFact,
    label: &str,
    invalidation_key: &SemanticInvalidationKey,
    ordinal: usize,
) -> SemanticGraphRecord {
    graph_record(
        document,
        GraphRecordSpec {
            kind,
            source_range: Some(fact.range),
            source_symbol: None,
            target: None,
            label,
            properties: vec![
                property("line", &fact.line.to_string()),
                property("value_hash", &fact.value_hash.value),
                property("label", &fact.label),
            ],
        },
        invalidation_key,
        ordinal,
    )
}

/// Constructs a cancellation token descriptor for semantic work owned by this crate.
pub fn semantic_cancellation_token(
    token_id: CancellationTokenId,
    workspace_id: WorkspaceId,
    file_id: Option<FileId>,
    snapshot_id: Option<SnapshotId>,
    content_hash: Option<FileFingerprint>,
    workspace_generation: Option<WorkspaceGeneration>,
    privacy_scope: SemanticPrivacyScope,
) -> SemanticCancellationToken {
    SemanticCancellationToken {
        token_id,
        workspace_id,
        file_id,
        snapshot_id,
        content_hash,
        workspace_generation,
        privacy_scope,
        reason: None,
        issued_at: TimestampMillis::now(),
        expires_at: None,
        schema_version: INDEX_SCHEMA_VERSION,
    }
}

/// Builds a proposal-ready rename preview payload without applying it to buffers or files.
pub fn build_rename_preview_payload(
    symbol: &SymbolFileMapRecord,
    new_name: &str,
) -> devil_protocol::WorkspaceEditProposalPayload {
    let mut edits = Vec::new();
    if let Some(range) = symbol.declaration_range {
        edits.push(TextEdit {
            range: protocol_to_text_range(range),
            replacement: new_name.to_string(),
        });
    }
    for range in &symbol.reference_ranges {
        edits.push(TextEdit {
            range: protocol_to_text_range(*range),
            replacement: new_name.to_string(),
        });
    }

    let identity = FileIdentity {
        file_id: symbol.file_id,
        workspace_id: symbol.workspace_id,
        canonical_path: symbol.path.clone(),
        content_version: symbol.invalidation_key.file_content_version,
        content_hash: Some(symbol.invalidation_key.content_hash.value.clone()),
    };
    let preconditions = ProposalVersionPreconditions {
        file_version: Some(symbol.invalidation_key.file_content_version),
        buffer_version: None,
        snapshot_id: symbol.invalidation_key.snapshot_id,
        generation: Some(symbol.invalidation_key.workspace_generation),
        file_content_version: Some(symbol.invalidation_key.file_content_version),
        workspace_generation: Some(symbol.invalidation_key.workspace_generation),
        expected_fingerprint: Some(symbol.invalidation_key.content_hash.clone()),
        expected_file_length: None,
        expected_modified_at: None,
    };
    let byte_ranges = symbol
        .declaration_range
        .into_iter()
        .chain(symbol.reference_ranges.iter().copied())
        .filter_map(protocol_range_to_byte_range)
        .collect::<Vec<_>>();

    devil_protocol::WorkspaceEditProposalPayload {
        workspace_id: symbol.workspace_id,
        edit_id: deterministic_preview_uuid(symbol, new_name),
        title: format!(
            "rename {} to {new_name}",
            symbol
                .display_name
                .as_deref()
                .unwrap_or("metadata-only symbol")
        ),
        source: devil_protocol::WorkspaceEditSourceKind::SemanticRefactor,
        target_coverage: ProposalTargetCoverage {
            coverage_kind: ProposalTargetCoverageKind::Complete,
            targets: vec![ProposalAffectedTarget {
                target_id: format!("rename-target-{}", symbol.file_id.0),
                kind: ProposalTargetKind::ClosedFile,
                workspace_id: Some(symbol.workspace_id),
                file_id: Some(symbol.file_id),
                buffer_id: None,
                path: Some(symbol.path.clone()),
                terminal_session_id: None,
                plugin_id: None,
                remote_authority: None,
                collaboration_session_id: None,
                byte_ranges,
                redaction_hints: vec![RedactionHint::MetadataOnly],
            }],
            omitted_target_count: 0,
            redaction_hints: vec![RedactionHint::MetadataOnly],
        },
        file_edits: vec![WorkspaceTextEdit {
            file: identity,
            buffer_id: None,
            edits: EditBatch { edits },
            preconditions,
        }],
        file_operations: Vec::new(),
        required_capability: CapabilityId("editor.write".to_string()),
        diagnostics: Vec::new(),
        schema_version: INDEX_SCHEMA_VERSION,
    }
}

fn result_from_symbol(
    symbol: &SymbolFileMapRecord,
    request: &SemanticQueryRequest,
    ordinal: usize,
    kind: SemanticQueryResultKind,
    range_override: Option<ProtocolTextRange>,
) -> SemanticQueryResult {
    SemanticQueryResult {
        result_id: SemanticRecordId(format!("query:{}:{}", symbol.symbol_id.0, ordinal)),
        kind,
        label: symbol
            .display_name
            .clone()
            .unwrap_or_else(|| symbol.symbol_name_hash.value.clone()),
        file_id: Some(symbol.file_id),
        path: Some(symbol.path.clone()),
        range: range_override.or(symbol.declaration_range),
        score_basis_points: score_for_ordinal(ordinal),
        freshness: SemanticFreshness {
            state: SemanticFreshnessState::Fresh,
            key: symbol.invalidation_key.clone(),
            degraded_reasons: Vec::new(),
            observed_at: TimestampMillis::now(),
        },
        provenance: symbol.provenance.clone(),
        related_record_ids: Vec::new(),
        proposal_preview: if request.kind == SemanticQueryKind::RefactoringPreview {
            Some(ProposalPayloadSummary {
                kind: ProposalPayloadKind::WorkspaceEdit,
                affected_files: vec![symbol.file_id],
                title: Some("semantic refactoring preview".to_string()),
                byte_count: None,
            })
        } else {
            None
        },
    }
}

fn result_from_graph(
    record: &SemanticGraphRecord,
    _request: &SemanticQueryRequest,
    ordinal: usize,
) -> SemanticQueryResult {
    SemanticQueryResult {
        result_id: record.record_id.clone(),
        kind: SemanticQueryResultKind::GraphRecord,
        label: record.label.clone(),
        file_id: record.source.file_id,
        path: None,
        range: record.source.range,
        score_basis_points: score_for_ordinal(ordinal),
        freshness: SemanticFreshness {
            state: record.freshness,
            key: record.invalidation_key.clone(),
            degraded_reasons: Vec::new(),
            observed_at: TimestampMillis::now(),
        },
        provenance: record.provenance.clone(),
        related_record_ids: record
            .target
            .as_ref()
            .and_then(|target| target.record_id.clone())
            .into_iter()
            .collect(),
        proposal_preview: None,
    }
}

fn symbol_in_scope(symbol: &SymbolFileMapRecord, request: &SemanticQueryRequest) -> bool {
    symbol.workspace_id == request.scope.workspace_id
        && (request.scope.file_ids.is_empty() || request.scope.file_ids.contains(&symbol.file_id))
        && (request.scope.paths.is_empty() || request.scope.paths.contains(&symbol.path))
        && (request.scope.language_ids.is_empty()
            || request.scope.language_ids.contains(&symbol.language_id))
        && privacy_visible(
            symbol.invalidation_key.privacy_scope,
            request.scope.privacy_scope,
        )
}

fn graph_in_scope(record: &SemanticGraphRecord, request: &SemanticQueryRequest) -> bool {
    record.workspace_id == request.scope.workspace_id
        && record.source.file_id.is_none_or(|file_id| {
            request.scope.file_ids.is_empty() || request.scope.file_ids.contains(&file_id)
        })
        && privacy_visible(
            record.invalidation_key.privacy_scope,
            request.scope.privacy_scope,
        )
}

fn privacy_visible(record: SemanticPrivacyScope, requested: SemanticPrivacyScope) -> bool {
    if requested == SemanticPrivacyScope::Redacted {
        return true;
    }
    record == requested
        || requested == SemanticPrivacyScope::Workspace
        || requested == SemanticPrivacyScope::Project
}

fn score_for_ordinal(ordinal: usize) -> u16 {
    10_000u16.saturating_sub((ordinal as u16).saturating_mul(50))
}

fn declaration_from_line(trimmed: &str, leading: usize) -> Option<(String, String, usize)> {
    let normalized = trimmed
        .strip_prefix("pub ")
        .or_else(|| trimmed.strip_prefix("pub(crate) "))
        .or_else(|| trimmed.strip_prefix("export "))
        .unwrap_or(trimmed);
    let base_adjust = trimmed.len().saturating_sub(normalized.len());

    for (keyword, kind) in [
        ("fn", "function"),
        ("async fn", "function"),
        ("function", "function"),
        ("def", "function"),
        ("class", "class"),
        ("struct", "struct"),
        ("enum", "enum"),
        ("trait", "trait"),
        ("interface", "interface"),
        ("type", "type"),
        ("mod", "module"),
        ("const", "constant"),
        ("static", "static"),
        ("let", "variable"),
        ("var", "variable"),
    ] {
        if let Some(rest) = normalized.strip_prefix(keyword)
            && rest.chars().next().is_some_and(char::is_whitespace)
        {
            let rest_start = leading + base_adjust + keyword.len() + 1;
            let name = first_identifier(rest.trim_start())?;
            let local = normalized.find(&name)? + base_adjust + leading;
            return Some((
                kind.to_string(),
                name,
                local.max(rest_start.saturating_sub(1)),
            ));
        }
    }

    if let Some(rest) = normalized.strip_prefix("impl ") {
        let name = first_identifier(rest)?;
        let local = normalized.find(&name)? + base_adjust + leading;
        return Some(("implementation".to_string(), name, local));
    }

    None
}

fn call_facts_from_line(
    line: &str,
    line_number: u32,
    base_byte: usize,
    current_scope: Option<&str>,
) -> Vec<CallFact> {
    let mut facts = Vec::new();
    let bytes = line.as_bytes();
    let mut index = 0;
    while index < bytes.len() {
        if is_identifier_start(bytes[index] as char) {
            let start = index;
            index += 1;
            while index < bytes.len() && is_identifier_continue(bytes[index] as char) {
                index += 1;
            }
            let name = &line[start..index];
            let after = line[index..].trim_start();
            if after.starts_with('(') && !is_call_keyword(name) {
                facts.push(CallFact {
                    caller: current_scope.map(ToString::to_string),
                    callee: name.to_string(),
                    range: range_for_cols(line_number, start, index, base_byte),
                });
            }
        } else {
            index += 1;
        }
    }
    facts
}

fn tokenize_line(line: &str, line_number: u32, base_byte: usize) -> Vec<TokenFact> {
    let mut tokens = Vec::new();
    let bytes = line.as_bytes();
    let mut index = 0;
    while index < bytes.len() {
        if is_identifier_start(bytes[index] as char) {
            let start = index;
            index += 1;
            while index < bytes.len() && is_identifier_continue(bytes[index] as char) {
                index += 1;
            }
            tokens.push(TokenFact {
                text: line[start..index].to_string(),
                range: range_for_cols(line_number, start, index, base_byte),
            });
        } else {
            index += 1;
        }
    }
    tokens
}

fn line_fact(label: &str, line: &str, line_number: u32, base_byte: usize) -> LineFact {
    LineFact {
        label: label.to_string(),
        value_hash: content_fingerprint(line.as_bytes()),
        range: range_for_cols(line_number, 0, line.len(), base_byte),
        line: line_number,
    }
}

fn is_import_line(trimmed: &str) -> bool {
    trimmed.starts_with("use ")
        || trimmed.starts_with("mod ")
        || trimmed.starts_with("extern crate ")
        || trimmed.starts_with("import ")
        || trimmed.starts_with("from ")
        || trimmed.contains("require(")
}

fn is_export_line(trimmed: &str) -> bool {
    trimmed.starts_with("pub ")
        || trimmed.starts_with("export ")
        || trimmed.starts_with("module.exports")
        || trimmed.starts_with("exports.")
}

fn is_identifier_start(ch: char) -> bool {
    ch == '_' || ch.is_ascii_alphabetic()
}

fn is_identifier_continue(ch: char) -> bool {
    ch == '_' || ch.is_ascii_alphanumeric()
}

fn is_call_keyword(name: &str) -> bool {
    matches!(
        name,
        "if" | "while" | "for" | "match" | "switch" | "return" | "fn" | "function"
    )
}

fn first_identifier(input: &str) -> Option<String> {
    let mut chars = input.char_indices().peekable();
    while let Some((_, ch)) = chars.peek().copied() {
        if is_identifier_start(ch) {
            break;
        }
        chars.next();
    }
    let (start, first) = chars.next()?;
    if !is_identifier_start(first) {
        return None;
    }
    let mut end = start + first.len_utf8();
    for (index, ch) in chars {
        if is_identifier_continue(ch) {
            end = index + ch.len_utf8();
        } else {
            break;
        }
    }
    Some(input[start..end].to_string())
}

fn is_type_like(kind: &str) -> bool {
    matches!(
        kind,
        "struct" | "class" | "enum" | "trait" | "interface" | "type"
    )
}

fn declaration_line_has_type(display_name: &Option<String>, lexical: &LexicalFacts) -> bool {
    let Some(name) = display_name else {
        return false;
    };
    lexical
        .declarations
        .iter()
        .any(|candidate| &candidate.name == name && candidate.has_type_hint)
}

fn is_test_symbol(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    lower.starts_with("test_") || lower.ends_with("_test") || lower.contains("test")
}

fn range_for_cols(
    line: u32,
    start_col: usize,
    end_col: usize,
    line_start_byte: usize,
) -> ProtocolTextRange {
    ProtocolTextRange {
        start: TextCoordinate {
            line,
            character: start_col as u32,
            byte_offset: Some((line_start_byte + start_col) as u64),
            utf16_offset: Some(start_col as u64),
        },
        end: TextCoordinate {
            line,
            character: end_col as u32,
            byte_offset: Some((line_start_byte + end_col) as u64),
            utf16_offset: Some(end_col as u64),
        },
    }
}

fn range_contains(range: ProtocolTextRange, position: TextCoordinate) -> bool {
    if position.line < range.start.line || position.line > range.end.line {
        return false;
    }
    if range.start.line == range.end.line {
        return position.character >= range.start.character
            && position.character < range.end.character;
    }
    if position.line == range.start.line {
        return position.character >= range.start.character;
    }
    if position.line == range.end.line {
        return position.character < range.end.character;
    }
    true
}

fn ranges_equal(left: ProtocolTextRange, right: ProtocolTextRange) -> bool {
    left.start.line == right.start.line
        && left.start.character == right.start.character
        && left.end.line == right.end.line
        && left.end.character == right.end.character
}

fn protocol_to_text_range(range: ProtocolTextRange) -> TextRange {
    TextRange::new(
        TextOffset::byte(range.start.byte_offset.unwrap_or(0)),
        TextOffset::byte(
            range
                .end
                .byte_offset
                .unwrap_or(range.start.byte_offset.unwrap_or(0)),
        ),
    )
}

fn protocol_range_to_byte_range(range: ProtocolTextRange) -> Option<ByteRange> {
    Some(ByteRange::new(
        range.start.byte_offset?,
        range.end.byte_offset?,
    ))
}

fn content_fingerprint(bytes: &[u8]) -> FileFingerprint {
    FileFingerprint {
        algorithm: "fnv1a64-devil-index-v1".to_string(),
        value: format!("{:016x}", hash64(bytes, FNV_OFFSET)),
    }
}

fn discovery_file_id(record: &WorkspaceDiscoveryRecord) -> Option<FileId> {
    record
        .identity
        .as_ref()
        .map(|identity| identity.file_id)
        .or_else(|| {
            record
                .metadata
                .as_ref()
                .and_then(|metadata| metadata.file_id)
        })
}

fn discovery_workspace_id(record: &WorkspaceDiscoveryRecord) -> Option<WorkspaceId> {
    record
        .workspace_id
        .or_else(|| {
            record
                .identity
                .as_ref()
                .map(|identity| identity.workspace_id)
        })
        .or_else(|| {
            record
                .metadata
                .as_ref()
                .and_then(|metadata| metadata.workspace_id)
        })
}

fn discovery_path(record: &WorkspaceDiscoveryRecord) -> Option<CanonicalPath> {
    record
        .path
        .clone()
        .or_else(|| {
            record
                .identity
                .as_ref()
                .map(|identity| identity.canonical_path.clone())
        })
        .or_else(|| {
            record
                .metadata
                .as_ref()
                .map(|metadata| metadata.canonical_path.clone())
        })
}

fn discovery_content_version(record: &WorkspaceDiscoveryRecord) -> FileContentVersion {
    record
        .identity
        .as_ref()
        .map(|identity| identity.content_version)
        .or_else(|| {
            record
                .metadata
                .as_ref()
                .and_then(|metadata| metadata.content_version)
        })
        .unwrap_or(FileContentVersion(0))
}

fn semantic_identity_from_discovery(
    record: &WorkspaceDiscoveryRecord,
) -> Option<SemanticFileFingerprintIdentity> {
    if record.policy.decision != WorkspaceDiscoveryDecision::ContentAllowed {
        return None;
    }
    Some(SemanticFileFingerprintIdentity {
        workspace_id: discovery_workspace_id(record)?,
        file_id: discovery_file_id(record)?,
        canonical_path: discovery_path(record)?,
        file_content_version: discovery_content_version(record),
        workspace_generation: record.workspace_generation,
        content_hash: record
            .content_hash
            .clone()
            .or_else(|| record.content_fingerprint.clone())?,
        disk_fingerprint: record.content_fingerprint.clone(),
        byte_len: record
            .metadata
            .as_ref()
            .and_then(|metadata| metadata.size_bytes),
        modified_at: record
            .metadata
            .as_ref()
            .and_then(|metadata| metadata.modified_at),
        privacy_scope: record.privacy_scope,
        schema_version: INDEX_SCHEMA_VERSION,
    })
}

fn symbol_name_fingerprint(name: &str) -> FileFingerprint {
    FileFingerprint {
        algorithm: "fnv1a64-devil-symbol-name-v1".to_string(),
        value: format!("{:016x}", hash64(name.as_bytes(), 0x1234_5678_9abc_def0)),
    }
}

fn hash64(bytes: &[u8], seed: u64) -> u64 {
    let mut hash = FNV_OFFSET ^ seed;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    hash
}

fn hash_to_u128(bytes: &[u8], seed: u64) -> u128 {
    let high = hash64(bytes, seed) as u128;
    let low = hash64(bytes, seed.rotate_left(17)) as u128;
    (high << 64) | low
}

fn symbol_id(document: &SourceDocument, candidate: &SymbolCandidate) -> SemanticSymbolId {
    SemanticSymbolId(format!(
        "sym:{}:{}:{}:{}",
        document.identity.workspace_id.0,
        document.identity.file_id.0,
        symbol_name_fingerprint(&candidate.name).value,
        candidate.range.start.byte_offset.unwrap_or(0)
    ))
}

fn display_name_for_scope(name: &str, privacy_scope: SemanticPrivacyScope) -> Option<String> {
    match privacy_scope {
        SemanticPrivacyScope::MetadataOnly | SemanticPrivacyScope::Redacted => None,
        _ => Some(name.to_string()),
    }
}

fn provenance(source: SemanticRecordSource) -> SemanticRecordProvenance {
    SemanticRecordProvenance {
        source,
        server_id: None,
        extraction_version: LEXICAL_EXTRACTION_VERSION.to_string(),
        confidence_basis_points: 7_500,
    }
}

fn property(key: &str, value: &str) -> SemanticProperty {
    SemanticProperty {
        key: key.to_string(),
        value: value.to_string(),
        redaction: RedactionHint::MetadataOnly,
    }
}

fn diagnostic(
    code: &str,
    message: &str,
    severity: ProtocolDiagnosticSeverity,
    path: Option<CanonicalPath>,
    range: Option<ProtocolTextRange>,
) -> ProtocolDiagnostic {
    ProtocolDiagnostic {
        code: code.to_string(),
        message: message.to_string(),
        severity,
        path,
        range,
    }
}

fn graph_kind_label(kind: SemanticGraphRecordKind) -> &'static str {
    match kind {
        SemanticGraphRecordKind::Symbol => "symbol",
        SemanticGraphRecordKind::Reference => "reference",
        SemanticGraphRecordKind::Import => "import",
        SemanticGraphRecordKind::Export => "export",
        SemanticGraphRecordKind::CallEdge => "call",
        SemanticGraphRecordKind::TypeRelation => "type",
        SemanticGraphRecordKind::TestLink => "test",
        SemanticGraphRecordKind::DiagnosticLink => "diagnostic",
        SemanticGraphRecordKind::OwnershipMetadata => "ownership",
    }
}

fn find_case_insensitive(haystack: &str, needle: &str) -> Option<usize> {
    haystack
        .to_ascii_lowercase()
        .find(&needle.to_ascii_lowercase())
}

fn deterministic_preview_uuid(symbol: &SymbolFileMapRecord, new_name: &str) -> uuid::Uuid {
    let mut bytes = [0u8; 16];
    let hash = hash_to_u128(
        format!("{}:{}", symbol.symbol_id.0, new_name).as_bytes(),
        0x0ddc_0ffe_e15e_d000,
    );
    bytes.copy_from_slice(&hash.to_be_bytes());
    bytes[6] = (bytes[6] & 0x0f) | 0x40;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    uuid::Uuid::from_bytes(bytes)
}
