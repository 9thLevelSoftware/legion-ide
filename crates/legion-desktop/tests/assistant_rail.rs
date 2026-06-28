#![cfg(feature = "ai")]

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
                && code_block.proposal_id == Some(ProposalId(7))
    ));
    assert!(matches!(&rows[0].segments[2], AssistantRailSegmentViewModel::Text(text) if text == "after"));
}

#[test]
fn assistant_rail_rows_bind_proposal_to_a_single_block() {
    // The matched proposal binds to exactly one code block; a second complete
    // block in the same response must be unbound so the same proposal cannot be
    // applied from multiple places.
    let rows = assistant_rail_rows(
        &["```rust\nfn a() {}\n```\n```rust\nfn b() {}\n```".to_string()],
        Some(ProposalId(7)),
    );

    let blocks: Vec<_> = rows[0]
        .segments
        .iter()
        .filter_map(|segment| match segment {
            AssistantRailSegmentViewModel::CodeBlock(code_block) => Some(code_block),
            AssistantRailSegmentViewModel::Text(_) => None,
        })
        .collect();
    assert_eq!(blocks.len(), 2);
    assert_eq!(blocks[0].proposal_id, Some(ProposalId(7)));
    assert!(blocks[0].apply_as_proposal_available);
    assert_eq!(blocks[1].proposal_id, None);
    assert!(!blocks[1].apply_as_proposal_available);
}

#[test]
fn assistant_rail_rows_without_proposal_are_not_applyable() {
    let rows = assistant_rail_rows(&["```rust\nfn demo() {}\n```".to_string()], None);
    assert!(matches!(
        &rows[0].segments[0],
        AssistantRailSegmentViewModel::CodeBlock(code_block)
            if code_block.complete
                && code_block.proposal_id.is_none()
                && !code_block.apply_as_proposal_available
    ));
}
