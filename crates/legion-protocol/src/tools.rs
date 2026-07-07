//! Schema-validated tool definitions shared between agent and protocol layers.

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

/// Tool kinds supported by the native Legion agent harness.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum LegionToolKind {
    /// Read a file or bounded slice of file content.
    Read,
    /// Grep for a pattern across workspace content.
    Grep,
    /// Glob workspace paths.
    Glob,
    /// Produce a structural outline for a file.
    Outline,
    /// Propose an edit rather than mutating the workspace directly.
    EditAsProposal,
    /// Launch a terminal command through the audited runtime boundary.
    TerminalCommand,
    /// Forward a call to an MCP tool by server/tool name.
    McpPassthrough,
}

impl LegionToolKind {
    /// Stable registry name for the tool.
    pub const fn tool_name(self) -> &'static str {
        match self {
            Self::Read => "read",
            Self::Grep => "grep",
            Self::Glob => "glob",
            Self::Outline => "outline",
            Self::EditAsProposal => "edit-as-proposal",
            Self::TerminalCommand => "terminal-command",
            Self::McpPassthrough => "mcp-passthrough",
        }
    }

    /// Display label for the tool.
    pub const fn description_label(self) -> &'static str {
        match self {
            Self::Read => "Read workspace content",
            Self::Grep => "Search workspace content",
            Self::Glob => "Enumerate matching workspace paths",
            Self::Outline => "Project a structural outline",
            Self::EditAsProposal => "Draft an edit as a proposal",
            Self::TerminalCommand => "Launch an audited terminal command",
            Self::McpPassthrough => "Forward an MCP tool invocation",
        }
    }

    /// Required schema keys for the tool input object.
    pub const fn required_fields(self) -> &'static [&'static str] {
        match self {
            Self::Read => &["path"],
            Self::Grep => &["pattern"],
            Self::Glob => &["pattern"],
            Self::Outline => &["path"],
            Self::EditAsProposal => &["path", "replacement"],
            Self::TerminalCommand => &["command"],
            Self::McpPassthrough => &["server_id", "tool_name", "arguments"],
        }
    }
}

/// Registry entry for a schema-validated native tool.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LegionToolSchemaDefinition {
    /// Tool kind.
    pub kind: LegionToolKind,
    /// Stable registry name.
    pub tool_name: String,
    /// Display-safe description label.
    pub description_label: String,
    /// JSON Schema for the tool input payload.
    pub input_schema: Value,
    /// Schema version.
    pub schema_version: u16,
}

impl LegionToolSchemaDefinition {
    fn new(kind: LegionToolKind, input_schema: Value) -> Self {
        Self {
            kind,
            tool_name: kind.tool_name().to_string(),
            description_label: kind.description_label().to_string(),
            input_schema,
            schema_version: 1,
        }
    }
}

/// Canonical feedback categories for invalid or retryable tool calls.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum LegionToolCallFeedbackKind {
    /// The selected tool was not recognized.
    UnknownTool,
    /// The tool arguments failed schema validation.
    InvalidArguments,
    /// The tool call was outside the selected scope.
    ScopeDenied,
    /// Policy or broker metadata denied the tool call.
    PolicyDenied,
    /// Runtime dispatch failed after the tool call was accepted.
    RuntimeFailure,
}

impl LegionToolCallFeedbackKind {
    /// Stable label used when routing feedback back to the model.
    pub const fn code_label(self) -> &'static str {
        match self {
            Self::UnknownTool => "tool_call.unknown_tool",
            Self::InvalidArguments => "tool_call.invalid_arguments",
            Self::ScopeDenied => "tool_call.scope_denied",
            Self::PolicyDenied => "tool_call.policy_denied",
            Self::RuntimeFailure => "tool_call.runtime_failure",
        }
    }

    /// Whether the model should be allowed to retry after seeing this feedback.
    pub const fn retryable(self) -> bool {
        matches!(self, Self::InvalidArguments)
    }
}

