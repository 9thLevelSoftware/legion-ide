//! Signature verification for org policy bundles (P9.F2.T3).
//!
//! Every test here is a refusal test except the two that establish the happy
//! path. That balance is deliberate: a signing scheme is only worth anything if
//! it says no, and the failure modes that matter — a relaxed policy re-signed by
//! nobody, an unsigned bundle presented as signed, a trust store nobody
//! configured — all look like success to a permissive implementation.

use legion_security::{
    POLICY_BUNDLE_SIGNATURE_ALGORITHM, PolicyBundleError, PolicyKeyring, PolicySigningKey,
    SignedPolicyBundle, policy_bundle_verifying_key_b64, sign_policy_bundle,
};

/// Deterministic test seeds. Never written to disk, never a real key.
const ORG_SEED: [u8; 32] = [7u8; 32];
const IMPOSTOR_SEED: [u8; 32] = [9u8; 32];

const ORG_KEY_ID: &str = "org-policy-signer-1";

fn enterprise_payload() -> String {
    include_str!("../../../xtask/legion-policy.example.toml").to_string()
}

fn org_keyring() -> PolicyKeyring {
    PolicyKeyring::new(vec![PolicySigningKey {
        key_id: ORG_KEY_ID.to_string(),
        verifying_key_b64: policy_bundle_verifying_key_b64(&ORG_SEED),
    }])
}

fn signed_enterprise_bundle() -> SignedPolicyBundle {
    sign_policy_bundle(enterprise_payload(), ORG_KEY_ID, &ORG_SEED)
}

// -------------------------------------------------------------------------
// Happy path — established so the refusals below are known to be refusals of
// something that would otherwise work.
// -------------------------------------------------------------------------

#[test]
fn correctly_signed_enterprise_bundle_verifies() {
    let verified = signed_enterprise_bundle()
        .verify(&org_keyring())
        .expect("a bundle signed by a configured trust anchor must verify");

    assert_eq!(verified.signing_key_id(), ORG_KEY_ID);
    assert_eq!(verified.bundle().bundle_id, "enterprise-restrictive");
    assert_eq!(
        verified.bundle().mode_ceiling,
        legion_protocol::ProductMode::Assist
    );
}

#[test]
fn signature_algorithm_is_the_release_manifest_algorithm() {
    // ADR-0042 fixed Ed25519 for the release manifest. Reusing that identifier
    // here is what keeps the product on one signing scheme rather than two.
    assert_eq!(POLICY_BUNDLE_SIGNATURE_ALGORITHM, "ed25519");
    assert_eq!(
        signed_enterprise_bundle().algorithm,
        POLICY_BUNDLE_SIGNATURE_ALGORITHM
    );
}

// -------------------------------------------------------------------------
// Fail-closed refusals
// -------------------------------------------------------------------------

#[test]
fn tampered_payload_is_rejected() {
    let mut bundle = signed_enterprise_bundle();
    bundle.payload_toml.push_str("\n# appended after signing\n");

    assert!(matches!(
        bundle.verify(&org_keyring()),
        Err(PolicyBundleError::SignatureMismatch(_))
    ));
}

#[test]
fn relaxing_the_mode_ceiling_after_signing_is_rejected() {
    // The attack this scheme exists to stop: an operator (or malware) edits the
    // distributed bundle to raise the mode ceiling and leaves the signature
    // alone, hoping nobody checks.
    let mut bundle = signed_enterprise_bundle();
    assert!(
        bundle.payload_toml.contains("mode_ceiling = \"Assist\""),
        "fixture must contain the ceiling this test rewrites"
    );
    bundle.payload_toml = bundle
        .payload_toml
        .replace("mode_ceiling = \"Assist\"", "mode_ceiling = \"Automate\"");

    assert!(matches!(
        bundle.verify(&org_keyring()),
        Err(PolicyBundleError::SignatureMismatch(_))
    ));
}

#[test]
fn widening_the_provider_allowlist_after_signing_is_rejected() {
    let mut bundle = signed_enterprise_bundle();
    assert!(
        bundle.payload_toml.contains("\"ollama\","),
        "fixture must contain the allowlist entry this test rewrites"
    );
    bundle.payload_toml = bundle
        .payload_toml
        .replace("\"ollama\",", "\"ollama\",\n  \"openai\",");

    assert!(matches!(
        bundle.verify(&org_keyring()),
        Err(PolicyBundleError::SignatureMismatch(_))
    ));
}

#[test]
fn tampered_signature_is_rejected() {
    let mut bundle = signed_enterprise_bundle();
    // Flip one base64 character to something else in the alphabet, keeping the
    // signature decodable so this exercises verification, not decoding.
    let flipped = if bundle.signature_b64.starts_with('A') {
        'B'
    } else {
        'A'
    };
    bundle
        .signature_b64
        .replace_range(0..1, &flipped.to_string());

    assert!(matches!(
        bundle.verify(&org_keyring()),
        Err(PolicyBundleError::SignatureMismatch(_))
    ));
}

#[test]
fn signature_from_a_different_key_is_rejected() {
    // Same key id, different key material: an impostor who knows the org's key
    // id but not its private key.
    let bundle = sign_policy_bundle(enterprise_payload(), ORG_KEY_ID, &IMPOSTOR_SEED);

    assert!(matches!(
        bundle.verify(&org_keyring()),
        Err(PolicyBundleError::SignatureMismatch(_))
    ));
}

