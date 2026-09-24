use std::{
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
    time::Duration,
};

use legion_app::language::{
    NODE_RUNTIME_PROBE_STREAM_LIMIT, NODE_RUNTIME_PROBE_TIMEOUT, NodeRuntimeApprovalRequest,
    approve_node_runtime,
};
use legion_lsp::LspNodeVersion;
use legion_platform::{
    BoundedProcessRequest, NativeProcessService, PlatformError, ProcessRequest, ProcessResult,
    ProcessService,
};
use legion_protocol::{
    CapabilityBrokerPort, CapabilityDecision, CapabilityId, CapabilityRequest, CapabilityResponse,
    CausalityId, CorrelationId, PrincipalId, ProtocolResult, WorkspaceId, WorkspaceTrustState,
};
use uuid::Uuid;

struct FakeBroker {
    response: Mutex<Option<CapabilityResponse>>,
    request: Mutex<Option<CapabilityRequest>>,
    mutate_after_decision: Option<PathBuf>,
}

impl CapabilityBrokerPort for FakeBroker {
    fn handle(&self, request: CapabilityRequest) -> ProtocolResult<CapabilityResponse> {
        *self.request.lock().unwrap() = Some(request);
        if let Some(path) = &self.mutate_after_decision {
            std::fs::write(path, b"replacement-node").unwrap();
        }
        Ok(self.response.lock().unwrap().take().expect("response"))
    }
}

struct FakeProcess {
    calls: AtomicUsize,
    request: Mutex<Option<BoundedProcessRequest>>,
    result: Mutex<ProcessResult>,
    cancel_before_return: Option<Arc<AtomicBool>>,
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
        Ok(self.result.lock().unwrap().clone())
    }
}

fn request(path: PathBuf) -> NodeRuntimeApprovalRequest {
    NodeRuntimeApprovalRequest {
        executable: path,
        principal_id: PrincipalId("operator".to_string()),
        workspace_id: WorkspaceId(42),
        workspace_trust_state: WorkspaceTrustState::Trusted,
        correlation_id: CorrelationId(77),
        causality_id: CausalityId(Uuid::from_u128(7)),
        minimum_version: LspNodeVersion {
            major: 14,
            minor: 0,
            patch: 0,
        },
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

fn process(stdout: &str, exit_code: i32) -> FakeProcess {
    FakeProcess {
        calls: AtomicUsize::new(0),
        request: Mutex::new(None),
        result: Mutex::new(ProcessResult {
            exit_code,
            stdout: stdout.to_string(),
            stderr: String::new(),
            elapsed: Duration::from_millis(1),
        }),
        cancel_before_return: None,
    }
}

#[test]
fn denied_runtime_never_spawns() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("node");
    std::fs::write(&path, b"node").unwrap();
    let broker = FakeBroker {
        response: Mutex::new(Some(CapabilityResponse::Denied(
            legion_protocol::CapabilityDenial {
                decision_id: legion_protocol::CapabilityDecisionId(3),
                principal_id: PrincipalId("operator".to_string()),
                capability_id: CapabilityId("lsp.launch".to_string()),
                reason: "denied".to_string(),
            },
        ))),
        request: Mutex::new(None),
        mutate_after_decision: None,
    };
    let process = process("v20.1.0\n", 0);
    let result = approve_node_runtime(
        &broker,
        &process,
        request(path),
        Arc::new(AtomicBool::new(false)),
    );
    assert!(result.is_err());
    assert_eq!(process.calls.load(Ordering::SeqCst), 0);
}

#[test]
fn untrusted_runtime_denies_before_filesystem_or_process_effects() {
    let broker = FakeBroker {
        response: Mutex::new(None),
        request: Mutex::new(None),
        mutate_after_decision: None,
    };
    let process = process("v20.1.0\n", 0);
    let mut denied = request(PathBuf::from("definitely-not-present/node"));
    denied.workspace_trust_state = WorkspaceTrustState::Untrusted;
    assert!(
        approve_node_runtime(&broker, &process, denied, Arc::new(AtomicBool::new(false))).is_err()
    );
    assert!(broker.request.lock().unwrap().is_none());
    assert_eq!(process.calls.load(Ordering::SeqCst), 0);
}

#[test]
fn relative_runtime_path_is_rejected_before_broker_or_process() {
    let broker = FakeBroker {
        response: Mutex::new(Some(granted())),
        request: Mutex::new(None),
        mutate_after_decision: None,
    };
    let process = process("v20.1.0\n", 0);
    assert!(
        approve_node_runtime(
            &broker,
            &process,
            request(PathBuf::from("node")),
            Arc::new(AtomicBool::new(false))
        )
        .is_err()
    );
    assert!(broker.request.lock().unwrap().is_none());
    assert_eq!(process.calls.load(Ordering::SeqCst), 0);
}

#[test]
fn wrong_decision_response_is_rejected_before_spawn() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("node");
    std::fs::write(&path, b"node").unwrap();
    let broker = FakeBroker {
        response: Mutex::new(Some(CapabilityResponse::Decision(CapabilityDecision {
            decision_id: legion_protocol::CapabilityDecisionId(8),
            granted: true,
            capability: CapabilityId("process.spawn".to_string()),
            reason: None,
        }))),
        request: Mutex::new(None),
        mutate_after_decision: None,
    };
    let process = process("v20.1.0\n", 0);
    assert!(
        approve_node_runtime(
            &broker,
            &process,
            request(path),
            Arc::new(AtomicBool::new(false))
        )
        .is_err()
    );
    assert_eq!(process.calls.load(Ordering::SeqCst), 0);
}

