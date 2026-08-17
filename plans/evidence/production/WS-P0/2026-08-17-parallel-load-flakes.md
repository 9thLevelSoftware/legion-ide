# Parallel-load flakes in the workspace test suite — WS-P0

Two flake families were observed on 2026-08-17 under
`cargo test --workspace --all-targets`, both passing when re-run and both
passing when their binary ran alone.

- **Family A** — four tests in `crates/legion-app/tests/legion_workflow_integration.rs`:
  `workflow_spawn_failure_transfers_launched_worker_into_draining_ownership`,
  `app_drop_hands_draining_workflow_to_global_supervisor_until_reaped`,
  `workflow_worker_error_drains_uninterruptible_sibling_when_failure_is_assigned_first`,
  `workflow_worker_error_drains_uninterruptible_sibling_when_failure_completes_second`.
  Grouped with family B by behaviour; no root cause was proven at the time.
- **Family B** — `terminal_orphan_cleanup_kills_and_records_evidence` in
  `crates/legion-app/tests/terminal_workflow.rs`. Diagnosed: launches
  `cmd /C exit`, sleeps a fixed 400 ms, then asserts the process has already
  exited.

Frequency was unmeasured at the time of that record. It is measured below.

---

## 2026-08-17 — diagnosis, reproduction, and fix

### Summary

Every failure in both families traces to a test asserting a **wall-clock
duration** where the property under test is actually a **happens-before
relationship**. The machine is oversubscribed under
`cargo test --workspace --all-targets`, thread wakeup and process creation
latency rise, and a constant sized near the expected duration is exhausted by
scheduling delay that has nothing to do with the code being tested.

Three findings, in decreasing order of confidence:

1. **Family B's diagnosis is confirmed** and fixed by waiting on the event
   instead of guessing how long it takes. 19/20 → 0/20 under load.
2. **Family A has the same cause and is twice the size it was recorded as** —
   eight tests in that binary, of which the reported four are simply the four
   that fail most often (31–33 out of 40 under load, versus 5–11 for the other
   four). 33/40 → 0/40.
3. **Family A and family B share a cause but not a rate.** Their budgets differ
   5x (2 s vs 400 ms), which is why B trips in ordinary full-suite runs and A
   needs heavier load. They should be treated as one defect class for the
   purpose of fixing them and as two separate observations for the purpose of
   predicting when they appear. See "Are A and B really the same thing?" below.

One test in family A turned out to be **asserting something unmeasurable**
rather than merely racing; that assertion was deleted rather than widened. A
fourth, unrelated flake was found by the verification runs and is recorded but
not fixed.

### Family A — mechanism

Five tests shared a structure with no headroom at all. Each ran a workflow that
must fail fast, wrapped in:

```rust
let started = Instant::now();
let error = app.execute_legion_workflow_with_providers(&session_id, &resolver)…;
assert!(started.elapsed() < Duration::from_secs(2), …);
```

The call being measured **blocks on a rendezvous that was itself budgeted at
2 s** — the test resolver waits `recv_timeout(Duration::from_secs(2))` for the
first worker thread to enter its provider before letting the second worker fail
(`legion_workflow_integration.rs:475`, `:510`) or before panicking
(`:706`). An inner wait equal to the outer deadline it lives inside can never
leave room for the outer assertion; any delay scheduling the sibling worker
thread is charged directly against it.

The failure then surfaces **disguised**. The inner `recv_timeout` expires first
and panics with `first workflow worker must enter before second spawn: Timeout`,
so the workflow returns the wrong error, and the assertion the reader sees is:

```
assertion failed: error.to_string().contains("injected")
```

which reads as a product defect in the spawn-failure path. That is why no cause
was found by inspection: the visible assertion is three steps downstream of the
actual expiry.

Three further sites in the same file failed the same way:

