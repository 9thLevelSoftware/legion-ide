use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};

use legion_desktop::{
    bridge::DesktopAction,
    view::DesktopProjectionViewModel,
    workflow::{DesktopLaunchConfig, DesktopRuntime, DesktopWorkflowOutcome},
};
use legion_ui::{DebugStatusKindProjection, DebugStepKindProjection};

static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

fn create_root() -> PathBuf {
    let root = std::env::temp_dir().join(format!(
        "legion-desktop-debug-{}-{}",
        std::process::id(),
        TEMP_COUNTER.fetch_add(1, Ordering::SeqCst)
    ));
    fs::create_dir_all(root.join("src")).expect("create src");
    fs::write(
        root.join("Cargo.toml"),
        r#"[package]
name = "desktop-debug"
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

#[test]
fn desktop_debug_workflow_projects_right_and_bottom_debug_rows() {
    let root = create_root();
    let source = root.join("src/main.rs");
    let mut runtime = DesktopRuntime::open(DesktopLaunchConfig::new(
        root.clone(),
        Some(source.to_string_lossy().into_owned()),
    ))
    .expect("desktop runtime should open workspace");
    runtime.enable_debug_fixture_for_tests();

    assert_eq!(
        runtime
            .handle_action(DesktopAction::RefreshDebugConfigurations)
            .expect("refresh debug configs"),
        DesktopWorkflowOutcome::DebugProjectionUpdated
    );
    let config_id = runtime
        .projection_snapshot()
        .debug_projection
        .configurations
        .iter()
        .find(|config| config.configuration_id.0 == "cargo:desktop-debug:bin:desktop-debug")
        .expect("cargo debug config should exist")
        .configuration_id
        .clone();

    assert_eq!(
        runtime
            .handle_action(DesktopAction::ToggleDebugBreakpoint {
                line: 1,
                condition: Some("count > 2".to_string()),
                hit_condition: Some("3".to_string()),
                log_message: Some("count changed".to_string()),
            })
            .expect("toggle breakpoint"),
        DesktopWorkflowOutcome::DebugProjectionUpdated
    );
    assert_eq!(
        runtime
            .handle_action(DesktopAction::LaunchDebugSession {
                configuration_id: config_id,
            })
            .expect("launch debug"),
        DesktopWorkflowOutcome::DebugProjectionUpdated
    );
    let snapshot = runtime.projection_snapshot();
    assert_eq!(
        snapshot.debug_projection.status.kind,
        DebugStatusKindProjection::Paused
    );
    let model = DesktopProjectionViewModel::from_snapshot(&snapshot);
    assert!(
        model
            .debug_rows
            .iter()
            .any(|row| row.contains("debug config"))
    );
    assert!(
        model
            .debug_rows
            .iter()
            .any(|row| row.contains("debug breakpoint"))
    );
    assert!(
        model
            .debug_rows
            .iter()
            .any(|row| row.contains("debug frame"))
    );
    assert!(
        model
            .debug_rows
            .iter()
            .any(|row| row.contains("debug variable"))
    );
    assert!(
        model
            .debug_rows
            .iter()
            .any(|row| row.contains("debug console"))
    );

    let session_id = snapshot
        .debug_projection
        .active_session_id
        .clone()
        .expect("active debug session");
    assert_eq!(
        runtime
            .handle_action(DesktopAction::DebugStep {
                session_id: session_id.clone(),
                kind: DebugStepKindProjection::Over,
            })
            .expect("debug step"),
        DesktopWorkflowOutcome::DebugProjectionUpdated
    );
    assert_eq!(
        runtime
            .handle_action(DesktopAction::DebugEvaluateSelection {
                session_id,
                expression_label: "count".to_string(),
            })
            .expect("debug evaluate"),
        DesktopWorkflowOutcome::DebugProjectionUpdated
    );
    let model = DesktopProjectionViewModel::from_snapshot(&runtime.projection_snapshot());
    assert!(model.debug_rows.iter().any(|row| row.contains("step=over")));
    assert!(model.debug_rows.iter().any(|row| row.contains("evaluate")));

    fs::remove_dir_all(root).ok();
}
