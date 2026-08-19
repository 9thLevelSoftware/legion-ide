//! Every capability the read side gates on must survive `initialize`.
//!
//! `issue_lsp_read` refuses to send a request unless the server advertised the
//! matching capability, and the answer comes from the health record built while
//! parsing the initialize response. Those are two lists in two files, and
//! nothing tied them together: a request could gate on a capability the parser
//! never recorded, and the gate would then be permanently closed.
//!
//! It failed exactly that way. The parser recorded four capabilities —
//! `hoverProvider`, `definitionProvider`, `completionProvider` and
//! `diagnosticProvider` — while the read side gated on nine. References,
//! document symbols, inlay hints and code lenses could never reach the server.
//!
//! The reason it went unnoticed is worth more than the bug. The tests covering
//! those requests inject a health record directly
//! (`set_lsp_health_for_test(health_with_caps(&[("referencesProvider", true)]))`),
//! so they assert that a request fires when the capability is present, which is
//! true, and never that the capability is present in the first place, which was
//! false. The fixture taught the code a contract the product did not honour.
//!
//! And it was invisible in use: when the gate refuses, the caller falls back to
//! the lexical index, so the panel fills with rows. They were the index's
//! answers rather than rust-analyzer's, which looks like a working feature
//! until you ask why references in another crate never appear.
//!
//! This test drives the real `initialize` against the mock server, which now
//! advertises every gated capability, and asserts the parser kept them all.

use legion_app::language::{RustAnalyzerDiscovery, RustAnalyzerLaunchConfig, RustAnalyzerSession};
use legion_protocol::{LanguageId, LanguageServerId};

mod lsp_mock;

/// The gated capability withheld for the negative control.
///
/// The mock advertises everything by default, because the round-trip tests need
/// a server that can answer every read. `LEGION_MOCK_WITHHOLD_CAPABILITY` drops
/// exactly one, which is what lets this suite check the other half of the
/// property: an implementation marking everything supported would pass a test
/// that only ever sees advertised capabilities.
const UNADVERTISED_CAPABILITY: &str = "codeLensProvider";

/// Every capability string the read side gates on.
///
/// Kept here rather than imported so the test fails when the two drift, which
/// is the failure mode it exists to catch. If a new gated capability is added
/// to `lsp_reads.rs`, add it here too — and the test will tell you whether the
/// parser learned about it.
const GATED_CAPABILITIES: &[&str] = &[
    "hoverProvider",
    "definitionProvider",
    "completionProvider",
    "referencesProvider",
    "documentSymbolProvider",
    "inlayHintProvider",
    "codeLensProvider",
    "callHierarchyProvider",
];

#[test]
fn initialize_records_every_capability_the_read_side_gates_on() {
    let mock_path = lsp_mock::mock_server_path().expect(
        "mock_lsp_server not found — run `cargo build -p legion-lsp --bin mock_lsp_server`",
    );

    let config = RustAnalyzerLaunchConfig {
        discovery: RustAnalyzerDiscovery {
            configured_path: Some(mock_path),
            ..Default::default()
        },
        supervisor: lsp_mock::mock_supervisor_config(),
        server_id: LanguageServerId(11),
        language_id: LanguageId("rust".to_string()),
    };

    let mut launcher = legion_lsp::LspStdioLauncher::new();
    let mut session =
        RustAnalyzerSession::launch(config, &mut launcher).expect("launch should succeed");
    session
        .initialize("file:///workspace")
        .expect("initialize should succeed");

    let health = session.health();
    let recorded: std::collections::BTreeSet<&str> = health
        .capabilities
        .iter()
        .map(|capability| capability.capability.as_str())
        .collect();

    let missing: Vec<&str> = GATED_CAPABILITIES
        .iter()
        .copied()
        .filter(|capability| !recorded.contains(capability))
        .collect();

    assert!(
        missing.is_empty(),
        "the read side gates on these capabilities and `initialize` never recorded them, so \
         those requests can never fire: {missing:?}. Recorded: {recorded:?}"
    );
}

