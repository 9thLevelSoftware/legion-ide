#![cfg(feature = "ai")]

//! GP-4 Golden Path smoke runner for Legion Workflows (M11).
//!
//! Invoked by `cargo run -p xtask -- golden-path-4`. The runner uses the real
//! app composition APIs and deterministic local providers; it does not make
//! hosted provider calls.

use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
    process,
    sync::{Arc, Barrier, Mutex, mpsc},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use legion_agent::{LegionWorkflowCoordinatorOutput, comm::AgentCommTag};
use legion_ai::{
    ChatCompletionRequest, ChatCompletionResponse, EmbeddingRequest, EmbeddingResponse,
    ModelProvider, ProviderCapabilities, ProviderError, ProviderId,
    tool_calls::{
        ScriptedToolCallingProviderBuilder, ToolCallingProvider, ToolCompletionRequest,
        ToolCompletionResponse, ToolCompletionStopReason, ToolTurnBlock,
    },
};
use legion_app::{
    AppComposition, AppProductMode, LegionWorkerProviderResolver, SharedCancellationFlag,
};
use legion_protocol::{
    ByteRange, CausalityId, CommandRiskLabel, CorrelationId, DelegatedTaskAffectedTargetSummary,
    DelegatedTaskOperationClass, DelegatedTaskPlanId, DelegatedTaskPlanningBoundaryInput,
    DelegatedTaskStepState, DirectiveArtifact, EditablePlanSectionKind, FileFingerprint,
    LegionWorkflowConflict, LegionWorkflowConflictId, LegionWorkflowConflictKind,
    LegionWorkflowConflictState, LegionWorkflowDecisionKind, LegionWorkflowMergeApproval,
    LegionWorkflowMergeReadinessBlocker, LegionWorkflowMergeReadinessState,
    LegionWorkflowModelBackend, LegionWorkflowSession, LegionWorkflowSessionId,
    LegionWorkflowSignOff, LegionWorkflowSignOffId, LegionWorkflowSignOffState,
    LegionWorkflowState, LegionWorkflowVerificationGate, LegionWorkflowVerificationGateId,
    LegionWorkflowVerificationGateState, LegionWorkflowWorkerAssignment, LegionWorkflowWorkerId,
    LegionWorkflowWorkerRole, LegionWorkflowWorkerState, PrincipalId, PrivacyClassification,
    ProductMode, ProposalId, ProposalPrivacyLabel, ProposalRiskLabel, ProposalTargetKind,
    RedactionHint, SpecArtifact, TaskGraphArtifact, TaskNode, TimestampMillis, WorkspaceId,
    WorkspaceTrustState, delegated_task_plan_from_boundary_input,
};
use serde_json::json;

#[derive(Debug, Clone, PartialEq, Eq)]
enum StepStatus {
    Passed,
    Failed,
}

impl StepStatus {
    fn as_str(&self) -> &'static str {
        match self {
            Self::Passed => "passed",
            Self::Failed => "failed",
        }
    }
}

struct StepRecord {
    id: &'static str,
    started_utc: String,
    finished_utc: String,
    duration_ms: u128,
    status: StepStatus,
    detail: String,
}

struct Args {
    fixture_dir: PathBuf,
    out_dir: PathBuf,
    evidence_dir: Option<PathBuf>,
}

struct Gp4Context {
    temp_dir: PathBuf,
    app: AppComposition,
    plan_id: Option<String>,
    main_session_id: Option<LegionWorkflowSessionId>,
    main_worker_ids: Vec<LegionWorkflowWorkerId>,
    main_session_snapshot: Option<LegionWorkflowSession>,
}

struct QueueWorkerProviderResolver {
    providers: Mutex<Vec<Box<dyn ToolCallingProvider + Send>>>,
}

impl QueueWorkerProviderResolver {
    fn new(providers: Vec<Box<dyn ToolCallingProvider + Send>>) -> Self {
        Self {
            providers: Mutex::new(providers.into_iter().rev().collect()),
        }
    }
}

impl LegionWorkerProviderResolver for QueueWorkerProviderResolver {
    fn resolve_worker_provider(
        &self,
        _assignment: &LegionWorkflowWorkerAssignment,
    ) -> Option<Box<dyn ToolCallingProvider + Send>> {
        self.providers.lock().expect("providers lock").pop()
    }
}

struct NamedWorkerProviderResolver {
    providers: Mutex<HashMap<String, Box<dyn ToolCallingProvider + Send>>>,
}

impl NamedWorkerProviderResolver {
    fn new(
        providers: impl IntoIterator<Item = (String, Box<dyn ToolCallingProvider + Send>)>,
    ) -> Self {
        Self {
            providers: Mutex::new(providers.into_iter().collect()),
        }
    }
}

impl LegionWorkerProviderResolver for NamedWorkerProviderResolver {
    fn resolve_worker_provider(
        &self,
        assignment: &LegionWorkflowWorkerAssignment,
    ) -> Option<Box<dyn ToolCallingProvider + Send>> {
        self.providers
            .lock()
            .expect("providers lock")
            .remove(&assignment.worker_id.0)
    }
}

struct BarrierEditProvider {
    id: ProviderId,
    worker_id: String,
    replacement: String,
    barrier: Arc<Barrier>,
    timeout: Duration,
    dispatch_log: Arc<Mutex<Vec<String>>>,
    cursor: Mutex<usize>,
}

impl BarrierEditProvider {
    fn new(
        worker_id: &str,
        replacement: &str,
        barrier: Arc<Barrier>,
        dispatch_log: Arc<Mutex<Vec<String>>>,
    ) -> Self {
        Self {
            id: format!("provider:{worker_id}"),
            worker_id: worker_id.to_string(),
            replacement: replacement.to_string(),
            barrier,
            timeout: Duration::from_secs(2),
            dispatch_log,
            cursor: Mutex::new(0),
        }
    }
}

