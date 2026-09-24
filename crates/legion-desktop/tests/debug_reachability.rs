//! Checklist rows 9-12: can a person debug anything from the rendered UI?
//!
//! Rows 9-12 had never been exercised in a windowed session, and the existing
//! debug suites (`debug_workflow`, `debug_keyboard`, `breakpoint_hit`,
//! `live_continue_auto_poll`) all begin by calling a test seam —
//! `enable_debug_fixture_for_tests` or `enable_debug_live_fake_for_tests` —
//! before they touch anything. That seam was the only caller in the workspace
//! that ever set `DebugWorkflow::runtime_enabled`, so every one of those suites
//! was green while the shipped app answered every Launch with
//! `Denied: Debug runtime is disabled`. The toolbar button existed, responded,
//! wrote a status line, and started nothing.
//!
//! So the rule for this file: **nothing here enables the debug runtime.** These
//! tests open a workspace the way the app does and click what a person clicks,
//! and the runtime has to turn itself on the way it does for a user. The one
//! exception is the live-adapter test, which must widen the adapter allowlist
//! to reach the in-tree CI fake; it is marked where it does so.
//!
//! The fixture tests do pin `DapMode::Fixture`, which chooses *which* runtime
//! answers a launch and does not enable anything. Without it they would be
//! machine-dependent: `LEGION_DAP_MODE` defaults to `auto`, so anywhere
//! `lldb-dap` or `codelldb` is on `PATH` the "this is the fixture path" tests
//! would quietly take the live path and assert about the wrong one.

use std::path::{Path, PathBuf};

mod common;
use common::{
    TempWorkspace, click_at, clickable_center, full_frame_input, press_key, rendered_text,
};

use legion_desktop::{
    bridge::DesktopAction,
    cut_lines::{DEBUG_LIVE_BANNER, DEBUG_NO_SESSION_BANNER, DEBUG_SIMULATED_BANNER},
    workflow::{DesktopEframeApp, DesktopLaunchConfig, DesktopRuntime},
};
use legion_ui::DebugStatusKindProjection;

/// A minimal cargo project: `discover_cargo_debug_configurations` needs a real
/// manifest, so a bare directory would leave the surface with nothing to launch
/// and make every assertion below vacuous.
fn cargo_workspace() -> TempWorkspace {
    let workspace = TempWorkspace::new("legion_desktop_debug_reachability");
    workspace.write(
        "Cargo.toml",
        "[package]\nname = \"debug-sample\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
    );
    workspace.write(
        "src/main.rs",
        "fn main() {\n    let count = 3;\n    println!(\"{count}\");\n}\n",
    );
    workspace
}

fn open_app(root: &Path, active_file: Option<PathBuf>) -> DesktopEframeApp {
    let mut runtime = DesktopRuntime::open(DesktopLaunchConfig::new(
        root.to_path_buf(),
        active_file.map(|path| path.to_string_lossy().into_owned()),
    ))
    .expect("desktop runtime should open workspace");
    // Deterministic fixture path — see the module header. Note what this does
    // *not* do: it never sets `runtime_enabled`, so the launch these tests
    // click still has to enable the runtime for itself.
    runtime.force_debug_fixture_mode_for_tests();
    DesktopEframeApp::new(runtime)
}

/// Select `Run and Debug` and click `Refresh configs`, returning the frame that
/// shows the result. Both controls are required, not optional: if either is
/// missing the surface is unusable and every later assertion would be vacuous.
fn open_debug_surface(app: &mut DesktopEframeApp) -> egui::FullOutput {
    let primed = app.run_headless_full_frame(full_frame_input(Vec::new()));
    let rail = clickable_center(&primed, "Run and Debug")
        .expect("the activity rail must offer a `Run and Debug` control");
    let on_debug = click_at(app, rail);
    let refresh = clickable_center(&on_debug, "Refresh configs")
        .expect("the debug surface must offer a `Refresh configs` control");
    click_at(app, refresh)
}

