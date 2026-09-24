//! Binds a remote session to the production transport state machine.
//!
//! This module is the activation point for `legion-remote-transport` (roadmap
//! P9.F3.T2, ADR-0046 Amendment 1). Until it existed that crate had zero
//! fan-in: a complete handshake/replay/resume state machine that no product
//! path could reach.
//!
//! ## Why binding them is the work, rather than glue over it
//!
//! Both layers already had a reconnect story, and they did not agree.
//!
//! [`RemoteSessionRuntime`] tracks session lifecycle and network health, with
//! `begin_reconnect` / `complete_reconnect`. The doc comment on
//! `complete_reconnect` says it completes "after identity, cache, and version
//! preconditions are externally validated" — and nothing external validated
//! them. Any caller could move a session straight back to `Active`.
//!
//! The transport state machine has the checks that comment is describing:
//! a resume token bound to a checkpoint, an offline manifest that must contain
//! that checkpoint, and a replay window. What it lacked was any way to reach
//! them from a session.
//!
//! So the two halves are joined here: a drop moves both layers, and a resume
//! must satisfy the transport's token and manifest checks before the session is
//! allowed back to `Active`. "Externally validated" now names something.
//!
//! ## What this deliberately does not do
//!
//! No sockets. [`RemoteSessionTransport`] drives the metadata state machine,
//! and a network drop is *reported to* it rather than detected by it — the
//! caller owning the connection is the only layer that can know. That is the
//! same boundary the rest of `legion-remote` keeps, and it is what makes a
//! forced-drop test a real test rather than a simulation of one.

use legion_protocol::{
    CausalityId, CorrelationId, EventSequence, RemoteNetworkHealthState,
    RemoteOfflineResumeManifest, RemoteTransportHandshake, RemoteTransportHealthSummary,
    RemoteTransportLifecycleState, RemoteTransportResumeToken, RemoteWorkspaceLifecycleState,
    RemoteWorkspaceSessionId, TimestampMillis,
};
use legion_remote_transport::{
    RemoteTransportConfig, RemoteTransportCoreError, RemoteTransportStateMachine,
};

use crate::{RemoteRuntimeError, RemoteSessionRuntime};

