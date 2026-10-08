//! Auto-updater client for Legion IDE (ADR-0042).
//!
//! # Manifest sources
//!
//! [`LocalDirManifestSource`] reads a directory. `HttpManifestSource`
//! (feature `updater-http`) reads a signed HTTP manifest. The production feed
//! URL and Ed25519 verifying key are owner-provided process configuration
//! ([`UpdateFeedConfig`]); neither is compiled into the binary.
//!
//! Product checks go through [`Updater::poll`] or
//! `Updater::fetch_stage_swap_and_launch`. Manual mode returns before any
//! fetch. [`Updater::check_for_update`] stays mode-agnostic so the local
//! update drill can exercise verification without a feed.
//!
//! # Security invariants
//!
//! * Ed25519 signature verification runs **BEFORE** TOML parsing (fail-closed).
//!   A tampered manifest cannot reach the parser.
//! * Unsigned manifests are rejected unless
//!   [`UpdatePolicy::allow_unsigned_beta`] is `true`.
//! * A candidate that is not strictly newer is [`UpdateCheck::NoUpdate`].
//!   Downgrades are not installed.
//! * Key material is never logged or persisted.
//! * File operations use copy-then-rename (never in-place overwrite) for
//!   Windows safety.
//! * Applying an update copies the installed package to an N-1 slot, swaps in
//!   the staged package, launches it, and restores N-1 when launch fails.

