//! Reconnect and offline behaviour under a forced network drop (P9.F3.T2).
//!
//! The task's stop condition is "stop if reconnect logic is untested under a
//! forced network drop". Before this file that condition could not be met, and
//! not because the test was missing: `RemoteTransportLifecycleState::Reconnecting`
//! was a declared state that **no transition ever entered**. Eight assignments
//! to `state` existed in the transport state machine and none of them
//! represented losing the connection, so there was no reconnect logic to test —
//! only a resume path that assumed you had never been disconnected.
//!
//! These tests drive the real transition. "Forced" here means the drop is
//! injected at the layer that owns the connection and the state machine must
//! cope, rather than the state machine being asked politely to pretend.

use legion_protocol::{
    CapabilityDecision, CapabilityDecisionId, CapabilityId, CausalityId, CorrelationId,
    EventSequence, PrincipalId, RedactionHint, RemoteAgentDescriptor, RemoteAuthorityDescriptor,
    RemoteNetworkHealthState, RemoteOperationId, RemoteOperationLogCheckpoint,
    RemoteOperationLogCheckpointId, RemoteTransportEndpointDescriptor,
    RemoteTransportFrameMetadata, RemoteTransportHandshake, RemoteTransportLifecycleState,
    RemoteTransportPeerIdentity, RemoteTransportSchemaCompatibility, RemoteWorkspaceLifecycleState,
    RemoteWorkspaceSessionDescriptor, RemoteWorkspaceSessionId, TimestampMillis,
    WorkspaceGeneration, WorkspaceId, WorkspaceTrustState,
};
use legion_remote::{
    RemoteRuntimeConfig, RemoteSessionRuntime,
    transport::{RemoteSessionTransport, RemoteSessionTransportError},
};
use legion_remote_transport::{RemoteTransportConfig, RemoteTransportCoreError};
use uuid::Uuid;

const SESSION: RemoteWorkspaceSessionId = RemoteWorkspaceSessionId(7001);

fn uuid_from_sequence(sequence: u128) -> Uuid {
    Uuid::from_u128(sequence)
}

fn session_descriptor() -> RemoteWorkspaceSessionDescriptor {
    RemoteWorkspaceSessionDescriptor {
        session_id: SESSION,
        authority: RemoteAuthorityDescriptor {
            authority_id: legion_protocol::RemoteAuthorityId(7101),
            authority_label: "edge-authority:hash".to_string(),
            workspace_id: WorkspaceId(11),
            trust_state: WorkspaceTrustState::Trusted,
            principal_id: PrincipalId("principal-remote".to_string()),
            redaction_hints: vec![RedactionHint::MetadataOnly],
            schema_version: 1,
        },
        agent: RemoteAgentDescriptor {
            agent_id: legion_protocol::RemoteAgentId(7201),
            authority_id: legion_protocol::RemoteAuthorityId(7101),
            agent_version: "test-agent/1".to_string(),
            runtime_enabled: true,
            schema_version: 1,
        },
        state: RemoteWorkspaceLifecycleState::Active,
        granted_capabilities: vec![],
        created_at: TimestampMillis(1700),
        last_heartbeat_at: Some(TimestampMillis(1800)),
        schema_version: 1,
    }
}

fn runtime() -> RemoteSessionRuntime {
    RemoteSessionRuntime::new(
        session_descriptor(),
        WorkspaceGeneration(1),
        RemoteRuntimeConfig::enabled(),
    )
    .expect("remote runtime should start")
}

