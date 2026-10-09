//! App-owned named MCP lifecycle. No inference request or credential fallback.

use crate::AppComposition;
use legion_ai_providers::{
    McpClient, McpClientError, McpTransport, StreamableHttpMcpTransport,
    StreamableHttpMcpTransportConfig,
};
use legion_protocol::{
    McpJsonRpcEnvelope, McpListChangedKind, McpRegistrySnapshot, McpServerDescriptor,
    RedactionHint, TimestampMillis,
};
use legion_protocol::{McpServerId, McpTransportKind, named_mcp_peer::*};
use legion_storage::secrets::{SecretReference, SecretStore};
use serde_json::Value;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use thiserror::Error;

/// Explicit transport permissions reviewed for one exact peer revision.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NamedMcpPeerPermissions {
    /// Permit requests only to the selected endpoint; no redirects or proxy fallback.
    pub network: bool,
    /// Permit the declared process, subject to independently enforceable containment.
    pub process: bool,
}

impl NamedMcpPeerPermissions {
    /// Permit only network access to this revision's selected endpoint.
    pub fn network() -> Self {
        Self {
            network: true,
            process: false,
        }
    }
}

/// SecretStore binding for the reviewed peer, endpoint/launch, protocol, role,
/// authentication route and declared scopes. Changing that contract requires a
/// separately stored credential. Display-label edits do not rebind credentials.
/// The account contains a digest, never the endpoint or credential value.
pub fn named_mcp_peer_secret_reference(config: &NamedMcpPeerConfig) -> SecretReference {
    use sha2::{Digest, Sha256};
    let transport = match &config.transport {
        NamedMcpPeerTransport::Http { endpoint } => serde_json::json!({ "http": endpoint }),
        NamedMcpPeerTransport::Stdio { command, args } => {
            serde_json::json!({ "stdio": command, "args": args })
        }
    };
    let binding = serde_json::json!({
        "binding_version": 1,
        "peer": config.metadata.peer_id,
        "role": config.metadata.role,
        "protocol": config.metadata.protocol_version,
        "transport_kind": config.metadata.transport,
        "transport": transport,
        "authentication": config.metadata.authentication,
        "scopes": config.metadata.credential_scopes,
        "privacy": config.metadata.privacy,
    });
    let digest = hex::encode(Sha256::digest(binding.to_string().as_bytes()));
    SecretReference::new(
        "legion-mcp-peers",
        format!("{}:{digest}", config.metadata.peer_id.0),
    )
}

/// Operational transport configuration, excluded from retained metadata.
#[derive(Clone)]
pub enum NamedMcpPeerTransport {
    /// Existing HTTP transport endpoint.
    Http {
        /// Exact endpoint; never projected.
        endpoint: String,
    },
    /// Existing stdio transport launch declaration.
    Stdio {
        /// Executable, subject to process permission and containment.
        command: String,
        /// Explicit arguments; never projected.
        args: Vec<String>,
    },
}

/// Configuration input; Debug/serialization intentionally omit operational data.
#[derive(Clone)]
pub struct NamedMcpPeerConfig {
    /// Safe, reviewed peer metadata.
    pub metadata: McpPeerMetadata,
    /// Endpoint/launch details retained only in app memory.
    pub transport: NamedMcpPeerTransport,
}

impl NamedMcpPeerConfig {
    /// Declare an unauthenticated HTTP client pinned to the ratified protocol.
    pub fn http_client(
        peer_id: McpServerId,
        label: impl Into<String>,
        endpoint: impl Into<String>,
    ) -> Self {
        Self {
            metadata: McpPeerMetadata {
                peer_id,
                display_label: label.into(),
                role: McpPeerRole::Client,
                protocol_version: MCP_PEER_PROTOCOL_VERSION.into(),
                transport: McpTransportKind::StreamableHttp,
                authentication: McpPeerAuthentication::None,
                credential_scopes: vec![],
                privacy: McpPeerPrivacy::MetadataOnly,
            },
            transport: NamedMcpPeerTransport::Http {
                endpoint: endpoint.into(),
            },
        }
    }
}

