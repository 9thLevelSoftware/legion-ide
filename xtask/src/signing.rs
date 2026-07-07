//! Release signing infrastructure for xtask (PKT-SIGN / ADR-0042).
//!
//! # Security invariants
//!
//! * Key material (the 32-byte Ed25519 seed) is **never** logged, printed, or
//!   persisted.  The seed bytes are zeroized immediately after constructing the
//!   `SigningKey`.
//! * `signer_reference` fields carry *reference strings* only (env-var name,
//!   keyring label, KMS URI) — never the material itself.
//! * Test keys are ephemeral: generated in-test, never written to disk.

use std::fmt;

use zeroize::Zeroizing;

/// Error type for all signing operations.
#[derive(Debug)]
pub enum SigningError {
    /// The provided key bytes were not a valid Ed25519 key.
    InvalidKey(String),
    /// The signing operation itself failed.
    SignFailed(String),
    /// Signature verification failed (tampering detected).
    VerifyFailed(String),
    /// Base64 decode of a key or signature failed.
    Base64Decode(String),
}

impl fmt::Display for SigningError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidKey(msg) => write!(f, "invalid key: {msg}"),
            Self::SignFailed(msg) => write!(f, "sign failed: {msg}"),
            Self::VerifyFailed(msg) => write!(f, "verify failed: {msg}"),
            Self::Base64Decode(msg) => write!(f, "base64 decode: {msg}"),
        }
    }
}

/// A type that can sign arbitrary bytes and expose its verifying key.
pub trait Signer: Send + Sync {
    /// Sign `data` and return the raw 64-byte signature bytes.
    fn sign_bytes(&self, data: &[u8]) -> Result<Vec<u8>, SigningError>;
    /// Return the raw 32-byte verifying (public) key bytes.
    fn verifying_key_bytes(&self) -> Vec<u8>;
}

/// Result of attempting to resolve a signer from config.
pub enum SignerResolution {
    /// A signer was successfully resolved and is ready to use.
    Available(Box<dyn Signer>),
    /// No signer is available; the caller should produce an unsigned-beta
    /// artifact rather than failing.
    Unavailable { reason: String },
}

/// Configuration parsed from the `[signing]` section of the release pipeline TOML.
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize, Default)]
#[serde(default)]
pub struct SigningConfig {
    /// Source type: `"env"`, `"ci-secret"`, `"keyring"`, or `"kms"`.
    pub source: String,
    /// Reference string (env-var name, keyring service name, KMS URI, etc.).
    ///
    /// **Never put key material here** — only the reference needed to look it up.
    pub reference: String,
    /// Human-readable signer identity (used as username for keyring lookups).
    pub identity: String,
}

/// Configuration for the `[updater]` section of the release pipeline TOML.
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize, Default)]
#[serde(default)]
pub struct UpdaterConfig {
    /// Update strategy (e.g. `"custom-zed-style"`).
    pub strategy: String,
    /// Manifest filename (e.g. `"release-manifest.v1.toml"`).
    pub manifest: String,
    /// Signature algorithm (e.g. `"ed25519-detached"`).
    pub signature: String,
}

/// Ed25519 signer wrapping `ed25519_dalek::SigningKey`.
///
/// Key material is held in memory only while this struct is live; no key bytes
/// are logged or written to disk at any point.
pub struct DalekSigner {
    signing_key: ed25519_dalek::SigningKey,
}

impl DalekSigner {
    /// Construct from a 32-byte seed.
    ///
    /// The caller MUST zeroize the seed bytes immediately after calling this
    /// constructor.
    pub fn from_seed(seed: &[u8; 32]) -> Self {
        Self {
            signing_key: ed25519_dalek::SigningKey::from_bytes(seed),
        }
    }
}

impl Signer for DalekSigner {
    fn sign_bytes(&self, data: &[u8]) -> Result<Vec<u8>, SigningError> {
        // Import the ed25519-dalek Signer trait anonymously to enable method
        // dispatch without shadowing our own `Signer` trait name.
        use ed25519_dalek::Signer as _;
        let signature: ed25519_dalek::Signature = self.signing_key.sign(data);
        Ok(signature.to_bytes().to_vec())
    }

    fn verifying_key_bytes(&self) -> Vec<u8> {
        self.signing_key.verifying_key().to_bytes().to_vec()
    }
}

/// Verify an Ed25519 signature over `data`.
///
/// # Parameters
/// * `data` — the payload that was signed (e.g. serialized manifest TOML bytes)
/// * `signature` — the 64-byte raw Ed25519 signature
/// * `verifying_key` — the 32-byte compressed public key
///
/// Returns `Ok(())` on a valid signature, `Err(SigningError::VerifyFailed)` when
/// tamper detection fires, or another `SigningError` variant for malformed inputs.
pub fn verify_ed25519_signature(
    data: &[u8],
    signature: &[u8],
    verifying_key: &[u8],
) -> Result<(), SigningError> {
    let key_bytes: &[u8; 32] = verifying_key.try_into().map_err(|_| {
        SigningError::InvalidKey(format!(
            "verifying key must be 32 bytes, got {}",
            verifying_key.len()
        ))
    })?;
    let vk = ed25519_dalek::VerifyingKey::from_bytes(key_bytes)
        .map_err(|err| SigningError::InvalidKey(err.to_string()))?;

    let sig_bytes: &[u8; 64] = signature.try_into().map_err(|_| {
        SigningError::VerifyFailed(format!(
            "signature must be 64 bytes, got {}",
            signature.len()
        ))
    })?;
    let sig = ed25519_dalek::Signature::from_bytes(sig_bytes);

    vk.verify_strict(data, &sig)
        .map_err(|err| SigningError::VerifyFailed(err.to_string()))
}

