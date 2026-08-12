# Phase 6 Review: Packaging, Platform Integration, and Accessibility

## Verdict

PASSED after remediation on 2026-05-27.

## Review Panel

- `testing-qa-verification-specialist`: session, diagnostics, smoke, and regression coverage.
- `engineering-infrastructure-devops`: packaging scripts, CI dry runs, evidence gates, and platform replace behavior.
- `engineering-senior-developer`: ownership boundaries, crash-safety semantics, and diagnostics truthfulness.
- `product-technical-writer`: evidence wording, residual risks, and acceptance-state consistency.

## Cycle 1 Findings

1. Diagnostics adapter status overreported failures.
   - Evidence: `DesktopRuntime::diagnostics_export` built platform smoke state with `DesktopPlatformAdapterChecks::default()`, while `adapter_status(false)` rendered `failed`.
   - Impact: diagnostics exports could report clipboard, IME, and file-dialog adapter paths as failed even when no adapter check had been attempted in the diagnostics path.
   - Fix: adapter check fields now distinguish unobserved from observed pass/fail, `build_platform_adapter_checks` is shared by smoke and diagnostics, and diagnostics tests assert the adapter paths are reported as passed instead of false failures.

2. Session publish was not crash-safe enough for the Phase 6 claim.
   - Evidence: `DesktopSessionStore::save` moved the existing final session file to a backup before renaming the temp file into place.
   - Impact: a crash between those two renames could leave the canonical session file missing, contradicting the crash-safe session restore evidence.
   - Fix: session persistence now validates and syncs a same-directory temp file, then replaces the final path using platform replace semantics. Windows uses `MoveFileExW` with replace/write-through flags; non-Windows uses atomic rename plus parent directory sync.

## Cycle 2 Verification

- Passed: `cargo test -p devil-desktop --test platform_integration -- --nocapture`
- Passed: `cargo test -p devil-desktop --test diagnostics_export -- --nocapture`
- Passed: `cargo test -p devil-desktop --test session_restore -- --nocapture`
- Passed: `cargo test -p devil-desktop --test platform_smoke -- --nocapture`
- Passed: `cargo test -p devil-desktop --test packaging -- --nocapture`
- Passed: `cargo test -p devil-cli gui_phase6 -- --nocapture`
- Passed: `cargo run -p xtask -- check-deps`
- Passed: `cargo fmt --all --check`
- Passed: `cargo check --workspace --all-targets`
- Passed: `cargo test --workspace --all-targets`
- Passed: `cargo clippy --workspace --all-targets -- -D warnings`
- Passed: `cargo deny check` with warning-level duplicate dependency diagnostics from the existing lockfile policy.
- Passed: `cargo run -p devil-cli -- evidence check --phase gui-phase6`
- Passed: `powershell -NoProfile -ExecutionPolicy Bypass -File scripts/package-windows.ps1 -DryRun`
- Passed: `powershell -NoProfile -ExecutionPolicy Bypass -File scripts/gui-smoke.ps1 -DryRun`
- Passed: `bash scripts/gui-smoke.sh --dry-run`
- Passed: `cargo run -p devil-desktop -- --smoke --workspace . --file Cargo.toml --duration-ms 250 --evidence plans/evidence/gui-productization/phase-6-platform-accessibility-smoke.md --session-state target/gui-phase6-session.json --diagnostics-export target/gui-phase6-diagnostics.md`
- Not locally runnable: `sh scripts/gui-smoke.sh --dry-run` because `sh` is not on this Windows PATH; the workflow runs this command on Unix CI runners, and the same script passed locally under `bash`.

## Scope Reviewed

- Phase artifacts: `06-CONTEXT.md`, `06-CRITIQUE.md`, `06-01-PLAN.md` through `06-07-PLAN.md`, and `06-01-RESULT.md` through `06-07-RESULT.md`.
- Product changes: desktop packaging, platform smoke, diagnostics, session persistence, smoke harness, CLI evidence gate, xtask evidence gate, scripts, and CI.
- Evidence changes: all GUI Phase 6 evidence files under `plans/evidence/gui-productization/`.

## Residual Risks

- OS accessibility inspection is still not claimed. Phase 6 accepts deterministic metadata-only accessibility projection smoke while preserving `accessibility_smoke: not observed` for OS tree observation.
- The Windows package path is a deterministic executable package/dry run, not a signed installer.
- POSIX smoke wrapper was verified locally through `bash`; local `sh` is unavailable on this Windows host.

## Final Decision

No unresolved review blockers remain. Phase 6 remains accepted and ready for Phase 7 planning.
