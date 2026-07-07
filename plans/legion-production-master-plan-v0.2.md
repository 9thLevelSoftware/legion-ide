# Legion IDE - Production Master Plan v0.2

- Status: Draft for review
- Date: 2026-06-19
- Supersedes: `plans/legion-production-master-plan-v0.1.md`
- Preserves: all historical evidence and milestone gates under `plans/evidence/`
- Primary intent: convert the current Legion substrate, accepted M0-M6 evidence, and 2026 AI-coding market reality into a production utility plan that can be executed without weakening Legion's authority, proposal, privacy, and audit boundaries.

## 1. Executive Verdict

Legion is no longer accurately described by the v0.1 sentence "most IDE verbs are simulated." Since v0.1, the repo has moved forward: real LSP process/session primitives exist, tree-sitter overlay plumbing is present, workspace search has a streaming/indexed path, the app terminal composes through native PTY services, Anthropic and OpenAI provider work is materially deeper, MCP and ACP surfaces exist, M4-M6 milestone evidence is accepted, and external security/evidence work has been recorded.

That does not make Legion production-ready. The correct current diagnosis is sharper:

Legion has a strong control-first IDE substrate and several accepted product-track milestones, but it still lacks enough real-user, cross-platform, failure-mode evidence to claim daily-driver production utility. The highest production risk is not absence of architecture. It is evidence drift: code and plans say many important surfaces exist, while the product-readiness ledger still marks UI/accessibility, large-workspace behavior, language/debug/test/SCM, runtime extension execution, remote development, collaboration/admin controls, signing/updates, and crash reporting as substrate-validated, in progress, or explicitly deferred.

This plan therefore focuses less on inventing new architecture and more on productizing, proving, and narrowing:

1. Make Manual mode a boring, excellent native IDE.
2. Make Assist mode inspectable, proposal-mediated, local-first, and useful on real projects.
3. Make Delegate mode an evidence-producing worktree/sandbox harness, not a chat panel.
4. Make Automate mode an operator console for bounded multi-agent work, with kill switches, budgets, replay, and review.
5. Treat extensions, remote, collaboration, telemetry, raw-source retention, and autonomous apply as controlled activation programs.
6. Turn every "accepted" claim into a runnable product workflow, per-platform smoke, or explicit deferred cut line.

The moat remains the trust stack: proposal-mediated mutation, metadata-first evidence, default-deny capabilities, air-gap/local-first options, context manifests, privacy inspector, and auditable decision surfaces. The 2026 market has validated that direction. Cursor 3, Devin Desktop, GitHub Agent HQ, OpenAI Codex, Claude Code, Zed ACP, JetBrains Junie, VS Code Agent Mode, and Kiro specs all converge on agent orchestration, plans/specs, sandboxes, approvals, evidence, MCP/ACP, and diff-first review. Legion should not chase every feature. It should become the most trustworthy native control surface for manual-to-autonomous development.

## 2. Evidence Basis

### 2.1 Local evidence used

Authoritative repo sources:

- `README.md`
- `AGENTS.md`
- `docs/INDEX.md`
- `docs/MODES.md`
- `docs/ARCHITECTURE_AUTHORITY_BOUNDARIES.md`
- `plans/product-readiness-ledger.md`
- `plans/phase-status-ledger.md`
- `plans/legion-production-master-plan-v0.1.md`
- `plans/evidence/production/M4/M4-milestone-acceptance.md`
- `plans/evidence/production/M5/M5-milestone-acceptance.md`
- `plans/evidence/production/M6/M6-milestone-acceptance.md`
- Workspace crate manifests and current Rust sources under `crates/`

Current workspace facts:

- Branch before this plan: `main`
- Current workspace package count: 27 members including `xtask`
- Current app architecture: `legion-protocol` contracts, `legion-app` authority/composition, `legion-ui` projection-only UI model, `legion-desktop` egui renderer, and subsystem crates for editor/text/project/LSP/terminal/AI/agent/plugin/remote/collaboration/security/storage/telemetry/retention.
- Current product-readiness ledger still distinguishes substrate acceptance from product workflow validation.

### 2.2 Web research used

Primary or official sources checked for current market/technology direction:

- Cursor 3: <https://cursor.com/blog/cursor-3>
- Cursor changelog: <https://cursor.com/changelog>
- VS Code agents: <https://code.visualstudio.com/docs/agents/overview>
- VS Code agent tools/MCP: <https://code.visualstudio.com/docs/copilot/agents/agent-tools>
- GitHub Copilot cloud agent docs: <https://docs.github.com/en/copilot/concepts/agents/cloud-agent/about-cloud-agent>
- GitHub Agent HQ announcement: <https://github.blog/news-insights/company-news/welcome-home-agents/>
- Zed edit prediction: <https://zed.dev/docs/ai/edit-prediction>
- Zed ACP: <https://zed.dev/acp>
- Devin Desktop: <https://devin.ai/desktop/>
- Devin ACP docs: <https://docs.devin.ai/desktop/acp>
- OpenAI Codex: <https://openai.com/codex/>
- OpenAI Codex CLI docs: <https://developers.openai.com/codex/cli>
- Anthropic Claude Code sandboxing: <https://www.anthropic.com/engineering/claude-code-sandboxing>
- Claude Code hooks: <https://code.claude.com/docs/en/agent-sdk/hooks>
- JetBrains Junie: <https://www.jetbrains.com/junie/>
- Kiro specs: <https://kiro.dev/docs/specs/>
- Kiro hooks: <https://kiro.dev/docs/hooks/>
- Kiro steering: <https://kiro.dev/docs/steering/>
- Agent Client Protocol: <https://agentclientprotocol.com/get-started/introduction>
- Model Context Protocol spec 2025-11-25: <https://modelcontextprotocol.io/specification/2025-11-25>
- Tree-sitter: <https://tree-sitter.github.io/tree-sitter/>
- LSP 3.17: <https://microsoft.github.io/language-server-protocol/specifications/lsp/3.17/specification/>
- DAP overview: <https://microsoft.github.io/debug-adapter-protocol/overview.html>

These sources influenced priorities, not implementation claims. Local code and ledgers remain authoritative for Legion's actual state.

## 3. Rebaseline Against v0.1

### 3.1 Claims from v0.1 that are now stale

