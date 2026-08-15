//! Legion-Bench live-local task runner.
//!
//! Invoked by `cargo run -p xtask -- legion-bench --mode live-local` (subprocess
//! model — xtask cannot depend on legion-app, so it spawns this binary exactly
//! like the golden-path runners). The binary:
//!
//! 1. reads a `LiveRunInput` TOML (endpoint, model, api key, task specs),
//! 2. per task: copies the fixture to a temp checkout, git-inits a baseline
//!    commit, opens the checkout as a Trusted workspace in Delegate mode, and
//!    drives `AppComposition::start_delegated_task` — the SAME worktree/broker/
//!    scope containment the product uses — against a live OpenAI-compatible
//!    endpoint,
//! 3. applies the resulting proposals to the checkout (proposal pipeline where
//!    possible, direct write of the accepted content otherwise — recorded in
//!    the notes), runs the task's verification command with a timeout, and
//! 4. writes raw measured metrics as a `LiveRunOutput` TOML. Scoring happens
//!    in xtask.
//!
//! The binary never opens the network beyond the configured endpoint (the
//! agent loop tools are all local), never writes inside the Legion repo, and
//! cleans up its temp checkouts.

use std::{
    collections::HashSet,
    fs,
    path::{Path, PathBuf},
    process,
    sync::{Arc, Mutex},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use legion_ai::{
    ChatCompletionRequest, ChatCompletionResponse, EmbeddingRequest, EmbeddingResponse,
    InlinePredictionRequest, InlinePredictionResponse, ModelProvider, ProviderCapabilities,
    ProviderError, ProviderId,
    tool_calls::{ToolCallingProvider, ToolCompletionRequest, ToolCompletionResponse},
};
use legion_ai_providers::{
    OpenAiCompatibleProvider, ProviderHttpTransport, ReqwestProviderHttpTransport,
};
use legion_app::{AppComposition, AppDelegatedTaskOutcome, AppProductMode};
use legion_protocol::{
    AssistedAiEditProposalOutput, CanonicalPath, CapabilityId, CorrelationId, CreateFileProposal,
    DelegatedTaskLoopStepKind, DelegatedTaskProposalHunkDisposition, DelegatedTaskRiskTolerance,
    DelegatedTaskScope, DelegatedTaskScopeTargetKind, LegionToolKind, PreviewSummary, PrincipalId,
    ProposalId, ProposalPayload, ProposalRequest, ProposalResponse, ProposalVersionPreconditions,
    TimestampMillis, WorkspaceProposal, WorkspaceTrustState,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;

const PRINCIPAL: &str = "legion-bench-live";

// ─── Interchange DTOs (wire contract with xtask::legion_bench_corpus) ────────

#[derive(Debug, Deserialize)]
struct LiveRunInput {
    schema_version: u32,
    endpoint: String,
    model: String,
    api_key: String,
    tasks: Vec<LiveRunTaskInput>,
}

#[derive(Debug, Clone, Deserialize)]
struct LiveRunTaskInput {
    id: String,
    fixture_dir: String,
    prompt: String,
    target_kind: String,
    #[serde(default)]
    target_path: Option<String>,
    allowed_tools: Vec<String>,
    forbidden_paths: Vec<String>,
    verification_command: String,
    expected_exit: i32,
    timeout_secs: u64,
    expected_files: Vec<String>,
}

#[derive(Debug, Serialize)]
struct LiveRunOutput {
    schema_version: u32,
    results: Vec<LiveRunTaskResult>,
}

#[derive(Debug, Clone, Serialize)]
struct LiveRunTaskResult {
    id: String,
    outcome: String,
    task_success: bool,
    tests_passed: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    verification_exit: Option<i32>,
    proposals_total: u32,
    proposals_applied: u32,
    diff_files: u32,
    turns: u32,
    tool_calls: u32,
    duplicate_tool_calls: u32,
    retries: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    context_tokens: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    generation_tokens: Option<u64>,
    wall_ms: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
    notes: String,
}

impl LiveRunTaskResult {
    fn new(id: &str) -> Self {
        Self {
            id: id.to_string(),
            outcome: "error".to_string(),
            task_success: false,
            tests_passed: false,
            verification_exit: None,
            proposals_total: 0,
            proposals_applied: 0,
            diff_files: 0,
            turns: 0,
            tool_calls: 0,
            duplicate_tool_calls: 0,
            retries: 0,
            context_tokens: None,
            generation_tokens: None,
            wall_ms: 0,
            error: None,
            notes: String::new(),
        }
    }
}

// ─── Usage/duplicate metering transport ──────────────────────────────────────

#[derive(Debug, Default)]
struct MeterState {
    requests: u32,
    prompt_tokens: u64,
    completion_tokens: u64,
    usage_seen: bool,
    seen_calls: HashSet<(String, String)>,
    duplicate_tool_calls: u32,
}

/// Wraps the reqwest transport so the runner can observe token usage and
/// repeated (name, arguments) tool calls without changing provider or loop
/// code. All observation happens on the raw response JSON.
#[derive(Clone)]
struct MeteringTransport {
    inner: ReqwestProviderHttpTransport,
    state: Arc<Mutex<MeterState>>,
}

impl MeteringTransport {
    fn new() -> Self {
        Self {
            inner: ReqwestProviderHttpTransport,
            state: Arc::new(Mutex::new(MeterState::default())),
        }
    }

    /// Record usage + duplicate tool-call observations from one chat response.
    fn record(state: &mut MeterState, response: &Value) {
        state.requests += 1;
        if let Some(usage) = response.get("usage") {
            if let Some(prompt) = usage.get("prompt_tokens").and_then(Value::as_u64) {
                state.prompt_tokens += prompt;
                state.usage_seen = true;
            }
            if let Some(completion) = usage.get("completion_tokens").and_then(Value::as_u64) {
                state.completion_tokens += completion;
                state.usage_seen = true;
            }
        }
        let tool_calls = response
            .get("choices")
            .and_then(Value::as_array)
            .and_then(|choices| choices.first())
            .and_then(|choice| choice.get("message"))
            .and_then(|message| message.get("tool_calls"))
            .and_then(Value::as_array);
        if let Some(tool_calls) = tool_calls {
            for call in tool_calls {
                let function = call.get("function");
                let name = function
                    .and_then(|f| f.get("name"))
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string();
                let arguments = function
                    .and_then(|f| f.get("arguments"))
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string();
                if !state.seen_calls.insert((name, arguments)) {
                    state.duplicate_tool_calls += 1;
                }
            }
        }
    }
}

impl ProviderHttpTransport for MeteringTransport {
    fn post_json(
        &self,
        endpoint: &str,
        bearer_token: Option<&str>,
        payload: Value,
    ) -> Result<Value, ProviderError> {
        let response = self.inner.post_json(endpoint, bearer_token, payload)?;
        if let Ok(mut state) = self.state.lock() {
            Self::record(&mut state, &response);
        }
        Ok(response)
    }
}

// ─── Model-override provider wrapper ─────────────────────────────────────────

/// `start_delegated_task` builds its loop config with a hardcoded hosted model
/// id. The wire model id must be the one the local endpoint serves, so this
/// wrapper rewrites `request.model` before delegating.
struct ModelOverrideProvider<P> {
    inner: P,
    model: String,
}

impl<P: ModelProvider> ModelProvider for ModelOverrideProvider<P> {
    fn provider_id(&self) -> ProviderId {
        self.inner.provider_id()
    }

    fn capabilities(&self) -> ProviderCapabilities {
        self.inner.capabilities()
    }

    fn complete(
        &self,
        mut request: ChatCompletionRequest,
    ) -> Result<ChatCompletionResponse, ProviderError> {
        request.model = self.model.clone();
        self.inner.complete(request)
    }

    fn embed(&self, request: EmbeddingRequest) -> Result<EmbeddingResponse, ProviderError> {
        self.inner.embed(request)
    }

    fn predict_inline(
        &self,
        request: InlinePredictionRequest,
    ) -> Result<InlinePredictionResponse, ProviderError> {
        self.inner.predict_inline(request)
    }
}

impl<P: ToolCallingProvider> ToolCallingProvider for ModelOverrideProvider<P> {
    fn complete_with_tools(
        &self,
        mut request: ToolCompletionRequest,
    ) -> Result<ToolCompletionResponse, ProviderError> {
        request.model = self.model.clone();
        self.inner.complete_with_tools(request)
    }
}

// ─── Checkout helpers ────────────────────────────────────────────────────────

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
        .map_err(|e| format!("git {args:?} spawn failed: {e}"))?;
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).into_owned())
    } else {
        Err(format!(
            "git {args:?} failed ({}): {}",
            output.status,
            String::from_utf8_lossy(&output.stderr)
        ))
    }
}

