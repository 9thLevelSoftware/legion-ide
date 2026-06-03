# Legion E2E Design, Development, and Implementation Plan

> For Hermes: execute this with `subagent-driven-development`; dispatch fresh implementation subagents per task, then run spec-compliance and quality reviewers. Kimi 2.6 implementer prompts must include the exact task text, exact files, exact commands, and exact pass criteria from this document.

Goal: turn the current `legion-ide` repository into the Legion IDE product described by the five planning documents in `plans/legion-e2e/source-package/`, with complete local-first deterministic IDE foundations, Assist, Delegate, Automate, Cloud Lane, and model-training/data-flywheel features.

Architecture: Legion remains local-first and proposal-gated. UI crates are projection-only; app/runtime crates own workflow execution; workspace/file mutation remains proposal-mediated; AI and worker lanes produce evidence and proposals only; cloud and training lanes are opt-in, consent-gated, and policy-enforced.

Branding rule: use Legion in all user-facing docs, panels, modes, product strings, and plans. Keep internal `legion-*` crate names until the rename phase has full dependency-policy, CI, and migration coverage.

Non-negotiable constraints:
- No stub features may be marked complete. A feature is complete only when its code path is implemented, tests cover it, docs describe it, and the gate command passes.
- Manual mode must exclude AI panels, AI commands, network providers, cloud routes, hosted telemetry, and worker automation.
- AI/provider/agent code must never mutate the main workspace directly. It may create proposals, evidence, and metadata-only audit records.
- Any raw trace/diff/model-output retention for training requires explicit opt-in consent, redaction, and export controls.
- Autonomous merge/apply remains unsupported until explicit user approval and merge-readiness gates pass.

Source package traceability:

## Source: `00_INDEX.md`
- # Legion IDE planning package

## Source: `01_FRONTEND_APP_ARCHITECTURE_PLAN.md`
- # 01 — Legion IDE Front-End App Architecture Plan
- ## 0. Executive summary
- ## 1. Current known repo state
- ## 2. Front-end architecture principles
- ### 2.1 Projection-only UI
- ### 2.2 Shared registry, filtered by mode
- ### 2.3 Mode-specific layouts
- ### 2.4 Deterministic-first UX
- ## 3. Dock system design
- ### 3.1 Core types
- ### 3.2 Panel filtering rules
- ### 3.3 Dock layout persistence
- ### 3.4 Dock rendering approach
- ### 3.5 Front-end test requirements
- ## 4. Panel catalog
- ### 4.1 Manual-eligible panels
- ### 4.2 Assist panels
- ### 4.3 Delegate panels
- ### 4.4 Automate panels
- ## 5. Proposal/evidence UX
- ### 5.1 Proposal review surface
- ### 5.2 Review states
- ### 5.3 Per-hunk controls
- ### 5.4 Evidence-first display
- ## 6. Legion cloud UX
- ## 7. Front-end rename plan
- ### 7.1 Immediate user-facing rename
- ### 7.2 Controlled internal rename
- ## 8. Front-end implementation sequence
- ### Phase FE-1: Panel registry foundation
- ### Phase FE-2: Dock host rendering
- ### Phase FE-3: Manual deterministic panels
- ### Phase FE-4: Structural tools panels
- ### Phase FE-5: VCS/debug/test panels
- ### Phase FE-6: Assist UI
- ### Phase FE-7: Delegate UI
- ### Phase FE-8: Automate UI
- ## 9. Front-end quality gates
- ## 10. Front-end product language
- ## 11. Front-end immediate next actions

