//! Optional ACP host command used by delegated-task proposal generation.

use std::path::{Path, PathBuf};
use std::time::Duration;

use legion_platform::{NativeProcessService, ProcessRequest, ProcessService};

#[cfg(feature = "ai")]
use legion_agent::{DelegatedTaskProposalGenerator, DelegatedTaskProposalInput};
#[cfg(feature = "ai")]
use legion_protocol::{
    CapabilityId, CausalityId, CorrelationId, PrincipalId, ProposalId, TimestampMillis,
};

#[cfg(feature = "ai")]
use crate::{AppCompositionError, trust_reference};

/// Hard bound so a hung ACP host cannot pin the delegated worker forever.
const ACP_HOST_TIMEOUT: Duration = Duration::from_secs(30);

/// Bytes captured from a supervised ACP host process.
#[derive(Debug, Clone)]
pub(crate) struct AcpHostOutput {
    pub(crate) success: bool,
    pub(crate) status_label: String,
    pub(crate) stdout: Vec<u8>,
    pub(crate) stderr: Vec<u8>,
}

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
    ) -> Result<AcpHostOutput, String> {
        let mut request = ProcessRequest::new(self.program.to_string_lossy().into_owned());
        request.args = self.args.clone();
        request.cwd = Some(sandbox_path.to_path_buf());
        request.env = vec![
            ("LEGION_ACP_PLAN_ID".to_string(), plan_id.to_string()),
            (
                "LEGION_ACP_SANDBOX_PATH".to_string(),
                sandbox_path.display().to_string(),
            ),
            (
                "LEGION_ACP_TARGET_PATH".to_string(),
                target_path.display().to_string(),
            ),
            (
                "LEGION_ACP_TARGET_DIR".to_string(),
                target_path
                    .parent()
                    .unwrap_or(sandbox_path)
                    .display()
                    .to_string(),
            ),
            (
                "LEGION_ACP_TARGET_FILE".to_string(),
                target_path
                    .file_name()
                    .and_then(|value| value.to_str())
                    .unwrap_or("proposal.txt")
                    .to_string(),
            ),
        ];
        request.timeout = Some(ACP_HOST_TIMEOUT);
        let result = NativeProcessService.execute(&request).map_err(|error| {
            format!("ACP host command failed under process supervision: {error}")
        })?;
        Ok(AcpHostOutput {
            success: result.exit_code == 0,
            status_label: result.exit_code.to_string(),
            stdout: result.stdout.into_bytes(),
            stderr: result.stderr.into_bytes(),
        })
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
        .map_err(AppCompositionError::AiRuntime)?;
    if !output.success {
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
