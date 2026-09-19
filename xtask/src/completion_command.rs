//! Composition and presentation for the verify-completion command.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

use serde::de::DeserializeOwned;

use crate::completion::links::validate_metadata_links;
use crate::completion::schema::{
    Acceptance, Defect, DefectStatus, DefectsDocument, DependenciesDocument, DependencyExecution,
    EvidenceResult, EvidenceRun, MatrixDocument, RequirementKind, RequirementsDocument,
    ScenariosDocument,
};
use crate::completion::structure::validate_register_structure;
use crate::completion_evidence::{load_evidence_runs, validate_evidence_files};

/// Separate implementation and acceptance counts for CLI reporting.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct CompletionStatusCounts {
    pub implementation: BTreeMap<String, usize>,
    pub acceptance: BTreeMap<String, usize>,
}

/// Compose the canonical completion validators.
///
/// Operational load/IO/JSON failures are returned as Err. Invariant failures
/// are returned as a sorted, nonempty violation list. Development mode permits
/// incomplete unaccepted rows while preserving all false-acceptance checks;
/// release mode adds required product/configuration coverage and unresolved
/// required-outcome defect checks.
pub fn validate_completion(
    root: &Path,
    candidate: &str,
    release: bool,
) -> Result<Vec<String>, String> {
    let mut issues = validate_register_structure(root)?;
    issues.extend(validate_evidence_files(root, candidate)?);

    if release {
        let requirements: RequirementsDocument =
            load_json(root, "plans/completion/requirements.json")?;
        let matrix: MatrixDocument = load_json(root, "plans/completion/matrix.json")?;
        let scenarios: ScenariosDocument = load_json(root, "plans/completion/scenarios.json")?;
        let runs = load_evidence_runs(root, candidate)?;
        issues.extend(validate_metadata_links(
            &requirements,
            &matrix,
            &scenarios,
            &runs,
            candidate,
            true,
        ));
        let defects: DefectsDocument = load_json(root, "plans/completion/defects.json")?;
        issues.extend(validate_defects(
            &defects.defects,
            &requirements,
            &runs,
            candidate,
            true,
        ));
        let dependencies: DependenciesDocument =
            load_json(root, "plans/completion/dependencies.json")?;
        issues.extend(validate_release_dependencies(
            &dependencies,
            &requirements,
            &scenarios,
            &runs,
        ));
        issues.extend(validate_release_requirement_coverage(
            &requirements,
            &matrix,
            &scenarios,
            &runs,
            true,
        ));
    }

    if !release {
        let defects: DefectsDocument = load_json(root, "plans/completion/defects.json")?;
        let runs = load_evidence_runs(root, candidate)?;
        let requirements: RequirementsDocument =
            load_json(root, "plans/completion/requirements.json")?;
        let matrix: MatrixDocument = load_json(root, "plans/completion/matrix.json")?;
        let scenarios: ScenariosDocument = load_json(root, "plans/completion/scenarios.json")?;
        issues.extend(validate_defects(
            &defects.defects,
            &requirements,
            &runs,
            candidate,
            false,
        ));
        issues.extend(validate_release_requirement_coverage(
            &requirements,
            &matrix,
            &scenarios,
            &runs,
            false,
        ));
    }

    issues.sort();
    issues.dedup();
    Ok(issues)
}

/// Validate the register structure alone, before a candidate SHA exists.
///
/// This is a strict pass-through to
/// [`validate_register_structure`](crate::completion::structure::validate_register_structure):
/// no issue is filtered, reordered, deduplicated, downgraded or swallowed
/// here, and no lenient or partial-register mode exists. Operational
/// load/IO/JSON failures are returned as Err; invariant failures are returned
/// as the validator's own sorted issue list.
///
/// It is a narrower entry point than [`validate_completion`], never a weaker
/// one: no evidence, metadata-link, defect, dependency or release-coverage
/// validator is reachable from it. It establishes no EvidenceRun, no
/// acceptance status and no implementation status, so a clean result is a
/// structural lint and never a completion or acceptance verdict.
pub fn validate_completion_register(root: &Path) -> Result<Vec<String>, String> {
    validate_register_structure(root)
}

/// Run the register-only CLI command and return its process exit code.
pub fn run_verify_completion_register_command(root: &Path) -> i32 {
    let counts = match completion_status_counts(root) {
        Ok(counts) => counts,
        Err(error) => {
            eprintln!("verify-completion-register failed: {error}");
            return 1;
        }
    };
    let issues = match validate_completion_register(root) {
        Ok(issues) => issues,
        Err(error) => {
            eprintln!("verify-completion-register failed: {error}");
            return 1;
        }
    };

    // Ordering matters, and mirrors run_verify_completion_command statement for
    // statement: nothing is written to stdout until the register has loaded and
    // the validator has returned Ok. An operational failure - a missing or
    // malformed register - must emit its stderr line and exit nonzero with no
    // stdout at all, so no reader can scrape progress numbers off a register
    // that never loaded.
    println!("implementation status counts:");
    print_counts(&counts.implementation);
    println!("acceptance status counts:");
    print_counts(&counts.acceptance);
    if issues.is_empty() {
        println!(
            "verify-completion-register passed: register structure only, no evidence or acceptance was assessed"
        );
        0
    } else {
        eprintln!(
            "verify-completion-register found {} structural issue(s):",
            issues.len()
        );
        for issue in issues {
            eprintln!("- {issue}");
        }
        1
    }
}

