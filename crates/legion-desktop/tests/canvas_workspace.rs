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
use common::{TempWorkspace, click_at, clickable_center, full_frame_input, rendered_text};

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
