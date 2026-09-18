//! App-owned language-server startup authority.
//!
//! This module builds the fully-bound descriptor consumed by the background
//! LSP worker.  Selection, capability evaluation, and artifact/runtime
//! binding remain on the app side; the worker receives no broker or discovery
//! authority.

use std::{
    path::Path,
    sync::{atomic::AtomicBool, Arc, Mutex},
};

use legion_lsp::{
    node_compatible_path, LanguageServerAdapterPlan, LspServerBinarySource, LspServerProcessConfig,
    LspSupervisorConfig, TYPESCRIPT_LANGUAGE_SERVER_ARCHIVE,
};
use legion_platform::NativeProcessService;
use legion_protocol::{
    CapabilityBrokerPort, CapabilityCommandClass, CapabilityDecisionId, CapabilityId,
    CapabilityRequest, CapabilityRequestContext, CapabilityResponse, CausalityId, CorrelationId,
    FileFingerprint, LanguageId, LanguageServerId, LspConfiguredServerIdentity,
    LspLaunchPolicyDecision, LspServerBinaryProvenance, LspWorkspaceTrustPosture, PrincipalId,
    RedactionHint, SemanticPrivacyScope, WorkspaceId, WorkspaceRootId, WorkspaceTrustState,
};
use legion_security::DenyByDefaultBroker;
use sha2::{Digest, Sha256};

use super::typescript_bundle::TypeScriptBundleDescriptor;
use super::{
    ApprovedNodeRuntime, ArtifactDescriptor, ArtifactSource, LanguageArtifactMaterializer,
    LanguageServerLaunchConfig, LanguageServerStartConfig, LanguageSessionError,
    MaterializeRequest, MaterializedArtifact, NodeRuntimeApprovalRequest,
};

/// Inputs captured from the opened app workspace and event context.
#[derive(Debug, Clone)]
pub struct LanguageStartupContext {
    /// Real workspace identity returned by the workspace authority.
    pub workspace_id: WorkspaceId,
    /// Real root identity returned by the workspace authority.
    pub root_id: WorkspaceRootId,
    /// Canonical workspace root used for initialize and process cwd.
    pub workspace_root: std::path::PathBuf,
    /// Principal that explicitly requested startup.
    pub principal_id: PrincipalId,
    /// Current workspace trust posture.
    pub trust: WorkspaceTrustState,
    /// Nonzero operation correlation.
    pub correlation_id: CorrelationId,
    /// Non-nil operation lineage.
    pub causality_id: CausalityId,
}

/// App-owned authority for one bound language adapter registry.
#[derive(Clone)]
pub struct LanguageStartupAuthority {
    /// Real capability broker shared with app language policy.
    broker: Arc<dyn CapabilityBrokerPort + Send + Sync>,
    /// Optional app-owned policy store used for explicit exact-path config.
    policy_store: Option<Arc<Mutex<DenyByDefaultBroker>>>,
}

