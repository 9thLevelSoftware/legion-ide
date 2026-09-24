//! Strict build and running identity receipts for completion evidence.
//!
//! Receipts bind independently collected metadata to the nominated candidate
//! and an [`EvidenceRun`].  They do not authenticate the observer or claim
//! that an external process was honestly observed.

use std::collections::BTreeMap;

use serde::{Deserialize, Deserializer, Serialize};

use super::{
    artifact_files::parse_utc_timestamp,
    schema::{CandidateManifest, EvidenceRun},
};

fn schema_version<'de, D>(deserializer: D) -> Result<u32, D::Error>
where
    D: Deserializer<'de>,
{
    let version = u32::deserialize(deserializer)?;
    if version == 1 {
        Ok(version)
    } else {
        Err(serde::de::Error::custom("schema_version must be 1"))
    }
}

/// Metadata captured by the independent build observer.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BuildProvenanceReceipt {
    #[serde(deserialize_with = "schema_version")]
    pub schema_version: u32,
    pub candidate_sha: String,
    pub verifier_sha: String,
    pub component: String,
    pub configuration_id: String,
    pub artifact_sha256: String,
    pub build_command: String,
    pub tool_versions: BTreeMap<String, String>,
    pub capture_artifact_path: String,
    pub captured_at_utc: String,
    pub observer: String,
}

/// Identity observed for a running executable or service image.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum RunningIdentity {
    Process {
        pid: u32,
        start_token: String,
    },
    Service {
        instance_id: String,
        image_digest: String,
    },
}

/// Metadata captured by an independent runtime observer.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RunningIdentityReceipt {
    #[serde(deserialize_with = "schema_version")]
    pub schema_version: u32,
    pub run_id: String,
    pub candidate_sha: String,
    pub component: String,
    pub configuration_id: String,
    pub observed_sha256: String,
    pub observed_version: String,
    pub observed_path: String,
    pub observer: String,
    pub captured_at_utc: String,
    pub capture_artifact_path: String,
    pub identity: RunningIdentity,
}

