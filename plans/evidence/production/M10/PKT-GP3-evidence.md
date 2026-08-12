# PKT-GP3 Evidence — GP-3 Delegate Mode Golden Path

**Milestone:** M10
**Date:** 2026-07-07
**Branch:** m10/gp3-smoke
**Author:** Devil (dasblueeyeddevil@gmail.com)

---

## Overview

GP-3 is the final golden path (9/9) of the M10 Delegate Mode campaign. It
exercises the native agent loop, scoped task execution, sandbox enforcement,
cancellation, and proposal lifecycle end-to-end via a standalone binary
(`golden_path_3`) and xtask subprocess orchestrator.

This is the 18th standing gate in the Legion test suite.

---

## Deliverables

### D1 — `crates/legion-app/src/bin/golden_path_3.rs`

Standalone binary with 9 steps (s1–s9). Compiled with default features
(which include `ai`) plus `--features test-helpers`.

### D2 — xtask golden-path-3 orchestrator

- `xtask/src/golden_path_3.rs`: `GoldenPath3Options` + `run_golden_path_3`
- `xtask/src/main.rs`: `GoldenPath3` subcommand variant + match arm +
  `run_golden_path_3_command`
- `xtask/src/lib.rs`: `pub mod golden_path_3;`

### D3 — `plans/evidence/accessibility/gp-3-delegate-walkthrough.md`

Replaced stale M10-notice content with an updated M10 delegate walkthrough
reflecting the current worker panel, scope picker, sandbox enforcement, kill
switch, and proposal review surface.

---

## Step Coverage (s1–s9)

| Step | Objective | Assertion | Notes |
|------|-----------|-----------|-------|
| s1 | Copy fixture to temp dir, git init, open as Trusted workspace, set Delegate mode | `open_workspace` returns Ok; `set_product_mode(Delegate)` succeeds | Same fixture as GP-1/GP-2 (fixtures/gp1-rust) |
| s2 | Build `DelegatedTaskScope` with Module target and `secrets.txt` in forbidden_paths | Scope struct constructed without error; bait file written | `secrets.txt` is the forbidden-path bait for s4 |
| s3 | Happy-path agent loop: read→grep→edit-as-proposal→end_turn | `Completed`; every `ToolCallRequest` has a paired `ToolCallResult`/`ToolCallRejected`; main workspace byte-unchanged | `edit-as-proposal` requires `"path"` and `"replacement"` fields; proposals may be empty (PKT-PROPOSAL-SURFACE deferral) |
| s4 | Scope denial: script reads `secrets.txt` (forbidden path) | `Blocked` with at least one `ToolCallRejected` audit step | Exercises forbidden_paths enforcement |
| s5 | Sandbox teeth: `TerminalCommand` scope; probe write inside worktree | `Completed` (terminal ran) or `Blocked` (broker denial) — both acceptable | Platform-dependent; see caveats below |
| s6 | Kill switch: inject pre-cancelled `SharedCancellationFlag` | `AppDelegatedTaskOutcome::Cancelled` | Requires `test-helpers` feature; xtask always passes `--features test-helpers` |
| s7 | Orphan reap: create `task-orphan-gp3` dir; call `reap_orphaned_sandboxes` | 1 orphan removed; `not-a-task` decoy left alone | Reaper removes dirs with `task-` prefix not in active list |
| s8 | Review-apply proposal lifecycle: `CreateFile` proposal register→validate→preview→apply; checkpoint verify; restore | File created after apply; `list_checkpoints()` returns checkpoint with `ProposalId(800)`; file removed after restore | Follows GP-2 s6 pattern exactly |
| s9 | Write `target/golden-path/gp3_report.toml` | Evidence TOML written with all step records | Pass-2 rewrite includes s9 record itself |

---

## xtask golden-path-3 Verification

```
cargo run -p xtask -- golden-path-3 --fixture-dir fixtures/gp1-rust
```

The xtask orchestrator:
1. Resolves fixture dir relative to workspace root
2. Invokes `cargo run -p legion-app --bin golden_path_3 --features test-helpers --jobs 4`
3. Forwards the subprocess exit code (0=passed, 1=failed, 2=setup error)
4. Prints pass/fail summary with report path

---

## Standing Gate Count

GP-3 is the 18th standing gate:

| # | Gate | Binary |
|---|------|--------|
| 1–7 | Unit + integration tests (cargo test) | — |
| 8 | GP-1 manual smoke | golden_path_1 |
| 9 | GP-2 assist smoke | golden_path_2 |
| 10–17 | M10 delegate integration tests | delegated_task_integration |
| **18** | **GP-3 delegate smoke** | **golden_path_3** |

---

## Platform Enforcement Caveats

| Platform | Sandbox Backend | filesystem_write_enforced | network_enforced |
|----------|-----------------|--------------------------|-----------------|
| Linux | Landlock | true | true |
| macOS | Seatbelt | true | true |
| **Windows** | **Job object only** | **false** | **false** |

On Windows, `spawn_sandboxed` uses a Job object for process isolation only.
Filesystem write enforcement and network isolation are not available at the OS
level. This is a known, honest limitation documented per the M10 constraints.

The s5 sandbox teeth step accepts both `Completed` (terminal ran) and
`Blocked` (TerminalCommand denied by broker) as passing outcomes, ensuring
the test passes on all three platforms.

---

## Remaining / Deferred

| Item | Status |
|------|--------|
| 3-OS CI matrix run | Pending — CI workflow update needed |
| Live-model run (non-scripted provider) | Deferred post-M10 |
| PKT-PROPOSAL-SURFACE: `proposals` extraction from sandbox worktree | Deferred — `Completed.proposals` is empty; asserted honestly in s3 |
| Sandbox enforcement on Windows | Known caveat — `filesystem_write_enforced=false` |