/// Copy the fixture into a fresh temp checkout with a git baseline commit.
fn prepare_checkout(fixture_dir: &Path, task_id: &str) -> Result<PathBuf, String> {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let checkout = std::env::temp_dir().join(format!(
        "legion-bench-live-{task_id}-{}-{nanos}",
        process::id()
    ));
    copy_dir_recursive(fixture_dir, &checkout)?;
    git_cmd(&checkout, &["init", "-b", "main"])?;
    git_cmd(&checkout, &["config", "user.email", "bench@legion.test"])?;
    git_cmd(&checkout, &["config", "user.name", "Legion Bench"])?;
    git_cmd(&checkout, &["add", "."])?;
    git_cmd(
        &checkout,
        &[
            "commit",
            "--allow-empty",
            "-m",
            "baseline: legion-bench fixture",
        ],
    )?;
    Ok(checkout)
}

/// Count files changed vs the baseline commit (staged so new files count too).
fn count_diff_files(checkout: &Path) -> Result<u32, String> {
    git_cmd(checkout, &["add", "-A"])?;
    let diff = git_cmd(checkout, &["diff", "--cached", "--name-only"])?;
    Ok(diff.lines().filter(|line| !line.trim().is_empty()).count() as u32)
}

fn parse_tool(name: &str) -> Result<LegionToolKind, String> {
    match name {
        "read" => Ok(LegionToolKind::Read),
        "grep" => Ok(LegionToolKind::Grep),
        "glob" => Ok(LegionToolKind::Glob),
        "outline" => Ok(LegionToolKind::Outline),
        "edit-as-proposal" => Ok(LegionToolKind::EditAsProposal),
        "terminal-command" => Ok(LegionToolKind::TerminalCommand),
        other => Err(format!("unknown tool `{other}`")),
    }
}

