//! Legion-Bench live-local task corpus.
//!
//! Live-local tasks are defined one-per-file as TOML under
//! `evals/legion-bench/tasks/`. Each file carries the report-facing task
//! definition (id / kind / fixture / prompt / gate budget — everything that
//! goes into the suite fingerprint) plus the live execution spec (verification
//! command, scope, holdout flag, scoring weights) that never appears inside
//! the report's embedded `LegionBenchTask`, so live reports keep report schema
//! version 2.
//!
//! The same loader is used by `legion-bench --mode live-local` (to plan the
//! run) and by `verify-legion-bench` (to recompute the suite fingerprint the
//! live report must match).

use std::{fs, path::Path};

use serde::{Deserialize, Serialize};

use crate::legion_bench::{
    LegionBenchGateBudget, LegionBenchSuite, LegionBenchTask, LegionBenchTaskKind,
    fingerprint_suite,
};

/// Default corpus directory, relative to the workspace root.
pub const DEFAULT_CORPUS_PATH: &str = "evals/legion-bench/tasks";
/// Suite name for corpus-derived live-local suites.
pub const LIVE_SUITE_NAME: &str = "legion-bench-live-v0";
/// Task-level provider profile for live-local tasks. Kept model-agnostic so the
/// suite fingerprint does not change when the local model changes; the actual
/// model + endpoint are recorded in the report-level `provider_profile`.
pub const LIVE_TASK_PROVIDER_PROFILE: &str = "live-local";

/// Scoring weights, overridable per task via `[scoring]`.
/// Defaults mirror `legion_bench::compute_score`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LiveScoringWeights {
    #[serde(default = "default_diff_file_penalty")]
    pub diff_file_penalty: u32,
    #[serde(default = "default_turn_penalty")]
    pub turn_penalty: u32,
    #[serde(default = "default_cost_half_cents_penalty")]
    pub cost_half_cents_penalty: u32,
    #[serde(default = "default_fail_penalty")]
    pub fail_penalty: u32,
}

fn default_diff_file_penalty() -> u32 {
    4
}
fn default_turn_penalty() -> u32 {
    3
}
fn default_cost_half_cents_penalty() -> u32 {
    1
}
fn default_fail_penalty() -> u32 {
    40
}

impl Default for LiveScoringWeights {
    fn default() -> Self {
        Self {
            diff_file_penalty: default_diff_file_penalty(),
            turn_penalty: default_turn_penalty(),
            cost_half_cents_penalty: default_cost_half_cents_penalty(),
            fail_penalty: default_fail_penalty(),
        }
    }
}

/// Verification spec: command run in the checkout after proposals are applied.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LiveVerificationSpec {
    /// Shell command run with cwd = the task checkout.
    pub command: String,
    /// Pass iff the exit code matches.
    #[serde(default)]
    pub expected_exit: i32,
    /// Wall-clock kill for the verification subprocess AND for the agent loop
    /// (via the cancellation watchdog).
    #[serde(default = "default_timeout_secs")]
    pub timeout_secs: u64,
    /// Checkout-relative paths that must exist after apply; empty = skip.
    #[serde(default)]
    pub expected_files: Vec<String>,
    /// Whether this command passes on the untouched fixture.
    ///
    /// Defaults by kind: a `refactor` preserves behaviour so its suite passes
    /// at rest, and every other kind must fail so that passing proves work.
    /// Set explicitly to `"fails"` for a refactor verified by a script that
    /// checks the restructuring happened rather than by the existing suite —
    /// the stronger design, and the reason this is an override rather than a
    /// rule.
    ///
    /// The corpus-health gate enforces it in both directions. Without that, a
    /// task made unwinnable by *another* task's deliberate breakage in the
    /// same fixture passes unnoticed, which is exactly how `bench-rust-04`
    /// became unpassable when `tests/merge_layers.rs` was added.
    #[serde(default)]
    pub at_rest: Option<String>,
}

fn default_timeout_secs() -> u64 {
    300
}

/// Scope spec mapped onto `DelegatedTaskScope` by the live runner binary.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LiveScopeSpec {
    /// "file" | "module" | "repo"
    #[serde(default = "default_target_kind")]
    pub target_kind: String,
    /// Checkout-relative anchor path (required for file/module targets).
    #[serde(default)]
    pub target_path: Option<String>,
    /// Tool registry names ("read", "grep", "glob", "outline",
    /// "edit-as-proposal", "terminal-command").
    #[serde(default = "default_allowed_tools")]
    pub allowed_tools: Vec<String>,
    /// Checkout-relative forbidden paths.
    #[serde(default)]
    pub forbidden_paths: Vec<String>,
}

fn default_target_kind() -> String {
    "repo".to_string()
}

fn default_allowed_tools() -> Vec<String> {
    vec![
        "read".to_string(),
        "grep".to_string(),
        "glob".to_string(),
        "outline".to_string(),
        "edit-as-proposal".to_string(),
    ]
}

