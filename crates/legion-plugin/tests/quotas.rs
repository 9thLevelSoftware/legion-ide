//! Quota enforcement, crash containment, capability denial, and audit.
//!
//! Every test loads a real `.wasm` module and runs it under Wasmtime. The
//! central claim under test is that a plugin manifest is a *request*, never a
//! grant: whatever a manifest declares, the host applies its own ceiling and
//! there is no per-plugin path that turns a quota off.

use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
    time::Instant,
};

use legion_plugin::{PluginAuditKind, PluginRuntimeState, WasmPluginHost};
use legion_protocol::{
    CapabilityId, LanguageId, PluginActivationEvent, PluginCommandDescriptor, PluginContribution,
    PluginId, PluginManifest, PluginQuotaClass, PluginQuotaDeclaration,
    PluginTreeSitterGrammarContribution, PluginTrustDecision, PluginTrustMetadata,
    PluginTrustSource,
};
use legion_security::PluginQuotaCeiling;

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
            max_fuel: 1_000,
            max_wall_time_ms: 50,
            max_memory_pages: 8,
            max_storage_bytes: 4_096,
            max_host_calls,
            max_events: 4,
            max_output_bytes: 64,
        },
    }
}

fn write_fixture_wasm(name: &str, wat_source: &str) -> PathBuf {
    static SEQ: AtomicU64 = AtomicU64::new(0);
    let path = std::env::temp_dir().join(format!(
        "legion-plugin-{name}-{}-{}.wasm",
        std::process::id(),
        SEQ.fetch_add(1, Ordering::Relaxed)
    ));
    let wasm = wat::parse_str(wat_source).expect("compile fixture wat to wasm");
    assert_eq!(
        &wasm[..4],
        b"\0asm",
        "fixture {name} did not assemble to a WebAssembly module"
    );
    fs::write(&path, wasm).expect("write wasm fixture");
    path
}

fn hostile_fixture(name: &str) -> PathBuf {
    let source = fs::read_to_string(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("fixtures")
            .join("hostile")
            .join(format!("{name}.wat")),
    )
    .expect("read hostile fixture");
    write_fixture_wasm(name, &source)
}

const BENIGN: &str = r#"
    (module
      (func (export "run") (result i32)
        i32.const 7))
"#;

// ---------------------------------------------------------------------------
// Acceptance: a fixture .wasm loads, runs, and cannot escape permissions.
// ---------------------------------------------------------------------------

#[test]
fn a_real_wasm_fixture_loads_runs_and_is_audited() {
    let wasm_path = write_fixture_wasm("audit", BENIGN);

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
    for expected in [
        PluginAuditKind::Loaded,
        PluginAuditKind::Invoked,
        PluginAuditKind::Completed,
    ] {
        assert!(
            audit.iter().any(|entry| entry.kind == expected),
            "audit did not record {expected:?}: {audit:?}"
        );
    }
}

// ---------------------------------------------------------------------------
// Stop condition: no quota may be silently disabled per plugin.
// ---------------------------------------------------------------------------

#[test]
fn a_manifest_asking_for_unlimited_quotas_is_granted_the_host_ceiling() {
    // The manifest is the only per-plugin quota surface that exists. If it
    // could raise its own limits, every quota would be optional. Ask for
    // everything and check what actually arrives.
    let wasm_path = write_fixture_wasm("greedy", BENIGN);

    let mut manifest = manifest(1);
    manifest.quotas = PluginQuotaDeclaration {
        max_fuel: u64::MAX,
        max_wall_time_ms: u64::MAX,
        max_memory_pages: u32::MAX,
        max_storage_bytes: u64::MAX,
        max_host_calls: u32::MAX,
        max_events: u32::MAX,
        max_output_bytes: u64::MAX,
    };

    let mut host = WasmPluginHost::new();
    let ceiling = host.quota_ceiling();
    let plugin_id = host.load_fixture(manifest, &wasm_path).expect("loads");

    let granted = host.granted_quotas(plugin_id).expect("granted quotas");
    assert_eq!(granted.max_fuel, ceiling.max_fuel);
    assert_eq!(granted.max_wall_time_ms, ceiling.max_wall_time_ms);
    assert_eq!(granted.max_memory_pages, ceiling.max_memory_pages);
    assert_eq!(granted.max_storage_bytes, ceiling.max_storage_bytes);
    assert_eq!(granted.max_host_calls, ceiling.max_host_calls);
    assert_eq!(granted.max_events, ceiling.max_events);
    assert_eq!(granted.max_output_bytes, ceiling.max_output_bytes);

    // The declared values are kept for audit, so the gap is visible.
    assert_eq!(
        host.declared_quotas(plugin_id).expect("declared").max_fuel,
        u64::MAX
    );
}

