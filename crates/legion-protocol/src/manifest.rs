use serde::{Deserialize, Serialize};

use crate::{
    ContextManifestEgressStatus, ContextManifestItem, ContextManifestPermissionSummary,
    ContextManifestPurpose, ContextManifestRecord, ProposalId, ProposalPrivacyLabel,
    ProposalRiskLabel, RedactionHint, TimestampMillis, WorkspaceId, WorkspaceTrustState,
};

/// Structured source bundle used to assemble a metadata-only context manifest.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContextManifestSources {
    /// Metadata-only file context items.
    pub files: Vec<ContextManifestItem>,
    /// Metadata-only selection items.
    pub selections: Vec<ContextManifestItem>,
    /// Metadata-only symbol items.
    pub symbols: Vec<ContextManifestItem>,
    /// Metadata-only diagnostic items.
    pub diagnostics: Vec<ContextManifestItem>,
    /// Metadata-only terminal excerpt items.
    pub terminal_excerpts: Vec<ContextManifestItem>,
    /// Metadata-only memory items.
    pub memory: Vec<ContextManifestItem>,
    /// Metadata-only rules and policy items.
    pub rules: Vec<ContextManifestItem>,
}

impl ContextManifestSources {
    /// Flatten all source categories into the stable manifest item order.
    pub fn into_items(self) -> Vec<ContextManifestItem> {
        let Self {
            files,
            selections,
            symbols,
            diagnostics,
            terminal_excerpts,
            memory,
            rules,
        } = self;

        files
            .into_iter()
            .chain(selections)
            .chain(symbols)
            .chain(diagnostics)
            .chain(terminal_excerpts)
            .chain(memory)
            .chain(rules)
            .collect()
    }
}

/// Structured DTO used to assemble a metadata-only context manifest.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContextManifestAssembly {
    /// Stable manifest identifier or hash label.
    pub manifest_id: String,
    /// Optional workspace identifier.
    pub workspace_id: Option<WorkspaceId>,
    /// Optional proposal identifier associated with the manifest.
    pub proposal_id: Option<ProposalId>,
    /// Manifest purpose.
    pub purpose: ContextManifestPurpose,
    /// Workspace trust state represented in the manifest.
    pub workspace_trust_state: Option<WorkspaceTrustState>,
    /// Overall privacy label.
    pub privacy_label: ProposalPrivacyLabel,
    /// Overall risk label.
    pub risk_label: ProposalRiskLabel,
    /// Overall egress posture.
    pub egress: ContextManifestEgressStatus,
    /// Structured source bundle.
    pub sources: ContextManifestSources,
    /// Permission summaries represented by this manifest.
    pub permissions: Vec<ContextManifestPermissionSummary>,
    /// Number of omitted items.
    pub omitted_item_count: u32,
    /// True when stale or missing freshness/precondition metadata is visible.
    pub stale_or_missing_metadata_risk_present: bool,
    /// Manifest generation timestamp.
    pub generated_at: TimestampMillis,
    /// Redaction hints that apply to the manifest.
    pub redaction_hints: Vec<RedactionHint>,
    /// Manifest schema version.
    pub schema_version: u16,
}

impl ContextManifestAssembly {
    /// Convert the structured assembly DTO into the flattened record DTO.
    pub fn into_record(self) -> ContextManifestRecord {
        let Self {
            manifest_id,
            workspace_id,
            proposal_id,
            purpose,
            workspace_trust_state,
            privacy_label,
            risk_label,
            egress,
            sources,
            permissions,
            omitted_item_count,
            stale_or_missing_metadata_risk_present,
            generated_at,
            redaction_hints,
            schema_version,
        } = self;

        ContextManifestRecord {
            manifest_id,
            workspace_id,
            proposal_id,
            purpose,
            workspace_trust_state,
            privacy_label,
            risk_label,
            egress,
            items: sources.into_items(),
            permissions,
            omitted_item_count,
            stale_or_missing_metadata_risk_present,
            generated_at,
            redaction_hints,
            schema_version,
        }
    }
}
