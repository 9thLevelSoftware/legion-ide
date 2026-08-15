//! Legion-Bench live-local orchestration.
//!
//! `legion-bench --mode live-local` runs the real delegated-task agent loop
//! against corpus-defined fixture checkouts, using a local OpenAI-compatible
//! endpoint. xtask stays thin: it loads the corpus, writes an input spec, and
//! spawns the `legion_bench_live` binary from `legion-app` (subprocess model —
//! xtask may not depend on legion-app, and `AppComposition::start_delegated_task`
//! already owns the worktree/broker/scope containment). The binary writes raw
//! measured metrics; xtask scores them against the gate budgets and emits the
//! standard `LegionBenchReport` with `scoring_mode = "live_local_execution"`.
//!
//! Environment:
//! - `LEGION_BENCH_ENDPOINT` — OpenAI-compatible base URL
//!   (default `http://127.0.0.1:11434/v1`, i.e. Ollama).
//! - `LEGION_BENCH_MODEL` — model id, REQUIRED in live-local mode.
//! - `LEGION_BENCH_API_KEY` — optional bearer token; local servers ignore it.

use std::{env, fs, path::Path, process};

use crate::legion_bench::{
    self, LegionBenchLiveMetrics, LegionBenchReport, LegionBenchRunMode, LegionBenchTaskResult,
    LegionBenchTaskScore, LegionBenchTaskStatus, SCORING_MODE_LIVE_LOCAL,
};
use crate::legion_bench_corpus::{
    CorpusTask, LiveRunInput, LiveRunOutput, LiveRunTaskInput, LiveRunTaskResult, corpus_suite,
    load_corpus,
};

/// Default OpenAI-compatible endpoint (Ollama's loopback server).
pub const DEFAULT_LIVE_ENDPOINT: &str = "http://127.0.0.1:11434/v1";
/// Placeholder bearer token sent when the user configured no API key. Local
/// OpenAI-compatible servers (Ollama, llama.cpp, LM Studio, vLLM without auth)
/// accept and ignore it; the provider adapter requires a non-empty token.
pub const PLACEHOLDER_API_KEY: &str = "legion-bench-local";

const LIVE_INPUT_FILE: &str = "live_run_input.toml";
const LIVE_RESULTS_FILE: &str = "live_run_results.toml";

/// Resolved live-local endpoint configuration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LiveConfig {
    pub endpoint: String,
    pub model: String,
    pub api_key: String,
}

/// Pure resolution of the live-local configuration from optional env values.
/// Split out of the env read so tests can exercise the failure modes.
pub fn resolve_live_config(
    endpoint: Option<String>,
    model: Option<String>,
    api_key: Option<String>,
) -> Result<LiveConfig, String> {
    let endpoint = endpoint
        .map(|value| value.trim().trim_end_matches('/').to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| DEFAULT_LIVE_ENDPOINT.to_string());
    let model = model
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            format!(
                "LEGION_BENCH_MODEL is required in live-local mode (the model id served by your \
                 OpenAI-compatible endpoint, e.g. `qwen2.5-coder:7b`). Endpoint: {endpoint}"
            )
        })?;
    let api_key = api_key
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| PLACEHOLDER_API_KEY.to_string());
    Ok(LiveConfig {
        endpoint,
        model,
        api_key,
    })
}

/// Read the live-local configuration from the environment.
pub fn live_config_from_env() -> Result<LiveConfig, String> {
    resolve_live_config(
        env::var("LEGION_BENCH_ENDPOINT").ok(),
        env::var("LEGION_BENCH_MODEL").ok(),
        env::var("LEGION_BENCH_API_KEY").ok(),
    )
}

