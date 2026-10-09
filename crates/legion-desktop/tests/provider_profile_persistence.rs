use std::sync::Arc;

use legion_app::{AiProviderProfile, AppComposition, AppProductMode};
use legion_desktop::session::DesktopSessionStore;
use legion_protocol::{PrincipalId, WorkspaceTrustState};
use legion_storage::InMemorySecretStore;

fn app(root: &std::path::Path, store: Arc<InMemorySecretStore>) -> AppComposition {
    let mut app = AppComposition::with_provider_secret_store(store);
    app.open_workspace(
        root,
        WorkspaceTrustState::Trusted,
        PrincipalId("profile-persistence".into()),
    )
    .unwrap();
    app.open_file(root.join("main.rs").to_string_lossy())
        .unwrap();
    app
}

fn profile() -> AiProviderProfile {
    AiProviderProfile {
        name: "mimo-sgp".into(),
        provider_id: "openai-compatible".into(),
        endpoint: "https://token-plan-sgp.xiaomimimo.com/v1".into(),
        model: "mimo-v2.6-pro".into(),
        max_completion_tokens: true,
        disable_thinking: true,
    }
}

#[test]
fn reopened_session_preserves_named_profiles_selection_and_secure_reference_in_manual() {
    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("main.rs"), "fn main() {}\n").unwrap();
    let path = root.path().join("session.json");
    let store = Arc::new(InMemorySecretStore::default());
    let mut original = app(root.path(), store.clone());
    original.configure_ai_provider_profile(profile()).unwrap();
    let mut alternate = profile();
    alternate.name = "unselected".into();
    original.configure_ai_provider_profile(alternate).unwrap();
    original.select_ai_provider_profile("mimo-sgp").unwrap();
    original
        .replace_ai_profile_credential("mimo-sgp", "synthetic-persistence-key")
        .unwrap();
    original.set_product_mode(AppProductMode::Assist);
    DesktopSessionStore::save(&path, &original.capture_workspace_session_record().unwrap())
        .unwrap();
    drop(original);

    let bytes = std::fs::read_to_string(&path).unwrap();
    assert!(!bytes.contains("synthetic-persistence-key"));
    let saved = DesktopSessionStore::load(&path).unwrap().unwrap();
    let mut reopened = app(root.path(), store);
    reopened.restore_workspace_session_record(&saved).unwrap();
    let profiles = reopened.ai_provider_profiles();
    assert_eq!(
        profiles.len(),
        2,
        "profiles must survive reopening the existing settings file"
    );
    assert_eq!(profiles[0].profile.name, "mimo-sgp");
    assert_eq!(
        profiles[0].profile.endpoint,
        "https://token-plan-sgp.xiaomimimo.com/v1"
    );
    assert_eq!(profiles[0].profile.model, "mimo-v2.6-pro");
    assert!(profiles[0].profile.max_completion_tokens && profiles[0].profile.disable_thinking);
    assert!(profiles[0].selected);
    assert!(!profiles[1].selected);
    assert_eq!(profiles[0].credential_state, "stored");
    assert_eq!(profiles[1].credential_state, "missing");
    assert_eq!(profiles[0].health, "not checked");
    assert_eq!(reopened.product_mode(), AppProductMode::Manual);
    assert!(!reopened.product_ai_stream_in_flight());
    reopened.revoke_ai_profile_credential("mimo-sgp").unwrap();
    assert_eq!(
        reopened.ai_provider_profiles()[0].credential_state,
        "missing"
    );
}

#[test]
fn invalid_profile_metadata_cannot_replace_saved_or_active_session_settings() {
    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("main.rs"), "fn main() {}\n").unwrap();
    let path = root.path().join("session.json");
    let corrupt_path = root.path().join("invalid-session.json");
    let mut active = app(root.path(), Arc::new(InMemorySecretStore::default()));
    active.configure_ai_provider_profile(profile()).unwrap();
    active.select_ai_provider_profile("mimo-sgp").unwrap();
    let good = active.capture_workspace_session_record().unwrap();
    DesktopSessionStore::save(&path, &good).unwrap();
    let previous_bytes = std::fs::read(&path).unwrap();
    let previous_config = active.ai_provider_configuration_json().unwrap();
    let previous_settings = active
        .shell_projection_snapshot("before")
        .unwrap()
        .settings_projection;
    let invalid = [
        "{".to_string(),
        r#"{"profiles":[],"selected":"missing"}"#.into(),
        r#"{"profiles":[],"selected":null,"api_key":"synthetic-forbidden-key"}"#.into(),
        " ".repeat(32769),
    ];
    for metadata in invalid {
        let mut record = good.clone();
        record.workbench_settings.ai_provider_configuration_json = Some(metadata);
        record.workbench_settings.zoom_percent = 175;
        let error = DesktopSessionStore::save(&path, &record)
            .expect_err("invalid metadata cannot be published");
        assert!(!error.to_string().contains("synthetic-forbidden-key"));
        assert_eq!(std::fs::read(&path).unwrap(), previous_bytes);
        assert!(active.restore_workspace_session_record(&record).is_err());
        assert_eq!(
            active.ai_provider_configuration_json().unwrap(),
            previous_config
        );
        assert_eq!(
            active
                .shell_projection_snapshot("after")
                .unwrap()
                .settings_projection,
            previous_settings
        );
        assert_eq!(active.product_mode(), AppProductMode::Manual);
        std::fs::write(&corrupt_path, serde_json::to_vec(&record).unwrap()).unwrap();
        assert!(
            DesktopSessionStore::load(&corrupt_path).is_err(),
            "invalid stored metadata must not silently become a default profile"
        );
    }
    let saved = DesktopSessionStore::load(&path).unwrap().unwrap();
    let mut reopened = app(root.path(), Arc::new(InMemorySecretStore::default()));
    reopened.restore_workspace_session_record(&saved).unwrap();
    assert_eq!(
        reopened.ai_provider_profiles()[0].profile.model,
        "mimo-v2.6-pro"
    );
    assert!(reopened.ai_provider_profiles()[0].selected);
}

