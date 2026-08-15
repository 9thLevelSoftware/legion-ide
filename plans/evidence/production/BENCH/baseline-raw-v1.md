# Legion-Bench raw baseline and first raw-vs-governed comparison (P9.F1.T4)

Date: 2026-08-15. Roadmap Phase 0.6 / Phase 2 exit criterion.
Model: **qwen2.5-coder:7b** (Q4_K_M, 4.68 GB) via Ollama 0.32.13 at
`http://127.0.0.1:11434/v1`. Suite fingerprint
`bench-suite-v1:929fa3b1f30b0920`. 13 executed tasks, 5 holdout tasks
excluded (ADR-0049 holdout policy).

## Headline: a real qualitative difference, not yet a ≥20% claim

Three runs per arm, same corpus, same model, alternating nothing but
`LEGION_AI_GOVERNORS`:

| per run | raw | governed |
| --- | ---: | ---: |
| **tasks passed (gate)** | 0, 0, 0 | 0, 0, 2 |
| task_success | 0, 0, 0 | 0, 2, 4 |
| tasks where the model acted | 0, 0, 0 | 6, 11, 9 |
| tool calls | 0, 0, 0 | 12, 21, 20 |
| files actually changed | 0, 0, 0 | 0, 2, 4 |

**The raw arm is a deterministic zero.** Not "low" — zero on every metric in
every run, because the model emits no structured tool calls at all and a strict
provider therefore sees prose and an ended turn. Nothing happens, reproducibly.

**The governed arm acts on roughly two thirds of tasks** and completes between
zero and two of them. That is the difference between a local model being
unusable and being marginal.

**It is still not the ≥20% improvement the roadmap asks for.** Mean pass rate
is 0.7/13 ≈ 5%, and the run-to-run spread (0 to 2 passes) is larger than the
effect being claimed. Phase 2's exit criterion is **not met**, and no
percentage from this file should be quoted as a result. What is established is
a floor comparison: zero versus non-zero, reproducible in both directions.

**Three runs is the minimum honest sample, not a sufficient one.** Local
sampling is nondeterministic and n=13 tasks on one model at one quantization
cannot support a percentage. A defensible figure needs more tasks, more
repetitions, and more than one model.

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

## What still stops the governed arm

Corpus problems recorded in the first version of this file have been fixed:
task prompts told the model to run verification the harness already performs,
and scopes granted a terminal that Windows cannot sandbox — so the model
dutifully tried, was denied, and the run ended. Prompts now say explicitly not
to run commands, and no task grants `terminal-command`. Blocked runs fell from
5 of 13 to 1–2.

The benchmark then surfaced a real defect in the port itself: a model that
named `edit-as-proposal` correctly but passed `content` instead of
`replacement` had its arguments forwarded untouched, because a literal
tool-name match skipped argument canonicalization. One run spent its entire
retry budget re-sending the same rejected field. Fixed, with the argument
renaming now applied to Legion's own tools even when the name needs no change.

What remains is mostly the model: it reads files, sometimes proposes an edit,
and often produces an edit that does not satisfy the task. That is the honest
category — a 7B model at Q4 on multi-step repository tasks — and it is what
the rest of Phase 2 (loop governors, plan anchoring, context budgeting) exists
to improve.

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

**Everything the port added is off in the raw arm, not just the visible
part.** Review found the switch leaking governed behaviour three ways, each of
which would have run a reliability mechanism inside the supposedly ungoverned
baseline: the advertised edit schema (below), the *ordering* of validation (a
malformed edit reached the path checks first and became a terminal block
instead of retryable feedback), and malformed structured arguments (recovered
into a retryable `MalformedToolCall` where the pre-port provider failed the
completion outright). All three now follow the switch, each pinned by a test
in `governor_ab_seam.rs`.

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
