use std::{
    fs,
    path::Path,
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

use legion_desktop::{
    platform::{
        DesktopPlatformAdapterChecks, NativePlatformObservation, build_platform_smoke_snapshot,
    },
    workflow::{DesktopEframeApp, DesktopLaunchConfig, DesktopRuntime},
};
use legion_protocol::{
    BufferId, CanonicalPath, FileId, ProtocolTextRange, TextCoordinate, TimestampMillis,
    WorkbenchAccessibilityProfile,
};
use legion_ui::ui::{
    DailyEditingProjection, EditorTabProjection, EditorTabsProjection, SearchScopeProjection,
    SearchStatusKindProjection,
};
use legion_ui::{
    ActiveBufferProjection, ActiveBufferProjectionState, ExplorerNodeProjection,
    ExplorerProjection, ExplorerSelectionProjection, SearchProjection, SearchResultProjection,
    SearchStatusProjection, Shell, StatusMessageProjection, StatusSeverity,
};

static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

struct TempWorkspace {
    root: std::path::PathBuf,
}

impl TempWorkspace {
    fn new() -> Self {
        let temp_root = std::env::temp_dir();
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be after epoch")
            .as_nanos();
        let id = TEMP_COUNTER.fetch_add(1, Ordering::SeqCst);
        let root = temp_root.join(format!(
            "legion_desktop_accessibility_{}_{}_{}",
            std::process::id(),
            nanos,
            id
        ));
        fs::create_dir(&root).expect("temp workspace should be created");
        Self { root }
    }

    fn path(&self) -> &Path {
        &self.root
    }
}

impl Drop for TempWorkspace {
    fn drop(&mut self) {
        let temp_root = std::env::temp_dir();
        let file_name = self.root.file_name().and_then(|name| name.to_str());
        if self.root.starts_with(&temp_root)
            && file_name.is_some_and(|name| name.starts_with("legion_desktop_accessibility_"))
        {
            let _ = fs::remove_dir_all(&self.root);
        }
    }
}

fn coord(line: u32, character: u32, byte_offset: u64) -> TextCoordinate {
    TextCoordinate {
        line,
        character,
        byte_offset: Some(byte_offset),
        utf16_offset: Some(byte_offset),
    }
}

fn range(start: u64, end: u64) -> ProtocolTextRange {
    ProtocolTextRange {
        start: coord(0, start as u32, start),
        end: coord(0, end as u32, end),
    }
}

fn open_runtime(root: &Path) -> DesktopRuntime {
    DesktopRuntime::open(DesktopLaunchConfig::new(root.to_path_buf(), None))
        .expect("desktop runtime should open workspace")
}

#[test]
fn accessibility_profile_round_trips_high_contrast_and_reduced_motion_flags() {
    let profile = WorkbenchAccessibilityProfile {
        high_contrast: true,
        screen_reader_projection: true,
        reduce_motion: true,
        ime_diagnostics_enabled: false,
        schema_version: 1,
    };

    let encoded = serde_json::to_value(&profile).expect("profile should serialize");
    assert_eq!(encoded["high_contrast"], true);
    assert_eq!(encoded["screen_reader_projection"], true);
    assert_eq!(encoded["reduce_motion"], true);
    assert_eq!(encoded["ime_diagnostics_enabled"], false);

    let decoded: WorkbenchAccessibilityProfile =
        serde_json::from_value(encoded).expect("profile should deserialize");
    assert_eq!(decoded, profile);
}

#[test]
fn keyboard_only_operation_opens_the_command_palette() {
    let workspace = TempWorkspace::new();
    let runtime = open_runtime(workspace.path());
    let mut app = DesktopEframeApp::new(runtime);

    assert!(!app.runtime_snapshot().palette_projection.open);

    let raw_input = egui::RawInput {
        focused: true,
        modifiers: egui::Modifiers {
            command: true,
            ..egui::Modifiers::default()
        },
        events: vec![egui::Event::Key {
            key: egui::Key::P,
            physical_key: Some(egui::Key::P),
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers {
                command: true,
                ..egui::Modifiers::default()
            },
        }],
        ..egui::RawInput::default()
    };

    let _ = app.run_headless_input(raw_input);

    assert!(
        app.runtime_snapshot().palette_projection.open,
        "synthetic Cmd+P input should open the command palette projection"
    );
}

#[test]
fn reduced_motion_is_preserved_through_the_settings_projection() {
    let mut snapshot = Shell::empty("Reduced motion").projection_snapshot();
    snapshot.settings_projection.editor.smooth_scrolling_enabled = false;

    let model = legion_desktop::view::DesktopProjectionViewModel::from_snapshot(&snapshot);

    assert!(
        !model.settings.smooth_scrolling_enabled,
        "reduced motion should disable the smooth-scrolling setting in the projected settings view"
    );
}

#[test]
fn focus_order_follows_the_projected_accessibility_node_sequence() {
    let mut snapshot = Shell::empty("Focus order").projection_snapshot();
    snapshot.explorer_projection = ExplorerProjection {
        nodes: vec![
            ExplorerNodeProjection {
                file_id: FileId(1),
                canonical_path: CanonicalPath("Cargo.toml".to_string()),
                name: "Cargo.toml".to_string(),
                children: vec![FileId(2)],
            },
            ExplorerNodeProjection {
                file_id: FileId(2),
                canonical_path: CanonicalPath("src/lib.rs".to_string()),
                name: "lib.rs".to_string(),
                children: Vec::new(),
            },
        ],
        selection: Some(ExplorerSelectionProjection { file_id: FileId(1) }),
    };
    snapshot.active_buffer_projection = ActiveBufferProjection {
        workspace_id: None,
        buffer_id: Some(BufferId(7)),
        file_id: Some(FileId(1)),
        file_path: Some(CanonicalPath("Cargo.toml".to_string())),
        viewport: None,
        state: ActiveBufferProjectionState::Full,
        small_buffer_preview: None,
        degraded: false,
        dirty: false,
    };
    snapshot.daily_editing_projection = DailyEditingProjection {
        tabs: EditorTabsProjection {
            tabs: vec![EditorTabProjection {
                buffer_id: BufferId(7),
                file_id: Some(FileId(1)),
                file_path: Some(CanonicalPath("Cargo.toml".to_string())),
                title: "Cargo.toml".to_string(),
                active: true,
                dirty: false,
                pinned: false,
                preview: false,
            }],
            active_buffer_id: Some(BufferId(7)),
        },
        close_dirty_prompt: None,
        viewport_states: Vec::new(),
        session_record: None,
    };
    snapshot.status_messages = vec![StatusMessageProjection {
        severity: StatusSeverity::Info,
        message: "Status live region".to_string(),
    }];
    snapshot.search_projection = SearchProjection {
        query_id: Some("search:test".to_string()),
        scope: SearchScopeProjection::ActiveFile,
        query_label: "search:test".to_string(),
        status: SearchStatusProjection {
            kind: SearchStatusKindProjection::Completed,
            message: "1 result found".to_string(),
        },
        results: vec![SearchResultProjection {
            query_id: "search:test".to_string(),
            scope: SearchScopeProjection::ActiveFile,
            workspace_id: None,
            buffer_id: None,
            file_id: Some(FileId(1)),
            file_path: Some(CanonicalPath("Cargo.toml".to_string())),
            line_number: 12,
            range: range(0, 1),
            snippet: "match".to_string(),
            snippet_truncated: false,
        }],
        result_limit: 1,
        omitted_result_count: 0,
        omitted_file_count: 0,
        diagnostics: Vec::new(),
        generated_at: TimestampMillis(1),
        schema_version: 1,
    };

    let smoke = build_platform_smoke_snapshot(
        &snapshot,
        DesktopPlatformAdapterChecks::default(),
        NativePlatformObservation::default(),
    );

    let roles = smoke
        .accessibility_nodes
        .iter()
        .map(|node| node.role.as_str())
        .collect::<Vec<_>>();

    assert_eq!(
        roles,
        ["window", "explorer", "editor", "tabs", "status", "search"]
    );
    assert_eq!(smoke.accessibility_projection_node_count, 6);
}

#[test]
fn live_regions_surface_status_message_counts_in_the_accessibility_projection() {
    let mut snapshot = Shell::empty("Live regions").projection_snapshot();
    snapshot.status_messages = vec![
        StatusMessageProjection {
            severity: StatusSeverity::Info,
            message: "First announcement".to_string(),
        },
        StatusMessageProjection {
            severity: StatusSeverity::Warning,
            message: "Second announcement".to_string(),
        },
    ];

    let smoke = build_platform_smoke_snapshot(
        &snapshot,
        DesktopPlatformAdapterChecks::default(),
        NativePlatformObservation::default(),
    );

    let status_node = smoke
        .accessibility_nodes
        .iter()
        .find(|node| node.role == "status")
        .expect("status live region should be projected");

    assert_eq!(status_node.label, "2 status messages");
    assert!(
        smoke
            .accessibility_tree_smoke
            .contains("metadata-only projection accessibility nodes 2; OS tree not observed")
    );
}
