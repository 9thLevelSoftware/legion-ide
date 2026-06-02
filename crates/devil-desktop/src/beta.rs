//! GUI Phase 7 local IDE beta smoke harness.

use std::{
    fs, io,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, anyhow, bail};
use devil_app::AppProductMode;
use devil_protocol::TextCoordinate;
use devil_ui::SearchScopeProjection;

use crate::{
    bridge::DesktopAction,
    smoke::GUI_PHASE7_BETA_SMOKE_LABEL,
    workflow::{DesktopLaunchConfig, DesktopRuntime, DesktopWorkflowOutcome},
};

const FIXTURE_CARGO_TOML: &str = r#"[package]
name = "devil-phase7-beta-fixture"
version = "0.1.0"
edition = "2024"

[workspace]
"#;

const FIXTURE_MAIN_RS: &str = r#"fn main() {
    println!("beta fixture ready");
}
"#;

const FIXTURE_README: &str = r#"# GUI Phase 7 beta fixture

This workspace is generated under `target/` by the beta smoke harness.
"#;

const EDIT_TEXT: &str = "// metadata-only beta edit\n";

/// Default isolated workspace for GUI Phase 7 beta smoke write actions.
pub const DEFAULT_BETA_WORKSPACE_PATH: &str = "target/gui-phase7-beta-workspace";

/// Default metadata-only session path for GUI Phase 7 beta smoke.
pub const DEFAULT_BETA_SESSION_STATE_PATH: &str = "target/gui-phase7-session.json";

/// Default metadata-only diagnostics export path for GUI Phase 7 beta smoke.
pub const DEFAULT_BETA_DIAGNOSTICS_EXPORT_PATH: &str = "target/gui-phase7-diagnostics.md";

/// Launch-time configuration for GUI Phase 7 beta smoke.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BetaWorkflowConfig {
    /// Real workspace used only for command/evidence context.
    pub real_workspace_root: PathBuf,
    /// Isolated Rust workspace root used for all write actions.
    pub beta_workspace_root: PathBuf,
    /// Metadata-only workflow evidence path.
    pub evidence_path: PathBuf,
    /// Metadata-only session state path.
    pub session_state: PathBuf,
    /// Metadata-only diagnostics export path.
    pub diagnostics_export: PathBuf,
}

impl BetaWorkflowConfig {
    /// Create a beta smoke configuration.
    pub fn new(
        real_workspace_root: PathBuf,
        beta_workspace_root: PathBuf,
        evidence_path: PathBuf,
        session_state: PathBuf,
        diagnostics_export: PathBuf,
    ) -> Result<Self> {
        if real_workspace_root.as_os_str().is_empty() {
            bail!("real workspace root cannot be empty");
        }
        if beta_workspace_root.as_os_str().is_empty() {
            bail!("beta workspace root cannot be empty");
        }
        if evidence_path.as_os_str().is_empty() {
            bail!("beta evidence path cannot be empty");
        }
        if session_state.as_os_str().is_empty() {
            bail!("beta session state path cannot be empty");
        }
        if diagnostics_export.as_os_str().is_empty() {
            bail!("beta diagnostics export path cannot be empty");
        }
        Ok(Self {
            real_workspace_root,
            beta_workspace_root,
            evidence_path,
            session_state,
            diagnostics_export,
        })
    }
}

/// Overall beta workflow status.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BetaWorkflowStatus {
    /// Workflow completed and evidence was written.
    Passed,
    /// Workflow could not run in this environment.
    Blocked,
    /// Workflow ran but one or more checks failed.
    Failed,
}

impl BetaWorkflowStatus {
    fn as_str(self) -> &'static str {
        match self {
            Self::Passed => "passed",
            Self::Blocked => "blocked",
            Self::Failed => "failed",
        }
    }
}

