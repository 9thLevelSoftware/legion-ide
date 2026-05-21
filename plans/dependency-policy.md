# Dependency Policy for Devil IDE v0.1

## Scope

This document defines the required internal crate dependency direction and runtime-surface activation gates used by `cargo run -p xtask -- check-deps` during milestone-gate validation.

## Rules

### 1. Directional Intent

Every current workspace crate must have an explicit internal dependency policy entry, even when the allowed internal dependency set is empty.

- `xtask` may depend on:

- `devil-protocol` may depend on:

- `devil-observability` may depend on:
  - `devil-protocol`

- `devil-security` may depend on:
  - `devil-protocol`

- `devil-text` may depend on:
  - `devil-protocol`

- `devil-platform` may depend on:
  - `devil-protocol`

- `devil-platform` MUST directly depend on:
  - `devil-protocol`

- `devil-storage` may depend on:
  - `devil-observability`
  - `devil-protocol`
  - `devil-security`

- `devil-project` may depend on:
  - `devil-observability`
  - `devil-platform`
  - `devil-protocol`
  - `devil-security`

- `devil-editor` may depend on:
  - `devil-observability`
  - `devil-protocol`
  - `devil-text`

- `devil-editor` MUST directly depend on:
  - `devil-protocol`
  - `devil-text`

- `devil-editor` MUST NOT depend on `devil-project`.

- `devil-ui` may depend on:
  - `devil-protocol`

- `devil-ui` MUST directly depend on:
  - `devil-protocol`

- `devil-ui` MUST NOT depend on `devil-app`.
- `devil-ui` MUST NOT depend on `devil-editor`.
- `devil-ui` MUST NOT depend on `devil-project`.
- `devil-ui` MUST NOT depend on `devil-storage`.

- `devil-app` may depend on:
  - `devil-editor`
  - `devil-observability`
  - `devil-platform`
  - `devil-project`
  - `devil-protocol`
  - `devil-security`
  - `devil-storage`
  - `devil-ui`

- `devil-ai` may depend on:
  - `devil-protocol`
  - `devil-security`

- `devil-ai` MUST directly depend on:
  - `devil-protocol`
  - `devil-security`

- `devil-ai-providers` may depend on:
  - `devil-ai`
  - `devil-protocol`
  - `devil-security`

- `devil-ai-providers` MUST directly depend on:
  - `devil-ai`

- `devil-index` may depend on:
  - `devil-protocol`
  - `devil-storage`
  - `devil-text`

Phase 3 semantic fabric activation for `crates/devil-index/Cargo.toml` is limited to the three internal dependencies listed above. No other internal crate edge is authorized for `crates/devil-index/Cargo.toml` while activating actor-owned indexing, lexical maps, tree-sitter syntax caches, normalized graph records, semantic query APIs, and LSP fusion. Repository, editor, workspace, app, and UI facts must cross through protocol DTOs, text snapshot contracts, storage metadata, or proposal-mediated workflows rather than direct crate coupling. This policy entry does not authorize vector indexing, embeddings, model-provider dependencies, or direct mutation of buffers and workspaces.

- `devil-tracker` may depend on:
  - `devil-protocol`
  - `devil-storage`

- `devil-memory` may depend on:
  - `devil-protocol`
  - `devil-storage`

- `devil-agent` may depend on:
  - `devil-ai`
  - `devil-protocol`
  - `devil-tracker`

- `devil-cli` may depend on:
  - `devil-index`
  - `devil-protocol`
  - `devil-storage`

The planned runtime surfaces below are policy placeholders only. They do not authorize activation, crate creation, or runtime behavior before the activation gates in section 4 are satisfied.

- `devil-lsp` may depend on:
  - `devil-observability`
  - `devil-platform`
  - `devil-protocol`
  - `devil-security`
  - `devil-storage`

- `devil-plugin` may depend on:
  - `devil-observability`
  - `devil-platform`
  - `devil-protocol`
  - `devil-security`
  - `devil-storage`

- `devil-terminal` may depend on:
  - `devil-observability`
  - `devil-platform`
  - `devil-protocol`
  - `devil-security`

- `devil-collaboration` may depend on:
  - `devil-observability`
  - `devil-protocol`
  - `devil-security`
  - `devil-storage`

- `devil-remote` may depend on:
  - `devil-observability`
  - `devil-platform`
  - `devil-protocol`
  - `devil-security`
  - `devil-storage`

### 2. Shared Contracts Boundary

