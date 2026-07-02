use legion_desktop::view::agent_comm::agent_comm_rows;

#[test]
fn agent_comm_rows_drop_freeform_messages_without_tags() {
    let rows = agent_comm_rows(&[
        "[12:04:11] [PLAN] Planner → Backend Team: Assigned checkout session endpoint".to_string(),
        "Planner → Backend Team: assigned checkout session endpoint".to_string(),
    ]);

    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].tag_label, "PLAN");
    assert_eq!(rows[0].actor, "Planner → Backend Team");
    assert_eq!(rows[0].message, "Assigned checkout session endpoint");
}
