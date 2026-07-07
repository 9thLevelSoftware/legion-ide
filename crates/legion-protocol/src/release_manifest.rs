//! Release manifest DTO for the Legion auto-updater (ADR-0042).
//!
//! [`ReleaseManifestV1`] is a control document consumed by the signed-manifest
//! update path.  It records release metadata but **never key material** — the
//! `signer_reference` field holds only a reference string (an env-var name,
//! keyring service name, etc.).

use serde::{Deserialize, Serialize};

/// A single artifact entry inside a [`ReleaseManifestV1`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct ReleaseArtifact {
    /// Human-readable artifact name (e.g. `legion-desktop-windows-x64-msi`).
    pub name: String,
    /// Target platform (e.g. `windows`, `macos`, `linux`).
    pub platform: String,
    /// Rust target triple (e.g. `x86_64-pc-windows-msvc`).
    pub target: String,
    /// Artifact file path relative to the distribution root.
    pub artifact_path: String,
    /// SHA-256 hex digest of the artifact bytes.
    pub sha256: String,
}

/// Version-1 release manifest (ADR-0042 signed manifest format).
///
/// This struct is `#[non_exhaustive]` so that callers cannot exhaustively
/// match on its fields and future fields can be added without a breaking change.
///
/// **Security invariant**: `signer_reference` holds a *reference* only (env-var
/// name, keyring service label, KMS key URI, etc.).  Key material MUST NOT
/// appear here.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct ReleaseManifestV1 {
    /// Always `1` for this version.
    pub schema_version: u32,
    /// Package name (e.g. `legion-desktop`).
    pub package_name: String,
    /// Release channel: `"stable"` or `"preview"`.
    pub channel: String,
    /// Semver release version string.
    pub version: String,
    /// Optional rollout policy (e.g. `"full"`, `"staged"`).
    pub rollout_policy: Option<String>,
    /// Previous accepted version / rollback pointer.
    pub previous_version: Option<String>,
    /// Artifact entries covered by this manifest.
    pub artifacts: Vec<ReleaseArtifact>,
    /// Issuance timestamp in ISO 8601 / RFC 3339 UTC format.
    pub issued_at_utc: String,
    /// Signer reference string — **NEVER key material**.  Identifies the signer
    /// by env-var name, keyring label, or KMS URI so the material stays
    /// outside the repository.
    pub signer_reference: Option<String>,
}

impl ReleaseArtifact {
    /// Construct a new [`ReleaseArtifact`].
    ///
    /// Provided because the struct is `#[non_exhaustive]` and cannot be
    /// constructed with a struct-literal expression from outside the crate.
    pub fn new(
        name: String,
        platform: String,
        target: String,
        artifact_path: String,
        sha256: String,
    ) -> Self {
        Self {
            name,
            platform,
            target,
            artifact_path,
            sha256,
        }
    }
}

impl ReleaseManifestV1 {
    /// Construct a new [`ReleaseManifestV1`] with `schema_version = 1`.
    ///
    /// Provided because the struct is `#[non_exhaustive]` and cannot be
    /// constructed with a struct-literal expression from outside the crate.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        package_name: String,
        channel: String,
        version: String,
        rollout_policy: Option<String>,
        previous_version: Option<String>,
        artifacts: Vec<ReleaseArtifact>,
        issued_at_utc: String,
        signer_reference: Option<String>,
    ) -> Self {
        Self {
            schema_version: 1,
            package_name,
            channel,
            version,
            rollout_policy,
            previous_version,
            artifacts,
            issued_at_utc,
            signer_reference,
        }
    }

    /// Validate the manifest fields according to the ADR-0042 contract.
    ///
    /// Returns `Ok(())` when all invariants hold, or an `Err` with a
    /// human-readable description of the first failing constraint.
    pub fn validate(&self) -> Result<(), String> {
        if self.schema_version != 1 {
            return Err(format!(
                "schema_version must be 1, got {}",
                self.schema_version
            ));
        }
        if self.package_name.is_empty() {
            return Err("package_name must not be empty".to_string());
        }
        if self.channel.is_empty() {
            return Err("channel must not be empty".to_string());
        }
        if self.version.is_empty() {
            return Err("version must not be empty".to_string());
        }
        if self.artifacts.is_empty() {
            return Err("artifacts must contain at least one entry".to_string());
        }
        for artifact in &self.artifacts {
            if artifact.sha256.is_empty() {
                return Err(format!(
                    "artifact `{}` has an empty sha256 field",
                    artifact.name
                ));
            }
        }
        Ok(())
    }
}
