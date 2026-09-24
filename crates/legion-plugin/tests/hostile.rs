//! Hostile-plugin containment tests.
//!
//! Every test here loads a real `.wasm` module — assembled from the WAT
//! fixtures in `fixtures/hostile/` and instantiated by Wasmtime — and asserts
//! two things: the attack is contained, and the audit log records the attempt.
//!
//! No fixture is allowlisted. Each one is refused on its merits by a host
//! guard, and each test names the specific error code the guard produces so a
//! different guard firing by accident cannot make the test pass.

use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
    time::Instant,
};

use legion_plugin::{PluginAuditEntry, PluginAuditKind, PluginRuntimeState, WasmPluginHost};
use legion_protocol::{
    CapabilityId, LanguageId, PluginActivationEvent, PluginCommandDescriptor, PluginContribution,
    PluginId, PluginManifest, PluginQuotaClass, PluginQuotaDeclaration, PluginStateNamespace,
    PluginTreeSitterGrammarContribution, PluginTrustDecision, PluginTrustMetadata,
    PluginTrustSource,
};

/// Quotas the hostile fixtures run under. All are well below the host ceiling,
/// so these exact numbers are what gets granted and what the tests assert on.
fn hostile_quotas() -> PluginQuotaDeclaration {
    PluginQuotaDeclaration {
        max_fuel: 1_000,
        max_wall_time_ms: 50,
        max_memory_pages: 8,
        max_storage_bytes: 4_096,
        max_host_calls: 4,
        max_events: 4,
        max_output_bytes: 512,
    }
}

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
        quotas: hostile_quotas(),
    }
}

/// A manifest that also holds the host-log capability, for fixtures whose
/// attack is not "call a function I was not granted".
fn manifest_with_host_log() -> PluginManifest {
    let mut manifest = manifest();
    manifest
        .requested_capabilities
        .push(CapabilityId(legion_plugin::HOST_LOG_CAPABILITY.to_string()));
    manifest
}

fn hostile_fixture_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("fixtures")
        .join("hostile")
        .join(format!("{name}.wat"))
}

fn unique_temp_wasm(label: &str) -> PathBuf {
    static SEQ: AtomicU64 = AtomicU64::new(0);
    // pid + monotonic seq, not the clock: macOS `as_nanos()` can collide when
    // cargo runs these tests in parallel, and a second writer truncating the
    // shared path leaves `load_fixture` compiling an empty file.
    std::env::temp_dir().join(format!(
        "legion-plugin-hostile-{label}-{}-{}.wasm",
        std::process::id(),
        SEQ.fetch_add(1, Ordering::Relaxed)
    ))
}

/// Assemble a hostile WAT fixture into a real `.wasm` file on disk.
///
/// The host reads and compiles these bytes exactly as it would a shipped
/// plugin artifact: nothing here models a plugin in Rust.
fn compile_fixture(name: &str) -> PathBuf {
    let source = fs::read_to_string(hostile_fixture_path(name)).expect("read hostile fixture");
    let path = unique_temp_wasm(name);
    let wasm = wat::parse_str(&source).expect("compile hostile fixture");
    assert_eq!(
        &wasm[..4],
        b"\0asm",
        "fixture {name} did not assemble to a WebAssembly module"
    );
    fs::write(&path, wasm).expect("write hostile fixture wasm");
    path
}

fn assert_audit_contains(host: &WasmPluginHost, plugin_id: PluginId, kind: PluginAuditKind) {
    let audit = host.audit_log(plugin_id);
    assert!(
        audit.iter().any(|entry| entry.kind == kind),
        "audit for plugin {plugin_id:?} did not contain {kind:?}: {audit:?}"
    );
}

/// Find the audit row that records a specific quota being hit.
fn quota_audit_row(
    host: &WasmPluginHost,
    plugin_id: PluginId,
    class: PluginQuotaClass,
) -> PluginAuditEntry {
    let audit = host.audit_log(plugin_id);
    audit
        .iter()
        .find(|entry| {
            entry.kind == PluginAuditKind::QuotaExceeded && entry.quota_class == Some(class)
        })
        .unwrap_or_else(|| {
            panic!("no QuotaExceeded audit row for {class:?} in audit log: {audit:?}")
        })
        .clone()
}