/// Validate receipt metadata against a nominated candidate and one evidence run.
///
/// This is a consistency check over declared metadata and hashes. It does not
/// read external installations, execute observers, or establish authenticity.
pub fn validate_identity_receipts(
    manifest: &CandidateManifest,
    run: &EvidenceRun,
    build: &BuildProvenanceReceipt,
    running: &RunningIdentityReceipt,
) -> Vec<String> {
    let mut issues = Vec::new();
    let matches: Vec<_> = manifest
        .artifacts
        .iter()
        .filter(|artifact| {
            artifact.component == build.component
                && artifact.configuration_id == build.configuration_id
        })
        .collect();
    if matches.is_empty() {
        issues.push("receipt component/configuration does not match a candidate artifact".into());
    } else if matches.len() > 1 {
        issues.push("receipt component/configuration matches multiple candidate artifacts".into());
    }

    if !is_git_sha(&manifest.code_sha) {
        issues.push("manifest code_sha must be exactly 40 lowercase hexadecimal characters".into());
    }
    if build.candidate_sha != manifest.code_sha {
        issues.push("build receipt candidate_sha does not match manifest code_sha".into());
    }
    if run.candidate_sha != manifest.code_sha {
        issues.push("evidence run candidate_sha does not match manifest code_sha".into());
    }
    if !is_git_sha(&build.verifier_sha) {
        issues.push(
            "build receipt verifier_sha must be exactly 40 lowercase hexadecimal characters".into(),
        );
    }
    if !is_sha256(&build.artifact_sha256) {
        issues.push(
            "build receipt artifact_sha256 must be exactly 64 ASCII hexadecimal characters".into(),
        );
    }
    if !is_sha256(&run.artifact_sha256) {
        issues.push(
            "evidence run artifact_sha256 must be exactly 64 ASCII hexadecimal characters".into(),
        );
    }
    if !is_sha256(&running.observed_sha256) {
        issues.push(
            "running receipt observed_sha256 must be exactly 64 ASCII hexadecimal characters"
                .into(),
        );
    }
    if let Some(artifact) = matches.first().filter(|_| matches.len() == 1) {
        if !equal_hex(&artifact.sha256, &run.artifact_sha256)
            || !equal_hex(&artifact.sha256, &build.artifact_sha256)
            || !equal_hex(&artifact.sha256, &running.observed_sha256)
        {
            issues.push("candidate, run, build, and running artifact hashes do not match".into());
        }
        if running.component != artifact.component {
            issues.push("running receipt component does not match candidate artifact".into());
        }
        if running.configuration_id != artifact.configuration_id {
            issues
                .push("running receipt configuration_id does not match candidate artifact".into());
        }
        if run.configuration_id != artifact.configuration_id {
            issues.push("evidence run configuration_id does not match candidate artifact".into());
        }
        if !declared_attachment(&run.artifact_hashes, &artifact.build_provenance_path) {
            issues.push(
                "candidate build_provenance_path is not declared in run artifact_hashes".into(),
            );
        }
    }

    if running.run_id != run.id {
        issues.push("running receipt run_id does not match evidence run id".into());
    }
    if running.candidate_sha != run.candidate_sha {
        issues.push("running receipt candidate_sha does not match evidence run".into());
    }
    if !is_attachment_path(&build.capture_artifact_path) {
        issues
            .push("build capture_artifact_path must be a portable evidence attachment path".into());
    } else if matches.len() == 1 && build.capture_artifact_path == matches[0].build_provenance_path
    {
        issues.push(
            "build capture_artifact_path must be distinct from candidate build_provenance_path"
                .into(),
        );
    } else if !declared_attachment(&run.artifact_hashes, &build.capture_artifact_path) {
        issues.push("build capture_artifact_path is not declared in run artifact_hashes".into());
    }
    if !is_attachment_path(&running.capture_artifact_path) {
        issues.push(
            "running capture_artifact_path must be a portable evidence attachment path".into(),
        );
    } else if !declared_attachment(&run.artifact_hashes, &running.capture_artifact_path) {
        issues.push("running capture_artifact_path is not declared in run artifact_hashes".into());
    }

    for (name, value) in [
        ("build component", build.component.as_str()),
        ("build configuration_id", build.configuration_id.as_str()),
        ("build command", build.build_command.as_str()),
        ("build observer", build.observer.as_str()),
        ("running component", running.component.as_str()),
        (
            "running configuration_id",
            running.configuration_id.as_str(),
        ),
        (
            "running observed_version",
            running.observed_version.as_str(),
        ),
        ("running observed_path", running.observed_path.as_str()),
        ("running observer", running.observer.as_str()),
    ] {
        if value.trim().is_empty() {
            issues.push(format!("{name} must be nonblank"));
        }
    }
    if build.tool_versions.is_empty()
        || build
            .tool_versions
            .iter()
            .any(|(name, version)| name.trim().is_empty() || version.trim().is_empty())
    {
        issues.push("build tool_versions must contain nonblank names and versions".into());
    }

    let build_time =
        parse_receipt_time("build captured_at_utc", &build.captured_at_utc, &mut issues);
    let start_time = parse_receipt_time("run started_at_utc", &run.started_at_utc, &mut issues);
    let end_time = parse_receipt_time("run ended_at_utc", &run.ended_at_utc, &mut issues);
    let running_time = parse_receipt_time(
        "running captured_at_utc",
        &running.captured_at_utc,
        &mut issues,
    );
    if let (Some(build_time), Some(start_time)) = (build_time, start_time)
        && build_time > start_time
    {
        issues.push("build captured_at_utc must be no later than run started_at_utc".into());
    }
    if let (Some(running_time), Some(start_time), Some(end_time)) =
        (running_time, start_time, end_time)
        && (running_time < start_time || running_time > end_time)
    {
        issues.push("running captured_at_utc must be within the evidence run interval".into());
    }

    match &running.identity {
        RunningIdentity::Process { pid, start_token } => {
            if *pid == 0 {
                issues.push("process identity pid must be nonzero".into());
            }
            if start_token.trim().is_empty() {
                issues.push("process identity start_token must be nonblank".into());
            }
        }
        RunningIdentity::Service {
            instance_id,
            image_digest,
        } => {
            if instance_id.trim().is_empty() {
                issues.push("service identity instance_id must be nonblank".into());
            }
            match image_digest.strip_prefix("sha256:") {
                Some(expected) if is_sha256(expected) => {
                    if !equal_hex(expected, &running.observed_sha256) {
                        issues.push(
                            "service identity image_digest must match observed_sha256".into(),
                        );
                    }
                }
                _ => {
                    issues.push("service identity image_digest must be sha256:<64 ASCII hexadecimal characters>".into());
                }
            }
        }
    }

    issues.sort();
    issues.dedup();
    issues
}

