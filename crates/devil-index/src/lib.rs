//! Indexing Engine: actor-owned semantic scheduling, repository discovery,
//! lexical symbol maps, deterministic parser-cache fallbacks, and pure query DTOs.

#![warn(missing_docs)]

use std::collections::{HashMap, VecDeque};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use devil_protocol::{
    ByteRange, CancellationTokenId, CanonicalPath, CapabilityId, EditBatch, FileContentVersion,
    FileFingerprint, FileId, FileIdentity, LanguageId, ProposalAffectedTarget, ProposalPayloadKind,
    ProposalPayloadSummary, ProposalTargetCoverage, ProposalTargetCoverageKind, ProposalTargetKind,
    ProposalVersionPreconditions, ProtocolDiagnostic, ProtocolDiagnosticSeverity,
    ProtocolTextRange, RedactionHint, SemanticCancellationReason, SemanticCancellationToken,
    SemanticFileFingerprintIdentity, SemanticFreshness, SemanticFreshnessState,
    SemanticGrammarVersion, SemanticGraphEndpoint, SemanticGraphRecord, SemanticGraphRecordKind,
    SemanticInvalidationKey, SemanticModelVersion, SemanticPrivacyScope, SemanticProperty,
    SemanticQueryFreshnessPolicy, SemanticQueryKind, SemanticQueryRequest, SemanticQueryResponse,
    SemanticQueryResult, SemanticQueryResultKind, SemanticQueryStatus, SemanticRecordId,
    SemanticRecordProvenance, SemanticRecordSource, SemanticSymbolId, SnapshotId,
    SymbolFileMapRecord, TextCoordinate, TextEdit, TextOffset, TextRange, TimestampMillis,
    WorkspaceGeneration, WorkspaceId, WorkspaceTextEdit,
};
use devil_text::TextSnapshot;
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

/// Configuration for deterministic repository discovery.
#[derive(Debug, Clone)]
pub struct RepositoryScanConfig {
    /// Root directory to scan.
    pub root: PathBuf,
    /// Workspace receiving discovered file identities.
    pub workspace_id: WorkspaceId,
    /// Workspace generation attached to emitted identities.
    pub workspace_generation: WorkspaceGeneration,
    /// Privacy scope attached to discovered file identities.
    pub privacy_scope: SemanticPrivacyScope,
    /// Maximum number of files to emit.
    pub max_files: usize,
    /// Maximum directory depth relative to the root.
    pub max_depth: usize,
    /// Maximum file size in bytes to fingerprint.
    pub max_file_bytes: u64,
    /// Deterministic ignore patterns and path segments.
    pub ignore_patterns: Vec<String>,
}

impl RepositoryScanConfig {
    /// Constructs a scan config with conservative deterministic bounds.
    pub fn new(root: impl Into<PathBuf>, workspace_id: WorkspaceId) -> Self {
        Self {
            root: root.into(),
            workspace_id,
            workspace_generation: WorkspaceGeneration(1),
            privacy_scope: SemanticPrivacyScope::Workspace,
            max_files: 10_000,
            max_depth: 64,
            max_file_bytes: 2 * 1024 * 1024,
            ignore_patterns: Self::default_ignore_patterns(),
        }
    }

    /// Returns the built-in ignore patterns used by repository discovery.
    pub fn default_ignore_patterns() -> Vec<String> {
        [
            ".git",
            ".hg",
            ".svn",
            "target",
            "node_modules",
            ".idea",
            ".gitignore",
            ".DS_Store",
            "*.tmp",
            "*.log",
        ]
        .iter()
        .map(|pattern| (*pattern).to_string())
        .collect()
    }
}

/// Repository file discovered by a bounded scan.
#[derive(Debug, Clone)]
pub struct RepositoryFileRecord {
    /// Identity used for semantic invalidation.
    pub identity: SemanticFileFingerprintIdentity,
    /// Root-relative slash-normalized path.
    pub relative_path: String,
    /// Language inferred from the file extension.
    pub language_id: LanguageId,
}

