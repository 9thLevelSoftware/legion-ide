//! Opt-in startup coverage for the pinned local TypeScript bundle.
//!
//! Run explicitly with the retained archives and an operator-selected Node:
//! `LEGION_TEST_NODE_RUNTIME=<absolute-node> cargo test -p legion-app
//! --test typescript_app_startup -- --ignored --nocapture`.

use std::path::PathBuf;
use std::time::{Duration, Instant};

use legion_app::{AppComposition, AppSaveAllStatus};
use legion_editor::{TextEdit, TextPosition};
use legion_lsp::LanguageServerAdapterRegistry;
use legion_protocol::{
    CausalityId, FileId, LanguageId, LanguageToolingOperationKind, LspResultStatus,
    LspSessionLifecycleKind, PrincipalId, ProposalLifecycleAction, ProposalLifecycleCommand,
    ProposalLifecycleCommandReason, ProposalLifecycleState, ProposalPayload, ProposalRequest,
    ProposalResponse, TextCoordinate, TimestampMillis, WorkspaceTrustState,
};
use legion_ui::CommandDispatchIntent;

fn retained_bundle(name: &str) -> PathBuf {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../.superpowers/sdd/2026-09-04-full-product-completion/typescript-bundle")
        .join(name);
    assert!(
        path.is_file(),
        "retained TypeScript archive is required: {}",
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

fn wait_for_live(app: &mut AppComposition, language: &str) {
    let deadline = Instant::now() + Duration::from_secs(45);
    loop {
        app.drain_lsp_session();
        let health = app.lsp_server_health_record();
        if health.as_ref().is_some_and(|record| {
            record.init_status == LspResultStatus::Fresh
                && record.language_id == LanguageId(language.to_string())
        }) {
            return;
        }
        let lifecycle = app.lsp_session_status_projection().lifecycle;
        assert_ne!(
            lifecycle,
            LspSessionLifecycleKind::Refused,
            "TypeScript startup was refused: status={:?}, health={:?}, stderr={:?}",
            app.lsp_session_status_projection(),
            app.lsp_server_health_record(),
            app.lsp_session_log_projection(),
        );
        assert_ne!(
            lifecycle,
            LspSessionLifecycleKind::Failed,
            "TypeScript startup failed: status={:?}, health={:?}, stderr={:?}",
            app.lsp_session_status_projection(),
            app.lsp_server_health_record(),
            app.lsp_session_log_projection(),
        );
        assert!(Instant::now() < deadline, "TypeScript startup timed out");
        std::thread::sleep(Duration::from_millis(25));
    }
}

fn wait_for_completion(app: &mut AppComposition, expected_label: &str) {
    let deadline = Instant::now() + Duration::from_secs(15);
    loop {
        app.drain_lsp_session();
        assert!(
            Instant::now() < deadline,
            "TypeScript completion did not arrive"
        );
        if app
            .language_tooling_projection()
            .completions
            .iter()
            .any(|completion| completion.label == expected_label)
        {
            return;
        }
        std::thread::sleep(Duration::from_millis(25));
    }
}

fn wait_for_hover(app: &mut AppComposition, file_id: FileId) {
    let deadline = Instant::now() + Duration::from_secs(15);
    loop {
        app.drain_lsp_session();
        assert!(Instant::now() < deadline, "TypeScript hover did not arrive");
        if app
            .language_tooling_projection()
            .hover
            .as_ref()
            .is_some_and(|hover| hover.file_id == Some(file_id))
        {
            return;
        }
        std::thread::sleep(Duration::from_millis(25));
    }
}

fn wait_for_problem(app: &mut AppComposition, file_id: FileId, needle: &str) {
    let deadline = Instant::now() + Duration::from_secs(15);
    loop {
        app.drain_lsp_session();
        if app
            .language_tooling_projection()
            .problems
            .iter()
            .any(|problem| {
                problem.file_id == Some(file_id)
                    && (problem.message.contains(needle)
                        || problem.code_label.as_deref() == Some(needle))
            })
        {
            return;
        }
        assert!(
            Instant::now() < deadline,
            "TypeScript diagnostic {needle:?} did not arrive: problems={:?}, health={:?}, session={:?}, stderr={:?}",
            app.language_tooling_projection().problems,
            app.lsp_server_health_record(),
            app.lsp_session_status_projection(),
            app.lsp_session_log_projection(),
        );
        std::thread::sleep(Duration::from_millis(25));
    }
}

fn wait_for_problem_to_clear(app: &mut AppComposition, file_id: FileId, needle: &str) {
    let deadline = Instant::now() + Duration::from_secs(15);
    loop {
        app.drain_lsp_session();
        if !app
            .language_tooling_projection()
            .problems
            .iter()
            .any(|problem| {
                problem.file_id == Some(file_id)
                    && (problem.message.contains(needle)
                        || problem.code_label.as_deref() == Some(needle))
            })
        {
            return;
        }
        assert!(
            Instant::now() < deadline,
            "TypeScript diagnostic {needle:?} did not clear: problems={:?}",
            app.language_tooling_projection().problems,
        );
        std::thread::sleep(Duration::from_millis(25));
    }
}

fn wait_for_code_action_candidate(
    app: &mut AppComposition,
    title_fragment: &str,
) -> legion_protocol::LanguageCodeActionProjection {
    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
        app.drain_lsp_session();
        if let Some(candidate) = app
            .language_tooling_projection()
            .code_action_candidates
            .iter()
            .find(|candidate| {
                candidate
                    .title
                    .to_ascii_lowercase()
                    .contains(title_fragment)
                    && candidate.has_edit
                    && candidate.disabled_reason.is_none()
            })
            .cloned()
        {
            return candidate;
        }
        assert!(
            Instant::now() < deadline,
            "TypeScript code-action candidate did not arrive: candidates={:?}, problems={:?}, health={:?}, session={:?}, stderr={:?}",
            app.language_tooling_projection().code_action_candidates,
            app.language_tooling_projection().problems,
            app.lsp_server_health_record(),
            app.lsp_session_status_projection(),
            app.lsp_session_log_projection(),
        );
        std::thread::sleep(Duration::from_millis(25));
    }
}

fn wait_for_native_proposal(
    app: &mut AppComposition,
    operation_kind: LanguageToolingOperationKind,
    excluded_proposal_ids: &[legion_protocol::ProposalId],
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
                operation.kind == operation_kind
                    && operation
                        .proposal_id
                        .is_some_and(|proposal_id| !excluded_proposal_ids.contains(&proposal_id))
            })
            .cloned();
        if let Some(proposal_id) = operation.and_then(|operation| operation.proposal_id)
            && let Some(proposal) = app.workspace_proposal_for_id(proposal_id)
        {
            return proposal;
        }
        assert!(
            Instant::now() < deadline,
            "native {operation_kind:?} proposal timed out: operations={:?}, health={:?}, session={:?}, stderr={:?}",
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
        .expect("cancel native proposal");
    assert!(
        matches!(response, ProposalResponse::Cancelled { .. }),
        "expected cancelled proposal, got {response:?}"
    );
}