fn debug_status(app: &DesktopEframeApp) -> String {
    let debug = app.runtime_snapshot().debug_projection;
    format!(
        "status={:?} session={:?} live={} message={}",
        debug.status.kind,
        debug.active_session_id.map(|session| session.0),
        debug.live_adapter,
        debug.status.message
    )
}

fn has_row(output: &egui::FullOutput, needle: &str) -> bool {
    rendered_text(output).iter().any(|row| row.contains(needle))
}

/// Row 9: refreshing configs from the UI finds something to launch.
#[test]
fn refreshing_configs_from_the_ui_offers_a_launch() {
    let workspace = cargo_workspace();
    let mut app = open_app(workspace.path(), None);

    let before = app.runtime_snapshot().debug_projection.configurations.len();
    let refreshed = open_debug_surface(&mut app);
    let after = app.runtime_snapshot().debug_projection.configurations.len();

    assert!(
        after > before,
        "`Refresh configs` discovered nothing in a cargo project: {}",
        debug_status(&app)
    );
    assert!(
        clickable_center(&refreshed, "Launch").is_some(),
        "configs were discovered but no `Launch` control appeared, so the \
         surface has something to run and no way to run it"
    );
}

/// Row 9, the defect: clicking `Launch` must start a session, not deny one.
///
/// The session id is checked before the status kind on purpose. `status`
/// alone would let a denial that happens to be worded gently pass; an
/// `active_session_id` can only come from a launch that actually happened.
#[test]
fn clicking_launch_starts_a_session_instead_of_denying_it() {
    let workspace = cargo_workspace();
    let mut app = open_app(workspace.path(), None);
    let refreshed = open_debug_surface(&mut app);

    let launch = clickable_center(&refreshed, "Launch")
        .expect("a refreshed cargo workspace must offer a `Launch` control");
    let launched = click_at(&mut app, launch);

    let debug = app.runtime_snapshot().debug_projection;
    assert!(
        debug.active_session_id.is_some(),
        "clicking `Launch` started no session: {}. This is the shape the \
         terminal `Run cargo test` button had — the control responds, the \
         status line changes, and nothing runs.",
        debug_status(&app)
    );
    assert_ne!(
        debug.status.kind,
        DebugStatusKindProjection::Denied,
        "`Launch` was denied: {}",
        debug_status(&app)
    );
    assert!(
        !debug.status.message.contains("runtime is disabled"),
        "`Launch` reported the debug runtime disabled in the shipped app; the \
         runtime must enable itself on an explicit, trust-approved launch: {}",
        debug_status(&app)
    );

    // Row 11/12 controls only exist once a session does, so their appearance
    // is a second, independent witness that the launch landed.
    for control in ["Continue", "Step Over", "Stop"] {
        assert!(
            clickable_center(&launched, control).is_some(),
            "`{control}` is missing after a launch: {}",
            debug_status(&app)
        );
    }
}

/// Row 9: `F5` with configs and no session launches, from the rendered frame.
#[test]
fn f5_launches_a_session_from_the_rendered_frame() {
    let workspace = cargo_workspace();
    let main = workspace.path().join("src/main.rs");
    let mut app = open_app(workspace.path(), Some(main));
    let _ = open_debug_surface(&mut app);
    assert!(
        app.runtime_snapshot()
            .debug_projection
            .active_session_id
            .is_none(),
        "no session may exist before F5, or this proves nothing"
    );

    let _ = press_key(&mut app, egui::Key::F5, egui::Modifiers::default());

    assert!(
        app.runtime_snapshot()
            .debug_projection
            .active_session_id
            .is_some(),
        "F5 with configs and no session did not launch: {}",
        debug_status(&app)
    );
}

