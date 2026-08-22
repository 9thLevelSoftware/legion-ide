use std::{fs, path::PathBuf};

#[cfg(feature = "ai")]
use std::{
    sync::{Arc, Condvar, Mutex, mpsc},
    time::{Duration, Instant},
};

use legion_ai::tool_calls::ScriptedToolCallingProviderBuilder;
#[cfg(feature = "ai")]
use legion_ai::{
    ChatCompletionRequest, ChatCompletionResponse, EmbeddingRequest, EmbeddingResponse,
    ModelProvider, ProviderCapabilities, ProviderError, ProviderId,
    tool_calls::{
        ToolCallingProvider, ToolCompletionRequest, ToolCompletionResponse,
        ToolCompletionStopReason, ToolTurnBlock,
    },
};
#[cfg(feature = "ai")]
use legion_app::AppDelegatedToolHost;
use legion_app::{
    AppComposition, AppDelegatedTaskExecutionOutcome, AppDelegatedTaskOutcome, AppProductMode,
};
use legion_protocol::{
    CanonicalPath, CausalityId, CorrelationId, DelegatedTaskPlanContract, DelegatedTaskPlanId,
    DelegatedTaskPlanningBoundaryInput, DelegatedTaskProposalHunkDisposition,
    DelegatedTaskRiskTolerance, DelegatedTaskRuntimeActivationState, DelegatedTaskScope,
    DelegatedTaskScopeTargetKind, DelegatedTaskToolPermissionDecision, FileFingerprint,
    LegionToolKind, PrincipalId, ProposalPayload, TimestampMillis, WorkspaceId,
    WorkspaceTrustState, delegated_task_plan_from_boundary_input,
};

fn delegated_plan_contract(plan_id: DelegatedTaskPlanId) -> DelegatedTaskPlanContract {
    let boundary_input = DelegatedTaskPlanningBoundaryInput {
        plan_id,
        workspace_id: Some(WorkspaceId(1)),
        objective_summary_hash: FileFingerprint {
            algorithm: "sha256".to_string(),
            value: "test-hash".to_string(),
        },
        allowed_operation_classes: vec![],
        context_manifest: None,
        privacy_inspector: None,
        permission_budget_projection: None,
        approval_checklist: None,
        checkpoint_rollback: None,
        assisted_ai_projection: None,
        assisted_ai_required: false,
        affected_targets: vec![],
        steps: vec![],
        proposal_preview_links: vec![],
        workspace_trust_state: WorkspaceTrustState::Trusted,
        privacy_denied: false,
        permission_budget_denied: false,
        permission_budget_depleted: false,
        approval_checklist_valid: true,
        checkpoint_required: false,
        checkpoint_available: true,
        rollback_required: false,
        rollback_available: true,
        correlation_id: CorrelationId(1),
        causality_id: CausalityId(uuid::Uuid::from_u128(1)),
        created_at: TimestampMillis(1),
        schema_version: 1,
    };
    delegated_task_plan_from_boundary_input(boundary_input)
}

fn unique_plan_id(label: &str) -> DelegatedTaskPlanId {
    DelegatedTaskPlanId(format!("{label}-{}", uuid::Uuid::now_v7()))
}

/// Returns the expected sandbox path when a workspace root is known.
/// After PKT-WORKTREE D2, sandbox paths are derived from the workspace root,
/// not CWD, so callers that opened a workspace must pass it here.
fn sandbox_path_in(workspace_root: &std::path::Path, plan_id: &DelegatedTaskPlanId) -> PathBuf {
    workspace_root
        .join("target/delegated-tasks")
        .join(format!("task-{}", plan_id.0))
}

/// Fallback for tests that do not open a workspace: sandboxes fall back to CWD-relative paths.
fn sandbox_path_cwd(plan_id: &DelegatedTaskPlanId) -> PathBuf {
    PathBuf::from("target/delegated-tasks").join(format!("task-{}", plan_id.0))
}

#[cfg(windows)]
fn acp_host_command() -> (PathBuf, Vec<String>) {
    (
        PathBuf::from("powershell"),
        vec![
            "-NoProfile".to_string(),
            "-Command".to_string(),
            r#"$ErrorActionPreference = 'Stop'; New-Item -ItemType Directory -Force -Path $env:LEGION_ACP_TARGET_DIR | Out-Null; @('external-agent=claude-code', "plan=$env:LEGION_ACP_PLAN_ID") | Set-Content -LiteralPath $env:LEGION_ACP_TARGET_PATH -Encoding UTF8"#
                .to_string(),
        ],
    )
}

#[cfg(not(windows))]
fn acp_host_command() -> (PathBuf, Vec<String>) {
    (
        PathBuf::from("/bin/sh"),
        vec![
            "-c".to_string(),
            r#"mkdir -p "$(dirname "$LEGION_ACP_TARGET_PATH")"; printf 'external-agent=claude-code\nplan=%s\n' "$LEGION_ACP_PLAN_ID" > "$LEGION_ACP_TARGET_PATH""#
                .to_string(),
        ],
    )
}

/// Drop-guarded temporary workspace. Removes the directory on drop with a
/// prefix/location check so a panic mid-test never leaks the temp dir.
struct TempWorkspace {
    root: PathBuf,
}

impl std::ops::Deref for TempWorkspace {
    type Target = std::path::Path;

    fn deref(&self) -> &std::path::Path {
        &self.root
    }
}

impl AsRef<std::path::Path> for TempWorkspace {
    fn as_ref(&self) -> &std::path::Path {
        &self.root
    }
}

impl Drop for TempWorkspace {
    fn drop(&mut self) {
        let temp_root = std::env::temp_dir();
        let file_name = self.root.file_name().and_then(|name| name.to_str());
        if self.root.starts_with(&temp_root)
            && file_name.is_some_and(|name| name.starts_with("legion_app_delegated_"))
        {
            let _ = fs::remove_dir_all(&self.root);
        }
    }
}

fn temp_workspace(label: &str) -> TempWorkspace {
    let root = std::env::temp_dir().join(format!(
        "legion_app_delegated_{label}_{}",
        uuid::Uuid::now_v7()
    ));
    fs::create_dir(&root).expect("temp workspace should be created");
    TempWorkspace { root }
}

#[cfg(feature = "ai")]
struct BlockingDelegatedProvider {
    entered: Mutex<Option<mpsc::Sender<()>>>,
    release: Arc<(Mutex<bool>, Condvar)>,
}

#[cfg(feature = "ai")]
struct CompletionSignallingProvider {
    finished: Mutex<Option<mpsc::Sender<()>>>,
    panic_on_complete: bool,
}

#[cfg(feature = "ai")]
impl Drop for CompletionSignallingProvider {
    fn drop(&mut self) {
        if let Some(finished) = self.finished.lock().expect("finished lock").take() {
            let _ = finished.send(());
        }
    }
}

#[cfg(feature = "ai")]
impl ModelProvider for CompletionSignallingProvider {
    fn provider_id(&self) -> ProviderId {
        "provider:completion-signalling".to_string()
    }

    fn capabilities(&self) -> ProviderCapabilities {
        ProviderCapabilities {
            completion: false,
            embedding: false,
            batch: false,
            inline_prediction: false,
            tool_use: true,
        }
    }

    fn complete(
        &self,
        request: ChatCompletionRequest,
    ) -> Result<ChatCompletionResponse, ProviderError> {
        Err(ProviderError::unsupported(request.provider, "complete"))
    }

