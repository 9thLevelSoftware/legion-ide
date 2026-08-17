# PR-UI-001 promotion verification — 2026-08-16

Roadmap item 1.10. Backlog task `P8.F4.T1`. Readiness row `PR-UI-001` in
`plans/product-readiness-ledger.md`.

- Tree: worktree `agent-a5483447ba1f2d4fd`, commit `7609c7752ad395fb9818d04f00c9c866d70c66d7`.
- Host: Windows 11 Pro 10.0.26200, single OS. macOS and Linux legs cannot be
  produced from this machine and are recorded as not obtainable here, not
  inferred.

## Verdict

**`PR-UI-001` cannot be promoted. It stays `Substrate validated`.**

Neither half of the roadmap 1.10 requirement is met, and one of the two is worse
than "incomplete": `xtask perf-harness --strict` currently reports success while
the only workload carrying ADR-0048 budgets does not run at all. The exact
blocking list is in "What is missing" below.

Two things did move forward. The accessibility contradiction is resolved, and
resolving it produced the **first Windows OS accessibility-tree observation for
this product**, now repeatable from a committed probe — one of the three legs the
row needs. And the perf harness's true gating state is now measured rather than
assumed, including two ADR-0048 budget misses on the 100MB workload that no gate
was watching.

---

## Part 1 — The accessibility-evidence contradiction

### The two claims

| Document | Claim |
| --- | --- |
| `plans/evidence/accessibility/README.md:5` (before this repair) | "Product-level accessibility evidence: passed." — no platform or reproducibility qualifier. `:16` cites "OS accessibility-tree inspection for the product window". |
| `plans/evidence/gui-productization/phase-7-known-limitations.md:18` | "Projection accessibility metadata is available; OS accessibility tree inspection remains not observed in the current smoke evidence." |

### How the contradiction was resolved

By running the only accessibility harnesses that exist in this tree, and by
reading the code that produces the disputed status string — not by deciding
which document sounded more plausible.

| Check | Command / method | Exit | Result |
| --- | --- | --- | --- |
| In-process AccessKit/egui projection coverage on Windows | `cargo test -p legion-desktop --test accessibility` | 0 | 11 passed, 0 failed, 0 ignored, 0.86s |
| Whole-workspace suite (contains the same accessibility targets) | `cargo test --workspace --all-targets --no-fail-fast` | 0 | 2861 passed, 0 failed, 17 ignored, 253 suites |
| Could any command in this repo observe an **OS** accessibility tree, before this change? | `rg` over `xtask/src`, `scripts/`, `.github/workflows/`, `crates/legion-desktop`; full `Commands` enum in `xtask/src/main.rs` | — | **No.** No xtask subcommand, script, test, or CI job inspected an OS accessibility tree on any platform. `rg -i accessib .github/workflows/` returns zero matches, still. |
| Was the macOS probe ever committed? | `git log -S"AXUIElement" --all`; `git log --all --diff-filter=A -- "*.swift"` | — | Both empty. `AXIsProcessTrusted` occurs in exactly one file in the repo, and that file is a markdown evidence document, not source. |

### The decisive finding

`accessibility_tree_status` in `crates/legion-desktop/src/platform.rs:214-220`
cannot report anything else:

```rust
fn accessibility_tree_status(node_count: usize) -> String {
    if node_count == 0 {
        NOT_OBSERVED.to_string()
    } else {
        format!("metadata-only projection accessibility nodes {node_count}; OS tree not observed")
    }
}
```

`crates/legion-desktop/src/smoke.rs:365` likewise hardcodes
`accessibility_smoke: NOT_OBSERVED`. Two tests
(`crates/legion-desktop/tests/platform_smoke.rs:142-144`,
`crates/legion-desktop/tests/platform_integration.rs:37-38`) assert the literal
string `"OS tree not observed"`, locking the behaviour in. There is no
`--dump-a11y`-style flag on `legion-desktop`; the accepted flag list is closed at
`crates/legion-desktop/src/workflow.rs:211`.

### The re-run that settles it, and a result neither document predicted

Rather than stop at "no tooling exists", a Windows UI Automation probe was
written and run against a live product window. It is committed at
`scripts/a11y-uia-walk.ps1` so this observation is repeatable; the raw output is
at `plans/evidence/production/PR-UI-001/2026-08-16-windows-uia-tree.txt`.

