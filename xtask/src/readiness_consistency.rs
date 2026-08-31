//! Cross-check the product-readiness ledger against the Kanban backlog and
//! against Appendix A of the production master plan.
//!
//! The two files are independently maintained and they drifted: the ledger's
//! PR-LANG-001 row named `P3.F1.T2` (write-side apply activation) as a
//! promotion blocker while the backlog had recorded it `done` with M9 evidence
//! since M9 — and `cargo test -p legion-app --test apply_activation` passed
//! 13/13 the whole time. Neither file's own gate could see the contradiction,
//! because each validates only itself.
//!
//! This gate reads both and fails when a task id mentioned in the ledger is
//! described in a way its backlog status contradicts.

use std::{collections::BTreeMap, fs, path::Path};

use crate::claim_audit::{LedgerRow, parse_ledger_rows};
use crate::kanban_backlog::KanbanBacklog;

/// Outer bound on the context taken either side of a task-id mention. The
/// binding constraint is usually the clause clamp in [`context_around`], not
/// this number; it exists so a separator-free line cannot pull in the whole row.
///
/// Known limit (shared with the `claim_audit` negation heuristic): this is
/// separator splitting, not sentence parsing. It does not understand negation,
/// so a sentence *reporting* that a past claim was wrong will read as making
/// that claim — write corrections so the task id sits in the resolved clause,
/// not the historical one.
const CONTEXT_CHARS: usize = 160;

/// Phrases that assert a task is *not* finished.
const OPEN_MARKERS: &[&str] = &[
    "blocker",
    "blocks",
    "remain",
    "remaining",
    "still open",
    "not yet",
    "unmet",
    "outstanding",
];

/// Phrases that assert a task *is* finished.
const DELIVERED_MARKERS: &[&str] = &["landed", "delivered", "complete", "shipped", "closed"];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConsistencyViolation {
    pub task_id: String,
    pub backlog_status: String,
    pub line_number: usize,
    pub message: String,
}

impl std::fmt::Display for ConsistencyViolation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.backlog_status == "missing" {
            write!(f, "{}:{}: {}", self.task_id, self.line_number, self.message)
        } else {
            write!(
                f,
                "{}:{}: {} (backlog says `{}`)",
                self.task_id, self.line_number, self.message, self.backlog_status
            )
        }
    }
}

/// Index every task id in the backlog to its status.
pub fn backlog_statuses(backlog: &KanbanBacklog) -> BTreeMap<String, String> {
    let mut statuses = BTreeMap::new();
    for epic in &backlog.epics {
        for feature in &epic.features {
            for task in &feature.tasks {
                if let Some(status) = task.status.as_deref() {
                    statuses.insert(task.id.clone(), status.to_string());
                }
            }
        }
    }
    statuses
}

/// One `Pn.Fm.Tk` mention located in the ledger.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Mention {
    line_number: usize,
    id: String,
    start: usize,
    end: usize,
}

/// Find every `Pn.Fm.Tk` mention in `text`.
fn task_id_mentions(text: &str) -> Vec<Mention> {
    let mut found = Vec::new();
    for (index, line) in text.lines().enumerate() {
        let bytes = line.as_bytes();
        let mut cursor = 0;
        while let Some(rel) = line[cursor..].find('P') {
            let start = cursor + rel;
            cursor = start + 1;
            // Shape check: P<digit> "." F<digits> "." T<digits><optional letter>
            let mut i = start + 1;
            if !bytes.get(i).is_some_and(u8::is_ascii_digit) {
                continue;
            }
            i += 1;
            if bytes.get(i) != Some(&b'.') || bytes.get(i + 1) != Some(&b'F') {
                continue;
            }
            i += 2;
            let feature_start = i;
            while bytes.get(i).is_some_and(u8::is_ascii_digit) {
                i += 1;
            }
            if i == feature_start || bytes.get(i) != Some(&b'.') || bytes.get(i + 1) != Some(&b'T')
            {
                continue;
            }
            i += 2;
            let task_start = i;
            while bytes.get(i).is_some_and(u8::is_ascii_digit) {
                i += 1;
            }
            if i == task_start {
                continue;
            }
            if bytes.get(i).is_some_and(u8::is_ascii_lowercase) {
                i += 1;
            }
            found.push(Mention {
                line_number: index + 1,
                id: line[start..i].to_string(),
                start,
                end: i,
            });
        }
    }
    found
}

