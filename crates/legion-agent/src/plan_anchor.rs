//! Keeping a run pointed at the plan it started with.
//!
//! Ported from SmallCode's `plan_tracker.js` per ADR-0049 — the capture and
//! re-injection half of that row. Nothing here executes anything; it reads the
//! model's own words and, every few turns, puts them back in front of it.
//!
//! The failure it addresses is specific to long tool loops on small models. The
//! task arrives in turn one and is then buried under tool results: by turn ten
//! the conversation is mostly file contents and diagnostics, the original
//! directive is far away, and the model starts solving whatever the last tool
//! output suggested instead of what it was asked. It does not announce this —
//! it just works confidently on the wrong thing, which is why the loop's other
//! governors cannot see it. Idle-turn detection sees progress; dedup sees
//! distinct calls; retry counting sees successes.
//!
//! Re-anchoring is a reminder, not a constraint. The model may legitimately
//! discover the plan was wrong, and this must not stop it saying so.

/// The plan a run is working from, and when to restate it.
#[derive(Debug)]
pub struct PlanAnchor {
    enabled: bool,
    steps: Vec<String>,
    turns_since_anchor: u32,
    reanchor_every: u32,
}

/// Turns between restatements of the plan.
///
/// Four, because that is roughly where the directive stops being visible in a
/// tool-heavy conversation and well before the idle governor's three-turn stop
/// could fire on a model that has quietly changed subject. Restating every turn
/// would be its own kind of noise — the plan would compete with the tool output
/// the model actually needs to read.
pub const DEFAULT_REANCHOR_EVERY: u32 = 4;

/// Most steps a captured plan keeps.
///
/// The plan is re-sent every few turns, so its size is multiplied by the run.
/// A model that answers with a long numbered section, over a 50-turn budget,
/// would add tens of thousands of input tokens in reminders alone -- and the
/// small-context providers this exists to help are exactly the ones that then
/// start refusing requests whose tool output was tiny. A plan too long to
/// restate is also too long to be anchoring anything.
pub const MAX_PLAN_STEPS: usize = 12;

/// Most bytes a captured plan keeps, across all steps.
///
/// A second bound because the first one is not enough: twelve steps of prose
/// is still unbounded. Individual steps are truncated to fit rather than
/// dropped, so a plan stays a plan.
pub const MAX_PLAN_BYTES: usize = 1_200;

/// Fewest steps a list must have before it counts as a plan.
///
/// One bullet is a sentence with a dash in front of it. Two is an ordering, and
/// an ordering is the thing worth holding a model to.
pub const MIN_PLAN_STEPS: usize = 2;

impl PlanAnchor {
    /// Create anchor state. `enabled` follows `LEGION_AI_GOVERNORS`.
    pub fn new(enabled: bool) -> Self {
        Self {
            enabled,
            steps: Vec::new(),
            turns_since_anchor: 0,
            reanchor_every: DEFAULT_REANCHOR_EVERY,
        }
    }

    /// Capture a plan from the model's text, if it stated one and none is held.
    ///
    /// First plan only. A model that restates its plan mid-run is often already
    /// drifting, and adopting the new version would make this mechanism agree
    /// with the drift it exists to catch.
    pub fn capture(&mut self, text: &str) -> bool {
        if !self.enabled || !self.steps.is_empty() {
            return false;
        }
        let steps = bounded_plan(parse_plan_steps(text));
        if steps.len() < MIN_PLAN_STEPS {
            return false;
        }
        self.steps = steps;
        true
    }

    /// The reminder to inject this turn, if one is due.
    ///
    /// Counts turns rather than tool calls: drift is a function of how much
    /// conversation sits between the model and its instructions, and a turn is
    /// what adds conversation.
    pub fn reanchor(&mut self) -> Option<String> {
        if !self.enabled || self.steps.is_empty() {
            return None;
        }
        self.turns_since_anchor += 1;
        if self.turns_since_anchor < self.reanchor_every {
            return None;
        }
        self.turns_since_anchor = 0;
        Some(anchor_notice(&self.steps))
    }

    /// The captured plan, empty when none was stated.
    pub fn steps(&self) -> &[String] {
        &self.steps
    }

    /// Whether a plan is being held.
    pub fn has_plan(&self) -> bool {
        !self.steps.is_empty()
    }
}

