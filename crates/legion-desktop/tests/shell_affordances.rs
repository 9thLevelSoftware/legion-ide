//! The shell's primary affordances, driven through the rendered UI.
//!
//! Each test here corresponds to a defect found only by actually running the
//! app — none of them were visible to the projection-level tests, because every
//! one is a property of *rendering and hit-testing*, not of app state:
//!
//! * clicking a button parked egui's keyboard focus on it, and the editor's
//!   input guard treated any focused widget as a text field, so typing stopped
//!   permanently with nothing on screen to explain it;
//! * a tab's close button sat inside the tab's own click rect and, being
//!   registered first, lost every click to it — the `×` switched tabs;
//! * the unsaved-changes prompt was appended to a panel whose remaining height
//!   had already been consumed, so it rendered below the window edge while
//!   simultaneously disabling typing.
//!
//! Several surfaces render a clickable control labelled with the same file
//! name — the explorer row, the tab, the excerpt panel. A bare label lookup
//! grabs whichever one happens to come first and silently changes meaning when
//! unrelated code moves, so every lookup here is qualified by screen region.

use std::{
    fs,
    path::Path,
    sync::{Mutex, OnceLock},
};

mod common;
use common::TempWorkspace;

use legion_desktop::{
    bridge::DesktopAction,
    workflow::{DesktopEframeApp, DesktopLaunchConfig, DesktopRuntime},
};

const SCREEN_W: f32 = 1_600.0;
const SCREEN_H: f32 = 1_000.0;

static TEST_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

/// These tests share a process-wide egui/font cache and temp-dir namespace.
fn guard() -> std::sync::MutexGuard<'static, ()> {
    match TEST_LOCK.get_or_init(|| Mutex::new(())).lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    }
}

fn open_app(root: &Path) -> DesktopEframeApp {
    let runtime = DesktopRuntime::open(DesktopLaunchConfig::new(root.to_path_buf(), None))
        .expect("desktop runtime should open workspace");
    DesktopEframeApp::new(runtime)
}

fn screen() -> egui::Rect {
    egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(SCREEN_W, SCREEN_H))
}

fn frame(events: Vec<egui::Event>) -> egui::RawInput {
    egui::RawInput {
        focused: true,
        screen_rect: Some(screen()),
        events,
        ..egui::RawInput::default()
    }
}

/// All clickable accessibility nodes: (label, rect).
fn clickables(output: &egui::FullOutput) -> Vec<(String, egui::Rect)> {
    output
        .platform_output
        .accesskit_update
        .as_ref()
        .expect("full headless frames should expose the accessibility tree")
        .nodes
        .iter()
        .filter(|(_, node)| node.supports_action(egui::accesskit::Action::Click))
        .filter_map(|(_, node)| {
            let bounds = node.bounds()?;
            Some((
                node.label()?.to_string(),
                egui::Rect::from_min_max(
                    egui::pos2(bounds.x0 as f32, bounds.y0 as f32),
                    egui::pos2(bounds.x1 as f32, bounds.y1 as f32),
                ),
            ))
        })
        .collect()
}

/// Centre of the one clickable node with `label` whose centre is in `region`.
///
/// Asserts uniqueness rather than taking the first match: a lookup that
/// silently picks between duplicates produces tests that pass while exercising
/// something other than what they name.
fn clickable_center_in(output: &egui::FullOutput, label: &str, region: egui::Rect) -> egui::Pos2 {
    let hits: Vec<egui::Rect> = clickables(output)
        .into_iter()
        .filter(|(found, rect)| found == label && region.contains(rect.center()))
        .map(|(_, rect)| rect)
        .collect();
    assert_eq!(
        hits.len(),
        1,
        "expected exactly one clickable `{label}` inside {region:?}, found {hits:?}"
    );
    hits[0].center()
}

fn clickable_center(output: &egui::FullOutput, label: &str) -> egui::Pos2 {
    clickable_center_in(output, label, screen())
}

/// The editor tab strip. Tabs, explorer rows and excerpt rows can all carry a
/// file name as their label, so tab lookups must be bounded to this band.
fn tab_strip_band() -> egui::Rect {
    egui::Rect::from_min_max(egui::pos2(200.0, 40.0), egui::pos2(SCREEN_W, 130.0))
}