| v0.1 claim | Current correction | Plan implication |
| --- | --- | --- |
| Tree-sitter is not wired into highlighting | `legion-index` depends on tree-sitter/tree-sitter-rust and `legion-app` has `tree_sitter_semantic_token_overlays_for_visible_lines` plus tests | Treat syntax as activation/productization, not greenfield |
| No language server process is ever launched | `legion-lsp` has process configs, `LspSupervisor`, `LspStdioLauncher`, `LspStdioSession`, framing, initialization, request/notification plumbing | Focus on real rust-analyzer workflow, lifecycle UX, server install/discovery, and stale-response safety |
| App terminal uses only deterministic fixture | `legion-app` `TerminalWorkflow` composes `TerminalRuntime<NativePtyService>`; `legion-platform` has Windows ConPTY and Unix PTY code | Focus on PTY hardening, UX, shell config, process cleanup, and platform proof |
| Project search has no index integration | `legion-project` has workspace streaming search and indexed search state with user setting wired through app/settings | Focus on performance, cancellation, search syntax, ignore policy, binary/large-file behavior, and UX |
| Anthropic provider is a stub | `legion-ai-providers` includes `AnthropicMessagesClient`, native messages/count-tokens/streaming/batch tests | Focus on provider policy, prompt/cache discipline, cost telemetry, and live provider smoke |
| ACP is future-only | `legion-app` has ACP host command wiring and M4 evidence says ACP host wiring is present | Focus on real ACP spec conformance and interop with real agents |
| M0-M6 are future milestones | M4-M6 acceptance evidence exists | Shift from milestone construction to production validation and release gating |

### 3.2 Claims that remain substantially true

| Area | Current state | Production gap |
| --- | --- | --- |
| Product-readiness distinction | Product ledger still says substrate validation is not product readiness | Keep this as the controlling truth |
| Full Rust language workflow | LSP primitives exist, but ledger says full LSP completion/diagnostics product UX is not validated | Need real rust-analyzer user workflow and failure-mode evidence |
| DAP/debug/test explorer | Debug projections and tests exist, DAP remains fixture-heavy | Need real adapter launch, breakpoints, variables, console, cargo test integration |
| Large files | Degraded mode exists; 100MB streaming remains a known gap | Need measurable product behavior and streaming model |
| Runtime VS Code extension host | Manifest/contribution compatibility exists; sidecar execution is deferred | Keep deferred or launch a separate policy-gated program |
| Remote/collaboration/admin | Substrate exists; UX remains deferred with explicit cut lines | Do not market as production until activated and tested |
| Signing/updates/crash reporting | Release pipeline evidence exists; signed installers, auto-update/rollback, crash-report controls not fully validated | Release gate remains open |

## 4. Product Definition

### 4.1 Target product

Legion IDE is a native, control-first IDE for professional software teams that need a continuous path from manual editing to AI-assisted and eventually delegated/autonomous development, without surrendering authority over code, data, egress, policy, or audit evidence.

### 4.2 Primary customers

1. Security-conscious enterprise teams that cannot let AI tools freely exfiltrate code or mutate repos.
2. Senior engineers who want agent leverage but still demand real diff, test, terminal, LSP, debugger, and git workflows.
3. Platform/tooling teams that need policy-controlled MCP/ACP/provider/extension integration.
4. AI-forward teams that want multi-agent work without losing reproducibility, review, or cost control.

### 4.3 Non-goals for production v1

- Do not out-feature VS Code or JetBrains across every language.
- Do not ship unrestricted autonomous apply.
- Do not claim VS Code marketplace compatibility beyond the implemented Open VSX/manifest/contribution surface.
- Do not promise uniform sandbox guarantees across macOS/Linux/Windows if the implementation tiers differ.
- Do not ship cloud, collaboration, or raw-source retention as default-on.
- Do not build proprietary next-edit prediction before the product can prove basic IDE utility.

### 4.4 Product pillars

1. Manual excellence: fast native editor, LSP, terminal, search, git, debug/test, keymaps, accessibility.
2. Inspectable assist: context manifest, inline preview, citations/provenance, proposal-only edits, dismiss/cancel, local-first providers.
3. Delegated execution: bounded worktree/sandbox task packets, explicit capabilities, output proposals, validation evidence.
4. Workflow command center: task graph, fleet status, budgets, risk, kill switch, replay, merge readiness.
5. Trust and governance: metadata-first audit, privacy inspector, egress visibility, policy packs, adversarial evals.
6. Interop: MCP client, ACP host/client posture, Open VSX metadata, small WASM extension surface.

## 5. Competitive Synthesis

### 5.1 What the 2026 market has made table stakes

- Agent command centers: Cursor 3, Devin Desktop, Codex, and GitHub Agent HQ make multiple local/cloud/background agents visible and steerable.
- Local/cloud handoff: agents need to move between desktop and cloud without losing context.
- Spec/plan workflows: Kiro, GitHub Agent HQ, and Codex-style planning make requirements/design/tasks first-class artifacts.
- Runtime tools: agents now routinely search, read, edit, run commands, browse, test, and self-correct.
- MCP: tool/data integration standardization is expected.
- ACP: editor-agent interoperability is becoming the LSP-like bridge for coding agents.
- Sandboxing and approvals: Claude Code, Codex, and enterprise Copilot all sell control, permissions, and safe execution.
- Evidence artifacts: screenshots, demos, test logs, diff summaries, task traces, and review notes are now part of the product.
- Cost/usage controls: agentic coding has shifted from flat "autocomplete" economics to budgets, credits, usage, and cache discipline.
- AI review: Bugbot, Copilot review, Junie, and similar tools make code review another agent lane.

### 5.2 Where Legion can win

Legion should not win by being the fastest to bolt a chat agent onto an editor. It should win by making agentic development operationally sane:

- Manual mode has provable zero egress.
- Every mutation has a proposal identity and lifecycle.
- Every provider/tool/agent invocation has visible policy, privacy, and cost state.
- Agents work in isolated lanes and return proposals plus evidence.
- Review is diff-first, risk-ranked, and replayable.
- Enterprise administrators can cap modes, providers, tools, budgets, and retention.
- Local/self-hosted paths are first-class, not downgraded escape hatches.

### 5.3 Where Legion must avoid ego-driven scope

- Do not build a bespoke LSP ecosystem; implement LSP well.
- Do not build a full VS Code clone; stage compatibility and own a smaller extension API.
- Do not build a custom vector database before lexical/AST/context manifests are measured.
- Do not build cloud remote development before local Manual/Assist are daily-drivable.
- Do not optimize agent autonomy before proposal review is excellent.

## 6. Target Architecture

### 6.1 Authority boundaries

The existing boundaries remain correct:

- `legion-protocol`: shared DTOs/contracts only.
- `legion-app`: composition root, policy enforcement, authoritative product state, workflow orchestration.
- `legion-ui`: projection-only state and typed command intents.
- `legion-desktop`: renderer/adapter edge, no product authority.
- `legion-text`/`legion-editor`: buffer, snapshot, edit, viewport, undo/redo authority.
- `legion-project`: workspace trust, discovery, VFS, filesystem mutation, git/search.
- `legion-lsp`: process/framing/protocol/session lifecycle for language servers.
- `legion-terminal`: terminal/debug runtime abstractions; app composes policy.
- `legion-ai`/`legion-ai-providers`: provider interfaces and adapters, no file mutation.
- `legion-agent`: worktree/sandbox task runtime and proposal generation, no main workspace direct mutation.
- `legion-security`: default-deny decisions and policy packs.
- `legion-storage`/`legion-observability`/`legion-tracker`/`legion-memory`/`legion-retention`: metadata-first persistence, audit, consent, and retention.

### 6.2 Core end-to-end flow

Manual edit flow:

1. User opens trusted workspace.
2. Workspace authority discovers files and trust policy.
3. Editor opens buffer through project/app services.
4. Text model emits viewport snapshot.
5. Parser/LSP/search/git services enrich projections.
6. UI renders snapshots.
7. Save routes through proposal/save workflow with fingerprints, versions, generation, and conflict checks.

Assist flow:

1. User requests inline/chat help.
2. App builds context manifest with files, symbols, diagnostics, privacy labels, egress route, model, budget, and citations.
3. Policy denies, asks, or allows provider invocation.
4. Provider returns completion/stream.
5. App converts write intent into proposal.
6. UI renders diff, risk, privacy, rollback, and verification commands.
7. User approves/rejects/applies through app authority.

Delegate flow:

1. User scopes a task packet: goal, allowed files, forbidden files, allowed tools, budgets, validation commands.
2. App/agent creates worktree/sandbox lane.
3. Agent operates only inside lane.
4. Worker outputs patch/proposal/evidence, not direct main workspace writes.
5. App validates provenance, policy, diff, and test evidence.
6. User reviews and applies through proposal lifecycle.

Automate flow:

1. User creates/loads task graph.
2. Coordinator schedules worker lanes with dependencies and budgets.
3. Fleet console shows state, risk, evidence, decisions, failures, costs.
4. Kill switch can terminate lane group.
5. Merge readiness requires passing checks, resolved conflicts, approved proposals, and audit records.

### 6.3 Production data model additions

Required new or hardened records:

- `ProductWorkflowEvidenceRecord`: one record per golden path run.
- `ProviderUsageRecord`: provider/model/tokens/cache/cost/latency/egress label.
- `AgentLaneRecord`: sandbox/worktree path, capabilities, state, parent workflow, cleanup status.
- `ApprovalPolicyRecord`: risk class -> prompt/auto/deny policy.
- `TerminalSessionEvidenceRecord`: shell, platform, process group, exit/cleanup, redaction summary.
- `LspServerHealthRecord`: server binary provenance, init status, capabilities, diagnostics latency, restart count.
- `ReleaseEvidenceRecord`: installer hash/signing/notarization/update/rollback/smoke state.
- `AccessibilityEvidenceRecord`: screen-reader/high-contrast/focus/keyboard results.
- `ExtensionCompatibilityRecord`: manifest/API/contribution/runtime status.
- `RemoteCollaborationEvidenceRecord`: network/CRDT/admin-retention state when those surfaces activate.

## 7. Production Workstreams

Each workstream below is intentionally granular. A task is complete only when code, tests, product workflow evidence, docs/ledger updates, and relevant gates are done.

### WS-P0 - Rebaseline, Ledgers, and Plan Hygiene

Objective: make repo truth internally consistent before more feature work.

Tasks:

1. P0.01 Create `plans/legion-production-master-plan-v0.2.md` and mark v0.1 historical.
2. P0.02 Update `README.md` and `docs/INDEX.md` to point production readers at v0.2.
3. P0.03 Add a "current-state corrections" note to v0.1 or a companion historical note so stale v0.1 claims are not repeated.
4. P0.04 Reconcile `plans/product-readiness-ledger.md` with M4-M6 evidence without inflating product-ready statuses.
5. P0.05 Add one table mapping M0-M6 accepted evidence to remaining product-readiness gates.
6. P0.06 Add a standing rule: milestone evidence can be accepted while product-readiness remains open.
7. P0.07 Add a doc-hygiene check that current README references the latest master plan.
8. P0.08 Add a release-note style "what changed since v0.1" appendix.
9. P0.09 Identify evidence files with dirty-worktree caveats and decide whether they need clean reruns.
10. P0.10 Create a weekly "Legion-on-Legion" dogfood journal template.

Acceptance:

- Docs and ledgers no longer contradict current code/evidence.
- `cargo run -p xtask -- docs-hygiene` passes.
- `git diff --check` passes.

### WS-MANUAL-01 - Editor Feel, Rendering, and Input

Objective: make Manual mode feel like a credible native IDE before AI features are emphasized.

Tasks:

1. MANUAL.01 Define editor latency budgets: keypress-to-paint p50/p95, scroll p95, open file, save, search, LSP completion.
2. MANUAL.02 Extend `xtask perf-harness` from skeleton to renderer-backed input-to-paint measurement.
3. MANUAL.03 Verify egui custom editor path avoids `TextEdit` regressions under `xtask no-egui-textedit`.
4. MANUAL.04 Add IME smoke tests for Windows, macOS, Linux where automatable.
5. MANUAL.05 Add clipboard copy/cut/paste/select-all integration tests.
6. MANUAL.06 Add multi-cursor and rectangular selection design if not already intentionally out of v1.
7. MANUAL.07 Validate keyboard focus across editor, panels, command palette, terminal, and diff review.
8. MANUAL.08 Add configurable font fallback and missing font diagnostics.
9. MANUAL.09 Add line wrapping policy with stable viewport math.
10. MANUAL.10 Add visible large-file/degraded-mode banner with explicit capability reduction.
11. MANUAL.11 Add deterministic screenshot or textual renderer evidence for core editor states.
12. MANUAL.12 Add manual-mode zero-egress smoke that opens/edits/saves/searches without network.

Touches:

- `crates/legion-desktop`
- `crates/legion-ui`
- `crates/legion-app`
- `crates/legion-editor`
- `crates/legion-text`
- `xtask`

Acceptance:

- `PR-UI-001` can move from substrate-validated toward product-workflow validated only after real renderer-backed input, focus, accessibility, and platform evidence exist.

### WS-MANUAL-02 - Large Files and Workspace Scale

Objective: support real repositories and large files without blocking typing.

Tasks:

