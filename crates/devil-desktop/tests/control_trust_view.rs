use std::{
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

use devil_app::{AppCommandOutcome, AppComposition};
use devil_desktop::view::DesktopProjectionViewModel;
use devil_protocol::{PrincipalId, WorkspaceTrustState};
use devil_ui::CommandDispatchIntent;

static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

struct TempWorkspace {
    root: PathBuf,
}

impl TempWorkspace {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "devil-desktop-control-trust-view-{}-{}",
            std::process::id(),
            TEMP_COUNTER.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&root).expect("create temp workspace");
        Self { root }
    }

    fn path(&self) -> &Path {
        &self.root
    }

    fn write(&self, name: &str, content: &str) -> PathBuf {
        let path = self.root.join(name);
        fs::write(&path, content).expect("write workspace file");
        path
    }
}

impl Drop for TempWorkspace {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn app_with_open_file(
    trust: WorkspaceTrustState,
    file_name: &str,
) -> (TempWorkspace, AppComposition) {
    let workspace = TempWorkspace::new();
    let target = workspace.write(file_name, "fn main() {}\n");
    let mut app = AppComposition::new();
    app.open_workspace(
        workspace.path(),
        trust,
        PrincipalId("desktop-control-trust-test".to_string()),
    )
    .expect("open workspace");
    app.open_file(target.to_string_lossy()).expect("open file");
    (workspace, app)
}

fn start_proposal(app: &mut AppComposition) {
    let outcome = app
        .dispatch_ui_intent(CommandDispatchIntent::StartAiProposal {
            instruction_label: "add control trust guard".to_string(),
        })
        .expect("start assisted proposal");
    assert!(matches!(outcome, AppCommandOutcome::AiRunStarted(_)));
}

#[test]
fn proposal_details_render_selected_ledger_and_diff_rows() {
    let (_workspace, mut app) = app_with_open_file(WorkspaceTrustState::Trusted, "proposal.rs");
    start_proposal(&mut app);

    let snapshot = app
        .shell_projection_snapshot("control trust")
        .expect("shell projection");
    let model = DesktopProjectionViewModel::from_snapshot(&snapshot);

    assert!(
        model
            .proposal_rows
            .iter()
            .any(|row| row.contains("proposal") && row.contains("payload=TextEdit"))
    );
    assert!(
        model
            .proposal_rows
            .iter()
            .any(|row| row.contains("diff:") && row.contains("hunks="))
    );
    assert!(
        model
            .proposal_rows
            .iter()
            .any(|row| row.contains("targets:") && row.contains("shown="))
    );
    assert!(
        model
            .proposal_rows
            .iter()
            .any(|row| row.contains("context:") && row.contains("items="))
    );
}

#[test]
fn trust_details_render_manifest_privacy_budget_approval_rollback_rows() {
    let (_workspace, mut app) = app_with_open_file(WorkspaceTrustState::Trusted, "trust.rs");
    start_proposal(&mut app);

    let snapshot = app
        .shell_projection_snapshot("control trust")
        .expect("shell projection");
    let model = DesktopProjectionViewModel::from_snapshot(&snapshot);

    assert!(
        model
            .trust_rows
            .iter()
            .any(|row| row.contains("context item"))
    );
    assert!(
        model
            .trust_rows
            .iter()
            .any(|row| row.contains("context permission"))
    );
    assert!(
        model
            .trust_rows
            .iter()
            .any(|row| row.contains("privacy record"))
    );
    assert!(
        model
            .trust_rows
            .iter()
            .any(|row| row.contains("permission budget"))
    );
    assert!(
        model
            .trust_rows
            .iter()
            .any(|row| row.contains("permission evaluation"))
    );
    assert!(
        model
            .trust_rows
            .iter()
            .any(|row| row.contains("approval gate"))
    );
    assert!(
        model
            .trust_rows
            .iter()
            .any(|row| row.contains("checkpoint:"))
    );
    assert!(
        model
            .trust_rows
            .iter()
            .any(|row| row.contains("rollback target"))
    );
}

#[test]
fn assisted_ai_details_render_provider_request_refusal_preview_rows() {
    let (_trusted_workspace, mut trusted) =
        app_with_open_file(WorkspaceTrustState::Trusted, "assistant.rs");
    start_proposal(&mut trusted);

    let trusted_snapshot = trusted
        .shell_projection_snapshot("control trust")
        .expect("trusted shell projection");
    let trusted_model = DesktopProjectionViewModel::from_snapshot(&trusted_snapshot);

    assert!(
        trusted_model
            .assistant_rows
            .iter()
            .any(|row| row.contains("assisted provider"))
    );
    assert!(
        trusted_model
            .assistant_rows
            .iter()
            .any(|row| row.contains("assisted route"))
    );
    assert!(
        trusted_model
            .assistant_rows
            .iter()
            .any(|row| row.contains("assisted request"))
    );
    assert!(
        trusted_model
            .assistant_rows
            .iter()
            .any(|row| row.contains("assisted preview") && row.contains("apply_ready=false"))
    );

    let (_untrusted_workspace, mut untrusted) =
        app_with_open_file(WorkspaceTrustState::Untrusted, "refused.rs");
    let outcome = untrusted
        .dispatch_ui_intent(CommandDispatchIntent::StartAiExplain {
            instruction_label: "explain refused route".to_string(),
        })
        .expect("start refused explain");
    assert!(matches!(outcome, AppCommandOutcome::AiRunStarted(_)));

    let untrusted_snapshot = untrusted
        .shell_projection_snapshot("control trust refused")
        .expect("untrusted shell projection");
    let untrusted_model = DesktopProjectionViewModel::from_snapshot(&untrusted_snapshot);

    assert!(
        untrusted_model
            .assistant_rows
            .iter()
            .any(|row| row.contains("assisted refusal") && row.contains("capability.denied"))
    );
}
