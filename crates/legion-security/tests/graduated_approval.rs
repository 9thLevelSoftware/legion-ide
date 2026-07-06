// PKT-RISK P3.F4.T2 — graduated approval ladder tests.

use legion_protocol::ProposalRiskLabel;
use legion_protocol::risk::{ApprovalLevel, RiskRuleId, RiskRuleInput};
use legion_security::risk::{DeterministicRiskRuleEngine, RiskRuleThresholds};
use legion_security::{ProposalAutoApprovalPolicy, approval_level_audit_metadata, derive_approval_level};

fn all_allow_input() -> RiskRuleInput {
    RiskRuleInput {
        workspace_root: Some("/repo/workspace".to_string()),
        touched_paths: vec![
            "/repo/workspace/src/main.rs".to_string(),
            "/repo/workspace/src/lib.rs".to_string(),
        ],
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

fn file_count_deny_input() -> RiskRuleInput {
    // 5 files exceeds the 4-file threshold below
    RiskRuleInput {
        workspace_root: Some("/repo/workspace".to_string()),
        touched_paths: (0..5)
            .map(|i| format!("/repo/workspace/src/file_{i}.rs"))
            .collect(),
        deleted_file_count: 0,
    }
}

fn engine_with_low_threshold() -> DeterministicRiskRuleEngine {
    DeterministicRiskRuleEngine::new(RiskRuleThresholds {
        max_touched_files: 4,
        max_deletion_ratio_percent: 49,
    })
}

fn full_policy() -> ProposalAutoApprovalPolicy {
    ProposalAutoApprovalPolicy {
        enabled: true,
        allowed_rule_ids: RiskRuleId::all()
            .iter()
            .map(|id| id.stable_id().to_string())
            .collect(),
    }
}

fn disabled_policy() -> ProposalAutoApprovalPolicy {
    ProposalAutoApprovalPolicy {
        enabled: false,
        allowed_rule_ids: RiskRuleId::all()
            .iter()
            .map(|id| id.stable_id().to_string())
            .collect(),
    }
}

#[test]
fn all_allow_with_matching_policy_is_auto() {
    let engine = engine_with_low_threshold();
    let assessment = engine.evaluate(&all_allow_input());
    assert!(assessment.is_allow(), "all rules should allow");
    assert_eq!(
        assessment.aggregate_risk_label,
        ProposalRiskLabel::Low,
        "aggregate should be Low"
    );

    let level = derive_approval_level(&assessment, &full_policy());
    assert_eq!(
        level,
        ApprovalLevel::Auto,
        "all-allow + full policy should be Auto"
    );
}

#[test]
fn all_allow_without_policy_is_ask() {
    let engine = engine_with_low_threshold();
    let assessment = engine.evaluate(&all_allow_input());
    assert!(assessment.is_allow());

    let level = derive_approval_level(&assessment, &disabled_policy());
    assert_eq!(
        level,
        ApprovalLevel::Ask,
        "all-allow + disabled policy should be Ask"
    );
}

#[test]
fn any_deny_is_require_explicit() {
    let engine = engine_with_low_threshold();
    let assessment = engine.evaluate(&file_count_deny_input());
    assert!(
        !assessment.is_allow(),
        "file count rule should deny"
    );
    // PathScope should still allow for this input
    let path_finding = assessment.finding(RiskRuleId::PathScope).unwrap();
    assert!(path_finding.outcome.is_allow(), "path scope should allow");

    let level = derive_approval_level(&assessment, &full_policy());
    assert_eq!(
        level,
        ApprovalLevel::RequireExplicit,
        "non-critical deny should be RequireExplicit"
    );
}

#[test]
fn critical_deny_is_deny() {
    let engine = engine_with_low_threshold();
    let assessment = engine.evaluate(&path_escape_input());
    assert_eq!(assessment.aggregate_risk_label, ProposalRiskLabel::High);
    let path_finding = assessment.finding(RiskRuleId::PathScope).unwrap();
    assert!(path_finding.outcome.is_deny(), "path scope should deny");

    // Even with a permissive policy, a path-scope escape is unconditionally Deny
    let level = derive_approval_level(&assessment, &full_policy());
    assert_eq!(level, ApprovalLevel::Deny, "path escape should be Deny");
}

#[test]
fn empty_rule_ids_never_auto() {
    // Start from a real assessment and clear findings to create a vacuous allow state.
    let engine = engine_with_low_threshold();
    let mut assessment = engine.evaluate(&all_allow_input());
    assessment.findings.clear();
    assert!(assessment.is_allow(), "vacuous assessment allows (empty findings)");

    let level = derive_approval_level(&assessment, &full_policy());
    // empty findings → empty rule_ids → allows_rule_ids returns false → Ask
    assert_eq!(
        level,
        ApprovalLevel::Ask,
        "empty findings must never reach Auto"
    );
}

#[test]
fn approval_level_appears_in_audit_metadata() {
    let engine = engine_with_low_threshold();
    let assessment = engine.evaluate(&all_allow_input());
    let level = derive_approval_level(&assessment, &full_policy());
    assert_eq!(level, ApprovalLevel::Auto);

    let metadata = approval_level_audit_metadata(level);
    assert!(
        metadata.contains_key("approval_level"),
        "metadata must contain approval_level key"
    );
    assert_eq!(
        metadata["approval_level"], "Auto",
        "audit metadata must record the computed level"
    );

    // Verify all four levels round-trip through metadata
    for (level, expected) in [
        (ApprovalLevel::Auto, "Auto"),
        (ApprovalLevel::Ask, "Ask"),
        (ApprovalLevel::RequireExplicit, "RequireExplicit"),
        (ApprovalLevel::Deny, "Deny"),
    ] {
        let meta = approval_level_audit_metadata(level);
        assert_eq!(meta["approval_level"], expected, "level {level:?} should map to {expected}");
    }
}
