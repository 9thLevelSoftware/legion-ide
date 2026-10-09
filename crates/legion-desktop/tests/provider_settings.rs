use legion_app::AiProviderProfile;
use legion_desktop::bridge::{DesktopAction, SensitiveString};
use legion_desktop::workflow::{DesktopLaunchConfig, DesktopRuntime, DesktopWorkflowOutcome};
use legion_storage::{InMemorySecretStore, SecretReference, SecretStore, SecretStoreError};
use std::sync::Arc;
mod common;
use common::{click_at, clickable_center, full_frame_input, rendered_text};
use legion_desktop::workflow::DesktopEframeApp;

fn require(output: &egui::FullOutput, label: &str) -> egui::Pos2 {
    clickable_center(output, label).unwrap_or_else(|| panic!("missing control: {label}"))
}

fn providers_page(app: &mut DesktopEframeApp) -> egui::FullOutput {
    let frame = app.run_headless_full_frame(full_frame_input(Vec::new()));
    let frame = click_at(app, require(&frame, "Settings"));
    click_at(app, require(&frame, "AI Providers"))
}

fn fill_profile_field(app: &mut DesktopEframeApp, label: &str, value: &str) -> egui::FullOutput {
    let frame = app.run_headless_full_frame(full_frame_input(Vec::new()));
    let nodes = &frame
        .platform_output
        .accesskit_update
        .as_ref()
        .unwrap()
        .nodes;
    let label_ids: Vec<_> = nodes
        .iter()
        .filter(|(_, node)| node.value() == Some(label) || node.label() == Some(label))
        .map(|(id, _)| *id)
        .collect();
    let bounds = nodes
        .iter()
        .find_map(|(_, node)| {
            (node.role() == egui::accesskit::Role::TextInput
                && node.labelled_by().iter().any(|id| label_ids.contains(id)))
            .then(|| node.bounds())
            .flatten()
        })
        .unwrap_or_else(|| {
            panic!(
                "label {label} must identify an editable metadata field: {:?}",
                nodes
                    .iter()
                    .filter(|(_, n)| n.role() == egui::accesskit::Role::TextInput
                        || n.value() == Some(label)
                        || n.label() == Some(label))
                    .collect::<Vec<_>>()
            )
        });
    click_at(
        app,
        egui::pos2(
            ((bounds.x0 + bounds.x1) / 2.0) as f32,
            ((bounds.y0 + bounds.y1) / 2.0) as f32,
        ),
    );
    common::press_key(
        app,
        egui::Key::A,
        egui::Modifiers {
            ctrl: true,
            command: true,
            ..egui::Modifiers::default()
        },
    );
    app.run_headless_full_frame(full_frame_input(vec![egui::Event::Text(value.into())]))
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

fn runtime(root: &std::path::Path, store: Arc<dyn SecretStore + Send + Sync>) -> DesktopRuntime {
    DesktopRuntime::open_with_provider_secret_store(
        DesktopLaunchConfig::new(root.to_path_buf(), None)
            .with_session_state(root.join("session.json")),
        store,
    )
    .unwrap()
}

#[test]
fn named_profile_actions_persist_explicit_selection_without_leaving_manual() {
    let root = tempfile::tempdir().unwrap();
    let store = Arc::new(InMemorySecretStore::default());
    let mut runtime = runtime(root.path(), store.clone());
    assert_eq!(
        runtime
            .handle_action(DesktopAction::ConfigureAiProviderProfile { profile: profile() })
            .unwrap(),
        DesktopWorkflowOutcome::Noop
    );
    assert_eq!(
        runtime
            .handle_action(DesktopAction::SelectAiProviderProfile {
                name: "mimo-sgp".into(),
            })
            .unwrap(),
        DesktopWorkflowOutcome::Noop
    );
    let record = runtime.capture_session_record().unwrap();
    let metadata = record
        .workbench_settings
        .ai_provider_configuration_json
        .unwrap();
    assert!(metadata.contains("mimo-v2.6-pro"));
    assert!(metadata.contains("\"selected\":\"mimo-sgp\""));
    let mut reopened = DesktopRuntime::open_with_provider_secret_store(
        DesktopLaunchConfig::new(root.path().to_path_buf(), None)
            .with_session_state(root.path().join("session.json")),
        store,
    )
    .unwrap();
    assert_eq!(
        reopened
            .app_mut_for_test()
            .ai_provider_configuration_json()
            .unwrap(),
        metadata
    );
    assert_eq!(
        reopened.projection_snapshot().product_mode,
        legion_ui::DockMode::Manual
    );
    assert!(!reopened.product_ai_stream_in_flight());
}

#[test]
fn credential_actions_bind_to_the_displayed_route_and_never_persist_the_key() {
    let root = tempfile::tempdir().unwrap();
    let mut runtime = runtime(root.path(), Arc::new(InMemorySecretStore::default()));
    runtime
        .handle_action(DesktopAction::ConfigureAiProviderProfile { profile: profile() })
        .unwrap();
    let stale_action = DesktopAction::ReplaceAiProfileCredential {
        expected_profile: profile(),
        credential: SensitiveString("synthetic-settings-secret".into()),
    };
    assert!(!format!("{stale_action:?}").contains("synthetic-settings-secret"));
    let mut changed = profile();
    changed.model = "owner-selected-replacement".into();
    runtime
        .handle_action(DesktopAction::ConfigureAiProviderProfile {
            profile: changed.clone(),
        })
        .unwrap();
    assert!(matches!(
        runtime.handle_action(stale_action).unwrap(),
        DesktopWorkflowOutcome::Error(_)
    ));
    assert_eq!(
        runtime.app_mut_for_test().ai_provider_profiles()[0].credential_state,
        "missing"
    );
    assert_eq!(
        runtime
            .handle_action(DesktopAction::ReplaceAiProfileCredential {
                expected_profile: changed.clone(),
                credential: SensitiveString("synthetic-settings-secret".into()),
            })
            .unwrap(),
        DesktopWorkflowOutcome::Noop
    );
    assert_eq!(
        runtime.app_mut_for_test().ai_provider_profiles()[0].credential_state,
        "stored"
    );
    assert!(
        !std::fs::read_to_string(root.path().join("session.json"))
            .unwrap()
            .contains("synthetic-settings-secret")
    );
    assert!(!format!("{:?}", runtime.projection_snapshot()).contains("synthetic-settings-secret"));
    assert!(matches!(
        runtime
            .handle_action(DesktopAction::RevokeAiProfileCredential {
                expected_profile: profile(),
            })
            .unwrap(),
        DesktopWorkflowOutcome::Error(_)
    ));
    assert_eq!(
        runtime.app_mut_for_test().ai_provider_profiles()[0].credential_state,
        "stored"
    );
    assert_eq!(
        runtime
            .handle_action(DesktopAction::RevokeAiProfileCredential {
                expected_profile: changed,
            })
            .unwrap(),
        DesktopWorkflowOutcome::Noop
    );
    assert_eq!(
        runtime.app_mut_for_test().ai_provider_profiles()[0].credential_state,
        "missing"
    );
    assert_eq!(
        runtime.projection_snapshot().product_mode,
        legion_ui::DockMode::Manual
    );
}

#[test]
fn named_profile_settings_render_exact_route_and_masked_key_without_editor_input() {
    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("target.txt"), "original contents\n").unwrap();
    let mut runtime = runtime(root.path(), Arc::new(InMemorySecretStore::default()));
    runtime
        .app_mut_for_test()
        .open_file(root.path().join("target.txt").to_string_lossy())
        .unwrap();
    runtime
        .handle_action(DesktopAction::ConfigureAiProviderProfile { profile: profile() })
        .unwrap();
    runtime
        .handle_action(DesktopAction::SelectAiProviderProfile {
            name: "mimo-sgp".into(),
        })
        .unwrap();
    let mut app = DesktopEframeApp::new(runtime);
    let frame = providers_page(&mut app);
    let text = rendered_text(&frame).join("\n");
    assert!(
        text.contains("https://token-plan-sgp.xiaomimimo.com/v1"),
        "{text}"
    );
    assert!(text.contains("mimo-v2.6-pro"));
    assert!(text.contains("not checked"));
    assert!(text.contains("remote"));
    assert!(text.contains("inline via chat (unqualified)"));
    require(&frame, "Replace profile key");
    let field = frame
        .platform_output
        .accesskit_update
        .as_ref()
        .unwrap()
        .nodes
        .iter()
        .find_map(|(_, node)| {
            (node.role() == egui::accesskit::Role::PasswordInput)
                .then(|| node.bounds())
                .flatten()
        })
        .expect("a masked password control must be accessible");
    click_at(
        &mut app,
        egui::pos2(
            ((field.x0 + field.x1) / 2.0) as f32,
            ((field.y0 + field.y1) / 2.0) as f32,
        ),
    );
    let before = app
        .runtime_snapshot()
        .active_buffer_projection
        .small_buffer_preview;
    let frame = app.run_headless_full_frame(full_frame_input(vec![egui::Event::Text(
        "synthetic-masked-key".into(),
    )]));
    assert_eq!(
        app.runtime_snapshot()
            .active_buffer_projection
            .small_buffer_preview,
        before
    );
    assert!(
        !rendered_text(&frame)
            .join("\n")
            .contains("synthetic-masked-key")
    );
    click_at(&mut app, require(&frame, "Replace profile key"));
    assert_eq!(
        app.runtime_mut_for_test()
            .app_mut_for_test()
            .ai_provider_profiles()[0]
            .credential_state,
        "stored",
        "submission must prove typing reached the password field"
    );
    let frame = app.run_headless_full_frame(full_frame_input(Vec::new()));
    assert!(
        rendered_text(&frame)
            .join("\n")
            .contains("Credential: stored")
    );
    click_at(&mut app, require(&frame, "Revoke profile key"));
    assert_eq!(
        app.runtime_mut_for_test()
            .app_mut_for_test()
            .ai_provider_profiles()[0]
            .credential_state,
        "missing"
    );
    assert_eq!(
        app.runtime_snapshot().product_mode,
        legion_ui::DockMode::Manual
    );
}

