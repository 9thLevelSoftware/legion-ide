use legion_desktop::platform::{
    DesktopPlatformAdapterChecks, NativePlatformObservation, build_platform_smoke_snapshot,
};
use legion_protocol::BufferId;
use legion_ui::{ActiveBufferProjection, Shell};

#[test]
fn platform_snapshot_records_projection_and_adapter_statuses() {
    let mut snapshot = Shell::empty("Platform Smoke").projection_snapshot();
    snapshot.active_buffer_projection = ActiveBufferProjection {
        buffer_id: Some(BufferId(7)),
        ..ActiveBufferProjection::empty()
    };

    let platform = build_platform_smoke_snapshot(
        &snapshot,
        DesktopPlatformAdapterChecks::observed(true, true, true),
        NativePlatformObservation {
            focused: Some(true),
            pixels_per_point: Some(2.0),
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
            .contains("OS tree not observed")
    );
    assert!(platform.accessibility_projection_node_count >= 2);
}

#[test]
fn platform_snapshot_keeps_accessibility_labels_metadata_only() {
    let mut snapshot = Shell::empty("Metadata").projection_snapshot();
    snapshot.active_buffer_projection = ActiveBufferProjection {
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
