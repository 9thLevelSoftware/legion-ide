#![cfg(feature = "ai")]
use legion_app::{AppComposition, named_mcp_peer::NamedMcpPeerConfig};
use legion_protocol::{McpServerId, named_mcp_peer::*};

#[test]
fn changing_endpoint_or_scopes_cannot_send_a_previous_binding_bearer() {
    use legion_app::{
        AppProductMode,
        named_mcp_peer::{
            NamedMcpPeerError, NamedMcpPeerPermissions, named_mcp_peer_secret_reference,
        },
    };
    use legion_storage::secrets::{InMemorySecretStore, SecretStore};
    let a = ProtocolPeer::start("2025-11-25");
    let b = ProtocolPeer::start("2025-11-25");
    let id = McpServerId("peer:credential-binding".into());
    let mut config_a = NamedMcpPeerConfig::http_client(id.clone(), "Bound peer", &a.endpoint);
    config_a.metadata.authentication = McpPeerAuthentication::Bearer;
    config_a.metadata.credential_scopes = vec!["tools:read".into()];
    let mut config_b = config_a.clone();
    config_b.transport = legion_app::named_mcp_peer::NamedMcpPeerTransport::Http {
        endpoint: b.endpoint.clone(),
    };
    let secrets = InMemorySecretStore::default();
    secrets
        .store(
            &named_mcp_peer_secret_reference(&config_a),
            "synthetic-A-token",
        )
        .unwrap();
    let mut app = AppComposition::new();
    app.set_product_mode(AppProductMode::Automate);
    let state_a = app.configure_named_mcp_peer(config_a.clone()).unwrap();
    app.grant_named_mcp_peer_transport(&id, state_a.revision, NamedMcpPeerPermissions::network())
        .unwrap();
    app.activate_named_mcp_peer(&id, state_a.revision, &secrets)
        .unwrap();
    let state_b = app.configure_named_mcp_peer(config_b.clone()).unwrap();
    app.grant_named_mcp_peer_transport(&id, state_b.revision, NamedMcpPeerPermissions::network())
        .unwrap();
    assert_eq!(
        app.activate_named_mcp_peer(&id, state_b.revision, &secrets),
        Err(NamedMcpPeerError::CredentialUnavailable)
    );
    assert!(
        b.transcript().is_empty(),
        "endpoint B must receive no request carrying endpoint A's credential"
    );
    secrets
        .store(
            &named_mcp_peer_secret_reference(&config_b),
            "synthetic-B-token",
        )
        .unwrap();
    app.grant_named_mcp_peer_transport(&id, state_b.revision, NamedMcpPeerPermissions::network())
        .unwrap();
    app.activate_named_mcp_peer(&id, state_b.revision, &secrets)
        .unwrap();
    assert!(
        a.transcript()
            .iter()
            .all(|(auth, _, _)| auth == "Bearer synthetic-A-token")
    );
    assert!(
        b.transcript()
            .iter()
            .all(|(auth, _, _)| auth == "Bearer synthetic-B-token")
    );
    config_b.metadata.credential_scopes = vec!["tools:write".into()];
    let new_scope = app.configure_named_mcp_peer(config_b).unwrap();
    app.grant_named_mcp_peer_transport(&id, new_scope.revision, NamedMcpPeerPermissions::network())
        .unwrap();
    assert_eq!(
        app.activate_named_mcp_peer(&id, new_scope.revision, &secrets),
        Err(NamedMcpPeerError::CredentialUnavailable)
    );
    assert_eq!(b.transcript().len(), 4);
}

