//! Budget caps and audit-step DTOs for the delegated task execution loop.

use serde::{Deserialize, Serialize};

/// Budget caps for a delegated task loop.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DelegatedTaskLoopBudget {
    /// Maximum model turns before termination.
    pub max_model_turns: u32,
    /// Maximum total tool calls before termination.
    pub max_tool_calls: u32,
    /// Maximum consecutive retries (InvalidArguments) before termination.
    pub max_consecutive_retries: u32,
    /// Maximum bytes in a single tool output before truncation.
    pub max_tool_output_bytes: u64,
    /// Maximum total tool output bytes before termination.
    pub max_total_tool_output_bytes: u64,
    /// Wall clock limit in milliseconds (0 = no limit).
    pub wall_clock_limit_ms: u64,
}

impl Default for DelegatedTaskLoopBudget {
    fn default() -> Self {
        Self {
            max_model_turns: 50,
            max_tool_calls: 200,
            max_consecutive_retries: 3,
            max_tool_output_bytes: 100_000,
            max_total_tool_output_bytes: 5_000_000,
            wall_clock_limit_ms: 0,
        }
    }
}

/// What kind of event a loop step represents.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DelegatedTaskLoopStepKind {
    /// Model produced a response.
    ModelResponse,
    /// Tool call request (before execution).
    ToolCallRequest,
    /// Tool call result (after execution).
    ToolCallResult,
    /// Tool call rejected (scope/policy/validation).
    ToolCallRejected,
    /// Budget exhausted.
    BudgetExhausted,
    /// Loop terminated by cancellation.
    Cancelled,
}

/// One auditable step in the loop.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DelegatedTaskLoopStepRecord {
    /// Run ID for the delegated task.
    pub run_id: String,
    /// Monotonically increasing step index within this run.
    pub step_index: u32,
    /// What happened.
    pub kind: DelegatedTaskLoopStepKind,
    /// Correlation ID (constant for the entire run).
    pub correlation_id: String,
    /// Causality ID (pairs request + outcome for a single tool call).
    pub causality_id: String,
    /// Monotonically increasing event sequence within this run.
    pub event_sequence: u32,
    /// Tool name, if this step involves a tool call.
    pub tool_name: Option<String>,
    /// Whether the tool call was allowed.
    pub allowed: Option<bool>,
    /// Human-readable reason label.
    pub reason: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_budget_produces_sane_values() {
        let budget = DelegatedTaskLoopBudget::default();
        assert_eq!(budget.max_model_turns, 50);
        assert_eq!(budget.max_tool_calls, 200);
        assert_eq!(budget.max_consecutive_retries, 3);
        assert_eq!(budget.max_tool_output_bytes, 100_000);
        assert_eq!(budget.max_total_tool_output_bytes, 5_000_000);
        assert_eq!(budget.wall_clock_limit_ms, 0);
    }

    #[test]
    fn budget_serde_round_trip() {
        let budget = DelegatedTaskLoopBudget {
            max_model_turns: 10,
            max_tool_calls: 50,
            max_consecutive_retries: 2,
            max_tool_output_bytes: 4096,
            max_total_tool_output_bytes: 1_000_000,
            wall_clock_limit_ms: 30_000,
        };
        let json = serde_json::to_string(&budget).unwrap();
        let decoded: DelegatedTaskLoopBudget = serde_json::from_str(&json).unwrap();
        assert_eq!(budget, decoded);
    }

    #[test]
    fn step_record_serde_round_trip() {
        let step = DelegatedTaskLoopStepRecord {
            run_id: "run-1".to_string(),
            step_index: 0,
            kind: DelegatedTaskLoopStepKind::ToolCallRequest,
            correlation_id: "corr-1".to_string(),
            causality_id: "cause-1".to_string(),
            event_sequence: 0,
            tool_name: Some("Read".to_string()),
            allowed: None,
            reason: None,
        };
        let json = serde_json::to_string(&step).unwrap();
        let decoded: DelegatedTaskLoopStepRecord = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.run_id, "run-1");
        assert_eq!(decoded.step_index, 0);
        assert_eq!(decoded.kind, DelegatedTaskLoopStepKind::ToolCallRequest);
        assert_eq!(decoded.tool_name, Some("Read".to_string()));
    }

    #[test]
    fn all_step_kinds_are_serializable() {
        let kinds = [
            DelegatedTaskLoopStepKind::ModelResponse,
            DelegatedTaskLoopStepKind::ToolCallRequest,
            DelegatedTaskLoopStepKind::ToolCallResult,
            DelegatedTaskLoopStepKind::ToolCallRejected,
            DelegatedTaskLoopStepKind::BudgetExhausted,
            DelegatedTaskLoopStepKind::Cancelled,
        ];
        for kind in kinds {
            let json = serde_json::to_string(&kind).unwrap();
            let decoded: DelegatedTaskLoopStepKind = serde_json::from_str(&json).unwrap();
            assert_eq!(kind, decoded);
        }
    }
}
