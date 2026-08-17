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

- `legion-ui` remains projection-only. The live boundary check for `legion-app`, `legion-editor`, `legion-project`, `legion-storage`, `legion-desktop`, `eframe`, and `egui` references in `crates/legion-ui/Cargo.toml` and `crates/legion-ui/src/ui.rs` returned no matches.
- `crates/legion-desktop/src/bridge.rs` and `crates/legion-desktop/src/view.rs` do not import editor/project/storage internals; the live check for `legion_editor`, `legion_project`, `legion_storage`, `EditorEngine`, `WorkspaceActor`, and `StorageRepository` returned no matches.
- Desktop actions continue through adapter bridge and app authority. `crates/legion-desktop/src/workflow.rs` calls `self.app.dispatch_ui_intent(intent)`.
- Saves remain proposal-mediated. `crates/legion-app/src/lib.rs` routes saves through `SaveWorkflowService::save_active_buffer` and `workspace.save_file_with_proposal`.
- Phase 3 source reads override the stale code map. `.planning/phases/03-daily-editing-mvp/03-CONTEXT.md` records that `.planning/CODEBASE.md` predates Phase 2 and Phase 3 source changes.

## Daily Editing Proof

| Capability | Decision | Evidence |
| --- | --- | --- |
| Multi-tab editing and close/reopen behavior | Met | Plan 03-01 added tab/session contracts; Plan 03-02 routed desktop tab controls; `cargo test -p legion-app daily_editing -- --nocapture` passed; `cargo test -p legion-desktop --all-targets` passed. |
| Explorer expand/collapse/selection/reveal | Met | Plan 03-02 added adapter-local explorer expansion and reveal routing through `CommandDispatchIntent`; desktop `daily_editing_controls`, `intent_bridge`, and `projection_rendering` tests passed. |
| Cursor, selection, scrolling, undo/redo routing | Met | Plan 03-01/03-02 added projection and command handling for cursor/selection/viewport; desktop all-target tests passed. Undo/redo remains routed through existing command dispatch rather than re-owned by UI. |
| Save all | Met | Plan 03-04 records per-buffer save-all outcomes, rejection metadata, and generation refresh after successful saves; app daily-editing tests and desktop save-all conflict tests passed. |
| Close-dirty prompts | Met | Plan 03-01/03-02/03-04 preserve dirty buffers and expose save/cancel behavior; no unverified discard path was added. |
| Active-file and workspace search | Met | Plan 03-03 added bounded lexical search through app/workspace authority; `daily_editing_search` and `search_workflow` tests passed. |
| Session restore | Met | Plan 03-05 added `DesktopSessionStore`, `--session-state`, and restore via `AppComposition::restore_workspace_session_record`; `session_restore` tests passed. |
| External overwrite conflict | Met | Plan 03-04 preserved visible rejection/conflict metadata and dirty text; `save_all_conflict` and the external-overwrite desktop workflow regression passed. |
| Large-file degraded mode | Met with documented limitation | Plan 03-05 proved degraded desktop rendering/search remains bounded; `large_file_guardrails` passed and the editor performance suite list still records the ignored 100MB workload as a known degraded/streaming-mode gap. |

## Search Proof

- Search projections are projection-only DTOs in `crates/legion-ui/src/ui.rs`.
- `AppComposition::run_search` in `crates/legion-app/src/lib.rs` performs bounded lexical active-file/workspace search and caps limits.
- Degraded active-file search is limited to visible viewport content with a visible degraded-limited status.
- Workspace search uses metadata bounds and skips oversized or unreadable files rather than reading unbounded file bodies.
- Desktop search display is built from `SearchProjection` in `crates/legion-desktop/src/search.rs`.

## Session Proof

- `crates/legion-desktop/src/session.rs` serializes/deserializes `WorkspaceSessionRecord` JSON only.
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
| `cargo check --workspace --all-targets` | Passed | Workspace all-target check completed for `legion-ui`, `legion-app`, and `legion-desktop`. |
| `cargo test -p legion-app daily_editing -- --nocapture` | Passed | App daily-editing filters passed: save-all unit coverage, 7 `daily_editing_contracts` tests, and 6 `daily_editing_search` tests. |
| `cargo test -p legion-desktop --all-targets` | Passed | Desktop tests passed, including workflow, daily-editing controls, intent bridge, large-file guardrails, platform smoke, projection rendering, save-all conflict, search workflow, and session restore. |
| `cargo test --workspace --all-targets` | Passed on rerun | After freeing disk space, the workspace all-target test passed. The performance suite reported 7 passed and 3 ignored, including the intentionally ignored 100MB degraded-mode workload. |
| `cargo clippy --workspace --all-targets -- -D warnings` | Passed | Finished warning-clean for `legion-ui`, `legion-app`, and `legion-desktop`. |
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