/// Typed, redacted lifecycle failure. Raw transport/store errors are never projected.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum NamedMcpPeerError {
    /// Configuration violates the pinned role/version/transport contract.
    #[error("invalid named MCP peer configuration")]
    InvalidConfiguration,
    /// Unknown peer identity.
    #[error("named MCP peer is not configured")]
    UnknownPeer,
    /// Reviewed grant belongs to a different revision.
    #[error("named MCP peer revision is stale")]
    StaleRevision,
    /// Manual mode performs no endpoint/process activity, including loopback.
    #[error("Manual mode denies MCP activation")]
    ManualMode,
    /// Exact transport grant has not been approved.
    #[error("named MCP peer requires transport permission")]
    PermissionRequired,
    /// Required peer secret is missing or the store failed.
    #[error("named MCP peer credential unavailable")]
    CredentialUnavailable,
    /// No ratified containment/server activation contract exists for this path.
    #[error("named MCP peer execution environment is not qualified")]
    UnsupportedEnvironment,
    /// The selected endpoint did not complete a valid protocol request.
    #[error("named MCP peer endpoint unavailable")]
    EndpointUnavailable,
    /// Peer negotiated a revision outside the pinned contract.
    #[error("named MCP peer protocol mismatch")]
    ProtocolMismatch,
    /// Local grant/runtime has been revoked.
    #[error("named MCP peer grant revoked")]
    Revoked,
    /// SecretStore deletion failed, while local runtime/grant remain revoked.
    #[error("named MCP peer credential deletion failed; local grant revoked")]
    CredentialDeletionFailed,
}

pub(crate) struct NamedMcpPeer {
    config: NamedMcpPeerConfig,
    snapshot: McpPeerSnapshot,
    permissions: NamedMcpPeerPermissions,
    live_grant: Arc<AtomicBool>,
    client: Option<McpClient<GuardedMcpTransport>>,
}

#[derive(Clone)]
struct GuardedMcpTransport {
    transport: StreamableHttpMcpTransport,
    live_grant: Arc<AtomicBool>,
}

impl McpTransport for GuardedMcpTransport {
    fn send(&self, envelope: &McpJsonRpcEnvelope) -> Result<Value, McpClientError> {
        let denied = || McpClientError::Transport("named MCP grant revoked".into());
        if !self.live_grant.load(Ordering::SeqCst) {
            return Err(denied());
        }
        let mut response = self
            .transport
            .send(envelope)
            .map_err(|_| McpClientError::Transport("named MCP endpoint unavailable".into()))?;
        if !self.live_grant.load(Ordering::SeqCst) {
            return Err(denied());
        }
        if let Some(id) = envelope.id.as_deref()
            && (response.get("jsonrpc").and_then(Value::as_str) != Some("2.0")
                || response.get("id").and_then(Value::as_str) != Some(id)
                || response.get("error").is_some()
                || response.get("result").is_none())
        {
            return Err(McpClientError::Transport(
                "invalid named MCP response".into(),
            ));
        }
        // Peer output cannot choose a retained app receipt label.
        if envelope.method == "tools/call"
            && let Some(object) = response.as_object_mut()
        {
            object.remove("result_label");
        }
        Ok(response)
    }
}

impl NamedMcpPeer {
    fn invalidate(&mut self) {
        self.live_grant.store(false, Ordering::SeqCst);
        self.client = None;
        self.permissions = NamedMcpPeerPermissions::default();
        self.snapshot.transport_granted = false;
    }
}

impl Drop for NamedMcpPeer {
    fn drop(&mut self) {
        self.live_grant.store(false, Ordering::SeqCst);
    }
}

