//! Route an operator-configured, broker-approved external formatter into the
//! existing reviewable-proposal path.
//!
//! Three pieces landed separately and this module is the only thing that joins
//! them:
//!
//! * bounded stdin on [`legion_platform::ProcessService::execute_bounded`]
//!   (`ProcessRequest::stdin`, capped at [`MAX_BOUNDED_STDIN_BYTES`]),
//! * [`super::approve_formatter_executable`], which is the only way to obtain
//!   fresh broker authority plus canonical identity for a formatter binary,
//! * `AppComposition::configure_python_toolchain`, which records the operator's
//!   explicit formatter path and one exact-binary allowance for it.
//!
//! # What this route will not do
//!
//! * **It never writes.** The buffer snapshot goes to the child on stdin. No
//!   temporary file, no spool, no `--in-place`, and the workspace path is never
//!   handed to the child as an argument. Everything the formatter produces
//!   becomes a *proposal*, reviewed through the same `workspace/applyEdit`
//!   arbitration as an LSP formatting response.
//! * **It never fabricates a proposal.** The only text that can become an edit
//!   is the child's own stdout, read only after its exit status matched
//!   [`PYTHON_FORMATTER_EXPECTED_EXIT_CODE`]. An unavailable capability is an
//!   explicit `Failed` operation naming what is missing; output identical to the
//!   input is a terminal no-op with no proposal id, never an empty proposal.
//! * **It never guesses the invocation.** [`PYTHON_FORMATTER_STDIN_ARGS`] and
//!   [`PYTHON_FORMATTER_EXPECTED_EXIT_CODE`] are explicit constants documented
//!   against one named CLI convention. Nothing here branches on the
//!   executable's file name, stem or extension: guessing wrong would silently
//!   corrupt a file through a proposal the user trusted.
//! * **It never retains child stderr.** The bounded result is destructured so
//!   that only `exit_code` and `stdout` are ever bound to a name; no error, no
//!   operation message and no proposal field can carry the child's diagnostics.
//!
//! # Source-kind naming compromise
//!
//! [`WorkspaceEditSourceKind`] lives in `crates/legion-protocol/src/lib.rs`, a
//! frozen chokepoint, so the proposal is recorded as
//! [`WorkspaceEditSourceKind::LspFormatting`] even though no language server
//! produced it. The true provenance is carried where a reviewer actually reads
//! it: the proposal title names the tool, the detail tag is
//! [`EXTERNAL_FORMATTER_DETAIL_TAG`], and the detail rows say the bytes came
//! from an external process. A dedicated source kind is a follow-up that must
//! extract the enum from the chokepoint first.

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::time::{Duration, Instant};

use legion_platform::{
    BoundedProcessRequest, MAX_BOUNDED_STDIN_BYTES, NativeProcessService, PlatformError,
    ProcessRequest, ProcessService,
};
use thiserror::Error;

use crate::*;

use super::lsp_reads::LspWriteSideSpec;
use super::{
    FormatterApprovalError, FormatterApprovalRequest, FormatterProbe, PendingLspWriteOperation,
    approve_formatter_executable, is_stale_response,
};

/// Argument vector handed to a configured Python formatter.
///
/// This encodes exactly one CLI convention and nothing else: **read the whole
/// document from standard input, write the whole formatted document to standard
/// output, and change nothing on disk.** `-` is the spelling that convention
/// uses for "the input file is stdin"; it is the same single argument for every
/// formatter that honours the convention.
///
/// It is a constant, not a guess. Deriving an argument vector from the
/// executable's file name would mean a renamed, wrapped or shimmed binary
/// silently gets somebody else's flags — and a formatter invoked with the wrong
/// flags can rewrite a file in place, or print something that is not the
/// formatted document, either of which corrupts a file through a proposal the
/// user trusted. A formatter that does not honour this convention must not be
/// configured as a Legion formatter.
pub const PYTHON_FORMATTER_STDIN_ARGS: &[&str] = &["-"];

