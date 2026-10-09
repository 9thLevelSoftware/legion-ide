#![cfg(feature = "ai")]

use legion_app::named_mcp_peer::NamedMcpPeerConfig;
use legion_app::named_mcp_peer::named_mcp_peer_secret_reference;
use legion_desktop::bridge::DesktopAction;
use legion_desktop::bridge::SensitiveString;
use legion_desktop::view::mcp_settings::{McpHttpPeerForm, McpSettingsOperation};
use legion_desktop::workflow::{DesktopEframeApp, DesktopLaunchConfig, DesktopRuntime};
use legion_protocol::{McpServerId, named_mcp_peer::*};
use legion_storage::{InMemorySecretStore, SecretStore};
use legion_storage::{SecretReference, SecretStoreError};
use std::sync::Arc;
mod common;
use common::{click_at, clickable_center, full_frame_input, rendered_text};

fn runtime(root: &std::path::Path, store: Arc<dyn SecretStore + Send + Sync>) -> DesktopRuntime {
    DesktopRuntime::open_with_provider_secret_store(
        DesktopLaunchConfig::new(root.to_path_buf(), None)
            .with_session_state(root.join("session.json")),
        store,
    )
    .unwrap()
}

struct LocalPeer {
    endpoint: String,
    exchanges: Arc<std::sync::Mutex<Vec<(String, String, String)>>>,
    stop: Arc<std::sync::atomic::AtomicBool>,
    thread: Option<std::thread::JoinHandle<()>>,
    release: Arc<std::sync::atomic::AtomicBool>,
    held_request: Arc<std::sync::atomic::AtomicBool>,
}

impl LocalPeer {
    fn start() -> Self {
        Self::with_pause(None)
    }

    fn with_pause(pause_method: Option<&'static str>) -> Self {
        use std::io::{BufRead, Read, Write};
        use std::sync::atomic::{AtomicBool, Ordering};
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let endpoint = format!("http://{}/mcp", listener.local_addr().unwrap());
        let exchanges = Arc::new(std::sync::Mutex::new(Vec::new()));
        let stop = Arc::new(AtomicBool::new(false));
        let rows = exchanges.clone();
        let stopping = stop.clone();
        let release = Arc::new(AtomicBool::new(pause_method.is_none()));
        let held_request = Arc::new(AtomicBool::new(false));
        let releasing = release.clone();
        let held = held_request.clone();
        let thread = std::thread::spawn(move || {
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(20);
            while !stopping.load(Ordering::SeqCst) && std::time::Instant::now() < deadline {
                let mut stream = match listener.accept() {
                    Ok((s, _)) => s,
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        std::thread::sleep(std::time::Duration::from_millis(2));
                        continue;
                    }
                    Err(e) => panic!("local peer: {e}"),
                };
                stream.set_nonblocking(false).unwrap();
                stream
                    .set_read_timeout(Some(std::time::Duration::from_secs(5)))
                    .unwrap();
                let mut reader = std::io::BufReader::new(stream.try_clone().unwrap());
                let mut line = String::new();
                reader.read_line(&mut line).unwrap();
                assert_eq!(line.trim(), "POST /mcp HTTP/1.1");
                let mut length = 0;
                let mut auth = String::new();
                let mut protocol = String::new();
                loop {
                    line.clear();
                    reader.read_line(&mut line).unwrap();
                    if line == "\r\n" {
                        break;
                    }
                    let (key, value) = line.split_once(':').unwrap();
                    match key.to_ascii_lowercase().as_str() {
                        "content-length" => length = value.trim().parse::<usize>().unwrap(),
                        "authorization" => auth = value.trim().to_owned(),
                        "mcp-protocol-version" => protocol = value.trim().to_owned(),
                        _ => {}
                    }
                }
                let mut body = vec![0; length];
                reader.read_exact(&mut body).unwrap();
                let request: serde_json::Value = serde_json::from_slice(&body).unwrap();
                let method = request["method"].as_str().unwrap();
                let result = match method {
                    "initialize" => {
                        serde_json::json!({"protocolVersion":"2025-11-25", "capabilities":{"tools":{}}, "serverInfo":{"name":"local-oracle","version":"1"}})
                    }
                    "notifications/initialized" | "ping" => serde_json::json!({}),
                    "tools/list" => serde_json::json!({"tools":[]}),
                    other => panic!("unexpected effect: {other}"),
                };
                rows.lock().unwrap().push((method.into(), auth, protocol));
                if pause_method == Some(method) {
                    held.store(true, Ordering::SeqCst);
                    let until = std::time::Instant::now() + std::time::Duration::from_secs(3);
                    while !releasing.load(Ordering::SeqCst)
                        && !stopping.load(Ordering::SeqCst)
                        && std::time::Instant::now() < until
                    {
                        std::thread::sleep(std::time::Duration::from_millis(2));
                    }
                }
                if request["id"].is_null() {
                    write!(
                        stream,
                        "HTTP/1.1 204 No Content\r\nConnection: close\r\nContent-Length: 0\r\n\r\n"
                    )
                    .unwrap();
                } else {
                    let response =
                        serde_json::json!({"jsonrpc":"2.0","id":request["id"],"result":result})
                            .to_string();
                    write!(stream, "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nConnection: close\r\nContent-Length: {}\r\n\r\n{}", response.len(), response).unwrap();
                }
            }
        });
        Self {
            endpoint,
            exchanges,
            stop,
            thread: Some(thread),
            release,
            held_request,
        }
    }
}

