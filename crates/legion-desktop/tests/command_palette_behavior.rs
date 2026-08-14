use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

use legion_desktop::{
    bridge::DesktopAction,
    workflow::{DesktopEframeApp, DesktopLaunchConfig, DesktopRuntime},
};
use legion_ui::{PaletteMode, SearchScopeProjection, SearchStatusKindProjection};

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

    fn write(&self, relative: &str, content: &str) -> PathBuf {
        let path = self.root.join(relative);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("temp parent should be created");
        }
        fs::write(&path, content).expect("temp file should be written");
        path
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

fn app_with_workspace_files(files: &[(&str, &str)]) -> (TempWorkspace, DesktopEframeApp) {
    let workspace = TempWorkspace::new();
    for (path, content) in files {
        workspace.write(path, content);
    }
    let runtime = DesktopRuntime::open(DesktopLaunchConfig::new(
        workspace.path().to_path_buf(),
        None,
    ))
    .expect("desktop runtime should open workspace files");
    let app = DesktopEframeApp::new(runtime);
    app.headless_egui_context().enable_accesskit();
    (workspace, app)
}

fn git_app() -> (TempWorkspace, DesktopEframeApp) {
    let workspace = TempWorkspace::new();
    run_git(workspace.path(), &["init"]);
    run_git(workspace.path(), &["branch", "-M", "main"]);
    run_git(
        workspace.path(),
        &["config", "user.email", "legion@example.test"],
    );
    run_git(workspace.path(), &["config", "user.name", "Legion Test"]);
    workspace.write("README.md", "palette confirmation\n");
    run_git(workspace.path(), &["add", "."]);
    run_git(workspace.path(), &["commit", "-m", "initial"]);
    let runtime = DesktopRuntime::open(DesktopLaunchConfig::new(
        workspace.path().to_path_buf(),
        None,
    ))
    .expect("desktop runtime should open git workspace");
    let app = DesktopEframeApp::new(runtime);
    app.headless_egui_context().enable_accesskit();
    (workspace, app)
}

