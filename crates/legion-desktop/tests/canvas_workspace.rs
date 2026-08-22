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

/// Give a control keyboard focus the way a screen reader does.
///
/// `Action::Focus` is the tree's own way of saying "the keyboard is here now",
/// and it involves no pointer at all -- which is the whole point: a canvas
/// reachable only by dragging is one a keyboard cannot arrange.
fn focus(app: &mut DesktopEframeApp, target: egui::accesskit::NodeId) -> egui::FullOutput {
    let request = egui::accesskit::ActionRequest {
        action: egui::accesskit::Action::Focus,
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

/// Press one key, then settle.
fn press_key(
    app: &mut DesktopEframeApp,
    key: egui::Key,
    modifiers: egui::Modifiers,
) -> egui::FullOutput {
    let _ = app.run_headless_full_frame(full_frame_input(vec![
        egui::Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers,
        },
        egui::Event::Key {
            key,
            physical_key: None,
            pressed: false,
            repeat: false,
            modifiers,
        },
    ]));
    app.run_headless_full_frame(full_frame_input(Vec::new()))
}

/// Hold a key across several frames, then let go.
///
/// The repeat flag is what a real key repeat carries, and it is the difference
/// between a gesture and a sequence of gestures.
fn hold_key(app: &mut DesktopEframeApp, key: egui::Key, repeats: usize) -> egui::FullOutput {
    let _ = app.run_headless_full_frame(full_frame_input(vec![egui::Event::Key {
        key,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: egui::Modifiers::NONE,
    }]));
    for _ in 0..repeats {
        let _ = app.run_headless_full_frame(full_frame_input(vec![egui::Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: true,
            modifiers: egui::Modifiers::NONE,
        }]));
    }
    let _ = app.run_headless_full_frame(full_frame_input(vec![egui::Event::Key {
        key,
        physical_key: None,
        pressed: false,
        repeat: false,
        modifiers: egui::Modifiers::NONE,
    }]));
    app.run_headless_full_frame(full_frame_input(Vec::new()))
}

/// Hold a key across several frames and do *not* let go.
fn hold_key_without_release(
    app: &mut DesktopEframeApp,
    key: egui::Key,
    repeats: usize,
) -> egui::FullOutput {
    let _ = app.run_headless_full_frame(full_frame_input(vec![egui::Event::Key {
        key,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: egui::Modifiers::NONE,
    }]));
    let mut last = app.run_headless_full_frame(full_frame_input(Vec::new()));
    for _ in 0..repeats {
        last = app.run_headless_full_frame(full_frame_input(vec![egui::Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: true,
            modifiers: egui::Modifiers::NONE,
        }]));
    }
    last
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

/// A card can be arranged without a pointer.
///
/// Every move came from `dragged()` or `drag_stopped()`, so the canvas
/// published its cards as controls a keyboard can reach and then had nothing
/// for that keyboard to do on arrival: activation switched tabs, and
/// arrangement -- the surface's entire reason to exist -- was pointer-only.
#[test]
fn a_focused_card_can_be_moved_with_the_keyboard() {
    let workspace = workspace_with_files("legion_desktop_canvas_keyboard_move");
    let mut app = open_app(workspace.path(), None);
    open_all_files(&mut app);
    let canvas = show_canvas(&mut app);

    let card = accesskit_id(&canvas, "Card alpha.rs").expect("alpha.rs must have a card");
    let before = clickable_center(&canvas, "Card alpha.rs").expect("the card must be on screen");
    let focused = focus(&mut app, card);
    assert!(
        clickable_center(&focused, "Card alpha.rs").is_some(),
        "focusing a card must not remove it"
    );

    let moved = press_key(&mut app, egui::Key::ArrowRight, egui::Modifiers::NONE);
    let after = clickable_center(&moved, "Card alpha.rs")
        .expect("the card must still be on the canvas after a keyboard move");
    assert!(
        after.x > before.x,
        "ArrowRight left the card at {after:?} from {before:?}; a canvas whose cards          can be focused and not moved is one a keyboard cannot arrange"
    );
    assert_eq!(
        after.y, before.y,
        "a horizontal nudge moved the card vertically as well"
    );

    // A card moving on screen is not an arrangement being kept: a keyboard user
    // who nudges a card and closes the window has to find it where they left it.
    let record = app
        .capture_session_record()
        .expect("the runtime must be able to capture a session record");
    let placed = record
        .canvas_nodes
        .iter()
        .find(|node| node.path.0.ends_with("alpha.rs"))
        .expect("a keyboard move must be recorded like a released drag");
    assert!(
        placed.x.is_finite() && placed.y.is_finite(),
        "a recorded position must be a real place, got ({}, {})",
        placed.x,
        placed.y
    );

    // And Shift moves further, so crossing the canvas is not a career.
    let coarse = press_key(&mut app, egui::Key::ArrowRight, egui::Modifiers::SHIFT);
    let far = clickable_center(&coarse, "Card alpha.rs").expect("the card must still be there");
    assert!(
        far.x - after.x > after.x - before.x,
        "Shift+ArrowRight moved the card no further than a plain press"
    );
}