1. SCALE.01 Define reference workspaces: Legion repo, 100K-file generated repo, 100MB single file, large Cargo workspace, mixed binary/text workspace.
2. SCALE.02 Convert ignored 100MB workload into a measured, non-green-baseline test with explicit thresholds.
3. SCALE.03 Implement or harden streaming text viewport so 100MB files do not materialize full caches by default.
4. SCALE.04 Add binary-file detection and safe preview refusal.
5. SCALE.05 Add file-size policy projection and UX status rows.
6. SCALE.06 Prove workspace tree open does not block editor input.
7. SCALE.07 Prove watcher burst/debounce behavior under generated churn.
8. SCALE.08 Prove workspace search cancellation releases resources.
9. SCALE.09 Add memory ceiling measurement for GP-1 workload.
10. SCALE.10 Add stale snapshot/lease tests for large-file edits.

Touches:

- `crates/legion-text`
- `crates/legion-editor`
- `crates/legion-project`
- `crates/legion-app`
- `crates/legion-desktop`
- `xtask`

Acceptance:

- `PR-UI-002` has real large-file and large-workspace evidence, not just guardrail tests.

### WS-LANG-01 - Rust LSP Product Workflow

Objective: make Rust language intelligence real, fast, and user-visible.

Tasks:

1. LANG.01 Define rust-analyzer discovery order: bundled, project-local, system path, configured path.
2. LANG.02 Record server binary provenance and version in `LspServerHealthRecord`.
3. LANG.03 Launch rust-analyzer through `LspStdioSession` for a real fixture project.
4. LANG.04 Complete initialize/initialized handshake and workspace folder config.
5. LANG.05 Wire document open/change/save synchronization from editor snapshots.
6. LANG.06 Wire publishDiagnostics into problems panel with metadata-safe redaction.
7. LANG.07 Wire completion request/response into UI with stale-snapshot rejection.
8. LANG.08 Wire hover, go-to-definition, references, rename, format, code actions, semantic tokens, inlay hints, code lenses, and folding in priority order.
9. LANG.09 Route write-producing code actions through proposal lifecycle.
10. LANG.10 Add server restart/backoff/crash UX.
11. LANG.11 Add LSP log redaction.
12. LANG.12 Add 3-OS rust-analyzer smoke in CI or release gate.

Touches:

- `crates/legion-lsp`
- `crates/legion-app`
- `crates/legion-ui`
- `crates/legion-desktop`
- `crates/legion-protocol`
- `plans/evidence/production`

Acceptance:

- User can open Legion itself, see real rust-analyzer diagnostics/completions/hover/definition, perform a rename proposal, and recover from server restart.

### WS-LANG-02 - Syntax, Structural Search, and Symbols

Objective: turn tree-sitter and structural indexing into reliable editor affordances.

Tasks:

1. SYNTAX.01 Inventory supported grammar set for v1: Rust first, then TOML/JSON/Markdown.
2. SYNTAX.02 Define parser crate ownership and dependency policy.
3. SYNTAX.03 Harden tree-sitter overlay caching and invalidation by content hash/snapshot id.
4. SYNTAX.04 Add per-language query files or embedded query definitions.
5. SYNTAX.05 Add parse-error overlays and degraded fallback.
6. SYNTAX.06 Project outline/symbol tree from parser/LSP when available.
7. SYNTAX.07 Add sticky scopes and breadcrumbs from syntax tree.
8. SYNTAX.08 Wire structural search across active file and workspace with cancellation.
9. SYNTAX.09 Add rewrite-as-proposal for safe structural transformations.
10. SYNTAX.10 Add parser performance tests for edited file incremental updates.

Acceptance:

- Rust files highlight with tree-sitter overlays, expose outline/breadcrumbs, and support structural search/rewrite without UI owning parser state.

### WS-TERM-01 - Terminal Runtime Productization

Objective: make the integrated terminal safe and useful on all supported platforms.

Tasks:

1. TERM.01 Define terminal shell selection policy: default shell, configured shell, workspace shell.
2. TERM.02 Verify Windows ConPTY, Unix PTY, and process-group kill behavior.
3. TERM.03 Add terminal launch permission policy and Manual-mode allowances.
4. TERM.04 Add terminal input/output redaction classification.
5. TERM.05 Add terminal scrollback limits and search.
6. TERM.06 Add resize propagation.
7. TERM.07 Add environment variable allow/deny policy.
8. TERM.08 Add terminal working-directory selection.
9. TERM.09 Add orphan process cleanup and evidence.
10. TERM.10 Add terminal command proposal route for agent-suggested commands.
11. TERM.11 Add terminal failure UX: denied, unavailable, exited, crashed, policy-blocked.
12. TERM.12 Add platform smoke tests for PowerShell, cmd, bash/zsh where available.

Acceptance:

- User can run `cargo test`, see output, search output, stop the session, and export metadata-safe evidence.

### WS-DEBUG-01 - Debug and Test Explorer

Objective: move from debug projections to real debug/test workflows.

Tasks:

1. DEBUG.01 Choose DAP adapter strategy for Rust: CodeLLDB, GDB DAP, or configured adapter.
2. DEBUG.02 Define adapter install/provenance policy.
3. DEBUG.03 Launch a real DAP session against a tiny Rust binary.
4. DEBUG.04 Implement breakpoint set/remove/disable with path/version awareness.
5. DEBUG.05 Implement start/continue/pause/step/stop.
6. DEBUG.06 Project stack frames, variables, watches, console output.
7. DEBUG.07 Route debug console commands through policy.
8. DEBUG.08 Add Cargo test discovery for workspace packages.
9. DEBUG.09 Add test explorer projection and rerun failed tests.
10. DEBUG.10 Correlate test failures with problems panel and terminal output.
11. DEBUG.11 Add zero-config Rust debug path from Cargo metadata.
12. DEBUG.12 Add failure-mode tests for missing adapter, stale breakpoint, crashed target.

Acceptance:

- User can set a breakpoint in a Rust fixture, debug it, inspect variables, run tests, and see failures without fixture-only data.

### WS-SEARCH-01 - Search, Navigation, and Command Surface

Objective: make navigation feel competitive with modern IDEs.

Tasks:

1. SEARCH.01 Define search query grammar: literal, regex, case, whole word, globs.
2. SEARCH.02 Harden current streaming workspace search for large repos.
3. SEARCH.03 Decide whether to add ripgrep library crates or keep current implementation with measured proof.
4. SEARCH.04 Verify Tantivy/indexed search behavior, rebuild policy, and invalidation.
5. SEARCH.05 Add fuzzy file opener.
6. SEARCH.06 Add command palette ranking and telemetry-free local history.
7. SEARCH.07 Add symbol search from LSP/tree-sitter.
8. SEARCH.08 Add references/usages search.
9. SEARCH.09 Add search result preview and keyboard navigation.
10. SEARCH.10 Add search cancellation and stale result markers.
11. SEARCH.11 Add ignore policy parity with `.gitignore` and workspace trust.
12. SEARCH.12 Add binary/large-file safeguards.

