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
    view::ProjectionView,
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

fn desktop_raw_input(size: egui::Vec2, events: Vec<egui::Event>) -> egui::RawInput {
    egui::RawInput {
        focused: true,
        screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, size)),
        events,
        ..egui::RawInput::default()
    }
}

fn render_projection(
    ctx: &egui::Context,
    view: &mut ProjectionView,
    snapshot: &legion_ui::ShellProjectionSnapshot,
    size: egui::Vec2,
) -> egui::FullOutput {
    ctx.run_ui(desktop_raw_input(size, Vec::new()), |ui| {
        let _ = view.render(ui, snapshot);
    })
}

fn logical_viewport_for_physical_size(physical_size: egui::Vec2, zoom_percent: u16) -> egui::Vec2 {
    physical_size / (f32::from(zoom_percent) / 100.0)
}

fn top_bar_painted_text(output: &egui::FullOutput) -> Vec<String> {
    fn collect(shape: &egui::Shape, texts: &mut Vec<String>) {
        match shape {
            egui::Shape::Text(text) if text.pos.y < 42.0 => {
                texts.push(text.galley.job.text.clone());
            }
            egui::Shape::Vec(shapes) => {
                for shape in shapes {
                    collect(shape, texts);
                }
            }
            _ => {}
        }
    }

    let mut texts = Vec::new();
    for clipped in &output.shapes {
        collect(&clipped.shape, &mut texts);
    }
    texts
}

fn click_projection_control(
    ctx: &egui::Context,
    view: &mut ProjectionView,
    snapshot: &legion_ui::ShellProjectionSnapshot,
    primed: &egui::FullOutput,
    label: &str,
    size: egui::Vec2,
) -> egui::FullOutput {
    let bounds = primed
        .platform_output
        .accesskit_update
        .as_ref()
        .expect("projection should expose AccessKit")
        .nodes
        .iter()
        .find_map(|(_id, node)| {
            (node.label() == Some(label) && node.supports_action(egui::accesskit::Action::Click))
                .then(|| node.bounds())
                .flatten()
        })
        .unwrap_or_else(|| panic!("{label} should be a clickable accessible control"));
    let pos = egui::pos2(
        ((bounds.x0 + bounds.x1) * 0.5) as f32,
        ((bounds.y0 + bounds.y1) * 0.5) as f32,
    );
    let _ = ctx.run_ui(
        desktop_raw_input(
            size,
            vec![
                egui::Event::PointerMoved(pos),
                egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: egui::Modifiers::default(),
                },
            ],
        ),
        |ui| {
            let _ = view.render(ui, snapshot);
        },
    );
    ctx.run_ui(
        desktop_raw_input(
            size,
            vec![
                egui::Event::PointerMoved(pos),
                egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed: false,
                    modifiers: egui::Modifiers::default(),
                },
            ],
        ),
        |ui| {
            let _ = view.render(ui, snapshot);
        },
    )
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
            stale: false,
        }],
        result_limit: 1,
        omitted_result_count: 0,
        omitted_file_count: 0,
        skipped_binary_count: 0,
        case_sensitive: true,
        whole_word: false,
        use_regex: false,
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
fn rendered_mode_switch_and_confirmation_dialog_expose_accessible_semantics() {
    let ctx = egui::Context::default();
    ctx.enable_accesskit();
    let mut view = ProjectionView::new();
    let snapshot = Shell::empty("Accessible modes").projection_snapshot();
    let size = egui::vec2(1_440.0, 900.0);

    let full = render_projection(&ctx, &mut view, &snapshot, size);
    let update = full
        .platform_output
        .accesskit_update
        .as_ref()
        .expect("mode switch should expose AccessKit");
    for (label, selected) in [
        ("Manual", true),
        ("Assist", false),
        ("Delegate", false),
        ("Legion Workflows", false),
    ] {
        let node = update
            .nodes
            .iter()
            .find_map(|(_id, node)| {
                (node.label() == Some(label)
                    && node.role() == egui::accesskit::Role::Button
                    && node.bounds().is_some_and(|bounds| bounds.y1 <= 42.0))
                .then_some(node)
            })
            .unwrap_or_else(|| panic!("{label} should be a top-bar mode button"));
        assert_eq!(
            node.is_selected(),
            Some(selected),
            "wrong current state for {label}"
        );
        assert!(node.supports_action(egui::accesskit::Action::Click));
        assert!(node.supports_action(egui::accesskit::Action::Focus));
        let bounds = node.bounds().expect("mode button should have bounds");
        assert!(bounds.x1 - bounds.x0 >= 24.0);
        assert!(bounds.y1 - bounds.y0 >= 24.0);
    }

    let _modal_full = click_projection_control(&ctx, &mut view, &snapshot, &full, "Delegate", size);
    let modal_full = render_projection(&ctx, &mut view, &snapshot, size);
    let modal_update = modal_full
        .platform_output
        .accesskit_update
        .as_ref()
        .expect("confirmation should expose AccessKit");
    let (_dialog_id, dialog) = modal_update
        .nodes
        .iter()
        .find_map(|(id, node)| {
            (node.role() == egui::accesskit::Role::Dialog).then_some((*id, node))
        })
        .expect("escalation should expose a real Dialog node");
    assert_eq!(dialog.label(), Some("Confirm Delegate mode"));
    assert!(dialog.is_modal());
    let description = dialog
        .description()
        .expect("dialog should expose its explanatory body");
    assert!(description.contains("proposal-mediated"));
    assert!(description.contains("bounded permissions"));
    let mut descendants = dialog.children().to_vec();
    while let Some(descendant_id) = descendants.pop() {
        let descendant = modal_update
            .nodes
            .iter()
            .find_map(|(id, node)| (*id == descendant_id).then_some(node))
            .unwrap_or_else(|| panic!("dialog descendant {descendant_id:?} should be projected"));
        assert_ne!(
            descendant.role(),
            egui::accesskit::Role::CheckBox,
            "presentation dialog must not expose permission-grant checkboxes"
        );
        descendants.extend(descendant.children().iter().copied());
    }
    for label in ["Confirm", "Cancel"] {
        let action = modal_update
            .nodes
            .iter()
            .find_map(|(_id, node)| {
                (node.label() == Some(label)
                    && node.role() == egui::accesskit::Role::Button
                    && node.supports_action(egui::accesskit::Action::Click))
                .then_some(node)
            })
            .unwrap_or_else(|| panic!("dialog should expose {label}"));
        let bounds = action.bounds().expect("dialog action should have bounds");
        assert!(bounds.x1 - bounds.x0 >= 24.0);
        assert!(bounds.y1 - bounds.y0 >= 24.0);
        assert!(action.supports_action(egui::accesskit::Action::Click));
        assert!(action.supports_action(egui::accesskit::Action::Focus));
    }
}

