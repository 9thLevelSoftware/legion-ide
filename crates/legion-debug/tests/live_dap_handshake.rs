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

    // Deliberately not asserting `initialized_event`. Per the DAP sequence the
    // adapter sends `initialized` when it is ready for configuration — after
    // `launch`/`attach`, not after `initialize`. The in-tree fake sends it
    // early, and requiring it here is what let this suite stay green while
    // every real adapter hung: the handshake waited for an event that was not
    // coming until a request it had not made yet.
    assert_eq!(outcome.adapter_type, "legion-fake");
    assert_eq!(outcome.lifecycle_state, DapLifecycleState::Launching);
    assert!(outcome.metadata_summary.contains("live=true"));

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

#[test]
fn a_silent_adapter_times_out_instead_of_hanging() {
    // The bug this guards is why a wrong protocol expectation cost sixty
    // minutes per CI job instead of failing in seconds. Every wait was a
    // blocking `read_from` inside a `while Instant::now() < deadline` loop, so
    // the deadline was only consulted *between* frames and could never fire
    // while waiting for one. A timeout that cannot fire is worse than no
    // timeout: it hid whether the adapter was slow, silent, or gone.
    let mut session =
        LiveDapSession::spawn(adapter_path(), &["--silent".to_string()], "legion-fake")
            .expect("spawn silent fake adapter");

    let started = std::time::Instant::now();
    let result = session.initialize_handshake(Duration::from_millis(300));
    let elapsed = started.elapsed();

    assert!(
        result.is_err(),
        "a silent adapter cannot complete a handshake"
    );
    assert!(
        elapsed < Duration::from_secs(5),
        "the deadline must be enforced while waiting for a frame, not only \
         between frames; took {elapsed:?}"
    );
}

/// The same session against an adapter that answers `launch` the way real ones do.
///
/// Real adapters defer the `launch` response until configuration is finished:
/// they emit `initialized`, wait for the client's breakpoints and
/// `configurationDone`, and only then answer `launch`. lldb-dap does exactly
/// this, and the client used to block on the launch response before sending
/// `configurationDone` — a deadlock in which both sides were waiting for the
/// other and neither was misbehaving.
///
/// It cost a fifteen-second timeout per CI run on macOS to observe, reported as
/// `no DAP frame within 15s; adapter still running; adapter stderr: <empty>` —
/// alive, silent and blameless. The in-tree fake answered `launch` immediately,
/// so the whole suite stayed green against a sequence no real adapter follows.
/// That is the second time this fake's convenience has hidden a real defect;
/// the first was `initialized` at handshake time.
///
/// `--defer-launch-response` makes the fake behave correctly,
/// so the deadlock is reproducible in-tree rather than only on a runner.
#[test]
fn launch_completes_against_an_adapter_that_defers_its_launch_response() {
    let mut session = LiveDapSession::spawn(
        adapter_path(),
        &["--defer-launch-response".to_string()],
        "legion-fake",
    )
    .expect("spawn fake adapter");

    session
        .initialize_handshake(Duration::from_secs(5))
        .expect("initialize handshake");

    // Three seconds is deliberately tight. The bug this pins does not produce a
    // wrong answer, it produces no answer at all, so a generous timeout would
    // turn a deadlock into a slow test rather than a failing one.
    let outcome = session
        .launch_until_stopped("/tmp/legion-fake-program", Duration::from_secs(3))
        .expect("launch must complete against an adapter that defers its launch response");
    assert_eq!(
        outcome.reason, "entry",
        "the stop that follows configurationDone is the launch's answer"
    );

    session
        .disconnect_and_wait(Duration::from_secs(2))
        .expect("disconnect");
}

/// The ordering lldb-dap actually uses, captured from a runner transcript.
///
/// `LEGION_DAP_TRACE_FRAMES=1` on the Ubuntu dogfood produced this sequence:
///
/// ```text
/// --> launch
/// <-- response(launch)          the answer comes straight back
/// <-- event(process)
/// <-- event(initialized)        and only then is it ready for configuration
/// --> configurationDone
/// <-- response(configurationDone)
/// <-- event(stopped)
/// ```
///
/// Every frame the adapter owed arrived, and the client still timed out — because
/// waiting for `initialized` necessarily reads the launch response on the way,
/// and that response was being discarded. The later wait then looked for a frame
/// that had already come and gone.
///
/// This is the ordering that broke the one platform that worked, and no in-tree
/// test covered it: the fake sent `initialized` during the handshake instead,
/// which is a third ordering that resembles neither real adapter.
#[test]
fn launch_completes_when_the_adapter_answers_before_it_is_ready() {
    // Modes travel as arguments so this test cannot disturb any other running
    // beside it — process-wide `set_var` broke exactly that way one revision
    // ago, taking a sibling test with it.
    let mut session = LiveDapSession::spawn(
        adapter_path(),
        &["--initialized-after-launch".to_string()],
        "legion-fake",
    )
    .expect("spawn fake adapter");

    let outcome = session
        .initialize_handshake(Duration::from_secs(5))
        .expect("initialize handshake");
    assert!(
        !outcome.initialized_event,
        "this adapter withholds `initialized` until after launch, which is the \
         whole point of the ordering under test"
    );

    // Tight on purpose: the failure this pins is a wait that never ends, so a
    // generous timeout would turn a hang into a slow pass.
    let stop = session
        .launch_until_stopped("/tmp/legion-fake-program", Duration::from_secs(3))
        .expect("launch must complete when the launch response precedes `initialized`");
    assert_eq!(stop.reason, "entry");

    session
        .disconnect_and_wait(Duration::from_secs(2))
        .expect("disconnect");
}
