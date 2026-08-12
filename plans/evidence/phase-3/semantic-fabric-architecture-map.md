# Semantic Fabric Architecture Map

Date: 2026-05-24

Accepted Phase 3 authority flow:

```text
WorkspaceActor discovery authority
  -> WorkspaceDiscoverySnapshot / WorkspaceDiscoveryDelta DTOs
  -> RepositoryDiscoveryImporter
  -> SemanticFabricScheduler / IndexingActor
  -> SourceDocument descriptors, changed ranges, or snapshot lease chunks
  -> SyntaxTreeCache with identity, grammar, schema, descriptor, and privacy keys
  -> LexicalIndexer / ParserWorker semantic extraction
  -> SemanticIndex query DTOs and SemanticMetadataBatch persistence
  -> UI projections and proposal previews only

LSP supervision DTOs
  -> LspLaunchPolicyDecision / LspSupervisionEvent
  -> LspResultMetadata and feature DTOs
  -> convert_lsp_edit_to_workspace_proposal for edits
  -> AppComposition proposal execution authority
```

Accepted ownership boundaries:

- `legion-index` owns queue state, parser cache state, semantic records, query results, and metadata-only scheduling decisions.
- `legion-project` remains the workspace discovery, file identity, fingerprint, and VFS authority.
- `legion-editor` remains the buffer, snapshot, text transaction, and undo authority.
- `legion-app` remains the proposal lifecycle, apply, audit-before-success, and rollback orchestrator.
- `legion-ui` remains projection-only and receives snapshots/intents, not editor sessions or mutation authority.
- LSP supervision records are metadata-only; edit-producing responses become proposals before any mutation.

Accepted non-blocking guarantees:

- Bounded `IndexingActor` queues reject or displace lower-priority work under pressure.
- Live snapshot work supersedes stale background work by priority, generation, file content version, and content hash.
- Query DTOs expose freshness, partial, stale, degraded, or unavailable state instead of blocking editor input or saves.
- Saves continue through proposal-mediated workspace preconditions and do not wait for semantic or LSP freshness.
