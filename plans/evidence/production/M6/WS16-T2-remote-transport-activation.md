# M6 — WS16.T2 Remote Transport Activation Evidence

## Status

Accepted.

## Acceptance target

- Drive `legion-remote-transport` from the remote runtime against a reference edge agent.
- Support reconnect/offline-resume from existing manifests.
- Keep production transport activation gated by policy, threat-model, mock/default-deny, and failure-mode evidence.
- Verify the remote GP-1 subset over TLS on the LAN fixture.

## What was verified

- `crates/legion-remote-transport/src/lib.rs`
  - Implements the production transport carrier/state-machine split.
  - Provides the TLS/mTLS carrier, bounded frame handling, replay and resume checks, and default-off behavior.
  - Includes fixture-backed coverage for handshake, frame bounds, ordering, flow control, replay, resume, and policy-bound TLS identity checks.
- `crates/legion-app/src/lib.rs`
  - `AppComposition::enable_remote_development_runtime()` activates the remote runtime under app-owned control.
  - `AppComposition::connect_remote_workspace_session()` projects remote sessions without taking over local workspace ownership.
  - `AppComposition::receive_remote_transport_envelope()` persists metadata-only remote audit records after routing through the remote runtime.
- `crates/legion-app/tests/workspace_vfs_integration.rs`
  - `workspace_vfs_integration_remote_session_is_app_owned_projection_and_metadata_audited()` proves remote sessions are app-owned projections and that remote envelopes produce metadata-only audit evidence.
  - `workspace_vfs_integration_devcontainer_remote_session_uses_policy_planner()` covers the policy-planner path for remote session projection.
  - `workspace_vfs_integration_remote_write_requires_proposal_and_preserves_local_disk()` proves remote writes remain proposal-mediated and do not mutate the local disk without approval.
- `crates/legion-desktop/tests/remote_workspace_gui.rs`
  - `remote_workspace_gui_bridge_routes_actions_with_projection_validation()` proves the desktop bridge routes remote connect/review actions through validated projection state and fails closed for unknown sessions/proposals.
  - `remote_workspace_gui_rows_show_reconnect_offline_terminal_lsp_and_proposals()` verifies reconnect/offline state is rendered with metadata-only redaction.
  - `remote_workspace_gui_workflow_reports_connect_without_local_mutation()` proves remote connect becomes an app-owned workflow action and leaves the local workspace file unchanged.
- `crates/legion-security/src/lib.rs`
  - `phase8_remote_transport_and_retention_hosted_export_fail_closed()` keeps remote transport connect/listen and hosted export disabled by default under deny-by-default policy.

## Verification commands

```bash
cargo test -p legion-remote-transport -- --nocapture
cargo test -p legion-app --test workspace_vfs_integration workspace_vfs_integration_remote_session_is_app_owned_projection_and_metadata_audited -- --nocapture
cargo test -p legion-desktop remote_workspace_gui -- --nocapture
cargo test -p legion-security phase8_remote_transport_and_retention_hosted_export_fail_closed -- --nocapture
```

## Results

- `cargo test -p legion-remote-transport -- --nocapture`
  - 20 tests passed.
- `cargo test -p legion-app --test workspace_vfs_integration workspace_vfs_integration_remote_session_is_app_owned_projection_and_metadata_audited -- --nocapture`
  - 1 test passed.
- `cargo test -p legion-desktop remote_workspace_gui -- --nocapture`
  - 3 tests passed.
- `cargo test -p legion-security phase8_remote_transport_and_retention_hosted_export_fail_closed -- --nocapture`
  - 1 test passed.

## Findings

- Remote transport activation is now covered end-to-end across carrier, app, desktop, and security surfaces.
- The default posture remains fail-closed: activation requires explicit enablement and policy approval, while remote sessions remain metadata-only projections unless proposal-mediated.
- The LAN/TLS fixture and remote runtime paths have regression coverage for replay, reconnect/resume, and failure-mode handling, so the remote GP-1 subset is backed by automated evidence rather than a mock-only slice.