use std::{
    cmp::Ordering,
    fs,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use legion_protocol::ReleaseManifestV1;

// ---------------------------------------------------------------------------
// UpdateError
// ---------------------------------------------------------------------------

/// All errors that can occur during an update pipeline step.
#[derive(Debug, thiserror::Error)]
pub enum UpdateError {
    /// Ed25519 signature verification failed.
    #[error("signature verification failed: {0}")]
    SignatureInvalid(String),

    /// The manifest carries no signature and the policy forbids unsigned builds.
    #[error("unsigned manifest not allowed by policy (allow_unsigned_beta is false)")]
    UnsignedNotAllowed,

    /// The manifest channel does not match the policy channel.
    #[error("channel mismatch: manifest channel is `{manifest}`, policy channel is `{policy}`")]
    ChannelMismatch {
        /// Channel name from the manifest.
        manifest: String,
        /// Channel name from the policy.
        policy: String,
    },

    /// [`ReleaseManifestV1::validate`] returned an error.
    #[error("manifest validation failed: {0}")]
    ManifestInvalid(String),

    /// An artifact's SHA-256 digest did not match the manifest entry.
    #[error("artifact hash mismatch for `{name}`: expected {expected}, got {actual}")]
    HashMismatch {
        /// Artifact name from the manifest.
        name: String,
        /// Expected hex digest (from manifest).
        expected: String,
        /// Actual hex digest (computed from file on disk).
        actual: String,
    },

    /// An artifact listed in the manifest was not found on disk.
    #[error("artifact not found: {path}")]
    ArtifactNotFound {
        /// Path that was expected to exist.
        path: String,
    },

    /// A journal operation failed because no previous version is recorded.
    #[error("journal has no previous_version; cannot rollback from initial state")]
    NoPreviousVersion,

    /// Filesystem I/O error.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// TOML serialization/deserialization error.
    #[error("TOML error: {0}")]
    Toml(String),

    /// The update feed request failed, or owner feed configuration is invalid.
    #[error("update feed error: {0}")]
    Feed(String),

    /// An artifact path is absolute, contains `..`, or is otherwise not a
    /// relative package path.
    #[error("refusing artifact path `{0}`")]
    UnsafePath(String),
}

// ---------------------------------------------------------------------------
// ManifestSource trait
// ---------------------------------------------------------------------------

/// A source of raw manifest bytes and an optional detached Ed25519 signature.
///
/// [`LocalDirManifestSource`] reads a directory. `HttpManifestSource` reads
/// a signed HTTP feed when the `updater-http` feature is enabled.
pub trait ManifestSource {
    /// Fetch the manifest.
    ///
    /// Returns `(manifest_bytes, optional_signature_bytes)`.  When a `.sig`
    /// file is present alongside the manifest, its bytes are returned as the
    /// second element; otherwise `None` is returned and the caller decides
    /// whether to accept an unsigned manifest via [`UpdatePolicy`].
    fn fetch_manifest(&self) -> Result<(Vec<u8>, Option<Vec<u8>>), UpdateError>;
}

// ---------------------------------------------------------------------------
// LocalDirManifestSource
// ---------------------------------------------------------------------------

/// Reads `release-manifest.v1.toml` and, when present,
/// `release-manifest.v1.toml.sig` from a local directory.
///
/// Directory feed used by the local update drill. The configured HTTP feed is
/// [`HttpManifestSource`].
pub struct LocalDirManifestSource {
    /// Directory that contains the manifest (and optional `.sig`) file.
    pub dir: PathBuf,
}

impl LocalDirManifestSource {
    /// Construct a source that reads from `dir`.
    pub fn new(dir: impl Into<PathBuf>) -> Self {
        Self { dir: dir.into() }
    }
}

/// Canonical manifest filename.
pub const MANIFEST_FILE: &str = "release-manifest.v1.toml";
/// Canonical detached-signature filename.
pub const SIG_FILE: &str = "release-manifest.v1.toml.sig";

impl ManifestSource for LocalDirManifestSource {
    fn fetch_manifest(&self) -> Result<(Vec<u8>, Option<Vec<u8>>), UpdateError> {
        let manifest_path = self.dir.join(MANIFEST_FILE);
        let manifest_bytes = fs::read(&manifest_path).map_err(|e| {
            UpdateError::Io(std::io::Error::new(
                e.kind(),
                format!("reading {}: {e}", manifest_path.display()),
            ))
        })?;

        let sig_path = self.dir.join(SIG_FILE);
        let sig_bytes = if sig_path.is_file() {
            Some(fs::read(&sig_path).map_err(|e| {
                UpdateError::Io(std::io::Error::new(
                    e.kind(),
                    format!("reading {}: {e}", sig_path.display()),
                ))
            })?)
        } else {
            None
        };

        Ok((manifest_bytes, sig_bytes))
    }
}

// ---------------------------------------------------------------------------
// UpdatePolicy
// ---------------------------------------------------------------------------

/// Policy that governs the update decision.
pub struct UpdatePolicy {
    /// Version string of the currently-installed build (e.g. `"0.1.0"`).
    pub current_version: String,
    /// Update channel of the running build: `"stable"` or `"preview"`.
    pub current_channel: String,
    /// When `true`, a manifest with no detached signature is accepted and
    /// journaled as `"unsigned-beta"`.  When `false`, unsigned manifests are
    /// rejected with [`UpdateError::UnsignedNotAllowed`].
    pub allow_unsigned_beta: bool,
}

// ---------------------------------------------------------------------------
// UpdateCheck
// ---------------------------------------------------------------------------

/// The result of [`Updater::check_for_update`].
#[derive(Debug)]
pub enum UpdateCheck {
    /// A version newer than the installed one is available.
    Available {
        /// Parsed and validated manifest.
        manifest: Box<ReleaseManifestV1>,
        /// `"signed/ed25519"` when the manifest carried a valid signature,
        /// `"unsigned-beta"` otherwise (only when policy permits).
        signer_status: String,
        /// The version that was current at check time (from `UpdatePolicy::current_version`).
        /// Carried forward so `apply_update` can record it as `previous_version`.
        previous_version: String,
    },
    /// The installed version is already at or above the available version.
    NoUpdate,
}

// ---------------------------------------------------------------------------
// StagedUpdate
// ---------------------------------------------------------------------------

/// An update whose artifact hashes have been verified and whose files have been
/// copied into a staging directory.
#[derive(Debug)]
pub struct StagedUpdate {
    /// Parsed and validated manifest.
    pub manifest: ReleaseManifestV1,
    /// Directory holding the staged artifact copies.
    pub staged_dir: PathBuf,
    /// Propagated from [`UpdateCheck::Available`].
    pub signer_status: String,
    /// The version that was running before this update was staged.
    /// Sourced from [`UpdatePolicy::current_version`] via [`UpdateCheck::Available`].
    pub previous_version: Option<String>,
}

// ---------------------------------------------------------------------------
// UpdateJournal
// ---------------------------------------------------------------------------

/// In-memory representation of the TOML update journal.
///
/// Written as the `[journal]` table by [`Updater::apply_update`] and toggled
/// by [`Updater::rollback`].
///
/// ```toml
/// [journal]
/// current_version  = "0.2.0"
/// previous_version = "0.1.0"
/// channel          = "stable"
/// staged_at        = "2026-07-07T12:00:00Z"
/// signer_status    = "signed/ed25519"
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct UpdateJournal {
    /// Version string of the currently-installed (post-update) build.
    pub current_version: String,
    /// Version string that was running before the last `apply_update`. `None` if this
    /// is the first update journal entry.
    pub previous_version: Option<String>,
    /// Release channel (e.g. `"stable"` or `"preview"`).
    pub channel: String,
    /// UTC timestamp when the update was staged / the journal was written.
    pub staged_at: String,
    /// `"signed/ed25519"` or `"unsigned-beta"` — propagated from the manifest check.
    pub signer_status: String,
}

/// Wrapper that serializes the journal under a `[journal]` TOML table.
#[derive(Debug, Serialize, Deserialize)]
struct JournalFile {
    journal: UpdateJournal,
}

// ---------------------------------------------------------------------------
// Updater
// ---------------------------------------------------------------------------

/// Stateless update client.
///
/// All mutable state lives in the on-disk journal and the staged artifact
/// directory; this struct carries no fields.
pub struct Updater;

impl Updater {
    /// Create a new `Updater`.
    pub fn new() -> Self {
        Self
    }

    /// Check whether an update is available.
    ///
    /// # Pipeline
    ///
    /// 1. Fetch raw manifest bytes + optional signature via `source`.
    /// 2. **If a signature is present**: verify the Ed25519 signature over the
    ///    raw manifest bytes *before* parsing (fail-closed; a bad sig is rejected
    ///    even if the TOML is syntactically valid).
    /// 3. **If no signature**: check `policy.allow_unsigned_beta`; if `false`,
    ///    return [`UpdateError::UnsignedNotAllowed`].
    /// 4. Parse the manifest TOML into [`ReleaseManifestV1`].
    /// 5. Validate the manifest (call [`ReleaseManifestV1::validate`]).
    /// 6. Reject channel mismatch.
    /// 7. Compare versions; return [`UpdateCheck::NoUpdate`] if not newer.
    ///
    /// # Parameters
    ///
    /// * `verifying_key` — the 32-byte Ed25519 verifying (public) key bytes.
    ///   Required when the manifest carries a signature; ignored otherwise.
    pub fn check_for_update(
        &self,
        source: &dyn ManifestSource,
        policy: &UpdatePolicy,
        verifying_key: Option<&[u8]>,
    ) -> Result<UpdateCheck, UpdateError> {
        let (manifest_bytes, sig_bytes) = source.fetch_manifest()?;

        // Step 2 / 3: Signature gate — runs BEFORE TOML parsing (fail-closed).
        let signer_status = match &sig_bytes {
            Some(sig) => {
                let vk = verifying_key.ok_or_else(|| {
                    UpdateError::SignatureInvalid(
                        "signature bytes present but no verifying key supplied".to_string(),
                    )
                })?;
                verify_ed25519_signature(&manifest_bytes, sig, vk)
                    .map_err(UpdateError::SignatureInvalid)?;
                "signed/ed25519".to_string()
            }
            None => {
                if !policy.allow_unsigned_beta {
                    return Err(UpdateError::UnsignedNotAllowed);
                }
                "unsigned-beta".to_string()
            }
        };

        // Step 4: Parse TOML (after signature check).
        let manifest_str = String::from_utf8_lossy(&manifest_bytes);
        let manifest: ReleaseManifestV1 =
            toml::from_str(&manifest_str).map_err(|e| UpdateError::Toml(e.to_string()))?;

        // Step 5: Validate.
        manifest.validate().map_err(UpdateError::ManifestInvalid)?;

        // Step 6: Channel check.
        if manifest.channel != policy.current_channel {
            return Err(UpdateError::ChannelMismatch {
                manifest: manifest.channel.clone(),
                policy: policy.current_channel.clone(),
            });
        }

        // Step 7: Version comparison.
        if !version_is_newer(&manifest.version, &policy.current_version) {
            return Ok(UpdateCheck::NoUpdate);
        }

        Ok(UpdateCheck::Available {
            manifest: Box::new(manifest),
            signer_status,
            previous_version: policy.current_version.clone(),
        })
    }

    /// Verify artifact hashes and copy artifacts into a staging directory.
    ///
    /// For every artifact entry in `manifest`:
    /// 1. Locate the file in `artifacts_dir` via `artifact.artifact_path`.
    /// 2. Compute its SHA-256 digest and compare to `artifact.sha256`.
    /// 3. Copy it to `<artifacts_dir>/staged/<artifact_path>`.
    ///
    /// `previous_version` should be set to the currently-installed version string
    /// (i.e. `UpdateCheck::Available::previous_version`) so the journal can record
    /// the before/after pair when [`apply_update`][Updater::apply_update] runs.
    pub fn stage_update(
        &self,
        manifest: ReleaseManifestV1,
        artifacts_dir: &Path,
        signer_status: String,
        previous_version: Option<String>,
    ) -> Result<StagedUpdate, UpdateError> {
        for artifact in &manifest.artifacts {
            let src_path = artifacts_dir.join(&artifact.artifact_path);
            if !src_path.is_file() {
                return Err(UpdateError::ArtifactNotFound {
                    path: src_path.to_string_lossy().into_owned(),
                });
            }
            let bytes = fs::read(&src_path)?;
            let actual = hex::encode(Sha256::digest(&bytes));
            if actual != artifact.sha256 {
                return Err(UpdateError::HashMismatch {
                    name: artifact.name.clone(),
                    expected: artifact.sha256.clone(),
                    actual,
                });
            }
        }

        let staged_dir = artifacts_dir.join("staged");
        fs::create_dir_all(&staged_dir)?;

        for artifact in &manifest.artifacts {
            let src = artifacts_dir.join(&artifact.artifact_path);
            let dst = staged_dir.join(&artifact.artifact_path);
            if let Some(parent) = dst.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::copy(&src, &dst)?;
        }

        Ok(StagedUpdate {
            manifest,
            staged_dir,
            signer_status,
            previous_version,
        })
    }

    /// Apply a staged update: write or update the TOML journal at `journal_path`.
    ///
    /// Records the new version as `current_version` and the previously-current
    /// version as `previous_version`.
    ///
    /// This records the journal only. [`Updater::swap_keep_previous_and_launch`]
    /// performs the package swap, launch, and N-1 rollback.
    pub fn apply_update(
        &self,
        staged: &StagedUpdate,
        journal_path: &Path,
        now_utc: &str,
    ) -> Result<UpdateJournal, UpdateError> {
        // Determine previous_version: prefer the on-disk journal's current
        // version if one exists; otherwise use the version the caller recorded
        // at check-time in `staged.previous_version`.
        let previous_version = if journal_path.is_file() {
            let text = fs::read_to_string(journal_path)?;
            toml::from_str::<JournalFile>(&text)
                .ok()
                .map(|f| f.journal.current_version)
        } else {
            staged.previous_version.clone()
        };

        let journal = UpdateJournal {
            current_version: staged.manifest.version.clone(),
            previous_version,
            channel: staged.manifest.channel.clone(),
            staged_at: now_utc.to_string(),
            signer_status: staged.signer_status.clone(),
        };

        write_journal(journal_path, &journal)?;
        Ok(journal)
    }

    /// Rollback by swapping `current_version` and `previous_version`.
    ///
    /// This is an **idempotent toggle**: calling rollback twice returns the
    /// journal to the post-`apply_update` state.
    ///
    /// Returns [`UpdateError::NoPreviousVersion`] if the journal has no
    /// `previous_version` entry (i.e. it was never updated from a prior state).
    pub fn rollback(
        &self,
        journal_path: &Path,
        now_utc: &str,
    ) -> Result<UpdateJournal, UpdateError> {
        let text = fs::read_to_string(journal_path)?;
        let file: JournalFile =
            toml::from_str(&text).map_err(|e| UpdateError::Toml(e.to_string()))?;
        let existing = file.journal;

        let prev = existing
            .previous_version
            .clone()
            .ok_or(UpdateError::NoPreviousVersion)?;

        let rolled = UpdateJournal {
            current_version: prev,
            previous_version: Some(existing.current_version),
            channel: existing.channel,
            staged_at: now_utc.to_string(),
            signer_status: existing.signer_status,
        };

        write_journal(journal_path, &rolled)?;
        Ok(rolled)
    }
}

impl Default for Updater {
    fn default() -> Self {
        Self::new()
    }
}

/// Result of [`Updater::poll`].
#[derive(Debug)]
pub enum UpdatePoll {
    /// Manual mode does not contact the feed.
    SkippedManual,
    /// The feed was fetched and checked.
    Checked(UpdateCheck),
}

/// Result of swapping in a staged package and launching it.
#[derive(Debug)]
pub struct InstalledPackage {
    /// Journal after the swap, or after N-1 rollback when launch failed.
    pub journal: UpdateJournal,
    /// Exit code of the launched package. `None` when the process could not be spawned.
    pub launch_exit_code: Option<i32>,
    /// `true` when launch failed and the N-1 package was restored.
    pub rolled_back: bool,
}

/// Owner-provided update feed. No production URL or key is compiled in.
///
/// * `LEGION_UPDATE_MANIFEST_URL` — manifest URL ending in
///   `release-manifest.v1.toml`. OWNER-PROVIDED. Unset means no feed.
/// * `LEGION_UPDATE_VERIFYING_KEY_HEX` — 32-byte Ed25519 verifying key, hex
///   encoded. OWNER-PROVIDED. Unset means no key.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpdateFeedConfig {
    /// Manifest URL from process configuration.
    pub manifest_url: Option<String>,
    /// 32-byte verifying key from process configuration.
    pub verifying_key: Option<Vec<u8>>,
}

