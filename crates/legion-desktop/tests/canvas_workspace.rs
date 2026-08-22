//! The canvas workspace, driven through the rendered UI.
//!
//! A spatial surface is exactly the kind of thing that can look finished and be
//! unreachable: a renderer that draws cards nobody can get to, or drag handling
//! that moves a rectangle on screen while the arrangement is never recorded. So
//! every test here reaches the canvas the way a person does — click the rail
//! toggle — and then asks the *runtime*, not the renderer, what it believes.
//!
//! The distinction that matters most: a card moving is not the same as a layout
//! being kept. `the_arrangement_survives_a_restart` reopens the workspace from
//! its session record, which is the only proof that dragging did anything
//! durable.

use std::path::Path;

mod common;
use common::{
    TempWorkspace, click_at, clickable_center, full_frame_input, node_description, rendered_text,
};

use legion_desktop::workflow::{DesktopEframeApp, DesktopLaunchConfig, DesktopRuntime};

/// Every accessibility *value* in a frame.
///
/// `rendered_text` prefers a node's label and falls back to its value, so a
/// node carrying both -- which is what a named region of text is -- yields only
/// its name. A card's code lives in the value.
fn rendered_values(output: &egui::FullOutput) -> Vec<String> {
    output
        .platform_output
        .accesskit_update
        .as_ref()
        .map(|update| {
            update
                .nodes
                .iter()
                .filter_map(|(_id, node)| node.value().map(str::to_string))
                .collect()
        })
        .unwrap_or_default()
}

/// Three files, so the canvas has something to arrange.
fn workspace_with_files(prefix: &'static str) -> TempWorkspace {
    let workspace = TempWorkspace::new(prefix);
    workspace.write("alpha.rs", "fn alpha() {\n    let a = 1;\n    a\n}\n");
    workspace.write("beta.rs", "fn beta() {\n    let b = 2;\n    b\n}\n");
    workspace.write("gamma.rs", "fn gamma() {\n    let c = 3;\n    c\n}\n");
    workspace
}

fn open_app(root: &Path, session: Option<&Path>) -> DesktopEframeApp {
    let mut config = DesktopLaunchConfig::new(root.to_path_buf(), None);
    if let Some(session) = session {
        config = config.with_session_state(session.to_path_buf());
    }
    let runtime = DesktopRuntime::open(config).expect("desktop runtime should open workspace");
    DesktopEframeApp::new(runtime)
}

/// Open every file, so each becomes a card.
fn open_all_files(app: &mut DesktopEframeApp) {
    for name in ["alpha.rs", "beta.rs", "gamma.rs"] {
        let frame = app.run_headless_full_frame(full_frame_input(Vec::new()));
        let row = clickable_center(&frame, name)
            .unwrap_or_else(|| panic!("the explorer must offer {name} as a clickable row"));
        let _ = click_at(app, row);
    }
}

/// Click the rail's Canvas toggle and settle a frame.
///
/// The toggle routes through the runtime rather than flipping renderer state in
/// place, so the switch lands on the following frame — the same shape as every
/// other action in this shell.
fn show_canvas(app: &mut DesktopEframeApp) -> egui::FullOutput {
    let frame = app.run_headless_full_frame(full_frame_input(Vec::new()));
    let toggle =
        clickable_center(&frame, "Canvas").expect("the activity rail must offer a Canvas control");
    let _ = click_at(app, toggle);
    app.run_headless_full_frame(full_frame_input(Vec::new()))
}

/// Press, move, release — a drag, as a mouse performs it.
///
/// Split across frames deliberately. egui begins a drag on one frame and reports
/// the delta on the next, so a press and release in the same frame is a click
/// and moves nothing.
fn drag(app: &mut DesktopEframeApp, from: egui::Pos2, to: egui::Pos2) -> egui::FullOutput {
    let press = vec![
        egui::Event::PointerMoved(from),
        egui::Event::PointerButton {
            pos: from,
            button: egui::PointerButton::Primary,
            pressed: true,
            modifiers: egui::Modifiers::default(),
        },
    ];
    let _ = app.run_headless_full_frame(full_frame_input(press));
    let _ = app.run_headless_full_frame(full_frame_input(vec![egui::Event::PointerMoved(to)]));
    let release = vec![egui::Event::PointerButton {
        pos: to,
        button: egui::PointerButton::Primary,
        pressed: false,
        modifiers: egui::Modifiers::default(),
    }];
    let _ = app.run_headless_full_frame(full_frame_input(release));
    app.run_headless_full_frame(full_frame_input(Vec::new()))
}

/// The accessibility node carrying a label, by id.
///
/// The pointer helpers above answer "where is this control"; this answers "which
/// node is it", which is what an assistive technology addresses instead of a
/// coordinate.
fn accesskit_id(output: &egui::FullOutput, label: &str) -> Option<egui::accesskit::NodeId> {
    output
        .platform_output
        .accesskit_update
        .as_ref()
        .and_then(|update| {
            update
                .nodes
                .iter()
                .find_map(|(id, node)| (node.label() == Some(label)).then_some(*id))
        })
}

/// Activate a control the way a screen reader does — no pointer at all.
///
/// egui turns an AccessKit `Click` on a click-sensing widget into the same
/// `clicked()` that Space and Enter produce (`context.rs`, `FAKE_PRIMARY_CLICKED`).
/// No pointer button is pressed and none is released, so anything that only
/// watches for drags or for `any_released` sees nothing happen.
fn activate(app: &mut DesktopEframeApp, target: egui::accesskit::NodeId) -> egui::FullOutput {
    let request = egui::accesskit::ActionRequest {
        action: egui::accesskit::Action::Click,
        target_tree: egui::accesskit::TreeId::ROOT,
        target_node: target,
        data: None,
    };
    let _ =
        app.run_headless_full_frame(full_frame_input(vec![egui::Event::AccessKitActionRequest(
            request,
        )]));
    app.run_headless_full_frame(full_frame_input(Vec::new()))
}