fn validate_config(config: &NamedMcpPeerConfig) -> Result<(), NamedMcpPeerError> {
    let metadata = &config.metadata;
    let safe_label = |label: &str| {
        !label.trim().is_empty() && label.len() <= 128 && !label.chars().any(char::is_control)
    };
    let safe_scope = |scope: &String| {
        !scope.is_empty()
            && scope.len() <= 64
            && scope
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b":._-".contains(&b))
    };
    if !safe_label(&metadata.peer_id.0)
        || !metadata
            .peer_id
            .0
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b":._-".contains(&b))
        || !safe_label(&metadata.display_label)
        || metadata.protocol_version != MCP_PEER_PROTOCOL_VERSION
        || metadata.credential_scopes.len() > 32
        || !metadata.credential_scopes.iter().all(safe_scope)
        || (metadata.authentication == McpPeerAuthentication::Bearer)
            != !metadata.credential_scopes.is_empty()
    {
        return Err(NamedMcpPeerError::InvalidConfiguration);
    }
    match (
        &config.transport,
        metadata.transport,
        metadata.role,
        metadata.authentication,
    ) {
        (
            NamedMcpPeerTransport::Http { endpoint },
            McpTransportKind::StreamableHttp,
            McpPeerRole::Client,
            _,
        ) => {
            legion_ai_providers::mcp_endpoint_network_target(endpoint)
                .map_err(|_| NamedMcpPeerError::InvalidConfiguration)?;
        }
        (
            NamedMcpPeerTransport::Stdio { command, args },
            McpTransportKind::Stdio,
            _,
            McpPeerAuthentication::None,
        ) if !command.trim().is_empty()
            && command.len() <= 4096
            && !command.chars().any(char::is_control)
            && args.len() <= 64
            && args
                .iter()
                .all(|arg| arg.len() <= 4096 && !arg.contains('\0')) => {}
        _ => return Err(NamedMcpPeerError::InvalidConfiguration),
    }
    Ok(())
}

impl AppComposition {
    /// Validate and retain named configuration without performing any I/O.
    pub fn configure_named_mcp_peer(
        &mut self,
        config: NamedMcpPeerConfig,
    ) -> Result<McpPeerSnapshot, NamedMcpPeerError> {
        validate_config(&config)?;
        let peer_id = config.metadata.peer_id.0.clone();
        let revision = self
            .named_mcp_peers
            .get(&peer_id)
            .map_or(Some(1), |peer| peer.snapshot.revision.checked_add(1))
            .ok_or(NamedMcpPeerError::InvalidConfiguration)?;
        let snapshot = McpPeerSnapshot {
            metadata: config.metadata.clone(),
            revision,
            health: McpPeerHealth::Configured,
            transport_granted: false,
            credential_state: McpPeerCredentialState::Unchanged,
        };
        self.automate_mcp_tool_runtimes.remove(&peer_id);
        self.automate_workflow.mcp_registries.remove(&peer_id);
        self.named_mcp_peers.insert(
            peer_id,
            NamedMcpPeer {
                config,
                snapshot: snapshot.clone(),
                permissions: NamedMcpPeerPermissions::default(),
                live_grant: Arc::new(AtomicBool::new(false)),
                client: None,
            },
        );
        Ok(snapshot)
    }

    /// Inspect metadata without probing an endpoint or loading a credential.
    pub fn inspect_named_mcp_peer(&self, peer_id: &McpServerId) -> Option<McpPeerSnapshot> {
        self.named_mcp_peers
            .get(&peer_id.0)
            .map(|peer| peer.snapshot.clone())
    }

    /// Bind an explicit endpoint/process permission to the current configuration.
    pub fn grant_named_mcp_peer_transport(
        &mut self,
        peer_id: &McpServerId,
        revision: u64,
        permissions: NamedMcpPeerPermissions,
    ) -> Result<McpPeerSnapshot, NamedMcpPeerError> {
        let peer = self
            .named_mcp_peers
            .get_mut(&peer_id.0)
            .ok_or(NamedMcpPeerError::UnknownPeer)?;
        if peer.snapshot.revision != revision {
            return Err(NamedMcpPeerError::StaleRevision);
        }
        let allowed = match peer.config.transport {
            NamedMcpPeerTransport::Http { .. } => permissions.network,
            NamedMcpPeerTransport::Stdio { .. } => permissions.process,
        };
        if !allowed {
            return Err(NamedMcpPeerError::PermissionRequired);
        }
        peer.invalidate();
        self.automate_mcp_tool_runtimes.remove(&peer_id.0);
        peer.permissions = permissions;
        peer.snapshot.transport_granted = true;
        peer.snapshot.health = McpPeerHealth::Configured;
        Ok(peer.snapshot.clone())
    }

