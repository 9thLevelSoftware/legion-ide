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
    /// Steps the model stated that the caps dropped entirely.
    ///
    /// Held so the reminder can say they exist. Without it the notice presents
    /// the first twelve steps as "the plan you stated", and a model reading its
    /// own plan back with the tail missing has been told the work ends at step
    /// twelve.
    omitted_steps: usize,
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
            omitted_steps: 0,
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
        let parsed = parse_plan_steps(text);
        let stated = parsed.len();
        let steps = bounded_plan(parsed);
        if steps.len() < MIN_PLAN_STEPS {
            return false;
        }
        self.omitted_steps = stated - steps.len();
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
        Some(anchor_notice(&self.steps, self.omitted_steps))
    }

    /// The captured plan, empty when none was stated.
    pub fn steps(&self) -> &[String] {
        &self.steps
    }

    /// Whether a plan is being held.
    pub fn has_plan(&self) -> bool {
        !self.steps.is_empty()
    }

    /// How many stated steps the caps dropped from the held plan.
    pub fn omitted_steps(&self) -> usize {
        self.omitted_steps
    }
}

/// The reminder text put back in front of the model.
///
/// Phrased as a reminder of what it said, not an instruction from the harness:
/// the model is more likely to reconcile with its own stated plan than to obey
/// a system voice it has been ignoring for ten turns. The last line matters as
/// much as the list — a plan that turned out to be wrong has to be sayable, or
/// this becomes a mechanism for holding a run to a bad idea.
pub fn anchor_notice(steps: &[String], omitted: usize) -> String {
    let mut notice = String::from("Reminder — the plan you stated for this task:\n");
    for (index, step) in steps.iter().enumerate() {
        notice.push_str(&format!("{}. {}\n", index + 1, step));
    }
    // Said out loud, because the alternative is a lie the model acts on.
    //
    // The caps exist so a reminder re-sent every few turns stays small, and a
    // plan longer than they allow gets its tail dropped. Presenting what
    // survived as "the plan you stated" tells a model on a long migration that
    // the work finishes at step twelve, and it will finish there.
    if omitted > 0 {
        notice.push_str(&format!(
            "…and {omitted} further step{} you stated, too long to repeat here. \
             They are still yours to finish.\n",
            if omitted == 1 { "" } else { "s" }
        ));
    }
    notice.push_str(
        "\nContinue from where that plan stands. If it is no longer the right \
         plan, say so and state the new one before acting on it.",
    );
    notice
}

/// Smallest slice of the byte budget a step is guaranteed.
///
/// The budget used to be first-come: a step long enough to spend all of it
/// returned a one-element plan, which [`PlanAnchor::capture`] then rejected for
/// having fewer than [`MIN_PLAN_STEPS`] steps. An otherwise good plan whose
/// opening step ran long got no anchoring at all -- and a verbose opening step
/// is exactly what a small model writes when it is about to need anchoring.
///
/// Forty bytes is a short sentence. Enough that a reserved step still says
/// something; small enough that reserving one for each of the twelve costs less
/// than half the budget.
pub const MIN_STEP_BYTES: usize = 40;

/// Trim a captured plan to something worth re-sending every few turns.
///
/// Steps beyond the cap are dropped and the remainder is truncated to the byte
/// budget. Truncation is per step and marked, because a step cut off mid-word
/// with no sign of it reads as a plan the model never wrote.
///
/// Each step spends only what it can without starving the ones behind it, so
/// the shape of the plan survives a long step near the front. The steps this
/// drops entirely are counted by the caller and named in the reminder.
fn bounded_plan(steps: Vec<String>) -> Vec<String> {
    let kept = steps.len().min(MAX_PLAN_STEPS);
    let mut bounded = Vec::new();
    let mut budget = MAX_PLAN_BYTES;
    for (index, step) in steps.into_iter().take(kept).enumerate() {
        // What the steps after this one are owed before this one may spend.
        let reserved = (kept - index - 1) * MIN_STEP_BYTES;
        let allowance = budget.saturating_sub(reserved);
        if allowance == 0 {
            break;
        }
        if step.len() <= allowance {
            budget -= step.len();
            bounded.push(step);
            continue;
        }
        // The marker is part of what gets sent, so it is part of what gets
        // counted. Charging only the text meant every truncated step overran
        // the budget by three bytes, and twelve of them overran it by
        // thirty-six -- a small lie about a number that exists to be exact.
        let marker = '…'.len_utf8();
        let mut end = allowance.saturating_sub(marker);
        while end > 0 && !step.is_char_boundary(end) {
            end -= 1;
        }
        if end == 0 {
            break;
        }
        bounded.push(format!("{}…", &step[..end]));
        budget -= end + marker;
    }
    bounded
}

