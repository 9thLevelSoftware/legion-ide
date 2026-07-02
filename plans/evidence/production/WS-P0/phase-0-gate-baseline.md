# Phase 0 Gate Baseline — Clean Full-Gate Run with Evidence

Date: 2026-07-02
Branch: `fix/phase-0-truth-repair`
Working directory for all commands: repo root, `C:\Users\dasbl\RustroverProjects\legion-ide`
Commit at gate-baseline capture: `3474de691f69b0d93436bd1e31f67d7abcc9a4e0`
(Task started at commit `62502af`; three fix commits landed before the baseline run below.)

## Environment substitution (per controller amendment 1)

The task brief's Step 1 ("close RustRover, add Defender exclusions") could not be
performed — the user's IDE could not be closed and Defender settings could not be
changed in this session. Per controller instruction, the substitution used instead was:

- Every cargo command below was run **strictly one at a time** (never concurrently).
- `cargo test` / `cargo check` / `cargo clippy` build phases were run with **`-j 4`**
  instead of default parallelism. A prior session root-caused Windows-specific
  rustc ICE / "crate required to be available in rlib format, but was not found in
  this form" rlib-linking flakiness at full parallelism that disappears at `-j 4`
  (see Task 5 report addendum, `.superpowers/sdd/task-5-report.md`). This baseline
  run did not encounter either symptom at `-j 4`.
- One genuine disk-space exhaustion was hit and resolved during this task (see
  "Environmental incident" below) — this was not build-artifact corruption from
  concurrent IDE/antivirus interference, but real disk pressure from a 104GB
  `target/` directory on a drive with only ~3GB free at the time.

## Real failures found and fixed (per Step 3 decision rules)