/// Output from deterministic repository discovery.
#[derive(Debug, Clone)]
pub struct RepositoryScanOutput {
    /// Canonicalized root path that was scanned.
    pub root: CanonicalPath,
    /// Files emitted in deterministic path order.
    pub files: Vec<RepositoryFileRecord>,
    /// Ignored root-relative paths.
    pub ignored_paths: Vec<String>,
    /// Number of files omitted because traversal bounds were reached.
    pub omitted_file_count: u32,
    /// Diagnostics emitted by bounded traversal.
    pub diagnostics: Vec<ProtocolDiagnostic>,
}

/// Deterministic filesystem repository scanner with built-in ignore and bound handling.
#[derive(Debug, Default, Clone, Copy)]
pub struct RepositoryScanner;

impl RepositoryScanner {
    /// Constructs a scanner.
    pub const fn new() -> Self {
        Self
    }

    /// Scans a repository according to the supplied bounded configuration.
    pub fn scan(&self, config: &RepositoryScanConfig) -> IndexResult<RepositoryScanOutput> {
        if config.max_depth == 0 {
            return Err(IndexError::InvalidConfig {
                message: "max_depth must be greater than zero".to_string(),
            });
        }

        let root = fs::canonicalize(&config.root).map_err(|err| IndexError::Io {
            path: normalize_path_string(&config.root),
            message: err.to_string(),
        })?;
        let mut patterns = config.ignore_patterns.clone();
        patterns.extend(read_gitignore_patterns(&root));

        let mut output = RepositoryScanOutput {
            root: CanonicalPath(normalize_path_string(&root)),
            files: Vec::new(),
            ignored_paths: Vec::new(),
            omitted_file_count: 0,
            diagnostics: Vec::new(),
        };

        self.walk_dir(&root, &root, 0, config, &patterns, &mut output)?;
        output.files.sort_by(|left, right| {
            left.relative_path.cmp(&right.relative_path).then_with(|| {
                left.identity
                    .content_hash
                    .value
                    .cmp(&right.identity.content_hash.value)
            })
        });
        output.ignored_paths.sort();
        Ok(output)
    }

    fn walk_dir(
        &self,
        root: &Path,
        dir: &Path,
        depth: usize,
        config: &RepositoryScanConfig,
        patterns: &[String],
        output: &mut RepositoryScanOutput,
    ) -> IndexResult<()> {
        let _scanner = self;
        if depth > config.max_depth {
            output.diagnostics.push(diagnostic(
                "index.scan.depth_bound",
                "repository traversal depth bound reached",
                ProtocolDiagnosticSeverity::Warning,
                Some(CanonicalPath(normalize_path_string(dir))),
                None,
            ));
            return Ok(());
        }

        let mut entries = fs::read_dir(dir)
            .map_err(|err| IndexError::Io {
                path: normalize_path_string(dir),
                message: err.to_string(),
            })?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|err| IndexError::Io {
                path: normalize_path_string(dir),
                message: err.to_string(),
            })?;
        entries.sort_by_key(|entry| normalize_path_string(entry.path()));

