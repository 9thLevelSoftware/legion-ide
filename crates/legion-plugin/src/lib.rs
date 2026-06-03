//! Phase 5 WASM plugin runtime boundary.
//!
//! This crate deliberately exposes protocol DTOs only. The current runtime slice
//! validates manifests, enforces capability/quota metadata before invocation, and
//! returns typed fail-closed responses without granting ambient host authority.

#![warn(missing_docs)]

use std::{
    collections::{HashMap, HashSet},
    sync::Mutex,
};

use legion_protocol::{
    CapabilityId, PluginDenialReason, PluginHostCallRequest, PluginHostCallResponse, PluginId,
    PluginManifest, PluginPort, PluginRequest, PluginResponse, PluginSandboxOperationClass,
    PluginStateNamespace, ProtocolError, ProtocolResult, validate_plugin_host_call_request,
    validate_plugin_manifest,
};
use legion_security::{DenyByDefaultBroker, TrustState};
use thiserror::Error;

/// Host ABI version accepted by this Phase 5 runtime slice.
pub const PHASE5_PLUGIN_ABI_VERSION: u16 = 1;

/// Runtime lifecycle state for isolated plugins.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PluginRuntimeState {
    /// Manifest discovered but not validated.
    Discovered,
    /// Manifest or trust policy rejected the plugin.
    Rejected,
    /// Plugin manifest is loaded.
    Loaded,
    /// Plugin activation completed.
    Activated,
    /// Plugin invocation is running.
    Running,
    /// Plugin is idle.
    Idle,
    /// Plugin cancellation requested.
    Cancelling,
    /// Plugin cancellation completed.
    Cancelled,
    /// Plugin trapped or crashed.
    Crashed,
    /// Plugin is disabled.
    Disabled,
    /// Plugin was unloaded.
    Unloaded,
}

/// Runtime errors converted to protocol diagnostics at host boundaries.
#[derive(Debug, Error)]
pub enum PluginRuntimeError {
    /// Manifest validation failed.
    #[error("manifest rejected: {0}")]
    ManifestRejected(String),
    /// Host call validation failed.
    #[error("host call rejected: {0}")]
    HostCallRejected(String),
}

/// Loaded plugin metadata tracked without retaining WASM memory or host objects.
#[derive(Debug, Clone)]
pub struct LoadedPlugin {
    /// Plugin manifest.
    pub manifest: PluginManifest,
    /// Runtime state.
    pub state: PluginRuntimeState,
    /// Capabilities declared by manifest.
    pub declared_capabilities: HashSet<CapabilityId>,
    /// Host calls consumed during the active invocation.
    pub host_calls_used: u32,
    /// Output bytes produced during the active invocation.
    pub output_bytes_used: u64,
}

impl LoadedPlugin {
    fn new(manifest: PluginManifest) -> Self {
        Self {
            declared_capabilities: manifest.requested_capabilities.iter().cloned().collect(),
            manifest,
            state: PluginRuntimeState::Loaded,
            host_calls_used: 0,
            output_bytes_used: 0,
        }
    }
}

/// Deny-by-default plugin host.
#[derive(Debug, Default)]
pub struct PluginRuntimeHost {
    plugins: HashMap<PluginId, LoadedPlugin>,
    broker: DenyByDefaultBroker,
}

impl PluginRuntimeHost {
    /// Construct an empty plugin runtime host.
    pub fn new() -> Self {
        Self::default()
    }

    /// Construct with an explicit capability broker.
    pub fn with_broker(broker: DenyByDefaultBroker) -> Self {
        Self {
            plugins: HashMap::new(),
            broker,
        }
    }

    /// Load a manifest after ABI/trust/capability metadata validation.
    pub fn load_manifest(&mut self, manifest: PluginManifest) -> ProtocolResult<PluginId> {
        validate_plugin_manifest(&manifest, PHASE5_PLUGIN_ABI_VERSION)?;
        if !matches!(
            manifest.trust.decision,
            legion_protocol::PluginTrustDecision::Trusted
                | legion_protocol::PluginTrustDecision::ExplicitlyAllowed
        ) {
            return Err(ProtocolError {
                code: "plugin_trust_denied".to_string(),
                message: "plugin manifest is not trusted for activation".to_string(),
            });
        }
        let plugin_id = manifest.plugin_id;
        self.plugins.insert(plugin_id, LoadedPlugin::new(manifest));
        Ok(plugin_id)
    }

