//! Projection-only UI primitives for the native shell.

use devil_protocol::{
    AssistedAiProjection, BufferId, CanonicalPath, CheckpointRollbackProjection,
    ContextManifestEgressStatus, ContextManifestProjection, ContextManifestPurpose,
    ContextManifestRecord, DelegatedTaskProjection, DelegatedTaskRuntimeActivationState, FileId,
    PermissionBudgetProjection, PrivacyInspectorProjection, ProposalApprovalChecklistProjection,
    ProposalCancellationReason, ProposalId, ProposalLedgerProjection, ProposalPrivacyLabel,
    ProposalRejectionReason, ProposalRiskLabel, ProposalRollbackReason, ProtocolTextRange,
    RedactionHint, TextCoordinate, TimestampMillis, WorkspaceId,
};
use thiserror::Error;

/// Render mode for shell projections.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RenderMode {
    /// Basic projection listing.
    Plain,
}

/// Explorer tree projection consumed by shell-style UI surfaces.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExplorerProjection {
    /// Flat node list from workspace tree snapshot.
    pub nodes: Vec<ExplorerNodeProjection>,
    /// Optional selected node in the explorer.
    pub selection: Option<ExplorerSelectionProjection>,
}

/// Projected explorer node.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExplorerNodeProjection {
    /// Stable file identifier.
    pub file_id: FileId,
    /// Canonical file path.
    pub canonical_path: CanonicalPath,
    /// Display name for UI list/tree rows.
    pub name: String,
    /// Child identifiers for directory rows.
    pub children: Vec<FileId>,
}

/// Projected explorer selection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExplorerSelectionProjection {
    /// Selected file identifier.
    pub file_id: FileId,
}

/// Minimal layout model used by the shell projection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Layout {
    /// Window title for the shell.
    pub title: String,
    /// Width of the frame.
    pub width: u16,
    /// Height of the frame.
    pub height: u16,
}

impl Layout {
    /// Construct a layout.
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            width: 80,
            height: 24,
        }
    }
}

/// Top-level layout projection consumed by the shell.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShellLayoutProjection {
    /// Window layout.
    pub layout: Layout,
    /// Current render mode.
    pub mode: RenderMode,
}

impl ShellLayoutProjection {
    /// Construct a plain layout projection.
    pub fn plain(title: impl Into<String>) -> Self {
        Self {
            layout: Layout::new(title),
            mode: RenderMode::Plain,
        }
    }
}

/// Active editor-buffer projection received by the UI from application state.
#[derive(Debug, Clone, PartialEq)]
pub struct ActiveBufferProjection {
    /// Owning workspace identifier if a workspace is open.
    pub workspace_id: Option<WorkspaceId>,
    /// Active editor buffer identifier.
    pub buffer_id: Option<BufferId>,
    /// Active workspace file identifier.
    pub file_id: Option<FileId>,
    /// Canonical path for display only.
    pub file_path: Option<CanonicalPath>,
    /// Bounded viewport projection instead of unbounded text.
    pub viewport: Option<devil_protocol::ViewportProjection>,
    /// Degraded status from the application layer.
    pub degraded: bool,
    /// Bounded small-buffer preview, requested explicitly.
    pub small_buffer_preview: Option<String>,
    /// Dirty indicator projected from the editor engine.
    pub dirty: bool,
}

impl ActiveBufferProjection {
    /// Construct an empty active-buffer projection.
    pub fn empty() -> Self {
        Self {
            workspace_id: None,
            buffer_id: None,
            file_id: None,
            file_path: None,
            viewport: None,
            degraded: false,
            small_buffer_preview: None,
            dirty: false,
        }
    }

    /// Return a bounded small-buffer preview if available.
    pub fn small_buffer_text(&self) -> Option<&str> {
        self.small_buffer_preview.as_deref()
    }
}

impl Default for ActiveBufferProjection {
    fn default() -> Self {
        Self::empty()
    }
}

/// UI status severity.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatusSeverity {
    /// Informational status message.
    Info,
    /// Warning status message.
    Warning,
    /// Error status message.
    Error,
}

/// Projected status message shown by the shell.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StatusMessageProjection {
    /// Severity classification.
    pub severity: StatusSeverity,
    /// Human-readable message.
    pub message: String,
}

/// Typed command intent emitted by UI input handling.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CommandDispatchIntent {
    /// No command was recognized.
    Noop,
    /// Quit the active shell loop.
    Quit,
    /// Undo through application/editor authority for the target buffer.
    Undo {
        /// Target buffer identifier.
        buffer_id: BufferId,
    },
    /// Redo through application/editor authority for the target buffer.
    Redo {
        /// Target buffer identifier.
        buffer_id: BufferId,
    },
    /// Insert text through application/editor authority for the target buffer.
    Insert {
        /// Target buffer identifier.
        buffer_id: BufferId,
        /// Insertion position in projected protocol text coordinates.
        at: TextCoordinate,
        /// Replacement payload.
        text: String,
    },
    /// Delete a protocol text range through application/editor authority for the target buffer.
    Delete {
        /// Target buffer identifier.
        buffer_id: BufferId,
        /// Range to delete.
        range: ProtocolTextRange,
    },
    /// Replace a protocol text range through application/editor authority for the target buffer.
    Replace {
        /// Target buffer identifier.
        buffer_id: BufferId,
        /// Range to replace.
        range: ProtocolTextRange,
        /// Replacement payload.
        replacement: String,
    },
    /// Save through the editor save-request and workspace write path.
    Save {
        /// Target buffer identifier.
        buffer_id: BufferId,
    },
    /// Open a file by path through workspace authority.
    OpenPath {
        /// User-provided path text.
        path: String,
    },
    /// Refresh explorer state through workspace ports.
    RefreshExplorer,
    /// Reveal a workspace file in the explorer projection.
    RevealInExplorer {
        /// File identifier to reveal.
        file_id: FileId,
    },
    /// Request a proposal preview through app/protocol authority.
    PreviewProposal {
        /// Proposal identifier selected from projection data.
        proposal_id: ProposalId,
    },
    /// Approve a proposal through app/protocol authority.
    ApproveProposal {
        /// Proposal identifier selected from projection data.
        proposal_id: ProposalId,
    },
    /// Reject a proposal through app/protocol authority.
    RejectProposal {
        /// Proposal identifier selected from projection data.
        proposal_id: ProposalId,
        /// User rejection reason.
        reason: ProposalRejectionReason,
    },
    /// Apply a proposal through app/protocol authority.
    ApplyProposal {
        /// Proposal identifier selected from projection data.
        proposal_id: ProposalId,
    },
    /// Roll back a proposal through app/protocol authority.
    RollbackProposal {
        /// Proposal identifier selected from projection data.
        proposal_id: ProposalId,
        /// User rollback reason.
        reason: ProposalRollbackReason,
    },
    /// Cancel a proposal through app/protocol authority.
    CancelProposal {
        /// Proposal identifier selected from projection data.
        proposal_id: ProposalId,
        /// User cancellation reason.
        reason: ProposalCancellationReason,
    },
    /// Open proposal details by selecting static projection data.
    OpenProposalDetails {
        /// Proposal identifier selected from projection data.
        proposal_id: ProposalId,
    },
}

/// Projection snapshot provided to the shell by the application layer.
#[derive(Debug, Clone, PartialEq)]
pub struct ShellProjectionSnapshot {
    /// Layout projection.
    pub layout_projection: ShellLayoutProjection,
    /// Explorer projection.
    pub explorer_projection: ExplorerProjection,
    /// Active buffer projection.
    pub active_buffer_projection: ActiveBufferProjection,
    /// Status message projections.
    pub status_messages: Vec<StatusMessageProjection>,
    /// Proposal ledger projection supplied by the application layer.
    pub proposal_ledger_projection: ProposalLedgerProjection,
    /// Trust-layer context manifest projection supplied by the application layer.
    pub context_manifest_projection: ContextManifestProjection,
    /// Trust-layer privacy inspector projection supplied by the application layer.
    pub privacy_inspector_projection: PrivacyInspectorProjection,
    /// Trust-layer permission budget projection supplied by the application layer.
    pub permission_budget_projection: PermissionBudgetProjection,
    /// Trust-layer approval checklist projection supplied by the application layer.
    pub approval_checklist_projection: ProposalApprovalChecklistProjection,
    /// Trust-layer checkpoint/rollback projection supplied by the application layer.
    pub checkpoint_rollback_projection: CheckpointRollbackProjection,
    /// Assisted-AI projection supplied by the application layer.
    pub assisted_ai_projection: AssistedAiProjection,
    /// Delegated-task plan projection supplied by the application layer.
    pub delegated_task_projection: DelegatedTaskProjection,
}

/// Command parsing errors surfaced by projection-only shell input handling.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ShellCommandError {
    /// A command requires an active buffer projection, but none is present.
    #[error("active buffer projection is missing")]
    ActiveBufferMissing,
    /// A command supplied a range with start after end.
    #[error("command range start must be <= end")]
    InvalidRange,
}