fn parse_receipt_time(
    label: &str,
    value: &str,
    issues: &mut Vec<String>,
) -> Option<super::artifact_files::UtcTimestamp> {
    match parse_utc_timestamp(value) {
        Ok(timestamp) => Some(timestamp),
        Err(_) => {
            issues.push(format!("{label} must be a valid UTC timestamp"));
            None
        }
    }
}

fn is_git_sha(value: &str) -> bool {
    crate::completion::hash::is_git_sha(value)
}

fn is_sha256(value: &str) -> bool {
    crate::completion::hash::is_sha256(value)
}

fn equal_hex(left: &str, right: &str) -> bool {
    left.eq_ignore_ascii_case(right)
}

fn is_attachment_path(value: &str) -> bool {
    value.starts_with("plans/evidence/completion/")
        && !value.starts_with('/')
        && !value.contains(['\\', ':'])
        && !value.chars().any(|character| character.is_ascii_control())
        && value
            .split('/')
            .all(|segment| !segment.is_empty() && segment != "." && segment != "..")
}

fn declared_attachment(hashes: &BTreeMap<String, String>, path: &str) -> bool {
    hashes.get(path).is_some_and(|hash| is_sha256(hash))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::completion::schema::{
        CandidateArtifact, EvidenceLayer, EvidenceResult, InputRoute, ReviewDecision,
    };

    const CANDIDATE: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    const VERIFIER: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
    const HASH: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
    const START: &str = "2026-01-01T00:00:00Z";
    const END: &str = "2026-01-01T00:00:10Z";

    fn manifest() -> CandidateManifest {
        CandidateManifest {
            schema_version: 1,
            code_sha: CANDIDATE.into(),
            artifacts: vec![CandidateArtifact {
                component: "desktop".into(),
                configuration_id: "windows-x64".into(),
                path: "dist/legion.exe".into(),
                sha256: HASH.into(),
                build_provenance_path: "plans/evidence/completion/run-1/build-receipt.json".into(),
            }],
            configuration_ids: vec!["windows-x64".into()],
            nominated_at_utc: START.into(),
            nominated_by: "owner".into(),
        }
    }

    fn run() -> EvidenceRun {
        let mut hashes = BTreeMap::new();
        hashes.insert(
            "plans/evidence/completion/run-1/build-receipt.json".into(),
            HASH.into(),
        );
        hashes.insert(
            "plans/evidence/completion/run-1/build-observer.log".into(),
            HASH.into(),
        );
        hashes.insert(
            "plans/evidence/completion/run-1/runtime.json".into(),
            HASH.into(),
        );
        EvidenceRun {
            schema_version: 1,
            id: "run-1".into(),
            scenario_id: "scenario".into(),
            configuration_id: "windows-x64".into(),
            candidate_sha: CANDIDATE.into(),
            artifact_sha256: HASH.into(),
            layer: EvidenceLayer::Product,
            input_route: InputRoute::NativeInput,
            dependencies: vec![],
            result: EvidenceResult::Passed,
            oracle_results: vec![],
            recovery_results: vec![],
            artifact_hashes: hashes,
            defect_ids: vec![],
            implementation_owners: vec!["builder".into()],
            reviewer: "reviewer".into(),
            review_decision: ReviewDecision::Accepted,
            started_at_utc: START.into(),
            ended_at_utc: END.into(),
        }
    }

    fn build() -> BuildProvenanceReceipt {
        BuildProvenanceReceipt {
            schema_version: 1,
            candidate_sha: CANDIDATE.into(),
            verifier_sha: VERIFIER.into(),
            component: "desktop".into(),
            configuration_id: "windows-x64".into(),
            artifact_sha256: HASH.into(),
            build_command: "cargo build".into(),
            tool_versions: BTreeMap::from([("rustc".into(), "1.92".into())]),
            capture_artifact_path: "plans/evidence/completion/run-1/build-observer.log".into(),
            captured_at_utc: "2025-12-31T23:59:59Z".into(),
            observer: "builder".into(),
        }
    }

    fn running(identity: RunningIdentity) -> RunningIdentityReceipt {
        RunningIdentityReceipt {
            schema_version: 1,
            run_id: "run-1".into(),
            candidate_sha: CANDIDATE.into(),
            component: "desktop".into(),
            configuration_id: "windows-x64".into(),
            observed_sha256: HASH.into(),
            observed_version: "1.0.0".into(),
            observed_path: "C:/installed/legion.exe".into(),
            observer: "operator".into(),
            captured_at_utc: "2026-01-01T00:00:05Z".into(),
            capture_artifact_path: "plans/evidence/completion/run-1/runtime.json".into(),
            identity,
        }
    }

    #[test]
    fn accepts_process_and_service_receipts_and_distinct_verifier() {
        let m = manifest();
        let r = run();
        let mut b = build();
        b.verifier_sha = VERIFIER.into();
        assert!(
            validate_identity_receipts(
                &m,
                &r,
                &b,
                &running(RunningIdentity::Process {
                    pid: 42,
                    start_token: "token".into()
                })
            )
            .is_empty()
        );
        assert!(
            validate_identity_receipts(
                &m,
                &r,
                &b,
                &running(RunningIdentity::Service {
                    instance_id: "svc".into(),
                    image_digest: format!("sha256:{HASH}")
                })
            )
            .is_empty()
        );
    }

    #[test]
    fn accepts_uppercase_artifact_hashes_and_json_round_trips_for_both_identities() {
        let mut m = manifest();
        let mut r = run();
        let mut b = build();
        let mut i = running(RunningIdentity::Service {
            instance_id: "svc".into(),
            image_digest: format!("sha256:{HASH}"),
        });
        let upper = HASH.to_ascii_uppercase();
        m.artifacts[0].sha256 = upper.clone();
        r.artifact_sha256 = upper.clone();
        b.artifact_sha256 = upper.clone();
        i.observed_sha256 = upper.clone();
        i.identity = RunningIdentity::Service {
            instance_id: "svc".into(),
            image_digest: format!("sha256:{upper}"),
        };
        for hash in r.artifact_hashes.values_mut() {
            *hash = upper.clone();
        }
        assert!(validate_identity_receipts(&m, &r, &b, &i).is_empty());

        let process = running(RunningIdentity::Process {
            pid: 42,
            start_token: "token".into(),
        });
        let process_json = serde_json::to_string(&process).unwrap();
        let service_json = serde_json::to_string(&i).unwrap();
        assert_eq!(
            serde_json::from_str::<RunningIdentityReceipt>(&process_json).unwrap(),
            process
        );
        assert_eq!(
            serde_json::from_str::<RunningIdentityReceipt>(&service_json).unwrap(),
            i
        );
    }

    #[test]
    fn rejects_each_candidate_component_configuration_and_hash_binding_independently() {
        let base_manifest = manifest();
        let base_run = run();
        let base_build = build();
        let base_running = running(RunningIdentity::Process {
            pid: 42,
            start_token: "token".into(),
        });
        let mut altered_build = base_build.clone();
        altered_build.candidate_sha = "c".repeat(40);
        assert!(
            validate_identity_receipts(&base_manifest, &base_run, &altered_build, &base_running)
                .iter()
                .any(|issue| issue.contains("build receipt candidate_sha"))
        );

        let mut altered_run = base_run.clone();
        altered_run.candidate_sha = "c".repeat(40);
        assert!(
            validate_identity_receipts(&base_manifest, &altered_run, &base_build, &base_running)
                .iter()
                .any(|issue| issue.contains("evidence run candidate_sha"))
        );

        let mut altered_component = base_build.clone();
        altered_component.component = "helper".into();
        assert!(
            validate_identity_receipts(
                &base_manifest,
                &base_run,
                &altered_component,
                &base_running
            )
            .iter()
            .any(|issue| issue.contains("does not match a candidate artifact"))
        );

        let mut altered_configuration = base_build.clone();
        altered_configuration.configuration_id = "linux-x64".into();
        assert!(
            validate_identity_receipts(
                &base_manifest,
                &base_run,
                &altered_configuration,
                &base_running
            )
            .iter()
            .any(|issue| issue.contains("does not match a candidate artifact"))
        );

        let mut altered_hash = base_build;
        altered_hash.artifact_sha256 = "f".repeat(64);
        assert!(
            validate_identity_receipts(&base_manifest, &base_run, &altered_hash, &base_running)
                .iter()
                .any(|issue| issue.contains("hashes do not match"))
        );

        let mut altered_manifest = base_manifest;
        altered_manifest.artifacts[0].sha256 = "f".repeat(64);
        assert!(
            validate_identity_receipts(&altered_manifest, &base_run, &build(), &base_running)
                .iter()
                .any(|issue| issue.contains("hashes do not match"))
        );
    }

    #[test]
    fn rejects_hash_candidate_component_config_run_and_capture_mismatches() {
        let m = manifest();
        let mut r = run();
        let mut b = build();
        let mut i = running(RunningIdentity::Process {
            pid: 1,
            start_token: "t".into(),
        });
        r.candidate_sha = "c".repeat(40);
        b.artifact_sha256 = "f".repeat(64);
        i.run_id = "other".into();
        i.configuration_id = "other".into();
        r.artifact_hashes
            .remove("plans/evidence/completion/run-1/runtime.json");
        let issues = validate_identity_receipts(&m, &r, &b, &i);
        assert!(issues.iter().any(|v| v.contains("candidate_sha")));
        assert!(issues.iter().any(|v| v.contains("hashes do not match")));
        assert!(issues.iter().any(|v| v.contains("run_id")));
        assert!(
            issues
                .iter()
                .any(|v| v.contains("capture_artifact_path is not declared"))
        );
    }

    #[test]
    fn requires_provenance_and_build_capture_attachments_independently() {
        let m = manifest();
        let r = run();
        let b = build();
        let i = running(RunningIdentity::Process {
            pid: 42,
            start_token: "token".into(),
        });

        let mut without_provenance = r.clone();
        without_provenance
            .artifact_hashes
            .remove("plans/evidence/completion/run-1/build-receipt.json");
        let issues = validate_identity_receipts(&m, &without_provenance, &b, &i);
        assert!(
            issues
                .iter()
                .any(|v| v.contains("build_provenance_path is not declared"))
        );

        let mut without_capture = r;
        without_capture
            .artifact_hashes
            .remove("plans/evidence/completion/run-1/build-observer.log");
        let issues = validate_identity_receipts(&m, &without_capture, &b, &i);
        assert!(
            issues
                .iter()
                .any(|v| v.contains("build capture_artifact_path is not declared"))
        );

        let mut self_referential = build();
        self_referential.capture_artifact_path =
            "plans/evidence/completion/run-1/build-receipt.json".into();
        let issues = validate_identity_receipts(&m, &run(), &self_referential, &i);
        assert!(
            issues
                .iter()
                .any(|v| v.contains("must be distinct from candidate build_provenance_path"))
        );
    }

    #[test]
    fn rejects_timestamps_identity_kinds_and_service_digest() {
        let m = manifest();
        let mut r = run();
        let mut b = build();
        b.captured_at_utc = END.into();
        let i = running(RunningIdentity::Service {
            instance_id: "".into(),
            image_digest: "sha256:bad".into(),
        });
        let issues = validate_identity_receipts(&m, &r, &b, &i);
        assert!(
            issues
                .iter()
                .any(|v| v.contains("build captured_at_utc must be no later"))
        );
        assert!(issues.iter().any(|v| v.contains("instance_id")));
        assert!(issues.iter().any(|v| v.contains("image_digest")));
        r.started_at_utc = "bad".into();
        assert!(
            validate_identity_receipts(&m, &r, &build(), &i)
                .iter()
                .any(|v| v.contains("valid UTC"))
        );
        let p = running(RunningIdentity::Process {
            pid: 0,
            start_token: " ".into(),
        });
        let issues = validate_identity_receipts(&m, &run(), &build(), &p);
        assert!(issues.iter().any(|v| v.contains("pid")));
        assert!(issues.iter().any(|v| v.contains("start_token")));

        let mut before = run();
        before.started_at_utc = "2026-01-01T00:00:05Z".into();
        before.ended_at_utc = "2026-01-01T00:00:10Z".into();
        let mut late_build = build();
        late_build.captured_at_utc = "2026-01-01T00:00:06Z".into();
        assert!(
            validate_identity_receipts(
                &m,
                &before,
                &late_build,
                &running(RunningIdentity::Process {
                    pid: 1,
                    start_token: "t".into(),
                })
            )
            .iter()
            .any(|v| v.contains("build captured_at_utc must be no later"))
        );
        let mut before_runtime = running(RunningIdentity::Process {
            pid: 1,
            start_token: "t".into(),
        });
        before_runtime.captured_at_utc = "2025-12-31T23:59:59Z".into();
        assert!(
            validate_identity_receipts(&m, &run(), &build(), &before_runtime)
                .iter()
                .any(|v| v.contains("within the evidence run interval"))
        );
        let mut after = running(RunningIdentity::Process {
            pid: 1,
            start_token: "t".into(),
        });
        after.captured_at_utc = "2026-01-01T00:00:11Z".into();
        assert!(
            validate_identity_receipts(&m, &run(), &build(), &after)
                .iter()
                .any(|v| v.contains("within the evidence run interval"))
        );

        let mut blank_build = build();
        blank_build.build_command.clear();
        blank_build.observer.clear();
        blank_build.tool_versions.clear();
        let mut blank_running = running(RunningIdentity::Process {
            pid: 1,
            start_token: "t".into(),
        });
        blank_running.observed_version.clear();
        blank_running.observed_path.clear();
        blank_running.observer.clear();
        let issues = validate_identity_receipts(&m, &run(), &blank_build, &blank_running);
        assert!(
            issues
                .iter()
                .any(|v| v.contains("build command must be nonblank"))
        );
        assert!(
            issues
                .iter()
                .any(|v| v.contains("build observer must be nonblank"))
        );
        assert!(issues.iter().any(|v| v.contains("tool_versions")));
        assert!(
            issues
                .iter()
                .any(|v| v.contains("observed_version must be nonblank"))
        );
        assert!(
            issues
                .iter()
                .any(|v| v.contains("observed_path must be nonblank"))
        );
        assert!(
            issues
                .iter()
                .any(|v| v.contains("running observer must be nonblank"))
        );

        let mismatched_service = running(RunningIdentity::Service {
            instance_id: "svc".into(),
            image_digest: format!("sha256:{}", "f".repeat(64)),
        });
        assert!(
            validate_identity_receipts(&m, &run(), &build(), &mismatched_service)
                .iter()
                .any(|v| v.contains("image_digest must match observed_sha256"))
        );
    }

    #[test]
    fn strict_receipt_wire_schema_rejects_unknown_fields_and_versions() {
        let json = r#"{"schema_version":1,"candidate_sha":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","verifier_sha":"bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb","component":"desktop","configuration_id":"windows-x64","artifact_sha256":"0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef","build_command":"cargo build","tool_versions":{"rustc":"1.92"},"capture_artifact_path":"plans/evidence/completion/run-1/build-observer.log","captured_at_utc":"2025-12-31T23:59:59Z","observer":"builder","extra":true}"#;
        assert!(serde_json::from_str::<BuildProvenanceReceipt>(json).is_err());
        assert!(
            serde_json::from_str::<BuildProvenanceReceipt>(
                &json.replace("\"schema_version\":1", "\"schema_version\":2")
            )
            .is_err()
        );
        let running_json = r#"{"schema_version":1,"run_id":"run-1","candidate_sha":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","component":"desktop","configuration_id":"windows-x64","observed_sha256":"0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef","observed_version":"1.0","observed_path":"legion.exe","observer":"operator","captured_at_utc":"2026-01-01T00:00:05Z","capture_artifact_path":"plans/evidence/completion/run-1/runtime.json","identity":{"kind":"unknown","value":"x"}}"#;
        assert!(serde_json::from_str::<RunningIdentityReceipt>(running_json).is_err());
        let unknown_identity = running_json.replace(
            "\"kind\":\"unknown\",\"value\":\"x\"",
            "\"kind\":\"process\",\"pid\":1,\"start_token\":\"t\",\"extra\":true",
        );
        assert!(serde_json::from_str::<RunningIdentityReceipt>(&unknown_identity).is_err());
    }
}
