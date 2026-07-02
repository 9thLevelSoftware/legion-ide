use legion_protocol::ProposalRiskLabel;
use legion_protocol::risk::{RiskRuleId, RiskRuleInput, RiskRuleOutcome};
use legion_security::risk::{DeterministicRiskRuleEngine, RiskRuleThresholds, evaluate_risk_rules};

#[test]
fn risk_rule_ids_are_stable_and_enumerated() {
    assert_eq!(
        RiskRuleId::all(),
        &[
            RiskRuleId::PathScope,
            RiskRuleId::FileCount,
            RiskRuleId::DeletionRatio,
            RiskRuleId::DependencyOrLockfileTouch,
            RiskRuleId::Migration,
            RiskRuleId::SecretsProximity,
            RiskRuleId::BinaryOrGeneratedFileChange,
        ]
    );

    assert_eq!(RiskRuleId::PathScope.stable_id(), "risk.path_scope");
    assert_eq!(RiskRuleId::FileCount.stable_id(), "risk.file_count");
    assert_eq!(RiskRuleId::DeletionRatio.stable_id(), "risk.deletion_ratio");
    assert_eq!(
        RiskRuleId::DependencyOrLockfileTouch.stable_id(),
        "risk.dependency_or_lockfile_touch"
    );
    assert_eq!(RiskRuleId::Migration.stable_id(), "risk.migration");
    assert_eq!(
        RiskRuleId::SecretsProximity.stable_id(),
        "risk.secrets_proximity"
    );
    assert_eq!(
        RiskRuleId::BinaryOrGeneratedFileChange.stable_id(),
        "risk.binary_or_generated_file_change"
    );
}