#[test]
fn secret_deletion_failure_is_redacted_and_cannot_restore_a_revoked_runtime() {
    use legion_app::{
        AppProductMode,
        named_mcp_peer::{NamedMcpPeerError, NamedMcpPeerPermissions},
    };
    use legion_storage::secrets::{SecretReference, SecretStore, SecretStoreError};
    struct FailingDelete;
    impl SecretStore for FailingDelete {
        fn store(&self, _: &SecretReference, _: &str) -> Result<(), SecretStoreError> {
            Ok(())
        }
        fn load(&self, _: &SecretReference) -> Result<Option<String>, SecretStoreError> {
            Ok(Some("synthetic-secret".into()))
        }
        fn delete(&self, _: &SecretReference) -> Result<(), SecretStoreError> {
            Err(SecretStoreError::KeyringFailure {
                message: "synthetic-secret raw-store-error".into(),
            })
        }
    }
    struct BypassRuntime;
    impl legion_app::AppAutomateMcpToolRuntime for BypassRuntime {
        fn call_tool(
            &self,
            _: &legion_app::AppAutomateMcpToolInvocation,
        ) -> Result<
            legion_app::AppAutomateMcpToolInvocationReceipt,
            legion_app::AppAutomateMcpToolRuntimeError,
        > {
            panic!("revoked runtime must not be invoked")
        }
    }
    let peer = ProtocolPeer::start("2025-11-25");
    let id = McpServerId("peer:failed-delete".into());
    let config = NamedMcpPeerConfig::http_client(id.clone(), "Failing deletion", &peer.endpoint);
    let mut app = AppComposition::new();
    app.set_product_mode(AppProductMode::Automate);
    let state = app.configure_named_mcp_peer(config).unwrap();
    app.grant_named_mcp_peer_transport(&id, state.revision, NamedMcpPeerPermissions::network())
        .unwrap();
    app.activate_named_mcp_peer(&id, state.revision, &FailingDelete)
        .unwrap();
    let error = app
        .revoke_named_mcp_peer_credential(&id, &FailingDelete)
        .unwrap_err();
    assert_eq!(error, NamedMcpPeerError::CredentialDeletionFailed);
    let revoked = app.inspect_named_mcp_peer(&id).unwrap();
    assert_eq!(revoked.health, McpPeerHealth::Revoked);
    assert_eq!(
        revoked.credential_state,
        McpPeerCredentialState::DeletionFailed
    );
    assert_eq!(revoked.metadata.privacy, McpPeerPrivacy::MetadataOnly);
    assert_eq!(
        app.probe_named_mcp_peer(&id, revoked.revision),
        Err(NamedMcpPeerError::Revoked)
    );
    assert!(
        app.register_legion_workflow_mcp_tool_runtime(
            id.clone(),
            std::sync::Arc::new(BypassRuntime)
        )
        .is_err()
    );
    let retained = format!("{} {}", error, serde_json::to_string(&revoked).unwrap());
    assert!(!retained.contains("synthetic-secret"));
    assert!(!retained.contains("raw-store-error"));
    assert_eq!(peer.transcript().len(), 4);
}

#[test]
fn grant_and_credential_revocation_disable_peer_and_invalidate_old_revision() {
    use legion_app::{
        AppProductMode,
        named_mcp_peer::{
            NamedMcpPeerError, NamedMcpPeerPermissions, named_mcp_peer_secret_reference,
        },
    };
    use legion_storage::secrets::{InMemorySecretStore, SecretStore};
    let peer = ProtocolPeer::start("2025-11-25");
    let id = McpServerId("peer:revoke".into());
    let mut config = NamedMcpPeerConfig::http_client(id.clone(), "Revocable", &peer.endpoint);
    config.metadata.authentication = McpPeerAuthentication::Bearer;
    config.metadata.credential_scopes = vec!["tools:read".into()];
    let secrets = InMemorySecretStore::default();
    let reference = named_mcp_peer_secret_reference(&config);
    secrets
        .store(&reference, "synthetic-revocable-token")
        .unwrap();
    let mut app = AppComposition::new();
    app.set_product_mode(AppProductMode::Automate);
    let config_state = app.configure_named_mcp_peer(config).unwrap();
    app.grant_named_mcp_peer_transport(
        &id,
        config_state.revision,
        NamedMcpPeerPermissions::network(),
    )
    .unwrap();
    app.activate_named_mcp_peer(&id, config_state.revision, &secrets)
        .unwrap();
    let revoked = app.revoke_named_mcp_peer_grant(&id).unwrap();
    assert_eq!(revoked.health, McpPeerHealth::Revoked);
    assert!(!revoked.transport_granted);
    assert_eq!(
        secrets.load(&reference).unwrap().as_deref(),
        Some("synthetic-revocable-token")
    );
    assert_eq!(
        app.probe_named_mcp_peer(&id, config_state.revision),
        Err(NamedMcpPeerError::StaleRevision)
    );
    assert_eq!(
        app.probe_named_mcp_peer(&id, revoked.revision),
        Err(NamedMcpPeerError::Revoked)
    );
    let deleted = app.revoke_named_mcp_peer_credential(&id, &secrets).unwrap();
    assert_eq!(deleted.credential_state, McpPeerCredentialState::Deleted);
    assert_eq!(secrets.load(&reference).unwrap(), None);
    app.grant_named_mcp_peer_transport(&id, deleted.revision, NamedMcpPeerPermissions::network())
        .unwrap();
    assert_eq!(
        app.activate_named_mcp_peer(&id, deleted.revision, &secrets),
        Err(NamedMcpPeerError::CredentialUnavailable)
    );
    assert_eq!(peer.transcript().len(), 4);
}

