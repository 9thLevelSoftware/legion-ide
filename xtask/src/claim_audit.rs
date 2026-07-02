//! Claim-audit gate: fails when current public docs make product claims
//! the product-readiness ledger does not support. Closes the M7/WS-P0
//! "claim-audit script or checklist" requirement (v1 scope).

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
}

#[derive(Debug)]
pub struct LedgerRow {
    pub gate_id: String,
    pub status: String,
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