fn click_at(app: &mut DesktopEframeApp, pos: egui::Pos2) -> egui::FullOutput {
    let _ = app.run_headless_full_frame(frame(vec![egui::Event::PointerMoved(pos)]));
    let _ = app.run_headless_full_frame(frame(vec![
        egui::Event::PointerMoved(pos),
        egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed: true,
            modifiers: egui::Modifiers::default(),
        },
    ]));
    let _ = app.run_headless_full_frame(frame(vec![
        egui::Event::PointerMoved(pos),
        egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: egui::Modifiers::default(),
        },
    ]));
    // The action the click produced is dispatched on the following frame.
    app.run_headless_full_frame(frame(Vec::new()))
}

fn type_text(app: &mut DesktopEframeApp, text: &str) -> egui::FullOutput {
    app.run_headless_full_frame(frame(vec![egui::Event::Text(text.to_string())]))
}

fn open_file(app: &mut DesktopEframeApp, name: &str) {
    let _ = app.handle_action(DesktopAction::RefreshExplorer);
    let node = app
        .runtime_snapshot()
        .explorer_projection
        .nodes
        .into_iter()
        .find(|node| node.name == name)
        .unwrap_or_else(|| panic!("explorer should project `{name}`"));
    app.handle_action(DesktopAction::SelectExplorerFile {
        file_id: node.file_id,
    })
    .expect("opening a file should succeed");
}

fn buffer_text(app: &DesktopEframeApp) -> String {
    app.runtime_snapshot()
        .active_buffer_projection
        .viewport
        .as_ref()
        .map(|viewport| {
            viewport
                .line_slices
                .iter()
                .map(|slice| slice.visible_text.clone())
                .collect::<Vec<_>>()
                .join("\n")
        })
        .unwrap_or_default()
}

fn open_tab_titles(app: &DesktopEframeApp) -> Vec<(String, bool)> {
    app.runtime_snapshot()
        .daily_editing_projection
        .tabs
        .tabs
        .iter()
        .map(|tab| (tab.title.clone(), tab.active))
        .collect()
}

#[test]
fn clicking_a_rail_button_does_not_stop_the_editor_accepting_keystrokes() {
    let _guard = guard();
    let workspace = TempWorkspace::new("legion_desktop_shell_affordances");
    workspace.write("focus.txt", "seed\n");
    let mut app = open_app(workspace.path());
    open_file(&mut app, "focus.txt");

    let primed = app.run_headless_full_frame(frame(Vec::new()));
    let gear = clickable_center(&primed, "Settings");
    let _ = click_at(&mut app, gear);

    let before = buffer_text(&app);
    let _ = type_text(&mut app, "A");
    let after = buffer_text(&app);

    assert_ne!(
        before, after,
        "typing must still reach the buffer after clicking a rail button. \
         The editor-input guard tested `mem.focused().is_some()`, but egui \
         hands focus to plain buttons too and a button never surrenders it, so \
         one click on the gear discarded every keystroke from then on — with \
         nothing on screen to say why."
    );
}

#[test]
fn closing_the_settings_overlay_leaves_the_editor_usable() {
    let _guard = guard();
    let workspace = TempWorkspace::new("legion_desktop_shell_affordances");
    workspace.write("focus.txt", "seed\n");
    let mut app = open_app(workspace.path());
    open_file(&mut app, "focus.txt");

    // Open and close again: the worst form of the old bug, because the overlay
    // was gone and the UI looked exactly as it had before it was opened.
    let primed = app.run_headless_full_frame(frame(Vec::new()));
    let gear = clickable_center(&primed, "Settings");
    let _ = click_at(&mut app, gear);
    let _ = click_at(&mut app, gear);

    let before = buffer_text(&app);
    let _ = type_text(&mut app, "B");
    assert_ne!(
        before,
        buffer_text(&app),
        "typing must work once the settings overlay is dismissed"
    );
}

