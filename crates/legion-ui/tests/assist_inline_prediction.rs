use legion_protocol::{
    BufferId, BufferVersion, FileFingerprint, FileId, ProtocolTextRange, SnapshotId,
    TextCoordinate, TimestampMillis, WorkspaceId,
};
use legion_ui::{
    AssistInlinePredictionProjection, AssistInlinePredictionRowProjection,
    AssistInlinePredictionStatusProjection, CommandDispatchIntent, Shell,
};

fn coord(line: u32, character: u32, byte_offset: u64) -> TextCoordinate {
    TextCoordinate {
        line,
        character,
        byte_offset: Some(byte_offset),
        utf16_offset: None,
    }
}

fn range(start: u64, end: u64) -> ProtocolTextRange {
    ProtocolTextRange {
        start: coord(0, start as u32, start),
        end: coord(0, end as u32, end),
    }
}

fn prediction_row() -> AssistInlinePredictionRowProjection {
    AssistInlinePredictionRowProjection {
        prediction_id: "assist:prediction:1".to_string(),
        workspace_id: Some(WorkspaceId(1)),
        buffer_id: Some(BufferId(7)),
        file_id: Some(FileId(11)),
        provider_label: "Local fixture".to_string(),
        status: AssistInlinePredictionStatusProjection::Ready,
        status_label: "ready".to_string(),
        latency_ms: Some(38),
        requested_at: TimestampMillis(100),
        completed_at: Some(TimestampMillis(138)),
        snapshot_id: Some(SnapshotId(5)),
        buffer_version: Some(BufferVersion(9)),
        file_fingerprint: Some(FileFingerprint {
            algorithm: "sha256".to_string(),
            value: "fingerprint-a".to_string(),
        }),
        stale: true,
        stale_reason_label: Some("buffer advanced after prediction".to_string()),
        ghost_text_label: ".await".to_string(),
        replacement_preview_label: Some("value.await".to_string()),
        apply_range: range(4, 4),
        apply_range_label: "0:4..0:4".to_string(),
        diagnostics: vec!["metadata-only display label".to_string()],
    }
}

#[test]
fn shell_carries_assist_inline_prediction_projection_and_routes_commands_without_authority() {
    let mut snapshot = Shell::empty("assist").projection_snapshot();
    snapshot.active_buffer_projection.workspace_id = Some(WorkspaceId(1));
    snapshot.active_buffer_projection.buffer_id = Some(BufferId(7));
    snapshot.active_buffer_projection.file_id = Some(FileId(11));
    snapshot.active_buffer_projection.small_buffer_preview = Some("let value = future".to_string());

    let row = prediction_row();
    let projection = AssistInlinePredictionProjection {
        active_prediction: Some(row.clone()),
        rows: vec![row],
        request_in_flight: true,
        stale_prediction_count: 1,
        generated_at: TimestampMillis(150),
        schema_version: 1,
    };
    snapshot.assist_inline_prediction_projection = projection.clone();

    let mut shell = Shell::new(snapshot);
    assert_eq!(shell.assist_inline_prediction_projection, projection);
    assert_eq!(
        shell
            .projection_snapshot()
            .assist_inline_prediction_projection,
        projection
    );

    let before_commands = shell.projection_snapshot();
    assert_eq!(
        shell
            .handle_command(":assist-predict 4")
            .expect("assist prediction request should parse"),
        Some(CommandDispatchIntent::RequestAssistInlinePrediction {
            buffer_id: BufferId(7),
            position: coord(0, 4, 4),
        })
    );
    assert_eq!(
        shell
            .handle_command(":tab")
            .expect("tab-equivalent accept command should parse"),
        Some(CommandDispatchIntent::AcceptAssistInlinePrediction {
            buffer_id: BufferId(7),
            prediction_id: Some("assist:prediction:1".to_string()),
        })
    );
    assert_eq!(
        shell
            .handle_command(":assist-dismiss")
            .expect("assist dismiss command should parse"),
        Some(CommandDispatchIntent::DismissAssistInlinePrediction {
            buffer_id: BufferId(7),
            prediction_id: Some("assist:prediction:1".to_string()),
        })
    );
    assert_eq!(
        shell
            .handle_command(":assist-cancel")
            .expect("assist cancel command should parse"),
        Some(CommandDispatchIntent::CancelAssistInlinePrediction {
            buffer_id: BufferId(7),
            prediction_id: Some("assist:prediction:1".to_string()),
        })
    );
    assert_eq!(
        shell
            .handle_command(":ai-explain summarize")
            .expect("existing explain command should still parse"),
        Some(CommandDispatchIntent::StartAiExplain {
            instruction_label: "summarize".to_string(),
        })
    );
    assert_eq!(
        shell
            .handle_command(":ai-propose add guard")
            .expect("existing propose command should still parse"),
        Some(CommandDispatchIntent::StartAiProposal {
            instruction_label: "add guard".to_string(),
        })
    );
    assert_eq!(shell.projection_snapshot(), before_commands);
}
