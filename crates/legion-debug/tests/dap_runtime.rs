use legion_debug::{DapClientConfig, DapClientRuntime, DapLifecycleState};
use legion_protocol::{
    DebugAdapterLaunchRequest, DebugConfigurationId, DebugSessionState, WorkspaceId,
};

fn launch_request() -> DebugAdapterLaunchRequest {
    DebugAdapterLaunchRequest {
        workspace_id: WorkspaceId(42),
        configuration_id: DebugConfigurationId("cargo:sample:bin:sample".to_string()),
        adapter_type: "lldb-dap".to_string(),
        breakpoints: Vec::new(),
        schema_version: 1,
    }
}

#[test]
fn dap_client_runtime_drives_launch_initialize_and_pause_lifecycle() {
    let runtime = DapClientRuntime::new(DapClientConfig::enabled());

    let outcome = runtime.launch(launch_request()).expect("launch dap client");

    assert_eq!(outcome.audit.state, DebugSessionState::Paused);
    assert_eq!(outcome.lifecycle_state, DapLifecycleState::Paused);
    assert_eq!(outcome.adapter_type, "lldb-dap");
    assert!(outcome.audit.metadata_summary.contains("initialized"));
    assert!(outcome.audit.metadata_summary.contains("launch"));
    assert!(outcome.console.iter().any(|entry| {
        entry.message_label.contains("initialize") && entry.message_label.contains("launch")
    }));
    assert!(
        outcome
            .stack_frames
            .iter()
            .any(|frame| frame.name == "main"),
        "non-fixture lifecycle still needs a metadata-only paused frame"
    );
}

#[test]
fn dap_client_runtime_rejects_launch_when_disabled() {
    let runtime = DapClientRuntime::new(DapClientConfig::default());

    let denied = runtime
        .launch(launch_request())
        .expect_err("runtime disabled");

    assert!(denied.to_string().contains("disabled"));
}
