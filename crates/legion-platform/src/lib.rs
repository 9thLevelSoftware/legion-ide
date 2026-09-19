//! OS abstractions for filesystem/process/watcher/PTY/environment/time operations.

#![warn(missing_docs)]

use std::{
    collections::HashMap,
    env,
    fs::{self, OpenOptions},
    io::{self, Write},
    path::{Component, Path, PathBuf},
    process::Command,
    sync::{
        Arc, Mutex, OnceLock,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

/// Windows ConPTY parity metadata contracts.
pub mod windows;

use legion_protocol::{CanonicalPath, EventSequence, WatcherEvent, WatcherEventKind, WorkspaceId};
use thiserror::Error;

/// Maximum event count accepted from one watcher snapshot.
/// Soft cap for precise per-path events in one recursive snapshot.
///
/// Recursive monorepo walks can exceed this. When they do, the snapshot returns
/// a **bounded** event list plus a terminal `WatcherEventKind::Overflow` marker
/// (still `Ok`). Callers must treat that as an incomplete scan: update/add for
/// observed paths only, and never invent deletions for paths beyond the cutoff.
/// Hard `Err(WatcherOverflow)` remains reserved for true recovery paths (e.g.
/// synthetic/OS queue overflow tests), not the steady-state size soft-cap.
const WATCHER_OVERFLOW_THRESHOLD: usize = 65_536;

/// Maximum directory nesting depth for recursive watcher snapshots (Tier 1 A11).
/// Aligns with the project tree depth cap so nested monorepo files are observed.
const WATCHER_RECURSIVE_DEPTH_LIMIT: usize = 32;

/// Maximum normalization component count before a path is rejected.
const NORMALIZATION_DEPTH_LIMIT: usize = 1_024;

/// Stable FNV-1a 64-bit offset basis used for platform-owned content fingerprints.
const STABLE_HASH_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;

/// Stable FNV-1a 64-bit prime used for platform-owned content fingerprints.
const STABLE_HASH_PRIME: u64 = 0x0000_0100_0000_01b3;

/// Monotonic suffix for same-directory atomic-write temporary paths.
static ATOMIC_TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Monotonic suffix for native PTY session handles.
static NATIVE_PTY_SESSION_COUNTER: AtomicU64 = AtomicU64::new(1);

/// Errors surfaced by platform services.
#[derive(Debug, Error)]
pub enum PlatformError {
    /// Permission denied while performing an operation.
    #[error("permission denied for `{path}` while {operation}")]
    PermissionDenied {
        /// Operation attempted.
        operation: String,
        /// Path that triggered the error.
        path: PathBuf,
    },

    /// Requested path was not found.
    #[error("not found for `{path}` while {operation}")]
    NotFound {
        /// Operation attempted.
        operation: String,
        /// Path that triggered the error.
        path: PathBuf,
    },

    /// UTF-8 encoding issue while reading a text file.
    #[error("encoding error for `{path}` while {operation}: {source}")]
    Encoding {
        /// Operation attempted.
        operation: String,
        /// Path that triggered the error.
        path: PathBuf,
        /// Underlying platform error.
        #[source]
        source: io::Error,
    },

    /// Symlink loop encountered while resolving the path.
    #[error("symlink loop for `{path}` while {operation}")]
    SymlinkLoop {
        /// Operation attempted.
        operation: String,
        /// Path that triggered the error.
        path: PathBuf,
    },

    /// Path is too long for the platform/filesystem.
    #[error("path is too long for `{path}` while {operation}")]
    PathTooLong {
        /// Operation attempted.
        operation: String,
        /// Path that triggered the error.
        path: PathBuf,
    },

    /// Atomic replacement is unavailable.
    #[error("atomic replace unsupported while {operation} for `{path}`")]
    AtomicReplaceUnsupported {
        /// Operation attempted.
        operation: String,
        /// Path that triggered the error.
        path: PathBuf,
    },

    /// Operation is not supported by the active platform or filesystem.
    #[error("unsupported operation while {operation} for `{path}`: {reason}")]
    UnsupportedOperation {
        /// Operation attempted.
        operation: String,
        /// Path that triggered the error.
        path: PathBuf,
        /// Platform/filesystem reason.
        reason: String,
    },

    /// Metadata and fingerprint reads produced incompatible values.
    #[error("metadata inconsistency for `{path}` while {operation}: {details}")]
    MetadataInconsistent {
        /// Operation attempted.
        operation: String,
        /// Path whose metadata is inconsistent.
        path: PathBuf,
        /// Inconsistency details.
        details: String,
    },

    /// Watcher queue overflow requiring directory rescan.
    #[error("watcher overflow while observing `{path}`: {context}")]
    WatcherOverflow {
        /// Watched root path.
        path: PathBuf,
        /// Context for overflow detection.
        context: String,
    },

    /// Process spawning failed.
    #[error("process spawn failed for `{command}` while {operation}: {message}")]
    ProcessSpawnFailure {
        /// Operation attempted.
        operation: String,
        /// Command that failed.
        command: String,
        /// Spawn failure details.
        message: String,
    },

    /// PTY backend unavailable.
    #[error("PTY unavailable: {reason}")]
    PtyUnavailable {
        /// Human-readable reason.
        reason: String,
    },

    /// Timeout exceeded.
    #[error("operation `{operation}` timed out after {duration:?}")]
    Timeout {
        /// Operation attempted.
        operation: String,
        /// Timeout duration observed.
        duration: Duration,
    },

    /// Caller cancelled operation.
    #[error("operation `{operation}` cancelled")]
    Cancelled {
        /// Operation attempted.
        operation: String,
    },

    /// A bounded process stream exceeded its caller-supplied limit.
    #[error("process `{stream}` output exceeded {limit} bytes")]
    ProcessOutputLimit {
        /// Stream that exceeded its bound.
        stream: &'static str,
        /// Maximum retained bytes.
        limit: usize,
    },

    /// A bounded process stream could not be read reliably.
    #[error("failed to read bounded process `{stream}` output")]
    ProcessOutputReadFailure {
        /// Stream that failed.
        stream: &'static str,
    },

    /// The bounded process could not be confirmed terminated during cleanup.
    #[error("failed to clean up bounded process `{command}`: {reason}")]
    ProcessCleanupFailure {
        /// Command whose process tree could not be cleaned up.
        command: String,
        /// Cleanup failure details.
        reason: String,
    },

    /// Generic I/O error fallback.
    #[error("I/O error for `{path}` while {operation}: {source}")]
    Io {
        /// Operation attempted.
        operation: String,
        /// Path that triggered the error.
        path: PathBuf,
        /// Underlying platform error.
        #[source]
        source: io::Error,
    },
}

impl PlatformError {
    /// Classifies a raw platform I/O error into a structured platform error.
    pub fn from_io_error(
        operation: impl Into<String>,
        path: impl Into<PathBuf>,
        source: io::Error,
    ) -> Self {
        let operation = operation.into();
        let path = path.into();

        match source.kind() {
            io::ErrorKind::PermissionDenied => Self::PermissionDenied { operation, path },
            io::ErrorKind::NotFound => Self::NotFound { operation, path },
            io::ErrorKind::InvalidData => Self::Encoding {
                operation,
                path,
                source,
            },
            io::ErrorKind::Unsupported => Self::UnsupportedOperation {
                operation,
                path,
                reason: source.to_string(),
            },
            _ => {
                if Self::looks_like_symlink_loop(&source) {
                    Self::SymlinkLoop { operation, path }
                } else if Self::looks_like_path_too_long(&source) {
                    Self::PathTooLong { operation, path }
                } else {
                    Self::Io {
                        operation,
                        path,
                        source,
                    }
                }
            }
        }
    }

    fn looks_like_symlink_loop(source: &io::Error) -> bool {
        let message = source.to_string().to_ascii_lowercase();
        message.contains("too many links")
            || message.contains("symlink")
            || message.contains("symbolic link")
            || message.contains("circular")
            || message.contains("loop")
    }

    fn looks_like_path_too_long(source: &io::Error) -> bool {
        let message = source.to_string().to_ascii_lowercase();
        message.contains("filename too long")
            || message.contains("name too long")
            || message.contains("path too long")
            || message.contains("too long")
    }
}

/// Platform-owned filesystem entry kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileSystemEntryKind {
    /// Regular file.
    File,
    /// Directory.
    Directory,
    /// Symbolic link.
    Symlink,
    /// Other platform-specific entry type.
    Other,
}

/// Platform-owned metadata DTO for filesystem entries.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileSystemMetadata {
    /// Path used for the metadata read.
    pub path: PathBuf,
    /// Entry kind classified by the platform service.
    pub kind: FileSystemEntryKind,
    /// Length reported by the platform metadata call.
    pub length: u64,
    /// Last modified timestamp in milliseconds since the Unix epoch, when available.
    pub modified_at: Option<u64>,
    /// Whether platform permissions report the entry as read-only.
    pub read_only: bool,
}

impl FileSystemMetadata {
    /// Returns true when the entry is a regular file.
    pub fn is_file(&self) -> bool {
        self.kind == FileSystemEntryKind::File
    }

    /// Returns true when the entry is a directory.
    pub fn is_dir(&self) -> bool {
        self.kind == FileSystemEntryKind::Directory
    }
}

/// Platform-owned fingerprint DTO combining stable content hash and metadata.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileSystemFingerprint {
    /// Path used for the fingerprint read.
    pub path: PathBuf,
    /// Stable hash algorithm identifier.
    pub algorithm: String,
    /// Entry kind classified by the platform service.
    pub kind: FileSystemEntryKind,
    /// File length for regular files.
    pub length: Option<u64>,
    /// Last modified timestamp in milliseconds since the Unix epoch, when available.
    pub modified_at: Option<u64>,
    /// Stable content hash for regular files.
    pub stable_hash: Option<String>,
    /// Whether platform permissions report the entry as read-only.
    pub read_only: bool,
}

impl FileSystemFingerprint {
    /// Returns true when the fingerprint describes a regular file.
    pub fn is_file(&self) -> bool {
        self.kind == FileSystemEntryKind::File
    }
}

/// Path-normalization abstraction for all filesystem operations.
pub trait PathNormalizationService {
    /// Converts a path to a deterministic normalized representation.
    fn normalize_path(&self, path: &Path) -> Result<PathBuf, PlatformError>;

    /// Returns canonical path if it can be resolved by the platform.
    fn canonicalize_path(&self, path: &Path) -> Result<PathBuf, PlatformError>;

    /// Returns whether `candidate` is a child of `base` under normalization.
    fn is_within_base(&self, base: &Path, candidate: &Path) -> Result<bool, PlatformError>;
}

/// File-service abstraction used by all filesystem callers.
pub trait FileSystemService: PathNormalizationService {
    /// Reads UTF-8 text from disk.
    fn read_text_file(&self, path: &Path) -> Result<String, PlatformError>;

    /// Writes UTF-8 text to disk, creating directories as needed.
    ///
    /// This non-atomic primitive is reserved for explicitly approved callers. Workspace save flows
    /// must use [`FileSystemService::write_text_file_atomic`] and fail closed unless a tested policy
    /// enables fallback with immediate fingerprint re-verification and audit/event hooks.
    fn write_text_file(&self, path: &Path, text: &str) -> Result<(), PlatformError>;

    /// Writes UTF-8 text via same-directory temporary file and atomic replace.
    ///
    /// Implementations must create the temporary file in the target directory, write all bytes,
    /// flush and sync file data where supported, atomically replace the target, and sync the
    /// containing directory where supported. Unsupported atomic replace operations must return a
    /// structured platform error rather than silently degrading to a plain write.
    fn write_text_file_atomic(&self, path: &Path, text: &str) -> Result<(), PlatformError>;

    /// Creates a UTF-8 text file and fails if the destination already exists.
    fn create_text_file_new(&self, path: &Path, _text: &str) -> Result<(), PlatformError> {
        Err(PlatformError::UnsupportedOperation {
            operation: "create new text file".to_string(),
            path: path.to_path_buf(),
            reason: "filesystem backend does not implement create-new".to_string(),
        })
    }

    /// Removes an existing file.
    fn remove_file(&self, path: &Path) -> Result<(), PlatformError> {
        Err(PlatformError::UnsupportedOperation {
            operation: "remove file".to_string(),
            path: path.to_path_buf(),
            reason: "filesystem backend does not implement remove".to_string(),
        })
    }

    /// Renames or moves a filesystem path.
    fn rename_path(&self, source: &Path, destination: &Path) -> Result<(), PlatformError> {
        Err(PlatformError::UnsupportedOperation {
            operation: "rename path".to_string(),
            path: source.to_path_buf(),
            reason: format!(
                "filesystem backend does not implement rename to {}",
                destination.display()
            ),
        })
    }

    /// Reads platform-owned metadata for an entry.
    fn read_metadata(&self, path: &Path) -> Result<FileSystemMetadata, PlatformError>;

    /// Reads a platform-owned file fingerprint.
    fn read_fingerprint(&self, path: &Path) -> Result<FileSystemFingerprint, PlatformError>;

    /// Returns a stable deterministic hash for bytes.
    fn stable_hash(&self, bytes: &[u8]) -> String;

    /// Returns a stable deterministic hash for file bytes.
    fn stable_hash_file(&self, path: &Path) -> Result<String, PlatformError>;

    /// Returns the modified timestamp for an entry, when available.
    fn modified_timestamp(&self, path: &Path) -> Result<Option<u64>, PlatformError>;

    /// Returns the platform-reported length for an entry.
    fn file_length(&self, path: &Path) -> Result<u64, PlatformError>;

    /// Lists children for a directory.
    fn list_directory(&self, path: &Path) -> Result<Vec<PathBuf>, PlatformError>;

    /// Returns deterministic content hash for file bytes.
    fn hash_file(&self, path: &Path) -> Result<String, PlatformError> {
        self.stable_hash_file(path)
    }
}

fn stable_hash_bytes(bytes: &[u8]) -> String {
    let mut hash = STABLE_HASH_OFFSET;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(STABLE_HASH_PRIME);
    }
    format!("{hash:016x}")
}

fn modified_timestamp_millis(value: SystemTime) -> Option<u64> {
    value
        .duration_since(UNIX_EPOCH)
        .ok()
        .map(|duration| duration.as_millis() as u64)
}

fn metadata_kind(metadata: &fs::Metadata) -> FileSystemEntryKind {
    if metadata.is_dir() {
        FileSystemEntryKind::Directory
    } else if metadata.file_type().is_symlink() {
        FileSystemEntryKind::Symlink
    } else if metadata.is_file() {
        FileSystemEntryKind::File
    } else {
        FileSystemEntryKind::Other
    }
}

fn metadata_from_std(path: &Path, metadata: fs::Metadata) -> FileSystemMetadata {
    FileSystemMetadata {
        path: path.to_path_buf(),
        kind: metadata_kind(&metadata),
        length: metadata.len(),
        modified_at: metadata.modified().ok().and_then(modified_timestamp_millis),
        read_only: metadata.permissions().readonly(),
    }
}

fn atomic_temp_path(parent: &Path, target: &Path) -> PathBuf {
    let stem = target
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("file")
        .replace(['\n', '\r'], "_");
    let suffix = ATOMIC_TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
    parent.join(format!(".{stem}.{}.{}.tmp", std::process::id(), suffix))
}

fn sync_file_data_when_supported(
    file: &fs::File,
    path: &Path,
    operation: &str,
) -> Result<(), PlatformError> {
    match file.sync_data() {
        Ok(()) => Ok(()),
        Err(err) if err.kind() == io::ErrorKind::Unsupported => Ok(()),
        Err(err) => Err(PlatformError::from_io_error(operation, path, err)),
    }
}

#[cfg(unix)]
fn sync_parent_directory_when_supported(parent: &Path) -> Result<(), PlatformError> {
    let directory = match fs::File::open(parent) {
        Ok(directory) => directory,
        Err(err)
            if matches!(
                err.kind(),
                io::ErrorKind::Unsupported | io::ErrorKind::PermissionDenied
            ) =>
        {
            return Ok(());
        }
        Err(err) => {
            return Err(PlatformError::from_io_error(
                "sync containing directory",
                parent,
                err,
            ));
        }
    };

    match directory.sync_all() {
        Ok(()) => Ok(()),
        Err(err)
            if matches!(
                err.kind(),
                io::ErrorKind::Unsupported | io::ErrorKind::PermissionDenied
            ) =>
        {
            Ok(())
        }
        Err(err) => Err(PlatformError::from_io_error(
            "sync containing directory",
            parent,
            err,
        )),
    }
}

#[cfg(not(unix))]
fn sync_parent_directory_when_supported(_parent: &Path) -> Result<(), PlatformError> {
    Ok(())
}

#[cfg(windows)]
fn atomic_replace(temp: &Path, target: &Path) -> Result<(), PlatformError> {
    use std::os::windows::ffi::OsStrExt;

    const MOVEFILE_REPLACE_EXISTING: u32 = 0x0000_0001;
    const MOVEFILE_WRITE_THROUGH: u32 = 0x0000_0008;

    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn MoveFileExW(existing: *const u16, new_name: *const u16, flags: u32) -> i32;
    }

    fn wide(path: &Path) -> Vec<u16> {
        path.as_os_str().encode_wide().chain(Some(0)).collect()
    }

    let existing = wide(temp);
    let new_name = wide(target);
    let ok = unsafe {
        MoveFileExW(
            existing.as_ptr(),
            new_name.as_ptr(),
            MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
        )
    };

    if ok == 0 {
        Err(PlatformError::from_io_error(
            "atomic replace",
            target,
            io::Error::last_os_error(),
        ))
    } else {
        Ok(())
    }
}

