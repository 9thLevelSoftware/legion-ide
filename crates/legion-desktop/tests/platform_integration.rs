use legion_desktop::platform::{
    DesktopPlatformAdapterChecks, NativePlatformObservation, build_platform_smoke_snapshot,
};
use legion_protocol::{
    BufferId, CapabilityId, ExtensionCatalogEntry, ExtensionInstallState,
    ExtensionPermissionProjection, ExtensionPermissionState, ExtensionSignatureState,
};
use legion_ui::{ActiveBufferProjection, ActiveBufferProjectionState, Shell};

#[test]
fn platform_snapshot_records_projection_and_adapter_statuses() {
    let mut snapshot = Shell::empty("Platform Smoke").projection_snapshot();
    snapshot.active_buffer_projection = ActiveBufferProjection {
        state: ActiveBufferProjectionState::Full,
        buffer_id: Some(BufferId(7)),
        ..ActiveBufferProjection::empty()
    };

    let platform = build_platform_smoke_snapshot(
        &snapshot,
        DesktopPlatformAdapterChecks::observed(true, true, true),
        NativePlatformObservation {
            focused: Some(true),
            pixels_per_point: Some(2.0),
            os_accessibility_tree: None,
        },
    );

    assert_eq!(platform.menu_smoke, "projection command surface present");
    assert_eq!(
        platform.shortcut_smoke,
        "adapter shortcut targets projected"
    );
    assert_eq!(platform.clipboard_smoke, "adapter-path passed");
    assert_eq!(platform.ime_smoke, "adapter-path passed");
    assert_eq!(platform.file_dialog_smoke, "adapter-path passed");
    assert_eq!(platform.high_dpi_smoke, "os-observed scale 2.000");
    assert!(platform.focus_traversal_smoke.contains("viewport focused"));
    assert!(
        platform
            .accessibility_tree_smoke
            .contains("metadata-only projection accessibility nodes")
    );
    assert!(
        !platform.accessibility_tree_smoke.contains("macOS")
            && !platform.accessibility_tree_smoke.contains("Linux")
            && !platform.accessibility_tree_smoke.contains("AT-SPI")
            && !platform.accessibility_tree_smoke.contains("AXUIElement"),
        "must not claim a macOS or Linux OS-tree probe: {}",
        platform.accessibility_tree_smoke
    );
    assert!(
        platform
            .accessibility_tree_smoke
            .contains("OS tree not observed")
            || platform
                .accessibility_tree_smoke
                .contains("Windows UIA observed"),
        "OS tree status must be the committed Windows probe or an honest miss: {}",
        platform.accessibility_tree_smoke
    );
    assert!(platform.accessibility_projection_node_count >= 2);
}

#[test]
fn platform_snapshot_keeps_accessibility_labels_metadata_only() {
    let mut snapshot = Shell::empty("Metadata").projection_snapshot();
    snapshot.active_buffer_projection = ActiveBufferProjection {
        state: ActiveBufferProjectionState::Full,
        buffer_id: Some(BufferId(11)),
        small_buffer_preview: Some("SECRET_DIRTY_BODY".to_string()),
        ..ActiveBufferProjection::empty()
    };

    let platform = build_platform_smoke_snapshot(
        &snapshot,
        DesktopPlatformAdapterChecks::default(),
        NativePlatformObservation::default(),
    );
    let labels = platform
        .accessibility_nodes
        .iter()
        .map(|node| node.label.as_str())
        .collect::<Vec<_>>()
        .join("\n");

    assert!(labels.contains("Metadata"));
    assert!(labels.contains("active buffer"));
    assert!(!labels.contains("SECRET_DIRTY_BODY"));
    assert!(!labels.contains("small_buffer_preview"));
}

/// The extensions panel is announced, and announced with its pending count.
///
/// It projected no accessibility node at all until this test existed: the one
/// surface in the shell that asks a user to grant capabilities to third-party
/// code was silent to a screen reader. The pending count is asserted because
/// "3 extensions" and "3 extensions, 2 permissions awaiting review" are
/// different situations and only the second one needs the user.
#[test]
fn platform_snapshot_announces_the_extensions_panel_and_its_pending_reviews() {
    let mut snapshot = Shell::empty("Extensions").projection_snapshot();
    snapshot.extension_catalog = vec![ExtensionCatalogEntry {
        manifest_id: "legion.json.grammar".to_string(),
        display_name: "Legion JSON Grammar".to_string(),
        version: "1.0.0".to_string(),
        signature_state: ExtensionSignatureState::VerifiedSigned {
            signer: "legion-first-party".to_string(),
        },
        install_state: ExtensionInstallState::Available,
        permissions: vec![
            ExtensionPermissionProjection {
                ordinal: 1,
                capability: CapabilityId("plugin.command".to_string()),
                title: "Run commands".to_string(),
                reason: "command json.format".to_string(),
                risk_label: "elevated".to_string(),
                state: ExtensionPermissionState::Granted,
            },
            ExtensionPermissionProjection {
                ordinal: 2,
                capability: CapabilityId("plugin.grammar.tree_sitter".to_string()),
                title: "Provide a syntax grammar".to_string(),
                reason: "tree-sitter grammar json".to_string(),
                state: ExtensionPermissionState::Undecided,
                risk_label: "standard".to_string(),
            },
        ],
        blocked_reason: None,
    }];

    let platform = build_platform_smoke_snapshot(
        &snapshot,
        DesktopPlatformAdapterChecks::default(),
        NativePlatformObservation::default(),
    );

    let extensions = platform
        .accessibility_nodes
        .iter()
        .find(|node| node.role == "extensions")
        .expect("a catalog with entries must project an extensions node");
    assert_eq!(
        extensions.label,
        "1 extensions, 1 permissions awaiting review"
    );

    // And an empty catalog stays silent rather than announcing a panel with
    // nothing in it.
    let empty = build_platform_smoke_snapshot(
        &Shell::empty("Extensions").projection_snapshot(),
        DesktopPlatformAdapterChecks::default(),
        NativePlatformObservation::default(),
    );
    assert!(
        empty
            .accessibility_nodes
            .iter()
            .all(|node| node.role != "extensions")
    );
}