/// Exit status a configured Python formatter must report for its stdout to be
/// treated as the formatted document.
///
/// Part of the same explicit contract as [`PYTHON_FORMATTER_STDIN_ARGS`]: under
/// the stdin/stdout convention, `0` means "the formatted document is on
/// stdout". Any other status means stdout is a diagnostic, a partial write, or
/// nothing at all, and it is discarded unread.
pub const PYTHON_FORMATTER_EXPECTED_EXIT_CODE: i32 = 0;

/// Largest in-memory document this route will hand to a formatter.
///
/// Half of the process-service transport limit
/// ([`MAX_BOUNDED_STDIN_BYTES`], 8 MiB), which leaves the same amount of room
/// again for a formatted document that grew. A larger buffer fails closed
/// before the broker is asked and before any child exists.
pub const PYTHON_FORMATTER_MAX_DOCUMENT_BYTES: usize = MAX_BOUNDED_STDIN_BYTES / 2;

/// Bytes of child stdout retained before the run is failed.
///
/// Sized at the transport limit so a formatted document may be up to twice the
/// input and still arrive whole. The bounded runner reports an overflow as an
/// error rather than a truncated success, so a document larger than this is a
/// failed operation and never a silently clipped proposal.
pub const PYTHON_FORMATTER_STDOUT_LIMIT: usize = MAX_BOUNDED_STDIN_BYTES;

/// Bytes of child stderr retained before the run is failed.
///
/// The retained bytes are never read by this module; the cap exists only so a
/// chatty child cannot grow unbounded. Exceeding it fails the run, which is the
/// safe direction: no proposal is minted from a run that misbehaved.
pub const PYTHON_FORMATTER_STDERR_LIMIT: usize = 64 * 1024;

/// Finite budget for one external formatting run, shared by approval and the
/// formatting child.
pub const PYTHON_FORMATTER_TIMEOUT: Duration = Duration::from_secs(20);

/// Detail tag recorded on every proposal produced by this route.
///
/// Deliberately different from `language_tooling.lsp_formatting`: the proposal
/// reuses [`WorkspaceEditSourceKind::LspFormatting`] because the enum lives in a
/// frozen chokepoint, and this tag is how a reviewer still sees that no
/// language server was involved.
pub const EXTERNAL_FORMATTER_DETAIL_TAG: &str = "language_tooling.external_formatter";

/// Reason recorded on the approval receipt for an external formatting run.
///
/// The run path asks for approval with [`FormatterProbe::Unprobeable`] rather
/// than spawning a throwaway probe child: the formatting invocation *is* the
/// invocation, it is bounded, and its exit status is checked against
/// [`PYTHON_FORMATTER_EXPECTED_EXIT_CODE`] before its output is looked at. The
/// receipt therefore records fingerprint identity only and claims no execution
/// evidence it does not have.
pub const EXTERNAL_FORMATTER_UNPROBEABLE_REASON: &str =
    "the bounded formatting run is the invocation; its exit status is checked before its output";

/// Exact prerequisite recorded when the app cannot hand this route a language
/// capability broker.
///
/// Retained for the defensive `None` arm and for tests that still name the
/// blocked-prerequisite string. Production `AppComposition` now obtains the
/// broker from `LanguageStartupAuthority::capability_broker`.
pub const EXTERNAL_FORMATTER_BROKER_PREREQUISITE: &str = "external formatter launch is blocked: AppComposition cannot obtain the language capability \
     broker because crates/legion-app/src/language/startup_authority.rs exposes no accessor for \
     LanguageStartupAuthority's broker";

/// Explicit bounds for one external formatter run.
///
/// Every field is a hard limit checked before or enforced during the run. The
/// struct is a value rather than four constants read directly so that a caller
/// — including a test that must prove the over-size path fails closed — states
/// the bounds it is running under instead of inheriting them invisibly.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExternalFormatterBounds {
    /// Largest document, in bytes, admitted to the child's stdin.
    pub max_document_bytes: usize,
    /// Largest stdout, in bytes, retained before the run fails.
    pub max_stdout_bytes: usize,
    /// Largest stderr, in bytes, retained before the run fails.
    pub max_stderr_bytes: usize,
    /// Finite budget shared by approval and the formatting child.
    pub timeout: Duration,
}

