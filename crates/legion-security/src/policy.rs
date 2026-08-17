//! Deterministic approval-policy helpers for proposal auto-approval and apply gating.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

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

/// Whether a host is a loopback address that never leaves the machine.
fn is_loopback_host(host: &str) -> bool {
    matches!(
        host.to_ascii_lowercase().as_str(),
        "localhost" | "127.0.0.1" | "::1"
    )
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
/// 5. Host remotes are checked against the blocklist, then air-gap, then the
///    allowlist.
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
    fn unknown_or_blank_rule_ids_are_rejected() {
        let policy = ProposalAutoApprovalPolicy {
            enabled: true,
            allowed_rule_ids: vec!["rule-a".to_string()],
        };
        assert!(!policy.allows_rule_ids(&["rule-b".to_string()]));
        assert!(!policy.allows_rule_ids(&[String::new()]));
        assert!(policy.allows_rule_ids(&["rule-a".to_string()]));
    }
}