- Cross-domain project/editor/indexer/tracker interactions should flow through `devil-protocol` types and traits.
- UI shell code is projection-only: `devil-ui` consumes protocol projections, emits `CommandDispatchIntent`, and may not depend on editor, project, storage, or app crates for text ownership, command execution, save orchestration, or file authority.
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
  - `WorkspaceDiscoveryDecision`
  - `WorkspaceDiscoverySkipReason`
  - `WorkspaceDiscoveryPathPolicyResult`
  - `WorkspaceDiscoveryTrustResult`
  - `WorkspaceDiscoveryChangeKind`
  - `WorkspaceDiscoveryPolicyDecision`
  - `WorkspaceDiscoveryRecord`
  - `WorkspaceDiscoverySnapshot`
  - `WorkspaceDiscoveryDelta`
  - `SemanticFabricWorkSourceKind`
  - `SemanticFabricSchedulingTrigger`
  - `SemanticFabricPriority`
  - `SemanticFabricInvalidationCause`
  - `SemanticFabricSchedulingAction`
  - `SemanticFabricPrivacyLabel`
  - `SemanticFabricDependencyHint`
  - `SemanticFabricDescriptorReference`
  - `SemanticFabricJobRequest`
  - `SemanticFabricSchedulingDecision`
  - `SemanticFabricSchedulePlan`
  - `ContextManifestPurpose`
  - `ContextManifestItemKind`
  - `ContextManifestInclusionState`
  - `ContextManifestEgressStatus`
  - `ContextManifestPermissionKind`
  - `ContextManifestPermissionSummary`
  - `ContextManifestItemCount`
  - `ContextManifestFreshnessSummary`
  - `ContextManifestPreconditionSummary`
  - `ContextManifestItem`
  - `ContextManifestRecord`
  - `ContextManifestProjection`
  - `PrivacyInspectorSourceKind`
  - `PrivacyInspectorRedactionState`
  - `PrivacyInspectorRefusal`
  - `PrivacyInspectorExposureRecord`
  - `PrivacyInspectorProjection`
  - `PrivacyInspectorProposalContext`
  - `PermissionBudgetActionClass`
  - `PermissionBudgetState`
  - `PermissionBudgetResetPolicyLabel`
  - `PermissionBudgetConsentRequirementLabel`
  - `PermissionBudgetUsageSummary`
  - `PermissionBudgetContract`
  - `PermissionBudgetActionSummary`
  - `PermissionBudgetEvaluationDisposition`
  - `PermissionBudgetEvaluation`
  - `PermissionBudgetProjection`
  - `ApprovalChecklistGateKind`
  - `ApprovalChecklistGateStatus`
  - `ApprovalChecklistReason`
  - `ApprovalChecklistGateSummary`
  - `ProposalApprovalChecklistProjection`
  - `CheckpointRollbackAuditStatus`
  - `CheckpointRollbackLimitation`
  - `CheckpointRollbackTargetSummary`
  - `ProposalCheckpointProjection`
  - `ProposalRollbackProjection`
  - `CheckpointRollbackProjection`
  - `AssistedAiProviderClass`
  - `AssistedAiOperationClass`
  - `AssistedAiSupportLabel`
  - `AssistedAiProviderAvailabilityState`
  - `AssistedAiConsentState`
  - `AssistedAiRequestDisposition`
  - `AssistedAiProviderInvocationState`
  - `AssistedAiTrustProjectionReference`
  - `AssistedAiTrustProjectionKind`
  - `AssistedAiPermissionBudgetEvaluationReference`
  - `AssistedAiRefusalMetadata`
  - `AssistedAiProviderCapability`
  - `AssistedAiConsentBoundary`
  - `AssistedAiRouteDecision`
  - `AssistedAiProposalTargetIntent`
  - `AssistedAiRequestContract`
  - `AssistedAiEditProposalOutput`
  - `AssistedAiProposalPreviewReadiness`
  - `AssistedAiProviderCapabilitySummary`
  - `AssistedAiRouteDecisionSummary`
  - `AssistedAiRequestContractSummary`
  - `AssistedAiProposalPreviewSummary`
  - `AssistedAiProjection`
  - `AssistedAiAuditPrivacyDisposition`
  - `AssistedAiAuditOutcomeCategory`
  - `AssistedAiAuditRedactionState`
  - `AssistedAiAuditRecord`
  - `AssistedAiContractError`
  - `DelegatedTaskPlanId`
  - `DelegatedTaskStepId`
  - `DelegatedTaskOperationClass`
  - `DelegatedTaskTrustGateKind`
  - `DelegatedTaskPlanState`
  - `DelegatedTaskStepState`
  - `DelegatedTaskRuntimeActivationState`
  - `DelegatedTaskPlanReadinessStatus`
  - `DelegatedTaskReadinessClassification`
  - `DelegatedTaskRequiredTrustGate`
  - `DelegatedTaskPlanBlocker`
  - `DelegatedTaskAffectedTargetSummary`
  - `DelegatedTaskProposalPreviewLink`
  - `DelegatedTaskPlanStep`
  - `DelegatedTaskAuditReadinessStatus`
  - `DelegatedTaskAssistedAiAuditReference`
  - `DelegatedTaskAuditLinkageRecord`
  - `DelegatedTaskPlanContract`
  - `DelegatedTaskPlanningBoundaryInput`
  - `DelegatedTaskPlanRow`
  - `DelegatedTaskStepSummary`
  - `DelegatedTaskProjection`
  - `FutureSurfaceGateId`
  - `FutureSurfaceClass`
  - `FutureSurfaceOperationClass`
  - `FutureSurfaceRequirementStatus`
  - `FutureSurfaceRuntimeActivationState`
  - `FutureSurfaceGateClassification`
  - `FutureSurfaceBlockerCategory`
  - `FutureSurfaceGateReason`
  - `FutureSurfacePlanningGateInput`
  - `FutureSurfacePlanningGate`
  - `FutureSurfaceGateProjection`
  - `Utf16Position`
  - `Utf16Range`
  - `ChangedTextRange`
  - `CausalityId`
  - `EventId`
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
  - `LspConfiguredServerIdentity`
  - `LspWorkspaceTrustPosture`
  - `LspLaunchDisposition`
  - `LspLaunchPolicyDecision`
  - `LspSupervisionLifecycleState`
  - `LspHealthState`
  - `LspRestartBackoffMetadata`
  - `LspCapabilitySummary`
  - `LspDiagnosticSummary`
  - `LspRequestCorrelation`
  - `LspSupervisionEventKind`
  - `LspSupervisionEvent`
  - `LspContractValidationError`
  - `LspEditProposalConversionInput`
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
  - `EventSeverity`
  - `RetentionLabel`
  - `RedactionHint`
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