#[test]
fn clicking_a_tabs_close_button_closes_that_tab() {
    let _guard = guard();
    let workspace = TempWorkspace::new("legion_desktop_shell_affordances");
    workspace.write("alpha.txt", "alpha\n");
    workspace.write("beta.txt", "beta\n");
    let mut app = open_app(workspace.path());
    open_file(&mut app, "alpha.txt");
    open_file(&mut app, "beta.txt");
    assert_eq!(open_tab_titles(&app).len(), 2, "two tabs should be open");

    let primed = app.run_headless_full_frame(frame(Vec::new()));
    let close = clickable_center_in(&primed, "Close alpha.txt", tab_strip_band());
    let _ = click_at(&mut app, close);

    let titles = open_tab_titles(&app);
    assert_eq!(
        titles.len(),
        1,
        "the close button must close its tab, got {titles:?}"
    );
    assert!(
        !titles.iter().any(|(title, _)| title == "alpha.txt"),
        "alpha.txt should be the tab that closed, got {titles:?}"
    );
}

#[test]
fn a_tabs_close_button_does_not_merely_switch_to_that_tab() {
    // The specific failure this guards: the close button's rect sits inside the
    // tab's, and egui hit-tests in favour of the widget registered last. With
    // the tab registered second it won every click on the `×`, so clicking
    // close on an inactive tab activated it instead — which looks like the
    // button "not working" rather than like a hit-testing order bug.
    let _guard = guard();
    let workspace = TempWorkspace::new("legion_desktop_shell_affordances");
    workspace.write("alpha.txt", "alpha\n");
    workspace.write("beta.txt", "beta\n");
    let mut app = open_app(workspace.path());
    open_file(&mut app, "alpha.txt");
    open_file(&mut app, "beta.txt");

    let primed = app.run_headless_full_frame(frame(Vec::new()));
    let close = clickable_center_in(&primed, "Close alpha.txt", tab_strip_band());
    let _ = click_at(&mut app, close);

    let titles = open_tab_titles(&app);
    assert!(
        !titles
            .iter()
            .any(|(title, active)| title == "alpha.txt" && *active),
        "clicking close on an inactive tab must not activate it, got {titles:?}"
    );
}

#[test]
fn the_unsaved_changes_prompt_renders_inside_the_window() {
    let _guard = guard();
    let workspace = TempWorkspace::new("legion_desktop_shell_affordances");
    workspace.write("dirty.txt", "seed\n");
    let mut app = open_app(workspace.path());
    open_file(&mut app, "dirty.txt");

    let _ = app.run_headless_full_frame(frame(Vec::new()));
    let _ = type_text(&mut app, "X");
    assert!(
        app.runtime_snapshot()
            .daily_editing_projection
            .tabs
            .tabs
            .iter()
            .any(|tab| tab.dirty),
        "typing should leave the buffer dirty"
    );

    let buffer_id = app
        .runtime_snapshot()
        .active_buffer_projection
        .buffer_id
        .expect("a buffer should be open");
    app.handle_action(DesktopAction::CloseTab { buffer_id })
        .expect("closing a dirty tab should raise the prompt");
    let output = app.run_headless_full_frame(frame(Vec::new()));

    assert!(
        app.runtime_snapshot()
            .daily_editing_projection
            .close_dirty_prompt
            .is_some(),
        "closing a dirty tab should raise the unsaved-changes prompt"
    );

    // Both escapes must be reachable. The prompt also disables editor input,
    // so a prompt rendered off-screen is not a cosmetic bug — it is a lock-up
    // with no keyboard way out.
    for label in ["Save and close", "Cancel"] {
        let rects: Vec<egui::Rect> = clickables(&output)
            .into_iter()
            .filter(|(found, _)| found == label)
            .map(|(_, rect)| rect)
            .collect();
        assert_eq!(
            rects.len(),
            1,
            "expected one `{label}` control, got {rects:?}"
        );
        assert!(
            screen().contains_rect(rects[0]),
            "`{label}` must render inside the window: {:?} is not within {:?}",
            rects[0],
            screen()
        );
    }
}