struct ProtocolPeer {
    endpoint: String,
    stop: std::sync::Arc<std::sync::atomic::AtomicBool>,
    exchanges: std::sync::Arc<std::sync::Mutex<Vec<(String, String, serde_json::Value)>>>,
    worker: Option<std::thread::JoinHandle<()>>,
}

impl ProtocolPeer {
    fn start(version: &str) -> Self {
        Self::with_behavior(version, false, false)
    }

    fn with_behavior(version: &str, wrong_ping_id: bool, redirect: bool) -> Self {
        use std::{
            io::{BufRead, Read, Write},
            sync::{
                Arc, Mutex,
                atomic::{AtomicBool, Ordering},
            },
            time::{Duration, Instant},
        };
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let endpoint = format!("http://{}/rpc", listener.local_addr().unwrap());
        let stop = Arc::new(AtomicBool::new(false));
        let exchanges = Arc::new(Mutex::new(vec![]));
        let exit = stop.clone();
        let rows = exchanges.clone();
        let version = version.to_owned();
        let worker = std::thread::spawn(move || {
            let deadline = Instant::now() + Duration::from_secs(20);
            while !exit.load(Ordering::SeqCst) && Instant::now() < deadline {
                let (mut stream, _) = match listener.accept() {
                    Ok(pair) => pair,
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        std::thread::sleep(Duration::from_millis(2));
                        continue;
                    }
                    Err(error) => panic!("peer accept: {error}"),
                };
                stream
                    .set_read_timeout(Some(Duration::from_secs(5)))
                    .unwrap();
                let mut reader = std::io::BufReader::new(stream.try_clone().unwrap());
                let mut line = String::new();
                reader.read_line(&mut line).unwrap();
                assert_eq!(line.trim(), "POST /rpc HTTP/1.1");
                let mut length = 0;
                let mut auth = String::new();
                let mut protocol = String::new();
                loop {
                    line.clear();
                    reader.read_line(&mut line).unwrap();
                    if line == "\r\n" {
                        break;
                    }
                    let (name, value) = line.split_once(':').unwrap();
                    match name.to_ascii_lowercase().as_str() {
                        "content-length" => length = value.trim().parse::<usize>().unwrap(),
                        "authorization" => auth = value.trim().to_string(),
                        "mcp-protocol-version" => protocol = value.trim().to_string(),
                        _ => {}
                    }
                }
                let mut body = vec![0; length];
                reader.read_exact(&mut body).unwrap();
                let request: serde_json::Value = serde_json::from_slice(&body).unwrap();
                if request["method"] == "tools/call" {
                    // A selected endpoint drops the connection during invocation:
                    // reqwest's raw error includes its URL/path unless the named
                    // transport boundary redacts it before workflow publication.
                    rows.lock().unwrap().push((auth, protocol, request));
                    continue;
                }
                let result = match request["method"].as_str().unwrap() {
                    "initialize" => {
                        serde_json::json!({"protocolVersion":version, "capabilities":{"tools":{}}, "serverInfo":{"name":"reference-peer","version":"1"}})
                    }
                    "tools/list" => {
                        serde_json::json!({"tools":[{"name":"inspect", "description":"UNTRUSTED RAW DESCRIPTION", "inputSchema":{"type":"object"}}]})
                    }
                    "ping" => serde_json::json!({}),
                    "notifications/initialized" => serde_json::Value::Null,
                    method => panic!("unexpected peer effect: {method}"),
                };
                rows.lock().unwrap().push((auth, protocol, request.clone()));
                if redirect {
                    write!(stream, "HTTP/1.1 302 Found\r\nLocation: http://127.0.0.1:1/forbidden\r\nConnection: close\r\nContent-Length: 0\r\n\r\n").unwrap();
                } else if request["id"].is_null() {
                    write!(
                        stream,
                        "HTTP/1.1 204 No Content\r\nConnection: close\r\nContent-Length: 0\r\n\r\n"
                    )
                    .unwrap();
                } else {
                    let id = if wrong_ping_id && request["method"] == "ping" {
                        serde_json::json!("wrong-request")
                    } else {
                        request["id"].clone()
                    };
                    let response =
                        serde_json::json!({"jsonrpc":"2.0", "id":id, "result":result}).to_string();
                    write!(stream, "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nConnection: close\r\nContent-Length: {}\r\n\r\n{}", response.len(), response).unwrap();
                }
            }
        });
        Self {
            endpoint,
            stop,
            exchanges,
            worker: Some(worker),
        }
    }

    fn transcript(&self) -> Vec<(String, String, serde_json::Value)> {
        self.exchanges.lock().unwrap().clone()
    }
}

