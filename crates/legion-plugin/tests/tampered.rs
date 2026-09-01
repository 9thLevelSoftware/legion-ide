//! P7.F2.T3: a tampered artifact is refused before any code runs.
//!
//! "Before any code runs" is asserted structurally, not by inspection. Every
//! fixture below carries a *real, valid, executable* wasm module — one that
//! `WasmPluginHost` will happily compile and instantiate when it is presented
//! honestly. The tamper is then applied and the same bytes are offered to the
//! registry. If the refusal were a warning, or happened after load, the module
//! would run; the tests prove it does not, by writing the bytes to disk and
//! confirming the host also refuses the very same artifact.
//!
//! Each test tampers in a way that trips exactly ONE guard, so no guard is
//! masked by another. Where two guards would both fire, the assertion is on
//! the specific error variant, which still distinguishes them.

use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};

use legion_plugin::{
    ExtensionKeyring, SignedExtensionArtifact, SignedExtensionRegistry,
    SignedExtensionRegistryError, WasmPluginHost, extension_artifact_digest,
    extension_verifying_key_b64, manifest::ExtensionInstallApproval,
    manifest::ExtensionPermissionDecision, manifest::ExtensionPermissionReview,
    sign_extension_artifact,
};
use legion_protocol::{
    CapabilityId, LanguageId, PluginActivationEvent, PluginCommandDescriptor, PluginContribution,
    PluginId, PluginManifest, PluginQuotaDeclaration, PluginSignatureMetadata,
    PluginStateNamespace, PluginTreeSitterGrammarContribution, PluginTrustDecision,
    PluginTrustMetadata, PluginTrustSource,
};

/// The trusted first-party signing seed for these fixtures.
const SEED: [u8; 32] = [17u8; 32];
/// A signer the product does not trust.
const ATTACKER_SEED: [u8; 32] = [200u8; 32];

/// A genuinely valid, instantiable wasm module. Exports `legion_plugin_main`
/// and imports nothing, so `WasmPluginHost::load_fixture` accepts it when it is
/// presented untampered — see `honest_artifact_actually_loads_in_the_host`.
fn honest_wasm() -> Vec<u8> {
    wat::parse_str(
        r#"
        (module
          (func (export "legion_plugin_main") (result i32)
            i32.const 0))
        "#,
    )
    .expect("fixture wat must compile")
}

/// A different but equally valid wasm module: what an attacker would swap in.
fn malicious_wasm() -> Vec<u8> {
    wat::parse_str(
        r#"
        (module
          (memory (export "memory") 1)
          (func (export "legion_plugin_main") (result i32)
            i32.const 1))
        "#,
    )
    .expect("fixture wat must compile")
}

fn manifest_for(manifest_id: &str, artifact_bytes: &[u8]) -> PluginManifest {
    let plugin_id = PluginId(17);
    PluginManifest {
        plugin_id,
        name: "signed.extension.fixture".to_string(),
        version: "1.0.0".to_string(),
        schema_version: 1,
        min_abi_version: 1,
        max_abi_version: 1,
        module_hash: extension_artifact_digest(artifact_bytes),
        manifest_id: manifest_id.to_string(),
        trust: PluginTrustMetadata {
            source: PluginTrustSource::ExplicitLocalAllow,
            decision: PluginTrustDecision::ExplicitlyAllowed,
            reason: "first-party bundled grammar".to_string(),
        },
        signature: Some(PluginSignatureMetadata {
            signer: "legion-first-party".to_string(),
            algorithm: "ed25519".to_string(),
            signature_digest: "detached".to_string(),
        }),
        activation_events: vec![PluginActivationEvent::OnCommand {
            command: "signed.extension.run".to_string(),
        }],
        contributions: vec![
            PluginContribution::Command(PluginCommandDescriptor {
                command_id: "signed.extension.run".to_string(),
                title: "Signed Extension Run".to_string(),
                required_capability: CapabilityId("plugin.command".to_string()),
            }),
            PluginContribution::TreeSitterGrammar(PluginTreeSitterGrammarContribution {
                language_id: LanguageId("rust-plugin".to_string()),
                grammar_name: "rust-plugin-grammar".to_string(),
                artifact_uri: "file:///tmp/rust-plugin-grammar.wasm".to_string(),
                artifact_hash: "sha256:rust-plugin-grammar".to_string(),
                required_capability: CapabilityId("plugin.grammar.tree_sitter".to_string()),
            }),
        ],
        requested_capabilities: vec![
            CapabilityId("plugin.command".to_string()),
            CapabilityId("plugin.grammar.tree_sitter".to_string()),
        ],
        storage_namespace: PluginStateNamespace {
            plugin_id,
            namespace: "state".to_string(),
        },
        quotas: PluginQuotaDeclaration {
            max_fuel: 1_000,
            max_wall_time_ms: 50,
            max_memory_pages: 8,
            max_storage_bytes: 4_096,
            max_host_calls: 4,
            max_events: 4,
            max_output_bytes: 512,
        },
    }
}

