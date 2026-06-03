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
        Mutex, OnceLock,
        atomic::{AtomicU64, Ordering},
    },
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

#[cfg(windows)]
use std::sync::Arc;

use devil_protocol::{CanonicalPath, EventSequence, WatcherEvent, WatcherEventKind, WorkspaceId};
use thiserror::Error;

/// Maximum event count accepted from one watcher snapshot.
const WATCHER_OVERFLOW_THRESHOLD: usize = 4_096;

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
    /// Optional timeout.
    pub timeout: Option<Duration>,
    /// Cancellation flag.
    pub cancelled: bool,
}

impl ProcessRequest {
    /// Constructs a new request for `command`.
    pub fn new(command: impl Into<String>) -> Self {
        Self {
            command: command.into(),
            args: Vec::new(),
            cwd: None,
            env: Vec::new(),
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
            algorithm: "devil-stable-fingerprint-v1".to_string(),
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
        let mut entries = fs::read_dir(path)
            .map_err(|err| PlatformError::from_io_error("list directory", path, err))?
            .filter_map(|entry| entry.ok())
            .map(|entry| entry.path())
            .collect::<Vec<_>>();
        entries.sort();
        Ok(entries)
    }
}

impl ProcessService for NativeProcessService {
    fn execute(&self, request: &ProcessRequest) -> Result<ProcessResult, PlatformError> {
        if request.cancelled {
            return Err(PlatformError::Cancelled {
                operation: "execute".to_string(),
            });
        }

        let started = Instant::now();
        let mut command = Command::new(&request.command);
        command.args(&request.args);

        if let Some(cwd) = &request.cwd {
            command.current_dir(cwd);
        }

        for (key, value) in &request.env {
            command.env(key, value);
        }

        let output = command
            .output()
            .map_err(|err| PlatformError::ProcessSpawnFailure {
                operation: "execute".to_string(),
                command: request.command.clone(),
                message: err.to_string(),
            })?;

        let elapsed = started.elapsed();
        if let Some(timeout) = request.timeout
            && elapsed > timeout
        {
            return Err(PlatformError::Timeout {
                operation: format!("process `{}`", request.command),
                duration: elapsed,
            });
        }

        Ok(ProcessResult {
            exit_code: output.status.code().unwrap_or(-1),
            stdout: String::from_utf8(output.stdout).unwrap_or_default(),
            stderr: String::from_utf8(output.stderr).unwrap_or_default(),
            elapsed,
        })
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
    if let Some(cwd) = &request.cwd {
        command.current_dir(cwd);
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
    use std::mem::{size_of, zeroed};
    use std::ptr;
    use windows::Win32::Foundation::{CloseHandle, HANDLE};
    use windows::Win32::System::Console::{COORD, CreatePseudoConsole, HPCON, ResizePseudoConsole};
    use windows::Win32::System::Pipes::CreatePipe;
    use windows::Win32::System::Threading::{
        CREATE_NEW_PROCESS_GROUP, CREATE_UNICODE_ENVIRONMENT, CreateProcessW,
        DeleteProcThreadAttributeList, EXTENDED_STARTUPINFO_PRESENT,
        InitializeProcThreadAttributeList, LPPROC_THREAD_ATTRIBUTE_LIST,
        PROC_THREAD_ATTRIBUTE_PSEUDOCONSOLE, PROCESS_INFORMATION, STARTF_USESTDHANDLES,
        STARTUPINFOEXW, UpdateProcThreadAttribute,
    };
    use windows::core::{PCWSTR, PWSTR};

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

        let spawn_result = CreateProcessW(
            PCWSTR(ptr::null()),
            Some(PWSTR(command_line.as_mut_ptr())),
            None,
            None,
            false,
            EXTENDED_STARTUPINFO_PRESENT | CREATE_UNICODE_ENVIRONMENT | CREATE_NEW_PROCESS_GROUP,
            None,
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
        use windows::Win32::Foundation::{CloseHandle, HANDLE};
        use windows::Win32::Storage::FileSystem::ReadFile;
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
    use windows::Win32::Foundation::HANDLE;
    use windows::Win32::Storage::FileSystem::WriteFile;

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
    use windows::Win32::Foundation::HANDLE;
    use windows::Win32::System::Threading::GetExitCodeProcess;
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
    use windows::Win32::Foundation::{CloseHandle, HANDLE};
    use windows::Win32::System::Console::{ClosePseudoConsole, HPCON};
    use windows::Win32::System::Threading::{TerminateProcess, WaitForSingleObject};
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
    conpty: Option<windows::Win32::System::Console::HPCON>,
    handles: &[windows::Win32::Foundation::HANDLE],
) {
    use windows::Win32::Foundation::CloseHandle;
    use windows::Win32::System::Console::ClosePseudoConsole;
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
                use windows::Win32::System::Console::{COORD, HPCON, ResizePseudoConsole};
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
                    let target = if matches!(mode, PtyKillMode::Interrupt | PtyKillMode::KillTree) {
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
                        Ok(())
                    } else {
                        Err(PlatformError::PtyUnavailable {
                            reason: format!(
                                "Unix PTY session `{session_id}` did not exit after {signal:?}"
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
        let mut events = Vec::new();
        let mut sequence = 0u64;

        let entries = fs::read_dir(path)
            .map_err(|err| PlatformError::from_io_error("watcher snapshot", path, err))?;

        for entry in entries {
            if events.len() >= WATCHER_OVERFLOW_THRESHOLD {
                return Err(PlatformError::WatcherOverflow {
                    path: path.to_path_buf(),
                    context: "overflow threshold exceeded".to_string(),
                });
            }

            let path = entry
                .map_err(|err| PlatformError::from_io_error("watcher snapshot", path, err))?
                .path();

            sequence = sequence.saturating_add(1);
            events.push(WatcherEvent {
                workspace_id,
                kind: WatcherEventKind::Modified,
                path: CanonicalPath(path.to_string_lossy().to_string()),
                old_path: None,
                sequence: EventSequence(sequence),
            });
        }

        Ok(events)
    }
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
            .map(|(key, value)| (key.to_ascii_lowercase(), value.clone()))
            .collect()
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn typed_services_can_read_and_write_text() {
        let fs_service = NativeFileSystem;
        let path = env::temp_dir().join("devil-platform-service-roundtrip.txt");

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
    fn atomic_write_is_exposed() {
        let fs_service = NativeFileSystem;
        let path = env::temp_dir().join("devil-platform-atomic.txt");

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
            "devil-platform-atomic-failure-{}",
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
            timeout: None,
            cancelled: true,
        });

        assert!(matches!(result, Err(PlatformError::Cancelled { .. })));
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
        };
        #[cfg(unix)]
        let request = PtyRequest {
            command: "sh".to_string(),
            args: vec!["-c".to_string(), "printf hello".to_string()],
            cwd: None,
        };
        #[cfg(not(any(unix, windows)))]
        let request = PtyRequest {
            command: "unsupported".to_string(),
            args: vec![],
            cwd: None,
        };
        let session = service.spawn_pty(&request).expect("spawn native pty");
        assert!(session.id.starts_with("native-"));
        let mut output = session.output;
        let mut reads = Vec::new();
        for _ in 0..20 {
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
                break;
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
            quote_windows_arg("C:\\Program Files\\devil\\file.txt"),
            "\"C:\\Program Files\\devil\\file.txt\""
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
}
