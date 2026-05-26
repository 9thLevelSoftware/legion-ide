//! Deterministic and production-gated Phase 8 remote transport carriers.

#![warn(missing_docs)]

use std::collections::{HashSet, VecDeque};
use std::future::Future;
use std::net::IpAddr;
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::Arc;
use std::time::{Duration, Instant};

use devil_protocol::{
    CausalityId, EventSequence, RedactionHint, RemoteAgentPackageDescriptor,
    RemoteNetworkHealthState, RemoteOfflineResumeManifest, RemoteOperationId,
    RemoteOperationLogCheckpoint, RemoteOperationLogCheckpointId, RemoteTransportAuditSummary,
    RemoteTransportCarrierDiagnostic, RemoteTransportConnectionAttempt,
    RemoteTransportFlowControlWindow, RemoteTransportFrameMetadata, RemoteTransportHandshake,
    RemoteTransportHealthSummary, RemoteTransportLifecycleState, RemoteTransportMutualTlsMode,
    RemoteTransportPeerIdentity, RemoteTransportReplayWindow, RemoteTransportResumeToken,
    RemoteWorkspaceSessionId, TimestampMillis, validate_remote_agent_package_descriptor,
    validate_remote_transport_audit_summary, validate_remote_transport_carrier_diagnostic,
    validate_remote_transport_connection_attempt, validate_remote_transport_flow_control_window,
    validate_remote_transport_handshake, validate_remote_transport_replay_window,
};
use rustls::{ClientConfig, RootCertStore};
use rustls_pki_types::{CertificateDer, PrivateKeyDer, ServerName, pem::PemObject};
use thiserror::Error;
use tokio::net::TcpStream;
use tokio_rustls::TlsConnector;

/// Remote transport fixture error.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum RemoteTransportFixtureError {
    /// Fixture runtime is disabled.
    #[error("remote transport fixture is disabled")]
    Disabled,
    /// Handshake metadata was rejected.
    #[error("invalid remote transport handshake: {reason}")]
    InvalidHandshake {
        /// Rejection reason.
        reason: String,
    },
    /// Frame metadata was rejected.
    #[error("invalid remote transport frame: {reason}")]
    InvalidFrame {
        /// Rejection reason.
        reason: String,
    },
}

/// Production transport-core error.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum RemoteTransportCoreError {
    /// Production transport core is disabled.
    #[error("remote transport core is disabled")]
    Disabled,
    /// Operation was invalid for the current lifecycle state.
    #[error("invalid remote transport state: {reason}")]
    InvalidState {
        /// Rejection reason.
        reason: String,
    },
    /// Handshake metadata was invalid.
    #[error("invalid remote transport handshake: {reason}")]
    InvalidHandshake {
        /// Rejection reason.
        reason: String,
    },
    /// Agent package metadata was invalid.
    #[error("invalid remote agent package: {reason}")]
    InvalidAgentPackage {
        /// Rejection reason.
        reason: String,
    },
    /// Frame metadata was invalid.
    #[error("invalid remote transport frame: {reason}")]
    InvalidFrame {
        /// Rejection reason.
        reason: String,
    },
    /// Frame was rejected by flow control.
    #[error("remote transport is backpressured")]
    Backpressured,
    /// Frame was rejected by replay/order checks.
    #[error("remote transport replay rejected: {reason}")]
    ReplayRejected {
        /// Rejection reason.
        reason: String,
    },
    /// Resume metadata was invalid.
    #[error("remote transport resume rejected: {reason}")]
    ResumeRejected {
        /// Rejection reason.
        reason: String,
    },
}

/// Production remote transport carrier error.
#[derive(Debug, Error)]
pub enum RemoteTransportCarrierError {
    /// Carrier is disabled.
    #[error("remote transport carrier is disabled")]
    Disabled,
    /// Protocol or policy metadata was invalid.
    #[error("invalid remote transport carrier policy: {reason}")]
    InvalidPolicy {
        /// Rejection reason.
        reason: String,
    },
    /// TLS credential material was missing or invalid.
    #[error("remote transport TLS credential error: {reason}")]
    Credential {
        /// Failure reason.
        reason: String,
    },
    /// Network connection failed.
    #[error("remote transport network error: {reason}")]
    Network {
        /// Failure reason.
        reason: String,
    },
    /// TLS handshake failed.
    #[error("remote transport TLS error: {reason}")]
    Tls {
        /// Failure reason.
        reason: String,
    },
    /// Connection attempt was canceled before activation.
    #[error("remote transport connection canceled: {reason}")]
    Canceled {
        /// Failure reason.
        reason: String,
    },
}

/// Production remote transport carrier.
pub trait RemoteTransportCarrier {
    /// Connect to a policy-validated endpoint and return metadata-only diagnostics.
    fn connect<'a>(
        &'a self,
        attempt: RemoteTransportConnectionAttempt,
    ) -> Pin<
        Box<
            dyn Future<
                    Output = Result<RemoteTransportCarrierDiagnostic, RemoteTransportCarrierError>,
                > + Send
                + 'a,
        >,
    >;
}

/// Rustls-backed outbound TLS/mTLS carrier configuration.
#[derive(Debug, Clone, Default)]
pub struct RustlsMtlsCarrierConfig {
    /// Whether outbound carrier use is enabled.
    pub enabled: bool,
    /// PEM files containing trusted server roots. Empty uses the rustls empty root store.
    pub root_ca_pem_paths: Vec<PathBuf>,
    /// PEM file containing the client certificate chain for required mTLS.
    pub client_cert_chain_pem_path: Option<PathBuf>,
    /// PEM file containing the client private key for required mTLS.
    pub client_private_key_pem_path: Option<PathBuf>,
}

impl RustlsMtlsCarrierConfig {
    /// Return an enabled carrier configuration.
    pub fn enabled() -> Self {
        Self {
            enabled: true,
            ..Self::default()
        }
    }
}

/// Rustls-backed production outbound TLS/mTLS carrier.
#[derive(Debug, Clone)]
pub struct RustlsMtlsCarrier {
    config: RustlsMtlsCarrierConfig,
}

impl RustlsMtlsCarrier {
    /// Construct a carrier from explicit configuration.
    pub fn new(config: RustlsMtlsCarrierConfig) -> Self {
        Self { config }
    }

