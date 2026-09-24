//! Filesystem discovery and composition for completion evidence.

use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use serde::de::DeserializeOwned;

use crate::completion::artifact_files::{ArtifactValidation, validate_declared_artifact};
use crate::completion::candidate::validate_candidate_manifest;
use crate::completion::identity_receipts::{
    BuildProvenanceReceipt, RunningIdentityReceipt, validate_identity_receipts,
};
use crate::completion::links::validate_metadata_links;
use crate::completion::run_artifacts::validate_run_artifact_files;
use crate::completion::schema::{
    CandidateManifest, EvidenceLayer, EvidenceResult, EvidenceRun, MatrixDocument,
    RequirementsDocument, ScenariosDocument,
};

const EVIDENCE_REL: &str = "plans/evidence/completion";

#[derive(Clone, Debug)]
struct DiscoveredRun {
    directory_name: String,
    directory: PathBuf,
    run: EvidenceRun,
}

/// Load every strictly parsed run whose candidate SHA matches candidate.
///
/// All discovered run records are parsed before candidate selection. A missing
/// evidence root is an empty development runset.
pub fn load_evidence_runs(root: &Path, candidate: &str) -> Result<Vec<EvidenceRun>, String> {
    Ok(discover_runs(root)?
        .into_iter()
        .filter(|record| record.run.candidate_sha == candidate)
        .map(|record| record.run)
        .collect())
}

/// Validate the selected candidate's registers, evidence metadata, attachments,
/// and identity receipts.
pub fn validate_evidence_files(root: &Path, candidate: &str) -> Result<Vec<String>, String> {
    let requirements: RequirementsDocument = load_json(root, "plans/completion/requirements.json")?;
    let matrix: MatrixDocument = load_json(root, "plans/completion/matrix.json")?;
    let scenarios: ScenariosDocument = load_json(root, "plans/completion/scenarios.json")?;
    let manifest: CandidateManifest = load_json(root, "plans/completion/candidate.json")?;

    let mut issues = validate_candidate_manifest(&manifest, &matrix, candidate);
    let records = discover_runs(root)?;
    let selected: Vec<_> = records
        .iter()
        .filter(|record| record.run.candidate_sha == candidate)
        .collect();
    let runs: Vec<_> = selected.iter().map(|record| record.run.clone()).collect();

    for record in &records {
        if !is_single_component_name(&record.directory_name) {
            issues.push(format!(
                "evidence run directory {} is not a portable single component",
                record.directory_name
            ));
        }
        if record.directory_name != record.run.id {
            issues.push(format!(
                "evidence run directory {} does not match run id {}",
                record.directory_name, record.run.id
            ));
        }
    }

    issues.extend(validate_metadata_links(
        &requirements,
        &matrix,
        &scenarios,
        &runs,
        candidate,
        false,
    ));

    for record in selected {
        let artifact_issues = validate_run_artifact_files(root, &record.run)?;
        issues.extend(artifact_issues.iter().cloned());
        let identity_path = format!("{EVIDENCE_REL}/{}/identity.json", record.run.id);
        let identity_declared = record.run.artifact_hashes.contains_key(&identity_path);
        let identity_present = path_exists(&record.directory.join("identity.json"))?;
        let identity_required = record.run.layer == EvidenceLayer::Product
            && record.run.result == EvidenceResult::Passed;
        if identity_required && !identity_present {
            issues.push(format!(
                "run {} passed product evidence requires identity.json",
                record.run.id
            ));
        }
        if identity_required || identity_declared || identity_present {
            issues.extend(validate_identity_files(
                root,
                &manifest,
                &record.run,
                &identity_path,
                identity_declared,
                identity_present,
            )?);
        }
    }

    issues.sort();
    issues.dedup();
    Ok(issues)
}

fn load_json<T: DeserializeOwned>(root: &Path, relative: &str) -> Result<T, String> {
    let path = root.join(relative);
    let bytes = fs::read(&path).map_err(|error| format!("{relative}: {error}"))?;
    serde_json::from_slice(&bytes).map_err(|error| format!("{relative}: {error}"))
}

fn discover_runs(root: &Path) -> Result<Vec<DiscoveredRun>, String> {
    let evidence_input = root.join(EVIDENCE_REL);
    let evidence_root = match fs::canonicalize(&evidence_input) {
        Ok(path) => {
            if !path.is_dir() {
                return Err(format!("{EVIDENCE_REL}: evidence root is not a directory"));
            }
            path
        }
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(format!("{EVIDENCE_REL}: {error}")),
    };

    let mut entries = fs::read_dir(&evidence_root)
        .map_err(|error| format!("{EVIDENCE_REL}: {error}"))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("{EVIDENCE_REL}: {error}"))?;
    entries.sort_by_key(|entry| entry.file_name());
    let mut records = Vec::new();
    for entry in entries {
        let file_type = entry
            .file_type()
            .map_err(|error| format!("{}: {error}", entry.path().display()))?;
        if file_type.is_symlink() {
            return Err(format!(
                "{}: symlinked evidence run directory is not allowed",
                entry.path().display()
            ));
        }
        if !file_type.is_dir() {
            continue;
        }
        let directory = entry.path();
        let run_json = directory.join("run.json");
        let run_metadata = match fs::symlink_metadata(&run_json) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == ErrorKind::NotFound => continue,
            Err(error) => return Err(format!("{}: {error}", run_json.display())),
        };
        if run_metadata.file_type().is_symlink() {
            return Err(format!(
                "{}: symlinked run.json is not allowed",
                run_json.display()
            ));
        }
        let canonical_run = fs::canonicalize(&run_json)
            .map_err(|error| format!("{}: {error}", run_json.display()))?;
        if !canonical_run.starts_with(&evidence_root) || !canonical_run.is_file() {
            return Err(format!(
                "{}: run.json escapes the evidence subtree or is not a file",
                run_json.display()
            ));
        }
        let run: EvidenceRun = read_json_file(&canonical_run)?;
        records.push(DiscoveredRun {
            directory_name: entry.file_name().to_string_lossy().into_owned(),
            directory,
            run,
        });
    }
    Ok(records)
}

