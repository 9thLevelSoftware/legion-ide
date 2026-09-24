//! Claim-audit gate: fails when current public docs make product claims
//! the product-readiness ledger does not support, and when AGENTS.md, the
//! ledger, or USER_GUIDE deny hosted workflows that exist in `.github/workflows/`.
//! Closes the M7/WS-P0 "claim-audit script or checklist" requirement (v1 scope)
//! and GAP-08.1 cross-document truth.

const FORBIDDEN_PHRASES: [&str; 4] = [
    "production-ready",
    "production ready",
    "generally available",
    "ga-ready",
];
const NEGATION_MARKERS: [&str; 4] = ["not", "n't", "never", "until"];
/// Number of characters immediately preceding a forbidden-phrase match that
/// are searched for a negation marker. Keeps negation scoped to the claim
/// itself ("Legion is **not** production-ready") rather than the whole
/// line, so an unrelated negation elsewhere on the line (e.g. "auto-update
/// is not validated" after a "generally available" claim) cannot suppress
/// a real violation.
const NEGATION_LOOKBEHIND_CHARS: usize = 30;
/// Phrases immediately following a forbidden-phrase match (after optional
/// whitespace) that negate it, e.g. "production-ready is not reached". Note
/// "is not reached" is intentionally omitted: it is a strict superstring of
/// "is not", which already matches via `starts_with` and therefore covers
/// it.
const NEGATION_FOLLOWUPS: [&str; 2] = ["is not", "has not"];

#[derive(Debug)]
pub enum ClaimViolation {
    ForbiddenPhrase {
        file: String,
        line_number: usize,
        phrase: &'static str,
    },
    MissingReadmeCaveat,
    /// A doc asserts a hosted fact that the workflow tree contradicts.
    CrossDocContradiction {
        file: String,
        line_number: usize,
        message: String,
    },
}

#[derive(Debug)]
pub struct LedgerRow {
    pub gate_id: String,
    pub status: String,
    pub evidence: String,
}

/// Hosted workflow facts the docs must not deny.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct HostedFacts {
    /// `.github/workflows/legion-release.yml` is present.
    pub release_workflow: bool,
    /// `legion-gates.yml` invokes `xtask -- perf-harness`.
    pub gates_runs_perf_harness: bool,
    /// `legion-gates.yml` invokes `xtask -- rust-analyzer-smoke`.
    pub gates_runs_rust_analyzer_smoke: bool,
    /// `.github/workflows/legion-smoke.yml` is present.
    pub smoke_workflow: bool,
}

impl HostedFacts {
    /// `Some` means the file exists (content may be empty); `None` means absent.
    pub fn from_workflow_texts(
        release_yml: Option<&str>,
        gates_yml: Option<&str>,
        smoke_yml: Option<&str>,
    ) -> Self {
        let gates = gates_yml.unwrap_or("");
        Self {
            release_workflow: release_yml.is_some(),
            gates_runs_perf_harness: workflow_invokes_xtask(gates, "perf-harness"),
            gates_runs_rust_analyzer_smoke: workflow_invokes_xtask(gates, "rust-analyzer-smoke"),
            smoke_workflow: smoke_yml.is_some(),
        }
    }
}

fn workflow_invokes_xtask(workflow: &str, command: &str) -> bool {
    workflow.contains(&format!("xtask -- {command}"))
        || workflow.contains(&format!("xtask.exe -- {command}"))
}

/// Inputs for GAP-08.1 cross-document claim checks.
pub struct CrossDocInputs<'a> {
    /// `AGENTS.md` body, when present.
    pub agents: Option<&'a str>,
    /// Product-readiness ledger body.
    pub ledger: &'a str,
    /// `docs/USER_GUIDE.md` body, when present.
    pub user_guide: Option<&'a str>,
    /// Facts taken from `.github/workflows/`.
    pub facts: HostedFacts,
}

