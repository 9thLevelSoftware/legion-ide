# ADR-0024: Remote Execution Boundary

## Status

Accepted for Phase 7 bounded remote execution descriptors.

## Context

Phase 7 includes remote process, PTY, terminal, LSP, and semantic-query behavior only inside the edge workspace agent scope. Older terminal-focused Phase 7 labels are historical and do not authorize standalone local terminal runtime.

## Decision

Remote process, PTY, LSP, and semantic-query behavior is represented as protocol descriptors handled by `legion-remote`. The accepted implementation validates capabilities, session identity, cancellation tokens where applicable, output/transcript limits, schema versions, and metadata-only redaction. It does not spawn local processes, launch standalone terminal crates, persist raw transcripts, persist raw process output, or apply LSP edits directly.

LSP, format, code action, terminal-triggered mutation, and remote filesystem mutation outputs must be lowered into proposal-mediated paths before local durable state can change. The current Phase 7 implementation tests the remote descriptor boundary and app-owned audit bridge; production process isolation and broad terminal/LSP product surfaces remain later hardening work.

## Rejected Alternatives

- Standalone local terminal activation: rejected as outside the current authoritative Phase 7 remote-development scope.
- Direct LSP or terminal mutation: rejected because all non-user-direct mutation must flow through proposals.
- Unbounded transcript/output retention: rejected because durable audit remains metadata-only.

## Consequences

- Remote execution tests cover process, PTY, LSP, semantic query, cancellation-token, capability, and bounded-output validation.
- App integration confirms remote fixture writes cannot bypass local disk/workspace authority.
