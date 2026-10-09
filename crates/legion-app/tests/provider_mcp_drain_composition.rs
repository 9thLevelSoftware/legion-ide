#![cfg(feature = "ai")]

//! Composition contract: independent held HTTP peers outlive neither policy nor
//! their drain leases. This is local protocol evidence, not native/live acceptance.

use legion_app::{
    AiProviderConnectionState, AiProviderProfile, AppComposition, AppProductMode,
    named_mcp_peer::{NamedMcpPeerConfig, NamedMcpPeerError, NamedMcpPeerPermissions},
};
use legion_protocol::{
    CanonicalPath, DelegatedTaskRiskTolerance, DelegatedTaskScope, DelegatedTaskScopeTargetKind,
    LegionToolKind, McpServerId, PrincipalId, WorkspaceTrustState, named_mcp_peer::McpPeerHealth,
};
use std::{
    io::{BufRead, Read, Write},
    sync::{Arc, mpsc},
    time::{Duration, Instant},
};

struct HeldResponse {
    endpoint: String,
    received: mpsc::Receiver<()>,
    release: Option<mpsc::Sender<()>>,
    worker: Option<std::thread::JoinHandle<()>>,
}

impl HeldResponse {
    fn new(mcp: bool) -> Self {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let endpoint = format!(
            "http://{}/{}",
            listener.local_addr().unwrap(),
            if mcp { "mcp" } else { "v1" }
        );
        let (observed, received) = mpsc::channel();
        let (release, released) = mpsc::channel();
        let worker = std::thread::spawn(move || {
            let deadline = Instant::now() + Duration::from_secs(10);
            let mut stream = loop {
                match listener.accept() {
                    Ok((stream, _)) => break stream,
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        assert!(
                            Instant::now() < deadline,
                            "expected HTTP request never arrived"
                        );
                        std::thread::sleep(Duration::from_millis(2));
                    }
                    Err(e) => panic!("controlled listener: {e}"),
                }
            };
            stream.set_nonblocking(false).unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            let mut reader = std::io::BufReader::new(stream.try_clone().unwrap());
            let mut line = String::new();
            reader.read_line(&mut line).unwrap();
            assert_eq!(
                line.trim(),
                if mcp {
                    "POST /mcp HTTP/1.1"
                } else {
                    "POST /v1/chat/completions HTTP/1.1"
                }
            );
            let mut length = None;
            loop {
                line.clear();
                assert!(reader.read_line(&mut line).unwrap() > 0);
                if line == "\r\n" {
                    break;
                }
                let (name, value) = line.split_once(':').unwrap();
                if name.eq_ignore_ascii_case("content-length") {
                    length = Some(value.trim().parse::<usize>().unwrap());
                }
            }
            let length = length.expect("bounded JSON request");
            assert!(length < 8192);
            let mut body = vec![0; length];
            reader.read_exact(&mut body).unwrap();
            let request: serde_json::Value = serde_json::from_slice(&body).unwrap();
            let response = if mcp {
                assert_eq!(request["method"], "initialize");
                serde_json::json!({"jsonrpc":"2.0", "id":request["id"], "result":{
                    "protocolVersion":"2025-11-25", "capabilities":{"tools":{}},
                    "serverInfo":{"name":"held-composition-peer","version":"1"}
                }})
            } else {
                assert_eq!(request["model"], "composition-local");
                assert_eq!(request["max_tokens"], 8);
                assert_eq!(
                    request["messages"][1]["content"],
                    "Reply OK to confirm this configured connection."
                );
                serde_json::json!({"choices":[{"message":{"content":"late-composition-result"}}]})
            }
            .to_string();
            observed.send(()).unwrap();
            // Only explicit test release ends this hold; timeout is a failure,
            // never an oracle for transport completion or cancellation.
            released.recv_timeout(Duration::from_secs(10)).unwrap();
            write!(stream,
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nConnection: close\r\nContent-Length: {}\r\n\r\n{}",
                response.len(), response).unwrap();
        });
        Self {
            endpoint,
            received,
            release: Some(release),
            worker: Some(worker),
        }
    }

    fn wait_received(&self) {
        self.received.recv_timeout(Duration::from_secs(5)).unwrap();
    }

    fn release_response(&mut self) {
        self.release.take().unwrap().send(()).unwrap();
        self.worker.take().unwrap().join().unwrap();
    }
}