/// Count implementation and acceptance statuses independently.
pub fn completion_status_counts(root: &Path) -> Result<CompletionStatusCounts, String> {
    let requirements: RequirementsDocument = load_json(root, "plans/completion/requirements.json")?;
    let mut counts = CompletionStatusCounts::default();
    for requirement in requirements.requirements {
        *counts
            .implementation
            .entry(requirement.implementation.to_string())
            .or_default() += 1;
        *counts
            .acceptance
            .entry(requirement.acceptance.to_string())
            .or_default() += 1;
    }
    Ok(counts)
}

/// Run the CLI command and return its process exit code.
pub fn run_verify_completion_command(root: &Path, candidate: &str, release: bool) -> i32 {
    let counts = match completion_status_counts(root) {
        Ok(counts) => counts,
        Err(error) => {
            eprintln!("verify-completion failed: {error}");
            return 1;
        }
    };
    let issues = match validate_completion(root, candidate, release) {
        Ok(issues) => issues,
        Err(error) => {
            eprintln!("verify-completion failed: {error}");
            return 1;
        }
    };

    println!("implementation status counts:");
    print_counts(&counts.implementation);
    println!("acceptance status counts:");
    print_counts(&counts.acceptance);
    if issues.is_empty() {
        println!(
            "verify-completion passed in {} mode",
            if release { "release" } else { "development" }
        );
        0
    } else {
        eprintln!("verify-completion found {} violation(s):", issues.len());
        for issue in issues {
            eprintln!("- {issue}");
        }
        1
    }
}

fn print_counts(counts: &BTreeMap<String, usize>) {
    for (status, count) in counts {
        println!("  {status}: {count}");
    }
}

fn validate_defects(
    defects: &[Defect],
    requirements: &RequirementsDocument,
    runs: &[EvidenceRun],
    candidate: &str,
    release: bool,
) -> Vec<String> {
    let mut issues = Vec::new();
    for defect in defects {
        if release && defect.invalidates_required_outcome && defect.status != DefectStatus::Closed {
            issues.push(format!(
                "release has unresolved defect {} invalidating a required outcome",
                defect.id
            ));
        }
        if defect.status != DefectStatus::Closed {
            continue;
        }
        for verification_id in &defect.verification_run_ids {
            let Some(run) = runs.iter().find(|run| run.id == *verification_id) else {
                issues.push(format!(
                    "closed defect {} verification run {} is absent from the selected candidate",
                    defect.id, verification_id
                ));
                continue;
            };
            if run.candidate_sha != candidate {
                issues.push(format!(
                    "closed defect {} verification run {} has a different candidate",
                    defect.id, verification_id
                ));
            }
            if run.result != EvidenceResult::Passed {
                issues.push(format!(
                    "closed defect {} verification run {} did not pass",
                    defect.id, verification_id
                ));
            }
            let product_linked = defect.requirement_ids.iter().any(|requirement_id| {
                requirements.requirements.iter().any(|requirement| {
                    requirement.id == *requirement_id
                        && requirement.kind == RequirementKind::Product
                })
            });
            let eligible = if product_linked {
                qualifies_run(run)
            } else {
                qualifies_internal_safety_run(run)
            };
            if !eligible {
                issues.push(format!(
                    "closed defect {} verification run {} is not eligible linked evidence",
                    defect.id, verification_id
                ));
            }
            if run.scenario_id != defect.scenario_id
                || run.configuration_id != defect.configuration_id
            {
                issues.push(format!(
                    "closed defect {} verification run {} does not match its scenario/configuration",
                    defect.id, verification_id
                ));
            }
        }
    }
    issues
}

