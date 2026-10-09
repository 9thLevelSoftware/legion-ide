#![cfg(feature = "ai")]

use legion_app::named_mcp_peer::{
    NamedMcpPeerConfig, NamedMcpPeerError, NamedMcpPeerPermissions, named_mcp_peer_secret_reference,
};
use legion_app::{AppComposition, AppProductMode};
use legion_desktop::session::DesktopSessionStore;
use legion_protocol::{McpServerId, PrincipalId, WorkspaceTrustState, named_mcp_peer::*};
use legion_storage::{InMemorySecretStore, SecretStore};
use std::sync::Arc;

fn app(root: &std::path::Path) -> AppComposition {
    let mut app =
        AppComposition::with_provider_secret_store(Arc::new(InMemorySecretStore::default()));
    app.open_workspace(
        root,
        WorkspaceTrustState::Trusted,
        PrincipalId("mcp-persistence".into()),
    )
    .unwrap();
    app.open_file(root.join("main.rs").to_string_lossy())
        .unwrap();
    app
}

fn config(endpoint: &str) -> NamedMcpPeerConfig {
    let mut config = NamedMcpPeerConfig::http_client(
        McpServerId("reviewed-peer".into()),
        "Reviewed peer",
        endpoint,
    );
    config.metadata.authentication = McpPeerAuthentication::Bearer;
    config.metadata.credential_scopes = vec!["tools:read".into()];
    config
}

fn provider() -> legion_app::AiProviderProfile {
    legion_app::AiProviderProfile {
        name: "local".into(),
        provider_id: "ollama".into(),
        endpoint: "http://127.0.0.1:11434".into(),
        model: "fixture".into(),
        max_completion_tokens: false,
        disable_thinking: false,
    }
}

#[test]
fn malformed_provider_or_memory_cannot_partially_replace_mcp_or_other_domains() {
    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("main.rs"), "fn main() {}\n").unwrap();
    let mut active = app(root.path());
    let peer = config("https://peer.invalid/mcp");
    let snapshot = active.configure_named_mcp_peer(peer.clone()).unwrap();
    active
        .grant_named_mcp_peer_transport(
            &peer.metadata.peer_id,
            snapshot.revision,
            NamedMcpPeerPermissions::network(),
        )
        .unwrap();
    active.configure_ai_provider_profile(provider()).unwrap();
    let good = active.capture_workspace_session_record().unwrap();
    let baseline = active.inspect_named_mcp_peer(&peer.metadata.peer_id);
    for bad_memory in [false, true] {
        let mut incoming = good.clone();
        incoming
            .workbench_settings
            .named_mcp_peer_configuration_json = Some(r#"{"version":1,"peers":[]}"#.into());
        incoming.workbench_settings.ai_provider_configuration_json = Some(
            if bad_memory {
                r#"{"profiles":[],"selected":null}"#
            } else {
                "{"
            }
            .into(),
        );
        incoming.workbench_settings.zoom_percent = 175;
        if bad_memory {
            incoming.memory_snapshot_json = Some("{".into());
        }
        assert!(active.restore_workspace_session_record(&incoming).is_err());
        let after = active.capture_workspace_session_record().unwrap();
        assert_eq!(after.workbench_settings, good.workbench_settings);
        assert_eq!(after.memory_snapshot_json, good.memory_snapshot_json);
        assert_eq!(
            active.inspect_named_mcp_peer(&peer.metadata.peer_id),
            baseline
        );
    }
}

