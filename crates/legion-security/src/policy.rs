//! Deterministic approval-policy helpers for proposal auto-approval and apply gating.

use std::collections::HashMap;

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

/// Capability identifier a debug adapter launch must be granted under.
///
/// `legion-debug` refuses to resolve an adapter binary without a granted decision
/// carrying exactly this id, so the string is part of the contract between the
/// broker and the debug crate — not a log label.
pub const DEBUG_ADAPTER_LAUNCH_CAPABILITY: &str = "debug.adapter.launch";

/// Debug adapter launch policy controls (P2.F3.T2).
///
/// Two independent conditions must hold before an adapter process can exist:
/// the workspace must be trusted, and the *resolved binary* must be named in
/// [`Self::allowed_adapter_binaries`]. The second condition is what makes
/// `LEGION_DAP_ADAPTER` safe to honor: an operator-supplied path that resolves
/// to something other than a known debug adapter is refused.
///
/// There is deliberately no "trust every adapter" switch. Widening the set is
/// done by naming binaries, which keeps the allowed set auditable.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DebugAdapterLaunchPolicy {
    /// Trusted workspaces only by default.
    pub require_trusted_workspace: bool,
    /// Adapter binaries that may be launched, matched on the file stem without
    /// extension (`codelldb`, `codelldb.exe`, and `/opt/x/codelldb` all match
    /// `codelldb`). An empty list denies every adapter.
    #[serde(default = "default_allowed_adapter_binaries")]
    pub allowed_adapter_binaries: Vec<String>,
}

/// Adapter binaries allowed out of the box: the two Microsoft-DAP stdio adapters
/// that ship with LLDB, plus CodeLLDB.
fn default_allowed_adapter_binaries() -> Vec<String> {
    vec![
        "lldb-dap".to_string(),
        "lldb-vscode".to_string(),
        "codelldb".to_string(),
    ]
}

impl Default for DebugAdapterLaunchPolicy {
    fn default() -> Self {
        Self {
            require_trusted_workspace: true,
            allowed_adapter_binaries: default_allowed_adapter_binaries(),
        }
    }
}

impl DebugAdapterLaunchPolicy {
    /// Returns true when adapter discovery is allowed for the given workspace trust.
    pub fn allows_resolution(&self, trust: legion_protocol::WorkspaceTrustState) -> bool {
        !self.require_trusted_workspace || trust == legion_protocol::WorkspaceTrustState::Trusted
    }

    /// Returns true when `binary` is an allowlisted adapter.
    ///
    /// `binary` is compared case-insensitively against the configured stems; an
    /// empty allowlist denies everything rather than allowing everything, the
    /// same vacuous-truth guard used by [`ProposalAutoApprovalPolicy::allows_rule_ids`].
    pub fn allows_adapter_binary(&self, binary: &str) -> bool {
        let binary = binary.trim();
        if binary.is_empty() {
            return false;
        }
        self.allowed_adapter_binaries
            .iter()
            .any(|allowed| allowed.trim().eq_ignore_ascii_case(binary))
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

// ---------------------------------------------------------------------------
// Graduated approval ladder
// ---------------------------------------------------------------------------

/// Derives a `ApprovalLevel` from a deterministic risk assessment and policy.
///
/// The graduated ladder maps the assessment outcome to one of four levels:
///
/// * **`Auto`** — all deterministic rules allow and the policy permits auto-approval
///   for the exact set of rule IDs cited in the assessment.
/// * **`Ask`** — all rules allow but the policy does not grant auto-approval.
/// * **`RequireExplicit`** — one or more non-critical rules deny the change.
/// * **`Deny`** — a critical path-scope violation is detected (workspace escape).
///
/// Empty findings can never produce `Auto` because `allows_rule_ids` rejects an
/// empty slice (vacuous-truth guard in [`ProposalAutoApprovalPolicy::allows_rule_ids`]).
pub fn derive_approval_level(
    assessment: &legion_protocol::risk::RiskAssessment,
    policy: &ProposalAutoApprovalPolicy,
) -> legion_protocol::risk::ApprovalLevel {
    use legion_protocol::risk::{ApprovalLevel, RiskRuleId};

    // Critical violation: workspace-scope escape is unconditionally denied.
    if let Some(finding) = assessment.finding(RiskRuleId::PathScope)
        && finding.outcome.is_deny()
    {
        return ApprovalLevel::Deny;
    }

    // Any non-critical rule deny → pause and require explicit approval.
    if !assessment.is_allow() {
        return ApprovalLevel::RequireExplicit;
    }

    // All rules allow — check whether the policy grants auto-approval.
    let rule_ids: Vec<String> = assessment
        .findings
        .iter()
        .map(|f| f.rule_id.stable_id().to_string())
        .collect();

    if policy.allows_rule_ids(&rule_ids) {
        ApprovalLevel::Auto
    } else {
        ApprovalLevel::Ask
    }
}

/// Produces a metadata map recording the computed `ApprovalLevel` for audit rows.
///
/// Insert the returned map into any proposal audit record so every apply/deny
/// decision carries which approval level was computed.
pub fn approval_level_audit_metadata(
    level: legion_protocol::risk::ApprovalLevel,
) -> HashMap<String, String> {
    use legion_protocol::risk::ApprovalLevel;

    let level_str = match level {
        ApprovalLevel::Auto => "Auto",
        ApprovalLevel::Ask => "Ask",
        ApprovalLevel::RequireExplicit => "RequireExplicit",
        ApprovalLevel::Deny => "Deny",
    };
    let mut map = HashMap::new();
    map.insert("approval_level".to_string(), level_str.to_string());
    map
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
    fn debug_adapter_policy_allowlists_known_adapters_only() {
        let policy = DebugAdapterLaunchPolicy::default();
        assert!(policy.allows_adapter_binary("lldb-dap"));
        assert!(policy.allows_adapter_binary("codelldb"));
        // Case folding matters on Windows, where the stem may arrive as `CodeLLDB`.
        assert!(policy.allows_adapter_binary("CodeLLDB"));
        assert!(!policy.allows_adapter_binary("bash"));
        assert!(!policy.allows_adapter_binary("fake_dap_adapter"));
    }

    #[test]
    fn debug_adapter_policy_empty_allowlist_denies_every_binary() {
        // Vacuous-truth guard: an empty list must not mean "anything goes".
        let policy = DebugAdapterLaunchPolicy {
            require_trusted_workspace: true,
            allowed_adapter_binaries: Vec::new(),
        };
        assert!(!policy.allows_adapter_binary("lldb-dap"));
        assert!(!policy.allows_adapter_binary(""));
    }

    #[test]
    fn debug_adapter_policy_rejects_blank_binary_names() {
        let policy = DebugAdapterLaunchPolicy {
            require_trusted_workspace: true,
            allowed_adapter_binaries: vec![String::new(), "  ".to_string()],
        };
        assert!(!policy.allows_adapter_binary(""));
        assert!(!policy.allows_adapter_binary("   "));
        assert!(!policy.allows_adapter_binary("lldb-dap"));
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
