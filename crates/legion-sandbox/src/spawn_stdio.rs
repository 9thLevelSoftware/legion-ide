//! Long-lived sandboxed process with inherited stdio pipes (C4 / DAP adapters).
//!
//! Unlike [`crate::spawn::spawn_sandboxed`], this API returns a running
//! [`std::process::Child`] with stdin/stdout handles for interactive protocols
//! (Microsoft DAP). Enforcement is best-effort and reported honestly.

use std::collections::BTreeSet;
use std::path::PathBuf;
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

use crate::SandboxError;
use crate::spawn::SandboxEnforcementReport;

/// Spec for a long-lived sandboxed stdio process.
#[derive(Debug, Clone)]
pub struct SandboxStdioSpec {
    /// Program to execute.
    pub program: PathBuf,
    /// Arguments.
    pub args: Vec<String>,
    /// Working directory.
    pub working_dir: PathBuf,
    /// Writable root for FS-write isolation (when the platform enforces it).
    pub writable_root: PathBuf,
    /// Allowed network egress (empty = deny-all where enforced).
    pub allowed_egress: BTreeSet<String>,
    /// Extra environment pairs.
    pub env: Vec<(String, String)>,
}

/// Running sandboxed child with DAP-friendly stdio.
#[derive(Debug)]
pub struct SandboxedStdioProcess {
    child: Child,
    /// Child stdin (write end).
    pub stdin: ChildStdin,
    /// Child stdout (read end).
    pub stdout: ChildStdout,
    /// Honest enforcement report.
    pub enforcement: SandboxEnforcementReport,
    /// Platform lifetime guard (e.g. Windows job object).
    _guard: PlatformGuard,
}

impl SandboxedStdioProcess {
    /// Split into process + pipes + report (for `LiveDapSession::from_stdio`).
    pub fn into_parts(
        self,
    ) -> (
        Child,
        ChildStdin,
        ChildStdout,
        SandboxEnforcementReport,
        PlatformGuard,
    ) {
        (
            self.child,
            self.stdin,
            self.stdout,
            self.enforcement,
            self._guard,
        )
    }
}

/// Opaque platform lifetime handle (must outlive the child for kill-on-close).
#[derive(Debug, Default)]
pub struct PlatformGuard {
    #[cfg(windows)]
    _job: Option<WindowsJobGuard>,
}

#[cfg(windows)]
struct WindowsJobGuard(isize); // HANDLE as isize

#[cfg(windows)]
impl Drop for WindowsJobGuard {
    fn drop(&mut self) {
        use windows::Win32::Foundation::{CloseHandle, HANDLE};
        unsafe {
            let _ = CloseHandle(HANDLE(self.0 as *mut core::ffi::c_void));
        }
    }
}

#[cfg(windows)]
impl std::fmt::Debug for WindowsJobGuard {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WindowsJobGuard").finish_non_exhaustive()
    }
}

/// Spawn a long-lived child under the best available OS sandbox for stdio protocols.
pub fn spawn_sandboxed_stdio(
    spec: &SandboxStdioSpec,
) -> Result<SandboxedStdioProcess, SandboxError> {
    #[cfg(target_os = "linux")]
    let result = linux_stdio(spec);
    #[cfg(target_os = "macos")]
    let result = macos_stdio(spec);
    #[cfg(windows)]
    let result = windows_stdio(spec);
    #[cfg(not(any(target_os = "linux", target_os = "macos", windows)))]
    let result = Err(SandboxError::PlatformUnavailable {
        platform: std::env::consts::OS.to_string(),
        reason: "no stdio sandbox implementation for this platform".to_string(),
    });
    result
}

fn base_command(spec: &SandboxStdioSpec) -> Command {
    let mut cmd = Command::new(&spec.program);
    cmd.args(&spec.args)
        .current_dir(&spec.working_dir)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    for (k, v) in &spec.env {
        cmd.env(k, v);
    }
    cmd
}

