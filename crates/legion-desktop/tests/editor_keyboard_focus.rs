//! Real renderer pointer/keyboard events and the published accessibility tree.
use accesskit_consumer::Tree;
use legion_desktop::workflow::{DesktopEframeApp, DesktopLaunchConfig, DesktopRuntime};

fn frame(events: Vec<egui::Event>) -> egui::RawInput {
    egui::RawInput {
        focused: true,
        screen_rect: Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(1600.0, 1000.0),
        )),
        events,
        ..Default::default()
    }
}

fn document(output: &egui::FullOutput) -> (egui::accesskit::NodeId, egui::Rect) {
    let update = output.platform_output.accesskit_update.as_ref().unwrap();
    let docs: Vec<_> = update
        .nodes
        .iter()
        .filter(|(_, node)| node.label() == Some("Editor document"))
        .collect();
    assert_eq!(docs.len(), 1);
    let (id, node) = docs[0];
    let rect = node.bounds().unwrap();
    (
        *id,
        egui::Rect::from_min_max(
            egui::pos2(rect.x0 as f32, rect.y0 as f32),
            egui::pos2(rect.x1 as f32, rect.y1 as f32),
        ),
    )
}

fn click(app: &mut DesktopEframeApp, pos: egui::Pos2) -> egui::FullOutput {
    app.run_headless_full_frame(frame(vec![egui::Event::PointerMoved(pos)]));
    for pressed in [true, false] {
        app.run_headless_full_frame(frame(vec![
            egui::Event::PointerMoved(pos),
            egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed,
                modifiers: egui::Modifiers::NONE,
            },
        ]));
    }
    app.run_headless_full_frame(frame(vec![]))
}

fn key(key: egui::Key, modifiers: egui::Modifiers) -> egui::Event {
    egui::Event::Key {
        key,
        physical_key: Some(key),
        pressed: true,
        repeat: false,
        modifiers,
    }
}

fn assert_editor_focus(output: &egui::FullOutput, id: egui::accesskit::NodeId) {
    let update = output.platform_output.accesskit_update.as_ref().unwrap();
    assert_eq!(
        update.focus, id,
        "the actual editor document owns accessible keyboard focus"
    );
    let tree = Tree::new(update.clone(), true);
    let editor = tree
        .state()
        .node_by_tree_local_id(id, egui::accesskit::TreeId::ROOT)
        .unwrap();
    assert!(editor.is_focused());
    assert!(editor.is_focusable(&|_| accesskit_consumer::FilterResult::Include));
    assert!(
        !update
            .nodes
            .iter()
            .find(|(candidate, _)| *candidate == id)
            .unwrap()
            .1
            .supports_action(egui::accesskit::Action::Focus)
    );
}

#[test]
fn real_editor_background_click_owns_focus_and_keeps_text_home_and_save_routing() {
    let workspace = tempfile::tempdir().unwrap();
    let path = workspace.path().join("note.txt");
    std::fs::write(&path, "seed\nsecond").unwrap();
    let runtime = DesktopRuntime::open(DesktopLaunchConfig::new(
        workspace.path().to_path_buf(),
        Some(path.to_string_lossy().into_owned()),
    ))
    .unwrap();
    let mut app = DesktopEframeApp::new(runtime);
    let primed = app.run_headless_full_frame(frame(vec![]));
    let (id, rect) = document(&primed);
    let focused = click(&mut app, rect.center());
    assert_editor_focus(&focused, id);
    assert!(
        !app.headless_egui_context().text_edit_focused(),
        "the canvas is not an egui TextEdit buffer"
    );
    let ctrl = egui::Modifiers {
        ctrl: true,
        command: true,
        ..Default::default()
    };
    let home = app.run_headless_full_frame(frame(vec![key(egui::Key::Home, ctrl)]));
    assert_editor_focus(&home, id);
    let typed = app.run_headless_full_frame(frame(vec![egui::Event::Text("!".into())]));
    assert_editor_focus(&typed, id);
    assert_eq!(
        app.runtime_snapshot()
            .active_buffer_projection
            .small_buffer_text(),
        Some("!seed\nsecond")
    );
    let saved = app.run_headless_full_frame(frame(vec![key(egui::Key::S, ctrl)]));
    assert_editor_focus(&saved, id);
    assert!(!app.runtime_snapshot().active_buffer_projection.dirty);
    assert_eq!(std::fs::read_to_string(path).unwrap(), "!seed\nsecond");
}

