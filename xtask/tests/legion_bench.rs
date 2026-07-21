use xtask::legion_bench::{
    LegionBenchReport, LegionBenchRunMode, LegionBenchTaskKind, plan_default_legion_bench_suite,
    plan_legion_bench_report, read_report, verify_legion_bench_report, write_report,
};

#[test]
fn legion_bench_default_suite_has_twenty_tasks() {
    let suite = plan_default_legion_bench_suite();
    assert_eq!(suite.tasks.len(), 20);
}

#[test]
fn legion_bench_default_suite_covers_four_task_kinds() {
    let suite = plan_default_legion_bench_suite();
    let bug_fix = suite
        .tasks
        .iter()
        .filter(|task| task.kind == LegionBenchTaskKind::BugFix)
        .count();
    let test_add = suite
        .tasks
        .iter()
        .filter(|task| task.kind == LegionBenchTaskKind::TestAdd)
        .count();
    let refactor = suite
        .tasks
        .iter()
        .filter(|task| task.kind == LegionBenchTaskKind::Refactor)
        .count();
    let multi_file = suite
        .tasks
        .iter()
        .filter(|task| task.kind == LegionBenchTaskKind::MultiFileFeature)
        .count();

    assert_eq!(bug_fix, 5);
    assert_eq!(test_add, 5);
    assert_eq!(refactor, 5);
    assert_eq!(multi_file, 5);
}

#[test]
fn legion_bench_report_round_trip_preserves_baseline() {
    let suite = plan_default_legion_bench_suite();
    let report = plan_legion_bench_report(
        "legion-desktop",
        "feedface",
        LegionBenchRunMode::RecordedOffline,
        &suite,
    );
    assert_eq!(report.summary.total, 20);
    assert_eq!(report.summary.passed, 20);
    assert_eq!(report.summary.failed, 0);
    assert_eq!(report.summary.regressed, 0);
    assert_eq!(report.mode, LegionBenchRunMode::RecordedOffline);

    let temp_dir = tempfile_dir("round-trip");
    let path = write_report(&temp_dir, &report).expect("write bench report");
    let round_trip: LegionBenchReport = read_report(&path).expect("read bench report");
    assert_eq!(round_trip, report);
    verify_legion_bench_report(&round_trip, &suite).expect("baseline verification");
}

#[test]
fn legion_bench_verify_rejects_suite_fingerprint_mismatch() {
    let suite = plan_default_legion_bench_suite();
    let report = plan_legion_bench_report(
        "legion-desktop",
        "feedface",
        LegionBenchRunMode::RecordedOffline,
        &suite,
    );
    let mut mutated = suite.clone();
    mutated.tasks[0].objective.push_str(" (mutated)");

    let err = verify_legion_bench_report(&report, &mutated).expect_err("fingerprint should differ");
    assert!(err.contains("fingerprint"), "unexpected error: {err}");
}

#[test]
fn legion_bench_verify_rejects_tampered_summary_counts() {
    let suite = plan_default_legion_bench_suite();
    let mut report = plan_legion_bench_report(
        "legion-desktop",
        "feedface",
        LegionBenchRunMode::RecordedOffline,
        &suite,
    );
    // Tamper only with the summary aggregate; the per-task results are intact.
    report.summary.average_score = report.summary.average_score.wrapping_add(1);

    let err = verify_legion_bench_report(&report, &suite)
        .expect_err("tampered summary should be rejected");
    assert!(err.contains("summary"), "unexpected error: {err}");
}

#[test]
fn legion_bench_verify_rejects_tampered_task_definition() {
    let suite = plan_default_legion_bench_suite();
    let mut report = plan_legion_bench_report(
        "legion-desktop",
        "feedface",
        LegionBenchRunMode::RecordedOffline,
        &suite,
    );
    // Tamper with a non-fingerprinted-but-embedded task field; the suite
    // fingerprint still matches so only full equality can catch this.
    report.tasks[0].task.objective.push_str(" (tampered)");

    let err = verify_legion_bench_report(&report, &suite)
        .expect_err("tampered task definition should be rejected");
    assert!(
        err.contains("task definition mismatch"),
        "unexpected error: {err}"
    );
}

#[test]
fn legion_bench_report_tracks_run_mode_profile() {
    let suite = plan_default_legion_bench_suite();
    let recorded = plan_legion_bench_report(
        "legion-desktop",
        "feedface",
        LegionBenchRunMode::RecordedOffline,
        &suite,
    );
    let live = plan_legion_bench_report(
        "legion-desktop",
        "feedface",
        LegionBenchRunMode::LiveWeekly,
        &suite,
    );

    assert_eq!(recorded.provider_profile, suite.recorded_provider_profile);
    assert_eq!(live.provider_profile, suite.live_provider_profile);
    assert_eq!(recorded.schema_version, 2);
    assert_eq!(
        recorded.scoring_mode,
        xtask::legion_bench::SCORING_MODE_SYNTHETIC_BUDGET_ARITHMETIC
    );
    assert!(
        recorded.tasks[0]
            .score
            .notes
            .contains("synthetic=true")
            && recorded.tasks[0]
                .score
                .notes
                .contains("budget-derived placeholders"),
        "recorded task notes must self-identify synthetic scoring, got: {}",
        recorded.tasks[0].score.notes
    );
}

fn tempfile_dir(name: &str) -> std::path::PathBuf {
    use std::{
        fs,
        sync::atomic::{AtomicU64, Ordering},
        time::{SystemTime, UNIX_EPOCH},
    };

    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock before epoch")
        .as_nanos();
    let seq = COUNTER.fetch_add(1, Ordering::Relaxed);
    let root = std::env::temp_dir().join(format!("legion-bench-{name}-{nanos}-{seq}"));
    fs::create_dir_all(&root).expect("create temp dir");
    root
}