    fn embed(&self, request: EmbeddingRequest) -> Result<EmbeddingResponse, ProviderError> {
        Err(ProviderError::unsupported(request.provider, "embed"))
    }
}

#[cfg(feature = "ai")]
impl ToolCallingProvider for CompletionSignallingProvider {
    fn complete_with_tools(
        &self,
        request: ToolCompletionRequest,
    ) -> Result<ToolCompletionResponse, ProviderError> {
        assert!(!self.panic_on_complete, "provider panic fixture");
        Ok(ToolCompletionResponse {
            provider: self.provider_id(),
            model: request.model,
            blocks: vec![ToolTurnBlock::Text("completed before cancel".to_string())],
            stop_reason: ToolCompletionStopReason::EndTurn,
        })
    }
}

#[cfg(feature = "ai")]
impl ModelProvider for BlockingDelegatedProvider {
    fn provider_id(&self) -> ProviderId {
        "provider:blocking-delegated".to_string()
    }

    fn capabilities(&self) -> ProviderCapabilities {
        ProviderCapabilities {
            completion: false,
            embedding: false,
            batch: false,
            inline_prediction: false,
            tool_use: true,
        }
    }

    fn complete(
        &self,
        request: ChatCompletionRequest,
    ) -> Result<ChatCompletionResponse, ProviderError> {
        Err(ProviderError::unsupported(request.provider, "complete"))
    }

    fn embed(&self, request: EmbeddingRequest) -> Result<EmbeddingResponse, ProviderError> {
        Err(ProviderError::unsupported(request.provider, "embed"))
    }
}

#[cfg(feature = "ai")]
impl ToolCallingProvider for BlockingDelegatedProvider {
    fn complete_with_tools(
        &self,
        request: ToolCompletionRequest,
    ) -> Result<ToolCompletionResponse, ProviderError> {
        if let Some(entered) = self.entered.lock().expect("entered lock").take() {
            let _ = entered.send(());
        }
        let (released, wake) = &*self.release;
        let mut released = released.lock().expect("release lock");
        while !*released {
            released = wake.wait(released).expect("release wait");
        }
        Ok(ToolCompletionResponse {
            provider: self.provider_id(),
            model: request.model,
            blocks: vec![ToolTurnBlock::Text("cancel checkpoint".to_string())],
            stop_reason: ToolCompletionStopReason::EndTurn,
        })
    }
}

#[cfg(feature = "ai")]
#[test]
fn delegated_background_submit_stays_responsive_and_manual_waits_for_cancel_ack() {
    let workspace_root = temp_workspace("background-cancel");
    fs::write(workspace_root.join("main.txt"), "before\n").expect("write fixture");
    let mut app = AppComposition::new();
    app.open_workspace(
        &workspace_root,
        WorkspaceTrustState::Trusted,
        PrincipalId("delegate-background-test".to_string()),
    )
    .expect("open workspace");
    app.set_product_mode(AppProductMode::Delegate);

    let (entered_tx, entered_rx) = mpsc::channel();
    let release = Arc::new((Mutex::new(false), Condvar::new()));
    let provider = BlockingDelegatedProvider {
        entered: Mutex::new(Some(entered_tx)),
        release: release.clone(),
    };
    let scope = DelegatedTaskScope {
        target_kind: DelegatedTaskScopeTargetKind::Repo,
        workspace_root: CanonicalPath(workspace_root.to_string_lossy().into_owned()),
        target_path: None,
        risk_tolerance: DelegatedTaskRiskTolerance::Balanced,
        allowed_tools: vec![LegionToolKind::Read],
        forbidden_paths: vec![],
        schema_version: 1,
    };

    let submitted_at = Instant::now();
    app.start_delegated_task_background(
        "wait until cancelled".to_string(),
        scope,
        Box::new(provider),
    )
    .expect("background submit");
    assert!(
        submitted_at.elapsed() < Duration::from_secs(1),
        "submission must return without waiting for the provider"
    );
    entered_rx
        .recv_timeout(Duration::from_secs(2))
        .expect("worker must enter provider call");

    app.cancel_delegated_task()
        .expect("cancel remains available while provider call is blocked");
    app.set_product_mode(AppProductMode::Manual);
    assert_eq!(
        app.product_mode(),
        AppProductMode::Delegate,
        "Manual must not be projected before delegated cancellation is acknowledged"
    );

    let (released, wake) = &*release;
    *released.lock().expect("release lock") = true;
    wake.notify_all();

    let deadline = Instant::now() + Duration::from_secs(3);
    let outcome = loop {
        if let Some(outcome) = app
            .poll_delegated_task()
            .expect("poll delegated completion")
        {
            break outcome;
        }
        assert!(Instant::now() < deadline, "delegated task did not finish");
        std::thread::yield_now();
    };
    assert!(
        matches!(outcome, AppDelegatedTaskOutcome::Completed { .. }),
        "a provider completion already produced before the cancellation boundary remains authoritative"
    );

    app.set_product_mode(AppProductMode::Manual);
    assert_eq!(app.product_mode(), AppProductMode::Manual);
}

#[cfg(feature = "ai")]
#[test]
fn delegated_background_rejects_sync_overlap_and_defers_assist_downgrade() {
    let workspace_root = temp_workspace("background-owner");
    fs::write(workspace_root.join("main.txt"), "before\n").expect("write fixture");
    let mut app = AppComposition::new();
    app.open_workspace(
        &workspace_root,
        WorkspaceTrustState::Trusted,
        PrincipalId("delegate-owner-test".to_string()),
    )
    .expect("open workspace");
    app.set_product_mode(AppProductMode::Delegate);

    let (entered_tx, entered_rx) = mpsc::channel();
    let release = Arc::new((Mutex::new(false), Condvar::new()));
    app.start_delegated_task_background(
        "hold the delegated worker".to_string(),
        test_scope(&workspace_root),
        Box::new(BlockingDelegatedProvider {
            entered: Mutex::new(Some(entered_tx)),
            release: release.clone(),
        }),
    )
    .expect("background submit");
    entered_rx
        .recv_timeout(Duration::from_secs(2))
        .expect("worker must enter provider call");

    let sync_provider = ScriptedToolCallingProviderBuilder::new()
        .end_turn("must not start")
        .build("overlap-rejected");
    let overlap = app.start_delegated_task(
        "overlapping sync task".to_string(),
        test_scope(&workspace_root),
        &sync_provider,
    );
    assert!(
        overlap
            .expect_err("sync overlap must be rejected")
            .to_string()
            .contains("already running")
    );

    app.set_product_mode(AppProductMode::Assist);
    assert_eq!(
        app.product_mode(),
        AppProductMode::Delegate,
        "Assist cannot be projected while a Delegate worker still owns WorkerRuntime"
    );

    let (released, wake) = &*release;
    *released.lock().expect("release lock") = true;
    wake.notify_all();
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        if app
            .poll_delegated_task()
            .expect("poll delegated completion")
            .is_some()
        {
            break;
        }
        assert!(Instant::now() < deadline, "delegated task did not finish");
        std::thread::yield_now();
    }
    app.set_product_mode(AppProductMode::Assist);
    assert_eq!(app.product_mode(), AppProductMode::Assist);
}

