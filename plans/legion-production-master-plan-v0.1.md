# Legion IDE — Production Master Plan v0.1

> Historical status: this v0.1 plan was superseded by `plans/legion-production-master-plan-v0.2.md` on 2026-06-19. It is retained for audit traceability. Do not treat its current-state assessment as authoritative without checking v0.2 and `plans/product-readiness-ledger.md`.

- Status: Proposed (for ratification)
- Date: 2026-06-09
- Supersedes: nothing; this plan composes on top of `plans/phase-status-ledger.md`, `plans/product-readiness-ledger.md`, and `plans/remaining-implementation-tasks-plan-v0.1.md`. Where this plan and the ledgers disagree about *current state*, the ledgers win; where they disagree about *future sequencing*, this plan (once ratified) wins.
- Inputs:
  1. Full-workspace code survey (all 26 crates + xtask), 2026-06-09.
  2. Documentation/ledger/ADR corpus synthesis (`docs/`, `plans/`, ADR-0001..ADR-0031), 2026-06-09.
  3. Web research: state of the art in AI-assisted coding tools and agentic development environments, mid-2026 snapshot (Cursor 3.x, Devin Desktop/Cascade, Zed, VS Code + Copilot agent mode + Agent HQ, JetBrains Junie, Kiro, Trae, Firebase Studio, Google Antigravity 2.0, Claude Code, OpenAI Codex, Gemini CLI, Aider, Amp, OpenHands, Goose, Devin, Jules). Source list in Appendix B.
  4. Web research: production technology landscape for Rust desktop IDEs (GUI stacks, text/buffer tech, tree-sitter, LSP/DAP, terminal embedding, search/indexing, MCP/provider plumbing, sandboxing, WASM plugins, git libraries, signing/updates/crash reporting). Source list in Appendix B.

---

## 1. Executive Summary

Legion IDE today is a **validated substrate, not a product**. Substrate phases 0–8 (and 13) are accepted with evidence: ~156K lines of Rust across 26 crates implement a disciplined control-first architecture — proposal-mediated mutation, projection-only UI, metadata-first observability, fail-closed saves, capability-brokered policy, phase-gated runtime activation. The protocol layer, editor/text substrate, workspace VFS + watcher, git CLI integration, provider routing (Ollama / llama.cpp / OpenAI-compatible), MCP client (stdio + streamable HTTP), agent state machine with worktree sandboxing, multi-worker DAG coordinator, encrypted secret storage, TLS transport, and an egui/eframe renderer are real and tested.

However, **most of the verbs a user would call "an IDE" are still simulated**:

| Verb | Today |
| --- | --- |
| Highlight code | Theme/token plumbing exists; no real tokenizer wired (no tree-sitter, no syntect in the render path) |
| Complete / hover / rename / diagnose | Full LSP DTO surface; **no language server is ever launched** |
| Debug | DAP projections are deterministic fixtures |
| Run a terminal | Real PTY primitives exist in `legion-platform`; the app uses a deterministic fixture (`fixture_enabled` defaults off) |
| Apply an AI proposal | The entire loop ends at metadata; `runtime_apply_disabled` defaults true (Stage 1E) |
| Search the repo | No ripgrep/index integration; deterministic lexical fallback only |
| Embed / retrieve semantically | Deterministic 64-dim stub embedding |
| Run a plugin | Manifest validation only; no WASM VM |
| Collaborate / work remotely | Deterministic fixtures; transports exist but are not driven |

Meanwhile, the mid-2026 market has converged on exactly the thesis Legion bet on. The battleground is no longer "editor with AI bolted on" — it is the **harness + fleet + trust stack**: classifier-mediated approvals (Cursor auto-review, Codex approval routing), OS-level sandboxing marketed as a UX feature (Anthropic reports 84% fewer permission prompts with sandboxing on), checkpoints and diff-first review surfaces, mission-control fleet UIs (GitHub Agent HQ, Devin Desktop Agent Command Center, Antigravity Agent Manager), evidence artifacts (Antigravity), and spec-driven planning (Kiro, editable plans). Legion's proposal/evidence/trust architecture is *conceptually ahead* of most shipping products — but a trust stack wrapped around simulated verbs convinces no one.

**The plan in one sentence:** activate Legion's simulated verbs in strict dependency order using the proven 2026 component stack (tree-sitter, real LSP, real PTY, ripgrep-as-library, real apply), then productize the trust/fleet layer that is Legion's actual differentiator, then ship — without ever breaking the proposal-gated invariants that make Legion worth building.

The plan defines:

- **9 new ADRs** (ADR-0032..ADR-0040) that must be decided before or during early milestones (§6).
- **20 workstreams** (WS-01..WS-20) with 109 granular tasks, each with crate-level touchpoints, dependencies, and acceptance evidence (§7).
- **7 milestones** (M0..M6) with hard exit criteria, from "Credible Editor" through "Production GA" to post-GA expansion (§8).
- An **integration process** that extends the existing phase-gate / evidence culture rather than replacing it (§9).
- A **risk register** (§10) and **measurable quality bars** (§11).

Honest scale note: executed seriously, M1–M5 is on the order of 8–14 engineer-months of focused work for a small (1–3 person) team using heavy agentic leverage, dominated by WS-01/WS-03 (editor surface + LSP) and WS-17/WS-18 (distribution + parity). The single highest-leverage decision in this document is the **dogfooding gate at M1**: Legion must become the daily editor for its own development as early as possible, because every subsequent milestone's quality bar is enforced by that loop.

---

## 2. Method and Evidence Base

1. **Code reality first.** Every claim about current state in §3 traces to the 2026-06-09 workspace survey (per-crate implementation vs. fixture analysis, test counts, dependency inspection). Where the survey and the ledgers disagreed, the more conservative reading was kept.
2. **Market evidence second.** The competitive analysis (§4) was built from ~23 distinct web searches plus primary-source fetches of vendor docs/blogs (Cursor, Zed, GitHub, Anthropic, OpenAI, Google, Cognition, AWS, JetBrains, Sourcegraph, Block). Claims sourced only from secondary blogs are marked "reported" in Appendix B.
3. **Technology evidence third.** The stack recommendations (§6, §7) were built from ~24 searches plus crate/docs verification covering Rust GUI stacks, rope/CRDT libraries, tree-sitter, LSP/DAP client patterns (Zed/Helix/Lapce), `alacritty_terminal`/`portable-pty`, ripgrep's library crates, tantivy, LanceDB/sqlite-vec, the MCP spec status (rev 2025-11-25) and `rmcp`, provider API specifics (Anthropic Messages incl. prompt-caching economics; OpenAI Responses), sandboxing practice (bubblewrap/Seatbelt/Landlock), wasmtime + WIT extension models, gitoxide vs git2, and the cargo-dist / rcodesign / minidumper distribution stack.
4. **Invariant preservation.** Every workstream was checked against the authority boundaries in `docs/ARCHITECTURE_AUTHORITY_BOUNDARIES.md` and the dependency policy in `plans/dependency-policy.md`. No task in this plan requires `legion-ui` to take renderer dependencies, lets providers/workers mutate the workspace outside proposals, or persists raw payloads without consent.

### 2.1 Authoritative ledger inputs

This plan consolidates the existing repository truth surface rather than replacing it. The most important ledgers and runbooks are:

- [`plans/phase-status-ledger.md`](phase-status-ledger.md): substrate acceptance ledger for phases 0-8 and Legion workflow orchestration.
- [`plans/product-readiness-ledger.md`](product-readiness-ledger.md): product-readiness gates and beta acceptance scenario.
- [`plans/remaining-implementation-tasks-plan-v0.1.md`](remaining-implementation-tasks-plan-v0.1.md): remaining implementation track and phase guardrails.
- [`plans/legion-e2e/00_CONSOLIDATED_E2E_IMPLEMENTATION_PLAN.md`](legion-e2e/00_CONSOLIDATED_E2E_IMPLEMENTATION_PLAN.md): end-to-end product roadmap.
- [`AGENTS.md`](../AGENTS.md): non-negotiable repository invariants and phase-gate commands.

### 2.2 Non-negotiable invariants

1. UI stays projection-only: `legion-ui` emits intents and renders snapshots; it must not own editor sessions, workspace state, or mutation authority.
2. Workspace and editor mutations stay proposal-mediated through the accepted app/project/editor boundaries.
3. Storage, semantic, telemetry, AI, plugin, collaboration, remote, and extension compatibility surfaces remain metadata-first and default-deny unless an implementation packet adds the matching policy, tests, evidence, and user-visible workflow.
4. Cloud egress, hosted providers, production remote transports, extension-host sidecars, raw-source retention, signing credentials, and autonomous apply require explicit policy and evidence before activation.
5. Every implementation packet must pass the repository gates listed in `AGENTS.md` or record a current, actionable blocker.

---

## 3. Current State Assessment

### 3.1 What is real today

- **Protocol layer** (`legion-protocol`, ~26K LOC): complete DTO/contract surface for every subsystem, including agents, proposals, streaming, terminal, debug, collaboration, remote, semantic indexing. Gold-tier; every other crate depends on it.
- **Text substrate** (`legion-text`, ropey-backed): full UTF-8/UTF-16 coordinate model, immutable snapshots with leases and retention pins, deterministic chunk descriptors, large-file degradation path. 24 unit tests. The known 100MB streaming-mode gap is documented.
- **Editor** (`legion-editor`): multi-buffer lifecycle, transactional undo/redo, snapshot leases, viewport decorations, completion request routing with stale-snapshot protection.
- **Workspace** (`legion-project`): discovery with ignore filtering, real notify-based watcher with debounce/recovery, **real git CLI integration** (status/blame/diff), trust-aware path filtering, deterministic Cargo debug-config locator.
- **Security/policy** (`legion-security`): deny-by-default capability broker, normalized path policy (UNC, escape handling), network/provider policy, air-gap mode.
- **AI plane** (`legion-ai`, `legion-ai-providers`): provider trait + registry + policy-gated router; **real HTTP providers** for Ollama, llama.cpp, OpenAI-compatible (BYOK via env); deterministic inline-prediction provider; **working MCP client** over stdio and streamable HTTP with permission-gated tool calls.
- **Agent substrate** (`legion-agent`): deterministic state machine (Observing → Planning → Proposing → WaitingForApproval → Applying → Verifying → Completed), replay manifests, **git worktree sandbox orchestrator** with directory fallback, delegated-task proposal generator, **LegionWorkflowCoordinator** multi-worker DAG executor with conflict detection and merge-readiness evaluation.
- **Storage/audit** (`legion-storage`, `legion-tracker`, `legion-memory`, `legion-retention`): JSON-backed config/trust/metadata repositories with integrity validation, append-only audit logs, consent-gated memory candidates, ChaCha20-Poly1305 + OS-keyring secret store (24 tests).
- **Renderer** (`legion-desktop`): real egui/eframe window, event loop, projection rendering for shell/panels/code canvas with a theme/token system, smoke + beta modes with evidence export. GUI rebuild phases 1–2 (foundation + editor syntax interactions) recently merged.
- **Transport** (`legion-remote-transport`): rustls TLS 1.3 with cert verification and fingerprinting (20 tests) — built but not driven by a production feature.
- **Platform** (`legion-platform`): real cross-platform FS/PTY/process primitives (nix `openpty` on Unix; `CreateProcessW`/pipes on Windows) — built but not wired to the app-level terminal.
- **Process/CI culture**: xtask gates (`check-deps`, `docs-hygiene`), fmt/check/test/clippy/cargo-deny phase gates, evidence packages, ADR discipline, readiness ledgers.
- **Workspace gate baseline**: after the P0.1 mock-fixture regression, the full workspace test run recorded 931 passed, 0 failed, and 3 ignored performance tests; the ignored 100MB workload remains an explicit streaming-mode gap, not a green benchmark.

### 3.2 What is fixture, stub, or disabled

