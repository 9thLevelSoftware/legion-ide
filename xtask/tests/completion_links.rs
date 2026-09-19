use std::collections::BTreeMap;

use xtask::completion::{
    links::validate_metadata_links,
    schema::{
        CheckResult, Configuration, DependencyExecution, DependencyObservation, EvidenceLayer,
        EvidenceResult, EvidenceRun, InputRoute, MatrixDocument, OperatingSystem, Requirement,
        RequirementKind, RequirementsDocument, Scenario, ScenarioCheck, ScenariosDocument, Stage,
    },
};

fn requirement(
    acceptance: &str,
    required: bool,
    scenarios: &[&str],
    configs: &[&str],
) -> Requirement {
    Requirement {
        id: "REQ-1".into(),
        title: "Product outcome".into(),
        kind: RequirementKind::Product,
        required,
        source_refs: Vec::new(),
        legacy_ids: Vec::new(),
        stage: Stage::S1,
        package_id: "S1-01".into(),
        owner_role: "owner".into(),
        depends_on: Vec::new(),
        implementation: xtask::completion::schema::Implementation::Implemented,
        acceptance: match acceptance {
            "accepted" => xtask::completion::schema::Acceptance::Accepted,
            _ => xtask::completion::schema::Acceptance::Unassessed,
        },
        scenario_ids: scenarios.iter().map(|id| (*id).into()).collect(),
        configuration_ids: configs.iter().map(|id| (*id).into()).collect(),
        protected_product_ids: Vec::new(),
        defect_ids: Vec::new(),
    }
}

fn scenario(requirement_ids: &[&str], configs: &[&str]) -> Scenario {
    Scenario {
        id: "SCN-1".into(),
        requirement_ids: requirement_ids.iter().map(|id| (*id).into()).collect(),
        configuration_ids: configs.iter().map(|id| (*id).into()).collect(),
        steps: vec!["perform workflow".into()],
        external_oracles: vec![ScenarioCheck {
            id: "oracle".into(),
            description: "observe".into(),
        }],
        recovery_cases: vec![ScenarioCheck {
            id: "recovery".into(),
            description: "recover".into(),
        }],
        sensitive_artifact_policy: "metadata-only".into(),
    }
}

fn matrix(configs: &[&str]) -> MatrixDocument {
    matrix_with_approval(configs, "approval")
}

fn matrix_with_approval(configs: &[&str], owner_approval_ref: &str) -> MatrixDocument {
    MatrixDocument {
        schema_version: 1,
        configurations: configs
            .iter()
            .map(|id| Configuration {
                id: (*id).into(),
                os: OperatingSystem::Windows,
                architecture: "x64".into(),
                tool_versions: BTreeMap::from([("tool".into(), "1".into())]),
                hardware: "fixture".into(),
                project_category: "desktop".into(),
                required: true,
                owner_approval_ref: owner_approval_ref.into(),
            })
            .collect(),
    }
}

fn run(id: &str) -> EvidenceRun {
    EvidenceRun {
        schema_version: 1,
        id: id.into(),
        scenario_id: "SCN-1".into(),
        configuration_id: "CFG-1".into(),
        candidate_sha: "candidate-a".into(),
        artifact_sha256: "artifact".into(),
        layer: EvidenceLayer::Product,
        input_route: InputRoute::NativeInput,
        dependencies: vec![DependencyObservation {
            name: "tool".into(),
            version: "1".into(),
            execution: DependencyExecution::Real,
            required_for_outcome: true,
            substitution_reason: String::new(),
        }],
        result: EvidenceResult::Passed,
        oracle_results: vec![CheckResult {
            id: "oracle".into(),
            passed: true,
            artifact_path: "oracle.txt".into(),
            observed: "observed".into(),
        }],
        recovery_results: vec![CheckResult {
            id: "recovery".into(),
            passed: true,
            artifact_path: "recovery.txt".into(),
            observed: "recovered".into(),
        }],
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
        review_decision: xtask::completion::schema::ReviewDecision::Accepted,
        started_at_utc: "2026-01-01T00:00:00Z".into(),
        ended_at_utc: "2026-01-01T00:01:00Z".into(),
    }
}

fn docs(
    requirement: Requirement,
    scenario: Scenario,
    configs: &[&str],
) -> (RequirementsDocument, MatrixDocument, ScenariosDocument) {
    docs_with_approval(requirement, scenario, configs, "approval")
}

