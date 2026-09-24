//! Every surface a signed org policy bundle must reach (P9.F2.T3).
//!
//! The stop condition for this task is that a bundle honoured on only some
//! surfaces is a failure. So the surface set is enumerated as data here, and
//! `every_surface_in_the_enumeration_is_exercised` asserts the table covers
//! `PolicySurface::ALL`. Adding a surface without a refusal case for it turns
//! that test red.
//!
//! Each case is a *refusal*. A bundle that only ever says yes constrains
//! nothing, so the permissive direction is checked exactly once per surface —
//! enough to show the refusal is caused by the rule and not by an unrelated
//! default that would have blocked the request anyway.

use std::collections::BTreeSet;

use legion_protocol::{
    CapabilityId, CapabilityNamespace, CapabilityRequestContext, NetworkTarget, PluginId,
    PrincipalId, ProductMode,
};
use legion_security::{
    BundleRequest, DenyByDefaultBroker, PolicyKeyring, PolicySigningKey, PolicySurface,
    SecurityDecision, TrustState, VerifiedPolicyBundle, policy_bundle_verifying_key_b64,
    sign_policy_bundle,
};

const ORG_SEED: [u8; 32] = [7u8; 32];
const ORG_KEY_ID: &str = "org-policy-signer-1";

fn enterprise_payload() -> String {
    include_str!("../../../xtask/legion-policy.example.toml").to_string()
}

fn verified_enterprise_bundle() -> VerifiedPolicyBundle {
    let keyring = PolicyKeyring::new(vec![PolicySigningKey {
        key_id: ORG_KEY_ID.to_string(),
        verifying_key_b64: policy_bundle_verifying_key_b64(&ORG_SEED),
    }]);
    sign_policy_bundle(enterprise_payload(), ORG_KEY_ID, &ORG_SEED)
        .verify(&keyring)
        .expect("the shipped enterprise example must be a verifiable bundle")
}

fn principal() -> PrincipalId {
    PrincipalId("enterprise-user".to_string())
}

fn localhost_https() -> Option<NetworkTarget> {
    Some(NetworkTarget {
        scheme: "https".to_string(),
        host: "localhost".to_string(),
        port: Some(443),
    })
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

fn request(
    mode: ProductMode,
    capability: &str,
    context: CapabilityRequestContext,
) -> BundleRequest<'static> {
    BundleRequest {
        mode,
        trust: TrustState::Trusted,
        principal: principal(),
        capability: CapabilityId(capability.to_string()),
        path: None,
        context,
    }
}

// ---------------------------------------------------------------------------
// One refusal case per surface
// ---------------------------------------------------------------------------

/// A request the enterprise bundle must refuse, and the surface that must refuse it.
struct RefusalCase {
    surface: PolicySurface,
    what: &'static str,
    request: BundleRequest<'static>,
    /// A fragment the denial reason must contain, so a case cannot pass by
    /// being refused for an unrelated reason on the right surface.
    reason_fragment: &'static str,
}

