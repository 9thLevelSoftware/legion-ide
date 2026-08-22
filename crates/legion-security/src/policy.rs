//! Deterministic approval-policy helpers for proposal auto-approval and apply gating.

use std::collections::HashMap;

use base64::Engine as _;
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Envelope policy controlling when a proposal may be auto-approved without a human in the loop.
///
/// The default is fail-closed: auto-approval is disabled and no rules are trusted
/// until explicitly configured.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ProposalAutoApprovalPolicy {
    /// Whether deterministic auto-approval is permitted at all.
    pub enabled: bool,
    /// Rule identifiers that are recognized as auto-approvable risk evidence.
    pub allowed_rule_ids: Vec<String>,
}

impl ProposalAutoApprovalPolicy {
    /// Returns true only when auto-approval is enabled and every supplied rule id is
    /// non-empty, recognized, and there is at least one rule backing the decision.
    ///
    /// An empty `rule_ids` slice can never be auto-approved: `.all(..)` on an empty
    /// iterator is vacuously true, so without this guard auto-approval would be granted
    /// with zero deterministic rule evidence.
    pub fn allows_rule_ids(&self, rule_ids: &[String]) -> bool {
        if !self.enabled || rule_ids.is_empty() {
            return false;
        }

        rule_ids.iter().all(|requested| {
            !requested.is_empty()
                && self
                    .allowed_rule_ids
                    .iter()
                    .any(|allowed| allowed == requested)
        })
    }
}

/// Policy controlling batched runtime application of approved proposals.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchRuntimeApplyPolicy {
    /// Whether batched runtime apply is permitted at all.
    pub enabled: bool,
    /// Maximum number of proposals that may be applied in a single batch.
    pub max_batch_size: usize,
}

impl Default for BatchRuntimeApplyPolicy {
    fn default() -> Self {
        // Fail closed: batching is disabled and limited to a single proposal until configured.
        Self {
            enabled: false,
            max_batch_size: 1,
        }
    }
}

impl BatchRuntimeApplyPolicy {
    /// Returns true when the given trust state is sufficient for batch runtime apply.
    ///
    /// Only `Trusted` workspaces pass this check. Untrusted, unknown, or missing
    /// trust states are rejected regardless of the `enabled` flag.
    pub fn allows_workspace_trust(
        &self,
        trust: Option<legion_protocol::WorkspaceTrustState>,
    ) -> bool {
        matches!(trust, Some(legion_protocol::WorkspaceTrustState::Trusted))
    }

    /// Returns true when runtime apply is disabled for the given trust state.
    ///
    /// Runtime apply is disabled when the policy is disabled OR the workspace
    /// is not trusted. Both conditions must be satisfied for apply to proceed.
    pub fn runtime_apply_disabled(
        &self,
        trust: Option<legion_protocol::WorkspaceTrustState>,
    ) -> bool {
        !self.enabled || !self.allows_workspace_trust(trust)
    }
}

/// Capability identifier a debug adapter launch must be granted under.
///
/// `legion-debug` refuses to resolve an adapter binary without a granted decision
/// carrying exactly this id, so the string is part of the contract between the
/// broker and the debug crate — not a log label.
pub const DEBUG_ADAPTER_LAUNCH_CAPABILITY: &str = "debug.adapter.launch";

/// Debug adapter launch policy controls (P2.F3.T2).
///
/// Two independent conditions must hold before an adapter process can exist:
/// the workspace must be trusted, and the *resolved binary* must be named in
/// [`Self::allowed_adapter_binaries`]. The second condition is what makes
/// `LEGION_DAP_ADAPTER` safe to honor: an operator-supplied path that resolves
/// to something other than a known debug adapter is refused.
///
/// There is deliberately no "trust every adapter" switch. Widening the set is
/// done by naming binaries, which keeps the allowed set auditable.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DebugAdapterLaunchPolicy {
    /// Trusted workspaces only by default.
    pub require_trusted_workspace: bool,
    /// Adapter binaries that may be launched, matched on the file stem without
    /// extension (`codelldb`, `codelldb.exe`, and `/opt/x/codelldb` all match
    /// `codelldb`). An empty list denies every adapter.
    #[serde(default = "default_allowed_adapter_binaries")]
    pub allowed_adapter_binaries: Vec<String>,
}

/// Adapter binaries allowed out of the box: the two Microsoft-DAP stdio adapters
/// that ship with LLDB, plus CodeLLDB.
fn default_allowed_adapter_binaries() -> Vec<String> {
    vec![
        "lldb-dap".to_string(),
        "lldb-vscode".to_string(),
        "codelldb".to_string(),
    ]
}

impl Default for DebugAdapterLaunchPolicy {
    fn default() -> Self {
        Self {
            require_trusted_workspace: true,
            allowed_adapter_binaries: default_allowed_adapter_binaries(),
        }
    }
}

impl DebugAdapterLaunchPolicy {
    /// Returns true when adapter discovery is allowed for the given workspace trust.
    pub fn allows_resolution(&self, trust: legion_protocol::WorkspaceTrustState) -> bool {
        !self.require_trusted_workspace || trust == legion_protocol::WorkspaceTrustState::Trusted
    }

    /// Returns true when `binary` is an allowlisted adapter.
    ///
    /// `binary` is compared case-insensitively against the configured stems; an
    /// empty allowlist denies everything rather than allowing everything, the
    /// same vacuous-truth guard used by [`ProposalAutoApprovalPolicy::allows_rule_ids`].
    pub fn allows_adapter_binary(&self, binary: &str) -> bool {
        let binary = binary.trim();
        if binary.is_empty() {
            return false;
        }
        self.allowed_adapter_binaries
            .iter()
            .any(|allowed| allowed.trim().eq_ignore_ascii_case(binary))
    }
}

/// Gate evaluated before a proposal may be applied to the workspace.
#[derive(Debug, Clone)]
pub struct ProposalApplyGate {
    /// Policy decision from the security broker.
    policy_decision: super::SecurityDecision,
    /// Require explicit human approval before apply.
    pub require_human_approval: bool,
    /// Require a trusted workspace before apply.
    pub require_trusted_workspace: bool,
    /// Whether explicit human approval has been recorded.
    human_approval_recorded: bool,
    /// Advisory classifier output. This is never authoritative for apply.
    classifier_recommendation: Option<legion_protocol::ProposalRiskLabel>,
}

impl ProposalApplyGate {
    /// Creates a proposal apply gate from the authoritative policy decision.
    pub fn new(policy_decision: super::SecurityDecision) -> Self {
        Self {
            policy_decision,
            require_human_approval: true,
            require_trusted_workspace: true,
            human_approval_recorded: false,
            classifier_recommendation: None,
        }
    }

    /// Records whether human approval has been provided.
    pub fn with_human_approval_recorded(mut self, recorded: bool) -> Self {
        self.human_approval_recorded = recorded;
        self
    }

    /// Adds an advisory classifier recommendation.
    pub fn with_classifier_recommendation(
        mut self,
        recommendation: Option<legion_protocol::ProposalRiskLabel>,
    ) -> Self {
        self.classifier_recommendation = recommendation;
        self
    }

    /// Returns the advisory classifier recommendation, if any.
    pub fn classifier_recommendation(&self) -> Option<legion_protocol::ProposalRiskLabel> {
        self.classifier_recommendation
    }

    /// Returns the authoritative policy decision.
    pub fn policy_decision(&self) -> &super::SecurityDecision {
        &self.policy_decision
    }

    /// Returns true only when policy allows and the human gate is satisfied.
    pub fn can_apply(&self) -> bool {
        matches!(self.policy_decision, super::SecurityDecision::Allow)
            && (!self.require_human_approval || self.human_approval_recorded)
    }
}

impl Default for ProposalApplyGate {
    fn default() -> Self {
        // Fail closed: policy denies by default, human approval and trust are required.
        Self::new(super::SecurityDecision::Deny(
            "proposal apply gate default deny".to_string(),
        ))
    }
}

// ---------------------------------------------------------------------------
// Graduated approval ladder
// ---------------------------------------------------------------------------

/// Derives a `ApprovalLevel` from a deterministic risk assessment and policy.
///
/// The graduated ladder maps the assessment outcome to one of four levels:
///
/// * **`Auto`** — all deterministic rules allow and the policy permits auto-approval
///   for the exact set of rule IDs cited in the assessment.
/// * **`Ask`** — all rules allow but the policy does not grant auto-approval.
/// * **`RequireExplicit`** — one or more non-critical rules deny the change.
/// * **`Deny`** — a critical path-scope violation is detected (workspace escape).
///
/// Empty findings can never produce `Auto` because `allows_rule_ids` rejects an
/// empty slice (vacuous-truth guard in [`ProposalAutoApprovalPolicy::allows_rule_ids`]).
pub fn derive_approval_level(
    assessment: &legion_protocol::risk::RiskAssessment,
    policy: &ProposalAutoApprovalPolicy,
) -> legion_protocol::risk::ApprovalLevel {
    use legion_protocol::risk::{ApprovalLevel, RiskRuleId};

    // Critical violation: workspace-scope escape is unconditionally denied.
    if let Some(finding) = assessment.finding(RiskRuleId::PathScope)
        && finding.outcome.is_deny()
    {
        return ApprovalLevel::Deny;
    }

    // Any non-critical rule deny → pause and require explicit approval.
    if !assessment.is_allow() {
        return ApprovalLevel::RequireExplicit;
    }

    // All rules allow — check whether the policy grants auto-approval.
    let rule_ids: Vec<String> = assessment
        .findings
        .iter()
        .map(|f| f.rule_id.stable_id().to_string())
        .collect();

    if policy.allows_rule_ids(&rule_ids) {
        ApprovalLevel::Auto
    } else {
        ApprovalLevel::Ask
    }
}

/// A git operation that contacts a remote.
///
/// These are the three verbs the product surface exposes. They are separated
/// from the rest of the git verbs because they are the only ones that can leave
/// the machine, so they are the only ones that need a network policy decision.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GitRemoteOperation {
    /// `git fetch <remote>` — reads refs and objects, does not touch the working tree.
    Fetch,
    /// `git pull <remote> <branch>` — fetch plus a working-tree merge.
    Pull,
    /// `git push <remote> <branch>` — publishes local commits to the remote.
    Push,
}

