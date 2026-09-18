//! App-owned approval and bounded probing for one explicitly selected local
//! formatter executable.
//!
//! This is the Node-runtime approval boundary (`approve_node_runtime`, in the
//! sibling `runtime` module) generalized to a formatter, and it reuses that
//! module's shared mechanics rather than restating them: the
//! canonical-regular-file plus bounded content fingerprint, the single finite
//! budget and cancellation checks, the broker request/response match, the
//! post-broker and post-probe identity rechecks, and the bounded child probe.
//!
//! What this boundary refuses, by construction:
//!
//! * **No `PATH` discovery.** The request carries an explicit absolute path. A
//!   bare name with no path separator is rejected before any filesystem,
//!   broker or process effect. There is no environment fallback and no
//!   "looks absolute" heuristic.
//! * **No untrusted workspace.** A workspace that is not
//!   [`WorkspaceTrustState::Trusted`] is refused before the path is resolved.
//! * **No self-issued authority.** Only a granted, non-zero broker decision for
//!   the exact requested capability admits the probe.
//! * **No probe output retention.** The child's stdout and stderr are dropped
//!   at the boundary; no error variant and no receipt field carries them.
//!
//! # Capability
//!
//! [`FORMATTER_PROBE_CAPABILITY`] is `lsp.launch`, the same capability the Node
//! runtime probe uses and one the broker already recognises: the
//! deny-by-default policy has an `lsp.` branch that requires a trusted
//! workspace and matches the exact binary against the operator's allowlist
//! (`LspLaunchPolicy::allowed_binaries`), which is precisely the
//! per-executable authorization this boundary needs. A formatter approved here
//! is language tooling launched by the language subsystem, so it is authorized
//! under the same posture. Two alternatives were rejected: `terminal.launch`
//! gates on the terminal runtime being enabled and authorizes by command
//! *class* rather than by exact binary, and a fresh `formatter.launch` string
//! would be a capability no policy knows about, which is a hole rather than a
//! gate.
//!
//! # The receipt is not a launch grant
//!
//! [`ApprovedFormatterExecutable`] is identity evidence for the exact
//! executable that was probed. It is not permission to launch that executable,
//! and it is never permission to launch another one. Later launch code must
//! revalidate it with [`ApprovedFormatterExecutable::revalidate`] and obtain
//! fresh broker authority before spawning a formatter.

use std::{
    path::{Path, PathBuf},
    sync::{atomic::AtomicBool, Arc},
    time::{Duration, Instant},
};

use legion_platform::ProcessService;
use legion_protocol::{
    CanonicalPath, CapabilityBrokerPort, CapabilityDecisionId, CapabilityId, CausalityId,
    CorrelationId, FileFingerprint, PrincipalId, WorkspaceId, WorkspaceTrustState,
};
use thiserror::Error;

use super::runtime::{
    canonical_path_to_string, canonical_regular_file, check_budget,
    language_tool_capability_context, recheck_executable_identity, request_granted_decision,
    run_bounded_probe, BoundedProbeError, ExecutableIdentityError,
    EXECUTABLE_FINGERPRINT_MAX_BYTES, EXECUTABLE_PROBE_STREAM_LIMIT, EXECUTABLE_PROBE_TIMEOUT,
};

/// Capability requested before any formatter executable is probed.
///
/// See the module documentation for why this is an existing capability id and
/// not a newly invented one.
pub const FORMATTER_PROBE_CAPABILITY: &str = "lsp.launch";

/// Maximum bytes retained from either probe stream.
pub const FORMATTER_PROBE_STREAM_LIMIT: usize = EXECUTABLE_PROBE_STREAM_LIMIT;

/// Finite overall approval budget. Hashing, broker evaluation and the child
/// probe share this budget; the child receives only the remaining duration.
pub const FORMATTER_PROBE_TIMEOUT: Duration = EXECUTABLE_PROBE_TIMEOUT;

