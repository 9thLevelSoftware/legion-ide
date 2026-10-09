//! Isolated test binary: its mock backend never reads the operating-system store.

use std::any::Any;

use keyring::credential::{Credential, CredentialBuilderApi};
use legion_storage::{OsKeyringSecretStore, SecretReference, SecretStore};

struct BoundaryBuilder;

impl CredentialBuilderApi for BoundaryBuilder {
    fn build(
        &self,
        _target: Option<&str>,
        _service: &str,
        user: &str,
    ) -> keyring::Result<Box<Credential>> {
        let credential = keyring::mock::MockCredential::default();
        if user == "platform-failure" {
            credential.set_error(keyring::Error::PlatformFailure(Box::new(
                std::io::Error::new(
                    std::io::ErrorKind::NotFound,
                    "credential storage device not found",
                ),
            )));
        }
        Ok(Box::new(credential))
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

#[test]
fn absent_credentials_are_none_but_storage_failures_remain_errors() {
    // This binary owns its process-global builder; no native-keyring test runs
    // in this process, and other storage test binaries retain their own backend.
    keyring::set_default_credential_builder(Box::new(BoundaryBuilder));
    let store = OsKeyringSecretStore;
    let missing = SecretReference::new("legion-test", "missing");
    assert!(
        store
            .load(&missing)
            .expect("NoEntry must mean absent")
            .is_none(),
        "an absent credential must not acquire a value"
    );

    let failed = SecretReference::new("legion-test", "platform-failure");
    assert!(
        store.load(&failed).is_err(),
        "a platform error mentioning 'not found' is not a missing credential"
    );
}
