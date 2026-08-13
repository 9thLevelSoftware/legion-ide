# Legion IDE - Architecture Review for Phases 5-6 v0.1

Status: **HOLD FOR REQUIRED CHANGES**

## Review scope

This review covers phase 5 multi-tab UI shell and session restore, and phase 6 save pipeline, conflict state, and proposal lifecycle.

Primary source artifacts reviewed:

- Phase 5 and phase 6 roadmap scope in [`plans/foundational-core-ide-platform-roadmap-v0.1.md`](plans/foundational-core-ide-platform-roadmap-v0.1.md:167).
- Phase 5 and phase 6 implementation tasks in [`plans/foundational-core-ide-platform-implementation-plan-v0.1.md`](plans/foundational-core-ide-platform-implementation-plan-v0.1.md:134).
- Phase transition gates in [`plans/foundational-core-ide-platform-implementation-plan-v0.1.md`](plans/foundational-core-ide-platform-implementation-plan-v0.1.md:257).
- Core ownership, mutation, latency, and save-pipeline rules in [`plans/ide-core-architecture-spec-v0.1.md`](plans/ide-core-architecture-spec-v0.1.md:47) and [`plans/ide-core-architecture-spec-v0.1.md`](plans/ide-core-architecture-spec-v0.1.md:329).
- Current UI shell spike in [`crates/legion-ui/src/ui.rs`](crates/legion-ui/src/ui.rs:64).
- Current application save path in [`crates/legion-app/src/lib.rs`](crates/legion-app/src/lib.rs:118).
- Current workspace write path in [`crates/legion-project/src/lib.rs`](crates/legion-project/src/lib.rs:1063).
- Current session storage record in [`crates/legion-storage/src/lib.rs`](crates/legion-storage/src/lib.rs:41).
- Current proposal and event contracts in [`crates/legion-protocol/src/lib.rs`](crates/legion-protocol/src/lib.rs:783) and [`crates/legion-protocol/src/lib.rs`](crates/legion-protocol/src/lib.rs:1725).

## Executive outcome

The phase 5 and phase 6 direction is architecturally correct, but the plan is not implementation-ready without stronger contract and sequencing guards.

> Historical rebaseline note (2026-05-15): the first two bullets below, phase 5 current-state gaps 1-2, and phase 6 current-state gap 1 describe superseded spike behavior. The current shell is projection-only through [`Shell`](crates/legion-ui/src/ui.rs:228), and current manual saves route through [`SaveWorkflowService::save_active_buffer()`](crates/legion-app/src/lib.rs:938) into [`WorkspaceActor::save_file_with_proposal()`](crates/legion-app/src/lib.rs:1021).
>
> Current correction (2026-06-02): this review also predates later observability, proposal, collaboration, plugin, remote, terminal, telemetry, retention, GUI productization, and Legion workflow slices. Treat placeholder language below as historical unless it is explicitly describing still-gated production expansion.

- **Historical (superseded by the current shell): Phase 5 had to begin as a replacement of spike UI ownership**, not as incremental extension of the earlier shell. The current [`Shell`](crates/legion-ui/src/ui.rs:228) no longer owns `EditorSession` and now emits [`CommandDispatchIntent`](crates/legion-ui/src/ui.rs:141) without mutating editor or workspace state.
- **Historical (superseded by the current manual save workflow): Phase 6 was blocked until the save path became proposal-mediated.** Current [`SaveWorkflowService::save_active_buffer()`](crates/legion-app/src/lib.rs:938) performs save request, proposal creation, validation, preview, event/audit observation, and then applies the write through [`WorkspaceActor::save_file_with_proposal()`](crates/legion-app/src/lib.rs:1021).
- **The phase 5-to-6 gate should be treated as hard.** The documented stop condition says phase 6 must not start when UI directly mutates text or workspace state in [`plans/foundational-core-ide-platform-implementation-plan-v0.1.md`](plans/foundational-core-ide-platform-implementation-plan-v0.1.md:258).
- **The phase 6-to-7 gate should be treated as hard.** The documented stop condition says phase 7 must not start when a stale proposal can apply or an external overwrite can clobber data in [`plans/foundational-core-ide-platform-implementation-plan-v0.1.md`](plans/foundational-core-ide-platform-implementation-plan-v0.1.md:259).

