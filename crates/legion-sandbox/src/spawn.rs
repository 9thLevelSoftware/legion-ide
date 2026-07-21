//! Real process spawning under OS-level sandbox enforcement.
//!
//! This module provides [`spawn_sandboxed`], which spawns a child process under
//! platform-specific OS-level constraints, and returns an honest
//! [`SandboxEnforcementReport`] describing what was actually enforced.
//!
//! # Fail-closed guarantee
//!
//! If the platform sandbox mechanism is unavailable, [`spawn_sandboxed`] returns
//! [`SandboxError::PlatformUnavailable`]. It never silently spawns an unsandboxed
//! process.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::SandboxError;

/// What to spawn inside the sandbox.
#[derive(Debug, Clone)]
pub struct SandboxSpawnSpec {
    /// Program to execute.
    pub program: PathBuf,
    /// Arguments to pass.
    pub args: Vec<String>,
    /// Working directory for the child.
    pub working_dir: PathBuf,
    /// Writable root — the child may write inside this directory only.
    pub writable_root: PathBuf,
    /// Allowed network egress destinations (empty = no network).
    pub allowed_egress: BTreeSet<String>,
    /// Timeout for the child process.
    pub timeout: Duration,
    /// Environment variables to pass to the child (empty = inherit nothing extra).
    pub env: Vec<(String, String)>,
}

/// What the sandbox observed about its own enforcement.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SandboxEnforcementReport {
    /// Whether filesystem writes outside writable_root are enforced.
    pub filesystem_write_enforced: bool,
    /// Whether filesystem reads outside writable_root are enforced.
    pub filesystem_read_enforced: bool,
    /// Whether network egress is enforced by the OS.
    pub network_enforced: bool,
    /// Human-readable caveat labels for anything not enforced.
    pub caveat_labels: Vec<String>,
    /// Which backend was used.
    pub backend_used: String,
}

/// Result of a sandboxed spawn.
#[derive(Debug)]
pub struct SandboxedCommandOutput {
    /// Exit code (None if killed by signal/timeout).
    pub exit_code: Option<i32>,
    /// Captured stdout.
    pub stdout: Vec<u8>,
    /// Captured stderr.
    pub stderr: Vec<u8>,
    /// Enforcement report.
    pub enforcement: SandboxEnforcementReport,
    /// Whether the process was killed due to timeout.
    pub timed_out: bool,
}

