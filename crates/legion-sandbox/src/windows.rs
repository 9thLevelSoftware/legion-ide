//! Windows restricted-token / AppContainer sandbox profile compilation.

use crate::{SandboxBackend, SandboxError, SandboxProfile, SandboxScope};

/// Windows sandbox profile wrapper.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowsProfile {
    /// Compiled sandbox profile.
    pub profile: SandboxProfile,
    /// Human-readable Windows enforcement notes.
    pub notes: Vec<String>,
    /// Honest fallback message when stronger host APIs are unavailable.
    pub documented_fallback: Option<String>,
}

impl WindowsProfile {
    /// Compiles a Windows sandbox profile.
    ///
    /// The profile stays explicit about weaker guarantees instead of silently
    /// dropping to no sandbox.
    pub fn compile(scope: SandboxScope) -> Result<Self, SandboxError> {
        let (backend, note) = if cfg!(windows) {
            (
                SandboxBackend::RestrictedToken,
                "Windows restricted-token profile compiled fail-closed",
            )
        } else {
            (
                SandboxBackend::DocumentedFallback {
                    reason: "Windows-specific sandbox APIs are unavailable on this host"
                        .to_string(),
                },
                "Windows sandboxing uses an explicit documented fallback on non-Windows hosts",
            )
        };

        Ok(Self {
            profile: SandboxProfile::new(backend, scope).with_note(note),
            notes: vec![
                "restricted token / AppContainer style enforcement".to_string(),
                "filesystem scope limited to workspace root".to_string(),
                "egress remains allowlist-based and audited".to_string(),
            ],
            documented_fallback: Some(
                "Windows sandboxing is explicit about weaker guarantees when host APIs are unavailable"
                    .to_string(),
            ),
        })
    }

    /// Returns the explicit fallback message used by the documentation surface.
    pub fn fallback_message() -> &'static str {
        "Windows sandboxing uses restricted token / AppContainer-style constraints or an explicitly documented weaker fallback; it never silently becomes no sandbox."
    }
}
