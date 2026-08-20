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
use common::{TempWorkspace, click_at, clickable_center, full_frame_input, rendered_text};

use legion_desktop::workflow::{DesktopEframeApp, DesktopLaunchConfig, DesktopRuntime};

fn open_app(root: &Path) -> DesktopEframeApp {
    let runtime = DesktopRuntime::open(DesktopLaunchConfig::new(root.to_path_buf(), None))
        .expect("desktop runtime should open workspace");
    DesktopEframeApp::new(runtime)
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
