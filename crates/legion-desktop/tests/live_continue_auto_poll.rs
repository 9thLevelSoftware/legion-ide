//! B10: headless desktop dogfood for live continue → auto-poll → paused.
//!
//! Exercises the production frame path (`run_headless_full_frame` →
//! `render_app_frame` → `PollDebugSession`) against the in-tree fake adapter.
//! This is the automated substitute for interactive GUI continue dogfood;
//! a human windowed session remains residual.

use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
    time::{Duration, Instant},
};

use legion_desktop::{
    bridge::DesktopAction,
    debug_auto_poll::debug_needs_auto_poll,
    view::DesktopProjectionViewModel,
    workflow::{DesktopEframeApp, DesktopLaunchConfig, DesktopRuntime, DesktopWorkflowOutcome},
};
use legion_ui::{DebugStatusKindProjection, DebugStepKindProjection};

static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

fn create_root() -> PathBuf {
    let root = std::env::temp_dir().join(format!(
        "legion-desktop-live-continue-{}-{}",
        std::process::id(),
        TEMP_COUNTER.fetch_add(1, Ordering::SeqCst)
    ));
    fs::create_dir_all(root.join("src")).expect("create src");
    fs::write(
        root.join("Cargo.toml"),
        r#"[package]
name = "desktop-live-continue"
version = "0.1.0"
edition = "2024"
"#,
    )
    .expect("write Cargo.toml");
    fs::write(
        root.join("src/main.rs"),
        "fn main() {\n    let count = 3;\n    println!(\"{count}\");\n}\n",
    )
    .expect("write main");
    root
}

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
        .expect("spawn cargo build fake_dap_adapter");
    assert!(
        status.success(),
        "failed to build fake_dap_adapter for B10 live continue dogfood"
    );
    assert!(
        legion_debug::fake_dap_adapter_path().is_some(),
        "fake_dap_adapter binary still missing after build"
    );
}

#[test]
fn desktop_live_continue_headless_auto_poll_reaches_paused() {
    ensure_fake_adapter_built();

    let root = create_root();
    let source = root.join("src/main.rs");
    let mut runtime = DesktopRuntime::open(DesktopLaunchConfig::new(
        root.clone(),
        Some(source.to_string_lossy().into_owned()),
    ))
    .expect("desktop runtime should open workspace");
    runtime.enable_debug_live_fake_for_tests();

    runtime
        .handle_action(DesktopAction::RefreshDebugConfigurations)
        .expect("refresh debug configs");
    let config_id = runtime
        .projection_snapshot()
        .debug_projection
        .configurations
        .first()
        .expect("debug config")
        .configuration_id
        .clone();

    runtime
        .handle_action(DesktopAction::ToggleDebugBreakpoint {
            line: 1,
            condition: None,
            hit_condition: None,
            log_message: None,
        })
        .expect("toggle breakpoint");

    assert_eq!(
        runtime
            .handle_action(DesktopAction::LaunchDebugSession {
                configuration_id: config_id,
            })
            .expect("live launch"),
        DesktopWorkflowOutcome::DebugProjectionUpdated
    );

    let launched = runtime.projection_snapshot().debug_projection;
    assert!(
        launched.live_adapter,
        "live fake should set live_adapter: {}",
        launched.status.message
    );
    assert_eq!(
        launched.status.kind,
        DebugStatusKindProjection::Paused,
        "launch should stop: {}",
        launched.status.message
    );
    let session_id = launched
        .active_session_id
        .clone()
        .expect("active live session");

    assert_eq!(
        runtime
            .handle_action(DesktopAction::DebugStep {
                session_id: session_id.clone(),
                kind: DebugStepKindProjection::Continue,
            })
            .expect("live continue"),
        DesktopWorkflowOutcome::DebugProjectionUpdated
    );

    let continued = runtime.projection_snapshot().debug_projection;
    assert!(
        continued.live_adapter,
        "continue should stay live: {}",
        continued.status.message
    );
    assert_eq!(
        continued.status.kind,
        DebugStatusKindProjection::Running,
        "B7 continue is non-blocking: {}",
        continued.status.message
    );
    assert!(
        debug_needs_auto_poll(&continued),
        "B8 predicate must hold while live Running"
    );
    let model = DesktopProjectionViewModel::from_snapshot(&runtime.projection_snapshot());
    assert!(
        model
            .debug_rows
            .iter()
            .any(|row| row.contains("auto-poll active")),
        "status rows should note auto-poll: {:?}",
        model.debug_rows
    );

    // Frame loop (same path as production eframe) must drain stop without
    // manual :debug-poll / PollDebugSession from the test.
    let mut app = DesktopEframeApp::new(runtime);
    let deadline = Instant::now() + Duration::from_secs(5);
    let final_debug = loop {
        let _ = app.run_headless_full_frame(egui::RawInput::default());
        let debug = app.runtime_snapshot().debug_projection;
        if debug.status.kind == DebugStatusKindProjection::Paused {
            break debug;
        }
        if Instant::now() >= deadline {
            panic!(
                "headless auto-poll did not observe stop within 5s: kind={:?} msg={}",
                debug.status.kind, debug.status.message
            );
        }
        std::thread::sleep(Duration::from_millis(20));
    };

    assert!(
        final_debug.live_adapter,
        "auto-poll stop should remain live: {}",
        final_debug.status.message
    );
    assert!(
        final_debug.status.message.contains("stopped")
            || final_debug.status.message.contains("breakpoint")
            || final_debug.status.message.contains("continue"),
        "stop message should note continue/stop: {}",
        final_debug.status.message
    );
    assert!(
        final_debug
            .stack_frames
            .iter()
            .any(|frame| frame.name == "main"),
        "auto-poll should re-project stack: {:?}",
        final_debug.stack_frames
    );
    assert!(
        !debug_needs_auto_poll(&final_debug),
        "auto-poll predicate must clear after Paused"
    );

    assert_eq!(
        app.handle_action(DesktopAction::StopDebugSession)
            .expect("stop"),
        DesktopWorkflowOutcome::DebugProjectionUpdated
    );
    let stopped = app.runtime_snapshot().debug_projection;
    assert_eq!(
        stopped.status.kind,
        DebugStatusKindProjection::Exited,
        "stop should exit: {}",
        stopped.status.message
    );
    assert!(!stopped.live_adapter);

    fs::remove_dir_all(root).ok();
}
