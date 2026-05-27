# GUI Phase 6 packaging, platform, and accessibility evidence

## Acceptance Status

- Phase 6 acceptance: Accepted.

## Scope

GUI Phase 6 covers Windows packaging dry runs, platform integration smoke coverage, accessibility-smoke evidence, session metadata persistence safety, diagnostics export, scripted smoke parity, and final repo gates for the current `devil-desktop` adapter.

Legacy Phase 6 collaboration evidence under `plans/evidence/phase-6/` remains intentionally out of scope and unchanged.

## Required Artifacts

- `plans/evidence/gui-productization/phase-6-package-runbook.md`
- `plans/evidence/gui-productization/phase-6-packaging-smoke.md`
- `plans/evidence/gui-productization/phase-6-platform-accessibility-smoke.md`
- `plans/evidence/gui-productization/phase-6-session-diagnostics-safety.md`
- `plans/evidence/gui-productization/phase-6-workflow-smoke.md`
- `plans/evidence/gui-productization/phase-6-performance-reliability.md`
- `plans/evidence/gui-productization/phase-6-ci-parity-plan.md`
- `.planning/phases/06-packaging-platform-integration-and-accessibility/06-01-RESULT.md`
- `.planning/phases/06-packaging-platform-integration-and-accessibility/06-02-RESULT.md`
- `.planning/phases/06-packaging-platform-integration-and-accessibility/06-03-RESULT.md`
- `.planning/phases/06-packaging-platform-integration-and-accessibility/06-04-RESULT.md`
- `.planning/phases/06-packaging-platform-integration-and-accessibility/06-05-RESULT.md`
- `.planning/phases/06-packaging-platform-integration-and-accessibility/06-06-RESULT.md`
- `.planning/phases/06-packaging-platform-integration-and-accessibility/06-07-RESULT.md`

## Required Commands

- `cargo run -p xtask -- check-deps`
- `cargo fmt --all --check`
- `cargo check --workspace --all-targets`
- `cargo test --workspace --all-targets`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo deny check`
- `cargo test -p devil-desktop --test packaging -- --nocapture`
- `cargo test -p devil-desktop --test platform_integration -- --nocapture`
- `cargo test -p devil-desktop --test platform_smoke -- --nocapture`
- `cargo test -p devil-desktop --test session_restore -- --nocapture`
- `cargo test -p devil-desktop --test diagnostics_export -- --nocapture`
- `cargo test -p devil-cli gui_phase6 -- --nocapture`
- `scripts/package-windows.ps1 -DryRun`
- `scripts/gui-smoke.ps1 -DryRun`
- `scripts/gui-smoke.sh --dry-run`
- `cargo run -p devil-cli -- evidence check --phase gui-phase6`

## Final Validation Checklist

- [x] Packaging runbook and dry-run smoke evidence are complete.
- [x] Platform and accessibility smoke evidence is complete.
- [x] Session persistence and diagnostics evidence is complete.
- [x] CI/script parity evidence is complete.
- [x] Final repo gates and GUI Phase 6 CLI evidence check passed.

## Final Gate Results

- `cargo run -p xtask -- check-deps`: passed.
- `cargo fmt --all --check`: passed.
- `cargo check --workspace --all-targets`: passed.
- `cargo test --workspace --all-targets`: passed.
- `cargo clippy --workspace --all-targets -- -D warnings`: passed.
- `cargo deny check`: passed with warning-level duplicate dependency diagnostics from the existing lockfile policy.
- `cargo run -p devil-cli -- evidence check --phase gui-phase6`: passed.