impl GitRemoteOperation {
    /// The command line handed to [`CommandTaxonomy::classify`].
    ///
    /// Classification is driven by the same taxonomy the terminal and `cmd.*`
    /// capability paths use, so a policy author who reclassifies `git push`
    /// changes both surfaces at once instead of only one of them.
    ///
    /// [`CommandTaxonomy::classify`]: super::CommandTaxonomy::classify
    pub fn command_line(&self) -> &'static str {
        match self {
            Self::Fetch => "git fetch",
            Self::Pull => "git pull",
            Self::Push => "git push",
        }
    }

    /// Short label used in user-visible audit rows.
    pub fn label(&self) -> &'static str {
        match self {
            Self::Fetch => "fetch",
            Self::Pull => "pull",
            Self::Push => "push",
        }
    }

    /// Whether the operation publishes local state to the remote.
    ///
    /// Only `push` writes outward; `fetch` and `pull` are inbound. Audit
    /// consumers use this to distinguish egress of repository contents from
    /// ingress of remote refs.
    pub fn publishes_local_content(&self) -> bool {
        matches!(self, Self::Push)
    }
}

/// Where a configured git remote points, as far as network policy is concerned.
///
/// Git remotes are not URLs in the usual sense: a remote may be a plain
/// filesystem path (`/srv/mirror.git`, `C:\repos\mirror.git`), a `file://` URL,
/// an scp-style SSH address (`git@github.com:org/repo.git`), or a conventional
/// URL. Only the last two can leave the machine, so they are the only ones that
/// are subject to allowlist/air-gap checks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GitRemoteTarget {
    /// A filesystem path or `file://` URL — no egress is possible.
    Local,
    /// A network endpoint, reduced to the host that policy matches against.
    Host {
        /// Lowercased hostname with any userinfo, port, and path removed.
        host: String,
        /// URL scheme (`ssh` for scp-style addresses).
        scheme: String,
    },
}

impl GitRemoteTarget {
    /// Label used in audit rows so the user can see what was matched.
    pub fn label(&self) -> String {
        match self {
            Self::Local => "local-path".to_string(),
            Self::Host { host, scheme } => format!("{scheme}://{host}"),
        }
    }
}

/// Classify a configured git remote URL into a policy target.
///
/// Returns [`GitRemoteTarget::Local`] for anything that cannot reach the
/// network. Unparseable values fall back to `Local` only when they look like a
/// path; a value that carries a scheme or an scp-style `host:path` separator is
/// always treated as a host so that a malformed remote cannot silently downgrade
/// itself to the unchecked local case.
pub fn classify_git_remote_url(remote_url: &str) -> GitRemoteTarget {
    let trimmed = remote_url.trim();
    if trimmed.is_empty() {
        return GitRemoteTarget::Local;
    }

    // Explicit `file://` is local by definition.
    if let Some(rest) = strip_scheme_prefix(trimmed, "file") {
        let _ = rest;
        return GitRemoteTarget::Local;
    }

    // Conventional `scheme://[user@]host[:port]/path` remotes.
    if let Some((scheme, rest)) = split_scheme(trimmed) {
        let authority = rest.split(['/', '?', '#']).next().unwrap_or(rest);
        return match host_from_authority(authority) {
            Some(host) => GitRemoteTarget::Host { host, scheme },
            // A scheme with no authority (e.g. `https:///x`) is malformed; fail
            // toward the checked branch rather than the unchecked one.
            None => GitRemoteTarget::Host {
                host: String::new(),
                scheme,
            },
        };
    }

    // Windows drive paths (`C:\repos\mirror.git`, `C:/repos/mirror.git`) look
    // like scp-style addresses if the colon is inspected naively.
    if is_windows_drive_path(trimmed) {
        return GitRemoteTarget::Local;
    }

    // scp-style `[user@]host:path`.
    if let Some((authority, _path)) = trimmed.split_once(':')
        && let Some(host) = host_from_authority(authority)
    {
        return GitRemoteTarget::Host {
            host,
            scheme: "ssh".to_string(),
        };
    }

    GitRemoteTarget::Local
}

/// Split `scheme://rest`, returning the lowercased scheme and the remainder.
fn split_scheme(value: &str) -> Option<(String, &str)> {
    let (scheme, rest) = value.split_once("://")?;
    if scheme.is_empty()
        || !scheme
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '+')
    {
        return None;
    }
    Some((scheme.to_ascii_lowercase(), rest))
}

/// Return the remainder when `value` carries exactly the supplied scheme.
fn strip_scheme_prefix<'a>(value: &'a str, scheme: &str) -> Option<&'a str> {
    let (found, rest) = split_scheme(value)?;
    (found == scheme).then_some(rest)
}

/// Reduce `[user@]host[:port]` to its lowercased host.
fn host_from_authority(authority: &str) -> Option<String> {
    let without_user = authority.rsplit_once('@').map_or(authority, |(_, h)| h);
    // IPv6 literals are bracketed; the inner colons are not port separators.
    let host = if let Some(inner) = without_user
        .strip_prefix('[')
        .and_then(|rest| rest.split_once(']'))
        .map(|(inner, _)| inner)
    {
        inner
    } else {
        without_user.split(':').next().unwrap_or(without_user)
    };
    (!host.is_empty()).then(|| host.to_ascii_lowercase())
}

/// Whether the value is a Windows drive-qualified path rather than `host:path`.
fn is_windows_drive_path(value: &str) -> bool {
    let mut chars = value.chars();
    matches!(
        (chars.next(), chars.next(), chars.next()),
        (Some(drive), Some(':'), Some('\\' | '/')) if drive.is_ascii_alphabetic()
    )
}

/// Whether a host never leaves the machine.
///
/// One definition, because there were three and they disagreed. The broker
/// accepted exactly `localhost`, `127.0.0.1` and `::1`; the product AI route
/// descriptor accepted any loopback `IpAddr` and bracketed IPv6. So an
/// `OLLAMA_BASE_URL` of `http://127.0.0.2:11434` was recognised as local by the
/// descriptor, added to the allowlist, selected by the reachability probe -- and
/// then denied by the broker as remote. Every Assist and Delegate request
/// failed, for a server that was running on a loopback address.
///
/// The wider reading is the correct one: `127.0.0.0/8` is loopback in its
/// entirety, and a bracketed `[::1]` is the same host as `::1` written the way a
/// URL requires. This is not permissiveness -- none of these addresses can reach
/// another machine, which is the property air-gap mode actually cares about.
pub fn is_loopback_host(host: &str) -> bool {
    let host = host.trim();
    let host = host
        .strip_prefix('[')
        .and_then(|h| h.strip_suffix(']'))
        .unwrap_or(host);
    if host.eq_ignore_ascii_case("localhost") {
        return true;
    }
    host.parse::<std::net::IpAddr>()
        .is_ok_and(|address| address.is_loopback())
}

/// The policy decision for one git remote operation, together with the audit
/// row that must be shown to the user.
///
/// Every evaluation produces one of these whether it allowed or denied, so a
/// network operation can never happen without a corresponding audit row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitRemoteDecision {
    /// Operation that was evaluated.
    pub operation: GitRemoteOperation,
    /// Configured remote name (`origin`).
    pub remote_name: String,
    /// Classified target of the configured remote URL.
    pub target: GitRemoteTarget,
    /// Command class the taxonomy assigned to the operation.
    pub command_class: super::CommandClass,
    /// Allow or deny, with the denial reason when denied.
    pub decision: super::SecurityDecision,
}

impl GitRemoteDecision {
    /// Whether the operation may proceed.
    pub fn is_allowed(&self) -> bool {
        matches!(self.decision, super::SecurityDecision::Allow)
    }

    /// A single display-safe audit row describing this decision.
    ///
    /// The row carries only metadata — operation, remote name, classified
    /// target, command class, and the policy verdict. It never includes
    /// credentials, full remote URLs with userinfo, or command output.
    pub fn audit_row(&self) -> String {
        let verdict = match &self.decision {
            super::SecurityDecision::Allow => "allow".to_string(),
            super::SecurityDecision::Deny(reason) => format!("deny ({reason})"),
        };
        format!(
            "git {} remote={} target={} class={:?} decision={}",
            self.operation.label(),
            self.remote_name,
            self.target.label(),
            self.command_class,
            verdict
        )
    }
}

/// Decide whether a git remote operation may run, and produce its audit row.
///
/// The evaluation is deliberately fail-closed and layered so that each denial
/// names exactly one cause:
///
/// 1. The operation is classified through [`CommandTaxonomy`], the same
///    taxonomy the `cmd.*` capability path uses. A class other than `Network`
///    means an operator reclassified the verb, and the operation is denied
///    rather than silently taking the untrusted-command path.
/// 2. Network-class commands require a trusted workspace, matching the `cmd.*`
///    rule that denies `Network`/`Mutate` classes outside `TrustState::Trusted`.
/// 3. A missing remote URL is denied — policy cannot evaluate a target it
///    cannot see, mirroring the "network target metadata required" rule.
/// 4. Filesystem remotes short-circuit to allow: they cannot egress, so
///    allowlist and air-gap checks do not apply to them.
/// 5. Host remotes are checked against the blocklist, then explicit user
///    consent, then air-gap, then the operator allowlist.
///
/// Consent sits above air-gap on purpose. Air-gap's job is to stop egress the
/// user did not ask for; a host the user named and granted is the opposite of
/// that, and it stays recorded and revocable. The blocklist is checked first so
/// an operator block still outranks any user grant.
///
/// [`CommandTaxonomy`]: super::CommandTaxonomy
pub fn decide_git_remote_operation(
    policy: &super::SecurityPolicy,
    trust: super::TrustState,
    operation: GitRemoteOperation,
    remote_name: &str,
    remote_url: Option<&str>,
) -> GitRemoteDecision {
    use super::{CommandClass, SecurityDecision, TrustState};

    let command_class = policy.command_taxonomy.classify(operation.command_line());
    let target = remote_url.map_or(GitRemoteTarget::Local, classify_git_remote_url);

    let decide = |decision: SecurityDecision, target: GitRemoteTarget| GitRemoteDecision {
        operation,
        remote_name: remote_name.to_string(),
        target,
        command_class,
        decision,
    };

    if command_class != CommandClass::Network {
        return decide(
            SecurityDecision::Deny(format!(
                "`{}` is classified {command_class:?}, not Network; refusing to run it \
                 outside the network policy path",
                operation.command_line()
            )),
            target,
        );
    }

    if trust != TrustState::Trusted {
        return decide(
            SecurityDecision::Deny(
                "network-class git operations require a trusted workspace".to_string(),
            ),
            target,
        );
    }

    let Some(remote_url) = remote_url else {
        return decide(
            SecurityDecision::Deny(format!(
                "remote `{remote_name}` has no configured URL; network target metadata \
                 is required by policy"
            )),
            GitRemoteTarget::Local,
        );
    };

    let target = classify_git_remote_url(remote_url);
    let GitRemoteTarget::Host { ref host, .. } = target else {
        // Filesystem remotes never reach the network, so the network policy's
        // allowlist and air-gap rules do not apply. The audit row is still
        // emitted so the user sees that the operation was evaluated.
        return decide(SecurityDecision::Allow, target);
    };
    let host = host.clone();

    if host.is_empty() {
        return decide(
            SecurityDecision::Deny(format!(
                "remote `{remote_name}` has no resolvable host; policy cannot evaluate it"
            )),
            target,
        );
    }

    if policy
        .network_policy
        .blocklist
        .iter()
        .any(|blocked| blocked.eq_ignore_ascii_case(&host))
    {
        return decide(
            SecurityDecision::Deny(format!("host `{host}` is blocked by network policy")),
            target,
        );
    }

    // Explicit user consent for this host, checked before air-gap and recorded
    // as its own verdict. Air-gap exists to stop egress nobody asked for; this
    // egress was asked for by name, granted deliberately, audited, and can be
    // revoked. The blocklist above still outranks consent, so an operator can
    // hard-block a host that no user grant can reopen.
    if policy
        .network_policy
        .consented_git_remote_hosts
        .iter()
        .any(|consented| consented.eq_ignore_ascii_case(&host))
    {
        return decide(SecurityDecision::Allow, target);
    }

    if policy.network_policy.air_gap && !is_loopback_host(&host) {
        return decide(
            SecurityDecision::Deny(format!(
                "air-gap mode denies non-loopback git {} to `{host}`",
                operation.label()
            )),
            target,
        );
    }

    if policy
        .network_policy
        .allowlist
        .iter()
        .any(|allowed| allowed.eq_ignore_ascii_case(&host))
    {
        decide(SecurityDecision::Allow, target)
    } else {
        decide(
            SecurityDecision::Deny(format!(
                "host `{host}` is not allowlisted by network policy"
            )),
            target,
        )
    }
}