/// Metadata-only beta workflow report.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BetaWorkflowReport {
    /// Command represented by this run.
    pub command: String,
    /// Overall workflow status.
    pub status: BetaWorkflowStatus,
    /// Real workspace path used for launch context.
    pub real_workspace_root: PathBuf,
    /// Isolated workspace path used for write actions.
    pub beta_workspace_root: PathBuf,
    /// Open/browse status label.
    pub browse_status: String,
    /// Edit/save status label.
    pub edit_save_status: String,
    /// Active-file search status label.
    pub active_file_search_status: String,
    /// Workspace search status label.
    pub workspace_search_status: String,
    /// Language tooling status label.
    pub language_status: String,
    /// Terminal policy status label.
    pub terminal_status: String,
    /// Proposal lifecycle status label.
    pub proposal_status: String,
    /// Number of projected status messages.
    pub status_message_count: usize,
    /// Path to the metadata-only diagnostics export.
    pub diagnostics_export: PathBuf,
    /// Unsupported advanced surface labels.
    pub unsupported_surfaces: Vec<String>,
    /// Metadata-only errors or blockers.
    pub errors: Vec<String>,
}

impl BetaWorkflowReport {
    /// Render the report as stable markdown.
    #[must_use]
    pub fn to_markdown(&self) -> String {
        let unsupported = self
            .unsupported_surfaces
            .iter()
            .map(|surface| format!("- {surface}"))
            .collect::<Vec<_>>()
            .join("\n");
        let errors = if self.errors.is_empty() {
            "- none".to_string()
        } else {
            self.errors
                .iter()
                .map(|error| format!("- {error}"))
                .collect::<Vec<_>>()
                .join("\n")
        };

        format!(
            concat!(
                "# GUI Phase 7 Local Workflow Smoke\n\n",
                "## Status\n\n",
                "status: {status}\n",
                "smoke_label: {smoke_label}\n",
                "metadata-only: true\n",
                "real_workspace_root: {real_workspace_root}\n",
                "beta_workspace_root: {beta_workspace_root}\n",
                "diagnostics_export: {diagnostics_export}\n",
                "status_message_count: {status_message_count}\n\n",
                "## Command\n\n",
                "`{command}`\n\n",
                "## Local IDE Workflow\n\n",
                "browse: {browse_status}\n",
                "edit_save: {edit_save_status}\n",
                "active_file_search: {active_file_search_status}\n",
                "workspace_search: {workspace_search_status}\n",
                "language: {language_status}\n",
                "terminal: {terminal_status}\n",
                "proposal: {proposal_status}\n\n",
                "## Unsupported Surfaces\n\n",
                "unsupported_surfaces:\n",
                "{unsupported}\n\n",
                "## Privacy\n\n",
                "- Evidence records paths, counts, statuses, and labels only.\n",
                "- Evidence does not include raw source, dirty buffer text, prompts, provider payloads, terminal payloads, or secrets.\n\n",
                "## Errors\n\n",
                "{errors}\n"
            ),
            status = self.status.as_str(),
            smoke_label = GUI_PHASE7_BETA_SMOKE_LABEL,
            real_workspace_root = self.real_workspace_root.display(),
            beta_workspace_root = self.beta_workspace_root.display(),
            diagnostics_export = self.diagnostics_export.display(),
            status_message_count = self.status_message_count,
            command = self.command,
            browse_status = self.browse_status,
            edit_save_status = self.edit_save_status,
            active_file_search_status = self.active_file_search_status,
            workspace_search_status = self.workspace_search_status,
            language_status = self.language_status,
            terminal_status = self.terminal_status,
            proposal_status = self.proposal_status,
            unsupported = unsupported,
            errors = errors,
        )
    }

    /// Write markdown evidence to disk.
    pub fn write_evidence(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent()
            && !parent.as_os_str().is_empty()
        {
            fs::create_dir_all(parent)?;
        }
        fs::write(path, self.to_markdown())?;
        Ok(())
    }
}

struct PreparedBetaWorkspace {
    root: PathBuf,
    main_rs: PathBuf,
}