impl Drop for HeldResponse {
    fn drop(&mut self) {
        if let Some(release) = self.release.take() {
            let _ = release.send(());
        }
        if let Some(worker) = self.worker.take() {
            let result = worker.join();
            if !std::thread::panicking() {
                result.unwrap();
            }
        }
    }
}

fn pump_until(app: &mut AppComposition, mut finished: impl FnMut(&AppComposition) -> bool) {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        // Exercise both public owners. Reversing transport release order below
        // also checks a still-pending provider handoff after MCP has returned.
        app.poll_ai_provider_connection_check();
        app.poll_named_mcp_peer_operations();
        if finished(app) {
            return;
        }
        assert!(
            Instant::now() < deadline,
            "combined drains did not reconcile"
        );
        std::thread::sleep(Duration::from_millis(2));
    }
}

fn install_policy(app: &mut AppComposition, payload: &toml::Value) {
    use legion_security::{
        PolicyKeyring, PolicySigningKey, policy_bundle_verifying_key_b64, sign_policy_bundle,
    };
    let keyring = PolicyKeyring::new(vec![PolicySigningKey {
        key_id: "composition-test".into(),
        verifying_key_b64: policy_bundle_verifying_key_b64(&[7; 32]),
    }]);
    app.set_org_policy_bundle(
        sign_policy_bundle(
            &toml::to_string(payload).unwrap(),
            "composition-test",
            &[7; 32],
        )
        .verify(&keyring)
        .unwrap(),
    );
}

fn assert_new_work_denied(
    app: &mut AppComposition,
    profile: &AiProviderProfile,
    other_peer: &McpServerId,
    root: &std::path::Path,
) {
    assert_eq!(app.product_mode(), AppProductMode::Delegate);
    assert!(app.provider_configuration_busy());
    assert!(
        app.start_ai_provider_connection_check(profile)
            .unwrap_err()
            .to_string()
            .contains("installed organization mode ceiling")
    );
    assert_eq!(
        app.start_named_mcp_peer_connection(other_peer, 1),
        Err(NamedMcpPeerError::PermissionRequired)
    );
    assert_eq!(
        app.activate_named_mcp_peer(
            other_peer,
            1,
            &legion_storage::InMemorySecretStore::default()
        ),
        Err(NamedMcpPeerError::PermissionRequired)
    );
    let scope = DelegatedTaskScope {
        target_kind: DelegatedTaskScopeTargetKind::Repo,
        workspace_root: CanonicalPath(root.to_string_lossy().into_owned()),
        target_path: None,
        risk_tolerance: DelegatedTaskRiskTolerance::Balanced,
        allowed_tools: vec![LegionToolKind::Read],
        forbidden_paths: vec![],
        schema_version: 1,
    };
    let provider = legion_ai::tool_calls::ScriptedToolCallingProviderBuilder::new()
        .end_turn("must not be invoked")
        .build("denied-composition");
    assert!(
        app.start_delegated_task_background("must not start".into(), scope, Box::new(provider))
            .unwrap_err()
            .to_string()
            .contains("installed organization mode ceiling")
    );
    assert!(!root.join("target/delegated-tasks").exists());
}