/// The gate opening is only worth something if the request completes.
///
/// Recording `referencesProvider` and refusing to send the request are two
/// different failures with the same symptom, and the tests that inject a health
/// record could never tell them apart because they never involved a server.
/// This drives the real thing end to end: a live session against the mock, the
/// request issued through the capability gate, the response drained, and the
/// server's locations landing in the projection.
///
/// Before the parser was fixed this could not have got past the second step —
/// `issue_lsp_references_request` returned `false` and no request was sent.
#[test]
fn a_references_request_now_reaches_the_server_and_comes_back() {
    let mock_path = lsp_mock::mock_server_path().expect("mock_lsp_server not found");
    let root = tempfile::tempdir().expect("tempdir");
    // A real workspace, because the session refuses one without a manifest —
    // `lifecycle=Refused reason="no Cargo.toml in workspace root"`, which is the
    // product being careful rather than the test being wrong.
    std::fs::write(
        root.path().join("Cargo.toml"),
        "[package]\nname = \"fixture\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
    )
    .expect("write manifest");
    std::fs::create_dir_all(root.path().join("src")).expect("mkdir src");
    let source = root.path().join("src").join("main.rs");
    std::fs::write(&source, "fn main() {}\n").expect("write");

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

    // Startup is asynchronous: drain until the session reports Live rather than
    // sleeping for a guessed interval.
    let mut became_live = false;
    for _ in 0..600 {
        app.drain_lsp_session();
        if app.lsp_session_status_projection().lifecycle
            == legion_protocol::LspSessionLifecycleKind::Live
        {
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

    let position = legion_protocol::TextCoordinate {
        line: 0,
        character: 3,
        byte_offset: None,
        utf16_offset: None,
    };
    assert!(
        app.issue_lsp_references_request(buffer_id, position, true),
        "the capability gate refused the request; before the parser fix this is \
         exactly where references died, silently, for every workspace"
    );

    let mut locations = Vec::new();
    for _ in 0..600 {
        app.drain_lsp_session();
        locations = app.language_tooling_projection().references;
        if !locations.is_empty() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }

    assert!(
        !locations.is_empty(),
        "the request was sent and nothing came back into the projection"
    );
    // The mock answers with two locations, one of them in a file the caret is
    // not in. Asserting on that one specifically is what distinguishes the
    // server's answer from the lexical index's: the index cannot know about
    // `caller.rs`, which does not exist on disk.
    assert!(
        locations
            .iter()
            .any(|location| location.label.contains("caller.rs")
                || location
                    .path
                    .as_ref()
                    .is_some_and(|path| path.0.contains("caller.rs"))),
        "the rows must be the server's answer, not the index's; got {locations:?}"
    );
}

#[test]
fn a_recorded_capability_reports_what_the_server_actually_said() {
    // Recording the key is only half of it. A capability recorded as
    // `supported: false` when the server advertised it would close the gate
    // just as firmly, and the failure would look identical from the outside.
    let mock_path = lsp_mock::mock_server_path().expect("mock_lsp_server not found");

    let config = RustAnalyzerLaunchConfig {
        discovery: RustAnalyzerDiscovery {
            configured_path: Some(mock_path),
            ..Default::default()
        },
        supervisor: lsp_mock::mock_supervisor_config(),
        server_id: LanguageServerId(12),
        language_id: LanguageId("rust".to_string()),
    };

    // Safety: this test process sets the variable before launching the mock and
    // the mock reads it at startup. Single-threaded within this test, and no
    // other test in this binary launches a server while it is set.
    unsafe {
        std::env::set_var("LEGION_MOCK_WITHHOLD_CAPABILITY", UNADVERTISED_CAPABILITY);
    }
    let mut launcher = legion_lsp::LspStdioLauncher::new();
    let mut session =
        RustAnalyzerSession::launch(config, &mut launcher).expect("launch should succeed");
    session
        .initialize("file:///workspace")
        .expect("initialize should succeed");
    unsafe {
        std::env::remove_var("LEGION_MOCK_WITHHOLD_CAPABILITY");
    }

    let health = session.health();
    let supported = |name: &str| {
        health
            .capabilities
            .iter()
            .find(|capability| capability.capability == name)
            .map(|capability| capability.supported)
    };

    for name in GATED_CAPABILITIES {
        if *name == UNADVERTISED_CAPABILITY {
            continue;
        }
        assert_eq!(
            supported(name),
            Some(true),
            "the mock advertises {name}; recording it as unsupported closes the gate just as \
             firmly as not recording it at all"
        );
    }

    // The negative control. Without it this test would pass on an
    // implementation that marks every capability supported regardless of what
    // the server said — which would open every gate and fire requests at a
    // server that cannot serve them.
    assert_eq!(
        supported(UNADVERTISED_CAPABILITY),
        Some(false),
        "{UNADVERTISED_CAPABILITY} is absent from the mock's initialize response, so it must be \
         recorded as present-and-unsupported rather than assumed available"
    );
}
