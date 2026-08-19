use xtask::legion_bench::{
    LegionBenchReport, LegionBenchRunMode, LegionBenchTaskKind, LegionBenchTaskStatus,
    SCORING_MODE_LIVE_LOCAL, SCORING_MODE_RECORDED_REPLAY, read_report, verify_legion_bench_report,
    write_report,
};
use xtask::legion_bench_corpus::{CorpusTask, LiveRunInput, corpus_suite, parse_corpus_task};
use xtask::legion_bench_live::{
    DEFAULT_LIVE_ENDPOINT, resolve_live_config, score_live_task, skipped_holdout_score,
};
use xtask::legion_bench_recorded::{
    RecordedBaseline, cassette_set_hash, compare_to_baseline, expectations_from_report,
};

fn repo_root() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("workspace root")
        .to_path_buf()
}

fn in_repo_corpus() -> Vec<CorpusTask> {
    let corpus_dir = repo_root().join(xtask::legion_bench_corpus::DEFAULT_CORPUS_PATH);
    xtask::legion_bench_corpus::load_corpus(&corpus_dir).expect("load in-repo corpus")
}

/// P9.F1.T1/T4 corpus floor. The acceptance criteria name concrete minimums
/// (20-50 tasks, >=3 fixture repos, a held-out subset); without a test they
/// are prose that a later trim can silently violate.
#[test]
fn in_repo_corpus_meets_the_documented_size_floor() {
    let corpus = in_repo_corpus();
    let repos: std::collections::BTreeSet<&str> = corpus
        .iter()
        .map(|task| task.task.fixture_repo.as_str())
        .collect();
    let holdout = corpus.iter().filter(|task| task.live.holdout).count();

    assert!(
        (20..=50).contains(&corpus.len()),
        "corpus must hold 20-50 tasks, found {}",
        corpus.len()
    );
    assert!(
        repos.len() >= 3,
        "corpus must span at least 3 fixture repos, found {repos:?}"
    );
    assert!(holdout > 0, "corpus must reserve a held-out subset");
    assert!(
        holdout < corpus.len(),
        "corpus must leave tasks outside the holdout"
    );
}

/// Every fixture must carry a deterministic scoring rule — the explicit stop
/// condition on P9.F1.T1. "Deterministic" here means: an exit-code comparison
/// against a named command, run in a fresh checkout, with integer budgets.
/// Nothing in the scoring path may be a judgement call.
#[test]
fn every_corpus_task_has_a_deterministic_scoring_rule() {
    for task in in_repo_corpus() {
        let id = &task.task.id;
        assert!(
            !task.live.verification.command.trim().is_empty(),
            "{id}: verification command is the scoring rule and must be present"
        );
        assert!(
            task.live.verification.timeout_secs > 0,
            "{id}: an unbounded verification cannot produce a deterministic verdict"
        );
        assert!(
            task.task.gate_budget.max_diff_files > 0 && task.task.gate_budget.max_turns > 0,
            "{id}: budgets must be positive integers, not open-ended"
        );
        // `at_rest` records whether the command passes on the untouched
        // fixture, so a task that is green before the model runs cannot be
        // read as one the model solved. Only two values are decidable.
        if let Some(declared) = task.live.verification.at_rest.as_deref() {
            assert!(
                declared == "passes" || declared == "fails",
                "{id}: at_rest must be `passes` or `fails`, got `{declared}`"
            );
        }
    }

    // The structural half of the corpus-health gate, run in-process: it is
    // what proves each task's rule can distinguish a working agent from one
    // that did nothing, and running it here means a bad task fails
    // `cargo test` rather than only the xtask command.
    let corpus_dir = repo_root().join(xtask::legion_bench_corpus::DEFAULT_CORPUS_PATH);
    let health = xtask::legion_bench_corpus_health::check_corpus(&corpus_dir, &repo_root(), false)
        .expect("corpus health check runs");
    let unhealthy: Vec<_> = health
        .iter()
        .filter(|task| !task.problems.is_empty())
        .collect();
    assert!(
        unhealthy.is_empty(),
        "unhealthy corpus tasks: {unhealthy:?}"
    );
}

