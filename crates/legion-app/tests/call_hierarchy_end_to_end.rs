//! Call hierarchy driven against a real language server, both directions.
//!
//! The app-level tests in `call_hierarchy_flow.rs` hand-deliver worker results,
//! so they can prove the drain routes a response it is *given*. What they could
//! not reach is the request the drain *issues*: the follow-up
//! `callHierarchy/incomingCalls` / `outgoingCalls` goes to a worker thread no
//! test could observe, so the best available assertion was "one more slot in the
//! bounded channel was consumed" — proof that *a* request went out, and nothing
//! about which one. Two failures survive that: a chain that asks the server for
//! callers when the user asked for callees, and a chain that drops the prepared
//! item's opaque `data`. Both render a plausible, wrong list.
//!
//! The mock server now answers all three methods with direction-specific
//! payloads, so the whole two-step conversation can be driven for real:
//!
//! - step one, `textDocument/prepareCallHierarchy`, resolves the caret to
//!   `mock_prepared_symbol`;
//! - step two answers `callHierarchy/incomingCalls` with `mock_caller` at two
//!   call sites, and `callHierarchy/outgoingCalls` with `mock_callee` at one.
//!
//! `mock_caller` and `mock_callee` are disjoint names that only ever appear in
//! one direction's answer, which is what turns "the panel filled up" into "the
//! panel filled up with the answer to the question that was asked". Every row
//! here exists only if both round trips completed: the prepare response had to
//! be parsed into an item, the item had to be handed to the follow-up request,
//! and the follow-up's answer had to be projected under the right direction.

use legion_app::AppComposition;
use legion_protocol::{
    CallHierarchyDirection, LanguageLocationProjection, LspSessionLifecycleKind, PrincipalId,
    TextCoordinate, WorkspaceTrustState,
};
use legion_ui::CommandDispatchIntent;

mod lsp_mock;

/// Bound on every wait in this file: 600 polls of 10ms, matching the other
/// mock-server suites. A wait that needs longer than six seconds is a hang, and
/// a hang reported as a timeout is more useful than one reported as a deadlock.
const MAX_POLLS: usize = 600;
const POLL_INTERVAL: std::time::Duration = std::time::Duration::from_millis(10);

/// Budget for waiting on something that must *not* happen.
///
/// A negative wait cannot be bounded by the event it is watching for, so it
/// burns its whole budget every run. One second is many times what the two
/// round trips take against the same server in the tests above, which is the
/// only calibration available.
const NEGATIVE_POLLS: usize = 100;

/// An app with a Live mock-server session and one open Rust file.
///
/// The `TempDir` is returned so the workspace outlives the test body; dropping
/// it early deletes the manifest under the running session.
struct LiveApp {
    app: AppComposition,
    buffer_id: legion_protocol::BufferId,
    _root: tempfile::TempDir,
}

fn coordinate(line: u32, character: u32) -> TextCoordinate {
    TextCoordinate {
        line,
        character,
        byte_offset: None,
        utf16_offset: None,
    }
}

/// The caret position every test asks from: inside `fn main`, on the call.
fn caret() -> TextCoordinate {
    coordinate(1, 8)
}

fn live_app() -> LiveApp {
    let mock_path = lsp_mock::mock_server_path().expect(
        "mock_lsp_server not found — run `cargo build -p legion-lsp --bin mock_lsp_server -j 6`",
    );

    let root = tempfile::tempdir().expect("tempdir");
    // A real manifest, because the session refuses a workspace without one
    // (`lifecycle=Refused reason="no Cargo.toml in workspace root"`). That is
    // the product being careful rather than the test being wrong.
    std::fs::write(
        root.path().join("Cargo.toml"),
        "[package]\nname = \"fixture\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
    )
    .expect("write manifest");
    std::fs::create_dir_all(root.path().join("src")).expect("mkdir src");
    let source = root.path().join("src").join("main.rs");
    std::fs::write(&source, "fn main() {\n    helper();\n}\n").expect("write source");

    let mut app = AppComposition::new();
    app.open_workspace(
        root.path(),
        WorkspaceTrustState::Trusted,
        PrincipalId("test".to_string()),
    )
    .expect("open workspace");
    app.open_file(source.to_string_lossy()).expect("open file");
    let buffer_id = app.active_buffer_id().expect("active buffer");

    app.force_lsp_start_with_server_path_for_test(mock_path);

    // Startup is asynchronous: drain until the session reports Live rather than
    // sleeping for a guessed interval.
    let mut became_live = false;
    for _ in 0..MAX_POLLS {
        app.drain_lsp_session();
        if app.lsp_session_status_projection().lifecycle == LspSessionLifecycleKind::Live {
            became_live = true;
            break;
        }
        std::thread::sleep(POLL_INTERVAL);
    }
    let status = app.lsp_session_status_projection();
    assert!(
        became_live,
        "the mock session never reached Live; lifecycle={:?} reason={:?}",
        status.lifecycle, status.failure_reason
    );

    LiveApp {
        app,
        buffer_id,
        _root: root,
    }
}

