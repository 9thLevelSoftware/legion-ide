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

/// Every capability string the read side gates on.
///
/// Kept here rather than imported so the test fails when the two drift, which
/// is the failure mode it exists to catch. If a new gated capability is added
/// to `lsp_reads.rs`, add it here too — and the test will tell you whether the
/// parser learned about it.
/// The one gated capability the mock deliberately does not advertise.
///
/// A mock that advertises everything cannot tell "records the key" from
/// "records the right answer": an implementation marking everything supported
/// would pass.
const UNADVERTISED_CAPABILITY: &str = "codeLensProvider";

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

    let mut launcher = legion_lsp::LspStdioLauncher::new();
    let mut session =
        RustAnalyzerSession::launch(config, &mut launcher).expect("launch should succeed");
    session
        .initialize("file:///workspace")
        .expect("initialize should succeed");

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