#[test]
fn escape_dismisses_the_unsaved_changes_prompt_and_restores_typing() {
    // Keyboard rather than mouse, deliberately. Every modal test in this crate
    // drives dialogs from the keyboard, and this prompt in particular *must*
    // have a keyboard answer: it disables editor input while it is up, so a
    // modal reachable only by mouse is one bad layout away from a hang — which
    // is exactly what it was before, rendering below the window's bottom edge.
    let _guard = guard();
    let workspace = TempWorkspace::new("legion_desktop_shell_affordances");
    workspace.write("dirty.txt", "seed\n");
    let mut app = open_app(workspace.path());
    open_file(&mut app, "dirty.txt");
    let _ = app.run_headless_full_frame(frame(Vec::new()));
    let _ = type_text(&mut app, "X");

    let buffer_id = app
        .runtime_snapshot()
        .active_buffer_projection
        .buffer_id
        .expect("a buffer should be open");
    app.handle_action(DesktopAction::CloseTab { buffer_id })
        .expect("closing a dirty tab should raise the prompt");
    let _ = app.run_headless_full_frame(frame(Vec::new()));

    let _ = app.run_headless_full_frame(frame(vec![egui::Event::Key {
        key: egui::Key::Escape,
        physical_key: Some(egui::Key::Escape),
        pressed: true,
        repeat: false,
        modifiers: egui::Modifiers::default(),
    }]));

    assert!(
        app.runtime_snapshot()
            .daily_editing_projection
            .close_dirty_prompt
            .is_none(),
        "Escape should dismiss the unsaved-changes prompt"
    );
    let before = buffer_text(&app);
    let _ = type_text(&mut app, "Y");
    assert_ne!(
        before,
        buffer_text(&app),
        "dismissing the prompt must give keyboard input back to the editor"
    );
}

#[test]
fn enter_saves_and_closes_from_the_unsaved_changes_prompt() {
    let _guard = guard();
    let workspace = TempWorkspace::new("legion_desktop_shell_affordances");
    workspace.write("dirty.txt", "seed\n");
    let mut app = open_app(workspace.path());
    open_file(&mut app, "dirty.txt");
    let _ = app.run_headless_full_frame(frame(Vec::new()));
    let _ = type_text(&mut app, "X");

    let buffer_id = app
        .runtime_snapshot()
        .active_buffer_projection
        .buffer_id
        .expect("a buffer should be open");
    app.handle_action(DesktopAction::CloseTab { buffer_id })
        .expect("closing a dirty tab should raise the prompt");
    let _ = app.run_headless_full_frame(frame(Vec::new()));

    let _ = app.run_headless_full_frame(frame(vec![egui::Event::Key {
        key: egui::Key::Enter,
        physical_key: Some(egui::Key::Enter),
        pressed: true,
        repeat: false,
        modifiers: egui::Modifiers::default(),
    }]));

    assert!(
        app.runtime_snapshot()
            .daily_editing_projection
            .close_dirty_prompt
            .is_none(),
        "Enter should answer the unsaved-changes prompt"
    );
    assert!(
        open_tab_titles(&app).is_empty(),
        "the tab should be closed, got {:?}",
        open_tab_titles(&app)
    );
    let on_disk = fs::read_to_string(workspace.path().join("dirty.txt"))
        .expect("the file should still be readable");
    assert!(
        on_disk.contains('X'),
        "Enter must actually save before closing, disk holds {on_disk:?}"
    );
}

// --- Multi-cursor is reachable from the keyboard ---------------------------
//
// `AddCursorAbove`, `AddCursorBelow` and `ClearExtraCursors` existed as intents
// with app handling and eight passing tests, and no `DesktopAction`, no bridge
// translation and no keybinding — so the feature could not be used. Same shape
// as the explorer, the session path, and the panel sizes.

fn ctrl_alt_key(key: egui::Key) -> egui::RawInput {
    let modifiers = egui::Modifiers {
        command: true,
        alt: true,
        ..egui::Modifiers::default()
    };
    egui::RawInput {
        focused: true,
        modifiers,
        screen_rect: Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(SCREEN_W, SCREEN_H),
        )),
        events: vec![egui::Event::Key {
            key,
            physical_key: Some(key),
            pressed: true,
            repeat: false,
            modifiers,
        }],
        ..egui::RawInput::default()
    }
}

