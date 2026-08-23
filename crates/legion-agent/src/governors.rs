//! Waste containment for the delegated-task loop.
//!
//! Three governors, ported behaviourally from SmallCode (MIT, see
//! `docs/legal/smallcode-attribution.md`) and reimplemented against Legion's
//! authority model per ADR-0049:
//!
//! * **Dedup** — a read-only call the model already made returns the previous
//!   answer instead of re-executing.
//! * **Read-first hint** — an edit that fails on a file the model never read
//!   is told, once, to read it.
//! * **Idle-turn stop** — a run that stops producing new information ends,
//!   rather than grinding to budget exhaustion.
//!
//! None of these expand what the agent may do. They do not grant a
//! capability, widen a scope, or let anything reach the workspace without
//! human review; each one either declines to repeat work or stops a run
//! earlier than the budget would have. That classification is what keeps this
//! module inside the master plan's §5.3 anti-scope rules, and it is the
//! posture `claim-audit` checks for.
//!
//! Every governor here is behind the `LEGION_AI_GOVERNORS` switch so the
//! measurement arm (`plans/evidence/production/BENCH/`) sees the pre-port
//! loop.

use std::collections::{HashMap, HashSet};

/// Consecutive turns without new information before a run is stopped.
///
/// Three, not one: a single unproductive turn is normal — the model reads
/// something that turns out to be irrelevant, or re-reads a file after a
/// rejection. Three in a row is a loop.
pub const DEFAULT_MAX_IDLE_TURNS: u32 = 3;

/// Consecutive failures of one tool before the model is told to stop using it.
///
/// Three, matching the idle-turn threshold: two failures is a model correcting
/// itself, which is the loop working. The third is the point where the pattern
/// is the information.
pub const DEFAULT_MAX_TOOL_FAILURES: u32 = 3;

/// Tools whose results depend only on worktree state, so a repeated identical
/// call within one run cannot produce a different answer.
///
/// `edit-as-proposal` is deliberately absent. Edits do not write to the
/// worktree — they stage content and emit proposals for review — so they are
/// not a source of divergence for reads, but an identical *repeated* edit is
/// a signal worth executing rather than papering over: the model may be
/// retrying after a failure, and handing back a cached success would hide
/// that.
const CACHEABLE_TOOLS: &[&str] = &["read", "grep", "glob", "outline"];

/// Loop state for the three governors.
///
/// Constructed once per run. All methods are no-ops when `enabled` is false.
#[derive(Debug)]
pub struct LoopGovernors {
    enabled: bool,
    /// Output of each read-only call already executed this run, keyed by a
    /// canonical (tool, arguments) fingerprint.
    cached_reads: HashMap<String, String>,
    /// Paths the model has read.
    read_paths: HashSet<String>,
    /// Paths already hinted once about reading before editing.
    ///
    /// The hint fires at most once per path, so a model that ignores it does
    /// not receive the same advice on every subsequent failure in place of
    /// the diagnostic that would actually help it.
    hinted_paths: HashSet<String>,
    /// Consecutive turns that produced no new information.
    idle_turns: u32,
    max_idle_turns: u32,
    /// Consecutive failures per tool, reset by that tool succeeding.
    ///
    /// Per tool rather than per run, which is the difference from
    /// `max_consecutive_retries`: that counter terminates the run when
    /// *anything* keeps failing, and cannot tell "this model is stuck" from
    /// "this one tool does not work here". A `grep` that fails four times
    /// while `read` keeps working is the second case, and the useful response
    /// is to say so rather than to end the run.
    tool_failures: HashMap<String, u32>,
    /// Tools already told they are failing, so the notice fires once each.
    ///
    /// Repeating it every turn would push the actual diagnostic further from
    /// the model's attention on exactly the turns it needs it most.
    demoted_tools: HashSet<String>,
    max_tool_failures: u32,
}

