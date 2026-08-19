//! Inlay hints and code lenses, proven over the wire rather than assumed.
//!
//! Both reads gate on a capability (`inlayHintProvider`, `codeLensProvider`)
//! that the `initialize` parser did not record until commit 1a2b9c7. For as
//! long as it did not, `issue_lsp_inlay_hint_request` and
//! `issue_lsp_code_lens_request` returned `false` and no request ever left the
//! process — the two features were wired, marked done, and dead.
//!
//! The reason nobody noticed is the thing worth guarding against here. The
//! existing coverage injects a health record with `set_lsp_health_for_test`,
//! so it asserts a request fires *when the capability is present*, which is
//! true, and never that the capability was present, which was false. And in
//! the product it looked like it worked: a refused gate falls back to the
//! lexical index, so the hint gutter and the lens row still fill with
//! something.
//!
//! That fallback is exactly why the assertions below are written against
//! content the index cannot possibly produce. `MockInferredType` is an
//! inferred type — the lexical index does no type inference — and
//! `mock_lens_target` is a runnable the index has no concept of. If either
//! string reaches the projection, it came off the socket.
//!
//! Requires the mock server binary:
//!   cargo build -p legion-lsp --bin mock_lsp_server -j 6

mod lsp_mock;

use legion_protocol::{BufferId, LspSessionLifecycleKind};

/// How long the polling loops will wait, in 10ms drains. Generous because
/// process launch plus handshake is the slow part on a cold Windows box, and a
/// flaky timeout here would read as a broken feature.
const DRAIN_ATTEMPTS: usize = 600;

/// A live session against the mock, with the workspace kept alive.
///
/// The `TempDir` must outlive the app: dropping it removes the manifest and
/// the source file out from under a session that is still reading them.
struct MockWorkspace {
    app: legion_app::AppComposition,
    buffer_id: BufferId,
    _root: tempfile::TempDir,
}

