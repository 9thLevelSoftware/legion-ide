use legion_sandbox::{
    SandboxScope, landlock::LandlockProfile, seatbelt::SeatbeltProfile, windows::WindowsProfile,
};

#[test]
fn seatbelt_profile_mentions_kernel_enforced_network_rules() {
    let profile = SeatbeltProfile::compile(SandboxScope::workspace_only("/tmp/legion-workspace"));

    assert!(
        profile
            .rules
            .iter()
            .any(|rule| rule.contains("network-outbound"))
    );
    assert_eq!(
        profile.profile.backend,
        legion_sandbox::SandboxBackend::Seatbelt
    );
}

#[test]
fn landlock_profile_mentions_unshared_network_and_workspace_scope() {
    let profile = LandlockProfile::compile(SandboxScope::workspace_only("/tmp/legion-workspace"));

    assert!(
        profile
            .notes
            .iter()
            .any(|note| note.contains("bwrap --unshare-net"))
    );
    assert_eq!(
        profile.profile.backend,
        legion_sandbox::SandboxBackend::BubblewrapLandlock
    );
}

#[test]
fn windows_profile_exposes_an_explicit_fallback_message() {
    let profile = WindowsProfile::compile(SandboxScope::workspace_only("/tmp/legion-workspace"))
        .expect("windows profile compiles");

    assert!(
        profile
            .documented_fallback
            .as_ref()
            .expect("fallback note")
            .contains("weaker guarantees")
    );
    assert!(WindowsProfile::fallback_message().contains("never silently becomes no sandbox"));
    if cfg!(windows) {
        assert_eq!(
            profile.profile.backend,
            legion_sandbox::SandboxBackend::RestrictedToken
        );
    } else {
        match &profile.profile.backend {
            legion_sandbox::SandboxBackend::DocumentedFallback { reason } => {
                assert!(reason.contains("unavailable"));
            }
            other => {
                panic!("expected documented fallback backend on non-Windows hosts, got {other:?}")
            }
        }
    }
}