impl LoopGovernors {
    /// Create governor state. `enabled` follows `LEGION_AI_GOVERNORS`.
    pub fn new(enabled: bool) -> Self {
        Self {
            enabled,
            cached_reads: HashMap::new(),
            read_paths: HashSet::new(),
            hinted_paths: HashSet::new(),
            idle_turns: 0,
            max_idle_turns: DEFAULT_MAX_IDLE_TURNS,
            tool_failures: HashMap::new(),
            demoted_tools: HashSet::new(),
            max_tool_failures: DEFAULT_MAX_TOOL_FAILURES,
        }
    }

    /// Whether any governor is active.
    pub fn enabled(&self) -> bool {
        self.enabled
    }

    /// Previously computed output for an identical read-only call, if any.
    pub fn cached_result(&self, tool: &str, arguments: &serde_json::Value) -> Option<&str> {
        if !self.enabled || !CACHEABLE_TOOLS.contains(&tool) {
            return None;
        }
        self.cached_reads
            .get(&fingerprint(tool, arguments))
            .map(String::as_str)
    }

    /// Discard cached reads if `tool` could have changed the worktree.
    ///
    /// Called on the failure path as well as the success one. A
    /// `terminal-command` that is killed on timeout still ran, and may have
    /// rewritten files before it died — serving a pre-command read afterwards
    /// would have the model editing against state that no longer exists.
    /// Errors are the case where that is *most* likely, so treating only
    /// success as invalidating would get it exactly backwards.
    pub fn note_possible_mutation(&mut self, tool: &str) {
        if self.enabled && !CACHEABLE_TOOLS.contains(&tool) {
            self.cached_reads.clear();
        }
    }

    /// Record a completed tool call.
    ///
    /// A call to any tool outside `CACHEABLE_TOOLS` invalidates the whole read
    /// cache: `terminal-command` can modify the worktree, so a read taken
    /// before it ran is no longer an answer to the same question.
    pub fn record_execution(&mut self, tool: &str, arguments: &serde_json::Value, output: &str) {
        if !self.enabled {
            return;
        }
        if CACHEABLE_TOOLS.contains(&tool) {
            self.cached_reads
                .insert(fingerprint(tool, arguments), output.to_string());
        } else {
            self.cached_reads.clear();
        }
        if tool == "read"
            && let Some(path) = path_argument(arguments)
        {
            self.read_paths.insert(path);
        }
    }

    /// Whether a *failed* edit to `path` should be told to read it first.
    ///
    /// This deliberately does not refuse the edit up front. An earlier version
    /// did, and it cost a model that emits one edit and ends its turn the
    /// edit entirely — the nudge destroyed the only work the run produced.
    /// Legion already matches edits against exact file text and reports a
    /// nearest-candidate diagnostic when they miss, so a pre-emptive refusal
    /// buys nothing that failing loudly does not; the useful moment is after
    /// a miss, when "read the file" is the actual remedy.
    ///
    /// Returns false for a path the model has read, a path already hinted
    /// once, and a path that does not exist — a file being created has
    /// nothing to read.
    pub fn should_hint_read_first(&mut self, path: &str, exists: bool) -> bool {
        if !self.enabled || !exists || self.read_paths.contains(path) {
            return false;
        }
        // `insert` returns false when the path was already hinted.
        self.hinted_paths.insert(path.to_string())
    }

    /// Record the outcome of a model turn; returns true when the run should
    /// stop for lack of progress.
    ///
    /// `produced_new_signal` is true when the turn executed at least one tool
    /// call that was not served from cache, **or** had a call rejected.
    ///
    /// Rejections count deliberately. They are not progress, but they are
    /// already governed by `max_consecutive_retries`, which ends the run with
    /// a diagnostic naming the actual cause ("unparseable tool arguments four
    /// times in a row"). Counting them here too would let this governor fire
    /// first and replace that diagnostic with a vaguer one. This governor
    /// owns the case no budget catches: a model calmly re-asking questions it
    /// has already had answered.
    pub fn note_turn(&mut self, produced_new_signal: bool) -> bool {
        if !self.enabled {
            return false;
        }
        if produced_new_signal {
            self.idle_turns = 0;
            return false;
        }
        self.idle_turns += 1;
        self.idle_turns >= self.max_idle_turns
    }

