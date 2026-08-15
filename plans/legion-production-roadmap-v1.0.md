# Legion Production Roadmap v1.0 — Current State → GA

Adopted 2026-08-14. This roadmap **plugs into** the master plan
(`plans/legion-production-master-plan-v0.2.md`, milestones M7–M13) and the
product-readiness ledger; it does not replace them. It sequences the remaining
work as dependency-gated phases (no calendar dates) for a solo owner working
with AI agents, and it folds in the local-first AI acceleration track adopted
by ADR-0049 (SmallCode behavioral cannibalization).

External inputs (not in this tree): the 2026 IDE market research and the
local-first AI deep-research report reviewed on 2026-08-14. Their theses, in
one line each: developers want a deterministic, native, offline-capable IDE
with AI as a strictly opt-in utility; and Legion's missing AI layer is
small-model *reliability mechanics* (routing, tolerant parsing, plan
anchoring, patch-first editing), not another AI architecture.

## Tracks

- **Track A — the IDE** (Phases 1, 6): Manual-mode daily driver, then
  language/extensibility breadth.
- **Track B — the AI control plane** (Phases 2, 3, 4): SmallCode behavioral
  port → local-model UX → managed runtime + model manager.
- **Track C — delivery & trust** (Phase 0 procurement, Phase 5): signing,
  update feed, crash reporting, sandbox hardening.

Tracks A and B run in parallel (disjoint crate footprints); Track C is mostly
external-credential-gated (`EXT-*` ids in the kanban backlog).

## Serialization rules

1. Phase 0 truth repair precedes everything.
2. **Extract-before-modify in `crates/legion-app/src/lib.rs`**: no feature edit
   lands inside `lib.rs`; the touched region is extracted to a module in its
   own commit first (Track A: intent routing → `intent_routing.rs`; Track B:
   assist proposal path → `assist_proposal.rs`).
3. Patch-first proposal generation (Phase 2) lands before the fixture default
   is retired (Phase 3) — a real model never ships through the
   `TextRange::byte(0,0)` insertion path.
4. PR-UI-001 promotion (Phase 1 exit) precedes any daily-driver claim, and is
   the ADR-0046 unfreeze criterion.
5. Ollama/llama.cpp tool-calling (Phase 3) precedes the managed sidecar
   (Phase 4).
6. Release signing keys (Phase 5) precede extension bundle signing (Phase 6,
   ADR-0047).
7. The standing gate set stays green at every merge.

## Phases (summary — full work-item tables live in the kanban backlog)

| Phase | Goal | Maps to | Exit gate (ledger currency) |
| --- | --- | --- | --- |
| **0 — Truth & baseline repair** | Ledger/backlog/docs tell the truth; smallcode governance (ADR-0049) in place; bench raw baseline frozen; long-lead purchases started; hosted 3-OS smoke activated | M7 residue / WS-P0 | All standing gates green; `cargo check -p legion-app --no-default-features` CI-enforced; ADR-0049 accepted; baseline evidence at `plans/evidence/production/BENCH/baseline-raw-v1.md`; first hosted 3-OS smoke run recorded |
| **1 — Manual-mode daily driver** (Track A) | The boring, excellent, zero-AI native IDE: Vim wired, multi-cursor, streaming projections, LSP daily loop, Git surface, CodeLLDB, strict perf budgets (ADR-0048) | M8 | **PR-UI-001 and PR-LANG-001 → product workflow validated**; GP-1 green on 3-OS CI; ≥5-day dogfood journal with no P0/P1 |
| **2 — AI control plane** (Track B, parallel with 1) | SmallCode port per ADR-0049: patch-first proposals, tolerant tool-call normalizer, loop governors (dedup / read-before-write / progress early-stop / plan anchoring), model profiles + context budgets, schema narrowing | M9 prep | GP-2/3/4 stay green; governed loop ≥20% relative over the frozen raw baseline on the held-out bench subset |
| **3 — Local model UX** | Real local model by default (fixture retired), Ollama/llama.cpp tool-calling, provider setup UX, hardware/memory-fit, offline egress gate, live-model hostile evals, streaming-active perf guard | M9 residue | PR-AI-001 fixture caveat retired with named evidence; GP-2 with a real local model; Manual zero-egress smoke still green |
| **4 — Managed runtime & model manager** | "Enable Local AI" on a clean machine: sidecar supervisor (module in `legion-ai-providers`, ADR-0050), verified resumable model downloader, curated Apache-2.0 catalog, first-run wizard, Fast/Balanced/Strong | M9/M10 | Clean-machine E2E wizard → download → verify → run → GP-2/GP-3 on managed runtime; four-check offline egress proof |
| **5 — Delivery & trust** (Track C) | Signed installers (Azure Trusted Signing / Developer ID + notarization / minisign), update feed + `HttpManifestSource` + installer swap (ADR-0042 D5), native minidump, Windows sandbox hardening, secret-scanning upgrade, SBOM/model BOM | M12 | **PR-REL-001 → product workflow validated**; P8.F1.T3 unblocked → done; extended `update-drill` green 3-OS; fresh-VM evidence on file |
| **6 — Extensibility & breadth** (Track A) | Real plugin host (WIT ABI, fuel/epoch/memory limits, host imports, hostile suite), plugin lifecycle UI + signed bundles, LSP breadth (TS/pyright/gopls), tree-sitter workers beyond Rust, Tantivy on-disk persistence | M11/M12 residue | GP-5 promoted to a standing gate and green; per-language smokes; a real example plugin ships |
| **7 — GA hardening** | Enforced 3-OS perf matrix + dashboard, vLLM/self-hosted endpoint tier, enterprise offline bundle, external security + license audit, GP-6 harness | M13 | GP-1..GP-6 green in CI; ledger has zero rows citing stale evidence; audit findings triaged; full install→update→rollback qualification drill |

## Cross-cutting rules

1. Extract-before-modify in `legion-app/src/lib.rs` (enforced by a lightweight
   xtask check once Track A/B both run).
2. Every phase boundary flips its ledger/backlog rows in the same change;
   `verify-readiness-consistency` must pass on the merged state.
3. Claim honesty: `claim-audit` stays green; AI remains visibly subordinate to
   the editor; route changes to stronger/hosted models are explicit consent
   events (`NeedsStrongerModel`), never silent escalation.
4. MIT attribution for every SmallCode-derived behavior
   (`THIRD_PARTY_NOTICES.md`, `docs/legal/smallcode-attribution.md`).
5. Anti-scope holds: ADR-0046 clauses 2–3 (remote/collab/vscode-compat) stay
   frozen through GA absent an ADR amendment; the first-party model catalog
   stays at 5 curated Apache-2.0 entries + BYOM.

## Top risks

1. macOS/3-OS infrastructure is the most common promotion blocker (dev machine
   is Windows) → hosted 3-OS smoke activated in Phase 0; cloud Mac in
   procurement (`plans/release/procurement-and-key-escrow.md`).
2. Bench overfitting invalidates the ≥20% governed-vs-raw claim → ≥15 tasks
   across ≥3 fixture repos with a held-out subset scored only at phase exits
   (policy in ADR-0049).
3. `lib.rs` as a merge chokepoint under agent-parallel work → rule 1 above.
4. AI streaming regressing proven ADR-0048 budgets → streaming-active perf
   scenario lands in the same phase that retires the fixture default.
5. Solo-owner key bus factor → escrow policy in
   `plans/release/procurement-and-key-escrow.md`; break-glass key-rotation
   drill added to `update-drill` in Phase 5.
