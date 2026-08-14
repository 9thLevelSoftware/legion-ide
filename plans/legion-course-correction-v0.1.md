# Legion IDE — Report-vs-Reality Course Correction Plan

> **For Hermes:** This is a strategic review plan, not a code-implementation plan. Use it
> to (a) decide which findings the user wants acted on, then (b) dispatch bounded
> implementation via subagent-driven-development per finding. Do NOT start coding from this
> file directly.

**Goal:** Objectively and adversarially compare the 2026 IDE research report
(`~/legion-ide-research-2026.md`) against what Legion has actually designed and built
(30 crates, ~296K LOC Rust, 2,166 tests), identify what was done right vs wrong, and
specify a course correction that replaces the wrongs with more rights.

**Inputs compared:**
- Research report: `/Users/christopherwilloughby/legion-ide-research-2026.md`
- Codebase: `/Users/christopherwilloughby/legion-ide` (HEAD `236a492`)
- Authoritative docs: `README.md`, `CODEBASE.md`, `plans/legion-production-master-plan-v0.2.md`,
  `plans/architecture-review-2026-ide-roadmap-v0.1.md`, `plans/product-readiness-ledger.md`

**Architecture reality (verified):** 30-crate workspace, clean 7-layer DAG. `legion-app`
composition root (51K LOC), `legion-ui` projection-only (emits `CommandDispatchIntent`,
~40 variants), `legion-desktop` egui/eframe 0.34.2 renderer, `legion-text` rope buffer,
tree-sitter 0.26.9 (10 grammars), LSP client, DAP debug, VT100 terminal, Tantivy search,
Wasmtime WASM plugin runtime with hostile fixtures, OT collaboration engine, mTLS remote
transport, ChaCha20-Poly1305 retention vault. Proposal-mediated writes, metadata-only
egress, deny-by-default security. 21 standing gates + xtask + golden paths + hostile evals.

---

## PART 1 — OBJECTIVE VERDICT

Legion's stated moat is the *trust stack*: proposal-mediated mutation, metadata-only
egress, default-deny capabilities, air-gap options, privacy inspector, auditable decision
surfaces. The 2026 market independently validates this exact direction — Cursor 3, Devin
Desktop, GitHub Agent HQ, Codex, Claude Code, Zed ACP, JetBrains Junie, and Kiro all
converge on agent orchestration, plans/specs, sandboxes, approvals, evidence, MCP/ACP, and
diff-first review. Legion's architectural instincts are therefore correct and forward-looking.

The honest risk (which the repo's own `legion-production-master-plan-v0.2.md` names as
"evidence drift") is: **~296K LOC of substrate that is still not a daily-driver editor.**
The product-readiness ledger marks almost every core gate — renderer latency, full LSP UX,
debug/test UX, large-file behavior — as "substrate validated," not "product workflow
validated." The moat is real; the editor that carries it is not yet proven.

---

## PART 2 — WHAT WAS DONE RIGHT (keep these; they map directly to the research)

| # | Finding | Evidence | Maps to report |
|---|---------|----------|----------------|
| R1 | Rust-native substrate, no Electron | `Cargo.toml` (Rust 2024, 30 crates) | A1/A7 — the #1 trend and #1 want |
| R2 | Consumed tree-sitter + LSP + rust-analyzer instead of reinventing | 10 grammars; `legion-lsp`; rust-analyzer smoke gate | A3 — exactly the recommended posture |
| R3 | WASM sandbox with hostile fixtures, ABI versioning, quotas | `legion-plugin` (Wasmtime); `capability_probe.wat`, `oom.wat` | A2 + C4 — supply-chain trust; AHEAD of Zed here |
| R4 | Trust stack: proposal-mediated writes, metadata-only egress, deny-by-default | `SaveWorkflowService`; `DenyByDefaultBroker` (20+ namespaces); secret scanner | B2 + C1 — the antidote to AI-bloat fatigue |
| R5 | Built-in DAP debugger, VT100 terminal, Tantivy search, OT collab, mTLS remote | `legion-debug`, `legion-terminal`, `legion-project`, `legion-collaboration`, `legion-remote` | A4/A5/A6 — all "mandatory" 2026 layers present at substrate level |
| R6 | Exceptional engineering discipline | 2,166 tests; 21 gates; xtask; cargo-deny; golden paths 1–5; hostile evals; 3-OS CI | — (enviable process) |
| R7 | Honest self-assessment culture | ledger distinguishes "substrate validated" from "product workflow validated"; does not inflate | — (rare and strategically vital) |