/// Projection-only IDE shell state.
#[derive(Debug)]
pub struct Shell {
    /// Projection-only layout state.
    pub layout_projection: ShellLayoutProjection,
    /// Projection-only explorer state.
    pub explorer_projection: ExplorerProjection,
    /// Projection-only active buffer state.
    pub active_buffer_projection: ActiveBufferProjection,
    /// Projected status messages.
    pub status_messages: Vec<StatusMessageProjection>,
    /// Static proposal ledger projection.
    pub proposal_ledger_projection: ProposalLedgerProjection,
    /// Static trust-layer context manifest projection.
    pub context_manifest_projection: ContextManifestProjection,
    /// Static trust-layer privacy inspector projection.
    pub privacy_inspector_projection: PrivacyInspectorProjection,
    /// Static trust-layer permission budget projection.
    pub permission_budget_projection: PermissionBudgetProjection,
    /// Static trust-layer approval checklist projection.
    pub approval_checklist_projection: ProposalApprovalChecklistProjection,
    /// Static trust-layer checkpoint/rollback projection.
    pub checkpoint_rollback_projection: CheckpointRollbackProjection,
    /// Static assisted-AI projection.
    pub assisted_ai_projection: AssistedAiProjection,
    /// Static delegated-task plan projection.
    pub delegated_task_projection: DelegatedTaskProjection,
    /// Command dispatch intents emitted by input parsing.
    pub command_dispatch_intents: Vec<CommandDispatchIntent>,
}

impl Shell {
    /// Create a shell from a projection snapshot.
    pub fn new(snapshot: ShellProjectionSnapshot) -> Self {
        Self {
            layout_projection: snapshot.layout_projection,
            explorer_projection: snapshot.explorer_projection,
            active_buffer_projection: snapshot.active_buffer_projection,
            status_messages: snapshot.status_messages,
            proposal_ledger_projection: snapshot.proposal_ledger_projection,
            context_manifest_projection: snapshot.context_manifest_projection,
            privacy_inspector_projection: snapshot.privacy_inspector_projection,
            permission_budget_projection: snapshot.permission_budget_projection,
            approval_checklist_projection: snapshot.approval_checklist_projection,
            checkpoint_rollback_projection: snapshot.checkpoint_rollback_projection,
            assisted_ai_projection: snapshot.assisted_ai_projection,
            delegated_task_projection: snapshot.delegated_task_projection,
            command_dispatch_intents: Vec::new(),
        }
    }

    /// Create an empty projection-only shell.
    pub fn empty(title: impl Into<String>) -> Self {
        Self::new(ShellProjectionSnapshot {
            layout_projection: ShellLayoutProjection::plain(title),
            explorer_projection: ExplorerProjection {
                nodes: Vec::new(),
                selection: None,
            },
            active_buffer_projection: ActiveBufferProjection::empty(),
            status_messages: Vec::new(),
            proposal_ledger_projection: empty_proposal_ledger_projection(),
            context_manifest_projection: empty_context_manifest_projection(),
            privacy_inspector_projection: empty_privacy_inspector_projection(),
            permission_budget_projection: empty_permission_budget_projection(),
            approval_checklist_projection: empty_approval_checklist_projection(),
            checkpoint_rollback_projection: empty_checkpoint_rollback_projection(),
            assisted_ai_projection: empty_assisted_ai_projection(),
            delegated_task_projection: empty_delegated_task_projection(),
        })
    }

    /// Return a cloned shell projection snapshot.
    pub fn projection_snapshot(&self) -> ShellProjectionSnapshot {
        ShellProjectionSnapshot {
            layout_projection: self.layout_projection.clone(),
            explorer_projection: self.explorer_projection.clone(),
            active_buffer_projection: self.active_buffer_projection.clone(),
            status_messages: self.status_messages.clone(),
            proposal_ledger_projection: self.proposal_ledger_projection.clone(),
            context_manifest_projection: self.context_manifest_projection.clone(),
            privacy_inspector_projection: self.privacy_inspector_projection.clone(),
            permission_budget_projection: self.permission_budget_projection.clone(),
            approval_checklist_projection: self.approval_checklist_projection.clone(),
            checkpoint_rollback_projection: self.checkpoint_rollback_projection.clone(),
            assisted_ai_projection: self.assisted_ai_projection.clone(),
            delegated_task_projection: self.delegated_task_projection.clone(),
        }
    }

    /// Replace all render projections at once.
    pub fn replace_projection_snapshot(&mut self, snapshot: ShellProjectionSnapshot) {
        self.layout_projection = snapshot.layout_projection;
        self.explorer_projection = snapshot.explorer_projection;
        self.active_buffer_projection = snapshot.active_buffer_projection;
        self.status_messages = snapshot.status_messages;
        self.proposal_ledger_projection = snapshot.proposal_ledger_projection;
        self.context_manifest_projection = snapshot.context_manifest_projection;
        self.privacy_inspector_projection = snapshot.privacy_inspector_projection;
        self.permission_budget_projection = snapshot.permission_budget_projection;
        self.approval_checklist_projection = snapshot.approval_checklist_projection;
        self.checkpoint_rollback_projection = snapshot.checkpoint_rollback_projection;
        self.assisted_ai_projection = snapshot.assisted_ai_projection;
        self.delegated_task_projection = snapshot.delegated_task_projection;
    }

    /// Drain queued command-dispatch intents.
    pub fn drain_command_dispatch_intents(&mut self) -> Vec<CommandDispatchIntent> {
        self.command_dispatch_intents.drain(..).collect()
    }

