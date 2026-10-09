//! Product projection -> real rendered AccessKit text-range contracts.
//! No native window or OS input is used by these tests.

use accesskit_consumer::Tree;
use legion_app::AppComposition;
use legion_desktop::{
    view::ProjectionView,
    workflow::{DesktopLaunchConfig, DesktopRuntime},
};
use legion_protocol::{PrincipalId, TextCoordinate, ViewportScroll, WorkspaceTrustState};
use legion_ui::CommandDispatchIntent;
use legion_ui::ShellProjectionSnapshot;

fn open_app(text: &str) -> (tempfile::TempDir, AppComposition) {
    let workspace = tempfile::tempdir().unwrap();
    let path = workspace.path().join("note.txt");
    std::fs::write(&path, text).unwrap();
    let mut app = AppComposition::new();
    app.open_workspace(
        workspace.path(),
        WorkspaceTrustState::Untrusted,
        PrincipalId("a11y".into()),
    )
    .unwrap();
    app.open_file(path.to_string_lossy()).unwrap();
    (workspace, app)
}

#[derive(Default)]
struct Changes;
impl accesskit_consumer::TreeChangeHandler for Changes {
    fn node_added(&mut self, _: &accesskit_consumer::Node) {}
    fn node_updated(&mut self, _: &accesskit_consumer::Node, _: &accesskit_consumer::Node) {}
    fn focus_moved(
        &mut self,
        _: Option<&accesskit_consumer::Node>,
        _: Option<&accesskit_consumer::Node>,
    ) {
    }
    fn node_removed(&mut self, _: &accesskit_consumer::Node) {}
}

fn current_text(tree: &Tree, id: egui::accesskit::NodeId) -> String {
    tree.state()
        .node_by_tree_local_id(id, egui::accesskit::TreeId::ROOT)
        .unwrap()
        .document_range()
        .text()
}

fn render(
    ctx: &egui::Context,
    view: &mut ProjectionView,
    snapshot: &ShellProjectionSnapshot,
) -> egui::accesskit::TreeUpdate {
    ctx.run_ui(
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(1200.0, 600.0),
            )),
            focused: true,
            ..Default::default()
        },
        |ui| {
            view.render(ui, snapshot);
        },
    )
    .platform_output
    .accesskit_update
    .expect("accessibility enabled")
}

fn document(update: &egui::accesskit::TreeUpdate) -> egui::accesskit::NodeId {
    let documents: Vec<_> = update
        .nodes
        .iter()
        .filter(|(_, node)| {
            node.role() == egui::accesskit::Role::MultilineTextInput
                && node.label() == Some("Editor document")
        })
        .collect();
    assert_eq!(documents.len(), 1, "one actual complete editor document");
    documents[0].0
}

fn document_text(update: egui::accesskit::TreeUpdate) -> String {
    let id = document(&update);
    let tree = Tree::new(update, true);
    let node = tree
        .state()
        .node_by_tree_local_id(id, egui::accesskit::TreeId::ROOT)
        .expect("document belongs to tree");
    assert!(node.supports_text_ranges());
    assert!(!node.is_read_only(), "real editable buffer");
    node.document_range().text()
}

#[test]
fn complete_document_preserves_unicode_newlines_and_offscreen_text() {
    let workspace = tempfile::tempdir().unwrap();
    let path = workspace.path().join("note.txt");
    let text = concat!(
        "a\u{301} 🦀 👩\u{200d}💻\r\n\n",
        "01\n02\n03\n04\n05\n06\n07\n08\n09\n10\n",
        "11\n12\n13\n14\n15\n16\n17\n18\n19\n20\n",
        "21\n22\n23\n24\n25\n26\n27\n28\n29\n30\n",
        "offscreen tail\r\n"
    );
    std::fs::write(&path, text).unwrap();
    let runtime = DesktopRuntime::open(DesktopLaunchConfig::new(
        workspace.path().to_path_buf(),
        Some(path.to_string_lossy().into_owned()),
    ))
    .unwrap();
    let snapshot = runtime.projection_snapshot();
    let ctx = egui::Context::default();
    ctx.enable_accesskit();
    let mut view = ProjectionView::new();
    assert_eq!(document_text(render(&ctx, &mut view, &snapshot)), text);
}