impl LiveApp {
    /// Ask the question a user's keypress asks.
    ///
    /// Deliberately routed through `dispatch_ui_intent` rather than by calling
    /// `issue_lsp_prepare_call_hierarchy_request` directly: the direction is
    /// chosen in the intent router, carried through `pending_call_hierarchy`,
    /// and consumed in the drain. Calling the issue function directly would
    /// skip the first of those three and test a chain no gesture travels.
    fn ask(&mut self, direction: CallHierarchyDirection) {
        let intent = match direction {
            CallHierarchyDirection::Incoming => CommandDispatchIntent::ShowIncomingCalls {
                buffer_id: self.buffer_id,
                position: caret(),
            },
            CallHierarchyDirection::Outgoing => CommandDispatchIntent::ShowOutgoingCalls {
                buffer_id: self.buffer_id,
                position: caret(),
            },
        };
        self.app
            .dispatch_ui_intent(intent)
            .expect("call-hierarchy intent dispatches");
    }

    /// Drain until rows arrive, then return them with the direction stamped on
    /// them and whether the panel still thinks it is waiting.
    ///
    /// Both round trips happen inside this loop: the first drain that carries
    /// the prepare response is also the one that issues the follow-up request,
    /// and the answer to that arrives on a later poll.
    fn await_rows(
        &mut self,
    ) -> (
        Vec<LanguageLocationProjection>,
        Option<CallHierarchyDirection>,
        bool,
    ) {
        for _ in 0..MAX_POLLS {
            self.app.drain_lsp_session();
            let projection = self.app.language_tooling_projection();
            if !projection.call_hierarchy.is_empty() {
                return (
                    projection.call_hierarchy,
                    projection.call_hierarchy_direction,
                    projection.call_hierarchy_awaiting,
                );
            }
            std::thread::sleep(POLL_INTERVAL);
        }
        let projection = self.app.language_tooling_projection();
        panic!(
            "no call-hierarchy rows arrived within {MAX_POLLS} polls; the two round trips did not \
             complete. direction={:?} awaiting={} status={:?} message={:?}",
            projection.call_hierarchy_direction,
            projection.call_hierarchy_awaiting,
            projection.status,
            projection.status_message,
        );
    }
}

/// The call-site lines of a row set, in order.
fn lines(rows: &[LanguageLocationProjection]) -> Vec<u32> {
    rows.iter()
        .filter_map(|row| row.range.as_ref().map(|range| range.start.line))
        .collect()
}

/// "Who calls this" reaches the server and comes back with callers.
///
/// The row content is the load-bearing part. `mock_caller` exists nowhere but
/// in the `incomingCalls` answer — not on disk, not in the lexical index, not
/// in the `outgoingCalls` payload — so a row naming it can only have come from
/// a completed prepare -> incomingCalls conversation with this server.
#[test]
fn asking_who_calls_this_returns_the_callers_the_server_named() {
    let mut live = live_app();

    // Before any answer: the panel is empty *and* says it is waiting. This is
    // the distinction `call_hierarchy_awaiting` exists to draw — empty rows
    // while a question is outstanding must not read as "the server said nobody
    // calls this".
    live.ask(CallHierarchyDirection::Incoming);
    let asking = live.app.language_tooling_projection();
    assert!(
        asking.call_hierarchy.is_empty(),
        "the index leg has no call graph, so nothing may appear before the server answers"
    );
    assert!(
        asking.call_hierarchy_awaiting,
        "an outstanding question must be flagged as outstanding, or an empty panel is \
         indistinguishable from the answer 'nobody calls this'"
    );

    let (rows, direction, awaiting) = live.await_rows();

    assert_eq!(
        direction,
        Some(CallHierarchyDirection::Incoming),
        "callers must be labelled as callers; the rows render identically either way"
    );
    assert!(
        !awaiting,
        "the answer has landed, so the wait must be over — a panel stuck on 'waiting' with rows \
         in it tells the reader the list is provisional when it is final"
    );

    // Two `fromRanges` for one caller: the design says one row per call site,
    // not per symbol. A caller that invokes the symbol twice is two places a
    // reader may want to go, and collapsing them loses one.
    assert_eq!(
        rows.len(),
        2,
        "the mock's single caller has two call sites and must produce two rows, got {rows:?}"
    );
    assert!(
        rows.iter()
            .all(|row| row.label == "mock_caller — caller_module"),
        "every row must name the server's caller, with the detail that disambiguates it: {rows:?}"
    );
    // Lines 5 and 6 are the call sites; line 4 is `mock_caller`'s own
    // declaration. Pointing at 4 would render a plausible list that navigates
    // to the wrong place.
    assert_eq!(
        lines(&rows),
        vec![5, 6],
        "rows must point at the call sites, not at the caller's declaration (line 4)"
    );
    assert_eq!(
        rows.iter()
            .map(|row| row.location_id.clone())
            .collect::<std::collections::BTreeSet<_>>()
            .len(),
        2,
        "two call sites need two distinct ids, or a selection cannot address them: {rows:?}"
    );
    assert!(
        rows.iter().all(|row| !row.degraded),
        "the server gave real call-site ranges, so no row may claim to be a fallback: {rows:?}"
    );

    // The negative half of the direction property. `mock_callee` is only ever
    // in the `outgoingCalls` answer, so its absence proves the follow-up asked
    // the question the user asked.
    assert!(
        !rows.iter().any(|row| row.label.contains("mock_callee")),
        "asking who calls this must not have asked what this calls: {rows:?}"
    );
}

