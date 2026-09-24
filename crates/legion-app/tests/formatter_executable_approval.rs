use std::{
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
    time::{Duration, Instant},
};

use legion_app::language::{
    FORMATTER_PROBE_STREAM_LIMIT, FORMATTER_PROBE_TIMEOUT, FormatterApprovalError,
    FormatterApprovalRequest, FormatterProbe, FormatterProbeOutcome, approve_formatter_executable,
};
use legion_platform::{
    BoundedProcessRequest, PlatformError, ProcessRequest, ProcessResult, ProcessService,
};
use legion_protocol::{
    CapabilityBrokerPort, CapabilityDecision, CapabilityId, CapabilityRequest, CapabilityResponse,
    CausalityId, CorrelationId, PrincipalId, ProtocolResult, WorkspaceId, WorkspaceTrustState,
};
use uuid::Uuid;

/// Content written by every fixture. It is exactly as long as
/// [`REPLACEMENT_CONTENT`], so a replacement can only be caught by comparing
/// the content fingerprint, never the file length.
const ORIGINAL_CONTENT: &[u8] = b"original-fmt";

/// Same-size replacement used to reopen the identity window mid-approval.
const REPLACEMENT_CONTENT: &[u8] = b"replaced-fmt";

struct FakeBroker {
    response: Mutex<Option<CapabilityResponse>>,
    request: Mutex<Option<CapabilityRequest>>,
    mutate_after_decision: Option<PathBuf>,
}

impl CapabilityBrokerPort for FakeBroker {
    fn handle(&self, request: CapabilityRequest) -> ProtocolResult<CapabilityResponse> {
        *self.request.lock().unwrap() = Some(request);
        if let Some(path) = &self.mutate_after_decision {
            std::fs::write(path, REPLACEMENT_CONTENT).unwrap();
        }
        Ok(self.response.lock().unwrap().take().expect("response"))
    }
}

struct FakeProcess {
    calls: AtomicUsize,
    request: Mutex<Option<BoundedProcessRequest>>,
    result: Mutex<ProcessResult>,
    cancel_before_return: Option<Arc<AtomicBool>>,
    mutate_before_return: Option<PathBuf>,
}

impl ProcessService for FakeProcess {
    fn execute(&self, _request: &ProcessRequest) -> Result<ProcessResult, PlatformError> {
        Err(PlatformError::UnsupportedOperation {
            operation: "test execute".to_string(),
            path: PathBuf::new(),
            reason: "not used".to_string(),
        })
    }

    fn execute_bounded(
        &self,
        request: &BoundedProcessRequest,
    ) -> Result<ProcessResult, PlatformError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        *self.request.lock().unwrap() = Some(request.clone());
        if let Some(flag) = &self.cancel_before_return {
            flag.store(true, Ordering::SeqCst);
        }
        if let Some(path) = &self.mutate_before_return {
            std::fs::write(path, REPLACEMENT_CONTENT).unwrap();
        }
        Ok(self.result.lock().unwrap().clone())
    }
}

fn fixture(dir: &tempfile::TempDir) -> PathBuf {
    let path = dir.path().join("legion-test-formatter");
    std::fs::write(&path, ORIGINAL_CONTENT).unwrap();
    path
}

fn request(path: PathBuf) -> FormatterApprovalRequest {
    FormatterApprovalRequest {
        executable: path,
        principal_id: PrincipalId("operator".to_string()),
        workspace_id: WorkspaceId(42),
        workspace_trust_state: WorkspaceTrustState::Trusted,
        correlation_id: CorrelationId(77),
        causality_id: CausalityId(Uuid::from_u128(7)),
        probe: FormatterProbe::Arguments {
            args: vec!["--check".to_string(), "--quiet".to_string()],
            expected_exit_code: 0,
        },
        deadline: None,
    }
}

fn granted() -> CapabilityResponse {
    CapabilityResponse::Decision(CapabilityDecision {
        decision_id: legion_protocol::CapabilityDecisionId(9),
        granted: true,
        capability: CapabilityId("lsp.launch".to_string()),
        reason: None,
    })
}

fn broker(response: Option<CapabilityResponse>) -> FakeBroker {
    FakeBroker {
        response: Mutex::new(response),
        request: Mutex::new(None),
        mutate_after_decision: None,
    }
}

