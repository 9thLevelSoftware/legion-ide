use legion_agent::tools::native_tool_registry;
use legion_protocol::tools::{LegionToolKind, validate_tool_schema_definition};

#[test]
fn native_tool_registry_contains_schema_validated_tool_set() {
    let registry = native_tool_registry();

    assert_eq!(registry.len(), 7);
    assert_eq!(
        registry.iter().map(|tool| tool.kind).collect::<Vec<_>>(),
        vec![
            LegionToolKind::Read,
            LegionToolKind::Grep,
            LegionToolKind::Glob,
            LegionToolKind::Outline,
            LegionToolKind::EditAsProposal,
            LegionToolKind::TerminalCommand,
            LegionToolKind::McpPassthrough,
        ]
    );
    assert!(
        registry
            .iter()
            .all(|tool| validate_tool_schema_definition(tool).is_ok())
    );
}

#[test]
fn each_tool_schema_declares_the_expected_required_fields() {
    let registry = native_tool_registry();

    let expected = [
        (LegionToolKind::Read, vec!["path"]),
        (LegionToolKind::Grep, vec!["pattern"]),
        (LegionToolKind::Glob, vec!["pattern"]),
        (LegionToolKind::Outline, vec!["path"]),
        (LegionToolKind::EditAsProposal, vec!["path", "replacement"]),
        (LegionToolKind::TerminalCommand, vec!["command"]),
        (
            LegionToolKind::McpPassthrough,
            vec!["server_id", "tool_name", "arguments"],
        ),
    ];

    for (kind, required_fields) in expected {
        let tool = registry
            .iter()
            .find(|tool| tool.kind == kind)
            .expect("tool kind present");
        let required = tool
            .input_schema
            .get("required")
            .and_then(|value| value.as_array())
            .expect("tool schema has required array");
        let required = required
            .iter()
            .map(|value| value.as_str().expect("required field is a string"))
            .collect::<Vec<_>>();
        assert_eq!(required, required_fields);
        assert_eq!(
            tool.input_schema.get("type").and_then(|v| v.as_str()),
            Some("object")
        );
    }
}