impl UpdateFeedConfig {
    /// Environment variable for the owner-provided manifest URL.
    pub const MANIFEST_URL_ENV: &'static str = "LEGION_UPDATE_MANIFEST_URL";
    /// Environment variable for the owner-provided verifying key, hex encoded.
    pub const VERIFYING_KEY_HEX_ENV: &'static str = "LEGION_UPDATE_VERIFYING_KEY_HEX";

    /// Read owner feed configuration from the process environment.
    pub fn from_env() -> Result<Self, UpdateError> {
        let url = match std::env::var(Self::MANIFEST_URL_ENV) {
            Ok(value) => Some(value),
            Err(std::env::VarError::NotPresent) => None,
            Err(std::env::VarError::NotUnicode(_)) => {
                return Err(UpdateError::Feed(format!(
                    "{} is not unicode",
                    Self::MANIFEST_URL_ENV
                )));
            }
        };
        let key = match std::env::var(Self::VERIFYING_KEY_HEX_ENV) {
            Ok(value) => Some(value),
            Err(std::env::VarError::NotPresent) => None,
            Err(std::env::VarError::NotUnicode(_)) => {
                return Err(UpdateError::Feed(format!(
                    "{} is not unicode",
                    Self::VERIFYING_KEY_HEX_ENV
                )));
            }
        };
        Self::parse(url.as_deref(), key.as_deref())
    }