#[test]
fn a_clamped_quota_is_recorded_in_the_audit_rather_than_applied_silently() {
    let wasm_path = write_fixture_wasm("greedy-audit", BENIGN);

    let mut manifest = manifest(1);
    manifest.quotas.max_fuel = u64::MAX;
    manifest.quotas.max_memory_pages = u32::MAX;

    let mut host = WasmPluginHost::new();
    let plugin_id = host.load_fixture(manifest, &wasm_path).expect("loads");

    let audit = host.audit_log(plugin_id);
    let clamps: Vec<_> = audit
        .iter()
        .filter(|entry| entry.kind == PluginAuditKind::QuotaClamped)
        .collect();
    assert_eq!(
        clamps.len(),
        2,
        "expected one audit row per clamped dimension: {audit:?}"
    );
    assert!(
        clamps
            .iter()
            .any(|entry| entry.quota_class == Some(PluginQuotaClass::Fuel)),
        "the fuel clamp was applied without an audit row: {audit:?}"
    );
    assert!(
        clamps
            .iter()
            .any(|entry| entry.quota_class == Some(PluginQuotaClass::Memory)),
        "the memory clamp was applied without an audit row: {audit:?}"
    );
}

#[test]
fn a_manifest_declaring_unlimited_fuel_is_still_stopped_mid_loop() {
    // The end-to-end version of the stop condition: the clamp is not just a
    // number in a struct, it is what actually cuts the guest off.
    let mut manifest = manifest(1);
    manifest.quotas.max_fuel = u64::MAX;
    manifest.quotas.max_wall_time_ms = u64::MAX;

    let mut host = WasmPluginHost::new();
    let plugin_id = host
        .load_fixture(manifest, hostile_fixture("infinite_loop"))
        .expect("loads");

    let started = Instant::now();
    let error = host
        .invoke(plugin_id, "run")
        .expect_err("a manifest cannot buy itself unlimited CPU");
    assert!(
        error.code == "plugin_fuel_quota_exceeded"
            || error.code == "plugin_wall_time_quota_exceeded",
        "expected a quota to stop the loop, got {error:?}"
    );
    assert!(
        started.elapsed().as_secs() < 20,
        "containment took {:?}",
        started.elapsed()
    );
}

#[test]
fn a_manifest_declaring_unlimited_memory_is_still_held_to_the_page_ceiling() {
    let mut manifest = manifest(1);
    manifest.quotas.max_memory_pages = u32::MAX;

    let mut host = WasmPluginHost::new();
    let ceiling = host.quota_ceiling().max_memory_pages;
    let plugin_id = host
        .load_fixture(manifest, hostile_fixture("oom"))
        .expect("loads");

    // The fixture asks for 4096 pages; the host ceiling is far below that.
    assert!(
        ceiling < 4096,
        "ceiling of {ceiling} would not test anything"
    );
    let pages = host
        .invoke(plugin_id, "run")
        .expect("grow is refused, not fatal");
    assert_eq!(
        pages, 1,
        "a manifest declaring u32::MAX pages was granted {pages} pages"
    );
}

#[test]
fn the_host_ceiling_is_read_only_from_outside_the_crate() {
    // There is no setter. If one is ever added, this test is the place the
    // reviewer will be reminded that quotas are host-owned, not per-plugin.
    let host = WasmPluginHost::new();
    assert_eq!(host.quota_ceiling(), PluginQuotaCeiling::default());
    let bounded = PluginQuotaCeiling::default();
    assert!(bounded.max_fuel <= PluginQuotaCeiling::HARD_MAX.max_fuel);
    assert!(bounded.max_memory_pages <= PluginQuotaCeiling::HARD_MAX.max_memory_pages);
}

// ---------------------------------------------------------------------------
// Individual quota enforcement.
// ---------------------------------------------------------------------------

#[test]
fn the_host_call_quota_stops_a_plugin_that_holds_the_capability() {
    let mut manifest = manifest(2);
    manifest
        .requested_capabilities
        .push(CapabilityId(legion_plugin::HOST_LOG_CAPABILITY.to_string()));
    manifest.quotas.max_output_bytes = 64;
    // Generous fuel, so that running out of CPU cannot stand in for the
    // host-call counter being what stops the flood.
    manifest.quotas.max_fuel = 5_000_000;

    let mut host = WasmPluginHost::new();
    let plugin_id = host
        .load_fixture(manifest, hostile_fixture("host_call_flood"))
        .expect("fixture loads");

    let error = host
        .invoke(plugin_id, "run")
        .expect_err("the host-call quota must be reached");
    assert_eq!(error.code, "plugin_host_call_quota_exceeded");

    let audit = host.audit_log(plugin_id);
    assert_eq!(
        audit
            .iter()
            .filter(|entry| entry.kind == PluginAuditKind::HostCallAccepted)
            .count(),
        2,
        "only the granted host calls may be accepted: {audit:?}"
    );
    assert!(
        audit
            .iter()
            .any(|entry| entry.kind == PluginAuditKind::QuotaExceeded
                && entry.quota_class == Some(PluginQuotaClass::HostCall)),
        "the host-call quota breach was not audited: {audit:?}"
    );
    // A plugin that ran a quota to exhaustion does not get a second budget.
    assert_eq!(
        host.plugin_state(plugin_id),
        Some(PluginRuntimeState::Disabled)
    );
}