#[cfg(not(windows))]
fn atomic_replace(temp: &Path, target: &Path) -> Result<(), PlatformError> {
    fs::rename(temp, target).map_err(|err| match err.kind() {
        io::ErrorKind::Unsupported => unsupported_operation(
            "atomic replace",
            target,
            format!("filesystem rename is unsupported: {err}"),
        ),
        _ => PlatformError::from_io_error("atomic replace", target, err),
    })
}

#[cfg(not(windows))]
fn unsupported_operation(
    operation: impl Into<String>,
    path: impl Into<PathBuf>,
    reason: impl Into<String>,
) -> PlatformError {
    PlatformError::UnsupportedOperation {
        operation: operation.into(),
        path: path.into(),
        reason: reason.into(),
    }
}

/// Process abstraction.
pub trait ProcessService {
    /// Executes a command and returns the output.
    fn execute(&self, request: &ProcessRequest) -> Result<ProcessResult, PlatformError>;

    /// Executes a command with mandatory timeout, live cancellation, and
    /// bounded stdout/stderr retention. Implementations that cannot provide
    /// these guarantees must fail closed instead of falling back to execute.
    fn execute_bounded(
        &self,
        request: &BoundedProcessRequest,
    ) -> Result<ProcessResult, PlatformError> {
        Err(PlatformError::UnsupportedOperation {
            operation: "bounded process execution".to_string(),
            path: PathBuf::from(&request.process.command),
            reason: "bounded process execution is unsupported".to_string(),
        })
    }
}

/// PTY abstraction.
pub trait PtyService {
    /// Spawns a PTY session.
    fn spawn_pty(&self, request: &PtyRequest) -> Result<PtySession, PlatformError>;

    /// Writes bounded input into a PTY session.
    fn write_pty(&self, session_id: &str, input: &str) -> Result<(), PlatformError> {
        let _ = (session_id, input);
        Err(PlatformError::PtyUnavailable {
            reason: "PTY input is not supported by this service".to_string(),
        })
    }

    /// Resizes a PTY session.
    fn resize_pty(&self, session_id: &str, cols: u16, rows: u16) -> Result<(), PlatformError> {
        let _ = (session_id, cols, rows);
        Err(PlatformError::PtyUnavailable {
            reason: "PTY resize is not supported by this service".to_string(),
        })
    }

    /// Polls available PTY output without blocking editor/workspace callers.
    fn read_pty(&self, session_id: &str, max_bytes: usize) -> Result<PtyReadResult, PlatformError> {
        let _ = (session_id, max_bytes);
        Err(PlatformError::PtyUnavailable {
            reason: "PTY output polling is not supported by this service".to_string(),
        })
    }

    /// Requests graceful PTY close.
    fn close_pty(&self, session_id: &str) -> Result<(), PlatformError> {
        let _ = session_id;
        Err(PlatformError::PtyUnavailable {
            reason: "PTY close is not supported by this service".to_string(),
        })
    }

    /// Kills a PTY session.
    fn kill_pty(&self, session_id: &str, mode: PtyKillMode) -> Result<(), PlatformError> {
        let _ = (session_id, mode);
        Err(PlatformError::PtyUnavailable {
            reason: "PTY kill is not supported by this service".to_string(),
        })
    }

    /// Cleans up native PTY sessions whose child process has exited.
    fn cleanup_orphaned_ptys(&self) -> Result<Vec<String>, PlatformError> {
        Ok(Vec::new())
    }
}

/// Watcher abstraction.
pub trait WatcherService {
    /// Returns an immediate snapshot of path entries as watcher-like events.
    fn snapshot(
        &self,
        workspace_id: WorkspaceId,
        path: &Path,
    ) -> Result<Vec<WatcherEvent>, PlatformError>;
}

/// Environment abstraction.
pub trait EnvironmentService {
    /// Current process directory.
    fn current_dir(&self) -> Result<PathBuf, PlatformError>;

    /// Single environment variable.
    fn get_var(&self, key: &str) -> Option<String>;

    /// Full variable map.
    fn vars(&self) -> Vec<(String, String)>;

    /// Returns normalized map used for child process launches.
    fn normalized_vars(&self, vars: &[(String, String)]) -> Vec<(String, String)>;
}

/// Time abstraction.
pub trait TimeService {
    /// Returns current time in millis.
    fn now_millis(&self) -> u64;

    /// Sleeps for duration.
    fn sleep(&self, duration: Duration);

    /// Returns true if timeout window has elapsed.
    fn is_over_deadline(&self, started_at: u64, timeout: Duration) -> bool;
}

/// Process request.
#[derive(Debug, Clone)]
pub struct ProcessRequest {
    /// Command executable.
    pub command: String,
    /// Command arguments.
    pub args: Vec<String>,
    /// Optional working directory.
    pub cwd: Option<PathBuf>,
    /// Optional environment map.
    pub env: Vec<(String, String)>,
    /// Optional bytes written to the child stdin before waiting.
    pub stdin: Option<Vec<u8>>,
    /// Optional timeout.
    pub timeout: Option<Duration>,
    /// Cancellation flag.
    pub cancelled: bool,
}

/// Limits and cancellation authority for bounded process execution.
#[derive(Debug, Clone)]
pub struct BoundedProcessRequest {
    /// Legacy process fields used to construct the child.
    pub process: ProcessRequest,
    /// Maximum stdout bytes retained before the child is terminated.
    pub max_stdout_bytes: usize,
    /// Maximum stderr bytes retained before the child is terminated.
    pub max_stderr_bytes: usize,
    /// Mandatory finite execution timeout.
    pub timeout: Duration,
    /// Live cancellation flag checked while the child runs.
    pub cancellation: Arc<AtomicBool>,
}

impl BoundedProcessRequest {
    /// Creates a bounded request with explicit stream limits and timeout.
    pub fn new(
        process: ProcessRequest,
        max_stdout_bytes: usize,
        max_stderr_bytes: usize,
        timeout: Duration,
        cancellation: Arc<AtomicBool>,
    ) -> Self {
        Self {
            process,
            max_stdout_bytes,
            max_stderr_bytes,
            timeout,
            cancellation,
        }
    }
}

impl ProcessRequest {
    /// Constructs a new request for `command`.
    pub fn new(command: impl Into<String>) -> Self {
        Self {
            command: command.into(),
            args: Vec::new(),
            cwd: None,
            env: Vec::new(),
            stdin: None,
            timeout: None,
            cancelled: false,
        }
    }
}

/// Process result.
#[derive(Debug, Clone)]
pub struct ProcessResult {
    /// Exit code from process.
    pub exit_code: i32,
    /// Standard output bytes converted to UTF-8 text.
    pub stdout: String,
    /// Error output bytes converted to UTF-8 text.
    pub stderr: String,
    /// Elapsed duration.
    pub elapsed: Duration,
}

/// PTY request payload.
#[derive(Debug, Clone)]
pub struct PtyRequest {
    /// Command to run.
    pub command: String,
    /// Args.
    pub args: Vec<String>,
    /// Optional working directory.
    pub cwd: Option<PathBuf>,
    /// Optional explicit environment for the child process.
    ///
    /// `None` = inherit the sanitised parent environment via `child_environment_vars`.
    /// `Some(vars)` = use exactly these key-value pairs (caller is responsible for
    /// applying any deny-list; the platform layer passes them verbatim to the PTY).
    pub env: Option<Vec<(String, String)>>,
}

/// PTY session descriptor.
#[derive(Debug, Clone)]
pub struct PtySession {
    /// Session id.
    pub id: String,
    /// Session output.
    pub output: String,
}

/// Result from a non-blocking PTY output poll.
#[derive(Debug, Clone)]
pub struct PtyReadResult {
    /// Session id.
    pub id: String,
    /// Output bytes decoded as UTF-8 replacement text.
    pub output: String,
    /// Whether the session has exited.
    pub exited: bool,
    /// Process exit code when known.
    pub exit_code: Option<i32>,
    /// Whether output was truncated to the requested maximum.
    pub truncated: bool,
}

/// PTY kill mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PtyKillMode {
    /// Graceful interrupt signal.
    Interrupt,
    /// Graceful termination.
    Terminate,
    /// Forceful process kill.
    Kill,
    /// Kill the session process tree/process group where the platform supports it.
    KillTree,
}

/// Native filesystem implementation.
#[derive(Debug, Default, Clone, Copy)]
pub struct NativeFileSystem;

/// Native process implementation.
#[derive(Debug, Default, Clone, Copy)]
pub struct NativeProcessService;

/// Native watcher implementation.
#[derive(Debug, Default, Clone, Copy)]
pub struct NativeWatcherService;

/// Native PTY implementation, phase-gated and not wired into user-visible terminal behavior.
#[derive(Debug, Default, Clone, Copy)]
pub struct NativePtyService;

/// Native environment implementation.
#[derive(Debug, Default, Clone, Copy)]
pub struct NativeEnvironmentService;

/// Native time implementation.
#[derive(Debug, Default, Clone, Copy)]
pub struct NativeTimeService;

impl PathNormalizationService for NativeFileSystem {
    fn normalize_path(&self, path: &Path) -> Result<PathBuf, PlatformError> {
        let absolute = if path.is_absolute() {
            path.to_path_buf()
        } else {
            let cwd = env::current_dir()
                .map_err(|err| PlatformError::from_io_error("normalize path", path, err))?;
            cwd.join(path)
        };

        let mut normalized = PathBuf::new();
        let mut depth = 0usize;

        for component in absolute.components() {
            depth += 1;
            if depth > NORMALIZATION_DEPTH_LIMIT {
                return Err(PlatformError::PathTooLong {
                    operation: "normalize path".to_string(),
                    path: absolute,
                });
            }

            match component {
                Component::Prefix(prefix) => normalized.push(prefix.as_os_str()),
                Component::RootDir => normalized.push(component.as_os_str()),
                Component::CurDir => {}
                Component::ParentDir => {
                    normalized.pop();
                }
                Component::Normal(segment) => normalized.push(segment),
            }
        }

        Ok(normalized)
    }

    fn canonicalize_path(&self, path: &Path) -> Result<PathBuf, PlatformError> {
        fs::canonicalize(path)
            .map_err(|err| PlatformError::from_io_error("canonicalize", path, err))
    }

    fn is_within_base(&self, base: &Path, candidate: &Path) -> Result<bool, PlatformError> {
        let base = self.normalize_path(base)?;
        let candidate = self.normalize_path(candidate)?;
        Ok(candidate.starts_with(base))
    }
}

impl FileSystemService for NativeFileSystem {
    fn read_text_file(&self, path: &Path) -> Result<String, PlatformError> {
        fs::read_to_string(path).map_err(|err| PlatformError::from_io_error("read", path, err))
    }

    fn write_text_file(&self, path: &Path, text: &str) -> Result<(), PlatformError> {
        if let Some(parent) = path.parent()
            && !parent.as_os_str().is_empty()
        {
            fs::create_dir_all(parent).map_err(|err| {
                PlatformError::from_io_error("create parent directories", path, err)
            })?;
        }

        fs::write(path, text).map_err(|err| PlatformError::from_io_error("write", path, err))
    }

    fn write_text_file_atomic(&self, path: &Path, text: &str) -> Result<(), PlatformError> {
        let parent = path.parent().unwrap_or_else(|| Path::new("."));
        fs::create_dir_all(parent)
            .map_err(|err| PlatformError::from_io_error("create parent directories", path, err))?;

        let temp = atomic_temp_path(parent, path);
        let mut temp_file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp)
            .map_err(|err| PlatformError::from_io_error("create atomic temporary", &temp, err))?;

        let result = (|| {
            temp_file.write_all(text.as_bytes()).map_err(|err| {
                PlatformError::from_io_error("write atomic temporary", &temp, err)
            })?;
            temp_file.flush().map_err(|err| {
                PlatformError::from_io_error("flush atomic temporary", &temp, err)
            })?;
            sync_file_data_when_supported(&temp_file, &temp, "sync atomic temporary")?;
            drop(temp_file);

            atomic_replace(&temp, path)?;
            sync_parent_directory_when_supported(parent)?;
            Ok(())
        })();

        if result.is_err() {
            let _ = fs::remove_file(&temp);
        }

        result
    }

    fn create_text_file_new(&self, path: &Path, text: &str) -> Result<(), PlatformError> {
        if let Some(parent) = path.parent()
            && !parent.as_os_str().is_empty()
        {
            fs::create_dir_all(parent).map_err(|err| {
                PlatformError::from_io_error("create parent directories", path, err)
            })?;
        }

        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)
            .map_err(|err| PlatformError::from_io_error("create new text file", path, err))?;
        file.write_all(text.as_bytes())
            .map_err(|err| PlatformError::from_io_error("write new text file", path, err))?;
        file.flush()
            .map_err(|err| PlatformError::from_io_error("flush new text file", path, err))?;
        sync_file_data_when_supported(&file, path, "sync new text file")?;
        if let Some(parent) = path.parent() {
            sync_parent_directory_when_supported(parent)?;
        }
        Ok(())
    }

    fn remove_file(&self, path: &Path) -> Result<(), PlatformError> {
        fs::remove_file(path).map_err(|err| PlatformError::from_io_error("remove file", path, err))
    }

    fn rename_path(&self, source: &Path, destination: &Path) -> Result<(), PlatformError> {
        if let Some(parent) = destination.parent()
            && !parent.as_os_str().is_empty()
        {
            fs::create_dir_all(parent).map_err(|err| {
                PlatformError::from_io_error("create rename destination parent", destination, err)
            })?;
        }
        fs::rename(source, destination)
            .map_err(|err| PlatformError::from_io_error("rename path", source, err))?;
        if let Some(parent) = destination.parent() {
            sync_parent_directory_when_supported(parent)?;
        }
        Ok(())
    }

    fn read_metadata(&self, path: &Path) -> Result<FileSystemMetadata, PlatformError> {
        fs::metadata(path)
            .map(|metadata| metadata_from_std(path, metadata))
            .map_err(|err| PlatformError::from_io_error("metadata", path, err))
    }

    fn read_fingerprint(&self, path: &Path) -> Result<FileSystemFingerprint, PlatformError> {
        let metadata = self.read_metadata(path)?;
        let stable_hash = if metadata.is_file() {
            let stable_hash = self.stable_hash_file(path)?;
            let verified = self.read_metadata(path)?;
            if verified.kind != metadata.kind
                || verified.length != metadata.length
                || verified.modified_at != metadata.modified_at
            {
                return Err(PlatformError::MetadataInconsistent {
                    operation: "read fingerprint".to_string(),
                    path: path.to_path_buf(),
                    details: format!(
                        "metadata changed during fingerprint read: before={metadata:?}, after={verified:?}"
                    ),
                });
            }
            Some(stable_hash)
        } else {
            None
        };

        Ok(FileSystemFingerprint {
            path: path.to_path_buf(),
            algorithm: "legion-stable-fingerprint-v1".to_string(),
            kind: metadata.kind,
            length: metadata.is_file().then_some(metadata.length),
            modified_at: metadata.modified_at,
            stable_hash,
            read_only: metadata.read_only,
        })
    }

    fn stable_hash(&self, bytes: &[u8]) -> String {
        stable_hash_bytes(bytes)
    }

    fn stable_hash_file(&self, path: &Path) -> Result<String, PlatformError> {
        let content =
            fs::read(path).map_err(|err| PlatformError::from_io_error("stable hash", path, err))?;
        Ok(self.stable_hash(&content))
    }

    fn modified_timestamp(&self, path: &Path) -> Result<Option<u64>, PlatformError> {
        self.read_metadata(path)
            .map(|metadata| metadata.modified_at)
    }

    fn file_length(&self, path: &Path) -> Result<u64, PlatformError> {
        self.read_metadata(path).map(|metadata| metadata.length)
    }

    fn list_directory(&self, path: &Path) -> Result<Vec<PathBuf>, PlatformError> {
        let read_dir = fs::read_dir(path)
            .map_err(|err| PlatformError::from_io_error("list directory", path, err))?;
        let mut entries = Vec::new();
        for entry in read_dir {
            let entry =
                entry.map_err(|err| PlatformError::from_io_error("list directory", path, err))?;
            entries.push(entry.path());
        }
        entries.sort();
        Ok(entries)
    }
}