/// "What does this call" reaches the server and comes back with callees.
///
/// The mirror of the test above, and the one the previous agent could not
/// write. `incomingCalls` reports `from` and `outgoingCalls` reports `to`;
/// reading the wrong side, or issuing the wrong follow-up method, compiles
/// fine. Both mistakes are caught here, because the row names a symbol only the
/// `outgoingCalls` arm of the mock ever emits.
#[test]
fn asking_what_this_calls_returns_the_callees_the_server_named() {
    let mut live = live_app();

    live.ask(CallHierarchyDirection::Outgoing);
    let asking = live.app.language_tooling_projection();
    assert!(
        asking.call_hierarchy_awaiting,
        "the outgoing question must flag its own wait too"
    );

    let (rows, direction, awaiting) = live.await_rows();

    assert_eq!(
        direction,
        Some(CallHierarchyDirection::Outgoing),
        "callees must be labelled as callees"
    );
    assert!(!awaiting, "the answer has landed, so the wait must be over");
    assert_eq!(
        rows.len(),
        1,
        "the mock's single callee has one call site, got {rows:?}"
    );
    assert_eq!(
        rows[0].label, "mock_callee — callee_module",
        "the row must name the server's callee; `mock_caller` here would mean the follow-up \
         asked for the opposite direction, and `mock_prepared_symbol` would mean the second \
         round trip never happened"
    );
    // Line 1 is where the caller invokes the callee; line 10 is the callee's
    // own declaration. `outgoingCalls` reports the *call site in the caller*,
    // which is the counter-intuitive half of the protocol and the easy one to
    // wire to `to.selectionRange` instead.
    assert_eq!(
        lines(&rows),
        vec![1],
        "an outgoing row points at the call site in the caller, not the callee's declaration"
    );
    assert!(
        !rows.iter().any(|row| row.label.contains("mock_caller")),
        "asking what this calls must not have asked who calls this: {rows:?}"
    );
}

/// One session, both questions, and the second answer replaces the first.
///
/// The direction lives in `pending_call_hierarchy` between the two round trips,
/// which is state that survives a request. A chain that latched the first
/// direction — or that failed to clear the slot — would answer the second
/// question with the first question's answer, and the two single-direction
/// tests above could not see it because each one only ever asks once.
#[test]
fn switching_direction_against_the_same_live_server_switches_the_answer() {
    let mut live = live_app();

    live.ask(CallHierarchyDirection::Incoming);
    let (incoming, incoming_direction, _) = live.await_rows();
    assert_eq!(incoming_direction, Some(CallHierarchyDirection::Incoming));
    assert!(
        incoming.iter().all(|row| row.label.contains("mock_caller")),
        "first question must be answered with callers: {incoming:?}"
    );

    live.ask(CallHierarchyDirection::Outgoing);
    // The index leg clears the previous rows as it stamps the new wait, so the
    // callers must be gone the moment the second question is asked rather than
    // lingering under an "outgoing" heading.
    let mid_flight = live.app.language_tooling_projection();
    assert!(
        mid_flight.call_hierarchy.is_empty(),
        "the previous answer must not survive into a question it does not answer: {:?}",
        mid_flight.call_hierarchy
    );
    assert!(
        mid_flight.call_hierarchy_awaiting,
        "the second question is outstanding and must say so"
    );

    let (outgoing, outgoing_direction, awaiting) = live.await_rows();
    assert_eq!(outgoing_direction, Some(CallHierarchyDirection::Outgoing));
    assert!(!awaiting);
    assert!(
        outgoing.iter().all(|row| row.label.contains("mock_callee")),
        "the second question must be answered with callees, not with the first answer \
         relabelled: {outgoing:?}"
    );
    assert_ne!(
        incoming, outgoing,
        "two opposite questions answered identically means the direction never reached the \
         follow-up request"
    );
}