        for entry in entries {
            let path = entry.path();
            let relative = path
                .strip_prefix(root)
                .map_or_else(|_| normalize_path_string(&path), normalize_path_string);
            let metadata = entry.metadata().map_err(|err| IndexError::Io {
                path: normalize_path_string(&path),
                message: err.to_string(),
            })?;

            if matches_ignore(&relative, metadata.is_dir(), patterns) {
                output.ignored_paths.push(relative);
                continue;
            }

            if metadata.is_dir() {
                self.walk_dir(
                    root,
                    &path,
                    depth.saturating_add(1),
                    config,
                    patterns,
                    output,
                )?;
                continue;
            }

            if !metadata.is_file() {
                continue;
            }

            if output.files.len() >= config.max_files {
                output.omitted_file_count = output.omitted_file_count.saturating_add(1);
                continue;
            }
            if metadata.len() > config.max_file_bytes {
                output.omitted_file_count = output.omitted_file_count.saturating_add(1);
                output.diagnostics.push(diagnostic(
                    "index.scan.file_size_bound",
                    "file exceeded repository scan byte bound",
                    ProtocolDiagnosticSeverity::Warning,
                    Some(CanonicalPath(normalize_path_string(&path))),
                    None,
                ));
                continue;
            }

            let bytes = fs::read(&path).map_err(|err| IndexError::Io {
                path: normalize_path_string(&path),
                message: err.to_string(),
            })?;
            let canonical_path = fs::canonicalize(&path).unwrap_or(path.clone());
            let canonical = CanonicalPath(normalize_path_string(&canonical_path));
            let content_hash = content_fingerprint(&bytes);
            let modified_at = metadata.modified().ok().map(system_time_to_millis);
            let file_id = FileId(hash_to_u128(canonical.0.as_bytes(), 0x5eed_cafe_d00d_f00d));

            output.files.push(RepositoryFileRecord {
                identity: SemanticFileFingerprintIdentity {
                    workspace_id: config.workspace_id,
                    file_id,
                    canonical_path: canonical,
                    file_content_version: FileContentVersion(1),
                    workspace_generation: config.workspace_generation,
                    content_hash: content_hash.clone(),
                    disk_fingerprint: Some(content_hash),
                    byte_len: Some(bytes.len() as u64),
                    modified_at,
                    privacy_scope: config.privacy_scope,
                    schema_version: INDEX_SCHEMA_VERSION,
                },
                relative_path: relative.clone(),
                language_id: language_for_path(Path::new(&relative)),
            });
        }

        Ok(())
    }
}

/// Immutable source document copy owned by indexing work.
#[derive(Debug, Clone)]
pub struct SourceDocument {
    /// Semantic file identity for invalidation.
    pub identity: SemanticFileFingerprintIdentity,
    /// Optional live snapshot id when sourced from editor-owned state.
    pub snapshot_id: Option<SnapshotId>,
    /// Language identifier.
    pub language_id: LanguageId,
    /// Full source text copy owned by the indexing work item.
    pub text: String,
}

impl SourceDocument {
    /// Constructs a document with default version and workspace generation values.
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

    /// Constructs a document with explicit version, generation, snapshot, and privacy metadata.
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
        Self {
            identity: SemanticFileFingerprintIdentity {
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
            },
            snapshot_id,
            language_id,
            text,
        }
    }

    /// Copies a bounded text snapshot into a source document for indexing.
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
        Ok(Self::with_versions(
            workspace_id,
            file_id,
            canonical_path,
            language_id,
            file_content_version,
            workspace_generation,
            Some(snapshot.snapshot_id()),
            privacy_scope,
            text,
        ))
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
            schema_version: INDEX_SCHEMA_VERSION,
        }
    }
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
    /// Content hash of the parsed document.
    pub content_hash: FileFingerprint,
    /// Language identifier.
    pub language_id: LanguageId,
    /// Grammar version.
    pub grammar_version: SemanticGrammarVersion,
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

/// Parser cache keyed by content hash, language id, and grammar version.
#[derive(Debug, Default, Clone)]
pub struct SyntaxTreeCache {
    entries: HashMap<SyntaxCacheKey, ParseOutcome>,
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

    /// Returns `true` when the syntax cache is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Inserts a parser outcome into the cache.
    pub fn insert(&mut self, outcome: ParseOutcome) {
        self.entries
            .insert(outcome.syntax_tree.cache_key.clone(), outcome);
    }

    /// Returns a cached parse outcome or invokes the provided worker.
    pub fn get_or_parse<W: ParserWorker>(
        &mut self,
        worker: &W,
        request: ParseRequest,
    ) -> IndexResult<ParseOutcome> {
        let key = SyntaxCacheKey {
            content_hash: request.document.identity.content_hash.clone(),
            language_id: request.document.language_id.clone(),
            grammar_version: request.grammar_version.clone(),
        };

        if let Some(outcome) = self.entries.get(&key) {
            return Ok(outcome.clone());
        }

        let outcome = worker.parse(request)?;
        self.entries.insert(key, outcome.clone());
        Ok(outcome)
    }

