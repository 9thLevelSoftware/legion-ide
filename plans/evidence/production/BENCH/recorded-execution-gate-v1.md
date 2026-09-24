# Legion-Bench: recorded execution as a regression gate (P9.F1.T1 / T3 / T4)

Date: 2026-08-19. Closes P9.F1.T1, P9.F1.T3 and P9.F1.T4.

## What changed, in one sentence

`legion-bench --mode recorded` no longer computes scores from gate budgets; it
executes every corpus task against a real fixture checkout and replays only the
model's half of the conversation from committed cassettes, and
`verify-legion-bench` fails when the measured result differs from a committed
per-task baseline.

## What was there before

`plan_default_legion_bench_suite()` built twenty tasks over four fixture
directories that **did not exist** (`fixtures/workspace-save`,
`fixtures/diff-review`, `fixtures/symbol-refactor`,
`fixtures/multi-file-feature`). `score_task()` then derived every metric from
the task's own budget:

```rust
let slack = (ordinal as u32 % 3) + 1;
let diff_files = budget.max_diff_files.saturating_sub(slack).max(1);
let tests_passed = true;
```

The report labelled itself `scoring_mode = "synthetic_budget_arithmetic"`, so
it was honest, but it was arithmetic over a task list, not a measurement of
anything. Twenty of those tasks had no deterministic scoring rule because they
had no fixture to score. That is the P9.F1.T1 stop condition, and it is why
the whole synthetic path is deleted rather than relabelled.

A real execution path already existed as `--mode live-local`, but it needed a
local model, so it could not be a CI gate.

## The design

Recorded mode and live mode now run the *same* code path. The only difference
is where the model's responses come from.

```
fixture repo ──copy──▶ temp checkout (LF-normalized, git baseline commit)
                          │
                          ▼
      AppComposition::start_delegated_task  ── tools ──▶ checkout
                          │                             (read/grep/glob/
                          │                              outline/edit-as-proposal)
                     ModelProvider
                          │
              ┌───────────┴────────────┐
        live endpoint            cassette replay
     (record / live-local)         (recorded)
                          │
                          ▼
             proposals ──apply──▶ checkout ──▶ verification command ──▶ exit code
```

Everything below the provider is the product: the delegated-task loop, scope
containment, the tool broker, the patch applier, the proposal and save
pipelines. A change to any of them changes what the replayed conversation does
to the checkout, which changes the measured result, which fails the gate.

### The seam

`ProviderHttpTransport::post_json` is the only place the runner touches the
network, so record and replay wrap that one method. Recording appends
`(request fingerprint, response)` to a per-task tape; replay serves the tape in
order.

### Why replay is deterministic

* **Ordered, not keyed.** The Nth request gets the Nth recorded response.
* **The tape cannot be over-run.** Running past the end returns a provider
  error naming the exhausted tape. A replay that quietly ended early would
  score as a real, worse result with no explanation.
* **The checkout is LF-normalized on copy.** Git hands Windows a CRLF working
  copy and Linux an LF one from the same commit; the model's `old_str` anchors
  match one or the other, not both. `copy_dir_recursive` rewrites CRLF to LF
  for every UTF-8 file and the checkout's git config sets
  `core.autocrlf false`, so a cassette is not tied to the platform that cut it.
  Pinned by `checkout_copies_are_lf_normalized`.
* **The tape names its own model.** The wire model id goes into the request
  payload, so replaying under a different name would make every request differ
  from the recorded one. A replay uses the cassette's model, not the
  invocation's.
* **Drift is counted, not ignored.** Each recorded exchange stores a
  fingerprint of the request it answered, with the temp checkout path and every
  UUID normalized out — a fresh proposal id per edit would otherwise make every
  post-edit request differ on a value the model never reads. Replay counts
  mismatches as `cassette_drift` and the baseline pins the value per task, so
  "the loop still asks the model the same thing" is an assertion rather than a
  hope. See "What is not established" for the two tasks whose drift is not
  zero.

### The deterministic scoring rule (P9.F1.T1 stop condition)

Per task, all of it committed in the task's TOML:

| Input | Source | Determinism |
| --- | --- | --- |
| `tests_passed` | `[verification] command` exit code == `expected_exit`, run with cwd = the checkout after proposals are applied | exit-code comparison |
| `task_success` | proposals applied cleanly, a non-empty diff, and every `expected_files` path present | file-system facts |
| `diff_files` | `git diff --cached --name-only` against the baseline commit, minus Legion's own runtime artifacts | git |
| `turns` | count of `ModelResponse` audit steps | audit log |
| `cost_cents` | 0 for local/replayed runs, recorded as such with a note | constant |
| `score` | `100 − diff·w_diff − turns·w_turn − (cents/2)·w_cost − (fail ? w_fail : 0)`, weights in `[scoring]` | integer arithmetic |
| `status` | `task_success ∧ tests-gate ∧ within every budget` | boolean |

No step is a judgement, a similarity measure, or a model-graded score.
`every_corpus_task_has_a_deterministic_scoring_rule` asserts the shape of this
for every task in the corpus and additionally runs the structural half of the
corpus-health gate in-process.

### The regression gate

`evals/legion-bench/recorded/baseline.toml` holds provenance (model, arm,
endpoint, corpus fingerprint, and a SHA-256 over the cassette files) plus one
row per task. `verify-legion-bench` compares status, score, `tests_passed`,
`diff_files`, `turns`, `task_success`, `tool_calls`, `duplicate_tool_calls`,
`retries` and `cassette_drift`, and reports **every** difference rather than
the first.

`legion-bench --mode recorded` additionally refuses to run at all when the
cassette files no longer hash to `cassette_set_hash`, so a hand-edited tape is
a refusal rather than a new number.

A recorded run's task *failures* are not themselves a gate failure — they are
the reference model's failures, faithfully replayed, and they are the baseline.
Treating them as CI failures would make the offline leg red forever the moment
the model got one task wrong.

## The corpus (P9.F1.T1, P9.F1.T4)

**25 tasks across 6 fixture repositories and 3 languages, 5 held out.**
Suite fingerprint `bench-suite-v1:e6ae1f88e5dfbca4`.

| Fixture repo | Language | Tasks |
| --- | --- | ---: |
| `fixtures/bench-py-tool` | Python | 8 |
| `fixtures/bench-ts-app` | JavaScript | 7 |
| `fixtures/bench-rust-lib` | Rust | 5 |
| `fixtures/bench-rust-cli` | Rust | 2 |
| `fixtures/gp1-rust` | Rust | 2 |
| `fixtures/bugfix-count-markers` | Rust | 1 |

By kind: 11 bug fixes, 8 test-adds, 3 refactors, 3 multi-file features.

The 5 holdout tasks are excluded from every run and reported as `skipped`;
they enter a run only under `--include-holdout`, which is reserved for a phase
exit. They are still in the suite fingerprint, so the fingerprint does not
depend on the flag.

The acceptance floors (20-50 tasks, >=3 repos, a non-empty holdout that is not
the whole corpus) are asserted by
`in_repo_corpus_meets_the_documented_size_floor`.

Two task timeouts moved in this change: `bench-py-04` and `bench-ts-03` go from
120s to 300s. At 120s the reference model was still mid-request when the
watchdog cancelled the loop, which recorded a tape that ended before the
conversation did. `timeout_secs` is not part of the suite fingerprint, so this
does not move the corpus identity.

## CI (P9.F1.T3)

| Workflow | Triggers | Needs a model? | Can gate a merge? |
| --- | --- | --- | --- |
| `legion-bench.yml` | push to main, **pull_request**, dispatch | no | yes — that is the point |
| `legion-bench-live.yml` | schedule, dispatch | yes | no |

The recorded leg installs Rust, Python and Node, runs the corpus-health gate,
runs `legion-bench --mode recorded`, and runs `verify-legion-bench`. It sets no
`LEGION_BENCH_*` variable and reads no secret.

The live leg has no `push` and no `pull_request` trigger, does not run unless
the repository variable `LEGION_BENCH_LIVE` is `true` *and* a runner label is
configured, is `continue-on-error: true`, and invokes the bench with
`--no-strict`.

The separation is a test, not a convention:
`xtask/tests/legion_bench_ci_contract.rs` parses the workflow files and asserts
that the always-on workflow triggers on `pull_request` and mentions no
`LEGION_BENCH_*` variable or `secrets.`, that **no workflow except
`legion-bench-live.yml` invokes a non-recorded bench mode**, and that the live
workflow has no push/PR trigger and is `continue-on-error`. A future edit that
wires a live run into a PR gate fails `cargo test`.

## What the two frozen baselines say

