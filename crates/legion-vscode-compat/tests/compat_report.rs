use legion_protocol::{
    CapabilityId, CausalityId, CorrelationId, EventSequence, PluginId, VsCodeCompatibilityStatus,
    VsCodeExtensionHostRuntime,
};
use legion_vscode_compat::{load_open_vsx_extension, resolve_open_vsx_extension_metadata};
use serde_json::json;
use uuid::Uuid;

fn causality_id() -> CausalityId {
    CausalityId(Uuid::parse_str("bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb").unwrap())
}

#[test]
fn compat_report_loads_metadata_without_node_runtime() {
    let resolved = resolve_open_vsx_extension_metadata(
        "https://open-vsx.org",
        &json!({
            "namespace": "legion",
            "name": "theme-fixture",
            "version": "1.0.0",
            "files": {
                "download": "https://open-vsx.org/api/legion/theme-fixture/1.0.0/file/legion.theme-fixture-1.0.0.vsix"
            }
        }),
    )
    .expect("Open VSX metadata should resolve");

    let loaded = load_open_vsx_extension(
        PluginId(101),
        resolved,
        json!({
            "publisher": "legion",
            "name": "theme-fixture",
            "version": "1.0.0",
            "engines": { "vscode": "^1.90.0" },
            "contributes": {
                "themes": [{ "label": "Legion Dark" }]
            }
        }),
        CorrelationId(1001),
        causality_id(),
        EventSequence(1),
    )
    .expect("theme manifest should load as a compatibility report");

    assert_eq!(loaded.manifest.status, VsCodeCompatibilityStatus::Supported);
    assert_eq!(
        loaded.host_session.runtime,
        VsCodeExtensionHostRuntime::NoneRequired
    );
    assert_eq!(loaded.host_session.process_label, "none-required");
    assert!(
        !loaded.host_session.process_label.contains("node"),
        "metadata-only reporting must not leak a Node runtime label"
    );
}

#[test]
fn compat_report_does_not_select_node_runtime_for_tier2_extensions() {
    let resolved = resolve_open_vsx_extension_metadata(
        "https://open-vsx.org",
        &json!({
            "namespace": "legion",
            "name": "debug-fixture",
            "version": "1.0.0",
            "files": {
                "download": "https://open-vsx.org/api/legion/debug-fixture/1.0.0/file/legion.debug-fixture-1.0.0.vsix"
            }
        }),
    )
    .expect("Open VSX metadata should resolve");

    let loaded = load_open_vsx_extension(
        PluginId(102),
        resolved,
        json!({
            "publisher": "legion",
            "name": "debug-fixture",
            "version": "1.0.0",
            "activationEvents": ["onCommand:legion.run"],
            "contributes": {
                "commands": [{ "command": "legion.run", "title": "Run" }],
                "debuggers": [{ "type": "lldb" }],
                "views": {
                    "explorer": [{ "id": "legion.debugView", "name": "Debug View" }]
                }
            }
        }),
        CorrelationId(1002),
        causality_id(),
        EventSequence(2),
    )
    .expect("tier2 manifest should still normalize into a report");

    assert_eq!(
        loaded.manifest.status,
        VsCodeCompatibilityStatus::SupportedWithPolicy
    );
    assert_eq!(
        loaded.host_session.runtime,
        VsCodeExtensionHostRuntime::Deferred,
        "metadata-only ingestion must not choose a Node.js sidecar"
    );
    assert_eq!(loaded.host_session.process_label, "deferred-extension-host");
    assert!(
        loaded
            .manifest
            .requested_capabilities
            .contains(&CapabilityId("vscode.command.dispatch".to_string()))
    );
    assert!(
        loaded
            .manifest
            .requested_capabilities
            .contains(&CapabilityId("debug.adapter.dispatch".to_string()))
    );
    assert!(
        loaded
            .manifest
            .requested_capabilities
            .contains(&CapabilityId("vscode.view.project".to_string()))
    );
}
