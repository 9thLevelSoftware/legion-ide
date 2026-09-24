//! Semantic validation of one evidence run against its canonical scenario.
//!
//! This checks record consistency only. It does not authenticate a human,
//! observer, collector, or artifact bytes; file hashing and identity evidence
//! belong to the consuming evidence-file validator.

use std::collections::BTreeSet;

use super::{
    artifact_files::parse_utc_timestamp,
    schema::{EvidenceLayer, EvidenceResult, EvidenceRun, Scenario},
};

/// Validate the outcome and review fields of one run against its scenario.
///
/// The returned messages are deterministic and sorted. Non-passed runs may be
/// incomplete and may carry a changes-required review, but every supplied
/// check, attachment reference, owner, and timestamp field must still be
/// well-formed.
pub fn validate_run_outcomes(scenario: &Scenario, run: &EvidenceRun) -> Vec<String> {
    let mut issues = Vec::new();

    if run.scenario_id != scenario.id {
        issues.push(format!(
            "run `{}` scenario identity `{}` does not match `{}`",
            run.id, run.scenario_id, scenario.id
        ));
    }
    validate_scenario_ids(scenario, &mut issues);
    validate_checks(
        "oracle",
        &scenario.external_oracles,
        &run.oracle_results,
        &run.artifact_hashes,
        run.result == EvidenceResult::Passed,
        &mut issues,
    );
    validate_checks(
        "recovery",
        &scenario.recovery_cases,
        &run.recovery_results,
        &run.artifact_hashes,
        run.result == EvidenceResult::Passed,
        &mut issues,
    );
    validate_artifact_hashes(run, &mut issues);
    validate_times(run, &mut issues);

    let owners: Vec<_> = run
        .implementation_owners
        .iter()
        .map(|owner| owner.trim())
        .collect();
    if run
        .implementation_owners
        .iter()
        .any(|owner| owner.trim().is_empty())
    {
        issues.push(format!(
            "run `{}` has an empty implementation owner",
            run.id
        ));
    }
    if run.result == EvidenceResult::Passed {
        if owners.is_empty() {
            issues.push(format!("run `{}` has no implementation owner", run.id));
        }
        let reviewer = run.reviewer.trim();
        if reviewer.is_empty() {
            issues.push(format!("run `{}` has a blank reviewer", run.id));
        } else if owners.contains(&reviewer) {
            issues.push(format!(
                "run `{}` reviewer must be distinct from implementation owners",
                run.id
            ));
        }
        if run.review_decision != super::schema::ReviewDecision::Accepted {
            issues.push(format!(
                "run `{}` passed result requires accepted review",
                run.id
            ));
        }
        if run.layer == EvidenceLayer::Product && scenario.external_oracles.is_empty() {
            issues.push(format!(
                "run `{}` passed product result requires at least one external oracle",
                run.id
            ));
        }
    } else if !run.reviewer.trim().is_empty()
        && owners.iter().any(|owner| *owner == run.reviewer.trim())
    {
        issues.push(format!(
            "run `{}` reviewer must be distinct from implementation owners",
            run.id
        ));
    }

    issues.sort();
    issues.dedup();
    issues
}

fn validate_scenario_ids(scenario: &Scenario, issues: &mut Vec<String>) {
    validate_definition_ids("oracle", &scenario.external_oracles, issues);
    validate_definition_ids("recovery", &scenario.recovery_cases, issues);
}

fn validate_definition_ids(
    kind: &str,
    checks: &[super::schema::ScenarioCheck],
    issues: &mut Vec<String>,
) {
    let mut ids = BTreeSet::new();
    for check in checks {
        if check.id.trim().is_empty() {
            issues.push(format!("scenario has an empty {kind} id"));
        } else if !ids.insert(check.id.as_str()) {
            issues.push(format!("scenario has duplicate {kind} id `{}`", check.id));
        }
    }
}

fn validate_checks(
    kind: &str,
    expected: &[super::schema::ScenarioCheck],
    reported: &[super::schema::CheckResult],
    artifact_hashes: &std::collections::BTreeMap<String, String>,
    require_complete: bool,
    issues: &mut Vec<String>,
) {
    let expected_ids: BTreeSet<_> = expected.iter().map(|check| check.id.as_str()).collect();
    let mut reported_ids = BTreeSet::new();
    for check in reported {
        if check.id.trim().is_empty() {
            issues.push(format!("{kind} check has an empty id"));
        } else if !reported_ids.insert(check.id.as_str()) {
            issues.push(format!("duplicate {kind} check `{}`", check.id));
        } else if !expected_ids.contains(check.id.as_str()) {
            issues.push(format!("unknown {kind} check `{}`", check.id));
        }
        if require_complete && !check.passed {
            issues.push(format!("passed run has failed {kind} `{}`", check.id));
        }
        if check.observed.trim().is_empty() {
            issues.push(format!(
                "{kind} check `{}` has empty observed text",
                check.id
            ));
        }
        if check.artifact_path.trim().is_empty() {
            issues.push(format!(
                "{kind} check `{}` has empty artifact path",
                check.id
            ));
        } else if !artifact_hashes.contains_key(&check.artifact_path) {
            issues.push(format!(
                "{kind} check `{}` artifact `{}` is not listed in artifact_hashes",
                check.id, check.artifact_path
            ));
        }
    }
    if require_complete {
        for check in expected {
            if !reported_ids.contains(check.id.as_str()) {
                issues.push(format!("passed run is missing {kind} `{}`", check.id));
            }
        }
    }
}

fn validate_artifact_hashes(run: &EvidenceRun, issues: &mut Vec<String>) {
    for (path, hash) in &run.artifact_hashes {
        if path.trim().is_empty() {
            issues.push(format!("run `{}` has an empty artifact path", run.id));
        }
        if !is_sha256(hash) {
            issues.push(format!(
                "run `{}` artifact `{}` has invalid SHA-256 hash",
                run.id, path
            ));
        }
    }
}

fn validate_times(run: &EvidenceRun, issues: &mut Vec<String>) {
    let started = parse_utc_timestamp(&run.started_at_utc);
    let ended = parse_utc_timestamp(&run.ended_at_utc);
    if started.is_err() {
        issues.push(format!("run `{}` has invalid started_at_utc", run.id));
    }
    if ended.is_err() {
        issues.push(format!("run `{}` has invalid ended_at_utc", run.id));
    }
    if let (Ok(started), Ok(ended)) = (started, ended)
        && ended < started
    {
        issues.push(format!(
            "run `{}` ended_at_utc is before started_at_utc",
            run.id
        ));
    }
}

fn is_sha256(value: &str) -> bool {
    crate::completion::hash::is_sha256(value)
}
