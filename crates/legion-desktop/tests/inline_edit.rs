//! Integration tests for the inline edit loop (PKT-INLINE T1 + T2).
//!
//! Covers:
//! - T1: Streaming diff overlay anchored to current text (5 tests)
//! - T2: Per-hunk accept/reject through proposal pipeline (5 tests)

use legion_desktop::bridge::{DesktopAction, DesktopBridgeOutput, DesktopCommandBridge};
use legion_desktop::view::{
    InlineEditError, InlineEditOverlayState, accumulate_inline_edit_chunks,
    build_inline_edit_audit_record, check_inline_edit_anchor_freshness,
    inline_edit_from_instruction, inline_edit_to_workspace_proposal,
    set_inline_edit_hunk_disposition,
};
use legion_protocol::{
    BufferId, BufferVersion, DelegatedTaskProposalHunkDisposition, FileFingerprint,
    InlineEditInstruction, ProposalLifecycleState, ProposalPayload, ProposalPayloadKind,
    ProtocolTextRange, SnapshotId, TextCoordinate,
};

// ─── Helpers ─────────────────────────────────────────────────────────────────

fn coord(line: u32, character: u32) -> TextCoordinate {
    TextCoordinate {
        line,
        character,
        byte_offset: Some((line * 80 + character) as u64),
        utf16_offset: Some((line * 80 + character) as u64),
    }
}

fn sample_range() -> ProtocolTextRange {
    ProtocolTextRange {
        start: coord(0, 0),
        end: coord(0, 5),
    }
}

fn sample_instruction() -> InlineEditInstruction {
    InlineEditInstruction {
        instruction_text: "rename this variable to `count`".to_string(),
        anchor_range: sample_range(),
        anchor_snapshot_id: SnapshotId(42),
        anchor_buffer_version: BufferVersion(7),
        anchor_content_fingerprint: Some(FileFingerprint {
            algorithm: "sha256".to_string(),
            value: "abc123".to_string(),
        }),
    }
}

/// Builds a complete single-hunk chunk string in the expected format.
fn complete_chunk(hunk_id: &str, original: &str, replacement: &str) -> String {
    format!("{hunk_id}\n{original}\n---SEP---\n{replacement}\n---END---\n")
}

/// Builds an incomplete (still-streaming) single-hunk chunk — no `---END---`.
fn incomplete_chunk(hunk_id: &str, original: &str, replacement: &str) -> String {
    format!("{hunk_id}\n{original}\n---SEP---\n{replacement}")
}

// ─── T1 Tests ────────────────────────────────────────────────────────────────

#[test]
fn inline_edit_from_instruction_creates_overlay() {
    let instruction = sample_instruction();
    let overlay = inline_edit_from_instruction(instruction.clone(), "inst-1".to_string());

    assert_eq!(overlay.instruction_id, "inst-1");
    assert_eq!(overlay.instruction, instruction);
    assert!(
        overlay.diff_hunks.is_empty(),
        "new overlay must have no hunks"
    );
    assert_eq!(
        overlay.state,
        InlineEditOverlayState::Streaming,
        "new overlay must start in Streaming state"
    );
    assert!(!overlay.stale, "new overlay must not be stale");
}

#[test]
fn stale_anchor_prevents_application() {
    let instruction = sample_instruction();
    // Simulate the buffer version changing after the instruction was issued.
    let current_snapshot_id = SnapshotId(42); // same
    let changed_buffer_version = BufferVersion(8); // one ahead

    let is_fresh = check_inline_edit_anchor_freshness(
        &instruction,
        current_snapshot_id,
        changed_buffer_version,
    );

    assert!(
        !is_fresh,
        "stale-anchor (buffer_version changed) must return false — cannot apply"
    );
}