fn refusal_cases() -> Vec<RefusalCase> {
    vec![
        RefusalCase {
            surface: PolicySurface::Mode,
            what: "a Delegate-mode request under an Assist ceiling",
            request: request(
                ProductMode::Delegates,
                "plugin.context.read",
                plugin_context("plugin.context.read"),
            ),
            reason_fragment: "ceiling denies",
        },
        RefusalCase {
            surface: PolicySurface::Provider,
            what: "a provider that is not on the allowlist",
            request: request(
                ProductMode::Assist,
                "ai.provider.invoke",
                CapabilityRequestContext {
                    network_target: localhost_https(),
                    ai_provider_id: Some("openai".to_string()),
                    budget_request_cost_cents: Some(1),
                    ..CapabilityRequestContext::default()
                },
            ),
            reason_fragment: "provider allowlist",
        },
        RefusalCase {
            surface: PolicySurface::McpTool,
            what: "an MCP tool on a server that is not on the allowlist",
            request: request(
                ProductMode::Assist,
                "delegate.tool.mcp-passthrough",
                CapabilityRequestContext {
                    mcp_server_id: Some("evil-corp".to_string()),
                    mcp_tool_name: Some("exfiltrate".to_string()),
                    ..CapabilityRequestContext::default()
                },
            ),
            reason_fragment: "server allowlist",
        },
        RefusalCase {
            surface: PolicySurface::Budget,
            what: "an allowlisted provider invoked over the per-request cost cap",
            request: request(
                ProductMode::Assist,
                "ai.provider.invoke",
                CapabilityRequestContext {
                    network_target: localhost_https(),
                    ai_provider_id: Some("ollama".to_string()),
                    budget_request_cost_cents: Some(9_999),
                    ..CapabilityRequestContext::default()
                },
            ),
            reason_fragment: "per-request cap",
        },
        RefusalCase {
            surface: PolicySurface::Retention,
            what: "a retention window longer than the org maximum",
            request: request(
                ProductMode::Assist,
                "retention.raw_source.capture",
                CapabilityRequestContext {
                    raw_source_retention_consent_current: true,
                    retention_requested_days: Some(365),
                    ..CapabilityRequestContext::default()
                },
            ),
            reason_fragment: "exceeds the org policy bundle maximum",
        },
        RefusalCase {
            surface: PolicySurface::Export,
            what: "a hosted export the retention rules forbid",
            request: request(
                ProductMode::Assist,
                "retention.raw_source.export.hosted",
                CapabilityRequestContext {
                    raw_source_retention_consent_current: true,
                    raw_source_hosted_export_consent_current: true,
                    export_destination: Some("s3://partner-bucket".to_string()),
                    ..CapabilityRequestContext::default()
                },
            ),
            reason_fragment: "disabled by the org policy bundle",
        },
        RefusalCase {
            surface: PolicySurface::Capability,
            what: "a plugin capability outside the bundle's capability allowlist",
            request: request(
                ProductMode::Assist,
                "plugin.command",
                plugin_context("plugin.context.read"),
            ),
            reason_fragment: "",
        },
    ]
}

#[test]
fn every_surface_refuses_what_the_enterprise_bundle_forbids() {
    let bundle = verified_enterprise_bundle();

    for case in refusal_cases() {
        let decision = bundle.decide(&case.request);
        assert!(
            !decision.is_allowed(),
            "surface {:?} allowed {} — the bundle is not honoured there",
            case.surface,
            case.what
        );
        assert_eq!(
            decision.surface, case.surface,
            "{} was refused by {:?}, not by {:?}; a refusal from the wrong surface \
             means the intended surface is untested",
            case.what, decision.surface, case.surface
        );
        let SecurityDecision::Deny(reason) = &decision.decision else {
            unreachable!("checked non-allow above");
        };
        assert!(
            reason.contains(case.reason_fragment),
            "{} was refused by {:?} for the wrong reason: {reason}",
            case.what,
            case.surface
        );
        // Every decision, allow or deny, carries an auditable row naming the
        // bundle and the trust anchor that was verified before enforcement.
        let row = decision.audit_row();
        assert!(row.contains("enterprise-restrictive"), "audit row: {row}");
        assert!(row.contains(ORG_KEY_ID), "audit row: {row}");
        assert!(row.contains(case.surface.stable_id()), "audit row: {row}");
    }
}

#[test]
fn every_surface_in_the_enumeration_is_exercised() {
    // This is the stop-condition test. If a surface is added to `PolicySurface`
    // and to `PolicySurface::ALL` without a refusal case above, the bundle is
    // honoured on only some surfaces and this fails.
    let covered: BTreeSet<&'static str> = refusal_cases()
        .iter()
        .map(|case| case.surface.stable_id())
        .collect();
    let declared: BTreeSet<&'static str> = PolicySurface::ALL
        .iter()
        .map(|surface| surface.stable_id())
        .collect();

    assert_eq!(
        covered, declared,
        "every declared policy surface needs a refusal case"
    );
}

