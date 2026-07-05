//! Terminal product policy: shell selection, env passthrough, scrollback, and failure UX.
//!
//! This module owns the product-gate types used by `TerminalWorkflow` in `lib.rs`:
//! shell selection precedence, env allow/deny, scrollback limit, and the projection-level
//! failure state that surfaces to the UI without leaking process internals.

/// Default maximum scrollback rows retained in the projection.
pub const SCROLLBACK_DEFAULT_MAX_ROWS: usize = 5_000;

/// Shell selector: which shell binary the terminal should launch.
///
/// Precedence (highest to lowest): workspace settings → user settings → platform default.
/// Only the first `Some(…)` in the chain is used; `None` falls through to the next level.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TerminalShellSelection {
    /// Explicit shell path/name (e.g. `"/bin/zsh"`, `"pwsh"`, `"cmd"`).
    Explicit(String),
    /// PowerShell Core (`pwsh`) — cross-platform, preferred on Windows.
    PowerShell,
    /// Legacy `cmd.exe` (Windows only; used when PowerShell is not installed).
    Cmd,
    /// `bash` (Unix default).
    Bash,
    /// `zsh` (alternative Unix shell).
    Zsh,
}

impl TerminalShellSelection {
    /// Return the shell command and argument list for this selection.
    pub fn to_command_args(&self) -> (String, Vec<String>) {
        match self {
            Self::PowerShell => (
                "pwsh".to_string(),
                vec!["-NoLogo".to_string(), "-NoExit".to_string()],
            ),
            Self::Cmd => (
                "cmd".to_string(),
                vec!["/Q".to_string(), "/K".to_string()],
            ),
            Self::Bash => (
                "bash".to_string(),
                vec![
                    "-lc".to_string(),
                    r#"export PROMPT_COMMAND='status=$?; printf "\033]7;file://localhost%s\033\\" "$PWD"; printf "\033]133;D;%d\033\\" "$status"'; exec bash --noprofile --norc -i"#
                        .to_string(),
                ],
            ),
            Self::Zsh => ("zsh".to_string(), vec!["-i".to_string()]),
            Self::Explicit(path) => (path.clone(), vec![]),
        }
    }

    /// Display label projected to the UI.
    pub fn label(&self) -> String {
        match self {
            Self::PowerShell => "pwsh".to_string(),
            Self::Cmd => "cmd.exe".to_string(),
            Self::Bash => "bash".to_string(),
            Self::Zsh => "zsh".to_string(),
            Self::Explicit(path) => path.clone(),
        }
    }

    /// Parse a shell selection from a settings label string.
    ///
    /// Returns `None` for empty strings or unknown labels (caller falls back to next tier).
    pub fn from_label(label: &str) -> Option<Self> {
        match label.trim() {
            "" => None,
            "pwsh" | "PowerShell" => Some(Self::PowerShell),
            "cmd" | "cmd.exe" | "Cmd" => Some(Self::Cmd),
            "bash" | "Bash" => Some(Self::Bash),
            "zsh" | "Zsh" => Some(Self::Zsh),
            other => Some(Self::Explicit(other.to_string())),
        }
    }

    /// Return the platform default shell selection.
    pub fn platform_default() -> Self {
        #[cfg(windows)]
        {
            Self::Cmd
        }
        #[cfg(unix)]
        {
            Self::Bash
        }
        #[cfg(not(any(windows, unix)))]
        {
            Self::Explicit("sh".to_string())
        }
    }

    /// Return the platform default shell command and args directly (avoids allocating a variant).
    pub fn platform_default_command_args() -> (String, Vec<String>) {
        Self::platform_default().to_command_args()
    }
}

/// Env passthrough policy for terminal sessions.
///
/// In trusted workspaces the full environment is passed through by default; a hard deny-list
/// (`LEGION_*` prefixed secrets) is always applied regardless of trust state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalEnvPolicy {
    /// Whether to pass the full host environment to the shell (default: `true` for trusted).
    pub passthrough_env: bool,
    /// Variable name prefixes whose values are always stripped before PTY spawn.
    pub deny_prefixes: Vec<String>,
}