#[test]
fn streaming_chunks_accumulate_into_hunks() {
    let instruction = sample_instruction();

    // Incomplete chunk: no ---END--- yet.
    let partial = vec![incomplete_chunk("h1", "hello", "world")];
    let hunks = accumulate_inline_edit_chunks(&partial, &instruction);

    assert_eq!(hunks.len(), 1, "one in-flight hunk expected");
    assert!(
        !hunks[0].complete,
        "partial chunk must produce incomplete hunk"
    );
    assert_eq!(hunks[0].hunk_id, "h1");
    assert_eq!(hunks[0].original_text, "hello");
    assert_eq!(hunks[0].replacement_text, "world");

    // Complete chunk: has ---END---
    let complete = vec![complete_chunk("h1", "hello", "world")];
    let hunks = accumulate_inline_edit_chunks(&complete, &instruction);

    assert_eq!(hunks.len(), 1, "one hunk expected");
    assert!(
        hunks[0].complete,
        "chunk with ---END--- must produce complete hunk"
    );
}

#[test]
fn incomplete_hunk_not_eligible_for_accept() {
    let instruction = sample_instruction();
    let chunks = vec![incomplete_chunk("h1", "foo", "bar")];
    let mut overlay = inline_edit_from_instruction(instruction, "inst-2".to_string());

    // Simulate receiving the partial chunks.
    overlay.diff_hunks = accumulate_inline_edit_chunks(&chunks, &overlay.instruction.clone());
    overlay.state = InlineEditOverlayState::Complete;

    let result = set_inline_edit_hunk_disposition(
        &mut overlay,
        "h1",
        DelegatedTaskProposalHunkDisposition::Accepted,
    );

    assert!(
        matches!(result, Err(InlineEditError::HunkNotComplete { .. })),
        "streaming hunk with complete=false must reject disposition change"
    );
}

#[test]
fn fresh_anchor_allows_application() {
    let instruction = sample_instruction();
    // Both snapshot and buffer version are identical to the instruction anchor.
    let current_snapshot_id = SnapshotId(42);
    let current_buffer_version = BufferVersion(7);

    let is_fresh = check_inline_edit_anchor_freshness(
        &instruction,
        current_snapshot_id,
        current_buffer_version,
    );

    assert!(
        is_fresh,
        "unchanged snapshot/version must report fresh — can apply"
    );
}

// ─── T2 Tests ────────────────────────────────────────────────────────────────

#[test]
fn accept_hunk_sets_disposition() {
    let instruction = sample_instruction();
    let mut overlay = inline_edit_from_instruction(instruction.clone(), "inst-3".to_string());
    let chunks = vec![complete_chunk("h2", "old_value", "new_value")];
    overlay.diff_hunks = accumulate_inline_edit_chunks(&chunks, &instruction);
    overlay.state = InlineEditOverlayState::Complete;

    set_inline_edit_hunk_disposition(
        &mut overlay,
        "h2",
        DelegatedTaskProposalHunkDisposition::Accepted,
    )
    .expect("accept on complete hunk must succeed");

    assert_eq!(
        overlay.hunk_dispositions.get("h2"),
        Some(&DelegatedTaskProposalHunkDisposition::Accepted),
        "hunk disposition must be Accepted after accept call"
    );
}

#[test]
fn reject_hunk_sets_disposition() {
    let instruction = sample_instruction();
    let mut overlay = inline_edit_from_instruction(instruction.clone(), "inst-4".to_string());
    let chunks = vec![complete_chunk("h3", "before", "after")];
    overlay.diff_hunks = accumulate_inline_edit_chunks(&chunks, &instruction);
    overlay.state = InlineEditOverlayState::Complete;

    set_inline_edit_hunk_disposition(
        &mut overlay,
        "h3",
        DelegatedTaskProposalHunkDisposition::Rejected,
    )
    .expect("reject on complete hunk must succeed");

    assert_eq!(
        overlay.hunk_dispositions.get("h3"),
        Some(&DelegatedTaskProposalHunkDisposition::Rejected),
        "hunk disposition must be Rejected after reject call"
    );
}

