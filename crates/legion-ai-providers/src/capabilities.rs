//! Explicit provider capability matrix declarations for assisted-AI activation.

use legion_protocol::{
    AssistedAiCapabilityMatrix, AssistedAiProviderAvailabilityState, AssistedAiProviderClass,
    AssistedAiProviderTier, AssistedAiWorkspaceConsent, RedactionHint,
};

use crate::can_activate_provider;

/// Builds a metadata-only capability matrix with explicit labels.
#[allow(clippy::too_many_arguments)]
pub fn provider_capability_matrix(
    provider_id: impl Into<String>,
    provider_label: impl Into<String>,
    provider_class: AssistedAiProviderClass,
    supports_streaming: bool,
    supports_structured_output: bool,
    tool_labels: Vec<String>,
    structured_output_labels: Vec<String>,
    vision_labels: Vec<String>,
    context_length_label: impl Into<String>,
    context_length_tokens: Option<u32>,
    thinking_mode_labels: Vec<String>,
    cost_usage_label: impl Into<String>,
    availability: AssistedAiProviderAvailabilityState,
) -> AssistedAiCapabilityMatrix {
    AssistedAiCapabilityMatrix {
        provider_id: provider_id.into(),
        provider_label: provider_label.into(),
        provider_class,
        supports_streaming,
        supports_structured_output,
        tool_labels,
        structured_output_labels,
        vision_labels,
        context_length_label: context_length_label.into(),
        context_length_tokens,
        thinking_mode_labels,
        cost_usage_label: cost_usage_label.into(),
        availability,
        redaction_hints: vec![RedactionHint::MetadataOnly],
        schema_version: 1,
    }
}

/// Returns a zeroed capability matrix when the provider cannot be activated,
/// or the original matrix when activation succeeds.
///
/// Structural fields (provider_id, provider_label, provider_class, context lengths,
/// cost label, redaction hints, schema_version) are always preserved so callers
/// can still display provider metadata even for denied providers.
pub fn gate_provider_capabilities(
    matrix: &AssistedAiCapabilityMatrix,
    tier: AssistedAiProviderTier,
    consent: &AssistedAiWorkspaceConsent,
    has_credential: bool,
) -> AssistedAiCapabilityMatrix {
    if can_activate_provider(tier, consent, has_credential).is_ok() {
        matrix.clone()
    } else {
        AssistedAiCapabilityMatrix {
            provider_id: matrix.provider_id.clone(),
            provider_label: matrix.provider_label.clone(),
            provider_class: matrix.provider_class,
            supports_streaming: false,
            supports_structured_output: false,
            tool_labels: vec![],
            structured_output_labels: vec![],
            vision_labels: vec![],
            context_length_label: matrix.context_length_label.clone(),
            context_length_tokens: matrix.context_length_tokens,
            thinking_mode_labels: vec![],
            cost_usage_label: matrix.cost_usage_label.clone(),
            availability: AssistedAiProviderAvailabilityState::Unavailable,
            redaction_hints: matrix.redaction_hints.clone(),
            schema_version: matrix.schema_version,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn provider_capability_matrix_declares_explicit_metadata() {
        let matrix = provider_capability_matrix(
            "provider:matrix-anthropic",
            "Anthropic Messages",
            AssistedAiProviderClass::ByokRemote,
            true,
            true,
            vec!["strict_tools".to_string()],
            vec!["output_config.json_schema".to_string()],
            vec![],
            "provider-configured",
            None,
            vec!["thinking.budget_tokens".to_string()],
            "usage-reported",
            AssistedAiProviderAvailabilityState::Available,
        );

        assert!(matrix.has_explicit_declaration());
        assert!(matrix.supports_streaming);
        assert!(matrix.supports_structured_output);
        assert_eq!(matrix.tool_labels, vec!["strict_tools".to_string()]);
        assert_eq!(
            matrix.thinking_mode_labels,
            vec!["thinking.budget_tokens".to_string()]
        );
    }
}
