---
name: verify-legion-ide
description: "Drive Legion IDE's CLI proof (`legion-app`, commands :w and :q) and the windowed desktop E2E (`xtask windowed-gui-e2e`) the way a user would, and capture proof under target/verify-legion-ide/evidence. Use when a change touches opening a workspace file, saving the active buffer, quitting the CLI, or creating a real desktop window."
---

# Verify Legion IDE

Primary surface is the CLI proof in `legion-app`: it opens the process current directory as a trusted workspace and one file path, then accepts only `:w` and `:q`. It is not the renderer. The desktop product is `legion-desktop`; the scripted user-visible window check is `cargo run -p xtask -- windowed-gui-e2e`. Do not treat unit tests as a substitute for either drive.

There is no shared port. Each CLI drive uses its own working directory under `target/verify-legion-ide/runs/<run-id>/workspace`. Never point a drive at the repository root or at a workspace the user already has open. Two drives may run together only when their working directories differ. Do not kill processes by image name.

Evidence lives in `target/verify-legion-ide/evidence/<run-id>/` and must survive cleanup. The run workspace may be deleted. `target/` is gitignored.

## Launch

From the repository root, build once:

```powershell
cargo build -p legion-app
```

Ready when that command exits 0 and `target\debug\legion-app.exe` exists (on non-Windows, `target/debug/legion-app`).

Each drive is short-lived. Create a run id, then:

```powershell
$run = "cli-" + [DateTimeOffset]::UtcNow.ToUnixTimeSeconds()
$ws = Join-Path (Resolve-Path .) "target\verify-legion-ide\runs\$run\workspace"
New-Item -ItemType Directory -Force -Path $ws | Out-Null
Set-Content -Path (Join-Path $ws "hello.txt") -Value "seed`n" -NoNewline
```

Start the CLI with that directory as the process working directory and `hello.txt` as the only argument. Feed commands on stdin. The process is ready when stdout contains `Opened file id` and `Commands: :w | :q`, then a `>` prompt.

Windowed launch is separate and writes its own report. From the repository root:

```powershell
cargo run -p xtask -- windowed-gui-e2e
```

Ready when the process exits 0 and `target\windowed-gui\report.toml` contains `window_created = true`. On Linux the desktop binary needs a display (`xvfb-run` in CI). This command builds `legion-desktop`, copies an unsigned package, and runs `eframe::run_native` via `--windowed-e2e`. It is not `--beta-smoke` and not golden-path-5.

## Doctor

Read-only. Refuse to drive when any check fails.

```powershell
cargo metadata --no-deps --format-version 1 --manifest-path Cargo.toml | Select-String -Pattern '"name":"legion-app"' -Quiet
Test-Path target\debug\legion-app.exe
```

Both must succeed (use the extensionless binary path off Windows). Confirm the working directory you are about to use is under `target\verify-legion-ide\runs\` and is not the repository root. There is no long-running CLI server and no auth token. A windowed doctor, after that command has been run, is: `target\windowed-gui\report.toml` exists and contains `not_beta_smoke = true`, `not_golden_path_5 = true`, and `window_created = true`.

## Drive

Harness is the CLI stdin session plus, for the window feature only, `xtask windowed-gui-e2e`.

Prompt strings are literal: `Opened file id`, `Commands: :w | :q`, `>`, `Saved file_id=`, `Save did not apply:`.

Commands are only `:w` and `:q`. Anything else is ignored and is not a successful action. `:w` saves the active buffer through the proposal-mediated save path; the CLI has no insert-text command, so a save proof is the save line plus the file bytes on disk, not a typed edit.

Capture stdout, stderr, and the exit code for every CLI drive. Example shape, with `$ws` from Launch:

```powershell
$evidence = Join-Path (Resolve-Path .) "target\verify-legion-ide\evidence\$run"
New-Item -ItemType Directory -Force -Path $evidence | Out-Null
$exe = Join-Path (Resolve-Path .) "target\debug\legion-app.exe"
Push-Location $ws
try {
  ":q" | & $exe hello.txt *> (Join-Path $evidence "quit.txt")
  $code = $LASTEXITCODE
} finally { Pop-Location }
Set-Content -Path (Join-Path $evidence "quit.exit") -Value $code
```

Prefer invoking `target\debug\legion-app.exe` with working directory `$ws` over `cargo run`, so the recorded command is the product binary. If the binary is missing, run Launch first.

## Evidence

Write proof under `target/verify-legion-ide/evidence/<run-id>/`. Each feature proof includes the command, stdout, stderr, and exit code. A mutation (`:w`) also includes a second read of the file that was saved. The windowed feature's proof is a copy of `target/windowed-gui/report.toml` into the evidence directory; the xtask report is necessary but copying it preserves it if a later xtask run overwrites `target/windowed-gui`.

Exercise the real CLI or the real `--windowed-e2e` binary. Do not call `AppComposition` methods from a unit test and call that this skill. Do not use `--beta-smoke` as proof that a window was created. Manual mode stays zero-egress: do not point a drive at a live model provider.

## Cleanup

Delete only `target/verify-legion-ide/runs/<run-id>/` for runs this verification started. Leave `target/verify-legion-ide/evidence/<run-id>/` in place. Do not delete `target/windowed-gui` if you did not create that run; if you did, you may remove `target/windowed-gui/workspace` and `target/windowed-gui/package`, and you still keep the copied report in the evidence directory. Do not kill `legion-app.exe` or `legion-desktop.exe` by name. If a CLI child is still running, it is the process you spawned in that working directory; stop that process id only.

## Helpers

No helper script is shipped. The commands above are the harness.
