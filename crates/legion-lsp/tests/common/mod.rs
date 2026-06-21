use std::path::PathBuf;

use legion_lsp::{LspServerProcessConfig, LspSupervisorConfig};
use legion_protocol::{
    BufferId, BufferVersion, CancellationTokenId, CapabilityDecisionId, CapabilityId, CausalityId,
    CorrelationId, FileFingerprint, FileId, LanguageId, LanguageServerId,
    LspConfiguredServerIdentity, LspOperationContext, LspRequestId, LspWorkspaceTrustPosture,
    RedactionHint, SemanticPrivacyScope, SnapshotId, WorkspaceId, WorkspaceRootId,
    WorkspaceTrustState,
};
use uuid::Uuid;

fn fingerprint(value: &str) -> FileFingerprint {
    FileFingerprint {
        algorithm: "test".to_string(),
        value: value.to_string(),
    }
}

fn identity() -> LspConfiguredServerIdentity {
    LspConfiguredServerIdentity {
        server_id: LanguageServerId(7),
        workspace_id: WorkspaceId(55),
        root_id: Some(WorkspaceRootId(5)),
        language_id: LanguageId("rust".to_string()),
        display_name: "rust-analyzer".to_string(),
        command_hash: fingerprint("command"),
        args_hash: Some(fingerprint("args")),
        env_hash: Some(fingerprint("env")),
        cwd_hash: Some(fingerprint("cwd")),
        settings_hash: Some(fingerprint("settings")),
        redaction_hints: vec![RedactionHint::MetadataOnly],
        schema_version: 1,
    }
}

fn posture(
    trust: WorkspaceTrustState,
    privacy_scope_allowed: bool,
) -> LspWorkspaceTrustPosture {
    LspWorkspaceTrustPosture {
        workspace_id: WorkspaceId(55),
        workspace_trust_state: trust,
        privacy_scope: SemanticPrivacyScope::Workspace,
        privacy_scope_allowed,
        required_capability: CapabilityId("process.spawn".to_string()),
        decision_id: Some(CapabilityDecisionId(99)),
        diagnostics: Vec::new(),
        schema_version: 1,
    }
}

fn launch_policy(
    trust: WorkspaceTrustState,
    privacy_scope_allowed: bool,
    runtime_activation_accepted: bool,
) -> legion_protocol::LspLaunchPolicyDecision {
    legion_protocol::LspLaunchPolicyDecision::evaluate(
        identity(),
        posture(trust, privacy_scope_allowed),
        runtime_activation_accepted,
        CorrelationId(91),
        CausalityId(Uuid::from_u128(92)),
        Vec::new(),
        1,
    )
}

fn supervisor_config(command: impl Into<String>, args: Vec<String>) -> LspSupervisorConfig {
    LspSupervisorConfig {
        launch_policy: launch_policy(WorkspaceTrustState::Trusted, true, true),
        process: LspServerProcessConfig {
            command: command.into(),
            args,
            cwd: Some(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")),
            env: Vec::new(),
        },
        initial_backoff_ms: 25,
        max_backoff_ms: 400,
        max_restart_attempts: 3,
    }
}

pub fn mock_server_config() -> LspSupervisorConfig {
    supervisor_config(env!("CARGO_BIN_EXE_mock_lsp_server"), Vec::new())
}

pub fn mock_server_config_with_diagnostics() -> LspSupervisorConfig {
    let mut config = mock_server_config();
    config
        .process
        .env
        .push(("MOCK_LSP_EMIT_DIAGNOSTICS".to_string(), "1".to_string()));
    config
}

pub fn ctx() -> LspOperationContext {
    LspOperationContext {
        request_id: LspRequestId(Uuid::from_u128(1)),
        workspace_id: WorkspaceId(55),
        file_id: FileId(11),
        buffer_id: BufferId(12),
        snapshot_id: SnapshotId(13),
        buffer_version: BufferVersion(14),
        language_id: LanguageId("rust".to_string()),
        correlation_id: CorrelationId(1u64),
        causality_id: CausalityId(Uuid::from_u128(1001)),
        timeout_ms: 5000,
        cancellation_token: CancellationTokenId(Uuid::from_u128(2001)),
        content_hash: None,
        privacy_scope: SemanticPrivacyScope::Workspace,
        schema_version: 1,
    }
}
