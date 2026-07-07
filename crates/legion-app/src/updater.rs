//! Auto-updater client for Legion IDE (ADR-0042).
//!
//! # HTTP manifest source deferral
//!
//! Only [`LocalDirManifestSource`] is implemented. An HTTP-backed manifest
//! source is explicitly deferred — no update server currently exists, and
//! implementing one would require a separate ADR and an egress policy review.
//! The [`ManifestSource`] trait is the stable extension point when that work
//! lands.
//!
//! # Security invariants
//!
//! * Ed25519 signature verification runs **BEFORE** TOML parsing (fail-closed).
//!   A tampered manifest cannot reach the parser.
//! * Unsigned manifests are rejected unless
//!   [`UpdatePolicy::allow_unsigned_beta`] is `true`.
//! * Key material is never logged or persisted.
//! * File operations use copy-then-rename (never in-place overwrite) for
//!   Windows safety.

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
}

// ---------------------------------------------------------------------------
// ManifestSource trait
// ---------------------------------------------------------------------------

/// A source of raw manifest bytes and an optional detached Ed25519 signature.
///
/// The single concrete implementation is [`LocalDirManifestSource`]. An HTTP
/// implementation is explicitly deferred (see module-level docs).
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
/// **This is the only [`ManifestSource`] implementation.** HTTP-based manifest
/// fetch is explicitly deferred to a future ADR.
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
        manifest: ReleaseManifestV1,
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
            manifest,
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
    /// **Binary swap and process restart are explicitly out of scope** (per
    /// ADR-0042 D5). This method records intent; the OS installer or a future
    /// restart-manager packet completes the swap.
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
