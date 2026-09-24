use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::Serialize;
use serde_json::Value;
use sha2::{Digest, Sha256};
use xtask::completion::identity_receipts::{
    BuildProvenanceReceipt, RunningIdentity, RunningIdentityReceipt,
};
use xtask::completion::schema::{
    CandidateArtifact, CandidateManifest, CheckResult, Configuration, EvidenceLayer,
    EvidenceResult, InputRoute, MatrixDocument, OperatingSystem, Requirement, RequirementKind,
    RequirementsDocument, ReviewDecision, Scenario, ScenarioCheck, ScenariosDocument, SourceRef,
    Stage,
};
use xtask::completion_evidence::{load_evidence_runs, validate_evidence_files};

const CANDIDATE: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const HISTORICAL: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
const ARTIFACT_HASH: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

struct Fixture {
    root: PathBuf,
    run_path: PathBuf,
    identity_path: PathBuf,
}

fn digest(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

fn temp_root(label: &str) -> PathBuf {
    // Process id + monotonic counter keeps the fixture unique when cargo test
    // runs this crate and the included completion_command suite in parallel.
    // Windows clock granularity can be coarser than a nanosecond, so a
    // timestamp-only name lets one test's remove_dir_all delete another's tree
    // (the CLI subprocess then fails with ERROR_PATH_NOT_FOUND).
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let pid = std::process::id();
    let seq = COUNTER.fetch_add(1, Ordering::Relaxed);
    let root = std::env::temp_dir().join(format!("legion-completion-{label}-{pid}-{stamp}-{seq}"));
    fs::create_dir_all(&root).expect("temporary root");
    root
}

fn write_json<T: Serialize>(path: &Path, value: &T) {
    fs::write(
        path,
        serde_json::to_vec_pretty(value).expect("serialize fixture"),
    )
    .expect("write fixture");
}

fn base_fixture() -> Fixture {
    let root = temp_root("evidence");
    let completion = root.join("plans/evidence/completion/run-1");
    fs::create_dir_all(&completion).expect("evidence directory");
    fs::create_dir_all(root.join("plans/completion")).expect("completion registers");

    let configuration = Configuration {
        id: "windows-x64".into(),
        os: OperatingSystem::Windows,
        architecture: "x64".into(),
        tool_versions: BTreeMap::from([("rust".into(), "1.92".into())]),
        hardware: "lab".into(),
        project_category: "desktop".into(),
        required: true,
        owner_approval_ref: "APR-1".into(),
    };
    write_json(
        &root.join("plans/completion/requirements.json"),
        &RequirementsDocument {
            schema_version: 1,
            requirements: vec![Requirement {
                id: "REQ-1".into(),
                title: "Evidence".into(),
                kind: RequirementKind::Product,
                required: true,
                source_refs: vec![SourceRef {
                    path: "docs/spec.md".into(),
                    identity: "spec".into(),
                }],
                legacy_ids: vec![],
                stage: Stage::S0,
                package_id: "PKG-1".into(),
                owner_role: "owner".into(),
                depends_on: vec![],
                implementation: xtask::completion::schema::Implementation::Implemented,
                acceptance: xtask::completion::schema::Acceptance::Unassessed,
                scenario_ids: vec!["SC-1".into()],
                configuration_ids: vec!["windows-x64".into()],
                protected_product_ids: vec![],
                defect_ids: vec![],
            }],
        },
    );
    write_json(
        &root.join("plans/completion/matrix.json"),
        &MatrixDocument {
            schema_version: 1,
            configurations: vec![configuration],
        },
    );
    write_json(
        &root.join("plans/completion/scenarios.json"),
        &ScenariosDocument {
            schema_version: 1,
            scenarios: vec![Scenario {
                id: "SC-1".into(),
                requirement_ids: vec!["REQ-1".into()],
                configuration_ids: vec!["windows-x64".into()],
                steps: vec!["run".into()],
                external_oracles: vec![ScenarioCheck {
                    id: "oracle-1".into(),
                    description: "oracle".into(),
                }],
                recovery_cases: vec![ScenarioCheck {
                    id: "recovery-1".into(),
                    description: "recovery".into(),
                }],
                sensitive_artifact_policy: "redact".into(),
            }],
        },
    );

    let build_path = "plans/evidence/completion/run-1/build-receipt.json";
    let artifact_path = "C:/installed/legion.exe";
    write_json(
        &root.join("plans/completion/candidate.json"),
        &CandidateManifest {
            schema_version: 1,
            code_sha: CANDIDATE.into(),
            artifacts: vec![CandidateArtifact {
                component: "desktop".into(),
                configuration_id: "windows-x64".into(),
                path: artifact_path.into(),
                sha256: ARTIFACT_HASH.into(),
                build_provenance_path: build_path.into(),
            }],
            configuration_ids: vec!["windows-x64".into()],
            nominated_at_utc: "2026-01-01T00:00:00Z".into(),
            nominated_by: "owner".into(),
        },
    );

    let build_capture = "plans/evidence/completion/run-1/build-observer.log";
    let runtime_capture = "plans/evidence/completion/run-1/runtime.json";
    let identity_path = "plans/evidence/completion/run-1/identity.json";
    let oracle_path = "plans/evidence/completion/run-1/oracle.txt";
    let recovery_path = "plans/evidence/completion/run-1/recovery.txt";
    let mut hashes = BTreeMap::new();
    for (path, bytes) in [
        (build_capture, b"build capture".as_slice()),
        (runtime_capture, b"runtime capture".as_slice()),
        (oracle_path, b"oracle".as_slice()),
        (recovery_path, b"recovery".as_slice()),
    ] {
        fs::write(root.join(path), bytes).expect("attachment");
        hashes.insert(path.into(), digest(bytes));
    }

    let mut run = xtask::completion::schema::EvidenceRun {
        schema_version: 1,
        id: "run-1".into(),
        scenario_id: "SC-1".into(),
        configuration_id: "windows-x64".into(),
        candidate_sha: CANDIDATE.into(),
        artifact_sha256: ARTIFACT_HASH.into(),
        layer: EvidenceLayer::Product,
        input_route: InputRoute::NativeInput,
        dependencies: vec![],
        result: EvidenceResult::Passed,
        oracle_results: vec![CheckResult {
            id: "oracle-1".into(),
            passed: true,
            artifact_path: oracle_path.into(),
            observed: "ok".into(),
        }],
        recovery_results: vec![CheckResult {
            id: "recovery-1".into(),
            passed: true,
            artifact_path: recovery_path.into(),
            observed: "ok".into(),
        }],
        artifact_hashes: hashes.clone(),
        defect_ids: vec![],
        implementation_owners: vec!["builder".into()],
        reviewer: "reviewer".into(),
        review_decision: ReviewDecision::Accepted,
        started_at_utc: "2026-01-01T00:00:00Z".into(),
        ended_at_utc: "2026-01-01T00:00:10Z".into(),
    };
    write_json(&completion.join("run.json"), &run);
    write_json(
        &completion.join("build-receipt.json"),
        &BuildProvenanceReceipt {
            schema_version: 1,
            candidate_sha: CANDIDATE.into(),
            verifier_sha: HISTORICAL.into(),
            component: "desktop".into(),
            configuration_id: "windows-x64".into(),
            artifact_sha256: ARTIFACT_HASH.into(),
            build_command: "build".into(),
            tool_versions: BTreeMap::from([("rust".into(), "1.92".into())]),
            capture_artifact_path: build_capture.into(),
            captured_at_utc: "2025-12-31T23:59:59Z".into(),
            observer: "builder".into(),
        },
    );
    write_json(
        &completion.join("identity.json"),
        &RunningIdentityReceipt {
            schema_version: 1,
            run_id: "run-1".into(),
            candidate_sha: CANDIDATE.into(),
            component: "desktop".into(),
            configuration_id: "windows-x64".into(),
            observed_sha256: ARTIFACT_HASH.into(),
            observed_version: "1.0".into(),
            observed_path: artifact_path.into(),
            observer: "operator".into(),
            captured_at_utc: "2026-01-01T00:00:05Z".into(),
            capture_artifact_path: runtime_capture.into(),
            identity: RunningIdentity::Process {
                pid: 42,
                start_token: "start".into(),
            },
        },
    );
    hashes.insert(
        build_path.into(),
        digest(&fs::read(completion.join("build-receipt.json")).unwrap()),
    );
    hashes.insert(
        identity_path.into(),
        digest(&fs::read(completion.join("identity.json")).unwrap()),
    );
    run.artifact_hashes = hashes;
    write_json(&completion.join("run.json"), &run);

    Fixture {
        root,
        run_path: completion.join("run.json"),
        identity_path: completion.join("identity.json"),
    }
}

fn cleanup(fixture: &Fixture) {
    let _ = fs::remove_dir_all(&fixture.root);
}

fn rehash_attachment(fixture: &Fixture, relative: &str) {
    let mut run: Value = serde_json::from_slice(&fs::read(&fixture.run_path).unwrap()).unwrap();
    let hash = digest(&fs::read(fixture.root.join(relative)).unwrap());
    run["artifact_hashes"][relative] = Value::String(hash);
    fs::write(&fixture.run_path, serde_json::to_vec_pretty(&run).unwrap()).unwrap();
}

#[test]
fn validates_explicit_testdata_fixture() {
    let fixture = base_fixture();
    assert!(
        validate_evidence_files(&fixture.root, CANDIDATE)
            .expect("operationally valid fixture")
            .is_empty()
    );
    assert_eq!(
        load_evidence_runs(&fixture.root, CANDIDATE).unwrap().len(),
        1
    );
    cleanup(&fixture);
}

#[test]
fn rejects_missing_altered_and_escape_attachments() {
    for mode in ["missing", "altered", "escape"] {
        let fixture = base_fixture();
        if mode == "missing" {
            fs::remove_file(
                fixture
                    .root
                    .join("plans/evidence/completion/run-1/oracle.txt"),
            )
            .unwrap();
        } else if mode == "altered" {
            fs::write(
                fixture
                    .root
                    .join("plans/evidence/completion/run-1/oracle.txt"),
                b"altered",
            )
            .unwrap();
        } else {
            let mut run: Value =
                serde_json::from_slice(&fs::read(&fixture.run_path).unwrap()).unwrap();
            run["artifact_hashes"]["outside.txt"] = Value::String(digest(b"outside"));
            fs::write(&fixture.run_path, serde_json::to_vec_pretty(&run).unwrap()).unwrap();
        }
        let issues = validate_evidence_files(&fixture.root, CANDIDATE).unwrap();
        assert!(!issues.is_empty(), "{mode} attachment unexpectedly passed");
        cleanup(&fixture);
    }
}

#[test]
fn rejects_selected_candidate_mismatch() {
    let fixture = base_fixture();
    let issues = validate_evidence_files(&fixture.root, HISTORICAL).unwrap();
    assert!(issues.iter().any(|issue| issue.contains("code_sha")));
    cleanup(&fixture);
}

#[test]
fn rejects_unknown_run_fields_as_contextual_json_error() {
    let fixture = base_fixture();
    let mut run: Value = serde_json::from_slice(&fs::read(&fixture.run_path).unwrap()).unwrap();
    run["unexpected"] = Value::Bool(true);
    fs::write(&fixture.run_path, serde_json::to_vec_pretty(&run).unwrap()).unwrap();
    let error = validate_evidence_files(&fixture.root, CANDIDATE).unwrap_err();
    assert!(error.contains("run.json"));
    cleanup(&fixture);
}

#[test]
fn rejects_wrong_scenario_configuration_and_missing_reviewer() {
    let fixture = base_fixture();
    let mut run: Value = serde_json::from_slice(&fs::read(&fixture.run_path).unwrap()).unwrap();
    run["scenario_id"] = "UNKNOWN".into();
    fs::write(&fixture.run_path, serde_json::to_vec_pretty(&run).unwrap()).unwrap();
    let issues = validate_evidence_files(&fixture.root, CANDIDATE).unwrap();
    assert!(
        issues
            .iter()
            .any(|issue| issue.contains("unknown scenario"))
    );
    cleanup(&fixture);

    let fixture = base_fixture();
    let mut run: Value = serde_json::from_slice(&fs::read(&fixture.run_path).unwrap()).unwrap();
    run["configuration_id"] = "UNKNOWN".into();
    run["reviewer"] = " ".into();
    fs::write(&fixture.run_path, serde_json::to_vec_pretty(&run).unwrap()).unwrap();
    let issues = validate_evidence_files(&fixture.root, CANDIDATE).unwrap();
    assert!(
        issues
            .iter()
            .any(|issue| issue.contains("unknown configuration"))
    );
    assert!(issues.iter().any(|issue| issue.contains("blank reviewer")));
    cleanup(&fixture);
}

#[test]
fn rejects_failed_oracle_and_missing_recovery() {
    let fixture = base_fixture();
    let mut run: Value = serde_json::from_slice(&fs::read(&fixture.run_path).unwrap()).unwrap();
    run["oracle_results"][0]["passed"] = Value::Bool(false);
    run["recovery_results"] = Value::Array(vec![]);
    fs::write(&fixture.run_path, serde_json::to_vec_pretty(&run).unwrap()).unwrap();
    let issues = validate_evidence_files(&fixture.root, CANDIDATE).unwrap();
    assert!(issues.iter().any(|issue| issue.contains("failed oracle")));
    assert!(
        issues
            .iter()
            .any(|issue| issue.contains("missing recovery"))
    );
    cleanup(&fixture);
}

#[test]
fn rejects_missing_and_mismatched_identity_receipts() {
    let fixture = base_fixture();
    fs::remove_file(&fixture.identity_path).unwrap();
    let issues = validate_evidence_files(&fixture.root, CANDIDATE).unwrap();
    assert!(
        issues
            .iter()
            .any(|issue| issue.contains("requires identity.json"))
    );
    cleanup(&fixture);

    let fixture = base_fixture();
    let mut identity: Value =
        serde_json::from_slice(&fs::read(&fixture.identity_path).unwrap()).unwrap();
    identity["observed_sha256"] = "f".repeat(64).into();
    fs::write(
        &fixture.identity_path,
        serde_json::to_vec_pretty(&identity).unwrap(),
    )
    .unwrap();
    let issues = validate_evidence_files(&fixture.root, CANDIDATE).unwrap();
    assert!(
        issues
            .iter()
            .any(|issue| issue.contains("identity.json") && issue.contains("does not match"))
    );
    cleanup(&fixture);
}

#[test]
fn rejects_identity_receipt_without_declared_hash() {
    let fixture = base_fixture();
    let mut run: Value = serde_json::from_slice(&fs::read(&fixture.run_path).unwrap()).unwrap();
    run["artifact_hashes"]
        .as_object_mut()
        .unwrap()
        .remove("plans/evidence/completion/run-1/identity.json");
    fs::write(&fixture.run_path, serde_json::to_vec_pretty(&run).unwrap()).unwrap();
    let issues = validate_evidence_files(&fixture.root, CANDIDATE).unwrap();
    assert!(
        issues
            .iter()
            .any(|issue| issue.contains("identity.json must be declared"))
    );
    cleanup(&fixture);
}

#[test]
fn rejects_nominated_build_provenance_without_declared_hash() {
    let fixture = base_fixture();
    let mut run: Value = serde_json::from_slice(&fs::read(&fixture.run_path).unwrap()).unwrap();
    run["artifact_hashes"]
        .as_object_mut()
        .unwrap()
        .remove("plans/evidence/completion/run-1/build-receipt.json");
    fs::write(&fixture.run_path, serde_json::to_vec_pretty(&run).unwrap()).unwrap();
    let issues = validate_evidence_files(&fixture.root, CANDIDATE).unwrap();
    assert!(
        issues
            .iter()
            .any(|issue| issue.contains("build_provenance_path is not declared"))
    );
    cleanup(&fixture);
}

#[test]
fn rejects_malformed_identity_and_build_receipts_as_contextual_errors() {
    let fixture = base_fixture();
    fs::write(&fixture.identity_path, b"{").unwrap();
    rehash_attachment(&fixture, "plans/evidence/completion/run-1/identity.json");
    let error = validate_evidence_files(&fixture.root, CANDIDATE).unwrap_err();
    assert!(error.contains("identity.json"));
    cleanup(&fixture);

    let fixture = base_fixture();
    fs::write(
        fixture
            .root
            .join("plans/evidence/completion/run-1/build-receipt.json"),
        b"{",
    )
    .unwrap();
    rehash_attachment(
        &fixture,
        "plans/evidence/completion/run-1/build-receipt.json",
    );
    let error = validate_evidence_files(&fixture.root, CANDIDATE).unwrap_err();
    assert!(error.contains("build receipt"));
    cleanup(&fixture);
}

#[cfg(unix)]
#[test]
fn rejects_symlinked_run_directories_and_run_records() {
    use std::os::unix::fs::symlink;

    let fixture = base_fixture();
    symlink(
        fixture.root.join("plans/evidence/completion/run-1"),
        fixture.root.join("plans/evidence/completion/linked"),
    )
    .unwrap();
    let error = validate_evidence_files(&fixture.root, CANDIDATE).unwrap_err();
    assert!(error.contains("symlinked evidence run directory"));
    cleanup(&fixture);

    let fixture = base_fixture();
    let run_dir = fixture.root.join("plans/evidence/completion/run-1");
    let outside = fixture.root.join("outside-run.json");
    fs::copy(run_dir.join("run.json"), &outside).unwrap();
    fs::remove_file(run_dir.join("run.json")).unwrap();
    symlink(&outside, run_dir.join("run.json")).unwrap();
    let error = validate_evidence_files(&fixture.root, CANDIDATE).unwrap_err();
    assert!(error.contains("symlinked run.json"));
    cleanup(&fixture);
}

#[cfg(unix)]
#[test]
fn rejects_declared_identity_and_build_receipt_symlinks() {
    use std::os::unix::fs::symlink;

    let fixture = base_fixture();
    let outside = fixture.root.join("outside-identity.json");
    fs::copy(&fixture.identity_path, &outside).unwrap();
    fs::remove_file(&fixture.identity_path).unwrap();
    symlink(&outside, &fixture.identity_path).unwrap();
    let issues = validate_evidence_files(&fixture.root, CANDIDATE).unwrap();
    assert!(issues.iter().any(|issue| issue.contains("identity.json")));
    cleanup(&fixture);

    let fixture = base_fixture();
    let build_path = fixture
        .root
        .join("plans/evidence/completion/run-1/build-receipt.json");
    let outside = fixture.root.join("outside-build-receipt.json");
    fs::copy(&build_path, &outside).unwrap();
    fs::remove_file(&build_path).unwrap();
    symlink(&outside, &build_path).unwrap();
    let issues = validate_evidence_files(&fixture.root, CANDIDATE).unwrap();
    assert!(
        issues
            .iter()
            .any(|issue| issue.contains("build-receipt.json"))
    );
    cleanup(&fixture);
}

#[cfg(windows)]
#[test]
fn rejects_symlinked_run_directories_and_run_records_on_windows() {
    use std::io::ErrorKind;
    use std::os::windows::fs::{symlink_dir, symlink_file};

    let fixture = base_fixture();
    let linked = fixture.root.join("plans/evidence/completion/linked");
    if let Err(error) = symlink_dir(
        fixture.root.join("plans/evidence/completion/run-1"),
        &linked,
    ) {
        if error.kind() == ErrorKind::PermissionDenied || error.raw_os_error() == Some(1314) {
            eprintln!("NOT EXERCISED: Windows run-directory symlink creation denied: {error}");
        } else {
            panic!("unexpected run-directory symlink error: {error}");
        }
    } else {
        let error = validate_evidence_files(&fixture.root, CANDIDATE).unwrap_err();
        assert!(error.contains("symlinked evidence run directory"));
    }
    cleanup(&fixture);

    let fixture = base_fixture();
    let run_dir = fixture.root.join("plans/evidence/completion/run-1");
    let outside = fixture.root.join("outside-run.json");
    fs::copy(run_dir.join("run.json"), &outside).unwrap();
    fs::remove_file(run_dir.join("run.json")).unwrap();
    if let Err(error) = symlink_file(&outside, run_dir.join("run.json")) {
        if error.kind() == ErrorKind::PermissionDenied || error.raw_os_error() == Some(1314) {
            eprintln!("NOT EXERCISED: Windows run.json symlink creation denied: {error}");
        } else {
            panic!("unexpected run.json symlink error: {error}");
        }
    } else {
        let error = validate_evidence_files(&fixture.root, CANDIDATE).unwrap_err();
        assert!(error.contains("symlinked run.json"));
    }
    cleanup(&fixture);
}

#[cfg(windows)]
#[test]
fn rejects_declared_receipt_symlinks_on_windows() {
    use std::io::ErrorKind;
    use std::os::windows::fs::symlink_file;

    let fixture = base_fixture();
    let identity_outside = fixture.root.join("outside-identity.json");
    fs::copy(&fixture.identity_path, &identity_outside).unwrap();
    fs::remove_file(&fixture.identity_path).unwrap();
    if let Err(error) = symlink_file(&identity_outside, &fixture.identity_path) {
        if error.kind() == ErrorKind::PermissionDenied || error.raw_os_error() == Some(1314) {
            eprintln!("NOT EXERCISED: Windows identity symlink creation denied: {error}");
        } else {
            panic!("unexpected identity symlink error: {error}");
        }
    } else {
        let issues = validate_evidence_files(&fixture.root, CANDIDATE).unwrap();
        assert!(issues.iter().any(|issue| issue.contains("identity.json")));
    }
    cleanup(&fixture);

    let fixture = base_fixture();
    let build_path = fixture
        .root
        .join("plans/evidence/completion/run-1/build-receipt.json");
    let build_outside = fixture.root.join("outside-build-receipt.json");
    fs::copy(&build_path, &build_outside).unwrap();
    fs::remove_file(&build_path).unwrap();
    if let Err(error) = symlink_file(&build_outside, &build_path) {
        if error.kind() == ErrorKind::PermissionDenied || error.raw_os_error() == Some(1314) {
            eprintln!("NOT EXERCISED: Windows build-receipt symlink creation denied: {error}");
        } else {
            panic!("unexpected build receipt symlink error: {error}");
        }
    } else {
        let issues = validate_evidence_files(&fixture.root, CANDIDATE).unwrap();
        assert!(
            issues
                .iter()
                .any(|issue| issue.contains("build-receipt.json"))
        );
    }
    cleanup(&fixture);
}

#[test]
fn ignores_historical_candidate_runs_after_parsing() {
    let fixture = base_fixture();
    let old_dir = fixture.root.join("plans/evidence/completion/historical");
    fs::create_dir_all(&old_dir).unwrap();
    let mut old_run: Value = serde_json::from_slice(&fs::read(&fixture.run_path).unwrap()).unwrap();
    old_run["id"] = "historical".into();
    old_run["candidate_sha"] = HISTORICAL.into();
    fs::write(
        old_dir.join("run.json"),
        serde_json::to_vec_pretty(&old_run).unwrap(),
    )
    .unwrap();
    assert_eq!(
        load_evidence_runs(&fixture.root, CANDIDATE).unwrap().len(),
        1
    );
    assert!(
        validate_evidence_files(&fixture.root, CANDIDATE)
            .unwrap()
            .is_empty()
    );
    cleanup(&fixture);
}

#[test]
fn missing_evidence_root_is_an_empty_development_runset() {
    let fixture = base_fixture();
    fs::remove_dir_all(fixture.root.join("plans/evidence/completion")).unwrap();
    assert!(
        validate_evidence_files(&fixture.root, CANDIDATE)
            .unwrap()
            .is_empty()
    );
    assert!(
        load_evidence_runs(&fixture.root, CANDIDATE)
            .unwrap()
            .is_empty()
    );
    cleanup(&fixture);
}