#[test]
fn slow_peer_connect_does_not_block_input_and_revocation_rejects_late_completion() {
    use legion_desktop::view::mcp_settings::McpSettingsOperation;
    use std::sync::atomic::Ordering::SeqCst;
    let root = tempfile::tempdir().unwrap();
    let peer = LocalPeer::with_pause(Some("initialize"));
    let id = McpServerId("slow-connect".into());
    let mut runtime = runtime(root.path(), Arc::new(InMemorySecretStore::default()));
    runtime
        .app_mut_for_test()
        .configure_named_mcp_peer(NamedMcpPeerConfig::http_client(
            id.clone(),
            "Slow oracle",
            &peer.endpoint,
        ))
        .unwrap();
    runtime
        .handle_action(DesktopAction::SelectNamedMcpPeer {
            peer_id: id.clone(),
        })
        .unwrap();
    runtime
        .handle_action(DesktopAction::ManageNamedMcpPeer {
            peer_id: id.clone(),
            revision: 1,
            operation: McpSettingsOperation::GrantTransport,
        })
        .unwrap();
    runtime
        .handle_action(DesktopAction::SetProductMode {
            mode: legion_ui::DockMode::Assist,
        })
        .unwrap();
    let mut app = DesktopEframeApp::new(runtime);
    let frame = page(&mut app);
    let release = peer.release.clone();
    let release_thread = std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_secs(1));
        release.store(true, SeqCst);
    });
    let started = std::time::Instant::now();
    let frame = click_at(&mut app, require(&frame, "Connect selected peer"));
    let elapsed = started.elapsed();
    assert!(
        elapsed < std::time::Duration::from_millis(300),
        "native input blocked for {elapsed:?}"
    );
    assert!(
        rendered_text(&frame)
            .join("\n")
            .contains("MCP operation pending")
    );
    let until = std::time::Instant::now() + std::time::Duration::from_millis(500);
    while !peer.held_request.load(SeqCst) && std::time::Instant::now() < until {
        app.run_headless_full_frame(full_frame_input(Vec::new()));
        std::thread::sleep(std::time::Duration::from_millis(2));
    }
    assert!(peer.held_request.load(SeqCst));
    app.runtime_mut_for_test()
        .handle_action(DesktopAction::ManageNamedMcpPeer {
            peer_id: id.clone(),
            revision: 1,
            operation: McpSettingsOperation::RevokeGrant,
        })
        .unwrap();
    release_thread.join().unwrap();
    let until = std::time::Instant::now() + std::time::Duration::from_millis(500);
    while std::time::Instant::now() < until {
        app.run_headless_full_frame(full_frame_input(Vec::new()));
        std::thread::sleep(std::time::Duration::from_millis(2));
    }
    let snapshot = app
        .runtime_mut_for_test()
        .app_mut_for_test()
        .inspect_named_mcp_peer(&id)
        .unwrap();
    assert_eq!(snapshot.health, McpPeerHealth::Revoked);
    assert!(!snapshot.transport_granted);
    assert_eq!(
        peer.exchanges
            .lock()
            .unwrap()
            .iter()
            .map(|r| r.0.as_str())
            .collect::<Vec<_>>(),
        ["initialize"]
    );
}

impl Drop for LocalPeer {
    fn drop(&mut self) {
        self.stop.store(true, std::sync::atomic::Ordering::SeqCst);
        let result = self.thread.take().unwrap().join();
        if !std::thread::panicking() {
            result.unwrap();
        }
    }
}