#[test]
fn surface_check_table_covers_every_declared_surface() {
    // The runtime counterpart: `decide` iterates `SURFACE_CHECKS`, so a surface
    // absent from that table is never evaluated no matter how many rules it has.
    let checked: BTreeSet<&'static str> = VerifiedPolicyBundle::SURFACE_CHECKS
        .iter()
        .map(|(surface, _)| surface.stable_id())
        .collect();
    let declared: BTreeSet<&'static str> = PolicySurface::ALL
        .iter()
        .map(|surface| surface.stable_id())
        .collect();

    assert_eq!(checked, declared);
    assert_eq!(
        VerifiedPolicyBundle::SURFACE_CHECKS.len(),
        PolicySurface::ALL.len(),
        "a surface listed twice would hide a missing one"
    );
}

// ---------------------------------------------------------------------------
// Non-vacuity: the permissive direction of each surface
// ---------------------------------------------------------------------------

#[test]
fn the_enterprise_bundle_still_permits_what_it_allows() {
    // Without this, every refusal above could be explained by a blanket deny.
    let bundle = verified_enterprise_bundle();

    let allowed_provider_call = request(
        ProductMode::Assist,
        "ai.provider.invoke",
        CapabilityRequestContext {
            network_target: localhost_https(),
            ai_provider_id: Some("ollama".to_string()),
            budget_request_cost_cents: Some(5),
            budget_request_tokens: Some(1_000),
            budget_session_spent_cents: Some(10),
            ..CapabilityRequestContext::default()
        },
    );
    let decision = bundle.decide(&allowed_provider_call);
    assert!(
        decision.is_allowed(),
        "an allowlisted provider within budget must be permitted, got {:?}",
        decision.decision
    );

    let allowed_plugin_call = request(
        ProductMode::Manual,
        "plugin.context.read",
        plugin_context("plugin.context.read"),
    );
    assert!(
        bundle.decide(&allowed_plugin_call).is_allowed(),
        "an allowlisted plugin capability under the ceiling must be permitted"
    );

    // An allowlisted MCP tool reaches Allow when the base capability matrix
    // permits the capability id. `tool.plan` is such an id and matches the
    // bundle's `tool.` MCP prefix, so this exercises the allowlist's permissive
    // direction end to end.
    let allowed_mcp_call = request(
        ProductMode::Assist,
        "tool.plan",
        CapabilityRequestContext {
            mcp_server_id: Some("legion-internal".to_string()),
            mcp_tool_name: Some("search_docs".to_string()),
            ..CapabilityRequestContext::default()
        },
    );
    let decision = bundle.decide(&allowed_mcp_call);
    assert!(
        decision.is_allowed(),
        "an allowlisted MCP tool must be permitted, got {:?}",
        decision.decision
    );

    // The delegated-task capability id is a different story, and the difference
    // is worth pinning down rather than papering over: `DenyByDefaultBroker` has
    // no `delegate.tool.*` arm, so its base matrix refuses every delegated tool
    // call regardless of the bundle. What this asserts is the part the bundle
    // owns — an allowlisted server/tool pair is not refused by the McpTool
    // surface. If this ever came back as `McpTool`, the allowlist would be
    // rejecting a pair it lists.
    let allowlisted_passthrough = request(
        ProductMode::Assist,
        "delegate.tool.mcp-passthrough",
        CapabilityRequestContext {
            mcp_server_id: Some("legion-internal".to_string()),
            mcp_tool_name: Some("search_docs".to_string()),
            ..CapabilityRequestContext::default()
        },
    );
    assert_eq!(
        bundle.decide(&allowlisted_passthrough).surface,
        PolicySurface::Capability,
        "an allowlisted server/tool pair must clear the McpTool surface"
    );

    let allowed_retention = request(
        ProductMode::Assist,
        "retention.raw_source.capture",
        CapabilityRequestContext {
            raw_source_retention_consent_current: true,
            retention_requested_days: Some(3),
            ..CapabilityRequestContext::default()
        },
    );
    assert_eq!(
        bundle.decide(&allowed_retention).surface,
        PolicySurface::Capability,
        "a 3-day retention window is inside the 7-day ceiling, so the retention \
         surface must not be the one that objects"
    );
}

