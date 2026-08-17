//! P2.F1.T4: a read result reaches the projection field it belongs in.
//!
//! `drain_lsp_session` looks at a result's tag and picks one of seven ingest
//! methods. Getting that wrong does not fail anything — the response is parsed
//! by the wrong projector, produces nothing, and the feature is silently dead.
//! That is exactly the failure this task's stop condition exists to prevent, so
//! each of the four newly-routed kinds is checked against a payload only its
//! own projector can read.

use legion_app::AppComposition;
use legion_app::language::{LspReadKind, LspReadOutcome, LspRequestTag, LspWorkerResult};
use legion_protocol::{
    BufferId, LspCapabilitySummary, LspResultStatus, LspServerBinaryProvenance,
    LspServerHealthRecord, PrincipalId, WorkspaceTrustState,
};

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
            "referencesProvider",
            "documentSymbolProvider",
            "inlayHintProvider",
            "codeLensProvider",
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

/// A workspace with one open Rust file and a Live session whose worker channel
/// the caller holds.
fn app_with_live_session() -> (AppComposition, BufferId, tempfile::TempDir) {
    let root = tempfile::tempdir().expect("tempdir");
    std::fs::write(root.path().join("Cargo.toml"), "[package]\nname = \"x\"\n").expect("manifest");
    let src = root.path().join("main.rs");
    std::fs::write(&src, "fn main() {\n    let x = 1;\n}\n").expect("source");

    let mut app = AppComposition::new();
    app.open_workspace(
        root.path(),
        WorkspaceTrustState::Trusted,
        PrincipalId("test".to_string()),
    )
    .expect("open workspace");
    app.open_file(src.to_string_lossy()).expect("open file");
    let buffer_id = app.active_buffer_id().expect("active buffer");
    (app, buffer_id, root)
}

/// Feed one read result through the real drain path and return the projection.
fn drain_one(
    app: &mut AppComposition,
    buffer_id: BufferId,
    kind: LspReadKind,
    result: serde_json::Value,
) {
    let sender = app.inject_lsp_result_sender_for_test(live_health());
    let issued_snapshot = app
        .current_snapshot_id_for_test(buffer_id)
        .expect("snapshot id");
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

#[test]
fn a_references_result_lands_in_the_projection_as_locations() {
    let (mut app, buffer_id, _root) = app_with_live_session();
    let payload = serde_json::json!([
        {
            "uri": "file:///w/main.rs",
            "range": {
                "start": { "line": 1, "character": 8 },
                "end": { "line": 1, "character": 9 }
            }
        }
    ]);
    drain_one(&mut app, buffer_id, LspReadKind::References, payload);

    let projection = app.language_tooling_projection();
    assert!(
        !projection.references.is_empty(),
        "a references response must reach `references`, got {projection:?}"
    );
}

#[test]
fn a_document_symbol_result_lands_in_the_projection_as_an_outline() {
    let (mut app, buffer_id, _root) = app_with_live_session();
    let payload = serde_json::json!([
        {
            "name": "main",
            "kind": 12,
            "range": {
                "start": { "line": 0, "character": 0 },
                "end": { "line": 2, "character": 1 }
            },
            "selectionRange": {
                "start": { "line": 0, "character": 3 },
                "end": { "line": 0, "character": 7 }
            }
        }
    ]);
    drain_one(&mut app, buffer_id, LspReadKind::Outline, payload);

    let projection = app.language_tooling_projection();
    assert!(
        projection.outline.iter().any(|item| item.label == "main"),
        "a documentSymbol response must reach `outline`, got {:?}",
        projection.outline
    );
}

#[test]
fn an_inlay_hint_result_lands_in_the_projection_attributed_to_the_server() {
    let (mut app, buffer_id, _root) = app_with_live_session();
    let payload = serde_json::json!([
        {
            "position": { "line": 1, "character": 9 },
            "label": ": i32",
            "kind": 1
        }
    ]);
    drain_one(&mut app, buffer_id, LspReadKind::InlayHints, payload);

    let projection = app.language_tooling_projection();
    let hint = projection
        .inlay_hints
        .first()
        .expect("an inlayHint response must reach `inlay_hints`");
    assert!(hint.label.contains("i32"));
    assert_eq!(
        hint.source_label, "rust",
        "the hint must name the session that produced it, not a hardcoded server"
    );
}

#[test]
fn a_code_lens_result_lands_in_the_projection_and_carries_its_command() {
    let (mut app, buffer_id, _root) = app_with_live_session();
    // Shaped like a rust-analyzer runnable, which is how runnables reach the
    // editor: a lens whose command runs a single test.
    let payload = serde_json::json!([
        {
            "range": {
                "start": { "line": 0, "character": 0 },
                "end": { "line": 0, "character": 7 }
            },
            "command": {
                "title": "\u{25b6} Run",
                "command": "rust-analyzer.runSingle"
            }
        }
    ]);
    drain_one(&mut app, buffer_id, LspReadKind::CodeLens, payload);

    let projection = app.language_tooling_projection();
    let lens = projection
        .code_lenses
        .first()
        .expect("a codeLens response must reach `code_lenses`");
    assert!(
        lens.title.contains("Run"),
        "the runnable's title must survive projection, got {:?}",
        lens.title
    );
}

/// A result tagged for one feature must not be ingested as another.
///
/// The negative half of the routing claim: feeding an outline-shaped payload
/// under the references tag must leave the outline empty. Without this, four
/// tests that each only assert their own field would still pass if the routing
/// collapsed every kind into a single ingest.
#[test]
fn a_result_does_not_leak_into_a_feature_it_was_not_tagged_for() {
    let (mut app, buffer_id, _root) = app_with_live_session();
    let outline_shaped = serde_json::json!([
        {
            "name": "main",
            "kind": 12,
            "range": {
                "start": { "line": 0, "character": 0 },
                "end": { "line": 2, "character": 1 }
            },
            "selectionRange": {
                "start": { "line": 0, "character": 3 },
                "end": { "line": 0, "character": 7 }
            }
        }
    ]);
    drain_one(&mut app, buffer_id, LspReadKind::References, outline_shaped);

    let projection = app.language_tooling_projection();
    assert!(
        projection.outline.is_empty(),
        "a payload tagged References must not populate the outline"
    );
}

/// The two new intents dispatch, and the projection records what was asked.
///
/// Reachability is only half of the acceptance — the intent has to arrive at
/// app authority and be recorded as the operation it is, or the operations list
/// (which is what the health surface reads) will not know the feature exists.
#[test]
fn the_new_refresh_intents_dispatch_and_are_recorded_as_their_own_operations() {
    use legion_protocol::LanguageToolingOperationKind;
    use legion_ui::CommandDispatchIntent;

    let (mut app, buffer_id, _root) = app_with_live_session();

    app.dispatch_ui_intent(CommandDispatchIntent::RefreshInlayHints { buffer_id })
        .expect("inlay-hint refresh dispatches");
    app.dispatch_ui_intent(CommandDispatchIntent::RefreshCodeLenses { buffer_id })
        .expect("code-lens refresh dispatches");

    let operations = app.language_tooling_projection().operations;
    for expected in [
        LanguageToolingOperationKind::InlayHints,
        LanguageToolingOperationKind::CodeLens,
    ] {
        assert!(
            operations.iter().any(|op| op.kind == expected),
            "{expected:?} must appear in the operations list, got {operations:?}"
        );
    }
}
