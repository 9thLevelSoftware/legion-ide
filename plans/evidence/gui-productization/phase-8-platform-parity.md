# GUI Phase 8 platform parity evidence

## Status

- Platform parity: Windows - evidenced locally on 2026-05-27 and by GitHub Actions run `26590800830` on 2026-05-28.
- Platform parity: macOS - evidenced by GitHub Actions run `26590800830` on 2026-05-28.
- Platform parity: Linux - evidenced by GitHub Actions run `26590800830` on 2026-05-28.
- GA platform parity acceptance: Accepted from current CI matrix proof plus local Windows dry-run evidence.

## Windows Evidence

Platform parity: Windows

| Check | Outcome |
|---|---|
| `powershell -ExecutionPolicy Bypass -File scripts/gui-smoke.ps1 -Help` | passed |
| `powershell -ExecutionPolicy Bypass -File scripts/gui-smoke.ps1 -Phase8 -DryRun` | passed |

The Windows evidence proves the PowerShell smoke wrapper exposes GUI phase-8 help and can build a non-executing advanced-surface smoke command with Phase 8 evidence, session state, and diagnostics defaults.

## macOS Evidence

Platform parity: macOS

Status: accepted.

Current proof:

- GitHub Actions run: `26590800830`
- Run URL: `https://github.com/9thLevelSoftware/devil-ide/actions/runs/26590800830`
- Job: `Milestone validation (macos-latest)`
- Job conclusion: `success`
- Created: `2026-05-28T17:24:44Z`
- Completed: `2026-05-28T17:27:51Z`
- Successful steps included format check, clippy, tests, workspace check, dependency policy gate, `GUI Phase 8 smoke dry run (Unix)`, `GUI Phase 8 evidence gate`, and `Phase 8 evidence gate`.

## Linux Evidence

Platform parity: Linux

Status: accepted.

Current proof:

- GitHub Actions run: `26590800830`
- Run URL: `https://github.com/9thLevelSoftware/devil-ide/actions/runs/26590800830`
- Job: `Milestone validation (ubuntu-latest)`
- Job conclusion: `success`
- Created: `2026-05-28T17:24:44Z`
- Completed: `2026-05-28T17:28:45Z`
- Successful steps included format check, clippy, tests, workspace check, dependency policy gate, `GUI Phase 8 smoke dry run (Unix)`, `GUI Phase 8 evidence gate`, `Phase 8 evidence gate`, and `Cargo deny check`.

## Cross-Platform Decision

GUI Phase 8 platform parity is accepted from current matrix evidence. Windows has local dry-run evidence plus `windows-latest` CI proof; macOS and Linux have current CI proof from run `26590800830`.

Windows CI details:

- GitHub Actions run: `26590800830`
- Job: `Milestone validation (windows-latest)`
- Job conclusion: `success`
- Completed: `2026-05-28T17:31:58Z`
- Successful steps included format check, clippy, tests, workspace check, dependency policy gate, `GUI Phase 8 smoke dry run (Windows)`, `GUI Phase 8 evidence gate`, and `Phase 8 evidence gate`.

## Evidence Handling Rules

- Archive command labels, exit status, platform, run id, and artifact checksums.
- Do not archive raw source, dirty buffers, terminal output bodies, transport frames, prompts, provider payloads, secrets, private keys, or signing credentials.
- Keep failed or missing future platform proof as `BLOCKED` until replaced by current passing evidence.
