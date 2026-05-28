//! Metadata-only operational health summaries for desktop beta diagnostics.

use devil_ui::ShellProjectionSnapshot;

use crate::workflow::DesktopWorkflowOutcome;

const NOT_OBSERVED: &str = "not_observed";

/// Metadata-only operational health snapshot for desktop beta triage.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DesktopOperationalHealthSnapshot {
    /// Display-only workspace label or path.
    pub workspace_label: String,
    /// Count of open tabs in the app-owned projection.
    pub open_tab_count: usize,
    /// Count of dirty tabs in the app-owned projection.
    pub dirty_tab_count: usize,
    /// Count of projected status messages.
    pub status_message_count: usize,
    /// Last desktop workflow outcome label.
    pub last_outcome_label: String,
    /// Search status kind label.
    pub search_status_label: String,
    /// Current bounded search result count.
    pub search_result_count: usize,
    /// Current bounded omitted file count.
    pub search_omitted_file_count: usize,
    /// Current bounded omitted result count.
    pub search_omitted_result_count: usize,
    /// Language tooling status label.
    pub language_status_label: String,
    /// Count of projected language operations.
    pub language_operation_count: usize,
    /// Count of projected language problems.
    pub language_problem_count: usize,
    /// Count of projected language cancellations.
    pub language_cancellation_count: u32,
    /// Terminal status kind label.
    pub terminal_status_label: String,
    /// Count of projected terminal rows.
    pub terminal_output_row_count: usize,
    /// Count of omitted terminal rows.
    pub terminal_omitted_row_count: u32,
    /// Whether the latest terminal request was denied.
    pub terminal_denial_label: String,
    /// Count of projected proposal ledger rows.
    pub proposal_row_count: usize,
    /// Selected proposal id label.
    pub selected_proposal_label: String,
    /// Count of projected assisted-AI providers.
    pub assisted_provider_count: u32,
    /// Count of projected assisted-AI requests.
    pub assisted_request_count: u32,
    /// Count of projected assisted-AI refusals.
    pub assisted_refusal_count: u32,
    /// Count of reviewable assisted-AI proposal previews.
    pub assisted_preview_ready_count: u32,
    /// Whether a metadata-only session state path is configured.
    pub session_state_configured: bool,
    /// Whether a metadata-only diagnostics export path is configured.
    pub diagnostics_export_configured: bool,
    /// Unsupported advanced surface labels that Phase 7 must not claim.
    pub unsupported_surfaces: Vec<String>,
}

impl DesktopOperationalHealthSnapshot {
    /// Build operational health for a runtime diagnostics export.
    #[must_use]
    pub fn from_runtime(
        snapshot: &ShellProjectionSnapshot,
        workspace_label: impl Into<String>,
        last_outcome: &DesktopWorkflowOutcome,
        session_state_configured: bool,
        diagnostics_export_configured: bool,
    ) -> Self {
        Self::from_parts(
            snapshot,
            workspace_label.into(),
            format!("{last_outcome:?}"),
            session_state_configured,
            diagnostics_export_configured,
        )
    }

    /// Build operational health for a projection-only view model.
    #[must_use]
    pub fn from_projection(snapshot: &ShellProjectionSnapshot) -> Self {
        Self::from_parts(
            snapshot,
            snapshot.layout_projection.layout.title.clone(),
            NOT_OBSERVED.to_string(),
            false,
            false,
        )
    }

    fn from_parts(
        snapshot: &ShellProjectionSnapshot,
        workspace_label: String,
        last_outcome_label: String,
        session_state_configured: bool,
        diagnostics_export_configured: bool,
    ) -> Self {
        let tabs = &snapshot.daily_editing_projection.tabs.tabs;
        let search = &snapshot.search_projection;
        let language = &snapshot.language_tooling_projection;
        let terminal = &snapshot.terminal_panel_projection;
        let ledger = &snapshot.proposal_ledger_projection;
        let assisted = &snapshot.assisted_ai_projection;

        Self {
            workspace_label,
            open_tab_count: tabs.len(),
            dirty_tab_count: tabs.iter().filter(|tab| tab.dirty).count(),
            status_message_count: snapshot.status_messages.len(),
            last_outcome_label,
            search_status_label: format!("{:?}", search.status.kind),
            search_result_count: search.results.len(),
            search_omitted_file_count: search.omitted_file_count,
            search_omitted_result_count: search.omitted_result_count,
            language_status_label: format!("{:?}", language.status),
            language_operation_count: language.operations.len(),
            language_problem_count: language.problems.len(),
            language_cancellation_count: language.cancellation_count,
            terminal_status_label: format!("{:?}", terminal.status.kind),
            terminal_output_row_count: terminal.output_rows.len(),
            terminal_omitted_row_count: terminal.scrollback.omitted_row_count,
            terminal_denial_label: if terminal.last_denial.is_some() {
                "denied".to_string()
            } else {
                "none".to_string()
            },
            proposal_row_count: ledger.rows.len(),
            selected_proposal_label: ledger
                .selected_proposal_id
                .map(|proposal_id| proposal_id.0.to_string())
                .unwrap_or_else(|| "none".to_string()),
            assisted_provider_count: assisted.provider_count,
            assisted_request_count: assisted.request_count,
            assisted_refusal_count: assisted.refusal_count,
            assisted_preview_ready_count: assisted.preview_ready_count,
            session_state_configured,
            diagnostics_export_configured,
            unsupported_surfaces: phase7_unsupported_surfaces(),
        }
    }

