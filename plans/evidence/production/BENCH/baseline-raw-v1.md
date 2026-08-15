# Legion-Bench realism + raw baseline — status (P9.F1.T4)

Date: 2026-08-15. Branch: `phase-0-truth-repair`. Roadmap Phase 0.6.

## Why this exists

The roadmap's central AI claim — that the SmallCode-derived control plane
makes the *same* local model measurably better (target ≥20% relative) — is
only meaningful against a baseline measured **before** any governed layer
exists, on a frozen corpus, with a held-out subset. ADR-0049 records the
holdout policy. This file tracks that baseline.

## Done: bench scoring is now real

Previously `xtask legion-bench` scored tasks by
`synthetic_budget_arithmetic` — it never opened a fixture repo or ran an
agent (self-declared in `.github/workflows/legion-bench.yml`). Now:

- **`--mode live-local`** copies each fixture repo to a temp workdir, drives
  the delegated-task agent loop against an OpenAI-compatible endpoint
  (`LEGION_BENCH_ENDPOINT`, default `http://127.0.0.1:11434/v1`;
  `LEGION_BENCH_MODEL` required), applies the resulting proposals, runs the
  task's verification command, and scores real outcomes (task success, tests
  passed, turns, tool calls, wall time, errors).
- Execution runs through `crates/legion-app/src/bin/legion_bench_live.rs`
  (spawned by xtask, mirroring the golden-path binaries) so the loop keeps
  the same worktree/broker/scope containment as GP-3 — xtask stays thin and
  the dependency policy (xtask must not depend on legion-app) is preserved.
- **Recorded mode remains the CI default** with its report shape and suite
  fingerprint unchanged (`bench-suite-v1:bd2aa3a7d84d9485`, 20/20 passing),
  so `verify-legion-bench` and the weekly workflow stay green. Its honest
  `synthetic_budget_arithmetic` labeling is retained.
- Holdout support: live mode skips `holdout = true` tasks unless
  `--include-holdout` is passed; skipped tasks are recorded as `Skipped`.

Tests: `cargo test -p xtask --test legion_bench` — 15/15 pass, covering
corpus TOML parsing (incl. holdout + verification fields), recorded report
shape stability, live scoring within/over budget, holdout skip accounting,
and live-config resolution.

## Done: corpus expanded to 18 tasks across 4 fixture repos, 3 languages

| Fixture repo | Language | Tasks | Notes |
| --- | --- | --- | --- |
| `fixtures/bench-rust-cli` | Rust | 2 | word/line/char counting CLI; two seeded bugs, RED at rest by design |
| `fixtures/bench-rust-lib` | Rust | 3 | INI-style config parser; GREEN at rest (13 unit + 5 integration + 1 doctest) |
| `fixtures/gp1-rust` | Rust | 1 | existing smoke fixture, reused for a test-add task |
| `fixtures/bench-ts-app` | JavaScript | 5 | expense-ledger CLI, stock `node`, zero deps |
| `fixtures/bench-py-tool` | Python | 5 | "TextKit" text utilities, stdlib only |
| (live-mode examples) | — | 2 | `bench-live-01/02` authored with the runner |

All four task kinds (BugFix, TestAdd, Refactor, MultiFileFeature) are
covered. **5 of 18 are holdout.** Every fixture is self-contained: no
network at verification time, no package installation, no `.git`, no build
artifacts. Each corpus author verified their tasks in both states where
applicable — pristine (bug present → verification fails, toolchain healthy)
and hand-fixed (verification passes).

Corpus lives at `evals/legion-bench/tasks/`; loading is deterministic and
fingerprinted (`in_repo_corpus_loads_and_fingerprints_deterministically`).

### Grading-oracle protection (bench validity)

A task is only a valid measurement if the model cannot pass it by editing
the thing that grades it. Each task's `[scope].forbidden_paths` now protects
its own oracle — 16 of 18 tasks:

- bug-fix / refactor tasks forbid the test directory or the specific test
  files that must go red→green;
- test-add tasks forbid the **source** under test, so the model cannot
  reshape the code to satisfy a trivial test;
- feature tasks forbid the verification/check scripts and their fixture
  inputs.

**Known limitation (2 tasks):** `bench-rust-01` and `bench-rust-02` use
`fixtures/bench-rust-cli`, whose unit tests are inline `#[cfg(test)] mod
tests` blocks **inside the same files the fix must edit** (`src/stats.rs`,
`src/cli.rs`). Path-level forbidding cannot separate oracle from subject
there. Until a runner-side oracle restore (re-materialize the pristine test
blocks before verification) exists, treat those two tasks' results as
requiring a diff check at baseline time — the live runner already records
per-task diffs, so this is an inspection step, not an unknown.

## Blocked: the raw baseline itself

**No local model runtime is installed on this machine** — no Ollama, no
`llama-server`; ports 11434/8080/1234 are closed. A live-local run therefore
executes the full harness but every task errors at the provider call:

```
legion bench (live-local): total=18 passed=0 failed=13 skipped=5 ... exit 1
error=... provider `http` request failed: error sending request for url
       (http://127.0.0.1:11434/v1/chat/completions)
```

That is the correct failure shape (clear error, non-zero exit, no panic, no
partial-credit scoring), and it proves the harness end-to-end up to the
model boundary.

Toolchain note: verification commands are shelled (`cmd /C` on Windows,
`sh -c` elsewhere). The Python tasks invoke `python`, which resolves on this
machine (3.11.15) but is frequently absent on Linux images that ship only
`python3`; the Node tasks invoke `node` (v24.19.0 here). If the baseline is
ever measured on another OS, confirm both interpreters resolve first — a
missing interpreter would score as a task failure rather than an
environment error.

**To freeze the baseline**, install a runtime and re-run:

```
ollama pull qwen2.5-coder:7b
LEGION_BENCH_MODEL=qwen2.5-coder:7b cargo run -p xtask -- legion-bench --mode live-local
```

Then record here: the model + quantization, endpoint, suite fingerprint,
per-task outcomes, and the aggregate pass rate — that becomes
**baseline-raw-v1**, the number Phase 2's governed loop must beat on the
held-out subset. Until then, treat the ≥20% target as unmeasured.