    /// Build a rustls client configuration after policy validation.
    pub fn build_client_config(
        &self,
        attempt: &RemoteTransportConnectionAttempt,
    ) -> Result<ClientConfig, RemoteTransportCarrierError> {
        self.ensure_enabled()?;
        validate_remote_transport_connection_attempt(attempt).map_err(|err| {
            RemoteTransportCarrierError::InvalidPolicy {
                reason: err.message,
            }
        })?;

        let mut roots = RootCertStore::empty();
        for path in &self.config.root_ca_pem_paths {
            for cert in load_certs(path)? {
                roots
                    .add(cert)
                    .map_err(|err| RemoteTransportCarrierError::Credential {
                        reason: format!("root certificate rejected: {err}"),
                    })?;
            }
        }

        let wants_mtls = attempt.tls_policy.mtls_mode == RemoteTransportMutualTlsMode::Required;
        let mut client_config = if wants_mtls {
            let cert_path = self
                .config
                .client_cert_chain_pem_path
                .as_ref()
                .ok_or_else(|| RemoteTransportCarrierError::Credential {
                    reason: "required mTLS has no client certificate chain path".to_string(),
                })?;
            let key_path = self
                .config
                .client_private_key_pem_path
                .as_ref()
                .ok_or_else(|| RemoteTransportCarrierError::Credential {
                    reason: "required mTLS has no client private key path".to_string(),
                })?;
            let certs = load_certs(cert_path)?;
            let key = load_private_key(key_path)?;
            ClientConfig::builder()
                .with_root_certificates(roots)
                .with_client_auth_cert(certs, key)
                .map_err(|err| RemoteTransportCarrierError::Credential {
                    reason: format!("client certificate rejected: {err}"),
                })?
        } else {
            ClientConfig::builder()
                .with_root_certificates(roots)
                .with_no_client_auth()
        };
        client_config.alpn_protocols = attempt
            .tls_policy
            .alpn_protocols
            .iter()
            .map(|protocol| protocol.as_bytes().to_vec())
            .collect();
        Ok(client_config)
    }

    async fn connect_inner(
        &self,
        attempt: RemoteTransportConnectionAttempt,
    ) -> Result<RemoteTransportCarrierDiagnostic, RemoteTransportCarrierError> {
        self.ensure_enabled()?;
        validate_remote_transport_connection_attempt(&attempt).map_err(|err| {
            RemoteTransportCarrierError::InvalidPolicy {
                reason: err.message,
            }
        })?;
        if attempt.cancellation_requested {
            return Err(RemoteTransportCarrierError::Canceled {
                reason: "connection attempt canceled before activation".to_string(),
            });
        }
        let client_config = self.build_client_config(&attempt)?;
        let endpoint = &attempt.endpoint_policy.endpoint;
        let port = endpoint.port.unwrap_or(443);
        let deadline = Instant::now() + Duration::from_millis(attempt.timeout_ms);
        let stream = tokio::time::timeout(
            remaining_attempt_budget(deadline).ok_or_else(|| {
                RemoteTransportCarrierError::Network {
                    reason: "tcp connect timed out".to_string(),
                }
            })?,
            TcpStream::connect((endpoint.host.as_str(), port)),
        )
        .await
        .map_err(|_| RemoteTransportCarrierError::Network {
            reason: "tcp connect timed out".to_string(),
        })?
        .map_err(|err| RemoteTransportCarrierError::Network {
            reason: err.to_string(),
        })?;
        let server_identity = tls_server_identity_name(&attempt.tls_policy.server_identity)?;
        let server_name = ServerName::try_from(server_identity).map_err(|err| {
            RemoteTransportCarrierError::InvalidPolicy {
                reason: format!("invalid TLS policy server identity: {err}"),
            }
        })?;
        let connector = TlsConnector::from(Arc::new(client_config));
        let tls_stream = tokio::time::timeout(
            remaining_attempt_budget(deadline).ok_or_else(|| RemoteTransportCarrierError::Tls {
                reason: "tls handshake timed out".to_string(),
            })?,
            connector.connect(server_name, stream),
        )
        .await
        .map_err(|_| RemoteTransportCarrierError::Tls {
            reason: "tls handshake timed out".to_string(),
        })?
        .map_err(|err| RemoteTransportCarrierError::Tls {
            reason: err.to_string(),
        })?;
        let negotiated_alpn = tls_stream.get_ref().1.alpn_protocol().ok_or_else(|| {
            RemoteTransportCarrierError::Tls {
                reason: "tls handshake did not negotiate ALPN".to_string(),
            }
        })?;
        if negotiated_alpn != attempt.selected_alpn.as_bytes() {
            return Err(RemoteTransportCarrierError::Tls {
                reason: format!(
                    "negotiated ALPN `{}` did not match selected policy `{}`",
                    String::from_utf8_lossy(negotiated_alpn),
                    attempt.selected_alpn
                ),
            });
        }
        let diagnostic = RemoteTransportCarrierDiagnostic {
            session_id: None,
            state: RemoteTransportLifecycleState::Active,
            event_sequence: attempt.event_sequence,
            correlation_id: attempt.correlation_id,
            causality_id: attempt.causality_id,
            metadata_summary: format!(
                "tls_handshake=ok mtls={:?} alpn={} schema={}",
                attempt.tls_policy.mtls_mode,
                attempt.selected_alpn,
                attempt.selected_schema_version
            ),
            redaction_hints: vec![RedactionHint::MetadataOnly],
            schema_version: 1,
        };
        validate_remote_transport_carrier_diagnostic(&diagnostic).map_err(|err| {
            RemoteTransportCarrierError::InvalidPolicy {
                reason: err.message,
            }
        })?;
        Ok(diagnostic)
    }

    fn ensure_enabled(&self) -> Result<(), RemoteTransportCarrierError> {
        if self.config.enabled {
            Ok(())
        } else {
            Err(RemoteTransportCarrierError::Disabled)
        }
    }
}

impl RemoteTransportCarrier for RustlsMtlsCarrier {
    fn connect<'a>(
        &'a self,
        attempt: RemoteTransportConnectionAttempt,
    ) -> Pin<
        Box<
            dyn Future<
                    Output = Result<RemoteTransportCarrierDiagnostic, RemoteTransportCarrierError>,
                > + Send
                + 'a,
        >,
    > {
        Box::pin(self.connect_inner(attempt))
    }
}

