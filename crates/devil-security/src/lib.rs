//! Policy engine, air-gap mode, exfiltration checks, secrets boundaries.

#![warn(missing_docs)]

use std::{
    collections::{HashMap, HashSet},
    fmt,
    path::Path,
};

use devil_protocol::{
    CapabilityBrokerPort, CapabilityDecision, CapabilityDecisionId, CapabilityDenial,
    CapabilityGrant, CapabilityId, CapabilityNamespace, CapabilityRequest,
    CapabilityRequestContext, CapabilityResponse, CorrelationId,
    DelegatedTaskToolPermissionDecision, DelegatedTaskToolPermissionRequest,
    DelegatedTaskToolPermissionRequestInput, McpServerId, McpToolDescriptor, McpToolName,
    PrincipalId, ProductMode, WorkspaceTrustState, delegated_task_tool_permission_request,
    product_runtime_surface_for_capability,
};
use thiserror::Error;

/// Trust state accepted by policy for workspace-sensitive decisions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrustState {
    /// User explicitly marked the workspace trusted.
    Trusted,
    /// Workspace is explicitly marked untrusted.
    Untrusted,
    /// Trust is not yet established.
    Unknown,
}

impl From<WorkspaceTrustState> for TrustState {
    fn from(value: WorkspaceTrustState) -> Self {
        match value {
            WorkspaceTrustState::Trusted => Self::Trusted,
            WorkspaceTrustState::Untrusted => Self::Untrusted,
            WorkspaceTrustState::Unknown => Self::Unknown,
        }
    }
}

/// Path access mode supported by policy checks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PathAccess {
    /// Read-only file access.
    Read,
    /// Write/create/delete mutation access.
    Write,
    /// Watcher metadata/listing access.
    List,
}

/// Path-specific constraints.
#[derive(Debug, Clone)]
pub struct PathPolicy {
    /// Writable roots allowed by policy.
    pub writable_roots: Vec<String>,
    /// Read-only allowed roots.
    pub readable_roots: Vec<String>,
    /// Explicit blocked roots.
    pub blocked_roots: Vec<String>,
    /// Maximum file-length update boundary for writes (bytes).
    pub max_write_bytes: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct NormalizedPolicyPath {
    prefix: Option<String>,
    segments: Vec<String>,
}

impl NormalizedPolicyPath {
    fn parse(raw: &str) -> Option<Self> {
        let mut normalized = raw.trim().replace('\\', "/");

        if normalized.starts_with("//?/UNC/") {
            normalized = format!("//{}", &normalized[8..]);
        } else if normalized.starts_with("//?/") || normalized.starts_with("//./") {
            normalized = normalized[4..].to_string();
        }

        let mut prefix = None;
        let mut tail = normalized;

        if let Some(rest) = tail.strip_prefix("//") {
            let mut iter = rest.split('/').filter(|part| !part.is_empty());
            let host = iter.next()?;
            let share = iter.next()?;
            prefix = Some(Self::normalize_case(format!("//{host}/{share}")));
            tail = iter.collect::<Vec<_>>().join("/");
        } else if tail.len() >= 2 && tail.as_bytes()[1] == b':' {
            prefix = Some(Self::normalize_case(tail[..2].to_string()));
            tail = tail[2..].trim_start_matches('/').to_string();
        } else if let Some(rest) = tail.strip_prefix('/') {
            prefix = Some(Self::normalize_case("/".to_string()));
            tail = rest.to_string();
        }

        let mut segments = Vec::new();
        for part in tail.split('/') {
            if part.is_empty() || part == "." {
                continue;
            }
            if part == ".." {
                segments.pop()?;
                continue;
            }
            segments.push(Self::normalize_case(part.to_string()));
        }

        Some(Self { prefix, segments })
    }

    fn normalize_case(value: String) -> String {
        #[cfg(windows)]
        {
            value.to_ascii_lowercase()
        }

        #[cfg(not(windows))]
        {
            value
        }
    }

    fn starts_with(&self, root: &Self) -> bool {
        if let Some(expected_prefix) = &root.prefix
            && self.prefix.as_ref() != Some(expected_prefix)
        {
            return false;
        }

        if root.segments.len() > self.segments.len() {
            return false;
        }

        self.segments
            .iter()
            .zip(root.segments.iter())
            .all(|(left, right)| left == right)
    }
}

impl PathPolicy {
    /// Evaluates whether `path` can be used for provided access mode.
    pub fn can_access(&self, path: &str, access: PathAccess) -> bool {
        let Some(candidate) = NormalizedPolicyPath::parse(path) else {
            return false;
        };

        if self.blocked_roots.iter().any(|blocked| {
            NormalizedPolicyPath::parse(blocked)
                .map(|blocked| candidate.starts_with(&blocked))
                .unwrap_or(false)
        }) {
            return false;
        }

        let allowed = match access {
            PathAccess::Read | PathAccess::List => &self.readable_roots,
            PathAccess::Write => &self.writable_roots,
        };

        if allowed.is_empty() {
            return false;
        }

        allowed.iter().any(|root| {
            NormalizedPolicyPath::parse(root)
                .map(|root| candidate.starts_with(&root))
                .unwrap_or(false)
        })
    }
}

impl Default for PathPolicy {
    fn default() -> Self {
        Self {
            writable_roots: vec!["./".to_string()],
            readable_roots: vec!["./".to_string()],
            blocked_roots: vec![".secrets/".to_string(), "/etc/".to_string()],
            max_write_bytes: 512 * 1024,
        }
    }
}

/// Builds the canonical MCP tool permission target identifier.
pub fn mcp_tool_target_id(server_id: &McpServerId, tool_name: &McpToolName) -> String {
    format!("mcp-tool:{}|{}", server_id.0, tool_name.0)
}

/// Builds display-safe policy labels for an MCP tool permission request.
pub fn mcp_tool_permission_labels(
    session_label: impl Into<String>,
    tool: &McpToolDescriptor,
) -> Vec<String> {
    vec![
        "automate.permission.mcp_tool_call".to_string(),
        session_label.into(),
        format!("mcp.server:{}", tool.server_id.0),
        format!("mcp.tool:{}", tool.name.0),
        format!("mcp.capability:{}", tool.capability.0),
    ]
}

/// Builds an MCP tool permission request from the security-owned descriptor policy.
pub fn mcp_tool_permission_request(
    request_id: impl Into<String>,
    tool: &McpToolDescriptor,
    decision: DelegatedTaskToolPermissionDecision,
    session_label: impl Into<String>,
    schema_version: u16,
) -> DelegatedTaskToolPermissionRequest {
    delegated_task_tool_permission_request(DelegatedTaskToolPermissionRequestInput {
        request_id: request_id.into(),
        profile: tool.required_permission_profile,
        action_class: tool.action_class,
        capability: Some(tool.capability.clone()),
        target_id: Some(mcp_tool_target_id(&tool.server_id, &tool.name)),
        decision,
        labels: mcp_tool_permission_labels(session_label, tool),
        schema_version,
    })
}

/// Evaluates whether an MCP tool permission allows runtime invocation.
pub fn mcp_tool_permission_allows_runtime(permission: &DelegatedTaskToolPermissionRequest) -> bool {
    permission.runtime_allowed && permission.human_approval_recorded && !permission.deny_overrides
}

impl fmt::Display for TrustState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Trusted => write!(f, "trusted"),
            Self::Untrusted => write!(f, "untrusted"),
            Self::Unknown => write!(f, "unknown"),
        }
    }
}

/// Command family classification.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CommandClass {
    /// Commands that only read state.
    Read,
    /// Commands that mutate local process state.
    Mutate,
    /// Commands that start terminal execution.
    Terminal,
    /// Commands that reach the network.
    Network,
    /// Commands that launch language tooling.
    LanguageServer,
    /// Commands with no recognized behavior.
    Unknown,
}

/// Command-level policy for taxonomy checks.
#[derive(Debug, Clone)]
pub struct CommandTaxonomy {
    /// Explicitly classified commands.
    pub by_name: HashMap<String, CommandClass>,
}

impl CommandTaxonomy {
    /// Builds a conservative command taxonomy.
    pub fn new() -> Self {
        Self {
            by_name: HashMap::from([
                ("ls".to_string(), CommandClass::Read),
                ("cat".to_string(), CommandClass::Read),
                ("git".to_string(), CommandClass::Read),
                ("rm".to_string(), CommandClass::Mutate),
                ("cp".to_string(), CommandClass::Mutate),
                ("mv".to_string(), CommandClass::Mutate),
                ("rustup".to_string(), CommandClass::LanguageServer),
                ("cargo".to_string(), CommandClass::LanguageServer),
                ("curl".to_string(), CommandClass::Network),
                ("wget".to_string(), CommandClass::Network),
                ("cmd".to_string(), CommandClass::Terminal),
                ("powershell".to_string(), CommandClass::Terminal),
                ("bash".to_string(), CommandClass::Terminal),
                ("sh".to_string(), CommandClass::Terminal),
            ]),
        }
    }

    /// Classifies command by token prefix.
    pub fn classify(&self, command: &str) -> CommandClass {
        self.by_name
            .get(command)
            .copied()
            .or_else(|| {
                let first = command.split_whitespace().next().unwrap_or("unknown");
                self.by_name.get(first).copied()
            })
            .unwrap_or(CommandClass::Unknown)
    }
}

impl Default for CommandTaxonomy {
    fn default() -> Self {
        Self::new()
    }
}

/// Terminal policy controls.
#[derive(Debug, Clone)]
pub struct TerminalPolicy {
    /// Output byte ceiling for any one terminal session.
    pub max_output_bytes: usize,
    /// Whether untrusted trust states may launch terminal.
    pub allow_untrusted: bool,
    /// Allowed terminal capabilities. Unknown capabilities remain denied.
    pub allowed_capabilities: HashSet<String>,
    /// Whether standalone terminal runtime operations are enabled.
    pub runtime_enabled: bool,
    /// Maximum command timeout in seconds.
    pub max_command_timeout_seconds: u64,
}

impl Default for TerminalPolicy {
    fn default() -> Self {
        Self {
            max_output_bytes: 256 * 1024,
            allow_untrusted: false,
            allowed_capabilities: HashSet::from([
                "terminal.launch".to_string(),
                "terminal.input".to_string(),
                "terminal.resize".to_string(),
                "terminal.close".to_string(),
                "terminal.kill".to_string(),
            ]),
            runtime_enabled: false,
            max_command_timeout_seconds: 60,
        }
    }
}

/// LSP launch policy controls.
#[derive(Debug, Clone)]
pub struct LspLaunchPolicy {
    /// Trusted workspaces only by default.
    pub require_trusted_workspace: bool,
    /// Allowed LSP command binaries.
    pub allowed_binaries: Vec<String>,
    /// Deny launch when command looks like networked update.
    pub deny_network_refresh: bool,
}

impl Default for LspLaunchPolicy {
    fn default() -> Self {
        Self {
            require_trusted_workspace: true,
            allowed_binaries: vec!["rust-analyzer".to_string(), "rustc".to_string()],
            deny_network_refresh: true,
        }
    }
}

/// Plugin capability policy controls.
#[derive(Debug, Clone)]
pub struct PluginCapabilityPolicy {
    /// Allowed plugin host capabilities. Unknown capabilities remain denied.
    pub allowed_capabilities: HashSet<String>,
    /// Required namespace if capability requested.
    pub namespace_required: bool,
    /// Allow plugins in untrusted workspaces.
    pub allow_in_untrusted_workspace: bool,
    /// Deny all plugin network host calls, including in trusted workspaces.
    pub deny_network: bool,
    /// Deny process/filesystem/terminal-like host authority.
    pub deny_ambient_host_authority: bool,
}

impl Default for PluginCapabilityPolicy {
    fn default() -> Self {
        Self {
            allowed_capabilities: HashSet::from([
                "plugin.command".to_string(),
                "plugin.context.read".to_string(),
                "plugin.semantic.query".to_string(),
                "plugin.contribution.register".to_string(),
                "plugin.proposal.create".to_string(),
                "plugin.event.emit".to_string(),
                "plugin.cancel.check".to_string(),
                "plugin.storage".to_string(),
            ]),
            namespace_required: true,
            allow_in_untrusted_workspace: false,
            deny_network: true,
            deny_ambient_host_authority: true,
        }
    }
}

/// Collaboration capability policy controls.
#[derive(Debug, Clone)]
pub struct CollaborationCapabilityPolicy {
    /// Allowed collaboration capabilities. Unknown capabilities remain denied.
    pub allowed_capabilities: HashSet<String>,
    /// Require trusted workspace for all collaboration actions.
    pub require_trusted_workspace: bool,
    /// Whether runtime session creation/join and operation publish are enabled.
    pub runtime_sessions_enabled: bool,
    /// Whether metadata-only presence publication is enabled.
    pub presence_enabled: bool,
    /// Whether shared proposal approval is enabled.
    pub shared_proposal_approval_enabled: bool,
    /// Whether metadata-only audit export is enabled.
    pub audit_export_enabled: bool,
}

