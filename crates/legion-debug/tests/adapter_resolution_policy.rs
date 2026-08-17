//! P2.F3.T2: adapter binary resolution is policy-gated end to end.
//!
//! `LEGION_DAP_ADAPTER` is the highest-priority resolution source, so it is the
//! sharpest test of the gate: if the allowlist can veto an explicitly configured
//! path, it can veto the lower-priority `PATH` hits too.
//!
//! This is a single `#[test]` in its own test binary on purpose — it mutates
//! process env, and sibling tests running in parallel threads would see it.

use std::path::PathBuf;

use legion_debug::{
    AdapterResolutionGrant, DEBUG_ADAPTER_LAUNCH_CAPABILITY, resolve_system_adapter,
};
use legion_protocol::{CapabilityDecision, CapabilityDecisionId, CapabilityId};

fn granted_decision() -> CapabilityDecision {
    CapabilityDecision {
        decision_id: CapabilityDecisionId(11),
        granted: true,
        capability: CapabilityId(DEBUG_ADAPTER_LAUNCH_CAPABILITY.to_string()),
        reason: None,
    }
}

fn grant_allowing(binaries: &[&str]) -> AdapterResolutionGrant {
    let owned: Vec<String> = binaries.iter().map(|name| (*name).to_string()).collect();
    AdapterResolutionGrant::from_decision(&granted_decision(), &owned).expect("grant")
}

#[test]
fn explicit_adapter_path_is_refused_unless_the_binary_is_allowlisted() {
    // A real, existing, definitely-not-a-debug-adapter executable: this test's
    // own binary. Using a path that exists is what makes the negative case
    // meaningful — resolution fails on policy, not on a missing file.
    let this_exe: PathBuf = std::env::current_exe().expect("current exe");
    let stem = this_exe
        .file_stem()
        .and_then(|stem| stem.to_str())
        .expect("test binary stem")
        .to_string();

    // SAFETY: single-test binary; no other thread reads or writes the env here.
    unsafe { std::env::set_var("LEGION_DAP_ADAPTER", &this_exe) };

    let denied = grant_allowing(&["lldb-dap", "codelldb"]);
    assert!(
        resolve_system_adapter(&denied, "lldb-dap").is_none(),
        "LEGION_DAP_ADAPTER must not launch {} — it is not an allowlisted adapter",
        this_exe.display()
    );

    // Positive control: the *only* difference is the allowlist, which proves the
    // refusal above came from policy and not from an unrelated resolution failure.
    let allowed = grant_allowing(&[stem.as_str()]);
    let resolved = resolve_system_adapter(&allowed, "lldb-dap")
        .expect("allowlisted explicit adapter path resolves");
    assert_eq!(resolved.program, this_exe);
    assert!(
        !resolved.is_fake,
        "system resolve must never claim the fake"
    );

    // SAFETY: as above.
    unsafe { std::env::remove_var("LEGION_DAP_ADAPTER") };
}