fn take_pipes(mut child: Child) -> Result<(Child, ChildStdin, ChildStdout), SandboxError> {
    let stdin = child
        .stdin
        .take()
        .ok_or_else(|| SandboxError::SpawnFailed {
            reason: "missing stdin pipe".to_string(),
        })?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| SandboxError::SpawnFailed {
            reason: "missing stdout pipe".to_string(),
        })?;
    Ok((child, stdin, stdout))
}

// ---------------------------------------------------------------------------
// Linux — Landlock write isolation (+ optional bwrap net) for long-lived child
// ---------------------------------------------------------------------------

#[cfg(target_os = "linux")]
fn linux_stdio(spec: &SandboxStdioSpec) -> Result<SandboxedStdioProcess, SandboxError> {
    use landlock::{
        ABI, AccessFs, BitFlags, CompatLevel, Compatible, PathBeneath, PathFd, Ruleset,
        RulesetAttr, RulesetCreatedAttr,
    };
    use std::io;
    use std::os::unix::process::CommandExt;
    use std::process::Stdio as ProcStdio;

    const WRITE_POLICY_ABI: ABI = ABI::V5;
    let write_access = AccessFs::from_write(WRITE_POLICY_ABI);

    Ruleset::default()
        .set_compatibility(CompatLevel::HardRequirement)
        .handle_access(write_access)
        .map_err(|e| SandboxError::PlatformUnavailable {
            platform: "linux".to_string(),
            reason: format!("Landlock write rights unavailable: {e}"),
        })?
        .create()
        .map_err(|e| SandboxError::PlatformUnavailable {
            platform: "linux".to_string(),
            reason: format!("Landlock create failed: {e}"),
        })?;

    let deny_all_network = spec.allowed_egress.is_empty();
    let bwrap = if deny_all_network {
        ["bwrap", "/usr/bin/bwrap", "/bin/bwrap"]
            .into_iter()
            .find(|p| {
                Command::new(p)
                    .arg("--version")
                    .stdout(ProcStdio::null())
                    .stderr(ProcStdio::null())
                    .status()
                    .map(|s| s.success())
                    .unwrap_or(false)
            })
            .map(PathBuf::from)
    } else {
        None
    };
    let network_enforced = deny_all_network && bwrap.is_some();

    let writable_root = spec.writable_root.clone();
    let mut cmd = if let Some(ref bwrap_bin) = bwrap {
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
            .args(&spec.args)
            .stdin(ProcStdio::piped())
            .stdout(ProcStdio::piped())
            .stderr(ProcStdio::null());
        for (k, v) in &spec.env {
            c.env(k, v);
        }
        c
    } else {
        base_command(spec)
    };

    unsafe {
        cmd.pre_exec(move || {
            let write_access_child = AccessFs::from_write(WRITE_POLICY_ABI);
            let path_fd =
                PathFd::new(&writable_root).map_err(|e| io::Error::other(format!("{e}")))?;
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

    let child = cmd.spawn().map_err(|e| SandboxError::SpawnFailed {
        reason: e.to_string(),
    })?;
    let (child, stdin, stdout) = take_pipes(child)?;

    let mut caveats = Vec::new();
    if !network_enforced {
        if deny_all_network {
            caveats.push("bwrap-unshare-net-unavailable".to_string());
        } else {
            caveats.push("linux-egress-allowlist-not-implemented".to_string());
        }
    }
    let backend = if network_enforced {
        format!(
            "landlock-v{}+bwrap-unshare-net+stdio",
            WRITE_POLICY_ABI as u32
        )
    } else {
        format!("landlock-v{}+stdio", WRITE_POLICY_ABI as u32)
    };

    Ok(SandboxedStdioProcess {
        child,
        stdin,
        stdout,
        enforcement: SandboxEnforcementReport {
            filesystem_write_enforced: true,
            filesystem_read_enforced: false,
            network_enforced,
            caveat_labels: caveats,
            backend_used: backend,
        },
        _guard: PlatformGuard::default(),
    })
}

// ---------------------------------------------------------------------------
// macOS — sandbox-exec with piped stdio
// ---------------------------------------------------------------------------

#[cfg(target_os = "macos")]
fn macos_stdio(spec: &SandboxStdioSpec) -> Result<SandboxedStdioProcess, SandboxError> {
    use crate::spawn::generate_sbpl_profile;

    let sandbox_exec = std::path::Path::new("/usr/bin/sandbox-exec");
    if !sandbox_exec.exists() {
        return Err(SandboxError::PlatformUnavailable {
            platform: "macos".to_string(),
            reason: "sandbox-exec not found".to_string(),
        });
    }
    let sbpl = generate_sbpl_profile(&spec.writable_root, &spec.allowed_egress);
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
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    for (k, v) in &spec.env {
        cmd.env(k, v);
    }

    let child = cmd.spawn().map_err(|e| SandboxError::SpawnFailed {
        reason: e.to_string(),
    })?;
    let (child, stdin, stdout) = take_pipes(child)?;

    Ok(SandboxedStdioProcess {
        child,
        stdin,
        stdout,
        enforcement: SandboxEnforcementReport {
            filesystem_write_enforced: true,
            filesystem_read_enforced: false,
            network_enforced: true,
            caveat_labels: vec![],
            backend_used: "seatbelt-sbpl+stdio".to_string(),
        },
        _guard: PlatformGuard::default(),
    })
}

// ---------------------------------------------------------------------------
// Windows — std::process pipes + job-object kill-on-close when assignable
// ---------------------------------------------------------------------------

#[cfg(windows)]
fn windows_stdio(spec: &SandboxStdioSpec) -> Result<SandboxedStdioProcess, SandboxError> {
    use windows::Win32::Foundation::CloseHandle;
    use windows::Win32::System::JobObjects::{
        AssignProcessToJobObject, CreateJobObjectW, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
        JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JobObjectExtendedLimitInformation,
        SetInformationJobObject,
    };
    use windows::Win32::System::Threading::{
        OpenProcess, PROCESS_QUERY_INFORMATION, PROCESS_SET_QUOTA, PROCESS_TERMINATE,
    };
    use windows::core::PCWSTR;

    let mut cmd = base_command(spec);
    let child = cmd.spawn().map_err(|e| SandboxError::SpawnFailed {
        reason: e.to_string(),
    })?;
    let pid = child.id();
    let (child, stdin, stdout) = take_pipes(child)?;

    let mut job_guard = None;
    let mut backend = "windows-stdio-no-job".to_string();
    let mut caveats = vec![
        "windows-no-restricted-token".to_string(),
        "windows-no-network-enforcement".to_string(),
        "windows-no-filesystem-enforcement".to_string(),
    ];

    unsafe {
        let access = PROCESS_TERMINATE | PROCESS_SET_QUOTA | PROCESS_QUERY_INFORMATION;
        if let Ok(proc) = OpenProcess(access, false, pid) {
            if let Ok(job) = CreateJobObjectW(None, PCWSTR::null()) {
                let mut ext_info: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = std::mem::zeroed();
                ext_info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
                if SetInformationJobObject(
                    job,
                    JobObjectExtendedLimitInformation,
                    &ext_info as *const _ as *const core::ffi::c_void,
                    std::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
                )
                .is_ok()
                    && AssignProcessToJobObject(job, proc).is_ok()
                {
                    job_guard = Some(WindowsJobGuard(job.0 as isize));
                    backend = "job-object-kill-on-close+stdio".to_string();
                    caveats.push("windows-job-assigned-after-spawn".to_string());
                } else {
                    let _ = CloseHandle(job);
                    caveats.push("windows-job-assign-failed".to_string());
                }
            }
            let _ = CloseHandle(proc);
        } else {
            caveats.push("windows-open-process-failed".to_string());
        }
    }

    Ok(SandboxedStdioProcess {
        child,
        stdin,
        stdout,
        enforcement: SandboxEnforcementReport {
            filesystem_write_enforced: false,
            filesystem_read_enforced: false,
            network_enforced: false,
            caveat_labels: caveats,
            backend_used: backend,
        },
        _guard: PlatformGuard { _job: job_guard },
    })
}
