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
fn compat_report_selects_node_sidecar_runtime_for_non_web_tier2_extensions() {
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
    // `commands`/`debuggers` contributions are Tier1, `views` is Tier2
    // (`Tier2ExtensionHostSidecar`) — this fixture caps at Tier2, not Tier3
    // (`webviews`/`notebooks`/`customEditors`), so `Deferred` never applies here.
    // Per the established tier-to-runtime mapping in
    // `extension_host_session_for_manifest` (see the `executable_entrypoint_requires_host_policy_without_activation_events`
    // unit test in `crates/legion-vscode-compat/src/lib.rs`), a non-web Tier2
    // extension resolves to `NodeSidecar`.
    assert_eq!(
        loaded.host_session.runtime,
        VsCodeExtensionHostRuntime::NodeSidecar,
        "non-web Tier2 (extension-host-sidecar) manifests resolve to a Node.js sidecar runtime"
    );
    assert_eq!(
        loaded.host_session.process_label,
        "node-extension-host-sidecar"
    );
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