impl ModelProvider for BarrierEditProvider {
    fn provider_id(&self) -> ProviderId {
        self.id.clone()
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

impl ToolCallingProvider for BarrierEditProvider {
    fn complete_with_tools(
        &self,
        request: ToolCompletionRequest,
    ) -> Result<ToolCompletionResponse, ProviderError> {
        let mut cursor = self.cursor.lock().expect("cursor lock");
        let turn = *cursor;
        *cursor += 1;
        drop(cursor);

        match turn {
            0 => {
                self.dispatch_log
                    .lock()
                    .expect("dispatch log lock")
                    .push(format!("barrier-enter:{}", self.worker_id));
                let (tx, rx) = mpsc::channel();
                let barrier = self.barrier.clone();
                std::thread::spawn(move || {
                    barrier.wait();
                    let _ = tx.send(());
                });
                rx.recv_timeout(self.timeout)
                    .map_err(|_| ProviderError::RequestFailed {
                        provider: self.id.clone(),
                        message: format!("lane barrier timeout for {}", self.worker_id),
                    })?;
                self.dispatch_log
                    .lock()
                    .expect("dispatch log lock")
                    .push(format!("barrier-pass:{}", self.worker_id));
                Ok(ToolCompletionResponse {
                    provider: self.id.clone(),
                    model: request.model,
                    blocks: vec![ToolTurnBlock::ToolUse {
                        id: format!("edit-{}", self.worker_id),
                        name: "edit-as-proposal".to_string(),
                        input: json!({
                            "path": "main.txt",
                            "replacement": self.replacement,
                            "proposal_title": format!("GP-4 parallel lane {}", self.worker_id),
                            "proposal_reason": "GP-4 workflow command center proof",
                        }),
                    }],
                    stop_reason: ToolCompletionStopReason::ToolUse,
                })
            }
            1 => {
                self.dispatch_log
                    .lock()
                    .expect("dispatch log lock")
                    .push(format!("completed:{}", self.worker_id));
                Ok(ToolCompletionResponse {
                    provider: self.id.clone(),
                    model: request.model,
                    blocks: vec![ToolTurnBlock::Text(format!(
                        "{} completed with proposal output.",
                        self.worker_id
                    ))],
                    stop_reason: ToolCompletionStopReason::EndTurn,
                })
            }
            _ => Err(ProviderError::RequestFailed {
                provider: self.id.clone(),
                message: format!("provider {} exhausted", self.worker_id),
            }),
        }
    }
}

struct LoggingEditProvider {
    id: ProviderId,
    worker_id: String,
    replacement: String,
    dispatch_log: Arc<Mutex<Vec<String>>>,
    cursor: Mutex<usize>,
}

impl LoggingEditProvider {
    fn new(worker_id: &str, replacement: &str, dispatch_log: Arc<Mutex<Vec<String>>>) -> Self {
        Self {
            id: format!("provider:{worker_id}"),
            worker_id: worker_id.to_string(),
            replacement: replacement.to_string(),
            dispatch_log,
            cursor: Mutex::new(0),
        }
    }
}

impl ModelProvider for LoggingEditProvider {
    fn provider_id(&self) -> ProviderId {
        self.id.clone()
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

impl ToolCallingProvider for LoggingEditProvider {
    fn complete_with_tools(
        &self,
        request: ToolCompletionRequest,
    ) -> Result<ToolCompletionResponse, ProviderError> {
        let mut cursor = self.cursor.lock().expect("cursor lock");
        let turn = *cursor;
        *cursor += 1;
        drop(cursor);

        match turn {
            0 => {
                self.dispatch_log
                    .lock()
                    .expect("dispatch log lock")
                    .push(format!("dispatch:{}", self.worker_id));
                Ok(ToolCompletionResponse {
                    provider: self.id.clone(),
                    model: request.model,
                    blocks: vec![ToolTurnBlock::ToolUse {
                        id: format!("edit-{}", self.worker_id),
                        name: "edit-as-proposal".to_string(),
                        input: json!({
                            "path": "main.txt",
                            "replacement": self.replacement,
                            "proposal_title": format!("GP-4 ordered lane {}", self.worker_id),
                            "proposal_reason": "GP-4 dependency ordering proof",
                        }),
                    }],
                    stop_reason: ToolCompletionStopReason::ToolUse,
                })
            }
            1 => Ok(ToolCompletionResponse {
                provider: self.id.clone(),
                model: request.model,
                blocks: vec![ToolTurnBlock::Text(format!(
                    "{} completed with proposal output.",
                    self.worker_id
                ))],
                stop_reason: ToolCompletionStopReason::EndTurn,
            }),
            _ => Err(ProviderError::RequestFailed {
                provider: self.id.clone(),
                message: format!("provider {} exhausted", self.worker_id),
            }),
        }
    }
}

struct RepeatedInvalidReadProvider {
    id: ProviderId,
}

impl RepeatedInvalidReadProvider {
    fn new(provider_id: &str) -> Self {
        Self {
            id: provider_id.to_string(),
        }
    }
}

impl ModelProvider for RepeatedInvalidReadProvider {
    fn provider_id(&self) -> ProviderId {
        self.id.clone()
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

impl ToolCallingProvider for RepeatedInvalidReadProvider {
    fn complete_with_tools(
        &self,
        request: ToolCompletionRequest,
    ) -> Result<ToolCompletionResponse, ProviderError> {
        Ok(ToolCompletionResponse {
            provider: self.id.clone(),
            model: request.model,
            blocks: vec![ToolTurnBlock::ToolUse {
                id: "invalid-read".to_string(),
                name: "read".to_string(),
                input: json!({}),
            }],
            stop_reason: ToolCompletionStopReason::ToolUse,
        })
    }
}

struct CancelOnSecondTurnProvider {
    id: ProviderId,
    flag: SharedCancellationFlag,
    cancelled_at: Arc<Mutex<Option<Instant>>>,
    cursor: Mutex<usize>,
}

impl CancelOnSecondTurnProvider {
    fn new(flag: SharedCancellationFlag, cancelled_at: Arc<Mutex<Option<Instant>>>) -> Self {
        Self {
            id: "provider:gp4-cancel-turn-two".to_string(),
            flag,
            cancelled_at,
            cursor: Mutex::new(0),
        }
    }
}

impl ModelProvider for CancelOnSecondTurnProvider {
    fn provider_id(&self) -> ProviderId {
        self.id.clone()
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

impl ToolCallingProvider for CancelOnSecondTurnProvider {
    fn complete_with_tools(
        &self,
        request: ToolCompletionRequest,
    ) -> Result<ToolCompletionResponse, ProviderError> {
        let mut cursor = self.cursor.lock().expect("cursor lock");
        let turn = *cursor;
        *cursor += 1;
        drop(cursor);

        match turn {
            0 => Ok(ToolCompletionResponse {
                provider: self.id.clone(),
                model: request.model,
                blocks: vec![ToolTurnBlock::ToolUse {
                    id: "read-before-cancel".to_string(),
                    name: "read".to_string(),
                    input: json!({ "path": "main.txt" }),
                }],
                stop_reason: ToolCompletionStopReason::ToolUse,
            }),
            1 => {
                *self.cancelled_at.lock().expect("cancelled_at lock") = Some(Instant::now());
                self.flag.cancel();
                Ok(ToolCompletionResponse {
                    provider: self.id.clone(),
                    model: request.model,
                    blocks: vec![ToolTurnBlock::ToolUse {
                        id: "read-after-cancel".to_string(),
                        name: "read".to_string(),
                        input: json!({ "path": "main.txt" }),
                    }],
                    stop_reason: ToolCompletionStopReason::ToolUse,
                })
            }
            _ => Err(ProviderError::RequestFailed {
                provider: self.id.clone(),
                message: "cancel-on-second-turn provider exhausted".to_string(),
            }),
        }
    }
}

fn parse_args() -> Result<Args, String> {
    let args: Vec<String> = std::env::args().collect();
    let mut fixture_dir: Option<PathBuf> = None;
    let mut out_dir: Option<PathBuf> = None;
    let mut evidence_dir: Option<PathBuf> = None;
    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--fixture-dir" => {
                i += 1;
                fixture_dir = Some(PathBuf::from(
                    args.get(i).ok_or("--fixture-dir needs value")?,
                ));
            }
            "--out-dir" => {
                i += 1;
                out_dir = Some(PathBuf::from(args.get(i).ok_or("--out-dir needs value")?));
            }
            "--record-evidence" => {
                i += 1;
                evidence_dir = Some(PathBuf::from(
                    args.get(i).ok_or("--record-evidence needs value")?,
                ));
            }
            _ => {}
        }
        i += 1;
    }
    Ok(Args {
        fixture_dir: fixture_dir.ok_or("--fixture-dir required")?,
        out_dir: out_dir.unwrap_or_else(|| PathBuf::from("target/golden-path")),
        evidence_dir,
    })
}

fn epoch_secs_to_rfc3339(secs: u64) -> String {
    let days = secs / 86400;
    let rem = secs % 86400;
    let h = rem / 3600;
    let m = (rem % 3600) / 60;
    let s = rem % 60;
    let (year, month, day) = days_to_ymd(days as i64);
    format!("{year:04}-{month:02}-{day:02}T{h:02}:{m:02}:{s:02}Z")
}

fn days_to_ymd(days: i64) -> (u32, u32, u32) {
    let z = days + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = (z - era * 146097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let mon = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe as i64 + era * 400;
    let y = if mon <= 2 { y + 1 } else { y };
    (y as u32, mon as u32, d as u32)
}

fn utc_now() -> String {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    epoch_secs_to_rfc3339(now.as_secs())
}

fn copy_dir_recursive(src: &Path, dst: &Path) -> Result<(), String> {
    fs::create_dir_all(dst).map_err(|e| format!("create dir {}: {e}", dst.display()))?;
    for entry in fs::read_dir(src).map_err(|e| format!("read dir {}: {e}", src.display()))? {
        let entry = entry.map_err(|e| format!("read entry: {e}"))?;
        let ft = entry.file_type().map_err(|e| format!("file type: {e}"))?;
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());
        if ft.is_dir() {
            copy_dir_recursive(&src_path, &dst_path)?;
        } else if ft.is_file() {
            fs::copy(&src_path, &dst_path).map_err(|e| {
                format!("copy {} -> {}: {e}", src_path.display(), dst_path.display())
            })?;
        }
    }
    Ok(())
}

fn git_cmd(dir: &Path, args: &[&str]) -> Result<String, String> {
    let output = process::Command::new("git")
        .current_dir(dir)
        .args(args)
        .output()
        .map_err(|e| format!("git {:?} spawn failed: {e}", args))?;
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).into_owned())
    } else {
        Err(format!(
            "git {:?} failed ({}): {}",
            args,
            output.status,
            String::from_utf8_lossy(&output.stderr)
        ))
    }
}

fn resolve_legion_git_sha(workspace_root: &Path) -> String {
    git_cmd(workspace_root, &["rev-parse", "--short", "HEAD"])
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|_| "unknown".to_string())
}

fn fingerprint(value: &str) -> FileFingerprint {
    FileFingerprint {
        algorithm: "sha256".to_string(),
        value: value.to_string(),
    }
}

fn causality(value: u128) -> CausalityId {
    CausalityId(uuid::Uuid::from_u128(value))
}

fn affected_target(target_id: &str) -> DelegatedTaskAffectedTargetSummary {
    DelegatedTaskAffectedTargetSummary {
        target_id: target_id.to_string(),
        kind: ProposalTargetKind::MetadataOnly,
        workspace_id: Some(WorkspaceId(1)),
        file_id: None,
        buffer_id: None,
        ranges: vec![ByteRange::new(0, 0)],
        hashes: vec![fingerprint(target_id)],
        counts: Vec::new(),
        labels: vec![format!("target:{target_id}")],
        risk_label: ProposalRiskLabel::Low,
        privacy_label: ProposalPrivacyLabel::WorkspaceMetadata,
        redaction_hints: vec![RedactionHint::MetadataOnly],
        schema_version: 1,
    }
}

fn worker(
    worker_id: &str,
    plan_id: Option<DelegatedTaskPlanId>,
    target_id: &str,
    correlation: u64,
) -> LegionWorkflowWorkerAssignment {
    LegionWorkflowWorkerAssignment {
        worker_id: LegionWorkflowWorkerId(worker_id.to_string()),
        role: LegionWorkflowWorkerRole::Implementer,
        state: LegionWorkflowWorkerState::Ready,
        model_backend: LegionWorkflowModelBackend::Local,
        display_safe_model_label: format!("model:{worker_id}"),
        allowed_command_classes: vec![DelegatedTaskOperationClass::DraftProposalMetadata],
        linked_delegated_plan_id: plan_id,
        assisted_ai_route: None,
        affected_targets: vec![affected_target(target_id)],
        risk_labels: vec![CommandRiskLabel::Review],
        privacy_labels: vec![PrivacyClassification::Metadata],
        correlation_id: CorrelationId(correlation),
        causality_id: causality(correlation as u128),
        redaction_hints: vec![RedactionHint::MetadataOnly],
        schema_version: 1,
    }
}

fn verification_gate(state: LegionWorkflowVerificationGateState) -> LegionWorkflowVerificationGate {
    LegionWorkflowVerificationGate {
        gate_id: LegionWorkflowVerificationGateId("verification:unit".to_string()),
        state,
        label: "cargo run -p xtask -- golden-path-4".to_string(),
        evidence_artifact_id: (state == LegionWorkflowVerificationGateState::Passed)
            .then(|| "evidence:gp4".to_string()),
        command_class_label: "cargo-test".to_string(),
        redaction_hints: vec![RedactionHint::MetadataOnly],
        schema_version: 1,
    }
}

fn signoff(state: LegionWorkflowSignOffState) -> LegionWorkflowSignOff {
    LegionWorkflowSignOff {
        sign_off_id: LegionWorkflowSignOffId("signoff:reviewer".to_string()),
        state,
        required_role: LegionWorkflowWorkerRole::Reviewer,
        reviewer_principal_id: (state == LegionWorkflowSignOffState::SignedOff)
            .then(|| PrincipalId("reviewer:gp4".to_string())),
        label: "reviewer sign-off".to_string(),
        redaction_hints: vec![RedactionHint::MetadataOnly],
        schema_version: 1,
    }
}

fn approval(approval_granted: bool) -> LegionWorkflowMergeApproval {
    LegionWorkflowMergeApproval {
        approval_artifact_id: Some("approval:gp4".to_string()),
        approval_granted,
        rollback_available: true,
        audit_persisted_before_success: true,
        main_workspace_dirty_conflict: false,
        proposal_preconditions_stale: false,
        labels: vec!["approval.metadata".to_string()],
        redaction_hints: vec![RedactionHint::MetadataOnly],
        schema_version: 1,
    }
}

fn workflow_session(
    label: &str,
    workers: Vec<LegionWorkflowWorkerAssignment>,
    verification_gates: Vec<LegionWorkflowVerificationGate>,
    sign_off_records: Vec<LegionWorkflowSignOff>,
    proposal_ids: Vec<ProposalId>,
    merge_approval: Option<LegionWorkflowMergeApproval>,
) -> LegionWorkflowSession {
    LegionWorkflowSession {
        session_id: LegionWorkflowSessionId(format!("session:gp4:{label}")),
        directive_artifact_id: Some(format!("directive:gp4:{label}")),
        spec_artifact_id: Some(format!("spec:gp4:{label}")),
        task_graph_artifact_id: Some(format!("task-graph:gp4:{label}")),
        product_mode: ProductMode::LegionWorkflows,
        worker_assignments: workers,
        dependency_edges: Vec::new(),
        conflict_summaries: Vec::new(),
        verification_gates,
        sign_off_records,
        proposal_ids,
        merge_approval,
        lifecycle_state: LegionWorkflowState::Executing,
        generated_at: TimestampMillis(1),
        redaction_hints: vec![RedactionHint::MetadataOnly],
        schema_version: 1,
        correlation_id: CorrelationId(13),
        causality_id: causality(13),
    }
}

fn delegated_contract(plan_id: DelegatedTaskPlanId) -> legion_protocol::DelegatedTaskPlanContract {
    delegated_task_plan_from_boundary_input(DelegatedTaskPlanningBoundaryInput {
        plan_id,
        workspace_id: Some(WorkspaceId(1)),
        objective_summary_hash: fingerprint("delegated-objective"),
        allowed_operation_classes: vec![DelegatedTaskOperationClass::DraftProposalMetadata],
        context_manifest: None,
        privacy_inspector: None,
        permission_budget_projection: None,
        approval_checklist: None,
        checkpoint_rollback: None,
        assisted_ai_projection: None,
        assisted_ai_required: false,
        affected_targets: vec![affected_target("delegated-target")],
        steps: Vec::new(),
        proposal_preview_links: Vec::new(),
        workspace_trust_state: WorkspaceTrustState::Trusted,
        privacy_denied: false,
        permission_budget_denied: false,
        permission_budget_depleted: false,
        approval_checklist_valid: true,
        checkpoint_required: false,
        checkpoint_available: true,
        rollback_required: false,
        rollback_available: true,
        correlation_id: CorrelationId(21),
        causality_id: causality(21),
        created_at: TimestampMillis(1),
        schema_version: 1,
    })
}

fn scripted_main_edit_provider(
    provider_id: &str,
    replacement: &str,
) -> Box<dyn ToolCallingProvider + Send> {
    Box::new(
        ScriptedToolCallingProviderBuilder::new()
            .tool_use("read-main", "read", json!({ "path": "main.txt" }))
            .expect_prior_result_contains("clean")
            .tool_use(
                "edit-main",
                "edit-as-proposal",
                json!({
                    "path": "main.txt",
                    "replacement": replacement,
                    "proposal_title": "GP-4 workflow edit",
                    "proposal_reason": "resolver-backed worker script",
                }),
            )
            .expect_prior_result_contains("Proposal created")
            .end_turn("Workflow worker completed with a proposal.")
            .build(provider_id),
    )
}

fn scripted_rejected_tool_provider(provider_id: &str) -> Box<dyn ToolCallingProvider + Send> {
    Box::new(
        ScriptedToolCallingProviderBuilder::new()
            .tool_use(
                "terminal-denied",
                "terminal-command",
                json!({ "command": "echo forbidden" }),
            )
            .end_turn("should not be reached")
            .build(provider_id),
    )
}

fn directive() -> DirectiveArtifact {
    DirectiveArtifact {
        artifact_id: "artifact:directive:gp4".to_string(),
        directive_id: "directive:gp4".to_string(),
        goal_hash: fingerprint("goal:gp4"),
        scope_labels: vec!["workspace:gp4".to_string()],
        workspace_id: Some(WorkspaceId(7)),
        product_mode: ProductMode::LegionWorkflows,
        policy_profile_id: "policy:metadata-only".to_string(),
        retention_policy_label: "metadata-only".to_string(),
        raw_payload_retained: false,
        correlation_id: CorrelationId(11),
        causality_id: causality(12),
        created_at: TimestampMillis(13),
        redaction_hints: vec![RedactionHint::MetadataOnly],
        schema_version: 1,
    }
}

fn spec() -> SpecArtifact {
    SpecArtifact {
        artifact_id: "artifact:spec:gp4".to_string(),
        directive_id: "directive:gp4".to_string(),
        requirement_hashes: vec![fingerprint("requirement:gp4")],
        design_note_hashes: vec![fingerprint("design:gp4")],
        acceptance_criteria_hashes: vec![fingerprint("acceptance:gp4")],
        constraint_labels: vec!["metadata-only".to_string()],
        retention_policy_label: "metadata-only".to_string(),
        raw_payload_retained: false,
        generated_at: TimestampMillis(14),
        redaction_hints: vec![RedactionHint::MetadataOnly],
        schema_version: 1,
    }
}

fn task_node(task_id: &str, depends_on: Vec<&str>) -> TaskNode {
    TaskNode {
        task_id: task_id.to_string(),
        depends_on: depends_on.into_iter().map(str::to_string).collect(),
        target_labels: vec![format!("target:{task_id}")],
        verification_requirements: vec!["cargo run -p xtask -- golden-path-4".to_string()],
        state: DelegatedTaskStepState::Planned,
        risk_label: ProposalRiskLabel::Low,
        privacy_label: ProposalPrivacyLabel::WorkspaceMetadata,
        redaction_hints: vec![RedactionHint::MetadataOnly],
        schema_version: 1,
    }
}

fn task_graph() -> TaskGraphArtifact {
    TaskGraphArtifact {
        artifact_id: "artifact:task-graph:gp4".to_string(),
        directive_id: "directive:gp4".to_string(),
        nodes: vec![
            task_node("task:gp4:left", vec![]),
            task_node("task:gp4:right", vec![]),
            task_node(
                "task:gp4:dependent",
                vec!["task:gp4:left", "task:gp4:right"],
            ),
        ],
        edge_count: 2,
        blocked_task_count: 0,
        retention_policy_label: "metadata-only".to_string(),
        raw_payload_retained: false,
        generated_at: TimestampMillis(15),
        redaction_hints: vec![RedactionHint::MetadataOnly],
        schema_version: 1,
    }
}

fn builder_config() -> legion_agent::coordinator::LegionWorkflowSessionBuilderConfig {
    legion_agent::coordinator::LegionWorkflowSessionBuilderConfig {
        session_id: "session:gp4:main".to_string(),
        generated_at: TimestampMillis(30),
        correlation_id: CorrelationId(31),
        causality_id: causality(32),
        workspace_id: Some(WorkspaceId(7)),
    }
}

fn run_s1(fixture_dir: &Path) -> Result<Gp4Context, String> {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let temp_dir =
        std::env::temp_dir().join(format!("legion-gp4-smoke-{}-{}", process::id(), nanos));
    copy_dir_recursive(fixture_dir, &temp_dir)?;
    fs::write(temp_dir.join("main.txt"), "clean\n")
        .map_err(|e| format!("s1: write main.txt: {e}"))?;

    git_cmd(&temp_dir, &["init", "-b", "main"])?;
    git_cmd(
        &temp_dir,
        &["config", "user.email", "gp4-smoke@legion.test"],
    )?;
    git_cmd(&temp_dir, &["config", "user.name", "GP-4 Smoke"])?;
    git_cmd(&temp_dir, &["add", "."])?;
    git_cmd(
        &temp_dir,
        &["commit", "-m", "initial: gp4 smoke fixture baseline"],
    )?;

    let mut app = AppComposition::new();
    app.open_workspace(
        &temp_dir,
        WorkspaceTrustState::Trusted,
        PrincipalId("gp4-smoke".to_string()),
    )
    .map_err(|e| format!("s1: open_workspace failed: {e:?}"))?;
    app.open_file("main.txt")
        .map_err(|e| format!("s1: open main.txt failed: {e:?}"))?;
    app.set_product_mode(AppProductMode::Automate);

    Ok(Gp4Context {
        temp_dir,
        app,
        plan_id: None,
        main_session_id: None,
        main_worker_ids: Vec::new(),
        main_session_snapshot: None,
    })
}

fn run_s2(ctx: &mut Gp4Context) -> Result<String, String> {
    let plan = ctx
        .app
        .create_legion_workflow_plan(directive(), Some(spec()), Some(task_graph()));
    if !plan.review_required {
        return Err("s2: newly created plan must require review".to_string());
    }
    if ctx
        .app
        .legion_workflow_dag_for_plan(&plan.artifact_id)
        .is_some()
    {
        return Err("s2: unapproved plan unexpectedly produced a DAG".to_string());
    }
    let err = ctx
        .app
        .create_legion_workflow_session_from_plan(&plan.artifact_id, builder_config())
        .expect_err("unapproved plan should refuse session creation");
    if !err.to_string().contains("requires review") {
        return Err(format!("s2: wrong pre-approval refusal: {err}"));
    }
    ctx.plan_id = Some(plan.artifact_id.clone());
    Ok(format!(
        "plan {} created; DAG/session refused before approval",
        plan.artifact_id
    ))
}

fn run_s3(ctx: &mut Gp4Context) -> Result<String, String> {
    let plan_id = ctx
        .plan_id
        .clone()
        .ok_or_else(|| "s3: missing plan id".to_string())?;
    let latest = ctx
        .app
        .latest_plan_revision(&plan_id)
        .ok_or_else(|| "s3: missing latest plan revision".to_string())?
        .plan;
    let mut sections = latest.sections.clone();
    sections
        .iter_mut()
        .find(|section| section.kind == EditablePlanSectionKind::Design)
        .ok_or_else(|| "s3: missing design section".to_string())?
        .entries
        .push("Drive GP-4 through real workflow command center projections".to_string());
    let revision = ctx
        .app
        .revise_legion_workflow_plan(&plan_id, sections)
        .map_err(|e| format!("s3: revise plan failed: {e:?}"))?;
    if revision.changed_section_count() != 1 {
        return Err(format!(
            "s3: expected one changed section, got {}",
            revision.changed_section_count()
        ));
    }
    ctx.app
        .approve_legion_workflow_plan(&plan_id)
        .map_err(|e| format!("s3: approve plan failed: {e:?}"))?;
    if ctx.app.legion_workflow_dag_for_plan(&plan_id).is_none() {
        return Err("s3: approved plan did not produce a DAG".to_string());
    }
    let revision_count = ctx.app.plan_revisions(&plan_id).len();
    if revision_count < 3 {
        return Err(format!(
            "s3: expected at least 3 revisions, got {revision_count}"
        ));
    }
    Ok(format!(
        "plan revised and approved; revision_count={revision_count}"
    ))
}

fn run_s4(ctx: &mut Gp4Context) -> Result<String, String> {
    let plan_id = ctx
        .plan_id
        .clone()
        .ok_or_else(|| "s4: missing plan id".to_string())?;
    let mut session = ctx
        .app
        .create_legion_workflow_session_from_plan(&plan_id, builder_config())
        .map_err(|e| format!("s4: session build failed: {e:?}"))?;
    session.lifecycle_state = LegionWorkflowState::Executing;
    session.verification_gates = vec![verification_gate(
        LegionWorkflowVerificationGateState::Pending,
    )];
    session.sign_off_records = vec![signoff(LegionWorkflowSignOffState::Pending)];
    let ready_workers = session
        .worker_assignments
        .iter()
        .filter(|worker| {
            !session
                .dependency_edges
                .iter()
                .any(|edge| edge.successor_worker_id == worker.worker_id)
        })
        .count();
    if session.worker_assignments.len() != 3 || ready_workers != 2 {
        return Err(format!(
            "s4: expected three workers with two lane starters, got workers={} starters={ready_workers}",
            session.worker_assignments.len()
        ));
    }
    let session_id = session.session_id.clone();
    let worker_ids = session
        .worker_assignments
        .iter()
        .map(|worker| worker.worker_id.clone())
        .collect::<Vec<_>>();
    ctx.app
        .seed_legion_workflow_sessions(vec![session])
        .map_err(|e| format!("s4: seed main session failed: {e:?}"))?;
    ctx.main_session_id = Some(session_id);
    ctx.main_worker_ids = worker_ids;
    Ok("session built from approved plan with two parallel lane starters".to_string())
}

fn run_s5(ctx: &mut Gp4Context) -> Result<String, String> {
    let session_id = ctx
        .main_session_id
        .clone()
        .ok_or_else(|| "s5: missing main session id".to_string())?;
    if ctx.main_worker_ids.len() != 3 {
        return Err("s5: main session does not have three workers".to_string());
    }
    let left = ctx.main_worker_ids[0].0.clone();
    let right = ctx.main_worker_ids[1].0.clone();
    let dependent = ctx.main_worker_ids[2].0.clone();
    let dispatch_log = Arc::new(Mutex::new(Vec::new()));
    let barrier = Arc::new(Barrier::new(2));
    let resolver = NamedWorkerProviderResolver::new([
        (
            left.clone(),
            Box::new(BarrierEditProvider::new(
                &left,
                "gp4 resolver payload: left\n",
                barrier.clone(),
                dispatch_log.clone(),
            )) as Box<dyn ToolCallingProvider + Send>,
        ),
        (
            right.clone(),
            Box::new(BarrierEditProvider::new(
                &right,
                "gp4 resolver payload: right\n",
                barrier.clone(),
                dispatch_log.clone(),
            )) as Box<dyn ToolCallingProvider + Send>,
        ),
        (
            dependent.clone(),
            Box::new(LoggingEditProvider::new(
                &dependent,
                "gp4 resolver payload: dependent\n",
                dispatch_log.clone(),
            )) as Box<dyn ToolCallingProvider + Send>,
        ),
    ]);
    let outcome = ctx
        .app
        .execute_legion_workflow_with_providers(&session_id, &resolver)
        .map_err(|e| format!("s5: execute workflow failed: {e:?}"))?;
    let proposal_count = outcome
        .outputs
        .iter()
        .filter(|output| matches!(output, LegionWorkflowCoordinatorOutput::ProposalReady(_)))
        .count();
    if proposal_count != 3 {
        return Err(format!("s5: expected 3 proposals, got {proposal_count}"));
    }
    let log = dispatch_log.lock().expect("dispatch log lock").clone();
    let left_pass = log
        .iter()
        .position(|entry| entry == &format!("barrier-pass:{left}"))
        .ok_or_else(|| format!("s5: missing left barrier pass in {log:?}"))?;
    let right_pass = log
        .iter()
        .position(|entry| entry == &format!("barrier-pass:{right}"))
        .ok_or_else(|| format!("s5: missing right barrier pass in {log:?}"))?;
    let dependent_dispatch = log
        .iter()
        .position(|entry| entry == &format!("dispatch:{dependent}"))
        .ok_or_else(|| format!("s5: missing dependent dispatch in {log:?}"))?;
    if dependent_dispatch <= left_pass || dependent_dispatch <= right_pass {
        return Err(format!(
            "s5: dependent dispatched before both lane workers passed barrier: {log:?}"
        ));
    }
    let stored = ctx
        .app
        .legion_workflow_session(&session_id)
        .ok_or_else(|| "s5: missing stored main session".to_string())?;
    if stored
        .worker_assignments
        .iter()
        .any(|worker| worker.state != LegionWorkflowWorkerState::Completed)
    {
        return Err("s5: not all main workers completed".to_string());
    }
    ctx.main_session_snapshot = Some(stored.clone());
    Ok(format!(
        "three proposal-only workers completed; dispatch_log={log:?}"
    ))
}

fn run_s6(ctx: &mut Gp4Context) -> Result<String, String> {
    let (session, plan_id) = single_worker_session("tool-rejected", true);
    let session_id = session.session_id.clone();
    ctx.app
        .seed_delegated_task_plan_contracts(vec![delegated_contract(plan_id)]);
    ctx.app
        .seed_legion_workflow_sessions(vec![session])
        .map_err(|e| format!("s6: seed workflow failed: {e:?}"))?;
    let resolver = QueueWorkerProviderResolver::new(vec![scripted_rejected_tool_provider(
        "gp4-scripted-tool-rejected",
    )]);
    let outcome = ctx
        .app
        .execute_legion_workflow_with_providers(&session_id, &resolver)
        .map_err(|e| format!("s6: execute rejected-tool workflow failed: {e:?}"))?;
    let blocked = outcome.outputs.iter().any(|output| {
        matches!(output, LegionWorkflowCoordinatorOutput::Blocked { reasons, .. }
            if reasons.iter().any(|reason| reason.contains("ToolCallRejected")))
    });
    if !blocked {
        return Err("s6: expected ToolCallRejected blocked output".to_string());
    }
    Ok("policy boundary produced ToolCallRejected evidence".to_string())
}

fn run_s7(ctx: &mut Gp4Context) -> Result<String, String> {
    let (session, plan_id) = single_worker_session("budget-exhausted", true);
    let session_id = session.session_id.clone();
    ctx.app
        .seed_delegated_task_plan_contracts(vec![delegated_contract(plan_id)]);
    ctx.app
        .seed_legion_workflow_sessions(vec![session])
        .map_err(|e| format!("s7: seed workflow failed: {e:?}"))?;
    let resolver = QueueWorkerProviderResolver::new(vec![Box::new(
        RepeatedInvalidReadProvider::new("gp4-budget-invalid-read"),
    )]);
    let outcome = ctx
        .app
        .execute_legion_workflow_with_providers(&session_id, &resolver)
        .map_err(|e| format!("s7: execute budget workflow failed: {e:?}"))?;
    let exhausted = outcome.outputs.iter().any(|output| {
        matches!(output, LegionWorkflowCoordinatorOutput::Blocked { reasons, .. }
            if reasons.iter().any(|reason| reason.contains("worker_budget_exhausted")))
    });
    if !exhausted {
        return Err("s7: expected worker_budget_exhausted blocked output".to_string());
    }
    Ok("retryable invalid tool calls exhausted worker retry budget".to_string())
}

fn run_s8(ctx: &mut Gp4Context) -> Result<String, String> {
    let plan_id = DelegatedTaskPlanId("plan-gp4-validation".to_string());
    let mut session = workflow_session(
        "validation-failed",
        vec![worker(
            "worker:gp4:validation",
            Some(plan_id.clone()),
            "target:validation",
            71,
        )],
        vec![verification_gate(
            LegionWorkflowVerificationGateState::Pending,
        )],
        vec![signoff(LegionWorkflowSignOffState::SignedOff)],
        vec![ProposalId(710)],
        Some(approval(true)),
    );
    session.worker_assignments[0].state = LegionWorkflowWorkerState::Completed;
    let session_id = session.session_id.clone();
    ctx.app
        .seed_delegated_task_plan_contracts(vec![delegated_contract(plan_id)]);
    ctx.app
        .seed_legion_workflow_sessions(vec![session])
        .map_err(|e| format!("s8: seed workflow failed: {e:?}"))?;
    let readiness = ctx
        .app
        .record_legion_workflow_verification(
            &session_id,
            &LegionWorkflowVerificationGateId("verification:unit".to_string()),
            LegionWorkflowVerificationGateState::Failed,
            Some("evidence:gp4:validation-failed".to_string()),
        )
        .map_err(|e| format!("s8: record failed verification failed: {e:?}"))?;
    if readiness.state != LegionWorkflowMergeReadinessState::Blocked
        || !readiness
            .blockers
            .contains(&LegionWorkflowMergeReadinessBlocker::FailedVerification)
    {
        return Err(format!(
            "s8: expected FailedVerification blocker, got {:?}",
            readiness
        ));
    }
    Ok("failed verification gate blocked merge readiness".to_string())
}

fn run_s9(ctx: &mut Gp4Context) -> Result<String, String> {
    let plan_id = DelegatedTaskPlanId("plan-gp4-conflict".to_string());
    let independent_plan_id = DelegatedTaskPlanId("plan-gp4-conflict-independent".to_string());
    let mut session = workflow_session(
        "conflict-pause",
        vec![
            worker(
                "worker:gp4:conflicted",
                Some(plan_id.clone()),
                "target:conflicted",
                81,
            ),
            worker(
                "worker:gp4:independent",
                Some(independent_plan_id.clone()),
                "target:independent",
                82,
            ),
        ],
        vec![verification_gate(
            LegionWorkflowVerificationGateState::Passed,
        )],
        vec![signoff(LegionWorkflowSignOffState::SignedOff)],
        Vec::new(),
        Some(approval(true)),
    );
    let conflict_id = LegionWorkflowConflictId("conflict:gp4:pause-target".to_string());
    session.conflict_summaries.push(LegionWorkflowConflict {
        conflict_id: conflict_id.clone(),
        kind: LegionWorkflowConflictKind::SameTarget,
        state: LegionWorkflowConflictState::Unresolved,
        worker_ids: Vec::new(),
        target_label: "conflict target label".to_string(),
        target_hash: Some(fingerprint("target:conflicted")),
        labels: vec!["legion_workflow.same_target_conflict".to_string()],
        redaction_hints: vec![RedactionHint::MetadataOnly],
        schema_version: 1,
    });
    let session_id = session.session_id.clone();
    ctx.app.seed_delegated_task_plan_contracts(vec![
        delegated_contract(plan_id),
        delegated_contract(independent_plan_id),
    ]);
    ctx.app
        .seed_legion_workflow_sessions(vec![session])
        .map_err(|e| format!("s9: seed workflow failed: {e:?}"))?;
    let resolver = NamedWorkerProviderResolver::new([
        (
            "worker:gp4:conflicted".to_string(),
            scripted_main_edit_provider("gp4-scripted-conflict", "gp4 conflict payload\n"),
        ),
        (
            "worker:gp4:independent".to_string(),
            scripted_main_edit_provider("gp4-scripted-independent", "gp4 independent payload\n"),
        ),
    ]);
    let first = ctx
        .app
        .execute_legion_workflow_with_providers(&session_id, &resolver)
        .map_err(|e| format!("s9: execute paused workflow failed: {e:?}"))?;
    let paused = first.outputs.iter().any(|output| {
        matches!(output, LegionWorkflowCoordinatorOutput::Blocked { reasons, .. }
            if reasons.iter().any(|reason| reason.contains("conflict_pause")))
    });
    if !paused {
        return Err("s9: expected unresolved conflict pause".to_string());
    }
    ctx.app
        .resolve_legion_workflow_conflict(&session_id, &conflict_id)
        .map_err(|e| format!("s9: resolve conflict failed: {e:?}"))?;
    let second = ctx
        .app
        .execute_legion_workflow_with_providers(&session_id, &resolver)
        .map_err(|e| format!("s9: execute resumed workflow failed: {e:?}"))?;
    let proposals = second
        .outputs
        .iter()
        .filter(|output| matches!(output, LegionWorkflowCoordinatorOutput::ProposalReady(_)))
        .count();
    if proposals != 2 {
        return Err(format!(
            "s9: expected 2 proposals after resume, got {proposals}"
        ));
    }
    Ok(
        "unresolved conflict paused dispatch and explicit resolution resumed both workers"
            .to_string(),
    )
}

fn run_s10(ctx: &mut Gp4Context) -> Result<String, String> {
    let plan_id = DelegatedTaskPlanId("plan-gp4-cancelled".to_string());
    let sibling_plan_id = DelegatedTaskPlanId("plan-gp4-cancelled-sibling".to_string());
    let session = workflow_session(
        "kill-switch-mid-run",
        vec![
            worker(
                "worker:gp4:cancelled",
                Some(plan_id.clone()),
                "target:cancelled",
                91,
            ),
            worker(
                "worker:gp4:sibling",
                Some(sibling_plan_id.clone()),
                "target:sibling",
                92,
            ),
        ],
        vec![verification_gate(
            LegionWorkflowVerificationGateState::Passed,
        )],
        vec![signoff(LegionWorkflowSignOffState::SignedOff)],
        Vec::new(),
        Some(approval(true)),
    );
    let session_id = session.session_id.clone();
    ctx.app.seed_delegated_task_plan_contracts(vec![
        delegated_contract(plan_id),
        delegated_contract(sibling_plan_id),
    ]);
    ctx.app
        .seed_legion_workflow_sessions(vec![session])
        .map_err(|e| format!("s10: seed workflow failed: {e:?}"))?;
    let cancellation_flag = SharedCancellationFlag::default();
    let cancelled_at = Arc::new(Mutex::new(None));
    let dispatch_log = Arc::new(Mutex::new(Vec::new()));
    ctx.app
        .inject_cancellation_flag_for_test(cancellation_flag.clone());
    let resolver = NamedWorkerProviderResolver::new([
        (
            "worker:gp4:cancelled".to_string(),
            Box::new(CancelOnSecondTurnProvider::new(
                cancellation_flag,
                cancelled_at.clone(),
            )) as Box<dyn ToolCallingProvider + Send>,
        ),
        (
            "worker:gp4:sibling".to_string(),
            Box::new(LoggingEditProvider::new(
                "worker:gp4:sibling",
                "gp4 sibling should be cancelled\n",
                dispatch_log,
            )) as Box<dyn ToolCallingProvider + Send>,
        ),
    ]);
    let started = Instant::now();
    let outcome = ctx
        .app
        .execute_legion_workflow_with_providers(&session_id, &resolver)
        .map_err(|e| format!("s10: execute cancelled workflow failed: {e:?}"))?;
    let finished = Instant::now();
    let blocked = outcome.outputs.iter().any(|output| {
        matches!(output, LegionWorkflowCoordinatorOutput::Blocked { reasons, .. }
            if reasons.iter().any(|reason| reason == "legion_workflow.worker_cancelled"))
    });
    if !blocked {
        return Err("s10: expected worker_cancelled blocked output".to_string());
    }
    let cancelled_at = cancelled_at
        .lock()
        .expect("cancelled_at lock")
        .ok_or_else(|| "s10: provider did not record cancellation instant".to_string())?;
    if finished.duration_since(cancelled_at) >= Duration::from_secs(2) {
        return Err(format!(
            "s10: kill switch cancellation ack exceeded 2s: {:?}",
            finished.duration_since(cancelled_at)
        ));
    }
    if started > cancelled_at || cancelled_at > finished {
        return Err("s10: cancellation instant fell outside execution window".to_string());
    }
    if !outcome
        .projection
        .decision_feed
        .iter()
        .any(|entry| entry.kind == LegionWorkflowDecisionKind::KillSwitchTriggered)
    {
        return Err("s10: missing KillSwitchTriggered decision feed row".to_string());
    }
    Ok(format!(
        "kill switch cancelled in-flight workers in {:?}",
        finished.duration_since(cancelled_at)
    ))
}

fn run_s11(ctx: &mut Gp4Context) -> Result<String, String> {
    let session_id = ctx
        .main_session_id
        .clone()
        .ok_or_else(|| "s11: missing main session id".to_string())?;
    if ctx.app.legion_workflow_session(&session_id).is_none() {
        let snapshot = ctx
            .main_session_snapshot
            .clone()
            .ok_or_else(|| "s11: missing main session snapshot".to_string())?;
        ctx.app
            .seed_legion_workflow_sessions(vec![snapshot])
            .map_err(|e| format!("s11: restore main session failed: {e:?}"))?;
    }
    ctx.app
        .record_legion_workflow_verification(
            &session_id,
            &LegionWorkflowVerificationGateId("verification:unit".to_string()),
            LegionWorkflowVerificationGateState::Passed,
            Some("evidence:gp4:main".to_string()),
        )
        .map_err(|e| format!("s11: record main verification failed: {e:?}"))?;
    ctx.app
        .record_legion_workflow_sign_off(
            &session_id,
            &LegionWorkflowSignOffId("signoff:reviewer".to_string()),
            LegionWorkflowSignOffState::SignedOff,
            Some(PrincipalId("reviewer:gp4".to_string())),
        )
        .map_err(|e| format!("s11: record main signoff failed: {e:?}"))?;
    ctx.app
        .record_legion_workflow_merge_approval(&session_id, true, true, true, false)
        .map_err(|e| format!("s11: record main approval failed: {e:?}"))?;

    let snapshot = ctx
        .app
        .shell_projection_snapshot("GP-4 Workflow Command Center")
        .map_err(|e| format!("s11: shell projection failed: {e:?}"))?;
    if snapshot.legion_workflow_board_columns.len() != 5 {
        return Err(format!(
            "s11: expected five board columns, got {}",
            snapshot.legion_workflow_board_columns.len()
        ));
    }
    if snapshot.legion_workflow_fleet_card_projections.len() < 3 {
        return Err(format!(
            "s11: expected at least 3 fleet cards, got {}",
            snapshot.legion_workflow_fleet_card_projections.len()
        ));
    }
    if !snapshot
        .legion_workflow_fleet_card_projections
        .iter()
        .any(|card| {
            card.test_status_label.contains("linked") || card.test_status_label.contains("unlinked")
        })
    {
        return Err("s11: fleet cards did not include projected verification labels".to_string());
    }
    let tags = snapshot
        .legion_workflow_comm_rows
        .iter()
        .filter_map(|row| legion_agent::comm::parse_agent_comm_line(row))
        .map(|parsed| parsed.tag)
        .collect::<Vec<_>>();
    for tag in AgentCommTag::ALL {
        if !tags.contains(&tag) {
            return Err(format!("s11: missing comm tag {}", tag.label()));
        }
    }
    let report = ctx
        .app
        .legion_workflow_merge_readiness_report(&session_id)
        .map_err(|e| format!("s11: merge readiness report failed: {e:?}"))?;
    if report.readiness.state != LegionWorkflowMergeReadinessState::Ready {
        return Err(format!(
            "s11: main session should be merge-ready, got {:?}",
            report.readiness
        ));
    }
    let cites_evidence = ctx
        .app
        .legion_workflow_session(&session_id)
        .ok_or_else(|| "s11: missing main session".to_string())?
        .verification_gates
        .iter()
        .any(|gate| gate.evidence_artifact_id.as_deref() == Some("evidence:gp4:main"));
    if !cites_evidence {
        return Err(
            "s11: merge-ready session does not cite GP-4 verification evidence".to_string(),
        );
    }
    Ok(format!(
        "board_columns=5 fleet_cards={} comm_tags=7 merge_ready=true",
        snapshot.legion_workflow_fleet_card_projections.len()
    ))
}

fn run_s12(ctx: &mut Gp4Context) -> Result<String, String> {
    let session_id = ctx
        .main_session_id
        .clone()
        .ok_or_else(|| "s12: missing main session id".to_string())?;
    let bundle = ctx
        .app
        .export_legion_workflow_evidence_bundle(&session_id)
        .map_err(|e| format!("s12: export evidence bundle failed: {e:?}"))?;
    let mut live_projection = ctx
        .app
        .legion_workflow_projection(bundle.projection_generated_at);
    live_projection
        .rows
        .retain(|row| row.session_id == session_id);
    live_projection
        .decision_feed
        .retain(|entry| entry.session_id == session_id);
    live_projection
        .risk_monitors
        .retain(|monitor| monitor.session_id == session_id);
    live_projection
        .kill_switches
        .retain(|kill_switch| kill_switch.session_id == session_id);
    live_projection.tool_permission_requests.retain(|request| {
        request
            .labels
            .iter()
            .any(|label| label == &format!("legion.session:{}", session_id.0))
    });
    live_projection.total_session_count = live_projection.rows.len() as u32;
    live_projection.decision_feed_count = live_projection.decision_feed.len() as u32;
    live_projection.risk_monitor_count = live_projection.risk_monitors.len() as u32;
    live_projection.kill_switch_count = live_projection.kill_switches.len() as u32;
    live_projection.tool_permission_request_count =
        live_projection.tool_permission_requests.len() as u32;
    if bundle.replay_projection() != live_projection {
        return Err("s12: replay projection did not match live projection".to_string());
    }
    if bundle.task_packets.is_empty()
        || bundle.worker_results.is_empty()
        || bundle.evidence_records.is_empty()
        || bundle.decision_feed_rows.is_empty()
    {
        return Err(format!(
            "s12: incomplete bundle packets={} results={} evidence={} decisions={}",
            bundle.task_packets.len(),
            bundle.worker_results.len(),
            bundle.evidence_records.len(),
            bundle.decision_feed_rows.len()
        ));
    }
    Ok(format!(
        "bundle replay matched live projection; packets={} results={} evidence={}",
        bundle.task_packets.len(),
        bundle.worker_results.len(),
        bundle.evidence_records.len()
    ))
}

fn run_s13(ctx: &mut Gp4Context) -> Result<String, String> {
    if ctx.temp_dir.exists() {
        fs::remove_dir_all(&ctx.temp_dir)
            .map_err(|e| format!("s13: remove temp workspace {}: {e}", ctx.temp_dir.display()))?;
    }
    Ok("report complete; temporary workspace removed".to_string())
}

fn single_worker_session(
    label: &str,
    approval_granted: bool,
) -> (LegionWorkflowSession, DelegatedTaskPlanId) {
    let plan_id = DelegatedTaskPlanId(format!("plan-gp4-{label}"));
    let session = workflow_session(
        label,
        vec![worker(
            &format!("worker:gp4:{label}"),
            Some(plan_id.clone()),
            "target:local",
            31,
        )],
        vec![verification_gate(
            LegionWorkflowVerificationGateState::Passed,
        )],
        vec![signoff(LegionWorkflowSignOffState::SignedOff)],
        Vec::new(),
        Some(approval(approval_granted)),
    );
    (session, plan_id)
}

fn record_step<T>(
    steps: &mut Vec<StepRecord>,
    id: &'static str,
    run: impl FnOnce() -> Result<T, String>,
    detail: impl FnOnce(&T) -> String,
) -> Result<T, String> {
    let started = utc_now();
    let timer = Instant::now();
    let result = run();
    let duration_ms = timer.elapsed().as_millis();
    let finished = utc_now();
    match result {
        Ok(value) => {
            let detail = detail(&value);
            eprintln!("[{id}] passed: {detail}");
            steps.push(StepRecord {
                id,
                started_utc: started,
                finished_utc: finished,
                duration_ms,
                status: StepStatus::Passed,
                detail,
            });
            Ok(value)
        }
        Err(error) => {
            eprintln!("[{id}] FAILED: {error}");
            steps.push(StepRecord {
                id,
                started_utc: started,
                finished_utc: finished,
                duration_ms,
                status: StepStatus::Failed,
                detail: error.clone(),
            });
            Err(error)
        }
    }
}

fn write_evidence(
    out_dir: &Path,
    evidence_dir: Option<&Path>,
    legion_sha: &str,
    started_utc: &str,
    finished_utc: &str,
    steps: &[StepRecord],
) -> Result<PathBuf, String> {
    fs::create_dir_all(out_dir)
        .map_err(|e| format!("create out_dir {}: {e}", out_dir.display()))?;
    let overall_status = if steps.iter().any(|step| step.status == StepStatus::Failed) {
        "failed"
    } else {
        "passed"
    };

    let mut toml = String::new();
    toml.push_str("schema_version = 1\n");
    toml.push_str(&format!("git_sha = \"{legion_sha}\"\n"));
    toml.push_str(&format!("started_utc = \"{started_utc}\"\n"));
    toml.push_str(&format!("finished_utc = \"{finished_utc}\"\n"));
    toml.push_str(&format!("overall_status = \"{overall_status}\"\n\n"));
    for step in steps {
        toml.push_str("[[steps]]\n");
        toml.push_str(&format!("id = \"{}\"\n", step.id));
        toml.push_str(&format!("status = \"{}\"\n", step.status.as_str()));
        toml.push_str(&format!("started_utc = \"{}\"\n", step.started_utc));
        toml.push_str(&format!("finished_utc = \"{}\"\n", step.finished_utc));
        toml.push_str(&format!("duration_ms = {}\n", step.duration_ms));
        let detail = if step.detail.chars().count() > 512 {
            format!("{}...", step.detail.chars().take(512).collect::<String>())
        } else {
            step.detail.clone()
        };
        toml.push_str(&format!("detail = {:?}\n\n", detail));
    }

    let out_path = out_dir.join("gp4_report.toml");
    fs::write(&out_path, &toml).map_err(|e| format!("write {}: {e}", out_path.display()))?;
    eprintln!("[gp4] wrote evidence: {}", out_path.display());

    if let Some(ev_dir) = evidence_dir {
        fs::create_dir_all(ev_dir)
            .map_err(|e| format!("create evidence_dir {}: {e}", ev_dir.display()))?;
        let ev_path = ev_dir.join("gp4_report.toml");
        fs::write(&ev_path, &toml)
            .map_err(|e| format!("write evidence copy {}: {e}", ev_path.display()))?;
        eprintln!("[gp4] wrote evidence copy: {}", ev_path.display());
    }

    Ok(out_path)
}

fn finalize(
    args: &Args,
    legion_sha: &str,
    started_utc: &str,
    steps: &[StepRecord],
    exit_code: i32,
) -> ! {
    let finished_utc = utc_now();
    if let Err(error) = write_evidence(
        &args.out_dir,
        args.evidence_dir.as_deref(),
        legion_sha,
        started_utc,
        &finished_utc,
        steps,
    ) {
        eprintln!("golden-path-4: failed to write evidence: {error}");
        process::exit(2);
    }
    process::exit(exit_code);
}

fn main() {
    let args = match parse_args() {
        Ok(args) => args,
        Err(error) => {
            eprintln!("golden-path-4: argument error: {error}");
            eprintln!(
                "Usage: golden_path_4 --fixture-dir <path> [--out-dir <path>] [--record-evidence <path>]"
            );
            process::exit(2);
        }
    };

    let started_utc = utc_now();
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let legion_sha = resolve_legion_git_sha(&cwd);
    let mut steps = Vec::new();
    eprintln!("[gp4] Legion git SHA: {legion_sha}");
    eprintln!("[gp4] fixture dir: {}", args.fixture_dir.display());

    let mut ctx = match record_step(
        &mut steps,
        "s1",
        || run_s1(&args.fixture_dir),
        |ctx| {
            format!(
                "workspace opened in Automate mode at {}",
                ctx.temp_dir.display()
            )
        },
    ) {
        Ok(ctx) => ctx,
        Err(_) => finalize(&args, &legion_sha, &started_utc, &steps, 1),
    };
    if record_step(
        &mut steps,
        "s2",
        || run_s2(&mut ctx),
        |detail| detail.clone(),
    )
    .is_err()
    {
        finalize(&args, &legion_sha, &started_utc, &steps, 1);
    }
    if record_step(
        &mut steps,
        "s3",
        || run_s3(&mut ctx),
        |detail| detail.clone(),
    )
    .is_err()
    {
        finalize(&args, &legion_sha, &started_utc, &steps, 1);
    }
    if record_step(
        &mut steps,
        "s4",
        || run_s4(&mut ctx),
        |detail| detail.clone(),
    )
    .is_err()
    {
        finalize(&args, &legion_sha, &started_utc, &steps, 1);
    }
    if record_step(
        &mut steps,
        "s5",
        || run_s5(&mut ctx),
        |detail| detail.clone(),
    )
    .is_err()
    {
        finalize(&args, &legion_sha, &started_utc, &steps, 1);
    }
    if record_step(
        &mut steps,
        "s6",
        || run_s6(&mut ctx),
        |detail| detail.clone(),
    )
    .is_err()
    {
        finalize(&args, &legion_sha, &started_utc, &steps, 1);
    }
    if record_step(
        &mut steps,
        "s7",
        || run_s7(&mut ctx),
        |detail| detail.clone(),
    )
    .is_err()
    {
        finalize(&args, &legion_sha, &started_utc, &steps, 1);
    }
    if record_step(
        &mut steps,
        "s8",
        || run_s8(&mut ctx),
        |detail| detail.clone(),
    )
    .is_err()
    {
        finalize(&args, &legion_sha, &started_utc, &steps, 1);
    }
    if record_step(
        &mut steps,
        "s9",
        || run_s9(&mut ctx),
        |detail| detail.clone(),
    )
    .is_err()
    {
        finalize(&args, &legion_sha, &started_utc, &steps, 1);
    }
    if record_step(
        &mut steps,
        "s10",
        || run_s10(&mut ctx),
        |detail| detail.clone(),
    )
    .is_err()
    {
        finalize(&args, &legion_sha, &started_utc, &steps, 1);
    }
    if record_step(
        &mut steps,
        "s11",
        || run_s11(&mut ctx),
        |detail| detail.clone(),
    )
    .is_err()
    {
        finalize(&args, &legion_sha, &started_utc, &steps, 1);
    }
    if record_step(
        &mut steps,
        "s12",
        || run_s12(&mut ctx),
        |detail| detail.clone(),
    )
    .is_err()
    {
        finalize(&args, &legion_sha, &started_utc, &steps, 1);
    }
    if record_step(
        &mut steps,
        "s13",
        || run_s13(&mut ctx),
        |detail| detail.clone(),
    )
    .is_err()
    {
        finalize(&args, &legion_sha, &started_utc, &steps, 1);
    }

    finalize(&args, &legion_sha, &started_utc, &steps, 0);
}
