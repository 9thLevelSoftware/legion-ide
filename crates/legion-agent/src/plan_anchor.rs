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
    /// Index of the line the run starts on.
    first_line: usize,
    /// Index of the first line that could introduce it.
    ///
    /// The line after the previous run ended, so a cue can only introduce the
    /// list it sits above. Searching the whole reply let one "My plan:" at the
    /// top introduce a findings list four paragraphs down.
    search_from: usize,
}

impl ListRun {
    fn new(marker: ListMarker, first_line: usize, search_from: usize) -> Self {
        Self {
            items: Vec::new(),
            marker,
            first_line,
            search_from,
        }
    }
}

/// Every run of adjacent list lines in the reply, in order.
///
/// A run ends at a non-list line, or at a change of marker: `1.` `2.` followed
/// by `-` `-` is two lists, and a blank line between them does not make it one.
fn list_runs(text: &str) -> Vec<ListRun> {
    let mut runs: Vec<ListRun> = Vec::new();
    let mut current: Option<ListRun> = None;
    let mut previous_run_end: Option<usize> = None;
    let mut open_fence: Option<FenceMarker> = None;

    /// Close the run in progress, remembering where it ended.
    fn close(
        current: &mut Option<ListRun>,
        runs: &mut Vec<ListRun>,
        previous_run_end: &mut Option<usize>,
        last: usize,
    ) {
        if let Some(run) = current.take() {
            runs.push(run);
            *previous_run_end = Some(last);
        }
    }

    for (index, line) in text.lines().enumerate() {
        // A fence is a quotation, and a quotation is not a statement.
        //
        // A model pasting a README with "1. Install" and "2. Configure" was
        // handing over an ordered list, which needs no planning cue -- so
        // somebody else's install instructions became the plan this run was
        // held to, permanently, and came back every fourth turn as "the plan
        // you stated".
        if let Some(marker) = fence_marker(line.trim()) {
            match open_fence {
                Some(open) if closes(open, marker) => open_fence = None,
                Some(_) => continue,
                None => open_fence = Some(marker),
            }
            close(
                &mut current,
                &mut runs,
                &mut previous_run_end,
                index.saturating_sub(1),
            );
            continue;
        }
        if open_fence.is_some() {
            continue;
        }
        match list_item_body(line.trim()) {
            Some((marker, body)) if !body.is_empty() => {
                if current.as_ref().is_some_and(|run| run.marker != marker) {
                    close(
                        &mut current,
                        &mut runs,
                        &mut previous_run_end,
                        index.saturating_sub(1),
                    );
                }
                current
                    .get_or_insert_with(|| {
                        ListRun::new(marker, index, previous_run_end.map_or(0, |end| end + 1))
                    })
                    .items
                    .push(body);
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
            None if line.trim().is_empty() && current.is_some() => {}
            _ => close(
                &mut current,
                &mut runs,
                &mut previous_run_end,
                index.saturating_sub(1),
            ),
        }
    }
    close(
        &mut current,
        &mut runs,
        &mut previous_run_end,
        text.lines().count(),
    );
    runs
}

/// Whether a line is a Markdown section heading.
///
/// One to six `#` followed by a space or nothing, per CommonMark -- `#import`
/// and `#!/bin/sh` are not headings, and treating them as boundaries would stop
/// the search on a line of quoted code.
///
/// A heading claims what follows it exactly as a colon-terminated line does,
/// and only the colon form was recognised: "I will summarize the inspection."
/// under a `## Findings` heading handed the findings the earlier cue.
fn is_heading(line: &str) -> bool {
    let hashes = line.chars().take_while(|found| *found == '#').count();
    (1..=6).contains(&hashes)
        && line[hashes..]
            .chars()
            .next()
            .is_none_or(char::is_whitespace)
}

/// A fence delimiter: which character, and how many of it.
///
/// Both are needed to close one. A boolean toggled by any fence let a `~~~`
/// line *inside* a backtick-fenced quotation close the outer fence, and a
/// shorter backtick run inside a longer one do the same -- after which the rest
/// of the quotation was read as the model's own words, and its numbered lines
/// became the plan the run was held to.
#[derive(Clone, Copy, PartialEq, Eq)]
struct FenceMarker {
    character: char,
    length: usize,
    /// Whether anything follows the delimiter.
    ///
    /// An opening fence may carry an info string -- ```` ```yaml ```` -- and a
    /// closing one may be followed only by whitespace. Ignoring the suffix let
    /// a `~~~~yaml` line *inside* a four-tilde fence close it, and the rest of
    /// the quotation was then read as the model's own words.
    has_info: bool,
}

/// The fence a line opens or closes, if it is one at all.
///
/// Three or more of the same character, per CommonMark. The length is returned
/// rather than compared here because a closing fence has to be at least as long
/// as the one it closes, which only the caller holding the open fence knows.
fn fence_marker(line: &str) -> Option<FenceMarker> {
    ['`', '~'].into_iter().find_map(|character| {
        let length = line.chars().take_while(|found| *found == character).count();
        (length >= 3).then(|| FenceMarker {
            character,
            length,
            has_info: !line[length..].trim().is_empty(),
        })
    })
}

/// Whether `closing` may close `open`.
///
/// Same character, at least as long, and carrying nothing after it. A shorter
/// run of the same character is content inside the fence, which is exactly how
/// a nested example is written; so is a longer run with an info string after it.
fn closes(open: FenceMarker, closing: FenceMarker) -> bool {
    open.character == closing.character && closing.length >= open.length && !closing.has_info
}

/// Pull an ordered list of steps out of free model text.
///
/// Two rules, in order.
///
/// A run the surrounding text introduces as a plan is the plan, whatever its
/// markers -- that is the model saying so, and nothing beats it. Failing that,
/// the longest *numbered* run is taken: numbering is an ordering the model
/// wrote down, and this module's argument for two items being enough is that
/// "two is an ordering", which is true of `1.` `2.` and simply not true of `-`
/// `-`. A bullet run with nothing introducing it is a set, and a set of
/// findings is shaped exactly like a set of steps.
///
/// Length decides only between runs of the same standing. Preferring the
/// longest outright is what let a three-bullet findings list displace the
/// two-step plan stated above it.
pub fn parse_plan_steps(text: &str) -> Vec<String> {
    // A run too short to be a plan does not get to be one, and does not get to
    // beat a run that is. "Plan:\n- inspect\nExecution:\n1. read\n2. edit"
    // returned the single introduced bullet, which `capture` then rejected for
    // being under `MIN_PLAN_STEPS` -- and the two-step plan below it was never
    // considered, so the run went unanchored.
    let runs: Vec<ListRun> = list_runs(text)
        .into_iter()
        .filter(|run| run.items.len() >= MIN_PLAN_STEPS)
        .collect();
    if let Some(introduced) = runs
        .iter()
        .filter(|run| introduced_as_a_plan(text, run.search_from, run.first_line))
        .max_by_key(|run| run.items.len())
    {
        return introduced.items.clone();
    }
    runs.iter()
        .filter(|run| run.marker == ListMarker::Ordered)
        .max_by_key(|run| run.items.len())
        .map(|run| run.items.clone())
        .unwrap_or_default()
}

/// Whether the text between `search_from` and `first_line` announces a plan.
///
/// A window rather than everything above, because a cue introduces the list it
/// sits over and not every list after it. "My plan:" followed by two numbered
/// steps, and then "Findings:" followed by three bullets, used to hand the
/// findings the earlier cue -- and since the anchor keeps its first capture for
/// the life of the run, the plan the model actually stated was gone for good.
///
/// Read backwards, and stopped by a heading that is not itself a cue.
///
/// A line ending in a colon claims the list beneath it. "I will summarize the
/// inspection." followed by "Findings:" and two bullets was reading past the
/// heading, finding "i will" above it, and locking the findings in as the plan
/// -- the same defect as the wide window, one section further along.
///
/// Still a window rather than a single line: models write "I'll do this:", then
/// prose, then the list, and a one-line lookback would miss it. The cue is
/// checked before the heading rule, so "My plan:" introduces its own list.
///
/// The trade is deliberate and asymmetric. A plan under an unrelated heading
/// now goes uncaptured, which costs the run its anchor -- the state it was in
/// before this feature existed. A findings list captured as a plan costs the
/// run a permanent, actively misleading anchor it cannot shed.
fn introduced_as_a_plan(text: &str, search_from: usize, first_line: usize) -> bool {
    // Fenced lines are marked on a forward pass, because fence state only reads
    // forwards. Without it a YAML key inside a quoted block -- `config:`,
    // `password:` -- stopped the search as though it were a heading, and
    // discarded a plan whose cue sat above the fence. The same defect as the
    // wide window, in the other direction: reading a boundary in one context as
    // a boundary in another.
    let lines: Vec<&str> = text.lines().take(first_line).collect();
    let mut fenced = vec![false; lines.len()];
    let mut open_fence: Option<FenceMarker> = None;
    for (index, line) in lines.iter().enumerate() {
        match (open_fence, fence_marker(line.trim())) {
            (Some(open), Some(marker)) if closes(open, marker) => open_fence = None,
            (None, Some(marker)) => open_fence = Some(marker),
            _ => fenced[index] = open_fence.is_some(),
        }
    }

    for index in (search_from..first_line).rev() {
        let line = lines[index].trim();
        if line.is_empty() || fenced[index] {
            continue;
        }
        if line_states_a_plan(line) {
            return true;
        }
        if line.ends_with(':') || is_heading(line) {
            return false;
        }
    }
    false
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
    // A delimiter has to be followed by space, or by nothing.
    //
    // `1.0` and `2.0` are a version number, and stripping the digit and the dot
    // left `0` and `0` -- two ordered "steps", which need no planning cue, so a
    // changelog line became the plan the run was held to for the rest of its
    // life. Nothing follows the delimiter in `2.`, and that stays a step: an
    // empty item is a plan with a gap in it, which the run parser already
    // tolerates deliberately.
    if !rest.is_empty() && !rest.starts_with([' ', '\t']) {
        return None;
    }
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

    /// A version number is not a numbered list.
    ///
    /// `1.0` and `2.0` stripped down to `0` and `0` -- two ordered steps, which
    /// need no planning cue -- so a changelog or a dependency list became the
    /// plan the run was held to, and blocked the real one from ever being
    /// captured.
    #[test]
    fn decimal_values_are_not_read_as_steps() {
        assert!(
            parse_plan_steps("Versions in play:\n1.0\n2.0\n").is_empty(),
            "a version number is not a step"
        );
        assert!(
            parse_plan_steps("serde 1.0.204\ntokio 1.40.0\n").is_empty(),
            "and neither is a dependency line"
        );
    }

    /// A step with nothing after its number is still a step.
    ///
    /// The empty item is deliberately tolerated -- "1. foo / 2. / 3. baz" is a
    /// plan with a gap in it, not the end of a plan -- so the whitespace rule
    /// must not take it out.
    #[test]
    fn an_empty_numbered_item_survives_the_whitespace_rule() {
        assert_eq!(
            parse_plan_steps("1. read\n2.\n3. verify\n"),
            vec!["read", "verify"]
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

    /// A cue introduces the list it sits over, not every list after it.
    ///
    /// "My plan:" with two numbered steps, then "Findings:" with three bullets:
    /// searching every line above the bullets found the earlier cue, and the
    /// longer list won on length. The anchor keeps its first capture for the
    /// life of the run, so the plan the model actually stated was gone for good
    /// and the findings came back every fourth turn as "the plan you stated".
    #[test]
    fn a_later_findings_list_does_not_borrow_an_earlier_plan_cue() {
        let text = "My plan:
             1. update the field
             2. run the tests
             
             Findings:
             - stale cache
             - missing guard
             - unused import
";

        assert_eq!(
            parse_plan_steps(text),
            vec!["update the field", "run the tests"],
            "the cue belongs to the list beneath it"
        );
    }

    /// Length decides between runs of the same standing, and only then.
    ///
    /// A longer bullet list introduced as a plan still beats a shorter numbered
    /// one that is not -- the model saying "plan" outranks the marker.
    #[test]
    fn an_introduced_bullet_list_beats_an_unintroduced_numbered_one() {
        let text = "Versions in play:
             1. serde
             2. tokio
             
             My plan:
             - read the ledger
             - add the field
             - run the tests
";

        assert_eq!(
            parse_plan_steps(text),
            vec!["read the ledger", "add the field", "run the tests"]
        );
    }

    /// A change of marker ends a run, blank line or not.
    #[test]
    fn a_marker_change_starts_a_new_list() {
        let text = "1. read
2. edit
- a note
- another
";

        assert_eq!(
            parse_plan_steps(text),
            vec!["read", "edit"],
            "the numbered run is a plan; the bullets after it are not the same list"
        );
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
        let text = "Here is my plan.\n\n- read the ledger\n- add the field\n";
        assert_eq!(
            parse_plan_steps(text),
            vec!["read the ledger", "add the field"]
        );
    }

    /// A heading claims the list under it, and stops the search.
    ///
    /// "I will summarize the inspection." followed by "Findings:" and two
    /// bullets read past the heading, found "i will" above it, and locked the
    /// findings in as the plan -- permanently, since the anchor keeps its first
    /// capture, and visibly, since it comes back every fourth turn wearing the
    /// name of a plan the model never stated.
    #[test]
    fn a_heading_stops_the_cue_search() {
        let text = "I will summarize the inspection.\nFindings:\n- stale cache\n- missing guard\n";

        assert!(
            parse_plan_steps(text).is_empty(),
            "the findings are introduced by \"Findings:\", not by the sentence above it"
        );
    }

    /// A Markdown heading stops the search as a colon heading does.
    ///
    /// Only the colon form was recognised, so "I will summarize the
    /// inspection." under a `## Findings` heading handed the findings the
    /// earlier cue -- and the first capture is permanent.
    #[test]
    fn a_markdown_heading_stops_the_cue_search() {
        let text =
            "I will summarize the inspection.\n## Findings\n- stale cache\n- missing guard\n";

        assert!(
            parse_plan_steps(text).is_empty(),
            "the findings are introduced by their own heading"
        );
    }

    /// A Markdown heading that states a plan still introduces its list.
    #[test]
    fn a_markdown_heading_that_states_a_plan_still_counts() {
        let text = "Some notes.\n## My plan\n- read the ledger\n- add the field\n";

        assert_eq!(
            parse_plan_steps(text),
            vec!["read the ledger", "add the field"]
        );
    }

    /// A hash that is not a heading does not stop the search.
    #[test]
    fn a_shebang_is_not_a_heading() {
        let text = "My plan:\n#!/bin/sh\n- read the ledger\n- add the field\n";

        assert_eq!(
            parse_plan_steps(text),
            vec!["read the ledger", "add the field"]
        );
    }

    /// A fence line carrying an info string does not close anything.
    ///
    /// An opening fence may name a language; a closing one may be followed only
    /// by whitespace. Ignoring the suffix let `~~~~yaml` inside a four-tilde
    /// fence close it, and the quotation below was read as the model's words.
    #[test]
    fn a_fence_with_an_info_string_does_not_close_one() {
        let text = "The README says:\n\
             ~~~~\n\
             ~~~~yaml\n\
             1. Install the toolchain\n\
             2. Configure the endpoint\n\
             ~~~~\n";

        assert!(
            parse_plan_steps(text).is_empty(),
            "a delimiter with a language after it opens, it does not close"
        );
    }

    /// A heading that is itself a cue still introduces its list.
    #[test]
    fn a_heading_that_states_a_plan_still_counts() {
        let text =
            "I will summarize the inspection.\nMy plan:\n- read the ledger\n- add the field\n";

        assert_eq!(
            parse_plan_steps(text),
            vec!["read the ledger", "add the field"]
        );
    }

    /// A list quoted inside a fence is not a plan the model stated.
    ///
    /// A README excerpt with "1. Install" and "2. Configure" is an ordered
    /// list, which needs no planning cue -- so somebody else's instructions
    /// became the plan the run was held to, and blocked the real one.
    #[test]
    fn a_fenced_list_is_not_captured_as_a_plan() {
        let text = "Here is what the README says:\n\
             ```\n\
             1. Install the toolchain\n\
             2. Configure the endpoint\n\
             ```\n";

        assert!(
            parse_plan_steps(text).is_empty(),
            "quoting a list is not stating one"
        );
    }

    /// A fence closes only with its own delimiter.
    ///
    /// A boolean toggled by any fence let a `~~~` line inside a backtick-fenced
    /// quotation close the outer fence, after which the rest of the quotation
    /// was read as the model's own words and its numbered lines became the plan.
    #[test]
    fn a_foreign_delimiter_does_not_close_a_fence() {
        let text = "The README says:\n\
             ```\n\
             ~~~\n\
             1. Install the toolchain\n\
             2. Configure the endpoint\n\
             ```\n";

        assert!(
            parse_plan_steps(text).is_empty(),
            "a tilde line inside a backtick fence is content, not the close"
        );
    }

    /// A shorter run of the same character is content too.
    #[test]
    fn a_shorter_fence_run_does_not_close_a_longer_one() {
        let text = "The README says:\n\
             ````\n\
             ```\n\
             1. Install the toolchain\n\
             2. Configure the endpoint\n\
             ````\n";

        assert!(
            parse_plan_steps(text).is_empty(),
            "nested example, not a close"
        );
    }

    /// A key inside a fenced block is not a heading.
    ///
    /// The heading-stop reads a trailing colon as a section boundary, and a
    /// YAML block full of them sat between a cue and its list -- so a plan
    /// introduced above a quoted config was discarded. Reading a boundary in
    /// one context as a boundary in another, which is the defect the cue window
    /// was narrowed for, facing the other way.
    #[test]
    fn a_colon_inside_a_fence_does_not_stop_the_cue_search() {
        let text = "I'll outline the setup:\n\
             ```yaml\n\
             config:\n\
               - val\n\
             ```\n\
             - step one\n\
             - step two\n";

        // Bullets deliberately. A numbered run is rescued by the ordered
        // fallback whether or not the cue search ever reaches its cue, so with
        // numbers this test passes with the fence mask removed and asserts
        // nothing about the thing it is named for.
        assert_eq!(parse_plan_steps(text), vec!["step one", "step two"]);
    }

    /// A one-item list does not beat the plan below it.
    ///
    /// The introduced single bullet won on precedence, `capture` then rejected
    /// it for being under `MIN_PLAN_STEPS`, and the two-step plan beneath was
    /// never considered -- so a reply containing a perfectly good plan left the
    /// run unanchored.
    #[test]
    fn an_undersized_introduced_list_does_not_block_a_real_plan() {
        let text = "Plan:\n- inspect\nExecution:\n1. read\n2. edit\n";

        assert_eq!(parse_plan_steps(text), vec!["read", "edit"]);
    }

    /// A plan outside the fence survives a fence in the same reply.
    #[test]
    fn a_fence_does_not_hide_the_plan_beside_it() {
        let text = "The README says:\n\
             ```\n\
             1. Install the toolchain\n\
             2. Configure the endpoint\n\
             ```\n\
             My plan:\n\
             1. read the ledger\n\
             2. add the field\n";

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
