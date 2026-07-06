//! Inline edit diff overlay view model and lifecycle helpers (PKT-INLINE T1).
//!
//! The overlay is projection-only — it NEVER mutates buffers directly.
//! All mutations must go through the proposal pipeline in legion-app.

use std::collections::HashMap;

use legion_protocol::{
    BufferVersion, DelegatedTaskProposalHunkDisposition, InlineEditDiffHunk, InlineEditInstruction,
    SnapshotId,
};

// ─── Lifecycle state ─────────────────────────────────────────────────────────

/// Lifecycle state of an inline edit diff overlay.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InlineEditOverlayState {
    /// AI is streaming the diff response; overlay is partially populated.
    Streaming,
    /// All hunks have been received; overlay is ready for per-hunk review.
    Complete,
    /// The buffer changed since the instruction was issued; hunks may be misaligned.
    Stale,
    /// All accepted hunks have been applied through the proposal pipeline.
    Applied,
    /// The overlay was dismissed without applying any hunks.
    Rejected,
}

// ─── View model ──────────────────────────────────────────────────────────────

/// View model for the inline edit diff overlay.
///
/// The overlay is projection-only — it NEVER mutates buffers directly.
/// All mutations must be routed through the proposal pipeline in legion-app.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InlineEditOverlayViewModel {
    /// Stable instruction identifier (user-supplied or generated).
    pub instruction_id: String,
    /// The original instruction from the user.
    pub instruction: InlineEditInstruction,
    /// Structured diff hunks from the streaming response.
    pub diff_hunks: Vec<InlineEditDiffHunk>,
    /// Per-hunk accept/reject dispositions (keyed by `hunk_id`).
    ///
    /// Missing entries are treated as `Pending`. Set via
    /// [`set_inline_edit_hunk_disposition`].
    pub hunk_dispositions: HashMap<String, DelegatedTaskProposalHunkDisposition>,
    /// Current lifecycle state of the overlay.
    pub state: InlineEditOverlayState,
    /// True when the buffer has changed since the instruction was anchored,
    /// rendering the diff overlay potentially misaligned.
    pub stale: bool,
}

// ─── T1: Construction + freshness ────────────────────────────────────────────

/// Constructs a fresh [`InlineEditOverlayViewModel`] from an instruction.
///
/// The overlay starts in [`InlineEditOverlayState::Streaming`] with no diff
/// hunks; hunks are added as the AI streams them via
/// [`accumulate_inline_edit_chunks`].
#[must_use]
pub fn inline_edit_from_instruction(
    instruction: InlineEditInstruction,
    instruction_id: String,
) -> InlineEditOverlayViewModel {
    InlineEditOverlayViewModel {
        instruction_id,
        instruction,
        diff_hunks: Vec::new(),
        hunk_dispositions: HashMap::new(),
        state: InlineEditOverlayState::Streaming,
        stale: false,
    }
}

/// Returns `true` when the inline edit anchor is still fresh.
///
/// Returns `false` (stale) when either `snapshot_id` **or** `buffer_version`
/// has changed since the instruction was anchored.  A stale anchor means the
/// diff overlay is desynchronized from the current buffer and must not be
/// applied.
#[must_use]
pub fn check_inline_edit_anchor_freshness(
    instruction: &InlineEditInstruction,
    current_snapshot_id: SnapshotId,
    current_buffer_version: BufferVersion,
) -> bool {
    instruction.anchor_snapshot_id == current_snapshot_id
        && instruction.anchor_buffer_version == current_buffer_version
}

// ─── T1: Streaming accumulation ──────────────────────────────────────────────

/// Parses streaming diff chunks into structured [`InlineEditDiffHunk`]s.
///
/// The chunk format is a simple line-oriented diff.  Each hunk occupies a
/// section delimited by `---END---`.  Hunks without the closing delimiter are
/// still in-flight and have `complete: false`.
///
/// ## Format
///
/// ```text
/// {hunk_id}
/// {original_text}
/// ---SEP---
/// {replacement_text}
/// ---END---
/// ```
///
/// Multiple complete hunks may appear in sequence by repeating the block.
/// The final block without `---END---` (if present) produces an incomplete hunk.
///
/// # Incomplete hunks
///
/// Incomplete hunks have `complete: false`.  Only complete hunks are eligible
/// for accept/reject disposition.
#[must_use]
pub fn accumulate_inline_edit_chunks(
    chunks: &[String],
    instruction: &InlineEditInstruction,
) -> Vec<InlineEditDiffHunk> {
    let accumulated = chunks.join("");
    parse_inline_edit_text(&accumulated, instruction)
}

