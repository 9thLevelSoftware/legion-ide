//! Sandboxed Wasmtime host for plugin fixtures.
//!
//! The runtime keeps host authority narrow: no WASI imports, a single audited
//! `env::host_log` capability for fixtures, capability validation via the
//! security broker, and fail-closed quota / trap handling.
//!
//! # Quotas are not optional
//!
//! Every quota is enforced by the runtime itself, not by cooperation from the
//! guest:
//!
//! * **Fuel** bounds CPU. [`wasmtime::Config::consume_fuel`] is enabled on the
//!   engine, so an infinite loop runs out of fuel and traps instead of hanging
//!   the host.
//! * **Wall time** bounds latency. Epoch interruption plus a ticker thread
//!   traps a guest that outlives its deadline even if it is blocked in a
//!   host call.
//! * **Memory** is bounded by a [`wasmtime::ResourceLimiter`] that refuses
//!   `memory.grow` past the granted page ceiling.
//! * **Host calls**, **output bytes**, and **invocations** are counted by the
//!   host, in host state the guest cannot reach.
//!
//! A plugin manifest is untrusted input, so the declared quotas are only ever a
//! *request*: [`legion_security::PluginQuotaCeiling::grant`] returns the
//! minimum of the request and the host ceiling. There is no per-plugin switch
//! that disables a quota, and a manifest that asks for more than the ceiling is
//! clamped with a [`PluginAuditKind::QuotaClamped`] audit row recording the
//! attempt.

use std::{
    collections::HashMap,
    fs,
    path::Path,
    sync::{Arc, Mutex},
    thread,
    time::Duration,
};

use legion_protocol::{
    CapabilityRequestContext, PluginId, PluginManifest, PluginQuotaClass, PluginQuotaDeclaration,
    PluginSandboxOperationClass, PrincipalId, ProtocolError, ProtocolResult,
    validate_plugin_manifest,
};
use legion_security::{DenyByDefaultBroker, PluginQuotaCeiling, TrustState};
use wasmtime::{Caller, Config, Engine, Linker, Module, ResourceLimiter, Store, Trap};

use crate::{PHASE5_PLUGIN_ABI_VERSION, PluginRuntimeState};

/// WebAssembly linear-memory page size.
const WASM_PAGE_BYTES: usize = 64 * 1024;

/// Interval at which the engine epoch advances, in milliseconds.
///
/// A guest's wall-clock deadline is expressed in whole ticks of this interval,
/// so it is the granularity of wall-time enforcement.
const EPOCH_TICK_MS: u64 = 10;

/// Capability a plugin must hold before the host will accept `env::host_log`.
///
/// This is checked at the call boundary rather than at load, so that a plugin
/// probing for the capability produces an audit row recording the attempt.
pub const HOST_LOG_CAPABILITY: &str = "plugin.event.emit";

/// Maximum WebAssembly tables a plugin instance may create.
const MAX_TABLES: usize = 4;
/// Maximum linear memories a plugin instance may create.
const MAX_MEMORIES: usize = 1;
/// Maximum instances a plugin store may create.
const MAX_INSTANCES: usize = 1;
/// Maximum elements in any plugin table.
const MAX_TABLE_ELEMENTS: usize = 4096;

/// Audit event kinds recorded by the sandboxed fixture host.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PluginAuditKind {
    /// Plugin manifest was loaded successfully.
    Loaded,
    /// A plugin invocation started.
    Invoked,
    /// A host call was accepted and counted against quota.
    HostCallAccepted,
    /// A host call exceeded quota.
    QuotaExceeded,
    /// A declared quota was reduced to the host ceiling.
    QuotaClamped,
    /// The guest trapped or otherwise crashed.
    Crashed,
    /// Invocation finished successfully.
    Completed,
    /// A load or host-call decision was denied.
    Denied,
}

/// Audit entry for a plugin lifecycle transition.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginAuditEntry {
    /// Plugin identifier.
    pub plugin_id: PluginId,
    /// Event kind.
    pub kind: PluginAuditKind,
    /// Quota dimension this entry concerns, when it concerns one.
    pub quota_class: Option<PluginQuotaClass>,
    /// Human-readable audit message.
    pub message: String,
}

type AuditLog = Arc<Mutex<Vec<PluginAuditEntry>>>;