#[test]
fn opened_file_owns_initial_keyboard_focus_without_a_pointer_or_at_request() {
    let workspace = tempfile::tempdir().unwrap();
    let path = workspace.path().join("note.txt");
    std::fs::write(&path, "seed").unwrap();
    let runtime = DesktopRuntime::open(DesktopLaunchConfig::new(
        workspace.path().to_path_buf(),
        Some(path.to_string_lossy().into_owned()),
    ))
    .unwrap();
    let mut app = DesktopEframeApp::new(runtime);
    let opened = app.run_headless_full_frame(frame(vec![]));
    let (id, _) = document(&opened);
    assert_editor_focus(&opened, id);
    let stable = app.run_headless_full_frame(frame(vec![]));
    assert_editor_focus(&stable, id);
}

fn labelled_rect(output: &egui::FullOutput, label: &str, region: egui::Rect) -> egui::Rect {
    let nodes: Vec<_> = output
        .platform_output
        .accesskit_update
        .as_ref()
        .unwrap()
        .nodes
        .iter()
        .filter(|(_, node)| node.label() == Some(label) || node.value() == Some(label))
        .filter(|(_, node)| node.supports_action(egui::accesskit::Action::Click))
        .filter_map(|(_, node)| node.bounds())
        .filter(|rect| {
            region.contains(egui::pos2(
                ((rect.x0 + rect.x1) / 2.0) as f32,
                ((rect.y0 + rect.y1) / 2.0) as f32,
            ))
        })
        .collect();
    assert_eq!(nodes.len(), 1, "unique rendered {label} target");
    let rect = nodes[0];
    egui::Rect::from_min_max(
        egui::pos2(rect.x0 as f32, rect.y0 as f32),
        egui::pos2(rect.x1 as f32, rect.y1 as f32),
    )
}

#[test]
fn text_line_click_takes_focus_back_from_palette_and_enter_space_keep_editing() {
    let workspace = tempfile::tempdir().unwrap();
    let path = workspace.path().join("note.txt");
    std::fs::write(&path, "seed\nsecond").unwrap();
    let runtime = DesktopRuntime::open(DesktopLaunchConfig::new(
        workspace.path().to_path_buf(),
        Some(path.to_string_lossy().into_owned()),
    ))
    .unwrap();
    let mut app = DesktopEframeApp::new(runtime);
    let primed = app.run_headless_full_frame(frame(vec![]));
    let (id, code) = document(&primed);
    let row = labelled_rect(&primed, "seed", code);
    let ctrl = egui::Modifiers {
        ctrl: true,
        command: true,
        ..Default::default()
    };
    app.run_headless_full_frame(frame(vec![key(egui::Key::P, ctrl)]));
    app.run_headless_full_frame(frame(vec![]));
    assert!(app.headless_egui_context().text_edit_focused());
    app.run_headless_full_frame(frame(vec![key(egui::Key::Escape, egui::Modifiers::NONE)]));
    app.run_headless_full_frame(frame(vec![]));
    let focused = click(&mut app, row.center());
    assert_editor_focus(&focused, id);
    app.run_headless_full_frame(frame(vec![key(egui::Key::Home, ctrl)]));
    app.run_headless_full_frame(frame(vec![egui::Event::Text("X".into())]));
    app.run_headless_full_frame(frame(vec![key(egui::Key::Enter, egui::Modifiers::NONE)]));
    let space = app.run_headless_full_frame(frame(vec![
        key(egui::Key::Space, egui::Modifiers::NONE),
        egui::Event::Text(" ".into()),
    ]));
    assert_editor_focus(&space, id);
    assert_eq!(
        app.runtime_snapshot()
            .active_buffer_projection
            .small_buffer_text(),
        Some("X\n seed\nsecond")
    );
}