fn parse_inline_edit_text(
    text: &str,
    instruction: &InlineEditInstruction,
) -> Vec<InlineEditDiffHunk> {
    let mut hunks = Vec::new();

    // Split by ---END--- to identify complete hunk sections.
    let parts: Vec<&str> = text.split("---END---").collect();

    for (idx, part) in parts.iter().enumerate() {
        let part = part.trim_start_matches('\n');
        if part.is_empty() {
            continue;
        }

        // Parts that are NOT the last segment end with ---END---: complete.
        // The last segment has no ---END--- yet: incomplete.
        let is_trailing = idx == parts.len() - 1;
        let complete = !is_trailing;

        if let Some(hunk) = parse_hunk_section(part, instruction, complete) {
            hunks.push(hunk);
        }
    }

    hunks
}

fn parse_hunk_section(
    section: &str,
    instruction: &InlineEditInstruction,
    complete: bool,
) -> Option<InlineEditDiffHunk> {
    const SEP: &str = "---SEP---\n";

    let sep_pos = section.find(SEP)?;
    let header_and_original = &section[..sep_pos];
    let replacement_text = section[sep_pos + SEP.len()..]
        .trim_end_matches('\n')
        .to_string();

    let mut lines = header_and_original.splitn(2, '\n');
    let hunk_id = lines.next()?.trim().to_string();
    if hunk_id.is_empty() {
        return None;
    }
    let original_text = lines
        .next()
        .unwrap_or("")
        .trim_end_matches('\n')
        .to_string();

    Some(InlineEditDiffHunk {
        hunk_id,
        range: instruction.anchor_range,
        original_text,
        replacement_text,
        complete,
    })
}

// ─── T1: Disposition error (needed by T1 test incomplete_hunk_not_eligible_for_accept) ─

/// Error type for inline edit view-model operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InlineEditError {
    /// Attempted to set a disposition on an incomplete (still-streaming) hunk.
    HunkNotComplete {
        /// The incomplete hunk's identifier.
        hunk_id: String,
    },
    /// No hunk with the given identifier was found in the overlay.
    HunkNotFound {
        /// The missing hunk's identifier.
        hunk_id: String,
    },
    /// The overlay is in a state that does not allow disposition changes
    /// (e.g., still streaming, already applied, or stale).
    OverlayNotReady,
    /// Attempted to apply the overlay while one or more hunks still have a
    /// `Pending` disposition.
    UndecidedHunksRemaining,
}

impl std::fmt::Display for InlineEditError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::HunkNotComplete { hunk_id } => write!(
                f,
                "hunk `{hunk_id}` is still streaming and cannot be accepted/rejected"
            ),
            Self::HunkNotFound { hunk_id } => write!(
                f,
                "hunk `{hunk_id}` was not found in the inline edit overlay"
            ),
            Self::OverlayNotReady => {
                write!(f, "inline edit overlay is not in a reviewable state")
            }
            Self::UndecidedHunksRemaining => {
                write!(f, "all hunks must be accepted or rejected before applying")
            }
        }
    }
}

/// Sets the accept/reject disposition for a single hunk in the overlay.
///
/// # Errors
///
/// Returns [`InlineEditError::OverlayNotReady`] when the overlay is streaming,
/// already applied, or stale.
///
/// Returns [`InlineEditError::HunkNotFound`] when no hunk with `hunk_id`
/// exists in the overlay.
///
/// Returns [`InlineEditError::HunkNotComplete`] when the hunk is still being
/// streamed (`complete == false`).
pub fn set_inline_edit_hunk_disposition(
    overlay: &mut InlineEditOverlayViewModel,
    hunk_id: &str,
    disposition: DelegatedTaskProposalHunkDisposition,
) -> Result<(), InlineEditError> {
    if overlay.state == InlineEditOverlayState::Streaming
        || overlay.state == InlineEditOverlayState::Applied
        || overlay.state == InlineEditOverlayState::Stale
    {
        return Err(InlineEditError::OverlayNotReady);
    }

    let hunk = overlay
        .diff_hunks
        .iter()
        .find(|h| h.hunk_id == hunk_id)
        .ok_or_else(|| InlineEditError::HunkNotFound {
            hunk_id: hunk_id.to_string(),
        })?;

    if !hunk.complete {
        return Err(InlineEditError::HunkNotComplete {
            hunk_id: hunk_id.to_string(),
        });
    }

    overlay
        .hunk_dispositions
        .insert(hunk_id.to_string(), disposition);
    Ok(())
}

// ─── T2: Proposal pipeline integration ───────────────────────────────────────

