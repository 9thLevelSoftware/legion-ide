# WS-P0 Rebaseline, Ledgers, and Plan Hygiene Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Complete WS-P0 from `plans/legion-production-master-plan-v0.2.md` by making the repository truth surface internally consistent, preserving historical v0.1 audit value, reconciling product-readiness status against accepted evidence, adding a regression-proof docs-hygiene guard for the current production master plan, auditing dirty-worktree caveats, and creating the weekly Legion-on-Legion dogfood journal template.

**Architecture:** WS-P0 is a governance and evidence packet. It must not change runtime IDE behavior. It updates Markdown truth surfaces and one small `xtask` docs-hygiene rule. The source of truth after completion is: `README.md` and `docs/INDEX.md` route readers to v0.2, v0.1 is explicitly historical, `plans/product-readiness-ledger.md` remains the controlling product-readiness ledger, dirty evidence caveats are recorded without retroactively overstating acceptance, and `xtask` fails if current entry-point docs stop pointing at the latest production master plan.

**Tech Stack:** Markdown documentation, Rust `xtask` docs-hygiene code, `cargo test -p xtask --test docs_hygiene`, `cargo run -p xtask -- docs-hygiene`, `cargo run -p xtask -- check-deps`, `cargo fmt --all --check`, `cargo check --workspace --all-targets`, and `git diff --check`.

---

## Current Branch Facts to Preserve

- `plans/legion-production-master-plan-v0.2.md` already exists on the current branch and contains the WS-P0 task list.
- `README.md` already lists `plans/legion-production-master-plan-v0.2.md` as the current production master plan and `plans/legion-production-master-plan-v0.1.md` as historical.
- `docs/INDEX.md` already lists `../plans/legion-production-master-plan-v0.2.md` as current and `../plans/legion-production-master-plan-v0.1.md` as historical.
- `plans/product-readiness-ledger.md` already has the core distinction that substrate evidence is not product readiness. WS-P0 should sharpen and evidence-map that distinction, not replace it.
- `plans/evidence/production/M0/`, `M1/`, `M2/`, `M4/`, `M5/`, and `M6/` contain formal milestone acceptance files. `plans/evidence/production/M3/` contains workstream evidence files, but no `M3-milestone-acceptance.md` was present during plan creation. Implementation must not invent retroactive M3 milestone acceptance.
- `xtask/src/perf_harness.rs` still has a comment that points to `plans/legion-production-master-plan-v0.1.md`. It should be updated as stale plan hygiene, but the new docs-hygiene rule only needs to enforce the current entry-point docs.

## Files to Edit

- `README.md`
- `docs/INDEX.md`
- `plans/legion-production-master-plan-v0.1.md`
- `plans/legion-production-master-plan-v0.2.md`
- `plans/product-readiness-ledger.md`
- `plans/evidence/production/WS-P0/dirty-worktree-caveat-audit.md`
- `plans/evidence/production/WS-P0/WS-P0-rebaseline-evidence.md`
- `plans/dogfood/legion-on-legion-weekly-journal-template.md`
- `xtask/src/docs_hygiene.rs`
- `xtask/tests/docs_hygiene.rs`
- `xtask/src/perf_harness.rs`

## Non-Goals

- Do not promote any readiness row to `Product workflow validated` unless the row already has current, direct, user-facing evidence and targeted tests.
- Do not rewrite v0.1 body claims in place beyond a short historical banner. Preserve the old document as an audit snapshot.
- Do not create or backfill a formal M3 milestone acceptance file unless a separate task validates that history. WS-P0 should state the current evidence shape accurately.
- Do not add runtime IDE features.
- Do not change dependency policy, release descriptors, product architecture, or provider behavior.

---

## Phase 0 - Baseline and Guardrails

- [ ] Run `git status --short --branch` from repo root.
  - Expected result: record the current branch and any pre-existing dirty files.
  - If unrelated dirty files exist, preserve them and avoid broad formatting unless needed by verification.

