# Phase 3 Daily Editing MVP

## Acceptance Status

Phase 3 daily editing MVP: Accepted

Decision date: 2026-05-27

Phase 3 is accepted for the Daily Editing MVP scope. The accepted scope is local daily editing in the renderer-backed desktop adapter: tabs, explorer controls, cursor/selection/scrolling, undo/redo routing, save all, dirty-close prompts, bounded active-file/workspace search, metadata-only session restore, visible conflict preservation, and large-file degraded guardrails.

This is not acceptance of Phase 4 language/terminal workflows, Phase 5 control and AI surfaces, or Phase 6 packaging/accessibility proof. The broad workspace test command initially failed in the low-disk local environment, then passed after disk space was restored.

## Artifact Inventory

| Artifact | Status | Evidence |
| --- | --- | --- |
| `.planning/phases/03-daily-editing-mvp/03-01-RESULT.md` | Complete | Daily editing app state, UI projections, tab/viewport/save-all/session contracts, and app tests. |
| `.planning/phases/03-daily-editing-mvp/03-02-RESULT.md` | Complete | Desktop tab strip, explorer expansion/selection/reveal, viewport controls, bridge routing, and desktop tests. |
| `.planning/phases/03-daily-editing-mvp/03-03-RESULT.md` | Complete | Bounded lexical active-file/workspace search through app and workspace authority. |
| `.planning/phases/03-daily-editing-mvp/03-04-RESULT.md` | Complete | Save-all status hardening, dirty-close save/cancel, conflict preservation, and external-overwrite regression evidence. |
| `.planning/phases/03-daily-editing-mvp/03-05-RESULT.md` | Complete | Metadata-only session persistence/restore and large-file degraded guardrail tests. |
| `.planning/phases/03-daily-editing-mvp/WAVE-CHECKLIST.md` | Complete | Plan-by-plan status, boundary proof, map freshness warning, and final gate notes. |
| `plans/evidence/gui-productization/phase-3-session-and-large-file.md` | Complete | Session and large-file guardrail evidence, including the documented 100MB degraded workload gap. |

## Boundary Proof

- `devil-ui` remains projection-only. The live boundary check for `devil-app`, `devil-editor`, `devil-project`, `devil-storage`, `devil-desktop`, `eframe`, and `egui` references in `crates/devil-ui/Cargo.toml` and `crates/devil-ui/src/ui.rs` returned no matches.
- `crates/devil-desktop/src/bridge.rs` and `crates/devil-desktop/src/view.rs` do not import editor/project/storage internals; the live check for `devil_editor`, `devil_project`, `devil_storage`, `EditorEngine`, `WorkspaceActor`, and `StorageRepository` returned no matches.
- Desktop actions continue through adapter bridge and app authority. `crates/devil-desktop/src/workflow.rs` calls `self.app.dispatch_ui_intent(intent)`.
- Saves remain proposal-mediated. `crates/devil-app/src/lib.rs` routes saves through `SaveWorkflowService::save_active_buffer` and `workspace.save_file_with_proposal`.
- Phase 3 source reads override the stale code map. `.planning/phases/03-daily-editing-mvp/03-CONTEXT.md` records that `.planning/CODEBASE.md` predates Phase 2 and Phase 3 source changes.

## Daily Editing Proof

| Capability | Decision | Evidence |
| --- | --- | --- |
| Multi-tab editing and close/reopen behavior | Met | Plan 03-01 added tab/session contracts; Plan 03-02 routed desktop tab controls; `cargo test -p devil-app daily_editing -- --nocapture` passed; `cargo test -p devil-desktop --all-targets` passed. |
| Explorer expand/collapse/selection/reveal | Met | Plan 03-02 added adapter-local explorer expansion and reveal routing through `CommandDispatchIntent`; desktop `daily_editing_controls`, `intent_bridge`, and `projection_rendering` tests passed. |
| Cursor, selection, scrolling, undo/redo routing | Met | Plan 03-01/03-02 added projection and command handling for cursor/selection/viewport; desktop all-target tests passed. Undo/redo remains routed through existing command dispatch rather than re-owned by UI. |
| Save all | Met | Plan 03-04 records per-buffer save-all outcomes, rejection metadata, and generation refresh after successful saves; app daily-editing tests and desktop save-all conflict tests passed. |
| Close-dirty prompts | Met | Plan 03-01/03-02/03-04 preserve dirty buffers and expose save/cancel behavior; no unverified discard path was added. |
| Active-file and workspace search | Met | Plan 03-03 added bounded lexical search through app/workspace authority; `daily_editing_search` and `search_workflow` tests passed. |
| Session restore | Met | Plan 03-05 added `DesktopSessionStore`, `--session-state`, and restore via `AppComposition::restore_workspace_session_record`; `session_restore` tests passed. |
| External overwrite conflict | Met | Plan 03-04 preserved visible rejection/conflict metadata and dirty text; `save_all_conflict` and the external-overwrite desktop workflow regression passed. |
| Large-file degraded mode | Met with documented limitation | Plan 03-05 proved degraded desktop rendering/search remains bounded; `large_file_guardrails` passed and the editor performance suite list still records the ignored 100MB workload as a known degraded/streaming-mode gap. |

