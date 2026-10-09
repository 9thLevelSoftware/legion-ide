//! Bounded renderer diagnostics, not OS input or native UIA qualification.
use accesskit_consumer::{FilterResult, Tree};
use legion_desktop::workflow::{DesktopEframeApp, DesktopLaunchConfig, DesktopRuntime};

fn input(events: Vec<egui::Event>) -> egui::RawInput {
    egui::RawInput {
        focused: true,
        screen_rect: Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(960.0, 720.0),
        )),
        events,
        ..Default::default()
    }
}

fn pointer(pos: egui::Pos2, pressed: bool) -> egui::Event {
    egui::Event::PointerButton {
        pos,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: egui::Modifiers::NONE,
    }
}

fn target(output: &egui::FullOutput, label: &str) -> (egui::accesskit::NodeId, egui::Rect) {
    scoped_target(output, label, egui::Rect::EVERYTHING)
}

fn scoped_target(
    output: &egui::FullOutput,
    label: &str,
    scope: egui::Rect,
) -> (egui::accesskit::NodeId, egui::Rect) {
    let matches: Vec<_> = output
        .platform_output
        .accesskit_update
        .as_ref()
        .expect("published tree")
        .nodes
        .iter()
        .filter(|(_, node)| node.label() == Some(label) || node.value() == Some(label))
        // A selectable egui Label also publishes a same-text TextRun child.
        // Target the actual clickable widget, not its text-range metadata.
        .filter(|(_, node)| {
            node.supports_action(egui::accesskit::Action::Click)
                || node.role() == egui::accesskit::Role::MultilineTextInput
        })
        .filter_map(|(id, node)| node.bounds().map(|bounds| (*id, bounds)))
        .filter(|(_, bounds)| {
            scope.contains(egui::pos2(
                ((bounds.x0 + bounds.x1) / 2.0) as f32,
                ((bounds.y0 + bounds.y1) / 2.0) as f32,
            ))
        })
        .collect();
    assert_eq!(matches.len(), 1, "unique rendered {label}");
    let (id, bounds) = matches[0];
    (
        id,
        egui::Rect::from_min_max(
            egui::pos2(bounds.x0 as f32, bounds.y0 as f32),
            egui::pos2(bounds.x1 as f32, bounds.y1 as f32),
        ),
    )
}

fn assert_document_focus(output: &egui::FullOutput, id: egui::accesskit::NodeId) {
    let update = output.platform_output.accesskit_update.as_ref().unwrap();
    assert_eq!(update.focus, id, "rendered document owns keyboard focus");
    let tree = Tree::new(update.clone(), true);
    let node = tree
        .state()
        .node_by_tree_local_id(id, egui::accesskit::TreeId::ROOT)
        .unwrap();
    assert!(node.is_focused());
    assert!(node.is_focusable(&|_| FilterResult::Include));
    assert_eq!(node.document_range().text(), "seed\nsecond");
    assert!(
        !update
            .nodes
            .iter()
            .find(|(node_id, _)| *node_id == id)
            .unwrap()
            .1
            .supports_action(egui::accesskit::Action::Focus)
    );
}

fn cursor(app: &DesktopEframeApp) -> (u32, u32) {
    let position = app
        .runtime_snapshot()
        .active_buffer_projection
        .viewport
        .unwrap()
        .cursor;
    (position.line, position.character)
}

fn batched_click(app: &mut DesktopEframeApp, pos: egui::Pos2) -> egui::FullOutput {
    app.run_headless_full_frame(input(vec![
        egui::Event::PointerMoved(pos),
        pointer(pos, true),
        pointer(pos, false),
    ]));
    app.run_headless_full_frame(input(vec![]))
}

