# Legion IDE Production-Shippable Program Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Take Legion IDE from its current state (M7 complete, M8 partially evidenced, truth drift at HEAD) to a production-shippable beta and GA per `plans/legion-production-master-plan-v0.2.md`, starting from the findings in `HERMESGOAL-GAP-ANALYSIS.md`.

**Architecture:** Phase 0 repairs repo truth and gate health at full task granularity (it is completely known from the gap analysis). Phases 1–6 are milestone-scoped work packets (M8→M13); each packet is a self-contained spec that gets its own detailed implementation plan (via superpowers:writing-plans) when its milestone becomes active — writing code-level detail for milestones months out would fabricate precision the codebase does not yet support.

**Tech Stack:** Rust workspace (30 crates + xtask), egui/eframe renderer, tree-sitter, LSP 3.17, ConPTY/Unix PTY, clap-based xtask gates, TOML Kanban backlog, Python evals harness.

## Global Constraints

Copied from `HERMESGOAL.md` §2 (non-negotiable invariants). Every task in every phase implicitly includes these:

- `legion-ui` is projection-only; `legion-desktop` is renderer edge; `legion-app` owns composition/authority; `legion-project` owns filesystem mutation.
- Direct provider/worker/plugin/UI file mutation is forbidden. All AI write intent becomes a proposal; durable mutation requires app/workspace authority.
- Saves/applies preserve fingerprints, versions, generations, snapshot IDs, non-zero correlation IDs, non-nil causality IDs. Non-atomic write fallback stays fail-closed.
- Default retention is metadata-only. Manual mode has zero hosted egress and no AI/network/cloud/worker surfaces. No secrets in commits, logs, or evidence.
- No readiness row promotes without: named golden path, current evidence, UX path, platform scope, failure-mode behavior, security review, docs + ledger update (`HERMESGOAL.md` §10).
- Canonical mode names: Manual, Assist, Delegate, Legion Workflows (docs-hygiene rejects legacy labels).
- Conventional Commits. Full standing gate set (README "Required Local Gates" + xtask gates) before claiming a broad packet complete; targeted gates per task.
- Stop conditions in `HERMESGOAL.md` §11 apply: record a blocker instead of improvising past a policy, secret, ADR, or gate failure.

---

# Phase 0 — Truth Repair and Gate Restoration (active now)

Everything in this phase comes from `HERMESGOAL-GAP-ANALYSIS.md` §4–§6. No new product surface may land until this phase is complete (repo doctrine: truth before feature work). Branch: create `fix/phase-0-truth-repair` from `main`.

### Task 1: Remove foreign-project contamination

**Files:**
- Delete: `audit-reports/quaternius-megakit-spike-subset.md`
- Delete: `audit-reports/quaternius-calibration-adapter-proposal.md`

These are Godot game-asset audits for a different project ("Off The Rails", macOS user paths) accidentally committed in `236a492`.

- [ ] **Step 1: Verify the files are foreign** — open both; confirm they reference `Modular SciFi MegaKit`, Godot, and `/Users/christopherwilloughby/...` paths, and contain no Legion content.
- [ ] **Step 2: Delete**

```bash
git rm audit-reports/quaternius-megakit-spike-subset.md audit-reports/quaternius-calibration-adapter-proposal.md
```

- [ ] **Step 3: Verify no references remain**

Run: `grep -ri quaternius --include="*.md" --include="*.toml" .`
Expected: no matches outside `.git/`.

- [ ] **Step 4: Run docs-hygiene**

Run: `cargo run -p xtask -- docs-hygiene`
Expected: `documentation hygiene checks passed`

- [ ] **Step 5: Commit**

```bash
git commit -m "fix: remove foreign-project audit files committed by mistake"
```

### Task 2: Fix the README CI claim

**Files:**
- Modify: `README.md:73`

`.github/workflows/legion-bench.yml` now exists, so "No GitHub Actions CI workflow is currently configured" is false.

- [ ] **Step 1: Read `.github/workflows/legion-bench.yml`** and confirm what it runs (legion-bench recorded mode) so the replacement sentence is accurate.
- [ ] **Step 2: Replace README.md line 73** with:

```markdown
A single GitHub Actions workflow (`.github/workflows/legion-bench.yml`) runs the recorded legion-bench eval fixtures. It is not a full CI gate: local developer machines must install the CLI before using `scripts/run-phase-gates.*`, and those local gates remain the active verification source for all other checks.
```

- [ ] **Step 3: Sweep for the same stale claim elsewhere** (per global "comprehensive sweeps" rule)

Run: `grep -rn "No GitHub Actions" --include="*.md" .`
Expected: only `HERMESGOAL.md:376` remains (a conditional statement — "unless one exists in the repository" — which is now satisfied and needs no edit; leave it).

