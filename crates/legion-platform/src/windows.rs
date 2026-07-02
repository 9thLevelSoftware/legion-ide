//! Metadata-only Windows ConPTY parity contract.
//!
//! This module is intentionally platform-neutral: it documents the Windows
//! ConPTY capabilities that the native PTY boundary must surface to terminal
//! callers without granting shell-emitted metadata any security authority.

/// Metadata-only contract for the Windows ConPTY-backed terminal runtime.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowsConptyParityContract {
    /// Native session id prefix used by the platform ConPTY backend.
    pub session_id_prefix: &'static str,
    /// User/debug-visible backend label.
    pub backend_label: &'static str,
    /// Whether native spawn is implemented by this backend.
    pub supports_spawn: bool,
    /// Whether bounded input is implemented by this backend.
    pub supports_input: bool,
    /// Whether resize events are forwarded to ConPTY.
    pub supports_resize: bool,
    /// Whether interrupt is represented distinctly from terminate.
    pub supports_interrupt: bool,
    /// Whether termination is supported.
    pub supports_terminate: bool,
    /// Whether kill-tree escalation is accepted by the backend boundary.
    pub supports_kill_tree: bool,
    /// Whether process exit codes are queryable from polls.
    pub supports_exit_code: bool,
    /// Whether unavailable/fallback paths must be projected to the user.
    pub surfaces_fallback_to_user: bool,
    /// Whether fallback may be denied silently.
    pub silent_fallback_denial: bool,
    /// User-facing status kind for fallback/unavailable ConPTY paths.
    pub fallback_status_kind: &'static str,
    /// Schema version for this metadata contract.
    pub schema_version: u32,
}

impl WindowsConptyParityContract {
    /// Build the metadata-only ConPTY parity contract.
    pub fn metadata_only() -> Self {
        Self {
            session_id_prefix: "native-conpty",
            backend_label: "windows-conpty",
            supports_spawn: true,
            supports_input: true,
            supports_resize: true,
            supports_interrupt: true,
            supports_terminate: true,
            supports_kill_tree: true,
            supports_exit_code: true,
            surfaces_fallback_to_user: true,
            silent_fallback_denial: false,
            fallback_status_kind: "degraded",
            schema_version: 1,
        }
    }
}

/// Return the current metadata-only Windows ConPTY parity contract.
pub fn windows_conpty_parity_contract() -> WindowsConptyParityContract {
    WindowsConptyParityContract::metadata_only()
}