    /// Record that `tool` failed, and say so once it has failed enough.
    ///
    /// Returns the notice to append to the failure the model is already being
    /// shown, or `None`. Advisory only: nothing is withdrawn and nothing stops.
    /// ADR-0049 classifies these governors as waste containment rather than
    /// autonomy, and removing a tool mid-run would change what the model is
    /// permitted to do — the scope decides that, not a failure count.
    pub fn note_tool_failure(&mut self, tool: &str) -> Option<String> {
        if !self.enabled {
            return None;
        }
        let failures = self.tool_failures.entry(tool.to_string()).or_insert(0);
        *failures += 1;
        let failures = *failures;
        if failures < self.max_tool_failures || !self.demoted_tools.insert(tool.to_string()) {
            return None;
        }
        Some(demotion_notice(tool, failures))
    }

    /// Record that `tool` worked, clearing its failure streak.
    ///
    /// The streak is consecutive, so a success is what makes it not a pattern.
    /// The demotion itself is not cleared: the model has already been told, and
    /// re-arming the notice would let a tool that fails, works once, then fails
    /// again deliver the same advice repeatedly.
    pub fn note_tool_success(&mut self, tool: &str) {
        if !self.enabled {
            return;
        }
        self.tool_failures.remove(tool);
    }

    /// Consecutive failures recorded for `tool`.
    pub fn tool_failures(&self, tool: &str) -> u32 {
        self.tool_failures.get(tool).copied().unwrap_or(0)
    }

    /// Consecutive turns without new information.
    pub fn idle_turns(&self) -> u32 {
        self.idle_turns
    }
}

/// Text handed back in place of a re-executed read.
///
/// The cached output is included verbatim so the model is not deprived of the
/// information; only the repeated work is declined.
pub fn dedup_notice(tool: &str, cached: &str) -> String {
    format!(
        "You already called `{tool}` with these exact arguments in this run. \
         Its result is unchanged and repeated below. Use it rather than \
         calling again.\n\n{cached}"
    )
}

/// The notice appended when one tool keeps failing.
///
/// Names the count, because "it failed" is already in the diagnostic above it
/// and the number is the part the model cannot see. Suggests a different route
/// rather than forbidding this one: the tool may still be the right choice with
/// different arguments, and this governor has no way to know.
pub fn demotion_notice(tool: &str, failures: u32) -> String {
    format!(
        "`{tool}` has now failed {failures} times in a row. Treat it as unreliable \
         for this task and reach the same goal another way if one exists."
    )
}

/// Hint appended to a failed edit on a file the model never read.
pub fn read_first_hint(path: &str) -> String {
    format!(
        "\n\nYou have not read `{path}` in this run. Edits are matched against \
         the file's exact current text, so read it and copy the anchor from \
         what you see rather than writing it from memory."
    )
}

/// Stable string for a (tool, arguments) pair.
///
/// Object keys are sorted so argument order from the model cannot make two
/// identical calls look different.
fn fingerprint(tool: &str, arguments: &serde_json::Value) -> String {
    let mut out = String::with_capacity(64);
    out.push_str(tool);
    out.push('\u{1}');
    write_canonical(arguments, &mut out);
    out
}

fn write_canonical(value: &serde_json::Value, out: &mut String) {
    match value {
        serde_json::Value::Object(map) => {
            let mut keys: Vec<&String> = map.keys().collect();
            keys.sort();
            out.push('{');
            for key in keys {
                out.push_str(key);
                out.push(':');
                if let Some(v) = map.get(key) {
                    write_canonical(v, out);
                }
                out.push(',');
            }
            out.push('}');
        }
        serde_json::Value::Array(items) => {
            out.push('[');
            for item in items {
                write_canonical(item, out);
                out.push(',');
            }
            out.push(']');
        }
        other => out.push_str(&other.to_string()),
    }
}

