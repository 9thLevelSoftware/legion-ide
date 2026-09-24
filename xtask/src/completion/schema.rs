//! Canonical completion record wire types and strict structural parsing.
//!
//! This module intentionally performs no semantic validation, authenticity checks,
//! evidence qualification, or release/completion verdict computation. Those belong
//! to a later increment.

use std::collections::BTreeMap;

use serde::{Deserialize, Deserializer, Serialize};

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

macro_rules! wire_enum {
    ($(#[$meta:meta])* $name:ident { $($(#[$variant_meta:meta])* $variant:ident => $wire:literal),+ $(,)? }) => {
        $(#[$meta])*
        #[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
        #[serde(rename_all = "kebab-case")]
        pub enum $name { $($(#[$variant_meta])* #[serde(rename = $wire)] $variant),+ }
        impl std::fmt::Display for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str(match self { $(Self::$variant => $wire),+ })
            }
        }
    };
}

wire_enum! { RequirementKind { Product => "product", Internal => "internal" } }
wire_enum! { Implementation { Absent => "absent", Partial => "partial", Implemented => "implemented", RepairRequired => "repair-required" } }
wire_enum! { Acceptance { Unassessed => "unassessed", Blocked => "blocked", Failed => "failed", Accepted => "accepted" } }
wire_enum! { Stage { S0 => "S0", S1 => "S1", S2 => "S2", S3 => "S3", S4 => "S4", S5 => "S5", S6 => "S6" } }
wire_enum! { OperatingSystem { Windows => "windows", Macos => "macos", Linux => "linux" } }
wire_enum! { DependencyExecution { Real => "real", Substituted => "substituted" } }
wire_enum! { EvidenceLayer { Component => "component", Integrated => "integrated", Product => "product" } }
wire_enum! { InputRoute { NativeInput => "native-input", RuntimeDispatch => "runtime-dispatch", SnapshotInjection => "snapshot-injection", None => "none" } }
wire_enum! { EvidenceResult { Passed => "passed", Failed => "failed", Blocked => "blocked", Skipped => "skipped" } }
wire_enum! { ReviewDecision { Accepted => "accepted", ChangesRequired => "changes-required" } }
wire_enum! { DefectSeverity { P0 => "P0", P1 => "P1", P2 => "P2", P3 => "P3" } }
wire_enum! { DefectStatus { Open => "open", FixedAwaitingVerification => "fixed-awaiting-verification", Closed => "closed" } }

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourceRef {
    pub path: String,
    pub identity: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Requirement {
    pub id: String,
    pub title: String,
    pub kind: RequirementKind,
    pub required: bool,
    pub source_refs: Vec<SourceRef>,
    pub legacy_ids: Vec<String>,
    pub stage: Stage,
    pub package_id: String,
    pub owner_role: String,
    pub depends_on: Vec<String>,
    pub implementation: Implementation,
    pub acceptance: Acceptance,
    pub scenario_ids: Vec<String>,
    pub configuration_ids: Vec<String>,
    pub protected_product_ids: Vec<String>,
    pub defect_ids: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RequirementsDocument {
    #[serde(deserialize_with = "schema_version")]
    pub schema_version: u32,
    pub requirements: Vec<Requirement>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Configuration {
    pub id: String,
    pub os: OperatingSystem,
    pub architecture: String,
    pub tool_versions: BTreeMap<String, String>,
    pub hardware: String,
    pub project_category: String,
    pub required: bool,
    pub owner_approval_ref: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MatrixDocument {
    #[serde(deserialize_with = "schema_version")]
    pub schema_version: u32,
    pub configurations: Vec<Configuration>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ScenarioCheck {
    pub id: String,
    pub description: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Scenario {
    pub id: String,
    pub requirement_ids: Vec<String>,
    pub configuration_ids: Vec<String>,
    pub steps: Vec<String>,
    pub external_oracles: Vec<ScenarioCheck>,
    pub recovery_cases: Vec<ScenarioCheck>,
    pub sensitive_artifact_policy: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ScenariosDocument {
    #[serde(deserialize_with = "schema_version")]
    pub schema_version: u32,
    pub scenarios: Vec<Scenario>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DependencyObservation {
    pub name: String,
    pub version: String,
    pub execution: DependencyExecution,
    pub required_for_outcome: bool,
    pub substitution_reason: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CheckResult {
    pub id: String,
    pub passed: bool,
    pub artifact_path: String,
    pub observed: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EvidenceRun {
    #[serde(deserialize_with = "schema_version")]
    pub schema_version: u32,
    pub id: String,
    pub scenario_id: String,
    pub configuration_id: String,
    pub candidate_sha: String,
    pub artifact_sha256: String,
    pub layer: EvidenceLayer,
    pub input_route: InputRoute,
    pub dependencies: Vec<DependencyObservation>,
    pub result: EvidenceResult,
    pub oracle_results: Vec<CheckResult>,
    pub recovery_results: Vec<CheckResult>,
    pub artifact_hashes: BTreeMap<String, String>,
    pub defect_ids: Vec<String>,
    pub implementation_owners: Vec<String>,
    pub reviewer: String,
    pub review_decision: ReviewDecision,
    pub started_at_utc: String,
    pub ended_at_utc: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Defect {
    pub id: String,
    pub requirement_ids: Vec<String>,
    pub scenario_id: String,
    pub configuration_id: String,
    pub severity: DefectSeverity,
    pub invalidates_required_outcome: bool,
    pub reproduction: Vec<String>,
    pub expected: String,
    pub observed: String,
    pub owner: String,
    pub status: DefectStatus,
    pub repair_package_id: String,
    pub verification_run_ids: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DefectsDocument {
    #[serde(deserialize_with = "schema_version")]
    pub schema_version: u32,
    pub defects: Vec<Defect>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Milestone {
    pub id: String,
    pub depends_on: Vec<String>,
    pub deliverable_refs: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PackageDependency {
    pub package_id: String,
    pub requirement_ids: Vec<String>,
    pub milestones: Vec<Milestone>,
    pub external_prerequisites: Vec<String>,
    pub owner_role: String,
    pub implementation_stage: Stage,
    pub acceptance_stage: Stage,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DependenciesDocument {
    #[serde(deserialize_with = "schema_version")]
    pub schema_version: u32,
    pub packages: Vec<PackageDependency>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CandidateArtifact {
    pub component: String,
    pub configuration_id: String,
    pub path: String,
    pub sha256: String,
    pub build_provenance_path: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CandidateManifest {
    #[serde(deserialize_with = "schema_version")]
    pub schema_version: u32,
    pub code_sha: String,
    pub artifacts: Vec<CandidateArtifact>,
    pub configuration_ids: Vec<String>,
    pub nominated_at_utc: String,
    pub nominated_by: String,
}
