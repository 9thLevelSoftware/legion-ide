# ADR-0020: Collaboration Operation Model

## Status

Accepted for Phase 6 local operation-log collaboration runtime.

## Context

Phase 6 targets real-time multiplayer collaboration while preserving editor ownership, proposal-mediated durable writes, and metadata-only audit defaults. Existing protocol symbols were scaffolding only and did not define collaboration runtime contracts.

## Decision

Use an operation-log collaboration model for the accepted Phase 6 slice. Operations carry deterministic identifiers, participant-local sequence numbers, base version vectors, document epochs, snapshot and buffer preconditions, author principal metadata, capability decisions, correlation IDs, causality IDs, and redaction hints.

The active runtime implementation is `legion-collaboration`. It remains isolated from app, UI, editor, project, remote, terminal, and process internals. Runtime application is default-off by configuration and becomes active only when constructed with an enabled `CollaborationRuntimeConfig` by an app-owned composition root. Durable workspace writes remain outside this runtime and continue through proposal/workspace save preconditions.

## Rejected Alternatives

- Full CRDT runtime in the first slice: deferred because it expands correctness and retention scope beyond the accepted operation-log evidence.
- Whole-snapshot replacement: rejected because it risks dirty-buffer clobbering and raw-source retention.
- UI-owned collaboration state: rejected because UI must remain projection-only.

## Consequences

- Collaboration operation payloads may be source-bearing in bounded in-memory transport/runtime payloads, but durable records default to metadata-only summaries.
- Causal gaps, stale epochs, missing capability decisions, zero correlation IDs, and nil causality IDs are fail-closed conditions.
- Durable file changes produced by collaborative decisions still route through proposal/workspace save preconditions.
