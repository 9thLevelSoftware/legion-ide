//! Checklist rows 5–7: can a person actually use Assist and Delegate?
//!
//! None of the three had ever been exercised in a windowed session, and all
//! three were broken from the outside while every projection test passed.
//!
//! * Row 5 (Assist: a deterministic proposal appears). Assist's rail blocked on
//!   `assisted_ai_projection.providers` being non-empty and told the user to go
//!   to Settings. That projection describes a Phase-4 assisted-AI *run*; it is
//!   populated only by one, no rendered control starts one, and choosing a
//!   preferred provider in Settings never adds a row to it. The block could not
//!   be cleared from inside the app, so Assist mode showed one dead-end card
//!   forever — while `Predict`, hidden behind the block, works with zero
//!   configuration through the always-registered deterministic local provider.
//!
//! * Row 6 (Assist Auto with Ollama). The fallback itself was already honest;
//!   nothing guarded that the surface keeps naming whichever provider actually
//!   answered, or that a preference for an absent provider resolves instead of
//!   leaving a request in flight.
//!
//! * Row 7 (Delegate chat: Streaming… then reply). `send_delegate_chat` —
//!   retrieval-backed, citation-carrying, able to stream a live reply — had no
//!   rendered control at all. `DesktopAction::SendDelegateChat` was pushed by
//!   nothing, the desktop palette does not offer the `:delegate-chat` verb, and
//!   the chat transcript was projected but never drawn. There was nowhere to
//!   type, so the row could not be exercised even in principle.
//!
//! Every test here drives the real accessibility tree: find the control, click
//! its real centre, settle a frame. Where a test needs a field to be focused or
//! filled, it proves that happened *before* asserting the property, because a
//! test that would also pass with the click landing on empty canvas is worth
//! nothing.

use std::path::Path;
use std::time::{Duration, Instant};

/// The placeholder `send_delegate_chat` inserts before the worker replies.
const STREAMING_PLACEHOLDER: &str = "Streaming response";

mod common;
use common::{
    TempWorkspace, click_at, clickable_center, enabled_clickable_center, full_frame_input,
    rendered_text,
};

use legion_desktop::{
    bridge::DesktopAction,
    workflow::{DesktopEframeApp, DesktopLaunchConfig, DesktopRuntime},
};

/// The copy the Assist rail used to show forever. Named once, asserted absent.
const DEAD_END_PROVIDER_COPY: &str = "Choose an AI provider to enable predictions.";

fn open_runtime(root: &Path) -> DesktopRuntime {
    DesktopRuntime::open(DesktopLaunchConfig::new(root.to_path_buf(), None))
        .expect("desktop runtime should open workspace")
}

/// A runtime with `main.rs` open, through the same path a mouse takes.
fn runtime_with_open_file(root: &Path) -> DesktopRuntime {
    let mut runtime = open_runtime(root);
    let _ = runtime.handle_action(DesktopAction::RefreshExplorer);
    let node = runtime
        .projection_snapshot()
        .explorer_projection
        .nodes
        .into_iter()
        .find(|node| node.name == "main.rs")
        .expect("the fixture workspace should project a `main.rs` row");
    runtime
        .handle_action(DesktopAction::SelectExplorerFile {
            file_id: node.file_id,
        })
        .expect("activating an explorer file should open it");
    assert!(
        runtime
            .projection_snapshot()
            .active_buffer_projection
            .buffer_id
            .is_some(),
        "the fixture must really have a buffer open before a mode is exercised"
    );
    runtime
}

fn fixture(prefix: &'static str) -> TempWorkspace {
    let workspace = TempWorkspace::new(prefix);
    workspace.write("main.rs", "fn main() {\n    let value = 1;\n}\n");
    workspace
}