## Phase 5 review — Multi-tab UI shell and session restore

### Architecture fit

The intended phase 5 architecture is sound: the UI should own view projection, focus, tab ergonomics, command palette state, viewport layout, and panel surfaces, while editor and workspace remain authoritative for text and workspace identity. This aligns with the UI non-ownership rule in [`plans/ide-core-architecture-spec-v0.1.md`](plans/ide-core-architecture-spec-v0.1.md:47), editor ownership rules in [`plans/ide-core-architecture-spec-v0.1.md`](plans/ide-core-architecture-spec-v0.1.md:426), and phase 5 tasks in [`plans/foundational-core-ide-platform-implementation-plan-v0.1.md`](plans/foundational-core-ide-platform-implementation-plan-v0.1.md:136).

### Current-state gaps

1. **Historical (resolved): UI owned an editor session instead of projections.** The current [`Shell`](crates/legion-ui/src/ui.rs:228) stores layout, explorer, active-buffer, and status projections only.
2. **Historical (resolved): UI commands mutated text directly.** The current [`Shell::handle_command()`](crates/legion-ui/src/ui.rs:319) emits typed [`CommandDispatchIntent`](crates/legion-ui/src/ui.rs:141) values without mutating editor or workspace state.
3. **No production tab model exists.** The current UI surface exposes an explorer projection only, while phase 5 requires tab groups, file-buffer binding, dirty indicators, pinned and preview flags, active tab, split metadata, and close-save prompts in [`plans/foundational-core-ide-platform-implementation-plan-v0.1.md`](plans/foundational-core-ide-platform-implementation-plan-v0.1.md:140).
4. **Session persistence is too narrow.** [`SessionRecord`](crates/legion-storage/src/lib.rs:43) persists workspace id, workspace path, and trust state only. It does not persist open tabs, active buffer, layout, explorer expansion, panel state, or last workspace as required by [`plans/foundational-core-ide-platform-implementation-plan-v0.1.md`](plans/foundational-core-ide-platform-implementation-plan-v0.1.md:142).
5. **The storage protocol does not expose session restore operations.** [`StorageRepositoryRequest`](crates/legion-protocol/src/lib.rs:1803) supports workspace config and file metadata only, which leaves phase 5 session restore outside the stable service-port boundary.

### Required phase 5 changes

> Historical note: items 1-2 below are already satisfied by the current shell baseline and remain here as traceability for why the phase 5 gate existed.

1. Replace [`Shell`](crates/legion-ui/src/ui.rs:66) with projection-only shell state that contains layout, explorer projection, tab groups, panel state, command palette state, focus target, and status projection.
2. Remove direct text mutation from [`Shell::handle_command()`](crates/legion-ui/src/ui.rs:101). UI command handling should emit typed command requests to app-level protocol ports or command-dispatch services.
3. Introduce protocol DTOs for UI session projection, tab groups, active tab, split metadata, dirty indicators, pinned and preview flags, explorer expansion, panel state, and focus target.
4. Extend session persistence beyond [`SessionRecord`](crates/legion-storage/src/lib.rs:43) so restore can reconstruct open tabs and layout without restoring full text snapshots.
5. Gate session loading behind trust resolution, consistent with the phase 5 load-after-trust task in [`plans/foundational-core-ide-platform-implementation-plan-v0.1.md`](plans/foundational-core-ide-platform-implementation-plan-v0.1.md:143).
6. Add phase 5 validation for duplicate open behavior, split behavior, dirty close prompts, restart restore, focus retention, resize behavior, and no direct UI mutation.

## Phase 6 review — Save pipeline, conflict state, and proposal lifecycle

### Architecture fit

The intended phase 6 architecture is essential and correctly sequenced. It enforces the non-negotiable deterministic mutation rule in [`plans/ide-core-architecture-spec-v0.1.md`](plans/ide-core-architecture-spec-v0.1.md:61), the central file mutation policy in [`plans/ide-core-architecture-spec-v0.1.md`](plans/ide-core-architecture-spec-v0.1.md:329), and the phase 6 goal that every durable mutation be explicit, versioned, previewable, auditable, and safe in [`plans/foundational-core-ide-platform-implementation-plan-v0.1.md`](plans/foundational-core-ide-platform-implementation-plan-v0.1.md:150).

