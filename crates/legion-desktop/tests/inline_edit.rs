//! Integration tests for the inline edit loop (PKT-INLINE T1 + T2 + T3).
//!
//! Covers:
//! - T1: Streaming diff overlay anchored to current text (5 tests)
//! - T2: Per-hunk accept/reject through proposal pipeline (5 tests)
//! - T3: Undo integration with editor history + checkpoint ledger (4 tests)

use legion_desktop::bridge::{DesktopAction, DesktopBridgeOutput, DesktopCommandBridge};
use legion_desktop::view::{
    InlineEditError, InlineEditOverlayState, accumulate_inline_edit_chunks,
    apply_inline_edit_with_undo_group, build_inline_edit_audit_record,
    check_inline_edit_anchor_freshness, inline_edit_from_instruction,
    inline_edit_to_workspace_proposal, set_inline_edit_hunk_disposition,
};
use legion_protocol::{
    BufferId, BufferVersion, CausalityId, CorrelationId, DelegatedTaskProposalHunkDisposition,
    FileFingerprint, InlineEditInstruction, ProposalId, ProposalLifecycleState, ProposalPayload,
    ProposalPayloadKind, ProtocolTextRange, SnapshotId, TextCoordinate,
};
use legion_storage::checkpoint::CheckpointStore;

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

fn test_correlation_id() -> CorrelationId {
    CorrelationId(42)
}