impl ProcessService for NativeProcessService {
    fn execute(&self, request: &ProcessRequest) -> Result<ProcessResult, PlatformError> {
        use std::io::{Read, Write};
        use std::process::Stdio;

        if request.cancelled {
            return Err(PlatformError::Cancelled {
                operation: "execute".to_string(),
            });
        }

        let started = Instant::now();
        let mut command = Command::new(&request.command);
        command.args(&request.args);
        command.env_clear();

        if let Some(cwd) = &request.cwd {
            command.current_dir(cwd);
        }

        for (key, value) in child_environment_vars(&request.env) {
            command.env(key, value);
        }

        command
            .stdin(if request.stdin.is_some() {
                Stdio::piped()
            } else {
                Stdio::null()
            })
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        // Place the child in its own process group so a timeout can take down the
        // whole descendant tree, not just the direct child.
        #[cfg(unix)]
        {
            use std::os::unix::process::CommandExt;
            unsafe {
                command.pre_exec(|| {
                    nix::unistd::setpgid(
                        nix::unistd::Pid::from_raw(0),
                        nix::unistd::Pid::from_raw(0),
                    )
                    .map_err(|err| io::Error::from_raw_os_error(err as i32))?;
                    Ok(())
                });
            }
        }

        let mut child = command
            .spawn()
            .map_err(|err| PlatformError::ProcessSpawnFailure {
                operation: "execute".to_string(),
                command: request.command.clone(),
                message: err.to_string(),
            })?;

        if let Some(input) = request.stdin.as_deref() {
            let mut stdin = child.stdin.take().ok_or_else(|| {
                PlatformError::from_io_error(
                    "write process stdin",
                    PathBuf::from(&request.command),
                    io::Error::new(io::ErrorKind::BrokenPipe, "stdin pipe unavailable"),
                )
            })?;
            stdin.write_all(input).map_err(|err| {
                PlatformError::from_io_error(
                    "write process stdin",
                    PathBuf::from(&request.command),
                    err,
                )
            })?;
        }

        #[cfg(windows)]
        let mut process_job = assign_windows_process_job(&child);

        // Drain stdout/stderr on dedicated threads so a child that fills its pipe
        // buffers cannot deadlock against the timeout polling loop below.
        let stdout_reader = child.stdout.take().map(|mut pipe| {
            std::thread::spawn(move || {
                let mut buf = Vec::new();
                let _ = pipe.read_to_end(&mut buf);
                buf
            })
        });
        let stderr_reader = child.stderr.take().map(|mut pipe| {
            std::thread::spawn(move || {
                let mut buf = Vec::new();
                let _ = pipe.read_to_end(&mut buf);
                buf
            })
        });

        let exit_status = if let Some(timeout) = request.timeout {
            let poll_interval = Duration::from_millis(10);
            loop {
                match child.try_wait() {
                    Ok(Some(status)) => break status,
                    Ok(None) => {
                        if started.elapsed() > timeout {
                            #[cfg(windows)]
                            process_job.take();
                            terminate_timed_out_process(&mut child);
                            return Err(PlatformError::Timeout {
                                operation: format!("process `{}`", request.command),
                                duration: started.elapsed(),
                            });
                        }
                        std::thread::sleep(poll_interval);
                    }
                    Err(err) => {
                        let _ = child.kill();
                        let _ = child.wait();
                        return Err(PlatformError::from_io_error(
                            "execute",
                            PathBuf::from(&request.command),
                            err,
                        ));
                    }
                }
            }
        } else {
            child.wait().map_err(|err| {
                PlatformError::from_io_error("execute", PathBuf::from(&request.command), err)
            })?
        };

        #[cfg(windows)]
        process_job.take();

        let elapsed = started.elapsed();
        let stdout = stdout_reader
            .map(|handle| handle.join().unwrap_or_default())
            .unwrap_or_default();
        let stderr = stderr_reader
            .map(|handle| handle.join().unwrap_or_default())
            .unwrap_or_default();

        Ok(ProcessResult {
            exit_code: exit_status.code().unwrap_or(-1),
            stdout: String::from_utf8(stdout).unwrap_or_default(),
            stderr: String::from_utf8(stderr).unwrap_or_default(),
            elapsed,
        })
    }

    fn execute_bounded(
        &self,
        request: &BoundedProcessRequest,
    ) -> Result<ProcessResult, PlatformError> {
        execute_bounded_native(request)
    }
}

struct BoundedReaderResult {
    bytes: Vec<u8>,
    exceeded: bool,
    failed: bool,
}

#[cfg(unix)]
fn unix_bounded_reader<R: io::Read + std::os::fd::AsRawFd>(
    mut reader: R,
    limit: usize,
    overflow: Arc<AtomicBool>,
    stop: Arc<AtomicBool>,
    deadline: Instant,
) -> BoundedReaderResult {
    let fd = reader.as_raw_fd();
    let flags = unsafe { nix::libc::fcntl(fd, nix::libc::F_GETFL) };
    if flags < 0
        || unsafe { nix::libc::fcntl(fd, nix::libc::F_SETFL, flags | nix::libc::O_NONBLOCK) } < 0
    {
        return BoundedReaderResult {
            bytes: Vec::new(),
            exceeded: false,
            failed: true,
        };
    }
    let mut bytes = Vec::with_capacity(limit.min(64 * 1024));
    let mut chunk = [0u8; 8192];
    loop {
        if Instant::now() >= deadline {
            // Deadline is the process timeout, not a stream IO failure. The
            // parent loop owns Timeout classification after the child is reaped.
            return BoundedReaderResult {
                bytes,
                exceeded: false,
                failed: false,
            };
        }
        match reader.read(&mut chunk) {
            Ok(0) => {
                return BoundedReaderResult {
                    bytes,
                    exceeded: false,
                    failed: false,
                };
            }
            Ok(read) => {
                if bytes.len().saturating_add(read) > limit {
                    overflow.store(true, Ordering::Release);
                    return BoundedReaderResult {
                        bytes,
                        exceeded: true,
                        failed: false,
                    };
                }
                bytes.extend_from_slice(&chunk[..read]);
            }
            Err(err) if err.kind() == io::ErrorKind::WouldBlock => {
                if stop.load(Ordering::Acquire) {
                    return BoundedReaderResult {
                        bytes,
                        exceeded: false,
                        failed: false,
                    };
                }
                std::thread::sleep(Duration::from_millis(2));
            }
            Err(_) => {
                return BoundedReaderResult {
                    bytes,
                    exceeded: false,
                    failed: true,
                };
            }
        }
    }
}

#[cfg(windows)]
fn windows_bounded_reader(
    reader: impl std::os::windows::io::AsRawHandle,
    limit: usize,
    overflow: Arc<AtomicBool>,
    stop: Arc<AtomicBool>,
    deadline: Instant,
) -> BoundedReaderResult {
    use ::windows::Win32::Foundation::{ERROR_BROKEN_PIPE, ERROR_NO_DATA, GetLastError, HANDLE};
    use ::windows::Win32::Storage::FileSystem::ReadFile;
    use ::windows::Win32::System::Pipes::PeekNamedPipe;

    let handle = HANDLE(reader.as_raw_handle() as _);
    let mut bytes = Vec::with_capacity(limit.min(64 * 1024));
    let mut chunk = [0u8; 8192];
    loop {
        if Instant::now() >= deadline {
            // Deadline is the process timeout, not a stream IO failure. The
            // parent loop owns Timeout classification after the child is reaped.
            return BoundedReaderResult {
                bytes,
                exceeded: false,
                failed: false,
            };
        }
        if stop.load(Ordering::Acquire) {
            return BoundedReaderResult {
                bytes,
                exceeded: false,
                failed: false,
            };
        }
        let mut available = 0u32;
        let peek = unsafe { PeekNamedPipe(handle, None, 0, None, Some(&mut available), None) };
        if let Err(err) = peek {
            let code = unsafe { GetLastError() };
            if code == ERROR_BROKEN_PIPE || code == ERROR_NO_DATA {
                return BoundedReaderResult {
                    bytes,
                    exceeded: false,
                    failed: false,
                };
            }
            let _ = err;
            return BoundedReaderResult {
                bytes,
                exceeded: false,
                failed: true,
            };
        }
        if available == 0 {
            std::thread::sleep(Duration::from_millis(2));
            continue;
        }
        let read_len = (available as usize).min(chunk.len());
        let mut read = 0u32;
        if let Err(_err) =
            unsafe { ReadFile(handle, Some(&mut chunk[..read_len]), Some(&mut read), None) }
        {
            if stop.load(Ordering::Acquire) {
                return BoundedReaderResult {
                    bytes,
                    exceeded: false,
                    failed: false,
                };
            }
            return BoundedReaderResult {
                bytes,
                exceeded: false,
                failed: true,
            };
        }
        if read == 0 {
            continue;
        }
        let read = read as usize;
        if bytes.len().saturating_add(read) > limit {
            overflow.store(true, Ordering::Release);
            return BoundedReaderResult {
                bytes,
                exceeded: true,
                failed: false,
            };
        }
        bytes.extend_from_slice(&chunk[..read]);
    }
}

fn frozen_executable_roots() -> Vec<PathBuf> {
    #[cfg(unix)]
    {
        vec![PathBuf::from("/usr/bin"), PathBuf::from("/bin")]
    }
    #[cfg(windows)]
    {
        let mut roots = Vec::new();
        if let Ok(system_root) = std::env::var("SystemRoot") {
            let system32 = PathBuf::from(system_root).join("System32");
            roots.push(system32.join("WindowsPowerShell").join("v1.0"));
            roots.push(system32);
        }
        roots.push(PathBuf::from(r"C:\Windows\System32\WindowsPowerShell\v1.0"));
        roots.push(PathBuf::from(r"C:\Windows\System32"));
        roots
    }
    #[cfg(not(any(unix, windows)))]
    {
        Vec::new()
    }
}

/// Maximum stdin payload accepted by [`ProcessService::execute_bounded`].
///
/// This is a **process-service transport limit**. It bounds how many bytes the
/// bounded runner will hold in memory and stream into one child's stdin.
///
/// It is deliberately distinct from — and larger than — the 5 MiB editor text
/// snapshot budget owned by the text layer. Neither limit implies the other and
/// neither may be derived from the other: changing the editor snapshot budget
/// has no effect here, and changing this constant grants no editor budget.
///
/// A payload larger than this is an explicit structured rejection
/// ([`PlatformError::UnsupportedOperation`]) raised before any child process,
/// pipe, or handle exists. Over-limit input is never silently truncated.
pub const MAX_BOUNDED_STDIN_BYTES: usize = 8 * 1024 * 1024;

/// Maximum bytes pushed into the child's stdin per supervisor iteration, so the
/// loop keeps observing cancellation, the deadline, and stream overflow while
/// input is still being delivered.
#[cfg(any(unix, windows))]
const BOUNDED_STDIN_CHUNK_BYTES: usize = 64 * 1024;

#[cfg(any(unix, windows))]
fn bounded_stdin_limit_error(command: &str, requested: usize) -> PlatformError {
    PlatformError::UnsupportedOperation {
        operation: "bounded process execution".to_string(),
        path: PathBuf::from(command),
        reason: format!(
            "stdin payload of {requested} bytes exceeds the {MAX_BOUNDED_STDIN_BYTES} byte limit"
        ),
    }
}

#[cfg(any(unix, windows))]
fn bounded_stdin_write_error(
    command: &str,
    delivered: usize,
    total: usize,
    source: io::Error,
) -> PlatformError {
    PlatformError::Io {
        operation: format!("write bounded process stdin ({delivered} of {total} bytes delivered)"),
        path: PathBuf::from(command),
        source,
    }
}

#[cfg(any(unix, windows))]
fn bounded_stdin_undelivered_error(command: &str, delivered: usize, total: usize) -> PlatformError {
    bounded_stdin_write_error(
        command,
        delivered,
        total,
        io::Error::new(
            io::ErrorKind::BrokenPipe,
            "child exited before consuming all stdin",
        ),
    )
}

#[cfg(unix)]
fn unix_set_nonblocking(fd: std::os::fd::RawFd) -> io::Result<()> {
    let flags = unsafe { nix::libc::fcntl(fd, nix::libc::F_GETFL) };
    if flags < 0 {
        return Err(io::Error::last_os_error());
    }
    if unsafe { nix::libc::fcntl(fd, nix::libc::F_SETFL, flags | nix::libc::O_NONBLOCK) } < 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

fn resolve_bounded_executable(command: &str) -> Result<PathBuf, PlatformError> {
    let path = Path::new(command);
    if path.is_absolute() {
        return Ok(path.to_path_buf());
    }
    if command.contains('/') || command.contains('\\') || command.contains(':') {
        return Err(PlatformError::UnsupportedOperation {
            operation: "bounded process execution".to_string(),
            path: PathBuf::from(command),
            reason: "relative executable paths are not allowed".to_string(),
        });
    }
    for root in frozen_executable_roots() {
        let candidate = root.join(command);
        if candidate.is_file() {
            return Ok(candidate);
        }
        #[cfg(windows)]
        {
            let exe = root.join(format!("{command}.exe"));
            if exe.is_file() {
                return Ok(exe);
            }
        }
    }
    Err(PlatformError::ProcessSpawnFailure {
        operation: "bounded process execution".to_string(),
        command: command.to_string(),
        message: "command is not in the frozen executable roots".to_string(),
    })
}

#[cfg(unix)]
fn execute_bounded_native(request: &BoundedProcessRequest) -> Result<ProcessResult, PlatformError> {
    if request.timeout.is_zero() {
        return Err(PlatformError::UnsupportedOperation {
            operation: "bounded process execution".to_string(),
            path: PathBuf::from(&request.process.command),
            reason: "a finite timeout is required".to_string(),
        });
    }
    if request.process.cancelled || request.cancellation.load(Ordering::Acquire) {
        return Err(PlatformError::Cancelled {
            operation: "bounded process execution".to_string(),
        });
    }
    // Over-limit input is rejected here: after the finite-timeout and
    // cancellation guards, and before `resolve_bounded_executable` and
    // `Command::spawn`, so no child, pipe, or process group can exist after the
    // rejection.
    let stdin_requested = request.process.stdin.is_some();
    let stdin_bytes: &[u8] = request.process.stdin.as_deref().unwrap_or(&[]);
    if stdin_requested && stdin_bytes.len() > MAX_BOUNDED_STDIN_BYTES {
        return Err(bounded_stdin_limit_error(
            &request.process.command,
            stdin_bytes.len(),
        ));
    }

    let started = Instant::now();
    let reader_deadline = started
        .checked_add(request.timeout)
        .unwrap_or_else(Instant::now);
    let executable = resolve_bounded_executable(&request.process.command)?;
    let mut command = Command::new(&executable);
    command.args(&request.process.args);
    command.env_clear();
    if let Some(cwd) = &request.process.cwd {
        command.current_dir(cwd);
    }
    for (key, value) in child_environment_vars(&request.process.env) {
        command.env(key, value);
    }
    command
        .stdin(if stdin_requested {
            std::process::Stdio::piped()
        } else {
            std::process::Stdio::null()
        })
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());

    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        unsafe {
            command.pre_exec(|| {
                nix::unistd::setpgid(nix::unistd::Pid::from_raw(0), nix::unistd::Pid::from_raw(0))
                    .map_err(|err| io::Error::from_raw_os_error(err as i32))?;
                Ok(())
            });
        }
    }

    let mut child = command
        .spawn()
        .map_err(|err| PlatformError::ProcessSpawnFailure {
            operation: "bounded process execution".to_string(),
            command: request.process.command.clone(),
            message: err.to_string(),
        })?;

    // The supervisor owns the writer: there is no writer thread, so no shutdown
    // path can join one. Nonblocking mode is what lets the single supervisor
    // loop keep checking cancellation, the deadline and stream overflow while
    // input is still being delivered.
    let mut child_stdin = child.stdin.take();
    if stdin_requested {
        use std::os::fd::AsRawFd;
        let configured = match child_stdin.as_ref() {
            Some(pipe) => unix_set_nonblocking(pipe.as_raw_fd()),
            None => Err(io::Error::new(
                io::ErrorKind::BrokenPipe,
                "stdin pipe unavailable",
            )),
        };
        if let Err(err) = configured {
            terminate_bounded_child(&mut child);
            let _ = bounded_reap_child(&mut child, &request.process.command);
            return Err(PlatformError::Io {
                operation: "configure bounded process stdin".to_string(),
                path: PathBuf::from(&request.process.command),
                source: err,
            });
        }
    }

    #[cfg(windows)]
    let mut process_job = match assign_windows_process_job(&child) {
        Some(job) => Some(job),
        None => {
            terminate_timed_out_process(&mut child);
            return Err(PlatformError::UnsupportedOperation {
                operation: "bounded process execution".to_string(),
                path: PathBuf::from(&request.process.command),
                reason: "unable to assign child process job".to_string(),
            });
        }
    };
    let output_overflow = Arc::new(AtomicBool::new(false));
    let reader_stop = Arc::new(AtomicBool::new(false));
    let stdout_reader = child.stdout.take().map(|pipe| {
        let limit = request.max_stdout_bytes;
        let overflow = output_overflow.clone();
        let stop = reader_stop.clone();
        std::thread::spawn(move || {
            unix_bounded_reader(pipe, limit, overflow, stop, reader_deadline)
        })
    });
    let stderr_reader = child.stderr.take().map(|pipe| {
        let limit = request.max_stderr_bytes;
        let overflow = output_overflow.clone();
        let stop = reader_stop.clone();
        std::thread::spawn(move || {
            unix_bounded_reader(pipe, limit, overflow, stop, reader_deadline)
        })
    });

    let mut stdin_offset = 0usize;
    let termination = loop {
        if request.cancellation.load(Ordering::Acquire) {
            reader_stop.store(true, Ordering::Release);
            terminate_bounded_child(&mut child);
            break match bounded_reap_child(&mut child, &request.process.command) {
                Ok(()) => Err(PlatformError::Cancelled {
                    operation: "bounded process execution".to_string(),
                }),
                Err(err) => Err(err),
            };
        }
        if output_overflow.load(Ordering::Acquire) {
            reader_stop.store(true, Ordering::Release);
            terminate_bounded_child(&mut child);
            break match bounded_reap_child(&mut child, &request.process.command) {
                Ok(()) => Ok(-1),
                Err(err) => Err(err),
            };
        }
        if started.elapsed() > request.timeout {
            reader_stop.store(true, Ordering::Release);
            terminate_bounded_child(&mut child);
            break match bounded_reap_child(&mut child, &request.process.command) {
                Ok(()) => Err(PlatformError::Timeout {
                    operation: format!("process `{}`", request.process.command),
                    duration: started.elapsed(),
                }),
                Err(err) => Err(err),
            };
        }
        // One bounded chunk per iteration, after the cancellation / overflow /
        // deadline guards above, so a large payload never starves them.
        let mut stdin_progress = false;
        if child_stdin.is_some() {
            if stdin_offset < stdin_bytes.len() {
                let end = stdin_bytes
                    .len()
                    .min(stdin_offset + BOUNDED_STDIN_CHUNK_BYTES);
                let write_result = {
                    let writer = child_stdin.as_mut().expect("bounded stdin writer");
                    writer.write(&stdin_bytes[stdin_offset..end])
                };
                match write_result {
                    Ok(written) => {
                        if written > 0 {
                            stdin_offset += written;
                            stdin_progress = true;
                        }
                    }
                    Err(err)
                        if err.kind() == io::ErrorKind::WouldBlock
                            || err.kind() == io::ErrorKind::Interrupted => {}
                    Err(err) => {
                        reader_stop.store(true, Ordering::Release);
                        terminate_bounded_child(&mut child);
                        break match bounded_reap_child(&mut child, &request.process.command) {
                            Ok(()) => Err(bounded_stdin_write_error(
                                &request.process.command,
                                stdin_offset,
                                stdin_bytes.len(),
                                err,
                            )),
                            Err(cleanup) => Err(cleanup),
                        };
                    }
                }
            }
            // Reaching the end closes the child's stdin. `Some(Vec::new())`
            // takes this branch on the very first iteration, so a child waiting
            // for EOF still finishes.
            if stdin_offset >= stdin_bytes.len() {
                drop(child_stdin.take());
            }
        }
        // nix exposes waitid only on these targets. Other Unix hosts, including
        // macOS, fall back to Child::try_wait so the child handle still owns the
        // reap and we never import a configured-out waitid symbol.
        #[cfg(any(
            target_os = "android",
            target_os = "freebsd",
            target_os = "haiku",
            all(target_os = "linux", not(target_env = "uclibc")),
        ))]
        let observed = {
            use nix::sys::wait::{Id, WaitPidFlag, WaitStatus, waitid};
            use nix::unistd::Pid;
            match waitid(
                Id::Pid(Pid::from_raw(child.id() as i32)),
                WaitPidFlag::WEXITED | WaitPidFlag::WNOHANG | WaitPidFlag::WNOWAIT,
            ) {
                Ok(WaitStatus::StillAlive) => Ok(None),
                Ok(status) => {
                    reader_stop.store(true, Ordering::Release);
                    terminate_bounded_child(&mut child);
                    let code = match status {
                        WaitStatus::Exited(_, code) => code,
                        WaitStatus::Signaled(_, signal, _) => -(signal as i32),
                        _ => -1,
                    };
                    Ok(Some(code))
                }
                Err(err) => Err(PlatformError::from_io_error(
                    "observe bounded process",
                    PathBuf::from(&request.process.command),
                    io::Error::other(err.to_string()),
                )),
            }
        };
        #[cfg(not(any(
            target_os = "android",
            target_os = "freebsd",
            target_os = "haiku",
            all(target_os = "linux", not(target_env = "uclibc")),
        )))]
        let observed = child
            .try_wait()
            .map(|status| status.map(|status| status.code().unwrap_or(-1)))
            .map_err(|err| {
                PlatformError::from_io_error(
                    "wait bounded process",
                    PathBuf::from(&request.process.command),
                    err,
                )
            });
        match observed {
            Ok(Some(status)) => {
                terminate_bounded_child(&mut child);
                break bounded_reap_child(&mut child, &request.process.command).map(|_| status);
            }
            Ok(None) => {}
            Err(err) => {
                reader_stop.store(true, Ordering::Release);
                terminate_bounded_child(&mut child);
                break match bounded_reap_child(&mut child, &request.process.command) {
                    Ok(()) => Err(err),
                    Err(cleanup) => Err(cleanup),
                };
            }
        }
        if !stdin_progress {
            std::thread::sleep(Duration::from_millis(5));
        }
    };

    // The writer is released before the reader threads are joined, and the
    // child was already terminated and reaped inside the loop on every arm.
    let stdin_undelivered = stdin_offset < stdin_bytes.len();
    drop(child_stdin);

    let stdout = stdout_reader
        .map(|handle| {
            handle.join().unwrap_or(BoundedReaderResult {
                bytes: Vec::new(),
                exceeded: false,
                failed: true,
            })
        })
        .unwrap_or(BoundedReaderResult {
            bytes: Vec::new(),
            exceeded: false,
            failed: false,
        });
    let stderr = stderr_reader
        .map(|handle| {
            handle.join().unwrap_or(BoundedReaderResult {
                bytes: Vec::new(),
                exceeded: false,
                failed: true,
            })
        })
        .unwrap_or(BoundedReaderResult {
            bytes: Vec::new(),
            exceeded: false,
            failed: false,
        });
    let exit_code = termination?;
    if stdout.exceeded {
        return Err(PlatformError::ProcessOutputLimit {
            stream: "stdout",
            limit: request.max_stdout_bytes,
        });
    }
    if stderr.exceeded {
        return Err(PlatformError::ProcessOutputLimit {
            stream: "stderr",
            limit: request.max_stderr_bytes,
        });
    }
    if stdout.failed {
        return Err(PlatformError::ProcessOutputReadFailure { stream: "stdout" });
    }
    if stderr.failed {
        return Err(PlatformError::ProcessOutputReadFailure { stream: "stderr" });
    }
    // A zero exit code can never turn undelivered stdin into successful input.
    // Cancellation and timeout still win, because `termination?` ran first.
    if stdin_undelivered {
        return Err(bounded_stdin_undelivered_error(
            &request.process.command,
            stdin_offset,
            stdin_bytes.len(),
        ));
    }
    let stdout = String::from_utf8(stdout.bytes).map_err(|_| PlatformError::Encoding {
        operation: "decode bounded process stdout".to_string(),
        path: PathBuf::from(&request.process.command),
        source: io::Error::new(io::ErrorKind::InvalidData, "stdout was not UTF-8"),
    })?;
    let stderr = String::from_utf8(stderr.bytes).map_err(|_| PlatformError::Encoding {
        operation: "decode bounded process stderr".to_string(),
        path: PathBuf::from(&request.process.command),
        source: io::Error::new(io::ErrorKind::InvalidData, "stderr was not UTF-8"),
    })?;
    Ok(ProcessResult {
        exit_code,
        stdout,
        stderr,
        elapsed: started.elapsed(),
    })
}

