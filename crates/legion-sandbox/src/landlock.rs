//! Linux bubblewrap + Landlock sandbox profile compilation.

use crate::{SandboxBackend, SandboxProfile, SandboxScope};

/// Linux sandbox profile wrapper.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LandlockProfile {
    /// Compiled sandbox profile.
    pub profile: SandboxProfile,
    /// Human-readable Linux enforcement notes.
    pub notes: Vec<String>,
}

impl LandlockProfile {
    /// Compiles a Linux sandbox profile that expects bubblewrap plus Landlock.
    pub fn compile(scope: SandboxScope) -> Self {
        Self {
            profile: SandboxProfile::new(SandboxBackend::BubblewrapLandlock, scope)
                .with_note("Linux bubblewrap + Landlock profile compiled fail-closed"),
            notes: vec![
                "bwrap --unshare-net".to_string(),
                "bubblewrap writable root limited to workspace".to_string(),
                "Landlock write rules deny paths outside workspace".to_string(),
            ],
        }
    }
}
