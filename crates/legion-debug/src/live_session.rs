//! Live DAP adapter process session (B1 scaffold).
//!
//! Spawns an adapter binary, performs `initialize` + waits for `initialized`,
//! then `disconnect`. Full launch/breakpoints land in B2.
//!
//! CI uses the in-tree `fake_dap_adapter` binary; real CodeLLDB is B3.

use std::io::{BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use serde_json::json;
use thiserror::Error;

use crate::framing::{DapFrameError, DapFramer, DapJsonRpc};
use crate::state::DapLifecycleState;

/// Errors from a live DAP session.
#[derive(Debug, Error)]
pub enum LiveDapSessionError {
    /// Adapter process could not be started.
    #[error("DAP adapter spawn failed: {message}")]
    Spawn {
        /// Bounded diagnostic.
        message: String,
    },
    /// Wire framing or I/O failed.
    #[error("DAP session I/O failed: {source}")]
    Io {
        /// Framing source.
        #[from]
        source: DapFrameError,
    },
    /// Protocol sequence unexpected.
    #[error("DAP protocol error: {message}")]
    Protocol {
        /// Bounded diagnostic.
        message: String,
    },
    /// Timed out waiting for adapter.
    #[error("DAP session timed out: {message}")]
    Timeout {
        /// Bounded diagnostic.
        message: String,
    },
}

/// Outcome of a live initialize handshake.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LiveDapHandshakeOutcome {
    /// Lifecycle after successful initialize.
    pub lifecycle_state: DapLifecycleState,
    /// Adapter type label from launch request / binary.
    pub adapter_type: String,
    /// Whether `initialized` event was observed.
    pub initialized_event: bool,
    /// Metadata-only summary for audit projections.
    pub metadata_summary: String,
}

/// Supervised live DAP session handle.
pub struct LiveDapSession {
    child: Child,
    stdin: Option<std::process::ChildStdin>,
    stdout: BufReader<std::process::ChildStdout>,
    next_id: u64,
    adapter_type: String,
}

impl LiveDapSession {
    /// Spawn `adapter_program` with optional args (stdio DAP).
    pub fn spawn(
        adapter_program: impl AsRef<Path>,
        args: &[String],
        adapter_type: impl Into<String>,
    ) -> Result<Self, LiveDapSessionError> {
        let program = adapter_program.as_ref();
        let mut command = Command::new(program);
        command
            .args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());
        let mut child = command.spawn().map_err(|err| LiveDapSessionError::Spawn {
            message: format!("{}: {err}", program.display()),
        })?;
        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| LiveDapSessionError::Spawn {
                message: "missing stdin pipe".to_string(),
            })?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| LiveDapSessionError::Spawn {
                message: "missing stdout pipe".to_string(),
            })?;
        Ok(Self {
            child,
            stdin: Some(stdin),
            stdout: BufReader::new(stdout),
            next_id: 1,
            adapter_type: adapter_type.into(),
        })
    }

    /// Run initialize → wait for initialized event → return handshake outcome.
    pub fn initialize_handshake(
        &mut self,
        timeout: Duration,
    ) -> Result<LiveDapHandshakeOutcome, LiveDapSessionError> {
        let id = self.alloc_id();
        let req = DapJsonRpc::request(
            id,
            "initialize",
            json!({
                "clientID": "legion",
                "clientName": "Legion IDE",
                "adapterID": self.adapter_type,
                "pathFormat": "path",
                "linesStartAt1": true,
                "columnsStartAt1": true,
            }),
        );
        let stdin = self
            .stdin
            .as_mut()
            .ok_or_else(|| LiveDapSessionError::Protocol {
                message: "stdin already closed".to_string(),
            })?;
        DapFramer::write_to(stdin, &req)?;

        let deadline = Instant::now() + timeout;
        let mut saw_initialize_response = false;
        let mut saw_initialized_event = false;

        while Instant::now() < deadline {
            // Blocking read with overall deadline enforced by outer loop length;
            // fake adapter is local and fast. For real adapters B2 may add non-block.
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                break;
            }
            let msg = DapFramer::read_from(&mut self.stdout)?;
            if msg.method.as_deref() == Some("initialized") {
                saw_initialized_event = true;
            }
            if msg.id == Some(serde_json::Value::from(id)) {
                if msg.error.is_some() {
                    return Err(LiveDapSessionError::Protocol {
                        message: format!("initialize error: {:?}", msg.error),
                    });
                }
                saw_initialize_response = true;
            }
            if saw_initialize_response && saw_initialized_event {
                return Ok(LiveDapHandshakeOutcome {
                    lifecycle_state: DapLifecycleState::Launching,
                    adapter_type: self.adapter_type.clone(),
                    initialized_event: true,
                    metadata_summary: format!(
                        "action=initialize state=launching adapter={} initialized=true live=true",
                        self.adapter_type
                    ),
                });
            }
        }

        Err(LiveDapSessionError::Timeout {
            message: format!(
                "initialize response={saw_initialize_response} initialized_event={saw_initialized_event}"
            ),
        })
    }

    /// Send disconnect and wait for process exit (best-effort).
    pub fn disconnect_and_wait(mut self, timeout: Duration) -> Result<(), LiveDapSessionError> {
        let id = self.alloc_id();
        let req = DapJsonRpc::request(
            id,
            "disconnect",
            json!({ "restart": false, "terminateDebuggee": true }),
        );
        if let Some(mut stdin) = self.stdin.take() {
            let _ = DapFramer::write_to(&mut stdin, &req);
            let _ = stdin.flush();
        }

        let deadline = Instant::now() + timeout;
        loop {
            match self.child.try_wait() {
                Ok(Some(_)) => return Ok(()),
                Ok(None) if Instant::now() >= deadline => {
                    let _ = self.child.kill();
                    let _ = self.child.wait();
                    return Ok(());
                }
                Ok(None) => std::thread::sleep(Duration::from_millis(10)),
                Err(err) => {
                    return Err(LiveDapSessionError::Spawn {
                        message: format!("wait failed: {err}"),
                    });
                }
            }
        }
    }

    fn alloc_id(&mut self) -> u64 {
        let id = self.next_id;
        self.next_id = self.next_id.saturating_add(1);
        id
    }
}

impl Drop for LiveDapSession {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// Resolve the path to the in-tree fake DAP adapter built by cargo.
///
/// Looks next to the current test executable (`CARGO_BIN_EXE_fake_dap_adapter`
/// when available) or `target/{debug,release}/fake_dap_adapter(.exe)`.
pub fn fake_dap_adapter_path() -> Option<PathBuf> {
    if let Ok(path) = std::env::var("CARGO_BIN_EXE_fake_dap_adapter") {
        let p = PathBuf::from(path);
        if p.exists() {
            return Some(p);
        }
    }
    let target = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target");
    for profile in ["debug", "release"] {
        let mut candidate = target.join(profile).join("fake_dap_adapter");
        if cfg!(windows) {
            candidate.set_extension("exe");
        }
        if candidate.exists() {
            return Some(candidate);
        }
    }
    None
}
