use legion_desktop::view::DesktopProjectionViewModel;
use legion_protocol::DelegatedTaskRuntimeActivationState;
use legion_ui::{DockMode, Shell};

fn snapshot_with_activation(
    activation: DelegatedTaskRuntimeActivationState,
) -> legion_ui::ShellProjectionSnapshot {
    let mut snapshot = Shell::empty("Sandbox").projection_snapshot();
    snapshot.product_mode = DockMode::Delegate;
    snapshot.delegated_task_projection.runtime_activation = activation;
    snapshot
}

/// Active sandbox states should surface the backend and enforcement caveats with honest labels.
#[test]
fn sandbox_panel_surfaces_active_backend_and_caveats() {
    let snapshot = snapshot_with_activation(DelegatedTaskRuntimeActivationState::SandboxAllocated);
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

/// Strength labels must be honest: os-enforced*, process-lifetime-only / process-isolated, or fallback.
#[test]
fn sandbox_panel_shows_honest_strength_label_not_descriptor_only() {
    let snapshot = snapshot_with_activation(DelegatedTaskRuntimeActivationState::SandboxAllocated);
    let model = DesktopProjectionViewModel::from_snapshot(&snapshot);
    let all = model.sandbox_rows.join("\n");

    assert!(
        all.contains("os-enforced")
            || all.contains("process-isolated")
            || all.contains("process-lifetime-only")
            || all.contains("fs-write-only")
            || all.contains("fallback"),
        "sandbox rows must contain an honest enforcement label, got: {all}"
    );
    assert!(
        !all.contains("descriptor-only"),
        "sandbox rows must not contain 'descriptor-only' after PKT-SANDBOX landed real enforcement, got: {all}"
    );
    assert!(
        !all.contains("strong"),
        "sandbox rows must not contain 'strong' — dishonest label, got: {all}"
    );
}

/// NotEncoded/Planned activation states should produce NoSandbox rows.
#[test]
fn sandbox_panel_no_sandbox_state_shows_not_allocated() {
    for activation in [
        DelegatedTaskRuntimeActivationState::NotEncoded,
        DelegatedTaskRuntimeActivationState::Planned,
    ] {
        let snapshot = snapshot_with_activation(activation);
        let model = DesktopProjectionViewModel::from_snapshot(&snapshot);
        let all = model.sandbox_rows.join("\n");

        assert!(
            all.contains("no sandbox/worktree allocated yet"),
            "activation={activation:?}: expected 'no sandbox/worktree allocated yet', got: {all}"
        );
        assert!(
            !all.contains("sandbox backend:"),
            "activation={activation:?}: NoSandbox must not show backend, got: {all}"
        );
        assert!(
            !all.contains("sandbox isolation:"),
            "activation={activation:?}: NoSandbox must not show isolation mode, got: {all}"
        );
    }
}

/// Active sandbox states should show isolation mode and lease status.
#[test]
fn sandbox_panel_active_state_shows_isolation_and_lease() {
    let snapshot = snapshot_with_activation(DelegatedTaskRuntimeActivationState::Executing);
    let model = DesktopProjectionViewModel::from_snapshot(&snapshot);
    let all = model.sandbox_rows.join("\n");

    assert!(
        all.contains("sandbox isolation:"),
        "Active sandbox rows must show isolation mode, got: {all}"
    );
    assert!(
        all.contains("sandbox lease:"),
        "Active sandbox rows must show lease status, got: {all}"
    );
}