/// Produces a metadata map recording the computed `ApprovalLevel` for audit rows.
///
/// Insert the returned map into any proposal audit record so every apply/deny
/// decision carries which approval level was computed.
pub fn approval_level_audit_metadata(
    level: legion_protocol::risk::ApprovalLevel,
) -> HashMap<String, String> {
    use legion_protocol::risk::ApprovalLevel;

    let level_str = match level {
        ApprovalLevel::Auto => "Auto",
        ApprovalLevel::Ask => "Ask",
        ApprovalLevel::RequireExplicit => "RequireExplicit",
        ApprovalLevel::Deny => "Deny",
    };
    let mut map = HashMap::new();
    map.insert("approval_level".to_string(), level_str.to_string());
    map
}

// ---------------------------------------------------------------------------
// Signed org policy bundles (P9.F2.T3)
// ---------------------------------------------------------------------------

/// Signature algorithm accepted for policy bundles.
///
/// This is the ADR-0042 release-manifest algorithm, deliberately reused so the
/// product has exactly one signing scheme. Any other value is rejected rather
/// than treated as "no signature required".
pub const POLICY_BUNDLE_SIGNATURE_ALGORITHM: &str = "ed25519";

/// Bundle schema version this build understands.
pub const POLICY_BUNDLE_SCHEMA_VERSION: u16 = 1;

/// Why an Ed25519 verification attempt failed.
///
/// Split into two variants so callers can distinguish a malformed trust anchor
/// (an operator configuration error) from a payload that does not match its
/// signature (tampering). Both are failures — neither is ever a pass.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Ed25519VerifyFailure {
    /// The verifying key was not a valid 32-byte Ed25519 public key.
    InvalidKey(String),
    /// The signature was malformed, or valid-shaped but wrong for this payload.
    VerifyFailed(String),
}

impl std::fmt::Display for Ed25519VerifyFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidKey(msg) => write!(f, "invalid key: {msg}"),
            Self::VerifyFailed(msg) => write!(f, "verify failed: {msg}"),
        }
    }
}

impl std::error::Error for Ed25519VerifyFailure {}

/// Verify a detached Ed25519 signature over `data`.
///
/// This is the single Ed25519 verification primitive in the workspace: the
/// release-manifest path (`xtask::signing::verify_ed25519_signature`) delegates
/// here, so a policy bundle and a release manifest are checked by the same code
/// with the same `verify_strict` semantics.
///
/// `verify_strict` (rather than `verify`) is deliberate: it rejects signatures
/// made under small-order public keys, which would otherwise verify against
/// more than one message.
pub fn verify_ed25519_signature(
    data: &[u8],
    signature: &[u8],
    verifying_key: &[u8],
) -> Result<(), Ed25519VerifyFailure> {
    let key_bytes: &[u8; 32] = verifying_key.try_into().map_err(|_| {
        Ed25519VerifyFailure::InvalidKey(format!(
            "verifying key must be 32 bytes, got {}",
            verifying_key.len()
        ))
    })?;
    let vk = ed25519_dalek::VerifyingKey::from_bytes(key_bytes)
        .map_err(|err| Ed25519VerifyFailure::InvalidKey(err.to_string()))?;

    let sig_bytes: &[u8; 64] = signature.try_into().map_err(|_| {
        Ed25519VerifyFailure::VerifyFailed(format!(
            "signature must be 64 bytes, got {}",
            signature.len()
        ))
    })?;
    let sig = ed25519_dalek::Signature::from_bytes(sig_bytes);

    vk.verify_strict(data, &sig)
        .map_err(|err| Ed25519VerifyFailure::VerifyFailed(err.to_string()))
}

/// One trust anchor an org policy bundle may be signed under.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolicySigningKey {
    /// Stable identifier the bundle names in its `key_id` field.
    pub key_id: String,
    /// Base64 (standard alphabet) encoding of the 32-byte Ed25519 public key.
    ///
    /// Public key material only. A private seed must never be written here; the
    /// bundle format has no field that would carry one.
    pub verifying_key_b64: String,
}

/// The set of keys whose signatures this installation will honour.
///
/// An empty keyring trusts nothing. That is the fail-closed default and is what
/// makes "no keys configured" mean "no bundle is honoured" rather than "every
/// bundle is honoured".
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PolicyKeyring {
    /// Trusted policy-signing keys.
    pub keys: Vec<PolicySigningKey>,
}

impl PolicyKeyring {
    /// Build a keyring from a list of trust anchors.
    pub fn new(keys: Vec<PolicySigningKey>) -> Self {
        Self { keys }
    }

    /// A keyring that trusts nothing.
    pub fn empty() -> Self {
        Self { keys: Vec::new() }
    }

    /// Whether the keyring holds no trust anchors.
    pub fn is_empty(&self) -> bool {
        self.keys.is_empty()
    }

    /// Look up a trust anchor by its exact (case-sensitive) key id.
    fn find(&self, key_id: &str) -> Option<&PolicySigningKey> {
        self.keys.iter().find(|key| key.key_id == key_id)
    }
}

/// Why a signed policy bundle was refused.
///
/// Every variant is a refusal. There is no "warn and continue" outcome: an org
/// policy bundle that cannot be proven authentic is not applied at all.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum PolicyBundleError {
    /// The bundle declared an algorithm other than [`POLICY_BUNDLE_SIGNATURE_ALGORITHM`].
    #[error("unsupported policy bundle signature algorithm `{0}`")]
    UnsupportedAlgorithm(String),
    /// No trust anchors are configured, so nothing can be honoured.
    #[error("policy signing keyring is empty; no bundle can be honoured")]
    EmptyKeyring,
    /// The bundle named a key id that is not a configured trust anchor.
    #[error("policy bundle signed by unknown key id `{0}`")]
    UnknownKeyId(String),
    /// The configured trust anchor could not be decoded.
    #[error("policy signing key `{key_id}` is malformed: {reason}")]
    MalformedKey {
        /// Key id whose material failed to decode.
        key_id: String,
        /// Decoder or key-validation message.
        reason: String,
    },
    /// The detached signature could not be decoded.
    #[error("policy bundle signature is malformed: {0}")]
    MalformedSignature(String),
    /// The signature did not match the payload under the named key.
    #[error("policy bundle signature does not match payload: {0}")]
    SignatureMismatch(String),
    /// The signed payload was not a parseable bundle.
    #[error("policy bundle payload is not a valid bundle: {0}")]
    MalformedPayload(String),
    /// The payload parsed but declared a schema this build cannot enforce.
    #[error(
        "policy bundle schema version {found} is not supported (this build enforces {supported})"
    )]
    UnsupportedSchemaVersion {
        /// Version the bundle declared.
        found: u16,
        /// Version this build enforces.
        supported: u16,
    },
}

/// A policy bundle as distributed: an opaque payload plus its detached signature.
///
/// The payload is carried as the *exact* TOML text that was signed. Re-serializing
/// a parsed bundle to check the signature would let a formatting difference break
/// verification, or worse, let a semantically different re-serialization verify.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignedPolicyBundle {
    /// Signature algorithm; must be [`POLICY_BUNDLE_SIGNATURE_ALGORITHM`].
    pub algorithm: String,
    /// Trust anchor id the signature was produced under.
    pub key_id: String,
    /// Base64 (standard alphabet) encoding of the 64-byte detached signature.
    pub signature_b64: String,
    /// The exact bundle TOML text the signature covers.
    pub payload_toml: String,
}

