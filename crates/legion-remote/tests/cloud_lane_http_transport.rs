//! Deterministic HTTP JSON cloud-lane transport integration tests.
//!
//! Uses a local `std::net::TcpListener` to avoid external network dependencies
//! and keep tests deterministic.

use std::io::{Read, Write};
use std::net::TcpListener;
use std::thread;
use std::time::Duration;

use legion_protocol::{
    AssistedAiProviderClass, CancellationTokenId, CapabilityDecision, CapabilityDecisionId,
    CapabilityId, CorrelationId, EventSequence, LegionCloudLaneBudget,
    LegionCloudLaneProposalResponse, LegionCloudLaneTaskEvent, LegionCloudLaneTaskId,
    LegionCloudLaneTaskRequest, LegionCloudLaneTaskState, LegionCloudLaneTaskStatus,
    LegionCloudLaneUploadManifest, LegionEvidenceKind, LegionEvidencePrivacyScope,
    LegionEvidenceRecord, LegionEvidenceSource, LegionModelCapability,
    LegionProviderLocalityPreference, LegionProviderPrivacyPolicy, LegionProviderRouteHealth,
    LegionProviderRouteMetadata, LegionTaskFileScope, LegionTaskOutputContract, LegionTaskPacket,
    LegionTaskPacketId, LegionTaskPolicy, LegionTaskValidationPlan, LegionWorkerResultKind,
    RedactionHint, TimestampMillis, WorkspaceId,
};
use legion_remote::{
    HttpLegionCloudLaneTransport, HttpLegionCloudLaneTransportConfig, LegionCloudLaneClient,
    LegionCloudLaneClientConfig, RemoteRuntimeError,
};
use uuid::Uuid;

fn task_id() -> LegionCloudLaneTaskId {
    LegionCloudLaneTaskId("cloud-task:http:1".to_string())
}

fn base_request() -> LegionCloudLaneTaskRequest {
    LegionCloudLaneTaskRequest {
        task_id: task_id(),
        lane_id: "cloud-lane:http-test".to_string(),
        control_plane_endpoint_id: "endpoint:http-test".to_string(),
        task_packet: LegionTaskPacket {
            packet_id: LegionTaskPacketId("packet:http:1".to_string()),
            workspace_id: WorkspaceId(1),
            objective_summary_hash: legion_protocol::FileFingerprint {
                algorithm: "sha256".to_string(),
                value: "objective".to_string(),
            },
            allowed_files: vec![LegionTaskFileScope {
                scope_id: "allowed:src".to_string(),
                path: legion_protocol::CanonicalPath("/workspace/src/lib.rs".to_string()),
                fingerprint: None,
                redaction_hints: vec![RedactionHint::MetadataOnly],
                schema_version: 1,
            }],
            forbidden_files: vec![],
            context_snippet_refs: vec![],
            full_file_refs: vec![],
            command_output_refs: vec![],
            output_contract: LegionTaskOutputContract {
                expected_result_kind: LegionWorkerResultKind::PatchProposal,
                proposal_only: true,
                direct_mutation_allowed: false,
                required_evidence_kinds: vec![],
                redaction_hints: vec![RedactionHint::MetadataOnly],
                schema_version: 1,
            },
            validation_plan: LegionTaskValidationPlan {
                required_commands: vec![],
                success_criteria: vec![],
                stop_conditions: vec![],
                redaction_hints: vec![RedactionHint::MetadataOnly],
                schema_version: 1,
            },
            policy: LegionTaskPolicy {
                locality_preference: LegionProviderLocalityPreference::RemoteAllowed,
                privacy_policy: LegionProviderPrivacyPolicy::MetadataOnly,
                cost_budget_cents: Some(75),
                latency_budget_ms: Some(30_000),
                allow_network: true,
                allow_direct_workspace_mutation: false,
                redaction_hints: vec![RedactionHint::MetadataOnly],
                schema_version: 1,
            },
            correlation_id: CorrelationId(901),
            causality_id: legion_protocol::CausalityId(Uuid::from_u128(1)),
            redaction_hints: vec![RedactionHint::MetadataOnly],
            schema_version: 1,
        },
        upload_manifest: LegionCloudLaneUploadManifest {
            manifest_id: "upload:http:1".to_string(),
            allowed_files: vec![LegionTaskFileScope {
                scope_id: "allowed:src".to_string(),
                path: legion_protocol::CanonicalPath("/workspace/src/lib.rs".to_string()),
                fingerprint: None,
                redaction_hints: vec![RedactionHint::MetadataOnly],
                schema_version: 1,
            }],
            forbidden_files: vec![],
            total_upload_bytes: 1024,
            scope_visible_to_user: true,
            contains_forbidden_material: false,
            secret_scan_status: legion_protocol::LegionCloudLaneSecretScanStatus::Passed,
            redaction_hints: vec![RedactionHint::MetadataOnly],
            schema_version: 1,
        },
        budget: LegionCloudLaneBudget {
            max_cost_cents: 75,
            estimated_cost_cents: 50,
            max_queue_depth: 2,
            current_queue_depth: 1,
            usage_metering_label: "meter:http:test".to_string(),
            hard_cap_enforced: true,
            redaction_hints: vec![RedactionHint::MetadataOnly],
            schema_version: 1,
        },
        capability_decision: CapabilityDecision {
            decision_id: CapabilityDecisionId(1),
            granted: true,
            capability: CapabilityId("cloud.lane.submit".to_string()),
            reason: Some("test".to_string()),
        },
        cancellation_token: CancellationTokenId(
            Uuid::parse_str("aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa").unwrap(),
        ),
        correlation_id: CorrelationId(901),
        causality_id: legion_protocol::CausalityId(Uuid::from_u128(1)),
        redaction_hints: vec![RedactionHint::MetadataOnly],
        schema_version: 1,
    }
}