/// Structured feedback surfaced to the model when a tool call is invalid.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LegionToolCallFeedback {
    /// Tool that was attempted.
    pub tool: LegionToolKind,
    /// Feedback classification.
    pub kind: LegionToolCallFeedbackKind,
    /// Display-safe reason label.
    pub detail_label: String,
    /// Target path, if the invalid tool call was path-scoped.
    pub target_path: Option<String>,
    /// Whether the model should attempt a corrected retry.
    pub retryable: bool,
    /// Feedback schema version.
    pub schema_version: u16,
}

impl LegionToolCallFeedback {
    /// Builds structured tool-call feedback with the canonical retryability policy.
    pub fn new(
        tool: LegionToolKind,
        kind: LegionToolCallFeedbackKind,
        detail_label: impl Into<String>,
        target_path: Option<String>,
    ) -> Self {
        Self {
            tool,
            kind,
            detail_label: detail_label.into(),
            target_path,
            retryable: kind.retryable(),
            schema_version: 1,
        }
    }
}

/// Builds structured feedback for a model-visible invalid tool call.
pub fn delegated_task_tool_call_feedback(
    tool: LegionToolKind,
    kind: LegionToolCallFeedbackKind,
    detail_label: impl Into<String>,
    target_path: Option<String>,
) -> LegionToolCallFeedback {
    LegionToolCallFeedback::new(tool, kind, detail_label, target_path)
}

/// Validates a structured invalid-tool-call feedback record.
pub fn validate_tool_call_feedback(feedback: &LegionToolCallFeedback) -> Result<(), String> {
    if feedback.detail_label.trim().is_empty() {
        return Err(format!(
            "tool {} feedback requires a detail label",
            feedback.tool.tool_name()
        ));
    }
    if feedback.schema_version == 0 {
        return Err(format!(
            "tool {} feedback requires a non-zero schema version",
            feedback.tool.tool_name()
        ));
    }
    if feedback.kind.retryable() && !feedback.retryable {
        return Err(format!(
            "tool {} feedback kind {:?} must be retryable",
            feedback.tool.tool_name(),
            feedback.kind
        ));
    }
    if !feedback.kind.retryable() && feedback.retryable {
        return Err(format!(
            "tool {} feedback kind {:?} must not be retryable",
            feedback.tool.tool_name(),
            feedback.kind
        ));
    }
    Ok(())
}

/// Returns the native schema-validated tool registry.
pub fn tool_schema_definitions() -> Vec<LegionToolSchemaDefinition> {
    vec![
        LegionToolSchemaDefinition::new(
            LegionToolKind::Read,
            json!({
                "type": "object",
                "additionalProperties": false,
                "properties": {
                    "path": {"type": "string"},
                    "start_line": {"type": "integer", "minimum": 1},
                    "end_line": {"type": "integer", "minimum": 1},
                    "max_bytes": {"type": "integer", "minimum": 1}
                },
                "required": ["path"]
            }),
        ),
        LegionToolSchemaDefinition::new(
            LegionToolKind::Grep,
            json!({
                "type": "object",
                "additionalProperties": false,
                "properties": {
                    "pattern": {"type": "string"},
                    "path": {"type": ["string", "null"]},
                    "file_glob": {"type": ["string", "null"]},
                    "limit": {"type": ["integer", "null"], "minimum": 1}
                },
                "required": ["pattern"]
            }),
        ),
        LegionToolSchemaDefinition::new(
            LegionToolKind::Glob,
            json!({
                "type": "object",
                "additionalProperties": false,
                "properties": {
                    "pattern": {"type": "string"},
                    "path": {"type": ["string", "null"]},
                    "limit": {"type": ["integer", "null"], "minimum": 1}
                },
                "required": ["pattern"]
            }),
        ),
        LegionToolSchemaDefinition::new(
            LegionToolKind::Outline,
            json!({
                "type": "object",
                "additionalProperties": false,
                "properties": {
                    "path": {"type": "string"},
                    "max_symbols": {"type": ["integer", "null"], "minimum": 1}
                },
                "required": ["path"]
            }),
        ),
        LegionToolSchemaDefinition::new(
            LegionToolKind::EditAsProposal,
            json!({
                "type": "object",
                "additionalProperties": false,
                "properties": {
                    "path": {"type": "string"},
                    "replacement": {"type": "string"},
                    "start_line": {"type": ["integer", "null"], "minimum": 1},
                    "end_line": {"type": ["integer", "null"], "minimum": 1},
                    "proposal_title": {"type": ["string", "null"]},
                    "proposal_reason": {"type": ["string", "null"]}
                },
                "required": ["path", "replacement"]
            }),
        ),
        LegionToolSchemaDefinition::new(
            LegionToolKind::TerminalCommand,
            json!({
                "type": "object",
                "additionalProperties": false,
                "properties": {
                    "command": {"type": "string"},
                    "workdir": {"type": ["string", "null"]},
                    "timeout_seconds": {"type": ["integer", "null"], "minimum": 1}
                },
                "required": ["command"]
            }),
        ),
        LegionToolSchemaDefinition::new(
            LegionToolKind::McpPassthrough,
            json!({
                "type": "object",
                "additionalProperties": false,
                "properties": {
                    "server_id": {"type": "string"},
                    "tool_name": {"type": "string"},
                    "arguments": {"type": "object"}
                },
                "required": ["server_id", "tool_name", "arguments"]
            }),
        ),
    ]
}

