//! Pure validation of a nominated completion candidate manifest.
//!
//! This validator checks only the manifest's declared identity and joins to the
//! supplied configuration matrix. It does not inspect files, hash external
//! artifacts, authenticate the nominator, or infer the verifier checkout.

use std::collections::BTreeSet;

use super::artifact_files::parse_utc_timestamp;
use super::schema::{CandidateManifest, MatrixDocument};

/// Validate a candidate nomination against the canonical configuration matrix.
///
/// The returned violations are deterministic and contain every independently
/// observed invariant failure. An empty vector means the declaration is
/// structurally and semantically valid for the supplied matrix.
pub fn validate_candidate_manifest(
    manifest: &CandidateManifest,
    matrix: &MatrixDocument,
    selected_candidate: &str,
) -> Vec<String> {
    let mut violations = Vec::new();

    if !is_git_sha(&manifest.code_sha) {
        violations.push(
            "candidate code_sha must be exactly 40 lowercase hexadecimal ASCII characters".into(),
        );
    }
    if selected_candidate != manifest.code_sha {
        violations.push(format!(
            "selected candidate SHA does not match manifest code_sha (selected {selected_candidate:?}, manifest {:?})",
            manifest.code_sha
        ));
    }

    if manifest.nominated_by.trim().is_empty() {
        violations.push("candidate nominated_by must be nonempty".into());
    }
    if parse_utc_timestamp(&manifest.nominated_at_utc).is_err() {
        violations.push("candidate nominated_at_utc must be a valid UTC timestamp".into());
    }

    let matrix_ids = matrix_configuration_ids(matrix, &mut violations);
    let declared_ids = declared_configuration_ids(manifest, &matrix_ids, &mut violations);

    if manifest.artifacts.is_empty() {
        violations.push("candidate artifacts must be nonempty".into());
    }
    let mut artifact_keys = BTreeSet::new();
    let mut artifact_configuration_ids = BTreeSet::new();
    for (index, artifact) in manifest.artifacts.iter().enumerate() {
        let provenance_is_blank = artifact.build_provenance_path.trim().is_empty();
        if artifact.component.trim().is_empty() {
            violations.push(format!(
                "candidate artifact {index} component must be nonblank"
            ));
        }
        if artifact.path.trim().is_empty() {
            violations.push(format!("candidate artifact {index} path must be nonblank"));
        }
        if provenance_is_blank {
            violations.push(format!(
                "candidate artifact {index} build_provenance_path must be nonblank"
            ));
        } else if !is_portable_provenance_path(&artifact.build_provenance_path) {
            violations.push(format!(
                "candidate artifact {index} build_provenance_path must be a portable repository-relative forward-slash path"
            ));
        }
        if !is_sha256(&artifact.sha256) {
            violations.push(format!(
                "candidate artifact {index} sha256 must be exactly 64 ASCII hexadecimal characters"
            ));
        }

        artifact_configuration_ids.insert(artifact.configuration_id.clone());
        let key = (&artifact.component, &artifact.configuration_id);
        if !artifact_keys.insert(key) {
            violations.push(format!(
                "candidate artifacts contain duplicate component/configuration_id pair at index {index}"
            ));
        }
        if !declared_ids.contains(&artifact.configuration_id) {
            violations.push(format!(
                "candidate artifact {index} configuration_id {:?} is not declared by the manifest",
                artifact.configuration_id
            ));
        }
        if !matrix_ids.contains(&artifact.configuration_id) {
            violations.push(format!(
                "candidate artifact {index} configuration_id {:?} is not present in the matrix",
                artifact.configuration_id
            ));
        }
    }
    for configuration_id in &declared_ids {
        if !artifact_configuration_ids.contains(configuration_id) {
            violations.push(format!(
                "declared configuration {configuration_id:?} must have at least one artifact"
            ));
        }
    }

    violations
}

fn matrix_configuration_ids(
    matrix: &MatrixDocument,
    violations: &mut Vec<String>,
) -> BTreeSet<String> {
    let mut ids = BTreeSet::new();
    for (index, configuration) in matrix.configurations.iter().enumerate() {
        if configuration.id.trim().is_empty() {
            violations.push(format!("matrix configuration {index} id must be nonblank"));
        }
        if !ids.insert(configuration.id.clone()) {
            violations.push(format!(
                "matrix contains duplicate configuration id {:?}",
                configuration.id
            ));
        }
    }
    ids
}

fn declared_configuration_ids(
    manifest: &CandidateManifest,
    matrix_ids: &BTreeSet<String>,
    violations: &mut Vec<String>,
) -> BTreeSet<String> {
    if manifest.configuration_ids.is_empty() {
        violations.push("candidate configuration_ids must be nonempty".into());
    }
    let mut ids = BTreeSet::new();
    for (index, id) in manifest.configuration_ids.iter().enumerate() {
        if id.trim().is_empty() {
            violations.push(format!(
                "candidate configuration_ids[{index}] must be nonblank"
            ));
        }
        if !ids.insert(id.clone()) {
            violations.push(format!(
                "candidate configuration_ids contains duplicate id {id:?}"
            ));
        }
        if !matrix_ids.contains(id) {
            violations.push(format!(
                "candidate configuration_id {id:?} is not present in the matrix"
            ));
        }
    }
    ids
}

fn is_git_sha(value: &str) -> bool {
    crate::completion::hash::is_git_sha(value)
}

fn is_sha256(value: &str) -> bool {
    crate::completion::hash::is_sha256(value)
}

fn is_portable_provenance_path(value: &str) -> bool {
    !value.is_empty()
        && !value.starts_with('/')
        && !value.contains(['\\', ':'])
        && !value.chars().any(|character| character.is_ascii_control())
        && value
            .split('/')
            .all(|segment| !segment.is_empty() && segment != "." && segment != "..")
}
