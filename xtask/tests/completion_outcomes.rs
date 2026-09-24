use std::collections::BTreeMap;

use xtask::completion::{
    outcomes::validate_run_outcomes,
    schema::{
        CheckResult, EvidenceLayer, EvidenceResult, EvidenceRun, InputRoute, ReviewDecision,
        Scenario, ScenarioCheck,
    },
};

fn scenario() -> Scenario {
    Scenario {
        id: "SCN-1".into(),
        requirement_ids: vec!["REQ-1".into()],
        configuration_ids: vec!["CFG-1".into()],
        steps: vec!["run workflow".into()],
        external_oracles: vec![ScenarioCheck {
            id: "oracle-1".into(),
            description: "external observation".into(),
        }],
        recovery_cases: vec![ScenarioCheck {
            id: "recovery-1".into(),
            description: "recover state".into(),
        }],
        sensitive_artifact_policy: "metadata-only".into(),
    }
}

fn check(id: &str, passed: bool, artifact_path: &str, observed: &str) -> CheckResult {
    CheckResult {
        id: id.into(),
        passed,
        artifact_path: artifact_path.into(),
        observed: observed.into(),
    }
}

fn run() -> EvidenceRun {
    EvidenceRun {
        schema_version: 1,
        id: "RUN-1".into(),
        scenario_id: "SCN-1".into(),
        configuration_id: "CFG-1".into(),
        candidate_sha: "candidate".into(),
        artifact_sha256: "artifact".into(),
        layer: EvidenceLayer::Product,
        input_route: InputRoute::NativeInput,
        dependencies: Vec::new(),
        result: EvidenceResult::Passed,
        oracle_results: vec![check("oracle-1", true, "oracle.txt", "observed")],
        recovery_results: vec![check("recovery-1", true, "recovery.txt", "recovered")],
        artifact_hashes: BTreeMap::from([
            (
                "oracle.txt".into(),
                "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".into(),
            ),
            (
                "recovery.txt".into(),
                "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb".into(),
            ),
        ]),
        defect_ids: Vec::new(),
        implementation_owners: vec!["implementer".into()],
        reviewer: "reviewer".into(),
        review_decision: ReviewDecision::Accepted,
        started_at_utc: "2026-01-01T00:00:00Z".into(),
        ended_at_utc: "2026-01-01T00:01:00Z".into(),
    }
}

fn assert_issue(run: &EvidenceRun, expected: &str) {
    let issues = validate_run_outcomes(&scenario(), run);
    assert!(
        issues.iter().any(|issue| issue.contains(expected)),
        "expected {expected:?} in {issues:?}"
    );
}

#[test]
fn complete_passed_product_run_is_valid() {
    assert!(validate_run_outcomes(&scenario(), &run()).is_empty());
}

#[test]
fn scenario_identity_and_reported_ids_are_exact() {
    let mut mismatched = run();
    mismatched.scenario_id = "SCN-other".into();
    assert_issue(&mismatched, "scenario identity");

    let mut duplicate = run();
    duplicate
        .oracle_results
        .push(check("oracle-1", true, "oracle.txt", "again"));
    assert_issue(&duplicate, "duplicate oracle");

    let mut unknown = run();
    unknown
        .recovery_results
        .push(check("recovery-unknown", true, "recovery.txt", "x"));
    assert_issue(&unknown, "unknown recovery");
}

#[test]
fn empty_reported_id_is_rejected() {
    let mut empty = run();
    empty.oracle_results[0].id = " ".into();
    assert_issue(&empty, "empty id");
}

#[test]
fn empty_reported_observed_text_is_rejected() {
    let mut empty = run();
    empty.oracle_results[0].observed = "".into();
    assert_issue(&empty, "empty observed");
}

#[test]
fn empty_reported_artifact_path_is_rejected() {
    let mut empty = run();
    empty.oracle_results[0].artifact_path = " ".into();
    assert_issue(&empty, "empty artifact path");
}

