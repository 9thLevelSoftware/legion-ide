//! Integration tests for the Legion updater module (PKT-UPDATER / ADR-0042).
//!
//! # TDD evidence
//!
//! Version-compare tests were written first (before the implementation) and
//! verified red before the `compare_versions` / `version_is_newer` functions
//! were implemented. The remaining tests follow the same pinned-first pattern.
//!
//! # Note on filename
//!
//! This file is named `upd_tests.rs` (not `updater_tests.rs`) because Windows'
//! installer-detection heuristic auto-elevates any executable whose name contains
//! the substring "update" (case-insensitive), including Rust test binaries.
//! Renaming avoids the `ERROR_ELEVATION_REQUIRED` (os error 740) on Windows.

use std::{
    fs,
    hash::{Hash, Hasher},
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use legion_app::updater::{
    compare_versions, version_is_newer, LocalDirManifestSource, UpdateError,
    UpdateJournal, UpdatePolicy, Updater, verify_ed25519_signature,
};
use legion_protocol::{ReleaseArtifact, ReleaseManifestV1};

// ─────────────────────────────────────────────────────────────────────────────
// Helpers
// ─────────────────────────────────────────────────────────────────────────────

fn thread_id_u64() -> u64 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    std::thread::current().id().hash(&mut h);
    h.finish()
}

fn temp_dir(tag: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .subsec_nanos();
    std::env::temp_dir().join(format!(
        "legion_upd_test_{}_{}_{}_{}",
        std::process::id(),
        nanos,
        tag,
        // Additional entropy so parallel tests don't collide.
        thread_id_u64()
    ))
}

/// Compute SHA-256 hex digest of bytes.
fn sha256_hex(data: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    hex::encode(Sha256::digest(data))
}

/// Build a minimal valid `ReleaseManifestV1` using provided artifact bytes.
fn make_manifest(
    channel: &str,
    version: &str,
    artifact_name: &str,
    artifact_bytes: &[u8],
) -> ReleaseManifestV1 {
    let artifact = ReleaseArtifact::new(
        artifact_name.to_string(),
        "test-platform".to_string(),
        "x86_64-test".to_string(),
        format!("{artifact_name}.bin"),
        sha256_hex(artifact_bytes),
    );
    ReleaseManifestV1::new(
        "legion-test".to_string(),
        channel.to_string(),
        version.to_string(),
        None,
        None,
        vec![artifact],
        "2026-07-07T00:00:00Z".to_string(),
        None,
    )
}

/// Generate an ephemeral Ed25519 keypair from a time-based seed.
/// Returns `(signing_key_bytes_32, verifying_key_bytes_32)`.
fn ephemeral_keypair() -> ([u8; 32], Vec<u8>) {
    let seed = make_seed();
    let sk = ed25519_dalek::SigningKey::from_bytes(&seed);
    let vk = sk.verifying_key().to_bytes().to_vec();
    (seed, vk)
}

fn make_seed() -> [u8; 32] {
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    let secs = ts.as_secs();
    let nanos = ts.subsec_nanos() as u64;
    let pid = std::process::id() as u64;
    let tid = thread_id_u64();
    let mut seed = [0u8; 32];
    for i in 0..8usize {
        seed[i] = ((secs >> (i * 8)) & 0xff) as u8;
        seed[8 + i] = ((nanos >> (i * 4)) & 0xff) as u8;
        seed[16 + i] = ((pid >> (i * 8)) & 0xff) as u8;
        seed[24 + i] = ((tid >> (i * 8)) & 0xff) as u8;
    }
    seed
}

/// Sign `data` with the 32-byte seed key; returns 64 raw signature bytes.
fn sign_bytes(seed: &[u8; 32], data: &[u8]) -> Vec<u8> {
    use ed25519_dalek::Signer as _;
    let sk = ed25519_dalek::SigningKey::from_bytes(seed);
    sk.sign(data).to_bytes().to_vec()
}

/// Create a temp dir, write a manifest TOML and optional sig file; return the dir path.
fn write_manifest_dir(
    manifest: &ReleaseManifestV1,
    sig: Option<&[u8]>,
    artifacts: &[(&str, &[u8])],
) -> PathBuf {
    let dir = temp_dir("manifest");
    fs::create_dir_all(&dir).unwrap();

    let manifest_toml = toml::to_string_pretty(manifest).unwrap();
    fs::write(dir.join("release-manifest.v1.toml"), manifest_toml.as_bytes()).unwrap();

    if let Some(sig_bytes) = sig {
        fs::write(dir.join("release-manifest.v1.toml.sig"), sig_bytes).unwrap();
    }

    for (filename, bytes) in artifacts {
        fs::write(dir.join(filename), bytes).unwrap();
    }

    dir
}

