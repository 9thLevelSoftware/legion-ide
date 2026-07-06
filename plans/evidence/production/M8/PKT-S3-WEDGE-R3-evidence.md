# PKT-S3-WEDGE-R3 — GP-1 s3 rust-analyzer wedge, round 3

Status: investigation in progress (this file is updated as evidence lands).
Branch: `m8/s3-wedge-round-3`.

## Problem statement

GP-1 s3 intermittently times out waiting for diagnostics after `didChange`:
the `didOpen` v1 publish arrives (~1–2 s), then **zero notifications for any
URI for 120 s+**. Round 1 added post-mortem instrumentation
(`buffered_notifications=0`, health Fresh) and nudges (PR #44 — proven
ineffective: all three nudges ignored on both wedged CI runs). Round 2
captured a rust-analyzer notify-watcher failure on temp paths in the stderr
ring and fixed it with `files.watcher: "client"` + `didChangeWatchedFiles`
dynamic registration (PR #47). The wedge then **recurred with the fix in
place and an empty stderr ring** — locally and on CI (run 28759076132,
ubuntu): the notify failure was *a* wedge mode, not *the* wedge mode.

## Round-3 phase-1 findings (code-verified, before any new capture)

Two candidate mechanisms both turned out to be real:

### (a) Two rust-analyzer processes per GP-1 run (topology, confirmed in code)

- GP-1 opens its temp workspace as **Trusted** and the fixture has a
  `Cargo.toml`.
- s3 opens `scratchpad.rs` through the product path (`app.open_file`), which
  fires the PKT-LSP-C lazy-start trigger (`bind_opened_file` →
  `try_start_lsp_session_for_current_workspace`) → the product session
  spawns a **second real rust-analyzer** (PATH discovery) against the same
  temp workspace, concurrent with the standalone probe session from s2.
- The product session's `startup_session` initializes with plain
  `initialize(root_uri)` — **without** `files.watcher: "client"` — so the
  second RA is exposed to the round-2 notify wedge on the same temp path,
  and shares the workspace `target/` directory with the probe RA.
- Before PKT-LSP-C the start was eager (trusted-workspace open), so the
  two-RA topology existed in every earlier wedged run as well — consistent
  history, though not discriminating on its own.

### (b) Reader-thread death was silently unobservable (transport, confirmed in code)

- The stdout reader thread exits on the first EOF or framing/parse error.
- `try_recv_envelope` mapped both `Disconnected` and `Eof` to `None`;
  `try_drain_diagnostic_params`'s `while let Some(Ok(...))` dropped terminal
  `Err` events entirely.
- Consequence: a dead reader — or a **dead rust-analyzer** — was
  indistinguishable from a healthy-but-quiet server, forever: zero
  notifications, writes still succeeding into the pipe buffer, empty stderr
  ring. This is exactly the observed wedge signature, and it is also a
  product defect: the product session projected Live indefinitely after an
  RA crash outside an in-flight request.

## Round-3 instrumentation and fixes landed on the branch

| Commit | Change |
| --- | --- |
| 16773c1 | legion-lsp reader stats: frames/bytes counters + terminal-event slot; `mock.malformedFrame` arm (child alive, reader dead); 3 red→green contract tests |
| 2a3bc0f | Product transport-death detection: worker idle poll checks reader terminal → `LspWorkerResult::TransportDead` (redacted reason) once → worker exits; `try_drain_results` intercepts → T3 circuit breaker (BackingOff/auto-restart), not silent-Live; 2 red→green tests |
| e513ffb | GP-1 s3 post-mortem dumps reader stats + child liveness + product-session status/health/stderr projections; topology line on every run; clear-pump failure path now dumps too |
| 89f5f53 | All four s3 `did_change` failure paths dump the post-mortem before failing (capture-1 was lost to a `?` early-return); nudge comment over-claim fixed |
| 04d50c3 | Child exit status collected in the post-mortem (panic 101 vs signal vs clean 0 discriminate death modes) |

## Captures

### Capture 1 — local Windows, run 6 of loop 1 (2026-07-05, commit e513ffb)

`target/s3-repro/capture-1-run06-broken-pipe.log`:

- s2 init 53 ms; product session `lifecycle=Starting` after `open_file`
  (two-RA topology live).
- v1 publish at 710 ms; `didChange`(error) write **succeeded** (~0.7 s).
- 30 s pump: silence. First nudge write failed:
  `LSP stdio I/O failed: write frame: The pipe is being closed. (os error 232)`.

**Finding: the standalone rust-analyzer process DIED between ~0.7 s and
~31 s.** At least one wedge mode is process death, not a stalled analysis
queue (the pre-round-3 comment claiming "the stall is inside RA's analysis
queue, not our transport" was an over-claim — reader state was never
measured before this packet). The post-mortem did not run on this capture
(fixed in 89f5f53).

### CI signature (run 28759076132, ubuntu, main post-#47, pre-round-3 instrumentation)

- v1 publish 1.8 s; didChange + 3 nudges all **succeeded** (no pipe error);
  `buffered_notifications=0`; stderr ring **empty (0 lines)**.
- Distinction from capture 1: nudge writes succeeding is compatible with
  either an alive-but-silent RA or a dead RA whose pipe buffer absorbed the
  small writes (platform-dependent). Round-3 instrumentation (reader
  terminal + child_running + exit_status) resolves this ambiguity on the
  next wedged CI run.

## Reproduction log

| Attempt | Channel | Commit | Result |
| --- | --- | --- | --- |
| Loop 1 (8 runs max) | local Windows | e513ffb | wedge on run 6 (capture 1, post-mortem lost) |
| Loop 2 (8 runs) | local Windows | 89f5f53 | 8/8 green |
| CI dispatch 28759864236 | 3-OS | e513ffb | green |
| CI dispatch 28759998401 | 3-OS | 89f5f53 | green |
| CI dispatch 28760091635 | 3-OS | 04d50c3 | in flight |
| Loop 3 (12 runs, concurrent release build load) | local Windows | 04d50c3 | in flight |

## Open questions the next capture answers

1. Reader terminal event: `Eof` (RA died/closed stdout) vs `Error(..)`
   (our framing/parse bug) vs `None` (RA alive, truly silent).
2. `child_running` / `exit_status`: RA death mode (panic 101, signal,
   clean 0).
3. Product-session state + stderr at wedge time: does the second RA hit the
   notify error (it has no watcher=client), and does its activity correlate
   with the probe RA's failure (shared `target/` contention)?
