//! Task 8 (TERM.02/12): Platform shell launch smoke tests.
//!
//! Verifies that the terminal runtime can spawn real shell processes via the native PTY
//! backend. Each test is guarded by `#[cfg(target_os = ...)]` so only the platforms
//! where the shell binary is available run the test.
//!
//! Evidence scope:
//! - Windows: `cmd.exe` (always present) and `pwsh` (PowerShell Core; skipped if absent)
//! - Unix: `bash` and `zsh` (skipped if absent)

use legion_platform::NativePtyService;
use legion_protocol::{
    CapabilityId, PrincipalId, TerminalLaunchPolicyContract, WorkspaceId, WorkspaceTrustState,
};
use legion_terminal::{TerminalRuntime, TerminalRuntimeConfig, TerminalRuntimeLaunchRequest};

fn make_launch_request(
    command: impl Into<String>,
    args: Vec<String>,
) -> TerminalRuntimeLaunchRequest {
    TerminalRuntimeLaunchRequest {
        policy: TerminalLaunchPolicyContract {
            principal_id: PrincipalId("smoke-test".to_string()),
            workspace_id: WorkspaceId(99),
            trust_state: WorkspaceTrustState::Trusted,
            capability_id: CapabilityId("terminal.launch".to_string()),
            cwd_policy: "workspace-root".to_string(),
            output_byte_limit: 64 * 1024,
            timeout_seconds: 30,
            schema_version: 1,
        },
        command: command.into(),
        args,
    }
}

/// Windows `cmd.exe` one-shot smoke: verifies the runtime can spawn a cmd session
/// and receive initial output via the ConPTY backend.
#[cfg(windows)]
#[test]
fn windows_cmd_launch_smoke() {
    let runtime = TerminalRuntime::new(TerminalRuntimeConfig::enabled(), NativePtyService);
    let result = runtime.launch(make_launch_request(
        "cmd",
        vec!["/Q".to_string(), "/K".to_string()],
    ));
    match result {
        Ok(outcome) => {
            // The audit record must identify a running session.
            assert!(
                outcome.audit.session_id.0 > 0,
                "cmd session id must be nonzero"
            );
            eprintln!(
                "[TERM-SMOKE cmd] session={} state={:?} bytes={}",
                outcome.audit.session_id.0, outcome.audit.state, outcome.output.byte_count
            );
        }
        Err(err) => {
            panic!("cmd.exe smoke launch failed: {err}");
        }
    }
}

/// Windows `pwsh` (PowerShell Core) smoke: verifies that the PowerShell Core shell
/// can be launched when it is installed. If `pwsh` is not on PATH the test is skipped
/// gracefully.
#[cfg(windows)]
#[test]
fn windows_powershell_core_launch_smoke() {
    use std::process::Command;
    // Detect whether pwsh is on PATH without spawning a PTY.
    let which = Command::new("where").args(["pwsh"]).output();
    let pwsh_available = which.map(|o| o.status.success()).unwrap_or(false);
    if !pwsh_available {
        eprintln!("[TERM-SMOKE pwsh] SKIPPED — pwsh not found on PATH");
        return;
    }

    let runtime = TerminalRuntime::new(TerminalRuntimeConfig::enabled(), NativePtyService);
    let result = runtime.launch(make_launch_request(
        "pwsh",
        vec!["-NoLogo".to_string(), "-NoExit".to_string()],
    ));
    match result {
        Ok(outcome) => {
            assert!(
                outcome.audit.session_id.0 > 0,
                "pwsh session id must be nonzero"
            );
            eprintln!(
                "[TERM-SMOKE pwsh] session={} state={:?} bytes={}",
                outcome.audit.session_id.0, outcome.audit.state, outcome.output.byte_count
            );
        }
        Err(err) => {
            panic!("pwsh smoke launch failed: {err}");
        }
    }
}

/// Unix `bash` one-shot smoke.
#[cfg(unix)]
#[test]
fn unix_bash_launch_smoke() {
    let runtime = TerminalRuntime::new(TerminalRuntimeConfig::enabled(), NativePtyService);
    let result = runtime.launch(make_launch_request(
        "bash",
        vec![
            "-lc".to_string(),
            r#"export PROMPT_COMMAND='status=$?; printf "\033]7;file://localhost%s\033\\" "$PWD"; printf "\033]133;D;%d\033\\" "$status"'; exec bash --noprofile --norc -i"#
                .to_string(),
        ],
    ));
    match result {
        Ok(outcome) => {
            assert!(outcome.audit.session_id.0 > 0);
            eprintln!(
                "[TERM-SMOKE bash] session={} state={:?} bytes={}",
                outcome.audit.session_id.0, outcome.audit.state, outcome.output.byte_count
            );
        }
        Err(err) => {
            panic!("bash smoke launch failed: {err}");
        }
    }
}

/// Unix `zsh` smoke: skipped gracefully if not installed.
#[cfg(unix)]
#[test]
fn unix_zsh_launch_smoke() {
    use std::process::Command;
    let which = Command::new("which").args(["zsh"]).output();
    let zsh_available = which.map(|o| o.status.success()).unwrap_or(false);
    if !zsh_available {
        eprintln!("[TERM-SMOKE zsh] SKIPPED — zsh not found on PATH");
        return;
    }

    let runtime = TerminalRuntime::new(TerminalRuntimeConfig::enabled(), NativePtyService);
    let result = runtime.launch(make_launch_request("zsh", vec!["-i".to_string()]));
    match result {
        Ok(outcome) => {
            assert!(outcome.audit.session_id.0 > 0);
            eprintln!(
                "[TERM-SMOKE zsh] session={} state={:?} bytes={}",
                outcome.audit.session_id.0, outcome.audit.state, outcome.output.byte_count
            );
        }
        Err(err) => {
            panic!("zsh smoke launch failed: {err}");
        }
    }
}
