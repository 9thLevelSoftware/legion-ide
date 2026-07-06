use legion_protocol::{
    ContextManifestEgressStatus, ContextManifestInclusionState, ContextManifestItemKind,
    ContextManifestRecord,
};
use legion_ui::ShellProjectionSnapshot;

use super::bounded_join;

/// View-model for a single manifest item's exclusion toggle affordance.
///
/// Rendered before every provider invocation so the user can inspect and
/// exclude items before any egress path is entered.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DesktopManifestItemToggleViewModel {
    /// Stable item identifier.
    pub item_id: String,
    /// Manifest category.
    pub kind: ContextManifestItemKind,
    /// Current inclusion state.
    pub current_inclusion: ContextManifestInclusionState,
    /// Whether this item can be excluded by the user.
    ///
    /// False for items whose `labels` contain `"mandatory"`.
    pub can_exclude: bool,
}

/// Return toggle view-models for every item in the manifest.
pub fn manifest_item_toggle_view_models(
    manifest: &ContextManifestRecord,
) -> Vec<DesktopManifestItemToggleViewModel> {
    manifest
        .items
        .iter()
        .map(|item| DesktopManifestItemToggleViewModel {
            item_id: item.item_id.clone(),
            kind: item.kind,
            current_inclusion: item.inclusion,
            can_exclude: !item.labels.iter().any(|l| l == "mandatory"),
        })
        .collect()
}

/// Toggle the inclusion state of a manifest item between `Included` and `Excluded`.
///
/// Returns `true` when the toggle was applied; `false` when the item is
/// mandatory (has `"mandatory"` in its labels) or is not found.
///
/// After a successful toggle, `manifest.omitted_item_count` is recomputed to
/// reflect the new state.
pub fn toggle_manifest_item_inclusion(manifest: &mut ContextManifestRecord, item_id: &str) -> bool {
    let Some(item) = manifest.items.iter_mut().find(|i| i.item_id == item_id) else {
        return false;
    };

    // Mandatory items cannot be excluded.
    if item.labels.iter().any(|l| l == "mandatory") {
        return false;
    }

    // Flip Included ↔ Excluded; other states are left unchanged.
    item.inclusion = match item.inclusion {
        ContextManifestInclusionState::Included => ContextManifestInclusionState::Excluded,
        ContextManifestInclusionState::Excluded => ContextManifestInclusionState::Included,
        other => other,
    };

    // Recompute omitted_item_count to stay consistent with the new state.
    manifest.omitted_item_count = manifest
        .items
        .iter()
        .filter(|i| i.inclusion == ContextManifestInclusionState::Excluded)
        .count() as u32;

    true
}

/// Produce pre-invocation manifest preview rows.
///
/// Every item is visible with its kind, inclusion state, and exclusion
/// affordance (can_exclude). Items that would leave the machine are clearly
/// marked with `[LEAVES_MACHINE]`. Mandatory items that cannot be excluded
/// trigger a warning row.
pub fn preview_rows(snapshot: &ShellProjectionSnapshot) -> Vec<String> {
    let manifest = &snapshot.context_manifest_projection.manifest;
    if manifest.items.is_empty() {
        return vec!["manifest preview: no context items projected before invocation".to_string()];
    }

    let selected_item_id = snapshot
        .context_manifest_projection
        .selected_item_id
        .as_deref()
        .unwrap_or("<none>");
    let included_count = manifest
        .items
        .iter()
        .filter(|item| item.inclusion == ContextManifestInclusionState::Included)
        .count();
    let excluded_count = manifest
        .items
        .iter()
        .filter(|item| item.inclusion == ContextManifestInclusionState::Excluded)
        .count();

    let mut rows = vec![format!(
        "manifest preview {}: {} items, included={}, excluded={}, selected={}, before invocation",
        manifest.manifest_id,
        manifest.items.len(),
        included_count,
        excluded_count,
        selected_item_id
    )];

    rows.extend(manifest.items.iter().take(12).map(|item| {
        let is_selected = snapshot
            .context_manifest_projection
            .selected_item_id
            .as_deref()
            .is_some_and(|selected| selected == item.item_id);
        let can_exclude = !item.labels.iter().any(|l| l == "mandatory");
        let egress_marker = if matches!(
            item.egress,
            ContextManifestEgressStatus::ExternalEgressMetadata
                | ContextManifestEgressStatus::RemoteApprovalRequired
        ) {
            " [LEAVES_MACHINE]"
        } else {
            ""
        };
        format!(
            "manifest item {}{}: kind={:?} inclusion={:?} selected={} risk={:?} privacy={:?} egress={:?} can_exclude={} labels={}",
            item.item_id,
            egress_marker,
            item.kind,
            item.inclusion,
            is_selected,
            item.risk_label,
            item.privacy_label,
            item.egress,
            can_exclude,
            bounded_join(&item.labels)
        )
    }));

    // Warn when mandatory items are present (they cannot be excluded by the user).
    let mandatory_count = manifest
        .items
        .iter()
        .filter(|item| item.labels.iter().any(|l| l == "mandatory"))
        .count();
    if mandatory_count > 0 {
        rows.push(format!(
            "manifest warning: {} mandatory item(s) cannot be excluded before invocation",
            mandatory_count
        ));
    }

    rows
}