| Site | Constant | What it was really waiting for |
| --- | --- | --- |
| `:816` `wait_timeout_while(…, 5s, …)` | 5 s | the *test's own main thread* to reach the release point |
| `:1489` `lane_barrier_timeout` | 2 s | both lane mates to be scheduled onto a core |
| `:2569` `finished.duration_since(cancelled_at) < 2s` | 2 s | cancellation to unwind rather than block |

One test in this file already carried a comment recording this exact lesson —
"verify the ownership invariant at the point of return instead of comparing
timestamps across independently scheduled threads" — but the insight was applied
to one assertion and not to the seven others around it.

### Family B — mechanism

As diagnosed, and confirmed: `sleep(400 ms)` then assert the child has exited.
400 ms is ample on an idle machine and not ample when process creation is
competing with every other test binary in the workspace.

Worth recording: `terminal_workflow.rs` **already had** the right pattern for
this — `poll_terminal_until`, with a deadline and a sleep backoff, documented as
"PTY output is asynchronous, so a fixed iteration count can race on a loaded
host". The orphan test did not use it.

### Reproduction

Host: Windows 11, i9-14900HX, 24 cores / 32 logical, 32 GB.
`rustc 1.97.1`. All runs use the release-gate command's own defaults
(`--test-threads=32`).

Two load models were used, and the difference between them turned out to matter.

**Model 1 — priority starvation (diagnostic).** 64 always-runnable CPU burners
at Normal priority, test binary demoted to Idle. Windows scheduling is
priority-preemptive, so this is not "loaded", it is "suspended": the process
receives almost no CPU at all. Baseline failed **every** run, which made it a
good instrument for reading the mechanism off the panic messages, and a bad
model of CI. Baseline, unmodified tree (working changes `git stash`-ed):

```
test result: FAILED. 29 passed; 8 failed; 0 ignored; finished in 297.45s
```

The same binary run alone: `37 passed; 0 failed; finished in 0.15s`.

**Model 2 — fair-share oversubscription (representative).** 512 burners at
Normal priority, i.e. ~16x oversubscription with the test binary at equal
priority, so it is competing rather than being preempted. This is the model that
matches CI, and it reproduces the reported flake exactly: an individual run
takes 2–10 s, and the overshoot on the 2 s budget is 3.7–4.9 s — the
right order of magnitude, not the pathological 248 s of model 1.

Harness: `scratchpad/repro.ps1`, `scratchpad/ab.ps1` (not committed — measurement
tools, not gates).

Under model 1, eight tests failed, with the line that actually expired:

| Test | Expiring site |
| --- | --- |
| `workflow_worker_error_drains_uninterruptible_sibling_when_failure_is_assigned_first` | `:707` rendezvous, then `:2785` `elapsed() < 2s` reporting **247.99 s** |
| `workflow_worker_error_drains_uninterruptible_sibling_when_failure_completes_second` | `:2785` |
| `app_drop_hands_draining_workflow_to_global_supervisor_until_reaped` | `:2785` |
| `workflow_spawn_failure_transfers_launched_worker_into_draining_ownership` | `:476` rendezvous → `:2975` wrong-error assertion |
| `workflow_resolver_panic_transfers_launched_worker_into_draining_ownership` | `:511` rendezvous → `:3090` wrong-error assertion |
| `workflow_worker_panic_cancels_and_reaps_blocked_lane_sibling_before_owner_clears` | `:707` → `:2675` ack `Disconnected` |
| `legion_workflow_shared_kill_switch_cancels_inflight_worker_with_fast_ack` | `:2568` ack window, reporting **95.31 s** |
| `legion_workflow_parallel_lane_executes_lane_mates_concurrently_and_delays_dependents` | `:1523` lane barrier timeout |

Two of these — `:818` `release-gated provider was never explicitly released`
(twice) — fired from the 5 s condvar wait, confirming that site independently.

`workflow_resolver_panic_transfers_launched_worker_into_draining_ownership` was
**not** in the reported flake set. It was predicted to fail before the
experiment was run, on the grounds that it carries the identical
zero-headroom structure, and it did.

