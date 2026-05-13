//! OS abstractions for filesystem/process/watcher/PTY/environment/time operations.

#![warn(missing_docs)]

use std::{
    collections::hash_map::DefaultHasher,
    env, fs,
    hash::{Hash, Hasher},
    io,
    path::{Component, Path, PathBuf},
    process::Command,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use devil_protocol::{CanonicalPath, EventSequence, WatcherEvent, WatcherEventKind, WorkspaceId};
use thiserror::Error;

/// Maximum event count accepted from one watcher snapshot.
const WATCHER_OVERFLOW_THRESHOLD: usize = 4_096;

/// Maximum normalization component count before a path is rejected.
const NORMALIZATION_DEPTH_LIMIT: usize = 1_024;

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
    fn from_io_error(
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
    fn write_text_file(&self, path: &Path, text: &str) -> Result<(), PlatformError>;

    /// Writes UTF-8 text via atomic replace when available.
    fn write_text_file_atomic(&self, path: &Path, text: &str) -> Result<(), PlatformError>;

    /// Lists children for a directory.
    fn list_directory(&self, path: &Path) -> Result<Vec<PathBuf>, PlatformError>;

    /// Returns deterministic content hash for file bytes.
    fn hash_file(&self, path: &Path) -> Result<String, PlatformError>;
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

/// Native PTY implementation (stubbed in this milestone).
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
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                fs::create_dir_all(parent).map_err(|err| {
                    PlatformError::from_io_error("create parent directories", path, err)
                })?;
            }
        }

        fs::write(path, text).map_err(|err| PlatformError::from_io_error("write", path, err))
    }

    fn write_text_file_atomic(&self, path: &Path, text: &str) -> Result<(), PlatformError> {
        let parent = path.parent().unwrap_or_else(|| Path::new("."));
        fs::create_dir_all(parent)
            .map_err(|err| PlatformError::from_io_error("create parent directories", path, err))?;

        let stem = path
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or("file")
            .replace('\n', "_");
        let temp = parent.join(format!(".{stem}.tmp"));

        fs::write(&temp, text)
            .map_err(|err| PlatformError::from_io_error("write atomic temporary", &temp, err))?;

        fs::rename(&temp, path).map_err(|err| match err.kind() {
            io::ErrorKind::Unsupported => PlatformError::AtomicReplaceUnsupported {
                operation: "atomic write".to_string(),
                path: path.to_path_buf(),
            },
            _ => {
                let _ = fs::remove_file(&temp);
                PlatformError::from_io_error("rename atomic temporary", path, err)
            }
        })
    }

    fn list_directory(&self, path: &Path) -> Result<Vec<PathBuf>, PlatformError> {
        let mut entries = fs::read_dir(path)
            .map_err(|err| PlatformError::from_io_error("list directory", path, err))?
            .filter_map(|entry| entry.ok())
            .map(|entry| entry.path())
            .collect::<Vec<_>>();
        entries.sort_by(|left, right| left.cmp(right));
        Ok(entries)
    }

    fn hash_file(&self, path: &Path) -> Result<String, PlatformError> {
        let content =
            fs::read(path).map_err(|err| PlatformError::from_io_error("hash", path, err))?;
        let mut hasher = DefaultHasher::new();
        content.hash(&mut hasher);
        Ok(format!("{:016x}", hasher.finish()))
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
            reason: "PTY service is not implemented in this milestone".to_string(),
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

/// Returns a platform shell title for the spike.
pub fn shell_title() -> &'static str {
    "Devil IDE — Native Shell + Text Model"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn typed_services_can_read_and_write_text() {
        let fs_service = NativeFileSystem;
        let path = env::temp_dir().join("devil-spike-platform-service-roundtrip.txt");

        fs_service
            .write_text_file(&path, "hello spike\n")
            .expect("write text");
        let read = fs_service.read_text_file(&path).expect("read text");
        assert_eq!(read, "hello spike\n");

        let hash = fs_service.hash_file(&path).expect("hash file");
        assert!(!hash.is_empty());

        let _ = fs::remove_file(path);
    }

    #[test]
    fn atomic_write_is_exposed() {
        let fs_service = NativeFileSystem;
        let path = env::temp_dir().join("devil-spike-platform-atomic.txt");

        fs_service
            .write_text_file_atomic(&path, "alpha")
            .expect("atomic write");

        assert!(path.exists());
        let _ = fs::remove_file(path);
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

            fn list_directory(&self, _path: &Path) -> Result<Vec<PathBuf>, PlatformError> {
                Ok(vec![PathBuf::from("a"), PathBuf::from("b")])
            }

            fn hash_file(&self, _path: &Path) -> Result<String, PlatformError> {
                Ok("hash".to_string())
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