/// Remote-development capability policy controls.
#[derive(Debug, Clone)]
pub struct RemoteDevelopmentPolicy {
    /// Allowed remote capabilities. Unknown capabilities remain denied.
    pub allowed_capabilities: HashSet<String>,
    /// Require trusted workspace for all remote-development actions.
    pub require_trusted_workspace: bool,
    /// Whether remote workspace sessions may connect.
    pub runtime_sessions_enabled: bool,
    /// Whether remote filesystem read and proposal-mediated mutation requests are enabled.
    pub filesystem_enabled: bool,
    /// Whether remote process, PTY, and terminal descriptors are enabled.
    pub execution_enabled: bool,
    /// Whether remote LSP descriptors are enabled.
    pub lsp_enabled: bool,
    /// Whether remote semantic-query descriptors are enabled.
    pub semantic_query_enabled: bool,
    /// Whether metadata-only remote audit export is enabled.
    pub audit_export_enabled: bool,
    /// Whether remote agent package activation is enabled.
    pub agent_package_activation_enabled: bool,
    /// Whether offline resume manifests are enabled.
    pub offline_resume_enabled: bool,
}

/// Legion Cloud Lane capability policy controls.
#[derive(Debug, Clone)]
pub struct CloudLaneSecurityPolicy {
    /// Allowed cloud-lane capabilities. Unknown capabilities remain denied.
    pub allowed_capabilities: HashSet<String>,
    /// Require trusted workspace for all cloud lane actions.
    pub require_trusted_workspace: bool,
    /// Whether cloud task submission is enabled.
    pub task_submission_enabled: bool,
    /// Whether cloud event streaming is enabled.
    pub event_stream_enabled: bool,
    /// Whether cloud cancellation is enabled.
    pub cancellation_enabled: bool,
    /// Whether cloud proposal/evidence fetch is enabled.
    pub artifact_fetch_enabled: bool,
    /// Maximum upload bytes for one cloud task.
    pub max_upload_bytes: u64,
    /// Maximum cloud cost in cents for one task.
    pub max_cost_cents: u32,
    /// Require visible upload scope before submit.
    pub require_scope_visibility: bool,
    /// Require prior task-packet validation before submit.
    pub require_validated_task_packet: bool,
    /// Require a hard cost cap before submit.
    pub require_hard_cost_cap: bool,
}

impl Default for CloudLaneSecurityPolicy {
    fn default() -> Self {
        Self {
            allowed_capabilities: HashSet::from([
                "cloud.lane.submit".to_string(),
                "cloud.lane.events.stream".to_string(),
                "cloud.lane.cancel".to_string(),
                "cloud.lane.proposal.fetch".to_string(),
                "cloud.lane.evidence.fetch".to_string(),
                "cloud.usage.meter".to_string(),
            ]),
            require_trusted_workspace: true,
            task_submission_enabled: false,
            event_stream_enabled: false,
            cancellation_enabled: false,
            artifact_fetch_enabled: false,
            max_upload_bytes: 0,
            max_cost_cents: 0,
            require_scope_visibility: true,
            require_validated_task_packet: true,
            require_hard_cost_cap: true,
        }
    }
}

impl Default for RemoteDevelopmentPolicy {
    fn default() -> Self {
        Self {
            allowed_capabilities: HashSet::from([
                "remote.session.connect".to_string(),
                "remote.fs.read".to_string(),
                "remote.fs.write".to_string(),
                "remote.process.launch".to_string(),
                "remote.pty.input".to_string(),
                "remote.terminal.access".to_string(),
                "remote.lsp.launch".to_string(),
                "remote.semantic.query".to_string(),
                "remote.cache.access".to_string(),
                "remote.egress".to_string(),
                "remote.audit.export".to_string(),
                "remote.offline.resume".to_string(),
                "remote.transport.connect".to_string(),
                "remote.transport.listen".to_string(),
                "remote.agent.package.activate".to_string(),
            ]),
            require_trusted_workspace: true,
            runtime_sessions_enabled: false,
            filesystem_enabled: false,
            execution_enabled: false,
            lsp_enabled: false,
            semantic_query_enabled: false,
            audit_export_enabled: false,
            agent_package_activation_enabled: false,
            offline_resume_enabled: false,
        }
    }
}

/// Hosted telemetry policy controls.
#[derive(Debug, Clone)]
pub struct HostedTelemetryPolicy {
    /// Whether hosted export is enabled at all.
    pub export_enabled: bool,
    /// Whether metadata-only spool writes are enabled at all.
    pub spool_write_enabled: bool,
    /// Whether explicit consent is required before export.
    pub require_explicit_consent: bool,
    /// Allowed telemetry capabilities. Unknown capabilities remain denied.
    pub allowed_capabilities: HashSet<String>,
}

impl Default for HostedTelemetryPolicy {
    fn default() -> Self {
        Self {
            export_enabled: false,
            spool_write_enabled: false,
            require_explicit_consent: true,
            allowed_capabilities: HashSet::from([
                "telemetry.export.hosted".to_string(),
                "telemetry.spool.write".to_string(),
                "telemetry.consent.revoke".to_string(),
            ]),
        }
    }
}

/// Raw-source retention policy controls.
#[derive(Debug, Clone)]
pub struct RawSourceRetentionSecurityPolicy {
    /// Whether raw-source capture is enabled at all.
    pub capture_enabled: bool,
    /// Whether explicit consent is required before capture or access.
    pub require_explicit_consent: bool,
    /// Allowed retention capabilities. Unknown capabilities remain denied.
    pub allowed_capabilities: HashSet<String>,
}

impl Default for RawSourceRetentionSecurityPolicy {
    fn default() -> Self {
        Self {
            capture_enabled: false,
            require_explicit_consent: true,
            allowed_capabilities: HashSet::from([
                "retention.raw_source.capture".to_string(),
                "retention.raw_source.read".to_string(),
                "retention.raw_source.delete".to_string(),
                "retention.raw_source.export".to_string(),
                "retention.raw_source.export.hosted".to_string(),
            ]),
        }
    }
}

/// Storage migration and repair policy controls.
#[derive(Debug, Clone)]
pub struct StorageMigrationSecurityPolicy {
    /// Allowed storage migration capabilities. Unknown capabilities remain denied.
    pub allowed_capabilities: HashSet<String>,
    /// Whether mutation-capable migration apply is enabled.
    pub apply_enabled: bool,
    /// Whether repair operations are enabled.
    pub repair_enabled: bool,
    /// Whether repair requires an explicit operator repair flag.
    pub explicit_repair_required: bool,
}

impl Default for StorageMigrationSecurityPolicy {
    fn default() -> Self {
        Self {
            allowed_capabilities: HashSet::from([
                "storage.migration.apply".to_string(),
                "storage.migration.repair".to_string(),
            ]),
            apply_enabled: false,
            repair_enabled: false,
            explicit_repair_required: true,
        }
    }
}

impl Default for CollaborationCapabilityPolicy {
    fn default() -> Self {
        Self {
            allowed_capabilities: HashSet::from([
                "collaboration.session.create".to_string(),
                "collaboration.session.join".to_string(),
                "collaboration.operation.publish".to_string(),
                "collaboration.presence.publish".to_string(),
                "collaboration.proposal.approve".to_string(),
                "collaboration.replay.metadata".to_string(),
                "collaboration.audit.export".to_string(),
            ]),
            require_trusted_workspace: true,
            runtime_sessions_enabled: false,
            presence_enabled: false,
            shared_proposal_approval_enabled: false,
            audit_export_enabled: false,
        }
    }
}

/// File-write policy controls.
#[derive(Debug, Clone)]
pub struct FileWritePolicy {
    /// Allowed write operations by principal and trust state.
    pub deny_when_untrusted: bool,
    /// Blocked file suffixes.
    pub blocked_extensions: HashSet<String>,
    /// Maximum bytes written per file at once.
    pub max_bytes_per_write: usize,
}

impl Default for FileWritePolicy {
    fn default() -> Self {
        Self {
            deny_when_untrusted: true,
            blocked_extensions: HashSet::from([".exe".to_string(), ".dll".to_string()]),
            max_bytes_per_write: 4 * 1024 * 1024,
        }
    }
}

/// Network policy controls.
#[derive(Debug, Clone)]
pub struct NetworkPolicy {
    /// Allow outbound network only from trusted workspaces.
    pub allow_untrusted: bool,
    /// Allowed host allowlist for network access.
    pub allowlist: Vec<String>,
    /// Deny explicit host blocklist.
    pub blocklist: Vec<String>,
    /// Air-gap mode blocks hosted provider, telemetry, gateway, and non-loopback egress.
    pub air_gap: bool,
    /// Provider invocation is restricted to local or loopback targets.
    pub local_provider_only: bool,
}

impl Default for NetworkPolicy {
    fn default() -> Self {
        Self {
            allow_untrusted: false,
            allowlist: vec!["localhost".to_string()],
            blocklist: vec!["example.exfiltration.invalid".to_string()],
            air_gap: true,
            local_provider_only: true,
        }
    }
}

/// AI provider policy controls.
#[derive(Debug, Clone)]
pub struct AiProviderPolicy {
    /// Whether provider invocation is enabled at all.
    pub provider_invocation_enabled: bool,
    /// Whether local provider invocation is allowed.
    pub allow_local_provider: bool,
    /// Whether remote/cloud provider invocation is allowed.
    pub allow_remote_provider: bool,
    /// Deny provider capability requests in untrusted workspaces.
    pub deny_when_untrusted: bool,
}

impl Default for AiProviderPolicy {
    fn default() -> Self {
        Self {
            provider_invocation_enabled: true,
            allow_local_provider: true,
            allow_remote_provider: false,
            deny_when_untrusted: true,
        }
    }
}

/// Domain-level root policy.
#[derive(Debug, Clone, Default)]
pub struct SecurityPolicy {
    /// Path access policy.
    pub path_policy: PathPolicy,
    /// Command taxonomy.
    pub command_taxonomy: CommandTaxonomy,
    /// Terminal policy.
    pub terminal_policy: TerminalPolicy,
    /// LSP launch policy.
    pub lsp_policy: LspLaunchPolicy,
    /// Plugin capability policy.
    pub plugin_policy: PluginCapabilityPolicy,
    /// File write policy.
    pub file_write_policy: FileWritePolicy,
    /// Network policy.
    pub network_policy: NetworkPolicy,
    /// AI provider policy.
    pub ai_provider_policy: AiProviderPolicy,
    /// Collaboration policy.
    pub collaboration_policy: CollaborationCapabilityPolicy,
    /// Remote-development policy.
    pub remote_policy: RemoteDevelopmentPolicy,
    /// Legion Cloud Lane policy.
    pub cloud_lane_policy: CloudLaneSecurityPolicy,
    /// Hosted telemetry policy.
    pub telemetry_policy: HostedTelemetryPolicy,
    /// Raw-source retention policy.
    pub retention_policy: RawSourceRetentionSecurityPolicy,
    /// Storage migration and repair policy.
    pub storage_migration_policy: StorageMigrationSecurityPolicy,
}

/// Security errors.
#[derive(Debug, Error)]
pub enum SecurityError {
    /// Decision denied by policy.
    #[error("denied: {reason}")]
    Denied {
        /// Human-readable denial reason.
        reason: String,
    },
    /// Malformed request.
    #[error("malformed request: {reason}")]
    Malformed {
        /// Human-readable parsing/validation issue.
        reason: String,
    },
}

/// Explicit allow/deny decision used by matrix tests.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SecurityDecision {
    /// Decision approved.
    Allow,
    /// Decision denied.
    Deny(String),
}

impl SecurityDecision {
    /// Converts decision into protocol decision response.
    pub fn into_protocol(
        self,
        decision_id: CapabilityDecisionId,
        _principal: PrincipalId,
        capability: CapabilityId,
    ) -> CapabilityResponse {
        match self {
            Self::Allow => CapabilityResponse::Decision(CapabilityDecision {
                decision_id,
                granted: true,
                capability,
                reason: Some("policy allowed".to_string()),
            }),
            Self::Deny(reason) => CapabilityResponse::Decision(CapabilityDecision {
                decision_id,
                granted: false,
                capability,
                reason: Some(reason),
            }),
        }
    }

    /// Converts decision to broker grant/deny records for audit.
    pub fn into_capability_records(
        self,
        decision_id: CapabilityDecisionId,
        principal: PrincipalId,
        capability: CapabilityId,
        namespace: CapabilityNamespace,
        _correlation: CorrelationId,
    ) -> (Option<CapabilityGrant>, Option<CapabilityDenial>) {
        match self {
            Self::Allow => (
                Some(CapabilityGrant {
                    decision_id,
                    principal_id: principal,
                    capability_id: capability,
                    namespace,
                    expires_at: None,
                }),
                None,
            ),
            Self::Deny(reason) => (
                None,
                Some(CapabilityDenial {
                    decision_id,
                    principal_id: principal,
                    capability_id: capability,
                    reason,
                }),
            ),
        }
    }
}

impl SecurityDecision {
    fn deny(reason: impl Into<String>) -> Self {
        Self::Deny(reason.into())
    }