    /// Invoke a metadata-only host call for a loaded plugin.
    pub fn dispatch_host_call(
        &mut self,
        request: PluginHostCallRequest,
    ) -> ProtocolResult<PluginHostCallResponse> {
        validate_plugin_host_call_request(&request)?;
        let Some(plugin) = self.plugins.get_mut(&request.plugin_id) else {
            return Ok(PluginHostCallResponse::Denied {
                reason: PluginDenialReason::UnsupportedHostCall,
                message: "plugin is not loaded".to_string(),
            });
        };

        if !plugin
            .declared_capabilities
            .contains(&request.declared_capability)
        {
            return Ok(PluginHostCallResponse::Denied {
                reason: PluginDenialReason::MissingCapability,
                message: "host call capability was not declared in manifest".to_string(),
            });
        }

        if plugin.host_calls_used >= plugin.manifest.quotas.max_host_calls {
            return Ok(PluginHostCallResponse::Denied {
                reason: PluginDenialReason::QuotaExceeded,
                message: "host-call quota exceeded".to_string(),
            });
        }

        if request.metadata_label.len() as u64 > plugin.manifest.quotas.max_output_bytes {
            return Ok(PluginHostCallResponse::Denied {
                reason: PluginDenialReason::QuotaExceeded,
                message: "bounded output quota exceeded".to_string(),
            });
        }

        let decision = self.broker.decide_with_request_context(
            TrustState::Trusted,
            legion_protocol::PrincipalId(format!("plugin:{}", request.plugin_id.0)),
            request.declared_capability.clone(),
            None,
            legion_protocol::CapabilityRequestContext {
                plugin_namespace: Some(legion_protocol::CapabilityNamespace(format!(
                    "plugin.{}",
                    request.plugin_id.0
                ))),
                plugin_id: Some(request.plugin_id),
                plugin_host_call_name: Some(request.host_call_name.clone()),
                plugin_module_hash: Some(plugin.manifest.module_hash.clone()),
                plugin_manifest_id: Some(plugin.manifest.manifest_id.clone()),
                plugin_declared_capability_id: Some(request.declared_capability.clone()),
                plugin_quota_class: Some(legion_protocol::PluginQuotaClass::HostCall),
                plugin_sandbox_operation_class: Some(PluginSandboxOperationClass::HostCall),
                ..Default::default()
            },
        );
        if let legion_security::SecurityDecision::Deny(message) = decision {
            return Ok(PluginHostCallResponse::Denied {
                reason: PluginDenialReason::MissingCapability,
                message,
            });
        }

        plugin.host_calls_used = plugin.host_calls_used.saturating_add(1);
        plugin.output_bytes_used = plugin
            .output_bytes_used
            .saturating_add(request.metadata_label.len() as u64);
        plugin.state = PluginRuntimeState::Idle;
        Ok(PluginHostCallResponse::Accepted {
            metadata_label: request.metadata_label,
        })
    }

    /// Return the loaded plugin state.
    pub fn plugin_state(&self, plugin_id: PluginId) -> Option<PluginRuntimeState> {
        self.plugins.get(&plugin_id).map(|plugin| plugin.state)
    }

    /// Unload a plugin deterministically.
    pub fn unload(&mut self, plugin_id: PluginId) -> bool {
        self.plugins
            .remove(&plugin_id)
            .map(|mut plugin| {
                plugin.state = PluginRuntimeState::Unloaded;
                true
            })
            .unwrap_or(false)
    }
}

/// Thread-safe protocol port for plugin runtime dispatch.
#[derive(Debug, Default)]
pub struct PluginRuntimePort {
    inner: Mutex<PluginRuntimeHost>,
}

impl PluginRuntimePort {
    /// Construct a protocol port around an empty runtime host.
    pub fn new() -> Self {
        Self::default()
    }

    /// Construct a protocol port around an existing runtime host.
    pub fn from_host(host: PluginRuntimeHost) -> Self {
        Self {
            inner: Mutex::new(host),
        }
    }

    fn lock(&self) -> ProtocolResult<std::sync::MutexGuard<'_, PluginRuntimeHost>> {
        self.inner.lock().map_err(|_| ProtocolError {
            code: "plugin_runtime_lock_poisoned".to_string(),
            message: "plugin runtime lock poisoned".to_string(),
        })
    }
}

impl PluginPort for PluginRuntimePort {
    fn handle(&self, request: PluginRequest) -> ProtocolResult<PluginResponse> {
        let mut host = self.lock()?;
        match request {
            PluginRequest::Manifest(manifest) => {
                host.load_manifest(manifest).map(PluginResponse::Loaded)
            }
            PluginRequest::CommandDescriptor(descriptor) => {
                Ok(PluginResponse::CommandRegistered(descriptor.command_id))
            }
            PluginRequest::Contribution(descriptor) => {
                Ok(PluginResponse::ContributionRegistered(descriptor.name))
            }
            PluginRequest::HostCall(request) => host
                .dispatch_host_call(request)
                .map(PluginResponse::HostCall),
        }
    }
}