### Measured frequency, and A/B against the fix

Frequency was previously unmeasured. Under model 2, running the baseline and
fixed binaries **alternately** under one continuous burner set — so neither arm
gets a systematically quieter machine — 40 rounds each:

```
SUMMARY baseline_failures=33/40  fixed_failures=0/40
```

Per-test failure counts across the 40 baseline runs:

| Test | Failures / 40 |
| --- | --- |
| `workflow_worker_error_drains_uninterruptible_sibling_when_failure_is_assigned_first` | 33 |
| `workflow_worker_error_drains_uninterruptible_sibling_when_failure_completes_second` | 32 |
| `app_drop_hands_draining_workflow_to_global_supervisor_until_reaped` | 32 |
| `workflow_spawn_failure_transfers_launched_worker_into_draining_ownership` | 31 |
| `workflow_resolver_panic_transfers_launched_worker_into_draining_ownership` | 11 |
| `legion_workflow_parallel_lane_executes_lane_mates_concurrently_and_delays_dependents` | 11 |
| `legion_workflow_shared_kill_switch_cancels_inflight_worker_with_fast_ack` | 9 |
| `workflow_worker_panic_cancels_and_reaps_blocked_lane_sibling_before_owner_clears` | 5 |

The top four are **exactly** the four originally reported, and they are the four
that fail most often. The other four are the same defect at a lower rate, which
is why the original report saw four and not eight — not because they are a
different problem. **Family A's grouping is confirmed, and the family is
twice the size it was recorded as.**

The fix's own headroom is visible in the same data: the slowest passing run of
the fixed arm took **27.9 s** (round 17). An earlier iteration of this fix used
a 30 s hang limit, which that run would have come within 2 s of exhausting.
That observation is why `RENDEZVOUS_HANG_LIMIT` is 120 s and not 30 s — the
number was set from measurement, not from taste.

### What changed

All changes are in test code. No product code was modified.

`crates/legion-app/tests/legion_workflow_integration.rs`

- The three `started.elapsed() < 2s` assertions are replaced by the signal they
  were a proxy for. The property is "the failure path did not wait for the
  release-gated worker"; that worker sends on `finished` **only** after the test
  releases its condvar, which has not happened at that point, so
  `finished_rx.try_recv() == Err(TryRecvError::Empty)` *is* the property,
  observed directly. This has no dependence on the clock at all and is strictly
  stronger than the old bound, which a broken build could satisfy by failing
  fast for the wrong reason.
- Every remaining wait is a **rendezvous on a signal a correct build always
  sends**, so each is now `RENDEZVOUS_HANG_LIMIT` (30 s), documented as a hang
  detector rather than a performance budget — its job is to fail a genuine
  regression instead of wedging CI, so it is sized ~1000x above expected rather
  than near it.
- The `std::thread::yield_now()` spin loops are replaced by a `poll_until`
  helper that **sleeps** between polls. A yield loop stays runnable and competes
  for the core it is waiting for another thread to be given, so on a saturated
  machine it actively delays the event it is polling for.
- `:2569`'s "fast ack" bound is widened and re-labelled as liveness, not
  latency. See "Not claimed" below.

`crates/legion-app/tests/terminal_workflow.rs`

- The fixed 400 ms sleep is replaced by polling `cleanup_terminal_orphans()`
  until it yields a record. This is safe and non-destructive: underneath it is
  `NativePtyService::cleanup_orphaned_ptys`, a `try_wait`/`GetExitCodeProcess`
  poll that reaps only already-exited processes and is a no-op on a live one
  (`crates/legion-platform/src/lib.rs:2033`). Every call before the child exits
  returns an empty record set and changes nothing, so the loop costs only the
  wait it replaces, and the assertions that follow are unchanged.

### One test was asserting the wrong thing, not racing

