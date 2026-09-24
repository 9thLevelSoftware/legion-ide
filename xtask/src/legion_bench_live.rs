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
    SCORING_MODE_RECORDED_REPLAY,
};
use crate::legion_bench_corpus::{
    CorpusTask, LiveRunInput, LiveRunOutput, LiveRunTaskInput, LiveRunTaskResult, corpus_suite,
    load_corpus,
};
use crate::legion_bench_recorded;

/// Default OpenAI-compatible endpoint (Ollama's loopback server).
pub const DEFAULT_LIVE_ENDPOINT: &str = "http://127.0.0.1:11434/v1";
/// Placeholder bearer token sent when the user configured no API key. Local
/// OpenAI-compatible servers (Ollama, llama.cpp, LM Studio, vLLM without auth)
/// accept and ignore it; the provider adapter requires a non-empty token.
pub const PLACEHOLDER_API_KEY: &str = "legion-bench-local";

const LIVE_INPUT_FILE: &str = "live_run_input.toml";
const LIVE_RESULTS_FILE: &str = "live_run_results.toml";

/// How the corpus run obtains model responses.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExecutionMode {
    /// Replay committed cassettes. Offline, repeatable, and the CI gate.
    Recorded,
    /// Run live and write cassettes for later replay.
    Record,
    /// Run live against the configured endpoint, writing no cassettes.
    LiveLocal,
}

impl ExecutionMode {
    /// Value written into the runner input's `provider_mode`.
    fn provider_mode(self) -> &'static str {
        match self {
            Self::Recorded => "replay",
            Self::Record => "record",
            Self::LiveLocal => "live",
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Recorded => "recorded",
            Self::Record => "record",
            Self::LiveLocal => "live-local",
        }
    }

    fn scoring_mode(self) -> &'static str {
        match self {
            Self::Recorded => SCORING_MODE_RECORDED_REPLAY,
            Self::Record | Self::LiveLocal => SCORING_MODE_LIVE_LOCAL,
        }
    }

    fn run_mode(self) -> LegionBenchRunMode {
        match self {
            Self::Recorded => LegionBenchRunMode::RecordedOffline,
            Self::Record | Self::LiveLocal => LegionBenchRunMode::LiveLocal,
        }
    }
}

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
pub fn score_live_task(
    corpus_task: &CorpusTask,
    raw: &LiveRunTaskResult,
    scoring_mode: &str,
) -> LegionBenchTaskScore {
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
        "scoring_mode={scoring_mode} outcome={} task_success={} tests_passed={} tests_passed_at_rest={} \
         verification_exit={} proposals_applied={}/{} diff_files={} turns={} tool_calls={} \
         duplicate_tool_calls={} retries={} context_tokens={} generation_tokens={} wall_ms={} \
         cost_cents=0 (local endpoint; no billing accounting){}{}",
        raw.outcome,
        raw.task_success,
        raw.tests_passed,
        raw.tests_passed_at_rest,
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
            cassette_exchanges: raw.cassette_exchanges,
            cassette_drift: raw.cassette_drift,
        }),
    }
}

/// Result score for a holdout task excluded from this run.
pub fn skipped_holdout_score(scoring_mode: &str) -> LegionBenchTaskScore {
    LegionBenchTaskScore {
        tests_passed: false,
        diff_files: 0,
        turns: 0,
        cost_cents: 0,
        score: 0,
        status: LegionBenchTaskStatus::Skipped,
        notes: format!(
            "scoring_mode={scoring_mode} holdout=true excluded from this run; \
             pass --include-holdout to execute"
        ),
        live: None,
    }
}

/// Result score for a task the runner binary produced no result for.
fn missing_result_score(id: &str, scoring_mode: &str) -> LegionBenchTaskScore {
    LegionBenchTaskScore {
        tests_passed: false,
        diff_files: 0,
        turns: 0,
        cost_cents: 0,
        score: 0,
        status: LegionBenchTaskStatus::Failed,
        notes: format!(
            "scoring_mode={scoring_mode} runner produced no result for task `{id}` \
             (legion_bench_live exited before executing it)"
        ),
        live: None,
    }
}

/// Options for a corpus-execution run (recorded, record, or live-local).
#[derive(Debug, Clone)]
pub struct LiveLocalOptions {
    pub out_dir: String,
    pub corpus_dir: String,
    pub cassette_dir: String,
    pub execution: ExecutionMode,
    pub include_holdout: bool,
    pub strict: bool,
    /// Regenerate the committed recorded baseline from this run instead of
    /// gating against it. Only meaningful in recorded mode.
    pub write_baseline: bool,
    pub config: LiveConfig,
}

