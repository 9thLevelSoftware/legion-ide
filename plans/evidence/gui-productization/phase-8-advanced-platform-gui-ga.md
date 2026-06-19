# GUI Phase 8 advanced platform GUI GA evidence

## Acceptance Status

- Phase 8 acceptance: Accepted.

This document is GUI Phase 8 acceptance evidence for the advanced GUI GA productization track. It is separate from the accepted legacy Phase 8 runtime substrate evidence under `plans/evidence/phase-8/`.

## Scope

GUI Phase 8 covers production-grade GUI workflows for plugin management, collaboration, remote workspace status, delegated task command-center review, release/update/rollback/incident procedures, and cross-platform parity evidence.

The GUI track must preserve app/protocol/runtime authority boundaries. `legion-ui` remains projection-only, `legion-desktop` remains renderer and adapter-local state only, and mutation-capable plugin, collaboration, remote, delegated task, language, terminal, provider, storage, and security behavior remains proposal-mediated or policy-gated outside UI ownership.

## Required Artifacts

- `plans/evidence/gui-productization/phase-8-plugin-management.md`
- `plans/evidence/gui-productization/phase-8-collaboration-gui.md`
- `plans/evidence/gui-productization/phase-8-remote-workspace-gui.md`
- `plans/evidence/gui-productization/phase-8-delegated-task-command-center.md`
- `plans/evidence/gui-productization/phase-8-ga-release-runbook.md`
- `plans/evidence/gui-productization/phase-8-update-rollback-incident.md`
- `plans/evidence/gui-productization/phase-8-platform-parity.md`
- `plans/evidence/gui-productization/phase-8-final-gates.md`
- `.planning/phases/08-advanced-platform-gui-ga/08-01-RESULT.md`
- `.planning/phases/08-advanced-platform-gui-ga/08-02-RESULT.md`
- `.planning/phases/08-advanced-platform-gui-ga/08-03-RESULT.md`
- `.planning/phases/08-advanced-platform-gui-ga/08-04-RESULT.md`
- `.planning/phases/08-advanced-platform-gui-ga/08-05-RESULT.md`
- `.planning/phases/08-advanced-platform-gui-ga/08-06-RESULT.md`
- `.planning/phases/08-advanced-platform-gui-ga/08-07-RESULT.md`
- `scripts/gui-smoke.ps1`
- `scripts/gui-smoke.sh`

## Required Commands

- `cargo run -p xtask -- check-deps`
- `cargo fmt --all --check`
- `cargo check --workspace --all-targets`
- `cargo test --workspace --all-targets`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo deny check`
- `cargo test -p legion-desktop --test plugin_management -- --nocapture`
- `cargo test -p legion-desktop --test collaboration_gui -- --nocapture`
- `cargo test -p legion-desktop --test remote_workspace_gui -- --nocapture`
- `cargo test -p legion-desktop --test delegated_task_command_center -- --nocapture`
- `cargo run -p legion-cli -- evidence check --phase gui-phase8`
- `cargo run -p legion-cli -- evidence check --phase phase8`
- `powershell -ExecutionPolicy Bypass -File scripts/gui-smoke.ps1 -Help`
- `bash scripts/gui-smoke.sh --help`

## Supported Advanced GUI Surface Markers

- Plugin management GUI: supported
- Collaboration GUI: supported
- Remote workspace GUI: supported
- Delegated task command center: approval-gated
- Autonomous apply: unsupported

## Platform Parity Markers

- Platform parity: Windows
- Platform parity: macOS
- Platform parity: Linux
- Update rollback: documented
- Incident response: documented

## Final Validation Checklist

- [x] Plugin management GUI evidence is complete.
- [x] Collaboration GUI evidence is complete.
- [x] Remote workspace GUI evidence is complete.
- [x] Delegated task command-center evidence is complete and approval-gated.
- [x] GA release, update, rollback, incident, smoke, and platform parity evidence is complete.
- [x] GUI Phase 8 final gates passed and required command outputs are archived.
- [x] Phase 8 accepted status is recorded only after all required artifacts and commands pass.

## Residual Risks

- The accepted legacy runtime substrate evidence under `plans/evidence/phase-8/` remains valid but is not a substitute for GUI productization GA evidence.
- Phase 8 does not enable autonomous apply. Delegated-task runtime activation beyond metadata-only planning is a post-GA track and remains proposal-mediated, isolated, and approval-gated.
- No hosted CI workflow is currently configured; local repository gates and GUI evidence commands are the active verification source.