#[test]
fn editor_arrow_moves_the_caret_without_spatial_widget_focus_navigation() {
    let workspace = tempfile::tempdir().unwrap();
    let path = workspace.path().join("note.txt");
    std::fs::write(&path, "seed\nsecond").unwrap();
    let runtime = DesktopRuntime::open(DesktopLaunchConfig::new(
        workspace.path().to_path_buf(),
        Some(path.to_string_lossy().into_owned()),
    ))
    .unwrap();
    let mut app = DesktopEframeApp::new(runtime);
    let primed = app.run_headless_full_frame(frame(vec![]));
    let (id, _) = document(&primed);
    app.run_headless_full_frame(frame(vec![]));
    let cursor = app
        .runtime_snapshot()
        .active_buffer_projection
        .viewport
        .unwrap()
        .cursor;
    assert_eq!((cursor.line, cursor.character), (0, 0));
    let moved = app.run_headless_full_frame(frame(vec![key(
        egui::Key::ArrowRight,
        egui::Modifiers::NONE,
    )]));
    let cursor = app
        .runtime_snapshot()
        .active_buffer_projection
        .viewport
        .unwrap()
        .cursor;
    assert_eq!((cursor.line, cursor.character), (0, 1));
    assert_editor_focus(&moved, id);
    let following = app.run_headless_full_frame(frame(vec![]));
    assert_editor_focus(&following, id);
    for (arrow, expected) in [
        (egui::Key::ArrowLeft, (0, 0)),
        (egui::Key::ArrowDown, (1, 0)),
        (egui::Key::ArrowUp, (0, 0)),
    ] {
        let moved = app.run_headless_full_frame(frame(vec![key(arrow, egui::Modifiers::NONE)]));
        let cursor = app
            .runtime_snapshot()
            .active_buffer_projection
            .viewport
            .unwrap()
            .cursor;
        assert_eq!((cursor.line, cursor.character), expected);
        assert_editor_focus(&moved, id);
        assert_editor_focus(&app.run_headless_full_frame(frame(vec![])), id);
    }
}

#[test]
fn first_arrow_immediately_after_file_activation_keeps_document_focus() {
    let workspace = tempfile::tempdir().unwrap();
    let path = workspace.path().join("note.txt");
    std::fs::write(&path, "seed\nsecond").unwrap();
    let runtime = DesktopRuntime::open(DesktopLaunchConfig::new(
        workspace.path().to_path_buf(),
        Some(path.to_string_lossy().into_owned()),
    ))
    .unwrap();
    let mut app = DesktopEframeApp::new(runtime);
    let opened = app.run_headless_full_frame(frame(vec![]));
    let (id, _) = document(&opened);
    let moved = app.run_headless_full_frame(frame(vec![key(
        egui::Key::ArrowDown,
        egui::Modifiers::NONE,
    )]));
    let cursor = app
        .runtime_snapshot()
        .active_buffer_projection
        .viewport
        .unwrap()
        .cursor;
    assert_eq!((cursor.line, cursor.character), (1, 0));
    assert_editor_focus(&moved, id);
    assert_editor_focus(&app.run_headless_full_frame(frame(vec![])), id);
}

#[test]
fn palette_owns_text_input_and_file_activation_does_not_steal_its_focus() {
    use legion_desktop::bridge::DesktopAction;
    let workspace = tempfile::tempdir().unwrap();
    let path = workspace.path().join("note.txt");
    let second = workspace.path().join("second.txt");
    std::fs::write(&path, "seed").unwrap();
    std::fs::write(&second, "second").unwrap();
    let runtime = DesktopRuntime::open(DesktopLaunchConfig::new(
        workspace.path().to_path_buf(),
        Some(path.to_string_lossy().into_owned()),
    ))
    .unwrap();
    let mut app = DesktopEframeApp::new(runtime);
    let primed = app.run_headless_full_frame(frame(vec![]));
    let (doc_id, _) = document(&primed);
    let ctrl = egui::Modifiers {
        ctrl: true,
        command: true,
        ..Default::default()
    };
    app.run_headless_full_frame(frame(vec![key(egui::Key::P, ctrl)]));
    let palette = app.run_headless_full_frame(frame(vec![]));
    let palette_id = palette.platform_output.accesskit_update.unwrap().focus;
    assert_ne!(palette_id, doc_id);
    assert!(app.headless_egui_context().text_edit_focused());
    let typed = app.run_headless_full_frame(frame(vec![egui::Event::Text("needle".into())]));
    assert_eq!(
        typed.platform_output.accesskit_update.unwrap().focus,
        palette_id
    );
    assert_eq!(app.runtime_snapshot().palette_projection.query, "needle");
    assert_eq!(
        app.runtime_snapshot()
            .active_buffer_projection
            .small_buffer_text(),
        Some("seed")
    );
    assert!(!app.runtime_snapshot().active_buffer_projection.dirty);
    app.handle_action(DesktopAction::OpenPathText(
        second.to_string_lossy().into_owned(),
    ))
    .unwrap();
    let activated = app.run_headless_full_frame(frame(vec![]));
    assert_eq!(
        activated.platform_output.accesskit_update.unwrap().focus,
        palette_id
    );
    let typed = app.run_headless_full_frame(frame(vec![egui::Event::Text("!".into())]));
    assert_eq!(
        typed.platform_output.accesskit_update.unwrap().focus,
        palette_id
    );
    assert_eq!(app.runtime_snapshot().palette_projection.query, "needle!");
    assert_eq!(
        app.runtime_snapshot()
            .active_buffer_projection
            .small_buffer_text(),
        Some("second")
    );
    assert!(!app.runtime_snapshot().active_buffer_projection.dirty);
}

