//! TDD signing tests for PKT-SIGN (ADR-0042).
//!
//! # Security invariants enforced in tests
//!
//! * Test keys are ephemeral (derived from a fixed seed); they are NEVER
//!   written to disk, logged, or persisted.
//! * No env var holding key material is left set after a test completes.
//! * The `env_resolver_roundtrip` test removes its env var immediately after
//!   the signer is resolved, before exercising sign/verify — the material
//!   does not stay in the environment.

use xtask::signing::{
    DalekSigner, Signer, SignerResolution, SigningConfig, resolve_signer,
    verify_ed25519_signature,
};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Fixed test seed — deterministic, ephemeral, never persisted.
/// Different from all-zeros to exercise real key derivation.
fn test_seed() -> [u8; 32] {
    let mut seed = [0u8; 32];
    for (i, b) in seed.iter_mut().enumerate() {
        *b = (i as u8).wrapping_add(0x5a).wrapping_mul(0x9e);
    }
    seed
}

/// Alternative seed for cross-key tamper tests.
fn alt_seed() -> [u8; 32] {
    let mut seed = [0u8; 32];
    for (i, b) in seed.iter_mut().enumerate() {
        *b = (i as u8).wrapping_add(0x1f).wrapping_mul(0x7d);
    }
    seed
}

// ---------------------------------------------------------------------------
// Core crypto tests
// ---------------------------------------------------------------------------

/// An ephemeral key can sign data and verify the signature with the same key.
#[test]
fn signing_keypair_roundtrip() {
    let seed = test_seed();
    let signer = DalekSigner::from_seed(&seed);
    let vk = signer.verifying_key_bytes();

    let data = b"legion-ide release manifest roundtrip test payload";
    let sig = signer.sign_bytes(data).expect("sign should succeed");

    assert_eq!(sig.len(), 64, "Ed25519 signature must be 64 bytes");
    assert_eq!(vk.len(), 32, "Ed25519 verifying key must be 32 bytes");

    verify_ed25519_signature(data, &sig, &vk)
        .expect("verify should succeed for a valid signature");
}

/// Changing the manifest body (data) makes verification fail.
#[test]
fn tampered_manifest_fails_verification() {
    let seed = test_seed();
    let signer = DalekSigner::from_seed(&seed);
    let vk = signer.verifying_key_bytes();

    let original = b"schema_version = 1\npackage_name = \"legion-desktop\"";
    let sig = signer.sign_bytes(original).expect("sign");

    let tampered = b"schema_version = 1\npackage_name = \"evil-desktop\"";
    let result = verify_ed25519_signature(tampered, &sig, &vk);
    assert!(
        result.is_err(),
        "tampered manifest body must fail verification"
    );
}

/// Flipping a bit in the signature makes verification fail (closed-form tamper).
#[test]
fn tampered_signature_fails_verification() {
    let seed = test_seed();
    let signer = DalekSigner::from_seed(&seed);
    let vk = signer.verifying_key_bytes();

    let data = b"authentic manifest content";
    let mut sig = signer.sign_bytes(data).expect("sign");

    // Flip one bit in the signature.
    sig[0] ^= 0x01;

    let result = verify_ed25519_signature(data, &sig, &vk);
    assert!(
        result.is_err(),
        "tampered signature must fail verification"
    );
}

/// Modifying an artifact sha256 in the manifest payload makes verification fail.
#[test]
fn tampered_artifact_hash_fails_verification() {
    let seed = test_seed();
    let signer = DalekSigner::from_seed(&seed);
    let vk = signer.verifying_key_bytes();

    let manifest_data =
        b"schema_version = 1\nsha256 = \"aabbccdd1122334455667788\"\npackage_name = \"legion-desktop\"";
    let sig = signer.sign_bytes(manifest_data).expect("sign original manifest");

    // Tamper: change the sha256 field value.
    let tampered =
        b"schema_version = 1\nsha256 = \"ffffffff0000000000000000\"\npackage_name = \"legion-desktop\"";
    let result = verify_ed25519_signature(tampered, &sig, &vk);
    assert!(
        result.is_err(),
        "manifest with tampered artifact hash must fail verification"
    );
}