fn build_scope(checkout: &Path, task: &LiveRunTaskInput) -> Result<DelegatedTaskScope, String> {
    let target_kind = match task.target_kind.as_str() {
        "repo" => DelegatedTaskScopeTargetKind::Repo,
        "module" => DelegatedTaskScopeTargetKind::Module,
        "file" => DelegatedTaskScopeTargetKind::File,
        other => return Err(format!("unknown target_kind `{other}`")),
    };
    let target_path = match (&target_kind, &task.target_path) {
        (DelegatedTaskScopeTargetKind::Repo, _) => None,
        (_, Some(rel)) => Some(CanonicalPath(
            checkout.join(rel).to_string_lossy().into_owned(),
        )),
        (_, None) => return Err("target_path required for file/module targets".to_string()),
    };
    let allowed_tools = task
        .allowed_tools
        .iter()
        .map(|name| parse_tool(name))
        .collect::<Result<Vec<_>, _>>()?;
    let forbidden_paths = task
        .forbidden_paths
        .iter()
        .map(|rel| CanonicalPath(checkout.join(rel).to_string_lossy().into_owned()))
        .collect();
    Ok(DelegatedTaskScope {
        target_kind,
        workspace_root: CanonicalPath(checkout.to_string_lossy().into_owned()),
        target_path,
        risk_tolerance: DelegatedTaskRiskTolerance::Balanced,
        allowed_tools,
        forbidden_paths,
        schema_version: 1,
    })
}

// ─── Proposal apply ──────────────────────────────────────────────────────────