#[test]
fn legacy_reopen_clears_mcp_grants_without_deleting_bound_credentials() {
    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("main.rs"), "fn main() {}\n").unwrap();
    let mut active = app(root.path());
    let peer = config("https://peer.invalid/mcp");
    let secrets = InMemorySecretStore::default();
    let reference = named_mcp_peer_secret_reference(&peer);
    secrets
        .store(&reference, "synthetic-legacy-secret")
        .unwrap();
    let snapshot = active.configure_named_mcp_peer(peer.clone()).unwrap();
    active
        .grant_named_mcp_peer_transport(
            &peer.metadata.peer_id,
            snapshot.revision,
            NamedMcpPeerPermissions::network(),
        )
        .unwrap();
    active.set_product_mode(AppProductMode::Assist);
    let mut legacy = active.capture_workspace_session_record().unwrap();
    legacy.workbench_settings.named_mcp_peer_configuration_json = None;
    let path = root.path().join("legacy.json");
    DesktopSessionStore::save(&path, &legacy).unwrap();
    assert!(
        !std::fs::read_to_string(&path)
            .unwrap()
            .contains("named_mcp_peer_configuration_json")
    );
    let loaded = DesktopSessionStore::load(&path).unwrap().unwrap();
    active.restore_workspace_session_record(&loaded).unwrap();
    assert!(
        active
            .inspect_named_mcp_peer(&peer.metadata.peer_id)
            .is_none()
    );
    assert_eq!(active.product_mode(), AppProductMode::Manual);
    assert_eq!(
        secrets.load(&reference).unwrap().as_deref(),
        Some("synthetic-legacy-secret")
    );
    let restored = active.configure_named_mcp_peer(peer).unwrap();
    assert!(!restored.transport_granted);
}

#[cfg(windows)]
#[test]
fn failed_atomic_publication_reopens_previous_mcp_and_provider_metadata() {
    use std::os::windows::fs::OpenOptionsExt;
    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("main.rs"), "fn main() {}\n").unwrap();
    let path = root.path().join("session.json");
    let mut active = app(root.path());
    let peer = config("https://previous.invalid/mcp");
    active.configure_named_mcp_peer(peer.clone()).unwrap();
    active.configure_ai_provider_profile(provider()).unwrap();
    active.select_ai_provider_profile("local").unwrap();
    DesktopSessionStore::save(&path, &active.capture_workspace_session_record().unwrap()).unwrap();
    let previous = std::fs::read(&path).unwrap();
    active
        .configure_named_mcp_peer(config("https://replacement.invalid/mcp"))
        .unwrap();
    let mut changed = provider();
    changed.model = "replacement".into();
    active.configure_ai_provider_profile(changed).unwrap();
    // Permit in-place writes but deny DELETE, so the actual atomic replacement fails.
    let locked = std::fs::OpenOptions::new()
        .read(true)
        .share_mode(3)
        .open(&path)
        .unwrap();
    assert!(
        DesktopSessionStore::save(&path, &active.capture_workspace_session_record().unwrap())
            .is_err()
    );
    assert_eq!(std::fs::read(&path).unwrap(), previous);
    let saved = DesktopSessionStore::load(&path).unwrap().unwrap();
    drop(locked);
    let mut reopened = app(root.path());
    reopened.restore_workspace_session_record(&saved).unwrap();
    let secrets = InMemorySecretStore::default();
    let reference = named_mcp_peer_secret_reference(&peer);
    secrets.store(&reference, "synthetic-old-binding").unwrap();
    reopened
        .revoke_named_mcp_peer_credential(&peer.metadata.peer_id, &secrets)
        .unwrap();
    assert!(
        secrets.load(&reference).unwrap().is_none(),
        "reopen must use the previous endpoint binding"
    );
    assert_eq!(reopened.ai_provider_profiles()[0].profile.model, "fixture");
    assert!(reopened.ai_provider_profiles()[0].selected);
    assert_eq!(reopened.product_mode(), AppProductMode::Manual);
}

