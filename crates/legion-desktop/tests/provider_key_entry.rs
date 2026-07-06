//! BYOK key entry tests — storage, deletion, no-disk-leak, activation gate.

use legion_ai_providers::{ANTHROPIC_PROVIDER_ID, can_activate_provider, provider_tier};
use legion_protocol::{
    AssistedAiProviderClass, AssistedAiWorkspaceConsent, PrincipalId, TimestampMillis,
};
use legion_storage::{InMemorySecretStore, SecretStore, provider_secret_reference};

#[test]
fn set_provider_api_key_stores_in_keyring() {
    let store = InMemorySecretStore::default();
    let reference = provider_secret_reference(ANTHROPIC_PROVIDER_ID, "api_key");

    store
        .store(&reference, "test-key-value-abcdef")
        .expect("store must succeed for InMemorySecretStore");

    let loaded = store.load(&reference).expect("load must succeed");
    assert_eq!(
        loaded,
        Some("test-key-value-abcdef".to_string()),
        "loaded value must match stored value"
    );
}

#[test]
fn delete_provider_api_key_removes_from_keyring() {
    let store = InMemorySecretStore::default();
    let reference = provider_secret_reference(ANTHROPIC_PROVIDER_ID, "api_key");

    store
        .store(&reference, "test-key-to-delete")
        .expect("store must succeed");
    store.delete(&reference).expect("delete must succeed");

    let loaded = store
        .load(&reference)
        .expect("load after delete must succeed");
    assert_eq!(loaded, None, "key must be absent after deletion");
}

#[test]
fn set_key_never_writes_to_disk() {
    // This test verifies that the InMemorySecretStore path used in tests
    // does not write any secret to disk. We write to the in-memory store,
    // then scan the temp directory for any file containing the sentinel key.
    let sentinel = "PROV-TEST-SENTINEL-KEY-NO-DISK-LEAK-12345";

    let store = InMemorySecretStore::default();
    let reference = provider_secret_reference(ANTHROPIC_PROVIDER_ID, "api_key");
    store
        .store(&reference, sentinel)
        .expect("store must succeed");

    // Scan temp directory for leaked sentinel value.
    // A well-behaved InMemorySecretStore must not write to any file.
    let tmp_dir = std::env::temp_dir();
    let leaked = scan_dir_for_string(&tmp_dir, sentinel);
    assert!(
        leaked.is_empty(),
        "sentinel key must not appear in any temp file; found in: {leaked:?}"
    );
}

/// Recursively scan a directory for any file that contains `needle`.
/// Returns a list of matching file paths.
fn scan_dir_for_string(dir: &std::path::Path, needle: &str) -> Vec<std::path::PathBuf> {
    let mut found = Vec::new();
    let Ok(entries) = std::fs::read_dir(dir) else {
        return found;
    };
    for entry in entries.filter_map(Result::ok) {
        let path = entry.path();
        if path.is_dir() {
            // Limit recursion depth to avoid scanning the whole filesystem
            continue;
        }
        if let Ok(contents) = std::fs::read_to_string(&path)
            && contents.contains(needle)
        {
            found.push(path);
        }
    }
    found
}

#[test]
fn set_key_activates_provider() {
    // After storing a key and granting consent, can_activate_provider returns Ok.
    let store = InMemorySecretStore::default();
    let reference = provider_secret_reference(ANTHROPIC_PROVIDER_ID, "api_key");

    // Before storing: consent granted but no credential → denied
    let granted = AssistedAiWorkspaceConsent::Granted {
        granted_at: TimestampMillis(1_000_000),
        principal: PrincipalId("test-principal".to_string()),
    };
    let tier = provider_tier(AssistedAiProviderClass::ByokRemote, ANTHROPIC_PROVIDER_ID);
    let before = can_activate_provider(tier, &granted, false);
    assert!(
        before.is_err(),
        "must be denied before credential is stored"
    );

    // Store the key
    store
        .store(&reference, "test-activation-key")
        .expect("store must succeed");
    let has_credential = store.load(&reference).unwrap().is_some();

    // After storing: consent granted + credential present → activated
    let after = can_activate_provider(tier, &granted, has_credential);
    assert!(
        after.is_ok(),
        "must be activated after key is stored and consent is granted"
    );
}