/// Apply one CreateFile proposal via the real proposal lifecycle pipeline
/// (register → Validate → Preview → Apply), mirroring GP-3 s8.
fn apply_via_pipeline(
    app: &mut AppComposition,
    checkout: &Path,
    absolute_target: &Path,
    content: &str,
    proposal_id: u64,
) -> Result<(), String> {
    let generation = app
        .open_workspace(
            checkout,
            WorkspaceTrustState::Trusted,
            PrincipalId(PRINCIPAL.to_string()),
        )
        .map_err(|e| format!("workspace generation refresh failed: {e:?}"))?
        .generation;

    let proposal = WorkspaceProposal {
        proposal_id: ProposalId(proposal_id),
        principal: PrincipalId(PRINCIPAL.to_string()),
        capability: CapabilityId("fs.write".to_string()),
        correlation_id: CorrelationId(proposal_id),
        payload: ProposalPayload::CreateFile(CreateFileProposal {
            path: CanonicalPath(absolute_target.to_string_lossy().into_owned()),
            initial_content: Some(content.to_string()),
        }),
        preconditions: ProposalVersionPreconditions {
            file_version: None,
            buffer_version: None,
            snapshot_id: None,
            generation: None,
            file_content_version: None,
            workspace_generation: Some(generation),
            expected_fingerprint: None,
            expected_file_length: None,
            expected_modified_at: None,
        },
        preview: PreviewSummary {
            summary: "legion-bench live proposal apply".to_string(),
            details: Vec::new(),
        },
        expires_at: None,
        created_at: TimestampMillis(1),
    };

    match app
        .register_proposal_lifecycle(&proposal)
        .map_err(|e| format!("register failed: {e:?}"))?
    {
        ProposalResponse::Created(_) => {}
        other => return Err(format!("register returned {other:?}")),
    }
    match app
        .handle_proposal_request(ProposalRequest::Validate(proposal.clone()))
        .map_err(|e| format!("validate failed: {e:?}"))?
    {
        ProposalResponse::Validated(_) => {}
        other => return Err(format!("validate returned {other:?}")),
    }
    match app
        .handle_proposal_request(ProposalRequest::Preview(proposal.clone()))
        .map_err(|e| format!("preview failed: {e:?}"))?
    {
        ProposalResponse::Previewed { .. } => {}
        other => return Err(format!("preview returned {other:?}")),
    }
    match app
        .handle_proposal_request(ProposalRequest::Apply(proposal))
        .map_err(|e| format!("apply failed: {e:?}"))?
    {
        ProposalResponse::Applied(_) => Ok(()),
        other => Err(format!("apply returned {other:?}")),
    }
}

/// Apply the loop's proposals to the checkout. Returns (applied, notes).
///
/// The delegated proposal generator emits path-based CreateFile payloads with
/// full replacement content (checkout-relative, forward slashes). New files go
/// through the real proposal pipeline; existing files cannot (the CreateFile
/// apply route rejects existing destinations), so the harness — acting as the
/// reviewer who accepted the hunks — materializes the accepted content with a
/// direct write. Every route decision is recorded in the notes.
fn apply_proposals(
    app: &mut AppComposition,
    checkout: &Path,
    proposals: &[AssistedAiEditProposalOutput],
) -> (u32, Vec<String>) {
    let mut applied = 0_u32;
    let mut notes = Vec::new();
    for (index, output) in proposals.iter().enumerate() {
        // Record the reviewer accept on the real review surface (best effort).
        let hunk_id = format!(
            "delegate:proposal:{}:metadata-chunk:0",
            output.proposal_id.0
        );
        if let Err(err) = app.review_delegate_proposal_hunk(
            output.proposal_id,
            hunk_id,
            DelegatedTaskProposalHunkDisposition::Accepted,
        ) {
            notes.push(format!("proposal[{index}]: hunk accept failed: {err:?}"));
        }

        let ProposalPayload::CreateFile(create) = &output.payload else {
            notes.push(format!(
                "proposal[{index}]: unsupported payload variant (expected CreateFile); skipped"
            ));
            continue;
        };
        let relative = create.path.0.clone();
        if Path::new(&relative).is_absolute() || relative.split('/').any(|part| part == "..") {
            notes.push(format!(
                "proposal[{index}]: rejected non-relative target `{relative}`"
            ));
            continue;
        }
        let absolute = checkout.join(&relative);
        let content = create.initial_content.clone().unwrap_or_default();
        if let Some(parent) = absolute.parent() {
            let _ = fs::create_dir_all(parent);
        }

        if absolute.exists() {
            match fs::write(&absolute, &content) {
                Ok(()) => {
                    applied += 1;
                    notes.push(format!(
                        "proposal[{index}] {relative}: applied by direct write \
                         (create-file pipeline rejects existing destinations)"
                    ));
                }
                Err(err) => {
                    notes.push(format!("proposal[{index}] {relative}: write failed: {err}"));
                }
            }
        } else {
            match apply_via_pipeline(app, checkout, &absolute, &content, 900_000 + index as u64) {
                Ok(()) => {
                    applied += 1;
                    notes.push(format!(
                        "proposal[{index}] {relative}: applied via proposal pipeline"
                    ));
                }
                Err(err) => match fs::write(&absolute, &content) {
                    Ok(()) => {
                        applied += 1;
                        notes.push(format!(
                            "proposal[{index}] {relative}: pipeline failed ({err}); \
                             applied by direct write"
                        ));
                    }
                    Err(write_err) => {
                        notes.push(format!(
                            "proposal[{index}] {relative}: pipeline failed ({err}); \
                             direct write failed: {write_err}"
                        ));
                    }
                },
            }
        }
    }
    (applied, notes)
}

