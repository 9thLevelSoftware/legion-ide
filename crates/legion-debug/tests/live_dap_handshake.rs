//! B1/B2: live DAP session against in-tree fake adapter.

use std::time::Duration;

use legion_debug::{DapLifecycleState, LiveDapSession, fake_dap_adapter_path};

fn adapter_path() -> std::path::PathBuf {
    fake_dap_adapter_path().unwrap_or_else(|| {
        panic!(
            "fake_dap_adapter binary not found; run `cargo build -p legion-debug --bin fake_dap_adapter` first"
        );
    })
}

#[test]
fn live_dap_initialize_handshake_against_fake_adapter() {
    let mut session =
        LiveDapSession::spawn(adapter_path(), &[], "legion-fake").expect("spawn fake adapter");
    let outcome = session
        .initialize_handshake(Duration::from_secs(5))
        .expect("initialize handshake");

    assert!(outcome.initialized_event);
    assert_eq!(outcome.adapter_type, "legion-fake");
    assert_eq!(outcome.lifecycle_state, DapLifecycleState::Launching);
    assert!(outcome.metadata_summary.contains("live=true"));
    assert!(outcome.metadata_summary.contains("initialized=true"));

    session
        .disconnect_and_wait(Duration::from_secs(2))
        .expect("disconnect");
}

#[test]
fn live_dap_breakpoints_launch_stack_step_against_fake_adapter() {
    let mut session =
        LiveDapSession::spawn(adapter_path(), &[], "legion-fake").expect("spawn fake adapter");
    session
        .initialize_handshake(Duration::from_secs(5))
        .expect("initialize");

    let bps = session
        .set_breakpoints("src/main.rs", &[10, 20], Duration::from_secs(2))
        .expect("setBreakpoints");
    assert_eq!(bps.len(), 2);
    assert!(bps.iter().all(|bp| bp.verified));
    assert_eq!(bps[0].line, 10);

    let stop = session
        .launch_until_stopped("/tmp/fake-program", Duration::from_secs(3))
        .expect("launch until stopped");
    assert_eq!(stop.lifecycle_state, DapLifecycleState::Paused);
    assert_eq!(stop.reason, "entry");
    assert_eq!(stop.thread_id, 1);
    assert!(
        stop.stack_frames.iter().any(|f| f.name == "main"),
        "expected main frame: {:?}",
        stop.stack_frames
    );
    assert!(
        stop.variables
            .iter()
            .any(|v| v.name == "count" && v.value == "42"),
        "expected locals: {:?}",
        stop.variables
    );
    assert!(stop.metadata_summary.contains("live=true"));

    let stepped = session
        .step_over_until_stopped(stop.thread_id, Duration::from_secs(3))
        .expect("step over");
    assert_eq!(stepped.reason, "step");
    assert!(stepped.stack_frames.iter().any(|f| f.name == "main"));

    let cont = session
        .continue_until_stopped(stepped.thread_id, Duration::from_secs(3))
        .expect("continue until stopped");
    assert_eq!(cont.reason, "breakpoint");
    assert!(cont.stack_frames.iter().any(|f| f.name == "main"));

    session
        .disconnect_and_wait(Duration::from_secs(2))
        .expect("disconnect");
}