#[test]
fn named_tool_transport_failure_retains_only_redacted_workflow_metadata() {
    use legion_app::{AppProductMode, named_mcp_peer::NamedMcpPeerPermissions};
    use legion_protocol::*;
    let peer = ProtocolPeer::start("2025-11-25");
    let id = McpServerId("peer:tool-fault".into());
    let mut app = AppComposition::new();
    app.set_product_mode(AppProductMode::Automate);
    let configured = app
        .configure_named_mcp_peer(NamedMcpPeerConfig::http_client(
            id.clone(),
            "Tool fault",
            &peer.endpoint,
        ))
        .unwrap();
    app.grant_named_mcp_peer_transport(
        &id,
        configured.revision,
        NamedMcpPeerPermissions::network(),
    )
    .unwrap();
    app.activate_named_mcp_peer(
        &id,
        configured.revision,
        &legion_storage::InMemorySecretStore::default(),
    )
    .unwrap();
    let session_id = LegionWorkflowSessionId("session:named-tool-fault".into());
    let target_id = format!("mcp-tool:{}|inspect", id.0);
    let causality = CausalityId(uuid::Uuid::from_u128(63));
    let worker = LegionWorkflowWorkerAssignment {
        worker_id: LegionWorkflowWorkerId("worker:named-tool".into()),
        role: LegionWorkflowWorkerRole::Implementer,
        state: LegionWorkflowWorkerState::Ready,
        model_backend: LegionWorkflowModelBackend::Unavailable,
        display_safe_model_label: "No inference provider".into(),
        allowed_command_classes: vec![DelegatedTaskOperationClass::DraftProposalMetadata],
        linked_delegated_plan_id: None,
        assisted_ai_route: None,
        affected_targets: vec![DelegatedTaskAffectedTargetSummary {
            target_id: target_id.clone(),
            kind: ProposalTargetKind::MetadataOnly,
            workspace_id: Some(WorkspaceId(1)),
            file_id: None,
            buffer_id: None,
            ranges: vec![ByteRange::new(0, 0)],
            hashes: vec![FileFingerprint {
                algorithm: "sha256".into(),
                value: "fixture-target".into(),
            }],
            counts: vec![],
            labels: vec!["MCP metadata".into()],
            risk_label: ProposalRiskLabel::Low,
            privacy_label: ProposalPrivacyLabel::WorkspaceMetadata,
            redaction_hints: vec![RedactionHint::MetadataOnly],
            schema_version: 1,
        }],
        risk_labels: vec![CommandRiskLabel::Review],
        privacy_labels: vec![PrivacyClassification::Metadata],
        correlation_id: CorrelationId(63),
        causality_id: causality.clone(),
        redaction_hints: vec![RedactionHint::MetadataOnly],
        schema_version: 1,
    };
    app.seed_legion_workflow_sessions(vec![LegionWorkflowSession {
        session_id: session_id.clone(),
        directive_artifact_id: Some("directive:fixture".into()),
        spec_artifact_id: Some("spec:fixture".into()),
        task_graph_artifact_id: Some("graph:fixture".into()),
        product_mode: ProductMode::LegionWorkflows,
        worker_assignments: vec![worker],
        dependency_edges: vec![],
        conflict_summaries: vec![],
        verification_gates: vec![LegionWorkflowVerificationGate {
            gate_id: LegionWorkflowVerificationGateId("check:fixture".into()),
            state: LegionWorkflowVerificationGateState::Passed,
            label: "Fixture check".into(),
            evidence_artifact_id: Some("evidence:fixture".into()),
            command_class_label: "metadata".into(),
            redaction_hints: vec![RedactionHint::MetadataOnly],
            schema_version: 1,
        }],
        sign_off_records: vec![LegionWorkflowSignOff {
            sign_off_id: LegionWorkflowSignOffId("signoff:fixture".into()),
            state: LegionWorkflowSignOffState::SignedOff,
            required_role: LegionWorkflowWorkerRole::Reviewer,
            reviewer_principal_id: Some(PrincipalId("principal:fixture".into())),
            label: "Fixture signoff".into(),
            redaction_hints: vec![RedactionHint::MetadataOnly],
            schema_version: 1,
        }],
        proposal_ids: vec![],
        merge_approval: Some(LegionWorkflowMergeApproval {
            approval_artifact_id: Some("approval:fixture".into()),
            approval_granted: true,
            rollback_available: true,
            audit_persisted_before_success: true,
            main_workspace_dirty_conflict: false,
            proposal_preconditions_stale: false,
            labels: vec!["fixture".into()],
            redaction_hints: vec![RedactionHint::MetadataOnly],
            schema_version: 1,
        }),
        lifecycle_state: LegionWorkflowState::Executing,
        generated_at: TimestampMillis(1),
        redaction_hints: vec![RedactionHint::MetadataOnly],
        schema_version: 1,
        correlation_id: CorrelationId(63),
        causality_id: causality,
    }])
    .unwrap();
    app.execute_legion_workflow(&session_id).unwrap();
    assert_eq!(
        peer.transcript().len(),
        4,
        "network activation must not grant tool execution"
    );
    app.record_legion_workflow_tool_permission_decision(
        &session_id,
        &id,
        &McpToolName("inspect".into()),
        DelegatedTaskToolPermissionDecision::Allow,
    )
    .unwrap();
    let failed = app.execute_legion_workflow(&session_id).unwrap();
    assert!(
        failed
            .projection
            .decision_feed
            .iter()
            .any(|entry| entry.kind == LegionWorkflowDecisionKind::ToolCallFailed)
    );
    let retained = format!(
        "{} {:?}",
        serde_json::to_string(&failed.projection).unwrap(),
        failed.outputs
    );
    assert!(retained.contains("mcp_worker_tool_call_failed"));
    assert!(
        !retained.contains(&peer.endpoint),
        "raw endpoint reached workflow failure metadata"
    );
    assert!(
        !retained.contains("/rpc"),
        "raw endpoint path reached workflow failure metadata"
    );
    assert!(
        peer.transcript()
            .iter()
            .any(|(_, _, request)| request["method"] == "tools/call")
    );
}