// ─── Verification ────────────────────────────────────────────────────────────

/// Run the verification command in the checkout. Returns `Ok(Some(exit))`,
/// `Ok(None)` when the command was killed on timeout.
fn run_verification(command: &str, cwd: &Path, timeout: Duration) -> Result<Option<i32>, String> {
    let mut cmd = if cfg!(windows) {
        let mut cmd = process::Command::new("cmd");
        cmd.args(["/C", command]);
        cmd
    } else {
        let mut cmd = process::Command::new("sh");
        cmd.args(["-c", command]);
        cmd
    };
    // Isolate cargo builds inside the checkout even when the caller exported a
    // global CARGO_TARGET_DIR.
    cmd.current_dir(cwd)
        .env("CARGO_TARGET_DIR", cwd.join("target"));
    let mut child = cmd
        .spawn()
        .map_err(|err| format!("unable to spawn verification command `{command}`: {err}"))?;
    let deadline = Instant::now() + timeout;
    loop {
        match child.try_wait() {
            Ok(Some(status)) => return Ok(Some(status.code().unwrap_or(-1))),
            Ok(None) => {
                if Instant::now() >= deadline {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Ok(None);
                }
                std::thread::sleep(Duration::from_millis(250));
            }
            Err(err) => return Err(format!("verification wait failed: {err}")),
        }
    }
}

// ─── Per-task execution ──────────────────────────────────────────────────────

