//! P2.F5.T4 — every git operation that reaches a remote produces a policy
//! decision and an audit row.
//!
//! The acceptance for this surface is not "push works"; it is that the user can
//! always see the policy verdict. These tests therefore assert the audit row on
//! both the allow and the deny side, and pair each allow with the deny that
//! proves the check is actually running.

use legion_security::{
    CommandClass, GitRemoteOperation, GitRemoteTarget, NetworkPolicy, SecurityDecision,
    SecurityPolicy, TrustState, classify_git_remote_url, decide_git_remote_operation,
};

/// A policy that permits egress to exactly the supplied hosts.
fn policy_allowing(hosts: &[&str]) -> SecurityPolicy {
    SecurityPolicy {
        network_policy: NetworkPolicy {
            air_gap: false,
            allowlist: hosts.iter().map(|host| (*host).to_string()).collect(),
            ..NetworkPolicy::default()
        },
        ..SecurityPolicy::default()
    }
}

#[test]
fn remote_urls_are_classified_into_local_and_host_targets() {
    // Anything that can leave the machine must classify as a host.
    assert_eq!(
        classify_git_remote_url("https://github.com/legion/example.git"),
        GitRemoteTarget::Host {
            host: "github.com".to_string(),
            scheme: "https".to_string(),
        }
    );
    // scp-style SSH remotes carry no scheme and must not be mistaken for paths.
    assert_eq!(
        classify_git_remote_url("git@github.com:legion/example.git"),
        GitRemoteTarget::Host {
            host: "github.com".to_string(),
            scheme: "ssh".to_string(),
        }
    );
    // Userinfo and ports are stripped before the host is matched.
    assert_eq!(
        classify_git_remote_url("ssh://git@Example.COM:2222/legion/example.git"),
        GitRemoteTarget::Host {
            host: "example.com".to_string(),
            scheme: "ssh".to_string(),
        }
    );
    // IPv6 literals keep their inner colons.
    assert_eq!(
        classify_git_remote_url("ssh://[::1]:22/srv/repo.git"),
        GitRemoteTarget::Host {
            host: "::1".to_string(),
            scheme: "ssh".to_string(),
        }
    );

    // Negative side: none of these can egress, so none may be classified as a host.
    for local in [
        "/srv/mirror.git",
        "../sibling-repo",
        "file:///srv/mirror.git",
        r"C:\repos\mirror.git",
        "C:/repos/mirror.git",
        "",
    ] {
        assert_eq!(
            classify_git_remote_url(local),
            GitRemoteTarget::Local,
            "`{local}` must classify as a local target"
        );
    }
}

#[test]
fn air_gap_denies_a_non_loopback_push_and_says_so_in_the_audit_row() {
    let decision = decide_git_remote_operation(
        &SecurityPolicy::default(),
        TrustState::Trusted,
        GitRemoteOperation::Push,
        "origin",
        Some("git@github.com:legion/example.git"),
    );

    assert!(!decision.is_allowed());
    assert_eq!(decision.command_class, CommandClass::Network);
    let row = decision.audit_row();
    assert!(
        row.contains("decision=deny") && row.contains("air-gap"),
        "audit row must name the air-gap denial; got: {row}"
    );
    // The row is metadata only — the repository path from the URL must not leak.
    assert!(
        !row.contains("legion/example"),
        "audit row must not carry the remote path; got: {row}"
    );
}

#[test]
fn allowlisted_host_is_permitted_and_still_emits_an_audit_row() {
    let decision = decide_git_remote_operation(
        &policy_allowing(&["github.com"]),
        TrustState::Trusted,
        GitRemoteOperation::Push,
        "origin",
        Some("git@github.com:legion/example.git"),
    );

    assert!(decision.is_allowed());
    assert_eq!(decision.decision, SecurityDecision::Allow);
    assert!(
        decision.audit_row().contains("decision=allow"),
        "an allowed operation must still produce an audit row"
    );
}

