# Legion IDE AI, Agent, Extension, Remote, and Team Completion Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Complete Stages 3–5 of the approved full-product vision so real Assist, Delegate, multi-agent, extension, remote, collaboration, enterprise, and consented-training workflows work through the shipped desktop application.

**Architecture:** Preserve the projection-only UI, app-owned orchestration, proposal-mediated writes, default-deny capabilities, and metadata-first observability. Add bounded service modules behind `AppComposition`; new protocol or authority boundaries receive a reviewed contract package before implementation, and every implementation package ends in externally observable workflow evidence rather than fixture-only proof.

**Tech Stack:** Rust 1.92 workspace, Tokio, eframe/egui, Wasmtime/WIT, reqwest/rustls, OS keyring, existing Legion protocol/security/storage/observability crates, native process supervision, SSH/container tooling selected by the Stage 0 matrices.

**Spec:** `docs/superpowers/specs/2026-09-04-product-completion-design.md`

**Master plan and canonical records:** `docs/superpowers/plans/2026-09-04-full-product-completion.md`. This linked plan defines implementation packages; the master plan alone defines completion-register and evidence-run schemas.

## Global Constraints

- Full existing Legion vision plus missing everyday IDE essentials remains required; no feature in this plan may be parked to manufacture completion.
- Rust, TypeScript/JavaScript, and Python are all required in AI, remote, extension, and team acceptance projects.
- Consume the ratified Stage 0 register, matrices, scenarios, evidence validator, baseline, and dependency graph (`S0-01`–`S0-06`); this plan does not choose versions or define a second status/evidence format.
- Write every run to `plans/evidence/completion/<run-id>/` with the master plan's schema, then validate it with `cargo run -p xtask -- verify-completion --candidate <candidate-code-sha>`; stage-specific collectors may gather evidence but cannot redefine eligibility.
- Candidate identity comes only from `plans/completion/candidate.json`: its `code_sha` is the commit built into the desktop/service artifacts and its artifact hashes bind those exact binaries. `EvidenceRun.candidate_sha` and every `--candidate <sha>` argument mean that pinned `code_sha`, never the repository HEAD after an evidence commit.
- Commit implementation, tests, harnesses, and driver fixtures first; build and record `candidate.json` from that code commit; then run qualification and commit canonical evidence separately. Never mix product code and evidence in one commit or rebuild after pinning without creating a new candidate.
- Files under `acceptance/scenarios/stage*/` are immutable input-driver fixtures mapped by stable scenario ID to canonical `plans/completion/scenarios.json`; they are never a second scenario register or status source.
- A hosted-model matrix may select owner-approved providers, including OpenAI-compatible endpoints. No Anthropic purchase is compulsory, but existing explicitly promised Anthropic compatibility stays in scope when owner-provided credentials are available; removing it requires an owner scope decision, and unavailable credentials remain an honest external blocker.
- Preserve `native input -> desktop/UI intent -> AppComposition -> authoritative service -> external effect -> app snapshot -> rendered feedback`.
- Keep `legion-ui` projection-only; it emits intents and never owns editor text, credentials, provider sessions, remote files, collaboration state, or extension processes.
- All workspace mutations, including AI, extension, remote, and collaboration mutations, remain proposal-mediated with complete affected-target and version preconditions.
- Default-deny capabilities, explicit egress consent, metadata-only diagnostics, non-zero correlation/causality, secret redaction, cancellation, and bounded process cleanup apply throughout.
- Deterministic providers, scripted tool loops, loopback transports, simulated adapters, and constructed projections are component/integration evidence only. They cannot close product acceptance.
- Product acceptance launches the ordinary packaged executable, uses actual keyboard/mouse/accessibility controls, uses real supported dependencies/endpoints, and verifies effects outside Legion.
- Every work package has an implementer and an independent `sol_reviewer`; architecture, security, persistence, protocol, and cross-platform changes require Sol review before dependents start.
- Any incomplete required register row or required configuration blocks its stage exit; a deadline, fixture result, or completed implementation field cannot override an unassessed, blocked, or failed acceptance field.
- Workers own only the files listed for their package, preserve concurrent changes, and never edit overlapping files in parallel. Changes to `crates/legion-app/src/lib.rs`, `crates/legion-protocol/src/lib.rs`, and desktop bridge/workflow files are serialized by the dependencies below.
- Existing standing gates remain required. Proposed `xtask` commands in this plan are explicitly marked **NEW** and must be implemented before they are invoked.

---

## File and authority map

| Boundary | Existing files to extend | Focused files proposed by this plan |
| --- | --- | --- |
| Provider/setup/runtime | `legion-ai-providers/src/lib.rs`, `legion-app/src/first_run.rs`, `local_ai_diagnosis.rs`, `product_ai_policy.rs` | `legion-ai-providers/src/managed_runtime.rs`, `legion-app/src/ai_setup.rs` |
| Context and Assist | `legion-index/src/lib.rs`, `legion-memory/src/lib.rs`, `legion-ai/src/manifest.rs`, `legion-app/src/assist_proposal.rs`, `product_ai_completion.rs` | `legion-app/src/ai_context.rs` |
| Delegate | `legion-agent/src/{agent_loop,scope,worktree,evidence}.rs`, `legion-app/src/delegate_workflow.rs`, `lib.rs` | `legion-app/src/delegate_session.rs` |
| Multi-agent/MCP/ACP | `legion-agent/src/{plan,dag,coordinator,scheduler,state}.rs`, `legion-ai-providers/src/mcp_server.rs`, `legion-app/src/acp_host.rs`, `offline_ai.rs` | `legion-protocol/src/interoperability.rs`, `legion-app/src/workflow_supervisor.rs`, `legion-ai-providers/src/mcp_client.rs` |
| Extensions | `legion-plugin/src/{lib,host,manifest,registry}.rs`, `legion-vscode-compat/src/lib.rs`, `legion-app/src/extension_management.rs` | `legion-protocol/src/extension_host.rs`, `legion-plugin/src/storage.rs`, `legion-vscode-compat/src/{node_host,web_host,tier3}.rs` |
| Remote | `legion-remote/src/{lib,transport}.rs`, `legion-remote-transport/src/lib.rs` | `legion-protocol/src/remote_dev.rs`, `legion-remote/src/{ssh,container,workspace}.rs`, `legion-remote-transport/src/agent.rs` |
| Collaboration/enterprise | `legion-collaboration/src/lib.rs`, `legion-retention/src/training.rs`, `legion-telemetry/src/lib.rs` | `legion-protocol/src/enterprise.rs`, `legion-collaboration/src/{shared_proposals,identity}.rs`, `legion-app/src/{enterprise,training_consent}.rs` |
| Native acceptance | existing desktop reachability/rendering tests | `xtask/src/*_product_acceptance.rs` plus `acceptance/scenarios/stage{3,4,5}/` (**NEW harness and scenario assets**) |

Each proposed protocol module must be exported from `crates/legion-protocol/src/lib.rs`; each proposed app module must be registered from `crates/legion-app/src/lib.rs`. Those integration edits belong to the package that creates the module and are serialized.

## Stage 3 — Complete Assist and Delegate

### S3-01: Ratify the provider, managed-runtime, context, and AI-session contract

**Dependencies:** `S0-01`–`S0-06`, `S2-06` (ratified records/matrices and completed language journeys).

**Owner model:** `sol_engineer`; independent review: `sol_reviewer`.

**Outputs:** `AI-RUNTIME-001` ADR, request/state/error schemas, credential and egress boundary, lifecycle/recovery table, migration rule for current settings.

**Files:**
- Create: `plans/adrs/ADR-0052-ai-runtime-and-session-authority.md`
- Create: `crates/legion-protocol/src/ai_runtime.rs`
- Modify: `crates/legion-protocol/src/lib.rs`
- Modify: `plans/dependency-policy.md`
- Test: `crates/legion-protocol/tests/ai_runtime_contract.rs`

**Interfaces:**
- Consumes current: `legion_ai::ModelProvider`, `legion_ai::ToolCallingProvider`, `legion_ai::ContextManifest`, `ProductAiProviderPreference`, `AssistedAiProviderRouteRequest`, and proposal lifecycle types.
- Produces proposed: `AiRuntimeSpec`, `AiRuntimeId`, `AiRuntimeLifecycle`, `AiProviderSetupRequest`, `AiProviderSetupOutcome`, `AiSessionCheckpoint`, and `AiRuntimeError`; no type grants filesystem/network authority by construction.

- [ ] **Step 1: Write contract tests first.** Define compile-time/exhaustive tests proving runtime IDs are stable, credentials are references rather than secret strings, hosted routes require explicit egress consent, lifecycle states distinguish downloading/ready/degraded/failed, and checkpoints bind provider/model/context/proposal IDs.
- [ ] **Step 2: Run `cargo test -p legion-protocol --test ai_runtime_contract`; expect compilation or assertion failure because the proposed module does not exist.**
- [ ] **Step 3: Write the ADR and the minimal serializable protocol types.** Include transition tables for setup, cancellation, interrupted download, process crash, app restart, stale context, provider unavailability, and credential revocation; include schema-version migration and rollback rules.
- [ ] **Step 4: Update dependency policy and `xtask check-deps` symbols only where the new protocol edge requires them; do not activate a runtime in this package.**
- [ ] **Step 5: Run `cargo test -p legion-protocol --test ai_runtime_contract` and `cargo run -p xtask -- check-deps`; expect all contract cases and policy validation to pass.**
- [ ] **Step 6: Have the reviewer reject any contract that permits silent hosted fallback, stores a raw credential, bypasses proposals, or lacks restart semantics; record approval in the ADR.**
- [ ] **Step 7: Commit only the contract package:** `git add plans/adrs/ADR-0052-ai-runtime-and-session-authority.md plans/dependency-policy.md crates/legion-protocol && git commit -m "design: define production AI runtime authority"`.

### S3-02: Deliver real provider setup and credential lifecycle

**Dependencies:** `S3-01`.

**Owner model:** `terra_worker`; security/keyring integration: `sol_engineer`; review: `sol_reviewer`.

**Outputs:** first-run/settings setup for the complete Stage 0 local/hosted matrix, named provider health panel, pre-invocation cost/token estimate and policy ceiling, actual usage/cost reconciliation, credential add/replace/delete, selected-route visibility, no silent fallback.

**Files:**
- Create: `crates/legion-app/src/ai_setup.rs`
- Modify: `crates/legion-app/src/lib.rs`
- Modify: `crates/legion-app/src/first_run.rs`
- Modify: `crates/legion-ai-providers/src/lib.rs`
- Modify: `crates/legion-desktop/src/view/assistant_rail.rs`
- Modify: `crates/legion-desktop/src/bridge.rs`
- Test: `crates/legion-app/tests/ai_provider_setup.rs`
- Test: `crates/legion-desktop/tests/provider_key_entry.rs`

**Interfaces:**
- Consumes current: `make_provider_registry()`, `provider_setup_rows()`, `can_activate_provider(...)`, `OpenAiResponsesProvider::from_env(...)`, OS keyring support already used by the app, and S3-01 setup contracts.
- Produces proposed: `AiSetupService::handle(&mut self, AiProviderSetupRequest) -> Result<AiProviderSetupOutcome, AiRuntimeError>`; `AppComposition::apply_ai_setup_request(...)`; projection-only `AiProviderSetupProjection` containing provider/model/status/capabilities and redacted credential state; `ProviderHealthProjection`; `AiUsageEstimate` and `AiUsageActual` bound to request/session IDs.