fn push_audit(
    audit: &AuditLog,
    plugin_id: PluginId,
    kind: PluginAuditKind,
    quota_class: Option<PluginQuotaClass>,
    message: impl Into<String>,
) {
    audit.lock().expect("audit lock").push(PluginAuditEntry {
        plugin_id,
        kind,
        quota_class,
        message: message.into(),
    });
}

/// Per-invocation host state. The guest has no way to reach or mutate this.
struct InvocationState {
    plugin_id: PluginId,
    audit: AuditLog,
    /// Quotas already clamped to the host ceiling.
    quotas: PluginQuotaDeclaration,
    /// Host calls made so far in this invocation.
    host_calls_used: u32,
    /// Whether the manifest holds [`HOST_LOG_CAPABILITY`].
    host_log_granted: bool,
    /// Set when the host refused an operation, so the trap that follows can be
    /// reported with its real cause instead of a generic trap.
    refusal: Option<ProtocolError>,
}

impl InvocationState {
    fn refuse(
        &mut self,
        kind: PluginAuditKind,
        quota_class: Option<PluginQuotaClass>,
        code: &str,
        message: String,
    ) -> wasmtime::Error {
        push_audit(
            &self.audit,
            self.plugin_id,
            kind,
            quota_class,
            message.clone(),
        );
        if self.refusal.is_none() {
            self.refusal = Some(ProtocolError {
                code: code.to_string(),
                message: message.clone(),
            });
        }
        wasmtime::Error::msg(message)
    }
}

impl ResourceLimiter for InvocationState {
    fn memory_growing(
        &mut self,
        _current: usize,
        desired: usize,
        _maximum: Option<usize>,
    ) -> wasmtime::Result<bool> {
        let ceiling_bytes = (self.quotas.max_memory_pages as usize).saturating_mul(WASM_PAGE_BYTES);
        if desired > ceiling_bytes {
            let desired_pages = desired.div_ceil(WASM_PAGE_BYTES);
            push_audit(
                &self.audit,
                self.plugin_id,
                PluginAuditKind::QuotaExceeded,
                Some(PluginQuotaClass::Memory),
                format!(
                    "memory growth to {desired_pages} pages refused; granted ceiling is {} pages",
                    self.quotas.max_memory_pages
                ),
            );
            if self.refusal.is_none() {
                self.refusal = Some(ProtocolError {
                    code: "plugin_memory_quota_exceeded".to_string(),
                    message: format!(
                        "plugin memory growth to {desired_pages} pages exceeds the granted ceiling of {} pages",
                        self.quotas.max_memory_pages
                    ),
                });
            }
            return Ok(false);
        }
        Ok(true)
    }

    fn table_growing(
        &mut self,
        _current: usize,
        desired: usize,
        _maximum: Option<usize>,
    ) -> wasmtime::Result<bool> {
        if desired > MAX_TABLE_ELEMENTS {
            push_audit(
                &self.audit,
                self.plugin_id,
                PluginAuditKind::QuotaExceeded,
                Some(PluginQuotaClass::Memory),
                format!(
                    "table growth to {desired} elements refused; ceiling is {MAX_TABLE_ELEMENTS}"
                ),
            );
            return Ok(false);
        }
        Ok(true)
    }

    fn instances(&self) -> usize {
        MAX_INSTANCES
    }

    fn tables(&self) -> usize {
        MAX_TABLES
    }

    fn memories(&self) -> usize {
        MAX_MEMORIES
    }
}

#[derive(Debug)]
struct LoadedPlugin {
    manifest: PluginManifest,
    module: Module,
    state: PluginRuntimeState,
    audit: AuditLog,
    /// Quotas granted by the host: `min(manifest request, host ceiling)`.
    granted_quotas: PluginQuotaDeclaration,
    /// Whether the manifest was granted [`HOST_LOG_CAPABILITY`].
    host_log_granted: bool,
    /// Invocations consumed against `max_events`.
    invocations_used: u32,
}

/// Minimal Wasmtime-backed host for plugin fixtures.
#[derive(Debug)]
pub struct WasmPluginHost {
    engine: Engine,
    plugins: HashMap<PluginId, LoadedPlugin>,
    rejected_audit: HashMap<PluginId, Vec<PluginAuditEntry>>,
    broker: DenyByDefaultBroker,
    quota_ceiling: PluginQuotaCeiling,
}