impl ExternalFormatterBounds {
    /// The bounds every production Python formatting run uses.
    pub const PYTHON: Self = Self {
        max_document_bytes: PYTHON_FORMATTER_MAX_DOCUMENT_BYTES,
        max_stdout_bytes: PYTHON_FORMATTER_STDOUT_LIMIT,
        max_stderr_bytes: PYTHON_FORMATTER_STDERR_LIMIT,
        timeout: PYTHON_FORMATTER_TIMEOUT,
    };
}

/// One fully described external formatting run.
///
/// The argument vector and the expected exit status are carried here rather
/// than inferred at the spawn site, so the exact contract is visible in the
/// request a reviewer reads and in the request a test asserts on.
#[derive(Debug, Clone)]
pub struct ExternalFormatterRun {
    /// Operator-selected formatter executable. Explicit and absolute; this
    /// route performs no `PATH` lookup.
    pub executable: PathBuf,
    /// Exact argument vector handed to the child.
    pub args: Vec<String>,
    /// Exit status required before the child's stdout may be read.
    pub expected_exit_code: i32,
    /// In-memory buffer snapshot delivered on the child's stdin. This is the
    /// same text the resulting proposal is diffed against; nothing is read back
    /// from disk.
    pub document: String,
    /// Hard limits for this run.
    pub bounds: ExternalFormatterBounds,
    /// Principal the approval is requested for.
    pub principal_id: PrincipalId,
    /// Workspace the approval is requested for.
    pub workspace_id: WorkspaceId,
    /// Workspace trust posture at admission time.
    pub workspace_trust_state: WorkspaceTrustState,
    /// Non-zero request correlation.
    pub correlation_id: CorrelationId,
    /// Non-nil request lineage.
    pub causality_id: CausalityId,
}