#[test]
fn selection_preserves_directed_unicode_endpoints_without_advertising_actions() {
    use legion_app::AppComposition;
    use legion_protocol::{PrincipalId, TextCoordinate, WorkspaceTrustState};
    use legion_ui::CommandDispatchIntent;

    let workspace = tempfile::tempdir().unwrap();
    let path = workspace.path().join("selection.txt");
    std::fs::write(&path, "a\u{301}🦀\r\nend").unwrap();
    let mut app = AppComposition::new();
    app.open_workspace(
        workspace.path(),
        WorkspaceTrustState::Untrusted,
        PrincipalId("a11y".into()),
    )
    .unwrap();
    app.open_file(path.to_string_lossy()).unwrap();
    let buffer_id = app.active_buffer_id().unwrap();
    app.dispatch_ui_intent(CommandDispatchIntent::SetDirectedSelection {
        buffer_id,
        anchor: TextCoordinate {
            line: 0,
            character: 7,
            byte_offset: Some(7),
            utf16_offset: Some(4),
        },
        head: TextCoordinate {
            line: 0,
            character: 0,
            byte_offset: Some(0),
            utf16_offset: Some(0),
        },
    })
    .unwrap();
    let snapshot = app.shell_projection_snapshot("a11y").unwrap();
    let ctx = egui::Context::default();
    ctx.enable_accesskit();
    let update = render(&ctx, &mut ProjectionView::new(), &snapshot);
    let id = document(&update);
    let raw = &update
        .nodes
        .iter()
        .find(|(node_id, _)| *node_id == id)
        .unwrap()
        .1;
    let selection = raw
        .text_selection()
        .expect("primary directed editor caret is published");
    assert_eq!(
        selection.anchor.character_index, 2,
        "combining sequence and crab are each selectable units"
    );
    assert_eq!(
        selection.focus.character_index, 0,
        "reverse selection keeps head at start"
    );
    assert!(!raw.supports_action(egui::accesskit::Action::Focus));
    assert!(!raw.supports_action(egui::accesskit::Action::SetTextSelection));
    let tree = Tree::new(update, true);
    let node = tree
        .state()
        .node_by_tree_local_id(id, egui::accesskit::TreeId::ROOT)
        .unwrap();
    assert_eq!(node.text_selection().unwrap().text(), "a\u{301}🦀");
}

#[test]
fn qualification_empty_document_has_real_empty_range_and_caret() {
    let (_workspace, app) = open_app("");
    let ctx = egui::Context::default();
    ctx.enable_accesskit();
    let update = render(
        &ctx,
        &mut ProjectionView::new(),
        &app.shell_projection_snapshot("a11y").unwrap(),
    );
    let id = document(&update);
    let tree = Tree::new(update, true);
    let node = tree
        .state()
        .node_by_tree_local_id(id, egui::accesskit::TreeId::ROOT)
        .unwrap();
    assert!(node.supports_text_ranges());
    assert_eq!(node.document_range().text(), "");
    assert_eq!(node.text_selection().unwrap().text(), "");
}

