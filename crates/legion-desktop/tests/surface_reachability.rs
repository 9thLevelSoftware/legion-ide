//! Every activity-rail surface must be reachable by clicking it.
//!
//! The dogfood journal for 2026-08-17 records the owner's first windowed
//! session as "nothing in the app seems to really work" — the file tree drew
//! correctly and clicking a row opened nothing. That defect was invisible to
//! every existing test because projection tests cannot see rendering or
//! hit-testing, and the interactive checklist never named the step.
//!
//! Ten of the thirteen checklist rows have still never been exercised in a
//! windowed session. This suite starts closing that by driving the primary
//! navigation column the way a person does: find the control in the real
//! accessibility tree, click its real centre, and assert the surface it
//! promises actually appears.

use std::path::Path;

mod common;
use common::TempWorkspace;

use legion_desktop::workflow::{DesktopEframeApp, DesktopLaunchConfig, DesktopRuntime};

fn open_app(root: &Path) -> DesktopEframeApp {
    let runtime = DesktopRuntime::open(DesktopLaunchConfig::new(root.to_path_buf(), None))
        .expect("desktop runtime should open workspace");
    DesktopEframeApp::new(runtime)
}

fn full_frame_input(events: Vec<egui::Event>) -> egui::RawInput {
    egui::RawInput {
        focused: true,
        screen_rect: Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(1_440.0, 900.0),
        )),
        events,
        ..egui::RawInput::default()
    }
}

/// Every piece of text the rendered frame exposes to assistive technology.
///
/// Reads `label` **or** `value`. egui puts a control's explicit label in the
/// first and static text in the second, so a label-only reader sees buttons and
/// misses every heading, hint and empty state — which made the command palette
/// look like it rendered nothing at all when it renders sixteen nodes.
fn rendered_text(output: &egui::FullOutput) -> Vec<String> {
    output
        .platform_output
        .accesskit_update
        .as_ref()
        .expect("full headless frames should expose the accessibility tree")
        .nodes
        .iter()
        .filter_map(|(_id, node)| {
            node.label()
                .map(str::to_string)
                .or_else(|| node.value().map(str::to_string))
        })
        .collect()
}

/// Centre of the clickable control carrying `label`.
fn clickable_center(output: &egui::FullOutput, label: &str) -> Option<egui::Pos2> {
    output
        .platform_output
        .accesskit_update
        .as_ref()?
        .nodes
        .iter()
        .find_map(|(_id, node)| {
            (node.label() == Some(label) && node.supports_action(egui::accesskit::Action::Click))
                .then(|| node.bounds())
                .flatten()
        })
        .map(|bounds| {
            egui::pos2(
                ((bounds.x0 + bounds.x1) * 0.5) as f32,
                ((bounds.y0 + bounds.y1) * 0.5) as f32,
            )
        })
}

/// Click at `pos` and settle: press, release, then one frame for the action.
fn click_at(app: &mut DesktopEframeApp, pos: egui::Pos2) -> egui::FullOutput {
    let _ = app.run_headless_full_frame(full_frame_input(vec![
        egui::Event::PointerMoved(pos),
        egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed: true,
            modifiers: egui::Modifiers::default(),
        },
    ]));
    let _ = app.run_headless_full_frame(full_frame_input(vec![
        egui::Event::PointerMoved(pos),
        egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: egui::Modifiers::default(),
        },
    ]));
    app.run_headless_full_frame(full_frame_input(Vec::new()))
}

/// The six activity surfaces the rail offers, by their accessibility labels.
const RAIL_SURFACES: [&str; 6] = [
    "Explorer",
    "Search",
    "Symbols",
    "Source Control",
    "Tests",
    "Run and Debug",
];

#[test]
fn every_activity_rail_control_is_present_and_clickable() {
    let workspace = TempWorkspace::new("legion_desktop_surface_reachability");
    workspace.write("main.rs", "fn main() {}\n");
    let mut app = open_app(workspace.path());
    let primed = app.run_headless_full_frame(full_frame_input(Vec::new()));

    let mut missing = Vec::new();
    for surface in RAIL_SURFACES {
        if clickable_center(&primed, surface).is_none() {
            missing.push(surface);
        }
    }
    assert!(
        missing.is_empty(),
        "activity rail controls absent or not clickable: {missing:?}. \
         The rail is the primary navigation column; a surface with no reachable \
         control is a surface a person cannot get to at all."
    );
}

#[test]
fn clicking_each_rail_surface_changes_what_is_rendered() {
    let workspace = TempWorkspace::new("legion_desktop_surface_reachability");
    workspace.write("main.rs", "fn main() {}\n");
    let mut app = open_app(workspace.path());
    let primed = app.run_headless_full_frame(full_frame_input(Vec::new()));

    // Baseline: what the shell shows on the default surface.
    let baseline = rendered_text(&primed);

    let mut inert = Vec::new();
    for surface in RAIL_SURFACES {
        let mut app = open_app(workspace.path());
        let primed = app.run_headless_full_frame(full_frame_input(Vec::new()));
        let Some(pos) = clickable_center(&primed, surface) else {
            continue; // reported by the reachability test above
        };
        let after = rendered_text(&click_at(&mut app, pos));

        // A surface that renders exactly what the default one did has not
        // shown the user anything, whatever its internal selection state says.
        let gained: Vec<&String> = after.iter().filter(|l| !baseline.contains(l)).collect();
        if gained.is_empty() && surface != "Explorer" {
            inert.push(surface);
        }
    }
    assert!(
        inert.is_empty(),
        "rail surfaces that rendered nothing new when clicked: {inert:?}. \
         Selecting a surface that draws the same frame is the defect shape of \
         D1: the control responds, the state changes, and the user sees no \
         difference."
    );
}