| Area | Mechanism | Location |
| --- | --- | --- |
| Proposal apply to disk | `runtime_apply_disabled` flag, defaults true (Stage 1E) | `legion-app` |
| Terminal execution | `TerminalFixtureRuntime`; `fixture_enabled` defaults false; real PTY never invoked from app | `legion-terminal`, `legion-app` |
| LSP | DTOs + supervision contracts only; no server process launched | `legion-index`, `legion-app` |
| DAP | `DapAdapterFixtureRuntime` deterministic projections | `legion-terminal` |
| Syntax highlighting | Token kinds → theme colors; no tokenizer in the path | `legion-desktop` |
| Embeddings/retrieval | Deterministic 64-dim embedding; lexical symbol fallback | `legion-index` |
| Project-wide search | Projection UI only; no ripgrep/tantivy | `legion-ui`, `legion-app` |
| Plugins | Manifest validation + capability gates; no WASM VM | `legion-plugin` |
| VS Code compat | package.json/Open VSX metadata only; no execution (by design) | `legion-vscode-compat` |
| Collaboration | In-memory operation log + replay; no network sync | `legion-collaboration` |
| Remote | Deterministic fixture; all feature flags default false | `legion-remote` |
| Telemetry | Fixture spool/export, disabled by default | `legion-telemetry` |
| Anthropic provider | Stub (no native Messages client) | `legion-ai-providers` |
| CLI | Argument parsing + workspace index stub | `legion-cli` |

Source-cleanliness audit baseline: `rg -n 'unimplemented!|todo!' crates --glob '*.rs'` returns no product-code matches; `rg -n '//\s*(TODO|FIXME|STUB|HACK|XXX)' crates --glob '*.rs'` returns only intentional fixture/scanner constants, including the documented test-fixture string in `crates/legion-app/tests/language_tooling_workflow.rs`.

### 3.3 Crate maturity snapshot (survey, 2026-06-09)

Gold (production-shaped): `legion-protocol`, `legion-text`, `legion-editor`, `legion-security`, `legion-storage`, `legion-observability`, `legion-retention`, `legion-remote-transport`, `legion-agent`, `legion-platform`, `legion-tracker`, `legion-ui`, `legion-desktop` (renderer shell).
Silver (substantial, gaps known): `legion-app` (~80%, fixture-heavy), `legion-project`, `legion-index`, `legion-ai`, `legion-ai-providers`, `legion-vscode-compat`, `legion-collaboration`, `legion-memory`.
Bronze (fixture/stub): `legion-terminal`, `legion-remote`, `legion-telemetry`, `legion-plugin`, `legion-cli`.

Workspace test surface: 475 Rust `#[test]` functions are present under `crates/**/*.rs`; the broader full-workspace gate currently records 931 passed tests with 3 ignored performance workloads.

### 3.4 Product-gap recap (from the ledgers, unchanged)

PR-UI-001/002 (renderer latency/accessibility product UX, 100MB streaming), PR-LANG-001/002 (LSP + DAP/test/SCM product UX), PR-AI-001/002 (context-manifest UX, real adversarial evals), PR-VSC-002 (runtime extension host — deferred cut line), PR-ENT-001/002 (remote transport, collaboration/admin — deferred cut lines), PR-REL-001 (signed installers, auto-update/rollback, crash reporting — in progress). Known limitations ledger additionally records: Windows-only GUI evidence, no signed installer, hosted-provider activation unsupported, native PTY hardening incomplete, autonomous apply unsupported.

---

## 4. Market and Technology Context (mid-2026)

### 4.1 The landscape in five layers

**(a) Editor table stakes** are commoditized and non-negotiable: LSP (80+ languages in Zed), built-in DAP debugging, tree-sitter highlighting, fuzzy finder, integrated terminal, git UI including worktree management (now agent-load-bearing), and *some* extension story.

**(b) AI assist layer**: custom next-edit-prediction models are the retention feature users feel every keystroke (Cursor Tab, Zed Zeta2 — open-weight and LSP-context-aware, Copilot NES); inline chat/edits; codebase retrieval by three competing philosophies (embeddings + AST chunking à la Cursor; **agentic search** — Claude Code dropped RAG because runtime grep/glob/read outperformed it; structural repo maps à la Aider); rules/memory files (AGENTS.md is now a Linux Foundation-governed standard with 60K+ repos); multi-model picker with BYOK/local.

**(c) Agentic layer**: schema-validated tool harnesses; plan mode with editable plans; checkpoints/rollback independent of git; OS-level sandboxing (bubblewrap/Seatbelt/Landlock; Codex sandboxes *by default*); classifier-mediated approvals replacing binary allow/deny; background/long-running tasks (Codex Goal mode is multi-day); browser tools driven via the accessibility tree (Playwright MCP); voice input (shipping in Cursor 3.7 and Antigravity 2.0); MCP client + skills/hooks; subagents with isolated context.