fn docs_with_approval(
    requirement: Requirement,
    scenario: Scenario,
    configs: &[&str],
    owner_approval_ref: &str,
) -> (RequirementsDocument, MatrixDocument, ScenariosDocument) {
    (
        RequirementsDocument {
            schema_version: 1,
            requirements: vec![requirement],
        },
        matrix_with_approval(configs, owner_approval_ref),
        ScenariosDocument {
            schema_version: 1,
            scenarios: vec![scenario],
        },
    )
}

/// The `owner_approval_ref` every configuration in
/// `plans/completion/matrix.json` carries today, written out literally so that
/// a reword of the register's marker fails a test instead of passing silently.
const REGISTER_OWNER_APPROVAL_REF_TODAY: &str = "plans/completion/decisions.md#2026-09-08-provisional-register-ratification (PROVISIONAL: agent-made, not owner-ratified)";

/// Issues raised by the release-mode provisional-ratification rejection only.
fn provisional_issues(issues: &[String]) -> Vec<&String> {
    issues
        .iter()
        .filter(|issue| issue.contains("provisional ratification marker"))
        .collect()
}

#[test]
fn valid_incomplete_development_register_is_allowed_without_evidence() {
    let (requirements, matrix, scenarios) = docs(
        requirement("unassessed", true, &[], &[]),
        scenario(&[], &[]),
        &["CFG-1"],
    );
    let issues = validate_metadata_links(
        &requirements,
        &matrix,
        &scenarios,
        &[],
        "candidate-a",
        false,
    );
    assert!(issues.is_empty(), "{issues:?}");
}

#[test]
fn release_rejects_required_unaccepted_rows_and_uncovered_matrix() {
    let (requirements, matrix, scenarios) = docs(
        requirement("unassessed", true, &["SCN-1"], &["CFG-1"]),
        scenario(&["REQ-1"], &["CFG-1"]),
        &["CFG-1"],
    );
    let issues =
        validate_metadata_links(&requirements, &matrix, &scenarios, &[], "candidate-a", true);
    assert!(
        issues
            .iter()
            .any(|issue| issue.contains("REQ-1") && issue.contains("not accepted"))
    );
    assert!(
        issues
            .iter()
            .any(|issue| issue.contains("CFG-1") && issue.contains("no eligible"))
    );
}

#[test]
fn accepted_product_requires_every_reciprocal_pair() {
    let (requirements, matrix, scenarios) = docs(
        requirement("accepted", true, &["SCN-1"], &["CFG-1"]),
        scenario(&["REQ-1"], &["CFG-1"]),
        &["CFG-1"],
    );
    let issues = validate_metadata_links(
        &requirements,
        &matrix,
        &scenarios,
        &[],
        "candidate-a",
        false,
    );
    assert!(issues.iter().any(|issue| issue.contains("REQ-1")
        && issue.contains("SCN-1")
        && issue.contains("CFG-1")));

    let issues = validate_metadata_links(
        &requirements,
        &matrix,
        &scenarios,
        &[run("run-1")],
        "candidate-a",
        false,
    );
    assert!(issues.is_empty(), "{issues:?}");
}

#[test]
fn passed_run_with_failed_required_oracle_is_rejected() {
    let (requirements, matrix, scenarios) = docs(
        requirement("accepted", true, &["SCN-1"], &["CFG-1"]),
        scenario(&["REQ-1"], &["CFG-1"]),
        &["CFG-1"],
    );
    let mut evidence = run("run-forged-passed");
    evidence.oracle_results = vec![CheckResult {
        id: "oracle".into(),
        passed: false,
        artifact_path: "oracle.txt".into(),
        observed: "external effect failed".into(),
    }];
    evidence.recovery_results = vec![CheckResult {
        id: "recovery".into(),
        passed: true,
        artifact_path: "recovery.txt".into(),
        observed: "recovered".into(),
    }];
    evidence.artifact_hashes = BTreeMap::from([
        (
            "oracle.txt".into(),
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".into(),
        ),
        (
            "recovery.txt".into(),
            "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb".into(),
        ),
    ]);
    let issues = validate_metadata_links(
        &requirements,
        &matrix,
        &scenarios,
        &[evidence],
        "candidate-a",
        false,
    );
    assert!(
        issues
            .iter()
            .any(|issue| issue.contains("oracle") && issue.contains("passed")),
        "forged passed run was accepted: {issues:?}"
    );
}

