//! Deterministic metadata joins for completion evidence.
//!
//! This module validates typed register/evidence links and run outcome,
//! reviewer, and timestamp integrity. It does not read files, hash artifacts,
//! authenticate a candidate nomination, or produce a release verdict. An
//! empty issue list proves metadata consistency, outcome integrity, and
//! coverage only; it is not authentic product acceptance or release readiness.
//!
//! In release mode it additionally rejects any matrix configuration whose
//! `owner_approval_ref` carries a provisional/agent-made ratification marker
//! (see [`super::ratification`]). That rejection is a fail-closed text check
//! and is not authentication of owner ratification: its absence establishes
//! nothing positive about who ratified the matrix.

use std::collections::{BTreeMap, BTreeSet};

use super::schema::{
    DependencyExecution, EvidenceRun, MatrixDocument, Requirement, RequirementKind,
    RequirementsDocument, Scenario, ScenariosDocument,
};

/// Validate typed requirement/scenario/configuration/evidence metadata joins
/// and each run's semantic outcome/review integrity.
///
/// `candidate` is supplied by the caller and is compared literally with every
/// selected run. No verifier checkout HEAD or candidate manifest is read, and
/// no artifact bytes are loaded or hashed.
/// `release` adds required-product acceptance and required-matrix coverage
/// checks, and rejects every matrix configuration whose `owner_approval_ref`
/// carries a provisional/agent-made ratification marker; it does not
/// authenticate evidence, authenticate owner ratification, or nominate a
/// candidate.
pub fn validate_metadata_links(
    requirements: &RequirementsDocument,
    matrix: &MatrixDocument,
    scenarios: &ScenariosDocument,
    runs: &[EvidenceRun],
    candidate: &str,
    release: bool,
) -> Vec<String> {
    let mut issues = Vec::new();
    if candidate.trim().is_empty() {
        issues.push("candidate is empty".to_string());
    }

    let req_by_id: BTreeMap<_, _> = requirements
        .requirements
        .iter()
        .map(|row| (row.id.as_str(), row))
        .collect();
    let scenario_by_id: BTreeMap<_, _> = scenarios
        .scenarios
        .iter()
        .map(|row| (row.id.as_str(), row))
        .collect();
    let config_ids: BTreeSet<_> = matrix
        .configurations
        .iter()
        .map(|row| row.id.as_str())
        .collect();

    let mut run_ids = BTreeSet::new();
    for run in runs {
        if run.id.trim().is_empty() {
            issues.push("evidence run id is empty".to_string());
        } else if !run_ids.insert(run.id.as_str()) {
            issues.push(format!("duplicate evidence run id `{}`", run.id));
        }
        if run.candidate_sha != candidate {
            issues.push(format!(
                "evidence run `{}` candidate `{}` does not match selected candidate `{}`",
                run.id, run.candidate_sha, candidate
            ));
        }
        let scenario = match scenario_by_id.get(run.scenario_id.as_str()) {
            Some(scenario) => *scenario,
            None => {
                issues.push(format!(
                    "evidence run `{}` references unknown scenario `{}`",
                    run.id, run.scenario_id
                ));
                continue;
            }
        };
        issues.extend(super::outcomes::validate_run_outcomes(scenario, run));
        if !config_ids.contains(run.configuration_id.as_str()) {
            issues.push(format!(
                "evidence run `{}` references unknown configuration `{}`",
                run.id, run.configuration_id
            ));
        } else if !scenario
            .configuration_ids
            .iter()
            .any(|id| id == &run.configuration_id)
        {
            issues.push(format!(
                "evidence run `{}` configuration `{}` is outside scenario `{}`",
                run.id, run.configuration_id, run.scenario_id
            ));
        }
        if scenario.requirement_ids.is_empty() {
            issues.push(format!(
                "evidence run `{}` scenario `{}` has no requirement scope",
                run.id, run.scenario_id
            ));
        } else if scenario
            .requirement_ids
            .iter()
            .any(|id| !req_by_id.contains_key(id.as_str()))
        {
            issues.push(format!(
                "evidence run `{}` scenario `{}` has an unknown requirement scope",
                run.id, run.scenario_id
            ));
        } else if scenario.requirement_ids.iter().any(|id| {
            req_by_id.get(id.as_str()).is_some_and(|requirement| {
                !requirement
                    .scenario_ids
                    .iter()
                    .any(|scenario_id| scenario_id == &run.scenario_id)
            })
        }) {
            issues.push(format!(
                "evidence run `{}` scenario `{}` has an out-of-scope requirement link",
                run.id, run.scenario_id
            ));
        }
    }

    let mut eligible_by_pair = BTreeSet::new();
    for run in runs {
        if run.candidate_sha == candidate
            && qualifies_run(run)
            && scenario_by_id
                .get(run.scenario_id.as_str())
                .is_some_and(|scenario| {
                    scenario
                        .configuration_ids
                        .iter()
                        .any(|id| id == &run.configuration_id)
                        && scenario
                            .requirement_ids
                            .iter()
                            .any(|id| req_by_id.contains_key(id.as_str()))
                })
        {
            eligible_by_pair.insert((run.scenario_id.as_str(), run.configuration_id.as_str()));
        }
    }

    let mut covered_required_configs = BTreeSet::new();
    for requirement in &requirements.requirements {
        if requirement.kind == RequirementKind::Product
            && requirement.acceptance == super::schema::Acceptance::Accepted
        {
            validate_accepted_product(
                requirement,
                &scenario_by_id,
                &config_ids,
                &eligible_by_pair,
                &mut issues,
            );
        }
        if release && requirement.kind == RequirementKind::Product && requirement.required {
            if requirement.acceptance != super::schema::Acceptance::Accepted {
                issues.push(format!(
                    "required product requirement `{}` is not accepted",
                    requirement.id
                ));
            }
            for config_id in &requirement.configuration_ids {
                if requirement.scenario_ids.iter().any(|scenario_id| {
                    eligible_by_pair.contains(&(scenario_id.as_str(), config_id.as_str()))
                }) {
                    covered_required_configs.insert(config_id.as_str());
                }
            }
        }
    }
    if release {
        for configuration in &matrix.configurations {
            if configuration.required
                && !covered_required_configs.contains(configuration.id.as_str())
            {
                issues.push(format!(
                    "required matrix configuration `{}` has no eligible required-product coverage",
                    configuration.id
                ));
            }
            // Applies to every configuration, required or not: a provisional
            // cell is provisional either way.
            if let Some(marker) = super::ratification::provisional_ratification_marker(
                &configuration.owner_approval_ref,
            ) {
                issues.push(format!(
                    "matrix configuration `{}` owner_approval_ref carries provisional ratification marker `{marker}`: release mode rejects a provisionally ratified matrix",
                    configuration.id
                ));
            }
        }
    }

    issues.sort();
    issues.dedup();
    issues
}