fn approve_native_proposal(
    app: &mut AppComposition,
    proposal: &legion_protocol::WorkspaceProposal,
) {
    let response = app
        .handle_proposal_request(ProposalRequest::Approve(ProposalLifecycleCommand {
            proposal_id: proposal.proposal_id,
            principal: proposal.principal.clone(),
            capability: proposal.capability.clone(),
            correlation_id: proposal.correlation_id,
            causality_id: CausalityId(uuid::Uuid::now_v7()),
            reason: None,
            diagnostics: Vec::new(),
            requested_at: TimestampMillis::now(),
            schema_version: 1,
            action: ProposalLifecycleAction::Approve,
        }))
        .expect("approve native proposal");
    assert!(
        matches!(response, ProposalResponse::Approved(_)),
        "expected approved proposal, got {response:?}"
    );
}

fn configure_bundle(app: &mut AppComposition) {
    app.dispatch_ui_intent(CommandDispatchIntent::ConfigureTypeScriptToolchain {
        server_archive: retained_bundle("typescript-language-server-6.0.0.tgz")
            .to_string_lossy()
            .into_owned(),
        compiler_archive: retained_bundle("typescript-6.0.3.tgz")
            .to_string_lossy()
            .into_owned(),
        node_executable: selected_node().to_string_lossy().into_owned(),
    })
    .expect("configure retained local TypeScript bundle through normal intent");
}