#[test]
fn slow_health_probe_keeps_mode_honest_until_transport_drains_and_rejects_late_ready() {
    use std::sync::atomic::Ordering::SeqCst;
    let root = tempfile::tempdir().unwrap();
    let peer = LocalPeer::with_pause(Some("ping"));
    peer.release.store(true, SeqCst);
    let id = McpServerId("slow-health".into());
    let mut runtime = runtime(root.path(), Arc::new(InMemorySecretStore::default()));
    runtime
        .app_mut_for_test()
        .configure_named_mcp_peer(NamedMcpPeerConfig::http_client(
            id.clone(),
            "Slow health",
            &peer.endpoint,
        ))
        .unwrap();
    runtime
        .handle_action(DesktopAction::SelectNamedMcpPeer {
            peer_id: id.clone(),
        })
        .unwrap();
    runtime
        .handle_action(DesktopAction::ManageNamedMcpPeer {
            peer_id: id.clone(),
            revision: 1,
            operation: McpSettingsOperation::GrantTransport,
        })
        .unwrap();
    runtime
        .handle_action(DesktopAction::SetProductMode {
            mode: legion_ui::DockMode::Assist,
        })
        .unwrap();
    let mut app = DesktopEframeApp::new(runtime);
    let frame = page(&mut app);
    click_at(&mut app, require(&frame, "Connect selected peer"));
    let frame = wait_idle(&mut app, &id);
    assert!(rendered_text(&frame).join("\n").contains("Ready"));
    peer.release.store(false, SeqCst);
    peer.held_request.store(false, SeqCst);
    let release = peer.release.clone();
    let release_thread = std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_secs(1));
        release.store(true, SeqCst);
    });
    let started = std::time::Instant::now();
    let frame = click_at(&mut app, require(&frame, "Probe selected peer health"));
    assert!(started.elapsed() < std::time::Duration::from_millis(300));
    assert!(common::enabled_clickable_center(&frame, "Connect selected peer").is_none());
    assert!(common::enabled_clickable_center(&frame, "Probe selected peer health").is_none());
    let until = std::time::Instant::now() + std::time::Duration::from_millis(500);
    while !peer.held_request.load(SeqCst) && std::time::Instant::now() < until {
        app.run_headless_full_frame(full_frame_input(Vec::new()));
        std::thread::sleep(std::time::Duration::from_millis(2));
    }
    assert!(peer.held_request.load(SeqCst));
    assert_eq!(
        app.runtime_mut_for_test()
            .app_mut_for_test()
            .probe_named_mcp_peer(&id, 1),
        Err(legion_app::named_mcp_peer::NamedMcpPeerError::OperationBusy)
    );
    app.runtime_mut_for_test()
        .handle_action(DesktopAction::SetProductMode {
            mode: legion_ui::DockMode::Manual,
        })
        .unwrap();
    assert_eq!(
        app.runtime_mut_for_test().app_mut_for_test().product_mode(),
        legion_app::AppProductMode::Assist
    );
    assert!(
        app.runtime_mut_for_test()
            .app_mut_for_test()
            .provider_configuration_busy()
    );
    release_thread.join().unwrap();
    wait_idle(&mut app, &id);
    let snapshot = app
        .runtime_mut_for_test()
        .app_mut_for_test()
        .inspect_named_mcp_peer(&id)
        .unwrap();
    assert_eq!(snapshot.health, McpPeerHealth::Revoked);
    assert!(!snapshot.transport_granted);
    assert_eq!(peer.exchanges.lock().unwrap().len(), 5);
    app.runtime_mut_for_test()
        .handle_action(DesktopAction::SetProductMode {
            mode: legion_ui::DockMode::Manual,
        })
        .unwrap();
    assert_eq!(
        app.runtime_mut_for_test().app_mut_for_test().product_mode(),
        legion_app::AppProductMode::Manual
    );
}

#[test]
fn selected_peer_requires_grant_and_nonmanual_mode_before_protocol_health_and_revocation() {
    let root = tempfile::tempdir().unwrap();
    let peer = LocalPeer::start();
    let id = McpServerId("local-health".into());
    let mut runtime = runtime(root.path(), Arc::new(InMemorySecretStore::default()));
    runtime
        .app_mut_for_test()
        .configure_named_mcp_peer(NamedMcpPeerConfig::http_client(
            id.clone(),
            "Health oracle",
            &peer.endpoint,
        ))
        .unwrap();
    runtime.save_session_state().unwrap();
    let mut app = DesktopEframeApp::new(runtime);
    let frame = page(&mut app);
    let frame = click_at(&mut app, require(&frame, "Select peer"));
    assert!(common::enabled_clickable_center(&frame, "Connect selected peer").is_none());
    let frame = click_at(&mut app, require(&frame, "Grant selected HTTP route"));
    assert!(common::enabled_clickable_center(&frame, "Connect selected peer").is_none());
    assert!(peer.exchanges.lock().unwrap().is_empty());
    app.runtime_mut_for_test()
        .handle_action(DesktopAction::SetProductMode {
            mode: legion_ui::DockMode::Assist,
        })
        .unwrap();
    let frame = app.run_headless_full_frame(full_frame_input(Vec::new()));
    click_at(&mut app, require(&frame, "Connect selected peer"));
    let frame = wait_idle(&mut app, &id);
    assert!(rendered_text(&frame).join("\n").contains("Ready"));
    click_at(&mut app, require(&frame, "Probe selected peer health"));
    let frame = wait_idle(&mut app, &id);
    let rows = peer.exchanges.lock().unwrap().clone();
    assert_eq!(
        rows.iter().map(|r| r.0.as_str()).collect::<Vec<_>>(),
        [
            "initialize",
            "notifications/initialized",
            "tools/list",
            "ping",
            "ping"
        ]
    );
    assert!(rows.iter().all(|r| r.1.is_empty() && r.2 == "2025-11-25"));
    assert!(
        app.runtime_mut_for_test()
            .app_mut_for_test()
            .ai_provider_profiles()
            .is_empty()
    );
    assert!(!app.runtime_mut_for_test().product_ai_stream_in_flight());
    let frame = click_at(&mut app, require(&frame, "Revoke local MCP grant"));
    assert!(rendered_text(&frame).join("\n").contains("Revoked"));
    let snapshot = app
        .runtime_mut_for_test()
        .app_mut_for_test()
        .inspect_named_mcp_peer(&id)
        .unwrap();
    assert!(!snapshot.transport_granted);
    assert!(common::enabled_clickable_center(&frame, "Probe selected peer health").is_none());
    assert_eq!(peer.exchanges.lock().unwrap().len(), 5);
}