#[test]
fn named_selection_hides_legacy_choices_and_never_claims_auto_discovery() {
    let root = tempfile::tempdir().unwrap();
    let mut runtime = runtime(root.path(), Arc::new(InMemorySecretStore::default()));
    runtime
        .handle_action(DesktopAction::ConfigureAiProviderProfile { profile: profile() })
        .unwrap();
    let mut app = DesktopEframeApp::new(runtime);
    let frame = providers_page(&mut app);
    require(&frame, "Fixture");
    assert!(
        !rendered_text(&frame)
            .join("\n")
            .contains("Auto uses providers on this computer")
    );
    click_at(&mut app, require(&frame, "Select profile"));
    let frame = app.run_headless_full_frame(full_frame_input(Vec::new()));
    assert!(
        clickable_center(&frame, "Fixture").is_none(),
        "a selected named route must not display conflicting legacy choices"
    );
    assert!(clickable_center(&frame, "Anthropic").is_none());
    assert!(
        app.runtime_mut_for_test()
            .app_mut_for_test()
            .ai_provider_profiles()[0]
            .selected
    );
    app.runtime_mut_for_test()
        .handle_action(DesktopAction::SetPreferredAiProvider {
            provider_id: "deterministic".into(),
        })
        .unwrap();
    let frame = app.run_headless_full_frame(full_frame_input(Vec::new()));
    require(&frame, "Fixture");
    assert!(
        !app.runtime_mut_for_test()
            .app_mut_for_test()
            .ai_provider_profiles()[0]
            .selected
    );
    assert!(
        app.runtime_mut_for_test()
            .capture_session_record()
            .unwrap()
            .workbench_settings
            .ai_provider_configuration_json
            .unwrap()
            .contains("\"selected\":null")
    );
}

