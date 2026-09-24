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
//! ## Authorization (P2.F3.T2)
//!
//! Resolution is policy-gated: every entry point requires an
//! [`AdapterResolutionGrant`], which can only be minted from a **granted**
//! `debug.adapter.launch` capability decision and carries the allowlist of
//! adapter binaries policy will accept. A program that is not on that list is
//! never returned, including one named by `LEGION_DAP_ADAPTER` — env is an
//! override for *where* the adapter is, never for *what* may be launched.
//!
//! This crate does not evaluate workspace trust (that stays app/security-owned,
//! see `plans/dependency-policy.md`); it refuses to hand out a spawnable path
//! without evidence that the decision was already made and allowed.
//!
//! ## Resolution order (first hit when mode allows live)
//!
//! 1. `LEGION_DAP_ADAPTER` — explicit path to adapter executable
//! 2. `PATH` lookup for type-specific names (`lldb-dap`, `codelldb`, …)
//!    (preferred type first, then aliases — not alphabetical demotion)
//! 3. In-tree `fake_dap_adapter` when `LEGION_DAP_USE_FAKE=1` (CI / local dev)
//!
//! Every hit is filtered through the grant's allowlist before it is returned.
//!
//! ## Mode
//!
//! `LEGION_DAP_MODE=fixture|live|auto` (default `auto`):
//! - `fixture` — never resolve live (callers use simulated runtime)
//! - `live` — require a resolution; callers must fail closed (no fixture)
//! - `auto` — try live, but report no session if unresolved or spawn fails
//!
//! ## Dogfood
//!
//! Set `LEGION_DAP_DOGFOOD=1` on the optional system-adapter handshake test to
//! **require** a real adapter (fail if missing). Without it, the test skips
//! when no system binary is present so CI stays green.

use std::env;
use std::path::{Path, PathBuf};

use legion_protocol::{CapabilityDecision, CapabilityDecisionId};

use crate::live_session::fake_dap_adapter_path;

/// Capability a debug adapter launch must be granted under.
///
/// Mirrors `legion_security::DEBUG_ADAPTER_LAUNCH_CAPABILITY`; the two crates
/// cannot depend on each other (`plans/dependency-policy.md`), so the id is the
/// contract between them and is asserted on both sides.
pub const DEBUG_ADAPTER_LAUNCH_CAPABILITY: &str = "debug.adapter.launch";

/// Product DAP mode from the environment.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DapMode {
    /// Always use the simulated client (no process).
    Fixture,
    /// Require live adapter; callers should fail if unresolved or spawn fails.
    Live,
    /// Try live; report an unavailable adapter instead of fabricating a session.
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

/// Authorization to resolve — and therefore to spawn — a debug adapter binary.
///
/// Minted only by [`AdapterResolutionGrant::from_decision`] from a granted
/// `debug.adapter.launch` decision, and carries the adapter binaries the
/// granting policy allows. Holding one is the caller's evidence that the trust
/// and policy checks already ran.
///
/// The grant is an in-process authorization object, not an unforgeable token:
/// it stops resolution from happening *without* a decision, it does not defend
/// this crate against a caller in the same process that fabricates a decision.
#[derive(Debug, Clone)]
pub struct AdapterResolutionGrant {
    decision_id: CapabilityDecisionId,
    allowed_binaries: Vec<String>,
}

impl AdapterResolutionGrant {
    /// Mint a grant from a capability decision plus the policy's allowlist.
    ///
    /// Returns [`None`] — refusing resolution — when the decision was denied,
    /// when it was granted for some *other* capability, or when the allowlist is
    /// empty or blank. An empty allowlist must never mean "any binary": that is
    /// the same vacuous-truth trap guarded in `legion-security`'s policy rules.
    pub fn from_decision(
        decision: &CapabilityDecision,
        allowed_binaries: &[String],
    ) -> Option<Self> {
        if !decision.granted || decision.capability.0 != DEBUG_ADAPTER_LAUNCH_CAPABILITY {
            return None;
        }
        let allowed_binaries: Vec<String> = allowed_binaries
            .iter()
            .map(|name| name.trim().to_string())
            .filter(|name| !name.is_empty())
            .collect();
        if allowed_binaries.is_empty() {
            return None;
        }
        Some(Self {
            decision_id: decision.decision_id,
            allowed_binaries,
        })
    }

    /// Decision id backing this grant, for audit rows.
    pub fn decision_id(&self) -> CapabilityDecisionId {
        self.decision_id
    }

    /// Whether policy allows launching `program`.
    ///
    /// Matches the file stem case-insensitively, so `codelldb`, `codelldb.exe`
    /// and `C:\tools\CodeLLDB.exe` all check against `codelldb`. Callers that
    /// build a [`ResolvedAdapter`] without going through this module (test seams)
    /// must run their program through here too, or the gate has a hole.
    pub fn permits_program(&self, program: &Path) -> bool {
        let Some(stem) = program.file_stem().and_then(|stem| stem.to_str()) else {
            return false;
        };
        self.allowed_binaries
            .iter()
            .any(|allowed| allowed.eq_ignore_ascii_case(stem))
    }
}

