//! Integration tests for the inline edit loop (PKT-INLINE T1).
//!
//! Covers:
//! - T1: Streaming diff overlay anchored to current text (5 tests)

use legion_desktop::view::{
    InlineEditError, InlineEditOverlayState, accumulate_inline_edit_chunks,
    check_inline_edit_anchor_freshness, inline_edit_from_instruction,
    set_inline_edit_hunk_disposition,
};
use legion_protocol::{
    BufferVersion, DelegatedTaskProposalHunkDisposition, FileFingerprint, InlineEditInstruction,
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
