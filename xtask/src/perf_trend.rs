//! Perf-harness trend archive and regression gate (P8.F4.T3).
//!
//! Every perf-harness run drops one entry into
//! `plans/evidence/perf-harness-trend/entries/`, and compares the run against
//! `plans/evidence/perf-harness-trend/baseline.toml`. The directory is in the
//! tracked tree so the baseline is reviewable in a diff: a performance budget
//! that can be changed without anyone seeing the change is not a budget.
//!
//! # What counts as a regression
//!
//! A workload regresses when its gated metric exceeds the baseline for the
//! same OS by more than `tolerance_percent`. The tolerance exists because
//! wall-clock measurements on a real machine move by tens of percent between
//! runs for reasons that have nothing to do with the code; it is deliberately
//! wide enough that a green run means something and narrow enough that a 2x
//! regression cannot hide inside it.
//!
//! # Why the baseline is per OS
//!
//! A Windows number and a macOS number for the same workload are different
//! measurements of different systems. Comparing a run against a baseline
//! recorded elsewhere would produce a gate that fires on the operating system
//! rather than on the change. An OS with no baseline row is reported as such —
//! in the printed output and in the archived entry — rather than passing
//! quietly.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::perf_harness::{PerfReport, SkeletonMeasurement, SkeletonStatus};

/// Tracked trend directory, relative to the workspace root.
pub const TREND_DIR: &str = "plans/evidence/perf-harness-trend";
/// Baseline file inside [`TREND_DIR`].
pub const BASELINE_FILE: &str = "baseline.toml";
/// Sub-directory inside [`TREND_DIR`] holding one entry per run.
pub const ENTRIES_DIR: &str = "entries";

/// Recorded reference numbers a run is compared against.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrendBaseline {
    pub schema_version: u32,
    /// How far above the baseline a measurement may drift before it counts as
    /// a regression.
    pub tolerance_percent: u64,
    #[serde(default)]
    pub workload: Vec<BaselineWorkload>,
}

/// One baseline row: a workload measured on one OS, on one class of machine.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BaselineWorkload {
    pub name: String,
    /// `std::env::consts::OS` value the baseline was recorded on.
    pub os: String,
    /// Class of machine the baseline was recorded on. See
    /// [`baseline_profile`]: a shared GitHub runner and a developer workstation
    /// are different machines, and comparing one against the other produces a
    /// gate that fires on the hardware rather than on the change.
    #[serde(default = "default_profile")]
    pub profile: String,
    #[serde(default)]
    pub p50_micros: u64,
    #[serde(default)]
    pub p95_micros: u64,
    #[serde(default)]
    pub bytes_value: u64,
    /// Where the number came from, so a reviewer can judge it.
    #[serde(default)]
    pub note: String,
}

/// Baseline rows written before profiles existed came from a developer
/// machine.
fn default_profile() -> String {
    "reference".to_string()
}

/// Which class of machine this run is happening on.
///
/// Derived from the environment rather than from a flag, because it is a fact
/// about where the run is, not a choice about how to grade it: `GITHUB_ACTIONS`
/// is set by the runner itself. A hosted runner is several times slower than a
/// developer workstation, so comparing its numbers against a workstation
/// baseline at any tolerance would produce a gate that fires on the hardware.
pub fn baseline_profile() -> &'static str {
    match std::env::var("GITHUB_ACTIONS") {
        Ok(value) if value.eq_ignore_ascii_case("true") => "github-hosted",
        _ => "reference",
    }
}

/// One workload's regression finding.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrendRegression {
    pub name: String,
    /// Which number regressed: `p50_micros`, `p95_micros`, or `bytes_value`.
    pub metric: String,
    pub baseline: u64,
    pub observed: u64,
    pub allowed: u64,
}

impl std::fmt::Display for TrendRegression {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} {} regressed: baseline {}, allowed {}, observed {}",
            self.name, self.metric, self.baseline, self.allowed, self.observed
        )
    }
}

/// Whether a baseline comparison could be made at all.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BaselineStatus {
    /// Baseline rows for this OS existed and were compared.
    Compared,
    /// No baseline row for this OS and machine class. Loud, not silent:
    /// recorded in the entry and printed, so the first run on a new OS or a new
    /// class of runner asks for a baseline instead of pretending to have
    /// passed one.
    MissingForOs,
}

impl BaselineStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Compared => "compared",
            Self::MissingForOs => "missing_for_os",
        }
    }
}

