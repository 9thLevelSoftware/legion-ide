//! Recorded-mode provenance and the regression baseline it is gated against.
//!
//! Recorded mode replays a cassette of model responses through the real agent
//! loop against a real fixture checkout, applies the proposals it produces and
//! runs the task's own verification command. Nothing about the score is
//! derived from the gate budget; every number is measured.
//!
//! What makes that a *gate* rather than a measurement is this file: the
//! expected per-task outcome is committed, and `verify-legion-bench` fails on
//! any difference. A change to the agent loop, the tool dispatch, the patch
//! applier or the proposal pipeline changes what the replayed conversation
//! does to the checkout, and the difference shows up here.

use std::{fs, path::Path};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::legion_bench::LegionBenchReport;

/// Default cassette directory, relative to the workspace root.
pub const DEFAULT_CASSETTE_PATH: &str = "evals/legion-bench/recorded";
/// Baseline file name inside the cassette directory.
pub const BASELINE_FILE: &str = "baseline.toml";
/// Schema version of the recorded baseline file.
pub const BASELINE_SCHEMA_VERSION: u32 = 1;

/// Committed expectation for one replayed task.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecordedTaskExpectation {
    pub id: String,
    /// `passed` | `failed` | `skipped`.
    pub status: String,
    pub score: u8,
    pub tests_passed: bool,
    pub diff_files: u32,
    pub turns: u32,
    pub task_success: bool,
    pub tool_calls: u32,
    pub duplicate_tool_calls: u32,
    pub retries: u32,
    /// Pinned per task, and 0 for all but two of them. A change means the loop
    /// no longer asks the model what it asked when the tape was cut, so the
    /// replayed answers are answering a different conversation.
    pub cassette_drift: u32,
}

/// Provenance of the cassette set plus the expected result of replaying it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecordedBaseline {
    pub schema_version: u32,
    /// The model whose responses are on the tapes.
    pub model: String,
    /// `governed` or `raw` — the `LEGION_AI_GOVERNORS` arm the tapes were cut
    /// under. Replaying a tape under the other arm measures neither.
    pub arm: String,
    /// Human-readable description of the endpoint that served the model.
    pub endpoint: String,
    pub recorded_at_utc: String,
    /// Fingerprint of the corpus the tapes were cut against.
    pub suite_fingerprint: String,
    /// Hash over the cassette files themselves, so an edited tape is a gate
    /// failure rather than a new baseline.
    pub cassette_set_hash: String,
    pub tasks: Vec<RecordedTaskExpectation>,
}

/// Hash the cassette files for `task_ids`, in id order.
///
/// Length-prefixing every field keeps the digest unambiguous: without it a
/// byte moved from a file name into that file's contents would hash the same.
pub fn cassette_set_hash(cassette_dir: &Path, task_ids: &[String]) -> Result<String, String> {
    let mut ids: Vec<&String> = task_ids.iter().collect();
    ids.sort();
    let mut hasher = Sha256::new();
    for id in ids {
        let path = cassette_dir.join(format!("{id}.json"));
        let bytes = fs::read(&path).map_err(|err| {
            format!(
                "unable to read cassette `{}`: {err} (record it with \
                 `cargo run -p xtask -- legion-bench --mode record`)",
                path.display()
            )
        })?;
        // Cassettes are JSON written by the runner; a git checkout can still
        // hand them back with CRLF, and the hash must not depend on that.
        let normalized: Vec<u8> = String::from_utf8(bytes)
            .map(|text| text.replace("\r\n", "\n").into_bytes())
            .map_err(|err| err.into_bytes())
            .unwrap_or_else(|bytes| bytes);
        hasher.update((id.len() as u64).to_le_bytes());
        hasher.update(id.as_bytes());
        hasher.update((normalized.len() as u64).to_le_bytes());
        hasher.update(&normalized);
    }
    Ok(format!("sha256:{}", hex::encode(hasher.finalize())))
}

pub fn baseline_path(cassette_dir: &Path) -> std::path::PathBuf {
    cassette_dir.join(BASELINE_FILE)
}

