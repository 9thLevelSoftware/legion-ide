//! Call hierarchy at app level: the seams between the intent and the panel.
//!
//! The module's own unit tests cover row shaping. What they cannot see is the
//! four places this feature can be complete and still answer nothing, or answer
//! wrongly:
//!
//! 1. the intent has to route to its own request — this one silently routed to
//!    `Noop` for months while every layer below it worked;
//! 2. a response has to reach `projection.call_hierarchy` *and* stamp the
//!    direction, because "who calls this" and "what does this call" are
//!    opposite answers that render as the same list;
//! 3. the second question has to replace the first answer, not extend it;
//! 4. the two round trips have to chain, and only for the request that is
//!    actually outstanding.

use legion_app::language::{LspReadKind, LspReadOutcome, LspRequestTag, LspWorkerResult};
use legion_app::{AppCommandOutcome, AppComposition};
use legion_protocol::{
    BufferId, CallHierarchyDirection, LanguageToolingOperationKind, LspCapabilitySummary,
    LspResultStatus, LspServerBinaryProvenance, LspServerHealthRecord, PrincipalId, SnapshotId,
    TextCoordinate, WorkspaceTrustState,
};
use legion_ui::CommandDispatchIntent;

/// A Live session advertising exactly the capabilities these tests issue.
///
/// `callHierarchyProvider` gates both round trips; `referencesProvider` and
/// `definitionProvider` are here because the cross-contamination tests need
/// neighbouring features to have something in them.
fn live_health() -> LspServerHealthRecord {
    LspServerHealthRecord {
        server_id: legion_protocol::LanguageServerId(1),
        language_id: legion_protocol::LanguageId("rust".to_string()),
        binary_provenance: LspServerBinaryProvenance::Configured,
        binary_path_hash: None,
        artifact_hash: None,
        version: None,
        init_status: LspResultStatus::Fresh,
        capabilities: [
            "callHierarchyProvider",
            "referencesProvider",
            "definitionProvider",
        ]
        .iter()
        .map(|name| LspCapabilitySummary {
            capability: name.to_string(),
            supported: true,
            dynamic_registration: false,
            option_hash: None,
            redaction_hints: Vec::new(),
            schema_version: 1,
        })
        .collect(),
        diagnostics_latency_ms: None,
        restart_count: 0,
        download_decision_id: None,
        schema_version: 1,
    }
}

fn coordinate(line: u32, character: u32) -> TextCoordinate {
    TextCoordinate {
        line,
        character,
        byte_offset: None,
        utf16_offset: None,
    }
}

fn lsp_range(start_line: u32, start_character: u32, end_character: u32) -> serde_json::Value {
    serde_json::json!({
        "start": { "line": start_line, "character": start_character },
        "end": { "line": start_line, "character": end_character },
    })
}

fn protocol_range(
    line: u32,
    start_character: u32,
    end_character: u32,
) -> legion_protocol::ProtocolTextRange {
    legion_protocol::ProtocolTextRange {
        start: coordinate(line, start_character),
        end: coordinate(line, end_character),
    }
}

/// One `prepareCallHierarchy` item, shaped the way rust-analyzer sends it.
fn prepare_item(name: &str) -> serde_json::Value {
    serde_json::json!({
        "name": name,
        "kind": 12,
        "uri": "file:///w/main.rs",
        "range": lsp_range(0, 0, 12),
        "selectionRange": lsp_range(0, 3, 7),
        "data": { "resolution": "opaque-server-state" },
    })
}

