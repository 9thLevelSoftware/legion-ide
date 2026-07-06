// PKT-RISK P3.F4.T4 — advisory classifier wiring tests.
//
// These tests prove that the advisory classifier recommendation is metadata-only:
// it attaches to the assessment for display but NEVER changes deterministic rule
// outcomes, the aggregate label, or the approval level.

use legion_protocol::ProposalRiskLabel;
use legion_protocol::risk::{RiskRuleId, RiskRuleInput, RiskRuleOutcome};
use legion_security::risk::evaluate_with_advisory;

fn all_allow_input() -> RiskRuleInput {
    RiskRuleInput {
        workspace_root: Some("/repo/workspace".to_string()),
        touched_paths: vec!["/repo/workspace/src/main.rs".to_string()],
        deleted_file_count: 0,
    }
}

fn path_escape_input() -> RiskRuleInput {
    RiskRuleInput {
        workspace_root: Some("/repo/workspace".to_string()),
        touched_paths: vec!["/repo/other/secret.toml".to_string()],
        deleted_file_count: 0,
    }
}

#[test]
fn classifier_low_does_not_override_deterministic_deny() {
    // Even if the classifier suggests Low risk, a PathScope deny keeps the aggregate High.
    let assessment = evaluate_with_advisory(&path_escape_input(), Some(ProposalRiskLabel::Low));

    // Advisory recommendation is attached
    assert_eq!(
        assessment.advisory_recommendation,
        Some(ProposalRiskLabel::Low),
        "advisory recommendation must be attached"
    );

    // Deterministic outcome is unchanged: PathScope still denies
    let path_finding = assessment.finding(RiskRuleId::PathScope).unwrap();
    assert_eq!(
        path_finding.outcome,
        RiskRuleOutcome::Deny,
        "deterministic PathScope deny must not be overridden by classifier"
    );

    // Aggregate stays High — classifier Low suggestion is ignored
    assert_eq!(
        assessment.aggregate_risk_label,
        ProposalRiskLabel::High,
        "aggregate must stay High when a rule denies, regardless of advisory label"
    );
    assert!(
        !assessment.is_allow(),
        "is_allow must be false when any rule denies"
    );
}

#[test]
fn classifier_high_does_not_override_deterministic_allow() {
    // Even if the classifier suggests High risk, all-allow rules keep the aggregate Low.
    let assessment = evaluate_with_advisory(&all_allow_input(), Some(ProposalRiskLabel::High));

    // Advisory recommendation is attached
    assert_eq!(
        assessment.advisory_recommendation,
        Some(ProposalRiskLabel::High),
        "advisory recommendation must be attached"
    );

    // Deterministic outcome is unchanged: all rules still allow
    assert!(
        assessment.is_allow(),
        "is_allow must be true when no rule denies, regardless of advisory label"
    );

    // Aggregate stays Low — classifier High suggestion is ignored
    assert_eq!(
        assessment.aggregate_risk_label,
        ProposalRiskLabel::Low,
        "aggregate must stay Low when all rules allow, regardless of advisory label"
    );
}

#[test]
fn classifier_recommendation_appears_in_assessment() {
    // The advisory label is preserved as metadata in the assessment.
    for label in [
        ProposalRiskLabel::Low,
        ProposalRiskLabel::Medium,
        ProposalRiskLabel::High,
        ProposalRiskLabel::Informational,
        ProposalRiskLabel::Unknown,
    ] {
        let assessment = evaluate_with_advisory(&all_allow_input(), Some(label));
        assert_eq!(
            assessment.advisory_recommendation,
            Some(label),
            "advisory recommendation {label:?} must survive into assessment"
        );
    }
}

#[test]
fn assessment_without_classifier_has_none() {
    // Default evaluation (no advisory) leaves advisory_recommendation as None.
    let assessment = evaluate_with_advisory(&all_allow_input(), None);
    assert_eq!(
        assessment.advisory_recommendation, None,
        "evaluation without advisory must produce None advisory_recommendation"
    );

    // evaluate_risk_rules (convenience wrapper) also produces None
    let assessment2 = legion_security::risk::evaluate_risk_rules(&all_allow_input());
    assert_eq!(
        assessment2.advisory_recommendation, None,
        "evaluate_risk_rules must produce None advisory_recommendation"
    );
}
