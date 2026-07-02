//! Deterministic Phase 8 local terminal fixture runtime.

#![warn(missing_docs)]

/// Windows ConPTY parity metadata contracts.
pub mod conpty;
/// Renderer-friendly terminal grid projection helpers.
pub mod grid;
/// OSC 7/133 shell metadata parsing.
pub mod osc;
/// Per-session terminal metadata tracking.
pub mod session;

use std::{
    collections::{HashMap, HashSet},
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, Ordering},
    },
    time::{Duration, Instant},
};

use legion_platform::{PtyKillMode, PtyRequest, PtyService};
use legion_protocol::{
    CanonicalPath, CausalityId, CorrelationId, DebugAdapterAuditRecord, DebugAdapterLaunchRequest,
    DebugBreakpointRecord, DebugConsoleCategory, DebugConsoleEntry, DebugInlineValue,
    DebugSessionId, DebugSessionState, DebugStackFrame, DebugStepKind, DebugVariable,
    DebugWatchExpression, EventSequence, ProtocolTextRange, RedactionHint, TerminalAuditRecord,
    TerminalCloseRequest, TerminalInput, TerminalKillEscalation, TerminalKillRequest,
    TerminalLaunchPolicyContract, TerminalOutputChunk, TerminalResize, TerminalRuntimeState,
    TerminalSessionId, TextCoordinate, WorkspaceTrustState, validate_debug_adapter_audit_record,
    validate_terminal_audit_record, validate_terminal_close_request, validate_terminal_input,
    validate_terminal_kill_request, validate_terminal_launch_policy_contract,
    validate_terminal_output_chunk, validate_terminal_resize,
};
use thiserror::Error;

static TERMINAL_SESSION_COUNTER: AtomicU64 = AtomicU64::new(10_000);

/// Terminal fixture error.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum TerminalFixtureError {
    /// Fixture runtime is disabled.
    #[error("terminal fixture is disabled")]
    Disabled,
    /// Launch policy rejected the request.
    #[error("terminal launch denied: {reason}")]
    Denied {
        /// Denial reason.
        reason: String,
    },
    /// Output metadata exceeded configured bounds.
    #[error("terminal output exceeded bounds: {reason}")]
    LimitExceeded {
        /// Limit reason.
        reason: String,
    },
}

/// Terminal runtime error.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum TerminalRuntimeError {
    /// Runtime is disabled.
    #[error("terminal runtime is disabled")]
    Disabled,
    /// Launch was denied by policy or metadata validation.
    #[error("terminal runtime denied: {reason}")]
    Denied {
        /// Denial reason.
        reason: String,
    },
    /// Backend failed.
    #[error("terminal backend failed: {reason}")]
    Backend {
        /// Backend failure reason.
        reason: String,
    },
}

/// Debug adapter fixture error.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum DapAdapterFixtureError {
    /// Fixture runtime is disabled.
    #[error("DAP adapter fixture is disabled")]
    Disabled,
    /// Launch request was invalid.
    #[error("DAP adapter fixture denied launch: {reason}")]
    Denied {
        /// Display-safe denial reason.
        reason: String,
    },
}

/// Deterministic DAP adapter fixture configuration.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DapAdapterFixtureConfig {
    /// Whether fixture behavior is enabled.
    pub enabled: bool,
}

impl DapAdapterFixtureConfig {
    /// Return an enabled fixture configuration.
    pub fn enabled() -> Self {
        Self { enabled: true }
    }
}

/// Deterministic DAP launch/step projection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DapAdapterFixtureOutcome {
    /// Metadata-only adapter audit.
    pub audit: DebugAdapterAuditRecord,
    /// Verified breakpoint metadata.
    pub breakpoints: Vec<DebugBreakpointRecord>,
    /// Projected stack frames.
    pub stack_frames: Vec<DebugStackFrame>,
    /// Projected variables.
    pub variables: Vec<DebugVariable>,
    /// Projected watch expressions.
    pub watches: Vec<DebugWatchExpression>,
    /// Projected inline values.
    pub inline_values: Vec<DebugInlineValue>,
    /// Projected debug console entries.
    pub console: Vec<DebugConsoleEntry>,
}

/// Deterministic metadata-only DAP adapter fixture runtime.
#[derive(Debug, Clone)]
pub struct DapAdapterFixtureRuntime {
    config: DapAdapterFixtureConfig,
    session_adapter_types: Arc<Mutex<HashMap<DebugSessionId, String>>>,
}

impl DapAdapterFixtureRuntime {
    /// Construct a runtime from configuration.
    pub fn new(config: DapAdapterFixtureConfig) -> Self {
        Self {
            config,
            session_adapter_types: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Launch a deterministic paused debug session.
    pub fn launch(
        &self,
        request: DebugAdapterLaunchRequest,
    ) -> Result<DapAdapterFixtureOutcome, DapAdapterFixtureError> {
        if !self.config.enabled {
            return Err(DapAdapterFixtureError::Disabled);
        }
        if request.workspace_id.0 == 0
            || request.configuration_id.0.trim().is_empty()
            || request.adapter_type.trim().is_empty()
            || request.schema_version == 0
        {
            return Err(DapAdapterFixtureError::Denied {
                reason: "debug launch request is incomplete".to_string(),
            });
        }
        let session_id = DebugSessionId(format!(
            "debug:{}:{}",
            request.workspace_id.0, request.configuration_id.0
        ));
        self.session_adapter_types
            .lock()
            .expect("DAP adapter fixture session map should be lockable")
            .insert(session_id.clone(), request.adapter_type.clone());
        let sequence = EventSequence(1);
        let audit = DebugAdapterAuditRecord {
            session_id: session_id.clone(),
            state: DebugSessionState::Paused,
            adapter_type: request.adapter_type.clone(),
            event_sequence: sequence,
            correlation_id: CorrelationId(request.workspace_id.0 as u64),
            causality_id: CausalityId(uuid_from_value(request.workspace_id.0 as u64)),
            metadata_summary: format!(
                "action=launch state=paused adapter={} breakpoints={}",
                request.adapter_type,
                request.breakpoints.len()
            ),
            redaction_hints: vec![RedactionHint::MetadataOnly],
            schema_version: 1,
        };
        validate_debug_adapter_audit_record(&audit).map_err(|err| {
            DapAdapterFixtureError::Denied {
                reason: err.message,
            }
        })?;
        let breakpoints = request
            .breakpoints
            .into_iter()
            .map(|mut breakpoint| {
                breakpoint.session_id = Some(session_id.clone());
                breakpoint.verified = true;
                breakpoint.message = Some("verified by deterministic DAP fixture".to_string());
                breakpoint
            })
            .collect::<Vec<_>>();
        let first_path = breakpoints
            .first()
            .map(|breakpoint| breakpoint.path.clone())
            .unwrap_or_else(|| CanonicalPath("workspace://debug-entry".to_string()));
        let first_range = breakpoints
            .first()
            .map(|breakpoint| breakpoint.range)
            .unwrap_or_else(debug_zero_range);
        Ok(DapAdapterFixtureOutcome {
            audit: audit.clone(),
            breakpoints,
            stack_frames: vec![DebugStackFrame {
                session_id: session_id.clone(),
                frame_id: 1,
                name: "main".to_string(),
                path: Some(first_path.clone()),
                range: Some(first_range),
                schema_version: 1,
            }],
            variables: vec![DebugVariable {
                session_id: session_id.clone(),
                variables_reference: 1,
                name: "count".to_string(),
                value_label: "metadata-only".to_string(),
                type_label: Some("i32".to_string()),
                has_children: false,
                redaction_hints: vec![RedactionHint::MetadataOnly],
                schema_version: 1,
            }],
            watches: Vec::new(),
            inline_values: vec![DebugInlineValue {
                session_id: session_id.clone(),
                path: first_path,
                range: first_range,
                expression_label: "count".to_string(),
                value_label: "metadata-only".to_string(),
                redaction_hints: vec![RedactionHint::MetadataOnly],
                schema_version: 1,
            }],
            console: vec![DebugConsoleEntry {
                session_id,
                category: DebugConsoleCategory::Adapter,
                message_label: format!(
                    "launch adapter={} configuration={}",
                    request.adapter_type, request.configuration_id.0
                ),
                sequence,
                redaction_hints: vec![RedactionHint::MetadataOnly],
                schema_version: 1,
            }],
        })
    }

    /// Project a deterministic paused step outcome.
    pub fn step(
        &self,
        session_id: DebugSessionId,
        kind: DebugStepKind,
    ) -> Result<DapAdapterFixtureOutcome, DapAdapterFixtureError> {
        if !self.config.enabled {
            return Err(DapAdapterFixtureError::Disabled);
        }
        if session_id.0.trim().is_empty() {
            return Err(DapAdapterFixtureError::Denied {
                reason: "debug session id is required".to_string(),
            });
        }
        let label = match kind {
            DebugStepKind::Continue => "continue",
            DebugStepKind::Over => "over",
            DebugStepKind::Into => "into",
            DebugStepKind::Out => "out",
            DebugStepKind::Back => "back",
        };
        let adapter_type = self
            .session_adapter_types
            .lock()
            .expect("DAP adapter fixture session map should be lockable")
            .get(&session_id)
            .cloned()
            .unwrap_or_else(|| "lldb-dap".to_string());
        let audit = DebugAdapterAuditRecord {
            session_id: session_id.clone(),
            state: DebugSessionState::Paused,
            adapter_type: adapter_type.clone(),
            event_sequence: EventSequence(2),
            correlation_id: CorrelationId(2),
            causality_id: CausalityId(uuid_from_value(2)),
            metadata_summary: format!("action=step state=paused step={label}"),
            redaction_hints: vec![RedactionHint::MetadataOnly],
            schema_version: 1,
        };
        validate_debug_adapter_audit_record(&audit).map_err(|err| {
            DapAdapterFixtureError::Denied {
                reason: err.message,
            }
        })?;
        Ok(DapAdapterFixtureOutcome {
            audit,
            breakpoints: Vec::new(),
            stack_frames: vec![DebugStackFrame {
                session_id: session_id.clone(),
                frame_id: 1,
                name: "main".to_string(),
                path: None,
                range: None,
                schema_version: 1,
            }],
            variables: vec![DebugVariable {
                session_id: session_id.clone(),
                variables_reference: 1,
                name: "count".to_string(),
                value_label: "metadata-only".to_string(),
                type_label: Some("i32".to_string()),
                has_children: false,
                redaction_hints: vec![RedactionHint::MetadataOnly],
                schema_version: 1,
            }],
            watches: Vec::new(),
            inline_values: Vec::new(),
            console: vec![DebugConsoleEntry {
                session_id,
                category: DebugConsoleCategory::Adapter,
                message_label: format!("step={label} state=paused adapter={adapter_type}"),
                sequence: EventSequence(2),
                redaction_hints: vec![RedactionHint::MetadataOnly],
                schema_version: 1,
            }],
        })
    }
}

/// Production terminal runtime configuration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalRuntimeConfig {
    /// Whether terminal runtime is enabled.
    pub enabled: bool,
    /// Maximum projected output bytes.
    pub max_output_bytes: u64,
}

impl TerminalRuntimeConfig {
    /// Return an enabled terminal runtime configuration.
    pub fn enabled() -> Self {
        Self {
            enabled: true,
            ..Self::default()
        }
    }
}

impl Default for TerminalRuntimeConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            max_output_bytes: 256 * 1024,
        }
    }
}