/// Run the GUI Phase 7 local IDE beta smoke workflow.
pub fn run_beta_workflow(config: BetaWorkflowConfig) -> Result<BetaWorkflowReport> {
    let command = beta_smoke_command(&config);
    let report = match run_beta_workflow_inner(config.clone(), command.clone()) {
        Ok(report) => report,
        Err(error) => BetaWorkflowReport {
            command,
            status: BetaWorkflowStatus::Blocked,
            real_workspace_root: config.real_workspace_root.clone(),
            beta_workspace_root: config.beta_workspace_root.clone(),
            browse_status: "blocked".to_string(),
            edit_save_status: "blocked".to_string(),
            active_file_search_status: "blocked".to_string(),
            workspace_search_status: "blocked".to_string(),
            language_status: "blocked".to_string(),
            terminal_status: "blocked".to_string(),
            proposal_status: "blocked".to_string(),
            status_message_count: 0,
            diagnostics_export: config.diagnostics_export.clone(),
            unsupported_surfaces: unsupported_surfaces(),
            errors: vec![error.to_string()],
        },
    };

    report.write_evidence(&config.evidence_path)?;
    match report.status {
        BetaWorkflowStatus::Passed => Ok(report),
        BetaWorkflowStatus::Blocked => {
            bail!(
                "GUI Phase 7 beta workflow blocked; see {}",
                config.evidence_path.display()
            );
        }
        BetaWorkflowStatus::Failed => {
            bail!(
                "GUI Phase 7 beta workflow failed; see {}",
                config.evidence_path.display()
            );
        }
    }
}

fn run_beta_workflow_inner(
    config: BetaWorkflowConfig,
    command: String,
) -> Result<BetaWorkflowReport> {
    let prepared = prepare_beta_workspace(&config.beta_workspace_root)?;
    let launch_config = DesktopLaunchConfig::new(
        prepared.root.clone(),
        Some(prepared.main_rs.to_string_lossy().into_owned()),
    )
    .with_session_state(config.session_state.clone())
    .with_diagnostics_export(config.diagnostics_export.clone());
    let mut runtime = DesktopRuntime::open(launch_config)?;
    runtime.set_product_mode(AppProductMode::Assist)?;
    let mut errors = Vec::new();

    let browse_status = run_browse_actions(&mut runtime);
    let edit_save_status = run_edit_save_actions(&mut runtime, &prepared.main_rs, &mut errors);
    let active_file_search_status =
        run_search_action(&mut runtime, SearchScopeProjection::ActiveFile, "beta");
    let workspace_search_status =
        run_search_action(&mut runtime, SearchScopeProjection::Workspace, "beta");
    let language_status = run_language_actions(&mut runtime);
    let terminal_status = run_terminal_actions(&mut runtime);
    let proposal_status = run_proposal_actions(&mut runtime);

    let _ = runtime.handle_action(DesktopAction::Quit);
    let final_snapshot = runtime.projection_snapshot();
    let diagnostics_export_label = config.diagnostics_export.display().to_string();
    errors.extend(beta_workflow_gate_errors(BetaWorkflowGateInputs {
        browse_status: &browse_status,
        edit_save_status: &edit_save_status,
        active_file_search_status: &active_file_search_status,
        workspace_search_status: &workspace_search_status,
        language_status: &language_status,
        terminal_status: &terminal_status,
        proposal_status: &proposal_status,
        diagnostics_export_written: config.diagnostics_export.is_file(),
        diagnostics_export_label: &diagnostics_export_label,
    }));
    let status = if errors.is_empty() {
        BetaWorkflowStatus::Passed
    } else {
        BetaWorkflowStatus::Failed
    };

    Ok(BetaWorkflowReport {
        command,
        status,
        real_workspace_root: config.real_workspace_root,
        beta_workspace_root: prepared.root,
        browse_status,
        edit_save_status,
        active_file_search_status,
        workspace_search_status,
        language_status,
        terminal_status,
        proposal_status,
        status_message_count: final_snapshot.status_messages.len(),
        diagnostics_export: config.diagnostics_export,
        unsupported_surfaces: unsupported_surfaces(),
        errors,
    })
}