/// One archived run.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrendEntry {
    pub schema_version: u32,
    pub recorded_at_utc: String,
    pub git_sha: String,
    pub os: String,
    pub arch: String,
    /// Machine class this entry was recorded on; see [`baseline_profile`].
    pub profile: String,
    pub baseline_status: String,
    pub tolerance_percent: u64,
    pub total: usize,
    pub passed: usize,
    pub failed: usize,
    pub skipped: usize,
    pub unmeasured: usize,
    pub workload: Vec<TrendWorkload>,
    pub regression: Vec<TrendRegression>,
}

/// One workload row inside an entry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrendWorkload {
    pub name: String,
    pub kind: String,
    pub status: String,
    pub measured: bool,
    pub synthetic_stand_in: bool,
    pub p50_micros: u64,
    pub p95_micros: u64,
    pub bytes_value: u64,
    pub budget_millis: u64,
}

/// Read the tracked baseline.
pub fn read_baseline(workspace_root: &Path) -> Result<TrendBaseline, String> {
    let path = baseline_path(workspace_root);
    let text = std::fs::read_to_string(&path).map_err(|err| {
        format!(
            "unable to read perf trend baseline `{}`: {err}",
            path.display()
        )
    })?;
    parse_baseline(&text)
}

/// Parse a baseline document.
pub fn parse_baseline(text: &str) -> Result<TrendBaseline, String> {
    let baseline: TrendBaseline = toml::from_str(text)
        .map_err(|err| format!("unable to parse perf trend baseline: {err}"))?;
    if baseline.schema_version != 1 {
        return Err(format!(
            "perf trend baseline uses unsupported schema_version {}",
            baseline.schema_version
        ));
    }
    Ok(baseline)
}

pub fn baseline_path(workspace_root: &Path) -> PathBuf {
    workspace_root.join(TREND_DIR).join(BASELINE_FILE)
}

pub fn entries_dir(workspace_root: &Path) -> PathBuf {
    workspace_root.join(TREND_DIR).join(ENTRIES_DIR)
}

/// Compare a run's measurements against the baseline for `os`.
///
/// Unmeasured rows are not compared: they carry no number, and reporting them
/// as regressions would bury the real message, which is that the workload did
/// not run.
pub fn detect_regressions(
    baseline: &TrendBaseline,
    measurements: &[SkeletonMeasurement],
    os: &str,
) -> (BaselineStatus, Vec<TrendRegression>) {
    detect_regressions_for_profile(baseline, measurements, os, baseline_profile())
}

/// [`detect_regressions`] with the machine class named explicitly, so tests can
/// exercise both classes without mutating process-global environment state.
pub fn detect_regressions_for_profile(
    baseline: &TrendBaseline,
    measurements: &[SkeletonMeasurement],
    os: &str,
    profile: &str,
) -> (BaselineStatus, Vec<TrendRegression>) {
    let rows_for_os: Vec<&BaselineWorkload> = baseline
        .workload
        .iter()
        .filter(|row| row.os == os && row.profile == profile)
        .collect();
    if rows_for_os.is_empty() {
        return (BaselineStatus::MissingForOs, Vec::new());
    }

    let mut regressions = Vec::new();
    for measurement in measurements {
        if !measurement.measured {
            continue;
        }
        let Some(row) = rows_for_os.iter().find(|row| row.name == measurement.name) else {
            continue;
        };
        for (metric, base, observed) in [
            ("p50_micros", row.p50_micros, measurement.p50_micros),
            ("p95_micros", row.p95_micros, measurement.p95_micros),
            ("bytes_value", row.bytes_value, measurement.bytes_value),
        ] {
            // A zero baseline means the metric is not tracked for this
            // workload (a single-sample workload has no p95, a latency
            // workload has no byte value); there is nothing to regress from.
            if base == 0 {
                continue;
            }
            let allowed = allowed_ceiling(base, baseline.tolerance_percent);
            if observed > allowed {
                regressions.push(TrendRegression {
                    name: measurement.name.clone(),
                    metric: metric.to_string(),
                    baseline: base,
                    observed,
                    allowed,
                });
            }
        }
    }
    (BaselineStatus::Compared, regressions)
}

/// The largest value that is still not a regression.
///
/// Saturating on purpose: a baseline near `u64::MAX` is nonsense, and
/// wrapping it into a tiny ceiling would turn nonsense into a false gate.
pub fn allowed_ceiling(baseline: u64, tolerance_percent: u64) -> u64 {
    baseline.saturating_add(
        baseline
            .saturating_mul(tolerance_percent)
            .saturating_div(100),
    )
}