/// The view comes to a card the keyboard reached.
///
/// Focus can reach a card the view cannot: Tab walks every card in the
/// arrangement, and an arrangement larger than the zoom floor cannot be shown
/// at once however carefully it is fitted -- which is what stops "fit all
/// cards" from being a complete answer on its own. Without this, tabbing to an
/// off-screen card put the keyboard somewhere invisible, and the arrow keys
/// then arranged a card nobody could see.
#[test]
fn focusing_a_card_off_screen_brings_the_view_to_it() {
    let workspace = workspace_with_files("legion_desktop_canvas_focus_follows");
    let mut app = open_app(workspace.path(), None);
    open_all_files(&mut app);
    let canvas = show_canvas(&mut app);

    // Put a card far outside any view the canvas would choose on its own.
    let card = accesskit_id(&canvas, "Card alpha.rs").expect("alpha.rs must have a card");
    let centre = clickable_center(&canvas, "Card alpha.rs").expect("the card must be on screen");
    let _ = drag(&mut app, centre, centre + egui::vec2(0.0, 400.0));
    for _ in 0..12 {
        let frame = app.run_headless_full_frame(full_frame_input(Vec::new()));
        let Some(now) = clickable_center(&frame, "Card alpha.rs") else {
            break;
        };
        let _ = drag(&mut app, now, now + egui::vec2(0.0, 400.0));
    }

    let parked = app.run_headless_full_frame(full_frame_input(Vec::new()));
    let panel = app
        .last_editor_rect_for_test()
        .expect("the canvas must report the region it drew into");
    let where_it_went = clickable_center(&parked, "Card alpha.rs");
    assert!(
        where_it_went.is_none_or(|centre| !panel.contains(centre)),
        "this test needs a card the view does not reach; it is still at {where_it_went:?}          inside {panel:?}"
    );

    let followed = focus(&mut app, card);
    let panel = app
        .last_editor_rect_for_test()
        .expect("the canvas must report the region it drew into");
    let centre = clickable_center(&followed, "Card alpha.rs")
        .expect("a focused card must be somewhere in the tree");
    assert!(
        panel.contains(centre),
        "the keyboard reached a card at {centre:?} that the view never came to, outside          {panel:?} -- so the arrow keys would arrange a card nobody can see"
    );
}

/// A shortcut that happens to use Tab is not focus navigation.
///
/// `Ctrl+Tab` is the published Next Tab shortcut and moves no widget focus at
/// all. Latching it left the navigation flag armed, so the next focus change --
/// from a click -- was misread as deliberate, and a leading space after that
/// was swallowed as an activation instead of typed.
///
/// Asserted on the flag rather than through the surface: a click leaves no
/// widget focused in this harness, so an end-to-end version passes whether or
/// not the flag is armed, which is how the first attempt at this test fooled me.
#[test]
fn a_modified_tab_does_not_count_as_focus_navigation() {
    let workspace = workspace_with_files("legion_desktop_canvas_modified_tab");
    let mut app = open_app(workspace.path(), None);
    open_all_files(&mut app);
    let _ = app.run_headless_full_frame(full_frame_input(Vec::new()));

    let _ = press_key(&mut app, egui::Key::Tab, egui::Modifiers::COMMAND);
    assert!(
        !app.focus_navigation_pending_for_test(),
        "a tab *shortcut* armed focus navigation, so the next focus change from a click is misread as deliberate"
    );

    let _ = press_key(&mut app, egui::Key::Tab, egui::Modifiers::CTRL);
    assert!(
        !app.focus_navigation_pending_for_test(),
        "Ctrl+Tab armed focus navigation"
    );

    // And the traversal chords still do, or the rule has swallowed the feature.
    let _ = app.run_headless_full_frame(full_frame_input(vec![egui::Event::Key {
        key: egui::Key::Tab,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: egui::Modifiers::NONE,
    }]));
    assert!(
        app.focus_navigation_pending_for_test(),
        "plain Tab is focus traversal and must still arm it"
    );
}

/// Navigating to a control buys one activation, not tenure.
///
/// The provenance is recomputed only when focus *changes*, so a control tabbed
/// to and then activated stayed classified as intentionally focused. If its
/// action returned to the editor, the next leading space was swallowed again
/// and could press the same control a second time.
#[test]
fn activating_a_focused_control_spends_its_keyboard_claim() {
    let workspace = workspace_with_files("legion_desktop_canvas_provenance");
    let mut app = open_app(workspace.path(), None);
    open_all_files(&mut app);

    let editor = app.run_headless_full_frame(full_frame_input(Vec::new()));
    let control = accesskit_id(&editor, "Canvas").expect("the rail must publish a Canvas control");
    let _ = focus(&mut app, control);

    // Space opens the canvas, spending the claim that navigation gave it.
    let opened = press_key(&mut app, egui::Key::Space, egui::Modifiers::NONE);
    assert!(
        clickable_center(&opened, "Card alpha.rs").is_some(),
        "Space must still press a control the keyboard navigated to"
    );

    // A modified chord belongs to whoever documented it: Alt+Enter applies a
    // review hunk from the shell handler, and eating it here stopped that
    // working whenever a control happened to hold the keyboard. It must not
    // read as a plain activation either -- the canvas stays open.
    let _ = focus(&mut app, control);
    let modified = press_key(&mut app, egui::Key::Enter, egui::Modifiers::ALT);
    assert!(
        clickable_center(&modified, "Card alpha.rs").is_some(),
        "Alt+Enter toggled the surface, so the modified chord was read as a plain activation"
    );

    // Back to the editor through the same control, then type a leading space.
    let _ = press_key(&mut app, egui::Key::Space, egui::Modifiers::NONE);
    let _ = app.run_headless_full_frame(full_frame_input(Vec::new()));
    let before = app.runtime_snapshot().active_buffer_projection.dirty;
    assert!(!before, "the fixture must return to a clean editor");

    let _ = app.run_headless_full_frame(full_frame_input(vec![
        egui::Event::Key {
            key: egui::Key::Space,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        },
        egui::Event::Text(" ".to_string()),
    ]));
    let after = app.run_headless_full_frame(full_frame_input(Vec::new()));

    assert!(
        app.runtime_snapshot().active_buffer_projection.dirty,
        "a leading space was swallowed by a control whose activation was already spent"
    );
    assert!(
        clickable_center(&after, "Card alpha.rs").is_none(),
        "the swallowed space pressed the control again"
    );
}

/// The card whose buffer the app is acting on says so.
///
/// Activating a card switches the active buffer, and Next/Previous Tab changes
/// it without touching the canvas -- so Save Active and Close Tab were aimed at
/// a card nobody could identify here. A selection that is real and invisible is
/// one somebody acts on by accident.
#[test]
fn the_card_owning_the_active_buffer_is_marked_as_such() {
    let workspace = workspace_with_files("legion_desktop_canvas_active_card");
    let mut app = open_app(workspace.path(), None);
    open_all_files(&mut app);
    let canvas = show_canvas(&mut app);

    let selected: Vec<String> = ["alpha.rs", "beta.rs", "gamma.rs"]
        .iter()
        .filter(|name| node_is_selected(&canvas, &format!("Card {name}")) == Some(true))
        .map(|name| (*name).to_string())
        .collect();
    assert_eq!(
        selected.len(),
        1,
        "exactly one card owns the active buffer; the tree marked {selected:?}"
    );

    // Switching to another card moves the mark with the buffer.
    let other = ["alpha.rs", "beta.rs", "gamma.rs"]
        .into_iter()
        .find(|name| !selected.contains(&(*name).to_string()))
        .expect("the fixture opens three files");
    let card = accesskit_id(&canvas, &format!("Card {other}")).expect("the other card exists");
    let switched = activate(&mut app, card);
    assert_eq!(
        node_is_selected(&switched, &format!("Card {other}")),
        Some(true),
        "activating a card switched the buffer without moving the mark, so the surface          shows one card and the app acts on another"
    );
    assert_eq!(
        node_is_selected(&switched, &format!("Card {}", selected[0])),
        Some(false),
        "the previously active card is still marked, so two cards claim the buffer"
    );
}

