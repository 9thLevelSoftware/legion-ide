# Host observations — native-manual-open-type-save-r05-win11-x64

## THIS DIRECTORY IS NOT AN EVIDENCE RUN

There is no `run.json` in this directory and there is no
`native_product_acceptance.toml`. Per `plans/evidence/completion/README.md`, a
directory without `run.json` is a support or incomplete directory and is ignored
as evidence. It establishes nothing, promotes nothing, and must not be cited as
a run.

The reason is stated plainly: **`xtask native-product-acceptance` was never
invoked.** Packet `s1-02b` was executed by role `sol` (implementer), which the
round's cargo lane rule forbids from running any `cargo`, `rustc` or `xtask`
command. No serialized cargo lane executed the acceptance command during this
packet's window. A `run.json` written without that invocation would be a
fabricated result, so none was written.

What follows is only what the operator observed about the host directly, with
no cargo, no build and no product launch.

## Observation window

- Observed at: `2026-09-08T20:34:33Z` .. `2026-09-08T20:35:09Z` (UTC, host clock).
- Worktree: `D:/legion-ide-completion`
- Branch: `codex/full-product-resume`
- `git rev-parse HEAD`: `e8a649b9a7209ef6157048f34b084376ed4451f2`
- `git status --porcelain`: empty (clean tree) at the time of observation.

## Host facts

Collected with `Get-CimInstance Win32_OperatingSystem` and environment reads.

| Fact | Observed value |
| ---- | -------------- |
| OS caption | `Microsoft Windows 11 Pro` |
| OS version | `10.0.26200` (build `26200`) |
| OS architecture | `64-bit` |
| Processor architecture | `AMD64` |
| `SESSIONNAME` | `Console` |
| `[Environment]::UserInteractive` | `True` |
| `explorer.exe` | running (pid 6864) |

The session facts above are consistent with an interactive logged-in desktop
session. They are **not** a substitute for the harness's own session handshake:
`run_native_product_acceptance` answers the interactive-session question by
launching the driver with `--probe-session` and reading
`interactive_session = true` out of `driver_session.toml`. That handshake never
ran here, because discovery gates before it.

## Prerequisite gates, checked on disk

Checked by directory listing only; nothing was created, staged or copied.

| Gate | Path checked | Present? |
| ---- | ------------ | -------- |
| External native input driver | `tools/native-input-driver/legion-input-driver.exe` | **No.** `tools/` does not exist in this worktree at all. |
| Harness output directory | `target/native-input-acceptance/` | No. |
| Packaged product directory | `target/native-input-acceptance/package/` | No. |
| Packaged product executable | `target/native-input-acceptance/package/legion-desktop.exe` | No. |

`target/debug/legion-desktop.exe` (a development build from earlier rounds) does
exist. It was **not** copied into `target/native-input-acceptance/package/` and
must not be. ADR-0056's subject is an owner-installed package the harness must
not build; staging a development build there would make a run that is blocked on
a missing driver look as though it had a product to drive.

No stub driver was written. `tools/native-input-driver/` was not created.

## What the harness would have to be asked

The exact command the cargo lane must run, from `D:/legion-ide-completion`:

```
cargo run -p xtask -- native-product-acceptance --out-dir target/native-input-acceptance
```

Its integer exit code is the result: `0` passed, `1` conformance-failed,
`2` operational-error, `3` blocked. `xtask/src/main.rs` line 1213 dispatches
straight to `run_native_product_acceptance_command` and returns its code.

`xtask/tests/native_product_acceptance.rs` was not run either; it belongs to the
same cargo lane.

## The prerequisite string, quoted from source — NOT an observation

Reproduced here so a later run's report can be byte-diffed against the constant
rather than against a paraphrase. This is the text of
`PREREQUISITE_DRIVER_MISSING` at `xtask/src/native_product_acceptance.rs:75-82`
on commit `e8a649b9a7209ef6157048f34b084376ed4451f2`, with the `concat!` parts
joined. **It is source text, not a harness observation.** Nothing here reports
that the harness emitted it.

```text
A Windows 11 x64 host with an interactive logged-in desktop session and the external native input driver installed at `tools/native-input-driver/legion-input-driver.exe`, able to inject OS-level keyboard, pointer, text, clipboard and IME/CJK input into another process and to read that process's UI Automation text and clipboard state from outside it.
```

## Note for whoever completes this run

If a `run.json` is later added to this directory, this file becomes an
attachment the run owns, and `plans/evidence/completion/README.md` requires that
every owned attachment be declared in `artifact_hashes` with its real SHA-256.
Declare it or delete it; do not leave it undeclared.
