# ADR-0019: WASM Plugin Runtime Boundary

Status: Accepted
Date: 2026-05-25

## Context

Phase 5 activates a WASM isolated extension ecosystem. Existing protocol placeholders did not define ABI versioning, manifest trust, sandbox authority, quotas, plugin storage, or metadata-only audit requirements.

## Decision

Plugins are loaded only through `devil-plugin`, which exposes protocol DTOs and never depends on app, UI, editor, project, AI, tracker, memory, terminal, collaboration, remote, or LSP internals. The initial accepted ABI version is `1`.

Plugin manifests must declare plugin identity, semver, ABI range, module hash, manifest id, trust/signature metadata, activation events, declarative contributions, requested capabilities, storage namespace, quota limits, and schema version. Manifest validation fails closed for missing identity, invalid ABI, namespace mismatch, forbidden raw payload markers, untrusted decision, or ABI mismatch.

The runtime has no ambient filesystem preopens, inherited environment, process launch, network authority, terminal authority, editor/workspace ownership, or direct storage authority. Host interactions are data-only `PluginHostCallRequest` envelopes checked by `devil-security` against manifest context and declared capabilities.

Plugin outputs that can mutate workspace state must be represented as proposal creation host calls and routed through existing generalized proposal authorities. UI receives only `PluginContributionProjection` data and emits `InvokePluginCommand` intents.

Plugin storage is scoped by workspace id, plugin id, namespace, key, schema version, retention label, redaction hint, and byte count. Storage rejects namespace escapes, quota overflow, raw source markers, secrets, prompts, provider payloads, and unbounded output markers.

Observability uses existing event envelope validation and metadata-only redaction. Plugin events require non-zero schema, correlation, causality, and sequence metadata.

## Consequences

The Phase 5 runtime slice can be tested without granting host authority or adding app/UI ownership edges. A future Wasmtime/WASI engine may be added only after supply-chain review and evidence proves equivalent no-ambient-authority behavior.
