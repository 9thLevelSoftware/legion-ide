//! Composition of declared evidence attachments for one completion run.
//!
//! This validates the bytes of every attachment declared by an [`EvidenceRun`]
//! by composing the bounded path and streaming-hash primitive in
//! [`super::artifact_files`]. It does not establish receipt authenticity,
//! running identity, or release qualification. The checkout is assumed
//! quiescent for the duration of validation, as required by the primitive.

use std::path::Path;

use super::artifact_files::{
    ArtifactInvalidReason, ArtifactValidation, validate_declared_artifact,
};
use super::schema::EvidenceRun;

/// Validate every declared evidence attachment for `run`.
///
/// `root` is the repository root. Artifact paths in `run.artifact_hashes` are
/// repository-relative and must resolve beneath the canonical
/// `plans/evidence/completion` subtree. The returned messages are deterministic
/// because the schema stores declarations in a `BTreeMap`; invalid declarations
/// and operational filesystem failures are both reported with the run and path
/// context. An empty vector means that every declared attachment was validated.
/// Operational filesystem failures are returned as `Err`; malformed or
/// mismatched evidence is returned as `Ok` violations.
pub fn validate_run_artifact_files(root: &Path, run: &EvidenceRun) -> Result<Vec<String>, String> {
    let evidence_subtree = root.join("plans/evidence/completion");
    let mut issues = Vec::new();

    for (path, declared_sha256) in &run.artifact_hashes {
        match validate_declared_artifact(root, &evidence_subtree, path, declared_sha256) {
            Ok(ArtifactValidation::Valid) => {}
            Ok(ArtifactValidation::Invalid(reason)) => {
                issues.push(format_invalid_issue(&run.id, path, reason))
            }
            Err(error) => {
                return Err(format!(
                    "run `{}` artifact `{}`: operational error while validating attachment: {}",
                    run.id, path, error
                ));
            }
        }
    }

    Ok(issues)
}