fn serve_one<F>(handler: F) -> String
where
    F: FnOnce(&str) -> String + Send + 'static,
{
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind listener");
    let port = listener.local_addr().unwrap().port();
    thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("accept connection");
        stream
            .set_read_timeout(Some(Duration::from_secs(5)))
            .expect("set read timeout");
        let request = read_full_http_request(&mut stream);
        let response = handler(&request);
        stream
            .write_all(response.as_bytes())
            .expect("write response");
        let _ = stream.flush();
    });
    format!("http://127.0.0.1:{port}")
}

/// Reads a complete HTTP request (headers plus Content-Length-delimited body)
/// rather than a single fixed-size read, so segmented or larger-than-buffer
/// requests are not truncated before assertions.
fn read_full_http_request(stream: &mut std::net::TcpStream) -> String {
    let mut buffer = Vec::new();
    let mut scratch = [0u8; 1024];
    let header_end = loop {
        let read = stream.read(&mut scratch).expect("read request bytes");
        assert!(read > 0, "client closed connection before sending request");
        buffer.extend_from_slice(&scratch[..read]);
        if let Some(position) = buffer.windows(4).position(|window| window == b"\r\n\r\n") {
            break position + 4;
        }
    };
    let header_text = String::from_utf8_lossy(&buffer[..header_end]).to_string();
    let content_length = header_text
        .lines()
        .find_map(|line| {
            let lower = line.to_ascii_lowercase();
            if lower.starts_with("content-length:") {
                line.split_once(':')
                    .and_then(|(_, value)| value.trim().parse::<usize>().ok())
            } else {
                None
            }
        })
        .unwrap_or(0);
    while buffer.len() < header_end + content_length {
        let read = stream.read(&mut scratch).expect("read request body");
        assert!(read > 0, "client closed connection before sending body");
        buffer.extend_from_slice(&scratch[..read]);
    }
    String::from_utf8_lossy(&buffer).to_string()
}

/// Base URL for tests that must reject before any network I/O. Points at a
/// reserved, unroutable address so an accidental request fails fast instead of
/// silently succeeding.
fn unreachable_base_url() -> String {
    "http://127.0.0.1:1".to_string()
}

fn http_ok_json(body: &str) -> String {
    format!(
        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}",
        body.len(),
        body
    )
}