fn run_git(root: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .current_dir(root)
        .args(args)
        .output()
        .expect("git command should run");
    assert!(
        output.status.success(),
        "git {args:?} failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).trim().to_string()
}

fn input(events: Vec<egui::Event>) -> egui::RawInput {
    input_at(egui::vec2(1_200.0, 900.0), events)
}

fn input_at(size: egui::Vec2, events: Vec<egui::Event>) -> egui::RawInput {
    egui::RawInput {
        focused: true,
        screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, size)),
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

fn accesskit_clickable_bounds(output: &egui::FullOutput, label: &str) -> egui::accesskit::Rect {
    output
        .platform_output
        .accesskit_update
        .as_ref()
        .expect("frame should expose AccessKit")
        .nodes
        .iter()
        .find_map(|(_id, node)| {
            (node.label() == Some(label) && node.supports_action(egui::accesskit::Action::Click))
                .then(|| node.bounds())
                .flatten()
        })
        .unwrap_or_else(|| panic!("frame should expose clickable bounds for `{label}`"))
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

fn palette_option_semantics(
    output: &egui::FullOutput,
) -> Vec<(String, Option<String>, Option<bool>, bool)> {
    output
        .platform_output
        .accesskit_update
        .as_ref()
        .map(|update| {
            let Some((_list_id, list)) = update.nodes.iter().find(|(_id, node)| {
                node.role() == egui::accesskit::Role::ListBox
                    && node.label() == Some("Command results")
            }) else {
                return Vec::new();
            };
            let mut pending = list.children().to_vec();
            let mut options = Vec::new();
            while let Some(id) = pending.first().copied() {
                pending.remove(0);
                let Some((_id, node)) = update.nodes.iter().find(|(candidate, _)| *candidate == id)
                else {
                    continue;
                };
                if node.role() == egui::accesskit::Role::ListBoxOption {
                    options.push((
                        node.label().unwrap_or_default().to_string(),
                        node.description().map(str::to_string),
                        node.is_selected(),
                        node.is_disabled(),
                    ));
                }
                pending.splice(0..0, node.children().iter().copied());
            }
            options
        })
        .unwrap_or_default()
}

fn palette_group_headings(output: &egui::FullOutput) -> Vec<String> {
    const GROUPS: [&str; 6] = ["Suggested", "Files", "View", "Run", "Git", "Destructive"];
    let mut headings = output
        .platform_output
        .accesskit_update
        .as_ref()
        .map(|update| {
            update
                .nodes
                .iter()
                .filter_map(|(_id, node)| {
                    let label = node.label().or(node.value())?;
                    let bounds = node.bounds()?;
                    GROUPS
                        .contains(&label)
                        .then(|| (bounds.y0, label.to_string()))
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    headings.sort_by(|left, right| left.0.total_cmp(&right.0));
    headings.into_iter().map(|(_, label)| label).collect()
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

fn press_arrow_down() -> egui::RawInput {
    input(vec![egui::Event::Key {
        key: egui::Key::ArrowDown,
        physical_key: Some(egui::Key::ArrowDown),
        pressed: true,
        repeat: false,
        modifiers: egui::Modifiers::default(),
    }])
}

fn press_escape() -> egui::RawInput {
    press_escape_at(egui::vec2(1_200.0, 900.0))
}

fn press_escape_at(size: egui::Vec2) -> egui::RawInput {
    input_at(
        size,
        vec![egui::Event::Key {
            key: egui::Key::Escape,
            physical_key: Some(egui::Key::Escape),
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::default(),
        }],
    )
}

fn full_frame_click_at(
    app: &mut DesktopEframeApp,
    output: &egui::FullOutput,
    label: &str,
    size: egui::Vec2,
) -> egui::FullOutput {
    let target = output
        .platform_output
        .accesskit_update
        .as_ref()
        .expect("frame should expose AccessKit")
        .nodes
        .iter()
        .find_map(|(id, node)| {
            (node.label() == Some(label) && node.supports_action(egui::accesskit::Action::Click))
                .then_some(*id)
        })
        .unwrap_or_else(|| panic!("frame should expose clickable `{label}`"));
    app.run_headless_full_frame(input_at(
        size,
        vec![egui::Event::AccessKitActionRequest(
            egui::accesskit::ActionRequest {
                action: egui::accesskit::Action::Click,
                target_tree: egui::accesskit::TreeId::ROOT,
                target_node: target,
                data: None,
            },
        )],
    ))
}

fn full_frame_accesskit_action(
    app: &mut DesktopEframeApp,
    output: &egui::FullOutput,
    label: &str,
    action: egui::accesskit::Action,
) -> egui::FullOutput {
    let target = output
        .platform_output
        .accesskit_update
        .as_ref()
        .expect("frame should expose AccessKit")
        .nodes
        .iter()
        .find_map(|(id, node)| {
            (node.label() == Some(label) && node.supports_action(action)).then_some(*id)
        })
        .unwrap_or_else(|| panic!("frame should expose {action:?} for `{label}`"));
    app.run_headless_full_frame(input(vec![egui::Event::AccessKitActionRequest(
        egui::accesskit::ActionRequest {
            action,
            target_tree: egui::accesskit::TreeId::ROOT,
            target_node: target,
            data: None,
        },
    )]))
}

fn click_accessible_control(
    app: &mut DesktopEframeApp,
    output: &egui::FullOutput,
    label: &str,
) -> egui::FullOutput {
    let target = output
        .platform_output
        .accesskit_update
        .as_ref()
        .expect("frame should expose AccessKit")
        .nodes
        .iter()
        .find_map(|(id, node)| {
            (node.label() == Some(label) && node.supports_action(egui::accesskit::Action::Click))
                .then_some(*id)
        })
        .unwrap_or_else(|| panic!("frame should expose clickable `{label}`"));
    app.run_headless_input(input(vec![egui::Event::AccessKitActionRequest(
        egui::accesskit::ActionRequest {
            action: egui::accesskit::Action::Click,
            target_tree: egui::accesskit::TreeId::ROOT,
            target_node: target,
            data: None,
        },
    )]))
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
fn command_palette_group_headings_are_contiguous_and_canonical() {
    let (_workspace, mut app) = app();
    app.handle_action(DesktopAction::OpenPalette {
        mode: PaletteMode::Command,
        query: ">".to_string(),
        scope: SearchScopeProjection::Workspace,
    })
    .expect("command palette should open");

    let output = app.run_headless_input(input(Vec::new()));
    assert_eq!(
        palette_group_headings(&output),
        ["Suggested", "Files", "View", "Run", "Git", "Destructive"],
        "each command group heading should appear once in canonical product order"
    );
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
fn command_palette_visual_keyboard_and_accessibility_order_match_app_projection() {
    let (_workspace, mut app) = app();
    app.handle_action(DesktopAction::OpenPalette {
        mode: PaletteMode::Command,
        query: ">".to_string(),
        scope: SearchScopeProjection::Workspace,
    })
    .expect("command palette should open");
    let projected = app.runtime_snapshot().palette_projection;
    let first_disabled = projected
        .results
        .iter()
        .position(|result| result.disabled_reason.is_some())
        .expect("an empty workspace should expose unavailable commands");
    assert!(
        projected.results[..first_disabled]
            .iter()
            .all(|result| result.disabled_reason.is_none()),
        "the app projection must rank every available command first"
    );

    let output = app.run_headless_input(input(Vec::new()));
    let options = palette_option_semantics(&output);
    assert_eq!(
        options.iter().map(|row| row.0.as_str()).collect::<Vec<_>>(),
        projected
            .results
            .iter()
            .take(options.len())
            .map(|result| result.title.as_str())
            .collect::<Vec<_>>(),
        "painted/list-option order must be the app-owned navigation order"
    );
    let update = output
        .platform_output
        .accesskit_update
        .as_ref()
        .expect("palette should expose AccessKit");
    assert!(update.nodes.iter().any(|(_id, node)| {
        node.role() == egui::accesskit::Role::ListBox && node.label() == Some("Command results")
    }));
    for (index, (label, description, selected, disabled)) in options.iter().enumerate() {
        let result = &projected.results[index];
        assert_eq!(label, &result.title);
        assert_eq!(
            description.as_deref(),
            result
                .disabled_reason
                .as_deref()
                .or(result.detail.as_deref())
        );
        assert_eq!(*selected, Some(index == projected.selected_index));
        assert_eq!(*disabled, result.disabled_reason.is_some());
    }

    let moved = app.run_headless_input(press_arrow_down());
    let moved_projection = app.runtime_snapshot().palette_projection;
    assert!(
        moved_projection.results[moved_projection.selected_index]
            .disabled_reason
            .is_none()
    );
    let moved_selected = palette_option_semantics(&moved)
        .into_iter()
        .find_map(|(label, _, selected, _)| (selected == Some(true)).then_some(label));
    assert_eq!(
        moved_selected.as_deref(),
        Some(
            moved_projection.results[moved_projection.selected_index]
                .title
                .as_str()
        )
    );

    app.handle_action(DesktopAction::OpenPalette {
        mode: PaletteMode::Command,
        query: ">save".to_string(),
        scope: SearchScopeProjection::Workspace,
    })
    .expect("mixed available and unavailable commands should remain searchable");
    let mixed_projection = app.runtime_snapshot().palette_projection;
    let mixed = palette_option_semantics(&app.run_headless_input(input(Vec::new())));
    assert_eq!(mixed.len(), mixed_projection.results.len());
    assert!(mixed.iter().any(|row| row.3));
    for (index, (_, description, selected, disabled)) in mixed.iter().enumerate() {
        let result = &mixed_projection.results[index];
        assert_eq!(
            description.as_deref(),
            result
                .disabled_reason
                .as_deref()
                .or(result.detail.as_deref())
        );
        assert_eq!(*selected, Some(index == mixed_projection.selected_index));
        assert_eq!(*disabled, result.disabled_reason.is_some());
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
    assert!(
        app.runtime_snapshot()
            .palette_projection
            .pending_confirmation
            .is_some(),
        "the renderer must display the app-owned pending confirmation"
    );
    assert!(accesskit_has_label(
        &output,
        "Confirm Preferences: Reset Settings"
    ));
    for label in ["Confirm", "Cancel"] {
        let bounds = accesskit_clickable_bounds(&output, label);
        assert!(
            bounds.x1 - bounds.x0 >= 28.0 && bounds.y1 - bounds.y0 >= 28.0,
            "{label} confirmation target must be at least 28x28: {bounds:?}"
        );
    }

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
fn palette_escape_closes_only_the_foreground_layer_over_a_compact_drawer() {
    let (_workspace, mut app) = app();
    let size = egui::vec2(960.0, 720.0);
    let initial = app.run_headless_full_frame(input_at(size, Vec::new()));
    let _opened = full_frame_click_at(&mut app, &initial, "Explorer drawer", size);
    let drawer = app.run_headless_full_frame(input_at(size, Vec::new()));
    assert!(accesskit_has_label(&drawer, "Close Explorer drawer"));

    app.handle_action(DesktopAction::OpenPalette {
        mode: PaletteMode::Command,
        query: ">".to_string(),
        scope: SearchScopeProjection::Workspace,
    })
    .expect("palette should open above the compact drawer");
    let layered = app.run_headless_full_frame(input_at(size, Vec::new()));
    assert!(accesskit_has_label(&layered, "Close Explorer drawer"));
    assert!(app.runtime_snapshot().palette_projection.open);

    app.run_headless_full_frame(press_escape_at(size));
    let drawer = app.run_headless_full_frame(input_at(size, Vec::new()));
    assert!(!app.runtime_snapshot().palette_projection.open);
    assert!(
        accesskit_has_label(&drawer, "Close Explorer drawer"),
        "the Escape consumed by the foreground palette must not close its underlying drawer"
    );

    app.run_headless_full_frame(press_escape_at(size));
    let closed = app.run_headless_full_frame(input_at(size, Vec::new()));
    assert!(!accesskit_has_label(&closed, "Close Explorer drawer"));
}

#[test]
fn git_command_confirmation_shows_resolved_target_and_cancel_does_not_dispatch() {
    let (workspace, mut app) = git_app();
    app.handle_action(DesktopAction::OpenPalette {
        mode: PaletteMode::Command,
        query: ">git create branch feature/cancelled".to_string(),
        scope: SearchScopeProjection::Workspace,
    })
    .expect("Git command should remain searchable with an operand");

    let output = app.run_headless_input(press_enter());
    assert!(accesskit_has_label(
        &output,
        "Create and switch to branch ‘feature/cancelled’"
    ));
    let _cancelled = click_accessible_control(&mut app, &output, "Cancel");

    assert!(app.runtime_snapshot().palette_projection.open);
    assert_eq!(
        run_git(workspace.path(), &["branch", "--show-current"]),
        "main"
    );
    assert!(run_git(workspace.path(), &["branch", "--list", "feature/cancelled"]).is_empty());
}

#[test]
fn git_command_confirmation_dispatches_the_resolved_operand() {
    let (workspace, mut app) = git_app();
    app.handle_action(DesktopAction::OpenPalette {
        mode: PaletteMode::Command,
        query: ">git create branch feature/confirmed".to_string(),
        scope: SearchScopeProjection::Workspace,
    })
    .expect("Git command should remain searchable with an operand");

    let output = app.run_headless_input(press_enter());
    assert!(accesskit_has_label(
        &output,
        "Create and switch to branch ‘feature/confirmed’"
    ));
    let _confirmed = click_accessible_control(&mut app, &output, "Confirm");

    assert_eq!(
        run_git(workspace.path(), &["branch", "--show-current"]),
        "feature/confirmed",
        "confirmation status: {:?}",
        app.runtime_snapshot().status_messages
    );
    assert!(!app.runtime_snapshot().palette_projection.open);
}

#[test]
fn palette_settings_command_opens_overlay_and_escape_restores_command_focus() {
    let (_workspace, mut app) = app();
    let initial = app.run_headless_full_frame(input(Vec::new()));
    let focused = full_frame_accesskit_action(
        &mut app,
        &initial,
        "Command",
        egui::accesskit::Action::Focus,
    );
    let origin = app
        .headless_egui_context()
        .memory(|memory| memory.focused())
        .expect("Command should accept keyboard focus");
    let _opened = full_frame_accesskit_action(
        &mut app,
        &focused,
        "Command",
        egui::accesskit::Action::Click,
    );
    let _palette = app.run_headless_full_frame(input(Vec::new()));
    app.run_headless_full_frame(input(vec![egui::Event::Text(
        "preferences open settings".to_string(),
    )]));
    assert_eq!(
        app.runtime_snapshot().palette_projection.query,
        ">preferences open settings"
    );

    app.run_headless_full_frame(press_enter());
    let settings = app.run_headless_full_frame(input(Vec::new()));
    assert!(
        accesskit_has_label(&settings, "Close Settings"),
        "palette outcome must open the renderer-local Settings overlay"
    );
    assert!(!app.runtime_snapshot().palette_projection.open);

    app.run_headless_full_frame(press_escape());
    app.run_headless_full_frame(input(Vec::new()));
    assert_eq!(
        app.headless_egui_context()
            .memory(|memory| memory.focused()),
        Some(origin),
        "Escape must restore focus to the Command control that originated the palette"
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

    app.run_headless_input(press_enter());
    let search = &app.runtime_snapshot().search_projection;
    assert_eq!(search.query_label, "needle");
    assert_eq!(search.scope, SearchScopeProjection::Workspace);
    assert!(search.query_id.is_some(), "Enter should dispatch RunSearch");
}

#[test]
fn search_palette_enter_opens_the_selected_match() {
    let (workspace, mut app) = app_with_workspace_files(&[("match.txt", "needle here\n")]);
    let target = workspace.path().join("match.txt");
    app.handle_action(DesktopAction::OpenPathText(
        target.to_string_lossy().into_owned(),
    ))
    .expect("matching file should open");
    app.handle_action(DesktopAction::OpenPalette {
        mode: PaletteMode::Search,
        query: "/needle".to_string(),
        scope: SearchScopeProjection::ActiveFile,
    })
    .expect("search palette should open");

    let results = app.run_headless_input(press_enter());
    assert!(accesskit_has_label(
        &results,
        "Enter open result · ↑↓ choose · Esc close"
    ));
    let selected = app.runtime_snapshot().palette_projection.results[0].clone();

    app.run_headless_input(press_enter());
    let snapshot = app.runtime_snapshot();
    assert!(!snapshot.palette_projection.open);
    assert!(
        snapshot
            .active_buffer_projection
            .file_path
            .as_ref()
            .is_some_and(|path| path.0.ends_with("match.txt"))
    );
    let active_buffer_id = snapshot
        .active_buffer_projection
        .buffer_id
        .expect("opened match should have an active buffer");
    assert_eq!(
        snapshot
            .daily_editing_projection
            .viewport_states
            .iter()
            .find(|viewport| viewport.buffer_id == active_buffer_id)
            .and_then(|viewport| viewport.cursor),
        selected.position
    );
}

#[test]
fn search_palette_keeps_all_matches_in_one_scrollable_result_list() {
    let files = [
        ("result-0.txt", "needle-0\n"),
        ("result-1.txt", "needle-1\n"),
        ("result-2.txt", "needle-2\n"),
        ("result-3.txt", "needle-3\n"),
        ("result-4.txt", "needle-4\n"),
        ("result-5.txt", "needle-5\n"),
        ("result-6.txt", "needle-6\n"),
        ("result-7.txt", "needle-7\n"),
        ("result-8.txt", "needle-8\n"),
        ("result-9.txt", "needle-9\n"),
        ("result-10.txt", "needle-10\n"),
        ("result-11.txt", "needle-11\n"),
    ];
    let (_workspace, mut app) = app_with_workspace_files(&files);
    app.handle_action(DesktopAction::OpenPalette {
        mode: PaletteMode::Search,
        query: "/needle".to_string(),
        scope: SearchScopeProjection::Workspace,
    })
    .expect("search palette should open");

    let output = app.run_headless_input(press_enter());
    let snapshot = app.runtime_snapshot();
    assert!(snapshot.palette_projection.open);
    assert_eq!(snapshot.palette_projection.results.len(), 12);
    assert_eq!(snapshot.palette_projection.selected_index, 0);
    let text = accesskit_text(&output);
    assert!(
        accesskit_has_label(&output, "12 matches in the workspace for \"needle\" [Cc]"),
        "palette text: {text:?}"
    );
    for index in 0..12 {
        let snippet = format!("needle-{index}\n");
        assert_eq!(
            text.iter().filter(|row| row.contains(&snippet)).count(),
            1,
            "{snippet} must appear once in the palette and nowhere in a second results panel: {text:?}"
        );
    }
}

#[test]
fn search_palette_keyboard_navigation_moves_between_matches() {
    let files = [
        ("alpha.txt", "needle alpha\n"),
        ("beta.txt", "needle beta\n"),
        ("gamma.txt", "needle gamma\n"),
    ];
    let (_workspace, mut app) = app_with_workspace_files(&files);
    app.handle_action(DesktopAction::OpenPalette {
        mode: PaletteMode::Search,
        query: "/needle".to_string(),
        scope: SearchScopeProjection::Workspace,
    })
    .expect("search palette should open");
    app.run_headless_input(press_enter());

    let output = app.run_headless_input(press_arrow_down());
    let snapshot = app.runtime_snapshot();
    assert_eq!(snapshot.palette_projection.selected_index, 1);
    let selected_title = snapshot.palette_projection.results[1].title.clone();
    assert!(accesskit_has_label(&output, &selected_title));
    assert!(snapshot.palette_projection.open);
}

#[test]
fn search_palette_retries_no_match_and_validation_states_with_the_same_query() {
    let (_workspace, mut app) = app_with_workspace_files(&[("alpha.txt", "present\n")]);
    app.handle_action(DesktopAction::OpenPalette {
        mode: PaletteMode::Search,
        query: "/missing".to_string(),
        scope: SearchScopeProjection::Workspace,
    })
    .expect("search palette should open");

    let no_match = app.run_headless_input(press_enter());
    assert!(app.runtime_snapshot().palette_projection.open);
    assert!(accesskit_has_label(
        &no_match,
        "No matches. Try another term or search the active file."
    ));
    assert!(!accesskit_has_label(&no_match, "No search results"));
    assert!(accesskit_has_label(
        &no_match,
        "Enter run search again · ↑↓ choose · Esc close"
    ));
    assert_eq!(
        app.runtime_snapshot().palette_projection.results[0].id,
        "search:retry"
    );
    let no_match_query_id = app.runtime_snapshot().search_projection.query_id.clone();
    app.run_headless_input(press_enter());
    let retried_no_match = app.runtime_snapshot();
    assert!(
        retried_no_match.palette_projection.open,
        "retry should keep the integrated search result host open"
    );
    assert_eq!(retried_no_match.search_projection.query_label, "missing");
    assert_eq!(
        retried_no_match.search_projection.scope,
        SearchScopeProjection::Workspace
    );
    assert_ne!(
        retried_no_match.search_projection.query_id, no_match_query_id,
        "retry Enter should dispatch a new RunSearch intent"
    );

    app.handle_action(DesktopAction::UpdatePaletteQuery {
        query: "/regex:[".to_string(),
    })
    .expect("search query should update");
    let error = app.run_headless_input(press_enter());
    assert!(app.runtime_snapshot().palette_projection.open);
    assert!(accesskit_has_label(
        &error,
        "Check the search term and try again."
    ));
    assert!(accesskit_has_label(
        &error,
        "Enter run search again · ↑↓ choose · Esc close"
    ));
    let validation_query_id = app.runtime_snapshot().search_projection.query_id.clone();
    app.run_headless_input(press_enter());
    let retried_validation = app.runtime_snapshot();
    assert!(retried_validation.palette_projection.open);
    assert_eq!(retried_validation.search_projection.query_label, "regex:[");
    assert_eq!(
        retried_validation.search_projection.status.kind,
        SearchStatusKindProjection::ValidationError
    );
    assert_ne!(
        retried_validation.search_projection.query_id, validation_query_id,
        "validation retry should dispatch the same query through RunSearch"
    );
}

#[test]
fn search_palette_cancelled_state_retries_with_enter() {
    let (_workspace, mut app) = app_with_workspace_files(&[("alpha.txt", "present\n")]);
    app.handle_action(DesktopAction::OpenPalette {
        mode: PaletteMode::Search,
        query: "/missing".to_string(),
        scope: SearchScopeProjection::Workspace,
    })
    .expect("search palette should open");
    app.run_headless_input(press_enter());
    let original_query_id = app
        .runtime_snapshot()
        .search_projection
        .query_id
        .expect("search should have a query id");

    app.handle_action(DesktopAction::CancelSearch {
        query_id: original_query_id.clone(),
    })
    .expect("search cancellation should route");
    let cancelled = app.run_headless_input(input(Vec::new()));
    assert!(accesskit_has_label(
        &cancelled,
        "Enter run search again · ↑↓ choose · Esc close"
    ));
    assert_eq!(
        app.runtime_snapshot().palette_projection.results[0].id,
        "search:retry"
    );

    app.run_headless_input(press_enter());
    let retried = app.runtime_snapshot();
    assert!(retried.palette_projection.open);
    assert_eq!(retried.search_projection.query_label, "missing");
    assert_eq!(
        retried.search_projection.scope,
        SearchScopeProjection::Workspace
    );
    assert_ne!(
        retried.search_projection.query_id.as_deref(),
        Some(original_query_id.as_str())
    );
}

#[test]
fn search_palette_degraded_state_defaults_enter_to_retry() {
    let mut content = String::from("needle visible\n");
    content.push_str(&"x".repeat(5 * 1024 * 1024));
    let (workspace, mut app) = app_with_workspace_files(&[("large.txt", &content)]);
    let target = workspace.path().join("large.txt");
    app.handle_action(DesktopAction::OpenPathText(
        target.to_string_lossy().into_owned(),
    ))
    .expect("large file should open in degraded mode");
    app.handle_action(DesktopAction::OpenPalette {
        mode: PaletteMode::Search,
        query: "/needle".to_string(),
        scope: SearchScopeProjection::ActiveFile,
    })
    .expect("search palette should open");

    let degraded = app.run_headless_input(press_enter());
    let first = app.runtime_snapshot();
    assert_eq!(
        first.search_projection.status.kind,
        SearchStatusKindProjection::DegradedLimited
    );
    assert_eq!(first.palette_projection.results[0].id, "search:retry");
    assert!(accesskit_has_label(
        &degraded,
        "Enter run search again · ↑↓ choose · Esc close"
    ));
    let first_query_id = first.search_projection.query_id;

    app.run_headless_input(press_enter());
    let retried = app.runtime_snapshot();
    assert!(retried.palette_projection.open);
    assert_eq!(retried.search_projection.query_label, "needle");
    assert_eq!(
        retried.search_projection.status.kind,
        SearchStatusKindProjection::DegradedLimited
    );
    assert_ne!(retried.search_projection.query_id, first_query_id);
}
