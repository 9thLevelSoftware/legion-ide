//! Extensions panel: install / update / remove for signed extension artifacts.
//!
//! P7.F2.T1 and P7.F2.T2.
//!
//! The panel is split the way the other panels in this directory are: a pure
//! view model built from the projection snapshot (unit-testable with no render
//! pass) and a thin painter that only reads the model and pushes
//! [`DesktopAction`]s. No control here is decorative — every button carries a
//! prebuilt action that the bridge translates into a real
//! `CommandDispatchIntent`, which app-owned extension authority then executes
//! against `legion_plugin::SignedExtensionRegistry`.
//!
//! Two properties are enforced in the model rather than left to the painter:
//!
//! * **Unsigned and tamper-failed artifacts get no install control at all.**
//!   [`ExtensionRowViewModel::install_action`] is `None` unless the projection
//!   says the entry can install, and the projection only says that for a
//!   verified signature. (P7.F2.T1 stop condition.)
//! * **Permissions are one control per capability.** The model emits one
//!   [`ExtensionPermissionRowViewModel`] per requested capability, each with its
//!   own Allow and Deny actions naming that capability alone. There is no
//!   "trust this extension" affordance to build, because no action exists that
//!   could carry more than one capability. (P7.F2.T2 stop condition.)

use legion_protocol::{ExtensionCatalogEntry, ExtensionPermissionState, ExtensionSignatureState};
use legion_ui::ShellProjectionSnapshot;

use super::{components, theme};
use crate::bridge::DesktopAction;

/// One reviewable permission control: exactly one capability.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExtensionPermissionRowViewModel {
    /// Display text for the capability, e.g. `1. Provide a syntax grammar`.
    pub label: String,
    /// Why the extension asks for it.
    pub reason: String,
    /// Risk classification label.
    pub risk_label: String,
    /// The current per-capability decision.
    pub state: ExtensionPermissionState,
    /// Action that grants this one capability.
    pub allow_action: DesktopAction,
    /// Action that denies this one capability.
    pub deny_action: DesktopAction,
}

/// One extension in the catalog.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExtensionRowViewModel {
    /// Manifest id, used as the action target.
    pub manifest_id: String,
    /// Display heading, e.g. `Legion JSON Grammar 1.0.0`.
    pub heading: String,
    /// Signature posture summary shown next to the heading.
    pub signature_label: String,
    /// Lifecycle state summary.
    pub install_label: String,
    /// Why the entry cannot be acted on, when it cannot.
    pub blocked_reason: Option<String>,
    /// One control per requested capability, never fewer.
    pub permissions: Vec<ExtensionPermissionRowViewModel>,
    /// Install action, present only when the projection permits an install.
    pub install_action: Option<DesktopAction>,
    /// Update action, present only when the projection permits an update.
    pub update_action: Option<DesktopAction>,
    /// Remove action, present only when the extension is installed.
    pub remove_action: Option<DesktopAction>,
}

impl ExtensionRowViewModel {
    /// Whether any lifecycle control is offered for this row.
    pub fn has_lifecycle_control(&self) -> bool {
        self.install_action.is_some()
            || self.update_action.is_some()
            || self.remove_action.is_some()
    }
}

/// The extensions panel view model.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DesktopExtensionsPanelViewModel {
    /// One row per catalog entry, in projection order.
    pub rows: Vec<ExtensionRowViewModel>,
}

impl DesktopExtensionsPanelViewModel {
    /// Build the panel model from a projection snapshot.
    pub fn from_snapshot(snapshot: &ShellProjectionSnapshot) -> Self {
        Self {
            rows: snapshot.extension_catalog.iter().map(row).collect(),
        }
    }

    /// Whether the catalog is empty.
    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }

    /// Stable text rows for audit assertions and accessibility projections.
    pub fn audit_rows(&self) -> Vec<String> {
        let mut rows = Vec::new();
        for entry in &self.rows {
            rows.push(format!(
                "extension {}: {} signature={} state={} install_offered={} permissions={}",
                entry.manifest_id,
                entry.heading,
                entry.signature_label,
                entry.install_label,
                entry.install_action.is_some(),
                entry.permissions.len()
            ));
            for permission in &entry.permissions {
                rows.push(format!(
                    "extension {} permission {}: risk={} decision={} reason={}",
                    entry.manifest_id,
                    permission.label,
                    permission.risk_label,
                    permission.state.label(),
                    permission.reason
                ));
            }
            if let Some(reason) = &entry.blocked_reason {
                rows.push(format!("extension {} blocked: {reason}", entry.manifest_id));
            }
        }
        rows
    }
}