/// Terminal launch request for the process-backed degraded runtime.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalRuntimeLaunchRequest {
    /// Launch policy contract.
    pub policy: TerminalLaunchPolicyContract,
    /// Command executable selected by policy-owned caller.
    pub command: String,
    /// Command arguments selected by policy-owned caller.
    pub args: Vec<String>,
}

/// Terminal launch outcome.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalRuntimeLaunchOutcome {
    /// Metadata-only audit record.
    pub audit: TerminalAuditRecord,
    /// Bounded redacted projection chunk.
    pub output: TerminalOutputChunk,
    /// OSC 7/133 shell metadata parsed from this launch chunk.
    pub shell_projection: crate::osc::TerminalShellProjection,
}

/// Terminal output poll request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalRuntimeOutputPollRequest {
    /// Terminal session identifier returned by launch.
    pub session_id: TerminalSessionId,
    /// Event sequence for the poll.
    pub event_sequence: EventSequence,
    /// Correlation identifier for the poll.
    pub correlation_id: CorrelationId,
    /// Causality identifier for the poll.
    pub causality_id: CausalityId,
}

/// Terminal output poll outcome.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalRuntimeOutputPollOutcome {
    /// Metadata-only audit record.
    pub audit: TerminalAuditRecord,
    /// Bounded redacted projection chunk.
    pub output: TerminalOutputChunk,
    /// OSC 7/133 shell metadata parsed from this poll chunk.
    pub shell_projection: crate::osc::TerminalShellProjection,
}

/// Terminal fixture configuration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalFixtureConfig {
    /// Whether deterministic terminal fixture behavior is enabled.
    pub enabled: bool,
    /// Maximum output bytes accepted by the fixture.
    pub max_output_bytes: u64,
}

impl TerminalFixtureConfig {
    /// Return an enabled deterministic fixture configuration.
    pub fn enabled() -> Self {
        Self {
            enabled: true,
            ..Self::default()
        }
    }
}

impl Default for TerminalFixtureConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            max_output_bytes: 256 * 1024,
        }
    }
}

/// Deterministic terminal fixture runtime.
#[derive(Debug, Clone)]
pub struct TerminalFixtureRuntime {
    config: TerminalFixtureConfig,
}

impl TerminalFixtureRuntime {
    /// Construct a runtime from configuration.
    pub fn new(config: TerminalFixtureConfig) -> Self {
        Self { config }
    }

    /// Launch a metadata-only deterministic fixture session.
    pub fn launch(
        &self,
        policy: TerminalLaunchPolicyContract,
    ) -> Result<TerminalAuditRecord, TerminalFixtureError> {
        if !self.config.enabled {
            return Err(TerminalFixtureError::Disabled);
        }
        if policy.schema_version == 0
            || policy.principal_id.0.trim().is_empty()
            || policy.workspace_id.0 == 0
            || policy.trust_state != WorkspaceTrustState::Trusted
            || policy.capability_id.0.trim().is_empty()
            || policy.output_byte_limit == 0
            || policy.output_byte_limit > self.config.max_output_bytes
            || policy.timeout_seconds == 0
        {
            return Err(TerminalFixtureError::Denied {
                reason: "terminal launch policy is incomplete or outside bounds".to_string(),
            });
        }
        let record = TerminalAuditRecord {
            session_id: legion_protocol::TerminalSessionId(policy.workspace_id.0 as u64),
            state: TerminalRuntimeState::Running,
            event_sequence: legion_protocol::EventSequence(policy.workspace_id.0 as u64),
            correlation_id: legion_protocol::CorrelationId(policy.workspace_id.0 as u64),
            causality_id: legion_protocol::CausalityId(uuid_from_value(
                policy.workspace_id.0 as u64,
            )),
            metadata_summary: format!(
                "state=running cwd_policy={} output_limit={} timeout_seconds={}",
                policy.cwd_policy, policy.output_byte_limit, policy.timeout_seconds
            ),
            redaction_hints: vec![RedactionHint::MetadataOnly],
            schema_version: 1,
        };
        validate_terminal_audit_record(&record).map_err(|err| TerminalFixtureError::Denied {
            reason: err.message,
        })?;
        Ok(record)
    }

    /// Build a bounded redacted output chunk for projection-only consumers.
    pub fn output_chunk(
        &self,
        record: &TerminalAuditRecord,
        redacted_payload: impl Into<String>,
        byte_count: u64,
    ) -> Result<TerminalOutputChunk, TerminalFixtureError> {
        if !self.config.enabled {
            return Err(TerminalFixtureError::Disabled);
        }
        let redacted_payload = redacted_payload.into();
        if byte_count > self.config.max_output_bytes {
            return Err(TerminalFixtureError::LimitExceeded {
                reason: "terminal output exceeds configured byte limit".to_string(),
            });
        }
        let output = TerminalOutputChunk {
            session_id: record.session_id,
            sequence: record.event_sequence,
            redacted_payload,
            byte_count,
            truncated: byte_count == self.config.max_output_bytes,
            redaction: RedactionHint::MetadataOnly,
            schema_version: 1,
        };
        validate_terminal_output_chunk(&output).map_err(|err| TerminalFixtureError::Denied {
            reason: err.message,
        })?;
        Ok(output)
    }

    /// Produce a deterministic exit audit record.
    pub fn exit(&self, record: &TerminalAuditRecord) -> TerminalAuditRecord {
        TerminalAuditRecord {
            state: TerminalRuntimeState::Exited,
            metadata_summary: "state=exited exit_code=0".to_string(),
            ..record.clone()
        }
    }
}

/// Process-backed terminal runtime using the platform PTY boundary.
pub struct TerminalRuntime<P> {
    config: TerminalRuntimeConfig,
    pty: P,
    sessions: Mutex<HashMap<TerminalSessionId, RuntimeSession>>,
}