#[cfg(feature = "ai")]
#[test]
fn delegated_cancel_after_worker_completion_preserves_completed_outcome() {
    let workspace_root = temp_workspace("background-completed-before-cancel");
    fs::write(workspace_root.join("main.txt"), "before\n").expect("write fixture");
    let mut app = AppComposition::new();
    app.open_workspace(
        &workspace_root,
        WorkspaceTrustState::Trusted,
        PrincipalId("delegate-completion-test".to_string()),
    )
    .expect("open workspace");
    app.set_product_mode(AppProductMode::Delegate);

    let (finished_tx, finished_rx) = mpsc::channel();
    app.start_delegated_task_background(
        "finish before cancellation".to_string(),
        test_scope(&workspace_root),
        Box::new(CompletionSignallingProvider {
            finished: Mutex::new(Some(finished_tx)),
            panic_on_complete: false,
        }),
    )
    .expect("background submit");
    finished_rx
        .recv_timeout(Duration::from_secs(3))
        .expect("worker must finish before cancellation");

    app.cancel_delegated_task()
        .expect("completion is still pending app-thread merge");
    let deadline = Instant::now() + Duration::from_secs(3);
    let outcome = loop {
        if let Some(outcome) = app
            .poll_delegated_task()
            .expect("poll delegated completion")
        {
            break outcome;
        }
        assert!(Instant::now() < deadline, "delegated task did not finish");
        std::thread::yield_now();
    };

    assert!(
        matches!(outcome, AppDelegatedTaskOutcome::Completed { .. }),
        "the worker-recorded Completed result is authoritative; got {outcome:?}"
    );
}

#[cfg(feature = "ai")]
#[test]
fn delegated_worker_panic_reports_failure_and_cleans_sandbox() {
    let workspace_root = temp_workspace("background-panic-cleanup");
    fs::write(workspace_root.join("main.txt"), "before\n").expect("write fixture");
    let mut app = AppComposition::new();
    app.open_workspace(
        &workspace_root,
        WorkspaceTrustState::Trusted,
        PrincipalId("delegate-panic-test".to_string()),
    )
    .expect("open workspace");
    app.set_product_mode(AppProductMode::Delegate);

    let (finished_tx, finished_rx) = mpsc::channel();
    app.start_delegated_task_background(
        "panic inside provider".to_string(),
        test_scope(&workspace_root),
        Box::new(CompletionSignallingProvider {
            finished: Mutex::new(Some(finished_tx)),
            panic_on_complete: true,
        }),
    )
    .expect("background submit");
    finished_rx
        .recv_timeout(Duration::from_secs(3))
        .expect("panicking provider must unwind");

    let deadline = Instant::now() + Duration::from_secs(3);
    let error = loop {
        match app.poll_delegated_task() {
            Err(error) => break error,
            Ok(None) => {}
            Ok(Some(outcome)) => panic!("panic cannot produce task outcome: {outcome:?}"),
        }
        assert!(Instant::now() < deadline, "delegated panic was not joined");
        std::thread::yield_now();
    };
    assert!(error.to_string().contains("panicked"));

    let sandbox_root = workspace_root.join("target").join("delegated-tasks");
    let remaining_directories = fs::read_dir(&sandbox_root)
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .filter(|entry| entry.path().is_dir())
        .collect::<Vec<_>>();
    assert!(
        remaining_directories.is_empty(),
        "panic cleanup must remove sandbox directories: {remaining_directories:?}"
    );
}

#[cfg(feature = "ai")]
#[test]
fn dropping_app_cancels_worker_without_blocking_and_reaper_joins_cleanup() {
    let workspace_root = temp_workspace("background-drop-cleanup");
    fs::write(workspace_root.join("main.txt"), "before\n").expect("write fixture");
    let mut app = AppComposition::new();
    app.open_workspace(
        &workspace_root,
        WorkspaceTrustState::Trusted,
        PrincipalId("delegate-drop-test".to_string()),
    )
    .expect("open workspace");
    app.set_product_mode(AppProductMode::Delegate);

    let (entered_tx, entered_rx) = mpsc::channel();
    let release = Arc::new((Mutex::new(false), Condvar::new()));
    app.start_delegated_task_background(
        "wait while app drops".to_string(),
        test_scope(&workspace_root),
        Box::new(BlockingDelegatedProvider {
            entered: Mutex::new(Some(entered_tx)),
            release: release.clone(),
        }),
    )
    .expect("background submit");
    entered_rx
        .recv_timeout(Duration::from_secs(2))
        .expect("worker must enter provider call");
    let owner_id = app
        .in_flight_delegated_owner_id_for_test()
        .expect("background worker owner id");

    let dropped_at = Instant::now();
    drop(app);
    assert!(
        dropped_at.elapsed() < Duration::from_secs(1),
        "Drop must hand joining to the reaper rather than block on provider transport"
    );
    let handoff_deadline = Instant::now() + Duration::from_secs(2);
    loop {
        let (handed_off, reaped) =
            AppComposition::delegated_worker_supervisor_state_for_test(owner_id);
        if handed_off {
            assert!(!reaped, "blocked worker cannot be reaped before release");
            break;
        }
        assert!(
            Instant::now() < handoff_deadline,
            "app drop did not transfer the delegated handle to the global supervisor"
        );
        std::thread::yield_now();
    }

    let (released, wake) = &*release;
    *released.lock().expect("release lock") = true;
    wake.notify_all();
    let sandbox_root = workspace_root.join("target").join("delegated-tasks");
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        let remaining_directories = fs::read_dir(&sandbox_root)
            .into_iter()
            .flatten()
            .filter_map(Result::ok)
            .filter(|entry| entry.path().is_dir())
            .count();
        if remaining_directories == 0 {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "drop reaper did not join worker cleanup"
        );
        std::thread::yield_now();
    }
    let reap_deadline = Instant::now() + Duration::from_secs(3);
    loop {
        let (_, reaped) = AppComposition::delegated_worker_supervisor_state_for_test(owner_id);
        if reaped {
            break;
        }
        assert!(
            Instant::now() < reap_deadline,
            "global supervisor did not join the delegated worker"
        );
        std::thread::yield_now();
    }
}

#[cfg(feature = "ai")]
#[test]
fn delegated_background_submit_rejects_scope_from_another_workspace() {
    let workspace_root = temp_workspace("canonical-root");
    let other_root = temp_workspace("mismatched-root");
    fs::write(workspace_root.join("main.txt"), "before\n").expect("write fixture");
    let mut app = AppComposition::new();
    app.open_workspace(
        &workspace_root,
        WorkspaceTrustState::Trusted,
        PrincipalId("delegate-root-test".to_string()),
    )
    .expect("open workspace");
    app.set_product_mode(AppProductMode::Delegate);
    let scope = DelegatedTaskScope {
        target_kind: DelegatedTaskScopeTargetKind::Repo,
        workspace_root: CanonicalPath(other_root.to_string_lossy().into_owned()),
        target_path: None,
        risk_tolerance: DelegatedTaskRiskTolerance::Balanced,
        allowed_tools: vec![LegionToolKind::Read],
        forbidden_paths: vec![],
        schema_version: 1,
    };
    let provider = ScriptedToolCallingProviderBuilder::new()
        .end_turn("must not run")
        .build("mismatched-root");

    let result = app.start_delegated_task_background(
        "must remain in canonical workspace".to_string(),
        scope,
        Box::new(provider),
    );

    assert!(
        result.is_err(),
        "mismatched workspace root must fail closed"
    );
    assert_eq!(
        app.shell_projection_snapshot("Legion")
            .expect("projection")
            .delegated_task_projection
            .runtime_activation,
        DelegatedTaskRuntimeActivationState::NotEncoded,
    );
}