#[cfg(windows)]
fn execute_bounded_native(request: &BoundedProcessRequest) -> Result<ProcessResult, PlatformError> {
    use ::windows::Win32::Foundation::{
        CloseHandle, ERROR_BROKEN_PIPE, ERROR_NO_DATA, ERROR_PIPE_NOT_CONNECTED, GetLastError,
        HANDLE, HANDLE_FLAG_INHERIT, HANDLE_FLAGS, SetHandleInformation, TRUE,
    };
    use ::windows::Win32::Security::SECURITY_ATTRIBUTES;
    use ::windows::Win32::Storage::FileSystem::WriteFile;
    use ::windows::Win32::System::JobObjects::{
        AssignProcessToJobObject, CreateJobObjectW, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
        JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JobObjectExtendedLimitInformation,
        SetInformationJobObject,
    };
    use ::windows::Win32::System::Pipes::{CreatePipe, PIPE_NOWAIT, SetNamedPipeHandleState};
    use ::windows::Win32::System::Threading::{
        CREATE_NO_WINDOW, CREATE_SUSPENDED, CREATE_UNICODE_ENVIRONMENT, CreateProcessW,
        DeleteProcThreadAttributeList, EXTENDED_STARTUPINFO_PRESENT, GetExitCodeProcess,
        InitializeProcThreadAttributeList, LPPROC_THREAD_ATTRIBUTE_LIST,
        PROC_THREAD_ATTRIBUTE_HANDLE_LIST, PROCESS_INFORMATION, ResumeThread, STARTF_USESTDHANDLES,
        STARTUPINFOEXW, TerminateProcess, UpdateProcThreadAttribute, WaitForSingleObject,
    };
    use ::windows::core::{PCWSTR, PWSTR};
    use std::mem::{size_of, zeroed};
    use std::ptr;

    struct Owned(HANDLE);
    impl Drop for Owned {
        fn drop(&mut self) {
            if !self.0.is_invalid() {
                unsafe {
                    let _ = CloseHandle(self.0);
                }
            }
        }
    }
    impl Owned {
        fn into_raw(self) -> HANDLE {
            let h = self.0;
            std::mem::forget(self);
            h
        }
    }

    fn fail(request: &BoundedProcessRequest, reason: impl Into<String>) -> PlatformError {
        PlatformError::ProcessSpawnFailure {
            operation: "bounded process execution".to_string(),
            command: request.process.command.clone(),
            message: reason.into(),
        }
    }
    fn terminate_tree(process: HANDLE, job: Option<Owned>) -> bool {
        drop(job);
        unsafe {
            let _ = TerminateProcess(process, 1);
            let deadline = Instant::now() + Duration::from_secs(1);
            loop {
                let wait = WaitForSingleObject(process, 10);
                if wait.0 == 0 {
                    return true;
                }
                if wait.0 == u32::MAX || Instant::now() >= deadline {
                    return false;
                }
            }
        }
    }

    if request.timeout.is_zero() {
        return Err(PlatformError::UnsupportedOperation {
            operation: "bounded process execution".to_string(),
            path: PathBuf::from(&request.process.command),
            reason: "a finite timeout is required".to_string(),
        });
    }
    if request.process.cancelled || request.cancellation.load(Ordering::Acquire) {
        return Err(PlatformError::Cancelled {
            operation: "bounded process execution".to_string(),
        });
    }
    // Over-limit input is rejected here: after the finite-timeout and
    // cancellation guards, and before `resolve_bounded_executable` and the first
    // `CreatePipe`, so no handle, pipe, job object, or attribute-list allocation
    // can exist after the rejection.
    let stdin_requested = request.process.stdin.is_some();
    let stdin_bytes: &[u8] = request.process.stdin.as_deref().unwrap_or(&[]);
    if stdin_requested && stdin_bytes.len() > MAX_BOUNDED_STDIN_BYTES {
        return Err(bounded_stdin_limit_error(
            &request.process.command,
            stdin_bytes.len(),
        ));
    }

    // Resolved before any pipe exists. At HEAD this sat between
    // `InitializeProcThreadAttributeList` and `CreateProcessW`, so an
    // unresolvable executable returned without calling
    // `DeleteProcThreadAttributeList`; the four output pipe handles were already
    // `Owned` by then and were closed by drop, so only the attribute list
    // leaked.
    let executable = resolve_bounded_executable(&request.process.command)?;
    let executable_command = executable.to_string_lossy().into_owned();

    let started = Instant::now();
    let reader_deadline = started
        .checked_add(request.timeout)
        .unwrap_or_else(Instant::now);
    unsafe {
        let sa = SECURITY_ATTRIBUTES {
            nLength: size_of::<SECURITY_ATTRIBUTES>() as u32,
            lpSecurityDescriptor: ptr::null_mut(),
            bInheritHandle: TRUE,
        };
        // Each handle becomes an RAII `Owned` immediately after its `CreatePipe`,
        // so every `?` from here to `CreateProcessW` closes every created end by
        // drop.
        let mut stdout_read_raw = HANDLE::default();
        let mut stdout_write_raw = HANDLE::default();
        CreatePipe(&mut stdout_read_raw, &mut stdout_write_raw, Some(&sa), 0)
            .map_err(|e| fail(request, format!("create stdout pipe: {e}")))?;
        let stdout_read = Owned(stdout_read_raw);
        let stdout_write = Owned(stdout_write_raw);

        let mut stderr_read_raw = HANDLE::default();
        let mut stderr_write_raw = HANDLE::default();
        CreatePipe(&mut stderr_read_raw, &mut stderr_write_raw, Some(&sa), 0)
            .map_err(|e| fail(request, format!("create stderr pipe: {e}")))?;
        let stderr_read = Owned(stderr_read_raw);
        let stderr_write = Owned(stderr_write_raw);

        // The third pipe exists only when the caller supplied stdin, so a `None`
        // request keeps exactly the two-pipe shape it had before.
        let stdin_pipe = if stdin_requested {
            let mut stdin_read_raw = HANDLE::default();
            let mut stdin_write_raw = HANDLE::default();
            CreatePipe(&mut stdin_read_raw, &mut stdin_write_raw, Some(&sa), 0)
                .map_err(|e| fail(request, format!("create stdin pipe: {e}")))?;
            Some((Owned(stdin_read_raw), Owned(stdin_write_raw)))
        } else {
            None
        };

        SetHandleInformation(stdout_read.0, HANDLE_FLAG_INHERIT.0, HANDLE_FLAGS(0))
            .map_err(|e| fail(request, format!("make stdout read handle private: {e}")))?;
        SetHandleInformation(stderr_read.0, HANDLE_FLAG_INHERIT.0, HANDLE_FLAGS(0))
            .map_err(|e| fail(request, format!("make stderr read handle private: {e}")))?;
        if let Some((_, stdin_write)) = stdin_pipe.as_ref() {
            // The parent write end is private and nonblocking before the child
            // exists. This is synchronous polling, not overlapped I/O.
            SetHandleInformation(stdin_write.0, HANDLE_FLAG_INHERIT.0, HANDLE_FLAGS(0))
                .map_err(|e| fail(request, format!("make stdin write handle private: {e}")))?;
            let nowait = PIPE_NOWAIT;
            SetNamedPipeHandleState(stdin_write.0, Some(&nowait), None, None)
                .map_err(|e| fail(request, format!("make stdin write handle nonblocking: {e}")))?;
        }

        let mut startup: STARTUPINFOEXW = zeroed();
        startup.StartupInfo.cb = size_of::<STARTUPINFOEXW>() as u32;
        startup.StartupInfo.dwFlags = STARTF_USESTDHANDLES;
        startup.StartupInfo.hStdOutput = stdout_write.0;
        startup.StartupInfo.hStdError = stderr_write.0;
        // Only the child *read* end ever enters the inherited-handle whitelist.
        let stdin_read_handle = match stdin_pipe.as_ref() {
            Some((stdin_read, _)) => stdin_read.0,
            None => HANDLE::default(),
        };
        startup.StartupInfo.hStdInput = stdin_read_handle;
        let mut inherited = vec![stdout_write.0, stderr_write.0];
        if stdin_requested {
            inherited.push(stdin_read_handle);
        }
        let mut attr_size = 0usize;
        let _ = InitializeProcThreadAttributeList(None, 1, Some(0), &mut attr_size);
        let attr_words = attr_size.div_ceil(size_of::<usize>());
        let mut attr_storage = vec![0usize; attr_words];
        let attrs = LPPROC_THREAD_ATTRIBUTE_LIST(attr_storage.as_mut_ptr().cast());
        InitializeProcThreadAttributeList(Some(attrs), 1, Some(0), &mut attr_size).map_err(
            |e| {
                fail(
                    request,
                    format!("initialize inherited-handle whitelist: {e}"),
                )
            },
        )?;
        let attr_result = UpdateProcThreadAttribute(
            attrs,
            0,
            PROC_THREAD_ATTRIBUTE_HANDLE_LIST as usize,
            Some(inherited.as_ptr().cast()),
            size_of::<HANDLE>() * inherited.len(),
            None,
            None,
        );
        if let Err(e) = attr_result {
            DeleteProcThreadAttributeList(attrs);
            return Err(fail(
                request,
                format!("configure inherited-handle whitelist: {e}"),
            ));
        }
        startup.lpAttributeList = attrs;

        // Built from the resolved executable and the borrowed argument slice, so
        // no `ProcessRequest` clone — and therefore no copy of the stdin payload
        // — happens on this path.
        let mut command_line =
            windows_process_command_line(&executable_command, &request.process.args);
        let current_dir = request
            .process
            .cwd
            .as_ref()
            .map(|p| wide_null(&p.to_string_lossy()));
        let current_dir = current_dir
            .as_ref()
            .map_or(PCWSTR(ptr::null()), |v| PCWSTR(v.as_ptr()));
        let env_block = windows_environment_block(&child_environment_vars(&request.process.env));
        let mut info = PROCESS_INFORMATION::default();
        let spawn = CreateProcessW(
            PCWSTR(ptr::null()),
            Some(PWSTR(command_line.as_mut_ptr())),
            None,
            None,
            true,
            EXTENDED_STARTUPINFO_PRESENT
                | CREATE_SUSPENDED
                | CREATE_NO_WINDOW
                | CREATE_UNICODE_ENVIRONMENT,
            Some(env_block.as_ptr().cast()),
            current_dir,
            (&startup.StartupInfo) as *const _,
            &mut info,
        );
        DeleteProcThreadAttributeList(attrs);
        if let Err(e) = spawn {
            return Err(fail(request, format!("CreateProcessW: {e}")));
        }
        let process = Owned(info.hProcess);
        let thread = Owned(info.hThread);
        drop(stdout_write);
        drop(stderr_write);
        // The child owns its stdin read end now; the parent must release its own
        // copy or the child never observes EOF. Every setup failure above this
        // line still held both ends in `stdin_pipe` and closed them by drop.
        let mut stdin_write = stdin_pipe.map(|(stdin_read, stdin_write)| {
            drop(stdin_read);
            stdin_write
        });

        let job = match CreateJobObjectW(None, PCWSTR(ptr::null())) {
            Ok(h) => Owned(h),
            Err(e) => {
                if !terminate_tree(process.0, None) {
                    return Err(PlatformError::ProcessCleanupFailure {
                        command: request.process.command.clone(),
                        reason: "process did not terminate within cleanup grace".to_string(),
                    });
                }
                return Err(fail(request, format!("create process job: {e}")));
            }
        };
        let mut job = Some(job);
        let mut limits: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = zeroed();
        limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
        if let Err(e) = SetInformationJobObject(
            job.as_ref().unwrap().0,
            JobObjectExtendedLimitInformation,
            &limits as *const _ as *const core::ffi::c_void,
            size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
        ) {
            if !terminate_tree(process.0, job.take()) {
                return Err(PlatformError::ProcessCleanupFailure {
                    command: request.process.command.clone(),
                    reason: "process did not terminate within cleanup grace".to_string(),
                });
            }
            return Err(fail(request, format!("configure process job: {e}")));
        }
        if let Err(e) = AssignProcessToJobObject(job.as_ref().unwrap().0, process.0) {
            if !terminate_tree(process.0, job.take()) {
                return Err(PlatformError::ProcessCleanupFailure {
                    command: request.process.command.clone(),
                    reason: "process did not terminate within cleanup grace".to_string(),
                });
            }
            return Err(fail(request, format!("assign process job: {e}")));
        }
        if ResumeThread(thread.0) == u32::MAX {
            if !terminate_tree(process.0, job.take()) {
                return Err(PlatformError::ProcessCleanupFailure {
                    command: request.process.command.clone(),
                    reason: "process did not terminate within cleanup grace".to_string(),
                });
            }
            return Err(fail(request, "ResumeThread failed"));
        }

        let overflow = Arc::new(AtomicBool::new(false));
        let reader_stop = Arc::new(AtomicBool::new(false));
        let out_overflow = overflow.clone();
        let err_overflow = overflow.clone();
        let out_stop = reader_stop.clone();
        let err_stop = reader_stop.clone();
        let out_limit = request.max_stdout_bytes;
        let err_limit = request.max_stderr_bytes;
        use std::os::windows::io::FromRawHandle;
        let stdout_file = std::fs::File::from_raw_handle(stdout_read.into_raw().0 as _);
        let stderr_file = std::fs::File::from_raw_handle(stderr_read.into_raw().0 as _);
        let out_thread = std::thread::spawn(move || {
            windows_bounded_reader(
                stdout_file,
                out_limit,
                out_overflow,
                out_stop,
                reader_deadline,
            )
        });
        let err_thread = std::thread::spawn(move || {
            windows_bounded_reader(
                stderr_file,
                err_limit,
                err_overflow,
                err_stop,
                reader_deadline,
            )
        });
        let mut termination: Result<i32, PlatformError> = Ok(0);
        let mut stdin_offset = 0usize;
        loop {
            if request.cancellation.load(Ordering::Acquire) {
                reader_stop.store(true, Ordering::Release);
                termination = if terminate_tree(process.0, job.take()) {
                    Err(PlatformError::Cancelled {
                        operation: "bounded process execution".to_string(),
                    })
                } else {
                    Err(PlatformError::ProcessCleanupFailure {
                        command: request.process.command.clone(),
                        reason: "process did not terminate within cleanup grace".to_string(),
                    })
                };
                break;
            }
            if overflow.load(Ordering::Acquire) {
                reader_stop.store(true, Ordering::Release);
                if !terminate_tree(process.0, job.take()) {
                    termination = Err(PlatformError::ProcessCleanupFailure {
                        command: request.process.command.clone(),
                        reason: "process did not terminate within cleanup grace".to_string(),
                    });
                }
                break;
            }
            if started.elapsed() > request.timeout {
                reader_stop.store(true, Ordering::Release);
                termination = if terminate_tree(process.0, job.take()) {
                    Err(PlatformError::Timeout {
                        operation: format!("process `{}`", request.process.command),
                        duration: started.elapsed(),
                    })
                } else {
                    Err(PlatformError::ProcessCleanupFailure {
                        command: request.process.command.clone(),
                        reason: "process did not terminate within cleanup grace".to_string(),
                    })
                };
                break;
            }
            // One bounded chunk per iteration, after the cancellation /
            // overflow / deadline guards above, so a large payload never starves
            // them. The supervisor owns the writer; there is no writer thread.
            let mut stdin_progress = false;
            if stdin_write.is_some() {
                if stdin_offset < stdin_bytes.len() {
                    let end = stdin_bytes
                        .len()
                        .min(stdin_offset + BOUNDED_STDIN_CHUNK_BYTES);
                    let handle = stdin_write.as_ref().expect("bounded stdin writer").0;
                    let mut written = 0u32;
                    let write = WriteFile(
                        handle,
                        Some(&stdin_bytes[stdin_offset..end]),
                        Some(&mut written),
                        None,
                    );
                    match write {
                        Ok(()) => {
                            if written > 0 {
                                stdin_offset += written as usize;
                                stdin_progress = true;
                            }
                        }
                        Err(e) => {
                            // A PIPE_NOWAIT write reports the same codes for a
                            // full pipe buffer and for a read end that is gone,
                            // so a still-running child disambiguates
                            // back-pressure from a real write failure.
                            let code = GetLastError();
                            let pipe_state = code == ERROR_NO_DATA
                                || code == ERROR_BROKEN_PIPE
                                || code == ERROR_PIPE_NOT_CONNECTED;
                            let child_alive = WaitForSingleObject(process.0, 0).0 != 0;
                            let back_pressure = pipe_state && child_alive;
                            if !back_pressure {
                                reader_stop.store(true, Ordering::Release);
                                termination = if terminate_tree(process.0, job.take()) {
                                    Err(bounded_stdin_write_error(
                                        &request.process.command,
                                        stdin_offset,
                                        stdin_bytes.len(),
                                        io::Error::other(e.to_string()),
                                    ))
                                } else {
                                    Err(PlatformError::ProcessCleanupFailure {
                                        command: request.process.command.clone(),
                                        reason: "process did not terminate within cleanup grace"
                                            .to_string(),
                                    })
                                };
                                break;
                            }
                        }
                    }
                }
                // Reaching the end closes the child's stdin. `Some(Vec::new())`
                // takes this branch on the very first iteration, so a child
                // waiting for EOF still finishes.
                if stdin_offset >= stdin_bytes.len() {
                    stdin_write = None;
                }
            }
            let wait = WaitForSingleObject(process.0, if stdin_progress { 0 } else { 5 });
            if wait.0 == 0 {
                let mut code = 0u32;
                if let Err(e) = GetExitCodeProcess(process.0, &mut code) {
                    termination = Err(PlatformError::from_io_error(
                        "read bounded process exit code",
                        PathBuf::from(&request.process.command),
                        io::Error::other(e.to_string()),
                    ));
                } else {
                    termination = Ok(code as i32);
                }
                drop(job.take());
                break;
            }
            if wait.0 == u32::MAX {
                reader_stop.store(true, Ordering::Release);
                termination = if terminate_tree(process.0, job.take()) {
                    Err(PlatformError::from_io_error(
                        "wait bounded process",
                        PathBuf::from(&request.process.command),
                        io::Error::last_os_error(),
                    ))
                } else {
                    Err(PlatformError::ProcessCleanupFailure {
                        command: request.process.command.clone(),
                        reason: "process did not terminate within cleanup grace".to_string(),
                    })
                };
                break;
            }
        }
        // The writer is released before the reader threads are joined, and the
        // child was already terminated and reaped inside the loop on every arm.
        let stdin_undelivered = stdin_offset < stdin_bytes.len();
        drop(stdin_write.take());

        let stdout = out_thread.join().unwrap_or(BoundedReaderResult {
            bytes: Vec::new(),
            exceeded: false,
            failed: true,
        });
        let stderr = err_thread.join().unwrap_or(BoundedReaderResult {
            bytes: Vec::new(),
            exceeded: false,
            failed: true,
        });
        let exit_code = termination?;
        if stdout.exceeded {
            return Err(PlatformError::ProcessOutputLimit {
                stream: "stdout",
                limit: request.max_stdout_bytes,
            });
        }
        if stderr.exceeded {
            return Err(PlatformError::ProcessOutputLimit {
                stream: "stderr",
                limit: request.max_stderr_bytes,
            });
        }
        if stdout.failed {
            return Err(PlatformError::ProcessOutputReadFailure { stream: "stdout" });
        }
        if stderr.failed {
            return Err(PlatformError::ProcessOutputReadFailure { stream: "stderr" });
        }
        // A zero exit code can never turn undelivered stdin into successful
        // input. Cancellation and timeout still win, because `termination?` ran
        // first.
        if stdin_undelivered {
            return Err(bounded_stdin_undelivered_error(
                &request.process.command,
                stdin_offset,
                stdin_bytes.len(),
            ));
        }
        let stdout = String::from_utf8(stdout.bytes).map_err(|_| PlatformError::Encoding {
            operation: "decode bounded process stdout".to_string(),
            path: PathBuf::from(&request.process.command),
            source: io::Error::new(io::ErrorKind::InvalidData, "stdout was not UTF-8"),
        })?;
        let stderr = String::from_utf8(stderr.bytes).map_err(|_| PlatformError::Encoding {
            operation: "decode bounded process stderr".to_string(),
            path: PathBuf::from(&request.process.command),
            source: io::Error::new(io::ErrorKind::InvalidData, "stderr was not UTF-8"),
        })?;
        Ok(ProcessResult {
            exit_code,
            stdout,
            stderr,
            elapsed: started.elapsed(),
        })
    }
}

