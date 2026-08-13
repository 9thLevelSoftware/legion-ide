# Plan 07-02 Result: End-To-End Local IDE Beta Smoke Harness

Status: Complete
Date: 2026-05-27

## Summary

Added a deterministic GUI Phase 7 beta smoke harness that runs through existing `DesktopRuntime` actions against an isolated Rust workspace under `target/gui-phase7-beta-workspace`, writes metadata-only workflow evidence, and keeps the Phase 6 smoke wrapper defaults unchanged.

## Files Changed

- `crates/devil-desktop/src/beta.rs`
- `crates/devil-desktop/src/lib.rs`
- `crates/devil-desktop/src/workflow.rs`
- `crates/devil-desktop/src/smoke.rs`
- `crates/devil-desktop/tests/beta_workflow.rs`
- `scripts/gui-smoke.ps1`
- `scripts/gui-smoke.sh`
- `.github/workflows/ci.yml`
- `plans/evidence/gui-productization/phase-7-local-workflow-smoke.md`
- `.planning/phases/07-fully-functional-local-ide-beta/WAVE-CHECKLIST.md`
- `.planning/phases/07-fully-functional-local-ide-beta/07-02-RESULT.md`

## Smoke Status

- `status: passed`
- Beta workspace: `target/gui-phase7-beta-workspace`
- Evidence: `plans/evidence/gui-productization/phase-7-local-workflow-smoke.md`
- Diagnostics export: `target/gui-phase7-diagnostics.md`
- Covered workflow: explorer refresh, isolated edit/save, active-file search, workspace search, language completion cancellation, terminal launch denial, AI proposal start/details/preview, and quit.
- Unsupported surfaces remain explicitly listed: remote production GUI, collaboration GUI, plugin management GUI, hosted provider activation, signed installer, cross-platform parity, and autonomous apply.

## Verification

| Command | Result |
|---|---|
| `Test-Path crates/devil-desktop/src/beta.rs` | passed |
| `rg -q "gui-phase7-beta-workspace" crates/devil-desktop/src/beta.rs` | passed |
| `rg -q "DesktopAction::StartAiProposal" crates/devil-desktop/src/beta.rs` | passed |
| `rg -q "unsupported_surfaces" crates/devil-desktop/src/beta.rs` | passed |
| `cargo fmt --all` | passed |
| `cargo test -p devil-desktop --test beta_workflow -- --nocapture` | passed, 4 tests |
| `powershell -NoProfile -ExecutionPolicy Bypass -File scripts/gui-smoke.ps1 -Beta -DryRun` | passed |
| `powershell -NoProfile -ExecutionPolicy Bypass -File scripts/gui-smoke.ps1 -DryRun` | passed |
| `bash scripts/gui-smoke.sh --beta --dry-run` | passed |
| `cargo run -p devil-desktop -- --beta-smoke --workspace . --beta-workspace target/gui-phase7-beta-workspace --evidence plans/evidence/gui-productization/phase-7-local-workflow-smoke.md --session-state target/gui-phase7-session.json --diagnostics-export target/gui-phase7-diagnostics.md` | passed |
| `rg -q "status: passed" plans/evidence/gui-productization/phase-7-local-workflow-smoke.md` | passed |
| `rg -q "metadata-only" plans/evidence/gui-productization/phase-7-local-workflow-smoke.md` | passed |
| `rg -q "GUI Phase 7 smoke dry run" .github/workflows/ci.yml` | passed |

## Decisions

- Beta smoke mode is explicit and mutually exclusive with native `--smoke`.
- Write actions are confined to the canonicalized beta workspace under repository `target/`.
- Terminal denial and default-deny unsupported surfaces are recorded as expected beta evidence rather than weakened.
- Evidence records statuses, counts, labels, and paths only; it does not include raw source bodies, dirty buffer text, terminal payloads, provider payloads, prompts, or secrets.

## Blockers

None.
