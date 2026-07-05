//! Agent-facing access to the schema-validated native tool registry.

pub use legion_protocol::tools::{
    LegionToolCallFeedback, LegionToolCallFeedbackKind, LegionToolKind, LegionToolSchemaDefinition,
    delegated_task_tool_call_feedback, validate_tool_call_feedback,
    validate_tool_schema_definition,
};

/// Returns the native tool registry as owned schema definitions.
pub fn native_tool_registry() -> Vec<LegionToolSchemaDefinition> {
    legion_protocol::tools::tool_schema_definitions()
}