#[cfg(feature = "ai")]
#[test]
fn delegated_background_spawn_failure_rolls_back_worker_and_activation() {
    let workspace_root = temp_workspace("spawn-failure-rollback");
    fs::write(workspace_root.join("main.txt"), "before\n").expect("write fixture");
    let mut app = AppComposition::new();
    app.open_workspace(
        &workspace_root,
        WorkspaceTrustState::Trusted,
        PrincipalId("delegate-spawn-failure-test".to_string()),
    )
    .expect("open workspace");
    app.set_product_mode(AppProductMode::Delegate);
    app.inject_delegated_spawn_failure_for_test();
    let provider = ScriptedToolCallingProviderBuilder::new()
        .end_turn("must not run")
        .build("spawn-failure");

    let error = app
        .start_delegated_task_background(
            "must fail before worker execution".to_string(),
            test_scope(&workspace_root),
            Box::new(provider),
        )
        .expect_err("injected worker spawn must be reported as an error");

    assert!(
        error
            .to_string()
            .contains("failed to spawn delegated task worker")
    );
    assert!(
        app.cancel_delegated_task()
            .expect_err("spawn failure must clear worker ownership")
            .to_string()
            .contains("no delegated task running")
    );
    assert_eq!(
        app.shell_projection_snapshot("Legion")
            .expect("projection")
            .delegated_task_projection
            .runtime_activation,
        DelegatedTaskRuntimeActivationState::NotEncoded,
    );
}

#[test]
fn execute_delegated_task_reports_missing_plan_without_error() {
    let mut app = AppComposition::new();
    app.set_product_mode(AppProductMode::Delegate);
    let plan_id = unique_plan_id("missing-plan");

    let outcome = app
        .execute_delegated_task(&plan_id)
        .expect("missing plan is a structured outcome");

    match outcome {
        AppDelegatedTaskExecutionOutcome::PlanMissing { plan_id: missing } => {
            assert_eq!(missing, plan_id);
        }
        other => panic!("expected PlanMissing, got {other:?}"),
    }
}

#[test]
fn execute_delegated_task_waits_for_write_permission_before_sandbox_allocation() {
    let mut app = AppComposition::new();
    app.set_product_mode(AppProductMode::Delegate);
    let plan_id = unique_plan_id("waiting-plan");
    let workspace_root = temp_workspace("waiting-plan");
    app.open_workspace(
        &workspace_root,
        WorkspaceTrustState::Trusted,
        PrincipalId(format!("delegate-test:{}", plan_id.0)),
    )
    .expect("workspace opens for projection snapshot");
    app.seed_delegated_task_plan_contracts(vec![delegated_plan_contract(plan_id.clone())]);

    let outcome = app
        .execute_delegated_task(&plan_id)
        .expect("permission wait is structured");

    match outcome {
        AppDelegatedTaskExecutionOutcome::WaitingForToolPermission { request } => {
            assert_eq!(
                request.decision,
                DelegatedTaskToolPermissionDecision::Confirm
            );
            assert!(!request.runtime_allowed);
            assert!(request.human_approval_required);
            assert!(!sandbox_path_in(&workspace_root, &plan_id).exists());
            let snapshot = app
                .shell_projection_snapshot("Legion")
                .expect("projection snapshot is available");
            assert_eq!(
                snapshot.delegated_task_projection.runtime_activation,
                DelegatedTaskRuntimeActivationState::Planned
            );
        }
        other => panic!("expected WaitingForToolPermission, got {other:?}"),
    }
}

#[test]
fn manual_mode_rejects_delegated_task_execution() {
    let mut app = AppComposition::new();
    let plan_id = unique_plan_id("manual-reject");
    app.seed_delegated_task_plan_contracts(vec![delegated_plan_contract(plan_id.clone())]);

    let err = app
        .execute_delegated_task(&plan_id)
        .expect_err("manual mode rejects delegated execution");

    assert!(err.to_string().contains("Delegate dispatch requires"));
}

#[test]
fn execute_delegated_task_fails_closed_after_denied_permission() {
    let mut app = AppComposition::new();
    app.set_product_mode(AppProductMode::Delegate);
    let plan_id = unique_plan_id("denied-plan");
    app.seed_delegated_task_plan_contracts(vec![delegated_plan_contract(plan_id.clone())]);
    let request_id = match app
        .execute_delegated_task(&plan_id)
        .expect("permission wait is structured")
    {
        AppDelegatedTaskExecutionOutcome::WaitingForToolPermission { request } => {
            request.request_id
        }
        other => panic!("expected WaitingForToolPermission, got {other:?}"),
    };

    app.record_delegate_tool_permission_decision(
        request_id.clone(),
        DelegatedTaskToolPermissionDecision::Deny,
    )
    .expect("deny decision is recorded");
    app.record_delegate_tool_permission_decision(
        request_id.clone(),
        DelegatedTaskToolPermissionDecision::Always,
    )
    .expect("later always decision keeps deny precedence");

    let outcome = app
        .execute_delegated_task(&plan_id)
        .expect("denied permission is structured");
    match outcome {
        AppDelegatedTaskExecutionOutcome::Denied { request } => {
            assert_eq!(request.request_id, request_id);
            assert_eq!(request.decision, DelegatedTaskToolPermissionDecision::Deny);
            assert!(request.deny_overrides);
            assert!(!request.runtime_allowed);
            // No workspace opened in this test: sandbox path falls back to CWD-relative.
            assert!(!sandbox_path_cwd(&plan_id).exists());
        }
        other => panic!("expected Denied, got {other:?}"),
    }
}

#[test]
fn execute_delegated_task_returns_proposal_after_explicit_write_allow() {
    let mut app = AppComposition::new();
    app.set_product_mode(AppProductMode::Delegate);
    let plan_id = unique_plan_id("approved-plan");
    let workspace_root = temp_workspace("approved-plan");
    app.open_workspace(
        &workspace_root,
        WorkspaceTrustState::Trusted,
        PrincipalId(format!("delegate-test:{}", plan_id.0)),
    )
    .expect("workspace opens for projection snapshot");
    app.seed_delegated_task_plan_contracts(vec![delegated_plan_contract(plan_id.clone())]);
    let request_id = match app
        .execute_delegated_task(&plan_id)
        .expect("permission wait is structured")
    {
        AppDelegatedTaskExecutionOutcome::WaitingForToolPermission { request } => {
            request.request_id
        }
        other => panic!("expected WaitingForToolPermission, got {other:?}"),
    };

    app.record_delegate_tool_permission_decision(
        request_id,
        DelegatedTaskToolPermissionDecision::Allow,
    )
    .expect("allow decision is recorded");

    let outcome = app
        .execute_delegated_task(&plan_id)
        .expect("approved execution succeeds");
    match outcome {
        AppDelegatedTaskExecutionOutcome::ProposalReady(proposal) => {
            assert!(proposal.correlation_id.0 > 0);
            assert!(!proposal.causality_id.0.is_nil());
            assert_ne!(proposal.provider_id, "provider-auto");
            assert_ne!(proposal.principal.0, "principal-auto");
            assert_eq!(
                proposal.request_id,
                format!("delegate:permission:{}:runtime", plan_id.0)
            );
            match &proposal.payload {
                ProposalPayload::CreateFile(create_file) => {
                    assert!(create_file.path.0.starts_with("delegated-task/"));
                    let content = create_file
                        .initial_content
                        .as_ref()
                        .expect("proposal content is derived from the plan");
                    assert!(content.contains("objective_hash=test-hash"));
                    assert!(!content.contains("modified content"));
                }
                other => panic!("expected CreateFile proposal, got {other:?}"),
            }
            assert!(!sandbox_path_in(&workspace_root, &plan_id).exists());
            let snapshot = app
                .shell_projection_snapshot("Legion")
                .expect("projection snapshot is available");
            assert_eq!(
                snapshot.delegated_task_projection.runtime_activation,
                DelegatedTaskRuntimeActivationState::WaitingForApproval
            );
        }
        other => panic!("expected ProposalReady, got {other:?}"),
    }
}