/// Click the rendered mode button and confirm the projection followed.
///
/// Manual → Delegate is a confirmed transition, so this also drives the
/// confirmation dialog rather than assuming which policy applies.
fn switch_mode(app: &mut DesktopEframeApp, label: &str) {
    let primed = app.run_headless_full_frame(full_frame_input(Vec::new()));
    let button = clickable_center(&primed, label)
        .unwrap_or_else(|| panic!("the mode rail must offer a clickable `{label}` control"));
    let after = click_at(app, button);
    if let Some(confirm) = clickable_center(&after, "Confirm") {
        let _ = click_at(app, confirm);
    }
    assert_eq!(
        app.runtime_snapshot().product_mode.label(),
        label,
        "clicking `{label}` must actually change the projected product mode; \
         asserting on the surface before this holds would pass on a missed click"
    );
}

// ─── Row 5: Assist ──────────────────────────────────────────────────────────

/// Switching to Assist with a file open must reach a usable prediction control.
#[test]
fn assist_offers_a_prediction_control_without_any_provider_setup() {
    let workspace = fixture("legion_desktop_assist_reachability");
    let mut app = DesktopEframeApp::new(runtime_with_open_file(workspace.path()));
    switch_mode(&mut app, "Assist");

    let frame = app.run_headless_full_frame(full_frame_input(Vec::new()));
    assert!(
        app.runtime_snapshot()
            .assisted_ai_projection
            .providers
            .is_empty(),
        "this is the shipped starting state: no assisted-AI run has happened, \
         so no provider is projected. If that ever stops being true the gate \
         under test has moved and this test must be rewritten, not relaxed."
    );
    assert!(
        clickable_center(&frame, "Predict").is_some(),
        "Assist must offer `Predict` in its first frame. Rendered text was: {:?}",
        rendered_text(&frame)
    );
    assert!(
        !rendered_text(&frame)
            .iter()
            .any(|line| line == DEAD_END_PROVIDER_COPY),
        "Assist must not show a prerequisite the user cannot satisfy from the app"
    );
}

/// Clicking `Predict` must put a real suggestion on screen.
#[test]
fn clicking_predict_puts_a_deterministic_suggestion_on_screen() {
    let workspace = fixture("legion_desktop_assist_predict");
    let mut app = DesktopEframeApp::new(runtime_with_open_file(workspace.path()));
    switch_mode(&mut app, "Assist");

    let frame = app.run_headless_full_frame(full_frame_input(Vec::new()));
    let predict = clickable_center(&frame, "Predict")
        .expect("Assist must offer `Predict` before this test can click it");
    let after = click_at(&mut app, predict);

    let prediction = app
        .runtime_snapshot()
        .assist_inline_prediction_projection
        .active_prediction
        .expect(
            "clicking Predict must produce a prediction. A button that changes a \
             status line and produces nothing is the defect this row exists to catch.",
        );
    let text = rendered_text(&after);
    assert!(
        text.iter().any(|line| line == &prediction.ghost_text_label),
        "the suggested text `{}` must be rendered, not merely projected; frame was {text:?}",
        prediction.ghost_text_label
    );
    assert!(
        clickable_center(&after, "Accept").is_some()
            && clickable_center(&after, "Dismiss").is_some(),
        "a suggestion the user cannot accept or dismiss is not a suggestion"
    );
}

/// Assist keeps provider configuration reachable without making it a gate.
#[test]
fn assist_names_its_route_and_keeps_settings_reachable() {
    let workspace = fixture("legion_desktop_assist_route");
    let mut runtime = runtime_with_open_file(workspace.path());
    // A non-default preference, so the rendered line has to come from runtime
    // state rather than a constant that happens to match the default.
    let _ = runtime.handle_action(DesktopAction::SetPreferredAiProvider {
        provider_id: "ollama".to_string(),
    });
    let mut app = DesktopEframeApp::new(runtime);
    switch_mode(&mut app, "Assist");

    let frame = app.run_headless_full_frame(full_frame_input(Vec::new()));
    let text = rendered_text(&frame);
    assert!(
        text.iter()
            .any(|line| line.starts_with("Route: ") && line.contains("ollama")),
        "Assist must say which route it will actually use; the preference was \
         set to `ollama` and the frame was {text:?}"
    );
    assert!(
        clickable_center(&frame, "AI provider settings").is_some(),
        "removing the provider *block* must not remove the provider *route* to \
         Settings; a remote provider is an upgrade and has to stay discoverable"
    );
}