/// An open Rust workspace with a Live session whose result channel is held by
/// the caller. Mirrors `lsp_read_drain_routing.rs` so both files describe the
/// same product setup.
fn app_with_live_session() -> (
    AppComposition,
    BufferId,
    std::sync::mpsc::SyncSender<LspWorkerResult>,
    tempfile::TempDir,
) {
    let root = tempfile::tempdir().expect("tempdir");
    std::fs::write(root.path().join("Cargo.toml"), "[package]\nname = \"x\"\n").expect("manifest");
    let src = root.path().join("main.rs");
    std::fs::write(&src, "fn main() {\n    render_frame();\n}\n").expect("source");

    let mut app = AppComposition::new();
    app.open_workspace(
        root.path(),
        WorkspaceTrustState::Trusted,
        PrincipalId("test".to_string()),
    )
    .expect("open workspace");
    app.open_file(src.to_string_lossy()).expect("open file");
    let buffer_id = app.active_buffer_id().expect("active buffer");
    let sender = app.inject_lsp_result_sender_for_test(live_health());
    (app, buffer_id, sender, root)
}

/// Push one worker result through the real drain path.
fn deliver(
    app: &mut AppComposition,
    sender: &std::sync::mpsc::SyncSender<LspWorkerResult>,
    buffer_id: BufferId,
    kind: LspReadKind,
    result: serde_json::Value,
) {
    let issued_snapshot = app
        .current_snapshot_id_for_test(buffer_id)
        .expect("snapshot id");
    deliver_at_snapshot(app, sender, buffer_id, kind, result, issued_snapshot);
}

fn deliver_at_snapshot(
    app: &mut AppComposition,
    sender: &std::sync::mpsc::SyncSender<LspWorkerResult>,
    buffer_id: BufferId,
    kind: LspReadKind,
    result: serde_json::Value,
    issued_snapshot: SnapshotId,
) {
    sender
        .send(LspWorkerResult::ReadResult {
            outcome: Ok(LspReadOutcome {
                result,
                issued_snapshot,
                status: LspResultStatus::Fresh,
            }),
            tag: LspRequestTag {
                buffer_id,
                kind,
                snapshot_id: issued_snapshot,
            },
        })
        .expect("send result");
    app.drain_lsp_session();
}

/// A caller that invokes the symbol at two sites, plus a second caller.
fn incoming_calls_payload() -> serde_json::Value {
    serde_json::json!([
        {
            "from": {
                "name": "render_frame",
                "kind": 12,
                "uri": "file:///w/ui.rs",
                "range": lsp_range(40, 0, 24),
                "selectionRange": lsp_range(40, 3, 15),
                "detail": "ui::frame",
            },
            "fromRanges": [lsp_range(44, 8, 20), lsp_range(46, 8, 20)],
        },
        {
            "from": {
                "name": "boot",
                "kind": 12,
                "uri": "file:///w/boot.rs",
                "range": lsp_range(2, 0, 10),
                "selectionRange": lsp_range(2, 3, 7),
            },
            "fromRanges": [lsp_range(7, 4, 16)],
        }
    ])
}

fn outgoing_calls_payload() -> serde_json::Value {
    serde_json::json!([
        {
            "to": {
                "name": "flush_buffers",
                "kind": 12,
                "uri": "file:///w/io.rs",
                "range": lsp_range(11, 0, 30),
                "selectionRange": lsp_range(11, 3, 16),
                "detail": "io::sink",
            },
            "fromRanges": [lsp_range(52, 12, 25)],
        }
    ])
}

fn location_payload(uri: &str, line: u32) -> serde_json::Value {
    serde_json::json!([{ "uri": uri, "range": lsp_range(line, 4, 9) }])
}

