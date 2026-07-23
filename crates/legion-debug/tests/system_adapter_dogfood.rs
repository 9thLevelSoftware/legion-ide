//! B9: optional handshake against a **system** DAP adapter (lldb-dap / codelldb).
//!
//! Default CI: soft-skip when no system adapter is present **or** when a PATH
//! hit exists but spawn/handshake fails (some runners ship a non-functional
//! `lldb-dap.exe` without full LLDB runtime).
//!
//! Intentional dogfood: `LEGION_DAP_DOGFOOD=1` fails closed if the adapter is
//! missing **or** the initialize handshake does not complete.
//!
//! Scope is **initialize + disconnect**. Full launch/step against a host
//! debugee is covered by `system_adapter_launch_step_dogfood` (B13).

use std::time::Duration;

use legion_debug::{LiveDapSession, dogfood_requires_system_adapter, resolve_system_adapter};

#[test]
fn system_adapter_initialize_handshake_dogfood() {
    let require = dogfood_requires_system_adapter();

    let Some(adapter) = resolve_system_adapter("lldb-dap") else {
        if require {
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
        "system DAP dogfood: program={} type={} require={}",
        adapter.program.display(),
        adapter.adapter_type,
        require
    );

    let mut session = match LiveDapSession::spawn(
        &adapter.program,
        &adapter.args,
        adapter.adapter_type.clone(),
    ) {
        Ok(session) => session,
        Err(err) => {
            if require {
                panic!(
                    "spawn system adapter {} failed: {err}",
                    adapter.program.display()
                );
            }
            eprintln!(
                "skip: spawn failed for {} ({err}); set LEGION_DAP_DOGFOOD=1 to fail closed",
                adapter.program.display()
            );
            return;
        }
    };

    let outcome = match session.initialize_handshake(Duration::from_secs(10)) {
        Ok(outcome) => outcome,
        Err(err) => {
            let _ = session.disconnect_and_wait(Duration::from_secs(2));
            if require {
                panic!(
                    "initialize handshake failed against {}: {err}",
                    adapter.program.display()
                );
            }
            eprintln!(
                "skip: initialize failed for {} ({err}); set LEGION_DAP_DOGFOOD=1 to fail closed",
                adapter.program.display()
            );
            return;
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