#[derive(Debug, Clone)]
struct RuntimeSession {
    platform_session_id: String,
    next_sequence: u64,
    correlation_id: CorrelationId,
    causality_id: CausalityId,
    /// Effective per-session output byte limit (already clamped to the runtime
    /// configuration maximum at launch).
    output_byte_limit: u64,
    /// Wall-clock deadline derived from the launch policy `timeout_seconds`.
    /// `None` only when the configured timeout would overflow the clock.
    deadline: Option<Instant>,
    /// Set under the registry lock before an irreversible close/kill backend
    /// call so concurrent lifecycle operations fail closed instead of acting on
    /// a session that is being torn down.
    closing: bool,
    /// Latest advisory OSC 7/133 metadata observed for this session.
    metadata: session::TerminalSessionMetadata,
}

/// Session-owned identity and limits resolved under the registry lock for a
/// single lifecycle operation.
struct LifecycleContext {
    platform_session_id: String,
    sequence: EventSequence,
    correlation_id: CorrelationId,
    causality_id: CausalityId,
    output_byte_limit: u64,
    deadline: Option<Instant>,
}

impl<P: PtyService> TerminalRuntime<P> {
    /// Construct a terminal runtime from configuration and platform PTY service.
    pub fn new(config: TerminalRuntimeConfig, pty: P) -> Self {
        Self {
            config,
            pty,
            sessions: Mutex::new(HashMap::new()),
        }
    }

    /// Return the latest advisory OSC 7/133 metadata for a live session.
    ///
    /// Shell-emitted cwd/exit-code metadata is projection metadata only and must
    /// not be used as security policy authority.
    pub fn session_metadata(
        &self,
        session_id: TerminalSessionId,
    ) -> Option<session::TerminalSessionMetadata> {
        self.sessions.lock().ok().and_then(|sessions| {
            sessions
                .get(&session_id)
                .map(|session| session.metadata.clone())
        })
    }

    /// Launch a terminal command and return metadata-only audit/projection records.
    pub fn launch(
        &self,
        request: TerminalRuntimeLaunchRequest,
    ) -> Result<TerminalRuntimeLaunchOutcome, TerminalRuntimeError> {
        if !self.config.enabled {
            return Err(TerminalRuntimeError::Disabled);
        }
        validate_launch_policy(&request.policy, self.config.max_output_bytes)?;
        if request.command.trim().is_empty() {
            return Err(TerminalRuntimeError::Denied {
                reason: "terminal command metadata is required".to_string(),
            });
        }
        // Per-launch policy bounds are clamped to the runtime configuration and
        // then actually enforced (Finding 1/2): the smaller of the policy/config
        // output limit is applied to projections, and a deadline is derived from
        // the policy timeout so it is enforced during polling.
        //
        // NOTE: `policy.cwd_policy` is a descriptive label only. This boundary
        // receives no concrete working-directory path (the launch request and
        // the policy contract carry no path), so the runtime cannot enforce a
        // cwd here. Forwarding an actual directory requires a path field added by
        // the policy-owning caller in `legion-protocol`/`legion-app`, which is
        // outside this crate. The PTY is therefore spawned in the platform
        // default cwd; this is documented to avoid mistaking validation for
        // enforcement.
        let effective_limit = request
            .policy
            .output_byte_limit
            .min(self.config.max_output_bytes);
        let deadline =
            Instant::now().checked_add(Duration::from_secs(request.policy.timeout_seconds));
        let session = self
            .pty
            .spawn_pty(&PtyRequest {
                command: request.command,
                args: request.args,
                cwd: None,
            })
            .map_err(|err| TerminalRuntimeError::Backend {
                reason: err.to_string(),
            })?;
        let shell_projection = crate::osc::parse_terminal_shell_output(&session.output);
        let redacted =
            redact_terminal_projection(&shell_projection.visible_output, effective_limit);
        let byte_count = session.output.len().min(effective_limit as usize) as u64;
        let terminal_session_id = next_terminal_session_id();
        let native_pty = session.id.starts_with("native-");
        let backend = if native_pty {
            session.id.split('-').take(2).collect::<Vec<_>>().join("-")
        } else {
            "process_lifecycle".to_string()
        };
        let event_sequence = EventSequence(terminal_session_id.0);
        let correlation_id = CorrelationId(terminal_session_id.0);
        let causality_id = CausalityId(uuid_from_value(terminal_session_id.0));
        let mut metadata = session::TerminalSessionMetadata::default();
        metadata.apply_shell_projection(&shell_projection);
        if native_pty {
            self.sessions
                .lock()
                .map_err(|_| TerminalRuntimeError::Backend {
                    reason: "terminal runtime session registry is unavailable".to_string(),
                })?
                .insert(
                    terminal_session_id,
                    RuntimeSession {
                        platform_session_id: session.id.clone(),
                        next_sequence: event_sequence.0.saturating_add(1),
                        correlation_id,
                        causality_id,
                        output_byte_limit: effective_limit,
                        deadline,
                        closing: false,
                        metadata,
                    },
                );
        }
        let audit = TerminalAuditRecord {
            session_id: terminal_session_id,
            state: if native_pty {
                TerminalRuntimeState::Running
            } else {
                TerminalRuntimeState::Degraded
            },
            event_sequence,
            correlation_id,
            causality_id,
            metadata_summary: format!(
                "state={} backend={} pty={} output_bytes={} truncated={}",
                if native_pty { "running" } else { "degraded" },
                backend,
                native_pty,
                byte_count,
                session.output.len() as u64 > effective_limit
            ),
            redaction_hints: vec![RedactionHint::MetadataOnly],
            schema_version: 1,
        };
        validate_terminal_audit_record(&audit).map_err(|err| TerminalRuntimeError::Denied {
            reason: err.message,
        })?;
        let output = TerminalRuntimeLaunchOutcome {
            output: {
                let output = TerminalOutputChunk {
                    session_id: audit.session_id,
                    sequence: audit.event_sequence,
                    redacted_payload: redacted,
                    byte_count,
                    truncated: session.output.len() as u64 > effective_limit,
                    redaction: RedactionHint::MetadataOnly,
                    schema_version: 1,
                };
                validate_terminal_output_chunk(&output).map_err(|err| {
                    TerminalRuntimeError::Denied {
                        reason: err.message,
                    }
                })?;
                output
            },
            audit,
            shell_projection,
        };
        Ok(output)
    }

    /// Write bounded input into a running terminal session.
    pub fn input(&self, input: TerminalInput) -> Result<TerminalAuditRecord, TerminalRuntimeError> {
        self.ensure_enabled()?;
        validate_terminal_input(&input).map_err(|err| TerminalRuntimeError::Denied {
            reason: err.message,
        })?;
        let ctx = self.session_for_lifecycle(input.session_id, false)?;
        self.pty
            .write_pty(&ctx.platform_session_id, &input.payload)
            .map_err(|err| TerminalRuntimeError::Backend {
                reason: err.to_string(),
            })?;
        self.audit_record(
            input.session_id,
            TerminalRuntimeState::Running,
            ctx.sequence,
            input.correlation_id,
            ctx.causality_id,
            format!(
                "state=running action=input pty=true input_bytes={} correlation={}",
                input.payload.len(),
                input.correlation_id.0
            ),
        )
    }

    /// Resize a running terminal session.
    pub fn resize(
        &self,
        resize: TerminalResize,
    ) -> Result<TerminalAuditRecord, TerminalRuntimeError> {
        self.ensure_enabled()?;
        validate_terminal_resize(&resize).map_err(|err| TerminalRuntimeError::Denied {
            reason: err.message,
        })?;
        let ctx = self.session_for_lifecycle(resize.session_id, false)?;
        self.pty
            .resize_pty(&ctx.platform_session_id, resize.cols, resize.rows)
            .map_err(|err| TerminalRuntimeError::Backend {
                reason: err.to_string(),
            })?;
        self.audit_record(
            resize.session_id,
            TerminalRuntimeState::Running,
            ctx.sequence,
            ctx.correlation_id,
            ctx.causality_id,
            format!(
                "state=running action=resize pty=true cols={} rows={}",
                resize.cols, resize.rows
            ),
        )
    }

