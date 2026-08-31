use std::{
    fs,
    path::Path,
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

use legion_desktop::{
    platform::{
        DesktopPlatformAdapterChecks, NativePlatformObservation, WindowsUiaProbeObservation,
        build_platform_smoke_snapshot, committed_windows_uia_probe_script,
        parse_windows_uia_probe_output, probe_windows_uia_tree,
    },
    view::ProjectionView,
    workflow::{DesktopEframeApp, DesktopLaunchConfig, DesktopRuntime},
};
use legion_protocol::{
    BufferId, CanonicalPath, FileId, ProtocolTextRange, TerminalSessionId, TextCoordinate,
    TimestampMillis, WorkbenchAccessibilityProfile,
};
use legion_ui::ui::{
    DailyEditingProjection, EditorTabProjection, EditorTabsProjection, SearchScopeProjection,
    SearchStatusKindProjection,
};
use legion_ui::{
    ActiveBufferProjection, ActiveBufferProjectionState, ExplorerNodeProjection,
    ExplorerProjection, ExplorerSelectionProjection, SearchProjection, SearchResultProjection,
    SearchStatusProjection, Shell, StatusMessageProjection, StatusSeverity,
    ThemePreferenceProjection,
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

fn render_projection_input(
    ctx: &egui::Context,
    view: &mut ProjectionView,
    snapshot: &legion_ui::ShellProjectionSnapshot,
    size: egui::Vec2,
    events: Vec<egui::Event>,
    enabled: bool,
) -> egui::FullOutput {
    ctx.run_ui(desktop_raw_input(size, events), |ui| {
        ui.add_enabled_ui(enabled, |ui| {
            let _ = view.render(ui, snapshot);
        });
    })
}

fn accessible_button_bounds(output: &egui::FullOutput, label: &str) -> egui::Rect {
    let bounds = output
        .platform_output
        .accesskit_update
        .as_ref()
        .expect("rendered controls should expose AccessKit")
        .nodes
        .iter()
        .find_map(|(_id, node)| {
            (node.label() == Some(label) && node.role() == egui::accesskit::Role::Button)
                .then(|| node.bounds())
                .flatten()
        })
        .unwrap_or_else(|| panic!("rendered control `{label}` should have semantic bounds"));
    egui::Rect::from_min_max(
        egui::pos2(bounds.x0 as f32, bounds.y0 as f32),
        egui::pos2(bounds.x1 as f32, bounds.y1 as f32),
    )
}

fn assert_minimum_interactive_target(output: &egui::FullOutput, label: &str) {
    let bounds = accessible_button_bounds(output, label);
    assert!(
        bounds.width() >= 28.0 && bounds.height() >= 28.0,
        "interactive control `{label}` must be at least 28x28 logical pixels; bounds={bounds:?}"
    );
}

fn assert_all_click_targets_meet_minimum(output: &egui::FullOutput, layout: &str) {
    let update = output
        .platform_output
        .accesskit_update
        .as_ref()
        .expect("interactive controls should expose AccessKit");
    let mut checked = 0_usize;
    for (_id, node) in &update.nodes {
        let role = node.role();
        let is_control = matches!(
            role,
            egui::accesskit::Role::Button
                | egui::accesskit::Role::DefaultButton
                | egui::accesskit::Role::CheckBox
                | egui::accesskit::Role::RadioButton
                | egui::accesskit::Role::Switch
                | egui::accesskit::Role::Tab
                | egui::accesskit::Role::TreeItem
                | egui::accesskit::Role::ListBoxOption
                | egui::accesskit::Role::MenuItem
                | egui::accesskit::Role::MenuItemCheckBox
                | egui::accesskit::Role::MenuItemRadio
                | egui::accesskit::Role::DisclosureTriangle
                | egui::accesskit::Role::ComboBox
                | egui::accesskit::Role::EditableComboBox
                | egui::accesskit::Role::TextInput
                | egui::accesskit::Role::MultilineTextInput
                | egui::accesskit::Role::SearchInput
                | egui::accesskit::Role::PasswordInput
                | egui::accesskit::Role::Link
        );
        if !is_control || !node.supports_action(egui::accesskit::Action::Click) {
            continue;
        }
        checked += 1;
        let label = node.label().unwrap_or("<unnamed>");
        let description = node.description().unwrap_or("<none>");
        let bounds = node
            .bounds()
            .unwrap_or_else(|| panic!("{layout} clickable control `{label}` must expose bounds"));
        assert!(
            bounds.x1 - bounds.x0 >= 28.0 && bounds.y1 - bounds.y0 >= 28.0,
            "{layout} clickable control `{label}` ({role:?}, description={description}) must be at least 28x28 logical pixels; bounds={bounds:?}"
        );
    }
    assert!(checked > 0, "{layout} frame must expose clickable controls");
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ControlPaintSignature {
    rectangles: Vec<([u8; 4], [u8; 4], u32)>,
}

fn control_paint_signature(output: &egui::FullOutput, bounds: egui::Rect) -> ControlPaintSignature {
    fn collect(
        shape: &egui::Shape,
        bounds: egui::Rect,
        rectangles: &mut Vec<([u8; 4], [u8; 4], u32)>,
    ) {
        match shape {
            egui::Shape::Rect(rect)
                if rect.rect.center().distance(bounds.center()) <= 2.0
                    && (rect.rect.width() - bounds.width()).abs() <= 8.0
                    && (rect.rect.height() - bounds.height()).abs() <= 8.0 =>
            {
                rectangles.push((
                    rect.fill.to_array(),
                    rect.stroke.color.to_array(),
                    rect.stroke.width.to_bits(),
                ));
            }
            egui::Shape::Vec(shapes) => {
                for shape in shapes {
                    collect(shape, bounds, rectangles);
                }
            }
            _ => {}
        }
    }

    let mut rectangles = Vec::new();
    for clipped in &output.shapes {
        collect(&clipped.shape, bounds, &mut rectangles);
    }
    assert!(
        !rectangles.is_empty(),
        "control at {bounds:?} should paint at least one frame rectangle"
    );
    rectangles.sort_unstable();
    ControlPaintSignature { rectangles }
}

fn rendered_mode_control_signatures(
    preference: ThemePreferenceProjection,
) -> Vec<(&'static str, ControlPaintSignature)> {
    let size = egui::vec2(1_440.0, 900.0);
    let snapshot_for = |mode| {
        let mut snapshot = Shell::empty("Control paint states").projection_snapshot();
        snapshot.product_mode = mode;
        snapshot.settings_projection.theme_preference = preference;
        snapshot
    };
    let pointer_events = |pos, pressed| {
        vec![
            egui::Event::PointerMoved(pos),
            egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed,
                modifiers: egui::Modifiers::default(),
            },
        ]
    };

    let mut signatures = Vec::new();

    let snapshot = snapshot_for(legion_ui::DockMode::Manual);
    let ctx = egui::Context::default();
    ctx.enable_accesskit();
    let mut view = ProjectionView::new();
    let standard = render_projection_input(&ctx, &mut view, &snapshot, size, Vec::new(), true);
    let standard_bounds = accessible_button_bounds(&standard, "Assist");
    signatures.push((
        "standard",
        control_paint_signature(&standard, standard_bounds),
    ));

    let selected_snapshot = snapshot_for(legion_ui::DockMode::Assist);
    let ctx = egui::Context::default();
    ctx.enable_accesskit();
    let mut view = ProjectionView::new();
    let selected =
        render_projection_input(&ctx, &mut view, &selected_snapshot, size, Vec::new(), true);
    signatures.push((
        "selected",
        control_paint_signature(&selected, accessible_button_bounds(&selected, "Assist")),
    ));

    let ctx = egui::Context::default();
    ctx.enable_accesskit();
    let mut view = ProjectionView::new();
    let primed = render_projection_input(&ctx, &mut view, &snapshot, size, Vec::new(), true);
    let assist_id = primed
        .platform_output
        .accesskit_update
        .as_ref()
        .expect("mode switch should expose AccessKit")
        .nodes
        .iter()
        .find_map(|(id, node)| (node.label() == Some("Assist")).then_some(*id))
        .expect("Assist should have an AccessKit id");
    let _focused = render_projection_input(
        &ctx,
        &mut view,
        &snapshot,
        size,
        vec![egui::Event::AccessKitActionRequest(
            egui::accesskit::ActionRequest {
                action: egui::accesskit::Action::Focus,
                target_tree: egui::accesskit::TreeId::ROOT,
                target_node: assist_id,
                data: None,
            },
        )],
        true,
    );
    let focused = render_projection_input(&ctx, &mut view, &snapshot, size, Vec::new(), true);
    signatures.push((
        "keyboard-focused",
        control_paint_signature(&focused, accessible_button_bounds(&focused, "Assist")),
    ));

    let ctx = egui::Context::default();
    ctx.enable_accesskit();
    let mut view = ProjectionView::new();
    let primed = render_projection_input(&ctx, &mut view, &snapshot, size, Vec::new(), true);
    let pos = accessible_button_bounds(&primed, "Assist").center();
    let _hover_started = render_projection_input(
        &ctx,
        &mut view,
        &snapshot,
        size,
        vec![egui::Event::PointerMoved(pos)],
        true,
    );
    let hovered = render_projection_input(
        &ctx,
        &mut view,
        &snapshot,
        size,
        vec![egui::Event::PointerMoved(pos)],
        true,
    );
    signatures.push((
        "hovered",
        control_paint_signature(&hovered, accessible_button_bounds(&hovered, "Assist")),
    ));

    let _press_started = render_projection_input(
        &ctx,
        &mut view,
        &snapshot,
        size,
        pointer_events(pos, true),
        true,
    );
    let pressed = render_projection_input(
        &ctx,
        &mut view,
        &snapshot,
        size,
        vec![egui::Event::PointerMoved(pos)],
        true,
    );
    signatures.push((
        "pressed",
        control_paint_signature(&pressed, accessible_button_bounds(&pressed, "Assist")),
    ));

    let ctx = egui::Context::default();
    ctx.enable_accesskit();
    let mut view = ProjectionView::new();
    let disabled = render_projection_input(&ctx, &mut view, &snapshot, size, Vec::new(), false);
    signatures.push((
        "disabled",
        control_paint_signature(&disabled, accessible_button_bounds(&disabled, "Assist")),
    ));

    signatures
}

fn accesskit_button_id(output: &egui::FullOutput, label: &str) -> egui::accesskit::NodeId {
    output
        .platform_output
        .accesskit_update
        .as_ref()
        .expect("rendered controls should expose AccessKit")
        .nodes
        .iter()
        .find_map(|(id, node)| {
            (node.label() == Some(label) && node.role() == egui::accesskit::Role::Button)
                .then_some(*id)
        })
        .unwrap_or_else(|| panic!("rendered control `{label}` should have an AccessKit id"))
}

fn focus_control_events(target_node: egui::accesskit::NodeId) -> Vec<egui::Event> {
    vec![egui::Event::AccessKitActionRequest(
        egui::accesskit::ActionRequest {
            action: egui::accesskit::Action::Focus,
            target_tree: egui::accesskit::TreeId::ROOT,
            target_node,
            data: None,
        },
    )]
}

fn opened_mode_confirmation(
    preference: ThemePreferenceProjection,
) -> (
    egui::Context,
    ProjectionView,
    legion_ui::ShellProjectionSnapshot,
    egui::FullOutput,
) {
    let size = egui::vec2(1_440.0, 900.0);
    let mut snapshot = Shell::empty("Confirmation paint states").projection_snapshot();
    snapshot.settings_projection.theme_preference = preference;
    let ctx = egui::Context::default();
    ctx.enable_accesskit();
    let mut view = ProjectionView::new();
    let primed = render_projection(&ctx, &mut view, &snapshot, size);
    let opened = click_projection_control(&ctx, &mut view, &snapshot, &primed, "Delegate", size);
    (ctx, view, snapshot, opened)
}

fn rendered_confirmation_control_signatures(
    preference: ThemePreferenceProjection,
) -> Vec<(&'static str, ControlPaintSignature)> {
    let size = egui::vec2(1_440.0, 900.0);
    let pointer_events = |pos, pressed| {
        vec![
            egui::Event::PointerMoved(pos),
            egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed,
                modifiers: egui::Modifiers::default(),
            },
        ]
    };
    let mut signatures = Vec::new();

    let (ctx, mut view, snapshot, opened) = opened_mode_confirmation(preference);
    let cancel_id = accesskit_button_id(&opened, "Cancel");
    let _cancel_focused = render_projection_input(
        &ctx,
        &mut view,
        &snapshot,
        size,
        focus_control_events(cancel_id),
        true,
    );
    let standard = render_projection_input(&ctx, &mut view, &snapshot, size, Vec::new(), true);
    signatures.push((
        "standard",
        control_paint_signature(&standard, accessible_button_bounds(&standard, "Confirm")),
    ));

    let confirm_id = accesskit_button_id(&standard, "Confirm");
    let _confirm_focused = render_projection_input(
        &ctx,
        &mut view,
        &snapshot,
        size,
        focus_control_events(confirm_id),
        true,
    );
    let focused = render_projection_input(&ctx, &mut view, &snapshot, size, Vec::new(), true);
    signatures.push((
        "keyboard-focused",
        control_paint_signature(&focused, accessible_button_bounds(&focused, "Confirm")),
    ));

    let cancel_id = accesskit_button_id(&focused, "Cancel");
    let _cancel_focused = render_projection_input(
        &ctx,
        &mut view,
        &snapshot,
        size,
        focus_control_events(cancel_id),
        true,
    );
    let unfocused = render_projection_input(&ctx, &mut view, &snapshot, size, Vec::new(), true);
    let pos = accessible_button_bounds(&unfocused, "Confirm").center();
    let _hover_started = render_projection_input(
        &ctx,
        &mut view,
        &snapshot,
        size,
        vec![egui::Event::PointerMoved(pos)],
        true,
    );
    let hovered = render_projection_input(
        &ctx,
        &mut view,
        &snapshot,
        size,
        vec![egui::Event::PointerMoved(pos)],
        true,
    );
    signatures.push((
        "hovered",
        control_paint_signature(&hovered, accessible_button_bounds(&hovered, "Confirm")),
    ));

    let _press_started = render_projection_input(
        &ctx,
        &mut view,
        &snapshot,
        size,
        pointer_events(pos, true),
        true,
    );
    let pressed = render_projection_input(
        &ctx,
        &mut view,
        &snapshot,
        size,
        vec![egui::Event::PointerMoved(pos)],
        true,
    );
    signatures.push((
        "pressed",
        control_paint_signature(&pressed, accessible_button_bounds(&pressed, "Confirm")),
    ));

    let (ctx, mut view, snapshot, opened) = opened_mode_confirmation(preference);
    let cancel_id = accesskit_button_id(&opened, "Cancel");
    let _cancel_focused = render_projection_input(
        &ctx,
        &mut view,
        &snapshot,
        size,
        focus_control_events(cancel_id),
        true,
    );
    let _disabled_started =
        render_projection_input(&ctx, &mut view, &snapshot, size, Vec::new(), false);
    let disabled = render_projection_input(&ctx, &mut view, &snapshot, size, Vec::new(), false);
    assert!(
        disabled
            .platform_output
            .accesskit_update
            .as_ref()
            .expect("disabled modal should expose AccessKit")
            .nodes
            .iter()
            .any(|(_id, node)| node.label() == Some("Confirm") && node.is_disabled()),
        "a disabled host must project Confirm as disabled"
    );
    signatures.push((
        "disabled",
        control_paint_signature(&disabled, accessible_button_bounds(&disabled, "Confirm")),
    ));

    signatures
}

#[test]
fn rendered_mode_controls_paint_distinct_interaction_states_in_both_themes() {
    for preference in [
        ThemePreferenceProjection::Dark,
        ThemePreferenceProjection::Light,
    ] {
        let signatures = rendered_mode_control_signatures(preference);
        for (index, (state, signature)) in signatures.iter().enumerate() {
            for (other_state, other_signature) in &signatures[index + 1..] {
                assert_ne!(
                    signature, other_signature,
                    "{preference:?} rendered mode control must distinguish {state} from {other_state}: {signature:?}"
                );
            }
        }
    }
}

#[test]
fn rendered_confirmation_controls_paint_distinct_interaction_states_in_both_themes() {
    for preference in [
        ThemePreferenceProjection::Dark,
        ThemePreferenceProjection::Light,
    ] {
        let signatures = rendered_confirmation_control_signatures(preference);
        for (index, (state, signature)) in signatures.iter().enumerate() {
            for (other_state, other_signature) in &signatures[index + 1..] {
                assert_ne!(
                    signature, other_signature,
                    "{preference:?} rendered Confirm control must distinguish {state} from {other_state}: {signature:?}"
                );
            }
        }
    }
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
    let target = primed
        .platform_output
        .accesskit_update
        .as_ref()
        .expect("projection should expose AccessKit")
        .nodes
        .iter()
        .find_map(|(id, node)| {
            (node.label() == Some(label) && node.supports_action(egui::accesskit::Action::Click))
                .then_some(*id)
        })
        .unwrap_or_else(|| panic!("{label} should be a clickable accessible control"));
    ctx.run_ui(
        desktop_raw_input(
            size,
            vec![egui::Event::AccessKitActionRequest(
                egui::accesskit::ActionRequest {
                    action: egui::accesskit::Action::Click,
                    target_tree: egui::accesskit::TreeId::ROOT,
                    target_node: target,
                    data: None,
                },
            )],
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
fn representative_interactive_controls_meet_28px_targets_in_standard_and_compact_layouts() {
    let mut snapshot = Shell::empty("Accessible target sizes").projection_snapshot();
    snapshot.daily_editing_projection.tabs = EditorTabsProjection {
        tabs: vec![EditorTabProjection {
            buffer_id: BufferId(7),
            file_id: Some(FileId(1)),
            file_path: Some(CanonicalPath("src/lib.rs".to_string())),
            title: "lib.rs".to_string(),
            active: true,
            dirty: false,
            pinned: false,
            preview: false,
        }],
        active_buffer_id: Some(BufferId(7)),
    };
    snapshot.terminal_panel_projection.active_session_id = Some(TerminalSessionId(9));

    let standard_ctx = egui::Context::default();
    standard_ctx.enable_accesskit();
    let mut standard_view = ProjectionView::new();
    let standard = render_projection(
        &standard_ctx,
        &mut standard_view,
        &snapshot,
        egui::vec2(1_440.0, 900.0),
    );
    for label in [
        "Manual",
        "Command",
        "Explorer",
        "Close lib.rs",
        "Send",
        "Poll",
        "Kill",
        "Close",
    ] {
        assert_minimum_interactive_target(&standard, label);
    }
    assert_all_click_targets_meet_minimum(&standard, "standard");

    let compact_ctx = egui::Context::default();
    compact_ctx.enable_accesskit();
    let mut compact_view = ProjectionView::new();
    let compact = render_projection(
        &compact_ctx,
        &mut compact_view,
        &snapshot,
        egui::vec2(960.0, 720.0),
    );
    for label in [
        "Manual",
        "Command",
        "Explorer drawer",
        "Bottom panel drawer",
    ] {
        assert_minimum_interactive_target(&compact, label);
    }
    assert_all_click_targets_meet_minimum(&compact, "compact");
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
                is_directory: true,
            },
            ExplorerNodeProjection {
                file_id: FileId(2),
                canonical_path: CanonicalPath("src/lib.rs".to_string()),
                name: "lib.rs".to_string(),
                children: Vec::new(),
                is_directory: false,
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
        assert!(bounds.x1 - bounds.x0 >= 28.0);
        assert!(bounds.y1 - bounds.y0 >= 28.0);
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
        assert!(bounds.x1 - bounds.x0 >= 28.0);
        assert!(bounds.y1 - bounds.y0 >= 28.0);
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

    let _first = render_projection(&ctx, &mut view, &snapshot, physical_size);
    let full = render_projection(&ctx, &mut view, &snapshot, physical_size);
    let update = full
        .platform_output
        .accesskit_update
        .as_ref()
        .expect("compact mode switch should expose AccessKit");
    let modes = [
        ("Manual", true),
        ("Assist", false),
        ("Delegate", false),
        ("Legion Workflows", false),
    ]
    .map(|(label, expected_selected)| {
        let node = update
            .nodes
            .iter()
            .find_map(|(_id, node)| {
                (node.label() == Some(label)
                    && node.role() == egui::accesskit::Role::Button
                    && node.bounds().is_some_and(|bounds| bounds.y1 <= 42.0))
                .then_some(node)
            })
            .unwrap_or_else(|| panic!("compact switch must retain full name `{label}`"));
        let bounds = node
            .bounds()
            .expect("compact mode button should have bounds");
        (label, node, bounds, expected_selected)
    });

    for (label, node, bounds, expected_selected) in &modes {
        assert!(bounds.x1 - bounds.x0 >= 28.0);
        assert!(bounds.y1 - bounds.y0 >= 28.0);
        assert!(bounds.x0 >= 0.0 && bounds.x1 <= 480.0);
        assert!(bounds.y0 >= 0.0 && bounds.y1 <= 42.0);
        assert_eq!(
            node.is_selected(),
            Some(*expected_selected),
            "wrong selected state for compact `{label}` mode"
        );
        assert_eq!(
            node.aria_current(),
            expected_selected.then_some(egui::accesskit::AriaCurrent::True),
            "wrong current state for compact `{label}` mode"
        );
    }

    for pair in modes.windows(2) {
        let (left_label, _left_node, left_bounds, _left_selected) = pair[0];
        let (right_label, _right_node, right_bounds, _right_selected) = pair[1];
        assert!(
            left_bounds.x0 < right_bounds.x0,
            "compact canonical order must be left-to-right: `{left_label}` before `{right_label}`"
        );
        assert!(
            left_bounds.x1 <= right_bounds.x0,
            "compact accessible targets `{left_label}` and `{right_label}` must not overlap"
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
    assert!(command_bounds.x1 - command_bounds.x0 >= 28.0);
    assert!(command_bounds.y1 - command_bounds.y0 >= 28.0);

    let painted = top_bar_painted_text(&full);
    for label in ["Manual", "Assist", "Delegate", "Legion Workflows"] {
        assert!(
            painted.iter().any(|text| text == label),
            "ultra-compact switch should paint full canonical label `{label}`; painted={painted:?}"
        );
    }

    let editor = view
        .last_editor_rect()
        .expect("ultra-compact render should retain a real editor allocation")
        .intersect(egui::Rect::from_min_size(egui::Pos2::ZERO, logical_size));
    assert!(
        editor.width() >= 360.0 && editor.height() >= 180.0,
        "200% zoom must collapse secondary panes and preserve a usable editor; editor={editor:?}"
    );

    for label in ["Explorer drawer", "Bottom panel drawer"] {
        let node = update
            .nodes
            .iter()
            .find_map(|(_id, node)| {
                (node.label() == Some(label)
                    && node.role() == egui::accesskit::Role::Button
                    && node.supports_action(egui::accesskit::Action::Click))
                .then_some(node)
            })
            .unwrap_or_else(|| panic!("ultra-compact layout must expose `{label}`"));
        let bounds = node.bounds().expect("drawer control should have bounds");
        assert!(bounds.x1 - bounds.x0 >= 28.0);
        assert!(bounds.y1 - bounds.y0 >= 28.0);
    }

    let _opened = click_projection_control(
        &ctx,
        &mut view,
        &snapshot,
        &full,
        "Explorer drawer",
        physical_size,
    );
    let opened = render_projection(&ctx, &mut view, &snapshot, physical_size);
    let opened_labels = opened
        .platform_output
        .accesskit_update
        .as_ref()
        .map(|update| {
            update
                .nodes
                .iter()
                .filter_map(|(_id, node)| node.label().map(str::to_string))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    assert!(
        opened
            .platform_output
            .accesskit_update
            .as_ref()
            .is_some_and(|update| update.nodes.iter().any(|(_id, node)| {
                node.label() == Some("Source Control")
                    && node.supports_action(egui::accesskit::Action::Click)
            })),
        "the compact Explorer drawer must make activity selections reachable; labels={opened_labels:?}"
    );
    assert!(
        opened
            .platform_output
            .accesskit_update
            .as_ref()
            .is_some_and(|update| update.nodes.iter().any(|(_id, node)| {
                node.label() == Some("Settings")
                    && node.supports_action(egui::accesskit::Action::Click)
            }))
    );
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
    assert_os_tree_status_matches_probe(&smoke.accessibility_tree_smoke, 2);
}

#[test]
fn committed_windows_uia_probe_output_parses_the_captured_walk() {
    let script = committed_windows_uia_probe_script()
        .expect("scripts/a11y-uia-walk.ps1 must be locatable from the crate");
    assert!(
        script.ends_with(std::path::Path::new("scripts/a11y-uia-walk.ps1")),
        "probe path should resolve to the committed script, got {script:?}"
    );

    let evidence = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../plans/evidence/production/PR-UI-001/2026-08-16-windows-uia-tree.txt");
    let stdout = fs::read_to_string(&evidence).expect("committed Windows UIA walk should exist");
    let observation =
        parse_windows_uia_probe_output(&stdout).expect("captured walk printed UIA_WALK_OK");
    assert_eq!(observation.descendant_count, 138);

    assert!(parse_windows_uia_probe_output("PROCESS_NOT_FOUND: legion-desktop").is_none());
    assert!(parse_windows_uia_probe_output("UIA_LOAD_FAILED: missing assemblies").is_none());
    assert!(parse_windows_uia_probe_output("NO_TOPLEVEL_WINDOW_FOR_PROCESS").is_none());
}

#[test]
fn accessibility_tree_status_reports_injected_windows_uia_observation() {
    let mut snapshot = Shell::empty("Windows UIA").projection_snapshot();
    snapshot.status_messages = vec![StatusMessageProjection {
        severity: StatusSeverity::Info,
        message: "Status live region".to_string(),
    }];

    let smoke = build_platform_smoke_snapshot(
        &snapshot,
        DesktopPlatformAdapterChecks::default(),
        NativePlatformObservation {
            os_accessibility_tree: Some(WindowsUiaProbeObservation {
                descendant_count: 138,
            }),
            ..NativePlatformObservation::default()
        },
    );

    assert_eq!(
        smoke.accessibility_tree_smoke,
        "metadata-only projection accessibility nodes 2; Windows UIA observed 138 descendants"
    );
    assert!(!smoke.accessibility_tree_smoke.contains("macOS"));
    assert!(!smoke.accessibility_tree_smoke.contains("Linux"));
}

#[test]
fn accessibility_tree_status_matches_the_live_windows_uia_probe() {
    let mut snapshot = Shell::empty("Live Windows UIA").projection_snapshot();
    snapshot.status_messages = vec![StatusMessageProjection {
        severity: StatusSeverity::Info,
        message: "Status live region".to_string(),
    }];

    let smoke = build_platform_smoke_snapshot(
        &snapshot,
        DesktopPlatformAdapterChecks::default(),
        NativePlatformObservation::default(),
    );

    assert_os_tree_status_matches_probe(&smoke.accessibility_tree_smoke, 2);
}

fn assert_os_tree_status_matches_probe(status: &str, node_count: usize) {
    assert!(
        status.starts_with(&format!(
            "metadata-only projection accessibility nodes {node_count}; "
        )),
        "unexpected accessibility tree status: {status}"
    );
    assert!(
        !status.contains("macOS")
            && !status.contains("Linux")
            && !status.contains("AT-SPI")
            && !status.contains("AXUIElement")
            && !status.contains("VoiceOver")
            && !status.contains("Orca"),
        "must not claim a macOS or Linux probe: {status}"
    );
    if let Some(rest) = status.rsplit_once("Windows UIA observed ") {
        let count = rest
            .1
            .strip_suffix(" descendants")
            .and_then(|value| value.parse::<usize>().ok())
            .unwrap_or_else(|| {
                panic!("Windows UIA status must include a descendant count: {status}")
            });
        let observed = probe_windows_uia_tree()
            .expect("status claimed a Windows UIA walk, so the committed probe must succeed");
        assert_eq!(observed.descendant_count, count);
        assert!(!status.contains("OS tree not observed"));
    } else {
        assert!(
            status.contains("OS tree not observed"),
            "absent Windows UIA walk must stay an honest miss, got {status}"
        );
    }
}

#[test]
fn pr15_accessibility_evidence_keeps_unobserved_platforms_explicit() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("workspace root");
    let probe = root.join("scripts/a11y-platform-probe.sh");
    let evidence = root.join("plans/evidence/accessibility/PR-15-manual-keyboard-path.md");

    let probe_text = fs::read_to_string(probe).expect("PR-15 probe contract");
    assert!(probe_text.contains("observation=unobserved"));
    assert!(probe_text.contains("a11y-uia-walk.ps1"));

    let evidence_text = fs::read_to_string(evidence).expect("PR-15 evidence packet");
    for platform in ["macOS", "Linux"] {
        assert!(evidence_text.contains(&format!("| {platform} |")));
        assert!(evidence_text.contains(&format!(
            "| {platform} | No committed OS-tree probe | Unobserved. |"
        )));
    }
    assert!(evidence_text.contains("Manual keyboard-only path"));
    for route in [
        ":search-workspace <query>",
        ":definition <byte-offset>",
        ":git-stage-hunk <hunk-id>",
        ":term-launch <command>",
        "Git: Commit Staged Changes",
    ] {
        assert!(
            evidence_text.contains(route),
            "evidence should name the available route `{route}`"
        );
    }
    assert!(evidence_text.contains("not a renderer-backed keyboard path"));
    assert!(evidence_text.contains("remains pending"));
    assert!(!evidence_text.contains("Use the published palette/keymap to"));
    assert!(!evidence_text.contains("`Git: Stage Focused Hunk` from its published"));
}
