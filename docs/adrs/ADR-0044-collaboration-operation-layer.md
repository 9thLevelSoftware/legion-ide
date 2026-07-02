# ADR-0044: Collaboration Operation Layer Sits on the Accepted Collaboration Substrate

## Status
Accepted — ratified for P9.F3.T1.

## Context
Legion already has an accepted collaboration substrate in `legion-collaboration`: a deterministic, metadata-first operation log and replay runtime that handles collaboration state, presence, acknowledgements, and conflict handling. ADR-0040 ratified the stable position / operation-log substrate over `legion-text` and explicitly deferred the full CRDT choice, while ADR-0041 later ratified the anchor-layer strategy over the same existing runtime instead of adopting an external CRDT crate.

This task needs the collaboration lane to be explicit about what sits above that substrate. The missing decision was whether collaboration should introduce a separate CRDT authority layer, or whether Legion should keep a focused operation layer that composes through the already-accepted collaboration runtime and protocol DTOs.

## Decision
Legion ratifies a collaboration operation layer that composes through the existing collaboration substrate rather than replacing it with a separate CRDT authority layer.

Concretely:

- `legion-collaboration` remains the accepted collaboration substrate for deterministic operation replay, presence, acknowledgements, and fail-closed conflict handling.
- The collaboration operation layer stays protocol-mediated and sits between editor transactions and downstream collaboration consumers.
- `legion-protocol` remains the contract boundary for collaborator identity, version vectors, operation preconditions, and operation records.
- `legion-ui` stays projection-only and does not own collaboration authority or CRDT state.
- No new external CRDT dependency is authorized by this ADR.

## Why this decision
1. The repository already has an accepted collaboration runtime, so the honest decision is how to compose with it, not whether to invent a second authority layer.
2. Keeping the operation layer on top of the existing substrate preserves the proposal-mediated mutation model and keeps collaboration behavior fail-closed.
3. The accepted anchor / operation decisions already cover the stable-position and replay requirements that collaboration depends on.
4. Deferring a separate CRDT runtime avoids unnecessary churn until a workload-backed gap is demonstrated.

## Consequences
- Collaboration features should build on `legion-collaboration` and `legion-protocol` rather than bypassing them.
- Any future move to a vendor CRDT or a different authority layer will require a new ADR and a dependency-policy update.
- UI and editor surfaces remain projections over collaboration state, not owners of collaboration truth.
- The collaboration lane can proceed with protocol and runtime integration work without reopening the substrate decision.