#[test]
fn execute_delegated_task_uses_acp_host_command_and_projects_comm_stream() {
    let mut app = AppComposition::new();
    app.set_product_mode(AppProductMode::Delegate);
    let plan_id = unique_plan_id("acp-host");
    let workspace_root = temp_workspace("acp-host");
    app.open_workspace(
        &workspace_root,
        WorkspaceTrustState::Trusted,
        PrincipalId(format!("delegate-test:{}", plan_id.0)),
    )
    .expect("workspace opens for projection snapshot");
    app.seed_delegated_task_plan_contracts(vec![delegated_plan_contract(plan_id.clone())]);
    let (program, args) = acp_host_command();
    app.set_acp_host_command(program, args);

    let request_id = match app
        .execute_delegated_task(&plan_id)
        .expect("permission wait is structured")
    {
        AppDelegatedTaskExecutionOutcome::WaitingForToolPermission { request } => {
            request.request_id
        }
        other => panic!("expected WaitingForToolPermission, got {other:?}"),
    };

    app.record_delegate_tool_permission_decision(
        request_id,
        DelegatedTaskToolPermissionDecision::Allow,
    )
    .expect("allow decision is recorded");

    let outcome = app
        .execute_delegated_task(&plan_id)
        .expect("approved external host execution succeeds");
    match outcome {
        AppDelegatedTaskExecutionOutcome::ProposalReady(proposal) => {
            assert!(proposal.correlation_id.0 > 0);
            assert!(!proposal.causality_id.0.is_nil());
            match &proposal.payload {
                ProposalPayload::CreateFile(create_file) => {
                    let content = create_file
                        .initial_content
                        .as_ref()
                        .expect("proposal content is derived from the host output");
                    assert!(content.contains("external-agent=claude-code"));
                    assert!(content.contains(&plan_id.0));
                }
                other => panic!("expected CreateFile proposal, got {other:?}"),
            }
            assert!(!sandbox_path_in(&workspace_root, &plan_id).exists());
            let snapshot = app
                .shell_projection_snapshot("Legion")
                .expect("projection snapshot is available");
            assert!(
                snapshot
                    .delegated_task_projection
                    .chat_messages
                    .iter()
                    .any(|message| {
                        message.role == legion_protocol::DelegatedTaskChatRole::System
                            && message.content_label.contains("acp.host.connect")
                    })
            );
            assert!(
                snapshot
                    .delegated_task_projection
                    .chat_messages
                    .iter()
                    .any(|message| {
                        message.role == legion_protocol::DelegatedTaskChatRole::System
                            && message.content_label.contains("acp.host.spawn")
                    })
            );
            assert!(
                snapshot
                    .delegated_task_projection
                    .chat_messages
                    .iter()
                    .any(|message| {
                        message.role == legion_protocol::DelegatedTaskChatRole::System
                            && message.content_label.contains("acp.host.terminate success")
                    })
            );
        }
        other => panic!("expected ProposalReady, got {other:?}"),
    }
}

#[test]
fn delegate_hunk_review_updates_projection_counts_and_rejects_unknown_hunk() {
    let root = temp_workspace("hunk");
    fs::write(root.join("lib.rs"), "pub fn original() {}\n")
        .expect("fixture file should be written");
    let mut app = AppComposition::new();
    app.open_workspace(
        &root,
        WorkspaceTrustState::Trusted,
        PrincipalId("delegate-hunk-test".to_string()),
    )
    .expect("workspace should open");
    app.open_file("lib.rs").expect("fixture file should open");
    app.set_product_mode(AppProductMode::Delegate);
    // Fixture path: live providers register Assist proposals asynchronously on poll.
    app.set_preferred_ai_provider(legion_app::ProductAiProviderPreference::Deterministic);
    let proposal_id = app
        .start_ai_proposal("add delegated guard")
        .expect("proposal run should complete")
        .proposal_id
        .expect("proposal id should be present");
    let snapshot = app
        .shell_projection_snapshot("delegate-hunk")
        .expect("snapshot should build");
    let review = snapshot
        .delegated_task_projection
        .proposal_reviews
        .iter()
        .find(|review| review.proposal_id == proposal_id)
        .expect("proposal review should be projected");
    let hunk_id = review
        .hunks
        .first()
        .expect("at least one hunk should be projected")
        .hunk_id
        .clone();

    let accepted = app
        .review_delegate_proposal_hunk(
            proposal_id,
            hunk_id.clone(),
            DelegatedTaskProposalHunkDisposition::Accepted,
        )
        .expect("known hunk should be reviewable");
    let accepted_review = accepted
        .proposal_reviews
        .iter()
        .find(|review| review.proposal_id == proposal_id)
        .expect("accepted review should be present");
    assert_eq!(accepted_review.accepted_hunk_count, 1);
    assert_eq!(accepted_review.pending_hunk_count, 0);
    assert!(accepted_review.ready_for_apply);

    let rejected = app
        .review_delegate_proposal_hunk(
            proposal_id,
            hunk_id,
            DelegatedTaskProposalHunkDisposition::Rejected,
        )
        .expect("known hunk should remain reviewable");
    let rejected_review = rejected
        .proposal_reviews
        .iter()
        .find(|review| review.proposal_id == proposal_id)
        .expect("rejected review should be present");
    assert_eq!(rejected_review.rejected_hunk_count, 1);
    assert_eq!(rejected_review.pending_hunk_count, 0);
    assert!(!rejected_review.ready_for_apply);

    assert!(
        app.review_delegate_proposal_hunk(
            proposal_id,
            "missing-hunk",
            DelegatedTaskProposalHunkDisposition::Accepted,
        )
        .is_err()
    );
}