fn format_invalid_issue(run_id: &str, path: &str, reason: ArtifactInvalidReason) -> String {
    let detail = match reason {
        ArtifactInvalidReason::InvalidSha256 => "declared SHA-256 is invalid",
        ArtifactInvalidReason::Missing => "attachment is missing",
        ArtifactInvalidReason::EvidenceRootNotDirectory => "evidence root is not a directory",
        ArtifactInvalidReason::Directory => "attachment resolves to a directory",
        ArtifactInvalidReason::OutsideEvidenceSubtree => {
            "attachment resolves outside the evidence subtree"
        }
        ArtifactInvalidReason::HashMismatch => "attachment SHA-256 does not match declaration",
        ArtifactInvalidReason::InvalidPath(reason) => {
            return format!("run `{run_id}` artifact `{path}`: invalid attachment path: {reason}");
        }
    };
    format!("run `{run_id}` artifact `{path}`: {detail}")
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::time::{SystemTime, UNIX_EPOCH};

    use sha2::{Digest, Sha256};

    use super::*;
    use crate::completion::schema::{
        DependencyObservation, EvidenceLayer, EvidenceResult, InputRoute, ReviewDecision,
    };

    fn temp_root() -> PathBuf {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock is before UNIX epoch")
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "legion-run-artifacts-{}-{suffix}",
            std::process::id()
        ));
        fs::create_dir_all(root.join("plans/evidence/completion/run-1")).unwrap();
        root
    }

    fn digest(bytes: &[u8]) -> String {
        hex::encode(Sha256::digest(bytes))
    }

    fn run(artifact_hashes: BTreeMap<String, String>, result: EvidenceResult) -> EvidenceRun {
        EvidenceRun {
            schema_version: 1,
            id: "run-1".into(),
            scenario_id: "scenario-1".into(),
            configuration_id: "config-1".into(),
            candidate_sha: "a".repeat(40),
            artifact_sha256: digest(b"candidate"),
            layer: EvidenceLayer::Component,
            input_route: InputRoute::None,
            dependencies: Vec::<DependencyObservation>::new(),
            result,
            oracle_results: Vec::new(),
            recovery_results: Vec::new(),
            artifact_hashes,
            defect_ids: Vec::new(),
            implementation_owners: Vec::new(),
            reviewer: String::new(),
            review_decision: ReviewDecision::ChangesRequired,
            started_at_utc: "2026-01-01T00:00:00Z".into(),
            ended_at_utc: "2026-01-01T00:00:01Z".into(),
        }
    }

    fn attachment(path: &str) -> String {
        format!("plans/evidence/completion/run-1/{path}")
    }

    fn cleanup(root: &Path) {
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn validates_actual_bytes_for_declared_nested_attachment() {
        let root = temp_root();
        let path = root.join("plans/evidence/completion/run-1/result.txt");
        fs::write(&path, b"actual evidence").unwrap();
        let mut hashes = BTreeMap::new();
        hashes.insert(attachment("result.txt"), digest(b"actual evidence"));

        assert!(
            validate_run_artifact_files(&root, &run(hashes, EvidenceResult::Passed))
                .unwrap()
                .is_empty()
        );
        cleanup(&root);
    }

    #[test]
    fn rejects_altered_attachment_bytes() {
        let root = temp_root();
        let path = root.join("plans/evidence/completion/run-1/result.txt");
        fs::write(&path, b"altered").unwrap();
        let mut hashes = BTreeMap::new();
        hashes.insert(attachment("result.txt"), digest(b"original"));

        let issues =
            validate_run_artifact_files(&root, &run(hashes, EvidenceResult::Passed)).unwrap();
        assert_eq!(issues.len(), 1);
        assert!(
            issues[0].contains("run `run-1` artifact `plans/evidence/completion/run-1/result.txt`")
        );
        assert!(issues[0].contains("does not match declaration"));
        cleanup(&root);
    }

    #[test]
    fn rejects_absent_attachment() {
        let root = temp_root();
        let mut hashes = BTreeMap::new();
        hashes.insert(attachment("missing.txt"), digest(b"missing"));

        let issues =
            validate_run_artifact_files(&root, &run(hashes, EvidenceResult::Passed)).unwrap();
        assert_eq!(issues.len(), 1);
        assert!(issues[0].contains("attachment is missing"));
        cleanup(&root);
    }

    #[test]
    fn rejects_invalid_declared_hash() {
        let root = temp_root();
        fs::write(
            root.join("plans/evidence/completion/run-1/result.txt"),
            b"evidence",
        )
        .unwrap();
        let mut hashes = BTreeMap::new();
        hashes.insert(attachment("result.txt"), "not-a-sha256".into());

        let issues =
            validate_run_artifact_files(&root, &run(hashes, EvidenceResult::Passed)).unwrap();
        assert_eq!(issues.len(), 1);
        assert!(issues[0].contains("declared SHA-256 is invalid"));
        cleanup(&root);
    }

    #[test]
    fn rejects_traversal_and_outside_evidence_root_paths() {
        let root = temp_root();
        fs::write(root.join("outside.txt"), b"outside").unwrap();
        let mut hashes = BTreeMap::new();
        hashes.insert("../outside.txt".into(), digest(b"outside"));
        hashes.insert("outside.txt".into(), digest(b"outside"));

        let issues =
            validate_run_artifact_files(&root, &run(hashes, EvidenceResult::Passed)).unwrap();
        assert_eq!(issues.len(), 2);
        assert!(
            issues
                .iter()
                .any(|issue| issue.contains("parent traversal is not allowed"))
        );
        assert!(
            issues
                .iter()
                .any(|issue| issue.contains("outside the evidence subtree"))
        );
        cleanup(&root);
    }

    #[test]
    fn validates_unused_declared_attachment_and_rejects_when_altered() {
        let root = temp_root();
        fs::write(
            root.join("plans/evidence/completion/run-1/used.txt"),
            b"used",
        )
        .unwrap();
        fs::write(
            root.join("plans/evidence/completion/run-1/unused.txt"),
            b"altered unused",
        )
        .unwrap();
        let mut hashes = BTreeMap::new();
        hashes.insert(attachment("used.txt"), digest(b"used"));
        hashes.insert(attachment("unused.txt"), digest(b"original unused"));

        let issues =
            validate_run_artifact_files(&root, &run(hashes, EvidenceResult::Passed)).unwrap();
        assert_eq!(issues.len(), 1);
        assert!(issues[0].contains("unused.txt"));
        assert!(issues[0].contains("does not match declaration"));
        cleanup(&root);
    }

    #[test]
    fn failed_and_blocked_runs_still_reject_malformed_present_attachment() {
        let root = temp_root();
        fs::write(
            root.join("plans/evidence/completion/run-1/result.txt"),
            b"present",
        )
        .unwrap();
        for result in [EvidenceResult::Failed, EvidenceResult::Blocked] {
            let mut hashes = BTreeMap::new();
            hashes.insert(attachment("result.txt"), digest(b"wrong"));
            let issues = validate_run_artifact_files(&root, &run(hashes, result)).unwrap();
            assert_eq!(issues.len(), 1);
            assert!(issues[0].contains("does not match declaration"));
        }
        cleanup(&root);
    }

    #[test]
    fn propagates_operational_root_failure_as_err() {
        let root = std::env::temp_dir().join(format!(
            "legion-run-artifacts-missing-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let mut hashes = BTreeMap::new();
        hashes.insert(attachment("result.txt"), digest(b"evidence"));

        let error = validate_run_artifact_files(&root, &run(hashes, EvidenceResult::Passed))
            .expect_err("an unresolvable repository root is operational failure");
        assert!(error.contains("operational error while validating attachment"));
    }
}
