# ADR-0018: LSP Runtime Supervision

## Status

Accepted for Phase 3 governance and implementation gating.

## Context

Phase 3 of [`implementation-plan.md`](../implementation-plan.md:246) requires LSP fusion with the semantic fabric: diagnostics, completions, hover, definitions, references, rename, formatting, semantic tokens, and code actions must flow through normalized protocol DTOs and must not create direct editor, UI, or workspace mutation paths.

The proposal substrate accepted in [`ADR-0016-generalized-proposal-service.md`](ADR-0016-generalized-proposal-service.md:1) is the only allowed route for mutations produced by LSP features. The streaming text substrate accepted in [`ADR-0015-streaming-text-viewport.md`](ADR-0015-streaming-text-viewport.md:1) supplies snapshot descriptors, chunk leases, and non-blocking event fanout. LSP supervision must consume those contracts without blocking editor input or saves.

## Decision

Phase 3 LSP runtime work will use a supervised, cancellable, backpressured worker model. Runtime workers communicate through protocol DTOs, publish metadata-first results, and route every mutation through the proposal substrate.

### 1. Supervision boundaries

- LSP runtime ownership is outside UI rendering and outside editor text ownership. UI may render LSP-derived projections, but it never owns server processes, document sync state, editor sessions, text mutation, or workspace writes.
- A coordinator supervises per-language or per-root workers, process IO, capability negotiation, health state, restart budgets, request routing, and bounded shutdown.
- Workers own JSON-RPC request state, server lifecycle state, capability cache, and in-flight cancellation state. They do not own buffers, workspace VFS authority, or proposal application.
- Cross-domain interactions use protocol DTOs and ports. Any missing diagnostics, completion, hover, definition, reference, rename, formatting, or code-action DTOs must be added to the protocol contract before runtime behavior depends on them.
- Server launch and document synchronization remain trust-gated and policy-scoped. Logs and observability retain metadata, identifiers, ranges, durations, and status, not full source by default.

### 2. Cancellable LSP operations

- Every request carries request identity, workspace identity, file identity, buffer identity, snapshot identity, buffer version, language identifier, correlation identifier, causality identifier, timeout budget, and cancellation token.
- Operations are cancelled when the user cancels, a newer snapshot supersedes the request, document sync state changes incompatibly, the timeout budget expires, the server restarts, workspace trust is revoked, or shutdown begins.
- Completion, hover, definition, reference, formatting, code-action, rename, semantic-token, and diagnostics-refresh requests must tolerate stale responses. Stale responses are discarded or published only as stale metadata.
- Cancellation is propagated to the language server when supported and is recorded locally even when the server cannot honor cancellation.

### 3. DTO flow for language features

- Diagnostics flow from server notifications into diagnostic DTOs, then into editor overlays, semantic fabric graph links, and metadata-only observability. Diagnostics are keyed by document version, snapshot identity, and content hash so obsolete diagnostics can be suppressed.
- Completion requests use protocol DTOs for position, trigger context, snapshot identity, and privacy scope. Responses return bounded completion items, ranking metadata, source freshness, and server capability provenance.
- Hover, definition, and reference requests use normalized positions and ranges. Results return symbol locations, bounded display content, provenance, and freshness metadata suitable for UI navigation and semantic graph enrichment.
- Rename, formatting, and code-action requests produce proposal payloads or proposal previews rather than direct edits. Workspace edits from a server are translated into proposal DTOs with version preconditions before user approval or application.
- Code-action DTOs separate command-only actions, edit-producing actions, and mixed actions. Command-only actions are denied or policy-routed until a later command execution surface is accepted.
- Formatting responses are treated as edit proposals with target coverage and preconditions. They must not overwrite buffers or disk outside the proposal workflow.

### 4. Backpressure and scheduling