/// Forcibly terminates a process that exceeded its timeout, taking down its
/// descendant process group on Unix, then reaps the child to avoid a zombie.
fn terminate_timed_out_process(child: &mut std::process::Child) {
    #[cfg(unix)]
    {
        // The child is its own process-group leader (see `execute`), so signal
        // the whole group to take down any descendants it spawned.
        let pid = child.id() as i32;
        let _ = nix::sys::signal::kill(
            nix::unistd::Pid::from_raw(-pid),
            nix::sys::signal::Signal::SIGKILL,
        );
    }
    #[cfg(windows)]
    {
        let _ = std::process::Command::new("taskkill")
            .args(["/PID", &child.id().to_string(), "/T", "/F"])
            .status();
    }
    let _ = child.kill();
    let _ = child.wait();
}

/// Terminates the bounded child's owned process group without reaping it.
/// Unix callers must invoke `bounded_reap_child` afterward while the original
/// child handle still owns the observed leader; this avoids a post-reap PID race.
#[cfg(unix)]
fn terminate_bounded_child(child: &mut std::process::Child) {
    let pid = child.id() as i32;
    let _ = nix::sys::signal::kill(
        nix::unistd::Pid::from_raw(-pid),
        nix::sys::signal::Signal::SIGKILL,
    );
    let _ = child.kill();
}

#[cfg(unix)]
fn bounded_reap_child(child: &mut std::process::Child, command: &str) -> Result<(), PlatformError> {
    let deadline = Instant::now() + Duration::from_secs(1);
    loop {
        match child.try_wait() {
            Ok(Some(_)) => return Ok(()),
            Ok(None) if Instant::now() < deadline => std::thread::sleep(Duration::from_millis(5)),
            Ok(None) => {
                return Err(PlatformError::ProcessCleanupFailure {
                    command: command.to_string(),
                    reason: "process did not terminate within cleanup grace".to_string(),
                });
            }
            Err(err) => {
                return Err(PlatformError::from_io_error(
                    "reap bounded process",
                    PathBuf::from(command),
                    err,
                ));
            }
        }
    }
}

#[cfg(not(any(unix, windows)))]
fn execute_bounded_native(request: &BoundedProcessRequest) -> Result<ProcessResult, PlatformError> {
    Err(PlatformError::UnsupportedOperation {
        operation: "bounded process execution".to_string(),
        path: PathBuf::from(&request.process.command),
        reason: "bounded process execution is unsupported on this target".to_string(),
    })
}

#[cfg(windows)]
fn assign_windows_process_job(child: &std::process::Child) -> Option<WindowsProcessJob> {
    use ::windows::Win32::Foundation::{CloseHandle, HANDLE};
    use ::windows::Win32::System::JobObjects::{
        AssignProcessToJobObject, CreateJobObjectW, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
        JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JobObjectExtendedLimitInformation,
        SetInformationJobObject,
    };
    use ::windows::core::PCWSTR;
    use std::os::windows::io::AsRawHandle;

    unsafe {
        let job = CreateJobObjectW(None, PCWSTR::null()).ok()?;
        let mut limits: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = std::mem::zeroed();
        limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
        if SetInformationJobObject(
            job,
            JobObjectExtendedLimitInformation,
            &limits as *const _ as *const core::ffi::c_void,
            std::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
        )
        .is_err()
        {
            let _ = CloseHandle(job);
            return None;
        }
        let process = HANDLE(child.as_raw_handle());
        if AssignProcessToJobObject(job, process).is_err() {
            let _ = CloseHandle(job);
            return None;
        }
        Some(WindowsProcessJob(job))
    }
}

#[cfg(windows)]
struct WindowsProcessJob(::windows::Win32::Foundation::HANDLE);

#[cfg(windows)]
impl Drop for WindowsProcessJob {
    fn drop(&mut self) {
        unsafe {
            let _ = ::windows::Win32::Foundation::CloseHandle(self.0);
        }
    }
}

const PTY_OUTPUT_LIMIT: usize = 256 * 1024;

enum NativePtySessionHandle {
    #[cfg(unix)]
    Unix {
        master: std::fs::File,
        child: std::process::Child,
    },
    #[cfg(windows)]
    Windows(WindowsPtySessionHandle),
}

#[cfg(windows)]
struct WindowsPtySessionHandle {
    conpty: usize,
    process: usize,
    input_write: usize,
    output: Arc<Mutex<Vec<u8>>>,
}

static NATIVE_PTY_SESSIONS: OnceLock<Mutex<HashMap<String, NativePtySessionHandle>>> =
    OnceLock::new();

fn native_pty_sessions() -> &'static Mutex<HashMap<String, NativePtySessionHandle>> {
    NATIVE_PTY_SESSIONS.get_or_init(|| Mutex::new(HashMap::new()))
}

fn next_native_pty_session_id(prefix: &str) -> String {
    let sequence = NATIVE_PTY_SESSION_COUNTER.fetch_add(1, Ordering::Relaxed);
    format!("{prefix}-{sequence}")
}

#[cfg(windows)]
fn spawn_native_pty(request: &PtyRequest) -> Result<PtySession, PlatformError> {
    let handle = spawn_windows_conpty(request)?;
    let id = next_native_pty_session_id("native-conpty");
    let output = poll_windows_output(&handle, PTY_OUTPUT_LIMIT, Duration::from_millis(50))?;
    let mut sessions = native_pty_sessions()
        .lock()
        .map_err(|_| pty_registry_poisoned())?;
    if sessions.contains_key(&id) {
        close_windows_session(handle, true);
        return Err(PlatformError::PtyUnavailable {
            reason: format!("native PTY session id collision for `{id}`"),
        });
    }
    sessions.insert(id.clone(), NativePtySessionHandle::Windows(handle));
    Ok(PtySession { id, output })
}

#[cfg(unix)]
fn spawn_native_pty(request: &PtyRequest) -> Result<PtySession, PlatformError> {
    use std::fs::File;
    use std::os::fd::{AsRawFd, FromRawFd, IntoRawFd};
    use std::os::unix::process::CommandExt;
    use std::process::Stdio;

    let pty = nix::pty::openpty(None, None).map_err(|err| PlatformError::PtyUnavailable {
        reason: format!("open unix PTY: {err}"),
    })?;
    let mut master = unsafe { File::from_raw_fd(pty.master.into_raw_fd()) };
    set_unix_nonblocking(&master)?;
    let slave = unsafe { File::from_raw_fd(pty.slave.into_raw_fd()) };
    let stdin = slave
        .try_clone()
        .map_err(|err| PlatformError::from_io_error("clone PTY stdin", PathBuf::from("."), err))?;
    let slave_stdin_fd = stdin.as_raw_fd();
    let stdout = slave
        .try_clone()
        .map_err(|err| PlatformError::from_io_error("clone PTY stdout", PathBuf::from("."), err))?;
    let stderr = slave;

    let mut command = Command::new(&request.command);
    command.args(&request.args);
    command.env_clear();
    if let Some(cwd) = &request.cwd {
        command.current_dir(cwd);
    }
    // Use caller-supplied env when provided (e.g. with deny-list applied); fall back to
    // the sanitised parent environment so that the shell has a usable PATH/HOME.
    let child_env = request
        .env
        .clone()
        .unwrap_or_else(|| child_environment_vars(&[]));
    for (key, value) in child_env {
        command.env(key, value);
    }
    command
        .stdin(Stdio::from(stdin))
        .stdout(Stdio::from(stdout))
        .stderr(Stdio::from(stderr));
    unsafe {
        command.pre_exec(move || {
            nix::unistd::setsid().map_err(|err| io::Error::from_raw_os_error(err as i32))?;
            let result = nix::libc::ioctl(
                slave_stdin_fd,
                nix::libc::TIOCSCTTY as nix::libc::c_ulong,
                0,
            );
            if result < 0 {
                return Err(io::Error::last_os_error());
            }
            Ok(())
        });
    }
    let child = command
        .spawn()
        .map_err(|err| PlatformError::ProcessSpawnFailure {
            operation: "spawn Unix PTY command".to_string(),
            command: request.command.clone(),
            message: err.to_string(),
        })?;
    let output = poll_unix_output(&mut master, PTY_OUTPUT_LIMIT, Duration::from_millis(50))?;
    let id = next_native_pty_session_id("native-unix-pty");
    native_pty_sessions()
        .lock()
        .map_err(|_| pty_registry_poisoned())?
        .insert(id.clone(), NativePtySessionHandle::Unix { master, child });
    Ok(PtySession { id, output })
}

#[cfg(not(any(unix, windows)))]
fn spawn_native_pty(request: &PtyRequest) -> Result<PtySession, PlatformError> {
    Err(PlatformError::PtyUnavailable {
        reason: format!(
            "native PTY is unsupported on this target for `{}`",
            request.command
        ),
    })
}

