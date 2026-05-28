# GUI Phase 8 advanced surface smoke evidence

## Status

- Advanced surface smoke: scripted markers added.
- Plugin management GUI: supported.
- Collaboration GUI: supported.
- Remote workspace GUI: supported.
- Delegated task command center: approval-gated.
- Autonomous apply: unsupported.
- Final GA acceptance: blocked until repository gates and platform parity proof are complete.

## Smoke Coverage

Advanced surface smoke covers the GUI Phase 8 operational surface as metadata-only evidence:

- Plugin management rows and projected plugin command dispatch through app-owned authority.
- Collaboration presence, reconnect/offline/conflict rows, and shared proposal review through proposal authority.
- Remote workspace connection, reconnect/offline indicators, terminal/LSP/filesystem descriptor rows, and remote proposal review through app-owned remote authority.
- Delegated task command-center rows, blockers, refusals, trust gates, proposal-preview links, audit readiness, and unsupported autonomous apply.
- Diagnostics export defaults that point to Phase 8 evidence and session paths.
- Proposal-review paths without direct UI or desktop mutation.

## Script Evidence

| Command | Outcome |
|---|---|
| `powershell -ExecutionPolicy Bypass -File scripts/gui-smoke.ps1 -Help` | passed |
| `bash scripts/gui-smoke.sh --help` | passed |
| `powershell -ExecutionPolicy Bypass -File scripts/gui-smoke.ps1 -Phase8 -DryRun` | passed |
| `bash scripts/gui-smoke.sh --phase-8 --dry-run` | passed |

The Phase 8 smoke mode uses the existing desktop smoke runtime path and changes only evidence/session/diagnostics defaults. It does not introduce new desktop runtime authority.

## CI Evidence

`.github/workflows/ci.yml` now includes:

- GUI Phase 8 smoke dry run on Windows.
- GUI Phase 8 smoke dry run on Unix runners.
- `cargo run -p devil-cli -- evidence check --phase gui-phase8`.
- The existing legacy `cargo run -p devil-cli -- evidence check --phase phase8` gate remains present.

## Boundary Notes

- `devil-ui` remains projection-only.
- `devil-desktop` owns renderer and smoke harness orchestration only.
- Plugin, collaboration, remote, delegated task, terminal, provider, storage, and proposal authority remain in app/protocol/runtime layers.
- Smoke evidence must stay metadata-only and must not include raw source, dirty buffer text, prompts, provider payloads, terminal output bodies, remote transport frames, secrets, or private keys.

## Remaining Proof

- Windows PowerShell and local Bash help/dry-run checks passed in this local run.
- macOS and Linux OS-level parity evidence remains blocked until current runner outputs are archived in `phase-8-platform-parity.md`.
