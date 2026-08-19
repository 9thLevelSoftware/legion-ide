//! Legion-Bench live-local task runner.
//!
//! Invoked by `cargo run -p xtask -- legion-bench --mode live-local` (subprocess
//! model — xtask cannot depend on legion-app, so it spawns this binary exactly
//! like the golden-path runners). The binary:
//!
//! 1. reads a `LiveRunInput` TOML (endpoint, model, task specs) and receives
//!    the API key only through the subprocess environment,
//! 2. per task: copies the fixture to a temp checkout, git-inits a baseline
//!    commit, opens the checkout as a Trusted workspace in Delegate mode, and
//!    drives `AppComposition::start_delegated_task` — the SAME worktree/broker/
//!    scope containment the product uses — against a live OpenAI-compatible
//!    endpoint,
//! 3. applies the resulting proposals to the checkout through the proposal and
//!    save pipelines, runs the task's verification command with a timeout, and
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
use legion_app::{AppComposition, AppDelegatedTaskOutcome, AppProductMode, AppSaveOutcome};
use legion_protocol::{
    AssistedAiEditProposalOutput, ByteRange, CanonicalPath, CapabilityId, CorrelationId,
    CreateFileProposal, DelegatedTaskLoopStepKind, DelegatedTaskProposalHunkDisposition,
    DelegatedTaskRiskTolerance, DelegatedTaskScope, DelegatedTaskScopeTargetKind, EditBatch,
    LegionToolKind, PreviewSummary, PrincipalId, ProposalAffectedTarget, ProposalId,
    ProposalPayload, ProposalRequest, ProposalResponse, ProposalTargetCoverage,
    ProposalTargetCoverageKind, ProposalTargetKind, ProposalVersionPreconditions, RedactionHint,
    TextEdit, TextRange, TimestampMillis, WorkspaceEditProposalPayload, WorkspaceEditSourceKind,
    WorkspaceProposal, WorkspaceTextEdit, WorkspaceTrustState,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;

const PRINCIPAL: &str = "legion-bench-live";
const API_KEY_ENV: &str = "LEGION_BENCH_API_KEY";

// ─── Interchange DTOs (wire contract with xtask::legion_bench_corpus) ────────

#[derive(Debug, Deserialize)]
struct LiveRunInput {
    schema_version: u32,
    endpoint: String,
    model: String,
    /// `live` | `record` | `replay`. Absent means `live`, so an input written
    /// by an older xtask still runs.
    #[serde(default)]
    provider_mode: Option<String>,
    /// Directory holding per-task cassettes. Required by `record` and `replay`.
    #[serde(default)]
    cassette_dir: Option<String>,
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
    /// Whether the task's verification command already passed before the model
    /// ran.
    ///
    /// Refactor tasks are supposed to preserve behaviour, so their tests pass
    /// on the untouched fixture. Without this field `tests_passed` reads as an
    /// achievement on those tasks, and a model that does nothing at all scores
    /// four of thirteen — which is exactly how the raw arm first appeared to
    /// beat the governed one.
    tests_passed_at_rest: bool,
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
    /// Model named by the replayed cassette. Empty when not replaying.
    /// Carried out to xtask so a replayed report can cite the model whose
    /// answers it replayed without xtask having to parse the tape.
    #[serde(skip_serializing_if = "String::is_empty")]
    cassette_model: String,
    /// `governed` | `raw` — the arm the replayed cassette was cut under.
    #[serde(skip_serializing_if = "String::is_empty")]
    cassette_arm: String,
    /// Recorded model exchanges replayed (or captured) for this task.
    cassette_exchanges: u32,
    /// Replayed exchanges whose request no longer fingerprints to the one that
    /// was recorded. Zero means the loop asked the model exactly what it asked
    /// when the cassette was cut; non-zero means the agent's request shape
    /// moved and the cassette is answering a question nobody asked.
    cassette_drift: u32,
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
            tests_passed_at_rest: false,
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
            cassette_model: String::new(),
            cassette_arm: String::new(),
            cassette_exchanges: 0,
            cassette_drift: 0,
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

/// How the runner obtains model responses.
///
/// `Replay` is what makes the benchmark runnable offline and byte-repeatably:
/// the agent loop, the tools, the proposal pipeline and the verification
/// command all execute for real against a real fixture checkout, and only the
/// model's side of the conversation comes from disk.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ProviderMode {
    Live,
    Record,
    Replay,
}

impl ProviderMode {
    fn parse(value: &str) -> Result<Self, String> {
        match value {
            "live" => Ok(Self::Live),
            "record" => Ok(Self::Record),
            "replay" => Ok(Self::Replay),
            other => Err(format!(
                "unknown provider_mode `{other}` (expected live|record|replay)"
            )),
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::Live => "live",
            Self::Record => "record",
            Self::Replay => "replay",
        }
    }
}

/// Schema version of the on-disk cassette format.
const CASSETTE_SCHEMA_VERSION: u32 = 1;

/// One task's recorded model conversation.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct Cassette {
    schema_version: u32,
    task_id: String,
    /// The model that produced these responses. Recorded so a replayed report
    /// can name the model it is standing in for instead of implying one.
    model: String,
    /// `governed` or `raw` — the value of the `LEGION_AI_GOVERNORS` seam when
    /// the tape was cut. A cassette recorded under one arm replayed under the
    /// other measures neither.
    arm: String,
    exchanges: Vec<CassetteExchange>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CassetteExchange {
    /// Fingerprint of the request that produced `response`, with the task's
    /// checkout path normalized out (the checkout is a fresh temp directory
    /// every run, so its name would otherwise make every request unique).
    ///
    /// Replay is ordered, not keyed: the fingerprint exists so that a change
    /// to what the agent *asks* is counted and reported rather than silently
    /// answered by a tape recorded for a different question.
    request_fingerprint: String,
    response: Value,
}

/// Replay bookkeeping for one task.
#[derive(Debug, Default)]
struct TapeState {
    exchanges: Vec<CassetteExchange>,
    cursor: usize,
    drift: u32,
}

/// Replace every 8-4-4-4-12 hex token with `<UUID>`.
///
/// `edit-as-proposal` answers with a freshly generated proposal id, and that
/// id travels back to the model in the next request. Without masking it, every
/// request after the first edit differs from the recorded one on a value that
/// carries no meaning for the model, and `cassette_drift` — the signal for
/// "the loop no longer asks what the tape answers" — would be permanently
/// non-zero and therefore useless.
fn mask_uuids(text: &str) -> String {
    // `uuid` is already a workspace dependency of this crate, and
    // `try_parse_ascii` accepts exactly the 8-4-4-4-12 hex form this needs. A
    // hand-written scanner would be a second implementation of a parser that is
    // already here and already tested.
    fn looks_like_uuid(bytes: &[u8]) -> bool {
        bytes.len() == 36 && uuid::Uuid::try_parse_ascii(bytes).is_ok()
    }

    let bytes = text.as_bytes();
    if bytes.len() < 36 {
        return text.to_string();
    }
    let mut out = String::with_capacity(text.len());
    let mut index = 0;
    while index < bytes.len() {
        // A UUID is 36 ASCII bytes, so a match can neither start nor end
        // inside a multi-byte character and `index` stays on a char boundary.
        if index + 36 <= bytes.len() && looks_like_uuid(&bytes[index..index + 36]) {
            out.push_str("<UUID>");
            index += 36;
            continue;
        }
        let ch = text[index..]
            .chars()
            .next()
            .expect("index is always a char boundary");
        out.push(ch);
        index += ch.len_utf8();
    }
    out
}

/// FNV-1a over the canonical JSON of a request payload.
///
/// Not a security hash: it exists to notice that two request payloads differ,
/// and a dependency-free 64-bit hash is enough for that.
fn fingerprint_request(payload: &Value, checkout: Option<&Path>) -> String {
    let mut text = serde_json::to_string(payload).unwrap_or_default();
    if let Some(checkout) = checkout {
        let raw = checkout.to_string_lossy().into_owned();
        // Two spellings, not three. `serde_json::to_string` escapes every
        // backslash, so a raw Windows path with single separators can never
        // appear in `text` — that arm was normalizing a form the haystack
        // cannot contain. The escaped form covers the JSON encoding; the
        // forward-slash form covers tool results that embed POSIX-style paths.
        for form in [raw.replace('\\', "\\\\"), raw.replace('\\', "/")] {
            if !form.is_empty() {
                text = text.replace(&form, "<CHECKOUT>");
            }
        }
    }
    let text = mask_uuids(&text);
    // Diagnosing cassette drift means comparing the exact bytes that were
    // fingerprinted; reconstructing them from a hash is not possible, and
    // storing every request on the tape would double its size for a case that
    // comes up only when drift is already non-zero.
    if let Ok(dir) = std::env::var("LEGION_BENCH_DUMP_REQUESTS")
        && !dir.trim().is_empty()
    {
        use std::sync::atomic::{AtomicU64, Ordering};
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let seq = NEXT.fetch_add(1, Ordering::Relaxed);
        let dir = PathBuf::from(dir);
        let _ = fs::create_dir_all(&dir);
        let _ = fs::write(dir.join(format!("request-{seq:03}.json")), &text);
    }
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    for byte in text.bytes() {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x100_0000_01b3);
    }
    format!("req-v1:{hash:016x}")
}

