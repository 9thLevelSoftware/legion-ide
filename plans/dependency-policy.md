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
- `devil-ui` MUST NOT depend on `eframe`.
- `devil-ui` MUST NOT depend on `egui`.
- `devil-ui` MUST NOT depend on `egui-winit`.
- `devil-ui` MUST NOT depend on `egui-wgpu`.
- `devil-ui` MUST NOT depend on `winit`.
- `devil-ui` MUST NOT depend on `wgpu`.
- `devil-ui` MUST NOT depend on `accesskit`.
- `devil-ui` MUST NOT depend on `slint`.
- `devil-ui` MUST NOT depend on `tauri`.
- `devil-ui` MUST NOT depend on `wry`.
- `devil-ui` MUST NOT depend on `tao`.
- `devil-ui` MUST NOT depend on `gpui`.

- `devil-desktop` may depend on:
  - `devil-app`
  - `devil-protocol`
  - `devil-ui`

`devil-desktop` is the active Phase 2 crate authorized to host GUI renderer dependencies. Phase 2 may use `eframe` and `egui` for the Windows-first desktop foundation proof, including their renderer/windowing/accessibility integration stack such as `egui-winit`, `egui-wgpu`, `winit`, `wgpu`, and `accesskit` when pulled in by or needed for the adapter. Slint is an explicit fallback candidate for native panel rendering if Phase 2 evidence shows the egui path cannot satisfy IME, clipboard, focus, accessibility, or high-DPI requirements. Tauri/WRY/TAO and GPUI are not approved for the core editor shell in Phase 2; Tauri/WRY remain auxiliary-only unless a later ADR supersedes ADR-0002, and GPUI remains a long-term architecture influence until its official Windows-first support is suitable for this project.

Renderer crates are adapter-only. They must not appear in `devil-ui`, app/editor/project/protocol/storage/security/observability/provider/runtime crates, or any core substrate crate until a later ADR and dependency-policy update explicitly authorize that edge.

- `devil-app` may depend on:
  - `devil-agent`
  - `devil-ai`
  - `devil-ai-providers`
  - `devil-collaboration`
  - `devil-editor`
  - `devil-index`
  - `devil-memory`
  - `devil-observability`
  - `devil-platform`
  - `devil-plugin`
  - `devil-project`
  - `devil-protocol`
  - `devil-remote`
  - `devil-security`
  - `devil-storage`
  - `devil-terminal`
  - `devil-tracker`
  - `devil-ui`

GUI Phase 4 activates `devil-app` composition edges to `devil-index` and `devil-terminal` only for the language-and-terminal IDE loop. Language features must consume semantic/index and LSP DTOs through proposal-mediated edit previews before mutation. Terminal features must remain policy-gated, projection-only at the UI boundary, metadata-redacted, and fail closed by default; native terminal execution stays controlled by the terminal crate and its security/runtime gates. This GUI Phase 4 policy does not authorize `devil-ui` ownership of editor sessions/text, direct workspace mutation from language tooling, or new dependencies from `devil-index`/`devil-terminal` back into app, UI, editor, project, or desktop internals.

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

Phase 4 activates `devil-agent`, `devil-tracker`, and `devil-memory` only for metadata-only local-provider planning, tracker ledger records, memory candidate review, and proposal-only agent outputs. These crates must not depend on app/UI/editor/workspace internals and must not gain direct filesystem, process, network, terminal, storage, settings, or buffer mutation authority.

- `devil-plugin` may depend on:
  - `devil-observability`
  - `devil-platform`
  - `devil-protocol`
  - `devil-security`
  - `devil-storage`

Phase 5 activates `devil-plugin` only as an isolated WASM plugin runtime boundary using protocol DTOs, manifest/capability validation, quota metadata, plugin-scoped storage, and metadata-only observability. It must not depend on app/UI/editor/project internals and must not gain direct filesystem, process, network, terminal, AI, tracker, memory, collaboration, remote, settings, or buffer mutation authority. Plugin mutation outputs must remain proposal-mediated.

Compatibility note: the plugin entry above is historical runtime evidence for the previously accepted Phase 5 plugin boundary. GUI Phase 5 is the active productization phase for control, trust, and assisted-AI surfaces. It authorizes only app-owned proposal, trust, permission, privacy, budget, and assisted-AI composition surfaced through protocol DTOs and projection snapshots. `devil-ui` and `devil-desktop` must remain projection and intent layers only; they must not own proposal lifecycle state, provider routing, editor text, workspace mutation, storage authority, raw-source retention, hosted-provider activation, or autonomous apply behavior.

