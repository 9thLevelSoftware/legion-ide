# Legion IDE verification map

This directory is the maintained source for verifying the user-facing CLI proof and the windowed desktop E2E. Read the index before driving the app, then use the matching feature file as the recipe.

## Baseline preconditions

- Repository root is the Legion IDE checkout. Build `legion-app` with `cargo build -p legion-app` before a CLI drive.
- Each CLI run gets a fresh directory `target/verify-legion-ide/runs/<run-id>/workspace` containing `hello.txt` with contents `seed` and a trailing newline.
- The CLI process working directory is that workspace. The file argument is `hello.txt`.
- Never open the repository root as the workspace.
- Evidence goes to `target/verify-legion-ide/evidence/<run-id>/` and is not deleted during cleanup.

## Driving conventions

- Start every CLI recipe from a new run workspace.
- Treat `:w` and `:q` as literal lines on stdin.
- Record stdout, stderr, and the exit code.
- A save is proved by the `Saved file_id=` line and a second read of `hello.txt`, not by the status line alone.
- The window recipe is `cargo run -p xtask -- windowed-gui-e2e` from the repository root. Do not describe that run as `--beta-smoke`.

## Proof and skip reporting

- Capture the user action and the resulting state.
- CLI proof includes the command, combined output, and exit code.
- Report an unreachable path with the attempted command and the unmet precondition.
- Do not report a skipped entry point as verified through a different path.

## Feature entry contract

Each feature file starts with an H1 title and one paragraph describing the user-visible behavior. It then uses exactly four H2 sections in this order.

1. `Sub-features`
2. `How to get to it (user POV)`
3. `Driving it with legion-app`
4. `Gotchas`

## Features

- [Open a workspace file](./open-file.md) covers CLI startup against a disposable workspace file.
- [Save the active buffer](./save-buffer.md) covers `:w` and the bytes left on disk.
- [Quit the CLI](./quit.md) covers `:q` leaving the process with exit code 0.
- [Open a desktop window](./desktop-window.md) covers `xtask windowed-gui-e2e` and `window_created = true`.