/// Current UTC timestamp string (minimal RFC 3339 approximation for tests).
fn now_utc() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    format!("{secs}")
}

// ─────────────────────────────────────────────────────────────────────────────
// Version compare tests (PINNED FIRST — TDD red → green evidence)
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn version_compare_basic_minor_bump() {
    assert_eq!(compare_versions("0.2.0", "0.1.0"), std::cmp::Ordering::Greater);
    assert!(version_is_newer("0.2.0", "0.1.0"));
}

#[test]
fn version_compare_patch_bump() {
    assert_eq!(compare_versions("0.1.1", "0.1.0"), std::cmp::Ordering::Greater);
    assert!(version_is_newer("0.1.1", "0.1.0"));
}

#[test]
fn version_compare_major_wins_over_minor() {
    // 1.0.0 > 0.99.99 — major takes precedence.
    assert_eq!(compare_versions("1.0.0", "0.99.99"), std::cmp::Ordering::Greater);
    assert!(version_is_newer("1.0.0", "0.99.99"));
}

#[test]
fn version_compare_preview_is_lower_than_release() {
    // 0.1.0-preview < 0.1.0 — preview suffix makes version lower.
    assert_eq!(compare_versions("0.1.0-preview", "0.1.0"), std::cmp::Ordering::Less);
    assert!(!version_is_newer("0.1.0-preview", "0.1.0"));
}

#[test]
fn version_compare_preview_vs_preview() {
    // 0.1.0-preview < 0.1.1-preview — numeric part dominates.
    assert_eq!(
        compare_versions("0.1.0-preview", "0.1.1-preview"),
        std::cmp::Ordering::Less
    );
    assert!(!version_is_newer("0.1.0-preview", "0.1.1-preview"));
    assert!(version_is_newer("0.1.1-preview", "0.1.0-preview"));
}

#[test]
fn version_compare_equal() {
    assert_eq!(compare_versions("0.1.0", "0.1.0"), std::cmp::Ordering::Equal);
    assert!(!version_is_newer("0.1.0", "0.1.0"));
}

#[test]
fn version_compare_equal_preview_vs_preview() {
    assert_eq!(
        compare_versions("0.2.0-preview", "0.2.0-preview"),
        std::cmp::Ordering::Equal
    );
    assert!(!version_is_newer("0.2.0-preview", "0.2.0-preview"));
}

#[test]
fn version_compare_release_is_higher_than_preview_same_version() {
    assert_eq!(compare_versions("0.1.0", "0.1.0-preview"), std::cmp::Ordering::Greater);
    assert!(version_is_newer("0.1.0", "0.1.0-preview"));
}

// ─────────────────────────────────────────────────────────────────────────────
// Signature verification tests
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn verify_ed25519_tampered_bytes_rejected_before_parse() {
    let (seed, vk) = ephemeral_keypair();
    let artifact_bytes = b"fake artifact v0.2.0";
    let manifest = make_manifest("stable", "0.2.0", "legion-test", artifact_bytes);
    let manifest_toml = toml::to_string_pretty(&manifest).unwrap();
    let manifest_bytes = manifest_toml.as_bytes();

    // Sign the original manifest.
    let sig = sign_bytes(&seed, manifest_bytes);

    // Tamper with the manifest bytes.
    let mut tampered = manifest_bytes.to_vec();
    tampered[0] ^= 0xff;

    // Verification must fail on the tampered bytes.
    let result = verify_ed25519_signature(&tampered, &sig, &vk);
    assert!(result.is_err(), "tampered manifest should fail verification");
}

#[test]
fn check_for_upd_bad_sig_rejected() {
    let (seed, vk) = ephemeral_keypair();
    let artifact_bytes = b"artifact content";
    let manifest = make_manifest("stable", "0.2.0", "legion-test", artifact_bytes);
    let manifest_toml = toml::to_string_pretty(&manifest).unwrap();
    let manifest_bytes = manifest_toml.as_bytes().to_vec();

    // Sign with the correct key, then flip a byte in the signature.
    let mut bad_sig = sign_bytes(&seed, &manifest_bytes);
    bad_sig[0] ^= 0xff;

    let dir = write_manifest_dir(&manifest, Some(&bad_sig), &[("legion-test.bin", artifact_bytes)]);

    let source = LocalDirManifestSource::new(&dir);
    let policy = UpdatePolicy {
        current_version: "0.1.0".to_string(),
        current_channel: "stable".to_string(),
        allow_unsigned_beta: false,
    };

    let result = Updater::new().check_for_update(&source, &policy, Some(&vk));
    assert!(
        matches!(result, Err(UpdateError::SignatureInvalid(_))),
        "expected SignatureInvalid, got {result:?}"
    );
    let _ = fs::remove_dir_all(&dir);
}

