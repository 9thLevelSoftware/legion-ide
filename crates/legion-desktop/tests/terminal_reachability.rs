//! Checklist row 4: can a person get a working terminal from the rendered UI?
//!
//! Never exercised in a windowed session. The 2026-08-17 journal records the
//! panel showing `status=disabled …` and the row as "Not exercised", so what
//! happens when someone actually asks for a terminal has never been checked
//! from the outside.
//!
//! The runtime enables lazily on first launch behind a workspace-trust gate, so
//! "disabled" before launch is correct rather than broken. This asserts the
//! part that matters: that a launch reaches a real PTY and its output comes
//! back to the projection a person is looking at.

use std::path::Path;
use std::time::{Duration, Instant};

mod common;
use common::TempWorkspace;

use legion_desktop::{
    bridge::DesktopAction,
    workflow::{DesktopEframeApp, DesktopLaunchConfig, DesktopRuntime},
};

fn open_runtime(root: &Path) -> DesktopRuntime {
    DesktopRuntime::open(DesktopLaunchConfig::new(root.to_path_buf(), None))
        .expect("desktop runtime should open workspace")
}

/// Terminal status as the projection reports it.
fn terminal_status(runtime: &DesktopRuntime) -> String {
    let snapshot = runtime.projection_snapshot();
    format!(
        "{:?}: {}",
        snapshot.terminal_panel_projection.status.kind,
        snapshot.terminal_panel_projection.status.message
    )
}

#[test]
fn a_terminal_launch_from_the_ui_reaches_a_real_session() {
    let workspace = TempWorkspace::new("legion_desktop_terminal_reachability");
    workspace.write("main.rs", "fn main() {}\n");
    let mut runtime = open_runtime(workspace.path());

    // Before launch the runtime is deliberately disabled: it enables on first
    // launch behind the workspace-trust gate. That is the state the dogfood
    // journal saw, and on its own it is not a defect.
    let before = terminal_status(&runtime);

    let _ = runtime.handle_action(DesktopAction::TerminalLaunch {
        command_label: "cargo --version".to_string(),
    });

    let after = terminal_status(&runtime);
    assert_ne!(
        before, after,
        "launching a terminal must change its status; it stayed at `{before}`, \
         which is what a user reads as a dead button"
    );
    assert!(
        !after.to_lowercase().contains("disabled"),
        "terminal still reports disabled after an explicit launch: {after}"
    );
}

#[test]
fn terminal_output_reaches_the_projection_a_person_reads() {
    let workspace = TempWorkspace::new("legion_desktop_terminal_reachability");
    workspace.write("main.rs", "fn main() {}\n");
    let mut runtime = open_runtime(workspace.path());

    let _ = runtime.handle_action(DesktopAction::TerminalLaunch {
        command_label: "cargo --version".to_string(),
    });

    // A PTY is a real process, so this polls rather than asserting on the
    // first frame. The bound is generous because the assertion is "output
    // arrives at all", not "output arrives quickly".
    let deadline = Instant::now() + Duration::from_secs(20);
    let mut lines = 0;
    while Instant::now() < deadline {
        let _ = runtime.handle_action(DesktopAction::TerminalOutputPoll);
        lines = runtime
            .projection_snapshot()
            .terminal_panel_projection
            .output_rows
            .len();
        if lines > 0 {
            break;
        }
        std::thread::sleep(Duration::from_millis(200));
    }

    assert!(
        lines > 0,
        "no terminal output reached the projection within 20s; status={}. \
         A terminal panel that launches and never shows a line is the same \
         defect shape as an explorer row that selects and never opens.",
        terminal_status(&runtime)
    );
}

/// The "Run cargo test" button must actually run something.
///
/// `TerminalLaunch { command_label }` spawns the shell and uses the label only
/// for the status line and the audit record. The command is never written to
/// the PTY, so the one control in the UI that offers to run a command opens a
/// shell instead — while the status bar reads "Terminal running: cargo test",
/// which is a statement the product cannot support.
#[test]
fn a_launched_command_actually_runs_in_the_terminal() {
    let workspace = TempWorkspace::new("legion_desktop_terminal_command");
    let mut runtime = open_runtime(workspace.path());

    // `echo` rather than `cargo`: it is present on every supported platform,
    // returns instantly, and its output is unmistakable in the scrollback.
    let marker = "legion-terminal-marker-9f3c";
    // The pair the Tests button now pushes: launch opens the shell, input runs
    // the command. Launch alone leaves a prompt sitting there.
    let _ = runtime.handle_action(DesktopAction::TerminalLaunch {
        command_label: format!("echo {marker}"),
    });
    let _ = runtime.handle_action(DesktopAction::TerminalInput {
        payload: format!("echo {marker}\r"),
    });

    // Counted, not merely found. The shell echoes the line it was sent, so a
    // marker appearing *once* proves only that the text reached the PTY. It
    // appears a second time when the shell actually executes `echo`, and that
    // second occurrence is the only evidence that the command ran rather than
    // being typed into a prompt and left there.
    let deadline = Instant::now() + Duration::from_secs(20);
    let mut occurrences = 0;
    while Instant::now() < deadline && occurrences < 2 {
        let _ = runtime.handle_action(DesktopAction::TerminalOutputPoll);
        occurrences = runtime
            .projection_snapshot()
            .terminal_panel_projection
            .output_rows
            .iter()
            .map(|row| row.redacted_payload.matches(marker).count())
            .sum();
        if occurrences < 2 {
            std::thread::sleep(Duration::from_millis(200));
        }
    }

    let status = terminal_status(&runtime);
    assert!(
        occurrences >= 2,
        "the launched command was not executed: `{marker}` appeared {occurrences}          time(s) in 20s (1 = echoed by the shell but never run), while the panel          reports `{status}`. A control that reports running a command it never          ran is worse than one that does nothing, because the status line is          what a user checks."
    );
}