/// Converts the accepted hunks in an inline edit overlay into a
/// [`WorkspaceProposal`] that the existing proposal-apply pipeline can execute.
///
/// Returns `None` when there are no accepted hunks (nothing to apply).
///
/// The proposal's preconditions encode the anchor snapshot and buffer version
/// so the apply pipeline can detect staleness before mutating the buffer.
///
/// Uses `FileId(buffer_id.0)` as the target file identity because the desktop
/// layer only has a `BufferId`.  The app layer maps `FileId` back to the open
/// buffer when executing the proposal.
#[must_use]
pub fn inline_edit_to_workspace_proposal(
    overlay: &InlineEditOverlayViewModel,
    buffer_id: legion_protocol::BufferId,
) -> Option<legion_protocol::WorkspaceProposal> {
    use legion_protocol::{
        CapabilityId, CorrelationId, EditBatch, FileId, PreviewSummary, PrincipalId,
        ProposalPayload, ProposalVersionPreconditions, TextEdit, TextEditProposal, TimestampMillis,
        WorkspaceProposal,
    };

    let accepted_hunks: Vec<&InlineEditDiffHunk> = overlay
        .diff_hunks
        .iter()
        .filter(|h| {
            overlay.hunk_dispositions.get(&h.hunk_id)
                == Some(&DelegatedTaskProposalHunkDisposition::Accepted)
        })
        .collect();

    if accepted_hunks.is_empty() {
        return None;
    }

    let edits = EditBatch {
        edits: accepted_hunks
            .iter()
            .map(|h| TextEdit {
                range: protocol_range_to_text_range(h.range),
                replacement: h.replacement_text.clone(),
            })
            .collect(),
    };

    let proposal_id = legion_protocol::ProposalId(TimestampMillis::now().0);

    Some(WorkspaceProposal {
        proposal_id,
        principal: PrincipalId("ai.inline-edit".to_string()),
        capability: CapabilityId("capability.inline-edit".to_string()),
        correlation_id: CorrelationId(0),
        payload: ProposalPayload::TextEdit(TextEditProposal {
            file_id: FileId(buffer_id.0),
            edits,
        }),
        preconditions: ProposalVersionPreconditions {
            file_version: None,
            buffer_version: Some(overlay.instruction.anchor_buffer_version),
            snapshot_id: Some(overlay.instruction.anchor_snapshot_id),
            generation: None,
            file_content_version: None,
            workspace_generation: None,
            expected_fingerprint: overlay.instruction.anchor_content_fingerprint.clone(),
            expected_file_length: None,
            expected_modified_at: None,
        },
        preview: PreviewSummary {
            summary: format!("Inline edit: {} accepted hunk(s)", accepted_hunks.len()),
            details: accepted_hunks
                .iter()
                .map(|h| format!("hunk {}", h.hunk_id))
                .collect(),
        },
        expires_at: None,
        created_at: TimestampMillis::now(),
    })
}

/// Builds a [`ProposalAuditRecord`] for an inline edit apply operation.
///
/// The audit record is created with:
/// - `lifecycle_state` = [`ProposalLifecycleState::Applied`]
/// - `payload_summary.kind` = [`ProposalPayloadKind::TextEdit`]
///
/// This is the same audit record structure used by multi-file proposal apply.
#[must_use]
pub fn build_inline_edit_audit_record(
    proposal_id: legion_protocol::ProposalId,
    applied_hunk_count: u32,
) -> legion_protocol::ProposalAuditRecord {
    use legion_protocol::{
        CapabilityId, CausalityId, CorrelationId, PrincipalId, ProposalAuditRecord,
        ProposalLifecycleState, ProposalPayloadKind, ProposalPayloadSummary, TimestampMillis,
    };
    use uuid::Uuid;

    ProposalAuditRecord {
        proposal_id,
        lifecycle_state: ProposalLifecycleState::Applied,
        timestamp: TimestampMillis::now(),
        principal: PrincipalId("ai.inline-edit".to_string()),
        capability: CapabilityId("capability.inline-edit".to_string()),
        correlation_id: CorrelationId(0),
        causality_id: CausalityId(Uuid::nil()),
        payload_summary: ProposalPayloadSummary {
            kind: ProposalPayloadKind::TextEdit,
            affected_files: Vec::new(),
            title: Some(format!("Inline edit ({applied_hunk_count} hunk(s))")),
            byte_count: None,
        },
        checkpoint_rollback_projection: None,
        risk_rule_ids: Vec::new(),
        diagnostics: Vec::new(),
        redaction_hints: Vec::new(),
        schema_version: 1,
    }
}

// ─── T3: Undo integration + checkpoint ───────────────────────────────────────