    /// Poll bounded output from a running terminal session.
    pub fn poll_output(
        &self,
        request: TerminalRuntimeOutputPollRequest,
    ) -> Result<TerminalRuntimeOutputPollOutcome, TerminalRuntimeError> {
        self.ensure_enabled()?;
        if request.session_id.0 == 0
            || request.event_sequence.0 == 0
            || request.correlation_id.0 == 0
            || request.causality_id.0.is_nil()
        {
            return Err(TerminalRuntimeError::Denied {
                reason: "terminal output poll requires valid event identity".to_string(),
            });
        }
        // Caller-supplied identity is only validated as request metadata; the
        // authoritative audit identity comes from the session registry
        // (Finding 5).
        let ctx = self.session_for_lifecycle(request.session_id, false)?;
        // Enforce the launch deadline fail-closed (Finding 1): an expired
        // session is terminated and removed before any further output flows.
        if let Some(deadline) = ctx.deadline
            && Instant::now() >= deadline
        {
            let _ = self
                .pty
                .kill_pty(&ctx.platform_session_id, PtyKillMode::Terminate);
            self.remove_session(request.session_id)?;
            let audit = self.audit_record(
                request.session_id,
                TerminalRuntimeState::Exited,
                ctx.sequence,
                ctx.correlation_id,
                ctx.causality_id,
                "state=exited action=output pty=true reason=timeout".to_string(),
            )?;
            let output = TerminalOutputChunk {
                session_id: audit.session_id,
                sequence: audit.event_sequence,
                redacted_payload: String::new(),
                byte_count: 0,
                truncated: false,
                redaction: RedactionHint::MetadataOnly,
                schema_version: 1,
            };
            validate_terminal_output_chunk(&output).map_err(|err| {
                TerminalRuntimeError::Denied {
                    reason: err.message,
                }
            })?;
            return Ok(TerminalRuntimeOutputPollOutcome {
                output,
                audit,
                shell_projection: crate::osc::TerminalShellProjection::default(),
            });
        }
        let effective_limit = ctx.output_byte_limit.min(self.config.max_output_bytes);
        let read = self
            .pty
            .read_pty(&ctx.platform_session_id, effective_limit as usize)
            .map_err(|err| TerminalRuntimeError::Backend {
                reason: err.to_string(),
            })?;
        let shell_projection = crate::osc::parse_terminal_shell_output(&read.output);
        self.apply_session_metadata(request.session_id, &shell_projection)?;
        if read.exited && !read.truncated {
            self.remove_session(request.session_id)?;
        }
        let redacted =
            redact_terminal_projection(&shell_projection.visible_output, effective_limit);
        let byte_count = read.output.len().min(effective_limit as usize) as u64;
        let state = if read.exited {
            TerminalRuntimeState::Exited
        } else {
            TerminalRuntimeState::Running
        };
        let audit = self.audit_record(
            request.session_id,
            state,
            ctx.sequence,
            ctx.correlation_id,
            ctx.causality_id,
            format!(
                "state={} action=output pty=true output_bytes={} truncated={} exit_code={:?}",
                if read.exited { "exited" } else { "running" },
                byte_count,
                read.truncated,
                read.exit_code
            ),
        )?;
        let output = TerminalRuntimeOutputPollOutcome {
            output: {
                let output = TerminalOutputChunk {
                    session_id: audit.session_id,
                    sequence: audit.event_sequence,
                    redacted_payload: redacted,
                    byte_count,
                    truncated: read.truncated || read.output.len() as u64 > effective_limit,
                    redaction: RedactionHint::MetadataOnly,
                    schema_version: 1,
                };
                validate_terminal_output_chunk(&output).map_err(|err| {
                    TerminalRuntimeError::Denied {
                        reason: err.message,
                    }
                })?;
                output
            },
            audit,
            shell_projection,
        };
        Ok(output)
    }

    /// Close a running terminal session.
    pub fn close(
        &self,
        request: TerminalCloseRequest,
    ) -> Result<TerminalAuditRecord, TerminalRuntimeError> {
        self.ensure_enabled()?;
        validate_terminal_close_request(&request).map_err(|err| TerminalRuntimeError::Denied {
            reason: err.message,
        })?;
        // Mark the session closing under the registry lock before the
        // irreversible backend call so a concurrent input/resize/poll fails
        // closed (Finding 6); roll back the flag if the backend call fails so
        // the session remains usable.
        let ctx = self.session_for_lifecycle(request.session_id, true)?;
        if let Err(err) = self.pty.close_pty(&ctx.platform_session_id) {
            self.clear_closing(request.session_id);
            return Err(TerminalRuntimeError::Backend {
                reason: err.to_string(),
            });
        }
        let _ = self.remove_session(request.session_id)?;
        self.audit_record(
            request.session_id,
            TerminalRuntimeState::Exited,
            ctx.sequence,
            ctx.correlation_id,
            ctx.causality_id,
            "state=exited action=close pty=true".to_string(),
        )
    }

    /// Kill a running terminal session.
    pub fn kill(
        &self,
        request: TerminalKillRequest,
    ) -> Result<TerminalAuditRecord, TerminalRuntimeError> {
        self.ensure_enabled()?;
        validate_terminal_kill_request(&request).map_err(|err| TerminalRuntimeError::Denied {
            reason: err.message,
        })?;
        let mode = match request.escalation {
            TerminalKillEscalation::Interrupt => PtyKillMode::Interrupt,
            TerminalKillEscalation::Terminate => PtyKillMode::Terminate,
            TerminalKillEscalation::KillTree => PtyKillMode::KillTree,
        };
        // Interrupt keeps the session alive; terminate/kill-tree remove it, so
        // only those escalations mark the session closing (Finding 6).
        let terminal = request.escalation != TerminalKillEscalation::Interrupt;
        let ctx = self.session_for_lifecycle(request.session_id, terminal)?;
        if let Err(err) = self.pty.kill_pty(&ctx.platform_session_id, mode) {
            if terminal {
                self.clear_closing(request.session_id);
            }
            return Err(TerminalRuntimeError::Backend {
                reason: err.to_string(),
            });
        }
        let state = if terminal {
            let _ = self.remove_session(request.session_id)?;
            TerminalRuntimeState::Exited
        } else {
            TerminalRuntimeState::Running
        };
        let state_label = if state == TerminalRuntimeState::Exited {
            "exited"
        } else {
            "running"
        };
        self.audit_record(
            request.session_id,
            state,
            ctx.sequence,
            ctx.correlation_id,
            ctx.causality_id,
            format!(
                "state={state_label} action=kill pty=true escalation={:?} kill_tree={}",
                request.escalation, request.kill_tree_authorized
            ),
        )
    }

    /// Clean up orphaned platform PTY sessions known to this runtime.
    pub fn cleanup_orphans(&self) -> Result<Vec<TerminalAuditRecord>, TerminalRuntimeError> {
        self.ensure_enabled()?;
        let orphaned =
            self.pty
                .cleanup_orphaned_ptys()
                .map_err(|err| TerminalRuntimeError::Backend {
                    reason: err.to_string(),
                })?;
        let orphaned = orphaned.into_iter().collect::<HashSet<_>>();
        if orphaned.is_empty() {
            return Ok(Vec::new());
        }
        let mut sessions = self
            .sessions
            .lock()
            .map_err(|_| TerminalRuntimeError::Backend {
                reason: "terminal runtime session registry is unavailable".to_string(),
            })?;
        let mut removed = Vec::new();
        sessions.retain(|session_id, session| {
            if orphaned.contains(&session.platform_session_id) {
                removed.push((
                    *session_id,
                    session.next_sequence(),
                    session.correlation_id,
                    session.causality_id,
                ));
                false
            } else {
                true
            }
        });
        drop(sessions);
        removed
            .into_iter()
            .map(|(session_id, sequence, correlation_id, causality_id)| {
                self.audit_record(
                    session_id,
                    TerminalRuntimeState::Exited,
                    sequence,
                    correlation_id,
                    causality_id,
                    "state=exited action=cleanup_orphan pty=true".to_string(),
                )
            })
            .collect()
    }

    fn ensure_enabled(&self) -> Result<(), TerminalRuntimeError> {
        if !self.config.enabled {
            return Err(TerminalRuntimeError::Disabled);
        }
        Ok(())
    }

    fn session_for_lifecycle(
        &self,
        session_id: TerminalSessionId,
        mark_closing: bool,
    ) -> Result<LifecycleContext, TerminalRuntimeError> {
        let mut sessions = self
            .sessions
            .lock()
            .map_err(|_| TerminalRuntimeError::Backend {
                reason: "terminal runtime session registry is unavailable".to_string(),
            })?;
        let session = sessions
            .get_mut(&session_id)
            .ok_or_else(|| missing_session_error(session_id))?;
        if session.closing {
            return Err(TerminalRuntimeError::Backend {
                reason: format!("terminal session {} is closing", session_id.0),
            });
        }
        let ctx = LifecycleContext {
            platform_session_id: session.platform_session_id.clone(),
            sequence: session.next_sequence(),
            correlation_id: session.correlation_id,
            causality_id: session.causality_id,
            output_byte_limit: session.output_byte_limit,
            deadline: session.deadline,
        };
        if mark_closing {
            session.closing = true;
        }
        Ok(ctx)
    }

