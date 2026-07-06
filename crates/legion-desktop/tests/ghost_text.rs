#![cfg(feature = "ai")]
//! Tests for ghost text overlay view model (PKT-RAIL T1).

use legion_desktop::bridge::{DesktopAction, DesktopBridgeOutput, DesktopCommandBridge};
use legion_desktop::view::{GhostTextState, ghost_text_from_prediction};
use legion_protocol::{
    AssistedAiOperationClass, AssistedAiProviderClass, AssistedAiProviderInvocationState,
    BufferId, BufferVersion, FileContentVersion, FileFingerprint, InlinePredictionFingerprintMetadata,
    InlinePredictionFreshness, InlinePredictionFreshnessState, InlinePredictionGhostText,
    InlinePredictionLatencyMetadata, InlinePredictionProviderMetadata,
    InlinePredictionRequestId, InlinePredictionResult, InlinePredictionResultId,
    InlinePredictionResultState, InlinePredictionRetention, ProtocolTextRange, RedactionHint,
    SnapshotId, TextCoordinate, TimestampMillis, WorkspaceGeneration,
};
use legion_ui::{ActiveBufferProjection, ActiveBufferProjectionState, CommandDispatchIntent, Shell};

fn coord(line: u32, character: u32) -> TextCoordinate {
    TextCoordinate {
        line,
        character,
        byte_offset: Some(0),
        utf16_offset: Some(0),
    }
}

fn sample_fingerprint() -> InlinePredictionFingerprintMetadata {
    InlinePredictionFingerprintMetadata {
        snapshot_id: SnapshotId(1),
        buffer_version: BufferVersion(1),
        file_content_version: Some(FileContentVersion(1)),
        workspace_generation: WorkspaceGeneration(1),
        content_fingerprint: Some(FileFingerprint {
            algorithm: "sha256".to_string(),
            value: "test".to_string(),
        }),
        context_fingerprint: FileFingerprint {
            algorithm: "sha256".to_string(),
            value: "ctx".to_string(),
        },
        schema_version: 1,
    }
}

fn sample_provider_metadata() -> InlinePredictionProviderMetadata {
    InlinePredictionProviderMetadata {
        provider_id: "deterministic-inline".to_string(),
        model_label: "test-model".to_string(),
        provider_class: AssistedAiProviderClass::Local,
        operation_class: AssistedAiOperationClass::InlinePrediction,
        invocation_state: AssistedAiProviderInvocationState::Completed,
        latency: InlinePredictionLatencyMetadata {
            queued_ms: 0,
            inference_ms: 1,
            total_ms: 1,
            timed_out: false,
        },
        health_labels: vec!["deterministic".to_string()],
        cost_labels: vec!["local".to_string()],
        redaction_hints: vec![RedactionHint::MetadataOnly],
        schema_version: 1,
    }
}

fn fresh_prediction_with_ghost_text(text: &str) -> InlinePredictionResult {
    InlinePredictionResult {
        result_id: InlinePredictionResultId("result:1".to_string()),
        request_id: InlinePredictionRequestId("req:1".to_string()),
        state: InlinePredictionResultState::Available,
        retention: InlinePredictionRetention::EphemeralDisplay,
        insert_range: ProtocolTextRange {
            start: coord(3, 10),
            end: coord(3, 10),
        },
        ghost_text: Some(InlinePredictionGhostText {
            text: text.to_string(),
            byte_len: text.len() as u32,
            line_count: 1,
            text_fingerprint: FileFingerprint {
                algorithm: "deterministic-inline-v1".to_string(),
                value: "rust:3:10".to_string(),
            },
        }),
        fingerprint: sample_fingerprint(),
        freshness: InlinePredictionFreshness {
            state: InlinePredictionFreshnessState::Fresh,
            stale_reasons: Vec::new(),
            schema_version: 1,
        },
        provider: sample_provider_metadata(),
        refusal: None,
        generated_at: TimestampMillis(1000),
        expires_at: None,
        redaction_hints: vec![RedactionHint::MetadataOnly],
        schema_version: 1,
    }
}

