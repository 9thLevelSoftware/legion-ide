use std::{
    fs,
    sync::atomic::{AtomicU64, Ordering},
};

use legion_app::{AppCommandOutcome, AppComposition};
use legion_protocol::{DebugSessionId, DebugSessionState, PrincipalId, WorkspaceTrustState};
use legion_ui::{CommandDispatchIntent, DebugStatusKindProjection, DebugStepKindProjection};

static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

fn create_root() -> std::path::PathBuf {
    create_root_named("debug-sample")
}

fn create_root_named(package_name: &str) -> std::path::PathBuf {
    let root = std::env::temp_dir().join(format!(
        "legion-debug-workflow-{}-{}-{}",
        package_name,
        std::process::id(),
        TEMP_COUNTER.fetch_add(1, Ordering::SeqCst)
    ));
    fs::create_dir_all(root.join("src")).expect("create src");
    fs::write(
        root.join("Cargo.toml"),
        format!(
            r#"[package]
name = "{package_name}"
version = "0.1.0"
edition = "2024"
"#
        ),
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
fn debug_workflow_persists_breakpoints_launches_runtime_and_projects_docks() {
    let root = create_root();
    let source = root.join("src/main.rs");
    let mut app = AppComposition::new();
    app.open_workspace(
        &root,
        WorkspaceTrustState::Trusted,
        PrincipalId("principal-debug".to_string()),
    )
    .expect("open workspace");
    app.open_file(source.to_string_lossy())
        .expect("open source file");
    let buffer_id = app.active_buffer_id().expect("active buffer");
    let original_text = app
        .editor()
        .text(buffer_id)
        .expect("active text")
        .to_string();
    // Fixture path (default): simulated DAP without requiring an adapter binary.
    app.enable_debug_runtime_for_tests();

    let configs = app
        .dispatch_ui_intent(CommandDispatchIntent::RefreshDebugConfigurations)
        .expect("refresh debug configs");
    let mut projection = match configs {
        AppCommandOutcome::DebugProjectionUpdated(projection) => projection,
        other => panic!("expected debug projection, got {other:?}"),
    };
    let configuration_id = projection
        .configurations
        .iter()
        .find(|config| config.configuration_id.0 == "cargo:debug-sample:bin:debug-sample")
        .expect("cargo config should exist")
        .configuration_id
        .clone();

    projection = match app
        .dispatch_ui_intent(CommandDispatchIntent::ToggleDebugBreakpoint {
            buffer_id,
            line: 1,
            condition: Some("count > 2".to_string()),
            hit_condition: Some("3".to_string()),
            log_message: Some("count changed".to_string()),
        })
        .expect("toggle breakpoint")
    {
        AppCommandOutcome::DebugProjectionUpdated(projection) => projection,
        other => panic!("expected debug projection, got {other:?}"),
    };
    assert_eq!(projection.breakpoints.len(), 1);
    assert!(projection.breakpoints[0].session_id.is_none());
    assert_eq!(
        projection.breakpoints[0].condition.as_deref(),
        Some("count > 2")
    );

    projection = match app
        .dispatch_ui_intent(CommandDispatchIntent::LaunchDebugSession { configuration_id })
        .expect("launch debug runtime")
    {
        AppCommandOutcome::DebugProjectionUpdated(projection) => projection,
        other => panic!("expected debug projection, got {other:?}"),
    };
    assert_eq!(projection.status.kind, DebugStatusKindProjection::Paused);
    assert_eq!(projection.session_state, Some(DebugSessionState::Paused));
    assert_eq!(
        projection.breakpoints[0].session_id, None,
        "persisted breakpoint projection must remain session-independent after launch"
    );
    assert!(
        projection
            .variables
            .iter()
            .any(|variable| variable.name == "count")
    );
    assert!(!projection.stack_frames.is_empty());
    assert!(!projection.inline_values.is_empty());
    assert!(
        projection
            .console
            .iter()
            .any(|entry| entry.message_label.contains("launch"))
    );
    let session_id = projection
        .active_session_id
        .clone()
        .expect("active debug session");

    for intent in [
        CommandDispatchIntent::DebugStep {
            session_id: session_id.clone(),
            kind: DebugStepKindProjection::Over,
        },
        CommandDispatchIntent::DebugRunToCursor {
            session_id: session_id.clone(),
            buffer_id,
            position: legion_protocol::TextCoordinate {
                line: 2,
                character: 4,
                byte_offset: None,
                utf16_offset: None,
            },
        },
        CommandDispatchIntent::DebugEvaluateSelection {
            session_id: session_id.clone(),
            expression_label: "count".to_string(),
        },
        CommandDispatchIntent::DebugAddWatch {
            session_id: session_id.clone(),
            expression_label: "count + 1".to_string(),
        },
    ] {
        projection = match app.dispatch_ui_intent(intent).expect("debug intent") {
            AppCommandOutcome::DebugProjectionUpdated(projection) => projection,
            other => panic!("expected debug projection, got {other:?}"),
        };
    }

    assert!(
        projection
            .watches
            .iter()
            .any(|watch| watch.expression_label == "count + 1")
    );
    assert!(
        projection
            .console
            .iter()
            .any(|entry| entry.message_label.contains("evaluate"))
    );
    projection = match app
        .dispatch_ui_intent(CommandDispatchIntent::RefreshDebugConfigurations)
        .expect("refresh after launch")
    {
        AppCommandOutcome::DebugProjectionUpdated(projection) => projection,
        other => panic!("expected debug projection, got {other:?}"),
    };
    assert_eq!(projection.breakpoints.len(), 1);
    assert_eq!(
        projection.breakpoints[0].session_id, None,
        "stored breakpoints must not be rebound to debug sessions"
    );
    assert_eq!(
        app.editor().text(buffer_id).expect("active text"),
        original_text
    );
    assert_eq!(
        fs::read_to_string(&source).expect("disk text"),
        "fn main() {\n    let count = 3;\n    println!(\"{count}\");\n}\n"
    );

    projection = match app
        .dispatch_ui_intent(CommandDispatchIntent::ToggleDebugBreakpoint {
            buffer_id,
            line: 1,
            condition: None,
            hit_condition: None,
            log_message: None,
        })
        .expect("remove breakpoint")
    {
        AppCommandOutcome::DebugProjectionUpdated(projection) => projection,
        other => panic!("expected debug projection, got {other:?}"),
    };
    assert!(projection.breakpoints.is_empty());
    projection = match app
        .dispatch_ui_intent(CommandDispatchIntent::RefreshDebugConfigurations)
        .expect("refresh after breakpoint removal")
    {
        AppCommandOutcome::DebugProjectionUpdated(projection) => projection,
        other => panic!("expected debug projection, got {other:?}"),
    };
    assert!(
        projection.breakpoints.is_empty(),
        "removed breakpoints must not be resurrected from storage"
    );

    fs::remove_dir_all(root).ok();
}

#[test]
fn debug_workflow_rejects_secondary_actions_without_launched_session() {
    let root = create_root();
    let source = root.join("src/main.rs");
    let mut app = AppComposition::new();
    app.open_workspace(
        &root,
        WorkspaceTrustState::Trusted,
        PrincipalId("principal-debug-secondary".to_string()),
    )
    .expect("open workspace");
    app.open_file(source.to_string_lossy())
        .expect("open source file");

    let projection = match app
        .dispatch_ui_intent(CommandDispatchIntent::DebugEvaluateSelection {
            session_id: DebugSessionId("debug:fake".to_string()),
            expression_label: "count".to_string(),
        })
        .expect("evaluate without active session")
    {
        AppCommandOutcome::DebugProjectionUpdated(projection) => projection,
        other => panic!("expected debug projection, got {other:?}"),
    };
    assert_eq!(projection.status.kind, DebugStatusKindProjection::Denied);
    assert!(projection.console.is_empty());
    assert!(projection.watches.is_empty());

    fs::remove_dir_all(root).ok();
}

#[test]
fn debug_workflow_resets_state_on_workspace_switch() {
    let first_root = create_root_named("debug-one");
    let second_root = create_root_named("debug-two");
    let first_source = first_root.join("src/main.rs");
    let second_source = second_root.join("src/main.rs");
    let mut app = AppComposition::new();
    app.open_workspace(
        &first_root,
        WorkspaceTrustState::Trusted,
        PrincipalId("principal-debug-switch".to_string()),
    )
    .expect("open first workspace");
    app.open_file(first_source.to_string_lossy())
        .expect("open first source");
    let first_buffer = app.active_buffer_id().expect("first active buffer");
    app.enable_debug_runtime_for_tests();

    let projection = match app
        .dispatch_ui_intent(CommandDispatchIntent::RefreshDebugConfigurations)
        .expect("refresh first debug configs")
    {
        AppCommandOutcome::DebugProjectionUpdated(projection) => projection,
        other => panic!("expected debug projection, got {other:?}"),
    };
    let first_config_id = projection.configurations[0].configuration_id.clone();
    app.dispatch_ui_intent(CommandDispatchIntent::ToggleDebugBreakpoint {
        buffer_id: first_buffer,
        line: 1,
        condition: None,
        hit_condition: None,
        log_message: None,
    })
    .expect("first breakpoint");
    app.dispatch_ui_intent(CommandDispatchIntent::LaunchDebugSession {
        configuration_id: first_config_id,
    })
    .expect("first launch");

    app.open_workspace(
        &second_root,
        WorkspaceTrustState::Trusted,
        PrincipalId("principal-debug-switch".to_string()),
    )
    .expect("open second workspace");
    app.open_file(second_source.to_string_lossy())
        .expect("open second source");
    let projection = app.debug_projection();
    assert!(projection.active_session_id.is_none());
    assert!(projection.configurations.is_empty());
    assert!(projection.breakpoints.is_empty());
    assert!(projection.stack_frames.is_empty());
    assert!(projection.variables.is_empty());
    assert!(projection.console.is_empty());

    fs::remove_dir_all(first_root).ok();
    fs::remove_dir_all(second_root).ok();
}

#[test]
fn debug_workflow_denies_launch_on_untrusted_workspace() {
    let root = create_root();
    let source = root.join("src/main.rs");
    let mut app = AppComposition::new();
    app.open_workspace(
        &root,
        WorkspaceTrustState::Untrusted,
        PrincipalId("principal-debug-untrusted".to_string()),
    )
    .expect("open untrusted workspace");
    app.open_file(source.to_string_lossy())
        .expect("open source");
    app.enable_debug_runtime_for_tests();

    let configs = match app
        .dispatch_ui_intent(CommandDispatchIntent::RefreshDebugConfigurations)
        .expect("refresh configs")
    {
        AppCommandOutcome::DebugProjectionUpdated(projection) => projection,
        other => panic!("expected debug projection, got {other:?}"),
    };
    let configuration_id = configs
        .configurations
        .first()
        .expect("config")
        .configuration_id
        .clone();

    let projection = match app
        .dispatch_ui_intent(CommandDispatchIntent::LaunchDebugSession { configuration_id })
        .expect("launch should return projection")
    {
        AppCommandOutcome::DebugProjectionUpdated(projection) => projection,
        other => panic!("expected debug projection, got {other:?}"),
    };
    assert_eq!(projection.status.kind, DebugStatusKindProjection::Denied);
    assert!(
        projection.status.message.contains("trusted")
            || projection.status.message.contains("debug.adapter.launch")
    );
    assert!(!projection.live_adapter);
    assert!(projection.active_session_id.is_none());

    fs::remove_dir_all(root).ok();
}

#[test]
fn debug_workflow_live_mode_fails_closed_without_adapter() {
    let root = create_root();
    let source = root.join("src/main.rs");
    let mut app = AppComposition::new();
    app.open_workspace(
        &root,
        WorkspaceTrustState::Trusted,
        PrincipalId("principal-debug-live-mode".to_string()),
    )
    .expect("open workspace");
    app.open_file(source.to_string_lossy())
        .expect("open source");
    app.enable_debug_runtime_for_tests();
    // Instance-scoped override (not process env) so parallel cargo test threads
    // cannot poison sibling debug_workflow cases.
    app.set_debug_dap_mode_for_tests(legion_debug::DapMode::Live);

    let configs = match app
        .dispatch_ui_intent(CommandDispatchIntent::RefreshDebugConfigurations)
        .expect("refresh")
    {
        AppCommandOutcome::DebugProjectionUpdated(projection) => projection,
        other => panic!("expected debug projection, got {other:?}"),
    };
    let configuration_id = configs
        .configurations
        .first()
        .expect("config")
        .configuration_id
        .clone();

    let projection = match app
        .dispatch_ui_intent(CommandDispatchIntent::LaunchDebugSession { configuration_id })
        .expect("launch should return projection")
    {
        AppCommandOutcome::DebugProjectionUpdated(projection) => projection,
        other => panic!("expected debug projection, got {other:?}"),
    };

    assert_eq!(
        projection.status.kind,
        DebugStatusKindProjection::Failed,
        "live mode must not fall back to fixture: {}",
        projection.status.message
    );
    assert!(!projection.live_adapter);
    assert!(
        projection.status.message.contains("live DAP required")
            || projection.status.message.contains("LEGION_DAP_MODE=live"),
        "message should explain fail-closed live mode: {}",
        projection.status.message
    );
    assert!(
        projection.stack_frames.is_empty(),
        "must not project fixture stack frames after live failure"
    );

    fs::remove_dir_all(root).ok();
}

#[test]
fn debug_workflow_live_fake_adapter_sets_live_projection_flag() {
    let root = create_root();
    let source = root.join("src/main.rs");
    let mut app = AppComposition::new();
    app.open_workspace(
        &root,
        WorkspaceTrustState::Trusted,
        PrincipalId("principal-debug-live".to_string()),
    )
    .expect("open workspace");
    app.open_file(source.to_string_lossy())
        .expect("open source");
    let buffer_id = app.active_buffer_id().expect("buffer");
    app.enable_debug_live_fake_for_tests();

    let configs = match app
        .dispatch_ui_intent(CommandDispatchIntent::RefreshDebugConfigurations)
        .expect("refresh")
    {
        AppCommandOutcome::DebugProjectionUpdated(projection) => projection,
        other => panic!("expected debug projection, got {other:?}"),
    };
    let configuration_id = configs
        .configurations
        .first()
        .expect("config")
        .configuration_id
        .clone();

    app.dispatch_ui_intent(CommandDispatchIntent::ToggleDebugBreakpoint {
        buffer_id,
        line: 1,
        condition: None,
        hit_condition: None,
        log_message: None,
    })
    .expect("breakpoint");

    let projection = match app
        .dispatch_ui_intent(CommandDispatchIntent::LaunchDebugSession { configuration_id })
        .expect("live launch")
    {
        AppCommandOutcome::DebugProjectionUpdated(projection) => projection,
        other => panic!("expected debug projection, got {other:?}"),
    };

    assert!(
        projection.live_adapter,
        "live fake path should set live_adapter=true: {}",
        projection.status.message
    );
    assert_eq!(projection.status.kind, DebugStatusKindProjection::Paused);
    assert!(projection.status.message.contains("Live DAP"));
    assert!(
        projection
            .stack_frames
            .iter()
            .any(|frame| frame.name == "main"),
        "live stop should project stack: {:?}",
        projection.stack_frames
    );
    assert!(
        projection
            .console
            .iter()
            .any(|entry| entry.message_label.contains("LIVE DAP")),
        "console should note live path"
    );
    assert!(
        projection.status.message.contains("persistent=true"),
        "B5 live session should stay connected: {}",
        projection.status.message
    );

    let session_id = projection
        .active_session_id
        .clone()
        .expect("live session id");
    let stepped = match app
        .dispatch_ui_intent(CommandDispatchIntent::DebugStep {
            session_id: session_id.clone(),
            kind: DebugStepKindProjection::Over,
        })
        .expect("live step over")
    {
        AppCommandOutcome::DebugProjectionUpdated(projection) => projection,
        other => panic!("expected debug projection after step, got {other:?}"),
    };
    assert!(
        stepped.live_adapter,
        "step should keep live_adapter: {}",
        stepped.status.message
    );
    assert_eq!(stepped.status.kind, DebugStatusKindProjection::Paused);
    assert!(
        stepped.status.message.contains("next") || stepped.status.message.contains("Live DAP"),
        "step message should note live step: {}",
        stepped.status.message
    );
    assert!(
        stepped
            .console
            .iter()
            .any(|entry| entry.message_label.contains("LIVE DAP step")),
        "console should record live step"
    );

    fs::remove_dir_all(root).ok();
}
