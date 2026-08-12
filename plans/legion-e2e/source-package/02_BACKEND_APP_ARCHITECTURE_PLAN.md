# 02 — Legion IDE Back-End / Local Runtime Architecture Plan

Generated: 2026-06-01 16:24:53 EDT

## 0. Executive summary

Legion’s back end should be a local-first Rust runtime that separates five things very strictly:

1. Observation
   - LSP, DAP, git, tree-sitter, test output, diagnostics, index data.

2. Planning
   - task decomposition, worker assignment, dependency/conflict graph, route selection.

3. Execution
   - isolated delegated task sandboxes, local/cloud worker lanes, validation commands.

4. Proposal
   - patches, evidence, validation results, risk metadata.

5. Authority
   - approval, application, merge, workspace mutation.

The current repo already points in the right direction:

- `legion-agent` has delegated task runtime concepts.
- Phase 12 specifies git worktree/copy-based task sandboxes.
- Phase 13 specifies Legion workflow orchestration.
- `legion-ai-providers` has provider registry scaffolding but currently uses stubs/deterministic providers.
- `legion-app` owns proposal-mediated application and should keep that authority.
- `legion-ui` and `legion-desktop` remain projection-only.

The core architectural rule:

`legion-agent` / future `legion-agent` may schedule workers and generate proposals, but it must never directly mutate the main workspace.

## 1. Existing crate responsibilities and target responsibilities

### 1.1 `legion-app` / future `legion-app`

Current/target responsibility:

- Application composition root.
- Owns workspace authority.
- Applies approved proposals.
- Mediates mode transitions.
- Connects UI commands to runtime services.
- Enforces approval gates.
- Maintains high-level app state.

Must own:

- main workspace mutation.
- approval decisions.
- proposal application.
- app policy.
- user preference storage.

Must not delegate to UI:

- patch application.
- trust policy.
- workspace mutation.

### 1.2 `legion-ui` / future `legion-ui`

Target responsibility:

- Render dock system.
- Render panels.
- Render editor overlays.
- Render proposals/evidence.
- Render Legion Board.
- Render debug/git/test views.

Must not:

- call AI providers.
- apply patches directly.
- run shell commands directly.
- mutate workspace state directly.

### 1.3 `legion-desktop` / future `legion-desktop`

Target responsibility:

- Native shell and platform integration.
- Desktop bridge.
- Windowing.
- Projection of app state.

Must not:

- own authority.
- own AI provider calls.
- own agent orchestration.

### 1.4 `legion-protocol` / future `legion-protocol`

Target responsibility:

- Typed DTOs.
- LSP client DTOs and capabilities.
- DAP client DTOs and capabilities.
- Agent protocol DTOs.
- Cloud protocol DTOs.
- Proposal/evidence DTOs.

Add:

- `LegionWorkflowId`.
- `LegionTaskId`.
- `LegionWorkerId`.
- `WorkerAssignment`.
- `TaskPacket`.
- `WorkerResult`.
- `ProposalEnvelope`.
- `EvidenceRecord`.
- `RiskFlag`.
- `ValidationRun`.
- `ProviderRouteRequest`.
- `CloudLaneRequest`.

### 1.5 `legion-agent` / future `legion-agent`

Target responsibility:

- Delegated task sandbox orchestration.
- Legion workflow scheduling.
- Worker lifecycle metadata.
- Task graph.
- Dependency graph.
- Conflict graph.
- Provider route requests.
- Proposal generation.
- Validation coordination.

Must not:

- depend on `legion-app`.
- depend on `legion-ui`.
- depend on `legion-desktop`.
- mutate main workspace.
- bypass approval gates.
- directly own app authority.

### 1.6 `legion-ai` / future `legion-ai`

Target responsibility:

- Prompt construction.
- Context packet construction.
- Structured output parsing.
- Inference-facing abstractions.
- RAG query logic.
- Assist/Delegate/Automate model calls through provider interface.

### 1.7 `legion-ai-providers` / future `legion-ai-providers`

Current state:

- Provider registry scaffolding.
- Stub providers such as deterministic local provider.

Target responsibility:

- OpenAI-compatible chat/completion provider.
- Local llama.cpp provider.
- Ollama provider.
- Fireworks/Kimi provider.
- OpenAI/Codex subscription route where supported by local CLI/product constraints.
- Anthropic/OpenRouter optional routes.
- Cloud Legion worker route.
- MCP client provider/context server integration.

Provider implementations should return route metadata and typed responses, never app-level mutation.

### 1.8 `legion-index` / future `legion-index`

Target responsibility:

- tree-sitter parsing.
- symbol index.
- structural search.
- ast-grep/GritQL integration.
- embeddings chunking.
- vector search.
- test discovery.
- semantic/syntactic diff support.

### 1.9 `legion-terminal` / future `legion-terminal`

Target responsibility:

- command execution.
- test runner.
- debug adapter subprocess hosting.
- validation command execution.
- terminal UI backing.
- process lifecycle.

Important:

- Agent workers should not receive unrestricted terminal access.
- Validation commands are policy-defined and controlled.
- Tool execution must be permissioned.

### 1.10 `legion-security` / future `legion-security`

Target responsibility:

- permission policy.
- cloud upload policy.
- secret scanning.
- path allowlists/denylists.
- supply-chain checks.
- high-risk file classification.
- tool invocation approval.
- risk scoring.

### 1.11 `legion-memory` / future `legion-memory`

Target responsibility:

- chat/session memory.
- accepted trace storage.
- local project context.
- retrieval metadata.
- user-approved training trace export.

Must separate:

- ephemeral worker context.
- durable project memory.
- training trace store.
- privacy-sensitive data.

### 1.12 `legion-tracker` / future `legion-tracker`

Target responsibility:

- decision feed.
- audit events.
- worker status events.
- validation events.
- proposal lifecycle events.
- cloud usage events.

## 2. Core back-end data model

### 2.1 Workflow

```rust
pub struct LegionWorkflow {
    pub id: LegionWorkflowId,
    pub objective: String,
    pub mode: LegionMode,
    pub created_at: Timestamp,
    pub status: WorkflowStatus,
    pub task_graph: TaskGraph,
    pub risk_policy: RiskPolicy,
    pub route_policy: RoutePolicy,
    pub evidence: Vec<EvidenceRecordId>,
}
```

Statuses:

- Draft.
- Planned.
- Running.
- Paused.
- Blocked.
- NeedsReview.
- Completed.
- Failed.
- Killed.

### 2.2 Task graph

```rust
pub struct LegionTask {
    pub id: LegionTaskId,
    pub workflow_id: LegionWorkflowId,
    pub title: String,
    pub role: WorkerRole,
    pub objective: String,
    pub scope: TaskScope,
    pub dependencies: Vec<LegionTaskId>,
    pub conflicts: Vec<ConflictId>,
    pub validation_plan: ValidationPlan,
    pub output_contract: OutputContract,
    pub status: TaskStatus,
    pub retry_policy: RetryPolicy,
}
```

Statuses:

- Intake.
- Planned.
- Ready.
- Assigned.
- Running.
- Validating.
- NeedsReview.
- Blocked.
- Accepted.
- Rejected.
- Killed.

### 2.3 Task packet

Task packets are the atomic unit sent to workers.

```rust
pub struct TaskPacket {
    pub task_id: LegionTaskId,
    pub role: WorkerRole,
    pub objective: String,
    pub allowed_files: Vec<PathBuf>,
    pub forbidden_files: Vec<PathBuf>,
    pub context_snippets: Vec<ContextSnippet>,
    pub full_files: Vec<ContextFile>,
    pub command_outputs: Vec<CommandOutputSnippet>,
    pub output_contract: OutputContract,
    pub validation_plan: ValidationPlan,
    pub stop_conditions: Vec<StopCondition>,
    pub policy: WorkerPolicy,
}
```

Rules:

- Workers receive task packets, not whole app authority.
- Cloud workers receive scoped packets, not implicit full repo access unless project policy allows.
- Every packet should have a hash for audit.

### 2.4 Worker assignment

```rust
pub struct WorkerAssignment {
    pub worker_id: LegionWorkerId,
    pub task_id: LegionTaskId,
    pub role: WorkerRole,
    pub model_route: ProviderRouteRequest,
    pub sandbox_id: SandboxId,
    pub lease: WorkerLease,
    pub timeout: Duration,
}
```