/// The intents reach app authority instead of dying as `Noop`.
///
/// This is the defect the feature exists to retire: every layer below worked
/// while the router answered `Noop`, so the gesture did nothing and nothing
/// failed. `IncomingCalls`/`OutgoingCalls` appearing in the operations list is
/// proof the request was routed as itself — a `Noop` records no operation, and
/// routing the two directions to one another would show the wrong kind.
#[test]
fn the_call_hierarchy_intents_route_to_their_own_requests() {
    let (mut app, buffer_id, _sender, _root) = app_with_live_session();
    let position = coordinate(1, 6);

    let outcome = app
        .dispatch_ui_intent(CommandDispatchIntent::ShowIncomingCalls {
            buffer_id,
            position,
        })
        .expect("incoming-calls intent dispatches");
    assert!(
        matches!(outcome, AppCommandOutcome::LanguageToolingUpdated(_)),
        "ShowIncomingCalls must reach language tooling, got {outcome:?}"
    );

    app.dispatch_ui_intent(CommandDispatchIntent::ShowOutgoingCalls {
        buffer_id,
        position,
    })
    .expect("outgoing-calls intent dispatches");

    let operations = app.language_tooling_projection().operations;
    for expected in [
        LanguageToolingOperationKind::IncomingCalls,
        LanguageToolingOperationKind::OutgoingCalls,
    ] {
        assert!(
            operations.iter().any(|op| op.kind == expected),
            "{expected:?} must be recorded as its own operation, got {operations:?}"
        );
    }
}

/// Prepare-only is a routed request that deliberately records nothing.
///
/// It resolves the symbol under the caret and stops; there is no answer for a
/// panel to show yet. Routing it to `Noop` would look identical from the
/// operations list, so the outcome variant is what separates them.
#[test]
fn prepare_call_hierarchy_routes_without_claiming_to_have_an_answer() {
    let (mut app, buffer_id, _sender, _root) = app_with_live_session();
    let before = app.language_tooling_projection().operations.len();

    let outcome = app
        .dispatch_ui_intent(CommandDispatchIntent::PrepareCallHierarchy {
            buffer_id,
            position: coordinate(1, 6),
        })
        .expect("prepare intent dispatches");

    assert!(
        matches!(outcome, AppCommandOutcome::LanguageToolingUpdated(_)),
        "PrepareCallHierarchy must route to language tooling, not Noop, got {outcome:?}"
    );
    assert_eq!(
        app.language_tooling_projection().operations.len(),
        before,
        "the prepare step asks no question a panel can answer, so it records no operation"
    );
}

/// An `incomingCalls` response reaches the rows and names the question it
/// answered.
///
/// The direction is the only thing distinguishing this list from its opposite,
/// so an unset direction is a list the panel cannot label. The row ranges are
/// checked against the call sites rather than the caller's own declaration:
/// pointing at line 40 instead of 44 would still render a plausible list and
/// navigate to the wrong place.
#[test]
fn an_incoming_calls_response_becomes_rows_stamped_incoming() {
    let (mut app, buffer_id, sender, _root) = app_with_live_session();
    assert_eq!(
        app.language_tooling_projection().call_hierarchy_direction,
        None,
        "no question has been answered yet, so nothing may claim a direction"
    );

    deliver(
        &mut app,
        &sender,
        buffer_id,
        LspReadKind::IncomingCalls,
        incoming_calls_payload(),
    );

    let projection = app.language_tooling_projection();
    assert_eq!(
        projection.call_hierarchy_direction,
        Some(CallHierarchyDirection::Incoming),
        "callers must be labelled as callers"
    );
    assert_eq!(
        projection.call_hierarchy.len(),
        3,
        "two sites in one caller plus one in another, got {:?}",
        projection.call_hierarchy
    );
    assert!(
        projection
            .call_hierarchy
            .iter()
            .any(|row| row.label == "render_frame — ui::frame"),
        "the detail disambiguates same-named callers and must survive to the row, got {:?}",
        projection.call_hierarchy
    );
    let call_site_lines: Vec<u32> = projection
        .call_hierarchy
        .iter()
        .filter_map(|row| row.range.as_ref().map(|range| range.start.line))
        .collect();
    assert_eq!(
        call_site_lines,
        vec![44, 46, 7],
        "rows must point at the call sites, not at the caller's declaration"
    );
}

