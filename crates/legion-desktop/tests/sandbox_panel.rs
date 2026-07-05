use legion_desktop::view::DesktopProjectionViewModel;
use legion_protocol::DelegatedTaskRuntimeActivationState;
use legion_ui::{DockMode, Shell};

#[test]
fn sandbox_panel_surfaces_active_backend_and_caveats() {
    let mut snapshot = Shell::empty("Sandbox").projection_snapshot();
    snapshot.product_mode = DockMode::Delegate;
    snapshot.delegated_task_projection.runtime_activation =
        DelegatedTaskRuntimeActivationState::SandboxAllocated;

    let model = DesktopProjectionViewModel::from_snapshot(&snapshot);

    assert!(
        model
            .sandbox_rows
            .iter()
            .any(|row| row.contains("sandbox backend:")),
        "expected sandbox backend row, got: {:?}",
        model.sandbox_rows
    );
    assert!(
        model
            .sandbox_rows
            .iter()
            .any(|row| row.contains("sandbox caveat:")),
        "expected sandbox caveat row, got: {:?}",
        model.sandbox_rows
    );
    assert!(
        model
            .sandbox_rows
            .iter()
            .any(|row| row.contains("sandbox allocated") || row.contains("SandboxAllocated")),
        "expected active sandbox allocation row, got: {:?}",
        model.sandbox_rows
    );
}
