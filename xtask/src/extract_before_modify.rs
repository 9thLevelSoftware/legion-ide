//! The extract-before-modify gate: chokepoint files may shrink, not grow.
//!
//! `crates/legion-app/src/lib.rs` is around 37,500 lines. Everything in the
//! product eventually routes through it, so every branch that adds a feature
//! adds lines to the same file, and every one of those branches conflicts with
//! every other. Under parallel development that is the single largest tax on
//! the project, and it compounds: the bigger the file gets, the more likely any
//! two changes collide inside it.
//!
//! The rule (production roadmap, cross-cutting rule 1) is that a feature change
//! does not land inside a chokepoint file. The touched region is first moved to
//! a module in its own commit — a pure move, reviewable as a move — and the
//! feature change is then made in the new module, where it is a small diff
//! against a small file.
//!
//! This gate enforces the arithmetic consequence: measured against the merge
//! base, a chokepoint file must not have grown. Shrinking is always fine; that
//! is what an extraction looks like. Growing by a few lines is fine too — a
//! ceiling with no slack turns a one-line bug fix into a refactor — but the
//! slack is small enough that a feature cannot hide in it.
//!
//! The gate is deliberately blunt. It cannot tell a good reason from a bad one,
//! so it does not try; it reports the growth and names the escape hatch, which
//! is to do the extraction the rule already asks for.

use std::path::Path;
use std::process::Command;

use serde::Deserialize;

/// Configuration: which files are chokepoints, and how much slack each gets.
#[derive(Debug, Clone, Deserialize)]
pub struct ExtractBeforeModifyConfig {
    /// Files watched by this gate.
    pub files: Vec<ChokepointFile>,
}

/// One watched file and the growth it tolerates.
#[derive(Debug, Clone, Deserialize)]
pub struct ChokepointFile {
    /// Workspace-relative path.
    pub path: String,
    /// Lines this file may grow by against the merge base before the gate fails.
    ///
    /// Small on purpose. Enough that a bug fix, a doc comment or a new match arm
    /// lands without ceremony; not enough for a feature to live in.
    pub allowed_growth_lines: i64,
    /// Where the extracted code should go, named in the failure message so the
    /// gate tells you the fix rather than only the problem.
    pub extract_to: String,
}

/// One file that grew past its allowance.
#[derive(Debug, Clone)]
pub struct Growth {
    /// Workspace-relative path of the file.
    pub path: String,
    /// Lines at the merge base.
    pub base_lines: i64,
    /// Lines now.
    pub head_lines: i64,
    /// Lines the file was allowed to grow by.
    pub allowed: i64,
    /// Where the extraction should go.
    pub extract_to: String,
}

impl Growth {
    /// Lines added beyond the allowance.
    pub fn overage(&self) -> i64 {
        self.head_lines - self.base_lines - self.allowed
    }
}

/// Why the gate could not reach a verdict.
#[derive(Debug)]
pub enum GateSkip {
    /// No merge base could be found — a shallow clone, or an orphan branch.
    ///
    /// Not a failure. A gate that fails when it cannot measure teaches people
    /// to disable it.
    NoMergeBase(String),
}

impl ExtractBeforeModifyConfig {
    /// Read the config from a TOML file.
    pub fn from_file(path: &Path) -> Result<Self, String> {
        let body = std::fs::read_to_string(path)
            .map_err(|err| format!("cannot read {}: {err}", path.display()))?;
        toml::from_str(&body).map_err(|err| format!("cannot parse {}: {err}", path.display()))
    }
}

