use legion_ai_providers::{
    capabilities::{gate_provider_capabilities, provider_capability_matrix},
    provider_setup_rows as ai_provider_setup_rows, provider_tier,
};
use legion_protocol::{
    AssistedAiProviderAvailabilityState, AssistedAiProviderClass, AssistedAiProviderTier,
    AssistedAiWorkspaceConsent,
};
use legion_ui::ShellProjectionSnapshot;

pub(crate) fn setup_rows(snapshot: &ShellProjectionSnapshot) -> Vec<String> {
    let assisted = &snapshot.assisted_ai_projection;
    let (local_count, byok_count, hosted_count) = assisted.providers.iter().fold(
        (0usize, 0usize, 0usize),
        |(local_count, byok_count, hosted_count), provider| match provider.provider_class {
            AssistedAiProviderClass::Local | AssistedAiProviderClass::LocalLoopback => {
                (local_count + 1, byok_count, hosted_count)
            }
            AssistedAiProviderClass::ByokRemote => (local_count, byok_count + 1, hosted_count),
            AssistedAiProviderClass::HostedRemote => (local_count, byok_count, hosted_count + 1),
            AssistedAiProviderClass::Gateway | AssistedAiProviderClass::Unknown => {
                (local_count, byok_count, hosted_count)
            }
        },
    );

    let mut rows = vec![format!(
        "provider setup: local-first defaults with {} projected providers; deterministic local stays the default",
        assisted.provider_count
    )];
    rows.extend(ai_provider_setup_rows());
    rows.push(format!(
        "provider setup: {} local/loopback providers, {} BYOK providers, {} hosted providers projected",
        local_count, byok_count, hosted_count
    ));
    rows.push(
        "provider setup: hosted provider calls require explicit workspace consent; air-gap keeps hosted providers unavailable"
            .to_string(),
    );

    // Project per-provider tier, consent state, credential presence, and activation eligibility.
    for provider in assisted.providers.iter().take(8) {
        let tier = provider_tier(provider.provider_class, &provider.provider_id);
        let tier_label = match tier {
            AssistedAiProviderTier::LocalDefault => "LocalDefault",
            AssistedAiProviderTier::LocalLoopbackOptIn => "LocalLoopbackOptIn",
            AssistedAiProviderTier::ByokConsentRequired => "ByokConsentRequired",
            AssistedAiProviderTier::HostedDenied => "HostedDenied",
        };
        let consent_state = match tier {
            AssistedAiProviderTier::LocalDefault
            | AssistedAiProviderTier::LocalLoopbackOptIn => "NotRequired",
            AssistedAiProviderTier::ByokConsentRequired => "Required",
            AssistedAiProviderTier::HostedDenied => "N/A",
        };
        // Credential presence is derived from availability metadata — never the credential value.
        let credential_present = provider.availability == AssistedAiProviderAvailabilityState::Available;
        let credential_label = if credential_present { "Present" } else { "Absent" };
        // Derive consent from tier: local tiers need no consent, BYOK defaults to Pending
        // (ConsentRequired) as the safe default until explicit consent is recorded, and
        // air-gapped workspaces map to Denied. This wires gate_provider_capabilities into
        // the capability projection path (C1) and correctly handles the air-gap case (I3).
        let consent = match tier {
            AssistedAiProviderTier::LocalDefault
            | AssistedAiProviderTier::LocalLoopbackOptIn => AssistedAiWorkspaceConsent::NotRequired,
            AssistedAiProviderTier::ByokConsentRequired => AssistedAiWorkspaceConsent::Pending,
            AssistedAiProviderTier::HostedDenied => AssistedAiWorkspaceConsent::Denied,
        };
        // Build a probe matrix and apply the activation gate; the gated availability
        // determines eligibility instead of the inline ad-hoc check.
        let probe = provider_capability_matrix(
            &provider.provider_id,
            &provider.provider_label,
            provider.provider_class,
            false,
            false,
            vec![],
            vec![],
            vec![],
            "N/A",
            None,
            vec![],
            "N/A",
            AssistedAiProviderAvailabilityState::Available,
        );
        let gated = gate_provider_capabilities(&probe, tier, &consent, credential_present);
        let eligible = gated.availability == AssistedAiProviderAvailabilityState::Available;
        let eligible_label = if eligible { "Eligible" } else { "NotEligible" };
        rows.push(format!(
            "provider {}: tier={} consent={} credential={} activation={}",
            provider.provider_id, tier_label, consent_state, credential_label, eligible_label
        ));
    }
    rows
}

pub(crate) fn provider_policy_rows(snapshot: &ShellProjectionSnapshot) -> Vec<String> {
    snapshot
        .assisted_ai_projection
        .providers
        .iter()
        .take(4)
        .map(|provider| {
            let (byok, local, offline, air_gap, consent) = match provider.provider_class {
                AssistedAiProviderClass::Local => (
                    "Unsupported",
                    "Supported",
                    "Supported",
                    "Supported",
                    "NotRequired",
                ),
                AssistedAiProviderClass::LocalLoopback => (
                    "Unsupported",
                    "Supported",
                    "Unsupported",
                    "Unsupported",
                    "NotRequired",
                ),
                AssistedAiProviderClass::ByokRemote => (
                    "ApprovalRequired",
                    "Unsupported",
                    "Unsupported",
                    "Unsupported",
                    "ApprovalRequired",
                ),
                AssistedAiProviderClass::HostedRemote => (
                    "Unsupported",
                    "Unsupported",
                    "Unsupported",
                    "Unsupported",
                    "ApprovalRequired",
                ),
                AssistedAiProviderClass::Gateway | AssistedAiProviderClass::Unknown => {
                    ("Unknown", "Unknown", "Unknown", "Unknown", "Unknown")
                }
            };
            format!(
                "{} policy: byok={} local={} offline={} air_gap={} consent={}",
                provider.provider_id, byok, local, offline, air_gap, consent
            )
        })
        .collect()
}
