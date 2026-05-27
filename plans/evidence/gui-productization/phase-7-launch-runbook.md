# GUI Phase 7 Local IDE Beta Launch Runbook

## Scope

This runbook covers the GUI Phase 7 local IDE beta. It is for local Rust repository workflows only: open, browse, edit/save through app authority, search, language/terminal projections, proposal review, diagnostics export, and evidence checks.

## Launch Commands

Normal desktop launch against this repository:

```powershell
cargo run -p devil-desktop -- --workspace . --file Cargo.toml --session-state target/gui-phase7-manual-session.json --diagnostics-export target/gui-phase7-manual-diagnostics.md
```

Non-mutating timed real-repository smoke:

```powershell
cargo run -p devil-desktop -- --smoke --workspace . --file Cargo.toml --duration-ms 250 --evidence target/gui-phase7-real-repo-smoke.md --session-state target/gui-phase7-real-repo-session.json --diagnostics-export target/gui-phase7-real-repo-diagnostics.md
```

Windows package dry run:

```powershell
scripts/package-windows.ps1 -DryRun
```

GUI Phase 7 beta fixture smoke:

```powershell
cargo run -p devil-desktop -- --beta-smoke --workspace . --beta-workspace target/gui-phase7-beta-workspace --evidence plans/evidence/gui-productization/phase-7-local-workflow-smoke.md --session-state target/gui-phase7-session.json --diagnostics-export target/gui-phase7-diagnostics.md
```

PowerShell wrapper dry run:

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/gui-smoke.ps1 -Beta -DryRun
```

POSIX wrapper dry run:

```sh
bash scripts/gui-smoke.sh --beta --dry-run
```

GUI Phase 7 evidence check:

```powershell
cargo run -p devil-cli -- evidence check --phase gui-phase7
```

Final repository gates before acceptance:

```powershell
cargo run -p xtask -- check-deps
cargo fmt --all --check
cargo check --workspace --all-targets
cargo test --workspace --all-targets
cargo clippy --workspace --all-targets -- -D warnings
cargo deny check
```

## Common Workflows

- Open workspace: launch with `--workspace <path>` and optional `--file <path>`.
- Browse explorer: use the explorer refresh/expand controls projected by `devil-desktop`.
- Edit/save: use normal editor input and save actions; automated write smoke uses `target/gui-phase7-beta-workspace` so it does not mutate the real checkout.
- Search: run active-file and workspace search from the GUI search surface.
- Language action: request completion/diagnostics-style actions; edit-producing language behavior must remain proposal-mediated.
- Terminal policy check: terminal launch is default-denied in beta evidence and is recorded as a policy outcome.
- Proposal review: start a proposal, open details, preview the proposal, and keep generated edits proposal-only until explicitly approved by app authority.
- Diagnostics export: pass `--diagnostics-export <path>` or use the Phase 7 beta smoke command to regenerate `target/gui-phase7-diagnostics.md`.
- Quit: use the GUI quit command or close the timed smoke window after the configured duration.

## Evidence Paths

- Beta workflow smoke: `plans/evidence/gui-productization/phase-7-local-workflow-smoke.md`
- Operational health diagnostics: `plans/evidence/gui-productization/phase-7-operational-health-diagnostics.md`
- Real-repository timed smoke: `target/gui-phase7-real-repo-smoke.md`
- Real-repository diagnostics: `target/gui-phase7-real-repo-diagnostics.md`

## Safety Notes

- Do not use the real checkout for automated write smoke. Use the beta fixture workspace under `target/`.
- Do not claim signed installer, cross-platform parity, remote production GUI, collaboration GUI, plugin management, hosted provider activation, or autonomous apply from this runbook.
- Diagnostics and smoke evidence are metadata-only: paths, counts, status labels, ids, and limitation labels.
