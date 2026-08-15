use xtask::legion_bench::{
    LegionBenchReport, LegionBenchRunMode, LegionBenchTaskKind, LegionBenchTaskStatus,
    SCORING_MODE_LIVE_LOCAL, plan_default_legion_bench_suite, plan_legion_bench_report,
    read_report, verify_legion_bench_report, write_report,
};
use xtask::legion_bench_corpus::{corpus_suite, parse_corpus_task};
use xtask::legion_bench_live::{
    DEFAULT_LIVE_ENDPOINT, resolve_live_config, score_live_task, skipped_holdout_score,
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
        recorded.tasks[0].score.notes.contains("synthetic=true")
            && recorded.tasks[0]
                .score
                .notes
                .contains("budget-derived placeholders"),
        "recorded task notes must self-identify synthetic scoring, got: {}",
        recorded.tasks[0].score.notes
    );
}

// ─── Recorded-mode report shape stability ────────────────────────────────────

#[test]
fn recorded_report_toml_shape_is_stable() {
    let suite = plan_default_legion_bench_suite();
    let report = plan_legion_bench_report(
        "legion-desktop",
        "feedface",
        LegionBenchRunMode::RecordedOffline,
        &suite,
    );
    let toml_text = toml::to_string_pretty(&report).expect("serialize recorded report");

    // Recorded reports keep their historical identity and stay honestly
    // self-labelled as synthetic.
    assert!(toml_text.contains("schema_version = 2"));
    assert!(toml_text.contains("scoring_mode = \"synthetic_budget_arithmetic\""));
    assert!(toml_text.contains("mode = \"recorded_offline\""));
    // The live-metrics table must NOT appear in recorded reports.
    assert!(
        !toml_text.contains("[tasks.score.live]"),
        "recorded reports must not contain live metrics tables"
    );
    assert!(!toml_text.contains("task_success"));
    // No task may be skipped in recorded mode.
    assert!(toml_text.contains("skipped = 0"));
    assert!(!toml_text.contains("status = \"skipped\""));

    // A pre-`skipped`-field report (the historical shape) must still parse.
    let legacy = toml_text.replace("skipped = 0\n", "");
    let parsed: LegionBenchReport = toml::from_str(&legacy).expect("legacy report parses");
    assert_eq!(parsed.summary.skipped, 0);
    verify_legion_bench_report(&parsed, &suite).expect("legacy-shaped report verifies");
}

// ─── Corpus task parsing ─────────────────────────────────────────────────────

const CORPUS_TASK_FULL: &str = r#"
schema_version = 1
id = "bench-live-77"
kind = "bug_fix"
fixture_repo = "fixtures/bugfix-count-markers"
holdout = true
prompt = "Fix the off-by-one."

[verification]
command = "cargo test --offline"
expected_exit = 2
timeout_secs = 120
expected_files = ["src/scratchpad.rs", "src/main.rs"]

[scope]
target_kind = "module"
target_path = "src"
allowed_tools = ["read", "edit-as-proposal"]
forbidden_paths = ["secrets.txt"]

[gate_budget]
require_tests_pass = true
max_diff_files = 2
max_turns = 6
max_cost_cents = 10

[scoring]
diff_file_penalty = 5
turn_penalty = 2
cost_half_cents_penalty = 1
fail_penalty = 50
"#;

#[test]
fn corpus_task_parses_all_fields_including_holdout_and_verification() {
    let task = parse_corpus_task(CORPUS_TASK_FULL, "test").expect("parse corpus task");
    assert_eq!(task.task.id, "bench-live-77");
    assert_eq!(task.task.kind, LegionBenchTaskKind::BugFix);
    assert_eq!(task.task.fixture_repo, "fixtures/bugfix-count-markers");
    assert_eq!(task.task.objective, "Fix the off-by-one.");
    assert_eq!(task.task.provider_profile, "live-local");
    assert_eq!(task.task.gate_budget.max_diff_files, 2);
    assert_eq!(task.task.gate_budget.max_turns, 6);
    assert_eq!(task.task.gate_budget.max_cost_cents, 10);

    assert!(task.live.holdout);
    assert_eq!(task.live.verification.command, "cargo test --offline");
    assert_eq!(task.live.verification.expected_exit, 2);
    assert_eq!(task.live.verification.timeout_secs, 120);
    assert_eq!(
        task.live.verification.expected_files,
        vec!["src/scratchpad.rs".to_string(), "src/main.rs".to_string()]
    );
    assert_eq!(task.live.scope.target_kind, "module");
    assert_eq!(task.live.scope.target_path.as_deref(), Some("src"));
    assert_eq!(task.live.scope.allowed_tools.len(), 2);
    assert_eq!(
        task.live.scope.forbidden_paths,
        vec!["secrets.txt".to_string()]
    );
    assert_eq!(task.live.scoring.diff_file_penalty, 5);
    assert_eq!(task.live.scoring.fail_penalty, 50);
}

