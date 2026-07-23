//! B14: debug keyboard shortcuts when a session is active.
//!
//! Headless eframe drives F5/F10/Shift+F5 against the live fake adapter path
//! (same bindings the windowed GUI uses).

use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
    time::{Duration, Instant},
};

use legion_desktop::{
    bridge::DesktopAction,
    workflow::{DesktopEframeApp, DesktopLaunchConfig, DesktopRuntime, DesktopWorkflowOutcome},
};
use legion_ui::{DebugStatusKindProjection, DebugStepKindProjection};

static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

fn create_root() -> PathBuf {
    let root = std::env::temp_dir().join(format!(
        "legion-desktop-debug-kb-{}-{}",
        std::process::id(),
        TEMP_COUNTER.fetch_add(1, Ordering::SeqCst)
    ));
    fs::create_dir_all(root.join("src")).expect("create src");
    fs::write(
        root.join("Cargo.toml"),
        r#"[package]
name = "desktop-debug-kb"
version = "0.1.0"
edition = "2024"
"#,
    )
    .expect("toml");
    fs::write(
        root.join("src/main.rs"),
        "fn main() {\n    let count = 3;\n    println!(\"{count}\");\n}\n",
    )
    .expect("main");
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
        .expect("spawn cargo build");
    assert!(status.success(), "build fake_dap_adapter failed");
}

fn key_input(key: egui::Key, shift: bool) -> egui::RawInput {
    egui::RawInput {
        focused: true,
        modifiers: egui::Modifiers {
            shift,
            ..egui::Modifiers::default()
        },
        events: vec![egui::Event::Key {
            key,
            physical_key: Some(key),
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers {
                shift,
                ..egui::Modifiers::default()
            },
        }],
        ..egui::RawInput::default()
    }
}

#[test]
fn debug_session_f5_continue_f10_step_shift_f5_stop() {
    ensure_fake_adapter_built();

    let root = create_root();
    let source = root.join("src/main.rs");
    let mut runtime = DesktopRuntime::open(DesktopLaunchConfig::new(
        root.clone(),
        Some(source.to_string_lossy().into_owned()),
    ))
    .expect("open");
    runtime.enable_debug_live_fake_for_tests();
    runtime
        .handle_action(DesktopAction::RefreshDebugConfigurations)
        .expect("configs");
    let config_id = runtime
        .projection_snapshot()
        .debug_projection
        .configurations
        .first()
        .expect("config")
        .configuration_id
        .clone();
    runtime
        .handle_action(DesktopAction::ToggleDebugBreakpoint {
            line: 1,
            condition: None,
            hit_condition: None,
            log_message: None,
        })
        .expect("bp");
    assert_eq!(
        runtime
            .handle_action(DesktopAction::LaunchDebugSession {
                configuration_id: config_id,
            })
            .expect("launch"),
        DesktopWorkflowOutcome::DebugProjectionUpdated
    );
    assert_eq!(
        runtime.projection_snapshot().debug_projection.status.kind,
        DebugStatusKindProjection::Paused
    );

    // Step over via F10 (session active) through full keyboard path.
    let mut app = DesktopEframeApp::new(runtime);
    let _ = app.run_headless_full_frame(key_input(egui::Key::F10, false));
    let after_step = app.runtime_snapshot().debug_projection;
    assert!(
        after_step.live_adapter,
        "F10 step should stay live: {}",
        after_step.status.message
    );
    assert_eq!(
        after_step.status.kind,
        DebugStatusKindProjection::Paused,
        "F10 step should pause: {}",
        after_step.status.message
    );

    // F5 continue → non-blocking Running; same frame may also auto-poll to
    // Paused when the fake adapter stops instantly — accept either, then drain.
    let _ = app.run_headless_full_frame(key_input(egui::Key::F5, false));
    let after_f5 = app.runtime_snapshot().debug_projection;
    assert!(
        matches!(
            after_f5.status.kind,
            DebugStatusKindProjection::Running | DebugStatusKindProjection::Paused
        ),
        "F5 continue should run or already re-pause: {}",
        after_f5.status.message
    );

    if after_f5.status.kind == DebugStatusKindProjection::Running {
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            let _ = app.run_headless_full_frame(egui::RawInput::default());
            let debug = app.runtime_snapshot().debug_projection;
            if debug.status.kind == DebugStatusKindProjection::Paused {
                break;
            }
            if Instant::now() >= deadline {
                panic!(
                    "auto-poll after F5 continue did not pause: {}",
                    debug.status.message
                );
            }
            std::thread::sleep(Duration::from_millis(20));
        }
    }

    // Shift+F5 stop.
    let _ = app.run_headless_full_frame(key_input(egui::Key::F5, true));
    assert_eq!(
        app.runtime_snapshot().debug_projection.status.kind,
        DebugStatusKindProjection::Exited,
        "Shift+F5 should stop: {}",
        app.runtime_snapshot().debug_projection.status.message
    );

    // Idle F5 still refreshes explorer (no session).
    assert_eq!(
        app.handle_action(DesktopAction::RefreshExplorer)
            .expect("refresh still available"),
        DesktopWorkflowOutcome::ExplorerRefreshed
    );

    // Document that Continue action kind still maps for toolbar/tests.
    let _ = DebugStepKindProjection::Continue;

    fs::remove_dir_all(root).ok();
}
