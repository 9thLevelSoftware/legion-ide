//! Opt-in product startup coverage for a locally materialized Pyright adapter.
//!
//! Run explicitly with the retained archive and an operator-selected Node:
//! `LEGION_TEST_NODE_RUNTIME=<absolute-node> cargo test -p legion-app
//! --test python_app_startup -- --ignored --nocapture`.

use std::path::PathBuf;
use std::time::{Duration, Instant};

use legion_app::{AppComposition, AppSaveAllStatus};
use legion_editor::{TextEdit, TextPosition};
use legion_lsp::LanguageServerAdapterRegistry;
use legion_protocol::{
    CausalityId, LanguageId, LanguageToolingOperationKind, LspResultStatus,
    LspSessionLifecycleKind, PrincipalId, ProposalLifecycleAction, ProposalLifecycleCommand,
    ProposalLifecycleCommandReason, ProposalPayload, ProposalRequest, ProposalResponse,
    ProtocolDiagnosticSeverity, RedactionHint, TextCoordinate, TimestampMillis,
    WorkspaceTrustState,
};
use legion_ui::CommandDispatchIntent;

fn retained_archive() -> PathBuf {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../.superpowers/sdd/2026-09-04-full-product-completion/pyright-1.1.400.tgz");
    assert!(
        path.is_file(),
        "retained Pyright archive is required: {}",
        path.display()
    );
    path
}

fn selected_node() -> PathBuf {
    let path = PathBuf::from(std::env::var("LEGION_TEST_NODE_RUNTIME").expect(
        "LEGION_TEST_NODE_RUNTIME must select an absolute Node executable for this opt-in test",
    ));
    assert!(path.is_absolute(), "selected Node path must be absolute");
    assert!(path.is_file(), "selected Node executable must be a file");
    std::fs::canonicalize(path).expect("canonicalize selected Node executable")
}

fn wait_for_live(app: &mut AppComposition) {
    let deadline = Instant::now() + Duration::from_secs(45);
    loop {
        app.drain_lsp_session();
        let health = app.lsp_server_health_record();
        if health
            .as_ref()
            .is_some_and(|record| record.init_status == LspResultStatus::Fresh)
        {
            return;
        }
        let lifecycle = app.lsp_session_status_projection().lifecycle;
        assert_ne!(
            lifecycle,
            LspSessionLifecycleKind::Refused,
            "Pyright startup was refused: status={:?}, health={:?}, stderr={:?}",
            app.lsp_session_status_projection(),
            app.lsp_server_health_record(),
            app.lsp_session_log_projection(),
        );
        assert_ne!(
            lifecycle,
            LspSessionLifecycleKind::Failed,
            "Pyright startup failed: status={:?}, health={:?}, stderr={:?}",
            app.lsp_session_status_projection(),
            app.lsp_server_health_record(),
            app.lsp_session_log_projection(),
        );
        assert!(Instant::now() < deadline, "Pyright startup timed out");
        std::thread::sleep(Duration::from_millis(25));
    }
}

fn wait_for_native_proposal(
    app: &mut AppComposition,
    excluded: &[legion_protocol::ProposalId],
) -> legion_protocol::WorkspaceProposal {
    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
        app.drain_lsp_session();
        let operation = app
            .language_tooling_projection()
            .operations
            .iter()
            .rev()
            .find(|operation| {
                operation.kind == LanguageToolingOperationKind::RenameProposal
                    && operation
                        .proposal_id
                        .is_some_and(|proposal_id| !excluded.contains(&proposal_id))
            })
            .cloned();
        if let Some(proposal_id) = operation.and_then(|operation| operation.proposal_id)
            && let Some(proposal) = app.workspace_proposal_for_id(proposal_id)
        {
            return proposal;
        }
        assert!(
            Instant::now() < deadline,
            "Python rename proposal timed out: operations={:?}, health={:?}, session={:?}, stderr={:?}",
            app.language_tooling_projection().operations,
            app.lsp_server_health_record(),
            app.lsp_session_status_projection(),
            app.lsp_session_log_projection(),
        );
        std::thread::sleep(Duration::from_millis(25));
    }
}

