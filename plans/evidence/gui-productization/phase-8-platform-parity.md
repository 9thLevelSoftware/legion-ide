# GUI Phase 8 platform parity evidence

## Status

- Platform parity: Windows - evidenced locally on 2026-05-27.
- Platform parity: macOS - BLOCKED; no current macOS runner output is archived in this local execution.
- Platform parity: Linux - BLOCKED; no current Linux runner output is archived in this local execution.
- GA platform parity acceptance: BLOCKED until Windows, macOS, and Linux evidence is present from current CI or equivalent platform runs.

## Windows Evidence

Platform parity: Windows

| Check | Outcome |
|---|---|
| `powershell -ExecutionPolicy Bypass -File scripts/gui-smoke.ps1 -Help` | passed |
| `powershell -ExecutionPolicy Bypass -File scripts/gui-smoke.ps1 -Phase8 -DryRun` | passed |

The Windows evidence proves the PowerShell smoke wrapper exposes GUI phase-8 help and can build a non-executing advanced-surface smoke command with Phase 8 evidence, session state, and diagnostics defaults.

## macOS Evidence

Platform parity: macOS

Status: BLOCKED.

Required proof is a current `macos-latest` CI or equivalent platform run showing:

- `sh scripts/gui-smoke.sh --phase-8 --dry-run`
- `cargo run -p devil-cli -- evidence check --phase gui-phase8`
- repository gates required by `AGENTS.md`

## Linux Evidence

Platform parity: Linux

Status: BLOCKED.

Required proof is a current `ubuntu-latest` CI or equivalent platform run showing:

- `sh scripts/gui-smoke.sh --phase-8 --dry-run`
- `cargo run -p devil-cli -- evidence check --phase gui-phase8`
- repository gates required by `AGENTS.md`

## Cross-Platform Decision

Do not mark GUI Phase 8 accepted from this local Windows run alone. The shell help and dry-run checks passed locally through `bash`, but that is not Linux or macOS OS parity proof.

## Evidence Handling Rules

- Archive command labels, exit status, platform, run id, and artifact checksums.
- Do not archive raw source, dirty buffers, terminal output bodies, transport frames, prompts, provider payloads, secrets, private keys, or signing credentials.
- Keep failed or missing platform proof as `BLOCKED` until replaced by current passing evidence.