## 2026-08-17 — P1.F2.T2 dock and panel completion

### What the acceptance required

`plans/kanban/legion-ga-backlog.toml` states one acceptance clause for P1.F2.T2:
"Every layout region has a projection and an integration test." Two words carry
the weight: *every* (the claim is universal, so it needs an enumeration) and
*integration test* (the claim is about coverage, not about rendering).

### What was already true before this change

The individual regions were in far better shape than an "in-progress" card
suggests. Of the nine regions named in the task title, seven already had both a
projection and a named integration test:

| Region | Projection | Pre-existing named integration test |
| --- | --- | --- |
| Top bar | `layout_projection.layout` + product mode | `projection_rendering_desktop_top_bar_uses_three_non_overlapping_regions` |
| Status bar | `active_buffer_projection` | none — assertions only, inside tests named for other subjects |
| Dock layout | `DockLayout::standard` + `PanelRegistry::standard` | `projection_rendering_uses_mode_filtered_dock_registry` |
| File tree | `explorer_projection.nodes` | `projection_rendering_marks_expanded_and_collapsed_explorer_rows` |
| Editor tabs | `daily_editing_projection.tabs.tabs` | `projection_rendering_editor_tabs_expose_tab_state_and_named_close_buttons` |
| Terminal panel | `terminal_panel_projection.output_rows` | `terminal_panel_render_model_exposes_grid_status_and_scrollback` |
| Tests panel | `test_explorer_projection.items` | none — one inline assertion inside an explorer/activity test |
| Problems panel | `language_tooling_projection.problems` | `diagnostic_problems_appear_in_language_rows` |
| Symbols panel | `language_tooling_projection.outline` | `projection_rendering_symbols_setup_and_settings_use_plain_copy_while_diagnostics_keeps_raw_rows` |

What was missing was the *universal* half. The string "layout region" appeared
nowhere in the tree. Nothing enumerated the regions, so nothing could check
"every": a region that stopped projecting, or a region added to the shell with
no coverage at all, would have failed no test. Coverage was an ad-hoc scatter
that happened to be nearly complete, not a gate.

### What changed

1. `crates/legion-ui/src/projection.rs` — added `LayoutRegion`, the missing
   enumeration. It carries a stable id, a label, the projection source each
   region draws from, and `projected_item_count`, which counts what the region
   has to draw from a `ShellProjectionSnapshot`. Every consumer matches
   exhaustively, so adding a region is a compile error until it is given a
   projection source and a covering test.
2. `crates/legion-desktop/tests/layout_region_coverage.rs` — new integration
   test file that walks `LayoutRegion::ALL` and enforces both acceptance
   clauses, in both directions.
3. `crates/legion-ui/src/ui.rs` — removed `AgentLogs` from the Delegate mode's
   bottom dock placement. See "Defect found and fixed".

Two regions are documented exceptions, each behind a named predicate rather
than an unexplained skip:

- **Top bar and dock are persistent chrome**, not content. They keep the mode
  switch and the mode-derived panel placement even for an empty workspace, so
  `is_content_backed()` returns false for them and the "empty shell projects
  nothing" rule does not apply. The dock gets its own negative case instead:
  mode filtering.
- **The symbols panel never reaches `DesktopProjectionViewModel`.** The outline
  is painted straight from the snapshot, so `view_model_rows` returns `None`
  for it and its proof is the rendered-frame AccessKit test named above.

### Defect found and fixed

`DockLayout::standard(DockMode::Delegate)` placed `AgentLogs` in the bottom
dock. `AgentLogs` declares the `Automation` runtime surface, which Delegate mode
does not grant, so `PanelRegistry` filtered the panel out on every render. The
placement was dead: Delegate's dock claimed a panel it could never construct.

The fix is the fail-closed half — drop the placement. The other repair,
granting Delegate the `Automation` surface, would widen a mode authority
boundary and is not a layout decision. A new test,
`layout_region_dock_never_places_a_panel_the_mode_cannot_construct`, walks all
four modes and fails on any recurrence.

Two further assumptions of this agent were caught by its own tests before they
were committed: Manual mode does *not* place fewer dock panels than Automate
(Manual is a deliberately rich local IDE layout), and the file tree does not
project zero rows for an empty shell (it draws an `<empty explorer>` empty-state
row). Both assertions were corrected to the behaviour the code actually has.

