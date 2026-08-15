# Legion-Bench raw baseline and raw-vs-governed comparison (P9.F1.T4)

Date: 2026-08-15. Roadmap Phase 0.6 / Phase 2 exit criterion.
Model: **qwen2.5-coder:7b** (Q4_K_M, 4.68 GB) via Ollama 0.32.13 at
`http://127.0.0.1:11434/v1`. Suite fingerprint
`bench-suite-v1:f34b7e1a124dbe91`. 13 executed tasks, 5 holdout tasks
excluded (ADR-0049 holdout policy). Four raw runs, seven governed.

## Correction to the previous version of this file

An earlier revision reported the governed arm passing "0, 0, 2" tasks. **That
figure was wrong.** It came from counting `tests_passed`, which is not the
suite gate and, worse, is inherited rather than earned on four of the thirteen
tasks — see "The metric that was lying" below. Every number in this revision
is computed from the gate the suite actually applies, and the harness now
records the measurement that makes the error impossible to repeat.

## Headline: zero versus non-zero activity, and a gate neither arm clears

The suite gate is `task_success ∧ tests pass ∧ within diff/turn budgets`. It
is the right measure for both task kinds: a bug fix must turn failing tests
green, a refactor must change the named files without turning green tests red,
and `task_success` (a real diff to the expected files, cleanly applied) is
what stops a model that did nothing from scoring either.

| per run | raw (4 runs) | governed (7 runs) |
| --- | ---: | ---: |
| **suite gate** | 0, 0, 0, 0 | 0, 0, 0, 1, 2, 0, 0 |
| task_success | 0, 0, 0, 0 | 3, 4, 4, 4, 4, 1, 6 |
| tasks where the model acted | 0, 0, 0, 0 | 8–11 |
| tool calls | 0, 0, 0, 0 | 15–26 |

**The raw arm is a deterministic zero.** Not "low" — zero on every metric in
every run, because the model emits no structured tool calls at all and a
strict provider therefore sees prose and an ended turn. Nothing happens,
reproducibly.

**The governed arm acts on most tasks and edits the right files on one to six
of thirteen.** That is the difference between a local model being inert and
being engaged.

**Neither arm reliably clears the gate.** Three passes across 91 governed
task-runs is 3%, and the run-to-run spread (`task_success` 1 to 6) is larger
than any effect being claimed. The governed arm changes the files a task names
and then usually changes them wrongly.

**Phase 2's ≥20% exit criterion is not met and is not close.** No percentage
in this file should be quoted as a result. What is established is narrow and
real: the reliability layer moves a local model from producing nothing to
producing something, and something is usually still wrong.

## What the failures actually are

Diagnosed by keeping the checkouts (`LEGION_BENCH_KEEP_CHECKOUTS=1`), recording
rejection reasons in the report, and probing the endpoint directly with
Legion's own `edit-as-proposal` schema. Two failure modes dominate, and both
are now handled:

* **A lone `new_str` with no `old_str`** — roughly half of the model's edit
  calls. It means "here is the new version of this function". Both obvious
  readings are wrong: refusing wastes a turn the model does not recover from,
  and treating it as the file's complete content deletes everything the model
  did not retype. Legion now anchors on the block's first line when that line
  appears exactly once, replacing that block and nothing else.
* **`old_str` not found exactly as written** — the anchor is reconstructed
  from memory and the spacing drifts. Resolution now falls back to a
  whitespace-insensitive, line-aligned, still-unique search. The bytes
  replaced are the file's own, so an applied edit stays exact; only the search
  became tolerant.

Both are individually tested (18 cases in `crates/legion-ai/src/patch.rs`) and
both reverse an earlier exact-only policy that the corpus vectors had pinned.
**Neither is shown to move the suite number.** Three runs before and three
after are indistinguishable against a spread this wide. They are recorded as
justified by an observed failure mode, not as a measured improvement.

What remains after them is the model. It reads a file, edits the right one,
and the edit does not do what the task asked — a 7B model at Q4 on multi-step
repository work. Closing that is not a matching problem.

## Why the raw arm scores zero: the model never emits a structured tool call

Probing the endpoint directly with a tool definition:

```
finish_reason: stop
tool_calls   : null
content      : "{\"name\": \"read\", \"arguments\": {\"path\": \"main.rs\"}}"
```

qwen2.5-coder:7b through Ollama's OpenAI-compatible endpoint returns **no**
`tool_calls`. It writes the call as bare JSON in the message content and
reports `finish_reason: "stop"`. A strict provider sees prose and an ended
turn, which is exactly what the raw arm measures: every task ran one turn,
made zero tool calls, and finished having done nothing.

This is the behaviour the SmallCode port was built for, observed on the
reference model rather than assumed. It also justifies two decisions that
looked defensive in review: `scan_bare` (a bare JSON object as the whole
message *is* the call) and reporting `ToolUse` when recovery fires despite
`finish_reason: "stop"` — without the latter the loop would end the run before
dispatching what it just recovered.

## Corpus problems fixed along the way

Recorded because they shaped the numbers above, not because they remain open.

Task prompts told the model to run verification the harness already performs,
and scopes granted a terminal that Windows cannot sandbox — so the model
dutifully tried, was denied, and the run ended. Prompts now say explicitly not
to run commands, and no task grants `terminal-command`. Blocked runs fell from
5 of 13 to 0–2.

The benchmark then surfaced a real defect in the port: a model that named
`edit-as-proposal` correctly but passed `content` instead of `replacement` had
its arguments forwarded untouched, because a literal tool-name match skipped
argument canonicalization. One run spent its entire retry budget re-sending
the same rejected field.

