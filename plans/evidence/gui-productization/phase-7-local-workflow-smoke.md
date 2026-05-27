# GUI Phase 7 Local Workflow Smoke

## Status

status: passed
smoke_label: GUI Phase 7 beta smoke
metadata-only: true
real_workspace_root: .
beta_workspace_root: \\?\C:\Users\dasbl\RustroverProjects\devil-ide\target\gui-phase7-beta-workspace
diagnostics_export: target/gui-phase7-diagnostics.md
status_message_count: 1

## Command

`cargo run -p devil-desktop -- --beta-smoke --workspace . --beta-workspace target/gui-phase7-beta-workspace --evidence plans/evidence/gui-productization/phase-7-local-workflow-smoke.md --session-state target/gui-phase7-session.json --diagnostics-export target/gui-phase7-diagnostics.md`

## Local IDE Workflow

browse: refreshed explorer nodes=4
edit_save: edited and saved isolated beta workspace file
active_file_search: completed Completed results=2 omitted_files=0 omitted_results=0
workspace_search: completed Completed results=5 omitted_files=0 omitted_results=0
language: status=Cancelled operations=2 cancellations=1 problems=0
terminal: terminal_denied_expected status=Denied rows=0 omitted=0
proposal: proposal=2 preview=Some(ProposalLifecycleUpdated { proposal_id: ProposalId(2), lifecycle_state: Previewed, status: "Proposal 2 previewed (Previewed)" }) ledger_rows=2 selected=Some(ProposalId(2))

## Unsupported Surfaces

unsupported_surfaces:
- Remote production GUI: unsupported
- Collaboration GUI: unsupported
- Plugin management GUI: unsupported
- Hosted provider activation: unsupported
- Signed installer: unsupported
- Cross-platform parity: unsupported
- Autonomous apply: unsupported

## Privacy

- Evidence records paths, counts, statuses, and labels only.
- Evidence does not include raw source, dirty buffer text, prompts, provider payloads, terminal payloads, or secrets.

## Errors

- none