#[test]
fn an_endless_loop_is_cut_off_by_fuel_rather_than_hanging_the_host() {
    // The fixture never exits on its own. If this test returns at all, the
    // host stopped it.
    let mut host = WasmPluginHost::new();
    let plugin_id = host
        .load_fixture(manifest(), compile_fixture("infinite_loop"))
        .expect("loop fixture loads");

    let started = Instant::now();
    let error = host
        .invoke(plugin_id, "run")
        .expect_err("an infinite loop must not be allowed to return normally");

    assert_eq!(
        error.code, "plugin_fuel_quota_exceeded",
        "the loop must be stopped by the fuel quota, not by some other guard: {error:?}"
    );
    assert!(
        started.elapsed().as_secs() < 10,
        "containment took {:?}, which is not containment",
        started.elapsed()
    );
    assert_audit_contains(&host, plugin_id, PluginAuditKind::Crashed);

    // A plugin that burned through its CPU quota is taken out of service, not
    // handed a fresh budget on the next call.
    assert_eq!(
        host.plugin_state(plugin_id),
        Some(PluginRuntimeState::Disabled)
    );
    assert_eq!(
        host.invoke(plugin_id, "run")
            .expect_err("a disabled plugin must not run again")
            .code,
        "plugin_disabled"
    );
}

#[test]
fn the_wall_clock_deadline_is_derived_from_the_declared_quota() {
    // Both directions in one test, deliberately.
    //
    // Wasmtime's default epoch deadline is *already expired*: a store that
    // never calls `set_epoch_deadline` traps the guest immediately. So a test
    // that only checked "a plugin with a 0 ms budget is interrupted" would
    // pass even if the host never computed a deadline at all. It is the second
    // half — the same fixture, the same fuel, a real budget, and a normal
    // return — that shows the deadline actually tracks the declared quota.
    let same_fixture = compile_fixture("slow_loop");

    let mut expired = manifest();
    expired.quotas.max_fuel = 5_000_000;
    expired.quotas.max_wall_time_ms = 0;

    let mut host = WasmPluginHost::new();
    let plugin_id = host
        .load_fixture(expired, &same_fixture)
        .expect("slow loop fixture loads");
    let error = host
        .invoke(plugin_id, "run")
        .expect_err("a plugin past its deadline must be interrupted");
    assert_eq!(
        error.code, "plugin_wall_time_quota_exceeded",
        "expected the wall-clock deadline to fire, got {error:?}"
    );
    assert_audit_contains(&host, plugin_id, PluginAuditKind::Crashed);

    // Same module, same fuel, a budget it fits inside: it must finish.
    let mut generous = manifest();
    generous.plugin_id = PluginId(24);
    generous.storage_namespace.plugin_id = PluginId(24);
    generous.quotas.max_fuel = 5_000_000;
    generous.quotas.max_wall_time_ms = 2_000;

    let generous_id = host
        .load_fixture(generous, &same_fixture)
        .expect("slow loop fixture loads");
    assert_eq!(
        host.invoke(generous_id, "run")
            .expect("a plugin inside its deadline must be allowed to finish"),
        200_000
    );
}

#[test]
fn unbounded_allocation_is_refused_at_the_granted_page_ceiling() {
    // The fixture asks for 4096 pages against a granted 8. The WebAssembly
    // specification permits that request, so only the host's limiter can
    // refuse it.
    let mut host = WasmPluginHost::new();
    let plugin_id = host
        .load_fixture(manifest(), compile_fixture("oom"))
        .expect("oom fixture loads");

    let pages = host
        .invoke(plugin_id, "run")
        .expect("a refused memory.grow is a normal -1, not a crash");
    assert_eq!(
        pages,
        1,
        "the guest grew to {pages} pages against a granted ceiling of {}",
        host.granted_quotas(plugin_id)
            .expect("granted")
            .max_memory_pages
    );

    let row = quota_audit_row(&host, plugin_id, PluginQuotaClass::Memory);
    assert!(
        row.message.contains("4096") && row.message.contains("8 pages"),
        "the memory audit row must record what was asked for and what was granted: {row:?}"
    );
}