impl Default for WasmPluginHost {
    fn default() -> Self {
        Self::new()
    }
}

impl WasmPluginHost {
    /// Construct a new sandboxed host.
    ///
    /// The engine is configured with fuel metering and epoch interruption, and
    /// a ticker thread advances the epoch so wall-clock deadlines fire. The
    /// ticker holds only a weak engine reference and exits when the host is
    /// dropped.
    pub fn new() -> Self {
        let mut config = Config::new();
        config.consume_fuel(true);
        config.epoch_interruption(true);
        config.wasm_threads(false);
        let engine = Engine::new(&config).expect("wasmtime engine");

        let ticker = engine.weak();
        thread::spawn(move || {
            loop {
                thread::sleep(Duration::from_millis(EPOCH_TICK_MS));
                match ticker.upgrade() {
                    Some(engine) => engine.increment_epoch(),
                    None => break,
                }
            }
        });

        Self {
            engine,
            plugins: HashMap::new(),
            rejected_audit: HashMap::new(),
            broker: DenyByDefaultBroker::default(),
            quota_ceiling: PluginQuotaCeiling::default(),
        }
    }

    /// The host quota ceiling applied to every plugin.
    ///
    /// There is intentionally no setter that widens it per plugin: the ceiling
    /// is a property of the host, and [`Self::load_fixture`] applies it to
    /// every manifest without exception.
    pub fn quota_ceiling(&self) -> PluginQuotaCeiling {
        self.quota_ceiling
    }

    /// The quotas actually granted to a loaded plugin, after clamping.
    pub fn granted_quotas(&self, plugin_id: PluginId) -> Option<PluginQuotaDeclaration> {
        self.plugins
            .get(&plugin_id)
            .map(|plugin| plugin.granted_quotas)
    }

    /// The quotas a loaded plugin's manifest asked for, before clamping.
    ///
    /// Paired with [`Self::granted_quotas`] this shows what a plugin requested
    /// against what it received.
    pub fn declared_quotas(&self, plugin_id: PluginId) -> Option<PluginQuotaDeclaration> {
        self.plugins
            .get(&plugin_id)
            .map(|plugin| plugin.manifest.quotas)
    }

    /// Load a fixture wasm file after manifest and import validation.
    pub fn load_fixture(
        &mut self,
        manifest: PluginManifest,
        wasm_path: impl AsRef<Path>,
    ) -> ProtocolResult<PluginId> {
        let plugin_id = manifest.plugin_id;
        validate_plugin_manifest(&manifest, PHASE5_PLUGIN_ABI_VERSION)?;
        if !matches!(
            manifest.trust.decision,
            legion_protocol::PluginTrustDecision::Trusted
                | legion_protocol::PluginTrustDecision::ExplicitlyAllowed
        ) {
            self.record_rejected(
                plugin_id,
                PluginAuditKind::Denied,
                None,
                "plugin manifest is not trusted for activation",
            );
            return Err(ProtocolError {
                code: "plugin_trust_denied".to_string(),
                message: "plugin manifest is not trusted for activation".to_string(),
            });
        }

        let bytes = fs::read(wasm_path).map_err(|error| ProtocolError {
            code: "plugin_fixture_missing".to_string(),
            message: format!("failed to read wasm fixture: {error}"),
        })?;
        let module = Module::new(&self.engine, &bytes).map_err(|error| ProtocolError {
            code: "plugin_module_invalid".to_string(),
            message: format!("failed to compile plugin fixture: {error}"),
        })?;

        for import in module.imports() {
            let import_module = import.module();
            let import_name = import.name();
            if import_module == "wasi_snapshot_preview1" {
                self.record_rejected(
                    plugin_id,
                    PluginAuditKind::Denied,
                    None,
                    "WASI imports are not granted to plugin fixtures",
                );
                return Err(ProtocolError {
                    code: "plugin_wasi_import_denied".to_string(),
                    message: "WASI imports are not granted to plugin fixtures".to_string(),
                });
            }
            if import_module != "env" || import_name != "host_log" {
                self.record_rejected(
                    plugin_id,
                    PluginAuditKind::Denied,
                    None,
                    format!("unsupported plugin import {import_module}.{import_name}"),
                );
                return Err(ProtocolError {
                    code: "plugin_import_denied".to_string(),
                    message: format!("unsupported plugin import {import_module}.{import_name}"),
                });
            }
        }

        if let Err(error) = self.validate_requested_capabilities(&manifest) {
            self.record_rejected(
                plugin_id,
                PluginAuditKind::Denied,
                None,
                error.message.clone(),
            );
            return Err(error);
        }

        // The manifest is untrusted input, so its quotas are a request. The
        // host grants the minimum of the request and its own ceiling.
        let grant = self.quota_ceiling.grant(&manifest.quotas);
        let audit: AuditLog = Arc::new(Mutex::new(Vec::new()));
        for clamp in &grant.clamps {
            push_audit(
                &audit,
                plugin_id,
                PluginAuditKind::QuotaClamped,
                Some(clamp.class),
                format!(
                    "manifest requested {:?} quota {} but the host granted {}",
                    clamp.class, clamp.declared, clamp.granted
                ),
            );
        }

        let host_log_granted = manifest
            .requested_capabilities
            .iter()
            .any(|capability| capability.0 == HOST_LOG_CAPABILITY);

        push_audit(
            &audit,
            plugin_id,
            PluginAuditKind::Loaded,
            None,
            "fixture manifest compiled and validated",
        );

        self.plugins.insert(
            plugin_id,
            LoadedPlugin {
                manifest,
                module,
                state: PluginRuntimeState::Loaded,
                audit,
                granted_quotas: grant.granted,
                host_log_granted,
                invocations_used: 0,
            },
        );
        Ok(plugin_id)
    }

