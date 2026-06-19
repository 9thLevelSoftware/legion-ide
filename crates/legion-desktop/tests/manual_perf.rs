use std::{
    ffi::OsString,
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

use legion_desktop::{
    manual_perf::{MANUAL_RENDERER_SCENARIO, ManualPerfConfig, run_manual_perf},
    workflow::DesktopLaunchConfig,
};

static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

struct TempWorkspace {
    root: PathBuf,
}

impl TempWorkspace {
    fn new() -> Self {
        let temp_root = std::env::temp_dir();
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be after epoch")
            .as_nanos();
        let id = TEMP_COUNTER.fetch_add(1, Ordering::SeqCst);
        let root = temp_root.join(format!(
            "legion_desktop_manual_perf_{}_{}_{}",
            std::process::id(),
            nanos,
            id
        ));
        fs::create_dir(&root).expect("temp workspace should be created");
        Self { root }
    }

    fn path(&self) -> &Path {
        &self.root
    }

    fn write(&self, name: &str, content: &str) -> PathBuf {
        let path = self.root.join(name);
        fs::write(&path, content).expect("temp file should be written");
        path
    }
}

impl Drop for TempWorkspace {
    fn drop(&mut self) {
        let temp_root = std::env::temp_dir();
        let file_name = self.root.file_name().and_then(|name| name.to_str());
        if self.root.starts_with(&temp_root)
            && file_name.is_some_and(|name| name.starts_with("legion_desktop_manual_perf_"))
        {
            let _ = fs::remove_dir_all(&self.root);
        }
    }
}

#[test]
fn manual_perf_runs_renderer_backed_edit_and_writes_metadata_report() {
    let workspace = TempWorkspace::new();
    let target = workspace.write(
        "manual.txt",
        "alpha\nbeta\ngamma\ndelta\nepsilon\nzeta\neta\ntheta\n",
    );
    let report = workspace.path().join("manual_renderer_perf.toml");
    let config = ManualPerfConfig::new(
        workspace.path().to_path_buf(),
        Some(target),
        report.clone(),
        2,
        10_000,
        10_000,
        10_000,
    )
    .expect("manual perf config should validate");

    run_manual_perf(config).expect("manual perf report should be written");

    let contents = fs::read_to_string(&report).expect("manual perf report should be readable");
    assert!(contents.contains("schema_version = 1"));
    assert!(contents.contains(&format!("scenario = \"{MANUAL_RENDERER_SCENARIO}\"")));
    assert!(contents.contains("workspace_root = "));
    assert!(contents.contains("report_path = "));
    assert!(contents.contains("sample_count = 2"));
    assert!(contents.contains("keypress_p50_micros = "));
    assert!(contents.contains("keypress_p95_micros = "));
    assert!(contents.contains("scroll_p95_micros = "));
    assert!(contents.contains("keypress_p50_budget_ms = 10000"));
    assert!(contents.contains("keypress_p95_budget_ms = 10000"));
    assert!(contents.contains("scroll_p95_budget_ms = 10000"));
    assert!(contents.contains("message = "));
    assert!(
        contents.contains("status = \"passed\"")
            || contents.contains("status = \"failed\"")
            || contents.contains("status = \"skipped\"")
    );
    if contents.contains("status = \"skipped\"") {
        assert!(contents.contains("blocked") || contents.contains("unavailable"));
    }
}

#[test]
fn manual_perf_launch_config_parses_cli_flags() {
    let config = DesktopLaunchConfig::from_args([
        OsString::from("--manual-perf"),
        OsString::from("--workspace"),
        OsString::from("."),
        OsString::from("--file"),
        OsString::from("Cargo.toml"),
        OsString::from("--perf-report"),
        OsString::from("target/perf-harness/manual_renderer_perf.toml"),
        OsString::from("--perf-samples"),
        OsString::from("3"),
    ])
    .expect("manual perf flags should parse");

    assert!(config.smoke.is_none());
    assert!(config.beta.is_none());
    let manual_perf = config
        .manual_perf
        .expect("manual perf config should be present");
    assert_eq!(manual_perf.workspace_root, PathBuf::from("."));
    assert_eq!(manual_perf.initial_file, Some(PathBuf::from("Cargo.toml")));
    assert_eq!(
        manual_perf.report_path,
        PathBuf::from("target/perf-harness/manual_renderer_perf.toml")
    );
    assert_eq!(manual_perf.sample_count, 3);
    assert_eq!(manual_perf.keypress_p50_budget_ms, 16);
    assert_eq!(manual_perf.keypress_p95_budget_ms, 32);
    assert_eq!(manual_perf.scroll_p95_budget_ms, 32);
}

#[test]
fn manual_perf_launch_config_rejects_smoke_combinations() {
    let smoke_error = DesktopLaunchConfig::from_args([
        OsString::from("--manual-perf"),
        OsString::from("--smoke"),
    ])
    .expect_err("manual perf and native smoke must be mutually exclusive");
    assert!(smoke_error.to_string().contains("--manual-perf"));

    let beta_error = DesktopLaunchConfig::from_args([
        OsString::from("--manual-perf"),
        OsString::from("--beta-smoke"),
    ])
    .expect_err("manual perf and beta smoke must be mutually exclusive");
    assert!(beta_error.to_string().contains("--manual-perf"));
}
