use std::{
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

use legion_desktop::{
    bridge::{DesktopAction, DesktopBridgeError, DesktopBridgeOutput, DesktopCommandBridge},
    workflow::{DesktopLaunchConfig, DesktopRuntime, DesktopWorkflowOutcome},
};
use legion_protocol::{
    AgentRunId, ProposalCancellationReason, ProposalId, ProposalLifecycleState,
    ProposalRejectionReason, ProposalRollbackReason, WorkspaceTrustState,
};
use legion_ui::{CommandDispatchIntent, DockMode, Shell};

static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

struct TempWorkspace {
    root: PathBuf,
}

impl TempWorkspace {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "legion-desktop-control-trust-bridge-{}-{}",
            std::process::id(),
            TEMP_COUNTER.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&root).expect("create temp workspace");
        Self { root }
    }

    fn path(&self) -> &Path {
        &self.root
    }

    fn write(&self, name: &str, content: &str) -> PathBuf {
        let path = self.root.join(name);
        fs::write(&path, content).expect("write workspace file");
        path
    }
}

impl Drop for TempWorkspace {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn runtime_with_file(file_name: &str) -> (TempWorkspace, DesktopRuntime) {
    let workspace = TempWorkspace::new();
    let target = workspace.write(file_name, "fn main() {}\n");
    let runtime = DesktopRuntime::open(DesktopLaunchConfig::new(
        workspace.path().to_path_buf(),
        Some(target.to_string_lossy().into_owned()),
    ))
    .expect("desktop runtime should open");
    (workspace, runtime)
}

fn start_proposal(runtime: &mut DesktopRuntime) -> (ProposalId, AgentRunId) {
    assert_eq!(
        runtime
            .handle_action(DesktopAction::SetProductMode {
                mode: DockMode::Assist
            })
            .expect("switch to assist"),
        DesktopWorkflowOutcome::ProductModeChanged {
            mode: DockMode::Assist
        }
    );
    let outcome = runtime
        .handle_action(DesktopAction::StartAiProposal {
            instruction_label: "add bridge guard".to_string(),
        })
        .expect("start assisted proposal");
    match outcome {
        DesktopWorkflowOutcome::AssistedAiUpdated {
            run_id,
            proposal_id: Some(proposal_id),
            status,
        } => {
            assert!(status.contains("created proposal"));
            (proposal_id, run_id)
        }
        other => panic!("unexpected proposal outcome: {other:?}"),
    }
}

#[test]
fn proposal_actions_translate_to_command_intents() {
    let (_workspace, mut runtime) = runtime_with_file("bridge-proposal.rs");
    let (proposal_id, _run_id) = start_proposal(&mut runtime);
    let snapshot = runtime.projection_snapshot();
    let bridge = DesktopCommandBridge::new();

    assert_eq!(
        bridge.translate(DesktopAction::PreviewProposal { proposal_id }, &snapshot),
        DesktopBridgeOutput::Intent(CommandDispatchIntent::PreviewProposal { proposal_id })
    );
    assert_eq!(
        bridge.translate(DesktopAction::ApproveProposal { proposal_id }, &snapshot),
        DesktopBridgeOutput::Intent(CommandDispatchIntent::ApproveProposal { proposal_id })
    );
    assert_eq!(
        bridge.translate(
            DesktopAction::RejectProposal {
                proposal_id,
                reason: ProposalRejectionReason::UserRejected,
            },
            &snapshot,
        ),
        DesktopBridgeOutput::Intent(CommandDispatchIntent::RejectProposal {
            proposal_id,
            reason: ProposalRejectionReason::UserRejected,
        })
    );
    assert_eq!(
        bridge.translate(DesktopAction::ApplyProposal { proposal_id }, &snapshot),
        DesktopBridgeOutput::Intent(CommandDispatchIntent::ApplyProposal { proposal_id })
    );
    assert_eq!(
        bridge.translate(
            DesktopAction::RollbackProposal {
                proposal_id,
                reason: ProposalRollbackReason::UserRequested,
            },
            &snapshot,
        ),
        DesktopBridgeOutput::Intent(CommandDispatchIntent::RollbackProposal {
            proposal_id,
            reason: ProposalRollbackReason::UserRequested,
        })
    );
    assert_eq!(
        bridge.translate(
            DesktopAction::CancelProposal {
                proposal_id,
                reason: ProposalCancellationReason::UserCancelled,
            },
            &snapshot,
        ),
        DesktopBridgeOutput::Intent(CommandDispatchIntent::CancelProposal {
            proposal_id,
            reason: ProposalCancellationReason::UserCancelled,
        })
    );
    assert_eq!(
        bridge.translate(
            DesktopAction::OpenProposalDetails { proposal_id },
            &snapshot,
        ),
        DesktopBridgeOutput::Intent(CommandDispatchIntent::OpenProposalDetails { proposal_id })
    );
}

#[test]
fn assisted_ai_actions_translate_to_command_intents() {
    let (_workspace, mut runtime) = runtime_with_file("bridge-ai.rs");
    let (_proposal_id, run_id) = start_proposal(&mut runtime);
    let snapshot = runtime.projection_snapshot();
    let bridge = DesktopCommandBridge::new();

    assert_eq!(
        bridge.translate(
            DesktopAction::StartAiExplain {
                instruction_label: " explain context ".to_string(),
            },
            &snapshot,
        ),
        DesktopBridgeOutput::Intent(CommandDispatchIntent::StartAiExplain {
            instruction_label: "explain context".to_string(),
        })
    );
    assert_eq!(
        bridge.translate(
            DesktopAction::StartAiProposal {
                instruction_label: " propose edit ".to_string(),
            },
            &snapshot,
        ),
        DesktopBridgeOutput::Intent(CommandDispatchIntent::StartAiProposal {
            instruction_label: "propose edit".to_string(),
        })
    );
    assert_eq!(
        bridge.translate(
            DesktopAction::CancelAiRun {
                run_id: run_id.clone(),
            },
            &snapshot,
        ),
        DesktopBridgeOutput::Intent(CommandDispatchIntent::CancelAiRun {
            run_id: run_id.clone(),
        })
    );
    assert_eq!(
        bridge.translate(
            DesktopAction::ReplayAiRun {
                run_id: run_id.clone(),
            },
            &snapshot,
        ),
        DesktopBridgeOutput::Intent(CommandDispatchIntent::ReplayAiRun {
            run_id: run_id.clone(),
        })
    );
    assert_eq!(
        bridge.translate(
            DesktopAction::InspectAiRun {
                run_id: run_id.clone(),
            },
            &snapshot,
        ),
        DesktopBridgeOutput::Intent(CommandDispatchIntent::InspectAiRun { run_id })
    );
}

#[test]
fn bridge_rejects_partial_assisted_ai_run_ids() {
    let (_workspace, mut runtime) = runtime_with_file("bridge-ai-partial.rs");
    let (_proposal_id, run_id) = start_proposal(&mut runtime);
    let snapshot = runtime.projection_snapshot();
    let bridge = DesktopCommandBridge::new();
    let partial_run_id = AgentRunId(
        run_id
            .0
            .trim_end_matches(|character: char| character.is_ascii_digit())
            .to_string(),
    );

    assert_ne!(partial_run_id, run_id);
    assert_eq!(
        bridge.translate(
            DesktopAction::InspectAiRun {
                run_id: partial_run_id.clone()
            },
            &snapshot,
        ),
        DesktopBridgeOutput::Error(DesktopBridgeError::UnknownAiRun {
            run_id: partial_run_id,
        })
    );
}

#[test]
fn bridge_rejects_missing_projection_inputs() {
    let snapshot = Shell::empty("bridge").projection_snapshot();
    let bridge = DesktopCommandBridge::new();

    assert_eq!(
        bridge.translate(
            DesktopAction::OpenProposalDetails {
                proposal_id: ProposalId(999)
            },
            &snapshot,
        ),
        DesktopBridgeOutput::Error(DesktopBridgeError::UnknownProposal {
            proposal_id: ProposalId(999)
        })
    );
    assert_eq!(
        bridge.translate(
            DesktopAction::StartAiExplain {
                instruction_label: "   ".to_string(),
            },
            &snapshot,
        ),
        DesktopBridgeOutput::Error(DesktopBridgeError::InvalidInstructionLabel)
    );
    assert_eq!(
        bridge.translate(
            DesktopAction::InspectAiRun {
                run_id: AgentRunId("missing-run".to_string())
            },
            &snapshot,
        ),
        DesktopBridgeOutput::Error(DesktopBridgeError::UnknownAiRun {
            run_id: AgentRunId("missing-run".to_string())
        })
    );
}

#[test]
fn workflow_status_maps_control_trust_outcomes() {
    let (_workspace, mut runtime) = runtime_with_file("workflow-status.rs");
    let (proposal_id, run_id) = start_proposal(&mut runtime);

    assert_eq!(
        runtime
            .handle_action(DesktopAction::OpenProposalDetails { proposal_id })
            .expect("open proposal details"),
        DesktopWorkflowOutcome::ProposalDetailsOpened(proposal_id)
    );

    match runtime
        .handle_action(DesktopAction::PreviewProposal { proposal_id })
        .expect("preview proposal")
    {
        DesktopWorkflowOutcome::ProposalLifecycleUpdated {
            proposal_id: preview_id,
            lifecycle_state,
            status,
        } => {
            assert_eq!(preview_id, proposal_id);
            assert_eq!(lifecycle_state, ProposalLifecycleState::Previewed);
            assert!(status.contains("previewed"));
        }
        other => panic!("unexpected preview outcome: {other:?}"),
    }

    match runtime
        .handle_action(DesktopAction::InspectAiRun { run_id })
        .expect("inspect run")
    {
        DesktopWorkflowOutcome::AssistedAiUpdated {
            proposal_id: Some(inspected_proposal),
            status,
            ..
        } => {
            assert_eq!(inspected_proposal, proposal_id);
            assert!(status.contains("inspected"));
        }
        other => panic!("unexpected inspect outcome: {other:?}"),
    }
}

#[test]
fn desktop_control_trust_bridge_preserves_projection_only_boundary() {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let bridge_source =
        fs::read_to_string(manifest_dir.join("src/bridge.rs")).expect("read bridge source");
    let view_source = fs::read_to_string(manifest_dir.join("src/view.rs")).expect("read view");

    for source in [bridge_source, view_source] {
        assert!(!source.contains("WorkspaceProposal"));
        assert!(!source.contains("ProviderRouter"));
        assert!(!source.contains("WorkspaceActor"));
        assert!(!source.contains("EditorEngine"));
    }
    let _ = WorkspaceTrustState::Trusted;
}