/// Row 10: with no session there is no mode to report, and the banner says so.
#[test]
fn the_banner_claims_no_mode_before_a_session_exists() {
    let workspace = cargo_workspace();
    let mut app = open_app(workspace.path(), None);
    let refreshed = open_debug_surface(&mut app);

    assert!(
        app.runtime_snapshot()
            .debug_projection
            .active_session_id
            .is_none(),
        "this test describes the no-session state: {}",
        debug_status(&app)
    );
    assert!(
        has_row(&refreshed, DEBUG_NO_SESSION_BANNER),
        "idle debug surface did not show the no-session banner: {:?}",
        rendered_text(&refreshed)
    );
    assert!(
        !has_row(&refreshed, DEBUG_SIMULATED_BANNER),
        "with no session running, the panel claimed the debugger is simulated \
         *in this build* — a statement about the build that a build with \
         LEGION_DAP_ADAPTER set makes false"
    );
    assert!(
        !has_row(&refreshed, DEBUG_LIVE_BANNER),
        "no session is connected, so nothing may claim a live adapter"
    );
}

/// Row 10, the honesty cut line: a fixture session must never read as live.
#[test]
fn a_fixture_session_never_claims_a_live_adapter() {
    let workspace = cargo_workspace();
    let mut app = open_app(workspace.path(), None);
    let refreshed = open_debug_surface(&mut app);
    let launch = clickable_center(&refreshed, "Launch").expect("`Launch` control");
    let launched = click_at(&mut app, launch);

    // Prove this is the fixture path before asserting what it may say. A test
    // that skipped this would also pass if the launch silently went live.
    let debug = app.runtime_snapshot().debug_projection;
    assert!(
        debug.active_session_id.is_some(),
        "no session launched: {}",
        debug_status(&app)
    );
    assert!(
        !debug.live_adapter,
        "this test describes the fixture path, but the session is live: {}",
        debug_status(&app)
    );

    assert!(
        has_row(&launched, DEBUG_SIMULATED_BANNER),
        "a simulated session did not say so: {:?}",
        rendered_text(&launched)
    );
    assert!(
        !has_row(&launched, DEBUG_LIVE_BANNER),
        "a fixture session presented itself as a live adapter process, which is \
         the one thing the dual-mode banner exists to prevent"
    );
}

/// Row 11/12: `Continue`, `Step Over` and `Stop` do something when clicked.
///
/// "Does something" is measured as the status *message* changing, then the
/// session ending. A step that leaves the message identical is indistinguishable
/// from a dead button to the person reading the panel.
#[test]
fn continue_step_over_and_stop_drive_the_session_from_the_toolbar() {
    let workspace = cargo_workspace();
    let mut app = open_app(workspace.path(), None);
    let refreshed = open_debug_surface(&mut app);
    let launch = clickable_center(&refreshed, "Launch").expect("`Launch` control");
    let launched = click_at(&mut app, launch);
    let after_launch = app.runtime_snapshot().debug_projection.status.message;
    assert!(
        app.runtime_snapshot()
            .debug_projection
            .active_session_id
            .is_some(),
        "no session to drive: {}",
        debug_status(&app)
    );

    let cont = clickable_center(&launched, "Continue").expect("`Continue` control");
    let continued = click_at(&mut app, cont);
    let after_continue = app.runtime_snapshot().debug_projection.status.message;
    assert_ne!(
        after_launch, after_continue,
        "`Continue` left the status untouched at `{after_continue}`"
    );

    let step = clickable_center(&continued, "Step Over").expect("`Step Over` control");
    let stepped = click_at(&mut app, step);
    let debug = app.runtime_snapshot().debug_projection;
    assert_eq!(
        debug.status.kind,
        DebugStatusKindProjection::Paused,
        "a step must leave the program paused: {}",
        debug_status(&app)
    );

    let stop = clickable_center(&stepped, "Stop").expect("`Stop` control");
    let stopped = click_at(&mut app, stop);
    let debug = app.runtime_snapshot().debug_projection;
    assert!(
        debug.active_session_id.is_none(),
        "`Stop` left the session running: {}",
        debug_status(&app)
    );
    assert_eq!(
        debug.status.kind,
        DebugStatusKindProjection::Exited,
        "`Stop` did not report an exit: {}",
        debug_status(&app)
    );
    assert!(
        clickable_center(&stopped, "Launch").is_some(),
        "after stopping, the toolbar must offer `Launch` again or the surface \
         is a one-shot"
    );
}