#[test]
fn a_capability_probe_is_denied_at_the_call_boundary_and_recorded() {
    // The manifest does not declare `plugin.event.emit`, so the host call is
    // refused. Crucially the refusal is a capability denial, not an incidental
    // link failure, and the audit names the capability that was missing.
    let mut host = WasmPluginHost::new();
    let plugin_id = host
        .load_fixture(manifest(), compile_fixture("capability_probe"))
        .expect("probe fixture loads: probing is a runtime act, not a load-time one");

    let error = host
        .invoke(plugin_id, "run")
        .expect_err("host_log must be denied to a plugin that never asked for it");
    assert_eq!(error.code, "plugin_capability_denied", "got {error:?}");

    let audit = host.audit_log(plugin_id);
    let denial = audit
        .iter()
        .find(|entry| entry.kind == PluginAuditKind::Denied)
        .unwrap_or_else(|| panic!("no Denied audit row recording the probe: {audit:?}"));
    assert!(
        denial.message.contains("env.host_log")
            && denial.message.contains(legion_plugin::HOST_LOG_CAPABILITY),
        "the audit row must name the host call and the missing capability: {denial:?}"
    );

    assert!(
        !audit
            .iter()
            .any(|entry| entry.kind == PluginAuditKind::HostCallAccepted),
        "a denied probe must not also be counted as an accepted host call: {audit:?}"
    );
    assert_eq!(
        host.plugin_state(plugin_id),
        Some(PluginRuntimeState::Disabled)
    );
}

#[test]
fn a_granted_host_call_still_runs_out_of_quota() {
    // Same host call, this time with the capability. The capability check no
    // longer applies, so the host-call counter is the only thing left to stop
    // the flood.
    //
    // The fuel budget is raised well past what 4096 iterations cost, so that
    // fuel exhaustion cannot stand in for the counter. Without this the test
    // would still pass with the host-call check deleted, because the guest
    // would run out of fuel instead.
    let mut host = WasmPluginHost::new();
    let mut manifest = manifest_with_host_log();
    manifest.quotas.max_fuel = 5_000_000;
    let plugin_id = host
        .load_fixture(manifest, compile_fixture("host_call_flood"))
        .expect("flood fixture loads");

    let error = host
        .invoke(plugin_id, "run")
        .expect_err("4096 host calls against a quota of 4 must be refused");
    assert_eq!(
        error.code, "plugin_host_call_quota_exceeded",
        "got {error:?}"
    );

    let audit = host.audit_log(plugin_id);
    let accepted = audit
        .iter()
        .filter(|entry| entry.kind == PluginAuditKind::HostCallAccepted)
        .count();
    assert_eq!(
        accepted, 4,
        "exactly the granted number of host calls may be accepted: {audit:?}"
    );
    quota_audit_row(&host, plugin_id, PluginQuotaClass::HostCall);
}

#[test]
fn an_oversized_host_call_payload_is_refused_before_it_is_read() {
    // The 1024-byte payload is inside guest memory, so the pointer bounds
    // check cannot be what refuses it. Only the output quota can. Without an
    // in-bounds payload this test would still pass with the output check
    // deleted, because the bounds check would refuse the call instead.
    let mut host = WasmPluginHost::new();
    let plugin_id = host
        .load_fixture(
            manifest_with_host_log(),
            compile_fixture("oversized_output"),
        )
        .expect("oversized fixture loads");

    let error = host
        .invoke(plugin_id, "run")
        .expect_err("a 1024 byte payload against a 512 byte ceiling must be refused");
    assert_eq!(error.code, "plugin_output_quota_exceeded", "got {error:?}");

    let row = quota_audit_row(&host, plugin_id, PluginQuotaClass::Output);
    assert!(
        row.message.contains("1024") && row.message.contains("512"),
        "the output audit row must record the attempted size: {row:?}"
    );
    assert!(
        !host
            .audit_log(plugin_id)
            .iter()
            .any(|entry| entry.kind == PluginAuditKind::HostCallAccepted),
        "a refused payload must not be counted as an accepted call"
    );
}

