# 04 — Legion Product Design, Development, and Implementation Roadmap

Generated: 2026-06-01 16:24:53 EDT

## 0. Executive summary

Legion should be built in this order:

1. Rename user-facing product from Devil to Legion without destabilizing internal crates.
2. Build the panel-host dock refactor first.
3. Ship deterministic Manual-mode IDE value.
4. Add structural search/rewrite as a differentiator.
5. Add VCS/debug/test capabilities to become a real IDE.
6. Add Assist mode.
7. Add Delegate mode with proposal approval.
8. Add Automate mode with Legion worker lanes.
9. Add Legion Cloud lanes.
10. Collect accepted traces and train specialists.

Do not start with AI swarm orchestration. Start with the dock registry and deterministic foundations. The AI assembly line only works if the IDE has a reliable substrate: panels, protocols, sandboxing, validation, proposals, and evidence.

## 1. What has already been built / found

From repo inspection and the Discord artifact:

- Rust workspace exists.
- Existing crates include:
  - `devil-agent`.
  - `devil-ai`.
  - `devil-ai-providers`.
  - `devil-app`.
  - `devil-ui`.
  - `devil-desktop`.
  - `devil-protocol`.
  - likely supporting crates for index/project/storage/etc.
- Phase 12 delegated task runtime exists/planned.
- `devil-agent` has concepts such as:
  - `AgentRuntime`.
  - `DelegatedTaskSandboxOrchestrator`.
  - `DelegatedTaskProposalGenerator`.
  - containment validation.
- Delegated task runtime uses git worktree isolation where possible.
- Copy-based isolation fallback exists/planned.
- Phase 13 Legion workflow orchestration exists/planned.
- `devil-ai-providers` has stub provider registry/deterministic local provider.
- `devil-ui`/`devil-desktop` are supposed to be projection-only.
- `devil-app` owns proposal-mediated saves and approval gates.
- Current UI likely uses whole-pane `RightConsole` swapping by mode.
- Dock/panel registry refactor is identified as gating.

## 2. Product pivot: Devil → Legion

### 2.1 Why Legion is a better name

Legion fits:

- many workers.
- coordinated force.
- named fleet.
- assembly-line automation.
- cloud worker lanes.
- local specialist army.

It is also less edgy than Devil and easier to sell to professional/enterprise users.

### 2.2 Rename strategy

Do not rename every crate immediately.

Immediate rename:

- app name.
- website/docs.
- UI strings.
- window title.
- icons.
- command palette labels.
- mode copy.
- plan filenames.
- product concepts.

Deferred rename:

- Rust crate names.
- internal module paths.
- repository name.
- package IDs.

Reason:

- crate rename creates huge churn.
- architecture is more important.
- user-facing rename captures product pivot now.

### 2.3 Product vocabulary

Use:

- Legion IDE.
- Legion Board.
- Worker Lane.
- Specialist.
- Fleet Console.
- Decision Feed.
- Risk Monitor.
- Proposal Queue.
- Evidence Ledger.
- Local Lane.
- Cloud Lane.
- Validation Lane.
- Task Packet.

Avoid:

- Devil in user-facing copy.
- “autonomous agent” as primary UX term.
- “swarm” as the only metaphor.

## 3. Product modes

### 3.1 Manual

Purpose:

- deterministic offline IDE.

Capabilities:

- no AI.
- no inference.
- no agent panels.
- no cloud.
- LSP/DAP/git/index/test/security tools.

Value:

- trust.
- offline use.
- professional baseline.

### 3.2 Assist

Purpose:

- low-friction AI help.

Capabilities:

- inline edit prediction.
- contextual AI actions.
- no chat required.
- no autonomous execution.

Value:

- faster editing.
- minimal cognitive overhead.

### 3.3 Delegate

Purpose:

- ask AI to produce scoped proposals.

Capabilities:

- chat.
- codebase context.
- multi-file proposals.
- per-hunk review.
- tool permissions.

Value:

- AI can help without taking authority.

### 3.4 Automate

Purpose:

- assembly-line execution.

Capabilities:

- task graph.
- planner.
- specialist workers.
- validation gates.
- decision feed.
- risk monitor.
- cloud/local routing.

