//! Corpus health: does each task actually distinguish a working agent from a
//! dead one?
//!
//! A benchmark task is only worth running if its verification command reports
//! something different before and after the change it asks for. This corpus has
//! shipped tasks that failed that bar more than once, and each time it took a
//! full suite run and a confused reading of the results to notice:
//!
//! * Four refactor tasks pass on the untouched fixture — correctly, since a
//!   refactor preserves behaviour — which made `tests_passed` read as an
//!   achievement and let a model making **zero tool calls** score four of
//!   thirteen "passing" tasks.
//! * Every task granted `terminal-command` while the prompts told the model to
//!   run verification the harness already performs, so runs ended in a denial
//!   that had nothing to do with model capability.
//!
//! Both were found by reading benchmark output that looked wrong. This gate
//! finds that class mechanically instead, by running each task's verification
//! command against its pristine fixture and checking the result matches what
//! the task's kind implies:
//!
//! Failing at rest is always sound: the model has to make the command pass, so
//! the exit code alone proves work happened. Passing at rest is defensible only
//! for a `refactor`, where keeping the tests green *is* the goal — and even
//! there it is the weaker design, because the gate then rests entirely on
//! `task_success`. A `bug_fix` whose tests already pass has no bug; a
//! `test_add` whose suite already passes is asking for a test that changes
//! nothing observable.
//!
//! It is deliberately a *corpus* check and not a model check: it never calls a
//! provider, so it runs offline in CI in seconds.

use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    process::Command,
    time::Duration,
};

use crate::legion_bench_corpus::{CorpusTask, load_corpus};

/// What a task's kind implies about its pristine fixture.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AtRestExpectation {
    /// Verification must succeed before the model runs.
    Passes,
    /// Verification must fail before the model runs.
    Fails,
}

/// The at-rest state a task of this kind must have.
pub fn expected_at_rest(kind_name: &str) -> AtRestExpectation {
    match kind_name {
        "refactor" => AtRestExpectation::Passes,
        _ => AtRestExpectation::Fails,
    }
}

/// One task's health verdict.
#[derive(Debug, Clone)]
pub struct TaskHealth {
    /// Task id.
    pub id: String,
    /// Problems found. Empty means healthy.
    pub problems: Vec<String>,
}

/// Check every structural property that does not require running anything.
///
/// Split from the execution check so the cheap failures report immediately and
/// so this half stays unit-testable without a fixture on disk.
pub fn check_task_statically(task: &CorpusTask, repo_root: &Path) -> Vec<String> {
    let mut problems = Vec::new();
    let fixture = repo_root.join(&task.task.fixture_repo);
    if !fixture.is_dir() {
        problems.push(format!(
            "fixture_repo `{}` does not exist",
            task.task.fixture_repo
        ));
        return problems;
    }

    if task.live.verification.expected_files.is_empty() {
        problems.push(
            "expected_files is empty, so `task_success` cannot check the model touched anything"
                .to_string(),
        );
    }

    // `test_add` and `multi_file_feature` exist to create files, so naming a
    // path that is not there yet is the point. A `refactor` may create one too
    // — moving a type into a new module is a refactor — so the requirement for
    // the code-changing kinds is that they name *something* that already
    // exists, not that everything they name does.
    if matches!(task.task.kind.as_str(), "bug_fix" | "refactor")
        && !task
            .live
            .verification
            .expected_files
            .iter()
            .any(|f| fixture.join(f).exists())
    {
        problems.push(format!(
            "kind `{}` changes existing code, but none of its expected_files are \
             present in the fixture: {:?}",
            task.task.kind.as_str(),
            task.live.verification.expected_files
        ));
    }

    // The corpus prompts tell the model that commands are unavailable, so a
    // task granting a terminal contradicts its own instructions.
    if task
        .live
        .scope
        .allowed_tools
        .iter()
        .any(|t| t == "terminal-command")
    {
        problems.push(
            "grants `terminal-command`, but the prompts state that commands are unavailable"
                .to_string(),
        );
    }

    if task.live.verification.timeout_secs == 0 {
        problems.push("timeout_secs is 0".to_string());
    }
    problems
}