/// A flick: press on one frame, then move and release together on the next.
///
/// This is not a slower drag with the same ending — it is the case egui reports
/// differently. `drag_stopped` is set on a frame where the widget is no longer
/// in `dragged`, and `Response::drag_delta` returns `Vec2::ZERO` unless
/// `dragged()` holds, so the movement that arrived with the release is invisible
/// to a delta. Any handler that builds its final position by accumulating deltas
/// drops it, and the card settles a frame behind the hand.
fn flick(app: &mut DesktopEframeApp, from: egui::Pos2, to: egui::Pos2) -> egui::FullOutput {
    let press = vec![
        egui::Event::PointerMoved(from),
        egui::Event::PointerButton {
            pos: from,
            button: egui::PointerButton::Primary,
            pressed: true,
            modifiers: egui::Modifiers::default(),
        },
    ];
    let _ = app.run_headless_full_frame(full_frame_input(press));
    // One ordinary drag frame, so a drag is genuinely under way. egui does not
    // call a press a drag until the pointer has moved, and a press whose very
    // next frame both moves and releases never becomes one at all -- a different
    // case, and not the one being tested here.
    let midpoint = from + (to - from) * 0.5;
    let _ =
        app.run_headless_full_frame(full_frame_input(vec![egui::Event::PointerMoved(midpoint)]));
    let move_and_release = vec![
        egui::Event::PointerMoved(to),
        egui::Event::PointerButton {
            pos: to,
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: egui::Modifiers::default(),
        },
    ];
    let _ = app.run_headless_full_frame(full_frame_input(move_and_release));
    app.run_headless_full_frame(full_frame_input(Vec::new()))
}

#[test]
fn the_canvas_is_reachable_and_shows_every_open_file() {
    let workspace = workspace_with_files("legion_desktop_canvas_reach");
    let mut app = open_app(workspace.path(), None);
    open_all_files(&mut app);

    // Before the switch there are no cards, only the editor.
    let editor = app.run_headless_full_frame(full_frame_input(Vec::new()));
    assert!(
        clickable_center(&editor, "Card alpha.rs").is_none(),
        "cards should not be on screen before the canvas is opened"
    );

    let canvas = show_canvas(&mut app);
    let labels = rendered_text(&canvas);

    // Every file by name, not a count.
    //
    // A count is weaker in the direction that matters: two of three files
    // vanishing leaves a count of one, which "at least two" would have caught
    // only by accident and "at least one" not at all. This test is the
    // regression gate for cards existing; the disambiguation rules have tests
    // of their own.
    for name in ["alpha.rs", "beta.rs", "gamma.rs"] {
        assert!(
            clickable_center(&canvas, &format!("Card {name}")).is_some(),
            "every open file must appear as a card on the canvas; {name} is missing. \
             Frame showed {labels:?}"
        );
    }
}

#[test]
fn a_card_carries_the_files_real_text() {
    // The point of a card over a tab: you can read the file without opening it.
    // Excerpt text is plainer than the active editor's -- no syntax colouring --
    // but it is the file's own content, not a placeholder.
    let workspace = workspace_with_files("legion_desktop_canvas_text");
    let mut app = open_app(workspace.path(), None);
    open_all_files(&mut app);
    let canvas = show_canvas(&mut app);

    // Read from the accessibility tree rather than from what was painted: a
    // card whose code is only painted is unreadable to a screen reader, and
    // this assertion would pass on it if it looked at pixels.
    let texts = rendered_values(&canvas);
    for line in ["fn alpha() {", "fn beta() {", "fn gamma() {"] {
        assert!(
            texts.iter().any(|text| text.contains(line)),
            "the canvas should expose each file's own text; {line:?} is absent. The tree had \
             {texts:?}"
        );
    }
}

#[test]
fn dragging_a_card_moves_it_and_the_runtime_records_where() {
    let workspace = workspace_with_files("legion_desktop_canvas_drag");
    let mut app = open_app(workspace.path(), None);
    open_all_files(&mut app);
    let canvas = show_canvas(&mut app);

    let before = clickable_center(&canvas, "Card alpha.rs").expect("alpha.rs must have a card");
    let target = before + egui::vec2(120.0, 90.0);
    let settled = drag(&mut app, before, target);

    let after = clickable_center(&settled, "Card alpha.rs")
        .expect("the card must still be on the canvas after a drag");
    assert_ne!(
        before, after,
        "dragging a card's header left it exactly where it started, so either the drag never \
         reached the header or the move was not applied"
    );

    // A card moving on screen is not the same as an arrangement being kept.
    // Asking the session record is what separates the two.
    let record = app
        .capture_session_record()
        .expect("the runtime must be able to capture a session record");
    let placed = record
        .canvas_nodes
        .iter()
        .find(|node| node.path.0.ends_with("alpha.rs"))
        .expect(
            "the moved card must appear in the session record; if it does not, the drag \
                 changed pixels and nothing else",
        );
    assert!(
        placed.x.is_finite() && placed.y.is_finite(),
        "a recorded position must be a real place, got ({}, {})",
        placed.x,
        placed.y
    );
}

#[test]
fn connecting_two_cards_records_an_edge() {
    let workspace = workspace_with_files("legion_desktop_canvas_connect");
    let mut app = open_app(workspace.path(), None);
    open_all_files(&mut app);
    let canvas = show_canvas(&mut app);

    let from = clickable_center(&canvas, "Connect from alpha.rs")
        .expect("each card must offer an outgoing connection port");
    let to = clickable_center(&canvas, "Connect to beta.rs")
        .expect("each card must offer an incoming connection port");

    let _ = drag(&mut app, from, to);

    let record = app
        .capture_session_record()
        .expect("the runtime must be able to capture a session record");
    assert!(
        record
            .canvas_edges
            .iter()
            .any(|edge| edge.from_path.0.ends_with("alpha.rs")
                && edge.to_path.0.ends_with("beta.rs")),
        "dragging from one card's port to another's recorded no connection; edges were {:?}",
        record
            .canvas_edges
            .iter()
            .map(|edge| (&edge.from_path.0, &edge.to_path.0))
            .collect::<Vec<_>>()
    );
}