## Search Proof

- Search projections are projection-only DTOs in `crates/devil-ui/src/ui.rs`.
- `AppComposition::run_search` in `crates/devil-app/src/lib.rs` performs bounded lexical active-file/workspace search and caps limits.
- Degraded active-file search is limited to visible viewport content with a visible degraded-limited status.
- Workspace search uses metadata bounds and skips oversized or unreadable files rather than reading unbounded file bodies.
- Desktop search display is built from `SearchProjection` in `crates/devil-desktop/src/search.rs`.

## Session Proof

- `crates/devil-desktop/src/session.rs` serializes/deserializes `WorkspaceSessionRecord` JSON only.
- Session restore is invoked through `AppComposition::restore_workspace_session_record`; desktop does not recreate editor buffers directly.
- Session validation rejects invalid schema/session ids and raw-source marker strings including `small_buffer_preview`, `source_body`, and `SECRET_DIRTY_BODY`.
- Dirty source bodies are not persisted or replayed during restore.

## Conflict Proof

- Save-all and single-save flows preserve proposal-mediated save authority.
- Rejected save-all items keep dirty text and expose proposal response metadata for desktop warning rows.
- External overwrite between open and save yields `SaveRejected`, does not clobber disk content, and preserves dirty projected editor text.
- Dirty-close cancel preserves the tab and text; dirty-close save clears the prompt only after an accepted app-owned save.

## Large-File Proof

- `ViewportProjectionMode::DegradedLargeFile` is preserved in UI projections.
- Desktop smoke evidence fields include `large_file_degraded_status`, `bounded_search_status`, and `full_text_projection_status`.
- Large-file guardrail tests prove desktop rendering uses viewport rows and degraded active-file search remains bounded to visible content.
- The ignored 100MB performance workload remains a known degraded/streaming-mode gap; Phase 3 does not claim it is green.

## Command Table

| Command | Result | Notes |
| --- | --- | --- |
| `cargo run -p xtask -- check-deps` | Passed | Output included `dependency policy checks passed`. |
| `cargo fmt --all --check` | Passed | No formatting diff. |
| `cargo check --workspace --all-targets` | Passed | Workspace all-target check completed for `devil-ui`, `devil-app`, and `devil-desktop`. |
| `cargo test -p devil-app daily_editing -- --nocapture` | Passed | App daily-editing filters passed: save-all unit coverage, 7 `daily_editing_contracts` tests, and 6 `daily_editing_search` tests. |
| `cargo test -p devil-desktop --all-targets` | Passed | Desktop tests passed, including workflow, daily-editing controls, intent bridge, large-file guardrails, platform smoke, projection rendering, save-all conflict, search workflow, and session restore. |
| `cargo test --workspace --all-targets` | Passed on rerun | After freeing disk space, the workspace all-target test passed. The performance suite reported 7 passed and 3 ignored, including the intentionally ignored 100MB degraded-mode workload. |
| `cargo clippy --workspace --all-targets -- -D warnings` | Passed | Finished warning-clean for `devil-ui`, `devil-app`, and `devil-desktop`. |
| `rg -q "Phase 3 daily editing MVP: Accepted" plans/evidence/gui-productization/phase-3-daily-editing-mvp.md` | Passed | Acceptance marker present in this evidence artifact. |

## Success Criteria Decisions

| Roadmap criterion | Decision | Evidence |
| --- | --- | --- |
| Multi-tab editor, close/reopen behavior, explorer expand/collapse/selection/reveal, cursor/selection, scrolling, undo/redo, save all, and close-dirty prompts work in the GUI. | Met | Plans 03-01, 03-02, and 03-04; app daily-editing tests; desktop all-target tests. |
| Search in file and search in workspace work through approved projections/services. | Met | Plan 03-03; `daily_editing_search` and `search_workflow` tests; search source boundary evidence. |
| Session restore recovers workspace, tabs, focus, layout, and explorer state. | Met | Plan 03-05; `session_restore` tests; session persistence source proof. |
| External overwrite between open and save yields a visible conflict and preserves dirty text. | Met | Plan 03-04; `save_all_conflict` tests and external-overwrite desktop workflow regression. |
| Large-file degraded mode is preserved; GUI never requires unbounded full-source projection. | Met with known performance limitation | Plan 03-05; `large_file_guardrails` tests; smoke fields; documented ignored 100MB degraded workload gap. |

## Residual Risks

- Accessibility proof remains limited. Phase 2 recorded accessibility smoke as not observed, and Phase 3 did not add Phase 6 accessibility-tree evidence.
- The ignored 100MB performance workload remains a known degraded/streaming-mode gap. Phase 3 proves bounded degraded behavior, not final large-file performance.
- `cargo deny check` was not part of Plan 03-06 frontmatter and was not rerun here; dependency policy was covered by `xtask check-deps`.

## Phase 4 Entry Criteria

- Preserve the Phase 3 daily editing boundary: desktop renders and routes, app/editor/workspace retain authority.
- Keep edit-producing language actions proposal-mediated.
- Do not route terminal or LSP output directly into editor buffers or disk writes.
- Preserve Phase 3's bounded large-file assumptions until the ignored 100MB degraded-mode workload is promoted by a later phase.