- [ ] Reconfirm WS-P0 task lines:
  ```powershell
  rg -n "WS-P0|P0\\.0|Rebaseline" plans/legion-production-master-plan-v0.2.md
  ```
  - Expected result: WS-P0 tasks P0.01 through P0.10 are visible.

- [ ] Reconfirm current entry-point plan references:
  ```powershell
  rg -n "legion-production-master-plan-v0\\.(1|2)" README.md docs/INDEX.md xtask/src/perf_harness.rs
  ```
  - Expected result: README and docs index reference both v0.2 current and v0.1 historical; `xtask/src/perf_harness.rs` still references v0.1 before cleanup.

- [ ] Reconfirm formal evidence shape:
  ```powershell
  Get-ChildItem -Path plans/evidence/production -Recurse -Filter '*milestone-acceptance.md' | Select-Object -ExpandProperty FullName
  Get-ChildItem -Path plans/evidence/production/M3 -File | Select-Object -ExpandProperty Name
  ```
  - Expected result: formal milestone files exist for M0, M1, M2, M4, M5, and M6; M3 has workstream evidence files only.

- [ ] Run the current docs gate before edits:
  ```powershell
  cargo run -p xtask -- docs-hygiene
  ```
  - Expected result: pass before WS-P0 edits. If it fails, fix only failures directly relevant to WS-P0 or record unrelated failures in `plans/evidence/production/WS-P0/WS-P0-rebaseline-evidence.md`.

---

## Phase 1 - TDD the Docs-Hygiene Latest-Plan Rule

### 1.1 Add failing tests first

- [ ] Edit `xtask/tests/docs_hygiene.rs` and add a test that proves a missing latest-plan reference is rejected in `README.md`:
  ```rust
  #[test]
  fn docs_hygiene_requires_readme_to_reference_latest_production_master_plan() {
      let repo = TempRepo::new("readme-latest-production-plan");
      repo.write(
          "README.md",
          "# Test\n\n- `plans/legion-production-master-plan-v0.1.md` - current production master plan.\n",
      );

      let result = run_docs_hygiene(&repo.root, &DocsHygieneConfig::default());
      let violations = result.expect_err("expected stale production plan reference violation");

      assert!(violations.iter().any(|violation| {
          violation.path == Path::new("README.md")
              && violation.message.contains("legion-production-master-plan-v0.2.md")
      }));
  }
  ```

- [ ] Add a test that proves `docs/INDEX.md` is also guarded:
  ```rust
  #[test]
  fn docs_hygiene_requires_docs_index_to_reference_latest_production_master_plan() {
      let repo = TempRepo::new("index-latest-production-plan");
      repo.write(
          "README.md",
          "# Test\n\n- `plans/legion-production-master-plan-v0.2.md` - current production master plan.\n",
      );
      repo.write(
          "docs/INDEX.md",
          "# Index\n\n- `../plans/legion-production-master-plan-v0.1.md` - historical production master plan.\n",
      );

      let result = run_docs_hygiene(&repo.root, &DocsHygieneConfig::default());
      let violations = result.expect_err("expected docs index latest-plan violation");

      assert!(violations.iter().any(|violation| {
          violation.path == Path::new("docs/INDEX.md")
              && violation.message.contains("legion-production-master-plan-v0.2.md")
      }));
  }
  ```

- [ ] Add a passing test for the intended current shape:
  ```rust
  #[test]
  fn docs_hygiene_accepts_current_production_master_plan_entrypoints() {
      let repo = TempRepo::new("current-production-plan-entrypoints");
      repo.write(
          "README.md",
          "# Test\n\n- `plans/legion-production-master-plan-v0.2.md` - current production master plan.\n- `plans/legion-production-master-plan-v0.1.md` - historical production master plan.\n",
      );
      repo.write(
          "docs/INDEX.md",
          "# Index\n\n- `../plans/legion-production-master-plan-v0.2.md` - current production master plan.\n- `../plans/legion-production-master-plan-v0.1.md` - historical production master plan.\n",
      );

      run_docs_hygiene(&repo.root, &DocsHygieneConfig::default())
          .expect("current production plan entrypoints should pass");
  }
  ```