/// Text around a mention, clamped to [`CONTEXT_CHARS`] and — more importantly —
/// to the nearest neighbouring task-id mention on the same line.
///
/// The clamp is what makes this usable on the real ledger, where one table cell
/// routinely names several tasks: without it, "P2.F3.T4 landed; P7.F2.T1
/// remains open" reads as both tasks having landed *and* both remaining open.
fn context_around(line: &str, mention: &Mention, siblings: &[&Mention]) -> String {
    let window_start = line[..mention.start]
        .char_indices()
        .rev()
        .nth(CONTEXT_CHARS.saturating_sub(1))
        .map(|(index, _)| index)
        .unwrap_or(0);
    let window_end = line[mention.end..]
        .char_indices()
        .nth(CONTEXT_CHARS)
        .map(|(index, _)| mention.end + index)
        .unwrap_or(line.len());

    let coordinated = coordinated_list_bounds(line, mention, siblings);
    let independent_start = coordinated.map_or(mention.start, |(run_start, _)| run_start);
    let independent_end = coordinated.map_or(mention.end, |(_, run_end)| run_end);
    let start = siblings
        .iter()
        .filter(|other| other.end <= mention.start)
        .filter(|other| coordinated.is_none_or(|(run_start, _)| other.start < run_start))
        .map(|other| independent_claim_start(line, other.end, independent_start))
        .max()
        .map_or(window_start, |boundary| boundary.max(window_start))
        .max(clause_start(line, mention.start))
        .max(comma_boundary_before(
            line,
            coordinated.map_or(mention.start, |(run_start, _)| run_start),
        ));
    let end = siblings
        .iter()
        .filter(|other| other.start >= mention.end)
        .filter(|other| coordinated.is_none_or(|(_, run_end)| other.end > run_end))
        .map(|other| independent_claim_end(line, independent_end, other.start))
        .min()
        .map_or(window_end, |boundary| boundary.min(window_end))
        .min(clause_end(line, mention.end))
        .min(comma_boundary_after(
            line,
            coordinated.map_or(mention.end, |(_, run_end)| run_end),
        ));

    line[start..end].to_lowercase()
}

/// Clamp an independent claim after the last comma between adjacent task IDs.
/// The text before that comma is the previous task's predicate, not context for
/// the current task (for example, `T1 landed, T2 remains open`).
fn independent_claim_start(line: &str, previous_end: usize, current_start: usize) -> usize {
    line[previous_end..current_start]
        .rfind(',')
        .map_or(previous_end, |offset| previous_end + offset + 1)
}

/// Clamp an independent claim before the first comma between adjacent task IDs.
/// Coordinated task lists bypass this boundary in [`context_around`].
fn independent_claim_end(line: &str, current_end: usize, next_start: usize) -> usize {
    line[current_end..next_start]
        .find(',')
        .map_or(next_start, |offset| current_end + offset)
}

/// Return the span of a coordinated task-id list containing `mention`.
///
/// Commas may join adjacent list items only when the complete run also has an
/// explicit `and`, `or`, or `&` connector. This preserves Oxford-comma
/// lists without conflating independent claims such as `T1 landed, T2 open`.
fn coordinated_list_bounds(
    line: &str,
    mention: &Mention,
    siblings: &[&Mention],
) -> Option<(usize, usize)> {
    let mut mentions = siblings.to_vec();
    mentions.push(mention);
    mentions.sort_by_key(|item| item.start);
    let index = mentions.iter().position(|item| *item == mention)?;

    let mut first = index;
    while first > 0 && is_list_separator(&line[mentions[first - 1].end..mentions[first].start]) {
        first -= 1;
    }
    let mut last = index;
    while last + 1 < mentions.len()
        && is_list_separator(&line[mentions[last].end..mentions[last + 1].start])
    {
        last += 1;
    }
    if first == last
        || !(first..last).any(|item| {
            is_explicit_coordinator(&line[mentions[item].end..mentions[item + 1].start])
        })
    {
        return None;
    }

    Some((mentions[first].start, mentions[last].end))
}