/// Why a session transport refused an operation.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum RemoteSessionTransportError {
    /// The transport layer refused.
    #[error("remote transport refused: {0}")]
    Transport(#[source] RemoteTransportCoreError),
    /// The session layer refused.
    #[error("remote session refused: {0}")]
    Session(#[source] RemoteRuntimeError),
    /// The transport and its session disagree about which session they serve.
    #[error("transport is bound to session {bound} but was used with {actual}")]
    SessionMismatch {
        /// Session the transport was activated for.
        bound: u128,
        /// Session the caller passed.
        actual: u128,
    },
}

impl From<RemoteTransportCoreError> for RemoteSessionTransportError {
    fn from(error: RemoteTransportCoreError) -> Self {
        Self::Transport(error)
    }
}

impl From<RemoteRuntimeError> for RemoteSessionTransportError {
    fn from(error: RemoteRuntimeError) -> Self {
        Self::Session(error)
    }
}

/// A remote session's production transport.
#[derive(Debug)]
pub struct RemoteSessionTransport {
    machine: RemoteTransportStateMachine,
    session_id: RemoteWorkspaceSessionId,
}

impl RemoteSessionTransport {
    /// Activate a transport for one session by completing its handshake.
    ///
    /// The handshake's session must match the runtime's. A transport bound to a
    /// different session than the one it is driven with would report health for
    /// a session nobody is watching, which is worse than refusing.
    pub fn activate(
        runtime: &RemoteSessionRuntime,
        handshake: RemoteTransportHandshake,
        config: RemoteTransportConfig,
    ) -> Result<Self, RemoteSessionTransportError> {
        let session_id = runtime.session_id();
        if handshake.session_id != session_id {
            return Err(RemoteSessionTransportError::SessionMismatch {
                bound: handshake.session_id.0,
                actual: session_id.0,
            });
        }
        let mut machine = RemoteTransportStateMachine::new(config);
        machine.begin_handshake()?;
        machine.accept_handshake(handshake)?;
        Ok(Self {
            machine,
            session_id,
        })
    }

    /// The transport's lifecycle state.
    pub fn state(&self) -> RemoteTransportLifecycleState {
        self.machine.state()
    }

    /// How many drops this transport has recorded.
    pub fn reconnect_attempts(&self) -> u32 {
        self.machine.reconnect_attempts()
    }

    /// Borrow the underlying state machine for frame and checkpoint work.
    pub fn machine_mut(&mut self) -> &mut RemoteTransportStateMachine {
        &mut self.machine
    }

    /// Borrow the underlying state machine.
    pub fn machine(&self) -> &RemoteTransportStateMachine {
        &self.machine
    }

    /// Report that the network dropped, moving session and transport together.
    ///
    /// Both layers move or neither does. A session showing `Active` over a
    /// transport that is reconnecting is the state that makes a reconnect bug
    /// invisible: the UI says connected, the frames go nowhere.
    pub fn report_network_drop(
        &mut self,
        runtime: &mut RemoteSessionRuntime,
        reason: impl Into<String>,
    ) -> Result<RemoteTransportHealthSummary, RemoteSessionTransportError> {
        self.ensure_bound(runtime)?;
        let summary = self.machine.mark_network_drop(reason)?;
        runtime.begin_reconnect();
        Ok(summary)
    }

    /// Resume a dropped session against a resume token and offline manifest.
    ///
    /// The session returns to `Active` only after the transport has accepted
    /// both. If either is refused the session stays reconnecting, which is the
    /// behaviour `complete_reconnect`'s "externally validated" precondition was
    /// always describing and never enforcing.
    pub fn resume(
        &mut self,
        runtime: &mut RemoteSessionRuntime,
        token: RemoteTransportResumeToken,
        manifest: RemoteOfflineResumeManifest,
        now: TimestampMillis,
    ) -> Result<RemoteTransportHealthSummary, RemoteSessionTransportError> {
        self.ensure_bound(runtime)?;
        self.machine.begin_resume(token, now)?;
        let summary = self.machine.complete_resume(manifest)?;
        runtime.complete_reconnect()?;
        Ok(summary)
    }

    /// Build the offline resume manifest for this transport's session.
    pub fn offline_resume_manifest(
        &self,
        runtime: &RemoteSessionRuntime,
        correlation_id: CorrelationId,
        causality_id: CausalityId,
        event_sequence: EventSequence,
    ) -> RemoteOfflineResumeManifest {
        runtime.offline_resume_manifest(correlation_id, causality_id, event_sequence)
    }

    /// Metadata-only health for this transport, reported against the session.
    ///
    /// The health value comes from the session rather than being passed in, so
    /// a caller cannot report `Healthy` for a session the runtime believes is
    /// offline.
    pub fn health(&self, runtime: &RemoteSessionRuntime) -> RemoteTransportHealthSummary {
        self.machine.health_summary(session_health(runtime))
    }

    fn ensure_bound(
        &self,
        runtime: &RemoteSessionRuntime,
    ) -> Result<(), RemoteSessionTransportError> {
        if runtime.session_id() != self.session_id {
            return Err(RemoteSessionTransportError::SessionMismatch {
                bound: self.session_id.0,
                actual: runtime.session_id().0,
            });
        }
        Ok(())
    }
}

/// Health as the session sees it, which is the authority for reporting.
fn session_health(runtime: &RemoteSessionRuntime) -> RemoteNetworkHealthState {
    match runtime.state() {
        RemoteWorkspaceLifecycleState::Offline => RemoteNetworkHealthState::Offline,
        RemoteWorkspaceLifecycleState::Reconnecting => RemoteNetworkHealthState::Disconnected,
        _ => runtime.network_health(),
    }
}