/// A target port says when it cannot yet be used.
///
/// With no source armed, activating an input port set the pending target and
/// the handler then did nothing -- an operable-looking control with no action
/// and no explanation, for the reader who cannot see that nothing happened.
#[test]
fn a_target_port_reports_that_it_needs_a_source_first() {
    let workspace = workspace_with_files("legion_desktop_canvas_target_unarmed");
    let mut app = open_app(workspace.path(), None);
    open_all_files(&mut app);
    let canvas = show_canvas(&mut app);

    let description = node_description(&canvas, "Connect to beta.rs").unwrap_or_default();
    assert!(
        description.contains("outgoing port first"),
        "a target port with nothing armed must say why activating it does nothing: {description:?}"
    );

    // And once a source is armed it stops saying so, because then it works.
    let source = accesskit_id(&canvas, "Connect from alpha.rs").expect("alpha.rs has a port");
    let armed = activate(&mut app, source);
    let description = node_description(&armed, "Connect to beta.rs").unwrap_or_default();
    assert!(
        !description.contains("outgoing port first"),
        "the target still claims it needs a source after one was armed: {description:?}"
    );
}

/// Nudging a card past the edge brings the view with it.
///
/// The reveal is one-shot, on the frame focus arrives -- right for a card
/// sitting still and wrong for one being moved. It walked off the edge while
/// keeping the keyboard, and every further press moved a card nobody could see.
#[test]
fn nudging_a_card_past_the_edge_keeps_it_in_view() {
    let workspace = workspace_with_files("legion_desktop_canvas_nudge_reveal");
    let mut app = open_app(workspace.path(), None);
    open_all_files(&mut app);
    let canvas = show_canvas(&mut app);

    let card = accesskit_id(&canvas, "Card alpha.rs").expect("alpha.rs must have a card");
    let _ = focus(&mut app, card);

    // Enough coarse nudges to cross the panel, in one direction.
    //
    // Movement is measured in the *arrangement*, not on screen: once the view
    // follows a card being nudged, its screen position stops changing, which is
    // the behaviour under test rather than a stalled gesture.
    let world_x = |app: &mut DesktopEframeApp| {
        app.capture_session_record()
            .expect("record")
            .canvas_nodes
            .iter()
            .find(|node| node.path.0.ends_with("alpha.rs"))
            .map(|node| node.x)
            .expect("alpha.rs must be on the canvas")
    };
    let mut moved = world_x(&mut app);
    for _ in 0..14 {
        let frame = press_key(&mut app, egui::Key::ArrowRight, egui::Modifiers::SHIFT);
        let panel = app
            .last_editor_rect_for_test()
            .expect("the canvas must report the region it drew into");
        let centre = clickable_center(&frame, "Card alpha.rs").expect("the card must still exist");
        assert!(
            panel.contains(centre),
            "a nudged card left the view at {centre:?}, outside {panel:?}, while still              holding the keyboard -- every press after this moves a card nobody sees"
        );
        let now = world_x(&mut app);
        assert!(
            now > moved,
            "the nudge stopped moving the card: {now} after {moved}"
        );
        moved = now;
    }
}

/// Panning away from a focused card is not undone the next frame.
///
/// The reveal was requested for as long as focus lasted rather than when it
/// arrived, so every frame put the request back and the frame after moved the
/// view to the card again. Looking at another part of the canvas was impossible
/// until focus moved somewhere else -- and focus is exactly what a keyboard user
/// has no reason to move.
#[test]
fn panning_away_from_a_focused_card_is_not_undone() {
    let workspace = workspace_with_files("legion_desktop_canvas_focus_pan");
    let mut app = open_app(workspace.path(), None);
    open_all_files(&mut app);
    let canvas = show_canvas(&mut app);

    let card = accesskit_id(&canvas, "Card alpha.rs").expect("alpha.rs must have a card");
    let focused = focus(&mut app, card);
    let before = clickable_center(&focused, "Card alpha.rs").expect("the card must be on screen");
    let panel = app
        .last_editor_rect_for_test()
        .expect("the canvas must report the region it drew into");
    assert!(
        panel.contains(before),
        "the focused card must start inside the view, or panning it out proves nothing"
    );

    // Pan far enough that the focused card leaves the view entirely: that is
    // the case the reveal reacts to, and a pan that keeps it on screen proves
    // nothing about the reveal being reissued.
    let panel = app
        .last_editor_rect_for_test()
        .expect("the canvas must report the region it drew into");
    let empty = egui::pos2(panel.right() - 40.0, panel.bottom() - 40.0);
    for _ in 0..6 {
        let _ = drag(&mut app, empty, empty + egui::vec2(-300.0, -200.0));
    }
    let panned = app.run_headless_full_frame(full_frame_input(Vec::new()));
    let panel = app
        .last_editor_rect_for_test()
        .expect("the canvas must report the region it drew into");
    let moved = clickable_center(&panned, "Card alpha.rs");
    assert!(
        moved.is_none_or(|centre| !panel.contains(centre)),
        "the pan left the focused card on screen at {moved:?}, so nothing here exercises the reveal"
    );

    // And it stays gone: no request is reissued while focus merely persists.
    let settled = app.run_headless_full_frame(full_frame_input(Vec::new()));
    let panel = app
        .last_editor_rect_for_test()
        .expect("the canvas must report the region it drew into");
    let after = clickable_center(&settled, "Card alpha.rs");
    assert!(
        after.is_none_or(|centre| !panel.contains(centre)),
        "the view snapped back to the focused card at {after:?}, so panning away from it is impossible while it holds the keyboard"
    );
}

