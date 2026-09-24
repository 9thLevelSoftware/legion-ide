# Recorded provider cassettes

One JSON file per corpus task, plus `baseline.toml`.

A cassette is the **model's half** of one task's conversation: the ordered
chat-completion responses a real model returned when the task was run live.
Everything else in a recorded run is real — the fixture is copied into a fresh
checkout, the delegated-task loop dispatches real tools against it, proposals
go through the real proposal and save pipelines, and the task's own
verification command decides whether the tests pass.

That split is the point. Recorded mode measures the product against a fixed
model, so a change in the numbers is a change in the product.

## Files

| File | What it is |
| --- | --- |
| `<task-id>.json` | Ordered model responses for that task, with a fingerprint of the request each one answered |
| `baseline.toml` | Provenance (model, arm, endpoint, corpus fingerprint, cassette-set hash) plus the expected per-task result |

## How a run uses them

```
cargo run -p xtask -- legion-bench --mode recorded   # execute + measure
cargo run -p xtask -- verify-legion-bench            # compare against baseline.toml
```

The first command refuses to run if the cassette files no longer hash to
`baseline.toml`'s `cassette_set_hash`. The second fails if any task's measured
status, score, `tests_passed`, `diff_files`, `turns`, `task_success`,
`tool_calls`, `duplicate_tool_calls`, `retries` or `cassette_drift` differs
from the committed expectation.

`cassette_drift` counts replayed exchanges whose request no longer matches the
one that was recorded, after normalizing out the task's temp checkout path and
every UUID (a fresh proposal id per edit would otherwise make every
post-edit request differ on a value the model does not read).

The baseline pins the value **per task**, not globally at zero: 23 of the 25
rows are 0, and two — `bench-rust-04` and `bench-rust-08`, the two tasks whose
tape contains more than one `edit-as-proposal` call — sit at 3 and 2. The
residual source was not isolated. What the gate needs is that the number is
*stable*, and it is: an independent replay reproduces the baseline exactly,
including those two values. Treat a change in any of them as what it is — the
loop asking the model something it did not ask when the tape was cut.

## Re-recording

Needed when the corpus changes, when the agent's request shape changes
(non-zero drift), or when moving to a different reference model. Requires a
local OpenAI-compatible endpoint serving the model.

```
LEGION_BENCH_MODEL=qwen2.5-coder:14b cargo run -p xtask -- legion-bench --mode record
cargo run -p xtask -- legion-bench --mode recorded --write-baseline
```

Re-record and re-baseline as one change, and say in the commit message which
of the three reasons above applies. A baseline refreshed to make a red gate
green is a deleted regression test.

## Arms

Every cassette records the `LEGION_AI_GOVERNORS` arm it was cut under, and a
replay refuses a tape from the other arm. The default set here is `governed`
(the product's shipping behaviour). The frozen ungoverned baseline lives in
`../recorded-raw/` and is described in
`plans/evidence/production/BENCH/baseline-raw-v1.md`.