#[test]
fn accepted_product_uses_applicable_intersection_across_split_scenarios() {
    let requirement = requirement("accepted", true, &["SCN-1", "SCN-2"], &["CFG-1", "CFG-2"]);
    let scenario_one = scenario(&["REQ-1"], &["CFG-1"]);
    let mut scenario_two = scenario(&["REQ-1"], &["CFG-2"]);
    scenario_two.id = "SCN-2".into();
    let requirements = RequirementsDocument {
        schema_version: 1,
        requirements: vec![requirement],
    };
    let matrix = matrix(&["CFG-1", "CFG-2"]);
    let scenarios = ScenariosDocument {
        schema_version: 1,
        scenarios: vec![scenario_one, scenario_two],
    };
    let mut run_two = run("run-2");
    run_two.scenario_id = "SCN-2".into();
    run_two.configuration_id = "CFG-2".into();
    let issues = validate_metadata_links(
        &requirements,
        &matrix,
        &scenarios,
        &[run("run-1"), run_two],
        "candidate-a",
        false,
    );
    assert!(issues.is_empty(), "{issues:?}");

    let issues = validate_metadata_links(
        &requirements,
        &matrix,
        &scenarios,
        &[run("run-1")],
        "candidate-a",
        false,
    );
    assert!(issues.iter().any(|issue| {
        issue.contains("REQ-1") && issue.contains("SCN-2") && issue.contains("CFG-2")
    }));
}

#[test]
fn candidate_identity_is_case_sensitive() {
    let (requirements, matrix, scenarios) = docs(
        requirement("accepted", true, &["SCN-1"], &["CFG-1"]),
        scenario(&["REQ-1"], &["CFG-1"]),
        &["CFG-1"],
    );
    let mut evidence = run("run-case");
    evidence.candidate_sha = "Candidate-A".into();
    let issues = validate_metadata_links(
        &requirements,
        &matrix,
        &scenarios,
        &[evidence],
        "candidate-a",
        false,
    );
    assert!(
        issues
            .iter()
            .any(|issue| issue.contains("does not match selected candidate"))
    );
    assert!(issues.iter().any(|issue| issue.contains("lacks eligible")));
}

#[test]
fn failed_run_does_not_cancel_a_passed_run_for_same_candidate_pair() {
    let (requirements, matrix, scenarios) = docs(
        requirement("accepted", true, &["SCN-1"], &["CFG-1"]),
        scenario(&["REQ-1"], &["CFG-1"]),
        &["CFG-1"],
    );
    let mut failed = run("run-failed");
    failed.result = EvidenceResult::Failed;
    assert!(
        validate_metadata_links(
            &requirements,
            &matrix,
            &scenarios,
            &[failed, run("run-passed")],
            "candidate-a",
            false,
        )
        .is_empty()
    );
}

#[test]
fn unrelated_requirement_run_cannot_certify_target_requirement() {
    let target = requirement("accepted", true, &["SCN-1"], &["CFG-1"]);
    let mut unrelated = requirement("accepted", true, &["SCN-2"], &["CFG-2"]);
    unrelated.id = "REQ-2".into();
    let requirements = RequirementsDocument {
        schema_version: 1,
        requirements: vec![target, unrelated],
    };
    let matrix = matrix(&["CFG-1", "CFG-2"]);
    let mut target_scenario = scenario(&["REQ-1"], &["CFG-1"]);
    let mut unrelated_scenario = scenario(&["REQ-2"], &["CFG-2"]);
    unrelated_scenario.id = "SCN-2".into();
    target_scenario.id = "SCN-1".into();
    let scenarios = ScenariosDocument {
        schema_version: 1,
        scenarios: vec![target_scenario, unrelated_scenario],
    };
    let mut evidence = run("run-unrelated");
    evidence.scenario_id = "SCN-2".into();
    evidence.configuration_id = "CFG-2".into();
    let issues = validate_metadata_links(
        &requirements,
        &matrix,
        &scenarios,
        &[evidence],
        "candidate-a",
        false,
    );
    assert!(issues.iter().any(|issue| {
        issue.contains("REQ-1") && issue.contains("SCN-1") && issue.contains("CFG-1")
    }));
    assert!(
        !issues
            .iter()
            .any(|issue| issue.contains("unknown scenario")
                || issue.contains("unknown configuration"))
    );
}

