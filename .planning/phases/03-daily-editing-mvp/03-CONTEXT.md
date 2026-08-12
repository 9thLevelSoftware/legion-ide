# Phase 3: Daily Editing MVP - Context

## Workflow Inputs

- User command: `$legion plan 3 --auto-refine`.
- Project root: `C:\Users\dasbl\RustroverProjects\devil-ide`.
- Phase source: `.planning/ROADMAP.md`, Phase 3, "Daily Editing MVP".
- Requirements source: `.planning/ROADMAP.md` only. `.planning/REQUIREMENTS.md` is absent, so roadmap requirement IDs and success criteria are authoritative.
- Prior phase source: `.planning/phases/02-renderer-backed-foundation-mode/02-REVIEW.md` and `plans/evidence/gui-productization/phase-2-renderer-foundation.md`.
- Code map source: `.planning/CODEBASE.md` plus `.planning/codebase/` artifacts, with current source reads overriding map summaries.
- Structured Legion `AskUserQuestion` confirmation is unavailable in this Codex runtime. The direct user command and `--auto-refine` flag are treated as approval to generate and refine executable Phase 3 plans from current repository evidence.

## Map Freshness Warning

`.planning/CODEBASE.md` was generated for commit `b521ab5e64696b9017f02595183de9ed1614f8eb`, while the current checkout is `6e1b5d5380e4b56ff436be54e985d3dad7711837`. The map still provides architecture context, but it predates Phase 2 source changes. Phase 3 plans therefore rely on live reads of `crates/devil-desktop`, `crates/devil-app`, `crates/devil-ui`, `crates/devil-editor`, `crates/devil-protocol`, and Phase 2 evidence before using map claims.

## Phase Goal

Make local GUI editing usable for real files and repeated sessions without weakening the existing authority boundaries.

Phase 3 is successful only if:

- Multi-tab editing, close/reopen behavior, explorer expand/collapse/selection/reveal, cursor/selection, scrolling, undo/redo, save all, and close-dirty prompts work in the GUI.
- Search in file and search in workspace work through approved projections/services.
- Session restore recovers workspace, tabs, focus, layout, and explorer state.
- External overwrite between open and save yields a visible conflict and preserves dirty text.
- Large-file degraded mode is preserved; GUI code never requires unbounded full-source projection.

## Current State Evidence

- Phase 2 accepted `devil-desktop` as the only renderer-backed adapter crate.
- `crates/devil-desktop/src/workflow.rs` opens a workspace/file, routes basic edit/save/rejection/quit actions through `AppComposition`, and refreshes `ShellProjectionSnapshot`.
- `crates/devil-desktop/src/bridge.rs` maps adapter actions into `CommandDispatchIntent`, `DesktopAppRequest`, `Noop`, or typed errors without importing app/editor/workspace internals.
- `crates/devil-desktop/src/view.rs` renders a `DesktopProjectionViewModel` from `ShellProjectionSnapshot`.
- `crates/devil-app/src/lib.rs` currently has one active document controller, but `buffer_file_metadata`, `bind_saved_buffer`, `EditorEngine::buffer_for_file`, `EditorEngine::buffer_for_path`, and `EditorEngine::close_buffer` provide usable seams for tab state, save-all, and close workflows.
- `crates/devil-editor/src/lib.rs` already supports undo/redo, dirty state, bounded viewport projections, `set_cursors`, `set_selections`, degraded mode, and close-buffer behavior.
- `crates/devil-protocol/src/lib.rs` already defines `WorkspaceSessionRecord`, `SessionTab`, `SessionTabGroup`, `SessionPanelState`, `SessionDirtyIndicator`, `ViewportProjection`, `ViewportScroll`, `SemanticQueryRequest`, and `SemanticQueryResponse`.
- Search is not yet a daily-editing GUI flow. `devil-index` has semantic query infrastructure, but Phase 3 search should start with bounded active-buffer/workspace lexical search through app/workspace authority, not by activating later LSP/runtime surfaces.

