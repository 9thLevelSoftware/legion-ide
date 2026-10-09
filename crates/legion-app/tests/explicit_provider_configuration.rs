use legion_app::{
    AppComposition, AppCompositionError, AppProductMode, ProductAiProviderPreference,
};
use legion_protocol::{PrincipalId, TextCoordinate, WorkspaceTrustState};
use legion_ui::CommandDispatchIntent;

fn app_with_file() -> (tempfile::TempDir, AppComposition, legion_protocol::BufferId) {
    let root = tempfile::tempdir().unwrap();
    let file = root.path().join("main.rs");
    std::fs::write(&file, "fn main() {}\n").unwrap();
    let mut app = AppComposition::with_provider_secret_store(std::sync::Arc::new(
        legion_storage::InMemorySecretStore::default(),
    ));
    app.open_workspace(
        root.path(),
        WorkspaceTrustState::Trusted,
        PrincipalId("profile-test".into()),
    )
    .unwrap();
    app.open_file(file.to_string_lossy()).unwrap();
    let buffer = app
        .shell_projection_snapshot("test")
        .unwrap()
        .active_buffer_projection
        .buffer_id
        .unwrap();
    (root, app, buffer)
}

#[test]
fn connection_check_uses_selected_route_fixed_prompt_and_truthful_lifecycle() {
    use legion_app::AiProviderConnectionState;
    use std::io::{Read, Write};
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let profile = legion_app::AiProviderProfile {
        name: "health-pilot".into(),
        provider_id: "openai-compatible".into(),
        endpoint: format!("http://{}/v1", listener.local_addr().unwrap()),
        model: "mimo-v2.6-pro".into(),
        max_completion_tokens: true,
        disable_thinking: true,
    };
    let (_root, mut app, buffer) = app_with_file();
    app.configure_ai_provider_profile(profile.clone()).unwrap();
    app.select_ai_provider_profile(&profile.name).unwrap();
    app.replace_ai_profile_credential(&profile.name, "synthetic-health-key")
        .unwrap();
    assert_eq!(
        app.ai_provider_profiles()[0].connection_check,
        AiProviderConnectionState::Idle
    );
    assert!(app.start_ai_provider_connection_check(&profile).is_err());
    assert_eq!(
        listener.accept().unwrap_err().kind(),
        std::io::ErrorKind::WouldBlock
    );
    let (sent, received) = std::sync::mpsc::channel();
    let (release, released) = std::sync::mpsc::channel();
    let peer = std::thread::spawn(move || {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(8);
        let (mut stream, _) = loop {
            if let Ok(connection) = listener.accept() {
                break connection;
            }
            assert!(std::time::Instant::now() < deadline, "no check arrived");
            std::thread::sleep(std::time::Duration::from_millis(5));
        };
        stream
            .set_read_timeout(Some(std::time::Duration::from_secs(5)))
            .unwrap();
        let mut bytes = Vec::new();
        loop {
            let mut chunk = [0; 2048];
            let count = stream.read(&mut chunk).unwrap();
            assert!(count > 0);
            bytes.extend_from_slice(&chunk[..count]);
            assert!(bytes.len() < 8192);
            if let Some(end) = bytes.windows(4).position(|w| w == b"\r\n\r\n") {
                let headers = String::from_utf8_lossy(&bytes[..end]).to_ascii_lowercase();
                let length: usize = headers
                    .lines()
                    .find_map(|s| s.strip_prefix("content-length: "))
                    .unwrap()
                    .parse()
                    .unwrap();
                if bytes.len() >= end + 4 + length {
                    break;
                }
            }
        }
        sent.send(String::from_utf8(bytes).unwrap()).unwrap();
        released
            .recv_timeout(std::time::Duration::from_secs(5))
            .unwrap();
        let body =
            r#"{"choices":[{"message":{"role":"assistant","content":"private-provider-output"}}]}"#;
        write!(stream, "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}", body.len(), body).unwrap();
    });
    app.set_product_mode(AppProductMode::Assist);
    app.start_ai_provider_connection_check(&profile).unwrap();
    assert_eq!(
        app.ai_provider_profiles()[0].connection_check,
        AiProviderConnectionState::Checking
    );
    let request = received
        .recv_timeout(std::time::Duration::from_secs(5))
        .unwrap();
    assert!(request.starts_with("POST /v1/chat/completions HTTP/1.1\r\n"));
    assert!(
        request
            .to_ascii_lowercase()
            .contains("authorization: bearer synthetic-health-key\r\n")
    );
    let body: serde_json::Value =
        serde_json::from_str(request.split_once("\r\n\r\n").unwrap().1).unwrap();
    assert_eq!(body["model"], "mimo-v2.6-pro");
    assert_eq!(body["max_completion_tokens"], 8);
    assert!(body.get("max_tokens").is_none());
    assert_eq!(body["thinking"]["type"], "disabled");
    assert_eq!(
        body["messages"],
        serde_json::json!([
            {"role":"system", "content":"Connection check only. Do not use tools."},
            {"role":"user", "content":"Reply OK to confirm this configured connection."}
        ])
    );
    release.send(()).unwrap();
    peer.join().unwrap();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while app.ai_provider_connection_check_in_flight() {
        app.poll_ai_provider_connection_check();
        assert!(std::time::Instant::now() < deadline);
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    assert_eq!(
        app.ai_provider_profiles()[0].connection_check,
        AiProviderConnectionState::Succeeded
    );
    assert_eq!(app.editor().text(buffer).unwrap(), "fn main() {}\n");
    let rendered = format!(
        "{:?} {}",
        app.ai_provider_profiles(),
        app.ai_provider_configuration_json().unwrap()
    );
    assert!(!rendered.contains("synthetic-health-key"));
    assert!(!rendered.contains("private-provider-output"));
}

#[test]
fn connection_check_cancellation_drains_then_discards_late_success() {
    use legion_app::{AiProviderConnectionState, AiProviderProfile};
    use std::io::{Read, Write};
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let profile = AiProviderProfile {
        name: "cancel-check".into(),
        provider_id: "llama-cpp".into(),
        endpoint: format!("http://{}/v1", listener.local_addr().unwrap()),
        model: "local-test".into(),
        max_completion_tokens: false,
        disable_thinking: false,
    };
    let (_root, mut app, buffer) = app_with_file();
    app.configure_ai_provider_profile(profile.clone()).unwrap();
    app.select_ai_provider_profile(&profile.name).unwrap();
    app.replace_ai_profile_credential(&profile.name, "synthetic-cancel-key")
        .unwrap();
    app.set_product_mode(AppProductMode::Assist);
    let (started, observed) = std::sync::mpsc::channel();
    let (release, released) = std::sync::mpsc::channel();
    listener.set_nonblocking(true).unwrap();
    let peer = std::thread::spawn(move || {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        let (mut stream, _) = loop {
            if let Ok(stream) = listener.accept() {
                break stream;
            }
            assert!(std::time::Instant::now() < deadline);
            std::thread::sleep(std::time::Duration::from_millis(5));
        };
        stream
            .set_read_timeout(Some(std::time::Duration::from_secs(5)))
            .unwrap();
        let mut request = [0; 4096];
        assert!(stream.read(&mut request).unwrap() > 0);
        started.send(()).unwrap();
        released
            .recv_timeout(std::time::Duration::from_secs(5))
            .unwrap();
        let body = r#"{"choices":[{"message":{"content":"late-private-check-result"}}]}"#;
        write!(
            stream,
            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            body.len(),
            body
        )
        .unwrap();
    });
    app.start_ai_provider_connection_check(&profile).unwrap();
    observed
        .recv_timeout(std::time::Duration::from_secs(5))
        .unwrap();
    let mut stale = profile.clone();
    stale.model = "different-model".into();
    assert!(app.cancel_ai_provider_connection_check(&stale).is_err());
    app.cancel_ai_provider_connection_check(&profile).unwrap();
    assert_eq!(
        app.ai_provider_profiles()[0].connection_check,
        AiProviderConnectionState::Cancelling
    );
    assert!(app.configure_ai_provider_profile(stale.clone()).is_err());
    assert!(app.select_ai_provider_profile(&profile.name).is_err());
    assert!(app.revoke_ai_profile_credential(&profile.name).is_err());
    assert!(app.start_ai_provider_connection_check(&profile).is_err());
    app.set_product_mode(AppProductMode::Manual);
    assert_eq!(
        app.product_mode(),
        AppProductMode::Assist,
        "do not claim Manual while the request is live"
    );
    release.send(()).unwrap();
    peer.join().unwrap();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while app.ai_provider_connection_check_in_flight() {
        app.poll_ai_provider_connection_check();
        assert!(std::time::Instant::now() < deadline);
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    assert_eq!(
        app.ai_provider_profiles()[0].connection_check,
        AiProviderConnectionState::Cancelled
    );
    assert!(
        !app.ai_provider_profiles()[0]
            .health
            .starts_with("responded")
    );
    assert!(
        !format!("{:?}", app.live_product_ai_stream_snapshot())
            .contains("late-private-check-result")
    );
    assert_eq!(app.editor().text(buffer).unwrap(), "fn main() {}\n");
    app.configure_ai_provider_profile(stale).unwrap();
    assert_eq!(
        app.ai_provider_profiles()[0].connection_check,
        AiProviderConnectionState::Idle
    );
    app.set_product_mode(AppProductMode::Manual);
    assert_eq!(app.product_mode(), AppProductMode::Manual);
}

#[test]
fn connection_check_refuses_stale_missing_credentials_and_policy_without_requests() {
    use legion_app::{AiProviderConnectionState, AiProviderProfile};
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let profile = AiProviderProfile {
        name: "refused-check".into(),
        provider_id: "openai-compatible".into(),
        endpoint: format!("http://{}/v1", listener.local_addr().unwrap()),
        model: "mimo-v2.6-pro".into(),
        max_completion_tokens: true,
        disable_thinking: true,
    };
    let (_root, mut app, _) = app_with_file();
    app.configure_ai_provider_profile(profile.clone()).unwrap();
    app.set_product_mode(AppProductMode::Assist);
    assert!(
        app.start_ai_provider_connection_check(&profile).is_err(),
        "unselected route is never authorized"
    );
    app.select_ai_provider_profile(&profile.name).unwrap();
    assert!(
        app.start_ai_provider_connection_check(&profile)
            .unwrap_err()
            .to_string()
            .contains("credential missing")
    );
    assert_eq!(
        app.ai_provider_profiles()[0].connection_check,
        AiProviderConnectionState::Failed
    );
    app.replace_ai_profile_credential(&profile.name, "synthetic-policy-key")
        .unwrap();
    let mut changed = profile.clone();
    changed.model = "replacement-model".into();
    app.configure_ai_provider_profile(changed.clone()).unwrap();
    assert!(app.start_ai_provider_connection_check(&profile).is_err());
    app.replace_ai_profile_credential(&changed.name, "synthetic-replacement-key")
        .unwrap();
    let seed = [7u8; 32];
    let keyring = legion_security::PolicyKeyring::new(vec![legion_security::PolicySigningKey {
        key_id: "health-policy".into(),
        verifying_key_b64: legion_security::policy_bundle_verifying_key_b64(&seed),
    }]);
    let payload = include_str!("../../../xtask/legion-policy.example.toml").replace(
        "provider_invocation_enabled = true",
        "provider_invocation_enabled = false",
    );
    app.set_org_policy_bundle(
        legion_security::sign_policy_bundle(&payload, "health-policy", &seed)
            .verify(&keyring)
            .unwrap(),
    );
    assert!(
        app.start_ai_provider_connection_check(&changed)
            .unwrap_err()
            .to_string()
            .contains("denied by provider policy")
    );
    assert_eq!(
        listener.accept().unwrap_err().kind(),
        std::io::ErrorKind::WouldBlock
    );
    assert!(!app.ai_provider_connection_check_in_flight());
    assert!(!app.provider_configuration_busy());
}

#[test]
fn connection_check_http_or_malformed_failure_never_falls_back_or_exposes_payloads() {
    use legion_app::{AiProviderConnectionState, AiProviderProfile};
    use std::io::{Read, Write};
    for (status, body) in [
        ("401 Unauthorized", "private-http-error-body"),
        ("200 OK", "{bad json private-content"),
        ("200 OK", r#"{"choices":[{"message":{"content":""}}]}"#),
    ] {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let profile = AiProviderProfile {
            name: "failed-check".into(),
            provider_id: "llama-cpp".into(),
            endpoint: format!("http://{}/v1", listener.local_addr().unwrap()),
            model: "local-test".into(),
            max_completion_tokens: false,
            disable_thinking: false,
        };
        let (_root, mut app, buffer) = app_with_file();
        app.configure_ai_provider_profile(profile.clone()).unwrap();
        app.select_ai_provider_profile(&profile.name).unwrap();
        app.set_product_mode(AppProductMode::Assist);
        let peer = std::thread::spawn(move || {
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
            let (mut stream, _) = loop {
                if let Ok(stream) = listener.accept() {
                    break stream;
                }
                assert!(std::time::Instant::now() < deadline);
                std::thread::sleep(std::time::Duration::from_millis(5));
            };
            stream
                .set_read_timeout(Some(std::time::Duration::from_secs(5)))
                .unwrap();
            let mut request = [0; 4096];
            assert!(stream.read(&mut request).unwrap() > 0);
            write!(
                stream,
                "HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            )
            .unwrap();
            stream.flush().unwrap();
            drop(stream);
            listener
        });
        app.start_ai_provider_connection_check(&profile).unwrap();
        let listener = peer.join().unwrap();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while app.ai_provider_connection_check_in_flight() {
            app.poll_ai_provider_connection_check();
            assert!(std::time::Instant::now() < deadline);
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        assert_eq!(
            app.ai_provider_profiles()[0].connection_check,
            AiProviderConnectionState::Failed
        );
        assert_eq!(
            listener.accept().unwrap_err().kind(),
            std::io::ErrorKind::WouldBlock,
            "no retry"
        );
        assert_eq!(app.editor().text(buffer).unwrap(), "fn main() {}\n");
        let projections = format!(
            "{:?} {:?}",
            app.ai_provider_profiles(),
            app.live_product_ai_stream_snapshot()
        );
        assert!(!projections.contains("private-"));
        assert!(!projections.contains("fixture"));
    }
}

#[test]
fn connection_check_policy_downgrade_waits_for_actual_request_drain() {
    use legion_app::{AiProviderConnectionState, AiProviderProfile};
    use std::io::{Read, Write};
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let profile = AiProviderProfile {
        name: "policy-drain-check".into(),
        provider_id: "llama-cpp".into(),
        endpoint: format!("http://{}/v1", listener.local_addr().unwrap()),
        model: "local-test".into(),
        max_completion_tokens: false,
        disable_thinking: false,
    };
    let (_root, mut app, _) = app_with_file();
    app.configure_ai_provider_profile(profile.clone()).unwrap();
    app.select_ai_provider_profile(&profile.name).unwrap();
    app.set_product_mode(AppProductMode::Assist);
    let (started, observed) = std::sync::mpsc::channel();
    let (release, released) = std::sync::mpsc::channel();
    let peer = std::thread::spawn(move || {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        let (mut stream, _) = loop {
            if let Ok(stream) = listener.accept() {
                break stream;
            }
            assert!(std::time::Instant::now() < deadline);
            std::thread::sleep(std::time::Duration::from_millis(5));
        };
        stream
            .set_read_timeout(Some(std::time::Duration::from_secs(5)))
            .unwrap();
        let mut request = [0; 4096];
        assert!(stream.read(&mut request).unwrap() > 0);
        started.send(()).unwrap();
        released
            .recv_timeout(std::time::Duration::from_secs(5))
            .unwrap();
        let body = r#"{"choices":[{"message":{"content":"late-policy-result"}}]}"#;
        let _ = write!(
            stream,
            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            body.len(),
            body
        );
    });
    app.start_ai_provider_connection_check(&profile).unwrap();
    observed
        .recv_timeout(std::time::Duration::from_secs(5))
        .unwrap();
    let seed = [7u8; 32];
    let keyring = legion_security::PolicyKeyring::new(vec![legion_security::PolicySigningKey {
        key_id: "health-ceiling".into(),
        verifying_key_b64: legion_security::policy_bundle_verifying_key_b64(&seed),
    }]);
    let payload = include_str!("../../../xtask/legion-policy.example.toml")
        .replace("mode_ceiling = \"Assist\"", "mode_ceiling = \"Manual\"");
    app.set_org_policy_bundle(
        legion_security::sign_policy_bundle(&payload, "health-ceiling", &seed)
            .verify(&keyring)
            .unwrap(),
    );
    assert_eq!(
        app.product_mode(),
        AppProductMode::Assist,
        "Manual cannot be claimed over a live request"
    );
    assert_eq!(
        app.ai_provider_profiles()[0].connection_check,
        AiProviderConnectionState::Cancelling
    );
    release.send(()).unwrap();
    peer.join().unwrap();
    // Withhold the app pump after the peer replies: the shared request lane
    // must cover result handoff, not just socket activity. Otherwise another
    // provider request can enter before the pending Manual ceiling is applied.
    let response_deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while !app.ai_provider_profiles()[0]
        .health
        .starts_with("responded")
    {
        assert!(std::time::Instant::now() < response_deadline);
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    let unpumped_until = std::time::Instant::now() + std::time::Duration::from_millis(100);
    while std::time::Instant::now() < unpumped_until {
        assert!(
            app.product_ai_stream_in_flight(),
            "hold the shared lane until policy reconciliation"
        );
        app.set_product_mode(AppProductMode::Manual);
        assert_eq!(app.product_mode(), AppProductMode::Assist);
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while app.ai_provider_connection_check_in_flight() {
        app.poll_ai_provider_connection_check();
        assert!(std::time::Instant::now() < deadline);
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    assert_eq!(app.product_mode(), AppProductMode::Manual);
    assert_eq!(
        app.ai_provider_profiles()[0].connection_check,
        AiProviderConnectionState::Cancelled
    );
}

#[test]
fn connection_check_drain_cannot_authorize_new_work_above_installed_ceiling() {
    use legion_ai::tool_calls::ScriptedToolCallingProviderBuilder;
    use legion_app::{AiProviderConnectionState, AiProviderProfile};
    use legion_protocol::{
        CanonicalPath, DelegatedTaskRiskTolerance, DelegatedTaskScope,
        DelegatedTaskScopeTargetKind, LegionToolKind, LegionWorkflowSessionId,
    };
    use std::io::{Read, Write};

    for retained_mode in [AppProductMode::Delegate, AppProductMode::Automate] {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let profile = AiProviderProfile {
            name: "admission-check".into(),
            provider_id: "llama-cpp".into(),
            endpoint: format!("http://{}/v1", listener.local_addr().unwrap()),
            model: "local-test".into(),
            max_completion_tokens: false,
            disable_thinking: false,
        };
        let (root, mut app, buffer_id) = app_with_file();
        app.configure_ai_provider_profile(profile.clone()).unwrap();
        app.select_ai_provider_profile(&profile.name).unwrap();
        app.set_product_mode(retained_mode);
        let (started, observed) = std::sync::mpsc::channel();
        let (release, released) = std::sync::mpsc::channel();
        let peer = std::thread::spawn(move || {
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
            let (mut stream, _) = loop {
                if let Ok(stream) = listener.accept() {
                    break stream;
                }
                assert!(std::time::Instant::now() < deadline);
                std::thread::sleep(std::time::Duration::from_millis(5));
            };
            stream
                .set_read_timeout(Some(std::time::Duration::from_secs(5)))
                .unwrap();
            let mut request = [0; 4096];
            assert!(stream.read(&mut request).unwrap() > 0);
            started.send(()).unwrap();
            released
                .recv_timeout(std::time::Duration::from_secs(10))
                .unwrap();
            let body = r#"{"choices":[{"message":{"content":"late-check"}}]}"#;
            let _ = write!(
                stream,
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                body.len(),
                body
            );
        });
        app.start_ai_provider_connection_check(&profile).unwrap();
        observed
            .recv_timeout(std::time::Duration::from_secs(5))
            .unwrap();
        let seed = [7u8; 32];
        let keyring =
            legion_security::PolicyKeyring::new(vec![legion_security::PolicySigningKey {
                key_id: "admission-ceiling".into(),
                verifying_key_b64: legion_security::policy_bundle_verifying_key_b64(&seed),
            }]);
        let payload = include_str!("../../../xtask/legion-policy.example.toml")
            .replace("mode_ceiling = \"Assist\"", "mode_ceiling = \"Manual\"");
        app.set_org_policy_bundle(
            legion_security::sign_policy_bundle(&payload, "admission-ceiling", &seed)
                .verify(&keyring)
                .unwrap(),
        );
        assert_eq!(
            app.product_mode(),
            retained_mode,
            "live transport remains honestly visible"
        );
        assert_eq!(
            app.ai_provider_profiles()[0].connection_check,
            AiProviderConnectionState::Cancelling
        );
        let scope = DelegatedTaskScope {
            target_kind: DelegatedTaskScopeTargetKind::Repo,
            workspace_root: CanonicalPath(root.path().to_string_lossy().into_owned()),
            target_path: None,
            risk_tolerance: DelegatedTaskRiskTolerance::Balanced,
            allowed_tools: vec![LegionToolKind::Read],
            forbidden_paths: vec![],
            schema_version: 1,
        };
        let provider = || {
            ScriptedToolCallingProviderBuilder::new()
                .end_turn("must not run")
                .build("denied-admission")
        };
        let admission = app.start_delegated_task_background(
            "must not start".into(),
            scope.clone(),
            Box::new(provider()),
        );
        let was_admitted = admission.is_ok();
        let mut errors = vec![(
            "background Delegate",
            admission.err().map(|e| e.to_string()),
        )];
        if !was_admitted {
            errors.push((
                "synchronous Delegate",
                app.start_delegated_task("must not start".into(), scope, &provider())
                    .err()
                    .map(|e| e.to_string()),
            ));
            errors.push((
                "Delegate chat",
                app.send_delegate_chat("must not send")
                    .err()
                    .map(|e| e.to_string()),
            ));
            errors.push((
                "Assist proposal",
                app.start_ai_proposal("must not send")
                    .err()
                    .map(|e| e.to_string()),
            ));
            errors.push((
                "Assist explain",
                app.start_ai_explain("must not start")
                    .err()
                    .map(|e| e.to_string()),
            ));
            errors.push((
                "inline request",
                app.dispatch_ui_intent(CommandDispatchIntent::RequestAssistInlinePrediction {
                    buffer_id,
                    position: TextCoordinate {
                        line: 0,
                        character: 0,
                        byte_offset: Some(0),
                        utf16_offset: Some(0),
                    },
                })
                .err()
                .map(|e| e.to_string()),
            ));
            errors.push((
                "connection check",
                app.start_ai_provider_connection_check(&profile)
                    .err()
                    .map(|e| e.to_string()),
            ));
            if retained_mode == AppProductMode::Automate {
                errors.push((
                    "Automate execution",
                    app.execute_legion_workflow(&LegionWorkflowSessionId(
                        "unallocated-denied-session".into(),
                    ))
                    .err()
                    .map(|e| e.to_string()),
                ));
                errors.push((
                    "cloud enable",
                    app.enable_legion_cloud_lane_runtime("http://127.0.0.1:1", 1, 1024)
                        .err()
                        .map(|e| e.to_string()),
                ));
            }
            // A tightened admission policy must not remove safe cancellation.
            app.cancel_ai_provider_connection_check(&profile).unwrap();
            app.dispatch_ui_intent(CommandDispatchIntent::CancelAssistInlinePrediction {
                buffer_id,
                prediction_id: None,
            })
            .unwrap();
            app.dispatch_ui_intent(CommandDispatchIntent::DismissAssistInlinePrediction {
                buffer_id,
                prediction_id: None,
            })
            .unwrap();
            assert!(app.product_ai_stream_in_flight());
            assert!(!root.path().join("target/delegated-tasks").exists());
        } else {
            // Keep the intentional red test bounded even if the bug starts a worker.
            let _ = app.cancel_delegated_task();
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
            while app.poll_delegated_task().unwrap().is_none() {
                assert!(std::time::Instant::now() < deadline);
                std::thread::sleep(std::time::Duration::from_millis(5));
            }
        }
        release.send(()).unwrap();
        peer.join().unwrap();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while app.ai_provider_connection_check_in_flight() {
            app.poll_ai_provider_connection_check();
            assert!(std::time::Instant::now() < deadline);
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        for (operation, error) in errors {
            assert!(
                error
                    .as_deref()
                    .is_some_and(|e| e.contains("installed organization mode ceiling")),
                "{retained_mode:?}: {operation} must be denied at policy admission, got {error:?}"
            );
        }
        assert_eq!(app.product_mode(), AppProductMode::Manual);
        assert_eq!(
            app.ai_provider_profiles()[0].connection_check,
            AiProviderConnectionState::Cancelled
        );
        assert_eq!(app.editor().text(buffer_id).unwrap(), "fn main() {}\n");
        assert_eq!(
            std::fs::read_to_string(root.path().join("main.rs")).unwrap(),
            "fn main() {}\n"
        );
    }
}

#[test]
fn explicit_unavailable_provider_refuses_instead_of_returning_fixture_prediction() {
    if std::env::var("LEGION_LLAMA_CPP_BASE_URL").as_deref() != Ok("http://127.0.0.1:1/v1") {
        let status = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "explicit_unavailable_provider_refuses_instead_of_returning_fixture_prediction",
            ])
            .env("LEGION_LLAMA_CPP_BASE_URL", "http://127.0.0.1:1/v1")
            .status()
            .unwrap();
        assert!(status.success());
        return;
    }
    // This isolated binary runs with LEGION_LLAMA_CPP_BASE_URL pinned to a closed loopback port.
    assert_eq!(
        std::env::var("LEGION_LLAMA_CPP_BASE_URL").unwrap(),
        "http://127.0.0.1:1/v1"
    );
    let (_root, mut app, buffer_id) = app_with_file();
    app.set_product_mode(AppProductMode::Assist);
    app.set_preferred_ai_provider(ProductAiProviderPreference::LlamaCpp);
    let result = app.dispatch_ui_intent(CommandDispatchIntent::RequestAssistInlinePrediction {
        buffer_id,
        position: TextCoordinate {
            line: 0,
            character: 12,
            byte_offset: Some(12),
            utf16_offset: Some(12),
        },
    });
    assert!(
        matches!(result, Err(AppCompositionError::AiRuntime(ref reason)) if reason.contains("unavailable")),
        "selected provider must refuse; got {result:?}"
    );
    assert_eq!(app.editor().text(buffer_id).unwrap(), "fn main() {}\n");
    assert!(
        app.shell_projection_snapshot("after")
            .unwrap()
            .assist_inline_prediction_projection
            .active_prediction
            .is_none()
    );
}

#[test]
fn named_profile_round_trips_without_credentials_and_rejects_secret_endpoints_atomically() {
    use legion_app::AiProviderProfile;
    use legion_storage::InMemorySecretStore;
    let store = std::sync::Arc::new(InMemorySecretStore::default());
    let mut app = AppComposition::with_provider_secret_store(store.clone());
    let profile = AiProviderProfile {
        name: "pilot".into(),
        provider_id: "llama-cpp".into(),
        endpoint: "http://127.0.0.1:9001/v1".into(),
        model: "pilot-model".into(),
        max_completion_tokens: false,
        disable_thinking: false,
    };
    app.configure_ai_provider_profile(profile.clone()).unwrap();
    app.select_ai_provider_profile("pilot").unwrap();
    app.replace_ai_profile_credential("pilot", "synthetic-test-credential")
        .unwrap();
    let config = app.ai_provider_configuration_json().unwrap();
    assert!(!config.contains("synthetic-test-credential"));
    let mut restored = AppComposition::with_provider_secret_store(store);
    restored
        .restore_ai_provider_configuration_json(&config)
        .unwrap();
    let projection = restored.ai_provider_profiles();
    assert_eq!(projection.len(), 1);
    assert_eq!(projection[0].profile, profile);
    assert!(projection[0].selected);
    assert_eq!(projection[0].locality, "loopback");
    assert_eq!(projection[0].credential_state, "stored");
    assert_eq!(projection[0].health, "not checked");
    assert!(
        projection[0]
            .capabilities
            .contains(&"chat completion".to_string())
    );
    for endpoint in [
        "https://user:secret@example.com/v1",
        "https://example.com?api_key=secret",
        "http://example.com/v1",
        "https:///v1",
        "ftp://example.com",
        "http://127.0.0.1:bad/v1",
    ] {
        let mut invalid = profile.clone();
        invalid.endpoint = endpoint.into();
        assert!(restored.configure_ai_provider_profile(invalid).is_err());
        assert_eq!(restored.ai_provider_configuration_json().unwrap(), config);
    }
    restored.revoke_ai_profile_credential("pilot").unwrap();
    assert_eq!(
        restored.ai_provider_profiles()[0].credential_state,
        "missing"
    );
    assert_eq!(restored.product_mode(), AppProductMode::Manual);
}

#[test]
fn selected_profile_sends_exact_endpoint_and_model_to_protocol_peer() {
    use std::io::{Read, Write};
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let endpoint = format!("http://{}/v1", listener.local_addr().unwrap());
    let (sent, received) = std::sync::mpsc::channel();
    let peer = std::thread::spawn(move || {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while std::time::Instant::now() < deadline {
            if let Ok((mut stream, _)) = listener.accept() {
                stream
                    .set_read_timeout(Some(std::time::Duration::from_secs(2)))
                    .unwrap();
                let mut bytes = Vec::new();
                loop {
                    let mut chunk = [0; 4096];
                    let n = stream.read(&mut chunk).unwrap();
                    bytes.extend_from_slice(&chunk[..n]);
                    if let Some(end) = bytes.windows(4).position(|w| w == b"\r\n\r\n") {
                        let headers = String::from_utf8_lossy(&bytes[..end]).to_ascii_lowercase();
                        let size: usize = headers
                            .lines()
                            .find_map(|l| l.strip_prefix("content-length: "))
                            .unwrap()
                            .parse()
                            .unwrap();
                        if bytes.len() >= end + 4 + size {
                            break;
                        }
                    }
                    assert!(n > 0);
                }
                sent.send(String::from_utf8(bytes).unwrap()).unwrap();
                let body = r#"{"model":"pilot-model","choices":[{"message":{"role":"assistant","content":"profile_peer"}}]}"#;
                write!(stream, "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}", body.len(), body).unwrap();
                return;
            }
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
    });
    let (_root, mut app, buffer_id) = app_with_file();
    app.configure_ai_provider_profile(legion_app::AiProviderProfile {
        name: "pilot".into(),
        provider_id: "llama-cpp".into(),
        endpoint,
        model: "pilot-model".into(),
        max_completion_tokens: false,
        disable_thinking: false,
    })
    .unwrap();
    app.select_ai_provider_profile("pilot").unwrap();
    app.set_product_mode(AppProductMode::Assist);
    app.dispatch_ui_intent(CommandDispatchIntent::RequestAssistInlinePrediction {
        buffer_id,
        position: TextCoordinate {
            line: 0,
            character: 12,
            byte_offset: Some(12),
            utf16_offset: Some(12),
        },
    })
    .unwrap();
    let request = received
        .recv_timeout(std::time::Duration::from_secs(6))
        .expect("selected endpoint receives request");
    assert!(request.starts_with("POST /v1/chat/completions HTTP/1.1\r\n"));
    let body: serde_json::Value =
        serde_json::from_str(request.split_once("\r\n\r\n").unwrap().1).unwrap();
    assert_eq!(body["model"], "pilot-model");
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while app.product_ai_stream_in_flight() && std::time::Instant::now() < deadline {
        app.poll_product_ai_stream();
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    app.poll_product_ai_stream();
    let result = app
        .shell_projection_snapshot("peer")
        .unwrap()
        .assist_inline_prediction_projection;
    assert_eq!(
        result.active_prediction.unwrap().ghost_text_label,
        "profile_peer"
    );
    assert_eq!(app.editor().text(buffer_id).unwrap(), "fn main() {}\n");
    peer.join().unwrap();
}

#[test]
fn token_plan_profile_emits_bounded_completion_tokens_and_explicit_disabled_thinking() {
    use std::io::{Read, Write};
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let endpoint = format!("http://{}/v1", listener.local_addr().unwrap());
    let (sent, received) = std::sync::mpsc::channel();
    let peer = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        stream
            .set_read_timeout(Some(std::time::Duration::from_secs(3)))
            .unwrap();
        let mut bytes = Vec::new();
        loop {
            let mut chunk = [0; 4096];
            let n = stream.read(&mut chunk).unwrap();
            bytes.extend_from_slice(&chunk[..n]);
            if let Some(end) = bytes.windows(4).position(|w| w == b"\r\n\r\n") {
                let headers = String::from_utf8_lossy(&bytes[..end]).to_ascii_lowercase();
                let size: usize = headers
                    .lines()
                    .find_map(|l| l.strip_prefix("content-length: "))
                    .unwrap()
                    .parse()
                    .unwrap();
                if bytes.len() >= end + 4 + size {
                    break;
                }
            }
            assert!(n > 0);
        }
        sent.send(String::from_utf8(bytes).unwrap()).unwrap();
        let body = r#"{"model":"mimo-v2.6-pro","choices":[{"message":{"role":"assistant","content":"bounded_peer"}}]}"#;
        write!(stream, "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}", body.len(), body).unwrap();
    });
    let (_root, mut app, buffer_id) = app_with_file();
    app.configure_ai_provider_profile(legion_app::AiProviderProfile {
        name: "mimo-sgp".into(),
        provider_id: "openai-compatible".into(),
        endpoint,
        model: "mimo-v2.6-pro".into(),
        max_completion_tokens: true,
        disable_thinking: true,
    })
    .unwrap();
    app.select_ai_provider_profile("mimo-sgp").unwrap();
    app.replace_ai_profile_credential("mimo-sgp", "synthetic-peer-key")
        .unwrap();
    app.set_product_mode(AppProductMode::Assist);
    app.dispatch_ui_intent(CommandDispatchIntent::RequestAssistInlinePrediction {
        buffer_id,
        position: TextCoordinate {
            line: 0,
            character: 12,
            byte_offset: Some(12),
            utf16_offset: Some(12),
        },
    })
    .unwrap();
    let request = received
        .recv_timeout(std::time::Duration::from_secs(5))
        .expect("wire request");
    assert!(request.starts_with("POST /v1/chat/completions HTTP/1.1\r\n"));
    assert!(
        request
            .to_ascii_lowercase()
            .contains("authorization: bearer synthetic-peer-key\r\n")
    );
    let body: serde_json::Value =
        serde_json::from_str(request.split_once("\r\n\r\n").unwrap().1).unwrap();
    assert_eq!(body["model"], "mimo-v2.6-pro");
    assert_eq!(body["max_completion_tokens"], 128);
    assert!(body.get("max_tokens").is_none());
    assert_eq!(body["thinking"]["type"], "disabled");
    assert!(body.get("reasoning_content").is_none());
    peer.join().unwrap();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
    while app.product_ai_stream_in_flight() && std::time::Instant::now() < deadline {
        app.poll_product_ai_stream();
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    app.poll_product_ai_stream();
    assert!(
        !app.ai_provider_configuration_json()
            .unwrap()
            .contains("synthetic-peer-key")
    );
}

#[test]
fn selected_profile_http_refusal_finishes_without_fixture_or_editor_mutation() {
    use std::io::{Read, Write};
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let endpoint = format!("http://{}/v1", listener.local_addr().unwrap());
    let peer = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        stream
            .set_read_timeout(Some(std::time::Duration::from_secs(3)))
            .unwrap();
        let mut bytes = [0; 4096];
        stream.read(&mut bytes).unwrap();
        stream
            .write_all(
                b"HTTP/1.1 429 Too Many Requests\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
            )
            .unwrap();
    });
    let (_root, mut app, buffer_id) = app_with_file();
    app.configure_ai_provider_profile(legion_app::AiProviderProfile {
        name: "quota".into(),
        provider_id: "llama-cpp".into(),
        endpoint,
        model: "model".into(),
        max_completion_tokens: false,
        disable_thinking: false,
    })
    .unwrap();
    app.select_ai_provider_profile("quota").unwrap();
    app.set_product_mode(AppProductMode::Assist);
    app.dispatch_ui_intent(CommandDispatchIntent::RequestAssistInlinePrediction {
        buffer_id,
        position: TextCoordinate {
            line: 0,
            character: 12,
            byte_offset: Some(12),
            utf16_offset: Some(12),
        },
    })
    .unwrap();
    peer.join().unwrap();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
    while app.product_ai_stream_in_flight() && std::time::Instant::now() < deadline {
        app.poll_product_ai_stream();
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    app.poll_product_ai_stream();
    let result = app
        .shell_projection_snapshot("refused")
        .unwrap()
        .assist_inline_prediction_projection;
    assert!(
        result.active_prediction.is_none(),
        "refusal must not become fixture ghost text"
    );
    assert!(!result.request_in_flight);
    assert_eq!(app.editor().text(buffer_id).unwrap(), "fn main() {}\n");
    assert!(
        app.ai_provider_profiles()[0]
            .health
            .starts_with("unavailable")
    );
}

#[test]
fn cancelled_request_keeps_credentials_and_manual_transition_blocked_until_worker_drains() {
    use std::io::{Read, Write};
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let endpoint = format!("http://{}/v1", listener.local_addr().unwrap());
    let (started, observed) = std::sync::mpsc::channel();
    let (release, released) = std::sync::mpsc::channel();
    let peer = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        stream
            .set_read_timeout(Some(std::time::Duration::from_secs(3)))
            .unwrap();
        let mut bytes = [0; 4096];
        stream.read(&mut bytes).unwrap();
        started.send(()).unwrap();
        released
            .recv_timeout(std::time::Duration::from_secs(3))
            .unwrap();
        let body = r#"{"choices":[{"message":{"content":"late_peer"}}]}"#;
        write!(stream, "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}", body.len(), body).unwrap();
    });
    let (_root, mut app, buffer_id) = app_with_file();
    app.configure_ai_provider_profile(legion_app::AiProviderProfile {
        name: "cancel".into(),
        provider_id: "llama-cpp".into(),
        endpoint,
        model: "model".into(),
        max_completion_tokens: false,
        disable_thinking: false,
    })
    .unwrap();
    app.select_ai_provider_profile("cancel").unwrap();
    app.replace_ai_profile_credential("cancel", "synthetic-peer-key")
        .unwrap();
    app.set_product_mode(AppProductMode::Assist);
    app.dispatch_ui_intent(CommandDispatchIntent::RequestAssistInlinePrediction {
        buffer_id,
        position: TextCoordinate {
            line: 0,
            character: 12,
            byte_offset: Some(12),
            utf16_offset: Some(12),
        },
    })
    .unwrap();
    observed
        .recv_timeout(std::time::Duration::from_secs(3))
        .unwrap();
    app.dispatch_ui_intent(CommandDispatchIntent::CancelAssistInlinePrediction {
        buffer_id,
        prediction_id: None,
    })
    .unwrap();
    assert!(
        app.revoke_ai_profile_credential("cancel").is_err(),
        "revocation cannot claim old worker is terminated"
    );
    app.set_product_mode(AppProductMode::Manual);
    assert_eq!(app.product_mode(), AppProductMode::Assist);
    release.send(()).unwrap();
    peer.join().unwrap();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
    while app.revoke_ai_profile_credential("cancel").is_err()
        && std::time::Instant::now() < deadline
    {
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    app.revoke_ai_profile_credential("cancel").unwrap();
    app.poll_product_ai_stream();
    app.set_product_mode(AppProductMode::Manual);
    assert_eq!(app.product_mode(), AppProductMode::Manual);
    assert!(
        app.shell_projection_snapshot("cancel")
            .unwrap()
            .assist_inline_prediction_projection
            .active_prediction
            .is_none()
    );
}

#[test]
fn selected_profile_redirect_cannot_send_prompt_or_credential_to_another_endpoint() {
    use std::io::{Read, Write};
    let destination = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    destination.set_nonblocking(true).unwrap();
    let location = format!(
        "http://{}/v1/chat/completions",
        destination.local_addr().unwrap()
    );
    let (sent, received) = std::sync::mpsc::channel();
    let other = std::thread::spawn(move || {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
        while std::time::Instant::now() < deadline {
            if let Ok((mut stream, _)) = destination.accept() {
                stream.set_nonblocking(false).unwrap();
                stream
                    .set_read_timeout(Some(std::time::Duration::from_secs(3)))
                    .unwrap();
                sent.send(()).unwrap();
                let mut bytes = [0; 4096];
                stream.read(&mut bytes).unwrap();
                stream
                    .write_all(
                        b"HTTP/1.1 403 Forbidden\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
                    )
                    .unwrap();
                return;
            }
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
    });
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let endpoint = format!("http://{}/v1", listener.local_addr().unwrap());
    let peer = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        stream
            .set_read_timeout(Some(std::time::Duration::from_secs(3)))
            .unwrap();
        let mut bytes = [0; 4096];
        stream.read(&mut bytes).unwrap();
        write!(stream, "HTTP/1.1 307 Temporary Redirect\r\nLocation: {location}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n").unwrap();
    });
    let (_root, mut app, buffer_id) = app_with_file();
    app.configure_ai_provider_profile(legion_app::AiProviderProfile {
        name: "redirect".into(),
        provider_id: "openai-compatible".into(),
        endpoint,
        model: "model".into(),
        max_completion_tokens: true,
        disable_thinking: true,
    })
    .unwrap();
    app.select_ai_provider_profile("redirect").unwrap();
    app.replace_ai_profile_credential("redirect", "synthetic-peer-key")
        .unwrap();
    app.set_product_mode(AppProductMode::Assist);
    app.dispatch_ui_intent(CommandDispatchIntent::RequestAssistInlinePrediction {
        buffer_id,
        position: TextCoordinate {
            line: 0,
            character: 12,
            byte_offset: Some(12),
            utf16_offset: Some(12),
        },
    })
    .unwrap();
    peer.join().unwrap();
    other.join().unwrap();
    assert!(
        received.try_recv().is_err(),
        "redirect must not cross the authorized endpoint"
    );
    app.poll_product_ai_stream();
}

#[test]
fn noncanonical_numeric_endpoints_cannot_replace_a_valid_provider_profile() {
    let mut app = AppComposition::with_provider_secret_store(std::sync::Arc::new(
        legion_storage::InMemorySecretStore::default(),
    ));
    let profile = legion_app::AiProviderProfile {
        name: "canonical".into(),
        provider_id: "openai-compatible".into(),
        endpoint: "https://8.8.8.8/v1".into(),
        model: "model".into(),
        max_completion_tokens: true,
        disable_thinking: true,
    };
    app.configure_ai_provider_profile(profile.clone()).unwrap();
    let before = app.ai_provider_configuration_json().unwrap();
    for endpoint in [
        "https://0x08080808/v1",
        "https://134744072/v1",
        "https://010.010.010.010/v1",
        "https://8.8.2056/v1",
        "https://[0:0:0:0:0:0:0:1]/v1",
        "https://8.8.8.8./v1",
    ] {
        let mut changed = profile.clone();
        changed.endpoint = endpoint.into();
        assert!(
            app.configure_ai_provider_profile(changed).is_err(),
            "noncanonical route accepted: {endpoint}"
        );
        assert_eq!(app.ai_provider_configuration_json().unwrap(), before);
    }
}

#[test]
fn selected_profile_edit_invalidates_completed_ghost_and_rejects_old_acceptance() {
    use std::io::{Read, Write};
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let endpoint = format!("http://{}/v1", listener.local_addr().unwrap());
    let peer = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        stream
            .set_read_timeout(Some(std::time::Duration::from_secs(3)))
            .unwrap();
        let mut bytes = [0; 4096];
        stream.read(&mut bytes).unwrap();
        let body = r#"{"choices":[{"message":{"content":"old_profile_ghost"}}]}"#;
        write!(stream, "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}", body.len(), body).unwrap();
    });
    let (_root, mut app, buffer_id) = app_with_file();
    let profile = legion_app::AiProviderProfile {
        name: "revision".into(),
        provider_id: "llama-cpp".into(),
        endpoint,
        model: "old-model".into(),
        max_completion_tokens: false,
        disable_thinking: false,
    };
    app.configure_ai_provider_profile(profile.clone()).unwrap();
    app.select_ai_provider_profile("revision").unwrap();
    app.set_product_mode(AppProductMode::Assist);
    app.dispatch_ui_intent(CommandDispatchIntent::RequestAssistInlinePrediction {
        buffer_id,
        position: TextCoordinate {
            line: 0,
            character: 12,
            byte_offset: Some(12),
            utf16_offset: Some(12),
        },
    })
    .unwrap();
    peer.join().unwrap();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
    while app.provider_configuration_busy() && std::time::Instant::now() < deadline {
        app.poll_product_ai_stream();
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    app.poll_product_ai_stream();
    let ghost = app
        .shell_projection_snapshot("completed")
        .unwrap()
        .assist_inline_prediction_projection
        .active_prediction
        .unwrap();
    assert_eq!(ghost.ghost_text_label, "old_profile_ghost");
    // Re-saving identical metadata preserves the authorized prediction.
    app.configure_ai_provider_profile(profile.clone()).unwrap();
    assert!(
        app.shell_projection_snapshot("unchanged")
            .unwrap()
            .assist_inline_prediction_projection
            .active_prediction
            .is_some()
    );
    let mut changed = profile.clone();
    changed.model = "new-model".into();
    changed.endpoint = "http://127.0.0.1:1/v1".into();
    app.configure_ai_provider_profile(changed).unwrap();
    assert!(
        app.shell_projection_snapshot("edited")
            .unwrap()
            .assist_inline_prediction_projection
            .active_prediction
            .is_none(),
        "selected profile edit must invalidate completed ghost text"
    );
    // Returning to identical metadata must not resurrect an older authorization.
    app.configure_ai_provider_profile(profile).unwrap();
    assert!(
        app.dispatch_ui_intent(CommandDispatchIntent::AcceptAssistInlinePrediction {
            buffer_id,
            prediction_id: Some(ghost.prediction_id),
        })
        .is_err()
    );
    assert_eq!(app.editor().text(buffer_id).unwrap(), "fn main() {}\n");
    assert_eq!(app.editor().undo_len(buffer_id).unwrap(), 0);
}