/// A signature produced with one key must not verify under a different key.
#[test]
fn wrong_verifying_key_fails_verification() {
    let seed_a = test_seed();
    let signer_a = DalekSigner::from_seed(&seed_a);

    let seed_b = alt_seed();
    let signer_b = DalekSigner::from_seed(&seed_b);
    let vk_b = signer_b.verifying_key_bytes();

    let data = b"cross-key tamper detection test";
    let sig_a = signer_a.sign_bytes(data).expect("sign with key A");

    let result = verify_ed25519_signature(data, &sig_a, &vk_b);
    assert!(
        result.is_err(),
        "signature from key A must not verify under key B"
    );
}

// ---------------------------------------------------------------------------
// Resolver tests
// ---------------------------------------------------------------------------

/// When no env var is set, the env resolver returns Unavailable.
#[test]
fn unsigned_beta_status_when_env_signer_unavailable() {
    let config = SigningConfig {
        source: "env".to_string(),
        reference: "LEGION_SIGNING_KEY_PKT_SIGN_TEST_ABSENT_ABCDEFGHIJKLM".to_string(),
        identity: "test-identity".to_string(),
    };

    match resolve_signer(&config) {
        SignerResolution::Unavailable { reason } => {
            assert!(
                !reason.is_empty(),
                "unavailable reason should be non-empty"
            );
        }
        SignerResolution::Available(_) => {
            panic!("env resolver should be Unavailable when env var is not set");
        }
    }
}

/// Set an env var with a valid base64 Ed25519 seed, resolve the signer, then
/// sign and verify.  The env var is removed before the sign/verify step.
#[test]
fn env_resolver_roundtrip() {
    use base64::Engine as _;

    let seed = test_seed();
    let encoded = base64::engine::general_purpose::STANDARD.encode(seed);

    // Use a uniquely-named var to avoid interference with other tests.
    let var_name = "LEGION_PKT_SIGN_TEST_ROUNDTRIP_EPHEMERAL_KEY";
    // Safety: this is a test-only env var carrying a test-only seed.
    // The var is removed immediately after the signer is resolved.
    unsafe { std::env::set_var(var_name, &encoded) };

    let config = SigningConfig {
        source: "env".to_string(),
        reference: var_name.to_string(),
        identity: "test".to_string(),
    };

    let signer = match resolve_signer(&config) {
        SignerResolution::Available(s) => s,
        SignerResolution::Unavailable { reason } => {
            // Remove env var before panicking.
            unsafe { std::env::remove_var(var_name) };
            panic!("expected env resolver to be Available: {reason}");
        }
    };

    // Remove the env var immediately — key material must not linger.
    unsafe { std::env::remove_var(var_name) };

    let data = b"env resolver roundtrip test payload for PKT-SIGN";
    let sig = signer.sign_bytes(data).expect("sign");
    let vk = signer.verifying_key_bytes();

    verify_ed25519_signature(data, &sig, &vk).expect("verify roundtrip after env var removed");
}

/// ci-secret source resolves the same way as env.
#[test]
fn ci_secret_resolver_is_same_as_env() {
    let config = SigningConfig {
        source: "ci-secret".to_string(),
        reference: "LEGION_SIGNING_KEY_PKT_SIGN_TEST_CI_SECRET_ABSENT".to_string(),
        identity: "ci-identity".to_string(),
    };

    match resolve_signer(&config) {
        SignerResolution::Unavailable { reason } => {
            assert!(!reason.is_empty());
        }
        SignerResolution::Available(_) => {
            panic!("ci-secret resolver should be Unavailable when env var is absent");
        }
    }
}

