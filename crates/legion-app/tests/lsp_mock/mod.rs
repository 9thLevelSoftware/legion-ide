//! Shared test helpers for integration tests that exercise the mock LSP server.
//!
//! The `mock_lsp_server` binary lives in `legion-lsp`.  Because
//! `env!("CARGO_BIN_EXE_mock_lsp_server")` is only available to `legion-lsp`'s
//! own test targets, we locate the binary relative to the running test
//! executable instead.  If the binary hasn't been built yet (e.g. when running
//! `cargo test -p legion-app` in isolation) the helpers return `None` and the
//! calling test should skip gracefully.
//!
//! Build the mock first to exercise the full test:
//!   cargo build -p legion-lsp --bin mock_lsp_server

use std::path::PathBuf;

use legion_lsp::{LspServerProcessConfig, LspSupervisorConfig};
use legion_protocol::{
    CapabilityDecisionId, CapabilityId, CausalityId, CorrelationId, FileFingerprint, LanguageId,
    LanguageServerId, LspConfiguredServerIdentity, LspWorkspaceTrustPosture, RedactionHint,
    SemanticPrivacyScope, WorkspaceId, WorkspaceRootId, WorkspaceTrustState,
};
use uuid::Uuid;

/// Locates the `mock_lsp_server` binary in `target/<profile>/`.
///
/// Returns `None` when the binary hasn't been built, so callers can skip
/// gracefully instead of failing the test suite.
#[allow(dead_code)]
pub fn mock_server_path() -> Option<PathBuf> {
    // Running test binary is at: target/<profile>/deps/<test_name>-<hash>[.exe]
    let exe = std::env::current_exe().ok()?;
    // parent() -> target/<profile>/deps
    // parent() -> target/<profile>
    let profile_dir = exe.parent()?.parent()?;
    let name = if cfg!(windows) {
        "mock_lsp_server.exe"
    } else {
        "mock_lsp_server"
    };
    let candidate = profile_dir.join(name);
    candidate.is_file().then_some(candidate)
}

fn fingerprint(value: &str) -> FileFingerprint {
    FileFingerprint {
        algorithm: "test".to_string(),
        value: value.to_string(),
    }
}

fn identity(command: &str) -> LspConfiguredServerIdentity {
    LspConfiguredServerIdentity {
        server_id: LanguageServerId(7),
        workspace_id: WorkspaceId(55),
        root_id: Some(WorkspaceRootId(5)),
        language_id: LanguageId("rust".to_string()),
        display_name: "mock-lsp-server".to_string(),
        command_hash: fingerprint(command),
        args_hash: Some(fingerprint("args")),
        env_hash: Some(fingerprint("env")),
        cwd_hash: Some(fingerprint("cwd")),
        settings_hash: Some(fingerprint("settings")),
        redaction_hints: vec![RedactionHint::MetadataOnly],
        schema_version: 1,
    }
}

fn posture(trust: WorkspaceTrustState, privacy_scope_allowed: bool) -> LspWorkspaceTrustPosture {
    LspWorkspaceTrustPosture {
        workspace_id: WorkspaceId(55),
        workspace_trust_state: trust,
        privacy_scope: SemanticPrivacyScope::Workspace,
        privacy_scope_allowed,
        required_capability: CapabilityId("process.spawn".to_string()),
        decision_id: Some(CapabilityDecisionId(99)),
        diagnostics: Vec::new(),
        schema_version: 1,
    }
}

fn launch_policy(command: &str) -> legion_protocol::LspLaunchPolicyDecision {
    legion_protocol::LspLaunchPolicyDecision::evaluate(
        identity(command),
        posture(WorkspaceTrustState::Trusted, true),
        true,
        CorrelationId(91),
        CausalityId(Uuid::from_u128(92)),
        Vec::new(),
        1,
    )
}

/// Constructs an [`LspSupervisorConfig`] pointing at the mock binary.
///
/// # Panics
/// Panics if `mock_server_path()` returns `None`.  Callers that want graceful
/// skipping should call `mock_server_path()` directly before calling this.
#[allow(dead_code)]
pub fn mock_supervisor_config() -> LspSupervisorConfig {
    let path = mock_server_path().expect("mock_lsp_server binary not found — run: cargo build -p legion-lsp --bin mock_lsp_server");
    let command = path.to_string_lossy().into_owned();
    LspSupervisorConfig {
        launch_policy: launch_policy(&command),
        process: LspServerProcessConfig {
            command,
            args: Vec::new(),
            cwd: Some(
                PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                    .parent()
                    .expect("legion-app crate is two levels below the workspace root")
                    .parent()
                    .expect("legion-app crate is two levels below the workspace root")
                    .to_path_buf(),
            ),
            env: Vec::new(),
        },
        initial_backoff_ms: 25,
        max_backoff_ms: 400,
        max_restart_attempts: 3,
    }
}