#[test]
fn empty_candidate_is_rejected_even_without_evidence() {
    let (requirements, matrix, scenarios) = docs(
        requirement("unassessed", true, &[], &[]),
        scenario(&[], &[]),
        &["CFG-1"],
    );
    let issues = validate_metadata_links(&requirements, &matrix, &scenarios, &[], "", false);
    assert_eq!(issues, vec!["candidate is empty"]);
}

#[test]
fn release_does_not_require_acceptance_for_required_internal_rows() {
    let product = requirement("accepted", true, &["SCN-1"], &["CFG-1"]);
    let mut internal = requirement("unassessed", true, &[], &[]);
    internal.id = "REQ-INTERNAL".into();
    internal.kind = RequirementKind::Internal;
    let requirements = RequirementsDocument {
        schema_version: 1,
        requirements: vec![product, internal],
    };
    let matrix = matrix(&["CFG-1"]);
    let scenarios = ScenariosDocument {
        schema_version: 1,
        scenarios: vec![scenario(&["REQ-1"], &["CFG-1"])],
    };
    let issues = validate_metadata_links(
        &requirements,
        &matrix,
        &scenarios,
        &[run("run-product")],
        "candidate-a",
        true,
    );
    assert!(issues.is_empty(), "{issues:?}");
}

#[test]
fn unknown_requirement_scope_is_reported_from_a_known_scenario() {
    let (requirements, matrix, mut scenarios) = docs(
        requirement("unassessed", true, &[], &[]),
        scenario(&["REQ-UNKNOWN"], &["CFG-1"]),
        &["CFG-1"],
    );
    scenarios.scenarios[0].id = "SCN-1".into();
    let issues = validate_metadata_links(
        &requirements,
        &matrix,
        &scenarios,
        &[run("run-unknown-requirement")],
        "candidate-a",
        false,
    );
    assert!(
        issues
            .iter()
            .any(|issue| issue.contains("unknown requirement scope"))
    );
    assert!(
        !issues
            .iter()
            .any(|issue| issue.contains("unknown scenario"))
    );
}

#[test]
fn only_product_native_passed_real_required_runs_qualify() {
    for (layer, route, result) in [
        (
            EvidenceLayer::Component,
            InputRoute::NativeInput,
            EvidenceResult::Passed,
        ),
        (
            EvidenceLayer::Product,
            InputRoute::RuntimeDispatch,
            EvidenceResult::Passed,
        ),
        (
            EvidenceLayer::Product,
            InputRoute::NativeInput,
            EvidenceResult::Failed,
        ),
        (
            EvidenceLayer::Product,
            InputRoute::NativeInput,
            EvidenceResult::Skipped,
        ),
        (
            EvidenceLayer::Product,
            InputRoute::NativeInput,
            EvidenceResult::Blocked,
        ),
    ] {
        let (requirements, matrix, scenarios) = docs(
            requirement("accepted", true, &["SCN-1"], &["CFG-1"]),
            scenario(&["REQ-1"], &["CFG-1"]),
            &["CFG-1"],
        );
        let mut evidence = run("run-1");
        evidence.layer = layer;
        evidence.input_route = route;
        evidence.result = result;
        let issues = validate_metadata_links(
            &requirements,
            &matrix,
            &scenarios,
            &[evidence],
            "candidate-a",
            false,
        );
        assert!(
            issues
                .iter()
                .any(|issue| issue.contains("REQ-1") && issue.contains("lacks eligible"))
        );
    }
    let (requirements, matrix, scenarios) = docs(
        requirement("accepted", true, &["SCN-1"], &["CFG-1"]),
        scenario(&["REQ-1"], &["CFG-1"]),
        &["CFG-1"],
    );
    let mut substituted = run("run-optional");
    substituted.dependencies[0].required_for_outcome = false;
    substituted.dependencies[0].execution = DependencyExecution::Substituted;
    assert!(
        validate_metadata_links(
            &requirements,
            &matrix,
            &scenarios,
            &[substituted],
            "candidate-a",
            false
        )
        .is_empty()
    );
    let mut substituted = run("run-required");
    substituted.dependencies[0].execution = DependencyExecution::Substituted;
    assert!(
        validate_metadata_links(
            &requirements,
            &matrix,
            &scenarios,
            &[substituted],
            "candidate-a",
            false
        )
        .iter()
        .any(|issue| issue.contains("lacks eligible"))
    );
}

