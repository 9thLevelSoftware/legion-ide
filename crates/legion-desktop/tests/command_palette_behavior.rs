use std::{
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

use legion_desktop::{
    bridge::DesktopAction,
    workflow::{DesktopEframeApp, DesktopLaunchConfig, DesktopRuntime},
};
use legion_ui::{PaletteMode, SearchScopeProjection};

static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

struct TempWorkspace {
    root: PathBuf,
}

impl TempWorkspace {
    fn new() -> Self {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be after epoch")
            .as_nanos();
        let id = TEMP_COUNTER.fetch_add(1, Ordering::SeqCst);
        let root = std::env::temp_dir().join(format!(
            "legion_desktop_command_palette_{}_{}_{}",
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
        if self.root.starts_with(std::env::temp_dir()) {
            let _ = fs::remove_dir_all(&self.root);
        }
    }
}

fn app() -> (TempWorkspace, DesktopEframeApp) {
    let workspace = TempWorkspace::new();
    let runtime = DesktopRuntime::open(DesktopLaunchConfig::new(
        workspace.path().to_path_buf(),
        None,
    ))
    .expect("desktop runtime should open");
    let app = DesktopEframeApp::new(runtime);
    app.headless_egui_context().enable_accesskit();
    (workspace, app)
}

fn input(events: Vec<egui::Event>) -> egui::RawInput {
    egui::RawInput {
        focused: true,
        screen_rect: Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(1_200.0, 900.0),
        )),
        events,
        ..egui::RawInput::default()
    }
}

fn accesskit_has_label(output: &egui::FullOutput, label: &str) -> bool {
    output
        .platform_output
        .accesskit_update
        .as_ref()
        .is_some_and(|update| {
            update
                .nodes
                .iter()
                .any(|(_id, node)| node.label() == Some(label) || node.value() == Some(label))
        })
}

fn accesskit_text(output: &egui::FullOutput) -> Vec<String> {
    output
        .platform_output
        .accesskit_update
        .as_ref()
        .map(|update| {
            update
                .nodes
                .iter()
                .flat_map(|(_id, node)| [node.label(), node.value()])
                .flatten()
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default()
}

fn press_enter() -> egui::RawInput {
    input(vec![egui::Event::Key {
        key: egui::Key::Enter,
        physical_key: Some(egui::Key::Enter),
        pressed: true,
        repeat: false,
        modifiers: egui::Modifiers::default(),
    }])
}

#[test]
fn command_palette_renders_all_product_groups() {
    let (_workspace, mut app) = app();
    app.handle_action(DesktopAction::OpenPalette {
        mode: PaletteMode::Command,
        query: ">".to_string(),
        scope: SearchScopeProjection::Workspace,
    })
    .expect("command palette should open");

    let output = app.run_headless_input(input(Vec::new()));
    for group in ["Suggested", "Files", "View", "Run", "Git", "Destructive"] {
        assert!(
            accesskit_has_label(&output, group),
            "command palette should render the {group} group"
        );
    }
}

#[test]
fn command_palette_uses_product_copy_instead_of_implementation_terms() {
    let (_workspace, mut app) = app();
    app.handle_action(DesktopAction::OpenPalette {
        mode: PaletteMode::Command,
        query: ">".to_string(),
        scope: SearchScopeProjection::Workspace,
    })
    .expect("command palette should open");

    let output = app.run_headless_input(input(Vec::new()));
    let text = accesskit_text(&output);
    for internal in ["app authority", "projection", "workbench", "foreground"] {
        assert!(
            text.iter().all(|row| !row.contains(internal)),
            "command palette must not expose `{internal}`: {text:?}"
        );
    }
}

#[test]
fn destructive_palette_command_requires_confirmation_before_dispatch() {
    let (_workspace, mut app) = app();
    app.handle_action(DesktopAction::OpenPalette {
        mode: PaletteMode::Command,
        query: ">preferences reset settings".to_string(),
        scope: SearchScopeProjection::Workspace,
    })
    .expect("destructive command should remain searchable");

    let output = app.run_headless_input(press_enter());
    assert!(
        app.runtime_snapshot().palette_projection.open,
        "Enter must not dispatch a destructive command before confirmation"
    );
    assert!(accesskit_has_label(
        &output,
        "Confirm Preferences: Reset Settings"
    ));

    let confirm = output
        .platform_output
        .accesskit_update
        .as_ref()
        .expect("confirmation should expose AccessKit")
        .nodes
        .iter()
        .find_map(|(id, node)| {
            (node.label() == Some("Confirm")
                && node.supports_action(egui::accesskit::Action::Click))
            .then_some(*id)
        })
        .expect("confirmation should expose a Confirm action");
    let _confirmed = app.run_headless_input(input(vec![egui::Event::AccessKitActionRequest(
        egui::accesskit::ActionRequest {
            action: egui::accesskit::Action::Click,
            target_tree: egui::accesskit::TreeId::ROOT,
            target_node: confirm,
            data: None,
        },
    )]));
    assert!(
        !app.runtime_snapshot().palette_projection.open,
        "Confirm should dispatch through the existing palette intent"
    );
}

#[test]
fn search_palette_shell_explains_empty_query_and_keyboard_controls() {
    let (_workspace, mut app) = app();
    app.handle_action(DesktopAction::OpenPalette {
        mode: PaletteMode::Search,
        query: "/".to_string(),
        scope: SearchScopeProjection::ActiveFile,
    })
    .expect("search palette should open");

    let output = app.run_headless_input(input(Vec::new()));
    for copy in [
        "Type a search term to find text in the active file.",
        "Enter run search · ↑↓ choose · Esc close",
    ] {
        assert!(
            accesskit_has_label(&output, copy),
            "empty search palette should expose `{copy}`"
        );
    }
    assert_eq!(
        app.runtime_snapshot().palette_projection.selected_index,
        0,
        "search keeps its disabled run row app-owned while the renderer presents an empty state"
    );
}

#[test]
fn search_palette_shell_presents_a_runnable_result() {
    let (_workspace, mut app) = app();
    app.handle_action(DesktopAction::OpenPalette {
        mode: PaletteMode::Search,
        query: "/needle".to_string(),
        scope: SearchScopeProjection::Workspace,
    })
    .expect("search palette should open");

    let output = app.run_headless_input(input(Vec::new()));
    assert!(accesskit_has_label(
        &output,
        "Search workspace for \"needle\""
    ));
    assert!(accesskit_has_label(
        &output,
        "Enter run search · ↑↓ choose · Esc close"
    ));
    assert!(!accesskit_has_label(
        &output,
        "Type a search term to find text in the active file."
    ));
    assert!(
        accesskit_text(&output)
            .iter()
            .all(|row| !row.contains("lexical")),
        "Search should use user-recognizable terms"
    );
}
