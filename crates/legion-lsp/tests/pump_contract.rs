use std::time::{Duration, Instant};

use legion_lsp::{LspStdioSession, PumpOutcome};

mod common; // reuse mock spawn helper pattern (copy from stdio_transport_contract.rs)

/// Verify that `pump_until` collects asynchronous diagnostic notifications and fires
/// the caller predicate once the condition is met.
///
/// The mock server emits one `textDocument/publishDiagnostics` notification at
/// startup (before answering requests). Calling `pump_until` before `initialize`
/// drains that notification so the pump – not `read_until_correlated_response` –
/// is the code path that collects it. This proves the pump accumulates diagnostics
/// and surfaces them to the predicate in real time.
#[test]
fn pump_collects_async_diagnostics_until_predicate() {
    let mut launcher = legion_lsp::LspStdioLauncher::new();
    let mut session =
        LspStdioSession::start(common::mock_server_config_with_diagnostics(), &mut launcher)
            .unwrap();

    // The mock emits one publishDiagnostics notification at startup.
    // Pump first so the pump – not initialize – is the consumer of that notification.
    let deadline = Instant::now() + Duration::from_secs(5);
    let mut seen = false;
    let outcome = session
        .pump_until(deadline, &mut |n| {
            if !n.diagnostics.is_empty() {
                seen = true;
            }
            seen
        })
        .unwrap();

    assert!(matches!(outcome, PumpOutcome::PredicateMet | PumpOutcome::Closed));
    // The pumped notification must also land in the session's durable buffer.
    assert!(!session.diagnostic_notifications().is_empty());

    // Verify the session still works normally after a pump.
    session
        .initialize(serde_json::json!({}), common::ctx())
        .unwrap();
}