impl SignedPolicyBundle {
    /// Verify the bundle against a keyring and return the enforceable form.
    ///
    /// Fail-closed at every step. In particular an empty keyring, an unknown key
    /// id, a non-Ed25519 algorithm, an undecodable signature, and a payload that
    /// does not parse are all refusals — none of them yields a bundle.
    ///
    /// [`VerifiedPolicyBundle`] has no other constructor, so an unverified bundle
    /// cannot be handed to the enforcement path by mistake: it is not the right
    /// type.
    pub fn verify(
        &self,
        keyring: &PolicyKeyring,
    ) -> Result<VerifiedPolicyBundle, PolicyBundleError> {
        if self.algorithm != POLICY_BUNDLE_SIGNATURE_ALGORITHM {
            return Err(PolicyBundleError::UnsupportedAlgorithm(
                self.algorithm.clone(),
            ));
        }
        if keyring.is_empty() {
            return Err(PolicyBundleError::EmptyKeyring);
        }
        let anchor = keyring
            .find(&self.key_id)
            .ok_or_else(|| PolicyBundleError::UnknownKeyId(self.key_id.clone()))?;

        let engine = base64::engine::general_purpose::STANDARD;
        let key_bytes = engine
            .decode(anchor.verifying_key_b64.trim())
            .map_err(|err| PolicyBundleError::MalformedKey {
                key_id: anchor.key_id.clone(),
                reason: err.to_string(),
            })?;
        let signature_bytes = engine
            .decode(self.signature_b64.trim())
            .map_err(|err| PolicyBundleError::MalformedSignature(err.to_string()))?;

        verify_ed25519_signature(self.payload_toml.as_bytes(), &signature_bytes, &key_bytes)
            .map_err(|err| match err {
                Ed25519VerifyFailure::InvalidKey(reason) => PolicyBundleError::MalformedKey {
                    key_id: anchor.key_id.clone(),
                    reason,
                },
                Ed25519VerifyFailure::VerifyFailed(reason) => {
                    PolicyBundleError::SignatureMismatch(reason)
                }
            })?;

        let bundle: super::OrgPolicyBundle = toml::from_str(&self.payload_toml)
            .map_err(|err| PolicyBundleError::MalformedPayload(err.to_string()))?;

        if bundle.schema_version != POLICY_BUNDLE_SCHEMA_VERSION {
            return Err(PolicyBundleError::UnsupportedSchemaVersion {
                found: bundle.schema_version,
                supported: POLICY_BUNDLE_SCHEMA_VERSION,
            });
        }

        Ok(VerifiedPolicyBundle {
            bundle,
            signing_key_id: anchor.key_id.clone(),
        })
    }
}

/// Produce a detached Ed25519 signature over `data` from a raw 32-byte seed.
///
/// This is the counterpart to [`verify_ed25519_signature`] and, like it, the
/// single place the workspace reaches for `ed25519_dalek` signing. Callers that
/// need a signature over something other than a policy bundle — signed extension
/// artifacts, for instance — use this rather than depending on `ed25519-dalek`
/// themselves, so there stays exactly one signing scheme.
///
/// The seed is borrowed, used, and never copied into the returned value; the
/// output carries only the public signature. Callers are responsible for
/// zeroizing their own seed buffer, exactly as `xtask::signing` does.
pub fn sign_ed25519_detached(data: &[u8], seed: &[u8; 32]) -> [u8; 64] {
    use ed25519_dalek::Signer as _;

    let signing_key = ed25519_dalek::SigningKey::from_bytes(seed);
    let signature: ed25519_dalek::Signature = signing_key.sign(data);
    signature.to_bytes()
}

/// Derive the 32-byte Ed25519 public key for a signing seed.
///
/// Returns public key material only.
pub fn ed25519_verifying_key(seed: &[u8; 32]) -> [u8; 32] {
    ed25519_dalek::SigningKey::from_bytes(seed)
        .verifying_key()
        .to_bytes()
}

/// Sign a bundle payload with a raw 32-byte Ed25519 seed.
///
/// The seed is borrowed, used, and never copied into the returned value; the
/// output carries only the public signature. Callers are responsible for
/// zeroizing their own seed buffer, exactly as `xtask::signing` does.
pub fn sign_policy_bundle(
    payload_toml: impl Into<String>,
    key_id: impl Into<String>,
    seed: &[u8; 32],
) -> SignedPolicyBundle {
    let payload_toml = payload_toml.into();
    let signature = sign_ed25519_detached(payload_toml.as_bytes(), seed);
    SignedPolicyBundle {
        algorithm: POLICY_BUNDLE_SIGNATURE_ALGORITHM.to_string(),
        key_id: key_id.into(),
        signature_b64: base64::engine::general_purpose::STANDARD.encode(signature),
        payload_toml,
    }
}

/// Derive the base64 trust anchor for a signing seed.
///
/// Returns public key material only.
pub fn policy_bundle_verifying_key_b64(seed: &[u8; 32]) -> String {
    base64::engine::general_purpose::STANDARD.encode(ed25519_verifying_key(seed))
}

// ---------------------------------------------------------------------------
// Per-surface bundle policies
// ---------------------------------------------------------------------------

/// Case-insensitive membership test that treats an empty list as "deny all".
///
/// The vacuous-truth guard matters: `iter().any(..)` over an empty list is
/// `false`, which is already deny — but an author reading `allowlist.is_empty()`
/// as "unconfigured, therefore unrestricted" is the classic way this goes wrong,
/// so the intent is written down once here and reused by every surface.
fn allowlist_contains(allowlist: &[String], candidate: &str) -> bool {
    let candidate = candidate.trim();
    if candidate.is_empty() {
        return false;
    }
    allowlist
        .iter()
        .any(|allowed| allowed.trim().eq_ignore_ascii_case(candidate))
}

/// Whether `capability` starts with any of the configured prefixes.
fn matches_any_prefix(prefixes: &[String], capability: &str) -> bool {
    prefixes
        .iter()
        .any(|prefix| !prefix.is_empty() && capability.starts_with(prefix.as_str()))
}

/// Provider allowlist: which AI providers this org permits by name.
///
/// The pre-existing [`AiProviderPolicy`](super::AiProviderPolicy) only separates
/// local from remote by network target. That cannot express "Anthropic yes,
/// everything else no", which is the actual enterprise requirement, so provider
/// *identity* is allowlisted here.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct ProviderAllowlistPolicy {
    /// Whether the allowlist is enforced.
    pub enforced: bool,
    /// Provider identifiers that may be invoked. Empty denies every provider.
    pub allowed_provider_ids: Vec<String>,
    /// Capability prefixes that must name a provider.
    pub provider_capability_prefixes: Vec<String>,
}

impl ProviderAllowlistPolicy {
    /// Prefixes used when a bundle does not name its own.
    pub fn default_capability_prefixes() -> Vec<String> {
        vec!["ai.provider.".to_string()]
    }

    fn prefixes(&self) -> Vec<String> {
        if self.provider_capability_prefixes.is_empty() {
            Self::default_capability_prefixes()
        } else {
            self.provider_capability_prefixes.clone()
        }
    }

    /// Evaluate a request, returning `Some(deny_reason)` when it is refused.
    pub fn refusal(&self, capability: &str, provider_id: Option<&str>) -> Option<String> {
        if !self.enforced {
            return None;
        }
        // Same two triggers as the MCP allowlist: a capability the author
        // declared to be a provider call must declare its provider, and a
        // request that names a provider is checked whatever its capability id.
        if !matches_any_prefix(&self.prefixes(), capability) && provider_id.is_none() {
            return None;
        }
        let Some(provider_id) = provider_id.map(str::trim).filter(|id| !id.is_empty()) else {
            return Some(format!(
                "capability `{capability}` did not declare a provider id; the org policy \
                 bundle's provider allowlist cannot evaluate an undeclared provider"
            ));
        };
        if allowlist_contains(&self.allowed_provider_ids, provider_id) {
            None
        } else {
            Some(format!(
                "provider `{provider_id}` is not on the org policy bundle provider allowlist"
            ))
        }
    }
}

/// MCP server and tool allowlist.
///
/// Both dimensions are checked. A tool named on a server that is not allowlisted
/// is refused, and a tool that is not itself allowlisted is refused even when its
/// server is — otherwise adding a server would silently admit every tool it later
/// chooses to advertise.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct McpToolAllowlistPolicy {
    /// Whether the allowlist is enforced.
    pub enforced: bool,
    /// MCP server ids that may be reached. Empty denies every server.
    pub allowed_servers: Vec<String>,
    /// Fully qualified `server/tool` names that may be called. Empty denies all.
    pub allowed_tools: Vec<String>,
    /// Capability prefixes that must name an MCP server and tool.
    pub tool_capability_prefixes: Vec<String>,
}

impl McpToolAllowlistPolicy {
    /// Prefixes used when a bundle does not name its own.
    ///
    /// `delegate.tool.mcp-passthrough` is the capability id the delegated-task
    /// loop mints for an MCP call (`legion-agent`'s `check_broker_capability`).
    /// It is listed explicitly because it does not start with `mcp.`, and
    /// leaving it out would mean an MCP call that declares no server or tool id
    /// slips past the allowlist instead of being refused for not declaring one.
    pub fn default_capability_prefixes() -> Vec<String> {
        vec![
            "mcp.".to_string(),
            "tool.".to_string(),
            "delegate.tool.mcp-passthrough".to_string(),
        ]
    }

    fn prefixes(&self) -> Vec<String> {
        if self.tool_capability_prefixes.is_empty() {
            Self::default_capability_prefixes()
        } else {
            self.tool_capability_prefixes.clone()
        }
    }

    /// Canonical `server/tool` name used in the allowlist.
    pub fn qualified_tool_name(server_id: &str, tool_name: &str) -> String {
        format!("{}/{}", server_id.trim(), tool_name.trim())
    }

    /// Evaluate a request, returning `Some(deny_reason)` when it is refused.
    pub fn refusal(
        &self,
        capability: &str,
        server_id: Option<&str>,
        tool_name: Option<&str>,
    ) -> Option<String> {
        if !self.enforced {
            return None;
        }
        let server_id = server_id.map(str::trim).filter(|id| !id.is_empty());
        let tool_name = tool_name.map(str::trim).filter(|name| !name.is_empty());
        // Two independent triggers. The prefix catches a capability the policy
        // author declared to be an MCP call, so omitting the operands is a
        // denial rather than a bypass. The operand check catches a call whose
        // capability id the author did not anticipate but which nonetheless
        // names an MCP server or tool — a bundle that only matched prefixes
        // would let a renamed capability route around the allowlist.
        if !matches_any_prefix(&self.prefixes(), capability)
            && server_id.is_none()
            && tool_name.is_none()
        {
            return None;
        }
        let (Some(server_id), Some(tool_name)) = (server_id, tool_name) else {
            return Some(format!(
                "capability `{capability}` did not declare both an MCP server id and tool \
                 name; the org policy bundle's tool allowlist cannot evaluate an \
                 undeclared tool"
            ));
        };
        if !allowlist_contains(&self.allowed_servers, server_id) {
            return Some(format!(
                "MCP server `{server_id}` is not on the org policy bundle server allowlist"
            ));
        }
        let qualified = Self::qualified_tool_name(server_id, tool_name);
        if allowlist_contains(&self.allowed_tools, &qualified) {
            None
        } else {
            Some(format!(
                "MCP tool `{qualified}` is not on the org policy bundle tool allowlist"
            ))
        }
    }
}