Both were cut on 2026-08-19 with **qwen2.5-coder:14b** (Q4_K_M) served by
Ollama at `http://127.0.0.1:11434/v1`, against suite fingerprint
`bench-suite-v1:e6ae1f88e5dfbca4`, 20 tasks executed and 5 held out.

| | governed (`evals/legion-bench/recorded/`) | raw (`evals/legion-bench/recorded-raw/`) |
| --- | --- | --- |
| `LEGION_AI_GOVERNORS` | unset | `off` |
| suite gate passed | **4 / 20** | **0 / 20** |
| `task_success` | 9 / 20 | 0 / 20 |
| tasks with any tool call | 12 / 20 | 0 / 20 |
| average score (graded tasks) | 59 | 57 |
| turns per task | 1-5 | 1 on every task |
| tool calls per task | 0-5 | **0 on every task** |
| cassette-set hash | `sha256:9a57f6d8…1b62cb1cd9` | `sha256:44f1711a…4d9af0352b` |

Tasks the governed arm cleared: `bench-py-04`, `bench-py-07`, `bench-py-08`,
`bench-rust-07`.

**The raw arm is a deterministic zero on the current corpus.** One turn per
task, no tool calls, no proposals, no file changed — because the model writes
its tool call as bare JSON in the message content and reports
`finish_reason: "stop"`, and an ungoverned provider sees prose and an ended
turn. This reproduces, on the repaired 20-task corpus, what
`baseline-raw-v1.md` recorded on the superseded 13-task one.

Both baselines are now replayable offline and re-checked on every pull request,
which is what "frozen" has to mean for a number this easy to accidentally
improve.

### Is the raw baseline genuinely ungoverned?

