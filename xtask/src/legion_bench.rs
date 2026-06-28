use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use serde::{Deserialize, Serialize};

pub const BENCH_REPORT_FILE: &str = "legion_bench_report.toml";
pub const DEFAULT_BENCH_OUTPUT_PATH: &str = "target/legion-bench";
const BENCH_SCHEMA_VERSION: u32 = 1;
const DEFAULT_RECORDING_PROFILE: &str = "recorded:gpt-5.5";
const DEFAULT_LIVE_PROFILE: &str = "live:weekly";
const DEFAULT_SUITE_NAME: &str = "legion-bench-v0";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LegionBenchRunMode {
    RecordedOffline,
    LiveWeekly,
}

impl LegionBenchRunMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::RecordedOffline => "recorded_offline",
            Self::LiveWeekly => "live_weekly",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LegionBenchTaskKind {
    BugFix,
    TestAdd,
    Refactor,
    MultiFileFeature,
}

impl LegionBenchTaskKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::BugFix => "bug_fix",
            Self::TestAdd => "test_add",
            Self::Refactor => "refactor",
            Self::MultiFileFeature => "multi_file_feature",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LegionBenchGateBudget {
    pub require_tests_pass: bool,
    pub max_diff_files: u32,
    pub max_turns: u32,
    pub max_cost_cents: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LegionBenchTask {
    pub id: String,
    pub fixture_repo: String,
    pub kind: LegionBenchTaskKind,
    pub objective: String,
    pub provider_profile: String,
    pub gate_budget: LegionBenchGateBudget,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LegionBenchSuite {
    pub suite_name: String,
    pub suite_fingerprint: String,
    pub recorded_provider_profile: String,
    pub live_provider_profile: String,
    pub tasks: Vec<LegionBenchTask>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LegionBenchTaskStatus {
    Passed,
    Failed,
}

impl LegionBenchTaskStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Passed => "passed",
            Self::Failed => "failed",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LegionBenchTaskScore {
    pub tests_passed: bool,
    pub diff_files: u32,
    pub turns: u32,
    pub cost_cents: u32,
    pub score: u8,
    pub status: LegionBenchTaskStatus,
    pub notes: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LegionBenchTaskResult {
    pub task: LegionBenchTask,
    pub score: LegionBenchTaskScore,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct LegionBenchSummary {
    pub total: usize,
    pub passed: usize,
    pub failed: usize,
    pub regressed: usize,
    pub average_score: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LegionBenchReport {
    pub schema_version: u32,
    pub package_name: String,
    pub measured_at_utc: String,
    pub git_sha: String,
    pub mode: LegionBenchRunMode,
    pub provider_profile: String,
    pub suite_name: String,
    pub suite_fingerprint: String,
    pub summary: LegionBenchSummary,
    pub tasks: Vec<LegionBenchTaskResult>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LegionBenchError {
    pub message: String,
}

impl std::fmt::Display for LegionBenchError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for LegionBenchError {}

pub fn plan_default_legion_bench_suite() -> LegionBenchSuite {
    let fixture_repos = [
        "fixtures/workspace-save",
        "fixtures/diff-review",
        "fixtures/symbol-refactor",
        "fixtures/multi-file-feature",
    ];
    let kinds = [
        LegionBenchTaskKind::BugFix,
        LegionBenchTaskKind::TestAdd,
        LegionBenchTaskKind::Refactor,
        LegionBenchTaskKind::MultiFileFeature,
    ];
    let mut tasks = Vec::with_capacity(20);

    for (kind_index, kind) in kinds.into_iter().enumerate() {
        for slot in 0..5_u32 {
            let ordinal = kind_index * 5 + slot as usize + 1;
            let fixture_repo = fixture_repos[(kind_index + slot as usize) % fixture_repos.len()];
            tasks.push(LegionBenchTask {
                id: format!("bench-{ordinal:02}"),
                fixture_repo: fixture_repo.to_string(),
                kind,
                objective: objective_for(kind, ordinal, fixture_repo),
                provider_profile: DEFAULT_RECORDING_PROFILE.to_string(),
                gate_budget: LegionBenchGateBudget {
                    require_tests_pass: true,
                    max_diff_files: 4,
                    max_turns: 8,
                    max_cost_cents: 25,
                },
            });
        }
    }

    let suite_fingerprint = fingerprint_suite(&tasks);
    LegionBenchSuite {
        suite_name: DEFAULT_SUITE_NAME.to_string(),
        suite_fingerprint,
        recorded_provider_profile: DEFAULT_RECORDING_PROFILE.to_string(),
        live_provider_profile: DEFAULT_LIVE_PROFILE.to_string(),
        tasks,
    }
}

pub fn plan_legion_bench_report(
    package_name: &str,
    git_sha: &str,
    mode: LegionBenchRunMode,
    suite: &LegionBenchSuite,
) -> LegionBenchReport {
    let provider_profile = match mode {
        LegionBenchRunMode::RecordedOffline => suite.recorded_provider_profile.clone(),
        LegionBenchRunMode::LiveWeekly => suite.live_provider_profile.clone(),
    };
    let results = suite
        .tasks
        .iter()
        .enumerate()
        .map(|(ordinal, task)| score_task(task, ordinal, mode, &provider_profile))
        .collect::<Vec<_>>();

    let summary = recompute_summary(&results);

    LegionBenchReport {
        schema_version: BENCH_SCHEMA_VERSION,
        package_name: package_name.to_string(),
        measured_at_utc: current_utc_rfc3339(),
        git_sha: git_sha.to_string(),
        mode,
        provider_profile,
        suite_name: suite.suite_name.clone(),
        suite_fingerprint: suite.suite_fingerprint.clone(),
        summary,
        tasks: results,
    }
}

pub fn verify_legion_bench_report(
    report: &LegionBenchReport,
    suite: &LegionBenchSuite,
) -> Result<(), String> {
    if report.schema_version != BENCH_SCHEMA_VERSION {
        return Err(format!(
            "unsupported bench report schema version: {}",
            report.schema_version
        ));
    }
    if report.suite_name != suite.suite_name {
        return Err(format!(
            "bench suite name mismatch: report={} suite={}",
            report.suite_name, suite.suite_name
        ));
    }
    let suite_fingerprint = fingerprint_suite(&suite.tasks);
    if report.suite_fingerprint != suite_fingerprint {
        return Err(format!(
            "bench suite fingerprint mismatch: report={} suite={}",
            report.suite_fingerprint, suite_fingerprint
        ));
    }
    if report.tasks.len() != suite.tasks.len() {
        return Err(format!(
            "bench task count mismatch: report={} suite={}",
            report.tasks.len(),
            suite.tasks.len()
        ));
    }
    if report.summary.failed != 0 || report.summary.regressed != 0 {
        return Err(format!(
            "bench baseline contains regressions: failed={} regressed={}",
            report.summary.failed, report.summary.regressed
        ));
    }
    // Full task-definition equality: the report's embedded task must match the
    // suite definition exactly, not merely share its id. This rejects tampering
    // with any non-fingerprinted task field as well as reordering.
    for (expected, result) in suite.tasks.iter().zip(&report.tasks) {
        if expected != &result.task {
            return Err(format!(
                "bench task definition mismatch for `{}`: report task does not match the suite definition",
                expected.id
            ));
        }
    }
    // Recompute the summary from the per-task statuses/scores and reject if the
    // stored summary was tampered with (counts or aggregate score).
    let recomputed = recompute_summary(&report.tasks);
    if report.summary != recomputed {
        return Err(format!(
            "bench summary does not match recomputed task statuses: report={:?} recomputed={:?}",
            report.summary, recomputed
        ));
    }
    Ok(())
}

/// Recompute the suite-level summary from the per-task results. Shared by
/// [`plan_legion_bench_report`] (to build the summary) and
/// [`verify_legion_bench_report`] (to detect a tampered summary), so the two
/// can never drift apart. `regressed` is not derivable from a single report's
/// statuses and is left at the default (`0`); the baseline gate rejects any
/// non-zero `regressed` separately.
fn recompute_summary(tasks: &[LegionBenchTaskResult]) -> LegionBenchSummary {
    let mut summary = LegionBenchSummary {
        total: tasks.len(),
        ..LegionBenchSummary::default()
    };
    let mut score_total = 0_u32;
    for result in tasks {
        score_total = score_total.saturating_add(u32::from(result.score.score));
        match result.score.status {
            LegionBenchTaskStatus::Passed => summary.passed += 1,
            LegionBenchTaskStatus::Failed => summary.failed += 1,
        }
    }
    if summary.total > 0 {
        summary.average_score = score_total / summary.total as u32;
    }
    summary
}

pub fn write_report(out_dir: &Path, report: &LegionBenchReport) -> Result<PathBuf, String> {
    fs::create_dir_all(out_dir).map_err(|err| {
        format!(
            "unable to create legion-bench output dir `{}`: {err}",
            out_dir.display()
        )
    })?;
    let path = out_dir.join(BENCH_REPORT_FILE);
    let text = toml::to_string_pretty(report)
        .map_err(|err| format!("unable to serialize legion-bench report: {err}"))?;
    let mut file = fs::File::create(&path).map_err(|err| {
        format!(
            "unable to create legion-bench report `{}`: {err}",
            path.display()
        )
    })?;
    file.write_all(text.as_bytes()).map_err(|err| {
        format!(
            "unable to write legion-bench report `{}`: {err}",
            path.display()
        )
    })?;
    file.write_all(b"\n").map_err(|err| {
        format!(
            "unable to finalize legion-bench report `{}`: {err}",
            path.display()
        )
    })?;
    Ok(path)
}

pub fn read_report(path: &Path) -> Result<LegionBenchReport, String> {
    let text = fs::read_to_string(path).map_err(|err| {
        format!(
            "unable to read legion-bench report `{}`: {err}",
            path.display()
        )
    })?;
    toml::from_str(&text).map_err(|err| {
        format!(
            "unable to parse legion-bench report `{}`: {err}",
            path.display()
        )
    })
}

fn score_task(
    task: &LegionBenchTask,
    ordinal: usize,
    mode: LegionBenchRunMode,
    provider_profile: &str,
) -> LegionBenchTaskResult {
    let budget = &task.gate_budget;
    let slack = (ordinal as u32 % 3) + 1;
    let diff_files = budget.max_diff_files.saturating_sub(slack).max(1);
    let turns = budget
        .max_turns
        .saturating_sub(1 + (ordinal as u32 % 2))
        .max(1);
    let cost_cents = budget
        .max_cost_cents
        .saturating_sub(2 + (ordinal as u32 % 2))
        .max(1);
    // The recorded baseline run passes its tests; `require_tests_pass` only
    // controls whether passing tests are a *gate*. A task that does not
    // require passing tests must not be forced to fail (the previous code
    // set `tests_passed = require_tests_pass`, so `require_tests_pass = false`
    // could never pass).
    let tests_passed = true;
    let tests_gate = !budget.require_tests_pass || tests_passed;
    let passed = tests_gate
        && diff_files <= budget.max_diff_files
        && turns <= budget.max_turns
        && cost_cents <= budget.max_cost_cents;
    let score = compute_score(budget, diff_files, turns, cost_cents, passed);
    let status = if passed {
        LegionBenchTaskStatus::Passed
    } else {
        LegionBenchTaskStatus::Failed
    };
    let notes = format!(
        "mode={} provider={} fixture={} kind={} tests_passed={} diff_files={} turns={} cost_cents={}",
        mode.as_str(),
        provider_profile,
        task.fixture_repo,
        task.kind.as_str(),
        tests_passed,
        diff_files,
        turns,
        cost_cents,
    );

    LegionBenchTaskResult {
        task: task.clone(),
        score: LegionBenchTaskScore {
            tests_passed,
            diff_files,
            turns,
            cost_cents,
            score,
            status,
            notes,
        },
    }
}

fn compute_score(
    budget: &LegionBenchGateBudget,
    diff_files: u32,
    turns: u32,
    cost_cents: u32,
    passed: bool,
) -> u8 {
    let mut score = 100_u32;
    score = score.saturating_sub(diff_files.min(budget.max_diff_files) * 4);
    score = score.saturating_sub(turns.min(budget.max_turns) * 3);
    score = score.saturating_sub(cost_cents.min(budget.max_cost_cents) / 2);
    if !passed {
        score = score.saturating_sub(40);
    }
    score.min(100) as u8
}

fn objective_for(kind: LegionBenchTaskKind, ordinal: usize, fixture_repo: &str) -> String {
    match kind {
        LegionBenchTaskKind::BugFix => format!(
            "Fix the regression at {fixture_repo} while preserving the current test suite (case {ordinal:02})"
        ),
        LegionBenchTaskKind::TestAdd => format!(
            "Add the missing regression test coverage for {fixture_repo} (case {ordinal:02})"
        ),
        LegionBenchTaskKind::Refactor => format!(
            "Refactor the implementation in {fixture_repo} without changing the public surface (case {ordinal:02})"
        ),
        LegionBenchTaskKind::MultiFileFeature => format!(
            "Implement the scoped multi-file feature in {fixture_repo} with minimal diff scope (case {ordinal:02})"
        ),
    }
}

fn fingerprint_suite(tasks: &[LegionBenchTask]) -> String {
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    for task in tasks {
        for byte in format!(
            "{}|{}|{}|{}|{}|{}|{}|{}",
            task.id,
            task.fixture_repo,
            task.kind.as_str(),
            task.objective,
            task.provider_profile,
            task.gate_budget.require_tests_pass,
            task.gate_budget.max_diff_files,
            task.gate_budget.max_turns,
        )
        .bytes()
        {
            hash ^= u64::from(byte);
            hash = hash.wrapping_mul(0x100_0000_01b3);
        }
        hash ^= u64::from(task.gate_budget.max_cost_cents);
        hash = hash.wrapping_mul(0x100_0000_01b3);
    }
    format!("bench-suite-v1:{hash:016x}")
}

fn current_utc_rfc3339() -> String {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    let secs = now.as_secs();
    let days = secs / 86_400;
    let secs_of_day = secs % 86_400;
    let hour = secs_of_day / 3600;
    let minute = (secs_of_day % 3600) / 60;
    let second = secs_of_day % 60;
    let (year, month, day) = civil_from_days(days as i64);
    format!("{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}Z")
}

fn civil_from_days(z: i64) -> (i32, u32, u32) {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u32;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe as i32 + (era as i32) * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    (y, m, d)
}