#[test]
fn masked_peer_credential_is_bound_to_reviewed_route_used_only_after_grant_and_deleted_on_revoke() {
    let root = tempfile::tempdir().unwrap();
    let peer = LocalPeer::start();
    let store = Arc::new(InMemorySecretStore::default());
    let id = McpServerId("credential-oracle".into());
    let mut config =
        NamedMcpPeerConfig::http_client(id.clone(), "Credential oracle", &peer.endpoint);
    config.metadata.authentication = McpPeerAuthentication::Bearer;
    config.metadata.credential_scopes = vec!["tools:read".into()];
    let reference = named_mcp_peer_secret_reference(&config);
    let mut runtime = runtime(root.path(), store.clone());
    runtime
        .app_mut_for_test()
        .configure_named_mcp_peer(config)
        .unwrap();
    runtime.save_session_state().unwrap();
    let mut app = DesktopEframeApp::new(runtime);
    let frame = page(&mut app);
    click_at(&mut app, require(&frame, "Select peer"));
    let frame = fill(&mut app, "MCP bearer token", "synthetic-mcp-settings-token");
    assert!(
        !format!("{:?}", frame.platform_output.accesskit_update)
            .contains("synthetic-mcp-settings-token")
    );
    assert!(
        !rendered_text(&frame)
            .join("\n")
            .contains("synthetic-mcp-settings-token")
    );
    let frame = click_at(&mut app, require(&frame, "Replace MCP token"));
    assert_eq!(
        store.load(&reference).unwrap().as_deref(),
        Some("synthetic-mcp-settings-token")
    );
    assert!(common::enabled_clickable_center(&frame, "Replace MCP token").is_none());
    assert!(peer.exchanges.lock().unwrap().is_empty());
    assert!(
        !std::fs::read_to_string(root.path().join("session.json"))
            .unwrap()
            .contains("synthetic-mcp-settings-token")
    );
    click_at(&mut app, require(&frame, "Grant selected HTTP route"));
    app.runtime_mut_for_test()
        .handle_action(DesktopAction::SetProductMode {
            mode: legion_ui::DockMode::Assist,
        })
        .unwrap();
    let frame = app.run_headless_full_frame(full_frame_input(Vec::new()));
    click_at(&mut app, require(&frame, "Connect selected peer"));
    let frame = wait_idle(&mut app, &id);
    assert!(rendered_text(&frame).join("\n").contains("Ready"));
    let rows = peer.exchanges.lock().unwrap().clone();
    assert_eq!(
        rows.iter().map(|r| r.0.as_str()).collect::<Vec<_>>(),
        [
            "initialize",
            "notifications/initialized",
            "tools/list",
            "ping"
        ]
    );
    assert!(
        rows.iter()
            .all(|r| r.1 == "Bearer synthetic-mcp-settings-token" && r.2 == "2025-11-25")
    );
    let frame = click_at(&mut app, require(&frame, "Revoke MCP token and grant"));
    assert!(rendered_text(&frame).join("\n").contains("Deleted"));
    assert!(store.load(&reference).unwrap().is_none());
    let snapshot = app
        .runtime_mut_for_test()
        .app_mut_for_test()
        .inspect_named_mcp_peer(&id)
        .unwrap();
    assert_eq!(snapshot.health, McpPeerHealth::Revoked);
    assert!(!snapshot.transport_granted);
    assert_eq!(peer.exchanges.lock().unwrap().len(), 4);
}

