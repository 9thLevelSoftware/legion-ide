//! Focused approval-lifecycle tests for the local TypeScript bundle APIs.
//!
//! These tests call the app's real startup authority after configuration and
//! observe the real broker decision without spawning a process.

use std::path::{Path, PathBuf};

use super::*;
use crate::language::LanguageStartupContext;
use legion_lsp::LspServerProcessConfig;
use legion_protocol::{
    CausalityId, CorrelationId, LanguageId, LanguageServerId, PrincipalId, WorkspaceTrustState,
};

fn fixture_files() -> (tempfile::TempDir, PathBuf, PathBuf, PathBuf, PathBuf) {
    let dir = tempfile::tempdir().expect("fixture directory");
    let server = dir.path().join("server.tgz");
    let compiler = dir.path().join("compiler.tgz");
    let old_node = dir.path().join(if cfg!(windows) {
        "old-node.exe"
    } else {
        "old-node"
    });
    let new_node = dir.path().join(if cfg!(windows) {
        "new-node.exe"
    } else {
        "new-node"
    });
    for path in [&server, &compiler, &old_node, &new_node] {
        std::fs::write(path, b"fixture").expect("fixture file");
    }
    (dir, server, compiler, old_node, new_node)
}

fn open_trusted_app(root: &Path) -> AppComposition {
    let mut app = AppComposition::new();
    app.open_workspace(
        root,
        WorkspaceTrustState::Trusted,
        PrincipalId("toolchain-approval-test".to_string()),
    )
    .expect("open trusted workspace");
    app
}

fn startup_context(app: &AppComposition) -> LanguageStartupContext {
    let opened = app
        .active_documents
        .opened_workspace
        .as_ref()
        .expect("workspace opened");
    let root = std::fs::canonicalize(
        app.active_documents
            .workspace_root_path
            .as_ref()
            .expect("workspace root"),
    )
    .expect("canonical workspace root");
    LanguageStartupContext {
        workspace_id: opened.workspace_id,
        root_id: opened.root_id,
        workspace_root: root,
        principal_id: app
            .active_documents
            .active_principal_id
            .clone()
            .expect("principal"),
        trust: WorkspaceTrustState::Trusted,
        correlation_id: CorrelationId(1),
        causality_id: CausalityId(uuid::Uuid::from_u128(1)),
    }
}

fn broker_allows_command(app: &AppComposition, command: &Path) -> bool {
    let context = startup_context(app);
    let root_uri = crate::canonical_path_to_uri(
        context
            .workspace_root
            .to_str()
            .expect("workspace root UTF-8"),
    );
    app.language_startup_authority
        .prepare_configured(
            &context,
            LanguageServerId(102),
            LanguageId("typescript".to_string()),
            "approval-test",
            LspServerProcessConfig {
                command: std::fs::canonicalize(command)
                    .expect("canonical command")
                    .to_string_lossy()
                    .into_owned(),
                args: Vec::new(),
                cwd: Some(context.workspace_root.clone()),
                env: Vec::new(),
            },
            root_uri,
            None,
            None,
        )
        .is_ok()
}

#[test]
fn manual_bundle_replacement_and_clear_revoke_exact_node_grants() {
    let (dir, server, compiler, old_node, new_node) = fixture_files();
    let mut app = open_trusted_app(dir.path());

    app.configure_typescript_bundle(
        LanguageServerId(102),
        server.clone(),
        compiler.clone(),
        old_node.clone(),
        dir.path().join("cache-old"),
    )
    .expect("configure old manual bundle");
    assert!(broker_allows_command(&app, &old_node));

    std::fs::remove_file(&old_node).expect("remove old Node fixture");
    app.configure_typescript_bundle(
        LanguageServerId(102),
        server,
        compiler,
        new_node.clone(),
        dir.path().join("cache-new"),
    )
    .expect("replace manual bundle");
    std::fs::write(&old_node, b"recreated old fixture").expect("recreate old Node fixture");
    assert!(
        !broker_allows_command(&app, &old_node),
        "recreated path must remain denied after replacement revocation"
    );
    assert!(broker_allows_command(&app, &new_node));

    app.clear_typescript_toolchain();
    assert!(
        !broker_allows_command(&app, &new_node),
        "cleared manual Node grant must be revoked"
    );
}

