use legion_ai_providers::provider_setup_rows as ai_provider_setup_rows;
use legion_protocol::AssistedAiProviderClass;
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
