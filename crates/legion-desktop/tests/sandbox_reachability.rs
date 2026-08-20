//! Checklist row 13: "Sandbox panel: Windows caveats visible if Job Object-only".
//!
//! Never exercised in a windowed session. Every existing sandbox test reads
//! `DesktopProjectionViewModel::sandbox_rows` directly, which answers "does the
//! view model contain an honest string" and not the question the checklist
//! asks, which is whether a person looking at the panel can see it.
//!
//! Those two came apart. The panel renders only the first few rows and
//! collapses the rest into an "N more rows" line. On Windows the model produced
//! eleven rows and the one saying filesystem and network are not enforced was
//! the ninth, so the rendered panel showed `RestrictedToken`, `profile compiled
//! fail-closed`, and a claim that filesystem scope was "limited to workspace
//! root" — while
//! `docs/SECURITY.md` records that the Windows spawn path is a Job Object with
//! `KILL_ON_JOB_CLOSE` and restricts neither filesystem nor network.
//!
//! A sandbox panel that overstates its containment is worse than no panel: it
//! is the surface a person consults before letting an agent run.
//!
//! Everything here is driven through the rendered accessibility tree — click
//! the real control at its real centre, then read what the frame exposes — and
//! every assertion is preceded by a check that the click actually landed, so a
//! missed click fails loudly instead of passing quietly.

use std::path::Path;

mod common;
use common::{TempWorkspace, click_at, clickable_center, full_frame_input, rendered_text};

use legion_desktop::workflow::{DesktopEframeApp, DesktopLaunchConfig, DesktopRuntime};
use legion_protocol::DelegatedTaskRuntimeActivationState;
use legion_ui::DockMode;

fn open_app(root: &Path) -> DesktopEframeApp {
    let runtime = DesktopRuntime::open(DesktopLaunchConfig::new(root.to_path_buf(), None))
        .expect("desktop runtime should open workspace");
    DesktopEframeApp::new(runtime)
}

