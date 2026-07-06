use legion_protocol::{
    ByteRange, CanonicalPath, ContextManifestAssembly, ContextManifestEgressStatus,
    ContextManifestInclusionState, ContextManifestItem, ContextManifestItemKind,
    ContextManifestPermissionSummary, ContextManifestPurpose, ContextManifestRecord,
    ContextManifestSources, LspDiagnosticSummary, ProposalId, ProposalPrivacyLabel,
    ProposalRiskLabel, RedactionHint, TimestampMillis, WorkspaceId, WorkspaceTrustState,
};

/// Assemble a metadata-only context manifest record from a structured DTO.
///
/// This helper keeps the AI crate on the structured-DTO path and avoids any
/// freeform prompt serialization for the manifest payload itself.
pub fn assemble_context_manifest(assembly: ContextManifestAssembly) -> ContextManifestRecord {
    assembly.into_record()
}

// ---------------------------------------------------------------------------
// Manifest assembly metadata
// ---------------------------------------------------------------------------

/// Metadata required for manifest assembly, separate from the source items.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManifestMetadata {
    /// Optional workspace identifier.
    pub workspace_id: Option<WorkspaceId>,
    /// Optional proposal identifier.
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
    /// Permission summaries for the manifest.
    pub permissions: Vec<ContextManifestPermissionSummary>,
    /// Manifest generation timestamp.
    pub generated_at: TimestampMillis,
    /// Manifest schema version.
    pub schema_version: u16,
}

// ---------------------------------------------------------------------------
// Minimal source descriptor DTOs for each manifest category
// ---------------------------------------------------------------------------

/// Minimal file source descriptor for context manifest assembly.
#[derive(Debug, Clone)]
pub struct ManifestFileSource {
    /// Canonical path to the file.
    pub path: CanonicalPath,
    /// Workspace owning the file.
    pub workspace_id: WorkspaceId,
}

/// Minimal user-selection source descriptor for context manifest assembly.
#[derive(Debug, Clone)]
pub struct ManifestSelectionSource {
    /// Stable item identifier.
    pub item_id: String,
    /// Optional file path.
    pub path: Option<CanonicalPath>,
    /// Byte ranges of the selection.
    pub ranges: Vec<ByteRange>,
}

/// Minimal symbol source descriptor for context manifest assembly.
#[derive(Debug, Clone)]
pub struct ManifestSymbolSource {
    /// Stable item identifier.
    pub item_id: String,
    /// Optional file path.
    pub path: Option<CanonicalPath>,
    /// Symbol label (metadata-only, no source text).
    pub label: String,
}

/// Minimal terminal excerpt descriptor for context manifest assembly.
#[derive(Debug, Clone)]
pub struct ManifestTerminalExcerpt {
    /// Stable item identifier.
    pub item_id: String,
    /// Terminal session label (metadata-only).
    pub label: String,
    /// Number of output lines represented (metadata-only count).
    pub line_count: u32,
}

/// Minimal memory record descriptor for context manifest assembly.
#[derive(Debug, Clone)]
pub struct ManifestMemoryRecordSource {
    /// Stable item identifier.
    pub item_id: String,
    /// Memory record label (metadata-only).
    pub label: String,
}

/// Minimal rule record descriptor for context manifest assembly.
#[derive(Debug, Clone)]
pub struct ManifestRuleRecordSource {
    /// Stable item identifier.
    pub item_id: String,
    /// Optional rule file path.
    pub path: Option<CanonicalPath>,
}

// ---------------------------------------------------------------------------
// Source collector functions
// ---------------------------------------------------------------------------

/// Build `File` manifest items from canonical paths.
///
/// Each item carries `MetadataOnly` redaction and `LocalOnly` egress; the path
/// is included because policy allows path-level disclosure without content.
pub fn collect_file_context(
    paths: &[CanonicalPath],
    workspace_id: WorkspaceId,
) -> Vec<ContextManifestItem> {
    paths
        .iter()
        .map(|path| ContextManifestItem {
            item_id: format!("file:{}", path.0),
            kind: ContextManifestItemKind::File,
            inclusion: ContextManifestInclusionState::Included,
            workspace_id: Some(workspace_id),
            file_id: None,
            buffer_id: None,
            proposal_id: None,
            target_id: None,
            path: Some(path.clone()),
            ranges: Vec::new(),
            counts: Vec::new(),
            hashes: Vec::new(),
            privacy_scope: None,
            privacy_label: ProposalPrivacyLabel::WorkspaceMetadata,
            risk_label: ProposalRiskLabel::Low,
            egress: ContextManifestEgressStatus::LocalOnly,
            freshness: None,
            preconditions: None,
            labels: Vec::new(),
            redaction_hints: vec![RedactionHint::MetadataOnly],
            schema_version: 1,
        })
        .collect()
}

