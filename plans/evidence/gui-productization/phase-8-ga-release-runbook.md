# GUI Phase 8 GA release runbook

## Status

- GA release readiness: not accepted.
- Signing status: unsigned local and CI dry-run evidence only; no signing keys, certificate material, notarization proof, or signed installer artifact is present.
- Update rollback: documented.
- Incident response: documented.

This runbook covers the GUI Phase 8 productization track only. It does not replace the accepted legacy runtime substrate evidence under `plans/evidence/phase-8/`.

## Source Evidence

- Plugin management GUI: supported in `phase-8-plugin-management.md`.
- Collaboration GUI: supported in `phase-8-collaboration-gui.md`.
- Remote workspace GUI: supported in `phase-8-remote-workspace-gui.md`.
- Delegated task command center: approval-gated in `phase-8-delegated-task-command-center.md`.
- Phase 7 local beta: accepted in `phase-7-local-ide-beta.md`.

## Release Prerequisites

Before a GUI Phase 8 GA release can be promoted:

1. Run the repository gates from `AGENTS.md`: `cargo run -p xtask -- check-deps`, `cargo fmt --all --check`, `cargo check --workspace --all-targets`, `cargo test --workspace --all-targets`, `cargo clippy --workspace --all-targets -- -D warnings`, and `cargo deny check`.
2. Run the GUI Phase 8 focused gate: `cargo test -p devil-desktop plugin_management collaboration_gui remote_workspace_gui delegated_task_command_center -- --nocapture`.
3. Run both GUI and legacy evidence gates: `cargo run -p devil-cli -- evidence check --phase gui-phase8` and `cargo run -p devil-cli -- evidence check --phase phase8`.
4. Run smoke entrypoint checks: `powershell -ExecutionPolicy Bypass -File scripts/gui-smoke.ps1 -Help` and `bash scripts/gui-smoke.sh --help`.
5. Archive Windows, macOS, and Linux CI outputs that include the GUI Phase 8 smoke dry runs and the `gui-phase8` evidence gate.

## Signing And Packaging

- Do not claim signed installer support until a signed artifact, signer identity reference, checksum, and verification command are archived.
- Do not store signing keys, private keys, certificates, token values, or notarization credentials in this repository.
- A production release candidate must record artifact names, checksums, signer references, and verification commands without storing private material.

## Update Path

1. Publish the release candidate only after all release prerequisites pass.
2. Start with a canary audience that is limited to internal or explicitly opted-in users.
3. Verify launch, open workspace, plugin management rows, collaboration rows, remote workspace rows, delegated task command-center rows, proposal review, diagnostics export, and quit.
4. Keep update telemetry metadata-only: artifact id, version, platform, success/failure state, redaction hints, and correlation identifiers only.
5. Do not capture raw source, dirty buffer text, prompts, provider payloads, terminal output bodies, remote transport frames, or secrets.

## Rollback Path

Rollback is required when any release gate fails, the canary violates health thresholds, diagnostics redaction fails, or platform parity evidence is incomplete.

1. Stop the canary rollout.
2. Remove the candidate from the promoted channel.
3. Re-promote the last accepted artifact or instruct users to relaunch the previous package.
4. Preserve metadata-only incident evidence: candidate id, platform, failed gate, rollback decision, and verification commands.
5. Re-run the GUI Phase 8 smoke and evidence gates against the restored artifact before closing the incident.

## Canary Criteria

- Minimum observation window: 15 minutes after candidate launch for standard-risk updates.
- Rollback trigger: smoke failure, launch failure, evidence gate failure, redaction failure, proposal authority bypass, direct UI/desktop mutation finding, or platform-specific regression.
- Promotion trigger: all smoke checks pass, metadata-only diagnostics remain redacted, and no platform has unresolved blocked parity evidence.

## Incident Response

1. Declare an incident if a candidate ships with failed gates, missing parity proof, raw diagnostic capture, proposal bypass, direct mutation outside app authority, or signing ambiguity.
2. Assign an incident owner and a release owner.
3. Capture command outcomes and decision records only; do not paste raw logs or payload bodies.
4. Roll back first when user data, privacy, or update integrity is at risk.
5. Record remediation, rerun gates, and update this evidence bundle before any renewed GA claim.

## Acceptance Criteria

GUI Phase 8 GA release readiness remains blocked until:

- Plugin, collaboration, remote workspace, and delegated task GUI evidence remain supported or approval-gated.
- GUI Phase 8 smoke scripts and CI reference the `gui-phase8` evidence gate.
- Windows, macOS, and Linux parity evidence is archived from current CI or equivalent platform runs.
- Signing status is explicitly evidenced or explicitly excluded from the release claim.
- Final repository gates and evidence gates pass from the current checkout.
