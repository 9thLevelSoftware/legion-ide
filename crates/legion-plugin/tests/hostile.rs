use std::{
    fs,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

use legion_plugin::{PluginAuditKind, PluginRuntimeState, WasmPluginHost};
use legion_protocol::{
    CapabilityId, LanguageId, PluginActivationEvent, PluginCommandDescriptor, PluginContribution,
    PluginId, PluginManifest, PluginQuotaDeclaration, PluginStateNamespace,
    PluginTreeSitterGrammarContribution, PluginTrustDecision, PluginTrustMetadata,
    PluginTrustSource,
};

fn manifest() -> PluginManifest {
    let plugin_id = PluginId(23);
    PluginManifest {
        plugin_id,
        name: "hostile.fixture".to_string(),
        version: "0.1.0".to_string(),
        schema_version: 1,
        min_abi_version: 1,
        max_abi_version: 1,
        module_hash: "sha256:hostile-fixture".to_string(),
        manifest_id: "manifest-hostile".to_string(),
        trust: PluginTrustMetadata {
            source: PluginTrustSource::ExplicitLocalAllow,
            decision: PluginTrustDecision::ExplicitlyAllowed,
            reason: "fixture".to_string(),
        },
        signature: None,
        activation_events: vec![PluginActivationEvent::Startup],
        contributions: vec![
            PluginContribution::Command(PluginCommandDescriptor {
                command_id: "hostile.run".to_string(),
                title: "Hostile Run".to_string(),
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
        storage_namespace: PluginStateNamespace {
            plugin_id,
            namespace: "state".to_string(),
        },
        quotas: PluginQuotaDeclaration {
            max_fuel: 1000,
            max_wall_time_ms: 50,
            max_memory_pages: 8,
            max_storage_bytes: 4096,
            max_host_calls: 4,
            max_events: 4,
            max_output_bytes: 512,
        },
    }
}

fn hostile_fixture_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("fixtures")
        .join("hostile")
        .join(format!("{name}.wat"))
}

fn compile_fixture(name: &str) -> PathBuf {
    let source = fs::read_to_string(hostile_fixture_path(name)).expect("read hostile fixture");
    let mut path = std::env::temp_dir();
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    path.push(format!("legion-plugin-hostile-{name}-{unique}.wasm"));
    let wasm = wat::parse_str(&source).expect("compile hostile fixture");
    fs::write(&path, wasm).expect("write hostile fixture wasm");
    path
}

fn assert_audit_contains(host: &WasmPluginHost, plugin_id: PluginId, kind: PluginAuditKind) {
    let audit = host.audit_log(plugin_id);
    assert!(
        audit.iter().any(|entry| entry.kind == kind),
        "audit for plugin {:?} did not contain {:?}: {:?}",
        plugin_id,
        kind,
        audit
    );
}

#[test]
fn hostile_loop_fixture_is_contained_and_audited() {
    let mut host = WasmPluginHost::new();
    let plugin_id = host
        .load_fixture(manifest(), compile_fixture("loop"))
        .expect("loop fixture loads");

    let error = host
        .invoke(plugin_id, "run")
        .expect_err("loop fixture should trap");
    assert_eq!(error.code, "plugin_trapped");
    assert_eq!(
        host.plugin_state(plugin_id),
        Some(PluginRuntimeState::Crashed)
    );
    assert_audit_contains(&host, plugin_id, PluginAuditKind::Loaded);
    assert_audit_contains(&host, plugin_id, PluginAuditKind::Invoked);
    assert_audit_contains(&host, plugin_id, PluginAuditKind::Crashed);
}

#[test]
fn hostile_oom_fixture_is_contained_and_audited() {
    let mut host = WasmPluginHost::new();
    let plugin_id = host
        .load_fixture(manifest(), compile_fixture("oom"))
        .expect("oom fixture loads");

    let error = host
        .invoke(plugin_id, "run")
        .expect_err("oom fixture should trap");
    assert_eq!(error.code, "plugin_trapped");
    assert_eq!(
        host.plugin_state(plugin_id),
        Some(PluginRuntimeState::Crashed)
    );
    assert_audit_contains(&host, plugin_id, PluginAuditKind::Loaded);
    assert_audit_contains(&host, plugin_id, PluginAuditKind::Invoked);
    assert_audit_contains(&host, plugin_id, PluginAuditKind::Crashed);
}

#[test]
fn hostile_capability_probe_fixture_is_contained_and_audited() {
    let mut host = WasmPluginHost::new();
    let plugin_id = host
        .load_fixture(manifest(), compile_fixture("capability_probe"))
        .expect("probe fixture loads");

    let error = host
        .invoke(plugin_id, "run")
        .expect_err("probe fixture should fail closed when host_log is unavailable");
    assert_eq!(error.code, "plugin_trapped");
    assert_eq!(
        host.plugin_state(plugin_id),
        Some(PluginRuntimeState::Crashed)
    );
    assert_audit_contains(&host, plugin_id, PluginAuditKind::Loaded);
    assert_audit_contains(&host, plugin_id, PluginAuditKind::Invoked);
    assert_audit_contains(&host, plugin_id, PluginAuditKind::Crashed);
}

#[test]
fn hostile_workspace_access_fixture_is_denied_and_audited() {
    let mut host = WasmPluginHost::new();
    let error = host
        .load_fixture(manifest(), compile_fixture("workspace_access"))
        .expect_err("workspace access fixture should be denied before execution");
    assert_eq!(error.code, "plugin_wasi_import_denied");
    assert_eq!(
        error.message,
        "WASI imports are not granted to plugin fixtures"
    );
    assert_audit_contains(&host, PluginId(23), PluginAuditKind::Denied);
}