/// An authorized run does not report itself blocked.
///
/// The context manifest is built before the broker answers, so its provider
/// permission starts ungranted -- true until the broker grants, and a record of
/// a permission never given after that. The privacy inspector reads an ungranted
/// model-provider permission as a denial and turns it into a refusal, so the
/// approval checklist reported blockers on every Assist proposal that had in
/// fact been authorized. A reviewer who sees blockers on a run nothing blocked
/// learns to click past them, which is the opposite of what a checklist is for.
#[test]
fn an_authorized_assist_run_records_the_permission_it_was_given() {
    let root = temp_workspace("assist_granted_permission");
    fs::write(
        root.join("lib.rs"),
        "pub fn marker() -> u32 {
    42
}
",
    )
    .expect("fixture file should be written");
    let mut app = AppComposition::new();
    app.open_workspace(
        &root,
        WorkspaceTrustState::Trusted,
        PrincipalId("assist-granted".to_string()),
    )
    .expect("workspace should open");
    app.open_file("lib.rs").expect("fixture file should open");
    app.set_product_mode(AppProductMode::Assist);
    app.set_preferred_ai_provider(legion_app::ProductAiProviderPreference::Deterministic);

    let outcome = app
        .start_ai_proposal("add a guard")
        .expect("the deterministic route must produce a proposal");

    // The inspector is a projection of the manifest, so it is what a reviewer
    // actually reads -- and an ungranted model-provider permission becomes a
    // denied record and a refusal there. Asked of the counts, not of the debug
    // text: `denied_record_count: 0` contains the word "denied".
    assert_eq!(
        outcome.privacy_inspector_projection.denied_record_count, 0,
        "the privacy inspector counts a denial for a run the broker authorized"
    );
    assert!(
        outcome.privacy_inspector_projection.refusal.is_none(),
        "the privacy inspector carries a refusal for a run nothing refused: {:?}",
        outcome.privacy_inspector_projection.refusal
    );

    let provider_permission = outcome
        .context_manifest_projection
        .manifest
        .permissions
        .iter()
        .find(|permission| permission.capability.0 == "ai.provider.invoke")
        .expect("the manifest must carry the provider permission it asked for");
    assert!(
        provider_permission.granted,
        "the broker granted this run and the manifest still records the permission as          never given"
    );
    assert!(
        provider_permission.decision_id.is_some(),
        "a granted permission must name the decision that granted it"
    );
}

/// A refused worker leaves an answered turn, not a question hanging.
///
/// The spawn is attempted after the user message, its citations and the
/// permission record are already in the transcript. Returning an error there
/// left the question standing with no reply and no explanation on screen, and
/// asking again appended a second copy of all of it -- so the record of what
/// was asked stopped matching what happened.
#[test]
fn a_delegate_turn_whose_worker_cannot_start_is_still_answered() {
    let root = temp_workspace("chat_spawn_failure");
    fs::write(
        root.join("lib.rs"),
        "pub fn marker() -> u32 {
    42
}
",
    )
    .expect("fixture file should be written");
    let mut app = AppComposition::new();
    app.open_workspace(
        &root,
        WorkspaceTrustState::Trusted,
        PrincipalId("delegate-chat-spawn".to_string()),
    )
    .expect("workspace should open");
    app.open_file("lib.rs").expect("fixture file should open");
    app.set_product_mode(AppProductMode::Delegate);
    app.set_preferred_ai_provider(legion_app::ProductAiProviderPreference::Deterministic);

    app.inject_delegate_chat_spawn_failure_for_test();
    let outcome = app
        .send_delegate_chat("explain marker")
        .expect("a refused worker must not fail the turn; the question is already recorded");

    assert_eq!(
        outcome.projection.chat_message_count, 2,
        "the question was recorded and its answer was not, so the transcript ends on an unanswered turn"
    );
    let answer = outcome
        .projection
        .chat_messages
        .iter()
        .rfind(|message| message.role == legion_protocol::DelegatedTaskChatRole::Assistant)
        .expect("the turn must have an assistant message");
    assert!(
        answer.content_label.contains("could not start a worker"),
        "the answer must say what happened; it said {:?}",
        answer.content_label
    );

    // And the lane is free: a run that never started must not refuse the next
    // one as already in flight.
    app.set_preferred_ai_provider(legion_app::ProductAiProviderPreference::Deterministic);
    let second = app
        .send_delegate_chat("try again")
        .expect("the lane must be free after a spawn that never happened");
    assert_eq!(
        second.projection.chat_message_count, 4,
        "the retry did not produce a turn of its own, so the lane is still held by a worker that does not exist"
    );
}

#[test]
fn delegate_chat_projects_rag_citations_without_raw_source_payload() {
    let root = temp_workspace("chat");
    fs::write(
        root.join("lib.rs"),
        "pub fn delegated_marker() -> u32 {\n 42\n}\n",
    )
    .expect("fixture file should be written");
    let mut app = AppComposition::new();
    app.open_workspace(
        &root,
        WorkspaceTrustState::Trusted,
        PrincipalId("delegate-chat-test".to_string()),
    )
    .expect("workspace should open");
    app.open_file("lib.rs").expect("fixture file should open");
    app.set_product_mode(AppProductMode::Delegate);
    // Keep offline/sync fixture path so CI does not depend on Ollama/BYOK.
    app.set_preferred_ai_provider(legion_app::ProductAiProviderPreference::Deterministic);

    let outcome = app
        .send_delegate_chat("explain delegated_marker")
        .expect("delegate chat should complete");

    assert_eq!(outcome.projection.chat_message_count, 2);
    assert!(outcome.citation_count > 0);
    assert!(outcome.projection.chat_messages.iter().any(|message| {
        message.role == legion_protocol::DelegatedTaskChatRole::Assistant
            && message
                .content_label
                .contains("Delegate provider answer ready")
    }));
    let citation = outcome
        .projection
        .context_citations
        .first()
        .expect("at least one citation should be projected");
    assert!(
        citation
            .path
            .as_ref()
            .is_some_and(|path| path.0.ends_with("lib.rs"))
    );
    assert!(citation.byte_range.is_some());
    assert!(citation.chunk_hash.is_some());
    assert!(
        outcome
            .projection
            .context_citations
            .iter()
            .all(|citation| !citation.metadata_label.contains("42"))
    );
    assert_eq!(outcome.projection.tool_permission_request_count, 1);
}

/// Build a repo-scoped `DelegatedTaskScope` for test workspace at `root`.
fn test_scope(root: &std::path::Path) -> DelegatedTaskScope {
    DelegatedTaskScope {
        target_kind: DelegatedTaskScopeTargetKind::Repo,
        workspace_root: CanonicalPath(root.to_string_lossy().into_owned()),
        target_path: None,
        risk_tolerance: DelegatedTaskRiskTolerance::Balanced,
        allowed_tools: vec![
            LegionToolKind::Read,
            LegionToolKind::Grep,
            LegionToolKind::Glob,
            LegionToolKind::Outline,
            LegionToolKind::EditAsProposal,
        ],
        forbidden_paths: vec![],
        schema_version: 1,
    }
}

#[test]
fn start_delegated_task_completes_with_scripted_end_turn() {
    let root = temp_workspace("start-task-complete");
    fs::write(root.join("hello.rs"), "fn hello() {}\n").expect("fixture file should be written");
    let mut app = AppComposition::new();
    app.open_workspace(
        &root,
        WorkspaceTrustState::Trusted,
        PrincipalId("start-task-test".to_string()),
    )
    .expect("workspace should open");
    app.set_product_mode(AppProductMode::Delegate);

    let provider = ScriptedToolCallingProviderBuilder::new()
        .end_turn("Task complete: read the file and understood the structure.")
        .build("test-scripted");

    let outcome = app
        .start_delegated_task(
            "Describe the structure of hello.rs".to_string(),
            test_scope(&root),
            &provider,
        )
        .expect("start_delegated_task should succeed");

    match outcome {
        AppDelegatedTaskOutcome::Completed {
            final_message,
            proposals,
            audit_steps,
        } => {
            assert!(
                final_message.contains("Task complete"),
                "final message should include scripted text; got: {final_message}"
            );
            // TODO(PKT-PROPOSAL-SURFACE): proposals will be non-empty once DelegatedTaskLoopResult surfaces them
            assert_eq!(
                proposals.len(),
                0,
                "no proposals expected from end_turn only run"
            );
            assert!(
                !audit_steps.is_empty(),
                "at least one audit step should be recorded"
            );
        }
        other => panic!("expected Completed, got {other:?}"),
    }
}