- LSP request queues are bounded per server, per workspace, and per feature class.
- Priority favors visible open buffers and direct user requests over background diagnostics, semantic tokens, prefetch, and repository-wide reference scans.
- Under pressure, the runtime coalesces diagnostics, cancels stale completion and hover requests, delays background refreshes, returns degraded status, and refuses low-priority work before it can affect editor latency.
- Queue saturation must not block editor input, viewport projection, proposal validation, or save workflows.
- Supervisors expose health and pressure state so the semantic fabric and UI projections can mark LSP data as delayed, stale, degraded, or unavailable.

### 5. Timeout behavior and failure handling

- Each operation class has a timeout budget selected by feature type and interaction criticality.
- Timeout results are typed outcomes, not panics and not blocking retries. The caller receives timeout, stale, degraded, or unavailable status with correlation metadata.
- Repeated timeouts or process failures trigger bounded restart policy. Restart storms trip a circuit breaker and mark the server unavailable until explicit recovery policy allows another attempt.
- Server crashes, malformed responses, protocol violations, and unsupported capabilities are isolated to the supervised worker and must not corrupt editor state, save state, or index state.

### 6. Non-blocking editor and save constraints

- Editor transactions do not wait for LSP request completion, diagnostics refresh, semantic token generation, or document sync acknowledgement.
- Saves do not wait for LSP formatting, diagnostics, server health, or semantic graph freshness. Saves remain governed by proposal preconditions, workspace fingerprints, content versions, capability policy, correlation, causality, and fail-closed write behavior.
- Document sync lag produces stale or unavailable LSP results rather than blocking keystrokes or saves.
- LSP output may enrich viewport overlays and semantic graph records after the fact, but stale output must not overwrite newer editor or index state.

### 7. Mutation routing through proposals

- Rename, formatting, organize imports, quick fixes, refactor actions, and workspace edits from language servers are mutation proposals.
- LSP workers translate edit-producing responses into [`WorkspaceProposal`](../../crates/devil-protocol/src/lib.rs:1343) payloads with explicit target coverage, version preconditions, privacy metadata, capability requirements, rollback expectations, and preview summaries.
- The proposal service validates, previews, approves, applies, rejects, cancels, or rolls back these proposals. LSP workers never apply edits directly to buffers or disk.
- If a proposal cannot express the server response safely, the action is denied with metadata-only diagnostics rather than applying a partial edit.

### 8. Semantic fabric integration

- LSP diagnostics, completions, hover facts, definitions, references, semantic tokens, and code-action metadata feed the semantic fabric as versioned, invalidatable records.
- Records are invalidated by content hash, snapshot identity, document version, grammar version where syntax-derived, model version where ranking-derived, and privacy scope.
- LSP data is enrichment, not sole authority. Lexical and syntax records can remain available while LSP is unavailable, and LSP records must carry freshness and capability provenance.

## Consequences

- **Positive**: LSP capabilities can be integrated without regressing projection-only UI, editor ownership, or proposal-mediated mutation.
- **Positive**: Cancellable operations and bounded queues keep editor input and save workflows responsive under slow or failing servers.
- **Positive**: LSP output can enrich the semantic graph while remaining invalidatable and privacy-scoped.
- **Negative**: Some LSP features will surface degraded or unavailable states until protocol DTOs, workers, and proposal translation are implemented and validated.
- **Negative**: Command-like code actions remain denied or metadata-only until a separate policy-gated command execution surface exists.

## Non-goals

- This ADR does not implement Rust source, manifests, tests, build scripts, or a new runtime crate.
- This ADR does not allow LSP workers to mutate buffers or workspace files directly.
- This ADR does not require saves or editor input to wait for LSP state.
- This ADR does not activate terminal, plugin, AI agent, collaboration, or remote runtime surfaces.

## Exit condition

This ADR is satisfied when Phase 3 implementation demonstrates supervised LSP workers, cancellable operations, bounded queues, timeout behavior, normalized DTO flow for diagnostics, completion, hover, definition, reference, rename, formatting, and code actions, proposal-only mutation routing, non-blocking editor and save behavior, and evidence captured in [`predictive-semantic-fabric.md`](../evidence/phase-3/predictive-semantic-fabric.md:1).
