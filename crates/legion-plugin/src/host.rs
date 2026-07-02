//! Sandboxed Wasmtime host for plugin fixtures.
//!
//! The runtime keeps host authority narrow: no WASI imports, a single audited
//! `env::host_log` capability for fixtures, capability validation via the
//! security broker, and fail-closed quota / trap handling.

use std::{
    collections::HashMap,
    fs,
    path::Path,
    sync::{Arc, Mutex},
};

use legion_protocol::{
    CapabilityRequestContext, PluginId, PluginManifest, PluginQuotaClass,
    PluginSandboxOperationClass, PrincipalId, ProtocolError, ProtocolResult,
    validate_plugin_manifest,
};
use legion_security::{DenyByDefaultBroker, TrustState};
use wasmtime::{Config, Engine, Linker, Module, Store};

use crate::{PHASE5_PLUGIN_ABI_VERSION, PluginRuntimeState};

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
    /// Human-readable audit message.
    pub message: String,
}

#[derive(Debug)]
struct LoadedPlugin {
    manifest: PluginManifest,
    module: Module,
    state: PluginRuntimeState,
    audit: Arc<Mutex<Vec<PluginAuditEntry>>>,
    host_calls_used: u32,
}

impl LoadedPlugin {
    fn new(manifest: PluginManifest, module: Module) -> Self {
        Self {
            manifest,
            module,
            state: PluginRuntimeState::Loaded,
            audit: Arc::new(Mutex::new(Vec::new())),
            host_calls_used: 0,
        }
    }
}

/// Minimal Wasmtime-backed host for plugin fixtures.
#[derive(Debug)]
pub struct WasmPluginHost {
    engine: Engine,
    plugins: HashMap<PluginId, LoadedPlugin>,
    rejected_audit: HashMap<PluginId, Vec<PluginAuditEntry>>,
    broker: DenyByDefaultBroker,
}

impl Default for WasmPluginHost {
    fn default() -> Self {
        let config = Config::new();
        let engine = Engine::new(&config).expect("wasmtime engine");
        Self {
            engine,
            plugins: HashMap::new(),
            rejected_audit: HashMap::new(),
            broker: DenyByDefaultBroker::default(),
        }
    }
}