The loop governors landed in this revision are not aimed at task completion at
all: they contain waste. The idle-turn stop did not fire once across every run
recorded here.

## A scoring bug found and fixed before recording anything

The first raw run reported **six** tasks with `task_success = true` while
making zero tool calls and zero proposals; one scored a full pass. Cause:
`task_success` was `expected_files_ok && applied == total`, and `0 == 0` holds
while `expected_files` mostly names files that already exist in the fixture.
A model that replies with prose and edits nothing therefore "succeeded", and
on a task whose tests pass at rest it passed outright.

That would have inflated the baseline the governed arm is measured against —
understating the port's value, the direction that quietly discredits real
work.

Review then found the same hole one level deeper, twice:

* An accepted proposal whose content **equals the existing file** still
  incremented `proposals_applied`, so a no-op edit passed a
  "proposals > 0" test. Success now requires a non-empty diff — evidence the
  requested change happened, not evidence the model produced output.
* `diff_files` itself was over-counting. Starting a delegated task writes
  `target/delegated-tasks/<id>.lock` **into the workspace**. Rust fixtures hide
  it behind `/target` in `.gitignore`; Python and JavaScript fixtures do not,
  because `target/` is a Rust convention. So every non-Rust task reported one
  changed file that the model never touched — visible in the raw arm, which
  made zero tool calls and still showed `diff_files = 1` on 8 tasks. The
  harness now excludes its own runtime artifacts, and records the *names* of
  changed files so a disagreement between diffs and proposals is diagnosable
  instead of mysterious.

`tests_passed_at_rest` is the fourth instance of the same failure, and the
first one caught by reading a result that looked *good* rather than one that
looked wrong. All four pointed the same way: crediting the model for work it
had not done. Pinned by `a_run_that_proposed_nothing_is_never_a_success`,
`a_proposal_that_changed_nothing_is_never_a_success`, and
`legion_runtime_artifacts_do_not_count_as_model_changes`.

## How the two arms are compared

The roadmap called for freezing a raw baseline *before* any governed code
landed. That window closed: the normalizer shipped in the same commit as the
benchmark harness (#132), so no revision has the harness without the
governors. Comparing across commits would confound the result with unrelated
changes.

Instead both arms run from the **same binary** under
`LEGION_AI_GOVERNORS=off|unset`, which disables tolerant recovery, fragment
resolution, and the loop governors, reproducing the pre-port contract. This is
a stronger design than the original plan. The seam is itself tested
(`crates/legion-agent/tests/governor_ab_seam.rs`), including that only the
exact value `off` disables it — a typo silently running the wrong arm would
invalidate a result without any visible symptom.

**Everything the port added is off in the raw arm, not just the visible
part.** Review found the switch leaking governed behaviour three ways, each of
which would have run a reliability mechanism inside the supposedly ungoverned
baseline: the advertised edit schema (below), the *ordering* of validation (a
malformed edit reached the path checks first and became a terminal block
instead of retryable feedback), and malformed structured arguments (recovered
into a retryable `MalformedToolCall` where the pre-port provider failed the
completion outright). All three now follow the switch, each pinned by a test
in `governor_ab_seam.rs`, as do the three loop governors.

**Both halves of the contract move together.** Review caught that the raw arm
was still *advertising* the governed edit schema (`old_str`/`new_str`, with
`replacement` no longer required) while *refusing* fragment edits — so a model
following the contract it was handed would have been penalised for an
interface that did not exist before the port, biasing the comparison toward
the governed arm. The advertised schema now switches with the enforcement,
asserted by `the_advertised_edit_schema_matches_the_arm`.

That flaw could not have affected the numbers above: the raw arm recorded zero
tool calls on all 13 tasks, so it never reached the edit executor at all. The
raw arm was nonetheless re-run after the fix and produced an identical result,
so the figures here are measured under schema parity rather than merely argued
to be unaffected.

## Sample size

Four raw runs and seven governed runs on 13 tasks, one model, one
quantization. The raw arm is perfectly reproducible, so its zero is solid. The
governed arm's spread is not: `task_success` ranged 1–6 and the gate 0–2 with
nothing changing but sampling. Three gate passes across 91 governed task-runs
is an anecdote, and no arithmetic on it becomes a percentage worth quoting.

## Status

`P9.F1.T4` stays **in-progress**. The baseline is recorded, reproducible, and
measured against the right gate. The comparison it exists to support shows a
real qualitative change — nothing to something — and no reliable movement on
task completion.

The two edit-resolution stages added here were chosen from measured failure
modes rather than guessed at, which is the right way to pick them, and they
still did not visibly move the suite. That is worth saying plainly: the
remaining gap is the model's ability to write a correct change, not Legion's
ability to apply one. Next, in order:

1. Context budgeting and model profiles (Phase 2.4).
2. Plan anchoring — re-inject the current step each turn.
3. Re-measure with more repetitions and at least one larger model, so the
   spread stops swamping the effect.

## Reproduction

```
ollama pull qwen2.5-coder:7b
LEGION_AI_GOVERNORS=off LEGION_BENCH_MODEL=qwen2.5-coder:7b \
  cargo run -p xtask -- legion-bench --mode live-local     # raw arm
LEGION_BENCH_MODEL=qwen2.5-coder:7b \
  cargo run -p xtask -- legion-bench --mode live-local     # governed arm
```

Holdout tasks stay excluded; add `--include-holdout` only at a phase exit.