#[test]
fn start_delegated_task_audit_steps_are_paired_for_tool_call() {
    use legion_protocol::DelegatedTaskLoopStepKind;

    let root = temp_workspace("start-task-paired");
    fs::write(root.join("target.rs"), "fn target() {}\n").expect("fixture file should be written");
    let mut app = AppComposition::new();
    app.open_workspace(
        &root,
        WorkspaceTrustState::Trusted,
        PrincipalId("start-task-paired-test".to_string()),
    )
    .expect("workspace should open");
    app.set_product_mode(AppProductMode::Delegate);

    let provider = ScriptedToolCallingProviderBuilder::new()
        .tool_use("tool-1", "read", serde_json::json!({ "path": "target.rs" }))
        .end_turn("Read target.rs successfully.")
        .build("test-scripted-paired");

    let outcome = app
        .start_delegated_task(
            "Read target.rs and summarize".to_string(),
            test_scope(&root),
            &provider,
        )
        .expect("start_delegated_task should succeed");

    match outcome {
        AppDelegatedTaskOutcome::Completed { audit_steps, .. } => {
            // There must be a ToolCallRequest step paired with a ToolCallResult.
            let request_steps: Vec<_> = audit_steps
                .iter()
                .filter(|s| s.kind == DelegatedTaskLoopStepKind::ToolCallRequest)
                .collect();
            let result_steps: Vec<_> = audit_steps
                .iter()
                .filter(|s| {
                    s.kind == DelegatedTaskLoopStepKind::ToolCallResult
                        || s.kind == DelegatedTaskLoopStepKind::ToolCallRejected
                })
                .collect();

            assert_eq!(
                request_steps.len(),
                result_steps.len(),
                "every ToolCallRequest must have a paired result/rejection"
            );

            for request in &request_steps {
                let paired = result_steps
                    .iter()
                    .any(|r| r.causality_id == request.causality_id);
                assert!(
                    paired,
                    "request with causality_id={} has no paired result",
                    request.causality_id
                );
            }
        }
        other => panic!("expected Completed, got {other:?}"),
    }
}

#[test]
fn start_delegated_task_rejects_manual_mode() {
    let root = temp_workspace("start-task-manual-reject");
    let mut app = AppComposition::new();
    app.open_workspace(
        &root,
        WorkspaceTrustState::Trusted,
        PrincipalId("manual-reject-test".to_string()),
    )
    .expect("workspace should open");
    // Manual mode (default): should reject.

    let provider = ScriptedToolCallingProviderBuilder::new()
        .end_turn("should not reach here")
        .build("test-scripted-reject");

    let err = app
        .start_delegated_task(
            "attempt in manual mode".to_string(),
            test_scope(&root),
            &provider,
        )
        .expect_err("manual mode should reject start_delegated_task");

    assert!(
        err.to_string().contains("Delegate dispatch requires"),
        "error should mention delegate requirement; got: {err}"
    );
}

#[test]
fn reap_orphaned_delegated_task_sandboxes_removes_preseeded_orphan_and_reports_it() {
    let reap_root =
        std::env::temp_dir().join(format!("legion_app_reap_test_{}", uuid::Uuid::now_v7()));
    fs::create_dir_all(reap_root.join("task-orphan-plan")).expect("orphan dir should be created");
    fs::write(
        reap_root.join("task-orphan-plan/marker.txt"),
        "stale sandbox from a crashed lane",
    )
    .expect("marker file should be written");
    fs::create_dir_all(reap_root.join("not-a-task-dir")).expect("unrelated dir should be created");

    let removed = AppComposition::reap_orphaned_delegated_task_sandboxes_at(&reap_root)
        .expect("reap should succeed");

    assert_eq!(removed.len(), 1);
    assert!(removed[0].ends_with("task-orphan-plan"));
    assert!(
        !reap_root.join("task-orphan-plan").exists(),
        "orphaned sandbox should be removed"
    );
    assert!(
        reap_root.join("not-a-task-dir").exists(),
        "non-task directories must be left untouched"
    );

    let _ = fs::remove_dir_all(&reap_root);
}

#[test]
#[cfg(feature = "ai")]
fn app_delegated_tool_host_denies_command_when_isolation_is_incomplete() {
    use legion_agent::agent_loop::DelegatedToolHost;

    let tmp = temp_workspace("tool-host-echo");
    let host = AppDelegatedToolHost::new(tmp.root.clone(), std::collections::BTreeSet::new());

    let error = host
        .run_terminal_command("echo hello", None, None)
        .expect_err("current sandbox backends lack required read isolation");

    assert!(
        error.contains("terminal command denied"),
        "error should explain the fail-closed denial; got: {error}"
    );
    assert!(
        error.contains("sandbox live enforcement:"),
        "tool host must surface live SandboxEnforcementReport; got: {error}"
    );
    assert!(
        host.last_enforcement_summary()
            .is_some_and(|s| s.contains("sandbox live enforcement:")),
        "last_enforcement_summary must be populated after spawn"
    );
}

#[test]
fn start_delegated_task_rejects_forbidden_path_read() {
    use legion_protocol::DelegatedTaskLoopStepKind;

    let root = temp_workspace("start-task-forbidden");
    fs::write(root.join("secrets.txt"), "top secret data\n")
        .expect("fixture file should be written");
    let mut app = AppComposition::new();
    app.open_workspace(
        &root,
        WorkspaceTrustState::Trusted,
        PrincipalId("forbidden-path-test".to_string()),
    )
    .expect("workspace should open");
    app.set_product_mode(AppProductMode::Delegate);

    // Scope forbids reading secrets.txt. The loop resolves tool paths against
    // the sandbox worktree and then maps them back to workspace-absolute paths,
    // so the forbidden-path entry must be an absolute path.
    let scope = DelegatedTaskScope {
        forbidden_paths: vec![CanonicalPath(
            root.join("secrets.txt").to_string_lossy().into_owned(),
        )],
        ..test_scope(&root)
    };

    // Scripted provider: attempt to read the forbidden file, then end turn.
    let provider = ScriptedToolCallingProviderBuilder::new()
        .tool_use(
            "tool-forbidden",
            "read",
            serde_json::json!({ "path": "secrets.txt" }),
        )
        .end_turn("Done after forbidden read attempt.")
        .build("test-scripted-forbidden");

    let outcome = app
        .start_delegated_task("Try to read secrets.txt".to_string(), scope, &provider)
        .expect("start_delegated_task should succeed even with a rejected tool call");

    // A non-retryable ScopeDenied rejection stops the loop with Blocked.
    // The audit_steps carried by Blocked must include the ToolCallRejected entry.
    match outcome {
        AppDelegatedTaskOutcome::Blocked { audit_steps, .. } => {
            let rejected_steps: Vec<_> = audit_steps
                .iter()
                .filter(|s| s.kind == DelegatedTaskLoopStepKind::ToolCallRejected)
                .collect();
            assert!(
                !rejected_steps.is_empty(),
                "at least one ToolCallRejected step expected when forbidden path is accessed; \
                 got audit steps: {audit_steps:?}"
            );
        }
        other => panic!("expected Blocked (scope denial is non-retryable), got {other:?}"),
    }
}