#[test]
fn a_card_never_connects_to_itself() {
    // Cheap to get wrong, and a self-edge draws as a meaningless loop that
    // cannot be told from a rendering bug.
    let workspace = workspace_with_files("legion_desktop_canvas_self");
    let mut app = open_app(workspace.path(), None);
    open_all_files(&mut app);
    let canvas = show_canvas(&mut app);

    let from = clickable_center(&canvas, "Connect from alpha.rs").expect("outgoing port");
    let to = clickable_center(&canvas, "Connect to alpha.rs").expect("incoming port");
    let _ = drag(&mut app, from, to);

    let record = app.capture_session_record().expect("session record");
    assert!(
        !record
            .canvas_edges
            .iter()
            .any(|edge| edge.from_path == edge.to_path),
        "a card connected to itself"
    );
}

#[test]
fn the_arrangement_survives_a_restart() {
    // The whole claim of a spatial workspace is that where you put things is
    // remembered. A layout that resets on restart is a screensaver.
    let workspace = workspace_with_files("legion_desktop_canvas_persist");
    let session = workspace.path().join("session.json");

    let mut app = open_app(workspace.path(), Some(&session));
    open_all_files(&mut app);
    let canvas = show_canvas(&mut app);
    let start = clickable_center(&canvas, "Card alpha.rs").expect("alpha.rs card");
    let _ = drag(&mut app, start, start + egui::vec2(150.0, 60.0));

    let saved = app
        .capture_session_record()
        .expect("session record")
        .canvas_nodes
        .iter()
        .find(|node| node.path.0.ends_with("alpha.rs"))
        .map(|node| (node.x, node.y))
        .expect("the drag must be recorded before a restart can restore it");
    drop(app);

    let restored = open_app(workspace.path(), Some(&session));
    let record = restored
        .capture_session_record()
        .expect("the reopened runtime must capture a session record");
    let after = record
        .canvas_nodes
        .iter()
        .find(|node| node.path.0.ends_with("alpha.rs"))
        .map(|node| (node.x, node.y))
        .expect("the arrangement did not survive the restart: no position for alpha.rs");

    assert_eq!(
        saved, after,
        "the card came back somewhere other than where it was left"
    );
}

#[test]
fn the_canvas_toggle_returns_to_the_editor() {
    // A surface you can enter and not leave is a trap, and the rail button is
    // the only way back.
    let workspace = workspace_with_files("legion_desktop_canvas_toggle");
    let mut app = open_app(workspace.path(), None);
    open_all_files(&mut app);

    let canvas = show_canvas(&mut app);
    assert!(
        clickable_center(&canvas, "Connect from alpha.rs").is_some(),
        "the canvas should be showing"
    );

    let back = show_canvas(&mut app);
    assert!(
        clickable_center(&back, "Connect from alpha.rs").is_none(),
        "clicking Canvas a second time left the canvas up, so there is no way back to the \
         editor. Frame showed {:?}",
        rendered_text(&back).len()
    );
    // Cards are gone, not merely their ports.
    assert!(
        clickable_center(&back, "Card alpha.rs").is_none(),
        "a canvas card is still on screen after switching back to the editor"
    );
    // And the editor is really back, rather than the centre being empty. The
    // code canvas renders the active buffer with line numbers, which no other
    // surface does. Deliberately not "some text from gamma.rs is present": the
    // shell renders other buffers' first lines elsewhere, so that assertion
    // would pass on the canvas too and prove nothing.
    let texts = rendered_text(&back);
    assert!(
        texts.iter().any(|text| text.contains("1: fn gamma() {")),
        "the editor is not showing the active buffer after toggling back; frame had {texts:?}"
    );
}

/// Moving one card must not move the others.
///
/// Default slots used to come from a running count of *unplaced* cards, so the
/// moment one card gained a saved position the counter stopped incrementing for
/// it and every later unplaced card shifted one slot left on the next frame.
/// Dragging the first of three made the second and third jump — possibly onto
/// the card just moved.
///
/// This is the property a spatial workspace cannot compromise on: things stay
/// where they were put, including the ones that were never put anywhere.
#[test]
fn moving_one_card_leaves_the_others_where_they_were() {
    let workspace = workspace_with_files("legion_desktop_canvas_stability");
    let mut app = open_app(workspace.path(), None);
    open_all_files(&mut app);
    let canvas = show_canvas(&mut app);

    let before_beta = clickable_center(&canvas, "Card beta.rs").expect("beta card");
    let before_gamma = clickable_center(&canvas, "Card gamma.rs").expect("gamma card");

    // Move a *different* card.
    let alpha = clickable_center(&canvas, "Card alpha.rs").expect("alpha card");
    let settled = drag(&mut app, alpha, alpha + egui::vec2(40.0, 220.0));

    let after_beta = clickable_center(&settled, "Card beta.rs").expect("beta card after");
    let after_gamma = clickable_center(&settled, "Card gamma.rs").expect("gamma card after");

    assert_eq!(
        before_beta, after_beta,
        "dragging alpha moved beta, which nobody touched"
    );
    assert_eq!(
        before_gamma, after_gamma,
        "dragging alpha moved gamma, which nobody touched"
    );
}

/// One file is one card, however many excerpt sections describe it.
///
/// Nothing upstream promises the sections are distinct by path. Two for the same
/// file would stack two cards in one slot, and every lookup by path — including
/// the one that resolves a dropped connection — would silently pick whichever
/// the iteration reached first.
#[test]
fn a_file_gets_exactly_one_card() {
    let workspace = workspace_with_files("legion_desktop_canvas_unique");
    let mut app = open_app(workspace.path(), None);
    open_all_files(&mut app);
    let canvas = show_canvas(&mut app);

    let update = canvas
        .platform_output
        .accesskit_update
        .as_ref()
        .expect("the canvas frame must publish an accessibility tree");
    for name in ["alpha.rs", "beta.rs", "gamma.rs"] {
        let label = format!("Card {name}");
        let count = update
            .nodes
            .iter()
            .filter(|(_id, node)| node.label() == Some(label.as_str()))
            .count();
        assert_eq!(
            count, 1,
            "{name} should have exactly one card, found {count}"
        );
    }
}