/// Maximum executable size admitted to the bounded identity hash.
pub const FORMATTER_FINGERPRINT_MAX_BYTES: u64 = EXECUTABLE_FINGERPRINT_MAX_BYTES;

/// Maximum number of probe arguments accepted from a caller.
pub const FORMATTER_PROBE_MAX_ARGS: usize = 16;

/// Maximum byte length of any single probe argument.
pub const FORMATTER_PROBE_ARG_MAX_BYTES: usize = 256;

/// Maximum byte length of the operator-supplied unprobeable reason.
pub const FORMATTER_UNPROBEABLE_REASON_MAX_BYTES: usize = 256;

/// How the selected formatter proves it can be executed.
///
/// Formatters share no `--version` contract, so the argument vector and the
/// caller's expectation are carried explicitly. Nothing here is inferred and no
/// version grammar is parsed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FormatterProbe {
    /// Run the executable once with exactly `args` and require exactly
    /// `expected_exit_code`.
    Arguments {
        /// Exact argument vector handed to the bounded process authority.
        args: Vec<String>,
        /// Exit status the caller requires from the probe.
        expected_exit_code: i32,
    },
    /// The operator declared this formatter cannot be version- or
    /// invocation-probed. No child is spawned, and the receipt records the
    /// absence of execution evidence instead of implying it.
    Unprobeable {
        /// Non-empty operator-supplied reason, recorded on the receipt.
        reason: String,
    },
}

impl FormatterProbe {
    /// The exact argument vector and required exit status, or `None` when the
    /// caller declared the formatter unprobeable.
    fn arguments(&self) -> Option<(&[String], i32)> {
        match self {
            Self::Arguments {
                args,
                expected_exit_code,
            } => Some((args, *expected_exit_code)),
            Self::Unprobeable { .. } => None,
        }
    }
}

/// What the approval established about execution.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FormatterProbeOutcome {
    /// The requested argument vector was executed under the shared budget and
    /// the child exited with the required status. Nothing the child printed is
    /// recorded.
    Executed,
    /// No child was spawned because the request declared the formatter
    /// unprobeable, so the receipt carries fingerprint identity only.
    NotProbed,
}

/// Input captured from the app's real workspace/principal/event context.
#[derive(Debug, Clone)]
pub struct FormatterApprovalRequest {
    /// Explicit operator-selected formatter executable. It must be an absolute
    /// path to a regular file; this type intentionally has no PATH lookup
    /// behavior.
    pub executable: PathBuf,
    /// Principal requesting the approval.
    pub principal_id: PrincipalId,
    /// Workspace owning the request.
    pub workspace_id: WorkspaceId,
    /// Current workspace trust posture.
    pub workspace_trust_state: WorkspaceTrustState,
    /// Non-zero request correlation.
    pub correlation_id: CorrelationId,
    /// Non-nil request lineage.
    pub causality_id: CausalityId,
    /// How the executable is probed, or an explicit declaration that it cannot
    /// be probed.
    pub probe: FormatterProbe,
    /// Optional caller-supplied absolute deadline for the whole approval.
    ///
    /// It can only shorten the budget: the effective deadline is the earlier of
    /// this value and [`FORMATTER_PROBE_TIMEOUT`] from now. An already-expired
    /// deadline fails before any filesystem, broker or process effect.
    pub deadline: Option<Instant>,
}

/// Opaque approval receipt for one canonical formatter executable.
///
/// Fields are private so callers cannot construct a trusted formatter from a
/// serialized or caller-supplied decision. Construction occurs only after the
/// broker grants the request and, when a probe was requested, the bounded probe
/// exits as required and the executable's identity is unchanged afterwards.
#[derive(Debug, Clone)]
pub struct ApprovedFormatterExecutable {
    canonical_path: PathBuf,
    fingerprint: FileFingerprint,
    decision_id: CapabilityDecisionId,
    probe: FormatterProbe,
    probe_outcome: FormatterProbeOutcome,
    principal_id: PrincipalId,
    workspace_id: WorkspaceId,
    workspace_trust_state: WorkspaceTrustState,
    correlation_id: CorrelationId,
    causality_id: CausalityId,
}