/// Budget ceilings a bundle imposes on cost- and token-bearing requests.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct BudgetCapPolicy {
    /// Whether budget caps are enforced.
    pub enforced: bool,
    /// Maximum cost, in cents, for a single request.
    pub max_request_cost_cents: u64,
    /// Maximum model tokens for a single request.
    pub max_request_tokens: u64,
    /// Maximum cumulative cost, in cents, across the enclosing session.
    pub max_session_cost_cents: u64,
    /// Capability prefixes whose requests must declare a cost before they run.
    pub cost_declaration_required_prefixes: Vec<String>,
}

impl BudgetCapPolicy {
    /// Evaluate a request, returning `Some(deny_reason)` when it is refused.
    ///
    /// A request that declares no cost is refused for capabilities named in
    /// `cost_declaration_required_prefixes`. Without that rule a caller could
    /// bypass every cap simply by omitting the estimate.
    pub fn refusal(
        &self,
        capability: &str,
        request_cost_cents: Option<u64>,
        request_tokens: Option<u64>,
        session_spent_cents: Option<u64>,
    ) -> Option<String> {
        if !self.enforced {
            return None;
        }

        let declaration_required =
            matches_any_prefix(&self.cost_declaration_required_prefixes, capability);
        if declaration_required && request_cost_cents.is_none() {
            return Some(format!(
                "capability `{capability}` did not declare an estimated cost; the org policy \
                 bundle's budget cap requires a declared cost before the request runs"
            ));
        }

        if let Some(cost) = request_cost_cents
            && cost > self.max_request_cost_cents
        {
            return Some(format!(
                "request cost {cost} cents exceeds the org policy bundle per-request cap of {} cents",
                self.max_request_cost_cents
            ));
        }

        if let Some(tokens) = request_tokens
            && tokens > self.max_request_tokens
        {
            return Some(format!(
                "request of {tokens} tokens exceeds the org policy bundle per-request token cap of {}",
                self.max_request_tokens
            ));
        }

        let spent = session_spent_cents.unwrap_or(0);
        let projected = spent.saturating_add(request_cost_cents.unwrap_or(0));
        if projected > self.max_session_cost_cents {
            return Some(format!(
                "session spend would reach {projected} cents, exceeding the org policy bundle \
                 session cap of {} cents",
                self.max_session_cost_cents
            ));
        }

        None
    }
}

/// Retention window and export destination rules.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct RetentionExportPolicy {
    /// Whether retention and export rules are enforced.
    pub enforced: bool,
    /// Longest retention window, in days, the org permits.
    pub max_retention_days: u32,
    /// Whether any export is permitted at all.
    pub export_enabled: bool,
    /// Destination labels an export may target. Empty denies every destination.
    pub allowed_export_destinations: Vec<String>,
    /// Capability prefixes governed by the retention window rule.
    pub retention_capability_prefixes: Vec<String>,
    /// Capability substrings that identify an export.
    pub export_capability_markers: Vec<String>,
}

impl RetentionExportPolicy {
    /// Prefixes used when a bundle does not name its own.
    pub fn default_retention_prefixes() -> Vec<String> {
        vec!["retention.".to_string(), "memory.retain".to_string()]
    }

    /// Markers used when a bundle does not name its own.
    pub fn default_export_markers() -> Vec<String> {
        vec![".export".to_string()]
    }

    fn retention_prefixes(&self) -> Vec<String> {
        if self.retention_capability_prefixes.is_empty() {
            Self::default_retention_prefixes()
        } else {
            self.retention_capability_prefixes.clone()
        }
    }

    fn export_markers(&self) -> Vec<String> {
        if self.export_capability_markers.is_empty() {
            Self::default_export_markers()
        } else {
            self.export_capability_markers.clone()
        }
    }

    /// Whether the capability is an export under this policy.
    pub fn is_export_capability(&self, capability: &str) -> bool {
        self.export_markers()
            .iter()
            .any(|marker| !marker.is_empty() && capability.contains(marker.as_str()))
    }

    /// Retention-window refusal, returning `Some(deny_reason)` when refused.
    pub fn retention_refusal(
        &self,
        capability: &str,
        requested_days: Option<u32>,
    ) -> Option<String> {
        if !self.enforced {
            return None;
        }
        if !matches_any_prefix(&self.retention_prefixes(), capability) && requested_days.is_none() {
            return None;
        }
        let Some(days) = requested_days else {
            // An export is governed by the export rule below. Demanding a
            // retention window from it as well would refuse every export for
            // the wrong reason and hide which rule actually objected.
            if self.is_export_capability(capability) {
                return None;
            }
            return Some(format!(
                "capability `{capability}` did not declare a retention window; the org policy \
                 bundle requires a declared window it can bound"
            ));
        };
        if days > self.max_retention_days {
            return Some(format!(
                "retention window of {days} days exceeds the org policy bundle maximum of {} days",
                self.max_retention_days
            ));
        }
        None
    }

    /// Export refusal, returning `Some(deny_reason)` when refused.
    pub fn export_refusal(&self, capability: &str, destination: Option<&str>) -> Option<String> {
        if !self.enforced {
            return None;
        }
        if !self.is_export_capability(capability) && destination.is_none() {
            return None;
        }
        if !self.export_enabled {
            return Some(format!(
                "export capability `{capability}` is disabled by the org policy bundle"
            ));
        }
        let Some(destination) = destination.map(str::trim).filter(|dest| !dest.is_empty()) else {
            return Some(format!(
                "export capability `{capability}` did not declare a destination; the org policy \
                 bundle cannot evaluate an undeclared export target"
            ));
        };
        if allowlist_contains(&self.allowed_export_destinations, destination) {
            None
        } else {
            Some(format!(
                "export destination `{destination}` is not on the org policy bundle export allowlist"
            ))
        }
    }
}

/// The bundle-level policies that reach surfaces the base broker did not cover.
///
/// This lives inside [`SecurityPolicy`](super::SecurityPolicy) rather than beside
/// it so that every existing caller of the broker — every tool call routed through
/// the capability broker under P5.F1.T2 — is subject to it without needing a new
/// call site.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct BundleEnforcementPolicy {
    /// Provider allowlist.
    pub provider: ProviderAllowlistPolicy,
    /// MCP server and tool allowlist.
    pub mcp: McpToolAllowlistPolicy,
    /// Budget caps.
    pub budget: BudgetCapPolicy,
    /// Retention window and export destination rules.
    pub retention_export: RetentionExportPolicy,
}

impl BundleEnforcementPolicy {
    /// The request cost a budget cap should be applied to.
    ///
    /// A cloud-lane task's estimated cost is a request cost. Falling back to it
    /// means every existing `cloud.lane.submit` call site is budget-capped by an
    /// org bundle without changing its code — and, more importantly, that a
    /// caller cannot dodge the cap by filling in only the older field.
    pub fn effective_request_cost_cents(
        context: &legion_protocol::CapabilityRequestContext,
    ) -> Option<u64> {
        context
            .budget_request_cost_cents
            .or_else(|| context.cloud_lane_estimated_cost_cents.map(u64::from))
    }

    /// First refusal across the provider, MCP, budget, retention, and export
    /// rules, paired with the surface that produced it.
    ///
    /// The mode-ceiling and base-capability surfaces are not evaluated here:
    /// mode is a bundle-level field and the base capability matrix is the broker
    /// itself. [`VerifiedPolicyBundle::decide`] runs all seven.
    pub fn refusal(
        &self,
        capability: &str,
        context: &legion_protocol::CapabilityRequestContext,
    ) -> Option<(PolicySurface, String)> {
        if let Some(reason) = self
            .provider
            .refusal(capability, context.ai_provider_id.as_deref())
        {
            return Some((PolicySurface::Provider, reason));
        }
        if let Some(reason) = self.mcp.refusal(
            capability,
            context.mcp_server_id.as_deref(),
            context.mcp_tool_name.as_deref(),
        ) {
            return Some((PolicySurface::McpTool, reason));
        }
        if let Some(reason) = self.budget.refusal(
            capability,
            Self::effective_request_cost_cents(context),
            context.budget_request_tokens,
            context.budget_session_spent_cents,
        ) {
            return Some((PolicySurface::Budget, reason));
        }
        if let Some(reason) = self
            .retention_export
            .retention_refusal(capability, context.retention_requested_days)
        {
            return Some((PolicySurface::Retention, reason));
        }
        if let Some(reason) = self
            .retention_export
            .export_refusal(capability, context.export_destination.as_deref())
        {
            return Some((PolicySurface::Export, reason));
        }
        None
    }
}

// ---------------------------------------------------------------------------
// Surface coverage
// ---------------------------------------------------------------------------

/// Every surface a signed policy bundle must reach.
///
/// The stop condition for P9.F2.T3 is that a bundle honoured on only some
/// surfaces is a failure, so the surface set is a first-class enumeration rather
/// than a list in a comment. [`VerifiedPolicyBundle::SURFACE_CHECKS`] holds one
/// evaluator per variant and `decide` iterates that table, so a surface cannot be
/// added to the product without an evaluator that runs on every request.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PolicySurface {
    /// Which AI providers may be invoked.
    Provider,
    /// Which MCP servers and tools may be called.
    McpTool,
    /// The highest product mode the bundle permits.
    Mode,
    /// Per-request and per-session cost and token ceilings.
    Budget,
    /// How long captured raw source may be retained.
    Retention,
    /// Whether and where data may be exported.
    Export,
    /// The base deny-by-default capability matrix.
    Capability,
}

impl PolicySurface {
    /// Every surface, in evaluation order.
    pub const ALL: [PolicySurface; 7] = [
        PolicySurface::Mode,
        PolicySurface::Provider,
        PolicySurface::McpTool,
        PolicySurface::Budget,
        PolicySurface::Retention,
        PolicySurface::Export,
        PolicySurface::Capability,
    ];

    /// Stable identifier used in audit rows.
    pub fn stable_id(self) -> &'static str {
        match self {
            Self::Provider => "policy.surface.provider",
            Self::McpTool => "policy.surface.mcp_tool",
            Self::Mode => "policy.surface.mode",
            Self::Budget => "policy.surface.budget",
            Self::Retention => "policy.surface.retention",
            Self::Export => "policy.surface.export",
            Self::Capability => "policy.surface.capability",
        }
    }
}

/// One capability request evaluated against a verified bundle.
#[derive(Debug, Clone)]
pub struct BundleRequest<'a> {
    /// Product mode the request is made in.
    pub mode: legion_protocol::ProductMode,
    /// Workspace trust state.
    pub trust: super::TrustState,
    /// Requesting principal.
    pub principal: legion_protocol::PrincipalId,
    /// Capability being requested.
    pub capability: legion_protocol::CapabilityId,
    /// Path operand, when the capability has one.
    pub path: Option<&'a str>,
    /// Structured operation context carrying the per-surface operands.
    pub context: legion_protocol::CapabilityRequestContext,
}

