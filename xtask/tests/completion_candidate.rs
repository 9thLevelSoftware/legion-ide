use std::collections::BTreeMap;
use xtask::completion::candidate::validate_candidate_manifest;
use xtask::completion::schema::{
    CandidateArtifact, CandidateManifest, Configuration, MatrixDocument, OperatingSystem,
};

const CODE_SHA: &str = "0123456789abcdef0123456789abcdef01234567";
const HASH: &str = "ABCDEF0123456789ABCDEF0123456789ABCDEF0123456789ABCDEF0123456789";

fn matrix(ids: &[&str]) -> MatrixDocument {
    MatrixDocument {
        schema_version: 1,
        configurations: ids
            .iter()
            .map(|id| Configuration {
                id: (*id).into(),
                os: OperatingSystem::Windows,
                architecture: "x64".into(),
                tool_versions: BTreeMap::new(),
                hardware: "lab".into(),
                project_category: "desktop".into(),
                required: true,
                owner_approval_ref: "APR-1".into(),
            })
            .collect(),
    }
}

fn manifest() -> CandidateManifest {
    CandidateManifest {
        schema_version: 1,
        code_sha: CODE_SHA.into(),
        artifacts: vec![CandidateArtifact {
            component: "legion-desktop".into(),
            configuration_id: "win-x64".into(),
            path: "C:/staged/legion.exe".into(),
            sha256: HASH.into(),
            build_provenance_path: "plans/evidence/completion/candidate/build.toml".into(),
        }],
        configuration_ids: vec!["win-x64".into()],
        nominated_at_utc: "2026-09-05T16:00:00Z".into(),
        nominated_by: "release-owner".into(),
    }
}

#[test]
fn accepts_realistic_candidate_without_checkout_head_input() {
    let result = validate_candidate_manifest(&manifest(), &matrix(&["win-x64"]), CODE_SHA);
    assert!(result.is_empty(), "unexpected violations: {result:?}");
}

#[test]
fn rejects_selected_sha_mismatch() {
    let result =
        validate_candidate_manifest(&manifest(), &matrix(&["win-x64"]), "f".repeat(40).as_str());
    assert!(
        result
            .iter()
            .any(|issue| issue.contains("does not match manifest code_sha"))
    );
}

#[test]
fn rejects_duplicate_artifact_pair_and_unknown_configuration() {
    let mut candidate = manifest();
    candidate.artifacts.push(candidate.artifacts[0].clone());
    candidate.artifacts.push(CandidateArtifact {
        configuration_id: "linux-x64".into(),
        ..candidate.artifacts[0].clone()
    });
    let result = validate_candidate_manifest(&candidate, &matrix(&["win-x64"]), CODE_SHA);
    assert!(
        result
            .iter()
            .any(|issue| issue.contains("duplicate component/configuration_id"))
    );
    assert!(
        result
            .iter()
            .any(|issue| issue.contains("not declared by the manifest"))
    );
    assert!(
        result
            .iter()
            .any(|issue| issue.contains("not present in the matrix"))
    );
}

#[test]
fn rejects_missing_nomination_invalid_utc_and_bad_hash() {
    let mut candidate = manifest();
    candidate.nominated_by = "  ".into();
    candidate.nominated_at_utc = "tomorrow".into();
    candidate.artifacts[0].sha256 = "not-a-hash".into();
    let result = validate_candidate_manifest(&candidate, &matrix(&["win-x64"]), CODE_SHA);
    assert!(result.iter().any(|issue| issue.contains("nominated_by")));
    assert!(
        result
            .iter()
            .any(|issue| issue.contains("nominated_at_utc"))
    );
    assert!(result.iter().any(|issue| issue.contains("sha256")));
}

