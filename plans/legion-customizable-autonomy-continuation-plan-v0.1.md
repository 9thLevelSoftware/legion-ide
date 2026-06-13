# Legion IDE — Customizable IDE & Autonomy Continuum Continuation Plan v0.1

- Status: Proposed (for ratification)
- Date: 2026-06-13
- Composes on: `legion-production-master-plan-v0.1.md` (the master plan). This document does not replace it. It (a) reconciles the master plan's workstream status against the current `main` tree, and (b) re-centers the path forward on three product pillars the master plan under-weighted: a great old-school manual IDE, deep customizability, and a configurable-autonomy continuum that scales from zero AI to full automation. Where this plan and the master plan disagree about *future priority and sequencing*, this plan (once ratified) wins; the master plan remains the canonical source for the WS-01..WS-20 task definitions this plan reuses.
- Inputs: full-workspace code survey at `main` HEAD `4d6aad7` (2026-06-13); `docs/MODES.md`; `docs/ARCHITECTURE_AUTHORITY_BOUNDARIES.md`; `plans/dependency-policy.md`; the master plan; the accepted M0/M1 evidence under `plans/evidence/production/`; ADR-0001..ADR-0040.

---

## 1. Executive Summary

The master plan's thesis — "activate the simulated verbs in dependency order, then productize the trust/fleet layer" — is being executed well. M0 is accepted (ADR-0032..0040 ratified, perf-harness and release-pipeline xtask gates, the no-`egui::TextEdit` gate, ~1047 tests green) and M1 is substantially underway. Two facts reshape the path forward:

1. **The hardest builds are done; what remains for a credible manual IDE is mostly *activation*.** A real `legion-lsp` crate exists with JSON-RPC framing and process supervision — but the app never spawns rust-analyzer. `legion-platform` has real cross-platform PTY — but the app still routes the terminal through a deterministic fixture. tree-sitter is genuinely wired; the native Anthropic streaming client is real and *ahead of plan*; proposal apply is real and already enabled by default for trusted workspaces. So the manual IDE is closer than the master plan's milestone math implies — the gating work is flipping substrates on, not inventing them.

2. **The owner's north star has sharpened** to three pillars the master plan treated as scattered tasks rather than first-class tracks:
   - **A — The Manual IDE.** An old-school, fast, keyboard-driven, *complete* IDE that stands on its own with AI fully off. Today the editor verbs are partly activated; power-user depth (splits, snippets, macros, advanced navigation, modal editing) is largely absent.
   - **B — Deep Customizability.** A real customization layer: settings-as-code, a remappable keymap engine (including modal/Vim and Emacs schemes), a user theme system, layout/perspective customization, and profiles. This is the thinnest pillar in the current tree — there is no theme system and no keybinding engine beyond VS Code-manifest parsing.
   - **C — The Autonomy Continuum.** Today autonomy is four hard-coded discrete modes (Manual / Assist / Delegate / Automate), and `MODES.md` forbids autonomous apply. The owner wants a *configurable continuum* the user dials in — per-capability, per-risk policies — scaling from pure manual to **full automation**. That is a genuine architectural step, not a UI toggle, and it must be reconciled with the proposal-mediated-mutation invariant (it can be: full automation = pre-authorized, signed, scoped auto-approval that still routes every change through proposal → validation → audit → reversible apply, with a standing kill switch).

This plan keeps every architectural invariant (proposal-mediated mutation, projection-only UI, metadata-first observability, fail-closed saves, phase-gated activation, dependency policy) and adds:

- **5 new ADRs** (ADR-0041..ADR-0045): configuration model, keymap engine, theme system, autonomy policy engine, and the full-automation envelope (the one that amends `MODES.md`).
- **9 new workstreams** (WS-21..WS-29) plus elevation/continuation of master-plan WS-03/04/05/06/08/09/10/11/12/13/14, with granular tasks, crate touchpoints, and acceptance evidence.
- **5 continuation milestones** (C1..C5) that nest into the master plan's M-scale: a credible manual IDE → a powerful & customizable manual IDE that ships standalone → assist + autonomy foundations → delegate + higher autonomy → a gated full-automation beta.

The organizing principle: **earn each level of autonomy on top of a manual IDE that is already excellent and fully customizable, so that every increment of automation is something the user explicitly dials up — never something bolted on or assumed.**

---

## 2. Current State vs. Master Plan (reconciliation)

Status legend: REAL (working runtime) · ACTIVATION (substrate built, not switched on) · PARTIAL · FIXTURE · ABSENT.