fn qualifies_run(run: &EvidenceRun) -> bool {
    super::qualifies_as_product_evidence(
        &run.layer.to_string(),
        &run.input_route.to_string(),
        &run.result.to_string(),
        run.dependencies.iter().any(|dependency| {
            dependency.required_for_outcome
                && dependency.execution == DependencyExecution::Substituted
        }),
    )
}

fn validate_accepted_product(
    requirement: &Requirement,
    scenarios: &BTreeMap<&str, &Scenario>,
    configurations: &BTreeSet<&str>,
    eligible_by_pair: &BTreeSet<(&str, &str)>,
    issues: &mut Vec<String>,
) {
    let mut pairs = BTreeSet::new();
    for scenario_id in &requirement.scenario_ids {
        let Some(scenario) = scenarios.get(scenario_id.as_str()) else {
            issues.push(format!(
                "accepted product requirement `{}` references unknown scenario `{}`",
                requirement.id, scenario_id
            ));
            continue;
        };
        if !scenario
            .requirement_ids
            .iter()
            .any(|id| id == &requirement.id)
        {
            issues.push(format!(
                "accepted product requirement `{}` scenario `{}` is not reciprocal",
                requirement.id, scenario_id
            ));
            continue;
        }
        for config_id in requirement.configuration_ids.iter().filter(|config_id| {
            scenario
                .configuration_ids
                .iter()
                .any(|scenario_config| scenario_config == *config_id)
        }) {
            if !configurations.contains(config_id.as_str()) {
                issues.push(format!(
                    "accepted product requirement `{}` references unknown configuration `{}`",
                    requirement.id, config_id
                ));
                continue;
            }
            pairs.insert((scenario_id.as_str(), config_id.as_str()));
        }
    }
    if pairs.is_empty() {
        issues.push(format!(
            "accepted product requirement `{}` has no applicable scenario/configuration pair",
            requirement.id
        ));
    }
    for (scenario_id, config_id) in pairs {
        if !eligible_by_pair.contains(&(scenario_id, config_id)) {
            issues.push(format!(
                "accepted product requirement `{}` lacks eligible evidence for scenario `{}` configuration `{}`",
                requirement.id, scenario_id, config_id
            ));
        }
    }
}
