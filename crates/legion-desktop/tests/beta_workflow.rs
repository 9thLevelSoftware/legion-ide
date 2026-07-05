use std::{
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

use legion_desktop::{
    beta::{
        BetaProposalMode, BetaSaveOutcome, BetaTerminalPolicyDecision, BetaWorkflowConfig,
        BetaWorkflowStatus, run_beta_workflow,
    },
    workflow::DesktopLaunchConfig,
};

static TEST_COUNTER: AtomicU64 = AtomicU64::new(0);

struct BetaTestPaths {
    prefix: String,
}

impl BetaTestPaths {
    fn new() -> Self {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be after epoch")
            .as_nanos();
        let id = TEST_COUNTER.fetch_add(1, Ordering::Relaxed);
        Self {
            prefix: format!("gui-phase7-beta-test-{}-{nanos}-{id}", std::process::id()),
        }
    }

    fn path(&self, name: &str) -> PathBuf {
        PathBuf::from("target").join(format!("{}-{name}", self.prefix))
    }
}

impl Drop for BetaTestPaths {
    fn drop(&mut self) {
        cleanup_test_paths("target", &self.prefix);
    }
}

fn cleanup_test_paths(target_root: impl AsRef<Path>, prefix: &str) {
    let target_root = target_root.as_ref();
    let Ok(entries) = fs::read_dir(target_root) else {
        return;
    };
    for entry in entries.flatten() {
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if !name.starts_with(prefix) {
            continue;
        }
        let path = entry.path();
        if path.is_dir() {
            let _ = fs::remove_dir_all(path);
        } else {
            let _ = fs::remove_file(path);
        }
    }
    let _ = fs::remove_dir(target_root);
}

fn beta_config(
    paths: &BetaTestPaths,
    beta_workspace: PathBuf,
    evidence_path: PathBuf,
) -> BetaWorkflowConfig {
    BetaWorkflowConfig::new(
        PathBuf::from("."),
        beta_workspace,
        evidence_path,
        paths.path("session.json"),
        paths.path("diagnostics.md"),
    )
    .expect("beta config should be valid")
}

#[test]
fn beta_workflow_runs_through_desktop_runtime_and_writes_metadata_evidence() {
    let paths = BetaTestPaths::new();
    let beta_workspace = paths.path("workspace");
    let evidence = paths.path("evidence.md");
    let report = run_beta_workflow(beta_config(
        &paths,
        beta_workspace.clone(),
        evidence.clone(),
    ))
    .expect("beta workflow should pass");

    assert_eq!(report.status, BetaWorkflowStatus::Passed);
    // Assert the typed outcome fields directly rather than scraping prose status.
    assert_eq!(report.save_outcome, BetaSaveOutcome::Saved);
    assert_eq!(report.terminal_decision, BetaTerminalPolicyDecision::Denied);
    assert_eq!(report.proposal_mode, BetaProposalMode::PreviewOnly);
    assert!(report.errors.is_empty());
    // Prose status strings remain populated for human-facing evidence only.
    assert!(report.edit_save_status.contains("saved"));
    assert!(report.terminal_status.contains("denied"));
    assert!(report.proposal_status.contains("preview"));

    let evidence_text = fs::read_to_string(&evidence).expect("evidence should be written");
    assert!(evidence_text.contains("status: passed"));
    assert!(evidence_text.contains("metadata-only: true"));
    assert!(evidence_text.contains("unsupported_surfaces"));
    assert!(!evidence_text.contains("println!"));
    assert!(!evidence_text.contains("metadata-only beta edit"));

    let saved_main = fs::read_to_string(beta_workspace.join("src/main.rs"))
        .expect("isolated beta fixture should be saved");
    assert!(saved_main.starts_with("// metadata-only beta edit"));
}

#[test]
fn beta_workflow_rejects_write_workspace_outside_target() {
    let paths = BetaTestPaths::new();
    let outside_target =
        std::env::temp_dir().join(format!("legion-phase7-beta-outside-{}", std::process::id()));
    let evidence = paths.path("outside-evidence.md");

    let error = run_beta_workflow(beta_config(&paths, outside_target, evidence.clone()))
        .expect_err("outside target beta workspace must be rejected");

    assert!(error.to_string().contains("blocked"));
    // The blocked report is not returned (the run bails), so its blocked status
    // is asserted through the persisted markdown evidence.
    let evidence_text = fs::read_to_string(&evidence).expect("blocked evidence should be written");
    assert!(evidence_text.contains("status: blocked"));
}

#[test]
fn beta_workflow_failed_report_exits_non_successfully_and_keeps_evidence() {
    let paths = BetaTestPaths::new();
    let beta_workspace = paths.path("workspace");
    let evidence = paths.path("failed-evidence.md");
    let diagnostics_directory = paths.path("diagnostics-directory");
    fs::create_dir_all(&diagnostics_directory).expect("diagnostics directory should be created");
    let config = BetaWorkflowConfig::new(
        PathBuf::from("."),
        beta_workspace,
        evidence.clone(),
        paths.path("session.json"),
        diagnostics_directory,
    )
    .expect("beta config should be valid");

    let error =
        run_beta_workflow(config).expect_err("failed beta workflow status must return an error");

    assert!(error.to_string().contains("failed"));
    let evidence_text = fs::read_to_string(&evidence).expect("failed evidence should be written");
    assert!(evidence_text.contains("status: failed"));
    assert!(evidence_text.contains("diagnostics export was not written"));
}

#[test]
fn desktop_launch_config_parses_beta_mode_without_enabling_native_smoke() {
    let config = DesktopLaunchConfig::from_args([
        "--beta-smoke".into(),
        "--workspace".into(),
        ".".into(),
        "--beta-workspace".into(),
        "target/gui-phase7-beta-workspace".into(),
        "--evidence".into(),
        "target/gui-phase7-beta-evidence.md".into(),
    ])
    .expect("beta launch args should parse");

    assert!(config.beta.is_some());
    assert!(config.smoke.is_none());
}

#[test]
fn desktop_launch_config_rejects_combined_smoke_modes() {
    let error = DesktopLaunchConfig::from_args(["--smoke".into(), "--beta-smoke".into()])
        .expect_err("native and beta smoke modes must be mutually exclusive");

    assert!(error.to_string().contains("cannot be combined"));
}
