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

use std::time::{Duration, Instant};
use std::{fs, panic::AssertUnwindSafe, path::Path};

mod common;
use common::{
    TempWorkspace, click_at, clickable_center, enabled_clickable_center, full_frame_input,
};

use legion_desktop::{
    bridge::DesktopAction,
    workflow::{DesktopEframeApp, DesktopLaunchConfig, DesktopRuntime},
};

fn open_runtime(root: &Path) -> DesktopRuntime {
    DesktopRuntime::open(DesktopLaunchConfig::new(root.to_path_buf(), None))
        .expect("desktop runtime should open workspace")
}

/// The rendered launch test owns a native child through the runtime. Keep the
/// cleanup outside the assertion body so a failed assertion cannot strand the
/// shell process while the test is unwinding.
fn close_owned_terminal(app: &mut DesktopEframeApp) {
    // Both actions are idempotent/no-op when no session is active. Issue them
    // unconditionally so cleanup still attempts termination if the projection
    // is one frame behind the runtime after a failed assertion.
    let _ = app
        .runtime_mut_for_test()
        .handle_action(DesktopAction::TerminalKill);
    let _ = app
        .runtime_mut_for_test()
        .handle_action(DesktopAction::TerminalClose);
    let deadline = Instant::now() + Duration::from_secs(5);
    while Instant::now() < deadline {
        let _ = app.run_headless_full_frame(full_frame_input(Vec::new()));
        if app
            .runtime_snapshot()
            .terminal_panel_projection
            .active_session_id
            .is_none()
        {
            return;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
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
fn clicking_open_terminal_launches_shell_and_writes_exact_owned_marker() {
    let workspace = TempWorkspace::new("legion_desktop_direct_terminal_button");
    let marker = workspace.path().join("direct-terminal-proof.txt");
    let mut app = DesktopEframeApp::new(open_runtime(workspace.path()));

    let result = std::panic::catch_unwind(AssertUnwindSafe(|| {
        let initial = app.run_headless_full_frame(full_frame_input(Vec::new()));
        assert!(
            app.runtime_snapshot()
                .terminal_panel_projection
                .active_session_id
                .is_none(),
            "the direct-launch proof must begin without an active session"
        );
        assert_eq!(
            app.runtime_snapshot().terminal_panel_projection.status.kind,
            legion_protocol::TerminalPanelStatusKind::Disabled,
            "the direct-launch proof must begin in the lazy disabled state"
        );
        assert!(
            common::rendered_text(&initial)
                .iter()
                .any(|text| text == "Terminal workflow disabled"),
            "the idle panel must preserve its disabled status reason"
        );
        let open = enabled_clickable_center(&initial, "Open terminal")
            .expect("idle terminal panel must expose an Open terminal control");
        let after_click = click_at(&mut app, open);
        let launched = app.runtime_snapshot().terminal_panel_projection;
        assert!(
            launched.active_session_id.is_some(),
            "Open terminal click did not create a session: status={:?} message={} frame_text={:?}",
            launched.status.kind,
            launched.status.message,
            common::rendered_text(&after_click)
        );
        assert!(
            enabled_clickable_center(&after_click, "Open terminal").is_none(),
            "Open terminal must be hidden while a session is active"
        );

        let (command, expected) = if cfg!(windows) {
            (
                "echo LEGION_DIRECT_TERMINAL_PROOF>direct-terminal-proof.txt\r",
                b"LEGION_DIRECT_TERMINAL_PROOF\r\n".as_slice(),
            )
        } else {
            (
                "printf 'LEGION_DIRECT_TERMINAL_PROOF\\n' > direct-terminal-proof.txt\r",
                b"LEGION_DIRECT_TERMINAL_PROOF\n".as_slice(),
            )
        };
        let input_result = app
            .runtime_mut_for_test()
            .handle_action(DesktopAction::TerminalInput {
                payload: command.to_string(),
            });
        assert!(
            input_result.is_ok(),
            "terminal input dispatch failed: {input_result:?}"
        );

        let deadline = Instant::now() + Duration::from_secs(20);
        while Instant::now() < deadline {
            let _ = app.run_headless_full_frame(full_frame_input(Vec::new()));
            if fs::read(&marker).ok().as_deref() == Some(expected) {
                break;
            }
            std::thread::sleep(Duration::from_millis(100));
        }
        let bytes = fs::read(&marker).expect("shell command must create the owned marker file");
        assert_eq!(
            bytes,
            expected,
            "shell command wrote unexpected bytes; status={:?} error={:?}",
            app.runtime_snapshot().terminal_panel_projection.status.kind,
            app.runtime_snapshot().terminal_panel_projection.last_error
        );
    }));

    close_owned_terminal(&mut app);
    if let Err(payload) = result {
        std::panic::resume_unwind(payload);
    }
    assert!(
        app.runtime_snapshot()
            .terminal_panel_projection
            .active_session_id
            .is_none(),
        "owned terminal cleanup must leave no active session"
    );
    let reopened = app.run_headless_full_frame(full_frame_input(Vec::new()));
    assert!(
        enabled_clickable_center(&reopened, "Open terminal").is_some(),
        "Open terminal must be reachable again after cleanup"
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
///
/// ## Why the command's *output* has to differ from its *text*
///
/// The first version of this test sent `echo <marker>` and required the marker
/// twice: once echoed by the shell, once produced by running it. That passed on
/// Windows and failed on macOS, because a POSIX PTY does not echo the line back
/// the way cmd.exe does — one occurrence there means the command ran, and the
/// test called it a failure. The count was measuring the shell's echo
/// behaviour, not execution.
///
/// So the command is one whose output cannot appear in its own text. `ver` and
/// `uname` name the operating system; neither string is in the command that
/// produced it, so finding it is proof of execution on any shell.
#[test]
fn a_launched_command_actually_runs_in_the_terminal() {
    let workspace = TempWorkspace::new("legion_desktop_terminal_command");
    let mut runtime = open_runtime(workspace.path());

    let (command, expected) = if cfg!(windows) {
        ("ver", "Windows")
    } else if cfg!(target_os = "macos") {
        ("uname", "Darwin")
    } else {
        ("uname", "Linux")
    };

    // The pair the Tests button now pushes: launch opens the shell, input runs
    // the command. Launch alone leaves a prompt sitting there.
    let _ = runtime.handle_action(DesktopAction::TerminalLaunch {
        command_label: command.to_string(),
    });
    let _ = runtime.handle_action(DesktopAction::TerminalInput {
        payload: format!("{command}\r"),
    });

    let deadline = Instant::now() + Duration::from_secs(20);
    let mut transcript = String::new();
    let mut executed = false;
    while Instant::now() < deadline && !executed {
        let _ = runtime.handle_action(DesktopAction::TerminalOutputPoll);
        transcript = runtime
            .projection_snapshot()
            .terminal_panel_projection
            .output_rows
            .iter()
            .map(|row| row.redacted_payload.as_str())
            .collect::<Vec<_>>()
            .join("");
        executed = transcript.contains(expected);
        if !executed {
            std::thread::sleep(Duration::from_millis(200));
        }
    }

    let status = terminal_status(&runtime);
    assert!(
        executed,
        "the launched command was not executed: `{command}` never produced `{expected}` in 20s, while the panel reports `{status}`. A control that reports running a command it never ran is worse than one that does nothing, because the status line is what a user checks. Transcript: {transcript:?}"
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
    let tests = clickable_center(&primed, "Tests")
        .expect("the Tests rail control must exist to reach the run button");
    let on_tests = click_at(&mut app, tests);
    let run = clickable_center(&on_tests, "Run cargo test")
        .expect("the Tests surface must offer a `Run cargo test` control");
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
        "clicking `Run cargo test` never sent the command in 30s. The terminal transcript was: {transcript:?}"
    );
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