GUI Phase 6 activates only packaging, platform integration, accessibility-smoke evidence, session metadata safety, diagnostics export, and CI/script parity for the existing `devil-desktop` adapter. It does not authorize new crate dependencies, renderer-owned editor/session/text state, direct workspace mutation outside app save workflows, raw-source diagnostics, hosted-provider activation, production collaboration/remote/terminal/LSP surfaces, or changes to legacy Phase 6 collaboration evidence. Acceptance is gated by `plans/evidence/gui-productization/phase-6-packaging-platform-accessibility.md` and the GUI Phase 6 checks in `xtask`.

GUI Phase 7 activates only local-beta evidence, deterministic smoke workflows, operational health projections, privacy-safe diagnostics, launch documentation, known-limitation documentation, and acceptance gating for the existing `devil-desktop` adapter. It is a local-beta productization track and does not alter the legacy remote-development Phase 7 acceptance under `plans/evidence/phase-7/`. GUI Phase 7 does not authorize plugin/collaboration/remote production GUI claims, hosted provider activation, autonomous apply, signed installers, platform-parity claims, new dependencies, or UI/desktop ownership of app, editor, workspace, proposal, storage, security, provider, or terminal authority. Acceptance is gated by `plans/evidence/gui-productization/phase-7-local-ide-beta.md`, the GUI Phase 7 checks in `xtask`, and `devil-cli evidence check --phase gui-phase7`.

GUI Phase 8 activates only advanced GUI GA productization evidence for plugin management, collaboration, remote workspace, delegated task command-center, and GA operations workflows through existing app/protocol authority. It is distinct from the accepted legacy Phase 8 runtime substrate evidence under `plans/evidence/phase-8/` and does not reopen or replace that acceptance record. GUI Phase 8 advanced GUI GA work does not authorize `devil-ui` or `devil-desktop` ownership of plugin runtime authority, collaboration runtime authority, remote runtime authority, terminal authority, provider routing, storage authority, security policy, raw-source diagnostics, autonomous apply, or direct mutation outside proposal-mediated app/workspace/editor paths. Acceptance is gated by `plans/evidence/gui-productization/phase-8-advanced-platform-gui-ga.md`, the GUI Phase 8 checks in `xtask`, and `devil-cli evidence check --phase gui-phase8`.

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

- `devil-terminal` may depend on:
  - `devil-observability`
  - `devil-platform`
  - `devil-protocol`
  - `devil-security`

- `devil-remote-transport` may depend on:
  - `devil-observability`
  - `devil-platform`
  - `devil-protocol`
  - `devil-security`
  - `devil-storage`

- `devil-telemetry` may depend on:
  - `devil-observability`
  - `devil-platform`
  - `devil-protocol`
  - `devil-security`
  - `devil-storage`

- `devil-retention` may depend on:
  - `devil-observability`
  - `devil-platform`
  - `devil-protocol`
  - `devil-security`
  - `devil-storage`

- `devil-collaboration` may depend on:
  - `devil-observability`
  - `devil-protocol`
  - `devil-security`
  - `devil-storage`

Phase 6 collaboration activation is currently limited to accepted protocol DTOs, governance scaffolding, evidence gates, and dependency-boundary planning. This policy entry authorizes a future `devil-collaboration` crate only within the dependency set above after accepted Phase 6 ADRs and contract tests exist. It does not authorize direct app, UI, editor, project, remote workspace, terminal/process, hosted egress, raw source retention, or direct durable workspace mutation authority. `devil-app` may compose collaboration only through protocol DTOs and proposal/workspace ports after the Phase 6 runtime gate is accepted.

- `devil-remote` may depend on:
  - `devil-observability`
  - `devil-platform`
  - `devil-protocol`
  - `devil-security`
  - `devil-storage`

Phase 7 activates `devil-remote` only as a deterministic, metadata-first edge workspace runtime harness using protocol DTOs, capability/write precondition validation, proposal IDs for remote-side mutation requests, bounded execution descriptors, reconnect/offline metadata, and metadata-only audit records. It must not depend on app, UI, editor, project, collaboration, terminal, LSP, AI, plugin, tracker, memory, or semantic-index internals. Durable local workspace writes remain app/workspace proposal-mediated; `devil-remote` must not gain direct local filesystem or editor authority.