### 2.5 Worker result

```rust
pub enum WorkerResult {
    PatchProposal(PatchProposal),
    DocumentationProposal(DocumentationProposal),
    Analysis(AnalysisResult),
    TestPlan(TestPlanResult),
    Blocked(BlockedReason),
    Invalid(InvalidOutputReason),
}
```

### 2.6 Proposal envelope

```rust
pub struct ProposalEnvelope {
    pub proposal_id: ProposalId,
    pub workflow_id: Option<LegionWorkflowId>,
    pub task_id: Option<LegionTaskId>,
    pub worker_id: Option<LegionWorkerId>,
    pub summary: String,
    pub diff: Option<UnifiedDiff>,
    pub files_touched: Vec<PathBuf>,
    pub evidence: Vec<EvidenceRecordId>,
    pub risk_flags: Vec<RiskFlag>,
    pub validation_runs: Vec<ValidationRunId>,
    pub status: ProposalStatus,
}
```

### 2.7 Evidence record

```rust
pub struct EvidenceRecord {
    pub id: EvidenceRecordId,
    pub kind: EvidenceKind,
    pub created_at: Timestamp,
    pub source: EvidenceSource,
    pub payload_hash: String,
    pub summary: String,
    pub redacted_payload: Option<String>,
}
```

Evidence kinds:

- ModelOutput.
- PatchAppliedToSandbox.
- CommandRun.
- TestResult.
- LintResult.
- TypecheckResult.
- SecurityScan.
- ConflictCheck.
- HumanDecision.
- CloudUploadScope.

## 3. Legion worker lifecycle

### 3.1 Lifecycle states

```text
Created
  ↓
Assigned
  ↓
SandboxPrepared
  ↓
ContextBuilt
  ↓
InferenceStarted
  ↓
OutputReceived
  ↓
OutputParsed
  ↓
PatchAppliedToSandbox
  ↓
Validated
  ↓
ProposalCreated
  ↓
Terminated
```

Terminal states:

- Completed.
- Failed.
- Blocked.
- TimedOut.
- KilledByUser.
- KilledByRiskMonitor.
- InvalidOutput.
- Escalated.

### 3.2 Disposable worker semantics

Important distinction:

- Disposable worker context must be killed.
- Model weights/process may be pooled.

For performance:

- Keep model servers warm.
- Dispose task session/context.
- Delete sandbox/worktree after evidence is captured.

This preserves the “Mr. Meeseeks” product metaphor without paying model cold-start cost on every task.

### 3.3 Sandbox preparation

Preferred:

- `git worktree add target/delegated-tasks/task-{id}`.

Fallback:

- copy-based sandbox.

Sandbox must include:

- isolated workspace.
- allowed path policy.
- clean baseline commit hash.
- validation command environment.
- no credentials unless explicitly scoped.

### 3.4 Containment validation

Checks:

- canonicalize paths.
- reject path traversal.
- reject symlink escape.
- reject writes outside allowed root.
- reject hidden secret paths.
- reject binary files unless allowed.
- reject generated dependency/vendor directories unless allowed.

### 3.5 Proposal-only output

Workers may produce:

- unified diff.
- structured analysis.
- docs proposal.
- test proposal.
- blocked reason.

Workers may not:

- write to main workspace.
- apply directly to main branch.
- approve themselves.
- alter policy.
- grant tool permissions.

## 4. Provider route architecture

### 4.1 Provider interface

```rust
pub trait AiProvider {
    fn id(&self) -> ProviderId;
    fn capabilities(&self) -> ProviderCapabilities;
    async fn complete_chat(&self, req: ChatCompletionRequest) -> Result<ChatCompletionResponse>;
    async fn complete_structured<T>(&self, req: StructuredRequest<T>) -> Result<T>;
}
```

### 4.2 Provider types

Local:

- llama.cpp OpenAI-compatible server.
- Ollama.
- LM Studio optional.
- local deterministic provider for tests.

Remote inference:

- Fireworks/Kimi.
- OpenAI API where available.
- Anthropic.
- OpenRouter.

Subscription/CLI routes:

- OpenAI Codex CLI/subscription route can be planner/architect in local dev if integration is CLI-based.
- Treat CLI route as a separate provider adapter with clear limitations.
- Do not assume ChatGPT subscription equals API access.

Cloud Legion route:

- cloud worker lane endpoint.
- accepts task packet.
- returns proposal/evidence.

### 4.3 Model route request

```rust
pub struct ProviderRouteRequest {
    pub task_kind: TaskKind,
    pub worker_role: WorkerRole,
    pub preferred_provider: Option<ProviderId>,
    pub preferred_model: Option<ModelId>,
    pub locality: LocalityPreference,
    pub max_cost: Option<CostBudget>,
    pub max_latency: Option<Duration>,
    pub privacy_policy: PrivacyPolicy,
    pub required_capabilities: Vec<ModelCapability>,
}
```

Locality:

- LocalOnly.
- PreferLocal.
- PreferCloud.
- CloudOnly.
- AskUser.

### 4.4 Routing policy

Rules:

- docs/summaries → small local model.
- lint/simple patch → local 1.5B/3B.
- compiler error small scope → local 3B.
- multi-file reasoning → Kimi/Codex/strong model.
- security/auth/build system → human review + strong model.
- cloud forbidden paths → LocalOnly.
- low hardware user → cloud lane if allowed.

## 5. Validation architecture

### 5.1 Validation plan

```rust
pub struct ValidationPlan {
    pub commands: Vec<ValidationCommand>,
    pub required_checks: Vec<ValidationCheck>,
    pub timeout: Duration,
    pub allowed_failure_modes: Vec<FailureMode>,
}
```

Validation commands:

- `cargo fmt --check`.
- `cargo check -p crate`.
- `cargo test -p crate`.
- `cargo clippy -p crate`.
- `pnpm typecheck`.
- `pytest`.
- custom project commands.

### 5.2 Validation rules

A proposal can be marked validation-passed only if:

- patch applies cleanly to sandbox.
- allowed files only.
- validation commands pass.
- risk scan passes or risk flags are acknowledged.
- conflict check passes.
- no forbidden paths touched.
- no secrets introduced.

### 5.3 Validation lane separation

Model workers and validation runners should be separate concepts.

Reason:

- validation often does not need GPU.
- validation can be deterministic.
- validation can run on CPU-only cloud lanes.
- validation should not trust model output.

## 6. Conflict detection architecture

### 6.1 File-level conflict detection

Initial rule:

- If parallel tasks touch same file, mark conflict.

### 6.2 Hunk-level conflict detection

Next rule:

- If hunks do not overlap, allow integration candidate.
- If hunks overlap, require reviewer/integrator.

### 6.3 Semantic conflict detection

Later rule:

- Use AST/symbol index to detect same function/type/module edits.
- Detect API change impact.
- Detect protocol/schema changes.

### 6.4 Merge readiness

A workflow is merge-ready if:

- all accepted proposals have passed validation.
- no unresolved conflicts.
- no unacknowledged high-risk flags.
- main workspace is clean or user explicitly approves merge over dirty state.
- integration worktree validation passes.

## 7. LSP/DAP/index runtime

### 7.1 LSP

Implement/complete:

- document symbols.
- workspace symbols.
- diagnostics.
- code actions.
- inlay hints.
- code lens.
- find references.
- go to definition/declaration/type definition.
- call hierarchy.
- type hierarchy.

Capability gate:

- LSP 3.17 finalized.
- LSP 3.18 emerging features must be capability-gated.

### 7.2 DAP

Implement:

- DAP client transport.
- request/response sequence matching.
- event handling.
- adapter subprocess lifecycle through terminal/process host.
- breakpoints.
- stack frames.
- scopes/variables.
- watches.
- debug console.
- stepping.

Adapters:

- Rust: lldb-dap/CodeLLDB.
- JS/TS: js-debug adapter.
- Python: debugpy.

### 7.3 Index

Index layers:

- file inventory.
- symbol index.
- tree-sitter syntax trees.
- structural search index.
- test discovery index.
- embeddings chunks.
- vector search.

Incremental rules:

- hash file contents.
- reparse changed files.
- update symbols.
- invalidate embeddings for changed chunks.

## 8. Cloud integration back-end API

Local Legion should communicate with Legion Cloud through typed APIs:

- create cloud workflow.
- upload scoped task packet.
- upload selected file context.
- request worker lane.
- request validation lane.
- poll/stream status.
- download proposal/evidence.
- cancel/kill task.

Cloud API must support:

- idempotency keys.
- request hashes.
- resumable uploads.
- redaction metadata.
- audit records.
- cost estimates.

## 9. Security architecture

### 9.1 Policy layers

1. Build-time policy
   - Offline build excludes AI crates.

2. Mode policy
   - Manual excludes AI panels/providers.

3. Project policy
   - cloud allowed/forbidden paths.
   - validation commands.
   - model route preferences.

4. Task policy
   - allowed files.
   - allowed tools.
   - timeout.
   - cost cap.

5. Runtime policy
   - risk monitor.
   - kill switch.
   - permission queue.

### 9.2 Risk flags

High-risk changes:

- auth.
- payment.
- secrets.
- deployment.
- CI/CD.
- build scripts.
- dependency changes.
- generated binary files.
- broad deletion.
- permission expansion.

### 9.3 Secret scanning

Before cloud upload:

- scan selected files/snippets.
- redact common secrets.
- block `.env`, credential files, SSH keys, API tokens.
- show upload scope to user.

Before proposal approval:

- scan diff for newly introduced secrets.

## 10. Back-end implementation sequence

### BE-1: Protocol DTO foundation

Tasks:

1. Add Legion workflow/task/worker IDs.
2. Add task graph DTOs.
3. Add task packet DTOs.
4. Add proposal/evidence DTOs.
5. Add validation DTOs.
6. Add risk DTOs.
7. Add provider route DTOs.

Exit criteria:

- DTOs compile.
- JSON serialization tests pass.
- UI can render fake workflow from DTOs.

### BE-2: Delegated sandbox hardening

Tasks:

1. Verify git worktree path.
2. Verify copy fallback.
3. Add containment tests.
4. Add allowed/forbidden path enforcement.
5. Add sandbox cleanup.
6. Add evidence capture.

Exit criteria:

- Worker cannot write outside sandbox.
- Proposal generated from sandbox diff only.

### BE-3: Provider adapters

Tasks:

1. OpenAI-compatible HTTP adapter.
2. llama.cpp adapter.
3. Ollama adapter.
4. Fireworks/Kimi adapter.
5. deterministic test provider.
6. structured output parser.

Exit criteria:

- Local model endpoint can receive task packet and return structured result.

### BE-4: Validation runner

Tasks:

1. Apply patch to sandbox.
2. Run validation commands.
3. Capture stdout/stderr.
4. Timeout/kill runaway command.
5. Store validation evidence.
6. Return pass/fail.

Exit criteria:

- Invalid patches fail closed.
- Passing validations create evidence.

### BE-5: Legion workflow coordinator

Tasks:

1. Create workflow.
2. Create task graph.
3. Assign tasks.
4. Schedule ready tasks.
5. Track dependencies.
6. Detect file conflicts.
7. Retry failed tasks.
8. Escalate blocked tasks.
9. Create proposals.
10. Cleanup workers.

Exit criteria:

- Multi-task workflow runs in isolated sandboxes and produces proposal queue.

### BE-6: Cloud API bridge

Tasks:

1. Define cloud API client.
2. Implement auth.
3. Implement task packet upload.
4. Implement status streaming/polling.
5. Implement proposal download.
6. Implement cancellation.

Exit criteria:

- Local Legion can route a low-risk docs task to cloud and receive proposal.

### BE-7: Full Automate integration

Tasks:

1. Risk monitor.
2. Kill switch.
3. Decision feed events.
4. Cloud/local route policy.
5. Merge readiness.
6. Human approval bridge.

Exit criteria:

- End-to-end Automate task visible in UI, validated, proposed, reviewed, and applied only after approval.

## 11. Back-end immediate next actions

1. Keep internal crate names temporarily.
2. Add Legion-branded protocol DTOs where possible.
3. Harden `legion-agent` sandbox isolation.
4. Implement provider adapters beyond stubs.
5. Implement validation runner as separate from model worker.
6. Implement workflow coordinator behind tests.
7. Connect to front-end Legion Board only after DTOs are stable.
