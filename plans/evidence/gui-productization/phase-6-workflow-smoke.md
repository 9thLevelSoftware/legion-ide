# GUI Phase 6 workflow smoke evidence

## Status

status: passed with one local shell limitation

## Commands

- `powershell -NoProfile -ExecutionPolicy Bypass -File scripts/gui-smoke.ps1 -DryRun`: passed.
- `bash scripts/gui-smoke.sh --dry-run`: passed.
- `sh scripts/gui-smoke.sh --dry-run`: not run locally because `sh` is not on this Windows PATH; CI runs `sh scripts/gui-smoke.sh --dry-run` on Unix runners.
- `cargo test -p legion-cli gui_phase6 -- --nocapture`: passed, 3 tests.
- `cargo run -p legion-cli -- evidence check --phase gui-phase6`: passed in scaffold mode.
- `rg -q "GUI Phase 6 evidence gate" .github/workflows/ci.yml`: passed.

## Script Parity

- PowerShell smoke dry run prints the same cargo command, evidence path, session state path, and diagnostics export path as the POSIX wrapper.
- CI runs Windows package dry run, Windows GUI smoke dry run, Unix GUI smoke dry run, and the GUI Phase 6 evidence gate.

## Residual Risk

- Local Windows verification used `bash` for the POSIX wrapper because `sh` is unavailable on the host PATH. The CI workflow uses `sh` on Unix runners.
