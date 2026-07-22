//! B9: optional handshake against a **system** DAP adapter (lldb-dap / codelldb).
//!
//! Default CI: skips when no system adapter is on `PATH` / `LEGION_DAP_ADAPTER`.
//! Intentional dogfood: `LEGION_DAP_DOGFOOD=1` fails closed if none is found.
//!
//! Scope is intentionally **initialize + disconnect** only. Full launch/step
//! against a real debugee needs a host-specific binary and remains interactive
//! residual (GUI dogfood).

use std::time::Duration;

use legion_debug::{LiveDapSession, dogfood_requires_system_adapter, resolve_system_adapter};

#[test]
fn system_adapter_initialize_handshake_dogfood() {
    let Some(adapter) = resolve_system_adapter("lldb-dap") else {
        if dogfood_requires_system_adapter() {
            panic!(
                "LEGION_DAP_DOGFOOD=1 requires a system adapter \
                 (set LEGION_DAP_ADAPTER or install lldb-dap/codelldb on PATH)"
            );
        }
        eprintln!(
            "skip system_adapter_initialize_handshake_dogfood: no system adapter \
             (LEGION_DAP_ADAPTER / PATH lldb-dap|lldb-vscode|codelldb); \
             set LEGION_DAP_DOGFOOD=1 to fail closed"
        );
        return;
    };

    assert!(
        !adapter.is_fake,
        "dogfood must not use in-tree fake: {:?}",
        adapter.program
    );

    eprintln!(
        "system DAP dogfood: program={} type={}",
        adapter.program.display(),
        adapter.adapter_type
    );

    let mut session = LiveDapSession::spawn(
        &adapter.program,
        &adapter.args,
        adapter.adapter_type.clone(),
    )
    .unwrap_or_else(|err| {
        panic!(
            "spawn system adapter {} failed: {err}",
            adapter.program.display()
        )
    });

    let outcome = match session.initialize_handshake(Duration::from_secs(10)) {
        Ok(outcome) => outcome,
        Err(err) => {
            let _ = session.disconnect_and_wait(Duration::from_secs(2));
            panic!(
                "initialize handshake failed against {}: {err}",
                adapter.program.display()
            );
        }
    };

    assert!(
        outcome.initialized_event,
        "system adapter must emit initialized event: {}",
        outcome.metadata_summary
    );
    assert!(
        outcome.metadata_summary.contains("live=true"),
        "expected live summary: {}",
        outcome.metadata_summary
    );
    assert!(
        outcome.metadata_summary.contains("wire=microsoft-dap"),
        "expected microsoft-dap wire marker: {}",
        outcome.metadata_summary
    );

    session
        .disconnect_and_wait(Duration::from_secs(3))
        .expect("disconnect after dogfood handshake");
}