#[test]
fn legacy_session_without_profiles_restores_empty_selection_without_deleting_keys() {
    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("main.rs"), "fn main() {}\n").unwrap();
    let path = root.path().join("legacy-session.json");
    let store = Arc::new(InMemorySecretStore::default());
    let mut active = app(root.path(), store);
    active.configure_ai_provider_profile(profile()).unwrap();
    active.select_ai_provider_profile("mimo-sgp").unwrap();
    active
        .replace_ai_profile_credential("mimo-sgp", "synthetic-legacy-key")
        .unwrap();
    let mut legacy = active.capture_workspace_session_record().unwrap();
    legacy.workbench_settings.ai_provider_configuration_json = None;
    let legacy_json = serde_json::to_string(&legacy).unwrap();
    assert!(!legacy_json.contains("ai_provider_configuration_json"));
    std::fs::write(&path, legacy_json).unwrap();
    let saved = DesktopSessionStore::load(&path).unwrap().unwrap();
    active.restore_workspace_session_record(&saved).unwrap();
    assert!(active.ai_provider_profiles().is_empty());
    assert_eq!(
        active.ai_provider_configuration_json().unwrap(),
        r#"{"profiles":[],"selected":null}"#
    );
    assert_eq!(active.product_mode(), AppProductMode::Manual);
    active.configure_ai_provider_profile(profile()).unwrap();
    assert!(!active.ai_provider_profiles()[0].selected);
    assert_eq!(active.ai_provider_profiles()[0].credential_state, "stored");
}

#[test]
fn malformed_session_memory_does_not_partially_restore_provider_or_workbench_settings() {
    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("main.rs"), "fn main() {}\n").unwrap();
    let mut active = app(root.path(), Arc::new(InMemorySecretStore::default()));
    active.configure_ai_provider_profile(profile()).unwrap();
    active.select_ai_provider_profile("mimo-sgp").unwrap();
    let previous = active.ai_provider_configuration_json().unwrap();
    let before = active
        .shell_projection_snapshot("before")
        .unwrap()
        .settings_projection;
    let mut record = active.capture_workspace_session_record().unwrap();
    record.workbench_settings.ai_provider_configuration_json =
        Some(r#"{"profiles":[],"selected":null}"#.into());
    record.workbench_settings.zoom_percent = 175;
    record.memory_snapshot_json = Some("{".into());
    assert!(active.restore_workspace_session_record(&record).is_err());
    assert_eq!(active.ai_provider_configuration_json().unwrap(), previous);
    assert_eq!(
        active
            .shell_projection_snapshot("after")
            .unwrap()
            .settings_projection,
        before
    );
    assert_eq!(active.product_mode(), AppProductMode::Manual);
}

#[cfg(windows)]
#[test]
fn failed_atomic_profile_publication_keeps_previous_session_bytes_and_reopen_state() {
    use std::os::windows::fs::OpenOptionsExt;
    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("main.rs"), "fn main() {}\n").unwrap();
    let path = root.path().join("session.json");
    let mut original = app(root.path(), Arc::new(InMemorySecretStore::default()));
    original.configure_ai_provider_profile(profile()).unwrap();
    original.select_ai_provider_profile("mimo-sgp").unwrap();
    DesktopSessionStore::save(&path, &original.capture_workspace_session_record().unwrap())
        .unwrap();
    let previous_bytes = std::fs::read(&path).unwrap();
    let mut changed = profile();
    changed.model = "replacement-model".into();
    original.configure_ai_provider_profile(changed).unwrap();
    // FILE_SHARE_READ | FILE_SHARE_WRITE: in-place writes would work, but an
    // atomic rename/replacement must fail while this handle denies DELETE.
    let locked = std::fs::OpenOptions::new()
        .read(true)
        .share_mode(3)
        .open(&path)
        .unwrap();
    assert!(
        DesktopSessionStore::save(&path, &original.capture_workspace_session_record().unwrap())
            .is_err()
    );
    assert_eq!(std::fs::read(&path).unwrap(), previous_bytes);
    let saved = DesktopSessionStore::load(&path).unwrap().unwrap();
    drop(locked);
    let mut reopened = app(root.path(), Arc::new(InMemorySecretStore::default()));
    reopened.restore_workspace_session_record(&saved).unwrap();
    assert_eq!(
        reopened.ai_provider_profiles()[0].profile.model,
        "mimo-v2.6-pro"
    );
    assert!(reopened.ai_provider_profiles()[0].selected);
    assert_eq!(reopened.product_mode(), AppProductMode::Manual);
}
