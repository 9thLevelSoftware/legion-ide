#![cfg(feature = "ai")]

use legion_app::{
    AppComposition, AppProductMode,
    named_mcp_peer::{NamedMcpPeerConfig, NamedMcpPeerError, NamedMcpPeerPermissions},
};
use legion_protocol::{McpServerId, named_mcp_peer::McpPeerHealth};
use std::{
    io::{BufRead, Read, Write},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering::SeqCst},
    },
    time::{Duration, Instant},
};

// Independent HTTP oracle holds the first initialize response until released.
// No client mock can mark the operation drained before this transport returns.
struct HeldPeer {
    endpoint: String,
    received: Arc<AtomicBool>,
    release: Arc<AtomicBool>,
    worker: Option<std::thread::JoinHandle<()>>,
}
impl HeldPeer {
    fn new() -> Self {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let endpoint = format!("http://{}/mcp", listener.local_addr().unwrap());
        let received = Arc::new(AtomicBool::new(false));
        let release = Arc::new(AtomicBool::new(false));
        let held = received.clone();
        let released = release.clone();
        let worker = std::thread::spawn(move || {
            let until = Instant::now() + Duration::from_secs(5);
            let mut stream = loop {
                match listener.accept() {
                    Ok((stream, _)) => break stream,
                    Err(e)
                        if e.kind() == std::io::ErrorKind::WouldBlock && Instant::now() < until =>
                    {
                        std::thread::sleep(Duration::from_millis(2))
                    }
                    Err(e) => panic!("controlled peer accept: {e}"),
                }
            };
            stream.set_nonblocking(false).unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            let mut reader = std::io::BufReader::new(stream.try_clone().unwrap());
            let mut line = String::new();
            reader.read_line(&mut line).unwrap();
            assert_eq!(line.trim(), "POST /mcp HTTP/1.1");
            let mut length = 0;
            loop {
                line.clear();
                reader.read_line(&mut line).unwrap();
                if line == "\r\n" {
                    break;
                }
                let (key, value) = line.split_once(':').unwrap();
                if key.eq_ignore_ascii_case("content-length") {
                    length = value.trim().parse::<usize>().unwrap();
                }
            }
            let mut body = vec![0; length];
            reader.read_exact(&mut body).unwrap();
            let request: serde_json::Value = serde_json::from_slice(&body).unwrap();
            assert_eq!(request["method"], "initialize");
            held.store(true, SeqCst);
            let until = Instant::now() + Duration::from_secs(5);
            while !released.load(SeqCst) && Instant::now() < until {
                std::thread::sleep(Duration::from_millis(2));
            }
            let response = serde_json::json!({"jsonrpc":"2.0", "id":request["id"], "result":{"protocolVersion":"2025-11-25", "capabilities":{"tools":{}}, "serverInfo":{"name":"held-oracle","version":"1"}}}).to_string();
            write!(stream, "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nConnection: close\r\nContent-Length: {}\r\n\r\n{}", response.len(), response).unwrap();
        });
        Self {
            endpoint,
            received,
            release,
            worker: Some(worker),
        }
    }
}
impl Drop for HeldPeer {
    fn drop(&mut self) {
        self.release.store(true, SeqCst);
        let result = self.worker.take().unwrap().join();
        if !std::thread::panicking() {
            result.unwrap();
        }
    }
}

fn wait_for(mut condition: impl FnMut() -> bool) {
    let until = Instant::now() + Duration::from_secs(3);
    while !condition() && Instant::now() < until {
        std::thread::sleep(Duration::from_millis(2));
    }
    assert!(
        condition(),
        "controlled operation did not reach its observed state"
    );
}

#[test]
fn installed_lower_ceiling_denies_new_mcp_dispatch_and_waits_for_active_and_retired_http() {
    use legion_security::{
        PolicyKeyring, PolicySigningKey, policy_bundle_verifying_key_b64, sign_policy_bundle,
    };
    for retire in [false, true] {
        let peer = HeldPeer::new();
        let id = McpServerId("policy-drain".into());
        let mut app = AppComposition::new();
        app.set_product_mode(AppProductMode::Assist);
        // Keep the identical loopback network policy on both sides of the
        // ceiling change. Otherwise Unknown trust could make refusal vacuous.
        let allowed_payload = include_str!("../../../xtask/legion-policy.example.toml")
            .replace("\r\n", "\n")
            .replace(
                "[security_policy.network_policy]\nallow_untrusted = false",
                "[security_policy.network_policy]\nallow_untrusted = true",
            );
        let keyring = PolicyKeyring::new(vec![PolicySigningKey {
            key_id: "mcp-test".into(),
            verifying_key_b64: policy_bundle_verifying_key_b64(&[7; 32]),
        }]);
        app.set_org_policy_bundle(
            sign_policy_bundle(&allowed_payload, "mcp-test", &[7; 32])
                .verify(&keyring)
                .unwrap(),
        );
        let config =
            NamedMcpPeerConfig::http_client(id.clone(), "Held policy peer", &peer.endpoint);
        app.configure_named_mcp_peer(config.clone()).unwrap();
        app.grant_named_mcp_peer_transport(&id, 1, NamedMcpPeerPermissions::network())
            .unwrap();
        app.start_named_mcp_peer_connection(&id, 1).unwrap();
        wait_for(|| peer.received.load(SeqCst));
        if retire {
            app.configure_named_mcp_peer(config).unwrap();
        }
        let payload =
            allowed_payload.replace("mode_ceiling = \"Assist\"", "mode_ceiling = \"Manual\"");
        app.set_org_policy_bundle(
            sign_policy_bundle(&payload, "mcp-test", &[7; 32])
                .verify(&keyring)
                .unwrap(),
        );
        assert!(app.org_policy_mode_ceiling_denies(AppProductMode::Assist));
        assert_eq!(
            app.product_mode(),
            AppProductMode::Assist,
            "held HTTP cannot be labelled Manual"
        );
        assert!(
            app.provider_configuration_busy(),
            "retired transport still owns the common drain lease"
        );
        app.poll_named_mcp_peer_operations();
        assert_eq!(app.product_mode(), AppProductMode::Assist);
        let other = McpServerId("new-dispatch".into());
        app.configure_named_mcp_peer(NamedMcpPeerConfig::http_client(
            other.clone(),
            "Denied peer",
            &peer.endpoint,
        ))
        .unwrap();
        app.grant_named_mcp_peer_transport(&other, 1, NamedMcpPeerPermissions::network())
            .unwrap();
        assert_eq!(
            app.start_named_mcp_peer_connection(&other, 1),
            Err(NamedMcpPeerError::PermissionRequired)
        );
        assert_eq!(
            app.activate_named_mcp_peer(&other, 1, &legion_storage::InMemorySecretStore::default()),
            Err(NamedMcpPeerError::PermissionRequired)
        );
        peer.release.store(true, SeqCst);
        wait_for(|| {
            app.poll_named_mcp_peer_operations();
            !app.provider_configuration_busy() && app.product_mode() == AppProductMode::Manual
        });
        assert!(!app.named_mcp_peer_operation_pending(&id));
        let snapshot = app.inspect_named_mcp_peer(&id).unwrap();
        assert_eq!(
            snapshot.health,
            if retire {
                McpPeerHealth::Configured
            } else {
                McpPeerHealth::Revoked
            }
        );
        assert!(!snapshot.transport_granted);
    }
}