/// Build a plugin namespace helper.
pub fn plugin_namespace(plugin_id: PluginId, namespace: impl Into<String>) -> PluginStateNamespace {
    PluginStateNamespace {
        plugin_id,
        namespace: namespace.into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use legion_protocol::{
        CapabilityId, CausalityId, CorrelationId, EventSequence, PluginActivationEvent,
        PluginCommandDescriptor, PluginContribution, PluginHostCallKind, PluginQuotaDeclaration,
        PluginTrustDecision, PluginTrustMetadata, PluginTrustSource,
    };
    use uuid::Uuid;

    fn manifest() -> PluginManifest {
        PluginManifest {
            plugin_id: PluginId(7),
            name: "phase5.test".to_string(),
            version: "0.1.0".to_string(),
            schema_version: 1,
            min_abi_version: 1,
            max_abi_version: 1,
            module_hash: "sha256:test".to_string(),
            manifest_id: "manifest-test".to_string(),
            trust: PluginTrustMetadata {
                source: PluginTrustSource::ExplicitLocalAllow,
                decision: PluginTrustDecision::ExplicitlyAllowed,
                reason: "test".to_string(),
            },
            signature: None,
            activation_events: vec![PluginActivationEvent::Startup],
            contributions: vec![PluginContribution::Command(PluginCommandDescriptor {
                command_id: "phase5.run".to_string(),
                title: "Phase 5 Run".to_string(),
                required_capability: CapabilityId("plugin.command".to_string()),
            })],
            requested_capabilities: vec![CapabilityId("plugin.command".to_string())],
            storage_namespace: plugin_namespace(PluginId(7), "state"),
            quotas: PluginQuotaDeclaration {
                max_fuel: 1000,
                max_wall_time_ms: 50,
                max_memory_pages: 8,
                max_storage_bytes: 4096,
                max_host_calls: 1,
                max_events: 4,
                max_output_bytes: 64,
            },
        }
    }

    fn host_call(capability: &str) -> PluginHostCallRequest {
        PluginHostCallRequest {
            plugin_id: PluginId(7),
            kind: PluginHostCallKind::ReadOnlyContext,
            host_call_name: "readContext".to_string(),
            declared_capability: CapabilityId(capability.to_string()),
            correlation_id: CorrelationId(1),
            causality_id: CausalityId(Uuid::from_u128(1)),
            sequence: EventSequence(1),
            metadata_label: "context-label".to_string(),
        }
    }

    #[test]
    fn plugin_runtime_loads_trusted_manifest() {
        let mut host = PluginRuntimeHost::new();
        let plugin_id = host.load_manifest(manifest()).expect("manifest loads");
        assert_eq!(plugin_id, PluginId(7));
        assert_eq!(
            host.plugin_state(plugin_id),
            Some(PluginRuntimeState::Loaded)
        );
    }

    #[test]
    fn plugin_runtime_denies_undeclared_host_call() {
        let mut host = PluginRuntimeHost::new();
        host.load_manifest(manifest()).expect("manifest loads");
        let response = host
            .dispatch_host_call(host_call("plugin.storage"))
            .expect("typed response");
        assert!(matches!(
            response,
            PluginHostCallResponse::Denied {
                reason: PluginDenialReason::MissingCapability,
                ..
            }
        ));
    }

    #[test]
    fn plugin_runtime_host_call_quota_fails_closed() {
        let mut host = PluginRuntimeHost::new();
        host.load_manifest(manifest()).expect("manifest loads");
        assert!(matches!(
            host.dispatch_host_call(host_call("plugin.command"))
                .expect("first call"),
            PluginHostCallResponse::Accepted { .. }
        ));
        assert!(matches!(
            host.dispatch_host_call(host_call("plugin.command"))
                .expect("quota response"),
            PluginHostCallResponse::Denied {
                reason: PluginDenialReason::QuotaExceeded,
                ..
            }
        ));
    }

    #[test]
    fn plugin_manifest_incompatible_abi_is_rejected_before_load() {
        let mut host = PluginRuntimeHost::new();
        let mut manifest = manifest();
        manifest.min_abi_version = 2;
        manifest.max_abi_version = 2;

        let error = host
            .load_manifest(manifest)
            .expect_err("ABI mismatch rejects");
        assert_eq!(error.code, "plugin_abi_mismatch");
    }

    #[test]
    fn plugin_sandbox_denies_invalid_core_ids_before_host_dispatch() {
        let mut host = PluginRuntimeHost::new();
        host.load_manifest(manifest()).expect("manifest loads");
        let mut request = host_call("plugin.command");
        request.correlation_id = CorrelationId(0);

        let error = host
            .dispatch_host_call(request)
            .expect_err("zero correlation is rejected");
        assert_eq!(error.code, "plugin_host_call_invalid");
    }

    #[test]
    fn plugin_runtime_port_dispatches_protocol_envelopes_only() {
        let port = PluginRuntimePort::new();
        let plugin_id = match port
            .handle(PluginRequest::Manifest(manifest()))
            .expect("manifest load through port")
        {
            PluginResponse::Loaded(plugin_id) => plugin_id,
            other => panic!("unexpected plugin response: {other:?}"),
        };
        assert_eq!(plugin_id, PluginId(7));

        let response = port
            .handle(PluginRequest::HostCall(host_call("plugin.command")))
            .expect("host call through port");
        assert!(matches!(
            response,
            PluginResponse::HostCall(PluginHostCallResponse::Accepted { .. })
        ));
    }
}