- [ ] **Step 1: Add failing app tests for local endpoint discovery, every required hosted provider, hosted base URL/model validation, named health/latency/capability rows, preflight token/cost estimate, over-budget denial before request, actual provider usage reconciliation, keyring replacement/deletion, revoked credentials, timeout, malformed capability response, and restart reconstruction from credential references.** Assert no secret appears in `Debug`, events, projections, support bundles, or settings files.
- [ ] **Step 2: Run `cargo test -p legion-app --test ai_provider_setup`; expect missing-service failures.**
- [ ] **Step 3: Implement `AiSetupService` around the existing provider registry and keyring pattern.** Connectivity checks must be cancellable and must report the exact selected provider/model; Auto may propose a reachable local route but may not silently select hosted egress.
- [ ] **Step 4: Add desktop controls for provider kind, endpoint, model, credential store/replace/delete, test connection, and activation, plus a named health panel and per-request estimate/ceiling/actual-usage rows.** Route each control through `AppComposition`; never retain key text in the projection after submission.
- [ ] **Step 5: Extend `provider_key_entry.rs` to type into the real fields, submit, navigate away/back, and assert only redacted state renders; add denied-egress and deleted-key recovery cases.**
- [ ] **Step 6: Run `cargo test -p legion-app --test ai_provider_setup` and `cargo test -p legion-desktop --test provider_key_entry`; expect all setup/recovery/redaction cases to pass.**
- [ ] **Step 7: Commit implementation/tests first:** `git add crates/legion-app crates/legion-ai-providers crates/legion-desktop && git commit -m "feat: complete provider setup workflow"`.
- [ ] **Step 8: Build the exact Step 7 code commit, pin it and artifact hashes in `plans/completion/candidate.json`, then launch the ordinary desktop app against one ratified local endpoint and every required hosted provider; externally verify endpoint request logs and confirm no request occurs before consent. Write canonical evidence under `plans/evidence/completion/<run-id>/` and validate it with `verify-completion --candidate <candidate-code-sha>`.**
- [ ] **Step 9: Commit only candidate/evidence records:** `git add plans/completion/candidate.json plans/evidence/completion && git commit -m "evidence: record provider setup acceptance"`.

### S3-03: Add the managed local-model runtime

**Dependencies:** `S3-01`; matrix entries from `S0-02` and evidence rules from `S0-03`/`S0-04`.

**Owner model:** `sol_engineer`; desktop setup UI may be delegated to `terra_worker`; review: `sol_reviewer`.

**Outputs:** runtime install/discovery, model catalog/download, digest verification, disk/hardware fit, bounded start/stop/health, interrupted-download resume, upgrade/uninstall, and explicit external-runtime mode.

**Files:**
- Create: `crates/legion-ai-providers/src/managed_runtime.rs`
- Modify: `crates/legion-ai-providers/src/lib.rs`
- Modify: `crates/legion-app/src/ai_setup.rs`
- Modify: `crates/legion-desktop/src/view/assistant_rail.rs`
- Test: `crates/legion-ai-providers/tests/managed_runtime.rs`
- Test: `crates/legion-app/tests/managed_ai_runtime.rs`

**Interfaces:**
- Consumes S3-01 `AiRuntimeSpec`/lifecycle/error types and S3-02 `AiSetupService`; consumes existing Ollama/llama.cpp providers after a runtime reports ready.
- Produces proposed: `ManagedRuntimeSupervisor::reconcile(&mut self, &AiRuntimeSpec, CancellationTokenId) -> Result<AiRuntimeStatus, AiRuntimeError>` and `ManagedModelArtifact { source, size_bytes, sha256, local_path }`. Runtime processes receive no workspace path.

- [ ] **Step 1: Write failing tests using a disposable HTTP server and child-process fixture for valid digest, mismatch deletion/quarantine, partial resume, low disk, unsupported hardware, cancellation, crash/backoff, port collision, and orphan reaping.**
- [ ] **Step 2: Run `cargo test -p legion-ai-providers --test managed_runtime`; expect missing supervisor failures.**
- [ ] **Step 3: Implement download-to-temporary-file, length and SHA-256 verification, atomic promotion, owner-scoped process supervision, health probes, bounded retries, and cleanup.** Never download a matrix-unknown artifact or execute before integrity verification.
- [ ] **Step 4: Connect runtime actions/status to `AiSetupService` and the setup UI; show storage/hardware estimates and require explicit download consent.**
- [ ] **Step 5: Add restart tests proving a verified model is reused, a partial is resumed safely, a corrupt artifact is never launched, and an app crash leaves no unowned serving process.**
- [ ] **Step 6: Run both managed-runtime test targets plus `cargo test -p legion-ai-providers --test provider_activation`; expect all cases to pass.**
- [ ] **Step 7: Commit implementation/tests first:** `git add crates/legion-ai-providers crates/legion-app crates/legion-desktop && git commit -m "feat: manage local model runtime"`.
- [ ] **Step 8: Build and pin that code commit in `candidate.json`; on every Stage 0 reference hardware class, install/download/start/stop/restart/uninstall through native controls and independently inspect process ownership, digest, disk reclamation, and offline reuse. Write canonical evidence and validate against the pinned code SHA.**
- [ ] **Step 9: Commit only candidate/evidence records:** `git add plans/completion/candidate.json plans/evidence/completion && git commit -m "evidence: record managed runtime acceptance"`.

### S3-04: Make context selection fresh, inspectable, bounded, and private

**Dependencies:** `S3-01`; dependable indexing and language workflows from Stage 2.

**Owner model:** `sol_engineer`; review: `sol_reviewer`.

**Outputs:** a single app-owned context assembler for Assist/Delegate; provenance, freshness, token budget, excludes, memory controls, remote boundary, user inspection before egress, and explicit provider prompt-cache behavior with cache provenance/usage.

**Files:**
- Create: `crates/legion-app/src/ai_context.rs`
- Modify: `crates/legion-app/src/lib.rs`
- Modify: `crates/legion-ai/src/manifest.rs`
- Modify: `crates/legion-ai-providers/src/lib.rs`
- Modify: `crates/legion-index/src/lib.rs`
- Modify: `crates/legion-memory/src/lib.rs`
- Modify: `crates/legion-desktop/src/view/manifest_panel.rs`
- Test: `crates/legion-ai/tests/context_manifest.rs`
- Test: `crates/legion-app/tests/ai_context_assembly.rs`
- Test: `crates/legion-desktop/tests/manifest_panel.rs`

**Interfaces:**
- Consumes current `ContextManifest`, index snapshots, memory records, open-buffer snapshots, workspace trust, route policy, and S3-01 session checkpoints.
- Produces proposed: `AiContextAssembler::assemble(AiContextRequest) -> Result<AiContextBundle, AiContextError>` where every `AiContextItem` carries source identity, content fingerprint, freshness, byte/token estimate, privacy class, inclusion reason, and redaction outcome; `PromptCachePlan` binds cacheable segments, provider/model, content fingerprints, privacy policy, expiry, and expected/actual cache-token usage.

- [ ] **Step 1: Add failing tests for open/dirty buffers, stale index hits, renamed/deleted files, ignored/secret files, symlink escape, remote/local workspace identity, memory opt-out, deterministic budget truncation, context changed between review and send, cache key stability, cache invalidation on content/privacy/model change, providers without cache support, and reported cache read/write token reconciliation.**
- [ ] **Step 2: Run the three focused targets; expect missing assembler/provenance failures.**
- [ ] **Step 3: Implement assembly as a read-only app service.** Bind the reviewed bundle fingerprint to the provider request and fail stale if any included source changed before dispatch.
- [ ] **Step 4: Render inspect/include/exclude controls and exact aggregate budget/egress state in the manifest panel; exclude actions update app state, not UI-owned copies.**
- [ ] **Step 5: Run focused tests; pass oracle is stable ordering, no forbidden bytes in provider payload/event/support bundle, and stale context blocks send with preserved prompt.**
- [ ] **Step 6: Through the ordinary app, inspect and edit context for a multi-crate Rust workspace, Node project, and Python package; externally capture the redacted provider payload and confirm it equals the approved manifest. Test external edit and index restart recovery.**
- [ ] **Step 7: Commit:** `git add crates/legion-app crates/legion-ai crates/legion-index crates/legion-memory crates/legion-desktop && git commit -m "feat: make AI context inspectable and fresh"`.

### S3-05: Complete the real Assist workflow

**Dependencies:** `S3-02`, `S3-03`, `S3-04`.

**Owner model:** `terra_worker`; proposal/stale-state portions: `sol_engineer`; review: `sol_reviewer`.

**Outputs:** useful inline completion, explain, inline edit, and multi-file change with streaming feedback; durable assistant session history; new/resume/rename/delete/export history controls; accept/reject/partial review; cancellation; retry; proposal conflict recovery; provider/model disclosure.

**Files:**
- Modify: `crates/legion-app/src/assist_proposal.rs`
- Modify: `crates/legion-app/src/product_ai_completion.rs`
- Modify: `crates/legion-app/src/product_ai_lane.rs`
- Modify: `crates/legion-storage/src/lib.rs`
- Modify: `crates/legion-desktop/src/view/assistant_rail.rs`
- Modify: `crates/legion-desktop/src/view/ghost_text.rs`
- Modify: `crates/legion-desktop/src/view/inline_edit.rs`
- Test: `crates/legion-app/tests/assist_inline_prediction_workflow.rs`
- Test: `crates/legion-desktop/tests/assist_delegate_reachability.rs`
- Test: `crates/legion-desktop/tests/assist_live_product.rs`

**Interfaces:**
- Consumes current `AppComposition::run_assisted_ai_operation(...)`, `start_ai_explain(...)`, `start_ai_proposal(...)`, `cancel_ai_run(...)`, S3 context bundles, provider routes, and generic proposal lifecycle.
- Produces no new write authority; it produces existing proposal payloads with complete version/target coverage and an `AiSessionCheckpoint` for restart/retry. Proposed `AssistantSessionStore::create/load/append/rename/delete/export` persists ordered user/assistant/tool/proposal references with schema version, provider/model/usage metadata, redaction state, and workspace identity.

- [ ] **Step 1: Extend app tests so real provider responses stream incrementally, cancellation ends the request, stale buffers cannot accept ghost text, ambiguous edit anchors remain reviewable failures, multi-file output creates one reviewable proposal set without writes, and session create/append/restart/resume/rename/delete/export preserves ordered history and proposal links without persisting secrets or unapproved raw context.**
- [ ] **Step 2: Run focused app tests; expect new assertions to fail on the current narrow/fixture path.**
- [ ] **Step 3: Connect Assist operations to `AiContextAssembler` and the selected real route, preserve prompt/context on retry, and keep every edit behind the proposal coordinator.**
- [ ] **Step 4: Complete keyboard and accessibility flows for request/cancel/retry, ghost-text word/line/all acceptance, explanation navigation, hunk review, approve/apply/reject, settings recovery, and session new/switch/search/rename/delete/export. Show preflight estimate and final usage/cache reconciliation for each turn.**
- [ ] **Step 5: Add native desktop tests that type requests through rendered controls and verify actual edited buffer/disk results only after approval; explicitly assert a missed click or echoed prompt cannot satisfy success.**
- [ ] **Step 6: Run the three test targets. Pass oracle: no direct dispatch from the acceptance case, correct provider/model visible, cancelled output never applies, conflicts preserve user text, and external disk content matches approved hunks.**
- [ ] **Step 7: Commit implementation/tests first:** `git add crates/legion-app crates/legion-storage crates/legion-desktop && git commit -m "feat: complete Assist product workflow"`.
- [ ] **Step 8: Build and pin that code commit in `candidate.json`; qualify one local and every required hosted matrix route on held-out Rust/TS/Python tasks with repeated-trial thresholds from Stage 0; record task success, latency/cost/resource/cache use, failures, and recovery in canonical evidence and validate against the pinned code SHA.**
- [ ] **Step 9: Commit only candidate/evidence records:** `git add plans/completion/candidate.json plans/evidence/completion && git commit -m "evidence: record Assist acceptance"`.

