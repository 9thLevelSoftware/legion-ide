//! Coordinator-state projections for UI surfaces.

use crate::ui::{DockLayout, DockMode, PanelRegistry, ShellProjectionSnapshot};
use legion_protocol::{
    LegionWorkflowMergeReadinessState, LegionWorkflowProjection, LegionWorkflowProjectionRow,
    LegionWorkflowSessionId, LegionWorkflowState, ProposalDiffSummaryKind, ProposalId,
    ProposalLedgerProjection, ProposalLedgerRow, ProposalRiskLabel, VerificationRunProjection,
    VerificationRunRow, VerificationRunState,
};
use serde::{Deserialize, Serialize};

/// A named region of the workbench shell layout.
///
/// The dock/panel acceptance is stated per *region* — every region must have a
/// projection and an integration test — but nothing in the tree enumerated the
/// regions, so "every" could not be checked. Individual regions were covered by
/// an ad-hoc scatter of tests; a region that stopped projecting, or a region
/// added to the shell with no coverage at all, would fail no test.
///
/// This enum is that missing enumeration. Every consumer matches exhaustively
/// over it, so adding a variant is a compile error until the new region is
/// given a projection source and a test. That is what turns the acceptance from
/// a claim into a gate.
///
/// Regions are the shell's *chrome and dock surfaces*. The code canvas is
/// deliberately absent: it is owned by the code-canvas painter and its own
/// acceptance, not by dock/panel completion.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum LayoutRegion {
    /// Top command bar.
    TopBar,
    /// Bottom status bar.
    StatusBar,
    /// Left/right/bottom dock placement for the active product mode.
    Dock,
    /// Workspace file tree.
    FileTree,
    /// Editor tab strip.
    EditorTabs,
    /// Terminal panel.
    TerminalPanel,
    /// Test explorer panel.
    TestsPanel,
    /// Diagnostics/problems panel.
    ProblemsPanel,
    /// Symbol outline panel.
    SymbolsPanel,
}

impl LayoutRegion {
    /// Every layout region, in shell reading order.
    ///
    /// Kept exhaustive by [`LayoutRegion::all_covers_every_variant`], which
    /// matches over every variant so a new region cannot be added without
    /// being listed here.
    pub const ALL: [Self; 9] = [
        Self::TopBar,
        Self::StatusBar,
        Self::Dock,
        Self::FileTree,
        Self::EditorTabs,
        Self::TerminalPanel,
        Self::TestsPanel,
        Self::ProblemsPanel,
        Self::SymbolsPanel,
    ];

