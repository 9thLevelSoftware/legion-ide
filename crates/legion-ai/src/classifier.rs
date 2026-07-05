//! Advisory model-assisted risk classifier outputs.

use legion_protocol::ProposalRiskLabel;
use serde::{Deserialize, Serialize};

/// Advisory recommendation produced by an optional classifier.
///
/// The recommendation never authorizes an apply by itself; it is metadata for
/// policy and human review surfaces.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RiskClassifierRecommendation {
    /// Suggested risk label from the classifier.
    pub suggested_label: ProposalRiskLabel,
    /// Optional display-safe classifier explanation.
    pub reason: Option<String>,
    /// Optional model identifier that produced the recommendation.
    pub model_id: Option<String>,
    /// Optional policy label carried through for display without changing the gate.
    pub policy_label: Option<ProposalRiskLabel>,
    /// True when the recommendation is advisory-only.
    pub advisory_only: bool,
}

impl RiskClassifierRecommendation {
    /// Creates a recommendation without policy state.
    pub fn new(suggested_label: ProposalRiskLabel, reason: Option<String>) -> Self {
        Self {
            suggested_label,
            reason,
            model_id: None,
            policy_label: None,
            advisory_only: true,
        }
    }
}

/// Optional classifier facade used by model-assisted flows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdvisoryRiskClassifier {
    /// Optional model identifier for the classifier source.
    pub model_id: Option<String>,
}

impl AdvisoryRiskClassifier {
    /// Creates a classifier facade with an optional model id.
    pub fn new(model_id: Option<String>) -> Self {
        Self { model_id }
    }

    /// Produces an advisory recommendation that leaves policy untouched.
    pub fn recommend(
        &self,
        suggested_label: ProposalRiskLabel,
        reason: impl Into<String>,
    ) -> RiskClassifierRecommendation {
        RiskClassifierRecommendation {
            suggested_label,
            reason: Some(reason.into()),
            model_id: self.model_id.clone(),
            policy_label: None,
            advisory_only: true,
        }
    }
}