#[test]
fn missing_required_recovery_is_rejected() {
    let mut missing = run();
    missing.recovery_results.clear();
    assert_issue(&missing, "missing recovery");
}

#[test]
fn duplicate_required_recovery_is_rejected() {
    let mut duplicate = run();
    duplicate
        .recovery_results
        .push(check("recovery-1", true, "recovery.txt", "again"));
    assert_issue(&duplicate, "duplicate recovery");
}

#[test]
fn unknown_oracle_is_rejected() {
    let mut unknown = run();
    unknown
        .oracle_results
        .push(check("oracle-unknown", true, "oracle.txt", "x"));
    assert_issue(&unknown, "unknown oracle");
}

#[test]
fn failed_run_still_rejects_malformed_present_check_fields_and_hashes() {
    let mut failed = run();
    failed.result = EvidenceResult::Failed;
    failed.oracle_results[0].observed.clear();
    failed
        .artifact_hashes
        .insert("bad.txt".into(), "nope".into());
    assert_issue(&failed, "empty observed");
    assert_issue(&failed, "invalid SHA-256");
}

#[test]
fn passed_product_run_requires_an_external_oracle() {
    let mut no_oracles = run();
    let mut scenario_without_oracles = scenario();
    scenario_without_oracles.external_oracles.clear();
    no_oracles.oracle_results.clear();
    let issues = validate_run_outcomes(&scenario_without_oracles, &no_oracles);
    assert!(
        issues.iter().any(|issue| issue.contains("external oracle")),
        "{issues:?}"
    );
}

#[test]
fn passed_run_requires_independent_accepted_review_and_owners() {
    let mut missing_owner = run();
    missing_owner.implementation_owners.clear();
    assert_issue(&missing_owner, "implementation owner");

    let mut self_review = run();
    self_review.reviewer = " implementer ".into();
    assert_issue(&self_review, "reviewer");

    let mut changes_required = run();
    changes_required.review_decision = ReviewDecision::ChangesRequired;
    assert_issue(&changes_required, "accepted review");
}

#[test]
fn passed_run_rejects_blank_reviewer_and_space_only_owner() {
    let mut blank_reviewer = run();
    blank_reviewer.reviewer = " ".into();
    assert_issue(&blank_reviewer, "blank reviewer");

    let mut blank_owner = run();
    blank_owner.implementation_owners = vec!["  ".into()];
    assert_issue(&blank_owner, "empty implementation owner");
}

#[test]
fn all_statuses_require_valid_timestamps_but_nonpassed_runs_may_be_incomplete() {
    let mut honest = run();
    honest.result = EvidenceResult::Failed;
    honest.oracle_results.clear();
    honest.recovery_results.clear();
    honest.review_decision = ReviewDecision::ChangesRequired;
    assert!(validate_run_outcomes(&scenario(), &honest).is_empty());

    let mut invalid_time = honest.clone();
    invalid_time.started_at_utc = "2026-02-30T00:00:00Z".into();
    assert_issue(&invalid_time, "started_at_utc");

    let mut malformed_time = honest;
    malformed_time.started_at_utc = "2026-01-01 00:00:00Z".into();
    assert_issue(&malformed_time, "started_at_utc");

    let mut reversed = run();
    reversed.ended_at_utc = "2025-12-31T23:59:59Z".into();
    assert_issue(&reversed, "before started_at_utc");
}

#[test]
fn artifact_hashes_accept_ascii_uppercase_hex_and_reject_nonhex() {
    let mut uppercase = run();
    uppercase
        .artifact_hashes
        .insert("oracle.txt".into(), "A".repeat(64));
    assert!(validate_run_outcomes(&scenario(), &uppercase).is_empty());

    let mut nonhex = run();
    nonhex
        .artifact_hashes
        .insert("oracle.txt".into(), format!("{}g", "a".repeat(63)));
    assert_issue(&nonhex, "invalid SHA-256");
}