Acceptance:

- Search over the Legion repo returns expected results within budget, can be cancelled, and never blocks typing.

### WS-GIT-01 - Git, Review, and Local History

Objective: make SCM a first-class surface for human and agent work.

Tasks:

1. GIT.01 Render changed files, hunks, staged/unstaged state, conflicts, blame, branches.
2. GIT.02 Add inline gutter diff markers.
3. GIT.03 Add diff viewer with keyboard review.
4. GIT.04 Add proposal diff integration with git diff.
5. GIT.05 Add stage/unstage/revert-by-hunk through proposal or explicit user command.
6. GIT.06 Add commit author/message validation.
7. GIT.07 Add branch/worktree creation UI for delegated tasks.
8. GIT.08 Add merge conflict viewer and resolution proposal route.
9. GIT.09 Add local history/checkpoint snapshots independent of git.
10. GIT.10 Add jj posture decision: support, defer, or explicit non-goal.
11. GIT.11 Add PR/review integration as post-v1 or extension-provided capability.
12. GIT.12 Add clean-worktree and dirty-worktree evidence exports.

Acceptance:

- User can review and commit a multi-file change produced by Assist/Delegate with full diff and checkpoint context.

### WS-AI-01 - Provider Plane and Cost Controls

Objective: make providers usable without hidden egress or surprise spend.

Tasks:

1. AI.01 Define provider tiers: local, loopback self-hosted, BYOK hosted, enterprise gateway, disabled.
2. AI.02 Verify Ollama and llama.cpp live smoke paths.
3. AI.03 Verify OpenAI-compatible and native OpenAI Responses paths with injected transports and optional live smoke.
4. AI.04 Verify Anthropic Messages/count-tokens/streaming/batch paths with current API behavior.
5. AI.05 Add provider health panel.
6. AI.06 Add per-provider/model cost estimate and actual usage records.
7. AI.07 Add prompt cache stability tests where provider supports caching.
8. AI.08 Add timeout/retry/cancellation policy.
9. AI.09 Add no-hidden-egress tests for Manual and air-gap modes.
10. AI.10 Add BYOK secret storage flow using retention/keyring policy.
11. AI.11 Add provider route refusal UX.
12. AI.12 Add model capability metadata: tool support, context window, structured outputs, streaming, embeddings.

Acceptance:

- Before every hosted invocation, the user can see provider, model, egress, files/context, budget, and retention state.

### WS-AI-02 - Context Engine and Retrieval

Objective: produce high-quality context manifests without uncontrolled raw-source retention.

Tasks:

1. CTX.01 Define context manifest schema for files, symbols, diagnostics, terminal excerpts, git diff, memory, policy, and privacy labels.
2. CTX.02 Add manifest preview before provider invocation.
3. CTX.03 Add citations/provenance rows in assistant responses.
4. CTX.04 Add AGENTS.md/rules discovery with precedence and explicit inclusion.
5. CTX.05 Add repo map generation from tree-sitter/LSP/search.
6. CTX.06 Add agentic search path: grep/read/symbol search under budgets.
7. CTX.07 Add embeddings only after a measured decision: local-first, metadata-safe, and deletable.
8. CTX.08 Add prompt-injection labels for untrusted file content/tool output.
9. CTX.09 Add context-size budgeting and truncation explanation.
10. CTX.10 Add cache invalidation when files change.
11. CTX.11 Add context replay for evidence without raw content by default.
12. CTX.12 Add privacy inspector diff between planned and actual context.

Acceptance:

- Assist/Delegate requests have inspectable, policy-checked context manifests with stable tests.

### WS-AI-03 - Assist UX

Objective: make human-in-control AI help pleasant and safe.

Tasks:

1. ASSIST.01 Add assistant rail with session history, provider state, and context manifest.
2. ASSIST.02 Add inline prediction with accept/reject/dismiss and stale snapshot handling.
3. ASSIST.03 Add inline edit preview with proposal creation.
4. ASSIST.04 Add chat-to-proposal for multi-file edits.
5. ASSIST.05 Add explanation-only mode that cannot create proposals.
6. ASSIST.06 Add "ask about selected code" with citations.
7. ASSIST.07 Add cancellation that terminates provider stream and leaves no partial mutation.
8. ASSIST.08 Add prompt/history retention settings.
9. ASSIST.09 Add local/offline model onboarding.
10. ASSIST.10 Add bad-output handling: invalid patch, unsafe command, policy denial.
11. ASSIST.11 Add keyboard workflows for review/apply/reject.
12. ASSIST.12 Add acceptance telemetry as metadata only.

Acceptance:

- A user can request a multi-file change, inspect context, review diff, run validation, apply via proposal, and export evidence.

### WS-AGENT-01 - Delegate Runtime and Sandboxing

Objective: make delegated work reliable, constrained, and reviewable.

Tasks:

1. AGENT.01 Define task packet schema: goal, scope, forbidden scope, tools, budget, validation, expected output.
2. AGENT.02 Harden git worktree creation and cleanup.
3. AGENT.03 Add copy-based fallback policy with clear degraded status.
4. AGENT.04 Add OS sandbox tiers: Linux, macOS, Windows, devcontainer.
5. AGENT.05 Add sandbox escape test suite.
6. AGENT.06 Add filesystem scope enforcement.
7. AGENT.07 Add network egress approval path.
8. AGENT.08 Add shell/tool permission prompts with risk classification.
9. AGENT.09 Add worker output contract: proposal, evidence, validation, no direct main writes.
10. AGENT.10 Add lane cleanup on cancel/crash/app close.
11. AGENT.11 Add resumable task state.
12. AGENT.12 Add local agent, hosted agent, and ACP-hosted external agent distinction.

Acceptance:

- Delegated task can modify a sandbox/worktree, produce a proposal, run validation, and be killed without mutating the main workspace directly.

### WS-AGENT-02 - Workflow Command Center

Objective: make Automate mode an operator surface, not a hidden loop.

Tasks:

1. FLEET.01 Render workflow graph: tasks, dependencies, lanes, state.
2. FLEET.02 Render agent lanes with status, cost, files touched, risk, and validation.
3. FLEET.03 Add global and per-lane kill switch with <2s acknowledgement target.
4. FLEET.04 Add approval queue grouped by risk and dependency.
5. FLEET.05 Add decision feed and audit log.
6. FLEET.06 Add replay view from metadata/evidence.
7. FLEET.07 Add conflict detection and merge-readiness state.
8. FLEET.08 Add budget caps by workflow/provider/tool.
9. FLEET.09 Add pause/resume/steer messages.
10. FLEET.10 Add workflow templates: bug fix, refactor, docs, test generation, PR review.
11. FLEET.11 Add ACP host conformance test with at least one real external agent adapter.
12. FLEET.12 Add "why stopped" terminal states for policy/cost/conflict/validation/cancel.

Acceptance:

- User can run a bounded multi-step workflow, see all lanes and evidence, stop it, and decide what reaches the main workspace.

### WS-TRUST-01 - Proposal Review, Evidence, and Graduated Approvals

Objective: make safety reduce friction instead of creating prompt fatigue.

Tasks:

1. TRUST.01 Define risk classes for read, edit, create/delete/rename, terminal, network, retention, extension, remote.
2. TRUST.02 Add graduated approval policy: auto, ask, require explicit, deny.
3. TRUST.03 Add proposal checklist with preconditions, affected targets, rollback, validation, privacy, cost.
4. TRUST.04 Add diff-first review surface with hunk navigation.
5. TRUST.05 Add evidence artifact bundle: plan, context manifest, diff, validation, screenshots/log excerpts, policy decisions.
6. TRUST.06 Add rollback/checkpoint UI.
7. TRUST.07 Add stale/conflict handling for proposals and saves.
8. TRUST.08 Add AI code-review second-opinion lane.
9. TRUST.09 Add adversarial eval fixtures: prompt injection, malicious tool output, exfiltration lures, bad patch, test spoof.
10. TRUST.10 Add "trust overhead" metric: prompts per completed task with audit coverage.
11. TRUST.11 Add manual override policy with audit.
12. TRUST.12 Add enterprise policy export/import.

Acceptance:

- Every mutation path is proposal-mediated or explicitly user-commanded, auditable, and covered by policy tests.

### WS-EXT-01 - Extensions and Compatibility

Objective: provide useful extensibility without inheriting VS Code's full risk surface.

Tasks:

1. EXT.01 Keep VSIX/Open VSX manifest ingestion metadata-only unless runtime policy is accepted.
2. EXT.02 Define v1 built-in extension surface: themes, keymaps, snippets, tree-sitter grammars, commands with safe host calls.
3. EXT.03 Decide WASM runtime scope and WIT ABI.
4. EXT.04 Add extension capability manifest and permission review UI.
5. EXT.05 Add extension storage policy.
6. EXT.06 Add extension-originated edit-as-proposal route.
7. EXT.07 Add extension crash/disable/bisect UX.
8. EXT.08 Add marketplace/trust metadata view.
9. EXT.09 Add API coverage report for VS Code contribution points.
10. EXT.10 Keep webviews/notebooks/custom editors deferred unless separately approved.
11. EXT.11 Add extension supply-chain scanning.
12. EXT.12 Ship a small launch extension set that proves the model.

Acceptance:

- Users can install/enable/disable approved extensions that cannot bypass policy or mutate files directly.

### WS-REMOTE-01 - Remote, Collaboration, and Enterprise Admin

Objective: activate enterprise surfaces only after local utility is stable.

Tasks:

1. REMOTE.01 Keep remote/collab default-off until threat model and product UX are accepted.
2. REMOTE.02 Define remote connection types: SSH, container, cloud lane.
3. REMOTE.03 Prove encrypted transport reconnect/failure behavior.
4. REMOTE.04 Route remote filesystem mutations through proposals.
5. REMOTE.05 Add remote terminal/LSP descriptors and UX.
6. REMOTE.06 Add collaboration CRDT/operation-log product test.
7. REMOTE.07 Add presence, shared proposals, review comments, replay.
8. REMOTE.08 Add org admin policy bundles.
9. REMOTE.09 Add retention/export controls.
10. REMOTE.10 Add audit export.
11. REMOTE.11 Add self-hosted diagnostics bundle.
12. REMOTE.12 Add explicit cut lines for anything not shipping in v1.

Acceptance:

- No remote/collab/enterprise claim is user-facing until product workflow evidence exists.

### WS-REL-01 - Packaging, Updates, Crash Reporting, and Support

Objective: make Legion installable, supportable, and rollback-safe.

Tasks:

1. REL.01 Decide release channels: dev, preview, beta, stable.
2. REL.02 Finish signed Windows installer evidence.
3. REL.03 Finish macOS signing/notarization/Gatekeeper evidence.
4. REL.04 Finish Linux packaging evidence.
5. REL.05 Add auto-update check, staged rollout, and rollback.
6. REL.06 Add crash reporting opt-in and local crash bundle generation.
7. REL.07 Add first-run privacy/provider setup.
8. REL.08 Add offline/air-gap install path.
9. REL.09 Add fresh-VM smoke tests per OS.
10. REL.10 Add release descriptor verification and SBOM/provenance.
11. REL.11 Add user docs for Manual, Assist, Delegate, Automate.
12. REL.12 Add support bundle redaction and troubleshooting docs.

Acceptance:

- A beta user can install, launch, configure, crash safely, update, rollback, and produce a redacted support bundle.

### WS-QUALITY-01 - Evals, Benchmarks, and Dogfooding

Objective: make quality measurable and regressions hard to hide.

Tasks:

1. QUAL.01 Define golden paths GP-1 through GP-6.
2. QUAL.02 Convert golden paths into tests or scripted smoke runs.
3. QUAL.03 Build Legion-Bench v0 with local fixture tasks.
4. QUAL.04 Add adversarial safety evals as blocking tests.
5. QUAL.05 Add external benchmark posture document.
6. QUAL.06 Add weekly dogfood journal and triage.
7. QUAL.07 Add performance dashboard artifacts.
8. QUAL.08 Add crash-free session metric once crash reporter exists.
9. QUAL.09 Add provider cost per completed task.
10. QUAL.10 Add acceptance/rejection metadata loop.
11. QUAL.11 Add release-blocker taxonomy.
12. QUAL.12 Add "claim audit" script that checks docs against ledgers.

Acceptance:

- Roadmap claims can be checked against current evidence with repeatable commands.

## 8. Golden Paths

### GP-1 Manual Daily Edit

User opens Legion repo, edits Rust code, sees syntax, uses search/fuzzy open, uses rust-analyzer completion/diagnostics, runs terminal tests, reviews git diff, saves safely, commits.

Acceptance:

- No AI surfaces required.
- Manual mode has zero hosted egress.
- Runs on Windows/macOS/Linux or has explicit platform caveat.