Phase 8 currently activates `devil-remote-transport`, `devil-terminal`, `devil-telemetry`, and `devil-retention` as default-deny, protocol-mediated implementation slices. Production activation remains gated: remote transport, native PTY execution, hosted telemetry export, raw-source vault operation, storage migration apply, and operational GA are accepted only after the matching DTO contracts, security policy, platform parity, privacy evidence, recovery evidence, and archived release gates pass. Deterministic fixture paths may remain for tests, but production paths must be explicitly named/configured and impossible to confuse with GA behavior.

Phase 8 production dependency rebaseline permits the following external crates only for the named gates and only when the same change updates manifests, tests, evidence, and `deny.toml` review notes:

- Remote TLS/mTLS carrier (`devil-remote-transport`): `tokio` with network/I/O/runtime features, `rustls`, `tokio-rustls`, `rustls-pki-types`, `sha2` for metadata-only root/pin digest checks, and certificate/root handling crates that do not expose private key material in diagnostics.
- Hosted telemetry HTTPS exporter (`devil-telemetry`): either `hyper` plus `hyper-rustls` or a rustls-only `reqwest` profile; native-tls/OpenSSL-backed production profiles are not approved by this policy. The accepted `reqwest` profile must disable default features and enable only rustls-backed TLS plus required request/serialization features.
- Native terminal PTY (`devil-platform` and `devil-terminal`): `windows` for ConPTY and either `nix` or `rustix` for Unix PTY, process-group, and signal handling.
- Raw-source production vault (`devil-retention`): `aes-gcm` or `chacha20poly1305`, `rand_core`/`getrandom`, `sha2`, `zeroize`, and `keyring` for the bundled OS key-provider. Cloud KMS SDKs are not bundled in Phase 8; KMS integration is represented by a provider contract and deployment-supplied adapters.

These dependency entries are approval boundaries, not activation by themselves. A production runtime may not depend on app/UI/editor/project authority and must reject before network, process, filesystem, or crypto side effects when the security broker denies a request.