#[test]
fn deterministic_risk_rules_cover_allow_and_deny_edges() {
    let engine = DeterministicRiskRuleEngine::new(RiskRuleThresholds {
        max_touched_files: 4,
        max_deletion_ratio_percent: 49,
    });

    let cases = [
        (
            RiskRuleId::PathScope,
            RiskRuleInput {
                workspace_root: Some("/repo/workspace".to_string()),
                touched_paths: vec!["/repo/workspace/src/main.rs".to_string()],
                deleted_file_count: 0,
            },
            RiskRuleOutcome::Allow,
            RiskRuleInput {
                workspace_root: Some("/repo/workspace".to_string()),
                touched_paths: vec!["/repo/other/src/main.rs".to_string()],
                deleted_file_count: 0,
            },
            RiskRuleOutcome::Deny,
        ),
        (
            RiskRuleId::FileCount,
            RiskRuleInput {
                workspace_root: Some("/repo/workspace".to_string()),
                touched_paths: vec![
                    "/repo/workspace/src/a.rs".to_string(),
                    "/repo/workspace/src/b.rs".to_string(),
                ],
                deleted_file_count: 0,
            },
            RiskRuleOutcome::Allow,
            RiskRuleInput {
                workspace_root: Some("/repo/workspace".to_string()),
                touched_paths: vec![
                    "/repo/workspace/src/a.rs".to_string(),
                    "/repo/workspace/src/b.rs".to_string(),
                    "/repo/workspace/src/c.rs".to_string(),
                    "/repo/workspace/src/d.rs".to_string(),
                    "/repo/workspace/src/e.rs".to_string(),
                ],
                deleted_file_count: 0,
            },
            RiskRuleOutcome::Deny,
        ),
        (
            RiskRuleId::DeletionRatio,
            RiskRuleInput {
                workspace_root: Some("/repo/workspace".to_string()),
                touched_paths: vec![
                    "/repo/workspace/src/a.rs".to_string(),
                    "/repo/workspace/src/b.rs".to_string(),
                    "/repo/workspace/src/c.rs".to_string(),
                    "/repo/workspace/src/d.rs".to_string(),
                ],
                deleted_file_count: 1,
            },
            RiskRuleOutcome::Allow,
            RiskRuleInput {
                workspace_root: Some("/repo/workspace".to_string()),
                touched_paths: vec![
                    "/repo/workspace/src/a.rs".to_string(),
                    "/repo/workspace/src/b.rs".to_string(),
                    "/repo/workspace/src/c.rs".to_string(),
                    "/repo/workspace/src/d.rs".to_string(),
                ],
                deleted_file_count: 3,
            },
            RiskRuleOutcome::Deny,
        ),
        (
            RiskRuleId::DependencyOrLockfileTouch,
            RiskRuleInput {
                workspace_root: Some("/repo/workspace".to_string()),
                touched_paths: vec!["/repo/workspace/src/lib.rs".to_string()],
                deleted_file_count: 0,
            },
            RiskRuleOutcome::Allow,
            RiskRuleInput {
                workspace_root: Some("/repo/workspace".to_string()),
                touched_paths: vec!["/repo/workspace/Cargo.lock".to_string()],
                deleted_file_count: 0,
            },
            RiskRuleOutcome::Deny,
        ),
        (
            RiskRuleId::Migration,
            RiskRuleInput {
                workspace_root: Some("/repo/workspace".to_string()),
                touched_paths: vec!["/repo/workspace/src/lib.rs".to_string()],
                deleted_file_count: 0,
            },
            RiskRuleOutcome::Allow,
            RiskRuleInput {
                workspace_root: Some("/repo/workspace".to_string()),
                touched_paths: vec![
                    "/repo/workspace/db/migrations/20260614_add_risk.sql".to_string(),
                ],
                deleted_file_count: 0,
            },
            RiskRuleOutcome::Deny,
        ),
        (
            RiskRuleId::SecretsProximity,
            RiskRuleInput {
                workspace_root: Some("/repo/workspace".to_string()),
                touched_paths: vec!["/repo/workspace/src/lib.rs".to_string()],
                deleted_file_count: 0,
            },
            RiskRuleOutcome::Allow,
            RiskRuleInput {
                workspace_root: Some("/repo/workspace".to_string()),
                touched_paths: vec!["/repo/workspace/secrets/api_keys.toml".to_string()],
                deleted_file_count: 0,
            },
            RiskRuleOutcome::Deny,
        ),
        (
            RiskRuleId::BinaryOrGeneratedFileChange,
            RiskRuleInput {
                workspace_root: Some("/repo/workspace".to_string()),
                touched_paths: vec!["/repo/workspace/src/lib.rs".to_string()],
                deleted_file_count: 0,
            },
            RiskRuleOutcome::Allow,
            RiskRuleInput {
                workspace_root: Some("/repo/workspace".to_string()),
                touched_paths: vec!["/repo/workspace/target/generated/schema.pb.rs".to_string()],
                deleted_file_count: 0,
            },
            RiskRuleOutcome::Deny,
        ),
    ];

    for (rule_id, allow_input, allow_outcome, deny_input, deny_outcome) in cases {
        let allow_assessment = engine.evaluate(&allow_input);
        let allow_finding = allow_assessment
            .finding(rule_id)
            .expect("missing allow finding");
        assert_eq!(
            allow_finding.outcome, allow_outcome,
            "rule {rule_id:?} allow case"
        );
        assert_eq!(
            allow_assessment.aggregate_risk_label,
            ProposalRiskLabel::Low
        );
        assert!(allow_assessment.is_allow());

        let deny_assessment = engine.evaluate(&deny_input);
        let deny_finding = deny_assessment
            .finding(rule_id)
            .expect("missing deny finding");
        assert_eq!(
            deny_finding.outcome, deny_outcome,
            "rule {rule_id:?} deny case"
        );
        assert_eq!(
            deny_assessment.aggregate_risk_label,
            ProposalRiskLabel::High
        );
        assert!(matches!(deny_finding.outcome, RiskRuleOutcome::Deny));
    }
}

#[test]
fn evaluate_risk_rules_uses_default_thresholds() {
    let assessment = evaluate_risk_rules(&RiskRuleInput {
        workspace_root: Some("/repo/workspace".to_string()),
        touched_paths: vec![
            "/repo/workspace/src/a.rs".to_string(),
            "/repo/workspace/src/b.rs".to_string(),
            "/repo/workspace/src/c.rs".to_string(),
            "/repo/workspace/src/d.rs".to_string(),
            "/repo/workspace/src/e.rs".to_string(),
            "/repo/workspace/src/f.rs".to_string(),
            "/repo/workspace/src/g.rs".to_string(),
            "/repo/workspace/src/h.rs".to_string(),
            "/repo/workspace/src/i.rs".to_string(),
        ],
        deleted_file_count: 0,
    });

    assert_eq!(assessment.aggregate_risk_label, ProposalRiskLabel::High);
    assert_eq!(
        assessment.finding(RiskRuleId::FileCount).unwrap().outcome,
        RiskRuleOutcome::Deny
    );
}