- [ ] Run the new tests and confirm they fail for the expected reason:
  ```powershell
  cargo test -p xtask --test docs_hygiene docs_hygiene_requires_readme_to_reference_latest_production_master_plan -- --exact
  cargo test -p xtask --test docs_hygiene docs_hygiene_requires_docs_index_to_reference_latest_production_master_plan -- --exact
  cargo test -p xtask --test docs_hygiene docs_hygiene_accepts_current_production_master_plan_entrypoints -- --exact
  ```
  - Expected result before implementation: the rejection tests fail because no `StaleProductionPlanReference` logic exists. Keep the failure output short in the evidence file.

### 1.2 Implement the docs-hygiene rule

- [ ] Edit `xtask/src/docs_hygiene.rs`.

- [ ] Add a new violation kind:
  ```rust
  StaleProductionPlanReference,
  ```

- [ ] Add constants near the top of the file:
  ```rust
  const LATEST_PRODUCTION_MASTER_PLAN: &str = "legion-production-master-plan-v0.2.md";
  const PRODUCTION_PLAN_ENTRYPOINTS: [&str; 2] = ["README.md", "docs/INDEX.md"];
  ```

- [ ] In `run_docs_hygiene`, call a new check after the stale rename-marker reference check:
  ```rust
  check_current_production_plan_reference(workspace_root, &path, &text, &mut violations);
  ```

- [ ] Add helper logic with these semantics:
  - Convert the scanned path to repo-relative forward-slash form using the existing `repo_relative_path` helper.
  - Return immediately unless the relative path is `README.md` or `docs/INDEX.md`.
  - Return success if the text contains `legion-production-master-plan-v0.2.md`.
  - Otherwise push `DocsHygieneViolation` with:
    - `path`: repo-relative path
    - `line`: `1`
    - `kind`: `DocsHygieneViolationKind::StaleProductionPlanReference`
    - `message`: `current documentation entrypoint must reference latest production master plan `legion-production-master-plan-v0.2.md``

- [ ] Run formatter and targeted tests:
  ```powershell
  cargo fmt --all --check
  cargo test -p xtask --test docs_hygiene
  ```
  - Expected result: both pass after implementation.

---

## Phase 2 - Mark v0.1 Historical Without Rewriting It

- [ ] Edit `plans/legion-production-master-plan-v0.1.md`.

- [ ] Add this banner directly below the H1:
  ```markdown
  > Historical status: this v0.1 plan was superseded by `plans/legion-production-master-plan-v0.2.md` on 2026-06-19. It is retained for audit traceability. Do not treat its current-state assessment as authoritative without checking v0.2 and `plans/product-readiness-ledger.md`.
  ```

- [ ] Do not modify the v0.1 body tables, web research, old milestone language, or original current-state claims. The banner is enough to stop stale repetition while preserving audit history.

- [ ] Confirm the banner is discoverable:
  ```powershell
  rg -n "Historical status|superseded by.*v0\\.2" plans/legion-production-master-plan-v0.1.md
  ```

---

## Phase 3 - Reconcile the Product-Readiness Ledger

### 3.1 Add the standing rule

- [ ] Edit `plans/product-readiness-ledger.md`.

- [ ] Add this bullet under `## Gate Rules`:
  ```markdown
  - Milestone acceptance evidence can close a plan milestone or workstream queue while product-readiness remains open; a readiness row changes status only when that row names current evidence, passing targeted tests, and a working UX path.
  ```

### 3.2 Add the evidence-to-gate reconciliation table

