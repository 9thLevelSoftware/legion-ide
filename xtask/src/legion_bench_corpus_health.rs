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
    path::{Path, PathBuf},
    process::Command,
    time::Duration,
};

use crate::legion_bench::LegionBenchTaskKind;
use crate::legion_bench_corpus::{CorpusTask, load_corpus};

/// What a task's kind implies about its pristine fixture.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AtRestExpectation {
    /// Verification must succeed before the model runs.
    Passes,
    /// Verification must fail before the model runs.
    Fails,
}

/// The at-rest state a task must have.
///
/// Takes the enum rather than its name so a new task kind is a compile error
/// here instead of a silent default — this module exists to stop a task being
/// scored wrongly, and guessing at an unknown kind is the same failure in
/// miniature.
pub fn expected_at_rest(
    kind: LegionBenchTaskKind,
    override_value: Option<&str>,
) -> Result<AtRestExpectation, String> {
    match override_value {
        Some("passes") => return Ok(AtRestExpectation::Passes),
        Some("fails") => return Ok(AtRestExpectation::Fails),
        Some(other) => {
            return Err(format!(
                "at_rest must be \"passes\" or \"fails\", got {other:?}"
            ));
        }
        None => {}
    }
    Ok(match kind {
        // Preserves behaviour, so the existing suite is green before and after.
        LegionBenchTaskKind::Refactor => AtRestExpectation::Passes,
        // Passing has to prove work, so the command must start red.
        LegionBenchTaskKind::BugFix
        | LegionBenchTaskKind::TestAdd
        | LegionBenchTaskKind::MultiFileFeature
        | LegionBenchTaskKind::HostileEval => AtRestExpectation::Fails,
    })
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

    problems
}

/// Run one task's verification command against a throwaway copy of its
/// fixture.
///
/// A copy, not the fixture, because these commands build: running `cargo test`
/// in `fixtures/bench-rust-lib` leaves a gitignored `target/` behind, and the
/// live runner's checkout step copies every file it finds with no ignore
/// rules — so one health check would make every later task deep-copy hundreds
/// of megabytes, once per task, per run. A gate that measures the corpus must
/// not be the thing that degrades it.
///
/// It also contains the timeout path. Killing the child kills the shell, not
/// the `cargo` and `rustc` grandchildren under it; those keep writing until
/// they finish, and here they write into a directory nothing else will read.
///
/// Returns the exit code, or `None` if the command timed out.
fn run_at_rest(task: &CorpusTask, repo_root: &Path) -> Result<Option<i32>, String> {
    let source = repo_root.join(&task.task.fixture_repo);
    let scratch = std::env::temp_dir().join(format!(
        "legion-bench-health-{}-{}",
        task.task.id,
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&scratch);
    copy_tree(&source, &scratch).map_err(|e| format!("copying fixture failed: {e}"))?;
    let result = run_in(&scratch, task);
    // Report a failed cleanup rather than swallowing it. A build tree left
    // behind is tens of megabytes, and 25 of them accumulating silently on a
    // CI box is the kind of leak nobody attributes to the corpus gate.
    if let Err(err) = std::fs::remove_dir_all(&scratch) {
        eprintln!(
            "legion-bench corpus: could not remove {}: {err}",
            scratch.display()
        );
    }
    result
}

/// Copy a fixture, skipping build output that would make the copy enormous.
fn copy_tree(from: &Path, to: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(to)?;
    for entry in std::fs::read_dir(from)? {
        let entry = entry?;
        let name = entry.file_name();
        if matches!(
            name.to_string_lossy().as_ref(),
            "target" | "node_modules" | "__pycache__" | ".git"
        ) {
            continue;
        }
        let target = to.join(&name);
        if entry.file_type()?.is_dir() {
            copy_tree(&entry.path(), &target)?;
        } else {
            std::fs::copy(entry.path(), &target)?;
        }
    }
    Ok(())
}

fn run_in(fixture: &Path, task: &CorpusTask) -> Result<Option<i32>, String> {
    let command = &task.live.verification.command;

    let (shell, flag) = if cfg!(windows) {
        ("cmd", "/C")
    } else {
        ("sh", "-c")
    };
    let mut child = Command::new(shell)
        .arg(flag)
        .arg(command)
        .current_dir(fixture)
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
    // `load_corpus` already rejects an empty directory, a duplicate id and a
    // zero timeout, so this checks only what it cannot: whether a
    // well-formed task can actually be scored.
    let tasks = load_corpus(corpus_dir)?;
    let mut report = Vec::with_capacity(tasks.len());

    for task in &tasks {
        let mut problems = check_task_statically(task, repo_root);

        if execute && problems.is_empty() {
            let kind = task.task.kind.as_str();
            let expectation =
                match expected_at_rest(task.task.kind, task.live.verification.at_rest.as_deref()) {
                    Ok(expectation) => expectation,
                    Err(message) => {
                        report.push(TaskHealth {
                            id: task.task.id.clone(),
                            problems: vec![message],
                        });
                        continue;
                    }
                };
            match run_at_rest(task, repo_root) {
                Ok(Some(code)) => {
                    let passed = code == task.live.verification.expected_exit;
                    match (expectation, passed) {
                        (AtRestExpectation::Fails, true) => problems.push(format!(
                            "kind `{kind}` already passes on the untouched fixture, so the task \
                             cannot distinguish a working agent from one that does nothing"
                        )),
                        // The case that matters most, and the one this gate
                        // originally missed: a task is unwinnable when its
                        // command is red for a reason the model is not allowed
                        // to fix — most easily when another task in the same
                        // fixture deliberately breaks a shared test.
                        (AtRestExpectation::Passes, false) => problems.push(format!(
                            "expected to pass on the untouched fixture but exited {code} \
                             (expected {}). Either the task is unwinnable — check whether \
                             another task in this fixture broke a test this one runs but \
                             cannot edit — or it is verified by a script that checks the \
                             change happened, in which case set `at_rest = \"fails\"`",
                            task.live.verification.expected_exit
                        )),
                        _ => {}
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
        assert_eq!(
            expected_at_rest(LegionBenchTaskKind::Refactor, None),
            Ok(AtRestExpectation::Passes)
        );
    }

    #[test]
    fn every_other_kind_must_fail_before_the_model_runs() {
        for kind in [
            LegionBenchTaskKind::BugFix,
            LegionBenchTaskKind::TestAdd,
            LegionBenchTaskKind::MultiFileFeature,
            // Listed so the match arm has a witness. Without it the next
            // hostile-style task copies this loop, sees green, and never
            // learns that nothing else looks at this variant.
            LegionBenchTaskKind::HostileEval,
        ] {
            assert_eq!(
                expected_at_rest(kind, None),
                Ok(AtRestExpectation::Fails),
                "a {kind:?} that already passes cannot distinguish a working agent \
                 from one that does nothing"
            );
        }
    }

    #[test]
    fn an_explicit_override_wins_over_the_kind_default() {
        assert_eq!(
            expected_at_rest(LegionBenchTaskKind::Refactor, Some("fails")),
            Ok(AtRestExpectation::Fails),
            "a refactor verified by a script that checks the change happened is \
             red at rest on purpose"
        );
    }

    #[test]
    fn an_unrecognised_override_is_rejected_rather_than_guessed() {
        assert!(expected_at_rest(LegionBenchTaskKind::BugFix, Some("maybe")).is_err());
    }
}