impl Default for TerminalEnvPolicy {
    fn default() -> Self {
        Self {
            passthrough_env: true,
            deny_prefixes: vec!["LEGION_SECRET".to_string(), "LEGION_TOKEN".to_string()],
        }
    }
}

/// Platform-safe baseline variable names injected when `passthrough_env=false`.
///
/// These are the minimum variables required for a shell to function at all.
/// Values are taken from the parent process environment so they are always valid
/// paths/settings for the current machine. The deny-prefix filter is applied on
/// top, so none of these names conflict with the hard secret deny-list in practice.
#[cfg(windows)]
const PLATFORM_BASELINE_KEYS: &[&str] = &[
    "SystemRoot",
    "SystemDrive",
    "PATH",
    "TEMP",
    "TMP",
    "COMSPEC",
    "USERPROFILE",
    "HOMEDRIVE",
    "HOMEPATH",
    "windir",
];

#[cfg(unix)]
const PLATFORM_BASELINE_KEYS: &[&str] = &["PATH", "HOME", "TERM", "USER", "SHELL", "LOGNAME"];

#[cfg(not(any(windows, unix)))]
const PLATFORM_BASELINE_KEYS: &[&str] = &["PATH", "HOME"];

impl TerminalEnvPolicy {
    /// Build the effective environment list from `std::env::vars()`, applying deny-list.
    ///
    /// - `passthrough_env=true` (default): returns all parent env vars minus denied prefixes.
    /// - `passthrough_env=false`: returns only the minimal platform-safe baseline vars
    ///   (platform-specific small set required for the shell binary to start), still minus
    ///   denied prefixes. Never returns an empty set — that would crash `cmd.exe` on Windows.
    ///
    /// The returned `Vec` is always `Some`; the caller may pass it verbatim to `PtyRequest::env`.
    pub fn effective_env(&self) -> Option<Vec<(String, String)>> {
        let deny_prefixes: Vec<_> = self.deny_prefixes.iter().map(|s| s.as_str()).collect();
        let is_denied = |key: &str| {
            let ku = key.to_ascii_uppercase();
            deny_prefixes.iter().any(|prefix| ku.starts_with(prefix))
        };

        if self.passthrough_env {
            // Full passthrough: all parent vars minus denied prefixes.
            let env: Vec<(String, String)> = std::env::vars()
                .filter(|(key, _)| !is_denied(key))
                .collect();
            Some(env)
        } else {
            // Isolation mode: only the minimal baseline keys, minus denied prefixes.
            let env: Vec<(String, String)> = PLATFORM_BASELINE_KEYS
                .iter()
                .filter(|&&key| !is_denied(key))
                .filter_map(|&key| {
                    // Case-insensitive lookup: on Windows, env vars are case-insensitive.
                    #[cfg(windows)]
                    {
                        std::env::vars()
                            .find(|(k, _)| k.eq_ignore_ascii_case(key))
                            .map(|(_, v)| (key.to_string(), v))
                    }
                    #[cfg(not(windows))]
                    {
                        std::env::var(key).ok().map(|v| (key.to_string(), v))
                    }
                })
                .collect();
            Some(env)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Task 5 (TERM.07): env allow/deny policy — LEGION_SECRET* and LEGION_TOKEN* are always
    /// stripped; other variables are passed through when `passthrough_env: true`.
    #[test]
    fn env_policy_strips_legion_secrets_and_passes_other_vars() {
        // Inject test vars into the process env temporarily.
        // SAFETY: these are test-only env mutations; test isolation is acceptable here since
        // these keys are unique to this test suite and cleaned up immediately after.
        unsafe {
            std::env::set_var("LEGION_SECRET_KEY", "secret-value");
            std::env::set_var("LEGION_TOKEN_API", "token-value");
            std::env::set_var("LEGION_NORMAL_VAR", "visible-value");
        }

        let policy = TerminalEnvPolicy::default();
        let env = policy
            .effective_env()
            .expect("passthrough enabled by default");

        let keys: Vec<_> = env.iter().map(|(k, _)| k.as_str()).collect();
        assert!(
            !keys.contains(&"LEGION_SECRET_KEY"),
            "LEGION_SECRET_KEY must be stripped; env keys: {keys:?}"
        );
        assert!(
            !keys.contains(&"LEGION_TOKEN_API"),
            "LEGION_TOKEN_API must be stripped; env keys: {keys:?}"
        );
        // LEGION_NORMAL_VAR (no secret/token prefix match) must pass through.
        assert!(
            keys.contains(&"LEGION_NORMAL_VAR"),
            "LEGION_NORMAL_VAR should pass through; env keys: {keys:?}"
        );

        // Cleanup
        unsafe {
            std::env::remove_var("LEGION_SECRET_KEY");
            std::env::remove_var("LEGION_TOKEN_API");
            std::env::remove_var("LEGION_NORMAL_VAR");
        }
    }

    /// Task 5 (TERM.07): passthrough=false must return a minimal platform-safe baseline,
    /// not an empty env (an empty env crashes cmd.exe on Windows and degrades Unix shells).
    #[test]
    fn passthrough_false_returns_minimal_safe_baseline_not_empty() {
        let policy = TerminalEnvPolicy {
            passthrough_env: false,
            ..TerminalEnvPolicy::default()
        };
        let env = policy
            .effective_env()
            .expect("passthrough=false should still return Some(baseline)");

        // Baseline must be non-empty: shells cannot start without PATH (and SystemRoot on
        // Windows).
        assert!(
            !env.is_empty(),
            "passthrough=false baseline must not be empty; shells cannot start without PATH"
        );

        let keys: Vec<&str> = env.iter().map(|(k, _)| k.as_str()).collect();

        // PATH must always be present on any platform.
        assert!(
            keys.iter().any(|k| k.eq_ignore_ascii_case("PATH")),
            "baseline must include PATH; keys: {keys:?}"
        );

        // Windows must include SystemRoot.
        #[cfg(windows)]
        assert!(
            keys.iter().any(|k| k.eq_ignore_ascii_case("SystemRoot")),
            "Windows baseline must include SystemRoot; keys: {keys:?}"
        );

        // Non-baseline vars must NOT leak through even if they are set in the parent env.
        const TEST_NON_BASELINE_KEY: &str = "TERM_TEST_NON_BASELINE_PASSTHROUGH_FALSE_XYZ";
        unsafe { std::env::set_var(TEST_NON_BASELINE_KEY, "should-not-leak") }
        let env2 = policy.effective_env().expect("second call must succeed");
        unsafe { std::env::remove_var(TEST_NON_BASELINE_KEY) }
        assert!(
            !env2.iter().any(|(k, _)| k == TEST_NON_BASELINE_KEY),
            "non-baseline parent var must NOT appear in passthrough=false env"
        );
    }

    #[test]
    fn shell_selection_powerShell_generates_pwsh_command() {
        let (cmd, _args) = TerminalShellSelection::PowerShell.to_command_args();
        assert_eq!(cmd, "pwsh");
    }

    #[test]
    fn shell_selection_cmd_generates_cmd_command() {
        let (cmd, _args) = TerminalShellSelection::Cmd.to_command_args();
        assert_eq!(cmd, "cmd");
    }

    #[test]
    fn shell_selection_label_matches_command() {
        assert_eq!(TerminalShellSelection::PowerShell.label(), "pwsh");
        assert_eq!(TerminalShellSelection::Cmd.label(), "cmd.exe");
        assert_eq!(TerminalShellSelection::Bash.label(), "bash");
    }
}

/// Projection-level terminal failure state surfaced to the UI.
///
/// The renderer shows each variant differently; all carry a bounded metadata-only reason
/// string and never expose raw command output or process internals.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TerminalFailureKind {
    /// Policy denied the launch or operation.
    Denied,
    /// The shell binary or PTY subsystem is unavailable on this platform.
    Unavailable,
    /// Session exited cleanly (exit code = 0).
    Exited,
    /// Session exited with a non-zero code or was killed.
    Crashed,
    /// A mode or network policy blocked the operation.
    PolicyBlocked,
}

impl TerminalFailureKind {
    /// Return a short display label for the renderer.
    pub fn display_label(&self) -> &'static str {
        match self {
            Self::Denied => "denied",
            Self::Unavailable => "unavailable",
            Self::Exited => "exited",
            Self::Crashed => "crashed",
            Self::PolicyBlocked => "policy-blocked",
        }
    }
}