#[test]
fn a_guest_pointer_outside_guest_memory_is_refused() {
    // The guest holds the capability and is inside every count-based quota.
    // Its attack is the pointer, so the host must bounds-check it rather than
    // trusting the guest's arithmetic.
    let mut host = WasmPluginHost::new();
    let plugin_id = host
        .load_fixture(manifest_with_host_log(), compile_fixture("memory_escape"))
        .expect("memory escape fixture loads");

    let error = host
        .invoke(plugin_id, "run")
        .expect_err("a pointer past the end of guest memory must be refused");
    assert_eq!(
        error.code, "plugin_host_call_out_of_bounds",
        "got {error:?}"
    );
    assert_audit_contains(&host, plugin_id, PluginAuditKind::Denied);
}

#[test]
fn workspace_access_through_wasi_is_denied_before_the_guest_runs() {
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

    // Denied means not loaded. A refused module must leave nothing runnable
    // behind.
    assert_eq!(host.plugin_state(PluginId(23)), None);
    assert_eq!(
        host.invoke(PluginId(23), "run")
            .expect_err("a denied module must not be invocable")
            .code,
        "plugin_not_loaded"
    );
}

#[test]
fn workspace_access_renamed_away_from_wasi_is_still_denied() {
    // The import rule must be an allowlist of one function, not a blocklist of
    // WASI. This fixture asks for the same authority under a name no blocklist
    // would carry.
    let mut host = WasmPluginHost::new();
    let error = host
        .load_fixture(manifest(), compile_fixture("workspace_import_probe"))
        .expect_err("an unlisted host import must be denied whatever it is called");
    assert_eq!(error.code, "plugin_import_denied", "got {error:?}");
    assert!(
        error.message.contains("env.read_file"),
        "the denial must name the import that was refused: {error:?}"
    );
    assert_audit_contains(&host, PluginId(23), PluginAuditKind::Denied);
    assert_eq!(host.plugin_state(PluginId(23)), None);
}

#[test]
fn a_hostile_plugin_cannot_take_the_host_down_with_it() {
    // Crash containment: every hostile fixture in turn, in one host, and the
    // host is still able to load and run a well-behaved plugin afterwards.
    let mut host = WasmPluginHost::new();

    for (index, (fixture, holds_host_log)) in [
        ("infinite_loop", false),
        ("oom", false),
        ("capability_probe", false),
        ("host_call_flood", true),
        ("oversized_output", true),
        ("memory_escape", true),
    ]
    .into_iter()
    .enumerate()
    {
        let mut manifest = if holds_host_log {
            manifest_with_host_log()
        } else {
            manifest()
        };
        // Distinct ids so each fixture gets its own slot in the same host.
        let plugin_id = PluginId(100 + index as u64);
        manifest.plugin_id = plugin_id;
        manifest.storage_namespace.plugin_id = plugin_id;

        let loaded = host
            .load_fixture(manifest, compile_fixture(fixture))
            .expect("hostile fixture loads");
        let _ = host.invoke(loaded, "run");
        assert!(
            !host.audit_log(loaded).is_empty(),
            "fixture {fixture} produced no audit rows at all"
        );
    }

    let benign = unique_temp_wasm("benign");
    fs::write(
        &benign,
        wat::parse_str(r#"(module (func (export "run") (result i32) i32.const 42))"#)
            .expect("assemble benign fixture"),
    )
    .expect("write benign fixture");

    let mut manifest = manifest();
    manifest.plugin_id = PluginId(200);
    manifest.storage_namespace.plugin_id = PluginId(200);
    let plugin_id = host
        .load_fixture(manifest, &benign)
        .expect("the host still loads plugins after six hostile ones");
    assert_eq!(
        host.invoke(plugin_id, "run")
            .expect("the host still runs plugins after six hostile ones"),
        42
    );
}