#[test]
fn bad_run_metadata_is_reported_even_without_accepted_rows() {
    let (requirements, matrix, scenarios) = docs(
        requirement("unassessed", true, &[], &[]),
        scenario(&[], &[]),
        &["CFG-1"],
    );
    let mut empty_id = run("run-empty");
    empty_id.id.clear();
    empty_id.scenario_id = "UNKNOWN".into();
    empty_id.configuration_id = "UNKNOWN-CFG".into();
    empty_id.candidate_sha = "other".into();
    let duplicate = run("run-duplicate");
    let duplicate_copy = duplicate.clone();
    let mut unknown_config = run("run-unknown-config");
    unknown_config.configuration_id = "UNKNOWN-CFG".into();
    let issues = validate_metadata_links(
        &requirements,
        &matrix,
        &scenarios,
        &[empty_id, duplicate, duplicate_copy, unknown_config],
        "candidate-a",
        false,
    );
    assert!(issues.iter().any(|issue| issue.contains("id is empty")));
    assert!(
        issues
            .iter()
            .any(|issue| issue.contains("duplicate evidence run id `run-duplicate`"))
    );
    assert!(
        issues
            .iter()
            .any(|issue| issue.contains("candidate") && issue.contains("does not match"))
    );
    assert!(
        issues
            .iter()
            .any(|issue| issue.contains("unknown scenario"))
    );
    assert!(
        issues
            .iter()
            .any(|issue| issue.contains("unknown configuration"))
    );
}

#[test]
fn out_of_scope_and_nonreciprocal_metadata_cannot_satisfy_row() {
    let (requirements, matrix, scenarios) = docs(
        requirement("accepted", true, &["SCN-1"], &["CFG-1"]),
        scenario(&[], &["CFG-1"]),
        &["CFG-1"],
    );
    let issues = validate_metadata_links(
        &requirements,
        &matrix,
        &scenarios,
        &[run("run-1")],
        "candidate-a",
        false,
    );
    assert!(issues.iter().any(|issue| issue.contains("not reciprocal")));
    assert!(
        issues
            .iter()
            .any(|issue| issue.contains("no applicable scenario/configuration pair"))
    );

    let (requirements, matrix, scenarios) = docs(
        requirement("accepted", true, &["SCN-1"], &["CFG-1"]),
        scenario(&["REQ-1"], &["CFG-2"]),
        &["CFG-1", "CFG-2"],
    );
    let issues = validate_metadata_links(
        &requirements,
        &matrix,
        &scenarios,
        &[run("run-1")],
        "candidate-a",
        false,
    );
    assert!(
        issues
            .iter()
            .any(|issue| issue.contains("outside scenario"))
    );
    assert!(
        issues
            .iter()
            .any(|issue| issue.contains("no applicable scenario/configuration pair"))
    );
}

#[test]
fn release_rejects_a_configuration_whose_owner_approval_is_marked_provisional() {
    let (requirements, matrix, scenarios) = docs_with_approval(
        requirement("unassessed", true, &[], &[]),
        scenario(&[], &[]),
        &["CFG-1"],
        "plans/completion/decisions.md#ratification (provisional)",
    );
    let issues =
        validate_metadata_links(&requirements, &matrix, &scenarios, &[], "candidate-a", true);
    let flagged = provisional_issues(&issues);
    assert_eq!(flagged.len(), 1, "{issues:?}");
    assert!(flagged[0].contains("CFG-1"), "{flagged:?}");
    assert!(flagged[0].contains("`provisional`"), "{flagged:?}");
}

#[test]
fn release_rejects_a_configuration_whose_owner_approval_is_marked_agent_made() {
    let (requirements, matrix, scenarios) = docs_with_approval(
        requirement("unassessed", true, &[], &[]),
        scenario(&[], &[]),
        &["CFG-1"],
        "plans/completion/decisions.md#ratification (agent-made, not owner-ratified)",
    );
    let issues =
        validate_metadata_links(&requirements, &matrix, &scenarios, &[], "candidate-a", true);
    let flagged = provisional_issues(&issues);
    assert_eq!(flagged.len(), 1, "{issues:?}");
    assert!(flagged[0].contains("CFG-1"), "{flagged:?}");
    assert!(flagged[0].contains("`agent-made`"), "{flagged:?}");
}

