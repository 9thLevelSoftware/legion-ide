//! Claim-audit gate: fails when current public docs make product claims
//! the product-readiness ledger does not support. Closes the M7/WS-P0
//! "claim-audit script or checklist" requirement (v1 scope).

const FORBIDDEN_PHRASES: [&str; 4] = [
    "production-ready",
    "production ready",
    "generally available",
    "ga-ready",
];
const NEGATION_MARKERS: [&str; 4] = ["not", "until", "require", "is not reached"];

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

pub fn audit_text(file: &str, text: &str) -> Vec<ClaimViolation> {
    let mut violations = Vec::new();
    for (index, line) in text.lines().enumerate() {
        let lower = line.to_lowercase();
        for phrase in FORBIDDEN_PHRASES {
            if lower.contains(phrase)
                && !NEGATION_MARKERS.iter().any(|marker| lower.contains(marker))
            {
                violations.push(ClaimViolation::ForbiddenPhrase {
                    file: file.to_string(),
                    line_number: index + 1,
                    phrase,
                });
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