fn row(entry: &ExtensionCatalogEntry) -> ExtensionRowViewModel {
    let permissions = entry
        .permissions
        .iter()
        .map(|permission| ExtensionPermissionRowViewModel {
            label: format!("{}. {}", permission.ordinal, permission.title),
            reason: permission.reason.clone(),
            risk_label: permission.risk_label.clone(),
            state: permission.state,
            allow_action: DesktopAction::SetExtensionPermission {
                manifest_id: entry.manifest_id.clone(),
                capability: permission.capability.clone(),
                granted: true,
            },
            deny_action: DesktopAction::SetExtensionPermission {
                manifest_id: entry.manifest_id.clone(),
                capability: permission.capability.clone(),
                granted: false,
            },
        })
        .collect();

    ExtensionRowViewModel {
        manifest_id: entry.manifest_id.clone(),
        heading: format!("{} {}", entry.display_name, entry.version),
        signature_label: signature_label(&entry.signature_state),
        install_label: entry.install_state.label().to_string(),
        blocked_reason: entry.blocked_reason.clone(),
        permissions,
        // The projection is the authority on what may be offered. An unsigned
        // or tamper-failed entry answers `false` regardless of consent, so no
        // install control is ever built for it.
        install_action: entry
            .can_install()
            .then(|| DesktopAction::InstallExtension {
                manifest_id: entry.manifest_id.clone(),
            }),
        update_action: entry.can_update().then(|| DesktopAction::UpdateExtension {
            manifest_id: entry.manifest_id.clone(),
        }),
        remove_action: entry.can_remove().then(|| DesktopAction::RemoveExtension {
            manifest_id: entry.manifest_id.clone(),
        }),
    }
}

fn signature_label(state: &ExtensionSignatureState) -> String {
    match state {
        ExtensionSignatureState::VerifiedSigned { signer } => format!("signed by {signer}"),
        ExtensionSignatureState::Unsigned => "unsigned — refused".to_string(),
        ExtensionSignatureState::VerificationFailed { .. } => {
            "signature invalid — refused".to_string()
        }
    }
}

/// Paint the extensions panel and collect any gestures.
pub(crate) fn render_extensions_panel(
    ui: &mut egui::Ui,
    model: &DesktopExtensionsPanelViewModel,
    actions: &mut Vec<DesktopAction>,
) {
    if model.is_empty() {
        ui.label(theme::muted("No extensions are offered."));
        return;
    }

    for entry in &model.rows {
        theme::small_card_frame().show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.label(theme::body_strong(&entry.heading));
                let signed = entry.signature_label.starts_with("signed by");
                components::status_badge(
                    ui,
                    &entry.signature_label,
                    if signed {
                        theme::tokens().accent.green
                    } else {
                        theme::tokens().accent.red
                    },
                    true,
                );
                components::status_badge(
                    ui,
                    &entry.install_label,
                    theme::tokens().accent.blue,
                    true,
                );
            });

            if let Some(reason) = &entry.blocked_reason {
                ui.label(theme::accent(reason, theme::tokens().accent.red));
            }

            // One control pair per capability. Deliberately a loop over rows
            // and not a single toggle: see the module docs.
            components::section_header(
                ui,
                "Permissions requested",
                Some(theme::tokens().accent.orange),
            );
            for permission in &entry.permissions {
                ui.horizontal_wrapped(|ui| {
                    ui.label(theme::body(&permission.label));
                    ui.label(theme::muted(format!(
                        "{} · {}",
                        permission.risk_label, permission.reason
                    )));
                    let granted = permission.state == ExtensionPermissionState::Granted;
                    let denied = permission.state == ExtensionPermissionState::Denied;
                    if components::selectable_pill_button(
                        ui,
                        "Allow",
                        theme::tokens().accent.green,
                        granted,
                    )
                    .clicked()
                    {
                        actions.push(permission.allow_action.clone());
                    }
                    if components::selectable_pill_button(
                        ui,
                        "Deny",
                        theme::tokens().accent.red,
                        denied,
                    )
                    .clicked()
                    {
                        actions.push(permission.deny_action.clone());
                    }
                });
            }

            ui.horizontal_wrapped(|ui| {
                if let Some(action) = &entry.install_action
                    && components::primary_button(ui, "Install", theme::tokens().accent.green)
                        .clicked()
                {
                    actions.push(action.clone());
                }
                if let Some(action) = &entry.update_action
                    && components::primary_button(ui, "Update", theme::tokens().accent.blue)
                        .clicked()
                {
                    actions.push(action.clone());
                }
                if let Some(action) = &entry.remove_action
                    && components::soft_button(ui, "Remove").clicked()
                {
                    actions.push(action.clone());
                }
                if !entry.has_lifecycle_control() {
                    ui.label(theme::muted(
                        "Install becomes available once the artifact verifies and every permission is allowed.",
                    ));
                }
            });
        });
    }
}

