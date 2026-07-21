//! B1: live DAP session against in-tree fake adapter.

use std::time::Duration;

use legion_debug::{DapLifecycleState, LiveDapSession, fake_dap_adapter_path};

#[test]
fn live_dap_initialize_handshake_against_fake_adapter() {
    let adapter = fake_dap_adapter_path().unwrap_or_else(|| {
        // Ensure the bin is built when running via `cargo test -p legion-debug`.
        panic!(
            "fake_dap_adapter binary not found; run `cargo build -p legion-debug --bin fake_dap_adapter` first"
        );
    });

    let mut session =
        LiveDapSession::spawn(&adapter, &[], "legion-fake").expect("spawn fake adapter");
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