## Key Design Decisions

- Architecture proposals: skipped by direct command execution. The selected approach is conservative and sequential because Phase 3 repeatedly touches high-risk files (`devil-app/src/lib.rs`, `devil-ui/src/ui.rs`, and `devil-desktop/src/workflow.rs`).
- Spec pipeline: skipped because no Phase 3 spec document exists and the roadmap success criteria are concrete enough for implementation contracts.
- Search scope: implement bounded lexical search for file/workspace daily editing. Do not activate LSP, hosted AI, vector search, remote search, or provider-backed semantic behavior in this phase.
- Session restore scope: persist protocol `WorkspaceSessionRecord` metadata only. Do not persist raw buffer text, search result bodies beyond bounded display snippets, terminal output, provider payloads, or workspace mutations.
- Close-dirty behavior: dirty close never discards text silently. It must surface a prompt/outcome and preserve the buffer unless an explicit discard path is implemented and verified.
- Save-all behavior: save buffers sequentially through `SaveWorkflowService`; stale/conflict/denial returns rejected outcomes and preserves dirty buffers.

## Plan Structure

- **Plan 03-01 (Wave 1)**: Daily Editing App State And Projection Contracts - add app/UI command and projection contracts for tabs, viewport/cursor state, save-all, close-dirty, and session records.
- **Plan 03-02 (Wave 2)**: Desktop Tabs Explorer And Viewport Controls - render and route tabs, explorer selection/expansion, cursor/selection/scrolling, keyboard shortcuts, and close-dirty prompts.
- **Plan 03-03 (Wave 3)**: Bounded File And Workspace Search - add bounded active-file and workspace lexical search services, projections, desktop search panel, and tests.
- **Plan 03-04 (Wave 4)**: Save-All Conflict And Dirty-Close Hardening - harden save-all, conflict visibility, dirty close, and rejection preservation with integration tests.
- **Plan 03-05 (Wave 5)**: Session Restore And Large-File Guardrails - persist/restore metadata-only session records and prove degraded-mode rendering/search does not demand full text.
- **Plan 03-06 (Wave 6)**: Phase 3 Evidence And Acceptance Gate - run focused and full gates, archive daily-editing evidence, and reconcile success criteria.

## Non-Negotiable Constraints

- `devil-ui` remains projection-only and must not depend on `devil-app`, `devil-editor`, `devil-project`, `devil-storage`, `devil-desktop`, or renderer crates.
- `devil-desktop` may render, collect local interaction state, and translate events, but app/editor/workspace authority remains in `AppComposition`, `EditorEngine`, and `WorkspaceActor`.
- Saves remain proposal-mediated. No save-all or close workflow may write disk outside `AppComposition::save_active_buffer` or a shared app-owned equivalent using `SaveWorkflowService`.
- External overwrite conflicts must stay visible and must not mark dirty text clean.
- Large-file degraded mode must remain bounded. Plans must use viewport/chunk projections and metadata, not `editor.text()` or `small_buffer_preview` for degraded buffers.
- Placeholder/gated AI, LSP, terminal, plugin, collaboration, remote, telemetry, and retention surfaces remain inert unless already projected by app state.

## Auto-Refine Summary

The `--auto-refine` pass identified and addressed these planning risks before finalization:

1. The codebase map predates Phase 2, so plan context now explicitly requires live source reads and warns that map claims about "no GUI renderer" are stale.
2. Initial search decomposition risked activating semantic/LSP surfaces early. Plan 03-03 now constrains search to bounded lexical app/workspace services and metadata-safe projections.
3. Initial session restore risked persisting dirty buffer text. Plan 03-05 now stores protocol session metadata only and verifies no raw buffer/source bodies are written.
4. Initial close/save-all scope risked silent dirty text loss. Plans 03-01, 03-02, and 03-04 now require prompt/rejection outcomes and dirty-preservation tests.
5. Shared high-risk files make parallel execution risky. The final wave structure is intentionally sequential.