    /// Invoke the exported guest function for a loaded plugin.
    ///
    /// The invocation runs under the granted fuel, wall-clock, and memory
    /// quotas. Every failure path is fail-closed: a trap, a quota exhaustion,
    /// and a capability refusal all return an error and leave an audit row.
    pub fn invoke(&mut self, plugin_id: PluginId, export_name: &str) -> ProtocolResult<i32> {
        let Some(plugin) = self.plugins.get(&plugin_id) else {
            return Err(ProtocolError {
                code: "plugin_not_loaded".to_string(),
                message: "plugin fixture is not loaded".to_string(),
            });
        };

        if plugin.state == PluginRuntimeState::Disabled {
            return Err(ProtocolError {
                code: "plugin_disabled".to_string(),
                message: "plugin was disabled after a quota or capability violation".to_string(),
            });
        }

        let module = plugin.module.clone();
        let audit = Arc::clone(&plugin.audit);
        let quotas = plugin.granted_quotas;
        let host_log_granted = plugin.host_log_granted;
        let invocations_used = plugin.invocations_used;

        if invocations_used >= quotas.max_events {
            self.disable(plugin_id);
            push_audit(
                &audit,
                plugin_id,
                PluginAuditKind::QuotaExceeded,
                Some(PluginQuotaClass::Event),
                format!("invocation quota of {} exhausted", quotas.max_events),
            );
            return Err(ProtocolError {
                code: "plugin_event_quota_exceeded".to_string(),
                message: "plugin invocation quota exceeded".to_string(),
            });
        }

        if let Some(plugin) = self.plugins.get_mut(&plugin_id) {
            plugin.state = PluginRuntimeState::Running;
            plugin.invocations_used = plugin.invocations_used.saturating_add(1);
        }
        push_audit(
            &audit,
            plugin_id,
            PluginAuditKind::Invoked,
            None,
            format!("invoking export {export_name}"),
        );

        let mut store = Store::new(
            &self.engine,
            InvocationState {
                plugin_id,
                audit: Arc::clone(&audit),
                quotas,
                host_calls_used: 0,
                host_log_granted,
                refusal: None,
            },
        );
        store.limiter(|state| state as &mut dyn ResourceLimiter);
        store
            .set_fuel(quotas.max_fuel)
            .map_err(|error| ProtocolError {
                code: "plugin_fuel_unavailable".to_string(),
                message: format!("failed to seed plugin fuel: {error}"),
            })?;

        let linker = Self::sandbox_linker(&self.engine);

        let outcome = (|| -> Result<i32, ProtocolError> {
            let instance = linker
                .instantiate(&mut store, &module)
                .map_err(|error| Self::classify(&mut store, error))?;
            let func = instance
                .get_typed_func::<(), i32>(&mut store, export_name)
                .map_err(|error| Self::classify(&mut store, error))?;
            // Set the wall-clock deadline immediately before entering the
            // guest so that compilation and instantiation do not eat the
            // plugin's budget.
            store.set_epoch_deadline(quotas.max_wall_time_ms.div_ceil(EPOCH_TICK_MS));
            func.call(&mut store, ())
                .map_err(|error| Self::classify(&mut store, error))
        })();

        let next_state = match &outcome {
            Ok(value) => {
                push_audit(
                    &audit,
                    plugin_id,
                    PluginAuditKind::Completed,
                    None,
                    format!("export {export_name} returned {value}"),
                );
                PluginRuntimeState::Idle
            }
            Err(error) => {
                push_audit(
                    &audit,
                    plugin_id,
                    PluginAuditKind::Crashed,
                    None,
                    format!(
                        "guest halted while invoking {export_name}: {}",
                        error.message
                    ),
                );
                // A plugin that broke a quota or reached for a capability it
                // was not granted is not merely crashed: it is hostile, and is
                // taken out of service rather than allowed to retry.
                if error.code == "plugin_trapped" {
                    PluginRuntimeState::Crashed
                } else {
                    PluginRuntimeState::Disabled
                }
            }
        };

        if let Some(plugin) = self.plugins.get_mut(&plugin_id) {
            plugin.state = next_state;
        }

        outcome
    }