fn stale_prediction() -> InlinePredictionResult {
    let mut result = fresh_prediction_with_ghost_text("fn stale() {}");
    result.freshness = InlinePredictionFreshness {
        state: InlinePredictionFreshnessState::Stale,
        stale_reasons: vec![legion_protocol::InlinePredictionStaleReason::BufferVersionChanged],
        schema_version: 1,
    };
    result
}

fn snapshot_with_active_buffer() -> legion_ui::ShellProjectionSnapshot {
    let mut snapshot = Shell::empty("GhostText").projection_snapshot();
    snapshot.active_buffer_projection = ActiveBufferProjection {
        state: ActiveBufferProjectionState::Full,
        buffer_id: Some(BufferId(42)),
        ..ActiveBufferProjection::empty()
    };
    snapshot
}

#[test]
fn ghost_text_from_valid_prediction_creates_overlay() {
    let result = fresh_prediction_with_ghost_text("fn answer() -> u32 { 42 }");
    let overlay = ghost_text_from_prediction(&result, "test-provider")
        .expect("fresh prediction with ghost text should create an overlay");

    assert_eq!(overlay.text, "fn answer() -> u32 { 42 }");
    assert_eq!(overlay.provider_id, "test-provider");
    assert_eq!(overlay.request_id, InlinePredictionRequestId("req:1".to_string()));
    assert_eq!(overlay.insert_position.line, 3);
    assert_eq!(overlay.insert_position.character, 10);
    assert_eq!(overlay.state, GhostTextState::Displaying);
    assert!(!overlay.stale);
}

#[test]
fn ghost_text_from_stale_prediction_returns_none() {
    let result = stale_prediction();
    let overlay = ghost_text_from_prediction(&result, "test-provider");
    assert!(
        overlay.is_none(),
        "stale prediction must return None to prevent display of outdated ghost text"
    );
}

#[test]
fn ghost_text_from_missing_text_returns_none() {
    let mut result = fresh_prediction_with_ghost_text("irrelevant");
    result.ghost_text = None;
    let overlay = ghost_text_from_prediction(&result, "test-provider");
    assert!(
        overlay.is_none(),
        "prediction without ghost text body must return None"
    );
}

#[test]
fn accept_ghost_text_dispatches_proposal() {
    let bridge = DesktopCommandBridge::new();
    let snapshot = snapshot_with_active_buffer();
    let request_id = InlinePredictionRequestId("req:accept:1".to_string());

    let result = bridge.translate(
        DesktopAction::AcceptGhostText {
            request_id: request_id.clone(),
        },
        &snapshot,
    );

    // Must dispatch through the inline prediction acceptance path — NOT a direct
    // Insert/Replace/Delete mutation.
    match result {
        DesktopBridgeOutput::Intent(CommandDispatchIntent::AcceptAssistInlinePrediction {
            prediction_id,
            ..
        }) => {
            assert_eq!(
                prediction_id,
                Some(request_id.0),
                "prediction_id must be the request_id forwarded through the intent"
            );
        }
        other => panic!(
            "AcceptGhostText must dispatch AcceptAssistInlinePrediction intent, got: {other:?}"
        ),
    }
}

#[test]
fn dismiss_ghost_text_clears_overlay() {
    let result = fresh_prediction_with_ghost_text("fn hello() {}");
    let overlay = ghost_text_from_prediction(&result, "test-provider")
        .expect("fresh prediction creates overlay");

    assert_eq!(overlay.state, GhostTextState::Displaying);

    let dismissed = overlay.dismiss();
    assert_eq!(
        dismissed.state,
        GhostTextState::Dismissed,
        "dismiss() must transition overlay state to Dismissed"
    );
    // Other fields are preserved
    assert_eq!(dismissed.text, "fn hello() {}");
    assert_eq!(dismissed.provider_id, "test-provider");
}