## Source: `02_BACKEND_APP_ARCHITECTURE_PLAN.md`
- # 02 — Legion IDE Back-End / Local Runtime Architecture Plan
- ## 0. Executive summary
- ## 1. Existing crate responsibilities and target responsibilities
- ### 1.1 `legion-app` / future `legion-app`
- ### 1.2 `legion-ui` / future `legion-ui`
- ### 1.3 `legion-desktop` / future `legion-desktop`
- ### 1.4 `legion-protocol` / future `legion-protocol`
- ### 1.5 `legion-agent` / future `legion-agent`
- ### 1.6 `legion-ai` / future `legion-ai`
- ### 1.7 `legion-ai-providers` / future `legion-ai-providers`
- ### 1.8 `legion-index` / future `legion-index`
- ### 1.9 `legion-terminal` / future `legion-terminal`
- ### 1.10 `legion-security` / future `legion-security`
- ### 1.11 `legion-memory` / future `legion-memory`
- ### 1.12 `legion-tracker` / future `legion-tracker`
- ## 2. Core back-end data model
- ### 2.1 Workflow
- ### 2.2 Task graph
- ### 2.3 Task packet
- ### 2.4 Worker assignment
- ### 2.5 Worker result
- ### 2.6 Proposal envelope
- ### 2.7 Evidence record
- ## 3. Legion worker lifecycle
- ### 3.1 Lifecycle states
- ### 3.2 Disposable worker semantics
- ### 3.3 Sandbox preparation
- ### 3.4 Containment validation
- ### 3.5 Proposal-only output
- ## 4. Provider route architecture
- ### 4.1 Provider interface
- ### 4.2 Provider types
- ### 4.3 Model route request
- ### 4.4 Routing policy
- ## 5. Validation architecture
- ### 5.1 Validation plan
- ### 5.2 Validation rules
- ### 5.3 Validation lane separation
- ## 6. Conflict detection architecture
- ### 6.1 File-level conflict detection
- ### 6.2 Hunk-level conflict detection
- ### 6.3 Semantic conflict detection
- ### 6.4 Merge readiness
- ## 7. LSP/DAP/index runtime
- ### 7.1 LSP
- ### 7.2 DAP
- ### 7.3 Index
- ## 8. Cloud integration back-end API
- ## 9. Security architecture
- ### 9.1 Policy layers
- ### 9.2 Risk flags
- ### 9.3 Secret scanning
- ## 10. Back-end implementation sequence
- ### BE-1: Protocol DTO foundation
- ### BE-2: Delegated sandbox hardening
- ### BE-3: Provider adapters
- ### BE-4: Validation runner
- ### BE-5: Legion workflow coordinator
- ### BE-6: Cloud API bridge
- ### BE-7: Full Automate integration
- ## 11. Back-end immediate next actions

## Source: `03_CLOUD_OFFERING_ARCHITECTURE_PLAN.md`
- # 03 — Legion Cloud Offering Architecture and Provider Plan
- ## 0. Executive summary
- ## 1. Verified provider research summary
- ### 1.1 RunPod
- ### 1.2 Fly.io
- ### 1.3 Modal
- ### 1.4 Vast.ai
- ### 1.5 Lambda Labs / Lambda AI
- ## 2. Product packaging
- ### 2.1 Product tiers
- #### Tier 0: Legion Local
- #### Tier 1: Legion Cloud Lane
- #### Tier 2: Legion Cloud Team
- #### Tier 3: Legion Cloud Forge
- ## 3. What to sell
- ## 4. Cloud architecture overview
- ## 5. Control plane architecture
- ### 5.1 Responsibilities
- ### 5.2 Suggested stack
- ## 6. Worker plane architecture
- ### 6.1 Worker lane types
- #### AI worker lane
- #### Validation lane
- #### Index lane
- #### Training lane
- ### 6.2 Worker lane lifecycle
- ### 6.3 Warm pools
- ## 7. Sandbox and repo cache architecture
- ### 7.1 Repo access modes
- ### 7.2 Recommended MVP
- ### 7.3 Sandbox implementation
- ## 8. Model pool architecture
- ### 8.1 Small specialist pool
- ### 8.2 Medium specialist pool
- ### 8.3 Remote escalation pool
- ### 8.4 Serving stack
- ## 9. Provider recommendations by phase
- ### Phase Cloud-0: Internal experiments
- ### Phase Cloud-1: MVP hosted lanes
- ### Phase Cloud-2: Production beta
- ### Phase Cloud-3: Enterprise
- ## 10. Cost control plan
- ### 10.1 Hard limits
- ### 10.2 Queue controls
- ### 10.3 Cost estimate before cloud use
- ### 10.4 Billing events
- ## 11. Security and privacy
- ### 11.1 Upload scope visibility
- ### 11.2 Forbidden by default
- ### 11.3 Cloud worker permissions
- ### 11.4 Audit
- ## 12. Cloud API sketch
- ### 12.1 Submit task
- ### 12.2 Get task status
- ### 12.3 Stream events
- ### 12.4 Cancel task
- ### 12.5 Fetch proposal
- ### 12.6 Fetch evidence
- ## 13. Cloud MVP implementation sequence
- ### Cloud-1: Control plane skeleton
- ### Cloud-2: CPU validation lane
- ### Cloud-3: Small model worker lane
- ### Cloud-4: Scheduler
- ### Cloud-5: Repo cache
- ### Cloud-6: Production hardening
- ## 14. Suggested initial pricing experiments
- ### Free/Local
- ### Cloud Lane
- ### Cloud Team
- ### Cloud Forge
- ## 15. Main cloud risks
- ## 16. Immediate cloud next actions

