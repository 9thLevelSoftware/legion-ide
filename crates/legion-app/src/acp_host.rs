//! Optional ACP host command used by delegated-task proposal generation.

use std::path::{Path, PathBuf};
use std::process::Command;

#[cfg(feature = "ai")]
use legion_agent::{DelegatedTaskProposalGenerator, DelegatedTaskProposalInput};
#[cfg(feature = "ai")]
use legion_protocol::{
    CapabilityId, CausalityId, CorrelationId, PrincipalId, ProposalId, TimestampMillis,
};

#[cfg(feature = "ai")]
use crate::{AppCompositionError, trust_reference};

/// ACP host command configured by the app.
#[derive(Debug, Clone)]
pub(crate) struct AcpHostCommand {
    pub(crate) program: PathBuf,
    args: Vec<String>,
}

impl AcpHostCommand {
    pub(crate) fn new(program: impl Into<PathBuf>, args: Vec<String>) -> Self {
        Self {
            program: program.into(),
            args,
        }
    }

    pub(crate) fn run(
        &self,
        sandbox_path: &Path,
        target_path: &Path,
        plan_id: &str,
    ) -> std::io::Result<std::process::Output> {
        let mut command = Command::new(&self.program);
        command.args(&self.args);
        command.current_dir(sandbox_path);
        command.env("LEGION_ACP_PLAN_ID", plan_id);
        command.env("LEGION_ACP_SANDBOX_PATH", sandbox_path);
        command.env("LEGION_ACP_TARGET_PATH", target_path);
        command.env(
            "LEGION_ACP_TARGET_DIR",
            target_path.parent().unwrap_or(sandbox_path),
        );
        command.env(
            "LEGION_ACP_TARGET_FILE",
            target_path
                .file_name()
                .and_then(|value| value.to_str())
                .unwrap_or("proposal.txt"),
        );
        command.output()
    }
}

#[cfg(feature = "ai")]
pub(crate) fn run_acp_host_proposal(
    command: &AcpHostCommand,
    sandbox_path: &Path,
    target_file: &Path,
    task_id: &str,
    correlation_id: CorrelationId,
    causality_id: CausalityId,
) -> Result<legion_protocol::AssistedAiEditProposalOutput, AppCompositionError> {
    let Some(parent) = target_file.parent() else {
        return Err(AppCompositionError::AiRuntime(
            "ACP proposal target has no parent directory".to_string(),
        ));
    };
    std::fs::create_dir_all(parent).map_err(|error| {
        AppCompositionError::AiRuntime(format!("failed to prepare ACP proposal target: {error}"))
    })?;
    std::fs::write(
        target_file,
        format!("delegated-task-proposal\ntask_id={task_id}\n"),
    )
    .map_err(|error| {
        AppCompositionError::AiRuntime(format!("failed to seed ACP proposal target: {error}"))
    })?;

    let output = command
        .run(sandbox_path, target_file, task_id)
        .map_err(|error| {
            AppCompositionError::AiRuntime(format!("ACP host command failed to start: {error}"))
        })?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(AppCompositionError::AiRuntime(format!(
            "ACP host command exited unsuccessfully: {}",
            stderr.trim()
        )));
    }

    let proposal_content = std::fs::read_to_string(target_file).map_err(|error| {
        AppCompositionError::AiRuntime(format!("failed to read ACP host proposal: {error}"))
    })?;
    let generator = DelegatedTaskProposalGenerator::new(sandbox_path.to_path_buf());
    generator
        .generate_proposal(DelegatedTaskProposalInput {
            target_path: target_file,
            modified_content: &proposal_content,
            output_id: format!("acp-output:{task_id}"),
            request_id: format!("acp:{task_id}"),
            provider_id: "acp.local-adapter".to_string(),
            proposal_id: ProposalId(0),
            principal: PrincipalId(format!("delegate-task:{task_id}")),
            capability: CapabilityId("delegated.runtime.allocate".to_string()),
            correlation_id,
            causality_id,
            created_at: TimestampMillis::now(),
            context_manifest: trust_reference(
                &format!("delegate:acp-context:{task_id}"),
                legion_protocol::AssistedAiTrustProjectionKind::ContextManifest,
            ),
            approval_checklist: trust_reference(
                &format!("delegate:acp-approval:{task_id}"),
                legion_protocol::AssistedAiTrustProjectionKind::ProposalApprovalChecklist,
            ),
        })
        .map_err(|error| {
            AppCompositionError::AiRuntime(format!("ACP proposal generation failed: {error}"))
        })
}