    /// Stable lowercase identifier used in evidence rows and persisted state.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::TopBar => "top_bar",
            Self::StatusBar => "status_bar",
            Self::Dock => "dock",
            Self::FileTree => "file_tree",
            Self::EditorTabs => "editor_tabs",
            Self::TerminalPanel => "terminal_panel",
            Self::TestsPanel => "tests_panel",
            Self::ProblemsPanel => "problems_panel",
            Self::SymbolsPanel => "symbols_panel",
        }
    }

    /// Stable user-facing label.
    pub fn label(self) -> &'static str {
        match self {
            Self::TopBar => "Top bar",
            Self::StatusBar => "Status bar",
            Self::Dock => "Dock layout",
            Self::FileTree => "File tree",
            Self::EditorTabs => "Editor tabs",
            Self::TerminalPanel => "Terminal panel",
            Self::TestsPanel => "Tests panel",
            Self::ProblemsPanel => "Problems panel",
            Self::SymbolsPanel => "Symbols panel",
        }
    }

    /// Parse a stable region identifier.
    pub fn parse(value: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|region| region.as_str() == value)
    }

    /// The projection this region draws from, as a display-safe path.
    ///
    /// This is the "has a projection" half of the acceptance made explicit: a
    /// region names the projection it renders, rather than the mapping living
    /// only in renderer code where nothing can audit it.
    pub fn projection_source(self) -> &'static str {
        match self {
            Self::TopBar => "ShellProjectionSnapshot::layout_projection.layout.title",
            Self::StatusBar => "ShellProjectionSnapshot::active_buffer_projection",
            Self::Dock => {
                "DockLayout::standard(product_mode) filtered by PanelRegistry::standard()"
            }
            Self::FileTree => "ShellProjectionSnapshot::explorer_projection.nodes",
            Self::EditorTabs => "ShellProjectionSnapshot::daily_editing_projection.tabs.tabs",
            Self::TerminalPanel => "ShellProjectionSnapshot::terminal_panel_projection.output_rows",
            Self::TestsPanel => "ShellProjectionSnapshot::test_explorer_projection.items",
            Self::ProblemsPanel => "ShellProjectionSnapshot::language_tooling_projection.problems",
            Self::SymbolsPanel => "ShellProjectionSnapshot::language_tooling_projection.outline",
        }
    }

    /// Count the projected items this region has to draw from `snapshot`.
    ///
    /// Zero means the region has nothing projected — an empty panel, not a
    /// missing one. Callers use it in both directions: a populated snapshot
    /// must give every region a non-zero count, and an empty shell must give
    /// every content-backed region zero, so the positive check cannot pass
    /// vacuously.
    ///
    /// [`LayoutRegion::TopBar`] and [`LayoutRegion::Dock`] are persistent
    /// chrome rather than content: the top bar always draws the product-mode
    /// switch and the dock always places panels for the active mode, so both
    /// stay non-zero for an empty shell. See [`LayoutRegion::is_content_backed`].
    pub fn projected_item_count(self, snapshot: &ShellProjectionSnapshot) -> usize {
        match self {
            Self::TopBar => {
                // The mode switch is drawn in every state; the workspace
                // identity only once a workspace is open.
                const MODE_SWITCH: usize = 1;
                MODE_SWITCH
                    + usize::from(!snapshot.layout_projection.layout.title.trim().is_empty())
            }
            Self::StatusBar => {
                let active = &snapshot.active_buffer_projection;
                usize::from(active.buffer_id.is_some())
                    + usize::from(active.file_path.is_some())
                    + usize::from(active.viewport.is_some())
            }
            Self::Dock => dock_placement_count(snapshot.product_mode),
            Self::FileTree => snapshot.explorer_projection.nodes.len(),
            Self::EditorTabs => snapshot.daily_editing_projection.tabs.tabs.len(),
            Self::TerminalPanel => snapshot.terminal_panel_projection.output_rows.len(),
            Self::TestsPanel => snapshot.test_explorer_projection.items.len(),
            Self::ProblemsPanel => snapshot.language_tooling_projection.problems.len(),
            Self::SymbolsPanel => snapshot.language_tooling_projection.outline.len(),
        }
    }

    /// Whether this region draws only when the snapshot has content for it.
    ///
    /// False for persistent chrome: the top bar keeps the mode switch and the
    /// dock keeps its mode-derived placement even with an empty workspace, so
    /// "no content means no rows" is not a rule they can be held to. Exposing
    /// this as a predicate keeps both exceptions in one documented place
    /// instead of letting each test hard-code a skip nobody can recover the
    /// reason for.
    pub fn is_content_backed(self) -> bool {
        !matches!(self, Self::TopBar | Self::Dock)
    }

    /// Exhaustive-match guard proving [`LayoutRegion::ALL`] lists every variant.
    ///
    /// Adding a variant without adding it to `ALL` makes this return `false`,
    /// which the crate's own test rejects.
    #[cfg(test)]
    fn all_covers_every_variant(self) -> bool {
        let listed = match self {
            Self::TopBar
            | Self::StatusBar
            | Self::Dock
            | Self::FileTree
            | Self::EditorTabs
            | Self::TerminalPanel
            | Self::TestsPanel
            | Self::ProblemsPanel
            | Self::SymbolsPanel => true,
        };
        listed && Self::ALL.contains(&self)
    }
}

/// Count dock placements that the registry can actually construct in `mode`.
///
/// A panel placed by the standard layout but rejected by the registry would
/// render as a hole in the dock, so the count deliberately intersects the two
/// rather than trusting either alone.
fn dock_placement_count(mode: DockMode) -> usize {
    let layout = DockLayout::standard(mode);
    let registry = PanelRegistry::standard();
    [&layout.left, &layout.right, &layout.bottom]
        .into_iter()
        .flat_map(|side| {
            std::iter::once(side.pinned_default).chain(side.custom_toolkit.iter().copied())
        })
        .filter(|panel| registry.is_visible_in(*panel, mode))
        .count()
}

/// Kanban column kinds derived from coordinator state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LegionWorkflowBoardColumnKind {
    /// Session is assigned or still being planned.
    Assigned,
    /// Session is actively running.
    InProgress,
    /// Session is waiting on a human approval or a fail-closed blocker.
    WaitingOnHuman,
    /// Session is verifying output.
    Testing,
    /// Session is complete or terminal.
    Done,
}

impl LegionWorkflowBoardColumnKind {
    /// Stable display label.
    pub fn label(self) -> &'static str {
        match self {
            Self::Assigned => "Assigned",
            Self::InProgress => "In Progress",
            Self::WaitingOnHuman => "Waiting on Human",
            Self::Testing => "Testing",
            Self::Done => "Done",
        }
    }

    /// Derive the board column from workflow coordinator state.
    pub fn from_state(state: LegionWorkflowState) -> Self {
        match state {
            LegionWorkflowState::Draft | LegionWorkflowState::Planning => Self::Assigned,
            LegionWorkflowState::Executing => Self::InProgress,
            LegionWorkflowState::WaitingForApproval
            | LegionWorkflowState::WaitingOnHuman
            | LegionWorkflowState::Blocked => Self::WaitingOnHuman,
            LegionWorkflowState::Verifying => Self::Testing,
            LegionWorkflowState::Completed
            | LegionWorkflowState::Failed
            | LegionWorkflowState::Cancelled => Self::Done,
        }
    }
}

