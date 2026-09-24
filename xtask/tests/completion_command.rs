use std::collections::BTreeSet;
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::json;
use xtask::completion::structure::validate_register_structure;
use xtask::completion_command::{
    completion_status_counts, run_verify_completion_command,
    run_verify_completion_register_command, validate_completion, validate_completion_register,
};

mod evidence_fixture {
    include!("completion_evidence.rs");

    pub fn complete_testdata() -> (std::path::PathBuf, std::path::PathBuf) {
        let fixture = base_fixture();
        std::fs::create_dir_all(fixture.root.join("docs")).unwrap();
        std::fs::write(fixture.root.join("docs/spec.md"), "spec").unwrap();
        std::fs::create_dir_all(fixture.root.join("plans/kanban")).unwrap();
        std::fs::write(
            fixture.root.join("plans/kanban/legion-ga-backlog.toml"),
            r#"
[meta]
plan = "completion"
milestone = "S0"
[[epics]]
id = "E1"
title = "Epic"
milestone = "S0"
[[epics.features]]
id = "F1"
title = "Feature"
[[epics.features.tasks]]
id = "LEG-1"
title = "Task"
mode = "Manual"
readiness_row = "REQ-1"
files = ["docs/spec.md"]
dependencies = []
verification = ["check"]
acceptance = ["done"]
stop_condition = "stop"
status = "todo"
"#,
        )
        .unwrap();
        let completion = fixture.root.join("plans/completion");
        let mut requirements: serde_json::Value =
            serde_json::from_slice(&std::fs::read(completion.join("requirements.json")).unwrap())
                .unwrap();
        requirements["requirements"][0]["acceptance"] = "accepted".into();
        requirements["requirements"][0]["legacy_ids"] = serde_json::json!(["LEG-1"]);
        std::fs::write(
            completion.join("requirements.json"),
            serde_json::to_vec_pretty(&requirements).unwrap(),
        )
        .unwrap();
        let dependencies = serde_json::json!({
            "schema_version": 1,
            "packages": [{
                "package_id": "PKG-1",
                "requirement_ids": ["REQ-1"],
                "milestones": [
                    {"id":"PKG-1:implemented","depends_on":[],"deliverable_refs":["src"]},
                    {"id":"PKG-1:accepted","depends_on":["PKG-1:implemented"],"deliverable_refs":["artifact"]}
                ],
                "external_prerequisites": ["runtime"], "owner_role":"owner",
                "implementation_stage":"S0", "acceptance_stage":"S0"
            }]
        });
        std::fs::write(
            completion.join("dependencies.json"),
            serde_json::to_vec_pretty(&dependencies).unwrap(),
        )
        .unwrap();
        std::fs::write(
            completion.join("defects.json"),
            serde_json::to_vec_pretty(&serde_json::json!({"schema_version":1,"defects":[]}))
                .unwrap(),
        )
        .unwrap();
        let mut run: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&fixture.run_path).unwrap()).unwrap();
        run["dependencies"] = serde_json::json!([{"name":"runtime","version":"1.0","execution":"real","required_for_outcome":true,"substitution_reason":""}]);
        std::fs::write(&fixture.run_path, serde_json::to_vec_pretty(&run).unwrap()).unwrap();
        (fixture.root, fixture.run_path)
    }
}

fn temp_root() -> PathBuf {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let pid = std::process::id();
    let seq = COUNTER.fetch_add(1, Ordering::Relaxed);
    let root = std::env::temp_dir().join(format!("legion-completion-command-{pid}-{stamp}-{seq}"));
    fs::create_dir_all(&root).expect("temporary root");
    root
}