impl ApprovedFormatterExecutable {
    /// Canonical executable path that was both authorized and probed.
    pub fn canonical_path(&self) -> &Path {
        &self.canonical_path
    }

    /// Content fingerprint captured before the broker window and verified again
    /// after the probe.
    pub fn fingerprint(&self) -> &FileFingerprint {
        &self.fingerprint
    }

    /// Broker-issued decision identifier.
    pub fn decision_id(&self) -> CapabilityDecisionId {
        self.decision_id
    }

    /// Probe description this receipt was minted for, exactly as requested.
    pub fn requested_probe(&self) -> &FormatterProbe {
        &self.probe
    }

    /// Whether a bounded child actually ran.
    pub fn probe_outcome(&self) -> FormatterProbeOutcome {
        self.probe_outcome
    }

    /// Principal bound to the approval.
    pub fn principal_id(&self) -> &PrincipalId {
        &self.principal_id
    }

    /// Workspace bound to the approval.
    pub fn workspace_id(&self) -> WorkspaceId {
        self.workspace_id
    }

    /// Trust posture used in the broker request.
    pub fn workspace_trust_state(&self) -> &WorkspaceTrustState {
        &self.workspace_trust_state
    }

    /// Correlation identifier used for the broker request.
    pub fn correlation_id(&self) -> CorrelationId {
        self.correlation_id
    }

    /// Causality identifier used for the broker request.
    pub fn causality_id(&self) -> CausalityId {
        self.causality_id
    }
}