/// Why an external formatting run produced no formatted document.
///
/// No variant carries child stdout or stderr. `RunFailed` carries the platform
/// error text only, which describes the transport (spawn, timeout, stream cap)
/// and never the child's output.
#[derive(Debug, Error)]
pub enum ExternalFormatterError {
    /// The run description itself was unusable.
    #[error("external formatter request is invalid: {0}")]
    InvalidRequest(&'static str),
    /// The document exceeded the run's input bound. Raised before the broker is
    /// asked and before any child exists.
    #[error("document of {actual} bytes exceeds the {limit} byte external formatter input bound")]
    DocumentTooLarge {
        /// Size of the rejected document.
        actual: usize,
        /// Bound that rejected it.
        limit: usize,
    },
    /// No fresh broker authority, or the executable failed identity checks.
    #[error("external formatter is not approved: {0}")]
    NotApproved(String),
    /// The bounded process authority refused or failed the child.
    #[error("external formatter run failed: {0}")]
    RunFailed(String),
    /// The child exited with a status other than the required one. Its stdout
    /// was discarded unread.
    #[error("external formatter exited with status {observed}, expected {expected}")]
    UnexpectedExitCode {
        /// Status the child actually reported.
        observed: i32,
        /// Status the explicit contract requires.
        expected: i32,
    },
    /// The run was cancelled.
    #[error("external formatter run was cancelled")]
    Cancelled,
}

/// Runs one approved external formatter over `run.document` and returns the
/// child's verified stdout.
///
/// Order of operations, and every step of it is load-bearing:
///
/// 1. Validate the run and reject an over-size document — before the broker is
///    asked, so an over-size buffer cannot cause a capability decision or a
///    child process.
/// 2. Observe cancellation.
/// 3. Obtain **fresh** authority through [`approve_formatter_executable`]. A
///    receipt minted earlier is identity evidence, not permission to launch, so
///    this route mints its own every time. A configured-but-unapproved
///    formatter therefore never reaches a spawn.
/// 4. Spawn exactly one bounded child, with the buffer snapshot on stdin and
///    the explicit argument vector, under the remaining budget and the caller's
///    cancellation flag.
/// 5. **Check the exit status before touching stdout.** A formatter that exits
///    non-zero while printing plausible source produces no formatted document.
/// 6. Observe cancellation again, because the process authority can return
///    successfully just as a cancel is raised.
///
/// The child's stderr is dropped at this boundary and is not bound to a name.
pub fn run_external_formatter(
    broker: &dyn CapabilityBrokerPort,
    process: &dyn ProcessService,
    run: &ExternalFormatterRun,
    cancellation: Arc<AtomicBool>,
) -> Result<String, ExternalFormatterError> {
    validate_run(run)?;
    if run.document.len() > run.bounds.max_document_bytes {
        return Err(ExternalFormatterError::DocumentTooLarge {
            actual: run.document.len(),
            limit: run.bounds.max_document_bytes,
        });
    }
    if cancellation.load(std::sync::atomic::Ordering::Relaxed) {
        return Err(ExternalFormatterError::Cancelled);
    }

    let deadline = Instant::now() + run.bounds.timeout;
    let approved = approve_formatter_executable(
        broker,
        process,
        FormatterApprovalRequest {
            executable: run.executable.clone(),
            principal_id: run.principal_id.clone(),
            workspace_id: run.workspace_id,
            workspace_trust_state: run.workspace_trust_state.clone(),
            correlation_id: run.correlation_id,
            causality_id: run.causality_id,
            probe: FormatterProbe::Unprobeable {
                reason: EXTERNAL_FORMATTER_UNPROBEABLE_REASON.to_string(),
            },
            deadline: Some(deadline),
        },
        Arc::clone(&cancellation),
    )
    .map_err(|error| match error {
        FormatterApprovalError::Cancelled => ExternalFormatterError::Cancelled,
        other => ExternalFormatterError::NotApproved(other.to_string()),
    })?;

    let command = approved
        .canonical_path()
        .to_str()
        .ok_or(ExternalFormatterError::InvalidRequest(
            "canonical formatter path is not representable as UTF-8",
        ))?
        .to_string();
    let remaining = deadline.saturating_duration_since(Instant::now());
    if remaining.is_zero() {
        return Err(ExternalFormatterError::RunFailed(
            "no budget remained for the formatting run".to_string(),
        ));
    }

    let result = process
        .execute_bounded(&BoundedProcessRequest::new(
            ProcessRequest {
                command,
                args: run.args.clone(),
                // No cwd and no environment: the child is given the document
                // and nothing else. It has no reason to resolve a path.
                cwd: None,
                env: Vec::new(),
                stdin: Some(run.document.as_bytes().to_vec()),
                timeout: Some(remaining),
                cancelled: false,
            },
            run.bounds.max_stdout_bytes,
            run.bounds.max_stderr_bytes,
            remaining,
            Arc::clone(&cancellation),
        ))
        .map_err(|error| match error {
            PlatformError::Cancelled { .. } => ExternalFormatterError::Cancelled,
            other => ExternalFormatterError::RunFailed(other.to_string()),
        })?;

    // The exit status is consulted first and alone. `stdout` is not bound to a
    // name until it has been established that it is the formatted document, and
    // `stderr` is never bound at all.
    if result.exit_code != run.expected_exit_code {
        return Err(ExternalFormatterError::UnexpectedExitCode {
            observed: result.exit_code,
            expected: run.expected_exit_code,
        });
    }
    if cancellation.load(std::sync::atomic::Ordering::Relaxed) {
        return Err(ExternalFormatterError::Cancelled);
    }
    Ok(result.stdout)
}

fn validate_run(run: &ExternalFormatterRun) -> Result<(), ExternalFormatterError> {
    if run.args.is_empty() {
        return Err(ExternalFormatterError::InvalidRequest(
            "argument vector is empty",
        ));
    }
    if run.bounds.max_document_bytes == 0 {
        return Err(ExternalFormatterError::InvalidRequest(
            "document bound is zero",
        ));
    }
    if run.bounds.max_document_bytes > MAX_BOUNDED_STDIN_BYTES {
        return Err(ExternalFormatterError::InvalidRequest(
            "document bound exceeds the bounded stdin transport limit",
        ));
    }
    if run.bounds.max_stdout_bytes == 0 || run.bounds.max_stderr_bytes == 0 {
        return Err(ExternalFormatterError::InvalidRequest(
            "stream bound is zero",
        ));
    }
    if run.bounds.timeout.is_zero() {
        return Err(ExternalFormatterError::InvalidRequest("timeout is zero"));
    }
    Ok(())
}

/// One admitted external formatting request, held between the request and the
/// result.
///
/// It carries the snapshot the input was taken at, so the result can be gated
/// by the existing staleness rule instead of assuming the buffer stood still.
#[derive(Debug)]
pub struct ExternalFormattingAdmission {
    pending: PendingLspWriteOperation,
    run: ExternalFormatterRun,
    uri: String,
    formatter_label: String,
}

impl ExternalFormattingAdmission {
    /// The fully described run this admission authorized.
    pub fn run(&self) -> &ExternalFormatterRun {
        &self.run
    }
}

/// End position of `text` in the coordinate system
/// [`super::lsp_position_to_byte_offset`] uses.
///
/// That translator counts `\n` only and measures the column in UTF-16 code
/// units, so the end position is computed the same way here rather than trusting
/// a position past the end of the document to be clamped. A whole-document
/// replacement whose end position resolves to the wrong offset would truncate
/// the file through an accepted proposal.
fn document_end_position(text: &str) -> (u64, u64) {
    let line = text.matches('\n').count() as u64;
    let last_line = match text.rfind('\n') {
        Some(index) => &text[index + 1..],
        None => text,
    };
    let character: u64 = last_line.chars().map(|ch| ch.len_utf16() as u64).sum();
    (line, character)
}

impl AppComposition {
    /// The language capability broker this route must consult for fresh
    /// authority, or `None` when the app cannot supply one.
    ///
    /// The broker that carries the operator's exact-binary allowance for a
    /// configured formatter is owned by `LanguageStartupAuthority`. This
    /// accessor returns that same broker so an external formatting run uses
    /// the recorded allowance instead of minting a second one.
    fn external_formatter_capability_broker(
        &self,
    ) -> Option<Arc<dyn CapabilityBrokerPort + Send + Sync>> {
        Some(self.language_startup_authority.capability_broker())
    }