/// Row 12: `Shift+F5` stops; `F5` alone would continue.
///
/// Uses `press_key`, which sets the frame's modifiers as well as the event's.
/// With the modifiers only on the event, egui dispatched this as a plain `F5`
/// — and because `F5` on an active session continues, the session still moved
/// and a weaker assertion would have called that a pass.
#[test]
fn shift_f5_stops_the_session_from_the_rendered_frame() {
    let workspace = cargo_workspace();
    let main = workspace.path().join("src/main.rs");
    let mut app = open_app(workspace.path(), Some(main));
    let _ = open_debug_surface(&mut app);
    let _ = press_key(&mut app, egui::Key::F5, egui::Modifiers::default());
    assert!(
        app.runtime_snapshot()
            .debug_projection
            .active_session_id
            .is_some(),
        "F5 did not launch, so there is nothing for Shift+F5 to stop: {}",
        debug_status(&app)
    );

    let stopped = press_key(
        &mut app,
        egui::Key::F5,
        egui::Modifiers {
            shift: true,
            ..egui::Modifiers::default()
        },
    );

    let debug = app.runtime_snapshot().debug_projection;
    assert!(
        debug.active_session_id.is_none(),
        "Shift+F5 did not stop the session: {}",
        debug_status(&app)
    );
    assert!(
        clickable_center(&stopped, "Launch").is_some(),
        "after Shift+F5 the toolbar must return to `Launch`"
    );
}

/// Row 12: `F9` adds a breakpoint and does not claim the debugger went idle.
///
/// `toggle_breakpoint` used to stamp `Idle` over whatever the session status
/// was, so pressing `F9` at a breakpoint rendered
/// `status=Idle session=Some(…) state=Paused` — and on the live path told the
/// user the debugger had stopped while the adapter was still running the
/// program.
#[test]
fn f9_adds_a_breakpoint_without_reporting_the_debugger_idle() {
    let workspace = cargo_workspace();
    let main = workspace.path().join("src/main.rs");
    let mut app = open_app(workspace.path(), Some(main));
    let _ = open_debug_surface(&mut app);
    let _ = press_key(&mut app, egui::Key::F5, egui::Modifiers::default());
    let launched = app.runtime_snapshot().debug_projection;
    assert_eq!(
        launched.status.kind,
        DebugStatusKindProjection::Paused,
        "the session must be paused before F9, or the status check below is \
         vacuous: {}",
        debug_status(&app)
    );
    let before = launched.breakpoints.len();

    let _ = press_key(&mut app, egui::Key::F9, egui::Modifiers::default());

    let debug = app.runtime_snapshot().debug_projection;
    assert_eq!(
        debug.breakpoints.len(),
        before + 1,
        "F9 added no breakpoint: {}",
        debug_status(&app)
    );
    assert_eq!(
        debug.status.kind,
        DebugStatusKindProjection::Paused,
        "toggling a breakpoint reported the debugger `{:?}` while a session was \
         still active: {}",
        debug.status.kind,
        debug_status(&app)
    );
}

/// Row 9's third route: `:debug-configs` must not claim the debugger went idle.
///
/// Not a click test, because the toolbar deliberately hides `Refresh configs`
/// while a session is active — the shell command is the only way to reach this
/// state, and the checklist's own command table lists it as a supported route.
/// It shares the defect the `F9` test above pins, so it shares the fix.
#[test]
fn refreshing_configs_during_a_session_does_not_report_the_debugger_idle() {
    let workspace = cargo_workspace();
    let mut app = open_app(workspace.path(), None);
    let refreshed = open_debug_surface(&mut app);
    let launch = clickable_center(&refreshed, "Launch").expect("`Launch` control");
    let _ = click_at(&mut app, launch);
    assert_eq!(
        app.runtime_snapshot().debug_projection.status.kind,
        DebugStatusKindProjection::Paused,
        "the session must be paused first, or the status check below is \
         vacuous: {}",
        debug_status(&app)
    );

    app.handle_action(DesktopAction::RefreshDebugConfigurations)
        .expect("`:debug-configs` should refresh while a session is active");

    let debug = app.runtime_snapshot().debug_projection;
    assert!(
        debug.active_session_id.is_some(),
        "refreshing configs dropped the session: {}",
        debug_status(&app)
    );
    assert_eq!(
        debug.status.kind,
        DebugStatusKindProjection::Paused,
        "refreshing the configuration list announced `{:?}` for a session that \
         is still paused: {}",
        debug.status.kind,
        debug_status(&app)
    );
}

