//! App-owned approval and bounded probing for local Node runtimes.
//!
//! This boundary deliberately accepts an explicit executable path.  It never
//! resolves `node` through `PATH`, manufactures a capability decision, or
//! retains probe output.  The resulting record is an approval receipt for the
//! exact executable that was probed; it is not permission to launch another
//! executable.
//!
//! The identity, budget, broker and bounded-probe mechanics live in the shared
//! items at the bottom of this module and are reused verbatim by the sibling
//! `formatter_approval` module, so a later change to the TOCTOU window is made
//! once rather than in two drifting copies.

use std::{
    fs::File,
    io::Read,
    path::{Path, PathBuf},
    sync::{Arc, atomic::AtomicBool},
    time::{Duration, Instant},
};

use legion_lsp::LspNodeVersion;
use legion_platform::{
    BoundedProcessRequest, PlatformError, ProcessRequest, ProcessResult, ProcessService,
};
use legion_protocol::{
    CanonicalPath, CapabilityBrokerPort, CapabilityCommandClass, CapabilityDecisionId,
    CapabilityId, CapabilityRequest, CapabilityRequestContext, CapabilityResponse, CausalityId,
    CorrelationId, FileFingerprint, PrincipalId, WorkspaceId, WorkspaceTrustState,
};
use sha2::{Digest, Sha256};
use thiserror::Error;

/// Capability used for the local, shell-free runtime probe.
///
/// The normal LSP launch posture also requires `lsp.launch`; using that same
/// capability here makes the probe's authority explicit and avoids treating a
/// runtime receipt as an independent launch grant.
pub const NODE_RUNTIME_PROBE_CAPABILITY: &str = "lsp.launch";

/// Maximum bytes retained from either probe stream.
pub const NODE_RUNTIME_PROBE_STREAM_LIMIT: usize = EXECUTABLE_PROBE_STREAM_LIMIT;

/// Finite overall approval budget. Hashing, broker evaluation, and the child
/// probe share this budget; the child receives only the remaining duration.
pub const NODE_RUNTIME_PROBE_TIMEOUT: Duration = EXECUTABLE_PROBE_TIMEOUT;

/// Maximum executable size admitted to the bounded identity hash.
pub const NODE_RUNTIME_FINGERPRINT_MAX_BYTES: u64 = EXECUTABLE_FINGERPRINT_MAX_BYTES;

/// Maximum bytes retained from either stream of any bounded executable probe.
pub(crate) const EXECUTABLE_PROBE_STREAM_LIMIT: usize = 4 * 1024;

/// Finite overall budget shared by hashing, broker evaluation and the child
/// probe at every executable-approval boundary.
pub(crate) const EXECUTABLE_PROBE_TIMEOUT: Duration = Duration::from_secs(5);

/// Maximum executable size admitted to a bounded identity hash.
pub(crate) const EXECUTABLE_FINGERPRINT_MAX_BYTES: u64 = 256 * 1024 * 1024;

/// Input captured from the app's real workspace/principal/event context.
#[derive(Debug, Clone)]
pub struct NodeRuntimeApprovalRequest {
    /// Explicit operator-selected Node executable. It must already identify a
    /// regular file; this type intentionally has no PATH lookup behavior.
    pub executable: PathBuf,
    /// Principal requesting the probe.
    pub principal_id: PrincipalId,
    /// Workspace owning the request.
    pub workspace_id: WorkspaceId,
    /// Current workspace trust posture.
    pub workspace_trust_state: WorkspaceTrustState,
    /// Non-zero request correlation.
    pub correlation_id: CorrelationId,
    /// Non-nil request lineage.
    pub causality_id: CausalityId,
    /// Minimum Node version required by the selected language-server artifact.
    pub minimum_version: LspNodeVersion,
}

/// Opaque approval receipt for one canonical Node executable.
///
/// Fields are private so callers cannot construct a trusted runtime from a
/// serialized or caller-supplied version/decision.  Construction occurs only
/// after the broker grants the request and the bounded probe succeeds.
#[derive(Debug, Clone)]
pub struct ApprovedNodeRuntime {
    canonical_path: PathBuf,
    observed_version: LspNodeVersion,
    fingerprint: FileFingerprint,
    decision_id: CapabilityDecisionId,
    principal_id: PrincipalId,
    workspace_id: WorkspaceId,
    workspace_trust_state: WorkspaceTrustState,
    correlation_id: CorrelationId,
    causality_id: CausalityId,
}