#[test]
fn edit_selected_http_peer_retires_grant_and_rejects_stale_or_unpersistable_changes() {
    let root = tempfile::tempdir().unwrap();
    let store = Arc::new(InMemorySecretStore::default());
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let endpoint = format!("http://{}/edited", listener.local_addr().unwrap());
    let id = McpServerId("editable-peer".into());
    let mut runtime = runtime(root.path(), store);
    runtime
        .app_mut_for_test()
        .configure_named_mcp_peer(NamedMcpPeerConfig::http_client(
            id.clone(),
            "Editable peer",
            "https://old.invalid/mcp",
        ))
        .unwrap();
    runtime
        .app_mut_for_test()
        .configure_named_mcp_peer(NamedMcpPeerConfig::http_client(
            McpServerId("untouched-peer".into()),
            "Untouched peer",
            "https://untouched.invalid/mcp",
        ))
        .unwrap();
    runtime
        .handle_action(DesktopAction::SelectNamedMcpPeer {
            peer_id: id.clone(),
        })
        .unwrap();
    runtime
        .handle_action(DesktopAction::ManageNamedMcpPeer {
            peer_id: id.clone(),
            revision: 1,
            operation: McpSettingsOperation::GrantTransport,
        })
        .unwrap();
    runtime.save_session_state().unwrap();
    let mut app = DesktopEframeApp::new(runtime);
    let frame = page(&mut app);
    click_at(&mut app, require(&frame, "Edit selected HTTP peer"));
    let frame = fill(&mut app, "HTTP endpoint", &endpoint);
    click_at(&mut app, require(&frame, "Save HTTP peer"));
    let runtime = app.runtime_mut_for_test();
    let snapshot = runtime
        .app_mut_for_test()
        .inspect_named_mcp_peer(&id)
        .unwrap();
    assert_eq!(snapshot.revision, 2);
    assert!(!snapshot.transport_granted);
    let saved = std::fs::read(root.path().join("session.json")).unwrap();
    assert!(
        String::from_utf8(saved.clone())
            .unwrap()
            .contains(&endpoint)
    );
    assert!(
        String::from_utf8(saved.clone())
            .unwrap()
            .contains("https://untouched.invalid/mcp")
    );
    for (expected_revision, invalid_endpoint) in [
        (Some(1), endpoint.clone()),
        (
            Some(2),
            "https://user:synthetic-password@peer.invalid/mcp".into(),
        ),
        (
            Some(2),
            format!("https://peer.invalid/{}", "a".repeat(4096)),
        ),
    ] {
        let result = runtime
            .handle_action(DesktopAction::ConfigureNamedMcpHttpPeer {
                form: McpHttpPeerForm {
                    peer_id: id.0.clone(),
                    display_label: "Editable peer".into(),
                    endpoint: SensitiveString(invalid_endpoint),
                    expected_revision,
                    ..Default::default()
                },
            })
            .unwrap();
        assert!(matches!(
            result,
            legion_desktop::workflow::DesktopWorkflowOutcome::Error(_)
        ));
        assert!(!format!("{result:?}").contains("synthetic-password"));
        assert_eq!(
            runtime
                .app_mut_for_test()
                .inspect_named_mcp_peer(&id)
                .unwrap(),
            snapshot
        );
        assert_eq!(
            std::fs::read(root.path().join("session.json")).unwrap(),
            saved
        );
    }
    let stale = DesktopAction::ReplaceNamedMcpPeerCredential {
        peer_id: id.clone(),
        revision: 1,
        credential: SensitiveString("synthetic-stale-token".into()),
    };
    assert!(!format!("{stale:?}").contains("synthetic-stale-token"));
    assert!(matches!(
        runtime.handle_action(stale).unwrap(),
        legion_desktop::workflow::DesktopWorkflowOutcome::Error(_)
    ));
    assert_eq!(
        runtime
            .app_mut_for_test()
            .inspect_named_mcp_peer(&id)
            .unwrap(),
        snapshot
    );
    assert_eq!(
        listener.accept().unwrap_err().kind(),
        std::io::ErrorKind::WouldBlock
    );
}

#[derive(Default)]
struct UnavailableStore {
    reads: std::sync::atomic::AtomicUsize,
    writes: std::sync::atomic::AtomicUsize,
    deletes: std::sync::atomic::AtomicUsize,
}
impl UnavailableStore {
    fn error() -> SecretStoreError {
        SecretStoreError::KeyringFailure {
            message: "unsafe-store-error-with-synthetic-token".into(),
        }
    }
}
impl SecretStore for UnavailableStore {
    fn load(&self, _: &SecretReference) -> Result<Option<String>, SecretStoreError> {
        self.reads.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        Err(Self::error())
    }
    fn store(&self, _: &SecretReference, _: &str) -> Result<(), SecretStoreError> {
        self.writes
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        Err(Self::error())
    }
    fn delete(&self, _: &SecretReference) -> Result<(), SecretStoreError> {
        self.deletes
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        Err(Self::error())
    }
}

