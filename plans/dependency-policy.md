# Dependency Policy for Devil IDE v0.1

## Scope

This document defines the required crate dependency direction used by `cargo run -p xtask -- check-deps` during milestone-gate validation.

## Rules

### 1. Directional Intent

- `devil-ai` may depend on:
  - `devil-protocol`
  - `devil-security`
  - `serde`, `serde_json`, `thiserror`
- `devil-ai` MUST NOT depend on `devil-ai-providers`.

- `devil-ai-providers` may depend on:
  - `devil-ai`
  - `devil-protocol`
  - `devil-security`

- `devil-editor` may depend on:
  - `devil-text`
  - `devil-protocol`

- `devil-editor` MUST NOT depend on `devil-project`.

- `devil-ui` may depend on:
  - `devil-editor`
  - `devil-protocol`

- `devil-platform` may depend on:
  - `devil-protocol`
  - `thiserror`

### 2. Shared Contracts Boundary

- Cross-domain project/editor/indexer/tracker interactions should flow through `devil-protocol` types and traits.
- The following boundary API symbols are authoritative in `devil-protocol`:
  - `ProjectId`
  - `WorkspaceId`
  - `WorkspaceRootId`
  - `SnapshotId`
  - `BufferId`
  - `FileId`
  - `BufferVersion`
  - `FileContentVersion`
  - `WorkspaceGeneration`
  - `TerminalSessionId`
  - `ProposalId`
  - `CorrelationId`
  - `LanguageServerId`
  - `PluginId`
  - `CapabilityDecisionId`
  - `EventSequence`
  - `PrincipalId`
  - `CapabilityId`
  - `CapabilityNamespace`
  - `CanonicalPath`
  - `LanguageId`
  - `TimestampMillis`
  - `ByteRange`
  - `ByteOffset`
  - `Utf16Offset`
  - `TextCoordinateEncoding`
  - `TextOffset`
  - `TextRange`
  - `WorkspaceTrustState`
  - `WorkspaceOpenRequest`
  - `WorkspaceOpened`
  - `WorkspaceCloseRequest`
  - `WorkspaceClosed`
  - `FileIdentity`
  - `FileKind`
  - `FileMetadata`
  - `FileTreeNode`
  - `FileTreeDeltaOp`
  - `FileTreeDelta`
  - `WatcherEventKind`
  - `WatcherEvent`
  - `WorkspaceConfigSnapshot`
  - `FileConflictState`
  - `BufferLifecycleKind`
  - `BufferLifecycle`
  - `SnapshotDescriptor`
  - `TransactionSource`
  - `TextEdit`
  - `EditBatch`
  - `TextTransactionDescriptor`
  - `UndoGroup`
  - `OverlaySeverity`
  - `DiagnosticOverlay`
  - `CompletionRequest`
  - `CompletionItem`
  - `WorkspaceEditProposal`
  - `ProposalVersionPreconditions`
  - `VersionContext`
  - `PreviewSummary`
  - `WorkspaceProposal`
  - `ProposalPayload`
  - `TextEditProposal`
  - `CreateFileProposal`
  - `DeleteFileProposal`
  - `RenameFileProposal`
  - `SaveFileProposal`
  - `FormatFileProposal`
  - `CodeActionProposal`
  - `TerminalCommandProposal`
  - `LspServerStatus`
  - `LanguageServerConfig`
  - `LspSyncKind`
  - `DocumentSyncState`
  - `LspDiagnosticSeverity`
  - `LspDiagnostic`
  - `DiagnosticSet`
  - `Hover`
  - `LspCompletionRequest`
  - `LspCompletionResponse`
  - `LspFormattingRequest`
  - `LspFormattingResponse`
  - `SemanticToken`
  - `SemanticTokenSet`
  - `SymbolLocation`
  - `LspCodeActionRequest`
  - `LspCodeAction`
  - `LspCodeActionResponse`
  - `TerminalSessionState`
  - `TerminalLaunchRequest`
  - `TerminalOutput`
  - `TerminalInput`
  - `TerminalResize`
  - `TerminalExit`
  - `TerminalCapability`
  - `PluginManifest`
  - `PluginActivationEvent`
  - `PluginCommandDescriptor`
  - `ContributionDescriptor`
  - `PluginStateNamespace`
  - `CapabilityGrant`
  - `CapabilityDenial`
  - `CapabilityDecision`
  - `PluginActionProposal`
  - `ContextProviderDescriptor`
  - `ProjectView`
  - `BufferOpened`
  - `ProjectInfoQuery`
  - `ProjectInfo`
  - `ProjectServiceError`
  - `EditorTransactionEvent`
  - `ProtocolError`
  - `ProtocolResult`
  - `WorkspaceRequest`
  - `WorkspaceResponse`
  - `EditorRequest`
  - `EditorResponse`
  - `ProposalRequest`
  - `ProposalResponse`
  - `TerminalRequest`
  - `TerminalResponse`
  - `LspRequest`
  - `LspResponse`
  - `PluginRequest`
  - `PluginResponse`
  - `CapabilityRequest`
  - `CapabilityResponse`
  - `EventEnvelope`
  - `EventSinkRequest`
  - `StorageRepositoryRequest`
  - `StorageRepositoryResponse`
  - `WorkspacePort`
  - `EditorPort`
  - `ProposalPort`
  - `TerminalPort`
  - `LspPort`
  - `CapabilityBrokerPort`
  - `EventSinkPort`
  - `StorageRepositoryPort`
  - `ProjectInfoPort`

### 3. Forbidden/Deferred Edges (Milestone 0)

- Do not add hard edges from:
  - `devil-editor` -> `devil-project`
  - `devil-ui` -> feature crates beyond declared contracts
  - `devil-tracker` -> feature crates that are not storage-protocol mediated
  - `devil-memory` -> non-storage non-protocol feature domains without explicit planning

### 4. Enforcement

`xtask check-deps` reads this policy and fails when forbidden edges are detected.
