# HERMESGOAL Gap Analysis — Built vs Deferred vs Ignored

Date: 2026-07-01
Baseline commit: `236a492` (`feat: advance Legion productionization surfaces`)
Sources reconciled: `HERMESGOAL.md`, `plans/legion-production-master-plan-v0.2.md`, `plans/product-readiness-ledger.md`, `plans/kanban/legion-ga-backlog.toml`, `plans/evidence/production/`, current crate sources.

## 1. Headline verdict

`HERMESGOAL.md` is a prompt-form restatement of `plans/legion-production-master-plan-v0.2.md` — its workstreams (WS-P0 through WS-QUALITY-01) and milestones (M7–M13) map 1:1 to the v0.2 plan. So the gap question reduces to: where is the repo on the v0.2 milestone path?

**Position: M7 is complete (evidence refreshed 2026-07-01). M8 is roughly one-quarter done by evidence (2 of 8 workstreams evidenced), though more than that by code. M9–M11 have substantial substrate and fresh code slices but no golden-path validation. M12–M13 are open with explicit cut lines.**

The single readiness row at "Product workflow validated" is PR-AI-001 (inspectable local-first AI). Everything else is substrate-validated, in progress, or deferred with an explicit cut line. That matches the v0.2 executive verdict: the risk is not missing architecture, it is **evidence drift** — and this analysis found several new instances of drift introduced by the latest commit (section 5).

## 2. Milestone-by-milestone status

### M7 — Truth and Beta Rebaseline: COMPLETE (with two loose ends)

Evidence: `plans/evidence/production/WS-P0/WS-P0-rebaseline-evidence-refresh.md` (2026-07-01).

| M7 item | Status |
| --- | --- |
| WS-P0 rebaseline | Done — v0.1 marked historical; README/INDEX/USER_GUIDE point to v0.2 |
| Ledger reconciliation | Done — ledger distinguishes substrate vs product validation per row |
| Golden path definitions | Done — defined in v0.2 §8; GP-1 manual accessibility walkthrough transcript captured |
| Kanban/backlog validation | Done — `verify-kanban-backlog` passes (10 epics, 38 features, 146 tasks) |
| Docs-hygiene gate | Done — passes |
| **Claim-audit script or checklist** | **MISSING** — no `claim-audit` script, xtask subcommand, or checklist exists anywhere in the repo |
| Dogfood journal template | Done, but path drift: template lives at `plans/dogfood/…` while the ledger references `dogfood/…` (nonexistent). Zero journal entries exist. |

### M8 — Manual Daily Driver Beta: IN PROGRESS (the active milestone)

| Workstream | Code state | Evidence state | Gap |
| --- | --- | --- | --- |
| WS-MANUAL-01 editor feel/input | Built | **Complete** (2026-06-19): latency budgets, input-to-paint perf harness, no-egui-textedit gate, IME/clipboard/focus tests, font fallback, wrapping, degraded banners, zero-egress smoke. Rectangular selection explicitly deferred. | Closed |
| WS-MANUAL-02 large files/scale | Built | **Complete**: 10/10 SCALE tasks — 100MB streaming viewport, binary refusal, `FileSizeClassification`, watcher debounce, search cancellation, memory ceiling, stale-lease tests. | Closed |
| WS-LANG-01 Rust LSP | Real primitives exist (`LspSupervisor`, `LspStdioLauncher`, framing, init handshake, diagnostics ingestion). rust-analyzer launch is env-var opt-in with a mock fallback under test. | Substrate tests only; **no WS-LANG-01 evidence dir**. Ledger: "Full LSP completion/diagnostics product UX path is not yet validated." | Real rust-analyzer product workflow + lifecycle/crash UX + platform smoke |
| WS-LANG-02 syntax/symbols | tree-sitter overlays in `legion-index`; structural search workflow test passes | No dedicated evidence | Overlay caching, outline/breadcrumbs, rewrite-as-proposal evidence |
| WS-TERM-01 terminal | **Real**: `legion-terminal` has ConPTY, grid, OSC, session; `legion-platform` has Windows/Unix PTY. App composes `TerminalRuntime<NativePtyService>`. Latest commit touched 9 terminal files. | No WS-TERM-01 evidence dir | Shell policy, orphan cleanup evidence, redaction classification, platform smoke |
| WS-DEBUG-01 debug/test explorer | **Not real**: `legion-debug` is DAP protocol/state modeling and evidence types only — it never spawns a process. No adapter launch, no cargo-test explorer against real runs. | Projection tests only | Largest M8 gap. HERMESGOAL allows "v1 critical path **or explicit beta cut line**" — neither exists; a decision is required |
| WS-SEARCH-01 search/nav | Built: streaming workspace search, indexed search state, fuzzy open, palette (`legion-desktop/src/search.rs`) | Substrate tests only | Query grammar, cancellation/stale markers evidence, large-repo measurement |
| WS-GIT-01 git/review | Built: real git shell-out; app (6), desktop (2), project git workflow tests pass | Substrate tests only | Hunk stage/unstage UX, merge-conflict route, local history, worktree UI evidence |