## Source: `04_PRODUCT_IMPLEMENTATION_ROADMAP.md`
- # 04 — Legion Product Design, Development, and Implementation Roadmap
- ## 0. Executive summary
- ## 1. What has already been built / found
- ## 2. Product pivot: Legion → Legion
- ### 2.1 Why Legion is a better name
- ### 2.2 Rename strategy
- ### 2.3 Product vocabulary
- ## 3. Product modes
- ### 3.1 Manual
- ### 3.2 Assist
- ### 3.3 Delegate
- ### 3.4 Automate
- ## 4. Development phases
- ## Phase 0 — Stabilize repo and rename surface
- ### Goals
- ### Tasks
- ### Exit criteria
- ## Phase 1 — Panel-host dock refactor
- ### Goals
- ### Tasks
- ### Exit criteria
- ## Phase 2 — Manual deterministic IDE foundation
- ### Goals
- ### Tasks
- ### Exit criteria
- ## Phase 3 — Structural Search & Replace
- ### Goals
- ### Tasks
- ### Exit criteria
- ## Phase 4 — VCS, syntactic diff, blame, and history
- ### Goals
- ### Tasks
- ### Exit criteria
- ## Phase 5 — DAP debugger and test runner
- ### Goals
- ### Tasks
- ### Exit criteria
- ## Phase 6 — Assist mode
- ### Goals
- ### Tasks
- ### Exit criteria
- ## Phase 7 — Delegate mode
- ### Goals
- ### Tasks
- ### Exit criteria
- ## Phase 8 — Automate / Legion workflow orchestration
- ### Goals
- ### Tasks
- ### Exit criteria
- ## Phase 9 — Legion Cloud MVP
- ### Goals
- ### Tasks
- ### Exit criteria
- ## Phase 10 — Trace collection and specialist training
- ### Goals
- ### Tasks
- ### Exit criteria
- ## 5. MVP cut lines
- ### MVP A: Deterministic Legion
- ### MVP B: Assist/Delegate Legion
- ### MVP C: Legion Automate Local
- ### MVP D: Legion Cloud Lane
- ### MVP E: Fine-tuned Legion Specialists
- ## 6. Development team lanes
- ### Lane 1: UI/Dock
- ### Lane 2: Protocol/Runtime
- ### Lane 3: Agent/Workflow
- ### Lane 4: AI/Providers
- ### Lane 5: Cloud
- ### Lane 6: Model Training
- ## 7. Quality gates
- ### Manual gate
- ### Proposal gate
- ### Cloud gate
- ### Automate gate
- ### Training gate
- ## 8. Immediate implementation checklist
- ## 9. Decision points
- ### Decision 1: Internal crate rename timing
- ### Decision 2: First cloud provider