fn validate_release_dependencies(
    dependencies: &DependenciesDocument,
    requirements: &RequirementsDocument,
    scenarios: &ScenariosDocument,
    runs: &[EvidenceRun],
) -> Vec<String> {
    let mut issues = Vec::new();
    for package in &dependencies.packages {
        for prerequisite in &package.external_prerequisites {
            let package_requirements: Vec<_> = requirements
                .requirements
                .iter()
                .filter(|requirement| {
                    requirement.required
                        && requirement.package_id == package.package_id
                        && package
                            .requirement_ids
                            .iter()
                            .any(|id| id == &requirement.id)
                })
                .collect();
            let applicable_configurations: BTreeSet<_> = package_requirements
                .iter()
                .flat_map(|requirement| {
                    requirement
                        .configuration_ids
                        .iter()
                        .filter(|configuration_id| {
                            requirement.scenario_ids.iter().any(|scenario_id| {
                                scenarios.scenarios.iter().any(|scenario| {
                                    scenario.id == *scenario_id
                                        && scenario.configuration_ids.contains(configuration_id)
                                })
                            })
                        })
                })
                .cloned()
                .collect();
            if applicable_configurations.is_empty() {
                issues.push(format!(
                    "required external prerequisite {} has no applicable required package requirement coverage for package {}",
                    prerequisite, package.package_id
                ));
                continue;
            }
            for configuration_id in applicable_configurations {
                let fulfilled = runs.iter().any(|run| {
                    run.configuration_id == configuration_id
                        && run.result == EvidenceResult::Passed
                        && run.dependencies.iter().any(|dependency| {
                            dependency.name == *prerequisite
                                && dependency.execution == DependencyExecution::Real
                                && dependency.required_for_outcome
                                && !dependency.version.trim().is_empty()
                        })
                        && package_requirements.iter().any(|requirement| {
                            requirement.configuration_ids.contains(&configuration_id)
                                && requirement.scenario_ids.contains(&run.scenario_id)
                                && match requirement.kind {
                                    RequirementKind::Product => qualifies_run(run),
                                    RequirementKind::Internal => qualifies_internal_safety_run(run),
                                }
                        })
                });
                if !fulfilled {
                    issues.push(format!(
                        "required external prerequisite {} lacks real selected-candidate fulfillment for package {} configuration {}",
                        prerequisite, package.package_id, configuration_id
                    ));
                }
            }
        }
    }
    issues
}

fn validate_release_requirement_coverage(
    requirements: &RequirementsDocument,
    _matrix: &MatrixDocument,
    scenarios: &ScenariosDocument,
    runs: &[EvidenceRun],
    release: bool,
) -> Vec<String> {
    let mut issues = Vec::new();
    for requirement in &requirements.requirements {
        if release
            && requirement.kind == RequirementKind::Product
            && requirement.required
            && requirement.acceptance == Acceptance::Accepted
        {
            for configuration_id in &requirement.configuration_ids {
                let mapped = requirement.scenario_ids.iter().any(|scenario_id| {
                    scenarios.scenarios.iter().any(|scenario| {
                        scenario.id == *scenario_id
                            && scenario.configuration_ids.contains(configuration_id)
                    })
                });
                if !mapped {
                    issues.push(format!(
                        "required product requirement {} has no scenario coverage for configuration {}",
                        requirement.id, configuration_id
                    ));
                }
            }
        }
        if requirement.kind == RequirementKind::Internal
            && requirement.acceptance == Acceptance::Accepted
            && requirement.protected_product_ids.is_empty()
        {
            issues.push(format!(
                "accepted internal requirement {} has no protected product linkage",
                requirement.id
            ));
        }
        if requirement.kind == RequirementKind::Internal
            && (requirement.acceptance == Acceptance::Accepted || (release && requirement.required))
        {
            if release && requirement.required && requirement.acceptance != Acceptance::Accepted {
                issues.push(format!(
                    "required internal requirement {} is not accepted",
                    requirement.id
                ));
            }
            if release && requirement.required && requirement.protected_product_ids.is_empty() {
                issues.push(format!(
                    "required internal requirement {} has no protected product linkage",
                    requirement.id
                ));
            }
            let mut applicable = false;
            for scenario_id in &requirement.scenario_ids {
                let Some(scenario) = scenarios.scenarios.iter().find(|s| s.id == *scenario_id)
                else {
                    continue;
                };
                for configuration_id in &requirement.configuration_ids {
                    if !scenario.configuration_ids.contains(configuration_id) {
                        continue;
                    }
                    applicable = true;
                    let covered = runs.iter().any(|run| {
                        run.scenario_id == *scenario_id
                            && run.configuration_id == *configuration_id
                            && run.result == EvidenceResult::Passed
                            && qualifies_internal_safety_run(run)
                    });
                    if !covered {
                        issues.push(format!(
                            "internal requirement {} lacks eligible safety evidence for scenario {} configuration {}",
                            requirement.id, scenario_id, configuration_id
                        ));
                    }
                }
            }
            if !applicable {
                issues.push(format!(
                    "internal requirement {} has no applicable scenario/configuration pair",
                    requirement.id
                ));
            }
        }
    }
    issues
}

fn qualifies_run(run: &EvidenceRun) -> bool {
    crate::completion::qualifies_as_product_evidence(
        &run.layer.to_string(),
        &run.input_route.to_string(),
        &run.result.to_string(),
        run.dependencies.iter().any(|dependency| {
            dependency.required_for_outcome
                && dependency.execution == DependencyExecution::Substituted
        }),
    )
}

fn qualifies_internal_safety_run(run: &EvidenceRun) -> bool {
    run.layer.to_string() != "product"
        && run.result == EvidenceResult::Passed
        && !run.oracle_results.is_empty()
        && !run.recovery_results.is_empty()
        && !run.dependencies.iter().any(|dependency| {
            dependency.required_for_outcome
                && dependency.execution == DependencyExecution::Substituted
        })
}

fn load_json<T: DeserializeOwned>(root: &Path, relative: &str) -> Result<T, String> {
    let path = root.join(relative);
    let bytes = fs::read(&path).map_err(|error| format!("{relative}: {error}"))?;
    serde_json::from_slice(&bytes).map_err(|error| format!("{relative}: {error}"))
}