- [ ] Add a new section after `## Status Vocabulary` and before `## Readiness Matrix`:
  ```markdown
  ## Production Evidence Reconciliation

  This table maps accepted production evidence to the product-readiness gates it informs. It is intentionally conservative: evidence can strengthen a gate without promoting the gate to product-ready.

  | Evidence | What it proves | Readiness gates informed | Remaining product-readiness gap |
  | --- | --- | --- | --- |
  | M0 formal acceptance (`plans/evidence/production/M0/M0-milestone-acceptance.md`) | ADR-0032..ADR-0040 are ratified; release-pipeline and perf-harness skeletons exist; baseline gates passed for the then-current tree. | PR-UI-001, PR-UI-002, PR-LANG-001, PR-LANG-002, PR-AI-002, PR-VSC-002, PR-REL-001 | Skeletons and ADRs are not user workflow proof. Product gates still require renderer-backed UX, real language/debug flows, extension-host cut lines, and installability evidence. |
  | M1 formal acceptance (`plans/evidence/production/M1/M1-milestone-acceptance.md`) | Editor canvas, tree-sitter, LSP lifecycle, terminal, search, and SCM-oriented surfaces had current-tree substrate evidence. | PR-UI-001, PR-UI-002, PR-LANG-001, PR-LANG-002 | The ledger still needs cross-platform user workflow evidence for daily editing, full LSP UX, terminal cleanup, debug/test explorer, accessibility, and large workspace behavior. |
  | M2 formal acceptance (`plans/evidence/production/M2/M2-milestone-acceptance.md`) | Apply/rollback proposal workflows, provider routing, local/provider policy, MCP coverage, semantic fabric, and assist surfaces had current-tree evidence. | PR-AI-001, PR-AI-002 | Proposal safety and inspectable AI are stronger, but adversarial evals, broader provider smoke, cancellation/cost evidence, and product assist/delegate workflows remain bounded by their row evidence. |
  | M3 workstream evidence package (`plans/evidence/production/M3/`) | MCP client GA, workflow review/replay, privacy inspector productization, AI second-opinion review, and benchmark posture have task-level evidence. | PR-AI-001, PR-AI-002, PR-ENT-002 | No formal `M3-milestone-acceptance.md` is present in the current tree. Treat M3 as workstream evidence unless a future audit adds formal milestone acceptance. |
  | M4 formal acceptance (`plans/evidence/production/M4/M4-milestone-acceptance.md`) | Workflow runtime, fleet console, approval/risk surfaces, and supporting trust/proposal substrate were accepted for the current tree. | PR-AI-002, PR-ENT-002 | Command-center substrate does not prove collaboration/admin product readiness or unrestricted automation. Product rows remain limited to their explicit evidence. |
  | M5 formal acceptance (`plans/evidence/production/M5/M5-milestone-acceptance.md`) | Release/signing follow-ons, accessibility/platform parity evidence, terminal hardening, extension launch posture, and docs/support surfaces were accepted or explicitly deferred. | PR-UI-001, PR-LANG-002, PR-VSC-001, PR-REL-001 | Hardware-limited multi-window/DPI coverage, signed installers, auto-update/rollback, crash-report controls, and fresh-machine evidence remain open where the matrix says so. |
  | M6 formal acceptance (`plans/evidence/production/M6/M6-milestone-acceptance.md`) | M6 predecessor queue and full phase gates passed after resolving security, license, and session-schema issues. | PR-AI-001, PR-ENT-002, PR-REL-001 | M6 strengthens governance evidence, but remote/collaboration/admin, signed release, crash controls, telemetry flywheel, and external benchmark claims remain constrained by explicit row statuses. |
  ```

- [ ] Scan the readiness matrix after adding the table. Do not promote any `Current Status` cell except to clarify conservative wording such as `Substrate validated` or `Deferred with explicit cut line`.

- [ ] Run:
  ```powershell
  cargo run -p xtask -- docs-hygiene
  ```
  - Expected result: pass. The new relative links must be valid.

---

## Phase 4 - Add v0.2 Release-Note Appendix and Clean Stale Plan Reference

### 4.1 Add the v0.2 appendix

- [ ] Edit `plans/legion-production-master-plan-v0.2.md`.

- [ ] Append this section at the end of the file:
  ```markdown
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
  ```

- [ ] Add an internal link from the WS-P0 section to the appendix if the file already has an appendix list. If there is no appendix list, leave the appended section discoverable by heading.

### 4.2 Clean stale plan reference in perf harness comment

- [ ] Edit `xtask/src/perf_harness.rs`.