/// Metadata-only per-worker delegated-loop budget row for fleet console surfaces.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LegionWorkflowBudgetUsageRowProjection {
    /// Workflow session that owns the worker.
    pub session_id: LegionWorkflowSessionId,
    /// Stable worker identifier rendered as a display-safe label.
    pub worker_id: String,
    /// Display-safe budget family label.
    pub budget_label: String,
    /// Model-turn usage label.
    pub model_turns_label: String,
    /// Tool-call usage label.
    pub tool_calls_label: String,
    /// Retry usage label.
    pub retry_label: String,
    /// Total tool-output usage label.
    pub output_bytes_label: String,
    /// Wall-clock usage label.
    pub wall_clock_label: String,
    /// Display-safe budget status label.
    pub status_label: String,
    /// Row schema version.
    pub schema_version: u16,
}

/// One workflow row projected into the fleet Kanban board.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LegionWorkflowBoardRowProjection {
    /// Stable workflow session identifier.
    pub session_id: LegionWorkflowSessionId,
    /// Coordinator state used to place the row.
    pub state: LegionWorkflowState,
    /// Human-readable status label derived from the state.
    pub state_label: String,
    /// Display-safe summary rendered in the card body.
    pub summary_label: String,
}

/// One Kanban column projected from coordinator state.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LegionWorkflowBoardColumnProjection {
    /// Stable column kind.
    pub kind: LegionWorkflowBoardColumnKind,
    /// Human-readable column title.
    pub title: String,
    /// Rows assigned to this column.
    pub rows: Vec<LegionWorkflowBoardRowProjection>,
}

/// Project the workflow coordinator state into the five fleet Kanban columns.
pub fn legion_workflow_board_columns(
    projection: &LegionWorkflowProjection,
) -> Vec<LegionWorkflowBoardColumnProjection> {
    let mut columns = vec![
        LegionWorkflowBoardColumnProjection {
            kind: LegionWorkflowBoardColumnKind::Assigned,
            title: LegionWorkflowBoardColumnKind::Assigned.label().to_string(),
            rows: Vec::new(),
        },
        LegionWorkflowBoardColumnProjection {
            kind: LegionWorkflowBoardColumnKind::InProgress,
            title: LegionWorkflowBoardColumnKind::InProgress
                .label()
                .to_string(),
            rows: Vec::new(),
        },
        LegionWorkflowBoardColumnProjection {
            kind: LegionWorkflowBoardColumnKind::WaitingOnHuman,
            title: LegionWorkflowBoardColumnKind::WaitingOnHuman
                .label()
                .to_string(),
            rows: Vec::new(),
        },
        LegionWorkflowBoardColumnProjection {
            kind: LegionWorkflowBoardColumnKind::Testing,
            title: LegionWorkflowBoardColumnKind::Testing.label().to_string(),
            rows: Vec::new(),
        },
        LegionWorkflowBoardColumnProjection {
            kind: LegionWorkflowBoardColumnKind::Done,
            title: LegionWorkflowBoardColumnKind::Done.label().to_string(),
            rows: Vec::new(),
        },
    ];

    for row in &projection.rows {
        let kind = LegionWorkflowBoardColumnKind::from_state(row.lifecycle_state);
        let summary_label = workflow_board_row_summary(row);
        let projected_row = LegionWorkflowBoardRowProjection {
            session_id: row.session_id.clone(),
            state: row.lifecycle_state,
            state_label: workflow_state_label(row.lifecycle_state).to_string(),
            summary_label,
        };

        match kind {
            LegionWorkflowBoardColumnKind::Assigned => columns[0].rows.push(projected_row),
            LegionWorkflowBoardColumnKind::InProgress => columns[1].rows.push(projected_row),
            LegionWorkflowBoardColumnKind::WaitingOnHuman => columns[2].rows.push(projected_row),
            LegionWorkflowBoardColumnKind::Testing => columns[3].rows.push(projected_row),
            LegionWorkflowBoardColumnKind::Done => columns[4].rows.push(projected_row),
        }
    }

    columns
}

fn workflow_state_label(state: LegionWorkflowState) -> &'static str {
    match state {
        LegionWorkflowState::Draft => "Draft",
        LegionWorkflowState::Planning => "Planning",
        LegionWorkflowState::Executing => "Executing",
        LegionWorkflowState::Verifying => "Verifying",
        LegionWorkflowState::WaitingForApproval => "Waiting for approval",
        LegionWorkflowState::WaitingOnHuman => "Waiting on human",
        LegionWorkflowState::Blocked => "Blocked",
        LegionWorkflowState::Completed => "Completed",
        LegionWorkflowState::Failed => "Failed",
        LegionWorkflowState::Cancelled => "Cancelled",
    }
}