fn is_list_separator(between: &str) -> bool {
    let connector = between.trim();
    connector.chars().all(|ch| ch == ',') || is_explicit_coordinator(connector)
}

fn is_explicit_coordinator(between: &str) -> bool {
    let connector = between
        .trim_matches(|ch: char| ch.is_whitespace() || ch == ',')
        .to_ascii_lowercase();
    matches!(connector.as_str(), "and" | "or" | "&")
}

/// Separators that end a claim. `|` is a Markdown table cell wall; `. ` and
/// `; ` end a sentence or clause. A claim about one task does not carry across
/// any of them.
const CLAUSE_SEPARATORS: [&str; 3] = [". ", "; ", "|"];

fn clause_start(line: &str, before: usize) -> usize {
    CLAUSE_SEPARATORS
        .iter()
        .filter_map(|sep| line[..before].rfind(sep).map(|index| index + sep.len()))
        .max()
        .unwrap_or(0)
}

fn clause_end(line: &str, after: usize) -> usize {
    CLAUSE_SEPARATORS
        .iter()
        .filter_map(|sep| line[after..].find(sep).map(|index| after + index))
        .min()
        .unwrap_or(line.len())
}

/// A comma also ends a claim, but only outside a coordinated run: in
/// `T1 landed, T2 remains open` it separates two verdicts, while in
/// `T1, T2 and T3 remain outstanding` it joins list items that share one.
///
/// Callers pass the run's bounds rather than the mention's when
/// [`coordinated_list_bounds`] matched, so the commas inside a run are never
/// treated as boundaries — only the ones enclosing it.
fn comma_boundary_before(line: &str, before: usize) -> usize {
    line[..before].rfind(',').map_or(0, |index| index + 1)
}

// These conjunctions introduce an appositive predicate that belongs to the
// preceding task claim; they are not independent-claim delimiters.
const APPOSITIVE_CONTINUATION_PREFIXES: &[&str] = &[
    "which ", "that ", "who ", "whom ", "whose ", "where ", "when ",
];

fn comma_boundary_after(line: &str, after: usize) -> usize {
    let mut search_from = after;
    while let Some(offset) = line[search_from..].find(',') {
        let index = search_from + offset;
        let continuation = line[index + 1..].trim_start();
        if is_appositive_continuation(continuation) {
            // In `T1, which remains open`, the comma introduces a predicate
            // about T1 rather than a new independent task claim.  Continue
            // looking so a later comma can still delimit the full claim.
            search_from = index + 1;
            continue;
        }
        return index;
    }
    line.len()
}

fn is_appositive_continuation(text: &str) -> bool {
    let text = text.to_ascii_lowercase();
    APPOSITIVE_CONTINUATION_PREFIXES
        .iter()
        .any(|prefix| text.starts_with(prefix))
}

/// Compare the ledger text against backlog statuses.
pub fn check_consistency(
    ledger_text: &str,
    statuses: &BTreeMap<String, String>,
) -> Vec<ConsistencyViolation> {
    let lines: Vec<&str> = ledger_text.lines().collect();
    let mentions = task_id_mentions(ledger_text);
    let mut violations = Vec::new();

    for mention in &mentions {
        let Some(status) = statuses.get(&mention.id) else {
            violations.push(ConsistencyViolation {
                task_id: mention.id.clone(),
                backlog_status: "missing".to_string(),
                line_number: mention.line_number,
                message: "ledger cites a task absent from the backlog".to_string(),
            });
            continue;
        };
        let siblings: Vec<&Mention> = mentions
            .iter()
            .filter(|other| other.line_number == mention.line_number && *other != mention)
            .collect();
        let context = context_around(lines[mention.line_number - 1], mention, &siblings);

        if status == "done" && OPEN_MARKERS.iter().any(|marker| context.contains(marker)) {
            violations.push(ConsistencyViolation {
                task_id: mention.id.clone(),
                backlog_status: status.clone(),
                line_number: mention.line_number,
                message: "ledger describes this task as open or blocking".to_string(),
            });
        }

        if status == "todo"
            && DELIVERED_MARKERS
                .iter()
                .any(|marker| context.contains(marker))
        {
            violations.push(ConsistencyViolation {
                task_id: mention.id.clone(),
                backlog_status: status.clone(),
                line_number: mention.line_number,
                message: "ledger describes this task as delivered".to_string(),
            });
        }
    }

    violations
}