/// Clicking `Run cargo test` sends the command to the terminal.
///
/// End-to-end through the rendered UI, because the defect this guards was
/// invisible at every other level: the action fired, the intent translated, the
/// PTY spawned, the status line said "Terminal running: cargo test" -- and the
/// command was never sent.
///
/// This asserts the command *reaches the PTY*, not that cargo finishes; a real
/// `cargo test` compiles for minutes and is nobody's idea of a unit test. That
/// the shell then executes what it is sent is pinned by
/// `a_launched_command_actually_runs_in_the_terminal`, which counts the marker
/// twice.
#[test]
fn clicking_run_cargo_test_sends_the_command_to_the_terminal() {
    let workspace = TempWorkspace::new("legion_desktop_run_cargo_button");
    workspace.write(
        "main.rs",
        "fn main() {}
",
    );
    let runtime = open_runtime(workspace.path());
    let mut app = DesktopEframeApp::new(runtime);

    let primed = app.run_headless_full_frame(full_frame_input(Vec::new()));
    let tests = clickable_center(&primed, "Tests");
    let on_tests = click_at(&mut app, tests);
    let run = clickable_center(&on_tests, "Run cargo test");
    let _ = click_at(&mut app, run);

    // The fixture has no Cargo.toml, so a cargo that really ran says so. That
    // error is the proof: it can only come from the process, never from the
    // status line the app writes for itself.
    let deadline = Instant::now() + Duration::from_secs(30);
    let mut transcript = String::new();
    let mut ran = false;
    while Instant::now() < deadline && !ran {
        let _ = app.run_headless_full_frame(full_frame_input(Vec::new()));
        transcript = app
            .runtime_snapshot()
            .terminal_panel_projection
            .output_rows
            .iter()
            .map(|row| row.redacted_payload.as_str())
            .collect::<Vec<_>>()
            .join("");
        ran = transcript.contains("cargo test");
        if !ran {
            std::thread::sleep(Duration::from_millis(250));
        }
    }

    assert!(
        ran,
        "clicking `Run cargo test` never sent the command in 30s. The terminal          transcript was: {transcript:?}"
    );
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

fn clickable_center(output: &egui::FullOutput, label: &str) -> egui::Pos2 {
    output
        .platform_output
        .accesskit_update
        .as_ref()
        .expect("full headless frames should expose the accessibility tree")
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
        .unwrap_or_else(|| panic!("no clickable control labelled `{label}`"))
}

fn click_at(app: &mut DesktopEframeApp, pos: egui::Pos2) -> egui::FullOutput {
    for pressed in [true, false] {
        let _ = app.run_headless_full_frame(full_frame_input(vec![
            egui::Event::PointerMoved(pos),
            egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed,
                modifiers: egui::Modifiers::default(),
            },
        ]));
    }
    app.run_headless_full_frame(full_frame_input(Vec::new()))
}

/// A terminal opened against a project starts *in* that project.
///
/// The launch policy has always declared `cwd_policy: "workspace-root"`, and
/// until the launch request carried a path nothing made that true: the PTY
/// inherited the process working directory, so a terminal opened against an
/// open project started wherever the app happened to be launched from. The
/// `legion-terminal` boundary said so in a comment rather than papering over
/// it — "documented to avoid mistaking validation for enforcement."
///
/// It matters for the button above: `cargo test` run in the wrong tree either
/// tests the wrong project or fails to find one.
#[test]
fn a_terminal_opens_in_the_workspace_root() {
    let workspace = TempWorkspace::new("legion_desktop_terminal_cwd");
    workspace.write("marker-file.txt", "present\n");
    let mut runtime = open_runtime(workspace.path());

    let _ = runtime.handle_action(DesktopAction::TerminalLaunch {
        command_label: "dir".to_string(),
    });
    // `cd` with no argument prints the working directory on cmd.exe; `pwd`
    // does the same on a POSIX shell. Sending both means this asserts the same
    // property on either platform without branching on the shell.
    let probe = if cfg!(windows) { "cd\r" } else { "pwd\r" };
    let _ = runtime.handle_action(DesktopAction::TerminalInput {
        payload: probe.to_string(),
    });

    let expected = workspace
        .path()
        .file_name()
        .expect("temp workspace has a directory name")
        .to_string_lossy()
        .to_string();

    let deadline = Instant::now() + Duration::from_secs(20);
    let mut transcript = String::new();
    let mut in_workspace = false;
    while Instant::now() < deadline && !in_workspace {
        let _ = runtime.handle_action(DesktopAction::TerminalOutputPoll);
        transcript = runtime
            .projection_snapshot()
            .terminal_panel_projection
            .output_rows
            .iter()
            .map(|row| row.redacted_payload.as_str())
            .collect::<Vec<_>>()
            .join("");
        in_workspace = transcript.contains(&expected);
        if !in_workspace {
            std::thread::sleep(Duration::from_millis(200));
        }
    }

    assert!(
        in_workspace,
        "the terminal did not start in the workspace root: expected the path to \
         contain `{expected}`, transcript was {transcript:?}"
    );
}