fn http_error(status: &str, body: &str) -> String {
    format!(
        "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}",
        body.len(),
        body
    )
}

fn transport_config(base_url: &str) -> HttpLegionCloudLaneTransportConfig {
    HttpLegionCloudLaneTransportConfig {
        base_url: base_url.to_string(),
        timeout: Duration::from_secs(5),
        client_identity_label: "http-test-client".to_string(),
        auth_token: Some(("Bearer".to_string(), "test-token-123".to_string())),
    }
}

fn enabled_client_config() -> LegionCloudLaneClientConfig {
    LegionCloudLaneClientConfig::enabled(75, 32 * 1024)
}

fn default_status() -> LegionCloudLaneTaskStatus {
    LegionCloudLaneTaskStatus {
        task_id: task_id(),
        state: LegionCloudLaneTaskState::Submitted,
        status_label: "submitted".to_string(),
        estimated_cost_cents: 50,
        billed_cost_cents: 0,
        queue_position: Some(1),
        event_sequence: EventSequence(1),
        generated_at: TimestampMillis(1700),
        redaction_hints: vec![RedactionHint::MetadataOnly],
        schema_version: 1,
    }
}

fn default_event() -> LegionCloudLaneTaskEvent {
    LegionCloudLaneTaskEvent {
        task_id: task_id(),
        event_id: "event:queued".to_string(),
        state: LegionCloudLaneTaskState::Queued,
        event_label: "queued".to_string(),
        event_sequence: EventSequence(2),
        generated_at: TimestampMillis(1710),
        redaction_hints: vec![RedactionHint::MetadataOnly],
        schema_version: 1,
    }
}

fn default_proposal() -> LegionCloudLaneProposalResponse {
    LegionCloudLaneProposalResponse {
        task_id: task_id(),
        proposal_id: Some(legion_protocol::ProposalId(9001)),
        worker_result: Some(legion_protocol::LegionWorkerResult {
            result_id: "worker-result:http:1".to_string(),
            packet_id: LegionTaskPacketId("packet:http:1".to_string()),
            result_kind: LegionWorkerResultKind::PatchProposal,
            patch_proposal: Some(legion_protocol::ProposalId(9001)),
            documentation_proposal: None,
            analysis_summary: Some("analysis".to_string()),
            test_plan_summary: Some("tests".to_string()),
            blocked_reason: None,
            invalid_reason: None,
            evidence_records: vec![],
            provider_route: Some(LegionProviderRouteMetadata {
                route_id: "route:http:1".to_string(),
                locality_preference: LegionProviderLocalityPreference::RemoteAllowed,
                cost_budget_cents: Some(75),
                latency_budget_ms: Some(30_000),
                privacy_policy: LegionProviderPrivacyPolicy::MetadataOnly,
                model_capability: LegionModelCapability::CodePatch,
                provider_class: AssistedAiProviderClass::HostedRemote,
                route_health: LegionProviderRouteHealth::Healthy,
                labels: vec![],
                redaction_hints: vec![RedactionHint::MetadataOnly],
                schema_version: 1,
            }),
            correlation_id: CorrelationId(901),
            causality_id: legion_protocol::CausalityId(Uuid::from_u128(1)),
            redaction_hints: vec![RedactionHint::MetadataOnly],
            schema_version: 1,
        }),
        redaction_hints: vec![RedactionHint::MetadataOnly],
        schema_version: 1,
    }
}

fn default_evidence() -> LegionEvidenceRecord {
    LegionEvidenceRecord {
        evidence_id: "evidence:http:1".to_string(),
        kind: LegionEvidenceKind::CommandRun,
        source: LegionEvidenceSource::ProviderMetadata,
        payload_hash: legion_protocol::FileFingerprint {
            algorithm: "sha256".to_string(),
            value: "evidence".to_string(),
        },
        redacted_payload_summary: "evidence metadata".to_string(),
        command_label: Some("cargo test".to_string()),
        exit_status: Some(0),
        privacy_scope: LegionEvidencePrivacyScope::WorkspaceMetadata,
        generated_at: TimestampMillis(1730),
        redaction_hints: vec![RedactionHint::MetadataOnly],
        schema_version: 1,
    }
}