#[test]
#[ignore = "opt-in native Node + retained TypeScript fixtures"]
fn explicit_typescript_startup_is_lazy_live_and_restart_preserves_dirty_text() {
    let root = tempfile::tempdir().expect("temporary TypeScript workspace");
    let source = root.path().join("main.ts");
    std::fs::write(
        &source,
        "const typed: string = 42;\nfunction greet(name: string): string { return `hello ${name}`; }\nconst result = greet(\"world\");\n",
    )
    .expect("seed TypeScript source");
    let typescript_adapter = LanguageServerAdapterRegistry::tier_two()
        .adapters_for_language(&LanguageId("typescript".to_string()))
        .into_iter()
        .find(|candidate| candidate.is_primary)
        .cloned()
        .expect("tier-two TypeScript adapter");
    assert_eq!(
        typescript_adapter.language_id,
        LanguageId("typescript".to_string())
    );

    let mut app = AppComposition::new();
    app.open_workspace(
        root.path(),
        WorkspaceTrustState::Trusted,
        PrincipalId("typescript-startup-test".to_string()),
    )
    .expect("open workspace");
    configure_bundle(&mut app);
    app.open_file(source.to_string_lossy())
        .expect("open TypeScript file");
    let buffer_id = app
        .active_buffer_id()
        .expect("active TypeScript buffer after open");
    let file_id = app
        .active_file_id()
        .expect("active TypeScript file after open");
    assert_eq!(
        app.lsp_session_status_projection().lifecycle,
        LspSessionLifecycleKind::Idle,
        "opening a TypeScript file must not implicitly start the server"
    );

    app.dispatch_ui_intent(CommandDispatchIntent::LspStartSession)
        .expect("explicit TypeScript start dispatch");
    wait_for_live(&mut app, "typescript");

    app.dispatch_ui_intent(CommandDispatchIntent::RequestCompletion {
        buffer_id,
        position: TextCoordinate {
            line: 2,
            character: 20,
            byte_offset: None,
            utf16_offset: None,
        },
    })
    .expect("completion request dispatch");
    wait_for_completion(&mut app, "greet");

    app.dispatch_ui_intent(CommandDispatchIntent::RequestHover {
        buffer_id,
        position: TextCoordinate {
            line: 1,
            character: 10,
            byte_offset: None,
            utf16_offset: None,
        },
    })
    .expect("hover request dispatch");
    wait_for_hover(&mut app, file_id);

    let diagnostics_deadline = Instant::now() + Duration::from_secs(15);
    while !app
        .language_tooling_projection()
        .problems
        .iter()
        .any(|problem| {
            problem.file_id == Some(file_id)
                && (problem.code_label.as_deref() == Some("2322")
                    || problem.message.contains("2322"))
        })
    {
        app.drain_lsp_session();
        assert!(
            Instant::now() < diagnostics_deadline,
            "TypeScript diagnostics for intentional TS2322 did not arrive: problems={:?}, health={:?}, session={:?}, stderr={:?}",
            app.language_tooling_projection().problems,
            app.lsp_server_health_record(),
            app.lsp_session_status_projection(),
            app.lsp_session_log_projection(),
        );
        std::thread::sleep(Duration::from_millis(25));
    }

    app.edit_active_buffer(TextEdit::insert(TextPosition::new(2, 20), " + result"))
        .expect("make TypeScript buffer dirty");
    let dirty_text = app
        .buffer_text_for_input(buffer_id)
        .expect("read dirty TypeScript buffer text");
    assert!(dirty_text.contains("+ result"));

    app.dispatch_ui_intent(CommandDispatchIntent::LspRestartSession)
        .expect("explicit TypeScript restart dispatch");
    wait_for_live(&mut app, "typescript");
    assert_eq!(
        app.buffer_text_for_input(buffer_id)
            .expect("dirty text after TypeScript restart request"),
        dirty_text,
        "restart must not discard dirty editor text"
    );

    let javascript = root.path().join("main.js");
    std::fs::write(
        &javascript,
        "function javascriptOnlyGreeting(name) { return `hello ${name}`; }\nconst value = javascriptOnlyGree;\n",
    )
        .expect("seed JavaScript source");
    app.open_file(javascript.to_string_lossy())
        .expect("open JavaScript file");
    let javascript_buffer = app
        .active_buffer_id()
        .expect("active JavaScript buffer after open");
    let javascript_file = app
        .active_file_id()
        .expect("active JavaScript file after open");
    app.dispatch_ui_intent(CommandDispatchIntent::LspRestartSession)
        .expect("restart for JavaScript language");
    wait_for_live(&mut app, "javascript");
    app.dispatch_ui_intent(CommandDispatchIntent::RequestCompletion {
        buffer_id: javascript_buffer,
        position: TextCoordinate {
            line: 1,
            character: 17,
            byte_offset: None,
            utf16_offset: None,
        },
    })
    .expect("JavaScript completion request dispatch");
    wait_for_completion(&mut app, "javascriptOnlyGreeting");
    app.dispatch_ui_intent(CommandDispatchIntent::RequestHover {
        buffer_id: javascript_buffer,
        position: TextCoordinate {
            line: 0,
            character: 10,
            byte_offset: None,
            utf16_offset: None,
        },
    })
    .expect("JavaScript hover request dispatch");
    wait_for_hover(&mut app, javascript_file);
}