    fn clear_closing(&self, session_id: TerminalSessionId) {
        if let Ok(mut sessions) = self.sessions.lock()
            && let Some(session) = sessions.get_mut(&session_id)
        {
            session.closing = false;
        }
    }

    fn apply_session_metadata(
        &self,
        session_id: TerminalSessionId,
        projection: &crate::osc::TerminalShellProjection,
    ) -> Result<(), TerminalRuntimeError> {
        let mut sessions = self
            .sessions
            .lock()
            .map_err(|_| TerminalRuntimeError::Backend {
                reason: "terminal runtime session registry is unavailable".to_string(),
            })?;
        let session = sessions
            .get_mut(&session_id)
            .ok_or_else(|| missing_session_error(session_id))?;
        session.metadata.apply_shell_projection(projection);
        Ok(())
    }

    fn remove_session(
        &self,
        session_id: TerminalSessionId,
    ) -> Result<String, TerminalRuntimeError> {
        self.sessions
            .lock()
            .map_err(|_| TerminalRuntimeError::Backend {
                reason: "terminal runtime session registry is unavailable".to_string(),
            })?
            .remove(&session_id)
            .map(|session| session.platform_session_id)
            .ok_or_else(|| missing_session_error(session_id))
    }

    fn audit_record(
        &self,
        session_id: TerminalSessionId,
        state: TerminalRuntimeState,
        event_sequence: EventSequence,
        correlation_id: CorrelationId,
        causality_id: CausalityId,
        metadata_summary: String,
    ) -> Result<TerminalAuditRecord, TerminalRuntimeError> {
        let audit = TerminalAuditRecord {
            session_id,
            state,
            event_sequence,
            correlation_id,
            causality_id,
            metadata_summary,
            redaction_hints: vec![RedactionHint::MetadataOnly],
            schema_version: 1,
        };
        validate_terminal_audit_record(&audit).map_err(|err| TerminalRuntimeError::Denied {
            reason: err.message,
        })?;
        Ok(audit)
    }
}

fn validate_launch_policy(
    policy: &TerminalLaunchPolicyContract,
    max_output_bytes: u64,
) -> Result<(), TerminalRuntimeError> {
    validate_terminal_launch_policy_contract(policy).map_err(|err| {
        TerminalRuntimeError::Denied {
            reason: err.message,
        }
    })?;
    if policy.schema_version == 0
        || policy.principal_id.0.trim().is_empty()
        || policy.workspace_id.0 == 0
        || policy.trust_state != WorkspaceTrustState::Trusted
        || policy.capability_id.0 != "terminal.launch"
        || policy.output_byte_limit == 0
        || policy.output_byte_limit > max_output_bytes
        || policy.timeout_seconds == 0
    {
        return Err(TerminalRuntimeError::Denied {
            reason: "terminal launch policy is incomplete or outside bounds".to_string(),
        });
    }
    Ok(())
}

impl RuntimeSession {
    fn next_sequence(&mut self) -> EventSequence {
        let sequence = EventSequence(self.next_sequence);
        self.next_sequence = self.next_sequence.saturating_add(1);
        sequence
    }
}

fn next_terminal_session_id() -> TerminalSessionId {
    TerminalSessionId(TERMINAL_SESSION_COUNTER.fetch_add(1, Ordering::Relaxed))
}

fn missing_session_error(session_id: TerminalSessionId) -> TerminalRuntimeError {
    TerminalRuntimeError::Backend {
        reason: format!("terminal session {} is not active", session_id.0),
    }
}

fn redact_terminal_projection(output: &str, limit: u64) -> String {
    let mut projected = redact_secrets(output);
    let limit = limit as usize;
    if projected.len() > limit {
        let mut end = limit;
        while end > 0 && !projected.is_char_boundary(end) {
            end -= 1;
        }
        projected.truncate(end);
    }
    projected
}

/// Redact credentials from a terminal projection by consuming the full
/// credential value (not just a marker prefix), case-insensitively for headers
/// and environment assignments, and by collapsing known token shapes.
fn redact_secrets(input: &str) -> String {
    const REDACTED: &str = "[redacted]";
    let lower = input.to_ascii_lowercase();
    let mut out = String::with_capacity(input.len());
    let mut i = 0;
    // Treat the start of the string as a word boundary so a leading assignment
    // such as `OPENAI_API_KEY=...` is recognized.
    let mut prev_is_ws = true;
    'outer: while i < input.len() {
        let rest_lower = &lower[i..];

        // Authorization header: redact the entire value to end of line so the
        // scheme and token (e.g. `Bearer <token>`) are both removed.
        if rest_lower.starts_with("authorization:") {
            let header_len = "authorization:".len();
            out.push_str(&input[i..i + header_len]);
            out.push(' ');
            out.push_str(REDACTED);
            let line_end = input[i + header_len..]
                .find('\n')
                .map(|p| i + header_len + p)
                .unwrap_or(input.len());
            i = line_end;
            prev_is_ws = false;
            continue;
        }

        // Sensitive environment/config assignment: NAME=value, value consumed
        // through a delimiter.
        if prev_is_ws
            && let Some((eq_end, value_end)) = match_sensitive_assignment(input, &lower, i)
        {
            out.push_str(&input[i..eq_end]);
            out.push_str(REDACTED);
            i = value_end;
            prev_is_ws = false;
            continue;
        }

        // Token shapes: prefix followed by the token body.
        for prefix in ["ghp_", "gho_", "xoxb-", "sk-"] {
            if rest_lower.starts_with(prefix) {
                let mut end = i + prefix.len();
                while end < input.len() {
                    let c = input[end..].chars().next().unwrap_or('\0');
                    if c.is_ascii_alphanumeric() || c == '_' || c == '-' {
                        end += c.len_utf8();
                    } else {
                        break;
                    }
                }
                out.push_str(REDACTED);
                i = end;
                prev_is_ws = false;
                continue 'outer;
            }
        }

        // Bare well-known credential names (no `=`), longest first.
        for marker in ["openai_api_key", "aws_secret_access_key", "secret"] {
            if rest_lower.starts_with(marker) {
                out.push_str(REDACTED);
                i += marker.len();
                prev_is_ws = false;
                continue 'outer;
            }
        }

        let c = input[i..].chars().next().unwrap_or('\0');
        prev_is_ws = c.is_whitespace();
        out.push(c);
        i += c.len_utf8();
    }
    out
}

/// If `input[start..]` begins with a sensitive `NAME=value` assignment, return
/// the byte offset just past `=` and the byte offset where the value ends
/// (whitespace, quote, or end of input).
fn match_sensitive_assignment(input: &str, lower: &str, start: usize) -> Option<(usize, usize)> {
    let mut ident_end = start;
    while ident_end < input.len() {
        let c = input[ident_end..].chars().next().unwrap_or('\0');
        if c.is_ascii_alphanumeric() || c == '_' {
            ident_end += c.len_utf8();
        } else {
            break;
        }
    }
    if ident_end == start || !input[ident_end..].starts_with('=') {
        return None;
    }
    let ident_lower = &lower[start..ident_end];
    let sensitive = [
        "secret",
        "token",
        "password",
        "passwd",
        "api_key",
        "access_key",
        "apikey",
    ]
    .iter()
    .any(|kw| ident_lower.contains(kw));
    if !sensitive {
        return None;
    }
    let eq_end = ident_end + 1;
    let mut value_end = eq_end;
    while value_end < input.len() {
        let c = input[value_end..].chars().next().unwrap_or('\0');
        if c.is_whitespace() || c == '"' || c == '\'' {
            break;
        }
        value_end += c.len_utf8();
    }
    Some((eq_end, value_end))
}

fn uuid_from_value(value: u64) -> uuid::Uuid {
    uuid::Uuid::from_u128(0x018f_0000_0000_7000_8000_1000_0000_0000_u128 + value as u128)
}

fn debug_zero_range() -> ProtocolTextRange {
    ProtocolTextRange {
        start: TextCoordinate {
            line: 0,
            character: 0,
            byte_offset: None,
            utf16_offset: None,
        },
        end: TextCoordinate {
            line: 0,
            character: 0,
            byte_offset: None,
            utf16_offset: None,
        },
    }
}

#[cfg(test)]
mod tests {
    use std::{
        collections::VecDeque,
        sync::{Arc, Mutex as StdMutex},
    };

    use legion_platform::{PlatformError, PtyReadResult, PtySession};
    use legion_protocol::{CapabilityId, PrincipalId, WorkspaceId};

    use super::*;

    #[derive(Debug, Clone)]
    struct FakePty {
        state: Arc<StdMutex<FakePtyState>>,
    }

