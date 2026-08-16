# Legion-Bench raw baseline and raw-vs-governed comparison (P9.F1.T4)

Date: 2026-08-15. Roadmap Phase 0.6 / Phase 2 exit criterion.
Models: **qwen2.5-coder:7b** (Q4_K_M, 4.7 GB, local), **qwen2.5-coder:14b**
(Q4_K_M, 9.0 GB, local) and **deepseek-v4-flash:0731-cloud** (Ollama Cloud),
all via Ollama 0.32.13 at `http://127.0.0.1:11434/v1`. Suite fingerprint
`bench-suite-v1:f34b7e1a124dbe91`. 13 executed tasks, 5 holdout tasks
excluded (ADR-0049 holdout policy). Four raw runs, seven governed.

## Correction to the previous version of this file

An earlier revision reported the governed arm passing "0, 0, 2" tasks. **That
figure was wrong.** It came from counting `tests_passed`, which is not the
suite gate and, worse, is inherited rather than earned on four of the thirteen
tasks — see "The metric that was lying" below. Every number in this revision
is computed from the gate the suite actually applies, and the harness now
records the measurement that makes the error impossible to repeat.

## Headline (local 7B): zero versus non-zero activity, and a gate neither arm clears

For the larger model, where both arms actually work and the comparison is
therefore meaningful, see "A non-degenerate comparison" below.

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

## A non-degenerate comparison: deepseek-v4-flash

Everything above is measured on a model that emits **no** structured tool calls,
which makes the raw arm's zero uninformative: "the reliability layer beats a
model that does nothing" is true and nearly vacuous. The obvious objection is
that the layer only rescues a broken driver and would add nothing to a
competent one.

Run against **deepseek-v4-flash:0731-cloud** through the same local endpoint
(Ollama proxying Ollama Cloud), the same 13 tasks, three runs per arm,
alternating so any drift in the service hits both arms equally:

| per run | raw | governed |
| --- | ---: | ---: |
| **suite gate** | 2, 3, 2 | **6, 5, 6** |
| mean | 2.33 / 13 (18%) | **5.67 / 13 (44%)** |
| task_success | 5, 5, 5 | 7, 5, 6 |
| tasks where the model acted | 13, 13, 13 | 13, 13, 13 |
| tool calls | 105, 50, 64 | 62, 82, 73 |

This model calls tools properly — `finish_reason: tool_calls`, the right tool
name, the right argument names — so **both arms act on all 13 tasks**. The
comparison is like-for-like: two working agents, one with the layer and one
without.

**The ranges do not overlap.** The governed arm's worst run (5) beats the raw
arm's best (3), and the governed passing set is nearly identical across runs —
`bench-live-01`, `bench-py-01`, `bench-py-03`, `bench-rust-01`, `bench-ts-01`
every time, plus `bench-ts-03` in two of three. That stability is what makes
three runs worth reporting here when three runs were not enough locally.

**It is not strict dominance.** In round 2 the raw arm passed `bench-py-04` and
the governed arm did not. One exception in 39 task-runs, but the layer is not
a pure superset and should not be described as one.

**It does not win by doing less work.** Tool-call volume is the same either way
— 73 raw against 72 governed on average. A single run suggested a 40% saving
and that was noise; the honest reading is that the layer makes the same amount
of work land, not that it shortens the path.

### What this does and does not establish

It answers the objection above: the port helps a model that was already
competent at tool use, roughly two and a half times as many tasks completed.
That is the first evidence in this file that the mechanism has value
independent of rescuing a model that cannot emit a tool call at all.

**It does not satisfy Phase 2's exit criterion.** That gate is about a *local*
model being daily-drivable, and this is a cloud model. The ≥20% figure is
cleared here — +143% relative, +26 points absolute — but against the wrong
subject. On the local 7B the governed arm still passes 0–2 of 13.

**Three runs is still three runs.** The non-overlap is the strongest signal
available, not a confidence interval. A percentage quoted from this table
should carry n=3 with it.

## Where the layer starts paying off: 7B, 14B, frontier

Three models, same 13 tasks, same build, same endpoint. Three runs per arm
except the 7B (four raw, seven governed, accumulated earlier).

| model | raw gate | governed gate | raw acted | governed acted |
| --- | ---: | ---: | ---: | ---: |
| qwen2.5-coder:7b | 0, 0, 0, 0 | 0, 0, 0, 1, 2, 0, 0 | 0 / 13 | 8–11 / 13 |
| **qwen2.5-coder:14b** | **0, 0, 0** | **3, 3, 4** | **0 / 13** | **10–11 / 13** |
| deepseek-v4-flash | 2, 3, 2 | 6, 5, 6 | 13 / 13 | 13 / 13 |

Means: 0.00 → 0.43 (7B), **0.00 → 3.33 (14B)**, 2.33 → 5.67 (cloud).

### The crossover is between 7B and 14B, and it is not where it looks

Both local models fail identically at the transport layer: neither emits a
structured tool call, so both raw arms are a flat zero across every run. The
recovery layer works equally well on both — it gets the 7B acting on 8–11 tasks
and the 14B on 10–11.

What changes between them is whether the recovered edit is *correct*. The 7B
converts that activity into 0.43 passes; the 14B converts almost identical
activity into 3.33. The layer is not what improves; the model's ability to
write a right answer is, and the layer is what lets that ability reach the
workspace at all.

So the value of the port is highest exactly at 14B, where the two failure modes
come apart:

* **7B** — can neither speak the transport nor write the change. The layer
  fixes the first and the second still fails. Passes 0–2 of 13.