fn tls_server_identity_name(identity: &str) -> Result<String, RemoteTransportCarrierError> {
    let trimmed = identity.trim();
    let (identity_kind, name) = if let Some(name) = trimmed.strip_prefix("dns:") {
        ("dns", name.trim())
    } else if let Some(name) = trimmed.strip_prefix("ip:") {
        ("ip", name.trim())
    } else {
        ("untyped", trimmed)
    };
    if name.is_empty() || name.contains('/') {
        return Err(RemoteTransportCarrierError::InvalidPolicy {
            reason: "TLS policy server identity must be a DNS name or IP address".to_string(),
        });
    }
    match identity_kind {
        "ip" if name.parse::<IpAddr>().is_err() => {
            return Err(RemoteTransportCarrierError::InvalidPolicy {
                reason: "TLS policy ip: server identity must be an IP literal".to_string(),
            });
        }
        "dns" if name.parse::<IpAddr>().is_ok() => {
            return Err(RemoteTransportCarrierError::InvalidPolicy {
                reason: "TLS policy dns: server identity must be a DNS name".to_string(),
            });
        }
        _ => {}
    }
    Ok(name.to_string())
}

/// Production transport-core configuration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoteTransportConfig {
    /// Whether the production transport state machine is enabled.
    pub enabled: bool,
    /// Maximum typed-envelope frame size.
    pub max_frame_bytes: u64,
    /// Maximum in-flight frames.
    pub max_inflight_frames: u32,
    /// Bounded replay window size.
    pub replay_window_size: u32,
    /// Whether an agent package descriptor is required after handshake.
    pub require_agent_package: bool,
}

impl RemoteTransportConfig {
    /// Return an enabled production-core configuration for tests/composition.
    pub fn enabled() -> Self {
        Self {
            enabled: true,
            ..Self::default()
        }
    }
}

impl Default for RemoteTransportConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            max_frame_bytes: 256 * 1024,
            max_inflight_frames: 16,
            replay_window_size: 256,
            require_agent_package: true,
        }
    }
}

/// Outcome of accepting one frame descriptor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RemoteTransportAcceptOutcome {
    /// Frame was accepted as a new operation.
    Accepted,
    /// Operation was already seen and was treated as idempotent duplicate metadata.
    Duplicate,
}

/// Remote transport fixture configuration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoteTransportFixtureConfig {
    /// Whether deterministic transport fixture behavior is enabled.
    pub enabled: bool,
    /// Maximum accepted frame size.
    pub max_frame_bytes: u64,
}

impl RemoteTransportFixtureConfig {
    /// Return an enabled deterministic fixture configuration.
    pub fn enabled() -> Self {
        Self {
            enabled: true,
            ..Self::default()
        }
    }
}

impl Default for RemoteTransportFixtureConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            max_frame_bytes: 256 * 1024,
        }
    }
}

/// Deterministic metadata-only remote transport fixture.
#[derive(Debug, Clone)]
pub struct RemoteTransportFixture {
    config: RemoteTransportFixtureConfig,
    session_id: Option<RemoteWorkspaceSessionId>,
    accepted_operations: HashSet<RemoteOperationId>,
    last_sequence: EventSequence,
}

impl RemoteTransportFixture {
    /// Construct a fixture from configuration.
    pub fn new(config: RemoteTransportFixtureConfig) -> Self {
        Self {
            config,
            session_id: None,
            accepted_operations: HashSet::new(),
            last_sequence: EventSequence(0),
        }
    }

    /// Accept a validated metadata-only handshake and return initial health metadata.
    pub fn accept_handshake(
        &mut self,
        handshake: RemoteTransportHandshake,
    ) -> Result<RemoteTransportHealthSummary, RemoteTransportFixtureError> {
        if !self.config.enabled {
            return Err(RemoteTransportFixtureError::Disabled);
        }
        validate_remote_transport_handshake(&handshake).map_err(|err| {
            RemoteTransportFixtureError::InvalidHandshake {
                reason: err.message,
            }
        })?;
        self.session_id = Some(handshake.session_id);
        self.last_sequence = handshake.event_sequence;
        Ok(RemoteTransportHealthSummary {
            session_id: handshake.session_id,
            health: RemoteNetworkHealthState::Healthy,
            last_operation_id: None,
            queued_frame_count: 0,
            reconnect_attempts: 0,
            event_sequence: handshake.event_sequence,
            correlation_id: handshake.correlation_id,
            causality_id: handshake.causality_id,
            redaction_hints: vec![RedactionHint::MetadataOnly],
            schema_version: 1,
        })
    }

    /// Accept one metadata-only frame descriptor without touching raw network payloads.
    pub fn accept_frame(
        &mut self,
        frame: RemoteTransportFrameMetadata,
    ) -> Result<RemoteTransportAuditSummary, RemoteTransportFixtureError> {
        if !self.config.enabled {
            return Err(RemoteTransportFixtureError::Disabled);
        }
        if Some(frame.session_id) != self.session_id {
            return Err(RemoteTransportFixtureError::InvalidFrame {
                reason: "frame session does not match accepted handshake".to_string(),
            });
        }
        if frame.schema_version == 0
            || frame.frame_sequence.0 == 0
            || frame.envelope_byte_len == 0
            || frame.max_frame_bytes == 0
            || frame.envelope_byte_len > frame.max_frame_bytes
            || frame.max_frame_bytes > self.config.max_frame_bytes
        {
            return Err(RemoteTransportFixtureError::InvalidFrame {
                reason: "frame metadata is missing required bounds".to_string(),
            });
        }
        let duplicate = !self.accepted_operations.insert(frame.operation_id);
        self.last_sequence = frame.frame_sequence;
        let summary = RemoteTransportAuditSummary {
            session_id: frame.session_id,
            event_sequence: frame.frame_sequence,
            correlation_id: devil_protocol::CorrelationId(frame.frame_sequence.0),
            causality_id: devil_protocol::CausalityId(uuid_from_sequence(frame.frame_sequence.0)),
            metadata_summary: format!(
                "operation_id={} bytes={} compressed={} duplicate={}",
                frame.operation_id.0, frame.envelope_byte_len, frame.compressed, duplicate
            ),
            redaction_hints: vec![RedactionHint::MetadataOnly],
            schema_version: 1,
        };
        validate_remote_transport_audit_summary(&summary).map_err(|err| {
            RemoteTransportFixtureError::InvalidFrame {
                reason: err.message,
            }
        })?;
        Ok(summary)
    }
}

