//! Resolve a live DAP adapter binary (WS-A-D Phase 2 B3).
//!
//! Order (first hit wins when mode allows live):
//! 1. `LEGION_DAP_ADAPTER` — explicit path to adapter executable
//! 2. `PATH` lookup for type-specific names (`lldb-dap`, `codelldb`, …)
//! 3. In-tree `fake_dap_adapter` when `LEGION_DAP_USE_FAKE=1` (CI / local dev)
//!
//! `LEGION_DAP_MODE=fixture|live|auto` (default `auto`):
//! - `fixture` — never resolve live (callers use simulated runtime)
//! - `live` — require a real resolution or error at call site
//! - `auto` — try live, fall back to fixture if unresolved

use std::env;
use std::path::{Path, PathBuf};

use crate::live_session::fake_dap_adapter_path;

/// Product DAP mode from the environment.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DapMode {
    /// Always use the simulated client (no process).
    Fixture,
    /// Prefer live adapter; callers should fail if unresolved.
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

/// Resolve a live adapter for `preferred_type` (e.g. `lldb-dap`).
///
/// Returns [`None`] when mode is fixture or no binary is found.
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

    for candidate in path_candidates(preferred_type) {
        if let Some(found) = find_on_path(&candidate) {
            return Some(ResolvedAdapter {
                program: found,
                args: Vec::new(),
                adapter_type: preferred_type.to_string(),
                is_fake: false,
            });
        }
    }

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

    None
}

fn path_candidates(preferred_type: &str) -> Vec<String> {
    let mut names = Vec::new();
    let t = preferred_type.to_ascii_lowercase();
    names.push(preferred_type.to_string());
    if t.contains("lldb") {
        names.extend([
            "lldb-dap".to_string(),
            "lldb-vscode".to_string(),
            "codelldb".to_string(),
        ]);
    } else if t.contains("code") {
        names.push("codelldb".to_string());
    }
    names.sort();
    names.dedup();
    names
}

fn find_on_path(name: &str) -> Option<PathBuf> {
    let path_var = env::var_os("PATH")?;
    for dir in env::split_paths(&path_var) {
        let mut candidate = dir.join(name);
        if candidate.is_file() {
            return Some(candidate);
        }
        if cfg!(windows) {
            candidate.set_extension("exe");
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }
    // Also accept bare name when it exists as relative path.
    let bare = Path::new(name);
    if bare.is_file() {
        return Some(bare.to_path_buf());
    }
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
    }
}
