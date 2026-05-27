use std::{
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

use devil_desktop::{
    bridge::{DesktopAction, DesktopBridgeError, DesktopBridgeOutput, DesktopCommandBridge},
    view::DesktopProjectionViewModel,
    workflow::{
        DesktopLaunchConfig, DesktopPluginCommandStatus, DesktopRuntime, DesktopWorkflowOutcome,
    },
};
use devil_protocol::{
    CapabilityId, PluginActivationEvent, PluginCommandDescriptor, PluginContribution,
    PluginContributionProjection, PluginId, PluginManifest, PluginQuotaDeclaration,
    PluginStateNamespace, PluginTrustDecision, PluginTrustMetadata, PluginTrustSource,
};
use devil_ui::{CommandDispatchIntent, Shell};

static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

struct TempWorkspace {
    root: PathBuf,
}

impl TempWorkspace {
    fn new() -> Self {
        let temp_root = std::env::temp_dir();
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be after epoch")
            .as_nanos();
        let id = TEMP_COUNTER.fetch_add(1, Ordering::SeqCst);
        let root = temp_root.join(format!(
            "devil_desktop_plugin_management_{}_{}_{}",
            std::process::id(),
            nanos,
            id
        ));
        fs::create_dir(&root).expect("temp workspace should be created");
        Self { root }
    }

    fn path(&self) -> &Path {
        &self.root
    }
}

impl Drop for TempWorkspace {
    fn drop(&mut self) {
        let temp_root = std::env::temp_dir();
        let file_name = self.root.file_name().and_then(|name| name.to_str());
        if self.root.starts_with(&temp_root)
            && file_name.is_some_and(|name| name.starts_with("devil_desktop_plugin_management_"))
        {
            let _ = fs::remove_dir_all(&self.root);
        }
    }
}

fn command_descriptor() -> PluginCommandDescriptor {
    PluginCommandDescriptor {
        command_id: "phase8.run".to_string(),
        title: "Phase 8 Run".to_string(),
        required_capability: CapabilityId("plugin.command".to_string()),
    }
}

fn plugin_projection(plugin_id: PluginId) -> PluginContributionProjection {
    PluginContributionProjection {
        plugin_id,
        contributions: vec![PluginContribution::Command(command_descriptor())],
        status_label: "loaded".to_string(),
    }
}

fn plugin_snapshot() -> devil_ui::ShellProjectionSnapshot {
    let mut snapshot = Shell::empty("Plugin Management").projection_snapshot();
    snapshot.plugin_contribution_projections = vec![plugin_projection(PluginId(7))];
    snapshot
}

fn plugin_manifest(plugin_id: PluginId, max_host_calls: u32) -> PluginManifest {
    PluginManifest {
        plugin_id,
        name: "phase8.desktop".to_string(),
        version: "0.1.0".to_string(),
        schema_version: 1,
        min_abi_version: 1,
        max_abi_version: 1,
        module_hash: format!("sha256:phase8:{}", plugin_id.0),
        manifest_id: format!("manifest:phase8:{}", plugin_id.0),
        trust: PluginTrustMetadata {
            source: PluginTrustSource::ExplicitLocalAllow,
            decision: PluginTrustDecision::ExplicitlyAllowed,
            reason: "desktop plugin management test allow".to_string(),
        },
        signature: None,
        activation_events: vec![PluginActivationEvent::OnCommand {
            command: "phase8.run".to_string(),
        }],
        contributions: vec![PluginContribution::Command(command_descriptor())],
        requested_capabilities: vec![CapabilityId("plugin.command".to_string())],
        storage_namespace: PluginStateNamespace {
            plugin_id,
            namespace: "state".to_string(),
        },
        quotas: PluginQuotaDeclaration {
            max_fuel: 1000,
            max_wall_time_ms: 50,
            max_memory_pages: 8,
            max_storage_bytes: 4096,
            max_host_calls,
            max_events: 4,
            max_output_bytes: 512,
        },
    }
}

fn open_runtime() -> (TempWorkspace, DesktopRuntime) {
    let workspace = TempWorkspace::new();
    let runtime = DesktopRuntime::open(DesktopLaunchConfig::new(
        workspace.path().to_path_buf(),
        None,
    ))
    .expect("desktop runtime should open temp workspace");
    (workspace, runtime)
}

#[test]
fn plugin_management_command_routes_to_app_intent() {
    let snapshot = plugin_snapshot();
    let bridge = DesktopCommandBridge::new();

    assert_eq!(
        bridge.translate(
            DesktopAction::InvokePluginCommand {
                plugin_id: PluginId(7),
                command_id: " phase8.run ".to_string(),
            },
            &snapshot,
        ),
        DesktopBridgeOutput::Intent(CommandDispatchIntent::InvokePluginCommand {
            plugin_id: PluginId(7),
            command_id: "phase8.run".to_string(),
            metadata_label:
                "plugin 7 command phase8.run: Phase 8 Run (status=loaded capability=plugin.command)"
                    .to_string(),
        })
    );
}

#[test]
fn plugin_management_unknown_plugin_and_command_ids_are_rejected() {
    let snapshot = plugin_snapshot();
    let bridge = DesktopCommandBridge::new();

    assert_eq!(
        bridge.translate(
            DesktopAction::InvokePluginCommand {
                plugin_id: PluginId(8),
                command_id: "phase8.run".to_string(),
            },
            &snapshot,
        ),
        DesktopBridgeOutput::Error(DesktopBridgeError::UnknownPlugin {
            plugin_id: PluginId(8),
        })
    );
    assert_eq!(
        bridge.translate(
            DesktopAction::InvokePluginCommand {
                plugin_id: PluginId(7),
                command_id: "missing.run".to_string(),
            },
            &snapshot,
        ),
        DesktopBridgeOutput::Error(DesktopBridgeError::UnknownPluginCommand {
            plugin_id: PluginId(7),
            command_id: "missing.run".to_string(),
        })
    );
    assert_eq!(
        bridge.translate(
            DesktopAction::InvokePluginCommand {
                plugin_id: PluginId(7),
                command_id: "  ".to_string(),
            },
            &snapshot,
        ),
        DesktopBridgeOutput::Error(DesktopBridgeError::InvalidPluginCommand {
            plugin_id: PluginId(7),
        })
    );
}

#[test]
fn plugin_management_rows_include_metadata_only_audit_notes() {
    let model = DesktopProjectionViewModel::from_snapshot(&plugin_snapshot());

    assert!(model.plugin_rows.iter().any(|row| {
        row.contains("plugin management plugin 7")
            && row.contains("contributions=1")
            && row.contains("commands=1")
            && row.contains("sandbox=metadata-only")
            && row.contains("audit=app-owned")
    }));
    assert!(model.plugin_rows.iter().any(|row| {
        row.contains("command phase8.run")
            && row.contains("Phase 8 Run")
            && row.contains("capability=plugin.command")
            && row.contains("audit=dispatch-intent-only")
    }));
}

#[test]
fn plugin_management_workflow_reports_invoked_and_denied_statuses() {
    let (_workspace, mut runtime) = open_runtime();
    runtime
        .load_plugin_manifest(plugin_manifest(PluginId(7), 1))
        .expect("plugin manifest should load");

    let invoked = runtime
        .handle_action(DesktopAction::InvokePluginCommand {
            plugin_id: PluginId(7),
            command_id: "phase8.run".to_string(),
        })
        .expect("plugin command should route through app authority");
    assert!(matches!(
        invoked,
        DesktopWorkflowOutcome::PluginCommand {
            plugin_id: PluginId(7),
            ref command_id,
            status: DesktopPluginCommandStatus::Invoked,
            ref message,
        } if command_id == "phase8.run" && message.contains("Plugin command invoked")
    ));

    let denied = runtime
        .handle_action(DesktopAction::InvokePluginCommand {
            plugin_id: PluginId(7),
            command_id: "phase8.run".to_string(),
        })
        .expect("quota denial should be a workflow outcome");
    assert!(matches!(
        denied,
        DesktopWorkflowOutcome::PluginCommand {
            plugin_id: PluginId(7),
            ref command_id,
            status: DesktopPluginCommandStatus::Denied,
            ref message,
        } if command_id == "phase8.run"
            && message.contains("Plugin command denied")
            && message.contains("QuotaExceeded")
    ));
}
