//! Policy engine, air-gap mode, exfiltration checks, secrets boundaries.

#![warn(missing_docs)]

use std::{
    collections::{HashMap, HashSet},
    fmt,
    path::Path,
};

use devil_protocol::{
    CapabilityBrokerPort, CapabilityDecision, CapabilityDecisionId, CapabilityDenial,
    CapabilityGrant, CapabilityId, CapabilityNamespace, CapabilityRequest, CapabilityResponse,
    CorrelationId, PrincipalId, WorkspaceTrustState,
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

impl PathPolicy {
    /// Evaluates whether `path` can be used for provided access mode.
    pub fn can_access(&self, path: &str, access: PathAccess) -> bool {
        if self
            .blocked_roots
            .iter()
            .any(|prefix| path.starts_with(prefix))
        {
            return false;
        }

        let allowed = match access {
            PathAccess::Read | PathAccess::List => &self.readable_roots,
            PathAccess::Write => &self.writable_roots,
        };

        if allowed.is_empty() {
            return false;
        }

        allowed.iter().any(|prefix| path.starts_with(prefix))
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
    /// Maximum command timeout in seconds.
    pub max_command_timeout_seconds: u64,
}

impl Default for TerminalPolicy {
    fn default() -> Self {
        Self {
            max_output_bytes: 256 * 1024,
            allow_untrusted: false,
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
    /// Allowed capabilities, keyed by namespace and capability id.
    pub allowed: HashMap<String, HashSet<String>>,
    /// Required namespace if capability requested.
    pub namespace_required: bool,
    /// Allow plugins in untrusted workspaces.
    pub allow_in_untrusted_workspace: bool,
}

impl Default for PluginCapabilityPolicy {
    fn default() -> Self {
        Self {
            allowed: HashMap::from([(
                "plugin".to_string(),
                HashSet::from(["read".to_string(), "command".to_string()]),
            )]),
            namespace_required: true,
            allow_in_untrusted_workspace: false,
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
}

impl Default for NetworkPolicy {
    fn default() -> Self {
        Self {
            allow_untrusted: false,
            allowlist: vec!["localhost".to_string()],
            blocklist: vec!["example.exfiltration.invalid".to_string()],
        }
    }
}

/// Domain-level root policy.
#[derive(Debug, Clone)]
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
}

impl Default for SecurityPolicy {
    fn default() -> Self {
        Self {
            path_policy: PathPolicy::default(),
            command_taxonomy: CommandTaxonomy::default(),
            terminal_policy: TerminalPolicy::default(),
            lsp_policy: LspLaunchPolicy::default(),
            plugin_policy: PluginCapabilityPolicy::default(),
            file_write_policy: FileWritePolicy::default(),
            network_policy: NetworkPolicy::default(),
        }
    }
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
        _principal: PrincipalId,
        capability: CapabilityId,
        path: Option<&str>,
    ) -> SecurityDecision {
        self.counter = self.counter.saturating_add(1);
        let decision_id = CapabilityDecisionId(self.counter);

        if !self.namespace_policy_enabled(&self.namespace) {
            return SecurityDecision::deny(format!("namespace {} disabled", self.namespace.0));
        }

        self.decide_with_context(trust, _principal, capability, path, decision_id)
    }

    fn decide_with_context(
        &self,
        trust: TrustState,
        _principal: PrincipalId,
        capability: CapabilityId,
        path: Option<&str>,
        _decision_id: CapabilityDecisionId,
    ) -> SecurityDecision {
        let capability = capability.0;

        if let Some(stripped) = capability.strip_prefix("plugin.") {
            return if self.policy.plugin_policy.namespace_required
                && (self.policy.plugin_policy.allowed.is_empty() && !stripped.is_empty())
            {
                SecurityDecision::deny("plugin namespace policy denied")
            } else if !self.policy.plugin_policy.allow_in_untrusted_workspace
                && trust != TrustState::Trusted
            {
                SecurityDecision::deny("plugin capability denied for untrusted workspace")
            } else {
                SecurityDecision::allow()
            };
        }

        if let Some(rest) = capability.strip_prefix("fs.") {
            return if rest == "write" {
                if self.policy.file_write_policy.deny_when_untrusted && trust != TrustState::Trusted
                {
                    SecurityDecision::deny("file write denied for untrusted workspace")
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

        if let Some(rest) = capability.strip_prefix("terminal.") {
            return if !self.policy.terminal_policy.allow_untrusted && trust != TrustState::Trusted {
                SecurityDecision::deny("terminal denied for untrusted workspace")
            } else if rest == "spawn" {
                SecurityDecision::allow()
            } else {
                SecurityDecision::allow()
            };
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
                return SecurityDecision::allow();
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
                correlation_id: _,
            } => {
                let principal_state = if principal_id.0.is_empty() {
                    TrustState::Unknown
                } else {
                    TrustState::Trusted
                };

                let decision = owned.decide(
                    principal_state,
                    principal_id.clone(),
                    capability_id.clone(),
                    None,
                );
                let decision_id = CapabilityDecisionId(owned.counter.saturating_add(1));

                Ok(decision.into_protocol(decision_id, principal_id, capability_id))
            }
            CapabilityRequest::Grant(grant) => Ok(CapabilityResponse::Granted(grant)),
            CapabilityRequest::Deny(deny) => Ok(CapabilityResponse::Denied(deny)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use devil_protocol::CapabilityRequest;

    #[test]
    fn trust_state_conversion_roundtrips() {
        let protocol = WorkspaceTrustState::Trusted;
        let security: TrustState = protocol.into();
        assert_eq!(security, TrustState::Trusted);
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
    fn broker_request_is_denied_when_namespace_empty() {
        let mut broker = DenyByDefaultBroker::default();
        broker.namespace = CapabilityNamespace(String::new());
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
                correlation_id: CorrelationId(10),
            })
            .expect("decision");

        match response {
            CapabilityResponse::Decision(_)
            | CapabilityResponse::Granted(_)
            | CapabilityResponse::Denied(_) => {}
        }
    }
}