### Current-state gaps

1. **Historical (resolved for manual saves): the app save flow bypassed proposals.** Current manual saves now pass through [`SaveWorkflowService::save_active_buffer()`](crates/legion-app/src/lib.rs:938), which builds a [`WorkspaceProposal`](crates/legion-protocol/src/lib.rs:1169), validates and previews it, and then applies it through [`WorkspaceActor::save_file_with_proposal()`](crates/legion-app/src/lib.rs:1021).
2. **Workspace writes do not compare against an explicit last-read or last-save fingerprint.** [`WorkspaceActor::write_file_text()`](crates/legion-project/src/lib.rs:1064) enforces path and capability checks, then performs atomic write with fallback through [`write_file_text_atomic`](crates/legion-project/src/lib.rs:1080), but it does not reject stale disk content before writing.
3. **Fallback semantics are too permissive.** [`WorkspaceActor::write_file_text()`](crates/legion-project/src/lib.rs:1080) falls back to non-atomic write when atomic write fails. Phase 6 needs explicit fallback policy because external overwrite must never be silently clobbered in [`plans/foundational-core-ide-platform-roadmap-v0.1.md`](plans/foundational-core-ide-platform-roadmap-v0.1.md:188).
4. **Conflict state is under-modeled.** [`FileConflictState`](crates/legion-protocol/src/lib.rs:502) has a generic reason string, but phase 6 requires clean, dirty, saving, save failed, disk changed clean, conflict dirty, reload available, keep-both pending, and compare pending states in [`plans/foundational-core-ide-platform-implementation-plan-v0.1.md`](plans/foundational-core-ide-platform-implementation-plan-v0.1.md:156).
5. **Proposal DTOs are close but incomplete.** [`WorkspaceProposal`](crates/legion-protocol/src/lib.rs:783) contains principal, capability, correlation, preconditions, preview, and timing, but the implementation plan also requires trust decision, required capability, diagnostics, and audit lifecycle fields in [`plans/foundational-core-ide-platform-implementation-plan-v0.1.md`](plans/foundational-core-ide-platform-implementation-plan-v0.1.md:157).
6. **Save proposal payload is too small for phase 6 safety.** [`SaveFileProposal`](crates/legion-protocol/src/lib.rs:873) contains file identity and snapshot id only. The save lifecycle needs buffer version, file content version, workspace generation, expected fingerprint, save intent, and conflict policy either in the payload or in mandatory preconditions.
7. **Proposal lifecycle responses do not express audit-grade outcomes.** [`ProposalResponse`](crates/legion-protocol/src/lib.rs:1568) returns valid, preview, applied, or denied, but phase 6 needs created, validated, approved, rejected, applied, failed, rolled back, conflict, and stale states.
8. **Historical: observability contracts existed before the current implementation.** [`legion-observability`](crates/legion-observability/src/lib.rs:1) now contains metadata-only sinks, envelope builders, redaction behavior, and proposal/event helper coverage. The remaining production work is durable event storage, operational replay, distributed trace correlation, and productized diagnostics surfaces without raw-source or secret retention.
9. **Filesystem and workspace ADRs are not accepted artifacts.** The core architecture requires ADR decisions for file system abstraction, atomic save, conflict detection, path policy, workspace state, trust policy, watcher behavior, and identity strategy in [`plans/ide-core-architecture-spec-v0.1.md`](plans/ide-core-architecture-spec-v0.1.md:962). Phase 6 should not finalize save semantics without those decisions.

### Required phase 6 changes

> Historical note: items 2, 4, and 5 below are already satisfied for the current manual save path and remain here as review traceability. The remaining work is to extend the same guarantees to broader proposal sources.