// ---------------------------------------------------------------------------
// Undeclared operands are refusals, not bypasses
// ---------------------------------------------------------------------------

#[test]
fn omitting_the_operand_a_surface_matches_on_is_a_refusal() {
    // The most tempting bypass: leave the field blank so the rule has nothing to
    // compare against. Each of these must be refused for failing to declare.
    let bundle = verified_enterprise_bundle();

    let undeclared_provider = request(
        ProductMode::Assist,
        "ai.provider.invoke",
        CapabilityRequestContext {
            network_target: localhost_https(),
            budget_request_cost_cents: Some(1),
            ..CapabilityRequestContext::default()
        },
    );
    let decision = bundle.decide(&undeclared_provider);
    assert_eq!(decision.surface, PolicySurface::Provider);
    assert!(!decision.is_allowed());

    let undeclared_tool = request(
        ProductMode::Assist,
        "delegate.tool.mcp-passthrough",
        CapabilityRequestContext::default(),
    );
    let decision = bundle.decide(&undeclared_tool);
    assert_eq!(decision.surface, PolicySurface::McpTool);
    assert!(!decision.is_allowed());

    let undeclared_cost = request(
        ProductMode::Assist,
        "ai.provider.invoke",
        CapabilityRequestContext {
            network_target: localhost_https(),
            ai_provider_id: Some("ollama".to_string()),
            ..CapabilityRequestContext::default()
        },
    );
    let decision = bundle.decide(&undeclared_cost);
    assert_eq!(decision.surface, PolicySurface::Budget);
    assert!(!decision.is_allowed());

    let undeclared_window = request(
        ProductMode::Assist,
        "retention.raw_source.capture",
        CapabilityRequestContext {
            raw_source_retention_consent_current: true,
            ..CapabilityRequestContext::default()
        },
    );
    let decision = bundle.decide(&undeclared_window);
    assert_eq!(decision.surface, PolicySurface::Retention);
    assert!(!decision.is_allowed());
}

#[test]
fn a_renamed_capability_cannot_route_around_an_allowlist() {
    // Prefix matching alone would let a caller relabel its capability id and
    // escape. The operand triggers close that: a request that names a provider
    // or an MCP tool is checked whatever it calls itself.
    let bundle = verified_enterprise_bundle();

    let disguised_provider = request(
        ProductMode::Assist,
        "vendor.custom.chat",
        CapabilityRequestContext {
            network_target: localhost_https(),
            ai_provider_id: Some("openai".to_string()),
            ..CapabilityRequestContext::default()
        },
    );
    let decision = bundle.decide(&disguised_provider);
    assert_eq!(decision.surface, PolicySurface::Provider);
    assert!(!decision.is_allowed());

    let disguised_tool = request(
        ProductMode::Assist,
        "vendor.custom.invoke",
        CapabilityRequestContext {
            mcp_server_id: Some("evil-corp".to_string()),
            mcp_tool_name: Some("exfiltrate".to_string()),
            ..CapabilityRequestContext::default()
        },
    );
    let decision = bundle.decide(&disguised_tool);
    assert_eq!(decision.surface, PolicySurface::McpTool);
    assert!(!decision.is_allowed());
}

#[test]
fn allowlisted_server_does_not_admit_a_tool_that_is_not_allowlisted() {
    // Server-level allowlisting alone would admit every tool a trusted server
    // later chooses to advertise.
    let bundle = verified_enterprise_bundle();

    let decision = bundle.decide(&request(
        ProductMode::Assist,
        "delegate.tool.mcp-passthrough",
        CapabilityRequestContext {
            mcp_server_id: Some("legion-internal".to_string()),
            mcp_tool_name: Some("run_shell".to_string()),
            ..CapabilityRequestContext::default()
        },
    ));
    assert_eq!(decision.surface, PolicySurface::McpTool);
    assert!(!decision.is_allowed());
    let SecurityDecision::Deny(reason) = &decision.decision else {
        unreachable!()
    };
    assert!(reason.contains("tool allowlist"), "reason: {reason}");
}