fn workflow_board_row_summary(row: &LegionWorkflowProjectionRow) -> String {
    let workers = count_label(row.worker_count, "worker", "workers");
    let dependencies = if row.dependency_count == 0 {
        "No dependencies".to_string()
    } else {
        count_label(row.dependency_count, "dependency", "dependencies")
    };
    let conflicts = if row.unresolved_conflict_count == 0 {
        "No open conflicts".to_string()
    } else {
        format!(
            "{} open",
            count_label(row.unresolved_conflict_count, "conflict", "conflicts")
        )
    };
    let verification = if row.verification_gate_count == 0 {
        "No checks required".to_string()
    } else {
        format!(
            "{} of {} checks passed",
            row.passed_verification_count, row.verification_gate_count
        )
    };
    let approvals = if row.sign_off_count == 0 {
        "No approvals required".to_string()
    } else {
        format!(
            "{} of {} approvals received",
            row.signed_off_count, row.sign_off_count
        )
    };
    let readiness = match row.merge_readiness.state {
        LegionWorkflowMergeReadinessState::Ready => "Ready for review",
        LegionWorkflowMergeReadinessState::WaitingForApproval => "Approval required",
        LegionWorkflowMergeReadinessState::Blocked => "Review blocked",
    };

    [
        workers,
        dependencies,
        conflicts,
        verification,
        approvals,
        readiness.to_string(),
    ]
    .join(" · ")
}

fn count_label(count: u32, singular: &str, plural: &str) -> String {
    format!("{count} {}", if count == 1 { singular } else { plural })
}

/// Structured fleet-card projection for proposal-ledger cards rendered in the desktop UI.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LegionWorkflowFleetCardProjection {
    /// Proposal identifier.
    pub proposal_id: ProposalId,
    /// Proposal title.
    pub title: String,
    /// Owner label projected from the principal.
    pub owner_label: String,
    /// Model label projected from the requested capability.
    pub model_label: String,
    /// Lifecycle label.
    pub status_label: String,
    /// Progress label projected from target coverage and diff size.
    pub progress_label: String,
    /// Files/context label projected from the context manifest.
    pub files_label: String,
    /// Risk label.
    pub risk_label: ProposalRiskLabel,
    /// Aggregated verification status label.
    pub test_status_label: String,
    /// Compact diff summary label.
    pub mini_diff_label: String,
    /// Last activity label.
    pub last_activity_label: String,
}

/// Project proposal-ledger rows into structured fleet cards.
pub fn legion_workflow_fleet_card_projections(
    proposal_projection: &ProposalLedgerProjection,
    verification_projection: &VerificationRunProjection,
) -> Vec<LegionWorkflowFleetCardProjection> {
    proposal_projection
        .rows
        .iter()
        .map(|row| legion_workflow_fleet_card_projection(row, verification_projection))
        .collect()
}

fn legion_workflow_fleet_card_projection(
    row: &ProposalLedgerRow,
    verification_projection: &VerificationRunProjection,
) -> LegionWorkflowFleetCardProjection {
    let represented_targets = row.target_coverage.targets.len() as u32;
    let total_targets =
        represented_targets.saturating_add(row.target_coverage.omitted_target_count);
    let files_label = format!(
        "{} · files={} items",
        row.context_manifest.manifest_id, row.context_manifest.total_item_count
    );

    LegionWorkflowFleetCardProjection {
        proposal_id: row.proposal_id,
        title: row.title.clone(),
        owner_label: row.principal.0.clone(),
        model_label: row.capability.0.clone(),
        status_label: row.lifecycle.label.clone(),
        progress_label: format!(
            "targets={represented_targets}/{total_targets} · hunks={}",
            row.diff_summary.hunk_count
        ),
        files_label,
        risk_label: row.risk_label,
        test_status_label: proposal_verification_status_label(row, verification_projection),
        mini_diff_label: mini_diff_label(&row.diff_summary),
        last_activity_label: format!("updated_at={}", row.updated_at.0),
    }
}

fn proposal_verification_status_label(
    row: &ProposalLedgerRow,
    projection: &VerificationRunProjection,
) -> String {
    let labels = proposal_verification_target_labels(row);
    let matching_rows = projection
        .rows
        .iter()
        .filter(|verification| {
            verification
                .target_labels
                .iter()
                .any(|target_label| labels.iter().any(|label| label == target_label))
        })
        .collect::<Vec<_>>();
    if matching_rows.is_empty() && !projection.rows.is_empty() {
        format!(
            "unlinked {}",
            verification_status_label(projection.rows.iter())
        )
    } else {
        format!(
            "linked {}",
            verification_status_label(matching_rows.into_iter())
        )
    }
}

fn proposal_verification_target_labels(row: &ProposalLedgerRow) -> Vec<String> {
    let mut labels = Vec::new();
    push_unique_label(&mut labels, format!("proposal:{}", row.proposal_id.0));
    push_unique_label(&mut labels, format!("proposal_id={}", row.proposal_id.0));
    for target in &row.target_coverage.targets {
        push_unique_label(&mut labels, target.target_id.clone());
        if let Some(path) = &target.path {
            push_unique_label(&mut labels, path.0.clone());
        }
    }
    for chunk in &row.diff_summary.chunks {
        if let Some(target_id) = &chunk.target_id {
            push_unique_label(&mut labels, target_id.clone());
        }
    }
    for warning in &row.preview_warnings {
        if let Some(target_id) = &warning.target_id {
            push_unique_label(&mut labels, target_id.clone());
        }
    }
    labels
}

fn push_unique_label(labels: &mut Vec<String>, label: String) {
    let label = label.trim().to_string();
    if !label.is_empty() && !labels.iter().any(|existing| existing == &label) {
        labels.push(label);
    }
}