/// Validates a schema-validated tool definition.
pub fn validate_tool_schema_definition(tool: &LegionToolSchemaDefinition) -> Result<(), String> {
    if tool.tool_name.trim().is_empty() {
        return Err("tool name must not be empty".to_string());
    }
    if tool.description_label.trim().is_empty() {
        return Err(format!(
            "tool {} requires a description label",
            tool.tool_name
        ));
    }
    if tool.schema_version == 0 {
        return Err(format!(
            "tool {} requires a non-zero schema version",
            tool.tool_name
        ));
    }

    let schema = tool
        .input_schema
        .as_object()
        .ok_or_else(|| format!("tool {} schema must be a JSON object", tool.tool_name))?;

    match schema.get("type").and_then(Value::as_str) {
        Some("object") => {}
        other => {
            return Err(format!(
                "tool {} schema must declare type object, found {other:?}",
                tool.tool_name
            ));
        }
    }

    let properties = schema
        .get("properties")
        .and_then(Value::as_object)
        .ok_or_else(|| format!("tool {} schema requires object properties", tool.tool_name))?;
    let required = schema
        .get("required")
        .and_then(Value::as_array)
        .ok_or_else(|| format!("tool {} schema requires a required array", tool.tool_name))?;

    let mut required_fields = Vec::with_capacity(required.len());
    for entry in required {
        required_fields.push(
            entry.as_str().ok_or_else(|| {
                format!("tool {} required entries must be strings", tool.tool_name)
            })?,
        );
    }

    let expected_required = tool.kind.required_fields();
    if required_fields != expected_required {
        return Err(format!(
            "tool {} required fields mismatch: expected {:?}, found {:?}",
            tool.tool_name, expected_required, required_fields
        ));
    }

    for key in expected_required {
        if !properties.contains_key(*key) {
            return Err(format!("tool {} missing property {key}", tool.tool_name));
        }
    }

    match schema.get("additionalProperties") {
        Some(Value::Bool(false)) => {}
        other => {
            return Err(format!(
                "tool {} schema must disallow additional properties, found {other:?}",
                tool.tool_name
            ));
        }
    }

    Ok(())
}

/// A tool call invocation request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LegionToolCallInvocation {
    /// Which tool is being called.
    pub tool: LegionToolKind,
    /// The tool's input arguments (JSON).
    pub input: serde_json::Value,
    /// Unique ID for this invocation (from the model's tool_use block).
    pub tool_use_id: String,
}

/// Outcome of executing a tool call.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LegionToolCallOutcome {
    /// Tool executed successfully.
    Success {
        /// Tool output content.
        content: String,
        /// Whether the output was truncated to fit budget.
        truncated: bool,
        /// Whether redaction was applied.
        redaction_applied: bool,
    },
    /// Tool call was rejected before execution.
    Rejected(LegionToolCallFeedback),
}