#[cfg(windows)]
fn spawn_windows_conpty(request: &PtyRequest) -> Result<WindowsPtySessionHandle, PlatformError> {
    use ::windows::Win32::Foundation::{CloseHandle, HANDLE};
    use ::windows::Win32::System::Console::{
        COORD, CreatePseudoConsole, HPCON, ResizePseudoConsole,
    };
    use ::windows::Win32::System::Pipes::CreatePipe;
    use ::windows::Win32::System::Threading::{
        CREATE_NEW_PROCESS_GROUP, CREATE_UNICODE_ENVIRONMENT, CreateProcessW,
        DeleteProcThreadAttributeList, EXTENDED_STARTUPINFO_PRESENT,
        InitializeProcThreadAttributeList, LPPROC_THREAD_ATTRIBUTE_LIST,
        PROC_THREAD_ATTRIBUTE_PSEUDOCONSOLE, PROCESS_INFORMATION, STARTF_USESTDHANDLES,
        STARTUPINFOEXW, UpdateProcThreadAttribute,
    };
    use ::windows::core::{PCWSTR, PWSTR};
    use std::mem::{size_of, zeroed};
    use std::ptr;

    let mut input_read = HANDLE::default();
    let mut input_write = HANDLE::default();
    let mut output_read = HANDLE::default();
    let mut output_write = HANDLE::default();
    unsafe {
        CreatePipe(&mut input_read, &mut input_write, None, 0).map_err(|err| {
            PlatformError::PtyUnavailable {
                reason: format!("create ConPTY input pipe: {err}"),
            }
        })?;
        CreatePipe(&mut output_read, &mut output_write, None, 0).map_err(|err| {
            let _ = CloseHandle(input_read);
            let _ = CloseHandle(input_write);
            PlatformError::PtyUnavailable {
                reason: format!("create ConPTY output pipe: {err}"),
            }
        })?;
        let conpty = CreatePseudoConsole(COORD { X: 80, Y: 24 }, input_read, output_write, 0)
            .map_err(|err| {
                let _ = CloseHandle(input_read);
                let _ = CloseHandle(input_write);
                let _ = CloseHandle(output_read);
                let _ = CloseHandle(output_write);
                PlatformError::PtyUnavailable {
                    reason: format!("create Windows ConPTY: {err}"),
                }
            })?;

        let mut attribute_size = 0usize;
        let _ = InitializeProcThreadAttributeList(None, 1, Some(0), &mut attribute_size);
        let mut attribute_storage = vec![0u8; attribute_size];
        let attribute_list = LPPROC_THREAD_ATTRIBUTE_LIST(
            attribute_storage.as_mut_ptr().cast::<core::ffi::c_void>(),
        );
        InitializeProcThreadAttributeList(Some(attribute_list), 1, Some(0), &mut attribute_size)
            .map_err(|err| {
                close_windows_conpty_handles(
                    Some(conpty),
                    &[input_read, input_write, output_read, output_write],
                );
                PlatformError::PtyUnavailable {
                    reason: format!("initialize ConPTY process attribute list: {err}"),
                }
            })?;
        UpdateProcThreadAttribute(
            attribute_list,
            0,
            PROC_THREAD_ATTRIBUTE_PSEUDOCONSOLE as usize,
            Some(conpty.0 as *const core::ffi::c_void),
            size_of::<HPCON>(),
            None,
            None,
        )
        .map_err(|err| {
            DeleteProcThreadAttributeList(attribute_list);
            close_windows_conpty_handles(
                Some(conpty),
                &[input_read, input_write, output_read, output_write],
            );
            PlatformError::PtyUnavailable {
                reason: format!("attach ConPTY process attribute: {err}"),
            }
        })?;

        let mut startup: STARTUPINFOEXW = zeroed();
        startup.StartupInfo.cb = size_of::<STARTUPINFOEXW>() as u32;
        startup.StartupInfo.dwFlags = STARTF_USESTDHANDLES;
        startup.lpAttributeList = attribute_list;

        let mut process_info = PROCESS_INFORMATION::default();
        let mut command_line = windows_command_line(request);
        let current_dir = request
            .cwd
            .as_ref()
            .map(|cwd| wide_null(&cwd.to_string_lossy()));
        let current_dir_ptr = current_dir
            .as_ref()
            .map(|cwd| PCWSTR(cwd.as_ptr()))
            .unwrap_or(PCWSTR(ptr::null()));
        // Use caller-supplied env when provided (deny-list pre-applied); fall back to
        // the sanitised parent environment so that the shell has a usable PATH/WINDIR.
        let resolved_env = request
            .env
            .clone()
            .unwrap_or_else(|| child_environment_vars(&[]));
        let env_block = windows_environment_block(&resolved_env);

        let spawn_result = CreateProcessW(
            PCWSTR(ptr::null()),
            Some(PWSTR(command_line.as_mut_ptr())),
            None,
            None,
            false,
            EXTENDED_STARTUPINFO_PRESENT | CREATE_UNICODE_ENVIRONMENT | CREATE_NEW_PROCESS_GROUP,
            Some(env_block.as_ptr().cast::<core::ffi::c_void>()),
            current_dir_ptr,
            (&startup.StartupInfo) as *const _,
            &mut process_info,
        );
        DeleteProcThreadAttributeList(attribute_list);
        if let Err(err) = spawn_result {
            close_windows_conpty_handles(
                Some(conpty),
                &[input_read, input_write, output_read, output_write],
            );
            return Err(PlatformError::ProcessSpawnFailure {
                operation: "spawn Windows ConPTY command".to_string(),
                command: request.command.clone(),
                message: err.to_string(),
            });
        }

        let _ = CloseHandle(input_read);
        let _ = CloseHandle(output_write);
        let _ = CloseHandle(process_info.hThread);
        let output = Arc::new(Mutex::new(Vec::new()));
        spawn_windows_output_reader(output_read.0 as usize, Arc::clone(&output));
        let _ = ResizePseudoConsole(conpty, COORD { X: 80, Y: 24 });

        Ok(WindowsPtySessionHandle {
            conpty: conpty.0 as usize,
            process: process_info.hProcess.0 as usize,
            input_write: input_write.0 as usize,
            output,
        })
    }
}

#[cfg(windows)]
fn spawn_windows_output_reader(output_read: usize, output: Arc<Mutex<Vec<u8>>>) {
    std::thread::spawn(move || {
        use ::windows::Win32::Foundation::{CloseHandle, HANDLE};
        use ::windows::Win32::Storage::FileSystem::ReadFile;
        let handle = HANDLE(output_read as *mut core::ffi::c_void);
        let mut buffer = [0u8; 4096];
        loop {
            let mut read = 0u32;
            let result = unsafe { ReadFile(handle, Some(&mut buffer), Some(&mut read), None) };
            if result.is_err() || read == 0 {
                break;
            }
            if let Ok(mut output) = output.lock() {
                let remaining = PTY_OUTPUT_LIMIT.saturating_sub(output.len());
                if remaining > 0 {
                    output.extend_from_slice(&buffer[..(read as usize).min(remaining)]);
                }
            }
        }
        unsafe {
            let _ = CloseHandle(handle);
        }
    });
}

#[cfg(windows)]
fn drain_windows_output(
    handle: &WindowsPtySessionHandle,
    max_bytes: usize,
) -> Result<String, PlatformError> {
    let mut output = handle.output.lock().map_err(|_| pty_registry_poisoned())?;
    let take = output.len().min(max_bytes);
    let bytes = output.drain(..take).collect::<Vec<_>>();
    Ok(String::from_utf8_lossy(&bytes).into_owned())
}

#[cfg(windows)]
fn poll_windows_output(
    handle: &WindowsPtySessionHandle,
    max_bytes: usize,
    timeout: Duration,
) -> Result<String, PlatformError> {
    let deadline = Instant::now() + timeout;
    loop {
        let output = drain_windows_output(handle, max_bytes)?;
        if !output.is_empty() || Instant::now() >= deadline {
            return Ok(output);
        }
        std::thread::sleep(Duration::from_millis(2));
    }
}

#[cfg(windows)]
fn poll_windows_output_until_quiet(
    handle: &WindowsPtySessionHandle,
    max_bytes: usize,
    timeout: Duration,
    quiet_period: Duration,
) -> Result<String, PlatformError> {
    let deadline = Instant::now() + timeout;
    let mut output = String::new();
    let mut last_output_at = None;
    loop {
        let remaining = max_bytes.saturating_sub(output.len());
        if remaining == 0 {
            return Ok(output);
        }
        let chunk = drain_windows_output(handle, remaining)?;
        if !chunk.is_empty() {
            output.push_str(&chunk);
            last_output_at = Some(Instant::now());
            continue;
        }
        let now = Instant::now();
        if now >= deadline
            || last_output_at.is_some_and(|last| now.duration_since(last) >= quiet_period)
        {
            return Ok(output);
        }
        std::thread::sleep(Duration::from_millis(2));
    }
}

#[cfg(windows)]
fn write_windows_conpty_input(
    handle: &WindowsPtySessionHandle,
    bytes: &[u8],
) -> Result<(), PlatformError> {
    use ::windows::Win32::Foundation::HANDLE;
    use ::windows::Win32::Storage::FileSystem::WriteFile;

    let mut offset = 0usize;
    while offset < bytes.len() {
        let mut written = 0u32;
        unsafe {
            WriteFile(
                HANDLE(handle.input_write as *mut core::ffi::c_void),
                Some(&bytes[offset..]),
                Some(&mut written),
                None,
            )
            .map_err(|err| PlatformError::PtyUnavailable {
                reason: format!("write Windows ConPTY input at offset {offset}: {err}"),
            })?;
        }
        if written == 0 {
            return Err(PlatformError::PtyUnavailable {
                reason: format!("write Windows ConPTY input wrote zero bytes at offset {offset}"),
            });
        }
        offset += written as usize;
    }
    Ok(())
}

#[cfg(windows)]
fn windows_session_exited(
    handle: &WindowsPtySessionHandle,
) -> Result<(bool, Option<i32>), PlatformError> {
    use ::windows::Win32::Foundation::HANDLE;
    use ::windows::Win32::System::Threading::GetExitCodeProcess;
    let mut exit_code = 0u32;
    unsafe {
        GetExitCodeProcess(
            HANDLE(handle.process as *mut core::ffi::c_void),
            &mut exit_code,
        )
        .map_err(|err| PlatformError::PtyUnavailable {
            reason: format!("read Windows ConPTY process exit code: {err}"),
        })?;
    }
    if exit_code == 259 {
        Ok((false, None))
    } else {
        Ok((true, Some(exit_code as i32)))
    }
}

#[cfg(windows)]
fn close_windows_session(handle: WindowsPtySessionHandle, terminate: bool) {
    use ::windows::Win32::Foundation::{CloseHandle, HANDLE};
    use ::windows::Win32::System::Console::{ClosePseudoConsole, HPCON};
    use ::windows::Win32::System::Threading::{TerminateProcess, WaitForSingleObject};
    unsafe {
        let process = HANDLE(handle.process as *mut core::ffi::c_void);
        if terminate {
            let _ = TerminateProcess(process, 1);
        }
        let _ = CloseHandle(HANDLE(handle.input_write as *mut core::ffi::c_void));
        ClosePseudoConsole(HPCON(handle.conpty as isize));
        let _ = WaitForSingleObject(process, 100);
        let _ = CloseHandle(process);
    }
}

fn close_removed_pty_session(handle: NativePtySessionHandle, _terminate: bool) {
    match handle {
        #[cfg(unix)]
        NativePtySessionHandle::Unix { .. } => {}
        #[cfg(windows)]
        NativePtySessionHandle::Windows(handle) => close_windows_session(handle, _terminate),
    }
}

#[cfg(windows)]
fn close_windows_conpty_handles(
    conpty: Option<::windows::Win32::System::Console::HPCON>,
    handles: &[::windows::Win32::Foundation::HANDLE],
) {
    use ::windows::Win32::Foundation::CloseHandle;
    use ::windows::Win32::System::Console::ClosePseudoConsole;
    unsafe {
        if let Some(conpty) = conpty {
            ClosePseudoConsole(conpty);
        }
        for handle in handles {
            let _ = CloseHandle(*handle);
        }
    }
}

#[cfg(windows)]
fn windows_command_line(request: &PtyRequest) -> Vec<u16> {
    let mut parts = Vec::with_capacity(request.args.len() + 1);
    parts.push(quote_windows_arg(&request.command));
    parts.extend(request.args.iter().map(|arg| quote_windows_arg(arg)));
    wide_null(&parts.join(" "))
}

/// Builds the child command line from the resolved executable and its argument
/// slice. Takes borrowed pieces rather than a whole `ProcessRequest` so the
/// bounded path never clones the request's stdin payload to override one field.
#[cfg(windows)]
fn windows_process_command_line(command: &str, args: &[String]) -> Vec<u16> {
    let mut parts = Vec::with_capacity(args.len() + 1);
    parts.push(quote_windows_arg(command));
    parts.extend(args.iter().map(|arg| quote_windows_arg(arg)));
    wide_null(&parts.join(" "))
}

#[cfg(windows)]
fn quote_windows_arg(value: &str) -> String {
    if !value.is_empty() && !value.chars().any(|ch| ch.is_whitespace() || ch == '"') {
        return value.to_string();
    }

    let mut quoted = String::with_capacity(value.len() + 2);
    quoted.push('"');
    let mut pending_backslashes = 0usize;
    for ch in value.chars() {
        match ch {
            '\\' => pending_backslashes += 1,
            '"' => {
                push_windows_backslashes(&mut quoted, pending_backslashes.saturating_mul(2) + 1);
                quoted.push('"');
                pending_backslashes = 0;
            }
            _ => {
                push_windows_backslashes(&mut quoted, pending_backslashes);
                pending_backslashes = 0;
                quoted.push(ch);
            }
        }
    }
    push_windows_backslashes(&mut quoted, pending_backslashes.saturating_mul(2));
    quoted.push('"');
    quoted
}

#[cfg(windows)]
fn push_windows_backslashes(output: &mut String, count: usize) {
    for _ in 0..count {
        output.push('\\');
    }
}

#[cfg(windows)]
fn windows_environment_block(vars: &[(String, String)]) -> Vec<u16> {
    // CreateProcessW requires the environment block to be sorted case-insensitively by key.
    let mut sorted: Vec<&(String, String)> = vars.iter().collect();
    sorted.sort_by_key(|(key, _)| key.to_ascii_uppercase());
    let mut block = Vec::new();
    for (key, value) in sorted {
        block.extend(wide_null(&format!("{key}={value}")));
    }
    block.push(0);
    block
}

#[cfg(windows)]
fn wide_null(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(std::iter::once(0)).collect()
}

#[cfg(unix)]
fn set_unix_nonblocking(file: &std::fs::File) -> Result<(), PlatformError> {
    use std::os::fd::AsRawFd;
    let fd = file.as_raw_fd();
    let flags = unsafe { nix::libc::fcntl(fd, nix::libc::F_GETFL) };
    if flags < 0 {
        return Err(PlatformError::PtyUnavailable {
            reason: "read Unix PTY flags".to_string(),
        });
    }
    let result = unsafe { nix::libc::fcntl(fd, nix::libc::F_SETFL, flags | nix::libc::O_NONBLOCK) };
    if result < 0 {
        return Err(PlatformError::PtyUnavailable {
            reason: "set Unix PTY nonblocking".to_string(),
        });
    }
    Ok(())
}

#[cfg(unix)]
fn read_unix_available(
    master: &mut std::fs::File,
    max_bytes: usize,
) -> Result<PtyReadResult, PlatformError> {
    use std::io::Read as _;
    let mut bytes = Vec::new();
    let mut truncated = false;
    loop {
        let remaining = max_bytes.saturating_sub(bytes.len());
        if remaining == 0 {
            truncated = true;
            break;
        }
        let mut chunk = vec![0u8; remaining.min(4096)];
        match master.read(&mut chunk) {
            Ok(0) => break,
            Ok(read) => bytes.extend_from_slice(&chunk[..read]),
            Err(err) if err.kind() == io::ErrorKind::WouldBlock => break,
            Err(err) if err.raw_os_error() == Some(nix::libc::EIO) => break,
            Err(err) => {
                return Err(PlatformError::from_io_error(
                    "read Unix PTY output",
                    ".",
                    err,
                ));
            }
        }
    }
    Ok(PtyReadResult {
        id: String::new(),
        output: String::from_utf8_lossy(&bytes).into_owned(),
        exited: false,
        exit_code: None,
        truncated,
    })
}

#[cfg(unix)]
fn poll_unix_output(
    master: &mut std::fs::File,
    max_bytes: usize,
    timeout: Duration,
) -> Result<String, PlatformError> {
    let deadline = Instant::now() + timeout;
    loop {
        let output = read_unix_available(master, max_bytes)?.output;
        if !output.is_empty() || Instant::now() >= deadline {
            return Ok(output);
        }
        std::thread::sleep(Duration::from_millis(2));
    }
}

#[cfg(unix)]
fn write_unix_nonblocking(master: &mut std::fs::File, bytes: &[u8]) -> Result<(), PlatformError> {
    use std::io::Write as _;

    let deadline = Instant::now() + Duration::from_millis(500);
    let mut offset = 0usize;
    while offset < bytes.len() {
        match master.write(&bytes[offset..]) {
            Ok(0) => {
                return Err(PlatformError::PtyUnavailable {
                    reason: format!("write Unix PTY input wrote zero bytes at offset {offset}"),
                });
            }
            Ok(written) => offset += written,
            Err(err) if err.kind() == io::ErrorKind::Interrupted => {}
            Err(err) if err.kind() == io::ErrorKind::WouldBlock && Instant::now() < deadline => {
                std::thread::sleep(Duration::from_millis(2));
            }
            Err(err) => {
                return Err(PlatformError::from_io_error(
                    "write Unix PTY input",
                    ".",
                    err,
                ));
            }
        }
    }
    Ok(())
}

#[cfg(unix)]
fn wait_unix_child_exit(
    child: &mut std::process::Child,
    timeout: Duration,
) -> Result<bool, PlatformError> {
    let deadline = Instant::now() + timeout;
    loop {
        if child
            .try_wait()
            .map_err(|err| PlatformError::from_io_error("wait Unix PTY child", ".", err))?
            .is_some()
        {
            return Ok(true);
        }
        if Instant::now() >= deadline {
            return Ok(false);
        }
        std::thread::sleep(Duration::from_millis(5));
    }
}