### S3-06: Complete Delegate as a durable bounded task loop

**Dependencies:** `S3-02`, `S3-04`; Stage 1 terminal/Git/work-preservation and Stage 2 build/test/debug workflows.

**Owner model:** `sol_engineer`; review: `sol_reviewer`.

**Outputs:** user-authored task/scope/plan, isolated execution, real tool loop, resource budgets, verification, proposal review, graceful cancel/hard kill, restart resume, and safe cleanup.

**Files:**
- Create: `crates/legion-app/src/delegate_session.rs`
- Modify: `crates/legion-app/src/lib.rs`
- Modify: `crates/legion-app/src/delegate_workflow.rs`
- Modify: `crates/legion-agent/src/agent_loop.rs`
- Modify: `crates/legion-agent/src/worktree.rs`
- Modify: `crates/legion-agent/src/evidence.rs`
- Modify: `crates/legion-desktop/src/view/worker_panel.rs`
- Modify: `crates/legion-desktop/src/view/sandbox_panel.rs`
- Test: `crates/legion-app/tests/delegated_task_integration.rs`
- Test: `crates/legion-desktop/tests/delegated_task_command_center.rs`

**Interfaces:**
- Consumes current `AppComposition::start_delegated_task_background(...)`, `start_delegated_task(...)`, `cancel_delegated_task(...)`, `DelegatedTaskSandboxOrchestrator`, `validate_delegated_task_tool_call(...)`, native tool registry, provider tool-calling, and proposal/evidence types.
- Produces proposed: `DelegateSessionService::start(DelegateSessionRequest) -> Result<DelegateSessionId, DelegateSessionError>`, `checkpoint(...)`, `resume(...)`, and `terminate(...)`; durable checkpoints bind plan/scope/sandbox lease/provider/tool turns/evidence/proposals.

- [ ] **Step 1: Add failing tests for actual file-read/edit/test tool turns, out-of-scope paths, command denial, budget exhaustion, malformed tool calls, provider timeout, child crash, cancellation during an uninterruptible child, app crash after proposal registration, restart without rerunning completed turns, dirty main workspace, and orphan reaping.**
- [ ] **Step 2: Run the focused agent/app integration tests; expect missing durable-session failures.**
- [ ] **Step 3: Extract durable session ownership from oversized composition code into `delegate_session.rs`; reuse current worktree/scope/evidence abstractions and persist before externally visible transitions.**
- [ ] **Step 4: Make verification execute the Stage 2 project command through the real terminal/process path and record exit status plus output fingerprint; fabricated passed summaries cannot make a proposal ready.**
- [ ] **Step 5: Wire rendered task/scope/plan/budget/cancel/kill/review controls through the app service. Show blocked/failed/cancelled/recoverable distinctly and preserve the user task after failure.**
- [ ] **Step 6: Run `cargo test -p legion-agent` plus the two focused app/desktop targets. Pass oracle: main workspace stays unchanged until approval, evidence references real processes, cancellation reaps children, and restart produces the same reviewable proposal without duplicate effects.**
- [ ] **Step 7: Commit implementation/tests first:** `git add crates/legion-agent crates/legion-app crates/legion-desktop && git commit -m "feat: complete durable Delegate workflow"`.
- [ ] **Step 8: Build and pin that code commit in `candidate.json`; complete held-out real-project tasks for Rust/TS/Python through the packaged app using local and hosted routes; externally verify Git diff/tests/process cleanup and exercise denial, conflict, disconnect, crash/restart, cancel, and rollback. Write canonical evidence and validate against the pinned code SHA.**
- [ ] **Step 9: Commit only candidate/evidence records:** `git add plans/completion/candidate.json plans/evidence/completion && git commit -m "evidence: record Delegate acceptance"`.

### S3-07: Add the Stage 3 native acceptance gate

**Dependencies:** `S3-05`, `S3-06`.

**Owner model:** `terra_worker`; evidence execution: `luna_worker`; review and acceptance: `sol_reviewer` plus a non-implementer.

**Outputs:** repeatable packaged-app Assist/Delegate qualification and hash-bound, independently reviewer-accepted evidence records for every Stage 0 model configuration.

**Files:**
- Create: `xtask/src/stage3_product_acceptance.rs`
- Modify: `xtask/src/main.rs`
- Create: `acceptance/scenarios/stage3/assist.toml`
- Create: `acceptance/scenarios/stage3/delegate.toml`
- Create: `.github/workflows/legion-stage3-live.yml`

**Interfaces:**
- Consumes packaged desktop binary, Stage 0 matrices and acceptance-record schema, S3-05/S3-06 workflows.
- Produces **NEW collector** `cargo run -p xtask -- stage3-product-acceptance --candidate <candidate-code-sha> --candidate-manifest plans/completion/candidate.json --matrix <path> --evidence <dir>` that writes the master plan's evidence-run records. `verify-completion` remains the only eligibility/status validator; the workflow is manual/scheduled and credential-gated, never a credential-free PR requirement.

- [ ] **Step 1: Add parser tests that reject fixture providers, direct runtime dispatch, absent external-effect checks, missing recovery cases, changed thresholds, secrets in evidence, or incomplete matrix rows.**
- [ ] **Step 2: Run `cargo test -p xtask stage3_product_acceptance`; expect missing-command failures.**
- [ ] **Step 3: Implement the NEW command as an external orchestrator: launch packaged Legion, drive native controls through the chosen Stage 0 automation route, inspect files/Git/process/provider logs externally, and emit immutable pass/fail records.**
- [ ] **Step 4: Add local and hosted scenarios for Assist and Delegate across all three languages, including cancellation, stale context, denial, conflict, provider loss, and restart.**
- [ ] **Step 5: Run parser/unit tests, then commit the collector and driver fixtures before creating the candidate:** `git add xtask acceptance/scenarios/stage3 .github/workflows/legion-stage3-live.yml && git commit -m "test: add Assist and Delegate qualification harness"`.
- [ ] **Step 6: Build the Step 5 code commit, pin `candidate.json`, execute the full NEW collector on the candidate matrix, then run `cargo run -p xtask -- verify-completion --candidate <candidate-code-sha>`. Pass oracle: every required row passes repeated-trial thresholds and safety invariants; an unavailable credential produces `blocked`, never `passed`.**
- [ ] **Step 7: Commit only candidate/evidence records:** `git add plans/completion/candidate.json plans/evidence/completion && git commit -m "evidence: qualify real Assist and Delegate workflows"`.

## Stage 4 — Complete multi-agent workflows and interoperability

### S4-01: Ratify multi-agent persistence, conflict, MCP-client, and ACP contracts

**Dependencies:** `S3-06`; Stage 0 interoperability matrix from `S0-02` and dependency graph from `S0-06`.

**Owner model:** `sol_engineer`; review: `sol_reviewer`.

**Outputs:** reviewed authority/state contract for plans, workers, budgets, tool permissions, protocol peers, replay, conflict resolution, and proposal ownership.

**Files:**
- Create: `plans/adrs/ADR-0053-multi-agent-and-interoperability-authority.md`
- Create: `crates/legion-protocol/src/interoperability.rs`
- Modify: `crates/legion-protocol/src/lib.rs`
- Modify: `plans/dependency-policy.md`
- Test: `crates/legion-protocol/tests/interoperability_contract.rs`

**Interfaces:**
- Consumes current editable plan/DAG/session/worker/evidence/MCP registry types, `McpServerDescriptor`, and current `AcpHostCommand` behavior as implementation evidence only.
- Produces proposed: `ExternalToolPeerSpec`, `ExternalAgentPeerSpec`, `InteroperabilitySessionId`, `PeerLifecycle`, `ToolPermissionGrant`, `WorkflowCheckpoint`, and `ConflictResolutionProposal`; ACP/MCP wire versions and named peers come from Stage 0.

- [ ] **Step 1: Write failing protocol tests for stable peer identity, capability negotiation, least-privilege grants, revocation, request correlation, replay/idempotency, version mismatch, disconnect, and proposal-only mutation.**
- [ ] **Step 2: Run the contract test and expect missing types.**
- [ ] **Step 3: Write the ADR and types with exact ownership and transition tables.** MCP support must include Legion acting as a client to named real servers; ACP support must include the exact Stage 0 host/client role and a real external implementation, rather than only `acp.local-adapter` metadata.
- [ ] **Step 4: Run protocol tests and `xtask check-deps`; require all contract/policy checks green and reviewer approval before S4-02/S4-04/S4-05.**
- [ ] **Step 5: Commit:** `git add plans/adrs/ADR-0053-multi-agent-and-interoperability-authority.md plans/dependency-policy.md crates/legion-protocol && git commit -m "design: define multi-agent interoperability authority"`.

### S4-02: Persist and recover real dependency-driven multi-agent execution

**Dependencies:** `S4-01`.

**Owner model:** `sol_engineer`; review: `sol_reviewer`.

**Outputs:** editable approved plans, dependency scheduling, isolated concurrent workers, durable checkpoints, deterministic resumption, budgets, conflict pause/resolution, cancellation/kill, and proposal-only merge readiness.

**Files:**
- Create: `crates/legion-app/src/workflow_supervisor.rs`
- Modify: `crates/legion-app/src/lib.rs`
- Modify: `crates/legion-agent/src/coordinator.rs`
- Modify: `crates/legion-agent/src/scheduler.rs`
- Modify: `crates/legion-agent/src/state.rs`
- Modify: `crates/legion-agent/src/merge_readiness.rs`
- Test: `crates/legion-app/tests/legion_workflow_plan_lifecycle.rs`
- Test: `crates/legion-app/tests/legion_workflow_integration.rs`

**Interfaces:**
- Consumes current `editable_plan_from_workflow_artifacts(...)`, `workflow_dag_from_approved_plan(...)`, `legion_workflow_session_from_approved_plan(...)`, `parallel_worker_lanes(...)`, and `merge_readiness_report_for_session(...)` plus S4-01 checkpoints.
- Produces proposed: `WorkflowSupervisor::start/resume/cancel/kill/resolve_conflict`, each returning canonical workflow outputs; completed workers are never replayed, and no method applies to the main workspace.

- [ ] **Step 1: Extend current lifecycle/integration tests with crash-at-every-persist-boundary, dependency fan-out/fan-in, worker panic, app drop, budget exhaustion, same-target conflict, resolution rejection, provider/tool revocation, and resume without duplicate side effects.**
- [ ] **Step 2: Run the two focused targets; capture the expected new failures.**
- [ ] **Step 3: Extract supervisor ownership and persist state before worker launch, tool dispatch, completion, conflict resolution, and proposal publication.** Reuse existing global draining supervisors so cancellation never abandons children.
- [ ] **Step 4: Bind merge readiness to real verification evidence and independent sign-off; retain the current invariant that ready state is proposal-mediated and never autonomous apply.**
- [ ] **Step 5: Run `cargo test -p legion-agent` and the focused app tests. Pass oracle: dependency order is correct, lane mates run concurrently, conflicts stop dependents, resume skips completed workers, and all sandboxes/processes are reaped.**
- [ ] **Step 6: Commit:** `git add crates/legion-agent crates/legion-app && git commit -m "feat: persist multi-agent workflow execution"`.

