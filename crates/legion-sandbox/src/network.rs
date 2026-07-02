//! Network egress policy helpers.

use crate::{
    SandboxAction, SandboxAuditEvent, SandboxBackend, SandboxDecision, SandboxPlatform,
    SandboxScope,
};

/// Canonicalizes a raw egress target to an authority string for policy matching.
fn canonicalize_egress_target(raw: &str) -> Option<String> {
    let mut target = raw.trim();
    if target.is_empty() {
        return None;
    }

    if let Some((_, rest)) = target.split_once("://") {
        target = rest;
    }

    if let Some(authority) = target.split(['/', '?', '#']).next() {
        target = authority;
    }

    if let Some((_, host)) = target.rsplit_once('@') {
        target = host;
    }

    let canonical = target.trim().trim_matches('/');
    if canonical.is_empty() {
        None
    } else {
        Some(canonical.to_string())
    }
}

fn authority_parts(authority: &str) -> (String, Option<String>) {
    let authority = authority.trim();
    if let Some(stripped) = authority.strip_prefix('[')
        && let Some(close) = stripped.find(']')
    {
        let host = format!("[{}]", &stripped[..close]);
        let rest = &stripped[close + 1..];
        if let Some(port) = rest.strip_prefix(':') {
            return (host, Some(port.to_string()));
        }
        return (host, None);
    }

    if let Some((host, port)) = authority.rsplit_once(':')
        && !host.contains(':')
    {
        return (host.to_string(), Some(port.to_string()));
    }

    (authority.to_string(), None)
}

fn allowlist_matches_target(allowlisted: &str, target: &str) -> bool {
    let Some(allowlisted) = canonicalize_egress_target(allowlisted) else {
        return false;
    };
    let Some(target) = canonicalize_egress_target(target) else {
        return false;
    };

    if allowlisted == target {
        return true;
    }

    let (allowlisted_host, allowlisted_port) = authority_parts(&allowlisted);
    let (target_host, target_port) = authority_parts(&target);
    allowlisted_port.is_none() && allowlisted_host == target_host && target_port.is_some()
}

/// Evaluates an egress attempt against the scope allowlist and records the audit decision.
pub fn authorize_egress(
    platform: SandboxPlatform,
    backend: SandboxBackend,
    scope: &SandboxScope,
    audit_log: &mut Vec<SandboxAuditEvent>,
    target: impl Into<String>,
) -> SandboxDecision {
    let target = target.into();
    let action = SandboxAction::Egress {
        target: target.clone(),
    };

    let allowed = scope
        .allowed_egress
        .iter()
        .any(|allowlisted| allowlist_matches_target(allowlisted, &target));

    let decision = if allowed {
        SandboxDecision::allow(
            platform,
            backend,
            action,
            "egress explicitly allowlisted after canonicalization",
        )
    } else {
        SandboxDecision::deny(
            platform,
            backend,
            action,
            "raw egress denied without permission",
        )
    };

    audit_log.push(decision.audit.clone());
    decision
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonicalizes_scheme_authority_and_path_before_matching() {
        assert_eq!(
            canonicalize_egress_target("https://localhost:8080/v1/status"),
            Some("localhost:8080".to_string())
        );
        assert_eq!(
            canonicalize_egress_target("localhost"),
            Some("localhost".to_string())
        );
    }

    #[test]
    fn allowlisted_target_matches_on_canonical_authority() {
        let scope = SandboxScope::workspace_only("/workspace").with_egress("localhost");
        let mut audit_log = Vec::new();

        let decision = authorize_egress(
            SandboxPlatform::Linux,
            SandboxBackend::BubblewrapLandlock,
            &scope,
            &mut audit_log,
            "https://localhost:8080/v1/status",
        );

        assert!(decision.allowed);
        assert_eq!(audit_log.len(), 1);
        assert!(audit_log[0].allowed);
    }
}
