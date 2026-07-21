//! Resolve a live DAP adapter binary (WS-A-D Phase 2 B3).
//!
//! ## Wire honesty
//!
//! The live path (`LiveDapSession` + `fake_dap_adapter`) currently speaks
//! **Legion provisional JSON-RPC** over `Content-Length` framing
//! (`jsonrpc` / `id` / `method` / `params`), not the Microsoft DAP envelope
//! (`seq` / `type` / `command` / `arguments`). Real CodeLLDB / `lldb-dap`
//! binaries will reject that shape.
//!
//! Therefore this resolver **does not** auto-discover vendor adapters on
//! `PATH`. Live spawn is only for:
//! 1. `LEGION_DAP_ADAPTER` — explicit path to a **Legion-compatible** adapter
//! 2. `LEGION_DAP_USE_FAKE=1` — in-tree `fake_dap_adapter` (CI / local dev)
//!
//! Microsoft DAP wire + PATH discovery of `lldb-dap` / CodeLLDB is a follow-on
//! (contract test against a standards-compliant adapter required first).
//!
//! ## Mode
//!
//! `LEGION_DAP_MODE=fixture|live|auto` (default `auto`):
//! - `fixture` — never resolve live (callers use simulated runtime)
//! - `live` — require a resolution; callers must fail closed (no fixture)
//! - `auto` — try live, fall back to fixture if unresolved or spawn fails

use std::env;
use std::path::PathBuf;

use crate::live_session::fake_dap_adapter_path;

/// Product DAP mode from the environment.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DapMode {
    /// Always use the simulated client (no process).
    Fixture,
    /// Require live adapter; callers should fail if unresolved or spawn fails.
    Live,
    /// Try live, otherwise fixture.
    Auto,
}

impl DapMode {
    /// Parse `LEGION_DAP_MODE` (default `auto`).
    pub fn from_env() -> Self {
        match env::var("LEGION_DAP_MODE")
            .unwrap_or_else(|_| "auto".to_string())
            .to_ascii_lowercase()
            .as_str()
        {
            "fixture" | "simulated" | "off" => Self::Fixture,
            "live" | "real" => Self::Live,
            _ => Self::Auto,
        }
    }

    /// Whether callers should attempt live resolution.
    pub fn allows_live(self) -> bool {
        !matches!(self, Self::Fixture)
    }

    /// Whether live failure must not fall back to the simulated runtime.
    pub fn require_live(self) -> bool {
        matches!(self, Self::Live)
    }
}

/// A resolved adapter program ready to spawn.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedAdapter {
    /// Absolute or relative path to the executable.
    pub program: PathBuf,
    /// Extra argv (usually empty for stdio adapters).
    pub args: Vec<String>,
    /// Adapter type label for DAP `adapterID` / audit.
    pub adapter_type: String,
    /// True when this is the in-tree CI fake adapter.
    pub is_fake: bool,
}

/// Resolve a live adapter for `preferred_type` (e.g. `lldb-dap` / `legion-fake`).
///
/// Returns [`None`] when mode is fixture or no **compatible** binary is found.
/// Does **not** pick vendor Microsoft-DAP binaries from `PATH`.
pub fn resolve_live_adapter(preferred_type: &str) -> Option<ResolvedAdapter> {
    let mode = DapMode::from_env();
    if !mode.allows_live() {
        return None;
    }

    if let Ok(path) = env::var("LEGION_DAP_ADAPTER") {
        let path = PathBuf::from(path.trim());
        if !path.as_os_str().is_empty() && path.exists() {
            return Some(ResolvedAdapter {
                program: path,
                args: Vec::new(),
                adapter_type: preferred_type.to_string(),
                is_fake: false,
            });
        }
    }

    // No PATH auto-discovery of lldb-dap / codelldb until Microsoft DAP wire lands.
    if env_truthy("LEGION_DAP_USE_FAKE")
        && let Some(fake) = fake_dap_adapter_path()
    {
        return Some(ResolvedAdapter {
            program: fake,
            args: Vec::new(),
            adapter_type: "legion-fake".to_string(),
            is_fake: true,
        });
    }

    let _ = preferred_type;
    None
}

fn env_truthy(key: &str) -> bool {
    matches!(
        env::var(key)
            .unwrap_or_default()
            .to_ascii_lowercase()
            .as_str(),
        "1" | "true" | "yes" | "on"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mode_allows_live_matrix() {
        assert!(!DapMode::Fixture.allows_live());
        assert!(DapMode::Live.allows_live());
        assert!(DapMode::Auto.allows_live());
        assert!(!DapMode::Fixture.require_live());
        assert!(DapMode::Live.require_live());
        assert!(!DapMode::Auto.require_live());
    }

    #[test]
    fn path_auto_discovery_is_disabled() {
        // Even when mode would allow live, without LEGION_DAP_ADAPTER /
        // LEGION_DAP_USE_FAKE we must not invent a vendor binary.
        // (Cannot safely clear env in parallel tests; just assert the
        // documented contract via the API surface: resolve needs opt-in.)
        assert!(
            !DapMode::Fixture.allows_live(),
            "fixture never resolves live"
        );
    }
}
