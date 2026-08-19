//! Tests for the perf-harness trend archive and regression gate (P8.F4.T3).

use std::path::PathBuf;

use xtask::perf_harness::{
    PerfReport, SkeletonKind, SkeletonMeasurement, SkeletonStatus, summarize_measurements,
};
use xtask::perf_trend::{
    BaselineStatus, TREND_DIR, allowed_ceiling, build_entry, detect_regressions_for_profile,
    missing_required_names, parse_baseline, read_entry, strict_failure, write_entry,
};

fn baseline_text() -> &'static str {
    "schema_version = 1\n\
     tolerance_percent = 60\n\
     \n\
     [[workload]]\n\
     name = \"p8.input_to_paint\"\n\
     os = \"windows\"\n\
     profile = \"reference\"\n\
     p50_micros = 2000\n\
     p95_micros = 2300\n\
     bytes_value = 0\n\
     note = \"reference machine\"\n\
     \n\
     [[workload]]\n\
     name = \"p8.memory_ceiling\"\n\
     os = \"windows\"\n\
     profile = \"reference\"\n\
     p50_micros = 0\n\
     p95_micros = 0\n\
     bytes_value = 24000000\n\
     note = \"reference machine\"\n"
}

fn measurement(name: &str, p50: u64, p95: u64, bytes: u64) -> SkeletonMeasurement {
    SkeletonMeasurement {
        name: name.to_string(),
        kind: SkeletonKind::ProductWorkload,
        fixture_bytes: 0,
        sample_count: 64,
        total_micros: p50 * 64,
        p50_micros: p50,
        p95_micros: p95,
        budget_millis: 32,
        status: SkeletonStatus::Passed,
        message: "test".to_string(),
        measured: true,
        bytes_value: bytes,
        synthetic_stand_in: false,
    }
}

fn report_with(measurements: Vec<SkeletonMeasurement>) -> PerfReport {
    let summary = summarize_measurements(&measurements);
    PerfReport {
        schema_version: 1,
        package_name: "legion-desktop".to_string(),
        measured_at_utc: "2026-08-19T10:11:12Z".to_string(),
        git_sha: "abcdef0123456789".to_string(),
        workload_kind: "product+skeleton".to_string(),
        os: "windows".to_string(),
        arch: "x86_64".to_string(),
        summary,
        skeletons: measurements,
    }
}

struct TempDir(PathBuf);