    /// Parse owner feed configuration from explicit strings.
    ///
    /// Empty strings are treated as unset. This does not contact the network.
    pub fn parse(
        manifest_url: Option<&str>,
        verifying_key_hex: Option<&str>,
    ) -> Result<Self, UpdateError> {
        let manifest_url = match manifest_url
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            Some(url) => {
                validate_manifest_url(url)?;
                Some(url.to_string())
            }
            None => None,
        };
        let verifying_key = match verifying_key_hex
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            Some(hex_key) => {
                let bytes = hex::decode(hex_key)
                    .map_err(|err| UpdateError::Feed(format!("verifying key hex: {err}")))?;
                if bytes.len() != 32 {
                    return Err(UpdateError::Feed(format!(
                        "verifying key must be 32 bytes, got {}",
                        bytes.len()
                    )));
                }
                Some(bytes)
            }
            None => None,
        };
        Ok(Self {
            manifest_url,
            verifying_key,
        })
    }
}

/// Upper bound for one manifest, signature, or package object.
pub const MAX_FEED_OBJECT_BYTES: u64 = 512 * 1024 * 1024;

impl Updater {
    /// Check for an update unless the product is in Manual mode.
    ///
    /// Manual returns [`UpdatePoll::SkippedManual`] before `source` is called.
    pub fn poll(
        &self,
        mode: crate::AppProductMode,
        source: &dyn ManifestSource,
        policy: &UpdatePolicy,
        verifying_key: Option<&[u8]>,
    ) -> Result<UpdatePoll, UpdateError> {
        if mode == crate::AppProductMode::Manual {
            return Ok(UpdatePoll::SkippedManual);
        }
        self.check_for_update(source, policy, verifying_key)
            .map(UpdatePoll::Checked)
    }

