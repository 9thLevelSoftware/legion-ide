use legion_protocol::risk::RiskRuleId;
use legion_protocol::{
    CapabilityId, CapabilityNamespace, CapabilityRequestContext, NetworkTarget, PluginId,
    PrincipalId, ProductMode,
};
use legion_security::{OrgPolicyBundle, SecurityDecision, TrustState};

fn enterprise_bundle() -> OrgPolicyBundle {
    toml::from_str(include_str!("../../../xtask/legion-policy.example.toml"))
        .expect("enterprise policy bundle must parse")
}

fn all_rule_ids() -> Vec<String> {
    RiskRuleId::all()
        .iter()
        .map(|rule_id| rule_id.stable_id().to_string())
        .collect()
}

fn plugin_context(capability: &str) -> CapabilityRequestContext {
    CapabilityRequestContext {
        plugin_namespace: Some(CapabilityNamespace("enterprise.plugins".to_string())),
        plugin_id: Some(PluginId(42)),
        plugin_host_call_name: Some("read-context".to_string()),
        plugin_module_hash: Some("sha256:enterprise-plugin".to_string()),
        plugin_manifest_id: Some("plugin:enterprise:1".to_string()),
        plugin_declared_capability_id: Some(CapabilityId(capability.to_string())),
        plugin_quota_class: Some(legion_protocol::PluginQuotaClass::HostCall),
        plugin_sandbox_operation_class: Some(
            legion_protocol::PluginSandboxOperationClass::HostCall,
        ),
        ..CapabilityRequestContext::default()
    }
}

fn localhost_https_context() -> CapabilityRequestContext {
    CapabilityRequestContext {
        network_target: Some(NetworkTarget {
            scheme: "https".to_string(),
            host: "localhost".to_string(),
            port: Some(443),
        }),
        ..CapabilityRequestContext::default()
    }
}

#[test]
fn enterprise_policy_bundle_round_trips_and_keeps_signed_profile_fields() {
    let bundle = enterprise_bundle();

    assert_eq!(bundle.schema_version, 1);
    assert_eq!(bundle.bundle_id, "enterprise-restrictive");
    assert_eq!(bundle.bundle_version, 1);
    assert_eq!(
        bundle.signature_label,
        "signed-by-enterprise-policy-service"
    );
    assert_eq!(bundle.mode_ceiling, ProductMode::Assist);
    assert!(bundle.allows_mode(ProductMode::Manual));
    assert!(bundle.allows_mode(ProductMode::Assist));
    assert!(!bundle.allows_mode(ProductMode::Delegates));
    assert!(!bundle.allows_mode(ProductMode::Automate));

    assert!(bundle.security_policy.proposal_auto_approval_policy.enabled);
    assert!(
        bundle
            .security_policy
            .proposal_auto_approval_policy
            .allows_rule_ids(&all_rule_ids())
    );
}

#[test]
fn enterprise_policy_bundle_enforces_tool_allowlists_and_mode_ceiling() {
    let bundle = enterprise_bundle();
    let principal = PrincipalId("enterprise-user".to_string());

    assert_eq!(
        bundle.decide_with_request_context(
            ProductMode::Assist,
            TrustState::Trusted,
            principal.clone(),
            CapabilityId("plugin.context.read".to_string()),
            None,
            plugin_context("plugin.context.read"),
        ),
        SecurityDecision::Allow
    );

    assert!(matches!(
        bundle.decide_with_request_context(
            ProductMode::Assist,
            TrustState::Trusted,
            principal.clone(),
            CapabilityId("plugin.command".to_string()),
            None,
            plugin_context("plugin.context.read"),
        ),
        SecurityDecision::Deny(_)
    ));

    assert!(matches!(
        bundle.decide_with_request_context(
            ProductMode::Delegates,
            TrustState::Trusted,
            principal,
            CapabilityId("plugin.context.read".to_string()),
            None,
            plugin_context("plugin.context.read"),
        ),
        SecurityDecision::Deny(_)
    ));
}

#[test]
fn enterprise_policy_bundle_enforces_provider_budget_and_retention_rules() {
    let bundle = enterprise_bundle();
    let principal = PrincipalId("enterprise-user".to_string());

    assert_eq!(
        bundle.decide_with_request_context(
            ProductMode::Manual,
            TrustState::Trusted,
            principal.clone(),
            CapabilityId("ai.provider.invoke".to_string()),
            None,
            localhost_https_context(),
        ),
        SecurityDecision::Allow
    );

    let remote_provider_context = CapabilityRequestContext {
        network_target: Some(NetworkTarget {
            scheme: "https".to_string(),
            host: "example.com".to_string(),
            port: Some(443),
        }),
        ..CapabilityRequestContext::default()
    };
    assert!(matches!(
        bundle.decide_with_request_context(
            ProductMode::Manual,
            TrustState::Trusted,
            principal.clone(),
            CapabilityId("ai.provider.invoke".to_string()),
            None,
            remote_provider_context,
        ),
        SecurityDecision::Deny(_)
    ));

    let cloud_submit_ok = CapabilityRequestContext {
        network_target: Some(NetworkTarget {
            scheme: "https".to_string(),
            host: "localhost".to_string(),
            port: Some(443),
        }),
        cloud_lane_scope_visible_to_user: true,
        cloud_lane_task_packet_validated: true,
        cloud_lane_hard_cap_enforced: true,
        cloud_lane_estimated_cost_cents: Some(200),
        cloud_lane_upload_bytes: Some(1024),
        ..CapabilityRequestContext::default()
    };
    assert_eq!(
        bundle.decide_with_request_context(
            ProductMode::Assist,
            TrustState::Trusted,
            principal.clone(),
            CapabilityId("cloud.lane.submit".to_string()),
            None,
            cloud_submit_ok,
        ),
        SecurityDecision::Allow
    );

    let cloud_submit_over_budget = CapabilityRequestContext {
        network_target: Some(NetworkTarget {
            scheme: "https".to_string(),
            host: "localhost".to_string(),
            port: Some(443),
        }),
        cloud_lane_scope_visible_to_user: true,
        cloud_lane_task_packet_validated: true,
        cloud_lane_hard_cap_enforced: true,
        cloud_lane_estimated_cost_cents: Some(999),
        cloud_lane_upload_bytes: Some(2_000_000),
        ..CapabilityRequestContext::default()
    };
    assert!(matches!(
        bundle.decide_with_request_context(
            ProductMode::Assist,
            TrustState::Trusted,
            principal.clone(),
            CapabilityId("cloud.lane.submit".to_string()),
            None,
            cloud_submit_over_budget,
        ),
        SecurityDecision::Deny(_)
    ));

    let telemetry_context = localhost_https_context();
    assert!(matches!(
        bundle.decide_with_request_context(
            ProductMode::Manual,
            TrustState::Trusted,
            principal.clone(),
            CapabilityId("telemetry.export.hosted".to_string()),
            None,
            telemetry_context,
        ),
        SecurityDecision::Deny(_)
    ));

    let retention_context = CapabilityRequestContext {
        raw_source_retention_consent_current: true,
        raw_source_hosted_export_consent_current: true,
        ..CapabilityRequestContext::default()
    };
    assert!(matches!(
        bundle.decide_with_request_context(
            ProductMode::Manual,
            TrustState::Trusted,
            principal,
            CapabilityId("retention.raw_source.export.hosted".to_string()),
            None,
            retention_context,
        ),
        SecurityDecision::Deny(_)
    ));
}
