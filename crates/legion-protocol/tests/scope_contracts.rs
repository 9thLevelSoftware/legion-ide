use legion_protocol::{
    CanonicalPath, DelegatedTaskRiskTolerance, DelegatedTaskScope, DelegatedTaskScopeTargetKind,
    LegionToolKind,
};

#[test]
fn scope_contracts_round_trip_and_enforce_forbidden_paths() {
    let scope = DelegatedTaskScope {
        target_kind: DelegatedTaskScopeTargetKind::Module,
        workspace_root: CanonicalPath("/workspace/project".to_string()),
        target_path: Some(CanonicalPath("/workspace/project/src".to_string())),
        risk_tolerance: DelegatedTaskRiskTolerance::Balanced,
        allowed_tools: vec![LegionToolKind::Read, LegionToolKind::Grep],
        forbidden_paths: vec![CanonicalPath("/workspace/project/src/secret".to_string())],
        schema_version: 1,
    };

    assert!(scope.allows_tool(LegionToolKind::Read));
    assert!(!scope.allows_tool(LegionToolKind::TerminalCommand));
    assert!(scope.forbids_path(&CanonicalPath(
        "/workspace/project/src/secret/config.rs".to_string(),
    )));

    let json = serde_json::to_value(&scope).expect("scope must serialize");
    let round_trip: DelegatedTaskScope =
        serde_json::from_value(json).expect("scope must deserialize");
    assert_eq!(round_trip, scope);
}