#[test]
fn qualification_stable_document_updates_after_edits_and_removes_closed_tab_runs() {
    let (workspace, mut app) = open_app("alpha\nbeta\ngamma\ndelta\n");
    let buffer_id = app.active_buffer_id().unwrap();
    let before = app.shell_projection_snapshot("a11y").unwrap();
    let identity = before.active_buffer_projection.accessibility.unwrap();
    let ctx = egui::Context::default();
    ctx.enable_accesskit();
    let mut view = ProjectionView::new();
    let update = render(&ctx, &mut view, &before);
    let id = document(&update);
    let last_run = *update
        .nodes
        .iter()
        .find(|(candidate, _)| *candidate == id)
        .unwrap()
        .1
        .children()
        .last()
        .unwrap();
    let mut tree = Tree::new(update, true);
    app.dispatch_ui_intent(CommandDispatchIntent::Insert {
        buffer_id,
        at: TextCoordinate {
            line: 0,
            character: 0,
            byte_offset: Some(0),
            utf16_offset: Some(0),
        },
        text: "!".into(),
    })
    .unwrap();
    let edited = app.shell_projection_snapshot("a11y").unwrap();
    assert!(edited.active_buffer_projection.dirty);
    let edited_identity = edited.active_buffer_projection.accessibility.unwrap();
    assert!(
        edited_identity.editable,
        "editing remains allowed in an untrusted workspace"
    );
    assert_ne!(identity.snapshot_id, edited_identity.snapshot_id);
    assert_ne!(identity.buffer_version, edited_identity.buffer_version);
    let update = render(&ctx, &mut view, &edited);
    assert_eq!(
        document(&update),
        id,
        "native retained control remains valid after typing"
    );
    tree.update_and_process_changes(update, &mut Changes);
    assert_eq!(current_text(&tree, id), "!alpha\nbeta\ngamma\ndelta\n");
    app.dispatch_ui_intent(CommandDispatchIntent::Undo { buffer_id })
        .unwrap();
    let second_path = workspace.path().join("empty.txt");
    std::fs::write(&second_path, "").unwrap();
    app.open_file(second_path.to_string_lossy()).unwrap();
    let second_id = app.active_buffer_id().unwrap();
    let update = render(
        &ctx,
        &mut view,
        &app.shell_projection_snapshot("a11y").unwrap(),
    );
    assert_eq!(document(&update), id);
    tree.update_and_process_changes(update, &mut Changes);
    assert_eq!(current_text(&tree, id), "");
    assert!(
        tree.state()
            .node_by_tree_local_id(last_run, egui::accesskit::TreeId::ROOT)
            .is_none(),
        "old tab's trailing runs are removed"
    );
    app.dispatch_ui_intent(CommandDispatchIntent::CloseTab {
        buffer_id: second_id,
    })
    .unwrap();
    let update = render(
        &ctx,
        &mut view,
        &app.shell_projection_snapshot("a11y").unwrap(),
    );
    tree.update_and_process_changes(update, &mut Changes);
    assert_eq!(current_text(&tree, id), "alpha\nbeta\ngamma\ndelta\n");
    app.dispatch_ui_intent(CommandDispatchIntent::CloseTab { buffer_id })
        .unwrap();
    // Undo restores the source, but does not mark it saved. The public close
    // route correctly prompts instead of discarding a dirty editor buffer.
    let pending_close = app.shell_projection_snapshot("a11y").unwrap();
    assert!(
        pending_close
            .daily_editing_projection
            .close_dirty_prompt
            .is_some()
    );
    let update = render(&ctx, &mut view, &pending_close);
    tree.update_and_process_changes(update, &mut Changes);
    assert_eq!(current_text(&tree, id), "alpha\nbeta\ngamma\ndelta\n");
    // Close the last clean buffer through the same public route, reusing the
    // renderer/consumer so stale nodes cannot survive a no-active-buffer frame.
    let (_clean_workspace, mut app) = open_app("last clean tab");
    let update = render(
        &ctx,
        &mut view,
        &app.shell_projection_snapshot("a11y").unwrap(),
    );
    tree.update_and_process_changes(update, &mut Changes);
    app.dispatch_ui_intent(CommandDispatchIntent::CloseTab {
        buffer_id: app.active_buffer_id().unwrap(),
    })
    .unwrap();
    let update = render(
        &ctx,
        &mut view,
        &app.shell_projection_snapshot("a11y").unwrap(),
    );
    tree.update_and_process_changes(update, &mut Changes);
    assert!(
        tree.state()
            .node_by_tree_local_id(id, egui::accesskit::TreeId::ROOT)
            .is_none(),
        "last closed tab removes the document control"
    );
}