/// Resolve a live adapter for `preferred_type` (e.g. `lldb-dap` / `legion-fake`).
///
/// Returns [`None`] when mode is fixture, when no binary is found, or when the
/// binary that was found is not permitted by `grant`.
pub fn resolve_live_adapter(
    grant: &AdapterResolutionGrant,
    preferred_type: &str,
) -> Option<ResolvedAdapter> {
    let mode = DapMode::from_env();
    if !mode.allows_live() {
        return None;
    }

    if let Some(system) = resolve_system_adapter(grant, preferred_type) {
        return Some(system);
    }

    if env_truthy("LEGION_DAP_USE_FAKE")
        && let Some(fake) = fake_dap_adapter_path()
        // The CI fake is a spawnable process like any other: it is only used
        // when policy names it, never because the env var was set.
        && grant.permits_program(&fake)
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
/// Every hit must satisfy `grant`, so `LEGION_DAP_ADAPTER` cannot be used to
/// point the debugger at an arbitrary executable. Never returns the in-tree
/// `fake_dap_adapter`.
pub fn resolve_system_adapter(
    grant: &AdapterResolutionGrant,
    preferred_type: &str,
) -> Option<ResolvedAdapter> {
    if let Ok(path) = env::var("LEGION_DAP_ADAPTER") {
        let path = PathBuf::from(path.trim());
        if !path.as_os_str().is_empty() && path.exists() && grant.permits_program(&path) {
            return Some(ResolvedAdapter {
                program: path,
                args: Vec::new(),
                adapter_type: preferred_type.to_string(),
                is_fake: false,
            });
        }
    }

    for candidate in path_candidates(preferred_type) {
        if let Some(found) = find_on_path(&candidate)
            && grant.permits_program(&found)
        {
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
        if let Some(resolved) = resolve_system_adapter(&test_grant(), "lldb-dap") {
            assert!(
                !resolved.is_fake,
                "system resolve must not return fake adapter"
            );
        }
    }

    #[test]
    fn auto_mode_does_not_resolve_an_unallowlisted_adapter() {
        let grant = AdapterResolutionGrant::from_decision(
            &granted_decision(DEBUG_ADAPTER_LAUNCH_CAPABILITY),
            &["legion-test-adapter-that-is-not-installed".to_string()],
        )
        .expect("granted decision with a non-empty allowlist mints a grant");

        assert!(
            resolve_live_adapter(&grant, "legion-test-adapter-that-is-not-installed").is_none(),
            "auto mode must leave the caller with no session when no permitted adapter resolves"
        );
    }

    /// Granted decision for the adapters the default policy allows.
    fn test_grant() -> AdapterResolutionGrant {
        AdapterResolutionGrant::from_decision(
            &granted_decision(DEBUG_ADAPTER_LAUNCH_CAPABILITY),
            &["lldb-dap".to_string(), "codelldb".to_string()],
        )
        .expect("granted decision with a non-empty allowlist mints a grant")
    }

    fn granted_decision(capability: &str) -> CapabilityDecision {
        CapabilityDecision {
            decision_id: CapabilityDecisionId(7),
            granted: true,
            capability: legion_protocol::CapabilityId(capability.to_string()),
            reason: None,
        }
    }

    #[test]
    fn grant_requires_a_granted_debug_adapter_launch_decision() {
        let allowed = ["lldb-dap".to_string()];

        let denied = CapabilityDecision {
            granted: false,
            ..granted_decision(DEBUG_ADAPTER_LAUNCH_CAPABILITY)
        };
        assert!(
            AdapterResolutionGrant::from_decision(&denied, &allowed).is_none(),
            "a denied decision must not authorize resolution"
        );

        assert!(
            AdapterResolutionGrant::from_decision(&granted_decision("terminal.launch"), &allowed)
                .is_none(),
            "a grant for another capability must not be reusable for debug adapters"
        );

        assert!(
            AdapterResolutionGrant::from_decision(
                &granted_decision(DEBUG_ADAPTER_LAUNCH_CAPABILITY),
                &[],
            )
            .is_none(),
            "an empty allowlist must deny, not allow everything"
        );
        assert!(
            AdapterResolutionGrant::from_decision(
                &granted_decision(DEBUG_ADAPTER_LAUNCH_CAPABILITY),
                &[String::new(), "   ".to_string()],
            )
            .is_none(),
            "blank allowlist entries must not authorize anything"
        );

        let grant = AdapterResolutionGrant::from_decision(
            &granted_decision("debug.adapter.launch"),
            &allowed,
        )
        .expect("granted decision mints a grant");
        assert_eq!(grant.decision_id(), CapabilityDecisionId(7));
    }

    #[test]
    fn grant_permits_only_allowlisted_binaries() {
        let grant = test_grant();
        assert!(grant.permits_program(Path::new("/usr/bin/lldb-dap")));
        // Case-insensitive, and the extension is not part of the name. Written
        // without a separator so it means the same thing everywhere: off
        // Windows a backslash is an ordinary filename character, so
        // `C:\tools\CodeLLDB.exe` has no directory part and its stem is the
        // entire string — which is how this assertion passed on Windows and
        // failed on Linux.
        assert!(grant.permits_program(Path::new("CodeLLDB.exe")));
        #[cfg(windows)]
        assert!(grant.permits_program(Path::new("C:\\tools\\CodeLLDB.exe")));
        // The whole point of the allowlist: an arbitrary executable handed to
        // LEGION_DAP_ADAPTER is not launchable just because it exists.
        assert!(!grant.permits_program(Path::new("/bin/sh")));
        assert!(!grant.permits_program(Path::new("fake_dap_adapter.exe")));
        assert!(!grant.permits_program(Path::new("")));
    }
}