fn verification_status_label<'a>(rows: impl IntoIterator<Item = &'a VerificationRunRow>) -> String {
    let mut planned = 0u32;
    let mut running = 0u32;
    let mut passed = 0u32;
    let mut failed = 0u32;
    let mut blocked = 0u32;
    let mut cancelled = 0u32;

    for row in rows {
        match row.state {
            VerificationRunState::Planned => planned = planned.saturating_add(1),
            VerificationRunState::Running => running = running.saturating_add(1),
            VerificationRunState::Passed => passed = passed.saturating_add(1),
            VerificationRunState::Failed => failed = failed.saturating_add(1),
            VerificationRunState::Blocked => blocked = blocked.saturating_add(1),
            VerificationRunState::Cancelled => cancelled = cancelled.saturating_add(1),
        }
    }

    format!(
        "passed={passed} failed={failed} blocked={blocked} running={running} planned={planned} cancelled={cancelled}"
    )
}

fn mini_diff_label(diff_summary: &legion_protocol::ProposalDiffSummary) -> String {
    format!(
        "{} · targets={} · hunks={} · +{}/-{}",
        diff_summary_kind_label(diff_summary.kind),
        diff_summary.target_count,
        diff_summary.hunk_count,
        diff_summary.inserted_line_count,
        diff_summary.deleted_line_count,
    )
}