#[test]
fn successful_probe_forwards_exact_path_version_flag_and_bounds() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("node");
    std::fs::write(&path, b"node").unwrap();
    let broker = FakeBroker {
        response: Mutex::new(Some(granted())),
        request: Mutex::new(None),
        mutate_after_decision: None,
    };
    let process = process("v20.11.1\n", 0);
    let runtime = approve_node_runtime(
        &broker,
        &process,
        request(path.clone()),
        Arc::new(AtomicBool::new(false)),
    )
    .unwrap();
    assert_eq!(runtime.observed_version().major, 20);
    assert_eq!(runtime.decision_id().0, 9);
    let sent = process.request.lock().unwrap().as_ref().unwrap().clone();
    assert_eq!(
        sent.process.command,
        runtime.canonical_path().to_string_lossy()
    );
    assert_eq!(sent.process.args, vec!["--version"]);
    assert_eq!(sent.max_stdout_bytes, NODE_RUNTIME_PROBE_STREAM_LIMIT);
    assert_eq!(sent.max_stderr_bytes, NODE_RUNTIME_PROBE_STREAM_LIMIT);
    assert!(sent.timeout > Duration::ZERO);
    assert!(sent.timeout <= NODE_RUNTIME_PROBE_TIMEOUT);
    assert_eq!(sent.timeout, sent.process.timeout.unwrap());
    let CapabilityRequest::Request {
        target_path,
        context,
        ..
    } = broker.request.lock().unwrap().clone().unwrap()
    else {
        panic!("request")
    };
    assert_eq!(
        target_path.unwrap().0,
        runtime.canonical_path().to_string_lossy()
    );
    let canonical = runtime.canonical_path().to_string_lossy().into_owned();
    assert_eq!(context.command_binary.as_deref(), Some(canonical.as_str()));
    assert_eq!(
        context.lsp_server_binary.as_deref(),
        context.command_binary.as_deref()
    );
}

#[test]
fn malformed_or_failed_probe_never_mints_runtime() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("node");
    std::fs::write(&path, b"node").unwrap();
    for (stdout, exit_code) in [
        ("v20.1.0\nv21.0.0\n", 0),
        ("v20.1.0\n", 1),
        ("v12.0.0\n", 0),
    ] {
        let broker = FakeBroker {
            response: Mutex::new(Some(granted())),
            request: Mutex::new(None),
            mutate_after_decision: None,
        };
        let process = process(stdout, exit_code);
        assert!(
            approve_node_runtime(
                &broker,
                &process,
                request(path.clone()),
                Arc::new(AtomicBool::new(false))
            )
            .is_err()
        );
    }
}

#[test]
fn broker_window_replacement_is_rejected_before_spawn() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("node");
    std::fs::write(&path, b"original-node").unwrap();
    let broker = FakeBroker {
        response: Mutex::new(Some(granted())),
        request: Mutex::new(None),
        mutate_after_decision: Some(path.clone()),
    };
    let process = process("v20.11.1\n", 0);
    assert!(
        approve_node_runtime(
            &broker,
            &process,
            request(path),
            Arc::new(AtomicBool::new(false))
        )
        .is_err()
    );
    assert_eq!(process.calls.load(Ordering::SeqCst), 0);
}

#[test]
fn cancelled_identity_hash_has_no_broker_or_process_effect() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("node");
    std::fs::write(&path, b"node").unwrap();
    let broker = FakeBroker {
        response: Mutex::new(Some(granted())),
        request: Mutex::new(None),
        mutate_after_decision: None,
    };
    let process = process("v20.11.1\n", 0);
    let cancellation = Arc::new(AtomicBool::new(true));
    assert!(approve_node_runtime(&broker, &process, request(path), cancellation).is_err());
    assert!(broker.request.lock().unwrap().is_none());
    assert_eq!(process.calls.load(Ordering::SeqCst), 0);
}

