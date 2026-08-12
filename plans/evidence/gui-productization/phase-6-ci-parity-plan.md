# GUI Phase 6 CI parity plan

## Scope

CI now carries the same non-interactive GUI Phase 6 checks that can run reliably without a visible desktop session.

## CI Checks

- Windows package dry run: `powershell -NoProfile -ExecutionPolicy Bypass -File scripts/package-windows.ps1 -DryRun`
- Windows GUI smoke dry run: `powershell -NoProfile -ExecutionPolicy Bypass -File scripts/gui-smoke.ps1 -DryRun`
- Unix GUI smoke dry run: `sh scripts/gui-smoke.sh --dry-run`
- GUI Phase 6 evidence gate: `cargo run -p legion-cli -- evidence check --phase gui-phase6`

## Local Checks

- `scripts/package-windows.ps1 -DryRun`
- `scripts/gui-smoke.ps1 -DryRun`
- `scripts/gui-smoke.sh --dry-run`
- `cargo test -p legion-cli gui_phase6 -- --nocapture`

## Boundary Notes

- CI uses dry runs for desktop GUI smoke because hosted runners may not expose a stable interactive desktop.
- The GUI Phase 6 CLI gate accepts an explicit not-accepted scaffold during implementation and becomes strict once final acceptance is claimed.
