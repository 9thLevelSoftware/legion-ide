# Legion-Bench raw baseline and raw-vs-governed comparison (P9.F1.T4)

## 2026-08-19: the raw baseline is now frozen, replayable and re-checked

The item this file left open — "re-measure the local models on the repaired
corpus" — is closed for the 14B, and the baseline is no longer a number written
down in a document.

Measured on the current 25-task corpus (20 executed, 5 holdout), suite
fingerprint `bench-suite-v1:e6ae1f88e5dfbca4`, with **qwen2.5-coder:14b**
through Ollama at `http://127.0.0.1:11434/v1` under `LEGION_AI_GOVERNORS=off`:

| | raw | governed |
| --- | ---: | ---: |
| suite gate | **0 / 20** | 4 / 20 |
| `task_success` (edited the right files, cleanly applied) | **0 / 20** | 9 / 20 |
| tasks where the model made any tool call | **0 / 20** | 12 / 20 |
| tool calls | **0 on every task** | 0-5 per task |
| turns | 1 on every task | 1-5 per task |
| cassette-set hash | `sha256:44f1711a…4d9af0352b` | `sha256:9a57f6d8…1b62cb1cd9` |

The shape this file described on the superseded 13-task corpus reproduces
exactly on the repaired one: **the raw arm is a deterministic zero**, for the
transport reason documented below — the model emits no structured tool call, so
an ungoverned provider sees prose and an ended turn.

What is new is that both arms are now **frozen as replayable cassettes**
(`evals/legion-bench/recorded-raw/` and `evals/legion-bench/recorded/`), hashed
into a committed baseline, and replayed and verified on every pull request.
A run no longer needs a model installed, and the raw baseline cannot drift
without a red gate. Every cassette records its arm and a replay refuses a tape
from the other one, so the ungoverned baseline cannot be measured with the
governed loop by accident.

Design, mutation proofs and limits:
`plans/evidence/production/BENCH/recorded-execution-gate-v1.md`.

Still open from the "Next" list below: more runs before any percentage is
quoted, the 7B re-measurement, and the three-transport comparison on the 14B.

---

Date: 2026-08-15. Roadmap Phase 0.6 / Phase 2 exit criterion.
Models: **qwen2.5-coder:7b** (Q4_K_M, 4.7 GB, local), **qwen2.5-coder:14b**
(Q4_K_M, 9.0 GB, local) and **deepseek-v4-flash:0731-cloud** (Ollama Cloud),
all via Ollama 0.32.13 at `http://127.0.0.1:11434/v1`. Suite fingerprint
`bench-suite-v1:020ff34100dd8689` — 25 tasks, 20 executed, 5 holdout excluded
(ADR-0049 holdout policy).

## Correction to the previous version of this file

An earlier revision reported the governed arm passing "0, 0, 2" tasks. **That
figure was wrong.** It came from counting `tests_passed`, which is not the
suite gate and, worse, is inherited rather than earned on four of the thirteen
tasks — see "The metric that was lying" below. Every number in this revision
is computed from the gate the suite actually applies, and the harness now
records the measurement that makes the error impossible to repeat.

## Headline (local 7B, superseded corpus): zero versus non-zero activity

Measured on the 13-task corpus, not the repaired 20-task one. Kept for the
mechanism it documents; the counts should not be quoted.

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

Everything measured on the local models runs against a model that emits **no**
structured tool calls, which makes the raw arm's zero uninformative: "the
reliability layer beats a model that does nothing" is true and nearly vacuous.
The obvious objection is that the layer only rescues a broken driver and would
add nothing to a competent one.

Run against **deepseek-v4-flash:0731-cloud** through the same local endpoint
(Ollama proxying Ollama Cloud), three runs per arm, alternating so any drift in
the service hits both arms equally:

| per run | raw | governed |
| --- | ---: | ---: |
| **suite gate** | 1, 2, 3 | 7, 3, 4 |
| mean | 2.00 / 20 (10%) | **4.67 / 20 (23%)** |
| task_success | 5, 6, 5 | 7, 5, 6 |
| tasks where the model acted | 20, 20, 20 | 20, 20, 20 |
| tool calls | 69, 67, 59 | 66, 95, 74 |

This model calls tools properly — `finish_reason: tool_calls`, the right tool
name, the right argument names — so **both arms act on all 20 tasks**. The
comparison is like-for-like: two working agents, one with the layer and one
without.

The layer roughly doubles the number of tasks clearing the suite gate — the
`task_success` row moves far less, and the two must not be read as one. That
answers the
objection above: it helps a model that was already competent at tool use, not
only one that cannot emit a call at all.

### One task in this run was unwinnable

`bench-rust-04` verified with the whole `cargo test` suite while forbidding
`tests/`, and a test added for `bench-rust-07` in the same fixture was red at
rest. A model could perform its refactor perfectly and still fail. Both arms
ate that zero equally, so the *comparison* holds, but the denominator does
not: these figures are 19 winnable tasks reported as 20.

Fixed after the run — `bench-rust-04` is now scoped to its own surface, and
the corpus-health gate enforces the pass-at-rest direction that let it
through. The numbers above have not been re-taken.

### What this measurement is not