fn prepare_beta_workspace(path: &Path) -> Result<PreparedBetaWorkspace> {
    let workspace_root = std::env::current_dir()
        .context("resolve current directory")?
        .canonicalize()
        .context("canonicalize current directory")?;
    let target_root = workspace_root.join("target");
    fs::create_dir_all(&target_root).context("create target directory")?;
    let target_root = target_root
        .canonicalize()
        .context("canonicalize target directory")?;

    let requested = if path.is_absolute() {
        path.to_path_buf()
    } else {
        workspace_root.join(path)
    };
    let Some(parent) = requested.parent() else {
        bail!("beta workspace path must have a parent directory");
    };
    fs::create_dir_all(parent).with_context(|| {
        format!(
            "create beta workspace parent directory `{}`",
            parent.display()
        )
    })?;
    let parent = parent
        .canonicalize()
        .with_context(|| format!("canonicalize beta workspace parent `{}`", parent.display()))?;
    let file_name = requested
        .file_name()
        .ok_or_else(|| anyhow!("beta workspace path must end with a directory name"))?;
    let beta_root = parent.join(file_name);

    if !beta_root.starts_with(&target_root) {
        bail!(
            "beta workspace `{}` must resolve inside `{}`",
            beta_root.display(),
            target_root.display()
        );
    }

    match fs::metadata(&beta_root) {
        Ok(metadata) if metadata.is_dir() => fs::remove_dir_all(&beta_root).with_context(|| {
            format!(
                "remove prior beta workspace under target `{}`",
                beta_root.display()
            )
        })?,
        Ok(_) => fs::remove_file(&beta_root).with_context(|| {
            format!(
                "remove prior beta workspace file under target `{}`",
                beta_root.display()
            )
        })?,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => {
            return Err(error).with_context(|| {
                format!("inspect prior beta workspace `{}`", beta_root.display())
            });
        }
    }

    let src_dir = beta_root.join("src");
    fs::create_dir_all(&src_dir)?;
    fs::write(beta_root.join("Cargo.toml"), FIXTURE_CARGO_TOML)?;
    fs::write(src_dir.join("main.rs"), FIXTURE_MAIN_RS)?;
    fs::write(beta_root.join("README.md"), FIXTURE_README)?;

    Ok(PreparedBetaWorkspace {
        root: beta_root.clone(),
        main_rs: beta_root.join("src/main.rs"),
    })
}

struct BetaWorkflowGateInputs<'a> {
    browse_status: &'a str,
    edit_save_status: &'a str,
    active_file_search_status: &'a str,
    workspace_search_status: &'a str,
    language_status: &'a str,
    terminal_status: &'a str,
    proposal_status: &'a str,
    diagnostics_export_written: bool,
    diagnostics_export_label: &'a str,
}

fn beta_workflow_gate_errors(input: BetaWorkflowGateInputs<'_>) -> Vec<String> {
    let mut errors = Vec::new();
    record_gate_error(
        &mut errors,
        input.browse_status.contains("refreshed explorer"),
        "browse workflow did not refresh explorer",
        input.browse_status,
    );
    record_gate_error(
        &mut errors,
        input.edit_save_status.contains("edited and saved"),
        "edit/save workflow did not save the isolated beta file",
        input.edit_save_status,
    );
    record_gate_error(
        &mut errors,
        input
            .active_file_search_status
            .contains("completed Completed"),
        "active-file search workflow did not complete with results",
        input.active_file_search_status,
    );
    record_gate_error(
        &mut errors,
        input
            .workspace_search_status
            .contains("completed Completed"),
        "workspace search workflow did not complete with results",
        input.workspace_search_status,
    );
    record_gate_error(
        &mut errors,
        input.language_status.contains("status=Cancelled")
            && input.language_status.contains("cancellations=1"),
        "language workflow did not record cancellation",
        input.language_status,
    );
    record_gate_error(
        &mut errors,
        input.terminal_status.contains("terminal_denied_expected"),
        "terminal workflow did not record expected denial",
        input.terminal_status,
    );
    record_gate_error(
        &mut errors,
        input.proposal_status.contains("preview=Some"),
        "proposal workflow did not record preview",
        input.proposal_status,
    );
    record_gate_error(
        &mut errors,
        input.diagnostics_export_written,
        "diagnostics export was not written",
        input.diagnostics_export_label,
    );
    errors
}

fn record_gate_error(errors: &mut Vec<String>, passed: bool, label: &str, status: &str) {
    if !passed {
        errors.push(format!("{label}: {status}"));
    }
}