    fn allow() -> Self {
        Self::Allow
    }
}

/// Evaluate whether a product mode may request a capability before detailed policy checks.
pub fn product_mode_capability_decision(
    mode: ProductMode,
    capability: &CapabilityId,
) -> SecurityDecision {
    let surface = product_runtime_surface_for_capability(capability);
    if mode.allows_runtime_surface(surface) {
        SecurityDecision::Allow
    } else {
        SecurityDecision::deny(format!(
            "{} mode denies {:?} capability surface",
            mode.label(),
            surface
        ))
    }
}

/// Deny-by-default broker stub.
#[derive(Debug, Clone)]
pub struct DenyByDefaultBroker {
    /// Optional static policy override set.
    pub policy: SecurityPolicy,
    /// Namespace for all generated decisions.
    pub namespace: CapabilityNamespace,
    counter: u64,
}

impl Default for DenyByDefaultBroker {
    fn default() -> Self {
        Self {
            policy: SecurityPolicy::default(),
            namespace: CapabilityNamespace("default".to_string()),
            counter: 0,
        }
    }
}

impl DenyByDefaultBroker {
    /// Construct stub with explicit policy.
    pub fn new(policy: SecurityPolicy, namespace: CapabilityNamespace) -> Self {
        Self {
            policy,
            namespace,
            counter: 0,
        }
    }

    /// Pure policy matrix for a capability request.
    pub fn decide(
        &mut self,
        trust: TrustState,
        principal: PrincipalId,
        capability: CapabilityId,
        path: Option<&str>,
    ) -> SecurityDecision {
        self.decide_with_request_context(
            trust,
            principal,
            capability,
            path,
            CapabilityRequestContext::default(),
        )
    }

    /// Pure policy matrix for a capability request with structured operation context.
    pub fn decide_with_request_context(
        &mut self,
        trust: TrustState,
        principal: PrincipalId,
        capability: CapabilityId,
        path: Option<&str>,
        context: CapabilityRequestContext,
    ) -> SecurityDecision {
        self.counter = self.counter.saturating_add(1);
        let decision_id = CapabilityDecisionId(self.counter);

        if !self.namespace_policy_enabled(&self.namespace) {
            return SecurityDecision::deny(format!("namespace {} disabled", self.namespace.0));
        }

        self.decide_with_context(trust, principal, capability, path, &context, decision_id)
    }

    fn effective_max_write_bytes(&self) -> u64 {
        // Workspace saves honor both legacy path-level and file-write limits. The stricter limit
        // wins so either policy surface can safely constrain a write payload before disk mutation.
        (self.policy.path_policy.max_write_bytes as u64)
            .min(self.policy.file_write_policy.max_bytes_per_write as u64)
    }

    fn write_size_decision(&self, context: &CapabilityRequestContext) -> Option<SecurityDecision> {
        let write_byte_count = context.write_byte_count?;
        let effective_max = self.effective_max_write_bytes();
        if write_byte_count > effective_max {
            Some(SecurityDecision::deny(format!(
                "write payload {write_byte_count} bytes exceeds configured write-size limit {effective_max} bytes"
            )))
        } else {
            None
        }
    }

    fn is_loopback_host(host: &str) -> bool {
        matches!(
            host.to_ascii_lowercase().as_str(),
            "localhost" | "127.0.0.1" | "::1"
        )
    }

    fn host_matches_configured(pattern: &str, host: &str) -> bool {
        pattern.eq_ignore_ascii_case(host)
    }

    fn network_target_decision(&self, context: &CapabilityRequestContext) -> SecurityDecision {
        let Some(target) = &context.network_target else {
            return SecurityDecision::deny("network target metadata required by policy");
        };

        if target.scheme != "http" && target.scheme != "https" {
            return SecurityDecision::deny("network scheme denied by policy");
        }

        if self
            .policy
            .network_policy
            .blocklist
            .iter()
            .any(|host| Self::host_matches_configured(host, &target.host))
        {
            return SecurityDecision::deny("network host blocked by policy");
        }

        if self.policy.network_policy.air_gap && !Self::is_loopback_host(&target.host) {
            return SecurityDecision::deny("air-gap mode denies non-loopback network access");
        }

        if self.policy.network_policy.local_provider_only && !Self::is_loopback_host(&target.host) {
            return SecurityDecision::deny("local-provider-only mode denies remote network access");
        }

        if self
            .policy
            .network_policy
            .allowlist
            .iter()
            .any(|host| Self::host_matches_configured(host, &target.host))
        {
            SecurityDecision::allow()
        } else {
            SecurityDecision::deny("network host not allowlisted by policy")
        }
    }

    fn require_https_network_target(
        &self,
        context: &CapabilityRequestContext,
    ) -> Option<SecurityDecision> {
        let Some(target) = &context.network_target else {
            return Some(SecurityDecision::deny(
                "network target metadata required by policy",
            ));
        };

        if target.scheme != "https" {
            return Some(SecurityDecision::deny(
                "production Phase 8 egress requires HTTPS transport",
            ));
        }

        None
    }

    fn ai_capability_decision(
        &self,
        trust: TrustState,
        capability: &str,
        context: &CapabilityRequestContext,
    ) -> SecurityDecision {
        if self.policy.ai_provider_policy.deny_when_untrusted && trust != TrustState::Trusted {
            return SecurityDecision::deny("AI capability denied for untrusted workspace");
        }

        match capability {
            "ai.provider.invoke" | "ai.provider.stream" => {
                if !self.policy.ai_provider_policy.provider_invocation_enabled {
                    return SecurityDecision::deny("AI provider invocation disabled by policy");
                }
                if let Some(target) = &context.network_target {
                    let loopback = Self::is_loopback_host(&target.host);
                    if loopback && !self.policy.ai_provider_policy.allow_local_provider {
                        return SecurityDecision::deny(
                            "local AI provider invocation disabled by policy",
                        );
                    }
                    if !loopback && !self.policy.ai_provider_policy.allow_remote_provider {
                        return SecurityDecision::deny(
                            "remote AI provider invocation disabled by policy",
                        );
                    }
                }
                if self.policy.network_policy.air_gap
                    && context
                        .network_target
                        .as_ref()
                        .is_some_and(|target| !Self::is_loopback_host(&target.host))
                {
                    return SecurityDecision::deny(
                        "air-gap mode denies hosted provider invocation",
                    );
                }
                self.network_target_decision(context)
            }
            "ai.provider.cancel" | "ai.context.assemble" | "ai.proposal.create" => {
                SecurityDecision::allow()
            }
            "tracker.write" | "memory.candidate.write" | "tool.plan" => SecurityDecision::allow(),
            "memory.retain" => SecurityDecision::deny("memory retention requires explicit consent"),
            _ => {
                SecurityDecision::deny(format!("capability {capability} denied by deny-by-default"))
            }
        }
    }

    fn collaboration_capability_decision(
        &self,
        trust: TrustState,
        capability: &str,
        context: &CapabilityRequestContext,
    ) -> SecurityDecision {
        if self.policy.collaboration_policy.require_trusted_workspace
            && trust != TrustState::Trusted
        {
            return SecurityDecision::deny(
                "collaboration capability denied for untrusted workspace",
            );
        }
        if !self
            .policy
            .collaboration_policy
            .allowed_capabilities
            .contains(capability)
        {
            return SecurityDecision::deny(format!(
                "capability {capability} denied by deny-by-default"
            ));
        }
        if let Some(target) = &context.network_target
            && (self.policy.network_policy.air_gap
                || self.policy.network_policy.local_provider_only)
            && !Self::is_loopback_host(&target.host)
        {
            return SecurityDecision::deny(
                "collaboration transport cannot use non-loopback egress in air-gap policy",
            );
        }

        match capability {
            "collaboration.presence.publish" => {
                if self.policy.collaboration_policy.presence_enabled {
                    SecurityDecision::allow()
                } else {
                    SecurityDecision::deny("collaboration presence is disabled by policy")
                }
            }
            "collaboration.proposal.approve" => {
                if self
                    .policy
                    .collaboration_policy
                    .shared_proposal_approval_enabled
                {
                    SecurityDecision::allow()
                } else {
                    SecurityDecision::deny("collaboration shared proposal approval is disabled")
                }
            }
            "collaboration.replay.metadata" | "collaboration.audit.export" => {
                if self.policy.collaboration_policy.audit_export_enabled {
                    SecurityDecision::allow()
                } else {
                    SecurityDecision::deny("collaboration replay/audit export is disabled")
                }
            }
            "collaboration.session.create"
            | "collaboration.session.join"
            | "collaboration.operation.publish" => {
                if self.policy.collaboration_policy.runtime_sessions_enabled {
                    SecurityDecision::allow()
                } else {
                    SecurityDecision::deny("collaboration runtime sessions are disabled by policy")
                }
            }
            _ => {
                SecurityDecision::deny(format!("capability {capability} denied by deny-by-default"))
            }
        }
    }

    fn remote_capability_decision(
        &self,
        trust: TrustState,
        capability: &str,
        context: &CapabilityRequestContext,
    ) -> SecurityDecision {
        if self.policy.remote_policy.require_trusted_workspace && trust != TrustState::Trusted {
            return SecurityDecision::deny("remote capability denied for untrusted workspace");
        }
        if !self
            .policy
            .remote_policy
            .allowed_capabilities
            .contains(capability)
        {
            return SecurityDecision::deny(format!(
                "capability {capability} denied by deny-by-default"
            ));
        }
        if let Some(target) = &context.network_target
            && (self.policy.network_policy.air_gap
                || self.policy.network_policy.local_provider_only)
            && !Self::is_loopback_host(&target.host)
        {
            return SecurityDecision::deny(
                "remote transport cannot use non-loopback egress in air-gap policy",
            );
        }

        match capability {
            "remote.session.connect" | "remote.transport.connect" => {
                if !self.policy.remote_policy.runtime_sessions_enabled {
                    return SecurityDecision::deny("remote sessions are disabled by policy");
                }
                if let Some(decision) = self.require_https_network_target(context) {
                    return decision;
                }
                self.network_target_decision(context)
            }
            "remote.transport.listen" => SecurityDecision::deny(
                "remote inbound transport listen is not enabled in Phase 8 policy",
            ),
            "remote.agent.package.activate" => {
                if self.policy.remote_policy.runtime_sessions_enabled
                    && self.policy.remote_policy.agent_package_activation_enabled
                {
                    SecurityDecision::allow()
                } else {
                    SecurityDecision::deny("remote agent package activation is disabled by policy")
                }
            }
            "remote.fs.read" | "remote.fs.write" => {
                if self.policy.remote_policy.runtime_sessions_enabled
                    && self.policy.remote_policy.filesystem_enabled
                {
                    if capability == "remote.fs.write"
                        && let Some(decision) = self.write_size_decision(context)
                    {
                        return decision;
                    }
                    SecurityDecision::allow()
                } else {
                    SecurityDecision::deny("remote filesystem is disabled by policy")
                }
            }
            "remote.process.launch" | "remote.pty.input" | "remote.terminal.access" => {
                if self.policy.remote_policy.runtime_sessions_enabled
                    && self.policy.remote_policy.execution_enabled
                {
                    SecurityDecision::allow()
                } else {
                    SecurityDecision::deny("remote execution is disabled by policy")
                }
            }
            "remote.lsp.launch" => {
                if self.policy.remote_policy.runtime_sessions_enabled
                    && self.policy.remote_policy.lsp_enabled
                {
                    SecurityDecision::allow()
                } else {
                    SecurityDecision::deny("remote LSP is disabled by policy")
                }
            }
            "remote.semantic.query" => {
                if self.policy.remote_policy.runtime_sessions_enabled
                    && self.policy.remote_policy.semantic_query_enabled
                {
                    SecurityDecision::allow()
                } else {
                    SecurityDecision::deny("remote semantic query is disabled by policy")
                }
            }
            "remote.cache.access" => {
                if self.policy.remote_policy.runtime_sessions_enabled {
                    SecurityDecision::allow()
                } else {
                    SecurityDecision::deny("remote sessions are disabled by policy")
                }
            }
            "remote.egress" => {
                if !self.policy.remote_policy.runtime_sessions_enabled {
                    return SecurityDecision::deny("remote sessions are disabled by policy");
                }
                self.network_target_decision(context)
            }
            "remote.audit.export" => {
                if self.policy.remote_policy.audit_export_enabled {
                    SecurityDecision::allow()
                } else {
                    SecurityDecision::deny("remote audit export is disabled by policy")
                }
            }
            "remote.offline.resume" => {
                if self.policy.remote_policy.runtime_sessions_enabled
                    && self.policy.remote_policy.offline_resume_enabled
                {
                    SecurityDecision::allow()
                } else {
                    SecurityDecision::deny("remote offline resume is disabled by policy")
                }
            }
            _ => {
                SecurityDecision::deny(format!("capability {capability} denied by deny-by-default"))
            }
        }
    }