#[test]
#[ignore = "opt-in native Node + retained TypeScript fixtures"]
fn native_typescript_rename_is_reviewable_and_applies_only_after_approval() {
    const LIB_BEFORE: &str = "export function greet(name: string): string { return name; }\n";
    const MAIN_BEFORE: &str =
        "import { greet } from \"./lib\";\nconst message = greet(\"world\");\n";
    let root = tempfile::tempdir().expect("temporary TypeScript workspace");
    let lib = root.path().join("lib.ts");
    let main = root.path().join("main.ts");
    std::fs::write(&lib, LIB_BEFORE).expect("seed lib");
    std::fs::write(&main, MAIN_BEFORE).expect("seed main");

    let mut app = AppComposition::new();
    app.open_workspace(
        root.path(),
        WorkspaceTrustState::Trusted,
        PrincipalId("typescript-native-rename".to_string()),
    )
    .expect("open workspace");
    configure_bundle(&mut app);
    app.open_file(lib.to_string_lossy()).expect("open lib");
    let lib_buffer = app.active_buffer_id().expect("lib buffer");
    app.open_file(main.to_string_lossy()).expect("open main");
    let main_buffer = app.active_buffer_id().expect("main buffer");
    app.dispatch_ui_intent(CommandDispatchIntent::LspStartSession)
        .expect("start TypeScript");
    wait_for_live(&mut app, "typescript");
    app.dispatch_ui_intent(CommandDispatchIntent::SwitchTab {
        buffer_id: lib_buffer,
    })
    .expect("activate exported definition");

    app.dispatch_ui_intent(CommandDispatchIntent::RequestRenameProposal {
        buffer_id: lib_buffer,
        position: TextCoordinate {
            line: 0,
            character: 18,
            byte_offset: None,
            utf16_offset: None,
        },
        new_name: "welcome".to_string(),
    })
    .expect("request native rename");
    let proposal =
        wait_for_native_proposal(&mut app, LanguageToolingOperationKind::RenameProposal, &[]);
    let ProposalPayload::WorkspaceEdit(payload) = &proposal.payload else {
        panic!(
            "native rename must produce a workspace edit: {:?}",
            proposal.payload
        );
    };
    assert!(
        payload.file_edits.len() >= 2,
        "native rename should cover definition and use: {:?}",
        payload.file_edits
    );
    assert_eq!(
        app.shell_projection_snapshot("native rename")
            .expect("rename shell projection")
            .proposal_ledger_projection
            .rows
            .into_iter()
            .find(|row| row.proposal_id == proposal.proposal_id)
            .expect("rename proposal ledger row")
            .lifecycle
            .state,
        ProposalLifecycleState::Previewed
    );
    assert_eq!(std::fs::read_to_string(&lib).expect("lib disk"), LIB_BEFORE);
    assert_eq!(
        std::fs::read_to_string(&main).expect("main disk"),
        MAIN_BEFORE
    );
    cancel_native_proposal(&mut app, &proposal);

    app.dispatch_ui_intent(CommandDispatchIntent::SwitchTab {
        buffer_id: lib_buffer,
    })
    .expect("reactivate exported definition");
    app.dispatch_ui_intent(CommandDispatchIntent::RequestRenameProposal {
        buffer_id: lib_buffer,
        position: TextCoordinate {
            line: 0,
            character: 18,
            byte_offset: None,
            utf16_offset: None,
        },
        new_name: "welcome".to_string(),
    })
    .expect("request native rename again");
    let proposal = wait_for_native_proposal(
        &mut app,
        LanguageToolingOperationKind::RenameProposal,
        &[proposal.proposal_id],
    );
    let response = app
        .approve_and_apply_rename_proposal(proposal.proposal_id)
        .expect("approve and apply native rename");
    assert!(
        matches!(response, ProposalResponse::Applied(_)),
        "native rename apply response: {response:?}"
    );
    assert_eq!(std::fs::read_to_string(&lib).expect("lib disk"), LIB_BEFORE);
    assert_eq!(
        std::fs::read_to_string(&main).expect("main disk"),
        MAIN_BEFORE
    );
    assert!(
        app.editor()
            .text(main_buffer)
            .expect("main editor text")
            .contains("welcome")
    );
    assert!(
        app.editor()
            .text(lib_buffer)
            .expect("lib editor text")
            .contains("welcome")
    );

    // Approval mutates editor buffers only.  The normal save authority must
    // persist both sides of a cross-file rename together.
    let save = app.save_all().expect("save approved native rename");
    assert_eq!(save.status, AppSaveAllStatus::Saved);
    assert_eq!(
        std::fs::read_to_string(&lib).expect("saved lib disk"),
        "export function welcome(name: string): string { return name; }\n"
    );
    assert_eq!(
        std::fs::read_to_string(&main).expect("saved main disk"),
        "import { welcome } from \"./lib\";\nconst message = welcome(\"world\");\n"
    );
}