impl Default for LiveScopeSpec {
    fn default() -> Self {
        Self {
            target_kind: default_target_kind(),
            target_path: None,
            allowed_tools: default_allowed_tools(),
            forbidden_paths: Vec::new(),
        }
    }
}

/// The live-execution half of a corpus task (fields that are NOT embedded in
/// the report's `LegionBenchTask`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LiveTaskSpec {
    pub prompt: String,
    pub holdout: bool,
    pub verification: LiveVerificationSpec,
    pub scope: LiveScopeSpec,
    pub scoring: LiveScoringWeights,
}

/// One fully-parsed corpus task: report-facing definition + live spec.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CorpusTask {
    pub task: LegionBenchTask,
    pub live: LiveTaskSpec,
}

/// On-disk TOML schema for a corpus task file.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct CorpusTaskFile {
    schema_version: u32,
    id: String,
    kind: LegionBenchTaskKind,
    fixture_repo: String,
    #[serde(default)]
    holdout: bool,
    prompt: String,
    verification: LiveVerificationSpec,
    #[serde(default)]
    scope: LiveScopeSpec,
    #[serde(default = "default_gate_budget")]
    gate_budget: LegionBenchGateBudget,
    #[serde(default)]
    scoring: Option<LiveScoringWeights>,
}

fn default_gate_budget() -> LegionBenchGateBudget {
    LegionBenchGateBudget {
        require_tests_pass: true,
        max_diff_files: 4,
        max_turns: 8,
        max_cost_cents: 25,
    }
}

const VALID_TOOLS: &[&str] = &[
    "read",
    "grep",
    "glob",
    "outline",
    "edit-as-proposal",
    "terminal-command",
];

/// Parse one corpus task file.
pub fn parse_corpus_task(text: &str, source: &str) -> Result<CorpusTask, String> {
    let file: CorpusTaskFile = toml::from_str(text)
        .map_err(|err| format!("unable to parse corpus task `{source}`: {err}"))?;
    if file.schema_version != 1 {
        return Err(format!(
            "corpus task `{source}`: unsupported schema_version {} (expected 1)",
            file.schema_version
        ));
    }
    let id = file.id.trim().to_string();
    if id.is_empty() || id.chars().any(char::is_whitespace) {
        return Err(format!(
            "corpus task `{source}`: id must be non-empty and contain no whitespace"
        ));
    }
    if file.kind == LegionBenchTaskKind::HostileEval {
        return Err(format!(
            "corpus task `{source}`: kind hostile_eval is reserved for the hostile suite"
        ));
    }
    if file.fixture_repo.trim().is_empty() {
        return Err(format!("corpus task `{source}`: fixture_repo is required"));
    }
    let prompt = file.prompt.trim().to_string();
    if prompt.is_empty() {
        return Err(format!("corpus task `{source}`: prompt is required"));
    }
    if file.verification.command.trim().is_empty() {
        return Err(format!(
            "corpus task `{source}`: verification.command is required"
        ));
    }
    if file.verification.timeout_secs == 0 {
        return Err(format!(
            "corpus task `{source}`: verification.timeout_secs must be > 0"
        ));
    }
    match file.scope.target_kind.as_str() {
        "repo" => {}
        "file" | "module" => {
            if file
                .scope
                .target_path
                .as_deref()
                .is_none_or(|path| path.trim().is_empty())
            {
                return Err(format!(
                    "corpus task `{source}`: scope.target_path is required for {} targets",
                    file.scope.target_kind
                ));
            }
        }
        other => {
            return Err(format!(
                "corpus task `{source}`: unknown scope.target_kind `{other}` (expected file|module|repo)"
            ));
        }
    }
    if file.scope.allowed_tools.is_empty() {
        return Err(format!(
            "corpus task `{source}`: scope.allowed_tools must not be empty"
        ));
    }
    for tool in &file.scope.allowed_tools {
        if !VALID_TOOLS.contains(&tool.as_str()) {
            return Err(format!(
                "corpus task `{source}`: unknown tool `{tool}` (expected one of {VALID_TOOLS:?})"
            ));
        }
    }

    Ok(CorpusTask {
        task: LegionBenchTask {
            id,
            fixture_repo: file.fixture_repo.trim().to_string(),
            kind: file.kind,
            objective: prompt.clone(),
            provider_profile: LIVE_TASK_PROVIDER_PROFILE.to_string(),
            gate_budget: file.gate_budget,
        },
        live: LiveTaskSpec {
            prompt,
            holdout: file.holdout,
            verification: file.verification,
            scope: file.scope,
            scoring: file.scoring.unwrap_or_default(),
        },
    })
}

