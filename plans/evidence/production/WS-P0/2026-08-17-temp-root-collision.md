# Test temp-root collision, and what it does not explain

Date: 2026-08-17
Scope: integration-test harnesses in `legion-app` and `legion-project`

## What prompted this

Two `legion-app` tests failed during a four-crate `cargo test` run and passed in
isolation:

- `control_trust_surfaces::dirty_text_preserved_on_rejected_stale_and_conflict_outcomes`
  — `conflict disk: Os { code: 2, kind: NotFound }`
- `workspace_vfs_integration::workspace_vfs_integration_conflicted_registered_save_preserves_dirty_buffer_and_disk`
  — expected `Conflict | Stale`, got neither

Both sit in the conflict-detection family and both read a file back from disk
after an external overwrite, which is the shape of a test whose workspace
someone else is touching.

## What was found

Fifteen test files build their temp workspace root like this:

```rust
format!("legion-app-control-trust-{}-{}",
    std::process::id(),
    SystemTime::now()...as_millis() as u64
        + TEMP_ROOT_COUNTER.fetch_add(1, Ordering::Relaxed))
```

The counter is **added to** a millisecond timestamp rather than being a
component of its own, so two roots collide whenever
`millis_a + counter_a == millis_b + counter_b`.

The collision is real but narrow. `millis + counter` is strictly increasing
whenever the counter order matches the clock order, so a collision needs one
thread to read the clock, be descheduled, and have another thread take a *later*
timestamp with a *lower* counter. Rare — and rare in exactly the way that
produces an occasional unexplained failure rather than a reproducible one.

The second half is what made it invisible: every one of those helpers used
`create_dir_all`, which **succeeds on an existing directory**. Two colliding
tests therefore shared a workspace silently and failed later, somewhere else,
with a confusing message about disk contents.

## What changed

In all fifteen files:

- the counter became its own path component (`…-{pid}-{millis}-{counter}`), which
  removes the collision rather than narrowing it;
- `create_dir_all` became `create_dir`, so a future collision fails immediately
  at the point of collision — `create temp root: AlreadyExists` — instead of
  surfacing as an inexplicable conflict-detection result several hundred lines
  later.

One test also gained a message it should always have had:
`workspace_vfs_integration`'s bare `assert!(matches!(response, Conflict | Stale))`
now prints the response it actually got.

## What this does not claim

**The original two failures were never reproduced.** Attempts, all with no
failure:

| Configuration | Runs | Failures |
| --- | --- | --- |
| `-p legion-app` alone | 10 | 0 |
| `legion-desktop` + `legion-ui` + `legion-app` | 6 | 0 |
| `control_trust_surfaces` with `create_dir` collision probe | 10 | 0 |
| `workspace_vfs_integration` with the same probe | 10 | 0 |
| Four crates, after the fix | 8 | 0 |

An earlier "12 of 12 failed" reading was a broken measurement — the loop matched
on output text rather than the exit code — and is retracted.

So this is a latent harness bug found while investigating, removed on its
merits. It is **not** established that it caused the two observed failures. The
circumstances of those failures point elsewhere as well: both occurred while two
subagents were running their own `cargo` builds against separate target
directories, so the machine was under heavy disk contention at the time.

If the failures recur, the harness is now instrumented to answer the first
question — a collision will say so directly — which is the practical value of
this change regardless of what caused the original two.

## Verification

```
cargo test -p legion-app -p legion-project -p legion-desktop -p legion-ui -j 6   # ×8, 0 failures
cargo clippy -p legion-app -p legion-project --all-targets                        # clean
```

Standing gates green: `verify-kanban-backlog`, `docs-hygiene`, `claim-audit`.
