# Frozen raw (ungoverned) cassettes

The P9.F1.T4 baseline: the reference local model driving the delegated-task
loop with **every SmallCode-derived governor disabled**.

Same format and same machinery as `../recorded/` — see that README first. The
only differences are the arm and what the arm implies.

## The arm

`LEGION_AI_GOVERNORS=off` is the tested seam (`crates/legion-agent/tests/
governor_ab_seam.rs`) that turns off tolerant tool-call recovery, fragment
resolution, the loop governors, and the governed edit schema — the whole port,
including the parts that are not visible in a transcript.

Every cassette here records `"arm": "raw"`, and a replay refuses a tape whose
arm does not match the process it is running in. Replaying these tapes
therefore *requires* `LEGION_AI_GOVERNORS=off`:

```
LEGION_AI_GOVERNORS=off cargo run -p xtask -- legion-bench --mode recorded \
  --cassettes evals/legion-bench/recorded-raw --out target/legion-bench-raw
LEGION_AI_GOVERNORS=off cargo run -p xtask -- verify-legion-bench \
  --cassettes evals/legion-bench/recorded-raw --out target/legion-bench-raw
```

Without the variable the run fails on the first task with an explicit
cross-arm error rather than quietly measuring the governed loop and calling it
the baseline.

## What the baseline says

Zero. Not "low" — zero on every task: one turn, no tool calls, no proposals,
no file changed. The model writes its tool call as bare JSON in the message
content and reports `finish_reason: "stop"`, and an ungoverned provider sees
prose and an ended turn.

That is the number the governed arm is measured against, and it is exactly why
it must be frozen rather than re-derived: a baseline of zero is the easiest
number in the world to accidentally improve.

The full analysis, including the comparison against the governed arm, is in
`plans/evidence/production/BENCH/baseline-raw-v1.md`.
