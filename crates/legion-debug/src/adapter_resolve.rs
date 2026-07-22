//! Resolve a live DAP adapter binary (WS-A-D Phase 2 B3/B4/B9).
//!
//! ## Wire
//!
//! Live path (`LiveDapSession` + `fake_dap_adapter`) speaks **Microsoft DAP**
//! over `Content-Length` framing (`seq` / `type` / `command` / `arguments`).
//! Real CodeLLDB / `lldb-dap` share this envelope; contract coverage is the
//! in-tree fake adapter (B4). Optional system-adapter dogfood is B9
//! ([`resolve_system_adapter`]).
//!
//! ## Resolution order (first hit when mode allows live)
//!
//! 1. `LEGION_DAP_ADAPTER` — explicit path to adapter executable
//! 2. `PATH` lookup for type-specific names (`lldb-dap`, `codelldb`, …)
//!    (preferred type first, then aliases — not alphabetical demotion)
//! 3. In-tree `fake_dap_adapter` when `LEGION_DAP_USE_FAKE=1` (CI / local dev)
//!
//! ## Mode
//!
//! `LEGION_DAP_MODE=fixture|live|auto` (default `auto`):
//! - `fixture` — never resolve live (callers use simulated runtime)
//! - `live` — require a resolution; callers must fail closed (no fixture)
//! - `auto` — try live, fall back to fixture if unresolved or spawn fails
//!
//! ## Dogfood
//!
//! Set `LEGION_DAP_DOGFOOD=1` on the optional system-adapter handshake test to
//! **require** a real adapter (fail if missing). Without it, the test skips
//! when no system binary is present so CI stays green.

use std::env;
use std::path::{Path, PathBuf};

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
/// Returns [`None`] when mode is fixture or no binary is found.
pub fn resolve_live_adapter(preferred_type: &str) -> Option<ResolvedAdapter> {
    let mode = DapMode::from_env();
    if !mode.allows_live() {
        return None;
    }

    if let Some(system) = resolve_system_adapter(preferred_type) {
        return Some(system);
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

/// Resolve a **system** (non-fake) adapter for dogfood / product live paths.
///
/// Order (independent of `LEGION_DAP_MODE` and `LEGION_DAP_USE_FAKE`):
/// 1. `LEGION_DAP_ADAPTER` when the path exists
/// 2. `PATH` candidates for `preferred_type` (preferred name first)
///
/// Never returns the in-tree `fake_dap_adapter`.
pub fn resolve_system_adapter(preferred_type: &str) -> Option<ResolvedAdapter> {
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
            // Prefer the found binary stem so audit rows match the real tool.
            let adapter_type = found
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or(preferred_type)
                .to_string();
            return Some(ResolvedAdapter {
                program: found,
                args: Vec::new(),
                adapter_type,
                is_fake: false,
            });
        }
    }

    None
}

/// Whether dogfood tests should fail closed when no system adapter is present.
pub fn dogfood_requires_system_adapter() -> bool {
    env_truthy("LEGION_DAP_DOGFOOD")
}

fn path_candidates(preferred_type: &str) -> Vec<String> {
    // Preserve preference order: preferred type first, then aliases. Do not
    // alphabetically re-sort — that demoted `lldb-dap` behind `codelldb`.
    let mut names = Vec::new();
    let t = preferred_type.to_ascii_lowercase();
    push_unique(&mut names, preferred_type.to_string());
    if t.contains("lldb") {
        for alias in ["lldb-dap", "lldb-vscode", "codelldb"] {
            push_unique(&mut names, alias.to_string());
        }
    } else if t.contains("code") {
        push_unique(&mut names, "codelldb".to_string());
    }
    names
}

fn push_unique(names: &mut Vec<String>, name: String) {
    if !names
        .iter()
        .any(|existing| existing.eq_ignore_ascii_case(&name))
    {
        names.push(name);
    }
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
        assert!(!DapMode::Fixture.require_live());
        assert!(DapMode::Live.require_live());
        assert!(!DapMode::Auto.require_live());
    }

    #[test]
    fn path_candidates_include_lldb_names() {
        let names = path_candidates("lldb-dap");
        assert!(names.iter().any(|n| n.contains("lldb")));
    }

    #[test]
    fn path_candidates_prefer_requested_name_first() {
        let names = path_candidates("lldb-dap");
        assert_eq!(
            names.first().map(String::as_str),
            Some("lldb-dap"),
            "preferred type must not be demoted by sort; got {names:?}"
        );
        assert!(names.iter().any(|n| n == "codelldb"));
        assert!(names.iter().any(|n| n == "lldb-vscode"));
    }

    #[test]
    fn resolve_system_adapter_never_returns_fake_flag() {
        // Without LEGION_DAP_ADAPTER / PATH tools this is None; if present, must be non-fake.
        if let Some(resolved) = resolve_system_adapter("lldb-dap") {
            assert!(
                !resolved.is_fake,
                "system resolve must not return fake adapter"
            );
        }
    }
}