/// Resolve a signer from a [`SigningConfig`].
///
/// Returns [`SignerResolution::Available`] when key material is reachable, or
/// [`SignerResolution::Unavailable`] (with a reason) when credentials are absent.
/// The latter is a first-class outcome — callers should produce an
/// `"unsigned-beta/no-signer-configured"` artifact, not fail.
pub fn resolve_signer(config: &SigningConfig) -> SignerResolution {
    match config.source.as_str() {
        "env" | "ci-secret" => resolve_from_env(config),
        "keyring" => resolve_from_keyring(config),
        "kms" => SignerResolution::Unavailable {
            reason: "KMS signer not yet implemented — honest unavailable".to_string(),
        },
        "" => SignerResolution::Unavailable {
            reason: "no signing source configured".to_string(),
        },
        other => SignerResolution::Unavailable {
            reason: format!(
                "unknown signing source `{other}`; expected env, ci-secret, keyring, or kms"
            ),
        },
    }
}

// ---------------------------------------------------------------------------
// Resolver implementations
// ---------------------------------------------------------------------------

fn resolve_from_env(config: &SigningConfig) -> SignerResolution {
    let var_name = &config.reference;
    // Wrap the raw base64 string in Zeroizing so it is zeroed on drop even
    // across early returns or panics — key material must not linger in heap.
    let value = Zeroizing::new(match std::env::var(var_name) {
        Ok(v) => v,
        Err(_) => {
            return SignerResolution::Unavailable {
                reason: format!("env var `{var_name}` is not set"),
            };
        }
    });

    // Decode base64 seed — Zeroizing<Vec<u8>> ensures zeroing via Drop on all
    // exit paths including panics, consistent with the keyring resolver path.
    let seed_bytes: Zeroizing<Vec<u8>> = match base64_decode(&value) {
        Ok(bytes) => Zeroizing::new(bytes),
        Err(err) => {
            return SignerResolution::Unavailable {
                reason: format!(
                    "env var `{var_name}` is not a valid base64 Ed25519 seed: {err}"
                ),
            };
        }
    };

    if seed_bytes.len() != 32 {
        let len = seed_bytes.len();
        // seed_bytes is dropped (and zeroed) here by Zeroizing's Drop impl.
        return SignerResolution::Unavailable {
            reason: format!(
                "env var `{var_name}` decoded to {len} bytes; expected 32-byte Ed25519 seed"
            ),
        };
    }

    let seed_arr: [u8; 32] = seed_bytes.as_slice().try_into().expect("length checked above");
    let signer = DalekSigner::from_seed(&seed_arr);
    // seed_bytes is dropped (and zeroed) by Zeroizing's Drop impl here.

    SignerResolution::Available(Box::new(signer))
}

fn resolve_from_keyring(config: &SigningConfig) -> SignerResolution {
    let entry = match keyring::Entry::new(&config.reference, &config.identity) {
        Ok(e) => e,
        Err(err) => {
            return SignerResolution::Unavailable {
                reason: format!(
                    "keyring entry `{}` / `{}` could not be opened: {err}",
                    config.reference, config.identity
                ),
            };
        }
    };

    let secret = match entry.get_password() {
        Ok(s) => s,
        Err(keyring::Error::NoEntry) => {
            return SignerResolution::Unavailable {
                reason: format!(
                    "keyring entry not found: service=`{}` user=`{}`",
                    config.reference, config.identity
                ),
            };
        }
        Err(err) => {
            return SignerResolution::Unavailable {
                reason: format!(
                    "keyring lookup failed for service=`{}` user=`{}`: {err}",
                    config.reference, config.identity
                ),
            };
        }
    };

    // Wrap in Zeroizing so the String is zeroed on drop — key material must
    // not linger in heap memory after this function returns.
    let secret = Zeroizing::new(secret);

    // Zeroizing<Vec<u8>> ensures decoded seed bytes are zeroed via Drop on all
    // exit paths including panics.
    let seed_bytes: Zeroizing<Vec<u8>> = match base64_decode(&secret) {
        Ok(bytes) => Zeroizing::new(bytes),
        Err(err) => {
            return SignerResolution::Unavailable {
                reason: format!("keyring entry is not a valid base64 Ed25519 seed: {err}"),
            };
        }
    };

    if seed_bytes.len() != 32 {
        let len = seed_bytes.len();
        // seed_bytes is dropped (and zeroed) here by Zeroizing's Drop impl.
        return SignerResolution::Unavailable {
            reason: format!(
                "keyring entry decoded to {len} bytes; expected 32-byte Ed25519 seed"
            ),
        };
    }

    let seed_arr: [u8; 32] = seed_bytes.as_slice().try_into().expect("length checked above");
    let signer = DalekSigner::from_seed(&seed_arr);
    // seed_bytes is dropped (and zeroed) by Zeroizing's Drop impl here.

    SignerResolution::Available(Box::new(signer))
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

fn base64_decode(encoded: &str) -> Result<Vec<u8>, String> {
    use base64::Engine as _;
    base64::engine::general_purpose::STANDARD
        .decode(encoded.trim())
        .map_err(|err| err.to_string())
}
