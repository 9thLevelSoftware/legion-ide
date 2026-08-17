# Two flake families that only appear under full parallel load

Date: 2026-08-17. Found while integrating the wave-2 branches; neither is
introduced by that work.

## What happened

`cargo test --workspace --all-targets --no-fail-fast` failed with four failures:

```
workflow_spawn_failure_transfers_launched_worker_into_draining_ownership
app_drop_hands_draining_workflow_to_global_supervisor_until_reaped
workflow_worker_error_drains_uninterruptible_sibling_when_failure_is_assigned_first
workflow_worker_error_drains_uninterruptible_sibling_when_failure_completes_second
```

The same suite run alone: `cargo test -p legion-app --test legion_workflow_integration`
— **37 passed, 0 failed**. The same full command re-run on the same tree —
**3,087 passed, 0 failed**. Nothing changed between the two runs.

A second, independent case was found the same day by the viewport workstream:
`terminal_orphan_cleanup_kills_and_records_evidence` failed once under
`--all-targets`, passed standalone both with and without that change applied,
and passed on re-run. Its diagnosis is concrete and worth carrying forward: the
test launches `cmd /C exit`, sleeps a fixed 400 ms, then asserts the process has
already exited. Under parallel load 400 ms is not enough.

Both families are process-supervision tests — spawning, draining, reaping — and
both assert on timing rather than on a signal.

## Why this is worth a task rather than a shrug

A gate that fails at random is worse than a slow gate, because the first
response to a red run stops being "what broke" and becomes "run it again". This
project already has a documented instance of the failure mode one step further
along: `perf-harness --strict` spent an unknown period exiting 0 while its only
budgeted workload silently never ran, and the green was believed because nobody
had reason to look. A flaky suite trains people into exactly the habit that
made that possible.

It also costs real time here. The 3-OS standing gates take 15-30 minutes per
platform; a spurious failure on one of them is an hour of wall clock and a
push-to-retrigger.

## What is not claimed

**No fix is attempted and no root cause is proven for the four workflow tests.**
The terminal one has a specific diagnosis (fixed sleep); the four here are
grouped with it by behaviour — timing-dependent, parallel-load-only,
supervision-related — not by shared evidence. Someone should confirm rather than
inherit that grouping.

The correct fix is almost certainly synchronisation — waiting on a signal the
supervisor actually emits — rather than a larger constant. A bigger sleep moves
the failure rate without removing it and makes the suite slower on every run.

Frequency is unmeasured. Two occurrences in one day of heavy use is enough to
act on and not enough to quantify.