### S4-03: Complete the fleet/plan/decision user workflow

**Dependencies:** `S4-02`.

**Owner model:** `terra_worker`; review: `sol_reviewer`.

**Outputs:** create/edit/approve plan, inspect dependencies/workers/context/budgets/evidence, grant/revoke permissions, resolve conflicts, gracefully stop or hard kill, resume after restart, review final proposals.

**Files:**
- Modify: `crates/legion-desktop/src/view/plan_editor.rs`
- Modify: `crates/legion-desktop/src/view/fleet_board.rs`
- Modify: `crates/legion-desktop/src/view/fleet_card.rs`
- Modify: `crates/legion-desktop/src/view/agent_comm.rs`
- Modify: `crates/legion-desktop/src/workflow.rs`
- Modify: `crates/legion-desktop/src/bridge.rs`
- Test: `crates/legion-desktop/tests/legion_workflow_command_center.rs`

**Interfaces:**
- Consumes `WorkflowSupervisor` methods and existing `LegionWorkflowProjection`/proposal/evidence projections.
- Produces only desktop intents such as plan revision/approval, permission decision, conflict-resolution proposal, cancellation, kill, and review; app service validates every ID against current projection/state.

- [ ] **Step 1: Add failing rendered-control tests for keyboard/accessibility operation, plan dependency editing, permission revocation, conflict comparison/choice, budget limit changes before launch, restart resume, and final proposal review.**
- [ ] **Step 2: Run the command-center test; expect unreachable/incomplete interaction failures.**
- [ ] **Step 3: Implement focused view/bridge changes without parsing display strings or moving ownership into UI.** Make every dangerous control name its workflow/worker/peer and require current app validation.
- [ ] **Step 4: Run the test target; pass oracle includes wrong/stale IDs denied, stop vs kill distinct, status derived from projection fields, and no proposal auto-applied.**
- [ ] **Step 5: Complete a native two-worker workflow, close/relaunch mid-run, resolve an induced overlapping edit, and externally verify final Git diff/tests and zero orphan processes.**
- [ ] **Step 6: Commit:** `git add crates/legion-desktop && git commit -m "feat: complete multi-agent command center"`.

### S4-04: Implement Legion as a production MCP client

**Dependencies:** `S4-01`, `S4-02`.

**Owner model:** `sol_engineer`; review: `sol_reviewer`.

**Outputs:** completed configured server lifecycle, handshake/capability discovery, tools/resources/prompts required by Stage 0, per-tool permission, bounded invocation, cancellation, audit, reconnect, and real named-server interoperability by extending the existing `McpClient<T>`, stdio transport, and streamable-HTTP transport.

**Files:**
- Create: `crates/legion-ai-providers/src/mcp_client.rs`
- Modify: `crates/legion-ai-providers/src/lib.rs`
- Modify: `crates/legion-app/src/workflow_supervisor.rs`
- Modify: `crates/legion-desktop/src/view/manifest_panel.rs`
- Test: `crates/legion-ai-providers/tests/mcp_client_interop.rs`
- Test: `crates/legion-app/tests/legion_workflow_integration.rs`

**Interfaces:**
- Consumes current `McpClient<T>`, `McpTransport`, `StdioMcpTransport`, `StreamableHttpMcpTransport`, MCP registry/descriptors, S4-01 `ExternalToolPeerSpec`/permission grants, and current MCP server conformance as complementary server-role evidence.
- Produces proposed: `McpClientSession::connect/list_capabilities/invoke/cancel/disconnect`; tool results re-enter worker loops as bounded, redacted tool-result blocks and never mutate workspace directly.

- [ ] **Step 1: Write failing client tests against the Stage 0 named reference server(s): initialize/version negotiation, list changes, invocation, malformed frames, oversized output, timeout, cancellation, process death, reconnect, and permission revocation.**
- [ ] **Step 2: Run `cargo test -p legion-ai-providers --test mcp_client_interop`; expect failures at the first missing production behavior, not a claim that no client exists.**
- [ ] **Step 3: Extract the current MCP client/transport region from `lib.rs` into `mcp_client.rs` without behavior change, then add the missing process/transport supervision, exact protocol framing, limits, correlation, capability diffing, and redaction.** Keep `McpStdioServer` behavior intact.
- [ ] **Step 4: Route a worker MCP request through projected permission review and back into the real tool loop; persist permission and invocation evidence.**
- [ ] **Step 5: Run MCP provider and workflow integration tests. Pass oracle: reference server sees the real request, denial causes no invocation, cancellation reaps/abandons no request, and tool output cannot directly write.**
- [ ] **Step 6: Commit:** `git add crates/legion-ai-providers crates/legion-app crates/legion-desktop && git commit -m "feat: add production MCP client"`.

### S4-05: Implement real ACP interoperability and external-agent containment

**Dependencies:** `S4-01`, `S4-02`.

**Owner model:** `sol_engineer`; review: `sol_reviewer`.

**Outputs:** negotiated ACP session with named Stage 0 peer, scoped task/context exchange, streaming progress, proposal/evidence return, cancellation/restart, and containment equivalent to local workers.

**Files:**
- Modify: `crates/legion-app/src/acp_host.rs`
- Modify: `crates/legion-app/src/workflow_supervisor.rs`
- Modify: `crates/legion-agent/src/external.rs`
- Modify: `crates/legion-agent/src/scope.rs`
- Test: `crates/legion-agent/tests/external_agent_containment.rs`
- Test: `crates/legion-app/tests/acp_interoperability.rs`

**Interfaces:**
- Consumes current `AcpHostCommand`, `run_acp_host_proposal(...)`, external-agent containment/scope, S4-01 `ExternalAgentPeerSpec`, and workflow checkpoints.
- Produces proposed: `AcpSession::negotiate/start_task/cancel/resume/close`; outputs are canonical task packet, progress, proposal, and evidence records associated with peer/session IDs.

- [ ] **Step 1: Add failing tests against the named real ACP peer for negotiation, task/progress/proposal, scope denial, version mismatch, malformed output, disconnect, resume, cancellation, and peer process escape attempts.**
- [ ] **Step 2: Run focused tests; expect current local-adapter-only behavior to fail interoperability assertions.**
- [ ] **Step 3: Implement the ratified transport/role and map messages to canonical Legion artifacts.** Apply the same path, command, network, budget, proposal, audit, and process-containment gates as local Delegate workers.
- [ ] **Step 4: Connect ACP workers to `WorkflowSupervisor` without special-case merge authority; peer proposals follow ordinary review/apply.**
- [ ] **Step 5: Run focused tests. Pass oracle: a real peer completes a task, denied actions have no effect, disconnection resumes without duplication, cancellation reaps the peer, and main workspace changes only after approval.**
- [ ] **Step 6: Commit:** `git add crates/legion-agent crates/legion-app && git commit -m "feat: complete ACP interoperability"`.

### S4-06: Qualify multi-agent, MCP, and ACP through the packaged app

**Dependencies:** `S4-03`, `S4-04`, `S4-05`.

**Owner model:** `terra_worker` for harness, `luna_worker` for evidence runs; acceptance: `sol_reviewer` plus non-implementer.

**Outputs:** full Stage 4 product gate with real multi-worker effects and named MCP/ACP peers.

**Files:**
- Create: `xtask/src/stage4_product_acceptance.rs`
- Modify: `xtask/src/main.rs`
- Create: `acceptance/scenarios/stage4/multi_agent.toml`
- Create: `acceptance/scenarios/stage4/mcp_acp.toml`
- Create: `.github/workflows/legion-stage4-live.yml`

**Interfaces:**
- Consumes packaged desktop, Stage 0 peer matrix, S4 workflows.
- Produces **NEW collector** `cargo run -p xtask -- stage4-product-acceptance --candidate <candidate-code-sha> --candidate-manifest plans/completion/candidate.json --matrix <path> --evidence <dir>` with master-schema records and external Git/process/server checks; `verify-completion` remains authoritative.

- [ ] **Step 1: Add harness tests rejecting scripted providers, loopback-only transports, fabricated evidence, missing conflict/restart cases, or absent external peer versions.**
- [ ] **Step 2: Implement the NEW command and scenarios for dependency fan-out/fan-in, real conflicting edits, approval, cancellation/kill, app crash/resume, MCP permission/revocation, and ACP disconnect/resume.**
- [ ] **Step 3: Run `cargo test -p xtask stage4_product_acceptance`, then commit the collector and driver fixtures:** `git add xtask acceptance/scenarios/stage4 .github/workflows/legion-stage4-live.yml && git commit -m "test: add multi-agent interoperability harness"`.
- [ ] **Step 4: Build and pin that code commit in `candidate.json`, run the full collector, then run `cargo run -p xtask -- verify-completion --candidate <candidate-code-sha>`. Pass oracle: workers complete a real cross-file task, externally run verification succeeds, peers are named/real, failures remain failures, and no process/sandbox leaks.**
- [ ] **Step 5: Commit only candidate/evidence records:** `git add plans/completion/candidate.json plans/evidence/completion && git commit -m "evidence: qualify multi-agent interoperability"`.

## Stage 5 — Complete extensions, remote development, collaboration, and enterprise governance

### S5-01: Ratify the full extension-host compatibility and storage contract

**Dependencies:** Stage 0 extension matrix `S0-02` and dependency graph `S0-06`; `S4-01` capability principles.

**Owner model:** `sol_engineer`; review: `sol_reviewer`.

**Outputs:** exact supported VS Code API/contribution/version matrix; WASM WIT contract; Node/web-worker isolation; webview/notebook/custom-editor security; workspace/global/secret storage; install/update/rollback lifecycle.

**Files:**
- Create: `plans/adrs/ADR-0054-full-extension-host-compatibility.md`
- Create: `crates/legion-protocol/src/extension_host.rs`
- Modify: `crates/legion-protocol/src/lib.rs`
- Modify: `plans/dependency-policy.md`
- Test: `crates/legion-protocol/tests/extension_host_contract.rs`

**Interfaces:**
- Consumes current `PluginRuntimeHost`, `WasmPluginHost`, `SignedExtensionRegistry`, extension catalog projections, `VsCodeCompatibilityTier`, `VsCodeExtensionHostRuntime`, and contribution classifications.
- Produces proposed: `ExtensionHostSpec`, `ExtensionInstanceId`, `ExtensionLifecycle`, `ExtensionStorageScope`, `ExtensionContributionRegistration`, `ExtensionUiChannel`, and versioned host request/response envelopes.

- [ ] **Step 1: Write failing contract tests covering signatures, permissions, activation, update rollback, disabled/quarantined state, runtime crash, API version negotiation, host-call quotas, storage namespaces/quotas/migration/deletion, webview origin/CSP/messages, notebook cell/document identity, and custom-editor save proposals.**
- [ ] **Step 2: Run the contract test; expect missing types.**
- [ ] **Step 3: Write ADR/types with a row for every Stage 0 required API and representative extension.** Metadata parsing or `NodeSidecar` classification alone is explicitly insufficient; each row names runtime, authority, recovery, and acceptance.
- [ ] **Step 4: Run contract tests and `xtask check-deps`; obtain security/architecture review.**
- [ ] **Step 5: Commit:** `git add plans/adrs/ADR-0054-full-extension-host-compatibility.md plans/dependency-policy.md crates/legion-protocol && git commit -m "design: ratify full extension host contract"`.

