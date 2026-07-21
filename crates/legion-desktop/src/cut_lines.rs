//! Honest cut-line and fixture status copy for deferred or simulated product surfaces.
//!
//! Tier 0 (truth repair): status strings and panel labels must not imply that
//! fixture/metadata/harness paths are full product capabilities (real DAP,
//! production remote transport, WASM plugin execution, or live default AI).

/// Status when a plugin manifest is registered without product WASM execution.
pub fn plugin_registered_status(plugin_id: u64) -> String {
    format!(
        "Plugin {plugin_id} registered (metadata-only; WASM execution not available in this build)"
    )
}

/// Status when the remote development runtime is policy-enabled (harness only).
pub const REMOTE_RUNTIME_ENABLED: &str = "Remote workspace runtime enabled by app policy (fixture/harness; PR-ENT-001 product UX deferred)";

/// Status when a remote workspace session is connected through the fixture harness.
pub fn remote_fixture_session_active(
    session_id: impl std::fmt::Display,
    authority_label: &str,
) -> String {
    format!(
        "Remote fixture session active {session_id} authority={authority_label} (no production transport; PR-ENT-001 deferred)"
    )
}

/// Status when the deterministic debug fixture is enabled.
pub const DEBUG_FIXTURE_ENABLED: &str =
    "Debug fixture enabled by app policy (simulated DAP — no adapter process)";

/// Short banner for debug panels when only the fixture is available.
pub const DEBUG_SIMULATED_BANNER: &str = "Debugger is simulated in this build";

/// Banner when a live DAP adapter process is connected (B3 dual-mode honesty).
pub const DEBUG_LIVE_BANNER: &str = "Debugger connected to a live adapter process";

/// Prefix for plugin management rows that cannot execute WASM in product composition.
pub const PLUGIN_EXECUTION_UNAVAILABLE: &str = "execution=not-available";

/// Provider id used by the deterministic fixture path.
pub const DETERMINISTIC_LOCAL_PROVIDER_ID: &str = "deterministic-local";

/// Label for the deterministic-local provider in UI copy.
pub const DETERMINISTIC_PROVIDER_UI_LABEL: &str = "Deterministic fixture (not a live model)";

/// Section subtitle for hardcoded sample context-pack lists.
pub const CONTEXT_PACKS_SAMPLE_LABEL: &str = "Sample / not live inventory";

/// Display label for a projected provider: honest fixture wording for deterministic-local.
pub fn provider_display_label(provider_id: &str, provider_label: &str) -> String {
    if provider_id == DETERMINISTIC_LOCAL_PROVIDER_ID
        || provider_label.eq_ignore_ascii_case("deterministic-local")
        || provider_label
            .to_ascii_lowercase()
            .contains("deterministic")
    {
        DETERMINISTIC_PROVIDER_UI_LABEL.to_string()
    } else {
        provider_label.to_string()
    }
}