    /// Runs the configured external Python formatter for `buffer_id`, if one
    /// applies, and returns whether this route handled the request.
    ///
    /// `false` means the request was not this route's to answer — the buffer is
    /// not Python, or no Python formatter is configured — and the caller must
    /// fall through to the language-server formatting request unchanged.
    /// `true` means a terminal operation was recorded: a reviewable proposal, a
    /// no-op, a stale result, a cancellation, or an explicit failure.
    pub(crate) fn route_external_python_formatting(&mut self, buffer_id: BufferId) -> bool {
        let Some(admission) = self.admit_external_python_formatting(buffer_id) else {
            return false;
        };
        // Synchronous on the app thread, so nothing else can raise this flag
        // during the run; it is still the live flag handed to the bounded
        // request and observed after it returns, which is what a cancelling
        // caller needs when this route grows an off-thread runner.
        let cancellation = Arc::new(AtomicBool::new(false));
        let broker = self.external_formatter_capability_broker();
        match broker {
            Some(broker) => self.run_admitted_external_formatting(
                admission,
                &*broker,
                &NativeProcessService,
                cancellation,
            ),
            // A capability this app cannot supply is recorded as a blocked
            // prerequisite naming exactly what is missing. It is never a silent
            // fall-through to the language server — the operator selected this
            // tool deliberately — and never an empty proposal.
            None => self.complete_external_python_formatting(
                admission,
                Err(ExternalFormatterError::NotApproved(
                    EXTERNAL_FORMATTER_BROKER_PREREQUISITE.to_string(),
                )),
            ),
        }
    }

    /// Runs one already-admitted request against the supplied ports and
    /// terminalizes it.
    ///
    /// This is the whole run half of the route and the only place those two
    /// steps are sequenced. The production entry point above and the
    /// port-injecting test seam below both call it, so no behaviour is forked
    /// for tests: only the two ports differ.
    fn run_admitted_external_formatting(
        &mut self,
        admission: ExternalFormattingAdmission,
        broker: &dyn CapabilityBrokerPort,
        process: &dyn ProcessService,
        cancellation: Arc<AtomicBool>,
    ) -> bool {
        let outcome = run_external_formatter(broker, process, &admission.run, cancellation);
        self.complete_external_python_formatting(admission, outcome)
    }