// --- Live adapter path -----------------------------------------------------

fn ensure_fake_adapter_built() {
    if legion_debug::fake_dap_adapter_path().is_some() {
        return;
    }
    let status = std::process::Command::new(env!("CARGO"))
        .args([
            "build",
            "-p",
            "legion-debug",
            "--bin",
            "fake_dap_adapter",
            "--quiet",
        ])
        .status()
        .expect("spawn cargo build for fake_dap_adapter");
    assert!(status.success(), "building fake_dap_adapter failed");
}

/// Row 10/11 on the live path, driven by clicks.
///
/// This is the one test here that uses a debug seam:
/// `enable_debug_live_fake_for_tests` adds the in-tree CI adapter to the
/// allowlist, which no product path does and none should. Everything after that
/// is the rendered UI.
#[test]
fn a_live_session_says_live_and_stops_saying_it_when_it_ends() {
    ensure_fake_adapter_built();

    let workspace = cargo_workspace();
    let main = workspace.path().join("src/main.rs");
    let mut runtime = DesktopRuntime::open(DesktopLaunchConfig::new(
        workspace.path().to_path_buf(),
        Some(main.to_string_lossy().into_owned()),
    ))
    .expect("desktop runtime should open workspace");
    runtime.enable_debug_live_fake_for_tests();
    let mut app = DesktopEframeApp::new(runtime);

    let refreshed = open_debug_surface(&mut app);
    let launch = clickable_center(&refreshed, "Launch").expect("`Launch` control");
    let launched = click_at(&mut app, launch);

    let debug = app.runtime_snapshot().debug_projection;
    assert!(
        debug.live_adapter,
        "the fake adapter did not produce a live session, so the live banner \
         below would be tested against the fixture path: {}",
        debug_status(&app)
    );
    assert!(
        has_row(&launched, DEBUG_LIVE_BANNER),
        "a live adapter session did not say so: {:?}",
        rendered_text(&launched)
    );
    assert!(
        !has_row(&launched, DEBUG_SIMULATED_BANNER),
        "a live session presented itself as simulated"
    );

    // Row 11: continue reaches a stop, whether it re-pauses in the same frame
    // or after the auto-poll drains it.
    let cont = clickable_center(&launched, "Continue").expect("`Continue` control");
    let continued = click_at(&mut app, cont);
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    while app.runtime_snapshot().debug_projection.status.kind != DebugStatusKindProjection::Paused
        && std::time::Instant::now() < deadline
    {
        let _ = app.run_headless_full_frame(full_frame_input(Vec::new()));
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    assert_eq!(
        app.runtime_snapshot().debug_projection.status.kind,
        DebugStatusKindProjection::Paused,
        "live continue never reached a paused stop: {}",
        debug_status(&app)
    );

    let settled = app.run_headless_full_frame(full_frame_input(Vec::new()));
    let stop = clickable_center(&settled, "Stop")
        .or_else(|| clickable_center(&continued, "Stop"))
        .expect("`Stop` control");
    let stopped = click_at(&mut app, stop);

    assert!(
        app.runtime_snapshot()
            .debug_projection
            .active_session_id
            .is_none(),
        "`Stop` left the live session attached: {}",
        debug_status(&app)
    );
    assert!(
        !has_row(&stopped, DEBUG_LIVE_BANNER),
        "the panel still claimed a live adapter connection after disconnecting"
    );
    assert!(
        !has_row(&stopped, DEBUG_SIMULATED_BANNER),
        "after disconnecting from a live adapter the panel announced that the \
         debugger is simulated *in this build* — contradicting the banner it \
         showed one click earlier"
    );
}