#[test]
fn negotiation_mismatch_malformed_health_and_redirect_disable_runtime_without_fallback() {
    use legion_app::{
        AppProductMode,
        named_mcp_peer::{NamedMcpPeerError, NamedMcpPeerPermissions},
    };
    use legion_storage::secrets::InMemorySecretStore;
    for (version, wrong_id, redirect, expected, exchanges) in [
        (
            "2024-11-05",
            false,
            false,
            NamedMcpPeerError::ProtocolMismatch,
            1,
        ),
        (
            "2025-11-25",
            true,
            false,
            NamedMcpPeerError::EndpointUnavailable,
            4,
        ),
        (
            "2025-11-25",
            false,
            true,
            NamedMcpPeerError::EndpointUnavailable,
            1,
        ),
    ] {
        let peer = ProtocolPeer::with_behavior(version, wrong_id, redirect);
        let id = McpServerId("peer:fault".into());
        let mut app = AppComposition::new();
        app.set_product_mode(AppProductMode::Automate);
        let state = app
            .configure_named_mcp_peer(NamedMcpPeerConfig::http_client(
                id.clone(),
                "Fault",
                &peer.endpoint,
            ))
            .unwrap();
        app.grant_named_mcp_peer_transport(&id, state.revision, NamedMcpPeerPermissions::network())
            .unwrap();
        assert_eq!(
            app.activate_named_mcp_peer(&id, state.revision, &InMemorySecretStore::default()),
            Err(expected)
        );
        let unavailable = app.inspect_named_mcp_peer(&id).unwrap();
        assert_eq!(unavailable.health, McpPeerHealth::Unavailable);
        assert!(!unavailable.transport_granted);
        assert_eq!(peer.transcript().len(), exchanges);
        assert_eq!(
            app.probe_named_mcp_peer(&id, state.revision),
            Err(NamedMcpPeerError::PermissionRequired)
        );
    }
}