    #[derive(Debug)]
    struct FakePtyState {
        id: String,
        output: String,
        exit_on_read: bool,
        fail_close: bool,
        fail_kill: bool,
        scripted_reads: VecDeque<PtyReadResult>,
        calls: Vec<String>,
        orphaned: Vec<String>,
    }

    impl FakePty {
        fn degraded(output: impl Into<String>) -> Self {
            Self::new("fake", output, false)
        }

        fn native(output: impl Into<String>) -> Self {
            Self::new("native-unix-pty-test", output, false)
        }

        fn native_exiting(output: impl Into<String>) -> Self {
            Self::new("native-unix-pty-test", output, true)
        }

        fn native_scripted_reads(reads: Vec<PtyReadResult>) -> Self {
            let pty = Self::new("native-unix-pty-test", "", false);
            pty.state.lock().expect("fake pty state").scripted_reads = VecDeque::from(reads);
            pty
        }

        fn new(id: impl Into<String>, output: impl Into<String>, exit_on_read: bool) -> Self {
            Self {
                state: Arc::new(StdMutex::new(FakePtyState {
                    id: id.into(),
                    output: output.into(),
                    exit_on_read,
                    fail_close: false,
                    fail_kill: false,
                    scripted_reads: VecDeque::new(),
                    calls: Vec::new(),
                    orphaned: Vec::new(),
                })),
            }
        }

        fn calls(&self) -> Vec<String> {
            self.state.lock().expect("fake pty state").calls.clone()
        }

        fn set_orphaned(&self, orphaned: Vec<String>) {
            self.state.lock().expect("fake pty state").orphaned = orphaned;
        }

        fn fail_close(&self) {
            self.state.lock().expect("fake pty state").fail_close = true;
        }

        fn fail_kill(&self) {
            self.state.lock().expect("fake pty state").fail_kill = true;
        }
    }

    impl PtyService for FakePty {
        fn spawn_pty(&self, request: &PtyRequest) -> Result<PtySession, PlatformError> {
            if request.command == "fail" {
                return Err(PlatformError::PtyUnavailable {
                    reason: "fake failure".to_string(),
                });
            }
            let mut state = self.state.lock().expect("fake pty state");
            state.calls.push(format!("spawn:{}", request.command));
            Ok(PtySession {
                id: state.id.clone(),
                output: state.output.clone(),
            })
        }

        fn write_pty(&self, session_id: &str, input: &str) -> Result<(), PlatformError> {
            self.state
                .lock()
                .expect("fake pty state")
                .calls
                .push(format!("write:{session_id}:{}", input.len()));
            Ok(())
        }

        fn resize_pty(&self, session_id: &str, cols: u16, rows: u16) -> Result<(), PlatformError> {
            self.state
                .lock()
                .expect("fake pty state")
                .calls
                .push(format!("resize:{session_id}:{cols}x{rows}"));
            Ok(())
        }

        fn read_pty(
            &self,
            session_id: &str,
            _max_bytes: usize,
        ) -> Result<PtyReadResult, PlatformError> {
            let mut state = self.state.lock().expect("fake pty state");
            state.calls.push(format!("read:{session_id}"));
            if let Some(mut read) = state.scripted_reads.pop_front() {
                read.id = session_id.to_string();
                return Ok(read);
            }
            let output = std::mem::take(&mut state.output);
            Ok(PtyReadResult {
                id: session_id.to_string(),
                output,
                exited: state.exit_on_read,
                exit_code: state.exit_on_read.then_some(0),
                truncated: false,
            })
        }

        fn close_pty(&self, session_id: &str) -> Result<(), PlatformError> {
            let mut state = self.state.lock().expect("fake pty state");
            state.calls.push(format!("close:{session_id}"));
            if state.fail_close {
                return Err(PlatformError::PtyUnavailable {
                    reason: "fake close failure".to_string(),
                });
            }
            Ok(())
        }

        fn kill_pty(&self, session_id: &str, mode: PtyKillMode) -> Result<(), PlatformError> {
            let mut state = self.state.lock().expect("fake pty state");
            state.calls.push(format!("kill:{session_id}:{mode:?}"));
            if state.fail_kill {
                return Err(PlatformError::PtyUnavailable {
                    reason: "fake kill failure".to_string(),
                });
            }
            Ok(())
        }

        fn cleanup_orphaned_ptys(&self) -> Result<Vec<String>, PlatformError> {
            let mut state = self.state.lock().expect("fake pty state");
            state.calls.push("cleanup".to_string());
            Ok(state.orphaned.clone())
        }
    }

    fn policy() -> TerminalLaunchPolicyContract {
        TerminalLaunchPolicyContract {
            principal_id: PrincipalId("tester".to_string()),
            workspace_id: WorkspaceId(42),
            trust_state: WorkspaceTrustState::Trusted,
            capability_id: CapabilityId("terminal.launch".to_string()),
            cwd_policy: "workspace-contained".to_string(),
            output_byte_limit: 1024,
            timeout_seconds: 60,
            schema_version: 1,
        }
    }

    fn poll_request(session_id: TerminalSessionId, value: u64) -> TerminalRuntimeOutputPollRequest {
        TerminalRuntimeOutputPollRequest {
            session_id,
            event_sequence: EventSequence(value),
            correlation_id: CorrelationId(value),
            causality_id: CausalityId(uuid_from_value(value)),
        }
    }

    fn close_request(session_id: TerminalSessionId, value: u64) -> TerminalCloseRequest {
        TerminalCloseRequest {
            session_id,
            principal_id: PrincipalId("tester".to_string()),
            capability_id: CapabilityId("terminal.close".to_string()),
            event_sequence: EventSequence(value),
            correlation_id: CorrelationId(value),
            causality_id: CausalityId(uuid_from_value(value)),
            metadata_summary: "state=close request".to_string(),
            redaction_hints: vec![RedactionHint::MetadataOnly],
            schema_version: 1,
        }
    }

    fn kill_request(
        session_id: TerminalSessionId,
        value: u64,
        escalation: TerminalKillEscalation,
    ) -> TerminalKillRequest {
        TerminalKillRequest {
            session_id,
            principal_id: PrincipalId("tester".to_string()),
            capability_id: CapabilityId("terminal.kill".to_string()),
            escalation,
            kill_tree_authorized: escalation == TerminalKillEscalation::KillTree,
            escalation_timeout_ms: 1_000,
            event_sequence: EventSequence(value),
            correlation_id: CorrelationId(value),
            causality_id: CausalityId(uuid_from_value(value)),
            metadata_summary: "state=kill request".to_string(),
            redaction_hints: vec![RedactionHint::MetadataOnly],
            schema_version: 1,
        }
    }

    #[test]
    fn terminal_runtime_tracks_osc_metadata_per_session() {
        let runtime = TerminalRuntime::new(
            TerminalRuntimeConfig::enabled(),
            FakePty::native_scripted_reads(vec![PtyReadResult {
                id: "native-unix-pty-test".to_string(),
                output: "\x1b]7;file:///repo\x1b\\visible\x1b]133;D;3\x1b\\".to_string(),
                exited: false,
                exit_code: None,
                truncated: false,
            }]),
        );
        let launched = runtime
            .launch(TerminalRuntimeLaunchRequest {
                policy: policy(),
                command: "bash".to_string(),
                args: vec![],
            })
            .expect("launch");
        let session_id = launched.audit.session_id;

        let output = runtime
            .poll_output(poll_request(session_id, 120))
            .expect("poll osc output");
        assert_eq!(output.output.redacted_payload, "visible");

        let metadata = runtime
            .session_metadata(session_id)
            .expect("session metadata should be queryable");
        assert_eq!(metadata.cwd.as_deref(), Some("/repo"));
        assert_eq!(metadata.exit_code, Some(3));
        assert_eq!(
            metadata.boundary,
            Some(crate::osc::TerminalShellBoundary::CommandFinished)
        );
    }

    #[test]
    fn terminal_fixture_is_default_off() {
        let runtime = TerminalFixtureRuntime::new(TerminalFixtureConfig::default());
        assert!(matches!(
            runtime.launch(policy()),
            Err(TerminalFixtureError::Disabled)
        ));
    }

    #[test]
    fn terminal_fixture_launches_metadata_only_and_exits() {
        let runtime = TerminalFixtureRuntime::new(TerminalFixtureConfig::enabled());
        let record = runtime.launch(policy()).expect("launch");
        assert_eq!(record.state, TerminalRuntimeState::Running);
        assert!(!record.metadata_summary.contains("terminal_output"));

        let chunk = runtime
            .output_chunk(&record, "redacted output", 15)
            .expect("chunk");
        assert_eq!(chunk.redaction, RedactionHint::MetadataOnly);
        assert!(!chunk.redacted_payload.contains("secret"));

        let exit = runtime.exit(&record);
        assert_eq!(exit.state, TerminalRuntimeState::Exited);
    }