## Source: `05_MODEL_ACQUISITION_AND_TRAINING_PLAN.md`
- # 05 — Legion Model Acquisition, Training, Evaluation, and Serving Plan
- ## 0. Executive summary
- ## 1. Verified model metadata
- ### 1.1 Qwen2.5-Coder-1.5B-Instruct
- ### 1.2 Qwen2.5-Coder-3B-Instruct
- ### 1.3 Qwen2.5-Coder-7B-Instruct
- ### 1.4 Qwen2.5-Coder-14B-Instruct
- ### 1.5 StarCoder2-3B
- ### 1.6 DeepSeek-Coder-V2-Lite-Instruct
- ## 2. Model acquisition plan
- ### Step 1: Create local model directory
- ### Step 2: Install model tooling
- ### Step 3: Install Hugging Face CLI
- ### Step 4: Download base models
- ### Step 5: Acquire GGUFs for inference
- ## 3. First specialist roster
- ### 3.1 Specialist 1: Legion Docs Summarizer 1.5B
- ### 3.2 Specialist 2: Legion Rust Compiler Fixer 3B
- ### 3.3 Specialist 3: Legion Test Writer 3B
- ### 3.4 Specialist 4: Legion Reviewer 7B
- ## 4. Data collection plan
- ### 4.1 Trace schema
- ### 4.2 What to include
- ### 4.3 Consent model
- ## 5. Dataset construction
- ### 5.1 Start with synthetic/teacher data
- ### 5.2 Use open-source repos
- ### 5.3 Dataset splits
- ### 5.4 Minimum useful dataset sizes
- ## 6. Training approach
- ### 6.1 Use QLoRA first
- ### 6.2 Sequence lengths
- ### 6.3 Local vs cloud training
- ## 7. Training tools
- ### 7.1 Unsloth
- ### 7.2 Axolotl
- ### 7.3 TRL
- ### 7.4 llama.cpp
- ### 7.5 vLLM
- ### 7.6 Evaluation tools
- ## 8. Baby-step training plan
- ## Stage 1 — Environment verification
- ## Stage 2 — Docs summarizer specialist
- ## Stage 3 — Rust compiler fixer specialist
- ## Stage 4 — Test writer specialist
- ## Stage 5 — Reviewer specialist
- ## Stage 6 — Preference tuning
- ## 9. Evaluation harness
- ### 9.1 Eval types
- ### 9.2 Eval command sketch
- ### 9.3 Metrics
- ## 10. Serving plan
- ### 10.1 Local serving with llama.cpp
- ### 10.2 Local worker config
- ### 10.3 Cloud serving
- ## 11. Logistical plan
- ### 11.1 What to do this week
- ### 11.2 What to do next month
- ### 11.3 What to do after beta
- ## 12. Recommended exact acquisition order
- ## 13. Risks
- ## 14. Final recommendation


# Current repository baseline

Verified from the cloned repo on this branch:
- Workspace contains 26 Rust crates under `crates/` plus `xtask`.
- Existing core gates in `AGENTS.md`: `cargo run -p xtask -- check-deps`; `cargo fmt --all --check`; `cargo check --workspace --all-targets`; `cargo test --workspace --all-targets`; `cargo clippy --workspace --all-targets -- -D warnings`; CI additionally runs `cargo deny check`.
- Existing implemented areas include editor/text/project/storage/security/protocol/app/workflow/proposal foundations, metadata-first Legion workflow orchestration, deterministic provider scaffolding, tracker/memory metadata ledgers, terminal/debug/search/git tests, and projection-only desktop/UI surfaces.
- The current execution host did not initially have `cargo` or `rustc` on PATH; install/verification must be completed before code gates can be claimed locally.

# Completion definition