#[test]
#[ignore = "opt-in native Node + retained TypeScript fixtures"]
fn native_typescript_rename_conflict_does_not_partially_apply() {
    const LIB_BEFORE: &str = "export function greet(name: string): string { return name; }\n";
    const MAIN_BEFORE: &str =
        "import { greet } from \"./lib\";\nconst message = greet(\"world\");\n";
    let root = tempfile::tempdir().expect("temporary TypeScript workspace");
    let lib = root.path().join("lib.ts");
    let main = root.path().join("main.ts");
    std::fs::write(&lib, LIB_BEFORE).expect("seed lib");
    std::fs::write(&main, MAIN_BEFORE).expect("seed main");
    let mut app = AppComposition::new();
    app.open_workspace(
        root.path(),
        WorkspaceTrustState::Trusted,
        PrincipalId("typescript-native-rename-conflict".to_string()),
    )
    .expect("open workspace");
    configure_bundle(&mut app);
    app.open_file(lib.to_string_lossy()).expect("open lib");
    let lib_buffer = app.active_buffer_id().expect("lib buffer");
    app.open_file(main.to_string_lossy()).expect("open main");
    let main_buffer = app.active_buffer_id().expect("main buffer");
    app.dispatch_ui_intent(CommandDispatchIntent::LspStartSession)
        .expect("start TypeScript");
    wait_for_live(&mut app, "typescript");
    app.dispatch_ui_intent(CommandDispatchIntent::SwitchTab {
        buffer_id: lib_buffer,
    })
    .expect("activate exported definition");
    app.dispatch_ui_intent(CommandDispatchIntent::RequestRenameProposal {
        buffer_id: lib_buffer,
        position: TextCoordinate {
            line: 0,
            character: 18,
            byte_offset: None,
            utf16_offset: None,
        },
        new_name: "welcome".to_string(),
    })
    .expect("request native rename");
    let proposal =
        wait_for_native_proposal(&mut app, LanguageToolingOperationKind::RenameProposal, &[]);

    // Change a named target after preview.  The save/apply authority must
    // reject the stale proposal before mutating either editor target.
    std::fs::write(&lib, "external definition\n").expect("external overwrite");
    let response = app.approve_and_apply_rename_proposal(proposal.proposal_id);
    assert!(
        response.is_err()
            || !matches!(
                response.expect("rename conflict response"),
                ProposalResponse::Applied(_)
            ),
        "external conflict must prevent rename apply"
    );
    assert_eq!(
        std::fs::read_to_string(&lib).expect("conflicted lib"),
        "external definition\n"
    );
    assert_eq!(
        std::fs::read_to_string(&main).expect("main disk after conflict"),
        MAIN_BEFORE
    );
    assert_eq!(
        app.editor().text(main_buffer).expect("main editor text"),
        MAIN_BEFORE
    );
    assert_eq!(
        app.editor().text(lib_buffer).expect("lib editor text"),
        LIB_BEFORE
    );
    assert!(
        !app.editor()
            .text(main_buffer)
            .expect("main editor text")
            .contains("welcome")
    );
}