impl ApprovedNodeRuntime {
    /// Canonical executable path that was both authorized and probed.
    pub fn canonical_path(&self) -> &Path {
        &self.canonical_path
    }

    /// Version parsed from the exact bounded `--version` stdout.
    pub fn observed_version(&self) -> LspNodeVersion {
        self.observed_version
    }

    /// Content fingerprint captured before and verified after the probe.
    pub fn fingerprint(&self) -> &FileFingerprint {
        &self.fingerprint
    }

    /// Broker-issued decision identifier.
    pub fn decision_id(&self) -> CapabilityDecisionId {
        self.decision_id
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

/// Errors returned by the runtime approval boundary.  Probe output is never
/// included, keeping raw command output out of records and diagnostics.
#[derive(Debug, Error)]
pub enum NodeRuntimeApprovalError {
    /// Required request identity was absent or zero.
    #[error("invalid runtime approval request: {0}")]
    InvalidRequest(&'static str),
    /// The selected path could not be canonicalized or is not a regular file.
    #[error("Node executable is not a canonical regular file: {0}")]
    InvalidExecutable(String),
    /// Broker did not return a matching granted decision.
    #[error("runtime capability decision rejected: {0}")]
    CapabilityRejected(String),
    /// Bounded process authority failed before a trustworthy version was read.
    #[error("Node runtime probe failed: {0}")]
    ProbeFailed(String),
    /// The executable changed identity during the probe window.
    #[error("Node executable changed during approval probe")]
    ExecutableChanged,
    /// The bounded identity operation was cancelled.
    #[error("Node executable identity check was cancelled")]
    Cancelled,
    /// The bounded identity operation exceeded its finite deadline.
    #[error("Node executable identity check timed out")]
    DeadlineExceeded,
    /// The bounded output did not contain exactly one valid Node version.
    #[error("Node runtime probe returned malformed version output")]
    MalformedVersion,
}

impl From<ExecutableIdentityError> for NodeRuntimeApprovalError {
    fn from(error: ExecutableIdentityError) -> Self {
        match error {
            ExecutableIdentityError::Invalid(message) => Self::InvalidExecutable(message),
            ExecutableIdentityError::Changed => Self::ExecutableChanged,
            ExecutableIdentityError::Cancelled => Self::Cancelled,
            ExecutableIdentityError::DeadlineExceeded => Self::DeadlineExceeded,
        }
    }
}

/// Request and probe one explicitly selected local Node executable.
///
/// Broker evaluation happens before `execute_bounded`, and the canonical path
/// plus content fingerprint are checked again after the child exits. This
/// closes ordinary replacement/symlink changes; a platform cannot eliminate
/// the final OS-level TOCTOU interval. The receipt is identity evidence and
/// does not mint a fresh launch grant, so later launch code must revalidate it
/// and obtain fresh broker authority before binding an LSP process.
pub fn approve_node_runtime(
    broker: &dyn CapabilityBrokerPort,
    process: &dyn ProcessService,
    request: NodeRuntimeApprovalRequest,
    cancellation: Arc<AtomicBool>,
) -> Result<ApprovedNodeRuntime, NodeRuntimeApprovalError> {
    validate_request(&request)?;
    if request.workspace_trust_state != WorkspaceTrustState::Trusted {
        return Err(NodeRuntimeApprovalError::CapabilityRejected(
            "workspace is not trusted".to_string(),
        ));
    }

    let deadline = Instant::now() + NODE_RUNTIME_PROBE_TIMEOUT;
    let (canonical_path, fingerprint) =
        canonical_regular_file(&request.executable, &cancellation, deadline)?;
    let canonical_command = canonical_path_to_string(&canonical_path)?;
    let capability_id = CapabilityId(NODE_RUNTIME_PROBE_CAPABILITY.to_string());
    let decision_id = request_granted_decision(
        broker,
        &request.principal_id,
        &capability_id,
        &request.workspace_trust_state,
        CanonicalPath(canonical_command.clone()),
        language_tool_capability_context(&canonical_command),
        request.correlation_id,
    )
    .map_err(NodeRuntimeApprovalError::CapabilityRejected)?;

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

    let probe = run_bounded_probe(
        process,
        canonical_command,
        vec!["--version".to_string()],
        NODE_RUNTIME_PROBE_STREAM_LIMIT,
        deadline,
        cancellation.clone(),
    )
    .map_err(|error| match error {
        BoundedProbeError::DeadlineExceeded => NodeRuntimeApprovalError::DeadlineExceeded,
        BoundedProbeError::Cancelled => NodeRuntimeApprovalError::Cancelled,
        BoundedProbeError::Failed(message) => NodeRuntimeApprovalError::ProbeFailed(message),
    })?;
    // The process authority may return successfully just as cancellation is
    // raised. Do not mint a receipt from that race.
    check_budget(&cancellation, deadline)?;
    if probe.exit_code != 0 {
        return Err(NodeRuntimeApprovalError::ProbeFailed(
            "probe exited unsuccessfully".to_string(),
        ));
    }
    let observed_version = LspNodeVersion::parse(&probe.stdout)
        .map_err(|_| NodeRuntimeApprovalError::MalformedVersion)?;
    if observed_version < request.minimum_version {
        return Err(NodeRuntimeApprovalError::ProbeFailed(
            "observed Node version is below the required minimum".to_string(),
        ));
    }
    recheck_executable_identity(
        &request.executable,
        &canonical_path,
        &fingerprint,
        &cancellation,
        deadline,
    )?;

    Ok(ApprovedNodeRuntime {
        canonical_path,
        observed_version,
        fingerprint,
        decision_id,
        principal_id: request.principal_id,
        workspace_id: request.workspace_id,
        workspace_trust_state: request.workspace_trust_state,
        correlation_id: request.correlation_id,
        causality_id: request.causality_id,
    })
}

impl ApprovedNodeRuntime {
    /// Revalidates this receipt against the context and current executable
    /// identity immediately before a later LSP launch binding.
    pub fn revalidate(
        &self,
        expected: &NodeRuntimeApprovalRequest,
        minimum_version: LspNodeVersion,
        cancellation: Arc<AtomicBool>,
    ) -> Result<(), NodeRuntimeApprovalError> {
        validate_request(expected)?;
        if expected.workspace_trust_state != WorkspaceTrustState::Trusted
            || expected.principal_id != self.principal_id
            || expected.workspace_id != self.workspace_id
            || expected.workspace_trust_state != self.workspace_trust_state
        {
            return Err(NodeRuntimeApprovalError::CapabilityRejected(
                "runtime receipt context does not match launch context".to_string(),
            ));
        }
        if self.observed_version < minimum_version {
            return Err(NodeRuntimeApprovalError::ProbeFailed(
                "approved Node version is below the required launch minimum".to_string(),
            ));
        }
        let deadline = Instant::now() + NODE_RUNTIME_PROBE_TIMEOUT;
        let (path, fingerprint) =
            canonical_regular_file(&expected.executable, &cancellation, deadline)?;
        if path != self.canonical_path || fingerprint != self.fingerprint {
            return Err(NodeRuntimeApprovalError::ExecutableChanged);
        }
        Ok(())
    }
}

fn validate_request(request: &NodeRuntimeApprovalRequest) -> Result<(), NodeRuntimeApprovalError> {
    if request.principal_id.0.is_empty() {
        return Err(NodeRuntimeApprovalError::InvalidRequest(
            "principal is empty",
        ));
    }
    if request.workspace_id.0 == 0 {
        return Err(NodeRuntimeApprovalError::InvalidRequest(
            "workspace id is zero",
        ));
    }
    if request.correlation_id.0 == 0 {
        return Err(NodeRuntimeApprovalError::InvalidRequest(
            "correlation id is zero",
        ));
    }
    if request.causality_id.0.is_nil() {
        return Err(NodeRuntimeApprovalError::InvalidRequest(
            "causality id is nil",
        ));
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Shared executable-approval mechanics.
//
// Every item below is boundary-agnostic and is used by both
// `approve_node_runtime` and the sibling `formatter_approval` module. Keeping
// exactly one copy is deliberate: a second copy would be a second place where
// the TOCTOU window could be reopened by a partial edit.
// ---------------------------------------------------------------------------

/// Failure of a bounded identity or budget check, mapped by each boundary into
/// its own public error enum. It never carries process output.
#[derive(Debug)]
pub(crate) enum ExecutableIdentityError {
    /// The selected path could not be canonicalized or is not a regular file.
    Invalid(String),
    /// The canonical path or content fingerprint changed.
    Changed,
    /// The bounded operation was cancelled.
    Cancelled,
    /// The bounded operation exceeded its finite deadline.
    DeadlineExceeded,
}

/// Failure of the bounded child probe. `Failed` carries the platform error
/// text only; probe stdout/stderr never reaches this value.
#[derive(Debug)]
pub(crate) enum BoundedProbeError {
    /// No budget remained for the child before it was started, or the platform
    /// classified the running child as timed out.
    DeadlineExceeded,
    /// The cancellation flag was raised while the child was running.
    Cancelled,
    /// The bounded process authority refused or failed the child.
    Failed(String),
}

/// Canonicalizes `path`, proves it is a bounded regular file, and hashes its
/// content under the shared cancellation flag and finite deadline.
pub(crate) fn canonical_regular_file(
    path: &Path,
    cancellation: &AtomicBool,
    deadline: Instant,
) -> Result<(PathBuf, FileFingerprint), ExecutableIdentityError> {
    check_budget(cancellation, deadline)?;
    if !path.is_absolute() {
        return Err(ExecutableIdentityError::Invalid(
            "selected executable path must be absolute".to_string(),
        ));
    }
    let canonical = std::fs::canonicalize(path)
        .map_err(|error| ExecutableIdentityError::Invalid(error.to_string()))?;
    let metadata = std::fs::metadata(&canonical)
        .map_err(|error| ExecutableIdentityError::Invalid(error.to_string()))?;
    if !metadata.is_file() {
        return Err(ExecutableIdentityError::Invalid(
            "selected path is not a regular file".to_string(),
        ));
    }
    if metadata.len() > EXECUTABLE_FINGERPRINT_MAX_BYTES {
        return Err(ExecutableIdentityError::Invalid(
            "selected executable exceeds bounded fingerprint size".to_string(),
        ));
    }
    let mut file = File::open(&canonical)
        .map_err(|error| ExecutableIdentityError::Invalid(error.to_string()))?;
    let opened_metadata = file
        .metadata()
        .map_err(|error| ExecutableIdentityError::Invalid(error.to_string()))?;
    if !opened_metadata.is_file() {
        return Err(ExecutableIdentityError::Invalid(
            "opened executable is not a regular file".to_string(),
        ));
    }
    if opened_metadata.len() > EXECUTABLE_FINGERPRINT_MAX_BYTES {
        return Err(ExecutableIdentityError::Invalid(
            "opened executable exceeds bounded fingerprint size".to_string(),
        ));
    }
    if std::fs::canonicalize(path).ok().as_deref() != Some(canonical.as_path()) {
        return Err(ExecutableIdentityError::Changed);
    }
    let opened_length = opened_metadata.len();
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 64 * 1024];
    let mut hashed_bytes = 0u64;
    loop {
        check_budget(cancellation, deadline)?;
        let read = file
            .read(&mut buffer)
            .map_err(|error| ExecutableIdentityError::Invalid(error.to_string()))?;
        if read == 0 {
            break;
        }
        hashed_bytes = hashed_bytes.saturating_add(read as u64);
        if hashed_bytes > EXECUTABLE_FINGERPRINT_MAX_BYTES {
            return Err(ExecutableIdentityError::Invalid(
                "executable grew beyond bounded fingerprint size".to_string(),
            ));
        }
        hasher.update(&buffer[..read]);
    }
    check_budget(cancellation, deadline)?;
    let final_metadata = file
        .metadata()
        .map_err(|error| ExecutableIdentityError::Invalid(error.to_string()))?;
    if !final_metadata.is_file() || final_metadata.len() != opened_length {
        return Err(ExecutableIdentityError::Changed);
    }
    let value = hex::encode(hasher.finalize());
    Ok((
        canonical,
        FileFingerprint {
            algorithm: "sha256-content-v1".to_string(),
            value,
        },
    ))
}

/// Re-reads the selected path and requires the same canonical path and the
/// same content fingerprint as the identity captured earlier.
pub(crate) fn recheck_executable_identity(
    path: &Path,
    expected_path: &Path,
    expected_fingerprint: &FileFingerprint,
    cancellation: &AtomicBool,
    deadline: Instant,
) -> Result<(), ExecutableIdentityError> {
    check_budget(cancellation, deadline)?;
    let (observed_path, observed_fingerprint) =
        canonical_regular_file(path, cancellation, deadline)?;
    if observed_path != expected_path || &observed_fingerprint != expected_fingerprint {
        return Err(ExecutableIdentityError::Changed);
    }
    Ok(())
}

/// Renders a canonical path as UTF-8 for the broker request and the child
/// command, refusing anything that cannot be represented exactly.
pub(crate) fn canonical_path_to_string(path: &Path) -> Result<String, ExecutableIdentityError> {
    path.to_str().map(str::to_owned).ok_or_else(|| {
        ExecutableIdentityError::Invalid(
            "canonical executable path is not representable as UTF-8".to_string(),
        )
    })
}

/// Checks the shared cancellation flag and the single finite deadline.
pub(crate) fn check_budget(
    cancellation: &AtomicBool,
    deadline: Instant,
) -> Result<(), ExecutableIdentityError> {
    if cancellation.load(std::sync::atomic::Ordering::Relaxed) {
        return Err(ExecutableIdentityError::Cancelled);
    }
    if Instant::now() >= deadline {
        return Err(ExecutableIdentityError::DeadlineExceeded);
    }
    Ok(())
}

/// Context handed to capability policies for a local language-tooling binary.
///
/// The `lsp.launch` policy branch matches on `lsp_server_binary`, so the
/// canonical path is supplied there as well as in `command_binary`; both are
/// the exact canonical path that was fingerprinted.
pub(crate) fn language_tool_capability_context(command_binary: &str) -> CapabilityRequestContext {
    CapabilityRequestContext {
        command_binary: Some(command_binary.to_string()),
        command_class: Some(CapabilityCommandClass::LanguageServer),
        lsp_server_binary: Some(command_binary.to_string()),
        ..CapabilityRequestContext::default()
    }
}

/// Asks the broker for `capability_id` against the canonical target path and
/// accepts only a granted, non-zero decision for that exact capability.
///
/// The error string is a rejection reason built from decision metadata and the
/// broker's own error text; no process output can reach it.
pub(crate) fn request_granted_decision(
    broker: &dyn CapabilityBrokerPort,
    principal_id: &PrincipalId,
    capability_id: &CapabilityId,
    workspace_trust_state: &WorkspaceTrustState,
    target_path: CanonicalPath,
    context: CapabilityRequestContext,
    correlation_id: CorrelationId,
) -> Result<CapabilityDecisionId, String> {
    let response = broker
        .handle(CapabilityRequest::Request {
            principal_id: principal_id.clone(),
            capability_id: capability_id.clone(),
            workspace_trust_state: workspace_trust_state.clone(),
            target_path: Some(target_path),
            decision_id: None,
            context,
            correlation_id,
        })
        .map_err(|error| error.message)?;
    match response {
        CapabilityResponse::Decision(decision)
            if decision.granted
                && decision.decision_id.0 != 0
                && &decision.capability == capability_id =>
        {
            Ok(decision.decision_id)
        }
        CapabilityResponse::Decision(decision) => Err(format!(
            "decision did not grant requested capability (id={}, granted={}, capability={})",
            decision.decision_id.0, decision.granted, decision.capability.0
        )),
        CapabilityResponse::Granted(grant)
            if grant.decision_id.0 != 0
                && &grant.principal_id == principal_id
                && &grant.capability_id == capability_id =>
        {
            Ok(grant.decision_id)
        }
        CapabilityResponse::Granted(_) => {
            Err("grant response did not match principal or capability".to_string())
        }
        CapabilityResponse::Denied(denial) => Err(denial.reason),
    }
}

/// Runs one bounded child under the remaining shared budget with explicit
/// stream caps. The argument vector is supplied by the caller; this helper
/// never adds, rewrites or infers arguments, and never inspects the output.
pub(crate) fn run_bounded_probe(
    process: &dyn ProcessService,
    command: String,
    args: Vec<String>,
    stream_limit: usize,
    deadline: Instant,
    cancellation: Arc<AtomicBool>,
) -> Result<ProcessResult, BoundedProbeError> {
    let remaining = deadline.saturating_duration_since(Instant::now());
    if remaining.is_zero() {
        return Err(BoundedProbeError::DeadlineExceeded);
    }
    process
        .execute_bounded(&BoundedProcessRequest::new(
            ProcessRequest {
                command,
                args,
                cwd: None,
                env: Vec::new(),
                stdin: None,
                timeout: Some(remaining),
                cancelled: false,
            },
            stream_limit,
            stream_limit,
            remaining,
            cancellation,
        ))
        .map_err(|error| match error {
            PlatformError::Cancelled { .. } => BoundedProbeError::Cancelled,
            PlatformError::Timeout { .. } => BoundedProbeError::DeadlineExceeded,
            other => BoundedProbeError::Failed(platform_error_message(other)),
        })
}

fn platform_error_message(error: PlatformError) -> String {
    error.to_string()
}