/// The reminder text put back in front of the model.
///
/// Phrased as a reminder of what it said, not an instruction from the harness:
/// the model is more likely to reconcile with its own stated plan than to obey
/// a system voice it has been ignoring for ten turns. The last line matters as
/// much as the list — a plan that turned out to be wrong has to be sayable, or
/// this becomes a mechanism for holding a run to a bad idea.
pub fn anchor_notice(steps: &[String]) -> String {
    let mut notice = String::from("Reminder — the plan you stated for this task:\n");
    for (index, step) in steps.iter().enumerate() {
        notice.push_str(&format!("{}. {}\n", index + 1, step));
    }
    notice.push_str(
        "\nContinue from where that plan stands. If it is no longer the right \
         plan, say so and state the new one before acting on it.",
    );
    notice
}

/// Trim a captured plan to something worth re-sending every few turns.
///
/// Steps beyond the cap are dropped and the remainder is truncated to the byte
/// budget. Truncation is per step and marked, because a step cut off mid-word
/// with no sign of it reads as a plan the model never wrote.
fn bounded_plan(steps: Vec<String>) -> Vec<String> {
    let mut bounded = Vec::new();
    let mut budget = MAX_PLAN_BYTES;
    for step in steps.into_iter().take(MAX_PLAN_STEPS) {
        if budget == 0 {
            break;
        }
        if step.len() <= budget {
            budget -= step.len();
            bounded.push(step);
            continue;
        }
        let mut end = budget;
        while end > 0 && !step.is_char_boundary(end) {
            end -= 1;
        }
        if end > 0 {
            bounded.push(format!("{}…", &step[..end]));
        }
        budget = 0;
    }
    bounded
}

/// Pull an ordered list of steps out of free model text.
///
/// Takes the longest run of *consecutive* list lines rather than every list
/// line in the reply. A model that writes a plan and then discusses it produces
/// several unrelated lists, and concatenating them yields a plan it never
/// stated — the run of adjacent lines is the one it wrote as a unit.
pub fn parse_plan_steps(text: &str) -> Vec<String> {
    let mut best: Vec<String> = Vec::new();
    let mut current: Vec<String> = Vec::new();

    for line in text.lines() {
        match list_item_body(line.trim()) {
            Some(body) if !body.is_empty() => current.push(body),
            // A blank line inside a list is formatting, not a break: models
            // routinely double-space numbered steps.
            //
            // An *empty item* -- "2." with nothing after it -- is the same
            // thing and used to end the run, so "1. foo / 2. / 3. baz" captured
            // only the first step and silently discarded the third. A
            // placeholder step is a plan with a gap in it, not the end of a
            // plan.
            Some(_) => {}
            None if line.trim().is_empty() && !current.is_empty() => {}
            _ => {
                if current.len() > best.len() {
                    best = std::mem::take(&mut current);
                } else {
                    current.clear();
                }
            }
        }
    }
    if current.len() > best.len() {
        best = current;
    }
    best
}