/// The same for a port, which traversal reaches just as readily.
///
/// Only card headers asked the view to follow, and Tab walks the ports too --
/// so traversal past the last visible card focused connection controls on cards
/// that were off screen: operable, and invisible.
#[test]
fn focusing_a_port_off_screen_brings_the_view_to_it() {
    let workspace = workspace_with_files("legion_desktop_canvas_port_follows");
    let mut app = open_app(workspace.path(), None);
    open_all_files(&mut app);
    let canvas = show_canvas(&mut app);

    let port = accesskit_id(&canvas, "Connect from alpha.rs")
        .expect("each card must publish an outgoing connection port");
    let card = clickable_center(&canvas, "Card alpha.rs").expect("the card must be on screen");
    let _ = drag(&mut app, card, card + egui::vec2(0.0, 400.0));
    for _ in 0..12 {
        let frame = app.run_headless_full_frame(full_frame_input(Vec::new()));
        let Some(now) = clickable_center(&frame, "Card alpha.rs") else {
            break;
        };
        let _ = drag(&mut app, now, now + egui::vec2(0.0, 400.0));
    }

    let parked = app.run_headless_full_frame(full_frame_input(Vec::new()));
    let panel = app
        .last_editor_rect_for_test()
        .expect("the canvas must report the region it drew into");
    let parked_port = clickable_center(&parked, "Connect from alpha.rs");
    assert!(
        parked_port.is_none_or(|centre| !panel.contains(centre)),
        "this test needs a port the view does not reach; it is still at {parked_port:?}          inside {panel:?}"
    );

    let followed = focus(&mut app, port);
    let panel = app
        .last_editor_rect_for_test()
        .expect("the canvas must report the region it drew into");
    let centre = clickable_center(&followed, "Connect from alpha.rs")
        .expect("a focused port must be somewhere in the tree");
    assert!(
        panel.contains(centre),
        "the keyboard reached a connection port at {centre:?} that the view never came          to, outside {panel:?} -- an operable control nobody can see"
    );
}

/// Strokes painted around a rect, with their width and colour.
///
/// Read from the paint output rather than the accessibility tree, because a
/// focus indicator is by definition the thing the tree cannot carry: it exists
/// for somebody who is looking at the screen and using a keyboard.
fn strokes_around(output: &egui::FullOutput, target: egui::Pos2) -> Vec<(u32, egui::Color32)> {
    fn collect(
        shape: &egui::epaint::Shape,
        target: egui::Pos2,
        found: &mut Vec<(u32, egui::Color32)>,
    ) {
        match shape {
            egui::epaint::Shape::Rect(rect) => {
                if rect.rect.expand(8.0).contains(target) && rect.stroke.width > 0.0 {
                    found.push((rect.stroke.width.round() as u32, rect.stroke.color));
                }
            }
            // Ports are circles, and a focus ring on one is a circle too.
            egui::epaint::Shape::Circle(circle) => {
                if circle.center.distance(target) <= circle.radius + 8.0
                    && circle.stroke.width > 0.0
                {
                    found.push((circle.stroke.width.round() as u32, circle.stroke.color));
                }
            }
            egui::epaint::Shape::Vec(shapes) => {
                for shape in shapes {
                    collect(shape, target, found);
                }
            }
            _ => {}
        }
    }

    let mut found = Vec::new();
    for clipped in &output.shapes {
        collect(&clipped.shape, target, &mut found);
    }
    found
}

/// A focused card looks different from an unfocused one.
///
/// Tabbing to a card left it painted exactly like every other card, and the
/// arrow keys then moved something the person could not identify. A focusable
/// control that looks unfocused is worse than one that cannot be focused at
/// all: it invites the gesture and hides its target.
#[test]
fn a_focused_card_is_drawn_differently_from_the_rest() {
    let workspace = workspace_with_files("legion_desktop_canvas_focus_ring");
    let mut app = open_app(workspace.path(), None);
    open_all_files(&mut app);
    let canvas = show_canvas(&mut app);

    let card = accesskit_id(&canvas, "Card alpha.rs").expect("alpha.rs must have a card");
    let centre = clickable_center(&canvas, "Card alpha.rs").expect("the card must be on screen");
    let before = strokes_around(&canvas, centre);

    let focused = focus(&mut app, card);
    let centre = clickable_center(&focused, "Card alpha.rs").expect("the card must still be there");
    let after = strokes_around(&focused, centre);

    assert!(
        after.len() > before.len(),
        "focusing the card painted nothing new around it: {before:?} then {after:?}"
    );

    // And the difference is not painted around every card, or it says nothing
    // about which one has the keyboard.
    let other = clickable_center(&focused, "Card beta.rs").expect("beta.rs must have a card");
    let unfocused = strokes_around(&focused, other);
    assert!(
        after.len() > unfocused.len(),
        "the focused card is painted like the unfocused one: {after:?} against {unfocused:?}"
    );
}

/// A gesture interrupted before the key comes up still reaches disk.
///
/// Batching the repeats onto the release means the release is the only durable
/// write -- and holding an arrow, then clicking something else, takes focus
/// away before it arrives. Every move so far was `settled: false`, so closing
/// the window lost the arrangement. Losing focus ends the gesture as surely as
/// letting go does.
#[test]
fn a_keyboard_move_survives_focus_leaving_mid_gesture() {
    let workspace = workspace_with_files("legion_desktop_canvas_focus_flush");
    let session = workspace.path().join("session.json");
    let mut app = open_app(workspace.path(), Some(&session));
    open_all_files(&mut app);
    let canvas = show_canvas(&mut app);

    let card = accesskit_id(&canvas, "Card alpha.rs").expect("alpha.rs must have a card");
    let _ = focus(&mut app, card);
    let _ = hold_key_without_release(&mut app, egui::Key::ArrowRight, 4);

    let moved = app
        .capture_session_record()
        .expect("record")
        .canvas_nodes
        .iter()
        .find(|node| node.path.0.ends_with("alpha.rs"))
        .map(|node| node.x)
        .expect("alpha.rs must be on the canvas");

    // Focus goes elsewhere with the key still down: the release will never
    // reach this card.
    let other = accesskit_id(&canvas, "Card beta.rs").expect("beta.rs must have a card");
    let _ = focus(&mut app, other);

    // What a restart would read.
    let restarted = open_app(workspace.path(), Some(&session));
    let saved = restarted
        .capture_session_record()
        .expect("record")
        .canvas_nodes
        .iter()
        .find(|node| node.path.0.ends_with("alpha.rs"))
        .map(|node| node.x)
        .expect("alpha.rs must be in the saved arrangement");
    assert!(
        (saved - moved).abs() < 0.5,
        "the arrangement on disk has alpha.rs at {saved} and the session has it at          {moved}; a gesture that lost focus before the key came up was never saved"
    );
}