#[test]
fn reconfiguration_retires_old_grant_and_stdio_roles_require_permission_and_containment() {
    use legion_app::{
        AppProductMode,
        named_mcp_peer::{NamedMcpPeerError, NamedMcpPeerPermissions, NamedMcpPeerTransport},
    };
    use legion_storage::secrets::InMemorySecretStore;
    let peer = ProtocolPeer::start("2025-11-25");
    let id = McpServerId("peer:revision".into());
    let mut app = AppComposition::new();
    app.set_product_mode(AppProductMode::Automate);
    let config = NamedMcpPeerConfig::http_client(id.clone(), "Revision", &peer.endpoint);
    let state = app.configure_named_mcp_peer(config.clone()).unwrap();
    app.grant_named_mcp_peer_transport(&id, state.revision, NamedMcpPeerPermissions::network())
        .unwrap();
    app.activate_named_mcp_peer(&id, state.revision, &InMemorySecretStore::default())
        .unwrap();
    let replacement = app.configure_named_mcp_peer(config).unwrap();
    assert_eq!(replacement.revision, state.revision + 1);
    assert_eq!(
        app.activate_named_mcp_peer(&id, state.revision, &InMemorySecretStore::default()),
        Err(NamedMcpPeerError::StaleRevision)
    );
    assert_eq!(
        app.activate_named_mcp_peer(&id, replacement.revision, &InMemorySecretStore::default()),
        Err(NamedMcpPeerError::PermissionRequired)
    );
    assert_eq!(peer.transcript().len(), 4);
    for role in [McpPeerRole::Client, McpPeerRole::Server] {
        let mut config =
            NamedMcpPeerConfig::http_client(id.clone(), "Stdio declaration", &peer.endpoint);
        config.metadata.role = role;
        config.metadata.transport = legion_protocol::McpTransportKind::Stdio;
        config.transport = NamedMcpPeerTransport::Stdio {
            command: "unprovisioned-mcp-peer".into(),
            args: vec![],
        };
        let state = app.configure_named_mcp_peer(config).unwrap();
        assert_eq!(
            app.grant_named_mcp_peer_transport(
                &id,
                state.revision,
                NamedMcpPeerPermissions::network()
            ),
            Err(NamedMcpPeerError::PermissionRequired)
        );
        app.grant_named_mcp_peer_transport(
            &id,
            state.revision,
            NamedMcpPeerPermissions {
                network: false,
                process: true,
            },
        )
        .unwrap();
        assert_eq!(
            app.activate_named_mcp_peer(&id, state.revision, &InMemorySecretStore::default()),
            Err(NamedMcpPeerError::UnsupportedEnvironment)
        );
    }
}