/// The outcome of evaluating a [`BundleRequest`] against a verified bundle.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BundleDecision {
    /// Surface that produced the verdict.
    pub surface: PolicySurface,
    /// Bundle that was enforced.
    pub bundle_id: String,
    /// Trust anchor whose signature was verified before enforcement.
    pub signing_key_id: String,
    /// Capability that was evaluated.
    pub capability: String,
    /// Allow or deny, with the reason when denied.
    pub decision: super::SecurityDecision,
}

impl BundleDecision {
    /// Whether the request may proceed.
    pub fn is_allowed(&self) -> bool {
        matches!(self.decision, super::SecurityDecision::Allow)
    }

    /// A display-safe audit row naming the bundle, key, surface, and verdict.
    ///
    /// Carries metadata only: no payloads, no path contents, no key material.
    pub fn audit_row(&self) -> String {
        let verdict = match &self.decision {
            super::SecurityDecision::Allow => "allow".to_string(),
            super::SecurityDecision::Deny(reason) => format!("deny ({reason})"),
        };
        format!(
            "policy-bundle={} key={} surface={} capability={} decision={verdict}",
            self.bundle_id,
            self.signing_key_id,
            self.surface.stable_id(),
            self.capability
        )
    }
}

/// A policy bundle whose signature has been verified against a trust anchor.
///
/// Construction is only possible through [`SignedPolicyBundle::verify`]. There is
/// no `new`, no `From<OrgPolicyBundle>`, and the fields are private, so "an
/// unsigned bundle honoured as if signed" is not a runtime mistake that can be
/// made — it does not typecheck.
#[derive(Debug, Clone)]
pub struct VerifiedPolicyBundle {
    bundle: super::OrgPolicyBundle,
    signing_key_id: String,
}

/// One surface evaluator: `None` means the surface raised no objection.
type SurfaceCheck =
    fn(&VerifiedPolicyBundle, &BundleRequest<'_>) -> Option<super::SecurityDecision>;

impl VerifiedPolicyBundle {
    /// One evaluator per [`PolicySurface`], in evaluation order.
    ///
    /// `decide` iterates this table rather than hard-coding a sequence of `if`
    /// blocks, and `policy_surface_checks_cover_every_surface` asserts the table
    /// matches [`PolicySurface::ALL`] entry for entry. Together those make a
    /// half-covered bundle a test failure rather than a silent gap.
    pub const SURFACE_CHECKS: [(PolicySurface, SurfaceCheck); 7] = [
        (PolicySurface::Mode, Self::check_mode),
        (PolicySurface::Provider, Self::check_provider),
        (PolicySurface::McpTool, Self::check_mcp_tool),
        (PolicySurface::Budget, Self::check_budget),
        (PolicySurface::Retention, Self::check_retention),
        (PolicySurface::Export, Self::check_export),
        (PolicySurface::Capability, Self::check_capability),
    ];

    /// The verified bundle payload.
    pub fn bundle(&self) -> &super::OrgPolicyBundle {
        &self.bundle
    }

    /// Trust anchor id whose signature was verified.
    pub fn signing_key_id(&self) -> &str {
        &self.signing_key_id
    }

    fn check_mode(&self, request: &BundleRequest<'_>) -> Option<super::SecurityDecision> {
        if self.bundle.allows_mode(request.mode) {
            None
        } else {
            Some(super::SecurityDecision::deny(format!(
                "{} ceiling denies {} mode request",
                self.bundle.mode_ceiling.label(),
                request.mode.label()
            )))
        }
    }

    fn enforcement(&self) -> &BundleEnforcementPolicy {
        &self.bundle.security_policy.bundle_enforcement
    }

    fn check_provider(&self, request: &BundleRequest<'_>) -> Option<super::SecurityDecision> {
        self.enforcement()
            .provider
            .refusal(
                &request.capability.0,
                request.context.ai_provider_id.as_deref(),
            )
            .map(super::SecurityDecision::deny)
    }

    fn check_mcp_tool(&self, request: &BundleRequest<'_>) -> Option<super::SecurityDecision> {
        self.enforcement()
            .mcp
            .refusal(
                &request.capability.0,
                request.context.mcp_server_id.as_deref(),
                request.context.mcp_tool_name.as_deref(),
            )
            .map(super::SecurityDecision::deny)
    }

    fn check_budget(&self, request: &BundleRequest<'_>) -> Option<super::SecurityDecision> {
        self.enforcement()
            .budget
            .refusal(
                &request.capability.0,
                BundleEnforcementPolicy::effective_request_cost_cents(&request.context),
                request.context.budget_request_tokens,
                request.context.budget_session_spent_cents,
            )
            .map(super::SecurityDecision::deny)
    }

    fn check_retention(&self, request: &BundleRequest<'_>) -> Option<super::SecurityDecision> {
        self.enforcement()
            .retention_export
            .retention_refusal(
                &request.capability.0,
                request.context.retention_requested_days,
            )
            .map(super::SecurityDecision::deny)
    }

    fn check_export(&self, request: &BundleRequest<'_>) -> Option<super::SecurityDecision> {
        self.enforcement()
            .retention_export
            .export_refusal(
                &request.capability.0,
                request.context.export_destination.as_deref(),
            )
            .map(super::SecurityDecision::deny)
    }

    fn check_capability(&self, request: &BundleRequest<'_>) -> Option<super::SecurityDecision> {
        let mut broker = self.bundle.broker();
        match broker.decide_with_request_context(
            request.trust,
            request.principal.clone(),
            request.capability.clone(),
            request.path,
            request.context.clone(),
        ) {
            super::SecurityDecision::Allow => None,
            deny @ super::SecurityDecision::Deny(_) => Some(deny),
        }
    }

    /// Evaluate a request against every surface the bundle governs.
    ///
    /// The first refusing surface wins and is named in the returned decision, so
    /// a denial always says which rule refused it. A request only reaches
    /// `Allow` after all seven surfaces have declined to refuse it.
    pub fn decide(&self, request: &BundleRequest<'_>) -> BundleDecision {
        for (surface, check) in Self::SURFACE_CHECKS {
            if let Some(decision) = check(self, request) {
                return BundleDecision {
                    surface,
                    bundle_id: self.bundle.bundle_id.clone(),
                    signing_key_id: self.signing_key_id.clone(),
                    capability: request.capability.0.clone(),
                    decision,
                };
            }
        }
        BundleDecision {
            surface: PolicySurface::Capability,
            bundle_id: self.bundle.bundle_id.clone(),
            signing_key_id: self.signing_key_id.clone(),
            capability: request.capability.0.clone(),
            decision: super::SecurityDecision::Allow,
        }
    }
}

/// Record of one quota dimension that was reduced below the plugin's request.
///
/// A clamp is the audit evidence that a plugin asked for more than the host
/// grants. Every clamp is surfaced to the caller so it can be written to the
/// plugin audit log; a clamp is never applied silently.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PluginQuotaClamp {
    /// Quota dimension that was reduced.
    pub class: legion_protocol::PluginQuotaClass,
    /// Value the plugin manifest asked for.
    pub declared: u64,
    /// Value the host actually granted.
    pub granted: u64,
}

/// Quotas actually granted to a plugin, plus the record of every reduction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginQuotaGrant {
    /// Enforced quota values. Never larger than the host ceiling.
    pub granted: legion_protocol::PluginQuotaDeclaration,
    /// One entry per dimension the manifest tried to exceed.
    pub clamps: Vec<PluginQuotaClamp>,
}

impl PluginQuotaGrant {
    /// Whether the manifest asked for more than the host was willing to give.
    pub fn was_clamped(&self) -> bool {
        !self.clamps.is_empty()
    }
}

/// Host-owned ceiling on plugin runtime quotas.
///
/// # Why this type has no "enforced" switch
///
/// Plugin quotas are enforced unconditionally. There is deliberately no
/// `enforced: bool`, no `unlimited` sentinel, and no per-plugin override: a
/// plugin manifest is attacker-controlled input, so if a manifest could raise
/// its own ceiling — or set a dimension to a value the host reads as
/// "unlimited" — every quota would be optional in practice. [`Self::grant`]
/// therefore takes the *minimum* of the manifest's request and the ceiling,
/// which can only ever narrow what a plugin receives.
///
/// [`Self::HARD_MAX`] is a second floor under the first: even a configured or
/// deserialized ceiling is itself clamped, so a policy bundle cannot widen the
/// sandbox beyond what this crate compiled in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct PluginQuotaCeiling {
    /// Maximum fuel units granted for a single invocation.
    pub max_fuel: u64,
    /// Maximum wall-clock milliseconds granted for a single invocation.
    pub max_wall_time_ms: u64,
    /// Maximum WebAssembly memory pages granted to the guest.
    pub max_memory_pages: u32,
    /// Maximum storage bytes granted to the plugin.
    pub max_storage_bytes: u64,
    /// Maximum host calls granted for a single invocation.
    pub max_host_calls: u32,
    /// Maximum invocations granted over the plugin's lifetime.
    pub max_events: u32,
    /// Maximum bytes a single host call may hand to the host.
    pub max_output_bytes: u64,
}

impl PluginQuotaCeiling {
    /// Absolute compiled-in maximum.
    ///
    /// No configuration file, policy bundle, or plugin manifest can raise a
    /// quota past these values, because [`Self::grant`] mins against them in
    /// addition to `self`.
    pub const HARD_MAX: Self = Self {
        max_fuel: 50_000_000,
        max_wall_time_ms: 5_000,
        max_memory_pages: 256,
        max_storage_bytes: 8 * 1024 * 1024,
        max_host_calls: 1_024,
        max_events: 4_096,
        max_output_bytes: 256 * 1024,
    };

    /// The ceiling as it is actually applied: `min(self, HARD_MAX)` per field.
    pub fn effective(&self) -> Self {
        Self {
            max_fuel: self.max_fuel.min(Self::HARD_MAX.max_fuel),
            max_wall_time_ms: self.max_wall_time_ms.min(Self::HARD_MAX.max_wall_time_ms),
            max_memory_pages: self.max_memory_pages.min(Self::HARD_MAX.max_memory_pages),
            max_storage_bytes: self.max_storage_bytes.min(Self::HARD_MAX.max_storage_bytes),
            max_host_calls: self.max_host_calls.min(Self::HARD_MAX.max_host_calls),
            max_events: self.max_events.min(Self::HARD_MAX.max_events),
            max_output_bytes: self.max_output_bytes.min(Self::HARD_MAX.max_output_bytes),
        }
    }

