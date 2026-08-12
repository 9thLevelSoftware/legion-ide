# M6 — WS15.T4 Agent-Capability Marketplace Position Evidence

## Status

Accepted.

## Acceptance target

- Define the registry schema for the marketplace objects.
- Preserve a local install flow that stays app-owned and proposal-gated.
- Keep the full remote marketplace posture post-GA.

## Position

Legion’s 2026 marketplace primitive is not a generalized third-party extension store. The primary objects are:

- MCP servers, represented today by the protocol registry schema.
- Skills, represented as local capability artifacts that install into the app-owned surface.
- Plan templates, represented as app-owned template artifacts that can be resolved locally and then projected into workflows.

The local install flow is intentionally bounded: validate the artifact schema, resolve it locally, project it into the app-owned registry/workflow state, and require policy review for any capability-bearing activation. Remote marketplace distribution remains deferred to the post-GA track.

## What was verified

- `crates/legion-protocol/src/lib.rs`
  - `McpServerDescriptor` and `McpRegistrySnapshot` define the current registry schema for MCP marketplace objects.
  - `validate_mcp_registry_snapshot()` enforces non-empty ids and non-zero schema versions, so the registry format fails closed.
- `crates/legion-protocol/tests/dto_contracts.rs`
  - `dto_contracts_automate_mcp_registry_decision_feed_and_risk_rows_are_metadata_only()` exercises a full MCP registry snapshot and validates the schema contract.
- `crates/legion-plugin/src/lib.rs`
  - `PluginRuntimeHost::load_manifest()` is the local install/activation path for app-owned capability manifests.
  - The trusted-manifest regression test covers the happy path for local manifest loading.
- `crates/legion-app/src/lib.rs`
  - `AppComposition::load_manifest()` loads a manifest into the app-owned plugin runtime and registers tree-sitter grammar contributions.
  - `seed_mcp_registry()` stores validated registry snapshots locally, and the Legion workflow projection uses those snapshots when it renders the Automate view.
- `crates/legion-desktop/src/view.rs`
  - The workflow view renders MCP registry summaries from the projection layer.
- `crates/legion-desktop/tests/legion_workflow_command_center.rs`
  - The Automate command-center contract test proves the UI exposes `legion workflow mcp registry` rows from the projection.

## Verification commands

```bash
cargo test -p legion-protocol --test dto_contracts dto_contracts_automate_mcp_registry_decision_feed_and_risk_rows_are_metadata_only -- --nocapture
cargo test -p legion-plugin plugin_runtime_loads_trusted_manifest -- --nocapture
cargo test -p legion-desktop legion_workflow_automate_rows_show_mcp_decisions_risk_kill_and_permissions -- --nocapture
```

## Results

- `cargo test -p legion-protocol --test dto_contracts dto_contracts_automate_mcp_registry_decision_feed_and_risk_rows_are_metadata_only -- --nocapture`
  - 1 test passed.
- `cargo test -p legion-plugin plugin_runtime_loads_trusted_manifest -- --nocapture`
  - 1 test passed.
- `cargo test -p legion-desktop legion_workflow_automate_rows_show_mcp_decisions_risk_kill_and_permissions -- --nocapture`
  - 1 test passed.

## Notes

- This card is a position and schema-definition slice, not a full marketplace launch.
- The repo already has the local install primitives needed for the current position; the post-GA remote marketplace remains intentionally deferred.
