//! Live DAP adapter process session (B1/B2/B4).
//!
//! Spawns an adapter binary and drives a minimal DAP product loop over the
//! **Microsoft DAP** wire (`seq`/`type`/`command`/`arguments`):
//! initialize → setBreakpoints → launch/configurationDone → stopped →
//! stackTrace/variables → step/continue → disconnect.
//!
//! CI uses the in-tree `fake_dap_adapter` binary (same wire shape as real
//! CodeLLDB / `lldb-dap`).

use std::io::{BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::{Receiver, RecvTimeoutError, sync_channel};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde_json::{Value, json};
use thiserror::Error;

use crate::framing::{DapFrameError, DapFramer, DapMessage};
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

/// One verified breakpoint from `setBreakpoints`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LiveBreakpoint {
    /// Adapter breakpoint id when present.
    pub id: Option<u64>,
    /// Source line (1-based when adapter uses linesStartAt1).
    pub line: u64,
    /// Whether the adapter verified the breakpoint.
    pub verified: bool,
    /// Optional adapter message.
    pub message: Option<String>,
}

/// One stack frame from `stackTrace`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LiveStackFrame {
    /// Frame id.
    pub id: u64,
    /// Frame name.
    pub name: String,
    /// Source path when present.
    pub path: Option<String>,
    /// Line number.
    pub line: u64,
}

/// One variable from `variables`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LiveVariable {
    /// Variable name.
    pub name: String,
    /// Display value.
    pub value: String,
    /// Type label when present.
    pub type_label: Option<String>,
}

/// Outcome of launch through first stop.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LiveDapStopOutcome {
    /// Lifecycle after stop.
    pub lifecycle_state: DapLifecycleState,
    /// DAP stop reason (`entry`, `step`, `breakpoint`, …).
    pub reason: String,
    /// Thread id from the stopped event.
    pub thread_id: u64,
    /// Stack frames after stop.
    pub stack_frames: Vec<LiveStackFrame>,
    /// Locals from the top frame when available.
    pub variables: Vec<LiveVariable>,
    /// Metadata-only summary.
    pub metadata_summary: String,
}

