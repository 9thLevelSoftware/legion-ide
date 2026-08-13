# 06-05 Result: Smoke Scripts, CI, and CLI Gate

## Summary

Added dry-run GUI smoke wrappers for PowerShell and POSIX shells, wired CI to execute package/smoke dry runs and the GUI Phase 6 evidence gate, and added `devil-cli evidence check --phase gui-phase6` with tests for scaffold and accepted evidence modes.

## Files Changed

- `scripts/gui-smoke.ps1`
- `scripts/gui-smoke.sh`
- `.github/workflows/ci.yml`
- `crates/devil-cli/src/main.rs`
- `plans/evidence/gui-productization/phase-6-ci-parity-plan.md`

## Verification

- Passed: `powershell -NoProfile -ExecutionPolicy Bypass -File scripts/gui-smoke.ps1 -DryRun`
- Passed: `bash scripts/gui-smoke.sh --dry-run`
- Not run locally: `sh scripts/gui-smoke.sh --dry-run` because `sh` is not on this Windows PATH; CI runs it on Unix runners.
- Passed: `cargo test -p devil-cli gui_phase6 -- --nocapture`
- Passed: `cargo run -p devil-cli -- evidence check --phase gui-phase6`
- Passed: `rg -q "GUI Phase 6 evidence gate" .github/workflows/ci.yml`