/// Build `UserSelection` manifest items from selection source descriptors.
pub fn collect_selection_context(
    selections: &[ManifestSelectionSource],
) -> Vec<ContextManifestItem> {
    selections
        .iter()
        .map(|sel| ContextManifestItem {
            item_id: sel.item_id.clone(),
            kind: ContextManifestItemKind::UserSelection,
            inclusion: ContextManifestInclusionState::Included,
            workspace_id: None,
            file_id: None,
            buffer_id: None,
            proposal_id: None,
            target_id: None,
            path: sel.path.clone(),
            ranges: sel.ranges.clone(),
            counts: Vec::new(),
            hashes: Vec::new(),
            privacy_scope: None,
            privacy_label: ProposalPrivacyLabel::WorkspaceMetadata,
            risk_label: ProposalRiskLabel::Low,
            egress: ContextManifestEgressStatus::LocalOnly,
            freshness: None,
            preconditions: None,
            labels: Vec::new(),
            redaction_hints: vec![RedactionHint::MetadataOnly],
            schema_version: 1,
        })
        .collect()
}

/// Build `SemanticRecord` manifest items from symbol source descriptors.
pub fn collect_symbol_context(symbols: &[ManifestSymbolSource]) -> Vec<ContextManifestItem> {
    symbols
        .iter()
        .map(|sym| ContextManifestItem {
            item_id: sym.item_id.clone(),
            kind: ContextManifestItemKind::SemanticRecord,
            inclusion: ContextManifestInclusionState::Included,
            workspace_id: None,
            file_id: None,
            buffer_id: None,
            proposal_id: None,
            target_id: Some(sym.label.clone()),
            path: sym.path.clone(),
            ranges: Vec::new(),
            counts: Vec::new(),
            hashes: Vec::new(),
            privacy_scope: None,
            privacy_label: ProposalPrivacyLabel::WorkspaceMetadata,
            risk_label: ProposalRiskLabel::Low,
            egress: ContextManifestEgressStatus::LocalOnly,
            freshness: None,
            preconditions: None,
            labels: vec![sym.label.clone()],
            redaction_hints: vec![RedactionHint::MetadataOnly],
            schema_version: 1,
        })
        .collect()
}

/// Build `LspDiagnosticSummary` manifest items from LSP diagnostic summary DTOs.
pub fn collect_diagnostic_context(
    diagnostics: &[LspDiagnosticSummary],
) -> Vec<ContextManifestItem> {
    diagnostics
        .iter()
        .map(|diag| ContextManifestItem {
            item_id: format!("diag:ws{}:file{}", diag.workspace_id.0, diag.file_id.0),
            kind: ContextManifestItemKind::LspDiagnosticSummary,
            inclusion: ContextManifestInclusionState::Included,
            workspace_id: Some(diag.workspace_id),
            file_id: Some(diag.file_id),
            buffer_id: None,
            proposal_id: None,
            target_id: None,
            path: None,
            ranges: Vec::new(),
            counts: Vec::new(),
            hashes: diag
                .content_hash
                .as_ref()
                .map(|h| vec![h.clone()])
                .unwrap_or_default(),
            privacy_scope: None,
            privacy_label: ProposalPrivacyLabel::WorkspaceMetadata,
            risk_label: ProposalRiskLabel::Low,
            egress: ContextManifestEgressStatus::LocalOnly,
            freshness: None,
            preconditions: None,
            labels: Vec::new(),
            redaction_hints: vec![RedactionHint::MetadataOnly],
            schema_version: 1,
        })
        .collect()
}

/// Build `TerminalSummary` manifest items from terminal excerpt descriptors.
pub fn collect_terminal_context(excerpts: &[ManifestTerminalExcerpt]) -> Vec<ContextManifestItem> {
    excerpts
        .iter()
        .map(|excerpt| ContextManifestItem {
            item_id: excerpt.item_id.clone(),
            kind: ContextManifestItemKind::TerminalSummary,
            inclusion: ContextManifestInclusionState::Included,
            workspace_id: None,
            file_id: None,
            buffer_id: None,
            proposal_id: None,
            target_id: None,
            path: None,
            ranges: Vec::new(),
            counts: Vec::new(),
            hashes: Vec::new(),
            privacy_scope: None,
            privacy_label: ProposalPrivacyLabel::WorkspaceMetadata,
            risk_label: ProposalRiskLabel::Low,
            egress: ContextManifestEgressStatus::LocalOnly,
            freshness: None,
            preconditions: None,
            labels: vec![excerpt.label.clone()],
            redaction_hints: vec![RedactionHint::MetadataOnly],
            schema_version: 1,
        })
        .collect()
}

/// Build `MemoryRecord` manifest items from memory record source descriptors.
pub fn collect_memory_context(
    memory_items: &[ManifestMemoryRecordSource],
) -> Vec<ContextManifestItem> {
    memory_items
        .iter()
        .map(|mem| ContextManifestItem {
            item_id: mem.item_id.clone(),
            kind: ContextManifestItemKind::MemoryRecord,
            inclusion: ContextManifestInclusionState::Included,
            workspace_id: None,
            file_id: None,
            buffer_id: None,
            proposal_id: None,
            target_id: None,
            path: None,
            ranges: Vec::new(),
            counts: Vec::new(),
            hashes: Vec::new(),
            privacy_scope: None,
            privacy_label: ProposalPrivacyLabel::WorkspaceMetadata,
            risk_label: ProposalRiskLabel::Low,
            egress: ContextManifestEgressStatus::LocalOnly,
            freshness: None,
            preconditions: None,
            labels: vec![mem.label.clone()],
            redaction_hints: vec![RedactionHint::MetadataOnly],
            schema_version: 1,
        })
        .collect()
}