#[test]
fn rendered_profile_form_adds_and_edits_exact_metadata_without_auto_selection() {
    let root = tempfile::tempdir().unwrap();
    let mut app = DesktopEframeApp::new(runtime(
        root.path(),
        Arc::new(InMemorySecretStore::default()),
    ));
    let frame = providers_page(&mut app);
    click_at(&mut app, require(&frame, "Add profile"));
    fill_profile_field(&mut app, "Profile name", "mimo-sgp");
    fill_profile_field(
        &mut app,
        "Base endpoint",
        "https://token-plan-sgp.xiaomimimo.com/v1",
    );
    let frame = fill_profile_field(&mut app, "Model", "mimo-v2.6-pro");
    // Token limits include reasoning; the owner explicitly disables thinking for this pilot.
    click_at(
        &mut app,
        require(&frame, "Disable thinking (endpoint must support this)"),
    );
    let frame = app.run_headless_full_frame(full_frame_input(Vec::new()));
    click_at(&mut app, require(&frame, "Save profile"));
    let profiles = app
        .runtime_mut_for_test()
        .app_mut_for_test()
        .ai_provider_profiles();
    assert_eq!(profiles[0].profile, profile());
    assert!(!profiles[0].selected, "adding metadata is not activation");
    let frame = app.run_headless_full_frame(full_frame_input(Vec::new()));
    click_at(&mut app, require(&frame, "Close profile form"));
    let frame = app.run_headless_full_frame(full_frame_input(Vec::new()));
    click_at(&mut app, require(&frame, "Select profile"));
    let frame = app.run_headless_full_frame(full_frame_input(Vec::new()));
    click_at(&mut app, require(&frame, "Edit profile"));
    let frame = fill_profile_field(&mut app, "Model", "owner-selected-replacement");
    click_at(&mut app, require(&frame, "Save profile"));
    let profiles = app
        .runtime_mut_for_test()
        .app_mut_for_test()
        .ai_provider_profiles();
    assert_eq!(profiles[0].profile.model, "owner-selected-replacement");
    assert_eq!(profiles[0].health, "not checked");
    assert!(profiles[0].selected);
    assert_eq!(
        app.runtime_snapshot().product_mode,
        legion_ui::DockMode::Manual
    );
}