#[test]
fn empty_keyring_honours_nothing() {
    // "No trust anchors configured" must mean "no bundle applies", never
    // "every bundle applies".
    assert!(matches!(
        signed_enterprise_bundle().verify(&PolicyKeyring::empty()),
        Err(PolicyBundleError::EmptyKeyring)
    ));
    assert!(PolicyKeyring::default().is_empty());
}

#[test]
fn unknown_key_id_is_rejected() {
    let bundle = sign_policy_bundle(enterprise_payload(), "some-other-signer", &ORG_SEED);

    assert!(matches!(
        bundle.verify(&org_keyring()),
        Err(PolicyBundleError::UnknownKeyId(id)) if id == "some-other-signer"
    ));
}

#[test]
fn unsigned_bundle_cannot_be_honoured_as_signed() {
    // Three shapes an "unsigned" bundle can take. None of them may verify, and
    // in particular none may verify *because* it declined to claim a signature.
    let payload = enterprise_payload();

    let algorithm_none = SignedPolicyBundle {
        algorithm: "none".to_string(),
        key_id: ORG_KEY_ID.to_string(),
        signature_b64: String::new(),
        payload_toml: payload.clone(),
    };
    assert!(matches!(
        algorithm_none.verify(&org_keyring()),
        Err(PolicyBundleError::UnsupportedAlgorithm(alg)) if alg == "none"
    ));

    let algorithm_blank = SignedPolicyBundle {
        algorithm: String::new(),
        key_id: ORG_KEY_ID.to_string(),
        signature_b64: String::new(),
        payload_toml: payload.clone(),
    };
    assert!(matches!(
        algorithm_blank.verify(&org_keyring()),
        Err(PolicyBundleError::UnsupportedAlgorithm(_))
    ));

    let empty_signature = SignedPolicyBundle {
        algorithm: POLICY_BUNDLE_SIGNATURE_ALGORITHM.to_string(),
        key_id: ORG_KEY_ID.to_string(),
        signature_b64: String::new(),
        payload_toml: payload,
    };
    assert!(matches!(
        empty_signature.verify(&org_keyring()),
        Err(PolicyBundleError::SignatureMismatch(_))
    ));
}

#[test]
fn algorithm_match_is_exact_not_case_folded() {
    // A downgrade dressed up as a spelling variant is still a downgrade.
    let mut bundle = signed_enterprise_bundle();
    bundle.algorithm = "ED25519".to_string();

    assert!(matches!(
        bundle.verify(&org_keyring()),
        Err(PolicyBundleError::UnsupportedAlgorithm(_))
    ));
}

#[test]
fn undecodable_signature_is_rejected() {
    let mut bundle = signed_enterprise_bundle();
    bundle.signature_b64 = "not base64 !!!".to_string();

    assert!(matches!(
        bundle.verify(&org_keyring()),
        Err(PolicyBundleError::MalformedSignature(_))
    ));
}

#[test]
fn malformed_trust_anchor_is_rejected_rather_than_skipped() {
    let keyring = PolicyKeyring::new(vec![PolicySigningKey {
        key_id: ORG_KEY_ID.to_string(),
        verifying_key_b64: "%%%not base64%%%".to_string(),
    }]);

    assert!(matches!(
        signed_enterprise_bundle().verify(&keyring),
        Err(PolicyBundleError::MalformedKey { key_id, .. }) if key_id == ORG_KEY_ID
    ));
}

#[test]
fn wrong_length_trust_anchor_is_rejected() {
    // A 16-byte "key" is not an Ed25519 public key. It must fail as a bad key,
    // not silently verify or panic.
    let keyring = PolicyKeyring::new(vec![PolicySigningKey {
        key_id: ORG_KEY_ID.to_string(),
        verifying_key_b64: base64_encode(&[3u8; 16]),
    }]);

    assert!(matches!(
        signed_enterprise_bundle().verify(&keyring),
        Err(PolicyBundleError::MalformedKey { .. })
    ));
}

#[test]
fn unparseable_payload_is_rejected_even_with_a_valid_signature() {
    // A correctly signed payload that is not a bundle must not become one.
    let bundle = sign_policy_bundle("this is not = valid ] toml [", ORG_KEY_ID, &ORG_SEED);

    assert!(matches!(
        bundle.verify(&org_keyring()),
        Err(PolicyBundleError::MalformedPayload(_))
    ));
}

#[test]
fn unsupported_schema_version_is_rejected_even_when_signed() {
    let payload = enterprise_payload().replace("schema_version = 1", "schema_version = 99");
    let bundle = sign_policy_bundle(payload, ORG_KEY_ID, &ORG_SEED);

    assert!(matches!(
        bundle.verify(&org_keyring()),
        Err(PolicyBundleError::UnsupportedSchemaVersion { found: 99, .. })
    ));
}

/// Local base64 helper so this test does not need its own base64 dependency.
fn base64_encode(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::new();
    for chunk in bytes.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = chunk.get(1).copied().unwrap_or(0) as u32;
        let b2 = chunk.get(2).copied().unwrap_or(0) as u32;
        let triple = (b0 << 16) | (b1 << 8) | b2;
        out.push(ALPHABET[(triple >> 18) as usize & 0x3f] as char);
        out.push(ALPHABET[(triple >> 12) as usize & 0x3f] as char);
        out.push(if chunk.len() > 1 {
            ALPHABET[(triple >> 6) as usize & 0x3f] as char
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            ALPHABET[triple as usize & 0x3f] as char
        } else {
            '='
        });
    }
    out
}
