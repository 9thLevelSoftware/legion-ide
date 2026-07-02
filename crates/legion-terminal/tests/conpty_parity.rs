use legion_terminal::conpty::{ConptyParityContract, conpty_parity_contract};

#[test]
fn conpty_parity_contract_surfaces_backend_and_fallback_metadata() {
    let contract = conpty_parity_contract();

    assert_eq!(contract.backend_label, "windows-conpty");
    assert!(contract.supports_resize);
    assert!(contract.supports_input);
    assert!(contract.supports_exit_code);
    assert!(contract.surfaces_fallback_to_user);
    assert_eq!(contract.schema_version, 1);
}

#[test]
fn conpty_contract_is_metadata_only_and_does_not_grant_policy_authority() {
    let contract = ConptyParityContract::metadata_only();

    assert_eq!(contract.backend_label, "windows-conpty");
    assert!(!contract.shell_metadata_is_policy_authority);
    assert!(contract.surfaces_fallback_to_user);
}
