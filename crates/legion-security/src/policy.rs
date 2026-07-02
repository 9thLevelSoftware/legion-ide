//! Deterministic approval-policy helpers for proposal auto-approval and apply gating.

use serde::{Deserialize, Serialize};

/// Envelope policy controlling when a proposal may be auto-approved without a human in the loop.
///
/// The default is fail-closed: auto-approval is disabled and no rules are trusted
/// until explicitly configured.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ProposalAutoApprovalPolicy {
    /// Whether deterministic auto-approval is permitted at all.
    pub enabled: bool,
    /// Rule identifiers that are recognized as auto-approvable risk evidence.
    pub allowed_rule_ids: Vec<String>,
}

impl ProposalAutoApprovalPolicy {
    /// Returns true only when auto-approval is enabled and every supplied rule id is
    /// non-empty, recognized, and there is at least one rule backing the decision.
    ///
    /// An empty `rule_ids` slice can never be auto-approved: `.all(..)` on an empty
    /// iterator is vacuously true, so without this guard auto-approval would be granted
    /// with zero deterministic rule evidence.
    pub fn allows_rule_ids(&self, rule_ids: &[String]) -> bool {
        if !self.enabled || rule_ids.is_empty() {
            return false;
        }

        rule_ids.iter().all(|requested| {
            !requested.is_empty()
                && self
                    .allowed_rule_ids
                    .iter()
                    .any(|allowed| allowed == requested)
        })
    }
}

/// Policy controlling batched runtime application of approved proposals.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchRuntimeApplyPolicy {
    /// Whether batched runtime apply is permitted at all.
    pub enabled: bool,
    /// Maximum number of proposals that may be applied in a single batch.
    pub max_batch_size: usize,
}

impl Default for BatchRuntimeApplyPolicy {
    fn default() -> Self {
        // Fail closed: batching is disabled and limited to a single proposal until configured.
        Self {
            enabled: false,
            max_batch_size: 1,
        }
    }
}

impl BatchRuntimeApplyPolicy {
    /// Returns true when the given trust state is sufficient for batch runtime apply.
    ///
    /// Only `Trusted` workspaces pass this check. Untrusted, unknown, or missing
    /// trust states are rejected regardless of the `enabled` flag.
    pub fn allows_workspace_trust(
        &self,
        trust: Option<legion_protocol::WorkspaceTrustState>,
    ) -> bool {
        matches!(trust, Some(legion_protocol::WorkspaceTrustState::Trusted))
    }

    /// Returns true when runtime apply is disabled for the given trust state.
    ///
    /// Runtime apply is disabled when the policy is disabled OR the workspace
    /// is not trusted. Both conditions must be satisfied for apply to proceed.
    pub fn runtime_apply_disabled(
        &self,
        trust: Option<legion_protocol::WorkspaceTrustState>,
    ) -> bool {
        !self.enabled || !self.allows_workspace_trust(trust)
    }
}

/// Gate evaluated before a proposal may be applied to the workspace.
#[derive(Debug, Clone)]
pub struct ProposalApplyGate {
    /// Policy decision from the security broker.
    policy_decision: super::SecurityDecision,
    /// Require explicit human approval before apply.
    pub require_human_approval: bool,
    /// Require a trusted workspace before apply.
    pub require_trusted_workspace: bool,
    /// Whether explicit human approval has been recorded.
    human_approval_recorded: bool,
    /// Advisory classifier output. This is never authoritative for apply.
    classifier_recommendation: Option<legion_protocol::ProposalRiskLabel>,
}

impl ProposalApplyGate {
    /// Creates a proposal apply gate from the authoritative policy decision.
    pub fn new(policy_decision: super::SecurityDecision) -> Self {
        Self {
            policy_decision,
            require_human_approval: true,
            require_trusted_workspace: true,
            human_approval_recorded: false,
            classifier_recommendation: None,
        }
    }

    /// Records whether human approval has been provided.
    pub fn with_human_approval_recorded(mut self, recorded: bool) -> Self {
        self.human_approval_recorded = recorded;
        self
    }

    /// Adds an advisory classifier recommendation.
    pub fn with_classifier_recommendation(
        mut self,
        recommendation: Option<legion_protocol::ProposalRiskLabel>,
    ) -> Self {
        self.classifier_recommendation = recommendation;
        self
    }

    /// Returns the advisory classifier recommendation, if any.
    pub fn classifier_recommendation(&self) -> Option<legion_protocol::ProposalRiskLabel> {
        self.classifier_recommendation
    }

    /// Returns the authoritative policy decision.
    pub fn policy_decision(&self) -> &super::SecurityDecision {
        &self.policy_decision
    }

    /// Returns true only when policy allows and the human gate is satisfied.
    pub fn can_apply(&self) -> bool {
        matches!(self.policy_decision, super::SecurityDecision::Allow)
            && (!self.require_human_approval || self.human_approval_recorded)
    }
}

impl Default for ProposalApplyGate {
    fn default() -> Self {
        // Fail closed: policy denies by default, human approval and trust are required.
        Self::new(super::SecurityDecision::Deny(
            "proposal apply gate default deny".to_string(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_rule_ids_are_never_auto_approved() {
        let policy = ProposalAutoApprovalPolicy {
            enabled: true,
            allowed_rule_ids: vec!["rule-a".to_string()],
        };
        assert!(!policy.allows_rule_ids(&[]));
    }

    #[test]
    fn disabled_policy_rejects_all_rule_ids() {
        let policy = ProposalAutoApprovalPolicy {
            enabled: false,
            allowed_rule_ids: vec!["rule-a".to_string()],
        };
        assert!(!policy.allows_rule_ids(&["rule-a".to_string()]));
    }

    #[test]
    fn unknown_or_blank_rule_ids_are_rejected() {
        let policy = ProposalAutoApprovalPolicy {
            enabled: true,
            allowed_rule_ids: vec!["rule-a".to_string()],
        };
        assert!(!policy.allows_rule_ids(&["rule-b".to_string()]));
        assert!(!policy.allows_rule_ids(&[String::new()]));
        assert!(policy.allows_rule_ids(&["rule-a".to_string()]));
    }
}