#[test]
fn qualification_scrolling_preserves_complete_text_and_viewport_bounds() {
    let text = "row\n".repeat(100);
    let (_workspace, mut app) = open_app(&text);
    let ctx = egui::Context::default();
    ctx.enable_accesskit();
    let mut view = ProjectionView::new();
    let snapshot = app.shell_projection_snapshot("a11y").unwrap();
    let before = render(&ctx, &mut view, &snapshot);
    let id = document(&before);
    assert_eq!(document_text(before), text);
    app.dispatch_ui_intent(CommandDispatchIntent::SetViewportScroll {
        buffer_id: app.active_buffer_id().unwrap(),
        scroll: ViewportScroll {
            top_line: 80,
            left_column: 0,
        },
    })
    .unwrap();
    let update = render(
        &ctx,
        &mut view,
        &app.shell_projection_snapshot("a11y").unwrap(),
    );
    assert_eq!(document(&update), id);
    let raw = &update
        .nodes
        .iter()
        .find(|(candidate, _)| *candidate == id)
        .unwrap()
        .1;
    let bounds = raw.bounds().unwrap();
    assert!(bounds.x1 > bounds.x0 && bounds.y1 > bounds.y0);
    assert!(bounds.x0 >= 0.0 && bounds.y0 >= 0.0 && bounds.x1 <= 1200.0 && bounds.y1 <= 600.0);
    assert!(!raw.is_hidden() && !raw.is_disabled());
    assert_eq!(document_text(update), text);
}

fn assert_constrained(update: egui::accesskit::TreeUpdate, reason: &str) {
    let controls: Vec<_> = update
        .nodes
        .iter()
        .filter(|(_, node)| node.label() == Some("Editor document"))
        .collect();
    assert_eq!(controls.len(), 1);
    let (id, raw) = controls[0];
    assert_eq!(raw.role(), egui::accesskit::Role::Group);
    assert!(
        raw.description().unwrap().contains(reason),
        "{:?}",
        raw.description()
    );
    assert!(
        raw.children().is_empty(),
        "constrained editor never publishes a fragment as a document"
    );
    let tree = Tree::new(update.clone(), true);
    assert!(
        !tree
            .state()
            .node_by_tree_local_id(*id, egui::accesskit::TreeId::ROOT)
            .unwrap()
            .supports_text_ranges()
    );
}

#[test]
fn qualification_large_degraded_document_omits_complete_text() {
    use legion_ui::ui::EditorAccessibilityCoverage;
    let (_workspace, app) = open_app(&"x".repeat(5 * 1024 * 1024 + 1));
    let snapshot = app.shell_projection_snapshot("a11y").unwrap();
    let active = &snapshot.active_buffer_projection;
    assert!(active.degraded);
    assert!(active.small_buffer_text().is_none());
    assert_eq!(
        active.accessibility.unwrap().coverage,
        EditorAccessibilityCoverage::Degraded
    );
    let ctx = egui::Context::default();
    ctx.enable_accesskit();
    assert_constrained(
        render(&ctx, &mut ProjectionView::new(), &snapshot),
        "degraded",
    );
}

#[test]
fn qualification_budgets_and_invalid_projection_never_publish_partial_documents() {
    for (text, reason) in [
        ("x".repeat(1024 * 1024 + 1), "text byte budget"),
        ("x".repeat(65_537), "character metadata budget"),
        ("\n".repeat(1_024), "text run node budget"),
        (
            format!("a{}", "\u{301}".repeat(128)),
            "character-length representation",
        ),
    ] {
        let (_workspace, app) = open_app(&text);
        let snapshot = app.shell_projection_snapshot("a11y").unwrap();
        let ctx = egui::Context::default();
        ctx.enable_accesskit();
        assert_constrained(render(&ctx, &mut ProjectionView::new(), &snapshot), reason);
    }
    let (_workspace, app) = open_app("whole\r\n");
    let snapshot = app.shell_projection_snapshot("a11y").unwrap();
    let ctx = egui::Context::default();
    ctx.enable_accesskit();
    let mut view = ProjectionView::new();
    let mut missing = snapshot.clone();
    missing.active_buffer_projection.small_buffer_preview = None;
    assert_constrained(
        render(&ctx, &mut view, &missing),
        "exact preview unavailable",
    );
    let mut stale = snapshot.clone();
    stale
        .active_buffer_projection
        .accessibility
        .as_mut()
        .unwrap()
        .buffer_version
        .0 += 1;
    assert_constrained(render(&ctx, &mut view, &stale), "identity");
    let mut incomplete = snapshot;
    incomplete.active_buffer_projection.small_buffer_preview = Some("whole".into());
    assert_constrained(render(&ctx, &mut view, &incomplete), "byte length mismatch");
}

