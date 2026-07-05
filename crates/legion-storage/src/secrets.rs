//! OS keyring-backed secret storage primitives for provider credentials.

#![warn(missing_docs)]

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use thiserror::Error;

/// Keyring service label used for Legion provider secrets.
pub const PROVIDER_SECRET_SERVICE: &str = "legion-ai-providers";

/// Metadata-only reference for a stored secret.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SecretReference {
    /// OS keyring service label.
    pub service: String,
    /// OS keyring account label.
    pub account: String,
}

impl SecretReference {
    /// Create a new metadata-only keyring reference.
    pub fn new(service: impl Into<String>, account: impl Into<String>) -> Self {
        Self {
            service: service.into(),
            account: account.into(),
        }
    }
}

/// Error returned by secret-store operations.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum SecretStoreError {
    /// Keyring-backed storage is unavailable or rejected the request.
    #[error("keyring operation failed: {message}")]
    KeyringFailure {
        /// Human-readable failure text.
        message: String,
    },
}

/// Store/load/delete secret values by metadata-only reference.
pub trait SecretStore {
    /// Persist a secret value.
    fn store(&self, reference: &SecretReference, secret: &str) -> Result<(), SecretStoreError>;

    /// Load a secret value if present.
    fn load(&self, reference: &SecretReference) -> Result<Option<String>, SecretStoreError>;

    /// Delete a stored secret value.
    fn delete(&self, reference: &SecretReference) -> Result<(), SecretStoreError>;
}

/// OS keyring-backed secret store.
#[derive(Debug, Clone, Default)]
pub struct OsKeyringSecretStore;

impl SecretStore for OsKeyringSecretStore {
    fn store(&self, reference: &SecretReference, secret: &str) -> Result<(), SecretStoreError> {
        keyring::Entry::new(&reference.service, &reference.account)
            .map_err(keyring_error)?
            .set_password(secret)
            .map_err(keyring_error)
    }

    fn load(&self, reference: &SecretReference) -> Result<Option<String>, SecretStoreError> {
        let entry =
            keyring::Entry::new(&reference.service, &reference.account).map_err(keyring_error)?;
        match entry.get_password() {
            Ok(secret) => Ok(Some(secret)),
            Err(error) if is_not_found_error(&error) => Ok(None),
            Err(error) => Err(keyring_error(error)),
        }
    }

    fn delete(&self, reference: &SecretReference) -> Result<(), SecretStoreError> {
        keyring::Entry::new(&reference.service, &reference.account)
            .map_err(keyring_error)?
            .delete_credential()
            .map_err(keyring_error)
    }
}

/// In-memory secret store for tests.
#[derive(Debug, Clone, Default)]
pub struct InMemorySecretStore {
    secrets: Arc<Mutex<HashMap<SecretReference, String>>>,
}

impl SecretStore for InMemorySecretStore {
    fn store(&self, reference: &SecretReference, secret: &str) -> Result<(), SecretStoreError> {
        self.secrets
            .lock()
            .expect("secret store lock")
            .insert(reference.clone(), secret.to_string());
        Ok(())
    }

    fn load(&self, reference: &SecretReference) -> Result<Option<String>, SecretStoreError> {
        Ok(self
            .secrets
            .lock()
            .expect("secret store lock")
            .get(reference)
            .cloned())
    }

    fn delete(&self, reference: &SecretReference) -> Result<(), SecretStoreError> {
        self.secrets
            .lock()
            .expect("secret store lock")
            .remove(reference);
        Ok(())
    }
}

/// Build the metadata-only keyring location for a provider secret.
pub fn provider_secret_reference(provider_id: &str, secret_name: &str) -> SecretReference {
    SecretReference::new(
        PROVIDER_SECRET_SERVICE,
        format!("{provider_id}:{secret_name}"),
    )
}

fn keyring_error(error: keyring::Error) -> SecretStoreError {
    SecretStoreError::KeyringFailure {
        message: error.to_string(),
    }
}

fn is_not_found_error(error: &keyring::Error) -> bool {
    error.to_string().to_ascii_lowercase().contains("not found")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn provider_secret_reference_is_metadata_only() {
        let reference = provider_secret_reference("openai", "OPENAI_API_KEY");

        assert_eq!(reference.service, PROVIDER_SECRET_SERVICE);
        assert_eq!(reference.account, "openai:OPENAI_API_KEY");
        let debug = format!("{reference:?}");
        assert!(debug.contains(PROVIDER_SECRET_SERVICE));
        assert!(!debug.contains("sk-"));
        assert!(!debug.contains("secret"));
    }

    #[test]
    fn in_memory_secret_store_round_trips_values_without_disk_io() {
        let store = InMemorySecretStore::default();
        let reference = provider_secret_reference("anthropic", "ANTHROPIC_API_KEY");

        assert_eq!(store.load(&reference).unwrap(), None);
        store.store(&reference, "stored-secret").unwrap();
        assert_eq!(
            store.load(&reference).unwrap(),
            Some("stored-secret".to_string())
        );
        store.delete(&reference).unwrap();
        assert_eq!(store.load(&reference).unwrap(), None);
    }
}