/// Claim-audit negation heuristic (v1): a forbidden-phrase occurrence is
/// treated as negated — and therefore not flagged — only when one of the
/// following holds for that specific occurrence:
///
/// 1. A negation marker (`not`, `n't`, `never`, `until`) occurs on a word
///    boundary within the [`NEGATION_LOOKBEHIND_CHARS`] characters
///    immediately preceding the phrase on the line ("Legion is **not**
///    production-ready"). Word-boundary means the characters immediately
///    before and after the marker occurrence are non-alphanumeric (or the
///    marker sits at a window/string edge) — this prevents `"not"` from
///    matching inside `"notification"`. `n't` is special-cased to require
///    only a *trailing* boundary, since it legitimately follows letters in
///    contractions ("isn't", "doesn't").
/// 2. The phrase is immediately followed (allowing whitespace) by one of
///    [`NEGATION_FOLLOWUPS`] ("production-ready **is not** reached" — "is
///    not" is a strict prefix, so this also covers "is not reached").
///
/// This is deliberately phrase-local rather than line-global: a negation
/// marker anywhere else on the line must never suppress a genuine claim
/// elsewhere on that same line (e.g. "Legion is generally available, but
/// auto-update is not validated" still flags "generally available").
///
/// Known v1 limits: this is a single-line, character-window heuristic with
/// no real parsing — it does not follow negation across sentence or line
/// boundaries, does not understand double negatives, and a marker that
/// merely co-occurs within the lookbehind window (rather than truly
/// governing the claim) can still suppress a finding. Widen or replace this
/// with real sentence segmentation if false negatives become a problem.
fn phrase_is_negated(lower_line: &str, phrase_start: usize, phrase_end: usize) -> bool {
    let lookbehind_start = lower_line[..phrase_start]
        .char_indices()
        .rev()
        .nth(NEGATION_LOOKBEHIND_CHARS.saturating_sub(1))
        .map(|(index, _)| index)
        .unwrap_or(0);
    let preceding = &lower_line[lookbehind_start..phrase_start];
    if NEGATION_MARKERS
        .iter()
        .any(|marker| marker_occurs_on_word_boundary(preceding, marker))
    {
        return true;
    }

    let following = lower_line[phrase_end..].trim_start();
    NEGATION_FOLLOWUPS
        .iter()
        .any(|followup| following.starts_with(followup))
}

/// Returns `true` if `marker` occurs anywhere in `text` such that it sits on
/// a word boundary: the character immediately before and the character
/// immediately after the occurrence are both either absent (window edge) or
/// non-alphanumeric. This rejects e.g. `"not"` inside `"notification"`,
/// where the trailing boundary check fails (`i` is alphanumeric).
///
/// `n't` is special-cased to require only the trailing boundary, since as a
/// contraction suffix it always legitimately follows a letter ("isn't",
/// "doesn't", "won't") — requiring a leading boundary too would make it
/// unmatchable in practice.
fn marker_occurs_on_word_boundary(text: &str, marker: &str) -> bool {
    let requires_leading_boundary = marker != "n't";
    let mut search_from = 0;
    while let Some(relative_start) = text[search_from..].find(marker) {
        let start = search_from + relative_start;
        let end = start + marker.len();
        let leading_ok = !requires_leading_boundary
            || text[..start]
                .chars()
                .next_back()
                .is_none_or(|c| !c.is_alphanumeric());
        let trailing_ok = text[end..]
            .chars()
            .next()
            .is_none_or(|c| !c.is_alphanumeric());
        if leading_ok && trailing_ok {
            return true;
        }
        search_from = end;
    }
    false
}

pub fn audit_text(file: &str, text: &str) -> Vec<ClaimViolation> {
    let mut violations = Vec::new();
    for (index, line) in text.lines().enumerate() {
        let lower = line.to_lowercase();
        for phrase in FORBIDDEN_PHRASES {
            let mut search_from = 0;
            while let Some(relative_start) = lower[search_from..].find(phrase) {
                let phrase_start = search_from + relative_start;
                let phrase_end = phrase_start + phrase.len();
                if !phrase_is_negated(&lower, phrase_start, phrase_end) {
                    violations.push(ClaimViolation::ForbiddenPhrase {
                        file: file.to_string(),
                        line_number: index + 1,
                        phrase,
                    });
                }
                search_from = phrase_end;
            }
        }
    }
    violations
}

pub fn parse_ledger_rows(ledger: &str) -> Result<Vec<LedgerRow>, String> {
    let mut rows = Vec::new();
    for line in ledger.lines() {
        let cells: Vec<&str> = line.split('|').map(str::trim).collect();
        // | Track | Gate | Criteria | Status | Evidence | -> 7 cells with
        // leading/trailing empties.
        if cells.len() < 6 {
            continue;
        }
        let gate_cell = cells[2];
        let Some(gate_id) = gate_cell.split_whitespace().next() else {
            continue;
        };
        if !gate_id.starts_with("PR-") {
            continue;
        }
        rows.push(LedgerRow {
            gate_id: gate_id.to_string(),
            status: cells[4].to_string(),
            evidence: cells.get(5).copied().unwrap_or("").to_string(),
        });
    }
    if rows.is_empty() {
        return Err("no PR-* rows found in readiness matrix".to_string());
    }
    Ok(rows)
}

pub fn readme_caveat_present(readme: &str) -> bool {
    readme.contains("Legion is not yet a general-availability desktop product")
}