    /// Activate only the selected peer after mode, revision, permission and secret checks.
    pub fn activate_named_mcp_peer(
        &mut self,
        peer_id: &McpServerId,
        revision: u64,
        secrets: &dyn SecretStore,
    ) -> Result<McpPeerSnapshot, NamedMcpPeerError> {
        if self.product_mode == crate::AppProductMode::Manual {
            return Err(NamedMcpPeerError::ManualMode);
        }
        let peer = self
            .named_mcp_peers
            .get(&peer_id.0)
            .ok_or(NamedMcpPeerError::UnknownPeer)?;
        if peer.snapshot.revision != revision {
            return Err(NamedMcpPeerError::StaleRevision);
        }
        if !peer.snapshot.transport_granted {
            return Err(NamedMcpPeerError::PermissionRequired);
        }
        if peer.config.metadata.role == McpPeerRole::Server
            || matches!(peer.config.transport, NamedMcpPeerTransport::Stdio { .. })
        {
            return Err(NamedMcpPeerError::UnsupportedEnvironment);
        }
        self.named_peer_network_policy(peer_id)?;
        let config = self.named_mcp_peers[&peer_id.0].config.clone();
        // Retire any previous session before loading a credential or negotiating.
        self.automate_mcp_tool_runtimes.remove(&peer_id.0);
        let peer = self
            .named_mcp_peers
            .get_mut(&peer_id.0)
            .expect("peer checked");
        peer.live_grant.store(false, Ordering::SeqCst);
        peer.client = None;
        peer.live_grant = Arc::new(AtomicBool::new(true));
        let result = connect_peer(&config, secrets, peer.live_grant.clone(), revision);
        match result {
            Ok((client, transport)) => {
                let registry = client.registry().clone();
                let runtime_client = McpClient::new(registry.clone(), transport)
                    .map_err(|_| NamedMcpPeerError::EndpointUnavailable)?;
                peer.client = Some(client);
                peer.snapshot.health = McpPeerHealth::Ready;
                self.automate_workflow.seed_mcp_registry(registry);
                self.automate_mcp_tool_runtimes.insert(
                    peer_id.0.clone(),
                    Arc::new(crate::AppMcpClientToolRuntime::new(runtime_client)),
                );
                Ok(peer.snapshot.clone())
            }
            Err(error) => {
                peer.invalidate();
                peer.snapshot.health = McpPeerHealth::Unavailable;
                Err(error)
            }
        }
    }

    /// Probe through the existing client; failed health disables runtime reuse.
    pub fn probe_named_mcp_peer(
        &mut self,
        peer_id: &McpServerId,
        revision: u64,
    ) -> Result<McpPeerSnapshot, NamedMcpPeerError> {
        if self.product_mode == crate::AppProductMode::Manual {
            return Err(NamedMcpPeerError::ManualMode);
        }
        let peer = self
            .named_mcp_peers
            .get(&peer_id.0)
            .ok_or(NamedMcpPeerError::UnknownPeer)?;
        if peer.snapshot.revision != revision {
            return Err(NamedMcpPeerError::StaleRevision);
        }
        self.ensure_named_peer_runtime(peer_id)?;
        let peer = self
            .named_mcp_peers
            .get_mut(&peer_id.0)
            .expect("peer checked");
        if peer
            .client
            .as_ref()
            .ok_or(NamedMcpPeerError::EndpointUnavailable)?
            .ping(format!("peer-health:{revision}"))
            .is_err()
        {
            peer.invalidate();
            peer.snapshot.health = McpPeerHealth::Unavailable;
            self.automate_mcp_tool_runtimes.remove(&peer_id.0);
            return Err(NamedMcpPeerError::EndpointUnavailable);
        }
        Ok(peer.snapshot.clone())
    }