fn cancel_native_proposal(app: &mut AppComposition, proposal: &legion_protocol::WorkspaceProposal) {
    let response = app
        .handle_proposal_request(ProposalRequest::Cancel(ProposalLifecycleCommand {
            proposal_id: proposal.proposal_id,
            principal: proposal.principal.clone(),
            capability: proposal.capability.clone(),
            correlation_id: proposal.correlation_id,
            causality_id: CausalityId(uuid::Uuid::now_v7()),
            reason: Some(ProposalLifecycleCommandReason::Cancellation(
                legion_protocol::ProposalCancellationReason::UserCancelled,
            )),
            diagnostics: Vec::new(),
            requested_at: TimestampMillis::now(),
            schema_version: 1,
            action: ProposalLifecycleAction::Cancel,
        }))
        .expect("cancel Python rename proposal");
    assert!(matches!(response, ProposalResponse::Cancelled { .. }));
}

fn python_assignment_problem(problem: &legion_protocol::LanguageProblemProjection) -> bool {
    problem
        .path
        .as_ref()
        .is_some_and(|path| path.0.ends_with("main.py"))
        && problem.code_label.as_deref() == Some("reportAssignmentType")
        && problem.range.is_some_and(|range| {
            range.start.line == 0
                && range.start.character == 13
                && range.end.line == 0
                && range.end.character == 20
        })
        && problem.severity == ProtocolDiagnosticSeverity::Error
        && problem.source_label.as_deref() == Some("Pyright")
}

fn python_assignment_error(problem: &legion_protocol::LanguageProblemProjection) -> bool {
    problem
        .path
        .as_ref()
        .is_some_and(|path| path.0.ends_with("main.py"))
        && problem.code_label.as_deref() == Some("reportAssignmentType")
        && problem.severity == ProtocolDiagnosticSeverity::Error
        && problem.source_label.as_deref() == Some("Pyright")
}

fn wait_for_python_problem(app: &mut AppComposition) -> legion_protocol::LanguageProblemProjection {
    let deadline = Instant::now() + Duration::from_secs(15);
    loop {
        app.drain_lsp_session();
        if let Some(problem) = app
            .language_tooling_projection()
            .problems
            .iter()
            .find(|problem| python_assignment_problem(problem))
        {
            return problem.clone();
        }
        assert!(
            Instant::now() < deadline,
            "Python assignment diagnostic did not arrive: problems={:?}, health={:?}, session={:?}, stderr={:?}",
            app.language_tooling_projection().problems,
            app.lsp_server_health_record(),
            app.lsp_session_status_projection(),
            app.lsp_session_log_projection(),
        );
        std::thread::sleep(Duration::from_millis(25));
    }
}

fn wait_for_python_problem_clear(app: &mut AppComposition) {
    let deadline = Instant::now() + Duration::from_secs(15);
    loop {
        app.drain_lsp_session();
        if !app
            .language_tooling_projection()
            .problems
            .iter()
            .any(python_assignment_error)
        {
            return;
        }
        assert!(
            Instant::now() < deadline,
            "Python assignment diagnostic did not clear: problems={:?}",
            app.language_tooling_projection().problems,
        );
        std::thread::sleep(Duration::from_millis(25));
    }
}