`legion_workflow_shared_kill_switch_cancels_inflight_worker_with_fast_ack`
asserted `finished.duration_since(cancelled_at) < Duration::from_secs(2)`. That
assertion has been **deleted**, not widened.

Both instants are captured inside the process being scheduled, so the interval
between them measures how much CPU the host chose to give it. The same
assertion read 95 s under model 1; raising the bound to 30 s produced 41 s.
There is no constant that makes an in-process latency measurement trustworthy
while the process is one of 32 test threads competing for the machine — the
quantity is not a property of the cancellation path at all.

What the test can honestly establish — that cancellation, not completion, is
what ended the run — it already established elsewhere and load-independently:
all workers terminal, the triggering worker `Blocked` or `Cancelled`, and the
`started <= cancelled_at <= finished` ordering (which is retained). The deleted
assertion added no coverage that survives being run on a busy machine.

This is a **reduction in coverage** and is recorded as such under "Not claimed".
A cancellation latency budget is a perf-harness question, on a controlled
machine, where the number would mean something.

### Family B — measured and A/B'd the same way

Same load, same alternating harness, 20 rounds each:

```
SUMMARY baseline_failures=19/20  fixed_failures=0/20
```

Every baseline failure was `terminal_orphan_cleanup_kills_and_records_evidence`
and every one had the diagnosed signature — the 400 ms sleep expiring before the
child did:

```
assertion `left == right` failed: cleanup must return exactly one audit record
for the orphaned session; got: []
  left: 0
 right: 1
```

Independently, on the same day and without synthetic load, this test failed
**twice** in real full-workspace `--all-targets --no-fail-fast` runs on two
different trees — most recently as the single failure out of 3,124 passing tests
across 274 suites — and then passed 3/3 in isolation in 0.43 s each. That is the
same signature as the synthetic result and it establishes the real-world rate
without needing the harness at all.

### Are A and B really the same thing?

They should not be merged carelessly, and there is a genuine asymmetry to
explain: in those two real full-suite runs, family B failed and family A did
**not**. If A and B were the same defect one would naively expect them to fail
together.

The frequency data resolves this, and it resolves it *quantitatively* rather
than by assertion. The two families are the same defect class — a wall-clock
budget standing in for a happens-before relation — but their budgets differ by
5x:

| | Budget | Fails in a real full-suite run | Fails at 16x synthetic oversubscription |
| --- | --- | --- | --- |
| Family B | 400 ms | yes, twice today | 19/20 |
| Family A | 2 s | not observed | 31–33/40 |

A scheduling delay distribution that routinely exceeds 400 ms and rarely exceeds
2 s produces exactly what was observed: B trips under ordinary full-suite load,
A needs more than ordinary load. Both trip once the load is raised. So:

- **Same defect class, same fix, different trigger thresholds.** That is the
  claim the evidence supports.
- **Not "the same flake".** They are not interchangeable in frequency, and a
  reader should not expect A and B to appear or disappear together. The
  coordinator's caution against forcing one explanation is right; what the data
  supports is a shared *cause*, not a shared *rate*.

Family A's original four-test grouping is separately confirmed on its own
evidence — see the frequency table above, where those four sit at the top and
the four additional tests of the same class sit below them.

### Was family B about process-exit observation rather than the sleep?

This was checked specifically, because it would have been the more interesting
finding. It is not what the evidence shows.

`cleanup_terminal_orphans()` bottoms out in `NativePtyService::cleanup_orphaned_ptys`
(`crates/legion-platform/src/lib.rs:2033`), which is a straightforward
`try_wait` (Unix) / `windows_session_exited` (Windows) poll. The observation
mechanism is sound, and the failure signature is `got: []` — zero records, i.e.
the child had **not** exited yet — rather than a wrong or missing record for a
child that had. That is the sleep, not the observation.

