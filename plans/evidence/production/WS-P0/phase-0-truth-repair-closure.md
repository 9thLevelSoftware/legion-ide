# Phase 0 Truth Repair — Closure Evidence

Date: 2026-07-02
Branch: `fix/phase-0-truth-repair`
Working directory for all commands: repo root, `C:\Users\dasbl\RustroverProjects\legion-ide`
Task started at commit `02bd068` (HEAD when Task 10 began).
Commit at closure evidence capture: `fd715c57ee5a9ec8c6d7631b16c38dbdc65944b4` (`fix: apply rustfmt to branch-touched sandbox reaping files`, landed during this task — see Gate 7 below).

This is Task 10 of the Phase 0 truth-repair plan
(`docs/superpowers/plans/2026-07-02-legion-production-shippable-program.md`, Phase 0).
Step 4 (merge) is out of scope for this file — handled by the controller after this
task's commit lands.

## Step 1: Full standing gate set — commands, results, real fixes

All commands run from repo root, one cargo command at a time, `-j 4` for every
`cargo test`/`cargo check`/`cargo clippy` invocation, per the controller's environment
rules. Disk was checked before the run (`df -h /c` → 81 GB free / 925 GB, well above
the 10 GB stop threshold) and stayed healthy throughout — no disk-exhaustion incident
this task.

| # | Command | Start | End | Exit | Result |
| --- | --- | --- | --- | --- | --- |
| 1 | `cargo run -p xtask -- check-deps` | 03:25:47 | 03:25:55 | 0 | `dependency policy checks passed` |
| 2 | `cargo run -p xtask -- docs-hygiene` | 03:25:55 | 03:25:58 | 0 | `documentation hygiene checks passed` |
| 3 | `cargo run -p xtask -- claim-audit` | 03:25:58 | 03:26:01 | 0 | `claim audit passed` |
| 4 | `cargo run -p xtask -- no-egui-textedit` | 03:26:01 | 03:26:04 | 0 | `no-egui-textedit checks passed` |
| 5 | `cargo run -p xtask -- verify-kanban-backlog` | 03:26:04 | 03:26:07 | 0 | `kanban backlog ok: 10 epic(s), 38 feature(s), 146 task(s)` |
| 6 | `cargo test -p xtask --test kanban_backlog -j 4` | 03:26:07 | 03:26:07 | 0 | 17 passed, 0 failed |
| 7 | `cargo fmt --all --check` | 03:26:10 | 03:26:10 | **1 (first run)**, then 0 after fix | See "Real finding: fmt drift" below |
| 8 | `cargo check --workspace --all-targets -j 4` | 03:26:45 | 03:26:54 | 0 | All workspace crates + xtask checked clean |
| 9 | `cargo test --workspace --all-targets --no-fail-fast -j 4` | 03:27:03 | 03:32:54 | 0 | **1522 passed, 0 failed, 12 ignored** |
| 10 | `cargo clippy --workspace --all-targets -j 4 -- -D warnings` | 04:25:10 | 04:25:50 | 0 | No warnings, no errors |
| 11 | `cargo deny check` | 04:25:54 (first) / 04:27:15 (final) | — | **1 (first run)**, then 0 after fix | See "Real finding: cargo-deny advisory" below |
| 12 | `python3 -m pytest evals training -q` | 04:27:20 (first) | — | 1 (first, environmental), 0 (PowerShell re-run) | See "Note on gate 12" below |

### Real finding 1: `cargo fmt --all --check` drift on branch-touched files

First run failed with formatting drift in exactly three files:

- `crates/legion-agent/tests/sandbox_reaping.rs`
- `crates/legion-app/src/offline_ai.rs`
- `crates/legion-app/tests/delegated_task_integration.rs`