    /// Revoke the local transport/runtime before any fallible credential operation.
    /// Remote token invalidation and in-flight termination are separately qualified.
    pub fn revoke_named_mcp_peer_grant(
        &mut self,
        peer_id: &McpServerId,
    ) -> Result<McpPeerSnapshot, NamedMcpPeerError> {
        let peer = self
            .named_mcp_peers
            .get_mut(&peer_id.0)
            .ok_or(NamedMcpPeerError::UnknownPeer)?;
        let revision = if peer.snapshot.health == McpPeerHealth::Revoked {
            peer.snapshot.revision
        } else {
            peer.snapshot
                .revision
                .checked_add(1)
                .ok_or(NamedMcpPeerError::InvalidConfiguration)?
        };
        peer.invalidate();
        peer.snapshot.revision = revision;
        peer.snapshot.health = McpPeerHealth::Revoked;
        self.automate_mcp_tool_runtimes.remove(&peer_id.0);
        self.automate_workflow.mcp_registries.remove(&peer_id.0);
        Ok(peer.snapshot.clone())
    }

    /// Revoke locally, then delete only this named peer's SecretStore reference.
    pub fn revoke_named_mcp_peer_credential(
        &mut self,
        peer_id: &McpServerId,
        secrets: &dyn SecretStore,
    ) -> Result<McpPeerSnapshot, NamedMcpPeerError> {
        self.revoke_named_mcp_peer_grant(peer_id)?;
        let reference = named_mcp_peer_secret_reference(&self.named_mcp_peers[&peer_id.0].config);
        let deleted = secrets.delete(&reference).is_ok();
        let peer = self
            .named_mcp_peers
            .get_mut(&peer_id.0)
            .expect("peer revoked");
        peer.snapshot.credential_state = if deleted {
            McpPeerCredentialState::Deleted
        } else {
            McpPeerCredentialState::DeletionFailed
        };
        if !deleted {
            return Err(NamedMcpPeerError::CredentialDeletionFailed);
        }
        Ok(peer.snapshot.clone())
    }

    fn named_peer_network_policy(&self, peer_id: &McpServerId) -> Result<(), NamedMcpPeerError> {
        let peer = self
            .named_mcp_peers
            .get(&peer_id.0)
            .ok_or(NamedMcpPeerError::UnknownPeer)?;
        // A signed org ceiling can narrow an explicit peer grant, never widen it.
        if let Some(bundle) = &self.org_policy_bundle {
            let NamedMcpPeerTransport::Http { endpoint } = &peer.config.transport else {
                return Err(NamedMcpPeerError::UnsupportedEnvironment);
            };
            let target = legion_ai_providers::mcp_endpoint_network_target(endpoint)
                .map_err(|_| NamedMcpPeerError::InvalidConfiguration)?;
            let mut broker = legion_security::DenyByDefaultBroker::new(
                bundle.bundle().security_policy.clone(),
                legion_protocol::CapabilityNamespace("app.mcp".into()),
            );
            let trust = self
                .active_documents
                .active_workspace_trust
                .clone()
                .unwrap_or(legion_protocol::WorkspaceTrustState::Unknown);
            let decision = broker.decide_with_request_context(
                trust.into(),
                legion_protocol::PrincipalId("mcp:operator".into()),
                legion_protocol::CapabilityId("network.egress".into()),
                None,
                legion_protocol::CapabilityRequestContext {
                    network_target: Some(target),
                    ..Default::default()
                },
            );
            if decision != legion_security::SecurityDecision::Allow {
                return Err(NamedMcpPeerError::PermissionRequired);
            }
        }
        Ok(())
    }