/// Score one executed live task against its gate budget.
///
/// `passed` requires: loop completed, proposals applied, expected files present
/// (folded into `task_success` by the runner), the independently configurable
/// verification gate, and every budget ceiling.
pub fn score_live_task(corpus_task: &CorpusTask, raw: &LiveRunTaskResult) -> LegionBenchTaskScore {
    let budget = &corpus_task.task.gate_budget;
    let weights = &corpus_task.live.scoring;
    let cost_cents = 0_u32; // local endpoint: no billing; recorded as 0 with a note.

    let tests_gate = !budget.require_tests_pass || raw.tests_passed;
    let passed = raw.task_success
        && tests_gate
        && raw.diff_files <= budget.max_diff_files
        && raw.turns <= budget.max_turns
        && cost_cents <= budget.max_cost_cents;

    let mut score = 100_u32;
    score =
        score.saturating_sub(raw.diff_files.min(budget.max_diff_files) * weights.diff_file_penalty);
    score = score.saturating_sub(raw.turns.min(budget.max_turns) * weights.turn_penalty);
    score = score.saturating_sub(
        (cost_cents.min(budget.max_cost_cents) / 2) * weights.cost_half_cents_penalty,
    );
    if !passed {
        score = score.saturating_sub(weights.fail_penalty);
    }

    let status = if passed {
        LegionBenchTaskStatus::Passed
    } else {
        LegionBenchTaskStatus::Failed
    };
    let error_suffix = raw
        .error
        .as_deref()
        .map(|err| format!(" error={err}"))
        .unwrap_or_default();
    let notes = format!(
        "scoring_mode={SCORING_MODE_LIVE_LOCAL} outcome={} task_success={} tests_passed={} \
         verification_exit={} proposals_applied={}/{} diff_files={} turns={} tool_calls={} \
         duplicate_tool_calls={} retries={} context_tokens={} generation_tokens={} wall_ms={} \
         cost_cents=0 (local endpoint; no billing accounting){}{}",
        raw.outcome,
        raw.task_success,
        raw.tests_passed,
        raw.verification_exit
            .map(|code| code.to_string())
            .unwrap_or_else(|| "none".to_string()),
        raw.proposals_applied,
        raw.proposals_total,
        raw.diff_files,
        raw.turns,
        raw.tool_calls,
        raw.duplicate_tool_calls,
        raw.retries,
        raw.context_tokens
            .map(|value| value.to_string())
            .unwrap_or_else(|| "unavailable".to_string()),
        raw.generation_tokens
            .map(|value| value.to_string())
            .unwrap_or_else(|| "unavailable".to_string()),
        raw.wall_ms,
        if raw.notes.is_empty() {
            String::new()
        } else {
            format!(" {}", raw.notes)
        },
        error_suffix,
    );

    LegionBenchTaskScore {
        tests_passed: raw.tests_passed,
        diff_files: raw.diff_files,
        turns: raw.turns,
        cost_cents,
        score: score.min(100) as u8,
        status,
        notes,
        live: Some(LegionBenchLiveMetrics {
            task_success: raw.task_success,
            tool_calls: raw.tool_calls,
            duplicate_tool_calls: raw.duplicate_tool_calls,
            retries: raw.retries,
            context_tokens: raw.context_tokens,
            generation_tokens: raw.generation_tokens,
            wall_ms: raw.wall_ms,
        }),
    }
}

/// Result score for a holdout task excluded from this run.
pub fn skipped_holdout_score() -> LegionBenchTaskScore {
    LegionBenchTaskScore {
        tests_passed: false,
        diff_files: 0,
        turns: 0,
        cost_cents: 0,
        score: 0,
        status: LegionBenchTaskStatus::Skipped,
        notes: format!(
            "scoring_mode={SCORING_MODE_LIVE_LOCAL} holdout=true excluded from this run; \
             pass --include-holdout to execute"
        ),
        live: None,
    }
}

/// Result score for a task the runner binary produced no result for.
fn missing_result_score(id: &str) -> LegionBenchTaskScore {
    LegionBenchTaskScore {
        tests_passed: false,
        diff_files: 0,
        turns: 0,
        cost_cents: 0,
        score: 0,
        status: LegionBenchTaskStatus::Failed,
        notes: format!(
            "scoring_mode={SCORING_MODE_LIVE_LOCAL} runner produced no result for task `{id}` \
             (legion_bench_live exited before executing it)"
        ),
        live: None,
    }
}

