//! Integration tests for LANG.09: rust-analyzer rename/code-action edits are
//! routed through the proposal lifecycle rather than written directly to disk.

use legion_app::language::workspace_edit_to_proposal_input;
use legion_protocol::{
    ProposalId, ProposalLifecycleState, ProposalPayload, WorkspaceEditSourceKind,
    convert_lsp_edit_to_workspace_proposal,
};

mod proposal_fixture;

#[test]
fn rust_analyzer_rename_edit_becomes_workspace_proposal() {
    let payload = proposal_fixture::rename_payload();

    let input = workspace_edit_to_proposal_input(
        payload,
        proposal_fixture::correlation(),
        ProposalId(1),
        proposal_fixture::principal(),
        proposal_fixture::rename_capability(),
        proposal_fixture::preconditions(),
        ProposalLifecycleState::Created,
        proposal_fixture::privacy_label(),
        proposal_fixture::preview(),
        proposal_fixture::created_at(),
        None,
    );

    let proposal = convert_lsp_edit_to_workspace_proposal(input)
        .expect("rename edit must convert to proposal without error");

    assert!(
        matches!(proposal.payload, ProposalPayload::WorkspaceEdit(_)),
        "rename edit payload must be ProposalPayload::WorkspaceEdit"
    );

    if let ProposalPayload::WorkspaceEdit(ref edit) = proposal.payload {
        assert_eq!(
            edit.source,
            WorkspaceEditSourceKind::LspRename,
            "source kind must round-trip as LspRename"
        );
    }
}

#[test]
fn rust_analyzer_code_action_edit_becomes_workspace_proposal() {
    let payload = proposal_fixture::code_action_payload();

    let input = workspace_edit_to_proposal_input(
        payload,
        proposal_fixture::correlation(),
        ProposalId(2),
        proposal_fixture::principal(),
        proposal_fixture::code_action_capability(),
        proposal_fixture::preconditions(),
        ProposalLifecycleState::Created,
        proposal_fixture::privacy_label(),
        proposal_fixture::preview(),
        proposal_fixture::created_at(),
        None,
    );

    let proposal = convert_lsp_edit_to_workspace_proposal(input)
        .expect("code-action edit must convert to proposal without error");

    assert!(
        matches!(proposal.payload, ProposalPayload::WorkspaceEdit(_)),
        "code-action edit payload must be ProposalPayload::WorkspaceEdit"
    );

    if let ProposalPayload::WorkspaceEdit(ref edit) = proposal.payload {
        assert_eq!(
            edit.source,
            WorkspaceEditSourceKind::LspCodeAction,
            "source kind must round-trip as LspCodeAction"
        );
    }
}
