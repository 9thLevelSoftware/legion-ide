use legion_desktop::view::{DesktopScopePickerViewModel, ScopeRiskTolerance, ScopeTargetKind};
use legion_protocol::{
    CanonicalPath, DelegatedTaskRiskTolerance, DelegatedTaskScope, DelegatedTaskScopeTargetKind,
    LegionToolKind,
};

#[test]
fn scope_picker_view_model_round_trips_the_structured_scope() {
    let scope = DelegatedTaskScope {
        target_kind: DelegatedTaskScopeTargetKind::Repo,
        workspace_root: CanonicalPath("/workspace/project".to_string()),
        target_path: None,
        risk_tolerance: DelegatedTaskRiskTolerance::Aggressive,
        allowed_tools: vec![
            LegionToolKind::Read,
            LegionToolKind::Glob,
            LegionToolKind::Outline,
        ],
        forbidden_paths: vec![CanonicalPath("/workspace/project/secret".to_string())],
        schema_version: 1,
    };

    let model = DesktopScopePickerViewModel::from(scope.clone());
    assert_eq!(model.target_kind, ScopeTargetKind::Repo);
    assert_eq!(model.risk_tolerance, ScopeRiskTolerance::Aggressive);
    assert_eq!(model.allowed_tools.len(), 3);

    let round_trip: DelegatedTaskScope = model.into();
    assert_eq!(round_trip, scope);
}