#[test]
fn qualification_unsolicited_accessibility_focus_preserves_enter_and_space_input() {
    use legion_desktop::workflow::DesktopEframeApp;
    let workspace = tempfile::tempdir().unwrap();
    let path = workspace.path().join("input.txt");
    std::fs::write(&path, "seed").unwrap();
    let runtime = DesktopRuntime::open(DesktopLaunchConfig::new(
        workspace.path().to_path_buf(),
        Some(path.to_string_lossy().into_owned()),
    ))
    .unwrap();
    let mut app = DesktopEframeApp::new(runtime);
    let frame = |events| egui::RawInput {
        focused: true,
        screen_rect: Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(1200.0, 600.0),
        )),
        events,
        ..Default::default()
    };
    let update = app
        .run_headless_full_frame(frame(vec![]))
        .platform_output
        .accesskit_update
        .unwrap();
    let id = document(&update);
    app.run_headless_full_frame(frame(vec![egui::Event::AccessKitActionRequest(
        egui::accesskit::ActionRequest {
            action: egui::accesskit::Action::Focus,
            target_tree: egui::accesskit::TreeId::ROOT,
            target_node: id,
            data: None,
        },
    )]));
    assert_eq!(
        app.headless_egui_context()
            .memory(|memory| memory.focused().map(|id| id.accesskit_id())),
        Some(id),
        "file-open keyboard ownership survives an unsolicited same-node Focus"
    );
    let key = |key| egui::Event::Key {
        key,
        physical_key: Some(key),
        pressed: true,
        repeat: false,
        modifiers: egui::Modifiers::NONE,
    };
    app.run_headless_full_frame(frame(vec![key(egui::Key::Enter)]));
    app.run_headless_full_frame(frame(vec![
        key(egui::Key::Space),
        egui::Event::Text(" ".into()),
    ]));
    assert_eq!(
        app.runtime_snapshot()
            .active_buffer_projection
            .small_buffer_text(),
        Some("\n seed")
    );
}

#[test]
fn edge_representable_graphemes_and_selection_crossing_run_boundaries_remain_exact() {
    let max_unit = format!("a{}", "\u{301}".repeat(127)); // 255 UTF-8 bytes, one grapheme.
    let (_workspace, app) = open_app(&max_unit);
    let ctx = egui::Context::default();
    ctx.enable_accesskit();
    let mut view = ProjectionView::new();
    assert_eq!(
        document_text(render(
            &ctx,
            &mut view,
            &app.shell_projection_snapshot("a11y").unwrap()
        )),
        max_unit
    );
    let text = format!("{}\r\nend", "🦀".repeat(300));
    let (_workspace, mut app) = open_app(&text);
    let buffer_id = app.active_buffer_id().unwrap();
    app.dispatch_ui_intent(CommandDispatchIntent::SetDirectedSelection {
        buffer_id,
        anchor: TextCoordinate {
            line: 0,
            character: 1016,
            byte_offset: Some(1016),
            utf16_offset: Some(508),
        },
        head: TextCoordinate {
            line: 0,
            character: 1024,
            byte_offset: Some(1024),
            utf16_offset: Some(512),
        },
    })
    .unwrap();
    let update = render(
        &ctx,
        &mut view,
        &app.shell_projection_snapshot("a11y").unwrap(),
    );
    let id = document(&update);
    let tree = Tree::new(update, true);
    let node = tree
        .state()
        .node_by_tree_local_id(id, egui::accesskit::TreeId::ROOT)
        .unwrap();
    assert_eq!(node.document_range().text(), text);
    assert_eq!(node.text_selection().unwrap().text(), "🦀🦀");
    app.dispatch_ui_intent(CommandDispatchIntent::SetCursor {
        buffer_id,
        cursor: TextCoordinate {
            line: 1,
            character: 0,
            byte_offset: Some(1202),
            utf16_offset: Some(602),
        },
    })
    .unwrap();
    let update = render(
        &ctx,
        &mut view,
        &app.shell_projection_snapshot("a11y").unwrap(),
    );
    let raw = &update
        .nodes
        .iter()
        .find(|(candidate, _)| *candidate == id)
        .unwrap()
        .1;
    let caret = raw.text_selection().unwrap();
    assert_eq!(caret.anchor, caret.focus);
    assert_eq!(
        caret.focus.character_index, 0,
        "after CRLF belongs to the next paragraph"
    );
}