fn run_one_task(
    task: &LiveRunTaskInput,
    endpoint: &str,
    model: &str,
    api_key: &str,
) -> LiveRunTaskResult {
    let started = Instant::now();
    let mut result = LiveRunTaskResult::new(&task.id);
    let mut notes: Vec<String> = Vec::new();

    let fixture_dir = PathBuf::from(&task.fixture_dir);
    let checkout = match prepare_checkout(&fixture_dir, &task.id) {
        Ok(checkout) => checkout,
        Err(err) => {
            result.error = Some(format!("checkout preparation failed: {err}"));
            result.wall_ms = started.elapsed().as_millis() as u64;
            return result;
        }
    };

    let outcome = (|| -> Result<AppDelegatedTaskOutcome, String> {
        let mut app = AppComposition::new();
        app.open_workspace(
            &checkout,
            WorkspaceTrustState::Trusted,
            PrincipalId(PRINCIPAL.to_string()),
        )
        .map_err(|e| format!("open_workspace failed: {e:?}"))?;
        app.set_product_mode(AppProductMode::Delegate);

        let scope = build_scope(&checkout, task)?;

        let meter = MeteringTransport::new();
        let provider = ModelOverrideProvider {
            inner: OpenAiCompatibleProvider::with_transport(
                "legion-bench-live",
                endpoint.to_string(),
                Some(api_key.to_string()),
                meter.clone(),
            ),
            model: model.to_string(),
        };

        // Wall-clock watchdog: cancel the loop through the product kill-switch
        // flag once the task timeout elapses. Requires the test-helpers seam
        // (xtask always compiles this binary with --features test-helpers).
        #[cfg(feature = "test-helpers")]
        {
            let flag = legion_app::SharedCancellationFlag::new();
            app.inject_cancellation_flag_for_test(flag.clone());
            let timeout = Duration::from_secs(task.timeout_secs);
            std::thread::spawn(move || {
                std::thread::sleep(timeout);
                flag.cancel();
            });
        }

        let outcome = app
            .start_delegated_task(task.prompt.clone(), scope, &provider)
            .map_err(|e| format!("delegated task failed: {e:?}"))?;

        // Harvest meter observations before app/provider drop.
        if let Ok(state) = meter.state.lock() {
            result.duplicate_tool_calls = state.duplicate_tool_calls;
            if state.usage_seen {
                result.context_tokens = Some(state.prompt_tokens);
                result.generation_tokens = Some(state.completion_tokens);
            }
            notes.push(format!("model_http_requests={}", state.requests));
        }

        // Apply proposals + audit metrics for the completed path.
        if let AppDelegatedTaskOutcome::Completed { proposals, .. } = &outcome {
            result.proposals_total = proposals.len() as u32;
            let (applied, mut apply_notes) = apply_proposals(&mut app, &checkout, proposals);
            result.proposals_applied = applied;
            notes.append(&mut apply_notes);
        }
        Ok(outcome)
    })();

    let audit_metrics = |steps: &[legion_protocol::DelegatedTaskLoopStepRecord]| {
        let turns = steps
            .iter()
            .filter(|s| s.kind == DelegatedTaskLoopStepKind::ModelResponse)
            .count() as u32;
        let tool_calls = steps
            .iter()
            .filter(|s| s.kind == DelegatedTaskLoopStepKind::ToolCallRequest)
            .count() as u32;
        let retries = steps
            .iter()
            .filter(|s| s.kind == DelegatedTaskLoopStepKind::ToolCallRejected)
            .count() as u32;
        (turns, tool_calls, retries)
    };

    match outcome {
        Ok(AppDelegatedTaskOutcome::Completed { audit_steps, .. }) => {
            result.outcome = "completed".to_string();
            let (turns, tool_calls, retries) = audit_metrics(&audit_steps);
            result.turns = turns;
            result.tool_calls = tool_calls;
            result.retries = retries;

            match count_diff_files(&checkout) {
                Ok(count) => result.diff_files = count,
                Err(err) => notes.push(format!("diff count failed: {err}")),
            }

            match run_verification(
                &task.verification_command,
                &checkout,
                Duration::from_secs(task.timeout_secs),
            ) {
                Ok(Some(exit)) => {
                    result.verification_exit = Some(exit);
                    result.tests_passed = exit == task.expected_exit;
                    notes.push(format!(
                        "verification `{}` exit={exit} expected={}",
                        task.verification_command, task.expected_exit
                    ));
                }
                Ok(None) => {
                    notes.push(format!(
                        "verification `{}` timed out after {}s and was killed",
                        task.verification_command, task.timeout_secs
                    ));
                }
                Err(err) => notes.push(format!("verification failed to run: {err}")),
            }

            let missing: Vec<&String> = task
                .expected_files
                .iter()
                .filter(|rel| !checkout.join(rel.as_str()).exists())
                .collect();
            let expected_files_ok = missing.is_empty();
            if !expected_files_ok {
                notes.push(format!("missing expected files: {missing:?}"));
            }

            result.task_success = result.tests_passed
                && expected_files_ok
                && result.proposals_applied == result.proposals_total;
        }
        Ok(AppDelegatedTaskOutcome::Blocked {
            reason,
            audit_steps,
        }) => {
            result.outcome = "blocked".to_string();
            let (turns, tool_calls, retries) = audit_metrics(&audit_steps);
            result.turns = turns;
            result.tool_calls = tool_calls;
            result.retries = retries;
            result.error = Some(format!("loop blocked: {reason}"));
        }
        Ok(AppDelegatedTaskOutcome::BudgetExhausted {
            reason,
            audit_steps,
        }) => {
            result.outcome = "budget_exhausted".to_string();
            let (turns, tool_calls, retries) = audit_metrics(&audit_steps);
            result.turns = turns;
            result.tool_calls = tool_calls;
            result.retries = retries;
            result.error = Some(format!("budget exhausted: {reason}"));
        }
        Ok(AppDelegatedTaskOutcome::Cancelled) => {
            result.outcome = "cancelled".to_string();
            result.error = Some(format!(
                "task cancelled by the wall-clock watchdog after {}s",
                task.timeout_secs
            ));
        }
        Ok(AppDelegatedTaskOutcome::SandboxAllocationFailed { reason }) => {
            result.outcome = "error".to_string();
            result.error = Some(format!("sandbox allocation failed: {reason}"));
        }
        Err(err) => {
            result.outcome = "error".to_string();
            result.error = Some(err);
        }
    }

    result.wall_ms = started.elapsed().as_millis() as u64;
    result.notes = notes.join("; ");
    if result.notes.len() > 2000 {
        let mut cut = 2000;
        while cut > 0 && !result.notes.is_char_boundary(cut) {
            cut -= 1;
        }
        result.notes.truncate(cut);
        result.notes.push_str("...");
    }

    let _ = fs::remove_dir_all(&checkout);
    result
}