    /// Removes all cache entries for a grammar version.
    pub fn invalidate_grammar(&mut self, grammar_version: &SemanticGrammarVersion) -> usize {
        let before = self.entries.len();
        self.entries
            .retain(|key, _| &key.grammar_version != grammar_version);
        before.saturating_sub(self.entries.len())
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
    /// Syntax tree/cache record.
    pub syntax_tree: SyntaxTreeRecord,
    /// Lexical symbol-to-file map records.
    pub symbols: Vec<SymbolFileMapRecord>,
    /// Normalized semantic graph records.
    pub graph_records: Vec<SemanticGraphRecord>,
    /// Diagnostics emitted during extraction.
    pub diagnostics: Vec<ProtocolDiagnostic>,
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
        let provenance = provenance(SemanticRecordSource::Lexical);
        let lexical = extract_lexical_facts(document);
        let syntax_tree = SyntaxTreeRecord {
            cache_key: SyntaxCacheKey {
                content_hash: document.identity.content_hash.clone(),
                language_id: document.language_id.clone(),
                grammar_version,
            },
            identity: document.identity.clone(),
            node_count: lexical.token_count,
            declaration_count: lexical.declarations.len(),
            freshness: SemanticFreshnessState::Fresh,
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
    source_line: String,
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

fn extract_lexical_facts(document: &SourceDocument) -> LexicalFacts {
    let mut declarations = Vec::new();
    let mut imports = Vec::new();
    let mut exports = Vec::new();
    let mut diagnostics = Vec::new();
    let mut owner_facts = Vec::new();
    let mut todo_facts = Vec::new();
    let mut tokens = Vec::new();
    let mut calls = Vec::new();
    let mut byte_cursor = 0usize;
    let mut current_scope: Option<String> = None;

    for (line_index, line) in document.text.lines().enumerate() {
        let line_number = line_index as u32;
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
                source_line: trimmed.to_string(),
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
    lexical.declarations.iter().any(|candidate| {
        &candidate.name == name
            && (candidate.source_line.contains(" -> ") || candidate.source_line.contains(':'))
    })
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

fn read_gitignore_patterns(root: &Path) -> Vec<String> {
    let path = root.join(".gitignore");
    fs::read_to_string(path).map_or_else(
        |_| Vec::new(),
        |text| {
            text.lines()
                .map(str::trim)
                .filter(|line| !line.is_empty() && !line.starts_with('#'))
                .map(|line| line.trim_end_matches('/').to_string())
                .collect()
        },
    )
}

fn matches_ignore(relative: &str, is_dir: bool, patterns: &[String]) -> bool {
    let normalized = relative.replace('\\', "/");
    let file_name = normalized.rsplit('/').next().unwrap_or(&normalized);
    patterns.iter().any(|pattern| {
        let pattern = pattern.trim().trim_end_matches('/');
        if pattern.is_empty() {
            return false;
        }
        if let Some(suffix) = pattern.strip_prefix('*') {
            return normalized.ends_with(suffix);
        }
        if pattern.contains('/') {
            return normalized == pattern || normalized.starts_with(&format!("{pattern}/"));
        }
        file_name == pattern || (is_dir && normalized.split('/').any(|part| part == pattern))
    })
}

fn normalize_path_string(path: impl AsRef<Path>) -> String {
    path.as_ref().to_string_lossy().replace('\\', "/")
}

fn language_for_path(path: &Path) -> LanguageId {
    let language = match path.extension().and_then(|extension| extension.to_str()) {
        Some("rs") => "rust",
        Some("ts") => "typescript",
        Some("tsx") => "typescriptreact",
        Some("js") => "javascript",
        Some("jsx") => "javascriptreact",
        Some("py") => "python",
        Some("go") => "go",
        Some("java") => "java",
        Some("kt") => "kotlin",
        Some("cpp" | "cc" | "cxx" | "hpp" | "h") => "cpp",
        Some("c") => "c",
        Some("md") => "markdown",
        Some("toml") => "toml",
        Some("json") => "json",
        Some("yaml" | "yml") => "yaml",
        _ => "text",
    };
    LanguageId(language.to_string())
}

fn system_time_to_millis(time: SystemTime) -> TimestampMillis {
    TimestampMillis(
        time.duration_since(UNIX_EPOCH)
            .map_or(0, |duration| duration.as_millis() as u64),
    )
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