/// Default path for the product-readiness ledger, repo-relative.
pub const DEFAULT_LEDGER_PATH: &str = "plans/product-readiness-ledger.md";

/// Compare Appendix A "Current ledger status" cells against the live matrix.
pub fn check_appendix_against_ledger(
    appendix: &str,
    rows: &[LedgerRow],
) -> Vec<ConsistencyViolation> {
    let by_id: BTreeMap<&str, &LedgerRow> =
        rows.iter().map(|row| (row.gate_id.as_str(), row)).collect();
    let mut violations = Vec::new();
    for (line_number, gate_id, appendix_status) in appendix_a_status_cells(appendix) {
        let Some(row) = by_id.get(gate_id.as_str()) else {
            violations.push(ConsistencyViolation {
                task_id: gate_id,
                backlog_status: "missing".to_string(),
                line_number,
                message: "Appendix A cites a readiness gate absent from the ledger matrix"
                    .to_string(),
            });
            continue;
        };
        if appendix_status
            .to_ascii_lowercase()
            .contains("evals deferred")
        {
            let blob = format!("{} {}", row.status, row.evidence).to_ascii_lowercase();
            if blob.contains("hostile-eval") || blob.contains("hostile eval") {
                violations.push(ConsistencyViolation {
                    task_id: gate_id.clone(),
                    backlog_status: row.status.clone(),
                    line_number,
                    message: "Appendix A says adversarial evals are deferred, but the ledger names hostile-eval evidence".to_string(),
                });
            }
        }
        if !status_heads_compatible(&appendix_status, &row.status) {
            violations.push(ConsistencyViolation {
                task_id: gate_id,
                backlog_status: row.status.clone(),
                line_number,
                message: format!(
                    "Appendix A status `{}` does not match ledger status `{}`",
                    appendix_status, row.status
                ),
            });
        }
    }
    violations
}

fn appendix_a_status_cells(text: &str) -> Vec<(usize, String, String)> {
    let mut in_appendix = false;
    let mut rows = Vec::new();
    for (index, line) in text.lines().enumerate() {
        if line.starts_with("## Appendix A") {
            in_appendix = true;
            continue;
        }
        if in_appendix && line.starts_with("## ") {
            break;
        }
        if !in_appendix {
            continue;
        }
        let cells: Vec<&str> = line.split('|').map(str::trim).collect();
        if cells.len() < 4 {
            continue;
        }
        let gate_cell = cells[1];
        let Some(gate_id) = gate_cell.split_whitespace().next() else {
            continue;
        };
        if !gate_id.starts_with("PR-") {
            continue;
        }
        rows.push((index + 1, gate_id.to_string(), cells[2].to_string()));
    }
    rows
}

fn status_heads_compatible(appendix: &str, ledger: &str) -> bool {
    let appendix_head = status_head(appendix);
    let ledger_head = status_head(ledger);
    if appendix_head.is_empty() || ledger_head.is_empty() {
        return true;
    }
    ledger_head.starts_with(appendix_head) || appendix_head.starts_with(ledger_head)
}

fn status_head(status: &str) -> &str {
    const VOCAB: [&str; 6] = [
        "Product workflow validated",
        "Deferred with explicit cut line",
        "Substrate validated",
        "In progress",
        "Not started",
        "Blocked",
    ];
    for vocab in VOCAB {
        if status.starts_with(vocab) {
            return vocab;
        }
    }
    status.split([';', '(']).next().unwrap_or(status).trim()
}