/// Opens a real workspace, starts the mock server, and waits for `Live`.
///
/// A `Cargo.toml` is not optional: without one the session refuses with
/// `lifecycle=Refused reason="no Cargo.toml in workspace root"`, which is the
/// product being careful rather than the fixture being wrong.
fn live_mock_workspace() -> MockWorkspace {
    let mock_path = lsp_mock::mock_server_path().expect(
        "mock_lsp_server not found — run `cargo build -p legion-lsp --bin mock_lsp_server -j 6`",
    );
    let root = tempfile::tempdir().expect("tempdir");
    std::fs::write(
        root.path().join("Cargo.toml"),
        "[package]\nname = \"fixture\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
    )
    .expect("write manifest");
    std::fs::create_dir_all(root.path().join("src")).expect("mkdir src");
    let source = root.path().join("src").join("main.rs");
    // Two lines with a binding, so the mock's hint position (line 1,
    // character 12) lands inside a document that plausibly has one. The mock
    // does not read the source, but a hint pointing past the end of the file
    // would be a fixture that proves less than it appears to.
    std::fs::write(&source, "fn main() {\n    let value = 1;\n}\n").expect("write source");

    let mut app = legion_app::AppComposition::new();
    app.open_workspace(
        root.path(),
        legion_protocol::WorkspaceTrustState::Trusted,
        legion_protocol::PrincipalId("test".to_string()),
    )
    .expect("open workspace");
    app.open_file(source.to_string_lossy()).expect("open file");
    let buffer_id = app.active_buffer_id().expect("active buffer");

    app.force_lsp_start_with_server_path_for_test(mock_path);

    // Startup is asynchronous. Drain until the session reports Live rather
    // than sleeping for a guessed interval — a fixed sleep is how this suite
    // would become flaky on a loaded machine.
    let mut became_live = false;
    for _ in 0..DRAIN_ATTEMPTS {
        app.drain_lsp_session();
        if app.lsp_session_status_projection().lifecycle == LspSessionLifecycleKind::Live {
            became_live = true;
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    let status = app.lsp_session_status_projection();
    assert!(
        became_live,
        "the mock session never reached Live; lifecycle={:?} reason={:?}",
        status.lifecycle, status.failure_reason
    );

    MockWorkspace {
        app,
        buffer_id,
        _root: root,
    }
}

/// Inlay hints: request sent, answer parsed, inferred type in the projection.
///
/// Two failures share one symptom and this separates them. The gate refusing
/// (the bug: `issue_lsp_inlay_hint_request` returns `false`, nothing is sent)
/// and the response never being routed back into `inlay_hints` both leave the
/// panel showing the lexical index's guesses. The first assertion catches the
/// former; the label assertion catches the latter and proves the row is the
/// server's.
#[test]
fn an_inlay_hint_request_reaches_the_server_and_its_label_lands_in_the_projection() {
    let mut fixture = live_mock_workspace();
    let app = &mut fixture.app;

    // Inlay hints are requested per-range; the app does not yet plumb the
    // viewport down here, so a refresh asks for the whole document.
    let range = app
        .whole_document_utf16_range_for_test(fixture.buffer_id)
        .expect("whole-document range for an open buffer");

    assert!(
        app.issue_lsp_inlay_hint_request(fixture.buffer_id, range),
        "the capability gate refused the request. Before `initialize` recorded \
         inlayHintProvider this returned false for every server on every \
         workspace, and the gutter filled from the lexical index instead"
    );

    let mut hints = Vec::new();
    for _ in 0..DRAIN_ATTEMPTS {
        app.drain_lsp_session();
        hints = app.language_tooling_projection().inlay_hints;
        if hints
            .iter()
            .any(|hint| hint.label.contains("MockInferredType"))
        {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }

    // `: MockInferredType` is an *inferred type*. The lexical index tokenizes;
    // it does not do type inference and could not have produced this string,
    // so its presence is proof the answer travelled over the wire.
    let hint = hints
        .iter()
        .find(|hint| hint.label.contains("MockInferredType"))
        .unwrap_or_else(|| {
            panic!("the server's inlay hint never reached the projection; got {hints:?}")
        });

    assert_eq!(
        hint.label, ": MockInferredType",
        "the label must survive projection verbatim, leading colon and space \
         included — the editor renders it inline and trimming it would change \
         what the user sees"
    );
    // The mock sends `kind: 1` (Type). Losing the kind would make every hint
    // render as a parameter hint, which is a different visual affordance.
    assert_eq!(
        hint.kind_label, "lsp.inlay.kind.1",
        "the LSP hint kind must be carried through, got {:?}",
        hint.kind_label
    );
    // Position, so a hint that arrived cannot be rendered at the wrong place.
    assert_eq!(hint.position.line, 1, "hint position line");
    assert_eq!(hint.position.character, 12, "hint position character");
    // Attribution: `lsp_read_source_label` names the language of the session
    // that answered. A hint that claims no source cannot be told apart from an
    // index-produced one downstream.
    assert_eq!(
        hint.source_label, "rust",
        "hints must be attributed to the answering session"
    );
}

/// Code lenses: request sent, answer parsed, and the runnable pinned.
///
/// This is the more consequential of the two. `ActivateLanguageCodeLens` hands
/// `command_label` to `TerminalWorkflow::launch`, so the field has to hold the
/// cargo invocation the lens describes, not `rust-analyzer.runSingle` — a
/// command id no shell can run. Getting that wrong means the Runnables row in
/// the UI executes the wrong thing, and until this fix no test could have
/// noticed because the request never left the process.
#[test]
fn a_code_lens_request_reaches_the_server_and_projects_a_runnable_cargo_command() {
    let mut fixture = live_mock_workspace();
    let app = &mut fixture.app;

    assert!(
        app.issue_lsp_code_lens_request(fixture.buffer_id),
        "the capability gate refused the request. Before `initialize` recorded \
         codeLensProvider this returned false, so runnables never existed — \
         rust-analyzer publishes Run/Debug as code lenses and nothing else \
         carries them"
    );

    let mut lenses = Vec::new();
    for _ in 0..DRAIN_ATTEMPTS {
        app.drain_lsp_session();
        lenses = app.language_tooling_projection().code_lenses;
        if lenses
            .iter()
            .any(|lens| lens.title.contains("mock_lens_target"))
        {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }

    // `mock_lens_target` is a runnable target the mock invented. The lexical
    // index has no notion of runnables and no way to name one, so a lens
    // carrying this string can only have come from the server.
    let lens = lenses
        .iter()
        .find(|lens| lens.title.contains("mock_lens_target"))
        .unwrap_or_else(|| {
            panic!("the server's code lens never reached the projection; got {lenses:?}")
        });

    assert_eq!(
        lens.title, "Run mock_lens_target",
        "the lens title is what the user clicks; it must be the server's"
    );

    // The load-bearing assertion of this file. `command_label` is what gets
    // handed to a terminal. The mock's lens carries
    // `rust-analyzer.runSingle` with `cargoArgs: ["test", "mock_lens_target"]`
    // and empty `executableArgs`, and the projection must rebuild that into
    // the command line those arguments describe. An equality check, not a
    // `contains`: a label that merely mentions the target while still saying
    // `rust-analyzer.runSingle` would pass a looser test and fail in a shell.
    assert_eq!(
        lens.command_label, "cargo test mock_lens_target",
        "a runnable lens must project the cargo invocation, never the LSP \
         command id — `ActivateLanguageCodeLens` hands this string to the \
         terminal"
    );
    // With no `executableArgs` there must be no dangling `--` separator; an
    // empty trailing separator is the kind of thing that survives review and
    // breaks the day someone appends to the string.
    assert!(
        !lens.command_label.contains("--"),
        "no executableArgs means no `--` separator, got {:?}",
        lens.command_label
    );

    // The marker `ActivateLanguageCodeLens` gates on before it will launch
    // anything. A correct command line under the wrong kind label is a
    // runnable the UI refuses to run.
    assert_eq!(
        lens.kind_label, "lsp.codelens.runnable",
        "the runnable marker is what makes the lens launchable"
    );
    assert_eq!(
        lens.source_label, "rust",
        "lenses must be attributed to the answering session"
    );
    // The lens has to point somewhere for the editor to anchor the row.
    let range = lens
        .range
        .as_ref()
        .expect("a code lens must carry its range");
    assert_eq!(range.start.line, 0, "lens range start line");
    assert_eq!(range.end.character, 7, "lens range end character");
}