// ─── Main ────────────────────────────────────────────────────────────────────

struct Args {
    input: PathBuf,
    output: PathBuf,
}

fn parse_args() -> Result<Args, String> {
    let argv: Vec<String> = std::env::args().collect();
    let mut input = None;
    let mut output = None;
    let mut i = 1;
    while i < argv.len() {
        match argv[i].as_str() {
            "--input" => {
                i += 1;
                input = Some(PathBuf::from(argv.get(i).ok_or("--input needs a value")?));
            }
            "--output" => {
                i += 1;
                output = Some(PathBuf::from(argv.get(i).ok_or("--output needs a value")?));
            }
            other => return Err(format!("unknown argument `{other}`")),
        }
        i += 1;
    }
    Ok(Args {
        input: input.ok_or("--input is required")?,
        output: output.ok_or("--output is required")?,
    })
}

fn write_results(path: &Path, results: &[LiveRunTaskResult]) -> Result<(), String> {
    let output = LiveRunOutput {
        schema_version: 1,
        results: results.to_vec(),
    };
    let text = toml::to_string_pretty(&output)
        .map_err(|err| format!("unable to serialize results: {err}"))?;
    fs::write(path, text).map_err(|err| format!("unable to write {}: {err}", path.display()))
}

fn main() {
    let args = match parse_args() {
        Ok(args) => args,
        Err(err) => {
            eprintln!("legion_bench_live: argument error: {err}");
            eprintln!("Usage: legion_bench_live --input <spec.toml> --output <results.toml>");
            process::exit(2);
        }
    };
    let input_text = match fs::read_to_string(&args.input) {
        Ok(text) => text,
        Err(err) => {
            eprintln!(
                "legion_bench_live: unable to read input `{}`: {err}",
                args.input.display()
            );
            process::exit(2);
        }
    };
    let input: LiveRunInput = match toml::from_str(&input_text) {
        Ok(input) => input,
        Err(err) => {
            eprintln!(
                "legion_bench_live: unable to parse input `{}`: {err}",
                args.input.display()
            );
            process::exit(2);
        }
    };
    if input.schema_version != 1 {
        eprintln!(
            "legion_bench_live: unsupported input schema_version {}",
            input.schema_version
        );
        process::exit(2);
    }

    eprintln!(
        "legion_bench_live: endpoint={} model={} tasks={}",
        input.endpoint,
        input.model,
        input.tasks.len()
    );

    let mut results: Vec<LiveRunTaskResult> = Vec::new();
    for (index, task) in input.tasks.iter().enumerate() {
        eprintln!(
            "legion_bench_live: [{}/{}] running task {}",
            index + 1,
            input.tasks.len(),
            task.id
        );
        let result = run_one_task(task, &input.endpoint, &input.model, &input.api_key);
        eprintln!(
            "legion_bench_live: [{}/{}] {} outcome={} task_success={} tests_passed={} \
             turns={} tool_calls={} wall_ms={}{}",
            index + 1,
            input.tasks.len(),
            task.id,
            result.outcome,
            result.task_success,
            result.tests_passed,
            result.turns,
            result.tool_calls,
            result.wall_ms,
            result
                .error
                .as_deref()
                .map(|err| format!(" error={err}"))
                .unwrap_or_default(),
        );
        results.push(result);
        // Incremental write so a crash mid-run still leaves usable results.
        if let Err(err) = write_results(&args.output, &results) {
            eprintln!("legion_bench_live: {err}");
            process::exit(1);
        }
    }

    eprintln!(
        "legion_bench_live: done; results written to {}",
        args.output.display()
    );
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_fixture() -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("legion-bench-live-test-{nanos}"));
        fs::create_dir_all(dir.join("src")).expect("create fixture dirs");
        fs::write(
            dir.join("Cargo.toml"),
            "[package]\nname = \"bench-test-fixture\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
        )
        .expect("write Cargo.toml");
        fs::write(dir.join("src").join("main.rs"), "fn main() {}\n").expect("write main.rs");
        dir
    }

    #[test]
    fn unreachable_endpoint_fails_gracefully_without_panic() {
        let fixture = temp_fixture();
        let task = LiveRunTaskInput {
            id: "graceful-failure".to_string(),
            fixture_dir: fixture.to_string_lossy().into_owned(),
            prompt: "Fix the bug.".to_string(),
            target_kind: "repo".to_string(),
            target_path: None,
            allowed_tools: vec!["read".to_string(), "edit-as-proposal".to_string()],
            forbidden_paths: vec![],
            verification_command: "cargo test --offline".to_string(),
            expected_exit: 0,
            timeout_secs: 60,
            expected_files: vec![],
        };
        // Port 9 (discard) on loopback: nothing listens there, so the provider
        // fails on the first model call. The run must produce a structured
        // error result — no panic, no partial state.
        let result = run_one_task(&task, "http://127.0.0.1:9/v1", "test-model", "unused-key");
        assert_eq!(
            result.outcome, "error",
            "notes={} err={:?}",
            result.notes, result.error
        );
        assert!(!result.task_success);
        assert!(!result.tests_passed);
        let error = result.error.expect("error message must be present");
        assert!(
            error.contains("provider error") || error.contains("delegated"),
            "error should name the provider failure, got: {error}"
        );
        let _ = fs::remove_dir_all(&fixture);
    }

    #[test]
    fn meter_records_usage_and_duplicate_tool_calls() {
        let mut state = MeterState::default();
        let response: Value = serde_json::json!({
            "choices": [{
                "message": {
                    "tool_calls": [
                        {"function": {"name": "read", "arguments": "{\"path\":\"a.rs\"}"}},
                        {"function": {"name": "read", "arguments": "{\"path\":\"a.rs\"}"}},
                        {"function": {"name": "grep", "arguments": "{\"pattern\":\"x\"}"}}
                    ]
                }
            }],
            "usage": {"prompt_tokens": 120, "completion_tokens": 30}
        });
        MeteringTransport::record(&mut state, &response);
        MeteringTransport::record(&mut state, &response);
        assert_eq!(state.requests, 2);
        assert_eq!(state.prompt_tokens, 240);
        assert_eq!(state.completion_tokens, 60);
        assert!(state.usage_seen);
        // First response: 1 duplicate (second identical read). Second response
        // repeats all three already-seen calls: +3.
        assert_eq!(state.duplicate_tool_calls, 4);
    }

    #[test]
    fn verification_reports_exit_code_and_timeout() {
        let dir = temp_fixture();
        // `exit N` is valid in both cmd.exe and sh.
        assert_eq!(
            run_verification("exit 0", &dir, Duration::from_secs(30)).unwrap(),
            Some(0)
        );
        assert_eq!(
            run_verification("exit 3", &dir, Duration::from_secs(30)).unwrap(),
            Some(3)
        );
        let sleep_cmd = if cfg!(windows) {
            "ping -n 30 127.0.0.1 > NUL"
        } else {
            "sleep 30"
        };
        assert_eq!(
            run_verification(sleep_cmd, &dir, Duration::from_millis(600)).unwrap(),
            None
        );
        let _ = fs::remove_dir_all(&dir);
    }
}