    /// Admits one external Python formatting request against the current buffer
    /// snapshot, recording a `Running` operation.
    ///
    /// Returns `None` when this route does not apply, which is the only path
    /// that leaves the language-server route's behaviour untouched.
    pub(crate) fn admit_external_python_formatting(
        &mut self,
        buffer_id: BufferId,
    ) -> Option<ExternalFormattingAdmission> {
        let formatter = PathBuf::from(
            self.language_toolchain_settings
                .python
                .as_ref()?
                .formatter_executable
                .0
                .clone(),
        );
        let metadata = self
            .active_documents
            .metadata_for_buffer(buffer_id)
            .cloned()?;
        if language_id_for_path(&metadata.identity.canonical_path).0 != "python" {
            return None;
        }
        let workspace_id = self.active_documents.workspace_id()?;
        let workspace_trust_state = self.active_documents.active_workspace_trust.clone()?;
        let principal_id = self
            .active_documents
            .active_principal_id
            .clone()
            .unwrap_or_else(|| PrincipalId("system".to_string()));
        let snapshot_id = self.editor.current_snapshot(buffer_id).ok()?.snapshot_id;
        let document = self.editor.text(buffer_id).ok()?.to_string();
        let uri = canonical_path_to_uri(&metadata.identity.canonical_path.0);
        // Display label only. Nothing in this module branches on it; see
        // `PYTHON_FORMATTER_STDIN_ARGS` for why the invocation may not be
        // derived from a file name.
        let formatter_label = formatter
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("external formatter")
            .to_string();
        let event_context = self.next_event_context();
        let pending = PendingLspWriteOperation {
            operation_id: uuid::Uuid::now_v7().to_string(),
            operation_kind: LanguageToolingOperationKind::FormattingProposal,
            workspace_id,
            file_id: metadata.identity.file_id,
            buffer_id,
            snapshot_id,
            event_context,
        };
        let run = ExternalFormatterRun {
            executable: formatter,
            args: PYTHON_FORMATTER_STDIN_ARGS
                .iter()
                .map(|arg| arg.to_string())
                .collect(),
            expected_exit_code: PYTHON_FORMATTER_EXPECTED_EXIT_CODE,
            document,
            bounds: ExternalFormatterBounds::PYTHON,
            principal_id,
            workspace_id,
            workspace_trust_state,
            correlation_id: event_context.correlation_id,
            causality_id: event_context.causality_id,
        };
        let _ = self.language_tooling.upsert_write_operation(
            &pending,
            LanguageToolingStatusKind::Running,
            "external formatter request accepted".to_string(),
            None,
        );
        Some(ExternalFormattingAdmission {
            pending,
            run,
            uri,
            formatter_label,
        })
    }

