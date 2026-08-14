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

fn press_arrow_down() -> egui::RawInput {
    input(vec![egui::Event::Key {
        key: egui::Key::ArrowDown,
        physical_key: Some(egui::Key::ArrowDown),
        pressed: true,
        repeat: false,
        modifiers: egui::Modifiers::default(),
    }])
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
fn search_palette_renders_no_match_and_error_states_in_place() {
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
}