#[test]
#[ignore = "opt-in native Node + retained Pyright fixture"]
fn explicit_python_startup_is_lazy_live_and_restart_preserves_dirty_text() {
    let archive = retained_archive();
    let node = selected_node();
    let root = tempfile::tempdir().expect("temporary Python workspace");
    let source = root.path().join("main.py");
    std::fs::write(&source, "def greet(name):\n    return name\n").expect("seed Python source");
    let cache_root = root.path().join("language-cache");

    let adapter = LanguageServerAdapterRegistry::tier_two()
        .adapters_for_language(&LanguageId("python".to_string()))
        .into_iter()
        .find(|candidate| candidate.is_primary)
        .cloned()
        .expect("tier-two Python adapter");

    let mut app = AppComposition::new();
    app.open_workspace(
        root.path(),
        WorkspaceTrustState::Trusted,
        PrincipalId("python-startup-test".to_string()),
    )
    .expect("open workspace");
    app.configure_downloaded_language_server_local(adapter, archive, node, cache_root)
        .expect("configure local Pyright");

    app.open_file(source.to_string_lossy())
        .expect("open Python file");
    let buffer_id = app
        .active_buffer_id()
        .expect("active Python buffer after open");
    assert_eq!(
        app.lsp_session_status_projection().lifecycle,
        LspSessionLifecycleKind::Idle,
        "opening a Python file must not implicitly start the server"
    );

    app.dispatch_ui_intent(CommandDispatchIntent::LspStartSession)
        .expect("explicit start dispatch");
    wait_for_live(&mut app);
    let health = app.lsp_server_health_record().expect("live Python health");
    assert_eq!(health.language_id, LanguageId("python".to_string()));
    assert_eq!(health.init_status, LspResultStatus::Fresh);

    let completion_position = TextCoordinate {
        line: 1,
        character: 15,
        byte_offset: None,
        utf16_offset: None,
    };
    app.dispatch_ui_intent(CommandDispatchIntent::RequestCompletion {
        buffer_id,
        position: completion_position,
    })
    .expect("completion request dispatch");
    let completion_deadline = Instant::now() + Duration::from_secs(15);
    loop {
        app.drain_lsp_session();
        assert!(
            Instant::now() < completion_deadline,
            "Python completion did not arrive"
        );
        if !app.language_tooling_projection().completions.is_empty() {
            break;
        }
        std::thread::sleep(Duration::from_millis(25));
    }

    let hover_position = TextCoordinate {
        line: 0,
        character: 5,
        byte_offset: None,
        utf16_offset: None,
    };
    app.dispatch_ui_intent(CommandDispatchIntent::RequestHover {
        buffer_id,
        position: hover_position,
    })
    .expect("hover request dispatch");
    let hover_deadline = Instant::now() + Duration::from_secs(15);
    loop {
        app.drain_lsp_session();
        assert!(
            Instant::now() < hover_deadline,
            "Python hover did not arrive"
        );
        if app.language_tooling_projection().hover.is_some() {
            break;
        }
        std::thread::sleep(Duration::from_millis(25));
    }

    app.edit_active_buffer(TextEdit::insert(TextPosition::new(1, 15), " + \"!\""))
        .expect("make Python buffer dirty");
    let dirty_text = app
        .buffer_text_for_input(buffer_id)
        .expect("read dirty buffer text");
    assert!(dirty_text.contains("+ \"!\""));

    app.dispatch_ui_intent(CommandDispatchIntent::LspRestartSession)
        .expect("explicit restart dispatch");
    wait_for_live(&mut app);
    assert_eq!(
        app.buffer_text_for_input(buffer_id)
            .expect("dirty text after restart request"),
        dirty_text,
        "restart must not discard dirty editor text"
    );
}

