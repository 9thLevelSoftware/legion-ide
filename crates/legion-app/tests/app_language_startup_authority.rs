use std::sync::Arc;

use legion_app::language::{LanguageStartupAuthority, LanguageStartupContext};
use legion_protocol::{
    CapabilityBrokerPort, CapabilityDecision, CapabilityResponse, CausalityId, CorrelationId,
    LanguageId, LanguageServerId, PrincipalId, WorkspaceId, WorkspaceRootId, WorkspaceTrustState,
};

struct DecisionBroker {
    granted: bool,
}

impl CapabilityBrokerPort for DecisionBroker {
    fn handle(
        &self,
        request: legion_protocol::CapabilityRequest,
    ) -> legion_protocol::ProtocolResult<CapabilityResponse> {
        let legion_protocol::CapabilityRequest::Request {
            capability_id,
            principal_id,
            ..
        } = request
        else {
            panic!("startup authority must issue a request envelope")
        };
        if self.granted {
            Ok(CapabilityResponse::Decision(CapabilityDecision {
                decision_id: legion_protocol::CapabilityDecisionId(41),
                granted: true,
                capability: capability_id,
                reason: None,
            }))
        } else {
            Ok(CapabilityResponse::Denied(
                legion_protocol::CapabilityDenial {
                    decision_id: legion_protocol::CapabilityDecisionId(41),
                    principal_id,
                    capability_id,
                    reason: "operator denied language server launch".to_string(),
                },
            ))
        }
    }
}

fn context() -> LanguageStartupContext {
    let workspace_root = std::env::current_dir().expect("workspace root");
    LanguageStartupContext {
        workspace_id: WorkspaceId(77),
        root_id: WorkspaceRootId(78),
        workspace_root,
        principal_id: PrincipalId("operator".to_string()),
        trust: WorkspaceTrustState::Trusted,
        correlation_id: CorrelationId(9),
        causality_id: CausalityId(uuid::Uuid::from_u128(9)),
    }
}

fn root_uri() -> String {
    let root = std::fs::canonicalize(std::env::current_dir().unwrap()).unwrap();
    let mut path = root.to_str().unwrap().replace('\\', "/");
    if let Some(rest) = path.strip_prefix("//?/") {
        path = rest.to_string();
    }
    format!("file:///{}", path.trim_start_matches('/'))
}

#[test]
fn configured_startup_binds_real_context_and_lsp_decision() {
    let dir = tempfile::tempdir().expect("temp dir");
    let binary = dir
        .path()
        .join(if cfg!(windows) { "node.exe" } else { "node" });
    std::fs::write(&binary, b"fixture").expect("binary fixture");
    let authority = LanguageStartupAuthority::new(Arc::new(DecisionBroker { granted: true }));
    let config = authority
        .prepare_configured(
            &context(),
            LanguageServerId(5),
            LanguageId("python".to_string()),
            "pyright",
            legion_lsp::LspServerProcessConfig {
                command: std::fs::canonicalize(&binary)
                    .unwrap()
                    .to_string_lossy()
                    .into_owned(),
                args: vec!["--stdio".to_string()],
                cwd: None,
                env: Vec::new(),
            },
            root_uri(),
            None,
            None,
        )
        .expect("operator-approved startup config");

    assert_eq!(config.launch_config.server_id, LanguageServerId(5));
    assert_eq!(
        config.launch_config.language_id,
        LanguageId("python".to_string())
    );
    assert_eq!(
        config
            .launch_config
            .supervisor
            .launch_policy
            .posture
            .required_capability
            .0,
        "lsp.launch"
    );
    assert_eq!(
        config
            .launch_config
            .supervisor
            .launch_policy
            .posture
            .decision_id,
        Some(legion_protocol::CapabilityDecisionId(41))
    );
}

#[test]
fn denied_operator_decision_prevents_descriptor_creation() {
    let dir = tempfile::tempdir().expect("temp dir");
    let binary = dir
        .path()
        .join(if cfg!(windows) { "node.exe" } else { "node" });
    std::fs::write(&binary, b"fixture").expect("binary fixture");
    let authority = LanguageStartupAuthority::new(Arc::new(DecisionBroker { granted: false }));
    let result = authority.prepare_configured(
        &context(),
        LanguageServerId(5),
        LanguageId("python".to_string()),
        "pyright",
        legion_lsp::LspServerProcessConfig {
            command: std::fs::canonicalize(&binary)
                .unwrap()
                .to_string_lossy()
                .into_owned(),
            args: Vec::new(),
            cwd: None,
            env: Vec::new(),
        },
        root_uri(),
        None,
        None,
    );
    assert!(result.is_err());
}

#[test]
fn real_policy_allows_only_explicit_configured_binary() {
    let dir = tempfile::tempdir().expect("temp dir");
    let binary = dir.path().join(if cfg!(windows) {
        "server.exe"
    } else {
        "server"
    });
    std::fs::write(&binary, b"fixture").expect("binary fixture");
    let policy = Arc::new(std::sync::Mutex::new(
        legion_security::DenyByDefaultBroker::default(),
    ));
    let authority = LanguageStartupAuthority::with_policy_store(Arc::clone(&policy));
    authority
        .allow_exact_binary(&binary)
        .expect("operator config");
    let configured = authority
        .prepare_configured(
            &context(),
            LanguageServerId(6),
            LanguageId("rust".to_string()),
            "rust-analyzer",
            legion_lsp::LspServerProcessConfig {
                command: std::fs::canonicalize(&binary)
                    .unwrap()
                    .to_string_lossy()
                    .into_owned(),
                args: Vec::new(),
                cwd: None,
                env: Vec::new(),
            },
            root_uri(),
            None,
            None,
        )
        .expect("explicit path should pass real policy");
    assert!(
        configured
            .launch_config
            .supervisor
            .launch_policy
            .process_launch_allowed
    );

    let other = dir
        .path()
        .join(if cfg!(windows) { "other.exe" } else { "other" });
    std::fs::write(&other, b"other fixture").expect("other binary fixture");
    let wrong_binary = authority.prepare_configured(
        &context(),
        LanguageServerId(6),
        LanguageId("rust".to_string()),
        "rust-analyzer",
        legion_lsp::LspServerProcessConfig {
            command: std::fs::canonicalize(&other)
                .unwrap()
                .to_string_lossy()
                .into_owned(),
            args: Vec::new(),
            cwd: None,
            env: Vec::new(),
        },
        root_uri(),
        None,
        None,
    );
    assert!(
        wrong_binary.is_err(),
        "unconfigured exact binary must be denied"
    );

    let mut untrusted = context();
    untrusted.trust = WorkspaceTrustState::Untrusted;
    let denied = authority.prepare_configured(
        &untrusted,
        LanguageServerId(6),
        LanguageId("rust".to_string()),
        "rust-analyzer",
        legion_lsp::LspServerProcessConfig {
            command: std::fs::canonicalize(&binary)
                .unwrap()
                .to_string_lossy()
                .into_owned(),
            args: Vec::new(),
            cwd: None,
            env: Vec::new(),
        },
        root_uri(),
        None,
        None,
    );
    assert!(denied.is_err());
}
