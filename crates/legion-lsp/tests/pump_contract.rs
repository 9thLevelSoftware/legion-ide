use std::time::{Duration, Instant};

use legion_lsp::{LspStdioSession, PumpOutcome};

mod common; // reuse mock spawn helper pattern (copy from stdio_transport_contract.rs)

/// Regression for the blocking-read bug (Finding 1): `pump_until` previously
/// called `read_envelope()` which blocks indefinitely when the server is alive
/// but silent. This test verifies that `pump_until` returns `PumpOutcome::Deadline`
/// within ~2x the deadline even when the mock server produces no notifications.
///
/// Before the fix: this test hangs (blocks on the blocking `read_envelope` call).
/// After the fix: `read_envelope_until(deadline)` times out and the pump returns
/// `Deadline` well within the allowed window.
#[test]
fn pump_deadline_when_server_is_alive_but_silent() {
    let mut launcher = legion_lsp::LspStdioLauncher::new();
    // Use the plain mock (no MOCK_LSP_EMIT_DIAGNOSTICS) so the server emits
    // nothing after the initialize exchange.
    let mut session = LspStdioSession::start(common::mock_server_config(), &mut launcher).unwrap();

    // Complete the initialize handshake so the server enters its request loop.
    // After this it is alive but will not emit any notifications on its own.
    session
        .initialize(serde_json::json!({}), common::ctx())
        .unwrap();

    let deadline_ms: u64 = 300;
    let deadline = Instant::now() + Duration::from_millis(deadline_ms);
    let start = Instant::now();
    // Predicate always returns false: the pump must return Deadline, not hang.
    let outcome = session.pump_until(deadline, &mut |_| false).unwrap();
    let elapsed = start.elapsed();

    assert_eq!(
        outcome,
        PumpOutcome::Deadline,
        "pump_until must return Deadline when server is silent; got {outcome:?}"
    );
    // Must return within 2× the deadline. A generous multiplier accounts for
    // slow CI; the old blocking read would take minutes, not milliseconds.
    assert!(
        elapsed <= Duration::from_millis(deadline_ms * 2),
        "pump_until took {elapsed:?} — must return within {}ms",
        deadline_ms * 2
    );
}

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

    assert!(matches!(
        outcome,
        PumpOutcome::PredicateMet | PumpOutcome::Closed
    ));
    // The pumped notification must also land in the session's durable buffer.
    assert!(!session.diagnostic_notifications().is_empty());

    // Verify the session still works normally after a pump.
    session
        .initialize(serde_json::json!({}), common::ctx())
        .unwrap();
}