- [ ] **Step 4: Run docs-hygiene** — `cargo run -p xtask -- docs-hygiene`; expected pass.
- [ ] **Step 5: Commit** — `git commit -am "docs: correct README CI claim after legion-bench workflow was added"`

### Task 3: Fix the ledger dogfood template path

**Files:**
- Modify: `plans/product-readiness-ledger.md:65`

- [ ] **Step 1: Edit line 65** — change `dogfood/legion-on-legion-weekly-journal-template.md` to `plans/dogfood/legion-on-legion-weekly-journal-template.md`.
- [ ] **Step 2: Verify the target exists** — `ls plans/dogfood/legion-on-legion-weekly-journal-template.md`; expected: file listed.
- [ ] **Step 3: Run docs-hygiene**; expected pass.
- [ ] **Step 4: Commit** — `git commit -am "docs: fix dogfood journal template path in readiness ledger"`

### Task 4: Qualify `docs/releases/v8.0.0/` as a forward template

**Files:**
- Modify: `docs/releases/v8.0.0/release-checklist.md`
- Modify: `docs/releases/v8.0.0/migration-policy.md`
- Modify: `docs/releases/v8.0.0/rollback-policy.md`
- Modify: any sibling files in the same directory (list with `ls docs/releases/v8.0.0/`)

The checklist names a v8.0.0 GA cut and cites `.github/workflows/ci.yml:166`, which does not exist. Left unqualified this violates the "do not claim GA" invariant.

- [ ] **Step 1: Add this banner as the first body line of every file in the directory** (below the H1):

```markdown
> **STATUS: FORWARD-LOOKING TEMPLATE — NOT A CURRENT RELEASE ARTIFACT.** No v8.0.0 release exists or is scheduled. The workspace version is 0.1.0 and the product is pre-beta; see `plans/product-readiness-ledger.md` for current status. CI references in this document (e.g. `.github/workflows/ci.yml`) describe a future pipeline that is not yet configured.
```

- [ ] **Step 2: Verify no active doc links to these files as current** — `grep -rn "v8.0.0" README.md docs/INDEX.md docs/USER_GUIDE.md`; expected: no matches (if any exist, mark them "(forward template)").
- [ ] **Step 3: Run docs-hygiene**; expected pass.
- [ ] **Step 4: Commit** — `git commit -am "docs: mark v8.0.0 release docs as forward-looking templates"`

### Task 5: Clean clippy warnings

**Files:**
- Modify: files reported by clippy in `legion-app`, `legion-agent`, `legion-desktop` (unused imports such as `LspDiagnosticProjectionContext`, `BufferId`, `DelegatedTaskScopeTargetKind`, `CanonicalProductMode`).

- [ ] **Step 1: Capture the authoritative list**

Run: `cargo clippy --workspace --all-targets -- -D warnings 2>&1 | grep -A2 "unused"`

- [ ] **Step 2: Remove each unused import named in the output.** Do not `#[allow]` them — if an import was added for a planned surface, deleting it is correct; the surface's own task will re-add it.
- [ ] **Step 3: Verify** — `cargo clippy --workspace --all-targets -- -D warnings`; expected: exit 0, no warnings.
- [ ] **Step 4: Commit** — `git commit -am "refactor: remove unused imports blocking clippy gate"`

### Task 6: Capture a clean full-gate baseline and fix real failures

**Files:**
- Create: `plans/evidence/production/WS-P0/phase-0-gate-baseline.md`
- Modify: whatever the failing tests require (known suspect list below)

Three prior full-suite attempts hit Windows build-artifact corruption (concurrent IDE cargo runs / antivirus scanning of `target/`). A clean baseline is the exit gate for Phase 0.

- [ ] **Step 1: Eliminate interference.** Close RustRover (or pause its cargo integration) and add the repo `target/` directory to Windows Defender exclusions for the duration of the run. Record both actions in the evidence file.
- [ ] **Step 2: Run the standing gates in order, capturing output** (working directory: repo root):

```bash
cargo run -p xtask -- check-deps
cargo run -p xtask -- docs-hygiene
cargo run -p xtask -- no-egui-textedit
cargo run -p xtask -- verify-kanban-backlog
cargo fmt --all --check
cargo check --workspace --all-targets
cargo test --workspace --all-targets --no-fail-fast
cargo clippy --workspace --all-targets -- -D warnings
cargo deny check   # if cargo-deny is not installed, record that explicitly per HERMESGOAL §4
python3 -m pytest evals training -q
```