#[test]
fn corpus_report_round_trips_and_verifies() {
    let corpus = in_repo_corpus();
    let suite = corpus_suite(&corpus);
    let report = replayed_report(&corpus, &suite);

    let temp_dir = tempfile_dir("round-trip");
    let path = write_report(&temp_dir, &report).expect("write bench report");
    let round_trip: LegionBenchReport = read_report(&path).expect("read bench report");
    assert_eq!(round_trip, report);
    verify_legion_bench_report(&round_trip, &suite).expect("baseline verification");
}

#[test]
fn legion_bench_verify_rejects_suite_fingerprint_mismatch() {
    let corpus = in_repo_corpus();
    let suite = corpus_suite(&corpus);
    let report = replayed_report(&corpus, &suite);
    let mut mutated = suite.clone();
    mutated.tasks[0].objective.push_str(" (mutated)");

    let err = verify_legion_bench_report(&report, &mutated).expect_err("fingerprint should differ");
    assert!(err.contains("fingerprint"), "unexpected error: {err}");
}

#[test]
fn legion_bench_verify_rejects_tampered_summary_counts() {
    let corpus = in_repo_corpus();
    let suite = corpus_suite(&corpus);
    let mut report = replayed_report(&corpus, &suite);
    // Tamper only with the summary aggregate; the per-task results are intact.
    report.summary.average_score = report.summary.average_score.wrapping_add(1);

    let err = verify_legion_bench_report(&report, &suite)
        .expect_err("tampered summary should be rejected");
    assert!(err.contains("summary"), "unexpected error: {err}");
}