/// A hover tooltip does not outlive the editor it describes.
///
/// The completion popup and the find bar were gated to the editor because they
/// dispatch buffer mutations. The hover tooltip mutates nothing, and was left
/// out for that reason -- but it still describes a symbol in a file the canvas
/// has replaced. A tooltip that survives the thing it points at is a label on
/// the wrong object, and here every card is a different file it could
/// plausibly belong to.
#[test]
fn a_hover_tooltip_does_not_survive_the_switch_to_the_canvas() {
    let workspace = workspace_with_files("legion_desktop_canvas_hover");
    let mut app = open_app(workspace.path(), None);
    open_all_files(&mut app);

    let snapshot = app.runtime_snapshot();
    let buffer_id = snapshot
        .active_buffer_projection
        .buffer_id
        .expect("a file is open");
    app.runtime_mut_for_test()
        .app_mut_for_test()
        .ingest_lsp_hover_response_for_buffer(
            buffer_id,
            &serde_json::json!({
                "contents": {"kind": "markdown", "value": "fn marker_from_the_editor"},
                "range": {
                    "start": {"line": 0, "character": 0},
                    "end": {"line": 0, "character": 3}
                }
            }),
            None,
        )
        .expect("inject hover");
    // The runtime caches its snapshot, so the injected hover has to be picked up
    // by a refresh before any frame can draw it.
    app.runtime_mut_for_test()
        .handle_action(legion_desktop::bridge::DesktopAction::SetCenterSurface {
            surface: legion_desktop::view::CenterSurface::Editor,
        })
        .expect("staying on the editor must refresh the projection");
    // Visibility is its own flag, set when a hover gesture opens the tooltip.
    app.runtime_mut_for_test()
        .set_hover_tooltip_visible_for_test(true);

    let editor = app.run_headless_full_frame(full_frame_input(Vec::new()));
    assert!(
        rendered_text(&editor)
            .iter()
            .any(|line| line == "Esc dismiss"),
        "the fixture must show the tooltip over the editor first, or the assertion below          holds for a tooltip that was never there; frame was {:?}",
        rendered_text(&editor)
    );

    let canvas = show_canvas(&mut app);
    assert!(
        clickable_center(&canvas, "Card alpha.rs").is_some(),
        "the canvas must be showing"
    );
    assert!(
        !rendered_text(&canvas)
            .iter()
            .any(|line| line == "Esc dismiss"),
        "a tooltip describing a symbol in the hidden editor is still on screen over the          canvas; frame was {:?}",
        rendered_text(&canvas)
    );
}

/// A modified arrow belongs to whoever else claimed it.
///
/// `Alt+ArrowRight`/`Left` navigate proposal review hunks, dispatched from the
/// shell's keyboard handler, which does not know that a card has focus. Reading
/// the same chord as a nudge made a documented review shortcut move and persist
/// an unrelated card as a side effect -- and the card had focus precisely
/// because somebody had been arranging it, so the wrong card was the one they
/// cared about.
#[test]
fn a_modified_arrow_does_not_move_a_focused_card() {
    let workspace = workspace_with_files("legion_desktop_canvas_alt_arrow");
    let mut app = open_app(workspace.path(), None);
    open_all_files(&mut app);
    let canvas = show_canvas(&mut app);

    let card = accesskit_id(&canvas, "Card alpha.rs").expect("alpha.rs must have a card");
    let before = clickable_center(&canvas, "Card alpha.rs").expect("the card must be on screen");
    let _ = focus(&mut app, card);

    for modifiers in [egui::Modifiers::ALT, egui::Modifiers::COMMAND] {
        let frame = press_key(&mut app, egui::Key::ArrowRight, modifiers);
        let after = clickable_center(&frame, "Card alpha.rs").expect("the card must still exist");
        assert_eq!(
            after, before,
            "a modified arrow moved the focused card, so a shortcut belonging to another              surface rearranges this one as a side effect"
        );
    }

    // And the release of a modified arrow does not settle either.
    //
    // The modifier filter covers movement; the release path is separate, and a
    // settled move at the card's own position is still recorded as a person
    // placing it there -- so an automatic slot became a person-reserved one,
    // held after the file closed, because somebody navigated a diff.
    let record = app.capture_session_record().expect("record");
    assert!(
        record
            .canvas_nodes
            .iter()
            .find(|node| node.path.0.ends_with("alpha.rs"))
            .is_none_or(|node| !node.placed_by_person),
        "a modified arrow marked an automatically placed card as person-placed, so it          reserves its slot even once the file is closed"
    );

    // Non-vacuity: the same key with no modifier still moves it, so this is a
    // test about modifiers and not about a gesture that stopped working.
    let plain = press_key(&mut app, egui::Key::ArrowRight, egui::Modifiers::NONE);
    let moved = clickable_center(&plain, "Card alpha.rs").expect("the card must still exist");
    assert!(
        moved.x > before.x,
        "an unmodified arrow no longer moves the card, so the guard above is measuring          a feature that is simply gone"
    );
}