fn run_browse_actions(runtime: &mut DesktopRuntime) -> String {
    let refresh = runtime.handle_action(DesktopAction::RefreshExplorer);
    let snapshot = runtime.projection_snapshot();
    if let Some(path) = snapshot
        .explorer_projection
        .nodes
        .iter()
        .find(|node| !node.children.is_empty())
        .map(|node| node.canonical_path.0.clone())
    {
        let _ = runtime.handle_action(DesktopAction::ToggleExplorerPath { path });
    }
    let node_count = runtime
        .projection_snapshot()
        .explorer_projection
        .nodes
        .len();
    match refresh {
        Ok(DesktopWorkflowOutcome::ExplorerRefreshed) => {
            format!("refreshed explorer nodes={node_count}")
        }
        Ok(other) => format!("unexpected browse outcome {other:?} nodes={node_count}"),
        Err(error) => format!("error {error}"),
    }
}

fn run_edit_save_actions(
    runtime: &mut DesktopRuntime,
    main_rs: &Path,
    errors: &mut Vec<String>,
) -> String {
    let edit = runtime.handle_action(DesktopAction::InsertText {
        text: EDIT_TEXT.to_string(),
        at: position(0),
    });
    let save = runtime.handle_action(DesktopAction::SaveActive);
    let saved_text = fs::read_to_string(main_rs).unwrap_or_default();
    if !saved_text.starts_with(EDIT_TEXT) {
        errors.push("isolated beta file did not contain the saved beta edit".to_string());
    }
    match (edit, save) {
        (Ok(DesktopWorkflowOutcome::Edited), Ok(DesktopWorkflowOutcome::Saved)) => {
            "edited and saved isolated beta workspace file".to_string()
        }
        (Ok(_), Ok(DesktopWorkflowOutcome::SaveRejected(reason))) => {
            errors.push(format!("save rejected: {reason}"));
            "save_rejected".to_string()
        }
        (edit, save) => {
            errors.push(format!("unexpected edit/save outcomes: {edit:?} {save:?}"));
            "failed".to_string()
        }
    }
}

fn run_search_action(
    runtime: &mut DesktopRuntime,
    scope: SearchScopeProjection,
    query: &str,
) -> String {
    match runtime.handle_action(DesktopAction::RunSearch {
        scope,
        query: query.to_string(),
        limit: 20,
    }) {
        Ok(DesktopWorkflowOutcome::SearchUpdated) => {
            let projection = &runtime.projection_snapshot().search_projection;
            format!(
                "completed {:?} results={} omitted_files={} omitted_results={}",
                projection.status.kind,
                projection.results.len(),
                projection.omitted_file_count,
                projection.omitted_result_count
            )
        }
        Ok(other) => format!("unexpected {other:?}"),
        Err(error) => format!("error {error}"),
    }
}

fn run_language_actions(runtime: &mut DesktopRuntime) -> String {
    let _ = runtime.handle_action(DesktopAction::RequestCompletion {
        position: position(3),
    });
    let _ = runtime.handle_action(DesktopAction::CancelLanguageOperation {
        operation_id: "language:Completion:1".to_string(),
    });
    let language = &runtime.projection_snapshot().language_tooling_projection;
    format!(
        "status={:?} operations={} cancellations={} problems={}",
        language.status,
        language.operations.len(),
        language.cancellation_count,
        language.problems.len()
    )
}

fn run_terminal_actions(runtime: &mut DesktopRuntime) -> String {
    let _ = runtime.handle_action(DesktopAction::TerminalLaunch {
        command_label: "beta fixture check".to_string(),
    });
    let terminal = &runtime.projection_snapshot().terminal_panel_projection;
    if terminal.last_denial.is_some() {
        format!(
            "terminal_denied_expected status={:?} rows={} omitted={}",
            terminal.status.kind,
            terminal.output_rows.len(),
            terminal.scrollback.omitted_row_count
        )
    } else {
        format!(
            "terminal_status={:?} rows={} omitted={}",
            terminal.status.kind,
            terminal.output_rows.len(),
            terminal.scrollback.omitted_row_count
        )
    }
}