#[test]
fn edge_every_projected_identity_component_and_coverage_is_required() {
    let (_workspace, app) = open_app("whole\n");
    let snapshot = app.shell_projection_snapshot("a11y").unwrap();
    let ctx = egui::Context::default();
    ctx.enable_accesskit();
    let mut view = ProjectionView::new();
    for component in ["buffer", "snapshot", "version", "mode"] {
        let mut stale = snapshot.clone();
        let active = &mut stale.active_buffer_projection;
        match component {
            "buffer" => active.accessibility.as_mut().unwrap().buffer_id.0 += 1,
            "snapshot" => active.accessibility.as_mut().unwrap().snapshot_id.0 += 1,
            "version" => active.accessibility.as_mut().unwrap().buffer_version.0 += 1,
            _ => {
                active.viewport.as_mut().unwrap().mode =
                    legion_protocol::ViewportProjectionMode::DegradedLargeFile
            }
        }
        assert_constrained(render(&ctx, &mut view, &stale), "identity or mode mismatch");
    }
    let mut no_metadata = snapshot.clone();
    no_metadata.active_buffer_projection.accessibility = None;
    assert_constrained(
        render(&ctx, &mut view, &no_metadata),
        "metadata unavailable",
    );
    let mut degraded_with_preview = snapshot;
    degraded_with_preview.active_buffer_projection.degraded = true;
    assert_constrained(
        render(&ctx, &mut view, &degraded_with_preview),
        "degraded buffer",
    );
}

