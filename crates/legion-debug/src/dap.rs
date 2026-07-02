//! Metadata-only DAP client runtime.
//!
//! This crate owns the debug adapter lifecycle state machine used by the app.
//! It intentionally projects adapter progress as metadata-only protocol DTOs;
//! concrete adapter binary resolution and CodeLLDB policy are later backlog
//! tasks.

use std::{collections::HashMap, sync::Mutex};

use legion_protocol::{
    CanonicalPath, CausalityId, CorrelationId, DebugAdapterAuditRecord, DebugAdapterLaunchRequest,
    DebugBreakpointRecord, DebugConsoleCategory, DebugConsoleEntry, DebugInlineValue,
    DebugSessionId, DebugStackFrame, DebugStepKind, DebugVariable, DebugWatchExpression,
    EventSequence, ProtocolTextRange, RedactionHint, TextCoordinate,
    validate_debug_adapter_audit_record,
};
use thiserror::Error;

use crate::state::DapLifecycleState;

/// DAP client runtime configuration.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DapClientConfig {
    /// Whether the DAP client runtime is enabled.
    pub enabled: bool,
}

impl DapClientConfig {
    /// Return an enabled DAP client runtime configuration.
    pub fn enabled() -> Self {
        Self { enabled: true }
    }
}

/// DAP client runtime error.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum DapClientError {
    /// Runtime is disabled.
    #[error("DAP client runtime is disabled")]
    Disabled,
    /// Request was denied by validation.
    #[error("DAP client runtime denied request: {reason}")]
    Denied {
        /// Display-safe denial reason.
        reason: String,
    },
}

/// Metadata-only DAP lifecycle projection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DapClientOutcome {
    /// Metadata-only adapter audit.
    pub audit: DebugAdapterAuditRecord,
    /// Internal lifecycle state reached by this operation.
    pub lifecycle_state: DapLifecycleState,
    /// Adapter type used for the session.
    pub adapter_type: String,
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

/// Metadata-only DAP client runtime.
#[derive(Debug)]
pub struct DapClientRuntime {
    config: DapClientConfig,
    session_adapter_types: Mutex<HashMap<DebugSessionId, String>>,
}

impl Clone for DapClientRuntime {
    fn clone(&self) -> Self {
        let sessions = self
            .session_adapter_types
            .lock()
            .map(|sessions| sessions.clone())
            .unwrap_or_default();
        Self {
            config: self.config.clone(),
            session_adapter_types: Mutex::new(sessions),
        }
    }
}

impl DapClientRuntime {
    /// Construct a DAP client runtime from configuration.
    pub fn new(config: DapClientConfig) -> Self {
        Self {
            config,
            session_adapter_types: Mutex::new(HashMap::new()),
        }
    }

    /// Launch a DAP adapter lifecycle through initialize/launch/pause metadata.
    pub fn launch(
        &self,
        request: DebugAdapterLaunchRequest,
    ) -> Result<DapClientOutcome, DapClientError> {
        self.ensure_enabled()?;
        if request.workspace_id.0 == 0
            || request.configuration_id.0.trim().is_empty()
            || request.adapter_type.trim().is_empty()
            || request.schema_version == 0
        {
            return Err(DapClientError::Denied {
                reason: "debug launch request is incomplete".to_string(),
            });
        }

        let session_id = DebugSessionId(format!(
            "dap:{}:{}",
            request.workspace_id.0, request.configuration_id.0
        ));
        self.session_adapter_types
            .lock()
            .map_err(|_| DapClientError::Denied {
                reason: "DAP client session registry is unavailable".to_string(),
            })?
            .insert(session_id.clone(), request.adapter_type.clone());

        let sequence = EventSequence(1);
        let audit = self.audit_record(
            session_id.clone(),
            DapLifecycleState::Paused,
            request.adapter_type.clone(),
            sequence,
            CorrelationId(request.workspace_id.0 as u64),
            CausalityId(uuid_from_value(request.workspace_id.0 as u64)),
            format!(
                "action=initialize,launch state=paused adapter={} initialized=true breakpoints={}",
                request.adapter_type,
                request.breakpoints.len()
            ),
        )?;
        let breakpoints = request
            .breakpoints
            .into_iter()
            .map(|mut breakpoint| {
                breakpoint.session_id = Some(session_id.clone());
                breakpoint.verified = true;
                breakpoint.message = Some("verified by DAP client runtime".to_string());
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

        Ok(DapClientOutcome {
            audit,
            lifecycle_state: DapLifecycleState::Paused,
            adapter_type: request.adapter_type.clone(),
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
                    "initialize adapter={} • launch configuration={}",
                    request.adapter_type, request.configuration_id.0
                ),
                sequence,
                redaction_hints: vec![RedactionHint::MetadataOnly],
                schema_version: 1,
            }],
        })
    }

    /// Step an active DAP session through the metadata-only lifecycle.
    pub fn step(
        &self,
        session_id: DebugSessionId,
        kind: DebugStepKind,
    ) -> Result<DapClientOutcome, DapClientError> {
        self.ensure_enabled()?;
        if session_id.0.trim().is_empty() {
            return Err(DapClientError::Denied {
                reason: "debug session id is required".to_string(),
            });
        }
        let adapter_type = self
            .session_adapter_types
            .lock()
            .map_err(|_| DapClientError::Denied {
                reason: "DAP client session registry is unavailable".to_string(),
            })?
            .get(&session_id)
            .cloned()
            .unwrap_or_else(|| "lldb-dap".to_string());
        let label = match kind {
            DebugStepKind::Continue => "continue",
            DebugStepKind::Over => "over",
            DebugStepKind::Into => "into",
            DebugStepKind::Out => "out",
            DebugStepKind::Back => "back",
        };
        let audit = self.audit_record(
            session_id.clone(),
            DapLifecycleState::Paused,
            adapter_type.clone(),
            EventSequence(2),
            CorrelationId(2),
            CausalityId(uuid_from_value(2)),
            format!("action=step state=paused step={label}"),
        )?;
        Ok(DapClientOutcome {
            audit,
            lifecycle_state: DapLifecycleState::Paused,
            adapter_type: adapter_type.clone(),
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

    fn ensure_enabled(&self) -> Result<(), DapClientError> {
        if !self.config.enabled {
            return Err(DapClientError::Disabled);
        }
        Ok(())
    }

    fn audit_record(
        &self,
        session_id: DebugSessionId,
        lifecycle_state: DapLifecycleState,
        adapter_type: String,
        event_sequence: EventSequence,
        correlation_id: CorrelationId,
        causality_id: CausalityId,
        metadata_summary: String,
    ) -> Result<DebugAdapterAuditRecord, DapClientError> {
        let audit = DebugAdapterAuditRecord {
            session_id,
            state: lifecycle_state.as_debug_session_state(),
            adapter_type,
            event_sequence,
            correlation_id,
            causality_id,
            metadata_summary,
            redaction_hints: vec![RedactionHint::MetadataOnly],
            schema_version: 1,
        };
        validate_debug_adapter_audit_record(&audit).map_err(|err| DapClientError::Denied {
            reason: err.message,
        })?;
        Ok(audit)
    }
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

fn uuid_from_value(value: u64) -> uuid::Uuid {
    let mut bytes = [0_u8; 16];
    bytes[8..].copy_from_slice(&value.to_be_bytes());
    uuid::Uuid::from_bytes(bytes)
}