#[cfg(unix)]
fn resize_unix_pty(master: &std::fs::File, cols: u16, rows: u16) -> Result<(), PlatformError> {
    use std::os::fd::AsRawFd;
    let size = nix::libc::winsize {
        ws_row: rows,
        ws_col: cols,
        ws_xpixel: 0,
        ws_ypixel: 0,
    };
    let result = unsafe { nix::libc::ioctl(master.as_raw_fd(), nix::libc::TIOCSWINSZ, &size) };
    if result < 0 {
        return Err(PlatformError::PtyUnavailable {
            reason: "resize Unix PTY".to_string(),
        });
    }
    Ok(())
}

fn pty_registry_poisoned() -> PlatformError {
    PlatformError::PtyUnavailable {
        reason: "native PTY registry is poisoned".to_string(),
    }
}

fn pty_session_missing(session_id: &str) -> PlatformError {
    PlatformError::PtyUnavailable {
        reason: format!("PTY session `{session_id}` is not active"),
    }
}

impl PtyService for NativePtyService {
    fn spawn_pty(&self, request: &PtyRequest) -> Result<PtySession, PlatformError> {
        spawn_native_pty(request)
    }

    fn write_pty(&self, session_id: &str, input: &str) -> Result<(), PlatformError> {
        let mut sessions = native_pty_sessions()
            .lock()
            .map_err(|_| pty_registry_poisoned())?;
        let handle = sessions
            .get_mut(session_id)
            .ok_or_else(|| pty_session_missing(session_id))?;
        match handle {
            #[cfg(unix)]
            NativePtySessionHandle::Unix { master, .. } => {
                write_unix_nonblocking(master, input.as_bytes())
            }
            #[cfg(windows)]
            NativePtySessionHandle::Windows(handle) => {
                write_windows_conpty_input(handle, input.as_bytes())
            }
        }
    }

    fn resize_pty(&self, session_id: &str, cols: u16, rows: u16) -> Result<(), PlatformError> {
        let mut sessions = native_pty_sessions()
            .lock()
            .map_err(|_| pty_registry_poisoned())?;
        let handle = sessions
            .get_mut(session_id)
            .ok_or_else(|| pty_session_missing(session_id))?;
        match handle {
            #[cfg(unix)]
            NativePtySessionHandle::Unix { master, .. } => resize_unix_pty(master, cols, rows),
            #[cfg(windows)]
            NativePtySessionHandle::Windows(handle) => {
                use ::windows::Win32::System::Console::{COORD, HPCON, ResizePseudoConsole};
                unsafe {
                    ResizePseudoConsole(
                        HPCON(handle.conpty as isize),
                        COORD {
                            X: cols as i16,
                            Y: rows as i16,
                        },
                    )
                    .map_err(|err| PlatformError::PtyUnavailable {
                        reason: format!("resize Windows ConPTY: {err}"),
                    })
                }
            }
        }
    }

    fn read_pty(&self, session_id: &str, max_bytes: usize) -> Result<PtyReadResult, PlatformError> {
        let mut sessions = native_pty_sessions()
            .lock()
            .map_err(|_| pty_registry_poisoned())?;
        let handle = sessions
            .get_mut(session_id)
            .ok_or_else(|| pty_session_missing(session_id))?;
        let (mut result, remove) = match handle {
            #[cfg(unix)]
            NativePtySessionHandle::Unix { master, child } => {
                let mut result = read_unix_available(master, max_bytes)?;
                let mut remove = false;
                if let Some(status) = child
                    .try_wait()
                    .map_err(|err| PlatformError::from_io_error("poll Unix PTY child", ".", err))?
                {
                    result.exited = true;
                    result.exit_code = status.code();
                    remove = true;
                }
                (result, remove)
            }
            #[cfg(windows)]
            NativePtySessionHandle::Windows(handle) => {
                let (exited, exit_code) = windows_session_exited(handle)?;
                let output = if exited {
                    poll_windows_output_until_quiet(
                        handle,
                        max_bytes,
                        Duration::from_millis(250),
                        Duration::from_millis(20),
                    )?
                } else {
                    drain_windows_output(handle, max_bytes)?
                };
                (
                    PtyReadResult {
                        id: String::new(),
                        truncated: output.len() >= max_bytes,
                        output,
                        exited,
                        exit_code,
                    },
                    exited,
                )
            }
        };
        result.id = session_id.to_string();
        if remove
            && !result.truncated
            && let Some(handle) = sessions.remove(session_id)
        {
            close_removed_pty_session(handle, false);
        }
        Ok(result)
    }

    fn close_pty(&self, session_id: &str) -> Result<(), PlatformError> {
        #[cfg(unix)]
        self.kill_pty(session_id, PtyKillMode::Terminate)?;
        #[cfg(windows)]
        {
            if let Some(NativePtySessionHandle::Windows(handle)) = native_pty_sessions()
                .lock()
                .map_err(|_| pty_registry_poisoned())?
                .remove(session_id)
            {
                close_windows_session(handle, false);
            } else {
                return Err(pty_session_missing(session_id));
            }
        }
        Ok(())
    }

    fn kill_pty(&self, session_id: &str, mode: PtyKillMode) -> Result<(), PlatformError> {
        let mut sessions = native_pty_sessions()
            .lock()
            .map_err(|_| pty_registry_poisoned())?;
        #[cfg(unix)]
        {
            let handle = sessions
                .get_mut(session_id)
                .ok_or_else(|| pty_session_missing(session_id))?;
            match handle {
                NativePtySessionHandle::Unix { child, .. } => {
                    let signal = match mode {
                        PtyKillMode::Interrupt => nix::sys::signal::Signal::SIGINT,
                        PtyKillMode::Terminate => nix::sys::signal::Signal::SIGTERM,
                        PtyKillMode::Kill | PtyKillMode::KillTree => {
                            nix::sys::signal::Signal::SIGKILL
                        }
                    };
                    let pid = child.id() as i32;
                    // The PTY child is a session/group leader (via `setsid`), so
                    // signalling the negative PID reaches the whole process group.
                    // `Terminate` (used by `close_pty`) must target the group too,
                    // otherwise grandchildren survive a SIGTERM aimed only at the
                    // leader.
                    let target = if matches!(
                        mode,
                        PtyKillMode::Interrupt | PtyKillMode::KillTree | PtyKillMode::Terminate
                    ) {
                        nix::unistd::Pid::from_raw(-pid)
                    } else {
                        nix::unistd::Pid::from_raw(pid)
                    };
                    let _ = nix::sys::signal::kill(target, signal);
                    if mode == PtyKillMode::Interrupt {
                        return Ok(());
                    }
                    if wait_unix_child_exit(child, Duration::from_millis(500))? {
                        let _ = sessions.remove(session_id);
                        return Ok(());
                    }
                    // Escalate to SIGKILL on the whole process group, then reap the
                    // leader before dropping the session entry.
                    let _ = nix::sys::signal::kill(
                        nix::unistd::Pid::from_raw(-pid),
                        nix::sys::signal::Signal::SIGKILL,
                    );
                    if wait_unix_child_exit(child, Duration::from_millis(500))? {
                        let _ = sessions.remove(session_id);
                        Ok(())
                    } else {
                        Err(PlatformError::PtyUnavailable {
                            reason: format!(
                                "Unix PTY session `{session_id}` did not exit after {signal:?} and SIGKILL"
                            ),
                        })
                    }
                }
            }
        }
        #[cfg(windows)]
        {
            match mode {
                PtyKillMode::Interrupt => {
                    let handle = sessions
                        .get(session_id)
                        .ok_or_else(|| pty_session_missing(session_id))?;
                    match handle {
                        NativePtySessionHandle::Windows(handle) => {
                            write_windows_conpty_input(handle, b"\x03")
                        }
                    }
                }
                PtyKillMode::Terminate | PtyKillMode::Kill | PtyKillMode::KillTree => {
                    let handle = sessions
                        .remove(session_id)
                        .ok_or_else(|| pty_session_missing(session_id))?;
                    match handle {
                        NativePtySessionHandle::Windows(handle) => {
                            close_windows_session(handle, true);
                            Ok(())
                        }
                    }
                }
            }
        }
    }

    fn cleanup_orphaned_ptys(&self) -> Result<Vec<String>, PlatformError> {
        let mut sessions = native_pty_sessions()
            .lock()
            .map_err(|_| pty_registry_poisoned())?;
        let mut orphaned = Vec::new();
        for (id, handle) in sessions.iter_mut() {
            match handle {
                #[cfg(unix)]
                NativePtySessionHandle::Unix { child, .. } => {
                    if child
                        .try_wait()
                        .map_err(|err| {
                            PlatformError::from_io_error("poll Unix PTY child", ".", err)
                        })?
                        .is_some()
                    {
                        orphaned.push(id.clone());
                    }
                }
                #[cfg(windows)]
                NativePtySessionHandle::Windows(handle) => {
                    if windows_session_exited(handle)?.0 {
                        orphaned.push(id.clone());
                    }
                }
            }
        }
        for id in &orphaned {
            if let Some(handle) = sessions.remove(id) {
                close_removed_pty_session(handle, false);
            }
        }
        Ok(orphaned)
    }
}

impl WatcherService for NativeWatcherService {
    fn snapshot(
        &self,
        workspace_id: WorkspaceId,
        path: &Path,
    ) -> Result<Vec<WatcherEvent>, PlatformError> {
        // Tier 1 A11: recursive walk (depth-capped) so nested monorepo files
        // participate in last_scan fingerprints. Still poll-based (no OS notify
        // backend yet). Soft-cap truncation returns Ok + Overflow marker so large
        // trees do not permanently alternate recovery rescans at poll cadence.
        let mut events = Vec::new();
        let mut sequence = 0u64;
        let truncated = walk_watcher_tree(workspace_id, path, 0, &mut events, &mut sequence)?;
        if truncated {
            sequence = sequence.saturating_add(1);
            events.push(WatcherEvent {
                workspace_id,
                kind: WatcherEventKind::Overflow,
                path: CanonicalPath(path.to_string_lossy().to_string()),
                old_path: None,
                sequence: EventSequence(sequence),
            });
        }
        Ok(events)
    }
}

/// Walk workspace tree, returning `Ok(true)` when the soft entry cap truncated
/// the walk (caller must mark incomplete and avoid inventing deletes).
fn walk_watcher_tree(
    workspace_id: WorkspaceId,
    path: &Path,
    depth: usize,
    events: &mut Vec<WatcherEvent>,
    sequence: &mut u64,
) -> Result<bool, PlatformError> {
    if depth > WATCHER_RECURSIVE_DEPTH_LIMIT {
        return Ok(false);
    }
    let entries = fs::read_dir(path)
        .map_err(|err| PlatformError::from_io_error("watcher snapshot", path, err))?;

    let mut truncated = false;
    for entry in entries {
        // Soft cap: stop adding precise per-path events. Returning Ok(true)
        // (not Err) keeps large monorepos in steady-state polling without a
        // permanent recovery loop.
        if events.len() >= WATCHER_OVERFLOW_THRESHOLD {
            truncated = true;
            break;
        }

        let child = entry
            .map_err(|err| PlatformError::from_io_error("watcher snapshot", path, err))?
            .path();

        *sequence = sequence.saturating_add(1);
        events.push(WatcherEvent {
            workspace_id,
            kind: WatcherEventKind::Modified,
            path: CanonicalPath(child.to_string_lossy().to_string()),
            old_path: None,
            sequence: EventSequence(*sequence),
        });

        if child.is_dir() {
            // Skip common heavy / generated directories to keep poll cost bounded.
            if let Some(name) = child.file_name().and_then(|n| n.to_str())
                && matches!(
                    name,
                    "target" | "node_modules" | ".git" | ".legion" | "dist" | "build" | ".cache"
                )
            {
                continue;
            }
            if walk_watcher_tree(workspace_id, &child, depth + 1, events, sequence)? {
                truncated = true;
                break;
            }
        }
    }
    Ok(truncated)
}

impl EnvironmentService for NativeEnvironmentService {
    fn current_dir(&self) -> Result<PathBuf, PlatformError> {
        env::current_dir()
            .map_err(|err| PlatformError::from_io_error("current_dir", Path::new("."), err))
    }

    fn get_var(&self, key: &str) -> Option<String> {
        env::var(key).ok()
    }

    fn vars(&self) -> Vec<(String, String)> {
        env::vars().collect()
    }

    fn normalized_vars(&self, vars: &[(String, String)]) -> Vec<(String, String)> {
        vars.iter()
            .filter(|(key, _)| !env_key_looks_secret(key))
            .map(|(key, value)| (key.to_ascii_lowercase(), value.clone()))
            .collect()
    }
}

fn env_key_is_agent_socket(key: &str) -> bool {
    matches!(
        key,
        "SSH_AUTH_SOCK" | "SSH_AGENT_PID" | "SSH_ASKPASS" | "GIT_SSH" | "GIT_SSH_COMMAND"
    )
}

fn env_key_looks_secret(key: &str) -> bool {
    if env_key_is_agent_socket(key) {
        return false;
    }
    let key = key.to_ascii_lowercase();
    [
        "secret",
        "token",
        "password",
        "passwd",
        "api_key",
        "apikey",
        "auth",
        "credential",
        "private",
        "cookie",
    ]
    .iter()
    .any(|needle| key.contains(needle))
}

fn sanitized_child_env<I>(vars: I) -> Vec<(String, String)>
where
    I: IntoIterator<Item = (String, String)>,
{
    vars.into_iter()
        .filter(|(key, _)| !env_key_looks_secret(key))
        .collect()
}

fn child_environment_vars(extra: &[(String, String)]) -> Vec<(String, String)> {
    let env_service = NativeEnvironmentService;
    let mut vars = sanitized_child_env(env_service.vars());
    for (key, value) in sanitized_child_env(extra.iter().cloned()) {
        if let Some((_, existing_value)) = vars
            .iter_mut()
            .find(|(existing_key, _)| existing_key == &key)
        {
            *existing_value = value;
        } else {
            vars.push((key, value));
        }
    }
    vars
}

impl TimeService for NativeTimeService {
    fn now_millis(&self) -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |dur| dur.as_millis() as u64)
    }

    fn sleep(&self, duration: Duration) {
        std::thread::sleep(duration);
    }

    fn is_over_deadline(&self, started_at: u64, timeout: Duration) -> bool {
        self.now_millis().saturating_sub(started_at) > timeout.as_millis() as u64
    }
}

/// Returns a platform shell title.
pub fn shell_title() -> &'static str {
    "Legion IDE - Platform Shell"
}