/// Socket-independent production remote transport state machine.
#[derive(Debug, Clone)]
pub struct RemoteTransportStateMachine {
    config: RemoteTransportConfig,
    state: RemoteTransportLifecycleState,
    session_id: Option<RemoteWorkspaceSessionId>,
    peer_identity: Option<RemoteTransportPeerIdentity>,
    agent_package: Option<RemoteAgentPackageDescriptor>,
    seen_operations: HashSet<RemoteOperationId>,
    accepted_sequences: VecDeque<EventSequence>,
    inflight_operations: HashSet<RemoteOperationId>,
    duplicate_operations: u32,
    last_sequence: EventSequence,
    last_checkpoint: Option<RemoteOperationLogCheckpointId>,
    resume_token_digest: Option<String>,
    correlation_id: devil_protocol::CorrelationId,
    causality_id: CausalityId,
}

impl RemoteTransportStateMachine {
    /// Construct a disabled-by-default production state machine.
    pub fn new(config: RemoteTransportConfig) -> Self {
        Self {
            config,
            state: RemoteTransportLifecycleState::Created,
            session_id: None,
            peer_identity: None,
            agent_package: None,
            seen_operations: HashSet::new(),
            accepted_sequences: VecDeque::new(),
            inflight_operations: HashSet::new(),
            duplicate_operations: 0,
            last_sequence: EventSequence(0),
            last_checkpoint: None,
            resume_token_digest: None,
            correlation_id: devil_protocol::CorrelationId(1),
            causality_id: CausalityId(uuid_from_sequence(1)),
        }
    }

    /// Return current transport lifecycle state.
    pub fn state(&self) -> RemoteTransportLifecycleState {
        self.state
    }

    /// Begin handshake negotiation.
    pub fn begin_handshake(&mut self) -> Result<(), RemoteTransportCoreError> {
        self.ensure_enabled()?;
        if self.state != RemoteTransportLifecycleState::Created {
            return Err(RemoteTransportCoreError::InvalidState {
                reason: "handshake can only begin from created state".to_string(),
            });
        }
        self.state = RemoteTransportLifecycleState::Handshaking;
        Ok(())
    }

    /// Accept validated handshake metadata and enter active or package-pending state.
    pub fn accept_handshake(
        &mut self,
        handshake: RemoteTransportHandshake,
    ) -> Result<RemoteTransportHealthSummary, RemoteTransportCoreError> {
        self.ensure_enabled()?;
        if self.state != RemoteTransportLifecycleState::Handshaking {
            return Err(RemoteTransportCoreError::InvalidState {
                reason: "handshake must be negotiated before acceptance".to_string(),
            });
        }
        validate_remote_transport_handshake(&handshake).map_err(|err| {
            RemoteTransportCoreError::InvalidHandshake {
                reason: err.message,
            }
        })?;
        self.session_id = Some(handshake.session_id);
        self.peer_identity = Some(handshake.peer_identity);
        self.last_sequence = handshake.event_sequence;
        self.correlation_id = handshake.correlation_id;
        self.causality_id = handshake.causality_id;
        self.state = RemoteTransportLifecycleState::Active;
        Ok(self.health_summary(RemoteNetworkHealthState::Healthy))
    }

    /// Activate a remote agent package after handshake.
    pub fn activate_agent_package(
        &mut self,
        package: RemoteAgentPackageDescriptor,
    ) -> Result<(), RemoteTransportCoreError> {
        self.ensure_enabled()?;
        let Some(peer) = &self.peer_identity else {
            return Err(RemoteTransportCoreError::InvalidState {
                reason: "agent package requires accepted peer identity".to_string(),
            });
        };
        validate_remote_agent_package_descriptor(&package).map_err(|err| {
            RemoteTransportCoreError::InvalidAgentPackage {
                reason: err.message,
            }
        })?;
        if package.agent_id != peer.agent_id || package.authority_id != peer.authority_id {
            return Err(RemoteTransportCoreError::InvalidAgentPackage {
                reason: "agent package does not match transport peer".to_string(),
            });
        }
        self.agent_package = Some(package);
        Ok(())
    }

    /// Accept one metadata-only frame descriptor with order, replay, and flow-control checks.
    pub fn try_accept_frame(
        &mut self,
        frame: RemoteTransportFrameMetadata,
    ) -> Result<RemoteTransportAcceptOutcome, RemoteTransportCoreError> {
        self.ensure_enabled()?;
        if !matches!(
            self.state,
            RemoteTransportLifecycleState::Active | RemoteTransportLifecycleState::Backpressured
        ) {
            return Err(RemoteTransportCoreError::InvalidState {
                reason: "frames require active transport state".to_string(),
            });
        }
        if self.config.require_agent_package && self.agent_package.is_none() {
            return Err(RemoteTransportCoreError::InvalidAgentPackage {
                reason: "agent package activation is required before frames".to_string(),
            });
        }
        if Some(frame.session_id) != self.session_id {
            return Err(RemoteTransportCoreError::InvalidFrame {
                reason: "frame session does not match transport session".to_string(),
            });
        }
        if frame.schema_version == 0
            || frame.frame_sequence.0 == 0
            || frame.envelope_byte_len == 0
            || frame.max_frame_bytes == 0
            || frame.envelope_byte_len > frame.max_frame_bytes
            || frame.envelope_byte_len > self.config.max_frame_bytes
        {
            return Err(RemoteTransportCoreError::InvalidFrame {
                reason: "frame metadata is missing required bounds".to_string(),
            });
        }
        if self.seen_operations.contains(&frame.operation_id) {
            self.duplicate_operations = self.duplicate_operations.saturating_add(1);
            return Ok(RemoteTransportAcceptOutcome::Duplicate);
        }
        if frame.frame_sequence.0 != self.last_sequence.0.saturating_add(1) {
            return Err(RemoteTransportCoreError::ReplayRejected {
                reason: "frame sequence must be contiguous and forward-only".to_string(),
            });
        }
        if self.inflight_operations.len() as u32 >= self.config.max_inflight_frames {
            self.state = RemoteTransportLifecycleState::Backpressured;
            return Err(RemoteTransportCoreError::Backpressured);
        }
        self.seen_operations.insert(frame.operation_id);
        self.inflight_operations.insert(frame.operation_id);
        self.accepted_sequences.push_back(frame.frame_sequence);
        while self.accepted_sequences.len() > self.config.replay_window_size as usize {
            self.accepted_sequences.pop_front();
        }
        self.last_sequence = frame.frame_sequence;
        if self.inflight_operations.len() as u32 >= self.config.max_inflight_frames {
            self.state = RemoteTransportLifecycleState::Backpressured;
        } else {
            self.state = RemoteTransportLifecycleState::Active;
        }
        Ok(RemoteTransportAcceptOutcome::Accepted)
    }