/// Centre of the multiline task-draft field.
///
/// Found by role rather than label: the field is described by a neighbouring
/// `Task description` label through `labelled_by`, so it carries no label of
/// its own and `clickable_center` cannot see it.
fn task_draft_center(output: &egui::FullOutput) -> Option<egui::Pos2> {
    output
        .platform_output
        .accesskit_update
        .as_ref()?
        .nodes
        .iter()
        .find_map(|(_id, node)| {
            (node.role() == egui::accesskit::Role::MultilineTextInput)
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

/// Switch to Delegate through the mode switch, confirming the transition.
///
/// Manual to Delegate is a `Confirm` transition, so the dialog is part of the
/// path a person walks and part of what this drives.
fn enter_delegate_mode(app: &mut DesktopEframeApp) -> egui::FullOutput {
    let primed = app.run_headless_full_frame(full_frame_input(Vec::new()));
    let delegate = clickable_center(&primed, "Delegate")
        .expect("the mode switch must offer a `Delegate` control");
    let opened = click_at(app, delegate);
    let confirm = clickable_center(&opened, "Confirm")
        .expect("switching into Delegate must raise a confirmation dialog with a `Confirm` button");
    let confirmed = click_at(app, confirm);

    // Proof the two clicks landed. Without this the assertions below would
    // pass just as happily on a frame that never left Manual mode.
    assert_eq!(
        app.runtime_snapshot().product_mode,
        DockMode::Delegate,
        "clicking `Delegate` and confirming did not put the shell in Delegate mode"
    );
    confirmed
}

/// Start a delegated task so a sandbox actually exists to describe.
fn start_delegated_task(app: &mut DesktopEframeApp) -> egui::FullOutput {
    let confirmed = enter_delegate_mode(app);
    let field = task_draft_center(&confirmed)
        .expect("the Delegate surface must render a task-description field");
    let _ = click_at(app, field);
    let typed = app.run_headless_full_frame(full_frame_input(vec![egui::Event::Text(
        "review the sandbox boundary".to_string(),
    )]));

    // The submit button is disabled until the draft is non-empty, so finding it
    // as a *clickable* node is itself proof the typing reached the field.
    let submit = clickable_center(&typed, "Delegate task").expect(
        "`Delegate task` must become clickable once a task is typed; it did not, so the \
         draft field never received the text",
    );
    let submitted = click_at(app, submit);

    assert_ne!(
        app.runtime_snapshot()
            .delegated_task_projection
            .runtime_activation,
        DelegatedTaskRuntimeActivationState::NotEncoded,
        "clicking `Delegate task` did not start a task, so nothing below is about a real sandbox"
    );
    submitted
}

/// Every rendered line, joined, for substring assertions.
fn rendered(output: &egui::FullOutput) -> String {
    rendered_text(output).join("\n")
}

/// The panel exists and is reachable, and says so honestly before a task runs.
#[test]
fn the_sandbox_panel_is_reachable_from_the_rendered_ui() {
    let workspace = TempWorkspace::new("legion_desktop_sandbox_reachability");
    workspace.write("main.rs", "fn main() {}\n");
    let mut app = open_app(workspace.path());

    let confirmed = enter_delegate_mode(&mut app);
    let text = rendered(&confirmed);

    assert!(
        text.contains("Sandbox"),
        "the Delegate surface renders no `Sandbox` section at all, so checklist row 13 has \
         nothing to exercise. Rendered text was: {text}"
    );
    assert!(
        text.contains("Sandbox starts after the task is submitted."),
        "before a task runs the panel must say no sandbox exists yet rather than leave the \
         section blank. Rendered text was: {text}"
    );
}

/// Once a task is running, the rendered panel names the backend *and* the limit.
///
/// The backend row is asserted first and separately: it is what proves the
/// panel switched from the pre-task copy to the live one, so a failure on the
/// limits row below is a failure of the limits row and not of the drive.
#[test]
fn a_running_sandbox_states_its_platform_limits_where_they_can_be_read() {
    let workspace = TempWorkspace::new("legion_desktop_sandbox_reachability");
    workspace.write("main.rs", "fn main() {}\n");
    let mut app = open_app(workspace.path());

    let submitted = start_delegated_task(&mut app);
    let text = rendered(&submitted);

    assert!(
        text.contains("sandbox backend:"),
        "the running sandbox panel never rendered a backend row, so the panel under test is \
         not the live one. Rendered text was: {text}"
    );
    assert!(
        text.contains("sandbox limits:"),
        "the rendered sandbox panel states a backend but never states what that backend does \
         not contain. The limitation exists in the view model; it was simply below the row \
         budget the panel draws, which is the same as not saying it. Rendered text was: {text}"
    );
}

/// On a Job-Object-only host the panel must say filesystem and network are out.
///
/// Job-Object-only is what `legion-sandbox` does on Windows: process lifetime
/// and nothing more. This is the checklist row in one assertion.
#[cfg(target_os = "windows")]
#[test]
fn windows_caveats_are_visible_when_the_sandbox_is_job_object_only() {
    let workspace = TempWorkspace::new("legion_desktop_sandbox_reachability");
    workspace.write("main.rs", "fn main() {}\n");
    let mut app = open_app(workspace.path());

    let submitted = start_delegated_task(&mut app);
    let text = rendered(&submitted);
    assert!(
        text.contains("sandbox backend:"),
        "the live sandbox panel did not render; nothing below would mean anything. \
         Rendered text was: {text}"
    );

    let limits = rendered_text(&submitted)
        .into_iter()
        .find(|row| row.starts_with("sandbox limits: "))
        .unwrap_or_else(|| {
            panic!("no `sandbox limits:` row is rendered on Windows. Rendered text was: {text}")
        });
    let lowered = limits.to_lowercase();
    for term in ["job object", "filesystem", "network", "not"] {
        assert!(
            lowered.contains(term),
            "the visible Windows limitation row does not mention `{term}`: {limits}"
        );
    }
}

// A rendered counterpart to `windows_rows_do_not_claim_filesystem_or_egress_containment`
// was written here and then deleted. It asserted that the frame never shows
// "filesystem scope limited to workspace root" — and it passed against the
// unfixed code, because that line was one of the six the panel hid behind
// "N more rows". A test that cannot fail on the defect it names is worse than
// no test, so the "do not claim what you do not enforce" property is held one
// level down, on the full row list, where a regression is visible. What is
// worth asserting *here* is the part only a rendered frame can answer: that the
// limitation is drawn rather than merely computed. That is the test above.
