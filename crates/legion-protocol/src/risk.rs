//! Deterministic risk-rule DTOs shared across Legion crates.

use serde::{Deserialize, Serialize};

use crate::ProposalRiskLabel;

/// Stable identifiers for deterministic risk rules.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum RiskRuleId {
    /// Workspace path scope is broader than the approved root.
    PathScope,
    /// Too many files are touched for an auto-approval path.
    FileCount,
    /// The deletion ratio is too high for a low-risk change.
    DeletionRatio,
    /// A dependency manifest or lockfile is being touched.
    DependencyOrLockfileTouch,
    /// A migration file is being touched.
    Migration,
    /// The change is near secrets or credential material.
    SecretsProximity,
    /// A binary or generated file is being changed.
    BinaryOrGeneratedFileChange,
}

impl RiskRuleId {
    /// Returns the stable machine-readable id for the rule.
    pub const fn stable_id(self) -> &'static str {
        match self {
            Self::PathScope => "risk.path_scope",
            Self::FileCount => "risk.file_count",
            Self::DeletionRatio => "risk.deletion_ratio",
            Self::DependencyOrLockfileTouch => "risk.dependency_or_lockfile_touch",
            Self::Migration => "risk.migration",
            Self::SecretsProximity => "risk.secrets_proximity",
            Self::BinaryOrGeneratedFileChange => "risk.binary_or_generated_file_change",
        }
    }

    /// Returns the canonical enumeration of all deterministic risk rules.
    pub const fn all() -> &'static [Self; 7] {
        &[
            Self::PathScope,
            Self::FileCount,
            Self::DeletionRatio,
            Self::DependencyOrLockfileTouch,
            Self::Migration,
            Self::SecretsProximity,
            Self::BinaryOrGeneratedFileChange,
        ]
    }
}

/// Deterministic rule outcome used by approval gating.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RiskRuleOutcome {
    /// The rule did not trip and the change stays eligible.
    Allow,
    /// The rule tripped and the change needs review or denial.
    Deny,
}

impl RiskRuleOutcome {
    /// Returns true when the rule outcome is allow.
    pub const fn is_allow(self) -> bool {
        matches!(self, Self::Allow)
    }

    /// Returns true when the rule outcome is deny.
    pub const fn is_deny(self) -> bool {
        matches!(self, Self::Deny)
    }
}

/// A single deterministic risk-rule finding.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RiskRuleFinding {
    /// Stable rule identifier.
    pub rule_id: RiskRuleId,
    /// Binary decision produced by the rule.
    pub outcome: RiskRuleOutcome,
    /// High-level risk label derived from the rule.
    pub risk_label: ProposalRiskLabel,
    /// Human-readable rationale or evidence snippets.
    pub evidence: Vec<String>,
}

impl RiskRuleFinding {
    /// Creates a new allow finding for the given rule.
    pub fn allow(
        rule_id: RiskRuleId,
        risk_label: ProposalRiskLabel,
        evidence: Vec<String>,
    ) -> Self {
        Self {
            rule_id,
            outcome: RiskRuleOutcome::Allow,
            risk_label,
            evidence,
        }
    }

    /// Creates a new deny finding for the given rule.
    pub fn deny(rule_id: RiskRuleId, evidence: Vec<String>) -> Self {
        Self {
            rule_id,
            outcome: RiskRuleOutcome::Deny,
            risk_label: ProposalRiskLabel::High,
            evidence,
        }
    }
}

/// Deterministic assessment returned by the risk engine.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RiskAssessment {
    /// All rule findings in canonical rule order.
    pub findings: Vec<RiskRuleFinding>,
    /// Aggregate label derived from the findings.
    pub aggregate_risk_label: ProposalRiskLabel,
}

impl RiskAssessment {
    /// Returns the finding for `rule_id`, if present.
    pub fn finding(&self, rule_id: RiskRuleId) -> Option<&RiskRuleFinding> {
        self.findings
            .iter()
            .find(|finding| finding.rule_id == rule_id)
    }

    /// Returns true when no rule denied the change.
    pub fn is_allow(&self) -> bool {
        self.findings
            .iter()
            .all(|finding| finding.outcome.is_allow())
    }
}

/// Normalized change-summary input evaluated by the risk engine.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RiskRuleInput {
    /// Approved workspace root used to bound path scope checks.
    pub workspace_root: Option<String>,
    /// File paths touched by the proposal.
    pub touched_paths: Vec<String>,
    /// Number of touched files that are deletes.
    pub deleted_file_count: usize,
}

impl RiskRuleInput {
    /// Returns the count of touched files.
    pub fn touched_file_count(&self) -> usize {
        self.touched_paths.len()
    }
}

/// Deterministic rule-engine interface shared by approval surfaces.
pub trait RiskRuleEngine {
    /// Evaluates a normalized change summary against deterministic rules.
    fn evaluate(&self, input: &RiskRuleInput) -> RiskAssessment;
}

/// Graduated human-approval level derived from a risk assessment and policy.
///
/// The ladder runs from fully-automatic (no human required) to unconditionally
/// denied.  Every level except `Auto` surfaces at least some human-visible
/// friction before the proposal can be applied.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ApprovalLevel {
    /// All deterministic rules allow and the auto-approval policy is satisfied.
    /// The proposal may be applied without a human approval step.
    Auto,
    /// All deterministic rules allow but the policy does not grant auto-approval.
    /// Surface to the human for a quick confirm before applying.
    Ask,
    /// One or more rules deny but the change is not a critical-path violation.
    /// The proposal is paused; explicit human approval is required before apply.
    RequireExplicit,
    /// Unconditionally denied — the proposal must not be applied.
    /// Reserved for critical violations such as workspace-scope escape.
    Deny,
}