    /// Render basic status and file content.
    pub fn render(&self) {
        print!("\x1b[2J\x1b[H");
        println!("{}", self.layout_projection.layout.title);
        println!(
            "Mode: {:?} | {}x{}",
            self.layout_projection.mode,
            self.layout_projection.layout.width,
            self.layout_projection.layout.height
        );
        println!(
            "{}",
            "-".repeat(self.layout_projection.layout.width as usize)
        );

        if self.active_buffer_projection.degraded {
            println!("<Degraded Mode: Large File>");
        }

        if let Some(text) = self.active_buffer_projection.small_buffer_text() {
            println!("{}", text);
        } else if let Some(viewport) = &self.active_buffer_projection.viewport {
            for slice in &viewport.line_slices {
                println!("{}", slice.visible_text);
            }
        } else {
            println!("<no active buffer>");
        }

        println!(
            "{}",
            "-".repeat(self.layout_projection.layout.width as usize)
        );
        let path = self
            .active_buffer_projection
            .file_path
            .as_ref()
            .map(|path| path.0.as_str())
            .unwrap_or("<no active file>");
        println!("Path: {}", path);
        if !self.proposal_ledger_projection.rows.is_empty() {
            println!("Proposals:");
            for row in &self.proposal_ledger_projection.rows {
                println!(
                    "#{} [{}] {} | risk={:?} privacy={:?} rollback={:?} targets={} hunks={} redacted={}",
                    row.proposal_id.0,
                    row.lifecycle.label,
                    row.title,
                    row.risk_label,
                    row.privacy_label,
                    row.rollback,
                    row.diff_summary.target_count,
                    row.diff_summary.hunk_count,
                    row.diff_summary.full_source_redacted
                );
            }
        }
        if !self.context_manifest_projection.manifest.items.is_empty() {
            let manifest = &self.context_manifest_projection.manifest;
            println!(
                "Context manifest {} | items={} omitted={} risk={:?} privacy={:?} egress={:?}",
                manifest.manifest_id,
                manifest.items.len(),
                manifest.omitted_item_count,
                manifest.risk_label,
                manifest.privacy_label,
                manifest.egress
            );
            for item in &manifest.items {
                println!(
                    "- {} {:?} {:?} ranges={} hashes={} risk={:?} privacy={:?}",
                    item.item_id,
                    item.kind,
                    item.inclusion,
                    item.ranges.len(),
                    item.hashes.len(),
                    item.risk_label,
                    item.privacy_label
                );
            }
        }
        if !self.privacy_inspector_projection.records.is_empty() {
            let inspector = &self.privacy_inspector_projection;
            println!(
                "Privacy inspector {} | records={} denied={} redacted={} egress={} high_risk={}",
                inspector.inspector_id,
                inspector.records.len(),
                inspector.denied_record_count,
                inspector.redacted_record_count,
                inspector.external_egress_record_count,
                inspector.high_risk_record_count
            );
            for record in &inspector.records {
                println!(
                    "- {} {:?} {:?} ranges={} hashes={} risk={:?} privacy={:?} redaction={:?}",
                    record.exposure_id,
                    record.source_kind,
                    record.inclusion,
                    record.ranges.len(),
                    record.hashes.len(),
                    record.risk_label,
                    record.privacy_label,
                    record.redaction_state
                );
            }
        }
        if !self.permission_budget_projection.budgets.is_empty()
            || !self.permission_budget_projection.evaluations.is_empty()
        {
            let budgets = &self.permission_budget_projection;
            println!(
                "Permission budgets {} | budgets={} denied={} depleted={} refused_evaluations={}",
                budgets.projection_id,
                budgets.budgets.len(),
                budgets.denied_budget_count,
                budgets.depleted_budget_count,
                budgets.refused_evaluation_count
            );
            for budget in &budgets.budgets {
                println!(
                    "- {} {:?} state={:?} used={} ceiling={:?} risk={:?}",
                    budget.budget_id,
                    budget.action_class,
                    budget.state,
                    budget.usage.used,
                    budget.usage.ceiling,
                    budget.risk_label
                );
            }
        }
        if !self.approval_checklist_projection.gates.is_empty() {
            let checklist = &self.approval_checklist_projection;
            println!(
                "Approval checklist {} | proposal={} ready={} blockers={}",
                checklist.checklist_id,
                checklist.proposal_id.0,
                checklist.ready_for_approval,
                checklist.blockers.len()
            );
            for gate in &checklist.gates {
                println!(
                    "- {:?} status={:?} risk={:?} privacy={:?} reasons={}",
                    gate.gate,
                    gate.status,
                    gate.risk_label,
                    gate.privacy_label,
                    gate.reasons.len()
                );
            }
        }
        if !self.checkpoint_rollback_projection.targets.is_empty()
            || !self
                .checkpoint_rollback_projection
                .rollback
                .limitations
                .is_empty()
        {
            let rollback = &self.checkpoint_rollback_projection;
            println!(
                "Checkpoint/Rollback {} | proposal={} checkpoint_available={} rollback={:?} targets={} limitations={}",
                rollback.projection_id,
                rollback.proposal_id.0,
                rollback.checkpoint.available,
                rollback.rollback.availability,
                rollback.targets.len(),
                rollback.rollback.limitations.len()
            );
        }
        if !self.assisted_ai_projection.providers.is_empty()
            || !self.assisted_ai_projection.requests.is_empty()
            || !self.assisted_ai_projection.proposal_previews.is_empty()
        {
            let assisted = &self.assisted_ai_projection;
            println!(
                "Assisted AI {} | providers={} requests={} refusals={} preview_ready={} invocation={:?}",
                assisted.projection_id,
                assisted.provider_count,
                assisted.request_count,
                assisted.refusal_count,
                assisted.preview_ready_count,
                assisted.provider_invocation
            );
            for provider in &assisted.providers {
                println!(
                    "- provider {} class={:?} availability={:?} ops={} model_labels={} tool_labels={} risk={:?} privacy={:?}",
                    provider.provider_id,
                    provider.provider_class,
                    provider.availability,
                    provider.supported_operation_count,
                    provider.model_capability_label_count,
                    provider.tool_capability_label_count,
                    provider.risk_label,
                    provider.privacy_label
                );
            }
            for route in &assisted.routes {
                println!(
                    "- route {} provider={} op={:?} disposition={:?} invocation={:?} refused_budgets={}",
                    route.request_id,
                    route.provider_id,
                    route.operation_class,
                    route.disposition,
                    route.provider_invocation,
                    route.refused_permission_budget_evaluation_count
                );
            }
            for preview in &assisted.proposal_previews {
                println!(
                    "- preview {} proposal={} readiness={:?} ready_preview={} ready_approval={} ready_apply={} targets={} hunks={} preconditions={}",
                    preview.preview_id,
                    preview.proposal_id.0,
                    preview.readiness,
                    preview.ready_for_preview,
                    preview.ready_for_approval,
                    preview.ready_for_apply,
                    preview.target_coverage.targets.len(),
                    preview.diff_summary.hunk_count,
                    preview.preconditions.core_preconditions_present
                );
            }
        }
        if !self.delegated_task_projection.plan_rows.is_empty()
            || !self.delegated_task_projection.blockers.is_empty()
            || !self.delegated_task_projection.refusals.is_empty()
        {
            let delegated = &self.delegated_task_projection;
            println!(
                "Delegated tasks {} | plans={} blocked={} refused={} activation={:?}",
                delegated.projection_id,
                delegated.plan_count,
                delegated.blocked_plan_count,
                delegated.refused_plan_count,
                delegated.runtime_activation
            );
            for row in &delegated.plan_rows {
                println!(
                    "- plan {} state={:?} readiness={:?} steps={} targets={} blockers={} refusals={} previews={} risk={:?} privacy={:?}",
                    row.plan_id.0,
                    row.plan_state,
                    row.readiness,
                    row.step_count,
                    row.affected_target_count,
                    row.blocker_count,
                    row.refusal_count,
                    row.proposal_preview_link_count,
                    row.risk_label,
                    row.privacy_label
                );
            }
            for step in &delegated.step_summaries {
                println!(
                    "- step {} plan={} op={:?} state={:?} deps={} targets={} proposal={:?} blockers={}",
                    step.step_id.0,
                    step.plan_id.0,
                    step.operation_class,
                    step.state,
                    step.dependency_count,
                    step.target_count,
                    step.proposal_id.map(|proposal| proposal.0),
                    step.blocker_count
                );
            }
        }
        println!("Commands: :i text | :d start,end | :r start,end,text | :w | :u | :redo | :q");
    }

    /// Parse a command and emit a typed dispatch intent without mutating editor or workspace state.
    pub fn handle_command(
        &mut self,
        input: &str,
    ) -> Result<Option<CommandDispatchIntent>, ShellCommandError> {
        let trimmed = input.trim();
        if trimmed == ":q" {
            return Ok(Some(self.push_intent(CommandDispatchIntent::Quit)));
        }
        if trimmed == ":u" {
            let buffer_id = self.active_buffer_id()?;
            return Ok(Some(
                self.push_intent(CommandDispatchIntent::Undo { buffer_id }),
            ));
        }
        if trimmed == ":redo" {
            let buffer_id = self.active_buffer_id()?;
            return Ok(Some(
                self.push_intent(CommandDispatchIntent::Redo { buffer_id }),
            ));
        }
        if trimmed == ":w" {
            let buffer_id = self.active_buffer_id()?;
            return Ok(Some(
                self.push_intent(CommandDispatchIntent::Save { buffer_id }),
            ));
        }

        if let Some(proposal_id) = parse_proposal_id(trimmed.strip_prefix(":proposal-preview ")) {
            return Ok(Some(self.push_intent(
                CommandDispatchIntent::PreviewProposal { proposal_id },
            )));
        }
        if let Some(proposal_id) = parse_proposal_id(trimmed.strip_prefix(":proposal-approve ")) {
            return Ok(Some(self.push_intent(
                CommandDispatchIntent::ApproveProposal { proposal_id },
            )));
        }
        if let Some(proposal_id) = parse_proposal_id(trimmed.strip_prefix(":proposal-reject ")) {
            return Ok(Some(self.push_intent(
                CommandDispatchIntent::RejectProposal {
                    proposal_id,
                    reason: ProposalRejectionReason::UserRejected,
                },
            )));
        }
        if let Some(proposal_id) = parse_proposal_id(trimmed.strip_prefix(":proposal-apply ")) {
            return Ok(Some(self.push_intent(
                CommandDispatchIntent::ApplyProposal { proposal_id },
            )));
        }
        if let Some(proposal_id) = parse_proposal_id(trimmed.strip_prefix(":proposal-rollback ")) {
            return Ok(Some(self.push_intent(
                CommandDispatchIntent::RollbackProposal {
                    proposal_id,
                    reason: ProposalRollbackReason::UserRequested,
                },
            )));
        }
        if let Some(proposal_id) = parse_proposal_id(trimmed.strip_prefix(":proposal-cancel ")) {
            return Ok(Some(self.push_intent(
                CommandDispatchIntent::CancelProposal {
                    proposal_id,
                    reason: ProposalCancellationReason::UserCancelled,
                },
            )));
        }
        if let Some(proposal_id) = parse_proposal_id(trimmed.strip_prefix(":proposal-details ")) {
            return Ok(Some(self.push_intent(
                CommandDispatchIntent::OpenProposalDetails { proposal_id },
            )));
        }

        if let Some(payload) = trimmed.strip_prefix(":i ") {
            let buffer_id = self.active_buffer_id()?;
            let pos = protocol_text_coordinate(0, 0, Some(0));
            return Ok(Some(self.push_intent(CommandDispatchIntent::Insert {
                buffer_id,
                at: pos,
                text: payload.to_string(),
            })));
        }

        if let Some(payload) = trimmed.strip_prefix(":d ") {
            let buffer_id = self.active_buffer_id()?;
            let mut split = payload.split(',');
            let start = split.next().unwrap_or("0").parse::<usize>().unwrap_or(0);
            let end = split.next().unwrap_or("0").parse::<usize>().unwrap_or(0);
            if start > end {
                return Err(ShellCommandError::InvalidRange);
            }
            let start = self.parse_pos(start);
            let end = self.parse_pos(end);
            return Ok(Some(self.push_intent(CommandDispatchIntent::Delete {
                buffer_id,
                range: ProtocolTextRange { start, end },
            })));
        }

        if let Some(payload) = trimmed.strip_prefix(":r ") {
            let buffer_id = self.active_buffer_id()?;
            let mut split = payload.splitn(3, ',');
            let start = split.next().unwrap_or("0").parse::<usize>().unwrap_or(0);
            let end = split.next().unwrap_or("0").parse::<usize>().unwrap_or(0);
            let replacement = split.next().unwrap_or("");
            if start > end {
                return Err(ShellCommandError::InvalidRange);
            }
            let start = self.parse_pos(start);
            let end = self.parse_pos(end);
            return Ok(Some(self.push_intent(CommandDispatchIntent::Replace {
                buffer_id,
                range: ProtocolTextRange { start, end },
                replacement: replacement.to_string(),
            })));
        }

        Ok(Some(self.push_intent(CommandDispatchIntent::Noop)))
    }