#[test]
fn coalesced_pointer_clicks_open_file_close_drawer_and_move_editor_caret_with_focus() {
    let workspace = tempfile::tempdir().unwrap();
    let path = workspace.path().join("note.txt");
    std::fs::write(&path, "seed\nsecond").unwrap();
    let runtime =
        DesktopRuntime::open(DesktopLaunchConfig::new(workspace.path().into(), None)).unwrap();
    let mut app = DesktopEframeApp::new(runtime);
    let initial = app.run_headless_full_frame(input(vec![]));
    let (_, toggle) = target(&initial, "Explorer drawer");
    let drawer = batched_click(&mut app, toggle.center());
    let (_, file) = target(&drawer, "note.txt");
    let opened = batched_click(&mut app, file.center());
    assert_eq!(
        app.runtime_snapshot()
            .active_buffer_projection
            .small_buffer_text(),
        Some("seed\nsecond")
    );
    let (_, close) = target(&opened, "Close Explorer drawer");
    let closed = batched_click(&mut app, close.center());
    let (doc, rect) = target(&closed, "Editor document");
    let blank = batched_click(&mut app, rect.center());
    assert_document_focus(&blank, doc);
    assert_eq!(cursor(&app), (0, 0));
    let (_, line) = scoped_target(&blank, "second", rect);
    let text_click = batched_click(&mut app, egui::pos2(line.left() + 1.0, line.center().y));
    assert_eq!(
        cursor(&app),
        (1, 0),
        "caret movement independently proves text-click delivery"
    );
    assert_document_focus(&text_click, doc);
    for _ in 0..3 {
        assert_document_focus(&app.run_headless_full_frame(input(vec![])), doc);
    }
    assert_eq!(std::fs::read(&path).unwrap(), b"seed\nsecond");
}

fn eframe_frame(
    app: &mut DesktopEframeApp,
    ctx: &egui::Context,
    events: Vec<egui::Event>,
) -> egui::FullOutput {
    // The same public App::ui entry point eframe uses, with no OS window/GPU.
    let mut frame = eframe::Frame::_new_kittest();
    ctx.run_ui(input(events), |ui| eframe::App::ui(app, ui, &mut frame))
}

#[test]
fn late_accesskit_activation_on_external_eframe_context_keeps_document_focus_and_caret_input() {
    let workspace = tempfile::tempdir().unwrap();
    let path = workspace.path().join("note.txt");
    std::fs::write(&path, "seed\nsecond").unwrap();
    let runtime = DesktopRuntime::open(DesktopLaunchConfig::new(
        workspace.path().into(),
        Some(path.to_string_lossy().into_owned()),
    ))
    .unwrap();
    let mut app = DesktopEframeApp::new(runtime);
    let ctx = egui::Context::default();
    for _ in 0..3 {
        let before_activation = eframe_frame(&mut app, &ctx, vec![]);
        assert!(before_activation.platform_output.accesskit_update.is_none());
    }
    assert_eq!(cursor(&app), (0, 0));
    assert!(
        app.headless_egui_context()
            .memory(|memory| memory.focused().is_none()),
        "rendering must use the external context rather than the headless context"
    );
    ctx.enable_accesskit();
    let activated = eframe_frame(&mut app, &ctx, vec![]);
    let (doc, rect) = target(&activated, "Editor document");
    assert_document_focus(&activated, doc);
    let (_, line) = scoped_target(&activated, "second", rect);
    let pos = egui::pos2(line.left() + 1.0, line.center().y);
    eframe_frame(&mut app, &ctx, vec![egui::Event::PointerMoved(pos)]);
    eframe_frame(&mut app, &ctx, vec![pointer(pos, true)]);
    eframe_frame(&mut app, &ctx, vec![pointer(pos, false)]);
    let clicked = eframe_frame(&mut app, &ctx, vec![]);
    assert_eq!(
        cursor(&app),
        (1, 0),
        "caret movement proves text-click delivery"
    );
    assert_document_focus(&clicked, doc);
    for _ in 0..3 {
        assert_document_focus(&eframe_frame(&mut app, &ctx, vec![]), doc);
    }
    eframe_frame(&mut app, &ctx, vec![egui::Event::Text("!".into())]);
    let typed = eframe_frame(&mut app, &ctx, vec![]);
    assert_eq!(
        app.runtime_snapshot()
            .active_buffer_projection
            .small_buffer_text(),
        Some("seed\n!second")
    );
    assert!(app.runtime_snapshot().active_buffer_projection.dirty);
    assert_eq!(
        typed
            .platform_output
            .accesskit_update
            .as_ref()
            .unwrap()
            .focus,
        doc
    );
    assert_eq!(
        std::fs::read(&path).unwrap(),
        b"seed\nsecond",
        "no save or OS input occurred"
    );
}