/// The KMS resolver always returns a clearly-labelled Unavailable.
#[test]
fn kms_resolver_returns_honest_unavailable() {
    let config = SigningConfig {
        source: "kms".to_string(),
        reference: "arn:aws:kms:us-east-1:000000000000:key/test-key".to_string(),
        identity: "test".to_string(),
    };

    match resolve_signer(&config) {
        SignerResolution::Unavailable { reason } => {
            assert!(
                reason.contains("not yet implemented"),
                "KMS should report honest unavailable, got: {reason}"
            );
        }
        SignerResolution::Available(_) => {
            panic!("KMS resolver must always return Unavailable");
        }
    }
}

/// An unknown source returns a clearly-labelled Unavailable (not a panic).
#[test]
fn unknown_source_returns_unavailable() {
    let config = SigningConfig {
        source: "hsm".to_string(),
        reference: "slot-0".to_string(),
        identity: "test".to_string(),
    };

    match resolve_signer(&config) {
        SignerResolution::Unavailable { reason } => {
            assert!(
                reason.contains("unknown signing source"),
                "unknown source should report unknown-source error, got: {reason}"
            );
        }
        SignerResolution::Available(_) => {
            panic!("unknown source must return Unavailable");
        }
    }
}

/// The keyring resolver either returns a signer (if the OS keyring has the
/// entry) or a visible Unavailable.  It must never panic — this test exercises
/// the non-panic invariant in any environment including CI.
#[test]
fn keyring_resolver_visible_skip_if_unavailable() {
    let config = SigningConfig {
        source: "keyring".to_string(),
        reference: "legion-pkt-sign-test-not-expected-to-exist-in-ci-abcxyz".to_string(),
        identity: "test-identity-pkt-sign".to_string(),
    };

    // Must not panic — either outcome is acceptable.
    match resolve_signer(&config) {
        SignerResolution::Unavailable { reason } => {
            // Expected outcome in CI environments without the keyring entry.
            assert!(!reason.is_empty(), "unavailable reason should be non-empty");
        }
        SignerResolution::Available(signer) => {
            // Acceptable if the test machine happens to have this entry.
            // Verify the signer actually works.
            let data = b"keyring signer smoke test";
            let sig = signer.sign_bytes(data).expect("keyring signer sign");
            let vk = signer.verifying_key_bytes();
            verify_ed25519_signature(data, &sig, &vk).expect("keyring signer verify");
        }
    }
}

/// An empty source string returns Unavailable with a clear "not configured" reason.
#[test]
fn empty_source_returns_unavailable_not_configured() {
    let config = SigningConfig {
        source: String::new(),
        reference: String::new(),
        identity: String::new(),
    };

    match resolve_signer(&config) {
        SignerResolution::Unavailable { reason } => {
            assert!(
                reason.contains("no signing source configured"),
                "empty source should report not-configured, got: {reason}"
            );
        }
        SignerResolution::Available(_) => {
            panic!("empty source must return Unavailable");
        }
    }
}

/// A base64 string that decodes to the wrong number of bytes (not 32) is rejected.
#[test]
fn env_resolver_rejects_wrong_length_seed() {
    use base64::Engine as _;

    // 16 bytes — too short for an Ed25519 seed.
    let short_seed = [0xabu8; 16];
    let encoded = base64::engine::general_purpose::STANDARD.encode(short_seed);

    let var_name = "LEGION_PKT_SIGN_TEST_SHORT_SEED_EPHEMERAL";
    unsafe { std::env::set_var(var_name, &encoded) };

    let config = SigningConfig {
        source: "env".to_string(),
        reference: var_name.to_string(),
        identity: "test".to_string(),
    };

    let result = resolve_signer(&config);
    unsafe { std::env::remove_var(var_name) };

    match result {
        SignerResolution::Unavailable { reason } => {
            assert!(
                reason.contains("16") || reason.contains("bytes") || reason.contains("seed"),
                "should explain wrong key length, got: {reason}"
            );
        }
        SignerResolution::Available(_) => {
            panic!("16-byte seed should be rejected");
        }
    }
}