/// 1-based line numbers where `phrase` occurs without the v1 negation heuristic.
pub fn unnegated_phrase_lines(text: &str, phrase: &str) -> Vec<usize> {
    let needle = phrase.to_ascii_lowercase();
    let mut lines = Vec::new();
    for (index, line) in text.lines().enumerate() {
        let lower = line.to_lowercase();
        let mut search_from = 0;
        let mut hit = false;
        while let Some(relative_start) = lower[search_from..].find(&needle) {
            let phrase_start = search_from + relative_start;
            let phrase_end = phrase_start + needle.len();
            if !phrase_is_negated(&lower, phrase_start, phrase_end) {
                hit = true;
                break;
            }
            search_from = phrase_end;
        }
        if hit {
            lines.push(index + 1);
        }
    }
    lines
}

fn contradiction(file: &str, line_number: usize, message: impl Into<String>) -> ClaimViolation {
    ClaimViolation::CrossDocContradiction {
        file: file.to_string(),
        line_number,
        message: message.into(),
    }
}

/// Fail when AGENTS.md, the ledger, or USER_GUIDE deny a hosted workflow that exists.
pub fn audit_cross_docs(inputs: CrossDocInputs<'_>) -> Vec<ClaimViolation> {
    let mut violations = Vec::new();
    if let Some(agents) = inputs.agents {
        if inputs.facts.release_workflow {
            for line in
                unnegated_phrase_lines(agents, "No hosted release workflow is currently configured")
            {
                violations.push(contradiction(
                    "AGENTS.md",
                    line,
                    "AGENTS.md denies a hosted release workflow, but `.github/workflows/legion-release.yml` exists",
                ));
            }
        }
        if inputs.facts.gates_runs_perf_harness {
            for line in unnegated_phrase_lines(
                agents,
                "No hosted validate job currently runs these commands",
            ) {
                violations.push(contradiction(
                    "AGENTS.md",
                    line,
                    "AGENTS.md denies a hosted perf-harness job, but `legion-gates.yml` runs `xtask -- perf-harness`",
                ));
            }
        }
    }

    let rows = match parse_ledger_rows(inputs.ledger) {
        Ok(rows) => rows,
        Err(_) => return violations,
    };

    if inputs.facts.gates_runs_rust_analyzer_smoke {
        for row in &rows {
            if !unnegated_phrase_lines(
                &row.evidence,
                "3-OS hosted CI smoke is deferred pending CI infrastructure",
            )
            .is_empty()
            {
                violations.push(contradiction(
                    "plans/product-readiness-ledger.md",
                    ledger_row_line(inputs.ledger, &row.gate_id),
                    format!(
                        "{} evidence says 3-OS hosted CI smoke is deferred pending infrastructure, but `legion-gates.yml` runs `xtask -- rust-analyzer-smoke`",
                        row.gate_id
                    ),
                ));
            }
        }
    }

    if inputs.facts.smoke_workflow {
        for row in &rows {
            let pending =
                unnegated_phrase_lines(&row.evidence, "3-OS CI pending via `legion-smoke.yml`");
            let pending_plain =
                unnegated_phrase_lines(&row.evidence, "3-OS CI pending via legion-smoke.yml");
            if !pending.is_empty() || !pending_plain.is_empty() {
                violations.push(contradiction(
                    "plans/product-readiness-ledger.md",
                    ledger_row_line(inputs.ledger, &row.gate_id),
                    format!(
                        "{} evidence says 3-OS CI is pending via legion-smoke.yml, but `.github/workflows/legion-smoke.yml` already exists (it is independent, not absent)",
                        row.gate_id
                    ),
                ));
            }
        }
    }

    if let Some(guide) = inputs.user_guide {
        for line in unnegated_phrase_lines(
            guide,
            "assumes the reader already has a working build or a packaged desktop app",
        ) {
            violations.push(contradiction(
                "docs/USER_GUIDE.md",
                line,
                "USER_GUIDE assumes a packaged desktop app; that overstates the current unsigned-beta / substrate state",
            ));
        }
    }

    violations
}

fn ledger_row_line(ledger: &str, gate_id: &str) -> usize {
    for (index, line) in ledger.lines().enumerate() {
        if line.contains(gate_id) {
            return index + 1;
        }
    }
    1
}

#[cfg(test)]
mod cross_doc_tests {
    use super::*;

    fn sample_ledger(ui: &str, lang: &str) -> String {
        format!(
            "| Track | Gate | Acceptance Criteria | Current Status | Current Evidence |\n\
             | --- | --- | --- | --- | --- |\n\
             | UI | PR-UI-001 renderer | criteria | Substrate validated | {ui} |\n\
             | Lang | PR-LANG-001 Rust | criteria | Substrate validated | {lang} |\n"
        )
    }