/// Result of applying an inline edit overlay with a single undo group.
///
/// All text edits in the apply share the same `undo_group_id`, so a single
/// undo in the editor reverts the entire inline edit.
///
/// The `checkpoint` field contains the pre-mutation state.  Callers (typically
/// the app layer) must persist this to their [`CheckpointStore`] to enable
/// durable rollback.
///
/// [`CheckpointStore`]: legion_storage::checkpoint::CheckpointStore
#[derive(Debug, Clone)]
pub struct InlineEditApplyResult {
    /// Proposal identifier for the generated workspace proposal.
    pub proposal_id: legion_protocol::ProposalId,
    /// Shared undo group identifier.  All text edits from this inline edit
    /// apply must use this id so that a single undo reverts them all.
    pub undo_group_id: uuid::Uuid,
    /// Stable checkpoint identifier.
    pub checkpoint_id: String,
    /// Number of hunks that were accepted and applied.
    pub applied_hunk_count: u32,
    /// Audit record for the apply operation.
    pub audit_record: legion_protocol::ProposalAuditRecord,
    /// Pre-mutation checkpoint blob.  The caller must persist this to the
    /// [`CheckpointStore`] for durable rollback support.
    ///
    /// [`CheckpointStore`]: legion_storage::checkpoint::CheckpointStore
    pub checkpoint: legion_storage::checkpoint::DurableCheckpoint,
}

/// Applies the accepted hunks in an inline edit overlay under a single undo group.
///
/// Generates a `Uuid` undo group id that must be shared across every editor
/// transaction produced during apply.  Creates a
/// [`DurableCheckpoint`][legion_storage::checkpoint::DurableCheckpoint] that
/// captures the pre-mutation state for all applied hunks so the caller can
/// persist it to a [`CheckpointStore`][legion_storage::checkpoint::CheckpointStore].
///
/// Returns an `InlineEditApplyResult` with `applied_hunk_count == 0` when
/// there are no accepted hunks.
#[must_use]
pub fn apply_inline_edit_with_undo_group(
    overlay: &InlineEditOverlayViewModel,
    buffer_id: legion_protocol::BufferId,
) -> InlineEditApplyResult {
    use legion_protocol::{CanonicalPath, PrincipalId, TimestampMillis};
    use legion_storage::checkpoint::{
        CHECKPOINT_SCHEMA_VERSION, CheckpointTarget, CheckpointTargetKind, DurableCheckpoint,
    };

    let accepted_hunks: Vec<&InlineEditDiffHunk> = overlay
        .diff_hunks
        .iter()
        .filter(|h| {
            overlay.hunk_dispositions.get(&h.hunk_id)
                == Some(&DelegatedTaskProposalHunkDisposition::Accepted)
        })
        .collect();

    let undo_group_id = uuid::Uuid::now_v7();
    let checkpoint_id = format!("ckpt-{}", uuid::Uuid::now_v7());
    let proposal_id = legion_protocol::ProposalId(TimestampMillis::now().0);
    let applied_hunk_count = accepted_hunks.len() as u32;

    // Build one checkpoint target per accepted hunk (SavedFile kind, original
    // text as content_before for rollback).
    let targets: Vec<CheckpointTarget> = accepted_hunks
        .iter()
        .enumerate()
        .map(|(i, h)| CheckpointTarget {
            target_id: format!("target-{}-{i}", h.hunk_id),
            kind: CheckpointTargetKind::SavedFile,
            path: CanonicalPath(format!("buffer://{}", buffer_id.0)),
            content_before: Some(h.original_text.clone()),
        })
        .collect();

    let checkpoint = DurableCheckpoint {
        checkpoint_id: checkpoint_id.clone(),
        proposal_id,
        principal: PrincipalId("ai.inline-edit".to_string()),
        created_at: TimestampMillis::now(),
        targets,
        available: true,
        schema_version: CHECKPOINT_SCHEMA_VERSION,
    };

    let audit_record = build_inline_edit_audit_record(proposal_id, applied_hunk_count);

    InlineEditApplyResult {
        proposal_id,
        undo_group_id,
        checkpoint_id,
        applied_hunk_count,
        audit_record,
        checkpoint,
    }
}

// ─── Internal helpers ─────────────────────────────────────────────────────────

/// Converts a [`ProtocolTextRange`] (line/character coordinates) to a
/// [`TextRange`] (byte offsets) for use in a proposal payload.
///
/// Uses the `byte_offset` field of each [`TextCoordinate`] when available;
/// falls back to the character value interpreted as a byte offset otherwise.
fn protocol_range_to_text_range(
    range: legion_protocol::ProtocolTextRange,
) -> legion_protocol::TextRange {
    let start = range
        .start
        .byte_offset
        .unwrap_or(range.start.character as u64);
    let end = range.end.byte_offset.unwrap_or(range.end.character as u64);
    legion_protocol::TextRange::byte(start, end)
}