/// Build the entry archived for this run.
pub fn build_entry(
    report: &PerfReport,
    os: &str,
    arch: &str,
    baseline_status: BaselineStatus,
    tolerance_percent: u64,
    regressions: Vec<TrendRegression>,
) -> TrendEntry {
    TrendEntry {
        schema_version: 1,
        recorded_at_utc: report.measured_at_utc.clone(),
        git_sha: report.git_sha.clone(),
        os: os.to_string(),
        arch: arch.to_string(),
        profile: baseline_profile().to_string(),
        baseline_status: baseline_status.as_str().to_string(),
        tolerance_percent,
        total: report.summary.total,
        passed: report.summary.passed,
        failed: report.summary.failed,
        skipped: report.summary.skipped,
        unmeasured: report
            .skeletons
            .iter()
            .filter(|measurement| !measurement.measured)
            .count(),
        workload: report
            .skeletons
            .iter()
            .map(|measurement| TrendWorkload {
                name: measurement.name.clone(),
                kind: measurement.kind.as_str().to_string(),
                status: measurement.status.as_str().to_string(),
                measured: measurement.measured,
                synthetic_stand_in: measurement.synthetic_stand_in,
                p50_micros: measurement.p50_micros,
                p95_micros: measurement.p95_micros,
                bytes_value: measurement.bytes_value,
                budget_millis: measurement.budget_millis,
            })
            .collect(),
        regression: regressions,
    }
}

/// Write an entry into the tracked trend directory. Returns its path.
///
/// The file name carries OS, revision, and timestamp so entries from the three
/// CI jobs land side by side without colliding and can be read without opening
/// them.
pub fn write_entry(workspace_root: &Path, entry: &TrendEntry) -> Result<PathBuf, String> {
    let dir = entries_dir(workspace_root);
    std::fs::create_dir_all(&dir).map_err(|err| {
        format!(
            "unable to create trend directory `{}`: {err}",
            dir.display()
        )
    })?;
    let sha = entry.git_sha.chars().take(12).collect::<String>();
    let stamp = entry
        .recorded_at_utc
        .replace([':', '-'], "")
        .replace('T', "-");
    let path = dir.join(format!("{}-{sha}-{stamp}.toml", entry.os));
    let text = toml::to_string_pretty(entry)
        .map_err(|err| format!("unable to serialize trend entry: {err}"))?;
    std::fs::write(&path, text)
        .map_err(|err| format!("unable to write trend entry `{}`: {err}", path.display()))?;
    Ok(path)
}

/// Read an entry back. Used by tooling and tests to prove the round trip.
pub fn read_entry(path: &Path) -> Result<TrendEntry, String> {
    let text = std::fs::read_to_string(path)
        .map_err(|err| format!("unable to read trend entry `{}`: {err}", path.display()))?;
    toml::from_str(&text).map_err(|err| format!("unable to parse trend entry: {err}"))
}

/// Workloads whose measurement did not happen.
///
/// Separate from the budget verdict on purpose: a workload that did not run is
/// not a workload that was within budget, and `Skipped` alone cannot tell the
/// two apart.
pub fn unmeasured_names(report: &PerfReport) -> Vec<String> {
    report
        .skeletons
        .iter()
        .filter(|measurement| !measurement.measured)
        .map(|measurement| format!("{}: {}", measurement.name, measurement.message))
        .collect()
}

/// Required workloads that did not run.
///
/// `required` holds the headless workloads: every host that can build Legion
/// can run them, so failing to measure one is a defect rather than a property
/// of the machine. The renderer-backed measurement is deliberately not in that
/// set — it needs a display, and a headless CI runner legitimately cannot
/// supply one. That exemption is a list in one place rather than a special
/// case scattered through the gate.
pub fn missing_required_names(report: &PerfReport, required: &[String]) -> Vec<String> {
    required
        .iter()
        .filter_map(|name| {
            match report
                .skeletons
                .iter()
                .find(|measurement| &measurement.name == name)
            {
                None => Some(format!("{name}: absent from the report")),
                Some(measurement) if !measurement.measured => {
                    Some(format!("{name}: {}", measurement.message))
                }
                Some(_) => None,
            }
        })
        .collect()
}

/// Whether any measured row is still a synthetic stand-in, for the report
/// header.
pub fn synthetic_names(report: &PerfReport) -> Vec<String> {
    report
        .skeletons
        .iter()
        .filter(|measurement| measurement.synthetic_stand_in)
        .map(|measurement| measurement.name.clone())
        .collect()
}

/// Whether a strict run should exit non-zero.
pub fn strict_failure(
    report: &PerfReport,
    regressions: &[TrendRegression],
    required: &[String],
) -> bool {
    report.summary.failed > 0
        || !regressions.is_empty()
        || !missing_required_names(report, required).is_empty()
}

/// Whether a measurement's status is a budget failure. Kept next to
/// `strict_failure` so the two definitions cannot drift apart.
pub fn is_budget_failure(measurement: &SkeletonMeasurement) -> bool {
    measurement.status == SkeletonStatus::Failed
}