#[test]
fn physical_960_by_720_mode_switch_is_accessible_at_two_hundred_percent_zoom() {
    let ctx = egui::Context::default();
    ctx.enable_accesskit();
    let mut view = ProjectionView::new();
    let mut snapshot = Shell::empty("Compact accessible modes").projection_snapshot();
    snapshot.settings_projection.zoom_percent = 200;
    let physical_size = egui::vec2(960.0, 720.0);
    let logical_size = logical_viewport_for_physical_size(physical_size, 200);
    assert_eq!(logical_size, egui::vec2(480.0, 360.0));

    let _first = render_projection(&ctx, &mut view, &snapshot, logical_size);
    let full = render_projection(&ctx, &mut view, &snapshot, logical_size);
    let update = full
        .platform_output
        .accesskit_update
        .as_ref()
        .expect("compact mode switch should expose AccessKit");
    let mut bounds = ["Manual", "Assist", "Delegate", "Legion Workflows"].map(|label| {
        update
            .nodes
            .iter()
            .find_map(|(_id, node)| {
                (node.label() == Some(label)
                    && node.role() == egui::accesskit::Role::Button
                    && node.bounds().is_some_and(|bounds| bounds.y1 <= 42.0))
                .then(|| node.bounds())
                .flatten()
            })
            .unwrap_or_else(|| panic!("compact switch must retain full name `{label}`"))
    });
    bounds.sort_by(|left, right| left.x0.total_cmp(&right.x0));
    for bounds in &bounds {
        assert!(bounds.x1 - bounds.x0 >= 24.0);
        assert!(bounds.y1 - bounds.y0 >= 24.0);
        assert!(bounds.x0 >= 0.0 && bounds.x1 <= 480.0);
        assert!(bounds.y0 >= 0.0 && bounds.y1 <= 42.0);
    }
    for pair in bounds.windows(2) {
        assert!(
            pair[0].x1 <= pair[1].x0,
            "compact accessible mode targets must not overlap at 200% zoom"
        );
    }

    let command = update
        .nodes
        .iter()
        .find_map(|(_id, node)| {
            (node.label() == Some("Command")
                && node.role() == egui::accesskit::Role::Button
                && node.supports_action(egui::accesskit::Action::Click))
            .then_some(node)
        })
        .expect("Command must remain reachable at physical 960x720 and 200% zoom");
    let command_bounds = command.bounds().expect("Command should be allocated");
    assert!(command_bounds.x0 >= 0.0 && command_bounds.x1 <= 480.0);
    assert!(command_bounds.y0 >= 0.0 && command_bounds.y1 <= 42.0);
    assert!(command_bounds.x1 - command_bounds.x0 >= 24.0);
    assert!(command_bounds.y1 - command_bounds.y0 >= 24.0);

    let painted = top_bar_painted_text(&full);
    for shortcut in ["M", "A", "D", "W"] {
        assert!(
            painted.iter().any(|text| text == shortcut),
            "ultra-compact switch should paint canonical shortcut `{shortcut}`; painted={painted:?}"
        );
    }
}

#[test]
fn physical_960_by_720_at_one_hundred_percent_keeps_full_visible_mode_labels() {
    let ctx = egui::Context::default();
    ctx.enable_accesskit();
    let mut view = ProjectionView::new();
    let snapshot = Shell::empty("Normal compact modes").projection_snapshot();
    let logical_size = logical_viewport_for_physical_size(egui::vec2(960.0, 720.0), 100);

    let _first = render_projection(&ctx, &mut view, &snapshot, logical_size);
    let full = render_projection(&ctx, &mut view, &snapshot, logical_size);
    let painted = top_bar_painted_text(&full);
    for label in ["Manual", "Assist", "Delegate", "Legion Workflows"] {
        assert!(
            painted.iter().any(|text| text == label),
            "normal compact switch should paint full label `{label}`; painted={painted:?}"
        );
    }
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