#[test]
fn capture_rejects_unpersistable_launch_details_without_running_a_process() {
    use legion_app::named_mcp_peer::NamedMcpPeerTransport;
    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("main.rs"), "fn main() {}\n").unwrap();
    let mut active = app(root.path());
    let mut peer = config("https://peer.invalid/mcp");
    peer.metadata.authentication = McpPeerAuthentication::None;
    peer.metadata.credential_scopes.clear();
    peer.metadata.transport = legion_protocol::McpTransportKind::Stdio;
    peer.transport = NamedMcpPeerTransport::Stdio {
        command: "not-a-real-peer-executable".into(),
        args: vec!["synthetic-private-launch-argument".into()],
    };
    active.configure_named_mcp_peer(peer).unwrap();
    let error = active
        .capture_workspace_session_record()
        .unwrap_err()
        .to_string();
    assert_eq!(
        error,
        "phase 4 AI runtime failed: invalid named MCP configuration metadata"
    );
}

#[test]
fn invalid_mcp_metadata_preserves_saved_bytes_and_all_active_domains() {
    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("main.rs"), "fn main() {}\n").unwrap();
    let path = root.path().join("session.json");
    let corrupt = root.path().join("corrupt.json");
    let mut active = app(root.path());
    let peer = config("https://peer.invalid/mcp");
    let snapshot = active.configure_named_mcp_peer(peer.clone()).unwrap();
    active
        .grant_named_mcp_peer_transport(
            &peer.metadata.peer_id,
            snapshot.revision,
            NamedMcpPeerPermissions::network(),
        )
        .unwrap();
    active.configure_ai_provider_profile(provider()).unwrap();
    active.select_ai_provider_profile("local").unwrap();
    let good = active.capture_workspace_session_record().unwrap();
    DesktopSessionStore::save(&path, &good).unwrap();
    let bytes = std::fs::read(&path).unwrap();
    let baseline = active.inspect_named_mcp_peer(&peer.metadata.peer_id);
    let valid: serde_json::Value = serde_json::from_str(
        good.workbench_settings
            .named_mcp_peer_configuration_json
            .as_deref()
            .unwrap(),
    )
    .unwrap();
    let mut invalid = vec![
        "{".into(),
        r#"{"version":2,"peers":[]}"#.into(),
        format!("{}{}", " ".repeat(65537), valid),
    ];
    for (field, value) in [
        (
            "credential",
            serde_json::json!("synthetic-forbidden-secret"),
        ),
        ("transport_granted", serde_json::json!(true)),
        ("health", serde_json::json!("Ready")),
        (
            "endpoint",
            serde_json::json!("https://user:synthetic-forbidden-secret@peer.invalid/mcp"),
        ),
        (
            "endpoint",
            serde_json::json!(format!("https://peer.invalid/{}", "x".repeat(4096))),
        ),
        ("protocol_version", serde_json::json!("unknown")),
        ("role", serde_json::json!("Server")),
    ] {
        let mut changed = valid.clone();
        changed["peers"][0][field] = value;
        invalid.push(changed.to_string());
    }
    let mut unknown = valid.clone();
    unknown["runtime"] = serde_json::json!({});
    invalid.push(unknown.to_string());
    let mut duplicates = valid.clone();
    duplicates["peers"]
        .as_array_mut()
        .unwrap()
        .push(valid["peers"][0].clone());
    invalid.push(duplicates.to_string());
    let mut too_many = valid.clone();
    too_many["peers"] = serde_json::Value::Array(
        (0..33)
            .map(|i| {
                let mut peer = valid["peers"][0].clone();
                peer["peer_id"] = serde_json::json!(format!("peer-{i}"));
                peer
            })
            .collect(),
    );
    invalid.push(too_many.to_string());
    for metadata in invalid {
        let mut record = good.clone();
        record.workbench_settings.named_mcp_peer_configuration_json = Some(metadata);
        record.workbench_settings.ai_provider_configuration_json =
            Some(r#"{"profiles":[],"selected":null}"#.into());
        record.workbench_settings.zoom_percent = 175;
        let error = DesktopSessionStore::save(&path, &record)
            .expect_err("invalid MCP configuration must not publish");
        assert!(!error.to_string().contains("synthetic-forbidden-secret"));
        assert_eq!(std::fs::read(&path).unwrap(), bytes);
        assert!(active.restore_workspace_session_record(&record).is_err());
        assert_eq!(
            active.inspect_named_mcp_peer(&peer.metadata.peer_id),
            baseline
        );
        let after = active.capture_workspace_session_record().unwrap();
        assert_eq!(after.workbench_settings, good.workbench_settings);
        assert_eq!(after.memory_snapshot_json, good.memory_snapshot_json);
        std::fs::write(&corrupt, serde_json::to_vec(&record).unwrap()).unwrap();
        assert!(DesktopSessionStore::load(&corrupt).is_err());
    }
    let saved = DesktopSessionStore::load(&path).unwrap().unwrap();
    let mut reopened = app(root.path());
    reopened.restore_workspace_session_record(&saved).unwrap();
    assert_eq!(
        reopened
            .inspect_named_mcp_peer(&peer.metadata.peer_id)
            .unwrap()
            .metadata,
        peer.metadata
    );
    assert!(reopened.ai_provider_profiles()[0].selected);
}