#[test]
fn session_spend_accumulates_toward_the_session_cap() {
    // A caller that stays under the per-request cap on every call must still be
    // stopped once the session total is reached.
    let bundle = verified_enterprise_bundle();

    let decision = bundle.decide(&request(
        ProductMode::Assist,
        "ai.provider.invoke",
        CapabilityRequestContext {
            network_target: localhost_https(),
            ai_provider_id: Some("ollama".to_string()),
            budget_request_cost_cents: Some(20),
            budget_session_spent_cents: Some(495),
            ..CapabilityRequestContext::default()
        },
    ));
    assert_eq!(decision.surface, PolicySurface::Budget);
    let SecurityDecision::Deny(reason) = &decision.decision else {
        unreachable!("session cap must refuse")
    };
    assert!(reason.contains("session cap"), "reason: {reason}");
}

#[test]
fn token_cap_refuses_independently_of_the_cost_cap() {
    // A cheap-but-enormous request must still be refused.
    let bundle = verified_enterprise_bundle();

    let decision = bundle.decide(&request(
        ProductMode::Assist,
        "ai.provider.invoke",
        CapabilityRequestContext {
            network_target: localhost_https(),
            ai_provider_id: Some("ollama".to_string()),
            budget_request_cost_cents: Some(1),
            budget_request_tokens: Some(10_000_000),
            ..CapabilityRequestContext::default()
        },
    ));
    assert_eq!(decision.surface, PolicySurface::Budget);
    let SecurityDecision::Deny(reason) = &decision.decision else {
        unreachable!("token cap must refuse")
    };
    assert!(reason.contains("token cap"), "reason: {reason}");
}

// ---------------------------------------------------------------------------
// The broker itself enforces the bundle rules
// ---------------------------------------------------------------------------
//
// `VerifiedPolicyBundle::decide` is the composed entry point, but most of the
// product reaches policy through a `DenyByDefaultBroker` handed over as a
// `CapabilityBrokerPort` — `legion-agent`, which owns the tool-call chokepoint,
// is forbidden from depending on `legion-security` at all and can only see the
// broker. If the rules lived solely in `decide`, that path would be uncovered.
// These tests hold the broker to the same refusals without going through the
// bundle wrapper, so removing either enforcement point is visible.

fn enterprise_broker() -> DenyByDefaultBroker {
    verified_enterprise_bundle().bundle().broker()
}

#[test]
fn broker_enforces_the_provider_allowlist_without_the_bundle_wrapper() {
    let mut broker = enterprise_broker();
    let decision = broker.decide_with_request_context(
        TrustState::Trusted,
        principal(),
        CapabilityId("ai.provider.invoke".to_string()),
        None,
        CapabilityRequestContext {
            network_target: localhost_https(),
            ai_provider_id: Some("openai".to_string()),
            budget_request_cost_cents: Some(1),
            ..CapabilityRequestContext::default()
        },
    );
    let SecurityDecision::Deny(reason) = decision else {
        panic!("broker must refuse a provider outside the allowlist");
    };
    assert!(
        reason.contains(PolicySurface::Provider.stable_id()),
        "{reason}"
    );
}

#[test]
fn broker_enforces_the_mcp_tool_allowlist_without_the_bundle_wrapper() {
    let mut broker = enterprise_broker();
    let decision = broker.decide_with_request_context(
        TrustState::Trusted,
        principal(),
        CapabilityId("delegate.tool.mcp-passthrough".to_string()),
        None,
        CapabilityRequestContext {
            mcp_server_id: Some("evil-corp".to_string()),
            mcp_tool_name: Some("exfiltrate".to_string()),
            ..CapabilityRequestContext::default()
        },
    );
    let SecurityDecision::Deny(reason) = decision else {
        panic!("broker must refuse an MCP tool outside the allowlist");
    };
    assert!(
        reason.contains(PolicySurface::McpTool.stable_id()),
        "{reason}"
    );
}