    fn telemetry_capability_decision(
        &self,
        trust: TrustState,
        capability: &str,
        context: &CapabilityRequestContext,
    ) -> SecurityDecision {
        if trust != TrustState::Trusted {
            return SecurityDecision::deny("hosted telemetry denied for untrusted workspace");
        }
        if !self
            .policy
            .telemetry_policy
            .allowed_capabilities
            .contains(capability)
        {
            return SecurityDecision::deny(format!(
                "capability {capability} denied by deny-by-default"
            ));
        }
        match capability {
            "telemetry.export.hosted" => {
                if !self.policy.telemetry_policy.export_enabled {
                    return SecurityDecision::deny("hosted telemetry export is disabled by policy");
                }
                if self.policy.telemetry_policy.require_explicit_consent
                    && !context.hosted_telemetry_consent_current
                {
                    return SecurityDecision::deny(
                        "hosted telemetry export requires current explicit consent",
                    );
                }
                if let Some(decision) = self.require_https_network_target(context) {
                    return decision;
                }
                self.network_target_decision(context)
            }
            "telemetry.spool.write" => {
                if !self.policy.telemetry_policy.spool_write_enabled {
                    return SecurityDecision::deny(
                        "hosted telemetry spool writes are disabled by policy",
                    );
                }
                if self.policy.telemetry_policy.require_explicit_consent
                    && !context.hosted_telemetry_consent_current
                {
                    return SecurityDecision::deny(
                        "hosted telemetry spool write requires current explicit consent",
                    );
                }
                SecurityDecision::allow()
            }
            "telemetry.consent.revoke" => SecurityDecision::allow(),
            _ => {
                SecurityDecision::deny(format!("capability {capability} denied by deny-by-default"))
            }
        }
    }

    fn retention_capability_decision(
        &self,
        trust: TrustState,
        capability: &str,
        context: &CapabilityRequestContext,
    ) -> SecurityDecision {
        if trust != TrustState::Trusted {
            return SecurityDecision::deny("raw-source retention denied for untrusted workspace");
        }
        if !self
            .policy
            .retention_policy
            .allowed_capabilities
            .contains(capability)
        {
            return SecurityDecision::deny(format!(
                "capability {capability} denied by deny-by-default"
            ));
        }
        match capability {
            "retention.raw_source.capture"
            | "retention.raw_source.read"
            | "retention.raw_source.export" => {
                if !self.policy.retention_policy.capture_enabled {
                    return SecurityDecision::deny("raw-source retention is disabled by policy");
                }
                if self.policy.retention_policy.require_explicit_consent
                    && !context.raw_source_retention_consent_current
                {
                    return SecurityDecision::deny(
                        "raw-source retention requires current explicit consent",
                    );
                }
                SecurityDecision::allow()
            }
            "retention.raw_source.export.hosted" => {
                if !self.policy.retention_policy.capture_enabled {
                    return SecurityDecision::deny(
                        "raw-source hosted export is disabled by policy",
                    );
                }
                if self.policy.retention_policy.require_explicit_consent
                    && (!context.raw_source_retention_consent_current
                        || !context.raw_source_hosted_export_consent_current)
                {
                    return SecurityDecision::deny(
                        "raw-source hosted export requires current local and hosted consent",
                    );
                }
                if let Some(decision) = self.require_https_network_target(context) {
                    return decision;
                }
                self.network_target_decision(context)
            }
            "retention.raw_source.delete" => SecurityDecision::allow(),
            _ => {
                SecurityDecision::deny(format!("capability {capability} denied by deny-by-default"))
            }
        }
    }

    fn storage_migration_capability_decision(
        &self,
        trust: TrustState,
        capability: &str,
        context: &CapabilityRequestContext,
    ) -> SecurityDecision {
        if trust != TrustState::Trusted {
            return SecurityDecision::deny("storage migration denied for untrusted workspace");
        }
        if !self
            .policy
            .storage_migration_policy
            .allowed_capabilities
            .contains(capability)
        {
            return SecurityDecision::deny(format!(
                "capability {capability} denied by deny-by-default"
            ));
        }
        match capability {
            "storage.migration.apply" => {
                if !self.policy.storage_migration_policy.apply_enabled {
                    SecurityDecision::deny("storage migration apply is disabled by policy")
                } else if context.storage_explicit_apply {
                    SecurityDecision::allow()
                } else {
                    SecurityDecision::deny("storage migration apply requires explicit apply flag")
                }
            }
            "storage.migration.repair" => {
                if !self.policy.storage_migration_policy.repair_enabled {
                    return SecurityDecision::deny(
                        "storage migration repair is disabled by policy",
                    );
                }
                if self
                    .policy
                    .storage_migration_policy
                    .explicit_repair_required
                {
                    if context.storage_explicit_repair {
                        SecurityDecision::allow()
                    } else {
                        SecurityDecision::deny(
                            "storage migration repair requires explicit repair flag",
                        )
                    }
                } else {
                    SecurityDecision::allow()
                }
            }
            _ => {
                SecurityDecision::deny(format!("capability {capability} denied by deny-by-default"))
            }
        }
    }

    fn cloud_lane_capability_decision(
        &self,
        trust: TrustState,
        capability: &str,
        context: &CapabilityRequestContext,
    ) -> SecurityDecision {
        let policy = &self.policy.cloud_lane_policy;
        if policy.require_trusted_workspace && trust != TrustState::Trusted {
            return SecurityDecision::deny("cloud lane capability denied for untrusted workspace");
        }
        if !policy.allowed_capabilities.contains(capability) {
            return SecurityDecision::deny(format!(
                "capability {capability} denied by deny-by-default"
            ));
        }

        match capability {
            "cloud.lane.submit" => {
                if !policy.task_submission_enabled {
                    return SecurityDecision::deny(
                        "cloud lane task submission is disabled by policy",
                    );
                }
                if policy.require_scope_visibility && !context.cloud_lane_scope_visible_to_user {
                    return SecurityDecision::deny(
                        "cloud lane upload scope must be visible before submit",
                    );
                }
                if context.cloud_lane_forbidden_upload_count > 0 {
                    return SecurityDecision::deny("cloud lane upload contains forbidden material");
                }
                if policy.require_validated_task_packet && !context.cloud_lane_task_packet_validated
                {
                    return SecurityDecision::deny(
                        "cloud lane submit requires a validated task packet",
                    );
                }
                if policy.require_hard_cost_cap && !context.cloud_lane_hard_cap_enforced {
                    return SecurityDecision::deny("cloud lane submit requires a hard cost cap");
                }
                let Some(estimated_cost_cents) = context.cloud_lane_estimated_cost_cents else {
                    return SecurityDecision::deny("cloud lane submit requires an estimated cost");
                };
                if policy.max_cost_cents == 0 || estimated_cost_cents > policy.max_cost_cents {
                    return SecurityDecision::deny(
                        "cloud lane estimated cost exceeds configured cost cap",
                    );
                }
                let Some(upload_bytes) = context.cloud_lane_upload_bytes else {
                    return SecurityDecision::deny("cloud lane submit requires upload byte count");
                };
                if policy.max_upload_bytes == 0 || upload_bytes > policy.max_upload_bytes {
                    return SecurityDecision::deny(
                        "cloud lane upload bytes exceed configured upload cap",
                    );
                }
                if let Some(decision) = self.require_https_network_target(context) {
                    return decision;
                }
                self.network_target_decision(context)
            }
            "cloud.lane.events.stream" => {
                if policy.event_stream_enabled {
                    SecurityDecision::allow()
                } else {
                    SecurityDecision::deny("cloud lane event streaming is disabled by policy")
                }
            }
            "cloud.lane.cancel" => {
                if policy.cancellation_enabled {
                    SecurityDecision::allow()
                } else {
                    SecurityDecision::deny("cloud lane cancellation is disabled by policy")
                }
            }
            "cloud.lane.proposal.fetch" | "cloud.lane.evidence.fetch" => {
                if policy.artifact_fetch_enabled {
                    SecurityDecision::allow()
                } else {
                    SecurityDecision::deny("cloud lane artifact fetch is disabled by policy")
                }
            }
            "cloud.usage.meter" => {
                if policy.task_submission_enabled {
                    SecurityDecision::allow()
                } else {
                    SecurityDecision::deny("cloud usage metering is disabled by policy")
                }
            }
            _ => {
                SecurityDecision::deny(format!("capability {capability} denied by deny-by-default"))
            }
        }
    }

    fn decide_with_context(
        &self,
        trust: TrustState,
        principal: PrincipalId,
        capability: CapabilityId,
        path: Option<&str>,
        context: &CapabilityRequestContext,
        _decision_id: CapabilityDecisionId,
    ) -> SecurityDecision {
        let capability = capability.0;

        if Self::phase8_sensitive_capability_requires_principal(&capability)
            && principal.0.trim().is_empty()
        {
            return SecurityDecision::deny(format!(
                "capability {capability} denied: principal is required"
            ));
        }

        if capability.starts_with("ai.")
            || capability.starts_with("tracker.")
            || capability.starts_with("memory.")
            || capability == "tool.plan"
        {
            return self.ai_capability_decision(trust, &capability, context);
        }

        if capability.starts_with("collaboration.") {
            return self.collaboration_capability_decision(trust, &capability, context);
        }

        if capability.starts_with("remote.") {
            return self.remote_capability_decision(trust, &capability, context);
        }

        if capability.starts_with("telemetry.") {
            return self.telemetry_capability_decision(trust, &capability, context);
        }

        if capability.starts_with("retention.raw_source.") {
            return self.retention_capability_decision(trust, &capability, context);
        }

        if capability.starts_with("storage.migration.") {
            return self.storage_migration_capability_decision(trust, &capability, context);
        }

        if capability.starts_with("cloud.") {
            return self.cloud_lane_capability_decision(trust, &capability, context);
        }

        if capability.starts_with("plugin.") {
            if self.policy.plugin_policy.deny_ambient_host_authority
                && matches!(
                    capability.as_str(),
                    "plugin.fs" | "plugin.process" | "plugin.terminal"
                )
            {
                return SecurityDecision::deny("plugin ambient host authority denied by policy");
            }
            if self.policy.plugin_policy.deny_network && capability == "plugin.network" {
                return SecurityDecision::deny(
                    "plugin network capability denied by air-gap policy",
                );
            }
            if !self
                .policy
                .plugin_policy
                .allowed_capabilities
                .contains(&capability)
            {
                return SecurityDecision::deny(format!(
                    "capability {capability} denied by deny-by-default"
                ));
            }
            if self.policy.plugin_policy.namespace_required
                && (context.plugin_namespace.is_none()
                    || context.plugin_id.is_none()
                    || context.plugin_host_call_name.is_none()
                    || context.plugin_module_hash.is_none()
                    || context.plugin_manifest_id.is_none()
                    || context.plugin_declared_capability_id.is_none()
                    || context.plugin_sandbox_operation_class.is_none())
            {
                return SecurityDecision::deny("plugin manifest and host-call context required");
            }
            if context
                .plugin_declared_capability_id
                .as_ref()
                .is_some_and(|declared| declared.0 != capability)
            {
                return SecurityDecision::deny(
                    "plugin host call capability does not match declaration",
                );
            }
            if !self.policy.plugin_policy.allow_in_untrusted_workspace
                && trust != TrustState::Trusted
            {
                return SecurityDecision::deny("plugin capability denied for untrusted workspace");
            }
            return SecurityDecision::allow();
        }

        if let Some(rest) = capability.strip_prefix("fs.") {
            return if rest == "write" {
                if self.policy.file_write_policy.deny_when_untrusted && trust != TrustState::Trusted
                {
                    SecurityDecision::deny("file write denied for untrusted workspace")
                } else if let Some(decision) = self.write_size_decision(context) {
                    decision
                } else if let Some(target_path) = path {
                    if !self
                        .policy
                        .path_policy
                        .can_access(target_path, PathAccess::Write)
                    {
                        SecurityDecision::deny("path write denied by policy")
                    } else if let Some(ext) = Path::new(target_path)
                        .extension()
                        .and_then(|ext| ext.to_str())
                    {
                        let ext = format!(".{ext}");
                        if self
                            .policy
                            .file_write_policy
                            .blocked_extensions
                            .contains(&ext)
                        {
                            SecurityDecision::deny("file extension blocked by policy")
                        } else {
                            SecurityDecision::allow()
                        }
                    } else {
                        SecurityDecision::allow()
                    }
                } else {
                    SecurityDecision::allow()
                }
            } else {
                SecurityDecision::allow()
            };
        }

        if capability.starts_with("terminal.") {
            if !self
                .policy
                .terminal_policy
                .allowed_capabilities
                .contains(&capability)
            {
                return SecurityDecision::deny(format!(
                    "capability {capability} denied by deny-by-default"
                ));
            }
            if !self.policy.terminal_policy.allow_untrusted && trust != TrustState::Trusted {
                return SecurityDecision::deny("terminal denied for untrusted workspace");
            }
            if !self.policy.terminal_policy.runtime_enabled {
                return SecurityDecision::deny("terminal runtime is disabled by policy");
            }
            if capability == "terminal.launch" {
                let Some(command) = context.command_binary.as_deref() else {
                    return SecurityDecision::deny("terminal launch requires command metadata");
                };
                if matches!(
                    self.policy.command_taxonomy.classify(command),
                    CommandClass::Network
                ) {
                    return SecurityDecision::deny(
                        "terminal launch denied for network-capable command",
                    );
                }
            }
            return SecurityDecision::allow();
        }

        if let Some(rest) = capability.strip_prefix("lsp.") {
            if self.policy.lsp_policy.require_trusted_workspace && trust != TrustState::Trusted {
                return SecurityDecision::deny("lsp launch denied for untrusted workspace");
            }

            return match rest {
                "launch" => SecurityDecision::allow(),
                _ => SecurityDecision::allow(),
            };
        }

        if let Some(rest) = capability.strip_prefix("network.") {
            if !self.policy.network_policy.allow_untrusted && trust != TrustState::Trusted {
                return SecurityDecision::deny("network denied for untrusted workspace");
            }

            if rest == "fetch" || rest == "egress" {
                return self.network_target_decision(context);
            }

            return SecurityDecision::allow();
        }

        if let Some(rest) = capability.strip_prefix("cmd.") {
            let class = self.policy.command_taxonomy.classify(rest);
            if matches!(
                class,
                CommandClass::Mutate | CommandClass::Terminal | CommandClass::Network
            ) && trust != TrustState::Trusted
            {
                SecurityDecision::deny(format!("command {rest} denied for untrusted workspace"))
            } else {
                SecurityDecision::allow()
            }
        } else {
            SecurityDecision::deny(format!(
                "capability {} denied by deny-by-default",
                capability
            ))
        }
    }