#[test]
fn review_unsolicited_document_focus_does_not_reclassify_retained_rail_focus() {
    use legion_desktop::workflow::DesktopEframeApp;
    let workspace = tempfile::tempdir().unwrap();
    let path = workspace.path().join("input.txt");
    std::fs::write(&path, "seed").unwrap();
    let runtime = DesktopRuntime::open(DesktopLaunchConfig::new(
        workspace.path().to_path_buf(),
        Some(path.to_string_lossy().into_owned()),
    ))
    .unwrap();
    let mut app = DesktopEframeApp::new(runtime);
    let frame = |events| egui::RawInput {
        focused: true,
        screen_rect: Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(1200.0, 600.0),
        )),
        events,
        ..Default::default()
    };
    let update = app
        .run_headless_full_frame(frame(vec![]))
        .platform_output
        .accesskit_update
        .unwrap();
    let doc_id = document(&update);
    let (rail_id, _) = update
        .nodes
        .iter()
        .find(|(_, node)| {
            node.label() == Some("Explorer") && node.supports_action(egui::accesskit::Action::Click)
        })
        .unwrap();
    // Reproduce retained control focus without a traversal/AT navigation event.
    // SAFETY: this is the high-entropy egui widget ID from its rendered tree.
    let rail_widget = unsafe { egui::Id::from_high_entropy_bits(rail_id.0) };
    app.headless_egui_context()
        .memory_mut(|memory| memory.request_focus(rail_widget));
    app.run_headless_full_frame(frame(vec![]));
    let focused = || {
        app.headless_egui_context()
            .memory(|memory| memory.focused().map(|id| id.accesskit_id()))
    };
    assert_eq!(
        focused(),
        Some(*rail_id),
        "actual rail widget retains focus without navigation provenance"
    );
    app.run_headless_full_frame(frame(vec![egui::Event::AccessKitActionRequest(
        egui::accesskit::ActionRequest {
            action: egui::accesskit::Action::Focus,
            target_tree: egui::accesskit::TreeId::ROOT,
            target_node: doc_id,
            data: None,
        },
    )]));
    assert_eq!(
        app.headless_egui_context()
            .memory(|memory| memory.focused().map(|id| id.accesskit_id())),
        Some(*rail_id)
    );
    let key = |key| egui::Event::Key {
        key,
        physical_key: Some(key),
        pressed: true,
        repeat: false,
        modifiers: egui::Modifiers::NONE,
    };
    app.run_headless_full_frame(frame(vec![key(egui::Key::Enter)]));
    app.run_headless_full_frame(frame(vec![
        key(egui::Key::Space),
        egui::Event::Text(" ".into()),
    ]));
    assert_eq!(
        app.runtime_snapshot()
            .active_buffer_projection
            .small_buffer_text(),
        Some("\n seed"),
        "ignored document Focus cannot arm activation of an unrelated retained button"
    );
}

#[test]
fn review_warm_cache_rejects_contradictory_preview_until_fresh_identity() {
    let (_workspace, mut app) = open_app("alpha");
    let snapshot = app.shell_projection_snapshot("a11y").unwrap();
    let ctx = egui::Context::default();
    ctx.enable_accesskit();
    let mut view = ProjectionView::new();
    assert_eq!(document_text(render(&ctx, &mut view, &snapshot)), "alpha");
    let mut contradictory = snapshot.clone();
    contradictory.active_buffer_projection.small_buffer_preview = Some("bravo".into());
    assert_constrained(
        render(&ctx, &mut view, &contradictory),
        "contradictory preview",
    );
    assert_constrained(render(&ctx, &mut view, &snapshot), "contradictory preview");
    app.dispatch_ui_intent(CommandDispatchIntent::Insert {
        buffer_id: app.active_buffer_id().unwrap(),
        at: TextCoordinate {
            line: 0,
            character: 0,
            byte_offset: Some(0),
            utf16_offset: Some(0),
        },
        text: "!".into(),
    })
    .unwrap();
    assert_eq!(
        document_text(render(
            &ctx,
            &mut view,
            &app.shell_projection_snapshot("a11y").unwrap()
        )),
        "!alpha"
    );
}

#[test]
fn review_storage_fragments_preserve_consumer_logical_line_queries() {
    let first_line = "🦀".repeat(300);
    let second_line = "x".repeat(300);
    let text = format!("{first_line}\r\n{second_line}\nend");
    let (_workspace, app) = open_app(&text);
    let ctx = egui::Context::default();
    ctx.enable_accesskit();
    let update = render(
        &ctx,
        &mut ProjectionView::new(),
        &app.shell_projection_snapshot("a11y").unwrap(),
    );
    let id = document(&update);
    let tree = Tree::new(update, true);
    let node = tree
        .state()
        .node_by_tree_local_id(id, egui::accesskit::TreeId::ROOT)
        .unwrap();
    assert_eq!(
        node.line_range_from_index(0).unwrap().text(),
        format!("{first_line}\r\n")
    );
    assert_eq!(
        node.line_range_from_index(1).unwrap().text(),
        format!("{second_line}\n")
    );
    assert_eq!(node.line_range_from_index(2).unwrap().text(), "end");
    // The consumer exposes its document-end position as a final empty range.
    assert_eq!(node.line_range_from_index(3).unwrap().text(), "");
    assert!(node.line_range_from_index(4).is_none());
}