#[test]
fn reopen_preserves_configuration_binding_without_grants_secrets_or_connections() {
    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("main.rs"), "fn main() {}\n").unwrap();
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let config = config(&format!("http://{}/mcp", listener.local_addr().unwrap()));
    let id = config.metadata.peer_id.clone();
    let secrets = InMemorySecretStore::default();
    let reference = named_mcp_peer_secret_reference(&config);
    secrets
        .store(&reference, "synthetic-mcp-persistence-secret")
        .unwrap();
    let mut original = app(root.path());
    let snapshot = original.configure_named_mcp_peer(config.clone()).unwrap();
    original
        .grant_named_mcp_peer_transport(&id, snapshot.revision, NamedMcpPeerPermissions::network())
        .unwrap();
    original.set_product_mode(AppProductMode::Assist);
    let record = original.capture_workspace_session_record().unwrap();
    let json = serde_json::to_value(&record.workbench_settings).unwrap();
    assert!(
        json["named_mcp_peer_configuration_json"].is_string(),
        "capture must retain named peer configuration"
    );
    let path = root.path().join("session.json");
    DesktopSessionStore::save(&path, &record).unwrap();
    drop(original);
    let bytes = std::fs::read_to_string(&path).unwrap();
    for forbidden in [
        "synthetic-mcp-persistence-secret",
        "transport_granted",
        "credential_state",
        "live_grant",
        "health",
    ] {
        assert!(!bytes.contains(forbidden), "retained {forbidden}");
    }
    let saved = DesktopSessionStore::load(&path).unwrap().unwrap();
    let mut reopened = app(root.path());
    reopened.restore_workspace_session_record(&saved).unwrap();
    let restored = reopened.inspect_named_mcp_peer(&id).unwrap();
    assert_eq!(restored.metadata, config.metadata);
    assert_eq!(restored.health, McpPeerHealth::Configured);
    assert!(!restored.transport_granted);
    assert_eq!(restored.credential_state, McpPeerCredentialState::Unchanged);
    assert_eq!(reopened.product_mode(), AppProductMode::Manual);
    assert_eq!(
        reopened.activate_named_mcp_peer(&id, restored.revision, &secrets),
        Err(NamedMcpPeerError::ManualMode)
    );
    reopened.set_product_mode(AppProductMode::Assist);
    assert_eq!(
        reopened.activate_named_mcp_peer(&id, restored.revision, &secrets),
        Err(NamedMcpPeerError::PermissionRequired)
    );
    reopened
        .revoke_named_mcp_peer_credential(&id, &secrets)
        .unwrap();
    assert!(
        secrets.load(&reference).unwrap().is_none(),
        "restored route must address the same bound SecretStore entry"
    );
    assert_eq!(
        listener.accept().unwrap_err().kind(),
        std::io::ErrorKind::WouldBlock
    );
}
