use legion_ai::classifier::{AdvisoryRiskClassifier, RiskClassifierRecommendation};
use legion_protocol::ProposalRiskLabel;

#[test]
fn advisory_classifier_keeps_the_policy_label_separate_from_recommendations() {
    let classifier = AdvisoryRiskClassifier::new(Some("model-assisted-risk".to_string()));
    let recommendation = classifier.recommend(
        ProposalRiskLabel::High,
        "destructive change should be flagged but not enforced",
    );

    assert_eq!(recommendation.suggested_label, ProposalRiskLabel::High);
    assert_eq!(
        recommendation.model_id.as_deref(),
        Some("model-assisted-risk")
    );
    assert!(recommendation.advisory_only);
    assert_eq!(recommendation.policy_label, None);
}

#[test]
fn advisory_classifier_can_wrap_an_existing_policy_label_without_overwriting_it() {
    let recommendation = RiskClassifierRecommendation::new(
        ProposalRiskLabel::Low,
        Some("policy already allows the apply path".to_string()),
    );

    assert_eq!(recommendation.suggested_label, ProposalRiskLabel::Low);
    assert_eq!(recommendation.policy_label, None);
    assert!(recommendation.advisory_only);
}