    /// Stable compact rows for GUI display.
    #[must_use]
    pub fn rows(&self) -> Vec<String> {
        let mut rows = vec![
            format!("workspace: {}", self.workspace_label),
            format!(
                "tabs: open={} dirty={}",
                self.open_tab_count, self.dirty_tab_count
            ),
            format!("status_messages: {}", self.status_message_count),
            format!("last_outcome: {}", self.last_outcome_label),
            format!(
                "search: status={} results={} omitted_files={} omitted_results={}",
                self.search_status_label,
                self.search_result_count,
                self.search_omitted_file_count,
                self.search_omitted_result_count
            ),
            format!(
                "language: status={} operations={} cancellations={} problems={}",
                self.language_status_label,
                self.language_operation_count,
                self.language_cancellation_count,
                self.language_problem_count
            ),
            format!(
                "terminal: status={} rows={} omitted={} denial={}",
                self.terminal_status_label,
                self.terminal_output_row_count,
                self.terminal_omitted_row_count,
                self.terminal_denial_label
            ),
            format!(
                "proposals: rows={} selected={}",
                self.proposal_row_count, self.selected_proposal_label
            ),
            format!(
                "assisted_ai: providers={} requests={} refusals={} previews={}",
                self.assisted_provider_count,
                self.assisted_request_count,
                self.assisted_refusal_count,
                self.assisted_preview_ready_count
            ),
            format!(
                "session_state_configured: {}",
                self.session_state_configured
            ),
            format!(
                "diagnostics_export_configured: {}",
                self.diagnostics_export_configured
            ),
            format!("unsupported_surfaces: {}", self.unsupported_surfaces.len()),
        ];
        rows.extend(
            self.unsupported_surfaces
                .iter()
                .map(|surface| format!("unsupported: {surface}")),
        );
        rows
    }

    /// Render a stable metadata-only markdown section body.
    #[must_use]
    pub fn to_markdown(&self) -> String {
        let mut lines = vec![
            format!("workspace_label: {}", self.workspace_label),
            format!("open_tab_count: {}", self.open_tab_count),
            format!("dirty_tab_count: {}", self.dirty_tab_count),
            format!("status_message_count: {}", self.status_message_count),
            format!("last_outcome_label: {}", self.last_outcome_label),
            format!("search_status: {}", self.search_status_label),
            format!("search_result_count: {}", self.search_result_count),
            format!(
                "search_omitted_file_count: {}",
                self.search_omitted_file_count
            ),
            format!(
                "search_omitted_result_count: {}",
                self.search_omitted_result_count
            ),
            format!("language_status: {}", self.language_status_label),
            format!(
                "language_operation_count: {}",
                self.language_operation_count
            ),
            format!("language_problem_count: {}", self.language_problem_count),
            format!(
                "language_cancellation_count: {}",
                self.language_cancellation_count
            ),
            format!("terminal_status: {}", self.terminal_status_label),
            format!(
                "terminal_output_row_count: {}",
                self.terminal_output_row_count
            ),
            format!(
                "terminal_omitted_row_count: {}",
                self.terminal_omitted_row_count
            ),
            format!("terminal_denial: {}", self.terminal_denial_label),
            format!("proposal_row_count: {}", self.proposal_row_count),
            format!("selected_proposal: {}", self.selected_proposal_label),
            format!("assisted_provider_count: {}", self.assisted_provider_count),
            format!("assisted_request_count: {}", self.assisted_request_count),
            format!("assisted_refusal_count: {}", self.assisted_refusal_count),
            format!(
                "assisted_preview_ready_count: {}",
                self.assisted_preview_ready_count
            ),
            format!(
                "session_state_configured: {}",
                self.session_state_configured
            ),
            format!(
                "diagnostics_export_configured: {}",
                self.diagnostics_export_configured
            ),
            "unsupported_surfaces:".to_string(),
        ];
        lines.extend(
            self.unsupported_surfaces
                .iter()
                .map(|surface| format!("- {surface}")),
        );
        lines.push(String::new());
        lines.join("\n")
    }
}

/// Phase 7 local beta limitations that must remain visible in evidence.
#[must_use]
pub fn phase7_unsupported_surfaces() -> Vec<String> {
    vec![
        "Remote production GUI: unsupported".to_string(),
        "Collaboration GUI: unsupported".to_string(),
        "Plugin management GUI: unsupported".to_string(),
        "Hosted provider activation: unsupported".to_string(),
        "Signed installer: unsupported".to_string(),
        "Cross-platform parity: unsupported".to_string(),
        "Autonomous apply: unsupported (autonomous execution unavailable)".to_string(),
    ]
}