#[test]
fn credential_drafts_clear_on_route_and_settings_changes_and_store_failures_stay_redacted() {
    use std::sync::atomic::Ordering::SeqCst;
    let root = tempfile::tempdir().unwrap();
    let store = Arc::new(UnavailableStore::default());
    let id = McpServerId("masked-draft".into());
    let mut config =
        NamedMcpPeerConfig::http_client(id.clone(), "Masked draft", "https://peer.invalid/mcp");
    config.metadata.authentication = McpPeerAuthentication::Bearer;
    config.metadata.credential_scopes = vec!["tools:read".into()];
    let mut runtime = runtime(root.path(), store.clone());
    runtime
        .app_mut_for_test()
        .configure_named_mcp_peer(config.clone())
        .unwrap();
    runtime
        .handle_action(DesktopAction::SelectNamedMcpPeer {
            peer_id: id.clone(),
        })
        .unwrap();
    runtime.save_session_state().unwrap();
    let mut app = DesktopEframeApp::new(runtime);
    page(&mut app);
    let frame = fill(&mut app, "MCP bearer token", "synthetic-discard-token");
    assert!(common::enabled_clickable_center(&frame, "Replace MCP token").is_some());
    click_at(&mut app, require(&frame, "Close Settings"));
    let frame = page(&mut app);
    assert!(common::enabled_clickable_center(&frame, "Replace MCP token").is_none());
    fill(&mut app, "MCP bearer token", "");
    let frame = common::press_key(
        &mut app,
        egui::Key::Z,
        egui::Modifiers {
            ctrl: true,
            command: true,
            ..Default::default()
        },
    );
    assert!(common::enabled_clickable_center(&frame, "Replace MCP token").is_none());
    fill(&mut app, "MCP bearer token", "synthetic-route-token");
    config.metadata.credential_scopes = vec!["tools:inspect".into()];
    app.runtime_mut_for_test()
        .app_mut_for_test()
        .configure_named_mcp_peer(config)
        .unwrap();
    let frame = app.run_headless_full_frame(full_frame_input(Vec::new()));
    assert!(common::enabled_clickable_center(&frame, "Replace MCP token").is_none());
    let stale = DesktopAction::ReplaceNamedMcpPeerCredential {
        peer_id: id.clone(),
        revision: 1,
        credential: SensitiveString("synthetic-stale-token".into()),
    };
    assert!(matches!(
        app.runtime_mut_for_test().handle_action(stale).unwrap(),
        legion_desktop::workflow::DesktopWorkflowOutcome::Error(_)
    ));
    assert_eq!(store.writes.load(SeqCst), 0);
    let frame = fill(&mut app, "MCP bearer token", "synthetic-failing-token");
    let frame = click_at(&mut app, require(&frame, "Replace MCP token"));
    assert_eq!(store.writes.load(SeqCst), 1);
    assert!(
        !rendered_text(&frame)
            .join("\n")
            .contains("unsafe-store-error")
    );
    assert!(!format!("{:?}", app.runtime_snapshot()).contains("synthetic-failing-token"));
    let frame = click_at(&mut app, require(&frame, "Revoke MCP token and grant"));
    assert!(rendered_text(&frame).join("\n").contains("DeletionFailed"));
    let snapshot = app
        .runtime_mut_for_test()
        .app_mut_for_test()
        .inspect_named_mcp_peer(&id)
        .unwrap();
    assert_eq!(snapshot.health, McpPeerHealth::Revoked);
    assert!(!snapshot.transport_granted);
    assert_eq!(store.deletes.load(SeqCst), 1);
    assert_eq!(
        store.reads.load(SeqCst),
        0,
        "paint and metadata inspection must never access the keyring"
    );
    let bytes = std::fs::read_to_string(root.path().join("session.json")).unwrap();
    assert!(!bytes.contains("synthetic-"));
    assert!(!bytes.contains("unsafe-store-error"));
}