#[test]
fn unrelated_configured_server_keeps_a_shared_node_grant() {
    let (dir, server, compiler, node, new_node) = fixture_files();
    let mut app = open_trusted_app(dir.path());
    app.configure_language_server_binary(LanguageServerId(101), node.clone())
        .expect("configure unrelated server");
    app.configure_typescript_bundle(
        LanguageServerId(102),
        server,
        compiler,
        node.clone(),
        dir.path().join("cache"),
    )
    .expect("configure shared Node bundle");
    app.clear_typescript_toolchain();
    assert!(
        broker_allows_command(&app, &node),
        "unrelated configured server still owns the shared grant"
    );
    assert!(!broker_allows_command(&app, &new_node));
}

#[test]
fn normal_configuration_is_atomic_and_populates_all_typescript_family_maps() {
    let (dir, server, compiler, node, new_node) = fixture_files();
    let mut app = open_trusted_app(dir.path());
    let node_c = dir.path().join(if cfg!(windows) {
        "replacement-node.exe"
    } else {
        "replacement-node"
    });
    std::fs::write(&node_c, b"replacement fixture").expect("replacement Node fixture");
    let legacy_b = dir.path().join(if cfg!(windows) {
        "legacy-b.exe"
    } else {
        "legacy-b"
    });
    std::fs::write(&legacy_b, b"legacy fixture").expect("legacy Node fixture");
    app.configure_typescript_bundle(
        LanguageServerId(102),
        server.clone(),
        compiler.clone(),
        node.clone(),
        dir.path().join("legacy-cache-a"),
    )
    .expect("configure first legacy bundle");
    app.configure_typescript_bundle(
        LanguageServerId(106),
        server.clone(),
        compiler.clone(),
        legacy_b.clone(),
        dir.path().join("legacy-cache-b"),
    )
    .expect("configure second legacy bundle");
    app.configure_typescript_toolchain(&server, &compiler, &node_c)
        .expect("configure normal toolchain");
    let before = app.language_toolchain_settings();
    assert_eq!(app.typescript_bundles.len(), 4);
    for id in [102_u64, 106, 107, 108] {
        assert!(app.typescript_bundles.contains_key(&LanguageServerId(id)));
    }
    assert!(!broker_allows_command(&app, &node));
    assert!(
        !broker_allows_command(&app, &legacy_b),
        "normal replacement must revoke every replaced legacy Node"
    );
    assert!(broker_allows_command(&app, &node_c));

    let missing = dir.path().join("missing-compiler.tgz");
    assert!(
        app.configure_typescript_toolchain(&server, &missing, &new_node)
            .is_err()
    );
    assert_eq!(app.language_toolchain_settings(), before);
    assert_eq!(app.typescript_bundles.len(), 4);
    assert!(broker_allows_command(&app, &node_c));
    assert!(!broker_allows_command(&app, &node));
    assert!(!broker_allows_command(&app, &legacy_b));
    assert!(!broker_allows_command(&app, &new_node));
}

fn python_fixture_files() -> (tempfile::TempDir, PathBuf, PathBuf) {
    let dir = tempfile::tempdir().expect("fixture directory");
    let interpreter = dir.path().join("python-interpreter.exe");
    let formatter = dir.path().join("python-formatter.exe");
    for path in [&interpreter, &formatter] {
        std::fs::write(path, b"fixture").expect("fixture file");
    }
    (dir, interpreter, formatter)
}

fn refusal(error: AppCompositionError) -> ProtocolError {
    match error {
        AppCompositionError::Protocol(protocol) => protocol,
        other => panic!("expected a protocol refusal, got {other:?}"),
    }
}

fn refusal_code(error: AppCompositionError) -> String {
    refusal(error).code
}