#[test]
fn http_transport_submit_task_sends_headers_and_body() {
    let base_url = serve_one(|request| {
        assert!(request.starts_with("POST /v1/cloud/tasks HTTP/1.1"));
        assert!(
            request
                .to_lowercase()
                .contains("authorization: bearer test-token-123")
        );
        assert!(
            request
                .to_lowercase()
                .contains("x-legion-client-identity: http-test-client")
        );
        assert!(
            request
                .to_lowercase()
                .contains("content-type: application/json")
        );
        assert!(request.contains("\"task_id\":\"cloud-task:http:1\""));
        http_ok_json(&serde_json::to_string(&default_status()).unwrap())
    });

    let transport = HttpLegionCloudLaneTransport::new(transport_config(&base_url)).unwrap();
    let mut client = LegionCloudLaneClient::new(transport, enabled_client_config());
    let status = client
        .submit_task(base_request())
        .expect("submit should succeed");
    assert_eq!(status.state, LegionCloudLaneTaskState::Submitted);
}

#[test]
fn http_transport_disabled_policy_rejects_before_network() {
    // No listener is spawned: an unreachable base URL guarantees any accidental
    // network attempt fails loudly instead of leaking a blocked accept() thread.
    let base_url = unreachable_base_url();

    let transport = HttpLegionCloudLaneTransport::new(transport_config(&base_url)).unwrap();
    let mut client = LegionCloudLaneClient::new(
        transport,
        LegionCloudLaneClientConfig {
            runtime_enabled: false,
            max_cost_cents: 75,
            max_upload_bytes: 32 * 1024,
        },
    );

    let err = client
        .submit_task(base_request())
        .expect_err("disabled policy must reject");
    assert!(matches!(err, RemoteRuntimeError::RuntimeDisabled));
}

#[test]
fn http_transport_forbidden_upload_rejects_before_network() {
    // No listener is spawned: an unreachable base URL guarantees any accidental
    // network attempt fails loudly instead of leaking a blocked accept() thread.
    let base_url = unreachable_base_url();

    let transport = HttpLegionCloudLaneTransport::new(transport_config(&base_url)).unwrap();
    let mut client = LegionCloudLaneClient::new(transport, enabled_client_config());
    let mut request = base_request();
    request.upload_manifest.contains_forbidden_material = true;

    let err = client
        .submit_task(request)
        .expect_err("forbidden upload must reject");
    assert!(err.to_string().contains("forbidden") || err.to_string().contains("upload"));
}

#[test]
fn http_transport_cost_cap_rejects_before_network() {
    // No listener is spawned: an unreachable base URL guarantees any accidental
    // network attempt fails loudly instead of leaking a blocked accept() thread.
    let base_url = unreachable_base_url();

    let transport = HttpLegionCloudLaneTransport::new(transport_config(&base_url)).unwrap();
    let mut client = LegionCloudLaneClient::new(
        transport,
        LegionCloudLaneClientConfig::enabled(10, 32 * 1024),
    );
    let mut request = base_request();
    request.budget.estimated_cost_cents = 50;

    let err = client
        .submit_task(request)
        .expect_err("cost cap must reject");
    assert!(err.to_string().contains("cost cap") || err.to_string().contains("budget"));
}

#[test]
fn http_transport_stream_task_events() {
    let base_url = serve_one(|request| {
        assert!(request.starts_with("GET /v1/cloud/tasks/cloud-task:http:1/events HTTP/1.1"));
        assert!(
            request
                .to_lowercase()
                .contains("authorization: bearer test-token-123")
        );
        http_ok_json(&serde_json::to_string(&vec![default_event()]).unwrap())
    });

    let transport = HttpLegionCloudLaneTransport::new(transport_config(&base_url)).unwrap();
    let mut client = LegionCloudLaneClient::new(transport, enabled_client_config());
    let events = client
        .stream_task_events(&task_id())
        .expect("stream events should succeed");
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].state, LegionCloudLaneTaskState::Queued);
}

