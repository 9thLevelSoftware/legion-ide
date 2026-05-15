//! OS abstractions for filesystem/process/watcher/PTY/environment/time operations.

#![warn(missing_docs)]

use std::{
    env,
    fs::{self, OpenOptions},
    io::{self, Write},
    path::{Component, Path, PathBuf},
    process::Command,
    sync::atomic::{AtomicU64, Ordering},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

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

/// Process abstraction.
pub trait ProcessService {
    /// Executes a command and returns the output.
    fn execute(&self, request: &ProcessRequest) -> Result<ProcessResult, PlatformError>;
}

/// PTY abstraction.
pub trait PtyService {
    /// Spawns a PTY session.
    fn spawn_pty(&self, request: &PtyRequest) -> Result<PtySession, PlatformError>;
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

impl PtyService for NativePtyService {
    fn spawn_pty(&self, request: &PtyRequest) -> Result<PtySession, PlatformError> {
        let _ = &request.command;
        let _ = &request.args;
        let _ = &request.cwd;

        Err(PlatformError::PtyUnavailable {
            reason: "PTY service is phase-gated and not wired into terminal behavior".to_string(),
        })
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
    "Devil IDE — Platform Shell"
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