- [ ] **Step 3: Triage any test failures.** Known suspects from the WS-P0 refresh (2026-07-01):
  - `legion-desktop --test input_conformance` — fails because runtime workspace trust policy denies temp-directory paths. Fix by mirroring the trust-granting pattern used by the passing desktop workflow tests (e.g. how `daily_editing_controls` or `beta_workflow` construct a trusted test workspace); do not weaken the trust policy itself. If the passing tests use a helper to mark the workspace trusted, reuse that helper.
  - TDD tests in `legion-ai`, `legion-plugin`, `legion-storage` referencing unimplemented types — commit `236a492` claims targeted runs of these crates passed, so these may already be fixed; if any still fail, implement the minimal type/function the test names (the failing test is the spec), or move the test behind `#[ignore = "tracked: <backlog task id>"]` with a matching Kanban task if the surface is genuinely future work.
  - `E0432 scope_picker::ScopeRiskTolerance` from earlier corrupted runs is NOT real — the types exist at `crates/legion-desktop/src/view/scope_picker.rs:7-9`; treat any recurrence as environment noise and re-run.
- [ ] **Step 4: Re-run the full set until green**, then write `plans/evidence/production/WS-P0/phase-0-gate-baseline.md` containing: each command, working directory, commit SHA, start/end time, exit code, and pass/fail counts (per `HERMESGOAL.md` §9 evidence requirements).
- [ ] **Step 5: Commit** — `git add -A && git commit -m "test: restore full standing gate set to green with evidence baseline"`

### Task 7: Delegated-task sandbox reaping

**Files:**
- Modify: `crates/legion-agent/src/lib.rs` (near `DelegatedTaskSandboxOrchestrator`, line ~226)
- Modify: `crates/legion-app/src/offline_ai.rs` (orchestrator creation, line ~212)
- Test: `crates/legion-agent/tests/sandbox_reaping.rs` (create)

**Interfaces:**
- Consumes: `DelegatedTaskSandboxOrchestrator` (existing), its `cleanup()` method (line ~301).
- Produces: `pub fn reap_orphaned_sandboxes(delegated_tasks_root: &Path, active_run_ids: &[String]) -> std::io::Result<Vec<PathBuf>>` in `legion-agent`, returning the removed paths.

Background: lanes create sandboxes under `target/delegated-tasks/task-{run_id}` (git worktree with copy fallback). `cleanup()` exists but is never invoked for crashed/abandoned lanes, so orphans accumulate (two were found live; they broke workspace pytest collection).

- [ ] **Step 1: Write the failing test** at `crates/legion-agent/tests/sandbox_reaping.rs`:

```rust
use legion_agent::reap_orphaned_sandboxes;
use std::fs;
use std::path::PathBuf;

fn temp_root(tag: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!("legion-reap-{tag}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).expect("create temp root");
    root
}

#[test]
fn reap_removes_orphans_and_preserves_active_and_unrelated() {
    let root = temp_root("basic");
    fs::create_dir_all(root.join("task-orphan-1")).unwrap();
    fs::write(root.join("task-orphan-1/marker.txt"), "stale").unwrap();
    fs::create_dir_all(root.join("task-active-1")).unwrap();
    fs::create_dir_all(root.join("not-a-task-dir")).unwrap();

    let removed =
        reap_orphaned_sandboxes(&root, &["active-1".to_string()]).expect("reap succeeds");

    assert_eq!(removed.len(), 1);
    assert!(removed[0].ends_with("task-orphan-1"));
    assert!(!root.join("task-orphan-1").exists(), "orphan removed");
    assert!(root.join("task-active-1").exists(), "active lane preserved");
    assert!(root.join("not-a-task-dir").exists(), "non-task dirs untouched");

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn reap_on_missing_root_is_a_noop() {
    let root = temp_root("missing").join("does-not-exist");
    let removed = reap_orphaned_sandboxes(&root, &[]).expect("noop on missing root");
    assert!(removed.is_empty());
}
```

- [ ] **Step 2: Run to verify failure** — `cargo test -p legion-agent --test sandbox_reaping`; expected: compile error, `reap_orphaned_sandboxes` not found.
- [ ] **Step 3: Implement in `crates/legion-agent/src/lib.rs`** (next to the orchestrator):