/// Run one task's verification command against its pristine fixture.
///
/// Returns the exit code, or `None` if the command timed out.
fn run_at_rest(task: &CorpusTask, repo_root: &Path) -> Result<Option<i32>, String> {
    let fixture = repo_root.join(&task.task.fixture_repo);
    let command = &task.live.verification.command;

    let (shell, flag) = if cfg!(windows) {
        ("cmd", "/C")
    } else {
        ("sh", "-c")
    };
    let mut child = Command::new(shell)
        .arg(flag)
        .arg(command)
        .current_dir(&fixture)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .map_err(|e| format!("spawn `{command}` failed: {e}"))?;

    let deadline =
        std::time::Instant::now() + Duration::from_secs(task.live.verification.timeout_secs.max(1));
    loop {
        match child.try_wait() {
            Ok(Some(status)) => return Ok(Some(status.code().unwrap_or(-1))),
            Ok(None) => {
                if std::time::Instant::now() >= deadline {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Ok(None);
                }
                std::thread::sleep(Duration::from_millis(100));
            }
            Err(e) => return Err(format!("waiting on `{command}` failed: {e}")),
        }
    }
}

/// Check the whole corpus.
///
/// `execute` runs each verification command; skipping it keeps the check
/// offline-fast when only structure is in question.
pub fn check_corpus(
    corpus_dir: &Path,
    repo_root: &Path,
    execute: bool,
) -> Result<Vec<TaskHealth>, String> {
    let tasks = load_corpus(corpus_dir)?;
    if tasks.is_empty() {
        return Err(format!("no corpus tasks found in {}", corpus_dir.display()));
    }

    let mut seen_ids: BTreeMap<String, usize> = BTreeMap::new();
    let mut report = Vec::with_capacity(tasks.len());

    for task in &tasks {
        let mut problems = check_task_statically(task, repo_root);
        *seen_ids.entry(task.task.id.clone()).or_insert(0) += 1;

        if execute && problems.is_empty() {
            let kind = task.task.kind.as_str();
            let expectation = expected_at_rest(kind);
            match run_at_rest(task, repo_root) {
                Ok(Some(code)) => {
                    let passed = code == task.live.verification.expected_exit;
                    // Failing at rest is always sound: the model has to make it
                    // pass, so the exit code alone proves work happened.
                    //
                    // Passing at rest is only defensible for a `refactor`,
                    // where keeping the tests green *is* the goal. Even then it
                    // is the weaker design, because the gate then rests
                    // entirely on `task_success` — and a refactor verified by a
                    // script that checks the restructuring happened, which
                    // fails at rest, is better still.
                    if passed && expectation == AtRestExpectation::Fails {
                        problems.push(format!(
                            "kind `{kind}` already passes on the untouched fixture, so the task \
                             cannot distinguish a working agent from one that does nothing"
                        ));
                    }
                }
                Ok(None) => problems.push(format!(
                    "verification command timed out after {}s on the untouched fixture",
                    task.live.verification.timeout_secs
                )),
                Err(e) => problems.push(e),
            }
        }

        report.push(TaskHealth {
            id: task.task.id.clone(),
            problems,
        });
    }

    for (id, count) in seen_ids {
        if count > 1 {
            report.push(TaskHealth {
                id: id.clone(),
                problems: vec![format!("duplicate task id appears {count} times")],
            });
        }
    }

    Ok(report)
}

/// Render the report and return whether the corpus is healthy.
pub fn report_corpus_health(report: &[TaskHealth]) -> bool {
    let unhealthy: Vec<&TaskHealth> = report.iter().filter(|t| !t.problems.is_empty()).collect();
    for task in &unhealthy {
        for problem in &task.problems {
            eprintln!("legion-bench corpus: {}: {problem}", task.id);
        }
    }
    println!(
        "legion-bench corpus health: {} task(s), {} healthy, {} with problems",
        report.len(),
        report.len() - unhealthy.len(),
        unhealthy.len()
    );
    unhealthy.is_empty()
}

/// Default corpus directory relative to the repo root.
pub fn default_corpus_dir(repo_root: &Path) -> PathBuf {
    repo_root.join(crate::legion_bench_corpus::DEFAULT_CORPUS_PATH)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_refactor_must_pass_before_the_model_runs() {
        assert_eq!(expected_at_rest("refactor"), AtRestExpectation::Passes);
    }

    #[test]
    fn every_other_kind_must_fail_before_the_model_runs() {
        for kind in ["bug_fix", "test_add", "multi_file_feature"] {
            assert_eq!(
                expected_at_rest(kind),
                AtRestExpectation::Fails,
                "`{kind}` that already passes cannot distinguish a working agent \
                 from one that does nothing"
            );
        }
    }
}
