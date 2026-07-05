use legion_protocol::risk::RiskRuleId;
use legion_security::ProposalAutoApprovalPolicy;

fn all_rule_ids() -> Vec<String> {
    RiskRuleId::all()
        .iter()
        .map(|rule_id| rule_id.stable_id().to_string())
        .collect()
}

#[test]
fn proposal_auto_approval_policy_is_opt_in_and_requires_matching_rule_ids() {
    let policy = ProposalAutoApprovalPolicy::default();

    assert!(!policy.enabled, "default policy must stay opt-in only");
    assert!(!policy.allows_rule_ids(&all_rule_ids()));

    let enabled = ProposalAutoApprovalPolicy {
        enabled: true,
        allowed_rule_ids: all_rule_ids(),
    };

    assert!(enabled.allows_rule_ids(&all_rule_ids()));
    assert!(enabled.allows_rule_ids(&[RiskRuleId::PathScope.stable_id().to_string()]));

    let missing_rule = ProposalAutoApprovalPolicy {
        enabled: true,
        allowed_rule_ids: vec![RiskRuleId::PathScope.stable_id().to_string()],
    };

    assert!(!missing_rule.allows_rule_ids(&all_rule_ids()));
}