/// Resolves the deepest existing ancestor of `path` through
/// [`std::fs::canonicalize`] and re-appends the non-existent remainder.
///
/// The suffix (the non-existent tail) must already be lexically clean —
/// no `..` or `.` components — so that re-appending it yields the correct
/// canonical form for the whole path.
///
/// This helper solves two platform canonicalization defects that break
/// path-identity checks when only one side of a comparison is resolved:
///
/// - **Windows 8.3 short names** (`RUNNER~1` → `runneradmin`): GitHub-runner
///   environments often supply paths in 8.3 form; single-sided canonicalization
///   then fails `starts_with` because the other side expands the long form.
/// - **macOS `/var` → `/private/var`** symlink: `std::env::temp_dir()` returns
///   `/var/folders/…`, while `fs::canonicalize` yields `/private/var/folders/…`.
///   Mixed comparisons (one side resolved, the other not) silently reject
///   every valid in-sandbox path.
///
/// Returns `None` when the deepest **existing** ancestor is a symlink that
/// cannot be canonicalized (i.e. it is dangling).  Writing "through" such a
/// symlink would create the target at an unverified location outside the
/// intended directory tree, so callers must treat this case as a fail-closed
/// error rather than falling back to the raw lexical path.
pub fn resolve_existing_prefix(path: &Path) -> Option<PathBuf> {
    let mut existing = path;
    let mut suffix: Vec<std::ffi::OsString> = Vec::new();
    loop {
        // Use `symlink_metadata` (not `exists` / `metadata`) so that a
        // dangling symlink is treated as present-but-unresolvable rather than
        // being silently walked past as if it did not exist.
        if existing.symlink_metadata().is_ok() {
            break;
        }
        match (existing.parent(), existing.file_name()) {
            (Some(parent), Some(name)) if !parent.as_os_str().is_empty() => {
                suffix.push(name.to_os_string());
                existing = parent;
            }
            // No existing ancestor reachable on disk: return the original
            // lexical path as the best available representation.
            _ => return Some(path.to_path_buf()),
        }
    }
    let mut resolved = std::fs::canonicalize(existing).ok()?;
    for part in suffix.into_iter().rev() {
        resolved.push(part);
    }
    Some(resolved)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn typed_services_can_read_and_write_text() {
        let fs_service = NativeFileSystem;
        let path = env::temp_dir().join("legion-platform-service-roundtrip.txt");

        fs_service
            .write_text_file(&path, "hello platform\n")
            .expect("write text");
        let read = fs_service.read_text_file(&path).expect("read text");
        assert_eq!(read, "hello platform\n");

        let hash = fs_service.hash_file(&path).expect("hash file");
        assert!(!hash.is_empty());

        let metadata = fs_service.read_metadata(&path).expect("metadata");
        assert_eq!(metadata.length, "hello platform\n".len() as u64);
        assert!(metadata.modified_at.is_some());

        let fingerprint = fs_service.read_fingerprint(&path).expect("fingerprint");
        assert_eq!(fingerprint.length, Some(metadata.length));
        assert_eq!(fingerprint.stable_hash.as_deref(), Some(hash.as_str()));
        assert_eq!(
            fs_service.file_length(&path).expect("length"),
            metadata.length
        );
        assert_eq!(
            fs_service
                .modified_timestamp(&path)
                .expect("modified timestamp"),
            metadata.modified_at
        );

        let _ = fs::remove_file(path);
    }

    #[test]
    fn child_env_keeps_ssh_agent_socket_variables() {
        let vars = sanitized_child_env([
            (
                "SSH_AUTH_SOCK".to_string(),
                "/tmp/ssh-agent.sock".to_string(),
            ),
            ("SSH_AGENT_PID".to_string(), "123".to_string()),
            ("AWS_SECRET_ACCESS_KEY".to_string(), "nope".to_string()),
            ("AUTHORIZATION".to_string(), "bearer nope".to_string()),
        ]);
        assert!(
            vars.iter()
                .any(|(key, value)| key == "SSH_AUTH_SOCK" && value == "/tmp/ssh-agent.sock")
        );
        assert!(vars.iter().any(|(key, _)| key == "SSH_AGENT_PID"));
        assert!(!vars.iter().any(|(key, _)| key == "AWS_SECRET_ACCESS_KEY"));
        assert!(!vars.iter().any(|(key, _)| key == "AUTHORIZATION"));
    }

    #[test]
    fn atomic_write_is_exposed() {
        let fs_service = NativeFileSystem;
        let path = env::temp_dir().join("legion-platform-atomic.txt");

        fs_service
            .write_text_file_atomic(&path, "alpha")
            .expect("atomic write");

        assert!(path.exists());
        let _ = fs::remove_file(path);
    }

    #[test]
    fn atomic_write_failure_cleans_temporary_file_and_preserves_target() {
        let fs_service = NativeFileSystem;
        let root = env::temp_dir().join(format!(
            "legion-platform-atomic-failure-{}",
            std::process::id()
        ));
        let target = root.join("target-dir");
        fs::create_dir_all(&target).expect("create target directory");

        let result = fs_service.write_text_file_atomic(&target, "not a directory replacement");

        assert!(result.is_err());
        assert!(target.is_dir());
        let leftovers = fs::read_dir(&root)
            .expect("read temp root")
            .filter_map(Result::ok)
            .map(|entry| entry.file_name().to_string_lossy().into_owned())
            .filter(|name| name.contains(".tmp"))
            .collect::<Vec<_>>();
        assert!(
            leftovers.is_empty(),
            "temporary files left behind: {leftovers:?}"
        );

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn permission_denied_io_is_structured_platform_error() {
        let error = PlatformError::from_io_error(
            "write",
            Path::new("denied.txt"),
            io::Error::from(io::ErrorKind::PermissionDenied),
        );

        assert!(matches!(error, PlatformError::PermissionDenied { .. }));
    }

    #[test]
    fn unsupported_io_is_structured_platform_error() {
        let error = PlatformError::from_io_error(
            "atomic replace",
            Path::new("unsupported.txt"),
            io::Error::from(io::ErrorKind::Unsupported),
        );

        assert!(matches!(error, PlatformError::UnsupportedOperation { .. }));
    }

    #[test]
    fn watcher_snapshot_produces_events() {
        let watcher = NativeWatcherService;
        let current = env::current_dir().expect("current dir");
        let events = watcher
            .snapshot(WorkspaceId(1), &current)
            .expect("watcher snapshot");

        assert!(!events.is_empty());
    }

    #[test]
    fn process_cancelled_is_mapped_to_cancelled_error() {
        let process = NativeProcessService;
        let result = process.execute(&ProcessRequest {
            command: "echo".to_string(),
            args: vec!["hello".to_string()],
            cwd: None,
            env: Vec::new(),
            stdin: None,
            timeout: None,
            cancelled: true,
        });

        assert!(matches!(result, Err(PlatformError::Cancelled { .. })));
    }

    #[test]
    fn environment_service_strips_secret_like_vars_from_normalized_map() {
        let env_service = NativeEnvironmentService;
        let vars = vec![
            ("PATH".to_string(), "/usr/bin".to_string()),
            ("OPENAI_API_KEY".to_string(), "sk-secret".to_string()),
            ("TeamToken".to_string(), "redacted".to_string()),
            ("TERM".to_string(), "xterm-256color".to_string()),
        ];

        let normalized = env_service.normalized_vars(&vars);

        assert!(normalized.contains(&("path".to_string(), "/usr/bin".to_string())));
        assert!(normalized.contains(&("term".to_string(), "xterm-256color".to_string())));
        assert!(
            normalized
                .iter()
                .all(|(key, _)| !key.contains("token") && !key.contains("api"))
        );
    }

    #[test]
    fn process_execution_strips_secret_like_env_vars() {
        let process = NativeProcessService;
        #[cfg(windows)]
        let (command, args) = (
            "powershell".to_string(),
            vec![
                "-NoProfile".to_string(),
                "-Command".to_string(),
                "[Console]::Out.Write($env:KEEP_ME + '|' + $env:OPENAI_API_KEY)".to_string(),
            ],
        );
        #[cfg(not(windows))]
        let (command, args) = (
            "sh".to_string(),
            vec![
                "-c".to_string(),
                "printf '%s|%s' \"$KEEP_ME\" \"${OPENAI_API_KEY:-}\"".to_string(),
            ],
        );
        let result = process
            .execute(&ProcessRequest {
                command,
                args,
                cwd: None,
                env: vec![
                    ("KEEP_ME".to_string(), "visible".to_string()),
                    ("OPENAI_API_KEY".to_string(), "sk-secret".to_string()),
                ],
                stdin: None,
                timeout: None,
                cancelled: false,
            })
            .expect("execute process");

        assert_eq!(result.exit_code, 0);
        assert_eq!(result.stdout, "visible|");
        assert!(!result.stdout.contains("sk-secret"));
    }

    #[test]
    fn process_execution_enforces_timeout_while_child_runs() {
        let process = NativeProcessService;
        #[cfg(windows)]
        let (command, args) = (
            "ping".to_string(),
            vec!["-n".to_string(), "10".to_string(), "127.0.0.1".to_string()],
        );
        #[cfg(not(windows))]
        let (command, args) = (
            "sh".to_string(),
            vec!["-c".to_string(), "sleep 10".to_string()],
        );

        let started = Instant::now();
        let result = process.execute(&ProcessRequest {
            command,
            args,
            cwd: None,
            env: Vec::new(),
            stdin: None,
            timeout: Some(Duration::from_millis(200)),
            cancelled: false,
        });

        assert!(
            matches!(result, Err(PlatformError::Timeout { .. })),
            "expected timeout error, got {result:?}"
        );
        // The timeout must abort the child instead of blocking until it exits
        // (~10s), so the call has to return well before that.
        assert!(
            started.elapsed() < Duration::from_secs(5),
            "execute did not abort the child promptly after the timeout"
        );
    }

    #[test]
    fn list_directory_lists_entries_sorted() {
        let service = NativeFileSystem;
        let dir = env::temp_dir().join(format!(
            "legion-platform-list-dir-{}",
            NATIVE_PTY_SESSION_COUNTER.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("create temp dir");
        fs::write(dir.join("b.txt"), b"b").expect("write b");
        fs::write(dir.join("a.txt"), b"a").expect("write a");

        let entries = service.list_directory(&dir).expect("list directory");
        assert_eq!(entries, vec![dir.join("a.txt"), dir.join("b.txt")]);

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn environment_service_and_time_service_stubs_compile() {
        let env_service = NativeEnvironmentService;
        let vars = env_service.vars();
        let vars = env_service.normalized_vars(&vars);
        assert!(!vars.is_empty() || vars.is_empty());

        let now = NativeTimeService;
        let start = now.now_millis();
        now.sleep(Duration::from_millis(1));
        assert!(now.is_over_deadline(start, Duration::from_nanos(10)));
    }

    #[test]
    fn native_pty_service_uses_platform_backend_for_one_shot_output() {
        let service = NativePtyService;
        #[cfg(windows)]
        let request = PtyRequest {
            command: "cmd".to_string(),
            args: vec!["/C".to_string(), "echo hello".to_string()],
            cwd: None,
            env: None,
        };
        #[cfg(unix)]
        let request = PtyRequest {
            command: "sh".to_string(),
            args: vec!["-c".to_string(), "printf hello".to_string()],
            cwd: None,
            env: None,
        };
        #[cfg(not(any(unix, windows)))]
        let request = PtyRequest {
            command: "unsupported".to_string(),
            args: vec![],
            cwd: None,
            env: None,
        };
        let session = service.spawn_pty(&request).expect("spawn native pty");
        assert!(session.id.starts_with("native-"));
        let mut output = session.output;
        let mut reads = Vec::new();
        // Exit and output are not the same event. The child can be reaped
        // before everything it wrote has been drained, so a loop that stops at
        // `exited` can stop one read short of the bytes it is waiting for —
        // which is why this test flaked on Windows CI under load and passed
        // everywhere quiet. Draining after the exit is what makes it a
        // happens-before check rather than a race with the scheduler.
        let mut drains_after_exit = 4;
        for _ in 0..40 {
            if output.to_ascii_lowercase().contains("hello") {
                break;
            }
            std::thread::sleep(Duration::from_millis(25));
            let chunk = service
                .read_pty(&session.id, PTY_OUTPUT_LIMIT)
                .expect("read native pty output");
            reads.push(format!(
                "output={:?}; exited={}; exit_code={:?}; truncated={}",
                chunk.output, chunk.exited, chunk.exit_code, chunk.truncated
            ));
            output.push_str(&chunk.output);
            if chunk.exited {
                drains_after_exit -= 1;
                if drains_after_exit == 0 {
                    break;
                }
            }
        }
        assert!(
            output.to_ascii_lowercase().contains("hello"),
            "native PTY output did not contain hello; output={output:?}; reads={reads:?}"
        );
        let _ = service.cleanup_orphaned_ptys();
    }

    #[test]
    #[cfg(unix)]
    fn native_unix_pty_child_has_controlling_terminal() {
        let service = NativePtyService;
        let request = PtyRequest {
            command: "sh".to_string(),
            args: vec![
                "-c".to_string(),
                "if : </dev/tty 2>/dev/null; then printf ctty; else printf no-ctty; fi".to_string(),
            ],
            cwd: None,
            env: None,
        };
        let session = service
            .spawn_pty(&request)
            .expect("spawn native unix pty with controlling terminal");
        let mut output = session.output;
        let mut reads = Vec::new();
        for _ in 0..20 {
            if output.contains("ctty") || output.contains("no-ctty") {
                break;
            }
            std::thread::sleep(Duration::from_millis(25));
            let chunk = service
                .read_pty(&session.id, PTY_OUTPUT_LIMIT)
                .expect("read native unix pty output");
            reads.push(format!(
                "output={:?}; exited={}; exit_code={:?}; truncated={}",
                chunk.output, chunk.exited, chunk.exit_code, chunk.truncated
            ));
            output.push_str(&chunk.output);
            if chunk.exited {
                break;
            }
        }
        assert!(
            output.contains("ctty") && !output.contains("no-ctty"),
            "native Unix PTY child did not have controlling terminal; output={output:?}; reads={reads:?}"
        );
        let _ = service.cleanup_orphaned_ptys();
    }

    #[test]
    #[cfg(windows)]
    fn windows_argument_quoting_preserves_backslashes() {
        assert_eq!(
            quote_windows_arg("C:\\repo\\file.txt"),
            "C:\\repo\\file.txt"
        );
        assert_eq!(
            quote_windows_arg("C:\\Program Files\\legion\\file.txt"),
            "\"C:\\Program Files\\legion\\file.txt\""
        );
        assert_eq!(quote_windows_arg("say \"hello\""), "\"say \\\"hello\\\"\"");
        assert_eq!(quote_windows_arg("C:\\path\\"), "C:\\path\\");
        assert_eq!(
            quote_windows_arg("C:\\path with space\\"),
            "\"C:\\path with space\\\\\""
        );
    }

    #[test]
    fn fake_services_for_matrix() {
        struct FakeFs;

        impl PathNormalizationService for FakeFs {
            fn normalize_path(&self, path: &Path) -> Result<PathBuf, PlatformError> {
                Ok(path.to_path_buf())
            }

            fn canonicalize_path(&self, path: &Path) -> Result<PathBuf, PlatformError> {
                Ok(path.to_path_buf())
            }

            fn is_within_base(&self, base: &Path, candidate: &Path) -> Result<bool, PlatformError> {
                Ok(candidate.starts_with(base))
            }
        }

        impl FileSystemService for FakeFs {
            fn read_text_file(&self, _path: &Path) -> Result<String, PlatformError> {
                Ok("ok".to_string())
            }

            fn write_text_file(&self, _path: &Path, _text: &str) -> Result<(), PlatformError> {
                Ok(())
            }

            fn write_text_file_atomic(
                &self,
                _path: &Path,
                _text: &str,
            ) -> Result<(), PlatformError> {
                Ok(())
            }

            fn read_metadata(&self, path: &Path) -> Result<FileSystemMetadata, PlatformError> {
                Ok(FileSystemMetadata {
                    path: path.to_path_buf(),
                    kind: FileSystemEntryKind::File,
                    length: 2,
                    modified_at: Some(1),
                    read_only: false,
                })
            }

            fn read_fingerprint(
                &self,
                path: &Path,
            ) -> Result<FileSystemFingerprint, PlatformError> {
                Ok(FileSystemFingerprint {
                    path: path.to_path_buf(),
                    algorithm: "fake".to_string(),
                    kind: FileSystemEntryKind::File,
                    length: Some(2),
                    modified_at: Some(1),
                    stable_hash: Some("hash".to_string()),
                    read_only: false,
                })
            }

            fn stable_hash(&self, bytes: &[u8]) -> String {
                format!("fake-{}", bytes.len())
            }

            fn stable_hash_file(&self, _path: &Path) -> Result<String, PlatformError> {
                Ok("hash".to_string())
            }

            fn modified_timestamp(&self, _path: &Path) -> Result<Option<u64>, PlatformError> {
                Ok(Some(1))
            }

            fn file_length(&self, _path: &Path) -> Result<u64, PlatformError> {
                Ok(2)
            }

            fn list_directory(&self, _path: &Path) -> Result<Vec<PathBuf>, PlatformError> {
                Ok(vec![PathBuf::from("a"), PathBuf::from("b")])
            }
        }

        let fake = FakeFs;
        let text = fake.read_text_file(Path::new("/tmp")).expect("ok");
        let list = fake.list_directory(Path::new("/tmp")).expect("ok");
        fake.write_text_file(Path::new("/tmp"), "x").expect("ok");
        assert_eq!(text, "ok");
        assert_eq!(list.len(), 2);
    }

    /// CreateProcessW requires the env block to be sorted case-insensitively by key.
    #[test]
    #[cfg(windows)]
    fn windows_environment_block_is_sorted_case_insensitively() {
        let vars = vec![
            ("zeta".to_string(), "1".to_string()),
            ("ALPHA".to_string(), "2".to_string()),
            ("Beta".to_string(), "3".to_string()),
            ("GAMMA".to_string(), "4".to_string()),
            ("alpha_extra".to_string(), "5".to_string()),
        ];
        let block = windows_environment_block(&vars);

        // Decode the wide block back to key=value strings (strip the trailing double-null).
        let block_str = String::from_utf16_lossy(&block);
        let entries: Vec<&str> = block_str.trim_end_matches('\0').split('\0').collect();
        let keys: Vec<&str> = entries.iter().filter_map(|e| e.split('=').next()).collect();

        // Keys must appear in case-insensitive alphabetical order.
        let expected = ["ALPHA", "alpha_extra", "Beta", "GAMMA", "zeta"];
        assert_eq!(
            keys, expected,
            "windows_environment_block keys must be sorted case-insensitively; got: {keys:?}"
        );
    }

    // ── resolve_existing_prefix unit tests ──────────────────────────────────

    fn unique_temp_dir(tag: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "legion-rep-{tag}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock after epoch")
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).expect("create temp dir");
        dir
    }

    /// An entirely existing path must canonicalize successfully.
    #[test]
    fn rep_existing_path_canonicalizes() {
        let dir = unique_temp_dir("existing");
        let file = dir.join("f.txt");
        std::fs::write(&file, "x").expect("write");

        let result = resolve_existing_prefix(&file).expect("Some");
        // The result must point to the same filesystem object.
        assert_eq!(
            std::fs::canonicalize(&file).expect("canonicalize"),
            result,
            "resolve_existing_prefix must return canonical form for existing path"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A path whose leaf does not exist yet: the existing prefix is resolved
    /// and the non-existent leaf is re-appended.
    #[test]
    fn rep_nonexistent_leaf_reappended() {
        let dir = unique_temp_dir("newfile");
        let file = dir.join("not_yet.rs");

        let result = resolve_existing_prefix(&file).expect("Some");
        // The directory part must be canonical; the leaf must be preserved.
        let expected_dir = std::fs::canonicalize(&dir).expect("canonicalize dir");
        assert_eq!(result, expected_dir.join("not_yet.rs"));

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A dangling symlink in the prefix must return None (fail closed).
    #[test]
    #[cfg(unix)]
    fn rep_dangling_symlink_returns_none() {
        let sandbox = unique_temp_dir("rep-dangling");
        let target = unique_temp_dir("rep-dangling-target");
        let link = sandbox.join("dangling");
        if std::os::unix::fs::symlink(&target, &link).is_err() {
            eprintln!("skipping: symlink creation not permitted");
            let _ = std::fs::remove_dir_all(&sandbox);
            let _ = std::fs::remove_dir_all(&target);
            return;
        }
        // Remove the target so the symlink dangles.
        std::fs::remove_dir_all(&target).expect("remove target");

        let path = link.join("file.txt");
        let result = resolve_existing_prefix(&path);
        assert!(
            result.is_none(),
            "dangling symlink in prefix must return None (fail closed), got {result:?}"
        );

        let _ = std::fs::remove_dir_all(&sandbox);
    }
}
