# Plan 08-06 Result: GA Release Update Rollback Incident Evidence

Status: Complete; final GA acceptance remains blocked on missing macOS/Linux platform proof
Date: 2026-05-27

## Summary

Added GUI Phase 8 smoke script markers, CI evidence wiring, GA release runbook, update/rollback/incident drill evidence, platform parity evidence, and advanced-surface smoke evidence. The plan did not change runtime or product code.

## Files Changed

- `scripts/gui-smoke.ps1`
- `scripts/gui-smoke.sh`
- `.github/workflows/ci.yml`
- `plans/evidence/gui-productization/phase-8-ga-release-runbook.md`
- `plans/evidence/gui-productization/phase-8-update-rollback-incident.md`
- `plans/evidence/gui-productization/phase-8-platform-parity.md`
- `plans/evidence/gui-productization/phase-8-advanced-surface-smoke.md`
- `.planning/phases/08-advanced-platform-gui-ga/08-06-RESULT.md`

## Verification

| Command | Result |
|---|---|
| `rg -q "phase-8" scripts/gui-smoke.ps1 scripts/gui-smoke.sh` | passed |
| `rg -q "gui-phase8" .github/workflows/ci.yml` | passed |
| `rg -q "GA release" plans/evidence/gui-productization/phase-8-ga-release-runbook.md` | passed |
| `rg -q "Rollback" plans/evidence/gui-productization/phase-8-update-rollback-incident.md` | passed |
| `rg -q "Platform parity: Windows" plans/evidence/gui-productization/phase-8-platform-parity.md` | passed |
| `rg -q "Advanced surface smoke" plans/evidence/gui-productization/phase-8-advanced-surface-smoke.md` | passed |
| `powershell -ExecutionPolicy Bypass -File scripts/gui-smoke.ps1 -Help` | passed |
| `bash scripts/gui-smoke.sh --help` | passed |
| `powershell -ExecutionPolicy Bypass -File scripts/gui-smoke.ps1 -Phase8 -DryRun` | passed |
| `bash scripts/gui-smoke.sh --phase-8 --dry-run` | passed |
| `cargo run -p devil-cli -- evidence check --phase gui-phase8` | passed |
| `cargo run -p xtask -- check-deps` | passed |
| `cargo fmt --all --check` | passed |
| `git diff --check` | passed; Git reported only line-ending normalization warnings for modified text files |

## Decisions

- Added `-Help` and `-Phase8` to the PowerShell smoke wrapper.
- Added `--help` and `--phase-8` to the shell smoke wrapper.
- Kept Phase 8 smoke on the existing desktop smoke runtime path and changed only Phase 8 evidence, session-state, and diagnostics defaults.
- Added a distinct GUI Phase 8 CI smoke dry run and `gui-phase8` evidence gate without removing the accepted legacy `phase8` gate.
- Recorded signing as unsupported by evidence; no signing keys, certificates, private keys, or release secrets were added.

## Platform Parity

- Platform parity: Windows - evidenced locally through PowerShell help and Phase 8 dry-run checks.
- Platform parity: macOS - BLOCKED until a current `macos-latest` CI or equivalent platform run is archived.
- Platform parity: Linux - BLOCKED until a current `ubuntu-latest` CI or equivalent platform run is archived.

These are final GA acceptance blockers, not evidence-free residual risks.

## Boundary Evidence

- No runtime/product crates were modified.
- `devil-ui` remains projection-only.
- `devil-desktop` does not gain plugin, collaboration, remote, terminal, storage, provider, delegated task, or proposal authority.
- Smoke and operations evidence is metadata-only and does not store raw source, dirty buffer text, prompts, provider payloads, terminal output bodies, remote transport frames, secrets, private keys, or signing credentials.
- Legacy accepted `plans/evidence/phase-8/` files were not modified.

## Blockers

- BLOCKED for final GUI Phase 8 GA acceptance: macOS platform parity proof is missing.
- BLOCKED for final GUI Phase 8 GA acceptance: Linux platform parity proof is missing.