`git diff --name-only 236a492..HEAD` confirmed all three were touched by this branch
(Task 7's `reap_orphaned_sandboxes` work and Task 7's offline-AI mirror/tests). Per the
controller's rule ("if fmt fails on branch-touched files, fix those"), ran
`cargo fmt --all` — it reformatted exactly these three files (confirmed via
`git status --short` showing no other files touched) — and `cargo fmt --all --check`
then passed clean (exit 0). No pre-existing/untouched-file drift was found in this run
(unlike Task 6's baseline, where none existed either).

- **Commit:** `fd715c5` — `fix: apply rustfmt to branch-touched sandbox reaping files`.
- **Verification:** `cargo fmt --all --check` exit 0 after the commit.

### Real finding 2: `cargo deny check` — new advisory RUSTSEC-2026-0194 (quick-xml)

First run failed: `advisories FAILED` on `RUSTSEC-2026-0194` (quick-xml 0.39.4,
quadratic-time start-tag duplicate-attribute check — a CPU-DoS-class algorithmic
complexity issue, not memory-unsafety/RCE). This advisory did not exist at Task 6's
baseline capture (`3474de6`) — it is a newly published RUSTSEC entry discovered by
this run, not a regression introduced by any commit on this branch.

Investigation:
- `git show 236a492:Cargo.lock` vs current `Cargo.lock`: `quick-xml 0.39.4` was already
  locked at branch start — this is a pre-existing transitive dependency, not something
  this branch's commits pulled in.
- `cargo tree -i quick-xml` shows a single resolved path:
  `quick-xml 0.39.4 <- plist 1.9.0 <- syntect 5.3.0 <- legion-app <- legion-desktop`
  (syntax-highlighting theme parsing of bundled/local `.tmTheme` files — not
  attacker-controlled network XML in Legion's usage).
- `cargo update -p quick-xml --precise 0.41.0 --dry-run` **fails**: `plist 1.9.0`
  (the latest release on crates.io, confirmed via `cargo info plist`) pins
  `quick-xml = "^0.39.2"`, which cannot resolve to the fixed 0.41.0 line. No in-repo
  dependency bump can fix this — it requires an upstream `plist` release that relaxes
  or bumps its own `quick-xml` constraint.

This is a genuine upstream blocker, not fixable within this branch's scope, following
the same pattern `deny.toml` already uses for reviewed/documented exceptions (see the
file's `[bans]` skip-list commentary and `[licenses]` review notes). Added a
documented `ignore` entry:

```toml
# RUSTSEC-2026-0194 (quick-xml 0.39.4, quadratic-time attribute-duplicate check):
# reached only transitively via plist 1.9.0 <- syntect 5.3.0 (bundled/local
# .tmTheme syntax-highlighting theme parsing, not attacker-controlled network
# XML). plist 1.9.0 is the latest release on crates.io and pins
# `quick-xml = "^0.39.2"`, so no in-repo dependency bump can reach the fixed
# 0.41.0 line (`cargo update -p quick-xml --precise 0.41.0` fails: no matching
# version satisfies plist's constraint). Upstream plist must relax/bump its
# quick-xml pin before this can be resolved from our side. Revisit when plist
# releases a version compatible with quick-xml >=0.41.0.
ignore = ["RUSTSEC-2026-0194"]
```

- **Verification after fix:** `cargo deny check` exits 0
  (`advisories ok, bans ok, licenses ok, sources ok`).
- Re-ran `cargo run -p xtask -- claim-audit` after this `deny.toml` edit as a
  precaution (docs/claim-audit gates can be sensitive to policy-file changes) — still
  passes clean.
- Two pre-existing `unmatched-skip` **warnings** remain (`objc2-metal@0.2.2`,
  `objc2-quartz-core@0.2.2`) — confirmed present in `deny.toml` at branch start
  (`git show 236a492:deny.toml`), unrelated to macOS crates not resolved on this
  Windows toolchain, unrelated to this branch's work, and warn-level (not
  gate-failing). Left untouched per the controller's fix-only-what-you-broke
  instruction; recorded here as a pre-existing gap, matching Task 6's baseline note.
- **Commit:** to be included in this task's closure commit (`deny.toml` change is
  currently unstaged pending Step 3).

### Note on gate 12 (`python3 -m pytest evals training -q`)

First run failed one of two tests:
`ReviewerFixtureEvalTest::test_reviewer_fixture_cli_writes_output` raised
`OSError: [WinError 6] The handle is invalid` inside Python's
`subprocess.run(..., capture_output=True)` → `_winapi.DuplicateHandle`. Re-ran once
immediately under the same shell (Git-Bash-backed Bash tool) — failed identically both
times (not a transient single-shot flake in that shell). This is the exact same
Windows subprocess-spawn handle-inheritance failure signature already documented in
Task 6's baseline evidence (`plans/evidence/production/WS-P0/phase-0-gate-baseline.md`,
"Note on gate 10 re-run") for the identical test.

To isolate whether this was shell-specific, re-ran the same command under native
PowerShell (not Git Bash): both tests passed cleanly
(`2 passed in 0.19s`). This confirms the failure is an artifact of the Git-Bash-backed
Bash tool's stdio handle setup when spawning nested Python subprocesses on Windows —
not a code defect, not a test assertion failure, and not caused by any change on this
branch (the test and the code under test are both untouched by this task). Recorded
here as an environment-shell artifact rather than silently discarded; the PowerShell
run is the authoritative pass for this gate.

## Pass/fail summary

