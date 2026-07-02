use std::{
    fs,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

use legion_plugin::{PluginAuditKind, PluginRuntimeState, WasmPluginHost};
use legion_protocol::{
    CapabilityId, LanguageId, PluginActivationEvent, PluginCommandDescriptor, PluginContribution,
    PluginId, PluginManifest, PluginQuotaDeclaration, PluginTreeSitterGrammarContribution,
    PluginTrustDecision, PluginTrustMetadata, PluginTrustSource,
};

fn manifest(max_host_calls: u32) -> PluginManifest {
    PluginManifest {
        plugin_id: PluginId(7),
        name: "phase5.fixture".to_string(),
        version: "0.1.0".to_string(),
        schema_version: 1,
        min_abi_version: 1,
        max_abi_version: 1,
        module_hash: "sha256:fixture".to_string(),
        manifest_id: "manifest-fixture".to_string(),
        trust: PluginTrustMetadata {
            source: PluginTrustSource::ExplicitLocalAllow,
            decision: PluginTrustDecision::ExplicitlyAllowed,
            reason: "fixture".to_string(),
        },
        signature: None,
        activation_events: vec![PluginActivationEvent::Startup],
        contributions: vec![
            PluginContribution::Command(PluginCommandDescriptor {
                command_id: "phase5.run".to_string(),
                title: "Phase 5 Run".to_string(),
                required_capability: CapabilityId("plugin.command".to_string()),
            }),
            PluginContribution::TreeSitterGrammar(PluginTreeSitterGrammarContribution {
                language_id: LanguageId("rust-plugin".to_string()),
                grammar_name: "rust-plugin-grammar".to_string(),
                artifact_uri: "file:///tmp/rust-plugin-grammar.wasm".to_string(),
                artifact_hash: "sha256:rust-plugin-grammar".to_string(),
                required_capability: CapabilityId("plugin.grammar.tree_sitter".to_string()),
            }),
        ],
        requested_capabilities: vec![
            CapabilityId("plugin.command".to_string()),
            CapabilityId("plugin.grammar.tree_sitter".to_string()),
        ],
        storage_namespace: legion_protocol::PluginStateNamespace {
            plugin_id: PluginId(7),
            namespace: "state".to_string(),
        },
        quotas: PluginQuotaDeclaration {
            max_fuel: 1000,
            max_wall_time_ms: 50,
            max_memory_pages: 8,
            max_storage_bytes: 4096,
            max_host_calls,
            max_events: 4,
            max_output_bytes: 64,
        },
    }
}

fn write_fixture_wasm(name: &str, wat_source: &str) -> PathBuf {
    let mut path = std::env::temp_dir();
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    path.push(format!("legion-plugin-{name}-{unique}.wasm"));
    let wasm = wat::parse_str(wat_source).expect("compile fixture wat to wasm");
    fs::write(&path, wasm).expect("write wasm fixture");
    path
}

#[test]
fn fixture_plugin_runs_and_records_audit() {
    let wat = r#"
        (module
          (func (export "run") (result i32)
            i32.const 7))
    "#;
    let wasm_path = write_fixture_wasm("audit", wat);

    let mut host = WasmPluginHost::new();
    let plugin_id = host
        .load_fixture(manifest(1), &wasm_path)
        .expect("fixture loads");
    let value = host
        .invoke(plugin_id, "run")
        .expect("fixture executes without escape");

    assert_eq!(value, 7);
    assert_eq!(host.plugin_state(plugin_id), Some(PluginRuntimeState::Idle));

    let audit = host.audit_log(plugin_id);
    assert!(
        audit
            .iter()
            .any(|entry| entry.kind == PluginAuditKind::Loaded)
    );
    assert!(
        audit
            .iter()
            .any(|entry| entry.kind == PluginAuditKind::Invoked)
    );
    assert!(
        audit
            .iter()
            .any(|entry| entry.kind == PluginAuditKind::Completed)
    );
}

#[test]
fn fixture_plugin_denies_second_host_call_on_quota() {
    let wat = r#"
        (module
          (func (export "run") (result i32)
            i32.const 7))
    "#;
    let wasm_path = write_fixture_wasm("quota", wat);

    let mut host = WasmPluginHost::new();
    let plugin_id = host
        .load_fixture(manifest(1), &wasm_path)
        .expect("fixture loads");

    assert_eq!(host.invoke(plugin_id, "run").expect("first invoke"), 7);
    let error = host
        .invoke(plugin_id, "run")
        .expect_err("second host call should hit quota");
    assert_eq!(error.code, "plugin_host_call_quota_exceeded");
    assert_eq!(
        host.plugin_state(plugin_id),
        Some(PluginRuntimeState::Disabled)
    );

    let audit = host.audit_log(plugin_id);
    assert!(
        audit
            .iter()
            .any(|entry| entry.kind == PluginAuditKind::QuotaExceeded)
    );
}

#[test]
fn fixture_plugin_denies_wasi_escape() {
    let wat = r#"
        (module
          (import "wasi_snapshot_preview1" "fd_write"
            (func $fd_write (param i32 i32 i32 i32) (result i32)))
          (func (export "run") (result i32)
            i32.const 0))
    "#;
    let wasm_path = write_fixture_wasm("wasi-deny", wat);

    let mut host = WasmPluginHost::new();
    let error = host
        .load_fixture(manifest(1), &wasm_path)
        .expect_err("WASI imports are not granted");
    assert_eq!(error.code, "plugin_wasi_import_denied");
}

#[test]
fn fixture_plugin_denies_network_capability() {
    let wat = r#"
        (module
          (func (export "run") (result i32)
            i32.const 0))
    "#;
    let wasm_path = write_fixture_wasm("network-deny", wat);

    let mut host = WasmPluginHost::new();
    let mut manifest = manifest(1);
    manifest
        .requested_capabilities
        .push(CapabilityId("plugin.network".to_string()));

    let error = host
        .load_fixture(manifest, &wasm_path)
        .expect_err("network capability should be denied");
    assert_eq!(error.code, "plugin_capability_denied");
}

#[test]
fn fixture_plugin_trap_is_contained() {
    let wat = r#"
        (module
          (func (export "run")
            unreachable))
    "#;
    let wasm_path = write_fixture_wasm("trap", wat);

    let mut host = WasmPluginHost::new();
    let plugin_id = host
        .load_fixture(manifest(1), &wasm_path)
        .expect("fixture loads");

    let error = host
        .invoke(plugin_id, "run")
        .expect_err("trap should be contained");
    assert_eq!(error.code, "plugin_trapped");
    assert_eq!(
        host.plugin_state(plugin_id),
        Some(PluginRuntimeState::Crashed)
    );
    let audit = host.audit_log(plugin_id);
    assert!(
        audit
            .iter()
            .any(|entry| entry.kind == PluginAuditKind::Crashed)
    );
}
