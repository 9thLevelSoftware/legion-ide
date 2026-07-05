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
        env: None,
    }
}

/// Helper: make a launch request with an explicit filtered env.
#[allow(dead_code)]
fn make_launch_request_with_env(
    command: impl Into<String>,
    args: Vec<String>,
    env: Vec<(String, String)>,
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
        env: Some(env),
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

// ---------------------------------------------------------------------------
// Task 5 (TERM.07): env deny-list enforcement at PTY spawn.
//
// TDD: write the test first (it fails until PtyRequest::env is plumbed through),
// then implement the fix, then verify it passes.
//
// The test sets a LEGION_SECRET_* env var in the test process, builds a filtered
// env (without it), spawns the shell with that filtered env, and asserts the
// secret value does NOT appear in the shell output while a non-denied control var
// IS visible.
// ---------------------------------------------------------------------------

/// Windows: LEGION_SECRET* vars must be stripped at PTY spawn time.
///
/// Uses `NativePtyService::spawn_pty` + `read_pty` polling so we can wait for the
/// cmd echo output that arrives after the initial ConPTY escape-sequence burst.
#[cfg(windows)]
#[test]
fn windows_env_deny_list_stripped_at_pty_spawn() {
    use legion_platform::{NativePtyService, PtyRequest, PtyService};

    const SECRET_KEY: &str = "LEGION_SECRET_TEST_TOKEN_PTY";
    const SECRET_VAL: &str = "supersecret-pkt-term-pty-test";
    const CONTROL_KEY: &str = "TERM_TEST_CONTROL_VAR_PTY";
    const CONTROL_VAL: &str = "visible123-pkt-term-pty";

    // SAFETY: env mutations are test-local; unique keys cleaned up before assertions.
    unsafe {
        std::env::set_var(SECRET_KEY, SECRET_VAL);
        std::env::set_var(CONTROL_KEY, CONTROL_VAL);
    }

    // Build filtered env: all parent vars EXCEPT those with LEGION_SECRET prefix.
    let filtered_env: Vec<(String, String)> = std::env::vars()
        .filter(|(k, _)| !k.starts_with("LEGION_SECRET"))
        .collect();

    // Pre-condition checks on the filter before spawning.
    assert!(
        filtered_env
            .iter()
            .any(|(k, v)| k == CONTROL_KEY && v == CONTROL_VAL),
        "control var must be in the filtered env"
    );
    assert!(
        !filtered_env.iter().any(|(k, _)| k == SECRET_KEY),
        "secret var must be stripped from the filtered env"
    );

    // Spawn cmd /C to echo both vars and exit. The filtered env is passed directly so
    // the PTY child cannot see LEGION_SECRET* vars.
    let service = NativePtyService;
    let request = PtyRequest {
        command: "cmd".to_string(),
        args: vec![
            "/C".to_string(),
            format!("echo deny=%{}% allow=%{}%", SECRET_KEY, CONTROL_KEY),
        ],
        cwd: None,
        env: Some(filtered_env),
    };
    let session = service
        .spawn_pty(&request)
        .expect("env-deny-list cmd spawn must succeed");

    // Poll for output until we see the echo result or timeout (5 s).
    let mut output = session.output.clone();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while !output.contains("allow=") && std::time::Instant::now() < deadline {
        std::thread::sleep(std::time::Duration::from_millis(50));
        match service.read_pty(&session.id, 64 * 1024) {
            Ok(chunk) => {
                output.push_str(&chunk.output);
                if chunk.exited {
                    break;
                }
            }
            Err(_) => break,
        }
    }

    // Clean up env vars before assertions.
    unsafe {
        std::env::remove_var(SECRET_KEY);
        std::env::remove_var(CONTROL_KEY);
    }

    eprintln!("[TERM-ENV-DENY windows] output={output:?}");
    assert!(
        !output.contains(SECRET_VAL),
        "PTY env must NOT expose the denied secret var; output: {output:?}"
    );
    assert!(
        output.contains(CONTROL_VAL) || output.contains("allow="),
        "PTY env MUST expose the allowed control var; output: {output:?}"
    );
}

/// Unix: LEGION_SECRET* vars must be stripped at PTY spawn time.
#[cfg(unix)]
#[test]
fn unix_env_deny_list_stripped_at_pty_spawn() {
    const SECRET_KEY: &str = "LEGION_SECRET_TEST_TOKEN_PTY";
    const SECRET_VAL: &str = "supersecret-pkt-term-pty-test";
    const CONTROL_KEY: &str = "TERM_TEST_CONTROL_VAR_PTY";
    const CONTROL_VAL: &str = "visible123-pkt-term-pty";

    // SAFETY: env mutations are test-local; keys are unique and cleaned up before assertions.
    unsafe {
        std::env::set_var(SECRET_KEY, SECRET_VAL);
        std::env::set_var(CONTROL_KEY, CONTROL_VAL);
    }

    let filtered_env: Vec<(String, String)> = std::env::vars()
        .filter(|(k, _)| !k.starts_with("LEGION_SECRET"))
        .collect();

    assert!(
        filtered_env
            .iter()
            .any(|(k, v)| k == CONTROL_KEY && v == CONTROL_VAL),
        "control var must be in the filtered env"
    );
    assert!(
        !filtered_env.iter().any(|(k, _)| k == SECRET_KEY),
        "secret var must be stripped from the filtered env"
    );

    let request = make_launch_request_with_env(
        "bash",
        vec![
            "-c".to_string(),
            format!(
                "printf 'deny=${{{}}} allow=${{{}}}'",
                SECRET_KEY, CONTROL_KEY
            ),
        ],
        filtered_env,
    );

    let runtime = TerminalRuntime::new(TerminalRuntimeConfig::enabled(), NativePtyService);
    let outcome = runtime
        .launch(request)
        .expect("env-deny-list bash launch must succeed");

    unsafe {
        std::env::remove_var(SECRET_KEY);
        std::env::remove_var(CONTROL_KEY);
    }

    let output = &outcome.output.redacted_payload;
    assert!(
        !output.contains(SECRET_VAL),
        "PTY env must NOT expose the denied secret var; output: {output:?}"
    );
    assert!(
        output.contains(CONTROL_VAL) || output.contains("allow="),
        "PTY env MUST expose the allowed control var; output: {output:?}"
    );
    eprintln!(
        "[TERM-ENV-DENY unix] session={} output={output:?}",
        outcome.audit.session_id.0
    );
}
