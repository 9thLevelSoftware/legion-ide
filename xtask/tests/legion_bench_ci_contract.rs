//! P9.F1.T3 as a test rather than a promise.
//!
//! The acceptance criterion is "CI always runs recorded mode; live mode is
//! opt-in and scheduled", with an explicit stop condition: "stop if live mode
//! is required for any CI gate". Both halves are properties of the workflow
//! files, so they are checked here — a future edit that wires a live bench run
//! into a pull-request trigger fails `cargo test`, not review.

use std::{fs, path::PathBuf};

fn workflows_dir() -> PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("workspace root")
        .join(".github")
        .join("workflows")
}

fn workflow(name: &str) -> String {
    let path = workflows_dir().join(name);
    fs::read_to_string(&path).unwrap_or_else(|err| panic!("read {}: {err}", path.display()))
}

fn all_workflows() -> Vec<(String, String)> {
    fs::read_dir(workflows_dir())
        .expect("read workflows dir")
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.extension()
                .is_some_and(|ext| ext == "yml" || ext == "yaml")
        })
        .map(|path| {
            let name = path
                .file_name()
                .expect("workflow file name")
                .to_string_lossy()
                .into_owned();
            let text = fs::read_to_string(&path).expect("read workflow");
            (name, text)
        })
        .collect()
}

/// Every `legion-bench` invocation in every workflow, as `(workflow, mode)`.
fn bench_invocations() -> Vec<(String, String)> {
    let mut found = Vec::new();
    for (name, text) in all_workflows() {
        for line in text.lines() {
            let Some(rest) = line.split("legion-bench --mode ").nth(1) else {
                continue;
            };
            let mode = rest
                .split_whitespace()
                .next()
                .unwrap_or_default()
                .to_string();
            found.push((name.clone(), mode));
        }
    }
    found
}

#[test]
fn recorded_mode_runs_on_every_pull_request() {
    let recorded = workflow("legion-bench.yml");
    assert!(
        recorded.contains("pull_request:"),
        "the recorded bench must run on every pull request"
    );
    assert!(
        recorded.contains("legion-bench --mode recorded"),
        "legion-bench.yml must invoke recorded mode"
    );
    assert!(
        recorded.contains("verify-legion-bench\n") || recorded.contains("verify-legion-bench\r\n"),
        "the recorded leg must run the verifier, which is what compares against the baseline"
    );
    // Recorded mode must not need a provider credential or endpoint: the whole
    // point is that CI can run it with nothing installed.
    for forbidden in [
        "LEGION_BENCH_MODEL",
        "LEGION_BENCH_ENDPOINT",
        "LEGION_BENCH_API_KEY",
        "secrets.",
    ] {
        assert!(
            !recorded.contains(forbidden),
            "the recorded leg must stay self-contained; found `{forbidden}` in legion-bench.yml"
        );
    }
}

/// P9.F1.T4 keeps the raw baseline *frozen*, which means re-checked, not
/// merely written down once. The always-on workflow replays the ungoverned
/// cassette set under the `LEGION_AI_GOVERNORS=off` seam and verifies it.
#[test]
fn the_ungoverned_baseline_is_replayed_under_the_raw_seam() {
    let recorded = workflow("legion-bench.yml");
    assert!(
        recorded.contains("evals/legion-bench/recorded-raw"),
        "the recorded workflow must replay the frozen ungoverned cassette set"
    );
    assert!(
        recorded.contains("LEGION_AI_GOVERNORS: \"off\""),
        "the raw legs must run under the ungoverned seam, or they measure the governed loop"
    );
    // Both the run and the verify must be present: replaying without
    // verifying produces a report nothing compares to a baseline.
    let raw_steps = recorded.matches("evals/legion-bench/recorded-raw").count();
    assert!(
        raw_steps >= 2,
        "expected both a raw replay and a raw verify step, found {raw_steps} reference(s)"
    );
}

#[test]
fn no_gating_workflow_invokes_a_live_bench_mode() {
    let live_only = "legion-bench-live.yml";
    for (workflow_name, mode) in bench_invocations() {
        if workflow_name == live_only {
            continue;
        }
        assert_eq!(
            mode, "recorded",
            "{workflow_name} invokes legion-bench in `{mode}` mode; only \
             {live_only} may use a live mode, because every other workflow can gate a merge"
        );
    }
}

#[test]
fn the_live_workflow_can_never_gate_a_merge() {
    let live = workflow("legion-bench-live.yml");

    // No push/PR trigger. A `branches:` key can legitimately appear under a
    // schedule-free file, so the triggers themselves are what is asserted.
    for trigger in ["\non:", "\r\non:"] {
        if let Some(rest) = live.split(trigger).nth(1) {
            let header = rest.split("\njobs:").next().unwrap_or(rest);
            assert!(
                !header.contains("push:"),
                "the live bench workflow must not trigger on push"
            );
            assert!(
                !header.contains("pull_request:"),
                "the live bench workflow must not trigger on pull_request"
            );
            assert!(
                header.contains("schedule:"),
                "the live bench workflow is the scheduled arm and must declare a schedule"
            );
        }
    }

    assert!(
        live.contains("continue-on-error: true"),
        "the live bench job must be continue-on-error so a provider outage cannot fail CI"
    );
    assert!(
        live.contains("--no-strict"),
        "the live run must be report-only: a model's task failures are data, not a CI failure"
    );
    assert!(
        live.contains("vars.LEGION_BENCH_LIVE"),
        "the live job must be opt-in behind a repository variable"
    );
}