/// Errors returned by the formatter approval boundary. Probe output is never
/// included, keeping raw command output out of records and diagnostics.
#[derive(Debug, Error)]
pub enum FormatterApprovalError {
    /// Required request identity or probe description was absent, zero or
    /// unbounded.
    #[error("invalid formatter approval request: {0}")]
    InvalidRequest(&'static str),
    /// The selected path could not be canonicalized or is not a regular file.
    #[error("formatter executable is not a canonical regular file: {0}")]
    InvalidExecutable(String),
    /// The workspace is not trusted, or the broker did not return a matching
    /// granted decision.
    #[error("formatter capability decision rejected: {0}")]
    CapabilityRejected(String),
    /// Bounded process authority failed, or the child did not exit as required.
    #[error("formatter probe failed: {0}")]
    ProbeFailed(String),
    /// The executable changed identity during the approval window.
    #[error("formatter executable changed during approval probe")]
    ExecutableChanged,
    /// The bounded identity operation was cancelled.
    #[error("formatter executable identity check was cancelled")]
    Cancelled,
    /// The bounded identity operation exceeded its finite deadline.
    #[error("formatter executable identity check timed out")]
    DeadlineExceeded,
}

impl From<ExecutableIdentityError> for FormatterApprovalError {
    fn from(error: ExecutableIdentityError) -> Self {
        match error {
            ExecutableIdentityError::Invalid(message) => Self::InvalidExecutable(message),
            ExecutableIdentityError::Changed => Self::ExecutableChanged,
            ExecutableIdentityError::Cancelled => Self::Cancelled,
            ExecutableIdentityError::DeadlineExceeded => Self::DeadlineExceeded,
        }
    }
}

/// Approve one explicitly selected local formatter executable.
///
/// The order of operations is the Node runtime boundary's order: validate the
/// request, refuse an untrusted workspace, canonicalize and fingerprint the
/// file, ask the broker, recheck identity and budget after the broker window,
/// run the bounded probe under the remaining budget, recheck the budget after
/// the child returns, require the caller's expected exit status, and recheck
/// identity again before minting the receipt.
///
/// Broker evaluation happens before `execute_bounded`, and the canonical path
/// plus content fingerprint are checked again after the child exits. This
/// closes ordinary replacement/symlink changes; a platform cannot eliminate the
/// final OS-level TOCTOU interval. The receipt is identity evidence and does not
/// mint a fresh launch grant, so later launch code must revalidate it and obtain
/// fresh broker authority before spawning the formatter.
pub fn approve_formatter_executable(
    broker: &dyn CapabilityBrokerPort,
    process: &dyn ProcessService,
    request: FormatterApprovalRequest,
    cancellation: Arc<AtomicBool>,
) -> Result<ApprovedFormatterExecutable, FormatterApprovalError> {
    validate_request(&request)?;
    if request.workspace_trust_state != WorkspaceTrustState::Trusted {
        return Err(FormatterApprovalError::CapabilityRejected(
            "workspace is not trusted".to_string(),
        ));
    }

    let deadline = effective_deadline(request.deadline);
    let (canonical_path, fingerprint) =
        canonical_regular_file(&request.executable, &cancellation, deadline)?;
    let canonical_command = canonical_path_to_string(&canonical_path)?;
    let capability_id = CapabilityId(FORMATTER_PROBE_CAPABILITY.to_string());
    let decision_id = request_granted_decision(
        broker,
        &request.principal_id,
        &capability_id,
        &request.workspace_trust_state,
        CanonicalPath(canonical_command.clone()),
        language_tool_capability_context(&canonical_command),
        request.correlation_id,
    )
    .map_err(FormatterApprovalError::CapabilityRejected)?;

    // The broker call is an unbounded external seam. Recheck the identity and
    // finite budget immediately before spawning so a replacement during the
    // broker window cannot be probed under the old decision.
    recheck_executable_identity(
        &request.executable,
        &canonical_path,
        &fingerprint,
        &cancellation,
        deadline,
    )?;

    let probe_outcome = match request.probe.arguments() {
        Some((args, expected_exit_code)) => {
            let probe = run_bounded_probe(
                process,
                canonical_command,
                args.to_vec(),
                FORMATTER_PROBE_STREAM_LIMIT,
                deadline,
                cancellation.clone(),
            )
            .map_err(|error| match error {
                BoundedProbeError::DeadlineExceeded => FormatterApprovalError::DeadlineExceeded,
                BoundedProbeError::Cancelled => FormatterApprovalError::Cancelled,
                BoundedProbeError::Failed(message) => FormatterApprovalError::ProbeFailed(message),
            })?;
            // The process authority may return successfully just as cancellation
            // is raised. Do not mint a receipt from that race.
            check_budget(&cancellation, deadline)?;
            // Only the exit status is consulted here. `probe.stdout` and
            // `probe.stderr` are deliberately dropped: no formatter output
            // reaches an error, a receipt or a diagnostic.
            if probe.exit_code != expected_exit_code {
                return Err(FormatterApprovalError::ProbeFailed(
                    "probe exited with an unexpected status".to_string(),
                ));
            }
            FormatterProbeOutcome::Executed
        }
        None => FormatterProbeOutcome::NotProbed,
    };

    recheck_executable_identity(
        &request.executable,
        &canonical_path,
        &fingerprint,
        &cancellation,
        deadline,
    )?;

    Ok(ApprovedFormatterExecutable {
        canonical_path,
        fingerprint,
        decision_id,
        probe: request.probe,
        probe_outcome,
        principal_id: request.principal_id,
        workspace_id: request.workspace_id,
        workspace_trust_state: request.workspace_trust_state,
        correlation_id: request.correlation_id,
        causality_id: request.causality_id,
    })
}

impl ApprovedFormatterExecutable {
    /// Revalidates this receipt against the launch context and the current
    /// executable identity immediately before a later formatter launch.
    ///
    /// Revalidation is necessary but not sufficient: the launch path must also
    /// obtain fresh broker authority. This receipt never authorizes a spawn on
    /// its own.
    pub fn revalidate(
        &self,
        expected: &FormatterApprovalRequest,
        cancellation: Arc<AtomicBool>,
    ) -> Result<(), FormatterApprovalError> {
        validate_request(expected)?;
        if expected.workspace_trust_state != WorkspaceTrustState::Trusted
            || expected.principal_id != self.principal_id
            || expected.workspace_id != self.workspace_id
            || expected.workspace_trust_state != self.workspace_trust_state
            || expected.probe != self.probe
        {
            return Err(FormatterApprovalError::CapabilityRejected(
                "formatter receipt context does not match launch context".to_string(),
            ));
        }
        let deadline = effective_deadline(expected.deadline);
        let (path, fingerprint) =
            canonical_regular_file(&expected.executable, &cancellation, deadline)?;
        if path != self.canonical_path || fingerprint != self.fingerprint {
            return Err(FormatterApprovalError::ExecutableChanged);
        }
        Ok(())
    }
}

/// The single finite deadline for the whole approval. A caller-supplied
/// deadline can only shorten it.
fn effective_deadline(requested: Option<Instant>) -> Instant {
    let ceiling = Instant::now() + FORMATTER_PROBE_TIMEOUT;
    match requested {
        Some(requested) if requested < ceiling => requested,
        _ => ceiling,
    }
}

/// Whether `path` is a bare executable name that only a `PATH` search could
/// resolve. Such a request is refused: this boundary never searches `PATH`.
fn is_bare_executable_name(path: &Path) -> bool {
    let parent = path.parent();
    parent.is_none_or(|parent| parent.as_os_str().is_empty())
}

fn validate_request(request: &FormatterApprovalRequest) -> Result<(), FormatterApprovalError> {
    if request.principal_id.0.is_empty() {
        return Err(FormatterApprovalError::InvalidRequest(
            "principal id is empty",
        ));
    }
    if request.workspace_id.0 == 0 {
        return Err(FormatterApprovalError::InvalidRequest(
            "workspace id is zero",
        ));
    }
    if request.correlation_id.0 == 0 {
        return Err(FormatterApprovalError::InvalidRequest(
            "correlation id is zero",
        ));
    }
    if request.causality_id.0.is_nil() {
        return Err(FormatterApprovalError::InvalidRequest(
            "causality id is nil",
        ));
    }
    if is_bare_executable_name(&request.executable) {
        return Err(FormatterApprovalError::InvalidRequest(
            "formatter must be an explicit path, not a PATH-resolved name",
        ));
    }
    if !request.executable.is_absolute() {
        return Err(FormatterApprovalError::InvalidRequest(
            "formatter path must be absolute",
        ));
    }
    match &request.probe {
        FormatterProbe::Arguments { args, .. } => {
            if args.is_empty() {
                return Err(FormatterApprovalError::InvalidRequest(
                    "probe argument vector is empty",
                ));
            }
            if args.len() > FORMATTER_PROBE_MAX_ARGS {
                return Err(FormatterApprovalError::InvalidRequest(
                    "probe argument vector exceeds its bound",
                ));
            }
            for arg in args {
                if arg.is_empty() {
                    return Err(FormatterApprovalError::InvalidRequest(
                        "probe argument is empty",
                    ));
                }
                if arg.len() > FORMATTER_PROBE_ARG_MAX_BYTES {
                    return Err(FormatterApprovalError::InvalidRequest(
                        "probe argument exceeds its bound",
                    ));
                }
                if arg.contains('\0') {
                    return Err(FormatterApprovalError::InvalidRequest(
                        "probe argument contains an interior NUL",
                    ));
                }
            }
        }
        FormatterProbe::Unprobeable { reason } => {
            if reason.trim().is_empty() {
                return Err(FormatterApprovalError::InvalidRequest(
                    "unprobeable formatter requires an explicit reason",
                ));
            }
            if reason.len() > FORMATTER_UNPROBEABLE_REASON_MAX_BYTES {
                return Err(FormatterApprovalError::InvalidRequest(
                    "unprobeable reason exceeds its bound",
                ));
            }
        }
    }
    Ok(())
}