    #[test]
    fn hosted_facts_detect_xtask_invocations() {
        let facts = HostedFacts::from_workflow_texts(
            Some("name: release\n"),
            Some(
                "run: cargo run -p xtask -- perf-harness\nrun: cargo run -p xtask -- rust-analyzer-smoke\n",
            ),
            Some("name: smoke\n"),
        );
        assert!(facts.release_workflow);
        assert!(facts.gates_runs_perf_harness);
        assert!(facts.gates_runs_rust_analyzer_smoke);
        assert!(facts.smoke_workflow);
    }

    #[test]
    fn agents_denying_hosted_release_is_a_contradiction() {
        let facts = HostedFacts::from_workflow_texts(Some(""), None, None);
        let agents = "No hosted release workflow is currently configured.\n";
        let ledger = sample_ledger("ok", "ok");
        let hits = audit_cross_docs(CrossDocInputs {
            agents: Some(agents),
            ledger: &ledger,
            user_guide: None,
            facts,
        });
        assert_eq!(hits.len(), 1);
        assert!(matches!(
            hits[0],
            ClaimViolation::CrossDocContradiction { .. }
        ));
    }

    #[test]
    fn honest_agents_with_hosted_release_passes() {
        let facts = HostedFacts::from_workflow_texts(Some(""), None, None);
        let agents = "`.github/workflows/legion-release.yml` is a manual unsigned-beta installer workflow; it is not a PR merge gate.\n";
        let ledger = sample_ledger("ok", "ok");
        let hits = audit_cross_docs(CrossDocInputs {
            agents: Some(agents),
            ledger: &ledger,
            user_guide: None,
            facts,
        });
        assert!(hits.is_empty(), "{hits:?}");
    }

    #[test]
    fn agents_denying_hosted_perf_harness_is_a_contradiction() {
        let facts = HostedFacts::from_workflow_texts(
            None,
            Some("run: cargo run -p xtask -- perf-harness\n"),
            None,
        );
        let agents = "No hosted validate job currently runs these commands.\n";
        let ledger = sample_ledger("ok", "ok");
        let hits = audit_cross_docs(CrossDocInputs {
            agents: Some(agents),
            ledger: &ledger,
            user_guide: None,
            facts,
        });
        assert_eq!(hits.len(), 1);
    }

    #[test]
    fn lang_row_deferring_hosted_ra_smoke_is_a_contradiction() {
        let facts = HostedFacts::from_workflow_texts(
            None,
            Some("run: cargo run -p xtask -- rust-analyzer-smoke\n"),
            None,
        );
        let ledger = sample_ledger(
            "ok",
            "3-OS hosted CI smoke is deferred pending CI infrastructure, consistent with PR-REL-001.",
        );
        let hits = audit_cross_docs(CrossDocInputs {
            agents: None,
            ledger: &ledger,
            user_guide: None,
            facts,
        });
        assert_eq!(hits.len(), 1);
        match &hits[0] {
            ClaimViolation::CrossDocContradiction { file, .. } => {
                assert_eq!(file, "plans/product-readiness-ledger.md");
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn ui_row_pending_existing_smoke_workflow_is_a_contradiction() {
        let facts = HostedFacts::from_workflow_texts(None, None, Some("name: Legion Smoke\n"));
        let ledger = sample_ledger(
            "GP-1 passed on Windows (single-OS; 3-OS CI pending via `legion-smoke.yml`).",
            "ok",
        );
        let hits = audit_cross_docs(CrossDocInputs {
            agents: None,
            ledger: &ledger,
            user_guide: None,
            facts,
        });
        assert_eq!(hits.len(), 1);
    }

    #[test]
    fn user_guide_assuming_packaged_app_is_a_contradiction() {
        let facts = HostedFacts::default();
        let ledger = sample_ledger("ok", "ok");
        let guide = "This guide is the end-user entry point.\nIt assumes the reader already has a working build or a packaged desktop app.\n";
        let hits = audit_cross_docs(CrossDocInputs {
            agents: None,
            ledger: &ledger,
            user_guide: Some(guide),
            facts,
        });
        assert_eq!(hits.len(), 1);
    }

    #[test]
    fn missing_workflows_do_not_invent_contradictions() {
        let facts = HostedFacts::default();
        let agents = "No hosted release workflow is currently configured.\nNo hosted validate job currently runs these commands.\n";
        let ledger = sample_ledger(
            "3-OS CI pending via `legion-smoke.yml`",
            "3-OS hosted CI smoke is deferred pending CI infrastructure",
        );
        let hits = audit_cross_docs(CrossDocInputs {
            agents: Some(agents),
            ledger: &ledger,
            user_guide: None,
            facts,
        });
        assert!(hits.is_empty(), "{hits:?}");
    }
}