    /// Acknowledge one in-flight frame and return the updated flow-control window.
    pub fn ack_frame(
        &mut self,
        operation_id: RemoteOperationId,
    ) -> Result<RemoteTransportFlowControlWindow, RemoteTransportCoreError> {
        self.ensure_enabled()?;
        if !self.inflight_operations.remove(&operation_id) {
            return Err(RemoteTransportCoreError::InvalidFrame {
                reason: "acknowledged operation was not in flight".to_string(),
            });
        }
        self.state = RemoteTransportLifecycleState::Active;
        self.flow_control_window()
    }

    /// Record a transport checkpoint and return replay-window metadata.
    pub fn checkpoint(
        &mut self,
        checkpoint: RemoteOperationLogCheckpoint,
    ) -> Result<RemoteTransportReplayWindow, RemoteTransportCoreError> {
        self.ensure_enabled()?;
        if Some(checkpoint.session_id) != self.session_id || checkpoint.event_sequence.0 == 0 {
            return Err(RemoteTransportCoreError::ReplayRejected {
                reason: "checkpoint does not match transport session".to_string(),
            });
        }
        if !self.seen_operations.contains(&checkpoint.last_operation_id) {
            return Err(RemoteTransportCoreError::ReplayRejected {
                reason: "checkpoint references an unseen operation".to_string(),
            });
        }
        self.last_checkpoint = Some(checkpoint.checkpoint_id);
        self.replay_window()
    }

    /// Issue a metadata-only resume token digest for the latest checkpoint.
    pub fn issue_resume_token(
        &mut self,
        token_digest: impl Into<String>,
        expires_at: TimestampMillis,
    ) -> Result<RemoteTransportResumeToken, RemoteTransportCoreError> {
        self.ensure_enabled()?;
        let Some(session_id) = self.session_id else {
            return Err(RemoteTransportCoreError::ResumeRejected {
                reason: "resume token requires active session".to_string(),
            });
        };
        let Some(checkpoint_id) = self.last_checkpoint else {
            return Err(RemoteTransportCoreError::ResumeRejected {
                reason: "resume token requires checkpoint".to_string(),
            });
        };
        let token_digest = token_digest.into();
        if token_digest.trim().is_empty() || expires_at.0 == 0 {
            return Err(RemoteTransportCoreError::ResumeRejected {
                reason: "resume token digest and expiry are required".to_string(),
            });
        }
        self.resume_token_digest = Some(token_digest.clone());
        Ok(RemoteTransportResumeToken {
            session_id,
            token_digest,
            checkpoint_id,
            expires_at,
            schema_version: 1,
        })
    }

    /// Begin resume with a previously issued digest token.
    pub fn begin_resume(
        &mut self,
        token: RemoteTransportResumeToken,
        now: TimestampMillis,
    ) -> Result<(), RemoteTransportCoreError> {
        self.ensure_enabled()?;
        if Some(token.session_id) != self.session_id
            || Some(token.checkpoint_id) != self.last_checkpoint
            || self.resume_token_digest.as_deref() != Some(token.token_digest.as_str())
            || token.expires_at.0 <= now.0
            || token.schema_version == 0
        {
            return Err(RemoteTransportCoreError::ResumeRejected {
                reason: "resume token does not match active transport checkpoint".to_string(),
            });
        }
        self.state = RemoteTransportLifecycleState::Resuming;
        Ok(())
    }

    /// Complete resume with a matching offline manifest.
    pub fn complete_resume(
        &mut self,
        manifest: RemoteOfflineResumeManifest,
    ) -> Result<RemoteTransportHealthSummary, RemoteTransportCoreError> {
        self.ensure_enabled()?;
        if self.state != RemoteTransportLifecycleState::Resuming {
            return Err(RemoteTransportCoreError::InvalidState {
                reason: "resume completion requires resuming state".to_string(),
            });
        }
        if Some(manifest.session_id) != self.session_id
            || manifest.schema_version == 0
            || manifest.correlation_id.0 == 0
            || manifest.causality_id.0.is_nil()
            || !self
                .last_checkpoint
                .is_some_and(|checkpoint| manifest.checkpoints.contains(&checkpoint))
        {
            return Err(RemoteTransportCoreError::ResumeRejected {
                reason: "resume manifest does not include the transport checkpoint".to_string(),
            });
        }
        self.state = RemoteTransportLifecycleState::Active;
        Ok(self.health_summary(RemoteNetworkHealthState::Healthy))
    }

    /// Return current flow-control metadata.
    pub fn flow_control_window(
        &self,
    ) -> Result<RemoteTransportFlowControlWindow, RemoteTransportCoreError> {
        let Some(session_id) = self.session_id else {
            return Err(RemoteTransportCoreError::InvalidState {
                reason: "flow-control window requires active session".to_string(),
            });
        };
        let window = RemoteTransportFlowControlWindow {
            session_id,
            max_inflight_frames: self.config.max_inflight_frames,
            available_credit: self
                .config
                .max_inflight_frames
                .saturating_sub(self.inflight_operations.len() as u32),
            max_frame_bytes: self.config.max_frame_bytes,
            queued_frame_count: self.inflight_operations.len() as u32,
            last_accepted_sequence: self.last_sequence,
            correlation_id: self.correlation_id,
            causality_id: self.causality_id,
            schema_version: 1,
        };
        validate_remote_transport_flow_control_window(&window).map_err(|err| {
            RemoteTransportCoreError::InvalidFrame {
                reason: err.message,
            }
        })?;
        Ok(window)
    }

    /// Return current replay-window metadata.
    pub fn replay_window(&self) -> Result<RemoteTransportReplayWindow, RemoteTransportCoreError> {
        let Some(session_id) = self.session_id else {
            return Err(RemoteTransportCoreError::InvalidState {
                reason: "replay window requires active session".to_string(),
            });
        };
        let Some(lowest) = self.accepted_sequences.front().copied() else {
            return Err(RemoteTransportCoreError::ReplayRejected {
                reason: "replay window has no accepted frames".to_string(),
            });
        };
        let window = RemoteTransportReplayWindow {
            session_id,
            lowest_accepted_sequence: lowest,
            highest_accepted_sequence: self.last_sequence,
            accepted_operation_count: self.seen_operations.len() as u32,
            duplicate_operation_count: self.duplicate_operations,
            checkpoint_id: self.last_checkpoint,
            schema_version: 1,
        };
        validate_remote_transport_replay_window(&window).map_err(|err| {
            RemoteTransportCoreError::ReplayRejected {
                reason: err.message,
            }
        })?;
        Ok(window)
    }