Procedure: launch `cargo run -p legion-desktop -- --smoke --workspace . --file Cargo.toml --duration-ms 45000`,
then walk the running process's UIA tree while the window is up.

**What the repo's own smoke harness wrote about that exact run:**

```
focus_smoke: os-observed not focused
high_dpi_smoke: os-observed scale 1.500
accessibility_smoke: not observed
accessibility_tree_smoke: metadata-only projection accessibility nodes 5; OS tree not observed
accessibility_projection_node_count: 5
```

**What Windows UI Automation saw in the same process at the same time:**

```
PROCESS_FOUND: 1 pid(s): 26368
PID 26368: top-level UIA windows = 2
WINDOW name='Legion IDE Smoke' controlType='ControlType.Window' className='Window Class' isEnabled=True hasKeyboardFocus=True
  [1] ControlType.Button name='Manual'
  [1] ControlType.Button name='Assist'
  [1] ControlType.Button name='Delegate'
  [1] ControlType.Button name='Legion Workflows'
  [1] ControlType.Button name='Command'
  [1] ControlType.StatusBar name=''
  [1] ControlType.Button name='Explorer drawer'
  [1] ControlType.Button name='Bottom panel drawer'
  [1] ControlType.Button name='TERMINAL'
  [1] ControlType.Button name='PROBLEMS (0)'
  [1] ControlType.TabItem name='Cargo.toml'
  [1] ControlType.Button name='Close Cargo.toml'
  [1] ControlType.Text name='Ln 1, Col 1'
  ... editor line numbers and buffer text as ControlType.Text ...
DESCENDANTS_ENUMERATED: 138
UIA_WALK_OK
```

**The product does publish an OS accessibility tree on Windows** — 138
descendants, with real `Button`, `TabItem`, `StatusBar`, and `Text` control types
carrying product labels. AccessKit + egui + winit is working. Nobody had ever
looked.

This is the first Windows OS accessibility-tree observation for Legion IDE, and
it reframes the dispute. The gap is in the **evidence tooling**, not in the
**capability**: the harness reports "OS tree not observed" because it is
structurally unable to report anything else, so every document that quotes it
inherits a statement that understates the product.

### Verdict on the contradiction

**`phase-7-known-limitations.md:18` is correct. `accessibility/README.md` was
overclaiming.** Both statements can be literally true at once only because they
are scoped differently, but the README presented single-host, one-off,
non-reproducible evidence as an unqualified pass. Specifically:

1. The OS accessibility-tree observation in
   `plans/evidence/production/M5/WS18-T2-accesskit-product-pass.md` is **macOS
   only** (`AXIsProcessTrusted`, `AXStandardWindow`, `AXCloseButton`), and the
   file never names its host OS — which
   `plans/evidence/production/WS-MANUAL-01/WS-MANUAL-01-evidence.md:74` requires
   of exactly this kind of evidence.