- [ ] Replace the comment reference:
  ```rust
  //! `plans/legion-production-master-plan-v0.1.md` §11
  ```
  with:
  ```rust
  //! `plans/legion-production-master-plan-v0.2.md` quality bars
  ```

- [ ] Keep the rest of the comment intact unless the sentence no longer parses after the reference update.

- [ ] Run:
  ```powershell
  cargo fmt --all --check
  cargo check -p xtask --all-targets
  ```

---

## Phase 5 - Audit Dirty-Worktree Caveats

- [ ] Create `plans/evidence/production/WS-P0/dirty-worktree-caveat-audit.md`.

- [ ] Use this exact structure:
  ```markdown
  # WS-P0 Dirty-Worktree Caveat Audit

  Date: 2026-06-19
  Scope: production evidence files with dirty-worktree caveats or related current-tree caveats.

  ## Decision

  No historical milestone should be retroactively upgraded from dirty-tree acceptance to clean-commit acceptance by WS-P0. WS-P0 should instead make the caveats explicit and require current gates to pass from the present branch before any new product-readiness status changes.

  ## Caveat Matrix

  | Evidence | Caveat found | Clean rerun decision |
  | --- | --- | --- |
  | `plans/evidence/production/M0/M0-milestone-acceptance.md` | Acceptance verified a current tree with uncommitted M0 evidence, implementation files, and some later-workstream files. | No clean rerun required for historical M0. Current WS-P0 gates must pass before merging WS-P0. |
  | `plans/evidence/production/M1/M1-milestone-acceptance.md` | Current workspace remained intentionally dirty with integrated predecessor workstream outputs. | No clean rerun required for historical M1. Do not use M1 alone to promote product-ready status. |
  | `plans/evidence/production/M2/M2-milestone-acceptance.md` | Current workspace remained intentionally dirty with integrated predecessor workstream outputs. | No clean rerun required for historical M2. Product-readiness rows still need direct row evidence. |
  | `plans/evidence/production/M3/` | No formal `M3-milestone-acceptance.md` was present during WS-P0 review; M3 contains task-level evidence files. | Do not claim formal M3 milestone acceptance unless a separate audit creates and verifies it. |
  | `plans/evidence/production/M4/M4-milestone-acceptance.md` | Workspace remained intentionally dirty because it contained integrated predecessor workstream outputs. | No clean rerun required for historical M4. Use M4 as substrate/workflow evidence only. |
  | `plans/evidence/production/M5/M5-milestone-acceptance.md` | Workspace contained unrelated pre-existing dirty changes from other in-flight work. | No clean rerun required for historical M5. Keep WS18.T4 and release-signing cut lines explicit. |
  | `plans/evidence/production/M6/M6-milestone-acceptance.md` | Workspace contained unrelated pre-existing dirty changes from other in-flight work. | No clean rerun required for historical M6. Current WS-P0 gates provide the cleanest present-tense validation available in this packet. |

  ## Current WS-P0 Required Checks

  WS-P0 completion requires these current commands to pass or be recorded with exact blockers:

  - `cargo run -p xtask -- docs-hygiene`
  - `cargo run -p xtask -- check-deps`
  - `cargo fmt --all --check`
  - `cargo check --workspace --all-targets`
  - `cargo test -p xtask --test docs_hygiene`
  - `git diff --check`
  ```

- [ ] Confirm the new evidence file has no broken relative links:
  ```powershell
  cargo run -p xtask -- docs-hygiene
  ```

---

## Phase 6 - Create Weekly Legion-on-Legion Dogfood Journal Template

- [ ] Create `plans/dogfood/legion-on-legion-weekly-journal-template.md`.