**(d) Orchestration/fleet layer**: mission-control UIs (Agent HQ, Devin Desktop's Kanban Agent Command Center, Antigravity Agent Manager); parallel agents in git worktrees as the default execution model; cloud agent VMs with cloud↔local handoff; coordinator/managed-agent hierarchies; scheduled and event-triggered agents; multi-vendor agent hosting via **ACP (Agent Client Protocol)** — Zed and Devin Desktop host Claude Code/Codex/Gemini agents natively.

**(e) Trust/safety/review layer**: diff-centric review surfaces (Zed multibuffers + editable diff review, Cursor 3 integrated diffs view); AI code-review agents; draft-PR-only write access and branch confinement; network firewalls/exfiltration defenses; **artifacts as verifiable evidence** (Antigravity's task lists, plans, screenshots, browser recordings); audit logs and org-level allowlists; usage/cost analytics.

Other load-bearing facts: MCP won (~97M monthly SDK downloads reported; AAIF under the Linux Foundation governs MCP + AGENTS.md + goose); the VS Code extension marketplace is legally closed to non-Microsoft products (everyone uses Open VSX, and native editors — Zed, Lapce, Helix — chose WASM/own models instead); SWE-bench Verified is saturating under contamination criticism, with emphasis shifting to SWE-bench Pro and Terminal-Bench 2.0 where the *harness* is reported alongside the model; $20/mo individual + usage-based drift + BYOK open-source flank is the pricing shape; spec-driven development (Kiro requirements/design/tasks) spread to GitHub Spec Kit and Antigravity Artifacts.

### 4.2 Gap matrix — Legion vs. 2026 state of the art

Legend: ✅ have (substrate or better) · 🟡 partial/fixture · ❌ missing · ⭐ Legion is ahead of most shipping products.

| Capability (2026 SOTA exemplar) | Legion today | Gap class |
| --- | --- | --- |
| **(a) Editor core** | | |
| Tree-sitter incremental highlighting (Zed/Helix) | ❌ token plumbing only | Build (WS-02) |
| LSP hover/def/diagnostics/rename (all) | 🟡 DTOs only, no server | Build (WS-03) |
| DAP debugging (Zed 2025, VS Code) | 🟡 fixtures | Build (WS-04) |
| Integrated terminal (all; Zed: alacritty_terminal) | 🟡 PTY primitives unwired | Build (WS-05) |
| Project search (ripgrep-class) + fuzzy finder | ❌ | Build (WS-06) |
| Git UI: gutter diff, blame, branches, worktrees | 🟡 CLI status/blame/diff data, thin UX | Extend (WS-08) |
| Large-file performance (streaming) | 🟡 5MiB budget, 100MB gap | Extend (WS-01) |
| Cross-platform parity + accessibility | 🟡 Windows evidence only; AccessKit via egui available | Extend (WS-18) |
| **(b) AI assist** | | |
| Next-edit prediction model (Tab/Zeta2/NES) | 🟡 deterministic ghost-text provider | Integrate then differentiate (WS-11, WS-19) |
| Inline edits + chat, proposal-gated | 🟡 streaming UI + inline diff substrate validated | Productize (WS-11) |
| Retrieval: repo map / AST chunks / embeddings / agentic search | 🟡 stub embeddings, lexical fallback | Build (WS-10) |
| AGENTS.md / rules files | ❌ | Build (cheap, WS-11) |
| Multi-provider incl. Anthropic native, OpenAI Responses, local | 🟡 OpenAI-compat + Ollama real; Anthropic stub | Extend (WS-09) |
| **(c) Agentic** | | |
| Tool harness w/ schema validation | ✅ substrate (proposal-typed) | Activate (WS-12) |
| Plan mode / editable plans | 🟡 agent Planning state exists; no product surface | Productize (WS-12) ⭐ potential |
| Checkpoints/rollback | 🟡 reversible batch apply + rollback substrate | Productize (WS-07) ⭐ potential |
| OS sandbox (bwrap/Seatbelt) | ❌ (capability broker is app-layer only) | Build (WS-12) |
| Classifier-mediated approvals (Cursor/Codex 2026) | ❌ (binary approve/reject today) | Build on proposal gates (WS-14) ⭐ potential |
| MCP client | ✅ stdio + streamable HTTP | Maintain/track spec (WS-09) |
| Subagents / isolated context | 🟡 DAG coordinator substrate | Productize (WS-13) |
| **(d) Fleet** | | |
| Mission-control UI (Agent HQ/Command Center) | 🟡 design.md specifies it; substrate DAG exists; no UI | Build (WS-13) ⭐ potential |
| Parallel worktree agents | ✅ substrate (worktree orchestrator) | Activate (WS-13) |
| Cloud↔local handoff | 🟡 Cloud Lane HTTP transport exists | Defer post-GA (WS-16) |
| Scheduled/event-triggered agents | ❌ | Defer post-GA |
| ACP multi-vendor agent hosting (Zed/Devin Desktop) | ❌ | Strategic build (WS-13) |
| **(e) Trust/review** | | |
| Proposal-only mutation w/ audit-before-success | ✅⭐ ahead of market | Keep; productize UX (WS-14) |
| Evidence artifacts (Antigravity) | ✅⭐ evidence-gate culture + metadata ledger | Productize UX (WS-14) |
| Diff-first review surface (Zed multibuffer-class) | 🟡 inline diff substrate | Build UX (WS-14) |
| Privacy inspector / egress visibility | ✅⭐ substrate validated | Productize UX (WS-14) |
| Cost/usage analytics | ❌ | Build (WS-09) |
| **Distribution** | | |
| Signed installers, auto-update, crash reporting | ❌/🟡 in progress | Build (WS-17) |

### 4.3 Strategic positioning

1. **Legion's moat is the trust stack — and the market just validated it.** Proposal-gated mutation, metadata-only audit, evidence gates, capability brokering, air-gap mode: every major vendor is now retrofitting weaker versions of these (approval classifiers, firewalls, artifacts). Legion has them as architecture, not features. The plan therefore treats WS-14 (trust/review UX) and WS-13 (fleet console) as the *differentiating* investments, while (a)/(b)-layer gaps are closed with proven off-the-shelf components, not invention.
2. **Do not build what ACP lets you rent.** Zed and Devin Desktop host best-of-breed external agents (Claude Code, Codex) via the Agent Client Protocol. Legion should do the same: its differentiated value is the *control plane around* agents (proposals, evidence, fleet console), which composes with external agents instead of competing with their harnesses. Legion's native agent remains the default and the air-gap option.
3. **Local-first/BYOK is a flank, not a compromise.** The open-source squeeze (Gemini CLI sunset for closed Antigravity CLI; usage-credit drift everywhere) creates demand for a control-first, local-first, BYOK-friendly IDE that enterprises and privacy-conscious developers can trust. Air-gap mode is a genuine enterprise wedge no major vendor offers credibly.
4. **Don't chase a proprietary tab model yet.** Custom next-edit-prediction models (Cursor Tab, Zeta2, NES) are the strongest retention feature but require data flywheels Legion doesn't have. Sequence: integrate open-weight/local prediction (Zeta-class via Ollama) at M2 → instrument acceptance telemetry (consented) → revisit specialist fine-tuning (the existing QLoRA pipeline) post-GA (WS-19).
5. **Keep PR-VSC-002 (VS Code runtime extension host) deferred — likely forever.** The marketplace is legally closed, the API surface is a moat for forks only, and every native editor that matters chose WASM/own models. Legion's WASM + WIT plugin runtime (Phase 5 substrate) is the right architecture; the *new* extension primitive is agent capability (MCP servers, skills) anyway.
6. **Spec-driven development is a natural fit.** Kiro's requirements/design/tasks artifacts and Legion's directive → task-graph → evidence pipeline are the same idea; Legion can make specs first-class proposal objects with almost no architectural change.

---

## 5. Product Definition for GA

### 5.1 Product pillars (unchanged from `docs/LEGION_PIVOT.md` + `mockups/design.md`)

1. **Manual** — deterministic IDE, no AI dependency, fastest path. The credibility foundation.
2. **Assist** — proposal-only AI help: ghost text, inline edits, chat rail, explain/fix/test actions.
3. **Delegate** — bounded disposable workers in sandboxes/worktrees returning proposals + evidence.
4. **Legion Workflows** — directive → task graph → multi-worker execution → validation/risk gates → human sign-off, surfaced as the Kanban fleet console with Directive Console, Agent Comm Stream, Approval Queue, Risk Monitor (per `mockups/design.md`).
5. **Cloud Lane** (post-GA) — opt-in hosted worker capacity with visible scope/cost/cancellation.
6. **Training Flywheel** (post-GA) — opt-in, redacted trace collection feeding specialist model training.

### 5.2 GA scope cut lines

**In scope for GA (M5):** Manual + Assist + Delegate fully; Legion Workflows in supervised beta; Rust as the flagship language (rust-analyzer deep integration) plus tier-2 LSP support for TypeScript/Python/Go via standard servers; macOS + Windows + Linux signed/notarized builds with auto-update and opt-in crash reporting; MCP client; AGENTS.md; native agent + at least one external agent via ACP; WASM plugin runtime activated for grammars/themes/language servers only.

**Out of scope for GA (explicitly deferred, consistent with existing cut lines):** VS Code runtime extension host (PR-VSC-002); production remote development (PR-ENT-001); real-time collaboration GUI (PR-ENT-002); hosted telemetry beyond opt-in crash reports; Cloud Lane productization; custom prediction-model training; voice input; browser/computer-use tooling beyond a Playwright-MCP integration recipe; scheduled/cron agents.

### 5.3 Golden paths (acceptance narratives — each becomes an e2e test + demo script)

- **GP-1 Manual:** open the Legion repo → tree-sitter-highlighted editing with rust-analyzer diagnostics/completions/hover/rename → ripgrep search → integrated terminal `cargo test` → stage hunks → commit. No AI surface visible. (M1 exit)
- **GP-2 Assist:** select a function → "explain" in the assistant rail → request a refactor → streaming inline diff proposal → context manifest shows exactly what left the machine → approve → apply → undo restores cleanly. (M2 exit)
- **GP-3 Delegate:** scope a task ("add tests for X") → worker runs in a sandboxed worktree with terminal/test tool access → returns a multi-file proposal + evidence (test run results) → review in the diff surface → approve → apply → audit ledger shows the full chain. (M3 exit)
- **GP-4 Workflow:** issue a directive → editable plan/spec artifact → task graph fans out to parallel workers on the Kanban board → risk gate pauses on a medium-risk migration → human approves → merge-readiness evidence → commit/PR. (M4 exit)
- **GP-5 Production:** fresh machine → download signed installer → first-run trust/consent/telemetry choices → open a real repo → GP-1 through GP-3 work → auto-update to next version → crash produces an opt-in minidump report. (M5 exit)

---

## 6. Architecture Decision Queue (ADR-0032 .. ADR-0040)

Each decision below blocks specific workstreams; the recommendation column is this plan's position, to be ratified through the normal ADR process. ADRs follow the existing numbering after ADR-0031.

| ADR | Decision | Options | Recommendation | Blocks |
| --- | --- | --- | --- | --- |
| ADR-0032 | **Editor render path** | (a) custom egui widget (own line shaping/`Galley` cache, `show_rows` virtualization, never `TextEdit`); (b) migrate renderer to GPUI; (c) Slint fallback per ADR-0030 | **(a)**, with the renderer kept behind the existing projection boundary so GPUI remains a live fallback re-evaluated every 6 months. egui+AccessKit is currently *ahead* of GPUI/Floem on accessibility, which Legion needs for PR-UI-001. Codify "no `egui::TextEdit` in the code canvas" as a check-deps-style gate. | WS-01, WS-18 |
| ADR-0033 | **Syntax/parse engine** | (a) tree-sitter (incremental, queries, AST chunking, wasm grammar distribution); (b) syntect | **(a)** tree-sitter. Syntect (already a workspace dep) may remain for read-only fallback rendering, then be removed. Grammars compiled to wasm and distributed through the Phase 5 plugin channel. | WS-02, WS-10, WS-15 |
| ADR-0034 | **LSP client architecture** | (a) hand-rolled stdio JSON-RPC client à la Helix/Zed with per-language adapter registry; (b) async-lsp framework; (c) tower-lsp | **(a) with `lsp-types`**, matching how every shipping Rust editor did it and fitting Legion's actor-supervision contracts (ADR-0018). async-lsp acceptable if hand-rolling stalls. rust-analyzer protocol extensions (flycheck, runnables, inlay hints) are in-scope for the flagship language. | WS-03 |
| ADR-0035 | **Terminal stack** | (a) `alacritty_terminal` VTE grid + `legion-platform` PTY; (b) `alacritty_terminal` + `portable-pty`; (c) wezterm-term | **(a)**: keep the existing audited PTY layer (it already passes policy gates), add `alacritty_terminal` for terminal state, custom egui renderer. Structured terminal output feeds the agent harness — a real differentiator. | WS-05, WS-12 |
| ADR-0036 | **Search & index stack** | (a) `grep-searcher`/`ignore`/`globset` in-process + tantivy for indexed search; (b) subprocess ripgrep | **(a)** in-process (no subprocess overhead, policy-inspectable). Tantivy activates under ADR-0005's storage reservations. | WS-06, WS-10 |
| ADR-0037 | **Semantic retrieval** | (a) tree-sitter AST-aware chunking + local embeddings (Ollama/llama.cpp-served) + embedded vector store (LanceDB or sqlite-vec) + Aider-style PageRank repo map, with **agentic search as the default** and the index as enhancement; (b) embeddings-first | **(a)**. Resolves the ADR-0005/ADR-0017 vector deferral. Store model name+version per index; lazy re-embed; repo map is the always-available deterministic fallback. Vector store choice (LanceDB vs sqlite-vec) decided by a 1-week spike. | WS-10 |
| ADR-0038 | **OS sandbox layer** | (a) bubblewrap (Linux) + Seatbelt profile (macOS) + restricted token/AppContainer (Windows, weaker, documented) under the existing capability broker; devcontainer opt-in as the strong tier; (b) app-layer only (status quo); (c) microVMs | **(a)** — matches Codex/Claude Code practice; kernel-enforced FS-write + network-egress policy for all Delegate/Workflow shell execution. App-layer-only enforcement has documented escapes. | WS-12, WS-13, WS-20 |
| ADR-0039 | **Agent interop** | (a) implement ACP host so external agents (Claude Code, Codex, Gemini-class) run inside Legion's proposal/evidence envelope; expose Legion as an MCP server; keep hand-rolled MCP client vs migrate to `rmcp` | **(a) all three**, sequenced: MCP client parity audit vs `rmcp` (migrate if spec rev ~June 2026 breaks the hand-rolled transport), ACP host at M4, Legion-as-MCP-server post-GA. | WS-09, WS-13 |
| ADR-0040 | **Concurrent-edit substrate** | (a) operation/anchor layer now (stable position IDs + version vectors over `legion-text`), full CRDT (Loro/yrs/homegrown SumTree-style) deferred; (b) CRDT core now | **(a)**. Agent+human concurrent editing needs anchors at M3; collaboration needs the CRDT only at post-GA. Retrofitting anchors later is the classic editor-rewrite trigger — do the layer now, cheaply. | WS-01, WS-12, WS-16 |

Also ratify as policy (no new ADR needed): **prompt-cache-stable prompt assembly** (deterministic, append-only prefixes; CI test diffing rendered prompts across runs; telemetry asserts `cache_read_input_tokens > 0`) as part of WS-09, and **provider capability flags** (sampling params, thinking modes, structured-output dialects are per-provider capabilities, not universal knobs) extending ADR-0006.

---

## 7. Workstreams

Conventions: tasks are numbered `WSnn.Tmm`. Every task lists its primary crates and an acceptance signal. "Gate evidence" means an artifact under `plans/evidence/` plus passing phase gates (`check-deps`, `docs-hygiene`, fmt, check, test, clippy, cargo-deny), per the existing process. Dependencies reference ADRs (§6), other tasks, or milestones (§8). Tasks marked 🔴 are on the critical path to M1; 🟠 to M2/M3; 🟢 later.

Standing P0 gate: strict source cleanliness remains active for every workstream. No `todo!()` or `unimplemented!()` may land in `crates/**/*.rs`; any remaining TODO/FIXME/STUB/HACK/XXX text in Rust code must be an intentional fixture or scanner constant with targeted tests. This gate is re-checked before any milestone or readiness-ledger status flip.

### WS-01 — Editor Surface & Rendering Productization

**Objective:** a code canvas that is credible against Zed/Cursor on feel: custom-rendered, virtualized, IME-correct, large-file-safe. **Current state:** projection rendering with theme/token plumbing; GUI phases 1–2 merged; 5MiB snapshot budget; 100MB streaming gap. **Depends:** ADR-0032, ADR-0040. **Exit:** GP-1 editing feel; perf budgets in §11 enforced by CI.

- 🔴 WS01.T1 **Codify the custom-widget boundary.** Audit `legion-desktop` view/bridge code; remove/forbid any `egui::TextEdit` use in the code canvas; define a `CodeCanvasPainter` seam (paint lines + handle input) so the editor core stays renderer-portable per ADR-0032. Add an xtask lint denying `TextEdit` in `legion-desktop` editor modules. *Accept:* gate exists and passes; canvas renders via custom painter only.
- 🔴 WS01.T2 **Line-galley shaping cache.** Per-line shaped-text (`Galley`) cache keyed by (line content hash, font, width), invalidated by buffer edits via snapshot identity; only visible lines shaped per frame using viewport rows. *Accept:* 10K-line file scrolls at vsync with <2ms shaping per frame on reference hardware; bench in CI.
- 🔴 WS01.T3 **Virtualized scrolling + gutter.** Row-virtualized rendering (line numbers, fold indicators, diagnostic/git gutter lanes) driven by `ViewportProjection`; no full-document layout ever. *Accept:* memory and frame-time flat vs. file size for viewport-constant workloads.
- 🔴 WS01.T4 **Input correctness pass.** Keyboard map (incl. existing remapping evidence), mouse selection (word/line/column modes), multi-cursor data model in `legion-editor` + rendering, bracket auto-pairing, indent behavior. *Accept:* scripted input-conformance test suite (extend GUI phase evidence).
- 🔴 WS01.T5 **IME + CJK correctness.** Wire egui IME events end-to-end (composition region rendering, candidate-window positioning); bundle/fallback fonts with CJK coverage; known egui IME issues (e.g., Tab consumed during composition) tracked with upstream links and local workarounds. *Accept:* manual IME test script on all 3 OSes recorded as evidence; automated composition-event tests.
- 🟠 WS01.T6 **Anchor/operation layer (ADR-0040).** Stable position anchors + operation log over `legion-text` snapshots so decorations, diagnostics, AI diff overlays, and concurrent agent edits stay attached across edits. *Accept:* property tests — anchors survive arbitrary edit sequences; agent-edit-while-typing integration test.
- 🟠 WS01.T7 **Large-file streaming mode (PR-UI-002 gap).** Implement the deferred streaming viewport for >5MiB files (chunked load, degraded features, explicit mode badge); un-ignore the 100MB perf workload. *Accept:* 100MB file opens <2s, scrolls smoothly, edits round-trip; the previously ignored benchmark is green and required.
- 🟠 WS01.T8 **Editor polish set:** sticky headers (current fn/scope via tree-sitter from WS-02), code folding execution (descriptors exist), minimap (optional, behind setting), whitespace/indent guides, smooth scrolling setting. *Accept:* settings-gated; GUI evidence updated.
- 🟢 WS01.T9 **Multibuffer/excerpt surface.** Zed-validated pattern: compose excerpts from many files into one editing surface — the substrate for review (WS-14) and project-wide diagnostics. Build as a projection over existing snapshots + anchors. *Accept:* excerpt view with live editing round-trips to source buffers.

### WS-02 — Syntax & Structural Intelligence (tree-sitter)

**Objective:** incremental parsing as the foundation for highlighting, folding, symbols, sticky headers, AST chunking, and repo maps. **Current state:** none wired. **Depends:** ADR-0033. **Exit:** flagship languages highlight incrementally with <5ms re-parse on edit.

- 🔴 WS02.T1 **tree-sitter runtime integration.** Add tree-sitter to an appropriate substrate crate (likely `legion-index` or a new `legion-syntax` crate per dependency policy — requires a dependency-policy entry either way); incremental parse on buffer edits keyed to snapshot identity; background full-parse, foreground edit-window re-parse via the existing semantic work queue. *Accept:* parse-on-edit latency bench; no editor-input blocking (Phase 3 invariant).
- 🔴 WS02.T2 **Highlight query pipeline.** Map tree-sitter highlight captures → existing semantic token kinds → theme colors in `legion-desktop`. Bundle grammars+queries for Rust, TOML, Markdown, JSON, TS/JS, Python, Go, C/C++, YAML, Bash. *Accept:* visual snapshot tests per language; capture→token mapping table documented.
- 🔴 WS02.T3 **Structural features.** Fold ranges, indent computation, bracket matching, symbol outline (file-level), sticky-scope headers — all from tree-sitter queries, replacing the deterministic lexical fallback (which remains as degraded mode for unknown languages). *Accept:* outline matches rust-analyzer's for the Legion repo within tolerance; fallback path still tested.
- 🟠 WS02.T4 **Grammar-as-WASM distribution.** Compile grammars to wasm (wasi-sdk) and load via the Phase 5 plugin channel so new languages ship without rebuilding Legion (Zed's model). Native compiled-in grammars remain for the bundled set. *Accept:* one grammar loads from a plugin artifact behind capability checks.
- 🟠 WS02.T5 **AST-aware chunking API.** Expose tree-sitter-based code chunking (function/class boundaries) from the syntax layer for WS-10 retrieval and WS-12 context assembly. *Accept:* chunker unit tests on fixture repos; recall comparison vs fixed-size chunks recorded.

### WS-03 — Language Intelligence Runtime (LSP)

**Objective:** real language servers, supervised under the existing ADR-0018 contracts, with rust-analyzer as the flagship; stale LSP responses must never mutate buffers or workspaces. **Current state:** complete DTO surface + supervision contracts; no process ever launched. **Depends:** ADR-0034; WS-05.T1 (process plumbing shared); proposal routes (done). **Exit:** PR-LANG-001 product-validated; GP-1.

- 🔴 WS03.T1 **LSP transport + lifecycle.** stdio JSON-RPC client (framing, request/response correlation, cancellation, `$/progress`), server process supervision (spawn via `legion-platform`, crash/restart with backoff, health states) inside the existing actor-owned scheduling. *Accept:* contract tests against a scripted mock server; rust-analyzer initializes against the Legion repo.
- 🔴 WS03.T2 **Document sync + diagnostics.** Incremental `didChange` from editor transactions (UTF-16 mapping already exists in `legion-text`), publishDiagnostics → existing diagnostic projections → gutter/underline rendering + diagnostics panel. *Accept:* introduce an error in the Legion repo → diagnostic appears <1s; cleared on fix.
- 🔴 WS03.T3 **Read-side features.** Completion (with existing stale-snapshot protection), hover, signature help, go-to-def/decl/impl/type-def, find references, document/workspace symbols, semantic tokens (merging with tree-sitter highlights — LSP wins where present), inlay hints, folding ranges. *Accept:* per-feature integration tests against rust-analyzer; completion p95 < 150ms after server warm.
- 🔴 WS03.T4 **Write-side features through proposals.** Rename, code actions, formatting (`textDocument/formatting` + range), organize imports — all materialize as workspace-edit proposals through the accepted Phase 2 routes (ADR-0016/0018 already gate this). *Accept:* rename across files lands as one reviewable, reversible proposal; audit-before-success verified.
- 🔴 WS03.T5 **rust-analyzer extension surface.** flycheck (cargo check/clippy push diagnostics), runnables (run/debug lenses feeding WS-04/WS-05), expand-macro, open-docs, inlay-hint config. *Accept:* runnables for Legion's own tests appear and execute via the terminal runtime.
- 🟠 WS03.T6 **Multi-server registry.** Per-language adapter registry (Zed pattern): server binary resolution (system PATH → downloaded artifact with checksum, policy-gated), per-workspace config, multiple servers per buffer (e.g., rust-analyzer + tailwind). Tier-2: typescript-language-server, pyright/based-pyright, gopls. *Accept:* tier-2 golden-path smoke per language.
- 🟠 WS03.T7 **LSP UX productization (PR-LANG-001).** Completion UI (filtering, snippets, docs panel), code-action lightbulb, hover popovers, references panel, problems panel with project-wide errors. *Accept:* product UX evidence package; keyboard-only operability.
- 🟢 WS03.T8 **Server-binary supply chain.** Checksummed download manifests, offline/air-gap behavior (deny download, allow system binary), version pinning per workspace. *Accept:* air-gap policy test denies downloads; manifest audit recorded.

### WS-04 — Debugging Runtime (DAP)

**Objective:** real debugging for the flagship stack; fixtures retire to test harnesses; stale debug/test responses must never mutate buffers or workspaces. **Current state:** deterministic DAP fixture projections; zero-config Cargo debug locator exists. **Depends:** WS-03.T5 (runnables), WS-05 (terminal for debuggee I/O), ADR-0024 boundaries. **Exit:** PR-LANG-002 debug product-validated.

- 🟠 WS04.T1 **DAP client state machine.** Hand-rolled client (initialize/launch/attach, capabilities negotiation, stopped/continued event flow, threads/stack/scopes/variables requests) with the Zed-style data/UI split; transport over stdio + TCP. *Accept:* scripted mock-adapter conformance suite (reuse fixture as the mock).
- 🟠 WS04.T2 **CodeLLDB adapter for Rust.** Adapter resolution/download (policy-gated as WS03.T8), zero-config launch from the existing Cargo locator + runnables; breakpoints (incl. conditional), step/continue, variable inspection, watch, debug console (REPL evaluate). *Accept:* GP-1 extension — set breakpoint in Legion test, hit it, inspect locals.
- 🟢 WS04.T3 **Tier-2 adapters:** debugpy (Python), delve (Go), js-debug (Node). *Accept:* smoke per adapter.
- 🟢 WS04.T4 **Test explorer productization.** Wire test discovery (runnables/`cargo test --list` and language equivalents) to the existing test-explorer projections; run/debug-at-cursor; failure → jump to assertion. *Accept:* Legion's own suite browsable/runnable in-product.
- 🟢 WS04.T5 **Agent-debuggable sessions.** Expose structured debug state (stack, locals at breakpoint) as harness tool output for Delegate-mode diagnosis. Metadata-only by default per observability policy. *Accept:* agent tool returns redacted-by-policy debug snapshots.

### WS-05 — Terminal Runtime (real PTY)

**Objective:** integrated terminal with structured output the agent harness can consume. **Current state:** real PTY primitives in `legion-platform` (untouched by app); deterministic fixture in `legion-terminal`; policy-gated launch denied by default in beta. **Depends:** ADR-0035, ADR-0026 (substrate accepted). **Exit:** GP-1 terminal; agent tool-grade output capture.

- 🔴 WS05.T1 **Wire PTY to terminal runtime.** Replace the fixture path: `legion-terminal` drives `legion-platform` PTY (spawn shell, resize, input, kill escalation already modeled); `alacritty_terminal`'s `Term` grid consumes output for state. Policy gate stays: terminal launch is a capability decision with audit. *Accept:* interactive shell session in-product on all 3 OSes; kill-escalation test passes against real processes.
- 🔴 WS05.T2 **Terminal renderer.** Custom egui grid renderer (cells, styles, cursor, selection/copy, scrollback, URL detection), 12px terminal type per design tokens. *Accept:* vttest-subset conformance; TUI apps (htop-class) render usably.
- 🔴 WS05.T3 **Shell integration.** OSC 133 prompt marking (command boundaries, per-command exit status), cwd tracking (OSC 7), command-duration display; this is what makes terminal output *structured* for both UI (jump between commands) and agents. *Accept:* command blocks navigable; exit codes attached.
- 🟠 WS05.T4 **Task execution layer.** "Run task" primitive (build/test/run from WS03.T5 runnables and user-defined tasks) executing in dedicated terminal tabs with structured result capture (exit code, duration, output ref) recorded as metadata events. *Accept:* `cargo test` task run emits structured completion event consumed by the test explorer.
- 🟠 WS05.T5 **Agent terminal tool.** Harness tool that executes commands in policy-scoped PTYs (sandbox per ADR-0038 when in Delegate mode), returns bounded/redacted output per observability policy, supports background processes with polling. *Accept:* delegated task runs `cargo test` and cites results in evidence; output redaction tests.
- 🟢 WS05.T6 **PTY production hardening** (known-limitations item): orphan reaping, session cleanup on crash, env hygiene (strip secrets per policy), Windows ConPTY parity audit. *Accept:* hardening evidence package; chaos tests (kill IDE mid-session) leave no orphans.

### WS-06 — Search, Navigation & Command Surface

**Objective:** instant project-wide search and keyboard-first navigation. **Current state:** search projections only. **Depends:** ADR-0036. **Exit:** GP-1 search; command palette covers every registered `CommandDispatchIntent` and stays fully keyboard-operable.

- 🔴 WS06.T1 **In-process ripgrep.** `grep-searcher`/`grep-regex`/`ignore`/`globset` integration in `legion-project` (or `legion-index`) behind the existing search DTOs: literal/regex/case/word modes, include/exclude globs, gitignore awareness, result streaming with bounded batches into the search panel. *Accept:* Legion-repo-wide search < 150ms warm; cancellation works; results stream incrementally.
- 🔴 WS06.T2 **Search & replace with proposals.** Multi-file replace materializes as a reviewable workspace-edit proposal (preview per match, partial selection), through Phase 2 routes. *Accept:* project-wide rename-string lands as one reversible proposal.
- 🔴 WS06.T3 **Fuzzy finder.** File finder (frecency-ranked), symbol finder (file + workspace, from WS-02/WS-03), recent-buffers switcher; one consistent matcher (e.g., nucleo-style scoring) and UI. *Accept:* p95 keystroke-to-results < 30ms on 50K-file fixture.
- 🔴 WS06.T4 **Command palette completion.** Every `CommandDispatchIntent` registered with name/keybinding/context; palette executes the full command set (design.md ⌘K). *Accept:* command coverage report ≥ 95% of intents; palette e2e test.
- 🟠 WS06.T5 **Tantivy indexed search (optional tier).** Background-indexed trigram/full-text search for >100K-file workspaces where live ripgrep degrades; freshness via watcher events. Behind a setting; ADR-0005 storage reservations apply. *Accept:* large-fixture benchmark shows crossover benefit; index staleness bounded by test.

### WS-07 — Proposal Apply Activation & Checkpoints

**Objective:** end Stage 1E — proposals actually mutate the workspace, safely, with checkpoint/rollback as a product feature. This is the keystone task of the whole plan. **Current state:** full lifecycle + reversible batch apply + audit-before-success substrate validated; `runtime_apply_disabled` defaults true. **Depends:** nothing external — this is pure activation + hardening. **Exit:** GP-2/GP-3 apply paths; checkpoints UX.

- 🔴 WS07.T1 **Apply-activation audit.** Enumerate every mutation route (save, text edit, closed-file, workspace-edit, code-action, batch) and its validation gates; write the activation ADR-checklist mapping each gate to a test; then flip `runtime_apply_disabled` default behind a per-workspace trust requirement. *Accept:* gate-by-gate evidence; mutation e2e suite green with apply enabled; fail-closed behavior on stale fingerprints re-verified live.
- 🔴 WS07.T2 **Conflict UX.** Stale-buffer / concurrent-modification / watcher-conflict outcomes get product surfaces (three-way view where applicable, retry-with-rebase for text edits via anchors from WS01.T6). *Accept:* scripted conflict scenarios produce actionable UI, never silent loss.
- 🟠 WS07.T3 **Checkpoints (product feature).** Named restore points over the existing reversible-batch substrate + editor undo history: auto-checkpoint before any AI proposal apply and before batch operations; timeline UI; restore preserves subsequent manual edits where non-conflicting (Cursor-validated pattern). *Accept:* apply → edit → restore-checkpoint scenario preserves the manual edit; checkpoint ledger audited.
- 🟠 WS07.T4 **Workspace snapshot isolation for agents.** Delegate-mode applies target the worker's worktree (existing orchestrator), never main workspace; promotion from worktree to main is itself a proposal. *Accept:* worktree → proposal → main flow e2e; direct-write attempts from worker context denied + audited.

### WS-08 — Git & VCS Productization

**Objective:** first-class in-editor git matching 2026 table stakes, with worktrees as agent infrastructure. **Current state:** real CLI status/blame/diff data; Git/jj view projections substrate-tested; thin product UX. **Depends:** WS-01 (gutter), WS-14 (diff surface shares components). **Exit:** GP-1 commit path.

- 🔴 WS08.T1 **Gutter diff + inline blame.** Live index/HEAD diff markers in the gutter (added/modified/deleted lanes), current-line inline blame (Zed pattern), hunk navigation. *Accept:* edits reflect in gutter <100ms after keystroke settle.
- 🔴 WS08.T2 **Stage/commit UX.** Status panel (staged/unstaged/untracked), hunk and range staging (CLI `git apply --cached` of constructed patches initially), commit editor with message validation, push/pull/fetch with auth (SSH agent + credential helper passthrough). *Accept:* GP-1 commit e2e on a fixture repo; auth paths documented.
- 🟠 WS08.T3 **Branch/worktree manager.** Branch switcher (status bar per design.md), create/switch/delete, stash; **worktree panel** listing agent worktrees (from `legion-agent` orchestrator) and manual worktrees with cleanup actions. *Accept:* agent worktrees visible/manageable; orphan-worktree GC test.
- 🟠 WS08.T4 **gix hot-path adoption.** Replace CLI for status/diff/blame with `gix` for latency (keep CLI/git2 for push/rebase/complex merges per 2026 maturity); benchmark before/after. *Accept:* status latency improvement recorded; behavior parity tests vs CLI output.
- 🟢 WS08.T5 **PR integration (forge-agnostic core).** Create branch + commit + push + open-PR URL flow; GitHub/GitLab API adapters behind a forge trait, policy-gated network. *Accept:* GP-4 ends in a real PR on a test repo.
- 🟢 WS08.T6 **jj (jujutsu) exploration spike.** Evaluate `jj-lib` for the operation-log/undo model as agent-edit insurance; report only, no commitment. *Accept:* spike doc with go/no-go criteria.

### WS-09 — AI Provider Plane

**Objective:** production-grade multi-provider client layer with cost discipline. **Current state:** real Ollama/llama.cpp/OpenAI-compatible; Anthropic stub; MCP client working; air-gap policy enforced. **Depends:** ADR-0039; capability-flag policy (§6). **Exit:** GP-2 streaming via hosted + local providers; cost meter.

- 🟠 WS09.T1 **Native Anthropic Messages client.** Streaming SSE (`message_start`/`content_block_delta`/`message_delta`), tool use with `strict` schemas, structured outputs via `output_config.format` (json_schema), `count_tokens` endpoint (never local tokenizer guessing), extended/adaptive thinking handled via capability flags, errors/retries/overload backoff. *Accept:* contract tests against recorded fixtures + live smoke; streaming renders token-by-token in the rail.
- 🟠 WS09.T2 **Prompt caching discipline.** Deterministic prompt assembly: byte-stable system/tool prefixes, append-only message history, ≤4 cache breakpoints placed by policy; CI test renders the same logical prompt twice and diffs bytes; telemetry asserts cache reads on warm sessions. *Accept:* warm-session input-token cost reduced ≥80% in instrumented runs.
- 🟠 WS09.T3 **OpenAI Responses + compatible consolidation.** Native Responses API client (stateful, built-in tools where allowed by policy); the OpenAI-compatible client formally designated the dialect for Ollama/LM Studio/OpenRouter/other; provider capability matrix (sampling, tools, structured output, vision, context length) in `legion-protocol`. *Accept:* same harness task runs across Anthropic/OpenAI/Ollama with capability-aware degradation.
- 🟠 WS09.T4 **Hosted-provider activation gates (PR-AI-001 completion).** First-class consent flow: per-workspace provider enablement, privacy inspector shows exact egress (context manifest), keys in `legion-retention` keyring store, air-gap mode hard-denies. *Accept:* hosted call impossible without recorded consent; egress matches manifest byte-for-byte in test.
- 🟠 WS09.T5 **Cost & usage analytics.** Per-request token/cost accounting (provider-reported usage), per-task and per-workflow rollups, budget limits with kill-switch hooks into the Phase 13 gates; usage panel UI. *Accept:* budget breach pauses a workflow in test; costs visible per proposal.
- 🟠 WS09.T6 **MCP client GA + rmcp decision.** Parity audit of the hand-rolled client vs `rmcp` against spec rev 2025-11-25; OAuth for streamable-HTTP servers; server allowlist policy (org-style controls); tool-permission UI consistent with capability broker. Migrate to `rmcp` if the ~June 2026 spec rev breaks transports. *Accept:* 3 reference servers (filesystem-class, web-class, custom) pass conformance; permission prompts audited.
- 🟢 WS09.T7 **Batch lane.** Anthropic Batch (and OpenAI equivalent) for offline jobs — bulk summarization/indexing in WS-10 at 50% cost. *Accept:* repo-summary batch job round-trips.

### WS-10 — Context Engine (retrieval, repo map, memory)

**Objective:** give models codebase awareness with agentic search as the default and indexes as enhancement, resolving the long-deferred vector question (ADR-0037). **Current state:** stub embeddings, lexical fallback, metadata-only memory with consent. **Depends:** ADR-0037, WS-02.T5, WS-06.T1, WS-09. **Exit:** GP-2 context manifest quality; retrieval eval baseline.

- 🟠 WS10.T1 **Agentic search tools.** Expose grep/glob/read/outline as harness tools (policy-scoped, metadata-audited) — the Claude-Code-validated default that needs no index and can't go stale. *Accept:* harness answers codebase questions on the Legion repo using only these tools; tool-call audit complete.
- 🟠 WS10.T2 **Repo map.** Aider-validated structural map: tree-sitter defs/refs → file/symbol graph → PageRank → top-ranked signatures within a token budget; deterministic, cheap, always available; cached with watcher invalidation. *Accept:* map for Legion repo fits budget and names the right files for 10 scripted queries.
- 🟠 WS10.T3 **Embedding pipeline (local-first).** AST-chunk (WS-02.T5) → local embedding model via Ollama/llama.cpp (policy: hosted embeddings only with consent) → embedded vector store per ADR-0037 spike (LanceDB vs sqlite-vec); model name+version stored per index; lazy re-embed on model change. *Accept:* index builds incrementally from watcher events; air-gap works fully.
- 🟠 WS10.T4 **Hybrid retrieval + eval.** Lexical (WS-06) + vector + repo-map fusion with rank blending; retrieval eval fixture (queries → expected files/symbols) wired into `evals/` so retrieval quality is a tracked number, not vibes. *Accept:* hybrid beats each single method on the eval; eval runs in CI (offline fixtures).
- 🟠 WS10.T5 **Context manifest UX (PR-AI-001).** The inspector that shows exactly what context was assembled (files, symbols, diagnostics, terminal excerpts, memory, privacy labels, and egress status) *before* invocation, with per-item exclusion. This is a trust differentiator — productize it prominently. *Accept:* GP-2 manifest interaction; manifest-to-egress equality test.
- 🟢 WS10.T6 **Memory productization.** AGENTS.md ingestion (WS-11.T4) + consented workspace memory (existing candidate/consent substrate) surfaced and editable; compaction policy for long sessions. *Accept:* memory survives restart; deletion handles verified.

### WS-11 — Assist Surfaces (inline AI)

**Objective:** the Assist mode of `docs/MODES.md` as a daily-driver product: ghost text, inline edits, chat rail — all proposal-gated. **Current state:** deterministic inline-prediction provider; streaming UI + inline diff substrate validated; no product loop. **Depends:** WS-09, WS-10, WS-07. **Exit:** GP-2.

- 🟠 WS11.T1 **Ghost-text completions.** Inline prediction loop: debounced context assembly (local window + repo-map header), provider call (local model default; Zeta-class open-weight via Ollama as reference config), render as ghost text, Tab-accept/word-accept/Esc-reject; acceptance telemetry (consented, metadata-only). *Accept:* p95 keystroke-to-suggestion < 400ms local; acceptance events audited.
- 🟠 WS11.T2 **Inline edit ("⌘K-class").** Selection or cursor-scoped instruction → streaming diff overlay anchored via WS01.T6 → accept/reject per hunk → applies as a text-edit proposal (auto-approved within the open buffer per Assist policy, still audited). *Accept:* GP-2 refactor scenario; undo integrates with editor history.
- 🟠 WS11.T3 **Assistant rail.** Right-rail chat per design.md: streaming markdown, code blocks with apply-as-proposal buttons, context chips (current file/selection/manifest link), model picker with capability flags, slash-commands (explain/fix/test/doc). *Accept:* rail e2e with two providers; all writes route through proposals.
- 🟠 WS11.T4 **Rules & instruction files.** AGENTS.md (standard) + `.legion/rules` ingestion into deterministic prompt prefixes (cache-stable per WS09.T2); per-workspace and per-user layers; visible in the context manifest. *Accept:* rules demonstrably alter behavior in eval fixture; manifest shows them.
- 🟢 WS11.T5 **Next-edit prediction (location).** Predict-next-edit-location assist (NES-class, scoped down): after an edit, suggest the next cursor target via cheap heuristics + model assist. Defer custom model training to WS-19. *Accept:* opt-in setting; precision tracked in telemetry.

### WS-12 — Agent Harness & Delegate Mode

**Objective:** activate the Phase 4 substrate into a real execution harness: tools, sandbox, plan mode — Legion's native agent running scoped tasks end-to-end. **Current state:** state machine, worktree orchestrator, proposal generator, DAG coordinator all real but metadata-only; no tool execution. **Depends:** ADR-0038, WS-05.T5, WS-07, WS-09, WS-10. **Exit:** GP-3.

- 🟠 WS12.T1 **Tool registry & execution loop.** Schema-validated tool set (read/grep/glob/outline from WS-10.T1; edit-as-proposal; terminal from WS-05.T5; MCP passthrough from WS-09.T6), executed inside the agent state machine with the existing capability broker mediating every call; per-mode tool allowlists from `docs/MODES.md`. *Accept:* harness conformance suite (tool-call validation, error feedback, retry); every call audited with causality chain.
- 🟠 WS12.T2 **OS sandbox layer (ADR-0038).** bubblewrap (Linux) + Seatbelt profile (macOS) wrapping Delegate/Workflow shell execution: FS write scope = worktree, network = policy-resolved allowlist via proxy; Windows: restricted token/AppContainer + documented weaker guarantee; devcontainer opt-in as the strong tier. *Accept:* escape-attempt test suite (write outside scope, raw egress) fails closed and audits; sandbox status visible in UI.
- 🟠 WS12.T3 **Plan mode as spec artifacts.** Directive → generated plan (requirements/tasks) as an *editable, diffable proposal object* before execution (Kiro/Jules-validated; natural fit for Legion's proposal model); approved plan becomes the task graph input. *Accept:* GP-4 starts from an edited plan; plan revisions audited.
- 🟠 WS12.T4 **Delegate golden path.** Single scoped task end-to-end: scope picker (files/module/repo + risk tolerance per design.md §8.5), worker in sandboxed worktree, tool loop with live status, evidence collection (test runs, diffs, decisions), returned proposal bundle into the review surface. *Accept:* GP-3 e2e on a fixture repo and on Legion itself.
- 🟠 WS12.T5 **Context management for long runs.** Token budgeting, history compaction with metadata retention, cache-aware prompt layout (WS09.T2), context-usage meter in UI (Cursor-validated transparency). *Accept:* 100-tool-call session stays within budget; compaction preserves task fidelity in eval.
- 🟠 WS12.T6 **Failure & recovery UX.** Stuck/looping detection (no-progress heuristics), error-state surface per design.md §15.2 (failed command, suspected cause, recovery plan, approval request), cancellation that reaps sandbox processes. *Accept:* chaos tests (kill tool mid-run, poison output) produce controlled intervention states.
- 🟢 WS12.T7 **Subagent fan-out.** Bounded subagents with isolated context (separate provider sessions) under one parent budget/policy — the substrate for WS-13 parallelism. *Accept:* parent/child audit chain; budget inheritance test.

### WS-13 — Legion Workflows & Fleet Console

**Objective:** the defining product surface — directive-driven multi-agent workflows on a mission-control Kanban (design.md §9.5, §10.5), the layer Agent HQ/Devin Desktop/Antigravity validated. **Current state:** DAG coordinator, conflict detection, merge-readiness substrate; Phase 13 accepted; no UI, no real execution. **Depends:** WS-12 complete; ADR-0039 (ACP). **Exit:** GP-4 (M4 beta).

- 🟢 WS13.T1 **Workflow runtime activation.** Wire `LegionWorkflowCoordinator` to real workers (WS-12): dependency-lane scheduling, parallel worktree execution, worker evidence aggregation, conflict detection on overlapping file claims, merge-readiness gates. *Accept:* 3-task DAG with one dependency executes with 2 parallel workers; conflict scenario pauses correctly.
- 🟢 WS13.T2 **Fleet console UI.** Kanban board (Assigned/In Progress/Waiting on Human/Testing/Done), task cards per design.md §10.5 (agent, model badge, progress, files, risk, tests, mini-diff), task inspector, Directive Console (input, objective, scope, constraints), Agent Comm Stream (PLAN/WRITE/TEST/REVIEW/ERROR/APPROVAL/COMPLETE tags), Risk Monitor. *Accept:* GP-4 fully drivable from the console; projection-only invariant audited.
- 🟢 WS13.T3 **Approval queue & risk gates.** Product surface for Phase 13 gates: approval items (action, owner, risk badge, files, approve/review/reject), risk classification per proposal (see WS-14.T4), budget/kill-switch controls, sign-off and merge-readiness ceremony. *Accept:* medium-risk gate pauses GP-4; kill switch halts fleet < 2s with sandbox reaping.
- 🟢 WS13.T4 **ACP host (ADR-0039).** Implement Agent Client Protocol hosting so external agents (Claude Code, Codex-class, Gemini-class) run as Legion workers: their edits land as proposals, their shell runs in Legion sandboxes, their activity feeds the Comm Stream. This converts competitors' harnesses into Legion's supply. *Accept:* one external agent completes GP-3 inside the Legion envelope.
- 🟢 WS13.T5 **Workflow review/replay.** Post-run report (files changed, tests, decisions feed, cost) per design.md §15.3; metadata replay of the run from the existing replay manifests for audit. *Accept:* replay reconstructs the run timeline from metadata alone.

### WS-14 — Trust & Review Surfaces (the differentiator)

**Objective:** make Legion's invisible safety architecture *visible and pleasurable* — the diff-first review experience plus graduated approvals. **Current state:** inline diff substrate, run ledger, semantic provenance, rollback-linked proposals all substrate-validated; binary approve/reject only. **Depends:** WS-01.T9 (multibuffer), WS-07. **Exit:** GP-2/3/4 review experiences; PR-AI-002 safety half.

- 🟠 WS14.T1 **Proposal review surface.** Multibuffer-based multi-file diff review (original/proposed, inline highlights per design.md §11.2, file tabs, change summary bar): per-hunk accept/reject/edit-in-place (editable diffs — Zed-validated), agent annotation bubbles from proposal metadata. *Accept:* GP-3 review of a 5-file proposal entirely keyboard-driven; partial acceptance produces a correct derived proposal.
- 🟠 WS14.T2 **Trust strip & mode surfaces.** Always-visible mode badge + Product Mode Switch (design.md §8) with confirmation flows and permission toggles; mode transitions re-evaluate policy and visibly change available surfaces (Manual hides AI chrome entirely per MODES.md). *Accept:* mode-policy conformance tests; Manual-mode network-silence test (zero provider sockets).
- 🟠 WS14.T3 **Evidence artifacts UX.** Proposal bundles carry first-class evidence (test results, command transcripts as metadata + consented excerpts, plan lineage, provenance) rendered in review; exportable evidence report per run (the Antigravity "artifacts" pattern Legion's substrate already supports). *Accept:* GP-3 evidence visible at review time; export round-trips.
- 🟠 WS14.T4 **Graduated approvals (classifier-mediated).** Replace binary approve/reject with risk-tiered policy: deterministic rules first (path scope, file count, deletion ratio, dependency-file touch, migration/SQL detection, secret-pattern proximity), optional model-assisted classification as a *recommender* — final authority remains the human + policy, consistent with Legion's promise. Auto-approve only within explicitly configured low-risk envelopes. *Accept:* risk-rule unit suite; envelope config respected; every auto-approval audited with rule citation.
- 🟠 WS14.T5 **Privacy inspector productization.** Egress viewer (what left, to whom, when, under which consent), redaction status, retention handles with working deletion — completing the substrate's promise as UX. *Accept:* PR-AI-001 product validation evidence.
- 🟢 WS14.T6 **AI review agent (second-opinion).** Optional reviewer worker (Amp-oracle pattern: stronger model reviews proposals) producing annotations into WS14.T1; never an approver, only an advisor. *Accept:* reviewer flags a seeded bug in eval fixture.

### WS-15 — Extension Runtime (WASM) & Distribution Channel

**Objective:** activate Phase 5's plugin boundary with a real VM, scoped to the Zed-validated launch set: grammars, themes, language-server adapters. **Current state:** manifest validation, capability gates, quotas — no VM. **Depends:** ADR-0019 (accepted), WS-02.T4. **Exit:** plugin-delivered grammar + theme + LSP adapter in production.

- 🟢 WS15.T1 **wasmtime + WIT host.** Versioned WIT interface (v0 surface deliberately tiny), wasmtime execution under the existing capability/quota gates, crash containment + blame (plugin crash never takes the IDE), and no arbitrary workspace read/write outside declared policy. *Accept:* hostile-plugin test suite (loops, OOM, capability probing, workspace-access attempts) contained and audited.
- 🟢 WS15.T2 **Launch extension set.** Ship 2–3 bundled capabilities *as extensions* (a tier-2 grammar, a theme, an LSP adapter) to dogfood the API before any third party. *Accept:* bundled set runs via the VM path in CI.
- 🟢 WS15.T3 **Distribution & trust.** Signed extension artifacts, checksum manifests, install/update/remove UX (known-limitations item), permission review screen per manifest capabilities. Extension-originated edits become proposals before preview/apply; marketplace/runtime execution remains off until policy and sandbox tests pass. *Accept:* tampered-artifact rejection test; permission UI evidence.
- 🟢 WS15.T4 **Agent-capability marketplace position.** MCP servers + skills + plan templates as the primary "marketplace" objects (the 2026 extension primitive), curated registry format defined; full marketplace post-GA. *Accept:* registry schema + local install flow.

### WS-16 — Collaboration & Remote (post-GA track, kept warm)

**Objective:** preserve optionality without GA cost; activate only after M5. **Current state:** operation-log collaboration (in-memory), TLS transport, remote fixtures, Cloud Lane HTTP transport — all substrate. **Depends:** ADR-0040 anchor layer; post-GA CRDT decision.

- 🟢 WS16.T1 **CRDT adoption ADR.** Decide Loro vs yrs vs homegrown over the anchor layer; prototype on the operation-log runtime. *Accept:* ADR with benchmark evidence.
- 🟢 WS16.T2 **Remote transport activation.** Drive `legion-remote-transport` from the remote runtime against a reference edge agent; reconnect/offline-resume from existing manifests; production transport activates only with policy, threat-model, mock/default-deny, and failure-mode evidence. *Accept:* remote GP-1 subset over TLS on LAN fixture.
- 🟢 WS16.T3 **Cloud Lane productization.** Hosted worker capacity with visible upload scope, budget, cancellation (existing HTTP transport + contract docs as the base). *Accept:* cloud-executed Delegate task with full egress visibility.

### WS-17 — Distribution, Updates & Crash Reporting (PR-REL-001)

**Objective:** installable, updatable, supportable product on three platforms. **Current state:** deterministic Windows package path + dry-run evidence only. **Depends:** none (can start immediately, parallel). **Exit:** GP-5.

- 🔴 WS17.T1 **Release pipeline.** cargo-dist-based multi-platform CI (plan/build/host) producing dmg + msi (WiX) + deb/rpm/AppImage; reproducible version stamping; release-channel model (stable/preview); installer descriptors record name, platform, sha256, build command, verification command, and signer status (`dry-run/no-production-signer` until real signing exists). *Accept:* tagged commit yields all artifacts in CI; dry-run descriptors are verifiable.
- 🔴 WS17.T2 **Signing & notarization.** macOS Developer ID + notarization + stapling via rcodesign (pure-Rust, runs on Linux CI with App Store Connect API key); Windows Authenticode (Azure Trusted Signing or EV cert); Linux artifact signatures. No private signing keys, certs, tokens, or notarization credentials may be committed. *Accept:* Gatekeeper/SmartScreen-clean installs verified on fresh VMs, or an explicitly unsigned-beta policy is recorded before any readiness-ledger status flip.
- 🟠 WS17.T3 **Auto-update + rollback.** Updater (Velopack or custom Zed-style) with Ed25519-signed manifests, delta updates where supported, staged rollout percentage, one-click rollback to previous version (existing update/rollback incident evidence extends here). *Accept:* update → rollback e2e on all 3 OSes.
- 🟠 WS17.T4 **Crash reporting (opt-in).** crash-handler + minidumper out-of-process capture → sentry-rust-minidump (or self-hosted endpoint per privacy posture); symbol upload in release CI; first-run consent with visible toggle, consistent with telemetry policy. *Accept:* induced crash produces symbolicated report only when consented.
- 🟠 WS17.T5 **First-run & onboarding.** Trust prompt for workspace, telemetry/crash consent, provider setup (local-first default; BYOK optional), keybinding scheme choice, interactive tour of mode switch. *Accept:* GP-5 first-run path usability-tested.
- 🟢 WS17.T6 **Docs & support surface.** User docs site, keyboard reference, troubleshooting (logs/diagnostics export already exists), issue-template diagnostics bundle, GA release runbook closure after package commands and artifacts exist. *Accept:* docs cover every GP path; `plans/product-readiness-ledger.md` moves `PR-REL-001` only after signed-installer or explicitly unsigned-beta evidence exists.

### WS-18 — Performance, Accessibility & Platform Parity

**Objective:** enforce the latency/accessibility budgets that justify "native"; reach 3-OS parity (known limitation: Windows-only evidence). **Current state:** substrate budgets validated (p50/p95 input-to-paint, IME, clipboard, focus, high-contrast, screen-reader projections); product UX paths unvalidated. **Depends:** WS-01; runs continuously. **Exit:** §11 budgets in CI; PR-UI-001 product-validated.

- 🔴 WS18.T1 **Performance harness in CI.** Automated input-to-paint p50/p95, scroll jank, startup time, memory ceiling on reference workloads (incl. Legion repo + 100K-file fixture + 100MB file), per-OS runners; regressions block merge. *Accept:* dashboards + failing-gate demonstration.
- 🟠 WS18.T2 **AccessKit product pass.** Screen-reader walkthrough of every GP path (NVDA/VoiceOver/Orca), focus order, live-region announcements for agent events, high-contrast + reduced-motion themes, full keyboard operability audit. *Accept:* OS accessibility-tree inspection evidence (closing the known limitation), scripted SR tests where feasible.
- 🟠 WS18.T3 **Platform parity matrix.** macOS/Linux feature-parity validation for everything with Windows-only evidence (rendering, IME, watcher, PTY/ConPTY, keyring, menus/shortcuts conventions, file dialogs); parity ledger per feature. *Accept:* GP-1..3 evidence on all three OSes.
- 🟢 WS18.T4 **Multi-window/multi-monitor + DPI.** Validate the recorded gap: per-monitor DPI, window restore, detachable panels (if kept in scope). *Accept:* multi-monitor smoke evidence.

### WS-19 — Evals, Benchmarks & Training Flywheel

**Objective:** measure the agent product like 2026 leaders do (harness-aware evals), and keep the specialist-model option alive. **Current state:** eval/training harnesses are dry-run/fixture scaffolds (PR-AI-002 evals deferred); QLoRA + GGUF pipelines exist. **Depends:** WS-12; consent substrate. **Exit:** internal eval suite gating agent changes.

- 🟠 WS19.T1 **Legion-Bench v0.** Internal harness-aware eval suite (VSC-Bench pattern): 20–50 scoped tasks on fixture repos (bug fix, test-add, refactor, multi-file feature) scored by gates (tests pass, diff scope, cost, turns); runs offline in CI with recorded providers, live weekly with real ones. *Accept:* baseline numbers published in evidence; regressions visible per harness change.
- 🟠 WS19.T2 **Safety/adversarial evals (PR-AI-002).** Prompt-injection fixtures (hostile file contents, malicious tool outputs, exfiltration lures) asserting the proposal/sandbox/redaction gates hold; convert the existing dry-run scaffolds into enforced tests. *Accept:* injection suite green; new gates added for any found bypass.
- 🟢 WS19.T3 **External benchmark posture.** Periodic SWE-bench-Pro / Terminal-Bench-2.0-style runs of the Legion harness (with chosen models) for honest positioning; publish harness config alongside scores per 2026 norms. *Accept:* first report produced.
- 🟢 WS19.T4 **Telemetry-to-flywheel (consented).** Acceptance/rejection signals from WS-11/WS-14 (metadata-only by default; raw traces only under the Phase 8 consent/redaction/deletion substrate) accumulate into training corpora; QLoRA pipeline graduates from fixture-smoke to scheduled runs producing evaluated specialist candidates. *Accept:* end-to-end consented trace → trained adapter → Legion-Bench comparison, fully reproducible.

### WS-20 — Security Hardening & Enterprise Policy

**Objective:** make the trust story externally credible. **Current state:** strong internal policy substrate; no external validation; supply-chain gates exist (cargo-deny). **Depends:** WS-12.T2; continuous. **Exit:** published security model + audit.

- 🟠 WS20.T1 **Threat model & security docs.** Public-facing security model (mutation gating, sandbox guarantees incl. honest Windows caveats, egress policy, secret handling, plugin isolation); responsible-disclosure policy. *Accept:* doc reviewed against implementation by adversarial pass.
- 🟠 WS20.T2 **Secret hygiene.** Secret-pattern scanning on proposal content and terminal excerpts before any consented retention/egress (substrate hooks exist); redaction conformance tests. *Accept:* seeded-secret corpus never egresses unredacted.
- 🟢 WS20.T3 **Org policy pack.** Admin-distributable policy bundles (provider/MCP/tool allowlists, mode ceilings, budget caps, retention rules, raw-source export rules) — signed, versioned; the PR-ENT-002 admin slice that doesn't require collaboration. Admin export is metadata-safe by default and policy-governed for raw-source inclusion. *Accept:* policy bundle enforcement e2e.
- 🟢 WS20.T4 **External audit + pen test** before GA marketing claims. *Accept:* findings triaged; report summarized publicly.

---

## 8. Milestones & Sequencing

Dependency spine: **WS-01/02 → WS-03/05/06 → (M1) → WS-09/10/11 + WS-07 → (M2) → WS-12/14 → (M3) → WS-13 → (M4) → WS-17/18 close-out → (M5) → WS-15/16/19/20 expansions → (M6)**. WS-17 and WS-18 start early and run in parallel throughout; WS-07.T1 (apply activation) lands inside M2 but its audit prep starts at M0.

| Milestone | Name | Contents (primary) | Hard exit criteria |
| --- | --- | --- | --- |
| **M0** (~2–4 wks) | Plan lock | Ratify this plan + ADR-0032..0040 drafts; CI additions (perf harness skeleton WS18.T1, no-TextEdit gate WS01.T1); WS-17.T1 pipeline bootstrap; ADR-0037 vector-store spike | Plan + ADRs accepted in-repo; gates running on main |
| **M1** (~8–12 wks) | **Credible Editor** (Manual mode alpha) | WS-01.T1–T5, WS-02.T1–T3, WS-03.T1–T5, WS-05.T1–T3, WS-06.T1–T4, WS-08.T1–T2, WS-17.T2 initial | **GP-1 on all 3 OSes**; **dogfooding gate: Legion developed in Legion for ≥1 week by every contributor**; §11 editor budgets green |
| **M2** (~6–10 wks) | **Assist** (private beta) | WS-07.T1–T3, WS-09.T1–T4, WS-10.T1–T5, WS-11.T1–T4, WS-14.T2, WS-01.T6, WS-02.T5 (prereq of WS-10.T3) | GP-2; apply enabled by default for trusted workspaces; manual + assist daily-drivable; cache-discipline test green |
| **M3** (~8–12 wks) | **Delegate** (public beta) | WS-12.T1–T6, WS-14.T1/T3/T4/T5, WS-01.T9 (prereq of WS-14.T1), WS-09.T6 (prereq of WS-12.T1 MCP passthrough), WS-03.T7, WS-05.T4–T5, WS-07.T4, WS-04.T1–T2, WS-08.T3, WS-19.T1–T2 | GP-3 incl. on Legion itself; sandbox escape suite green; Legion-Bench baseline published; injection suite green |
| **M4** (~6–10 wks) | **Legion Workflows beta** | WS-13.T1–T5, WS-12.T7, WS-09.T5, WS-08.T5, WS-10.T6 | GP-4; fleet kill-switch < 2s; one external agent via ACP completes GP-3 in-envelope |
| **M5** (~6–8 wks) | **Production GA** | WS-17 complete, WS-18 complete, WS-15.T1–T3, WS-01.T7–T8, WS-02.T4, WS-03.T6/T8, WS-04.T4, WS-05.T6, WS-20.T1–T2, docs | **GP-5**; signed/notarized installers + auto-update/rollback on 3 OSes; accessibility evidence; crash reporting opt-in working; readiness ledger flips PR-UI/LANG/AI/REL to product-validated |
| **M6** (ongoing) | Expansion | WS-16 (cloud lane, remote, collaboration), WS-15.T4 marketplace, WS-19.T3–T4 flywheel, WS-20.T3–T4, WS-04.T3/T5, WS-06.T5, WS-08.T4/T6, WS-09.T7, WS-11.T5, WS-14.T6, custom prediction model exploration, scheduled agents, voice | Per-feature ADRs + gates as established |

Duration ranges assume 1–3 focused engineers with heavy agentic leverage; they compress with parallel staffing on the WS-03/WS-05/WS-06 fan-out and stretch if WS-01 (editor feel) needs iteration. **Sequencing is the commitment; calendar is the estimate.** Appendix C carries the complete task→milestone matrix; every task ID in §7 appears there exactly once, and any future task addition must update it in the same PR.

---

## 9. Integration Plan (process, branches, gates)

1. **One workstream = one tracked epic**; tasks land as PRs referencing `WSnn.Tmm` IDs. Branch naming `ws/<nn>-<slug>`; merge to `main` only with phase gates green (existing list) plus the new CI gates as they come online (perf harness, no-TextEdit lint, prompt-stability test, retrieval/Legion-Bench fixtures offline).
2. **Evidence discipline continues.** Each milestone closes with an evidence package under `plans/evidence/production/<milestone>/` mirroring the GUI-productization format; the product-readiness ledger is updated in the same PR that claims a gate.
3. **ADR-first for boundary changes.** Tree-sitter/syntax crate placement, sandbox layer, WASM VM, ACP host, and any new renderer dependency all require dependency-policy updates + ADR acceptance *before* code merges (per `plans/dependency-policy.md`).
4. **Activation flags retire deliberately.** `runtime_apply_disabled`, `fixture_enabled`, and remote/telemetry flags each get an activation checklist (tests proving fail-closed behavior, audit coverage, UX surface) and a removal PR — flags don't linger as dead config.
5. **Dogfooding is a standing gate from M1.** A "Legion-on-Legion" weekly journal (friction log) feeds the backlog; any P0 friction item preempts roadmap work.
6. **Upstream hygiene.** egui/AccessKit/tree-sitter/alacritty_terminal issues encountered get tracked links in-repo; budget ~10% time for upstream fixes rather than local forks.

Required verification commands at the end of every packet that changes code or evidence, using narrower targeted tests first when useful:

```bash
cargo run -p xtask -- check-deps
cargo run -p xtask -- docs-hygiene
cargo fmt --all --check
cargo check --workspace --all-targets
cargo test --workspace --all-targets
cargo clippy --workspace --all-targets -- -D warnings
cargo deny check
```

---

## 10. Risk Register

| # | Risk | Likelihood | Impact | Mitigation |
| --- | --- | --- | --- | --- |
| R1 | **egui can't hit editor feel/IME bar** even with custom widget | Med | High | ADR-0032 keeps renderer behind projection boundary; GPUI fallback re-evaluated semi-annually; IME issues tracked upstream with workarounds; accessibility currently *favors* egui |
| R2 | **Scope drowning** (this plan is large; team is small) | High | High | Milestone cut lines are hard; M6 list is a parking lot; dogfooding gate forces value-ordering; "activate, don't invent" rule for (a)/(b)-layer gaps |
| R3 | **Apply activation regression** (Stage 1E flip corrupts user data) | Low | Critical | WS07.T1 gate-by-gate audit; fail-closed saves already proven; checkpoints before AI applies; staged rollout via preview channel |
| R4 | **Sandbox false confidence** (esp. Windows) | Med | High | Honest tiering in docs (WS20.T1); escape-test suite in CI; devcontainer strong tier; never market uniform guarantees |
| R5 | **LSP integration depth underestimated** (rust-analyzer extensions, multi-server) | Med | Med | Flagship-language-first; tier-2 behind adapters; reuse Helix/Zed-proven patterns; mock-server contract tests |
| R6 | **MCP/ACP spec churn** (transport rev ~June 2026) | Med | Low | Transport behind own trait; rmcp migration pre-decided (ADR-0039); stdio-first |
| R7 | **Provider economics** (agent loops are expensive without cache discipline) | Med | Med | WS09.T2 cache-stability CI; budgets/kill-switches (WS09.T5); local-first defaults |
| R8 | **Competitive velocity** (weekly releases by funded rivals) | High | Med | Compete on trust stack + local-first + ACP composition, not feature count; don't chase tab-model/voice/browser in GA scope |
| R9 | **Prediction-quality gap** (no proprietary tab model) | High | Med | Open-weight/local models at M2; acceptance telemetry; flywheel option preserved (WS-19.T4); position on control not magic |
| R10 | **Single-platform blind spots** (Windows-only evidence today) | Med | High | WS-18.T3 parity matrix from M1; 3-OS CI runners early; GP evidence required per-OS |
| R11 | **Trust-stack UX overhead** annoys users vs. frictionless rivals | Med | High | Graduated approvals (WS14.T4) make safety *reduce* prompts (the Anthropic 84% lesson); low-risk envelopes auto-approve; measure prompts-per-task in §11 |
| R12 | **Plugin API as forever-contract** | Low | Med | WIT v0 surface tiny (Zed launch scope); version everything; dogfood via bundled extensions first |

---

## 11. Quality Bars & KPIs

**Performance (CI-enforced from M1):** input-to-paint p50 ≤ 8ms / p95 ≤ 16ms on reference hardware; cold start ≤ 1.5s to interactive editor; file open (≤1MB) ≤ 100ms; 100MB file open ≤ 2s (streaming mode); search (Legion repo) ≤ 150ms warm; LSP completion p95 ≤ 150ms post-warm; memory ≤ 500MB for GP-1 workload steady-state.

**Reliability:** crash-free session rate ≥ 99.5% (measured via opt-in reports); zero data-loss bugs (any reproducible loss is a release blocker); watcher/PTY orphan leaks: zero in chaos suite.

**Agent quality (from M3):** Legion-Bench pass rate tracked per release (baseline at M3, regression-blocked); injection-suite pass rate 100% (blocking); mean human interventions per delegated task trending down; **approval prompts per completed task** trending down while audit coverage stays 100% (the R11 metric); cost per completed Legion-Bench task tracked.

**Trust (always):** 100% of mutations through proposal routes (audited); manifest-equals-egress equality test green; Manual-mode zero-egress test green; consent-before-retention invariant tests green.

**Adoption signals (from M3 beta):** weekly active dogfooders; GP completion rates in onboarding; assist acceptance rate; % sessions using Delegate.

---

## 12. Immediate Next Actions (the first two weeks)

1. Ratify this plan (or amend) and open ADR-0032..ADR-0040 as draft ADRs in `plans/adrs/`.
2. Land the M0 CI gates: no-`TextEdit` lint, perf-harness skeleton, prompt-stability test scaffold.
3. Start WS-17.T1 (cargo-dist pipeline) — it is independent and de-risks everything later.
4. Run the ADR-0037 spike (LanceDB vs sqlite-vec, 1 week, fixture corpus).
5. Begin WS-01.T1/T2 and WS-02.T1 in parallel — they are the longest poles to M1.
6. Stand up 3-OS CI runners (WS-18.T3 prerequisite) before M1 work needs them.

---


## 13. Production Finish Gates (P0-P7 status)

The P-gates are the substrate/readiness view of the plan: §8 answers "when do we ship," while this section answers "what must be accepted before a milestone claim is credible." P5-P7 remain deferred or blocked until their policy, ADR, dependency-policy, tests, and evidence prerequisites are represented in-repo.

| Phase | Gate | Status | Acceptance focus | M/WS mapping |
| --- | --- | --- | --- | --- |
| **P0** | Strict source cleanliness | **accepted / standing** | No unresolved `todo!()` or `unimplemented!()` in `crates/**/*.rs`; remaining markers are intentional fixtures/scanner constants with tests. | Standing gate for every WS and milestone |
| **P1** | Release/installability hardening (`PR-REL-001`) | **in-progress** | Installers are verifiable, unsigned/dry-run status is explicit, secrets are not committed, and readiness flips only with signed-installer or unsigned-beta policy evidence. | WS-17, M0/M5 |
| **P2** | Renderer/accessibility workflows (`PR-UI-001`, `PR-UI-002`) | **in-progress** | Renderer-backed workflows prove input, focus, accessibility, restore behavior, platform parity, and explicit large-file degradation. | WS-01, WS-18, M1/M5 |
| **P3** | Language/debug/test/SCM workflows (`PR-LANG-001`, `PR-LANG-002`) | **blocked** | LSP/DAP/test/SCM product flows are live; every write-producing action routes through proposals or default-deny policy; stale responses cannot mutate state. | WS-03, WS-04, WS-08, M1/M3/M5 |
| **P4** | Inspectable local-first AI and evals (`PR-AI-001`, `PR-AI-002`) | **blocked** | Context manifests show files, symbols, diagnostics, terminal excerpts, memory, privacy labels, and egress status before invocation; real adversarial evals produce pass/fail evidence. | WS-09, WS-10, WS-11, WS-14, WS-19, M2/M3 |
| **P5** | Extension runtime (WASM/WIT; PR-VSC-002 redirected) | **deferred / redirected** | WASM extensions cannot bypass declared policy; extension-originated edits become proposals; marketplace/runtime execution stays off until policy and sandbox tests pass. | WS-15, M5/M6 cut line |
| **P6** | Remote development UX (`PR-ENT-001`) | **deferred** | Production remote transport activates only with policy, threat model, default-deny tests, and failure-mode evidence; remote writes remain proposal-mediated. | WS-16, M6 |
| **P7** | Collaboration/admin controls (`PR-ENT-002`) | **deferred** | Collaboration cannot bypass proposal mediation; admin export is metadata-safe by default and policy-governed for raw-source inclusion. | WS-16, WS-20, M6 |

| P-gate | Required before milestone/readiness claim | Hardest open task IDs |
| --- | --- | --- |
| P0 | Every milestone | Standing source-cleanliness gate (§7, §9) |
| P1 | M5 / `PR-REL-001` product-validated | WS17.T1-WS17.T6 |
| P2 | M1 editor dogfood and M5 platform parity | WS01.T7, WS18.T1-WS18.T3 |
| P3 | M1/M3 language-debug credibility | WS03.T1-WS03.T7, WS04.T1-WS04.T4, WS08.T1-WS08.T3 |
| P4 | M2 Assist and M3 Delegate trust claims | WS09.T4, WS10.T5, WS14.T5, WS19.T2 |
| P5 | M5 extension launch set or M6 expansion | WS15.T1-WS15.T3 |
| P6 | M6 remote/cloud lane | WS16.T2-WS16.T3 |
| P7 | M6 collaboration/admin policy | WS16.T1, WS20.T3 |

---

## Appendix A — Capability → Workstream Cross-Reference

| 2026 SOTA capability | Workstream(s) |
| --- | --- |
| Tree-sitter highlighting | WS-02 |
| LSP / DAP / terminal / search / git table stakes | WS-03 / WS-04 / WS-05 / WS-06 / WS-08 |
| Next-edit prediction | WS-11 (integrate), WS-19 (own model later) |
| Repo map / AST chunks / embeddings / agentic search | WS-10 |
| AGENTS.md / rules | WS-11.T4 |
| Plan mode / spec-driven artifacts | WS-12.T3 |
| Checkpoints/rollback | WS-07.T3 |
| OS sandboxing | WS-12.T2 |
| Classifier-mediated approvals | WS-14.T4 |
| MCP client / Legion-as-MCP-server | WS-09.T6 / post-GA |
| Subagents | WS-12.T7 |
| Mission-control fleet UI | WS-13.T2 |
| Parallel worktree agents | WS-13.T1 |
| ACP multi-vendor agent hosting | WS-13.T4 |
| Evidence artifacts | WS-14.T3 |
| Diff-first review (multibuffer) | WS-01.T9 + WS-14.T1 |
| Cost/usage analytics | WS-09.T5 |
| Signed installers / auto-update / crash reporting | WS-17 |
| Harness-aware evals | WS-19 |
| WASM extensions | WS-15 |
| Collaboration / remote / cloud lane | WS-16 (post-GA) |

## Appendix B — Research Source Notes

Competitive landscape (primary sources verified 2026-06-09): cursor.com/blog/cursor-3 and /changelog; devin.ai/desktop (Windsurf→Devin Desktop redirect verified) and devin.ai/pricing; zed.dev/compare/cursor, zed.dev/docs/ai/edit-prediction, zed.dev/blog/debugger; docs.github.com Copilot features, github.blog Agent HQ + NES training + mission control; code.visualstudio.com 2026-05-15 agent-harness post; jetbrains.com/junie + plugin changelog; kiro.dev/docs/specs; trae.ai SOLO blog; firebase.google.com/docs/studio; en.wikipedia.org/wiki/Google_Antigravity, antigravity.google/blog (I/O 2026), theregister.com on Gemini CLI sunset; code.claude.com docs (subagents, sandboxing, web), anthropic.com/engineering/claude-code-sandboxing; developers.openai.com/codex (cli, sandboxing, changelog); github.com/google-gemini/gemini-cli; aider.chat repomap docs; sourcegraph.com/amp + ampcode.com (oracle, manual); goose-docs.ai AAIF move; jules.google + developers.google.com/jules/api; linuxfoundation.org AAIF announcement; swebench.com, tbench.ai leaderboards; playwright.dev/mcp. Items marked "reported" in research (Copilot credit details, SWE-1.6 specifics, valuation figures) were not load-bearing for any decision in this plan.

Technology stack (primary sources verified 2026-06-09): boringcactus.com 2025 Rust GUI survey; egui issues #3086/#7485 + AccessKit integration; zed.dev blogs (Rope & SumTree, CRDTs, extensions/WIT, GPUI README); lapce/floem; loro.dev + diamond-types; tree-sitter; rust-analyzer book; async-lsp/tower-lsp crates; helix LSP architecture; alacritty_terminal + portable-pty crates; BurntSushi ripgrep-as-library discussion #2509; tantivy; LanceDB/sqlite-vec materials; aider repo-map; modelcontextprotocol.io spec (2025-11-25) + transport-futures blog + rust-sdk (rmcp); platform.claude.com docs (pricing, prompt caching, structured outputs, count_tokens, batch); OpenAI Responses migration guide; Codex sandboxing internals (Landlock/seccomp/Seatbelt); gitoxide README (write-path gaps); jj-vcs/jj; rcodesign distribution guide; cargo-dist; velopack; Embark crash-handler/minidumper + sentry-rust-minidump; Mozilla rust-minidump.

Internal evidence: workspace code survey 2026-06-09 (per-crate maturity, fixture/flag inventory in §3); `plans/phase-status-ledger.md`; `plans/product-readiness-ledger.md`; `plans/evidence/gui-productization/phase-7-known-limitations.md`; `docs/ARCHITECTURE_AUTHORITY_BOUNDARIES.md`; `docs/MODES.md`; `docs/LEGION_PIVOT.md`; `mockups/design.md`; ADR-0001..ADR-0031.

## Appendix C — Complete Task → Milestone Matrix

Every task ID in §7 appears here exactly once. A task added to §7 without a row here fails plan review; this matrix is the completeness check for §8.

| Workstream | Task → milestone |
| --- | --- |
| WS-01 | T1–T5 → M1 · T6 → M2 · T9 → M3 · T7, T8 → M5 |
| WS-02 | T1–T3 → M1 · T5 → M2 · T4 → M5 |
| WS-03 | T1–T5 → M1 · T7 → M3 · T6, T8 → M5 |
| WS-04 | T1–T2 → M3 · T4 → M5 · T3, T5 → M6 |
| WS-05 | T1–T3 → M1 · T4, T5 → M3 · T6 → M5 |
| WS-06 | T1–T4 → M1 · T5 → M6 |
| WS-07 | T1–T3 → M2 · T4 → M3 |
| WS-08 | T1–T2 → M1 · T3 → M3 · T5 → M4 · T4, T6 → M6 |
| WS-09 | T1–T4 → M2 · T6 → M3 · T5 → M4 · T7 → M6 |
| WS-10 | T1–T5 → M2 · T6 → M4 |
| WS-11 | T1–T4 → M2 · T5 → M6 |
| WS-12 | T1–T6 → M3 · T7 → M4 |
| WS-13 | T1–T5 → M4 |
| WS-14 | T2 → M2 · T1, T3, T4, T5 → M3 · T6 → M6 |
| WS-15 | T1–T3 → M5 · T4 → M6 |
| WS-16 | T1–T3 → M6 |
| WS-17 | T1 → M0 · T2 → starts M1, completes M5 · T3–T6 → M5 |
| WS-18 | T1 → M0 (skeleton), M1 (enforced) · T3 → starts M1, completes M5 · T2, T4 → M5 |
| WS-19 | T1–T2 → M3 · T3, T4 → M6 |
| WS-20 | T1–T2 → M5 · T3, T4 → M6 |

