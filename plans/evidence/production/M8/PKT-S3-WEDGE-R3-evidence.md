# PKT-S3-WEDGE-R3 — GP-1 s3 rust-analyzer wedge, round 3

Status: **both root causes identified, fixed, and reproduced red→green**
(validation runs recorded below). Branch: `m8/s3-wedge-round-3`.

## Verdict (read this first)

The "s3 silent wedge" was two stacked defects, isolated with a dedicated
reproduction harness (`crates/legion-app/tests/two_ra_stress.rs`) after
brute-force GP-1 looping proved too slow (1 wedge in 38 local runs):

1. **URI drive-designator mismatch — our bug, Windows.** rust-analyzer
   echoes document URIs in its own canonical form: a document opened as
   `file:///C:/…` comes back in `publishDiagnostics` as `file:///c:/…`
   (lowercase drive; lsp-types' `Url` can also percent-encode the colon).
   Three matching sites hashed/compared the RAW string, so the echoed form
   never matched and published diagnostics were dropped as if the server
   were silent: the fingerprint filter every pump uses, the product
   worker-drain ingest (`buffer_id_for_path` lookup — which ALSO dropped
   the leading `/` of Unix absolute paths, breaking that ingest path on
   every platform), and the rename-translation document resolver (every
   Windows rename died with `UnresolvableUri`). Whether a given GP-1 run
   matched depended on a startup race in RA's VFS (which URI form the file
   was known under first) — hence the intermittency.
2. **Cache-priming starvation — RA behavior, all platforms, worst on
   2-core CI.** RA's background prime-caches pass (std indexing) holds
   salsa queries that demand-driven diagnostics block on (`RA_LOG` capture:
   `block_on: … inherent_impls_in_crate(core)` from the
   `fetch_native_diagnostics` thread). Publishes stall for as long as the
   priming pass takes — minutes on 2-core runners, doubled when the product
   lazy-start spawns a second RA that primes the same workspace
   concurrently. Signature: v1 publish arrives, then total silence,
   `buffered_notifications=0`, empty stderr ring, nudges useless — exactly
   CI runs 28752070723 / 28759076132. The GP-1 probe now disables priming
   (`cachePriming.enable: false`): a probe needs no warm caches, and
   demand-driven analysis of the fixture is ~300 ms per error→clear cycle.

A third, rarer mode — the probe RA process dying outright (capture 1,
broken-pipe nudge write) — remains unexplained but is now fully observable:
reader terminal event, child-alive bit, exit status, and both stderr rings
are dumped on every s3 failure path, and the product session detects
transport death and routes it through the restart circuit breaker instead
of projecting Live forever.

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
| Loop 3 (12 runs, concurrent release build) | local Windows | 89f5f53 | 12/12 green |
| Loop 4 (12 runs, sustained build load) | local Windows | 04d50c3 | 12/12 green |
| CI dispatches 28760091635 / 28760188624 | 3-OS | 04d50c3 | green |

## Harness-driven isolation (how the causes were found)

| Experiment | Result | Conclusion |
| --- | --- | --- |
| Dual-session harness, 30 s deadline | wedge at cycle 1, deterministic | reproducible at will (vs 1/38 GP-1 runs) |
| Solo control (`STRESS_SOLO=1`) | wedge at cycle 1 | two-RA topology NOT required |
| Direct exe run (no cargo wrapper) | identical wedge, byte-identical frames | env/lock inheritance ruled out |
| git-init parity | still wedges | git state ruled out |
| `cachePriming.enable=false` (pre-URI-fix) | still wedges | priming not the (only) cause |
| 300 s deadline (pre-URI-fix) | still "no diagnostics" | not mere slowness → looked deeper |
| Buffered-notification dump | **error diagnostic WAS buffered** under `uri_hash=ca1e1187…` | server not silent — matching broken |
| Candidate-hash forensics | RA's hash = lowercase-drive form of our URI | **root cause #1 confirmed** |
| Fingerprint fix, priming on | cycle 1 ok, wedge at cycle 2, `buffered=0`, reader+child alive, stderr empty | the true silence mode isolated (ubuntu CI signature) |
| `RA_LOG=info` capture | `fetch_native_diagnostics … block_on … inherent_impls_in_crate(core)` while priming churns | **root cause #2 confirmed** |
| Fingerprint fix + priming off | **10/10 cycles in 3.4 s** | both fixes verified red→green |

## Fixes landed (all TDD, red→green)

| Commit | Fix |
| --- | --- |
| 13fe8df | `normalize_file_uri_drive` + fingerprint normalization (legion-lsp, 4 tests); ingest `uri_to_canonical_path` drive/`%3A`/leading-slash fixes (3 tests); `AppDocumentResolver` key+lookup normalization (2 tests); translate.rs drive normalization with `%3A`-before-drive-check ordering fix (1 test); GP-1 probe `cachePriming.enable=false`; reproduction harness committed as `#[ignore]` test |

## Validation

- Local loop 5 (12 GP-1 runs at 13fe8df, all fixes): **12/12 green**;
  s3 initial publish matched at 762 ms on run 1.
- CI `legion-smoke.yml` on the branch at 13fe8df: recorded in the PR
  (3-OS matrix on the PR itself is the enforcement point).
- Note: `pump_until` already returns `PumpOutcome::Closed` the moment the
  reader records a terminal event, so a dead transport fails pumps fast;
  the remaining nudge iterations against a dead pipe fail in milliseconds
  and the write-failure path dumps the post-mortem.

## Residual risks / follow-ups

1. **Capture-1 process death** (Windows local, once): unexplained; now
   fully observable on recurrence (exit status + both stderr channels).
   Watch-list item.
2. **Product session on real workspaces (M9)**: initializes WITHOUT
   `files.watcher: client` and WITH cache priming. Priming is the right
   default for interactive use, but the M9 planning pass must weigh the
   notify-watcher wedge exposure (round 2) and first-diagnostic latency on
   cold starts; the product session's diagnostics UX should not assume
   publishes within seconds of didOpen on large workspaces.
3. **translate.rs path form**: output keeps URI forward slashes (documented
   in the fn); Windows consumers converting to editor canonical form must
   handle `/`→`\` at apply time — M9 apply-activation (P3.F1.T2) checklist
   item.