impl LanguageStartupAuthority {
    /// Materialize both pinned TypeScript archives and bind a fresh Node
    /// receipt to one launch preparation.
    #[allow(clippy::too_many_arguments)]
    pub fn prepare_typescript_bundle(
        &self,
        context: &LanguageStartupContext,
        bundle: &TypeScriptBundleDescriptor,
        server_archive: &Path,
        compiler_archive: &Path,
        node_path: &Path,
        cache_root: &Path,
        cancellation: Arc<AtomicBool>,
        root_uri: String,
        language_id: LanguageId,
        server_id: LanguageServerId,
    ) -> Result<LanguageServerStartConfig, LanguageSessionError> {
        validate_context(context)?;
        if !matches!(
            language_id.0.as_str(),
            "typescript" | "typescriptreact" | "javascript" | "javascriptreact"
        ) {
            return Err(invalid(
                "TypeScript bundle requires a TypeScript-family language",
            ));
        }
        let node_path = std::fs::canonicalize(node_path)
            .map_err(|e| invalid(format!("Node executable is invalid: {e}")))?;
        if !node_path.is_file() {
            return Err(invalid("Node executable must be a regular file"));
        }
        let runtime_request = NodeRuntimeApprovalRequest {
            executable: node_path,
            principal_id: context.principal_id.clone(),
            workspace_id: context.workspace_id,
            workspace_trust_state: context.trust.clone(),
            correlation_id: context.correlation_id,
            causality_id: context.causality_id,
            minimum_version: match &bundle.server.runtime {
                legion_lsp::LspArtifactRuntime::Node { minimum_version } => *minimum_version,
            },
        };
        let token = super::CancellationToken::from_atomic(Arc::clone(&cancellation));
        let materialize = |descriptor: &ArtifactDescriptor, archive: &Path| {
            let request = MaterializeRequest {
                descriptor: descriptor.clone(),
                source: ArtifactSource::LocalArchive {
                    path: archive.to_path_buf(),
                },
                operation_id: context.correlation_id.0 as u128,
                correlation_id: context.correlation_id,
                causality_id: context.causality_id,
                trusted: true,
                cache_root: cache_root.to_path_buf(),
                deadline: std::time::Duration::from_secs(600),
            };
            LanguageArtifactMaterializer::materialize_with_cancellation(&request, &token)
                .map_err(|e| invalid(e.to_string()))
        };
        let server = materialize(&bundle.server, server_archive)
            .map_err(|error| invalid(format!("server archive materialization failed: {error}")))?;
        let compiler = materialize(&bundle.compiler, compiler_archive).map_err(|error| {
            invalid(format!("compiler archive materialization failed: {error}"))
        })?;
        LanguageArtifactMaterializer::revalidate_materialized_artifact(
            &server,
            &bundle.server,
            &token,
        )
        .map_err(|e| invalid(e.to_string()))?;
        LanguageArtifactMaterializer::revalidate_materialized_artifact(
            &compiler,
            &bundle.compiler,
            &token,
        )
        .map_err(|e| invalid(e.to_string()))?;
        let node = super::approve_node_runtime(
            &*self.broker,
            &NativeProcessService,
            runtime_request,
            Arc::clone(&cancellation),
        )
        .map_err(|e| invalid(e.to_string()))?;
        let server_entrypoint = server
            .cache_root
            .join(&bundle.server.package_root)
            .join(&bundle.server.entrypoint);
        let compiler_entrypoint = compiler
            .cache_root
            .join(&bundle.compiler.package_root)
            .join(&bundle.tsserver_entrypoint);
        let server_entrypoint = server_entrypoint
            .canonicalize()
            .map_err(|_| invalid("server entrypoint missing"))?;
        let compiler_entrypoint = compiler_entrypoint
            .canonicalize()
            .map_err(|_| invalid("TypeScript compiler entrypoint missing"))?;
        let server_cache_root = server
            .cache_root
            .canonicalize()
            .map_err(|_| invalid("server artifact cache root missing"))?;
        let compiler_cache_root = compiler
            .cache_root
            .canonicalize()
            .map_err(|_| invalid("TypeScript compiler cache root missing"))?;
        if !server_entrypoint.is_file()
            || !compiler_entrypoint.is_file()
            || !server_entrypoint.starts_with(&server_cache_root)
            || !compiler_entrypoint.starts_with(&compiler_cache_root)
        {
            return Err(invalid("TypeScript bundle entrypoint containment failed"));
        }
        let process = LspServerProcessConfig {
            command: node
                .canonical_path()
                .to_str()
                .ok_or_else(|| invalid("Node path is not UTF-8"))?
                .to_string(),
            args: vec![
                node_compatible_path(&server_entrypoint, "server entrypoint")
                    .map_err(|e| invalid(e.to_string()))?,
                "--stdio".to_string(),
            ],
            cwd: Some(context.workspace_root.clone()),
            env: Vec::new(),
        };
        let compiler_entrypoint_for_node =
            node_compatible_path(&compiler_entrypoint, "TypeScript compiler entrypoint")
                .map_err(|e| invalid(e.to_string()))?;
        let mut config = self.prepare(
            context,
            server_id,
            language_id,
            "typescript-language-server".to_string(),
            process,
            root_uri,
            LspServerBinaryProvenance::Downloaded,
            Some(FileFingerprint {
                algorithm: "sha256".into(),
                value: server.sha256,
            }),
            None,
            Some(bundle.initialization_options(&compiler_entrypoint_for_node)),
            None,
        )?;
        config.launch_config.version = Some(bundle.server.version.clone());
        config.launch_config.node_runtime_version = Some(node.observed_version());
        config.launch_config.artifact_dependencies = vec![FileFingerprint {
            algorithm: "sha256".into(),
            value: compiler.sha256,
        }];
        Ok(config)
    }
    /// Creates an authority backed by the app's real capability broker.
    pub fn new(broker: Arc<dyn CapabilityBrokerPort + Send + Sync>) -> Self {
        Self {
            broker,
            policy_store: None,
        }
    }

