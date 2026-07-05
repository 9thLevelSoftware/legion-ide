use legion_agent::{
    AgentError, tool_call_feedback_for_scope_denial, validate_delegated_task_tool_call,
};
use legion_protocol::tools::LegionToolCallFeedbackKind;
use legion_protocol::{
    CanonicalPath, DelegatedTaskRiskTolerance, DelegatedTaskScope, DelegatedTaskScopeTargetKind,
    LegionToolKind,
};
use std::path::Path;

fn file_scope() -> DelegatedTaskScope {
    DelegatedTaskScope {
        target_kind: DelegatedTaskScopeTargetKind::File,
        workspace_root: CanonicalPath("/workspace/project".to_string()),
        target_path: Some(CanonicalPath("/workspace/project/src/main.rs".to_string())),
        risk_tolerance: DelegatedTaskRiskTolerance::Conservative,
        allowed_tools: vec![LegionToolKind::Read, LegionToolKind::EditAsProposal],
        forbidden_paths: vec![CanonicalPath(
            "/workspace/project/src/main.rs.generated".to_string(),
        )],
        schema_version: 1,
    }
}

#[test]
fn tool_calls_must_stay_within_the_selected_scope() {
    let scope = file_scope();

    validate_delegated_task_tool_call(
        &scope,
        LegionToolKind::Read,
        Some(Path::new("/workspace/project/src/main.rs")),
    )
    .expect("selected file should be allowed");

    let err = validate_delegated_task_tool_call(
        &scope,
        LegionToolKind::Read,
        Some(Path::new("/workspace/project/src/lib.rs")),
    )
    .expect_err("other files must be rejected");

    assert!(matches!(err, AgentError::DelegatedTaskScopeDenied { .. }));
}

#[test]
fn scope_denials_can_be_reported_as_structured_feedback() {
    let scope = file_scope();

    let err = validate_delegated_task_tool_call(
        &scope,
        LegionToolKind::Read,
        Some(Path::new("/workspace/project/src/lib.rs")),
    )
    .expect_err("other files must be rejected");

    let feedback = tool_call_feedback_for_scope_denial(&err).expect("scope feedback");
    assert_eq!(feedback.tool, LegionToolKind::Read);
    assert_eq!(feedback.kind, LegionToolCallFeedbackKind::ScopeDenied);
    assert!(!feedback.retryable);
    assert!(feedback.detail_label.contains("outside the selected"));
    assert!(
        feedback
            .target_path
            .as_deref()
            .is_some_and(|path| path.ends_with("src/lib.rs"))
    );
}
