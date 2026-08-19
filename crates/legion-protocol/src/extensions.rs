//! Extension catalog projection DTOs for the install / update / remove surface.
//!
//! P7.F2.T1 and P7.F2.T2. These are metadata-only projections: they carry what
//! the extensions panel must render and nothing the renderer could act on
//! directly. Verification, permission approval, and installation all remain in
//! app-owned authority over `legion-plugin`; the renderer sees the outcome and
//! sends back intents.
//!
//! The permission list is deliberately a `Vec` of per-capability rows rather
//! than a single trust flag. There is no field on any type here that grants an
//! extension as a whole.

use serde::{Deserialize, Serialize};

use crate::CapabilityId;

/// Signature posture of a catalog entry, as determined by app-owned verification.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ExtensionSignatureState {
    /// A trusted signer's signature verified over the exact bundled bytes.
    VerifiedSigned {
        /// Signer whose trust anchor validated the artifact.
        signer: String,
    },
    /// The artifact carries no signature at all. Never installable.
    Unsigned,
    /// A signature was present but did not verify. Never installable.
    VerificationFailed {
        /// Metadata-only refusal reason.
        reason: String,
    },
}

impl ExtensionSignatureState {
    /// Whether this posture permits installation.
    ///
    /// Only a verified signature does. Both failure postures are refusals, not
    /// warnings — this is the P7.F2.T1 and P7.F2.T3 stop condition expressed in
    /// the projection layer so a renderer cannot accidentally offer an install
    /// button for an artifact the app refused.
    pub fn is_installable(&self) -> bool {
        matches!(self, Self::VerifiedSigned { .. })
    }

    /// Stable lowercase label for rows and audit lines.
    pub fn label(&self) -> &'static str {
        match self {
            Self::VerifiedSigned { .. } => "signed",
            Self::Unsigned => "unsigned",
            Self::VerificationFailed { .. } => "signature-invalid",
        }
    }
}

/// Where a catalog entry currently sits in its lifecycle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ExtensionInstallState {
    /// Present in the catalog, not installed.
    Available,
    /// Installed at the catalog's current version.
    Installed,
    /// Installed, but the catalog offers a newer version.
    UpdateAvailable,
}

impl ExtensionInstallState {
    /// Stable lowercase label for rows and audit lines.
    pub fn label(self) -> &'static str {
        match self {
            Self::Available => "available",
            Self::Installed => "installed",
            Self::UpdateAvailable => "update-available",
        }
    }
}

/// The user's decision on one individual permission row.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ExtensionPermissionState {
    /// Not yet decided. Never treated as a grant.
    Undecided,
    /// Granted, for this capability only.
    Granted,
    /// Denied, for this capability only.
    Denied,
}

impl ExtensionPermissionState {
    /// Stable lowercase label for rows and audit lines.
    pub fn label(self) -> &'static str {
        match self {
            Self::Undecided => "undecided",
            Self::Granted => "granted",
            Self::Denied => "denied",
        }
    }
}

/// One reviewable permission: exactly one capability the extension requests.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExtensionPermissionProjection {
    /// 1-based position in the review list.
    pub ordinal: usize,
    /// Capability this row grants or withholds.
    pub capability: CapabilityId,
    /// Short human-readable title.
    pub title: String,
    /// Why the extension asks for it, derived from its declared contributions.
    pub reason: String,
    /// Authority classification label (`standard` or `elevated`).
    pub risk_label: String,
    /// The user's decision for this row alone.
    pub state: ExtensionPermissionState,
}

/// One extension offered by the catalog.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExtensionCatalogEntry {
    /// Stable manifest id, used as the intent target.
    pub manifest_id: String,
    /// Display name.
    pub display_name: String,
    /// Version offered by the catalog.
    pub version: String,
    /// Signature posture determined by app-owned verification.
    pub signature_state: ExtensionSignatureState,
    /// Lifecycle state.
    pub install_state: ExtensionInstallState,
    /// Per-capability permission rows. Never collapsed into one flag.
    pub permissions: Vec<ExtensionPermissionProjection>,
    /// Metadata-only refusal reason when the entry cannot be installed.
    pub blocked_reason: Option<String>,
}

impl ExtensionCatalogEntry {
    /// Whether an install may be offered for this entry right now.
    ///
    /// Requires a verified signature *and* every permission row individually
    /// granted. A renderer that asks this question gets the same answer the app
    /// will give, so the panel never shows a button that would be refused.
    pub fn can_install(&self) -> bool {
        self.signature_state.is_installable()
            && self.install_state == ExtensionInstallState::Available
            && self.every_permission_granted()
    }