/// Words in the text above a list that say the list is a plan.
///
/// Only consulted for bullet lists. A numbered list is an ordering the model
/// wrote down; a bullet list is a set, and a set of findings looks exactly like
/// a set of steps. "Found:\n- stale cache\n- missing guard" was being captured
/// and then restated every fourth turn as "the plan you stated", steering the
/// run toward incidental findings and locking out the real plan when it arrived.
/// Matched on whole words, not as substrings: "explanation" contains "plan",
/// and "Here is an explanation of the findings:" was introducing a bullet list
/// of findings as though the model had called it a plan. Plurals are listed
/// rather than derived, because a suffix rule is the same substring bug with
/// more steps.
const PLAN_CUES: &[&str] = &[
    "plan", "plans", "planning", "step", "steps", "approach", "i will", "i'll", "going to",
    "intend", "intends", "strategy",
];

/// One run of adjacent list lines, and how it marked itself.
struct ListRun {
    items: Vec<String>,
    marker: ListMarker,
    /// Index of the line the run starts on, for reading the text above it.
    first_line: usize,
}

impl ListRun {
    fn empty() -> Self {
        Self {
            items: Vec::new(),
            marker: ListMarker::Bulleted,
            first_line: 0,
        }
    }
}

/// Pull an ordered list of steps out of free model text.
///
/// Takes the longest run of *consecutive* list lines rather than every list
/// line in the reply. A model that writes a plan and then discusses it produces
/// several unrelated lists, and concatenating them yields a plan it never
/// stated — the run of adjacent lines is the one it wrote as a unit.
///
/// A bullet run additionally has to be introduced as a plan. The module's own
/// argument for two items being enough is that "two is an ordering" — which is
/// true of `1.` `2.` and simply not true of `-` `-`. A bullet list is a set,
/// and a set of findings is shaped exactly like a set of steps, so the
/// surrounding text has to say which one it is.
pub fn parse_plan_steps(text: &str) -> Vec<String> {
    let mut best = ListRun::empty();
    // The longest numbered run, kept separately so a bullet run that ties with
    // it and then fails the cue check does not take it down as well. A reply
    // opening with two bullets and following them with "Plan to fix:" and two
    // numbered steps used to return nothing at all.
    let mut best_ordered = ListRun::empty();
    let mut current = ListRun::empty();

    for (index, line) in text.lines().enumerate() {
        match list_item_body(line.trim()) {
            Some((marker, body)) if !body.is_empty() => {
                if current.items.is_empty() {
                    current.marker = marker;
                    current.first_line = index;
                }
                current.items.push(body);
            }
            // A blank line inside a list is formatting, not a break: models
            // routinely double-space numbered steps.
            //
            // An *empty item* -- "2." with nothing after it -- is the same
            // thing and used to end the run, so "1. foo / 2. / 3. baz" captured
            // only the first step and silently discarded the third. A
            // placeholder step is a plan with a gap in it, not the end of a
            // plan.
            Some(_) => {}
            None if line.trim().is_empty() && !current.items.is_empty() => {}
            _ => finish_run(&mut current, &mut best, &mut best_ordered),
        }
    }
    finish_run(&mut current, &mut best, &mut best_ordered);

    if best.marker == ListMarker::Bulleted && !introduced_as_a_plan(text, best.first_line) {
        // The bullets were not a plan. A numbered run elsewhere in the reply
        // still might be, and discarding it because a longer list of findings
        // came first would leave the run with no anchor at all.
        return best_ordered.items;
    }
    best.items
}

/// Close the run in progress, updating both the overall and the ordered best.
fn finish_run(current: &mut ListRun, best: &mut ListRun, best_ordered: &mut ListRun) {
    if current.marker == ListMarker::Ordered && current.items.len() > best_ordered.items.len() {
        best_ordered.items = current.items.clone();
        best_ordered.marker = ListMarker::Ordered;
        best_ordered.first_line = current.first_line;
    }
    if current.items.len() > best.items.len() {
        *best = std::mem::replace(current, ListRun::empty());
    } else {
        *current = ListRun::empty();
    }
}

/// Whether the text above line `first_line` announces a plan.
///
/// Everything above the list, not just the line immediately before it: models
/// write "Here is my plan." and then a blank line and then the bullets, and a
/// one-line lookback would miss it. Reading further up costs a false accept on
/// a reply that discussed a plan and then listed something else, which is the
/// cheaper mistake — the alternative is refusing to anchor a run that stated one.
fn introduced_as_a_plan(text: &str, first_line: usize) -> bool {
    text.lines()
        .take(first_line)
        .any(line_states_a_plan)
}