#[test]
fn rejects_duplicate_and_blank_matrix_ids() {
    let result =
        validate_candidate_manifest(&manifest(), &matrix(&["win-x64", "win-x64", ""]), CODE_SHA);
    assert!(
        result
            .iter()
            .any(|issue| issue.contains("duplicate configuration id"))
    );
    assert!(
        result
            .iter()
            .any(|issue| issue.contains("id must be nonblank"))
    );
}

#[test]
fn rejects_duplicate_manifest_configuration_ids() {
    let mut candidate = manifest();
    candidate.configuration_ids.push("win-x64".into());
    let result = validate_candidate_manifest(&candidate, &matrix(&["win-x64"]), CODE_SHA);
    assert!(
        result
            .iter()
            .any(|issue| issue.contains("configuration_ids contains duplicate id"))
    );
}

#[test]
fn rejects_matched_but_noncanonical_code_sha() {
    let mut candidate = manifest();
    candidate.code_sha = format!("A{}", "0".repeat(39));
    let result =
        validate_candidate_manifest(&candidate, &matrix(&["win-x64"]), &candidate.code_sha);
    assert!(
        result
            .iter()
            .any(|issue| issue.contains("code_sha must be exactly 40 lowercase"))
    );
    assert!(
        !result
            .iter()
            .any(|issue| issue.contains("does not match manifest code_sha"))
    );
}

#[test]
fn rejects_blank_artifact_fields_as_independent_violations() {
    let mut candidate = manifest();
    candidate.artifacts[0].component = " \t".into();
    candidate.artifacts[0].path = "\n".into();
    candidate.artifacts[0].build_provenance_path = "  ".into();
    let result = validate_candidate_manifest(&candidate, &matrix(&["win-x64"]), CODE_SHA);
    assert!(
        result
            .iter()
            .any(|issue| issue.contains("component must be nonblank"))
    );
    assert!(
        result
            .iter()
            .any(|issue| issue.contains("path must be nonblank"))
    );
    assert!(
        result
            .iter()
            .any(|issue| issue.contains("build_provenance_path must be nonblank"))
    );
}

#[test]
fn rejects_provenance_paths_that_are_not_portable_repository_references() {
    for path in [
        "",
        ".",
        "..",
        "plans//build.toml",
        "plans/./build.toml",
        "plans/../build.toml",
        "/root/build.toml",
        r"\root\build.toml",
        "C:/build.toml",
        "C:build.toml",
        "//server/share/build.toml",
        r"\\?\C:\build.toml",
        r"plans/build\toml",
        "plans/build.toml:stream",
        "plans/build\0.toml",
        "plans/build\u{001f}.toml",
    ] {
        let mut candidate = manifest();
        candidate.artifacts[0].build_provenance_path = path.into();
        let result = validate_candidate_manifest(&candidate, &matrix(&["win-x64"]), CODE_SHA);
        assert!(
            result
                .iter()
                .any(|issue| issue.contains("build_provenance_path")),
            "accepted invalid provenance path {path:?}: {result:?}"
        );
    }
}

#[test]
fn requires_at_least_one_artifact_for_each_declared_configuration() {
    let candidate = manifest();
    let mut candidate = CandidateManifest {
        configuration_ids: vec!["win-x64".into(), "linux-x64".into()],
        ..candidate
    };
    candidate.artifacts[0].configuration_id = "win-x64".into();
    let result =
        validate_candidate_manifest(&candidate, &matrix(&["win-x64", "linux-x64"]), CODE_SHA);
    assert!(result.iter().any(|issue| {
        issue.contains("declared configuration")
            && issue.contains("linux-x64")
            && issue.contains("artifact")
    }));
}

#[test]
fn permits_external_artifact_path_when_provenance_reference_is_repo_relative() {
    let mut candidate = manifest();
    candidate.artifacts[0].path = r"C:\staged\legion.exe".into();
    let result = validate_candidate_manifest(&candidate, &matrix(&["win-x64"]), CODE_SHA);
    assert!(result.is_empty(), "unexpected violations: {result:?}");
}