    /// Build the linker holding the entire host surface a plugin can reach.
    ///
    /// Exactly one function is exposed. Everything else — WASI, filesystem,
    /// network, process control — is absent rather than denied at call time,
    /// so there is nothing to disable and nothing to allowlist.
    fn sandbox_linker(engine: &Engine) -> Linker<InvocationState> {
        let mut linker = Linker::new(engine);
        linker
            .func_wrap(
                "env",
                "host_log",
                |mut caller: Caller<'_, InvocationState>,
                 ptr: i32,
                 len: i32|
                 -> Result<(), wasmtime::Error> {
                    if !caller.data().host_log_granted {
                        let capability = HOST_LOG_CAPABILITY;
                        return Err(caller.data_mut().refuse(
                            PluginAuditKind::Denied,
                            None,
                            "plugin_capability_denied",
                            format!(
                                "host call env.host_log denied: plugin does not hold capability {capability}"
                            ),
                        ));
                    }

                    if len < 0 || u64::try_from(len).unwrap_or(u64::MAX) > caller.data().quotas.max_output_bytes {
                        let ceiling = caller.data().quotas.max_output_bytes;
                        return Err(caller.data_mut().refuse(
                            PluginAuditKind::QuotaExceeded,
                            Some(PluginQuotaClass::Output),
                            "plugin_output_quota_exceeded",
                            format!(
                                "host call env.host_log asked to read {len} bytes; granted output ceiling is {ceiling} bytes"
                            ),
                        ));
                    }

                    if caller.data().host_calls_used >= caller.data().quotas.max_host_calls {
                        let ceiling = caller.data().quotas.max_host_calls;
                        return Err(caller.data_mut().refuse(
                            PluginAuditKind::QuotaExceeded,
                            Some(PluginQuotaClass::HostCall),
                            "plugin_host_call_quota_exceeded",
                            format!("host-call quota of {ceiling} exhausted"),
                        ));
                    }

                    let Some(memory) = caller.get_export("memory").and_then(|e| e.into_memory())
                    else {
                        return Err(caller.data_mut().refuse(
                            PluginAuditKind::Denied,
                            None,
                            "plugin_memory_unavailable",
                            "host call env.host_log requires an exported linear memory".to_string(),
                        ));
                    };
                    let start = usize::try_from(ptr).unwrap_or(usize::MAX);
                    let length = usize::try_from(len).unwrap_or(usize::MAX);
                    let in_bounds = start
                        .checked_add(length)
                        .is_some_and(|end| end <= memory.data_size(&caller));
                    if !in_bounds {
                        return Err(caller.data_mut().refuse(
                            PluginAuditKind::Denied,
                            None,
                            "plugin_host_call_out_of_bounds",
                            format!(
                                "host call env.host_log referenced {length} bytes at {start}, outside guest memory"
                            ),
                        ));
                    }

                    let state = caller.data_mut();
                    state.host_calls_used = state.host_calls_used.saturating_add(1);
                    let used = state.host_calls_used;
                    push_audit(
                        &state.audit.clone(),
                        state.plugin_id,
                        PluginAuditKind::HostCallAccepted,
                        Some(PluginQuotaClass::HostCall),
                        format!("host call env.host_log accepted ({length} bytes, call {used})"),
                    );
                    Ok(())
                },
            )
            .expect("host_log is definable");
        linker
    }