/// Cards and ports announce themselves as controls, not as text.
///
/// The rest of this module works hard to be legible to the accessibility tree —
/// bounds carry the layer transform, body text is hoisted into a value — and a
/// node with a label and no role undoes that: "Card alpha.rs" reads as a
/// heading, "Connect from alpha.rs" as a section title, and nothing says either
/// can be pressed or dragged.
#[test]
fn cards_and_ports_are_published_as_controls() {
    let workspace = workspace_with_files("legion_desktop_canvas_roles");
    let mut app = open_app(workspace.path(), None);
    open_all_files(&mut app);
    let canvas = show_canvas(&mut app);

    let update = canvas
        .platform_output
        .accesskit_update
        .as_ref()
        .expect("accessibility tree");
    for label in [
        "Card alpha.rs",
        "Connect from alpha.rs",
        "Connect to alpha.rs",
    ] {
        let node = update
            .nodes
            .iter()
            .find(|(_id, node)| node.label() == Some(label))
            .map(|(_id, node)| node)
            .unwrap_or_else(|| panic!("no node labelled {label:?}"));
        assert_eq!(
            node.role(),
            egui::accesskit::Role::Button,
            "{label} must announce itself as a control, not as text"
        );
    }
}

/// Typing while the canvas is up must not reach the hidden buffer.
///
/// `editor_input_enabled` derived only from palette and dirty-prompt state, so
/// characters, Backspace, Delete and every editor shortcut still mutated the
/// active buffer while the editor was off screen. Invisible edits are the worst
/// shape a keyboard defect can take: nothing looks wrong until the file is
/// saved, and by then there is nothing to point at.
///
/// This is the canvas twin of the BYOK isolation test — same property, different
/// surface.
#[test]
fn typing_on_the_canvas_never_reaches_the_open_buffer() {
    let workspace = workspace_with_files("legion_desktop_canvas_no_edit");
    let mut app = open_app(workspace.path(), None);
    open_all_files(&mut app);

    let before = app
        .runtime_snapshot()
        .active_buffer_projection
        .small_buffer_preview
        .unwrap_or_default();
    assert!(
        !before.is_empty(),
        "the fixture needs a readable buffer, or this proves nothing"
    );

    let canvas = show_canvas(&mut app);
    assert!(
        clickable_center(&canvas, "Card alpha.rs").is_some(),
        "the canvas must be showing for this test to mean anything"
    );

    for character in "ZZZ".chars() {
        let _ = app.run_headless_full_frame(full_frame_input(vec![egui::Event::Text(
            character.to_string(),
        )]));
    }
    let _ = app.run_headless_full_frame(full_frame_input(vec![egui::Event::Key {
        key: egui::Key::Backspace,
        physical_key: Some(egui::Key::Backspace),
        pressed: true,
        repeat: false,
        modifiers: egui::Modifiers::default(),
    }]));

    let after = app
        .runtime_snapshot()
        .active_buffer_projection
        .small_buffer_preview
        .unwrap_or_default();
    assert_eq!(
        before, after,
        "typing on the canvas edited the buffer behind it"
    );
}

/// A connection drawn by accident can be undone.
///
/// `DisconnectCanvasNodes` existed with no gesture that could emit it, so an
/// edge drawn by mistake was permanent: the state had an undo and the surface
/// did not. Repeating the same drag removes it — the smallest gesture that could
/// work, and no sixth control on a card that already carries five.
#[test]
fn drawing_the_same_connection_again_removes_it() {
    let workspace = workspace_with_files("legion_desktop_canvas_disconnect");
    let mut app = open_app(workspace.path(), None);
    open_all_files(&mut app);
    let canvas = show_canvas(&mut app);

    let from = clickable_center(&canvas, "Connect from alpha.rs").expect("outgoing port");
    let to = clickable_center(&canvas, "Connect to beta.rs").expect("incoming port");

    let _ = drag(&mut app, from, to);
    let edges = |app: &DesktopEframeApp| {
        app.capture_session_record()
            .expect("session record")
            .canvas_edges
            .iter()
            .filter(|edge| {
                edge.from_path.0.ends_with("alpha.rs") && edge.to_path.0.ends_with("beta.rs")
            })
            .count()
    };
    assert_eq!(
        edges(&app),
        1,
        "the first drag should create the connection"
    );

    // The ports have not moved, but read them again rather than assuming.
    let after = app.run_headless_full_frame(full_frame_input(Vec::new()));
    let from = clickable_center(&after, "Connect from alpha.rs").expect("outgoing port");
    let to = clickable_center(&after, "Connect to beta.rs").expect("incoming port");
    let _ = drag(&mut app, from, to);

    assert_eq!(
        edges(&app),
        0,
        "repeating the gesture should remove the connection, not duplicate or keep it"
    );
}

#[test]
fn connection_ports_answer_activation_and_not_only_dragging() {
    // Both ports publish as `Button` with bounds and a name, so a screen reader
    // finds them, announces them as pressable, and offers them. Pressing them
    // did nothing: the source was recorded on `drag_started` and the edge was
    // completed on pointer release, and an activation is neither. The controls
    // were a promise the surface did not keep.
    let workspace = workspace_with_files("legion_desktop_canvas_activate");
    let mut app = open_app(workspace.path(), None);
    open_all_files(&mut app);
    let canvas = show_canvas(&mut app);

    let source = accesskit_id(&canvas, "Connect from alpha.rs")
        .expect("each card must publish an outgoing connection port");
    let armed = activate(&mut app, source);

    let target = accesskit_id(&armed, "Connect to beta.rs")
        .expect("each card must publish an incoming connection port");
    let _ = activate(&mut app, target);

    let record = app
        .capture_session_record()
        .expect("the runtime must be able to capture a session record");
    assert!(
        record
            .canvas_edges
            .iter()
            .any(|edge| edge.from_path.0.ends_with("alpha.rs")
                && edge.to_path.0.ends_with("beta.rs")),
        "activating one card's outgoing port and then another's incoming port recorded no \
         connection, so the ports are pointer-only despite being published as buttons; edges \
         were {:?}",
        record
            .canvas_edges
            .iter()
            .map(|edge| (&edge.from_path.0, &edge.to_path.0))
            .collect::<Vec<_>>()
    );
}