**The ranges overlap.** The governed arm's worst run (3) is matched by the raw
arm's best (3). Three runs each, with that much spread, cannot establish a
separation — only a difference in means that could still be sampling.

**It is not dominance.** In two of the three rounds the raw arm passed a task
the governed arm did not: `bench-py-03` in both, and `bench-py-04` in addition
in one of them. The layer is not a superset of the baseline and must not be
described as one.

**It does not win by doing less work.** Tool-call volume is 65 raw against 78
governed on average: the layer makes more calls, not fewer, and converts them
into completed tasks at a better rate.

### This supersedes an earlier, cleaner-looking result

A previous revision reported 6, 5, 6 against 2, 3, 2 with **non-overlapping**
ranges, and described the governed arm's worst run as beating the raw arm's
best. Those runs were real but the corpus underneath them was not sound: it had
13 executed tasks, two of which (`bench-rust-03`, `bench-rust-05`) passed on
the untouched fixture and so could not distinguish a working agent from a dead
one, and the loop then advertised tools no task's scope granted.

On the repaired 20-task corpus the same comparison is **weaker**: the same
direction, a similar ratio, and none of the crispness. The earlier figure
should not be quoted, and the difference between the two is a fair measure of
how much a small unsound corpus can flatter a result.

## Where the layer starts paying off: 7B, 14B, frontier

**Every figure in this section was measured on the superseded corpus and has
not been re-taken.** It is kept because the *shape* it describes does not
depend on the task count, and removing it would lose the finding. The numbers
themselves should not be quoted.

| model | raw gate | governed gate | raw acted | governed acted |
| --- | ---: | ---: | ---: | ---: |
| qwen2.5-coder:7b | 0, 0, 0, 0 | 0, 0, 0, 1, 2, 0, 0 | 0 / 13 | 8–11 / 13 |
| qwen2.5-coder:14b | 0, 0, 0 | 3, 3, 4 | 0 / 13 | 10–11 / 13 |
| deepseek-v4-flash | *see above* | *see above* | 20 / 20 | 20 / 20 |

The frontier row deliberately carries no numbers: it has current ones in "A
non-degenerate comparison" sixty lines up, and printing the retracted pair here
as well is how a withdrawn figure gets quoted later.

The shape, which stands independently of the counts:

Both local models fail identically at the transport layer. Neither emits a
structured tool call, so both raw arms are a flat zero in every run, and the
recovery layer gets both acting on most tasks. What changes between them is
whether the recovered edit is *correct* — near-identical activity, very
different completion. The layer is not what improves; the model's ability to
write a right answer is, and the layer is what lets that ability reach the
workspace at all.

That makes the port's value highest at 14B, the only model measured whose
usefulness depends entirely on it: zero without, several with. The 7B cannot
write the change either way. The frontier model speaks the transport natively,
so the layer improves a working agent rather than enabling a dead one.

### What it means for Phase 4's hardware tiers

The roadmap's Fast/Balanced/Strong mapping is asserted rather than measured.
This is the first measurement bearing on it, and it says the Fast tier as
specified is not worth shipping: a 7B produces almost nothing usable even with
the whole reliability layer behind it.

Also measured, on the target hardware class (RTX 4070 Laptop, 8 GB VRAM,
32 GB RAM): qwen2.5-coder:14b at Q4_K_M loads as 10 GB and Ollama splits it
38% CPU / 62% GPU at a 4096-token context, still completing a 13-task suite in
about 8 minutes. Tolerable rather than fatal — so the memory-fit calculator
Phase 3.5 owes must model partial offload rather than treating "does not fit in
VRAM" as a refusal.

## Sample size

Three runs per arm on the cloud model against the repaired 20-task corpus. The
local-model figures above are from the superseded 13-task corpus and have not
been re-taken.

The cloud arms' ranges overlap (governed 3-7, raw 1-3), so what is established
is a difference in means on n=3, not a separation. That is weaker than the
previous revision of this file claimed, and the weakening came from repairing
the corpus rather than from any change to the code under test.

## Status

`P9.F1.T4` stays **in-progress**.

Established, on a corpus where every task is now checked to distinguish a
working agent from a dead one:

1. The reliability layer roughly doubles what a competent tool-calling model
   completes: 4.67 of 20 against 2.00 of 20, same direction in all three runs.
2. On local models it is the difference between nothing and something at all —
   both the 7B and the 14B emit no structured tool calls, so their raw arms are
   zero in every run.
3. It is not a superset of the baseline. The raw arm won tasks the governed arm
   did not in two rounds of three.

Not established: any of it to a standard that survives the overlap. Three runs
on 20 tasks with a governed spread of 3-7 supports "better on average" and
nothing sharper.

Next, in order:

1. **More runs before any percentage is quoted.** The overlap is the binding
   constraint now, not the corpus size.
2. **Re-measure the local models** on the repaired corpus. Their figures are
   the ones the Phase 2 exit criterion actually concerns, and they are stale.
3. **Re-run the three transports on the 14B** — native, recovery, and
   schema-constrained — which settles whether constrained decoding helps and
   whether the pre/post scope-fix gap was variance, both of which were raised
   on the old corpus and cannot be answered from it.

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