/// Whether one line uses a cue as a word rather than as a run of letters.
///
/// The line is reduced to lowercase words separated by single spaces and padded
/// at both ends, so a cue surrounded by spaces matches a word boundary on each
/// side. Apostrophes survive the reduction because `i'll` is one of the cues;
/// everything else non-alphanumeric becomes a separator, which is what lets
/// "My plan:" match while "explanation" does not.
fn line_states_a_plan(line: &str) -> bool {
    let mut padded = String::with_capacity(line.len() + 2);
    padded.push(' ');
    let mut pending_space = false;
    for character in line.chars() {
        if character.is_alphanumeric() || character == '\'' {
            if pending_space {
                padded.push(' ');
                pending_space = false;
            }
            padded.extend(character.to_lowercase());
        } else {
            pending_space = true;
        }
    }
    padded.push(' ');
    PLAN_CUES
        .iter()
        .any(|cue| padded.contains(&format!(" {cue} ")))
}

/// How a list line marks itself.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ListMarker {
    /// `1.` or `1)` — an ordering the model wrote down.
    Ordered,
    /// `-`, `*` or `•` — a set, which may or may not be a sequence.
    Bulleted,
}

/// The text of a list item and its marker, or `None` when the line is not one.
///
/// Accepts the markers models actually emit: `1.`, `1)`, `-`, `*`, `•`. A bare
/// number with no delimiter is rejected deliberately — "2024 was the release
/// year" is not step 2024.
///
/// The marker is returned because the two kinds carry different evidence: a
/// number is an order, a bullet is not, and only one of them is a plan on its
/// own.
fn list_item_body(line: &str) -> Option<(ListMarker, String)> {
    if let Some(rest) = line
        .strip_prefix("- ")
        .or_else(|| line.strip_prefix("* "))
        .or_else(|| line.strip_prefix("• "))
    {
        return Some((ListMarker::Bulleted, rest.trim().to_string()));
    }
    let digits: String = line.chars().take_while(char::is_ascii_digit).collect();
    if digits.is_empty() || digits.len() > 3 {
        return None;
    }
    let rest = &line[digits.len()..];
    let rest = rest.strip_prefix('.').or_else(|| rest.strip_prefix(')'))?;
    Some((ListMarker::Ordered, rest.trim().to_string()))
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
        let steps = parse_plan_steps("My plan:\n- first thing\n- second thing\n");
        assert_eq!(steps, vec!["first thing", "second thing"]);
    }

    /// A list of findings is not a plan just because it has two items.
    ///
    /// A bullet list is a set, and a set of findings is shaped exactly like a
    /// set of steps. Capturing one locked it in permanently and restated it
    /// every fourth turn as "the plan you stated" -- steering the run toward
    /// incidental findings and shutting out the real plan when it arrived.
    #[test]
    fn a_bare_list_of_findings_is_not_captured_as_a_plan() {
        assert!(
            parse_plan_steps("Found:\n- stale cache\n- missing guard\n").is_empty(),
            "nothing here says these are steps"
        );

        let mut anchor = PlanAnchor::new(true);
        assert!(!anchor.capture("Found:\n- stale cache\n- missing guard\n"));
        assert!(
            !anchor.has_plan(),
            "and the anchor stays free for a plan the model actually states"
        );
    }

    /// A cue has to be a word, not a run of letters inside one.
    ///
    /// "explanation" contains "plan", so "Here is an explanation of the
    /// findings:" introduced a bullet list of findings as though the model had
    /// called it a plan -- which is the same defect the cue check was added to
    /// fix, arriving through the check itself.
    #[test]
    fn a_cue_inside_a_longer_word_does_not_introduce_a_plan() {
        assert!(
            parse_plan_steps(
                "Here is an explanation of the findings:\n- stale cache\n- missing guard\n"
            )
            .is_empty(),
            "explanation is not a plan"
        );
        assert!(
            parse_plan_steps("The stepping stones:\n- one\n- two\n").is_empty(),
            "stepping is not step"
        );
    }

    /// The cue still matches next to punctuation, which is how models write it.
    #[test]
    fn a_cue_against_punctuation_still_counts() {
        for intro in ["Plan:", "**My plan**", "(the approach)", "I'll do this:"] {
            assert_eq!(
                parse_plan_steps(&format!("{intro}\n- read\n- edit\n")),
                vec!["read", "edit"],
                "{intro:?} introduces a plan"
            );
        }
    }

    /// A findings list does not take the plan beside it down with it.
    ///
    /// The bullet run is longer or ties, so it wins `best`, fails the cue check
    /// and returned nothing -- discarding the numbered plan the model actually
    /// stated two lines later, and leaving the run with no anchor at all.
    #[test]
    fn a_numbered_plan_survives_a_findings_list_above_it() {
        let text = "- read the ledger\n- check the field\nPlan to fix:\n1. update the field\n2. run the tests\n";

        assert_eq!(
            parse_plan_steps(text),
            vec!["update the field", "run the tests"]
        );
    }

    /// A numbered list needs no introduction, because it is already an order.
    ///
    /// The module's argument for two items being enough is that "two is an
    /// ordering". That holds for `1.` `2.` and does not hold for `-` `-`, which
    /// is the whole reason only one of them needs the surrounding text.
    #[test]
    fn a_numbered_list_stands_on_its_own() {
        assert_eq!(
            parse_plan_steps("Found:\n1. stale cache\n2. missing guard\n"),
            vec!["stale cache", "missing guard"]
        );
    }

    /// The cue may be several lines above the bullets.
    ///
    /// Models write the sentence, then a blank line, then the list. A lookback
    /// of one line would miss every one of them.
    #[test]
    fn a_cue_further_up_still_introduces_the_list() {
        let text = "Here is my plan.\n\nBefore starting:\n\n- read the ledger\n- add the field\n";
        assert_eq!(
            parse_plan_steps(text),
            vec!["read the ledger", "add the field"]
        );
    }

    /// A rejected findings list does not block a real plan on a later turn.
    #[test]
    fn a_plan_stated_after_a_findings_list_is_still_captured() {
        let mut anchor = PlanAnchor::new(true);
        assert!(!anchor.capture("Found:\n- stale cache\n- missing guard\n"));
        assert!(anchor.capture(PLAN));
        assert_eq!(anchor.steps().len(), 3);
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
        let notice = anchor_notice(&["read".to_string(), "edit".to_string()], 0);
        assert!(notice.contains("no longer the right"));
    }

    /// A plan the caps trimmed does not present itself as complete.
    ///
    /// `.take(MAX_PLAN_STEPS)` drops the tail, and the notice used to call what
    /// survived "the plan you stated". On a long migration that tells the model
    /// the work ends at step twelve -- and this exists precisely to make a model
    /// do what its plan says.
    #[test]
    fn a_trimmed_plan_says_that_steps_are_missing() {
        let long: String = (1..=20)
            .map(|index| format!("{index}. step number {index}\n"))
            .collect();
        let mut anchor = PlanAnchor::new(true);
        assert!(anchor.capture(&long));

        assert_eq!(anchor.steps().len(), MAX_PLAN_STEPS);
        assert_eq!(anchor.omitted_steps(), 20 - MAX_PLAN_STEPS);

        let notice = anchor_notice(anchor.steps(), anchor.omitted_steps());
        assert!(
            notice.contains("8 further steps"),
            "the reminder must own what it left out; got {notice}"
        );
    }

    /// A plan that fits says nothing about steps it did not drop.
    #[test]
    fn a_complete_plan_claims_nothing_was_omitted() {
        let mut anchor = PlanAnchor::new(true);
        assert!(anchor.capture(PLAN));

        assert_eq!(anchor.omitted_steps(), 0);
        let notice = anchor_notice(anchor.steps(), anchor.omitted_steps());
        assert!(!notice.contains("further step"), "got {notice}");
    }

    /// One verbose opening step does not cost the whole plan.
    ///
    /// The budget was first-come, so a step longer than `MAX_PLAN_BYTES` spent
    /// all of it and left a one-element plan -- which `capture` then rejected
    /// for having fewer than two steps. An otherwise good multi-step plan got
    /// no anchoring at all, and a long opening step is what a small model
    /// writes when it is about to need anchoring most.
    #[test]
    fn a_long_first_step_does_not_starve_the_rest_of_the_plan() {
        let text = format!(
            "1. {}\n2. then run the tests\n3. then report what changed\n",
            "consider every caller of this function in turn ".repeat(60)
        );
        let mut anchor = PlanAnchor::new(true);

        assert!(
            anchor.capture(&text),
            "a three-step plan with one long step is still a plan"
        );
        assert_eq!(anchor.steps().len(), 3);
        assert!(
            anchor.steps()[1].contains("run the tests"),
            "the steps behind the long one must survive; got {:?}",
            anchor.steps()
        );
        assert!(
            anchor.steps()[0].ends_with('…'),
            "and the long one is marked as cut; got {:?}",
            anchor.steps()[0]
        );

        let total: usize = anchor.steps().iter().map(String::len).sum();
        assert!(
            total <= MAX_PLAN_BYTES,
            "the whole point is still a small reminder; got {total} bytes"
        );
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