#[test]
#[ignore = "opt-in native Node + retained TypeScript fixtures"]
fn native_typescript_formatting_is_reviewable_before_disk_mutation() {
    const BEFORE: &str = "export const value={answer:42};\n";
    let root = tempfile::tempdir().expect("temporary TypeScript workspace");
    let source = root.path().join("format.ts");
    std::fs::write(&source, BEFORE).expect("seed unformatted TypeScript");
    let mut app = AppComposition::new();
    app.open_workspace(
        root.path(),
        WorkspaceTrustState::Trusted,
        PrincipalId("typescript-native-format".to_string()),
    )
    .expect("open workspace");
    configure_bundle(&mut app);
    app.open_file(source.to_string_lossy())
        .expect("open source");
    let buffer_id = app.active_buffer_id().expect("format buffer");
    app.dispatch_ui_intent(CommandDispatchIntent::LspStartSession)
        .expect("start TypeScript");
    wait_for_live(&mut app, "typescript");
    app.dispatch_ui_intent(CommandDispatchIntent::RequestFormattingProposal { buffer_id })
        .expect("request native formatting");
    let proposal = wait_for_native_proposal(
        &mut app,
        LanguageToolingOperationKind::FormattingProposal,
        &[],
    );
    assert!(
        matches!(
            &proposal.payload,
            ProposalPayload::WorkspaceEdit(_)
                | ProposalPayload::TextEdit(_)
                | ProposalPayload::FormatFile(_)
        ),
        "native formatting must produce an edit proposal: {:?}",
        proposal.payload
    );
    assert_eq!(
        app.shell_projection_snapshot("native formatting preview")
            .expect("format shell projection")
            .proposal_ledger_projection
            .rows
            .into_iter()
            .find(|row| row.proposal_id == proposal.proposal_id)
            .expect("format proposal ledger row")
            .lifecycle
            .state,
        ProposalLifecycleState::Previewed
    );
    assert_eq!(
        std::fs::read_to_string(&source).expect("format disk"),
        BEFORE,
        "format preview must not mutate disk"
    );
    cancel_native_proposal(&mut app, &proposal);
    let state = app
        .shell_projection_snapshot("native formatting")
        .expect("shell projection")
        .proposal_ledger_projection
        .rows
        .into_iter()
        .find(|row| row.proposal_id == proposal.proposal_id)
        .expect("format proposal ledger row")
        .lifecycle
        .state;
    assert_eq!(state, ProposalLifecycleState::Cancelled);

    app.dispatch_ui_intent(CommandDispatchIntent::RequestFormattingProposal { buffer_id })
        .expect("request native formatting again");
    let proposal = wait_for_native_proposal(
        &mut app,
        LanguageToolingOperationKind::FormattingProposal,
        &[proposal.proposal_id],
    );
    approve_native_proposal(&mut app, &proposal);
    let applied = app
        .handle_proposal_request(ProposalRequest::Apply(proposal.clone()))
        .expect("apply approved native formatting");
    assert!(
        matches!(applied, ProposalResponse::Applied(_)),
        "native formatting apply response: {applied:?}"
    );
    let formatted = app
        .editor()
        .text(buffer_id)
        .expect("formatted editor text")
        .to_string();
    assert_ne!(formatted, BEFORE, "formatting must produce a real edit");
    let save = app.save_all().expect("save approved formatting");
    assert_eq!(save.status, AppSaveAllStatus::Saved);
    assert_eq!(
        std::fs::read_to_string(&source).expect("formatted disk"),
        formatted,
        "normal save authority must persist the approved format exactly"
    );
}