    fn active_buffer_id(&self) -> Result<BufferId, ShellCommandError> {
        self.active_buffer_projection
            .buffer_id
            .ok_or(ShellCommandError::ActiveBufferMissing)
    }

    fn push_intent(&mut self, intent: CommandDispatchIntent) -> CommandDispatchIntent {
        self.command_dispatch_intents.push(intent.clone());
        intent
    }

    fn parse_pos(&self, byte_offset: usize) -> TextCoordinate {
        if let Some(text) = self.active_buffer_projection.small_buffer_text() {
            return text
                .as_bytes()
                .get(..byte_offset)
                .map(|prefix| {
                    let line = prefix.iter().filter(|b| **b == b'\n').count() as u32;
                    let character = prefix.iter().rev().take_while(|b| **b != b'\n').count() as u32;
                    protocol_text_coordinate(line, character, Some(byte_offset as u64))
                })
                .unwrap_or_else(|| protocol_text_coordinate(0, 0, Some(0)));
        }

        if let Some(viewport) = &self.active_buffer_projection.viewport {
            let mut current_offset = 0;
            for (i, slice) in viewport.line_slices.iter().enumerate() {
                let slice_len = slice.visible_text.len() + 1; // +1 for newline
                if current_offset + slice_len > byte_offset {
                    let character = (byte_offset - current_offset) as u32;
                    let line = viewport.scroll.top_line + i as u32;
                    return protocol_text_coordinate(line, character, Some(byte_offset as u64));
                }
                current_offset += slice_len;
            }
        }

        protocol_text_coordinate(0, 0, Some(0))
    }
}

fn protocol_text_coordinate(line: u32, character: u32, byte_offset: Option<u64>) -> TextCoordinate {
    TextCoordinate {
        line,
        character,
        byte_offset,
        utf16_offset: None,
    }
}

fn empty_proposal_ledger_projection() -> ProposalLedgerProjection {
    ProposalLedgerProjection {
        rows: Vec::new(),
        selected_proposal_id: None,
        omitted_row_count: 0,
        generated_at: TimestampMillis(0),
        redaction_hints: Vec::new(),
        schema_version: 1,
    }
}

fn empty_context_manifest_projection() -> ContextManifestProjection {
    ContextManifestProjection {
        manifest: ContextManifestRecord {
            manifest_id: "manifest:empty".to_string(),
            workspace_id: None,
            proposal_id: None,
            purpose: ContextManifestPurpose::TrustReview,
            workspace_trust_state: None,
            privacy_label: ProposalPrivacyLabel::PublicMetadata,
            risk_label: ProposalRiskLabel::Informational,
            egress: ContextManifestEgressStatus::LocalOnly,
            items: Vec::new(),
            permissions: Vec::new(),
            omitted_item_count: 0,
            stale_or_missing_metadata_risk_present: false,
            generated_at: TimestampMillis(0),
            redaction_hints: vec![RedactionHint::MetadataOnly],
            schema_version: 1,
        },
        selected_item_id: None,
        generated_at: TimestampMillis(0),
        redaction_hints: vec![RedactionHint::MetadataOnly],
        schema_version: 1,
    }
}

fn empty_privacy_inspector_projection() -> PrivacyInspectorProjection {
    PrivacyInspectorProjection {
        inspector_id: "privacy:empty".to_string(),
        manifest_id: None,
        workspace_id: None,
        proposal_id: None,
        records: Vec::new(),
        denied_record_count: 0,
        redacted_record_count: 0,
        external_egress_record_count: 0,
        high_risk_record_count: 0,
        refusal: None,
        generated_at: TimestampMillis(0),
        redaction_hints: vec![RedactionHint::MetadataOnly],
        schema_version: 1,
    }
}

fn empty_permission_budget_projection() -> PermissionBudgetProjection {
    PermissionBudgetProjection {
        projection_id: "permission-budgets:empty".to_string(),
        budgets: Vec::new(),
        evaluations: Vec::new(),
        denied_budget_count: 0,
        depleted_budget_count: 0,
        refused_evaluation_count: 0,
        generated_at: TimestampMillis(0),
        redaction_hints: vec![RedactionHint::MetadataOnly],
        schema_version: 1,
    }
}

fn empty_approval_checklist_projection() -> ProposalApprovalChecklistProjection {
    ProposalApprovalChecklistProjection {
        checklist_id: "approval-checklist:empty".to_string(),
        proposal_id: ProposalId(0),
        workspace_id: None,
        payload_kind: devil_protocol::ProposalPayloadKind::SaveFile,
        lifecycle_state: devil_protocol::ProposalLifecycleState::Created,
        correlation_id: devil_protocol::CorrelationId(0),
        causality_id: None,
        ready_for_approval: false,
        gates: Vec::new(),
        blockers: Vec::new(),
        risk_labels: Vec::new(),
        privacy_labels: Vec::new(),
        explicit_denial_reasons: Vec::new(),
        generated_at: TimestampMillis(0),
        redaction_hints: vec![RedactionHint::MetadataOnly],
        schema_version: 1,
    }
}

fn empty_checkpoint_rollback_projection() -> CheckpointRollbackProjection {
    let preconditions = devil_protocol::ContextManifestPreconditionSummary::from_preconditions(
        &devil_protocol::ProposalVersionPreconditions {
            file_version: None,
            buffer_version: None,
            snapshot_id: None,
            generation: None,
            file_content_version: None,
            workspace_generation: None,
            expected_fingerprint: None,
            expected_file_length: None,
            expected_modified_at: None,
        },
        1,
    );
    CheckpointRollbackProjection {
        projection_id: "checkpoint-rollback:empty".to_string(),
        proposal_id: ProposalId(0),
        workspace_id: None,
        payload_kind: devil_protocol::ProposalPayloadKind::SaveFile,
        lifecycle_state: devil_protocol::ProposalLifecycleState::Created,
        correlation_id: devil_protocol::CorrelationId(0),
        causality_id: None,
        checkpoint: devil_protocol::ProposalCheckpointProjection {
            checkpoint_id: "checkpoint:empty".to_string(),
            available: false,
            target_count: 0,
            expected_preconditions: preconditions,
            hashes: Vec::new(),
            audit_status: devil_protocol::CheckpointRollbackAuditStatus::NotRequired,
            labels: Vec::new(),
            limitations: Vec::new(),
            redaction_hints: vec![RedactionHint::MetadataOnly],
            schema_version: 1,
        },
        rollback: devil_protocol::ProposalRollbackProjection {
            availability: devil_protocol::ProposalRollbackAvailability::NotRequired,
            rollback_step_count: 0,
            reversible_target_count: 0,
            irreversible_target_count: 0,
            audit_status: devil_protocol::CheckpointRollbackAuditStatus::NotRequired,
            labels: Vec::new(),
            limitations: Vec::new(),
            redaction_hints: vec![RedactionHint::MetadataOnly],
            schema_version: 1,
        },
        targets: Vec::new(),
        risk_labels: Vec::new(),
        privacy_labels: Vec::new(),
        generated_at: TimestampMillis(0),
        redaction_hints: vec![RedactionHint::MetadataOnly],
        schema_version: 1,
    }
}

fn empty_assisted_ai_projection() -> AssistedAiProjection {
    AssistedAiProjection {
        projection_id: "assisted-ai:empty".to_string(),
        providers: Vec::new(),
        routes: Vec::new(),
        requests: Vec::new(),
        refusals: Vec::new(),
        proposal_previews: Vec::new(),
        provider_count: 0,
        request_count: 0,
        refusal_count: 0,
        preview_ready_count: 0,
        provider_invocation: devil_protocol::AssistedAiProviderInvocationState::NotEncoded,
        generated_at: TimestampMillis(0),
        redaction_hints: vec![RedactionHint::MetadataOnly],
        schema_version: 1,
    }
}

fn empty_delegated_task_projection() -> DelegatedTaskProjection {
    DelegatedTaskProjection {
        projection_id: "delegated-task:empty".to_string(),
        plan_rows: Vec::new(),
        step_summaries: Vec::new(),
        blockers: Vec::new(),
        refusals: Vec::new(),
        required_approvals: Vec::new(),
        proposal_preview_links: Vec::new(),
        audit_readiness: Vec::new(),
        plan_only_disclaimers: vec!["delegated_task.plan_only_no_runtime".to_string()],
        plan_count: 0,
        blocked_plan_count: 0,
        refused_plan_count: 0,
        runtime_activation: DelegatedTaskRuntimeActivationState::NotEncoded,
        generated_at: TimestampMillis(0),
        redaction_hints: vec![RedactionHint::MetadataOnly],
        schema_version: 1,
    }
}