    #[test]
    fn terminal_fixture_rejects_untrusted_launch() {
        let runtime = TerminalFixtureRuntime::new(TerminalFixtureConfig::enabled());
        let denied = TerminalLaunchPolicyContract {
            trust_state: WorkspaceTrustState::Untrusted,
            ..policy()
        };
        assert!(matches!(
            runtime.launch(denied),
            Err(TerminalFixtureError::Denied { .. })
        ));
    }

    #[test]
    fn terminal_runtime_default_off_denies_launch() {
        let runtime = TerminalRuntime::new(
            TerminalRuntimeConfig::default(),
            FakePty::degraded(String::new()),
        );
        assert!(matches!(
            runtime.launch(TerminalRuntimeLaunchRequest {
                policy: policy(),
                command: "test".to_string(),
                args: vec![],
            }),
            Err(TerminalRuntimeError::Disabled)
        ));
    }

    #[test]
    fn terminal_runtime_launch_creates_metadata_only_degraded_audit() {
        let runtime = TerminalRuntime::new(
            TerminalRuntimeConfig::enabled(),
            FakePty::degraded("hello secret"),
        );
        let outcome = runtime
            .launch(TerminalRuntimeLaunchRequest {
                policy: policy(),
                command: "test".to_string(),
                args: vec![],
            })
            .expect("launch");
        assert_eq!(outcome.audit.state, TerminalRuntimeState::Degraded);
        assert!(outcome.audit.metadata_summary.contains("pty=false"));
        assert!(!outcome.audit.metadata_summary.contains("terminal_output"));
        assert!(outcome.output.redacted_payload.contains("[redacted]"));
    }

    #[test]
    fn terminal_runtime_records_native_pty_audit_when_platform_backend_is_native() {
        let runtime =
            TerminalRuntime::new(TerminalRuntimeConfig::enabled(), FakePty::degraded("hello"));
        let outcome = runtime
            .launch(TerminalRuntimeLaunchRequest {
                policy: policy(),
                command: "test".to_string(),
                args: vec![],
            })
            .expect("launch");
        assert_eq!(outcome.audit.state, TerminalRuntimeState::Degraded);

        let runtime =
            TerminalRuntime::new(TerminalRuntimeConfig::enabled(), FakePty::native("hello"));
        let outcome = runtime
            .launch(TerminalRuntimeLaunchRequest {
                policy: policy(),
                command: "test".to_string(),
                args: vec![],
            })
            .expect("native launch");
        assert_eq!(outcome.audit.state, TerminalRuntimeState::Running);
        assert!(outcome.audit.metadata_summary.contains("pty=true"));
        assert!(!outcome.audit.metadata_summary.contains("terminal_output"));
    }

    #[test]
    fn terminal_runtime_dispatches_native_lifecycle_operations() {
        let pty = FakePty::native("boot secret");
        let runtime = TerminalRuntime::new(TerminalRuntimeConfig::enabled(), pty.clone());
        let outcome = runtime
            .launch(TerminalRuntimeLaunchRequest {
                policy: policy(),
                command: "test".to_string(),
                args: vec![],
            })
            .expect("native launch");
        let session_id = outcome.audit.session_id;

        let input = runtime
            .input(TerminalInput {
                session_id,
                correlation_id: CorrelationId(77),
                payload: "dir\r".to_string(),
            })
            .expect("input");
        assert_eq!(input.state, TerminalRuntimeState::Running);
        assert!(input.metadata_summary.contains("input_bytes=4"));
        assert!(input.metadata_summary.contains("correlation=77"));
        assert!(!input.metadata_summary.contains("dir"));

        let resize = runtime
            .resize(TerminalResize {
                session_id,
                cols: 120,
                rows: 40,
            })
            .expect("resize");
        assert_eq!(resize.state, TerminalRuntimeState::Running);
        assert!(resize.metadata_summary.contains("120"));

        let output = runtime
            .poll_output(poll_request(session_id, 88))
            .expect("poll output");
        assert_eq!(output.audit.state, TerminalRuntimeState::Running);
        assert!(output.output.redacted_payload.contains("[redacted]"));
        assert!(!output.audit.metadata_summary.contains("boot"));
        assert!(!output.audit.metadata_summary.contains("secret"));

        let close = runtime.close(close_request(session_id, 89)).expect("close");
        assert_eq!(close.state, TerminalRuntimeState::Exited);

        let calls = pty.calls();
        assert!(calls.contains(&"spawn:test".to_string()));
        assert!(calls.contains(&"write:native-unix-pty-test:4".to_string()));
        assert!(calls.contains(&"resize:native-unix-pty-test:120x40".to_string()));
        assert!(calls.contains(&"read:native-unix-pty-test".to_string()));
        assert!(calls.contains(&"close:native-unix-pty-test".to_string()));
    }

    #[test]
    fn terminal_runtime_keeps_session_registered_when_close_or_kill_fails() {
        let pty = FakePty::native("");
        let runtime = TerminalRuntime::new(TerminalRuntimeConfig::enabled(), pty.clone());
        let outcome = runtime
            .launch(TerminalRuntimeLaunchRequest {
                policy: policy(),
                command: "test".to_string(),
                args: vec![],
            })
            .expect("native launch");
        let close_session_id = outcome.audit.session_id;
        pty.fail_close();
        assert!(matches!(
            runtime.close(close_request(close_session_id, 89)),
            Err(TerminalRuntimeError::Backend { .. })
        ));
        runtime
            .input(TerminalInput {
                session_id: close_session_id,
                correlation_id: CorrelationId(90),
                payload: "x".to_string(),
            })
            .expect("session remains after close failure");

        let pty = FakePty::native("");
        let runtime = TerminalRuntime::new(TerminalRuntimeConfig::enabled(), pty.clone());
        let outcome = runtime
            .launch(TerminalRuntimeLaunchRequest {
                policy: policy(),
                command: "test".to_string(),
                args: vec![],
            })
            .expect("native launch");
        let kill_session_id = outcome.audit.session_id;
        pty.fail_kill();
        assert!(matches!(
            runtime.kill(kill_request(
                kill_session_id,
                91,
                TerminalKillEscalation::Terminate,
            )),
            Err(TerminalRuntimeError::Backend { .. })
        ));
        runtime
            .input(TerminalInput {
                session_id: kill_session_id,
                correlation_id: CorrelationId(92),
                payload: "x".to_string(),
            })
            .expect("session remains after kill failure");
    }

    #[test]
    fn terminal_runtime_interrupt_preserves_session_and_uses_interrupt_mode() {
        let pty = FakePty::native("");
        let runtime = TerminalRuntime::new(TerminalRuntimeConfig::enabled(), pty.clone());
        let outcome = runtime
            .launch(TerminalRuntimeLaunchRequest {
                policy: policy(),
                command: "test".to_string(),
                args: vec![],
            })
            .expect("native launch");
        let session_id = outcome.audit.session_id;

        let interrupt = runtime
            .kill(kill_request(
                session_id,
                90,
                TerminalKillEscalation::Interrupt,
            ))
            .expect("interrupt");
        assert_eq!(interrupt.state, TerminalRuntimeState::Running);
        assert!(
            pty.calls()
                .contains(&"kill:native-unix-pty-test:Interrupt".to_string())
        );
        runtime
            .input(TerminalInput {
                session_id,
                correlation_id: CorrelationId(91),
                payload: "x".to_string(),
            })
            .expect("interrupt keeps session available");
    }

    #[test]
    fn terminal_runtime_kill_and_orphan_cleanup_remove_sessions() {
        let pty = FakePty::native("");
        let runtime = TerminalRuntime::new(TerminalRuntimeConfig::enabled(), pty.clone());
        let killed = runtime
            .launch(TerminalRuntimeLaunchRequest {
                policy: policy(),
                command: "test".to_string(),
                args: vec![],
            })
            .expect("native launch");
        let killed_session_id = killed.audit.session_id;

        let kill = runtime
            .kill(kill_request(
                killed_session_id,
                90,
                TerminalKillEscalation::Terminate,
            ))
            .expect("kill");
        assert_eq!(kill.state, TerminalRuntimeState::Exited);
        assert!(
            pty.calls()
                .contains(&"kill:native-unix-pty-test:Terminate".to_string())
        );
        assert!(matches!(
            runtime.input(TerminalInput {
                session_id: killed_session_id,
                correlation_id: CorrelationId(91),
                payload: "x".to_string(),
            }),
            Err(TerminalRuntimeError::Backend { .. })
        ));

        let orphaned = runtime
            .launch(TerminalRuntimeLaunchRequest {
                policy: policy(),
                command: "test".to_string(),
                args: vec![],
            })
            .expect("native launch");
        pty.set_orphaned(vec!["native-unix-pty-test".to_string()]);
        let cleaned = runtime.cleanup_orphans().expect("cleanup");
        assert_eq!(cleaned.len(), 1);
        assert_eq!(cleaned[0].session_id, orphaned.audit.session_id);
        assert_eq!(cleaned[0].state, TerminalRuntimeState::Exited);
    }