    /// Build metadata-only health summary.
    pub fn health_summary(&self, health: RemoteNetworkHealthState) -> RemoteTransportHealthSummary {
        RemoteTransportHealthSummary {
            session_id: self.session_id.unwrap_or(RemoteWorkspaceSessionId(0)),
            health,
            last_operation_id: None,
            queued_frame_count: self.inflight_operations.len() as u32,
            reconnect_attempts: matches!(
                self.state,
                RemoteTransportLifecycleState::Reconnecting
                    | RemoteTransportLifecycleState::Resuming
            ) as u32,
            event_sequence: self.last_sequence,
            correlation_id: self.correlation_id,
            causality_id: self.causality_id,
            redaction_hints: vec![RedactionHint::MetadataOnly],
            schema_version: 1,
        }
    }

    /// Build metadata-only transport audit summary.
    pub fn audit_summary(&self) -> Result<RemoteTransportAuditSummary, RemoteTransportCoreError> {
        let Some(session_id) = self.session_id else {
            return Err(RemoteTransportCoreError::InvalidState {
                reason: "audit requires active session".to_string(),
            });
        };
        let summary = RemoteTransportAuditSummary {
            session_id,
            event_sequence: self.last_sequence,
            correlation_id: self.correlation_id,
            causality_id: self.causality_id,
            metadata_summary: format!(
                "state={:?} accepted={} duplicates={} inflight={} package_active={}",
                self.state,
                self.seen_operations.len(),
                self.duplicate_operations,
                self.inflight_operations.len(),
                self.agent_package.is_some()
            ),
            redaction_hints: vec![RedactionHint::MetadataOnly],
            schema_version: 1,
        };
        validate_remote_transport_audit_summary(&summary).map_err(|err| {
            RemoteTransportCoreError::InvalidFrame {
                reason: err.message,
            }
        })?;
        Ok(summary)
    }

    fn ensure_enabled(&self) -> Result<(), RemoteTransportCoreError> {
        if self.config.enabled {
            Ok(())
        } else {
            Err(RemoteTransportCoreError::Disabled)
        }
    }
}

fn uuid_from_sequence(sequence: u64) -> uuid::Uuid {
    uuid::Uuid::from_u128(0x018f_0000_0000_7000_8000_0000_0000_0000_u128 + sequence as u128)
}

fn load_certs(path: &PathBuf) -> Result<Vec<CertificateDer<'static>>, RemoteTransportCarrierError> {
    let certs = CertificateDer::pem_file_iter(path)
        .map_err(|err| RemoteTransportCarrierError::Credential {
            reason: format!("open certificate PEM `{}`: {err}", path.display()),
        })?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|err| RemoteTransportCarrierError::Credential {
            reason: format!("decode certificate PEM `{}`: {err}", path.display()),
        })?;
    if certs.is_empty() {
        return Err(RemoteTransportCarrierError::Credential {
            reason: format!(
                "certificate PEM `{}` contained no certificates",
                path.display()
            ),
        });
    }
    Ok(certs)
}

fn load_private_key(path: &PathBuf) -> Result<PrivateKeyDer<'static>, RemoteTransportCarrierError> {
    PrivateKeyDer::from_pem_file(path).map_err(|err| RemoteTransportCarrierError::Credential {
        reason: format!("decode private key PEM `{}`: {err}", path.display()),
    })
}

fn remaining_attempt_budget(deadline: Instant) -> Option<Duration> {
    let remaining = deadline.saturating_duration_since(Instant::now());
    if remaining.is_zero() {
        return None;
    }
    Some(remaining)
}

#[cfg(test)]
mod tests {
    use devil_protocol::{
        CapabilityDecision, CapabilityDecisionId, CapabilityId, CollaborationVersionVector,
        CorrelationId, FileFingerprint, PrincipalId, RemoteAgentId, RemoteAuthorityId,
        RemoteOperationLogCheckpoint, RemoteOperationLogCheckpointId,
        RemoteTransportConnectionAttempt, RemoteTransportCredentialReference,
        RemoteTransportEndpointDescriptor, RemoteTransportEndpointPolicy,
        RemoteTransportMutualTlsMode, RemoteTransportPeerIdentity,
        RemoteTransportSchemaCompatibility, RemoteTransportTlsPolicy, SnapshotId, TimestampMillis,
        WorkspaceGeneration, WorkspaceTrustState,
    };

    use super::*;

    fn handshake() -> RemoteTransportHandshake {
        RemoteTransportHandshake {
            session_id: RemoteWorkspaceSessionId(1),
            endpoint: RemoteTransportEndpointDescriptor {
                endpoint_id: "loopback".to_string(),
                scheme: "https".to_string(),
                host: "localhost".to_string(),
                port: Some(9443),
                loopback_only: true,
                schema_version: 1,
            },
            peer_identity: RemoteTransportPeerIdentity {
                authority_id: RemoteAuthorityId(2),
                agent_id: RemoteAgentId(3),
                principal_id: PrincipalId("tester".to_string()),
                credential_reference: "cert:sha256:test".to_string(),
                schema_version: 1,
            },
            trust_state: WorkspaceTrustState::Trusted,
            schema_compatibility: RemoteTransportSchemaCompatibility::Exact,
            capability_decision: CapabilityDecision {
                decision_id: CapabilityDecisionId(4),
                granted: true,
                capability: CapabilityId("remote.session.connect".to_string()),
                reason: None,
            },
            correlation_id: CorrelationId(5),
            causality_id: devil_protocol::CausalityId(uuid_from_sequence(5)),
            event_sequence: EventSequence(6),
            schema_version: 1,
        }
    }

    fn frame() -> RemoteTransportFrameMetadata {
        RemoteTransportFrameMetadata {
            session_id: RemoteWorkspaceSessionId(1),
            operation_id: RemoteOperationId(7),
            frame_sequence: EventSequence(8),
            envelope_byte_len: 128,
            max_frame_bytes: 1024,
            compressed: false,
            schema_version: 1,
        }
    }

    fn ordered_frame(sequence: u64, operation: u64) -> RemoteTransportFrameMetadata {
        RemoteTransportFrameMetadata {
            frame_sequence: EventSequence(sequence),
            operation_id: RemoteOperationId(operation as u128),
            ..frame()
        }
    }