fn parse_proposal_id(payload: Option<&str>) -> Option<ProposalId> {
    payload
        .and_then(|value| value.trim().parse::<u64>().ok())
        .map(ProposalId)
}

#[cfg(test)]
mod tests {
    use super::*;
    use devil_protocol::{
        BufferId, BufferVersion, ByteRange, CanonicalPath, CapabilityId, FileFingerprint, FileId,
        LargeFileStatus, PermissionBudgetActionClass, PermissionBudgetConsentRequirementLabel,
        PermissionBudgetContract, PermissionBudgetResetPolicyLabel, PermissionBudgetState,
        PermissionBudgetUsageSummary, PrincipalId, ProposalContextManifestEntrySummary,
        ProposalContextManifestSummary, ProposalDiffChunkDescriptor, ProposalDiffSummary,
        ProposalDiffSummaryKind, ProposalLedgerRow, ProposalLifecycleState,
        ProposalLifecycleStateDisplay, ProposalPayloadKind, ProposalPrivacyLabel,
        ProposalRiskLabel, ProposalRollbackAvailability, ProposalTargetCoverage,
        ProposalTargetCoverageKind, ProtocolTextRange, RedactionHint, SnapshotId, Utf16Position,
        Utf16Range, ViewportDimensions, ViewportLineMetric, ViewportLineSlice,
        ViewportLineTruncationState, ViewportProjection, ViewportProjectionMode, ViewportScroll,
        WorkspaceId,
    };

    fn test_coordinate(line: u32, character: u32) -> TextCoordinate {
        TextCoordinate {
            line,
            character,
            byte_offset: Some(character as u64),
            utf16_offset: None,
        }
    }

    fn test_proposal_ledger_projection() -> ProposalLedgerProjection {
        ProposalLedgerProjection {
            rows: vec![ProposalLedgerRow {
                proposal_id: ProposalId(42),
                workspace_id: Some(WorkspaceId(1)),
                title: "bounded save preview".to_string(),
                payload_kind: ProposalPayloadKind::SaveFile,
                lifecycle: ProposalLifecycleStateDisplay {
                    state: ProposalLifecycleState::Previewed,
                    label: "Previewed".to_string(),
                    description: "ready for user review".to_string(),
                },
                principal: PrincipalId("trusted".to_string()),
                capability: CapabilityId("fs.write".to_string()),
                created_at: TimestampMillis(1),
                updated_at: TimestampMillis(2),
                expires_at: None,
                risk_label: ProposalRiskLabel::Low,
                privacy_label: ProposalPrivacyLabel::WorkspaceMetadata,
                rollback: ProposalRollbackAvailability::Available,
                target_coverage: ProposalTargetCoverage {
                    coverage_kind: ProposalTargetCoverageKind::Complete,
                    targets: Vec::new(),
                    omitted_target_count: 0,
                    redaction_hints: vec![RedactionHint::MetadataOnly],
                },
                context_manifest: ProposalContextManifestSummary {
                    manifest_id: "manifest:42".to_string(),
                    category_count: 1,
                    total_item_count: 1,
                    omitted_item_count: 0,
                    categories: vec![ProposalContextManifestEntrySummary {
                        category: "files".to_string(),
                        item_count: 1,
                        omitted_item_count: 0,
                        privacy_label: ProposalPrivacyLabel::WorkspaceMetadata,
                        manifest_hash: Some(FileFingerprint {
                            algorithm: "sha256".to_string(),
                            value: "ctx".to_string(),
                        }),
                        redaction_hints: vec![RedactionHint::MetadataOnly],
                    }],
                    redaction_hints: vec![RedactionHint::MetadataOnly],
                },
                diff_summary: ProposalDiffSummary {
                    kind: ProposalDiffSummaryKind::Text,
                    target_count: 1,
                    hunk_count: 1,
                    inserted_line_count: 2,
                    deleted_line_count: 1,
                    omitted_hunk_count: 99,
                    full_source_redacted: true,
                    diff_hash: Some(FileFingerprint {
                        algorithm: "sha256".to_string(),
                        value: "diff".to_string(),
                    }),
                    chunks: vec![ProposalDiffChunkDescriptor {
                        chunk_id: "chunk-0".to_string(),
                        target_id: None,
                        byte_range: Some(ByteRange::new(10, 20)),
                        changed_line_count: 3,
                        inserted_line_count: 2,
                        deleted_line_count: 1,
                        content_hash: Some(FileFingerprint {
                            algorithm: "blake3".to_string(),
                            value: "chunk".to_string(),
                        }),
                    }],
                    redaction_hints: vec![RedactionHint::MetadataOnly],
                },
                preview_warnings: Vec::new(),
                diagnostics: Vec::new(),
                redaction_hints: vec![RedactionHint::MetadataOnly],
                schema_version: 1,
            }],
            selected_proposal_id: Some(ProposalId(42)),
            omitted_row_count: 0,
            generated_at: TimestampMillis(3),
            redaction_hints: vec![RedactionHint::MetadataOnly],
            schema_version: 1,
        }
    }

    fn degraded_viewport_projection() -> ViewportProjection {
        ViewportProjection {
            workspace_id: WorkspaceId(1),
            buffer_id: BufferId(2),
            file_id: Some(FileId(9)),
            snapshot_id: SnapshotId(3),
            buffer_version: BufferVersion(4),
            visible_range: ProtocolTextRange {
                start: test_coordinate(10, 0),
                end: test_coordinate(12, 14),
            },
            selections: Vec::new(),
            cursor: test_coordinate(10, 0),
            scroll: ViewportScroll {
                top_line: 10,
                left_column: 0,
            },
            dimensions: ViewportDimensions {
                width_px: 800,
                height_px: 32,
            },
            mode: ViewportProjectionMode::DegradedLargeFile,
            line_slices: vec![
                ViewportLineSlice {
                    line_number: 10,
                    visible_text: "bounded-alpha".to_string(),
                    byte_range: ByteRange::new(1024, 1037),
                    utf16_range: Utf16Range {
                        start: Utf16Position {
                            line: 10,
                            character: 0,
                        },
                        end: Utf16Position {
                            line: 10,
                            character: 13,
                        },
                    },
                    chunk_hash: FileFingerprint {
                        algorithm: "sha256".to_string(),
                        value: "chunk-a".to_string(),
                    },
                    truncation_state: ViewportLineTruncationState::None,
                },
                ViewportLineSlice {
                    line_number: 11,
                    visible_text: "bounded-beta".to_string(),
                    byte_range: ByteRange::new(2048, 2060),
                    utf16_range: Utf16Range {
                        start: Utf16Position {
                            line: 11,
                            character: 0,
                        },
                        end: Utf16Position {
                            line: 11,
                            character: 12,
                        },
                    },
                    chunk_hash: FileFingerprint {
                        algorithm: "sha256".to_string(),
                        value: "chunk-b".to_string(),
                    },
                    truncation_state: ViewportLineTruncationState::Trailing,
                },
            ],
            line_metrics: vec![
                ViewportLineMetric {
                    byte_length: 13,
                    utf16_length: 13,
                    line_ending_width: 1,
                    exact: true,
                },
                ViewportLineMetric {
                    byte_length: 4096,
                    utf16_length: 4096,
                    line_ending_width: 1,
                    exact: true,
                },
            ],
            decoration_spans: Vec::new(),
            fold_ranges: Vec::new(),
            semantic_token_overlays: Vec::new(),
            large_file_status: Some(LargeFileStatus {
                threshold_bytes: 5 * 1024 * 1024,
                byte_len: 6 * 1024 * 1024,
                disabled_overlay_reasons: vec!["semantic token overlays deferred".to_string()],
                bounded_search_enabled: true,
                message: "Large file degraded mode: viewport payloads are chunked".to_string(),
            }),
            schema_version: 2,
        }
    }

    #[test]
    fn shell_parses_commands_into_dispatch_intents_without_editor_ownership() {
        let mut shell = Shell::new(ShellProjectionSnapshot {
            layout_projection: ShellLayoutProjection::plain("t"),
            explorer_projection: ExplorerProjection {
                nodes: Vec::new(),
                selection: None,
            },
            active_buffer_projection: ActiveBufferProjection {
                workspace_id: Some(WorkspaceId(1)),
                buffer_id: Some(BufferId(2)),
                file_id: Some(FileId(9)),
                file_path: Some(CanonicalPath("a.md".to_string())),
                viewport: None,
                degraded: false,
                small_buffer_preview: Some("first".to_string()),
                dirty: false,
            },
            status_messages: Vec::new(),
            proposal_ledger_projection: test_proposal_ledger_projection(),
            context_manifest_projection: empty_context_manifest_projection(),
            privacy_inspector_projection: empty_privacy_inspector_projection(),
            permission_budget_projection: empty_permission_budget_projection(),
            approval_checklist_projection: empty_approval_checklist_projection(),
            checkpoint_rollback_projection: empty_checkpoint_rollback_projection(),
            assisted_ai_projection: empty_assisted_ai_projection(),
            delegated_task_projection: empty_delegated_task_projection(),
        });

        let intent = shell
            .handle_command(":i \\n")
            .expect("insert command should parse")
            .expect("intent should be emitted");

        assert_eq!(
            intent,
            CommandDispatchIntent::Insert {
                buffer_id: BufferId(2),
                at: test_coordinate(0, 0),
                text: "\\n".to_string(),
            }
        );
        assert_eq!(
            shell.active_buffer_projection.small_buffer_text(),
            Some("first")
        );
        assert_eq!(shell.command_dispatch_intents.len(), 1);
    }