pub fn load_baseline(cassette_dir: &Path) -> Result<RecordedBaseline, String> {
    let path = baseline_path(cassette_dir);
    let text = fs::read_to_string(&path).map_err(|err| {
        format!(
            "unable to read recorded baseline `{}`: {err} (regenerate with \
             `cargo run -p xtask -- legion-bench --mode recorded --write-baseline`)",
            path.display()
        )
    })?;
    let baseline: RecordedBaseline = toml::from_str(&text).map_err(|err| {
        format!(
            "unable to parse recorded baseline `{}`: {err}",
            path.display()
        )
    })?;
    if baseline.schema_version != BASELINE_SCHEMA_VERSION {
        return Err(format!(
            "recorded baseline `{}` has schema_version {} (expected {BASELINE_SCHEMA_VERSION})",
            path.display(),
            baseline.schema_version
        ));
    }
    if baseline.tasks.is_empty() {
        return Err(format!(
            "recorded baseline `{}` lists no tasks; an empty baseline would pass against any \
             report at all",
            path.display()
        ));
    }
    Ok(baseline)
}

pub fn write_baseline(cassette_dir: &Path, baseline: &RecordedBaseline) -> Result<(), String> {
    let path = baseline_path(cassette_dir);
    fs::create_dir_all(cassette_dir).map_err(|err| {
        format!(
            "unable to create cassette dir `{}`: {err}",
            cassette_dir.display()
        )
    })?;
    let text = toml::to_string_pretty(baseline)
        .map_err(|err| format!("unable to serialize recorded baseline: {err}"))?;
    fs::write(&path, format!("{text}\n"))
        .map_err(|err| format!("unable to write `{}`: {err}", path.display()))
}

/// Derive the expected-result rows from a freshly measured report.
pub fn expectations_from_report(report: &LegionBenchReport) -> Vec<RecordedTaskExpectation> {
    report
        .tasks
        .iter()
        .map(|result| {
            let live = result.score.live.as_ref();
            RecordedTaskExpectation {
                id: result.task.id.clone(),
                status: result.score.status.as_str().to_string(),
                score: result.score.score,
                tests_passed: result.score.tests_passed,
                diff_files: result.score.diff_files,
                turns: result.score.turns,
                task_success: live.is_some_and(|live| live.task_success),
                tool_calls: live.map_or(0, |live| live.tool_calls),
                duplicate_tool_calls: live.map_or(0, |live| live.duplicate_tool_calls),
                retries: live.map_or(0, |live| live.retries),
                cassette_drift: live.map_or(0, |live| live.cassette_drift),
            }
        })
        .collect()
}

/// Compare a fresh recorded-mode report against the committed baseline.
///
/// Returns every difference rather than the first: a change to the agent loop
/// usually moves several tasks at once, and one line of output per run turns
/// diagnosis into a series of re-runs.
pub fn compare_to_baseline(
    report: &LegionBenchReport,
    baseline: &RecordedBaseline,
) -> Result<(), Vec<String>> {
    let mut problems = Vec::new();
    if report.suite_fingerprint != baseline.suite_fingerprint {
        problems.push(format!(
            "suite fingerprint changed: report={} baseline={} (the corpus moved; re-record and \
             re-baseline)",
            report.suite_fingerprint, baseline.suite_fingerprint
        ));
    }
    let measured = expectations_from_report(report);
    for expected in &baseline.tasks {
        let Some(actual) = measured.iter().find(|row| row.id == expected.id) else {
            problems.push(format!(
                "baseline task `{}` is missing from the report",
                expected.id
            ));
            continue;
        };
        if actual != expected {
            problems.push(format!(
                "task `{}` regressed: measured {actual:?} but baseline expects {expected:?}",
                expected.id
            ));
        }
    }
    for actual in &measured {
        if !baseline.tasks.iter().any(|row| row.id == actual.id) {
            problems.push(format!(
                "report task `{}` has no baseline row (re-baseline after adding a task)",
                actual.id
            ));
        }
    }
    if problems.is_empty() {
        Ok(())
    } else {
        Err(problems)
    }
}