Value:

- visible, verifiable automation.

## 4. Development phases

## Phase 0 — Stabilize repo and rename surface

### Goals

- Establish Legion branding.
- Avoid internal churn.
- Verify current tests/build.
- Create baseline architecture docs.

### Tasks

1. Run current build/tests.
2. Record failing tests if any.
3. Add `LEGION_RENAME.md` migration note.
4. Change visible app name to Legion IDE.
5. Change window title.
6. Change about dialog.
7. Change splash/menu labels.
8. Change README visible branding.
9. Keep crate names unchanged.
10. Add product vocabulary doc.

### Exit criteria

- App still builds.
- User-facing UI says Legion.
- Internal crate names can still be `devil-*`.

## Phase 1 — Panel-host dock refactor

### Goals

- Replace whole-pane mode swaps.
- Create shared filtered panel registry.
- Enforce Manual no-AI contract structurally.

### Tasks

1. Add `PanelId` enum.
2. Add `PanelCapability` enum.
3. Add `DockSide` enum.
4. Add `DockPanel` trait.
5. Add `PanelMetadata`.
6. Add `PanelRegistry`.
7. Implement `visible_for(mode)`.
8. Implement Manual filter.
9. Implement Offline build feature exclusion.
10. Add fake AI panel test.
11. Add deterministic panel test.
12. Add mode-scoped layout struct.
13. Add layout persistence.
14. Add left/right/bottom dock rendering.
15. Migrate old `RightConsole` behavior.
16. Add mode defaults.
17. Add layout reset command.

### Exit criteria

- All modes render through registry.
- Manual cannot instantiate AI panels.
- Offline build compiles without AI crates.
- Layout persists per mode.

## Phase 2 — Manual deterministic IDE foundation

### Goals

- Make Manual useful.
- Prove deterministic-first product value.

### Tasks

1. Wire LSP diagnostics.
2. Build Problems panel.
3. Wire code actions.
4. Add quick-fix lightbulb.
5. Build Outline panel.
6. Add breadcrumbs.
7. Add sticky scope headers.
8. Add inlay hints.
9. Add code lens.
10. Add find references panel.
11. Add go-to-definition UI.
12. Add call hierarchy panel.
13. Add type hierarchy panel.
14. Add workspace symbol search.
15. Add command palette entries.

### Exit criteria

- Manual mode can navigate and fix simple issues through LSP.
- Works offline with rust-analyzer/local tools.

## Phase 3 — Structural Search & Replace

### Goals

- Add deterministic standout feature.
- Use tree-sitter/ast-grep/GritQL style rewrite.

### Tasks

1. Evaluate ast-grep Rust integration vs CLI/LSP mode.
2. Add structural pattern DTOs.
3. Add pattern editor panel.
4. Add metavariable preview.
5. Add workspace search command.
6. Add result grouping by file/symbol.
7. Add rewrite preview.
8. Add safe apply through proposal path.
9. Add dry-run mode.
10. Add saved patterns.
11. Add suppression support if using ast-grep.
12. Add large-repo performance tests.

### Exit criteria

- User can structurally search and rewrite with preview.
- Manual-eligible.
- No AI required.

## Phase 4 — VCS, syntactic diff, blame, and history

### Goals

- Upgrade git UX.
- Prepare proposal review foundation.

### Tasks

1. Add git status panel.
2. Add hunk-level staging.
3. Add inline blame.
4. Add commit details.
5. Add git graph.
6. Add branch history.
7. Add syntactic diff integration.
8. Add file-size threshold fallback.
9. Add conflict marker detection.
10. Add one-click conflict region choices.
11. Add diff view reusable by proposals.

### Exit criteria

- VCS is useful in Manual.
- Diff review component is ready for AI proposals.

## Phase 5 — DAP debugger and test runner

### Goals

- Close “real IDE” gap.
- Make deterministic validation visible.

### Tasks