| WS | Capability | Status | Evidence / note |
| --- | --- | --- | --- |
| WS-01 | Editor surface / custom canvas | PARTIAL | `legion-desktop/src/view/code_canvas_painter.rs`; no-`TextEdit` gate enforced; large-file streaming + anchor layer deferred |
| WS-02 | tree-sitter highlighting | REAL | `legion-index/src/lib.rs` `TreeSitterParser`; incremental parse + highlight queries wired |
| WS-03 | LSP | ACTIVATION | `legion-lsp` crate real (JSON-RPC, supervision) but **rust-analyzer never spawned from app** |
| WS-04 | DAP debugging | FIXTURE | `legion-terminal` `DapAdapterFixtureRuntime` only |
| WS-05 | Terminal | ACTIVATION | real PTY in `legion-platform`; grid renderer landed; app **still uses the terminal fixture** |
| WS-06 | Search + command palette | PARTIAL | palette REAL; project search is **lexical (no ripgrep)** |
| WS-07 | Proposal apply | REAL | enabled by default for **trusted** workspaces; conflict UX + checkpoints pending |
| WS-08 | Git/VCS | PARTIAL | CLI status/blame/diff data + gutter substrate; inline blame, branch/worktree UI thin |
| WS-09 | AI provider plane | PARTIAL (ahead) | **native Anthropic Messages streaming client REAL**; cost meter + cache instrumentation pending |
| WS-10 | Context engine | FIXTURE | 64-dim deterministic stub; no repo map, no agentic-search tools |
| WS-11 | Assist surfaces | PARTIAL | assistant rail projection; ghost-text / inline-edit loop not active |
| WS-12 | Agent harness / Delegate | PARTIAL | state machine + worktree orchestrator real; **tool execution loop not active**; no OS sandbox |
| WS-13 | Workflows / fleet console | FIXTURE | DAG coordinator real; no execution, no Kanban UI |
| WS-14 | Trust / review surfaces | PARTIAL | audit gates real; diff-review UI + graduated approvals absent |
| WS-15..20 | plugins/remote/dist/perf/evals/security | PARTIAL/FIXTURE | per master plan; release-pipeline + perf-harness skeletons live |
| — | **Settings / customization** | PARTIAL | `legion-ui` `SettingsProjection` (font/zoom/flags) + panel; **no theme system, no keymap engine** |
| — | **Autonomy model** | DISCRETE | `ProductMode` variants `{Manual, Assist, Delegates, Automate}` (the runtime enum variant is `Delegates`; `docs/MODES.md` prose calls the mode "Delegate"); `DockMode` panel filtering; **no per-capability policy; autonomous apply forbidden** |

**The activation insight:** WS-03 and WS-05 are each one focused task away from real (spawn the server / swap the fixture for the PTY path), and WS-06 needs the ripgrep crates dropped into an existing search seam. These three plus WS-04 (real CodeLLDB) and WS-08 depth convert "projection of an IDE" into "an IDE." They are the spine of Pillar A and the prerequisite for everything else.

---

## 3. The Re-Centered Product Thesis

Legion is **a deterministic, deeply customizable, keyboard-first IDE with a configurable autonomy dial.** The dial's zero position is a complete classic IDE that needs no network and no model. Each notch up the dial is an explicit, revocable grant of authority, governed by the same proposal/evidence/audit machinery already in the substrate. The top of the dial is full automation — hands-off execution *within a pre-authorized, signed, scoped envelope*, never unmediated mutation.

Three pillars, in dependency order:

- **Pillar A — The Manual IDE (foundation).** Must be excellent and complete before any autonomy is interesting. Old-school power-user expectations: splits, multi-cursor, snippets, macros, marks/registers, advanced navigation, real LSP/terminal/debug/search/git.
- **Pillar B — Deep Customizability (the product's identity).** Settings-as-code, keymaps (incl. modal), themes, layouts, profiles. This is what makes it *yours* and is the most under-built area today.
- **Pillar C — The Autonomy Continuum (the differentiator).** A per-capability policy engine generalizing the four modes into a dial, graduated approvals, and a gated full-automation envelope.

Pillars A and B together define a product that can ship and be loved *with AI off* — which is the honest meaning of "old school manual IDE." Pillar C is layered on top so that autonomy is always opt-in, legible, and bounded.

---

## 4. Pillar A — The Manual IDE (old-school excellence)

Goal: with autonomy at zero, Legion is a fast, complete, keyboard-driven IDE a power user would choose over Vim/Sublime/JetBrains for daily work. Two thrusts: **activate the verbs** (continue master-plan WS) and **add power-user depth** (new WS-25).

### 4.1 Verb activation (continue master-plan workstreams)

- **WS-03 (LSP) — flip it on.** `legion-lsp` already supervises a stdio JSON-RPC server. Remaining: app-layer spawn of rust-analyzer with policy-gated binary resolution (system PATH → checksummed download, air-gap denies download); wire `didOpen/didChange` from editor transactions; render `publishDiagnostics` to the gutter + problems panel; activate completion/hover/definition/references/symbols/inlay-hints; route rename/code-action/format through the existing proposal routes. *Acceptance:* introduce an error in the Legion repo → diagnostic < 1s; rename across files = one reversible proposal.
- **WS-05 (Terminal) — replace the fixture.** Swap `DeterministicTerminalFixture` for the real `legion-platform` PTY behind the existing policy gate; consume output into an `alacritty_terminal` grid; the grid renderer already exists for the panel. Add shell integration (OSC 133/7) so command boundaries and cwd are structured. *Acceptance:* interactive shell on all 3 OSes; `cargo test` runs in-panel; kill-escalation works against real processes.
- **WS-06.T1 (Search) — real ripgrep.** Drop `grep-searcher`/`grep-regex`/`ignore`/`globset` into the existing search seam in `legion-project`; literal/regex/word/case modes, glob include/exclude, streaming results; multi-file replace as a reviewable proposal. *Acceptance:* Legion-repo search < 150ms warm; replace = one reversible proposal.
- **WS-04 (Debug) — real DAP.** Hand-rolled DAP client (the master plan's WS-04.T1) + CodeLLDB for Rust, launched from the existing zero-config Cargo locator/runnables. *Acceptance:* breakpoint in a Legion test, hit it, inspect locals.
- **WS-08 (Git) — depth.** Inline current-line blame, branch switcher in the status bar, stage/commit hunk UX, and a worktree panel (also serves Delegate later). *Acceptance:* GP-1 commit path end-to-end; agent worktrees visible.

### 4.2 WS-25 — Manual Power-User Toolkit (new)

The "old school" depth that makes a manual IDE beloved. **Crates:** `legion-editor`, `legion-ui`, `legion-desktop`, `legion-protocol` (DTOs). All projection-only in the UI; editor authority stays in `EditorEngine`.

- WS-25.T1 **Split panes & editor groups.** Horizontal/vertical splits, editor groups with independent active buffers, drag/keyboard move between groups, grid layout persisted. *Accept:* 2×2 split editing four buffers; layout restored on reopen.
- WS-25.T2 **Tab & buffer management.** Pinned tabs, preview tabs, recently-used cycling, "close others/right", split-from-tab; per-group tab strips. *Accept:* tab operations e2e; keyboard-only.
- WS-25.T3 **Multi-cursor & column selection depth.** Add-cursor-above/below, add-next-occurrence, select-all-occurrences, column (box) selection, cursor-per-line on selection — over the existing multi-cursor data model. *Accept:* multi-cursor conformance suite.
- WS-25.T4 **Snippets engine.** Tab-stop/placeholder/choice/variable snippets (LSP-snippet-compatible syntax), per-language snippet sets, user-defined snippets (config-as-code from WS-21), snippet picker. *Accept:* insert a multi-stop snippet; tab through stops; user snippet from settings works.
- WS-25.T5 **Macro record/replay & command repeat.** Record a sequence of commands/edits, replay N times, named macros saved to config; "repeat last action". *Accept:* record a refactor edit, replay across 10 sites deterministically.
- WS-25.T6 **Advanced navigation.** Go-to-line/column, go-to-symbol (file + workspace, from WS-02/03), breadcrumb scope bar, jumplist (back/forward), bookmarks/marks, "go to matching bracket", expand/shrink selection by syntax node (tree-sitter). *Accept:* navigation conformance suite; jumplist survives edits via anchors (WS-01.T6).
- WS-25.T7 **Folding, comparison & focus.** Syntax-aware folding UX, fold-by-level, file/diff compare view (reuses WS-14 diff surface), distraction-free/zen layout, whitespace/indent guides. *Accept:* fold/compare/zen e2e; settings-gated.
- WS-25.T8 **Registers/clipboard ring & paste transforms.** Clipboard history ring, named registers (also the Vim register backend in WS-22), paste-and-indent, paste-from-history picker. *Accept:* clipboard-ring picker; register round-trip.

---

## 5. Pillar B — Deep Customizability

Goal: everything visible and behavioral is configurable, as code and through UI, without recompiling. This is the pillar most absent today. Architectural constraint: configuration is *owned by the app* and *projected to the UI*; `legion-ui` never reads files or owns config state — it renders a `SettingsProjection` and emits change intents (preserving the projection-only invariant and the `legion-ui` dependency ban).

### 5.1 WS-21 — Configuration & Settings-as-Code (new) · ADR-0041

**Crates:** `legion-app` (owns config), `legion-storage` (persistence), `legion-protocol` (schema DTOs), `legion-ui`/`legion-desktop` (projection + editor).

- WS-21.T1 **Layered config model.** Resolution order default → user → workspace → profile → session, with per-key provenance ("this value comes from workspace"). Deep-merge semantics; arrays replace, maps merge. *Accept:* provenance shown per key; override precedence tests.
- WS-21.T2 **`settings.json` + JSON-schema.** Canonical on-disk format (`~/.config/legion/settings.json` user; `.legion/settings.json` workspace), a published JSON-schema for validation + autocomplete-in-Legion, graceful handling of unknown/invalid keys (warn, don't crash). *Accept:* schema validates; invalid file degrades to defaults with a toast.
- WS-21.T3 **Two-way settings UI binding.** The existing settings panel reads the projection and writes through change intents that the app persists; editing `settings.json` live-reloads the UI and vice-versa. *Accept:* change a value in JSON → UI updates < 500ms; change in UI → JSON rewritten minimally (preserves comments/order where feasible).
- WS-21.T4 **Settings search & registry.** Every setting registered with id/type/default/scope/description/category; searchable settings UI (design.md command-palette feel). *Accept:* settings coverage report; search finds any registered key.
- WS-21.T5 **Live reload & hot-apply.** Categorize settings by apply-cost (instant / reopen-buffer / restart) and hot-apply the instant ones (fonts, theme, keymap, layout). *Accept:* font/theme/keymap changes apply without restart.
- WS-21.T6 **Migration & defaults.** Versioned settings schema with migrations; `derive settings defaults` (already landed) becomes the default source of truth; reset-to-default per key/section. *Accept:* old settings file migrates; reset works.

### 5.2 WS-22 — Keymap & Input Customization (new) · ADR-0042

**Crates:** `legion-protocol` (keymap DTOs — extend the existing `KeyBinding` types), `legion-app` (keymap resolution + command dispatch), `legion-ui`/`legion-desktop` (capture + editor). The current `KeyBinding` parsing exists only for VS Code-manifest ingestion; this builds a first-class engine.

- WS-22.T1 **Keymap engine.** Bindings map `(key-chord, when-context) → command-id`; full command registry (every `CommandDispatchIntent`); multi-key chords; context predicates (focus, mode, selection, language, autonomy level). *Accept:* chord + context resolution suite; conflicts detected and reported.
- WS-22.T2 **Keymap schemes.** Bundled schemes: Legion default, VS Code, Sublime, JetBrains; user override layer on top. Per-OS chord normalization (⌘/Ctrl). *Accept:* switch scheme at runtime; per-OS smoke.
- WS-22.T3 **Modal editing layer (Vim).** Normal/insert/visual/visual-line/visual-block/operator-pending states, motions, operators, counts, registers (WS-25.T8), marks, `.`-repeat, ex-command subset; implemented as a first-class input mode, not an emulation hack, gated by a setting. *Accept:* a curated Vim conformance subset; round-trips with multi-cursor and snippets.
- WS-22.T4 **Emacs scheme.** Prefix-key (C-x/C-c) chords, mark/region, kill-ring (clipboard ring), incremental search bindings. *Accept:* Emacs binding subset suite.
- WS-22.T5 **Keybinding editor UI.** Searchable command list, record-a-chord capture, conflict highlighting, per-scheme diff, export/import as `keybindings.json`. *Accept:* rebind a command via UI; persists to config; conflict surfaced.
- WS-22.T6 **Which-key / discoverability.** Optional chord-continuation hint overlay; command palette shows bindings (design.md ⌘K). *Accept:* setting-gated overlay; palette shows current binding per command.

### 5.3 WS-23 — Theming & Visual Customization (new) · ADR-0043

**Crates:** `legion-desktop` (theme application — the only crate allowed renderer deps), `legion-protocol` (theme token DTOs), `legion-app` (theme resolution/persistence). Today theming is hardcoded dark/light token maps; this makes them data.

- WS-23.T1 **Theme format.** A theme = UI token set (the `mockups/design.md` token vocabulary) + syntax token set (tree-sitter capture → color/style) + terminal palette. Themes are data files validated by schema; bundled set (≥1 dark, ≥1 light, high-contrast). *Accept:* swap theme at runtime; high-contrast meets WCAG AA (PR-UI-001 tie-in).
- WS-23.T2 **Custom theme authoring + live reload.** User themes in config dir; edit → live reload; "duplicate built-in to customize". *Accept:* author a theme, see it apply on save.
- WS-23.T3 **VS Code theme import.** Convert VS Code theme JSON (tokenColors + workbench colors) → Legion tokens with a documented mapping + fallback for unmapped scopes. *Accept:* a popular VS Code theme imports and renders recognizably.
- WS-23.T4 **Typography & rendering config.** Font family/size/weight/line-height/letter-spacing, ligatures toggle, per-UI vs per-editor fonts, font fallback chains (CJK coverage, WS-01.T5 tie-in). *Accept:* font settings hot-apply; ligature toggle works.
- WS-23.T5 **Icon themes & per-language overrides.** File-icon theme, syntax color overrides per language/scope, semantic-token vs tree-sitter precedence control. *Accept:* icon theme swap; per-language override respected.

### 5.4 WS-24 — Layout, Panes & Workspace Customization (new)

**Crates:** `legion-ui` (dock/layout projection — already has `DockLayout`/`DockMode`), `legion-app` (layout state ownership/persistence), `legion-desktop` (render).

- WS-24.T1 **Dockable, movable panels.** Move any panel to any side/area, tabbed panel groups, detach (multi-window tie-in WS-18.T4), show/hide, resize with persisted splitters — over the existing `DockLayout`. *Accept:* relocate a panel; layout persists.
- WS-24.T2 **Perspectives (saved layouts).** Named layouts bound to autonomy level/mode (Manual emphasizes editor+tree; higher levels surface directive/fleet per design.md §7); switch perspective from palette. *Accept:* per-mode perspective restores on mode switch.
- WS-24.T3 **Activity/status bar customization.** Configurable activity-bar items, status-bar segments (git branch, LSP status, autonomy badge, cursor/encoding), reorder/hide. *Accept:* status-bar config respected.
- WS-24.T4 **Workspace-scoped UI state.** Per-workspace open editors, layout, expanded tree nodes, active perspective (extends the existing session-restore). *Accept:* reopen workspace restores full UI state.

### 5.5 WS-29 — Profiles & Settings Sync (new)

**Crates:** `legion-app`, `legion-storage`, `legion-retention` (encrypted sync, post-GA).

- WS-29.T1 **Profiles.** A named profile bundles settings + keymap + theme + layout + enabled extensions + **default autonomy profile** (Pillar C). Per-workspace profile binding; quick-switch. *Accept:* switch profile changes all five layers atomically.
- WS-29.T2 **Export/import & reset.** Export a profile to a shareable file; import with conflict review; reset-to-factory. *Accept:* round-trip a profile between two installs.
- WS-29.T3 **Settings sync (post-GA, opt-in, encrypted).** Optional sync via `legion-retention` keyring/encryption; metadata-first; explicit consent; no raw secrets synced. *Accept:* consented sync round-trip; air-gap denies.

---

## 6. Pillar C — The Autonomy Continuum

Goal: replace the four hard-coded modes with a **configurable policy lattice** the user dials, from zero to full automation, where each level is an explicit, revocable, bounded grant of authority — and every mutation, at every level, still flows through proposal → validation → audit → reversible apply.

### 6.1 The model (ADR-0044)

An **Autonomy Profile** maps **capability classes × risk tiers → disposition**:

- **Capability classes:** read/inspect · edit-in-open-buffer · multi-file edit · create file · delete file · dependency/manifest change · run terminal command · network/provider call · git commit · git push · merge/PR.
- **Risk tiers:** derived deterministically (scope size, deletion ratio, manifest/migration/secret proximity, path sensitivity, network egress) per WS-14.T4.
- **Dispositions:** `forbid` · `propose` (human approves) · `auto` (pre-authorized auto-approval within envelope) — auto always still creates a proposal and audit record.

The four named modes become **presets** over this lattice, preserving `ProductMode` as a coarse selector:
- **Manual (0):** every class `forbid` except local read/edit; no network, no provider, no worker. (Equals today's Manual.)
- **Assist:** read/inspect/explain `auto`; edits `propose`; network/provider gated by consent. (Equals today's Assist.)
- **Delegate:** within a worktree, edits/terminal `auto`; promotion to main `propose`; network per policy.
- **Automate:** task-graph execution; risk-tiered `auto`/`propose`; merge `propose`.
- **Full Automation:** `auto` up to a configured risk ceiling within a signed envelope; above the ceiling `propose`; standing kill switch. (New — ADR-0045.)

Users dial autonomy by picking a preset *or* customizing any cell of the lattice (e.g., "Delegate, but deletes always `propose` and pushes always `forbid`"). Scope is bindable: global default, per-workspace, per-session, per-task.

### 6.2 WS-26 — Autonomy Policy Engine (new) · ADR-0044

**Crates:** `legion-security` (the broker already gates capabilities — extend it to evaluate the lattice), `legion-protocol` (Autonomy Profile DTOs generalizing `ProductMode`), `legion-app` (resolution + persistence), `legion-ui` (projection).

- WS-26.T1 **Autonomy Profile DTOs.** Lattice schema; presets; per-cell override; scope binding; serialization (config-as-code via WS-21). `ProductMode` becomes a derived coarse label. *Accept:* preset → lattice expansion tests; round-trip serialization.
- WS-26.T2 **Broker evaluation.** Every capability decision consults the active Autonomy Profile + risk tier → disposition; deny-by-default preserved; decision recorded with the governing cell + risk citation. *Accept:* lattice decision suite; Manual preset proves zero network/provider/worker surfaces (MODES.md completion requirement).
- WS-26.T3 **Scope resolution & precedence.** Resolve the effective profile from global/workspace/session/task layers with provenance. *Accept:* precedence tests; task-scoped tighten-only rule (a task can restrict but not exceed its session's grant).
- WS-26.T4 **Autonomy projection.** A `legion-ui` projection exposing the active profile, per-cell disposition, scope, and the governing reason — for the control surfaces (WS-28). *Accept:* projection matches broker decisions byte-for-byte in tests.

### 6.3 WS-14.T4 (elevated) — Graduated, classifier-mediated approvals

From the master plan, elevated to a Pillar-C keystone.

- Deterministic risk classifier first: path scope, file count, deletion ratio, dependency/manifest touch, migration/SQL detection, secret-pattern proximity, network egress. Optional model-assisted *recommender* (never an approver). Auto-approve only within configured envelopes; every auto-approval audited with the rule that authorized it. *Accept:* risk-rule unit suite; envelope respected; "fewer prompts, same audit coverage" measured (the §11 metric).

### 6.4 WS-27 — Full-Automation Envelope (new) · ADR-0045

The gated terminal autonomy state. **This is the one place this plan amends `MODES.md`** (Automate currently forbids autonomous apply). The amendment is narrow and defensible: full automation is **auto-approval within a pre-authorized, signed, scoped envelope**, not unmediated mutation.

Mandatory envelope components (all required; absence = fail-closed to `propose`):
- WS-27.T1 **Signed policy envelope.** An explicitly user- or enterprise-signed Autonomy Profile with a risk ceiling, allowed capability classes, path scope, network allowlist, and budget. Unsigned/expired ⇒ no auto. *Accept:* tampered/expired envelope denies auto; signature verified via `legion-retention`/`legion-security`.
- WS-27.T2 **Standing kill switch + budget/risk ceilings.** Always-available halt that reaps sandboxed processes < 2s (reuses WS-13.T3); hard budget and risk-tier ceilings that drop to `propose` when exceeded. *Accept:* kill halts a running automation < 2s; ceiling breach pauses.
- WS-27.T3 **Checkpoints & one-click rollback.** Auto-checkpoint before every auto-applied change (WS-07.T3); the whole automation run is reversible as a unit. *Accept:* full run rolled back to pre-run state; manual edits made during the run preserved where non-conflicting.
- WS-27.T4 **Sandbox requirement.** Auto terminal/edit execution requires the OS sandbox (ADR-0038 / WS-12.T2); no sandbox ⇒ no auto. *Accept:* sandbox-absent denies auto; escape-attempt suite fails closed.
- WS-27.T5 **Evidence & replay.** Every auto decision is metadata-audited and the run replayable from the existing replay manifests; egress matches the context manifest. *Accept:* run replays from metadata; manifest-equals-egress test green.
- WS-27.T6 **`MODES.md` + ADR amendment.** Amend `docs/MODES.md` Automate section and extend ADR-0016/ADR-0031 so "autonomous apply" is permitted *only* under a conforming envelope; record the reconciliation with the proposal-mediated invariant explicitly. *Accept:* docs-hygiene green; authority-boundaries doc updated; reviewers sign off.

### 6.5 WS-28 — Autonomy Control Surfaces (new)

**Crates:** `legion-ui` (projection-only), `legion-desktop` (render). Makes the dial legible and controllable.

- WS-28.T1 **The autonomy dial.** A continuum selector (presets + "custom") plus an expandable per-capability × per-risk matrix editor; live preview of "what this allows"; scope picker (global/workspace/session/task). *Accept:* dialing changes broker behavior immediately; matrix edits persist via config.
- WS-28.T2 **Always-visible autonomy/trust strip.** Current level, scope, active grants, egress status, and a prominent kill switch (design.md §2.3, §8). Manual hides all AI chrome. *Accept:* strip reflects the live profile; Manual zero-egress test green.
- WS-28.T3 **Approval & risk console.** The graduated-approval queue (action, owner, risk badge + governing rule, files, approve/review/reject), risk monitor, and per-run budget/cost (WS-09.T5) — reuses WS-13/WS-14 surfaces. *Accept:* a medium-risk action pauses for approval with its rule cited.
- WS-28.T4 **Activity & intervention feed.** The Agent Comm Stream (design.md §10.9) with PLAN/WRITE/TEST/REVIEW/ERROR/APPROVAL/COMPLETE tags; intervene/pause/scope-down controls mid-run. *Accept:* pause mid-run, tighten scope, resume.

### 6.6 Continuation of WS-12 / WS-13 (autonomy requires real execution)

"Up to full automation" is meaningless unless Delegate and Automate actually execute. Continue, per the master plan:
- **WS-12** tool execution loop (schema-validated read/grep/glob/edit-as-proposal/terminal/MCP), OS sandbox (ADR-0038), plan mode as editable spec artifacts. 
- **WS-13** workflow runtime activation (wire `LegionWorkflowCoordinator` to real workers), fleet console Kanban, ACP host (run external agents inside Legion's envelope).

---

## 7. New ADR Queue (ADR-0041 .. ADR-0045)

Following the established sequence after ADR-0040; ratified through the normal ADR process with evidence under `plans/evidence/production/`.

| ADR | Decision | Recommendation | Blocks |
| --- | --- | --- | --- |
| ADR-0041 | Configuration model | Layered config (default→user→workspace→profile→session); `settings.json` + JSON-schema; app owns config, UI projects it; live reload by apply-cost class | WS-21, WS-29 |
| ADR-0042 | Keymap engine | First-class `(chord, when-context)→command` engine; bundled schemes incl. first-class modal **Vim** and **Emacs** input modes (not emulation hacks); per-OS normalization; `keybindings.json` | WS-22, WS-25 |
| ADR-0043 | Theme system | Themes-as-data (UI + tree-sitter syntax + terminal tokens); user authoring + live reload; VS Code theme import with documented mapping; `legion-desktop` is the only crate applying renderer colors | WS-23 |
| ADR-0044 | Autonomy policy engine | Autonomy Profile = capability-class × risk-tier → disposition lattice; the four modes become presets; `ProductMode` retained as derived coarse label; evaluated in the capability broker | WS-26, WS-28, WS-14.T4 |
| ADR-0045 | Full-automation envelope | Auto-approval permitted **only** within a signed, scoped, risk-ceilinged, sandboxed, budgeted, replayable envelope with a standing kill switch; amends `MODES.md` Automate + ADR-0016/0031; reconciled with proposal-mediated mutation (auto-approval ≠ unmediated write) | WS-27 |

Also ratify as policy (no new ADR): **autonomy decisions are always audited with the governing lattice cell + risk rule**, and **task-scoped autonomy can only tighten, never exceed, its session grant** (the monotonic-restriction rule).

---

## 8. Continuation Milestones (C1 .. C5)

These nest into the master plan's M-scale and continue from the current M1-in-progress state. Each closes with an evidence package under `plans/evidence/production/<milestone>/` and a readiness-ledger update, per established process.

| Milestone | Theme | Primary contents | Hard exit criteria |
| --- | --- | --- | --- |
| **C1** — Manual IDE Credible (≈ master M1 exit) | Activate the verbs | WS-03 LSP spawn, WS-05 real terminal, WS-06.T1 ripgrep, WS-08 git depth, WS-04.T1–T2 debug, WS-01 editor-feel polish; 3-OS CI | GP-1 on all 3 OSes with **real** LSP/terminal/search; **dogfooding gate** (Legion built in Legion ≥1 week); §11 editor budgets green |
| **C2** — Manual IDE Powerful & Customizable | The standalone product | WS-25 power-user toolkit; WS-21 config-as-code; WS-22 keymaps incl. Vim/Emacs; WS-23 themes; WS-24 layout/perspectives; WS-29.T1–T2 profiles | A power user can run Legion daily **with AI fully off**: splits, snippets, macros, modal keymaps, custom themes, custom layouts, profiles — all config-as-code + UI. Customization conformance suites green |
| **C3** — Assist + Autonomy Foundations (≈ master M2) | The dial appears | WS-09 finish (cost meter, cache discipline), WS-10 context engine + agentic search, WS-11 ghost-text/inline-edit live, WS-07.T2–T3 conflict UX + checkpoints, WS-26 policy engine, WS-14.T4 graduated approvals, WS-28 control surfaces | GP-2; the **autonomy dial is real** for low levels (Manual↔Assist) with per-capability policy; every mutation audited with governing rule; manifest-equals-egress green |
| **C4** — Delegate + Higher Autonomy (≈ master M3–M4) | Bounded automation | WS-12 tool loop + OS sandbox + plan mode, WS-13 workflow execution + fleet console + ACP host, WS-08.T3 worktree panel | GP-3 and GP-4 with the dial reaching Delegate/Automate; bounded auto-apply **within worktrees**; sandbox-escape suite green; kill switch < 2s |
| **C5** — Full Automation (gated beta) | The top of the dial | WS-27 full-automation envelope (ADR-0045, MODES.md amendment), budget/risk ceilings, signed envelopes, full-run rollback, replay | A directive runs **hands-off within a signed envelope** end-to-end, with standing kill switch, one-click full-run rollback, and complete evidence/replay; full-automation safety suite (envelope tamper, ceiling breach, sandbox-absent) green |

Sequencing commitment: **C1 → C2 before any of C3–C5 ships to users.** The manual IDE must be excellent and fully customizable before autonomy is dialed up. C3–C5 increase autonomy one bounded, audited notch at a time. (Distribution/perf/accessibility/security tracks — master-plan WS-17/18/20 — run continuously and gate the eventual GA, unchanged.)

---

## 9. Invariant Reconciliation (especially full automation)

- **Proposal-mediated mutation holds at every autonomy level.** `auto` disposition does not write directly; it creates a proposal that passes the same validation gates and audit-before-success, then is auto-*approved* by a pre-authorized signed policy. The difference between `propose` and `auto` is *who satisfies the approval step*, never whether the gates run. (ADR-0045 records this explicitly.)
- **Projection-only UI holds.** All customization and autonomy surfaces are `legion-ui` projections emitting intents; config, keymap, theme, layout, and autonomy state are owned by `legion-app`/`legion-security`, never by `legion-ui`. The `legion-ui` dependency ban (no `legion-app`/editor/project/storage/renderer crates) is unaffected; renderer color application stays in `legion-desktop` only (ADR-0043).
- **Fail-closed everywhere.** Missing signature, expired envelope, exceeded ceiling, absent sandbox, or unparseable config all degrade safely (auto→propose, or settings→defaults) with a visible reason.
- **Metadata-first.** Autonomy decisions, approvals, and automation runs persist IDs/hashes/rule-citations/byte-counts, not raw payloads, unless the existing consent/redaction/retention substrate is explicitly engaged.
- **Phase/dependency policy.** New surfaces ship behind ADRs + `plans/dependency-policy.md` entries (new deps: ripgrep crates, `alacritty_terminal`, CodeLLDB resolution, any theme-import parser). No renderer deps leak past `legion-desktop`.

---

## 10. Risk Register (delta from the master plan)

| # | Risk | L | I | Mitigation |
| --- | --- | --- | --- | --- |
| C-R1 | Customization surface sprawl (settings/keymaps/themes are bottomless) | High | Med | Ship a registered, schema-bounded core set (C2) and stop; everything else is config-as-code the community extends; no bespoke UI per setting |
| C-R2 | Modal (Vim/Emacs) engine half-done is worse than absent | Med | High | First-class input mode (ADR-0042), curated conformance subset as the bar, behind a setting; explicitly scope what's *not* supported |
| C-R3 | Autonomy lattice too complex for users | Med | Med | Presets are the default UX; the matrix is progressive-disclosure; "what this allows" preview; sensible per-preset defaults |
| C-R4 | Full automation erodes trust if a bad change auto-applies | Low | Critical | Signed envelope + risk ceiling + mandatory sandbox + auto-checkpoint + one-click full-run rollback + standing kill switch; ship C5 as gated beta only |
| C-R5 | `MODES.md` amendment seen as weakening the safety story | Med | High | Frame precisely: full automation = pre-authorized auto-*approval*, still proposal/audit/reversible; document the reconciliation; keep it opt-in and signed |
| C-R6 | Verb activation (LSP/terminal) reveals integration depth underestimated | Med | Med | They are activation not builds; flagship-first (rust-analyzer/CodeLLDB/bash+pwsh); reuse proven Helix/Zed patterns already in `legion-lsp` |
| C-R7 | Config ownership accidentally pulled into `legion-ui` (invariant break) | Low | High | ADR-0041 makes app the owner; CI dependency gate already forbids the imports; review checklist item |

---

## 11. Quality Bars & KPIs (additions)

- **Manual-IDE-without-AI bar (C2):** every GP-1 + power-user flow (splits, snippets, macros, modal nav, theme/keymap swap) works with networking disabled; a "zero-egress in Manual" test passes continuously.
- **Customization:** settings hot-apply < 500ms; keymap/theme switch with no restart; settings/keymap/theme conformance suites green; ≥95% of commands bindable; VS Code theme import success on a curated set.
- **Autonomy legibility:** 100% of mutations (every level) audited with the governing lattice cell + risk rule; manifest-equals-egress green; **approval prompts per completed task trend down while audit coverage stays 100%** (the Anthropic "sandboxing reduces prompts" lesson, applied to envelopes).
- **Full-automation safety (C5):** kill switch halts < 2s with sandbox reaping; full-run rollback restores pre-run state in 100% of safety-suite cases; zero auto-apply outside the signed envelope (blocking).

---

## 12. Immediate Next Actions

1. Ratify this plan (or amend) and open ADR-0041..ADR-0045 as drafts in `plans/adrs/`.
2. **C1 critical path, start now:** WS-03 rust-analyzer spawn + WS-05 fixture→PTY swap + WS-06.T1 ripgrep — the three activation tasks that make the IDE real. Stand up 3-OS CI if not already green.
3. Land WS-21.T1–T2 (layered config + `settings.json` schema) early — it is the substrate every other customization workstream binds to.
4. Spike ADR-0042 modal engine scope (the Vim conformance subset) and ADR-0044 lattice schema in parallel; both are design-heavy and unblock C2/C3.
5. Begin the dogfooding journal the moment LSP + terminal are live; its friction log preempts roadmap work (the C1 gate).