Two genuinely new, real test failures were found in the full run (both from tests
added in commit `236a492`, "feat: advance Legion productionization surfaces," which
predates this task). Both were **stale test expectations that contradicted an
already-established, independently-tested product contract** — not product-threshold
judgment calls, and not code regressions. Per the Step 3 decision rule ("fix whichever
side is wrong — the test if it asserts a stale contract, the code if behavior
genuinely regressed"), both were fixed by correcting the test.

### 1. `legion-editor::large_file_100mb_open_and_scroll_stays_streaming` (amendment 4 known failure)

- **Symptom:** `assertion left == right failed: 100MB files should remain in
  streaming mode instead of degrading to the full-cache fallback; left: Degraded,
  right: Normal`.
- **Root cause:** The test (new in `236a492`, not present before) asserted
  `BufferMode::Normal` for a 100MB buffer opened with `EditorEngine::new()` (default
  thresholds). `DEFAULT_FULL_CACHE_BYTE_BUDGET_BYTES` is 5MB
  (`crates/legion-text/src/lib.rs:32`), and
  `EditorEngine::mode_for_byte_len_with_thresholds`
  (`crates/legion-editor/src/lib.rs:1952-1959`) deliberately routes any buffer above
  the configured threshold into `BufferMode::Degraded`. This is the accepted,
  independently-tested WS-MANUAL-02 product contract (see
  `plans/evidence/production/WS-MANUAL-02/WS-MANUAL-02-evidence.md`, SCALE.02/SCALE.05,
  and `crates/legion-editor/tests/large_file_scale.rs`, which predates this task and
  explicitly asserts `Degraded` for above-threshold buffers). `git log -p` confirms
  `large_file_streaming.rs` was newly added in `236a492`, and the Degraded-mode
  threshold logic long predates it — so the test's expectation was wrong, not the
  code.
- **Fix:** Corrected the test's expected `BufferMode`/`ViewportProjectionMode`/
  `large_file_status` assertions to `Degraded`/`DegradedLargeFile`/`Some(..)`, since
  Degraded mode is itself the streaming path (chunked/bounded viewport payloads —
  verified by the still-present `line_slices` bounded-size assertion, which now
  passes). No product threshold or runtime behavior was changed.
- **Commit:** `823bc42` — `test: fix stale BufferMode expectation in
  large_file_streaming test`.
- **Verification:** `cargo test -p legion-editor --test large_file_streaming -j 4` —
  1 passed, 0 failed.

### 2. `legion-vscode-compat::compat_report_does_not_select_node_runtime_for_tier2_extensions`

- **Symptom:** `assertion left == right failed: metadata-only ingestion must not
  choose a Node.js sidecar; left: NodeSidecar, right: Deferred`.
- **Root cause:** The test (also new in `236a492`) exercised a fixture whose
  contributions (`commands`, `debuggers`: Tier1; `views`: Tier2) and activation
  events cap out at `VsCodeCompatibilityTier::Tier2ExtensionHostSidecar` — never
  reaching `Tier3WebviewNotebookCustomEditor` (only `webviews`/`notebooks`/
  `customEditors` map to Tier3/Deferred, per `classify_contribution` in
  `crates/legion-vscode-compat/src/lib.rs:369-407`). The pre-existing, independently
  tested mapping in `extension_host_session_for_manifest`
  (`crates/legion-vscode-compat/src/lib.rs:207-223`, validated by the unit test
  `executable_entrypoint_requires_host_policy_without_activation_events`, which
  predates `236a492` by several commits per `git log -p`) routes non-web Tier2
  extensions to `NodeSidecar`, not `Deferred`. The new integration test's expected
  runtime was simply wrong.
- **Fix:** Corrected the runtime/process-label assertions to
  `NodeSidecar`/`"node-extension-host-sidecar"` and renamed the test to
  `compat_report_selects_node_sidecar_runtime_for_non_web_tier2_extensions` to
  describe the behavior it actually verifies. No production logic changed.
- **Commit:** `102ecdd` — `test: fix stale runtime expectation in compat_report
  Tier2 test`.
- **Verification:** `cargo test -p legion-vscode-compat --test compat_report -j 4`
  — 2 passed, 0 failed. Full crate suite (`cargo test -p legion-vscode-compat -j 4`)
  — 11 unit + 2 integration + 0 doc tests, all pass.

### Other Step 3 known suspects — already resolved, no action needed

- `legion-desktop --test input_conformance` (workspace trust policy suspect): ran
  clean at HEAD before this task touched anything — 6/6 passed
  (`cargo test -p legion-desktop --test input_conformance -j 4`). No fix required;
  a prior task already addressed this.
- TDD tests in `legion-ai`/`legion-plugin`/`legion-storage`: all pass at HEAD
  (`cargo test -p legion-ai -p legion-plugin -p legion-storage -j 4`, 0 failures).
  Confirms commit `236a492`'s claim that targeted runs of these crates passed.
- `scope_picker::ScopeRiskTolerance` E0432: not encountered in this session; the
  types are present and the workspace compiles cleanly.

## Additional real finding: `cargo deny check` (not in original suspect list)

`cargo-deny` **is installed** (`cargo-deny 0.19.7`) on this machine, so the
"if not installed, record explicitly" carve-out (amendment 5) did not apply — it ran
for real and surfaced genuine, pre-existing findings (present before this task's test
fixes; confirmed via `git diff 62502af..HEAD~2 --stat`, which touched no
`Cargo.lock`/`deny.toml`):

1. **`advisories` FAILED** — `RUSTSEC-2026-0190`: `anyhow 1.0.102` has an unsound
   `Error::downcast_mut()` after `Error::context()`, pulled in transitively via
   `wasmtime`/`legion-plugin`/`legion-app`/`legion-desktop` and directly by
   `legion-app`/`legion-cli`/`legion-desktop`. Fixed with `cargo update -p anyhow`
   (1.0.102 → 1.0.103), which satisfies the workspace's existing `anyhow = "1"`
   constraint with no source changes. `cargo deny check advisories` now passes.
2. **`bans` FAILED** (`multiple-versions = "deny"`) — `wasmtime` 46.0.1's dependency
   chain (via `wasmtime-internal-cache` and `wast`/`wat`) now pulls a newer
   `toml`/`serde_spanned`/`toml_datetime`/`wasm-encoder` lineage than the workspace's
   own `toml 0.8` and `wasm-compose 0.251` chain, producing 4 new duplicate-version
   pairs not yet in `deny.toml`'s reviewed skip baseline. Per the file's own
   documented policy ("the NEWEST version is kept as canonical and the older
   redundant version(s) are skipped... re-generate after dependency bumps with
   `cargo deny check bans`"), added `serde_spanned@0.6.9`, `toml@0.8.23`,
   `toml_datetime@0.7.5+spec-1.1.0`, and `wasm-encoder@0.251.0` to the skip list
   (each crate's newest resolved version kept canonical). `cargo deny check bans`
   now passes.
- **Commit:** `3474de6` — `fix: patch anyhow RUSTSEC-2026-0190 and rebaseline
  cargo-deny duplicate skips`.
- **Verification after fix:** `cargo deny check` exits 0
  (`advisories ok, bans ok, licenses ok, sources ok`); `cargo check --workspace
  --all-targets -j 4` and `cargo test -p legion-app -p legion-cli -p legion-plugin -j 4`
  both clean after the `anyhow`/`Cargo.lock` bump.
- Two pre-existing `unmatched-skip` **warnings** remain (`objc2-metal@0.2.2`,
  `objc2-quartz-core@0.2.2`) — macOS-only crates not resolved on this Windows
  toolchain. These were already warnings before this task and do not fail the gate
  (`cargo deny check` treats unmatched-skip as warn-level per its default config);
  left untouched as they are outside this task's scope and are not Windows-relevant.

## Environmental incident: disk exhaustion during final gate 7 re-run

The first re-run of `cargo test --workspace --all-targets --no-fail-fast -j 4` on the
final commit failed with a real, non-test-related error:

```
error: failed to write query cache to `...\collaboration_gui-.../query-cache.bin`:
There is not enough space on the disk. (os error 112)
error: linking with `link.exe` failed: exit code: 1201
```

`target/` had grown to **104.71 GB**, and the `C:` drive had only **~3.2 GB free**
(`df -h /c/` showed 100% used). This was diagnosed as legitimate disk pressure, not
build-artifact corruption from concurrent IDE/antivirus interference (no other cargo
process was running, and rustc/link.exe processes were confirmed actively working via
`tasklist`, not hung).

**Resolution:** `cargo clean -p legion-desktop` freed 59.7 GB (9,377 files) — this
crate's `--all-targets` build produces ~45 separate test binaries whose incremental
compilation caches had accumulated across many prior sessions. This is regenerable
build cache only; no source, dependency, or lockfile content was touched. After
cleaning, `C:` had 63 GB free and the full re-run (below) completed cleanly.
This incident and its resolution are recorded here per the "record any recorded gaps
/ re-runs due to flakiness" instruction; it is disk-space environmental noise, not a
code or test defect.

## Step 2: Full gate run (final, on commit `3474de6`, post-fixes)

All commands run from repo root. Commit SHA for this baseline: `3474de691f69b0d93436bd1e31f67d7abcc9a4e0`.

| # | Command | Working dir | Start | End | Exit | Result |
| --- | --- | --- | --- | --- | --- | --- |
| 1 | `cargo run -p xtask -- check-deps` | repo root | 01:44:36 | 01:44:58 | 0 | `dependency policy checks passed` |
| 2 | `cargo run -p xtask -- docs-hygiene` | repo root | 01:44:58 | 01:45:00 | 0 | `documentation hygiene checks passed` |
| 3 | `cargo run -p xtask -- no-egui-textedit` | repo root | 01:45:05 | 01:45:05 | 0 | `no-egui-textedit checks passed` |
| 4 | `cargo run -p xtask -- verify-kanban-backlog` | repo root | 01:45:08 | 01:45:09 | 0 | `kanban backlog ok: 10 epic(s), 38 feature(s), 146 task(s)` |
| 4b | `cargo test -p xtask --test kanban_backlog -j 4` (amendment 3) | repo root | 01:45:12 | 01:45:13 | 0 | 10 passed, 0 failed |
| 5 | `cargo fmt --all --check` | repo root | 01:45:15 | 01:45:20 | 0 | No diff — clean |
| 6 | `cargo check --workspace --all-targets -j 4` | repo root | 01:45:25 | 01:45:52 | 0 | All 29 workspace crates + xtask checked clean |
| 7 | `cargo test --workspace --all-targets --no-fail-fast -j 4` | repo root | 01:56:40 | 02:05:18 | 0 | **1509 passed, 0 failed, 12 ignored** across 174 test binaries/suites (includes one disk-exhaustion re-run; see incident note above — the run reported here is the clean re-run after `cargo clean -p legion-desktop`) |
| 8 | `cargo clippy --workspace --all-targets -j 4 -- -D warnings` | repo root | 02:05:35 | 02:06:09 | 0 | No warnings, no errors |
| 9 | `cargo deny check` | repo root | 02:06:19 | 02:06:33 | 0 | `advisories ok, bans ok, licenses ok, sources ok` (2 pre-existing unmatched-skip warnings, not errors) |
| 10 | `python3 -m pytest evals training -q` (amendment 2 substitution: `python3`, not `python`/`py`) | repo root | 02:06:56 | 02:06:58 | 0 | 2 passed, 0 failed (see re-run note below) |

### Note on gate 10 re-run

The first attempt at gate 10 failed with `OSError: [WinError 6] The handle is
invalid` inside Python's `subprocess.run(..., capture_output=True)` — a transient
Windows handle-inheritance failure in the test harness's own subprocess spawn, not an
assertion failure or code defect (1 of 2 tests affected;
`ReviewerFixtureEvalTest::test_reviewer_fixture_cli_writes_output`). Re-ran once per
the "re-run before treating any failure as real" guidance (applied here by analogy to
the cargo re-run rule, since it is the same class of Windows subprocess-spawn
flakiness); the immediate re-run passed cleanly (2 passed, 0 failed, exit 0). Recorded
here as a flaky re-run, not silently discarded.

## Pass/fail summary

All 10 gates (including the amendment-3 kanban_backlog test) exit 0 on the final
commit `3474de6`. Full workspace test suite: **1509 passed / 0 failed / 12 ignored**
across 174 test result blocks. `cargo clippy -D warnings`: 0 warnings. `cargo deny
check`: advisories/bans/licenses/sources all `ok` (cargo-deny is installed; no gap to
record there). Python suite: 2 passed / 0 failed after one flaky re-run.

## Commits produced by this task

1. `823bc42` — `test: fix stale BufferMode expectation in large_file_streaming test`
2. `102ecdd` — `test: fix stale runtime expectation in compat_report Tier2 test`
3. `3474de6` — `fix: patch anyhow RUSTSEC-2026-0190 and rebaseline cargo-deny duplicate skips`
4. This evidence file, committed separately as
   `test: restore full standing gate set to green with evidence baseline`.

## Self-review

- Every gate row above reflects a command actually executed in this session, with
  real captured tails (saved under the session scratchpad during the run); none are
  assumed or inferred from prior sessions.
- The `-j 4` substitution (amendment 1) is recorded above and was used for every
  `cargo test`/`cargo check`/`cargo clippy` invocation.
- The one legitimate re-run (gate 10, Windows subprocess handle flakiness) and the
  one legitimate environmental incident (disk exhaustion during gate 7, resolved via
  `cargo clean -p legion-desktop`) are both recorded, not silently discarded.
- cargo-deny's availability was recorded as "installed, ran for real" rather than
  claiming a gap that doesn't exist.
- No test was weakened to pass (no thresholds changed, no assertions deleted) —
  both test fixes corrected stale expectations to match already-established,
  independently-tested product contracts; the dependency fixes (anyhow bump,
  deny.toml skip-list additions) followed the file's own documented maintenance
  procedure exactly.