/// The `path` argument of a tool call, if it has one.
fn path_argument(arguments: &serde_json::Value) -> Option<String> {
    arguments
        .get("path")
        .and_then(serde_json::Value::as_str)
        .map(str::to_string)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn a_repeated_read_is_served_from_cache() {
        let mut g = LoopGovernors::new(true);
        let args = json!({"path": "src/lib.rs"});
        assert!(g.cached_result("read", &args).is_none());
        g.record_execution("read", &args, "fn main() {}");
        assert_eq!(g.cached_result("read", &args), Some("fn main() {}"));
    }

    #[test]
    fn argument_order_does_not_defeat_dedup() {
        let mut g = LoopGovernors::new(true);
        g.record_execution("grep", &json!({"pattern": "fn", "path": "src"}), "hit");
        assert_eq!(
            g.cached_result("grep", &json!({"path": "src", "pattern": "fn"})),
            Some("hit"),
            "the same call written with keys in a different order is the same call"
        );
    }

    #[test]
    fn different_arguments_are_different_calls() {
        let mut g = LoopGovernors::new(true);
        g.record_execution("read", &json!({"path": "a.rs"}), "A");
        assert!(g.cached_result("read", &json!({"path": "b.rs"})).is_none());
    }

    #[test]
    fn an_edit_is_never_served_from_cache() {
        let mut g = LoopGovernors::new(true);
        let args = json!({"path": "a.rs", "replacement": "x"});
        g.record_execution("edit-as-proposal", &args, "proposed");
        assert!(
            g.cached_result("edit-as-proposal", &args).is_none(),
            "a repeated edit may be a retry; returning a cached success would hide the failure"
        );
    }

    #[test]
    fn a_worktree_mutating_tool_invalidates_cached_reads() {
        let mut g = LoopGovernors::new(true);
        let read = json!({"path": "a.rs"});
        g.record_execution("read", &read, "before");
        g.record_execution("terminal-command", &json!({"command": "cargo fmt"}), "ok");
        assert!(
            g.cached_result("read", &read).is_none(),
            "a command may have rewritten the file, so the earlier read is no longer an answer"
        );
    }

    #[test]
    fn a_failed_command_also_invalidates_cached_reads() {
        let mut g = LoopGovernors::new(true);
        let read = json!({"path": "a.rs"});
        g.record_execution("read", &read, "before");
        // A command killed on timeout may still have rewritten files first.
        g.note_possible_mutation("terminal-command");
        assert!(
            g.cached_result("read", &read).is_none(),
            "failure is when a partial write is *most* likely, so treating only              success as invalidating gets it exactly backwards"
        );
    }

    #[test]
    fn a_failed_read_does_not_invalidate_the_cache() {
        let mut g = LoopGovernors::new(true);
        let read = json!({"path": "a.rs"});
        g.record_execution("read", &read, "contents");
        g.note_possible_mutation("read");
        assert_eq!(
            g.cached_result("read", &read),
            Some("contents"),
            "a read cannot have changed anything, however it ended"
        );
    }

    #[test]
    fn a_failed_edit_on_an_unread_file_is_hinted_once() {
        let mut g = LoopGovernors::new(true);
        assert!(g.should_hint_read_first("a.rs", true));
        assert!(
            !g.should_hint_read_first("a.rs", true),
            "repeating the hint would crowd out the nearest-candidate \
             diagnostic, which is the more useful half of the feedback"
        );
    }

    #[test]
    fn a_file_that_was_read_is_never_hinted() {
        let mut g = LoopGovernors::new(true);
        g.record_execution("read", &json!({"path": "a.rs"}), "contents");
        assert!(!g.should_hint_read_first("a.rs", true));
    }

    #[test]
    fn creating_a_new_file_is_never_hinted() {
        let mut g = LoopGovernors::new(true);
        assert!(
            !g.should_hint_read_first("new.rs", false),
            "a file that does not exist has nothing to read"
        );
    }

    #[test]
    fn three_turns_of_nothing_new_stop_the_run() {
        let mut g = LoopGovernors::new(true);
        assert!(!g.note_turn(false));
        assert!(!g.note_turn(false));
        assert!(g.note_turn(false));
    }

    #[test]
    fn progress_resets_the_idle_counter() {
        let mut g = LoopGovernors::new(true);
        g.note_turn(false);
        g.note_turn(false);
        g.note_turn(true);
        assert_eq!(g.idle_turns(), 0);
        assert!(!g.note_turn(false));
        assert!(!g.note_turn(false));
        assert!(g.note_turn(false));
    }

    #[test]
    fn every_governor_is_inert_when_disabled() {
        let mut g = LoopGovernors::new(false);
        let args = json!({"path": "a.rs"});
        g.record_execution("read", &args, "contents");
        assert!(
            g.cached_result("read", &args).is_none(),
            "the measurement arm must see the pre-port loop"
        );
        assert!(!g.should_hint_read_first("b.rs", true));
        for _ in 0..10 {
            assert!(!g.note_turn(false));
        }
    }

    #[test]
    fn the_dedup_notice_carries_the_cached_output() {
        let notice = dedup_notice("read", "fn main() {}");
        assert!(
            notice.contains("fn main() {}"),
            "declining to repeat the work must not withhold the answer"
        );
    }
}