fn process(exit_code: i32) -> FakeProcess {
    FakeProcess {
        calls: AtomicUsize::new(0),
        request: Mutex::new(None),
        result: Mutex::new(ProcessResult {
            exit_code,
            // Deliberately non-empty: nothing below may observe probe output,
            // and no error or receipt may carry it.
            stdout: "formatter stdout that must never be retained".to_string(),
            stderr: "formatter stderr that must never be retained".to_string(),
            elapsed: Duration::from_millis(1),
        }),
        cancel_before_return: None,
        mutate_before_return: None,
    }
}

#[test]
fn denied_formatter_never_spawns() {
    let dir = tempfile::tempdir().unwrap();
    let path = fixture(&dir);
    let broker = broker(Some(CapabilityResponse::Denied(
        legion_protocol::CapabilityDenial {
            decision_id: legion_protocol::CapabilityDecisionId(3),
            principal_id: PrincipalId("operator".to_string()),
            capability_id: CapabilityId("lsp.launch".to_string()),
            reason: "denied".to_string(),
        },
    )));
    let process = process(0);
    let result = approve_formatter_executable(
        &broker,
        &process,
        request(path),
        Arc::new(AtomicBool::new(false)),
    );
    assert!(matches!(
        result,
        Err(FormatterApprovalError::CapabilityRejected(_))
    ));
    assert_eq!(process.calls.load(Ordering::SeqCst), 0);
}

#[test]
fn untrusted_formatter_denies_before_filesystem_or_process_effects() {
    let dir = tempfile::tempdir().unwrap();
    // Absolute but absent: canonicalizing it would fail, so a rejection on the
    // capability path proves the trust gate ran before any filesystem effect.
    let absent = dir.path().join("legion-test-formatter-absent");
    let broker = broker(None);
    let process = process(0);
    let mut untrusted = request(absent);
    untrusted.workspace_trust_state = WorkspaceTrustState::Untrusted;
    let result = approve_formatter_executable(
        &broker,
        &process,
        untrusted,
        Arc::new(AtomicBool::new(false)),
    );
    assert!(matches!(
        result,
        Err(FormatterApprovalError::CapabilityRejected(_))
    ));
    assert!(broker.request.lock().unwrap().is_none());
    assert_eq!(process.calls.load(Ordering::SeqCst), 0);
}

#[test]
fn path_resolved_formatter_name_is_rejected_before_broker_or_process() {
    let broker = broker(Some(granted()));
    let process = process(0);
    let result = approve_formatter_executable(
        &broker,
        &process,
        request(PathBuf::from("black")),
        Arc::new(AtomicBool::new(false)),
    );
    assert!(matches!(
        result,
        Err(FormatterApprovalError::InvalidRequest(_))
    ));
    assert!(broker.request.lock().unwrap().is_none());
    assert_eq!(process.calls.load(Ordering::SeqCst), 0);
}

#[test]
fn wrong_decision_response_is_rejected_before_spawn() {
    let dir = tempfile::tempdir().unwrap();
    let path = fixture(&dir);
    let broker = broker(Some(CapabilityResponse::Decision(CapabilityDecision {
        decision_id: legion_protocol::CapabilityDecisionId(8),
        granted: true,
        capability: CapabilityId("process.spawn".to_string()),
        reason: None,
    })));
    let process = process(0);
    let result = approve_formatter_executable(
        &broker,
        &process,
        request(path),
        Arc::new(AtomicBool::new(false)),
    );
    assert!(matches!(
        result,
        Err(FormatterApprovalError::CapabilityRejected(_))
    ));
    assert_eq!(process.calls.load(Ordering::SeqCst), 0);
}