#[test]
fn activating_the_same_connection_again_removes_it() {
    // The pointer gesture toggles; the activation flow must agree, or an edge
    // made by keyboard could never be undone by keyboard.
    let workspace = workspace_with_files("legion_desktop_canvas_activate_toggle");
    let mut app = open_app(workspace.path(), None);
    open_all_files(&mut app);
    let _ = show_canvas(&mut app);

    for _ in 0..2 {
        let frame = app.run_headless_full_frame(full_frame_input(Vec::new()));
        let source = accesskit_id(&frame, "Connect from alpha.rs")
            .expect("the outgoing port must stay published");
        let armed = activate(&mut app, source);
        let target = accesskit_id(&armed, "Connect to beta.rs")
            .expect("the incoming port must stay published");
        let _ = activate(&mut app, target);
    }

    let record = app
        .capture_session_record()
        .expect("the runtime must be able to capture a session record");
    assert!(
        !record
            .canvas_edges
            .iter()
            .any(|edge| edge.from_path.0.ends_with("alpha.rs")
                && edge.to_path.0.ends_with("beta.rs")),
        "repeating the activation left the edge in place, so a connection made without a \
         pointer cannot be removed without one; edges were {:?}",
        record
            .canvas_edges
            .iter()
            .map(|edge| (&edge.from_path.0, &edge.to_path.0))
            .collect::<Vec<_>>()
    );
}

#[test]
fn a_card_settles_where_it_was_released_not_where_it_was_the_frame_before() {
    let workspace = workspace_with_files("legion_desktop_canvas_flick");
    let mut app = open_app(workspace.path(), None);
    open_all_files(&mut app);
    let canvas = show_canvas(&mut app);

    let grabbed = clickable_center(&canvas, "Card alpha.rs").expect("alpha.rs must have a card");
    let released = grabbed + egui::vec2(140.0, 110.0);
    let settled = flick(&mut app, grabbed, released);

    let landed = clickable_center(&settled, "Card alpha.rs")
        .expect("the card must still be on the canvas after a flick");
    // Grabbed at the header's centre and released at a point, so the header's
    // centre is that point. Anything else is the card lagging the hand.
    let drift = (landed - released).length();
    assert!(
        drift <= 1.0,
        "a card released at {released:?} settled at {landed:?}, {drift} away — the movement \
         that arrived with the release was dropped, which is precisely what a delta cannot \
         see on that frame"
    );
}

#[test]
fn every_card_is_placed_before_the_canvas_finishes_its_first_frame() {
    // The defaults travel as one action rather than one per card. What is
    // observable from here is the outcome that batching must not break: after
    // the first canvas frame every card has a durable position, not just the
    // last one to be handled.
    let workspace = workspace_with_files("legion_desktop_canvas_batch");
    let mut app = open_app(workspace.path(), None);
    open_all_files(&mut app);
    let _ = show_canvas(&mut app);

    let record = app
        .capture_session_record()
        .expect("the runtime must be able to capture a session record");
    for name in ["alpha.rs", "beta.rs", "gamma.rs"] {
        assert!(
            record
                .canvas_nodes
                .iter()
                .any(|node| node.path.0.ends_with(name)),
            "{name} has no recorded position after the canvas drew it, so its default slot was \
             never kept and it will move the next time the tab list changes; recorded {:?}",
            record
                .canvas_nodes
                .iter()
                .map(|node| &node.path.0)
                .collect::<Vec<_>>()
        );
    }
}

#[test]
fn undo_on_the_canvas_never_rewrites_the_buffer_behind_it() {
    // The typing gate closed one route into the hidden buffer and left another
    // open. `dispatch_keybindings` runs before any editor-specific handling and
    // consulted nothing about the centre surface, so Ctrl/Cmd+Z and
    // Ctrl/Cmd+Shift+Z went straight to `DesktopAction::Undo` and `Redo` while
    // the canvas was up -- rewriting a file that was not on screen, with
    // nothing to see until it was saved.
    let workspace = workspace_with_files("legion_desktop_canvas_no_undo");
    let mut app = open_app(workspace.path(), None);
    open_all_files(&mut app);

    // Give the buffer some history to undo. Without an edit first, an undo that
    // did reach the buffer would have nothing to do and the test would pass for
    // the wrong reason.
    for character in "EDIT".chars() {
        let _ = app.run_headless_full_frame(full_frame_input(vec![egui::Event::Text(
            character.to_string(),
        )]));
    }
    let edited = app
        .runtime_snapshot()
        .active_buffer_projection
        .small_buffer_preview
        .unwrap_or_default();
    assert!(
        edited.contains("EDIT"),
        "the fixture needs a real edit to undo, got {edited:?}"
    );

    let canvas = show_canvas(&mut app);
    assert!(
        clickable_center(&canvas, "Card alpha.rs").is_some(),
        "the canvas must be showing for this test to mean anything"
    );

    // Undo three times, and no redo. Pressing undo and then redo restores the
    // buffer whether or not either reached it, so a test that sends both
    // reports success against a product that is silently rewriting the file --
    // which is what the first version of this test did.
    for _ in 0..3 {
        let modifiers = egui::Modifiers {
            command: true,
            ctrl: true,
            ..Default::default()
        };
        let key = egui::Key::Z;
        let _ = app.run_headless_full_frame(full_frame_input(vec![egui::Event::Key {
            key,
            physical_key: Some(key),
            pressed: true,
            repeat: false,
            modifiers,
        }]));
        let _ = app.run_headless_full_frame(full_frame_input(Vec::new()));
    }

    let after = app
        .runtime_snapshot()
        .active_buffer_projection
        .small_buffer_preview
        .unwrap_or_default();
    assert_eq!(
        edited, after,
        "an undo or redo shortcut pressed on the canvas rewrote the buffer behind it"
    );
}