    /// The capability broker this authority uses for language-tool launches.
    ///
    /// External formatter approval has to ask the same broker that recorded the
    /// operator's exact-binary allowance; minting a second broker from the
    /// recorded path would be self-issued authority.
    pub fn capability_broker(&self) -> Arc<dyn CapabilityBrokerPort + Send + Sync> {
        Arc::clone(&self.broker)
    }

    /// Creates an authority with a clonable app-owned deny-by-default policy.
    pub fn with_policy_store(policy: Arc<Mutex<DenyByDefaultBroker>>) -> Self {
        let broker: Arc<dyn CapabilityBrokerPort + Send + Sync> = Arc::new(PolicyBroker {
            policy: Arc::clone(&policy),
        });
        Self {
            broker,
            policy_store: Some(policy),
        }
    }

    /// Adds one exact canonical executable to the app language allowlist.
    pub fn allow_exact_binary(&self, path: &Path) -> Result<(), LanguageSessionError> {
        let Some(policy) = &self.policy_store else {
            return Err(invalid("language policy store is not configurable"));
        };
        let canonical = std::fs::canonicalize(path)
            .map_err(|error| invalid(format!("configured binary is invalid: {error}")))?;
        let value = canonical
            .to_str()
            .ok_or_else(|| invalid("configured binary path is not valid UTF-8"))?
            .to_string();
        let mut broker = policy
            .lock()
            .map_err(|_| invalid("language policy store is poisoned"))?;
        if !broker
            .policy
            .lsp_policy
            .allowed_binaries
            .iter()
            .any(|entry| entry == &value)
        {
            broker.policy.lsp_policy.allowed_binaries.push(value);
        }
        Ok(())
    }

    /// Remove one exact app-language executable approval.
    pub fn revoke_exact_binary(&self, path: &Path) -> Result<(), LanguageSessionError> {
        let Some(policy) = &self.policy_store else {
            return Err(invalid("language policy store is not configurable"));
        };
        let value = path
            .to_str()
            .ok_or_else(|| invalid("configured binary path is not valid UTF-8"))?;
        let mut broker = policy
            .lock()
            .map_err(|_| invalid("language policy store is poisoned"))?;
        broker
            .policy
            .lsp_policy
            .allowed_binaries
            .retain(|entry| entry != value);
        Ok(())
    }

    /// Builds a launch descriptor for a configured/system binary.
    #[allow(clippy::too_many_arguments)]
    pub fn prepare_configured(
        &self,
        context: &LanguageStartupContext,
        server_id: LanguageServerId,
        language_id: LanguageId,
        display_name: impl Into<String>,
        process: LspServerProcessConfig,
        root_uri: String,
        initialization_options: Option<serde_json::Value>,
        client_capabilities: Option<serde_json::Value>,
    ) -> Result<LanguageServerStartConfig, LanguageSessionError> {
        self.prepare(
            context,
            server_id,
            language_id,
            display_name.into(),
            process,
            root_uri,
            LspServerBinaryProvenance::Configured,
            None,
            None,
            initialization_options,
            client_capabilities,
        )
    }

