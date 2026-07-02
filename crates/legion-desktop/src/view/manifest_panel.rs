use legion_protocol::ContextManifestInclusionState;
use legion_ui::ShellProjectionSnapshot;

use super::bounded_join;

pub(crate) fn preview_rows(snapshot: &ShellProjectionSnapshot) -> Vec<String> {
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
        format!(
            "manifest item {}: kind={:?} inclusion={:?} selected={} risk={:?} privacy={:?} egress={:?} labels={}",
            item.item_id,
            item.kind,
            item.inclusion,
            is_selected,
            item.risk_label,
            item.privacy_label,
            item.egress,
            bounded_join(&item.labels)
        )
    }));
    rows
}