// ---------------------------------------------------------------------------
// legion-protocol DTO tests
// ---------------------------------------------------------------------------

/// ReleaseManifestV1::validate() passes for a well-formed manifest.
#[test]
fn release_manifest_v1_validates_well_formed() {
    use legion_protocol::{ReleaseArtifact, ReleaseManifestV1};

    let manifest = ReleaseManifestV1::new(
        "legion-desktop".to_string(),
        "stable".to_string(),
        "0.1.0".to_string(),
        Some("full".to_string()),
        None,
        vec![ReleaseArtifact::new(
            "legion-desktop-windows-x64-msi".to_string(),
            "windows".to_string(),
            "x86_64-pc-windows-msvc".to_string(),
            "legion-desktop-windows-x64-msi.msi".to_string(),
            "a".repeat(64),
        )],
        "2026-07-07T00:00:00Z".to_string(),
        None,
    );

    manifest.validate().expect("well-formed manifest should validate");
}

/// ReleaseManifestV1::validate() rejects an empty artifact list.
#[test]
fn release_manifest_v1_rejects_empty_artifacts() {
    use legion_protocol::ReleaseManifestV1;

    let manifest = ReleaseManifestV1::new(
        "legion-desktop".to_string(),
        "stable".to_string(),
        "0.1.0".to_string(),
        None,
        None,
        vec![],
        "2026-07-07T00:00:00Z".to_string(),
        None,
    );

    let err = manifest.validate().expect_err("empty artifacts should fail validation");
    assert!(err.contains("artifact"), "error should mention artifacts: {err}");
}

/// ReleaseManifestV1::validate() rejects an artifact with an empty sha256 field.
#[test]
fn release_manifest_v1_rejects_empty_artifact_sha256() {
    use legion_protocol::{ReleaseArtifact, ReleaseManifestV1};

    let manifest = ReleaseManifestV1::new(
        "legion-desktop".to_string(),
        "preview".to_string(),
        "0.1.0-preview".to_string(),
        None,
        None,
        vec![ReleaseArtifact::new(
            "legion-desktop-linux-x64-deb".to_string(),
            "linux".to_string(),
            "x86_64-unknown-linux-gnu".to_string(),
            "legion-desktop-linux-x64-deb.deb".to_string(),
            String::new(), // empty sha256 — should fail
        )],
        "2026-07-07T00:00:00Z".to_string(),
        None,
    );

    let err = manifest.validate().expect_err("empty sha256 should fail validation");
    assert!(err.contains("sha256"), "error should mention sha256: {err}");
}

/// ReleaseManifestV1 round-trips through TOML serialize/deserialize.
#[test]
fn release_manifest_v1_toml_roundtrip() {
    use legion_protocol::{ReleaseArtifact, ReleaseManifestV1};

    let original = ReleaseManifestV1::new(
        "legion-desktop".to_string(),
        "stable".to_string(),
        "0.1.0".to_string(),
        Some("full".to_string()),
        Some("0.0.9".to_string()),
        vec![ReleaseArtifact::new(
            "legion-desktop-macos-arm64-dmg".to_string(),
            "macos".to_string(),
            "aarch64-apple-darwin".to_string(),
            "legion-desktop-macos-arm64-dmg.dmg".to_string(),
            "b".repeat(64),
        )],
        "2026-07-07T12:00:00Z".to_string(),
        Some("LEGION_SIGNING_KEY".to_string()),
    );

    let toml_str = toml::to_string_pretty(&original).expect("serialize to TOML");
    let parsed: ReleaseManifestV1 = toml::from_str(&toml_str).expect("deserialize from TOML");

    assert_eq!(original, parsed, "TOML round-trip must be lossless");
    // Crucially: signer_reference must NOT contain key material.
    assert_eq!(
        parsed.signer_reference.as_deref(),
        Some("LEGION_SIGNING_KEY"),
        "signer_reference should be a reference string, not key material"
    );
}