/// Load all `*.toml` corpus tasks from a directory, sorted by task id.
pub fn load_corpus(dir: &Path) -> Result<Vec<CorpusTask>, String> {
    if !dir.is_dir() {
        return Err(format!(
            "legion-bench corpus directory not found: {} (live-local mode requires task TOMLs; see evals/legion-bench/tasks/)",
            dir.display()
        ));
    }
    let mut entries: Vec<_> = fs::read_dir(dir)
        .map_err(|err| format!("unable to read corpus dir `{}`: {err}", dir.display()))?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "toml") && path.is_file())
        .collect();
    entries.sort();

    let mut tasks = Vec::new();
    for path in entries {
        let source = path.display().to_string();
        let text = fs::read_to_string(&path)
            .map_err(|err| format!("unable to read corpus task `{source}`: {err}"))?;
        tasks.push(parse_corpus_task(&text, &source)?);
    }
    if tasks.is_empty() {
        return Err(format!(
            "corpus dir `{}` contains no *.toml task files",
            dir.display()
        ));
    }
    tasks.sort_by(|a, b| a.task.id.cmp(&b.task.id));
    for pair in tasks.windows(2) {
        if pair[0].task.id == pair[1].task.id {
            return Err(format!(
                "corpus contains duplicate task id `{}`",
                pair[0].task.id
            ));
        }
    }
    Ok(tasks)
}

/// Build the fingerprinted suite for a corpus. The suite always includes ALL
/// corpus tasks (holdout included) so the fingerprint is stable regardless of
/// the `--include-holdout` flag; excluded tasks appear in reports as `skipped`.
pub fn corpus_suite(tasks: &[CorpusTask]) -> LegionBenchSuite {
    let bench_tasks: Vec<LegionBenchTask> = tasks.iter().map(|t| t.task.clone()).collect();
    let suite_fingerprint = fingerprint_suite(&bench_tasks);
    LegionBenchSuite {
        suite_name: LIVE_SUITE_NAME.to_string(),
        suite_fingerprint,
        recorded_provider_profile: LIVE_TASK_PROVIDER_PROFILE.to_string(),
        live_provider_profile: LIVE_TASK_PROVIDER_PROFILE.to_string(),
        tasks: bench_tasks,
    }
}

// ─── xtask ↔ legion_bench_live binary interchange DTOs ───────────────────────
//
// The live runner binary (`crates/legion-app/src/bin/legion_bench_live.rs`)
// deserializes `LiveRunInput` and serializes `LiveRunOutput` with structurally
// identical TOML definitions. Field names are the wire contract — keep both
// sides in sync.

/// Input handed to the live runner binary (written to `<out>/live_run_input.toml`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LiveRunInput {
    pub schema_version: u32,
    /// OpenAI-compatible base URL (e.g. `http://127.0.0.1:11434/v1`).
    pub endpoint: String,
    /// Model identifier sent on every chat completion request.
    pub model: String,
    pub tasks: Vec<LiveRunTaskInput>,
}

/// One executable task handed to the live runner binary.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LiveRunTaskInput {
    pub id: String,
    /// Absolute path to the fixture directory to copy into a temp checkout.
    pub fixture_dir: String,
    pub prompt: String,
    pub target_kind: String,
    #[serde(default)]
    pub target_path: Option<String>,
    pub allowed_tools: Vec<String>,
    pub forbidden_paths: Vec<String>,
    pub verification_command: String,
    pub expected_exit: i32,
    pub timeout_secs: u64,
    pub expected_files: Vec<String>,
}

/// Raw measured results written by the live runner binary
/// (`<out>/live_run_results.toml`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LiveRunOutput {
    pub schema_version: u32,
    #[serde(default)]
    pub results: Vec<LiveRunTaskResult>,
}

/// Raw measured metrics for one executed task. Scoring happens in xtask.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LiveRunTaskResult {
    pub id: String,
    /// "completed" | "blocked" | "budget_exhausted" | "cancelled" | "error"
    pub outcome: String,
    pub task_success: bool,
    pub tests_passed: bool,
    /// Whether the verification command already passed before the model ran.
    ///
    /// Refactor tasks keep their tests green by design, so `tests_passed`
    /// alone credits a model that did nothing. Carried through to the report
    /// so a reader can tell an inherited pass from an earned one without
    /// having to know which tasks are refactors.
    #[serde(default)]
    pub tests_passed_at_rest: bool,
    #[serde(default)]
    pub verification_exit: Option<i32>,
    pub proposals_total: u32,
    pub proposals_applied: u32,
    pub diff_files: u32,
    pub turns: u32,
    pub tool_calls: u32,
    pub duplicate_tool_calls: u32,
    pub retries: u32,
    #[serde(default)]
    pub context_tokens: Option<u64>,
    #[serde(default)]
    pub generation_tokens: Option<u64>,
    pub wall_ms: u64,
    #[serde(default)]
    pub error: Option<String>,
    #[serde(default)]
    pub notes: String,
}