    pub(crate) fn ensure_named_peer_runtime(
        &self,
        peer_id: &McpServerId,
    ) -> Result<(), NamedMcpPeerError> {
        if let Some(peer) = self.named_mcp_peers.get(&peer_id.0) {
            if self.product_mode == crate::AppProductMode::Manual {
                return Err(NamedMcpPeerError::ManualMode);
            }
            if peer.snapshot.health == McpPeerHealth::Revoked {
                return Err(NamedMcpPeerError::Revoked);
            }
            if peer.snapshot.health != McpPeerHealth::Ready
                || !peer.live_grant.load(Ordering::SeqCst)
            {
                return Err(NamedMcpPeerError::PermissionRequired);
            }
            self.named_peer_network_policy(peer_id)?;
        }
        Ok(())
    }
}

fn connect_peer(
    config: &NamedMcpPeerConfig,
    secrets: &dyn SecretStore,
    live_grant: Arc<AtomicBool>,
    revision: u64,
) -> Result<(McpClient<GuardedMcpTransport>, GuardedMcpTransport), NamedMcpPeerError> {
    let secret = if config.metadata.authentication == McpPeerAuthentication::Bearer {
        let value = secrets
            .load(&named_mcp_peer_secret_reference(config))
            .map_err(|_| NamedMcpPeerError::CredentialUnavailable)?
            .filter(|value| !value.trim().is_empty())
            .ok_or(NamedMcpPeerError::CredentialUnavailable)?;
        Some(value)
    } else {
        None
    };
    let NamedMcpPeerTransport::Http { endpoint } = &config.transport else {
        return Err(NamedMcpPeerError::UnsupportedEnvironment);
    };
    let transport = GuardedMcpTransport {
        transport: StreamableHttpMcpTransport::for_named_peer(
            StreamableHttpMcpTransportConfig {
                endpoint: endpoint.clone(),
            },
            secret.as_deref(),
            &config.metadata.protocol_version,
        )
        .map_err(|_| NamedMcpPeerError::EndpointUnavailable)?,
        live_grant,
    };
    let metadata = &config.metadata;
    let registry = McpRegistrySnapshot {
        registry_id: format!("mcp-peer:{}:{revision}", metadata.peer_id.0),
        server: McpServerDescriptor {
            server_id: metadata.peer_id.clone(),
            transport_kind: metadata.transport,
            display_label: metadata.display_label.clone(),
            endpoint_label: "named MCP endpoint".into(),
            tools_list_changed: false,
            resources_list_changed: false,
            prompts_list_changed: false,
            redaction_hints: vec![RedactionHint::MetadataOnly],
            schema_version: 1,
        },
        tools: vec![],
        resources: vec![],
        prompts: vec![],
        last_notification_kind: None,
        list_version: 1,
        generated_at: TimestampMillis::now(),
        redaction_hints: vec![RedactionHint::MetadataOnly],
        schema_version: 1,
    };
    let unavailable = |_| NamedMcpPeerError::EndpointUnavailable;
    let mut client = McpClient::new(registry, transport.clone()).map_err(unavailable)?;
    let response = client
        .initialize(
            format!("peer-initialize:{revision}"),
            &metadata.protocol_version,
        )
        .map_err(unavailable)?;
    if response["result"]["protocolVersion"].as_str() != Some(metadata.protocol_version.as_str()) {
        return Err(NamedMcpPeerError::ProtocolMismatch);
    }
    let capabilities = response["result"]["capabilities"]
        .as_object()
        .ok_or(NamedMcpPeerError::EndpointUnavailable)?;
    client.notify_initialized().map_err(unavailable)?;
    if capabilities.contains_key("tools") {
        client
            .reload_after_list_changed(
                McpListChangedKind::Tools,
                format!("peer-tools:{revision}"),
                TimestampMillis::now(),
            )
            .map_err(unavailable)?;
        let mut registry = client.registry().clone();
        registry.server.tools_list_changed =
            response["result"]["capabilities"]["tools"]["listChanged"]
                .as_bool()
                .unwrap_or(false);
        for tool in &mut registry.tools {
            tool.description_label = "MCP tool metadata".into();
        }
        client.replace_registry(registry).map_err(unavailable)?;
    }
    client
        .ping(format!("peer-health:{revision}"))
        .map_err(unavailable)?;
    Ok((client, transport))
}