    #[test]
    fn shell_renders_proposal_ledger_from_static_snapshot() {
        let ledger = test_proposal_ledger_projection();
        let shell = Shell::new(ShellProjectionSnapshot {
            layout_projection: ShellLayoutProjection::plain("t"),
            explorer_projection: ExplorerProjection {
                nodes: Vec::new(),
                selection: None,
            },
            active_buffer_projection: ActiveBufferProjection::empty(),
            status_messages: Vec::new(),
            proposal_ledger_projection: ledger.clone(),
            context_manifest_projection: empty_context_manifest_projection(),
            privacy_inspector_projection: empty_privacy_inspector_projection(),
            permission_budget_projection: empty_permission_budget_projection(),
            approval_checklist_projection: empty_approval_checklist_projection(),
            checkpoint_rollback_projection: empty_checkpoint_rollback_projection(),
            assisted_ai_projection: empty_assisted_ai_projection(),
            delegated_task_projection: empty_delegated_task_projection(),
        });

        let snapshot = shell.projection_snapshot();
        assert_eq!(snapshot.proposal_ledger_projection, ledger);
        assert_eq!(
            snapshot.proposal_ledger_projection.rows[0].proposal_id,
            ProposalId(42)
        );
        assert!(
            snapshot.proposal_ledger_projection.rows[0]
                .diff_summary
                .full_source_redacted
        );
    }

    #[test]
    fn shell_snapshot_large_file_projection_carries_only_viewport_slices() {
        let large_source_len = 6 * 1024 * 1024;
        let shell = Shell::new(ShellProjectionSnapshot {
            layout_projection: ShellLayoutProjection::plain("large"),
            explorer_projection: ExplorerProjection {
                nodes: Vec::new(),
                selection: None,
            },
            active_buffer_projection: ActiveBufferProjection {
                workspace_id: Some(WorkspaceId(1)),
                buffer_id: Some(BufferId(2)),
                file_id: Some(FileId(9)),
                file_path: Some(CanonicalPath("large.txt".to_string())),
                viewport: Some(degraded_viewport_projection()),
                degraded: true,
                small_buffer_preview: None,
                dirty: false,
            },
            status_messages: Vec::new(),
            proposal_ledger_projection: test_proposal_ledger_projection(),
            context_manifest_projection: empty_context_manifest_projection(),
            privacy_inspector_projection: empty_privacy_inspector_projection(),
            permission_budget_projection: empty_permission_budget_projection(),
            approval_checklist_projection: empty_approval_checklist_projection(),
            checkpoint_rollback_projection: empty_checkpoint_rollback_projection(),
            assisted_ai_projection: empty_assisted_ai_projection(),
            delegated_task_projection: empty_delegated_task_projection(),
        });

        let snapshot = shell.projection_snapshot();
        let active = snapshot.active_buffer_projection;
        let viewport = active.viewport.as_ref().expect("viewport projection");
        let payload_bytes = viewport
            .line_slices
            .iter()
            .map(|slice| slice.visible_text.len())
            .sum::<usize>();

        assert!(active.degraded);
        assert!(active.small_buffer_text().is_none());
        assert_eq!(viewport.mode, ViewportProjectionMode::DegradedLargeFile);
        assert!(viewport.large_file_status.is_some());
        assert!(payload_bytes < large_source_len / 1000);
        assert!(
            viewport
                .line_slices
                .iter()
                .all(|slice| slice.visible_text.len() < large_source_len)
        );
    }

    #[test]
    fn shell_proposal_intents_do_not_mutate_editor_or_workspace_projection() {
        let mut shell = Shell::new(ShellProjectionSnapshot {
            layout_projection: ShellLayoutProjection::plain("t"),
            explorer_projection: ExplorerProjection {
                nodes: Vec::new(),
                selection: None,
            },
            active_buffer_projection: ActiveBufferProjection {
                workspace_id: Some(WorkspaceId(1)),
                buffer_id: Some(BufferId(2)),
                file_id: Some(FileId(9)),
                file_path: Some(CanonicalPath("a.md".to_string())),
                viewport: None,
                degraded: false,
                small_buffer_preview: Some("first".to_string()),
                dirty: false,
            },
            status_messages: Vec::new(),
            proposal_ledger_projection: test_proposal_ledger_projection(),
            context_manifest_projection: empty_context_manifest_projection(),
            privacy_inspector_projection: empty_privacy_inspector_projection(),
            permission_budget_projection: empty_permission_budget_projection(),
            approval_checklist_projection: empty_approval_checklist_projection(),
            checkpoint_rollback_projection: empty_checkpoint_rollback_projection(),
            assisted_ai_projection: empty_assisted_ai_projection(),
            delegated_task_projection: empty_delegated_task_projection(),
        });

        let before = shell.projection_snapshot();
        let intent = shell
            .handle_command(":proposal-approve 42")
            .expect("proposal command should parse")
            .expect("intent should be emitted");

        assert_eq!(
            intent,
            CommandDispatchIntent::ApproveProposal {
                proposal_id: ProposalId(42)
            }
        );
        assert_eq!(shell.projection_snapshot(), before);
        assert_eq!(shell.command_dispatch_intents.len(), 1);
    }

    #[test]
    fn shell_renders_context_manifest_from_static_snapshot_without_ownership() {
        let mut manifest = empty_context_manifest_projection();
        manifest.manifest.manifest_id = "manifest:trust-review".to_string();
        manifest.manifest.risk_label = ProposalRiskLabel::Medium;
        manifest.manifest.privacy_label = ProposalPrivacyLabel::WorkspaceMetadata;
        manifest
            .manifest
            .items
            .push(devil_protocol::ContextManifestItem {
                item_id: "semantic-job:0".to_string(),
                kind: devil_protocol::ContextManifestItemKind::SemanticFabricJob,
                inclusion: devil_protocol::ContextManifestInclusionState::Included,
                workspace_id: Some(WorkspaceId(1)),
                file_id: Some(FileId(9)),
                buffer_id: Some(BufferId(2)),
                proposal_id: Some(ProposalId(42)),
                target_id: Some("target-buffer-main".to_string()),
                path: Some(CanonicalPath("C:/repo/src/main.rs".to_string())),
                ranges: vec![ByteRange::new(10, 20)],
                counts: vec![devil_protocol::ContextManifestItemCount {
                    label: "diagnostics".to_string(),
                    count: 2,
                }],
                hashes: vec![FileFingerprint {
                    algorithm: "sha256".to_string(),
                    value: "content".to_string(),
                }],
                privacy_scope: Some(devil_protocol::SemanticPrivacyScope::Workspace),
                privacy_label: ProposalPrivacyLabel::WorkspaceMetadata,
                risk_label: ProposalRiskLabel::Medium,
                egress: devil_protocol::ContextManifestEgressStatus::LocalOnly,
                freshness: None,
                preconditions: None,
                labels: vec!["semantic.fabric.metadata".to_string()],
                redaction_hints: vec![RedactionHint::MetadataOnly],
                schema_version: 1,
            });

        let shell = Shell::new(ShellProjectionSnapshot {
            layout_projection: ShellLayoutProjection::plain("trust"),
            explorer_projection: ExplorerProjection {
                nodes: Vec::new(),
                selection: None,
            },
            active_buffer_projection: ActiveBufferProjection::empty(),
            status_messages: Vec::new(),
            proposal_ledger_projection: test_proposal_ledger_projection(),
            context_manifest_projection: manifest.clone(),
            privacy_inspector_projection: empty_privacy_inspector_projection(),
            permission_budget_projection: empty_permission_budget_projection(),
            approval_checklist_projection: empty_approval_checklist_projection(),
            checkpoint_rollback_projection: empty_checkpoint_rollback_projection(),
            assisted_ai_projection: empty_assisted_ai_projection(),
            delegated_task_projection: empty_delegated_task_projection(),
        });

        let snapshot = shell.projection_snapshot();
        assert_eq!(snapshot.context_manifest_projection, manifest);
        assert_eq!(snapshot.context_manifest_projection.manifest.items.len(), 1);
        assert!(shell.command_dispatch_intents.is_empty());
    }

