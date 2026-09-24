#[allow(dead_code)]
mod common;

use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

use legion_lsp::{LspApplyWorkspaceEditResponse, LspStdioLauncher, LspStdioSession};
use serde_json::json;

fn configured(mode: Option<&str>, expected_applied: bool) -> legion_lsp::LspSupervisorConfig {
    let mut config = common::mock_server_config();
    config.process.env.push((
        "MOCK_APPLY_EDIT_EXPECT_APPLIED".to_string(),
        if expected_applied { "1" } else { "0" }.to_string(),
    ));
    config
        .process
        .env
        .push(("MOCK_APPLY_EDIT_TRIGGER".to_string(), "1".to_string()));
    if let Some(mode) = mode {
        config
            .process
            .env
            .push(("MOCK_APPLY_EDIT_PARAMS".to_string(), mode.to_string()));
    }
    config
}

fn initialized(
    config: legion_lsp::LspSupervisorConfig,
    request_number: u128,
) -> (LspStdioSession, legion_protocol::LspOperationContext) {
    let mut launcher = LspStdioLauncher::new();
    let mut session = LspStdioSession::start(config, &mut launcher).expect("start mock server");
    session
        .initialize(
            json!({"processId": null, "capabilities": {}}),
            common::ctx(),
        )
        .expect("initialize mock server");
    let mut context = common::ctx();
    context.request_id = legion_protocol::LspRequestId(uuid::Uuid::from_u128(request_number));
    context.correlation_id = legion_protocol::CorrelationId(request_number as u64);
    (session, context)
}

#[test]
fn apply_edit_handler_receives_exact_rpc_id_params_and_active_context() {
    let (mut session, context) = initialized(configured(None, true), 801);
    let expected_context = context.clone();
    session.set_apply_edit_handler(move |request| {
        assert_eq!(request.json_rpc_id, 9100);
        assert_eq!(
            serde_json::to_value(request.context).expect("serialize callback context"),
            serde_json::to_value(Some(expected_context.clone()))
                .expect("serialize expected context")
        );
        assert_eq!(request.params["edit"], json!({"changes": {}}));
        LspApplyWorkspaceEditResponse {
            applied: true,
            failure_reason: None,
        }
    });

    let response = session
        .request("mock.applyEditThenRespond", json!({}), context)
        .expect("request should receive final response after applyEdit ack");
    assert!(response.result.is_null());
}

#[test]
fn apply_edit_without_handler_returns_wire_negative_without_callback() {
    let (mut session, context) = initialized(configured(None, false), 802);
    let response = session
        .request("mock.applyEditThenRespond", json!({}), context)
        .expect("request should receive final response after negative ack");
    assert!(response.result.is_null());
}

#[test]
fn apply_edit_without_active_context_still_invokes_installed_handler() {
    let (mut session, _context) = initialized(configured(None, false), 803);
    let callback_count = Arc::new(AtomicUsize::new(0));
    let callback_count_for_handler = Arc::clone(&callback_count);
    session.set_apply_edit_handler(move |request| {
        assert!(request.context.is_none());
        callback_count_for_handler.fetch_add(1, Ordering::SeqCst);
        LspApplyWorkspaceEditResponse {
            applied: false,
            failure_reason: Some("preview required".to_string()),
        }
    });
    session
        .send_notification("mock.applyEditThenRespond", json!({}))
        .expect("trigger notification");
    let outcome = session
        .pump_until(
            std::time::Instant::now() + std::time::Duration::from_secs(5),
            &mut |notifications| {
                notifications.diagnostics.iter().any(|diagnostic| {
                    diagnostic.uri_hash
                        == legion_lsp::lsp_diagnostic_uri_fingerprint(
                            "file:///workspace/src/apply-edit.rs",
                        )
                })
            },
        )
        .expect("pump should receive post-ack diagnostic");
    assert_eq!(outcome, legion_lsp::PumpOutcome::PredicateMet);
    assert_eq!(callback_count.load(Ordering::SeqCst), 1);
}

#[test]
fn malformed_missing_edit_and_oversized_apply_edit_never_invoke_handler() {
    for (index, mode) in [
        "malformed-edit",
        "missing-edit",
        "missing-params",
        "oversized",
    ]
    .into_iter()
    .enumerate()
    {
        let (mut session, context) =
            initialized(configured(Some(mode), false), 810 + index as u128);
        let callback_count = Arc::new(AtomicUsize::new(0));
        let callback_count_for_handler = Arc::clone(&callback_count);
        session.set_apply_edit_handler(move |_request| {
            callback_count_for_handler.fetch_add(1, Ordering::SeqCst);
            LspApplyWorkspaceEditResponse {
                applied: true,
                failure_reason: None,
            }
        });
        let response = session
            .request("mock.applyEditThenRespond", json!({}), context)
            .expect("request should receive final response after negative ack");
        assert!(response.result.is_null());
        assert_eq!(callback_count.load(Ordering::SeqCst), 0, "mode={mode}");
    }
}
