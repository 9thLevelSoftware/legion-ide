// PKT-RISK P3.F4.T3 — risk strip view model and silent-apply prevention tests.
//
// Note on dependency policy: legion-desktop may not directly depend on
// legion-security (circular dependency via legion-ai).  These tests therefore
// construct RiskAssessment values using legion-protocol types directly, which
// is sufficient to exercise the view layer.  The gate-level integration is
// covered in crates/legion-security/tests/graduated_approval.rs.

use legion_protocol::ProposalRiskLabel;
use legion_protocol::risk::{
    ApprovalLevel, RiskAssessment, RiskRuleFinding, RiskRuleId, RiskRuleOutcome,
};
use legion_desktop::view::{risk_strip_rows, risk_strip_view_model};

// ---------------------------------------------------------------------------
// Assessment builders
// ---------------------------------------------------------------------------

/// All-allow assessment: every rule produces Allow/Low.
fn low_risk_assessment() -> RiskAssessment {
    let findings = RiskRuleId::all()
        .iter()
        .map(|&rule_id| {
            RiskRuleFinding::allow(
                rule_id,
                ProposalRiskLabel::Low,
                vec!["test-allow".to_string()],
            )
        })
        .collect();
    RiskAssessment {
        findings,
        aggregate_risk_label: ProposalRiskLabel::Low,
    }
}

/// Assessment with a FileCount deny (non-critical).
fn file_count_deny_assessment() -> RiskAssessment {
    let findings = RiskRuleId::all()
        .iter()
        .map(|&rule_id| {
            if rule_id == RiskRuleId::FileCount {
                RiskRuleFinding::deny(rule_id, vec!["5 touched files exceeds limit 4".to_string()])
            } else {
                RiskRuleFinding::allow(rule_id, ProposalRiskLabel::Low, vec!["ok".to_string()])
            }
        })
        .collect();
    RiskAssessment {
        findings,
        aggregate_risk_label: ProposalRiskLabel::High,
    }
}

/// Assessment with a PathScope deny (critical workspace escape).
fn path_escape_assessment() -> RiskAssessment {
    let findings = RiskRuleId::all()
        .iter()
        .map(|&rule_id| {
            if rule_id == RiskRuleId::PathScope {
                RiskRuleFinding::deny(
                    rule_id,
                    vec!["one or more touched paths escape workspace scope".to_string()],
                )
            } else {
                RiskRuleFinding::allow(rule_id, ProposalRiskLabel::Low, vec!["ok".to_string()])
            }
        })
        .collect();
    RiskAssessment {
        findings,
        aggregate_risk_label: ProposalRiskLabel::High,
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[test]
fn low_risk_auto_shows_no_pause() {
    let assessment = low_risk_assessment();
    assert_eq!(assessment.aggregate_risk_label, ProposalRiskLabel::Low);
    assert!(assessment.is_allow());

    let vm = risk_strip_view_model("prop-1", &assessment, ApprovalLevel::Auto);

    assert!(!vm.paused, "Auto approval must not pause the proposal");
    assert!(
        !vm.requires_human_approval,
        "Auto approval does not need human approval"
    );
    assert_eq!(vm.aggregate_risk_label, ProposalRiskLabel::Low);
    assert_eq!(vm.approval_level, ApprovalLevel::Auto);
    assert!(vm.findings_summary.is_empty(), "No deny findings expected");

    let rows = risk_strip_rows(&assessment, ApprovalLevel::Auto);
    assert!(rows.iter().any(|r| r.contains("Low")), "rows must show Low risk");
    assert!(
        rows.iter().any(|r| r.contains("Auto")),
        "rows must show Auto approval"
    );
    assert!(
        !rows.iter().any(|r| r.contains("paused")),
        "rows must not contain pause notice"
    );
}

#[test]
fn medium_risk_require_explicit_pauses() {
    let assessment = file_count_deny_assessment();
    assert_eq!(assessment.aggregate_risk_label, ProposalRiskLabel::High);
    assert!(!assessment.is_allow());

    let vm = risk_strip_view_model("prop-2", &assessment, ApprovalLevel::RequireExplicit);

    assert!(vm.paused, "RequireExplicit must pause the proposal");
    assert!(
        vm.requires_human_approval,
        "RequireExplicit must require human approval"
    );
    assert_eq!(vm.approval_level, ApprovalLevel::RequireExplicit);
    assert!(
        !vm.findings_summary.is_empty(),
        "deny findings must be summarised"
    );

    let rows = risk_strip_rows(&assessment, ApprovalLevel::RequireExplicit);
    assert!(
        rows.iter().any(|r| r.contains("paused")),
        "rows must include pause notice"
    );
    assert!(
        rows.iter().any(|r| r.contains("RequireExplicit")),
        "rows must show RequireExplicit level"
    );
}

#[test]
fn high_risk_deny_shows_denial_reason() {
    let assessment = path_escape_assessment();
    assert_eq!(assessment.aggregate_risk_label, ProposalRiskLabel::High);
    assert!(!assessment.is_allow());
    // PathScope is the first finding and should be Deny
    let path_finding = assessment.finding(RiskRuleId::PathScope).unwrap();
    assert_eq!(path_finding.outcome, RiskRuleOutcome::Deny);

    let vm = risk_strip_view_model("prop-3", &assessment, ApprovalLevel::Deny);

    assert!(vm.paused, "Deny must pause the proposal");
    assert!(
        vm.requires_human_approval,
        "Deny must require human approval"
    );
    assert_eq!(vm.approval_level, ApprovalLevel::Deny);
    assert!(
        !vm.findings_summary.is_empty(),
        "denial reason must appear in findings_summary"
    );

    let rows = risk_strip_rows(&assessment, ApprovalLevel::Deny);
    assert!(
        rows.iter().any(|r| r.contains("denied")),
        "rows must include denial notice"
    );
    let deny_row = rows
        .iter()
        .find(|r| r.contains("denied"))
        .expect("denial row must exist");
    assert!(
        deny_row.len() > "✕ Proposal denied — ".len(),
        "denial row must include a non-empty reason: {deny_row}"
    );
}

#[test]
fn high_risk_never_applies_silently() {
    // Proves that RequireExplicit and Deny approval levels always set
    // `requires_human_approval = true` in the view model, which wires into
    // ProposalApplyGate.require_human_approval in the policy layer.
    // Gate-level testing lives in crates/legion-security/tests/graduated_approval.rs.
    for (assessment, level) in [
        (path_escape_assessment(), ApprovalLevel::Deny),
        (file_count_deny_assessment(), ApprovalLevel::RequireExplicit),
    ] {
        let vm = risk_strip_view_model("prop-silent", &assessment, level);
        assert!(
            vm.requires_human_approval,
            "level {level:?} must always require human approval in view model"
        );
        assert!(
            vm.paused,
            "level {level:?} must always pause the proposal"
        );
    }

    // Auto and Ask do NOT require human approval at the view model level
    let low = low_risk_assessment();
    let auto_vm = risk_strip_view_model("prop-auto", &low, ApprovalLevel::Auto);
    assert!(
        !auto_vm.requires_human_approval,
        "Auto must not require human approval"
    );
    let ask_vm = risk_strip_view_model("prop-ask", &low, ApprovalLevel::Ask);
    assert!(
        !ask_vm.requires_human_approval,
        "Ask must not require human approval (it's a quick confirm, not a gate)"
    );
}