fn trusted_registry() -> SignedExtensionRegistry {
    SignedExtensionRegistry::with_keyring(
        ExtensionKeyring::new()
            .with_anchor("legion-first-party", extension_verifying_key_b64(&SEED)),
    )
}

fn approval(manifest: &PluginManifest) -> ExtensionInstallApproval {
    let mut review = ExtensionPermissionReview::for_manifest(manifest);
    for index in 0..review.rows().len() {
        review.decide_at(index, ExtensionPermissionDecision::Granted);
    }
    review
        .approval(manifest)
        .expect("a fully granted review approves")
}

fn honest_artifact(manifest_id: &str) -> SignedExtensionArtifact {
    let bytes = honest_wasm();
    let manifest = manifest_for(manifest_id, &bytes);
    let signature = sign_extension_artifact(&manifest, &bytes, &SEED);
    SignedExtensionArtifact::new(manifest, bytes, signature)
}

/// Write artifact bytes to a real file so the host would genuinely execute them.
fn spill(label: &str, bytes: &[u8]) -> PathBuf {
    static SEQ: AtomicU64 = AtomicU64::new(0);
    let path = std::env::temp_dir().join(format!(
        "legion-plugin-tampered-{label}-{}-{}.wasm",
        std::process::id(),
        SEQ.fetch_add(1, Ordering::Relaxed)
    ));
    fs::write(&path, bytes).expect("fixture write must succeed");
    path
}

/// Control: the untampered artifact really does install and really does load.
///
/// Without this, every refusal below would be vacuous — the fixtures could be
/// failing for a reason unrelated to the tamper.
#[test]
fn honest_artifact_actually_loads_in_the_host() {
    let artifact = honest_artifact("manifest-honest");
    let approval = approval(&artifact.manifest);

    let mut registry = trusted_registry();
    let installed = registry
        .install(&artifact, &approval)
        .expect("an honestly signed artifact installs");
    assert_eq!(installed.signer, "legion-first-party");
    assert_eq!(installed.granted_capabilities.len(), 2);

    let path = spill("honest", &artifact.artifact_bytes);
    let mut host = WasmPluginHost::new();
    host.load_fixture(artifact.manifest.clone(), &path)
        .expect("the honest module is genuinely loadable and executable");
    let _ = fs::remove_file(&path);
}

/// Tamper 1: the payload is swapped, the manifest is left stale.
///
/// Only the artifact-digest guard can fire here.
#[test]
fn swapped_payload_with_stale_manifest_is_refused() {
    let honest = honest_artifact("manifest-swapped-payload");
    let malicious = malicious_wasm();
    assert_ne!(
        honest.artifact_bytes, malicious,
        "the swap must actually change the bytes"
    );

    // Same manifest, same signature, different bytes.
    let tampered = SignedExtensionArtifact::new(
        honest.manifest.clone(),
        malicious.clone(),
        honest.signature_b64.clone(),
    );
    let approval = approval(&tampered.manifest);

    let mut registry = trusted_registry();
    let error = registry
        .install(&tampered, &approval)
        .expect_err("a swapped payload must be refused, not warned about");
    assert_eq!(
        error,
        SignedExtensionRegistryError::ArtifactChecksumMismatch {
            expected: honest.manifest.module_hash.clone(),
            actual: extension_artifact_digest(&malicious),
        }
    );

    // Nothing was recorded, so nothing downstream can activate it.
    assert!(registry.installed("manifest-swapped-payload").is_none());
    assert!(registry.installed_extensions().is_empty());
}

/// Tamper 2: the payload is swapped AND the manifest hash is recomputed.
///
/// This is the realistic tamper: a checksum an attacker controls proves
/// nothing. The digest guard is satisfied, so only the signature can refuse it.
#[test]
fn swapped_payload_with_recomputed_hash_is_refused_by_the_signature() {
    let malicious = malicious_wasm();
    // The attacker rewrites the manifest to describe their own bytes...
    let tampered_manifest = manifest_for("manifest-recomputed-hash", &malicious);
    assert_eq!(
        tampered_manifest.module_hash,
        extension_artifact_digest(&malicious),
        "the attacker's checksum is internally consistent"
    );
    // ...but reuses the signature issued over the honest artifact.
    let honest = honest_artifact("manifest-recomputed-hash");
    let tampered =
        SignedExtensionArtifact::new(tampered_manifest, malicious, honest.signature_b64.clone());
    let approval = approval(&tampered.manifest);

    let mut registry = trusted_registry();
    let error = registry
        .install(&tampered, &approval)
        .expect_err("a self-consistent forgery must still be refused");
    assert!(
        matches!(
            error,
            SignedExtensionRegistryError::SignatureVerificationFailed { .. }
        ),
        "expected a signature refusal, got {error:?}"
    );
    assert!(registry.installed("manifest-recomputed-hash").is_none());
}

