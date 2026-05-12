# ADR-0003: Define Editor Core Text Buffer, Rope, Transaction, and Snapshot Model

## Status
Accepted

## Context
The editor core must own all text state deterministically. AI patch application, undo/redo, diagnostics overlays, and indexing snapshots all depend on a stable, immutable-by-default text model. String-based buffers are insufficient for large files and high-frequency edits.

## Decision
Implement a rope-based text buffer in `devil-text` with immutable snapshots, explicit edit transactions, and deterministic undo/redo. Editor Core (`devil-editor`) consumes the rope and exposes typed edit APIs. Snapshots are content-addressed where practical for indexing correlation.

## Consequences
- **Positive**: O(log n) edits and copies for large files; snapshots can be shared safely across threads.
- **Positive**: AI-generated patches apply as explicit transactions, making them reversible and auditable.
- **Negative**: Rope implementation must be carefully benchmarked against allocation patterns and cache behavior.
- **Negative**: Snapshot frequency and retention require memory budgeting to avoid unbounded growth.
