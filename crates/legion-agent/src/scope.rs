//! Scope enforcement helpers for delegated task tool calls.

use std::path::Path;

use legion_protocol::{DelegatedTaskScope, DelegatedTaskScopeTargetKind, LegionToolKind};

use crate::AgentError;

/// Validates that a tool invocation stays inside the selected delegated-task scope.
pub fn validate_delegated_task_tool_call(
    scope: &DelegatedTaskScope,
    tool: LegionToolKind,
    target_path: Option<&Path>,
) -> Result<(), AgentError> {
    if !scope.allows_tool(tool) {
        return Err(AgentError::DelegatedTaskScopeDenied {
            tool,
            target_path: target_path.map(|path| path.to_string_lossy().to_string()),
            reason: format!("tool {tool:?} is not allowed by the selected scope"),
        });
    }

    if !scope.target_is_within_scope(target_path) {
        return Err(AgentError::DelegatedTaskScopeDenied {
            tool,
            target_path: target_path.map(|path| path.to_string_lossy().to_string()),
            reason: format!(
                "target {:?} is outside the selected {:?} scope",
                target_path.map(|path| path.to_string_lossy().to_string()),
                scope.target_kind
            ),
        });
    }

    if let Some(target_path) = target_path {
        let candidate = legion_protocol::CanonicalPath(target_path.to_string_lossy().to_string());
        if scope.forbids_path(&candidate) {
            return Err(AgentError::DelegatedTaskScopeDenied {
                tool,
                target_path: Some(candidate.0),
                reason: "target path matches a forbidden-path entry".to_string(),
            });
        }
    } else if scope.target_kind != DelegatedTaskScopeTargetKind::Repo {
        return Err(AgentError::DelegatedTaskScopeDenied {
            tool,
            target_path: None,
            reason: format!("tool {tool:?} requires a concrete file or module target"),
        });
    }

    Ok(())
}

/// Converts a delegated-task scope denial into structured model feedback.
pub fn tool_call_feedback_for_scope_denial(
    error: &AgentError,
) -> Option<legion_protocol::tools::LegionToolCallFeedback> {
    match error {
        AgentError::DelegatedTaskScopeDenied {
            tool,
            target_path,
            reason,
        } => Some(legion_protocol::tools::delegated_task_tool_call_feedback(
            *tool,
            legion_protocol::tools::LegionToolCallFeedbackKind::ScopeDenied,
            reason.clone(),
            target_path.clone(),
        )),
        _ => None,
    }
}