### Tests

New in `crates/legion-ui/src/projection.rs` (5):

- `layout_region_all_lists_every_variant_with_unique_ids_and_labels`
- `layout_region_ids_round_trip_and_reject_unknown_values`
- `layout_region_content_backed_regions_are_empty_for_an_empty_shell`
- `layout_region_dock_placement_excludes_panels_the_mode_may_not_construct`
- `layout_region_dock_never_places_a_panel_the_mode_cannot_construct`

New in `crates/legion-desktop/tests/layout_region_coverage.rs` (6):

- `every_layout_region_projects_content_in_a_populated_snapshot`
- `every_content_backed_layout_region_projects_nothing_for_an_empty_shell`
- `every_layout_region_with_a_view_model_field_projects_rows`
- `every_layout_region_names_an_integration_test_that_exists`
- `layout_region_status_bar_projects_active_file_metadata`
- `layout_region_tests_panel_projects_discovered_items_and_run_status`

The last two are the two regions that had a projection but no integration test
named for them. Every one of the eleven has a negative case: an empty shell, an
unknown region id, a mode that must not construct a panel, or a populated row
that must not survive into an empty shell.

### Not claimed

- **The dock's persisted layout does not drive the rendered dock.**
  `DockSideLayout::splitter_fraction` and `collapsed` are persisted by
  `crates/legion-desktop/src/workflow.rs`, validated by `crates/legion-storage`,
  and round-tripped by `session_restore`, but the renderer reads neither. In
  `crates/legion-desktop/src/view.rs` both appear only inside the formatted
  `dock side:` evidence row; dock widths come from egui's own `resizable` panels
  and are never written back, and no code path collapses a dock because
  `collapsed` is true. Restoring a session with a collapsed right dock therefore
  restores the record, not the appearance. P1.F2.T4's "Restart restores the
  layout" is true of the persisted record only. This needs its own task; it is
  not covered by this acceptance and was not fixed here.
- **No claim about how the shell looks or feels when driven by a human.** These
  are projection and view-model tests plus the pre-existing rendered-frame
  AccessKit tests. Nobody drove the built application for this task.
- **No claim that the nine regions are the complete set of shell surfaces.**
  They are the regions this task names. The code canvas is deliberately excluded
  — it belongs to P1.F3.T2 and its own acceptance.
- **`crates/legion-desktop/src/view/dock.rs`, named in the task's `files`, does
  not exist and was not created.** All dock code lives in
  `crates/legion-desktop/src/view.rs`. The `files` list was left as written
  rather than edited to match reality; the discrepancy is recorded here.
- **No performance claim.** Nothing here measures frame time or input latency.

### Verification

| Command | Result |
| --- | --- |
| `cargo fmt --all` | Clean; reformatted the two new/edited sources. |
| `cargo test --workspace --all-targets --no-fail-fast` | Passed. 263 suites, 3006 passed, 0 failed, 19 ignored. |
| `cargo clippy --workspace --all-targets -- -D warnings` | Passed, warning-clean. One `needless_lifetimes` finding in the new test file was fixed. |
| `cargo run -p xtask -- extract-before-modify` | Passed: "no chokepoint file grew past its slack". `crates/legion-ui/src/ui.rs` moved by +6 lines against the merge base. |
| `cargo run -p xtask -- no-egui-textedit` | Passed. |
| `cargo run -p xtask -- docs-hygiene` | Passed. |
| `cargo run -p xtask -- claim-audit` | Passed. |
| `cargo run -p xtask -- verify-kanban-backlog` | Passed: 10 epics, 41 features, 161 tasks. |
| `cargo run -p xtask -- verify-readiness-consistency` | Passed: 161 tasks cross-checked against the readiness ledger. |

Region-scoped runs: `cargo test -p legion-ui --lib layout_region` — 5 passed, 0
failed. `cargo test -p legion-desktop --test layout_region_coverage` — 6 passed,
0 failed.

### Decision

P1.F2.T2 is closed. Its single acceptance clause is now backed by tests that
fail if either half regresses: `every_layout_region_projects_content_in_a_populated_snapshot`
for the projection half, `every_layout_region_names_an_integration_test_that_exists`
for the coverage half, each with a negative case, over an enumeration the
compiler keeps exhaustive. The residual dock rendering gap recorded under "Not
claimed" is real but is not what this acceptance measures; it needs its own
task.