### 3. Forbidden/Deferred Edges

- Do not add hard edges from:
  - `devil-editor` -> `devil-project`
  - `devil-ui` -> `devil-app`
  - `devil-ui` -> `devil-editor`
  - `devil-ui` -> `devil-project`
  - `devil-ui` -> `devil-storage`
  - `devil-ui` -> feature crates beyond declared contracts
  - `devil-tracker` -> feature crates that are not storage-protocol mediated
  - `devil-memory` -> non-storage non-protocol feature domains without explicit planning
  - `devil-agent` -> `devil-app` or `devil-ui`
  - planned runtime surfaces -> `devil-app` internals without protocol-port mediation

### 4. Runtime Surface Activation Gates

- Phase 3 activates `devil-index` only for the semantic fabric scope accepted in `plans/adrs/ADR-0017-semantic-fabric-indexing.md` and evidenced through `plans/evidence/phase-3/predictive-semantic-fabric.md`.
- `devil-agent`, `devil-tracker`, `devil-memory`, `devil-plugin`, `devil-lsp`, `devil-terminal`, `devil-collaboration`, and `devil-remote` remain ADR-gated. LSP runtime behavior is additionally gated by `plans/adrs/ADR-0018-lsp-runtime-supervision.md` before implementation.
- Runtime behavior for placeholder crates or planned surfaces must not land until the same change also includes:
  - an accepted ADR for the surface being activated
  - an explicit dependency-policy entry in this document
  - an active phase gate recorded in the implementation plan or phase evidence
  - required protocol contracts in `crates/devil-protocol/src/lib.rs`
  - contract tests for the newly activated protocol and runtime behavior
  - architecture-gate tests proving the new surface preserves ownership and mutation rules
  - an owner recorded in the active implementation plan or evidence
- Existing ADRs for tracker or memory do not waive the other activation gates. Placeholder crates remain inert unless they have an accepted ADR, dependency-policy entry, phase gate, required protocol contracts, contract tests, ownership tests, and evidence.
- Vector indexing remains deferred until a later accepted ADR, dependency-policy update, syntax-aware chunking contract, provenance contract, privacy-scope contract, model-identity contract, invalidation contract, storage-retention decision, and contract-test suite exist.

### 5. Enforcement

- `xtask check-deps` reads this policy and fails when a workspace crate lacks policy coverage.
- `xtask check-deps` fails when forbidden edges are detected.
- `xtask check-deps` fails when required internal dependencies are missing.
- `xtask check-deps` fails when required protocol symbols are absent from `crates/devil-protocol/src/lib.rs`.