/// Compare each watched file against the merge base.
///
/// `Ok(Ok(()))` means every file is within its allowance. `Ok(Err(growths))`
/// means at least one grew too far. `Err(skip)` means the comparison could not
/// be made at all.
pub fn run_extract_before_modify(
    workspace_root: &Path,
    config: &ExtractBeforeModifyConfig,
    base_ref: &str,
) -> Result<Result<(), Vec<Growth>>, GateSkip> {
    let base = merge_base(workspace_root, base_ref)?;

    let mut growths = Vec::new();
    for file in &config.files {
        let head_lines = match count_lines_on_disk(workspace_root, &file.path) {
            Some(count) => count,
            // A watched file that no longer exists has been extracted out of
            // existence, which is the outcome this gate wants.
            None => continue,
        };
        let base_lines = match count_lines_at_revision(workspace_root, &base, &file.path) {
            Some(count) => count,
            // The file is new on this branch. A brand-new file is not a
            // chokepoint yet; it becomes one once it is on the base.
            None => continue,
        };
        if head_lines - base_lines > file.allowed_growth_lines {
            growths.push(Growth {
                path: file.path.clone(),
                base_lines,
                head_lines,
                allowed: file.allowed_growth_lines,
                extract_to: file.extract_to.clone(),
            });
        }
    }

    if growths.is_empty() {
        Ok(Ok(()))
    } else {
        Ok(Err(growths))
    }
}

/// The commit this branch diverged from.
fn merge_base(workspace_root: &Path, base_ref: &str) -> Result<String, GateSkip> {
    let output = Command::new("git")
        .current_dir(workspace_root)
        .args(["merge-base", "HEAD", base_ref])
        .output()
        .map_err(|err| GateSkip::NoMergeBase(format!("cannot run git: {err}")))?;
    if !output.status.success() {
        return Err(GateSkip::NoMergeBase(format!(
            "no merge base between HEAD and {base_ref}: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }
    let base = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if base.is_empty() {
        return Err(GateSkip::NoMergeBase(format!(
            "git merge-base HEAD {base_ref} produced no revision"
        )));
    }
    Ok(base)
}

/// Lines in the working-tree copy of `path`, or `None` if it is not there.
fn count_lines_on_disk(workspace_root: &Path, path: &str) -> Option<i64> {
    let body = std::fs::read_to_string(workspace_root.join(path)).ok()?;
    Some(count_lines(&body))
}

/// Lines in `path` as of `revision`, or `None` if it did not exist then.
fn count_lines_at_revision(workspace_root: &Path, revision: &str, path: &str) -> Option<i64> {
    let output = Command::new("git")
        .current_dir(workspace_root)
        .args(["show", &format!("{revision}:{path}")])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    Some(count_lines(&String::from_utf8_lossy(&output.stdout)))
}

/// Count lines the way a person would: a trailing newline does not add one.
///
/// Named rather than inlined so both sides of the comparison — the working tree
/// and the merge base — provably count the same way. A gate that trips at small
/// numbers cannot afford a one-line disagreement between its two measurements.
fn count_lines(body: &str) -> i64 {
    body.lines().count() as i64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_trailing_newline_does_not_invent_a_line() {
        assert_eq!(count_lines("a\nb\n"), 2);
        assert_eq!(count_lines("a\nb"), 2);
        assert_eq!(count_lines(""), 0);
    }

    #[test]
    fn overage_is_what_exceeds_the_allowance_not_the_whole_growth() {
        let growth = Growth {
            path: "x".to_string(),
            base_lines: 1000,
            head_lines: 1120,
            allowed: 100,
            extract_to: "y".to_string(),
        };
        assert_eq!(
            growth.overage(),
            20,
            "the report must name what is over the line, not the whole diff — \
             otherwise the number reads as though the allowance were zero"
        );
    }

    #[test]
    fn a_config_round_trips_from_toml() {
        let config: ExtractBeforeModifyConfig = toml::from_str(
            r#"
[[files]]
path = "crates/legion-app/src/lib.rs"
allowed_growth_lines = 120
extract_to = "crates/legion-app/src/"
"#,
        )
        .expect("parse");
        assert_eq!(config.files.len(), 1);
        assert_eq!(config.files[0].allowed_growth_lines, 120);
    }
}
