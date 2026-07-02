use xtask::claim_audit::{ClaimViolation, audit_text};

#[test]
fn forbidden_claim_is_flagged() {
    let violations = audit_text("README.md", "Legion is production-ready today.");
    assert_eq!(violations.len(), 1);
    assert!(matches!(
        violations[0],
        ClaimViolation::ForbiddenPhrase { .. }
    ));
}

#[test]
fn negated_claim_is_allowed() {
    let violations = audit_text(
        "README.md",
        "Legion is not production-ready until GP-1 through GP-6 pass.",
    );
    assert!(violations.is_empty());
}

#[test]
fn unrelated_negation_elsewhere_on_the_line_does_not_suppress_a_real_claim() {
    // Codex counterexample: a `not` later in the line (governing a
    // different clause) must not blanket-suppress a genuine claim earlier
    // in the same line.
    let violations = audit_text(
        "README.md",
        "Legion is generally available, but auto-update is not validated.",
    );
    assert_eq!(violations.len(), 1);
    assert!(matches!(
        violations[0],
        ClaimViolation::ForbiddenPhrase {
            phrase: "generally available",
            ..
        }
    ));
}

#[test]
fn ledger_rows_parse() {
    let ledger = "| Track | Gate | Acceptance Criteria | Current Status | Current Evidence |\n\
                  | --- | --- | --- | --- | --- |\n\
                  | AI | PR-AI-001 inspectable AI | criteria | Product workflow validated | tests |";
    let rows = xtask::claim_audit::parse_ledger_rows(ledger).expect("parses");
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].gate_id, "PR-AI-001");
    assert_eq!(rows[0].status, "Product workflow validated");
}
