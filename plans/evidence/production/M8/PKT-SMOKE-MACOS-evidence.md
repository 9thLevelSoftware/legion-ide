# PKT-SMOKE-MACOS — GP-1 s5 failure on macOS CI: root cause and fix

- Date: 2026-07-05
- Branch: `m8/smoke-macos-s5`
- Machine (local verification): Windows 11, `CARGO_BUILD_JOBS=4`

## Failure

First 3-OS `legion-smoke.yml` dispatch (run [28741840232](https://github.com/9thLevelSoftware/legion-ide/actions/runs/28741840232), macos-latest / macOS 26 arm64):

```
s5 failed (120406ms): s5: timeout (120s) waiting for 'SMOKE_EXIT:' in terminal output
[s5-poll] row[3] len=240 truncated=true payload="bash-3.2$ sh '/var/folders/.../gp1_smoke_test..."
[s5-poll] row[4] ... "command block finished • exit=0 • duration=173ms"
```

windows-latest and ubuntu-latest: green.

## Diagnosis (instrumented dispatch, run [28747873556](https://github.com/9thLevelSoftware/legion-ide/actions/runs/28747873556))

Instrumentation (commit `88a1d64`): the s5 Unix script records cargo resolution and its real exit code to a temp sidecar file — without altering what flows to the PTY — and the timeout dump prints the full scrollback.

macOS results:

```
[s5-sidecar] /Users/runner/.cargo/bin/cargo
[s5-sidecar] PROBE_CARGO_STATUS:0
[s5-sidecar] SMOKE_EXIT:0
[s5-poll] loop exit: session_done=false rows=4
[s5-poll] row[2] len=240 truncated=true payload="\r\nThe default interactive shell is now zsh.\r\n...bash-3.2$ sh '/var/folders/8j/sfr9qqcj73j4p6nhwcfpr0th000"
```

- cargo resolved and the fixture test **passed** (`SMOKE_EXIT:0`, 127ms — the runner pre-warms the test build).
- The PTY delivered the zsh banner + bash 3.2 prompt + command echo + cargo output + exit marker as **one read chunk**. The product projection stored **one row per chunk**, capped at 240 chars (`push_row` → `bounded_label(payload, 240)`), so every byte past the cap — including the marker — was invisible to the projection and to scrollback search.

This is a product defect, not a smoke defect: any terminal command whose output arrives in a chunk with ≥240 bytes of preceding content loses the remainder from the product's terminal panel.

## Fix (commit `4dfe9c8`)

`legion_terminal::osc::split_visible_rows` — chunks are split into screen-visible rows before projection:

- `\n` / `\r\n` end a row;
- bare `\r` is a redraw — last write wins (progress bars, bash horizontal-scroll echo);
- CSI cursor-position sequences (`ESC[…H`/`ESC[…f`, cmd.exe under ConPTY) act as row boundaries and are dropped;
- other escape sequences pass through unchanged;
- row `byte_count`/`truncated` now describe the row, not the chunk.

TDD: `terminal_multi_line_output_chunk_splits_into_per_line_rows` replays the exact macOS chunk shape (marker past char 240) — red before the fix, green after. Plus `terminal_carriage_return_redraw_keeps_final_line_content` and 6 unit tests on `split_visible_rows`.

## Secondary finding: s3 flake (open, instrumented)

s3 (rust-analyzer error diagnostic within 120s) failed 3× in ~9 runs — twice locally under concurrent builds, once on ubuntu-latest in run 28747873556 — while otherwise passing in 2.6–4.6s. Post-mortem instrumentation added (commit `87b03bd`): on the next timeout the runner dumps initial-pump outcome, all buffered diagnostic-notification metadata (uri-hash match, counts), and the session health record, distinguishing "server silent" from "notifications arrived but predicate never matched". Suspected CPU starvation of rust-analyzer; evidence pending next occurrence.

## Local verification (Windows, merged branch state)

- `cargo test -p legion-app -p legion-terminal -p legion-desktop --all-targets --no-fail-fast` — exit 0.
- `cargo clippy -p legion-terminal -p legion-app --all-targets -- -D warnings` — exit 0.
- `cargo run -p xtask -- golden-path-1` — all 7 steps pass.

## 3-OS verification

- Fix-verification dispatch: run [28748515224](https://github.com/9thLevelSoftware/legion-ide/actions/runs/28748515224) — **success on all three jobs** (ubuntu-latest, macos-latest, windows-latest).