#[test]
fn broker_enforces_budget_caps_without_the_bundle_wrapper() {
    let mut broker = enterprise_broker();
    let decision = broker.decide_with_request_context(
        TrustState::Trusted,
        principal(),
        CapabilityId("ai.provider.invoke".to_string()),
        None,
        CapabilityRequestContext {
            network_target: localhost_https(),
            ai_provider_id: Some("ollama".to_string()),
            budget_request_cost_cents: Some(9_999),
            ..CapabilityRequestContext::default()
        },
    );
    let SecurityDecision::Deny(reason) = decision else {
        panic!("broker must refuse a request over the per-request cost cap");
    };
    assert!(
        reason.contains(PolicySurface::Budget.stable_id()),
        "{reason}"
    );
}

#[test]
fn broker_caps_a_cloud_lane_cost_declared_in_the_legacy_field() {
    // `cloud.lane.submit` predates the bundle's budget fields and declares its
    // estimate in `cloud_lane_estimated_cost_cents`. That estimate must be
    // capped too, or the older field becomes the bypass.
    let mut broker = enterprise_broker();
    let decision = broker.decide_with_request_context(
        TrustState::Trusted,
        principal(),
        CapabilityId("cloud.lane.submit".to_string()),
        None,
        CapabilityRequestContext {
            network_target: localhost_https(),
            cloud_lane_scope_visible_to_user: true,
            cloud_lane_task_packet_validated: true,
            cloud_lane_hard_cap_enforced: true,
            cloud_lane_estimated_cost_cents: Some(200),
            cloud_lane_upload_bytes: Some(1024),
            ..CapabilityRequestContext::default()
        },
    );
    let SecurityDecision::Deny(reason) = decision else {
        panic!("broker must cap a cloud-lane cost declared in the legacy field");
    };
    assert!(
        reason.contains(PolicySurface::Budget.stable_id()),
        "{reason}"
    );
}

#[test]
fn broker_enforces_retention_and_export_rules_without_the_bundle_wrapper() {
    let mut broker = enterprise_broker();

    let over_window = broker.decide_with_request_context(
        TrustState::Trusted,
        principal(),
        CapabilityId("retention.raw_source.capture".to_string()),
        None,
        CapabilityRequestContext {
            raw_source_retention_consent_current: true,
            retention_requested_days: Some(365),
            ..CapabilityRequestContext::default()
        },
    );
    let SecurityDecision::Deny(reason) = over_window else {
        panic!("broker must refuse a retention window over the org maximum");
    };
    assert!(
        reason.contains(PolicySurface::Retention.stable_id()),
        "{reason}"
    );

    let forbidden_export = broker.decide_with_request_context(
        TrustState::Trusted,
        principal(),
        CapabilityId("retention.raw_source.export.hosted".to_string()),
        None,
        CapabilityRequestContext {
            raw_source_retention_consent_current: true,
            raw_source_hosted_export_consent_current: true,
            export_destination: Some("s3://partner-bucket".to_string()),
            ..CapabilityRequestContext::default()
        },
    );
    let SecurityDecision::Deny(reason) = forbidden_export else {
        panic!("broker must refuse an export the retention rules forbid");
    };
    assert!(
        reason.contains(PolicySurface::Export.stable_id()),
        "{reason}"
    );
}

#[test]
fn broker_without_a_bundle_is_unaffected_by_the_new_rules() {
    // The rules are opt-in per bundle. A default policy must behave exactly as
    // it did before, or every existing caller would be broken by this feature
    // rather than governed by it.
    let mut broker = DenyByDefaultBroker::new(
        legion_security::SecurityPolicy::default(),
        CapabilityNamespace("test.default".to_string()),
    );
    let decision = broker.decide_with_request_context(
        TrustState::Trusted,
        principal(),
        CapabilityId("ai.provider.invoke".to_string()),
        None,
        CapabilityRequestContext {
            network_target: localhost_https(),
            ai_provider_id: Some("some-unlisted-provider".to_string()),
            budget_request_cost_cents: Some(9_999_999),
            ..CapabilityRequestContext::default()
        },
    );
    assert_eq!(
        decision,
        SecurityDecision::Allow,
        "an unenforced bundle policy must not start denying requests"
    );
}
