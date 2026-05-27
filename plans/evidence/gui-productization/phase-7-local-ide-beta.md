# GUI Phase 7 local IDE beta evidence

## Acceptance Status

- Phase 7 acceptance: Accepted.

This document is GUI Phase 7 local-beta acceptance evidence. It is separate from the accepted legacy remote-development Phase 7 evidence under `plans/evidence/phase-7/`.

## Scope

GUI Phase 7 covers local IDE beta readiness for opening a Rust workspace, browsing files, editing and saving through app authority, search, language and terminal surfaces, proposal review, operational health, privacy-safe diagnostics, launch documentation, known limitations, and evidence-gated acceptance.

It does not accept remote production GUI, collaboration GUI, plugin management, hosted provider activation, autonomous apply, signed installer readiness, or cross-platform parity.

## Required Artifacts

- `plans/evidence/gui-productization/phase-7-local-workflow-smoke.md`
- `plans/evidence/gui-productization/phase-7-operational-health-diagnostics.md`
- `plans/evidence/gui-productization/phase-7-launch-runbook.md`
- `plans/evidence/gui-productization/phase-7-known-limitations.md`
- `plans/evidence/gui-productization/phase-7-release-readiness.md`
- `plans/evidence/gui-productization/phase-7-manual-beta-evidence.md`
- `.planning/phases/07-fully-functional-local-ide-beta/07-01-RESULT.md`
- `.planning/phases/07-fully-functional-local-ide-beta/07-02-RESULT.md`
- `.planning/phases/07-fully-functional-local-ide-beta/07-03-RESULT.md`
- `.planning/phases/07-fully-functional-local-ide-beta/07-04-RESULT.md`
- `.planning/phases/07-fully-functional-local-ide-beta/07-05-RESULT.md`
- `scripts/gui-smoke.ps1`
- `scripts/gui-smoke.sh`
- `.github/workflows/ci.yml`

## Required Commands

- `cargo run -p xtask -- check-deps`
- `cargo fmt --all --check`
- `cargo check --workspace --all-targets`
- `cargo test --workspace --all-targets`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo deny check`
- `cargo test -p devil-desktop --test beta_workflow -- --nocapture`
- `cargo test -p devil-desktop --test operational_health -- --nocapture`
- `cargo test -p devil-desktop --test diagnostics_export -- --nocapture`
- `powershell -NoProfile -ExecutionPolicy Bypass -File scripts/gui-smoke.ps1 -Beta -DryRun`
- `bash scripts/gui-smoke.sh --beta --dry-run`
- `cargo run -p devil-cli -- evidence check --phase gui-phase7`

## Known Limitations Required For Acceptance

- Remote production GUI: unsupported
- Collaboration GUI: unsupported
- Plugin management GUI: unsupported
- Hosted provider activation: unsupported
- Signed installer: unsupported
- Cross-platform parity: unsupported
- Autonomous apply: unsupported

## Final Validation Checklist

- [x] Beta workflow smoke evidence is complete.
- [x] Operational health and diagnostics evidence is complete.
- [x] Launch runbook, known limitations, release readiness, and manual beta evidence are complete.
- [x] Final repository gates and GUI Phase 7 evidence checks passed.
- [x] Unsupported advanced surfaces remain documented as unsupported.

## Residual Risks

- Native OS accessibility inspection and broad platform parity remain outside GUI Phase 7 acceptance until later evidence accepts them.
- GUI Phase 7 acceptance is local-beta acceptance only; unsupported advanced surfaces remain outside this phase.