The total goal is complete only when all phases below are complete and the final PR contains:
1. Passing dependency policy, format, check, test, clippy, and deny gates.
2. Updated product docs and operator runbooks.
3. Contract tests for every new DTO and policy boundary.
4. Integration tests for every mode transition and feature surface.
5. Evidence files under `plans/evidence/legion-e2e/` with exact command outputs.
6. A PR body listing phase completion, test evidence, residual risks, and explicit unsupported items. Unsupported items may only be external service availability (for example no GPU present), not missing product code.

# Phase 0 — Repository stabilization and Legion product surface

Objective: make the repo buildable, documented, and consistently Legion-branded at the product surface without a risky internal crate rename.

Tasks:
0.1 Create/refresh `docs/LEGION_PIVOT.md`, `docs/MODES.md`, `docs/ARCHITECTURE_AUTHORITY_BOUNDARIES.md`, and `docs/OPERATOR_RUNBOOK.md`.
0.2 Add `plans/legion-e2e/source-package/` with the five source documents for traceability.
0.3 Add `plans/evidence/legion-e2e/README.md` explaining where command outputs and gate evidence live.
0.4 Verify all workspace crates referenced by `Cargo.toml` exist and are included in dependency policy.
0.5 Add/verify `rust-toolchain.toml` if the repo requires a pinned Rust toolchain; otherwise document the minimum supported Rust version.
0.6 Run all phase gates and capture outputs.

Kimi implementation prompt:
- Read `AGENTS.md`, `Cargo.toml`, `plans/dependency-policy.md`, and this Phase 0 section.
- Do not rename internal crates.
- Only edit docs and evidence scaffolding unless the build fails because of missing toolchain metadata.
- Run: `cargo run -p xtask -- check-deps`, `cargo fmt --all --check`, `cargo check --workspace --all-targets`.
- If a command fails, save the exact output in `plans/evidence/legion-e2e/phase-0-<gate>.txt` and fix root cause before proceeding.

Exit criteria:
- Product docs exist and use Legion branding.
- Source package is committed.
- Build/check gates pass or a toolchain blocker is explicitly resolved.

# Phase 1 — Dock/panel registry and mode shell

Objective: implement a complete mode-aware projection shell for Manual, Assist, Delegate, and Automate.

Features:
- Canonical panel registry with panel IDs, capabilities, authority requirements, network/AI requirements, mode eligibility, and default placement.
- Mode-scoped dock layouts persisted independently.
- Manual layout filter hard-excludes AI/cloud/worker panels.
- Complete app chrome: mode selector, workspace/project selector, trust badge, search, settings, command palette, mode status bar.
- Right console with severity, retry/configure/dismiss actions.

Granular tasks:
1.1 In `crates/legion-ui/src/ui.rs`, audit `PanelId`, `PanelRegistry`, `PanelCapability`, `DockMode`, and dock layout tests. Add missing Assist/Delegate/Automate panel IDs only if absent.
1.2 Add/extend tests proving Manual never shows panels requiring AI, network, cloud, workers, or hosted telemetry.
1.3 In `crates/legion-desktop/src/view.rs`, implement top chrome/status/right-console rendering as projection-only state. Do not own editor/session text.
1.4 Add desktop projection tests for all mode layouts and chrome actions.

Gate commands:
- `cargo test -p legion-ui --all-targets`
- `cargo test -p legion-desktop --all-targets`
- `cargo check --workspace --all-targets`

Exit criteria:
- All planned panels are registered, filtered, rendered or intentionally hidden by mode.
- Manual mode AI exclusion is enforced by code and tests, not convention.

# Phase 2 — Manual deterministic IDE foundation

Objective: complete the fast deterministic IDE path independent of AI.

Features:
- File tree, tabs, editor viewport, search, structural search, symbols, problems, terminal, git status/diff/blame/history, debug/test explorer, command palette.
- Save workflow remains proposal-mediated with fingerprint/content-version/workspace-generation/buffer-version/snapshot preconditions.
- Offline/air-gap policy profile with visible status and fail-closed provider/network/telemetry routing.

