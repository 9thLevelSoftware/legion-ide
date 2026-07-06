// P4.F2.T2 — pre-invocation manifest panel with per-item exclusion tests.

use legion_desktop::view::{preview_rows, toggle_manifest_item_inclusion};
use legion_protocol::{
    CanonicalPath, ContextManifestEgressStatus, ContextManifestInclusionState, ContextManifestItem,
    ContextManifestItemKind, ContextManifestProjection, ContextManifestPurpose,
    ContextManifestRecord, ProposalPrivacyLabel, ProposalRiskLabel, RedactionHint, TimestampMillis,
    WorkspaceId,
};
use legion_ui::Shell;

// ---------------------------------------------------------------------------
// Test fixtures
// ---------------------------------------------------------------------------

fn manifest_item(
    item_id: &str,
    kind: ContextManifestItemKind,
    inclusion: ContextManifestInclusionState,
) -> ContextManifestItem {
    ContextManifestItem {
        item_id: item_id.to_string(),
        kind,
        inclusion,
        workspace_id: Some(WorkspaceId(1)),
        file_id: None,
        buffer_id: None,
        proposal_id: None,
        target_id: None,
        path: Some(CanonicalPath(format!("C:/repo/{item_id}"))),
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
    }
}

fn mandatory_item(item_id: &str) -> ContextManifestItem {
    ContextManifestItem {
        labels: vec!["mandatory".to_string()],
        ..manifest_item(
            item_id,
            ContextManifestItemKind::Rule,
            ContextManifestInclusionState::Included,
        )
    }
}

fn egress_item(item_id: &str) -> ContextManifestItem {
    ContextManifestItem {
        egress: ContextManifestEgressStatus::ExternalEgressMetadata,
        ..manifest_item(
            item_id,
            ContextManifestItemKind::File,
            ContextManifestInclusionState::Included,
        )
    }
}

fn test_manifest() -> ContextManifestRecord {
    ContextManifestRecord {
        manifest_id: "test:manifest:1".to_string(),
        workspace_id: Some(WorkspaceId(1)),
        proposal_id: None,
        purpose: ContextManifestPurpose::Explanation,
        workspace_trust_state: None,
        privacy_label: ProposalPrivacyLabel::WorkspaceMetadata,
        risk_label: ProposalRiskLabel::Low,
        egress: ContextManifestEgressStatus::LocalOnly,
        items: vec![
            manifest_item(
                "item-a",
                ContextManifestItemKind::File,
                ContextManifestInclusionState::Included,
            ),
            manifest_item(
                "item-b",
                ContextManifestItemKind::UserSelection,
                ContextManifestInclusionState::Included,
            ),
        ],
        permissions: Vec::new(),
        omitted_item_count: 0,
        stale_or_missing_metadata_risk_present: false,
        generated_at: TimestampMillis(1000),
        redaction_hints: vec![RedactionHint::MetadataOnly],
        schema_version: 1,
    }
}

fn snapshot_with_manifest(manifest: ContextManifestRecord) -> legion_ui::ShellProjectionSnapshot {
    let mut snapshot = Shell::empty("ManifestTest").projection_snapshot();
    snapshot.context_manifest_projection = ContextManifestProjection {
        manifest,
        selected_item_id: None,
        generated_at: TimestampMillis(1000),
        redaction_hints: vec![RedactionHint::MetadataOnly],
        schema_version: 1,
    };
    snapshot
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[test]
fn toggle_item_exclusion_flips_inclusion_state() {
    let mut manifest = test_manifest();
    assert_eq!(
        manifest.items[0].inclusion,
        ContextManifestInclusionState::Included
    );

    // First toggle: Included → Excluded.
    let toggled = toggle_manifest_item_inclusion(&mut manifest, "item-a");
    assert!(toggled, "toggle should succeed for a non-mandatory item");
    assert_eq!(
        manifest.items[0].inclusion,
        ContextManifestInclusionState::Excluded,
        "inclusion should flip to Excluded"
    );

    // Second toggle: Excluded → Included.
    let toggled_back = toggle_manifest_item_inclusion(&mut manifest, "item-a");
    assert!(toggled_back, "second toggle should also succeed");
    assert_eq!(
        manifest.items[0].inclusion,
        ContextManifestInclusionState::Included,
        "inclusion should flip back to Included"
    );
}

#[test]
fn excluded_items_counted_in_omitted() {
    let mut manifest = test_manifest();
    assert_eq!(manifest.omitted_item_count, 0);

    toggle_manifest_item_inclusion(&mut manifest, "item-a");
    assert_eq!(
        manifest.omitted_item_count, 1,
        "omitted_item_count must increase after exclusion"
    );

    toggle_manifest_item_inclusion(&mut manifest, "item-b");
    assert_eq!(
        manifest.omitted_item_count, 2,
        "omitted_item_count must reflect both excluded items"
    );

    // Re-include one item.
    toggle_manifest_item_inclusion(&mut manifest, "item-a");
    assert_eq!(
        manifest.omitted_item_count, 1,
        "omitted_item_count must decrease when item is re-included"
    );
}

#[test]
fn preview_rows_show_all_items_before_invocation() {
    let manifest = test_manifest();
    let snapshot = snapshot_with_manifest(manifest);

    let rows = preview_rows(&snapshot);

    // Header row must be present.
    assert!(
        rows.iter().any(|r| r.contains("before invocation")),
        "rows must include 'before invocation' marker; got: {:?}",
        rows
    );
    // Both items must be visible.
    assert!(
        rows.iter().any(|r| r.contains("item-a")),
        "item-a must be visible in rows; got: {:?}",
        rows
    );
    assert!(
        rows.iter().any(|r| r.contains("item-b")),
        "item-b must be visible in rows; got: {:?}",
        rows
    );
    // Inclusion state must be shown.
    assert!(
        rows.iter().any(|r| r.contains("Included")),
        "rows must show inclusion state; got: {:?}",
        rows
    );
    // can_exclude affordance must be shown.
    assert!(
        rows.iter().any(|r| r.contains("can_exclude")),
        "rows must show can_exclude affordance; got: {:?}",
        rows
    );
}

#[test]
fn egress_items_clearly_marked() {
    let mut manifest = test_manifest();
    // Replace first item with an external-egress item.
    manifest.items[0] = egress_item("file-leaves-machine");

    let snapshot = snapshot_with_manifest(manifest);
    let rows = preview_rows(&snapshot);

    let leaves_row = rows
        .iter()
        .find(|r| r.contains("file-leaves-machine"))
        .expect("egress item must appear in rows");
    assert!(
        leaves_row.contains("LEAVES_MACHINE"),
        "external egress item must have [LEAVES_MACHINE] marker; row: {leaves_row}"
    );
}

#[test]
fn mandatory_items_cannot_be_excluded() {
    let mut manifest = test_manifest();
    manifest.items.push(mandatory_item("mandatory-rule"));

    // Attempt to exclude the mandatory item.
    let result = toggle_manifest_item_inclusion(&mut manifest, "mandatory-rule");
    assert!(!result, "toggle must return false for mandatory items");
    assert_eq!(
        manifest
            .items
            .iter()
            .find(|i| i.item_id == "mandatory-rule")
            .unwrap()
            .inclusion,
        ContextManifestInclusionState::Included,
        "mandatory item inclusion must remain Included after rejected toggle"
    );

    // preview_rows must also include a warning about mandatory items.
    let snapshot = snapshot_with_manifest(manifest);
    let rows = preview_rows(&snapshot);
    assert!(
        rows.iter().any(|r| r.contains("mandatory")),
        "rows must warn about mandatory items; got: {:?}",
        rows
    );
}