/// A held arrow key writes the arrangement once, at the end.
///
/// Every repeat frame emitted a settled move, and a settled move validates,
/// `sync_all`s and atomically replaces the session file -- on the thread that
/// has to keep drawing. Moving a card any distance therefore queued a durable
/// write between every pair of frames. A pointer drag has always had the answer
/// to this: stream the movement, persist the end of the gesture.
#[test]
fn holding_an_arrow_key_persists_the_arrangement_once() {
    let workspace = workspace_with_files("legion_desktop_canvas_key_repeat");
    let mut app = open_app(workspace.path(), None);
    open_all_files(&mut app);
    let canvas = show_canvas(&mut app);

    let card = accesskit_id(&canvas, "Card alpha.rs").expect("alpha.rs must have a card");
    let before = clickable_center(&canvas, "Card alpha.rs").expect("the card must be on screen");
    let _ = focus(&mut app, card);

    let saves_before = app.session_saves_for_test();
    let held = hold_key(&mut app, egui::Key::ArrowRight, 8);
    let saves = app.session_saves_for_test() - saves_before;

    let after = clickable_center(&held, "Card alpha.rs").expect("the card must still be there");
    assert!(
        after.x > before.x,
        "the held key moved nothing, so this measures the cost of doing nothing"
    );
    assert_eq!(
        saves, 1,
        "a single held-key gesture asked for {saves} durable session writes; each one          validates, syncs and atomically replaces the file on the drawing thread"
    );
}

/// A focused port shows which of the two has the keyboard.
///
/// Bringing the card into view says which *card*. The two ports sit eight
/// pixels apart and Enter does opposite things on them -- arm a source, or
/// complete a connection -- so a sighted keyboard user needs to see which one
/// is about to act.
#[test]
fn a_focused_port_is_drawn_differently_from_the_rest() {
    let workspace = workspace_with_files("legion_desktop_canvas_port_ring");
    let mut app = open_app(workspace.path(), None);
    open_all_files(&mut app);
    let canvas = show_canvas(&mut app);

    let port = accesskit_id(&canvas, "Connect from alpha.rs").expect("the card must have a port");
    let centre = clickable_center(&canvas, "Connect from alpha.rs").expect("port on screen");
    let before = strokes_around(&canvas, centre);

    let focused = focus(&mut app, port);
    let centre = clickable_center(&focused, "Connect from alpha.rs").expect("port still there");
    let after = strokes_around(&focused, centre);
    assert!(
        after.len() > before.len(),
        "focusing the port painted nothing new around it: {before:?} then {after:?}"
    );

    // The ring is thicker than the ports' own outlines, and the port beside it
    // does not have one -- a difference that appears on both says nothing about
    // which has the keyboard. Compared by width rather than by count: the input
    // port paints its own thin circle, so counting strokes finds the same
    // number on each.
    assert!(
        after.iter().any(|(width, _)| *width >= 2),
        "the focused port has no ring, only its own outline: {after:?}"
    );
    let other = clickable_center(&focused, "Connect to alpha.rs").expect("the other port");
    let unfocused = strokes_around(&focused, other);
    assert!(
        !unfocused.iter().any(|(width, _)| *width >= 2),
        "the unfocused port is ringed too, so the ring says nothing about which has the          keyboard: {unfocused:?}"
    );
}

/// Typing takes the keyboard back from a button that still holds it.
///
/// Closing an overlay restores focus to the rail control that opened it, and
/// that focus outlives the reason for it. Suppressing activation keys while it
/// lasted meant every typed space was swallowed -- silently, and for as long as
/// the stale focus lasted.
#[test]
fn typing_reclaims_the_keyboard_from_a_button_that_still_holds_focus() {
    let workspace = workspace_with_files("legion_desktop_canvas_stale_focus");
    let mut app = open_app(workspace.path(), None);
    open_all_files(&mut app);

    let editor = app.run_headless_full_frame(full_frame_input(Vec::new()));
    let control = accesskit_id(&editor, "Canvas").expect("the rail must publish a Canvas control");
    let _ = focus(&mut app, control);

    // The person carries on typing into the buffer: a word, then a space.
    let _ =
        app.run_headless_full_frame(full_frame_input(vec![egui::Event::Text("hi".to_string())]));
    let _ = app.run_headless_full_frame(full_frame_input(vec![
        egui::Event::Key {
            key: egui::Key::Space,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        },
        egui::Event::Text(" ".to_string()),
    ]));
    let after = app.run_headless_full_frame(full_frame_input(Vec::new()));

    assert!(
        app.runtime_snapshot().active_buffer_projection.dirty,
        "typing into the editor was swallowed because a rail button still had focus"
    );
    assert!(
        clickable_center(&after, "Card alpha.rs").is_none(),
        "the swallowed space also pressed the button it was meant to be typed past"
    );
}

/// Space presses the control without also typing into the file.
///
/// A real window backend reports a Space press as both a key event and the text
/// it produces. Filtering only the key left the space to be typed -- so opening
/// the canvas with Space put an invisible character into the file the canvas
/// then covered, which is the hardest kind of edit to notice.
#[test]
fn opening_the_canvas_with_space_does_not_type_into_the_open_file() {
    let workspace = workspace_with_files("legion_desktop_canvas_space_route");
    let mut app = open_app(workspace.path(), None);
    open_all_files(&mut app);

    let editor = app.run_headless_full_frame(full_frame_input(Vec::new()));
    let control = accesskit_id(&editor, "Canvas").expect("the rail must publish a Canvas control");
    assert!(
        !app.runtime_snapshot().active_buffer_projection.dirty,
        "the fixture must start clean, or a typed space cannot be detected"
    );

    let _ = focus(&mut app, control);
    let _ = app.run_headless_full_frame(full_frame_input(vec![
        egui::Event::Key {
            key: egui::Key::Space,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        },
        egui::Event::Text(" ".to_string()),
        egui::Event::Key {
            key: egui::Key::Space,
            physical_key: None,
            pressed: false,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        },
    ]));
    let opened = app.run_headless_full_frame(full_frame_input(Vec::new()));

    assert!(
        !app.runtime_snapshot().active_buffer_projection.dirty,
        "Space on the focused Canvas control typed a space into the open file"
    );
    assert!(
        clickable_center(&opened, "Card alpha.rs").is_some(),
        "Space must still press the control; withholding the character is only half of          what it is for"
    );
}