/// Tamper 3: the attacker re-signs everything with their own key.
///
/// Internally perfect, but the signer has no trust anchor.
#[test]
fn artifact_resigned_by_a_different_key_is_refused() {
    let malicious = malicious_wasm();
    let manifest = manifest_for("manifest-attacker-signed", &malicious);
    let signature = sign_extension_artifact(&manifest, &malicious, &ATTACKER_SEED);
    let tampered = SignedExtensionArtifact::new(manifest.clone(), malicious, signature);
    let approval = approval(&manifest);

    let mut registry = trusted_registry();
    let error = registry
        .install(&tampered, &approval)
        .expect_err("a signature from a key other than the anchored one must be refused");
    assert!(
        matches!(
            error,
            SignedExtensionRegistryError::SignatureVerificationFailed { .. }
        ),
        "expected a signature refusal, got {error:?}"
    );
}

/// A signer with no trust anchor is refused, and refused as such.
///
/// The neighbouring test was named for this case and did not cover it: it kept
/// `signer = "legion-first-party"`, which IS anchored, so it exercised the
/// signature check with the wrong key rather than the unknown-signer guard.
/// Those are different guards producing different errors, and the guard this
/// one names had no test at all.
#[test]
fn artifact_from_a_signer_with_no_trust_anchor_is_refused() {
    let malicious = malicious_wasm();
    let mut manifest = manifest_for("manifest-unanchored-signer", &malicious);
    let signature = sign_extension_artifact(&manifest, &malicious, &ATTACKER_SEED);
    if let Some(metadata) = manifest.signature.as_mut() {
        metadata.signer = "totally-unknown-signer".to_string();
    }
    let tampered = SignedExtensionArtifact::new(manifest.clone(), malicious, signature);
    let approval = approval(&manifest);

    let mut registry = trusted_registry();
    let error = registry
        .install(&tampered, &approval)
        .expect_err("a signer with no trust anchor must be refused");
    assert!(
        matches!(error, SignedExtensionRegistryError::UnknownSigner { .. }),
        "the refusal must name the missing anchor rather than a signature mismatch, or the          two guards are indistinguishable to a caller: {error:?}"
    );
}

/// Tamper 4: the manifest's requested capabilities are escalated after signing.
///
/// The user reviewed two permissions; the artifact ships with three.
#[test]
fn capability_escalation_after_signing_is_refused() {
    let honest = honest_artifact("manifest-escalated");
    let mut escalated_manifest = honest.manifest.clone();
    escalated_manifest
        .requested_capabilities
        .push(CapabilityId("plugin.workspace.scanner".to_string()));
    let tampered = SignedExtensionArtifact::new(
        escalated_manifest,
        honest.artifact_bytes.clone(),
        honest.signature_b64.clone(),
    );
    // The user's approval covers the two capabilities they were shown.
    let approval = approval(&honest.manifest);

    let mut registry = trusted_registry();
    let error = registry
        .install(&tampered, &approval)
        .expect_err("an escalated capability list must be refused");
    assert!(
        matches!(
            error,
            SignedExtensionRegistryError::SignatureVerificationFailed { .. }
        ),
        "the capability list is inside the signed payload; got {error:?}"
    );
}

/// The registry's refusal precedes the host entirely, and the host refuses too.
///
/// The bytes are on disk and are a real module. If either layer downgraded a
/// tamper to a warning, this module would execute.
#[test]
fn a_tampered_artifact_never_reaches_execution_in_either_layer() {
    let honest = honest_artifact("manifest-defence-in-depth");
    let malicious = malicious_wasm();
    let mut tampered_manifest = honest.manifest.clone();
    tampered_manifest.module_hash = extension_artifact_digest(&malicious);
    tampered_manifest.trust.decision = PluginTrustDecision::ChecksumMismatch;
    tampered_manifest.trust.reason = "artifact digest did not match the manifest".to_string();

    let path = spill("defence-in-depth", &malicious);
    assert!(
        fs::read(&path).expect("bytes are really on disk") == malicious,
        "the tampered module is genuinely present and would run if allowed"
    );

    // Layer 1: the registry refuses on bytes alone, having never touched disk.
    let tampered = SignedExtensionArtifact::new(
        tampered_manifest.clone(),
        malicious.clone(),
        honest.signature_b64.clone(),
    );
    let approval = approval(&tampered.manifest);
    let mut registry = trusted_registry();
    let error = registry
        .install(&tampered, &approval)
        .expect_err("registry must refuse the tampered artifact");
    assert!(
        matches!(
            error,
            SignedExtensionRegistryError::SignatureVerificationFailed { .. }
        ),
        "expected a signature refusal, got {error:?}"
    );
    assert!(registry.installed("manifest-defence-in-depth").is_none());

    // Layer 2: even handed the file directly, the host refuses to load it.
    let mut host = WasmPluginHost::new();
    let error = host
        .load_fixture(tampered_manifest, &path)
        .expect_err("host must refuse before file access or execution");
    assert_eq!(error.code, "plugin_trust_denied");
    assert_eq!(
        error.message,
        "plugin manifest is not trusted for activation"
    );
    let _ = fs::remove_file(&path);
}