#[test]
fn successful_probe_forwards_exact_path_args_and_bounds() {
    let dir = tempfile::tempdir().unwrap();
    let path = fixture(&dir);
    let broker = broker(Some(granted()));
    let process = process(0);
    let approved = approve_formatter_executable(
        &broker,
        &process,
        request(path),
        Arc::new(AtomicBool::new(false)),
    )
    .unwrap();
    assert_eq!(process.calls.load(Ordering::SeqCst), 1);
    assert_eq!(approved.decision_id().0, 9);
    assert_eq!(approved.probe_outcome(), FormatterProbeOutcome::Executed);
    assert_eq!(approved.fingerprint().algorithm, "sha256-content-v1");
    let sent = process.request.lock().unwrap().as_ref().unwrap().clone();
    assert_eq!(
        sent.process.command,
        approved.canonical_path().to_string_lossy()
    );
    assert_eq!(sent.process.args, vec!["--check", "--quiet"]);
    assert_eq!(sent.max_stdout_bytes, FORMATTER_PROBE_STREAM_LIMIT);
    assert_eq!(sent.max_stderr_bytes, FORMATTER_PROBE_STREAM_LIMIT);
    assert!(sent.timeout > Duration::ZERO);
    assert!(sent.timeout <= FORMATTER_PROBE_TIMEOUT);
    assert_eq!(sent.timeout, sent.process.timeout.unwrap());
    let CapabilityRequest::Request {
        capability_id,
        target_path,
        context,
        ..
    } = broker.request.lock().unwrap().clone().unwrap()
    else {
        panic!("request")
    };
    assert_eq!(capability_id.0, "lsp.launch");
    assert_eq!(
        target_path.unwrap().0,
        approved.canonical_path().to_string_lossy()
    );
    let canonical = approved.canonical_path().to_string_lossy().into_owned();
    assert_eq!(context.command_binary.as_deref(), Some(canonical.as_str()));
    assert_eq!(
        context.lsp_server_binary.as_deref(),
        context.command_binary.as_deref()
    );
}

#[test]
fn broker_window_replacement_is_rejected_before_spawn() {
    let dir = tempfile::tempdir().unwrap();
    let path = fixture(&dir);
    let original_len = std::fs::metadata(&path).unwrap().len();
    let mut broker = broker(Some(granted()));
    broker.mutate_after_decision = Some(path.clone());
    let process = process(0);
    let result = approve_formatter_executable(
        &broker,
        &process,
        request(path.clone()),
        Arc::new(AtomicBool::new(false)),
    );
    assert!(matches!(
        result,
        Err(FormatterApprovalError::ExecutableChanged)
    ));
    assert_eq!(process.calls.load(Ordering::SeqCst), 0);
    assert_eq!(std::fs::metadata(&path).unwrap().len(), original_len);
}

#[test]
fn replacement_after_probe_does_not_mint_receipt() {
    let dir = tempfile::tempdir().unwrap();
    let path = fixture(&dir);
    let original_len = std::fs::metadata(&path).unwrap().len();
    let broker = broker(Some(granted()));
    let mut process = process(0);
    process.mutate_before_return = Some(path.clone());
    let result = approve_formatter_executable(
        &broker,
        &process,
        request(path.clone()),
        Arc::new(AtomicBool::new(false)),
    );
    assert!(matches!(
        result,
        Err(FormatterApprovalError::ExecutableChanged)
    ));
    assert_eq!(process.calls.load(Ordering::SeqCst), 1);
    assert_eq!(std::fs::metadata(&path).unwrap().len(), original_len);
}

#[test]
fn cancellation_and_deadline_have_no_broker_or_process_effect() {
    let dir = tempfile::tempdir().unwrap();
    let path = fixture(&dir);

    let cancelled_broker = broker(Some(granted()));
    let cancelled_process = process(0);
    let cancelled = approve_formatter_executable(
        &cancelled_broker,
        &cancelled_process,
        request(path.clone()),
        Arc::new(AtomicBool::new(true)),
    );
    assert!(matches!(cancelled, Err(FormatterApprovalError::Cancelled)));
    assert!(cancelled_broker.request.lock().unwrap().is_none());
    assert_eq!(cancelled_process.calls.load(Ordering::SeqCst), 0);

    let expired_broker = broker(Some(granted()));
    let expired_process = process(0);
    let mut expired_request = request(path);
    expired_request.deadline = Some(Instant::now());
    let expired = approve_formatter_executable(
        &expired_broker,
        &expired_process,
        expired_request,
        Arc::new(AtomicBool::new(false)),
    );
    assert!(matches!(
        expired,
        Err(FormatterApprovalError::DeadlineExceeded)
    ));
    assert!(expired_broker.request.lock().unwrap().is_none());
    assert_eq!(expired_process.calls.load(Ordering::SeqCst), 0);
}

