//! GP-2 Golden Path smoke runner (M9 milestone closer).
//!
//! Invoked by `cargo run -p xtask -- golden-path-2` (subprocess model — xtask
//! cannot depend on legion-app, so it spawns this binary and reads its exit
//! code + the evidence TOML).
//!
//! Compiled with **default features** (which include `ai`).  Unlike GP-1 which
//! uses `--no-default-features`, GP-2 exercises the AI-enabled product API.
//!
//! # Steps
//! s1 copy-fixture:       copy fixture to temp dir; git-init; open as Trusted workspace.
//! s2 provider-setup:     set_product_mode(Assist); open src/main.rs; get buffer_id.
//! s3 inline-prediction:  request inline prediction at cursor; assert ghost text; accept;
//!                        assert buffer changed; assert undo available.
//! s4 provider-route:     ProviderRegistry + DenyByDefaultBroker; route local-loopback
//!                        → assert Completed; route unauthorized remote → assert Refused.
//! s5 context-manifest:   assemble_context_manifest_from_sources; assert file entries
//!                        and valid manifest_id; assert permissions non-empty.
//! s6 checkpoint-apply:   undo s3; build CreateFile proposal; apply via lifecycle pipeline;
//!                        verify checkpoint created; restore; verify file removed + buffer at original.
//! s7 evidence:           write `target/golden-path/gp2_report.toml`.
//!
//! # Constraints
//! - Never writes inside the Legion repo (except target/ and --record-evidence path).
//! - Fixture copies live in OS temp; cleaned on success, left on failure.
//! - Zero egress: all operations are local; deterministic-local provider only in CI.

use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
    process,
    time::{Instant, SystemTime, UNIX_EPOCH},
};

use legion_ai::{
    ChatCompletionRequest, ChatCompletionResponse, DeterministicInlinePredictionProvider,
    EmbeddingRequest, EmbeddingResponse, ManifestMetadata, ModelProvider, ProviderCapabilities,
    ProviderError, ProviderId, ProviderRegistry, ProviderRouter,
    assemble_context_manifest_from_sources, collect_file_context,
};
use legion_app::{AppCommandOutcome, AppComposition, AppProductMode};
use legion_protocol::{
    AssistedAiOperationClass, AssistedAiProposalTargetIntent, AssistedAiProviderClass,
    AssistedAiProviderRouteRequest, AssistedAiTrustProjectionKind,
    AssistedAiTrustProjectionReference, BufferId, CancellationTokenId, CanonicalPath,
    CapabilityDecisionId, CapabilityId, CausalityId, ContextManifestEgressStatus,
    ContextManifestPermissionKind, ContextManifestPermissionSummary, ContextManifestPurpose,
    ContextManifestSources, CorrelationId, CreateFileProposal, EventSequence, FileFingerprint,
    FileId, NetworkTarget, PreviewSummary, PrincipalId, ProposalId, ProposalPayload,
    ProposalPayloadKind, ProposalPrivacyLabel, ProposalRequest, ProposalResponse,
    ProposalRiskLabel, ProposalTargetCoverage, ProposalTargetCoverageKind,
    ProposalVersionPreconditions, RedactionHint, SemanticPrivacyScope, TextCoordinate,
    TimestampMillis, WorkspaceGeneration, WorkspaceId, WorkspaceProposal, WorkspaceTrustState,
};
use legion_security::DenyByDefaultBroker;
use legion_ui::CommandDispatchIntent;
use uuid::Uuid;

// ─────────────────────────────────────────────────────────────────────────────
// Step status
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq)]
enum StepStatus {
    Passed,
    Failed,
    Skipped,
}

