# ADR-0020: Collaboration Operation Model

## Status

Draft for Phase 6 implementation review.

## Context

Phase 6 targets real-time multiplayer collaboration while preserving editor ownership, proposal-mediated durable writes, and metadata-only audit defaults. Existing protocol symbols were scaffolding only and did not define collaboration runtime contracts.

## Decision

Use an operation-log collaboration model for the first accepted Phase 6 slice. Operations carry deterministic identifiers, participant-local sequence numbers, base version vectors, document epochs, snapshot and buffer preconditions, author principal metadata, capability decisions, correlation IDs, causality IDs, and redaction hints. Runtime activation remains blocked until convergence, dirty-buffer, ownership, and replay tests exist.

## Rejected Alternatives

- Full CRDT runtime in the first slice: deferred because it expands correctness and retention scope before governance and contract evidence exist.
- Whole-snapshot replacement: rejected because it risks dirty-buffer clobbering and raw-source retention.
- UI-owned collaboration state: rejected because UI must remain projection-only.

## Consequences

- Collaboration operation payloads may be source-bearing in memory but durable records default to metadata-only summaries.
- Causal gaps, stale epochs, missing capability decisions, zero correlation IDs, and nil causality IDs are fail-closed conditions.
- Durable file changes produced by collaborative decisions still route through proposal/workspace save preconditions.