#[cfg(test)]
mod tests {
    use legion_protocol::{
        CapabilityId, ExtensionInstallState, ExtensionPermissionProjection,
        ExtensionPermissionState, ExtensionSignatureState,
    };
    use legion_ui::Shell;

    use super::*;

    fn permission(
        ordinal: usize,
        capability: &str,
        state: ExtensionPermissionState,
    ) -> ExtensionPermissionProjection {
        ExtensionPermissionProjection {
            ordinal,
            capability: CapabilityId(capability.to_string()),
            title: format!("Use {capability}"),
            reason: format!("declared by {capability}"),
            risk_label: "elevated".to_string(),
            state,
        }
    }

    fn snapshot_with(entries: Vec<ExtensionCatalogEntry>) -> ShellProjectionSnapshot {
        let mut snapshot = Shell::empty("test").projection_snapshot();
        snapshot.extension_catalog = entries;
        snapshot
    }

    fn entry(
        signature_state: ExtensionSignatureState,
        install_state: ExtensionInstallState,
        permissions: Vec<ExtensionPermissionProjection>,
    ) -> ExtensionCatalogEntry {
        ExtensionCatalogEntry {
            manifest_id: "legion.bundled.json-grammar".to_string(),
            display_name: "Legion JSON Grammar".to_string(),
            version: "1.0.0".to_string(),
            signature_state,
            install_state,
            permissions,
            blocked_reason: None,
        }
    }

    fn signed() -> ExtensionSignatureState {
        ExtensionSignatureState::VerifiedSigned {
            signer: "legion-first-party".to_string(),
        }
    }

    #[test]
    fn an_empty_catalog_renders_no_rows() {
        let model = DesktopExtensionsPanelViewModel::from_snapshot(&snapshot_with(Vec::new()));
        assert!(model.is_empty());
        assert!(model.audit_rows().is_empty());
    }

    /// P7.F2.T2 stop condition, asserted at the renderer boundary.
    #[test]
    fn every_requested_capability_gets_its_own_pair_of_controls() {
        let model = DesktopExtensionsPanelViewModel::from_snapshot(&snapshot_with(vec![entry(
            signed(),
            ExtensionInstallState::Available,
            vec![
                permission(1, "plugin.command", ExtensionPermissionState::Undecided),
                permission(
                    2,
                    "plugin.grammar.tree_sitter",
                    ExtensionPermissionState::Undecided,
                ),
            ],
        )]));

        let row = &model.rows[0];
        assert_eq!(row.permissions.len(), 2);

        // Each control names exactly one capability, and a different one.
        let allow_capabilities: Vec<String> = row
            .permissions
            .iter()
            .map(|permission| match &permission.allow_action {
                DesktopAction::SetExtensionPermission {
                    capability,
                    granted,
                    ..
                } => {
                    assert!(*granted, "the allow control must grant");
                    capability.0.clone()
                }
                other => panic!("unexpected allow action: {other:?}"),
            })
            .collect();
        assert_eq!(
            allow_capabilities,
            vec![
                "plugin.command".to_string(),
                "plugin.grammar.tree_sitter".to_string()
            ]
        );

        for permission in &row.permissions {
            match &permission.deny_action {
                DesktopAction::SetExtensionPermission { granted, .. } => {
                    assert!(!*granted, "the deny control must deny");
                }
                other => panic!("unexpected deny action: {other:?}"),
            }
        }
    }

    /// P7.F2.T1 stop condition, asserted at the renderer boundary.
    #[test]
    fn an_unsigned_entry_is_given_no_install_control() {
        let model = DesktopExtensionsPanelViewModel::from_snapshot(&snapshot_with(vec![entry(
            ExtensionSignatureState::Unsigned,
            ExtensionInstallState::Available,
            vec![permission(
                1,
                "plugin.command",
                // Fully consented — and still not installable.
                ExtensionPermissionState::Granted,
            )],
        )]));

        let row = &model.rows[0];
        assert_eq!(row.signature_label, "unsigned — refused");
        assert_eq!(
            row.install_action, None,
            "an unsigned artifact must have no install control at any consent level"
        );
        assert!(!row.has_lifecycle_control());
    }