/// Prepare-only stops after one round trip.
///
/// `PrepareCallHierarchy` resolves the symbol under the caret and asks nothing
/// further. The interesting failure is the opposite of the ones above: a chain
/// that treats a missing direction as a default would fire a follow-up nobody
/// asked for, and the panel would fill with an answer to an unasked question.
/// Rows arriving here at all is the defect.
#[test]
fn preparing_without_a_direction_never_asks_a_follow_up_question() {
    let mut live = live_app();

    live.app
        .dispatch_ui_intent(CommandDispatchIntent::PrepareCallHierarchy {
            buffer_id: live.buffer_id,
            position: caret(),
        })
        .expect("prepare intent dispatches");

    // Long enough for both round trips to have completed if either had been
    // issued: the two directional tests above reach their rows well inside this
    // budget against the same server. The loop keeps draining after rows appear
    // only to stop early — the assertion below is what fails.
    for _ in 0..NEGATIVE_POLLS {
        live.app.drain_lsp_session();
        if !live
            .app
            .language_tooling_projection()
            .call_hierarchy
            .is_empty()
        {
            break;
        }
        std::thread::sleep(POLL_INTERVAL);
    }

    let projection = live.app.language_tooling_projection();
    assert!(
        projection.call_hierarchy.is_empty(),
        "prepare-only asked no question, so no answer may appear: {:?}",
        projection.call_hierarchy
    );
    assert!(
        !projection.call_hierarchy_awaiting,
        "nothing is outstanding, so the panel must not claim to be waiting"
    );
    assert_eq!(
        projection.call_hierarchy_direction, None,
        "no direction was chosen, so none may be claimed"
    );
}

/// A question the product declined to ask must not look like one in flight.
///
/// `run_call_hierarchy` used to discard the result of issuing the prepare
/// request and run the index leg regardless, and that leg sets
/// `call_hierarchy_awaiting`. With no live session — or a server that never
/// advertised `callHierarchyProvider` — nothing went out and the panel sat on
/// "asking…" forever.
///
/// That is the inversion of the misreading the flag was added for. Empty rows
/// with a direction says "the server answered: nobody calls this"; empty rows
/// while awaiting says "we are still asking". Getting stuck in the second is
/// worse than either, because it is a promise the product cannot keep.
#[test]
fn declining_to_ask_is_not_reported_as_waiting_for_an_answer() {
    let workspace = tempfile::tempdir().expect("tempdir");
    std::fs::write(
        workspace.path().join("Cargo.toml"),
        "[package]\nname = \"fixture\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
    )
    .expect("write manifest");
    std::fs::create_dir_all(workspace.path().join("src")).expect("mkdir src");
    let source = workspace.path().join("src").join("main.rs");
    std::fs::write(&source, "fn main() {}\n").expect("write source");

    let mut app = legion_app::AppComposition::new();
    app.open_workspace(
        workspace.path(),
        legion_protocol::WorkspaceTrustState::Trusted,
        legion_protocol::PrincipalId("test".to_string()),
    )
    .expect("open workspace");
    app.open_file(source.to_string_lossy()).expect("open file");
    let buffer_id = app.active_buffer_id().expect("active buffer");

    // No session is started at all, so the prepare request cannot go out.
    let position = legion_protocol::TextCoordinate {
        line: 0,
        character: 3,
        byte_offset: None,
        utf16_offset: None,
    };
    let _ = app.dispatch_ui_intent(legion_ui::CommandDispatchIntent::ShowIncomingCalls {
        buffer_id,
        position,
    });

    let projection = app.language_tooling_projection();
    assert!(
        !projection.call_hierarchy_awaiting,
        "no request was issued, so nothing is awaited; a stuck flag renders as a \
         spinner that never resolves"
    );
    assert!(
        projection.call_hierarchy.is_empty(),
        "no answer can exist for a question that was never asked"
    );
}