/// Complete result of a tool call attempt.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LegionToolCallResult {
    /// The original invocation.
    pub invocation: LegionToolCallInvocation,
    /// What happened.
    pub outcome: LegionToolCallOutcome,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_contains_the_native_tool_set() {
        let registry = tool_schema_definitions();
        assert_eq!(registry.len(), 7);
        assert!(
            registry
                .iter()
                .all(|tool| validate_tool_schema_definition(tool).is_ok())
        );
    }

    #[test]
    fn read_schema_keeps_expected_required_fields() {
        let read = tool_schema_definitions()
            .into_iter()
            .find(|tool| tool.kind == LegionToolKind::Read)
            .expect("read tool present");

        let required = read
            .input_schema
            .get("required")
            .and_then(Value::as_array)
            .expect("required array");
        assert_eq!(required.len(), 1);
        assert_eq!(required[0].as_str(), Some("path"));
    }

    #[test]
    fn tool_call_invocation_serde_round_trip() {
        let inv = LegionToolCallInvocation {
            tool: LegionToolKind::Read,
            input: serde_json::json!({"path": "src/main.rs"}),
            tool_use_id: "tuid-1".to_string(),
        };
        let json = serde_json::to_string(&inv).unwrap();
        let decoded: LegionToolCallInvocation = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.tool, LegionToolKind::Read);
        assert_eq!(decoded.tool_use_id, "tuid-1");
    }

    #[test]
    fn tool_call_result_success_and_rejected_variants_are_serializable() {
        let inv = LegionToolCallInvocation {
            tool: LegionToolKind::Grep,
            input: serde_json::json!({"pattern": "fn main"}),
            tool_use_id: "tuid-2".to_string(),
        };
        let success = LegionToolCallResult {
            invocation: inv.clone(),
            outcome: LegionToolCallOutcome::Success {
                content: "found 3 matches".to_string(),
                truncated: false,
                redaction_applied: false,
            },
        };
        let json = serde_json::to_string(&success).unwrap();
        let decoded: LegionToolCallResult = serde_json::from_str(&json).unwrap();
        assert!(matches!(
            decoded.outcome,
            LegionToolCallOutcome::Success { .. }
        ));

        let feedback = LegionToolCallFeedback::new(
            LegionToolKind::Grep,
            LegionToolCallFeedbackKind::ScopeDenied,
            "scope_denied",
            None,
        );
        let rejected = LegionToolCallResult {
            invocation: inv,
            outcome: LegionToolCallOutcome::Rejected(feedback),
        };
        let json = serde_json::to_string(&rejected).unwrap();
        let decoded: LegionToolCallResult = serde_json::from_str(&json).unwrap();
        assert!(matches!(
            decoded.outcome,
            LegionToolCallOutcome::Rejected(_)
        ));
    }

    #[test]
    fn invalid_tool_call_feedback_is_structured_and_retry_policy_is_bound() {
        let feedback = delegated_task_tool_call_feedback(
            LegionToolKind::Read,
            LegionToolCallFeedbackKind::InvalidArguments,
            "tool_call.invalid_arguments",
            Some("/workspace/project/src/main.rs".to_string()),
        );

        assert_eq!(feedback.tool, LegionToolKind::Read);
        assert_eq!(feedback.kind, LegionToolCallFeedbackKind::InvalidArguments);
        assert!(feedback.retryable);
        assert_eq!(feedback.kind.code_label(), "tool_call.invalid_arguments");
        validate_tool_call_feedback(&feedback).expect("feedback validates");

        let denied = delegated_task_tool_call_feedback(
            LegionToolKind::Read,
            LegionToolCallFeedbackKind::ScopeDenied,
            "tool_call.scope_denied",
            Some("/workspace/project/src/lib.rs".to_string()),
        );
        assert!(!denied.retryable);
        validate_tool_call_feedback(&denied).expect("scope denial feedback validates");
    }
}