#[test]
fn receipt_revalidation_rejects_context_mismatch() {
    let dir = tempfile::tempdir().unwrap();
    let path = fixture(&dir);
    let broker = broker(Some(granted()));
    let process = process(0);
    let approved = approve_formatter_executable(
        &broker,
        &process,
        request(path.clone()),
        Arc::new(AtomicBool::new(false)),
    )
    .unwrap();
    let flag = Arc::new(AtomicBool::new(false));

    let mut other_principal = request(path.clone());
    other_principal.principal_id = PrincipalId("other-principal".to_string());
    let bad = approved.revalidate(&other_principal, flag.clone());
    assert!(matches!(
        bad,
        Err(FormatterApprovalError::CapabilityRejected(_))
    ));

    let mut other_probe = request(path.clone());
    other_probe.probe = FormatterProbe::Arguments {
        args: vec!["--write".to_string()],
        expected_exit_code: 0,
    };
    let bad = approved.revalidate(&other_probe, flag.clone());
    assert!(matches!(
        bad,
        Err(FormatterApprovalError::CapabilityRejected(_))
    ));

    let accepted = approved.revalidate(&request(path), flag);
    assert!(accepted.is_ok());
}

#[test]
fn receipt_revalidation_rejects_same_size_replacement() {
    let dir = tempfile::tempdir().unwrap();
    let path = fixture(&dir);
    let original_len = std::fs::metadata(&path).unwrap().len();
    let broker = broker(Some(granted()));
    let process = process(0);
    let approved = approve_formatter_executable(
        &broker,
        &process,
        request(path.clone()),
        Arc::new(AtomicBool::new(false)),
    )
    .unwrap();
    std::fs::write(&path, REPLACEMENT_CONTENT).unwrap();
    assert_eq!(std::fs::metadata(&path).unwrap().len(), original_len);
    let flag = Arc::new(AtomicBool::new(false));
    let bad = approved.revalidate(&request(path), flag);
    assert!(matches!(
        bad,
        Err(FormatterApprovalError::ExecutableChanged)
    ));
}

#[test]
fn unexpected_exit_status_never_mints_receipt() {
    let dir = tempfile::tempdir().unwrap();
    let path = fixture(&dir);
    let broker = broker(Some(granted()));
    let process = process(3);
    let result = approve_formatter_executable(
        &broker,
        &process,
        request(path),
        Arc::new(AtomicBool::new(false)),
    );
    assert!(matches!(
        result,
        Err(FormatterApprovalError::ProbeFailed(_))
    ));
    assert_eq!(process.calls.load(Ordering::SeqCst), 1);
}

#[test]
fn unprobeable_formatter_is_recorded_without_spawning() {
    let dir = tempfile::tempdir().unwrap();
    let path = fixture(&dir);
    let broker = broker(Some(granted()));
    let process = process(0);
    let mut unprobeable = request(path);
    unprobeable.probe = FormatterProbe::Unprobeable {
        reason: "vendored formatter exposes no probe invocation".to_string(),
    };
    let approved = approve_formatter_executable(
        &broker,
        &process,
        unprobeable,
        Arc::new(AtomicBool::new(false)),
    )
    .unwrap();
    assert_eq!(approved.probe_outcome(), FormatterProbeOutcome::NotProbed);
    assert_eq!(process.calls.load(Ordering::SeqCst), 0);
    assert!(broker.request.lock().unwrap().is_some());
}

#[test]
fn empty_probe_arguments_are_rejected_before_broker_or_process() {
    let dir = tempfile::tempdir().unwrap();
    let path = fixture(&dir);
    let broker = broker(Some(granted()));
    let process = process(0);
    let mut no_arguments = request(path);
    no_arguments.probe = FormatterProbe::Arguments {
        args: Vec::new(),
        expected_exit_code: 0,
    };
    let result = approve_formatter_executable(
        &broker,
        &process,
        no_arguments,
        Arc::new(AtomicBool::new(false)),
    );
    assert!(matches!(
        result,
        Err(FormatterApprovalError::InvalidRequest(_))
    ));
    assert!(broker.request.lock().unwrap().is_none());
    assert_eq!(process.calls.load(Ordering::SeqCst), 0);
}

#[test]
fn cancellation_after_probe_completion_does_not_mint_receipt() {
    let dir = tempfile::tempdir().unwrap();
    let path = fixture(&dir);
    let broker = broker(Some(granted()));
    let cancellation = Arc::new(AtomicBool::new(false));
    let mut process = process(0);
    process.cancel_before_return = Some(cancellation.clone());
    let result = approve_formatter_executable(&broker, &process, request(path), cancellation);
    assert!(matches!(result, Err(FormatterApprovalError::Cancelled)));
    assert_eq!(process.calls.load(Ordering::SeqCst), 1);
}
