use legion_platform::windows::{WindowsConptyParityContract, windows_conpty_parity_contract};

#[test]
fn windows_conpty_contract_matches_native_pty_lifecycle_expectations() {
    let contract = windows_conpty_parity_contract();

    assert_eq!(contract.session_id_prefix, "native-conpty");
    assert_eq!(contract.backend_label, "windows-conpty");
    assert!(contract.supports_spawn);
    assert!(contract.supports_input);
    assert!(contract.supports_resize);
    assert!(contract.supports_interrupt);
    assert!(contract.supports_terminate);
    assert!(contract.supports_kill_tree);
    assert!(contract.supports_exit_code);
    assert!(contract.surfaces_fallback_to_user);
    assert_eq!(contract.schema_version, 1);
}

#[test]
fn windows_conpty_contract_documents_fallback_as_user_visible_not_silent_denial() {
    let contract = WindowsConptyParityContract::metadata_only();

    assert!(contract.surfaces_fallback_to_user);
    assert!(!contract.silent_fallback_denial);
    assert_eq!(contract.fallback_status_kind, "degraded");
}