/// Options for a live-local run.
#[derive(Debug, Clone)]
pub struct LiveLocalOptions {
    pub out_dir: String,
    pub corpus_dir: String,
    pub include_holdout: bool,
    pub strict: bool,
    pub config: LiveConfig,
}

/// Run legion-bench in live-local mode. Returns the process exit code.
pub fn run_live_local(workspace_root: &Path, opts: &LiveLocalOptions) -> i32 {
    let corpus_dir = workspace_root.join(&opts.corpus_dir);
    let out_dir = workspace_root.join(&opts.out_dir);
    let corpus = match load_corpus(&corpus_dir) {
        Ok(corpus) => corpus,
        Err(err) => {
            eprintln!("legion bench (live-local) failed: {err}");
            return 1;
        }
    };
    let suite = corpus_suite(&corpus);

    // Plan the execution set (holdouts excluded unless requested) and verify
    // fixtures exist before spending any model time.
    let executable: Vec<&CorpusTask> = corpus
        .iter()
        .filter(|task| opts.include_holdout || !task.live.holdout)
        .collect();
    if executable.is_empty() {
        eprintln!(
            "legion bench (live-local) failed: every corpus task is holdout; \
             pass --include-holdout to execute them"
        );
        return 1;
    }
    for task in &executable {
        let fixture = workspace_root.join(&task.task.fixture_repo);
        if !fixture.is_dir() {
            eprintln!(
                "legion bench (live-local) failed: fixture for task `{}` not found: {}",
                task.task.id,
                fixture.display()
            );
            return 1;
        }
    }

    if let Err(err) = fs::create_dir_all(&out_dir) {
        eprintln!(
            "legion bench (live-local) failed: unable to create out dir `{}`: {err}",
            out_dir.display()
        );
        return 1;
    }

    // Write the runner input spec.
    let input = LiveRunInput {
        schema_version: 1,
        endpoint: opts.config.endpoint.clone(),
        model: opts.config.model.clone(),
        tasks: executable
            .iter()
            .map(|task| LiveRunTaskInput {
                id: task.task.id.clone(),
                fixture_dir: workspace_root
                    .join(&task.task.fixture_repo)
                    .to_string_lossy()
                    .into_owned(),
                prompt: task.live.prompt.clone(),
                target_kind: task.live.scope.target_kind.clone(),
                target_path: task.live.scope.target_path.clone(),
                allowed_tools: task.live.scope.allowed_tools.clone(),
                forbidden_paths: task.live.scope.forbidden_paths.clone(),
                verification_command: task.live.verification.command.clone(),
                expected_exit: task.live.verification.expected_exit,
                timeout_secs: task.live.verification.timeout_secs,
                expected_files: task.live.verification.expected_files.clone(),
            })
            .collect(),
    };
    let input_path = out_dir.join(LIVE_INPUT_FILE);
    let results_path = out_dir.join(LIVE_RESULTS_FILE);
    let input_text = match toml::to_string_pretty(&input) {
        Ok(text) => text,
        Err(err) => {
            eprintln!("legion bench (live-local) failed: unable to serialize runner input: {err}");
            return 1;
        }
    };
    if let Err(err) = fs::write(&input_path, input_text) {
        eprintln!(
            "legion bench (live-local) failed: unable to write `{}`: {err}",
            input_path.display()
        );
        return 1;
    }
    // Stale results from a previous run must not be mistaken for this run's.
    let _ = fs::remove_file(&results_path);

    eprintln!(
        "legion bench (live-local): endpoint={} model={} tasks={} (of {} in corpus; include_holdout={})",
        opts.config.endpoint,
        opts.config.model,
        executable.len(),
        corpus.len(),
        opts.include_holdout,
    );

    // Spawn the runner binary (subprocess model, like the golden paths).
    // --features test-helpers gives the runner the cancellation-watchdog seam.
    let cargo_args = [
        "run",
        "--jobs",
        "4",
        "-p",
        "legion-app",
        "--bin",
        "legion_bench_live",
        "--features",
        "test-helpers",
        "--",
        "--input",
        &input_path.to_string_lossy(),
        "--output",
        &results_path.to_string_lossy(),
    ]
    .map(str::to_string);
    eprintln!(
        "legion bench (live-local): spawning subprocess: cargo {}",
        cargo_args.join(" ")
    );
    let status = live_runner_command(&cargo_args, &opts.config.api_key)
        .current_dir(workspace_root)
        .status();
    match status {
        Ok(status) if status.success() => {}
        Ok(status) => {
            eprintln!(
                "legion bench (live-local) failed: legion_bench_live exited with {status}; \
                 see stderr above (is the endpoint `{}` reachable and serving model `{}`?)",
                opts.config.endpoint, opts.config.model
            );
            return 1;
        }
        Err(err) => {
            eprintln!("legion bench (live-local) failed: unable to spawn cargo: {err}");
            return 1;
        }
    }

    // Read raw results and score them.
    let results_text = match fs::read_to_string(&results_path) {
        Ok(text) => text,
        Err(err) => {
            eprintln!(
                "legion bench (live-local) failed: runner wrote no results file `{}`: {err}",
                results_path.display()
            );
            return 1;
        }
    };
    let output: LiveRunOutput = match toml::from_str(&results_text) {
        Ok(output) => output,
        Err(err) => {
            eprintln!(
                "legion bench (live-local) failed: unable to parse `{}`: {err}",
                results_path.display()
            );
            return 1;
        }
    };

    let results: Vec<LegionBenchTaskResult> = corpus
        .iter()
        .map(|task| {
            let score = if !opts.include_holdout && task.live.holdout {
                skipped_holdout_score()
            } else {
                match output.results.iter().find(|raw| raw.id == task.task.id) {
                    Some(raw) => score_live_task(task, raw),
                    None => missing_result_score(&task.task.id),
                }
            };
            LegionBenchTaskResult {
                task: task.task.clone(),
                score,
            }
        })
        .collect();

    let summary = legion_bench::recompute_summary(&results);
    let git_sha = crate::perf_harness::resolve_workspace_git_sha(workspace_root);
    let report = LegionBenchReport {
        schema_version: legion_bench::BENCH_SCHEMA_VERSION,
        package_name: "legion-desktop".to_string(),
        measured_at_utc: legion_bench::current_utc_rfc3339(),
        git_sha,
        mode: LegionBenchRunMode::LiveLocal,
        provider_profile: format!("live-local:{}@{}", opts.config.model, opts.config.endpoint),
        scoring_mode: SCORING_MODE_LIVE_LOCAL.to_string(),
        suite_name: suite.suite_name.clone(),
        suite_fingerprint: suite.suite_fingerprint.clone(),
        summary: summary.clone(),
        tasks: results,
    };

    let report_path = match legion_bench::write_report(&out_dir, &report) {
        Ok(path) => path,
        Err(err) => {
            eprintln!("legion bench (live-local) failed: {err}");
            return 1;
        }
    };

    println!(
        "legion bench (live-local): total={} passed={} failed={} skipped={} average_score={} \
         report={} strict={} provider={} fingerprint={}",
        summary.total,
        summary.passed,
        summary.failed,
        summary.skipped,
        summary.average_score,
        report_path.display(),
        opts.strict,
        report.provider_profile,
        report.suite_fingerprint,
    );
    if opts.strict && summary.failed > 0 {
        1
    } else {
        0
    }
}

fn live_runner_command(cargo_args: &[String], api_key: &str) -> process::Command {
    let mut command = process::Command::new("cargo");
    command
        .args(cargo_args)
        .env("LEGION_BENCH_API_KEY", api_key);
    command
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn live_runner_command_passes_provider_key_only_via_environment() {
        let command = live_runner_command(&["run".to_string()], "sk-env-secret");
        let configured_key = command
            .get_envs()
            .find(|(name, _)| *name == "LEGION_BENCH_API_KEY")
            .and_then(|(_, value)| value)
            .and_then(std::ffi::OsStr::to_str);

        assert_eq!(configured_key, Some("sk-env-secret"));
    }
}