    fn package() -> RemoteAgentPackageDescriptor {
        RemoteAgentPackageDescriptor {
            agent_id: RemoteAgentId(3),
            authority_id: RemoteAuthorityId(2),
            package_id: "agent-package".to_string(),
            version: "1.0.0".to_string(),
            package_digest: FileFingerprint {
                algorithm: "sha256".to_string(),
                value: "abc123".to_string(),
            },
            signature_reference: "sig:sha256:def".to_string(),
            declared_capabilities: vec![CapabilityId("remote.transport.connect".to_string())],
            capability_decision: CapabilityDecision {
                decision_id: CapabilityDecisionId(44),
                granted: true,
                capability: CapabilityId("remote.agent.package.activate".to_string()),
                reason: None,
            },
            redaction_hints: vec![RedactionHint::MetadataOnly],
            schema_version: 1,
        }
    }

    fn connection_attempt() -> RemoteTransportConnectionAttempt {
        RemoteTransportConnectionAttempt {
            endpoint_policy: RemoteTransportEndpointPolicy {
                endpoint: RemoteTransportEndpointDescriptor {
                    endpoint_id: "loopback".to_string(),
                    scheme: "https".to_string(),
                    host: "localhost".to_string(),
                    port: Some(9443),
                    loopback_only: true,
                    schema_version: 1,
                },
                allowed_schemes: vec!["https".to_string()],
                redirects_allowed: false,
                schema_version: 1,
            },
            tls_policy: RemoteTransportTlsPolicy {
                require_tls: true,
                server_identity: "dns:localhost".to_string(),
                root_store_reference: None,
                certificate_pin_reference: None,
                mtls_mode: RemoteTransportMutualTlsMode::Optional,
                client_credential_reference: None,
                alpn_protocols: vec!["devil-remote/phase8".to_string()],
                min_schema_version: 1,
                max_schema_version: 1,
                schema_version: 1,
            },
            selected_alpn: "devil-remote/phase8".to_string(),
            selected_schema_version: 1,
            timeout_ms: 100,
            cancellation_requested: false,
            capability_decision: CapabilityDecision {
                decision_id: CapabilityDecisionId(55),
                granted: true,
                capability: CapabilityId("remote.transport.connect".to_string()),
                reason: None,
            },
            event_sequence: EventSequence(9),
            correlation_id: CorrelationId(9),
            causality_id: CausalityId(uuid_from_sequence(9)),
            metadata_summary: "tls=required mtls=optional".to_string(),
            redaction_hints: vec![RedactionHint::MetadataOnly],
            schema_version: 1,
        }
    }

    fn active_machine() -> RemoteTransportStateMachine {
        let mut machine = RemoteTransportStateMachine::new(RemoteTransportConfig::enabled());
        machine.begin_handshake().expect("begin handshake");
        machine.accept_handshake(handshake()).expect("handshake");
        machine
            .activate_agent_package(package())
            .expect("activate package");
        machine
    }

    #[test]
    fn remote_transport_fixture_is_default_off() {
        let mut fixture = RemoteTransportFixture::new(RemoteTransportFixtureConfig::default());
        assert!(matches!(
            fixture.accept_handshake(handshake()),
            Err(RemoteTransportFixtureError::Disabled)
        ));
    }

    #[test]
    fn remote_transport_fixture_accepts_handshake_and_bounded_frames() {
        let mut fixture = RemoteTransportFixture::new(RemoteTransportFixtureConfig::enabled());
        let health = fixture.accept_handshake(handshake()).expect("handshake");
        assert_eq!(health.health, RemoteNetworkHealthState::Healthy);

        let audit = fixture.accept_frame(frame()).expect("frame");
        assert!(audit.metadata_summary.contains("bytes=128"));
        assert!(!audit.metadata_summary.contains("transport_payload"));
    }

    #[test]
    fn remote_transport_fixture_rejects_oversized_frames() {
        let mut fixture = RemoteTransportFixture::new(RemoteTransportFixtureConfig::enabled());
        fixture.accept_handshake(handshake()).expect("handshake");
        let oversized = RemoteTransportFrameMetadata {
            envelope_byte_len: 2048,
            max_frame_bytes: 1024,
            ..frame()
        };
        assert!(matches!(
            fixture.accept_frame(oversized),
            Err(RemoteTransportFixtureError::InvalidFrame { .. })
        ));
    }

    #[test]
    fn remote_transport_state_machine_requires_ordered_handshake_before_frames() {
        let mut machine = RemoteTransportStateMachine::new(RemoteTransportConfig::enabled());
        assert!(matches!(
            machine.try_accept_frame(ordered_frame(7, 7)),
            Err(RemoteTransportCoreError::InvalidState { .. })
        ));
        machine.begin_handshake().expect("begin handshake");
        machine.accept_handshake(handshake()).expect("handshake");
        assert!(matches!(
            machine.try_accept_frame(ordered_frame(7, 7)),
            Err(RemoteTransportCoreError::InvalidAgentPackage { .. })
        ));
    }

    #[test]
    fn remote_transport_state_machine_enforces_flow_control_credit() {
        let mut machine = RemoteTransportStateMachine::new(RemoteTransportConfig {
            max_inflight_frames: 1,
            ..RemoteTransportConfig::enabled()
        });
        machine.begin_handshake().expect("begin handshake");
        machine.accept_handshake(handshake()).expect("handshake");
        machine
            .activate_agent_package(package())
            .expect("activate package");
        assert_eq!(
            machine.try_accept_frame(ordered_frame(7, 7)),
            Ok(RemoteTransportAcceptOutcome::Accepted)
        );
        assert_eq!(
            machine.state(),
            RemoteTransportLifecycleState::Backpressured
        );
        assert!(matches!(
            machine.try_accept_frame(ordered_frame(8, 8)),
            Err(RemoteTransportCoreError::Backpressured)
        ));
        let flow = machine.ack_frame(RemoteOperationId(7)).expect("ack");
        assert_eq!(flow.available_credit, 1);
        assert_eq!(machine.state(), RemoteTransportLifecycleState::Active);
    }

    #[test]
    fn remote_transport_state_machine_rejects_sequence_gap_and_duplicate_is_idempotent() {
        let mut machine = active_machine();
        assert!(matches!(
            machine.try_accept_frame(ordered_frame(8, 8)),
            Err(RemoteTransportCoreError::ReplayRejected { .. })
        ));
        assert_eq!(
            machine.try_accept_frame(ordered_frame(7, 7)),
            Ok(RemoteTransportAcceptOutcome::Accepted)
        );
        assert_eq!(
            machine.try_accept_frame(ordered_frame(7, 7)),
            Ok(RemoteTransportAcceptOutcome::Duplicate)
        );
        let audit = machine.audit_summary().expect("audit");
        assert!(audit.metadata_summary.contains("duplicates=1"));
        assert!(!audit.metadata_summary.contains("transport_payload"));
    }

