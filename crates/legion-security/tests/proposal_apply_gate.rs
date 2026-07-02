use legion_protocol::ProposalRiskLabel;
use legion_security::{ProposalApplyGate, SecurityDecision};

#[test]
fn advisory_classifier_recommendation_does_not_override_policy_denial() {
    let gate = ProposalApplyGate::new(SecurityDecision::Deny("policy denied".to_string()))
        .with_human_approval_recorded(true)
        .with_classifier_recommendation(Some(ProposalRiskLabel::Low));

    assert!(!gate.can_apply());
    assert_eq!(
        gate.classifier_recommendation(),
        Some(ProposalRiskLabel::Low)
    );
    assert_eq!(
        gate.policy_decision(),
        &SecurityDecision::Deny("policy denied".to_string())
    );
}

#[test]
fn advisory_classifier_recommendation_does_not_replace_the_human_gate() {
    let gate = ProposalApplyGate::new(SecurityDecision::Allow)
        .with_human_approval_recorded(false)
        .with_classifier_recommendation(Some(ProposalRiskLabel::High));

    assert!(!gate.can_apply());
    assert_eq!(
        gate.classifier_recommendation(),
        Some(ProposalRiskLabel::High)
    );
    assert_eq!(gate.policy_decision(), &SecurityDecision::Allow);
}

#[test]
fn policy_allow_plus_human_approval_still_applies_even_when_classifier_is_high() {
    let gate = ProposalApplyGate::new(SecurityDecision::Allow)
        .with_human_approval_recorded(true)
        .with_classifier_recommendation(Some(ProposalRiskLabel::High));

    assert!(gate.can_apply());
    assert_eq!(
        gate.classifier_recommendation(),
        Some(ProposalRiskLabel::High)
    );
}