    #[test]
    fn shell_renders_privacy_and_budget_summaries_from_static_snapshot_without_ownership() {
        let mut privacy = empty_privacy_inspector_projection();
        privacy.inspector_id = "privacy:trust".to_string();
        privacy.records = vec![devil_protocol::PrivacyInspectorExposureRecord {
            exposure_id: "exposure:semantic".to_string(),
            source_kind: devil_protocol::PrivacyInspectorSourceKind::SemanticMetadata,
            context_item_id: Some("semantic:0".to_string()),
            proposal_id: Some(ProposalId(42)),
            target_id: Some("target-0".to_string()),
            workspace_id: Some(WorkspaceId(1)),
            file_id: Some(FileId(9)),
            buffer_id: Some(BufferId(2)),
            privacy_scope: Some(devil_protocol::SemanticPrivacyScope::Workspace),
            privacy_label: ProposalPrivacyLabel::WorkspaceMetadata,
            redaction_state: devil_protocol::PrivacyInspectorRedactionState::MetadataOnly,
            inclusion: devil_protocol::ContextManifestInclusionState::Included,
            egress: devil_protocol::ContextManifestEgressStatus::LocalOnly,
            risk_label: ProposalRiskLabel::Low,
            permission_label: Some(CapabilityId("semantic.read".to_string())),
            ranges: vec![ByteRange::new(10, 20)],
            counts: Vec::new(),
            hashes: vec![FileFingerprint {
                algorithm: "sha256".to_string(),
                value: "metadata-hash".to_string(),
            }],
            labels: vec!["semantic.metadata".to_string()],
            reasons: vec!["context.included".to_string()],
            redaction_hints: vec![RedactionHint::MetadataOnly],
            schema_version: 1,
        }];

        let mut budgets = empty_permission_budget_projection();
        budgets.projection_id = "budgets:trust".to_string();
        budgets.budgets = vec![PermissionBudgetContract {
            budget_id: "budget:semantic".to_string(),
            action_class: PermissionBudgetActionClass::ReadSemanticMetadata,
            capability: Some(CapabilityId("semantic.read".to_string())),
            state: PermissionBudgetState::Allowed,
            privacy_scope: devil_protocol::SemanticPrivacyScope::MetadataOnly,
            usage: PermissionBudgetUsageSummary {
                unit_label: "items".to_string(),
                used: 1,
                ceiling: Some(10),
                remaining: Some(9),
                attempted: 0,
                redaction_hints: vec![RedactionHint::MetadataOnly],
                schema_version: 1,
            },
            reset_policy_label: PermissionBudgetResetPolicyLabel::Session,
            consent_requirement_label: PermissionBudgetConsentRequirementLabel::NotRequired,
            risk_label: ProposalRiskLabel::Low,
            reasons: vec!["budget.seeded".to_string()],
            redaction_hints: vec![RedactionHint::MetadataOnly],
            schema_version: 1,
        }];

        let shell = Shell::new(ShellProjectionSnapshot {
            layout_projection: ShellLayoutProjection::plain("trust"),
            explorer_projection: ExplorerProjection {
                nodes: Vec::new(),
                selection: None,
            },
            active_buffer_projection: ActiveBufferProjection::empty(),
            status_messages: Vec::new(),
            proposal_ledger_projection: test_proposal_ledger_projection(),
            context_manifest_projection: empty_context_manifest_projection(),
            privacy_inspector_projection: privacy.clone(),
            permission_budget_projection: budgets.clone(),
            approval_checklist_projection: empty_approval_checklist_projection(),
            checkpoint_rollback_projection: empty_checkpoint_rollback_projection(),
            assisted_ai_projection: empty_assisted_ai_projection(),
            delegated_task_projection: empty_delegated_task_projection(),
        });

        let snapshot = shell.projection_snapshot();
        assert_eq!(snapshot.privacy_inspector_projection, privacy);
        assert_eq!(snapshot.permission_budget_projection, budgets);
        assert!(shell.command_dispatch_intents.is_empty());
    }

    #[test]
    fn shell_renders_approval_and_rollback_summaries_from_static_snapshot_without_ownership() {
        let mut checklist = empty_approval_checklist_projection();
        checklist.checklist_id = "approval-checklist:42".to_string();
        checklist.proposal_id = ProposalId(42);
        checklist.ready_for_approval = true;
        checklist.gates = vec![devil_protocol::ApprovalChecklistGateSummary {
            gate: devil_protocol::ApprovalChecklistGateKind::AuditBeforeSuccess,
            status: devil_protocol::ApprovalChecklistGateStatus::Satisfied,
            risk_label: ProposalRiskLabel::Low,
            privacy_label: ProposalPrivacyLabel::WorkspaceMetadata,
            labels: vec!["audit.metadata_only".to_string()],
            reasons: Vec::new(),
            redaction_hints: vec![RedactionHint::MetadataOnly],
            schema_version: 1,
        }];

        let mut rollback = empty_checkpoint_rollback_projection();
        rollback.projection_id = "checkpoint-rollback:42".to_string();
        rollback.proposal_id = ProposalId(42);
        rollback.checkpoint.available = true;
        rollback.rollback.availability = devil_protocol::ProposalRollbackAvailability::Available;
        rollback.targets = vec![devil_protocol::CheckpointRollbackTargetSummary {
            target_id: "target-buffer-main".to_string(),
            kind: devil_protocol::ProposalTargetKind::OpenBuffer,
            workspace_id: Some(WorkspaceId(1)),
            file_id: Some(FileId(9)),
            buffer_id: Some(BufferId(2)),
            terminal_session_id: None,
            plugin_id: None,
            ranges: vec![ByteRange::new(10, 20)],
            hashes: vec![FileFingerprint {
                algorithm: "sha256".to_string(),
                value: "expected".to_string(),
            }],
            expected_file_content_version: Some(devil_protocol::FileContentVersion(44)),
            expected_buffer_version: Some(BufferVersion(55)),
            expected_snapshot_id: Some(SnapshotId(66)),
            expected_workspace_generation: Some(devil_protocol::WorkspaceGeneration(77)),
            labels: vec!["target.kind.OpenBuffer".to_string()],
            redaction_hints: vec![RedactionHint::MetadataOnly],
            schema_version: 1,
        }];

        let shell = Shell::new(ShellProjectionSnapshot {
            layout_projection: ShellLayoutProjection::plain("trust"),
            explorer_projection: ExplorerProjection {
                nodes: Vec::new(),
                selection: None,
            },
            active_buffer_projection: ActiveBufferProjection::empty(),
            status_messages: Vec::new(),
            proposal_ledger_projection: test_proposal_ledger_projection(),
            context_manifest_projection: empty_context_manifest_projection(),
            privacy_inspector_projection: empty_privacy_inspector_projection(),
            permission_budget_projection: empty_permission_budget_projection(),
            approval_checklist_projection: checklist.clone(),
            checkpoint_rollback_projection: rollback.clone(),
            assisted_ai_projection: empty_assisted_ai_projection(),
            delegated_task_projection: empty_delegated_task_projection(),
        });

        let snapshot = shell.projection_snapshot();
        assert_eq!(snapshot.approval_checklist_projection, checklist);
        assert_eq!(snapshot.checkpoint_rollback_projection, rollback);
        assert!(snapshot.approval_checklist_projection.ready_for_approval);
        assert!(shell.command_dispatch_intents.is_empty());
    }

