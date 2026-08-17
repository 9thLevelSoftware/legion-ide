//! Policy gate and audit trail for git operations that contact a remote (P2.F5.T4).
//!
//! Local git verbs are gated by workspace trust at the point of use. Remote
//! verbs need more: they can leave the machine, so the user has to be able to
//! see *why* an operation was permitted or refused. This module owns that
//! decision-to-projection step so `lib.rs` only has to record the outcome.
//!
//! The design constraint that shapes this module is "no network operation
//! without an audit row". `evaluate` is the only way to obtain permission, and
//! it always returns a row alongside the verdict, so a caller cannot take the
//! allow path while skipping the record.

use legion_security::{
    GitRemoteDecision, GitRemoteOperation, GitRemoteTarget, SecurityPolicy, TrustState,
    decide_git_remote_operation,
};
use legion_ui::GitRemotePolicyProjection;

/// Maximum audit rows retained in the projection.
///
/// The rows are a visible trail, not a persistent log, so the projection keeps
/// only the recent tail. Older rows are dropped from the front.
pub const MAX_REMOTE_POLICY_AUDIT_ROWS: usize = 16;

/// Outcome of evaluating one git remote operation.
///
/// Holds both halves that the caller needs: whether to proceed, and the row to
/// record. They are returned together so they cannot drift apart.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitRemoteGateOutcome {
    /// Whether the operation may run.
    pub allowed: bool,
    /// Projection row describing the decision, for the SCM surface.
    pub audit: GitRemotePolicyProjection,
}

/// Evaluate a git remote operation against the workspace security policy.
///
/// `trust` is the workspace trust state; `remote_url` is the configured URL for
/// `remote_name`, or `None` when the remote has none. A remote with no URL is
/// denied rather than assumed local — policy cannot evaluate a target it cannot
/// see.
pub fn evaluate(
    policy: &SecurityPolicy,
    trust: TrustState,
    operation: GitRemoteOperation,
    remote_name: &str,
    remote_url: Option<&str>,
) -> GitRemoteGateOutcome {
    let decision = decide_git_remote_operation(policy, trust, operation, remote_name, remote_url);
    GitRemoteGateOutcome {
        allowed: decision.is_allowed(),
        audit: projection_row(&decision),
    }
}

/// Convert a security decision into its display-safe projection row.
fn projection_row(decision: &GitRemoteDecision) -> GitRemotePolicyProjection {
    GitRemotePolicyProjection {
        operation: decision.operation.label().to_string(),
        remote: decision.remote_name.clone(),
        target: decision.target.label(),
        host: match &decision.target {
            GitRemoteTarget::Host { host, .. } if !host.is_empty() => Some(host.clone()),
            _ => None,
        },
        allowed: decision.is_allowed(),
        detail: decision.audit_row(),
    }
}

/// Build the audit row for a consent grant or withdrawal.
///
/// Consent is itself a policy event, so it joins the same visible trail as the
/// operations it governs: the user sees the grant and the subsequent allow as
/// consecutive rows rather than an unexplained change of verdict.
pub fn consent_row(host: &str, granted: bool, changed: bool) -> GitRemotePolicyProjection {
    let action = if granted { "grant" } else { "revoke" };
    let effect = match (granted, changed) {
        (true, true) => "consent recorded",
        (true, false) => "consent already recorded",
        (false, true) => "consent withdrawn",
        (false, false) => "no consent was recorded",
    };
    GitRemotePolicyProjection {
        operation: format!("consent-{action}"),
        remote: String::new(),
        target: host.to_string(),
        host: (!host.is_empty()).then(|| host.to_string()),
        // A grant is an allow-shaped event; a revoke removes permission.
        allowed: granted && changed,
        detail: format!("git consent {action} host={host} result={effect}"),
    }
}

/// Append an audit row, trimming the oldest rows past the retention bound.
pub fn record(audit: &mut Vec<GitRemotePolicyProjection>, row: GitRemotePolicyProjection) {
    audit.push(row);
    if audit.len() > MAX_REMOTE_POLICY_AUDIT_ROWS {
        let excess = audit.len() - MAX_REMOTE_POLICY_AUDIT_ROWS;
        audit.drain(..excess);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use legion_security::NetworkPolicy;

    fn allowing(host: &str) -> SecurityPolicy {
        SecurityPolicy {
            network_policy: NetworkPolicy {
                air_gap: false,
                allowlist: vec![host.to_string()],
                ..NetworkPolicy::default()
            },
            ..SecurityPolicy::default()
        }
    }

    #[test]
    fn a_denied_operation_still_produces_a_projection_row() {
        let outcome = evaluate(
            &SecurityPolicy::default(),
            TrustState::Trusted,
            GitRemoteOperation::Push,
            "origin",
            Some("git@github.com:legion/example.git"),
        );

        assert!(!outcome.allowed);
        assert!(!outcome.audit.allowed);
        assert_eq!(outcome.audit.operation, "push");
        assert_eq!(outcome.audit.remote, "origin");
        assert_eq!(outcome.audit.target, "ssh://github.com");
        assert!(outcome.audit.detail.contains("air-gap"));
    }

    #[test]
    fn an_allowed_operation_also_produces_a_projection_row() {
        let outcome = evaluate(
            &allowing("github.com"),
            TrustState::Trusted,
            GitRemoteOperation::Fetch,
            "origin",
            Some("https://github.com/legion/example.git"),
        );

        assert!(outcome.allowed);
        assert!(outcome.audit.allowed);
        assert_eq!(outcome.audit.operation, "fetch");
        assert!(outcome.audit.detail.contains("decision=allow"));
    }

    #[test]
    fn audit_rows_are_bounded_and_drop_the_oldest_first() {
        let mut audit = Vec::new();
        for index in 0..(MAX_REMOTE_POLICY_AUDIT_ROWS + 3) {
            record(
                &mut audit,
                GitRemotePolicyProjection {
                    operation: "push".to_string(),
                    remote: format!("remote-{index}"),
                    target: "local-path".to_string(),
                    host: None,
                    allowed: true,
                    detail: String::new(),
                },
            );
        }

        assert_eq!(audit.len(), MAX_REMOTE_POLICY_AUDIT_ROWS);
        // The three oldest rows were dropped, not the newest.
        assert_eq!(audit.first().expect("non-empty").remote, "remote-3");
        assert_eq!(
            audit.last().expect("non-empty").remote,
            format!("remote-{}", MAX_REMOTE_POLICY_AUDIT_ROWS + 2)
        );
    }
}