/// Wraps the reqwest transport so the runner can observe token usage and
/// repeated (name, arguments) tool calls without changing provider or loop
/// code, and so a run can be recorded to — or served from — a cassette. All
/// observation happens on the raw response JSON.
#[derive(Clone)]
struct MeteringTransport {
    inner: ReqwestProviderHttpTransport,
    state: Arc<Mutex<MeterState>>,
    mode: ProviderMode,
    tape: Arc<Mutex<TapeState>>,
    /// Task checkout, normalized out of request fingerprints.
    checkout: Option<PathBuf>,
}

impl MeteringTransport {
    fn with_mode(
        mode: ProviderMode,
        exchanges: Vec<CassetteExchange>,
        checkout: Option<PathBuf>,
    ) -> Self {
        Self {
            inner: ReqwestProviderHttpTransport,
            state: Arc::new(Mutex::new(MeterState::default())),
            mode,
            tape: Arc::new(Mutex::new(TapeState {
                exchanges,
                cursor: 0,
                drift: 0,
            })),
            checkout,
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
        let fingerprint = fingerprint_request(&payload, self.checkout.as_deref());
        let response = match self.mode {
            ProviderMode::Replay => self.replay(&fingerprint)?,
            ProviderMode::Live | ProviderMode::Record => {
                let response = self.inner.post_json(endpoint, bearer_token, payload)?;
                if self.mode == ProviderMode::Record
                    && let Ok(mut tape) = self.tape.lock()
                {
                    tape.exchanges.push(CassetteExchange {
                        request_fingerprint: fingerprint,
                        response: response.clone(),
                    });
                }
                response
            }
        };
        if let Ok(mut state) = self.state.lock() {
            Self::record(&mut state, &response);
        }
        Ok(response)
    }
}

impl MeteringTransport {
    /// Serve the next recorded response.
    ///
    /// Running past the end of the tape is an error rather than a fabricated
    /// "the model stopped": a replayed run that quietly ends early would score
    /// as a real, worse result and the report would not say why.
    fn replay(&self, fingerprint: &str) -> Result<Value, ProviderError> {
        let mut tape = self.tape.lock().map_err(|_| ProviderError::RequestFailed {
            provider: "legion-bench-live".to_string(),
            message: "cassette tape lock poisoned".to_string(),
        })?;
        let cursor = tape.cursor;
        let Some(exchange) = tape.exchanges.get(cursor).cloned() else {
            let total = tape.exchanges.len();
            return Err(ProviderError::RequestFailed {
                provider: "legion-bench-live".to_string(),
                message: format!(
                    "cassette exhausted after {total} exchange(s): the agent loop asked for \
                     response {} but the tape has none. Re-record with \
                     `cargo run -p xtask -- legion-bench --mode record`.",
                    cursor + 1
                ),
            });
        };
        tape.cursor += 1;
        if exchange.request_fingerprint != fingerprint {
            tape.drift += 1;
        }
        Ok(exchange.response)
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

/// Copy `src` into `dst`, rewriting CRLF to LF in every UTF-8 text file.
///
/// The checkout's bytes are what the model reads, what its `old_str` anchors
/// have to match, and what the recorded request fingerprints were computed
/// over. Git hands Windows a CRLF working copy and Linux an LF one from the
/// same commit, so without this the same cassette would replay against two
/// different files and a recorded baseline could only ever be valid on the
/// platform that cut it.
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
            let bytes =
                fs::read(&src_path).map_err(|e| format!("read {}: {e}", src_path.display()))?;
            match String::from_utf8(bytes) {
                Ok(text) => fs::write(&dst_path, text.replace("\r\n", "\n")),
                Err(err) => fs::write(&dst_path, err.into_bytes()),
            }
            .map_err(|e| format!("write {}: {e}", dst_path.display()))?;
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
    // The checkout is already LF-normalized; leaving the developer's global
    // `core.autocrlf` in force would let git rewrite it back per platform and
    // undo the normalization that makes a cassette portable.
    git_cmd(&checkout, &["config", "core.autocrlf", "false"])?;
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

/// List files changed vs the baseline commit (staged so new files count too).
///
/// Returns the paths rather than a bare count: `diff_files` gates task
/// success, so when it disagrees with the proposals a run made, the names are
/// what tell you whether the model edited something or the harness dirtied the
/// checkout by itself.
fn changed_files(checkout: &Path) -> Result<Vec<String>, String> {
    git_cmd(checkout, &["add", "-A"])?;
    let diff = git_cmd(checkout, &["diff", "--cached", "--name-only"])?;
    Ok(diff
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .filter(|line| !is_harness_artifact(line))
        .map(str::to_string)
        .collect())
}

/// Whether a changed path is Legion's own runtime state rather than the
/// model's work.
///
/// Starting a delegated task writes `target/delegated-tasks/<id>.lock` into
/// the workspace. Rust fixtures hide it behind `/target` in `.gitignore`;
/// Python and JavaScript fixtures do not, because `target/` is a Rust
/// convention — so the harness was counting its own lock file as a model edit
/// on every non-Rust task. That inflated `diff_files`, which gates task
/// success and feeds the `max_diff_files` budget.
///
/// Filtering here rather than adding `target/` to each fixture keeps the
/// metric honest by definition: it measures what the model changed, not what
/// running the benchmark changed.
fn is_harness_artifact(path: &str) -> bool {
    let normalized = path.replace('\\', "/");
    normalized.starts_with("target/delegated-tasks/")
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

/// Drive one proposal through the real lifecycle (register → validate → preview
/// → apply), mirroring GP-3 s8.
fn apply_proposal_lifecycle(
    app: &mut AppComposition,
    proposal: WorkspaceProposal,
) -> Result<(), String> {
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

/// Apply one CreateFile proposal via the real proposal lifecycle pipeline.
fn apply_new_file_via_pipeline(
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

    apply_proposal_lifecycle(app, proposal)
}

/// Replace one existing file through WorkspaceEdit authority, then persist the
/// dirty editor buffer through the normal save proposal workflow.
fn apply_existing_file_via_pipeline(
    app: &mut AppComposition,
    checkout: &Path,
    absolute_target: &Path,
    content: &str,
    proposal_id: u64,
) -> Result<(), String> {
    let opened_workspace = app
        .open_workspace(
            checkout,
            WorkspaceTrustState::Trusted,
            PrincipalId(PRINCIPAL.to_string()),
        )
        .map_err(|e| format!("workspace generation refresh failed: {e:?}"))?;
    app.open_file(absolute_target.to_string_lossy())
        .map_err(|e| format!("open existing proposal target failed: {e:?}"))?;

    let opened_file = app
        .workspace()
        .open_existing_file_text(
            opened_workspace.workspace_id,
            absolute_target.to_string_lossy(),
        )
        .map_err(|e| format!("read existing proposal target failed: {e:?}"))?;
    let buffer_id = app
        .active_buffer_id()
        .ok_or_else(|| "existing proposal target has no active buffer".to_string())?;
    let buffer_version = app
        .editor()
        .buffer_version(buffer_id)
        .map_err(|e| format!("read active buffer version failed: {e:?}"))?;
    let snapshot_id = app
        .editor()
        .current_snapshot(buffer_id)
        .map_err(|e| format!("read active buffer snapshot failed: {e:?}"))?
        .snapshot_id;
    let preconditions = ProposalVersionPreconditions {
        file_version: Some(opened_file.file_content_version),
        buffer_version: Some(buffer_version),
        snapshot_id: Some(snapshot_id),
        generation: Some(opened_file.workspace_generation),
        file_content_version: Some(opened_file.file_content_version),
        workspace_generation: Some(opened_file.workspace_generation),
        expected_fingerprint: Some(opened_file.fingerprint.clone()),
        expected_file_length: opened_file.file_length,
        expected_modified_at: opened_file.modified_at,
    };
    let replacement_range = ByteRange::new(0, opened_file.text.len() as u64);
    let capability = CapabilityId("fs.write".to_string());
    let payload = WorkspaceEditProposalPayload {
        workspace_id: opened_workspace.workspace_id,
        edit_id: uuid::Uuid::now_v7(),
        title: "Legion-Bench accepted replacement".to_string(),
        source: WorkspaceEditSourceKind::AiAssisted,
        target_coverage: ProposalTargetCoverage {
            coverage_kind: ProposalTargetCoverageKind::Complete,
            targets: vec![ProposalAffectedTarget {
                target_id: format!("bench:file:{}", opened_file.identity.file_id.0),
                kind: ProposalTargetKind::OpenBuffer,
                workspace_id: Some(opened_workspace.workspace_id),
                file_id: Some(opened_file.identity.file_id),
                buffer_id: Some(buffer_id),
                path: Some(opened_file.identity.canonical_path.clone()),
                terminal_session_id: None,
                plugin_id: None,
                remote_authority: None,
                collaboration_session_id: None,
                byte_ranges: vec![replacement_range],
                redaction_hints: vec![RedactionHint::MetadataOnly],
            }],
            omitted_target_count: 0,
            redaction_hints: vec![RedactionHint::MetadataOnly],
        },
        file_edits: vec![WorkspaceTextEdit {
            file: opened_file.identity,
            buffer_id: Some(buffer_id),
            edits: EditBatch {
                edits: vec![TextEdit {
                    range: TextRange::byte(replacement_range.start, replacement_range.end),
                    replacement: content.to_string(),
                }],
            },
            preconditions: preconditions.clone(),
        }],
        file_operations: Vec::new(),
        required_capability: capability.clone(),
        diagnostics: Vec::new(),
        schema_version: 1,
    };
    let proposal = WorkspaceProposal {
        proposal_id: ProposalId(proposal_id),
        principal: PrincipalId(PRINCIPAL.to_string()),
        capability,
        correlation_id: CorrelationId(proposal_id),
        payload: ProposalPayload::WorkspaceEdit(payload),
        preconditions,
        preview: PreviewSummary {
            summary: "legion-bench live existing-file proposal apply".to_string(),
            details: Vec::new(),
        },
        expires_at: None,
        created_at: TimestampMillis(1),
    };

    apply_proposal_lifecycle(app, proposal)?;
    match app
        .save_active_buffer()
        .map_err(|e| format!("proposal-mediated save failed: {e:?}"))?
    {
        AppSaveOutcome::Saved(_) => Ok(()),
        other => Err(format!("proposal-mediated save returned {other:?}")),
    }
}

/// Apply the loop's proposals to the checkout. Returns (applied, notes).
///
/// The delegated proposal generator emits path-based CreateFile payloads with
/// full replacement content (checkout-relative, forward slashes). New files use
/// CreateFile proposals. Existing files are translated to full-replacement
/// WorkspaceEdit proposals and persisted through the save proposal workflow.
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
            match apply_existing_file_via_pipeline(
                app,
                checkout,
                &absolute,
                &content,
                900_000 + index as u64,
            ) {
                Ok(()) => {
                    applied += 1;
                    notes.push(format!(
                        "proposal[{index}] {relative}: applied via workspace-edit and save proposal pipelines"
                    ));
                }
                Err(err) => {
                    notes.push(format!(
                        "proposal[{index}] {relative}: workspace-edit pipeline failed: {err}"
                    ));
                }
            }
        } else {
            match apply_new_file_via_pipeline(
                app,
                checkout,
                &absolute,
                &content,
                900_000 + index as u64,
            ) {
                Ok(()) => {
                    applied += 1;
                    notes.push(format!(
                        "proposal[{index}] {relative}: applied via proposal pipeline"
                    ));
                }
                Err(err) => notes.push(format!(
                    "proposal[{index}] {relative}: create-file pipeline failed: {err}"
                )),
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

// ─── Cassette I/O ────────────────────────────────────────────────────────────

/// Which A/B arm the process is running under.
///
/// `LEGION_AI_GOVERNORS=off` is the tested seam that disables every
/// SmallCode-derived governor; anything else is the governed arm. A cassette
/// carries the arm it was cut under because replaying a governed tape while
/// the loop runs ungoverned measures neither arm.
fn current_arm() -> String {
    match std::env::var("LEGION_AI_GOVERNORS").as_deref() {
        Ok("off") => "raw".to_string(),
        _ => "governed".to_string(),
    }
}

fn load_cassette(path: &Path) -> Result<Cassette, String> {
    let text = fs::read_to_string(path).map_err(|err| {
        format!(
            "unable to read cassette `{}`: {err} (record it with \
             `cargo run -p xtask -- legion-bench --mode record`)",
            path.display()
        )
    })?;
    let cassette: Cassette = serde_json::from_str(&text)
        .map_err(|err| format!("unable to parse cassette `{}`: {err}", path.display()))?;
    if cassette.schema_version != CASSETTE_SCHEMA_VERSION {
        return Err(format!(
            "cassette `{}` has schema_version {} (expected {CASSETTE_SCHEMA_VERSION})",
            path.display(),
            cassette.schema_version
        ));
    }
    if cassette.exchanges.is_empty() {
        return Err(format!(
            "cassette `{}` records no model exchanges; an empty tape would replay as a model \
             that said nothing and score as a real failure",
            path.display()
        ));
    }
    let arm = current_arm();
    if cassette.arm != arm {
        return Err(format!(
            "cassette `{}` was recorded under the `{}` arm but this process runs `{arm}` \
             (LEGION_AI_GOVERNORS); replaying across arms measures neither",
            path.display(),
            cassette.arm
        ));
    }
    Ok(cassette)
}

fn write_cassette(
    path: &Path,
    task_id: &str,
    model: &str,
    exchanges: &[CassetteExchange],
) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|err| {
            format!(
                "unable to create cassette dir `{}`: {err}",
                parent.display()
            )
        })?;
    }
    let cassette = Cassette {
        schema_version: CASSETTE_SCHEMA_VERSION,
        task_id: task_id.to_string(),
        model: model.to_string(),
        arm: current_arm(),
        exchanges: exchanges.to_vec(),
    };
    let text = serde_json::to_string_pretty(&cassette)
        .map_err(|err| format!("unable to serialize cassette: {err}"))?;
    fs::write(path, format!("{text}\n"))
        .map_err(|err| format!("unable to write cassette `{}`: {err}", path.display()))
}

// ─── Per-task execution ──────────────────────────────────────────────────────

fn run_one_task(
    task: &LiveRunTaskInput,
    endpoint: &str,
    model: &str,
    api_key: &str,
    mode: ProviderMode,
    cassette_dir: Option<&Path>,
) -> LiveRunTaskResult {
    let started = Instant::now();
    let mut result = LiveRunTaskResult::new(&task.id);
    let mut notes: Vec<String> = Vec::new();

    // Load the tape before touching the filesystem: a replay with no cassette
    // must fail loudly, not run a task with an empty conversation and report
    // the resulting zero as a measurement.
    let cassette_path = cassette_dir.map(|dir| dir.join(format!("{}.json", task.id)));
    let recorded = match (mode, cassette_path.as_deref()) {
        (ProviderMode::Replay, Some(path)) => match load_cassette(path) {
            Ok(cassette) => {
                notes.push(format!(
                    "cassette={} model={} arm={}",
                    path.display(),
                    cassette.model,
                    cassette.arm
                ));
                result.cassette_model = cassette.model;
                result.cassette_arm = cassette.arm;
                cassette.exchanges
            }
            Err(err) => {
                result.error = Some(err);
                result.wall_ms = started.elapsed().as_millis() as u64;
                return result;
            }
        },
        (ProviderMode::Replay, None) => {
            result.error = Some("replay mode requires a cassette directory".to_string());
            result.wall_ms = started.elapsed().as_millis() as u64;
            return result;
        }
        _ => Vec::new(),
    };

    let fixture_dir = PathBuf::from(&task.fixture_dir);
    let checkout = match prepare_checkout(&fixture_dir, &task.id) {
        Ok(checkout) => checkout,
        Err(err) => {
            result.error = Some(format!("checkout preparation failed: {err}"));
            result.wall_ms = started.elapsed().as_millis() as u64;
            return result;
        }
    };

    // Measure the fixture before the model touches it, so a task that already
    // passes cannot be reported as one the model solved.
    result.tests_passed_at_rest = matches!(
        run_verification(
            &task.verification_command,
            &checkout,
            Duration::from_secs(task.timeout_secs),
        ),
        Ok(Some(exit)) if exit == task.expected_exit
    );

    // Built outside the closure so the tape survives an early `?`: a run that
    // dies because the cassette ran out must still report how far it got.
    let meter = MeteringTransport::with_mode(mode, recorded, Some(checkout.clone()));

    // The wire model id goes into the request payload, so replaying under a
    // different name would make every request differ from the recorded one
    // and report the whole run as drift. The tape names its own model; that
    // is the one a replay must use.
    let model = if result.cassette_model.is_empty() {
        model.to_string()
    } else {
        result.cassette_model.clone()
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

        let provider = ModelOverrideProvider {
            inner: OpenAiCompatibleProvider::with_transport(
                "legion-bench-live",
                endpoint.to_string(),
                Some(api_key.to_string()),
                meter.clone(),
            ),
            model: model.clone(),
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

        // Apply proposals for every outcome that carries them, here, while
        // `app` still exists. A run the idle governor stopped is scored on the
        // same evidence as a completed one, and that parity is only real if
        // its edits actually reach the checkout — relabelling the outcome
        // later cannot apply anything, because the composition is gone by
        // then.
        let proposals = match &outcome {
            AppDelegatedTaskOutcome::Completed { proposals, .. }
            | AppDelegatedTaskOutcome::StoppedNoProgress { proposals, .. } => Some(proposals),
            _ => None,
        };
        if let Some(proposals) = proposals {
            result.proposals_total = proposals.len() as u32;
            let (applied, mut apply_notes) = apply_proposals(&mut app, &checkout, proposals);
            result.proposals_applied = applied;
            notes.append(&mut apply_notes);
        }
        Ok(outcome)
    })();

    // Harvest meter and tape observations. Done after the closure, not inside
    // it, so a run that failed on an exhausted cassette still reports the
    // exchanges it consumed.
    if let Ok(state) = meter.state.lock() {
        result.duplicate_tool_calls = state.duplicate_tool_calls;
        if state.usage_seen {
            result.context_tokens = Some(state.prompt_tokens);
            result.generation_tokens = Some(state.completion_tokens);
        }
        notes.push(format!("model_http_requests={}", state.requests));
    }
    if let Ok(tape) = meter.tape.lock() {
        result.cassette_drift = tape.drift;
        result.cassette_exchanges = match mode {
            ProviderMode::Replay => tape.cursor as u32,
            _ => tape.exchanges.len() as u32,
        };
        // A failed write is reported here rather than folded into
        // `result.error`, which the outcome arms below overwrite. It cannot go
        // unnoticed: the next `--write-baseline` hashes the cassette set and
        // fails on the missing file.
        if mode == ProviderMode::Record
            && let Some(path) = cassette_path.as_deref()
            && let Err(err) = write_cassette(path, &task.id, &model, &tape.exchanges)
        {
            eprintln!("legion_bench_live: {err}");
            notes.push(format!("cassette_write_failed: {err}"));
        }
        notes.push(format!(
            "provider_mode={} cassette_exchanges={} cassette_drift={}",
            mode.as_str(),
            result.cassette_exchanges,
            result.cassette_drift
        ));
    }

    // Records every audit-derived field at once. Split across the outcome arms
    // it was three copies of the same five lines, which is three places to
    // forget the next time the report grows a field.
    let record_audit = |steps: &[legion_protocol::DelegatedTaskLoopStepRecord],
                        result: &mut LiveRunTaskResult,
                        notes: &mut Vec<String>| {
        result.turns = steps
            .iter()
            .filter(|s| s.kind == DelegatedTaskLoopStepKind::ModelResponse)
            .count() as u32;
        result.tool_calls = steps
            .iter()
            .filter(|s| s.kind == DelegatedTaskLoopStepKind::ToolCallRequest)
            .count() as u32;
        let rejections: Vec<String> = steps
            .iter()
            .filter(|s| s.kind == DelegatedTaskLoopStepKind::ToolCallRejected)
            .map(|s| {
                format!(
                    "{}:{}",
                    s.tool_name.as_deref().unwrap_or("?"),
                    s.reason.as_deref().unwrap_or("?")
                )
            })
            .collect();
        result.retries = rejections.len() as u32;
        // Why a call was rejected is the whole diagnosis. Without it a failed
        // run reports `retries=1` and nothing else, and the only way to learn
        // more is to run the suite again by hand.
        if !rejections.is_empty() {
            notes.push(format!("rejections: {}", rejections.join("; ")));
        }
    };

    // A run the idle-turn governor stopped still produced whatever it produced,
    // and is scored on exactly the same evidence as a completed one: files
    // actually changed, proposals applied, verification exit code. Scoring it
    // as a non-outcome instead would let the governor flatter itself by
    // removing its own weakest runs from the denominator. Only the label
    // differs, so the evidence can report how often it fired.
    let mut stopped_no_progress = false;
    let outcome = match outcome {
        Ok(AppDelegatedTaskOutcome::StoppedNoProgress {
            proposals,
            audit_steps,
            reason,
        }) => {
            stopped_no_progress = true;
            notes.push(format!("stopped without progress: {reason}"));
            Ok(AppDelegatedTaskOutcome::Completed {
                final_message: reason,
                proposals,
                audit_steps,
            })
        }
        other => other,
    };

    match outcome {
        Ok(AppDelegatedTaskOutcome::Completed { audit_steps, .. }) => {
            result.outcome = if stopped_no_progress {
                "stopped_no_progress".to_string()
            } else {
                "completed".to_string()
            };
            record_audit(&audit_steps, &mut result, &mut notes);

            match changed_files(&checkout) {
                Ok(changed) => {
                    result.diff_files = changed.len() as u32;
                    if !changed.is_empty() {
                        notes.push(format!("changed files: {}", changed.join(", ")));
                    }
                }
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

            finalize_completed_task_success(&mut result, expected_files_ok);
        }
        Ok(AppDelegatedTaskOutcome::Blocked {
            reason,
            audit_steps,
        }) => {
            result.outcome = "blocked".to_string();
            record_audit(&audit_steps, &mut result, &mut notes);
            result.error = Some(format!("loop blocked: {reason}"));
        }
        Ok(AppDelegatedTaskOutcome::BudgetExhausted {
            reason,
            audit_steps,
        }) => {
            result.outcome = "budget_exhausted".to_string();
            record_audit(&audit_steps, &mut result, &mut notes);
            result.error = Some(format!("budget exhausted: {reason}"));
        }
        // Rewritten into `Completed` above, so this arm is not reached. It
        // records the run as an error rather than panicking, because a
        // benchmark that crashes on an unexpected outcome loses the whole
        // suite's data to protect one row of it.
        Ok(AppDelegatedTaskOutcome::StoppedNoProgress { reason, .. }) => {
            result.outcome = "stopped_no_progress".to_string();
            result.error = Some(reason);
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

    // `LEGION_BENCH_KEEP_CHECKOUTS=1` leaves each task's checkout on disk.
    // A score says a run failed; only the diff says why, and reproducing one
    // by hand costs a full suite run. Off by default because a kept suite is
    // 18 fixture copies.
    if std::env::var("LEGION_BENCH_KEEP_CHECKOUTS").is_ok_and(|v| v == "1") {
        eprintln!(
            "legion_bench_live: kept checkout for {} at {}",
            task.id,
            checkout.display()
        );
    } else {
        let _ = fs::remove_dir_all(&checkout);
    }
    result
}

fn finalize_completed_task_success(result: &mut LiveRunTaskResult, expected_files_ok: bool) {
    // Success requires the worktree to have actually changed.
    //
    // Two weaker criteria were tried and both let a do-nothing run pass. First
    // `expected_files_ok && applied == total`, where `0 == 0` holds and
    // `expected_files` mostly names files that already exist — so a model that
    // replied with prose scored a success. Then `applied > 0`, which a
    // whole-file proposal whose content equals the existing file still
    // satisfies: the proposal is accepted, nothing changes, `diff_files` is 0.
    //
    // `diff_files` comes from a real git diff against the task's baseline
    // commit, so it is evidence the requested change happened rather than
    // evidence the model produced output. Every corpus task asks for a change,
    // so a zero diff is never success — and each of these holes inflated the
    // baseline the governed arm is measured against, understating the work
    // being evaluated.
    result.task_success = result.diff_files > 0
        && result.proposals_applied > 0
        && expected_files_ok
        && result.proposals_applied == result.proposals_total;
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
    let provider_mode = match ProviderMode::parse(input.provider_mode.as_deref().unwrap_or("live"))
    {
        Ok(mode) => mode,
        Err(err) => {
            eprintln!("legion_bench_live: {err}");
            process::exit(2);
        }
    };
    let cassette_dir = input.cassette_dir.as_deref().map(PathBuf::from);
    if provider_mode != ProviderMode::Live && cassette_dir.is_none() {
        eprintln!(
            "legion_bench_live: provider_mode `{}` requires cassette_dir",
            provider_mode.as_str()
        );
        process::exit(2);
    }
    // Replay never opens a socket, so it must not demand a credential either:
    // requiring one would make the offline CI leg depend on a secret.
    let api_key = match std::env::var(API_KEY_ENV) {
        Ok(value) if !value.trim().is_empty() => value,
        _ if provider_mode == ProviderMode::Replay => "replay-no-network".to_string(),
        _ => {
            eprintln!("legion_bench_live: {API_KEY_ENV} is required");
            process::exit(2);
        }
    };

    eprintln!(
        "legion_bench_live: endpoint={} model={} tasks={} provider_mode={} arm={}",
        input.endpoint,
        input.model,
        input.tasks.len(),
        provider_mode.as_str(),
        current_arm(),
    );

    let mut results: Vec<LiveRunTaskResult> = Vec::new();
    for (index, task) in input.tasks.iter().enumerate() {
        eprintln!(
            "legion_bench_live: [{}/{}] running task {}",
            index + 1,
            input.tasks.len(),
            task.id
        );
        let result = run_one_task(
            task,
            &input.endpoint,
            &input.model,
            &api_key,
            provider_mode,
            cassette_dir.as_deref(),
        );
        eprintln!(
            "legion_bench_live: [{}/{}] {} outcome={} task_success={} tests_passed={} tests_passed_at_rest={} \
             turns={} tool_calls={} wall_ms={}{}",
            index + 1,
            input.tasks.len(),
            task.id,
            result.outcome,
            result.task_success,
            result.tests_passed,
            result.tests_passed_at_rest,
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
    use legion_protocol::{
        AssistedAiTrustProjectionKind, AssistedAiTrustProjectionReference, CausalityId,
        FileFingerprint, RedactionHint,
    };
    use uuid::Uuid;

    fn temp_fixture() -> PathBuf {
        // A clock alone is not a unique name: Windows' system clock is coarse
        // enough that two tests starting together can read the same instant,
        // share a directory, and then delete each other's fixture on cleanup.
        // The counter makes the name unique regardless of clock resolution.
        use std::sync::atomic::{AtomicU64, Ordering};
        static NEXT_FIXTURE: AtomicU64 = AtomicU64::new(0);
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let seq = NEXT_FIXTURE.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!("legion-bench-live-test-{nanos}-{seq}"));
        fs::create_dir_all(dir.join("src")).expect("create fixture dirs");
        fs::write(
            dir.join("Cargo.toml"),
            "[package]\nname = \"bench-test-fixture\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
        )
        .expect("write Cargo.toml");
        fs::write(dir.join("src").join("main.rs"), "fn main() {}\n").expect("write main.rs");
        dir
    }

    fn create_file_output(path: &str, content: &str) -> AssistedAiEditProposalOutput {
        let trust_reference = |reference_id: &str, kind| AssistedAiTrustProjectionReference {
            reference_id: reference_id.to_string(),
            kind,
            projection_hash: FileFingerprint {
                algorithm: "sha256".to_string(),
                value: "test-projection".to_string(),
            },
            schema_version: 1,
        };
        AssistedAiEditProposalOutput {
            output_id: "bench-output".to_string(),
            request_id: "bench-request".to_string(),
            provider_id: "provider:test".to_string(),
            proposal_id: ProposalId(7),
            principal: PrincipalId(PRINCIPAL.to_string()),
            capability: CapabilityId("fs.write".to_string()),
            correlation_id: CorrelationId(7),
            causality_id: CausalityId(Uuid::now_v7()),
            payload: ProposalPayload::CreateFile(CreateFileProposal {
                path: CanonicalPath(path.to_string()),
                initial_content: Some(content.to_string()),
            }),
            preconditions: ProposalVersionPreconditions {
                file_version: None,
                buffer_version: None,
                snapshot_id: None,
                generation: None,
                file_content_version: None,
                workspace_generation: None,
                expected_fingerprint: None,
                expected_file_length: None,
                expected_modified_at: None,
            },
            preview: PreviewSummary {
                summary: "bench proposal".to_string(),
                details: Vec::new(),
            },
            expires_at: None,
            created_at: TimestampMillis(1),
            context_manifest: trust_reference(
                "context:test",
                AssistedAiTrustProjectionKind::ContextManifest,
            ),
            approval_checklist: trust_reference(
                "approval:test",
                AssistedAiTrustProjectionKind::ProposalApprovalChecklist,
            ),
            redaction_hints: vec![RedactionHint::MetadataOnly],
            schema_version: 1,
        }
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
        let result = run_one_task(
            &task,
            "http://127.0.0.1:9/v1",
            "test-model",
            "unused-key",
            ProviderMode::Live,
            None,
        );
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

    /// A cassette is only portable if the bytes it was cut against are the
    /// bytes a replay sees. Git hands Windows CRLF and Linux LF from the same
    /// commit, so the checkout copy is where that has to be equalized.
    #[test]
    fn checkout_copies_are_lf_normalized() {
        let src = temp_fixture();
        fs::write(src.join("src").join("crlf.rs"), "a\r\nb\r\n").expect("write crlf");
        fs::write(src.join("src").join("mixed.txt"), "x\r\ny\nz\r\n").expect("write mixed");
        let dst = src.with_extension("copy");

        copy_dir_recursive(&src, &dst).expect("copy fixture");

        let copied = fs::read(dst.join("src").join("crlf.rs")).expect("read copy");
        assert_eq!(copied, b"a\nb\n");
        let mixed = fs::read(dst.join("src").join("mixed.txt")).expect("read copy");
        assert_eq!(mixed, b"x\ny\nz\n");

        let _ = fs::remove_dir_all(&src);
        let _ = fs::remove_dir_all(&dst);
    }

    /// Every task runs in a freshly named temp checkout, so the path is
    /// different on every run and would otherwise make every request unique.
    #[test]
    fn request_fingerprints_ignore_the_checkout_path() {
        let first = PathBuf::from(r"C:\Temp\legion-bench-live-t-1");
        let second = PathBuf::from("/tmp/legion-bench-live-t-2");
        let payload = |checkout: &Path| {
            serde_json::json!({
                "model": "m",
                "messages": [{
                    "role": "tool",
                    "content": format!("read {}/src/main.rs", checkout.display()),
                }],
            })
        };

        assert_eq!(
            fingerprint_request(&payload(&first), Some(&first)),
            fingerprint_request(&payload(&second), Some(&second)),
            "the checkout path must not reach the fingerprint"
        );
        // But a real difference must still register.
        let mut other = payload(&first);
        other["model"] = serde_json::Value::String("other".to_string());
        assert_ne!(
            fingerprint_request(&payload(&first), Some(&first)),
            fingerprint_request(&other, Some(&first))
        );
    }

    /// A proposal id is freshly generated per edit and travels back to the
    /// model in the next request, so it must not reach the fingerprint.
    #[test]
    fn request_fingerprints_ignore_proposal_ids() {
        let with_id = |id: &str| {
            serde_json::json!({
                "messages": [{"role": "tool", "content": format!("proposal {id} created")}],
            })
        };
        assert_eq!(
            fingerprint_request(&with_id("fe77f6a7-71a5-4dde-bb9c-954f46ef8d72"), None),
            fingerprint_request(&with_id("b9d4963e-d150-414f-aea2-4d9b7737eada"), None),
        );
        // A token that only looks UUID-ish must survive: masking too much
        // would hide real differences.
        assert_ne!(
            fingerprint_request(&with_id("fe77f6a7-71a5-4dde-bb9c-954f46ef8d7"), None),
            fingerprint_request(&with_id("b9d4963e-d150-414f-aea2-4d9b7737ead"), None),
        );
        assert_eq!(
            mask_uuids("naïve fe77f6a7-71a5-4dde-bb9c-954f46ef8d72 ✓"),
            "naïve <UUID> ✓"
        );
    }

    #[test]
    fn replay_serves_recorded_responses_in_order_then_refuses_to_invent_one() {
        let exchanges = vec![
            CassetteExchange {
                request_fingerprint: "req-v1:0000000000000001".to_string(),
                response: serde_json::json!({"choices": [{"message": {"content": "first"}}]}),
            },
            CassetteExchange {
                request_fingerprint: "req-v1:0000000000000002".to_string(),
                response: serde_json::json!({"choices": [{"message": {"content": "second"}}]}),
            },
        ];
        let transport = MeteringTransport::with_mode(ProviderMode::Replay, exchanges, None);

        let first = transport
            .post_json("unused", None, serde_json::json!({"n": 1}))
            .expect("first replay");
        assert_eq!(first["choices"][0]["message"]["content"], "first");
        let second = transport
            .post_json("unused", None, serde_json::json!({"n": 2}))
            .expect("second replay");
        assert_eq!(second["choices"][0]["message"]["content"], "second");

        let err = transport
            .post_json("unused", None, serde_json::json!({"n": 3}))
            .expect_err("a third call must not be answered");
        assert!(
            format!("{err}").contains("cassette exhausted"),
            "unexpected error: {err}"
        );

        // Both live requests differ from the recorded fingerprints, so both
        // count as drift — the metric that tells a reader the tape no longer
        // answers the conversation the loop is having.
        let drift = transport.tape.lock().expect("tape").drift;
        assert_eq!(drift, 2);
    }

    #[test]
    fn a_cassette_from_the_other_arm_is_refused() {
        let dir = std::env::temp_dir().join(format!(
            "legion-bench-cassette-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        ));
        fs::create_dir_all(&dir).expect("create dir");
        let path = dir.join("t.json");
        let cassette = Cassette {
            schema_version: CASSETTE_SCHEMA_VERSION,
            task_id: "t".to_string(),
            model: "m".to_string(),
            // The process under test runs governed unless the seam is set.
            arm: "raw".to_string(),
            exchanges: vec![CassetteExchange {
                request_fingerprint: "req-v1:0000000000000001".to_string(),
                response: serde_json::json!({}),
            }],
        };
        fs::write(
            &path,
            serde_json::to_string_pretty(&cassette).expect("serialize"),
        )
        .expect("write cassette");

        let err = load_cassette(&path).expect_err("cross-arm replay must be refused");
        assert!(err.contains("arm"), "unexpected error: {err}");

        // An empty tape is refused too: replaying it would look like a model
        // that said nothing and score as a real failure.
        let empty = Cassette {
            exchanges: Vec::new(),
            arm: current_arm(),
            ..cassette
        };
        fs::write(
            &path,
            serde_json::to_string_pretty(&empty).expect("serialize"),
        )
        .expect("write cassette");
        let err = load_cassette(&path).expect_err("empty tape must be refused");
        assert!(
            err.contains("no model exchanges"),
            "unexpected error: {err}"
        );

        let _ = fs::remove_dir_all(&dir);
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

    #[test]
    fn completed_task_success_is_independent_of_verification_result() {
        let mut result = LiveRunTaskResult::new("optional-verification");
        result.tests_passed = false;
        result.proposals_total = 1;
        result.proposals_applied = 1;
        result.diff_files = 1;

        finalize_completed_task_success(&mut result, true);

        assert!(result.task_success);
    }

    /// A run that proposed nothing did not do the task. Without this, a model
    /// that replies with prose and edits nothing scores a success on any task
    /// whose expected files already exist — inflating the baseline that the
    /// governed arm is measured against.
    #[test]
    fn a_run_that_proposed_nothing_is_never_a_success() {
        let mut result = LiveRunTaskResult::new("did-nothing");
        result.tests_passed = true;
        result.proposals_total = 0;
        result.proposals_applied = 0;

        finalize_completed_task_success(&mut result, true);

        assert!(
            !result.task_success,
            "zero proposals must not score as task success"
        );
    }

    /// The harness must not count its own runtime state as the model's work.
    /// Starting a delegated task writes a lock file under
    /// `target/delegated-tasks/`, which non-Rust fixtures do not gitignore.
    #[test]
    fn legion_runtime_artifacts_do_not_count_as_model_changes() {
        assert!(is_harness_artifact("target/delegated-tasks/task-abc.lock"));
        assert!(is_harness_artifact(
            "target\\delegated-tasks\\task-abc.lock"
        ));
        assert!(!is_harness_artifact("textkit/stats.py"));
        assert!(!is_harness_artifact("target/release/thing"));
    }

    /// An accepted proposal that changed nothing is not work done. A
    /// whole-file proposal whose content equals the existing file still
    /// increments `proposals_applied`, so without the diff check a no-op
    /// scores a pass on any task whose verification already passes at rest.
    #[test]
    fn a_proposal_that_changed_nothing_is_never_a_success() {
        let mut result = LiveRunTaskResult::new("no-op-proposal");
        result.tests_passed = true;
        result.proposals_total = 1;
        result.proposals_applied = 1;
        result.diff_files = 0;

        finalize_completed_task_success(&mut result, true);

        assert!(
            !result.task_success,
            "an applied proposal with an empty diff must not score as success"
        );

        // The same run with a real change does succeed.
        result.diff_files = 1;
        finalize_completed_task_success(&mut result, true);
        assert!(result.task_success);
    }

    #[test]
    fn existing_file_replacement_uses_workspace_edit_proposal_pipeline() {
        let fixture = temp_fixture();
        let mut app = AppComposition::new();
        let output = create_file_output("src/main.rs", "fn main() { println!(\"updated\"); }\n");

        let (applied, notes) = apply_proposals(&mut app, &fixture, &[output]);

        assert_eq!(applied, 1, "notes={notes:?}");
        assert_eq!(
            fs::read_to_string(fixture.join("src/main.rs")).expect("read replaced file"),
            "fn main() { println!(\"updated\"); }\n"
        );
        let applied_proposal = app
            .workspace_proposal_for_id(ProposalId(900_000))
            .expect("replacement proposal must be recorded in the lifecycle ledger");
        assert!(matches!(
            applied_proposal.payload,
            ProposalPayload::WorkspaceEdit(_)
        ));
        let _ = fs::remove_dir_all(&fixture);
    }
}