#[test]
fn corpus_task_defaults_apply_when_optional_sections_are_omitted() {
    let minimal = r#"
schema_version = 1
id = "bench-live-min"
kind = "refactor"
fixture_repo = "fixtures/gp1-rust"
prompt = "Refactor."

[verification]
command = "cargo test"
"#;
    let task = parse_corpus_task(minimal, "test").expect("parse minimal corpus task");
    assert!(!task.live.holdout, "holdout defaults to false");
    assert_eq!(task.live.verification.expected_exit, 0);
    assert_eq!(task.live.verification.timeout_secs, 300);
    assert!(task.live.verification.expected_files.is_empty());
    assert_eq!(task.live.scope.target_kind, "repo");
    assert_eq!(
        task.live.scope.allowed_tools,
        vec!["read", "grep", "glob", "outline", "edit-as-proposal"]
    );
    assert_eq!(task.task.gate_budget.max_diff_files, 4);
    assert_eq!(task.live.scoring.diff_file_penalty, 4);
    assert_eq!(task.live.scoring.fail_penalty, 40);
}

#[test]
fn corpus_task_rejects_bad_definitions() {
    let hostile = CORPUS_TASK_FULL.replace("kind = \"bug_fix\"", "kind = \"hostile_eval\"");
    assert!(parse_corpus_task(&hostile, "test").is_err());

    let bad_kind = CORPUS_TASK_FULL.replace("kind = \"bug_fix\"", "kind = \"nonsense\"");
    assert!(parse_corpus_task(&bad_kind, "test").is_err());

    let bad_schema = CORPUS_TASK_FULL.replace("schema_version = 1", "schema_version = 9");
    assert!(parse_corpus_task(&bad_schema, "test").is_err());

    let bad_tool = CORPUS_TASK_FULL.replace("\"read\"", "\"browse-web\"");
    assert!(parse_corpus_task(&bad_tool, "test").is_err());

    let module_without_target = CORPUS_TASK_FULL.replace("target_path = \"src\"\n", "");
    assert!(parse_corpus_task(&module_without_target, "test").is_err());
}

#[test]
fn in_repo_corpus_loads_and_fingerprints_deterministically() {
    let repo_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("workspace root")
        .to_path_buf();
    let corpus_dir = repo_root.join(xtask::legion_bench_corpus::DEFAULT_CORPUS_PATH);
    let tasks = xtask::legion_bench_corpus::load_corpus(&corpus_dir).expect("load in-repo corpus");
    assert!(!tasks.is_empty());
    // Fixtures referenced by the corpus must exist.
    for task in &tasks {
        assert!(
            repo_root.join(&task.task.fixture_repo).is_dir(),
            "fixture missing for {}: {}",
            task.task.id,
            task.task.fixture_repo
        );
    }
    let suite_a = corpus_suite(&tasks);
    let suite_b = corpus_suite(&tasks);
    assert_eq!(suite_a.suite_fingerprint, suite_b.suite_fingerprint);
    assert_eq!(suite_a.suite_name, "legion-bench-live-v0");
}

// ─── Live-local config + scoring ─────────────────────────────────────────────

#[test]
fn live_config_requires_model_and_defaults_endpoint() {
    let err = resolve_live_config(None, None, None).expect_err("model must be required");
    assert!(
        err.contains("LEGION_BENCH_MODEL"),
        "error must name the missing env var: {err}"
    );

    let config = resolve_live_config(None, Some("qwen2.5-coder:7b".to_string()), None)
        .expect("model provided");
    assert_eq!(config.endpoint, DEFAULT_LIVE_ENDPOINT);
    assert_eq!(config.model, "qwen2.5-coder:7b");
    assert_eq!(
        config.api_key,
        xtask::legion_bench_live::PLACEHOLDER_API_KEY
    );

    let config = resolve_live_config(
        Some("http://localhost:8080/v1/".to_string()),
        Some("m".to_string()),
        Some("sk-test".to_string()),
    )
    .expect("full config");
    assert_eq!(config.endpoint, "http://localhost:8080/v1");
    assert_eq!(config.api_key, "sk-test");
}

fn sample_raw_result(id: &str) -> xtask::legion_bench_corpus::LiveRunTaskResult {
    xtask::legion_bench_corpus::LiveRunTaskResult {
        id: id.to_string(),
        outcome: "completed".to_string(),
        task_success: true,
        tests_passed: true,
        verification_exit: Some(0),
        proposals_total: 1,
        proposals_applied: 1,
        diff_files: 1,
        turns: 4,
        tool_calls: 6,
        duplicate_tool_calls: 1,
        retries: 0,
        context_tokens: Some(2048),
        generation_tokens: Some(512),
        wall_ms: 61_000,
        error: None,
        notes: String::new(),
    }
}