impl StepStatus {
    fn as_str(&self) -> &'static str {
        match self {
            StepStatus::Passed => "passed",
            StepStatus::Failed => "failed",
            StepStatus::Skipped => "skipped",
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

// ─────────────────────────────────────────────────────────────────────────────
// CLI args
// ─────────────────────────────────────────────────────────────────────────────

struct Args {
    fixture_dir: PathBuf,
    out_dir: PathBuf,
    evidence_dir: Option<PathBuf>,
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

// ─────────────────────────────────────────────────────────────────────────────
// Helpers (copied verbatim from golden_path_1.rs)
// ─────────────────────────────────────────────────────────────────────────────

/// Convert Unix epoch seconds to an RFC 3339 UTC timestamp string.
fn epoch_secs_to_rfc3339(secs: u64) -> String {
    let days = secs / 86400;
    let rem = secs % 86400;
    let h = rem / 3600;
    let m = (rem % 3600) / 60;
    let s = rem % 60;
    let (year, month, day) = days_to_ymd(days as i64);
    format!("{year:04}-{month:02}-{day:02}T{h:02}:{m:02}:{s:02}Z")
}

/// Convert days since Unix epoch to (year, month, day).
///
/// Algorithm: civil_from_days — Howard Hinnant, https://howardhinnant.github.io/date_algorithms.html
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

fn run_timer<F, T>(f: F) -> (T, u128)
where
    F: FnOnce() -> T,
{
    let start = Instant::now();
    let result = f();
    (result, start.elapsed().as_millis())
}

/// Copy a directory tree recursively.
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

// ─────────────────────────────────────────────────────────────────────────────
// Provider for s4 route test (defines a completion-capable local provider)
// ─────────────────────────────────────────────────────────────────────────────

struct Gp2LocalCompletionProvider;

impl ModelProvider for Gp2LocalCompletionProvider {
    fn provider_id(&self) -> ProviderId {
        "gp2-local".to_string()
    }

    fn capabilities(&self) -> ProviderCapabilities {
        ProviderCapabilities {
            completion: true,
            embedding: false,
            batch: false,
            inline_prediction: false,
        }
    }

    fn complete(
        &self,
        request: ChatCompletionRequest,
    ) -> Result<ChatCompletionResponse, ProviderError> {
        Ok(ChatCompletionResponse {
            provider: request.provider,
            model: request.model,
            text: "metadata-only".to_string(),
            metadata: HashMap::new(),
        })
    }

    fn embed(&self, request: EmbeddingRequest) -> Result<EmbeddingResponse, ProviderError> {
        Err(ProviderError::unsupported(request.provider, "embed"))
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Helper: build an AssistedAiTrustProjectionReference
// ─────────────────────────────────────────────────────────────────────────────

fn trust_reference(
    reference_id: &str,
    kind: AssistedAiTrustProjectionKind,
) -> AssistedAiTrustProjectionReference {
    AssistedAiTrustProjectionReference {
        reference_id: reference_id.to_string(),
        kind,
        projection_hash: FileFingerprint {
            algorithm: "sha256".to_string(),
            value: reference_id.to_string(),
        },
        schema_version: 1,
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Step s1: copy fixture + open workspace
// ─────────────────────────────────────────────────────────────────────────────

struct S1Result {
    temp_dir: PathBuf,
    app: AppComposition,
    workspace_id: WorkspaceId,
    generation: WorkspaceGeneration,
}

fn run_s1(fixture_dir: &Path) -> Result<S1Result, String> {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let temp_dir =
        std::env::temp_dir().join(format!("legion-gp2-smoke-{}-{}", process::id(), nanos));

    copy_dir_recursive(fixture_dir, &temp_dir)?;

    git_cmd(&temp_dir, &["init", "-b", "main"])?;
    git_cmd(
        &temp_dir,
        &["config", "user.email", "gp2-smoke@legion.test"],
    )?;
    git_cmd(&temp_dir, &["config", "user.name", "GP-2 Smoke"])?;
    git_cmd(&temp_dir, &["add", "."])?;
    git_cmd(
        &temp_dir,
        &["commit", "-m", "initial: gp2 smoke fixture baseline"],
    )?;

    let mut app = AppComposition::new();
    let opened = app
        .open_workspace(
            &temp_dir,
            WorkspaceTrustState::Trusted,
            PrincipalId("gp2-smoke".to_string()),
        )
        .map_err(|e| format!("open_workspace failed: {e:?}"))?;

    Ok(S1Result {
        temp_dir,
        app,
        workspace_id: opened.workspace_id,
        generation: opened.generation,
    })
}

// ─────────────────────────────────────────────────────────────────────────────
// Step s2: provider setup — set Assist mode, open src/main.rs
// ─────────────────────────────────────────────────────────────────────────────

struct S2Result {
    buffer_id: BufferId,
    file_id: FileId,
}

fn run_s2(temp_dir: &Path, app: &mut AppComposition) -> Result<S2Result, String> {
    // Enable AI dispatch (Assist mode).
    app.set_product_mode(AppProductMode::Assist);
    eprintln!("[s2] product_mode set to Assist");

    // Open src/main.rs and get the active buffer id.
    let main_rs = temp_dir.join("src").join("main.rs");
    let main_rs_str = main_rs.to_string_lossy().into_owned();
    let file_id = app
        .open_file(&main_rs_str)
        .map_err(|e| format!("open_file(src/main.rs) failed: {e:?}"))?;

    let buffer_id = app
        .active_buffer_id()
        .ok_or("s2: no active buffer after open_file(src/main.rs)")?;

    eprintln!("[s2] src/main.rs opened; buffer_id={buffer_id:?} file_id={file_id:?}");
    Ok(S2Result { buffer_id, file_id })
}

// ─────────────────────────────────────────────────────────────────────────────
// Step s3: inline prediction — request, assert ghost text, accept, assert change
// ─────────────────────────────────────────────────────────────────────────────

struct S3Result {
    original_text: String,
    // Captured for evidence; s6 now exercises the checkpoint pipeline rather than redo.
    #[allow(dead_code)]
    accepted_text: String,
}

fn run_s3(app: &mut AppComposition, buffer_id: BufferId) -> Result<S3Result, String> {
    // Record original buffer text before any prediction.
    // Use .to_owned() immediately so the &str borrow on app ends before we
    // call dispatch_ui_intent (which requires &mut app).
    let original_text: String = app
        .editor()
        .text(buffer_id)
        .map_err(|e| format!("s3: read original text: {e:?}"))?
        .to_owned();
    eprintln!(
        "[s3] original text length: {} chars",
        original_text.chars().count()
    );

    // Request inline prediction at the end of line 9 (fn main() {).
    // Line 9 in fixtures/gp1-rust/src/main.rs is "fn main() {" (11 chars).
    let position = TextCoordinate {
        line: 9,
        character: 11,
        byte_offset: Some(11),
        utf16_offset: Some(11),
    };

    let outcome = app
        .dispatch_ui_intent(CommandDispatchIntent::RequestAssistInlinePrediction {
            buffer_id,
            position,
        })
        .map_err(|e| format!("s3: request inline prediction failed: {e:?}"))?;

    let projection = match outcome {
        AppCommandOutcome::AssistInlinePredictionUpdated(p) => p,
        other => {
            return Err(format!(
                "s3: expected AssistInlinePredictionUpdated, got {other:?}"
            ));
        }
    };

    let active = projection
        .active_prediction
        .as_ref()
        .ok_or("s3: no active_prediction in projection after request")?;

    eprintln!(
        "[s3] ghost text: {:?}  status={:?}",
        active.ghost_text_label, active.status
    );

    if active.ghost_text_label.is_empty() {
        return Err("s3: ghost_text_label is empty — expected non-empty ghost text from deterministic provider".to_string());
    }

    let prediction_id = active.prediction_id.clone();

    // Accept the prediction.
    let accept_outcome = app
        .dispatch_ui_intent(CommandDispatchIntent::AcceptAssistInlinePrediction {
            buffer_id,
            prediction_id: Some(prediction_id.clone()),
        })
        .map_err(|e| format!("s3: accept inline prediction failed: {e:?}"))?;

    let accept_projection = match accept_outcome {
        AppCommandOutcome::AssistInlinePredictionUpdated(p) => p,
        other => {
            return Err(format!(
                "s3: expected AssistInlinePredictionUpdated after accept, got {other:?}"
            ));
        }
    };

    // Assert active prediction is cleared after accept.
    if accept_projection.active_prediction.is_some() {
        return Err("s3: active_prediction should be None after accept but it is Some".to_string());
    }

    // Assert buffer text changed.
    let accepted_text: String = app
        .editor()
        .text(buffer_id)
        .map_err(|e| format!("s3: read accepted text: {e:?}"))?
        .to_owned();

    if accepted_text == original_text {
        return Err("s3: buffer text did not change after accepting inline prediction".to_string());
    }
    eprintln!(
        "[s3] buffer text changed after accept (original_len={} accepted_len={})",
        original_text.len(),
        accepted_text.len()
    );

    // Assert undo is available (undo_len >= 1).
    let undo_len = app
        .editor()
        .undo_len(buffer_id)
        .map_err(|e| format!("s3: undo_len: {e:?}"))?;
    if undo_len == 0 {
        return Err("s3: undo_len == 0 after accepting prediction; expected >= 1".to_string());
    }
    eprintln!("[s3] undo_len={undo_len} (prediction is undoable)");

    Ok(S3Result {
        original_text,
        accepted_text,
    })
}

// ─────────────────────────────────────────────────────────────────────────────
// Step s4: provider route — local loopback passes, unauthorized remote refused
// ─────────────────────────────────────────────────────────────────────────────

fn run_s4() -> Result<(), String> {
    let mut registry = ProviderRegistry::new();
    registry.register(Box::new(Gp2LocalCompletionProvider));
    registry.register(Box::new(DeterministicInlinePredictionProvider::new(
        "gp2-deterministic-inline",
    )));
    let broker = DenyByDefaultBroker::default();
    let router = ProviderRouter::new(&registry, &broker);

    // ── Local-loopback route (expect Completed) ─────────────────────────────
    let local_request = AssistedAiProviderRouteRequest {
        route_id: "gp2-route-local-1".to_string(),
        provider_id: "gp2-local".to_string(),
        model_label: "gp2-deterministic".to_string(),
        provider_class: AssistedAiProviderClass::Local,
        operation_class: AssistedAiOperationClass::ProposeEdit,
        context_manifest: trust_reference("ctx-1", AssistedAiTrustProjectionKind::ContextManifest),
        privacy_inspector: trust_reference(
            "priv-1",
            AssistedAiTrustProjectionKind::PrivacyInspector,
        ),
        permission_budget: trust_reference(
            "budget-1",
            AssistedAiTrustProjectionKind::PermissionBudget,
        ),
        prompt_prefix: String::new(),
        proposal_intent: AssistedAiProposalTargetIntent {
            payload_kind: ProposalPayloadKind::TextEdit,
            target_coverage: ProposalTargetCoverage {
                coverage_kind: ProposalTargetCoverageKind::Complete,
                targets: Vec::new(),
                omitted_target_count: 0,
                redaction_hints: vec![RedactionHint::MetadataOnly],
            },
            required_capability: CapabilityId("ai.proposal.create".to_string()),
            risk_label: ProposalRiskLabel::Low,
            privacy_label: ProposalPrivacyLabel::WorkspaceMetadata,
            labels: vec!["proposal.intent".to_string()],
            redaction_hints: vec![RedactionHint::MetadataOnly],
            schema_version: 1,
        },
        policy_decision_id: None,
        required_capability: CapabilityId("ai.provider.invoke".to_string()),
        network_target: Some(NetworkTarget {
            scheme: "http".to_string(),
            host: "localhost".to_string(),
            port: Some(11434),
        }),
        cancellation_token: CancellationTokenId(
            Uuid::parse_str("11111111-1111-1111-1111-111111111111").unwrap(),
        ),
        health_labels: vec!["healthy".to_string()],
        cost_labels: vec!["local".to_string()],
        principal_id: PrincipalId("gp2-smoke".to_string()),
        workspace_trust_state: WorkspaceTrustState::Trusted,
        correlation_id: CorrelationId(1),
        causality_id: CausalityId(Uuid::parse_str("22222222-2222-2222-2222-222222222222").unwrap()),
        event_sequence: EventSequence(1),
        redaction_hints: vec![RedactionHint::MetadataOnly],
        schema_version: 1,
    };

    let local_response = router
        .route_completion(local_request)
        .map_err(|e| format!("s4: local route_completion error: {e}"))?;

    use legion_protocol::AssistedAiProviderInvocationState;
    if local_response.invocation_state != AssistedAiProviderInvocationState::Completed {
        return Err(format!(
            "s4: expected local route to be Completed, got {:?} refusal={:?}",
            local_response.invocation_state, local_response.refusal
        ));
    }
    eprintln!("[s4] local-loopback route: Completed (policy enforcement OK)");

    // ── Remote route (expect Refused — remote provider disabled by default policy) ─
    let remote_request = AssistedAiProviderRouteRequest {
        route_id: "gp2-route-remote-1".to_string(),
        provider_id: "gp2-remote-blocked".to_string(),
        model_label: "hosted-model".to_string(),
        provider_class: AssistedAiProviderClass::Local,
        operation_class: AssistedAiOperationClass::ProposeEdit,
        context_manifest: trust_reference("ctx-2", AssistedAiTrustProjectionKind::ContextManifest),
        privacy_inspector: trust_reference(
            "priv-2",
            AssistedAiTrustProjectionKind::PrivacyInspector,
        ),
        permission_budget: trust_reference(
            "budget-2",
            AssistedAiTrustProjectionKind::PermissionBudget,
        ),
        prompt_prefix: String::new(),
        proposal_intent: AssistedAiProposalTargetIntent {
            payload_kind: ProposalPayloadKind::TextEdit,
            target_coverage: ProposalTargetCoverage {
                coverage_kind: ProposalTargetCoverageKind::Complete,
                targets: Vec::new(),
                omitted_target_count: 0,
                redaction_hints: vec![RedactionHint::MetadataOnly],
            },
            required_capability: CapabilityId("ai.proposal.create".to_string()),
            risk_label: ProposalRiskLabel::Low,
            privacy_label: ProposalPrivacyLabel::WorkspaceMetadata,
            labels: vec!["proposal.intent".to_string()],
            redaction_hints: vec![RedactionHint::MetadataOnly],
            schema_version: 1,
        },
        policy_decision_id: None,
        required_capability: CapabilityId("ai.provider.invoke".to_string()),
        network_target: Some(NetworkTarget {
            scheme: "https".to_string(),
            host: "api.remote.invalid".to_string(),
            port: Some(443),
        }),
        cancellation_token: CancellationTokenId(
            Uuid::parse_str("33333333-3333-3333-3333-333333333333").unwrap(),
        ),
        health_labels: vec!["remote".to_string()],
        cost_labels: vec!["remote".to_string()],
        principal_id: PrincipalId("gp2-smoke".to_string()),
        workspace_trust_state: WorkspaceTrustState::Trusted,
        correlation_id: CorrelationId(2),
        causality_id: CausalityId(Uuid::parse_str("44444444-4444-4444-4444-444444444444").unwrap()),
        event_sequence: EventSequence(2),
        redaction_hints: vec![RedactionHint::MetadataOnly],
        schema_version: 1,
    };

    let remote_response = router
        .route_completion(remote_request)
        .map_err(|e| format!("s4: remote route_completion error: {e}"))?;

    if remote_response.invocation_state != AssistedAiProviderInvocationState::Refused {
        return Err(format!(
            "s4: expected remote route to be Refused (zero-egress policy), got {:?}",
            remote_response.invocation_state
        ));
    }
    if remote_response.refusal.is_none() {
        return Err(
            "s4: remote route Refused but refusal metadata is None — expected Some".to_string(),
        );
    }
    eprintln!("[s4] remote route: Refused (zero-egress policy enforced)");

    Ok(())
}

// ─────────────────────────────────────────────────────────────────────────────
// Step s5: context manifest — assemble from file sources, assert entries
// ─────────────────────────────────────────────────────────────────────────────

fn run_s5(temp_dir: &Path) -> Result<(), String> {
    let main_rs_path = temp_dir.join("src").join("main.rs");
    let canonical = CanonicalPath(
        main_rs_path
            .to_string_lossy()
            .replace('\\', "/")
            .to_string(),
    );

    let file_items = collect_file_context(&[canonical], WorkspaceId(1));
    eprintln!("[s5] collected {} file context item(s)", file_items.len());

    if file_items.is_empty() {
        return Err(
            "s5: collect_file_context returned 0 items — expected >= 1 for src/main.rs".to_string(),
        );
    }

    let sources = ContextManifestSources {
        files: file_items,
        selections: Vec::new(),
        symbols: Vec::new(),
        diagnostics: Vec::new(),
        terminal_excerpts: Vec::new(),
        memory: Vec::new(),
        rules: Vec::new(),
    };

    // Add a permission summary so we can assert permissions is non-empty.
    let permission = ContextManifestPermissionSummary {
        kind: ContextManifestPermissionKind::Filesystem,
        capability: CapabilityId("ai.context.assemble".to_string()),
        principal: Some(PrincipalId("gp2-smoke".to_string())),
        decision_id: Some(CapabilityDecisionId(1)),
        granted: true,
        privacy_scope: SemanticPrivacyScope::MetadataOnly,
        egress: ContextManifestEgressStatus::LocalOnly,
        risk_label: ProposalRiskLabel::Low,
        redaction_hints: vec![RedactionHint::MetadataOnly],
        schema_version: 1,
    };

    let metadata = ManifestMetadata {
        workspace_id: Some(WorkspaceId(1)),
        proposal_id: None,
        purpose: ContextManifestPurpose::ProviderRequest,
        workspace_trust_state: Some(WorkspaceTrustState::Trusted),
        privacy_label: ProposalPrivacyLabel::WorkspaceMetadata,
        risk_label: ProposalRiskLabel::Low,
        egress: ContextManifestEgressStatus::LocalOnly,
        permissions: vec![permission],
        generated_at: TimestampMillis(
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64,
        ),
        schema_version: 1,
    };

    let manifest = assemble_context_manifest_from_sources(sources, metadata);

    eprintln!(
        "[s5] manifest_id={:?}  items={}  permissions={}",
        manifest.manifest_id,
        manifest.items.len(),
        manifest.permissions.len()
    );

    // Assert manifest_id is non-empty and looks like a deterministic ID.
    if manifest.manifest_id.is_empty() {
        return Err("s5: manifest_id is empty — expected non-empty deterministic ID".to_string());
    }
    if !manifest.manifest_id.starts_with("manifest:") {
        return Err(format!(
            "s5: manifest_id does not start with 'manifest:'; got {:?}",
            manifest.manifest_id
        ));
    }

    // Assert at least one file entry.
    if manifest.items.is_empty() {
        return Err("s5: manifest.items is empty — expected >= 1 file entry".to_string());
    }

    // Assert permissions is non-empty (we added one above).
    if manifest.permissions.is_empty() {
        return Err("s5: manifest.permissions is empty — expected >= 1 entry".to_string());
    }

    eprintln!(
        "[s5] context manifest passed: manifest_id={} items={} permissions={}",
        manifest.manifest_id,
        manifest.items.len(),
        manifest.permissions.len()
    );
    Ok(())
}

// ─────────────────────────────────────────────────────────────────────────────
// Step s6: checkpoint-apply — undo s3, apply CreateFile proposal through the
// full lifecycle pipeline, verify checkpoint, restore, verify file removed.
// ─────────────────────────────────────────────────────────────────────────────

fn run_s6(
    app: &mut AppComposition,
    temp_dir: &Path,
    buffer_id: BufferId,
    workspace_id: WorkspaceId,
    generation: WorkspaceGeneration,
    s3: &S3Result,
) -> Result<(), String> {
    // ── 1. Undo the s3 accepted prediction so buffer matches disk ─────────────
    let undo_outcome = app
        .dispatch_ui_intent(CommandDispatchIntent::Undo { buffer_id })
        .map_err(|e| format!("s6: undo dispatch failed: {e:?}"))?;
    match undo_outcome {
        AppCommandOutcome::Edited(_) => {}
        other => {
            return Err(format!("s6: expected Edited from Undo, got {other:?}"));
        }
    }
    let post_undo_text: String = app
        .editor()
        .text(buffer_id)
        .map_err(|e| format!("s6: read post-undo text: {e:?}"))?
        .to_owned();
    if post_undo_text != s3.original_text {
        return Err(format!(
            "s6: post-undo text != original_text (lens: {} vs {})",
            post_undo_text.len(),
            s3.original_text.len()
        ));
    }
    eprintln!("[s6] undo verified: buffer restored to original text");

    // ── 2. Refresh workspace generation (idempotent re-open) ─────────────────
    let current_gen = app
        .open_workspace(
            temp_dir,
            WorkspaceTrustState::Trusted,
            PrincipalId("gp2-smoke".to_string()),
        )
        .map_err(|e| format!("s6: open_workspace (generation refresh) failed: {e:?}"))?
        .generation;
    let _ = workspace_id; // carried for context; generation refresh is sufficient
    let _ = generation;
    eprintln!("[s6] workspace generation refreshed: {current_gen:?}");

    // ── 3. Build a CreateFile proposal ───────────────────────────────────────
    // CreateFile is the canonical proposal kind that produces durable checkpoints:
    // TextEdit applies only to the editor buffer and produces no file-level
    // rollback material, so list_checkpoints() would return empty after apply.
    let smoke_file_path = temp_dir.join("gp2-s6-smoke.txt");
    let smoke_canonical = CanonicalPath(smoke_file_path.to_string_lossy().into_owned());

    let proposal = WorkspaceProposal {
        proposal_id: ProposalId(700),
        principal: PrincipalId("gp2-smoke".to_string()),
        capability: CapabilityId("fs.write".to_string()),
        correlation_id: CorrelationId(700),
        payload: ProposalPayload::CreateFile(CreateFileProposal {
            path: smoke_canonical.clone(),
            initial_content: Some("gp2 s6 checkpoint smoke\n".to_string()),
        }),
        preconditions: ProposalVersionPreconditions {
            file_version: None,
            buffer_version: None,
            snapshot_id: None,
            generation: None,
            file_content_version: None,
            workspace_generation: Some(current_gen),
            expected_fingerprint: None,
            expected_file_length: None,
            expected_modified_at: None,
        },
        preview: PreviewSummary {
            summary: "gp2 smoke s6 checkpoint".to_string(),
            details: Vec::new(),
        },
        expires_at: None,
        created_at: TimestampMillis(1),
    };

    // ── 4. Register → Validate → Preview → Apply ─────────────────────────────
    let register_resp = app
        .register_proposal_lifecycle(&proposal)
        .map_err(|e| format!("s6: register_proposal_lifecycle failed: {e:?}"))?;
    match register_resp {
        ProposalResponse::Created(_) => eprintln!("[s6] proposal registered (Created)"),
        other => {
            return Err(format!(
                "s6: expected Created from register_proposal_lifecycle, got {other:?}"
            ));
        }
    }

    let validate_resp = app
        .handle_proposal_request(ProposalRequest::Validate(proposal.clone()))
        .map_err(|e| format!("s6: Validate failed: {e:?}"))?;
    match validate_resp {
        ProposalResponse::Validated(_) => eprintln!("[s6] proposal Validated"),
        other => {
            return Err(format!(
                "s6: expected Validated from Validate, got {other:?}"
            ));
        }
    }

    let preview_resp = app
        .handle_proposal_request(ProposalRequest::Preview(proposal.clone()))
        .map_err(|e| format!("s6: Preview failed: {e:?}"))?;
    match preview_resp {
        ProposalResponse::Previewed { .. } => eprintln!("[s6] proposal Previewed"),
        other => {
            return Err(format!(
                "s6: expected Previewed from Preview, got {other:?}"
            ));
        }
    }

    let apply_resp = app
        .handle_proposal_request(ProposalRequest::Apply(proposal.clone()))
        .map_err(|e| format!("s6: Apply failed: {e:?}"))?;
    match apply_resp {
        ProposalResponse::Applied(_) => eprintln!("[s6] proposal Applied"),
        other => {
            return Err(format!("s6: expected Applied from Apply, got {other:?}"));
        }
    }

    // Assert the file was created on disk.
    if !smoke_file_path.exists() {
        return Err(format!(
            "s6: smoke file not created on disk: {}",
            smoke_file_path.display()
        ));
    }
    eprintln!("[s6] smoke file created: {}", smoke_file_path.display());

    // ── 5. Verify checkpoint was auto-created ─────────────────────────────────
    let checkpoints = app.list_checkpoints();
    if checkpoints.is_empty() {
        return Err(
            "s6: list_checkpoints() is empty after apply — expected >= 1 durable checkpoint"
                .to_string(),
        );
    }
    if checkpoints[0].proposal_id != ProposalId(700) {
        return Err(format!(
            "s6: checkpoint[0].proposal_id = {:?}; expected ProposalId(700)",
            checkpoints[0].proposal_id
        ));
    }
    let checkpoint_id = checkpoints[0].checkpoint_id.clone();
    eprintln!(
        "[s6] checkpoint verified: proposal_id={:?} checkpoint_id={checkpoint_id}",
        checkpoints[0].proposal_id
    );

    // ── 6. Restore the checkpoint ─────────────────────────────────────────────
    app.restore_checkpoint(&checkpoint_id)
        .map_err(|e| format!("s6: restore_checkpoint failed: {e:?}"))?;
    eprintln!("[s6] checkpoint restored");

    // ── 7. Verify file was removed (pre-apply state = did not exist) ──────────
    if smoke_file_path.exists() {
        return Err(format!(
            "s6: smoke file still exists after checkpoint restore: {}",
            smoke_file_path.display()
        ));
    }
    eprintln!("[s6] smoke file removed by restore (pre-apply state verified)");

    // ── 8. Verify main.rs buffer is still at original text ────────────────────
    let post_restore_text: String = app
        .editor()
        .text(buffer_id)
        .map_err(|e| format!("s6: read post-restore buffer text: {e:?}"))?
        .to_owned();
    if post_restore_text != s3.original_text {
        return Err(format!(
            "s6: post-restore buffer text != original_text (lens: {} vs {})",
            post_restore_text.len(),
            s3.original_text.len()
        ));
    }
    eprintln!("[s6] buffer verified: still at original text after checkpoint restore");

    Ok(())
}

// ─────────────────────────────────────────────────────────────────────────────
// Step s7: write evidence TOML
// ─────────────────────────────────────────────────────────────────────────────

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

    let mut toml = String::new();
    toml.push_str("schema_version = 1\n");
    toml.push_str(&format!("git_sha = \"{legion_sha}\"\n"));
    toml.push_str(&format!("started_utc = \"{started_utc}\"\n"));
    toml.push_str(&format!("finished_utc = \"{finished_utc}\"\n"));
    toml.push('\n');

    let overall_status = if steps.iter().any(|s| s.status == StepStatus::Failed) {
        "failed"
    } else if steps
        .iter()
        .all(|s| s.status == StepStatus::Passed || s.status == StepStatus::Skipped)
    {
        "passed"
    } else {
        "unknown"
    };
    toml.push_str(&format!("overall_status = \"{overall_status}\"\n\n"));

    for step in steps {
        toml.push_str("[[steps]]\n");
        toml.push_str(&format!("id = \"{}\"\n", step.id));
        toml.push_str(&format!("status = \"{}\"\n", step.status.as_str()));
        toml.push_str(&format!("started_utc = \"{}\"\n", step.started_utc));
        toml.push_str(&format!("finished_utc = \"{}\"\n", step.finished_utc));
        toml.push_str(&format!("duration_ms = {}\n", step.duration_ms));
        let detail = if step.detail.chars().count() > 256 {
            format!("{}...", step.detail.chars().take(256).collect::<String>())
        } else {
            step.detail.clone()
        };
        toml.push_str(&format!("detail = {:?}\n\n", detail));
    }

    let out_path = out_dir.join("gp2_report.toml");
    fs::write(&out_path, &toml).map_err(|e| format!("write {}: {e}", out_path.display()))?;
    eprintln!("[s7] wrote evidence: {}", out_path.display());

    if let Some(ev_dir) = evidence_dir {
        fs::create_dir_all(ev_dir)
            .map_err(|e| format!("create evidence_dir {}: {e}", ev_dir.display()))?;
        let ev_path = ev_dir.join("gp2_report.toml");
        fs::write(&ev_path, &toml)
            .map_err(|e| format!("write evidence copy {}: {e}", ev_path.display()))?;
        eprintln!("[s7] wrote evidence copy: {}", ev_path.display());
    }

    Ok(out_path)
}

// ─────────────────────────────────────────────────────────────────────────────
// Main
// ─────────────────────────────────────────────────────────────────────────────

fn main() {
    let args = match parse_args() {
        Ok(a) => a,
        Err(e) => {
            eprintln!("golden-path-2: argument error: {e}");
            eprintln!(
                "Usage: golden_path_2 --fixture-dir <path> [--out-dir <path>] [--record-evidence <path>]"
            );
            process::exit(2);
        }
    };

    let started_utc = utc_now();
    let mut steps: Vec<StepRecord> = Vec::new();

    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let legion_sha = resolve_legion_git_sha(&cwd);
    eprintln!("[gp2] Legion git SHA: {legion_sha}");
    eprintln!("[gp2] fixture dir: {}", args.fixture_dir.display());

    macro_rules! record_step {
        ($id:expr, $status:expr, $detail:expr, $duration_ms:expr, $started:expr, $finished:expr) => {
            steps.push(StepRecord {
                id: $id,
                started_utc: $started,
                finished_utc: $finished,
                duration_ms: $duration_ms,
                status: $status,
                detail: $detail,
            });
        };
    }

    // ── s1 ──────────────────────────────────────────────────────────────────
    let s1_start = utc_now();
    let (s1_result, s1_ms) = run_timer(|| run_s1(&args.fixture_dir));
    let s1_end = utc_now();
    let (temp_dir, mut app, workspace_id, generation) = match s1_result {
        Ok(r) => {
            eprintln!(
                "[s1] passed ({}ms); temp_dir={}",
                s1_ms,
                r.temp_dir.display()
            );
            record_step!(
                "s1",
                StepStatus::Passed,
                format!("fixture copied and workspace opened ({}ms)", s1_ms),
                s1_ms,
                s1_start,
                s1_end
            );
            (r.temp_dir, r.app, r.workspace_id, r.generation)
        }
        Err(e) => {
            eprintln!("[s1] FAILED: {e}");
            record_step!("s1", StepStatus::Failed, e.clone(), s1_ms, s1_start, s1_end);
            let _ = write_evidence(
                &args.out_dir,
                args.evidence_dir.as_deref(),
                &legion_sha,
                &started_utc,
                &utc_now(),
                &steps,
            );
            process::exit(1);
        }
    };

    // ── s2 ──────────────────────────────────────────────────────────────────
    let s2_start = utc_now();
    let (s2_result, s2_ms) = run_timer(|| run_s2(&temp_dir, &mut app));
    let s2_end = utc_now();
    let s2_buffer_id: Option<BufferId> = match s2_result {
        Ok(r) => {
            eprintln!(
                "[s2] passed ({}ms); buffer_id={:?} file_id={:?}",
                s2_ms, r.buffer_id, r.file_id
            );
            record_step!(
                "s2",
                StepStatus::Passed,
                format!(
                    "Assist mode enabled; src/main.rs opened; buffer_id={:?} file_id={:?} ({}ms)",
                    r.buffer_id, r.file_id, s2_ms
                ),
                s2_ms,
                s2_start,
                s2_end
            );
            Some(r.buffer_id)
        }
        Err(e) => {
            eprintln!("[s2] FAILED: {e}");
            record_step!("s2", StepStatus::Failed, e.clone(), s2_ms, s2_start, s2_end);
            None
        }
    };

    // ── s3 ──────────────────────────────────────────────────────────────────
    let s3_start = utc_now();
    let s3_outcome: Option<S3Result> = if let Some(buffer_id) = s2_buffer_id {
        let (s3_result, s3_ms) = run_timer(|| run_s3(&mut app, buffer_id));
        let s3_end = utc_now();
        match s3_result {
            Ok(r) => {
                eprintln!("[s3] passed ({}ms)", s3_ms);
                record_step!(
                    "s3",
                    StepStatus::Passed,
                    format!(
                        "ghost text received, accepted; buffer changed; undo available ({}ms)",
                        s3_ms
                    ),
                    s3_ms,
                    s3_start,
                    s3_end
                );
                Some(r)
            }
            Err(e) => {
                eprintln!("[s3] FAILED: {e}");
                record_step!("s3", StepStatus::Failed, e.clone(), s3_ms, s3_start, s3_end);
                None
            }
        }
    } else {
        let s3_end = utc_now();
        eprintln!("[s3] skipped — s2 failed (no buffer_id)");
        record_step!(
            "s3",
            StepStatus::Skipped,
            "skipped: s2 failed (no buffer_id available)".to_string(),
            0,
            s3_start,
            s3_end
        );
        None
    };

    // ── s4 ──────────────────────────────────────────────────────────────────
    let s4_start = utc_now();
    let (s4_result, s4_ms) = run_timer(run_s4);
    let s4_end = utc_now();
    match s4_result {
        Ok(()) => {
            eprintln!("[s4] passed ({}ms)", s4_ms);
            record_step!(
                "s4",
                StepStatus::Passed,
                format!(
                    "local-loopback → Completed; unauthorized remote → Refused ({}ms)",
                    s4_ms
                ),
                s4_ms,
                s4_start,
                s4_end
            );
        }
        Err(e) => {
            eprintln!("[s4] FAILED: {e}");
            record_step!("s4", StepStatus::Failed, e.clone(), s4_ms, s4_start, s4_end);
        }
    }

    // ── s5 ──────────────────────────────────────────────────────────────────
    let s5_start = utc_now();
    let (s5_result, s5_ms) = run_timer(|| run_s5(&temp_dir));
    let s5_end = utc_now();
    match s5_result {
        Ok(()) => {
            eprintln!("[s5] passed ({}ms)", s5_ms);
            record_step!(
                "s5",
                StepStatus::Passed,
                format!(
                    "context manifest assembled: file entries present; manifest_id valid; permissions non-empty ({}ms)",
                    s5_ms
                ),
                s5_ms,
                s5_start,
                s5_end
            );
        }
        Err(e) => {
            eprintln!("[s5] FAILED: {e}");
            record_step!("s5", StepStatus::Failed, e.clone(), s5_ms, s5_start, s5_end);
        }
    }

    // ── s6 ──────────────────────────────────────────────────────────────────
    let s6_start = utc_now();
    if let (Some(buffer_id), Some(s3)) = (s2_buffer_id, &s3_outcome) {
        let (s6_result, s6_ms) =
            run_timer(|| run_s6(&mut app, &temp_dir, buffer_id, workspace_id, generation, s3));
        let s6_end = utc_now();
        match s6_result {
            Ok(()) => {
                eprintln!("[s6] passed ({}ms)", s6_ms);
                record_step!(
                    "s6",
                    StepStatus::Passed,
                    format!(
                        "undo s3; CreateFile proposal applied; checkpoint verified; restore OK; buffer at original text ({}ms)",
                        s6_ms
                    ),
                    s6_ms,
                    s6_start,
                    s6_end
                );
            }
            Err(e) => {
                eprintln!("[s6] FAILED: {e}");
                record_step!("s6", StepStatus::Failed, e.clone(), s6_ms, s6_start, s6_end);
            }
        }
    } else {
        let s6_end = utc_now();
        eprintln!("[s6] skipped — s2 or s3 did not pass (no buffer_id or s3 result)");
        record_step!(
            "s6",
            StepStatus::Skipped,
            "skipped: s2 or s3 did not pass (checkpoint pipeline depends on accepted prediction)"
                .to_string(),
            0,
            s6_start,
            s6_end
        );
    }

    // ── s7 ──────────────────────────────────────────────────────────────────
    let s7_start = utc_now();
    let s7_wall = Instant::now();
    let finished_utc = utc_now();
    let first_result = write_evidence(
        &args.out_dir,
        None,
        &legion_sha,
        &started_utc,
        &finished_utc,
        &steps,
    );
    let s7_ms = s7_wall.elapsed().as_millis();
    let s7_end = utc_now();

    match &first_result {
        Ok(path) => eprintln!(
            "[s7] evidence written (preliminary, s1-s6): {}",
            path.display()
        ),
        Err(e) => eprintln!("[s7] FAILED to write evidence (pass 1): {e}"),
    }

    steps.push(StepRecord {
        id: "s7",
        started_utc: s7_start,
        finished_utc: s7_end.clone(),
        duration_ms: s7_ms,
        status: if first_result.is_ok() {
            StepStatus::Passed
        } else {
            StepStatus::Failed
        },
        detail: match &first_result {
            Ok(_) => format!("evidence TOML written ({}ms)", s7_ms),
            Err(e) => e.clone(),
        },
    });

    // Pass 2: rewrite with all steps including s7, copy to evidence_dir.
    match write_evidence(
        &args.out_dir,
        args.evidence_dir.as_deref(),
        &legion_sha,
        &started_utc,
        &s7_end,
        &steps,
    ) {
        Ok(path) => eprintln!("[s7] evidence rewritten (final, s1-s7): {}", path.display()),
        Err(e) => eprintln!("[s7] WARNING: pass-2 rewrite failed: {e}"),
    }

    // Print per-step summary.
    eprintln!("\n[gp2] SMOKE SUMMARY");
    for step in &steps {
        eprintln!(
            "  {} {} ({}ms): {}",
            step.id,
            step.status.as_str(),
            step.duration_ms,
            &step.detail[..step.detail.len().min(80)]
        );
    }

    // Clean up on success; leave for inspection on failure.
    let any_failed = steps.iter().any(|s| s.status == StepStatus::Failed);
    if any_failed {
        eprintln!(
            "\n[gp2] FAILED — temp workspace left for inspection: {}",
            temp_dir.display()
        );
        process::exit(1);
    } else {
        eprintln!(
            "\n[gp2] PASSED — cleaning up temp workspace: {}",
            temp_dir.display()
        );
        let _ = fs::remove_dir_all(&temp_dir);
        process::exit(0);
    }
}