/// Supervised live DAP session handle.
pub struct LiveDapSession {
    child: Child,
    stdin: Option<std::process::ChildStdin>,
    /// Frames decoded off the adapter's stdout by a reader thread.
    ///
    /// Every read used to be `DapFramer::read_from(&mut self.stdout)`, a
    /// blocking call inside a `while Instant::now() < deadline` loop — so the
    /// deadline was only consulted *between* frames and could not fire while
    /// waiting for one. An adapter that answered nothing blocked forever, and
    /// the CI job's 60-minute timeout was the only thing that stopped it. A
    /// timeout that cannot fire is worse than none: it made a hang look like a
    /// protocol failure and hid which of the two was happening.
    frames: Receiver<Result<DapMessage, DapFrameError>>,
    /// Whatever the adapter wrote to stderr, for error messages.
    ///
    /// Previously `Stdio::null()`, which discarded the adapter's own account of
    /// why it was unhappy — the single most useful thing to have when a
    /// handshake fails.
    stderr: Arc<Mutex<String>>,
    next_seq: u64,
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
            .stderr(Stdio::piped());
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
        let stderr = child.stderr.take();
        Self::from_parts(child, stdin, stdout, stderr, adapter_type)
    }

    /// Build a session from an already-spawned child with stdio pipes (C4 sandbox).
    pub fn from_stdio(
        child: std::process::Child,
        stdin: std::process::ChildStdin,
        stdout: std::process::ChildStdout,
        adapter_type: impl Into<String>,
    ) -> Result<Self, LiveDapSessionError> {
        Self::from_parts(child, stdin, stdout, None, adapter_type)
    }

    /// Build a session, moving stdout (and stderr, when piped) onto reader
    /// threads so every wait can honour a deadline.
    fn from_parts(
        child: std::process::Child,
        stdin: std::process::ChildStdin,
        stdout: std::process::ChildStdout,
        stderr: Option<std::process::ChildStderr>,
        adapter_type: impl Into<String>,
    ) -> Result<Self, LiveDapSessionError> {
        // Bounded so a chatty adapter cannot grow this without limit; DAP
        // traffic for one session is small and the consumer keeps up.
        let (tx, frames) = sync_channel(64);
        std::thread::Builder::new()
            .name("legion-dap-reader".to_string())
            .spawn(move || {
                let mut reader = BufReader::new(stdout);
                loop {
                    let frame = DapFramer::read_from(&mut reader);
                    let failed = frame.is_err();
                    // A closed receiver means the session is gone; stop rather
                    // than block forever on a send nobody will take.
                    if tx.send(frame).is_err() || failed {
                        break;
                    }
                }
            })
            .map_err(|err| LiveDapSessionError::Spawn {
                message: format!("cannot start DAP reader thread: {err}"),
            })?;

        let captured = Arc::new(Mutex::new(String::new()));
        if let Some(mut stderr) = stderr {
            let sink = Arc::clone(&captured);
            // Best effort: a failure to read the adapter's complaints must not
            // fail the session, only leave the error message thinner.
            let _ = std::thread::Builder::new()
                .name("legion-dap-stderr".to_string())
                .spawn(move || {
                    let mut buffer = String::new();
                    if stderr.read_to_string(&mut buffer).is_ok()
                        && let Ok(mut sink) = sink.lock()
                    {
                        sink.push_str(&buffer);
                    }
                });
        }

        Ok(Self {
            child,
            stdin: Some(stdin),
            frames,
            stderr: captured,
            next_seq: 1,
            adapter_type: adapter_type.into(),
        })
    }

    /// Run initialize → wait for initialize response + initialized event.
    pub fn initialize_handshake(
        &mut self,
        timeout: Duration,
    ) -> Result<LiveDapHandshakeOutcome, LiveDapSessionError> {
        let seq = self.alloc_seq();
        let req = DapMessage::request(
            seq,
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
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                break;
            }
            let msg = self.read_frame(deadline)?;
            if msg.event_name() == Some("initialized") {
                saw_initialized_event = true;
            }
            if let Some(result) = msg.response_for(seq) {
                result.map_err(|message| LiveDapSessionError::Protocol {
                    message: format!("initialize error: {message}"),
                })?;
                saw_initialize_response = true;
            }
            // The `initialize` response alone completes this step. Waiting for
            // the `initialized` event as well is what made every real adapter
            // fail: per the DAP sequence the adapter sends `initialized` when
            // it is ready for configuration, which is after `launch`/`attach`,
            // not after `initialize`. lldb-dap follows that on all three
            // platforms; the in-tree fake adapter sends it early, so every test
            // against the fake passed while the product hung against anything
            // real. The event is still recorded when an adapter volunteers it.
            if saw_initialize_response {
                return Ok(LiveDapHandshakeOutcome {
                    lifecycle_state: DapLifecycleState::Launching,
                    adapter_type: self.adapter_type.clone(),
                    initialized_event: saw_initialized_event,
                    metadata_summary: format!(
                        "action=initialize state=launching adapter={} initialized={saw_initialized_event} live=true wire=microsoft-dap",
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

    /// `setBreakpoints` for one source path and line list.
    pub fn set_breakpoints(
        &mut self,
        path: &str,
        lines: &[u64],
        timeout: Duration,
    ) -> Result<Vec<LiveBreakpoint>, LiveDapSessionError> {
        let breakpoints: Vec<Value> = lines.iter().map(|line| json!({ "line": line })).collect();
        let result = self.request(
            "setBreakpoints",
            json!({
                "source": { "path": path, "name": path.rsplit(['/', '\\']).next().unwrap_or(path) },
                "breakpoints": breakpoints,
                "sourceModified": false
            }),
            timeout,
        )?;
        let list = result
            .get("breakpoints")
            .and_then(|b| b.as_array())
            .cloned()
            .unwrap_or_default();
        Ok(list
            .into_iter()
            .map(|bp| LiveBreakpoint {
                id: bp.get("id").and_then(|v| v.as_u64()),
                line: bp.get("line").and_then(|v| v.as_u64()).unwrap_or(0),
                verified: bp
                    .get("verified")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false),
                message: bp
                    .get("message")
                    .and_then(|v| v.as_str())
                    .map(str::to_string),
            })
            .collect())
    }

    /// `launch` + `configurationDone`, then wait for `stopped`.
    pub fn launch_until_stopped(
        &mut self,
        program: &str,
        timeout: Duration,
    ) -> Result<LiveDapStopOutcome, LiveDapSessionError> {
        self.launch_until_stopped_with(program, None, false, timeout)
    }

    /// `launch` with optional working directory and `stopOnEntry` (B13).
    ///
    /// System adapters (lldb-dap / CodeLLDB) commonly need `cwd` and prefer
    /// `stopOnEntry` for a deterministic first stop during dogfood.
    pub fn launch_until_stopped_with(
        &mut self,
        program: &str,
        cwd: Option<&str>,
        stop_on_entry: bool,
        timeout: Duration,
    ) -> Result<LiveDapStopOutcome, LiveDapSessionError> {
        let mut arguments = json!({
            "name": "legion-live",
            "type": self.adapter_type,
            "request": "launch",
            "program": program,
            "stopOnEntry": stop_on_entry,
        });
        if let Some(cwd) = cwd
            && let Some(obj) = arguments.as_object_mut()
        {
            obj.insert("cwd".to_string(), json!(cwd));
        }
        let _ = self.request("launch", arguments, timeout)?;
        let _ = self.request("configurationDone", json!({}), timeout)?;
        self.wait_stopped_and_inspect("entry", timeout)
    }

    /// `next` (step over), then wait for `stopped` and inspect stack/locals.
    pub fn step_over_until_stopped(
        &mut self,
        thread_id: u64,
        timeout: Duration,
    ) -> Result<LiveDapStopOutcome, LiveDapSessionError> {
        self.step_command_until_stopped("next", thread_id, timeout)
    }

    /// `stepIn`, then wait for `stopped`.
    pub fn step_in_until_stopped(
        &mut self,
        thread_id: u64,
        timeout: Duration,
    ) -> Result<LiveDapStopOutcome, LiveDapSessionError> {
        self.step_command_until_stopped("stepIn", thread_id, timeout)
    }

    /// `stepOut`, then wait for `stopped`.
    pub fn step_out_until_stopped(
        &mut self,
        thread_id: u64,
        timeout: Duration,
    ) -> Result<LiveDapStopOutcome, LiveDapSessionError> {
        self.step_command_until_stopped("stepOut", thread_id, timeout)
    }

    /// Step command (`next` / `stepIn` / `stepOut`) then inspect.
    pub fn step_command_until_stopped(
        &mut self,
        command: &str,
        thread_id: u64,
        timeout: Duration,
    ) -> Result<LiveDapStopOutcome, LiveDapSessionError> {
        let _ = self.request(command, json!({ "threadId": thread_id }), timeout)?;
        self.wait_stopped_and_inspect("step", timeout)
    }

    /// `continue` and wait for the `continued` event (or response only).
    pub fn continue_execution(
        &mut self,
        thread_id: u64,
        timeout: Duration,
    ) -> Result<(), LiveDapSessionError> {
        let _ = self.request("continue", json!({ "threadId": thread_id }), timeout)?;
        let deadline = Instant::now() + timeout;
        while Instant::now() < deadline {
            let msg = self.read_frame(deadline)?;
            if msg.event_name() == Some("continued") {
                return Ok(());
            }
            // B6: fake/real adapters may stop again before we observe continued.
            if msg.event_name() == Some("stopped") {
                return Ok(());
            }
        }
        Ok(())
    }

    /// `continue`, then wait for the next `stopped` (breakpoint / pause) and inspect.
    ///
    /// Product path for "continue until next stop" after B5 persistent sessions.
    pub fn continue_until_stopped(
        &mut self,
        thread_id: u64,
        timeout: Duration,
    ) -> Result<LiveDapStopOutcome, LiveDapSessionError> {
        let _ = self.request("continue", json!({ "threadId": thread_id }), timeout)?;
        // `continued` is optional; wait_stopped ignores non-stopped events.
        self.wait_stopped_and_inspect("breakpoint", timeout)
    }

    /// `pause` request, then wait for `stopped`.
    pub fn pause_until_stopped(
        &mut self,
        thread_id: u64,
        timeout: Duration,
    ) -> Result<LiveDapStopOutcome, LiveDapSessionError> {
        let _ = self.request("pause", json!({ "threadId": thread_id }), timeout)?;
        self.wait_stopped_and_inspect("pause", timeout)
    }

    fn wait_stopped_and_inspect(
        &mut self,
        expected_reason_hint: &str,
        timeout: Duration,
    ) -> Result<LiveDapStopOutcome, LiveDapSessionError> {
        let deadline = Instant::now() + timeout;
        let mut reason = expected_reason_hint.to_string();
        let mut thread_id = 1u64;
        let mut saw_stopped = false;
        while Instant::now() < deadline {
            let msg = self.read_frame(deadline)?;
            if msg.event_name() == Some("stopped") {
                saw_stopped = true;
                if let Some(body) = msg.event_body() {
                    reason = body
                        .get("reason")
                        .and_then(|v| v.as_str())
                        .unwrap_or(expected_reason_hint)
                        .to_string();
                    thread_id = body.get("threadId").and_then(|v| v.as_u64()).unwrap_or(1);
                }
                break;
            }
        }
        if !saw_stopped {
            return Err(LiveDapSessionError::Timeout {
                message: format!("waiting for stopped ({expected_reason_hint})"),
            });
        }

        let stack = self.request(
            "stackTrace",
            json!({ "threadId": thread_id, "startFrame": 0, "levels": 20 }),
            timeout,
        )?;
        let stack_frames = stack
            .get("stackFrames")
            .and_then(|f| f.as_array())
            .cloned()
            .unwrap_or_default()
            .into_iter()
            .map(|frame| LiveStackFrame {
                id: frame.get("id").and_then(|v| v.as_u64()).unwrap_or(0),
                name: frame
                    .get("name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("?")
                    .to_string(),
                path: frame
                    .get("source")
                    .and_then(|s| s.get("path"))
                    .and_then(|v| v.as_str())
                    .map(str::to_string),
                line: frame.get("line").and_then(|v| v.as_u64()).unwrap_or(0),
            })
            .collect::<Vec<_>>();

        let mut variables = Vec::new();
        if let Some(frame_id) = stack_frames.first().map(|f| f.id) {
            let scopes = self.request("scopes", json!({ "frameId": frame_id }), timeout)?;
            if let Some(scope_ref) = scopes
                .get("scopes")
                .and_then(|s| s.as_array())
                .and_then(|arr| arr.first())
                .and_then(|s| s.get("variablesReference"))
                .and_then(|v| v.as_u64())
            {
                let vars = self.request(
                    "variables",
                    json!({ "variablesReference": scope_ref }),
                    timeout,
                )?;
                variables = vars
                    .get("variables")
                    .and_then(|v| v.as_array())
                    .cloned()
                    .unwrap_or_default()
                    .into_iter()
                    .map(|var| LiveVariable {
                        name: var
                            .get("name")
                            .and_then(|v| v.as_str())
                            .unwrap_or("?")
                            .to_string(),
                        value: var
                            .get("value")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string(),
                        type_label: var.get("type").and_then(|v| v.as_str()).map(str::to_string),
                    })
                    .collect();
            }
        }

        Ok(LiveDapStopOutcome {
            lifecycle_state: DapLifecycleState::Paused,
            reason: reason.clone(),
            thread_id,
            stack_frames: stack_frames.clone(),
            variables: variables.clone(),
            metadata_summary: format!(
                "action=stopped reason={reason} thread={thread_id} frames={} vars={} live=true wire=microsoft-dap",
                stack_frames.len(),
                variables.len()
            ),
        })
    }

    fn request(
        &mut self,
        command: &str,
        arguments: Value,
        timeout: Duration,
    ) -> Result<Value, LiveDapSessionError> {
        let seq = self.alloc_seq();
        let req = DapMessage::request(seq, command, arguments);
        let stdin = self
            .stdin
            .as_mut()
            .ok_or_else(|| LiveDapSessionError::Protocol {
                message: "stdin already closed".to_string(),
            })?;
        DapFramer::write_to(stdin, &req)?;

        let deadline = Instant::now() + timeout;
        while Instant::now() < deadline {
            let msg = self.read_frame(deadline)?;
            if let Some(result) = msg.response_for(seq) {
                return result
                    .map(|body| {
                        if body.is_null() {
                            json!({})
                        } else {
                            body.clone()
                        }
                    })
                    .map_err(|message| LiveDapSessionError::Protocol {
                        message: format!("{command} error: {message}"),
                    });
            }
            // Events while waiting for this response are ignored here; callers
            // that need stopped/continued use dedicated wait helpers after.
        }
        Err(LiveDapSessionError::Timeout {
            message: format!("waiting for {command} response seq={seq}"),
        })
    }

    /// Send disconnect and wait for process exit (best-effort).
    pub fn disconnect_and_wait(mut self, timeout: Duration) -> Result<(), LiveDapSessionError> {
        let seq = self.alloc_seq();
        let req = DapMessage::request(
            seq,
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

    /// Read one frame, or give up at `deadline`.
    ///
    /// The deadline is real here, which it was not before: the old loops called
    /// a blocking `read_from` and only re-checked the clock after a frame
    /// arrived, so an adapter that said nothing was waited on forever.
    ///
    /// A closed channel means the reader thread stopped — the adapter exited or
    /// its stdout broke — and the adapter's own stderr is the most useful thing
    /// to say about that, so it is attached when there is any.
    fn read_frame(&mut self, deadline: Instant) -> Result<DapMessage, LiveDapSessionError> {
        let remaining = deadline.saturating_duration_since(Instant::now());
        match self.frames.recv_timeout(remaining) {
            Ok(Ok(message)) => Ok(message),
            Ok(Err(source)) => Err(LiveDapSessionError::Io { source }),
            Err(RecvTimeoutError::Timeout) => Err(LiveDapSessionError::Timeout {
                message: format!(
                    "no DAP frame within {:?}{}",
                    deadline.saturating_duration_since(Instant::now()),
                    self.stderr_suffix()
                ),
            }),
            Err(RecvTimeoutError::Disconnected) => Err(LiveDapSessionError::Protocol {
                message: format!("adapter stopped producing frames{}", self.stderr_suffix()),
            }),
        }
    }

    /// Whatever the adapter wrote to stderr, as a trailing clause.
    fn stderr_suffix(&self) -> String {
        let Ok(captured) = self.stderr.lock() else {
            return String::new();
        };
        let trimmed = captured.trim();
        if trimmed.is_empty() {
            return String::new();
        }
        // Bounded: an adapter that logs heavily must not turn one error into a
        // page of unrelated output.
        let shown: String = trimmed.chars().take(400).collect();
        format!("; adapter stderr: {shown}")
    }

    fn alloc_seq(&mut self) -> u64 {
        let seq = self.next_seq;
        self.next_seq = self.next_seq.saturating_add(1);
        seq
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