#[test]
#[ignore = "opt-in native Node + retained TypeScript fixtures"]
fn native_typescript_missing_import_code_action_is_reviewable_and_applies_after_approval() {
    const LIB_BEFORE: &str = "export function knownlib(): string { return \"known\"; }\n";
    const APP_BEFORE: &str = "const value = knownlib();\n";
    let root = tempfile::tempdir().expect("temporary TypeScript workspace");
    let lib = root.path().join("lib.ts");
    let app_source = root.path().join("app.ts");
    std::fs::write(&lib, LIB_BEFORE).expect("seed exported library");
    std::fs::write(&app_source, APP_BEFORE).expect("seed missing-import app");

    let mut app = AppComposition::new();
    app.open_workspace(
        root.path(),
        WorkspaceTrustState::Trusted,
        PrincipalId("typescript-native-code-action".to_string()),
    )
    .expect("open workspace");
    configure_bundle(&mut app);
    app.open_file(lib.to_string_lossy()).expect("open library");
    app.open_file(app_source.to_string_lossy())
        .expect("open app source");
    let app_buffer = app.active_buffer_id().expect("app buffer");
    let app_file = app.active_file_id().expect("app file");
    app.dispatch_ui_intent(CommandDispatchIntent::LspStartSession)
        .expect("start TypeScript");
    wait_for_live(&mut app, "typescript");
    wait_for_problem(&mut app, app_file, "2304");

    let original_editor = app
        .editor()
        .text(app_buffer)
        .expect("app editor text")
        .to_string();
    let original_disk = std::fs::read_to_string(&app_source).expect("app disk text");
    let missing_name = TextCoordinate {
        line: 0,
        character: 14,
        byte_offset: None,
        utf16_offset: None,
    };
    let missing_name_end = TextCoordinate {
        line: 0,
        character: 22,
        byte_offset: None,
        utf16_offset: None,
    };
    app.dispatch_ui_intent(CommandDispatchIntent::RequestCodeActions {
        buffer_id: app_buffer,
        range: legion_protocol::ProtocolTextRange {
            start: missing_name,
            end: missing_name_end,
        },
    })
    .expect("request missing-import code actions");
    let candidate = wait_for_code_action_candidate(&mut app, "import");
    app.dispatch_ui_intent(CommandDispatchIntent::SelectCodeAction {
        response_id: candidate.response_id.clone(),
        action_id: candidate.action_id.clone(),
    })
    .expect("select missing-import code action");
    let proposal = wait_for_native_proposal(
        &mut app,
        LanguageToolingOperationKind::CodeActionProposal,
        &[],
    );
    assert!(
        matches!(
            &proposal.payload,
            ProposalPayload::WorkspaceEdit(payload) if !payload.file_edits.is_empty()
        ),
        "missing-import action must produce a code-action edit proposal: {:?}",
        proposal.payload
    );
    assert_eq!(
        app.editor().text(app_buffer).expect("preview editor text"),
        original_editor,
        "code-action preview must not mutate editor text"
    );
    assert_eq!(
        std::fs::read_to_string(&app_source).expect("preview disk text"),
        original_disk,
        "code-action preview must not mutate disk"
    );
    cancel_native_proposal(&mut app, &proposal);

    app.dispatch_ui_intent(CommandDispatchIntent::RequestCodeActions {
        buffer_id: app_buffer,
        range: legion_protocol::ProtocolTextRange {
            start: missing_name,
            end: missing_name_end,
        },
    })
    .expect("request missing-import code actions again");
    let candidate = wait_for_code_action_candidate(&mut app, "import");
    app.dispatch_ui_intent(CommandDispatchIntent::SelectCodeAction {
        response_id: candidate.response_id,
        action_id: candidate.action_id,
    })
    .expect("select missing-import code action again");
    let proposal = wait_for_native_proposal(
        &mut app,
        LanguageToolingOperationKind::CodeActionProposal,
        &[proposal.proposal_id],
    );
    approve_native_proposal(&mut app, &proposal);
    let applied = app
        .handle_proposal_request(ProposalRequest::Apply(proposal))
        .expect("apply approved missing-import code action");
    assert!(
        matches!(applied, ProposalResponse::Applied(_)),
        "missing-import apply response: {applied:?}"
    );
    let edited = app
        .editor()
        .text(app_buffer)
        .expect("applied app editor text")
        .to_string();
    assert!(
        edited.contains("import"),
        "applied action must add an import: {edited}"
    );
    assert!(
        edited.contains("knownlib") && edited.contains("./lib"),
        "applied action must import knownlib from lib: {edited}"
    );
    app.save_all().expect("save approved missing-import action");
    let saved = std::fs::read_to_string(&app_source).expect("saved app disk text");
    assert_eq!(
        saved, edited,
        "normal save authority must persist the approved import"
    );
    wait_for_problem_to_clear(&mut app, app_file, "2304");
}