Phase 8 production capability names are reserved for security-broker decisions before runtime activation: `remote.transport.connect`, `remote.transport.listen`, `remote.agent.package.activate`, `terminal.launch`, `terminal.input`, `terminal.resize`, `terminal.close`, `terminal.kill`, `telemetry.spool.write`, `telemetry.export.hosted`, `telemetry.consent.revoke`, `retention.raw_source.capture`, `retention.raw_source.read`, `retention.raw_source.delete`, `retention.raw_source.export.hosted`, `storage.migration.apply`, and `storage.migration.repair`. Unknown capability names remain denied, air-gap denies hosted egress and non-loopback remote transport, and terminal/runtime/retention/telemetry activation remains disabled by default.

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
  - `SemanticRequest`
  - `SemanticResponse`
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
  - `AgentRunId`
  - `AgentStepId`
  - `AgentRunState`
  - `AgentStepState`
  - `AssistedAiProviderRouteRequest`
  - `AssistedAiProviderRouteResponse`
  - `AssistedAiRuntimeProviderCapability`
  - `AssistedAiStructuredOutputSchemaMetadata`
  - `AssistedAiStructuredOutputValidationResult`
  - `AgentStateTransitionRecord`
  - `AgentReplayManifest`
  - `Phase4RuntimeAuditRecord`
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
  - `PluginTrustMetadata`
  - `PluginTrustSource`
  - `PluginTrustDecision`
  - `PluginSignatureMetadata`
  - `PluginQuotaDeclaration`
  - `PluginActivationEvent`
  - `PluginCommandDescriptor`
  - `PluginContribution`
  - `PluginMenuContribution`
  - `PluginPanelContribution`
  - `PluginStatusItemContribution`
  - `PluginEditorDecorationContribution`
  - `PluginSnippetContribution`
  - `PluginLanguageProviderContribution`
  - `PluginFormatterContribution`
  - `PluginLspRegistrationContribution`
  - `PluginWorkspaceScannerContribution`
  - `PluginHostCallKind`
  - `PluginQuotaClass`
  - `PluginSandboxOperationClass`
  - `PluginHostCallRequest`
  - `PluginDenialReason`
  - `PluginHostCallResponse`
  - `PluginStorageOperation`
  - `PluginStorageRecord`
  - `PluginStorageRequest`
  - `PluginStorageResponse`
  - `ContributionDescriptor`
  - `PluginStateNamespace`
  - `PluginContributionProjection`
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
  - `SemanticPort`
  - `CapabilityBrokerPort`
  - `EventSinkPort`
  - `StorageRepositoryPort`
  - `PluginPort`
  - `ProjectInfoPort`
  - `CollaborationSessionId`
  - `CollaborationParticipantId`
  - `CollaborationOperationId`
  - `CollaborationDocumentEpoch`
  - `CollaborationParticipantRole`
  - `CollaborationSessionState`
  - `CollaborationPermission`
  - `CollaborationDocumentBinding`
  - `CollaborationSessionDescriptor`
  - `CollaborationParticipant`
  - `CollaborationVersionVectorEntry`
  - `CollaborationVersionVector`
  - `CollaborationDocumentOperationKind`
  - `CollaborationOperationPreconditions`
  - `CollaborationDocumentOperation`
  - `CollaborationAcknowledgementStatus`
  - `CollaborationAcknowledgement`
  - `CollaborationCausalGap`
  - `CollaborationPresenceProjection`
  - `CollaborationSharedProposalDisposition`
  - `CollaborationSharedProposalApproval`
  - `CollaborationAuditRecord`
  - `CollaborationReplayManifest`
  - `CollaborationTransportEnvelope`
  - `CollaborationTransportPayload`
  - `RemoteAuthorityId`
  - `RemoteAgentId`
  - `RemoteWorkspaceSessionId`
  - `RemoteOperationId`
  - `RemoteOperationLogCheckpointId`
  - `RemoteWorkspaceLifecycleState`
  - `RemoteCapabilityKind`
  - `RemoteAuthorityDescriptor`
  - `RemoteAgentDescriptor`
  - `RemoteWorkspaceSessionDescriptor`
  - `RemoteTransportEnvelope`
  - `RemoteTransportPayload`
  - `RemoteFilesystemOperationKind`
  - `RemoteFilesystemSnapshot`
  - `RemoteWritePreconditions`
  - `RemoteFilesystemOperation`
  - `RemoteProcessDescriptor`
  - `RemotePtyDescriptor`
  - `RemoteLspDescriptor`
  - `RemoteSemanticQueryDescriptor`
  - `RemoteNetworkHealthState`
  - `RemoteOperationLogCheckpoint`
  - `RemoteOfflineResumeManifest`
  - `RemoteAuditRecord`
  - `RemoteTransportEndpointDescriptor`
  - `RemoteTransportPeerIdentity`
  - `RemoteTransportCredentialReference`
  - `RemoteTransportMutualTlsMode`
  - `RemoteTransportTlsPolicy`
  - `RemoteTransportEndpointPolicy`
  - `RemoteTransportConnectionAttempt`
  - `RemoteTransportCarrierDiagnostic`
  - `RemoteTransportSchemaCompatibility`
  - `RemoteTransportHandshake`
  - `RemoteTransportFrameMetadata`
  - `RemoteTransportResumeToken`
  - `RemoteTransportHealthSummary`
  - `RemoteTransportAuditSummary`
  - `RemoteAgentPackageLifecycleState`
  - `RemoteAgentPackageLifecycleRecord`
  - `TerminalRuntimeState`
  - `TerminalLaunchPolicyContract`
  - `TerminalOutputChunk`
  - `TerminalKillEscalation`
  - `TerminalCloseRequest`
  - `TerminalKillRequest`
  - `TerminalAuditRecord`
  - `HostedTelemetryCategory`
  - `HostedTelemetryEndpointDescriptor`
  - `HostedTelemetryConsentGrant`
  - `HostedTelemetryConsentState`
  - `HostedTelemetryConsentBinding`
  - `HostedTelemetryTlsPolicy`
  - `HostedTelemetryProxyPolicy`
  - `HostedTelemetryRetryPolicy`
  - `HostedTelemetryEndpointPolicy`
  - `HostedTelemetryDiagnosticsSnapshot`
  - `PrivacyClassification`
  - `HostedTelemetrySpoolRecord`
  - `HostedTelemetryExportBatch`
  - `HostedTelemetryUploadOutcome`
  - `RawSourceRetentionPurpose`
  - `RawSourceRetentionPolicy`
  - `RawSourceRetentionConsentGrant`
  - `RawSourceCaptureRequest`
  - `RawSourceRetentionLease`
  - `RawSourceRetentionBundleDescriptor`
  - `RawSourceRetentionAccessAudit`
  - `RawSourceRetentionTombstone`
  - `HostedRetentionExportLinkage`
  - `RawSourceVaultAlgorithm`
  - `RawSourceKeyReference`
  - `RawSourceVaultEnvelope`
  - `RawSourceKeyRotationRecord`
  - `RawSourceVaultRecoveryState`
  - `RawSourceVaultRecoveryReport`
  - `RawSourceHostedExportConsent`
  - `StorageSchemaManifest`
  - `StorageMigrationStep`
  - `StorageMigrationDryRunReport`
  - `StorageChecksum`
  - `StorageBackupMarker`
  - `StorageRecoveryOutcome`
  - `StorageRepairRequest`
  - `StorageReplayManifest`
  - `StorageSubsystemHealthSummary`
  - `StorageEvidenceSummary`
  - `StorageMigrationApplyRequest`
  - `StorageMigrationApplyOutcome`

