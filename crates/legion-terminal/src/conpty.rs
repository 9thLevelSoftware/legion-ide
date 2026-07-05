//! Terminal-facing Windows ConPTY parity metadata.
//!
//! The native platform backend owns process and ConPTY handles. This module
//! only exposes renderer/app-safe capability metadata so unavailable Windows
//! fallbacks can be surfaced instead of silently denied.

/// Terminal-facing ConPTY parity contract.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConptyParityContract {
    /// User/debug-visible backend label.
    pub backend_label: &'static str,
    /// Whether terminal input is expected to work through this backend.
    pub supports_input: bool,
    /// Whether terminal resize is expected to work through this backend.
    pub supports_resize: bool,
    /// Whether command exit codes are projected from polls.
    pub supports_exit_code: bool,
    /// Whether fallback/unavailable states must be visible to the user.
    pub surfaces_fallback_to_user: bool,
    /// Whether shell-emitted metadata is a security-policy authority.
    pub shell_metadata_is_policy_authority: bool,
    /// Schema version for this metadata contract.
    pub schema_version: u32,
}

impl ConptyParityContract {
    /// Build the metadata-only terminal-facing ConPTY contract.
    pub fn metadata_only() -> Self {
        let platform = legion_platform::windows::windows_conpty_parity_contract();
        Self {
            backend_label: platform.backend_label,
            supports_input: platform.supports_input,
            supports_resize: platform.supports_resize,
            supports_exit_code: platform.supports_exit_code,
            surfaces_fallback_to_user: platform.surfaces_fallback_to_user,
            shell_metadata_is_policy_authority: false,
            schema_version: 1,
        }
    }
}

/// Return the terminal-facing Windows ConPTY parity metadata contract.
pub fn conpty_parity_contract() -> ConptyParityContract {
    ConptyParityContract::metadata_only()
}
