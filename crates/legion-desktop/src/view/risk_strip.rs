//! Risk strip view models and row projections for proposal review surfaces.
//!
//! The risk strip surfaces the graduated approval level and deny findings so the
//! human reviewer has immediate, unambiguous signal about what risk the proposal
//! carries and whether they must explicitly approve before apply.

use legion_protocol::ProposalRiskLabel;
use legion_protocol::risk::{ApprovalLevel, RiskAssessment};

/// Desktop view model for the risk strip displayed on a proposal card or review panel.
///
/// Provides all data required to render the risk strip without further policy
/// evaluation in the renderer — the renderer is projection-only.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DesktopProposalRiskStripViewModel {
    /// Proposal identifier this strip is attached to.
    pub proposal_id: String,
    /// Aggregate risk label from deterministic rule evaluation.
    pub aggregate_risk_label: ProposalRiskLabel,
    /// Graduated approval level derived from assessment and policy.
    pub approval_level: ApprovalLevel,
    /// Human-readable one-line summaries of every deny finding.
    pub findings_summary: Vec<String>,
    /// True when explicit human approval must be recorded before apply.
    pub requires_human_approval: bool,
    /// True when the proposal is paused pending review (RequireExplicit or Deny).
    pub paused: bool,
}

/// Builds a [`DesktopProposalRiskStripViewModel`] from an assessment and its derived level.
pub fn risk_strip_view_model(
    proposal_id: impl Into<String>,
    assessment: &RiskAssessment,
    approval_level: ApprovalLevel,
) -> DesktopProposalRiskStripViewModel {
    let requires_human_approval = matches!(
        approval_level,
        ApprovalLevel::RequireExplicit | ApprovalLevel::Deny
    );
    let paused = requires_human_approval;

    let findings_summary = assessment
        .findings
        .iter()
        .filter(|f| f.outcome.is_deny())
        .map(|f| format!("{}: {}", f.rule_id.stable_id(), f.evidence.join("; ")))
        .collect();

    DesktopProposalRiskStripViewModel {
        proposal_id: proposal_id.into(),
        aggregate_risk_label: assessment.aggregate_risk_label,
        approval_level,
        findings_summary,
        requires_human_approval,
        paused,
    }
}

/// Renders the risk strip as display rows for the proposal review panel.
///
/// Rows include:
/// - The aggregate risk label
/// - The approval level
/// - One row per deny finding with its rule ID and evidence
/// - A pause or denial notice for RequireExplicit/Deny levels
pub fn risk_strip_rows(assessment: &RiskAssessment, approval_level: ApprovalLevel) -> Vec<String> {
    let mut rows = Vec::new();

    let label = match assessment.aggregate_risk_label {
        ProposalRiskLabel::Informational => "Risk: Informational",
        ProposalRiskLabel::Low => "Risk: Low",
        ProposalRiskLabel::Medium => "Risk: Medium",
        ProposalRiskLabel::High => "Risk: High",
        ProposalRiskLabel::Unknown => "Risk: Unknown",
    };
    rows.push(label.to_string());

    let level_label = match approval_level {
        ApprovalLevel::Auto => "Approval: Auto",
        ApprovalLevel::Ask => "Approval: Ask",
        ApprovalLevel::RequireExplicit => "Approval: RequireExplicit",
        ApprovalLevel::Deny => "Approval: Deny",
    };
    rows.push(level_label.to_string());

    for finding in &assessment.findings {
        if finding.outcome.is_deny() {
            let evidence = finding.evidence.join("; ");
            rows.push(format!("{}: {}", finding.rule_id.stable_id(), evidence));
        }
    }

    match approval_level {
        ApprovalLevel::RequireExplicit => {
            rows.push("⏸ Proposal paused — explicit approval required".to_string());
        }
        ApprovalLevel::Deny => {
            let reasons: Vec<&str> = assessment
                .findings
                .iter()
                .filter(|f| f.outcome.is_deny())
                .flat_map(|f| f.evidence.iter().map(String::as_str))
                .collect();
            let reason = if reasons.is_empty() {
                "critical violation".to_string()
            } else {
                reasons.join(", ")
            };
            rows.push(format!("✕ Proposal denied — {reason}"));
        }
        _ => {}
    }

    rows
}