    /// Grant a plugin the smaller of what it declared and what the host allows.
    ///
    /// Every dimension where the declaration lost is reported in
    /// [`PluginQuotaGrant::clamps`] so the host can audit the attempt.
    pub fn grant(&self, declared: &legion_protocol::PluginQuotaDeclaration) -> PluginQuotaGrant {
        use legion_protocol::PluginQuotaClass;

        let ceiling = self.effective();
        let mut clamps = Vec::new();

        let mut clamp_u64 = |class: PluginQuotaClass, declared: u64, ceiling: u64| -> u64 {
            if declared > ceiling {
                clamps.push(PluginQuotaClamp {
                    class,
                    declared,
                    granted: ceiling,
                });
                ceiling
            } else {
                declared
            }
        };

        let max_fuel = clamp_u64(PluginQuotaClass::Fuel, declared.max_fuel, ceiling.max_fuel);
        let max_wall_time_ms = clamp_u64(
            PluginQuotaClass::WallTime,
            declared.max_wall_time_ms,
            ceiling.max_wall_time_ms,
        );
        let max_memory_pages = clamp_u64(
            PluginQuotaClass::Memory,
            u64::from(declared.max_memory_pages),
            u64::from(ceiling.max_memory_pages),
        ) as u32;
        let max_storage_bytes = clamp_u64(
            PluginQuotaClass::Storage,
            declared.max_storage_bytes,
            ceiling.max_storage_bytes,
        );
        let max_host_calls = clamp_u64(
            PluginQuotaClass::HostCall,
            u64::from(declared.max_host_calls),
            u64::from(ceiling.max_host_calls),
        ) as u32;
        let max_events = clamp_u64(
            PluginQuotaClass::Event,
            u64::from(declared.max_events),
            u64::from(ceiling.max_events),
        ) as u32;
        let max_output_bytes = clamp_u64(
            PluginQuotaClass::Output,
            declared.max_output_bytes,
            ceiling.max_output_bytes,
        );

        PluginQuotaGrant {
            granted: legion_protocol::PluginQuotaDeclaration {
                max_fuel,
                max_wall_time_ms,
                max_memory_pages,
                max_storage_bytes,
                max_host_calls,
                max_events,
                max_output_bytes,
            },
            clamps,
        }
    }
}