#[test]
#[ignore = "opt-in native Node + retained Pyright fixture"]
fn native_python_rename_is_reviewable_cancelable_and_saves_cross_file_edit() {
    const LIB_BEFORE: &str = "def greet(name):\n    return name\n";
    const MAIN_BEFORE: &str = "from lib import greet\nmessage = greet(\"world\")\n";
    let archive = retained_archive();
    let node = selected_node();
    let root = tempfile::tempdir().expect("temporary Python workspace");
    let lib = root.path().join("lib.py");
    let main = root.path().join("main.py");
    std::fs::write(&lib, LIB_BEFORE).expect("seed lib");
    std::fs::write(&main, MAIN_BEFORE).expect("seed main");
    let mut app = AppComposition::new();
    app.open_workspace(
        root.path(),
        WorkspaceTrustState::Trusted,
        PrincipalId("python-native-rename".to_string()),
    )
    .expect("open workspace");
    let adapter = LanguageServerAdapterRegistry::tier_two()
        .adapters_for_language(&LanguageId("python".to_string()))
        .into_iter()
        .find(|candidate| candidate.is_primary)
        .cloned()
        .expect("tier-two Python adapter");
    app.configure_downloaded_language_server_local(
        adapter,
        archive,
        node,
        root.path().join("language-cache"),
    )
    .expect("configure local Pyright");
    app.open_file(lib.to_string_lossy()).expect("open lib");
    let lib_buffer = app.active_buffer_id().expect("lib buffer");
    app.open_file(main.to_string_lossy()).expect("open main");
    let main_buffer = app.active_buffer_id().expect("main buffer");
    app.dispatch_ui_intent(CommandDispatchIntent::LspStartSession)
        .expect("start Python");
    wait_for_live(&mut app);
    app.dispatch_ui_intent(CommandDispatchIntent::SwitchTab {
        buffer_id: lib_buffer,
    })
    .expect("activate Python definition");
    app.dispatch_ui_intent(CommandDispatchIntent::RequestRenameProposal {
        buffer_id: lib_buffer,
        position: TextCoordinate {
            line: 0,
            character: 4,
            byte_offset: None,
            utf16_offset: None,
        },
        new_name: "welcome".to_string(),
    })
    .expect("request Python rename");
    let proposal = wait_for_native_proposal(&mut app, &[]);
    let ProposalPayload::WorkspaceEdit(payload) = &proposal.payload else {
        panic!(
            "Python rename must produce a workspace edit: {:?}",
            proposal.payload
        );
    };
    assert!(
        payload.file_edits.len() >= 2,
        "rename must cover definition and use"
    );
    let annotation = payload
        .change_annotations
        .iter()
        .find(|annotation| annotation.id == "default")
        .expect("Pyright annotation remains attached to the proposal");
    assert!(
        annotation.needs_confirmation,
        "implicit server metadata requires explicit review"
    );
    assert!(
        annotation.targets.len() >= 2,
        "annotation must cover the cross-file edits"
    );
    assert!(
        annotation
            .description
            .as_deref()
            .is_some_and(|text| text.contains("omitted"))
    );
    assert_eq!(std::fs::read_to_string(&lib).unwrap(), LIB_BEFORE);
    assert_eq!(std::fs::read_to_string(&main).unwrap(), MAIN_BEFORE);
    cancel_native_proposal(&mut app, &proposal);

    app.dispatch_ui_intent(CommandDispatchIntent::SwitchTab {
        buffer_id: lib_buffer,
    })
    .expect("reactivate Python definition");
    app.dispatch_ui_intent(CommandDispatchIntent::RequestRenameProposal {
        buffer_id: lib_buffer,
        position: TextCoordinate {
            line: 0,
            character: 4,
            byte_offset: None,
            utf16_offset: None,
        },
        new_name: "welcome".to_string(),
    })
    .expect("request Python rename again");
    let proposal = wait_for_native_proposal(&mut app, &[proposal.proposal_id]);
    let response = app
        .approve_and_apply_rename_proposal(proposal.proposal_id)
        .expect("apply approved Python rename");
    assert!(matches!(response, ProposalResponse::Applied(_)));
    assert!(app.editor().text(lib_buffer).unwrap().contains("welcome"));
    assert!(app.editor().text(main_buffer).unwrap().contains("welcome"));
    assert_eq!(std::fs::read_to_string(&lib).unwrap(), LIB_BEFORE);
    assert_eq!(std::fs::read_to_string(&main).unwrap(), MAIN_BEFORE);
    let save = app.save_all().expect("save Python rename");
    assert_eq!(save.status, AppSaveAllStatus::Saved);
    assert!(
        std::fs::read_to_string(&lib)
            .unwrap()
            .contains("def welcome")
    );
    assert!(
        std::fs::read_to_string(&main)
            .unwrap()
            .contains("from lib import welcome")
    );
    assert!(
        std::fs::read_to_string(&main)
            .unwrap()
            .contains("welcome(\"world\")")
    );
}