1. Add DAP client DTOs.
2. Implement DAP transport.
3. Implement adapter process host.
4. Add breakpoint store.
5. Add breakpoint UI.
6. Add call stack panel.
7. Add variables panel.
8. Add watches panel.
9. Add debug console.
10. Add stepping commands.
11. Add inline debug values.
12. Add Cargo debug locator.
13. Add test explorer.
14. Add cargo test runner.
15. Add coverage gutter path.

### Exit criteria

- User can run/debug/test Rust code.
- Breakpoints persist.

## Phase 6 — Assist mode

### Goals

- Add inline AI without autonomous behavior.

### Tasks

1. Implement OpenAI-compatible provider adapter.
2. Implement local llama.cpp provider adapter.
3. Implement Ollama adapter.
4. Add provider config UI.
5. Add model status panel.
6. Add inline prediction request builder.
7. Add LSP-context packet.
8. Add ghost text renderer.
9. Add accept/reject commands.
10. Add latency budget metrics.
11. Add Assist mode registry panels.
12. Add Manual exclusion tests.

### Exit criteria

- AI inline prediction works in Assist.
- Manual remains AI-free.

## Phase 7 — Delegate mode

### Goals

- Add chat, repo context, proposal queue.

### Tasks

1. Add chat panel.
2. Add chat session model.
3. Add codebase context builder.
4. Add embeddings chunker.
5. Add vector search.
6. Add context inspector.
7. Add proposal DTO.
8. Add unified diff parser.
9. Add patch application to sandbox.
10. Add multi-file proposal queue.
11. Add per-hunk approval.
12. Add tool permission queue.
13. Add risk flags.
14. Add validation runner.
15. Add evidence records.

### Exit criteria

- Chat can propose multi-file diff.
- Diff cannot apply without approval.
- Validation evidence is shown.

## Phase 8 — Automate / Legion workflow orchestration

### Goals

- Build the assembly-line product.

### Tasks

1. Add workflow DTOs.
2. Add task graph DTOs.
3. Add worker DTOs.
4. Implement workflow coordinator.
5. Implement task decomposition path.
6. Implement task packet builder.
7. Implement worker assignment.
8. Implement local worker lane.
9. Implement worker lease/timeout.
10. Implement worker cleanup.
11. Implement dependency tracking.
12. Implement file conflict tracking.
13. Implement validation matrix.
14. Implement proposal aggregation.
15. Implement integration worktree.
16. Implement merge readiness.
17. Implement Decision Feed.
18. Implement Risk Monitor.
19. Implement Kill Switch.
20. Implement Legion Board UI.

### Exit criteria

- A workflow with multiple scoped tasks runs.
- Workers generate proposals.
- Validation happens.
- User reviews and applies.
- Worker contexts are killed/cleaned.

## Phase 9 — Legion Cloud MVP

### Goals

- Give weak-hardware users hosted worker lanes.

### Tasks

1. Define cloud task API.
2. Build control plane skeleton.
3. Add auth/project token.
4. Add upload-only task packet mode.
5. Build CPU validation lane.
6. Build small model lane.
7. Add RunPod/Modal prototype.
8. Add usage metering.
9. Add cloud lane UI.
10. Add cloud upload scope preview.
11. Add cancellation.
12. Add proposal return.
13. Add artifact/evidence store.

### Exit criteria

- User can send a scoped low-risk task to cloud and receive a proposal/evidence.

## Phase 10 — Trace collection and specialist training

### Goals

- Build training data flywheel.

### Tasks

1. Add trace schema.
2. Add opt-in consent UI.
3. Store task packets.
4. Store model outputs.
5. Store validation result.
6. Store human approval/rejection.
7. Store final accepted diff.
8. Build data export.
9. Build eval harness.
10. Fine-tune first specialist.
11. Quantize model.
12. Deploy to local/cloud worker lane.

### Exit criteria

- First Legion-specific specialist improves on baseline in held-out eval.

## 5. MVP cut lines

### MVP A: Deterministic Legion

Includes:

- user-facing rename.
- dock registry.
- Manual mode.
- LSP diagnostics/outline/quick-fix.
- structural search.

Excludes:

- AI.
- cloud.
- automations.

### MVP B: Assist/Delegate Legion

Adds:

- provider adapters.
- inline predictions.
- chat.
- proposal queue.
- validation runner.

Excludes:

- multi-agent workflows.
- cloud.

### MVP C: Legion Automate Local

Adds:

- Legion Board.
- workflow coordinator.
- local worker lanes.
- isolated sandboxes.
- evidence.
- risk monitor.

Excludes:

- cloud lanes.
- fine-tuned specialists.

### MVP D: Legion Cloud Lane

Adds:

- hosted validation lane.
- hosted small-model lane.
- cloud scope preview.
- usage metering.

### MVP E: Fine-tuned Legion Specialists

Adds:

- trained docs specialist.
- trained compiler-error fixer.
- trained test-writer.
- eval harness.

## 6. Development team lanes

### Lane 1: UI/Dock

Owns:

- panel registry.
- dock rendering.
- layouts.
- panels.
- proposal review UI.
- Legion Board.

### Lane 2: Protocol/Runtime

Owns:

- DTOs.
- LSP/DAP.
- event streams.
- validation runner.

### Lane 3: Agent/Workflow

Owns:

- sandbox.
- workflow coordinator.
- worker lifecycle.
- conflict detection.
- proposal aggregation.

### Lane 4: AI/Providers

Owns:

- provider adapters.
- prompt/task packets.
- structured output.
- local model serving.

### Lane 5: Cloud

Owns:

- control plane.
- worker lanes.
- validation lanes.
- usage/billing.
- artifact store.

### Lane 6: Model Training

Owns:

- trace data.
- eval harness.
- fine-tuning.
- quantization.
- deployment.

## 7. Quality gates

### Manual gate

Must pass:

- Manual cannot instantiate AI panel.
- Offline build excludes AI crates.
- deterministic tools work offline.

### Proposal gate

Must pass:

- no worker can apply to main workspace.
- invalid diff rejected.
- out-of-scope diff rejected.
- user/app approval required.

### Cloud gate

Must pass:

- upload scope visible.
- secrets blocked/redacted.
- task cancellation works.
- cost cap enforced.

### Automate gate

Must pass:

- kill switch works.
- risk monitor can halt workflow.
- conflict detection works.
- worker cleanup works.

### Training gate

Must pass:

- held-out eval improves vs base.
- no regression on safety format.
- output schema compliance.
- quantized model passes smoke tasks.

## 8. Immediate implementation checklist

Start here:

1. Pull latest repo.
2. Run current build/tests.
3. Create `docs/LEGION_PIVOT.md`.
4. Create `docs/ARCHITECTURE_AUTHORITY_BOUNDARIES.md`.
5. Create `docs/MODES.md`.
6. Implement `PanelCapability`.
7. Implement `DockPanel` trait.
8. Implement `PanelRegistry`.
9. Add fake AI panel and Manual exclusion test.
10. Replace `RightConsole` with dock host.
11. Add mode-scoped layout persistence.
12. Register Project/Outline/Problems panels.
13. Change user-facing branding to Legion.
14. Do not rename crates yet.

## 9. Decision points

### Decision 1: Internal crate rename timing

Recommendation:

- defer until Phase 2/3 stability.

### Decision 2: First cloud provider

Recommendation:

- RunPod for GPU MVP.
- Fly.io or simple VPS for control plane.
- Modal for fast training/validation experiments.

### Decision 3: First specialist to train

Recommendation:

- docs/summarizer first for easiest win.
- Rust compiler-error fixer second.
- test writer third.

### Decision 4: First local model

Recommendation:

- Qwen2.5-Coder-1.5B-Instruct and 3B-Instruct.

### Decision 5: Planner model

Recommendation:

- keep Codex/GPT-5.5 subscription route as high-level thinker where available.
- use Kimi 2.6 as strong delegated coding/documentation backend where Hermes/provider supports it.
- for Legion product, treat planner route as configurable provider, not hardcoded.

## 10. Product moat

The moat is not generic code completion.

The moat is:

- deterministic IDE substrate.
- mode-based trust boundaries.
- task packets.
- disposable worker lanes.
- proposal-only mutation.
- validation evidence.
- cloud lanes for weak hardware.
- trace/eval/fine-tuning flywheel.

Legion becomes better and cheaper over time as accepted traces train small specialists.