All 12 gates exit 0 as of the final state on this branch (commit `fd715c5` plus the
uncommitted `deny.toml` fix, both included in this task's closure commit). Full
workspace test suite: **1522 passed / 0 failed / 12 ignored**. `cargo clippy -D
warnings`: 0 warnings. `cargo deny check`: advisories/bans/licenses/sources all `ok`
after the documented RUSTSEC-2026-0194 ignore entry. Python suite: 2 passed / 0 failed
(PowerShell run; Git-Bash run hits a known shell-specific handle-inheritance artifact,
recorded above, not a real failure).

## Step 2: Task → commit table (Tasks 1–10, Phase 0 truth repair)

Source: `git log --oneline 236a492..HEAD` cross-referenced with
`.superpowers/sdd/progress.md` (completion log).

| Task | Commit(s) | One-line outcome |
| --- | --- | --- |
| 1. Remove foreign-project contamination | `6881a16`..`5c21454` (`5c21454`) | Removed foreign-project audit files committed by mistake; added HERMESGOAL gap analysis and production-shippable program plan. |
| 2. Fix README CI claim | `5c21454`..`f1e36ed` (`a6d701d`, `f1e36ed`) | Corrected README CI claim after legion-bench workflow was added; swept remaining stale no-CI claims in AGENTS.md and operator runbook. |
| 3. Fix ledger dogfood template path | `f1e36ed`..`ba0c787` (`ba0c787`) | Fixed dogfood journal template path in readiness ledger. |
| 4. Qualify docs/releases/v8.0.0 as forward template | `ba0c787`..`b77850d` (`b77850d`) | Marked v8.0.0 release docs as forward-looking templates (banner added to all 3 affected files). |
| 5. Clean clippy warnings | `b77850d`..`62502af` (`e1dbae1`, `757b605`, `56e83c1`, `62502af`) | Removed unused imports/orphaned scheduler helpers; grouped DAP audit_record args; satisfied clippy ptr_arg/useless_conversion lints in legion-desktop — clippy gate green. |
| 6. Clean full-gate baseline + fix real failures | `62502af`..`6c5d6d1` (`102ecdd`, `823bc42`, `3474de6`, `6c5d6d1`) | Fixed stale `large_file_streaming` and `compat_report` Tier2 test expectations; patched anyhow RUSTSEC-2026-0190 and rebaselined cargo-deny duplicate skips; restored full standing gate set to green (1509 tests passing) with evidence baseline (`phase-0-gate-baseline.md`). |
| 7. Delegated-task sandbox reaping | `6c5d6d1`..`c89b72e` (`4e3009c`, `c89b72e`) | Implemented `reap_orphaned_sandboxes` in legion-agent (TDD), wired into `legion-app`/desktop startup; added offline-AI mirror with bidirectional drift guards and offline tests. |
| 8. claim-audit xtask gate | `c89b72e`..`d7dda95` (`d7dda95`) | Added `claim-audit` xtask gate checking docs against the readiness ledger (3 TDD tests + CLI + README/RUNBOOK docs); fixed a real violation in HERMESGOAL.md's mission-statement wording. |
| 9. Kanban backlog status tracking | `d7dda95`..`02bd068` (`02bd068`) | Added `status`/`evidence` fields to kanban backlog schema with validation (`done` requires non-empty evidence); reconciled milestone mapping; 17/17 kanban tests passing. |
| 10. Phase 0 closure evidence | `02bd068`..(this commit) (`fd715c5` + this file's commit) | Re-ran full standing gate set (12 gates incl. claim-audit) to green; fixed rustfmt drift on 3 branch-touched files; added documented `cargo-deny` ignore for upstream-blocked RUSTSEC-2026-0194; wrote this closure evidence file and the task→commit table. |

## Environment notes carried forward from the ledger

- Disk-space pressure was flagged twice during Tasks 6–7 (`.superpowers/sdd/progress.md`
  "ENVIRONMENT NOTE" entries: 0 bytes free at one point, resolved via
  `target/` cleanup; then 5 GB free, resolved via `cargo clean -p legion-desktop/-app/-agent`).
  At Task 10 start, `df -h /c` showed 81 GB free (925 GB drive, 92% used) — healthy,
  no incident this task, but the drive remains systemically ~90%+ full. Recommend the
  user consider `[profile.dev] debug = "line-tables-only"` or `split-debuginfo` to
  structurally reduce `target/` size, or periodic `cargo clean -p <heavy-crate>` as
  a standing hygiene step, per the Task 6/7 recommendation.
- Task 7's pre-existing find (out of Task 10 scope, restated for final triage): 5 tests
  in `legion-app/tests/assist_inline_prediction_workflow.rs` fail under
  `--no-default-features --features offline` (they assume AI is always on). This does
  not affect the standing gate set above (which does not pass `--no-default-features`)
  and was not re-verified in this task; flagging again per the ledger's "final review
  triage" note.

## Self-review

- Every gate row above reflects a command actually executed in this session this task,
  with real captured tails; none are assumed or carried over from Task 6's baseline
  without re-running.
- Both real findings (fmt drift, cargo-deny advisory) were triaged against the
  environment rules: fmt drift was on branch-touched files, so fixed directly; the
  cargo-deny advisory was investigated for a direct dependency-bump fix first
  (`cargo update -p quick-xml --precise 0.41.0 --dry-run`, confirmed blocked by
  upstream `plist`'s pin) before falling back to a documented `ignore` entry — not a
  silent suppression.
- The gate-12 pytest failure was re-run once (per environment rules) in the same
  shell, reproduced identically, then cross-checked in a different shell (PowerShell)
  to isolate it as a shell artifact rather than a real regression, and recorded rather
  than discarded.
- The task→commit table covers all 19 commits from `236a492..HEAD` (18 commits present
  at task start + this task's own `fd715c5`), matching every non-merge commit in
  `git log --oneline 236a492..HEAD`.
- No test was weakened to pass and no assertion was deleted; the only source changes
  in this task are formatting (rustfmt, mechanical) and a documented, justified
  `deny.toml` policy exception.