impl Default for PluginQuotaCeiling {
    /// The shipped ceiling, well below [`Self::HARD_MAX`].
    fn default() -> Self {
        Self {
            max_fuel: 5_000_000,
            max_wall_time_ms: 2_000,
            max_memory_pages: 64,
            max_storage_bytes: 1024 * 1024,
            max_host_calls: 64,
            max_events: 256,
            max_output_bytes: 64 * 1024,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_rule_ids_are_never_auto_approved() {
        let policy = ProposalAutoApprovalPolicy {
            enabled: true,
            allowed_rule_ids: vec!["rule-a".to_string()],
        };
        assert!(!policy.allows_rule_ids(&[]));
    }

    #[test]
    fn disabled_policy_rejects_all_rule_ids() {
        let policy = ProposalAutoApprovalPolicy {
            enabled: false,
            allowed_rule_ids: vec!["rule-a".to_string()],
        };
        assert!(!policy.allows_rule_ids(&["rule-a".to_string()]));
    }

    #[test]
    fn debug_adapter_policy_allowlists_known_adapters_only() {
        let policy = DebugAdapterLaunchPolicy::default();
        assert!(policy.allows_adapter_binary("lldb-dap"));
        assert!(policy.allows_adapter_binary("codelldb"));
        // Case folding matters on Windows, where the stem may arrive as `CodeLLDB`.
        assert!(policy.allows_adapter_binary("CodeLLDB"));
        assert!(!policy.allows_adapter_binary("bash"));
        assert!(!policy.allows_adapter_binary("fake_dap_adapter"));
    }

    #[test]
    fn debug_adapter_policy_empty_allowlist_denies_every_binary() {
        // Vacuous-truth guard: an empty list must not mean "anything goes".
        let policy = DebugAdapterLaunchPolicy {
            require_trusted_workspace: true,
            allowed_adapter_binaries: Vec::new(),
        };
        assert!(!policy.allows_adapter_binary("lldb-dap"));
        assert!(!policy.allows_adapter_binary(""));
    }

    #[test]
    fn debug_adapter_policy_rejects_blank_binary_names() {
        let policy = DebugAdapterLaunchPolicy {
            require_trusted_workspace: true,
            allowed_adapter_binaries: vec![String::new(), "  ".to_string()],
        };
        assert!(!policy.allows_adapter_binary(""));
        assert!(!policy.allows_adapter_binary("   "));
        assert!(!policy.allows_adapter_binary("lldb-dap"));
    }

    // -----------------------------------------------------------------
    // Signed bundle surfaces (P9.F2.T3)
    // -----------------------------------------------------------------

    #[test]
    fn policy_surface_all_lists_every_variant_exactly_once() {
        // Exhaustive match: a new variant will not compile until it is given a
        // slot, and the assertions then fail unless `ALL` grew to hold it.
        fn position(surface: PolicySurface) -> usize {
            match surface {
                PolicySurface::Mode => 0,
                PolicySurface::Provider => 1,
                PolicySurface::McpTool => 2,
                PolicySurface::Budget => 3,
                PolicySurface::Retention => 4,
                PolicySurface::Export => 5,
                PolicySurface::Capability => 6,
            }
        }
        for (index, surface) in PolicySurface::ALL.iter().enumerate() {
            assert_eq!(position(*surface), index);
        }
        assert_eq!(PolicySurface::ALL.len(), 7);

        let ids: std::collections::HashSet<&str> = PolicySurface::ALL
            .iter()
            .map(|surface| surface.stable_id())
            .collect();
        assert_eq!(
            ids.len(),
            PolicySurface::ALL.len(),
            "stable ids must differ"
        );
    }

    #[test]
    fn provider_allowlist_empty_list_denies_every_provider() {
        // The vacuous-truth guard. `[]` must mean "nothing", never "anything".
        let policy = ProviderAllowlistPolicy {
            enforced: true,
            allowed_provider_ids: Vec::new(),
            provider_capability_prefixes: Vec::new(),
        };
        assert!(
            policy
                .refusal("ai.provider.invoke", Some("ollama"))
                .is_some()
        );
        assert!(policy.refusal("ai.provider.invoke", None).is_some());
        assert!(policy.refusal("ai.provider.invoke", Some("   ")).is_some());
    }

    #[test]
    fn provider_allowlist_is_inert_until_enforced() {
        let policy = ProviderAllowlistPolicy {
            enforced: false,
            allowed_provider_ids: vec!["ollama".to_string()],
            provider_capability_prefixes: Vec::new(),
        };
        assert!(
            policy
                .refusal("ai.provider.invoke", Some("openai"))
                .is_none()
        );
    }

    #[test]
    fn provider_allowlist_matches_case_insensitively() {
        let policy = ProviderAllowlistPolicy {
            enforced: true,
            allowed_provider_ids: vec!["Ollama".to_string()],
            provider_capability_prefixes: Vec::new(),
        };
        assert!(
            policy
                .refusal("ai.provider.invoke", Some("ollama"))
                .is_none()
        );
        assert!(
            policy
                .refusal("ai.provider.invoke", Some("openai"))
                .is_some()
        );
    }

    #[test]
    fn mcp_allowlist_checks_server_and_tool_independently() {
        let policy = McpToolAllowlistPolicy {
            enforced: true,
            allowed_servers: vec!["legion-internal".to_string()],
            allowed_tools: vec!["legion-internal/search_docs".to_string()],
            tool_capability_prefixes: Vec::new(),
        };
        let cap = "mcp.tool.call";
        assert!(
            policy
                .refusal(cap, Some("legion-internal"), Some("search_docs"))
                .is_none()
        );
        // Allowlisted server, tool that is not listed.
        assert!(
            policy
                .refusal(cap, Some("legion-internal"), Some("run_shell"))
                .is_some()
        );
        // Listed tool name, but on a server that is not allowlisted. Matching on
        // the bare tool name would wrongly admit this.
        assert!(
            policy
                .refusal(cap, Some("evil-corp"), Some("search_docs"))
                .is_some()
        );
        assert!(policy.refusal(cap, None, None).is_some());
    }

    #[test]
    fn mcp_allowlist_empty_lists_deny_everything() {
        let policy = McpToolAllowlistPolicy {
            enforced: true,
            allowed_servers: Vec::new(),
            allowed_tools: Vec::new(),
            tool_capability_prefixes: Vec::new(),
        };
        assert!(
            policy
                .refusal("mcp.tool.call", Some("anything"), Some("anything"))
                .is_some()
        );
    }

    #[test]
    fn budget_cap_refuses_undeclared_cost_for_required_prefixes() {
        let policy = BudgetCapPolicy {
            enforced: true,
            max_request_cost_cents: 25,
            max_request_tokens: 1_000,
            max_session_cost_cents: 500,
            cost_declaration_required_prefixes: vec!["ai.provider.".to_string()],
        };
        // Undeclared cost on a capability that must declare one.
        assert!(
            policy
                .refusal("ai.provider.invoke", None, None, None)
                .is_some()
        );
        // Declared and inside every cap.
        assert!(
            policy
                .refusal("ai.provider.invoke", Some(5), Some(10), Some(0))
                .is_none()
        );
        // Each cap refuses on its own.
        assert!(
            policy
                .refusal("ai.provider.invoke", Some(26), Some(10), Some(0))
                .is_some()
        );
        assert!(
            policy
                .refusal("ai.provider.invoke", Some(5), Some(1_001), Some(0))
                .is_some()
        );
        assert!(
            policy
                .refusal("ai.provider.invoke", Some(5), Some(10), Some(496))
                .is_some()
        );
    }

    #[test]
    fn budget_session_cap_cannot_be_overflowed_past_the_ceiling() {
        // A saturating add keeps a u64::MAX cost from wrapping to a small
        // projected total that would slip under the cap.
        let policy = BudgetCapPolicy {
            enforced: true,
            max_request_cost_cents: u64::MAX,
            max_request_tokens: u64::MAX,
            max_session_cost_cents: 500,
            cost_declaration_required_prefixes: Vec::new(),
        };
        assert!(
            policy
                .refusal("x", Some(u64::MAX), None, Some(u64::MAX))
                .is_some()
        );
    }

    #[test]
    fn retention_policy_bounds_the_window_and_requires_one() {
        let policy = RetentionExportPolicy {
            enforced: true,
            max_retention_days: 7,
            export_enabled: false,
            allowed_export_destinations: Vec::new(),
            retention_capability_prefixes: Vec::new(),
            export_capability_markers: Vec::new(),
        };
        let cap = "retention.raw_source.capture";
        assert!(policy.retention_refusal(cap, Some(7)).is_none());
        assert!(policy.retention_refusal(cap, Some(8)).is_some());
        assert!(policy.retention_refusal(cap, None).is_some());
        // An export is governed by the export rule, not by the window rule.
        assert!(
            policy
                .retention_refusal("retention.raw_source.export.hosted", None)
                .is_none()
        );
    }

    #[test]
    fn export_policy_refuses_when_disabled_and_when_destination_is_unlisted() {
        let disabled = RetentionExportPolicy {
            enforced: true,
            max_retention_days: 7,
            export_enabled: false,
            allowed_export_destinations: vec!["org-siem".to_string()],
            retention_capability_prefixes: Vec::new(),
            export_capability_markers: Vec::new(),
        };
        assert!(
            disabled
                .export_refusal("telemetry.export.hosted", Some("org-siem"))
                .is_some(),
            "export_enabled = false must refuse even an allowlisted destination"
        );

        let enabled = RetentionExportPolicy {
            export_enabled: true,
            ..disabled
        };
        assert!(
            enabled
                .export_refusal("telemetry.export.hosted", Some("org-siem"))
                .is_none()
        );
        assert!(
            enabled
                .export_refusal("telemetry.export.hosted", Some("s3://elsewhere"))
                .is_some()
        );
        assert!(
            enabled
                .export_refusal("telemetry.export.hosted", None)
                .is_some(),
            "an undeclared destination must be refused, not waved through"
        );
    }

    #[test]
    fn export_policy_with_empty_destination_allowlist_refuses_every_destination() {
        let policy = RetentionExportPolicy {
            enforced: true,
            max_retention_days: 7,
            export_enabled: true,
            allowed_export_destinations: Vec::new(),
            retention_capability_prefixes: Vec::new(),
            export_capability_markers: Vec::new(),
        };
        assert!(
            policy
                .export_refusal("telemetry.export.hosted", Some("anything"))
                .is_some()
        );
    }

    #[test]
    fn default_bundle_enforcement_refuses_nothing() {
        // Every existing caller must keep working: the rules are opt-in.
        let policy = BundleEnforcementPolicy::default();
        let context = legion_protocol::CapabilityRequestContext {
            ai_provider_id: Some("anything".to_string()),
            mcp_server_id: Some("anything".to_string()),
            mcp_tool_name: Some("anything".to_string()),
            budget_request_cost_cents: Some(u64::MAX),
            retention_requested_days: Some(u32::MAX),
            export_destination: Some("anywhere".to_string()),
            ..legion_protocol::CapabilityRequestContext::default()
        };
        assert!(policy.refusal("ai.provider.invoke", &context).is_none());
    }

    #[test]
    fn cloud_lane_estimate_is_used_as_the_request_cost_when_no_budget_field_is_set() {
        let context = legion_protocol::CapabilityRequestContext {
            cloud_lane_estimated_cost_cents: Some(200),
            ..legion_protocol::CapabilityRequestContext::default()
        };
        assert_eq!(
            BundleEnforcementPolicy::effective_request_cost_cents(&context),
            Some(200)
        );

        // The newer field wins when both are present, so a caller that has been
        // updated is not double-counted against the older estimate.
        let both = legion_protocol::CapabilityRequestContext {
            budget_request_cost_cents: Some(5),
            cloud_lane_estimated_cost_cents: Some(200),
            ..legion_protocol::CapabilityRequestContext::default()
        };
        assert_eq!(
            BundleEnforcementPolicy::effective_request_cost_cents(&both),
            Some(5)
        );
    }

    #[test]
    fn unknown_or_blank_rule_ids_are_rejected() {
        let policy = ProposalAutoApprovalPolicy {
            enabled: true,
            allowed_rule_ids: vec!["rule-a".to_string()],
        };
        assert!(!policy.allows_rule_ids(&["rule-b".to_string()]));
        assert!(!policy.allows_rule_ids(&[String::new()]));
        assert!(policy.allows_rule_ids(&["rule-a".to_string()]));
    }

    fn greedy_declaration() -> legion_protocol::PluginQuotaDeclaration {
        legion_protocol::PluginQuotaDeclaration {
            max_fuel: u64::MAX,
            max_wall_time_ms: u64::MAX,
            max_memory_pages: u32::MAX,
            max_storage_bytes: u64::MAX,
            max_host_calls: u32::MAX,
            max_events: u32::MAX,
            max_output_bytes: u64::MAX,
        }
    }

    #[test]
    fn a_manifest_cannot_raise_its_own_quotas_above_the_ceiling() {
        // The manifest is attacker-controlled. Asking for everything must get
        // the host ceiling, not the request.
        let ceiling = PluginQuotaCeiling::default();
        let grant = ceiling.grant(&greedy_declaration());

        assert_eq!(grant.granted.max_fuel, ceiling.max_fuel);
        assert_eq!(grant.granted.max_wall_time_ms, ceiling.max_wall_time_ms);
        assert_eq!(grant.granted.max_memory_pages, ceiling.max_memory_pages);
        assert_eq!(grant.granted.max_storage_bytes, ceiling.max_storage_bytes);
        assert_eq!(grant.granted.max_host_calls, ceiling.max_host_calls);
        assert_eq!(grant.granted.max_events, ceiling.max_events);
        assert_eq!(grant.granted.max_output_bytes, ceiling.max_output_bytes);
    }

    #[test]
    fn every_clamped_dimension_is_reported_so_it_can_be_audited() {
        // A quota reduced without a record would be a silent clamp; the host
        // could not write an audit row for an attempt it never learned about.
        let grant = PluginQuotaCeiling::default().grant(&greedy_declaration());
        assert!(grant.was_clamped());

        let classes: Vec<_> = grant.clamps.iter().map(|clamp| clamp.class).collect();
        for expected in [
            legion_protocol::PluginQuotaClass::Fuel,
            legion_protocol::PluginQuotaClass::WallTime,
            legion_protocol::PluginQuotaClass::Memory,
            legion_protocol::PluginQuotaClass::Storage,
            legion_protocol::PluginQuotaClass::HostCall,
            legion_protocol::PluginQuotaClass::Event,
            legion_protocol::PluginQuotaClass::Output,
        ] {
            assert!(
                classes.contains(&expected),
                "clamp for {expected:?} was not reported: {classes:?}"
            );
        }
    }

    #[test]
    fn a_configured_ceiling_cannot_exceed_the_compiled_hard_max() {
        // Policy-bundle configuration is one indirection away from being
        // attacker-controlled too, so it is clamped in the same direction.
        let wide_open = PluginQuotaCeiling {
            max_fuel: u64::MAX,
            max_wall_time_ms: u64::MAX,
            max_memory_pages: u32::MAX,
            max_storage_bytes: u64::MAX,
            max_host_calls: u32::MAX,
            max_events: u32::MAX,
            max_output_bytes: u64::MAX,
        };

        assert_eq!(wide_open.effective(), PluginQuotaCeiling::HARD_MAX);

        let grant = wide_open.grant(&greedy_declaration());
        assert_eq!(
            grant.granted.max_fuel,
            PluginQuotaCeiling::HARD_MAX.max_fuel
        );
        assert_eq!(
            grant.granted.max_memory_pages,
            PluginQuotaCeiling::HARD_MAX.max_memory_pages
        );
        assert!(grant.was_clamped());
    }

    #[test]
    fn a_ceiling_deserialized_from_policy_configuration_is_still_clamped() {
        // Serde is the realistic path by which an operator-supplied ceiling
        // arrives. It must not become a quota-disable switch.
        let ceiling: PluginQuotaCeiling = serde_json::from_str(
            r#"{"max_fuel":18446744073709551615,"max_memory_pages":4294967295}"#,
        )
        .expect("ceiling deserializes");

        let grant = ceiling.grant(&greedy_declaration());
        assert_eq!(
            grant.granted.max_fuel,
            PluginQuotaCeiling::HARD_MAX.max_fuel
        );
        assert_eq!(
            grant.granted.max_memory_pages,
            PluginQuotaCeiling::HARD_MAX.max_memory_pages
        );
    }

    #[test]
    fn a_modest_declaration_is_granted_as_declared_and_reports_no_clamp() {
        // Narrowing itself is always allowed: the ceiling is a maximum, not a
        // target, so a plugin that asks for less keeps less.
        let declared = legion_protocol::PluginQuotaDeclaration {
            max_fuel: 1_000,
            max_wall_time_ms: 50,
            max_memory_pages: 8,
            max_storage_bytes: 4_096,
            max_host_calls: 4,
            max_events: 4,
            max_output_bytes: 512,
        };
        let grant = PluginQuotaCeiling::default().grant(&declared);
        assert_eq!(grant.granted, declared);
        assert!(!grant.was_clamped());
    }

    #[test]
    fn a_zero_quota_is_zero_and_never_means_unlimited() {
        // A sentinel that read 0 as "unbounded" would be a per-plugin quota
        // disable spelled differently.
        let declared = legion_protocol::PluginQuotaDeclaration {
            max_fuel: 0,
            max_wall_time_ms: 0,
            max_memory_pages: 0,
            max_storage_bytes: 0,
            max_host_calls: 0,
            max_events: 0,
            max_output_bytes: 0,
        };
        let grant = PluginQuotaCeiling::default().grant(&declared);
        assert_eq!(grant.granted, declared);
        assert!(!grant.was_clamped());
    }

    #[test]
    fn the_hard_max_is_itself_bounded() {
        // Guards against a future edit that "raises the cap a little" into
        // uselessness. These are absolute sanity bounds on the sandbox, and
        // they are compile-time: raising HARD_MAX past them fails the build
        // rather than waiting for someone to run the tests.
        const {
            assert!(
                PluginQuotaCeiling::HARD_MAX.max_memory_pages <= 1024,
                "over 64 MiB of guest memory is not a sandbox"
            );
            assert!(PluginQuotaCeiling::HARD_MAX.max_fuel <= 1_000_000_000);
            assert!(PluginQuotaCeiling::HARD_MAX.max_wall_time_ms <= 30_000);
        }
        let default = PluginQuotaCeiling::default();
        assert!(default.max_fuel <= PluginQuotaCeiling::HARD_MAX.max_fuel);
        assert!(default.max_memory_pages <= PluginQuotaCeiling::HARD_MAX.max_memory_pages);
    }
}