#[test]
#[ignore = "opt-in native Node + retained TypeScript fixtures"]
fn native_typescript_organize_imports_is_reviewable_and_removes_only_unused_imports() {
    const LIB_BEFORE: &str = "export function used(): string { return \"used\"; }\nexport function unused(): string { return \"unused\"; }\n";
    const APP_BEFORE: &str = "import { unused } from \"./lib\";\nimport { used } from \"./lib\";\nconst value = used();\n";
    let root = tempfile::tempdir().expect("temporary TypeScript workspace");
    let lib = root.path().join("lib.ts");
    let app_source = root.path().join("app.ts");
    std::fs::write(&lib, LIB_BEFORE).expect("seed import library");
    std::fs::write(&app_source, APP_BEFORE).expect("seed unused import app");

    let mut app = AppComposition::new();
    app.open_workspace(
        root.path(),
        WorkspaceTrustState::Trusted,
        PrincipalId("typescript-native-organize-imports".to_string()),
    )
    .expect("open workspace");
    configure_bundle(&mut app);
    app.open_file(lib.to_string_lossy()).expect("open library");
    app.open_file(app_source.to_string_lossy())
        .expect("open app source");
    let app_buffer = app.active_buffer_id().expect("app buffer");
    app.dispatch_ui_intent(CommandDispatchIntent::LspStartSession)
        .expect("start TypeScript");
    wait_for_live(&mut app, "typescript");

    let original_editor = app
        .editor()
        .text(app_buffer)
        .expect("app editor text")
        .to_string();
    let original_disk = std::fs::read_to_string(&app_source).expect("app disk text");
    app.dispatch_ui_intent(CommandDispatchIntent::RequestOrganizeImportsProposal {
        buffer_id: app_buffer,
    })
    .expect("request organize imports");
    let proposal = wait_for_native_proposal(
        &mut app,
        LanguageToolingOperationKind::OrganizeImportsProposal,
        &[],
    );
    assert_eq!(
        app.editor().text(app_buffer).expect("preview editor text"),
        original_editor,
        "organize-imports preview must not mutate editor text"
    );
    assert_eq!(
        std::fs::read_to_string(&app_source).expect("preview disk text"),
        original_disk,
        "organize-imports preview must not mutate disk"
    );
    cancel_native_proposal(&mut app, &proposal);

    app.dispatch_ui_intent(CommandDispatchIntent::RequestOrganizeImportsProposal {
        buffer_id: app_buffer,
    })
    .expect("request organize imports again");
    let proposal = wait_for_native_proposal(
        &mut app,
        LanguageToolingOperationKind::OrganizeImportsProposal,
        &[proposal.proposal_id],
    );
    approve_native_proposal(&mut app, &proposal);
    let applied = app
        .handle_proposal_request(ProposalRequest::Apply(proposal))
        .expect("apply approved organize imports");
    assert!(
        matches!(applied, ProposalResponse::Applied(_)),
        "organize-imports apply response: {applied:?}"
    );
    let edited = app
        .editor()
        .text(app_buffer)
        .expect("applied app editor text")
        .to_string();
    assert!(
        !edited.contains("unused") && edited.contains("used") && edited.contains("./lib"),
        "organize imports must remove only unused import: {edited}"
    );
    app.save_all().expect("save approved organize imports");
    let saved = std::fs::read_to_string(&app_source).expect("saved app disk text");
    assert_eq!(
        saved, edited,
        "normal save authority must persist organized imports"
    );
    assert!(!saved.contains("unused"));
    assert!(saved.contains("used") && saved.contains("./lib"));
}