/// The keyboard route into the canvas does not edit the file it replaces.
///
/// Tab to the Canvas rail control and press Enter: the editor keyboard handler
/// ran first and inserted a newline into the open buffer, and the control was
/// activated afterwards during rendering. So the standard keyboard route into
/// another surface edited the file every single time -- silently, because the
/// file it edited is the one being replaced on screen.
#[test]
fn opening_the_canvas_from_the_keyboard_does_not_edit_the_open_file() {
    let workspace = workspace_with_files("legion_desktop_canvas_enter_route");
    let mut app = open_app(workspace.path(), None);
    open_all_files(&mut app);

    let editor = app.run_headless_full_frame(full_frame_input(Vec::new()));
    // Dirtiness rather than a line count: an inserted newline makes the buffer
    // dirty, and "the file was edited" is the claim under test.
    let before = app.runtime_snapshot().active_buffer_projection.dirty;
    assert!(
        !before,
        "the fixture must start with a clean buffer, or an edit cannot be detected"
    );
    let control = accesskit_id(&editor, "Canvas").expect("the rail must publish a Canvas control");

    let _ = focus(&mut app, control);
    let opened = press_key(&mut app, egui::Key::Enter, egui::Modifiers::NONE);

    assert!(
        !app.runtime_snapshot().active_buffer_projection.dirty,
        "Enter on the focused Canvas control edited the open file, which is the file the          canvas then covered up"
    );
    assert!(
        clickable_center(&opened, "Card alpha.rs").is_some(),
        "the control must still open the canvas; suppressing the newline is only half of          what Enter is for here"
    );
}

/// Enter on the canvas, with nothing focused, still belongs to the canvas.
///
/// The card test above cannot see this on its own: Enter on a focused card
/// switches to that card's file, which lands on the active buffer after the
/// diagnostic navigation does and hides it. With nothing focused there is no
/// second writer, so what `ProblemActivate` did is visible -- it opened the
/// diagnosed file and moved the cursor there, behind a surface somebody was
/// arranging.
#[test]
fn enter_on_the_canvas_with_nothing_focused_does_not_navigate_to_a_diagnostic() {
    let workspace = workspace_with_files("legion_desktop_canvas_enter_unfocused");
    let mut app = open_app(workspace.path(), None);
    open_all_files(&mut app);

    let gamma = workspace.path().join("gamma.rs");
    let uri = format!(
        "file:///{}",
        gamma
            .to_string_lossy()
            .replace('\\', "/")
            .trim_start_matches('/')
    );
    let app_ref = app.runtime_mut_for_test().app_mut_for_test();
    app_ref
        .open_file(gamma.to_string_lossy())
        .expect("gamma.rs must open");
    let diagnosed = app_ref
        .active_buffer_id()
        .expect("gamma.rs must have a buffer");
    app_ref
        .ingest_lsp_publish_diagnostics_for_buffer(
            diagnosed,
            &serde_json::json!({
                "uri": uri,
                "diagnostics": [{
                    "range": {
                        "start": {"line": 0, "character": 0},
                        "end": {"line": 0, "character": 1}
                    },
                    "severity": 1,
                    "message": "something is wrong in gamma.rs"
                }]
            }),
            false,
            None,
        )
        .expect("diagnostics must be ingested");

    // Alpha last, so the diagnosed file is not already the active one -- a
    // navigation that lands where the cursor already is proves nothing.
    let alpha = workspace.path().join("alpha.rs");
    app.runtime_mut_for_test()
        .app_mut_for_test()
        .open_file(alpha.to_string_lossy())
        .expect("alpha.rs must open");

    let canvas = show_canvas(&mut app);
    assert!(
        clickable_center(&canvas, "Card alpha.rs").is_some(),
        "the canvas must be showing"
    );
    assert!(
        !app.runtime_snapshot()
            .language_tooling_projection
            .problems
            .is_empty(),
        "the fixture must carry a problem, or Enter has nothing to collide with"
    );
    let before = app
        .capture_session_record()
        .expect("record")
        .active_buffer
        .expect("a file is open");

    let _ = press_key(&mut app, egui::Key::Enter, egui::Modifiers::NONE);

    let after = app
        .capture_session_record()
        .expect("record")
        .active_buffer
        .expect("a file is still open");
    assert_eq!(
        before, after,
        "Enter on the canvas opened the diagnosed file; nothing on this surface          asked for that, and the file somebody was working in is now behind it"
    );
}

