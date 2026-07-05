use legion_agent::comm::{AgentCommTag, parse_agent_comm_line};

#[test]
fn agent_comm_tag_set_is_explicit_and_stable() {
    assert_eq!(
        AgentCommTag::ALL,
        [
            AgentCommTag::Plan,
            AgentCommTag::Write,
            AgentCommTag::Test,
            AgentCommTag::Review,
            AgentCommTag::Error,
            AgentCommTag::Approval,
            AgentCommTag::Complete,
        ]
    );
}

#[test]
fn agent_comm_lines_require_a_tag_prefix() {
    assert!(
        parse_agent_comm_line("Planner → Backend Team: Assigned checkout session endpoint")
            .is_none()
    );
}

#[test]
fn agent_comm_lines_parse_the_documented_format() {
    let parsed = parse_agent_comm_line(
        "[12:04:11] [PLAN] Planner → Backend Team: Assigned checkout session endpoint",
    )
    .expect("tagged comm line should parse");

    assert_eq!(parsed.timestamp, "12:04:11");
    assert_eq!(parsed.tag, AgentCommTag::Plan);
    assert_eq!(parsed.actor, "Planner → Backend Team");
    assert_eq!(parsed.message, "Assigned checkout session endpoint");
}
