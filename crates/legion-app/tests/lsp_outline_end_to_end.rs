//! The outline panel must show the server's symbols, not the index's.
//!
//! `issue_lsp_document_symbol_request` gates on `documentSymbolProvider`, and
//! until the initialize parser was taught to record that key the gate was
//! permanently shut: `textDocument/documentSymbol` was never sent to any
//! server, for any workspace, ever.
//!
//! Nothing noticed, for two reasons worth keeping in front of whoever reads
//! this next. The existing tests inject a health record
//! (`set_lsp_health_for_test`) and so assert that a request fires when the
//! capability is present — true — never that it was present, which was false.
//! And in use the panel still filled with rows, because a refused gate falls
//! back to the lexical index; the outline looked alive while the server was
//! never asked.
//!
//! So a test that merely asserts "the outline is not empty" would have passed
//! throughout the entire outage. These drive a live session against the mock
//! and assert on `inner_helper`, a symbol the mock invents and that exists in
//! no file on disk — the index cannot produce it, so its presence is proof the
//! bytes came off the wire.

use legion_protocol::{LspSessionLifecycleKind, PrincipalId, WorkspaceTrustState};
use legion_ui::CommandDispatchIntent;

mod lsp_mock;

/// The symbol only the server can know.
///
/// The mock nests it inside `main`; no fixture file ever contains the text, so
/// the lexical index has nothing to derive it from. Every assertion in this
/// file that distinguishes "the server answered" from "the fallback answered"
/// hangs on this name.
const SERVER_ONLY_SYMBOL: &str = "inner_helper";

/// A workspace with a manifest, a source file, and an open buffer.
///
/// The `Cargo.toml` is not decoration: the session refuses a root without one
/// (`lifecycle=Refused reason="no Cargo.toml in workspace root"`), which is the
/// product being careful rather than the test being superstitious.
///
/// The source deliberately mentions neither `inner_helper` nor anything like
/// it, so the index fallback cannot accidentally manufacture the very symbol
/// these tests use as proof of the round trip.
fn fixture_workspace() -> (
    tempfile::TempDir,
    legion_app::AppComposition,
    legion_protocol::BufferId,
) {
    let root = tempfile::tempdir().expect("tempdir");
    std::fs::write(
        root.path().join("Cargo.toml"),
        "[package]\nname = \"fixture\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
    )
    .expect("write manifest");
    std::fs::create_dir_all(root.path().join("src")).expect("mkdir src");
    let source = root.path().join("src").join("main.rs");
    std::fs::write(&source, "fn main() {\n    let value = 1;\n}\n").expect("write source");

    let mut app = legion_app::AppComposition::new();
    app.open_workspace(
        root.path(),
        WorkspaceTrustState::Trusted,
        PrincipalId("test".to_string()),
    )
    .expect("open workspace");
    app.open_file(source.to_string_lossy()).expect("open file");
    let buffer_id = app.active_buffer_id().expect("active buffer");
    (root, app, buffer_id)
}