#[derive(Default)]
struct ObservableKeyring {
    reads: std::sync::atomic::AtomicUsize,
    unavailable: std::sync::atomic::AtomicBool,
}

impl SecretStore for ObservableKeyring {
    fn load(&self, _: &SecretReference) -> Result<Option<String>, SecretStoreError> {
        self.reads.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        if self.unavailable.load(std::sync::atomic::Ordering::SeqCst) {
            Err(SecretStoreError::KeyringFailure {
                message: "synthetic-untrusted-keyring-error".into(),
            })
        } else {
            Ok(None)
        }
    }
    fn store(&self, _: &SecretReference, _: &str) -> Result<(), SecretStoreError> {
        Err(SecretStoreError::KeyringFailure {
            message: "synthetic-untrusted-keyring-error".into(),
        })
    }
    fn delete(&self, _: &SecretReference) -> Result<(), SecretStoreError> {
        Ok(())
    }
}

#[test]
fn status_refresh_is_explicit_and_paint_does_not_read_secure_credentials() {
    use std::sync::atomic::Ordering::SeqCst;
    let root = tempfile::tempdir().unwrap();
    let store = Arc::new(ObservableKeyring::default());
    let mut runtime = runtime(root.path(), store.clone());
    runtime
        .handle_action(DesktopAction::ConfigureAiProviderProfile { profile: profile() })
        .unwrap();
    runtime
        .handle_action(DesktopAction::SelectAiProviderProfile {
            name: "mimo-sgp".into(),
        })
        .unwrap();
    let mut app = DesktopEframeApp::new(runtime);
    let before = store.reads.load(SeqCst);
    let frame = providers_page(&mut app);
    assert_eq!(
        store.reads.load(SeqCst),
        before,
        "painting/settings navigation must not block on the keyring"
    );
    store.unavailable.store(true, SeqCst);
    click_at(&mut app, require(&frame, "Refresh profile status"));
    assert!(store.reads.load(SeqCst) > before);
    let frame = app.run_headless_full_frame(full_frame_input(Vec::new()));
    let text = rendered_text(&frame).join("\n");
    assert!(text.contains("Credential: unavailable"));
    assert!(!text.contains("synthetic-untrusted-keyring-error"));
    let outcome = app
        .runtime_mut_for_test()
        .handle_action(DesktopAction::ReplaceAiProfileCredential {
            expected_profile: profile(),
            credential: SensitiveString("synthetic-key-to-refuse".into()),
        })
        .unwrap();
    assert!(matches!(outcome, DesktopWorkflowOutcome::Error(_)));
    assert!(!format!("{outcome:?}").contains("synthetic-untrusted-keyring-error"));
    assert!(!format!("{:?}", app.runtime_snapshot()).contains("synthetic-key-to-refuse"));
    assert_eq!(
        app.runtime_snapshot().product_mode,
        legion_ui::DockMode::Manual
    );
}