#[test]
fn legion_bench_verify_rejects_tampered_task_definition() {
    let corpus = in_repo_corpus();
    let suite = corpus_suite(&corpus);
    let mut report = replayed_report(&corpus, &suite);
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

// ─── Recorded-mode report shape and regression gate ──────────────────────────

/// Build a recorded-replay report over the whole corpus with measured-looking
/// metrics. Nothing here is a *score derived from the budget*: the numbers are
/// inputs, exactly as the runner's measurements are.
fn replayed_report(
    corpus: &[CorpusTask],
    suite: &xtask::legion_bench::LegionBenchSuite,
) -> LegionBenchReport {
    let tasks: Vec<_> = corpus
        .iter()
        .map(|task| xtask::legion_bench::LegionBenchTaskResult {
            task: task.task.clone(),
            score: score_live_task(
                task,
                &sample_raw_result(&task.task.id),
                SCORING_MODE_RECORDED_REPLAY,
            ),
        })
        .collect();
    let mut report = LegionBenchReport {
        schema_version: xtask::legion_bench::BENCH_SCHEMA_VERSION,
        package_name: "legion-desktop".to_string(),
        measured_at_utc: "2026-08-19T00:00:00Z".to_string(),
        git_sha: "feedface".to_string(),
        mode: LegionBenchRunMode::RecordedOffline,
        provider_profile: "recorded:qwen2.5-coder:14b@governed".to_string(),
        scoring_mode: SCORING_MODE_RECORDED_REPLAY.to_string(),
        suite_name: suite.suite_name.clone(),
        suite_fingerprint: suite.suite_fingerprint.clone(),
        summary: Default::default(),
        tasks,
    };
    report.summary.total = report.tasks.len();
    let mut total_score = 0_u32;
    for task in &report.tasks {
        match task.score.status {
            LegionBenchTaskStatus::Passed => report.summary.passed += 1,
            LegionBenchTaskStatus::Failed => report.summary.failed += 1,
            LegionBenchTaskStatus::Skipped => report.summary.skipped += 1,
        }
        if task.score.status != LegionBenchTaskStatus::Skipped {
            total_score += u32::from(task.score.score);
        }
    }
    let graded = report.summary.total - report.summary.skipped;
    if graded > 0 {
        report.summary.average_score = total_score / graded as u32;
    }
    report
}

#[test]
fn recorded_report_declares_real_execution_not_synthetic_arithmetic() {
    let corpus = in_repo_corpus();
    let suite = corpus_suite(&corpus);
    let report = replayed_report(&corpus, &suite);
    let toml_text = toml::to_string_pretty(&report).expect("serialize recorded report");

    assert!(toml_text.contains("schema_version = 3"));
    assert!(toml_text.contains("scoring_mode = \"recorded_replay_execution\""));
    assert!(toml_text.contains("mode = \"recorded_offline\""));
    // The synthetic vocabulary must be gone from the report entirely: a report
    // that still says "synthetic" is one whose numbers were not measured.
    assert!(
        !toml_text.contains("synthetic"),
        "recorded reports must not describe themselves as synthetic"
    );
    // Measured execution metrics are what a recorded report is for.
    assert!(toml_text.contains("[tasks.score.live]"));
    assert!(toml_text.contains("task_success"));
}

/// The regression gate itself: a recorded run that differs from the committed
/// expectations must be reported as a difference, per task.
#[test]
fn recorded_baseline_comparison_catches_a_moved_result() {
    let corpus = in_repo_corpus();
    let suite = corpus_suite(&corpus);
    let report = replayed_report(&corpus, &suite);
    let baseline = RecordedBaseline {
        schema_version: xtask::legion_bench_recorded::BASELINE_SCHEMA_VERSION,
        model: "qwen2.5-coder:14b".to_string(),
        arm: "governed".to_string(),
        endpoint: "test".to_string(),
        recorded_at_utc: "2026-08-19T00:00:00Z".to_string(),
        suite_fingerprint: suite.suite_fingerprint.clone(),
        cassette_set_hash: "sha256:unused-by-this-comparison".to_string(),
        tasks: expectations_from_report(&report),
    };
    compare_to_baseline(&report, &baseline).expect("an unchanged run matches its own baseline");

    // One task now takes an extra turn — nothing else moves.
    let mut moved = report.clone();
    moved.tasks[0].score.turns += 1;
    let problems =
        compare_to_baseline(&moved, &baseline).expect_err("a moved result must be reported");
    assert_eq!(problems.len(), 1, "only one task moved: {problems:?}");
    assert!(
        problems[0].contains(&report.tasks[0].task.id),
        "the difference must name the task: {problems:?}"
    );

    // A corpus change invalidates the whole baseline, not one row.
    let mut recorpused = report.clone();
    recorpused.suite_fingerprint = "bench-suite-v1:0000000000000000".to_string();
    let problems =
        compare_to_baseline(&recorpused, &baseline).expect_err("fingerprint drift must be caught");
    assert!(
        problems.iter().any(|p| p.contains("suite fingerprint")),
        "{problems:?}"
    );
}

/// An edited cassette must change the set hash. Without this the "recorded"
/// half of recorded mode is unpinned: anyone could hand-write a tape that
/// makes the suite look better.
#[test]
fn cassette_set_hash_changes_when_a_tape_changes() {
    let dir = tempfile_dir("cassette-hash");
    let ids = vec!["a".to_string(), "b".to_string()];
    std::fs::write(dir.join("a.json"), "{\"schema_version\":1}\n").expect("write a");
    std::fs::write(dir.join("b.json"), "{\"schema_version\":1}\n").expect("write b");
    let before = cassette_set_hash(&dir, &ids).expect("hash");

    std::fs::write(dir.join("b.json"), "{\"schema_version\":1,\"x\":1}\n").expect("rewrite b");
    let after = cassette_set_hash(&dir, &ids).expect("hash");
    assert_ne!(before, after, "an edited tape must change the set hash");

    // Same length, different bytes: a hash that only covered file sizes would
    // pass everything above and still let a tape be rewritten in place.
    std::fs::write(dir.join("b.json"), "{\"schema_version\":1,\"x\":2}\n").expect("rewrite b");
    assert_ne!(
        after,
        cassette_set_hash(&dir, &ids).expect("hash"),
        "the hash must cover cassette contents, not just their size"
    );
    std::fs::write(dir.join("b.json"), "{\"schema_version\":1,\"x\":1}\n").expect("restore b");

    // CRLF is a checkout artifact, not a content change.
    std::fs::write(dir.join("b.json"), "{\"schema_version\":1,\"x\":1}\r\n").expect("crlf b");
    assert_eq!(
        after,
        cassette_set_hash(&dir, &ids).expect("hash"),
        "line-ending normalization must not move the hash"
    );

    // A missing tape is an error, not a hash over fewer files.
    std::fs::remove_file(dir.join("b.json")).expect("remove b");
    assert!(cassette_set_hash(&dir, &ids).is_err());
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

#[test]
fn live_runner_input_never_serializes_provider_credentials() {
    let stale_input = r#"
schema_version = 1
endpoint = "https://provider.example/v1"
model = "provider-model"
api_key = "sk-plaintext-secret"
tasks = []
"#;
    let input: LiveRunInput = toml::from_str(stale_input).expect("parse legacy live runner input");

    let serialized = toml::to_string_pretty(&input).expect("serialize live runner input");

    assert!(!serialized.contains("sk-plaintext-secret"));
    assert!(!serialized.contains("api_key"));
}

fn sample_raw_result(id: &str) -> xtask::legion_bench_corpus::LiveRunTaskResult {
    xtask::legion_bench_corpus::LiveRunTaskResult {
        id: id.to_string(),
        outcome: "completed".to_string(),
        task_success: true,
        tests_passed: true,
        // A fixture whose tests already passed would make `tests_passed`
        // meaningless, so the sample models the case the metric is about.
        tests_passed_at_rest: false,
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
        cassette_model: "qwen2.5-coder:14b".to_string(),
        cassette_arm: "governed".to_string(),
        cassette_exchanges: 5,
        cassette_drift: 0,
        error: None,
        notes: String::new(),
    }
}

#[test]
fn live_scoring_passes_within_budget_and_fails_over_budget() {
    let corpus_task = parse_corpus_task(CORPUS_TASK_FULL, "test").expect("parse");

    let good = sample_raw_result("bench-live-77");
    let score = score_live_task(&corpus_task, &good, SCORING_MODE_LIVE_LOCAL);
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
    let score = score_live_task(&corpus_task, &over_turns, SCORING_MODE_LIVE_LOCAL);
    assert_eq!(score.status, LegionBenchTaskStatus::Failed);

    // Verification failed → failed with fail_penalty applied.
    let mut tests_failed = sample_raw_result("bench-live-77");
    tests_failed.tests_passed = false;
    tests_failed.task_success = false;
    tests_failed.verification_exit = Some(101);
    let score = score_live_task(&corpus_task, &tests_failed, SCORING_MODE_LIVE_LOCAL);
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
            score: skipped_holdout_score(SCORING_MODE_LIVE_LOCAL),
        },
        xtask::legion_bench::LegionBenchTaskResult {
            task: second.task.clone(),
            score: score_live_task(&second, &failing, SCORING_MODE_LIVE_LOCAL),
        },
    ];
    let report = LegionBenchReport {
        schema_version: xtask::legion_bench::BENCH_SCHEMA_VERSION,
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

    // A replayed report with failures verifies too: the reference model's
    // failures are the baseline, and the baseline comparison gates them.
    let mut replayed = report.clone();
    replayed.scoring_mode = SCORING_MODE_RECORDED_REPLAY.to_string();
    verify_legion_bench_report(&replayed, &suite)
        .expect("replayed report with failures verifies structurally");

    // The scripted hostile suite is the one frozen-green report; a failure in
    // it means the generator and the gate disagree.
    let mut hostile = report.clone();
    hostile.scoring_mode = xtask::legion_bench::SCORING_MODE_SCRIPTED_HOSTILE.to_string();
    let err = verify_legion_bench_report(&hostile, &suite)
        .expect_err("hostile report with failures must be rejected");
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

/// A task that opts out of `require_tests_pass` must still be able to pass
/// when verification fails — otherwise the budget flag is decorative.
///
/// This guards two halves that have to agree: the runner keeps `task_success`
/// independent of verification, and the scorer applies the flag.
#[test]
fn live_scoring_honors_require_tests_pass_opt_out() {
    let opt_out =
        CORPUS_TASK_FULL.replace("require_tests_pass = true", "require_tests_pass = false");
    let corpus_task = parse_corpus_task(&opt_out, "test").expect("parse corpus task");

    let mut verification_failed = sample_raw_result("bench-live-77");
    verification_failed.tests_passed = false;
    verification_failed.verification_exit = Some(101);
    // The loop and proposals succeeded; only the optional verification did not.
    verification_failed.task_success = true;

    let score = score_live_task(&corpus_task, &verification_failed, SCORING_MODE_LIVE_LOCAL);
    assert_eq!(
        score.status,
        LegionBenchTaskStatus::Passed,
        "require_tests_pass = false must let a task pass despite failing verification"
    );

    // With the flag on, the same result fails — proving the flag is what moved it.
    let required = parse_corpus_task(CORPUS_TASK_FULL, "test").expect("parse corpus task");
    assert_eq!(
        score_live_task(&required, &verification_failed, SCORING_MODE_LIVE_LOCAL).status,
        LegionBenchTaskStatus::Failed
    );
}