### GP-2 Assist Multi-File Change

User asks Assist for a scoped multi-file refactor. Legion shows context manifest, provider route, privacy/egress, cost estimate, diff proposal, validation command, rollback checkpoint. User applies after review.

Acceptance:

- No provider/agent writes directly.
- Proposal lifecycle and evidence export are complete.

### GP-3 Delegate Sandboxed Task

User delegates a bug fix to a local agent lane. Agent works in a worktree/sandbox, runs tests, returns proposal and evidence. User reviews and applies.

Acceptance:

- Main workspace is unchanged until approval.
- Kill switch and cleanup work.

### GP-4 Automate Multi-Agent Workflow

User runs a three-task workflow: reproduce, fix, review. Fleet console shows state, dependencies, risks, cost, conflicts, evidence, and merge readiness.

Acceptance:

- Workflow can stop safely at policy, cost, validation, conflict, or cancellation boundaries.

### GP-5 Extension-Constrained Workflow

User installs an approved extension/grammar/command contribution. The extension enhances editor behavior but cannot mutate files or access network unless policy allows it.

Acceptance:

- Extension permissions are inspectable and auditable.

### GP-6 Enterprise Evidence Export

Admin/user exports a redacted audit bundle for a completed AI-assisted change.

Acceptance:

- Bundle contains metadata, hashes, decisions, validation, and deletion handles where relevant.
- No raw source appears unless explicit consent and policy allow it.

## 9. Milestones v0.2

### M7 - Truth and Beta Rebaseline

Purpose: reconcile accepted M0-M6 evidence with product-readiness claims.

Must include:

- WS-P0 complete.
- Product-readiness ledger updated.
- v0.1 marked historical.
- GP definitions committed.
- Claim audit script or checklist exists.

Exit:

- Docs hygiene passes.
- No current public doc points to v0.1 as the active production plan.

### M8 - Manual Daily Driver Beta

Purpose: make Legion usable as a manual Rust IDE for its own development.

Must include:

- WS-MANUAL-01 core tasks complete.
- WS-MANUAL-02 critical scale tasks complete.
- WS-LANG-01 Rust LSP core complete.
- WS-TERM-01 core terminal complete.
- WS-SEARCH-01 core search complete.
- WS-GIT-01 core SCM complete.

Exit:

- GP-1 passes on Legion repo.
- Dogfood for one week produces no P0/P1 blockers.

### M9 - Assist Private Beta

Purpose: ship human-in-control AI with real context and proposal review.

Must include:

- WS-AI-01 provider controls complete.
- WS-AI-02 context manifest complete.
- WS-AI-03 core assist complete.
- WS-TRUST-01 proposal review core complete.

Exit:

- GP-2 passes with local provider and one hosted BYOK provider.
- Manual mode zero-egress remains green.

### M10 - Delegate Public Beta

Purpose: ship bounded local agent lanes.

Must include:

- WS-AGENT-01 complete.
- WS-TRUST-01 adversarial eval core complete.
- WS-GIT-01 worktree review integration complete.

Exit:

- GP-3 passes on Legion repo.
- Sandbox escape suite green.

### M11 - Workflow Command Center

Purpose: ship multi-agent orchestration as Legion's differentiator.

Must include:

- WS-AGENT-02 complete.
- ACP interop smoke with at least one real external agent.
- Fleet kill switch verified.

Exit:

- GP-4 passes.
- Workflow replay/evidence export works.

### M12 - Production Beta Release

Purpose: make Legion installable and supportable for external beta users.

Must include:

- WS-REL-01 beta subset complete.
- WS-QUALITY-01 golden path automation complete.
- WS-EXT-01 launch extension subset complete.
- Accessibility and platform parity evidence refreshed.

Exit:

- Signed or explicitly unsigned-beta installers are produced.
- Fresh-VM smoke passes.
- Support bundle redaction passes.

### M13 - GA Readiness

Purpose: only claim production when product workflows, release, support, and security evidence are current.

Must include:

- GP-1 through GP-6 pass.
- Product-readiness ledger statuses updated with current evidence.
- External security findings triaged.
- Release/update/crash reporting verified.

Exit:

- No P0/P1 blockers.
- Remaining deferred surfaces are explicit and not marketed as shipped.

## 10. Acceptance Gates

### Standing gates

Run for every implementation packet unless the packet is docs-only and explicitly scoped:

```bash
cargo run -p xtask -- check-deps
cargo run -p xtask -- docs-hygiene
cargo run -p xtask -- claim-audit
cargo run -p xtask -- no-egui-textedit
cargo run -p xtask -- verify-kanban-backlog
cargo run -p xtask -- release-pipeline --dry-run
cargo run -p xtask -- verify-release-pipeline
cargo fmt --all --check
cargo check --workspace --all-targets
cargo test --workspace --all-targets
cargo clippy --workspace --all-targets -- -D warnings
cargo deny check
cargo run -p xtask -- rust-analyzer-smoke
cargo run -p xtask -- golden-path-1
cargo run -p xtask -- golden-path-2
cargo run -p xtask -- golden-path-3
cargo run -p xtask -- perf-harness
cargo run -p xtask -- verify-perf-harness
cargo run -p xtask -- update-drill
```

### Docs-only gate

Minimum for this plan and related docs:

```bash
cargo run -p xtask -- docs-hygiene
cargo run -p xtask -- check-deps
git diff --check
```

### Product-readiness gate

No product-ready status may be claimed without:

1. A named golden path.
2. Current test or smoke evidence.
3. A user-visible UX path.
4. Platform scope stated explicitly.
5. Failure-mode behavior documented.
6. Ledger update in the same change.

## 11. Risk Register

| Risk | Likelihood | Impact | Mitigation |
| --- | --- | --- | --- |
| Evidence drift returns | High | High | Claim audit, ledger-first docs, current-state corrections |
| Manual editor feels second-rate | Medium | Critical | Dogfood gate before Assist/Delegate claims |
| LSP productization underestimates edge cases | Medium | High | Rust-first, real server smoke, crash/restart tests |
| PTY leaks or unsafe command handling | Medium | High | Process cleanup tests, redaction, policy prompts |
| Proposal UX too slow | Medium | High | Graduated approvals and prompts-per-task metric |
| Agent sandbox false confidence | Medium | Critical | OS-tiered guarantees and escape tests |
| Provider costs surprise users | High | Medium | Cost estimates, budgets, usage records |
| MCP/ACP spec churn | Medium | Medium | Protocol adapter layer and versioned conformance tests |
| Extension runtime becomes unbounded | Medium | High | Tiny launch surface, WASM/WIT gating, no sidecar by default |
| Remote/collab scope distracts from local utility | Medium | High | Keep M13+ unless Manual/Assist/Delegate are proven |
| Release evidence differs from real user installs | Medium | High | Fresh-VM smoke and support-bundle validation |
| Security claims exceed implementation | Low | Critical | Threat model, external audit, exact sandbox caveats |