Granular tasks:
2.1 Audit existing app/editor/project/storage/platform tests for manual editing, save conflicts, structural search, git, terminal, language tooling, and debug workflows.
2.2 Fill incomplete deterministic panels in UI/desktop with projection-only DTOs from app/runtime.
2.3 Add air-gap/offline policy tests in app/security/provider routing.
2.4 Add performance/regression tests for save and viewport not blocking on semantic/LSP/AI/plugin/remote/collab consumers.

Gate commands:
- `cargo test -p legion-app --test daily_editing_contracts --all-targets`
- `cargo test -p legion-app --test workspace_vfs_integration --all-targets`
- `cargo test -p legion-app --test structural_search_workflow --all-targets`
- `cargo test -p legion-app --test git_workflow --all-targets`
- `cargo test -p legion-app --test terminal_workflow --all-targets`
- `cargo test -p legion-app --test debug_workflow --all-targets`

Exit criteria:
- A user can perform daily editing, save, search, git, terminal, test/debug, and navigation workflows without any AI provider.

# Phase 3 — Protocol DTO foundation for workers, evidence, provider routing, and training

Objective: add the canonical DTOs missing from the source planning package while preserving metadata-only defaults.

Features:
- `LegionTaskPacket` / `TaskPacket` with scoped allowed files, forbidden files, context snippet refs, full-file refs, command output refs, output contract, validation plan, stop conditions, policy, correlation/causality.
- `LegionWorkerResult` with patch proposal, documentation proposal, analysis, test plan, blocked, invalid variants.
- `LegionEvidenceRecord` with evidence kind/source, payload hash, redacted payload summary, command/status metadata, and privacy scope.
- Rich provider route metadata: locality preference, cost budget, latency budget, privacy policy, model capability, provider class, and route health.
- Contract validators rejecting zero correlation, nil causality, raw secret markers, missing policy, and direct mutation intent.

Granular tasks:
3.1 Add DTOs in `crates/legion-protocol/src/lib.rs` near existing assisted-AI route and Legion workflow DTOs.
3.2 Add validators beside existing `validate_assisted_ai_*` helpers.
3.3 Add serde roundtrip and negative contract tests in `crates/legion-protocol/tests/dto_contracts.rs`.
3.4 Update dependent app/agent/provider code to use richer route metadata only where required; do not break existing route constructors.

Gate commands:
- `cargo test -p legion-protocol --test dto_contracts legion -- --nocapture`
- `cargo test -p legion-agent --all-targets`
- `cargo test -p legion-app --test legion_workflow_integration --all-targets`

Exit criteria:
- DTOs from backend/cloud/model plans exist, serialize/deserialize, validate, and are used at app/agent boundaries.

# Phase 4 — Assist mode

Objective: complete human-in-control AI assistance.

Features:
- Inline prediction lifecycle: request, cancel, stale, available, accept, dismiss, audit.
- Assistant right rail with context citations, provider route status, privacy inspector, proposal-only output.
- Inline diff/hunk proposal UI with per-hunk accept/reject and evidence-first display.
- Provider selection with local/offline default, BYOK cloud providers behind policy, and hosted providers disabled in Manual/offline.

Granular tasks:
4.1 Complete provider route UI projections and provider health/cost labels.
4.2 Implement local loopback OpenAI-compatible route profiles for llama.cpp/Ollama using existing generic provider infrastructure, with clear endpoint config.
4.3 Add Fireworks/Kimi route as BYOK/cloud provider if dependency policy permits; otherwise add config metadata and provider adapter tests with mock transport.
4.4 Complete app-level Assist APIs and desktop panels.
4.5 Test provider refusals, cancellation, privacy policy denial, proposal-only output, and Manual mode exclusion.

Gate commands:
- `cargo test -p legion-app --test assist_inline_prediction_workflow --all-targets`
- `cargo test -p legion-ai --all-targets`
- `cargo test -p legion-ai-providers --all-targets`
- `cargo test -p legion-desktop --all-targets assist`