**Verdict on the fundamentals:** Legion bet on the correct architectural thesis and built
an unusually disciplined substrate. The trust stack is genuinely differentiated and
validated by market convergence.

---

## PART 3 — WHAT WAS DONE WRONG / WHERE THE REPORT FLAGS RISK (adversarial)

These are ranked by severity. Each names the concrete evidence, the report section it
contradicts, and the correction (expanded in Part 4).

### W1 (HIGHEST) — Rendering stack is egui, not GPU-native. Contradicts report A1.

The report's single strongest technical finding is that the 2026 battleground is
GPU-backed native rendering (Zed GPUI→Metal/DirectX, 2ms input latency; IntelliJ→native
Wayland→Vulkan). Legion renders with **egui/eframe 0.34.2** — an immediate-mode, CPU/OpenGL
canvas framework the report explicitly flags: *"looks like a debug UI"*, styling "is not
the point," and (via the wgpu HN thread) that hand-rolling a retained editor on an
immediate-mode base is exactly the pain GPUI users describe ("even input components aren't
in gpui... you write cursor/selection/clipboard from scratch"). Legion's own gate
`no-egui-textedit` confirms they are hand-rolling the text renderer — fighting the
framework for the hard parts (IME, selection, caret, scrolling) while paying an
immediate-mode redraw tax. This is the single largest divergence between the research and
the build, and it is unvalidated: the perf-harness is still a "skeleton"/"stand-in" and
there are no published renderer benchmarks.

### W2 (HIGH) — "AI-native" positioning invites the exact fatigue users are fleeing. Report C1.

`README.md` opens with *"control-first, AI-native Rust IDE substrate."* The research's #1
user frustration is forced/inescapable AI ("every update there are new AI-related features
I need to figure out how to disable"; users leaving VS Code's 100K-extension ecosystem over
it). "AI-native" is the marketing frame of the products users are abandoning. The trust
stack is the moat; the headline should be *control-first, privacy-first, fast native
editor* with AI as an opt-in layer — not "AI-native." Additionally, the default desktop
assist path still routes through a `deterministic-local` fixture provider (placeholder
strings, not a live model) per the readiness ledger, so "AI-native" overpromises what ships.

### W3 (MEDIUM-HIGH) — No Vim mode at all. Report B6 names it a top want and a switcher blocker.

`grep -ril "vim" crates/` returns nothing. The research found Vim keybindings/.vimrc
compatibility are a concrete blocker for switchers ("I was disappointed that I could not
just use my .vimrc, have they fixed it yet? It's currently the only blocker for me") and
that the target customer — "senior engineers who want control" — is heavily modal-editing
saturated. For a control-first editor aimed at senior engineers, shipping without even a
basic Vim mode is a self-inflicted gap.

### W4 (MEDIUM-HIGH) — No call-graph / call-hierarchy navigation. Report B6 names it a voiced unmet need.

`grep -ril "call.graph|call_hierarchy|incoming.calls" crates/` returns nothing. The research
found this is a real, voiced gap in fast editors: *"the only hard blocker I have right now
is lack of a call-graph navigation widget."* Zero references in the codebase. Concrete,
differentiating opportunity unaddressed.

### W5 (MEDIUM) — 100MB large-file streaming gap still open. Report A7 headline win not yet claimable.

`AGENTS.md` itself states: *"the ignored 100MB performance workload is a known
degraded/streaming-mode gap, not a green benchmark."* Large-file speed (Zed opens 100K-line
files ~8x faster than VS Code) is one of the most citable native-editor wins; Legion cannot
make it yet. It is scheduled (WS-MANUAL-02) but not done.

### W6 (MEDIUM) — "Editor vs platform" scope risk. Report D9 + the repo's own master plan §5.3.

30 crates including remote, collaboration, telemetry, retention, vscode-compat, sandbox,
memory, tracker, agent — built as substrate while the *core editor* (PR-UI-001 renderer
latency, PR-LANG-001 full LSP UX, PR-LANG-002 debug/test UX) remains "substrate validated."
The repo's own `legion-production-master-plan-v0.2.md` §5.3 warns "do not build cloud
remote before Manual/Assist are daily-drivable," yet those crates exist. This is the
"ego-driven scope" the research warns against. The correction is not to delete them but to
freeze new surface expansion until Manual mode is provably daily-drivable.

### W7 (LOW-MEDIUM) — Proprietary + no marketplace = no extension ecosystem at a time it still matters. Report A2.

`license = "Proprietary"`, `publish = false`. The WASM extension runtime exists but there is
no marketplace, and ~1,000-extension Zed is itself struggling to close the 100K gap. A
proprietary enterprise play is defensible, but it forecloses the community momentum that
feeds extension breadth — and extension breadth is the single biggest remaining reason users
stay on VS Code. Needs a deliberate decision (enterprise-trust play vs community-growth
play), not a default.

### W8 (LOW) — Local checkout was stale (297 commits behind remote). Resolved by fast-forward.

Local repo HEAD was `236a492` while `origin/main` had advanced to `95229b3` (297 commits
ahead, plus a new `v0.0.1` tag). This was a stale working copy, not unpushed work. Resolved
before committing this plan: `git pull --ff-only origin main` brought local to `95229b3`.
Lesson for future reviews: always `git fetch` and check `rev-list --count HEAD...origin/main`
before drawing any divergence conclusion.

### W9 (LOW) — Canonical docs deleted by a broad cleanup commit while README/INDEX still referenced them. FIXED.

Commit `293d80f` ("clean up", 2026-08-12) swept away `.almanac/`, `.omh/`, and — as
collateral — the two canonical docs `docs/LEGION_PIVOT.md` and `docs/LEGION_RENAME.md`,
while `README.md` (lines 22, 111) and `docs/INDEX.md` (lines 16, 18, 27, 30) still list
them as primary docs. `LEGION_RENAME.md` documents a real validator rule ("validators
intentionally accept historical markers when checking archived evidence") that the xtask
evidence gates implement, and `LEGION_PIVOT.md` is the canonical product/roadmap entry
point. Both were restored from history (`git checkout 293d80f^ -- docs/LEGION_PIVOT.md
docs/LEGION_RENAME.md`). Root-cause note: the `docs-hygiene` gate's `BrokenRelativeLink`
check only catches Markdown-link syntax (bracket-then-parenthesis), not backtick-quoted
prose references like `` `docs/LEGION_PIVOT.md` ``, so this class of dangling-reference
regressed silently.
A follow-up could teach `docs-hygiene` to also validate backtick `docs/*.md` references in
`README.md`/`docs/INDEX.md`.

---

## PART 4 — COURSE CORRECTION (replace wrongs with rights)

Priority order. Each item is a bounded, independently-actionable work packet with exact
paths and acceptance. Execution should use subagent-driven-development per packet after the
user picks which to fund.

### P0 — Prove Manual mode is a daily driver (blocks everything else)

**Why:** The report's D1/D9. No other finding matters if the editor isn't a credible daily
driver. This is the "make Manual mode a boring, excellent native IDE" workstream.

- Define and publish editor latency budgets: keypress→paint p50/p95, scroll p95, open-file,
  save, LSP completion (extends `xtask perf-harness`, currently a skeleton).
- Ship a renderer-backed input-to-paint measurement (not the in-process stand-in).
- Add IME + clipboard + focus smoke tests per OS (WS-MANUAL-01.4/5/7).
- Produce a real benchmark table (Legion vs VS Code vs Zed) with evidence — the report's
  Part A shows this is *the* marketing wedge.
- **Renderer decision gate (the W1 correction):** make an explicit, documented ADR choice:
  (a) stay on egui and prove the budgets anyway (accept the polish/latency ceiling), or
  (b) evaluate `egui-wgpu` backend for GPU rendering, or (c) plan a migration path toward a
  GPUI-class renderer. Do NOT leave this implicit. `docs/` has no such ADR today.

**Touches:** `crates/legion-desktop`, `crates/legion-ui`, `crates/legion-app`,
`crates/legion-editor`, `crates/legion-text`, `xtask/`.
**Acceptance:** PR-UI-001 and PR-LANG-001 move from "substrate validated" to "product
workflow validated" with named evidence; a benchmark table is committed under `audit-reports/`.

### P1 — Reposition away from "AI-native" (the W2 correction)

- Rewrite `README.md` opening to lead with *control-first, privacy-first, fast native
  editor*; demote "AI-native" to a secondary, opt-in capability description.
- Update `docs/INDEX.md` and the mode docs (`docs/MODES.md`) to reflect AI as an explicit
  opt-in layer with the default-desktop path honestly labeled (currently `deterministic-local`
  fixture, not a live model).
- Add a one-paragraph "privacy posture" statement to `README.md` that directly answers the
  #1 2026 frustration (no default-on AI, no re-enabling disabled features, zero egress in
  Manual mode).
**Touches:** `README.md`, `docs/INDEX.md`, `docs/MODES.md`.
**Acceptance:** A fresh reader's first impression is "fast, private, control-first," not
"another AI IDE."

### P2 — Add Vim mode + call-graph navigation (the W3/W4 corrections; highest want-per-cost)

- **Vim mode (P2a):** design and land a modal editing layer: normal/insert/visual modes,
  hjkl motion, w/b/e word motion, dd/yy/p, u/redo, search `/`, and `.vimrc`-style keymap
  import as a stretch. Implement in `legion-editor`/`legion-ui` (the command-intent layer),
  NOT in the renderer. Reference: report B6 (Vim mode is a top want and a switcher blocker).
- **Call-graph navigation (P2b):** add an incoming/outgoing call-hierarchy view fed by
  `legion-index` symbol graph + `legion-lsp` callHierarchy/outgoingCalls (LSP 3.17) where
  available, with tree-sitter fallback. Project as a dock panel via `legion-ui`, render in
  `legion-desktop`. Reference: report B6 ("the only hard blocker... call-graph navigation
  widget").
**Touches:** `crates/legion-editor`, `crates/legion-ui`, `crates/legion-index`,
`crates/legion-lsp`, `crates/legion-desktop`, `crates/legion-protocol`.
**Acceptance:** A vim-mode session survives a full edit→save cycle; call-graph shows
real callers/callees for a Rust fixture with TDD coverage in both crates.

### P3 — Close the 100MB large-file gap (the W5 correction)

- Implement/harden streaming text viewport so 100MB files do not materialize full caches
  (report A7 — the "8x faster large-file open" claim is the citable win).
- Add binary-file detection + safe preview refusal, file-size policy UX, and a measurable
  memory ceiling (WS-MANUAL-02 tasks).
**Touches:** `crates/legion-text`, `crates/legion-editor`, `crates/legion-project`,
`crates/legion-app`, `xtask`.
**Acceptance:** `AGENTS.md`'s "known gap" note is deleted and replaced with a green,
measured large-file benchmark; the ledger's PR-UI-002 row updates.

### P4 — Freeze new-surface expansion until Manual ships (the W6 correction)

- Adopt a standing rule (ADR + `AGENTS.md` + a new `xtask` check if desired): no new
  runtime crate or activation of a deferred surface (remote, collaboration, telemetry,
  retention, vscode-compat sidecar) without a Manual-mode daily-driver milestone behind it.
- Mark the currently zero-fan-in crates (`legion-remote-transport`, `legion-retention`,
  `legion-telemetry`, `legion-vscode-compat`) with an explicit status banner in their
  `README`/crate docs: "substrate complete; product activation gated on X."
**Touches:** `plans/adrs/` (new ADR), `AGENTS.md`, the four zero-fan-in crate docs.
**Acceptance:** No new surface lands without the milestone gate; the zero-fan-in crates
self-describe their gated status.

### P5 — Make an explicit extension-ecosystem decision (the W7 correction)

- Write an ADR resolving: enterprise-trust play (proprietary, curated allowlist) vs
  community-growth play (some open extension path). At minimum, define a v1 extension
  distribution mechanism (signed allowlist? Open VSX read-only?) so the WASM runtime has a
  delivery channel. Do not leave "no marketplace" as an unconsidered default.
**Touches:** `plans/adrs/`, `docs/SECURITY.md`, `crates/legion-vscode-compat` docs.
**Acceptance:** An ADR documents the decision and the v1 distribution channel.

### P6 — Keep local in sync with remote (the W8 correction)

- DONE in this session: `git fetch` + `git pull --ff-only origin main` synced local to
  `origin/main` (`95229b3`). Add a standing pre-review habit: fetch and diff-count before
  concluding divergence. One-line hygiene, do first every review cycle.

---

## PART 5 — RISKS, TRADEOFFS, OPEN QUESTIONS

- **Renderer bet is the fork in the road.** Staying on egui is lower-risk-to-ship but caps
  the headline performance story; migrating toward a GPUI-class renderer is high-cost and
  (per the wgpu HN thread) not guaranteed to beat a well-tuned native renderer. This needs
  the user's explicit call — it is a product-identity decision, not a technical detail.
- **"Trust stack as moat" vs "users want speed."** The moat is real but the research also
  shows speed/feel is the #1 stated want. The trust stack must not become a reason the
  editor feels slow or over-gated in Manual mode (proposal-mediated *saves* are fine; don't
  put the AI proposal pipeline on the keystroke path).
- **Proprietary vs community.** Enterprise-trust (proprietary, audited) and community
  growth (open, extensible) pull in opposite directions. Decide explicitly (P5) rather than
  let the license field decide by default.
- **Scope discipline credibility.** The repo already self-diagnosed "evidence drift." The
  course correction only works if P0/P4 are enforced — otherwise this is another plan file
  among 30+ existing plan files.
- **Open question for the user:** which of P0–P6 to fund first? Recommended order: P6 (1
  line), P0 (renderer decision + Manual proof), P1 (positioning), P2 (Vim + call-graph), P3
  (large files), P4 (freeze), P5 (extension ADR).

---

## PART 6 — VERIFICATION

- P0: benchmark table committed under `audit-reports/`; `xtask perf-harness` upgraded from
  skeleton to renderer-backed; ledger rows PR-UI-001/PR-LANG-001 promoted with named evidence.
- P1: `README.md` + `docs/MODES.md` diff reviewed by a non-author reader.
- P2: new `#[test]` coverage in `legion-editor` (vim mode) and `legion-lsp`/`legion-index`
  (call graph); `cargo test -p <crate>` green.
- P3: `AGENTS.md` gap note removed; new measured large-file test green.
- P4: new ADR present; `cargo run -p xtask -- check-deps` and `docs-hygiene` still pass.
- P5: ADR file present and referenced from `docs/INDEX.md`.
- All: the 21 standing gates (`cargo test --workspace --all-targets --no-fail-fast`,
  `cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --all --check`,
  `cargo deny check`, xtask gates) remain green after every packet.

---

## SUMMARY

Legion did the hard architectural work *right* — Rust-native, tree-sitter/LSP consumption,
WASM sandboxing, and a trust stack that the 2026 market independently validated. The wrongs
are concentrated and correctable: (1) an unvalidated egui renderer where the market leads
with GPU-native, (2) "AI-native" framing against the year's biggest user frustration, (3) no
Vim mode, (4) no call-graph navigation, (5) an open large-file gap, (6) substrate sprawl
ahead of a proven daily driver, (7) an unconsidered extension-ecosystem decision. The
course correction prioritizes proving Manual mode first, then repositioning, then the
high-want/low-cost features (Vim + call-graph), then hardening, freeze discipline, and an
explicit ecosystem decision.