pub fn run_verify_readiness_consistency(
    ledger_path: &Path,
    backlog_path: &Path,
) -> Result<usize, String> {
    let ledger_text = fs::read_to_string(ledger_path).map_err(|err| {
        format!(
            "unable to read readiness ledger `{}`: {err}",
            ledger_path.display()
        )
    })?;
    let backlog = KanbanBacklog::from_file(backlog_path)?;
    let statuses = backlog_statuses(&backlog);

    let mut violations = check_consistency(&ledger_text, &statuses);
    if let Some(appendix_path) = ledger_path
        .parent()
        .map(|dir| dir.join("legion-production-master-plan-v0.2.md"))
        && appendix_path.is_file()
    {
        let appendix = fs::read_to_string(&appendix_path).map_err(|err| {
            format!(
                "unable to read production master plan `{}`: {err}",
                appendix_path.display()
            )
        })?;
        match parse_ledger_rows(&ledger_text) {
            Ok(rows) => {
                violations.extend(check_appendix_against_ledger(&appendix, &rows));
            }
            Err(err) => {
                return Err(format!(
                    "unable to parse readiness matrix in `{}`: {err}",
                    ledger_path.display()
                ));
            }
        }
    }
    if violations.is_empty() {
        Ok(statuses.len())
    } else {
        let mut report = String::from("readiness ledger contradicts the kanban backlog:\n");
        for violation in &violations {
            report.push_str(&format!("  {violation}\n"));
        }
        Err(report)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn statuses(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
        pairs
            .iter()
            .map(|(id, status)| (id.to_string(), status.to_string()))
            .collect()
    }

    #[test]
    fn task_ids_are_extracted_with_line_numbers() {
        let found = task_id_mentions("intro\n| row | P3.F1.T2 and P2.F3.T4b |\n");
        let ids: Vec<&str> = found.iter().map(|m| m.id.as_str()).collect();
        assert_eq!(ids, vec!["P3.F1.T2", "P2.F3.T4b"]);
        assert!(found.iter().all(|m| m.line_number == 2));
    }

    #[test]
    fn non_task_p_words_are_not_matched() {
        assert!(task_id_mentions("Phase 3 PR-LANG-001 P2 P2.F3 plans/").is_empty());
    }

    #[test]
    fn ledger_calling_a_done_task_a_blocker_is_reported() {
        // The exact regression: PR-LANG-001 named P3.F1.T2 a promotion blocker
        // while the backlog had it done with M9 evidence.
        let ledger = "| ... | Write-side apply activation (P3.F1.T2) and 3-OS CI smoke remain the primary blockers for promotion. |";
        let violations = check_consistency(ledger, &statuses(&[("P3.F1.T2", "done")]));
        assert_eq!(violations.len(), 1);
        assert_eq!(violations[0].task_id, "P3.F1.T2");
        assert!(violations[0].message.contains("open or blocking"));
    }

    #[test]
    fn ledger_citing_a_todo_task_as_delivered_is_reported() {
        let ledger = "| ... | Test explorer substrate (P7.F2.T1) landed 2026-07-23. |";
        let violations = check_consistency(ledger, &statuses(&[("P7.F2.T1", "todo")]));
        assert_eq!(violations.len(), 1);
        assert!(violations[0].message.contains("delivered"));
    }

    #[test]
    fn ledger_agreeing_with_the_backlog_passes() {
        let ledger = "| ... | Test explorer substrate (P2.F3.T4) landed 2026-07-23. P7.F2.T1 remains open. |";
        let violations = check_consistency(
            ledger,
            &statuses(&[("P2.F3.T4", "done"), ("P7.F2.T1", "todo")]),
        );
        assert!(violations.is_empty(), "unexpected: {violations:?}");
    }

    #[test]
    fn in_progress_tasks_are_not_constrained_either_way() {
        // `in-progress` legitimately reads as both partly delivered and partly
        // open, so neither marker class is a contradiction.
        let ledger = "| P2.F5.T2 landed partially and remains a blocker. |";
        let violations = check_consistency(ledger, &statuses(&[("P2.F5.T2", "in-progress")]));
        assert!(violations.is_empty(), "unexpected: {violations:?}");
    }

    #[test]
    fn claims_do_not_leak_across_clause_separators() {
        // The real ledger packs many claims into one table cell. A verdict about
        // one task must not be read as a verdict about the next one along.
        let ledger = "| A landed for P2.F3.T4. P7.F2.T1 remains open; P9.F2.T2 is unmet. |";
        let violations = check_consistency(
            ledger,
            &statuses(&[
                ("P2.F3.T4", "done"),
                ("P7.F2.T1", "todo"),
                ("P9.F2.T2", "todo"),
            ]),
        );
        assert!(violations.is_empty(), "unexpected: {violations:?}");
    }

    #[test]
    fn coordinated_task_ids_share_a_trailing_predicate() {
        let ledger = "P1.F1.T1 and P1.F1.T2 remain outstanding.";
        let violations = check_consistency(
            ledger,
            &statuses(&[("P1.F1.T1", "done"), ("P1.F1.T2", "todo")]),
        );
        assert_eq!(violations.len(), 1);
        assert_eq!(violations[0].task_id, "P1.F1.T1");
    }

    #[test]
    fn coordinated_task_ids_share_a_leading_predicate() {
        let ledger = "Delivered P1.F1.T1 and P1.F1.T2.";
        let violations = check_consistency(
            ledger,
            &statuses(&[("P1.F1.T1", "done"), ("P1.F1.T2", "todo")]),
        );
        assert_eq!(violations.len(), 1);
        assert_eq!(violations[0].task_id, "P1.F1.T2");
    }

    #[test]
    fn three_task_coordinated_list_shares_a_trailing_predicate() {
        let ledger = "P1.F1.T1, P1.F1.T2, and P1.F1.T3 remain outstanding.";
        let violations = check_consistency(
            ledger,
            &statuses(&[
                ("P1.F1.T1", "done"),
                ("P1.F1.T2", "todo"),
                ("P1.F1.T3", "todo"),
            ]),
        );
        assert_eq!(violations.len(), 1);
        assert_eq!(violations[0].task_id, "P1.F1.T1");
    }

    #[test]
    fn comma_separated_independent_claims_do_not_share_predicates() {
        let ledger = "P1.F1.T1 landed, P1.F1.T2 remains open.";
        let violations = check_consistency(
            ledger,
            &statuses(&[("P1.F1.T1", "done"), ("P1.F1.T2", "todo")]),
        );
        assert!(violations.is_empty(), "unexpected: {violations:?}");
    }

    #[test]
    fn a_claim_does_not_reach_forward_past_a_comma() {
        // The mirror of the case above: here the predicate trails the comma and
        // belongs to the second task, so it must not be read back onto the
        // first one even though no task id sits between them.
        let ledger = "P1.F1.T1, remaining work sits with P1.F1.T2.";
        let violations = check_consistency(
            ledger,
            &statuses(&[("P1.F1.T1", "done"), ("P1.F1.T2", "todo")]),
        );
        assert!(violations.is_empty(), "unexpected: {violations:?}");
    }

    #[test]
    fn appositive_comma_keeps_predicate_with_task_claim() {
        let ledger = "P1.F1.T1, which remains open, despite the current plan.";
        let violations = check_consistency(ledger, &statuses(&[("P1.F1.T1", "done")]));
        assert_eq!(violations.len(), 1);
        assert_eq!(violations[0].task_id, "P1.F1.T1");
        assert!(violations[0].message.contains("open or blocking"));
    }

    #[test]
    fn mixed_leading_coordinated_list_keeps_shared_leading_predicate() {
        let ledger = "P9.F9.T9 landed, delivered P1.F1.T1, P1.F1.T2 and P1.F1.T3.";
        let violations = check_consistency(
            ledger,
            &statuses(&[
                ("P9.F9.T9", "done"),
                ("P1.F1.T1", "todo"),
                ("P1.F1.T2", "todo"),
                ("P1.F1.T3", "todo"),
            ]),
        );
        assert_eq!(
            violations
                .iter()
                .map(|violation| violation.task_id.as_str())
                .collect::<Vec<_>>(),
            vec!["P1.F1.T1", "P1.F1.T2", "P1.F1.T3"]
        );
    }

    #[test]
    fn mixed_trailing_coordinated_list_keeps_shared_trailing_predicate() {
        let ledger = "P1.F1.T1, P1.F1.T2 and P1.F1.T3 remain outstanding, P2.F2.T1 landed.";
        let violations = check_consistency(
            ledger,
            &statuses(&[
                ("P1.F1.T1", "done"),
                ("P1.F1.T2", "done"),
                ("P1.F1.T3", "done"),
                ("P2.F2.T1", "done"),
            ]),
        );
        assert_eq!(
            violations
                .iter()
                .map(|violation| violation.task_id.as_str())
                .collect::<Vec<_>>(),
            vec!["P1.F1.T1", "P1.F1.T2", "P1.F1.T3"]
        );
    }

    #[test]
    fn missing_task_display_omits_synthetic_backlog_status() {
        let violation = ConsistencyViolation {
            task_id: "P1.F1.T9".to_string(),
            backlog_status: "missing".to_string(),
            line_number: 4,
            message: "ledger cites a task absent from the backlog".to_string(),
        };
        assert_eq!(
            violation.to_string(),
            "P1.F1.T9:4: ledger cites a task absent from the backlog"
        );
    }

    #[test]
    fn ledger_task_absent_from_backlog_is_reported() {
        let violations = check_consistency(
            "P1.F1.T9 remains outstanding.",
            &statuses(&[("P1.F1.T1", "todo")]),
        );
        assert_eq!(violations.len(), 1);
        assert_eq!(violations[0].task_id, "P1.F1.T9");
        assert_eq!(violations[0].backlog_status, "missing");
        assert!(violations[0].message.contains("absent from the backlog"));
    }

    #[test]
    fn a_distant_marker_on_the_same_line_does_not_trigger() {
        let filler = "x".repeat(CONTEXT_CHARS * 2);
        let ledger = format!("blocker {filler} P2.F3.T4");
        let violations = check_consistency(&ledger, &statuses(&[("P2.F3.T4", "done")]));
        assert!(violations.is_empty(), "unexpected: {violations:?}");
    }

    fn matrix_row(id: &str, status: &str, evidence: &str) -> LedgerRow {
        LedgerRow {
            gate_id: id.to_string(),
            status: status.to_string(),
            evidence: evidence.to_string(),
        }
    }

    #[test]
    fn appendix_evals_deferred_contradicts_hostile_eval_evidence() {
        let appendix = "## Appendix A - Product Gate Mapping\n\
             | Product gate | Current ledger status | v0.2 milestone target |\n\
             | --- | --- | --- |\n\
             | PR-AI-002 proposal safety/evals | Substrate validated; adversarial evals deferred | M9/M10 |\n\
             ## Appendix B - next\n";
        let rows = [matrix_row(
            "PR-AI-002",
            "Substrate validated (proposal safety + adversarial evals)",
            "cargo test -p legion-app --test hostile_eval_integration; xtask hostile-evals",
        )];
        let violations = check_appendix_against_ledger(appendix, &rows);
        assert_eq!(violations.len(), 1);
        assert!(violations[0].message.contains("hostile-eval"));
    }

    #[test]
    fn appendix_matching_substrate_status_without_evals_deferred_passes() {
        let appendix = "## Appendix A - Product Gate Mapping\n\
             | PR-AI-002 proposal safety/evals | Substrate validated (hostile evals + xtask hostile-evals); live-model evals remain deferred | M9/M10 |\n";
        let rows = [matrix_row(
            "PR-AI-002",
            "Substrate validated (proposal safety + adversarial evals)",
            "hostile-eval integration plus xtask hostile-evals",
        )];
        let violations = check_appendix_against_ledger(appendix, &rows);
        assert!(violations.is_empty(), "{violations:?}");
    }

    #[test]
    fn appendix_deferred_does_not_match_substrate_validated() {
        let appendix = "## Appendix A - Product Gate Mapping\n\
             | PR-ENT-001 remote | Deferred | M13+ |\n";
        let rows = [matrix_row(
            "PR-ENT-001",
            "Substrate validated",
            "contracts only",
        )];
        let violations = check_appendix_against_ledger(appendix, &rows);
        assert_eq!(violations.len(), 1);
        assert!(violations[0].message.contains("does not match"));
    }
}