    #[test]
    fn terminal_runtime_poll_output_removes_exited_native_session() {
        let runtime = TerminalRuntime::new(
            TerminalRuntimeConfig::enabled(),
            FakePty::native_exiting("done"),
        );
        let outcome = runtime
            .launch(TerminalRuntimeLaunchRequest {
                policy: policy(),
                command: "test".to_string(),
                args: vec![],
            })
            .expect("native launch");
        let session_id = outcome.audit.session_id;

        let output = runtime
            .poll_output(poll_request(session_id, 92))
            .expect("poll output");
        assert_eq!(output.audit.state, TerminalRuntimeState::Exited);
        assert!(output.output.redacted_payload.contains("done"));
        assert!(matches!(
            runtime.resize(TerminalResize {
                session_id,
                cols: 80,
                rows: 24,
            }),
            Err(TerminalRuntimeError::Backend { .. })
        ));
    }

    #[test]
    fn terminal_runtime_keeps_exited_native_session_until_truncated_output_is_drained() {
        let runtime = TerminalRuntime::new(
            TerminalRuntimeConfig::enabled(),
            FakePty::native_scripted_reads(vec![
                PtyReadResult {
                    id: String::new(),
                    output: "first".to_string(),
                    exited: true,
                    exit_code: Some(0),
                    truncated: true,
                },
                PtyReadResult {
                    id: String::new(),
                    output: "tail".to_string(),
                    exited: true,
                    exit_code: Some(0),
                    truncated: false,
                },
            ]),
        );
        let outcome = runtime
            .launch(TerminalRuntimeLaunchRequest {
                policy: policy(),
                command: "test".to_string(),
                args: vec![],
            })
            .expect("native launch");
        let session_id = outcome.audit.session_id;

        let first = runtime
            .poll_output(poll_request(session_id, 93))
            .expect("first poll");
        assert_eq!(first.audit.state, TerminalRuntimeState::Exited);
        assert!(first.output.truncated);
        assert!(first.output.redacted_payload.contains("first"));

        let second = runtime
            .poll_output(poll_request(session_id, 94))
            .expect("drain poll");
        assert_eq!(second.audit.state, TerminalRuntimeState::Exited);
        assert!(!second.output.truncated);
        assert!(second.output.redacted_payload.contains("tail"));
        assert!(matches!(
            runtime.resize(TerminalResize {
                session_id,
                cols: 80,
                rows: 24,
            }),
            Err(TerminalRuntimeError::Backend { .. })
        ));
    }

    #[test]
    fn terminal_runtime_rejects_untrusted_policy() {
        let runtime = TerminalRuntime::new(
            TerminalRuntimeConfig::enabled(),
            FakePty::degraded(String::new()),
        );
        assert!(matches!(
            runtime.launch(TerminalRuntimeLaunchRequest {
                policy: TerminalLaunchPolicyContract {
                    trust_state: WorkspaceTrustState::Untrusted,
                    ..policy()
                },
                command: "test".to_string(),
                args: vec![],
            }),
            Err(TerminalRuntimeError::Denied { .. })
        ));
    }

    #[test]
    fn terminal_runtime_enforces_per_launch_output_byte_limit() {
        let mut config = TerminalRuntimeConfig::enabled();
        config.max_output_bytes = 1024;
        let runtime = TerminalRuntime::new(config, FakePty::native("abcdefghij"));
        let outcome = runtime
            .launch(TerminalRuntimeLaunchRequest {
                policy: TerminalLaunchPolicyContract {
                    output_byte_limit: 4,
                    ..policy()
                },
                command: "test".to_string(),
                args: vec![],
            })
            .expect("native launch");
        // The smaller per-launch limit (4) wins over the global config max (1024).
        assert_eq!(outcome.output.byte_count, 4);
        assert!(outcome.output.truncated);
    }

    #[test]
    fn terminal_runtime_poll_uses_session_owned_event_identity() {
        let runtime = TerminalRuntime::new(TerminalRuntimeConfig::enabled(), FakePty::native("ok"));
        let launch = runtime
            .launch(TerminalRuntimeLaunchRequest {
                policy: policy(),
                command: "test".to_string(),
                args: vec![],
            })
            .expect("native launch");
        let session_id = launch.audit.session_id;
        // The caller supplies arbitrary identity (value 7); the audit must use
        // the session-owned monotonic sequence and correlation instead.
        let poll = runtime
            .poll_output(poll_request(session_id, 7))
            .expect("poll");
        assert_eq!(
            poll.audit.event_sequence.0,
            launch.audit.event_sequence.0 + 1
        );
        assert_eq!(poll.audit.correlation_id, launch.audit.correlation_id);
        assert_ne!(poll.audit.correlation_id, CorrelationId(7));
    }

    #[test]
    fn terminal_runtime_poll_enforces_launch_deadline_fail_closed() {
        let pty = FakePty::native("still-running");
        let runtime = TerminalRuntime::new(TerminalRuntimeConfig::enabled(), pty.clone());
        let launch = runtime
            .launch(TerminalRuntimeLaunchRequest {
                policy: policy(),
                command: "test".to_string(),
                args: vec![],
            })
            .expect("native launch");
        let session_id = launch.audit.session_id;
        // Force the deadline into the past to simulate timeout expiry.
        {
            let mut sessions = runtime.sessions.lock().expect("registry");
            let session = sessions.get_mut(&session_id).expect("session");
            session.deadline = Some(Instant::now() - Duration::from_secs(1));
        }
        let poll = runtime
            .poll_output(poll_request(session_id, 5))
            .expect("poll");
        assert_eq!(poll.audit.state, TerminalRuntimeState::Exited);
        assert!(poll.audit.metadata_summary.contains("reason=timeout"));
        assert!(
            pty.calls()
                .contains(&"kill:native-unix-pty-test:Terminate".to_string())
        );
        // Session removed; subsequent operations fail closed.
        assert!(matches!(
            runtime.resize(TerminalResize {
                session_id,
                cols: 80,
                rows: 24,
            }),
            Err(TerminalRuntimeError::Backend { .. })
        ));
    }

    #[test]
    fn terminal_runtime_rejects_operations_on_closing_session() {
        let runtime = TerminalRuntime::new(TerminalRuntimeConfig::enabled(), FakePty::native(""));
        let launch = runtime
            .launch(TerminalRuntimeLaunchRequest {
                policy: policy(),
                command: "test".to_string(),
                args: vec![],
            })
            .expect("native launch");
        let session_id = launch.audit.session_id;
        // Simulate an in-flight close having marked the session closing.
        {
            let mut sessions = runtime.sessions.lock().expect("registry");
            sessions.get_mut(&session_id).expect("session").closing = true;
        }
        assert!(matches!(
            runtime.input(TerminalInput {
                session_id,
                correlation_id: CorrelationId(1),
                payload: "x".to_string(),
            }),
            Err(TerminalRuntimeError::Backend { .. })
        ));
    }

    #[test]
    fn redact_secrets_consumes_full_credential_values() {
        let bearer = redact_terminal_projection("Authorization: Bearer abc.def-123\n", 4096);
        assert_eq!(bearer, "Authorization: [redacted]\n");

        let lower_header = redact_terminal_projection("authorization: bearer SEKRET", 4096);
        assert_eq!(lower_header, "authorization: [redacted]");

        let mixed_header = redact_terminal_projection("AuThOrIzAtIoN: Bearer tok", 4096);
        assert_eq!(mixed_header, "AuThOrIzAtIoN: [redacted]");

        let env = redact_terminal_projection("OPENAI_API_KEY=sk-livetoken123 next", 4096);
        assert_eq!(env, "OPENAI_API_KEY=[redacted] next");

        let aws = redact_terminal_projection("aws_secret_access_key=AKIAEXAMPLEVALUE", 4096);
        assert_eq!(aws, "aws_secret_access_key=[redacted]");

        let gh = redact_terminal_projection("token ghp_ABC123def_456 done", 4096);
        assert_eq!(gh, "token [redacted] done");

        let slack = redact_terminal_projection("xoxb-1-2-abcDEF and sk-xyz789", 4096);
        assert_eq!(slack, "[redacted] and [redacted]");

        // No false positives on ordinary text.
        let clean = redact_terminal_projection("hello world output", 4096);
        assert_eq!(clean, "hello world output");
    }
}