fn handshake(session_id: RemoteWorkspaceSessionId) -> RemoteTransportHandshake {
    RemoteTransportHandshake {
        session_id,
        endpoint: RemoteTransportEndpointDescriptor {
            endpoint_id: "loopback".to_string(),
            scheme: "https".to_string(),
            host: "localhost".to_string(),
            port: Some(9443),
            loopback_only: true,
            schema_version: 1,
        },
        peer_identity: RemoteTransportPeerIdentity {
            authority_id: legion_protocol::RemoteAuthorityId(7101),
            agent_id: legion_protocol::RemoteAgentId(7201),
            principal_id: PrincipalId("principal-remote".to_string()),
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
        causality_id: CausalityId(uuid_from_sequence(5)),
        event_sequence: EventSequence(6),
        schema_version: 1,
    }
}

fn frame(sequence: u64, operation: u128) -> RemoteTransportFrameMetadata {
    RemoteTransportFrameMetadata {
        session_id: SESSION,
        operation_id: RemoteOperationId(operation),
        frame_sequence: EventSequence(sequence),
        envelope_byte_len: 128,
        max_frame_bytes: 1024,
        compressed: false,
        schema_version: 1,
    }
}

fn checkpoint(sequence: u64, operation: u128) -> RemoteOperationLogCheckpoint {
    RemoteOperationLogCheckpoint {
        checkpoint_id: RemoteOperationLogCheckpointId(9001),
        session_id: SESSION,
        last_operation_id: RemoteOperationId(operation),
        version_vector: legion_protocol::CollaborationVersionVector { entries: vec![] },
        network_health: RemoteNetworkHealthState::Healthy,
        event_sequence: EventSequence(sequence),
        schema_version: 1,
    }
}

fn agent_package() -> legion_protocol::RemoteAgentPackageDescriptor {
    legion_protocol::RemoteAgentPackageDescriptor {
        agent_id: legion_protocol::RemoteAgentId(7201),
        authority_id: legion_protocol::RemoteAuthorityId(7101),
        package_id: "agent-package".to_string(),
        version: "1.0.0".to_string(),
        package_digest: legion_protocol::FileFingerprint {
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

/// Bring a transport up, pass one frame, and checkpoint it.
fn established() -> (RemoteSessionRuntime, RemoteSessionTransport) {
    let runtime = runtime();
    let mut transport = RemoteSessionTransport::activate(
        &runtime,
        handshake(SESSION),
        RemoteTransportConfig::enabled(),
    )
    .expect("transport activates for its session");
    transport
        .machine_mut()
        .activate_agent_package(agent_package())
        .expect("activate the agent package the config requires");
    transport
        .machine_mut()
        .try_accept_frame(frame(7, 7))
        .expect("established transport accepts a frame");
    transport
        .machine_mut()
        .checkpoint(checkpoint(7, 7))
        .expect("checkpoint the accepted frame");
    (runtime, transport)
}

#[test]
fn a_forced_drop_moves_both_the_session_and_the_transport() {
    let (mut runtime, mut transport) = established();
    assert_eq!(transport.state(), RemoteTransportLifecycleState::Active);
    assert_eq!(runtime.state(), RemoteWorkspaceLifecycleState::Active);

    let summary = transport
        .report_network_drop(&mut runtime, "forced drop: peer closed the socket")
        .expect("an established transport can be dropped");

    // Both layers, or the failure is invisible: a session reporting Active over
    // a reconnecting transport tells the user they are connected while every
    // frame is refused.
    assert_eq!(
        transport.state(),
        RemoteTransportLifecycleState::Reconnecting
    );
    assert_eq!(runtime.state(), RemoteWorkspaceLifecycleState::Reconnecting);
    assert_eq!(summary.health, RemoteNetworkHealthState::Offline);
    assert_eq!(
        transport.health(&runtime).health,
        RemoteNetworkHealthState::Disconnected,
        "health is read from the session, so it cannot claim healthy while reconnecting"
    );
}

#[test]
fn a_dropped_transport_refuses_frames_until_it_resumes() {
    let (mut runtime, mut transport) = established();
    transport
        .report_network_drop(&mut runtime, "forced drop")
        .expect("drop");

    let error = transport
        .machine_mut()
        .try_accept_frame(frame(8, 8))
        .expect_err("a disconnected transport must not accept frames");
    assert!(
        matches!(error, RemoteTransportCoreError::InvalidState { .. }),
        "expected a state refusal while offline, got {error:?}"
    );
}

#[test]
fn a_drop_clears_in_flight_frames_but_keeps_the_replay_window() {
    let (mut runtime, mut transport) = established();
    // The frame from `established` was accepted and never acked, so it is on
    // the wire when the connection dies.
    assert_eq!(
        transport.health(&runtime).queued_frame_count,
        1,
        "an unacked frame is in flight before the drop"
    );

    transport
        .report_network_drop(&mut runtime, "forced drop")
        .expect("drop");

    // In-flight frames go: nobody acknowledged them and no peer will, so
    // holding them reports queue depth that will never drain.
    assert_eq!(
        transport.health(&runtime).queued_frame_count,
        0,
        "frames in flight when the wire died are not still queued"
    );

    // The replay window does not go. It is what resume replays against, and
    // dropping it would turn every reconnect into a full resynchronisation.
    let window = transport
        .machine()
        .replay_window()
        .expect("the replay window survives a drop");
    assert_eq!(window.accepted_operation_count, 1);
    assert_eq!(window.highest_accepted_sequence, EventSequence(7));
    assert_eq!(
        window.checkpoint_id,
        Some(RemoteOperationLogCheckpointId(9001)),
        "the checkpoint survives, or the resume manifest could never match it"
    );
}

#[test]
fn resume_after_a_drop_restores_both_layers() {
    let (mut runtime, mut transport) = established();
    let token = transport
        .machine_mut()
        .issue_resume_token("digest", TimestampMillis(10_000))
        .expect("issue a resume token while still connected");
    transport
        .report_network_drop(&mut runtime, "forced drop")
        .expect("drop");

    // The manifest has to carry the checkpoint the transport last recorded;
    // this is the "cache and version precondition" the session layer used to
    // claim was validated externally.
    let mut manifest = runtime.offline_resume_manifest(
        CorrelationId(5),
        CausalityId(uuid_from_sequence(5)),
        EventSequence(8),
    );
    manifest.checkpoints = vec![RemoteOperationLogCheckpointId(9001)];

    let summary = transport
        .resume(&mut runtime, token, manifest, TimestampMillis(1_000))
        .expect("resume with a matching token and manifest");

    assert_eq!(transport.state(), RemoteTransportLifecycleState::Active);
    assert_eq!(runtime.state(), RemoteWorkspaceLifecycleState::Active);
    assert_eq!(summary.health, RemoteNetworkHealthState::Healthy);
    transport
        .machine_mut()
        .try_accept_frame(frame(8, 8))
        .expect("a resumed transport accepts frames again");
}

#[test]
fn a_resume_manifest_missing_the_checkpoint_leaves_the_session_reconnecting() {
    let (mut runtime, mut transport) = established();
    let token = transport
        .machine_mut()
        .issue_resume_token("digest", TimestampMillis(10_000))
        .expect("token");
    transport
        .report_network_drop(&mut runtime, "forced drop")
        .expect("drop");

    let mut manifest = runtime.offline_resume_manifest(
        CorrelationId(5),
        CausalityId(uuid_from_sequence(5)),
        EventSequence(8),
    );
    manifest.checkpoints = vec![RemoteOperationLogCheckpointId(4242)];

    let error = transport
        .resume(&mut runtime, token, manifest, TimestampMillis(1_000))
        .expect_err("a manifest without the checkpoint must be refused");
    assert!(
        matches!(
            error,
            RemoteSessionTransportError::Transport(RemoteTransportCoreError::ResumeRejected { .. })
        ),
        "expected a resume refusal, got {error:?}"
    );

    // The session must NOT have advanced. A refused resume that still moved the
    // session to Active would be worse than no validation at all, because the
    // error would be reported and ignored.
    assert_eq!(runtime.state(), RemoteWorkspaceLifecycleState::Reconnecting);
    assert_eq!(
        transport.state(),
        RemoteTransportLifecycleState::Resuming,
        "the transport stays mid-resume rather than silently reverting"
    );
}

#[test]
fn resume_without_a_drop_is_refused() {
    let (mut runtime, mut transport) = established();
    let token = transport
        .machine_mut()
        .issue_resume_token("digest", TimestampMillis(10_000))
        .expect("token");
    let manifest = runtime.offline_resume_manifest(
        CorrelationId(5),
        CausalityId(uuid_from_sequence(5)),
        EventSequence(8),
    );

    // Resume is recovery. An Active transport has nothing to recover from, and
    // allowing it would be a second path to Active with weaker checks than the
    // handshake.
    let error = transport
        .resume(&mut runtime, token, manifest, TimestampMillis(1_000))
        .expect_err("resume without a drop must be refused");
    assert!(
        matches!(
            error,
            RemoteSessionTransportError::Transport(RemoteTransportCoreError::InvalidState { .. })
        ),
        "expected a state refusal, got {error:?}"
    );
    assert_eq!(runtime.state(), RemoteWorkspaceLifecycleState::Active);
}

#[test]
fn reconnect_attempts_counts_every_drop_rather_than_the_current_state() {
    let (mut runtime, mut transport) = established();
    assert_eq!(transport.reconnect_attempts(), 0);

    for round in 1..=3 {
        let token = transport
            .machine_mut()
            .issue_resume_token(format!("digest-{round}"), TimestampMillis(10_000))
            .expect("token");
        transport
            .report_network_drop(&mut runtime, format!("forced drop {round}"))
            .expect("drop");
        assert_eq!(
            transport.reconnect_attempts(),
            round,
            "each drop is one attempt"
        );

        let mut manifest = runtime.offline_resume_manifest(
            CorrelationId(5),
            CausalityId(uuid_from_sequence(5)),
            EventSequence(8),
        );
        manifest.checkpoints = vec![RemoteOperationLogCheckpointId(9001)];
        transport
            .resume(&mut runtime, token, manifest, TimestampMillis(1_000))
            .expect("resume");
    }

    // The count survives recovery. It was previously computed as
    // `matches!(state, Reconnecting | Resuming) as u32`, which reported 0 here
    // -- a session that had dropped three times looked pristine the moment it
    // came back.
    assert_eq!(transport.reconnect_attempts(), 3);
    assert_eq!(
        transport.health(&runtime).reconnect_attempts,
        3,
        "the health summary reports the real count"
    );
}

#[test]
fn a_transport_refuses_a_session_it_was_not_activated_for() {
    let (_runtime, mut transport) = established();
    let mut other = RemoteSessionRuntime::new(
        RemoteWorkspaceSessionDescriptor {
            session_id: RemoteWorkspaceSessionId(7002),
            ..session_descriptor()
        },
        WorkspaceGeneration(1),
        RemoteRuntimeConfig::enabled(),
    )
    .expect("second runtime");

    let error = transport
        .report_network_drop(&mut other, "forced drop")
        .expect_err("a transport must refuse a session it is not bound to");
    assert!(
        matches!(
            error,
            RemoteSessionTransportError::SessionMismatch {
                bound: 7001,
                actual: 7002
            }
        ),
        "expected a session mismatch, got {error:?}"
    );
    assert_eq!(other.state(), RemoteWorkspaceLifecycleState::Active);
}

#[test]
fn activation_refuses_a_handshake_for_a_different_session() {
    let runtime = runtime();
    let error = RemoteSessionTransport::activate(
        &runtime,
        handshake(RemoteWorkspaceSessionId(9999)),
        RemoteTransportConfig::enabled(),
    )
    .expect_err("a handshake for another session must not activate");
    assert!(
        matches!(
            error,
            RemoteSessionTransportError::SessionMismatch {
                bound: 9999,
                actual: 7001
            }
        ),
        "expected a session mismatch, got {error:?}"
    );
}