// ─────────────────────────────────────────────────────────────────────────────
// Unsigned-beta policy gate
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn unsigned_manifest_rejected_when_policy_disallows() {
    let artifact_bytes = b"artifact";
    let manifest = make_manifest("stable", "0.2.0", "legion-test", artifact_bytes);
    // No sig file.
    let dir = write_manifest_dir(&manifest, None, &[("legion-test.bin", artifact_bytes)]);

    let source = LocalDirManifestSource::new(&dir);
    let policy = UpdatePolicy {
        current_version: "0.1.0".to_string(),
        current_channel: "stable".to_string(),
        allow_unsigned_beta: false,
    };

    let result = Updater::new().check_for_update(&source, &policy, None);
    assert!(
        matches!(result, Err(UpdateError::UnsignedNotAllowed)),
        "expected UnsignedNotAllowed, got {result:?}"
    );
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn unsigned_manifest_accepted_when_policy_allows_records_unsigned_beta() {
    let artifact_bytes = b"artifact";
    let manifest = make_manifest("stable", "0.2.0", "legion-test", artifact_bytes);
    let dir = write_manifest_dir(&manifest, None, &[("legion-test.bin", artifact_bytes)]);

    let source = LocalDirManifestSource::new(&dir);
    let policy = UpdatePolicy {
        current_version: "0.1.0".to_string(),
        current_channel: "stable".to_string(),
        allow_unsigned_beta: true,
    };

    let result = Updater::new().check_for_update(&source, &policy, None);
    match result {
        Ok(legion_app::updater::UpdateCheck::Available { signer_status, .. }) => {
            assert_eq!(signer_status, "unsigned-beta");
        }
        other => panic!("expected Available with unsigned-beta, got {other:?}"),
    }
    let _ = fs::remove_dir_all(&dir);
}

// ─────────────────────────────────────────────────────────────────────────────
// Channel mismatch
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn check_for_upd_channel_mismatch_rejected() {
    let artifact_bytes = b"artifact";
    // Manifest is "preview" but policy channel is "stable".
    let manifest = make_manifest("preview", "0.2.0", "legion-test", artifact_bytes);
    let dir = write_manifest_dir(&manifest, None, &[("legion-test.bin", artifact_bytes)]);

    let source = LocalDirManifestSource::new(&dir);
    let policy = UpdatePolicy {
        current_version: "0.1.0".to_string(),
        current_channel: "stable".to_string(),
        allow_unsigned_beta: true, // skip sig check for this test
    };

    let result = Updater::new().check_for_update(&source, &policy, None);
    assert!(
        matches!(
            result,
            Err(UpdateError::ChannelMismatch {
                manifest: ref m,
                policy: ref p,
            }) if m == "preview" && p == "stable"
        ),
        "expected ChannelMismatch, got {result:?}"
    );
    let _ = fs::remove_dir_all(&dir);
}

// ─────────────────────────────────────────────────────────────────────────────
// Hash mismatch during staging
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn stage_rejects_artifact_with_wrong_hash() {
    let artifact_bytes = b"correct content";
    let manifest = make_manifest("stable", "0.2.0", "legion-test", artifact_bytes);
    // Write a DIFFERENT file but the manifest still contains the original hash.
    let wrong_bytes = b"WRONG content";
    let dir = write_manifest_dir(&manifest, None, &[("legion-test.bin", wrong_bytes)]);

    let result = Updater::new().stage_update(
        manifest,
        &dir,
        "unsigned-beta".to_string(),
        None,
    );
    assert!(
        matches!(result, Err(UpdateError::HashMismatch { .. })),
        "expected HashMismatch, got {result:?}"
    );
    let _ = fs::remove_dir_all(&dir);
}

// ─────────────────────────────────────────────────────────────────────────────
// Full happy path: check → stage → apply → rollback (journal state machine)
// ─────────────────────────────────────────────────────────────────────────────

fn run_full_pipeline(dir: &Path) -> (UpdateJournal, UpdateJournal, UpdateJournal) {
    let artifact_bytes = b"v0.2.0 artifact content";
    let manifest = make_manifest("stable", "0.2.0", "legion-test", artifact_bytes);

    // Write manifest (unsigned) and artifact.
    let manifest_toml = toml::to_string_pretty(&manifest).unwrap();
    fs::write(dir.join("release-manifest.v1.toml"), manifest_toml.as_bytes()).unwrap();
    fs::write(dir.join("legion-test.bin"), artifact_bytes).unwrap();

    let updater = Updater::new();
    let source = LocalDirManifestSource::new(dir);
    let policy = UpdatePolicy {
        current_version: "0.1.0".to_string(),
        current_channel: "stable".to_string(),
        allow_unsigned_beta: true,
    };

    let check = updater.check_for_update(&source, &policy, None).unwrap();
    let (manifest_from_check, signer_status, previous_version) = match check {
        legion_app::updater::UpdateCheck::Available { manifest, signer_status, previous_version } => {
            (manifest, signer_status, previous_version)
        }
        legion_app::updater::UpdateCheck::NoUpdate => {
            panic!("expected Available but got NoUpdate");
        }
    };

    let staged = updater
        .stage_update(manifest_from_check, dir, signer_status, Some(previous_version))
        .unwrap();

    let journal_path = dir.join("upd_journal.toml");
    let now = now_utc();
    let applied = updater.apply_update(&staged, &journal_path, &now).unwrap();
    let rolled = updater.rollback(&journal_path, &now).unwrap();
    let double_rolled = updater.rollback(&journal_path, &now).unwrap();

    (applied, rolled, double_rolled)
}

#[test]
fn apply_journal_shows_new_as_current_old_as_previous() {
    let dir = temp_dir("pipeline");
    fs::create_dir_all(&dir).unwrap();
    let (applied, _, _) = run_full_pipeline(&dir);

    assert_eq!(applied.current_version, "0.2.0");
    assert_eq!(applied.previous_version.as_deref(), Some("0.1.0"));
    assert_eq!(applied.channel, "stable");
    assert_eq!(applied.signer_status, "unsigned-beta");

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn rollback_swaps_current_and_previous() {
    let dir = temp_dir("rollback");
    fs::create_dir_all(&dir).unwrap();
    let (_, rolled, _) = run_full_pipeline(&dir);

    assert_eq!(rolled.current_version, "0.1.0");
    assert_eq!(rolled.previous_version.as_deref(), Some("0.2.0"));

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn double_rollback_is_idempotent_toggle_back_to_applied_state() {
    let dir = temp_dir("double_rollback");
    fs::create_dir_all(&dir).unwrap();
    let (applied, _, double_rolled) = run_full_pipeline(&dir);

    // After two rollbacks the journal should be back to the apply state.
    assert_eq!(double_rolled.current_version, applied.current_version);
    assert_eq!(double_rolled.previous_version, applied.previous_version);

    let _ = fs::remove_dir_all(&dir);
}

// ─────────────────────────────────────────────────────────────────────────────
// Downgrade rejected (NoUpdate)
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn check_for_upd_downgrade_returns_no_upd() {
    let artifact_bytes = b"old artifact";
    // Manifest has v0.1.0 but we're "running" v0.2.0 → no update.
    let manifest = make_manifest("stable", "0.1.0", "legion-test", artifact_bytes);
    let dir = write_manifest_dir(&manifest, None, &[("legion-test.bin", artifact_bytes)]);

    let source = LocalDirManifestSource::new(&dir);
    let policy = UpdatePolicy {
        current_version: "0.2.0".to_string(),
        current_channel: "stable".to_string(),
        allow_unsigned_beta: true,
    };

    let result = Updater::new().check_for_update(&source, &policy, None).unwrap();
    assert!(
        matches!(result, legion_app::updater::UpdateCheck::NoUpdate),
        "expected NoUpdate for downgrade, got {result:?}"
    );
    let _ = fs::remove_dir_all(&dir);
}

// ─────────────────────────────────────────────────────────────────────────────
// Signed happy path (end-to-end sig verify)
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn signed_manifest_accepted_and_journal_records_signed_ed25519() {
    let (seed, vk) = ephemeral_keypair();
    let artifact_bytes = b"signed artifact";
    let manifest = make_manifest("stable", "0.3.0", "legion-test", artifact_bytes);
    let manifest_toml = toml::to_string_pretty(&manifest).unwrap();
    let sig = sign_bytes(&seed, manifest_toml.as_bytes());

    let dir = write_manifest_dir(
        &manifest,
        Some(&sig),
        &[("legion-test.bin", artifact_bytes)],
    );

    let source = LocalDirManifestSource::new(&dir);
    let policy = UpdatePolicy {
        current_version: "0.1.0".to_string(),
        current_channel: "stable".to_string(),
        allow_unsigned_beta: false,
    };

    let check = Updater::new()
        .check_for_update(&source, &policy, Some(&vk))
        .unwrap();

    match check {
        legion_app::updater::UpdateCheck::Available { signer_status, .. } => {
            assert_eq!(signer_status, "signed/ed25519");
        }
        other => panic!("expected Available, got {other:?}"),
    }
    let _ = fs::remove_dir_all(&dir);
}