## 12. First 30 Days

Week 1:

- Land v0.2 plan and README/docs references.
- Add current-state correction note for v0.1.
- Update product-readiness ledger with "M0-M6 accepted but product gates remain" mapping.
- Define GP-1 to GP-6 in a dedicated evidence document.

Week 2:

- Build claim-audit checklist or script.
- Start GP-1 manual daily edit smoke.
- Identify the smallest real rust-analyzer workflow test.
- Identify current terminal native PTY smoke gaps.

Week 3:

- Implement/extend rust-analyzer launch smoke.
- Implement terminal process cleanup smoke.
- Add search performance/cancel smoke for Legion repo.
- Start dogfood journal.

Week 4:

- Run GP-1 end-to-end on current host.
- Triage P0/P1 friction.
- Decide M8 cut line.
- Freeze M8 task order.

## 13. 60-90 Day Path

Days 31-60:

- Finish Rust LSP product path.
- Finish Manual mode terminal/search/git loop.
- Refresh accessibility/platform evidence.
- Use Legion for its own docs/code work where possible.
- Keep Assist work limited to provider/context/proposal paths that do not destabilize Manual.

Days 61-90:

- Private Assist beta.
- Context manifest and privacy inspector hardening.
- Hosted provider BYOK smoke.
- Proposal review UX hardening.
- Begin Delegate sandbox lane only after GP-1 and GP-2 are stable.

## 14. Completion Criteria for Production Utility

Legion is production-useful when all of the following are true:

1. GP-1 is daily-drivable for Legion development.
2. GP-2 works with local and one hosted BYOK provider.
3. GP-3 works with bounded local agent lanes.
4. Manual mode zero-egress is continuously tested.
5. All workspace mutation paths are proposal-mediated or explicit user commands.
6. Terminal/LSP/debug/search/git failures degrade visibly.
7. Installer/update/crash/support flows are proven on target platforms.
8. Product-readiness ledger has current evidence, not inherited milestone claims.
9. Deferred surfaces are named and absent from marketing claims.
10. Security/privacy claims match implementation.

## 15. Source Notes

The most important external design signals used in this plan:

- Cursor 3 validates the agent command-center direction: multi-repo agents, local/cloud handoff, integrated diffs, browser, marketplace.
- VS Code and GitHub validate agent mode as a normal IDE behavior: agents plan, edit, run commands, monitor failures, and use tools/MCP.
- Zed validates fast native editing, edit prediction, and ACP as a credible editor-agent bridge.
- Devin Desktop validates the command-center surface for local/cloud/third-party agents.
- OpenAI Codex validates worktrees, parallel agent workflows, local CLI, and cloud command-center usage.
- Claude Code validates sandboxing, hooks, and explicit permission/event interception as product features.
- Kiro validates specs, steering, and hooks as first-class agent workflow artifacts.
- MCP validates standardized tool/context integration.
- ACP validates standardized editor-agent communication.
- Tree-sitter, LSP, and DAP remain the durable technical standards for editor syntax/language/debug functionality.

## Appendix A - Product Gate Mapping

| Product gate | Current ledger status | v0.2 milestone target |
| --- | --- | --- |
| PR-UI-001 renderer latency/accessibility | Substrate validated | M8/M12 |
| PR-UI-002 large workspace behavior | Substrate validated | M8 |
| PR-LANG-001 Rust language workflow | Substrate validated | M8 |
| PR-LANG-002 debug/test/SCM | Substrate validated | M8/M10/M12 |
| PR-AI-001 inspectable local-first AI | Product workflow validated | Keep green; refresh in M9 |
| PR-AI-002 proposal safety/evals | Substrate validated; adversarial evals deferred | M9/M10 |
| PR-VSC-001 manifest/contribution compatibility | Substrate validated | M12 |
| PR-VSC-002 isolated extension host | Deferred | M13+ unless reduced WASM launch surface ships |
| PR-ENT-001 remote development UX | Deferred | M13+ |
| PR-ENT-002 collaboration/admin controls | Deferred | M13+ |
| PR-REL-001 installability/release | In progress | M12/M13 |

## Appendix B - Workstream Dependency Spine

```text
P0 rebaseline
  -> Manual editor/input/scale
  -> Rust LSP + syntax + search + terminal + git
  -> GP-1 Manual Daily Driver
  -> provider controls + context manifest + assist proposal UX
  -> GP-2 Assist
  -> sandbox/worktree delegate runtime + adversarial evals
  -> GP-3 Delegate
  -> fleet/ACP/workflow command center
  -> GP-4 Automate
  -> extension launch set + release/support/accessibility
  -> GP-5/GP-6 Production Beta
  -> GA readiness
```

## Appendix C - Definition of Done Template

Every task packet should include:

- User-visible goal
- Non-goals
- Crates/files touched
- Authority-boundary check
- Security/privacy check
- Product-mode behavior
- Test plan
- Evidence artifact path
- Docs/ledger update
- Rollback/failure behavior
- Commands run and results

## Appendix D - What Changed Since v0.1

v0.2 is a rebaseline, not a replacement for the historical evidence corpus. The major changes since v0.1 are:

| Area | v0.1 posture | v0.2 correction |
| --- | --- | --- |
| Current-state diagnosis | Described most IDE verbs as simulated. | Recognizes real substrate progress while keeping product-readiness gates open. |
| Milestone evidence | Treated M0-M6 as future production milestones. | Preserves accepted M0-M6 evidence and requires explicit mapping to remaining product gates. |
| Plan authority | v0.1 was the production planning entry point. | v0.2 is the current master plan; v0.1 is historical audit material. |
| Product-readiness posture | Mixed future architecture and current gaps in one plan. | Separates accepted substrate evidence from product workflow validation through the readiness ledger. |
| Market posture | Used the mid-2026 market snapshot to justify the architecture direction. | Keeps that market direction but focuses execution on daily-driver product utility and evidence drift control. |
| Evidence caveats | Dirty-worktree caveats were present in acceptance files but not summarized. | WS-P0 adds an explicit caveat audit and clean-rerun decision record. |
| Dogfooding | M1 dogfooding was named as a high-leverage gate. | WS-P0 adds a weekly Legion-on-Legion journal template so dogfooding becomes repeatable evidence. |
