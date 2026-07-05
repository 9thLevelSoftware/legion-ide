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

impl TerminalEnvPolicy {
    /// Build the effective environment list from `std::env::vars()`, applying deny-list.
    ///
    /// Always strips variables whose names start with any `deny_prefix`. Returns `None`
    /// when `passthrough_env` is `false` (the PTY inherits its default env from the
    /// platform process; callers pass an empty override when they want isolation).
    pub fn effective_env(&self) -> Option<Vec<(String, String)>> {
        if !self.passthrough_env {
            return None;
        }
        let deny_prefixes: Vec<_> = self.deny_prefixes.iter().map(|s| s.as_str()).collect();
        let env: Vec<(String, String)> = std::env::vars()
            .filter(|(key, _)| {
                let ku = key.to_ascii_uppercase();
                !deny_prefixes.iter().any(|prefix| ku.starts_with(prefix))
            })
            .collect();
        Some(env)
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

    #[test]
    fn env_policy_none_when_passthrough_disabled() {
        let policy = TerminalEnvPolicy {
            passthrough_env: false,
            ..TerminalEnvPolicy::default()
        };
        assert!(
            policy.effective_env().is_none(),
            "passthrough_env=false should return None"
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