#[test]
fn python_toolchain_requires_trusted_workspace() {
    let (dir, interpreter, formatter) = python_fixture_files();
    let mut app = AppComposition::new();
    app.open_workspace(
        dir.path(),
        WorkspaceTrustState::Untrusted,
        PrincipalId("python-toolchain-test".to_string()),
    )
    .expect("open untrusted workspace");

    let error = app
        .configure_python_toolchain(&interpreter, &formatter)
        .expect_err("an untrusted workspace must refuse Python configuration");
    assert_eq!(
        refusal_code(error),
        "language_toolchain_workspace_untrusted"
    );
    assert!(app.language_toolchain_settings().python.is_none());
    assert!(
        !broker_allows_command(&app, &interpreter),
        "a refused configuration must not leave an exact-binary allowance"
    );
    assert!(!broker_allows_command(&app, &formatter));

    // Positive control: the same fixtures and the same broker observe a real
    // grant once the workspace is trusted, so the denials above are about the
    // missing allowance rather than about the harness.
    app.open_workspace(
        dir.path(),
        WorkspaceTrustState::Trusted,
        PrincipalId("python-toolchain-test".to_string()),
    )
    .expect("re-open the same root as trusted");
    assert!(!broker_allows_command(&app, &interpreter));
    app.configure_python_toolchain(&interpreter, &formatter)
        .expect("configure once the workspace is trusted");
    assert!(broker_allows_command(&app, &interpreter));
    assert!(broker_allows_command(&app, &formatter));
}

#[test]
fn python_toolchain_rejects_path_resolved_executable_names() {
    let (dir, interpreter, formatter) = python_fixture_files();
    let mut app = open_trusted_app(dir.path());

    let error = app
        .configure_python_toolchain("python", &formatter)
        .expect_err("a bare interpreter name must be refused");
    assert_eq!(refusal_code(error), "language_toolchain_path_not_explicit");
    assert!(app.language_toolchain_settings().python.is_none());
    assert!(
        !broker_allows_command(&app, &formatter),
        "the bare name must be refused before any path is granted"
    );

    let error = app
        .configure_python_toolchain(&interpreter, "black")
        .expect_err("a bare formatter name must be refused");
    assert_eq!(refusal_code(error), "language_toolchain_path_not_explicit");
    assert!(app.language_toolchain_settings().python.is_none());
    assert!(
        !broker_allows_command(&app, &interpreter),
        "a valid first path must not be granted when the second is a bare name"
    );
    assert!(!broker_allows_command(&app, &formatter));

    // Positive control: the same pair is accepted once both are explicit.
    app.configure_python_toolchain(&interpreter, &formatter)
        .expect("explicit paths are accepted");
    assert!(broker_allows_command(&app, &interpreter));
    assert!(broker_allows_command(&app, &formatter));
}

#[test]
fn python_toolchain_rejects_missing_or_non_regular_executables() {
    let (dir, interpreter, formatter) = python_fixture_files();
    let mut app = open_trusted_app(dir.path());

    let missing = dir.path().join("missing-interpreter.exe");
    let error = app
        .configure_python_toolchain(&missing, &formatter)
        .expect_err("a missing interpreter must be refused");
    assert_eq!(refusal_code(error), "language_toolchain_input_invalid");

    let directory = dir.path().join("interpreter-directory");
    std::fs::create_dir(&directory).expect("directory fixture");
    let canonical_directory = std::fs::canonicalize(&directory).expect("canonical directory");
    let protocol = refusal(
        app.configure_python_toolchain(&directory, &formatter)
            .expect_err("a directory is not a regular file"),
    );
    assert_eq!(protocol.code, "language_toolchain_input_invalid");
    assert!(protocol.message.contains("must be a regular file"));
    assert!(
        protocol
            .message
            .contains(canonical_directory.to_str().expect("UTF-8 directory")),
        "the refusal must name the rejected path: {}",
        protocol.message
    );

    let protocol = refusal(
        app.configure_python_toolchain(&interpreter, &directory)
            .expect_err("a directory formatter is not a regular file"),
    );
    assert_eq!(protocol.code, "language_toolchain_input_invalid");

    assert!(app.language_toolchain_settings().python.is_none());
    assert!(
        !broker_allows_command(&app, &interpreter),
        "no allowance may survive a refused configuration"
    );
    assert!(!broker_allows_command(&app, &formatter));
}