/// Run legion-bench against the corpus. Returns the process exit code.
///
/// Every mode takes the same path — fixture checkout, agent loop, proposal
/// apply, verification command — and differs only in where the model's
/// responses come from. That is deliberate: a recorded run that took a
/// shortcut would not be measuring the thing the live run measures.
pub fn run_live_local(workspace_root: &Path, opts: &LiveLocalOptions) -> i32 {
    let label = opts.execution.label();
    let scoring_mode = opts.execution.scoring_mode();
    let corpus_dir = workspace_root.join(&opts.corpus_dir);
    let cassette_dir = workspace_root.join(&opts.cassette_dir);
    let out_dir = workspace_root.join(&opts.out_dir);
    let corpus = match load_corpus(&corpus_dir) {
        Ok(corpus) => corpus,
        Err(err) => {
            eprintln!("legion bench ({label}) failed: {err}");
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
            "legion bench ({label}) failed: every corpus task is holdout; \
             pass --include-holdout to execute them"
        );
        return 1;
    }
    for task in &executable {
        let fixture = workspace_root.join(&task.task.fixture_repo);
        if !fixture.is_dir() {
            eprintln!(
                "legion bench ({label}) failed: fixture for task `{}` not found: {}",
                task.task.id,
                fixture.display()
            );
            return 1;
        }
    }

    // Recorded mode is only offline and repeatable if the tapes on disk are
    // the tapes the baseline was cut from. Checking the set hash up front
    // turns an edited cassette into a refusal to run rather than a new number.
    let executed_ids: Vec<String> = executable.iter().map(|task| task.task.id.clone()).collect();
    let baseline = if opts.execution == ExecutionMode::Recorded {
        match legion_bench_recorded::load_baseline(&cassette_dir) {
            Ok(baseline) => {
                match legion_bench_recorded::cassette_set_hash(&cassette_dir, &executed_ids) {
                    Ok(hash) if hash == baseline.cassette_set_hash => {}
                    Ok(hash) if opts.write_baseline => {
                        eprintln!(
                            "legion bench ({label}): cassette set hash {hash} differs from the \
                             committed baseline; --write-baseline will replace it"
                        );
                    }
                    Ok(hash) => {
                        eprintln!(
                            "legion bench ({label}) failed: cassette set hash {hash} does not \
                             match the committed baseline {}. Re-record and re-baseline, or \
                             restore the tapes.",
                            baseline.cassette_set_hash
                        );
                        return 1;
                    }
                    Err(err) => {
                        eprintln!("legion bench ({label}) failed: {err}");
                        return 1;
                    }
                }
                Some(baseline)
            }
            Err(err) if opts.write_baseline => {
                eprintln!("legion bench ({label}): no usable baseline yet ({err}); writing one");
                None
            }
            Err(err) => {
                eprintln!("legion bench ({label}) failed: {err}");
                return 1;
            }
        }
    } else {
        None
    };

    if let Err(err) = fs::create_dir_all(&out_dir) {
        eprintln!(
            "legion bench ({label}) failed: unable to create out dir `{}`: {err}",
            out_dir.display()
        );
        return 1;
    }

    // Write the runner input spec.
    let input = LiveRunInput {
        schema_version: 1,
        endpoint: opts.config.endpoint.clone(),
        model: opts.config.model.clone(),
        provider_mode: opts.execution.provider_mode().to_string(),
        cassette_dir: Some(cassette_dir.to_string_lossy().into_owned()),
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
            eprintln!("legion bench ({label}) failed: unable to serialize runner input: {err}");
            return 1;
        }
    };
    if let Err(err) = fs::write(&input_path, input_text) {
        eprintln!(
            "legion bench ({label}) failed: unable to write `{}`: {err}",
            input_path.display()
        );
        return 1;
    }
    // Stale results from a previous run must not be mistaken for this run's.
    let _ = fs::remove_file(&results_path);

    eprintln!(
        "legion bench ({label}): provider_mode={} endpoint={} model={} tasks={} (of {} in corpus; include_holdout={})",
        opts.execution.provider_mode(),
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
        "legion bench ({label}): spawning subprocess: cargo {}",
        cargo_args.join(" ")
    );
    let status = live_runner_command(&cargo_args, &opts.config.api_key)
        .current_dir(workspace_root)
        .status();
    match status {
        Ok(status) if status.success() => {}
        Ok(status) => {
            eprintln!(
                "legion bench ({label}) failed: legion_bench_live exited with {status}; \
                 see stderr above (is the endpoint `{}` reachable and serving model `{}`?)",
                opts.config.endpoint, opts.config.model
            );
            return 1;
        }
        Err(err) => {
            eprintln!("legion bench ({label}) failed: unable to spawn cargo: {err}");
            return 1;
        }
    }

    // Read raw results and score them.
    let results_text = match fs::read_to_string(&results_path) {
        Ok(text) => text,
        Err(err) => {
            eprintln!(
                "legion bench ({label}) failed: runner wrote no results file `{}`: {err}",
                results_path.display()
            );
            return 1;
        }
    };
    let output: LiveRunOutput = match toml::from_str(&results_text) {
        Ok(output) => output,
        Err(err) => {
            eprintln!(
                "legion bench ({label}) failed: unable to parse `{}`: {err}",
                results_path.display()
            );
            return 1;
        }
    };

    let results: Vec<LegionBenchTaskResult> = corpus
        .iter()
        .map(|task| {
            let score = if !opts.include_holdout && task.live.holdout {
                skipped_holdout_score(scoring_mode)
            } else {
                match output.results.iter().find(|raw| raw.id == task.task.id) {
                    Some(raw) => score_live_task(task, raw, scoring_mode),
                    None => missing_result_score(&task.task.id, scoring_mode),
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
        mode: opts.execution.run_mode(),
        provider_profile: match opts.execution {
            // A replayed report names the model whose answers are on the tape,
            // not an endpoint it never dialed.
            ExecutionMode::Recorded => {
                let tape = output
                    .results
                    .iter()
                    .find(|raw| !raw.cassette_model.is_empty());
                format!(
                    "recorded:{}@{}",
                    tape.map_or("unbaselined", |raw| raw.cassette_model.as_str()),
                    tape.map_or("unbaselined", |raw| raw.cassette_arm.as_str()),
                )
            }
            ExecutionMode::Record | ExecutionMode::LiveLocal => {
                format!("live-local:{}@{}", opts.config.model, opts.config.endpoint)
            }
        },
        scoring_mode: scoring_mode.to_string(),
        suite_name: suite.suite_name.clone(),
        suite_fingerprint: suite.suite_fingerprint.clone(),
        summary: summary.clone(),
        tasks: results,
    };

    let report_path = match legion_bench::write_report(&out_dir, &report) {
        Ok(path) => path,
        Err(err) => {
            eprintln!("legion bench ({label}) failed: {err}");
            return 1;
        }
    };

    println!(
        "legion bench ({label}): total={} passed={} failed={} skipped={} average_score={} \
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

    if opts.execution == ExecutionMode::Recorded && opts.write_baseline {
        let hash = match legion_bench_recorded::cassette_set_hash(&cassette_dir, &executed_ids) {
            Ok(hash) => hash,
            Err(err) => {
                eprintln!("legion bench ({label}) failed: {err}");
                return 1;
            }
        };
        // Provenance comes from the tapes themselves, reported back by the
        // runner. Taking it from the invocation instead would let a
        // mis-typed `--model` relabel a cassette set as a model that never
        // produced it.
        let (model, arm) = match output
            .results
            .iter()
            .find(|raw| !raw.cassette_model.is_empty())
        {
            Some(raw) => (raw.cassette_model.clone(), raw.cassette_arm.clone()),
            None => {
                eprintln!(
                    "legion bench ({label}) failed: no replayed task reported a cassette model; \
                     cannot record baseline provenance"
                );
                return 1;
            }
        };
        let refreshed = legion_bench_recorded::RecordedBaseline {
            schema_version: legion_bench_recorded::BASELINE_SCHEMA_VERSION,
            model,
            arm,
            endpoint: baseline.as_ref().map_or_else(
                || opts.config.endpoint.clone(),
                |baseline| baseline.endpoint.clone(),
            ),
            recorded_at_utc: legion_bench::current_utc_rfc3339(),
            suite_fingerprint: suite.suite_fingerprint.clone(),
            cassette_set_hash: hash,
            tasks: legion_bench_recorded::expectations_from_report(&report),
        };
        if let Err(err) = legion_bench_recorded::write_baseline(&cassette_dir, &refreshed) {
            eprintln!("legion bench ({label}) failed: {err}");
            return 1;
        }
        println!(
            "legion bench ({label}): wrote baseline {}",
            legion_bench_recorded::baseline_path(&cassette_dir).display()
        );
        return 0;
    }

    // A recorded run's failures are the reference model's failures, faithfully
    // replayed; they are the baseline, not a regression. What makes them a
    // gate is `verify-legion-bench` comparing them to the committed
    // expectations — failing here instead would make the offline CI leg red
    // forever the moment the model got one task wrong.
    if opts.strict && summary.failed > 0 && opts.execution != ExecutionMode::Recorded {
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