/// Enter on the canvas belongs to the canvas.
///
/// `!editor_input_enabled` was standing in for "the Problems list has the
/// keyboard", and the canvas turns editor input off by design -- so Enter on a
/// focused card also activated whichever diagnostic happened to be selected,
/// opening its file and moving the cursor behind the surface being arranged.
/// Enter on a card means that card: it switches to the file the card is for,
/// and to nothing else.
#[test]
fn pressing_enter_on_the_canvas_activates_the_card_and_not_a_diagnostic() {
    let workspace = workspace_with_files("legion_desktop_canvas_enter_problem");
    let mut app = open_app(workspace.path(), None);
    open_all_files(&mut app);

    // Diagnostics, or the binding under test never fires and the assertions
    // below hold for a canvas that has nothing to collide with.
    let gamma = workspace.path().join("gamma.rs");
    let uri = format!(
        "file:///{}",
        gamma
            .to_string_lossy()
            .replace('\\', "/")
            .trim_start_matches('/')
    );
    let app_ref = app.runtime_mut_for_test().app_mut_for_test();
    app_ref
        .open_file(gamma.to_string_lossy())
        .expect("gamma.rs must open");
    let diagnosed = app_ref
        .active_buffer_id()
        .expect("gamma.rs must have a buffer");
    app_ref
        .ingest_lsp_publish_diagnostics_for_buffer(
            diagnosed,
            &serde_json::json!({
                "uri": uri,
                "diagnostics": [{
                    "range": {
                        "start": {"line": 0, "character": 0},
                        "end": {"line": 0, "character": 1}
                    },
                    "severity": 1,
                    "message": "something is wrong in gamma.rs"
                }]
            }),
            false,
            None,
        )
        .expect("diagnostics must be ingested");

    let canvas = show_canvas(&mut app);
    let card = accesskit_id(&canvas, "Card alpha.rs").expect("alpha.rs must have a card");
    let _ = focus(&mut app, card);
    assert!(
        !app.runtime_snapshot()
            .language_tooling_projection
            .problems
            .is_empty(),
        "the fixture must carry a problem, or Enter has nothing to collide with and          this test passes whatever the gate does"
    );
    let after_frame = press_key(&mut app, egui::Key::Enter, egui::Modifiers::NONE);
    let record = app
        .capture_session_record()
        .expect("the runtime must be able to capture a session record");

    let active = record
        .active_buffer
        .expect("a file is open, so something must be active");
    let path = record
        .open_tabs
        .iter()
        .find(|tab| tab.buffer_id == Some(active))
        .and_then(|tab| tab.path.as_ref())
        .map(|path| path.0.clone())
        .expect("the active buffer must belong to an open tab with a path");
    assert!(
        path.ends_with("alpha.rs"),
        "Enter on the card for alpha.rs made {path} the active file; the canvas was          arranging one thing and something else navigated"
    );
    assert!(
        clickable_center(&after_frame, "Card alpha.rs").is_some(),
        "the canvas must still be on screen after Enter"
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

/// Whether the node with this label reports itself as selected.
fn node_is_selected(output: &egui::FullOutput, label: &str) -> Option<bool> {
    output
        .platform_output
        .accesskit_update
        .as_ref()
        .and_then(|update| {
            update
                .nodes
                .iter()
                .find_map(|(_, node)| (node.label() == Some(label)).then(|| node.is_selected()))
        })
        .flatten()
}

/// Arming a connection source is announced, not just drawn.
///
/// Activating an output port is the first half of a two-step gesture, and the
/// only report that it took was a rubber band drawn to the pointer -- which is
/// exactly what a keyboard or screen-reader user does not have. So the first
/// step was indistinguishable from a control that did nothing, and nothing on
/// the target ports said that activating one would now complete a connection.
#[test]
fn arming_a_connection_source_is_announced_in_the_tree() {
    let workspace = workspace_with_files("legion_desktop_canvas_armed_a11y");
    let mut app = open_app(workspace.path(), None);
    open_all_files(&mut app);
    let canvas = show_canvas(&mut app);

    assert_eq!(
        node_is_selected(&canvas, "Connect from alpha.rs"),
        Some(false),
        "a port that has not been armed must not report itself as selected"
    );

    let source = accesskit_id(&canvas, "Connect from alpha.rs")
        .expect("each card must publish an outgoing connection port");
    let armed = activate(&mut app, source);

    assert_eq!(
        node_is_selected(&armed, "Connect from alpha.rs"),
        Some(true),
        "the armed source does not report the state it is in, so a successful first          step reads exactly like a control that did nothing"
    );
    let description = node_description(&armed, "Connect from alpha.rs").unwrap_or_default();
    assert!(
        description.contains("Escape"),
        "the armed port must say how to give up on the connection; it said {description:?}"
    );
    // The armed card's own target says it cannot complete the connection, since
    // the activation rejects a self-edge: a control that promises an action it
    // will not perform is worse than one that says it is unavailable, and worst
    // for the reader who cannot see that nothing happened.
    let itself = node_description(&armed, "Connect to alpha.rs").unwrap_or_default();
    assert!(
        !itself.contains("Activating connects"),
        "the armed card's own target offers a connection the activation refuses: {itself:?}"
    );
    assert!(
        itself.contains("connection source"),
        "the armed card's own target must say why it cannot be chosen: {itself:?}"
    );

    let target = node_description(&armed, "Connect to beta.rs").unwrap_or_default();
    assert!(
        target.contains("alpha.rs"),
        "a target port must say what activating it will now do, and which card it would          connect; it said {target:?}"
    );

    // And Escape puts it back, rather than leaving a state nobody can clear.
    let cleared = press_key(&mut app, egui::Key::Escape, egui::Modifiers::NONE);
    assert_eq!(
        node_is_selected(&cleared, "Connect from alpha.rs"),
        Some(false),
        "Escape cleared the armed edge without clearing what the tree says about it"
    );
    let target = node_description(&cleared, "Connect to beta.rs").unwrap_or_default();
    assert!(
        !target.contains("Activating connects"),
        "a target port still offers to complete a connection nobody is drawing: {target:?}"
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
    // Contains rather than equals: with no source armed the port also explains
    // that it cannot be activated yet, and that hint is added to the connection
    // list rather than replacing it.
    let other_end = node_description(&connected, "Connect to beta.rs").unwrap_or_default();
    assert!(
        other_end.contains("Connected from alpha.rs"),
        "the other end must report the connection too, from its own direction; it said {other_end:?}"
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
    let last = names.last().expect("the fixture opens files");
    let newest = clickable_center(&settled, &format!("Card {last}"))
        .unwrap_or_else(|| panic!("{last} is open and has no card at all"));
    assert!(
        panel.contains(newest),
        "{last} was opened while the canvas was showing and its card sits at {newest:?}, \
         outside the canvas region {panel:?}; the file somebody just opened is the one \
         they are looking for"
    );

    // The rest are placed and reachable rather than all on screen at once.
    //
    // They cannot all be on screen at once: eight cards of this size do not fit
    // a panel this size at any zoom somebody can read, and the view used to grow
    // to hold them -- which shrank every card already there each time a file was
    // opened. What has to be true is that no card is lost, so the fit control,
    // which is the answer to "where did it go", brings every one of them back.
    let fit = clickable_center(&settled, "Fit all cards")
        .expect("a canvas that can put a card off screen must offer a way back to it");
    let after_fit = click_at(&mut app, fit);
    let panel = app
        .last_editor_rect_for_test()
        .expect("the canvas must report the region it drew into");
    for name in &names {
        let centre = clickable_center(&after_fit, &format!("Card {name}"))
            .unwrap_or_else(|| panic!("{name} is open and has no card at all"));
        assert!(
            panel.contains(centre),
            "after fitting the view to the arrangement, {name} is still at {centre:?}, \
             outside {panel:?} -- the control that answers \"where did my card go\" \
             did not answer it"
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
    // The armed port now leads with the state it is in, so the fixture check is
    // on the connection half of what it says.
    let description = node_description(&armed, "Connect from alpha.rs").unwrap_or_default();
    assert!(
        description.ends_with("No connections"),
        "the fixture must start with nothing connected, or this proves nothing; the port          said {description:?}"
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