#[test]
fn release_rejects_the_exact_marker_string_the_register_uses_today() {
    let (requirements, matrix, scenarios) = docs_with_approval(
        requirement("unassessed", true, &[], &[]),
        scenario(&[], &[]),
        &["CFG-1"],
        REGISTER_OWNER_APPROVAL_REF_TODAY,
    );
    let issues =
        validate_metadata_links(&requirements, &matrix, &scenarios, &[], "candidate-a", true);
    let flagged = provisional_issues(&issues);
    assert_eq!(flagged.len(), 1, "{issues:?}");
    assert!(flagged[0].contains("CFG-1"), "{flagged:?}");
    assert!(flagged[0].contains("`provisional`"), "{flagged:?}");

    let repeated =
        validate_metadata_links(&requirements, &matrix, &scenarios, &[], "candidate-a", true);
    assert_eq!(issues, repeated, "release issues are not deterministic");
}

#[test]
fn release_reports_every_provisional_configuration_not_only_the_first() {
    let (requirements, matrix, scenarios) = docs_with_approval(
        requirement("unassessed", true, &[], &[]),
        scenario(&[], &[]),
        &["CFG-1", "CFG-2"],
        REGISTER_OWNER_APPROVAL_REF_TODAY,
    );
    let issues =
        validate_metadata_links(&requirements, &matrix, &scenarios, &[], "candidate-a", true);
    let flagged = provisional_issues(&issues);
    assert_eq!(flagged.len(), 2, "{issues:?}");
    assert!(flagged.iter().any(|issue| issue.contains("CFG-1")));
    assert!(flagged.iter().any(|issue| issue.contains("CFG-2")));
    assert!(
        issues.windows(2).all(|pair| pair[0] <= pair[1]),
        "issues are not sorted: {issues:?}"
    );
}

#[test]
fn release_accepts_an_owner_approval_reference_with_no_provisional_marker() {
    let (requirements, matrix, scenarios) = docs_with_approval(
        requirement("accepted", true, &["SCN-1"], &["CFG-1"]),
        scenario(&["REQ-1"], &["CFG-1"]),
        &["CFG-1"],
        "plans/completion/decisions.md#2026-09-08-owner-ratification",
    );
    let issues = validate_metadata_links(
        &requirements,
        &matrix,
        &scenarios,
        &[run("run-product")],
        "candidate-a",
        true,
    );
    assert!(issues.is_empty(), "{issues:?}");
}

#[test]
fn provisional_marker_detection_is_case_insensitive() {
    for (owner_approval_ref, quoted) in [
        ("decisions.md#x (PROVISIONAL)", "`PROVISIONAL`"),
        ("decisions.md#x (Provisional)", "`Provisional`"),
        ("decisions.md#x (Agent-Made)", "`Agent-Made`"),
    ] {
        let (requirements, matrix, scenarios) = docs_with_approval(
            requirement("unassessed", true, &[], &[]),
            scenario(&[], &[]),
            &["CFG-1"],
            owner_approval_ref,
        );
        let issues =
            validate_metadata_links(&requirements, &matrix, &scenarios, &[], "candidate-a", true);
        let flagged = provisional_issues(&issues);
        assert_eq!(flagged.len(), 1, "{owner_approval_ref}: {issues:?}");
        assert!(flagged[0].contains("CFG-1"), "{flagged:?}");
        assert!(flagged[0].contains(quoted), "{flagged:?}");
    }
}

#[test]
fn development_mode_does_not_reject_a_provisional_configuration() {
    let (requirements, matrix, scenarios) = docs_with_approval(
        requirement("unassessed", true, &[], &[]),
        scenario(&[], &[]),
        &["CFG-1"],
        REGISTER_OWNER_APPROVAL_REF_TODAY,
    );
    let issues = validate_metadata_links(
        &requirements,
        &matrix,
        &scenarios,
        &[],
        "candidate-a",
        false,
    );
    assert!(issues.is_empty(), "{issues:?}");
}