impl Drop for ProtocolPeer {
    fn drop(&mut self) {
        self.stop.store(true, std::sync::atomic::Ordering::SeqCst);
        if let Some(worker) = self.worker.take() {
            worker.join().unwrap();
        }
    }
}

#[test]
fn named_http_peer_negotiates_pinned_protocol_and_scoped_auth_without_inference() {
    use legion_app::{
        AppProductMode,
        named_mcp_peer::{NamedMcpPeerPermissions, named_mcp_peer_secret_reference},
    };
    use legion_storage::secrets::{InMemorySecretStore, SecretStore};
    let peer = ProtocolPeer::start("2025-11-25");
    let id = McpServerId("peer:reference".into());
    let mut config = NamedMcpPeerConfig::http_client(id.clone(), "Reference", &peer.endpoint);
    config.metadata.authentication = McpPeerAuthentication::Bearer;
    config.metadata.credential_scopes = vec!["tools:read".into()];
    let secrets = InMemorySecretStore::default();
    secrets
        .store(
            &named_mcp_peer_secret_reference(&config),
            "synthetic-peer-token",
        )
        .unwrap();
    let mut app = AppComposition::new();
    app.set_product_mode(AppProductMode::Automate);
    let configured = app.configure_named_mcp_peer(config).unwrap();
    app.grant_named_mcp_peer_transport(
        &id,
        configured.revision,
        NamedMcpPeerPermissions::network(),
    )
    .unwrap();
    let ready = app
        .activate_named_mcp_peer(&id, configured.revision, &secrets)
        .unwrap();
    assert_eq!(ready.health, McpPeerHealth::Ready);
    assert_eq!(
        app.probe_named_mcp_peer(&id, configured.revision)
            .unwrap()
            .health,
        McpPeerHealth::Ready
    );
    let transcript = peer.transcript();
    let methods: Vec<_> = transcript
        .iter()
        .map(|(_, _, request)| request["method"].as_str().unwrap())
        .collect();
    assert_eq!(
        methods,
        [
            "initialize",
            "notifications/initialized",
            "tools/list",
            "ping",
            "ping"
        ]
    );
    assert_eq!(transcript[0].2["params"]["protocolVersion"], "2025-11-25");
    assert!(
        transcript
            .iter()
            .all(|(auth, version, _)| auth == "Bearer synthetic-peer-token"
                && version == "2025-11-25")
    );
    let projected = serde_json::to_string(
        &app.legion_workflow_projection(legion_protocol::TimestampMillis::now()),
    )
    .unwrap();
    assert!(projected.contains("inspect"));
    assert!(!projected.contains("synthetic-peer-token"));
    assert!(!projected.contains("UNTRUSTED RAW DESCRIPTION"));
    app.set_product_mode(AppProductMode::Manual);
    assert!(app.probe_named_mcp_peer(&id, configured.revision).is_err());
    assert_eq!(peer.transcript().len(), 5);
}