#[cfg(all(feature = "ai", windows))]
#[test]
fn windows_scope_alias_and_case_are_normalized_before_forbidden_path_checks() {
    use legion_protocol::DelegatedTaskLoopStepKind;

    let root = temp_workspace("windows-scope-alias");
    fs::write(root.join("secrets.txt"), "top secret data\n").expect("write fixture");
    let mut app = AppComposition::new();
    app.open_workspace(
        &root,
        WorkspaceTrustState::Trusted,
        PrincipalId("windows-scope-alias-test".to_string()),
    )
    .expect("open workspace");
    app.set_product_mode(AppProductMode::Delegate);

    let upper_root = root.to_string_lossy().to_uppercase();
    let aliased_root = format!(r"\\?\{upper_root}");
    let scope = DelegatedTaskScope {
        workspace_root: CanonicalPath(aliased_root.clone()),
        forbidden_paths: vec![CanonicalPath(format!(r"{aliased_root}\SECRETS.TXT"))],
        ..test_scope(&root)
    };
    let provider = ScriptedToolCallingProviderBuilder::new()
        .tool_use(
            "tool-forbidden-windows-alias",
            "read",
            serde_json::json!({ "path": "SECRETS.TXT" }),
        )
        .end_turn("must remain forbidden")
        .build("test-scripted-windows-alias");

    let outcome = app
        .start_delegated_task("Try aliased forbidden path".to_string(), scope, &provider)
        .expect("scope alias should validate and execute fail-closed");
    let AppDelegatedTaskOutcome::Blocked { audit_steps, .. } = outcome else {
        panic!("forbidden path must block, got {outcome:?}");
    };
    assert!(audit_steps.iter().any(|step| {
        step.kind == DelegatedTaskLoopStepKind::ToolCallRejected
            && step
                .reason
                .as_deref()
                .is_some_and(|reason| reason.contains("forbidden"))
    }));
}

#[test]
fn delegated_scope_rejects_dot_traversal_in_nonexistent_target_suffix() {
    let root = temp_workspace("scope-dot-target");
    let mut app = AppComposition::new();
    app.open_workspace(
        &root,
        WorkspaceTrustState::Trusted,
        PrincipalId("scope-dot-target-test".to_string()),
    )
    .expect("open workspace");
    app.set_product_mode(AppProductMode::Delegate);
    let scope = DelegatedTaskScope {
        target_kind: DelegatedTaskScopeTargetKind::Module,
        target_path: Some(CanonicalPath(
            root.join("missing")
                .join("..")
                .join("outside")
                .to_string_lossy()
                .into_owned(),
        )),
        ..test_scope(&root)
    };
    let provider = ScriptedToolCallingProviderBuilder::new()
        .end_turn("must not run")
        .build("scope-dot-target-provider");

    let error = app
        .start_delegated_task("reject traversal".to_string(), scope, &provider)
        .expect_err("parent traversal must be rejected before prefix resolution");

    assert!(error.to_string().contains("parent traversal"));
}

#[test]
fn delegated_scope_rejects_dot_traversal_in_forbidden_path_alias() {
    let root = temp_workspace("scope-dot-forbidden");
    let mut app = AppComposition::new();
    app.open_workspace(
        &root,
        WorkspaceTrustState::Trusted,
        PrincipalId("scope-dot-forbidden-test".to_string()),
    )
    .expect("open workspace");
    app.set_product_mode(AppProductMode::Delegate);
    let scope = DelegatedTaskScope {
        forbidden_paths: vec![CanonicalPath(
            root.join("nonexistent")
                .join("..")
                .join("secrets.txt")
                .to_string_lossy()
                .into_owned(),
        )],
        ..test_scope(&root)
    };
    let provider = ScriptedToolCallingProviderBuilder::new()
        .end_turn("must not run")
        .build("scope-dot-forbidden-provider");

    let error = app
        .start_delegated_task("reject forbidden alias".to_string(), scope, &provider)
        .expect_err("forbidden path alias must be rejected before prefix resolution");

    assert!(error.to_string().contains("parent traversal"));
}

/// End-to-end integration test for the proposal surface path:
/// scripted provider → edit-as-proposal → proposals.len()==1 →
/// id resolves in the ledger projection → review_delegate_proposal_hunk succeeds.
///
/// This test was required by the PKT-PROPOSAL-SURFACE task brief and exercises the
/// fix for the silently-discarded register_proposal_lifecycle error (Finding 1):
/// a proposal that fails registration would not appear in the ledger and
/// review_delegate_proposal_hunk would return "proposal not found".
#[test]
fn start_delegated_task_surfaces_proposal_and_review_succeeds() {
    let root = temp_workspace("proposal-surface");
    fs::write(root.join("hello.rs"), "fn hello() -> u32 { 42 }\n")
        .expect("fixture file should be written");
    let mut app = AppComposition::new();
    app.open_workspace(
        &root,
        WorkspaceTrustState::Trusted,
        PrincipalId("proposal-surface-test".to_string()),
    )
    .expect("workspace should open");
    app.set_product_mode(AppProductMode::Delegate);

    // Scripted provider: read the file, then propose an edit via edit-as-proposal.
    let provider = ScriptedToolCallingProviderBuilder::new()
        .tool_use("t1", "read", serde_json::json!({ "path": "hello.rs" }))
        .tool_use(
            "t2",
            "edit-as-proposal",
            serde_json::json!({
                "path": "hello.rs",
                "replacement": "fn hello() -> u32 { 99 }\n"
            }),
        )
        .end_turn("Proposed an edit to hello.rs.")
        .build("test-scripted-proposal-surface");

    let outcome = app
        .start_delegated_task(
            "Edit hello.rs to return 99".to_string(),
            test_scope(&root),
            &provider,
        )
        .expect("start_delegated_task should succeed");

    match outcome {
        AppDelegatedTaskOutcome::Completed { proposals, .. } => {
            // The edit-as-proposal tool call must surface exactly one proposal.
            assert_eq!(
                proposals.len(),
                1,
                "expected 1 proposal from edit-as-proposal; got {}",
                proposals.len()
            );

            let proposal = &proposals[0];

            // The proposal must reference hello.rs.
            let targets_hello = match &proposal.payload {
                ProposalPayload::CreateFile(p) => {
                    p.path.0.ends_with("hello.rs") || p.path.0.contains("hello.rs")
                }
                _ => false,
            };
            assert!(
                targets_hello,
                "proposal should target hello.rs; got: {:?}",
                proposal.payload
            );

            // The proposal must be resolvable in the app's ledger. A phantom proposal
            // (one where register_proposal_lifecycle was silently discarded) would cause
            // review_delegate_proposal_hunk to return "proposal not found".
            // Retrieve the hunk_id from the shell projection rather than constructing
            // it manually: the exact chunk id format is an implementation detail.
            let proposal_id = proposal.proposal_id;
            let snapshot = app
                .shell_projection_snapshot("proposal-surface-review")
                .expect("snapshot should build");
            let review = snapshot
                .delegated_task_projection
                .proposal_reviews
                .iter()
                .find(|review| review.proposal_id == proposal_id)
                .expect(
                    "registered proposal must appear in the ledger projection — \
                     if registration was silently discarded no review would be projected",
                );
            let hunk_id = review
                .hunks
                .first()
                .expect("at least one hunk should be projected for the edit-as-proposal")
                .hunk_id
                .clone();
            app.review_delegate_proposal_hunk(
                proposal_id,
                hunk_id,
                DelegatedTaskProposalHunkDisposition::Accepted,
            )
            .expect(
                "proposal hunk must be reviewable via the app ledger — \
                 if registration was silently discarded this call would fail with \
                 'proposal not found'",
            );
        }
        other => panic!("expected Completed, got {other:?}"),
    }
}