### S5-02: Productize signed WASM extensions and extension storage

**Dependencies:** `S5-01`.

**Owner model:** `sol_engineer`; catalog/UI work: `terra_worker`; review: `sol_reviewer`.

**Outputs:** install/update/disable/enable/remove from a real artifact source, Wasmtime execution of required WIT contributions, scoped durable storage, permissions/audit, crash quarantine, rollback.

**Files:**
- Create: `crates/legion-plugin/src/storage.rs`
- Modify: `crates/legion-plugin/src/host.rs`
- Modify: `crates/legion-plugin/src/lib.rs`
- Modify: `crates/legion-plugin/src/registry.rs`
- Modify: `crates/legion-app/src/extension_management.rs`
- Modify: `crates/legion-desktop/src/view/extensions_panel.rs`
- Test: `crates/legion-plugin/tests/product_host.rs`
- Test: `crates/legion-desktop/tests/extensions_panel.rs`
- Test: `crates/legion-desktop/tests/plugin_management.rs`

**Interfaces:**
- Consumes current signed registry, permission review, `WasmPluginHost::invoke(...)`, audit/quota machinery, S5-01 envelopes/scopes.
- Produces proposed: `ExtensionStorageService::get/set/delete/clear_namespace`; product activation returns registered contributions and never raw host authority.

- [ ] **Step 1: Add failing tests for real signed install, contribution execution, storage persistence/isolation/quota/migration/delete, capability denial, tamper, infinite loop/memory/payload limits, crash quarantine, interrupted update rollback, and restart reactivation.**
- [ ] **Step 2: Run existing hostile/quotas/tampered tests plus new product-host tests; expect only new product path failures.**
- [ ] **Step 3: Implement storage and product activation by composing current verified registry/Wasmtime host.** Store no secrets outside keyring-backed secret scope; workspace mutation is always a proposal.
- [ ] **Step 4: Wire native lifecycle and permission controls; show source, signer, digest, runtime, permissions, quotas, crash state, and rollback result.**
- [ ] **Step 5: Run plugin and desktop tests. Pass oracle: actual WASM code contributes behavior, restart preserves allowed state, hostile module is contained/quarantined, and denied capabilities have no external effect.**
- [ ] **Step 6: Commit:** `git add crates/legion-plugin crates/legion-app crates/legion-desktop && git commit -m "feat: productize signed WASM extensions"`.

### S5-03: Implement the required VS Code Node and web-worker hosts

**Dependencies:** `S5-01`, `S5-02`.

**Owner model:** `sol_engineer`; review: `sol_reviewer`.

**Outputs:** supervised Node and web-worker runtimes for the exact Tier 1/2 API matrix, activation events, commands/languages/debug/tasks as ratified, API shims, storage, permissions, update/restart/crash handling.

**Files:**
- Create: `crates/legion-vscode-compat/src/node_host.rs`
- Create: `crates/legion-vscode-compat/src/web_host.rs`
- Modify: `crates/legion-vscode-compat/src/lib.rs`
- Modify: `crates/legion-app/src/extension_management.rs`
- Test: `crates/legion-vscode-compat/tests/node_host_interop.rs`
- Test: `crates/legion-vscode-compat/tests/web_host_interop.rs`

**Interfaces:**
- Consumes current `resolve_open_vsx_extension_metadata(...)`, `load_open_vsx_extension(...)`, `manifest_from_package_json(...)`, `extension_host_session_for_manifest(...)`, S5-01 envelopes, S5-02 storage/lifecycle.
- Produces proposed: `NodeExtensionHost::start/activate/dispatch/shutdown` and `WebExtensionHost::start/activate/dispatch/shutdown`; all host calls traverse capability checks and bounded envelopes.

- [ ] **Step 1: Write failing interop tests using every Stage 0 representative Tier 1/2 extension; verify real activation and user-visible capability, not only manifest classification. Include unsupported-API diagnostics.**
- [ ] **Step 2: Run compatibility tests; expect current sidecar-selection-only path to fail real execution.**
- [ ] **Step 3: Implement version-pinned sidecar launch, IPC framing, API shims, activation/event routing, storage bridge, output limits, cancellation, crash backoff/quarantine, and clean shutdown.**
- [ ] **Step 4: Connect runtime choice to extension management and desktop status; unsupported APIs fail explicitly without partially activating the extension.**
- [ ] **Step 5: Run all `legion-vscode-compat` tests and relevant extension desktop tests. Pass oracle: named extensions perform their promised capability, denied calls do nothing, and crashed/updated hosts recover without corrupting storage.**
- [ ] **Step 6: Commit:** `git add crates/legion-vscode-compat crates/legion-app crates/legion-desktop && git commit -m "feat: run supported VS Code extensions"`.

### S5-04: Implement webviews, notebooks, and custom editors

**Dependencies:** `S5-03`.

**Owner model:** `sol_engineer` for sandbox/authority; `terra_worker` for desktop rendering; review: `sol_reviewer`.

**Outputs:** Tier 3 runtime and UI surfaces required by the Stage 0 matrix, including safe resource loading/messages, notebook execution/state, custom-editor document/save/revert/backup, storage, restart recovery.

**Files:**
- Create: `crates/legion-vscode-compat/src/tier3.rs`
- Modify: `crates/legion-vscode-compat/src/lib.rs`
- Create: `crates/legion-desktop/src/view/extension_webview.rs`
- Create: `crates/legion-desktop/src/view/notebook.rs`
- Create: `crates/legion-desktop/src/view/custom_editor.rs`
- Modify: `crates/legion-desktop/src/view.rs`
- Test: `crates/legion-vscode-compat/tests/tier3_interop.rs`
- Test: `crates/legion-desktop/tests/extension_tier3.rs`

**Interfaces:**
- Consumes S5-01 `ExtensionUiChannel`/contribution registration, S5-03 hosts, app editor/terminal/proposal services.
- Produces proposed: versioned `WebviewMessage`, `NotebookOperation`, and `CustomEditorOperation`; custom-editor and notebook file changes produce ordinary proposals with document fingerprints.

- [ ] **Step 1: Add failing tests with Stage 0 representative extensions for rendered webview interaction, CSP/origin/resource denial, message quotas, notebook open/edit/run/interrupt/save/restart, and custom editor open/edit/save/revert/backup/conflict.**
- [ ] **Step 2: Run Tier 3 tests; expect current classification-only behavior to fail.**
- [ ] **Step 3: Implement bounded extension UI channels and focused projection-only views.** Execute notebook kernels through the real task/terminal authority; route document mutations through editor/proposal services.
- [ ] **Step 4: Run focused and hostile tests. Pass oracle: real extension UI is usable, CSP/permissions contain it, interrupted cells terminate, dirty documents recover, and save conflicts preserve both versions.**
- [ ] **Step 5: Commit:** `git add crates/legion-vscode-compat crates/legion-desktop && git commit -m "feat: support VS Code Tier 3 surfaces"`.

### S5-05: Ratify remote-workspace authority

**Dependencies:** Stage 0 remote matrix `S0-02` and dependency graph `S0-06`; Stage 1/2 local service contracts; `S4-01` interoperability principles.

**Owner model:** `sol_engineer`; review: `sol_reviewer`.

**Outputs:** reviewed remote workspace identity, client/agent split, authenticated transport, version/capability negotiation, remote file/LSP/terminal/task/debug authorities, proposal mediation, reconnect/offline semantics, and agent install/update/cleanup contract.

**Files:**
- Create: `plans/adrs/ADR-0055-remote-workspace-authority.md`
- Create: `crates/legion-protocol/src/remote_dev.rs`
- Modify: `crates/legion-protocol/src/lib.rs`
- Modify: `plans/dependency-policy.md`
- Test: `crates/legion-protocol/tests/remote_dev_contract.rs`

**Interfaces:**
- Consumes current `RemoteSessionRuntime`, `RemoteSessionTransport`, `RemoteDevelopmentRuntime`, mTLS carrier/state machine, `plan_ssh_session(...)`, `plan_devcontainer_session_from_json(...)` as substrate.
- Produces proposed: `RemoteWorkspaceRequest/Response`, `RemoteWorkspaceIdentity`, and service-specific handles; a remote file handle cannot be mistaken for a local `PathBuf`.

- [ ] **Step 1: Write failing contract tests for identity/version/capability negotiation, mTLS host identity, replay/idempotency, file/version fingerprints, terminal/LSP/debug process ownership, proposal-only writes, reconnect, offline read-only state, agent upgrade, and cleanup.**
- [ ] **Step 2: Run contract tests; expect missing protocol types.**
- [ ] **Step 3: Write ADR/protocol transition and authority rules.** Explicitly prohibit mapping remote paths into local workspace authority or fabricating local disk effects.
- [ ] **Step 4: Run protocol tests and `xtask check-deps`; obtain architecture/security review.**
- [ ] **Step 5: Commit:** `git add plans/adrs/ADR-0055-remote-workspace-authority.md plans/dependency-policy.md crates/legion-protocol && git commit -m "design: define remote workspace authority"`.

### S5-06: Implement and supervise the remote workspace agent

**Dependencies:** `S5-05`.

**Owner model:** `sol_engineer`; review: `sol_reviewer`.

**Outputs:** versioned remote agent binary, authenticated request dispatcher for file/search/LSP/terminal/task/debug/Git services, proposal-mediated writes, install/upgrade/rollback, health/reconnect, and cleanup.

**Files:**
- Create: `crates/legion-remote-transport/src/agent.rs`
- Create: `crates/legion-remote-transport/src/bin/legion-remote-agent.rs`
- Modify: `crates/legion-remote-transport/src/lib.rs`
- Modify: `crates/legion-remote-transport/Cargo.toml`
- Test: `crates/legion-remote-transport/tests/remote_agent_contract.rs`

**Interfaces:**
- Consumes S5-05 request/response/identity/service-handle protocol and current `RemoteTransportCarrier`/`RemoteTransportStateMachine`.
- Produces proposed: `RemoteWorkspaceAgent::serve(AgentConfig, impl RemoteTransportCarrier) -> Result<(), RemoteAgentError>` and versioned agent package/health descriptors; service dispatch delegates to remote-local authoritative services rather than shell strings.

- [ ] **Step 1: Add failing tests that launch the real agent process and exercise negotiation, file read/version, proposal write, search, LSP child, PTY child, task/test exit status, debug child, Git status, cancellation, malformed/oversized frame, process crash, reconnect/replay, upgrade rollback, and bounded shutdown.**
- [ ] **Step 2: Run `cargo test -p legion-remote-transport --test remote_agent_contract`; expect missing agent/binary failures.**
- [ ] **Step 3: Implement the agent dispatcher and service supervisors with scoped canonical remote paths, process ownership, output/resource limits, correlation, metadata-only logs, and no raw credentials in arguments or frames.**
- [ ] **Step 4: Implement staged agent install/upgrade with digest verification, health confirmation, atomic activation, and rollback to the prior compatible version.**
- [ ] **Step 5: Run all `legion-remote-transport` tests and security review. Pass oracle: external effects occur only on the agent host, cancelled/crashed requests leave no children, replay is idempotent, and failed upgrades restore the previous healthy agent.**
- [ ] **Step 6: Commit:** `git add crates/legion-remote-transport && git commit -m "feat: implement remote workspace agent"`.