```rust
/// Removes orphaned sandbox directories under `delegated_tasks_root`.
///
/// A directory is an orphan when its name starts with `task-` and its
/// run-id suffix is not in `active_run_ids`. Attempts `git worktree
/// remove --force` first (mirroring `initialize`'s worktree-first
/// strategy) and falls back to plain directory removal. Returns the
/// paths that were removed. A missing root is a successful no-op.
pub fn reap_orphaned_sandboxes(
    delegated_tasks_root: &Path,
    active_run_ids: &[String],
) -> Result<Vec<PathBuf>, std::io::Error> {
    let mut removed = Vec::new();
    if !delegated_tasks_root.exists() {
        return Ok(removed);
    }
    for entry in std::fs::read_dir(delegated_tasks_root)? {
        let entry = entry?;
        if !entry.file_type()?.is_dir() {
            continue;
        }
        let name = entry.file_name();
        let Some(name) = name.to_str() else { continue };
        let Some(run_id) = name.strip_prefix("task-") else {
            continue;
        };
        if active_run_ids.iter().any(|active| active == run_id) {
            continue;
        }
        let path = entry.path();
        let worktree_removed = Command::new("git")
            .arg("worktree")
            .arg("remove")
            .arg("--force")
            .arg(&path)
            .output()
            .map(|output| output.status.success())
            .unwrap_or(false);
        if !worktree_removed {
            std::fs::remove_dir_all(&path)?;
        }
        removed.push(path);
    }
    Ok(removed)
}
```

- [ ] **Step 4: Run the test** — `cargo test -p legion-agent --test sandbox_reaping`; expected: 2 passed.
- [ ] **Step 5: Wire the reap at startup.** In `crates/legion-app/src/offline_ai.rs`, at the point where the sandbox root `target/delegated-tasks` is first known (orchestrator construction, ~line 212), call `legion_agent::reap_orphaned_sandboxes(Path::new("target/delegated-tasks"), &[])` before the first lane starts, logging removed paths at `tracing::info!`. There are no live lanes before app startup, so the empty active list is correct there. Do NOT call it anywhere a lane may already be running.
- [ ] **Step 6: Verify affected crates** — `cargo test -p legion-agent -p legion-app`; expected: all pass. Also `cargo clippy -p legion-agent -p legion-app --all-targets -- -D warnings`.
- [ ] **Step 7: Commit** — `git add -A && git commit -m "fix: reap orphaned delegated-task sandboxes at startup"`

### Task 8: `claim-audit` xtask gate

**Files:**
- Create: `xtask/src/claim_audit.rs`
- Modify: `xtask/src/lib.rs` (add `pub mod claim_audit;`)
- Modify: `xtask/src/main.rs` (add `ClaimAudit` variant to `Commands` enum ~line 463 and dispatch arm ~line 595, mirroring `DocsHygiene`)
- Test: `xtask/tests/claim_audit.rs` (create)

**Interfaces:**
- Produces: `cargo run -p xtask -- claim-audit` — exits non-zero if any current public doc makes a product claim the readiness ledger does not support.
- Consumes: `plans/product-readiness-ledger.md` readiness matrix (rows `PR-*` with a `Current Status` column).

v1 scope (deliberately narrow — this closes the missing M7 item, it is not a full NLP audit):
1. Parse ledger matrix rows into `(gate_id, status)` pairs; fail if the table is missing or unparseable.
2. Scan `README.md`, `HERMESGOAL.md`, and `docs/*.md` for forbidden claim phrases — `"production-ready"`, `"production ready"`, `"generally available"`, `"GA-ready"` — outside lines that negate them (line also contains `not`, `until`, `require`, or `is not reached`). Any hit fails.
3. Require `README.md` to still contain the caveat sentence `Legion is not yet a general-availability desktop product` while any ledger row is below `Product workflow validated`.

- [ ] **Step 1: Write the failing test** at `xtask/tests/claim_audit.rs`:

```rust
use xtask::claim_audit::{audit_text, ClaimViolation};

#[test]
fn forbidden_claim_is_flagged() {
    let violations = audit_text("README.md", "Legion is production-ready today.");
    assert_eq!(violations.len(), 1);
    assert!(matches!(violations[0], ClaimViolation::ForbiddenPhrase { .. }));
}

#[test]
fn negated_claim_is_allowed() {
    let violations = audit_text(
        "README.md",
        "Legion is not production-ready until GP-1 through GP-6 pass.",
    );
    assert!(violations.is_empty());
}

#[test]
fn ledger_rows_parse() {
    let ledger = "| Track | Gate | Acceptance Criteria | Current Status | Current Evidence |\n\
                  | --- | --- | --- | --- | --- |\n\
                  | AI | PR-AI-001 inspectable AI | criteria | Product workflow validated | tests |";
    let rows = xtask::claim_audit::parse_ledger_rows(ledger).expect("parses");
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].gate_id, "PR-AI-001");
    assert_eq!(rows[0].status, "Product workflow validated");
}
```

- [ ] **Step 2: Run to verify failure** — `cargo test -p xtask --test claim_audit`; expected: compile error, module not found.
- [ ] **Step 3: Implement `xtask/src/claim_audit.rs`:**

```rust
//! Claim-audit gate: fails when current public docs make product claims
//! the product-readiness ledger does not support. Closes the M7/WS-P0
//! "claim-audit script or checklist" requirement (v1 scope).

const FORBIDDEN_PHRASES: [&str; 4] = [
    "production-ready",
    "production ready",
    "generally available",
    "ga-ready",
];
const NEGATION_MARKERS: [&str; 4] = ["not", "until", "require", "is not reached"];

#[derive(Debug)]
pub enum ClaimViolation {
    ForbiddenPhrase {
        file: String,
        line_number: usize,
        phrase: &'static str,
    },
    MissingReadmeCaveat,
}

#[derive(Debug)]
pub struct LedgerRow {
    pub gate_id: String,
    pub status: String,
}

pub fn audit_text(file: &str, text: &str) -> Vec<ClaimViolation> {
    let mut violations = Vec::new();
    for (index, line) in text.lines().enumerate() {
        let lower = line.to_lowercase();
        for phrase in FORBIDDEN_PHRASES {
            if lower.contains(phrase)
                && !NEGATION_MARKERS.iter().any(|marker| lower.contains(marker))
            {
                violations.push(ClaimViolation::ForbiddenPhrase {
                    file: file.to_string(),
                    line_number: index + 1,
                    phrase,
                });
            }
        }
    }
    violations
}

pub fn parse_ledger_rows(ledger: &str) -> Result<Vec<LedgerRow>, String> {
    let mut rows = Vec::new();
    for line in ledger.lines() {
        let cells: Vec<&str> = line.split('|').map(str::trim).collect();
        // | Track | Gate | Criteria | Status | Evidence | -> 7 cells with
        // leading/trailing empties.
        if cells.len() < 6 {
            continue;
        }
        let gate_cell = cells[2];
        let Some(gate_id) = gate_cell.split_whitespace().next() else {
            continue;
        };
        if !gate_id.starts_with("PR-") {
            continue;
        }
        rows.push(LedgerRow {
            gate_id: gate_id.to_string(),
            status: cells[4].to_string(),
        });
    }
    if rows.is_empty() {
        return Err("no PR-* rows found in readiness matrix".to_string());
    }
    Ok(rows)
}

pub fn readme_caveat_present(readme: &str) -> bool {
    readme.contains("Legion is not yet a general-availability desktop product")
}
```

- [ ] **Step 4: Run the unit tests** — `cargo test -p xtask --test claim_audit`; expected: 3 passed.
- [ ] **Step 5: Wire the CLI.** In `xtask/src/main.rs`, mirror `DocsHygiene`: add a `ClaimAudit` variant, and a `run_claim_audit_command()` that reads `plans/product-readiness-ledger.md`, `README.md`, `HERMESGOAL.md`, and the top-level `docs/*.md` canonical set only; calls `parse_ledger_rows`, `audit_text` per file, and `readme_caveat_present` (required while any row status is not `Product workflow validated`); prints each violation with file:line; exits 1 on any violation, else prints `claim audit passed`. Do NOT recurse into `docs/releases/` (forward templates), `docs/superpowers/` (plans quote the forbidden phrases as code literals — including this plan), or `plans/evidence/` (historical), matching how docs-hygiene allowlists archived material.
- [ ] **Step 6: Run against the real repo** — `cargo run -p xtask -- claim-audit`; expected: `claim audit passed`. If it flags real violations, fix the docs (that is the gate doing its job), then re-run.
- [ ] **Step 7: Document the gate.** Add `cargo run -p xtask -- claim-audit` to the gate list in `README.md` ("Required Local Gates") and `docs/OPERATOR_RUNBOOK.md`.
- [ ] **Step 8: Commit** — `git add -A && git commit -m "feat: add claim-audit xtask gate checking docs against readiness ledger"`

### Task 9: Kanban backlog status tracking

**Files:**
- Modify: `plans/kanban/legion-ga-backlog.toml` (add `status` to every task; update `[meta]`)
- Modify: `xtask/src/kanban_backlog.rs` (validate the new field)
- Modify: `xtask/tests/kanban_backlog.rs` (cover the new field)

The backlog has 146 tasks and no completion state, so "select the next unblocked task" cannot be computed. `meta.milestone` still says `M0`.

- [ ] **Step 1: Extend validation first (failing test).** In `xtask/tests/kanban_backlog.rs`, add a test asserting that a task with `status = "shipped"` (invalid value) is rejected and that valid values are exactly `todo`, `in-progress`, `done`, `blocked`, defaulting to `todo` when absent. Run `cargo test -p xtask --test kanban_backlog`; expected: new test fails.
- [ ] **Step 2: Implement** the optional `status` field in `xtask/src/kanban_backlog.rs` with that closed vocabulary and default. Run the test; expected: pass.
- [ ] **Step 3: Mark statuses from evidence, conservatively.** Set `status = "done"` ONLY for tasks whose acceptance is provably met by an existing evidence file or a currently-passing named test: the P0 truth/taxonomy tasks covered by `plans/evidence/production/WS-P0/`, and the P1/P2 tasks covered by `plans/evidence/production/WS-MANUAL-01/` and `WS-MANUAL-02/`. For each task marked done, add `evidence = "<path>"` on the task. Leave everything uncertain as `todo` — an unmarked done task is a smaller lie than a falsely marked one.
- [ ] **Step 4: Update `[meta]`** — set `milestone = "M8"` and add a comment line mapping backlog epics to v0.2 milestones: `# Epic mapping to v0.2 milestones: P0→M7, P1-P2→M8, P3-P4→M9, P5→M10, P6→M11, P7-P8→M12, P9→M13`.
- [ ] **Step 5: Verify** — `cargo run -p xtask -- verify-kanban-backlog` and `cargo test -p xtask --test kanban_backlog`; expected: pass.
- [ ] **Step 6: Commit** — `git add -A && git commit -m "feat: add status tracking to kanban backlog and reconcile milestone mapping"`

### Task 10: Phase 0 closure — evidence, ledger, merge

**Files:**
- Create: `plans/evidence/production/WS-P0/phase-0-truth-repair-closure.md`
- Modify: `plans/product-readiness-ledger.md` (dogfood section already fixed; add gate-baseline evidence reference where rows cite stale runs)

- [ ] **Step 1: Re-run the full standing gate set** (same list as Task 6 Step 2, now including `claim-audit`). All must pass.
- [ ] **Step 2: Write the closure evidence file** — commands, cwd, SHA, exit codes, and a table mapping each Phase 0 task to its commit hash.
- [ ] **Step 3: Commit** — `git add -A && git commit -m "docs: record phase 0 truth-repair closure evidence"`
- [ ] **Step 4: Merge.** Use superpowers:finishing-a-development-branch to merge `fix/phase-0-truth-repair` to `main` (or open a PR per repo convention — recent history shows PR-based merges).

---

# Phase 1 — M8: Manual Daily Driver Beta

**Exit gate (from v0.2 §9 / `HERMESGOAL.md` M8):** GP-1 passes on the Legion repo; Manual zero-egress stays green; one week of documented dogfood shows no P0/P1 blockers; failures degrade visibly; ledger updated with evidence and platform scope.

Each work packet below becomes its own detailed plan (superpowers:writing-plans) when picked up. Packets 1.1–1.4 are independent; 1.5–1.6 depend on all of them.

### WP-1.1: Rust LSP product workflow (WS-LANG-01)

- **Objective:** A user opens the Legion repo and gets real rust-analyzer completion, hover, and diagnostics in the product UI, with visible lifecycle/crash/restart states.
- **Current state:** `LspSupervisor`, `LspStdioLauncher`, framing, and init handshake exist in `legion-lsp`; rust-analyzer launch is env-var opt-in with a mock fallback (`crates/legion-lsp/src/lib.rs:524-536`); diagnostics ingestion exists (`ingest_lsp_publish_diagnostics_for_buffer` in `legion-desktop/src/workflow.rs`).
- **Deliverables:** rust-analyzer discovery order (PATH, config, bundled-none) with provenance surfaced; real launch as the default product path (mock only under `cfg(test)`/injected transports); open/change/save document sync; completion with stale-snapshot rejection; diagnostics in the problems panel; restart/backoff UX with a visible failure banner; redacted LSP logs; Windows smoke evidence (macOS/Linux caveats recorded if no runner).
- **Verification:** `cargo test -p legion-lsp -p legion-app --test language_tooling_workflow`; a scripted smoke that launches the desktop binary against this repo with rust-analyzer on PATH; evidence dir `plans/evidence/production/WS-LANG-01/`.
- **Acceptance:** PR-LANG-001 gains a current product UX evidence row (stays "Substrate validated" until cross-platform, per ledger rules).

### WP-1.2: Terminal productization (WS-TERM-01)

- **Objective:** The integrated terminal is safe and useful under policy: shell selection, kill, cleanup, scrollback, resize.
- **Current state:** Real ConPTY (`crates/legion-terminal/src/conpty.rs`) and Unix PTY (`legion-platform`); app composes `TerminalRuntime<NativePtyService>`; grid/OSC/session modules exist.
- **Deliverables:** shell selection policy; process-group kill verified; orphan-cleanup evidence (spawn, kill app, prove no orphan); scrollback limits and search; resize propagation; env allow/deny policy; failure UX; Windows smoke evidence.
- **Verification:** `cargo test -p legion-terminal -p legion-app --test terminal_workflow`; orphan-cleanup script output captured to `plans/evidence/production/WS-TERM-01/`.
- **Acceptance:** terminal rows of PR-LANG-002 cite current product evidence.

### WP-1.3: Search and Git promotion to product evidence (WS-SEARCH-01 + WS-GIT-01)

- **Objective:** Promote the already-working streaming/indexed search, fuzzy open, palette, and git workflows from substrate tests to evidenced product workflows.
- **Deliverables:** search cancellation + stale-marker UX evidence on a large repo (use the RW reference workspaces from `plans/evidence/production/WS-MANUAL-02/reference-workspaces.md`); `.gitignore` parity check; fuzzy-open latency measurement via the perf harness; git stage/unstage/commit-with-validation UX path; diff viewer evidence; explicit deferred cut lines recorded for local history and jj posture if not built.
- **Verification:** existing suites (`daily_editing_search`, `git_workflow` across app/desktop/project) plus new evidence in `plans/evidence/production/WS-SEARCH-01/` and `WS-GIT-01/`.
- **Acceptance:** search/git rows cite current evidence; any cut line is explicit in ledger and docs.

### WP-1.4: Debug decision (WS-DEBUG-01) — REQUIRED DECISION, either branch is acceptable

- **Objective:** Resolve the plan's biggest open decision: `legion-debug` models DAP state but never spawns a process.
- **Option A (build v1 critical path):** pick the Rust DAP adapter (CodeLLDB or lldb-dap) via a short ADR + dependency-policy entry; real adapter launch against a tiny fixture binary; breakpoint set/remove; start/continue/step/stop; stack/variables; zero-config `cargo test` debug for one test. This is multi-week work.
- **Option B (explicit beta cut line):** record in the ledger and `docs/USER_GUIDE.md` that interactive debugging is deferred from the Manual beta; keep the test explorer path (cargo test discovery + rerun failed) which does not need DAP; define the unblock condition (ADR + adapter provenance policy).
- **Recommendation:** Option B for M8, Option A scheduled inside M12. A daily-driver Rust beta survives without a debugger sooner than it survives without LSP/terminal/git; do not let DAP block GP-1.
- **Acceptance:** either current DAP evidence or an explicit ledger/docs cut line — the current silent gap is the only unacceptable state.

### WP-1.5: GP-1 automation (WS-QUALITY-01 slice)

- **Objective:** GP-1 (Manual Daily Edit) becomes a runnable check, not prose: open Legion repo → edit Rust → syntax visible → fuzzy open + search → LSP completion/diagnostics → terminal `cargo test` → git diff → safe save → commit, asserting zero hosted egress throughout.
- **Deliverables:** `cargo run -p xtask -- golden-path gp-1` orchestrating existing test suites plus the desktop smoke, emitting an evidence file; wire into the weekly dogfood procedure.
- **Verification:** the command itself, green, on Windows; evidence under `plans/evidence/production/WS-QUALITY-01/`.

### WP-1.6: Dogfood week (M8 exit criterion)

- **Objective:** One week of Legion-on-Legion journal entries using `plans/dogfood/legion-on-legion-weekly-journal-template.md`, each naming branch, commit, OS, workflow, evidence path, result, and readiness impact. P0/P1 findings become Kanban tasks and block M8 exit until fixed or explicitly waived.

---

# Phase 2 — M9: Assist Private Beta

**Exit gate:** GP-2 passes with a local provider (hosted BYOK live smoke or an explicitly recorded credential blocker); manifest preview before invocation; provider/model/egress/cost/retention visible; all write intent becomes proposals; validation/rollback/evidence export/rejection work; Manual zero-egress stays green.

- **WP-2.1 Provider plane completion (WS-AI-01):** live Ollama (or llama.cpp) smoke on the dev machine; provider health panel; per-provider cost estimate + actual usage records; timeout/retry/cancellation evidence; BYOK secret storage via the existing `keyring` dependency; route-refusal UX. Anthropic/OpenAI paths already have native clients + injected-transport tests — needing live smoke only where credentials exist, otherwise record the blocker per `HERMESGOAL.md` M9 exit wording.
- **WP-2.2 Context engine (WS-AI-02):** manifest preview UX before invocation (schema and tests exist; the preview-before-send product path is the gap); citations/provenance rows; AGENTS/rules discovery; context budgeting with truncation explanation; privacy-inspector planned-vs-actual diff. Embeddings stay deferred until the measured local-first decision the plan requires.
- **WP-2.3 Assist UX (WS-AI-03):** chat-to-proposal for multi-file changes; explanation-only mode that cannot create proposals; inline edit preview; cancellation that kills provider streams with no partial mutation; keyboard review/apply/reject. Inline prediction accept/reject already passes tests — build outward from it.
- **WP-2.4 Proposal review core (WS-TRUST-01 slice):** risk classes, graduated approvals, diff-first review surface, rollback/checkpoint UI, evidence bundle export. Substrate exists (`legion_workflow_integration`, save/conflict suites); the packet productizes review UX and export.
- **WP-2.5 GP-2 automation + evidence** mirroring WP-1.5.

# Phase 3 — M10: Delegate Public Beta

**Exit gate:** GP-3 passes on the Legion repo; sandbox-escape suite green or platform caveats explicit; kill/cancel/reap works; main workspace untouched until approval; worker output is proposal + evidence.

- **WP-3.1 Lane end-to-end (WS-AGENT-01):** task packet → worktree (fallback copy with degraded status) → scoped tools → validation → proposal bundle. Most parts landed in `236a492` (`dag.rs`, `scheduler.rs`, `scope.rs`, `plan.rs`, `evidence.rs`, worktree tests); the packet integrates them into one reviewed product flow with kill/cancel/reap evidence (Task 7's reaper is the floor, not the ceiling — add reap-on-cancel and reap-on-app-close).
- **WP-3.2 Adversarial evals made real (WS-TRUST-01 slice):** the four hostile fixtures under `evals/legion-bench/hostile/` are 3-line stubs; make prompt-injection, malicious tool output, exfiltration-lure, and bad-patch scenarios real, blocking tests wired into `cargo run -p xtask -- legion-bench` recorded mode. The ledger's "Deferred (adversarial evals)" row cannot survive into a public Delegate beta.
- **WP-3.3 Sandbox tiers + escape suite (legion-sandbox):** document OS tier differences honestly (Windows caveats must never be described as equivalent to Linux/macOS tiers — invariant §2.3); escape tests per tier.
- **WP-3.4 GP-3 automation + evidence.**

# Phase 4 — M11: Legion Workflows Command Center

**Exit gate:** GP-4 passes; workflows stop safely on policy/cost/validation/conflict/cancellation; every lane visible; merge readiness requires evidence and approvals.

- **WP-4.1 Fleet operations:** global/per-lane kill switch verified under fault injection (no lane may silently ignore kill); budget caps; pause/resume/steer; "why stopped" terminal states. Projections already pass 6 command-center tests — the packet proves the runtime honors them.
- **WP-4.2 Replay + merge readiness:** replay from metadata/evidence without raw content; conflict detection; merge-readiness gating on evidence + approvals (`merge_readiness.rs` landed in `236a492`).
- **WP-4.3 ACP interop (conditional):** only behind an ADR + policy + conformance tests with one real external adapter; otherwise record the explicit deferral. Do not let this block M11 exit.
- **WP-4.4 GP-4 automation + evidence.**

# Phase 5 — M12: Production Beta Release

**Exit gate:** unsigned-beta installers produced per the recorded M5 policy cut line; fresh-VM smoke passes or a platform blocker is documented; support-bundle redaction passes; update/rollback tested or explicitly cut with docs/ledger update; GP-5/GP-6 covered to beta scope.

- **WP-5.1 Installers + fresh-VM smoke (WS-REL-01):** Windows first (`scripts/package-windows.ps1` exists); macOS/Linux evidence or explicit platform blockers; first-run privacy/provider setup; offline/air-gap install path.
- **WP-5.2 Update/rollback + crash controls:** staged rollout + rollback path, opt-in crash reporting with local crash bundles — or an explicit beta cut with docs/ledger update (the plan allows either; silence allows neither).
- **WP-5.3 Launch extension subset (WS-EXT-01):** themes, keymaps, snippets, tree-sitter grammars, safe command contributions; permission review UI; extension-originated writes as proposals; GP-5 evidence. Runtime extension host stays behind its PR-VSC-002 cut line.
- **WP-5.4 Enterprise evidence export (GP-6):** redacted audit bundle export for a completed AI-assisted change — metadata, hashes, decisions, validation, deletion handles; support-bundle redaction test.
- **WP-5.5 Debug Option A lands here if WP-1.4 chose the cut line.**

# Phase 6 — M13: GA Readiness

Not a work phase — a verification gate. GA is claimed only when: GP-1–GP-6 pass (or carry accepted, explicit caveats in the ledger); every promoted ledger row has current evidence; release/update/crash/support flows are proven on target platforms; no P0/P1 blockers; deferred surfaces are named and not marketed; security/privacy claims match implementation (`HERMESGOAL.md` §13). The exit artifact is a refreshed readiness ledger where every row is either "Product workflow validated" or "Deferred with explicit cut line" — nothing in between.

---

## Sequencing and dependency spine

```
Phase 0 (truth repair)
  └─ Phase 1 / M8 (WP-1.1..1.4 parallel → WP-1.5 → WP-1.6)
       └─ Phase 2 / M9 (WP-2.1..2.4 mostly parallel → WP-2.5)
            └─ Phase 3 / M10 (WP-3.1 ∥ WP-3.2 ∥ WP-3.3 → WP-3.4)
                 └─ Phase 4 / M11
                      └─ Phase 5 / M12 (WP-5.1..5.5)
                           └─ Phase 6 / M13 gate
```

Rules carried from `HERMESGOAL.md` §12: do not start Assist work before GP-1 is stable; do not start Delegate before GP-2; every implementation step must move at least one golden path closer to passing; update the Kanban `status` field (Task 9) as packets complete so "next unblocked task" stays computable.