    /// Whether an update may be offered for this entry right now.
    pub fn can_update(&self) -> bool {
        self.signature_state.is_installable()
            && self.install_state == ExtensionInstallState::UpdateAvailable
            && self.every_permission_granted()
    }

    /// Whether a remove may be offered for this entry right now.
    pub fn can_remove(&self) -> bool {
        matches!(
            self.install_state,
            ExtensionInstallState::Installed | ExtensionInstallState::UpdateAvailable
        )
    }

    /// Whether every requested permission has been individually granted.
    pub fn every_permission_granted(&self) -> bool {
        !self.permissions.is_empty()
            && self
                .permissions
                .iter()
                .all(|permission| permission.state == ExtensionPermissionState::Granted)
    }

    /// Capabilities still awaiting an individual decision.
    pub fn undecided_permissions(&self) -> Vec<&ExtensionPermissionProjection> {
        self.permissions
            .iter()
            .filter(|permission| permission.state == ExtensionPermissionState::Undecided)
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn permission(
        capability: &str,
        state: ExtensionPermissionState,
    ) -> ExtensionPermissionProjection {
        ExtensionPermissionProjection {
            ordinal: 1,
            capability: CapabilityId(capability.to_string()),
            title: format!("title {capability}"),
            reason: format!("reason {capability}"),
            risk_label: "elevated".to_string(),
            state,
        }
    }

    fn entry(
        signature_state: ExtensionSignatureState,
        permissions: Vec<ExtensionPermissionProjection>,
    ) -> ExtensionCatalogEntry {
        ExtensionCatalogEntry {
            manifest_id: "manifest-fixture".to_string(),
            display_name: "Fixture".to_string(),
            version: "1.0.0".to_string(),
            signature_state,
            install_state: ExtensionInstallState::Available,
            permissions,
            blocked_reason: None,
        }
    }

    #[test]
    fn an_unsigned_entry_can_never_be_installed() {
        let unsigned = entry(
            ExtensionSignatureState::Unsigned,
            vec![permission(
                "plugin.command",
                ExtensionPermissionState::Granted,
            )],
        );
        assert!(!unsigned.signature_state.is_installable());
        assert!(
            !unsigned.can_install(),
            "granting every permission must not make an unsigned artifact installable"
        );
    }

    #[test]
    fn a_failed_signature_can_never_be_installed() {
        let failed = entry(
            ExtensionSignatureState::VerificationFailed {
                reason: "signature did not verify".to_string(),
            },
            vec![permission(
                "plugin.command",
                ExtensionPermissionState::Granted,
            )],
        );
        assert!(!failed.can_install());
    }

    #[test]
    fn one_undecided_permission_blocks_the_install() {
        let partial = entry(
            ExtensionSignatureState::VerifiedSigned {
                signer: "legion-first-party".to_string(),
            },
            vec![
                permission("plugin.command", ExtensionPermissionState::Granted),
                permission(
                    "plugin.grammar.tree_sitter",
                    ExtensionPermissionState::Undecided,
                ),
            ],
        );
        assert!(!partial.every_permission_granted());
        assert!(!partial.can_install());
        assert_eq!(partial.undecided_permissions().len(), 1);
    }

    #[test]
    fn one_denied_permission_blocks_the_install() {
        let partial = entry(
            ExtensionSignatureState::VerifiedSigned {
                signer: "legion-first-party".to_string(),
            },
            vec![
                permission("plugin.command", ExtensionPermissionState::Granted),
                permission(
                    "plugin.grammar.tree_sitter",
                    ExtensionPermissionState::Denied,
                ),
            ],
        );
        assert!(!partial.can_install());
        assert!(partial.undecided_permissions().is_empty());
    }

    #[test]
    fn a_signed_and_fully_granted_entry_can_install() {
        let ready = entry(
            ExtensionSignatureState::VerifiedSigned {
                signer: "legion-first-party".to_string(),
            },
            vec![
                permission("plugin.command", ExtensionPermissionState::Granted),
                permission(
                    "plugin.grammar.tree_sitter",
                    ExtensionPermissionState::Granted,
                ),
            ],
        );
        assert!(ready.can_install());
        assert!(!ready.can_update());
        assert!(!ready.can_remove());
    }

    #[test]
    fn an_entry_with_no_permission_rows_is_not_silently_approved() {
        let empty = entry(
            ExtensionSignatureState::VerifiedSigned {
                signer: "legion-first-party".to_string(),
            },
            Vec::new(),
        );
        assert!(
            !empty.every_permission_granted(),
            "an empty permission list is not a full grant"
        );
        assert!(!empty.can_install());
    }
}
