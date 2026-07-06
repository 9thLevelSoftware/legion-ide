#![cfg(feature = "ai")]

use legion_desktop::bridge::{DesktopAction, DesktopBridgeOutput, DesktopCommandBridge};
use legion_desktop::view::{
    AssistantRailSegmentViewModel, assistant_rail_rows, rail_command_view_models,
};
use legion_protocol::{AssistantRailCommand, ProposalId, RailCommandCapability};
use legion_ui::{CommandDispatchIntent, Shell};

#[test]
fn assistant_rail_rows_surface_apply_as_proposal_for_code_blocks() {
    let rows = assistant_rail_rows(
        &["before\n```rust\nfn demo() {}\n```\nafter".to_string()],
        Some(ProposalId(7)),
    );

    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].segments.len(), 3);
    assert!(
        matches!(&rows[0].segments[0], AssistantRailSegmentViewModel::Text(text) if text == "before\n")
    );
    assert!(matches!(
        &rows[0].segments[1],
        AssistantRailSegmentViewModel::CodeBlock(code_block)
            if code_block.language.as_deref() == Some("rust")
                && code_block.code.contains("fn demo() {}")
                && code_block.complete
                && code_block.apply_as_proposal_available
                && code_block.proposal_id == Some(ProposalId(7))
    ));
    assert!(
        matches!(&rows[0].segments[2], AssistantRailSegmentViewModel::Text(text) if text == "after")
    );
}

#[test]
fn assistant_rail_rows_bind_proposal_to_every_complete_block() {
    // Every complete code block must receive its own unique proposal binding so
    // each can be applied independently. Block 0 gets the base ID, block 1 gets
    // base + 1, etc. (T3 multi-block binding change.)
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
    // Second block gets ProposalId(7 + 1) = ProposalId(8)
    assert_eq!(blocks[1].proposal_id, Some(ProposalId(8)));
    assert!(blocks[1].apply_as_proposal_available);
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

// ─── T2: Rail commands ──────────────────────────────────────────────────────

#[test]
fn rail_commands_enumerate_all_five() {
    let caps = legion_protocol::rail_command_capabilities();
    assert_eq!(caps.len(), 5, "exactly 5 rail commands must be defined");
    let commands: Vec<AssistantRailCommand> = caps.iter().map(|c| c.command).collect();
    assert!(commands.contains(&AssistantRailCommand::Explain));
    assert!(commands.contains(&AssistantRailCommand::Fix));
    assert!(commands.contains(&AssistantRailCommand::Test));
    assert!(commands.contains(&AssistantRailCommand::Doc));
    assert!(commands.contains(&AssistantRailCommand::Refactor));
}

#[test]
fn rail_command_dispatches_proposal_not_mutation() {
    let bridge = DesktopCommandBridge::new();
    let snapshot = Shell::empty("RailTest").projection_snapshot();

    let result = bridge.translate(
        DesktopAction::ExecuteRailCommand {
            command: AssistantRailCommand::Explain,
            selection: None,
        },
        &snapshot,
    );

    // Must dispatch a StartAiProposal intent, never a direct Insert/Replace/Delete.
    match result {
        DesktopBridgeOutput::Intent(CommandDispatchIntent::StartAiProposal {
            instruction_label,
            ..
        }) => {
            assert_eq!(instruction_label, "ai.rail.explain");
        }
        other => panic!("ExecuteRailCommand must dispatch StartAiProposal, got: {other:?}"),
    }
}

#[test]
fn rail_command_with_selection_forwards_selection_to_intent() {
    use legion_protocol::{ProtocolTextRange, TextCoordinate};

    let bridge = DesktopCommandBridge::new();
    let snapshot = Shell::empty("RailWithSel").projection_snapshot();

    let selection = ProtocolTextRange {
        start: TextCoordinate {
            line: 2,
            character: 0,
            byte_offset: Some(0),
            utf16_offset: Some(0),
        },
        end: TextCoordinate {
            line: 5,
            character: 10,
            byte_offset: Some(0),
            utf16_offset: Some(0),
        },
    };

    let result = bridge.translate(
        DesktopAction::ExecuteRailCommand {
            command: AssistantRailCommand::Fix,
            selection: Some(selection),
        },
        &snapshot,
    );

    match result {
        DesktopBridgeOutput::Intent(CommandDispatchIntent::StartAiProposal {
            instruction_label,
            selection: forwarded_selection,
        }) => {
            assert_eq!(instruction_label, "ai.rail.fix");
            assert_eq!(
                forwarded_selection,
                Some(selection),
                "selection must be forwarded from ExecuteRailCommand into StartAiProposal"
            );
        }
        other => panic!("ExecuteRailCommand must dispatch StartAiProposal, got: {other:?}"),
    }
}

#[test]
fn rail_command_view_models_reflect_capability_gates() {
    // Only the Explain capability is granted; Fix and others must be unavailable.
    let capabilities = vec![RailCommandCapability {
        command: AssistantRailCommand::Explain,
        capability_id: "ai.rail.explain".to_string(),
    }];

    let view_models = rail_command_view_models(&capabilities);
    assert_eq!(view_models.len(), 5);

    let explain = view_models
        .iter()
        .find(|vm| vm.command == AssistantRailCommand::Explain)
        .expect("Explain must be present");
    let fix = view_models
        .iter()
        .find(|vm| vm.command == AssistantRailCommand::Fix)
        .expect("Fix must be present");

    assert!(
        explain.available,
        "Explain is available when capability is granted"
    );
    assert!(
        !fix.available,
        "Fix is unavailable when capability is absent"
    );
}

#[test]
fn each_rail_command_has_stable_capability_id() {
    let caps = legion_protocol::rail_command_capabilities();
    let ids: Vec<&str> = caps.iter().map(|c| c.capability_id.as_str()).collect();
    assert!(
        ids.contains(&"ai.rail.explain"),
        "explain id must be stable"
    );
    assert!(ids.contains(&"ai.rail.fix"), "fix id must be stable");
    assert!(ids.contains(&"ai.rail.test"), "test id must be stable");
    assert!(ids.contains(&"ai.rail.doc"), "doc id must be stable");
    assert!(
        ids.contains(&"ai.rail.refactor"),
        "refactor id must be stable"
    );
}

#[test]
fn rail_command_without_selection_is_valid() {
    let bridge = DesktopCommandBridge::new();
    let snapshot = Shell::empty("RailNoSel").projection_snapshot();

    // All 5 commands work without a selection (cursor context is used).
    for command in [
        AssistantRailCommand::Explain,
        AssistantRailCommand::Fix,
        AssistantRailCommand::Test,
        AssistantRailCommand::Doc,
        AssistantRailCommand::Refactor,
    ] {
        let result = bridge.translate(
            DesktopAction::ExecuteRailCommand {
                command,
                selection: None,
            },
            &snapshot,
        );
        assert!(
            matches!(
                result,
                DesktopBridgeOutput::Intent(CommandDispatchIntent::StartAiProposal { .. })
            ),
            "ExecuteRailCommand({command:?}) with no selection must dispatch proposal"
        );
    }
}