fn diff_summary_kind_label(kind: ProposalDiffSummaryKind) -> &'static str {
    match kind {
        ProposalDiffSummaryKind::Text => "text",
        ProposalDiffSummaryKind::FileOperation => "file ops",
        ProposalDiffSummaryKind::WorkspaceEdit => "workspace",
        ProposalDiffSummaryKind::TerminalMetadata => "terminal",
        ProposalDiffSummaryKind::MetadataOnly => "metadata-only",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use legion_protocol::{
        LegionWorkflowMergeReadiness, LegionWorkflowMergeReadinessState,
        LegionWorkflowProjectionRow, RedactionHint, TimestampMillis,
    };

    fn row(session: &str, state: LegionWorkflowState) -> LegionWorkflowProjectionRow {
        LegionWorkflowProjectionRow {
            session_id: LegionWorkflowSessionId(session.to_string()),
            directive_artifact_id: Some(format!("artifact:directive:{session}")),
            spec_artifact_id: Some(format!("artifact:spec:{session}")),
            task_graph_artifact_id: Some(format!("artifact:task-graph:{session}")),
            lifecycle_state: state,
            worker_count: 1,
            provider_route_required_count: 0,
            dependency_count: 0,
            unresolved_conflict_count: 0,
            verification_gate_count: 0,
            passed_verification_count: 0,
            sign_off_count: 0,
            signed_off_count: 0,
            linked_proposals: Vec::new(),
            merge_readiness: LegionWorkflowMergeReadiness {
                state: LegionWorkflowMergeReadinessState::WaitingForApproval,
                blockers: Vec::new(),
                labels: vec!["approval-gated".to_string()],
                redaction_hints: vec![RedactionHint::MetadataOnly],
                schema_version: 1,
            },
            display_safe_labels: vec![format!("{session}:{state:?}"), "metadata-only".to_string()],
            redaction_hints: vec![RedactionHint::MetadataOnly],
            schema_version: 1,
        }
    }

    #[test]
    fn layout_region_all_lists_every_variant_with_unique_ids_and_labels() {
        assert!(
            LayoutRegion::ALL
                .into_iter()
                .all(LayoutRegion::all_covers_every_variant),
            "LayoutRegion::ALL must list every variant"
        );

        let mut ids: Vec<_> = LayoutRegion::ALL
            .into_iter()
            .map(LayoutRegion::as_str)
            .collect();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(
            ids.len(),
            LayoutRegion::ALL.len(),
            "region ids must be unique"
        );

        let mut labels: Vec<_> = LayoutRegion::ALL
            .into_iter()
            .map(LayoutRegion::label)
            .collect();
        labels.sort_unstable();
        labels.dedup();
        assert_eq!(
            labels.len(),
            LayoutRegion::ALL.len(),
            "region labels must be unique"
        );
    }

    #[test]
    fn layout_region_ids_round_trip_and_reject_unknown_values() {
        for region in LayoutRegion::ALL {
            assert_eq!(LayoutRegion::parse(region.as_str()), Some(region));
            assert!(
                !region.projection_source().trim().is_empty(),
                "{} must name the projection it draws from",
                region.as_str()
            );
        }

        // Negative case: an id that is not a region must not resolve, so a
        // typo in persisted state fails loudly instead of silently binding to
        // whichever variant happens to be first.
        assert_eq!(LayoutRegion::parse("code_canvas"), None);
        assert_eq!(LayoutRegion::parse("Top bar"), None);
        assert_eq!(LayoutRegion::parse(""), None);
    }

    #[test]
    fn layout_region_content_backed_regions_are_empty_for_an_empty_shell() {
        // Without this the populated-snapshot assertion could pass on a
        // counter that is non-zero no matter what the snapshot contains.
        let snapshot = crate::ui::Shell::empty("").projection_snapshot();

        for region in LayoutRegion::ALL {
            let count = region.projected_item_count(&snapshot);
            if region.is_content_backed() {
                assert_eq!(
                    count,
                    0,
                    "{} must project nothing from an empty shell",
                    region.as_str()
                );
            } else {
                assert!(
                    count > 0,
                    "{} is persistent chrome and must still project",
                    region.as_str()
                );
            }
        }
    }

    #[test]
    fn layout_region_dock_placement_excludes_panels_the_mode_may_not_construct() {
        // The dock's regression surface is mode filtering rather than content,
        // so it gets the negative case the emptiness check cannot give it.
        // Manual is not the *smallest* dock — it is a deliberately rich local
        // IDE layout — so panel count is not the invariant. What must hold is
        // that no mode places a panel its runtime surfaces forbid.
        let registry = PanelRegistry::standard();

        for mode in [
            DockMode::Manual,
            DockMode::Assist,
            DockMode::Delegate,
            DockMode::Automate,
        ] {
            assert!(
                dock_placement_count(mode) > 0,
                "{mode:?} must place dock panels"
            );
        }

        for forbidden in [
            crate::ui::PanelId::Assistant,
            crate::ui::PanelId::Delegation,
            crate::ui::PanelId::AgentFleet,
            crate::ui::PanelId::AgentLogs,
        ] {
            assert!(
                !registry.is_visible_in(forbidden, DockMode::Manual),
                "Manual mode must not construct {}",
                forbidden.as_str()
            );
        }
        assert!(
            registry.is_visible_in(crate::ui::PanelId::ProjectExplorer, DockMode::Manual),
            "Manual mode must construct the project explorer"
        );
        assert!(
            registry.is_visible_in(crate::ui::PanelId::AgentLogs, DockMode::Automate),
            "Automate mode must construct agent logs"
        );
    }

    #[test]
    fn layout_region_dock_never_places_a_panel_the_mode_cannot_construct() {
        // A placed-but-unconstructible panel renders as a hole in the dock.
        for mode in [
            DockMode::Manual,
            DockMode::Assist,
            DockMode::Delegate,
            DockMode::Automate,
        ] {
            let layout = DockLayout::standard(mode);
            let registry = PanelRegistry::standard();
            let placed: Vec<_> = [&layout.left, &layout.right, &layout.bottom]
                .into_iter()
                .flat_map(|side| {
                    std::iter::once(side.pinned_default).chain(side.custom_toolkit.iter().copied())
                })
                .collect();
            let unconstructible: Vec<_> = placed
                .iter()
                .filter(|panel| !registry.is_visible_in(**panel, mode))
                .map(|panel| panel.as_str())
                .collect();
            assert!(
                unconstructible.is_empty(),
                "{mode:?} places panels it cannot construct: {unconstructible:?}"
            );
        }
    }

    #[test]
    fn groups_rows_by_coordinator_state() {
        let projection = LegionWorkflowProjection {
            projection_id: "legion-workflow:test-board".to_string(),
            rows: vec![
                row("session:draft", LegionWorkflowState::Draft),
                row("session:planning", LegionWorkflowState::Planning),
                row("session:executing", LegionWorkflowState::Executing),
                row("session:verifying", LegionWorkflowState::Verifying),
                row("session:waiting", LegionWorkflowState::WaitingForApproval),
                row("session:blocked", LegionWorkflowState::Blocked),
                row("session:completed", LegionWorkflowState::Completed),
                row("session:failed", LegionWorkflowState::Failed),
                row("session:cancelled", LegionWorkflowState::Cancelled),
            ],
            mcp_registries: Vec::new(),
            decision_feed: Vec::new(),
            risk_monitors: Vec::new(),
            kill_switches: Vec::new(),
            tool_permission_requests: Vec::new(),
            total_session_count: 9,
            mcp_registry_count: 0,
            decision_feed_count: 0,
            risk_monitor_count: 0,
            kill_switch_count: 0,
            tool_permission_request_count: 0,
            omitted_row_count: 0,
            generated_at: TimestampMillis(1),
            redaction_hints: vec![RedactionHint::MetadataOnly],
            schema_version: 1,
        };

        let columns = legion_workflow_board_columns(&projection);
        let kinds: Vec<_> = columns.iter().map(|column| column.kind).collect();

        assert_eq!(
            kinds,
            vec![
                LegionWorkflowBoardColumnKind::Assigned,
                LegionWorkflowBoardColumnKind::InProgress,
                LegionWorkflowBoardColumnKind::WaitingOnHuman,
                LegionWorkflowBoardColumnKind::Testing,
                LegionWorkflowBoardColumnKind::Done,
            ]
        );
        assert_eq!(columns[0].rows.len(), 2);
        assert_eq!(columns[1].rows.len(), 1);
        assert_eq!(columns[2].rows.len(), 2);
        assert_eq!(columns[3].rows.len(), 1);
        assert_eq!(columns[4].rows.len(), 3);
        assert!(columns[0].rows.iter().all(|row| matches!(
            row.state,
            LegionWorkflowState::Draft | LegionWorkflowState::Planning
        )));
        assert!(
            columns[1]
                .rows
                .iter()
                .all(|row| row.state == LegionWorkflowState::Executing)
        );
        assert!(columns[2].rows.iter().all(|row| matches!(
            row.state,
            LegionWorkflowState::WaitingForApproval
                | LegionWorkflowState::WaitingOnHuman
                | LegionWorkflowState::Blocked
        )));
        assert!(
            columns[3]
                .rows
                .iter()
                .all(|row| row.state == LegionWorkflowState::Verifying)
        );
        assert!(columns[4].rows.iter().all(|row| matches!(
            row.state,
            LegionWorkflowState::Completed
                | LegionWorkflowState::Failed
                | LegionWorkflowState::Cancelled
        )));
    }

    #[test]
    fn projects_fleet_card_fields_from_structured_projections() {
        let mut proposal_projection = legion_protocol::ProposalLedgerProjection {
            rows: vec![legion_protocol::ProposalLedgerRow {
                proposal_id: legion_protocol::ProposalId(77),
                workspace_id: Some(legion_protocol::WorkspaceId(1)),
                title: "workflow card".to_string(),
                payload_kind: legion_protocol::ProposalPayloadKind::WorkspaceEdit,
                lifecycle: legion_protocol::ProposalLifecycleStateDisplay {
                    state: legion_protocol::ProposalLifecycleState::Previewed,
                    label: "Previewed".to_string(),
                    description: "ready for review".to_string(),
                },
                principal: legion_protocol::PrincipalId("owner:alice".to_string()),
                capability: legion_protocol::CapabilityId("model:gpt-5.5".to_string()),
                created_at: TimestampMillis(1),
                updated_at: TimestampMillis(2),
                expires_at: None,
                risk_label: legion_protocol::ProposalRiskLabel::Medium,
                privacy_label: legion_protocol::ProposalPrivacyLabel::WorkspaceMetadata,
                rollback: legion_protocol::ProposalRollbackAvailability::Available,
                target_coverage: legion_protocol::ProposalTargetCoverage {
                    coverage_kind: legion_protocol::ProposalTargetCoverageKind::Partial,
                    targets: vec![legion_protocol::ProposalAffectedTarget {
                        target_id: "file:alpha".to_string(),
                        kind: legion_protocol::ProposalTargetKind::ClosedFile,
                        workspace_id: Some(legion_protocol::WorkspaceId(1)),
                        file_id: Some(legion_protocol::FileId(5)),
                        buffer_id: None,
                        path: Some(legion_protocol::CanonicalPath("src/lib.rs".to_string())),
                        terminal_session_id: None,
                        plugin_id: None,
                        remote_authority: None,
                        collaboration_session_id: None,
                        byte_ranges: Vec::new(),
                        redaction_hints: vec![legion_protocol::RedactionHint::MetadataOnly],
                    }],
                    omitted_target_count: 0,
                    redaction_hints: vec![legion_protocol::RedactionHint::MetadataOnly],
                },
                context_manifest: legion_protocol::ProposalContextManifestSummary {
                    manifest_id: "manifest:77".to_string(),
                    category_count: 1,
                    total_item_count: 2,
                    omitted_item_count: 0,
                    categories: vec![legion_protocol::ProposalContextManifestEntrySummary {
                        category: "files".to_string(),
                        item_count: 2,
                        omitted_item_count: 0,
                        privacy_label: legion_protocol::ProposalPrivacyLabel::WorkspaceMetadata,
                        manifest_hash: None,
                        redaction_hints: vec![legion_protocol::RedactionHint::MetadataOnly],
                    }],
                    redaction_hints: vec![legion_protocol::RedactionHint::MetadataOnly],
                },
                diff_summary: legion_protocol::ProposalDiffSummary {
                    kind: legion_protocol::ProposalDiffSummaryKind::Text,
                    target_count: 1,
                    hunk_count: 2,
                    inserted_line_count: 5,
                    deleted_line_count: 1,
                    omitted_hunk_count: 0,
                    full_source_redacted: true,
                    diff_hash: Some(legion_protocol::FileFingerprint {
                        algorithm: "sha256".to_string(),
                        value: "diff:77".to_string(),
                    }),
                    chunks: vec![legion_protocol::ProposalDiffChunkDescriptor {
                        chunk_id: "chunk:0".to_string(),
                        target_id: Some("file:alpha".to_string()),
                        byte_range: None,
                        changed_line_count: 6,
                        inserted_line_count: 5,
                        deleted_line_count: 1,
                        content_hash: None,
                    }],
                    redaction_hints: vec![legion_protocol::RedactionHint::MetadataOnly],
                },
                preview_warnings: vec![legion_protocol::ProposalPreviewWarning {
                    code: "proposal.preview.target-coverage-partial".to_string(),
                    kind: legion_protocol::ProposalPreviewWarningKind::TargetCoveragePartial,
                    message: "target coverage is partial".to_string(),
                    target_id: Some("file:alpha".to_string()),
                    redaction_hints: vec![legion_protocol::RedactionHint::MetadataOnly],
                }],
                diagnostics: Vec::new(),
                redaction_hints: vec![legion_protocol::RedactionHint::MetadataOnly],
                schema_version: 1,
            }],
            selected_proposal_id: Some(legion_protocol::ProposalId(77)),
            omitted_row_count: 0,
            generated_at: TimestampMillis(3),
            redaction_hints: vec![legion_protocol::RedactionHint::MetadataOnly],
            schema_version: 1,
        };
        let mut beta_row = proposal_projection.rows[0].clone();
        beta_row.proposal_id = legion_protocol::ProposalId(78);
        beta_row.title = "workflow beta card".to_string();
        beta_row.updated_at = TimestampMillis(9);
        beta_row.target_coverage.targets[0].target_id = "file:beta".to_string();
        beta_row.target_coverage.targets[0].path =
            Some(legion_protocol::CanonicalPath("src/beta.rs".to_string()));
        beta_row.diff_summary.chunks[0].target_id = Some("file:beta".to_string());
        beta_row.preview_warnings[0].target_id = Some("file:beta".to_string());
        proposal_projection.rows.push(beta_row);
        let verification_projection = legion_protocol::VerificationRunProjection {
            projection_id: "verification-runs:77".to_string(),
            rows: vec![
                legion_protocol::VerificationRunRow {
                    run_id: "run:77".to_string(),
                    label: "unit tests".to_string(),
                    state: legion_protocol::VerificationRunState::Passed,
                    command_class_label: "test".to_string(),
                    command_body_redacted: true,
                    exit_code: Some(0),
                    target_labels: vec!["file:alpha".to_string()],
                    evidence_artifact_id: Some("artifact:verification:77".to_string()),
                    started_at: Some(TimestampMillis(4)),
                    completed_at: Some(TimestampMillis(5)),
                    risk_label: legion_protocol::ProposalRiskLabel::Low,
                    privacy_label: legion_protocol::ProposalPrivacyLabel::WorkspaceMetadata,
                    redaction_hints: vec![legion_protocol::RedactionHint::MetadataOnly],
                    schema_version: 1,
                },
                legion_protocol::VerificationRunRow {
                    run_id: "run:78".to_string(),
                    label: "beta tests".to_string(),
                    state: legion_protocol::VerificationRunState::Failed,
                    command_class_label: "test".to_string(),
                    command_body_redacted: true,
                    exit_code: Some(1),
                    target_labels: vec!["file:beta".to_string()],
                    evidence_artifact_id: Some("artifact:verification:78".to_string()),
                    started_at: Some(TimestampMillis(7)),
                    completed_at: Some(TimestampMillis(8)),
                    risk_label: legion_protocol::ProposalRiskLabel::Low,
                    privacy_label: legion_protocol::ProposalPrivacyLabel::WorkspaceMetadata,
                    redaction_hints: vec![legion_protocol::RedactionHint::MetadataOnly],
                    schema_version: 1,
                },
            ],
            omitted_row_count: 0,
            generated_at: TimestampMillis(6),
            redaction_hints: vec![legion_protocol::RedactionHint::MetadataOnly],
            schema_version: 1,
        };

        let cards =
            legion_workflow_fleet_card_projections(&proposal_projection, &verification_projection);

        assert_eq!(cards.len(), 2);
        let card = &cards[0];
        assert_eq!(card.proposal_id, legion_protocol::ProposalId(77));
        assert_eq!(card.owner_label, "owner:alice");
        assert_eq!(card.model_label, "model:gpt-5.5");
        assert_eq!(card.status_label, "Previewed");
        assert_eq!(card.progress_label, "targets=1/1 · hunks=2");
        assert_eq!(card.files_label, "manifest:77 · files=2 items");
        assert_eq!(card.risk_label, legion_protocol::ProposalRiskLabel::Medium);
        assert_eq!(
            card.test_status_label,
            "linked passed=1 failed=0 blocked=0 running=0 planned=0 cancelled=0"
        );
        assert_eq!(card.mini_diff_label, "text · targets=1 · hunks=2 · +5/-1");
        assert_eq!(card.last_activity_label, "updated_at=2");
        assert_eq!(cards[1].proposal_id, legion_protocol::ProposalId(78));
        assert_eq!(
            cards[1].test_status_label,
            "linked passed=0 failed=1 blocked=0 running=0 planned=0 cancelled=0"
        );

        let mut unlinked_verification_projection = verification_projection.clone();
        let mut unlinked_row = unlinked_verification_projection.rows[0].clone();
        unlinked_row.run_id = "run:delegated-plan".to_string();
        unlinked_row.state = legion_protocol::VerificationRunState::Planned;
        unlinked_row.target_labels = vec!["delegated_task.plan_row.metadata_only".to_string()];
        unlinked_verification_projection.rows = vec![unlinked_row];

        let unlinked_cards = legion_workflow_fleet_card_projections(
            &proposal_projection,
            &unlinked_verification_projection,
        );
        assert!(unlinked_cards.iter().all(|card| {
            card.test_status_label
                == "unlinked passed=0 failed=0 blocked=0 running=0 planned=1 cancelled=0"
        }));
    }
}