    #[test]
    fn shell_renders_assisted_ai_projection_from_static_snapshot_without_ownership() {
        let mut assisted = empty_assisted_ai_projection();
        assisted.projection_id = "assisted-ai:p6-2".to_string();
        assisted.provider_count = 1;
        assisted.request_count = 1;
        assisted.preview_ready_count = 1;
        assisted.providers = vec![devil_protocol::AssistedAiProviderCapabilitySummary {
            provider_id: "provider:local-redacted".to_string(),
            provider_label: "Local metadata provider".to_string(),
            provider_class: devil_protocol::AssistedAiProviderClass::Local,
            supported_operations: vec![devil_protocol::AssistedAiOperationClass::ProposeEdit],
            supported_operation_count: 1,
            model_capability_label_count: 1,
            tool_capability_label_count: 0,
            context_window_label: "bounded".to_string(),
            cost_budget_label: "capped".to_string(),
            risk_budget_label: "review-required".to_string(),
            privacy_retention_label: "metadata-only".to_string(),
            availability: devil_protocol::AssistedAiProviderAvailabilityState::Available,
            refusal: None,
            risk_label: ProposalRiskLabel::Low,
            privacy_label: ProposalPrivacyLabel::WorkspaceMetadata,
            redaction_hints: vec![RedactionHint::MetadataOnly],
            schema_version: 1,
        }];
        assisted.proposal_previews = vec![devil_protocol::AssistedAiProposalPreviewSummary {
            preview_id: "assist:preview:42".to_string(),
            output_id: "assist:output:42".to_string(),
            request_id: "assist:req:42".to_string(),
            provider_id: "provider:local-redacted".to_string(),
            proposal_id: ProposalId(42),
            payload_kind: ProposalPayloadKind::TextEdit,
            lifecycle_state: ProposalLifecycleState::Previewed,
            readiness: devil_protocol::AssistedAiProposalPreviewReadiness::PreviewReady,
            ready_for_preview: true,
            ready_for_approval: true,
            ready_for_apply: false,
            correlation_id: devil_protocol::CorrelationId(901),
            causality_id: devil_protocol::CausalityId(
                uuid::Uuid::parse_str("cccccccc-cccc-cccc-cccc-cccccccccccc").unwrap(),
            ),
            context_manifest: devil_protocol::AssistedAiTrustProjectionReference {
                reference_id: "manifest:p5:context".to_string(),
                kind: devil_protocol::AssistedAiTrustProjectionKind::ContextManifest,
                projection_hash: FileFingerprint {
                    algorithm: "sha256".to_string(),
                    value: "manifest".to_string(),
                },
                schema_version: 1,
            },
            approval_checklist: devil_protocol::AssistedAiTrustProjectionReference {
                reference_id: "checklist:p5:approval".to_string(),
                kind: devil_protocol::AssistedAiTrustProjectionKind::ProposalApprovalChecklist,
                projection_hash: FileFingerprint {
                    algorithm: "sha256".to_string(),
                    value: "checklist".to_string(),
                },
                schema_version: 1,
            },
            checkpoint_rollback: None,
            preconditions: devil_protocol::ContextManifestPreconditionSummary::from_preconditions(
                &devil_protocol::ProposalVersionPreconditions {
                    file_version: Some(devil_protocol::FileContentVersion(44)),
                    buffer_version: Some(BufferVersion(55)),
                    snapshot_id: Some(SnapshotId(66)),
                    generation: Some(devil_protocol::WorkspaceGeneration(77)),
                    file_content_version: Some(devil_protocol::FileContentVersion(44)),
                    workspace_generation: Some(devil_protocol::WorkspaceGeneration(77)),
                    expected_fingerprint: Some(FileFingerprint {
                        algorithm: "sha256".to_string(),
                        value: "expected".to_string(),
                    }),
                    expected_file_length: Some(1234),
                    expected_modified_at: Some(TimestampMillis(9876)),
                },
                1,
            ),
            target_coverage: ProposalTargetCoverage {
                coverage_kind: ProposalTargetCoverageKind::Complete,
                targets: Vec::new(),
                omitted_target_count: 0,
                redaction_hints: vec![RedactionHint::MetadataOnly],
            },
            diff_summary: ProposalDiffSummary {
                kind: ProposalDiffSummaryKind::Text,
                target_count: 1,
                hunk_count: 1,
                inserted_line_count: 0,
                deleted_line_count: 0,
                omitted_hunk_count: 0,
                full_source_redacted: true,
                diff_hash: None,
                chunks: Vec::new(),
                redaction_hints: vec![RedactionHint::MetadataOnly],
            },
            trust_projection_references: Vec::new(),
            ledger_row_present: true,
            preview_warning_count: 0,
            refusal: None,
            risk_label: ProposalRiskLabel::Low,
            privacy_label: ProposalPrivacyLabel::WorkspaceMetadata,
            labels: vec!["proposal.apply.not_encoded".to_string()],
            redaction_hints: vec![RedactionHint::MetadataOnly],
            schema_version: 1,
        }];

        let shell = Shell::new(ShellProjectionSnapshot {
            layout_projection: ShellLayoutProjection::plain("assisted"),
            explorer_projection: ExplorerProjection {
                nodes: Vec::new(),
                selection: None,
            },
            active_buffer_projection: ActiveBufferProjection::empty(),
            status_messages: Vec::new(),
            proposal_ledger_projection: test_proposal_ledger_projection(),
            context_manifest_projection: empty_context_manifest_projection(),
            privacy_inspector_projection: empty_privacy_inspector_projection(),
            permission_budget_projection: empty_permission_budget_projection(),
            approval_checklist_projection: empty_approval_checklist_projection(),
            checkpoint_rollback_projection: empty_checkpoint_rollback_projection(),
            assisted_ai_projection: assisted.clone(),
            delegated_task_projection: empty_delegated_task_projection(),
        });

        let snapshot = shell.projection_snapshot();
        assert_eq!(snapshot.assisted_ai_projection, assisted);
        assert_eq!(
            snapshot.assisted_ai_projection.provider_invocation,
            devil_protocol::AssistedAiProviderInvocationState::NotEncoded
        );
        assert!(snapshot.assisted_ai_projection.proposal_previews[0].ready_for_preview);
        assert!(!snapshot.assisted_ai_projection.proposal_previews[0].ready_for_apply);
        assert!(shell.command_dispatch_intents.is_empty());
    }

    #[test]
    fn shell_renders_delegated_task_projection_from_static_snapshot_without_ownership() {
        let mut delegated = empty_delegated_task_projection();
        delegated.projection_id = "delegated-task:p7-1".to_string();
        delegated.plan_count = 1;
        delegated.plan_rows = vec![devil_protocol::DelegatedTaskPlanRow {
            plan_id: devil_protocol::DelegatedTaskPlanId("plan:p7-1".to_string()),
            workspace_id: Some(WorkspaceId(1)),
            objective_summary_hash: FileFingerprint {
                algorithm: "sha256".to_string(),
                value: "objective".to_string(),
            },
            plan_state: devil_protocol::DelegatedTaskPlanState::AwaitingApproval,
            readiness: devil_protocol::DelegatedTaskPlanReadinessStatus::PlanReady,
            step_count: 1,
            affected_target_count: 1,
            blocker_count: 0,
            refusal_count: 0,
            proposal_preview_link_count: 1,
            risk_label: ProposalRiskLabel::Medium,
            privacy_label: ProposalPrivacyLabel::WorkspaceMetadata,
            correlation_id: devil_protocol::CorrelationId(901),
            causality_id: devil_protocol::CausalityId(
                uuid::Uuid::parse_str("cccccccc-cccc-cccc-cccc-cccccccccccc").unwrap(),
            ),
            runtime_activation: devil_protocol::DelegatedTaskRuntimeActivationState::NotEncoded,
            labels: vec!["delegated_task.plan_row.metadata_only".to_string()],
            redaction_hints: vec![RedactionHint::MetadataOnly],
            schema_version: 1,
        }];
        delegated.step_summaries = vec![devil_protocol::DelegatedTaskStepSummary {
            step_id: devil_protocol::DelegatedTaskStepId("step:preview".to_string()),
            plan_id: devil_protocol::DelegatedTaskPlanId("plan:p7-1".to_string()),
            order: 1,
            objective_summary_hash: FileFingerprint {
                algorithm: "sha256".to_string(),
                value: "step".to_string(),
            },
            operation_class: devil_protocol::DelegatedTaskOperationClass::LinkProposalPreview,
            state: devil_protocol::DelegatedTaskStepState::ProposalPreviewLinked,
            dependency_count: 0,
            target_count: 1,
            proposal_id: Some(ProposalId(42)),
            blocker_count: 0,
            risk_label: ProposalRiskLabel::Medium,
            privacy_label: ProposalPrivacyLabel::WorkspaceMetadata,
            labels: vec!["proposal-preview-link-only".to_string()],
            redaction_hints: vec![RedactionHint::MetadataOnly],
            schema_version: 1,
        }];

        let shell = Shell::new(ShellProjectionSnapshot {
            layout_projection: ShellLayoutProjection::plain("delegated"),
            explorer_projection: ExplorerProjection {
                nodes: Vec::new(),
                selection: None,
            },
            active_buffer_projection: ActiveBufferProjection::empty(),
            status_messages: Vec::new(),
            proposal_ledger_projection: test_proposal_ledger_projection(),
            context_manifest_projection: empty_context_manifest_projection(),
            privacy_inspector_projection: empty_privacy_inspector_projection(),
            permission_budget_projection: empty_permission_budget_projection(),
            approval_checklist_projection: empty_approval_checklist_projection(),
            checkpoint_rollback_projection: empty_checkpoint_rollback_projection(),
            assisted_ai_projection: empty_assisted_ai_projection(),
            delegated_task_projection: delegated.clone(),
        });

        let snapshot = shell.projection_snapshot();
        assert_eq!(snapshot.delegated_task_projection, delegated);
        assert_eq!(
            snapshot.delegated_task_projection.runtime_activation,
            devil_protocol::DelegatedTaskRuntimeActivationState::NotEncoded
        );
        assert_eq!(
            snapshot.delegated_task_projection.step_summaries[0].proposal_id,
            Some(ProposalId(42))
        );
        assert!(shell.command_dispatch_intents.is_empty());
    }

    #[test]
    fn explorer_projection_holds_nodes_and_selection() {
        let projection = ExplorerProjection {
            nodes: vec![ExplorerNodeProjection {
                file_id: FileId(10),
                canonical_path: CanonicalPath("C:/repo/src/main.rs".to_string()),
                name: "main.rs".to_string(),
                children: vec![],
            }],
            selection: Some(ExplorerSelectionProjection {
                file_id: FileId(10),
            }),
        };

        assert_eq!(projection.nodes.len(), 1);
        assert_eq!(projection.nodes[0].name, "main.rs");
        assert_eq!(
            projection.selection.map(|sel| sel.file_id),
            Some(FileId(10))
        );
    }
}
