use std::path::PathBuf;

use legion_desktop::windowed_e2e::WindowedGuiE2eConfig;
use legion_desktop::workflow::DesktopLaunchConfig;

#[test]
fn windowed_e2e_parses_without_enabling_beta_or_native_smoke() {
    let config = DesktopLaunchConfig::from_args([
        "--windowed-e2e".into(),
        "--workspace".into(),
        ".".into(),
        "--file".into(),
        "notes.txt".into(),
        "--report".into(),
        "target/windowed-gui/report.toml".into(),
    ])
    .expect("windowed e2e args should parse");

    assert!(config.windowed_e2e.is_some());
    assert!(config.smoke.is_none());
    assert!(config.beta.is_none());
    assert!(config.manual_perf.is_none());
    assert_eq!(
        config.windowed_e2e.unwrap().report_path,
        PathBuf::from("target/windowed-gui/report.toml")
    );
}

#[test]
fn windowed_e2e_cannot_combine_with_beta_smoke() {
    let error = DesktopLaunchConfig::from_args(["--windowed-e2e".into(), "--beta-smoke".into()])
        .expect_err("windowed e2e and beta-smoke must be mutually exclusive");
    assert!(error.to_string().contains("cannot be combined"));
}

#[test]
fn windowed_e2e_cannot_combine_with_smoke() {
    let error = DesktopLaunchConfig::from_args(["--windowed-e2e".into(), "--smoke".into()])
        .expect_err("windowed e2e and --smoke must be mutually exclusive");
    assert!(error.to_string().contains("cannot be combined"));
}

#[test]
fn windowed_e2e_report_toml_is_metadata_only_and_names_the_window() {
    let report = legion_desktop::windowed_e2e::WindowedGuiE2eReport {
        binary_path: "target/windowed-gui/package/legion-desktop.exe".to_string(),
        package_dir: "target/windowed-gui/package".to_string(),
        os: "windows".to_string(),
        arch: "x86_64".to_string(),
        git_sha: "deadbeef".to_string(),
        window_created: true,
        window_backend: "eframe::run_native".to_string(),
        open: "passed".to_string(),
        edit: "passed".to_string(),
        save: "passed".to_string(),
        status: legion_desktop::windowed_e2e::WindowedGuiE2eStatus::Passed,
        errors: Vec::new(),
    };
    let toml = report.to_toml();
    assert!(toml.contains("task = \"GAP-01.1\""));
    assert!(toml.contains("not_golden_path_5 = true"));
    assert!(toml.contains("not_beta_smoke = true"));
    assert!(toml.contains("window_created = true"));
    assert!(toml.contains("window_backend = \"eframe::run_native\""));
    assert!(!toml.contains("WINDOWED_E2E_EDIT"));
    assert!(!toml.contains("SECRET"));
    let _ = WindowedGuiE2eConfig::default_report_path();
}
