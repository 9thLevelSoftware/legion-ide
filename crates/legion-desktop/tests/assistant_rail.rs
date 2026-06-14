use legion_desktop::view::{
    AssistantRailSegmentViewModel, assistant_rail_rows,
};
use legion_protocol::ProposalId;

#[test]
fn assistant_rail_rows_surface_apply_as_proposal_for_code_blocks() {
    let rows = assistant_rail_rows(
        &["before\n```rust\nfn demo() {}\n```\nafter".to_string()],
        Some(ProposalId(7)),
    );

    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].segments.len(), 3);
    assert!(matches!(&rows[0].segments[0], AssistantRailSegmentViewModel::Text(text) if text == "before\n"));
    assert!(matches!(
        &rows[0].segments[1],
        AssistantRailSegmentViewModel::CodeBlock(code_block)
            if code_block.language.as_deref() == Some("rust")
                && code_block.code.contains("fn demo() {}")
                && code_block.complete
                && code_block.apply_as_proposal_available
    ));
    assert!(matches!(&rows[0].segments[2], AssistantRailSegmentViewModel::Text(text) if text == "after"));
}