Yes, in the sense the stop condition means and no further. The roadmap's
original plan — measure before any governed code exists — was already
impossible when this task was written: the normalizer shipped in the same
commit as the benchmark harness (#132). What is used instead is the
`LEGION_AI_GOVERNORS=off` A/B seam, which disables tolerant tool-call recovery,
fragment resolution, the loop governors, the advertised edit schema and the
ordering of validation, and is itself tested in
`crates/legion-agent/tests/governor_ab_seam.rs` — including that only the exact
value `off` disables it.

The recorded harness now enforces the arm mechanically as well: every cassette
records the arm it was cut under, and a replay refuses a tape from the other
arm rather than measuring the wrong loop. Verified by running the raw replay
without the variable:

```
legion_bench_live: [1/20] bench-live-01 outcome=error ...
  error=cassette `…/recorded-raw/bench-live-01.json` was recorded under the
  `raw` arm but this process runs `governed` (LEGION_AI_GOVERNORS);
  replaying across arms measures neither
```

## Verification run locally

All on Windows 11, `cargo … -j 6`, 2026-08-19.

| Command | Result |
| --- | --- |
| `cargo run -p xtask -- legion-bench --mode recorded` | `total=25 passed=4 failed=16 skipped=5 average_score=59` |
| `cargo run -p xtask -- verify-legion-bench` | matches baseline (model=qwen2.5-coder:14b arm=governed) |
| `LEGION_AI_GOVERNORS=off … legion-bench --mode recorded --cassettes …/recorded-raw` | `total=25 passed=0 failed=20 skipped=5 average_score=57` |
| `LEGION_AI_GOVERNORS=off … verify-legion-bench --cassettes …/recorded-raw` | matches baseline (arm=raw) |
| `cargo run -p xtask -- verify-legion-bench-corpus` | 25 task(s), 25 healthy |
| `cargo clippy --workspace --all-targets -- -D warnings` | clean |
| `cargo fmt --all --check` | clean |
| `cargo test -p xtask` | green |
| `cargo test -p legion-app --bin legion_bench_live --features test-helpers` | 14 passed |
| `check-deps`, `docs-hygiene`, `claim-audit`, `verify-kanban-backlog`, `verify-readiness-consistency` | all exit 0 |

A full replay of the 20 executed tasks takes about 30 seconds and opens no
socket. The live recording it replaces took roughly 15 minutes.

## Mutation proofs

A test that passes whether or not the feature works is worthless. Each row was
produced by breaking the thing, observing the failure, restoring it, and
confirming `git diff` was clean.

**The one that matters — a real product regression moves the gate.** In
`crates/legion-ai/src/patch.rs`, `apply_edit`'s whitespace-insensitive fallback
was disabled (`.filter(|_| false)` on the fuzzy match), reproducing the
exact-only policy that preceded the SmallCode port. Nothing else changed. The
replayed run then differed on `bench-ts-03`:

```
task `bench-ts-03` regressed:
  measured  diff_files: 1, retries: 2, score: 47, cassette_drift: 1
  baseline  diff_files: 3, retries: 0, score: 39, cassette_drift: 0
```

`verify-legion-bench` exited 1. Restoring the line and re-running produced
`recorded run matches baseline` and `git diff` on the file was empty. This is
the proof that the gate observes the product and not just its own arithmetic:
one edited expression in the patch applier, three metrics moved, gate red.

| # | Break | Expected failure | Observed |
| --- | --- | --- | --- |
| 1 | Disable the fuzzy anchor fallback in `patch.rs` | recorded gate red | `verify-legion-bench` exit 1, `bench-ts-03` diff above |
| 2 | Append `"tampered": true` to a cassette | run refuses to start | `cassette set hash sha256:2a51af9f… does not match the committed baseline sha256:9a57f6d8…`, exit 1 |
| 3 | Change one baseline row's `score` 57 → 58 | verify red, naming the task | `task bench-py-06 regressed: measured … score: 57 … baseline expects … score: 58`, exit 1 |
| 4 | Add `pull_request:` to `legion-bench-live.yml` | CI-contract test red | `the live bench workflow must not trigger on pull_request` |
| 5 | Move the 7 `bench-ts-*` tasks out of the corpus | corpus-floor test red | `corpus must hold 20-50 tasks, found 18` |
| 6 | Drop `.replace("\r\n", "\n")` from `copy_dir_recursive` | LF-normalization test red | `assertion left == right failed` |
| 7 | Make `cassette_set_hash` skip file contents | hash test red | `the hash must cover cassette contents, not just their size` |
| 8 | Make replay wrap around instead of erroring | exhaustion test red | `a third call must not be answered: … "first"` |
| 9 | Set a task's `at_rest` to `"maybe"` | scoring-rule test red | `bench-ts-03: at_rest must be `passes` or `fails`, got `maybe`` |
| 10 | Replay the raw tapes without `LEGION_AI_GOVERNORS=off` | cross-arm refusal | every task errors with `recorded under the `raw` arm but this process runs `governed`` |

After each, the change was reverted and `git diff` confirmed clean.

**Mutation 7 found a vacuous assertion.** The first version of
`cassette_set_hash_changes_when_a_tape_changes` passed with the content
excluded from the digest, because the two tape versions also differed in
*length* and the length prefix alone moved the hash. The test now also rewrites
the tape to a same-length, different-byte value — and only then does removing
the content from the digest fail it.

## What is not established

**One platform.** The baselines were measured on Windows, and the CI recorded
leg is pinned to `windows-latest` for that reason. The harness LF-normalizes
every checkout and sets `core.autocrlf false` in it precisely so a cassette is
not tied to the platform that cut it, and `checkout_copies_are_lf_normalized`
pins that — but no run on Linux or macOS has happened, so portability is a
designed-for property, not a measured one. Extending the matrix needs either a
confirmed-identical replay on the new platform or a per-platform baseline.

**Two tasks carry non-zero cassette drift.** `bench-rust-04` (3) and
`bench-rust-08` (2) — the two tapes containing more than one
`edit-as-proposal` call. The checkout path and every UUID are normalized out of
the request fingerprint, and the residual source was not isolated. It is
*stable*: an independent replay reproduces both values exactly, and mutation 1
moved a third task's drift from 0 to 1, so the signal still works. But "drift
is zero unless the loop changed" is not true of these two, and a reader should
not expect it to be.

**The gate measures one model's conversation.** A change that makes Legion
better for a model whose responses are not on these tapes is invisible here, and
a change that only affects this model's failure modes will look larger than it
is. Recorded mode is a regression gate, not a quality measure.

**Four of twenty is not a result.** The governed arm's 4/20 and the raw arm's
0/20 are single runs of a 14B model at Q4 on multi-step repository work. They
are baselines to detect change against, not evidence about the reliability
layer's value; that argument lives in `baseline-raw-v1.md` and is weaker than
the numbers alone suggest.

**Re-recording is a manual, local operation.** It needs a machine with the
reference model installed. That is the cost of the design: CI cannot re-cut a
tape, which is exactly why CI cannot silently rebaseline a regression away.
