use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};

use legion_desktop::{
    bridge::DesktopAction,
    workflow::{DesktopLaunchConfig, DesktopRuntime, DesktopWorkflowOutcome},
};
use legion_protocol::TextCoordinate;
use legion_ui::DockMode;

static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

fn create_root() -> PathBuf {
    let root = std::env::temp_dir().join(format!(
        "legion-desktop-assist-inline-{}-{}",
        std::process::id(),
        TEMP_COUNTER.fetch_add(1, Ordering::SeqCst)
    ));
    fs::create_dir_all(&root).expect("create temp workspace");
    fs::write(root.join("main.rs"), "fn main() {}\n").expect("write rust file");
    root
}

fn cursor_at_line_end() -> TextCoordinate {
    TextCoordinate {
        line: 0,
        character: 12,
        byte_offset: Some(12),
        utf16_offset: Some(12),
    }
}

#[test]
fn desktop_runtime_switches_to_assist_before_inline_prediction_activity() {
    let root = create_root();
    let source = root.join("main.rs");
    let mut runtime = DesktopRuntime::open(DesktopLaunchConfig::new(
        root.clone(),
        Some(source.to_string_lossy().into_owned()),
    ))
    .expect("desktop runtime should open workspace");

    assert_eq!(runtime.projection_snapshot().product_mode, DockMode::Manual);
    assert_eq!(
        runtime
            .handle_action(DesktopAction::SetProductMode {
                mode: DockMode::Assist
            })
            .expect("switch to assist"),
        DesktopWorkflowOutcome::ProductModeChanged {
            mode: DockMode::Assist
        }
    );
    assert_eq!(runtime.projection_snapshot().product_mode, DockMode::Assist);

    match runtime
        .handle_action(DesktopAction::RequestAssistInlinePrediction {
            position: cursor_at_line_end(),
        })
        .expect("request inline prediction")
    {
        DesktopWorkflowOutcome::AssistInlinePredictionUpdated { active, .. } => {
            assert!(active, "Assist request should project active ghost text");
        }
        other => panic!("expected inline prediction outcome, got {other:?}"),
    }
    assert!(
        runtime
            .projection_snapshot()
            .assist_inline_prediction_projection
            .active_prediction
            .is_some()
    );

    assert_eq!(
        runtime
            .handle_action(DesktopAction::SetProductMode {
                mode: DockMode::Manual
            })
            .expect("switch back to manual"),
        DesktopWorkflowOutcome::ProductModeChanged {
            mode: DockMode::Manual
        }
    );
    let manual = runtime.projection_snapshot();
    assert_eq!(manual.product_mode, DockMode::Manual);
    assert!(
        manual
            .assist_inline_prediction_projection
            .active_prediction
            .is_none()
    );
    assert!(manual.assist_inline_prediction_projection.rows.is_empty());

    let _ = fs::remove_dir_all(root);
}
