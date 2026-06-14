use legion_sandbox::{ActivatedSandbox, SandboxBackend, SandboxPlatform, SandboxScope};

#[test]
fn outside_scope_write_is_denied_and_audited() {
    let scope = SandboxScope::workspace_only("/tmp/legion-workspace");
    let mut sandbox = ActivatedSandbox::activate(
        SandboxPlatform::Linux,
        SandboxBackend::BubblewrapLandlock,
        scope,
    );

    let decision = sandbox.authorize_write("/etc/shadow");

    assert!(!decision.allowed);
    assert!(decision.audit.reason.contains("outside workspace scope"));
    assert_eq!(sandbox.audit_log().len(), 2);
}

#[test]
fn unapproved_egress_is_denied_and_audited() {
    let scope = SandboxScope::workspace_only("/tmp/legion-workspace");
    let mut sandbox = ActivatedSandbox::activate(
        SandboxPlatform::MacOS,
        SandboxBackend::Seatbelt,
        scope,
    );

    let decision = sandbox.authorize_egress("https://example.exfiltration.invalid");

    assert!(!decision.allowed);
    assert!(decision.audit.reason.contains("raw egress denied"));
    assert_eq!(sandbox.audit_log().len(), 2);
}

#[test]
fn allowlisted_egress_matches_canonical_host_and_is_audited() {
    let scope = SandboxScope::workspace_only("/tmp/legion-workspace").with_egress("localhost");
    let mut sandbox = ActivatedSandbox::activate(
        SandboxPlatform::Linux,
        SandboxBackend::BubblewrapLandlock,
        scope,
    );

    let decision = sandbox.authorize_egress("https://localhost:8080/v1/status");

    assert!(decision.allowed);
    assert!(decision.audit.allowed);
    assert!(decision.audit.reason.contains("allowlisted"));
    assert_eq!(sandbox.audit_log().len(), 2);
}