/// An `outgoingCalls` response reads the callee side and stamps `Outgoing`.
///
/// `incomingCalls` reports `from` and `outgoingCalls` reports `to`. A payload
/// with only `to` produces nothing at all if this result is handed to the
/// incoming projector, which is what makes the row count load-bearing here.
#[test]
fn an_outgoing_calls_response_becomes_rows_stamped_outgoing() {
    let (mut app, buffer_id, sender, _root) = app_with_live_session();

    deliver(
        &mut app,
        &sender,
        buffer_id,
        LspReadKind::OutgoingCalls,
        outgoing_calls_payload(),
    );

    let projection = app.language_tooling_projection();
    assert_eq!(
        projection.call_hierarchy_direction,
        Some(CallHierarchyDirection::Outgoing),
        "callees must be labelled as callees"
    );
    assert_eq!(
        projection.call_hierarchy.len(),
        1,
        "the callee side of the response must be read, got {:?}",
        projection.call_hierarchy
    );
    assert_eq!(
        projection.call_hierarchy[0].label,
        "flush_buffers — io::sink"
    );
}

/// Asking the other direction replaces the answer instead of adding to it.
///
/// Stale callers listed beside fresh callees is not an untidy panel, it is a
/// wrong answer: the rows are indistinguishable once rendered, and the single
/// direction field would label all of them with whichever question came last.
#[test]
fn the_second_question_replaces_the_first_answer() {
    let (mut app, buffer_id, sender, _root) = app_with_live_session();

    deliver(
        &mut app,
        &sender,
        buffer_id,
        LspReadKind::IncomingCalls,
        incoming_calls_payload(),
    );
    assert_eq!(app.language_tooling_projection().call_hierarchy.len(), 3);

    deliver(
        &mut app,
        &sender,
        buffer_id,
        LspReadKind::OutgoingCalls,
        outgoing_calls_payload(),
    );

    let projection = app.language_tooling_projection();
    assert_eq!(
        projection.call_hierarchy.len(),
        1,
        "the callers must be gone, not appended to, got {:?}",
        projection.call_hierarchy
    );
    assert!(
        !projection
            .call_hierarchy
            .iter()
            .any(|row| row.label.starts_with("render_frame")),
        "a caller from the previous question must not survive into the answer to this one"
    );
    assert_eq!(
        projection.call_hierarchy_direction,
        Some(CallHierarchyDirection::Outgoing),
        "the label must follow the rows"
    );
}

/// Call hierarchy shares a projection with references and definitions, and
/// must not overwrite either.
///
/// All three are lists of locations in the same struct. A go-to-definition
/// result silently emptied by asking who calls something would look like the
/// language server had failed.
#[test]
fn a_call_hierarchy_answer_leaves_references_and_definitions_alone() {
    let (mut app, buffer_id, sender, _root) = app_with_live_session();

    deliver(
        &mut app,
        &sender,
        buffer_id,
        LspReadKind::Definition,
        location_payload("file:///w/defs.rs", 3),
    );
    deliver(
        &mut app,
        &sender,
        buffer_id,
        LspReadKind::References,
        location_payload("file:///w/refs.rs", 9),
    );
    let before = app.language_tooling_projection();
    assert_eq!(before.definitions.len(), 1, "definition fixture must land");
    assert_eq!(before.references.len(), 1, "references fixture must land");

    deliver(
        &mut app,
        &sender,
        buffer_id,
        LspReadKind::IncomingCalls,
        incoming_calls_payload(),
    );

    let after = app.language_tooling_projection();
    assert_eq!(
        after.definitions, before.definitions,
        "call hierarchy must not disturb definitions"
    );
    assert_eq!(
        after.references, before.references,
        "call hierarchy must not disturb references"
    );
    assert_eq!(after.call_hierarchy.len(), 3);
}

/// …and the reverse: a references answer must not empty the call list.
#[test]
fn a_references_answer_leaves_the_call_hierarchy_alone() {
    let (mut app, buffer_id, sender, _root) = app_with_live_session();

    deliver(
        &mut app,
        &sender,
        buffer_id,
        LspReadKind::IncomingCalls,
        incoming_calls_payload(),
    );
    let calls = app.language_tooling_projection().call_hierarchy;

    deliver(
        &mut app,
        &sender,
        buffer_id,
        LspReadKind::References,
        location_payload("file:///w/refs.rs", 9),
    );

    let after = app.language_tooling_projection();
    assert_eq!(
        after.call_hierarchy, calls,
        "a references answer must leave the call rows untouched"
    );
    assert_eq!(
        after.call_hierarchy_direction,
        Some(CallHierarchyDirection::Incoming),
        "and must leave the label that describes them untouched too"
    );
    assert_eq!(after.references.len(), 1);
}