#[test]
#[ignore = "opt-in native Node + retained Pyright fixture"]
fn native_python_rename_external_overwrite_rejects_without_partial_mutation() {
    const LIB_BEFORE: &str = "def greet(name):\n    return name\n";
    const MAIN_BEFORE: &str = "from lib import greet\nmessage = greet(\"world\")\n";
    let root = tempfile::tempdir().expect("temporary Python workspace");
    let lib = root.path().join("lib.py");
    let main = root.path().join("main.py");
    std::fs::write(&lib, LIB_BEFORE).expect("seed lib");
    std::fs::write(&main, MAIN_BEFORE).expect("seed main");
    let mut app = AppComposition::new();
    app.open_workspace(
        root.path(),
        WorkspaceTrustState::Trusted,
        PrincipalId("python-native-rename-conflict".to_string()),
    )
    .expect("open workspace");
    let adapter = LanguageServerAdapterRegistry::tier_two()
        .adapters_for_language(&LanguageId("python".to_string()))
        .into_iter()
        .find(|candidate| candidate.is_primary)
        .cloned()
        .expect("tier-two Python adapter");
    app.configure_downloaded_language_server_local(
        adapter,
        retained_archive(),
        selected_node(),
        root.path().join("language-cache"),
    )
    .expect("configure local Pyright");
    app.open_file(lib.to_string_lossy()).expect("open lib");
    let lib_buffer = app.active_buffer_id().expect("lib buffer");
    app.open_file(main.to_string_lossy()).expect("open main");
    let main_buffer = app.active_buffer_id().expect("main buffer");
    app.dispatch_ui_intent(CommandDispatchIntent::LspStartSession)
        .expect("start Python");
    wait_for_live(&mut app);
    app.dispatch_ui_intent(CommandDispatchIntent::SwitchTab {
        buffer_id: lib_buffer,
    })
    .expect("activate Python definition");
    app.dispatch_ui_intent(CommandDispatchIntent::RequestRenameProposal {
        buffer_id: lib_buffer,
        position: TextCoordinate {
            line: 0,
            character: 4,
            byte_offset: None,
            utf16_offset: None,
        },
        new_name: "welcome".to_string(),
    })
    .expect("request Python rename");
    let proposal = wait_for_native_proposal(&mut app, &[]);
    std::fs::write(&lib, "external definition\n").expect("external overwrite");
    let response = app.approve_and_apply_rename_proposal(proposal.proposal_id);
    assert!(response.is_err() || !matches!(response.unwrap(), ProposalResponse::Applied(_)));
    assert_eq!(
        std::fs::read_to_string(&lib).unwrap(),
        "external definition\n"
    );
    assert_eq!(std::fs::read_to_string(&main).unwrap(), MAIN_BEFORE);
    assert_eq!(app.editor().text(lib_buffer).unwrap(), LIB_BEFORE);
    assert_eq!(app.editor().text(main_buffer).unwrap(), MAIN_BEFORE);
}

#[test]
#[ignore = "opt-in native Node + retained Pyright fixture"]
fn native_python_diagnostic_clears_after_editor_replace_and_save() {
    const BEFORE: &str = "value: int = \"wrong\"\n";
    const AFTER: &str = "value: int = 42\n";
    let root = tempfile::tempdir().expect("temporary Python workspace");
    let source = root.path().join("main.py");
    std::fs::write(&source, BEFORE).expect("seed invalid Python source");
    let mut app = AppComposition::new();
    app.open_workspace(
        root.path(),
        WorkspaceTrustState::Trusted,
        PrincipalId("python-native-diagnostics".to_string()),
    )
    .expect("open workspace");
    let adapter = LanguageServerAdapterRegistry::tier_two()
        .adapters_for_language(&LanguageId("python".to_string()))
        .into_iter()
        .find(|candidate| candidate.is_primary)
        .cloned()
        .expect("tier-two Python adapter");
    app.configure_downloaded_language_server_local(
        adapter,
        retained_archive(),
        selected_node(),
        root.path().join("language-cache"),
    )
    .expect("configure local Pyright");
    app.open_file(source.to_string_lossy())
        .expect("open source");
    let buffer_id = app.active_buffer_id().expect("source buffer");
    app.dispatch_ui_intent(CommandDispatchIntent::LspStartSession)
        .expect("start Python");
    wait_for_live(&mut app);
    let problem = wait_for_python_problem(&mut app);
    assert_eq!(problem.message, "LSP error diagnostic");
    assert!(!problem.message.contains("cannot be assigned"));
    assert!(
        problem
            .redaction_hints
            .contains(&RedactionHint::MetadataOnly)
    );

    app.dispatch_ui_intent(CommandDispatchIntent::SetDirectedSelection {
        buffer_id,
        anchor: TextCoordinate {
            line: 0,
            character: 13,
            byte_offset: None,
            utf16_offset: None,
        },
        head: TextCoordinate {
            line: 0,
            character: 20,
            byte_offset: None,
            utf16_offset: None,
        },
    })
    .expect("select invalid Python value");
    app.dispatch_ui_intent(CommandDispatchIntent::ReplaceDirectedCarets {
        buffer_id,
        text: "42".to_string(),
    })
    .expect("replace invalid Python value through editor intent");
    assert_eq!(app.buffer_text_for_input(buffer_id).unwrap(), AFTER);
    wait_for_python_problem_clear(&mut app);
    let save = app.save_all().expect("save corrected Python source");
    assert_eq!(save.status, AppSaveAllStatus::Saved);
    assert_eq!(std::fs::read_to_string(&source).unwrap(), AFTER);
}