    #[test]
    fn a_tamper_failed_entry_is_given_no_install_control() {
        let mut catalog_entry = entry(
            ExtensionSignatureState::VerificationFailed {
                reason: "extension signature verification failed".to_string(),
            },
            ExtensionInstallState::Available,
            vec![permission(
                1,
                "plugin.command",
                ExtensionPermissionState::Granted,
            )],
        );
        catalog_entry.blocked_reason = Some("extension signature verification failed".to_string());
        let model =
            DesktopExtensionsPanelViewModel::from_snapshot(&snapshot_with(vec![catalog_entry]));

        let row = &model.rows[0];
        assert_eq!(row.signature_label, "signature invalid — refused");
        assert_eq!(row.install_action, None);
        assert!(row.blocked_reason.is_some());
        assert!(
            model
                .audit_rows()
                .iter()
                .any(|line| line.contains("blocked: extension signature verification failed"))
        );
    }

    #[test]
    fn an_undecided_permission_withholds_the_install_control() {
        let model = DesktopExtensionsPanelViewModel::from_snapshot(&snapshot_with(vec![entry(
            signed(),
            ExtensionInstallState::Available,
            vec![
                permission(1, "plugin.command", ExtensionPermissionState::Granted),
                permission(
                    2,
                    "plugin.grammar.tree_sitter",
                    ExtensionPermissionState::Undecided,
                ),
            ],
        )]));
        assert_eq!(model.rows[0].install_action, None);
    }

    #[test]
    fn a_denied_permission_withholds_the_install_control() {
        let model = DesktopExtensionsPanelViewModel::from_snapshot(&snapshot_with(vec![entry(
            signed(),
            ExtensionInstallState::Available,
            vec![
                permission(1, "plugin.command", ExtensionPermissionState::Granted),
                permission(
                    2,
                    "plugin.grammar.tree_sitter",
                    ExtensionPermissionState::Denied,
                ),
            ],
        )]));
        assert_eq!(model.rows[0].install_action, None);
    }

    #[test]
    fn a_signed_and_fully_granted_entry_gets_an_install_control() {
        let model = DesktopExtensionsPanelViewModel::from_snapshot(&snapshot_with(vec![entry(
            signed(),
            ExtensionInstallState::Available,
            vec![permission(
                1,
                "plugin.grammar.tree_sitter",
                ExtensionPermissionState::Granted,
            )],
        )]));

        let row = &model.rows[0];
        assert_eq!(
            row.install_action,
            Some(DesktopAction::InstallExtension {
                manifest_id: "legion.bundled.json-grammar".to_string()
            })
        );
        assert_eq!(row.update_action, None);
        assert_eq!(row.remove_action, None);
        assert_eq!(row.signature_label, "signed by legion-first-party");
    }

    #[test]
    fn an_installed_entry_offers_remove_and_not_install() {
        let model = DesktopExtensionsPanelViewModel::from_snapshot(&snapshot_with(vec![entry(
            signed(),
            ExtensionInstallState::Installed,
            vec![permission(
                1,
                "plugin.grammar.tree_sitter",
                ExtensionPermissionState::Granted,
            )],
        )]));

        let row = &model.rows[0];
        assert_eq!(row.install_action, None);
        assert_eq!(
            row.remove_action,
            Some(DesktopAction::RemoveExtension {
                manifest_id: "legion.bundled.json-grammar".to_string()
            })
        );
    }

    #[test]
    fn an_outdated_entry_offers_update_and_remove() {
        let model = DesktopExtensionsPanelViewModel::from_snapshot(&snapshot_with(vec![entry(
            signed(),
            ExtensionInstallState::UpdateAvailable,
            vec![permission(
                1,
                "plugin.grammar.tree_sitter",
                ExtensionPermissionState::Granted,
            )],
        )]));

        let row = &model.rows[0];
        assert_eq!(
            row.update_action,
            Some(DesktopAction::UpdateExtension {
                manifest_id: "legion.bundled.json-grammar".to_string()
            })
        );
        assert!(row.remove_action.is_some());
        assert_eq!(row.install_action, None);
    }
}