/// A stale call-hierarchy response is discarded like any other read.
///
/// The rows carry positions into a document that has since changed; showing
/// them would navigate to lines that have moved.
#[test]
fn a_call_hierarchy_response_issued_against_an_old_snapshot_is_discarded() {
    let (mut app, buffer_id, sender, _root) = app_with_live_session();
    let old_snapshot = app
        .current_snapshot_id_for_test(buffer_id)
        .expect("snapshot id");
    app.dispatch_ui_intent(CommandDispatchIntent::Insert {
        buffer_id,
        at: coordinate(0, 0),
        text: "// edited\n".to_string(),
    })
    .expect("edit applies");
    assert_ne!(
        app.current_snapshot_id_for_test(buffer_id),
        Some(old_snapshot),
        "the edit must actually move the snapshot, or this test proves nothing"
    );

    deliver_at_snapshot(
        &mut app,
        &sender,
        buffer_id,
        LspReadKind::IncomingCalls,
        incoming_calls_payload(),
        old_snapshot,
    );

    assert!(
        app.language_tooling_projection().call_hierarchy.is_empty(),
        "rows resolved against text that no longer exists must not be shown"
    );

    // The same payload delivered against the current snapshot does land, so the
    // emptiness above is the staleness gate and not a payload this path could
    // never read.
    deliver(
        &mut app,
        &sender,
        buffer_id,
        LspReadKind::IncomingCalls,
        incoming_calls_payload(),
    );
    assert_eq!(
        app.language_tooling_projection().call_hierarchy.len(),
        3,
        "a fresh response with the identical payload must reach the panel"
    );
}

/// Counts how many more requests the worker channel will accept.
///
/// The follow-up request the drain issues goes to a worker that no test can
/// observe directly, so what is measured is the space it consumed: every
/// `issue_lsp_read` is exactly one bounded-channel send, and the channel in a
/// test session is never drained. Comparing two runs that differ only in
/// whether the chain should fire turns "a request went out" into a number,
/// without depending on what that channel's capacity happens to be.
fn remaining_request_slots(app: &mut AppComposition, buffer_id: BufferId) -> usize {
    let mut slots = 0usize;
    while app.issue_lsp_references_request(buffer_id, coordinate(0, 0), true) {
        slots += 1;
        assert!(slots < 10_000, "the worker channel must be bounded");
    }
    slots
}