- [ ] Use this content:
  ```markdown
  # Legion-on-Legion Weekly Dogfood Journal Template

  ## Week

  - Week starting:
  - Legion branch / commit:
  - Host OS and display setup:
  - Primary workspace opened:
  - Daily-driver hours attempted:

  ## Workflows Exercised

  | Workflow | Attempted | Evidence path | Result | Notes |
  | --- | --- | --- | --- | --- |
  | Manual edit/open/save/search | No |  | Not run |  |
  | Rust LSP completion/diagnostics/rename | No |  | Not run |  |
  | Terminal task | No |  | Not run |  |
  | Git diff/stage/commit/review | No |  | Not run |  |
  | Assist context manifest and proposal | No |  | Not run |  |
  | Delegate worktree/sandbox task | No |  | Not run |  |
  | Automate/fleet command center | No |  | Not run |  |
  | Packaging/update/crash/support path | No |  | Not run |  |

  ## Friction Log

  | Time | Surface | Symptom | Repro steps | Severity | Follow-up issue or plan link |
  | --- | --- | --- | --- | --- | --- |

  ## Trust and Policy Observations

  - Egress posture observed:
  - Proposal review clarity:
  - Privacy inspector clarity:
  - Audit/evidence export clarity:
  - Capability prompts or denials:

  ## Product-Readiness Impact

  | Readiness gate | Evidence added | Status impact | Reason |
  | --- | --- | --- | --- |

  ## End-of-Week Decision

  - Continue dogfooding next week:
  - Required fixes before next dogfood session:
  - Evidence files added:
  - Commands run:
  ```

- [ ] Add a short reference to the template in `plans/product-readiness-ledger.md` after the beta acceptance scenario:
  ```markdown
  ## Dogfood Evidence

  Weekly Legion-on-Legion dogfood runs should use `plans/dogfood/legion-on-legion-weekly-journal-template.md`. A journal entry is evidence only when it names the branch, commit, OS, workflow attempted, evidence path, result, and product-readiness impact.
  ```

- [ ] Run:
  ```powershell
  cargo run -p xtask -- docs-hygiene
  ```

---

## Phase 7 - Record WS-P0 Evidence

- [ ] Create `plans/evidence/production/WS-P0/WS-P0-rebaseline-evidence.md`.

- [ ] Use this structure and fill it with actual command output summaries from the implementation run:
  ```markdown
  # WS-P0 Rebaseline Evidence

  Date: 2026-06-19
  Scope: WS-P0 rebaseline, ledgers, plan hygiene, docs-hygiene latest-plan guard, dirty-worktree caveat audit, and dogfood journal template.

  ## Branch State

  - Branch:
  - Starting dirty files:
  - Ending dirty files:

  ## Completed Tasks

  | Task | Evidence |
  | --- | --- |
  | P0.01 | `plans/legion-production-master-plan-v0.2.md` exists and is the current plan. |
  | P0.02 | `README.md` and `docs/INDEX.md` point production readers at v0.2. |
  | P0.03 | `plans/legion-production-master-plan-v0.1.md` has a historical-status banner. |
  | P0.04 | `plans/product-readiness-ledger.md` reconciles M0-M6 evidence without inflating statuses. |
  | P0.05 | `plans/product-readiness-ledger.md` includes the evidence-to-product-gates table. |
  | P0.06 | `plans/product-readiness-ledger.md` includes the standing milestone-vs-product-readiness rule. |
  | P0.07 | `xtask` docs-hygiene has tests and implementation for current production plan entrypoints. |
  | P0.08 | `plans/legion-production-master-plan-v0.2.md` includes Appendix D. |
  | P0.09 | `plans/evidence/production/WS-P0/dirty-worktree-caveat-audit.md` records caveats and rerun decisions. |
  | P0.10 | `plans/dogfood/legion-on-legion-weekly-journal-template.md` exists. |

  ## Verification

  | Command | Result | Notes |
  | --- | --- | --- |
  | `cargo test -p xtask --test docs_hygiene` |  |  |
  | `cargo run -p xtask -- docs-hygiene` |  |  |
  | `cargo run -p xtask -- check-deps` |  |  |
  | `cargo fmt --all --check` |  |  |
  | `cargo check --workspace --all-targets` |  |  |
  | `git diff --check` |  |  |

  ## Residual Risk

  - M3 remains task-level evidence unless a separate audit adds formal milestone acceptance.
  - Historical dirty-tree caveats remain historical caveats; WS-P0 does not rewrite them.
  - Product-readiness rows remain bounded by their current row evidence.
  ```