#[test]
fn editor_function_keys_do_not_reach_the_buffer_behind_the_canvas() {
    // F12 moves the cursor through a file that is not on screen and F9 drops a
    // breakpoint on it. Both are published keymap entries routed through the
    // same dispatcher as undo, and both were missing from the first version of
    // the gate -- which made ADR-0051 and the dependency-policy entry wrong,
    // not merely incomplete: they promise that no editor input reaches a buffer
    // while the canvas is showing, and that is a claim about the whole set.
    let workspace = workspace_with_files("legion_desktop_canvas_no_fkeys");
    let mut app = open_app(workspace.path(), None);
    open_all_files(&mut app);

    let before = app.runtime_snapshot();
    let breakpoints_before = before.debug_projection.breakpoints.len();

    let canvas = show_canvas(&mut app);
    assert!(
        clickable_center(&canvas, "Card alpha.rs").is_some(),
        "the canvas must be showing for this test to mean anything"
    );
    // The cursor position as the status bar states it. Read from the rendered
    // frame rather than from a projection field, because what must not change
    // is what the person can see.
    let cursor_line = |frame: &egui::FullOutput| {
        rendered_text(frame)
            .into_iter()
            .find(|line| line.starts_with("Ln "))
            .unwrap_or_default()
    };
    let cursor_before = cursor_line(&canvas);
    assert!(
        !cursor_before.is_empty(),
        "the status bar must state a cursor position, or this proves nothing"
    );

    for key in [egui::Key::F9, egui::Key::F12] {
        let _ = app.run_headless_full_frame(full_frame_input(vec![egui::Event::Key {
            key,
            physical_key: Some(key),
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::default(),
        }]));
        let _ = app.run_headless_full_frame(full_frame_input(Vec::new()));
    }

    let after = app.runtime_snapshot();
    assert_eq!(
        after.debug_projection.breakpoints.len(),
        breakpoints_before,
        "F9 on the canvas put a breakpoint on a file nobody was looking at"
    );
    let settled = app.run_headless_full_frame(full_frame_input(Vec::new()));
    assert_eq!(
        cursor_line(&settled),
        cursor_before,
        "F12 on the canvas moved the cursor through a buffer that was not on screen"
    );
}

#[test]
fn escape_on_the_canvas_does_not_clear_cursors_in_the_hidden_buffer() {
    // Escape is dispatched by a hard-coded block outside `dispatch_keybindings`,
    // so completing the keymap filter could not reach it. Multi-cursor state is
    // exactly the kind of thing that vanishing invisibly is worst for: nothing
    // is on screen to show it went, and the next edit does something other than
    // what was intended.
    let workspace = workspace_with_files("legion_desktop_canvas_no_escape");
    let mut app = open_app(workspace.path(), None);
    open_all_files(&mut app);

    // Two cursors, through the published keymap entry.
    let modifiers = egui::Modifiers {
        command: true,
        ctrl: true,
        alt: true,
        ..Default::default()
    };
    let _ = app.run_headless_full_frame(full_frame_input(vec![egui::Event::Key {
        key: egui::Key::ArrowDown,
        physical_key: Some(egui::Key::ArrowDown),
        pressed: true,
        repeat: false,
        modifiers,
    }]));
    let _ = app.run_headless_full_frame(full_frame_input(Vec::new()));

    // The same count the Escape handler itself consults.
    let cursor_count = |app: &DesktopEframeApp| {
        app.runtime_snapshot()
            .active_buffer_projection
            .viewport
            .as_ref()
            .map(|viewport| viewport.cursors.len().max(1))
            .unwrap_or(1)
    };
    let cursors_before = cursor_count(&app);
    if cursors_before <= 1 {
        // The fixture could not produce a second cursor, so this test cannot
        // say anything. Fail rather than pass quietly.
        panic!("the fixture needs more than one cursor to prove Escape did not clear them");
    }

    let canvas = show_canvas(&mut app);
    assert!(
        clickable_center(&canvas, "Card alpha.rs").is_some(),
        "the canvas must be showing for this test to mean anything"
    );

    let _ = app.run_headless_full_frame(full_frame_input(vec![egui::Event::Key {
        key: egui::Key::Escape,
        physical_key: Some(egui::Key::Escape),
        pressed: true,
        repeat: false,
        modifiers: egui::Modifiers::default(),
    }]));
    let _ = app.run_headless_full_frame(full_frame_input(Vec::new()));

    assert_eq!(
        cursor_count(&app),
        cursors_before,
        "Escape on the canvas cleared extra cursors in a buffer that was not on screen"
    );
}

#[test]
fn a_connection_is_readable_from_the_accessibility_tree() {
    // The ports can be activated, and until now the only report of what that
    // did was a painted curve. A screen reader could press a port and had no
    // way to learn whether it had connected or disconnected the cards, or what
    // was already connected to what -- and the gesture toggles, so the question
    // matters every single time.
    let workspace = workspace_with_files("legion_desktop_canvas_edge_a11y");
    let mut app = open_app(workspace.path(), None);
    open_all_files(&mut app);
    let canvas = show_canvas(&mut app);

    // Nothing connected yet: the ports must say so rather than say nothing.
    assert_eq!(
        node_description(&canvas, "Connect from alpha.rs").as_deref(),
        Some("No connections"),
        "an unconnected port must report that, or silence is indistinguishable from a \
         surface that has stopped working"
    );

    let source = accesskit_id(&canvas, "Connect from alpha.rs")
        .expect("each card must publish an outgoing connection port");
    let armed = activate(&mut app, source);
    let target = accesskit_id(&armed, "Connect to beta.rs")
        .expect("each card must publish an incoming connection port");
    let connected = activate(&mut app, target);

    assert_eq!(
        node_description(&connected, "Connect from alpha.rs").as_deref(),
        Some("Connects to beta.rs"),
        "after connecting, the outgoing port must name what it connects to"
    );
    assert_eq!(
        node_description(&connected, "Connect to beta.rs").as_deref(),
        Some("Connected from alpha.rs"),
        "the other end must report the connection too, from its own direction"
    );

    // And repeating the gesture removes it, which must be just as legible.
    let source = accesskit_id(&connected, "Connect from alpha.rs").expect("port must persist");
    let armed = activate(&mut app, source);
    let target = accesskit_id(&armed, "Connect to beta.rs").expect("port must persist");
    let disconnected = activate(&mut app, target);

    assert_eq!(
        node_description(&disconnected, "Connect from alpha.rs").as_deref(),
        Some("No connections"),
        "after disconnecting, the port must report that it no longer connects to anything"
    );
}