    fn namespace_policy_enabled(&self, namespace: &CapabilityNamespace) -> bool {
        !namespace.0.is_empty()
    }

    fn requires_trusted_workspace_for_request(&self, capability: &str) -> bool {
        if capability.starts_with("fs.write")
            || capability.starts_with("terminal.")
            || capability.starts_with("lsp.")
            || capability.starts_with("network.")
            || capability.starts_with("plugin.")
            || capability.starts_with("ai.")
            || capability.starts_with("tracker.")
            || capability.starts_with("memory.")
            || capability.starts_with("collaboration.")
            || capability.starts_with("remote.")
            || capability.starts_with("telemetry.")
            || capability.starts_with("retention.raw_source.")
            || capability.starts_with("storage.migration.")
        {
            return true;
        }

        if let Some(command) = capability.strip_prefix("cmd.") {
            return matches!(
                self.policy.command_taxonomy.classify(command),
                CommandClass::Mutate
                    | CommandClass::Terminal
                    | CommandClass::Network
                    | CommandClass::LanguageServer
            );
        }

        false
    }

    fn phase8_sensitive_capability_requires_principal(capability: &str) -> bool {
        capability.starts_with("remote.transport.")
            || capability == "remote.agent.package.activate"
            || capability.starts_with("terminal.")
            || capability.starts_with("telemetry.")
            || capability.starts_with("retention.raw_source.")
            || capability.starts_with("storage.migration.")
    }
}