2. It is **not reproducible**. The probe source was never committed, so the
   observation cannot be repeated by anyone, on any machine, including the
   machine that made it. The text entered the tree fully formed inside the
   squash commit `9cfa206` (2026-06-13, "feat: complete Legion production master
   plan") that added the whole `plans/evidence/production/M5/` set at once. There
   is no separate commit in which the probe was run.
3. Windows and Linux OS accessibility trees had **never** been inspected before
   today. The Windows leg is now captured and repeatable (see above). Linux
   remains uninspected.
4. The GP walkthroughs are **not screen-reader transcripts**. `gp-1-manual-walkthrough.md`
   presents quoted "VoiceOver" utterances that are the `AXStaticText` values from
   the macOS dump, in the same order. `WS18-T2-accesskit-product-pass.md` says so
   itself under "Residual risk": "Screen-reader end-to-end automation for NVDA,
   VoiceOver, and Orca still needs dedicated host-specific runs."

### Repair applied

`plans/evidence/accessibility/README.md` was edited to state the per-OS status
separately (Windows repeatable, macOS unreproducible, Linux absent), to relabel
the walkthroughs as label inventories rather than screen-reader transcripts, to
record why the smoke harness's "OS tree not observed" line is a harness
limitation rather than a product fact, and to state that the acceptance condition
is not met.

`plans/evidence/gui-productization/phase-7-known-limitations.md` was **not**
edited. Its entry is a statement about the smoke evidence, and it remains true of
the smoke evidence — the smoke run taken today still wrote "OS tree not observed"
while UIA was reading 138 nodes out of the same process. Whether to broaden that
entry now that a Windows observation exists is left to its owner.

---

## Part 2 — True gating state of `xtask perf-harness`

ADR-0048 budgets (`plans/adrs/ADR-0048-renderer-strategy.md:30-31`): keypress
p50 < 16 ms, keypress p95 < 32 ms, scroll p95 < 32 ms.

`--strict` is already the default (`xtask/src/main.rs:564`, `default_value_t = true`);
`--no-strict` opts out. So the roadmap's "`perf-harness --strict` ... not
report-only" is not a missing flag. The problem is what the flag is allowed to
gate.

### What the harness actually ran, 2026-08-16

`cargo run -p xtask -- perf-harness` → **exit 0**, `strict=true`,
`total=5 passed=3 failed=0 skipped=2`.

| Workload | Budget | Status today | Gated? |
| --- | --- | --- | --- |
| `m0.input_to_paint_microbenchmark` | 250 ms | passed (total 5 ms) | Yes — but 250 ms is not an ADR-0048 number, and the workload is 32 edits against a 64 KB in-memory byte buffer, not the editor. |
| `m1.line_galley_shaping_cache` | 2 ms | passed (0 ms) | Yes. Synthetic 10K-line galley lookup. |
| `m2.memory_ceiling_1mb` | 0 | passed (4.05 MB vs 10 MB) | Byte-based assertion; `budget_millis = 0` by construction. |
| `m8.search_stream_50k` | 0 | **skipped** — "full scan 21436ms (5000 hits in 50000 files); cancellation latency 3782ms" | **No.** Report-only by construction (`SEARCH_STREAM_50K_BUDGET_MILLIS = 0`). |
| `manual.renderer_input_to_paint` | 32 ms | **skipped** — "desktop build failed" | **No. It did not run.** This is the only workload carrying ADR-0048 budgets. |

`cargo run -p xtask -- verify-perf-harness` → **exit 0**, `strict=true`, same
`3 passed / 0 failed / 2 skipped`. The verify gate re-reads the report and
inherits the same blind spot.

### Finding 2A (critical) — the ADR-0048 gate is fail-open, and it is currently failing open

`run_renderer_backed_manual_measurement` (`xtask/src/perf_harness.rs:1012`)
spawns:

```
cargo run --release -p legion-desktop --no-default-features --features offline -- --manual-perf ...
```

That subprocess exited **101** today. The harness classified the output through
`manual_renderer_build_failed` (`xtask/src/perf_harness.rs:1161`) and downgraded
the measurement to `Skipped`, so `summary.failed` stayed 0 and strict mode
returned 0. The behaviour is deliberate and locked in by a unit test
(`xtask/src/main.rs:5494`, "PKT-0 Task 2: perf-harness build-failure heuristic").

**Consequence: any compile break in `legion-desktop` silently converts the only
ADR-0048-budgeted workload into a passing strict run.** No output distinguishes
"budgets met" from "budgets never measured" other than the word `skipped` in a
line the exit code does not reflect.

**Root cause of today's break**, reproduced directly:

| Check | Command | Exit |
| --- | --- | --- |
| The exact configuration the harness spawns | `cargo build --release -p legion-desktop --no-default-features --features offline` | **101** |

```
error[E0425]: cannot find function `render_streaming_assistant_rows` in this scope
    --> crates\legion-desktop\src\view.rs:6998:17
error: could not compile `legion-desktop` (lib) due to 1 previous error
```

`crates/legion-desktop/src/view.rs:37-42` re-exports `render_streaming_assistant_rows`
under `#[cfg(feature = "ai")]`, but the call site at `view.rs:6998` is ungated.
`--no-default-features` drops `ai`, so the symbol is absent.

This is the same defect class as the update-drill break diagnosed on 2026-08-15
(`plans/evidence/production/WS-P0/2026-08-15-hosted-smoke-first-run.md`,
"Failure 1"). That one was fixed and the configuration was added to
`.github/workflows/legion-gates.yml` as `cargo check -p legion-app --no-default-features`
so it could not silently regress. **The `legion-desktop --no-default-features
--features offline` configuration was never added to that guard**, so it
regressed the same way, and the perf harness absorbed the regression instead of
reporting it.

No product-code fix was applied here. Gating the call site out would silently
remove the streaming rail from offline builds, which is a product decision, not a
verification one.

### Finding 2A-bis — what the ADR-0048 workload reports when it is actually run

To find out what the harness *would* have measured, the same workload was run
out-of-band with **default** features, which do compile. No product code was
changed to do this.

`cargo run --release -p legion-desktop -- --manual-perf --workspace . --file Cargo.toml --perf-samples 16`

Seven runs. Runs 2-5 reuse the built binary; runs 6-7 relink first, reproducing
the harness's own "build then immediately measure" shape.

| Run | keypress p50 | keypress p95 | scroll p95 | report `status` |
| --- | ---: | ---: | ---: | --- |
| 1 (first after fresh compile) | 11.05 ms | **36.32 ms** | 10.01 ms | **failed** |
| 2 | 3.50 ms | 13.75 ms | 2.66 ms | passed |
| 3 | 3.47 ms | 14.18 ms | 2.71 ms | passed |
| 4 | 3.58 ms | 15.08 ms | 2.92 ms | passed |
| 5 | 4.24 ms | 15.81 ms | 4.36 ms | passed |
| 6 (relinked first) | 4.14 ms | 19.12 ms | 5.31 ms | passed |
| 7 (relinked first) | 3.66 ms | 15.67 ms | 2.88 ms | passed |

Budgets: p50 16 ms, p95 32 ms, scroll p95 32 ms.

Read this carefully, because it cuts both ways:

- **The budgets are not systematically missed.** Six of seven runs sit
  comfortably inside all three budgets — p50 around 3.5-4.2 ms, p95 around
  13.7-19.1 ms. It would be wrong to report "ADR-0048 fails" on this workload.
- **But the gate is not stable either.** One run in seven exceeded the 32 ms
  keypress p95, and `manual_renderer_perf_measurement` maps a `failed` report to
  `SkeletonStatus::Failed`, so that run *would* have made `perf-harness --strict`
  exit 1. At n=7 that is roughly a one-in-seven flake against a gate that is
  meant to be strict. The attempt to reproduce it by relinking first (runs 6-7)
  did not reproduce it, so the cause is unexplained rather than diagnosed.
- **On this host, on this 5 KB workload, the harness has never actually gated
  anything**, because it has not been able to build the workload at all.

This does not soften Finding 2A. It sharpens it: the offline build break has been
hiding a live gate whose behaviour on this host is unknown to the project,
including a run that would have failed the build.

### Finding 2B — hosted CI makes every budget report-only on all three OSes

`.github/workflows/legion-gates.yml:100-111` sets `LEGION_PERF_FAIL_ON_BUDGET_MS: "0"`
for both `perf-harness` and `verify-perf-harness`, on the
`[ubuntu-latest, windows-latest, macos-latest]` matrix. `apply_fail_on_budget_override`
sets every descriptor budget to 0, and
`apply_fail_on_budget_value_to_manual_measurement` reclassifies a `Failed`
manual measurement to `Skipped`. **In hosted CI, no perf budget can fail on any
OS.** The workflow comment is honest about this ("Strict budgets remain a local
gate"), but the consequence is that `P8.F4.T2`'s stop condition — "Stop if any OS
job is allowed to silently skip" — is tripped on every OS on every run.

### Finding 2C — the ADR-0048 workload is not a real workload

Even when it builds, `measure_manual_perf` (`crates/legion-desktop/src/manual_perf.rs:260`)
opens the workspace with `--file Cargo.toml` (5,053 bytes, 133 lines), takes 16
keypress samples and 16 scroll samples against a headless `egui::Context` via
`render_projection_once_for_perf`, and reports p50/p95 from those 16 samples. It
exercises the real projection render path, which is worth something, but the
roadmap asks for ADR-0048 budgets "on real workloads" and a 5 KB TOML file is not
one.

### Finding 2D — there is no 100K-file workload and no 100MB workload in the harness

- `SkeletonKind` (`xtask/src/perf_harness.rs:86-114`) has five variants. None is
  a 100K-file or 100MB workload.
- `plans/evidence/perf-harness-fixtures/100k-file-search.toml` declares
  `kind = "large_fixture_search"`, which is not a `SkeletonKind` variant.
  `rg -n "perf-harness-fixtures|large_fixture_search|fixture_file_count|search_scan_limit" xtask crates -g "*.rs"`
  returns **no matches** — nothing reads these manifests. The directory README
  claimed "They are read by the harness at runtime"; that claim was false and has
  been repaired.
- `plans/evidence/perf-harness-trend/README.md` claimed CI appends timestamped
  snapshots per OS "so the harness can compare the current run against the latest
  prior trend entry". No trend code exists (`rg -n trend xtask/src -g "*.rs"`
  returns one unrelated comment) and the directory contains only the README. Also
  repaired.
- `plans/evidence/production/WS-MANUAL-02/reference-workspaces.md:40,55` cites
  `xtask generate-test-workspace` as the generation command for the 100K-file
  reference workspace RW-2 and for RW-4. **That subcommand does not exist**
  (`rg -n "generate.test.workspace|generate_test_workspace" xtask crates -g "*.rs"`
  → no matches). RW-2 cannot be produced as documented.
- The largest workspace-open workload that actually exists is 1,000 files
  (`crates/legion-project/tests/workspace_scale.rs:98`), and it is `#[ignore]`d.

### Finding 2E — the 100MB budgets exist only behind `--ignored`, and one of them fails today

`cargo test --release -p legion-text --test large_scale_100mb -- --ignored --nocapture` → **exit 101**

```
test result: FAILED. 5 passed; 1 failed; 0 ignored; 0 measured; 2 filtered out
```

| Measurement | Value | Against |
| --- | --- | --- |
| `scale_100mb_viewport_slice_under_budget` | **1.1086 ms — FAILED** | its own < 1 ms budget |
| `scale_100mb_single_keystroke_edit_under_budget` | 44.2219 ms — passed | its own < 50 ms threshold (release) |
| `scale_100mb_memory_ceiling` | 320,180,111 bytes (305 MB) — passed | 400 MB ceiling, for a 99 MB document |

Two things follow.

1. **A 100MB budget is failing right now and no gate can see it.** All six
   `large_scale_100mb` tests are `#[ignore]`d, so `cargo test --workspace
   --all-targets` — which passed 2861/0 today — never runs them, and the perf
   harness has no 100MB workload to run them from. `P1.F4.T5`'s stop condition
   ("Stop if the workload is hidden behind an opt-in flag in CI") is tripped.
2. **The 100MB keystroke passes only because it is measured against the wrong
   budget.** 44.22 ms clears the WS-MANUAL-02-local threshold of 50 ms but is
   **2.76x over ADR-0048's keypress p50 of 16 ms**. The 50 ms threshold is
   asserted in `crates/legion-text/tests/large_scale_100mb.rs:159-162`; the
   16 ms budget is ADR-0048's. Reading the green result as an ADR-0048 pass would
   be wrong, and `reference-workspaces.md` has been annotated to say so.

### Finding 2F — the one place that produces a 100MB edit p50/p95 asserts nothing

`large_file_100mb_degraded_mode_measurement`
(`crates/legion-editor/tests/performance_suite.rs:347`) opens a real 100MB buffer
through the editor engine, takes 16 insert samples, computes `edit_p50` and
`edit_p95`, and `eprintln!`s them. Its three assertions cover projection mode,
payload size, and chunk count. **It asserts nothing about the timings**, and it
is `#[ignore]`d with the reason "performance suite: 100MB report-only
measurement".

Run today, in release:

`cargo test --release -p legion-editor --test performance_suite large_file_100mb_degraded_mode_measurement -- --ignored --nocapture` → **exit 0** (1 passed, 11 filtered out)

```
100MB degraded open=148.2649ms viewport=163.8µs edit_p50=21.7233ms edit_p95=22.6817ms
payload_bytes=1536 chunk_count=1601 threshold_bytes=5242880 byte_len=104857600
```

**`edit_p50 = 21.72 ms` against ADR-0048's 16 ms keypress p50 — 1.36x over
budget. `edit_p95 = 22.68 ms`.** The test passes anyway, because it does not
check. This independently reproduces the known 100MB edit-p50 finding — an
editor-path p50 in the low twenties of milliseconds against a 16 ms budget —
from a run taken today rather than from a stored figure.

So the 100MB workload misses ADR-0048 on two independent paths measured today:
21.72 ms p50 through the editor engine (this finding) and 44.22 ms for a single
insert through `legion-text` (Finding 2E). Neither is gated.

### Finding 2G — `P2.F4.T4` "Add 50K/100K-file fixture benchmarks" is marked `done` and is not

- The 100K half does not exist anywhere that executes.
- The 50K half that does run, `m8.search_stream_50k`, **generates its own fixture
  at runtime under the system temp directory** — which trips that task's own stop
  condition, "Stop if the fixture is regenerated as a side effect of running the
  harness" — and carries `budget_millis = 0`, so it cannot fail.
- `indexed_workspace_search_benchmark_large_fixture`
  (`crates/legion-project/tests/search_workspace.rs:221`) is the only other
  candidate. Its "large fixture" is **500 files**, it is `#[ignore]`d, and its
  only assertion is `assert_eq!(indexed.hit_count, live.hit_count)` — no timing
  assertion at all.
- `P8.F4.T1` depends on `P2.F4.T4`.

`m8.search_stream_50k` was measured twice today, and the pair is instructive.
Neither run can fail, because `budget_millis = 0`.

| Run | Full scan | Cancellation latency | Status |
| --- | ---: | ---: | --- |
| First (quiet machine) | 21,436 ms | 3,782 ms | `skipped` |
| Second (concurrent cargo builds on the same disk) | **670,354 ms** (11.2 min) | **192,530 ms** (3.2 min) | `skipped` |

`reference-workspaces.md` states a threshold of "Search cancellation resource
release — immediate (< 100ms)". The better of the two runs misses it by ~38x and
the worse by ~1,900x, and both are recorded as `skipped` in a run that exits 0.
An 11-minute workspace search inside a green `--strict` perf gate is the clearest
single illustration of what "report-only" costs.

---

## Part 3 — Required gate results

All seven required commands, run in order on commit `7609c775`, Windows.

| Command | Exit | Result |
| --- | --- | --- |
| `cargo run -p xtask -- perf-harness` | 0 | `total=5 passed=3 failed=0 skipped=2 strict=true`. Passes **because** the ADR-0048 workload was skipped, not because it met budget. See Finding 2A. |
| `cargo run -p xtask -- verify-perf-harness` | 0 | `total=5 passed=3 failed=0 skipped=2 strict=true`. Same blind spot. |
| `cargo test --workspace --all-targets --no-fail-fast` | 0 | 2861 passed, 0 failed, 17 ignored, across 253 suites. The 17 ignored include all six `large_scale_100mb` tests, one of which fails when run (Finding 2E). |
| `cargo run -p xtask -- docs-hygiene` | 0 | "documentation hygiene checks passed" |
| `cargo run -p xtask -- claim-audit` | 0 | "claim audit passed" |
| `cargo run -p xtask -- verify-readiness-consistency` | 0 | "readiness consistency ok: 160 backlog task(s) cross-checked" |
| `cargo run -p xtask -- verify-kanban-backlog` | 0 | "kanban backlog ok: 10 epic(s), 41 feature(s), 160 task(s)" |

Supplementary runs performed for this verification:

| Command | Exit | Result |
| --- | --- | --- |
| `cargo test -p legion-desktop --test accessibility` | 0 | 11 passed, 0 failed |
| `cargo build --release -p legion-desktop --no-default-features --features offline` | 101 | `error[E0425]` — root cause of the skipped ADR-0048 workload |
| `cargo test --release -p legion-text --test large_scale_100mb -- --ignored --nocapture` | 101 | 5 passed, 1 failed |
| `cargo test --release -p legion-editor --test performance_suite large_file_100mb_degraded_mode_measurement -- --ignored --nocapture` | 0 | 1 passed. `edit_p50=21.7233ms` against a 16 ms ADR-0048 budget, asserted by nothing |
| `cargo run --release -p legion-desktop -- --manual-perf --workspace . --file Cargo.toml --perf-samples 16` (x7) | 0 | 6 of 7 within all ADR-0048 budgets; 1 of 7 reported `status = "failed"` at keypress p95 36.32 ms. See Finding 2A-bis |

Gates were re-run after the document repairs in this change; see "Post-repair
gate re-run" at the end.

---

## What is missing for `PR-UI-001` promotion

Exact list. Each item is a blocker on its own.

**Accessibility / focus (3-OS)**

1. ~~A committed, re-runnable OS accessibility-tree probe for Windows.~~
   **Delivered today**: `scripts/a11y-uia-walk.ps1`, with the observation at
   `plans/evidence/production/PR-UI-001/2026-08-16-windows-uia-tree.txt`
   (138 descendants under the product window). One of three legs.
2. A **Linux** OS accessibility-tree probe (AT-SPI) and its observation. Never
   captured, and no committed probe exists.
3. A **macOS** OS accessibility-tree probe (AXUIElement) committed to this repo,
   plus a fresh observation from it. The existing macOS observation cannot be
   re-run and does not name its host.
4. Real screen-reader session transcripts (NVDA / VoiceOver / Orca) for GP-1..3.
   The current walkthroughs are label reconstructions, on all three paths.
5. Any accessibility step in CI at all. `.github/workflows/` contains zero
   accessibility references across all five workflows, so none of the above is
   protected against regression once it lands.
6. A decision about whether `legion-desktop --smoke` should keep hardcoding
   `accessibility_tree_smoke: ... OS tree not observed`. It is now demonstrably
   understating the product on Windows, and two tests assert the literal string,
   so the understatement is load-bearing.

Items 2 and 3 cannot be produced on this Windows machine at all and need macOS
and Linux hosts or CI runners.

**Perf harness / ADR-0048**

7. Fix `legion-desktop --no-default-features --features offline` so the ADR-0048
   workload builds, and add that configuration to `legion-gates.yml` next to the
   existing `legion-app` guard so it cannot silently regress again.
8. Make the harness fail, not skip, when the ADR-0048 workload cannot be
   measured. A budget that reports `skipped` on a build break is not a budget.
   `verify-perf-harness` needs the same change.
9. Replace the ADR-0048 workload's `--file Cargo.toml` (5 KB) with a real
   workload.
10. Add a 100MB workload to the standard harness run and gate it on ADR-0048's
    16 ms keypress p50, not WS-MANUAL-02's local 50 ms. Today's measured value is
    44.22 ms.
11. Add a 100K-file workload. The fixture manifest is inert, the reference
    workspace's generation command does not exist, and the largest workload that
    runs is 1,000 files.
12. Fix or re-baseline `scale_100mb_viewport_slice_under_budget`, which fails
    today at 1.1086 ms against a 1 ms budget, and un-`#[ignore]` the
    `large_scale_100mb` suite so a failure is visible to a gate.
13. Give `m8.search_stream_50k` a real budget. 21.4 s full scan and 3.8 s
    cancellation latency currently pass as `skipped`, against a documented
    "< 100ms" cancellation threshold.
14. Stop `LEGION_PERF_FAIL_ON_BUDGET_MS: "0"` from disabling every budget on all
    three hosted OSes, or stop describing the hosted matrix as perf evidence.
15. Explain the one-in-seven keypress-p95 outlier in Finding 2A-bis before the
    gate is switched on for real. A gate that reds the build once every seven
    runs will be disabled by whoever is on call, and the project will be back
    where it started.

**Backlog accuracy**

16. `P2.F4.T4` is `done` but its 100K half does not execute, its 50K half trips
    its own stop condition, and the only large-fixture search test uses 500 files
    with no timing assertion.
17. `P8.F4.T1`'s `evidence` field points at
    `plans/evidence/production/WS-MANUAL-02/WS-MANUAL-02-evidence.md`, which is
    the large-files workstream, not perf-harness replacement evidence.

No backlog status was changed by this verification. Items 16 and 17 are reported
for the owner to act on, because flipping a `done` to `in-progress` changes what
downstream gates read and is not a call to make from a verification pass.

---

## Not claimed

- **No 3-OS evidence was produced here.** Every command in this file ran on one
  Windows host. Nothing in it supports a macOS or Linux claim. The Windows
  accessibility observation is one leg of three.
- **The Windows UIA walk is a tree observation, not an accessibility audit.** It
  shows that control types and names reach the OS layer. It says nothing about
  whether the names are *good*, whether focus order is sane, whether live regions
  announce, whether high-contrast and reduced-motion behave, or whether a screen
  reader can actually complete a task. Do not cite it as "Windows accessibility
  passes".
- **Several UIA nodes are `ControlType.Text` with `automationId=''`**, including
  editor line numbers and buffer content. A tree that a screen reader can
  traverse is not the same as a tree it can navigate usefully. Not assessed here.
- **No screen-reader was run**, on any platform.
- **The ADR-0048 budgets were not proven met, and were not proven missed.** The
  harness could not build the workload that measures them, so the harness
  measured nothing. The out-of-band runs in Finding 2A-bis are seven samples of
  16 events each on one host against a 5 KB file; six passed and one failed. That
  is a stability signal, not a verdict on the budgets, and it is not a substitute
  for the harness doing its job.
- **The 44.22 ms figure is one sample, not a p50.**
  `scale_100mb_single_keystroke_edit_under_budget` performs a single insert at
  the document midpoint. It is enough to show the 16 ms budget is missed by a
  wide margin; it is not a distribution.
- **`m8.search_stream_50k`'s 21.4 s is one run on one machine** with whatever
  else this host was doing. It is not a baseline.
- **No claim is made about whether the macOS AX observation in `WS18-T2` ever
  happened.** It is unverifiable from this repository — the probe source was
  never committed. This file records that it cannot be reproduced, which is a
  statement about the evidence, not an accusation about the observation.
- **No product code was changed.** The `legion-desktop` offline build break was
  diagnosed, not fixed; the correct fix is a product decision about whether the
  streaming rail should be absent from offline builds. The only file added
  outside `plans/evidence/` is `scripts/a11y-uia-walk.ps1`, a verification probe
  that no product path depends on.
- **No backlog or ledger status was changed.**
- **Conclusions are scoped to commit `7609c775` in this worktree.** Other
  worktrees on this repository are ahead; in particular, a `LargeFile100Mb`
  perf-harness skeleton gated on ADR-0048's 16 ms p50 exists as unmerged work
  elsewhere and is not in this tree. The file
  `plans/evidence/production/WS-MANUAL-02/large-file-100mb-measurement.md`, and
  the 23.2 ms edit-p50 figure attributed to it, **do not exist at this commit**;
  the measurements above are independent and were taken today.

---

## Files changed

Documents repaired:

| File | Repair |
| --- | --- |
| `plans/evidence/accessibility/README.md` | Per-OS status separated: Windows repeatable, macOS unreproducible, Linux absent. Walkthroughs relabelled as label inventories rather than screen-reader transcripts. Records why the smoke harness's "OS tree not observed" is a harness limitation. Acceptance note now states the condition is unmet. |
| `plans/evidence/perf-harness-fixtures/README.md` | Removed the false claim that the manifests "are read by the harness at runtime". Marked inert, with the verifying `rg` command recorded. |
| `plans/evidence/perf-harness-trend/README.md` | Removed the description of a trend archive and regression comparison that does not exist. Replaced with what CI actually does (per-OS artifact upload). |
| `plans/evidence/production/WS-MANUAL-02/reference-workspaces.md` | Noted that `xtask generate-test-workspace` does not exist, so RW-2/RW-4 cannot be generated as documented, and that the RW-3 50 ms keystroke threshold is weaker than ADR-0048's 16 ms. |

Added:

| File | Purpose |
| --- | --- |
| `scripts/a11y-uia-walk.ps1` | Windows OS accessibility-tree probe. Added so this verification does not repeat the defect it is reporting — an OS-level observation with no committed probe behind it. Windows only; explicitly documented as not closing the 3-OS gap. No product path depends on it. |
| `plans/evidence/production/PR-UI-001/2026-08-16-windows-uia-tree.txt` | Raw output of that probe against a live product window. |
| `plans/evidence/production/PR-UI-001/2026-08-16-promotion-verification.md` | This file. |

Not edited: `plans/evidence/gui-productization/phase-7-known-limitations.md`
(accurate as scoped), `plans/product-readiness-ledger.md` (its PR-UI-001 row
already states the correct blocker), and `plans/kanban/legion-ga-backlog.toml`.

## Post-repair gate re-run

All seven commands re-run after the document repairs above, same commit, same
host. No regression from the edits.

| Command | Exit | Result |
| --- | --- | --- |
| `cargo run -p xtask -- perf-harness` | 0 | `total=5 passed=3 failed=0 skipped=2 strict=true`, unchanged — `manual.renderer_input_to_paint` still `skipped` on the same `legion-desktop` offline build failure |
| `cargo run -p xtask -- verify-perf-harness` | 0 | `total=5 passed=3 failed=0 skipped=2 strict=true` |
| `cargo run -p xtask -- docs-hygiene` | 0 | "documentation hygiene checks passed" |
| `cargo run -p xtask -- claim-audit` | 0 | "claim audit passed" |
| `cargo run -p xtask -- verify-readiness-consistency` | 0 | "readiness consistency ok: 160 backlog task(s) cross-checked" |
| `cargo run -p xtask -- verify-kanban-backlog` | 0 | "kanban backlog ok: 10 epic(s), 41 feature(s), 160 task(s)" |
| `cargo test --workspace --all-targets --no-fail-fast` | 0 | 2861 passed, 0 failed, 17 ignored, 253 suites |

`claim-audit` passes on the repaired wording without any change to the gate. The
repairs narrow claims; they do not need the anti-overclaiming gate weakened to
accommodate them.