    /// Copy the installed package to `previous_path` (N-1), swap in the named
    /// staged artifact, write the journal, and launch `current_path`.
    ///
    /// A non-zero exit or a spawn failure restores N-1 and toggles the journal
    /// back. The restored journal is returned with `rolled_back` set.
    pub fn swap_keep_previous_and_launch(
        &self,
        staged: &StagedUpdate,
        artifact_name: &str,
        current_path: &Path,
        previous_path: &Path,
        journal_path: &Path,
        now_utc: &str,
    ) -> Result<InstalledPackage, UpdateError> {
        if current_path == previous_path {
            return Err(UpdateError::UnsafePath(
                "current and previous package paths are the same".to_string(),
            ));
        }
        if !current_path.is_file() {
            return Err(UpdateError::ArtifactNotFound {
                path: current_path.display().to_string(),
            });
        }
        let artifact = staged
            .manifest
            .artifacts
            .iter()
            .find(|artifact| artifact.name == artifact_name)
            .ok_or_else(|| UpdateError::ArtifactNotFound {
                path: artifact_name.to_string(),
            })?;
        validate_artifact_path(&artifact.artifact_path)?;
        let staged_path = staged.staged_dir.join(&artifact.artifact_path);
        if !staged_path.is_file() {
            return Err(UpdateError::ArtifactNotFound {
                path: staged_path.display().to_string(),
            });
        }

        copy_file_replacing(current_path, previous_path)?;
        if let Err(err) = copy_file_replacing(&staged_path, current_path) {
            return Err(err);
        }
        if let Err(err) = self.apply_update(staged, journal_path, now_utc) {
            let _ = copy_file_replacing(previous_path, current_path);
            return Err(err);
        }

        match launch_and_wait(current_path) {
            Ok(status) if status.success() => {
                let journal = read_journal(journal_path)?;
                Ok(InstalledPackage {
                    journal,
                    launch_exit_code: status.code(),
                    rolled_back: false,
                })
            }
            Ok(status) => {
                copy_file_replacing(previous_path, current_path)?;
                let journal = self.rollback(journal_path, now_utc)?;
                Ok(InstalledPackage {
                    journal,
                    launch_exit_code: status.code(),
                    rolled_back: true,
                })
            }
            Err(_err) => {
                copy_file_replacing(previous_path, current_path)?;
                let journal = self.rollback(journal_path, now_utc)?;
                Ok(InstalledPackage {
                    journal,
                    launch_exit_code: None,
                    rolled_back: true,
                })
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Version comparison
// ---------------------------------------------------------------------------

/// Parse `"major.minor.patch[-preview]"` into its numeric triple and preview flag.
fn parse_version(v: &str) -> Option<(u64, u64, u64, bool)> {
    let (base, is_preview) = if let Some(stripped) = v.strip_suffix("-preview") {
        (stripped, true)
    } else {
        (v, false)
    };

    let parts: Vec<&str> = base.splitn(3, '.').collect();
    if parts.len() != 3 {
        return None;
    }

    let major = parts[0].parse::<u64>().ok()?;
    let minor = parts[1].parse::<u64>().ok()?;
    let patch = parts[2].parse::<u64>().ok()?;
    Some((major, minor, patch, is_preview))
}

/// Returns `true` when `candidate` is strictly newer than `current`.
///
/// Rules:
/// * Numeric `major.minor.patch` comparison takes precedence.
/// * When the numeric triple is equal, a `-preview` suffix makes the version
///   *lower* than its non-preview counterpart: `0.1.0-preview` < `0.1.0`.
pub fn version_is_newer(candidate: &str, current: &str) -> bool {
    compare_versions(candidate, current) == Ordering::Greater
}

/// Full ordering for two Legion version strings (exposed for tests).
///
/// Falls back to lexicographic comparison for strings that don't match the
/// `major.minor.patch[-preview]` format.
pub fn compare_versions(a: &str, b: &str) -> Ordering {
    match (parse_version(a), parse_version(b)) {
        (Some((am, an, ap, ai)), Some((bm, bn, bp, bi))) => {
            let triple = am.cmp(&bm).then(an.cmp(&bn)).then(ap.cmp(&bp));
            match triple {
                Ordering::Equal => {
                    // preview < release when numeric parts are identical.
                    match (ai, bi) {
                        (true, false) => Ordering::Less,
                        (false, true) => Ordering::Greater,
                        _ => Ordering::Equal,
                    }
                }
                other => other,
            }
        }
        // Fallback for non-standard version strings.
        _ => a.cmp(b),
    }
}

// ---------------------------------------------------------------------------
// Ed25519 verify (duplicated from xtask/src/signing.rs)
// ---------------------------------------------------------------------------
//
// This is a verbatim copy of `verify_ed25519_signature` from
// `xtask/src/signing.rs`.  It is duplicated here because `xtask` cannot be a
// dependency of `legion-app` (architecture policy).  The function is pure code
// (~25 lines) with no xtask-specific state.

/// Verify an Ed25519 signature over `data`.
///
/// * `data` — the payload that was signed (raw manifest bytes).
/// * `signature` — 64 raw signature bytes.
/// * `verifying_key` — 32-byte compressed Ed25519 public key.
///
/// Returns `Ok(())` on a valid signature, or `Err(String)` describing the
/// failure (bad key length, bad signature length, or tamper detection).
pub fn verify_ed25519_signature(
    data: &[u8],
    signature: &[u8],
    verifying_key: &[u8],
) -> Result<(), String> {
    let key_bytes: &[u8; 32] = verifying_key.try_into().map_err(|_| {
        format!(
            "verifying key must be 32 bytes, got {}",
            verifying_key.len()
        )
    })?;
    let vk = ed25519_dalek::VerifyingKey::from_bytes(key_bytes).map_err(|err| err.to_string())?;

    let sig_bytes: &[u8; 64] = signature
        .try_into()
        .map_err(|_| format!("signature must be 64 bytes, got {}", signature.len()))?;
    let sig = ed25519_dalek::Signature::from_bytes(sig_bytes);

    vk.verify_strict(data, &sig).map_err(|err| err.to_string())
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

/// Write `journal` to `path` using a temp-then-rename strategy (Windows-safe).
fn write_journal(path: &Path, journal: &UpdateJournal) -> Result<(), UpdateError> {
    let wrapper = JournalFile {
        journal: journal.clone(),
    };
    let toml_text =
        toml::to_string_pretty(&wrapper).map_err(|e| UpdateError::Toml(e.to_string()))?;

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    // Write to a sibling temp file, then rename — atomic on Windows when
    // source and destination are on the same filesystem.
    let tmp = path.with_extension("toml.tmp");
    fs::write(&tmp, toml_text.as_bytes())?;
    fs::rename(&tmp, path)?;
    Ok(())
}

fn read_journal(path: &Path) -> Result<UpdateJournal, UpdateError> {
    let text = fs::read_to_string(path)?;
    let file: JournalFile =
        toml::from_str(&text).map_err(|err| UpdateError::Toml(err.to_string()))?;
    Ok(file.journal)
}

fn validate_manifest_url(url: &str) -> Result<(), UpdateError> {
    if url.contains('@') || url.contains('?') || url.contains('#') || url.contains(' ') {
        return Err(UpdateError::Feed(format!("refusing manifest URL `{url}`")));
    }
    if !url.ends_with(MANIFEST_FILE) {
        return Err(UpdateError::Feed(format!(
            "manifest URL must end with {MANIFEST_FILE}"
        )));
    }
    if url.starts_with("https://") {
        return Ok(());
    }
    if url.starts_with("http://") {
        let host = url
            .trim_start_matches("http://")
            .split('/')
            .next()
            .unwrap_or("");
        let host = host.rsplit_once('@').map(|(_, host)| host).unwrap_or(host);
        let host_only = host
            .strip_prefix('[')
            .and_then(|bracketed| bracketed.split(']').next())
            .unwrap_or_else(|| host.split(':').next().unwrap_or(host));
        if matches!(host_only, "127.0.0.1" | "localhost" | "::1") {
            return Ok(());
        }
        return Err(UpdateError::Feed(
            "cleartext update feeds are limited to loopback; production feeds must be https"
                .to_string(),
        ));
    }
    Err(UpdateError::Feed(format!("refusing manifest URL `{url}`")))
}

fn validate_artifact_path(path: &str) -> Result<(), UpdateError> {
    let invalid = path.is_empty()
        || path.contains('\0')
        || path.contains(':')
        || path.contains('\\')
        || path.starts_with('/')
        || path.starts_with('.');
    let path_buf = Path::new(path);
    let escapes = path_buf.is_absolute()
        || path_buf.components().any(|component| {
            matches!(
                component,
                std::path::Component::ParentDir
                    | std::path::Component::RootDir
                    | std::path::Component::Prefix(_)
            )
        });
    if invalid || escapes {
        return Err(UpdateError::UnsafePath(path.to_string()));
    }
    Ok(())
}

fn sibling_temp(path: &Path, tag: &str) -> PathBuf {
    let name = path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| "package".to_string());
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    parent.join(format!("{name}.{tag}.tmp"))
}

fn commit_rename(tmp: &Path, dest: &Path) -> Result<(), UpdateError> {
    if dest.exists() {
        let aside = sibling_temp(dest, "aside");
        if aside.exists() {
            fs::remove_file(&aside)?;
        }
        fs::rename(dest, &aside)?;
        if let Err(err) = fs::rename(tmp, dest) {
            let _ = fs::rename(&aside, dest);
            return Err(UpdateError::Io(err));
        }
        let _ = fs::remove_file(&aside);
        Ok(())
    } else {
        fs::rename(tmp, dest).map_err(UpdateError::Io)
    }
}

fn copy_file_replacing(from: &Path, to: &Path) -> Result<(), UpdateError> {
    if let Some(parent) = to.parent()
        && !parent.as_os_str().is_empty()
    {
        fs::create_dir_all(parent)?;
    }
    let tmp = sibling_temp(to, "incoming");
    fs::copy(from, &tmp)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = fs::metadata(from)?.permissions().mode() | 0o111;
        let mut perms = fs::metadata(&tmp)?.permissions();
        perms.set_mode(mode);
        fs::set_permissions(&tmp, perms)?;
    }
    commit_rename(&tmp, to)
}

fn launch_command(path: &Path) -> std::process::Command {
    let extension = path
        .extension()
        .and_then(|extension| extension.to_str())
        .unwrap_or("");
    if cfg!(windows)
        && (extension.eq_ignore_ascii_case("cmd") || extension.eq_ignore_ascii_case("bat"))
    {
        let mut command = std::process::Command::new("cmd");
        command.arg("/C").arg(path);
        command
    } else {
        std::process::Command::new(path)
    }
}

fn launch_and_wait(path: &Path) -> Result<std::process::ExitStatus, UpdateError> {
    launch_command(path)
        .status()
        .map_err(|err| UpdateError::Feed(format!("launch {}: {err}", path.display())))
}

/// Arguments for [`Updater::fetch_stage_swap_and_launch`].
#[cfg(feature = "updater-http")]
pub struct HttpInstallRequest<'a> {
    /// Product mode. Manual returns before any request.
    pub mode: crate::AppProductMode,
    /// Signed manifest feed.
    pub source: &'a HttpManifestSource,
    /// Channel and current version.
    pub policy: &'a UpdatePolicy,
    /// 32-byte Ed25519 verifying key. Tests generate this; production supplies it via config.
    pub verifying_key: &'a [u8],
    /// Directory for the downloaded and staged package bytes.
    pub work_dir: &'a Path,
    /// Manifest artifact name to swap into `current_path`.
    pub artifact_name: &'a str,
    /// Installed package path that will be replaced.
    pub current_path: &'a Path,
    /// N-1 slot. The pre-swap bytes are kept here for rollback.
    pub previous_path: &'a Path,
    /// Journal path written by the swap and toggled on rollback.
    pub journal_path: &'a Path,
    /// UTC timestamp recorded in the journal.
    pub now_utc: &'a str,
}

/// Outcome of a configured HTTP install attempt.
#[cfg(feature = "updater-http")]
#[derive(Debug)]
pub enum FetchInstallOutcome {
    /// Manual mode did not contact the feed.
    SkippedManual,
    /// The feed version is not strictly newer.
    NoUpdate,
    /// The package was swapped and launched, or launch failed and N-1 was restored.
    Installed(InstalledPackage),
}

#[cfg(feature = "updater-http")]
impl Updater {
    /// Fetch a signed manifest, reject downgrades, download and hash-check the
    /// package, stage it, swap it into place, launch it, and keep N-1.
    pub fn fetch_stage_swap_and_launch(
        &self,
        request: HttpInstallRequest<'_>,
    ) -> Result<FetchInstallOutcome, UpdateError> {
        if request.mode == crate::AppProductMode::Manual {
            return Ok(FetchInstallOutcome::SkippedManual);
        }
        let check =
            self.check_for_update(request.source, request.policy, Some(request.verifying_key))?;
        let UpdateCheck::Available {
            manifest,
            signer_status,
            previous_version,
        } = check
        else {
            return Ok(FetchInstallOutcome::NoUpdate);
        };
        if !manifest
            .artifacts
            .iter()
            .any(|artifact| artifact.name == request.artifact_name)
        {
            return Err(UpdateError::ArtifactNotFound {
                path: request.artifact_name.to_string(),
            });
        }
        let download_root = request.work_dir.join("download");
        fs::create_dir_all(&download_root)?;
        for artifact in &manifest.artifacts {
            validate_artifact_path(&artifact.artifact_path)?;
            request
                .source
                .download_artifact(&artifact.artifact_path, &download_root)?;
        }
        let staged = self.stage_update(
            *manifest,
            &download_root,
            signer_status,
            Some(previous_version),
        )?;
        let installed = self.swap_keep_previous_and_launch(
            &staged,
            request.artifact_name,
            request.current_path,
            request.previous_path,
            request.journal_path,
            request.now_utc,
        )?;
        Ok(FetchInstallOutcome::Installed(installed))
    }
}

/// HTTP feed for `release-manifest.v1.toml` and its detached signature.
///
/// Artifact URLs are the manifest directory plus the relative artifact path.
/// The signature URL is the manifest URL with `.sig` appended.
#[cfg(feature = "updater-http")]
pub struct HttpManifestSource {
    manifest_url: String,
    client: reqwest::blocking::Client,
}

#[cfg(feature = "updater-http")]
impl HttpManifestSource {
    /// Build a client for `manifest_url`.
    ///
    /// Cleartext HTTP is limited to loopback. Redirects are disabled.
    pub fn new(manifest_url: impl Into<String>) -> Result<Self, UpdateError> {
        Self::with_timeout(manifest_url, std::time::Duration::from_secs(30))
    }

    /// Same as [`Self::new`] with an explicit request timeout.
    pub fn with_timeout(
        manifest_url: impl Into<String>,
        timeout: std::time::Duration,
    ) -> Result<Self, UpdateError> {
        let manifest_url = manifest_url.into();
        validate_manifest_url(&manifest_url)?;
        install_ring_provider();
        let client = reqwest::blocking::Client::builder()
            .timeout(timeout)
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .map_err(|err| UpdateError::Feed(format!("failed to build update client: {err}")))?;
        Ok(Self {
            manifest_url,
            client,
        })
    }

    /// Download one relative artifact into `dest_root`.
    pub fn download_artifact(
        &self,
        artifact_path: &str,
        dest_root: &Path,
    ) -> Result<PathBuf, UpdateError> {
        validate_artifact_path(artifact_path)?;
        let url = artifact_url(&self.manifest_url, artifact_path)?;
        let dest = dest_root.join(artifact_path);
        if let Some(parent) = dest.parent() {
            fs::create_dir_all(parent)?;
        }
        let tmp = sibling_temp(&dest, "download");
        self.download_limited(&url, &tmp)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = fs::metadata(&tmp)?.permissions();
            perms.set_mode(0o755);
            fs::set_permissions(&tmp, perms)?;
        }
        commit_rename(&tmp, &dest)?;
        Ok(dest)
    }

    fn download_limited(&self, url: &str, dest: &Path) -> Result<(), UpdateError> {
        use std::io::{Read, Write};
        let mut response = self
            .client
            .get(url)
            .send()
            .map_err(|err| UpdateError::Feed(format!("{url}: {err}")))?;
        let status = response.status();
        if !status.is_success() {
            return Err(UpdateError::Feed(format!("{url} returned {status}")));
        }
        if let Some(len) = response.content_length()
            && len > MAX_FEED_OBJECT_BYTES
        {
            return Err(UpdateError::Feed(format!(
                "{url} exceeds {MAX_FEED_OBJECT_BYTES} bytes"
            )));
        }
        let mut file = fs::File::create(dest)?;
        let mut buf = [0u8; 8192];
        let mut total = 0u64;
        loop {
            let read = response
                .read(&mut buf)
                .map_err(|err| UpdateError::Feed(format!("{url}: {err}")))?;
            if read == 0 {
                break;
            }
            total += read as u64;
            if total > MAX_FEED_OBJECT_BYTES {
                return Err(UpdateError::Feed(format!(
                    "{url} exceeds {MAX_FEED_OBJECT_BYTES} bytes"
                )));
            }
            file.write_all(&buf[..read])?;
        }
        Ok(())
    }

    fn fetch_required(&self, url: &str) -> Result<Vec<u8>, UpdateError> {
        self.fetch_optional(url)?
            .ok_or_else(|| UpdateError::Feed(format!("{url} returned 404")))
    }

    fn fetch_optional(&self, url: &str) -> Result<Option<Vec<u8>>, UpdateError> {
        let response = self
            .client
            .get(url)
            .send()
            .map_err(|err| UpdateError::Feed(format!("{url}: {err}")))?;
        if response.status().as_u16() == 404 {
            return Ok(None);
        }
        if !response.status().is_success() {
            return Err(UpdateError::Feed(format!(
                "{url} returned {}",
                response.status()
            )));
        }
        let bytes = response
            .bytes()
            .map_err(|err| UpdateError::Feed(format!("{url}: {err}")))?;
        if bytes.len() as u64 > MAX_FEED_OBJECT_BYTES {
            return Err(UpdateError::Feed(format!(
                "{url} exceeds {MAX_FEED_OBJECT_BYTES} bytes"
            )));
        }
        Ok(Some(bytes.to_vec()))
    }
}

#[cfg(feature = "updater-http")]
impl ManifestSource for HttpManifestSource {
    fn fetch_manifest(&self) -> Result<(Vec<u8>, Option<Vec<u8>>), UpdateError> {
        let manifest_url = &self.manifest_url;
        let manifest = self.fetch_required(manifest_url)?;
        let signature = self.fetch_optional(&format!("{manifest_url}.sig"))?;
        Ok((manifest, signature))
    }
}

#[cfg(feature = "updater-http")]
fn install_ring_provider() {
    use std::sync::Once;
    static INIT: Once = Once::new();
    INIT.call_once(|| {
        let _ = rustls::crypto::ring::default_provider().install_default();
    });
}

#[cfg(feature = "updater-http")]
fn artifact_url(manifest_url: &str, artifact_path: &str) -> Result<String, UpdateError> {
    validate_artifact_path(artifact_path)?;
    let (dir, _) = manifest_url
        .rsplit_once('/')
        .ok_or_else(|| UpdateError::Feed("manifest URL has no path".to_string()))?;
    Ok(format!("{dir}/{artifact_path}"))
}