    /// Map a wasmtime failure onto the host's error vocabulary.
    ///
    /// A refusal recorded by the host during the call wins over the generic
    /// trap it produced, so a capability denial is never reported as an
    /// ordinary crash.
    fn classify(store: &mut Store<InvocationState>, error: wasmtime::Error) -> ProtocolError {
        if let Some(refusal) = store.data_mut().refusal.take() {
            return refusal;
        }
        match error.downcast_ref::<Trap>() {
            Some(Trap::OutOfFuel) => ProtocolError {
                code: "plugin_fuel_quota_exceeded".to_string(),
                message: format!(
                    "plugin exhausted its fuel quota of {} units",
                    store.data().quotas.max_fuel
                ),
            },
            Some(Trap::Interrupt) => ProtocolError {
                code: "plugin_wall_time_quota_exceeded".to_string(),
                message: format!(
                    "plugin exceeded its wall-clock quota of {} ms",
                    store.data().quotas.max_wall_time_ms
                ),
            },
            _ => ProtocolError {
                code: "plugin_trapped".to_string(),
                message: format!("plugin trapped: {error}"),
            },
        }
    }

    /// Return a copy of the audit log for a plugin.
    pub fn audit_log(&self, plugin_id: PluginId) -> Vec<PluginAuditEntry> {
        if let Some(plugin) = self.plugins.get(&plugin_id) {
            plugin.audit.lock().expect("audit lock").clone()
        } else {
            self.rejected_audit
                .get(&plugin_id)
                .cloned()
                .unwrap_or_default()
        }
    }

    /// Return the tracked plugin runtime state.
    pub fn plugin_state(&self, plugin_id: PluginId) -> Option<PluginRuntimeState> {
        self.plugins.get(&plugin_id).map(|plugin| plugin.state)
    }

    fn disable(&mut self, plugin_id: PluginId) {
        if let Some(plugin) = self.plugins.get_mut(&plugin_id) {
            plugin.state = PluginRuntimeState::Disabled;
        }
    }

    fn validate_requested_capabilities(&mut self, manifest: &PluginManifest) -> ProtocolResult<()> {
        for capability in &manifest.requested_capabilities {
            let decision = self.broker.decide_with_request_context(
                TrustState::Trusted,
                PrincipalId(format!("plugin:{}", manifest.plugin_id.0)),
                capability.clone(),
                None,
                CapabilityRequestContext {
                    plugin_namespace: Some(legion_protocol::CapabilityNamespace(format!(
                        "plugin.{}",
                        manifest.plugin_id.0
                    ))),
                    plugin_id: Some(manifest.plugin_id),
                    plugin_host_call_name: Some("load_fixture".to_string()),
                    plugin_module_hash: Some(manifest.module_hash.clone()),
                    plugin_manifest_id: Some(manifest.manifest_id.clone()),
                    plugin_declared_capability_id: Some(capability.clone()),
                    plugin_quota_class: Some(PluginQuotaClass::HostCall),
                    plugin_sandbox_operation_class: Some(PluginSandboxOperationClass::HostCall),
                    ..Default::default()
                },
            );
            if let legion_security::SecurityDecision::Deny(message) = decision {
                return Err(ProtocolError {
                    code: "plugin_capability_denied".to_string(),
                    message,
                });
            }
        }
        Ok(())
    }

    fn record_rejected(
        &mut self,
        plugin_id: PluginId,
        kind: PluginAuditKind,
        quota_class: Option<PluginQuotaClass>,
        message: impl Into<String>,
    ) {
        self.rejected_audit
            .entry(plugin_id)
            .or_default()
            .push(PluginAuditEntry {
                plugin_id,
                kind,
                quota_class,
                message: message.into(),
            });
    }
}
