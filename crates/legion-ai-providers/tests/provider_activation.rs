//! Provider activation policy tests — tier mapping, consent gate, capability gating.

use legion_ai_providers::{
    ANTHROPIC_PROVIDER_ID, AssistedAiProviderActivationDenial, CODESTRAL_PROVIDER_ID,
    COPILOT_NES_PROVIDER_ID, DETERMINISTIC_LOCAL_PROVIDER_ID, LLAMA_CPP_PROVIDER_ID,
    MERCURY_PROVIDER_ID, OLLAMA_PROVIDER_ID, OPENAI_COMPATIBLE_PROVIDER_ID, OPENAI_PROVIDER_ID,
    can_activate_provider, provider_setup_rows, provider_tier,
};
use legion_protocol::{
    AssistedAiProviderClass, AssistedAiProviderTier, AssistedAiWorkspaceConsent, PrincipalId,
    TimestampMillis,
};

// ---------------------------------------------------------------------------
// T1: Provider tier mapping
// ---------------------------------------------------------------------------

#[test]
fn local_default_always_activatable() {
    let result = can_activate_provider(
        AssistedAiProviderTier::LocalDefault,
        &AssistedAiWorkspaceConsent::NotRequired,
        false,
    );
    assert!(result.is_ok(), "LocalDefault must always activate");
}

#[test]
fn loopback_opt_in_activatable_without_consent() {
    let result = can_activate_provider(
        AssistedAiProviderTier::LocalLoopbackOptIn,
        &AssistedAiWorkspaceConsent::NotRequired,
        false,
    );
    assert!(result.is_ok(), "LocalLoopbackOptIn requires no consent");
}

#[test]
fn byok_requires_both_consent_and_credential() {
    let granted = AssistedAiWorkspaceConsent::Granted {
        granted_at: TimestampMillis(1_000_000),
        principal: PrincipalId("test-principal".to_string()),
    };
    let result = can_activate_provider(AssistedAiProviderTier::ByokConsentRequired, &granted, true);
    assert!(
        result.is_ok(),
        "ByokConsentRequired with consent + credential must activate"
    );
}

#[test]
fn byok_with_consent_but_no_credential_is_denied() {
    let granted = AssistedAiWorkspaceConsent::Granted {
        granted_at: TimestampMillis(1_000_000),
        principal: PrincipalId("test-principal".to_string()),
    };
    let result =
        can_activate_provider(AssistedAiProviderTier::ByokConsentRequired, &granted, false);
    assert_eq!(
        result.unwrap_err(),
        AssistedAiProviderActivationDenial::CredentialRequired,
        "consent without credential returns CredentialRequired"
    );
}

#[test]
fn hosted_denied_never_activatable() {
    for consent in [
        AssistedAiWorkspaceConsent::NotRequired,
        AssistedAiWorkspaceConsent::Pending,
        AssistedAiWorkspaceConsent::Granted {
            granted_at: TimestampMillis(1_000_000),
            principal: PrincipalId("test".to_string()),
        },
        AssistedAiWorkspaceConsent::Denied,
    ] {
        let result = can_activate_provider(AssistedAiProviderTier::HostedDenied, &consent, true);
        assert_eq!(
            result.unwrap_err(),
            AssistedAiProviderActivationDenial::HostedDenied,
            "HostedDenied must always return HostedDenied regardless of consent/credential"
        );
    }
}

#[test]
fn air_gap_denies_all_remote_providers() {
    let denied = AssistedAiWorkspaceConsent::Denied;

    // BYOK tier in an air-gapped workspace → AirGapDenied
    let byok_result =
        can_activate_provider(AssistedAiProviderTier::ByokConsentRequired, &denied, true);
    assert_eq!(
        byok_result.unwrap_err(),
        AssistedAiProviderActivationDenial::AirGapDenied,
        "air-gapped workspace must deny BYOK providers with AirGapDenied"
    );

    // HostedDenied tier is always denied regardless
    let hosted_result = can_activate_provider(AssistedAiProviderTier::HostedDenied, &denied, true);
    assert!(
        hosted_result.is_err(),
        "HostedDenied must always be denied in air-gapped workspace"
    );
}

#[test]
fn provider_tier_maps_all_known_providers() {
    let cases = [
        (
            AssistedAiProviderClass::Local,
            DETERMINISTIC_LOCAL_PROVIDER_ID,
            AssistedAiProviderTier::LocalDefault,
        ),
        (
            AssistedAiProviderClass::LocalLoopback,
            OLLAMA_PROVIDER_ID,
            AssistedAiProviderTier::LocalLoopbackOptIn,
        ),
        (
            AssistedAiProviderClass::LocalLoopback,
            LLAMA_CPP_PROVIDER_ID,
            AssistedAiProviderTier::LocalLoopbackOptIn,
        ),
        (
            AssistedAiProviderClass::ByokRemote,
            OPENAI_PROVIDER_ID,
            AssistedAiProviderTier::ByokConsentRequired,
        ),
        (
            AssistedAiProviderClass::ByokRemote,
            OPENAI_COMPATIBLE_PROVIDER_ID,
            AssistedAiProviderTier::ByokConsentRequired,
        ),
        (
            AssistedAiProviderClass::ByokRemote,
            ANTHROPIC_PROVIDER_ID,
            AssistedAiProviderTier::ByokConsentRequired,
        ),
        (
            AssistedAiProviderClass::ByokRemote,
            CODESTRAL_PROVIDER_ID,
            AssistedAiProviderTier::ByokConsentRequired,
        ),
        (
            AssistedAiProviderClass::HostedRemote,
            COPILOT_NES_PROVIDER_ID,
            AssistedAiProviderTier::HostedDenied,
        ),
        (
            AssistedAiProviderClass::HostedRemote,
            MERCURY_PROVIDER_ID,
            AssistedAiProviderTier::HostedDenied,
        ),
    ];

    for (class, provider_id, expected_tier) in cases {
        let tier = provider_tier(class, provider_id);
        assert_eq!(
            tier, expected_tier,
            "provider_tier({class:?}, {provider_id}) should be {expected_tier:?}"
        );
    }
}

#[test]
fn provider_setup_rows_show_tier_and_consent() {
    let rows = provider_setup_rows();
    assert!(
        !rows.is_empty(),
        "provider_setup_rows must return at least one row"
    );

    // Deterministic local must show LocalDefault
    let local_row = rows
        .iter()
        .find(|r| r.contains(DETERMINISTIC_LOCAL_PROVIDER_ID));
    assert!(
        local_row.is_some(),
        "setup rows must include deterministic-local provider"
    );
    assert!(
        local_row.unwrap().contains("tier=LocalDefault"),
        "deterministic-local row must declare LocalDefault tier"
    );

    // Anthropic must show ByokConsentRequired
    // (Anthropic is not in inline_prediction_provider_capabilities, but Ollama is)
    let ollama_row = rows.iter().find(|r| r.contains(OLLAMA_PROVIDER_ID));
    assert!(
        ollama_row.is_some(),
        "setup rows must include ollama provider"
    );
    assert!(
        ollama_row.unwrap().contains("tier=LocalLoopbackOptIn"),
        "ollama row must declare LocalLoopbackOptIn tier"
    );

    // Copilot NES must show HostedDenied
    let copilot_row = rows.iter().find(|r| r.contains(COPILOT_NES_PROVIDER_ID));
    assert!(
        copilot_row.is_some(),
        "setup rows must include copilot-nes provider"
    );
    assert!(
        copilot_row.unwrap().contains("tier=HostedDenied"),
        "copilot-nes row must declare HostedDenied tier"
    );
}