1. Create a proposal service in app or workspace composition that implements [`ProposalPort`](crates/legion-protocol/src/lib.rs:1847) and mediates validation, preview, approval, application, rollback, conflict, and audit outcomes.
2. Replace direct save dispatch in [`AppComposition::save_active_buffer()`](crates/legion-app/src/lib.rs:119) with creation of a [`WorkspaceProposal`](crates/legion-protocol/src/lib.rs:783) whose payload is [`SaveFileProposal`](crates/legion-protocol/src/lib.rs:873) and whose preconditions include buffer version, file content version, snapshot id, workspace generation, and expected disk fingerprint.
3. Extend [`FileConflictState`](crates/legion-protocol/src/lib.rs:502) into an explicit conflict-state enum or typed state machine with reload, keep-both, compare, save-failed, and conflict-dirty transitions.
4. Make [`WorkspaceActor::write_file_text()`](crates/legion-project/src/lib.rs:1064) an internal VFS operation or replace it with a save-pipeline method that requires proposal context and expected fingerprint metadata.
5. Define atomic fallback policy as fail-closed by default unless an explicit fallback capability and conflict-safe precondition are present.
6. Persist audit metadata for proposal lifecycle events and save outcomes through storage contracts, without storing full source snapshots by default.
7. Emit [`EventEnvelope`](crates/legion-protocol/src/lib.rs:1725) events for proposal created, validated, previewed, approved, rejected, applied, failed, rolled back, stale, and conflict.
8. Add tests that prove external modification never clobbers disk content, stale proposals are rejected, conflict-dirty buffers preserve unsaved text, keep-both produces a distinct safe file identity, and batch apply rolls back on failure.

## Proposed phase 5-6 ownership model

```mermaid
flowchart TD
    UI[UI projection and command intent] --> Commands[Command dispatcher]
    Commands --> EditorPort[Editor protocol port]
    Commands --> WorkspacePort[Workspace protocol port]
    Commands --> ProposalPort[Proposal protocol port]
    EditorPort --> Editor[Editor buffers and transactions]
    WorkspacePort --> Workspace[Workspace identity trust tree VFS]
    ProposalPort --> Proposal[Proposal lifecycle audit]
    Proposal --> Editor
    Proposal --> Workspace
    Workspace --> Platform[Platform OS services]
    Proposal --> Observability[Event envelope stream]
    Proposal --> Storage[Audit and session metadata]
    UI --> SessionProjection[Tabs layout panels focus]
    SessionProjection --> Storage
```

## Gate recommendations

### Phase 4 to phase 5

Conditionally allow phase 5 only after phase 4 evidence is accepted under the transition rule in [`plans/foundational-core-ide-platform-implementation-plan-v0.1.md`](plans/foundational-core-ide-platform-implementation-plan-v0.1.md:257). Phase 5 implementation must start by replacing the current UI shell ownership model, not by adding tabs around [`EditorSession`](crates/legion-editor/src/lib.rs:1065).

### Phase 5 exit

Do not mark phase 5 complete until all of the following are true:

- UI state is projection-only and contains no editor engine or workspace actor ownership.
- Commands dispatch through ports or app-level command services instead of direct text mutation.
- Duplicate file open focuses existing tab unless an explicit split is requested.
- Session restore persists and reloads tabs, active tab, split metadata, explorer expansion, panel state, focus, and last workspace after trust resolution.
- Tests cover tab projection, session persistence, restart restore, focus, resize, dirty indicators, close prompts, and direct-mutation absence.

### Phase 5 to phase 6

Block phase 6 while any UI command can bypass proposal or workspace authority. This follows the documented phase 5-to-6 stop condition in [`plans/foundational-core-ide-platform-implementation-plan-v0.1.md`](plans/foundational-core-ide-platform-implementation-plan-v0.1.md:258).

### Phase 6 exit

Do not mark phase 6 complete until all durable mutation paths are proposal-mediated, version-checked, previewable, auditable, and conflict-safe. The phase 6-to-7 stop condition in [`plans/foundational-core-ide-platform-implementation-plan-v0.1.md`](plans/foundational-core-ide-platform-implementation-plan-v0.1.md:259) should be enforced with tests for stale proposal application and external overwrite clobbering.

## Final recommendation

**Hold phases 5-6 for required architecture changes.** The roadmap intent is correct, but current implementation and contracts need explicit projection-only UI, richer session restore DTOs, an implemented proposal lifecycle, conflict-state modeling, fail-closed save preconditions, and audit-grade observability before these phases can be approved for completion or downstream phase entry.