### S5-07: Complete SSH and container remote development

**Dependencies:** `S5-06`; complete Stage 2 language workflows.

**Owner model:** `sol_engineer`; desktop UI: `terra_worker`; review: `sol_reviewer`.

**Outputs:** real SSH host and dev-container connect/provision/open, remote explorer/edit/search/LSP/terminal/build/test/debug/Git, port/credential policy, reconnect/offline transitions, safe close/remove.

**Files:**
- Create: `crates/legion-remote/src/ssh.rs`
- Create: `crates/legion-remote/src/container.rs`
- Create: `crates/legion-remote/src/workspace.rs`
- Modify: `crates/legion-remote/src/lib.rs`
- Modify: `crates/legion-app/src/lib.rs`
- Modify: `crates/legion-desktop/src/view/cloud_lane.rs`
- Test: `crates/legion-remote/tests/ssh_product.rs`
- Test: `crates/legion-remote/tests/container_product.rs`
- Test: `crates/legion-desktop/tests/remote_workspace_gui.rs`

**Interfaces:**
- Consumes S5-05 remote protocol types, the S5-06 agent, current connection planners and reconnect transport; consumes Stage 2 service commands through remote handles.
- Produces proposed: `SshRemoteConnector::connect(...) -> RemoteWorkspaceSession`, `ContainerRemoteConnector::open(...) -> RemoteWorkspaceSession`, and app intents for connect/reconnect/go-offline/close/remove.

- [ ] **Step 1: Add failing tests against Stage 0 real SSH hosts and container engines for auth/host-key decisions, install/version negotiation, workspace open, remote file edit/save, search, LSP, terminal, test, debug, Git, disconnect/reconnect, offline state, low disk, permission change, and cleanup.**
- [ ] **Step 2: Run focused remote tests; expect fixture/loopback behavior to fail real endpoint assertions.**
- [ ] **Step 3: Implement SSH connector with pinned host identity, keyring credential references, bounded agent bootstrap, and no shell-string interpolation; implement container connector from parsed configuration with explicit mounts/env/ports and image identity.**
- [ ] **Step 4: Route all remote operations through remote handles and app orchestration.** Local disk remains untouched; remote writes use proposals and remote fingerprints; disconnect preserves dirty buffers and queued review state.
- [ ] **Step 5: Complete native connect/status/reconnect/offline/close/remove controls and accessibility.**
- [ ] **Step 6: Run remote and desktop targets. Pass oracle: external host/container shows actual files/processes/Git changes, real LSP/test/debug work, reconnect resumes without duplicate writes, and credentials/remote source stay out of diagnostics.**
- [ ] **Step 7: Commit:** `git add crates/legion-remote crates/legion-app crates/legion-desktop && git commit -m "feat: complete SSH and container development"`.

### S5-08: Ratify collaboration and enterprise identity/policy contracts

**Dependencies:** Stage 0 collaboration/identity matrix `S0-02` and dependency graph `S0-06`; `S5-05` workspace identity.

**Owner model:** `sol_engineer`; review: `sol_reviewer` and security owner.

**Outputs:** topology, participant/document identity, CRDT/reconciliation, shared proposal/approval semantics, RBAC, OIDC SSO, SCIM provisioning, policy distribution/enforcement, audit/export/retention, tenant isolation, outage/revocation behavior.

**Files:**
- Create: `plans/adrs/ADR-0056-collaboration-enterprise-authority.md`
- Create: `crates/legion-protocol/src/enterprise.rs`
- Modify: `crates/legion-protocol/src/lib.rs`
- Modify: `plans/dependency-policy.md`
- Test: `crates/legion-protocol/tests/enterprise_contract.rs`

**Interfaces:**
- Consumes current collaboration session/operation/presence/replay/audit types and proposal lifecycle; current loopback runtime is substrate evidence.
- Produces proposed: `EnterpriseSubject`, `EnterpriseRole`, `EnterprisePolicyBundle`, `CollaborationDocumentId`, `SharedProposal`, `SharedApproval`, `ScimProvisioningEvent`, and signed policy epoch/version semantics.

- [ ] **Step 1: Write failing tests for tenant/workspace/document binding, role permissions, concurrent operations, shared proposal quorum, stale/revoked approval, offline reconciliation, OIDC subject binding, SCIM create/update/disable/delete, signed policy update/downgrade/revocation, retention/export, and audit non-repudiation.**
- [ ] **Step 2: Run contract tests; expect missing types.**
- [ ] **Step 3: Write the ADR and types.** Define exact CRDT choice from Stage 0/ADR-0041 resolution, identity provider/profile versions, quorum rules, server trust, encryption, policy precedence, data residency/retention, and degraded-mode behavior.
- [ ] **Step 4: Run contract tests and `xtask check-deps`; require independent security review before implementation.**
- [ ] **Step 5: Commit:** `git add plans/adrs/ADR-0056-collaboration-enterprise-authority.md plans/dependency-policy.md crates/legion-protocol && git commit -m "design: define collaboration enterprise authority"`.

### S5-09: Implement the collaboration transport and control plane

**Dependencies:** `S5-08`.

**Owner model:** `sol_engineer`; review: `sol_reviewer` and security owner.

**Outputs:** an independently deployable collaboration service and desktop client transport with authenticated tenant/workspace sessions, durable operation/checkpoint storage, ordered/replay-safe delivery, reconnect, health, backup/restore, and bounded shutdown.

**Files:**
- Create: `crates/legion-collaboration/src/transport.rs`
- Create: `crates/legion-collaboration/src/control_plane.rs`
- Create: `crates/legion-collaboration/src/bin/legion-collaboration-server.rs`
- Modify: `crates/legion-collaboration/Cargo.toml`
- Modify: `crates/legion-collaboration/src/lib.rs`
- Modify: `crates/legion-app/src/lib.rs`
- Modify: `plans/dependency-policy.md`
- Test: `crates/legion-collaboration/tests/control_plane_transport.rs`

**Interfaces:**
- Consumes S5-08 tenant/subject/role/document/policy contracts and the transport/security profile ratified by its ADR; current `CollaborationSessionRuntime` remains the in-process state engine.
- Produces proposed: `CollaborationControlPlane::serve(CollaborationServerConfig)`, `CollaborationClientTransport::connect(CollaborationEndpoint, EnterpriseSubject) -> Result<CollaborationConnection, CollaborationTransportError>`, and durable `CollaborationCheckpointStore`; frames carry tenant/workspace/document/session identity and monotonic sequence/correlation data.

- [ ] **Step 1: Add failing multi-process tests that launch the real server binary and two independent clients for authenticated join, operation/presence delivery, duplicate/reorder rejection, reconnect/resume, backpressure, server restart from durable checkpoint, tenant isolation, revoked subject, corrupt storage, backup/restore, and bounded shutdown.**
- [ ] **Step 2: Run `cargo test -p legion-collaboration --test control_plane_transport`; expect missing transport/server failures.**
- [ ] **Step 3: Implement the exact ADR-ratified wire transport with certificate/token verification, bounded frames/queues, correlation, replay windows, heartbeat/health, cancellation, and secret-redacted diagnostics.** Do not reuse the current loopback calls as a production transport.
- [ ] **Step 4: Implement the control plane over `CollaborationSessionRuntime` with durable append/checkpoint order, tenant/workspace authorization before state access, atomic recovery, and explicit schema migration/rollback.**
- [ ] **Step 5: Connect the desktop app to `CollaborationClientTransport`; preserve offline dirty state and report unavailable/degraded/reconnecting without constructing successful collaboration projections.**
- [ ] **Step 6: Run the focused test, `cargo test -p legion-collaboration`, `cargo run -p xtask -- check-deps`, and security review. Pass oracle: real processes exchange/recover operations, restart preserves state, revoked/cross-tenant requests have no effect, and shutdown leaves no listener/child.**
- [ ] **Step 7: Commit:** `git add crates/legion-collaboration crates/legion-app plans/dependency-policy.md && git commit -m "feat: add collaboration control plane"`.

### S5-10: Implement concurrent collaboration and shared proposals

**Dependencies:** `S5-08`, `S5-09`.

**Owner model:** `sol_engineer`; UI: `terra_worker`; review: `sol_reviewer`.

**Outputs:** real multi-participant sessions over S5-09, CRDT-backed text/presence, reconnect/reconciliation, conflicts, shared proposal review/quorum, proposal application through workspace authority, history/audit.

**Files:**
- Create: `crates/legion-collaboration/src/shared_proposals.rs`
- Modify: `crates/legion-collaboration/src/lib.rs`
- Modify: `crates/legion-app/src/lib.rs`
- Modify: `crates/legion-desktop/src/view/agent_comm.rs`
- Test: `crates/legion-collaboration/tests/concurrent_sessions.rs`
- Test: `crates/legion-desktop/tests/collaboration_gui.rs`

**Interfaces:**
- Consumes current `CollaborationSessionRuntime::submit_operation(...)`, presence/replay/reconnect/audit behavior, S5-08 CRDT/shared proposal types, S5-09 real transport/control plane, editor snapshots and proposal coordinator.
- Produces proposed: `SharedProposalService::submit/review/revoke/apply`; only app/workspace authority performs apply after current quorum and version validation.

- [ ] **Step 1: Add failing multi-process/client tests for concurrent Unicode/multi-cursor edits, operation reorder/duplication/drop, offline edits, reconnect, identity removal, proposal quorum/revocation, external edit conflict, and server restart.**
- [ ] **Step 2: Run collaboration/desktop tests; expect current loopback/projection path to fail real concurrency assertions.**
- [ ] **Step 3: Implement ratified CRDT persistence/reconciliation and shared proposal service.** Preserve causal metadata, reject cross-tenant/document operations, and never turn remote approval into direct buffer mutation.
- [ ] **Step 4: Complete participant/presence/conflict/shared-review native controls; stale/quorum-lost approvals visibly return to review.**
- [ ] **Step 5: Run focused tests. Pass oracle: independently launched participants converge byte-for-byte, identity removal stops new operations, reconnect does not duplicate, and disk changes only after valid shared approval.**
- [ ] **Step 6: Commit:** `git add crates/legion-collaboration crates/legion-app crates/legion-desktop && git commit -m "feat: complete collaborative editing and review"`.

### S5-11: Implement SSO, SCIM, enterprise policy, audit, and retention

**Dependencies:** `S5-08`, `S5-09`, `S5-10`.

**Owner model:** `sol_engineer`; review: `sol_reviewer` and security owner.

**Outputs:** OIDC login/logout/session refresh; an authenticated server-side SCIM provisioning API used by the Stage 0 identity provider; user/group lifecycle and role mapping; signed policy distribution; offline cache/expiry; enforced AI/extension/remote/collaboration ceilings; audit export and retention/deletion.

**Files:**
- Create: `crates/legion-collaboration/src/identity.rs`
- Create: `crates/legion-collaboration/src/scim.rs`
- Modify: `crates/legion-collaboration/src/control_plane.rs`
- Modify: `crates/legion-collaboration/src/bin/legion-collaboration-server.rs`
- Modify: `crates/legion-protocol/src/enterprise.rs`
- Create: `crates/legion-app/src/enterprise.rs`
- Modify: `crates/legion-app/src/lib.rs`
- Modify: `crates/legion-security/src/lib.rs`
- Modify: `crates/legion-retention/src/lib.rs`
- Test: `crates/legion-app/tests/enterprise_identity_policy.rs`
- Test: `crates/legion-collaboration/tests/scim_provisioning_api.rs`
- Test: `crates/legion-security/tests/enterprise_policy.rs`

