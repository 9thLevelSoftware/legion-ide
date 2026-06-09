# Manual beta evidence

## Scope

This file records real-repository beta launch notes separately from the automated fixture smoke. Automated write smoke uses an isolated fixture under `target/gui-phase7-beta-workspace`; manual real-repository write checks should use an intentional scratch file only.

## Captured Run Notes

| Date | OS | Command | Workspace | Workflows attempted | Result | Evidence files | Blockers |
|---|---|---|---|---|---|---|---|
| 2026-05-27 | Windows, PowerShell | `cargo run -p legion-desktop -- --smoke --workspace . --file Cargo.toml --duration-ms 250 --evidence target/gui-phase7-real-repo-smoke.md --session-state target/gui-phase7-real-repo-session.json --diagnostics-export target/gui-phase7-real-repo-diagnostics.md` | `C:\Users\dasbl\RustroverProjects\legion-ide` | Native launch, open `Cargo.toml`, render projected shell, platform adapter smoke, diagnostics export | passed | `target/gui-phase7-real-repo-smoke.md`, `target/gui-phase7-real-repo-diagnostics.md` | No interactive real-repo edit/save was attempted; automated write evidence remains isolated to the beta fixture. |
| 2026-05-27 | Windows, PowerShell | `cargo run -p legion-desktop -- --beta-smoke --workspace . --beta-workspace target/gui-phase7-beta-workspace --evidence plans/evidence/gui-productization/phase-7-local-workflow-smoke.md --session-state target/gui-phase7-session.json --diagnostics-export target/gui-phase7-diagnostics.md` | isolated fixture under `target/gui-phase7-beta-workspace` | Browse, edit/save, active-file search, workspace search, language cancellation, terminal denial, proposal review, quit | passed | `plans/evidence/gui-productization/phase-7-local-workflow-smoke.md`, `target/gui-phase7-diagnostics.md` | Fixture smoke does not mutate the real checkout. |

## Manual Evidence Template

| Date | OS | Command | Workspace | Workflows attempted | Result | Evidence files | Blockers |
|---|---|---|---|---|---|---|---|
| `<date>` | `<OS and shell>` | `cargo run -p legion-desktop -- --workspace <repo> --file <scratch-or-readonly-file> --session-state <target/session.json> --diagnostics-export <target/diagnostics.md>` | `<repo path>` | Open, browse, search, optional scratch edit/save, terminal policy check, language action, proposal review, diagnostics export, quit | `<passed/failed>` | `<paths>` | `<none or exact blocker>` |

## Notes

- Manual beta evidence must not use the real checkout for destructive or broad write testing.
- Real-repository launch proof is non-mutating unless an operator intentionally chooses a scratch file.
- Proposal review means opening proposal details and previewing the proposal; generated edits remain proposal-only.