#[test]
fn real_explorer_open_then_editor_click_publishes_document_focus_and_routes_typing() {
    let workspace = tempfile::tempdir().unwrap();
    let path = workspace.path().join("note.txt");
    std::fs::write(&path, "seed").unwrap();
    let runtime = DesktopRuntime::open(DesktopLaunchConfig::new(
        workspace.path().to_path_buf(),
        None,
    ))
    .unwrap();
    let mut app = DesktopEframeApp::new(runtime);
    let primed = app.run_headless_full_frame(frame(vec![]));
    assert!(
        app.runtime_snapshot()
            .active_buffer_projection
            .buffer_id
            .is_none()
    );
    let explorer = labelled_rect(
        &primed,
        "note.txt",
        egui::Rect::from_min_max(egui::Pos2::ZERO, egui::pos2(400.0, 1000.0)),
    );
    let opened = click(&mut app, explorer.center());
    assert_eq!(
        app.runtime_snapshot()
            .active_buffer_projection
            .small_buffer_text(),
        Some("seed")
    );
    let (id, rect) = document(&opened);
    let focused = click(&mut app, rect.center());
    assert_editor_focus(&focused, id);
    let ctrl = egui::Modifiers {
        ctrl: true,
        command: true,
        ..Default::default()
    };
    app.run_headless_full_frame(frame(vec![key(egui::Key::Home, ctrl)]));
    let typed = app.run_headless_full_frame(frame(vec![egui::Event::Text("!".into())]));
    assert_editor_focus(&typed, id);
    assert_eq!(
        app.runtime_snapshot()
            .active_buffer_projection
            .small_buffer_text(),
        Some("!seed")
    );
    assert_eq!(
        std::fs::read_to_string(path).unwrap(),
        "seed",
        "typing remains editor-owned until save"
    );
}

#[test]
fn compact_drawer_open_close_then_canvas_click_retains_document_focus() {
    fn compact_frame(app: &mut DesktopEframeApp, events: Vec<egui::Event>) -> egui::FullOutput {
        let mut input = frame(events);
        input.screen_rect = Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(960.0, 720.0),
        ));
        app.run_headless_full_frame(input)
    }
    fn compact_click(app: &mut DesktopEframeApp, pos: egui::Pos2) -> egui::FullOutput {
        compact_frame(app, vec![egui::Event::PointerMoved(pos)]);
        for pressed in [true, false] {
            compact_frame(
                app,
                vec![
                    egui::Event::PointerMoved(pos),
                    egui::Event::PointerButton {
                        pos,
                        button: egui::PointerButton::Primary,
                        pressed,
                        modifiers: egui::Modifiers::NONE,
                    },
                ],
            );
        }
        compact_frame(app, vec![])
    }
    let workspace = tempfile::tempdir().unwrap();
    let path = workspace.path().join("note.txt");
    std::fs::write(&path, "seed\nsecond").unwrap();
    let runtime = DesktopRuntime::open(DesktopLaunchConfig::new(
        workspace.path().to_path_buf(),
        None,
    ))
    .unwrap();
    let mut app = DesktopEframeApp::new(runtime);
    let bounds = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(960.0, 720.0));
    let initial = compact_frame(&mut app, vec![]);
    let toggle = labelled_rect(&initial, "Explorer drawer", bounds);
    let drawer = compact_click(&mut app, toggle.center());
    let file = labelled_rect(&drawer, "note.txt", bounds);
    let opened = compact_click(&mut app, file.center());
    let close = labelled_rect(&opened, "Close Explorer drawer", bounds);
    let closed = compact_click(&mut app, close.center());
    let (id, rect) = document(&closed);
    let focused = compact_click(&mut app, rect.center());
    assert_editor_focus(&focused, id);
    for _ in 0..3 {
        let idle = compact_frame(&mut app, vec![]);
        assert_editor_focus(&idle, id);
    }
}