#[test]
fn counts_implementation_and_acceptance_independently() {
    let root = temp_root();
    fs::create_dir_all(root.join("plans/completion")).unwrap();
    fs::write(
        root.join("plans/completion/requirements.json"),
        serde_json::to_vec(&json!({
            "schema_version": 1,
            "requirements": [{
                "id": "R-1", "title": "One", "kind": "product", "required": true,
                "source_refs": [], "legacy_ids": [], "stage": "S0", "package_id": "P-1",
                "owner_role": "owner", "depends_on": [], "implementation": "partial",
                "acceptance": "unassessed", "scenario_ids": [], "configuration_ids": [],
                "protected_product_ids": [], "defect_ids": []
            }]
        }))
        .unwrap(),
    )
    .unwrap();
    let counts = completion_status_counts(&root).unwrap();
    assert_eq!(counts.implementation.get("partial"), Some(&1));
    assert_eq!(counts.acceptance.get("unassessed"), Some(&1));
    let _ = fs::remove_dir_all(root);
}

#[test]
fn missing_registers_are_operational_failure_and_command_nonzero() {
    let root = temp_root();
    let error =
        validate_completion(&root, "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa", false).unwrap_err();
    assert!(error.contains("requirements.json"));
    assert_eq!(
        run_verify_completion_command(&root, "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa", false,),
        1
    );
    let _ = fs::remove_dir_all(root);
}

fn complete_fixture() -> (PathBuf, PathBuf) {
    evidence_fixture::complete_testdata()
}

fn read_json(path: &std::path::Path) -> serde_json::Value {
    serde_json::from_slice(&fs::read(path).unwrap()).unwrap()
}

fn write_json(path: &std::path::Path, value: &serde_json::Value) {
    fs::write(path, serde_json::to_vec_pretty(value).unwrap()).unwrap();
}

#[test]
fn complete_testdata_passes_development_and_release() {
    let (root, _) = complete_fixture();
    let candidate = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    let dev_issues = validate_completion(&root, candidate, false).unwrap();
    assert!(dev_issues.is_empty());
    assert!(
        validate_completion(&root, candidate, true)
            .unwrap()
            .is_empty()
    );
    assert_eq!(run_verify_completion_command(&root, candidate, true), 0);
    let _ = fs::remove_dir_all(root);
}

