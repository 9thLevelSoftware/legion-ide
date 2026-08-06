//! Cross-check the product-readiness ledger against the Kanban backlog.
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
        write!(
            f,
            "{}:{}: {} (backlog says `{}`)",
            self.task_id, self.line_number, self.message, self.backlog_status
        )
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

    let start = siblings
        .iter()
        .filter(|other| other.end <= mention.start)
        .filter(|other| !mentions_are_coordinated(&line[other.end..mention.start]))
        .map(|other| other.end)
        .max()
        .map_or(window_start, |boundary| boundary.max(window_start))
        .max(clause_start(line, mention.start));
    let end = siblings
        .iter()
        .filter(|other| other.start >= mention.end)
        .filter(|other| !mentions_are_coordinated(&line[mention.end..other.start]))
        .map(|other| other.start)
        .min()
        .map_or(window_end, |boundary| boundary.min(window_end))
        .min(clause_end(line, mention.end));

    line[start..end].to_lowercase()
}

/// Whether adjacent task IDs form a coordinated list that shares the claim
/// before or after it (for example, `T1 and T2 remain outstanding`).
fn mentions_are_coordinated(between: &str) -> bool {
    let connector = between
        .trim_matches(|ch: char| ch.is_whitespace() || matches!(ch, ',' | '&' | '/' | '(' | ')'))
        .to_ascii_lowercase();
    matches!(connector.as_str(), "" | "and" | "or")
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

    let violations = check_consistency(&ledger_text, &statuses);
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
}