    /// Builds a descriptor after a downloaded package has been materialized
    /// and its explicit Node executable has been approved and probed.
    #[allow(clippy::too_many_arguments)]
    pub fn prepare_downloaded(
        &self,
        context: &LanguageStartupContext,
        adapter: &LanguageServerAdapterPlan,
        descriptor: &ArtifactDescriptor,
        artifact: &MaterializedArtifact,
        approved_node: &ApprovedNodeRuntime,
        runtime_request: &NodeRuntimeApprovalRequest,
        cancellation: Arc<AtomicBool>,
        root_uri: String,
        initialization_options: Option<serde_json::Value>,
        client_capabilities: Option<serde_json::Value>,
    ) -> Result<LanguageServerStartConfig, LanguageSessionError> {
        validate_context(context)?;
        if adapter.workspace_id != context.workspace_id {
            return Err(invalid("adapter workspace does not match opened workspace"));
        }
        if runtime_request.correlation_id != context.correlation_id
            || runtime_request.causality_id != context.causality_id
        {
            return Err(invalid(
                "Node approval context does not match startup event identity",
            ));
        }
        if runtime_request.principal_id != context.principal_id
            || runtime_request.workspace_id != context.workspace_id
            || runtime_request.workspace_trust_state != context.trust
        {
            return Err(invalid(
                "Node approval principal/workspace/trust does not match startup",
            ));
        }
        let LspServerBinarySource::DownloadedArtifact {
            checksum_sha256,
            metadata,
            ..
        } = &adapter.binary_source
        else {
            return Err(invalid("downloaded startup requires a downloaded adapter"));
        };
        if !checksum_sha256.eq_ignore_ascii_case(&descriptor.expected_sha256)
            || metadata.package_name != descriptor.package_name
            || metadata.version != descriptor.version
            || metadata.package_root != descriptor.package_root
            || metadata.entrypoint != descriptor.entrypoint
            || metadata.runtime != descriptor.runtime
        {
            return Err(invalid(
                "artifact descriptor does not match adapter metadata",
            ));
        }
        if metadata.package_name == TYPESCRIPT_LANGUAGE_SERVER_ARCHIVE.package_name {
            let has_peer = initialization_options
                .as_ref()
                .and_then(|value| value.get("tsserver"))
                .and_then(|value| value.get("path"))
                .and_then(serde_json::Value::as_str)
                .is_some_and(|path| !path.trim().is_empty());
            if !has_peer {
                return Err(invalid(
                    "typescript-language-server requires the pinned TypeScript compiler peer; \
                     start it through the TypeScript bundle path so TYPESCRIPT_COMPILER_ARCHIVE \
                     is materialized",
                ));
            }
        }
        approved_node
            .revalidate(
                runtime_request,
                runtime_request.minimum_version,
                Arc::clone(&cancellation),
            )
            .map_err(|error| invalid(error.to_string()))?;
        let materialize_cancellation =
            super::CancellationToken::from_atomic(Arc::clone(&cancellation));
        super::LanguageArtifactMaterializer::revalidate_materialized_artifact(
            artifact,
            descriptor,
            &materialize_cancellation,
        )
        .map_err(|error| invalid(error.to_string()))?;
        let mut process = adapter
            .resolve_downloaded_process(
                &artifact.cache_root,
                approved_node.canonical_path(),
                &format!(
                    "{}.{}.{}",
                    approved_node.observed_version().major,
                    approved_node.observed_version().minor,
                    approved_node.observed_version().patch
                ),
                &artifact.sha256,
            )
            .map_err(|error| invalid(error.to_string()))?;
        if process.cwd.is_none() {
            process.cwd = Some(context.workspace_root.clone());
        }
        let observed_version = format!(
            "{}.{}.{}",
            approved_node.observed_version().major,
            approved_node.observed_version().minor,
            approved_node.observed_version().patch
        );
        if artifact.sha256.len() != 64
            || !artifact.sha256.bytes().all(|byte| byte.is_ascii_hexdigit())
            || !artifact.cache_root.is_absolute()
        {
            return Err(invalid("materialized artifact identity is malformed"));
        }
        let artifact_hash = FileFingerprint {
            algorithm: "sha256".to_string(),
            value: artifact.sha256.clone(),
        };
        let mut config = self.prepare(
            context,
            adapter.server_id,
            adapter.language_id.clone(),
            adapter.display_name.clone(),
            process,
            root_uri,
            LspServerBinaryProvenance::Downloaded,
            Some(artifact_hash),
            None,
            initialization_options,
            client_capabilities,
        )?;
        config.launch_config.version = Some(observed_version);
        Ok(config)
    }

