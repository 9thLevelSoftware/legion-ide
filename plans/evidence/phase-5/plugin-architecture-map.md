# Phase 5 Plugin Architecture Map

## Acceptance Status

- Phase 5 acceptance: Accepted.
- Runtime surface status: `legion-plugin` boundary behavior is active for manifest-validated, capability-checked, quota-bound, metadata-only, app-composed plugin command invocation.

## Architecture Map

- `legion-protocol` owns versioned plugin manifests, contributions, host-call envelopes, storage DTOs, denial reasons, validation helpers, projection DTOs, and `PluginPort`.
- `legion-security` owns deny-by-default plugin capability decisions requiring plugin id, namespace, manifest id, module hash, declared capability, and host-call context.
- `legion-plugin` owns manifest loading, lifecycle state, host-call quota checks, typed fail-closed responses, and protocol-only runtime port APIs.
- `legion-storage` owns plugin-scoped metadata storage, namespace isolation, metadata validation, and quota accounting.
- `legion-observability` owns metadata-only plugin event envelope construction and validation.
- `legion-app` is the plugin composition root for loading manifests and dispatching plugin command host-call envelopes.
- `legion-ui` remains projection-only with plugin contribution projections and command intents.

## Governance Prerequisites

- ADR: `plans/adrs/ADR-0019-wasm-plugin-runtime.md`
- Dependency policy: `plans/dependency-policy.md`
- Runtime crate: `crates/legion-plugin`
- App composition: `crates/legion-app/src/lib.rs`

## Runtime Lifecycle

- Manifest validation rejects zero plugin ids, missing names, invalid ABI ranges, ABI mismatches, missing hashes, namespace mismatches, and raw payload markers.
- Host-call dispatch rejects unloaded plugins, undeclared capabilities, missing manifest/host-call context, zero correlation ids, nil causality ids, zero event sequences, quota exhaustion, and untrusted workspaces.
- Plugin command invocation enters the app as `CommandDispatchIntent::InvokePluginCommand`, routes to `AppCommandRequest::InvokePluginCommand`, and dispatches through `PluginRuntimeHost` with protocol DTOs only.
- Plugin storage persists metadata-only records under workspace id, plugin id, namespace, key, schema version, retention label, redaction hint, and byte count.

## Expected Evidence Artifacts

- `plugin-architecture-map.md`
- `dependency-boundary.txt`
- `wasm-abi-contract-tests.txt`
- `manifest-golden-tests.txt`
- `host-call-capability-tests.txt`
- `sandbox-denial-tests.txt`
- `plugin-crash-isolation-tests.txt`
- `plugin-storage-quota-tests.txt`
- `plugin-proposal-routing-tests.txt`
- `plugin-observability-redaction-audit.md`
- `future-surface-deferral-audit.md`
- `cargo-fmt-check.txt`
- `cargo-check-workspace-all-targets.txt`
- `cargo-test-workspace-all-targets.txt`
- `cargo-clippy-workspace-all-targets.txt`

## Final Validation Checklist

- [x] ADR accepted and referenced by dependency policy.
- [x] Protocol DTO contracts and golden tests pass.
- [x] Security capability-denial tests pass.
- [x] Runtime sandbox/no-ambient-authority tests pass.
- [x] Storage namespace/quota tests pass.
- [x] Observability metadata-only redaction tests pass.
- [x] App proposal-routing tests pass.
- [x] UI projection-only tests pass.
- [x] Global workspace gates pass and evidence artifacts are captured.
