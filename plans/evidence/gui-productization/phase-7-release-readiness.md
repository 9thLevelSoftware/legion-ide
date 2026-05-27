# GUI Phase 7 Release Readiness

## Status

status: accepted; Plan 07-05 ran final gates and the main Phase 7 evidence now says `Phase 7 acceptance: Accepted.`

## Readiness Checklist

| Area | Signoff | Evidence | Status |
|---|---|---|---|
| Security signoff | Proposal, terminal, provider, and unsupported-surface boundaries remain policy-gated. | `phase-7-local-workflow-smoke.md`, `phase-7-known-limitations.md` | Accepted |
| Privacy signoff | Diagnostics and health rows are metadata-only and tested against secret-like dirty text, terminal payload, and prompt markers. | `phase-7-operational-health-diagnostics.md`, `cargo test -p devil-desktop --test diagnostics_export -- --nocapture` | Accepted |
| Operations signoff | Launch, smoke, diagnostics export, and evidence check commands are documented. | `phase-7-launch-runbook.md` | Accepted |
| Rollback signoff | No migration or persisted data format change is introduced by Phase 7; rollback is reverting desktop beta code/docs and removing target evidence. | Phase 7 result files | Accepted |
| Support signoff | Known limitations and manual evidence paths are explicit. | `phase-7-known-limitations.md`, `phase-7-manual-beta-evidence.md` | Accepted |
| Supply-chain signoff | No new production dependency was added in Phase 7; `cargo deny check` passed with warning-level duplicate dependency output recorded in `07-05-RESULT.md`. | `cargo deny check`, `07-05-RESULT.md` | Accepted with warning-level baseline output |
| Diagnostics signoff | `target/gui-phase7-diagnostics.md` includes `## Operational Health` and unsupported-surface labels. | `phase-7-operational-health-diagnostics.md` | Accepted |
| Limitation-review signoff | Unsupported remote/collaboration/plugin/hosted-provider/autonomy/signed-installer/platform-parity behavior is listed. | `phase-7-known-limitations.md` | Accepted |

## Required Final Commands

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

## Go/No-Go Decision

Go for GUI Phase 7 local beta. Plan 07-05 records all required final commands as passed, `plans/evidence/gui-productization/phase-7-local-ide-beta.md` says `Phase 7 acceptance: Accepted.`, and known unsupported surfaces remain explicitly outside the accepted scope.