#[test]
fn cli_subprocess_accepts_populated_development_and_release_fixture() {
    let (root, _) = complete_fixture();
    let executable = std::env::var("CARGO_BIN_EXE_xtask").expect("xtask binary path");
    for args in [
        &[
            "verify-completion",
            "--candidate",
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        ][..],
        &[
            "verify-completion",
            "--candidate",
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            "--release",
        ][..],
    ] {
        let output = Command::new(&executable)
            .args(args)
            .arg("--root")
            .arg(&root)
            .output()
            .expect("run populated verify-completion");
        assert!(
            output.status.success(),
            "status {:?}, stderr {}",
            output.status,
            String::from_utf8_lossy(&output.stderr)
        );
    }
    let cwd_output = Command::new(&executable)
        .current_dir(&root)
        .args([
            "verify-completion",
            "--candidate",
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        ])
        .output()
        .expect("run verify-completion with fixture cwd");
    assert!(
        cwd_output.status.success(),
        "cwd status {:?}, stderr {}",
        cwd_output.status,
        String::from_utf8_lossy(&cwd_output.stderr)
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn command_rejects_empty_candidate_argument() {
    let (root, _) = complete_fixture();
    assert_eq!(run_verify_completion_command(&root, "", false), 1);
    let _ = fs::remove_dir_all(root);
}

#[test]
fn cli_help_and_missing_candidate_are_subprocess_checked() {
    let executable = std::env::var("CARGO_BIN_EXE_xtask").expect("xtask binary path");
    let help = Command::new(&executable)
        .args(["verify-completion", "--help"])
        .output()
        .expect("run xtask help");
    assert!(help.status.success());
    let missing = Command::new(executable)
        .args(["verify-completion"])
        .output()
        .expect("run xtask with missing candidate");
    assert!(!missing.status.success());
    let unknown = Command::new(std::env::var("CARGO_BIN_EXE_xtask").unwrap())
        .args(["verify-completion", "--candidate", "a", "--unknown"])
        .output()
        .expect("run xtask with unknown argument");
    assert!(!unknown.status.success());
    let missing_value = Command::new(std::env::var("CARGO_BIN_EXE_xtask").unwrap())
        .args(["verify-completion", "--candidate"])
        .output()
        .expect("run xtask with missing candidate value");
    assert!(!missing_value.status.success());
}

#[test]
fn development_allows_unaccepted_requirement_but_release_rejects_it() {
    let (root, _) = complete_fixture();
    let path = root.join("plans/completion/requirements.json");
    let mut value = read_json(&path);
    value["requirements"][0]["acceptance"] = "unassessed".into();
    write_json(&path, &value);
    let candidate = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    let dev_issues = validate_completion(&root, candidate, false).unwrap();
    assert!(dev_issues.is_empty());
    assert!(
        !validate_completion(&root, candidate, true)
            .unwrap()
            .is_empty()
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn mismatched_candidate_and_false_accepted_product_are_rejected() {
    let (root, _) = complete_fixture();
    let candidate = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    let path = root.join("plans/completion/candidate.json");
    let mut value = read_json(&path);
    value["code_sha"] = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb".into();
    write_json(&path, &value);
    let issues = validate_completion(&root, candidate, false).unwrap();
    assert!(
        issues
            .iter()
            .any(|issue| issue.contains("does not match manifest"))
    );
    value["code_sha"] = candidate.into();
    write_json(&path, &value);
    let mut run = read_json(&root.join("plans/evidence/completion/run-1/run.json"));
    run["result"] = "failed".into();
    write_json(&root.join("plans/evidence/completion/run-1/run.json"), &run);
    let issues = validate_completion(&root, candidate, false).unwrap();
    assert!(
        issues
            .iter()
            .any(|issue| issue.contains("lacks eligible evidence"))
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn accepted_internal_without_safety_evidence_fails_development() {
    let (root, _) = complete_fixture();
    let path = root.join("plans/completion/requirements.json");
    let mut value = read_json(&path);
    value["requirements"][0]["kind"] = "internal".into();
    value["requirements"][0]["protected_product_ids"] = json!(["REQ-1"]);
    write_json(&path, &value);
    let issues =
        validate_completion(&root, "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa", false).unwrap();
    assert!(issues.iter().any(|issue| issue.contains("safety evidence")));
    let _ = fs::remove_dir_all(root);
}

#[test]
fn release_rejects_invalidating_defect_and_closed_verification_failures() {
    let (root, _) = complete_fixture();
    let path = root.join("plans/completion/defects.json");
    let req_path = root.join("plans/completion/requirements.json");
    let mut requirements = read_json(&req_path);
    requirements["requirements"][0]["defect_ids"] = json!(["D-1"]);
    write_json(&req_path, &requirements);
    let defect = json!({"id":"D-1","requirement_ids":["REQ-1"],"scenario_id":"SC-1","configuration_id":"windows-x64","severity":"P1","invalidates_required_outcome":true,"reproduction":["run"],"expected":"ok","observed":"bad","owner":"owner","status":"open","repair_package_id":"PKG-1","verification_run_ids":[]});
    write_json(&path, &json!({"schema_version":1,"defects":[defect]}));
    let dev_issues =
        validate_completion(&root, "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa", false).unwrap();
    assert!(
        dev_issues.is_empty(),
        "development should tolerate open defect: {dev_issues:?}"
    );
    let issues =
        validate_completion(&root, "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa", true).unwrap();
    assert!(
        issues
            .iter()
            .any(|issue| issue.contains("unresolved defect"))
    );
    let mut defects = read_json(&path);
    defects["defects"][0]["status"] = "closed".into();
    defects["defects"][0]["verification_run_ids"] = json!(["missing"]);
    write_json(&path, &defects);
    let issues =
        validate_completion(&root, "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa", true).unwrap();
    assert!(issues.iter().any(|issue| issue.contains("absent")));
    let _ = fs::remove_dir_all(root);

    let (root, _) = complete_fixture();
    let path = root.join("plans/completion/defects.json");
    let req_path = root.join("plans/completion/requirements.json");
    let mut requirements = read_json(&req_path);
    requirements["requirements"][0]["defect_ids"] = json!(["D-1"]);
    write_json(&req_path, &requirements);
    let defect = json!({"id":"D-1","requirement_ids":["REQ-1"],"scenario_id":"SC-1","configuration_id":"windows-x64","severity":"P1","invalidates_required_outcome":true,"reproduction":["run"],"expected":"ok","observed":"bad","owner":"owner","status":"fixed-awaiting-verification","repair_package_id":"PKG-1","verification_run_ids":[]});
    write_json(&path, &json!({"schema_version":1,"defects":[defect]}));
    assert!(
        validate_completion(&root, "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa", false)
            .unwrap()
            .is_empty()
    );
    let issues =
        validate_completion(&root, "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa", true).unwrap();
    assert!(
        issues
            .iter()
            .any(|issue| issue.contains("unresolved defect"))
    );
    let _ = fs::remove_dir_all(root);

    for mutation in [
        "failed",
        "wrong-candidate",
        "wrong-scenario",
        "wrong-configuration",
    ] {
        let (root, run_path) = complete_fixture();
        let path = root.join("plans/completion/defects.json");
        let req_path = root.join("plans/completion/requirements.json");
        let mut requirements = read_json(&req_path);
        requirements["requirements"][0]["defect_ids"] = json!(["D-1"]);
        write_json(&req_path, &requirements);
        let defect = json!({"id":"D-1","requirement_ids":["REQ-1"],"scenario_id":"SC-1","configuration_id":"windows-x64","severity":"P1","invalidates_required_outcome":false,"reproduction":["run"],"expected":"ok","observed":"bad","owner":"owner","status":"closed","repair_package_id":"PKG-1","verification_run_ids":["run-1"]});
        write_json(&path, &json!({"schema_version":1,"defects":[defect]}));
        let mut run = read_json(&run_path);
        match mutation {
            "failed" => run["result"] = "failed".into(),
            "wrong-candidate" => {
                run["candidate_sha"] = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb".into()
            }
            "wrong-scenario" => run["scenario_id"] = "wrong".into(),
            "wrong-configuration" => {
                let matrix_path = root.join("plans/completion/matrix.json");
                let mut matrix = read_json(&matrix_path);
                let mut second = matrix["configurations"][0].clone();
                second["id"] = "windows-arm64".into();
                matrix["configurations"]
                    .as_array_mut()
                    .unwrap()
                    .push(second);
                write_json(&matrix_path, &matrix);
                run["configuration_id"] = "windows-arm64".into();
            }
            _ => unreachable!(),
        }
        write_json(&run_path, &run);
        let issues =
            validate_completion(&root, "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa", true).unwrap();
        assert!(!issues.is_empty(), "{mutation} unexpectedly passed");
        let _ = fs::remove_dir_all(root);
    }

    let (root, run_path) = complete_fixture();
    let req_path = root.join("plans/completion/requirements.json");
    let mut requirements = read_json(&req_path);
    requirements["requirements"][0]["kind"] = "internal".into();
    requirements["requirements"][0]["protected_product_ids"] = json!(["REQ-1"]);
    requirements["requirements"][0]["defect_ids"] = json!(["D-1"]);
    write_json(&req_path, &requirements);
    let mut run = read_json(&run_path);
    run["layer"] = "integrated".into();
    write_json(&run_path, &run);
    let defect = json!({"id":"D-1","requirement_ids":["REQ-1"],"scenario_id":"SC-1","configuration_id":"windows-x64","severity":"P1","invalidates_required_outcome":false,"reproduction":["run"],"expected":"ok","observed":"bad","owner":"owner","status":"closed","repair_package_id":"PKG-1","verification_run_ids":["run-1"]});
    write_json(
        &root.join("plans/completion/defects.json"),
        &json!({"schema_version":1,"defects":[defect]}),
    );
    let issues =
        validate_completion(&root, "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa", true).unwrap();
    assert!(
        !issues
            .iter()
            .any(|issue| issue.contains("not eligible linked evidence")),
        "internal safety evidence was incorrectly treated as product evidence: {issues:?}"
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn internal_safety_requires_oracle_recovery_and_no_required_substitution() {
    for mutation in ["oracle", "recovery", "substitution"] {
        let (root, run_path) = complete_fixture();
        let req_path = root.join("plans/completion/requirements.json");
        let mut requirements = read_json(&req_path);
        requirements["requirements"][0]["kind"] = "internal".into();
        requirements["requirements"][0]["protected_product_ids"] = json!(["REQ-1"]);
        write_json(&req_path, &requirements);
        let dependency_path = root.join("plans/completion/dependencies.json");
        let mut dependencies = read_json(&dependency_path);
        dependencies["packages"][0]["external_prerequisites"] = json!([]);
        write_json(&dependency_path, &dependencies);
        let mut run = read_json(&run_path);
        run["layer"] = "component".into();
        match mutation {
            "oracle" => run["oracle_results"] = json!([]),
            "recovery" => run["recovery_results"] = json!([]),
            "substitution" => {
                run["dependencies"] = json!([{"name":"mock","version":"1","execution":"substituted","required_for_outcome":true,"substitution_reason":"test"}])
            }
            _ => unreachable!(),
        }
        write_json(&run_path, &run);
        let issues =
            validate_completion(&root, "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa", false).unwrap();
        assert!(
            issues.iter().any(|issue| issue.contains("safety evidence")),
            "{mutation}: {issues:?}"
        );
        let _ = fs::remove_dir_all(root);
    }
}

#[test]
fn release_rejects_per_row_configuration_gap_and_prerequisite_variants() {
    let (root, run_path) = complete_fixture();
    let req_path = root.join("plans/completion/requirements.json");
    let mut requirements = read_json(&req_path);
    let matrix_path = root.join("plans/completion/matrix.json");
    let mut matrix = read_json(&matrix_path);
    let mut second_config = matrix["configurations"][0].clone();
    second_config["id"] = "windows-arm64".into();
    second_config["required"] = false.into();
    matrix["configurations"]
        .as_array_mut()
        .unwrap()
        .push(second_config);
    write_json(&matrix_path, &matrix);
    let scenario_path = root.join("plans/completion/scenarios.json");
    let mut scenarios = read_json(&scenario_path);
    let mut second_scenario = scenarios["scenarios"][0].clone();
    second_scenario["id"] = "SC-2".into();
    second_scenario["requirement_ids"] = json!(["REQ-2"]);
    second_scenario["configuration_ids"] = json!(["windows-arm64"]);
    scenarios["scenarios"]
        .as_array_mut()
        .unwrap()
        .push(second_scenario);
    write_json(&scenario_path, &scenarios);
    let mut second_requirement = requirements["requirements"][0].clone();
    second_requirement["id"] = "REQ-2".into();
    second_requirement["title"] = "Other covered row".into();
    second_requirement["scenario_ids"] = json!(["SC-2"]);
    second_requirement["configuration_ids"] = json!(["windows-arm64"]);
    requirements["requirements"]
        .as_array_mut()
        .unwrap()
        .push(second_requirement);
    let dependency_path = root.join("plans/completion/dependencies.json");
    let mut dependencies = read_json(&dependency_path);
    dependencies["packages"][0]["requirement_ids"] = json!(["REQ-1", "REQ-2"]);
    write_json(&dependency_path, &dependencies);
    requirements["requirements"][0]["configuration_ids"] = json!(["windows-x64", "windows-arm64"]);
    write_json(&req_path, &requirements);
    let issues =
        validate_completion(&root, "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa", true).unwrap();
    assert!(
        issues
            .iter()
            .any(|issue| issue.contains("no scenario coverage"))
    );
    requirements["requirements"][0]["configuration_ids"] = json!(["windows-x64"]);
    write_json(&req_path, &requirements);
    let mut run = read_json(&run_path);
    run["dependencies"][0]["name"] = "wrong".into();
    write_json(&run_path, &run);
    let issues =
        validate_completion(&root, "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa", true).unwrap();
    assert!(issues.iter().any(|issue| issue.contains("prerequisite")));
    let _ = fs::remove_dir_all(root);
}

#[test]
fn release_rejects_substituted_failed_and_versionless_prerequisites() {
    for mutation in [
        "substituted",
        "failed",
        "versionless",
        "non-required",
        "wrong-configuration",
        "wrong-package",
    ] {
        let (root, run_path) = complete_fixture();
        let mut run = read_json(&run_path);
        match mutation {
            "substituted" => run["dependencies"][0]["execution"] = "substituted".into(),
            "failed" => run["result"] = "failed".into(),
            "versionless" => run["dependencies"][0]["version"] = "".into(),
            "non-required" => run["dependencies"][0]["required_for_outcome"] = false.into(),
            "wrong-configuration" => {
                let matrix_path = root.join("plans/completion/matrix.json");
                let mut matrix = read_json(&matrix_path);
                let mut second = matrix["configurations"][0].clone();
                second["id"] = "windows-arm64".into();
                matrix["configurations"]
                    .as_array_mut()
                    .unwrap()
                    .push(second);
                write_json(&matrix_path, &matrix);
                run["configuration_id"] = "windows-arm64".into();
            }
            "wrong-package" => {
                let dependency_path = root.join("plans/completion/dependencies.json");
                let mut dependencies = read_json(&dependency_path);
                dependencies["packages"][0]["package_id"] = "PKG-2".into();
                write_json(&dependency_path, &dependencies);
            }
            _ => unreachable!(),
        }
        write_json(&run_path, &run);
        let issues =
            validate_completion(&root, "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa", true).unwrap();
        assert!(
            issues
                .iter()
                .any(|issue| issue.contains("prerequisite") || issue.contains("package")),
            "{mutation}: {issues:?}"
        );
        let _ = fs::remove_dir_all(root);
    }
}

#[test]
fn prerequisite_fulfillment_is_aggregated_per_package_configuration() {
    let (root, _) = complete_fixture();
    let requirement_path = root.join("plans/completion/requirements.json");
    let scenario_path = root.join("plans/completion/scenarios.json");
    let dependency_path = root.join("plans/completion/dependencies.json");
    let mut requirements = read_json(&requirement_path);
    let mut second = requirements["requirements"][0].clone();
    second["id"] = "REQ-2".into();
    second["title"] = "Second requirement".into();
    requirements["requirements"]
        .as_array_mut()
        .unwrap()
        .push(second);
    write_json(&requirement_path, &requirements);
    let mut scenarios = read_json(&scenario_path);
    scenarios["scenarios"][0]["requirement_ids"] = json!(["REQ-1", "REQ-2"]);
    write_json(&scenario_path, &scenarios);
    let mut dependencies = read_json(&dependency_path);
    dependencies["packages"][0]["requirement_ids"] = json!(["REQ-1", "REQ-2"]);
    write_json(&dependency_path, &dependencies);
    let issues =
        validate_completion(&root, "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa", true).unwrap();
    assert!(
        issues.is_empty(),
        "shared package/config fulfillment failed: {issues:?}"
    );
    let _ = fs::remove_dir_all(root);
}

fn xtask_binary() -> PathBuf {
    PathBuf::from(std::env::var("CARGO_BIN_EXE_xtask").expect("xtask binary path"))
}

/// The complete fixture with the candidate manifest and the entire evidence
/// tree removed: exactly the pre-candidate state this command exists to check.
fn register_only_fixture() -> PathBuf {
    let (root, _) = complete_fixture();
    fs::remove_file(root.join("plans/completion/candidate.json"))
        .expect("remove candidate manifest");
    fs::remove_dir_all(root.join("plans/evidence")).expect("remove evidence tree");
    root
}

fn run_register_cli(root: &std::path::Path) -> std::process::Output {
    Command::new(xtask_binary())
        .args(["verify-completion-register", "--root"])
        .arg(root)
        .output()
        .expect("run verify-completion-register")
}

#[test]
fn register_command_validates_structure_without_candidate_or_evidence() {
    let root = register_only_fixture();
    assert!(!root.join("plans/completion/candidate.json").exists());
    assert!(!root.join("plans/evidence").exists());
    let candidate = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

    // The evidence-bearing entry point genuinely cannot run in this state.
    let evidence_error = validate_completion(&root, candidate, false).unwrap_err();
    assert!(
        evidence_error.contains("plans/completion/candidate.json"),
        "expected a candidate manifest failure, got {evidence_error}"
    );
    assert_eq!(run_verify_completion_command(&root, candidate, false), 1);

    // The register-only path is clean on that same state.
    let issues = validate_completion_register(&root).expect("register loads without a candidate");
    assert!(
        issues.is_empty(),
        "unexpected structural issues: {issues:?}"
    );
    assert_eq!(run_verify_completion_register_command(&root), 0);

    let output = run_register_cli(&root);
    assert!(
        output.status.success(),
        "status {:?}, stderr {}",
        output.status,
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("implementation status counts:"), "{stdout}");
    assert!(stdout.contains("  implemented: 1"), "{stdout}");
    assert!(stdout.contains("acceptance status counts:"), "{stdout}");
    assert!(stdout.contains("  accepted: 1"), "{stdout}");
    assert!(
        stdout.contains(
            "verify-completion-register passed: register structure only, no evidence or acceptance was assessed"
        ),
        "{stdout}"
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn register_command_issue_list_matches_validate_register_structure_exactly() {
    let root = register_only_fixture();
    let path = root.join("plans/completion/requirements.json");
    let mut value = read_json(&path);
    value["requirements"][0]["title"] = "".into();
    value["requirements"][0]["depends_on"] = json!(["REQ-MISSING"]);
    write_json(&path, &value);

    let direct: BTreeSet<String> = validate_register_structure(&root)
        .expect("register loads")
        .into_iter()
        .collect();
    assert!(
        direct.contains("REQ-1.title is blank"),
        "validator did not report the blank title: {direct:?}"
    );
    assert!(
        direct.contains("REQ-1.depends_on unknown reference `REQ-MISSING`"),
        "validator did not report the unknown dependency: {direct:?}"
    );

    let via_command: BTreeSet<String> = validate_completion_register(&root)
        .expect("register loads")
        .into_iter()
        .collect();
    assert_eq!(
        via_command, direct,
        "command issue set diverged from validate_register_structure"
    );

    let output = run_register_cli(&root);
    assert!(!output.status.success(), "issues must exit nonzero");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains(&format!(
            "verify-completion-register found {} structural issue(s):",
            direct.len()
        )),
        "{stderr}"
    );
    let printed: BTreeSet<String> = stderr
        .lines()
        .filter_map(|line| line.strip_prefix("- "))
        .map(str::to_string)
        .collect();
    assert_eq!(
        printed, direct,
        "printed issue set diverged from validate_register_structure"
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn register_command_reports_structural_issue_and_exits_nonzero() {
    let root = register_only_fixture();
    assert!(
        validate_completion_register(&root).unwrap().is_empty(),
        "fixture must start structurally clean"
    );

    let path = root.join("plans/completion/requirements.json");
    let mut value = read_json(&path);
    assert_eq!(value["requirements"][0]["acceptance"], json!("accepted"));
    value["requirements"][0]["implementation"] = "partial".into();
    write_json(&path, &value);

    let issues = validate_completion_register(&root).unwrap();
    assert!(
        issues
            .iter()
            .any(|issue| issue == "REQ-1 accepted while implementation is not implemented"),
        "{issues:?}"
    );
    assert_eq!(run_verify_completion_register_command(&root), 1);

    let output = run_register_cli(&root);
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("REQ-1 accepted while implementation is not implemented"),
        "{stderr}"
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(!stdout.contains("passed"), "{stdout}");
    let _ = fs::remove_dir_all(root);
}

#[test]
fn register_command_missing_register_file_is_operational_failure() {
    // Case 1: an empty root, where the first register file is already missing.
    let root = temp_root();
    let error = validate_completion_register(&root).unwrap_err();
    assert!(
        error.contains("plans/completion/requirements.json"),
        "{error}"
    );
    assert_eq!(run_verify_completion_register_command(&root), 1);
    let output = run_register_cli(&root);
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("plans/completion/requirements.json"),
        "{stderr}"
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(!stdout.contains("passed"), "{stdout}");
    assert!(
        !stdout.contains("status counts:"),
        "counts must not print off a register that failed to load: {stdout}"
    );
    let _ = fs::remove_dir_all(root);

    // Case 2: requirements.json loads and matrix.json is missing, so the counts
    // would have been computable but must still not be printed.
    let root = register_only_fixture();
    fs::remove_file(root.join("plans/completion/matrix.json")).expect("remove matrix register");
    let error = validate_completion_register(&root).unwrap_err();
    assert!(error.contains("plans/completion/matrix.json"), "{error}");
    assert_eq!(run_verify_completion_register_command(&root), 1);
    let output = run_register_cli(&root);
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("plans/completion/matrix.json"), "{stderr}");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(!stdout.contains("passed"), "{stdout}");
    assert!(
        !stdout.contains("status counts:"),
        "counts must not print off a register that failed to load: {stdout}"
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn register_cli_subprocess_rejects_candidate_and_release_flags() {
    let executable = xtask_binary();
    let help = Command::new(&executable)
        .args(["verify-completion-register", "--help"])
        .output()
        .expect("run verify-completion-register help");
    assert!(
        help.status.success(),
        "stderr {}",
        String::from_utf8_lossy(&help.stderr)
    );
    let help_text = String::from_utf8_lossy(&help.stdout);
    assert!(help_text.contains("--root"), "{help_text}");
    assert!(!help_text.contains("--candidate"), "{help_text}");
    assert!(!help_text.contains("--release"), "{help_text}");

    for (args, flag) in [
        (
            &[
                "verify-completion-register",
                "--candidate",
                "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            ][..],
            "--candidate",
        ),
        (
            &["verify-completion-register", "--release"][..],
            "--release",
        ),
        (
            &[
                "verify-completion-register",
                "--candidate",
                "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                "--release",
            ][..],
            "--candidate",
        ),
    ] {
        let output = Command::new(&executable)
            .args(args)
            .output()
            .expect("run verify-completion-register with a rejected flag");
        assert!(
            !output.status.success(),
            "{args:?} unexpectedly succeeded: {}",
            String::from_utf8_lossy(&output.stdout)
        );
        let stderr = String::from_utf8_lossy(&output.stderr).to_lowercase();
        assert!(
            stderr.contains(flag),
            "{args:?}: stderr does not name {flag}: {stderr}"
        );
        assert!(
            stderr.contains("unexpected") || stderr.contains("unrecognized"),
            "{args:?}: nonzero exit was not an argument rejection: {stderr}"
        );
    }
}