One adjacent hazard was found and is **not** the cause but is worth recording:
`native_pty_sessions()` is a process-global registry shared by every
`AppComposition` in the test binary, and `cleanup_orphaned_ptys` reaps every
exited PTY in it, not only the caller's. It does not affect this test —
`cleanup_orphans` filters the returned audit records down to sessions the
calling runtime owns, and no other test in that binary calls cleanup, so nothing
can inflate `records.len()` or reap this test's PTY before it looks. But the
isolation here rests on "no other caller exists", which is a property of the
current test set rather than of the design, and a second test that called
orphan cleanup would break this one.

### Verification

| Command | Result |
| --- | --- |
| `cargo fmt --all` | exit 0, no diff |
| `cargo test --workspace --all-targets --no-fail-fast` run 1 | exit 0 — 267 suites, 3072 passed, **0 failed**, 20 ignored |
| `cargo test --workspace --all-targets --no-fail-fast` run 2 | exit 101 — 267 suites, 3071 passed, **1 failed**, 20 ignored (unrelated `legion-project` race, below) |
| `cargo test --workspace --all-targets --no-fail-fast` run 3 | exit 0 — 267 suites, 3072 passed, **0 failed**, 20 ignored |
| `cargo clippy --workspace --all-targets -- -D warnings` | exit 0 |
| `cargo run -p xtask -- extract-before-modify` | exit 0 — `no chokepoint file grew past its slack` |
| `cargo run -p xtask -- docs-hygiene` | exit 0 — `documentation hygiene checks passed` |
| `cargo run -p xtask -- claim-audit` | exit 0 — `claim audit passed` |
| `cargo run -p xtask -- verify-kanban-backlog` | exit 0 — `kanban backlog ok: 10 epic(s), 41 feature(s), 161 task(s)` |

All builds and test runs used `-j 6`; see the build-failure note under
"Not claimed" for why.

Across all three runs, **every family A and family B test passed**:
`terminal_orphan_cleanup_kills_and_records_evidence` 3/3,
`app_drop_hands_draining_workflow_to_global_supervisor_until_reaped` 3/3, both
`..._drains_uninterruptible_sibling_...` tests 3/3, and
`workflow_spawn_failure_transfers_launched_worker_into_draining_ownership` 3/3.
The only failure in 9,215 test results was the unrelated one below.

### A third flake family, found by the verification runs

Run 2 of the three required full-suite runs failed on a test in a different
crate, untouched by this change:

```
crates\legion-project\tests\path_boundary.rs:558
in_limit_save_uses_stricter_limit_and_writes_once
payload at effective write limit should save: Failed { … diagnostics: [
  "not found for `\\?\C:\…\in-limit.txt` while atomic replace;
   non-atomic fallback disabled; failing closed" ] }
```

It passed in runs 1 and 3. **This one is not a test-side timing assumption.**
The trail:

- `ProjectActor` save calls `FileSystemService::write_text_file_atomic`
  (`crates/legion-platform/src/lib.rs:855`), which creates a uniquely-named temp
  (`atomic_temp_path`, `:446` — `.{stem}.{pid}.{counter}.tmp` with a
  process-global atomic counter, so **no collision between concurrent writers**),
  writes, flushes, `sync_data`s, closes it, then calls `atomic_replace`.
- `atomic_replace` on Windows (`:513`) is `MoveFileExW(REPLACE_EXISTING |
  WRITE_THROUGH)`. The OS error was `ERROR_FILE_NOT_FOUND` — the *temp* it had
  just created and closed was gone by the time it moved it.
- The save path is deliberately fail-closed
  (`NonAtomicSaveFallbackPolicy::Disabled`), so a single transient OS error
  becomes a failed save rather than a retry.

That is the well-known Windows create→close→rename hazard: a filesystem filter
driver (Defender or an indexer) can still hold or transiently remove a
just-closed file. Under a loaded machine the window widens.

So this is a **product** robustness question — should a fail-closed atomic
replace retry transient Windows errors before failing? — not a test defect, and
it is **not fixed here**. Adding retry logic to a security-sensitive
fail-closed write path is its own decision with its own evidence, and doing it
inside a test-hygiene change would be exactly the scope creep this change is
arguing against. Recorded so it is not rediscovered as "the flake came back".