/// Drains until the session reports `Live`, rather than sleeping a guessed
/// interval and hoping. Panics with the lifecycle and refusal reason, because
/// "the mock never came up" and "the mock came up and said no" are different
/// bugs that look identical from a bare timeout.
fn drive_until_live(app: &mut legion_app::AppComposition, mock_path: std::path::PathBuf) {
    app.force_lsp_start_with_server_path_for_test(mock_path);
    for _ in 0..600 {
        app.drain_lsp_session();
        if app.lsp_session_status_projection().lifecycle == LspSessionLifecycleKind::Live {
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    let status = app.lsp_session_status_projection();
    panic!(
        "the mock session never reached Live; lifecycle={:?} reason={:?}",
        status.lifecycle, status.failure_reason
    );
}

/// Drains until the outline contains the server-only symbol.
///
/// Waiting on "outline is non-empty" would be the wrong condition and would
/// hide the bug this file exists to catch: the index fallback populates the
/// outline immediately, so that loop exits on the first poll whether or not a
/// request was ever sent.
fn drain_for_server_outline(
    app: &mut legion_app::AppComposition,
) -> Vec<legion_protocol::LanguageOutlineSymbolProjection> {
    let mut outline = Vec::new();
    for _ in 0..600 {
        app.drain_lsp_session();
        outline = app.language_tooling_projection().outline;
        if outline.iter().any(|row| row.label == SERVER_ONLY_SYMBOL) {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    outline
}

/// The full round trip: gate open, request on the wire, response in the panel.
///
/// Before the parser fix this could not have reached its second assertion —
/// `issue_lsp_document_symbol_request` returned `false` and no bytes were
/// written to the server's stdin. P2.F1.T4 was marked `done` on the strength of
/// wiring that had never once executed.
#[test]
fn a_document_symbol_request_now_reaches_the_server_and_comes_back() {
    let mock_path = lsp_mock::mock_server_path().expect(
        "mock_lsp_server not found — run `cargo build -p legion-lsp --bin mock_lsp_server`",
    );
    let (_root, mut app, buffer_id) = fixture_workspace();
    drive_until_live(&mut app, mock_path);

    assert!(
        app.issue_lsp_document_symbol_request(buffer_id),
        "the capability gate refused the request; this is exactly where document \
         symbols died, silently, for every workspace"
    );

    let outline = drain_for_server_outline(&mut app);
    assert!(
        !outline.is_empty(),
        "the request was sent and nothing came back into the projection"
    );

    // The load-bearing assertion. `main` proves nothing — the index produces a
    // row with that label from the fixture source, so an outline containing
    // only `main` is indistinguishable from the outage.
    let helper = outline
        .iter()
        .find(|row| row.label == SERVER_ONLY_SYMBOL)
        .unwrap_or_else(|| {
            panic!(
                "the outline must be the server's answer, not the index's; \
                 {SERVER_ONLY_SYMBOL} appears in no file on disk, so the index cannot \
                 have produced it. Got {outline:?}"
            )
        });

    // The nested child is where tree handling goes wrong: the projection
    // flattens the symbol tree into a row list, and a flatten that drops or
    // mis-parents children is invisible against a mock that returns a flat
    // list. Pin the real behaviour — pre-order, parent first, depth carrying
    // the nesting the list no longer expresses.
    let main_row = outline
        .iter()
        .find(|row| row.label == "main")
        .expect("the parent symbol must survive flattening alongside its child");
    assert_eq!(main_row.depth, 0, "the root symbol sits at depth 0");
    assert_eq!(
        helper.depth, 1,
        "flattening must preserve nesting in `depth`; a child projected at depth 0 \
         would render as a sibling of the function that contains it"
    );
    let main_index = outline
        .iter()
        .position(|row| row.label == "main")
        .expect("parent present");
    let helper_index = outline
        .iter()
        .position(|row| row.label == SERVER_ONLY_SYMBOL)
        .expect("child present");
    assert!(
        main_index < helper_index,
        "the flattened order must be pre-order — a child before its parent makes \
         `depth` unreadable, since a row's parent is the nearest preceding shallower row"
    );

    // `children_omitted` is the truncation flag (the projector caps at 500
    // rows). Two symbols is not a truncation, and a flag stuck on would tell
    // the panel to draw an expander for children that are already listed.
    assert!(
        !main_row.children_omitted,
        "nothing was truncated, so no row may claim omitted children"
    );
    assert!(!helper.children_omitted, "the leaf has no children to omit");

    // Kind and range come from the response body, not from anything the client
    // could infer: `kind: 12` is LSP's Function, and the mock's selection range
    // for the child starts on line 1 — a line the fixture file does not even
    // define a symbol on.
    assert_eq!(
        helper.kind_label, "lsp.symbol.kind.12",
        "the symbol kind must be carried through from the response"
    );
    let range = helper.range.as_ref().expect(
        "the child's range must survive the projection, or the panel cannot navigate to it",
    );
    assert_eq!(
        range.start.line, 1,
        "the range must be the server's, not a placeholder"
    );
}

/// The user-facing path, and the reason the outage was invisible.
///
/// `RefreshOutline` answers immediately from the lexical index so the panel is
/// never blank while the server thinks, then overwrites it when the response
/// lands. That fallback is what made a permanently closed gate look like a
/// working feature for the entire life of the bug, so it is worth asserting
/// both halves: the index answer really does lack the server's symbol, and the
/// server's answer really does replace it.
#[test]
fn the_outline_command_replaces_the_index_answer_with_the_servers() {
    let mock_path = lsp_mock::mock_server_path().expect("mock_lsp_server not found");
    let (_root, mut app, buffer_id) = fixture_workspace();
    drive_until_live(&mut app, mock_path);

    let immediate = app
        .dispatch_ui_intent(CommandDispatchIntent::RefreshOutline { buffer_id })
        .expect("refresh outline dispatch");
    let index_outline = match immediate {
        legion_app::AppCommandOutcome::LanguageToolingUpdated(projection) => projection.outline,
        other => panic!("expected a language tooling projection, got {other:?}"),
    };
    assert!(
        index_outline
            .iter()
            .all(|row| row.label != SERVER_ONLY_SYMBOL),
        "the synchronous answer is the index's and cannot contain {SERVER_ONLY_SYMBOL}; \
         if it does, this test has lost its ability to tell the two sources apart"
    );

    let outline = drain_for_server_outline(&mut app);
    assert!(
        outline.iter().any(|row| row.label == SERVER_ONLY_SYMBOL),
        "the server's answer never replaced the index's; the command path is where \
         users meet this feature, and before the parser fix it stopped at the index \
         forever. Got {outline:?}"
    );
}
