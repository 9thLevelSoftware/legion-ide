//! Explicit opt-in native qualification in a separate process from mock tests.

#![cfg(windows)]

use std::process::Command;

use legion_storage::{OsKeyringSecretStore, SecretReference, SecretStore};

const SERVICE: &str = "legion-ide-synthetic-keyring-qualification";
const OPT_IN: &str = "LEGION_NATIVE_KEYRING_QUALIFICATION";
const ACCOUNT_ENV: &str = "LEGION_KEYRING_QUALIFICATION_ACCOUNT";
const MODE_ENV: &str = "LEGION_KEYRING_QUALIFICATION_CHILD_MODE";
const VALUE_A: &str = "legion synthetic qualification value A";
const VALUE_B: &str = "legion synthetic qualification value B";

fn child(account: &str, mode: &str) -> Result<(), String> {
    let output = Command::new(std::env::current_exe().map_err(|e| e.to_string())?)
        .args([
            "--exact",
            "native_keyring_child",
            "--ignored",
            "--nocapture",
        ])
        .env(OPT_IN, "1")
        .env(ACCOUNT_ENV, account)
        .env(MODE_ENV, mode)
        .output()
        .map_err(|error| format!("cannot start qualification child: {error}"))?;
    if output.status.success() {
        Ok(())
    } else {
        Err(format!(
            "qualification child {mode} failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ))
    }
}

#[test]
#[ignore = "Requires explicit native Windows keyring opt-in; creates and removes only its own synthetic entry"]
fn native_store_survives_process_restart_replacement_and_revocation() {
    assert_eq!(std::env::var(OPT_IN).as_deref(), Ok("1"));
    let account = uuid::Uuid::now_v7().to_string();
    let reference = SecretReference::new(SERVICE, &account);
    let store = OsKeyringSecretStore;
    assert!(
        store
            .load(&reference)
            .expect("native missing-key lookup failed")
            .is_none(),
        "fresh qualification account unexpectedly exists; refusing to modify it"
    );

    let result = (|| -> Result<(), String> {
        store
            .store(&reference, VALUE_A)
            .map_err(|_| "native synthetic-key creation failed".to_string())?;
        child(&account, "read-a")?;
        store
            .store(&reference, VALUE_B)
            .map_err(|_| "native synthetic-key replacement failed".to_string())?;
        child(&account, "read-b")?;
        child(&account, "delete")?;
        let absent = store
            .load(&reference)
            .map_err(|_| "native lookup after revocation failed".to_string())?;
        if absent.is_some() {
            return Err("native revocation did not remove the synthetic entry".into());
        }
        Ok(())
    })();

    // The initial absence check happened before any write. Cleanup touches only
    // this generated account, even when an intermediate child fails. Values are
    // never printed or sent to a provider, and no other credential is queried.
    let cleanup = match store.load(&reference) {
        Ok(Some(_)) => store.delete(&reference),
        Ok(None) => Ok(()),
        Err(error) => Err(error),
    };
    assert!(
        cleanup.is_ok(),
        "could not confirm cleanup of synthetic qualification account {account}"
    );
    assert!(result.is_ok(), "{}", result.unwrap_err());
}

#[test]
#[ignore = "Internal child of the explicitly opted-in native qualification parent"]
fn native_keyring_child() {
    assert_eq!(std::env::var(OPT_IN).as_deref(), Ok("1"));
    let account = std::env::var(ACCOUNT_ENV).expect("qualification account required");
    uuid::Uuid::parse_str(&account).expect("qualification account must be a UUID");
    let reference = SecretReference::new(SERVICE, account);
    let store = OsKeyringSecretStore;
    match std::env::var(MODE_ENV).as_deref() {
        Ok("read-a") => assert!(
            store
                .load(&reference)
                .expect("native child read failed")
                .as_deref()
                == Some(VALUE_A),
            "native child did not reopen the initial synthetic value"
        ),
        Ok("read-b") => assert!(
            store
                .load(&reference)
                .expect("native child read failed")
                .as_deref()
                == Some(VALUE_B),
            "native child did not reopen the replacement synthetic value"
        ),
        Ok("delete") => store
            .delete(&reference)
            .expect("native child revocation failed"),
        _ => panic!("unknown qualification child mode"),
    }
}