/// With no file open, Assist blocks on the one thing that is really missing.
#[test]
fn assist_without_a_buffer_blocks_on_the_buffer_and_offers_to_open_one() {
    let workspace = fixture("legion_desktop_assist_no_buffer");
    let mut app = DesktopEframeApp::new(open_runtime(workspace.path()));
    switch_mode(&mut app, "Assist");

    let frame = app.run_headless_full_frame(full_frame_input(Vec::new()));
    let text = rendered_text(&frame);
    assert!(
        text.iter()
            .any(|line| line == "Open a file to enable predictions."),
        "with no buffer Assist must name the buffer as the prerequisite; frame was {text:?}"
    );
    assert!(
        clickable_center(&frame, "Open file").is_some(),
        "a named prerequisite must come with the control that satisfies it"
    );
    assert!(
        clickable_center(&frame, "Predict").is_none(),
        "Predict has nothing to predict into without a buffer"
    );
}

// ─── Row 6: Assist Auto / Ollama ────────────────────────────────────────────

/// A preference for a provider that is not installed must resolve, not hang,
/// and the surface must keep naming whoever actually answered.
///
/// This asserts a property that holds whether or not Ollama is running on the
/// machine, because assuming either way is how a suite becomes machine-specific.
/// What must never happen is a request that stays in flight, or a panel that
/// shows ghost text without saying where it came from.
#[test]
fn a_remote_route_preference_resolves_and_the_panel_names_the_real_provider() {
    let workspace = fixture("legion_desktop_assist_remote_pref");
    let mut runtime = runtime_with_open_file(workspace.path());
    let _ = runtime.handle_action(DesktopAction::SetProductMode {
        mode: legion_ui::DockMode::Assist,
    });
    let _ = runtime.handle_action(DesktopAction::SetPreferredAiProvider {
        provider_id: "ollama".to_string(),
    });
    let mut app = DesktopEframeApp::new(runtime);

    let frame = app.run_headless_full_frame(full_frame_input(Vec::new()));
    let predict =
        clickable_center(&frame, "Predict").expect("Assist must offer `Predict` to exercise row 6");

    let started = Instant::now();
    let after = click_at(&mut app, predict);
    let elapsed = started.elapsed();

    let projection = app
        .runtime_snapshot()
        .assist_inline_prediction_projection
        .clone();
    assert!(
        elapsed < Duration::from_secs(30),
        "a prediction request against an absent provider must fall back rather \
         than block the UI thread; it took {elapsed:?}"
    );
    assert!(
        !projection.request_in_flight,
        "the request must not still be in flight once the frame returns; a \
         status that says `requesting` forever is a lie the user cannot act on"
    );
    let prediction = projection
        .active_prediction
        .expect("the request must resolve into a prediction, not vanish");
    assert!(
        !prediction.provider_label.trim().is_empty(),
        "the panel must name the provider that answered; an unattributed \
         suggestion is exactly the surface claiming a capability it may not have"
    );
    let text = rendered_text(&after);
    assert!(
        text.iter().any(|line| line == &prediction.provider_label),
        "the projected provider label `{}` must be the one on screen, not a \
         substituted or omitted one; frame was {text:?}",
        prediction.provider_label
    );
}

// ─── Row 7: Delegate chat ───────────────────────────────────────────────────