#[test]
fn reconfiguration_and_app_drop_retire_slow_workers_without_blocking_or_publishing_old_health() {
    use std::sync::atomic::Ordering::SeqCst;
    for drop_app in [false, true] {
        let root = tempfile::tempdir().unwrap();
        let peer = LocalPeer::with_pause(Some("initialize"));
        let id = McpServerId("retired-worker".into());
        let mut runtime = runtime(root.path(), Arc::new(InMemorySecretStore::default()));
        runtime
            .app_mut_for_test()
            .configure_named_mcp_peer(NamedMcpPeerConfig::http_client(
                id.clone(),
                "Retired worker",
                &peer.endpoint,
            ))
            .unwrap();
        runtime
            .handle_action(DesktopAction::SelectNamedMcpPeer {
                peer_id: id.clone(),
            })
            .unwrap();
        runtime
            .handle_action(DesktopAction::ManageNamedMcpPeer {
                peer_id: id.clone(),
                revision: 1,
                operation: McpSettingsOperation::GrantTransport,
            })
            .unwrap();
        runtime
            .handle_action(DesktopAction::SetProductMode {
                mode: legion_ui::DockMode::Assist,
            })
            .unwrap();
        runtime
            .handle_action(DesktopAction::ManageNamedMcpPeer {
                peer_id: id.clone(),
                revision: 1,
                operation: McpSettingsOperation::Connect,
            })
            .unwrap();
        let until = std::time::Instant::now() + std::time::Duration::from_millis(500);
        while !peer.held_request.load(SeqCst) && std::time::Instant::now() < until {
            std::thread::sleep(std::time::Duration::from_millis(2));
        }
        assert!(peer.held_request.load(SeqCst));
        assert!(matches!(
            runtime
                .handle_action(DesktopAction::ManageNamedMcpPeer {
                    peer_id: id.clone(),
                    revision: 1,
                    operation: McpSettingsOperation::Connect
                })
                .unwrap(),
            legion_desktop::workflow::DesktopWorkflowOutcome::Error(_)
        ));
        let started = std::time::Instant::now();
        if drop_app {
            drop(runtime);
            assert!(started.elapsed() < std::time::Duration::from_millis(300));
            peer.release.store(true, SeqCst);
            std::thread::sleep(std::time::Duration::from_millis(100));
        } else {
            runtime
                .app_mut_for_test()
                .configure_named_mcp_peer(NamedMcpPeerConfig::http_client(
                    id.clone(),
                    "New reviewed route",
                    "https://replacement.invalid/mcp",
                ))
                .unwrap();
            assert!(started.elapsed() < std::time::Duration::from_millis(300));
            let baseline = runtime
                .app_mut_for_test()
                .inspect_named_mcp_peer(&id)
                .unwrap();
            runtime
                .app_mut_for_test()
                .set_product_mode(legion_app::AppProductMode::Manual);
            assert_eq!(
                runtime.app_mut_for_test().product_mode(),
                legion_app::AppProductMode::Assist
            );
            assert!(runtime.app_mut_for_test().provider_configuration_busy());
            peer.release.store(true, SeqCst);
            let mut app = DesktopEframeApp::new(runtime);
            let until = std::time::Instant::now() + std::time::Duration::from_secs(3);
            while app
                .runtime_mut_for_test()
                .app_mut_for_test()
                .provider_configuration_busy()
                && std::time::Instant::now() < until
            {
                app.run_headless_full_frame(full_frame_input(Vec::new()));
                std::thread::sleep(std::time::Duration::from_millis(2));
            }
            assert!(
                !app.runtime_mut_for_test()
                    .app_mut_for_test()
                    .provider_configuration_busy()
            );
            app.runtime_mut_for_test()
                .app_mut_for_test()
                .set_product_mode(legion_app::AppProductMode::Manual);
            assert_eq!(
                app.runtime_mut_for_test().app_mut_for_test().product_mode(),
                legion_app::AppProductMode::Manual
            );
            assert_eq!(
                app.runtime_mut_for_test()
                    .app_mut_for_test()
                    .inspect_named_mcp_peer(&id)
                    .unwrap(),
                baseline
            );
            assert_eq!(baseline.health, McpPeerHealth::Configured);
            assert!(!baseline.transport_granted);
            assert_eq!(baseline.revision, 2);
        }
        assert_eq!(
            peer.exchanges
                .lock()
                .unwrap()
                .iter()
                .map(|r| r.0.as_str())
                .collect::<Vec<_>>(),
            ["initialize"]
        );
    }
}

#[test]
fn invalid_http_form_keeps_entered_metadata_for_correction_without_changing_saved_state() {
    let root = tempfile::tempdir().unwrap();
    let mut app = DesktopEframeApp::new(runtime(
        root.path(),
        Arc::new(InMemorySecretStore::default()),
    ));
    let frame = page(&mut app);
    click_at(&mut app, require(&frame, "Add HTTP peer"));
    fill(&mut app, "Peer ID", "correctable-peer");
    fill(&mut app, "Peer label", "Draft to correct");
    let frame = fill(&mut app, "HTTP endpoint", "http://public.invalid/mcp");
    let frame = click_at(&mut app, require(&frame, "Save HTTP peer"));
    assert!(
        frame
            .platform_output
            .accesskit_update
            .as_ref()
            .unwrap()
            .nodes
            .iter()
            .any(|(_, node)| node.role() == egui::accesskit::Role::TextInput
                && node.value() == Some("Draft to correct"))
    );
    assert!(
        app.runtime_mut_for_test()
            .app_mut_for_test()
            .inspect_named_mcp_peer(&McpServerId("correctable-peer".into()))
            .is_none()
    );
    let frame = fill(&mut app, "HTTP endpoint", "https://corrected.invalid/mcp");
    let frame = click_at(&mut app, require(&frame, "Save HTTP peer"));
    assert!(rendered_text(&frame).join("\n").contains("Configured"));
    let record = app.runtime_mut_for_test().capture_session_record().unwrap();
    assert!(
        record
            .workbench_settings
            .named_mcp_peer_configuration_json
            .unwrap()
            .contains("https://corrected.invalid/mcp")
    );
}