### 3. Forbidden/Deferred Edges

- Do not add hard edges from:
  - `devil-editor` -> `devil-project`
  - `devil-ui` -> `devil-app`
  - `devil-ui` -> `devil-editor`
  - `devil-ui` -> `devil-project`
  - `devil-ui` -> `devil-storage`
  - `devil-ui` -> renderer/windowing crates, including `eframe`, `egui`, `egui-winit`, `egui-wgpu`, `winit`, `wgpu`, `accesskit`, `slint`, `tauri`, `wry`, `tao`, and `gpui`
  - core crates -> `devil-desktop`
  - `devil-ui` -> feature crates beyond declared contracts
  - `devil-tracker` -> feature crates that are not storage-protocol mediated
  - `devil-memory` -> non-storage non-protocol feature domains without explicit planning
  - `devil-agent` -> `devil-app` or `devil-ui`
  - planned runtime surfaces -> `devil-app` internals without protocol-port mediation

### 4. Runtime Surface Activation Gates

- Phase 3 activates `devil-index` only for the semantic fabric scope accepted in `plans/adrs/ADR-0017-semantic-fabric-indexing.md` and evidenced through `plans/evidence/phase-3/predictive-semantic-fabric.md`.
- `devil-agent`, `devil-tracker`, and `devil-memory` are activated for the limited Phase 4 metadata-only runtime slice described above. `devil-plugin` is activated for the limited Phase 5 isolated plugin boundary described above. `devil-collaboration` is activated for the limited Phase 6 deterministic local collaboration runtime described above. `devil-remote` is activated for the limited Phase 7 deterministic edge workspace harness described above. `devil-remote-transport`, `devil-terminal`, `devil-telemetry`, and `devil-retention` are activated only for the current Phase 8 default-deny implementation slice described above. Standalone production `devil-lsp`, production remote transport, native terminal/PTTY execution, hosted telemetry export, raw-source vault activation, and storage migration apply remain evidence gated until the Phase 8 GA checklist and archived release gates are accepted. LSP runtime behavior is additionally gated by `plans/adrs/ADR-0018-lsp-runtime-supervision.md` before implementation.
- Runtime behavior for placeholder crates or planned surfaces must not land until the same change also includes:
  - an accepted ADR for the surface being activated
  - an explicit dependency-policy entry in this document
  - an active phase gate recorded in the implementation plan or phase evidence
  - required protocol contracts in `crates/devil-protocol/src/lib.rs`
  - contract tests for the newly activated protocol and runtime behavior
  - architecture-gate tests proving the new surface preserves ownership and mutation rules
  - an owner recorded in the active implementation plan or evidence
- Existing ADRs for tracker or memory do not waive the other activation gates. Planned runtime crates remain inert unless they have an accepted ADR, dependency-policy entry, phase gate, required protocol contracts, contract tests, ownership tests, and evidence.
- Vector indexing remains deferred until a later accepted ADR, dependency-policy update, syntax-aware chunking contract, provenance contract, privacy-scope contract, model-identity contract, invalidation contract, storage-retention decision, and contract-test suite exist.

### 5. Enforcement

- `xtask check-deps` reads this policy and fails when a workspace crate lacks policy coverage.
- `xtask check-deps` fails when forbidden edges are detected.
- `xtask check-deps` fails when required internal dependencies are missing.
- `xtask check-deps` fails when required protocol symbols are absent from `crates/devil-protocol/src/lib.rs`.
- `xtask check-deps` fails when any workspace package other than `devil-desktop` declares renderer/windowing dependencies or when this policy stops documenting the `devil-desktop` renderer boundary.
