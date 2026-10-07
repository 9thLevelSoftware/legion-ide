# Open a desktop window

The windowed E2E builds `legion-desktop`, launches a real `eframe` window against a fixture workspace, and writes a report that records whether the window was created.

## Sub-features

- `window-created` requires `window_created = true` in the report.
- `window-identity` requires the report to say this run is not beta-smoke and not golden-path-5.

## How to get to it (user POV)

- From the repository root, run `cargo run -p xtask -- windowed-gui-e2e`.
- The fixture file is `target/windowed-gui/workspace/notes.txt`. The user-visible result is a native window plus `target/windowed-gui/report.toml`.

## Driving it with legion-app

Preconditions:

- A display is available. Linux CI uses `xvfb-run`. Windows and macOS use the normal desktop session.
- This run may build `legion-desktop`. That is part of the command, not a separate product step to skip.

- **Launch.** From the repository root run `cargo run -p xtask -- windowed-gui-e2e`. Exit code is 0.
- **Read the report.** `target/windowed-gui/report.toml` contains `window_created = true`, `not_beta_smoke = true`, and `not_golden_path_5 = true`.
- **Proof.** Copy that report to `target/verify-legion-ide/evidence/<run-id>/windowed-report.toml`. Keep the copy after any cleanup of `target/windowed-gui/workspace`.

## Gotchas

- `--beta-smoke` on a packaged binary is a different check. It does not prove `window_created`.
- xtask refuses to depend on `legion-desktop`, so the proof is the spawned packaged binary, not an in-process call.
- A missing report file is a failure even if the process exits 0.