/// Escape a string for safe inclusion in an SBPL double-quoted string literal.
///
/// SBPL uses C-style escaping: backslashes and embedded double quotes must be
/// escaped.  Without this, a `writable_root` or egress entry that contains `"`
/// could inject arbitrary SBPL rules into the profile.
fn escape_sbpl_string(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

/// Generate a macOS Seatbelt Profile Language (SBPL) profile string.
///
/// This is a pure function and can be tested on any platform.
pub fn generate_sbpl_profile(writable_root: &Path, allowed_egress: &BTreeSet<String>) -> String {
    let mut profile = String::new();
    profile.push_str("(version 1)\n");
    profile.push_str("(deny default)\n");
    profile.push_str("(allow file-read* (subpath \"/\"))\n");
    profile.push_str(&format!(
        "(allow file-write* (subpath \"{}\"))\n",
        escape_sbpl_string(&writable_root.to_string_lossy())
    ));
    profile.push_str("(allow process-exec)\n");
    profile.push_str("(allow process-fork)\n");

    if allowed_egress.is_empty() {
        profile.push_str("(deny network*)\n");
    } else {
        for dest in allowed_egress {
            profile.push_str(&format!(
                "(allow network* (remote tcp \"{}\"))\n",
                escape_sbpl_string(dest)
            ));
        }
    }

    profile
}

/// Spawn a child process under OS-level sandbox enforcement.
///
/// Fails closed: if the platform sandbox mechanism is unavailable,
/// returns an error — never silently spawns unsandboxed.
pub fn spawn_sandboxed(spec: &SandboxSpawnSpec) -> Result<SandboxedCommandOutput, SandboxError> {
    // Each block below is cfg-gated to exactly one platform.  Exactly one
    // block is compiled per target, so exactly one expression provides the
    // function's return value.

    #[cfg(target_os = "linux")]
    {
        linux::spawn_sandboxed_linux(spec)
    }

    #[cfg(target_os = "macos")]
    {
        macos::spawn_sandboxed_macos(spec)
    }

    #[cfg(target_os = "windows")]
    {
        windows_impl::spawn_sandboxed_windows(spec)
    }

    #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
    {
        Err(SandboxError::PlatformUnavailable {
            platform: std::env::consts::OS.to_string(),
            reason: "no sandbox implementation for this platform".to_string(),
        })
    }
}

// ---------------------------------------------------------------------------
// Linux — Landlock FS write + optional bubblewrap network namespace
// ---------------------------------------------------------------------------

#[cfg(target_os = "linux")]
mod linux {
    use super::*;
    use landlock::{
        ABI, AccessFs, BitFlags, CompatLevel, Compatible, PathBeneath, PathFd, Ruleset,
        RulesetAttr, RulesetCreatedAttr,
    };
    use std::io::{self, Read};
    use std::os::unix::process::CommandExt;
    use std::process::{Command, Stdio};

    const WRITE_POLICY_ABI: ABI = ABI::V5;

    fn write_access_rights() -> BitFlags<AccessFs> {
        AccessFs::from_write(WRITE_POLICY_ABI)
    }

    fn bwrap_path() -> Option<PathBuf> {
        let candidates = [
            PathBuf::from("/usr/bin/bwrap"),
            PathBuf::from("/bin/bwrap"),
            PathBuf::from("bwrap"),
        ];
        for path in candidates {
            if path.file_name().is_some_and(|n| n == "bwrap") {
                // PATH lookup for bare name
                if path.components().count() == 1 {
                    if std::process::Command::new("bwrap")
                        .arg("--version")
                        .stdout(Stdio::null())
                        .stderr(Stdio::null())
                        .status()
                        .map(|s| s.success())
                        .unwrap_or(false)
                    {
                        return Some(PathBuf::from("bwrap"));
                    }
                    continue;
                }
            }
            if path.is_file() {
                return Some(path);
            }
        }
        None
    }

    pub fn spawn_sandboxed_linux(
        spec: &SandboxSpawnSpec,
    ) -> Result<SandboxedCommandOutput, SandboxError> {
        let abi = WRITE_POLICY_ABI;
        let abi_version = abi as u32;

        // ABI v3 adds AccessFs::Truncate and ABI v5 adds AccessFs::IoctlDev;
        // both are required to report filesystem write enforcement honestly.
        let write_access = write_access_rights();

        // Probe Landlock availability in the parent process before spawning.
        Ruleset::default()
            .set_compatibility(CompatLevel::HardRequirement)
            .handle_access(write_access)
            .map_err(|e| SandboxError::PlatformUnavailable {
                platform: "linux".to_string(),
                reason: format!("Landlock required write rights unavailable: {e}"),
            })?
            .create()
            .map_err(|e| SandboxError::PlatformUnavailable {
                platform: "linux".to_string(),
                reason: format!("Landlock create failed: {e}"),
            })?;

        let writable_root = spec.writable_root.clone();

        // Network: when no egress is allowed, wrap with bubblewrap --unshare-net
        // so the child has an empty network namespace (connect fails). Selective
        // egress allowlists are not implemented on Linux yet (macOS Seatbelt).
        let deny_all_network = spec.allowed_egress.is_empty();
        let bwrap = if deny_all_network { bwrap_path() } else { None };
        let network_enforced = deny_all_network && bwrap.is_some();

        let mut cmd = if let Some(ref bwrap_bin) = bwrap {
            // bwrap --unshare-net --die-with-parent \
            //   --bind / / --dev /dev --proc /proc \
            //   --chdir <wd> -- <program> <args...>
            // Landlock still applied in pre_exec for FS write isolation.
            let mut c = Command::new(bwrap_bin);
            c.arg("--unshare-net")
                .arg("--die-with-parent")
                .arg("--bind")
                .arg("/")
                .arg("/")
                .arg("--dev")
                .arg("/dev")
                .arg("--proc")
                .arg("/proc")
                .arg("--chdir")
                .arg(&spec.working_dir)
                .arg("--")
                .arg(&spec.program)
                .args(&spec.args);
            c
        } else {
            let mut c = Command::new(&spec.program);
            c.args(&spec.args).current_dir(&spec.working_dir);
            c
        };

        cmd.stdout(Stdio::piped()).stderr(Stdio::piped());

        for (k, v) in &spec.env {
            cmd.env(k, v);
        }

        // Apply Landlock ruleset in the child before exec.
        // SAFETY: This closure is called after fork in the child process before exec.
        // Landlock syscalls are async-signal-safe.
        unsafe {
            cmd.pre_exec(move || {
                let write_access_child = write_access_rights();

                let path_fd =
                    PathFd::new(&writable_root).map_err(|e| io::Error::other(format!("{e}")))?;

                // HardRequirement keeps the enforcement report honest: if the
                // kernel cannot handle the requested write rights, spawning fails
                // instead of silently applying a weaker ruleset.
                let _ = Ruleset::default()
                    .set_compatibility(CompatLevel::HardRequirement)
                    .handle_access(write_access_child)
                    .map_err(|e| io::Error::other(format!("{e}")))?
                    .create()
                    .map_err(|e| io::Error::other(format!("{e}")))?
                    .add_rule(PathBeneath::new(path_fd, write_access_child))
                    .map_err(|e| io::Error::other(format!("{e}")))?
                    .restrict_self()
                    .map_err(|e| io::Error::other(format!("{e}")))?;

                Ok(())
            });
        }

        let mut child = cmd.spawn().map_err(|e| SandboxError::SpawnFailed {
            reason: e.to_string(),
        })?;

        // Grab pipe handles before polling loop.
        let mut stdout_pipe = child.stdout.take().expect("stdout piped");
        let mut stderr_pipe = child.stderr.take().expect("stderr piped");

        let stdout_thread = std::thread::spawn(move || {
            let mut buf = Vec::new();
            let _ = stdout_pipe.read_to_end(&mut buf);
            buf
        });
        let stderr_thread = std::thread::spawn(move || {
            let mut buf = Vec::new();
            let _ = stderr_pipe.read_to_end(&mut buf);
            buf
        });

        // Poll for exit with timeout.
        let deadline = std::time::Instant::now() + spec.timeout;
        let mut timed_out = false;
        let exit_code;

        loop {
            match child.try_wait().map_err(|e| SandboxError::SpawnFailed {
                reason: e.to_string(),
            })? {
                Some(status) => {
                    exit_code = status.code();
                    break;
                }
                None => {
                    if std::time::Instant::now() >= deadline {
                        child.kill().ok();
                        let _ = child.wait();
                        timed_out = true;
                        exit_code = None;
                        break;
                    }
                    std::thread::sleep(Duration::from_millis(10));
                }
            }
        }

        let stdout = stdout_thread.join().unwrap_or_default();
        let stderr = stderr_thread.join().unwrap_or_default();

        let mut caveat_labels = Vec::new();
        if !network_enforced {
            if deny_all_network {
                caveat_labels.push("bwrap-unshare-net-unavailable".to_string());
            } else {
                caveat_labels.push("linux-egress-allowlist-not-implemented".to_string());
            }
        }
        let backend = if network_enforced {
            format!("landlock-v{abi_version}+bwrap-unshare-net")
        } else {
            format!("landlock-v{abi_version}")
        };

        Ok(SandboxedCommandOutput {
            exit_code,
            stdout,
            stderr,
            enforcement: SandboxEnforcementReport {
                filesystem_write_enforced: true,
                filesystem_read_enforced: false,
                network_enforced,
                caveat_labels,
                backend_used: backend,
            },
            timed_out,
        })
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn write_access_rights_cover_truncation() {
            let rights = write_access_rights();

            assert!(rights.contains(AccessFs::WriteFile));
            assert!(rights.contains(AccessFs::Truncate));
            assert!(rights.contains(AccessFs::IoctlDev));
        }
    }
}

// ---------------------------------------------------------------------------
// macOS — Seatbelt sandbox-exec enforcement
// ---------------------------------------------------------------------------

#[cfg(target_os = "macos")]
mod macos {
    use super::*;
    use std::io::Read;
    use std::process::{Command, Stdio};

    pub fn spawn_sandboxed_macos(
        spec: &SandboxSpawnSpec,
    ) -> Result<SandboxedCommandOutput, SandboxError> {
        let sandbox_exec = std::path::Path::new("/usr/bin/sandbox-exec");
        if !sandbox_exec.exists() {
            return Err(SandboxError::PlatformUnavailable {
                platform: "macos".to_string(),
                reason: "sandbox-exec not found at /usr/bin/sandbox-exec".to_string(),
            });
        }

        let sbpl = generate_sbpl_profile(&spec.writable_root, &spec.allowed_egress);

        // Assemble sandbox-exec command line: sandbox-exec -p <profile> -- <program> <args...>
        let mut args: Vec<std::ffi::OsString> = vec![
            "-p".into(),
            sbpl.into(),
            "--".into(),
            spec.program.as_os_str().into(),
        ];
        args.extend(spec.args.iter().map(|a| a.into()));

        let mut cmd = Command::new(sandbox_exec);
        cmd.args(&args)
            .current_dir(&spec.working_dir)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        for (k, v) in &spec.env {
            cmd.env(k, v);
        }

        let mut child = cmd.spawn().map_err(|e| SandboxError::SpawnFailed {
            reason: e.to_string(),
        })?;

        let mut stdout_pipe = child.stdout.take().expect("stdout piped");
        let mut stderr_pipe = child.stderr.take().expect("stderr piped");

        let stdout_thread = std::thread::spawn(move || {
            let mut buf = Vec::new();
            let _ = stdout_pipe.read_to_end(&mut buf);
            buf
        });
        let stderr_thread = std::thread::spawn(move || {
            let mut buf = Vec::new();
            let _ = stderr_pipe.read_to_end(&mut buf);
            buf
        });

        let deadline = std::time::Instant::now() + spec.timeout;
        let mut timed_out = false;
        let exit_code;

        loop {
            match child.try_wait().map_err(|e| SandboxError::SpawnFailed {
                reason: e.to_string(),
            })? {
                Some(status) => {
                    exit_code = status.code();
                    break;
                }
                None => {
                    if std::time::Instant::now() >= deadline {
                        child.kill().ok();
                        let _ = child.wait();
                        timed_out = true;
                        exit_code = None;
                        break;
                    }
                    std::thread::sleep(Duration::from_millis(10));
                }
            }
        }

        let stdout = stdout_thread.join().unwrap_or_default();
        let stderr = stderr_thread.join().unwrap_or_default();

        Ok(SandboxedCommandOutput {
            exit_code,
            stdout,
            stderr,
            enforcement: SandboxEnforcementReport {
                filesystem_write_enforced: true,
                filesystem_read_enforced: false,
                network_enforced: true,
                caveat_labels: vec![],
                backend_used: "seatbelt-sbpl".to_string(),
            },
            timed_out,
        })
    }
}

// ---------------------------------------------------------------------------
// Windows — job object with KILL_ON_JOB_CLOSE
//
// SIMPLIFICATION: Uses CreateProcessW + CREATE_SUSPENDED + a job object with
// JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE instead of the full
// CreateRestrictedToken + CreateProcessAsUserW path. The job object ensures
// all child processes are killed when the job handle closes (timeout or
// normal exit), but does NOT restrict filesystem access. This is documented
// honestly in the enforcement report.
// ---------------------------------------------------------------------------

#[cfg(target_os = "windows")]
mod windows_impl {
    use super::*;

    use ::windows::Win32::Foundation::{CloseHandle, HANDLE};
    use ::windows::Win32::Security::SECURITY_ATTRIBUTES;
    use ::windows::Win32::Storage::FileSystem::ReadFile;

    /// Windows `WaitForSingleObject` return value for timeout.
    const WAIT_TIMEOUT_VAL: u32 = 258;
    /// Infinite timeout value for `WaitForSingleObject`.
    const WIN_INFINITE: u32 = 0xFFFF_FFFFu32;
    use ::windows::Win32::System::JobObjects::{
        AssignProcessToJobObject, CreateJobObjectW, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
        JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JobObjectExtendedLimitInformation,
        SetInformationJobObject,
    };
    use ::windows::Win32::System::Pipes::CreatePipe;
    use ::windows::Win32::System::Threading::{
        CREATE_NO_WINDOW, CREATE_SUSPENDED, CreateProcessW, GetExitCodeProcess,
        PROCESS_INFORMATION, ResumeThread, STARTF_USESTDHANDLES, STARTUPINFOW, TerminateProcess,
        WaitForSingleObject,
    };
    use ::windows::core::{PCWSTR, PWSTR};
    use std::mem::{size_of, zeroed};
    use std::ptr;

    /// RAII guard that closes a Windows HANDLE on drop.
    struct OwnedHandle(HANDLE);

    impl Drop for OwnedHandle {
        fn drop(&mut self) {
            if !self.0.0.is_null() {
                unsafe {
                    let _ = CloseHandle(self.0);
                }
            }
        }
    }

    impl OwnedHandle {
        /// Consumes the guard, releasing ownership without closing the handle.
        /// The caller becomes responsible for closing the handle.
        fn into_raw(self) -> HANDLE {
            let h = self.0;
            std::mem::forget(self);
            h
        }
    }

    fn wide_null(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(std::iter::once(0)).collect()
    }

    /// Quote a single command-line argument per Windows quoting rules.
    fn quote_arg(arg: &str) -> String {
        if !arg.is_empty() && !arg.chars().any(|c| c.is_whitespace() || c == '"') {
            return arg.to_string();
        }
        let mut quoted = String::with_capacity(arg.len() + 2);
        quoted.push('"');
        let mut pending_backslashes = 0usize;
        for ch in arg.chars() {
            match ch {
                '\\' => pending_backslashes += 1,
                '"' => {
                    for _ in 0..pending_backslashes.saturating_mul(2) + 1 {
                        quoted.push('\\');
                    }
                    quoted.push('"');
                    pending_backslashes = 0;
                }
                _ => {
                    for _ in 0..pending_backslashes {
                        quoted.push('\\');
                    }
                    pending_backslashes = 0;
                    quoted.push(ch);
                }
            }
        }
        for _ in 0..pending_backslashes.saturating_mul(2) {
            quoted.push('\\');
        }
        quoted.push('"');
        quoted
    }

    fn build_command_line(program: &Path, args: &[String]) -> Vec<u16> {
        let mut parts = Vec::with_capacity(args.len() + 1);
        parts.push(quote_arg(&program.to_string_lossy()));
        parts.extend(args.iter().map(|a| quote_arg(a)));
        wide_null(&parts.join(" "))
    }

    /// Read all bytes from a pipe handle and close it.
    /// Called from a background thread; the handle value is passed as usize for Send.
    fn drain_pipe(handle_usize: usize) -> Vec<u8> {
        let handle = HANDLE(handle_usize as *mut core::ffi::c_void);
        let mut buf = Vec::new();
        let mut chunk = [0u8; 4096];
        loop {
            let mut read = 0u32;
            let result = unsafe { ReadFile(handle, Some(&mut chunk), Some(&mut read), None) };
            if result.is_err() || read == 0 {
                break;
            }
            buf.extend_from_slice(&chunk[..read as usize]);
        }
        unsafe {
            let _ = CloseHandle(handle);
        }
        buf
    }

    pub fn spawn_sandboxed_windows(
        spec: &SandboxSpawnSpec,
    ) -> Result<SandboxedCommandOutput, SandboxError> {
        // SAFETY: All Win32 calls are wrapped with proper error handling.
        unsafe {
            // ------------------------------------------------------------------
            // 1. Create stdout / stderr pipes.  The write ends are inheritable
            //    so the child receives them via STARTUPINFOW.  The read ends are
            //    non-inheritable (stay in the parent).
            // ------------------------------------------------------------------
            let mut stdout_read = HANDLE::default();
            let mut stdout_write = HANDLE::default();
            let mut stderr_read = HANDLE::default();
            let mut stderr_write = HANDLE::default();

            let sa = SECURITY_ATTRIBUTES {
                nLength: size_of::<SECURITY_ATTRIBUTES>() as u32,
                lpSecurityDescriptor: ptr::null_mut(),
                bInheritHandle: ::windows::Win32::Foundation::TRUE,
            };

            CreatePipe(&mut stdout_read, &mut stdout_write, Some(&sa), 0).map_err(|e| {
                SandboxError::SpawnFailed {
                    reason: format!("CreatePipe stdout: {e}"),
                }
            })?;
            CreatePipe(&mut stderr_read, &mut stderr_write, Some(&sa), 0).map_err(|e| {
                let _ = CloseHandle(stdout_read);
                let _ = CloseHandle(stdout_write);
                SandboxError::SpawnFailed {
                    reason: format!("CreatePipe stderr: {e}"),
                }
            })?;

            // ------------------------------------------------------------------
            // 2. Build the command line and STARTUPINFOW.
            // ------------------------------------------------------------------
            let app_wide = wide_null(&spec.program.to_string_lossy());
            let mut cmd_wide = build_command_line(&spec.program, &spec.args);
            let working_dir_wide = wide_null(&spec.working_dir.to_string_lossy());

            let mut startup: STARTUPINFOW = zeroed();
            startup.cb = size_of::<STARTUPINFOW>() as u32;
            startup.dwFlags = STARTF_USESTDHANDLES;
            startup.hStdOutput = stdout_write;
            startup.hStdError = stderr_write;
            startup.hStdInput = HANDLE::default();

            let mut proc_info = PROCESS_INFORMATION::default();

            // ------------------------------------------------------------------
            // 3. Spawn the process in suspended state.
            // ------------------------------------------------------------------
            // Use lpApplicationName only for absolute paths; for bare names
            // (e.g. "cmd.exe"), pass null so Windows searches PATH via the
            // command line token.
            let app_name_ptr = if spec.program.is_absolute() {
                PCWSTR(app_wide.as_ptr())
            } else {
                PCWSTR(ptr::null())
            };

            let spawn_result = CreateProcessW(
                app_name_ptr,
                Some(PWSTR(cmd_wide.as_mut_ptr())),
                None,
                None,
                true, // bInheritHandles — pipe write ends must be inherited
                CREATE_SUSPENDED | CREATE_NO_WINDOW,
                None, // inherit parent environment
                PCWSTR(working_dir_wide.as_ptr()),
                &startup,
                &mut proc_info,
            );

            // Close the write ends in the parent regardless of spawn success.
            // When the child exits, its copies close and ReadFile returns EOF.
            let _ = CloseHandle(stdout_write);
            let _ = CloseHandle(stderr_write);

            if let Err(e) = spawn_result {
                let _ = CloseHandle(stdout_read);
                let _ = CloseHandle(stderr_read);
                return Err(SandboxError::SpawnFailed {
                    reason: format!("CreateProcessW: {e}"),
                });
            }

            // RAII guards for process and thread handles.
            let process_handle = OwnedHandle(proc_info.hProcess);
            let thread_handle = OwnedHandle(proc_info.hThread);
            // Wrap pipe read ends in RAII so they are closed on any subsequent
            // error path (CreateJobObjectW / SetInformationJobObject /
            // AssignProcessToJobObject failures) without manual CloseHandle calls.
            let stdout_read = OwnedHandle(stdout_read);
            let stderr_read = OwnedHandle(stderr_read);

            // ------------------------------------------------------------------
            // 4. Create a job object with KILL_ON_JOB_CLOSE and assign the
            //    suspended process to it before resuming.
            // ------------------------------------------------------------------
            let job = CreateJobObjectW(None, PCWSTR(ptr::null())).map_err(|e| {
                let _ = TerminateProcess(proc_info.hProcess, 1);
                SandboxError::PlatformUnavailable {
                    platform: "windows".to_string(),
                    reason: format!("CreateJobObjectW: {e}"),
                }
            })?;
            let job_handle = OwnedHandle(job);

            let mut ext_info: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = zeroed();
            ext_info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;

            SetInformationJobObject(
                job,
                JobObjectExtendedLimitInformation,
                &ext_info as *const _ as *const core::ffi::c_void,
                size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
            )
            .map_err(|e| {
                let _ = TerminateProcess(proc_info.hProcess, 1);
                SandboxError::PlatformUnavailable {
                    platform: "windows".to_string(),
                    reason: format!("SetInformationJobObject: {e}"),
                }
            })?;

            AssignProcessToJobObject(job, proc_info.hProcess).map_err(|e| {
                let _ = TerminateProcess(proc_info.hProcess, 1);
                SandboxError::PlatformUnavailable {
                    platform: "windows".to_string(),
                    reason: format!("AssignProcessToJobObject: {e}"),
                }
            })?;

            // ------------------------------------------------------------------
            // 5. Resume the process and start draining its output in background
            //    threads (prevents pipe-buffer deadlock for larger outputs).
            // ------------------------------------------------------------------
            let resume_result = ResumeThread(proc_info.hThread);
            if resume_result == u32::MAX {
                return Err(SandboxError::SpawnFailed {
                    reason: "ResumeThread failed".to_string(),
                });
            }

            // Transfer read handles to drain threads. into_raw() relinquishes
            // RAII ownership; drain_pipe is responsible for CloseHandle.
            let stdout_usize = stdout_read.into_raw().0 as usize;
            let stderr_usize = stderr_read.into_raw().0 as usize;
            let stdout_thread = std::thread::spawn(move || drain_pipe(stdout_usize));
            let stderr_thread = std::thread::spawn(move || drain_pipe(stderr_usize));

            // ------------------------------------------------------------------
            // 6. Wait with timeout.
            // ------------------------------------------------------------------
            let timeout_ms = spec.timeout.as_millis().clamp(0, u32::MAX as u128) as u32;
            let wait_result = WaitForSingleObject(proc_info.hProcess, timeout_ms);

            // WAIT_EVENT(258) = WAIT_TIMEOUT from the Windows SDK.
            let timed_out = wait_result.0 == WAIT_TIMEOUT_VAL;

            if timed_out {
                let _ = TerminateProcess(proc_info.hProcess, 1);
                // Wait for the process to actually terminate so handles are valid.
                WaitForSingleObject(proc_info.hProcess, WIN_INFINITE);
            }

            // ------------------------------------------------------------------
            // 7. Collect exit code and pipe output.
            // ------------------------------------------------------------------
            let mut exit_code_raw: u32 = 0;
            let _ = GetExitCodeProcess(proc_info.hProcess, &mut exit_code_raw);

            // Kill all remaining job processes FIRST so their pipe write ends close,
            // then join drain threads (which will see EOF and finish promptly).
            // If we joined first, a descendant that inherited the pipe write handle
            // (e.g. ping spawned by cmd.exe) would keep the drain thread blocked
            // indefinitely waiting for EOF, because job_handle — and therefore
            // KILL_ON_JOB_CLOSE — would not fire until after the join.
            drop(job_handle);

            let stdout = stdout_thread.join().unwrap_or_default();
            let stderr = stderr_thread.join().unwrap_or_default();

            drop(process_handle);
            drop(thread_handle);

            let exit_code = if timed_out {
                None
            } else {
                Some(exit_code_raw as i32)
            };

            Ok(SandboxedCommandOutput {
                exit_code,
                stdout,
                stderr,
                enforcement: SandboxEnforcementReport {
                    // SIMPLIFICATION: job-object-only (no restricted token).
                    // Filesystem writes are NOT blocked by the OS mechanism in
                    // use.  The enforcement report is honest about this.
                    filesystem_write_enforced: false,
                    filesystem_read_enforced: false,
                    network_enforced: false,
                    caveat_labels: vec![
                        "windows-no-restricted-token".to_string(),
                        "windows-no-network-enforcement".to_string(),
                        "windows-no-filesystem-enforcement".to_string(),
                    ],
                    backend_used: "job-object-kill-on-close".to_string(),
                },
                timed_out,
            })
        }
    }
}