    /// Materialize and approve a configured local package on the startup worker.
    #[allow(clippy::too_many_arguments)]
    pub fn prepare_downloaded_local(
        &self,
        context: &LanguageStartupContext,
        adapter: &LanguageServerAdapterPlan,
        archive: &Path,
        cache_root: &Path,
        node_path: &Path,
        cancellation: Arc<AtomicBool>,
        root_uri: String,
        initialization_options: Option<serde_json::Value>,
    ) -> Result<LanguageServerStartConfig, LanguageSessionError> {
        validate_context(context)?;
        if cancellation.load(std::sync::atomic::Ordering::Acquire) {
            return Err(invalid("language startup preparation cancelled"));
        }
        let node_path = std::fs::canonicalize(node_path)
            .map_err(|error| invalid(format!("Node executable is invalid: {error}")))?;
        if !node_path.is_file() {
            return Err(invalid("Node executable must be a regular file"));
        }
        let (checksum, metadata) = match &adapter.binary_source {
            LspServerBinarySource::DownloadedArtifact {
                checksum_sha256,
                metadata,
                ..
            } => (checksum_sha256.clone(), metadata.as_ref().clone()),
            _ => {
                return Err(invalid(
                    "local downloaded startup requires a downloaded adapter",
                ));
            }
        };
        let descriptor = ArtifactDescriptor::from_metadata(
            format!("language-server-{}", adapter.server_id.0),
            checksum,
            &metadata,
        );
        let request = MaterializeRequest {
            descriptor: descriptor.clone(),
            source: ArtifactSource::LocalArchive {
                path: archive.to_path_buf(),
            },
            operation_id: context.correlation_id.0 as u128,
            correlation_id: context.correlation_id,
            causality_id: context.causality_id,
            trusted: context.trust == WorkspaceTrustState::Trusted,
            cache_root: cache_root.to_path_buf(),
            deadline: std::time::Duration::from_secs(600),
        };
        let materialize_cancellation =
            super::CancellationToken::from_atomic(Arc::clone(&cancellation));
        let artifact = LanguageArtifactMaterializer::materialize_with_cancellation(
            &request,
            &materialize_cancellation,
        )
        .map_err(|error| invalid(error.to_string()))?;
        let minimum_version = match &descriptor.runtime {
            legion_lsp::LspArtifactRuntime::Node { minimum_version } => *minimum_version,
        };
        let runtime_request = NodeRuntimeApprovalRequest {
            executable: node_path,
            principal_id: context.principal_id.clone(),
            workspace_id: context.workspace_id,
            workspace_trust_state: context.trust.clone(),
            correlation_id: context.correlation_id,
            causality_id: context.causality_id,
            minimum_version,
        };
        let approved_node = super::approve_node_runtime(
            &*self.broker,
            &NativeProcessService,
            runtime_request.clone(),
            Arc::clone(&cancellation),
        )
        .map_err(|error| invalid(error.to_string()))?;
        self.prepare_downloaded(
            context,
            adapter,
            &descriptor,
            &artifact,
            &approved_node,
            &runtime_request,
            cancellation,
            root_uri,
            initialization_options,
            None,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn prepare(
        &self,
        context: &LanguageStartupContext,
        server_id: LanguageServerId,
        language_id: LanguageId,
        display_name: String,
        mut process: LspServerProcessConfig,
        root_uri: String,
        provenance: LspServerBinaryProvenance,
        artifact_hash: Option<FileFingerprint>,
        download_decision_id: Option<CapabilityDecisionId>,
        initialization_options: Option<serde_json::Value>,
        client_capabilities: Option<serde_json::Value>,
    ) -> Result<LanguageServerStartConfig, LanguageSessionError> {
        validate_context(context)?;
        if server_id.0 == 0 || language_id.0.trim().is_empty() {
            return Err(invalid("server and language identities are required"));
        }
        let command = process.command.as_str();
        if command.trim() != command || command.is_empty() || !Path::new(command).is_absolute() {
            return Err(invalid(
                "language server command must be an explicit absolute path",
            ));
        }
        let command_path = Path::new(command);
        if !command_path.is_file()
            || std::fs::canonicalize(command_path).ok().as_deref() != Some(command_path)
        {
            return Err(invalid(
                "language server command must be a canonical regular file",
            ));
        }
        if root_uri.trim().is_empty() || !root_uri.starts_with("file:///") {
            return Err(invalid(
                "language server root URI must be an absolute file URI",
            ));
        }
        let canonical_root = std::fs::canonicalize(&context.workspace_root)
            .map_err(|_| invalid("workspace root must be an existing directory"))?;
        if !canonical_root.is_dir() {
            return Err(invalid("workspace root must be a directory"));
        }
        let incoming_root = crate::uri_to_canonical_path(&root_uri);
        let incoming_canonical = std::fs::canonicalize(&incoming_root)
            .map_err(|_| invalid("language server root URI does not match workspace root"))?;
        if incoming_canonical != canonical_root {
            return Err(invalid(
                "language server root URI does not match workspace root",
            ));
        }
        if let Some(cwd) = process.cwd.as_ref() {
            let canonical_cwd = std::fs::canonicalize(cwd)
                .map_err(|_| invalid("language server cwd is not canonical"))?;
            let canonical_root = std::fs::canonicalize(&context.workspace_root)
                .map_err(|_| invalid("workspace root is not canonical"))?;
            if canonical_cwd != canonical_root {
                return Err(invalid("language server cwd does not match workspace root"));
            }
        } else {
            process.cwd = Some(canonical_root);
        }

        let decision_id = request_launch_capability(&*self.broker, context, command)?;
        let identity = LspConfiguredServerIdentity {
            server_id,
            workspace_id: context.workspace_id,
            root_id: Some(context.root_id),
            language_id: language_id.clone(),
            display_name,
            command_hash: fingerprint(command.as_bytes()),
            args_hash: Some(fingerprint(process.args.join("\0").as_bytes())),
            env_hash: Some(fingerprint(
                process
                    .env
                    .iter()
                    .map(|(k, v)| format!("{k}={v}\0"))
                    .collect::<String>()
                    .as_bytes(),
            )),
            cwd_hash: process
                .cwd
                .as_ref()
                .map(|cwd| {
                    cwd.to_str()
                        .map(|value| fingerprint(value.as_bytes()))
                        .ok_or_else(|| invalid("language server cwd must be valid UTF-8"))
                })
                .transpose()?,
            settings_hash: None,
            redaction_hints: vec![RedactionHint::MetadataOnly],
            schema_version: 1,
        };
        let posture = LspWorkspaceTrustPosture {
            workspace_id: context.workspace_id,
            workspace_trust_state: context.trust.clone(),
            privacy_scope: SemanticPrivacyScope::Workspace,
            privacy_scope_allowed: true,
            required_capability: CapabilityId("lsp.launch".to_string()),
            decision_id: Some(decision_id),
            diagnostics: Vec::new(),
            schema_version: 1,
        };
        let launch_policy = LspLaunchPolicyDecision::evaluate(
            identity,
            posture,
            true,
            context.correlation_id,
            context.causality_id,
            Vec::new(),
            1,
        );
        Ok(LanguageServerStartConfig {
            workspace_root: context.workspace_root.clone(),
            root_uri,
            launch_config: LanguageServerLaunchConfig {
                supervisor: LspSupervisorConfig {
                    launch_policy,
                    process,
                    initial_backoff_ms: 500,
                    max_backoff_ms: 30_000,
                    max_restart_attempts: 3,
                },
                server_id,
                language_id,
                binary_provenance: provenance,
                artifact_hash,
                artifact_dependencies: Vec::new(),
                version: None,
                node_runtime_version: None,
                download_decision_id,
            },
            initialization_options,
            client_capabilities,
        })
    }
}

fn request_launch_capability(
    broker: &dyn CapabilityBrokerPort,
    context: &LanguageStartupContext,
    command: &str,
) -> Result<CapabilityDecisionId, LanguageSessionError> {
    let capability = CapabilityId("lsp.launch".to_string());
    let response = broker
        .handle(CapabilityRequest::Request {
            principal_id: context.principal_id.clone(),
            capability_id: capability.clone(),
            workspace_trust_state: context.trust.clone(),
            target_path: Some(legion_protocol::CanonicalPath(command.to_string())),
            decision_id: None,
            context: CapabilityRequestContext {
                command_binary: Some(command.to_string()),
                command_class: Some(CapabilityCommandClass::LanguageServer),
                lsp_server_binary: Some(command.to_string()),
                ..CapabilityRequestContext::default()
            },
            correlation_id: context.correlation_id,
        })
        .map_err(|error| invalid(error.message))?;
    match response {
        CapabilityResponse::Decision(decision)
            if decision.granted
                && decision.decision_id.0 != 0
                && decision.capability == capability =>
        {
            Ok(decision.decision_id)
        }
        CapabilityResponse::Decision(decision) => Err(invalid(format!(
            "language server launch denied (decision {}, granted {})",
            decision.decision_id.0, decision.granted
        ))),
        CapabilityResponse::Denied(denial) => Err(invalid(denial.reason)),
        CapabilityResponse::Granted(grant)
            if grant.decision_id.0 != 0
                && grant.principal_id == context.principal_id
                && grant.capability_id == capability =>
        {
            Ok(grant.decision_id)
        }
        CapabilityResponse::Granted(_) => Err(invalid("language server grant has zero decision")),
    }
}

fn validate_context(context: &LanguageStartupContext) -> Result<(), LanguageSessionError> {
    if context.workspace_id.0 == 0 || context.root_id.0 == 0 {
        return Err(invalid("workspace and root identities must be nonzero"));
    }
    if context.principal_id.0.trim().is_empty() {
        return Err(invalid("principal identity is required"));
    }
    if context.trust != WorkspaceTrustState::Trusted {
        return Err(invalid(
            "language server launch requires a trusted workspace",
        ));
    }
    if context.correlation_id.0 == 0 || context.causality_id.0.is_nil() {
        return Err(invalid("event identity must be nonzero"));
    }
    Ok(())
}

fn fingerprint(bytes: &[u8]) -> FileFingerprint {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    FileFingerprint {
        algorithm: "sha256-content-v1".to_string(),
        value: hex::encode(hasher.finalize()),
    }
}

fn invalid(message: impl Into<String>) -> LanguageSessionError {
    LanguageSessionError::InvalidConfiguration(message.into())
}

struct PolicyBroker {
    policy: Arc<Mutex<DenyByDefaultBroker>>,
}

impl CapabilityBrokerPort for PolicyBroker {
    fn handle(
        &self,
        request: CapabilityRequest,
    ) -> legion_protocol::ProtocolResult<CapabilityResponse> {
        let broker = self
            .policy
            .lock()
            .map_err(|_| legion_protocol::ProtocolError::validation("language policy poisoned"))?;
        broker.handle(request)
    }
}