**Interfaces:**
- Consumes S5-08 enterprise types and S5-09 control-plane identity hooks, current org policy ceiling behavior, OS keyring, security broker, storage/retention services.
- Produces proposed: `ScimProvisioningApi::handle(AuthenticatedScimRequest) -> Result<ScimResponse, ScimError>` on the collaboration control plane; `EnterpriseIdentityService::login/refresh/logout`; `EnterprisePolicyService::verify_and_apply`; audit export/deletion handles. The server, rather than the desktop, applies accepted SCIM mutations idempotently; product services consume read-only effective identity/policy snapshots.

- [ ] **Step 1: Add failing real-endpoint tests against Stage 0 OIDC/SCIM references for PKCE/state/nonce, refresh/revocation, authenticated SCIM discovery and user/group create/read/replace/patch/delete, declared schema/version/filter/pagination semantics, bearer-token audience/scope/expiry/revocation, tenant binding, idempotent retries, request/body/rate limits, group-role changes, disabled/deleted users, signed policy rollout/rollback/expiry, offline behavior, tenant isolation, audit export, retention expiry, and deletion.**
- [ ] **Step 2: Run focused tests; expect missing services.**
- [ ] **Step 3: Implement desktop identity tokens as keyring-backed references and validate issuer/audience/signature/time/nonce. Implement the SCIM API in the control-plane server with separately scoped bearer credentials, tenant-from-credential binding, standards-compliant error/status bodies for the ratified profile, transactional idempotent user/group mutations, pagination/filter limits, revocation, rate/body bounds, and metadata-safe audit. Never expose SCIM bearer credentials to the desktop client.**
- [ ] **Step 4: Verify signed policies and atomically publish effective snapshots; wire ceilings into AI egress/models, extension install/capabilities, remote targets, collaboration roles, telemetry, retention, and export. Fail closed after ratified expiry/grace rules.**
- [ ] **Step 5: Add native account/policy/audit/retention views and recovery messages.** Users can inspect effective policy and why an action is denied without seeing secrets.
- [ ] **Step 6: Run focused and affected subsystem tests. Pass oracle: the real IdP provisions through the server API and the desktop observes the resulting role/disable/delete without injected events; invalid scopes/revoked tokens/cross-tenant IDs have no effect; policy denial happens before effects; cached policy cannot be downgraded; audit/export/deletion are externally verified.**
- [ ] **Step 7: Commit implementation/tests first:** `git add crates/legion-collaboration crates/legion-protocol crates/legion-app crates/legion-security crates/legion-retention crates/legion-desktop && git commit -m "feat: enforce enterprise identity and policy"`.
- [ ] **Step 8: Build desktop/service artifacts from the Step 7 code commit and pin them in `candidate.json`. Through the real Stage 0 IdP, create a user and group assignment, log into the packaged desktop, change roles, disable and delete the subject, and verify control-plane storage/audit plus desktop enforcement after each transition. Record canonical evidence with network/service/desktop versions and recovery after IdP/control-plane outage; validate against the pinned code SHA.**
- [ ] **Step 9: Commit only candidate/evidence records:** `git add plans/completion/candidate.json plans/evidence/completion && git commit -m "evidence: record enterprise identity acceptance"`.

### S5-12: Complete explicit opt-in telemetry and training-data control

**Dependencies:** `S5-11`; Stage 0 telemetry/retention requirements.

**Owner model:** `sol_engineer`; consent UI: `terra_worker`; review: `sol_reviewer` and privacy/security owner.

**Outputs:** separate crash/product/training consent, preview/redaction, local spool, explicit upload, revocation, export, deletion, retention, provenance, candidate-feedback workflow, and verified Manual/offline zero egress.

**Files:**
- Create: `crates/legion-app/src/training_consent.rs`
- Modify: `crates/legion-app/src/first_run.rs`
- Modify: `crates/legion-telemetry/src/lib.rs`
- Modify: `crates/legion-retention/src/training.rs`
- Modify: `crates/legion-observability/src/training.rs`
- Modify: `crates/legion-desktop/src/view/about.rs`
- Test: `crates/legion-app/tests/training_consent.rs`
- Test: `crates/legion-retention/tests/privacy_deletion.rs`
- Test: `crates/legion-app/tests/manual_zero_egress.rs`

**Interfaces:**
- Consumes current `FileBackedTelemetrySpool`, `HostedTelemetryExporter`, `RawTraceOptInLedger`, `capture/export/delete_raw_trace_under_opt_in`, file-backed encrypted vault, enterprise policy ceiling.
- Produces proposed: `TrainingConsentService::preview/grant/revoke/export/delete`; each capture binds consent row, source provenance, redaction report, purpose, expiry, and deletion handle.

- [ ] **Step 1: Add failing tests for consent separation, default-off state, exact preview-vs-upload bytes, source/secret redaction, workspace and enterprise denial, revoke-before-upload, retry after network failure, export, deletion/tombstone, expiry purge, corrupt spool recovery, and Manual artifact no-egress.**
- [ ] **Step 2: Run focused tests; expect missing end-to-end consent service.**
- [ ] **Step 3: Implement the app service by composing existing spool/ledger/vault primitives; capture/upload must require current user consent and enterprise permission.** Revocation stops future upload immediately and exposes deletion for retained bundles.
- [ ] **Step 4: Add separate native controls for crash, product, and raw training data; show exact scope/purpose/retention/destination and preview before grant/upload.**
- [ ] **Step 5: Run focused tests, then commit implementation/tests first:** `git add crates/legion-app crates/legion-telemetry crates/legion-retention crates/legion-observability crates/legion-desktop && git commit -m "feat: complete consented telemetry and training controls"`.
- [ ] **Step 6: Build and pin the Step 5 code commit in `candidate.json`; run the native consent/export/delete journey and OS-level Manual/offline no-egress capture. Pass oracle: no network before opt-in, uploaded bytes equal preview, revoked/expired data is not uploaded, export is decryptable by the authorized user, and deletion removes ciphertext/key and emits a tombstone. Validate canonical evidence against the pinned code SHA.**
- [ ] **Step 7: Commit only candidate/evidence records:** `git add plans/completion/candidate.json plans/evidence/completion && git commit -m "evidence: record telemetry training consent acceptance"`.

### S5-13: Run consent-gated QLoRA training and compare the adapter with Legion-Bench

**Dependencies:** `S5-12`; Stage 0 fixed corpus, base-model, hardware, reproducibility, and acceptance thresholds.

**Owner model:** `terra_worker` for harness/data checks, owner-authorized GPU operator for the real run, `sol_reviewer` for provenance and comparison review.

**Outputs:** a current-candidate consented corpus export, reproducible real QLoRA/LoRA adapter run, base-versus-adapter held-out evaluation, Legion-Bench comparison, artifact hashes/provenance, and an explicit accept/reject decision. Historical P9.F4.T3 evidence is supporting context and cannot satisfy this candidate run.

**Files:**
- Modify: `xtask/src/training_corpus.rs`
- Modify: `training/qlora_train.py`
- Modify: `training/eval_adapter.py`
- Modify: `training/test_qlora_train.py`
- Modify: `training/README.md`
- Create: `acceptance/scenarios/stage5/training_adapter.toml`
- Evidence: `plans/evidence/completion/<run-id>/`

**Interfaces:**
- Consumes current `cargo run -p xtask -- training-corpus`, `build_training_candidate_corpus`, `build_training_adapter_dataset`, `build_training_eval_comparison`, S5-12 active consent/export manifest, and existing Legion-Bench recorded/live modes.
- Produces a hash-bound training manifest containing candidate SHA, corpus/dataset/consent-manifest fingerprints, base model/revision, tokenizer, dependency/runtime/GPU versions, full hyperparameters/seed, loss/resource record, adapter artifact SHA-256, holdout result, Legion-Bench baseline/run IDs, and reviewer decision; this is internal evidence linked to the product outcomes the adapter is intended to protect.

- [ ] **Step 1: Add failing tests that reject absent/revoked/expired consent, dataset-manifest count/fingerprint mismatch, train/holdout overlap, changed label/token construction, unpinned base revision, missing seed/hyperparameters, missing adapter hash, and any training result without paired held-out and Legion-Bench comparisons.**
- [ ] **Step 2: Run `cargo test -p xtask --lib training_corpus` and `python -m unittest training.test_qlora_train`; expect only the new manifest/provenance assertions to fail.**
- [ ] **Step 3: Implement the minimal manifest/provenance extensions in the existing Rust/Python pipeline.** Preserve the invariant that `training-corpus` is the only supported input and that the trainer rechecks consent before loading the model.
- [ ] **Step 4: Commit the training/eval code, tests, docs, and driver fixture before creating the candidate:** `git add xtask/src/training_corpus.rs training acceptance/scenarios/stage5/training_adapter.toml && git commit -m "test: harden consented adapter training"`.
- [ ] **Step 5: Build that exact code commit and pin it plus artifact hashes in `candidate.json`. Export the owner-authorized current corpus with `cargo run -p xtask -- training-corpus --expand <approved-count> --out <run-dir>`.** `<approved-count>` and `<run-dir>` are operator values recorded in canonical scenario/configuration records. Secret-scan the export and stop on any non-consented row or fingerprint mismatch.
- [ ] **Step 6: Run `python training/qlora_train.py --model-id <approved-model-revision> --dataset <run-dir>/train.jsonl --consent-manifest <run-dir>/export_manifest.json --output-dir <run-dir>/adapter --max-steps <approved-steps> --batch-size <approved-batch> --sequence-length <approved-length> --learning-rate <approved-rate> --lora-rank <approved-rank> --seed <approved-seed> --device <approved-device>`.** These angle-bracket values come verbatim from S0-02; a dependency/GPU/credential absence records `blocked`, never a successful dry run.
- [ ] **Step 7: Run `python training/eval_adapter.py --model-id <approved-model-revision> --adapter <run-dir>/adapter --dataset <run-dir>/holdout.jsonl --consent-manifest <run-dir>/export_manifest.json --output <run-dir>/adapter_vs_base.json`, then `cargo run -p xtask -- legion-bench --mode recorded` and `cargo run -p xtask -- verify-legion-bench`.** If S0-02 requires live-product tasks for the adapter, run those separately with the approved endpoint/runtime and canonical evidence.
- [ ] **Step 8: Repeat training under the S0-02 reproducibility rule; compare adapter hashes/metrics within the predeclared tolerance.** Do not move thresholds after observing results. A regressed or irreproducible adapter is a valid rejected result and must not be shipped.
- [ ] **Step 9: Write the run and every artifact hash to `plans/evidence/completion/<run-id>/`, link it to canonical scenario/requirement/configuration IDs, obtain independent review, and run `cargo run -p xtask -- verify-completion --candidate <candidate-code-sha>`.**
- [ ] **Step 10: Commit only candidate/evidence records; do not commit large model weights or private raw traces:** `git add plans/completion/candidate.json plans/evidence/completion && git commit -m "evidence: qualify consented adapter training"`.

### S5-14: Package and operate the collaboration/enterprise service

**Dependencies:** `S5-09`, `S5-11`; Stage 6 signing/SBOM inputs may run in parallel but final signed qualification remains Stage 6.