Exit criteria:
- Assist can produce preview/proposal/evidence flows through configured providers without direct mutation.

# Phase 5 — Delegate mode

Objective: execute bounded, capability-scoped worker tasks in disposable lanes.

Features:
- Task decomposition into small task packets with allowed/forbidden file scopes.
- Worker lifecycle: pending, preparing sandbox, running, blocked, proposal-ready, validating, review-ready, done, failed, cancelled.
- Sandbox containment: git worktree/copy fallback, path allowlist, no main workspace mutation.
- Fleet console, task bar, delegate chat, proposal queue, risk monitor, decision feed.
- Tool permission requests routed through policy broker and explicit user approvals.

Granular tasks:
5.1 Extend/verify `legion-agent` sandbox orchestration, containment validation, and proposal generator.
5.2 Add `legion-app` delegate session APIs for task packet submission, status polling, cancellation, and proposal review.
5.3 Add UI panels and desktop rendering for Fleet Console, Task Bar, Delegate Chat, Proposal Queue, Risk Monitor, Decision Feed.
5.4 Add tests for allowed/forbidden file boundaries, stale proposal rejection, approval denial, permission request denial, and evidence display.

Gate commands:
- `cargo test -p legion-agent --all-targets delegated legion`
- `cargo test -p legion-app --test delegated_task_integration --all-targets`
- `cargo test -p legion-desktop --all-targets delegate`

Exit criteria:
- Delegate mode can run a bounded task to a reviewed proposal with full evidence and no direct mutation.

# Phase 6 — Automate / Legion workflow orchestration

Objective: support multi-step Legion workflows with deterministic gates and human authority.

Features:
- Workflow builder/runbook/trigger panels.
- Task graph dependencies, worker assignments, validation gates, conflict gates, sign-off, merge-readiness.
- Automate run lifecycle with kill switch, risk escalations, budget limits, validation lane separation, final approval gate.
- Durable tracker/memory metadata records and replayable evidence.

Granular tasks:
6.1 Verify ADR-0031 and dependency policy authorize every activation step.
6.2 Complete `LegionWorkflowCoordinator` with task graph scheduling, validation gate waiting, cancellation, and blocked states.
6.3 Complete app-owned workflow execution and merge-readiness routing.
6.4 Complete Automate panels in UI/desktop.
6.5 Add integration tests for successful run, blocked run, failed validation, conflict, cancellation, signoff, and merge approval.

Gate commands:
- `cargo test -p legion-app --test legion_workflow_integration --all-targets`
- `cargo test -p legion-desktop --test legion_workflow_command_center --all-targets`
- `cargo test -p legion-protocol --test dto_contracts legion_workflow --all-targets`
- `cargo test -p legion-tracker --all-targets legion_workflow`
- `cargo test -p legion-memory --all-targets legion_workflow`

Exit criteria:
- Automate mode can run an end-to-end local workflow to review-ready proposals with validation evidence and final human gate.

# Phase 7 — Cloud Lane MVP

Objective: implement hosted worker-lane integration without weakening local authority.

Features:
- Cloud API bridge: submit task, status, stream events, cancel, fetch proposal, fetch evidence.
- Local cloud-lane client with signed task packets, upload scope visibility, cost estimate, hard budget caps, queue limits, and cancellation.
- Control plane skeleton and worker lane contracts in repo if this repo owns cloud code; otherwise explicit client contracts and integration fixtures.
- Security: no secrets, `.env`, raw credentials, private key material, or forbidden files uploaded by default.

Granular tasks:
7.1 Add cloud DTOs/client contracts to protocol/app with mock transport tests.
7.2 Add cloud lane config and policy enforcement in security/app.
7.3 Add status/event/proposal/evidence projection panels.
7.4 Add cancellation and budget tests.
7.5 Add deployment docs/runbooks and local mock server test fixture.

Gate commands:
- `cargo test -p legion-remote --all-targets`
- `cargo test -p legion-app --all-targets cloud`
- `cargo test -p legion-security --all-targets cloud`