**M8 exit criteria not yet attempted:** GP-1 pass on the Legion repo (walkthrough transcript exists but GP-1 is not automated), and "one week of dogfood or equivalent documented repeated runs" (zero journal entries).

### M9 — Assist Private Beta: SUBSTRATE STRONG, NOT VALIDATED

- PR-AI-001 is **Product workflow validated** — the only row at that level. Privacy inspector, retention tombstones, inline prediction accept/reject, context manifests all have passing tests.
- Providers are real: native Anthropic client (messages/count-tokens/streaming/batch), OpenAI-compatible path, capability metadata (new in `236a492`), provider smoke fixtures.
- Gaps: live local-provider smoke (Ollama/llama.cpp) is recorded-fixture only; cost estimate/actual records partial; GP-2 (assist multi-file change with local + BYOK provider) not run.

### M10 — Delegate Public Beta: FRESH CODE, NO VALIDATION

- Commit `236a492` added large `legion-agent` slices: DAG, scheduler, plan artifacts, scope enforcement, merge readiness, evidence, external-agent, comm, worktree sandbox tests, plus the `legion-sandbox` crate.
- Ledger blocker: **adversarial evals remain dry-run scaffolds** (PR-AI-002 deferred portion); `evals/legion-bench/hostile/*.toml` are 3-line stubs. GP-3 not run. Sandbox-escape suite not evidenced.

### M11 — Workflow Command Center: PROJECTIONS EXIST

- `legion_workflow_command_center` (6) and `delegated_task_command_center` (5) desktop tests pass; ACP host wiring exists.
- Gaps: ACP external-agent interop has no ADR acceptance; fleet kill-switch verification and replay/evidence export not evidenced as product workflows. GP-4 not run.

### M12 — Production Beta Release: IN PROGRESS with honest cut lines

- Built: packaging/platform-smoke tests pass; `release-pipeline`/`verify-release-pipeline` xtask gates; unsigned-beta policy cut line recorded (M5 WS17-T2); docs/support-bundle coverage recorded (WS17-T6); `legion-bench` xtask with recorded/live modes.
- Not validated: signed installers, auto-update/rollback, crash-report controls, fresh-VM smoke. GP-5/GP-6 not run.

### M13 — GA Readiness: NOT CLOSE

GP-2 through GP-6 have not passed; release/update/crash/support evidence is not current. This is expected at this stage — no overclaim found in the ledger itself.

## 3. Deferred with explicit cut lines (healthy — do not "fix")

These are intentionally deferred per ledger/AGENTS.md and must stay explicit, not be silently implemented or marketed:

- PR-VSC-002: runtime extension host sidecars, webviews, notebooks, custom editors, extension storage, marketplace execution.
- PR-ENT-001: remote development UX (mock/default-deny transport only; 13 tests validate contracts, not production transport).
- PR-ENT-002: collaboration/admin (CRDT reconciliation, shared proposals, replay, admin policy export).
- Adversarial eval real execution (dry-run scaffolds acknowledged in ledger).
- Rectangular selection (WS-MANUAL-01 decision recorded).
- Signed installers (explicit unsigned-beta policy, M5 WS17-T2).

## 4. Ignored or missing relative to HERMESGOAL

1. **Claim-audit script/checklist** (M7/WS-P0 and WS-QUALITY-01 item): does not exist in any form.
2. **Dogfood loop**: template created today, zero entries, ledger points at the wrong path (`dogfood/` vs `plans/dogfood/`). The M8 exit criterion depends on this loop running.
3. **Kanban backlog has no status/progress field**: 146 tasks with no completion state, and `meta.milestone = "M0"` with a P0–P9 epic taxonomy that does not match the v0.2 M7–M13 milestone names. "Select the next unblocked task" cannot be computed from the file; progress lives only in evidence dirs and the ledger.
4. **WS-DEBUG-01 decision debt**: neither a real DAP v1 critical path nor an explicit beta cut line exists.
5. **GP automation** (WS-QUALITY-01): GP-1..GP-6 are defined in prose; none is an automated, runnable check (legion-bench covers eval fixtures, not golden paths).
6. **Evidence dirs for 6 of 8 M8 workstreams** (LANG-01/02, TERM-01, DEBUG-01, SEARCH-01, GIT-01) do not exist under `plans/evidence/production/`.

## 5. New truth drift introduced by commit `236a492` (fix first)

These directly violate the repo's own "truth and hygiene before feature work" doctrine:

1. **Foreign-project contamination**: `audit-reports/quaternius-megakit-spike-subset.md` and `audit-reports/quaternius-calibration-adapter-proposal.md` are Godot game-asset audits for a different project ("Off The Rails", macOS user paths). They do not belong in this repo.
2. **README CI claim now false**: `.github/workflows/legion-bench.yml` was added, but `README.md` (and HERMESGOAL §4) still state "No GitHub Actions CI workflow is currently configured."
3. **`docs/releases/v8.0.0/` overclaim hazard**: a "v8.0.0 GA Release Checklist" (with a `0.1.0 → 8.0.0` version bump and references to a nonexistent `.github/workflows/ci.yml:166`) sits in docs while the product is pre-beta. If kept, it must be explicitly marked as a forward template, not a current release artifact.
4. **Gate doctrine not followed for a broad packet**: the commit's recorded verification ran targeted crate tests only — no full `cargo test --workspace`, no clippy, no cargo-deny — despite HERMESGOAL §4 requiring the full standing gate set for broad implementation packets.
5. **Known-failing TDD tests on main**: the WS-P0 refresh records failing tests (`input_conformance` trust-policy failures; TDD tests referencing unimplemented types in legion-ai/legion-plugin/legion-storage) plus unused-import warnings that would fail `clippy -D warnings`. Full-workspace gate status at HEAD: see §6.

## 6. Current gate health at HEAD

- `cargo check --workspace --all-targets`: **passes** (verified this session, exit 0).
- `cargo run -p xtask -- docs-hygiene`: **passes** (verified this session, including this file).
- `cargo test --workspace --all-targets --no-fail-fast`: **could not be cleanly verified this session.** Three attempts (including one after `cargo clean`) hit build-artifact corruption ("invalid metadata files", freshly built rlibs "not found in this form") consistent with concurrent IDE cargo runs or antivirus interference on Windows during large parallel builds. Independent of that noise, the WS-P0 refresh (2026-07-01) records known failing tests on main: `legion-desktop --test input_conformance` (trust policy denies temp workspace paths) and TDD tests referencing unimplemented types in `legion-ai`, `legion-plugin`, `legion-storage`. Treat "full standing gates green at HEAD" as **unproven** until a clean run is captured as evidence.
- Unused-import warnings exist in `legion-app`/`legion-agent` sources — `cargo clippy --workspace --all-targets -- -D warnings` will fail until cleaned.
- Runtime cleanup gap observed live: two orphaned delegated-task sandbox copies (`task-acp-host-*`) were found under `crates/legion-app/target/delegated-tasks/`, each containing a full repo copy whose duplicated `evals/test_run_eval.py` broke workspace pytest collection. The stale copies were deleted during this analysis and `python -m pytest evals training` now passes (2 passed), but the root cause — delegated-task lanes not reaping their sandboxes — remains open and is concrete evidence for the WS-AGENT-01 "lane cleanup on cancel/crash/app close" requirement (gitignored, so invisible to git status — it will accumulate again until reaping is implemented).

## 7. Refined path from here to shippable

Ordered to respect the repo's own doctrine (truth first, then GP-1, then outward):

**Phase 0 — Truth repair (days).** Remove quaternius files; fix README/HERMESGOAL CI claim; fix ledger dogfood path; qualify or relocate `docs/releases/v8.0.0/`; get the full standing gate set green at HEAD (fix TDD-orphan tests and clippy warnings) and capture the run as evidence; fix delegated-task sandbox reaping (orphans under `crates/legion-app/target/delegated-tasks/`); add the claim-audit script; add a `status` field to the Kanban backlog and reconcile its milestone taxonomy with M7–M13.

**Phase 1 — Finish M8 (the daily-driver bar).**
- WS-LANG-01: real rust-analyzer product workflow on at least Windows (discovery, launch, completion/diagnostics into panels, restart/crash UX) with evidence dir.
- WS-TERM-01: terminal productization evidence (shell policy, kill/orphan cleanup, platform smoke).
- WS-SEARCH-01 / WS-GIT-01: promote existing substrate to evidenced product workflows (cancellation/stale markers; stage/commit UX).
- WS-DEBUG-01: make the decision — build the zero-config Rust debug critical path, or record an explicit beta cut line in ledger + docs.
- Automate GP-1; start the weekly dogfood journal and run it for the required week.

**Phase 2 — M9 Assist.** Live local-provider smoke (Ollama), manifest-preview-before-invocation UX, chat-to-proposal with validation/rollback/evidence export, GP-2 run (record hosted-BYOK blocker explicitly if credentials are unavailable).

**Phase 3 — M10 Delegate.** End-to-end lane: task packet → worktree → validation → proposal, with kill/cancel/reap and cleanup evidence; make the hostile eval fixtures real, blocking tests; GP-3.

**Phase 4 — M11 Workflows.** Fleet kill-switch verification, replay/evidence export, "why stopped" states, GP-4; ACP interop only behind an ADR.

**Phase 5 — M12 Beta release.** Unsigned-beta installers per the recorded policy, fresh-VM smoke (or documented platform blocker), update/rollback tested or explicitly cut, support-bundle redaction, GP-5/GP-6 to beta scope.

**Phase 6 — M13 GA** per the ledger's definition of done.