    /// Terminalizes one admitted external formatting request.
    ///
    /// The staleness gate runs first and uses the same
    /// [`super::is_stale_response`] rule the language-server result path uses:
    /// a result that arrives against a moved snapshot is `Stale` and never a
    /// proposal, whatever the run itself reported.
    ///
    /// Formatted output identical to the snapshot is a terminal no-op with no
    /// proposal id. Everything else goes through
    /// `AppComposition::ingest_lsp_write_side_result`, so translation,
    /// preconditions, the proposal lifecycle and `workspace/applyEdit` review
    /// are the reviewed ones rather than a second implementation.
    ///
    /// Always returns `true`: an admitted request has been answered.
    pub(crate) fn complete_external_python_formatting(
        &mut self,
        admission: ExternalFormattingAdmission,
        outcome: Result<String, ExternalFormatterError>,
    ) -> bool {
        let ExternalFormattingAdmission {
            pending,
            run,
            uri,
            formatter_label,
        } = admission;

        if let Ok(current) = self.editor.current_snapshot(pending.buffer_id)
            && is_stale_response(pending.snapshot_id, current.snapshot_id)
        {
            let _ = self.language_tooling.upsert_write_operation(
                &pending,
                LanguageToolingStatusKind::Stale,
                "external formatter result was stale for the current buffer snapshot".to_string(),
                None,
            );
            return true;
        }

        let formatted = match outcome {
            Ok(formatted) => formatted,
            Err(ExternalFormatterError::Cancelled) => {
                let _ = self.language_tooling.upsert_write_operation(
                    &pending,
                    LanguageToolingStatusKind::Cancelled,
                    "external formatter run was cancelled".to_string(),
                    None,
                );
                return true;
            }
            Err(error) => {
                // `error` is one of this module's variants; none of them can
                // carry child stdout or stderr.
                let _ = self.language_tooling.upsert_write_operation(
                    &pending,
                    LanguageToolingStatusKind::Failed,
                    format!("external formatter unavailable: {error}"),
                    None,
                );
                return true;
            }
        };

        if formatted == run.document {
            let _ = self.language_tooling.upsert_write_operation(
                &pending,
                LanguageToolingStatusKind::Ready,
                "external formatter reported no changes".to_string(),
                None,
            );
            return true;
        }

        // One whole-document replacement, lifted into the `{"changes": {uri:
        // [TextEdit]}}` shape a formatting response already arrives in, so the
        // same translation, preconditions and review apply. `formatted` is the
        // child's verified stdout and is the only text that reaches this edit.
        let (end_line, end_character) = document_end_position(&run.document);
        let edit = serde_json::json!({
            "changes": {
                uri: [{
                    "range": {
                        "start": { "line": 0, "character": 0 },
                        "end": { "line": end_line, "character": end_character }
                    },
                    "newText": formatted
                }]
            }
        });
        let label = bounded_label(formatter_label, 64);
        let spec = LspWriteSideSpec {
            proposal_kind: LanguageProposalKind::Formatting,
            source_kind: WorkspaceEditSourceKind::LspFormatting,
            title: format!("Format document with {label}"),
            detail_tag: EXTERNAL_FORMATTER_DETAIL_TAG,
            detail_extra: vec![
                format!("formatter={label}"),
                "provenance=external_process_stdout".to_string(),
                "provenance_note=not_produced_by_a_language_server".to_string(),
            ],
            command: None,
        };
        self.ingest_lsp_write_side_result(pending.buffer_id, spec, &edit, pending);
        true
    }

    /// Test-only: runs the external Python formatting route with injected
    /// capability-broker and process ports.
    ///
    /// The route itself is the production one; only the two ports differ, so
    /// there is no `cfg(test)` fork in the behaviour under test.
    #[cfg(any(test, feature = "test-helpers"))]
    pub fn run_external_python_formatting_with_ports_for_test(
        &mut self,
        buffer_id: BufferId,
        broker: &dyn CapabilityBrokerPort,
        process: &dyn ProcessService,
        cancellation: Arc<AtomicBool>,
    ) -> bool {
        let Some(admission) = self.admit_external_python_formatting(buffer_id) else {
            return false;
        };
        self.run_admitted_external_formatting(admission, broker, process, cancellation)
    }

    /// Test-only: admits an external Python formatting request without running
    /// it, so a test can move the buffer snapshot between request and result.
    #[cfg(any(test, feature = "test-helpers"))]
    pub fn admit_external_python_formatting_for_test(
        &mut self,
        buffer_id: BufferId,
    ) -> Option<ExternalFormattingAdmission> {
        self.admit_external_python_formatting(buffer_id)
    }

    /// Test-only: terminalizes a previously admitted external formatting
    /// request with an explicit outcome.
    #[cfg(any(test, feature = "test-helpers"))]
    pub fn complete_external_python_formatting_for_test(
        &mut self,
        admission: ExternalFormattingAdmission,
        outcome: Result<String, ExternalFormatterError>,
    ) -> bool {
        self.complete_external_python_formatting(admission, outcome)
    }
}

#[cfg(test)]
mod tests {
    use super::document_end_position;

    #[test]
    fn document_end_position_matches_the_translator_line_and_utf16_column_model() {
        assert_eq!(document_end_position(""), (0, 0));
        assert_eq!(document_end_position("abc"), (0, 3));
        // A trailing newline ends the document at column zero of the line after
        // the last one that carries text.
        assert_eq!(document_end_position("a\nb\n"), (2, 0));
        assert_eq!(document_end_position("a\nbc"), (1, 2));
        // Carriage returns are not line separators in the translator's model,
        // so they count as columns on the line they close.
        assert_eq!(document_end_position("a\r\nbc"), (1, 2));
        // Astral characters are two UTF-16 code units.
        assert_eq!(document_end_position("\u{1F600}"), (0, 2));
    }
}