    #[test]
    fn rustls_mtls_carrier_is_default_off() {
        let carrier = RustlsMtlsCarrier::new(RustlsMtlsCarrierConfig::default());
        assert!(matches!(
            carrier.build_client_config(&connection_attempt()),
            Err(RemoteTransportCarrierError::Disabled)
        ));
    }

    #[test]
    fn rustls_mtls_carrier_builds_optional_mtls_config_without_inline_material() {
        let carrier = RustlsMtlsCarrier::new(RustlsMtlsCarrierConfig::enabled());
        let config = carrier
            .build_client_config(&connection_attempt())
            .expect("optional mTLS config");
        assert_eq!(config.alpn_protocols, vec![b"devil-remote/phase8".to_vec()]);
    }

    #[test]
    fn tls_server_identity_is_policy_bound_and_normalized() {
        assert_eq!(
            tls_server_identity_name("dns:localhost").expect("dns identity"),
            "localhost"
        );
        assert_eq!(
            tls_server_identity_name("ip:127.0.0.1").expect("ip identity"),
            "127.0.0.1"
        );
        assert_eq!(
            tls_server_identity_name("ip:::1").expect("ipv6 identity"),
            "::1"
        );
        assert!(matches!(
            tls_server_identity_name("ip:example.com"),
            Err(RemoteTransportCarrierError::InvalidPolicy { .. })
        ));
        assert!(matches!(
            tls_server_identity_name("dns:127.0.0.1"),
            Err(RemoteTransportCarrierError::InvalidPolicy { .. })
        ));
        assert!(matches!(
            tls_server_identity_name("spiffe://example/workload"),
            Err(RemoteTransportCarrierError::InvalidPolicy { .. })
        ));
    }

    #[test]
    fn rustls_mtls_carrier_rejects_required_mtls_without_files_before_network() {
        let carrier = RustlsMtlsCarrier::new(RustlsMtlsCarrierConfig::enabled());
        let attempt = RemoteTransportConnectionAttempt {
            tls_policy: RemoteTransportTlsPolicy {
                mtls_mode: RemoteTransportMutualTlsMode::Required,
                client_credential_reference: Some(RemoteTransportCredentialReference {
                    reference_id: "client-cert-ref".to_string(),
                    kind: "client-cert".to_string(),
                    digest: FileFingerprint {
                        algorithm: "sha256".to_string(),
                        value: "abc123".to_string(),
                    },
                    schema_version: 1,
                }),
                ..connection_attempt().tls_policy
            },
            ..connection_attempt()
        };
        assert!(matches!(
            carrier.build_client_config(&attempt),
            Err(RemoteTransportCarrierError::Credential { .. })
        ));
    }

    #[test]
    fn rustls_mtls_carrier_short_circuits_canceled_attempt_before_credentials_or_network() {
        let carrier = RustlsMtlsCarrier::new(RustlsMtlsCarrierConfig::enabled());
        let attempt = RemoteTransportConnectionAttempt {
            cancellation_requested: true,
            tls_policy: RemoteTransportTlsPolicy {
                mtls_mode: RemoteTransportMutualTlsMode::Required,
                client_credential_reference: Some(RemoteTransportCredentialReference {
                    reference_id: "client-cert-ref".to_string(),
                    kind: "client-cert".to_string(),
                    digest: FileFingerprint {
                        algorithm: "sha256".to_string(),
                        value: "abc123".to_string(),
                    },
                    schema_version: 1,
                }),
                ..connection_attempt().tls_policy
            },
            ..connection_attempt()
        };
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("tokio runtime");
        let err = runtime
            .block_on(carrier.connect(attempt))
            .expect_err("canceled before credential loading or network dial");
        assert!(matches!(err, RemoteTransportCarrierError::Canceled { .. }));
    }

    #[test]
    fn rustls_mtls_carrier_uses_single_attempt_timeout_budget() {
        let expired = Instant::now() - Duration::from_millis(1);
        assert_eq!(remaining_attempt_budget(expired), None);
    }

    #[test]
    fn remote_transport_state_machine_resume_requires_matching_token_checkpoint() {
        let mut machine = active_machine();
        machine
            .try_accept_frame(ordered_frame(7, 7))
            .expect("accept frame");
        let checkpoint = RemoteOperationLogCheckpoint {
            checkpoint_id: RemoteOperationLogCheckpointId(99),
            session_id: RemoteWorkspaceSessionId(1),
            last_operation_id: RemoteOperationId(7),
            version_vector: CollaborationVersionVector { entries: vec![] },
            network_health: RemoteNetworkHealthState::Healthy,
            event_sequence: EventSequence(7),
            schema_version: 1,
        };
        machine.checkpoint(checkpoint).expect("checkpoint");
        let token = machine
            .issue_resume_token("digest", TimestampMillis(10_000))
            .expect("token");
        assert!(matches!(
            machine.begin_resume(
                RemoteTransportResumeToken {
                    checkpoint_id: RemoteOperationLogCheckpointId(100),
                    ..token.clone()
                },
                TimestampMillis(1_000)
            ),
            Err(RemoteTransportCoreError::ResumeRejected { .. })
        ));
        machine
            .begin_resume(token, TimestampMillis(1_000))
            .expect("begin resume");
        let manifest = RemoteOfflineResumeManifest {
            session_id: RemoteWorkspaceSessionId(1),
            checkpoints: vec![RemoteOperationLogCheckpointId(99)],
            workspace_generation: WorkspaceGeneration(1),
            snapshot_id: SnapshotId(1),
            correlation_id: CorrelationId(5),
            causality_id: CausalityId(uuid_from_sequence(5)),
            event_sequence: EventSequence(8),
            schema_version: 1,
        };
        machine.complete_resume(manifest).expect("complete resume");
        assert_eq!(machine.state(), RemoteTransportLifecycleState::Active);
    }

    #[test]
    fn remote_transport_state_machine_agent_package_must_match_peer_and_capability() {
        let mut machine = RemoteTransportStateMachine::new(RemoteTransportConfig::enabled());
        machine.begin_handshake().expect("begin handshake");
        machine.accept_handshake(handshake()).expect("handshake");
        let wrong_peer = RemoteAgentPackageDescriptor {
            agent_id: RemoteAgentId(99),
            ..package()
        };
        assert!(matches!(
            machine.activate_agent_package(wrong_peer),
            Err(RemoteTransportCoreError::InvalidAgentPackage { .. })
        ));
    }
}