/// Build `Rule` manifest items from rule record source descriptors.
pub fn collect_rules_context(rules: &[ManifestRuleRecordSource]) -> Vec<ContextManifestItem> {
    rules
        .iter()
        .map(|rule| ContextManifestItem {
            item_id: rule.item_id.clone(),
            kind: ContextManifestItemKind::Rule,
            inclusion: ContextManifestInclusionState::Included,
            workspace_id: None,
            file_id: None,
            buffer_id: None,
            proposal_id: None,
            target_id: None,
            path: rule.path.clone(),
            ranges: Vec::new(),
            counts: Vec::new(),
            hashes: Vec::new(),
            privacy_scope: None,
            privacy_label: ProposalPrivacyLabel::WorkspaceMetadata,
            risk_label: ProposalRiskLabel::Low,
            egress: ContextManifestEgressStatus::LocalOnly,
            freshness: None,
            preconditions: None,
            labels: Vec::new(),
            redaction_hints: vec![RedactionHint::MetadataOnly],
            schema_version: 1,
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Full assembly function
// ---------------------------------------------------------------------------

/// Assemble a context manifest from real workspace sources and metadata.
///
/// - Computes `omitted_item_count` from `Excluded` items in the sources.
/// - Sets `stale_or_missing_metadata_risk_present` when any item lacks freshness
///   metadata (indicating the manifest may be based on unverified or stale data).
/// - Generates a deterministic `manifest_id` as an FNV-1a hash of the sorted
///   item IDs so the same logical context always produces the same identifier.
/// - Collects `redaction_hints` from all items, deduplicating them.
///
/// The manifest is a structured DTO only — no raw source bodies flow through
/// this path.
pub fn assemble_context_manifest_from_sources(
    sources: ContextManifestSources,
    metadata: ManifestMetadata,
) -> ContextManifestRecord {
    // Collect all items without consuming sources yet.
    let all_items: Vec<&ContextManifestItem> = sources
        .files
        .iter()
        .chain(&sources.selections)
        .chain(&sources.symbols)
        .chain(&sources.diagnostics)
        .chain(&sources.terminal_excerpts)
        .chain(&sources.memory)
        .chain(&sources.rules)
        .collect();

    // Count excluded items.
    let omitted_item_count = all_items
        .iter()
        .filter(|item| item.inclusion == ContextManifestInclusionState::Excluded)
        .count() as u32;

    // Detect stale or missing freshness or precondition metadata — any item without
    // either field introduces risk that the manifest context is unverified.
    let stale_or_missing_metadata_risk_present = !all_items.is_empty()
        && all_items
            .iter()
            .any(|item| item.freshness.is_none() || item.preconditions.is_none());

    // Generate a deterministic manifest_id from the sorted item IDs.
    let manifest_id = compute_manifest_id(&all_items);

    // Collect redaction hints from all items, keeping the base MetadataOnly hint
    // and deduplicating any additional hints from items.
    let mut redaction_hints = vec![RedactionHint::MetadataOnly];
    for item in &all_items {
        for hint in &item.redaction_hints {
            if !redaction_hints.contains(hint) {
                redaction_hints.push(*hint);
            }
        }
    }

    let assembly = ContextManifestAssembly {
        manifest_id,
        workspace_id: metadata.workspace_id,
        proposal_id: metadata.proposal_id,
        purpose: metadata.purpose,
        workspace_trust_state: metadata.workspace_trust_state,
        privacy_label: metadata.privacy_label,
        risk_label: metadata.risk_label,
        egress: metadata.egress,
        sources,
        permissions: metadata.permissions,
        omitted_item_count,
        stale_or_missing_metadata_risk_present,
        generated_at: metadata.generated_at,
        redaction_hints,
        schema_version: metadata.schema_version,
    };

    assembly.into_record()
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

/// Compute a deterministic manifest identifier using FNV-1a over sorted item IDs.
///
/// FNV-1a is used rather than `DefaultHasher` because the standard hasher is not
/// guaranteed to be stable across Rust versions or compile invocations when
/// randomization is enabled, which would break the determinism guarantee.
fn compute_manifest_id(items: &[&ContextManifestItem]) -> String {
    let mut ids: Vec<&str> = items.iter().map(|item| item.item_id.as_str()).collect();
    ids.sort_unstable();

    // FNV-1a 64-bit parameters.
    const FNV_OFFSET: u64 = 0xcbf29ce484222325;
    const FNV_PRIME: u64 = 0x00000100000001b3;
    let mut hash = FNV_OFFSET;
    for id in ids {
        for byte in id.as_bytes() {
            hash ^= *byte as u64;
            hash = hash.wrapping_mul(FNV_PRIME);
        }
        // Non-zero separator between IDs prevents collisions like "ab"+"c" vs "a"+"bc".
        hash ^= 0xFF;
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    format!("manifest:{hash:016x}")
}