#[test]
fn receipt_revalidation_rejects_context_mismatch() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("node");
    std::fs::write(&path, b"node").unwrap();
    let broker = FakeBroker {
        response: Mutex::new(Some(granted())),
        request: Mutex::new(None),
        mutate_after_decision: None,
    };
    let process = process("v20.11.1\n", 0);
    let runtime = approve_node_runtime(
        &broker,
        &process,
        request(path.clone()),
        Arc::new(AtomicBool::new(false)),
    )
    .unwrap();
    let mut mismatched = request(path);
    mismatched.principal_id = PrincipalId("other-principal".to_string());
    assert!(
        runtime
            .revalidate(
                &mismatched,
                LspNodeVersion {
                    major: 14,
                    minor: 0,
                    patch: 0
                },
                Arc::new(AtomicBool::new(false))
            )
            .is_err()
    );
}

#[test]
fn receipt_revalidation_rejects_required_minimum_above_observed_version() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("node");
    std::fs::write(&path, b"node").unwrap();
    let broker = FakeBroker {
        response: Mutex::new(Some(granted())),
        request: Mutex::new(None),
        mutate_after_decision: None,
    };
    let process = process("v20.11.1\n", 0);
    let runtime = approve_node_runtime(
        &broker,
        &process,
        request(path.clone()),
        Arc::new(AtomicBool::new(false)),
    )
    .unwrap();
    assert!(
        runtime
            .revalidate(
                &request(path),
                LspNodeVersion {
                    major: 21,
                    minor: 0,
                    patch: 0
                },
                Arc::new(AtomicBool::new(false))
            )
            .is_err()
    );
}

#[test]
fn receipt_revalidation_rejects_same_size_replacement() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("node");
    std::fs::write(&path, b"original-node").unwrap();
    let broker = FakeBroker {
        response: Mutex::new(Some(granted())),
        request: Mutex::new(None),
        mutate_after_decision: None,
    };
    let process = process("v20.11.1\n", 0);
    let runtime = approve_node_runtime(
        &broker,
        &process,
        request(path.clone()),
        Arc::new(AtomicBool::new(false)),
    )
    .unwrap();
    std::fs::write(&path, b"replaced-node").unwrap();
    assert!(
        runtime
            .revalidate(
                &request(path),
                LspNodeVersion {
                    major: 14,
                    minor: 0,
                    patch: 0
                },
                Arc::new(AtomicBool::new(false))
            )
            .is_err()
    );
}

#[test]
fn cancellation_after_probe_completion_does_not_mint_receipt() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("node");
    std::fs::write(&path, b"node").unwrap();
    let broker = FakeBroker {
        response: Mutex::new(Some(granted())),
        request: Mutex::new(None),
        mutate_after_decision: None,
    };
    let cancellation = Arc::new(AtomicBool::new(false));
    let mut process = process("v20.11.1\n", 0);
    process.cancel_before_return = Some(cancellation.clone());
    assert!(approve_node_runtime(&broker, &process, request(path), cancellation).is_err());
}

#[test]
#[ignore = "opt-in native Node fixture; set LEGION_TEST_NODE_RUNTIME to an absolute executable path"]
fn native_node_runtime_probe_uses_selected_executable_and_revalidates() {
    let path = PathBuf::from(std::env::var("LEGION_TEST_NODE_RUNTIME").expect(
        "LEGION_TEST_NODE_RUNTIME must name the explicitly selected absolute Node executable",
    ));
    assert!(
        path.is_absolute(),
        "LEGION_TEST_NODE_RUNTIME must be absolute"
    );
    let broker = FakeBroker {
        response: Mutex::new(Some(granted())),
        request: Mutex::new(None),
        mutate_after_decision: None,
    };
    let runtime = approve_node_runtime(
        &broker,
        &NativeProcessService,
        request(path.clone()),
        Arc::new(AtomicBool::new(false)),
    )
    .expect("selected Node executable should answer --version");
    assert!(
        runtime.observed_version()
            >= LspNodeVersion {
                major: 14,
                minor: 0,
                patch: 0
            }
    );
    runtime
        .revalidate(
            &request(path),
            LspNodeVersion {
                major: 14,
                minor: 0,
                patch: 0,
            },
            Arc::new(AtomicBool::new(false)),
        )
        .expect("receipt should revalidate before launch binding");
}
