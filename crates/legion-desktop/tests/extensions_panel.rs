//! P7.F2.T1 / P7.F2.T2 end-to-end: the extensions panel reaches a real install.
//!
//! These tests drive the same `DesktopAction` values the panel's buttons push,
//! through the real `DesktopRuntime` (bridge -> intent -> app-owned extension
//! authority -> `legion_plugin::SignedExtensionRegistry`), and read the result
//! back out of the projection the panel renders. Nothing here stubs the app.

use std::{
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

use legion_desktop::{
    bridge::{DesktopAction, DesktopBridgeError, DesktopBridgeOutput, DesktopCommandBridge},
    view::extensions_panel::DesktopExtensionsPanelViewModel,
    workflow::{DesktopLaunchConfig, DesktopRuntime},
};
use legion_protocol::{
    CapabilityId, ExtensionInstallState, ExtensionPermissionState, ExtensionSignatureState,
};
use legion_ui::CommandDispatchIntent;

static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Manifest id of the bundled first-party grammar extension.
const BUNDLED: &str = "legion.bundled.json-grammar";

struct TempWorkspace {
    root: PathBuf,
}

impl TempWorkspace {
    fn new() -> Self {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be after epoch")
            .as_nanos();
        let id = TEMP_COUNTER.fetch_add(1, Ordering::SeqCst);
        let root = std::env::temp_dir().join(format!(
            "legion_desktop_extensions_{}_{}_{}",
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
        if self.root.starts_with(std::env::temp_dir())
            && self
                .root
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with("legion_desktop_extensions_"))
        {
            let _ = fs::remove_dir_all(&self.root);
        }
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

fn panel(runtime: &DesktopRuntime) -> DesktopExtensionsPanelViewModel {
    DesktopExtensionsPanelViewModel::from_snapshot(&runtime.projection_snapshot())
}

/// The bundled extension really is offered by the shipping product, signed, and
/// not silently pre-installed.
#[test]
fn the_bundled_extension_is_offered_signed_and_uninstalled_at_launch() {
    let (_workspace, runtime) = open_runtime();
    let snapshot = runtime.projection_snapshot();

    let entry = snapshot
        .extension_catalog
        .iter()
        .find(|entry| entry.manifest_id == BUNDLED)
        .expect("the bundled grammar extension is offered by the product");
    assert_eq!(
        entry.signature_state,
        ExtensionSignatureState::VerifiedSigned {
            signer: "legion-first-party".to_string()
        }
    );
    assert_eq!(entry.install_state, ExtensionInstallState::Available);

    // Signed does not mean approved: the panel offers no install control yet.
    let model = panel(&runtime);
    let row = model
        .rows
        .iter()
        .find(|row| row.manifest_id == BUNDLED)
        .expect("panel renders the bundled entry");
    assert_eq!(row.install_action, None);
    assert!(!row.permissions.is_empty());
    assert!(
        row.permissions
            .iter()
            .all(|permission| permission.state == ExtensionPermissionState::Undecided)
    );
}

/// The acceptance: a user can install the bundled grammar extension through the
/// product UI, using only the actions the panel's controls emit.
#[test]
fn granting_each_permission_then_clicking_install_really_installs() {
    let (_workspace, mut runtime) = open_runtime();

    // Click Allow on every permission row, one capability at a time — exactly
    // what the panel's per-capability controls push.
    let allow_actions: Vec<DesktopAction> = panel(&runtime)
        .rows
        .iter()
        .find(|row| row.manifest_id == BUNDLED)
        .expect("panel renders the bundled entry")
        .permissions
        .iter()
        .map(|permission| permission.allow_action.clone())
        .collect();
    assert!(!allow_actions.is_empty());
    for action in allow_actions {
        runtime
            .handle_action(action)
            .expect("a permission grant is handled");
    }

    // Only now does the panel offer an install control.
    let install_action = panel(&runtime)
        .rows
        .iter()
        .find(|row| row.manifest_id == BUNDLED)
        .expect("panel renders the bundled entry")
        .install_action
        .clone()
        .expect("install becomes available once every permission is allowed");
    assert_eq!(
        install_action,
        DesktopAction::InstallExtension {
            manifest_id: BUNDLED.to_string()
        }
    );

    runtime
        .handle_action(install_action)
        .expect("install is handled by app-owned extension authority");

    // The projection the panel reads now reports it installed, and the panel
    // swaps the install control for a remove control.
    let entry = runtime
        .projection_snapshot()
        .extension_catalog
        .into_iter()
        .find(|entry| entry.manifest_id == BUNDLED)
        .expect("bundled entry is still projected");
    assert_eq!(entry.install_state, ExtensionInstallState::Installed);

    let row = panel(&runtime)
        .rows
        .into_iter()
        .find(|row| row.manifest_id == BUNDLED)
        .expect("panel renders the bundled entry");
    assert_eq!(row.install_action, None);
    assert_eq!(
        row.remove_action,
        Some(DesktopAction::RemoveExtension {
            manifest_id: BUNDLED.to_string()
        })
    );

    // Remove really removes.
    runtime
        .handle_action(row.remove_action.expect("remove control exists"))
        .expect("remove is handled");
    let entry = runtime
        .projection_snapshot()
        .extension_catalog
        .into_iter()
        .find(|entry| entry.manifest_id == BUNDLED)
        .expect("bundled entry is still projected after removal");
    assert_eq!(entry.install_state, ExtensionInstallState::Available);
}

/// Denying one capability is enough to keep the extension uninstalled.
#[test]
fn denying_a_single_permission_keeps_the_install_out_of_reach() {
    let (_workspace, mut runtime) = open_runtime();

    let deny_action = panel(&runtime)
        .rows
        .iter()
        .find(|row| row.manifest_id == BUNDLED)
        .expect("panel renders the bundled entry")
        .permissions
        .first()
        .expect("at least one permission row")
        .deny_action
        .clone();
    runtime
        .handle_action(deny_action)
        .expect("a permission denial is handled");

    let row = panel(&runtime)
        .rows
        .into_iter()
        .find(|row| row.manifest_id == BUNDLED)
        .expect("panel renders the bundled entry");
    assert_eq!(row.install_action, None);

    // And a forged gesture — an install action the panel never offered — is
    // refused by the bridge rather than reaching app authority.
    let bridge = DesktopCommandBridge::new();
    let snapshot = runtime.projection_snapshot();
    match bridge.translate(
        DesktopAction::InstallExtension {
            manifest_id: BUNDLED.to_string(),
        },
        &snapshot,
    ) {
        DesktopBridgeOutput::Error(DesktopBridgeError::ExtensionOperationUnavailable {
            manifest_id,
            operation,
            ..
        }) => {
            assert_eq!(manifest_id, BUNDLED);
            assert_eq!(operation, "installed");
        }
        other => panic!("expected a refusal, got {other:?}"),
    }
}

/// The bridge refuses a permission decision for a capability the extension does
/// not request, so a gesture cannot invent a review row.
#[test]
fn a_permission_decision_for_an_unrequested_capability_is_refused() {
    let (_workspace, runtime) = open_runtime();
    let bridge = DesktopCommandBridge::new();
    let snapshot = runtime.projection_snapshot();

    match bridge.translate(
        DesktopAction::SetExtensionPermission {
            manifest_id: BUNDLED.to_string(),
            capability: CapabilityId("plugin.workspace.scanner".to_string()),
            granted: true,
        },
        &snapshot,
    ) {
        DesktopBridgeOutput::Error(DesktopBridgeError::UnknownExtensionCapability {
            manifest_id,
            capability,
        }) => {
            assert_eq!(manifest_id, BUNDLED);
            assert_eq!(capability, "plugin.workspace.scanner");
        }
        other => panic!("expected a refusal, got {other:?}"),
    }
}

/// A gesture naming an extension that is not in the catalog is refused.
#[test]
fn an_unknown_extension_is_refused_by_the_bridge() {
    let (_workspace, runtime) = open_runtime();
    let bridge = DesktopCommandBridge::new();
    let snapshot = runtime.projection_snapshot();

    match bridge.translate(
        DesktopAction::InstallExtension {
            manifest_id: "not.in.the.catalog".to_string(),
        },
        &snapshot,
    ) {
        DesktopBridgeOutput::Error(DesktopBridgeError::UnknownExtension { manifest_id }) => {
            assert_eq!(manifest_id, "not.in.the.catalog");
        }
        other => panic!("expected a refusal, got {other:?}"),
    }
}

/// Every permission control translates to a single-capability intent.
#[test]
fn each_permission_control_carries_exactly_one_capability() {
    let (_workspace, runtime) = open_runtime();
    let bridge = DesktopCommandBridge::new();
    let snapshot = runtime.projection_snapshot();

    let model = panel(&runtime);
    let row = model
        .rows
        .iter()
        .find(|row| row.manifest_id == BUNDLED)
        .expect("panel renders the bundled entry");

    for permission in &row.permissions {
        match bridge.translate(permission.allow_action.clone(), &snapshot) {
            DesktopBridgeOutput::Intent(CommandDispatchIntent::SetExtensionPermission {
                manifest_id,
                capability,
                granted,
            }) => {
                assert_eq!(manifest_id, BUNDLED);
                assert!(granted);
                assert!(
                    row.permissions
                        .iter()
                        .filter(|other| other.allow_action == permission.allow_action)
                        .count()
                        == 1,
                    "capability {} must belong to exactly one control",
                    capability.0
                );
            }
            other => panic!("expected a single-capability intent, got {other:?}"),
        }
    }
}