/// Cursors the active buffer is projecting.
///
/// `expect`s the viewport rather than defaulting, so a fixture without one
/// fails loudly instead of reporting a cursor count that production would
/// disagree with — `projected_cursor_count` in `workflow.rs` treats a missing
/// viewport as one caret, and two meanings for "how many cursors" is how a
/// test ends up passing against behaviour nobody implemented.
fn cursor_count(app: &DesktopEframeApp) -> usize {
    app.runtime_snapshot()
        .active_buffer_projection
        .viewport
        .as_ref()
        .expect("an open buffer projects a viewport")
        .cursors
        .len()
}

#[test]
fn ctrl_alt_down_adds_a_cursor_and_escape_clears_it() {
    let _guard = guard();
    let workspace = TempWorkspace::new("legion_desktop_shell_affordances");
    workspace.write("cursors.txt", "one\ntwo\nthree\n");
    let mut app = open_app(workspace.path());
    open_file(&mut app, "cursors.txt");
    let _ = app.run_headless_full_frame(frame(Vec::new()));

    assert_eq!(cursor_count(&app), 1, "a fresh buffer has one caret");

    let _ = app.run_headless_full_frame(ctrl_alt_key(egui::Key::ArrowDown));
    assert_eq!(
        cursor_count(&app),
        2,
        "Ctrl+Alt+Down must add a cursor — without a binding the feature is \
         unreachable however well the app layer works"
    );

    let _ = app.run_headless_full_frame(frame(vec![egui::Event::Key {
        key: egui::Key::Escape,
        physical_key: Some(egui::Key::Escape),
        pressed: true,
        repeat: false,
        modifiers: egui::Modifiers::default(),
    }]));
    assert_eq!(
        cursor_count(&app),
        1,
        "Escape must collapse the set back to the caret"
    );
}

#[test]
fn ctrl_alt_up_adds_a_cursor_above() {
    let _guard = guard();
    let workspace = TempWorkspace::new("legion_desktop_shell_affordances");
    workspace.write(
        "cursors.txt",
        "one
two
three
",
    );
    let mut app = open_app(workspace.path());
    open_file(&mut app, "cursors.txt");
    let _ = app.run_headless_full_frame(frame(Vec::new()));

    // Put the caret on the last line first. A caret on line 0 has nowhere to
    // go up and correctly adds nothing — asserting `>= before` around that
    // case would hold whether or not the binding existed, which is no test at
    // all.
    app.handle_action(DesktopAction::SetCursor {
        buffer_id: None,
        cursor: legion_protocol::TextCoordinate {
            line: 2,
            character: 0,
            byte_offset: None,
            utf16_offset: None,
        },
    })
    .expect("moving the caret should succeed");
    let _ = app.run_headless_full_frame(frame(Vec::new()));
    assert_eq!(cursor_count(&app), 1, "still one caret before the binding");

    let _ = app.run_headless_full_frame(ctrl_alt_key(egui::Key::ArrowUp));
    assert_eq!(
        cursor_count(&app),
        2,
        "Ctrl+Alt+Up must add a cursor on the line above"
    );
}

#[test]
fn escape_with_one_cursor_is_left_for_other_handlers() {
    // Escape is Vim's mode exit and the completion popup's dismiss. Clearing
    // cursors must not swallow it when there is nothing to clear.
    let _guard = guard();
    let workspace = TempWorkspace::new("legion_desktop_shell_affordances");
    workspace.write("cursors.txt", "one\ntwo\n");
    let mut app = open_app(workspace.path());
    open_file(&mut app, "cursors.txt");
    let _ = app.run_headless_full_frame(frame(Vec::new()));
    assert_eq!(cursor_count(&app), 1);

    let before = buffer_text(&app);
    let _ = app.run_headless_full_frame(frame(vec![egui::Event::Key {
        key: egui::Key::Escape,
        physical_key: Some(egui::Key::Escape),
        pressed: true,
        repeat: false,
        modifiers: egui::Modifiers::default(),
    }]));

    assert_eq!(cursor_count(&app), 1, "still one caret");
    assert_eq!(before, buffer_text(&app), "Escape must not edit the buffer");
}
