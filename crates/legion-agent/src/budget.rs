//! Metadata-only delegated-loop budget usage summaries.

use legion_protocol::{
    DelegatedTaskLoopBudget, DelegatedTaskLoopStepKind, DelegatedTaskLoopStepRecord,
};

/// Derived usage counters for a delegated task loop budget.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DelegatedTaskBudgetUsage {
    /// Observed model-response turns.
    pub model_turns_used: u32,
    /// Configured model-turn limit.
    pub model_turn_limit: u32,
    /// Observed tool-call attempts.
    pub tool_calls_used: u32,
    /// Configured tool-call limit.
    pub tool_call_limit: u32,
    /// Observed rejected tool calls, used as retry-like pressure metadata.
    pub retries_used: u32,
    /// Configured consecutive-retry limit.
    pub retry_limit: u32,
    /// Observed total tool output bytes when available.
    pub output_bytes_used: u64,
    /// Configured total tool output byte limit.
    pub output_byte_limit: u64,
    /// Observed wall-clock milliseconds when available.
    pub wall_clock_ms_used: Option<u64>,
    /// Configured wall-clock millisecond limit.
    pub wall_clock_ms_limit: u64,
    /// Whether the loop recorded explicit budget exhaustion.
    pub exhausted: bool,
}

impl DelegatedTaskBudgetUsage {
    /// Display-safe status label for the budget row.
    pub fn status_label(&self) -> &'static str {
        if self.exhausted
            || self.model_turns_used >= self.model_turn_limit
            || self.tool_calls_used >= self.tool_call_limit
            || self.retries_used >= self.retry_limit
            || self.output_bytes_used >= self.output_byte_limit
            || (self.wall_clock_ms_limit > 0
                && self
                    .wall_clock_ms_used
                    .is_some_and(|used| used >= self.wall_clock_ms_limit))
        {
            "exhausted"
        } else {
            "within-budget"
        }
    }
}

/// Derives metadata-only budget usage from delegated-loop audit steps.
///
/// The current audit record does not retain raw tool output sizes or elapsed
/// time, so callers pass those optional aggregate counters separately.
pub fn derive_delegated_task_budget_usage(
    budget: &DelegatedTaskLoopBudget,
    audit_steps: &[DelegatedTaskLoopStepRecord],
    output_bytes_used: u64,
    wall_clock_ms_used: Option<u64>,
) -> DelegatedTaskBudgetUsage {
    let mut model_turns_used = 0u32;
    let mut tool_calls_used = 0u32;
    let mut retries_used = 0u32;
    let mut exhausted = false;

    for step in audit_steps {
        match step.kind {
            DelegatedTaskLoopStepKind::ModelResponse => {
                model_turns_used = model_turns_used.saturating_add(1);
            }
            DelegatedTaskLoopStepKind::ToolCallRequest => {
                tool_calls_used = tool_calls_used.saturating_add(1);
            }
            DelegatedTaskLoopStepKind::ToolCallRejected => {
                retries_used = retries_used.saturating_add(1);
            }
            DelegatedTaskLoopStepKind::BudgetExhausted => {
                exhausted = true;
            }
            DelegatedTaskLoopStepKind::ToolCallResult | DelegatedTaskLoopStepKind::Cancelled => {}
        }
    }

    DelegatedTaskBudgetUsage {
        model_turns_used,
        model_turn_limit: budget.max_model_turns,
        tool_calls_used,
        tool_call_limit: budget.max_tool_calls,
        retries_used,
        retry_limit: budget.max_consecutive_retries,
        output_bytes_used,
        output_byte_limit: budget.max_total_tool_output_bytes,
        wall_clock_ms_used,
        wall_clock_ms_limit: budget.wall_clock_limit_ms,
        exhausted,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn step(kind: DelegatedTaskLoopStepKind, index: u32) -> DelegatedTaskLoopStepRecord {
        DelegatedTaskLoopStepRecord {
            run_id: "run:budget".to_string(),
            step_index: index,
            kind,
            correlation_id: "correlation:budget".to_string(),
            causality_id: "causality:budget".to_string(),
            event_sequence: index,
            tool_name: None,
            allowed: None,
            reason: None,
        }
    }

    #[test]
    fn derives_budget_usage_from_audit_steps_without_payloads() {
        let budget = DelegatedTaskLoopBudget {
            max_model_turns: 5,
            max_tool_calls: 8,
            max_consecutive_retries: 3,
            max_tool_output_bytes: 1024,
            max_total_tool_output_bytes: 4096,
            wall_clock_limit_ms: 1000,
        };
        let usage = derive_delegated_task_budget_usage(
            &budget,
            &[
                step(DelegatedTaskLoopStepKind::ModelResponse, 1),
                step(DelegatedTaskLoopStepKind::ToolCallRequest, 2),
                step(DelegatedTaskLoopStepKind::ToolCallRejected, 3),
                step(DelegatedTaskLoopStepKind::ToolCallRequest, 4),
            ],
            128,
            Some(10),
        );

        assert_eq!(usage.model_turns_used, 1);
        assert_eq!(usage.tool_calls_used, 2);
        assert_eq!(usage.retries_used, 1);
        assert_eq!(usage.output_bytes_used, 128);
        assert_eq!(usage.status_label(), "within-budget");
    }

    #[test]
    fn budget_exhaustion_step_marks_usage_exhausted() {
        let usage = derive_delegated_task_budget_usage(
            &DelegatedTaskLoopBudget::default(),
            &[step(DelegatedTaskLoopStepKind::BudgetExhausted, 1)],
            0,
            None,
        );

        assert!(usage.exhausted);
        assert_eq!(usage.status_label(), "exhausted");
    }

    #[test]
    fn retry_limit_marks_usage_exhausted_without_explicit_budget_step() {
        let budget = DelegatedTaskLoopBudget {
            max_consecutive_retries: 2,
            ..DelegatedTaskLoopBudget::default()
        };
        let usage = derive_delegated_task_budget_usage(
            &budget,
            &[
                step(DelegatedTaskLoopStepKind::ToolCallRejected, 1),
                step(DelegatedTaskLoopStepKind::ToolCallRejected, 2),
            ],
            0,
            None,
        );

        assert_eq!(usage.retries_used, 2);
        assert_eq!(usage.status_label(), "exhausted");
    }
}