#[test]
fn provider_and_active_or_retired_mcp_wait_for_both_response_orders_before_manual() {
    for retire_mcp in [false, true] {
        for provider_first in [false, true] {
            let root = tempfile::tempdir().unwrap();
            let path = root.path().join("main.rs");
            std::fs::write(&path, "fn main() {}\n").unwrap();
            let mut provider = HeldResponse::new(false);
            let mut mcp = HeldResponse::new(true);
            let mut app = AppComposition::with_provider_secret_store(Arc::new(
                legion_storage::InMemorySecretStore::default(),
            ));
            app.open_workspace(
                root.path(),
                WorkspaceTrustState::Trusted,
                PrincipalId("composition-test".into()),
            )
            .unwrap();
            app.open_file(path.to_string_lossy()).unwrap();
            let buffer = app
                .shell_projection_snapshot("composition")
                .unwrap()
                .active_buffer_projection
                .buffer_id
                .unwrap();
            app.set_product_mode(AppProductMode::Delegate);

            // Establish allowed loopback invocation first. The second bundle
            // changes only mode_ceiling, so trust/network denial cannot mask it.
            let mut policy: toml::Value =
                toml::from_str(include_str!("../../../xtask/legion-policy.example.toml")).unwrap();
            policy["mode_ceiling"] = "Delegates".into();
            policy["security_policy"]["bundle_enforcement"]["provider"]["allowed_provider_ids"] =
                toml::Value::Array(vec!["llama-cpp".into()]);
            // Configured routes intentionally declare unknown cost, including
            // this controlled local peer. The test policy permits that upfront.
            policy["security_policy"]["bundle_enforcement"]["budget"]["cost_declaration_required_prefixes"] =
                toml::Value::Array(vec![]);
            install_policy(&mut app, &policy);
            let profile = AiProviderProfile {
                name: "composition-provider".into(),
                provider_id: "llama-cpp".into(),
                endpoint: provider.endpoint.clone(),
                model: "composition-local".into(),
                max_completion_tokens: false,
                disable_thinking: false,
            };
            app.configure_ai_provider_profile(profile.clone()).unwrap();
            app.select_ai_provider_profile(&profile.name).unwrap();
            let id = McpServerId("composition-mcp".into());
            let config = NamedMcpPeerConfig::http_client(id.clone(), "Held MCP", &mcp.endpoint);
            app.configure_named_mcp_peer(config.clone()).unwrap();
            app.grant_named_mcp_peer_transport(&id, 1, NamedMcpPeerPermissions::network())
                .unwrap();
            let other = McpServerId("new-mcp-dispatch".into());
            app.configure_named_mcp_peer(NamedMcpPeerConfig::http_client(
                other.clone(),
                "Admission oracle",
                &mcp.endpoint,
            ))
            .unwrap();
            app.grant_named_mcp_peer_transport(&other, 1, NamedMcpPeerPermissions::network())
                .unwrap();

            // The existing provider idle gate requires this start order. Both
            // requests are independently observed held before policy changes.
            app.start_ai_provider_connection_check(&profile).unwrap();
            provider.wait_received();
            app.start_named_mcp_peer_connection(&id, 1).unwrap();
            mcp.wait_received();
            if retire_mcp {
                app.configure_named_mcp_peer(config).unwrap();
            }
            policy["mode_ceiling"] = "Manual".into();
            install_policy(&mut app, &policy);
            assert_eq!(
                app.ai_provider_profiles()[0].connection_check,
                AiProviderConnectionState::Cancelling
            );
            assert_new_work_denied(&mut app, &profile, &other, root.path());
            let snapshot = app.inspect_named_mcp_peer(&id).unwrap();
            assert!(!snapshot.transport_granted);
            assert_eq!(
                snapshot.health,
                if retire_mcp {
                    McpPeerHealth::Configured
                } else {
                    McpPeerHealth::Revoked
                }
            );

            if provider_first {
                provider.release_response();
                pump_until(&mut app, |app| {
                    !app.ai_provider_connection_check_in_flight()
                });
                assert_eq!(
                    app.ai_provider_profiles()[0].connection_check,
                    AiProviderConnectionState::Cancelled
                );
                // No current peer handle exists in the retired branch. Only
                // its worker's lease can keep this still-held HTTP non-Manual.
                assert_new_work_denied(&mut app, &profile, &other, root.path());
                mcp.release_response();
            } else {
                mcp.release_response();
                if !retire_mcp {
                    pump_until(&mut app, |app| !app.named_mcp_peer_operation_pending(&id));
                } else {
                    // Retirement has no per-peer completion projection. Observe
                    // response release, then final common drain below; never
                    // infer the retired worker finished from a timer.
                    app.poll_named_mcp_peer_operations();
                }
                assert!(app.ai_provider_connection_check_in_flight());
                assert_new_work_denied(&mut app, &profile, &other, root.path());
                provider.release_response();
            }
            pump_until(&mut app, |app| {
                !app.provider_configuration_busy()
                    && !app.named_mcp_peer_operation_pending(&id)
                    && app.product_mode() == AppProductMode::Manual
            });
            assert_eq!(
                app.ai_provider_profiles()[0].connection_check,
                AiProviderConnectionState::Cancelled
            );
            assert_eq!(app.inspect_named_mcp_peer(&id).unwrap(), snapshot);
            assert_eq!(app.editor().text(buffer).unwrap(), "fn main() {}\n");
            assert_eq!(std::fs::read(&path).unwrap(), b"fn main() {}\n");
            eprintln!(
                "composed drain passed: retired_mcp={retire_mcp}, provider_response_first={provider_first}"
            );
        }
    }
}