#[test]
fn closing_settings_discards_the_masked_key_and_undo_cannot_restore_it() {
    let root = tempfile::tempdir().unwrap();
    let mut runtime = runtime(root.path(), Arc::new(InMemorySecretStore::default()));
    runtime
        .handle_action(DesktopAction::ConfigureAiProviderProfile { profile: profile() })
        .unwrap();
    runtime
        .handle_action(DesktopAction::SelectAiProviderProfile {
            name: "mimo-sgp".into(),
        })
        .unwrap();
    let mut app = DesktopEframeApp::new(runtime);
    let frame = providers_page(&mut app);
    let field = frame
        .platform_output
        .accesskit_update
        .as_ref()
        .unwrap()
        .nodes
        .iter()
        .find_map(|(_, node)| {
            (node.role() == egui::accesskit::Role::PasswordInput)
                .then(|| node.bounds())
                .flatten()
        })
        .unwrap();
    let field_center = egui::pos2(
        ((field.x0 + field.x1) / 2.0) as f32,
        ((field.y0 + field.y1) / 2.0) as f32,
    );
    click_at(&mut app, field_center);
    let frame = app.run_headless_full_frame(full_frame_input(vec![egui::Event::Text(
        "synthetic-discard-key".into(),
    )]));
    assert!(common::enabled_clickable_center(&frame, "Replace profile key").is_some());
    click_at(&mut app, require(&frame, "Close Settings"));
    let frame = providers_page(&mut app);
    assert!(common::enabled_clickable_center(&frame, "Replace profile key").is_none());
    click_at(&mut app, field_center);
    let frame = common::press_key(
        &mut app,
        egui::Key::Z,
        egui::Modifiers {
            ctrl: true,
            command: true,
            ..egui::Modifiers::default()
        },
    );
    assert!(common::enabled_clickable_center(&frame, "Replace profile key").is_none());
    assert_eq!(
        app.runtime_mut_for_test()
            .app_mut_for_test()
            .ai_provider_profiles()[0]
            .credential_state,
        "missing"
    );
}

#[cfg(windows)]
#[test]
fn invalid_or_unsaved_profile_actions_report_failure_and_preserve_saved_metadata() {
    use std::os::windows::fs::OpenOptionsExt;
    let root = tempfile::tempdir().unwrap();
    let mut runtime = runtime(root.path(), Arc::new(InMemorySecretStore::default()));
    runtime
        .handle_action(DesktopAction::ConfigureAiProviderProfile { profile: profile() })
        .unwrap();
    let path = root.path().join("session.json");
    let previous = std::fs::read(&path).unwrap();
    let mut invalid = profile();
    invalid.endpoint = "https://example.com/v1?api_key=synthetic-forbidden-key".into();
    let refused = runtime
        .handle_action(DesktopAction::ConfigureAiProviderProfile { profile: invalid })
        .unwrap();
    assert!(matches!(refused, DesktopWorkflowOutcome::Error(_)));
    assert_eq!(
        runtime.app_mut_for_test().ai_provider_profiles()[0].profile,
        profile()
    );
    assert_eq!(std::fs::read(&path).unwrap(), previous);
    assert!(!format!("{:?}", runtime.projection_snapshot()).contains("synthetic-forbidden-key"));
    // READ/WRITE sharing allows an unsafe in-place rewrite but refuses replacement.
    let lock = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .share_mode(3)
        .open(&path)
        .unwrap();
    let mut changed = profile();
    changed.model = "owner-selected-replacement".into();
    let unsaved = runtime
        .handle_action(DesktopAction::ConfigureAiProviderProfile {
            profile: changed.clone(),
        })
        .unwrap();
    assert!(
        matches!(&unsaved, DesktopWorkflowOutcome::Error(message) if message.contains("could not be saved"))
    );
    assert_eq!(std::fs::read(&path).unwrap(), previous);
    assert_eq!(
        runtime.app_mut_for_test().ai_provider_profiles()[0].profile,
        changed
    );
    drop(lock);
    assert_eq!(
        runtime
            .handle_action(DesktopAction::ConfigureAiProviderProfile { profile: changed })
            .unwrap(),
        DesktopWorkflowOutcome::Noop
    );
    assert_ne!(std::fs::read(&path).unwrap(), previous);
}