fn validate_identity_files(
    root: &Path,
    manifest: &CandidateManifest,
    run: &EvidenceRun,
    identity_path: &str,
    identity_declared: bool,
    identity_present: bool,
) -> Result<Vec<String>, String> {
    let mut issues = Vec::new();
    if !identity_declared {
        if identity_present {
            issues.push(format!(
                "run {} identity.json must be declared in artifact_hashes",
                run.id
            ));
        }
        return Ok(issues);
    }
    let identity = match read_verified_json::<RunningIdentityReceipt>(root, run, identity_path)? {
        Some(receipt) => receipt,
        None => {
            issues.push(format!(
                "run {} identity.json attachment failed validation",
                run.id
            ));
            return Ok(issues);
        }
    };

    let matches: Vec<_> = manifest
        .artifacts
        .iter()
        .filter(|artifact| {
            artifact.component == identity.component
                && artifact.configuration_id == identity.configuration_id
        })
        .collect();
    if matches.len() != 1 {
        issues.push(format!(
            "run {} identity receipt component/configuration does not resolve to exactly one candidate artifact",
            run.id
        ));
        return Ok(issues);
    }
    let build_path = &matches[0].build_provenance_path;
    if !run.artifact_hashes.contains_key(build_path) {
        issues.push(format!(
            "run {} candidate build_provenance_path is not declared in artifact_hashes",
            run.id
        ));
        return Ok(issues);
    }
    let Some(build) = read_verified_build_receipt(root, run, build_path)? else {
        issues.push(format!(
            "run {} build provenance attachment failed validation",
            run.id
        ));
        return Ok(issues);
    };
    issues.extend(validate_identity_receipts(manifest, run, &build, &identity));
    Ok(issues)
}

fn read_verified_build_receipt(
    root: &Path,
    run: &EvidenceRun,
    path: &str,
) -> Result<Option<BuildProvenanceReceipt>, String> {
    let Some(declared_hash) = run.artifact_hashes.get(path) else {
        return Ok(None);
    };
    let validation =
        validate_declared_artifact(root, &root.join(EVIDENCE_REL), path, declared_hash)
            .map_err(|error| format!("run {} build receipt {path}: {error}", run.id))?;
    if !matches!(validation, ArtifactValidation::Valid) {
        return Ok(None);
    }
    Ok(Some(read_contained_json(root, path).map_err(|error| {
        format!("run {} build receipt {path}: {error}", run.id)
    })?))
}

fn read_verified_json<T: DeserializeOwned>(
    root: &Path,
    run: &EvidenceRun,
    path: &str,
) -> Result<Option<T>, String> {
    let Some(declared_hash) = run.artifact_hashes.get(path) else {
        return Ok(None);
    };
    let validation =
        validate_declared_artifact(root, &root.join(EVIDENCE_REL), path, declared_hash)
            .map_err(|error| format!("run {} attachment {path}: {error}", run.id))?;
    if !matches!(validation, ArtifactValidation::Valid) {
        return Ok(None);
    }
    Ok(Some(read_contained_json(root, path).map_err(|error| {
        format!("run {} attachment {path}: {error}", run.id)
    })?))
}

fn read_contained_json<T: DeserializeOwned>(root: &Path, relative: &str) -> Result<T, String> {
    let evidence_root = fs::canonicalize(root.join(EVIDENCE_REL))
        .map_err(|error| format!("evidence root: {error}"))?;
    if !is_evidence_attachment_path(relative) {
        return Err("path is not a portable evidence attachment path".into());
    }
    let path = root.join(relative);
    let metadata = fs::symlink_metadata(&path).map_err(|error| error.to_string())?;
    if metadata.file_type().is_symlink() {
        return Err("symlinked evidence attachment is not allowed".into());
    }
    let canonical = fs::canonicalize(&path).map_err(|error| error.to_string())?;
    if !canonical.starts_with(&evidence_root) || !canonical.is_file() {
        return Err("evidence attachment escapes the evidence subtree or is not a file".into());
    }
    read_json_file(&canonical)
}

fn read_json_file<T: DeserializeOwned>(path: &Path) -> Result<T, String> {
    let bytes = fs::read(path).map_err(|error| format!("{}: {error}", path.display()))?;
    serde_json::from_slice(&bytes).map_err(|error| format!("{}: {error}", path.display()))
}

fn path_exists(path: &Path) -> Result<bool, String> {
    match fs::symlink_metadata(path) {
        Ok(_) => Ok(true),
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(false),
        Err(error) => Err(format!("{}: {error}", path.display())),
    }
}

fn is_single_component_name(value: &str) -> bool {
    !value.is_empty()
        && !value.chars().any(|character| character.is_ascii_control())
        && !value.contains(['/', '\\', ':'])
        && value != "."
        && value != ".."
}

fn is_evidence_attachment_path(value: &str) -> bool {
    value.starts_with("plans/evidence/completion/")
        && !value.starts_with('/')
        && !value.contains(['\\', ':'])
        && !value.chars().any(|character| character.is_ascii_control())
        && value
            .split('/')
            .all(|segment| !segment.is_empty() && segment != "." && segment != "..")
}