impl WasmPluginHost {
    /// Construct a new sandboxed host.
    pub fn new() -> Self {
        Self::default()
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
                    format!("unsupported plugin import {import_module}.{import_name}"),
                );
                return Err(ProtocolError {
                    code: "plugin_import_denied".to_string(),
                    message: format!("unsupported plugin import {import_module}.{import_name}"),
                });
            }
        }

        let loaded = LoadedPlugin::new(manifest, module);
        if let Err(error) = self.validate_requested_capabilities(&loaded) {
            self.record_rejected(plugin_id, PluginAuditKind::Denied, error.message.clone());
            return Err(error);
        }
        self.audit_entry(
            loaded.manifest.plugin_id,
            PluginAuditKind::Loaded,
            "fixture manifest compiled and validated",
            &loaded.audit,
        );
        let plugin_id = loaded.manifest.plugin_id;
        self.plugins.insert(plugin_id, loaded);
        Ok(plugin_id)
    }

    /// Invoke the exported guest function for a loaded plugin.
    pub fn invoke(&mut self, plugin_id: PluginId, export_name: &str) -> ProtocolResult<i32> {
        let Some((module, audit, max_host_calls, used_host_calls)) =
            self.plugins.get(&plugin_id).map(|plugin| {
                (
                    plugin.module.clone(),
                    Arc::clone(&plugin.audit),
                    plugin.manifest.quotas.max_host_calls,
                    plugin.host_calls_used,
                )
            })
        else {
            return Err(ProtocolError {
                code: "plugin_not_loaded".to_string(),
                message: "plugin fixture is not loaded".to_string(),
            });
        };

        if used_host_calls >= max_host_calls {
            if let Some(plugin) = self.plugins.get_mut(&plugin_id) {
                plugin.state = PluginRuntimeState::Disabled;
            }
            Self::push_audit(
                &audit,
                plugin_id,
                PluginAuditKind::QuotaExceeded,
                "host-call quota exceeded",
            );
            return Err(ProtocolError {
                code: "plugin_host_call_quota_exceeded".to_string(),
                message: "plugin host-call quota exceeded".to_string(),
            });
        }

        if let Some(plugin) = self.plugins.get_mut(&plugin_id) {
            plugin.state = PluginRuntimeState::Running;
        }
        Self::push_audit(
            &audit,
            plugin_id,
            PluginAuditKind::Invoked,
            format!("invoking export {export_name}"),
        );

        let linker = Linker::new(&self.engine);
        let mut store = Store::new(&self.engine, ());

        let instance = linker
            .instantiate(&mut store, &module)
            .map_err(|error| Self::finish_trap(plugin_id, &audit, error))?;
        let func = instance
            .get_typed_func::<(), i32>(&mut store, export_name)
            .map_err(|error| Self::finish_trap(plugin_id, &audit, error))?;
        let result = func.call(&mut store, ());

        let (outcome, next_state) = match result {
            Ok(value) => {
                Self::push_audit(
                    &audit,
                    plugin_id,
                    PluginAuditKind::Completed,
                    format!("export {export_name} returned {value}"),
                );
                (Ok(value), PluginRuntimeState::Idle)
            }
            Err(error) => {
                Self::push_audit(
                    &audit,
                    plugin_id,
                    PluginAuditKind::Crashed,
                    format!("guest trapped while invoking {export_name}: {error}"),
                );
                (
                    Err(ProtocolError {
                        code: "plugin_trapped".to_string(),
                        message: format!("plugin trapped while running {export_name}: {error}"),
                    }),
                    PluginRuntimeState::Crashed,
                )
            }
        };

        if let Some(plugin) = self.plugins.get_mut(&plugin_id) {
            plugin.state = next_state;
            if outcome.is_ok() {
                plugin.host_calls_used = plugin.host_calls_used.saturating_add(1);
            }
        }

        outcome
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
        self.plugins.get(&plugin_id).map(|plugin| {
            if plugin.state == PluginRuntimeState::Running
                && plugin
                    .audit
                    .lock()
                    .expect("audit lock")
                    .iter()
                    .any(|entry| entry.kind == PluginAuditKind::Crashed)
            {
                PluginRuntimeState::Crashed
            } else {
                plugin.state
            }
        })
    }

    fn validate_requested_capabilities(&mut self, plugin: &LoadedPlugin) -> ProtocolResult<()> {
        for capability in &plugin.manifest.requested_capabilities {
            let decision = self.broker.decide_with_request_context(
                TrustState::Trusted,
                PrincipalId(format!("plugin:{}", plugin.manifest.plugin_id.0)),
                capability.clone(),
                None,
                CapabilityRequestContext {
                    plugin_namespace: Some(legion_protocol::CapabilityNamespace(format!(
                        "plugin.{}",
                        plugin.manifest.plugin_id.0
                    ))),
                    plugin_id: Some(plugin.manifest.plugin_id),
                    plugin_host_call_name: Some("load_fixture".to_string()),
                    plugin_module_hash: Some(plugin.manifest.module_hash.clone()),
                    plugin_manifest_id: Some(plugin.manifest.manifest_id.clone()),
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

    fn finish_trap(
        plugin_id: PluginId,
        audit: &Arc<Mutex<Vec<PluginAuditEntry>>>,
        error: impl std::fmt::Display,
    ) -> ProtocolError {
        Self::push_audit(
            audit,
            plugin_id,
            PluginAuditKind::Crashed,
            format!("guest trapped: {error}"),
        );
        ProtocolError {
            code: "plugin_trapped".to_string(),
            message: format!("plugin trapped: {error}"),
        }
    }

    fn audit_entry(
        &self,
        plugin_id: PluginId,
        kind: PluginAuditKind,
        message: impl Into<String>,
        audit: &Arc<Mutex<Vec<PluginAuditEntry>>>,
    ) {
        Self::push_audit(audit, plugin_id, kind, message);
    }

    fn record_rejected(
        &mut self,
        plugin_id: PluginId,
        kind: PluginAuditKind,
        message: impl Into<String>,
    ) {
        self.rejected_audit
            .entry(plugin_id)
            .or_default()
            .push(PluginAuditEntry {
                plugin_id,
                kind,
                message: message.into(),
            });
    }

    fn push_audit(
        audit: &Arc<Mutex<Vec<PluginAuditEntry>>>,
        plugin_id: PluginId,
        kind: PluginAuditKind,
        message: impl Into<String>,
    ) {
        audit.lock().expect("audit lock").push(PluginAuditEntry {
            plugin_id,
            kind,
            message: message.into(),
        });
    }
}