* **14B** — cannot speak the transport, *can* often write the change. Without
  the layer it scores zero on every run; with it, 3–4 of 13. This is the case
  the port was built for, and it is the only model measured here whose
  usefulness depends entirely on it.
* **frontier** — speaks the transport natively, so the raw arm works. The layer
  still roughly doubles it (2.33 → 5.67), but by improving a working agent
  rather than enabling a dead one.

### What this means for Phase 2's exit criterion

The criterion is that on the same local model the governed loop measurably
beats the raw loop by ≥20% relative. On the 14B the governed loop beats the raw
loop 3.33 to 0.00, reproducibly, with no overlap and no exceptions.

**But the ratio is undefined, not large.** A baseline of zero makes "≥20%
relative" unmeasurable in the same way it was unmeasurable on the 7B, and this
file should not pretend otherwise. Stated in absolute terms: 0% → 26% of tasks
completed. Whether that satisfies the gate is a judgement about what the gate
was written to mean, and it belongs in the ledger, not here.

**The product goal is clearly not met.** A local model completing 26% of
thirteen small tasks is not daily-drivable, and no reading of the criterion
makes it so.

### What it means for Phase 4's hardware tiers

The roadmap's Fast/Balanced/Strong mapping is currently asserted rather than
measured. This is the first measurement bearing on it, and it says the
Fast tier as specified is not worth shipping: a 7B produces almost nothing
usable even with the whole reliability layer behind it.

Also measured, on the target hardware class (RTX 4070 Laptop, 8 GB VRAM,
32 GB RAM): qwen2.5-coder:14b at Q4_K_M loads as 10 GB and Ollama splits it
38% CPU / 62% GPU at a 4096-token context. It still completed 13 tasks in
about 8 minutes, so the split is tolerable rather than fatal — but it is a
concrete instance of the fit problem Phase 3.5 has to solve, and it says the
memory-fit calculator needs to model partial offload rather than treating
"does not fit in VRAM" as a refusal.

## Sample size

Thirteen tasks, one quantization per model. Four raw and seven governed runs on
the 7B; three per arm on the 14B and on the cloud model.

Every raw arm on a local model is a flat zero in every run, which is solid
precisely because it is degenerate — the model never emits a tool call, so
there is nothing to vary. The governed arms vary: 0–2 on the 7B, 3–4 on the
14B, 5–6 on the cloud model. The 14B and cloud spreads are narrow and do not
overlap their raw arms; the 7B's does not clear zero often enough to mean
anything.

Three runs is still three runs. The separations reported here are large
relative to their spreads, which is the argument for taking them seriously —
not a confidence interval.

## Status

`P9.F1.T4` stays **in-progress**.

Established:

1. The reliability layer converts a local model that produces literally nothing
   into one that acts on most tasks. That holds at both 7B and 14B, and the
   raw arms are zero in every run at both sizes.
2. Whether that activity becomes a *passing* task depends on the model, not the
   layer. 7B: 0.43 of 13. 14B: 3.33 of 13, from near-identical activity.
3. The layer also helps a model that never needed rescuing — 2.33 → 5.67 on a
   frontier cloud model — so its value is not an artifact of broken transport.

Not established, and not close: a local model good enough to depend on. The
best local result here is 26% of thirteen small tasks.

The next questions worth measuring, in order:

1. **A 32B local model.** The 7B→14B step took pass rate from 3% to 26%; the
   shape of the next step decides whether "capable local coding agent" is a
   hardware problem that solves itself or a wall. It needs ~20 GB, which
   exceeds the free RAM on this machine, so it needs either a smaller quant or
   different hardware.
2. **Why the 14B fails the other 9 tasks.** The failure mode has changed since
   the 7B — these are wrong changes, not unparseable ones — and nothing here
   yet says whether context budgeting (Phase 2.4) would move them.
3. **A structured-output path for local models.** Both local models fail the
   same way at the transport layer while being fine at content. Ollama supports
   constrained decoding; forcing tool calls into the schema may be worth more
   than any further tolerance in the parser.

## Reproduction

Local. Substitute `qwen2.5-coder:14b` for the model that actually produces
something:

```
ollama pull qwen2.5-coder:7b        # or qwen2.5-coder:14b
LEGION_BENCH_MODEL=qwen2.5-coder:7b \
  cargo run -p xtask -- legion-bench --mode live-local                   # governed
LEGION_AI_GOVERNORS=off LEGION_BENCH_MODEL=qwen2.5-coder:7b \
  cargo run -p xtask -- legion-bench --mode live-local                   # raw
```

Cloud, the non-degenerate comparison:

```
LEGION_BENCH_MODEL=deepseek-v4-flash:0731-cloud \
  cargo run -p xtask -- legion-bench --mode live-local                   # governed
LEGION_AI_GOVERNORS=off LEGION_BENCH_MODEL=deepseek-v4-flash:0731-cloud \
  cargo run -p xtask -- legion-bench --mode live-local                   # raw
```

The cloud model needs an Ollama account signed in locally and is served through
the same `http://127.0.0.1:11434/v1` endpoint, so Legion needs no configuration
change and no separate API key. The dated tag (`0731`) is pinned deliberately:
`:preview` and `:cloud` move, and a moving model makes a recorded baseline
unreproducible — the predecessor `qwen3-coder:480b` was retired on 2026-07-15
and no longer runs at all.

Diagnosing a run: `LEGION_BENCH_KEEP_CHECKOUTS=1` keeps each task's checkout so
the diff can be read, and rejection reasons are recorded in each task's notes.

Holdout tasks stay excluded; add `--include-holdout` only at a phase exit.