Exit criteria:
- Local app can submit to a mock cloud lane, enforce policy/budget, stream status, cancel, and fetch proposal/evidence without uploading forbidden data.

# Phase 8 — Trace collection and specialist model flywheel

Objective: make model acquisition, training, eval, quantization, and serving reproducible and consent-gated.

Features:
- Model acquisition scripts for Qwen2.5-Coder 1.5B/3B/7B and StarCoder2-3B, with dry-run mode and checksum/metadata recording.
- Local serving scripts for llama.cpp/OpenAI-compatible endpoints.
- Training environment scripts for Unsloth/Axolotl/TRL with RTX 5070/Blackwell caveats documented.
- Trace schema with explicit consent, redaction, secret scanning, payload hashing, JSONL export, and delete controls.
- Specialist schemas/evals: docs summarizer, Rust compiler fixer, test writer, reviewer.
- Eval harness: schema compliance, patch apply, compile/test pass, regression rate, latency, cost, refusal rate.

Granular tasks:
8.1 Add `scripts/models/download-models.sh` with `--dry-run` and exact model IDs.
8.2 Add `scripts/models/start-local-workers.sh` and `config/workers.example.yaml`.
8.3 Add `training/` Python project files, QLoRA scripts, eval scripts, and conversion scripts.
8.4 Add `crates/legion-memory` or dedicated crate trace schemas with consent and redaction validators.
8.5 Add `crates/legion-security` secret scanner tests for traces/diffs/logs.
8.6 Add docs for dataset construction and eval gates.

Gate commands:
- `bash scripts/models/download-models.sh --dry-run`
- `bash scripts/models/start-local-workers.sh --dry-run --config config/workers.example.yaml`
- `python3 evals/run_eval.py --dry-run`
- `python3 -m compileall training evals scripts/models`
- `cargo test -p legion-memory --all-targets trace`
- `cargo test -p legion-security --all-targets redaction`

Exit criteria:
- The repo can reproduce acquisition/training/eval/serving workflows in dry-run mode in CI and real mode on an appropriately provisioned machine.

# Phase 9 — End-to-end hardening, accessibility, packaging, and release

Objective: make Legion usable as a complete product.

Features:
- Accessibility smoke coverage, keyboard navigation, high contrast, focus order, screen reader labels.
- Packaging and platform integration for supported OSes.
- Crash diagnostics, health checks, issue export bundles with redaction.
- CI matrix and product smoke fixtures.
- Documentation: user guide, admin/security guide, developer guide, cloud guide, training guide.

Gate commands:
- All phase gate commands.
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo deny check`
- product smoke test commands captured in evidence.

Exit criteria:
- PR is ready for review as a complete implementation line, with evidence and no unimplemented planned features in the agreed scope.

# Subagent coordination protocol

Use these rules for every implementation task:
1. Give Kimi one task only. Include objective, exact files, exact tests, and exact pass criteria.
2. Tell it to read only the needed files and `AGENTS.md`; do not make it digest all 4,481 source-plan lines.
3. Require TDD: add failing test, run it, implement, rerun target test, rerun affected package test.
4. Require no direct workspace mutation from AI/agent/runtime code; proposals only.
5. After implementer returns, run a spec-compliance reviewer with the task text and changed files.
6. After spec passes, run a quality/security reviewer.
7. Only then commit the task with a conventional commit message.
8. Save command outputs under `plans/evidence/legion-e2e/phase-N-*` for any feature gate.

# PR checklist

- [ ] Phase 0 complete.
- [ ] Phase 1 complete.
- [ ] Phase 2 complete.
- [ ] Phase 3 complete.
- [ ] Phase 4 complete.
- [ ] Phase 5 complete.
- [ ] Phase 6 complete.
- [ ] Phase 7 complete.
- [ ] Phase 8 complete.
- [ ] Phase 9 complete.
- [ ] Full workspace gates pass.
- [ ] PR body includes evidence links and risk assessment.
