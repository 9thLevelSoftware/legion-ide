# GUI Phase 8 update, rollback, and incident drill evidence

## Status

- Update drill: documented from script and CI marker checks.
- Rollback drill: documented; no production rollback was executed.
- Incident response: documented.
- GA acceptance: blocked until final platform parity and repository gates are archived.

## Drill Scope

The drill verifies that GUI Phase 8 has explicit smoke entrypoints, CI evidence wiring, and operational rollback criteria. It does not publish an update, sign an installer, promote a channel, or run a production incident.

## Commands And Outcomes

| Command | Outcome |
|---|---|
| `powershell -ExecutionPolicy Bypass -File scripts/gui-smoke.ps1 -Help` | passed; help lists GUI phase-8 advanced surface defaults |
| `bash scripts/gui-smoke.sh --help` | passed; help lists GUI phase-8 advanced surface defaults |
| `powershell -ExecutionPolicy Bypass -File scripts/gui-smoke.ps1 -Phase8 -DryRun` | passed; printed a non-executing Phase 8 smoke cargo command |
| `bash scripts/gui-smoke.sh --phase-8 --dry-run` | passed; printed a non-executing Phase 8 smoke cargo command |

Additional verification commands are recorded in `08-06-RESULT.md` after the final 08-06 checks run.

## Update Decision Record

- Candidate promotion is not approved by this drill alone.
- The GUI Phase 8 CI workflow now includes Phase 8 smoke dry-run steps and `cargo run -p devil-cli -- evidence check --phase gui-phase8`.
- The accepted legacy `phase8` evidence gate remains in CI and was not removed.
- Metadata-only diagnostics rules remain required for any update evidence.

## Rollback Decision Record

Rollback is required if any of these occur:

- The GUI Phase 8 smoke dry run fails on Windows, macOS, or Linux.
- `cargo run -p devil-cli -- evidence check --phase gui-phase8` fails.
- `cargo run -p devil-cli -- evidence check --phase phase8` fails.
- A GUI surface bypasses app/protocol authority or proposal mediation.
- Diagnostics contain raw source, dirty buffer text, prompts, provider payloads, terminal output bodies, remote transport frames, secrets, or private keys.
- Signed release claims cannot be matched to signer, checksum, and verification evidence.

## Incident Checklist

1. Freeze promotion for the candidate.
2. Record failed command labels, exit status, platform, candidate id, and correlation id.
3. Roll back to the last accepted package or release channel.
4. Preserve metadata-only diagnostics and omit raw logs or payload bodies.
5. Re-run smoke, evidence, dependency, format, check, test, clippy, and deny gates before restoring promotion.
6. Update the platform parity artifact with either passing proof or explicit blocked status.

## Privacy And Safety Notes

- The drill records command names and pass/fail outcomes only.
- No release secrets, signing keys, raw terminal output bodies, remote payload bodies, source text, prompts, or user data are stored here.
- Delegated task behavior remains approval-gated; autonomous apply remains unsupported.