fn test_causality_id() -> CausalityId {
    CausalityId(uuid::Uuid::now_v7())
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

    let proposal = inline_edit_to_workspace_proposal(&overlay, BufferId(1))
        .expect("no pending hunks — h5 and h6 both have explicit dispositions")
        .expect("proposal must be Some — at least one accepted hunk");

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
    let corr = test_correlation_id();
    let caus = test_causality_id();
    let audit_record = build_inline_edit_audit_record(proposal_id, 2, corr, caus);

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
    assert_ne!(
        audit_record.correlation_id,
        CorrelationId(0),
        "audit record must carry a non-zero CorrelationId"
    );
    assert_ne!(
        audit_record.causality_id,
        CausalityId(uuid::Uuid::nil()),
        "audit record must carry a non-nil CausalityId"
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

// ─── T3 Tests ────────────────────────────────────────────────────────────────

#[test]
fn apply_creates_checkpoint_with_matching_proposal_id() {
    let instruction = sample_instruction();
    let mut overlay = inline_edit_from_instruction(instruction.clone(), "inst-7".to_string());
    let chunks = vec![complete_chunk("h7", "before_text", "after_text")];
    overlay.diff_hunks = accumulate_inline_edit_chunks(&chunks, &instruction);
    overlay.state = InlineEditOverlayState::Complete;
    overlay.hunk_dispositions.insert(
        "h7".to_string(),
        DelegatedTaskProposalHunkDisposition::Accepted,
    );

    let proposal_id = ProposalId(7777);
    let mut store = CheckpointStore::new();
    let result = apply_inline_edit_with_undo_group(
        &overlay,
        BufferId(10),
        proposal_id,
        test_correlation_id(),
        test_causality_id(),
        &mut store,
    )
    .expect("no pending hunks — h7 has an explicit disposition");

    // The function persists the checkpoint internally; verify via load_checkpoint.
    let loaded = store
        .load_checkpoint(&result.checkpoint_id)
        .expect("load_checkpoint must not error")
        .expect("checkpoint must be present in the store");

    assert_eq!(
        loaded.proposal_id, result.proposal_id,
        "checkpoint proposal_id must match InlineEditApplyResult.proposal_id"
    );
    assert_eq!(
        loaded.proposal_id, proposal_id,
        "checkpoint proposal_id must match the supplied proposal_id"
    );
}

#[test]
fn apply_uses_single_undo_group() {
    let instruction = sample_instruction();
    let mut overlay = inline_edit_from_instruction(instruction.clone(), "inst-8".to_string());

    // Two accepted hunks.
    let chunks = vec![format!(
        "{}{}",
        complete_chunk("h8", "a", "A"),
        complete_chunk("h9", "b", "B"),
    )];
    overlay.diff_hunks = accumulate_inline_edit_chunks(&chunks, &instruction);
    overlay.state = InlineEditOverlayState::Complete;
    overlay.hunk_dispositions.insert(
        "h8".to_string(),
        DelegatedTaskProposalHunkDisposition::Accepted,
    );
    overlay.hunk_dispositions.insert(
        "h9".to_string(),
        DelegatedTaskProposalHunkDisposition::Accepted,
    );

    let proposal_id = ProposalId(8888);
    let mut store = CheckpointStore::new();
    let result = apply_inline_edit_with_undo_group(
        &overlay,
        BufferId(11),
        proposal_id,
        test_correlation_id(),
        test_causality_id(),
        &mut store,
    )
    .expect("no pending hunks — h8 and h9 both have explicit dispositions");

    assert_eq!(
        result.applied_hunk_count, 2,
        "two accepted hunks must produce applied_hunk_count == 2"
    );
    // The undo_group_id must be a non-nil UUID.
    assert_ne!(
        result.undo_group_id,
        uuid::Uuid::nil(),
        "undo_group_id must be a non-nil UUID for grouping editor transactions"
    );
    // The undo group, checkpoint, and proposal are correlated through proposal_id
    // (I1: app layer must pass undo_group_id to TransactionSource.undo_group_id).
    assert_eq!(
        result.checkpoint.proposal_id, result.proposal_id,
        "checkpoint.proposal_id must match result.proposal_id for undo-group correlation"
    );
    assert_eq!(
        result.proposal_id, proposal_id,
        "result.proposal_id must match the supplied proposal_id"
    );
}

#[test]
fn checkpoint_targets_cover_all_applied_hunks() {
    let instruction = sample_instruction();
    let mut overlay = inline_edit_from_instruction(instruction.clone(), "inst-9".to_string());

    // Three accepted hunks.
    let chunks = vec![format!(
        "{}{}{}",
        complete_chunk("ha", "x", "X"),
        complete_chunk("hb", "y", "Y"),
        complete_chunk("hc", "z", "Z"),
    )];
    overlay.diff_hunks = accumulate_inline_edit_chunks(&chunks, &instruction);
    overlay.state = InlineEditOverlayState::Complete;
    for id in ["ha", "hb", "hc"] {
        overlay.hunk_dispositions.insert(
            id.to_string(),
            DelegatedTaskProposalHunkDisposition::Accepted,
        );
    }

    let mut store = CheckpointStore::new();
    let result = apply_inline_edit_with_undo_group(
        &overlay,
        BufferId(12),
        ProposalId(1212),
        test_correlation_id(),
        test_causality_id(),
        &mut store,
    )
    .expect("no pending hunks — ha, hb, hc all have explicit dispositions");

    assert_eq!(
        result.checkpoint.targets.len(),
        3,
        "checkpoint must have one target per applied hunk (3 accepted hunks → 3 targets)"
    );
    // Each target must have content_before set (the original text for rollback).
    for target in &result.checkpoint.targets {
        assert!(
            target.content_before.is_some(),
            "checkpoint target must have content_before for rollback; target_id={}",
            target.target_id
        );
    }
}

#[test]
fn undo_group_and_checkpoint_are_correlated() {
    let instruction = sample_instruction();
    let mut overlay = inline_edit_from_instruction(instruction.clone(), "inst-10".to_string());
    let chunks = vec![complete_chunk("hd", "old", "new")];
    overlay.diff_hunks = accumulate_inline_edit_chunks(&chunks, &instruction);
    overlay.state = InlineEditOverlayState::Complete;
    overlay.hunk_dispositions.insert(
        "hd".to_string(),
        DelegatedTaskProposalHunkDisposition::Accepted,
    );

    let mut store = CheckpointStore::new();
    let result = apply_inline_edit_with_undo_group(
        &overlay,
        BufferId(13),
        ProposalId(1313),
        test_correlation_id(),
        test_causality_id(),
        &mut store,
    )
    .expect("no pending hunks — hd has an explicit disposition");

    // The checkpoint's proposal_id and the audit_record's proposal_id must
    // both match the result's proposal_id — proving the three artifacts
    // (undo group, checkpoint, audit record) are correlated.
    assert_eq!(
        result.checkpoint.proposal_id, result.proposal_id,
        "checkpoint.proposal_id must match InlineEditApplyResult.proposal_id"
    );
    assert_eq!(
        result.audit_record.proposal_id, result.proposal_id,
        "audit_record.proposal_id must match InlineEditApplyResult.proposal_id"
    );
    // The checkpoint_id in the result must match the checkpoint blob's id.
    assert_eq!(
        result.checkpoint.checkpoint_id, result.checkpoint_id,
        "result.checkpoint_id must match result.checkpoint.checkpoint_id"
    );
    // The undo_group_id must be non-nil (a real UUID was generated).
    assert_ne!(
        result.undo_group_id,
        uuid::Uuid::nil(),
        "undo_group_id must be non-nil"
    );
}

// ─── C1 correlation test ─────────────────────────────────────────────────────

/// The proposal_id generated by `inline_edit_to_workspace_proposal` must be the
/// same id that flows into the checkpoint produced by
/// `apply_inline_edit_with_undo_group`.  A single stable id ties proposal →
/// checkpoint → audit together.
#[test]
fn proposal_id_correlates_across_proposal_and_checkpoint() {
    let instruction = sample_instruction();
    let mut overlay = inline_edit_from_instruction(instruction.clone(), "inst-11".to_string());
    let chunks = vec![complete_chunk("he", "before_text", "after_text")];
    overlay.diff_hunks = accumulate_inline_edit_chunks(&chunks, &instruction);
    overlay.state = InlineEditOverlayState::Complete;
    overlay.hunk_dispositions.insert(
        "he".to_string(),
        DelegatedTaskProposalHunkDisposition::Accepted,
    );

    // Generate a proposal first, then pass its proposal_id into apply.
    let proposal = inline_edit_to_workspace_proposal(&overlay, BufferId(20))
        .expect("no pending hunks — he has an explicit disposition")
        .expect("proposal must be Some — at least one accepted hunk");

    let mut store = CheckpointStore::new();
    let result = apply_inline_edit_with_undo_group(
        &overlay,
        BufferId(20),
        proposal.proposal_id,
        test_correlation_id(),
        test_causality_id(),
        &mut store,
    )
    .expect("no pending hunks");

    // The same proposal_id must flow through to the checkpoint and apply result.
    assert_eq!(
        result.checkpoint.proposal_id, proposal.proposal_id,
        "checkpoint.proposal_id must match the WorkspaceProposal.proposal_id"
    );
    assert_eq!(
        result.proposal_id, proposal.proposal_id,
        "InlineEditApplyResult.proposal_id must match the WorkspaceProposal.proposal_id"
    );
}

// ─── C2 undecided-hunk guard tests ───────────────────────────────────────────

/// Both `apply_inline_edit_with_undo_group` and `inline_edit_to_workspace_proposal`
/// must refuse to proceed when any complete hunk has not yet been assigned a
/// disposition (the spec requires all hunks to be decided before apply).
#[test]
fn undecided_hunks_prevent_apply() {
    let instruction = sample_instruction();
    let mut overlay = inline_edit_from_instruction(instruction.clone(), "inst-12".to_string());

    // Two complete hunks; only one gets a disposition — hg is deliberately left Pending.
    let two_hunks = format!(
        "{}{}",
        complete_chunk("hf", "first_original", "first_replacement"),
        complete_chunk("hg", "second_original", "second_replacement"),
    );
    overlay.diff_hunks = accumulate_inline_edit_chunks(&[two_hunks], &instruction);
    overlay.state = InlineEditOverlayState::Complete;
    overlay.hunk_dispositions.insert(
        "hf".to_string(),
        DelegatedTaskProposalHunkDisposition::Accepted,
    );

    // apply must refuse when any complete hunk lacks a disposition.
    let mut store = CheckpointStore::new();
    let apply_result = apply_inline_edit_with_undo_group(
        &overlay,
        BufferId(21),
        ProposalId(2121),
        test_correlation_id(),
        test_causality_id(),
        &mut store,
    );
    assert!(
        matches!(apply_result, Err(InlineEditError::UndecidedHunksRemaining)),
        "apply must return UndecidedHunksRemaining when a complete hunk has no disposition"
    );

    // inline_edit_to_workspace_proposal must enforce the same guard.
    let proposal_result = inline_edit_to_workspace_proposal(&overlay, BufferId(21));
    assert!(
        matches!(
            proposal_result,
            Err(InlineEditError::UndecidedHunksRemaining)
        ),
        "inline_edit_to_workspace_proposal must also return UndecidedHunksRemaining"
    );
}

#[test]
fn explicit_pending_disposition_prevents_apply() {
    let instruction = sample_instruction();
    let mut overlay = inline_edit_from_instruction(instruction.clone(), "inst-13".to_string());

    let two_hunks = format!(
        "{}{}",
        complete_chunk("hp", "orig_p", "repl_p"),
        complete_chunk("hq", "orig_q", "repl_q"),
    );
    overlay.diff_hunks = accumulate_inline_edit_chunks(&[two_hunks], &instruction);
    overlay.state = InlineEditOverlayState::Complete;

    // hp is accepted, hq is explicitly set to Pending — must still be rejected
    overlay.hunk_dispositions.insert(
        "hp".to_string(),
        DelegatedTaskProposalHunkDisposition::Accepted,
    );
    overlay.hunk_dispositions.insert(
        "hq".to_string(),
        DelegatedTaskProposalHunkDisposition::Pending,
    );

    let mut store = CheckpointStore::new();
    let apply_result = apply_inline_edit_with_undo_group(
        &overlay,
        BufferId(22),
        ProposalId(2222),
        test_correlation_id(),
        test_causality_id(),
        &mut store,
    );
    assert!(
        matches!(apply_result, Err(InlineEditError::UndecidedHunksRemaining)),
        "explicit Pending disposition must be treated as undecided"
    );

    let proposal_result = inline_edit_to_workspace_proposal(&overlay, BufferId(22));
    assert!(
        matches!(
            proposal_result,
            Err(InlineEditError::UndecidedHunksRemaining)
        ),
        "explicit Pending disposition must also block proposal creation"
    );
}