#[cfg(test)]
mod trust_decay_tests {
    use super::*;

    #[test]
    fn a_tool_is_left_alone_until_it_has_failed_enough() {
        let mut governors = LoopGovernors::new(true);

        for _ in 1..DEFAULT_MAX_TOOL_FAILURES {
            assert!(
                governors.note_tool_failure("grep").is_none(),
                "correcting itself twice is the loop working, not a pattern"
            );
        }
        let notice = governors
            .note_tool_failure("grep")
            .expect("the third consecutive failure is the point the pattern is information");
        assert!(notice.contains("grep"));
        assert!(notice.contains("3 times"));
    }

    /// The notice fires once, not on every subsequent failure.
    ///
    /// Repeating it would push the actual diagnostic further from the model's
    /// attention on exactly the turns it needs it most.
    #[test]
    fn a_demoted_tool_is_not_told_again() {
        let mut governors = LoopGovernors::new(true);
        for _ in 0..DEFAULT_MAX_TOOL_FAILURES {
            let _ = governors.note_tool_failure("grep");
        }

        assert!(governors.note_tool_failure("grep").is_none());
        assert!(governors.note_tool_failure("grep").is_none());
    }

    /// A success clears the streak, because the streak is consecutive.
    #[test]
    fn a_success_ends_the_streak() {
        let mut governors = LoopGovernors::new(true);
        governors.note_tool_failure("grep");
        governors.note_tool_failure("grep");
        assert_eq!(governors.tool_failures("grep"), 2);

        governors.note_tool_success("grep");

        assert_eq!(governors.tool_failures("grep"), 0);
        assert!(
            governors.note_tool_failure("grep").is_none(),
            "one failure after a success is one failure, not the third"
        );
    }

    /// Failures are counted per tool, not pooled.
    ///
    /// This is the whole difference from `max_consecutive_retries`, which counts
    /// across every tool and ends the run. A `grep` that keeps failing while
    /// `read` keeps working is one tool being wrong, not a model being stuck.
    #[test]
    fn one_tools_failures_do_not_demote_another() {
        let mut governors = LoopGovernors::new(true);
        governors.note_tool_failure("grep");
        governors.note_tool_failure("grep");

        assert!(governors.note_tool_failure("read").is_none());
        assert_eq!(governors.tool_failures("read"), 1);
        assert_eq!(governors.tool_failures("grep"), 2);
    }

    /// Disabled governors record nothing and advise nothing.
    ///
    /// `LEGION_AI_GOVERNORS=off` has to leave the raw baseline measuring the
    /// un-ported loop, or the bench's A/B arms stop being comparable.
    #[test]
    fn a_disabled_governor_never_demotes() {
        let mut governors = LoopGovernors::new(false);
        for _ in 0..(DEFAULT_MAX_TOOL_FAILURES * 3) {
            assert!(governors.note_tool_failure("grep").is_none());
        }
        assert_eq!(governors.tool_failures("grep"), 0);
    }
}
