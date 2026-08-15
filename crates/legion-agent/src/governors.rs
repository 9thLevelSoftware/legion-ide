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
