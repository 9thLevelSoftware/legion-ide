# Legion-Bench raw baseline and first raw-vs-governed comparison (P9.F1.T4)

Date: 2026-08-15. Roadmap Phase 0.6 / Phase 2 exit criterion.
Model: **qwen2.5-coder:7b** (Q4_K_M, 4.68 GB) via Ollama 0.32.13 at
`http://127.0.0.1:11434/v1`. Suite fingerprint
`bench-suite-v1:929fa3b1f30b0920`. 13 executed tasks, 5 holdout tasks
excluded (ADR-0049 holdout policy).

## Headline: the ≥20% improvement claim is **not** demonstrated

| | raw (`LEGION_AI_GOVERNORS=off`) | governed |
| --- | ---: | ---: |
| **tasks passed (gate)** | **0 / 13** | **1 / 13** |
| task_success | 0 | 2 |
| tasks where the model acted | 0 | 10 |
| tool calls | 0 | 23 |
| proposals applied | 0 | 2 |
| files changed | 0 | 2 |
| runs blocked | 0 | 2 |

**Phase 2's exit criterion is not met.** One task out of thirteen is not a
≥20% improvement; it is a single success on a single model in a single run,
and this file should not be cited as evidence that the reliability layer
improves task completion.

Two things it does establish. The layer is the difference between a model that
does nothing and a model that acts: 0 → 10 tasks with tool calls, 0 → 23 calls,
0 → 2 files actually changed. And the arms now separate at all, which they did
not before the metric was corrected.

**One run per arm is an observation, not a measurement.** Local sampling is
nondeterministic: an earlier governed run of the same corpus produced 11 acted
/ 26 calls / 1 task_success where this one produced 10 / 23 / 2. Any figure
quoted from this file needs repeated runs before it means anything.

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

This is the behavior the SmallCode port was built for, observed on the
reference model rather than assumed. It also justifies two decisions that
looked defensive in review: `scan_bare` (a bare JSON object as the whole
message *is* the call) and reporting `ToolUse` when recovery fires despite
`finish_reason: "stop"` — without the latter the loop would end the run before
dispatching what it just recovered.

## Why the governed arm still scores near zero

Recovery works — 0 → 10 tasks where the model acted, 0 → 23 tool calls — but
almost all tasks still fail, for reasons that are mostly **not** model
quality (counts from the earlier run, whose per-task failure reasons were
captured; the pattern is unchanged):

| cause | tasks | nature |
| --- | ---: | --- |
| Windows sandbox cannot enforce isolation, so `terminal-command` is denied and the run ends | 2 | platform gap (roadmap Phase 5.12) |
| model reached for a tool the task did not grant, ending the run | 2 | corpus authoring |
| model tried to edit a file protected as the grading oracle | 1 | working as designed |
| acted but produced no accepted edit | 7 | model capability + corpus prompts |
| produced an edit that failed verification | 1 | model capability |

Two of these are ours to fix, and both inflate failure independently of the
model:

1. **Prompts tell the model to run verification the harness already runs.**
   Several tasks end with "then run `python -m unittest …` to confirm". On
   Windows the sandbox denies terminal commands, so the model dutifully tries,
   is denied, and the run terminates — having already made its edit in some
   cases. The harness runs verification itself after applying proposals; the
   prompts should not ask for it.
2. **Task scopes omit tools their prompts imply.** `bench-rust-01` grants no
   `terminal-command` yet the task invites verification.

`tests_passed` *dropped* 4 → 2 between arms, which looks like a regression and
is not: in the raw arm nothing changed, so tasks whose tests already pass at
rest kept passing. Blocked governed runs never reach verification at all. This
is why `tests_passed` is not a success metric on its own.

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

All three holes pointed the same way: crediting the model for work it had not
done. Pinned by `a_run_that_proposed_nothing_is_never_a_success`,
`a_proposal_that_changed_nothing_is_never_a_success`, and
`legion_runtime_artifacts_do_not_count_as_model_changes`.

## How the two arms are compared

The roadmap called for freezing a raw baseline *before* any governed code
landed. That window closed: the normalizer shipped in the same commit as the
benchmark harness (#132), so no revision has the harness without the
governors. Comparing across commits would confound the result with unrelated
changes.

Instead both arms run from the **same binary** under
`LEGION_AI_GOVERNORS=off|unset`, which disables tolerant recovery and fragment
resolution and reproduces the pre-port contract. This is a stronger design
than the original plan. The seam is itself tested
(`crates/legion-agent/tests/governor_ab_seam.rs`), including that only the
exact value `off` disables it — a typo silently running the wrong arm would
invalidate a result without any visible symptom.

**Both halves of the contract move together.** Review caught that the raw arm
was still *advertising* the governed edit schema (`old_str`/`new_str`, with
`replacement` no longer required) while *refusing* fragment edits — so a model
following the contract it was handed would have been penalised for an
interface that did not exist before the port, biasing the comparison toward
the governed arm. The advertised schema now switches with the enforcement,
asserted by `the_advertised_edit_schema_matches_the_arm`.

That flaw could not have affected the numbers above: the raw arm recorded zero
tool calls on all 13 tasks, so it never reached the edit executor at all. The
raw arm was nonetheless re-run after the fix and produced an identical result
(0/13 passed, 0 tasks acted), so the figures here are measured under schema
parity rather than merely argued to be unaffected.

## Status

`P9.F1.T4` stays **in-progress**. The baseline is recorded and reproducible,
but the comparison it exists to support is not yet meaningful, because both
arms are floored at zero by causes above. Next, in order:

1. Remove verification instructions from task prompts and align scopes with
   what each task actually needs.
2. Re-run both arms; expect the governed arm to separate from raw once runs
   are not terminated by denials.
3. Only then quote a percentage — and quote it with n, since 13 tasks on one
   model is a signal, not a proof.

## Reproduction

```
ollama pull qwen2.5-coder:7b
LEGION_AI_GOVERNORS=off LEGION_BENCH_MODEL=qwen2.5-coder:7b \
  cargo run -p xtask -- legion-bench --mode live-local     # raw arm
LEGION_BENCH_MODEL=qwen2.5-coder:7b \
  cargo run -p xtask -- legion-bench --mode live-local     # governed arm
```

Holdout tasks stay excluded; add `--include-holdout` only at a phase exit.