/// A person must be able to type a Delegate chat turn and get a reply.
///
/// The `Send` control is disabled while the draft is empty, which this test
/// uses as its proof-of-landing: it asserts Send is *not* clickable before
/// typing and *is* clickable after. Without that check a mistargeted click and
/// a dropped keystroke would both still leave the final assertions reachable
/// through some other path, and the test would pass while testing nothing.
#[test]
fn a_delegate_chat_turn_can_be_typed_and_sent_from_the_rendered_ui() {
    let workspace = fixture("legion_desktop_delegate_chat");
    let mut app = DesktopEframeApp::new(runtime_with_open_file(workspace.path()));
    switch_mode(&mut app, "Delegate");

    let frame = app.run_headless_full_frame(full_frame_input(Vec::new()));
    let field = clickable_center(&frame, "Ask Delegate")
        .expect("Delegate must offer a chat field; row 7 needs somewhere to type");
    assert!(
        clickable_center(&frame, "Send").is_none(),
        "Send must be inert while the draft is empty, so that it becoming \
         clickable is proof the typed text landed"
    );

    let focused = click_at(&mut app, field);
    assert!(
        clickable_center(&focused, "Send").is_none(),
        "focusing the field alone must not enable Send"
    );
    let _ = app.run_headless_full_frame(full_frame_input(vec![egui::Event::Text(
        "explain this file".to_string(),
    )]));
    let typed = app.run_headless_full_frame(full_frame_input(Vec::new()));
    let send = clickable_center(&typed, "Send").expect(
        "Send must become clickable once the field holds text. If it did not, \
         the click missed the field or the keystrokes went nowhere — and every \
         assertion after this point would be meaningless.",
    );

    let after = click_at(&mut app, send);

    let messages = app
        .runtime_snapshot()
        .delegated_task_projection
        .chat_messages;
    assert!(
        messages
            .iter()
            .any(|message| message.content_label.contains("explain this file")),
        "the typed prompt must reach the projected transcript; messages were {:?}",
        messages
            .iter()
            .map(|message| message.content_label.as_str())
            .collect::<Vec<_>>()
    );
    // `send_delegate_chat` inserts "Streaming response…" immediately and only
    // replaces it once `poll_product_ai_stream` merges the worker result. A
    // non-empty assistant message therefore proves nothing on its own: the
    // placeholder is non-empty, so this assertion passed even if streaming
    // never completed. Poll until the placeholder is gone.
    let mut reply_label = String::new();
    let deadline = Instant::now() + Duration::from_secs(20);
    while Instant::now() < deadline {
        let _ = app.run_headless_full_frame(full_frame_input(Vec::new()));
        reply_label = app
            .runtime_snapshot()
            .delegated_task_projection
            .chat_messages
            .iter()
            .rev()
            .find(|message| message.role == legion_protocol::DelegatedTaskChatRole::Assistant)
            .map(|message| message.content_label.clone())
            .unwrap_or_default();
        if !reply_label.trim().is_empty() && !reply_label.contains(STREAMING_PLACEHOLDER) {
            break;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    assert!(
        !reply_label.trim().is_empty(),
        "an empty reply is a chat surface claiming a capability it does not have"
    );
    assert!(
        !reply_label.contains(STREAMING_PLACEHOLDER),
        "the reply never advanced past the streaming placeholder in 20s, so this asserts only that a placeholder was inserted. Final label was {reply_label:?}"
    );

    let text = rendered_text(&after);
    assert!(
        text.iter().any(|line| line.contains("explain this file")),
        "the transcript must be drawn, not only projected; frame was {text:?}"
    );
}

/// Delegate says why chat is unavailable rather than offering a Send that fails.
#[test]
fn delegate_chat_without_a_buffer_says_so_instead_of_offering_a_dead_send() {
    let workspace = fixture("legion_desktop_delegate_chat_no_buffer");
    let mut app = DesktopEframeApp::new(open_runtime(workspace.path()));
    switch_mode(&mut app, "Delegate");

    let frame = app.run_headless_full_frame(full_frame_input(Vec::new()));
    let text = rendered_text(&frame);
    assert!(
        text.iter()
            .any(|line| line == "Open a file to give Delegate something to talk about."),
        "chat needs an active buffer for context, so the rail must say that; \
         frame was {text:?}"
    );
    assert!(
        clickable_center(&frame, "Send").is_none(),
        "a Send that can only return `active buffer is not open` should not be \
         clickable in the first place"
    );
}

/// Checklist row 5 asks for a deterministic **proposal**, not ghost text.
///
/// `DesktopAction::ExecuteRailCommand` and its `StartAiProposal` translation
/// have existed since PKT-RAIL, and until this suite no renderer pushed either
/// one — so the proposal pipeline was unreachable from the UI. Inline
/// prediction was the only assist feature with a button, and it is a different
/// feature: ghost text at the cursor, not a reviewable proposal.
#[test]
fn an_assist_rail_command_produces_a_real_proposal() {
    let workspace = fixture("legion_desktop_assist_rail_proposal");
    let mut runtime = runtime_with_open_file(workspace.path());
    // Pin the deterministic route. `ProductAiProviderPreference::from_env`
    // honours `LEGION_AI_PROVIDER` in test builds too, so on a machine with a
    // reachable Ollama this "deterministic" test would start a real background
    // request -- making a standing workspace gate environment-dependent and
    // issuing a provider call nobody asked for.
    let _ = runtime.handle_action(DesktopAction::SetPreferredAiProvider {
        provider_id: "deterministic".to_string(),
    });
    let mut app = DesktopEframeApp::new(runtime);
    switch_mode(&mut app, "Assist");

    let opened = app.run_headless_full_frame(full_frame_input(Vec::new()));
    let before = app.runtime_snapshot().proposal_ledger_projection.rows.len();

    let explain = clickable_center(&opened, "Explain")
        .expect("the Assist rail must offer a proposal command, not only Predict");
    let after = click_at(&mut app, explain);

    // Bounded by wall clock, not by frame count. A frame-counted loop with no
    // sleep spins in microseconds, so it would never give a worker thread time
    // to run -- and "the deterministic route is fast" describes the route's
    // latency, not this test's bound. If the proposal pipeline ever grows a
    // worker, a frame count would flake on slow CI or pass by luck.
    let mut rows = before;
    let deadline = Instant::now() + Duration::from_secs(10);
    while Instant::now() < deadline && rows <= before {
        let _ = app.run_headless_full_frame(full_frame_input(Vec::new()));
        rows = app.runtime_snapshot().proposal_ledger_projection.rows.len();
        if rows <= before {
            std::thread::sleep(Duration::from_millis(50));
        }
    }

    // The row is only half the requirement. Checklist row 5 asks for a proposal
    // to appear, and a proposal a person cannot act on has not appeared --
    // `render_proposal_cards` was reachable only from the Workflows rail, so an
    // Assist user could start one and had nowhere to approve or reject it.
    let settled = app.run_headless_full_frame(full_frame_input(Vec::new()));
    assert!(
        clickable_center(&settled, "Approve").is_some()
            && clickable_center(&settled, "Reject").is_some(),
        "the proposal must be reviewable on the surface that created it; frame showed {:?}",
        rendered_text(&settled)
    );

    assert!(
        rows > before,
        "clicking a rail command produced no proposal (ledger stayed at {before} rows). The action reaches StartAiProposal through the bridge, so a click that changes nothing means the control never dispatched. Frame showed: {:?}",
        rendered_text(&after)
    );
}

/// The proposal a person just made must be the one Assist makes reviewable.
///
/// `render_proposal_cards` drew `rows.iter().take(4)` and
/// `proposal_ledger_projection` sorts oldest-first, so on a ledger that already
/// held four, the proposal just created fell into the "N more proposals" line --
/// static text carrying no Approve, Review or Reject. The Assist rail therefore
/// still had the defect it was changed to fix: somewhere to start a proposal
/// and nowhere to act on the one you started.
///
/// Every card carries the same title on the deterministic route, so the rows
/// cannot be told apart by their text. They are told apart by state instead:
/// the four older proposals are put through Approve first, which leaves them in
/// a terminal state whose card renders its lifecycle controls disabled. An
/// *enabled* `Approve` in the final frame can then only belong to the fifth,
/// newest, still-pending proposal.
#[test]
fn the_newest_proposal_stays_reviewable_once_the_ledger_is_full() {
    /// `render_proposal_cards` draws this many cards; the ledger has to exceed
    /// it or the ordering under test never applies.
    const CARD_LIMIT: usize = 4;

    let workspace = fixture("legion_desktop_assist_newest_proposal");
    let mut runtime = runtime_with_open_file(workspace.path());
    let _ = runtime.handle_action(DesktopAction::SetPreferredAiProvider {
        provider_id: "deterministic".to_string(),
    });
    let mut app = DesktopEframeApp::new(runtime);
    switch_mode(&mut app, "Assist");

    // Fill the ledger one proposal past the card limit, through the real rail
    // control. The click is retried rather than issued a fixed number of times:
    // the rail disables itself while the shared product-AI lane is busy, so a
    // click sent too early lands on a dead button and the ledger silently stays
    // where it was.
    let wanted = CARD_LIMIT + 1;
    let deadline = Instant::now() + Duration::from_secs(60);
    let mut rows = app.runtime_snapshot().proposal_ledger_projection.rows.len();
    while rows < wanted && Instant::now() < deadline {
        let frame = app.run_headless_full_frame(full_frame_input(Vec::new()));
        if let Some(explain) = enabled_clickable_center(&frame, "Explain") {
            let _ = click_at(&mut app, explain);
        }
        std::thread::sleep(Duration::from_millis(25));
        rows = app.runtime_snapshot().proposal_ledger_projection.rows.len();
    }
    assert_eq!(
        rows, wanted,
        "the fixture needs {wanted} proposals to exercise a full card list, got {rows}"
    );

    // Oldest-first ordering makes the last row the newest, which is also the row
    // the ledger selects by default.
    let ledger = app.runtime_snapshot().proposal_ledger_projection;
    let newest = ledger
        .rows
        .last()
        .expect("the ledger was just filled")
        .proposal_id;
    assert_eq!(
        ledger.selected_proposal_id,
        Some(newest),
        "the ledger selects the newest proposal, and the renderer follows that selection"
    );
    for row in ledger.rows.iter().filter(|row| row.proposal_id != newest) {
        app.handle_action(DesktopAction::ApproveProposal {
            proposal_id: row.proposal_id,
        })
        .expect("approving an existing pending proposal must be handled");
    }

    // Non-vacuity, in both directions. The four older proposals must really
    // render Approve as disabled, or "an enabled Approve means the newest is on
    // screen" is not an inference this test is entitled to draw -- and the
    // frame must still carry four Approve controls, or the assertion below
    // could be satisfied by a ledger that simply emptied.
    let settled = app.run_headless_full_frame(full_frame_input(Vec::new()));
    let approve_controls = settled
        .platform_output
        .accesskit_update
        .as_ref()
        .map(|update| {
            update
                .nodes
                .iter()
                .filter(|(_id, node)| node.label() == Some("Approve"))
                .count()
        })
        .unwrap_or_default();
    assert_eq!(
        approve_controls,
        CARD_LIMIT,
        "the panel should still be drawing a full set of {CARD_LIMIT} cards; frame showed {:?}",
        rendered_text(&settled)
    );
    assert!(
        clickable_center(&settled, "Approve").is_some(),
        "sanity: disabled Approve controls are still present as nodes"
    );

    assert!(
        enabled_clickable_center(&settled, "Approve").is_some(),
        "with the four older proposals already put through Approve, the only \
         card that can offer a live Approve is the newest one -- so this frame \
         rendered the four oldest and hid the proposal the person just created. \
         Frame showed {:?}",
        rendered_text(&settled)
    );
}
