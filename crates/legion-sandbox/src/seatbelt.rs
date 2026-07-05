//! macOS Seatbelt profile compilation.

use crate::{SandboxBackend, SandboxProfile, SandboxScope};

/// Seatbelt profile wrapper.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SeatbeltProfile {
    /// Compiled sandbox profile.
    pub profile: SandboxProfile,
    /// Seatbelt rules emitted by the compiler.
    pub rules: Vec<String>,
}

impl SeatbeltProfile {
    /// Compiles a Seatbelt profile from a sandbox scope.
    pub fn compile(scope: SandboxScope) -> Self {
        Self {
            profile: SandboxProfile::new(SandboxBackend::Seatbelt, scope)
                .with_note("macOS Seatbelt profile compiled fail-closed"),
            rules: vec![
                "deny default".to_string(),
                "allow file-write* only within workspace root".to_string(),
                "allow network-outbound only for allowlisted destinations".to_string(),
            ],
        }
    }
}
