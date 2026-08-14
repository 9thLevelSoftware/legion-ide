use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
    sync::atomic::{AtomicU64, Ordering},
};

use xtask::claim_audit::{ClaimViolation, audit_text};

static TEMP_WORKSPACE_SEQUENCE: AtomicU64 = AtomicU64::new(1);

struct TempWorkspace {
    root: PathBuf,
}

impl TempWorkspace {
    fn new(label: &str) -> Self {
        let sequence = TEMP_WORKSPACE_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let root = std::env::temp_dir().join(format!(
            "legion-claim-audit-{label}-{}-{sequence}",
            std::process::id()
        ));
        fs::create_dir_all(root.join("plans")).expect("create claim-audit fixture");
        Self { root }
    }

    fn path(&self) -> &Path {
        &self.root
    }
}

impl Drop for TempWorkspace {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

#[test]
fn claim_audit_succeeds_without_retired_hermesgoal_document() {
    let workspace = TempWorkspace::new("without-hermesgoal");
    fs::write(
        workspace.path().join("README.md"),
        "Legion is not yet a general-availability desktop product.\n",
    )
    .expect("write README fixture");
    fs::write(
        workspace
            .path()
            .join("plans")
            .join("product-readiness-ledger.md"),
        "| Track | Gate | Acceptance Criteria | Current Status | Current Evidence |\n\
         | --- | --- | --- | --- | --- |\n\
         | Core | PR-CORE-001 | criteria | Product workflow validated | tests |\n",
    )
    .expect("write readiness ledger fixture");

    let output = Command::new(env!("CARGO_BIN_EXE_xtask"))
        .arg("claim-audit")
        .current_dir(workspace.path())
        .output()
        .expect("run claim-audit binary");

    assert!(
        output.status.success(),
        "claim-audit should not require the retired HERMESGOAL.md document\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

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
fn negation_marker_exactly_at_the_lookbehind_edge_still_negates() {
    // "not" followed by 27 filler chars, then "production-ready" starting at
    // index 30: the 30-char lookbehind window is exactly [0, 30), which
    // fully contains "not" (indices [0, 3)). This is the innermost edge of
    // the window where the marker must still be found.
    let line = format!("not{}production-ready", "-".repeat(27));
    let violations = audit_text("README.md", &line);
    assert!(
        violations.is_empty(),
        "marker at the inner edge of the lookbehind window must still negate"
    );
}

#[test]
fn negation_marker_one_char_past_the_lookbehind_edge_does_not_negate() {
    // Same construction, but with one extra filler char: "production-ready"
    // now starts at index 31, so the 30-char lookbehind window is [1, 31),
    // which clips the leading "n" off "not" and no longer contains any
    // negation marker. This must be flagged.
    let line = format!("not{}production-ready", "-".repeat(28));
    let violations = audit_text("README.md", &line);
    assert_eq!(
        violations.len(),
        1,
        "marker just outside the lookbehind window must not negate"
    );
    assert!(matches!(
        violations[0],
        ClaimViolation::ForbiddenPhrase {
            phrase: "production-ready",
            ..
        }
    ));
}

#[test]
fn mixed_line_with_one_negated_and_one_unnegated_occurrence_flags_only_the_unnegated_one() {
    let line = "Legion is not production-ready today, though marketing once claimed it was production-ready.";
    let violations = audit_text("README.md", line);
    assert_eq!(
        violations.len(),
        1,
        "only the unnegated occurrence of the repeated phrase should be flagged"
    );
    assert!(matches!(
        violations[0],
        ClaimViolation::ForbiddenPhrase {
            phrase: "production-ready",
            ..
        }
    ));
}

#[test]
fn substring_negation_marker_inside_another_word_does_not_negate() {
    // Codex counterexample: "notification" contains "not" as a literal
    // substring, but "not" does not occur there on a word boundary (the
    // character right after it, 'i', is alphanumeric), so it must not
    // suppress the claim.
    let violations = audit_text(
        "README.md",
        "Notification support is production-ready today",
    );
    assert_eq!(
        violations.len(),
        1,
        "\"not\" inside \"notification\" must not be treated as a negation marker"
    );
    assert!(matches!(
        violations[0],
        ClaimViolation::ForbiddenPhrase {
            phrase: "production-ready",
            ..
        }
    ));
}

#[test]
fn contraction_negation_marker_still_negates() {
    // "n't" legitimately follows a letter in a contraction ("isn't"), so it
    // must still count as a negation marker even though its leading
    // character is alphanumeric.
    let violations = audit_text("README.md", "Legion isn't production-ready yet.");
    assert!(
        violations.is_empty(),
        "the \"n't\" contraction marker must still negate despite following a letter"
    );
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