/// The prepare response chains into the second round trip — and only when it
/// should.
///
/// Three runs, identical up to the prepare response that comes back. An item
/// for the buffer that asked must produce a follow-up request; an empty list
/// (the caret was on whitespace) must not, and must not leave the caller
/// waiting; an item tagged for a different buffer must not be answered under
/// this buffer's heading. Without the chain, all three runs consume the same
/// number of slots and the first assertion fails.
#[test]
fn a_prepared_item_chains_into_the_second_request_only_for_the_buffer_that_asked() {
    fn run(prepare_response: serde_json::Value, respond_for_other_buffer: bool) -> usize {
        let root = tempfile::tempdir().expect("tempdir");
        std::fs::write(root.path().join("Cargo.toml"), "[package]\nname = \"x\"\n")
            .expect("manifest");
        let asking = root.path().join("main.rs");
        let other = root.path().join("other.rs");
        std::fs::write(&asking, "fn main() {\n    render_frame();\n}\n").expect("source");
        std::fs::write(&other, "fn other() {}\n").expect("other source");

        let mut app = AppComposition::new();
        app.open_workspace(
            root.path(),
            WorkspaceTrustState::Trusted,
            PrincipalId("test".to_string()),
        )
        .expect("open workspace");
        app.open_file(asking.to_string_lossy()).expect("open file");
        let asking_buffer = app.active_buffer_id().expect("active buffer");
        app.open_file(other.to_string_lossy()).expect("open other");
        let other_buffer = app.active_buffer_id().expect("active buffer");
        assert_ne!(asking_buffer, other_buffer, "two distinct buffers");

        // Injected last so the channel being counted starts empty.
        let sender = app.inject_lsp_result_sender_for_test(live_health());

        assert!(
            app.issue_lsp_prepare_call_hierarchy_request(
                asking_buffer,
                coordinate(1, 6),
                Some(CallHierarchyDirection::Incoming),
            ),
            "the prepare request must go out, or there is no pending state to test"
        );

        let responding_buffer = if respond_for_other_buffer {
            other_buffer
        } else {
            asking_buffer
        };
        deliver(
            &mut app,
            &sender,
            responding_buffer,
            LspReadKind::CallHierarchyPrepare,
            prepare_response,
        );

        remaining_request_slots(&mut app, asking_buffer)
    }

    let chained = run(serde_json::json!([prepare_item("main")]), false);
    let no_symbol = run(serde_json::json!([]), false);
    let wrong_buffer = run(serde_json::json!([prepare_item("other")]), true);

    assert_eq!(
        no_symbol,
        chained + 1,
        "a resolved item must cost one more request than an empty prepare response: \
         the incomingCalls follow-up"
    );
    assert_eq!(
        wrong_buffer, no_symbol,
        "a prepare response for a buffer that did not ask must not be followed up"
    );
}

/// Both round trips are gated on the same capability.
///
/// The follow-up request is issued from the drain, far from the gesture that
/// started it, so it is the easy one to leave ungated — and a server that never
/// advertised `callHierarchyProvider` answers it with an error the user sees as
/// the session misbehaving.
#[test]
fn no_call_hierarchy_request_is_issued_without_the_server_capability() {
    let root = tempfile::tempdir().expect("tempdir");
    std::fs::write(root.path().join("Cargo.toml"), "[package]\nname = \"x\"\n").expect("manifest");
    let src = root.path().join("main.rs");
    std::fs::write(&src, "fn main() {}\n").expect("source");

    let mut app = AppComposition::new();
    app.open_workspace(
        root.path(),
        WorkspaceTrustState::Trusted,
        PrincipalId("test".to_string()),
    )
    .expect("open workspace");
    app.open_file(src.to_string_lossy()).expect("open file");
    let buffer_id = app.active_buffer_id().expect("active buffer");

    let mut health = live_health();
    health
        .capabilities
        .retain(|capability| capability.capability != "callHierarchyProvider");
    let _sender = app.inject_lsp_result_sender_for_test(health);

    assert!(
        !app.issue_lsp_prepare_call_hierarchy_request(
            buffer_id,
            coordinate(0, 3),
            Some(CallHierarchyDirection::Incoming),
        ),
        "a server that does not advertise callHierarchyProvider must not be asked"
    );

    let item = legion_protocol::LspCallHierarchyItem {
        name: "main".to_string(),
        kind: 12,
        uri: "file:///w/main.rs".to_string(),
        range: protocol_range(0, 0, 12),
        selection_range: protocol_range(0, 3, 7),
        detail: None,
        data: None,
    };
    assert!(
        !app.issue_lsp_incoming_calls_request(buffer_id, &item),
        "the second round trip must be gated on the same capability as the first"
    );
    assert!(
        !app.issue_lsp_outgoing_calls_request(buffer_id, &item),
        "the second round trip must be gated on the same capability as the first"
    );
    // The gate must be the capability, not the session: another read the server
    // does advertise still goes out on the same channel.
    assert!(
        app.issue_lsp_references_request(buffer_id, coordinate(0, 3), true),
        "the session is Live, so refusing every read would prove nothing"
    );
}