#[test]
fn a_file_opened_after_the_canvas_is_showing_appears_on_screen() {
    // The case a first-frame test cannot reach.
    //
    // On the opening frame the view is computed to fit whatever exists, so every
    // card is visible whatever the placement rules do. The defect is what
    // happens *afterwards*: the view is saved, a file is opened, its card is
    // laid out on the next free grid slot, and once the grid is full inside that
    // view every free slot is outside it. The card is placed, saved, and
    // nowhere -- with no minimap and no fit-to-content control to find it with.
    let workspace = TempWorkspace::new("legion_desktop_canvas_late_open");
    let names: Vec<String> = (0..9).map(|index| format!("file{index}.rs")).collect();
    for name in &names {
        workspace.write(name, "fn main() {}\n");
    }

    let mut app = open_app(workspace.path(), None);
    let open_file = |app: &mut DesktopEframeApp, name: &str| {
        let frame = app.run_headless_full_frame(full_frame_input(Vec::new()));
        let row = clickable_center(&frame, name)
            .unwrap_or_else(|| panic!("the explorer must offer {name} as a clickable row"));
        let _ = click_at(app, row);
    };

    // Three files, then the canvas, which saves a view fitted to those three.
    for name in names.iter().take(3) {
        open_file(&mut app, name);
    }
    let canvas = show_canvas(&mut app);
    assert!(
        clickable_center(&canvas, "Card file0.rs").is_some(),
        "the canvas must be showing before the rest are opened"
    );

    // The rest, opened while it is showing.
    for name in names.iter().skip(3) {
        open_file(&mut app, name);
    }
    let settled = app.run_headless_full_frame(full_frame_input(Vec::new()));

    // Inside the canvas region, not merely present in the tree.
    //
    // `clickable_center` answers from the accessibility tree, and egui publishes
    // nodes for content the scene has scrolled past -- so asserting only that a
    // card is *findable* passes for a card nobody can see, which is the whole
    // defect.
    let panel = app
        .last_editor_rect_for_test()
        .expect("the canvas must report the region it drew into");
    for name in &names {
        let centre = clickable_center(&settled, &format!("Card {name}"))
            .unwrap_or_else(|| panic!("{name} is open and has no card at all"));
        assert!(
            panel.contains(centre),
            "{name} was opened while the canvas was showing and its card sits at {centre:?}, \
             outside the canvas region {panel:?}; a card that is placed, saved and off screen \
             cannot be reached at all"
        );
    }
}

/// Every clickable node carrying a label, not just the first.
///
/// Two files with the same name produce two explorer rows with the same name,
/// and `clickable_center` answers with whichever comes first -- so clicking it
/// twice opens one file twice, and a test built on it would compare a card
/// against itself.
fn clickable_centers(output: &egui::FullOutput, label: &str) -> Vec<egui::Pos2> {
    output
        .platform_output
        .accesskit_update
        .as_ref()
        .map(|update| {
            update
                .nodes
                .iter()
                .filter(|(_id, node)| {
                    node.label() == Some(label)
                        && node.supports_action(egui::accesskit::Action::Click)
                })
                .filter_map(|(_id, node)| node.bounds())
                .map(|bounds| {
                    egui::pos2(
                        ((bounds.x0 + bounds.x1) * 0.5) as f32,
                        ((bounds.y0 + bounds.y1) * 0.5) as f32,
                    )
                })
                .collect()
        })
        .unwrap_or_default()
}

#[test]
fn cards_sharing_a_file_name_are_distinguishable_on_every_control() {
    // The card header, which is the control that *selects* a card, was the one
    // place still using the display title after the first disambiguation pass.
    let workspace = TempWorkspace::new("legion_desktop_canvas_same_name");
    workspace.write("src/index.ts", "export const a = 1;\n");
    workspace.write("tests/index.ts", "export const b = 2;\n");

    let mut app = open_app(workspace.path(), None);
    for folder in ["src", "tests"] {
        let frame = app.run_headless_full_frame(full_frame_input(Vec::new()));
        let row = clickable_center(&frame, folder)
            .unwrap_or_else(|| panic!("the explorer must offer {folder} as a row"));
        let _ = click_at(&mut app, row);
    }

    // Both rows now exist and are named the same, so they are taken by position
    // rather than by name.
    let expanded = app.run_headless_full_frame(full_frame_input(Vec::new()));
    let rows = clickable_centers(&expanded, "index.ts");
    assert_eq!(
        rows.len(),
        2,
        "the fixture needs both files listed before either is opened, got {rows:?}"
    );
    for row in rows {
        let _ = click_at(&mut app, row);
    }

    let canvas = show_canvas(&mut app);
    let headers: Vec<String> = rendered_text(&canvas)
        .into_iter()
        .filter(|line| line.starts_with("Card "))
        .collect();

    // Non-vacuity first: with one card open there is no ambiguity to resolve
    // and "Card index.ts" is the right label, so an absence proves nothing
    // until two cards exist.
    assert_eq!(
        headers.len(),
        2,
        "the fixture must open two cards sharing a name, got {headers:?}"
    );
    assert!(
        !headers.iter().any(|line| line == "Card index.ts"),
        "two files share a name and their card headers are announced identically, so a \
         screen-reader user cannot tell which card activation will select; headers were \
         {headers:?}"
    );
}