#[test]
fn activation_requires_nonmanual_mode_current_transport_grant_and_peer_credential() {
    use legion_app::{
        AppProductMode,
        named_mcp_peer::{NamedMcpPeerError, NamedMcpPeerPermissions},
    };
    use legion_storage::secrets::InMemorySecretStore;
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let id = McpServerId("peer:denied".into());
    let mut config = NamedMcpPeerConfig::http_client(
        id.clone(),
        "Denied",
        format!("http://{}/rpc", listener.local_addr().unwrap()),
    );
    config.metadata.authentication = McpPeerAuthentication::Bearer;
    config.metadata.credential_scopes = vec!["tools:read".into()];
    let mut app = AppComposition::new();
    let state = app.configure_named_mcp_peer(config).unwrap();
    let secrets = InMemorySecretStore::default();
    assert_eq!(
        app.activate_named_mcp_peer(&id, state.revision, &secrets),
        Err(NamedMcpPeerError::ManualMode)
    );
    app.set_product_mode(AppProductMode::Automate);
    assert_eq!(
        app.activate_named_mcp_peer(&id, state.revision, &secrets),
        Err(NamedMcpPeerError::PermissionRequired)
    );
    assert_eq!(
        app.grant_named_mcp_peer_transport(
            &id,
            state.revision + 1,
            NamedMcpPeerPermissions::network()
        ),
        Err(NamedMcpPeerError::StaleRevision)
    );
    app.grant_named_mcp_peer_transport(&id, state.revision, NamedMcpPeerPermissions::network())
        .unwrap();
    assert_eq!(
        app.activate_named_mcp_peer(&id, state.revision, &secrets),
        Err(NamedMcpPeerError::CredentialUnavailable)
    );
    assert_eq!(
        listener.accept().unwrap_err().kind(),
        std::io::ErrorKind::WouldBlock
    );
}

#[test]
fn unsupported_or_ambiguous_peer_contract_is_rejected_without_replacing_configuration() {
    use legion_app::named_mcp_peer::{NamedMcpPeerError, NamedMcpPeerTransport};
    use legion_protocol::McpTransportKind;
    let mut app = AppComposition::new();
    let original = NamedMcpPeerConfig::http_client(
        McpServerId("peer:pinned".into()),
        "Pinned",
        "https://mcp.example.test/rpc",
    );
    let before = app.configure_named_mcp_peer(original.clone()).unwrap();
    let mut invalid = vec![];
    let mut version = original.clone();
    version.metadata.protocol_version = "unknown".into();
    invalid.push(version);
    let mut role = original.clone();
    role.metadata.role = McpPeerRole::Server;
    invalid.push(role);
    let mut transport = original.clone();
    transport.metadata.transport = McpTransportKind::Stdio;
    invalid.push(transport);
    let mut scope = original.clone();
    scope.metadata.authentication = McpPeerAuthentication::Bearer;
    invalid.push(scope);
    let mut id = original.clone();
    id.metadata.peer_id = McpServerId("".into());
    invalid.push(id);
    for endpoint in [
        "http://mcp.example.test/rpc",
        "https://user:password@mcp.example.test/rpc",
        "https://mcp.example.test/rpc?token=secret",
        "https://mcp.example.test/rpc#secret",
        "file:///tmp/peer",
    ] {
        let mut config = original.clone();
        config.transport = NamedMcpPeerTransport::Http {
            endpoint: endpoint.into(),
        };
        invalid.push(config);
    }
    for config in invalid {
        assert_eq!(
            app.configure_named_mcp_peer(config),
            Err(NamedMcpPeerError::InvalidConfiguration)
        );
    }
    assert_eq!(
        app.inspect_named_mcp_peer(&before.metadata.peer_id),
        Some(before)
    );
}

#[test]
fn configure_named_peer_in_manual_mode_exposes_only_metadata() {
    let mut app = AppComposition::new();
    let config = NamedMcpPeerConfig::http_client(
        McpServerId("peer:reference".into()),
        "Reference peer",
        "http://127.0.0.1:43123/private-endpoint",
    );
    let state = app.configure_named_mcp_peer(config).unwrap();
    assert_eq!(state.health, McpPeerHealth::Configured);
    assert_eq!(state.metadata.protocol_version, "2025-11-25");
    assert_eq!(
        app.inspect_named_mcp_peer(&McpServerId("peer:reference".into())),
        Some(state.clone())
    );
    let retained = serde_json::to_string(&state).unwrap();
    assert!(retained.contains("Reference peer"));
    assert!(!retained.contains("private-endpoint"));
    assert!(!retained.contains("43123"));
}
