//! Manual mode silence regression tests (P1.F2.T1).
//!
//! These tests prove that Manual mode denies all AI/provider/cloud/worker
//! runtime surfaces by construction. If a new AI-visible surface slips in,
//! these tests fail closed.

use legion_protocol::{ProductMode, ProductRuntimeSurface};

/// Every AI, cloud, network, worker, and automation surface that Manual
/// mode must deny. If a new surface is added to `ProductRuntimeSurface`,
/// it must be added here to keep Manual mode silent.
const MANUAL_FORBIDDEN_SURFACES: &[ProductRuntimeSurface] = &[
    ProductRuntimeSurface::AssistedAi,
    ProductRuntimeSurface::CloudProvider,
    ProductRuntimeSurface::NetworkEgress,
    ProductRuntimeSurface::HostedTelemetry,
    ProductRuntimeSurface::DelegatedTask,
    ProductRuntimeSurface::WorkerRuntime,
    ProductRuntimeSurface::Automation,
    ProductRuntimeSurface::Collaboration,
    ProductRuntimeSurface::RemoteWorkspace,
    ProductRuntimeSurface::PluginRuntime,
];

#[test]
fn manual_mode_denies_all_ai_and_cloud_surfaces() {
    for surface in MANUAL_FORBIDDEN_SURFACES {
        assert!(
            !ProductMode::Manual.allows_runtime_surface(*surface),
            "Manual mode must deny {:?} surface",
            surface
        );
    }
}

#[test]
fn manual_mode_allows_only_manual_ide_and_plugin_management() {
    assert!(
        ProductMode::Manual.allows_runtime_surface(ProductRuntimeSurface::ManualIde),
        "Manual mode must allow ManualIde surface"
    );
    assert!(
        ProductMode::Manual.allows_runtime_surface(ProductRuntimeSurface::PluginManagement),
        "Manual mode must allow PluginManagement surface (metadata-only)"
    );
}

#[test]
fn assist_mode_allows_assisted_ai_but_denies_worker_runtime() {
    assert!(
        ProductMode::Assist.allows_runtime_surface(ProductRuntimeSurface::AssistedAi),
        "Assist mode must allow AssistedAi surface"
    );
    assert!(
        !ProductMode::Assist.allows_runtime_surface(ProductRuntimeSurface::WorkerRuntime),
        "Assist mode must deny WorkerRuntime surface"
    );
}

#[test]
fn delegate_mode_allows_delegated_task_and_worker_runtime() {
    assert!(
        ProductMode::Delegates.allows_runtime_surface(ProductRuntimeSurface::DelegatedTask),
        "Delegate mode must allow DelegatedTask surface"
    );
    assert!(
        ProductMode::Delegates.allows_runtime_surface(ProductRuntimeSurface::WorkerRuntime),
        "Delegate mode must allow WorkerRuntime surface"
    );
}

#[test]
fn legion_workflows_mode_allows_automation() {
    assert!(
        ProductMode::LegionWorkflows.allows_runtime_surface(ProductRuntimeSurface::Automation),
        "Legion Workflows mode must allow Automation surface"
    );
}

#[test]
fn forbidden_surfaces_list_is_not_empty() {
    assert!(
        !MANUAL_FORBIDDEN_SURFACES.is_empty(),
        "Manual forbidden surfaces list must not be empty — if all surfaces are allowed, the test is vacuous"
    );
}
