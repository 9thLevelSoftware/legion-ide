//! Live DAP adapter process session (B1/B2/B4).
//!
//! Spawns an adapter binary and drives a minimal DAP product loop over the
//! **Microsoft DAP** wire (`seq`/`type`/`command`/`arguments`):
//! initialize → setBreakpoints → launch/configurationDone → stopped →
//! stackTrace/variables → step/continue → disconnect.
//!
//! CI uses the in-tree `fake_dap_adapter` binary (same wire shape as real
//! CodeLLDB / `lldb-dap`).

use std::io::{BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::{Receiver, RecvTimeoutError, sync_channel};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde_json::{Value, json};
use thiserror::Error;

use crate::framing::{DapFrameError, DapFramer, DapMessage};
use crate::state::DapLifecycleState;

/// How much adapter stderr to retain.
///
/// Bounded so a chatty adapter cannot grow the capture without limit over a
/// long session. Generous relative to `stderr_suffix`, which shows the first
/// 400 characters — the extra is headroom for reading the full capture while
/// debugging, not for display.
const STDERR_CAPTURE_BYTES: usize = 8 * 1024;

/// How long an error path waits for the stderr reader to catch up.
///
/// Long enough for a thread that has just been spawned to read a line that is
/// already in the pipe; short enough that no error is noticeably slower to
/// report.
const STDERR_SETTLE: Duration = Duration::from_millis(250);

/// Describe an adapter's liveness for an error message.
///
/// A free function so the three outcomes can be tested without standing up a
/// session around a real child process.
fn exit_clause(status: Result<Option<std::process::ExitStatus>, std::io::Error>) -> String {
    match status {
        Ok(Some(status)) => format!("; adapter exited: {status}"),
        Ok(None) => "; adapter still running".to_string(),
        Err(err) => format!("; adapter status unknown: {err}"),
    }
}

/// Format captured stderr as a trailing clause for an error message.
///
/// Waits briefly when the capture is empty. The errors that want stderr are
/// raised the instant stdout breaks — the Windows dogfood failure reaches
/// `unexpected EOF in headers` in 20ms — and the reader thread may not have
/// been scheduled yet, let alone read a line. Formatting immediately loses the
/// message to a race and reports the same bare frame error the capture was
/// added to explain. The wait is bounded and only on error paths.
///
/// When nothing arrives the clause says so rather than being omitted: "the
/// adapter said nothing" and "we failed to capture what it said" are different
/// diagnoses, and an absent clause cannot tell them apart.
fn stderr_clause(sink: &Arc<Mutex<String>>) -> String {
    let deadline = Instant::now() + STDERR_SETTLE;
    loop {
        match sink.lock() {
            Ok(captured) if !captured.trim().is_empty() => break,
            Ok(_) => {}
            Err(_) => return String::new(),
        }
        if Instant::now() >= deadline {
            break;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    let Ok(captured) = sink.lock() else {
        return String::new();
    };
    let trimmed = captured.trim();
    if trimmed.is_empty() {
        return "; adapter stderr: <empty>".to_string();
    }
    // Bounded: an adapter that logs heavily must not turn one error into a
    // page of unrelated output.
    let shown: String = trimmed.chars().take(400).collect();
    format!("; adapter stderr: {shown}")
}

/// Drain a reader into `sink` on a background thread, a line at a time.
///
/// Line at a time rather than `read_to_string`: the latter returns at EOF,
/// which is when the adapter exits — and the errors that most need stderr (a
/// timeout, a broken frame) are raised while it is still running. Draining to
/// EOF attached an empty string on exactly the path the capture exists for.
///
/// Best effort throughout: a failure to read the adapter's complaints must not
/// fail the session, only leave the error message thinner. A free function
/// rather than an inline closure so it can be tested against a reader that
/// never reaches EOF, which is the case that was wrong.
fn spawn_stderr_capture<R>(stderr: R, sink: Arc<Mutex<String>>)
where
    R: Read + Send + 'static,
{
    let _ = std::thread::Builder::new()
        .name("legion-dap-stderr".to_string())
        .spawn(move || {
            let mut reader = BufReader::new(stderr);
            let mut line = String::new();
            loop {
                line.clear();
                match reader.read_line(&mut line) {
                    Ok(0) | Err(_) => break,
                    Ok(_) => {}
                }
                let Ok(mut sink) = sink.lock() else {
                    break;
                };
                // Keep the head and stop. An adapter that logs steadily must
                // not grow this without bound, and the head is what gets
                // shown: `stderr_suffix` takes from the front, because the
                // first complaint is usually the one that explains the
                // failure.
                if sink.len() >= STDERR_CAPTURE_BYTES {
                    break;
                }
                sink.push_str(&line);
            }
        });
}

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
        if let Some(stderr) = stderr {
            spawn_stderr_capture(stderr, Arc::clone(&captured));
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
            // No explicit remaining-time check: `read_frame` enforces the
            // deadline through `recv_timeout` and returns `Timeout`, which `?`
            // propagates.
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
        let waited = deadline.saturating_duration_since(Instant::now());
        match self.frames.recv_timeout(waited) {
            Ok(Ok(message)) => Ok(message),
            // A framing error is usually the adapter having exited, which is
            // the case where its stderr is most likely to say why. Reporting
            // the bare frame error is what left "unexpected EOF in headers"
            // unexplained for a day.
            Ok(Err(source)) => Err(LiveDapSessionError::Protocol {
                message: format!("{source}{}{}", self.exit_suffix(), self.stderr_suffix()),
            }),
            // `waited`, not the remaining time: by the time this arm runs the
            // remainder is zero by definition, which is how the first version
            // of this message reported every timeout as "within 0ns".
            Err(RecvTimeoutError::Timeout) => Err(LiveDapSessionError::Timeout {
                message: format!(
                    "no DAP frame within {waited:?}{}{}",
                    self.exit_suffix(),
                    self.stderr_suffix()
                ),
            }),
            Err(RecvTimeoutError::Disconnected) => Err(LiveDapSessionError::Protocol {
                message: format!(
                    "adapter stopped producing frames{}{}",
                    self.exit_suffix(),
                    self.stderr_suffix()
                ),
            }),
        }
    }

    /// Whether the adapter process is still alive, as a trailing clause.
    ///
    /// The Windows dogfood failure reads `unexpected EOF in headers; adapter
    /// stderr: <empty>`: the adapter closes stdout at once and says nothing at
    /// all. Stderr cannot explain a process that never wrote to it, and the
    /// next question — did it die, and how — is answered by its exit status.
    ///
    /// Worth the line on Windows in particular, where a missing DLL kills a
    /// process silently and shows up only as an NTSTATUS in the exit code
    /// (`0xc0000135` is STATUS_DLL_NOT_FOUND). A silent death and a live
    /// adapter that simply is not speaking are different faults, and the bare
    /// frame error looks identical for both.
    fn exit_suffix(&mut self) -> String {
        exit_clause(self.child.try_wait())
    }

    /// Whatever the adapter wrote to stderr, as a trailing clause.
    fn stderr_suffix(&self) -> String {
        stderr_clause(&self.stderr)
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

#[cfg(test)]
mod stderr_capture_tests {
    use super::*;
    use std::process::Command;
    use std::sync::mpsc::{Sender, channel};

    /// A reader that yields some bytes and then blocks instead of ending.
    ///
    /// This is the adapter that is still running: it has complained, and it has
    /// not exited. `read_to_string` on this never returns, which is why the
    /// previous implementation had nothing to attach at the moment a timeout
    /// or a framing error was raised.
    struct WroteThenHung {
        data: Vec<u8>,
        position: usize,
        release: Receiver<()>,
    }

    impl Read for WroteThenHung {
        fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
            if self.position < self.data.len() {
                let take = (self.data.len() - self.position).min(buf.len());
                buf[..take].copy_from_slice(&self.data[self.position..self.position + take]);
                self.position += take;
                return Ok(take);
            }
            // Not EOF: the process is alive and simply has nothing more to say
            // yet. Blocks until the test lets go.
            let _ = self.release.recv();
            Ok(0)
        }
    }

    fn hung_reader(text: &str) -> (WroteThenHung, Sender<()>) {
        let (release_tx, release) = channel();
        (
            WroteThenHung {
                data: text.as_bytes().to_vec(),
                position: 0,
                release,
            },
            release_tx,
        )
    }

    /// Wait for `predicate`, so the test does not depend on thread scheduling.
    fn within(timeout: Duration, mut predicate: impl FnMut() -> bool) -> bool {
        let deadline = Instant::now() + timeout;
        while Instant::now() < deadline {
            if predicate() {
                return true;
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        predicate()
    }

    #[test]
    fn stderr_is_captured_before_the_adapter_exits() {
        // The defect this replaced: `read_to_string` returns at EOF, so the
        // capture was empty for as long as the adapter was alive — which is
        // every moment at which the error message wanted it.
        let (reader, _release) = hung_reader("dyld: library not loaded\n");
        let sink = Arc::new(Mutex::new(String::new()));
        spawn_stderr_capture(reader, Arc::clone(&sink));

        let captured = within(Duration::from_secs(5), || {
            !sink.lock().expect("sink").is_empty()
        });
        assert!(
            captured,
            "stderr must be readable while the adapter is still running; got {:?}",
            sink.lock().expect("sink")
        );
        assert!(
            sink.lock().expect("sink").contains("library not loaded"),
            "the captured text must be what was written"
        );
        // `_release` is still held: the reader has not seen EOF, and the
        // assertions above already passed.
    }

    #[test]
    fn a_clause_waits_for_stderr_that_has_not_arrived_yet() {
        // The Windows dogfood failure reaches `unexpected EOF in headers` in
        // 20ms. Formatting the error immediately raced the reader thread and
        // printed the bare frame error — the exact message the capture was
        // added to explain.
        let sink = Arc::new(Mutex::new(String::new()));
        let writer = Arc::clone(&sink);
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(60));
            writer.lock().expect("sink").push_str(
                "error: unable to find executable
",
            );
        });

        let clause = stderr_clause(&sink);
        assert!(
            clause.contains("unable to find executable"),
            "the clause must wait for stderr that is still in flight; got {clause:?}"
        );
    }

    #[test]
    fn an_empty_capture_says_so_rather_than_vanishing() {
        // "The adapter said nothing" and "we failed to capture what it said"
        // are different diagnoses. An omitted clause cannot tell them apart,
        // and that ambiguity is what made the first Windows failure unreadable.
        let sink = Arc::new(Mutex::new(String::new()));
        let clause = stderr_clause(&sink);
        assert_eq!(clause, "; adapter stderr: <empty>");
    }

    #[test]
    fn the_wait_is_bounded() {
        let sink = Arc::new(Mutex::new(String::new()));
        let started = Instant::now();
        let _ = stderr_clause(&sink);
        let waited = started.elapsed();
        assert!(
            waited < STDERR_SETTLE * 4,
            "an error path must not stall on a silent adapter; waited {waited:?}"
        );
    }

    /// Run a trivial command that exits with `code`, and return its status.
    fn exited_with(code: i32) -> std::process::ExitStatus {
        let mut command = if cfg!(windows) {
            let mut command = Command::new("cmd");
            command.args(["/c", &format!("exit {code}")]);
            command
        } else {
            let mut command = Command::new("sh");
            command.args(["-c", &format!("exit {code}")]);
            command
        };
        command.spawn().expect("spawn").wait().expect("wait")
    }

    #[test]
    fn a_dead_adapter_reports_how_it_died() {
        // The Windows failure says `adapter stderr: <empty>`: the process wrote
        // nothing at all, so the exit status is the only thing left that can
        // explain it. On Windows a missing DLL is a silent death visible only
        // as an NTSTATUS in this code.
        let clause = exit_clause(Ok(Some(exited_with(3))));
        assert!(
            clause.contains("adapter exited"),
            "a dead adapter must be reported as dead: {clause:?}"
        );
        assert!(
            clause.contains('3'),
            "the exit code is the diagnostic; it must survive into the message: {clause:?}"
        );
    }

    #[test]
    fn a_live_adapter_is_distinguished_from_a_dead_one() {
        // Same bare frame error, two different faults: a process that died and
        // one that is alive and simply not speaking DAP.
        assert_eq!(exit_clause(Ok(None)), "; adapter still running");
    }

    #[test]
    fn capture_stops_at_the_byte_cap() {
        // An adapter that logs steadily must not grow the capture without
        // bound over a long session.
        let line = "x".repeat(255);
        let mut noisy = String::new();
        while noisy.len() < STDERR_CAPTURE_BYTES * 2 {
            noisy.push_str(&line);
            noisy.push('\n');
        }
        let (reader, _release) = hung_reader(&noisy);
        let sink = Arc::new(Mutex::new(String::new()));
        spawn_stderr_capture(reader, Arc::clone(&sink));

        let settled = within(Duration::from_secs(5), || {
            sink.lock().expect("sink").len() >= STDERR_CAPTURE_BYTES
        });
        assert!(settled, "the capture should reach the cap and stop");
        let len = sink.lock().expect("sink").len();
        assert!(
            len < STDERR_CAPTURE_BYTES * 2,
            "the capture must stop near the cap, not drain everything: {len}"
        );
    }
}
