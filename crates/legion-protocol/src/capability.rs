//! Metadata-only provider capability matrices shared across assisted-AI routing surfaces.

use serde::{Deserialize, Serialize};

use crate::{AssistedAiProviderAvailabilityState, AssistedAiProviderClass, RedactionHint};

/// Metadata-only capability matrix for a provider slot.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AssistedAiCapabilityMatrix {
    /// Stable provider identifier.
    pub provider_id: String,
    /// Display-safe provider label.
    pub provider_label: String,
    /// Provider execution class.
    pub provider_class: AssistedAiProviderClass,
    /// Whether the slot explicitly declares streaming support.
    pub supports_streaming: bool,
    /// Whether the slot explicitly declares structured-output support.
    pub supports_structured_output: bool,
    /// Tool capability labels without payloads or execution authority.
    pub tool_labels: Vec<String>,
    /// Structured-output capability labels without payloads.
    pub structured_output_labels: Vec<String>,
    /// Vision capability labels without payloads.
    pub vision_labels: Vec<String>,
    /// Context-length display label.
    pub context_length_label: String,
    /// Context length in tokens, when known.
    pub context_length_tokens: Option<u32>,
    /// Thinking-mode capability labels without payloads.
    pub thinking_mode_labels: Vec<String>,
    /// Cost-usage display label.
    pub cost_usage_label: String,
    /// Current availability state for activation-time gating.
    pub availability: AssistedAiProviderAvailabilityState,
    /// Redaction hints for the matrix.
    pub redaction_hints: Vec<RedactionHint>,
    /// Schema version.
    pub schema_version: u16,
}

impl AssistedAiCapabilityMatrix {
    /// Returns true when the slot carries an explicit, non-empty declaration.
    pub fn has_explicit_declaration(&self) -> bool {
        !self.provider_id.is_empty()
            && !self.provider_label.is_empty()
            && !self.context_length_label.is_empty()
            && !self.cost_usage_label.is_empty()
    }
}