#[test]
fn live_scoring_passes_within_budget_and_fails_over_budget() {
    let corpus_task = parse_corpus_task(CORPUS_TASK_FULL, "test").expect("parse");

    let good = sample_raw_result("bench-live-77");
    let score = score_live_task(&corpus_task, &good);
    assert_eq!(score.status, LegionBenchTaskStatus::Passed);
    // weights: diff 1*5 + turns 4*2 = 13 → 87.
    assert_eq!(score.score, 87);
    let live = score.live.as_ref().expect("live metrics present");
    assert!(live.task_success);
    assert_eq!(live.tool_calls, 6);
    assert_eq!(live.duplicate_tool_calls, 1);
    assert_eq!(live.context_tokens, Some(2048));
    assert_eq!(live.generation_tokens, Some(512));
    assert_eq!(live.wall_ms, 61_000);

    // Over the turn budget (max_turns = 6) → failed even though tests passed.
    let mut over_turns = sample_raw_result("bench-live-77");
    over_turns.turns = 7;
    let score = score_live_task(&corpus_task, &over_turns);
    assert_eq!(score.status, LegionBenchTaskStatus::Failed);

    // Verification failed → failed with fail_penalty applied.
    let mut tests_failed = sample_raw_result("bench-live-77");
    tests_failed.tests_passed = false;
    tests_failed.task_success = false;
    tests_failed.verification_exit = Some(101);
    let score = score_live_task(&corpus_task, &tests_failed);
    assert_eq!(score.status, LegionBenchTaskStatus::Failed);
    assert_eq!(score.score, 37); // 87 - fail_penalty(50)
}

#[test]
fn live_report_with_failures_and_holdout_skips_verifies() {
    let corpus_task = parse_corpus_task(CORPUS_TASK_FULL, "test").expect("parse");
    let mut second = corpus_task.clone();
    second.task.id = "bench-live-78".to_string();
    second.live.holdout = false;
    let tasks = vec![corpus_task.clone(), second.clone()];
    let suite = corpus_suite(&tasks);

    let mut failing = sample_raw_result("bench-live-78");
    failing.tests_passed = false;
    failing.task_success = false;

    let results = vec![
        xtask::legion_bench::LegionBenchTaskResult {
            task: corpus_task.task.clone(),
            score: skipped_holdout_score(),
        },
        xtask::legion_bench::LegionBenchTaskResult {
            task: second.task.clone(),
            score: score_live_task(&second, &failing),
        },
    ];
    let report = LegionBenchReport {
        schema_version: 2,
        package_name: "legion-desktop".to_string(),
        measured_at_utc: "2026-08-15T00:00:00Z".to_string(),
        git_sha: "feedface".to_string(),
        mode: LegionBenchRunMode::LiveLocal,
        provider_profile: "live-local:test-model@http://127.0.0.1:11434/v1".to_string(),
        scoring_mode: SCORING_MODE_LIVE_LOCAL.to_string(),
        suite_name: suite.suite_name.clone(),
        suite_fingerprint: suite.suite_fingerprint.clone(),
        summary: Default::default(),
        tasks: results,
    };
    // Recompute the summary the way the live runner does (via round trip).
    let mut report = report;
    report.summary.total = 2;
    report.summary.skipped = 1;
    report.summary.failed = 1;
    report.summary.average_score = u32::from(report.tasks[1].score.score);

    // A live report with a failed task must still VERIFY (failures are data,
    // not baseline corruption); strictness is the caller's decision.
    verify_legion_bench_report(&report, &suite).expect("live report with failures verifies");

    // TOML round trip preserves skipped status + live metrics.
    let temp_dir = tempfile_dir("live-round-trip");
    let path = write_report(&temp_dir, &report).expect("write live report");
    let round_trip = read_report(&path).expect("read live report");
    assert_eq!(round_trip, report);
    assert_eq!(
        round_trip.tasks[0].score.status,
        LegionBenchTaskStatus::Skipped
    );
    assert!(round_trip.tasks[1].score.live.is_some());

    // But a recorded-mode report with failures must still be rejected.
    let mut synthetic = report.clone();
    synthetic.scoring_mode =
        xtask::legion_bench::SCORING_MODE_SYNTHETIC_BUDGET_ARITHMETIC.to_string();
    let err = verify_legion_bench_report(&synthetic, &suite)
        .expect_err("synthetic report with failures must be rejected");
    assert!(err.contains("regressions"), "unexpected error: {err}");

    // And unknown scoring modes remain rejected.
    let mut unknown = report.clone();
    unknown.scoring_mode = "vibes".to_string();
    let err = verify_legion_bench_report(&unknown, &suite).expect_err("unknown scoring mode");
    assert!(err.contains("scoring_mode"), "unexpected error: {err}");
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