- [ ] After all verification commands run, fill each `Result` and `Notes` cell with concrete results. Do not leave empty cells in the final committed evidence file.

---

## Phase 8 - Full Verification

- [ ] Run the targeted xtask regression suite:
  ```powershell
  cargo test -p xtask --test docs_hygiene
  ```
  - Required result: pass.

- [ ] Run the docs gate:
  ```powershell
  cargo run -p xtask -- docs-hygiene
  ```
  - Required result: pass.

- [ ] Run dependency policy:
  ```powershell
  cargo run -p xtask -- check-deps
  ```
  - Required result: pass.

- [ ] Run formatting:
  ```powershell
  cargo fmt --all --check
  ```
  - Required result: pass.

- [ ] Run Rust check:
  ```powershell
  cargo check --workspace --all-targets
  ```
  - Required result: pass.

- [ ] Run whitespace verification:
  ```powershell
  git diff --check
  ```
  - Required result: pass.

- [ ] If time allows and the branch is otherwise clean enough for the full gate, run:
  ```powershell
  cargo test --workspace --all-targets --no-fail-fast
  cargo clippy --workspace --all-targets -- -D warnings
  ```
  - These are not listed in WS-P0 acceptance, but they are repository-standard confidence checks after editing Rust code.

---

## Phase 9 - Commit Strategy

- [ ] Review the final diff:
  ```powershell
  git diff -- README.md docs/INDEX.md plans/legion-production-master-plan-v0.1.md plans/legion-production-master-plan-v0.2.md plans/product-readiness-ledger.md plans/evidence/production/WS-P0 plans/dogfood xtask/src/docs_hygiene.rs xtask/tests/docs_hygiene.rs xtask/src/perf_harness.rs
  ```

- [ ] If unrelated dirty files are present, stage only WS-P0 files:
  ```powershell
  git add README.md docs/INDEX.md plans/legion-production-master-plan-v0.1.md plans/legion-production-master-plan-v0.2.md plans/product-readiness-ledger.md plans/evidence/production/WS-P0 plans/dogfood/legion-on-legion-weekly-journal-template.md xtask/src/docs_hygiene.rs xtask/tests/docs_hygiene.rs xtask/src/perf_harness.rs
  ```

- [ ] Commit with:
  ```powershell
  git commit -m "docs: complete WS-P0 production rebaseline"
  ```

- [ ] Final implementation summary must include:
  - Files changed.
  - Which WS-P0 tasks were completed.
  - Exact verification commands and pass/fail results.
  - Any historical caveats intentionally preserved.
  - Any tests or full gates not run, with reason.

---

## Completion Checklist

- [ ] P0.01 is satisfied: v0.2 exists and is identified as current.
- [ ] P0.02 is satisfied: README and docs index point production readers at v0.2.
- [ ] P0.03 is satisfied: v0.1 has a historical/current-state correction note.
- [ ] P0.04 is satisfied: product-readiness ledger reconciles accepted evidence conservatively.
- [ ] P0.05 is satisfied: M0-M6 evidence-to-product-gate table exists.
- [ ] P0.06 is satisfied: milestone acceptance versus product-readiness standing rule exists.
- [ ] P0.07 is satisfied: docs-hygiene guards current production master-plan entrypoints.
- [ ] P0.08 is satisfied: v0.2 has a "what changed since v0.1" appendix.
- [ ] P0.09 is satisfied: dirty-worktree caveats and rerun decisions are recorded.
- [ ] P0.10 is satisfied: weekly Legion-on-Legion dogfood journal template exists.
- [ ] `cargo test -p xtask --test docs_hygiene` passes.
- [ ] `cargo run -p xtask -- docs-hygiene` passes.
- [ ] `cargo run -p xtask -- check-deps` passes.
- [ ] `cargo fmt --all --check` passes.
- [ ] `cargo check --workspace --all-targets` passes.
- [ ] `git diff --check` passes.