impl TempDir {
    fn new(name: &str) -> Self {
        let root = std::env::temp_dir().join(format!(
            "legion-perf-trend-{name}-{}-{:?}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ));
        std::fs::create_dir_all(&root).expect("create temp dir");
        Self(root)
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

// ── Tolerance arithmetic ─────────────────────────────────────────────────────

#[test]
fn allowed_ceiling_applies_the_tolerance() {
    assert_eq!(allowed_ceiling(1_000, 60), 1_600);
    assert_eq!(allowed_ceiling(1_000, 0), 1_000);
    // Saturating: a nonsense baseline must not wrap into a tiny ceiling that
    // fails everything.
    assert_eq!(allowed_ceiling(u64::MAX, 60), u64::MAX);
}

// ── Regression detection ─────────────────────────────────────────────────────

/// Drift inside the tolerance is not a regression, and drift outside it is.
/// Both halves are asserted so a detector that always returned "no regression"
/// (or always "regression") fails.
#[test]
fn regression_fires_only_outside_the_tolerance() {
    let baseline = parse_baseline(baseline_text()).expect("baseline parses");

    // 2300 * 1.6 = 3680; 3600 is drift, not a regression.
    let within = vec![measurement("p8.input_to_paint", 2_000, 3_600, 0)];
    let (status, regressions) =
        detect_regressions_for_profile(&baseline, &within, "windows", "reference");
    assert_eq!(status, BaselineStatus::Compared);
    assert!(
        regressions.is_empty(),
        "3.6ms against a 2.3ms baseline at 60% tolerance is drift, got {regressions:?}"
    );

    let outside = vec![measurement("p8.input_to_paint", 2_000, 9_000, 0)];
    let (_, regressions) =
        detect_regressions_for_profile(&baseline, &outside, "windows", "reference");
    assert_eq!(regressions.len(), 1, "got {regressions:?}");
    assert_eq!(regressions[0].metric, "p95_micros");
    assert_eq!(regressions[0].baseline, 2_300);
    assert_eq!(regressions[0].observed, 9_000);
    assert_eq!(regressions[0].allowed, 3_680);
}

#[test]
fn memory_regression_is_detected_on_bytes() {
    let baseline = parse_baseline(baseline_text()).expect("baseline parses");
    // 24_000_000 * 1.6 = 38_400_000.
    let within = vec![measurement("p8.memory_ceiling", 0, 0, 30_000_000)];
    let (_, regressions) =
        detect_regressions_for_profile(&baseline, &within, "windows", "reference");
    assert!(regressions.is_empty(), "got {regressions:?}");

    let leaked = vec![measurement("p8.memory_ceiling", 0, 0, 45_000_000)];
    let (_, regressions) =
        detect_regressions_for_profile(&baseline, &leaked, "windows", "reference");
    assert_eq!(regressions.len(), 1, "got {regressions:?}");
    assert_eq!(regressions[0].metric, "bytes_value");
}

/// A zero baseline means the metric is not tracked, not "must be zero".
#[test]
fn untracked_metrics_never_regress() {
    let baseline = parse_baseline(baseline_text()).expect("baseline parses");
    // p50/p95 baselines for the memory row are 0; any latency at all must not
    // be reported as an infinite regression.
    let noisy = vec![measurement("p8.memory_ceiling", 500_000, 900_000, 1_000)];
    let (_, regressions) =
        detect_regressions_for_profile(&baseline, &noisy, "windows", "reference");
    assert!(regressions.is_empty(), "got {regressions:?}");
}

/// An OS with no baseline is reported as such, not as a pass.
#[test]
fn missing_os_baseline_is_reported_rather_than_passed() {
    let baseline = parse_baseline(baseline_text()).expect("baseline parses");
    let huge = vec![measurement("p8.input_to_paint", 900_000, 900_000, 0)];
    let (status, regressions) =
        detect_regressions_for_profile(&baseline, &huge, "macos", "reference");
    assert_eq!(status, BaselineStatus::MissingForOs);
    assert!(
        regressions.is_empty(),
        "there is nothing to compare against; the gap is the finding"
    );
}

/// A hosted runner is several times slower than the workstation the baseline
/// came from, so grading one against the other would produce a gate that fires
/// on the hardware. The same OS with a different machine class must not
/// compare.
#[test]
fn a_different_machine_class_does_not_compare_against_the_reference_baseline() {
    let baseline = parse_baseline(baseline_text()).expect("baseline parses");
    let slow = vec![measurement("p8.input_to_paint", 8_000, 9_000, 0)];
    let (status, regressions) =
        detect_regressions_for_profile(&baseline, &slow, "windows", "github-hosted");
    assert_eq!(status, BaselineStatus::MissingForOs);
    assert!(regressions.is_empty(), "got {regressions:?}");

    // Same numbers, same OS, matching profile: now it is a regression. Without
    // this half, the assertion above would also pass for a detector that never
    // reports anything.
    let (status, regressions) =
        detect_regressions_for_profile(&baseline, &slow, "windows", "reference");
    assert_eq!(status, BaselineStatus::Compared);
    assert_eq!(regressions.len(), 2, "got {regressions:?}");
}

/// Unmeasured rows carry no number, so comparing them would report a fake
/// regression and bury the real message.
#[test]
fn unmeasured_rows_are_not_compared() {
    let baseline = parse_baseline(baseline_text()).expect("baseline parses");
    let mut absent = measurement("p8.input_to_paint", 0, 0, 0);
    absent.measured = false;
    let (_, regressions) =
        detect_regressions_for_profile(&baseline, &[absent], "windows", "reference");
    assert!(regressions.is_empty(), "got {regressions:?}");
}

// ── Strict gate ──────────────────────────────────────────────────────────────

/// `--strict` must fail on a regression even when every budget passed. This is
/// the P8.F4.T3 acceptance: a regressed trend entry reddens the build.
#[test]
fn strict_fails_on_regression_even_with_all_budgets_green() {
    let baseline = parse_baseline(baseline_text()).expect("baseline parses");
    let regressed = measurement("p8.input_to_paint", 2_000, 9_000, 0);
    let report = report_with(vec![regressed.clone()]);
    assert_eq!(
        report.summary.failed, 0,
        "the row's own budget verdict is Passed; only the trend says otherwise"
    );

    let (_, regressions) =
        detect_regressions_for_profile(&baseline, &report.skeletons, "windows", "reference");
    assert!(!regressions.is_empty());
    let required = vec!["p8.input_to_paint".to_string()];
    assert!(
        strict_failure(&report, &regressions, &required),
        "a regression must fail a strict run"
    );
    assert!(
        !strict_failure(&report, &[], &required),
        "with no regression and no budget failure, a strict run must pass — \
         otherwise the previous assertion proves nothing"
    );
}

/// `--strict` must fail when a required workload did not run, regardless of
/// budget policy. Report-only budgets are about noise, not a licence for a
/// workload to disappear on one OS.
#[test]
fn strict_fails_when_a_required_workload_did_not_run() {
    let mut absent = measurement("p8.fixture_100k_files", 0, 0, 0);
    absent.measured = false;
    absent.status = SkeletonStatus::Skipped;
    absent.message = "fixture generation failed: no space left on device".to_string();
    let report = report_with(vec![absent]);
    let required = vec!["p8.fixture_100k_files".to_string()];

    let missing = missing_required_names(&report, &required);
    assert_eq!(missing.len(), 1, "got {missing:?}");
    assert!(missing[0].contains("no space left"), "got {missing:?}");
    assert!(strict_failure(&report, &[], &required));
}

/// A workload absent from the report entirely is the same failure as one that
/// ran and could not measure.
#[test]
fn strict_fails_when_a_required_workload_is_absent_from_the_report() {
    let report = report_with(vec![measurement("p8.input_to_paint", 2_000, 2_300, 0)]);
    let required = vec!["p8.fixture_100k_files".to_string()];
    let missing = missing_required_names(&report, &required);
    assert_eq!(missing.len(), 1, "got {missing:?}");
    assert!(missing[0].contains("absent"), "got {missing:?}");
    assert!(strict_failure(&report, &[], &required));
}

#[test]
fn strict_fails_on_a_budget_failure() {
    let mut over = measurement("p8.input_to_paint", 2_000, 40_000, 0);
    over.status = SkeletonStatus::Failed;
    let report = report_with(vec![over]);
    assert_eq!(report.summary.failed, 1);
    assert!(strict_failure(&report, &[], &[]));
}

// ── Archive ──────────────────────────────────────────────────────────────────

#[test]
fn trend_entry_round_trips_through_the_archive() {
    let temp = TempDir::new("entry");
    let report = report_with(vec![
        measurement("p8.input_to_paint", 2_000, 2_300, 0),
        measurement("p8.memory_ceiling", 0, 0, 23_767_922),
    ]);
    let regressed = vec![xtask::perf_trend::TrendRegression {
        name: "p8.input_to_paint".to_string(),
        metric: "p95_micros".to_string(),
        baseline: 2_300,
        observed: 9_000,
        allowed: 3_680,
    }];
    let entry = build_entry(
        &report,
        "windows",
        "x86_64",
        BaselineStatus::Compared,
        60,
        regressed.clone(),
    );
    let path = write_entry(&temp.0, &entry).expect("write entry");
    assert!(
        path.starts_with(temp.0.join(TREND_DIR)),
        "entries must land inside the tracked trend directory, got {}",
        path.display()
    );
    assert!(
        path.file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.starts_with("windows-") && name.contains("abcdef012345")),
        "the file name must carry OS and revision, got {}",
        path.display()
    );

    let read_back = read_entry(&path).expect("read entry");
    assert_eq!(read_back, entry);
    assert_eq!(read_back.regression, regressed);
    assert_eq!(read_back.workload.len(), 2);
    assert_eq!(read_back.baseline_status, "compared");
}

/// The entry records how many workloads never ran, so an artifact can be read
/// without re-deriving it from the rows.
#[test]
fn trend_entry_counts_unmeasured_workloads() {
    let temp = TempDir::new("unmeasured");
    let mut absent = measurement("p8.fixture_100k_files", 0, 0, 0);
    absent.measured = false;
    let report = report_with(vec![measurement("p8.input_to_paint", 1, 1, 0), absent]);
    let entry = build_entry(
        &report,
        "linux",
        "x86_64",
        BaselineStatus::MissingForOs,
        60,
        Vec::new(),
    );
    assert_eq!(entry.unmeasured, 1);
    assert_eq!(entry.baseline_status, "missing_for_os");
    let path = write_entry(&temp.0, &entry).expect("write entry");
    assert_eq!(read_entry(&path).expect("read").unmeasured, 1);
}

// ── The tracked baseline ─────────────────────────────────────────────────────

/// The baseline committed to the repository must parse, and must not name a
/// workload the harness does not run. A baseline row for a workload that no
/// longer exists is a gate nobody notices has stopped firing.
#[test]
fn tracked_baseline_is_valid_and_names_only_real_workloads() {
    let workspace_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("xtask has a parent directory")
        .to_path_buf();
    let baseline =
        xtask::perf_trend::read_baseline(&workspace_root).expect("tracked baseline must parse");
    assert!(
        baseline.tolerance_percent > 0,
        "a zero tolerance would call every run a regression"
    );
    assert!(
        !baseline.workload.is_empty(),
        "an empty baseline is a regression gate that can never fire"
    );

    let known: Vec<String> = xtask::perf_workloads::product_workload_policies()
        .into_iter()
        .map(|policy| policy.name.to_string())
        .collect();
    for row in &baseline.workload {
        assert!(
            known.contains(&row.name),
            "baseline names `{}`, which is not a workload the harness runs; known: {known:?}",
            row.name
        );
        assert!(
            !row.note.trim().is_empty(),
            "baseline row `{}` ({}) has no note; a number nobody can trace is not reviewable",
            row.name,
            row.os
        );
        assert!(
            !row.profile.trim().is_empty(),
            "baseline row `{}` ({}) has no machine class; a number without one cannot be \
             compared safely",
            row.name,
            row.os
        );
    }
}