#[test]
fn the_invocation_quota_bounds_how_often_a_plugin_may_run() {
    let wasm_path = write_fixture_wasm("events", BENIGN);
    let mut manifest = manifest(1);
    manifest.quotas.max_events = 3;

    let mut host = WasmPluginHost::new();
    let plugin_id = host.load_fixture(manifest, &wasm_path).expect("loads");

    for attempt in 0..3 {
        assert_eq!(
            host.invoke(plugin_id, "run").expect("within quota"),
            7,
            "invocation {attempt} should have been inside the quota"
        );
    }
    let error = host
        .invoke(plugin_id, "run")
        .expect_err("the fourth invocation exceeds a quota of three");
    assert_eq!(error.code, "plugin_event_quota_exceeded");
    assert_eq!(
        host.plugin_state(plugin_id),
        Some(PluginRuntimeState::Disabled)
    );
    assert!(
        host.audit_log(plugin_id)
            .iter()
            .any(|entry| entry.quota_class == Some(PluginQuotaClass::Event)),
        "the invocation quota breach was not audited"
    );
}

// ---------------------------------------------------------------------------
// Capability denial.
// ---------------------------------------------------------------------------

#[test]
fn a_wasi_import_is_denied_at_load() {
    let wasm_path = write_fixture_wasm(
        "wasi-deny",
        r#"
        (module
          (import "wasi_snapshot_preview1" "fd_write"
            (func $fd_write (param i32 i32 i32 i32) (result i32)))
          (func (export "run") (result i32)
            i32.const 0))
    "#,
    );

    let mut host = WasmPluginHost::new();
    let error = host
        .load_fixture(manifest(1), &wasm_path)
        .expect_err("WASI imports are not granted");
    assert_eq!(error.code, "plugin_wasi_import_denied");
    assert!(
        host.audit_log(PluginId(7))
            .iter()
            .any(|entry| entry.kind == PluginAuditKind::Denied),
        "the denial was not audited"
    );
}

#[test]
fn an_undeclared_capability_is_denied_at_load() {
    let wasm_path = write_fixture_wasm("network-deny", BENIGN);

    let mut host = WasmPluginHost::new();
    let mut manifest = manifest(1);
    manifest
        .requested_capabilities
        .push(CapabilityId("plugin.network".to_string()));

    let error = host
        .load_fixture(manifest, &wasm_path)
        .expect_err("network capability should be denied");
    assert_eq!(error.code, "plugin_capability_denied");
    assert_eq!(host.plugin_state(PluginId(7)), None);
}

#[test]
fn a_host_call_is_denied_without_its_capability_even_though_the_import_links() {
    // The import resolves, so linking is not what refuses the call. The
    // capability check at the call boundary is.
    let mut host = WasmPluginHost::new();
    let plugin_id = host
        .load_fixture(manifest(4), hostile_fixture("capability_probe"))
        .expect("the module links; the call is what gets refused");

    let error = host
        .invoke(plugin_id, "run")
        .expect_err("host_log without plugin.event.emit must be denied");
    assert_eq!(error.code, "plugin_capability_denied");

    // Grant the capability and the very same module succeeds. Without this
    // control the test above would pass even if host_log were simply broken.
    let mut granted_manifest = manifest(4);
    granted_manifest.plugin_id = PluginId(8);
    granted_manifest.storage_namespace.plugin_id = PluginId(8);
    granted_manifest
        .requested_capabilities
        .push(CapabilityId(legion_plugin::HOST_LOG_CAPABILITY.to_string()));

    let granted_id = host
        .load_fixture(granted_manifest, hostile_fixture("capability_probe"))
        .expect("loads");
    assert_eq!(
        host.invoke(granted_id, "run")
            .expect("the same module runs once the capability is held"),
        0
    );
    assert!(
        host.audit_log(granted_id)
            .iter()
            .any(|entry| entry.kind == PluginAuditKind::HostCallAccepted),
        "the accepted host call was not audited"
    );
}

// ---------------------------------------------------------------------------
// Crash containment.
// ---------------------------------------------------------------------------

#[test]
fn a_guest_trap_is_contained_and_leaves_the_host_usable() {
    let wasm_path = write_fixture_wasm(
        "trap",
        r#"
        (module
          (func (export "run") (result i32)
            unreachable))
    "#,
    );

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
    assert!(
        host.audit_log(plugin_id)
            .iter()
            .any(|entry| entry.kind == PluginAuditKind::Crashed)
    );

    // An ordinary trap is a bug, not an attack: the plugin may try again, and
    // a fresh store means it starts from a clean state.
    let second = host
        .invoke(plugin_id, "run")
        .expect_err("still traps, still contained");
    assert_eq!(second.code, "plugin_trapped");
}

#[test]
fn a_missing_export_is_reported_rather_than_panicking_the_host() {
    let wasm_path = write_fixture_wasm("no-export", BENIGN);

    let mut host = WasmPluginHost::new();
    let plugin_id = host
        .load_fixture(manifest(1), &wasm_path)
        .expect("fixture loads");

    let error = host
        .invoke(plugin_id, "does_not_exist")
        .expect_err("a missing export is an error, not a crash");
    assert_eq!(error.code, "plugin_trapped");
    assert_eq!(host.invoke(plugin_id, "run").expect("host still works"), 7);
}