fn run_proposal_actions(runtime: &mut DesktopRuntime) -> String {
    let outcome = runtime.handle_action(DesktopAction::StartAiProposal {
        instruction_label: "beta fixture proposal".to_string(),
    });
    let proposal_id = match outcome {
        Ok(DesktopWorkflowOutcome::AssistedAiUpdated {
            proposal_id: Some(proposal_id),
            ..
        }) => proposal_id,
        Ok(other) => return format!("unexpected proposal start {other:?}"),
        Err(error) => return format!("error {error}"),
    };
    let _ = runtime.handle_action(DesktopAction::OpenProposalDetails { proposal_id });
    let preview = runtime.handle_action(DesktopAction::PreviewProposal { proposal_id });
    let snapshot = runtime.projection_snapshot();
    format!(
        "proposal={} preview={:?} ledger_rows={} selected={:?}",
        proposal_id.0,
        preview.ok(),
        snapshot.proposal_ledger_projection.rows.len(),
        snapshot.proposal_ledger_projection.selected_proposal_id
    )
}

fn position(byte_offset: u64) -> TextCoordinate {
    TextCoordinate {
        line: 0,
        character: byte_offset as u32,
        byte_offset: Some(byte_offset),
        utf16_offset: Some(byte_offset),
    }
}

fn beta_smoke_command(config: &BetaWorkflowConfig) -> String {
    [
        "cargo run -p devil-desktop -- --beta-smoke".to_string(),
        format!("--workspace {}", config.real_workspace_root.display()),
        format!("--beta-workspace {}", config.beta_workspace_root.display()),
        format!("--evidence {}", config.evidence_path.display()),
        format!("--session-state {}", config.session_state.display()),
        format!(
            "--diagnostics-export {}",
            config.diagnostics_export.display()
        ),
    ]
    .join(" ")
}

fn unsupported_surfaces() -> Vec<String> {
    vec![
        "Remote production GUI: unsupported".to_string(),
        "Collaboration GUI: unsupported".to_string(),
        "Plugin management GUI: unsupported".to_string(),
        "Hosted provider activation: unsupported".to_string(),
        "Signed installer: unsupported".to_string(),
        "Cross-platform parity: unsupported".to_string(),
        "Autonomous apply: unsupported".to_string(),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn beta_workflow_gate_errors_require_language_cancellation_and_diagnostics_export() {
        let errors = beta_workflow_gate_errors(BetaWorkflowGateInputs {
            browse_status: "refreshed explorer nodes=4",
            edit_save_status: "edited and saved isolated beta workspace file",
            active_file_search_status: "completed Completed results=2 omitted_files=0 omitted_results=0",
            workspace_search_status: "completed Completed results=5 omitted_files=0 omitted_results=0",
            language_status: "status=Running operations=1 cancellations=0 problems=0",
            terminal_status: "terminal_denied_expected status=Denied rows=0 omitted=0",
            proposal_status: "proposal=2 preview=Some(...) ledger_rows=2 selected=Some(ProposalId(2))",
            diagnostics_export_written: false,
            diagnostics_export_label: "target/gui-phase7-diagnostics.md",
        });

        assert!(
            errors
                .iter()
                .any(|error| error.contains("language workflow did not record cancellation"))
        );
        assert!(
            errors
                .iter()
                .any(|error| error.contains("diagnostics export was not written"))
        );
    }

    #[test]
    fn beta_workflow_gate_errors_pass_for_expected_smoke_statuses() {
        let errors = beta_workflow_gate_errors(BetaWorkflowGateInputs {
            browse_status: "refreshed explorer nodes=4",
            edit_save_status: "edited and saved isolated beta workspace file",
            active_file_search_status: "completed Completed results=2 omitted_files=0 omitted_results=0",
            workspace_search_status: "completed Completed results=5 omitted_files=0 omitted_results=0",
            language_status: "status=Cancelled operations=2 cancellations=1 problems=0",
            terminal_status: "terminal_denied_expected status=Denied rows=0 omitted=0",
            proposal_status: "proposal=2 preview=Some(...) ledger_rows=2 selected=Some(ProposalId(2))",
            diagnostics_export_written: true,
            diagnostics_export_label: "target/gui-phase7-diagnostics.md",
        });

        assert!(errors.is_empty());
    }
}