#[test]
fn configure_python_toolchain_records_canonical_interpreter_and_formatter() {
    let (dir, interpreter, formatter) = python_fixture_files();
    let mut app = open_trusted_app(dir.path());
    app.configure_python_toolchain(&interpreter, &formatter)
        .expect("configure the Python toolchain");

    let recorded = app
        .language_toolchain_settings()
        .python
        .expect("python section recorded");
    let canonical_interpreter =
        std::fs::canonicalize(&interpreter).expect("canonical interpreter fixture");
    let canonical_formatter =
        std::fs::canonicalize(&formatter).expect("canonical formatter fixture");
    assert_eq!(
        recorded.interpreter_executable,
        CanonicalPath(
            canonical_interpreter
                .to_str()
                .expect("UTF-8 interpreter")
                .to_string()
        )
    );
    assert_eq!(
        recorded.formatter_executable,
        CanonicalPath(
            canonical_formatter
                .to_str()
                .expect("UTF-8 formatter")
                .to_string()
        )
    );
    assert!(Path::new(&recorded.interpreter_executable.0).is_absolute());
    assert!(Path::new(&recorded.formatter_executable.0).is_absolute());
    assert_eq!(app.language_toolchain_settings().schema_version, 1);
    assert!(
        app.language_toolchain_settings().typescript.is_none(),
        "configuring Python must not touch the TypeScript section"
    );
    assert!(broker_allows_command(&app, &interpreter));
    assert!(broker_allows_command(&app, &formatter));

    // The recorded interpreter is what the Pyright payload carries.
    let options = pyright_initialization_options(&recorded);
    assert_eq!(
        options["settings"]["python"]["pythonPath"]
            .as_str()
            .expect("pythonPath is a string"),
        recorded.interpreter_executable.0
    );
    // The reachable accessor is the same payload built from the same recorded
    // settings, and it reports nothing at all once the section is cleared.
    assert_eq!(
        app.pyright_configuration_payload(),
        Some(options),
        "the accessor must return the payload built from the recorded settings"
    );
    app.clear_python_toolchain();
    assert_eq!(
        app.pyright_configuration_payload(),
        None,
        "no Python section means no Pyright payload"
    );
}

#[test]
fn invalid_python_formatter_leaves_previous_configuration_intact() {
    let (dir, interpreter, formatter) = python_fixture_files();
    let mut app = open_trusted_app(dir.path());
    app.configure_python_toolchain(&interpreter, &formatter)
        .expect("configure the first Python pair");
    let before = app.language_toolchain_settings();

    let replacement = dir.path().join("replacement-interpreter.exe");
    std::fs::write(&replacement, b"replacement fixture").expect("replacement fixture");
    let missing_formatter = dir.path().join("missing-formatter.exe");

    let error = app
        .configure_python_toolchain(&replacement, &missing_formatter)
        .expect_err("an invalid formatter must refuse the whole configuration");
    assert_eq!(refusal_code(error), "language_toolchain_input_invalid");

    assert_eq!(app.language_toolchain_settings(), before);
    assert!(
        broker_allows_command(&app, &interpreter),
        "the previously configured interpreter must keep its allowance"
    );
    assert!(broker_allows_command(&app, &formatter));
    assert!(
        !broker_allows_command(&app, &replacement),
        "the refused replacement interpreter must never be granted"
    );
}

#[test]
fn clear_python_toolchain_revokes_only_unshared_exact_binaries() {
    let (dir, interpreter, formatter) = python_fixture_files();
    let mut app = open_trusted_app(dir.path());
    app.configure_language_server_binary(LanguageServerId(104), interpreter.clone())
        .expect("configure an unrelated server on the same exact binary");
    app.configure_python_toolchain(&interpreter, &formatter)
        .expect("configure the Python toolchain");
    assert!(broker_allows_command(&app, &interpreter));
    assert!(broker_allows_command(&app, &formatter));

    app.clear_python_toolchain();
    assert!(app.language_toolchain_settings().python.is_none());
    assert!(
        broker_allows_command(&app, &interpreter),
        "an exact binary another configuration still holds must survive the clear"
    );
    assert!(
        !broker_allows_command(&app, &formatter),
        "the unshared formatter grant must be revoked"
    );

    // Clearing an already-cleared toolchain is a no-op, not a second revoke.
    app.clear_python_toolchain();
    assert!(broker_allows_command(&app, &interpreter));
    assert!(!broker_allows_command(&app, &formatter));
}
