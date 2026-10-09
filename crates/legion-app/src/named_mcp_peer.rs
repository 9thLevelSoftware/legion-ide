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
use serde::{Deserialize, Serialize};
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

/// Operational transport configuration, excluded from workflow projections.
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

/// Configuration input; persistence uses a separate restricted HTTP codec.
#[derive(Clone)]
pub struct NamedMcpPeerConfig {
    /// Safe, reviewed peer metadata.
    pub metadata: McpPeerMetadata,
    /// Endpoint/launch details; only validated HTTP endpoints may be persisted.
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

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct PersistedHttpPeer {
    peer_id: McpServerId,
    display_label: String,
    role: McpPeerRole,
    protocol_version: String,
    transport: McpTransportKind,
    authentication: McpPeerAuthentication,
    credential_scopes: Vec<String>,
    privacy: McpPeerPrivacy,
    endpoint: String,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct PersistedPeers {
    version: u16,
    peers: Vec<PersistedHttpPeer>,
}

fn persistence_error() -> crate::AppCompositionError {
    crate::AppCompositionError::AiRuntime("invalid named MCP configuration metadata".into())
}

fn parse_persisted_peers(
    json: &str,
) -> Result<Vec<NamedMcpPeerConfig>, crate::AppCompositionError> {
    if json.len() > 65536 {
        return Err(persistence_error());
    }
    let stored: PersistedPeers = serde_json::from_str(json).map_err(|_| persistence_error())?;
    if stored.version != 1 || stored.peers.len() > 32 {
        return Err(persistence_error());
    }
    let mut ids = std::collections::HashSet::new();
    stored
        .peers
        .into_iter()
        .map(|peer| {
            if !ids.insert(peer.peer_id.0.clone()) || peer.endpoint.len() > 4096 {
                return Err(persistence_error());
            }
            for value in [&peer.peer_id.0, &peer.display_label, &peer.endpoint]
                .into_iter()
                .chain(peer.credential_scopes.iter())
            {
                if !legion_security::secrets::scan_text_for_secrets(value)
                    .findings
                    .is_empty()
                {
                    return Err(persistence_error());
                }
            }
            let config = NamedMcpPeerConfig {
                metadata: McpPeerMetadata {
                    peer_id: peer.peer_id,
                    display_label: peer.display_label,
                    role: peer.role,
                    protocol_version: peer.protocol_version,
                    transport: peer.transport,
                    authentication: peer.authentication,
                    credential_scopes: peer.credential_scopes,
                    privacy: peer.privacy,
                },
                transport: NamedMcpPeerTransport::Http {
                    endpoint: peer.endpoint,
                },
            };
            validate_config(&config).map_err(|_| persistence_error())?;
            Ok(config)
        })
        .collect()
}

impl AppComposition {
    /// Enumerate configuration for explicit settings inspection, without I/O.
    /// Operational addresses remain excluded from workflow/telemetry projections.
    pub fn named_mcp_peer_configurations(&self) -> Vec<NamedMcpPeerConfig> {
        let mut configs: Vec<_> = self
            .named_mcp_peers
            .values()
            .map(|p| p.config.clone())
            .collect();
        configs.sort_by(|a, b| a.metadata.peer_id.0.cmp(&b.metadata.peer_id.0));
        configs
    }

    /// Preflight the complete durable HTTP settings before replacing a peer.
    /// None means a new identity; edits require the displayed revision.
    pub fn configure_named_mcp_http_peer_settings(
        &mut self,
        config: NamedMcpPeerConfig,
        expected_revision: Option<u64>,
    ) -> Result<McpPeerSnapshot, NamedMcpPeerError> {
        if self
            .inspect_named_mcp_peer(&config.metadata.peer_id)
            .map(|p| p.revision)
            != expected_revision
        {
            return Err(NamedMcpPeerError::StaleRevision);
        }
        let mut configs = self.named_mcp_peer_configurations();
        configs.retain(|p| p.metadata.peer_id != config.metadata.peer_id);
        configs.push(config.clone());
        encode_http_configurations(&configs)
            .map_err(|_| NamedMcpPeerError::InvalidConfiguration)?;
        self.configure_named_mcp_peer(config)
    }

    /// Encode HTTP configuration only, without accessing a secret or transport.
    pub fn named_mcp_peer_configuration_json(&self) -> Result<String, crate::AppCompositionError> {
        encode_http_configurations(&self.named_mcp_peer_configurations())
    }
}

fn encode_http_configurations(
    configs: &[NamedMcpPeerConfig],
) -> Result<String, crate::AppCompositionError> {
    let mut peers = Vec::new();
    for config in configs {
        let NamedMcpPeerTransport::Http { endpoint } = &config.transport else {
            return Err(persistence_error());
        };
        let metadata = &config.metadata;
        peers.push(PersistedHttpPeer {
            peer_id: metadata.peer_id.clone(),
            display_label: metadata.display_label.clone(),
            role: metadata.role,
            protocol_version: metadata.protocol_version.clone(),
            transport: metadata.transport,
            authentication: metadata.authentication,
            credential_scopes: metadata.credential_scopes.clone(),
            privacy: metadata.privacy,
            endpoint: endpoint.clone(),
        });
    }
    peers.sort_by(|a, b| a.peer_id.0.cmp(&b.peer_id.0));
    let json = serde_json::to_string(&PersistedPeers { version: 1, peers })
        .map_err(|_| persistence_error())?;
    AppComposition::validate_named_mcp_peer_configuration_json(&json)?;
    Ok(json)
}

impl AppComposition {
    /// Pure validation for the existing desktop session store.
    pub fn validate_named_mcp_peer_configuration_json(
        json: &str,
    ) -> Result<(), crate::AppCompositionError> {
        parse_persisted_peers(json).map(|_| ())
    }

    pub(crate) fn prepare_named_mcp_peer_restore(
        &self,
        json: Option<&str>,
    ) -> Result<std::collections::HashMap<String, NamedMcpPeer>, crate::AppCompositionError> {
        let configs = parse_persisted_peers(json.unwrap_or(r#"{"version":1,"peers":[]}"#))?;
        let mut peers = std::collections::HashMap::new();
        for config in configs {
            let id = config.metadata.peer_id.0.clone();
            let revision = self
                .named_mcp_peers
                .get(&id)
                .map_or(Some(1), |peer| peer.snapshot.revision.checked_add(1))
                .ok_or_else(persistence_error)?;
            let snapshot = McpPeerSnapshot {
                metadata: config.metadata.clone(),
                revision,
                health: McpPeerHealth::Configured,
                transport_granted: false,
                credential_state: McpPeerCredentialState::Unchanged,
            };
            peers.insert(
                id,
                NamedMcpPeer {
                    config,
                    snapshot,
                    permissions: NamedMcpPeerPermissions::default(),
                    live_grant: Arc::new(AtomicBool::new(false)),
                    client: None,
                    transport: None,
                    operation: None,
                },
            );
        }
        Ok(peers)
    }

    pub(crate) fn commit_named_mcp_peer_restore(
        &mut self,
        peers: std::collections::HashMap<String, NamedMcpPeer>,
    ) {
        for id in self.named_mcp_peers.keys().chain(peers.keys()) {
            self.automate_mcp_tool_runtimes.remove(id);
            self.automate_workflow.mcp_registries.remove(id);
        }
        self.named_mcp_peers = peers;
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
    /// Secure replacement failed; raw keyring text is never exposed.
    #[error("named MCP peer credential replacement failed; local grant revoked")]
    CredentialReplacementFailed,
    /// One operation per peer and at most two active named-peer I/O workers.
    #[error("named MCP operation is busy; wait for the current request to drain")]
    OperationBusy,
}

pub(crate) struct NamedMcpPeer {
    config: NamedMcpPeerConfig,
    snapshot: McpPeerSnapshot,
    permissions: NamedMcpPeerPermissions,
    live_grant: Arc<AtomicBool>,
    client: Option<McpClient<GuardedMcpTransport>>,
    transport: Option<GuardedMcpTransport>,
    operation: Option<NamedMcpOperation>,
}

type NamedMcpCompletion =
    Result<(McpClient<GuardedMcpTransport>, GuardedMcpTransport), NamedMcpPeerError>;
type NamedMcpHandle = std::thread::JoinHandle<NamedMcpCompletion>;

// Follow the existing app worker convention: never join live I/O on UI/drop,
// and never detach a handle on configuration replacement or app shutdown.
struct NamedMcpSupervisor {
    sender: std::sync::mpsc::Sender<NamedMcpHandle>,
    _handle: std::thread::JoinHandle<()>,
}
static NAMED_MCP_SUPERVISOR: std::sync::OnceLock<NamedMcpSupervisor> = std::sync::OnceLock::new();
static NAMED_MCP_SUPERVISOR_INIT: std::sync::Mutex<()> = std::sync::Mutex::new(());
static NAMED_MCP_RETAINED_HANDLES: std::sync::Mutex<Vec<NamedMcpHandle>> =
    std::sync::Mutex::new(Vec::new());
static NAMED_MCP_ACTIVE_WORKERS: std::sync::atomic::AtomicUsize =
    std::sync::atomic::AtomicUsize::new(0);

// Share the existing app egress-drain gate. The worker owns this lease until
// transport I/O returns, even when its peer/operation has been retired.
struct NamedMcpWorkerPermit(Arc<std::sync::atomic::AtomicUsize>);
impl NamedMcpWorkerPermit {
    fn acquire(drain: Arc<std::sync::atomic::AtomicUsize>) -> Result<Self, NamedMcpPeerError> {
        NAMED_MCP_ACTIVE_WORKERS
            .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |count| {
                (count < 2).then_some(count + 1)
            })
            .map_err(|_| NamedMcpPeerError::OperationBusy)?;
        drain.fetch_add(1, Ordering::SeqCst);
        Ok(Self(drain))
    }
}
impl Drop for NamedMcpWorkerPermit {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::SeqCst);
        NAMED_MCP_ACTIVE_WORKERS.fetch_sub(1, Ordering::SeqCst);
    }
}

fn named_mcp_supervisor_sender()
-> Result<std::sync::mpsc::Sender<NamedMcpHandle>, NamedMcpPeerError> {
    let _guard = NAMED_MCP_SUPERVISOR_INIT
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    if let Some(supervisor) = NAMED_MCP_SUPERVISOR.get() {
        return Ok(supervisor.sender.clone());
    }
    let (sender, receiver) = std::sync::mpsc::channel::<NamedMcpHandle>();
    let handle = std::thread::Builder::new()
        .name("legion-mcp-handle-supervisor".into())
        .spawn(move || {
            let mut pending: Vec<NamedMcpHandle> = Vec::new();
            loop {
                match receiver.recv_timeout(std::time::Duration::from_millis(10)) {
                    Ok(handle) => pending.push(handle),
                    Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
                    Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                        NAMED_MCP_RETAINED_HANDLES
                            .lock()
                            .unwrap_or_else(std::sync::PoisonError::into_inner)
                            .extend(pending);
                        return;
                    }
                }
                while let Ok(handle) = receiver.try_recv() {
                    pending.push(handle);
                }
                let mut index = 0;
                while index < pending.len() {
                    if pending[index].is_finished() {
                        let _ = pending.swap_remove(index).join();
                    } else {
                        index += 1;
                    }
                }
            }
        })
        .map_err(|_| NamedMcpPeerError::EndpointUnavailable)?;
    let supervisor = NamedMcpSupervisor {
        sender: sender.clone(),
        _handle: handle,
    };
    // Initialization is serialized above; the process-lifetime owner holds sender.
    let _ = NAMED_MCP_SUPERVISOR.set(supervisor);
    Ok(sender)
}

struct NamedMcpOperation {
    revision: u64,
    live_grant: Arc<AtomicBool>,
    handle: Option<NamedMcpHandle>,
    supervisor: std::sync::mpsc::Sender<NamedMcpHandle>,
}
impl NamedMcpOperation {
    fn is_finished(&self) -> bool {
        self.handle
            .as_ref()
            .is_some_and(std::thread::JoinHandle::is_finished)
    }
    fn finish(mut self) -> NamedMcpCompletion {
        self.handle
            .take()
            .expect("owned MCP handle")
            .join()
            .unwrap_or(Err(NamedMcpPeerError::EndpointUnavailable))
    }
}
impl Drop for NamedMcpOperation {
    fn drop(&mut self) {
        if let Some(handle) = self.handle.take() {
            self.live_grant.store(false, Ordering::SeqCst);
            if let Err(std::sync::mpsc::SendError(handle)) = self.supervisor.send(handle) {
                NAMED_MCP_RETAINED_HANDLES
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .push(handle);
            }
        }
    }
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
        self.transport = None;
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
                transport: None,
                operation: None,
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
        if peer.operation.is_some() {
            return Err(NamedMcpPeerError::OperationBusy);
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
        if peer.operation.is_some() {
            return Err(NamedMcpPeerError::OperationBusy);
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
                let runtime_client = McpClient::new(registry.clone(), transport.clone())
                    .map_err(|_| NamedMcpPeerError::EndpointUnavailable)?;
                peer.client = Some(client);
                peer.transport = Some(transport.clone());
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

    fn check_named_mcp_operation(
        &self,
        peer_id: &McpServerId,
        revision: u64,
    ) -> Result<(), NamedMcpPeerError> {
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
        if peer.operation.is_some() {
            return Err(NamedMcpPeerError::OperationBusy);
        }
        if !peer.snapshot.transport_granted {
            return Err(NamedMcpPeerError::PermissionRequired);
        }
        if peer.config.metadata.role != McpPeerRole::Client
            || !matches!(peer.config.transport, NamedMcpPeerTransport::Http { .. })
        {
            return Err(NamedMcpPeerError::UnsupportedEnvironment);
        }
        self.named_peer_network_policy(peer_id)
    }

    /// Start bounded app-owned connection I/O; does not wait for network/keyring.
    pub fn start_named_mcp_peer_connection(
        &mut self,
        peer_id: &McpServerId,
        revision: u64,
    ) -> Result<(), NamedMcpPeerError> {
        self.check_named_mcp_operation(peer_id, revision)?;
        let supervisor = named_mcp_supervisor_sender()?;
        let permit = NamedMcpWorkerPermit::acquire(self.named_provider_snapshots.clone())?;
        let secrets = self.provider_secret_store.clone();
        let peer = self
            .named_mcp_peers
            .get_mut(&peer_id.0)
            .expect("checked peer");
        let config = peer.config.clone();
        peer.live_grant.store(false, Ordering::SeqCst);
        peer.client = None;
        peer.transport = None;
        peer.live_grant = Arc::new(AtomicBool::new(true));
        let gate = peer.live_grant.clone();
        let live_grant = gate.clone();
        let handle = std::thread::Builder::new()
            .name("legion-mcp-connect".into())
            .spawn(move || {
                let _permit = permit;
                connect_peer(&config, secrets.as_ref(), gate, revision)
            })
            .map_err(|_| {
                peer.invalidate();
                NamedMcpPeerError::EndpointUnavailable
            })?;
        peer.snapshot.health = McpPeerHealth::Configured;
        peer.operation = Some(NamedMcpOperation {
            revision,
            live_grant,
            handle: Some(handle),
            supervisor,
        });
        self.automate_mcp_tool_runtimes.remove(&peer_id.0);
        self.automate_workflow.mcp_registries.remove(&peer_id.0);
        Ok(())
    }

    /// Start a health ping off-thread, retaining the exact app-owned grant.
    pub fn start_named_mcp_peer_health_probe(
        &mut self,
        peer_id: &McpServerId,
        revision: u64,
    ) -> Result<(), NamedMcpPeerError> {
        self.check_named_mcp_operation(peer_id, revision)?;
        self.ensure_named_peer_runtime(peer_id)?;
        let supervisor = named_mcp_supervisor_sender()?;
        let permit = NamedMcpWorkerPermit::acquire(self.named_provider_snapshots.clone())?;
        let peer = self
            .named_mcp_peers
            .get_mut(&peer_id.0)
            .expect("checked peer");
        let transport = peer
            .transport
            .clone()
            .ok_or(NamedMcpPeerError::EndpointUnavailable)?;
        let client = peer
            .client
            .take()
            .ok_or(NamedMcpPeerError::EndpointUnavailable)?;
        let live_grant = peer.live_grant.clone();
        let handle = std::thread::Builder::new()
            .name("legion-mcp-health".into())
            .spawn(move || {
                let _permit = permit;
                client
                    .ping(format!("peer-health:{revision}"))
                    .map_err(|_| NamedMcpPeerError::EndpointUnavailable)?;
                Ok((client, transport))
            })
            .map_err(|_| {
                peer.invalidate();
                NamedMcpPeerError::EndpointUnavailable
            })?;
        peer.operation = Some(NamedMcpOperation {
            revision,
            live_grant,
            handle: Some(handle),
            supervisor,
        });
        self.automate_mcp_tool_runtimes.remove(&peer_id.0);
        self.automate_workflow.mcp_registries.remove(&peer_id.0);
        Ok(())
    }

    /// Observe pending app-owned work without touching a transport or credential.
    pub fn named_mcp_peer_operation_pending(&self, peer_id: &McpServerId) -> bool {
        self.named_mcp_peers
            .get(&peer_id.0)
            .is_some_and(|p| p.operation.is_some())
    }

    /// Deny follow-up sends/results on a requested downgrade. The central mode
    /// setter still waits for the worker-owned drain leases, not these flags.
    pub(crate) fn request_named_mcp_operation_drain(&mut self) {
        let ids: Vec<_> = self
            .named_mcp_peers
            .values()
            .filter(|p| p.operation.is_some() && p.live_grant.load(Ordering::SeqCst))
            .map(|p| p.config.metadata.peer_id.clone())
            .collect();
        for id in ids {
            let _ = self.revoke_named_mcp_peer_grant(&id);
        }
    }

    /// Join only completed workers and publish only current, still-authorized results.
    pub fn poll_named_mcp_peer_operations(
        &mut self,
    ) -> Vec<(McpServerId, Result<McpPeerSnapshot, NamedMcpPeerError>)> {
        self.install_mode_policy_ceiling();
        let ids: Vec<_> = self
            .named_mcp_peers
            .values()
            .filter(|p| {
                p.operation
                    .as_ref()
                    .is_some_and(NamedMcpOperation::is_finished)
            })
            .map(|p| p.config.metadata.peer_id.clone())
            .collect();
        let mut outcomes = Vec::new();
        for id in ids {
            let peer = self
                .named_mcp_peers
                .get_mut(&id.0)
                .expect("enumerated peer");
            let operation = peer.operation.take().expect("finished operation");
            let current = operation.revision == peer.snapshot.revision
                && peer.snapshot.transport_granted
                && Arc::ptr_eq(&operation.live_grant, &peer.live_grant)
                && peer.live_grant.load(Ordering::SeqCst);
            let result = operation.finish();
            if !current {
                outcomes.push((id, Err(NamedMcpPeerError::StaleRevision)));
                continue;
            }
            let result = self.named_peer_network_policy(&id).and(result);
            let peer = self
                .named_mcp_peers
                .get_mut(&id.0)
                .expect("enumerated peer");
            match result {
                Ok((client, transport)) => {
                    let registry = client.registry().clone();
                    let runtime = McpClient::new(registry.clone(), transport.clone())
                        .map(crate::AppMcpClientToolRuntime::new);
                    match runtime {
                        Ok(runtime) => {
                            peer.client = Some(client);
                            peer.transport = Some(transport);
                            peer.snapshot.health = McpPeerHealth::Ready;
                            self.automate_workflow.seed_mcp_registry(registry);
                            self.automate_mcp_tool_runtimes
                                .insert(id.0.clone(), Arc::new(runtime));
                            outcomes.push((id, Ok(peer.snapshot.clone())));
                        }
                        Err(_) => {
                            peer.invalidate();
                            peer.snapshot.health = McpPeerHealth::Unavailable;
                            outcomes.push((id, Err(NamedMcpPeerError::EndpointUnavailable)));
                        }
                    }
                }
                Err(error) => {
                    peer.invalidate();
                    peer.snapshot.health = McpPeerHealth::Unavailable;
                    outcomes.push((id, Err(error)));
                }
            }
        }
        self.install_mode_policy_ceiling();
        outcomes
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
        if peer.operation.is_some() {
            return Err(NamedMcpPeerError::OperationBusy);
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

    /// Replace only the displayed bearer route using the existing secure port.
    /// Retire any current runtime/grant before the fallible store operation.
    pub fn replace_named_mcp_peer_credential(
        &mut self,
        peer_id: &McpServerId,
        revision: u64,
        credential: &str,
    ) -> Result<(), NamedMcpPeerError> {
        let peer = self
            .named_mcp_peers
            .get(&peer_id.0)
            .ok_or(NamedMcpPeerError::UnknownPeer)?;
        if peer.snapshot.revision != revision {
            return Err(NamedMcpPeerError::StaleRevision);
        }
        if peer.config.metadata.authentication != McpPeerAuthentication::Bearer
            || credential.trim().is_empty()
            || credential.len() > 8192
            || credential.chars().any(char::is_control)
        {
            return Err(NamedMcpPeerError::InvalidConfiguration);
        }
        let reference = named_mcp_peer_secret_reference(&peer.config);
        self.revoke_named_mcp_peer_grant(peer_id)?;
        self.provider_secret_store
            .store(&reference, credential)
            .map_err(|_| NamedMcpPeerError::CredentialReplacementFailed)
    }

    /// Revoke the displayed credential binding through the existing secure port.
    pub fn revoke_named_mcp_peer_stored_credential(
        &mut self,
        peer_id: &McpServerId,
        revision: u64,
    ) -> Result<McpPeerSnapshot, NamedMcpPeerError> {
        let peer = self
            .named_mcp_peers
            .get(&peer_id.0)
            .ok_or(NamedMcpPeerError::UnknownPeer)?;
        if peer.snapshot.revision != revision {
            return Err(NamedMcpPeerError::StaleRevision);
        }
        let secrets = self.provider_secret_store.clone();
        self.revoke_named_mcp_peer_credential(peer_id, secrets.as_ref())
    }

    fn named_peer_network_policy(&self, peer_id: &McpServerId) -> Result<(), NamedMcpPeerError> {
        // A retained mode while old I/O drains is not admission authority.
        if self.org_policy_mode_ceiling_denies(self.product_mode) {
            return Err(NamedMcpPeerError::PermissionRequired);
        }
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