#[test]
fn the_find_bar_is_not_drawn_over_the_canvas() {
    // Its Replace and Replace All controls dispatch buffer mutations, and they
    // are buttons rather than keys — so the keyboard gate never saw them.
    // Opening replace in the editor, toggling Canvas and pressing Replace
    // edited a file that was not on screen.
    let workspace = workspace_with_files("legion_desktop_canvas_no_find_bar");
    let mut app = open_app(workspace.path(), None);
    open_all_files(&mut app);

    // Ctrl/Cmd+H, the published binding for the find bar.
    let modifiers = egui::Modifiers {
        command: true,
        ctrl: true,
        ..Default::default()
    };
    let _ = app.run_headless_full_frame(full_frame_input(vec![egui::Event::Key {
        key: egui::Key::H,
        physical_key: Some(egui::Key::H),
        pressed: true,
        repeat: false,
        modifiers,
    }]));
    let opened = app.run_headless_full_frame(full_frame_input(Vec::new()));
    assert!(
        clickable_center(&opened, "Replace").is_some(),
        "the fixture needs the replace controls on screen before the canvas is shown"
    );

    let canvas = show_canvas(&mut app);
    assert!(
        clickable_center(&canvas, "Card alpha.rs").is_some(),
        "the canvas must be showing for this test to mean anything"
    );
    assert!(
        clickable_center(&canvas, "Replace").is_none(),
        "the replace controls are still on screen over the canvas, where pressing one edits \
         a buffer nobody is looking at"
    );
}

#[test]
fn escape_gives_up_on_a_half_drawn_connection() {
    // Activating an output port arms a source, and the only ways to clear it
    // were choosing a target or releasing a pointer -- neither of which a
    // keyboard user does. A source armed and then thought better of stayed
    // armed, and the next port activated, whenever that happened, silently
    // toggled an edge nobody was in the middle of drawing.
    let workspace = workspace_with_files("legion_desktop_canvas_escape_edge");
    let mut app = open_app(workspace.path(), None);
    open_all_files(&mut app);
    let canvas = show_canvas(&mut app);

    let source = accesskit_id(&canvas, "Connect from alpha.rs")
        .expect("each card must publish an outgoing connection port");
    let armed = activate(&mut app, source);
    assert_eq!(
        node_description(&armed, "Connect from alpha.rs").as_deref(),
        Some("No connections"),
        "the fixture must start with nothing connected, or this proves nothing"
    );

    // Changed their mind.
    let _ = app.run_headless_full_frame(full_frame_input(vec![egui::Event::Key {
        key: egui::Key::Escape,
        physical_key: Some(egui::Key::Escape),
        pressed: true,
        repeat: false,
        modifiers: egui::Modifiers::default(),
    }]));
    let cleared = app.run_headless_full_frame(full_frame_input(Vec::new()));

    // A later target activation must now do nothing, rather than complete a
    // connection begun some time ago.
    let target = accesskit_id(&cleared, "Connect to beta.rs")
        .expect("each card must publish an incoming connection port");
    let _ = activate(&mut app, target);

    let record = app
        .capture_session_record()
        .expect("the runtime must be able to capture a session record");
    assert!(
        record.canvas_edges.is_empty(),
        "activating a target after Escape completed a connection nobody was drawing; edges \
         were {:?}",
        record
            .canvas_edges
            .iter()
            .map(|edge| (&edge.from_path.0, &edge.to_path.0))
            .collect::<Vec<_>>()
    );
}

#[test]
fn the_completion_popup_is_not_drawn_over_the_canvas() {
    // Its Enter, Tab and row-click paths dispatch `CompletionAccept`, which
    // applies to the active buffer. Like the find bar, they are controls rather
    // than keys, so the canvas input gate never saw them.
    //
    // Completions are injected rather than waited for: the first version of
    // this test pressed the completion binding with no language server running,
    // so there was no popup to be drawn anywhere and it passed with the gate
    // removed.
    let workspace = workspace_with_files("legion_desktop_canvas_no_completion");
    let mut app = open_app(workspace.path(), None);
    open_all_files(&mut app);

    let buffer_id = app
        .runtime_snapshot()
        .active_buffer_projection
        .buffer_id
        .expect("a buffer must be open to complete into");
    let items: Vec<serde_json::Value> = ["zzz_completion_one", "zzz_completion_two"]
        .iter()
        .enumerate()
        .map(|(index, label)| {
            serde_json::json!({
                "label": label,
                "kind": 2,
                "detail": format!("fn {label}() detail"),
                "sortText": format!("{index:04}"),
            })
        })
        .collect();
    app.runtime_mut_for_test()
        .app_mut_for_test()
        .ingest_lsp_completion_response_for_buffer(
            buffer_id,
            &serde_json::json!({ "items": items, "isIncomplete": false }),
            None,
        )
        .expect("inject completions");
    // The projection is what the popup reads, so it has to be rebuilt after the
    // injection; the flag alone renders nothing.
    app.runtime_mut_for_test()
        .dispatch_ui_action(legion_desktop::bridge::DesktopAction::RefreshOutline);
    app.runtime_mut_for_test()
        .set_completion_popup_open_for_test(true);
    assert!(
        !app.runtime_snapshot()
            .language_tooling_projection
            .completions
            .is_empty(),
        "the injected completions must reach the projection the popup reads"
    );

    // Non-vacuity: the popup has to be on screen in the editor first.
    let editor = app.run_headless_full_frame(full_frame_input(Vec::new()));
    assert!(
        rendered_text(&editor)
            .iter()
            .any(|line| line.contains("zzz_completion_one")),
        "the fixture must put a completion popup on screen before the canvas is shown; \
         frame was {:?}",
        rendered_text(&editor)
    );

    let canvas = show_canvas(&mut app);
    assert!(
        clickable_center(&canvas, "Card alpha.rs").is_some(),
        "the canvas must be showing for this test to mean anything"
    );
    assert!(
        !rendered_text(&canvas)
            .iter()
            .any(|line| line.contains("zzz_completion_one")),
        "the completion popup is still on screen over the canvas, where accepting a row \
         edits a buffer nobody is looking at; frame was {:?}",
        rendered_text(&canvas)
    );
}