**Owner model:** `sol_engineer` for service/release architecture, `terra_worker` for deterministic packaging scripts, `sol_reviewer` plus operations/security reviewer.

**Outputs:** deployable versioned collaboration/enterprise service artifacts; production configuration and secret references; health/readiness; install/deploy/upgrade/backup/restore/rollback/uninstall procedures; desktop/service protocol compatibility; provenance/SBOM handoff to Stage 6.

**Files:**
- Create: `deploy/collaboration/Dockerfile`
- Create: `deploy/collaboration/config.example.toml`
- Create: `deploy/collaboration/legion-collaboration.service`
- Create: `scripts/package-collaboration-service.ps1`
- Create: `scripts/package-collaboration-service.sh`
- Create: `scripts/operate-collaboration-service.ps1`
- Create: `scripts/operate-collaboration-service.sh`
- Create: `deploy/collaboration/README.md`
- Create: `xtask/src/collaboration_service_package.rs`
- Modify: `xtask/src/main.rs`
- Modify: `docs/OPERATOR_RUNBOOK.md`
- Create: `xtask/tests/collaboration_service_package.rs`
- Create: `acceptance/scenarios/stage5/collaboration_service_operations.toml`
- Create: `.github/workflows/legion-collaboration-service.yml`

**Interfaces:**
- Consumes S5-09 server binary/control-plane store, S5-11 SCIM/policy endpoints, S0-02 service-platform matrix, and `candidate.json` artifact identity.
- Produces **NEW** `cargo run -p xtask -- package-collaboration-service --candidate <candidate-code-sha> --output <dir>` with descriptor fields for code SHA, binary/container digest, protocol/schema versions, supported desktop range, config schema, migrations, backup format, health commands, SBOM/provenance paths, and rollback artifact. The operation scripts expose explicit `preflight|install|deploy|upgrade|backup|restore|rollback|uninstall` verbs against that descriptor; secrets are runtime references, never descriptor or command-line values.

- [ ] **Step 1: Add failing packaging tests for reproducible artifacts, non-root runtime, read-only image/filesystem except declared data/temp paths, dropped capabilities, TLS/issuer/audience requirements, bounded resources, secret-reference-only config, health/readiness separation, exact desktop/service version compatibility, migration ordering, backup format, and rollback descriptor completeness.**
- [ ] **Step 2: Run `cargo test -p xtask --test collaboration_service_package`; expect missing command/artifact failures.**
- [ ] **Step 3: Implement deterministic native/container packaging and validated config loading.** Require explicit production database/storage, TLS, OIDC/SCIM issuer/audience/scope, signing/trust anchors, retention, rate/body/session limits, telemetry consent destination, and secret-provider references; reject insecure defaults in production mode.
- [ ] **Step 4: Implement migration preflight, quiesced consistent backup, integrity-checked restore, blue/green or equivalent health-gated upgrade, rollback to the prior compatible binary/schema, and uninstall that preserves or explicitly deletes retained data according to operator choice.**
- [ ] **Step 5: Add CI build/scan tests without deployment credentials.** Workflow emits unsigned/nonpublished service artifacts, hashes, test results, and SBOM for Stage 6; protected deployment remains owner-authorized and separate.
- [ ] **Step 6: Run packaging tests and a disposable local service drill: install, seed two tenants, backup, upgrade, verify desktop compatibility plus OIDC/SCIM/collaboration, restore, rollback, and uninstall. Pass oracle: hashes/versions match, health gates traffic, tenant data survives approved paths, incompatible clients fail clearly, and secrets never enter artifacts/logs.**
- [ ] **Step 7: Commit service packaging/operations code and driver fixtures first:** `git add deploy/collaboration scripts/package-collaboration-service.ps1 scripts/package-collaboration-service.sh scripts/operate-collaboration-service.ps1 scripts/operate-collaboration-service.sh xtask docs/OPERATOR_RUNBOOK.md acceptance/scenarios/stage5/collaboration_service_operations.toml .github/workflows/legion-collaboration-service.yml && git commit -m "feat: package collaboration enterprise service"`.
- [ ] **Step 8: Build desktop and service artifacts from the pinned Step 7 code SHA, record both hashes/compatibility in `candidate.json`, run the operations drill, and validate canonical evidence against that SHA. Then commit only the records:** `git add plans/completion/candidate.json plans/evidence/completion && git commit -m "evidence: record collaboration service operations"`. **Do not deploy external infrastructure in this package without separate owner authorization.**

### S5-15: Add the Stage 5 full-product acceptance gate

**Dependencies:** `S5-02`, `S5-03`, `S5-04`, `S5-07`, `S5-09`, `S5-10`, `S5-11`, `S5-12`, `S5-13`, `S5-14`.

**Owner model:** `terra_worker` for harness; `luna_worker` for matrix/evidence execution; acceptance: `sol_reviewer`, security reviewer, and non-implementer users.

**Outputs:** immutable candidate evidence for extension, remote, collaboration, identity/policy, telemetry/training, and recovery matrices bound to both desktop and deployable service artifact hashes.

**Files:**
- Create: `xtask/src/stage5_product_acceptance.rs`
- Modify: `xtask/src/main.rs`
- Create: `acceptance/scenarios/stage5/extensions.toml`
- Create: `acceptance/scenarios/stage5/remote.toml`
- Create: `acceptance/scenarios/stage5/collaboration_enterprise.toml`
- Create: `acceptance/scenarios/stage5/telemetry_training.toml`
- Create: `.github/workflows/legion-stage5-live.yml`

**Interfaces:**
- Consumes `candidate.json`, packaged desktop plus collaboration/enterprise service artifacts, Stage 0 matrices, Stage 5 product workflows and real endpoints.
- Produces **NEW collector** `cargo run -p xtask -- stage5-product-acceptance --candidate <candidate-code-sha> --candidate-manifest plans/completion/candidate.json --matrix <path> --evidence <dir>`; it writes the master-schema fields for candidate hash, OS/hardware, dependency observations, native input route, oracle/recovery results, substitutions, defects, owners, reviewer, and review decision. `verify-completion` remains authoritative.

- [ ] **Step 1: Add harness tests that reject metadata-only extension evidence, loopback remote/collaboration, constructed projections, missing real identity/SCIM endpoints, absent opt-in byte comparison/deletion, or incomplete matrices.**
- [ ] **Step 2: Implement NEW command and all scenario assets.** Extensions must execute each required contribution, SSH/container must complete Rust/TS/Python dev loops, two independent participants must converge/review through the packaged service, the real IdP must call the service SCIM API, OIDC/policy must use real endpoints, service install/upgrade/backup/restore/rollback must pass, and consent/export/delete must be independently checked.
- [ ] **Step 3: Include interruption at install/update, extension-host crash, SSH/container disconnect, remote agent mismatch, collaboration server restart, identity/policy revocation, telemetry outage, and deletion retry.**
- [ ] **Step 4: Run `cargo test -p xtask stage5_product_acceptance`, then commit the collector and driver fixtures before pinning the candidate:** `git add xtask acceptance/scenarios/stage5 .github/workflows/legion-stage5-live.yml && git commit -m "test: add extension remote and enterprise harness"`.
- [ ] **Step 5: Build both desktop and service artifacts from that exact code SHA, pin both hashes in `candidate.json`, run the full NEW collector on all required OS/configuration rows, then run `cargo run -p xtask -- verify-completion --candidate <candidate-code-sha>`. Pass oracle: every row is accepted or truthfully failed/blocked, no substitution satisfies a required outcome, and all external effects and cleanup are independently verified.**
- [ ] **Step 6: Have two non-implementers complete unfamiliar extension/remote/team tasks from published docs without coaching; any undocumented workaround fails the journey.**
- [ ] **Step 7: Commit only candidate/evidence records:** `git add plans/completion/candidate.json plans/evidence/completion && git commit -m "evidence: qualify extensions remote and enterprise workflows"`.

## Stage exit and integration checklist

- [ ] Stage 3 closes only when S3-07 passes the entire ratified local/hosted model matrix for Rust/TS/Python and safety/recovery invariants; provider unavailability is visible and never replaced by deterministic success.
- [ ] Stage 4 closes only when S4-06 proves a real multi-worker task plus named MCP and ACP peers through the packaged app, including conflict, interruption, revocation, restart, and cleanup.
- [ ] Stage 5 closes only when S5-15 proves actual required WASM/VS Code/Tier-3 capabilities, SSH/container workflows, a real remote agent and deployable collaboration/enterprise service plus concurrent collaboration, real IdP-to-SCIM provisioning, SSO/policy, consent/export/deletion, and the current consent-gated adapter training/evaluation result across the ratified matrix.
- [ ] After each package, run its focused tests, `cargo fmt --all --check`, `cargo check --workspace --all-targets`, and the standing gates affected by its claims. Do not repeat the entire 21-gate suite after every isolated package unless repository policy requires it; run the full suite at each stage exit.
- [ ] At each stage exit, run `cargo test --workspace --all-targets -j 1` on Windows if parallel rustc artifacts race, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo deny check`, all 21 standing phase gates, and the stage's NEW product-acceptance command.
- [ ] Update the authoritative completion register with separate implementation and acceptance states, immutable evidence hashes, matrix coverage, failures, blockers, and reviewer; never convert a green component test into layer-3 acceptance.
- [ ] Hand Stage 6 one `candidate.json` that pins the same code SHA and both desktop and collaboration/enterprise service artifact hashes; final platform/security/release qualification must verify their protocol compatibility, signatures/provenance, installation, upgrade, backup/restore, rollback, diagnostics, and clean-machine behavior together.
- [ ] Run `cargo run -p xtask -- docs-hygiene`, `cargo run -p xtask -- claim-audit`, and `cargo run -p xtask -- verify-readiness-consistency` after documentation/evidence reconciliation.
- [ ] A `sol_reviewer` independently reviews every architecture/security/persistence/cross-platform package and the integrated diff; the root Astra coordinator owns final scope, integration, contradiction resolution, and Stage 6 handoff.

## Self-review against the approved spec

- Stage 3 covers local/hosted setup, managed model runtime, hardware fit, context/retrieval/provenance/privacy, Assist, Delegate, proposals, cancellation, rollback/recovery, and real-model evaluation.
- Stage 4 covers editable plans, dependency scheduling, concurrent workers, budgets, approvals, conflict handling, interruption/replay, fleet controls, MCP client/server roles as required, ACP real-peer interoperability, and proposal-mediated results.
- Stage 5 covers signed WASM execution, required VS Code Node/web-worker compatibility, actual webviews/notebooks/custom editors/storage, SSH/container remote files/tools/terminal/LSP/debugging, CRDT collaboration/shared proposals, identity/SSO/SCIM/policy/audit/retention, explicit opt-in telemetry/training export/deletion, and a real consent-gated QLoRA/adapter run with held-out and Legion-Bench comparison.
- Every architecture-sensitive expansion starts with a bounded contract/ADR and tests before implementation.
- Every product family ends in a packaged-app scenario with real dependencies and externally verified effects; fixture-only evidence is rejected by the proposed stage gates.
- No feature family assigned to Stages 3–5 is deferred to Stage 6; Stage 6 receives completed implementations and reruns qualification.

Plan complete and saved to `docs/superpowers/plans/2026-09-04-ai-team-completion.md`. Execute it only after the root coordinator integrates it with the Stage 0–2 and Stage 6 plans and verifies cross-stage IDs, shared file ownership, and completion-register mappings.