/// The text of a list item, or `None` when the line is not one.
///
/// Accepts the markers models actually emit: `1.`, `1)`, `-`, `*`, `•`. A bare
/// number with no delimiter is rejected deliberately — "2024 was the release
/// year" is not step 2024.
fn list_item_body(line: &str) -> Option<String> {
    if let Some(rest) = line
        .strip_prefix("- ")
        .or_else(|| line.strip_prefix("* "))
        .or_else(|| line.strip_prefix("• "))
    {
        return Some(rest.trim().to_string());
    }
    let digits: String = line.chars().take_while(char::is_ascii_digit).collect();
    if digits.is_empty() || digits.len() > 3 {
        return None;
    }
    let rest = &line[digits.len()..];
    let rest = rest.strip_prefix('.').or_else(|| rest.strip_prefix(')'))?;
    Some(rest.trim().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    const PLAN: &str = "I'll do this in three steps:\n\
        1. Read the proposal ledger\n\
        2. Add the missing field\n\
        3. Update the tests\n\
        Starting now.";

    #[test]
    fn a_numbered_plan_is_captured() {
        let steps = parse_plan_steps(PLAN);
        assert_eq!(
            steps,
            vec![
                "Read the proposal ledger",
                "Add the missing field",
                "Update the tests"
            ]
        );
    }

    #[test]
    fn bullet_markers_are_accepted() {
        let steps = parse_plan_steps("- first thing\n- second thing\n");
        assert_eq!(steps, vec!["first thing", "second thing"]);
    }

    /// Blank lines inside a list do not end it.
    #[test]
    fn a_double_spaced_list_is_still_one_plan() {
        let steps = parse_plan_steps("1. first\n\n2. second\n\n3. third\n");
        assert_eq!(steps.len(), 3);
    }

    /// An empty step is a gap in a plan, not the end of one.
    #[test]
    fn an_empty_item_does_not_truncate_the_plan() {
        assert_eq!(
            parse_plan_steps("1. foo\n2. \n3. baz\n"),
            vec!["foo", "baz"],
            "the placeholder is dropped; the steps around it are not"
        );
    }

    /// A plan too long to restate is not kept whole.
    ///
    /// It is re-sent every few turns, so its size is multiplied by the run --
    /// and the small-context providers this exists to help are the ones that
    /// start refusing requests because of it.
    #[test]
    fn an_oversized_plan_is_bounded() {
        let long: String = (1..=40)
            .map(|index| format!("{index}. {}\n", "step text ".repeat(20)))
            .collect();
        let mut anchor = PlanAnchor::new(true);
        assert!(anchor.capture(&long));

        assert!(anchor.steps().len() <= MAX_PLAN_STEPS);
        let total: usize = anchor.steps().iter().map(String::len).sum();
        assert!(
            total <= MAX_PLAN_BYTES + 8,
            "a reminder re-sent every few turns has to stay small; got {total} bytes"
        );
    }

    /// The longest adjacent run wins, not every list line in the reply.
    ///
    /// A model that states a plan and then discusses it writes several lists.
    /// Concatenating them produces a plan it never stated.
    #[test]
    fn a_later_unrelated_list_does_not_join_the_plan() {
        let text = "1. read\n2. edit\n3. verify\n\nNotes on risk:\n- could conflict\n";
        assert_eq!(parse_plan_steps(text), vec!["read", "edit", "verify"]);
    }

    /// A bare number is not a step.
    #[test]
    fn a_number_without_a_delimiter_is_not_a_step() {
        assert!(parse_plan_steps("2024 was the release year\n2025 is next\n").is_empty());
    }

    /// One item is not a plan.
    #[test]
    fn a_single_bullet_is_not_captured() {
        let mut anchor = PlanAnchor::new(true);
        assert!(!anchor.capture("- just do the thing"));
        assert!(!anchor.has_plan());
    }

    #[test]
    fn prose_with_no_list_captures_nothing() {
        let mut anchor = PlanAnchor::new(true);
        assert!(!anchor.capture("I'll look at the ledger and then fix the field."));
        assert!(!anchor.has_plan());
    }

    /// The first plan is kept; a restatement does not replace it.
    ///
    /// A model that rewrites its plan mid-run is often already drifting, and
    /// adopting the new version would make this agree with the drift it exists
    /// to catch.
    #[test]
    fn a_second_plan_does_not_replace_the_first() {
        let mut anchor = PlanAnchor::new(true);
        assert!(anchor.capture(PLAN));
        assert!(!anchor.capture("1. abandon everything\n2. start over\n"));
        assert_eq!(anchor.steps()[0], "Read the proposal ledger");
    }

    #[test]
    fn the_plan_is_restated_on_the_fourth_turn() {
        let mut anchor = PlanAnchor::new(true);
        anchor.capture(PLAN);

        for turn in 1..DEFAULT_REANCHOR_EVERY {
            assert!(
                anchor.reanchor().is_none(),
                "turn {turn} is too early to interrupt with a reminder"
            );
        }
        let notice = anchor.reanchor().expect("the plan is due to be restated");
        assert!(notice.contains("Read the proposal ledger"));
        assert!(notice.contains("Update the tests"));
    }

    /// The counter restarts, so the reminder is periodic rather than constant.
    #[test]
    fn restating_the_plan_restarts_the_interval() {
        let mut anchor = PlanAnchor::new(true);
        anchor.capture(PLAN);
        for _ in 0..DEFAULT_REANCHOR_EVERY {
            let _ = anchor.reanchor();
        }

        assert!(anchor.reanchor().is_none(), "the interval starts over");
    }

    /// A run with no stated plan is never interrupted.
    #[test]
    fn nothing_is_restated_when_no_plan_was_stated() {
        let mut anchor = PlanAnchor::new(true);
        for _ in 0..(DEFAULT_REANCHOR_EVERY * 3) {
            assert!(anchor.reanchor().is_none());
        }
    }

    /// The reminder leaves room to abandon a plan that turned out wrong.
    ///
    /// Without that line this stops being an anchor and becomes a mechanism for
    /// holding a run to a bad idea.
    #[test]
    fn the_reminder_permits_replacing_the_plan() {
        let notice = anchor_notice(&["read".to_string(), "edit".to_string()]);
        assert!(notice.contains("no longer the right"));
    }

    /// Disabled governors capture nothing and restate nothing.
    #[test]
    fn a_disabled_anchor_does_nothing() {
        let mut anchor = PlanAnchor::new(false);
        assert!(!anchor.capture(PLAN));
        assert!(anchor.reanchor().is_none());
        assert!(!anchor.has_plan());
    }
}