#[test]
fn a_host_outside_the_allowlist_is_denied_even_without_air_gap() {
    // Same policy as the allow case above, different host: this is what proves
    // the allowlist is consulted rather than the air-gap flag alone.
    let decision = decide_git_remote_operation(
        &policy_allowing(&["github.com"]),
        TrustState::Trusted,
        GitRemoteOperation::Push,
        "origin",
        Some("https://gitlab.example.test/legion/example.git"),
    );

    assert!(!decision.is_allowed());
    assert!(
        decision.audit_row().contains("not allowlisted"),
        "got: {}",
        decision.audit_row()
    );
}

#[test]
fn blocklisted_host_is_denied_even_when_it_is_also_allowlisted() {
    let mut policy = policy_allowing(&["blocked.example.test"]);
    policy.network_policy.blocklist = vec!["blocked.example.test".to_string()];

    let decision = decide_git_remote_operation(
        &policy,
        TrustState::Trusted,
        GitRemoteOperation::Fetch,
        "origin",
        Some("https://blocked.example.test/legion/example.git"),
    );

    assert!(!decision.is_allowed());
    assert!(
        decision.audit_row().contains("blocked by network policy"),
        "got: {}",
        decision.audit_row()
    );
}

#[test]
fn untrusted_workspace_denies_every_remote_operation() {
    for trust in [TrustState::Untrusted, TrustState::Unknown] {
        for operation in [
            GitRemoteOperation::Fetch,
            GitRemoteOperation::Pull,
            GitRemoteOperation::Push,
        ] {
            let decision = decide_git_remote_operation(
                &policy_allowing(&["github.com"]),
                trust,
                operation,
                "origin",
                Some("git@github.com:legion/example.git"),
            );
            assert!(
                !decision.is_allowed(),
                "{operation:?} must be denied under {trust:?}"
            );
            assert!(
                decision.audit_row().contains("trusted workspace"),
                "got: {}",
                decision.audit_row()
            );
        }
    }
}

#[test]
fn filesystem_remotes_are_allowed_because_they_cannot_egress() {
    // A bare-repo path remote is the offline mirror workflow. Air-gap mode is on
    // (the default) and the allowlist does not mention it, yet it must still be
    // allowed — nothing leaves the machine.
    let decision = decide_git_remote_operation(
        &SecurityPolicy::default(),
        TrustState::Trusted,
        GitRemoteOperation::Push,
        "origin",
        Some("/srv/mirrors/example.git"),
    );

    assert!(decision.is_allowed());
    assert_eq!(decision.target, GitRemoteTarget::Local);
    assert!(decision.audit_row().contains("target=local-path"));
}

#[test]
fn a_remote_without_a_configured_url_is_denied_rather_than_assumed_local() {
    let decision = decide_git_remote_operation(
        &policy_allowing(&["github.com"]),
        TrustState::Trusted,
        GitRemoteOperation::Pull,
        "origin",
        None,
    );

    assert!(!decision.is_allowed());
    assert!(
        decision.audit_row().contains("no configured URL"),
        "got: {}",
        decision.audit_row()
    );
}

#[test]
fn reclassifying_git_push_away_from_network_denies_instead_of_bypassing() {
    // Guards the layering: if an operator's taxonomy override stops classifying
    // `git push` as Network, the operation must fail closed rather than fall
    // through to the unchecked path.
    let mut policy = policy_allowing(&["github.com"]);
    policy
        .command_taxonomy
        .by_name
        .insert("git push".to_string(), CommandClass::Read);

    let decision = decide_git_remote_operation(
        &policy,
        TrustState::Trusted,
        GitRemoteOperation::Push,
        "origin",
        Some("git@github.com:legion/example.git"),
    );

    assert!(!decision.is_allowed());
    assert_eq!(decision.command_class, CommandClass::Read);
    assert!(
        decision.audit_row().contains("not Network"),
        "got: {}",
        decision.audit_row()
    );
}

#[test]
fn only_push_publishes_local_content() {
    assert!(GitRemoteOperation::Push.publishes_local_content());
    assert!(!GitRemoteOperation::Fetch.publishes_local_content());
    assert!(!GitRemoteOperation::Pull.publishes_local_content());
}