fn require(frame: &egui::FullOutput, label: &str) -> egui::Pos2 {
    clickable_center(frame, label).unwrap_or_else(|| panic!("missing control: {label}"))
}

fn page(app: &mut DesktopEframeApp) -> egui::FullOutput {
    let frame = app.run_headless_full_frame(full_frame_input(Vec::new()));
    let frame = click_at(app, require(&frame, "Settings"));
    click_at(app, require(&frame, "MCP Peers"))
}

fn wait_idle(app: &mut DesktopEframeApp, id: &McpServerId) -> egui::FullOutput {
    let until = std::time::Instant::now() + std::time::Duration::from_secs(3);
    loop {
        let frame = app.run_headless_full_frame(full_frame_input(Vec::new()));
        if !app
            .runtime_mut_for_test()
            .app_mut_for_test()
            .named_mcp_peer_operation_pending(id)
        {
            return frame;
        }
        assert!(
            std::time::Instant::now() < until,
            "MCP operation did not settle"
        );
        std::thread::sleep(std::time::Duration::from_millis(2));
    }
}

fn fill(app: &mut DesktopEframeApp, label: &str, value: &str) -> egui::FullOutput {
    let frame = app.run_headless_full_frame(full_frame_input(Vec::new()));
    let nodes = &frame
        .platform_output
        .accesskit_update
        .as_ref()
        .unwrap()
        .nodes;
    let labels: Vec<_> = nodes
        .iter()
        .filter(|(_, n)| n.value() == Some(label) || n.label() == Some(label))
        .map(|(id, _)| *id)
        .collect();
    let bounds = nodes
        .iter()
        .find_map(|(_, n)| {
            (matches!(
                n.role(),
                egui::accesskit::Role::TextInput | egui::accesskit::Role::PasswordInput
            ) && n.labelled_by().iter().any(|id| labels.contains(id)))
            .then(|| n.bounds())
            .flatten()
        })
        .unwrap_or_else(|| panic!("missing labelled field: {label}"));
    click_at(
        app,
        egui::pos2(
            ((bounds.x0 + bounds.x1) / 2.0) as f32,
            ((bounds.y0 + bounds.y1) / 2.0) as f32,
        ),
    );
    common::press_key(
        app,
        egui::Key::A,
        egui::Modifiers {
            ctrl: true,
            command: true,
            ..Default::default()
        },
    );
    app.run_headless_full_frame(full_frame_input(vec![egui::Event::Text(value.into())]))
}

#[test]
fn settings_input_persists_named_http_configuration_without_connection_or_selection() {
    let root = tempfile::tempdir().unwrap();
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let endpoint = format!("http://{}/mcp", listener.local_addr().unwrap());
    let store = Arc::new(InMemorySecretStore::default());
    let mut app = DesktopEframeApp::new(runtime(root.path(), store.clone()));
    let frame = page(&mut app);
    assert!(
        rendered_text(&frame)
            .join("\n")
            .contains("Client stdio and server role are unsupported")
    );
    click_at(&mut app, require(&frame, "Add HTTP peer"));
    fill(&mut app, "Peer ID", "local-inspector");
    fill(&mut app, "Peer label", "Local inspector");
    let frame = fill(&mut app, "HTTP endpoint", &endpoint);
    click_at(&mut app, require(&frame, "Save HTTP peer"));
    let frame = app.run_headless_full_frame(full_frame_input(Vec::new()));
    let text = rendered_text(&frame).join("\n");
    assert!(text.contains("Local inspector"));
    assert!(text.contains("Configured"));
    assert!(text.contains("No peer selected"));
    let bytes = std::fs::read_to_string(root.path().join("session.json")).unwrap();
    assert!(bytes.contains("local-inspector"));
    assert!(bytes.contains(&endpoint));
    let mut reopened = DesktopEframeApp::new(runtime(root.path(), store));
    let text = rendered_text(&page(&mut reopened)).join("\n");
    assert!(text.contains("Local inspector"));
    assert!(text.contains("No peer selected"));
    assert_eq!(
        reopened
            .runtime_mut_for_test()
            .projection_snapshot()
            .product_mode,
        legion_ui::DockMode::Manual
    );
    assert_eq!(
        listener.accept().unwrap_err().kind(),
        std::io::ErrorKind::WouldBlock
    );
}