#[test]
fn streaming_hunk_rejects_disposition_change() {
    let instruction = sample_instruction();
    let mut overlay = inline_edit_from_instruction(instruction.clone(), "inst-5".to_string());
    let chunks = vec![incomplete_chunk("h4", "original", "replacement")];
    overlay.diff_hunks = accumulate_inline_edit_chunks(&chunks, &instruction);
    overlay.state = InlineEditOverlayState::Complete;

    let result = set_inline_edit_hunk_disposition(
        &mut overlay,
        "h4",
        DelegatedTaskProposalHunkDisposition::Accepted,
    );

    assert!(
        matches!(result, Err(InlineEditError::HunkNotComplete { ref hunk_id }) if hunk_id == "h4"),
        "set disposition on streaming hunk must return HunkNotComplete error"
    );
}

#[test]
fn inline_edit_to_proposal_includes_only_accepted_hunks() {
    let instruction = sample_instruction();
    let mut overlay = inline_edit_from_instruction(instruction.clone(), "inst-6".to_string());

    // Two complete hunks: one accepted, one rejected.
    let two_hunks = format!(
        "{}{}",
        complete_chunk("h5", "alpha", "ALPHA"),
        complete_chunk("h6", "beta", "BETA"),
    );
    overlay.diff_hunks = accumulate_inline_edit_chunks(&[two_hunks], &instruction);
    overlay.state = InlineEditOverlayState::Complete;

    set_inline_edit_hunk_disposition(
        &mut overlay,
        "h5",
        DelegatedTaskProposalHunkDisposition::Accepted,
    )
    .unwrap();
    set_inline_edit_hunk_disposition(
        &mut overlay,
        "h6",
        DelegatedTaskProposalHunkDisposition::Rejected,
    )
    .unwrap();

    let proposal =
        inline_edit_to_workspace_proposal(&overlay, BufferId(1)).expect("proposal must be Some");

    // The proposal must contain only the accepted hunk's edit (h5 → ALPHA).
    match &proposal.payload {
        ProposalPayload::TextEdit(p) => {
            assert_eq!(
                p.edits.edits.len(),
                1,
                "proposal must contain exactly 1 edit for 1 accepted hunk"
            );
            assert_eq!(
                p.edits.edits[0].replacement, "ALPHA",
                "edit replacement must match the accepted hunk's replacement_text"
            );
        }
        other => panic!("expected TextEdit payload, got: {other:?}"),
    }
}

#[test]
fn inline_edit_apply_produces_audit_record() {
    let proposal_id = legion_protocol::ProposalId(9999);
    let audit_record = build_inline_edit_audit_record(proposal_id, 2);

    assert_eq!(
        audit_record.payload_summary.kind,
        ProposalPayloadKind::TextEdit,
        "audit record operation class must be TextEdit for inline edits"
    );
    assert_eq!(
        audit_record.lifecycle_state,
        ProposalLifecycleState::Applied,
        "audit record lifecycle state must be Applied"
    );
    assert_eq!(
        audit_record.proposal_id, proposal_id,
        "audit record proposal_id must match the supplied proposal_id"
    );
}

// ─── Bridge: inline edit actions translate to Noop ───────────────────────────

#[test]
fn inline_edit_bridge_actions_are_noop_stubs() {
    let bridge = DesktopCommandBridge::new();
    let snapshot = legion_ui::Shell::empty("InlineEditBridge").projection_snapshot();

    let actions = vec![
        DesktopAction::AcceptInlineEditHunk {
            instruction_id: "i1".to_string(),
            hunk_id: "h1".to_string(),
        },
        DesktopAction::RejectInlineEditHunk {
            instruction_id: "i1".to_string(),
            hunk_id: "h1".to_string(),
        },
        DesktopAction::ApplyInlineEdit {
            instruction_id: "i1".to_string(),
        },
        DesktopAction::DismissInlineEdit {
            instruction_id: "i1".to_string(),
        },
    ];

    for action in actions {
        let output = bridge.translate(action, &snapshot);
        assert!(
            matches!(output, DesktopBridgeOutput::Noop),
            "inline edit bridge actions must translate to Noop (intercepted before bridge)"
        );
    }
}