#[test]
fn http_transport_cancel_task() {
    let base_url = serve_one(|request| {
        assert!(request.starts_with("POST /v1/cloud/tasks/cloud-task:http:1/cancel HTTP/1.1"));
        assert!(request.contains("\"cancellation_token\":"));
        assert!(request.contains("\"reason_label\":\"user-cancel\""));
        let mut status = default_status();
        status.state = LegionCloudLaneTaskState::Cancelled;
        status.status_label = "user-cancel".to_string();
        http_ok_json(&serde_json::to_string(&status).unwrap())
    });

    let transport = HttpLegionCloudLaneTransport::new(transport_config(&base_url)).unwrap();
    let mut client = LegionCloudLaneClient::new(transport, enabled_client_config());
    let status = client
        .cancel_task(
            &task_id(),
            CancellationTokenId(Uuid::parse_str("aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa").unwrap()),
            "user-cancel",
        )
        .expect("cancel should succeed");
    assert_eq!(status.state, LegionCloudLaneTaskState::Cancelled);
}

#[test]
fn http_transport_fetch_task_proposal() {
    let base_url = serve_one(|request| {
        assert!(request.starts_with("GET /v1/cloud/tasks/cloud-task:http:1/proposal HTTP/1.1"));
        http_ok_json(&serde_json::to_string(&default_proposal()).unwrap())
    });

    let transport = HttpLegionCloudLaneTransport::new(transport_config(&base_url)).unwrap();
    let mut client = LegionCloudLaneClient::new(transport, enabled_client_config());
    let proposal = client
        .fetch_task_proposal(&task_id())
        .expect("fetch proposal should succeed");
    assert_eq!(
        proposal.proposal_id,
        Some(legion_protocol::ProposalId(9001))
    );
}

#[test]
fn http_transport_fetch_task_evidence() {
    let base_url = serve_one(|request| {
        assert!(request.starts_with("GET /v1/cloud/tasks/cloud-task:http:1/evidence HTTP/1.1"));
        http_ok_json(&serde_json::to_string(&vec![default_evidence()]).unwrap())
    });

    let transport = HttpLegionCloudLaneTransport::new(transport_config(&base_url)).unwrap();
    let mut client = LegionCloudLaneClient::new(transport, enabled_client_config());
    let evidence = client
        .fetch_task_evidence(&task_id())
        .expect("fetch evidence should succeed");
    assert_eq!(evidence.len(), 1);
    assert_eq!(evidence[0].kind, LegionEvidenceKind::CommandRun);
}

#[test]
fn http_transport_classifies_4xx_as_http_response_error() {
    let base_url = serve_one(|_| http_error("404 Not Found", "{\"error\":\"task not found\"}"));

    let transport = HttpLegionCloudLaneTransport::new(transport_config(&base_url)).unwrap();
    let mut client = LegionCloudLaneClient::new(transport, enabled_client_config());
    let err = client
        .fetch_task_proposal(&task_id())
        .expect_err("404 should fail");
    assert!(matches!(
        err,
        RemoteRuntimeError::HttpResponse { status: 404, .. }
    ));
    assert!(err.to_string().contains("task not found"));
}

#[test]
fn http_transport_classifies_5xx_as_http_response_error() {
    let base_url =
        serve_one(|_| http_error("503 Service Unavailable", "{\"error\":\"overloaded\"}"));

    let transport = HttpLegionCloudLaneTransport::new(transport_config(&base_url)).unwrap();
    let mut client = LegionCloudLaneClient::new(transport, enabled_client_config());
    let err = client
        .fetch_task_proposal(&task_id())
        .expect_err("503 should fail");
    assert!(matches!(
        err,
        RemoteRuntimeError::HttpResponse { status: 503, .. }
    ));
    assert!(err.to_string().contains("overloaded"));
}

#[test]
fn http_transport_config_debug_redacts_auth_token() {
    let config = transport_config("http://example.invalid");
    let debug = format!("{config:?}");
    assert!(!debug.contains("test-token-123"));
    assert!(debug.contains("<redacted>"));
    assert!(debug.contains("http-test-client"));
}