impl CapabilityBrokerPort for DenyByDefaultBroker {
    fn handle(
        &self,
        request: CapabilityRequest,
    ) -> devil_protocol::ProtocolResult<CapabilityResponse> {
        let mut owned = self.clone();

        match request {
            CapabilityRequest::Request {
                principal_id,
                capability_id,
                workspace_trust_state,
                target_path,
                decision_id,
                context,
                correlation_id: _,
            } => {
                let trust_state: TrustState = workspace_trust_state.into();
                let decision = if trust_state == TrustState::Unknown
                    && owned.requires_trusted_workspace_for_request(&capability_id.0)
                {
                    owned.counter = owned.counter.saturating_add(1);
                    SecurityDecision::deny(format!(
                        "capability {} denied: workspace trust state is unknown",
                        capability_id.0
                    ))
                } else {
                    owned.decide_with_request_context(
                        trust_state,
                        principal_id.clone(),
                        capability_id.clone(),
                        target_path.as_ref().map(|value| value.0.as_str()),
                        context,
                    )
                };
                let resolved_decision_id =
                    decision_id.unwrap_or(CapabilityDecisionId(owned.counter));

                Ok(decision.into_protocol(resolved_decision_id, principal_id, capability_id))
            }
            CapabilityRequest::Grant(grant) => Ok(CapabilityResponse::Granted(grant)),
            CapabilityRequest::Deny(deny) => Ok(CapabilityResponse::Denied(deny)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use devil_protocol::{
        CapabilityRequest, DelegatedTaskToolPermissionDecision, DelegatedTaskToolPermissionProfile,
        FileFingerprint, McpToolDescriptor, McpToolName, PermissionBudgetActionClass,
        ProposalRiskLabel, RedactionHint,
    };

    fn cloud_lane_context(
        estimated_cost_cents: u32,
        upload_bytes: u64,
        scope_visible: bool,
        forbidden_upload_count: u32,
        task_packet_validated: bool,
    ) -> CapabilityRequestContext {
        CapabilityRequestContext {
            network_target: Some(devil_protocol::NetworkTarget {
                scheme: "https".to_string(),
                host: "cloud.legion.invalid".to_string(),
                port: Some(443),
            }),
            cloud_lane_estimated_cost_cents: Some(estimated_cost_cents),
            cloud_lane_upload_bytes: Some(upload_bytes),
            cloud_lane_scope_visible_to_user: scope_visible,
            cloud_lane_forbidden_upload_count: forbidden_upload_count,
            cloud_lane_task_packet_validated: task_packet_validated,
            cloud_lane_hard_cap_enforced: true,
            ..Default::default()
        }
    }

    #[test]
    fn trust_state_conversion_roundtrips() {
        let protocol = WorkspaceTrustState::Trusted;
        let security: TrustState = protocol.into();
        assert_eq!(security, TrustState::Trusted);
    }

    #[test]
    fn manual_product_mode_denies_ai_cloud_network_worker_telemetry_and_automation_surfaces() {
        for capability in [
            "ai.inline.predict",
            "ai.provider.openai",
            "network.fetch",
            "telemetry.export.hosted",
            "worker.spawn",
            "legion.workflow.run",
            "remote.workspace.connect",
        ] {
            let decision = product_mode_capability_decision(
                ProductMode::Manual,
                &CapabilityId(capability.to_string()),
            );
            assert!(
                matches!(decision, SecurityDecision::Deny(reason) if reason.contains("Manual mode denies")),
                "{capability} should be denied in Manual mode"
            );
        }

        assert_eq!(
            product_mode_capability_decision(
                ProductMode::Manual,
                &CapabilityId("fs.read".to_string())
            ),
            SecurityDecision::Allow
        );
        assert_eq!(
            product_mode_capability_decision(
                ProductMode::Manual,
                &CapabilityId("plugin.management.inspect".to_string())
            ),
            SecurityDecision::Allow
        );
    }

    #[test]
    fn path_policy_blocks_bad_roots() {
        let policy = PathPolicy {
            writable_roots: vec!["./workspace/".to_string()],
            readable_roots: vec!["./workspace/".to_string()],
            blocked_roots: vec!["./workspace/secret/".to_string()],
            max_write_bytes: 1024,
        };

        assert!(!policy.can_access("./workspace/secret/file.txt", PathAccess::Read));
        assert!(!policy.can_access("/outside/file", PathAccess::Read));
        assert!(policy.can_access("./workspace/public.rs", PathAccess::Write));
    }

    #[test]
    fn path_policy_rejects_sibling_prefix_escape() {
        let policy = PathPolicy {
            writable_roots: vec!["/repo/root".to_string()],
            readable_roots: vec!["/repo/root".to_string()],
            blocked_roots: vec![],
            max_write_bytes: 1024,
        };

        assert!(policy.can_access("/repo/root/src/main.rs", PathAccess::Read));
        assert!(!policy.can_access("/repo/root-evil/src/main.rs", PathAccess::Read));
    }

    #[test]
    fn path_policy_parent_escape_is_rejected() {
        let policy = PathPolicy {
            writable_roots: vec!["/repo/root".to_string()],
            readable_roots: vec!["/repo/root".to_string()],
            blocked_roots: vec![],
            max_write_bytes: 1024,
        };

        assert!(!policy.can_access("/repo/root/../../outside.txt", PathAccess::Write));
    }

    #[test]
    fn mcp_tool_permission_policy_uses_descriptor_fields_and_deny_override() {
        let server_id = McpServerId("mcp:test".to_string());
        let tool = McpToolDescriptor {
            server_id: server_id.clone(),
            name: McpToolName("write_file".to_string()),
            description_label: "write file".to_string(),
            input_schema_hash: FileFingerprint {
                algorithm: "sha256".to_string(),
                value: "schema".to_string(),
            },
            risk_label: ProposalRiskLabel::High,
            required_permission_profile: DelegatedTaskToolPermissionProfile::Write,
            action_class: PermissionBudgetActionClass::InvokeLocalTool,
            capability: CapabilityId("mcp.tool.call".to_string()),
            redaction_hints: vec![RedactionHint::MetadataOnly],
            schema_version: 1,
        };

        let allow = mcp_tool_permission_request(
            "permission:allow",
            &tool,
            DelegatedTaskToolPermissionDecision::Allow,
            "legion.session:session:test",
            1,
        );
        assert_eq!(allow.profile, DelegatedTaskToolPermissionProfile::Write);
        assert_eq!(
            allow.action_class,
            PermissionBudgetActionClass::InvokeLocalTool
        );
        assert_eq!(
            allow.capability,
            Some(CapabilityId("mcp.tool.call".to_string()))
        );
        assert_eq!(
            allow.target_id,
            Some("mcp-tool:mcp:test|write_file".to_string())
        );
        assert!(mcp_tool_permission_allows_runtime(&allow));

        let deny = mcp_tool_permission_request(
            "permission:deny",
            &tool,
            DelegatedTaskToolPermissionDecision::Deny,
            "legion.session:session:test",
            1,
        );
        assert!(!mcp_tool_permission_allows_runtime(&deny));
        assert!(deny.deny_overrides);
    }

    #[test]
    fn broker_rejects_unknown_trust_for_sensitive_requests() {
        let broker = DenyByDefaultBroker::default();
        let response = broker
            .handle(CapabilityRequest::Request {
                principal_id: PrincipalId("u".to_string()),
                capability_id: CapabilityId("fs.write".to_string()),
                workspace_trust_state: WorkspaceTrustState::Unknown,
                target_path: Some(devil_protocol::CanonicalPath(
                    "./workspace/file.txt".to_string(),
                )),
                decision_id: None,
                context: Default::default(),
                correlation_id: CorrelationId(10),
            })
            .expect("decision");

        match response {
            CapabilityResponse::Decision(decision) => {
                assert!(!decision.granted);
            }
            _ => panic!("expected decision response"),
        }
    }

    #[test]
    fn command_taxonomy_classifies_known_commands() {
        let taxonomy = CommandTaxonomy::new();
        assert_eq!(taxonomy.classify("rm"), CommandClass::Mutate);
        assert_eq!(taxonomy.classify("cargo"), CommandClass::LanguageServer);
        assert_eq!(taxonomy.classify("unknown-cmd"), CommandClass::Unknown);
    }

    #[test]
    fn terminal_decision_is_blocked_for_unknown_workspace() {
        let mut broker = DenyByDefaultBroker::default();
        let decision = broker.decide(
            TrustState::Untrusted,
            PrincipalId("principal-1".to_string()),
            CapabilityId("terminal.spawn".to_string()),
            Some("./workspace/a"),
        );
        assert!(matches!(decision, SecurityDecision::Deny(_)));
    }

    #[test]
    fn file_write_blocked_by_extension() {
        let mut broker = DenyByDefaultBroker::default();
        let decision = broker.decide(
            TrustState::Trusted,
            PrincipalId("principal-1".to_string()),
            CapabilityId("fs.write".to_string()),
            Some("./workspace/notes.txt"),
        );
        assert!(matches!(decision, SecurityDecision::Allow));
    }

    #[test]
    fn file_write_is_blocked_by_extension_policy() {
        let mut broker = DenyByDefaultBroker::default();
        let decision = broker.decide(
            TrustState::Trusted,
            PrincipalId("principal-1".to_string()),
            CapabilityId("fs.write".to_string()),
            Some("./workspace/secret.exe"),
        );
        assert!(matches!(decision, SecurityDecision::Deny(_)));
    }

    #[test]
    fn deny_by_default_for_unknown_capability() {
        let mut broker = DenyByDefaultBroker::default();
        let decision = broker.decide(
            TrustState::Trusted,
            PrincipalId("principal-1".to_string()),
            CapabilityId("custom.unknown".to_string()),
            Some("./workspace/a"),
        );
        assert!(matches!(decision, SecurityDecision::Deny(_)));
    }

    #[test]
    fn collaboration_capabilities_are_disabled_by_default_and_require_trust() {
        let mut broker = DenyByDefaultBroker::default();
        let operation = broker.decide(
            TrustState::Trusted,
            PrincipalId("principal-1".to_string()),
            CapabilityId("collaboration.operation.publish".to_string()),
            None,
        );
        assert!(matches!(operation, SecurityDecision::Deny(_)));

        let untrusted_presence = broker.decide(
            TrustState::Untrusted,
            PrincipalId("principal-1".to_string()),
            CapabilityId("collaboration.presence.publish".to_string()),
            None,
        );
        assert!(matches!(untrusted_presence, SecurityDecision::Deny(_)));
    }

    #[test]
    fn collaboration_policy_allows_presence_without_runtime_mutation() {
        let policy = SecurityPolicy {
            collaboration_policy: CollaborationCapabilityPolicy {
                presence_enabled: true,
                ..CollaborationCapabilityPolicy::default()
            },
            ..SecurityPolicy::default()
        };
        let mut broker = DenyByDefaultBroker::new(policy, CapabilityNamespace("test".to_string()));

        let presence = broker.decide(
            TrustState::Trusted,
            PrincipalId("principal-1".to_string()),
            CapabilityId("collaboration.presence.publish".to_string()),
            None,
        );
        let operation = broker.decide(
            TrustState::Trusted,
            PrincipalId("principal-1".to_string()),
            CapabilityId("collaboration.operation.publish".to_string()),
            None,
        );

        assert!(matches!(presence, SecurityDecision::Allow));
        assert!(matches!(operation, SecurityDecision::Deny(_)));
    }

    #[test]
    fn collaboration_transport_denies_non_loopback_air_gap_egress() {
        let policy = SecurityPolicy {
            collaboration_policy: CollaborationCapabilityPolicy {
                runtime_sessions_enabled: true,
                ..CollaborationCapabilityPolicy::default()
            },
            ..SecurityPolicy::default()
        };
        let mut broker = DenyByDefaultBroker::new(policy, CapabilityNamespace("test".to_string()));
        let decision = broker.decide_with_request_context(
            TrustState::Trusted,
            PrincipalId("principal-1".to_string()),
            CapabilityId("collaboration.session.join".to_string()),
            None,
            CapabilityRequestContext {
                network_target: Some(devil_protocol::NetworkTarget {
                    scheme: "https".to_string(),
                    host: "collab.example.com".to_string(),
                    port: Some(443),
                }),
                ..Default::default()
            },
        );

        assert!(matches!(decision, SecurityDecision::Deny(_)));
    }

    #[test]
    fn remote_capabilities_are_disabled_by_default_and_require_trust() {
        let mut broker = DenyByDefaultBroker::default();
        let fs_read = broker.decide(
            TrustState::Trusted,
            PrincipalId("principal-1".to_string()),
            CapabilityId("remote.fs.read".to_string()),
            None,
        );
        assert!(matches!(fs_read, SecurityDecision::Deny(_)));

        let untrusted = broker.decide(
            TrustState::Untrusted,
            PrincipalId("principal-1".to_string()),
            CapabilityId("remote.session.connect".to_string()),
            None,
        );
        assert!(matches!(untrusted, SecurityDecision::Deny(_)));
    }

    #[test]
    fn remote_policy_allows_filesystem_without_execution() {
        let policy = SecurityPolicy {
            remote_policy: RemoteDevelopmentPolicy {
                runtime_sessions_enabled: true,
                filesystem_enabled: true,
                ..RemoteDevelopmentPolicy::default()
            },
            ..SecurityPolicy::default()
        };
        let mut broker = DenyByDefaultBroker::new(policy, CapabilityNamespace("test".to_string()));

        let fs_write = broker.decide(
            TrustState::Trusted,
            PrincipalId("principal-1".to_string()),
            CapabilityId("remote.fs.write".to_string()),
            None,
        );
        let process = broker.decide(
            TrustState::Trusted,
            PrincipalId("principal-1".to_string()),
            CapabilityId("remote.process.launch".to_string()),
            None,
        );

        assert!(matches!(fs_write, SecurityDecision::Allow));
        assert!(matches!(process, SecurityDecision::Deny(_)));
    }

    #[test]
    fn phase8_remote_egress_requires_runtime_session() {
        let mut broker = DenyByDefaultBroker::default();
        let denied = broker.decide_with_request_context(
            TrustState::Trusted,
            PrincipalId("principal-1".to_string()),
            CapabilityId("remote.egress".to_string()),
            None,
            CapabilityRequestContext {
                network_target: Some(devil_protocol::NetworkTarget {
                    scheme: "https".to_string(),
                    host: "localhost".to_string(),
                    port: Some(9443),
                }),
                ..Default::default()
            },
        );
        assert!(
            matches!(denied, SecurityDecision::Deny(reason) if reason.contains("remote sessions"))
        );

        let policy = SecurityPolicy {
            remote_policy: RemoteDevelopmentPolicy {
                runtime_sessions_enabled: true,
                ..RemoteDevelopmentPolicy::default()
            },
            ..SecurityPolicy::default()
        };
        let mut broker = DenyByDefaultBroker::new(policy, CapabilityNamespace("test".to_string()));
        let allowed = broker.decide_with_request_context(
            TrustState::Trusted,
            PrincipalId("principal-1".to_string()),
            CapabilityId("remote.egress".to_string()),
            None,
            CapabilityRequestContext {
                network_target: Some(devil_protocol::NetworkTarget {
                    scheme: "https".to_string(),
                    host: "localhost".to_string(),
                    port: Some(9443),
                }),
                ..Default::default()
            },
        );
        assert!(matches!(allowed, SecurityDecision::Allow));
    }

    #[test]
    fn remote_connect_denies_non_loopback_air_gap_egress() {
        let policy = SecurityPolicy {
            remote_policy: RemoteDevelopmentPolicy {
                runtime_sessions_enabled: true,
                ..RemoteDevelopmentPolicy::default()
            },
            ..SecurityPolicy::default()
        };
        let mut broker = DenyByDefaultBroker::new(policy, CapabilityNamespace("test".to_string()));
        let decision = broker.decide_with_request_context(
            TrustState::Trusted,
            PrincipalId("principal-1".to_string()),
            CapabilityId("remote.session.connect".to_string()),
            None,
            CapabilityRequestContext {
                network_target: Some(devil_protocol::NetworkTarget {
                    scheme: "https".to_string(),
                    host: "remote.example.com".to_string(),
                    port: Some(443),
                }),
                ..Default::default()
            },
        );

        assert!(matches!(decision, SecurityDecision::Deny(_)));
    }

    #[test]
    fn cloud_lane_capabilities_are_disabled_by_default_and_require_trust() {
        let mut broker = DenyByDefaultBroker::default();
        let default_denied = broker.decide(
            TrustState::Trusted,
            PrincipalId("principal-1".to_string()),
            CapabilityId("cloud.lane.submit".to_string()),
            None,
        );
        assert!(
            matches!(default_denied, SecurityDecision::Deny(reason) if reason.contains("disabled"))
        );

        let policy = SecurityPolicy {
            cloud_lane_policy: CloudLaneSecurityPolicy {
                task_submission_enabled: true,
                ..CloudLaneSecurityPolicy::default()
            },
            network_policy: NetworkPolicy {
                air_gap: false,
                local_provider_only: false,
                allowlist: vec!["cloud.legion.invalid".to_string()],
                ..NetworkPolicy::default()
            },
            ..SecurityPolicy::default()
        };
        let mut broker = DenyByDefaultBroker::new(policy, CapabilityNamespace("test".to_string()));
        let untrusted = broker.decide_with_request_context(
            TrustState::Untrusted,
            PrincipalId("principal-1".to_string()),
            CapabilityId("cloud.lane.submit".to_string()),
            None,
            cloud_lane_context(50, 16_384, true, 0, true),
        );
        assert!(
            matches!(untrusted, SecurityDecision::Deny(reason) if reason.contains("untrusted"))
        );
    }

    #[test]
    fn cloud_lane_submit_requires_visible_scope_budget_cap_and_https_target() {
        let policy = SecurityPolicy {
            cloud_lane_policy: CloudLaneSecurityPolicy {
                task_submission_enabled: true,
                max_upload_bytes: 32_768,
                max_cost_cents: 75,
                ..CloudLaneSecurityPolicy::default()
            },
            network_policy: NetworkPolicy {
                air_gap: false,
                local_provider_only: false,
                allowlist: vec!["cloud.legion.invalid".to_string()],
                ..NetworkPolicy::default()
            },
            ..SecurityPolicy::default()
        };
        let mut broker = DenyByDefaultBroker::new(policy, CapabilityNamespace("test".to_string()));

        let missing_visibility = broker.decide_with_request_context(
            TrustState::Trusted,
            PrincipalId("principal-1".to_string()),
            CapabilityId("cloud.lane.submit".to_string()),
            None,
            cloud_lane_context(50, 16_384, false, 0, true),
        );
        assert!(
            matches!(missing_visibility, SecurityDecision::Deny(reason) if reason.contains("upload scope"))
        );

        let forbidden_scope = broker.decide_with_request_context(
            TrustState::Trusted,
            PrincipalId("principal-1".to_string()),
            CapabilityId("cloud.lane.submit".to_string()),
            None,
            cloud_lane_context(50, 16_384, true, 1, true),
        );
        assert!(
            matches!(forbidden_scope, SecurityDecision::Deny(reason) if reason.contains("forbidden"))
        );

        let budget_exceeded = broker.decide_with_request_context(
            TrustState::Trusted,
            PrincipalId("principal-1".to_string()),
            CapabilityId("cloud.lane.submit".to_string()),
            None,
            cloud_lane_context(76, 16_384, true, 0, true),
        );
        assert!(
            matches!(budget_exceeded, SecurityDecision::Deny(reason) if reason.contains("cost cap"))
        );

        let plaintext = broker.decide_with_request_context(
            TrustState::Trusted,
            PrincipalId("principal-1".to_string()),
            CapabilityId("cloud.lane.submit".to_string()),
            None,
            CapabilityRequestContext {
                network_target: Some(devil_protocol::NetworkTarget {
                    scheme: "http".to_string(),
                    host: "cloud.legion.invalid".to_string(),
                    port: Some(80),
                }),
                ..cloud_lane_context(50, 16_384, true, 0, true)
            },
        );
        assert!(matches!(plaintext, SecurityDecision::Deny(reason) if reason.contains("HTTPS")));

        let allowed = broker.decide_with_request_context(
            TrustState::Trusted,
            PrincipalId("principal-1".to_string()),
            CapabilityId("cloud.lane.submit".to_string()),
            None,
            cloud_lane_context(50, 16_384, true, 0, true),
        );
        assert!(matches!(allowed, SecurityDecision::Allow));
    }

    #[test]
    fn broker_request_is_denied_when_namespace_empty() {
        let mut broker = DenyByDefaultBroker {
            namespace: CapabilityNamespace(String::new()),
            ..DenyByDefaultBroker::default()
        };
        let decision = broker.decide(
            TrustState::Trusted,
            PrincipalId("principal-1".to_string()),
            CapabilityId("terminal.spawn".to_string()),
            Some("./workspace/a"),
        );
        assert!(matches!(decision, SecurityDecision::Deny(_)));
    }

    #[test]
    fn protocol_handle_request_returns_decision() {
        let broker = DenyByDefaultBroker::default();
        let response = broker
            .handle(CapabilityRequest::Request {
                principal_id: PrincipalId("u".to_string()),
                capability_id: CapabilityId("terminal.spawn".to_string()),
                workspace_trust_state: WorkspaceTrustState::Trusted,
                target_path: None,
                decision_id: None,
                context: Default::default(),
                correlation_id: CorrelationId(10),
            })
            .expect("decision");

        match response {
            CapabilityResponse::Decision(_)
            | CapabilityResponse::Granted(_)
            | CapabilityResponse::Denied(_) => {}
        }
    }

    #[test]
    fn network_fetch_requires_allowlisted_target_even_when_trusted() {
        let mut broker = DenyByDefaultBroker::default();
        let decision = broker.decide_with_request_context(
            TrustState::Trusted,
            PrincipalId("principal-1".to_string()),
            CapabilityId("network.fetch".to_string()),
            None,
            CapabilityRequestContext {
                network_target: Some(devil_protocol::NetworkTarget {
                    scheme: "https".to_string(),
                    host: "example.com".to_string(),
                    port: Some(443),
                }),
                ..Default::default()
            },
        );

        assert!(matches!(decision, SecurityDecision::Deny(_)));
    }

    #[test]
    fn ai_provider_invoke_allows_loopback_for_trusted_workspace() {
        let mut broker = DenyByDefaultBroker::default();
        let decision = broker.decide_with_request_context(
            TrustState::Trusted,
            PrincipalId("principal-1".to_string()),
            CapabilityId("ai.provider.invoke".to_string()),
            None,
            CapabilityRequestContext {
                network_target: Some(devil_protocol::NetworkTarget {
                    scheme: "http".to_string(),
                    host: "localhost".to_string(),
                    port: Some(11434),
                }),
                ..Default::default()
            },
        );

        assert!(matches!(decision, SecurityDecision::Allow));
    }

    #[test]
    fn air_gap_denies_hosted_provider_telemetry_embeddings_and_gateway() {
        let mut broker = DenyByDefaultBroker::default();
        let remote_target = CapabilityRequestContext {
            network_target: Some(devil_protocol::NetworkTarget {
                scheme: "https".to_string(),
                host: "api.openai.com".to_string(),
                port: Some(443),
            }),
            ..Default::default()
        };

        let hosted_provider = broker.decide_with_request_context(
            TrustState::Trusted,
            PrincipalId("principal-1".to_string()),
            CapabilityId("ai.provider.invoke".to_string()),
            None,
            remote_target,
        );
        assert!(matches!(hosted_provider, SecurityDecision::Deny(_)));

        for capability in [
            "ai.telemetry.hosted",
            "ai.embedding.hosted",
            "ai.gateway.invoke",
            "network.fetch",
        ] {
            let decision = broker.decide_with_request_context(
                TrustState::Trusted,
                PrincipalId("principal-1".to_string()),
                CapabilityId(capability.to_string()),
                None,
                CapabilityRequestContext {
                    network_target: Some(devil_protocol::NetworkTarget {
                        scheme: "https".to_string(),
                        host: "telemetry.example.com".to_string(),
                        port: Some(443),
                    }),
                    ..Default::default()
                },
            );
            assert!(matches!(decision, SecurityDecision::Deny(_)));
        }
    }

    #[test]
    fn phase8_hosted_telemetry_is_disabled_by_default_and_air_gap_denied() {
        let mut broker = DenyByDefaultBroker::default();
        let default_denied = broker.decide_with_request_context(
            TrustState::Trusted,
            PrincipalId("principal-1".to_string()),
            CapabilityId("telemetry.export.hosted".to_string()),
            None,
            CapabilityRequestContext {
                hosted_telemetry_consent_current: true,
                network_target: Some(devil_protocol::NetworkTarget {
                    scheme: "https".to_string(),
                    host: "telemetry.example.com".to_string(),
                    port: Some(443),
                }),
                ..Default::default()
            },
        );
        assert!(matches!(default_denied, SecurityDecision::Deny(_)));

        let policy = SecurityPolicy {
            telemetry_policy: HostedTelemetryPolicy {
                export_enabled: true,
                ..HostedTelemetryPolicy::default()
            },
            ..SecurityPolicy::default()
        };
        let mut broker = DenyByDefaultBroker::new(policy, CapabilityNamespace("test".to_string()));
        let air_gap_denied = broker.decide_with_request_context(
            TrustState::Trusted,
            PrincipalId("principal-1".to_string()),
            CapabilityId("telemetry.export.hosted".to_string()),
            None,
            CapabilityRequestContext {
                hosted_telemetry_consent_current: true,
                network_target: Some(devil_protocol::NetworkTarget {
                    scheme: "https".to_string(),
                    host: "telemetry.example.com".to_string(),
                    port: Some(443),
                }),
                ..Default::default()
            },
        );
        assert!(
            matches!(air_gap_denied, SecurityDecision::Deny(reason) if reason.contains("air-gap"))
        );
    }

    #[test]
    fn phase8_telemetry_spool_write_is_disabled_by_default() {
        let mut broker = DenyByDefaultBroker::default();
        let spool_write = broker.decide(
            TrustState::Trusted,
            PrincipalId("principal-1".to_string()),
            CapabilityId("telemetry.spool.write".to_string()),
            None,
        );
        assert!(
            matches!(spool_write, SecurityDecision::Deny(reason) if reason.contains("spool writes are disabled"))
        );

        let revoke_untrusted = broker.decide(
            TrustState::Untrusted,
            PrincipalId("principal-1".to_string()),
            CapabilityId("telemetry.consent.revoke".to_string()),
            None,
        );
        assert!(matches!(revoke_untrusted, SecurityDecision::Deny(_)));
    }

    #[test]
    fn phase8_hosted_telemetry_requires_current_consent_and_https() {
        let policy = SecurityPolicy {
            network_policy: NetworkPolicy {
                air_gap: false,
                local_provider_only: false,
                allowlist: vec!["telemetry.example.com".to_string()],
                ..NetworkPolicy::default()
            },
            telemetry_policy: HostedTelemetryPolicy {
                export_enabled: true,
                spool_write_enabled: true,
                require_explicit_consent: true,
                ..HostedTelemetryPolicy::default()
            },
            ..SecurityPolicy::default()
        };
        let mut broker = DenyByDefaultBroker::new(policy, CapabilityNamespace("test".to_string()));
        let missing_consent = broker.decide_with_request_context(
            TrustState::Trusted,
            PrincipalId("principal-1".to_string()),
            CapabilityId("telemetry.export.hosted".to_string()),
            None,
            CapabilityRequestContext {
                network_target: Some(devil_protocol::NetworkTarget {
                    scheme: "https".to_string(),
                    host: "telemetry.example.com".to_string(),
                    port: Some(443),
                }),
                ..Default::default()
            },
        );
        assert!(
            matches!(missing_consent, SecurityDecision::Deny(reason) if reason.contains("consent"))
        );

        let plaintext = broker.decide_with_request_context(
            TrustState::Trusted,
            PrincipalId("principal-1".to_string()),
            CapabilityId("telemetry.export.hosted".to_string()),
            None,
            CapabilityRequestContext {
                hosted_telemetry_consent_current: true,
                network_target: Some(devil_protocol::NetworkTarget {
                    scheme: "http".to_string(),
                    host: "telemetry.example.com".to_string(),
                    port: Some(80),
                }),
                ..Default::default()
            },
        );
        assert!(matches!(plaintext, SecurityDecision::Deny(reason) if reason.contains("HTTPS")));

        let spool = broker.decide_with_request_context(
            TrustState::Trusted,
            PrincipalId("principal-1".to_string()),
            CapabilityId("telemetry.spool.write".to_string()),
            None,
            CapabilityRequestContext {
                hosted_telemetry_consent_current: true,
                ..Default::default()
            },
        );
        assert!(matches!(spool, SecurityDecision::Allow));
    }

    #[test]
    fn phase8_raw_source_retention_is_disabled_by_default() {
        let mut broker = DenyByDefaultBroker::default();
        let capture = broker.decide(
            TrustState::Trusted,
            PrincipalId("principal-1".to_string()),
            CapabilityId("retention.raw_source.capture".to_string()),
            None,
        );
        assert!(matches!(capture, SecurityDecision::Deny(reason) if reason.contains("disabled")));

        let untrusted_delete = broker.decide(
            TrustState::Untrusted,
            PrincipalId("principal-1".to_string()),
            CapabilityId("retention.raw_source.delete".to_string()),
            None,
        );
        assert!(matches!(untrusted_delete, SecurityDecision::Deny(_)));
    }

    #[test]
    fn phase8_raw_source_read_export_and_delete_policy_paths_are_explicit() {
        let mut broker = DenyByDefaultBroker::default();
        for capability in [
            "retention.raw_source.read",
            "retention.raw_source.export",
            "retention.raw_source.export.hosted",
        ] {
            let decision = broker.decide(
                TrustState::Trusted,
                PrincipalId("principal-1".to_string()),
                CapabilityId(capability.to_string()),
                None,
            );
            assert!(
                matches!(&decision, SecurityDecision::Deny(reason) if reason.contains("disabled")),
                "{capability} should deny by default, got {decision:?}"
            );
        }

        let delete = broker.decide(
            TrustState::Trusted,
            PrincipalId("principal-1".to_string()),
            CapabilityId("retention.raw_source.delete".to_string()),
            None,
        );
        assert!(matches!(delete, SecurityDecision::Allow));
    }

    #[test]
    fn phase8_raw_source_retention_requires_current_consent() {
        let policy = SecurityPolicy {
            network_policy: NetworkPolicy {
                air_gap: false,
                local_provider_only: false,
                allowlist: vec!["support.example.com".to_string()],
                ..NetworkPolicy::default()
            },
            retention_policy: RawSourceRetentionSecurityPolicy {
                capture_enabled: true,
                require_explicit_consent: true,
                ..RawSourceRetentionSecurityPolicy::default()
            },
            ..SecurityPolicy::default()
        };
        let mut broker = DenyByDefaultBroker::new(policy, CapabilityNamespace("test".to_string()));
        let capture = broker.decide(
            TrustState::Trusted,
            PrincipalId("principal-1".to_string()),
            CapabilityId("retention.raw_source.capture".to_string()),
            None,
        );
        assert!(matches!(capture, SecurityDecision::Deny(reason) if reason.contains("consent")));

        let hosted = broker.decide_with_request_context(
            TrustState::Trusted,
            PrincipalId("principal-1".to_string()),
            CapabilityId("retention.raw_source.export.hosted".to_string()),
            None,
            CapabilityRequestContext {
                raw_source_retention_consent_current: true,
                raw_source_hosted_export_consent_current: true,
                network_target: Some(devil_protocol::NetworkTarget {
                    scheme: "https".to_string(),
                    host: "support.example.com".to_string(),
                    port: Some(443),
                }),
                ..Default::default()
            },
        );
        assert!(matches!(hosted, SecurityDecision::Allow));
    }

    #[test]
    fn phase8_terminal_runtime_capabilities_are_disabled_by_default() {
        let mut broker = DenyByDefaultBroker::default();

        for capability in [
            "terminal.launch",
            "terminal.input",
            "terminal.resize",
            "terminal.close",
            "terminal.kill",
        ] {
            let decision = broker.decide(
                TrustState::Trusted,
                PrincipalId("principal-1".to_string()),
                CapabilityId(capability.to_string()),
                None,
            );
            assert!(
                matches!(&decision, SecurityDecision::Deny(reason) if reason.contains("disabled")),
                "{capability} should deny by default, got {decision:?}"
            );
        }

        let unknown = broker.decide(
            TrustState::Trusted,
            PrincipalId("principal-1".to_string()),
            CapabilityId("terminal.spawn".to_string()),
            None,
        );
        assert!(
            matches!(unknown, SecurityDecision::Deny(reason) if reason.contains("deny-by-default"))
        );
    }

    #[test]
    fn phase8_terminal_launch_requires_command_metadata_and_denies_network_tools() {
        let policy = SecurityPolicy {
            terminal_policy: TerminalPolicy {
                runtime_enabled: true,
                ..TerminalPolicy::default()
            },
            ..SecurityPolicy::default()
        };
        let mut broker = DenyByDefaultBroker::new(policy, CapabilityNamespace("test".to_string()));
        let missing_command = broker.decide(
            TrustState::Trusted,
            PrincipalId("principal-1".to_string()),
            CapabilityId("terminal.launch".to_string()),
            None,
        );
        assert!(
            matches!(missing_command, SecurityDecision::Deny(reason) if reason.contains("command metadata"))
        );

        let network_tool = broker.decide_with_request_context(
            TrustState::Trusted,
            PrincipalId("principal-1".to_string()),
            CapabilityId("terminal.launch".to_string()),
            None,
            CapabilityRequestContext {
                command_binary: Some("curl".to_string()),
                ..Default::default()
            },
        );
        assert!(
            matches!(network_tool, SecurityDecision::Deny(reason) if reason.contains("network-capable"))
        );

        let shell = broker.decide_with_request_context(
            TrustState::Trusted,
            PrincipalId("principal-1".to_string()),
            CapabilityId("terminal.launch".to_string()),
            None,
            CapabilityRequestContext {
                command_binary: Some("bash".to_string()),
                ..Default::default()
            },
        );
        assert!(matches!(shell, SecurityDecision::Allow));
    }

    #[test]
    fn phase8_remote_transport_and_retention_hosted_export_fail_closed() {
        let mut broker = DenyByDefaultBroker::default();

        let remote_connect = broker.decide_with_request_context(
            TrustState::Trusted,
            PrincipalId("principal-1".to_string()),
            CapabilityId("remote.transport.connect".to_string()),
            None,
            CapabilityRequestContext {
                network_target: Some(devil_protocol::NetworkTarget {
                    scheme: "https".to_string(),
                    host: "remote.example.com".to_string(),
                    port: Some(443),
                }),
                ..Default::default()
            },
        );
        assert!(matches!(remote_connect, SecurityDecision::Deny(_)));

        let remote_connect_default = broker.decide(
            TrustState::Trusted,
            PrincipalId("principal-1".to_string()),
            CapabilityId("remote.transport.connect".to_string()),
            None,
        );
        assert!(
            matches!(remote_connect_default, SecurityDecision::Deny(reason) if reason.contains("disabled"))
        );

        let hosted_export = broker.decide_with_request_context(
            TrustState::Trusted,
            PrincipalId("principal-1".to_string()),
            CapabilityId("retention.raw_source.export.hosted".to_string()),
            None,
            CapabilityRequestContext {
                network_target: Some(devil_protocol::NetworkTarget {
                    scheme: "https".to_string(),
                    host: "support.example.com".to_string(),
                    port: Some(443),
                }),
                ..Default::default()
            },
        );
        assert!(
            matches!(hosted_export, SecurityDecision::Deny(reason) if reason.contains("disabled"))
        );

        let listen = broker.decide(
            TrustState::Trusted,
            PrincipalId("principal-1".to_string()),
            CapabilityId("remote.transport.listen".to_string()),
            None,
        );
        assert!(matches!(listen, SecurityDecision::Deny(reason) if reason.contains("not enabled")));
    }

    #[test]
    fn phase8_remote_agent_package_activation_is_disabled_by_default() {
        let mut broker = DenyByDefaultBroker::default();
        let default_denied = broker.decide(
            TrustState::Trusted,
            PrincipalId("principal-1".to_string()),
            CapabilityId("remote.agent.package.activate".to_string()),
            None,
        );
        assert!(
            matches!(default_denied, SecurityDecision::Deny(reason) if reason.contains("disabled"))
        );

        let policy = SecurityPolicy {
            remote_policy: RemoteDevelopmentPolicy {
                runtime_sessions_enabled: true,
                ..RemoteDevelopmentPolicy::default()
            },
            ..SecurityPolicy::default()
        };
        let mut broker = DenyByDefaultBroker::new(policy, CapabilityNamespace("test".to_string()));
        let untrusted = broker.decide(
            TrustState::Untrusted,
            PrincipalId("principal-1".to_string()),
            CapabilityId("remote.agent.package.activate".to_string()),
            None,
        );
        assert!(
            matches!(untrusted, SecurityDecision::Deny(reason) if reason.contains("untrusted"))
        );

        let trusted_without_activation = broker.decide(
            TrustState::Trusted,
            PrincipalId("principal-1".to_string()),
            CapabilityId("remote.agent.package.activate".to_string()),
            None,
        );
        assert!(
            matches!(trusted_without_activation, SecurityDecision::Deny(reason) if reason.contains("disabled"))
        );

        let policy = SecurityPolicy {
            remote_policy: RemoteDevelopmentPolicy {
                runtime_sessions_enabled: true,
                agent_package_activation_enabled: true,
                ..RemoteDevelopmentPolicy::default()
            },
            ..SecurityPolicy::default()
        };
        let mut broker = DenyByDefaultBroker::new(policy, CapabilityNamespace("test".to_string()));
        let allowed = broker.decide(
            TrustState::Trusted,
            PrincipalId("principal-1".to_string()),
            CapabilityId("remote.agent.package.activate".to_string()),
            None,
        );
        assert!(matches!(allowed, SecurityDecision::Allow));
    }

    #[test]
    fn phase8_storage_migration_apply_and_repair_are_disabled_by_default() {
        let mut broker = DenyByDefaultBroker::default();
        for capability in ["storage.migration.apply", "storage.migration.repair"] {
            let decision = broker.decide(
                TrustState::Trusted,
                PrincipalId("principal-1".to_string()),
                CapabilityId(capability.to_string()),
                None,
            );
            assert!(
                matches!(&decision, SecurityDecision::Deny(reason) if reason.contains("disabled")),
                "{capability} should deny by default, got {decision:?}"
            );
        }
    }

    #[test]
    fn phase8_storage_migration_repair_requires_explicit_flag() {
        let policy = SecurityPolicy {
            storage_migration_policy: StorageMigrationSecurityPolicy {
                repair_enabled: true,
                explicit_repair_required: true,
                ..StorageMigrationSecurityPolicy::default()
            },
            ..SecurityPolicy::default()
        };
        let mut broker = DenyByDefaultBroker::new(policy, CapabilityNamespace("test".to_string()));
        let repair = broker.decide(
            TrustState::Trusted,
            PrincipalId("principal-1".to_string()),
            CapabilityId("storage.migration.repair".to_string()),
            None,
        );
        assert!(
            matches!(repair, SecurityDecision::Deny(reason) if reason.contains("explicit repair flag"))
        );
        let allowed = broker.decide_with_request_context(
            TrustState::Trusted,
            PrincipalId("principal-1".to_string()),
            CapabilityId("storage.migration.repair".to_string()),
            None,
            CapabilityRequestContext {
                storage_explicit_repair: true,
                ..Default::default()
            },
        );
        assert!(matches!(allowed, SecurityDecision::Allow));
    }

    #[test]
    fn phase8_storage_migration_apply_requires_explicit_flag() {
        let policy = SecurityPolicy {
            storage_migration_policy: StorageMigrationSecurityPolicy {
                apply_enabled: true,
                ..StorageMigrationSecurityPolicy::default()
            },
            ..SecurityPolicy::default()
        };
        let mut broker = DenyByDefaultBroker::new(policy, CapabilityNamespace("test".to_string()));
        let apply = broker.decide(
            TrustState::Trusted,
            PrincipalId("principal-1".to_string()),
            CapabilityId("storage.migration.apply".to_string()),
            None,
        );
        assert!(
            matches!(apply, SecurityDecision::Deny(reason) if reason.contains("explicit apply flag"))
        );
        let allowed = broker.decide_with_request_context(
            TrustState::Trusted,
            PrincipalId("principal-1".to_string()),
            CapabilityId("storage.migration.apply".to_string()),
            None,
            CapabilityRequestContext {
                storage_explicit_apply: true,
                ..Default::default()
            },
        );
        assert!(matches!(allowed, SecurityDecision::Allow));
    }

    #[test]
    fn phase8_sensitive_capabilities_deny_empty_principal() {
        let mut broker = DenyByDefaultBroker::default();
        for capability in [
            "remote.transport.connect",
            "remote.agent.package.activate",
            "terminal.launch",
            "telemetry.export.hosted",
            "retention.raw_source.capture",
            "storage.migration.apply",
        ] {
            let decision = broker.decide(
                TrustState::Trusted,
                PrincipalId(String::new()),
                CapabilityId(capability.to_string()),
                None,
            );
            assert!(
                matches!(&decision, SecurityDecision::Deny(reason) if reason.contains("principal is required")),
                "{capability} should deny empty principal, got {decision:?}"
            );
        }
    }

    #[test]
    fn phase8_capability_families_reject_unknown_workspace_trust() {
        let broker = DenyByDefaultBroker::default();
        for capability in [
            "remote.transport.connect",
            "terminal.launch",
            "telemetry.export.hosted",
            "retention.raw_source.capture",
            "storage.migration.apply",
        ] {
            let response = broker
                .handle(CapabilityRequest::Request {
                    principal_id: PrincipalId("principal-1".to_string()),
                    capability_id: CapabilityId(capability.to_string()),
                    workspace_trust_state: WorkspaceTrustState::Unknown,
                    target_path: None,
                    decision_id: None,
                    context: Default::default(),
                    correlation_id: CorrelationId(10),
                })
                .expect("decision");
            match response {
                CapabilityResponse::Decision(decision) => {
                    assert!(!decision.granted, "{capability} should deny unknown trust");
                    assert!(
                        decision.reason.as_deref().is_some_and(
                            |reason| reason.contains("workspace trust state is unknown")
                        )
                    );
                }
                other => panic!("unexpected response for {capability}: {other:?}"),
            }
        }
    }

    #[test]
    fn provider_policy_denies_remote_even_when_network_allowlist_permits_host() {
        let policy = SecurityPolicy {
            network_policy: NetworkPolicy {
                allowlist: vec!["api.openai.com".to_string()],
                air_gap: false,
                local_provider_only: false,
                ..NetworkPolicy::default()
            },
            ai_provider_policy: AiProviderPolicy {
                allow_remote_provider: false,
                ..AiProviderPolicy::default()
            },
            ..SecurityPolicy::default()
        };
        let mut broker = DenyByDefaultBroker::new(policy, CapabilityNamespace("test".to_string()));

        let decision = broker.decide_with_request_context(
            TrustState::Trusted,
            PrincipalId("principal-1".to_string()),
            CapabilityId("ai.provider.invoke".to_string()),
            None,
            CapabilityRequestContext {
                network_target: Some(devil_protocol::NetworkTarget {
                    scheme: "https".to_string(),
                    host: "api.openai.com".to_string(),
                    port: Some(443),
                }),
                ..Default::default()
            },
        );

        assert!(
            matches!(decision, SecurityDecision::Deny(reason) if reason.contains("remote AI provider"))
        );
    }

    #[test]
    fn provider_policy_can_disable_local_loopback_invocation() {
        let policy = SecurityPolicy {
            ai_provider_policy: AiProviderPolicy {
                allow_local_provider: false,
                ..AiProviderPolicy::default()
            },
            ..SecurityPolicy::default()
        };
        let mut broker = DenyByDefaultBroker::new(policy, CapabilityNamespace("test".to_string()));

        let decision = broker.decide_with_request_context(
            TrustState::Trusted,
            PrincipalId("principal-1".to_string()),
            CapabilityId("ai.provider.invoke".to_string()),
            None,
            CapabilityRequestContext {
                network_target: Some(devil_protocol::NetworkTarget {
                    scheme: "http".to_string(),
                    host: "127.0.0.1".to_string(),
                    port: Some(11434),
                }),
                ..Default::default()
            },
        );

        assert!(
            matches!(decision, SecurityDecision::Deny(reason) if reason.contains("local AI provider"))
        );
    }

    #[test]
    fn memory_retain_denied_without_explicit_consent() {
        let mut broker = DenyByDefaultBroker::default();
        let decision = broker.decide(
            TrustState::Trusted,
            PrincipalId("principal-1".to_string()),
            CapabilityId("memory.retain".to_string()),
            None,
        );

        assert!(matches!(decision, SecurityDecision::Deny(_)));
    }

    #[test]
    fn plugin_manifest_context_is_required_and_unknown_capabilities_are_denied() {
        let mut broker = DenyByDefaultBroker::default();
        let missing_context = broker.decide_with_request_context(
            TrustState::Trusted,
            PrincipalId("plugin:7".to_string()),
            CapabilityId("plugin.command".to_string()),
            None,
            CapabilityRequestContext::default(),
        );
        assert!(
            matches!(missing_context, SecurityDecision::Deny(reason) if reason.contains("context required"))
        );

        let denied_unknown = broker.decide_with_request_context(
            TrustState::Trusted,
            PrincipalId("plugin:7".to_string()),
            CapabilityId("plugin.raw_source".to_string()),
            None,
            CapabilityRequestContext {
                plugin_namespace: Some(CapabilityNamespace("plugin.7".to_string())),
                plugin_id: Some(devil_protocol::PluginId(7)),
                plugin_host_call_name: Some("rawSource".to_string()),
                plugin_module_hash: Some("sha256:module".to_string()),
                plugin_manifest_id: Some("manifest:7".to_string()),
                plugin_declared_capability_id: Some(CapabilityId("plugin.raw_source".to_string())),
                plugin_quota_class: Some(devil_protocol::PluginQuotaClass::HostCall),
                plugin_sandbox_operation_class: Some(
                    devil_protocol::PluginSandboxOperationClass::HostCall,
                ),
                ..Default::default()
            },
        );
        assert!(
            matches!(denied_unknown, SecurityDecision::Deny(reason) if reason.contains("deny-by-default"))
        );
    }

    #[test]
    fn plugin_network_process_filesystem_and_untrusted_workspace_are_denied() {
        let mut broker = DenyByDefaultBroker::default();
        for capability in [
            "plugin.network",
            "plugin.fs",
            "plugin.process",
            "plugin.terminal",
        ] {
            let decision = broker.decide_with_request_context(
                TrustState::Trusted,
                PrincipalId("plugin:7".to_string()),
                CapabilityId(capability.to_string()),
                None,
                CapabilityRequestContext {
                    plugin_namespace: Some(CapabilityNamespace("plugin.7".to_string())),
                    plugin_id: Some(devil_protocol::PluginId(7)),
                    plugin_host_call_name: Some(capability.to_string()),
                    plugin_module_hash: Some("sha256:module".to_string()),
                    plugin_manifest_id: Some("manifest:7".to_string()),
                    plugin_declared_capability_id: Some(CapabilityId(capability.to_string())),
                    plugin_quota_class: Some(devil_protocol::PluginQuotaClass::HostCall),
                    plugin_sandbox_operation_class: Some(
                        devil_protocol::PluginSandboxOperationClass::HostCall,
                    ),
                    ..Default::default()
                },
            );
            assert!(matches!(decision, SecurityDecision::Deny(_)));
        }

        let untrusted = broker.decide_with_request_context(
            TrustState::Untrusted,
            PrincipalId("plugin:7".to_string()),
            CapabilityId("plugin.command".to_string()),
            None,
            CapabilityRequestContext {
                plugin_namespace: Some(CapabilityNamespace("plugin.7".to_string())),
                plugin_id: Some(devil_protocol::PluginId(7)),
                plugin_host_call_name: Some("command:phase5.run".to_string()),
                plugin_module_hash: Some("sha256:module".to_string()),
                plugin_manifest_id: Some("manifest:7".to_string()),
                plugin_declared_capability_id: Some(CapabilityId("plugin.command".to_string())),
                plugin_quota_class: Some(devil_protocol::PluginQuotaClass::HostCall),
                plugin_sandbox_operation_class: Some(
                    devil_protocol::PluginSandboxOperationClass::HostCall,
                ),
                ..Default::default()
            },
        );
        assert!(
            matches!(untrusted, SecurityDecision::Deny(reason) if reason.contains("untrusted"))
        );
    }
}