It also makes the point of this whole change concretely: three required runs
surfaced a fourth distinct flake nobody was looking for. A gate that fails at
random is not one problem, it is a habit.

## Not claimed

**For family A, the original CI event was not reproduced; the mechanism was.**
Every family-A failure recorded here was produced under synthetic load on one
Windows workstation. No unaided run of `cargo test --workspace --all-targets`
on this machine reproduced family A, and it did not recur in the two real
full-suite runs that did catch family B. What connects the synthetic
reproduction to the reported event is that under model 2 the failing set is
*exactly* the reported four, at the top of the frequency table, failing with a
2 s budget overshot by 3.7–4.9 s — strong circumstantial agreement, and a
mechanism visible directly in the panic text, but not the same event.

Family B is in a different position: it was caught unaided, twice, in real
full-suite runs, so its diagnosis does not rest on the synthetic harness at all.

**No load level proves absence of flakiness.** 0/40 and 0/20 are measurements at
one load level on one machine, not a proof. Under model 1 (Idle priority against
64 burners — effectively suspension rather than load) an earlier iteration of
this fix still failed 7/37, because the process received almost no CPU and
*every* bounded wait expired regardless of its size. That result stands: at
sufficient starvation, no finite bound survives. The claim being made is
"not flaky under realistic CI oversubscription", not "cannot fail under any
load".

**The 120 s hang limits are still constants.** Three assertions became genuinely
clock-independent (the `elapsed()` family, now `try_recv`) and the poll loops
exit on their condition. But nine rendezvous waits remain bounded by a number,
because there is no earlier signal to wait on than the signal itself. The number
is defended by ratio (120 s against a measured worst passing run of 27.9 s and
an expected value in milliseconds), not by proof.

**Coverage was deliberately reduced in one place.** The cancellation-latency
assertion in
`legion_workflow_shared_kill_switch_cancels_inflight_worker_with_fast_ack` is
deleted. If cancellation becomes slow without becoming wrong, no test now
catches it. The test's name still says "fast_ack" and now overstates what it
verifies; renaming it was left out of scope for this change.

**Windows only.** All runs are on Windows 11 / x86_64-msvc. The changed code has
no OS-conditional logic apart from the terminal test's pre-existing
`#[cfg(windows)]` / `#[cfg(unix)]` command selection, but scheduling behaviour
and process-creation cost differ per platform, and the 3-OS gates have not been
exercised with these changes.

**A genuine regression is now slower to report.** Where a real defect used to
fail in 2 s, it can now take up to 120 s per expiring wait. That cost was
accepted deliberately in exchange for the gate not lying; it is not free.

**Three clean full-suite runs prove very little on their own.** They are
recorded below because they are required, but the baseline passed full-suite
runs too — that is what made this a flake. The load experiments, not the three
green runs, are the evidence that anything changed.

**The suite is not clean.** Run 2 of 3 failed on an unrelated Windows
atomic-replace race in `legion-project` (above). The claim here is that families
A and B are fixed, not that `cargo test --workspace --all-targets` is now
deterministic.

**Unrelated pre-existing build failure observed.** The first
`cargo test --workspace --all-targets --no-run` at `-j 32` failed on
`legion-desktop` test targets with `STATUS_STACK_BUFFER_OVERRUN` inside rustc,
a rustc ICE in `back::link::ensure_removed`, and
`crate 'allocator_api2' required to be available in rlib format, but was not
found in this form`. This is resource exhaustion during highly parallel linking,
not a source defect, and it is unrelated to this change (the failing targets do
not depend on the modified files). It cleared after `cargo clean -p
legion-desktop` and a rebuild at `-j 6`. Recorded because it is a real hazard
for anyone running the workspace gates on a 32-thread machine.
