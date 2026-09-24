# S0-01f Workbench scope audit

Baseline: committed `ce9fbac099835865a531ccbf9c70e558e52d6091`. Implementation traces use `git show ce9fbac:<path>`/`git grep ce9fbac`; concurrent app/UI edits in the worktree are excluded. This increment replaces only `COMP-SCOPE-FAMILY-02` with thirteen atomic Workbench outcomes. All product acceptance remains `unassessed`.

## Governing wording and mappings

Approved family-02 wording is preserved exactly: **“File/folder operations, tabs, splits, multiple windows, layout persistence, settings/profiles, keymaps and discoverable commands”** (`docs/superpowers/specs/2026-09-04-product-completion-design.md:52`). The approved S1-03 deliverable is: **“A Manual workbench contract covering tab opening/closing, splits, the currently authorized read-only canvas arrangement, draggable cards, pan/zoom, person-drawn connections, multiple windows, layout restore, focus order and keyboard navigation.”** (`docs/superpowers/plans/2026-09-04-manual-language-completion.md:114-118`). S1-03B is a design gate for expansion beyond that arrangement (`:136-153`), while S1-03C owns accepted `COMP-CANVAS-*` implementation (`:155-174`). S1-05’s exact current deliverable wording is: **“Explorer create/rename/move/delete/open/reveal, quick file/symbol/definition/references navigation, literal/regex workspace search, replacement preview/apply/cancel and stale-result handling work on real trees while preserving ignores and write safeguards.”** (`docs/superpowers/plans/2026-09-04-manual-language-completion.md:198-216`).

| Exact source promise | Normalized outcome | Canonical mapping |
|---|---|---|
| family-02 wording (`product-completion-design.md:52`) | file/folder operations, tabs, splits, windows, layout persistence, settings/profiles, keymaps, commands | WB001-013; exact legacy canvas rows below |
| S1-03 deliverable (`manual-language-completion.md:118`) | tabs, splits, windows, layout restore, focus/order/keyboard | WB001-005 |
| S1-05 deliverable (`manual-language-completion.md:198-216`) | Explorer create, rename, move, delete, open, and reveal for real file/folder trees with write safeguards | WB006 (open/reveal), WB010-WB013 (create/rename/move/delete); package S1-05 for WB010-WB013 |
| S1-03 tests/scenario (`manual-language-completion.md:131-134`) | dirty tab close, split focus transfer, persisted arrangement, two-window restart restore, recoverable corrupt layout | WB001-005 |
| foundational Phase 5 tabs (`foundational-core-ide-platform-roadmap-v0.1.md:167-178`) | tab/group model, dirty/pinned/preview/activation, explicit split, commands, restore tabs/focus/layout/explorer | WB001-007; exact P1.F2.T4 mapping for broad restart restore |
| foundational Phase 3 tree (`foundational-core-ide-platform-roadmap-v0.1.md:127-137`) | trusted workspace tree, expansion/selection projection, stable IDs | WB006 |
| MANUAL.07 (`legion-production-master-plan-v0.2.md:288`) | focus across editor/panels/palette/terminal/diff | WB004; P1.F2.T3 remains exact pointerless-surface mapping |
| S1-04 settings/keymaps (`manual-language-completion.md:176-196`) | settings profiles/reset and keymap customization | WB008, package S1-04 |
| S1-08 native lifecycle (`manual-language-completion.md:273-291`) | OS-native accessibility, keyboard, DPI/window restore/focus/responsive workload matrix | WB009, package S1-08 |
| Phase 3 GUI evidence (`phase-3-daily-editing-mvp.md:9,93-95,112-137`) | tabs/explorer/dock/session restore projections and known dock restoration gap | WB001, WB003, WB005, WB006 |
| session persistence evidence (`session-persistence-default-evidence.md:5-16,37-39,71-76,91-103`) | workspace-scoped tabs/explorer/dock/session restore and actual dock geometry | WB003, WB005 |
| P0 gap review (`WS-P0/2026-08-31-release-gap-full-pass.md:267-277`) | splits/restoration/settings/profiles remain product gaps despite DTO substrate | WB002, WB005, WB008; source evidence does not promote acceptance |

No inspected approved source promises multi-root workspace switching. It is therefore recorded as an absent promise rather than invented as a requirement. S1-05’s file-operation promise is current required scope: historical status or deferral does not authorize omitting create, rename, move, delete, open, or reveal. Open/reveal for both files and folders is explicit in WB006; create, rename, move, and delete for both files and folders are explicit in WB010-WB013. Canvas arrangement is mapped to exact retained rows `COMP-P6-F5-T1-1` and `COMP-P6-F5-T1-1-02` (package S1-03C), not duplicated here. Existing exact workbench rows are retained: `COMP-P1-F2-T2-1` covers the broad layout-region projection/integration outcome, `COMP-P1-F2-T3-1` covers pointerless interactive surfaces, and `COMP-P1-F2-T4-1` covers broad restart layout restoration; new rows preserve narrower promises those titles do not prove.

## Source-by-source reconciliation

| Inspected source | Relevant promise or limitation | Mapping |
|---|---|---|
| `docs/superpowers/specs/2026-09-04-product-completion-design.md:52` family-02 | Full Workbench family wording, including file/folder operations | WB001-WB013 |
| `docs/superpowers/plans/2026-09-04-manual-language-completion.md:114-174` S1-03/S1-03B/S1-03C | layout contract, canvas boundary, design gate, accepted canvas rows | WB001-005; retained P6.F5 rows |
| `docs/superpowers/plans/2026-09-04-manual-language-completion.md:198-216` S1-05 | exact current file-operation/navigation promise: create, rename, move, delete, open, reveal | WB006, WB010-WB013; no historical deferral omits this current promise |
| `plans/foundational-core-ide-platform-roadmap-v0.1.md:127-178` | tree, tabs/groups, commands, session restore | WB001-007; file-tree authority supports WB006 and WB010-WB013 |
| `plans/foundational-core-ide-platform-implementation-plan-v0.1.md:137-170` Phase 5 | production shell projections and session restore | WB001-007 |
| `plans/legion-production-master-plan-v0.2.md:276-306` WS-MANUAL-01 | keyboard focus and renderer-backed Manual evidence | WB004; native limits remain WB009 |
| `plans/legion-production-roadmap-v1.0.md:19-56` Track A | Manual daily-driver track and product-workflow gate; no additional distinct Workbench promise | context only |
| `plans/product-readiness-ledger.md:49,63,75` PR-UI-001 | cross-OS focus/accessibility/window restore remain open beyond substrate | WB009 |
| `plans/phase-status-ledger.md:13-16` phase summary | phase evidence/status is historical implementation context | no additional outcome |
| `plans/p0-installed-product-sequence-v0.1.md:28-42,95-101` | windowed proof and crash-safe restore gates | WB005, WB009 limits |
| `plans/control-first-adaptive-ide-granular-implementation-plan-v0.1.md` | authority/persistence constraints; no distinct Workbench promise beyond listed contract | no additional outcome |
| `plans/control-first-adaptive-ide-technical-design-v0.1.md:117-127,230-232` | native input→UI intent→app authority and projection-only UI | WB004/007 authority limits |
| `plans/foundational-core-ide-platform-implementation-plan-v0.1.md:139-150` | shell, tabs, panels, command palette, restore | WB001-007 |
| `plans/remaining-implementation-tasks-plan-v0.1.md` | retained future implementation inventory; no additional approved Workbench promise | no additional outcome |
| `plans/evidence/gui-productization/phase-3-daily-editing-mvp.md:9-18,33-37,93-107,112-137` | accepted substrate projections, tabs/tree/dock/session evidence, known dock gap | WB001,003,005,006; acceptance unassessed |
| `plans/evidence/production/WS-MANUAL-01/session-persistence-default-evidence.md:1-6,15-16,37-39,71-103` | workspace-scoped session/tabs/explorer/dock restore and geometry caveats | WB003,005 |
| `plans/evidence/production/WS-P0/2026-08-31-release-gap-full-pass.md:267-277` | splits/profiles remain missing product GUI despite protocol DTOs | WB002,008 partial |
| `docs/ui/canvas-workspace-direction.md:76-84,198-206,260-280` | canvas direction and existing tab/card/dock concepts; S1-03B controls expansion | retained P6.F5 rows; no invented family-02 row |

Historical statuses and evidence labels are not acceptance. Deferred canvas expansion remains required through S1-03B/S1-03C; no historical deferral is used to omit approved Workbench scope.

## Committed implementation traces and limits

| ID | `ce9fbac` source evidence | Classification / limit |
|---|---|---|
| WB001 | `crates/legion-app/src/lib.rs:6277-6482,25062-25074`; `crates/legion-desktop/src/view/tab_strip.rs:119-322` | implemented source paths; native/product acceptance unassessed |
| WB002 | `crates/legion-protocol/src/lib.rs:22144-22164`; `crates/legion-app/src/lib.rs:11449-11500` | partial: DTO/session substrate exists; split command/focus product path not evidenced |
| WB003 | `crates/legion-desktop/src/view/dock_geometry.rs:42-257`; `crates/legion-desktop/src/workflow.rs:3749-3825` | partial: restore/geometry functions exist; persisted rendered collapse/layout qualification has gaps |
| WB004 | `crates/legion-desktop/src/view/keymap_dispatch.rs:77-196`; `crates/legion-desktop/src/view.rs:542-557` | partial: dispatch/palette projections exist; cross-surface native focus sequence unassessed |
| WB005 | `crates/legion-desktop/src/workflow.rs:1899-1948,3749-3869`; `crates/legion-app/src/lib.rs:28580-28637` | partial: session restore paths exist; multiple-window/native restart proof unassessed |
| WB006 | `crates/legion-ui/src/projection.rs:29-90`; `crates/legion-desktop/src/view.rs:8872-8877`; `crates/legion-app/src/intent_routing.rs:571-572` | implemented projection and committed reveal routing traces exist for both file/folder open/reveal scope; external filesystem/native workflow and product acceptance unassessed |
| WB007 | `crates/legion-desktop/src/view.rs:8174-8232`; `crates/legion-desktop/src/view/keymap_dispatch.rs:77-196` | implemented palette/dispatch source exists; discoverability and protocol effect acceptance unassessed |
| WB008 | `crates/legion-protocol/src/lib.rs:20482-20648`; `crates/legion-ui/src/ui.rs` `default_keymap` | partial: settings/keymap DTO/default substrate exists; profile switch/reset UI remains unassessed |
| WB009 | `crates/legion-protocol/src/lib.rs:20482-20648`; `crates/legion-desktop/src/workflow.rs:321-323` | partial: profile/session fields exist; supported-OS native window/DPI/accessibility proof absent |
| WB010 | `crates/legion-app/src/language/translate.rs:8,188,448`; `crates/legion-app/src/bin/golden_path_2.rs:19-20,738-910` | partial: committed workspace-edit/create proposal translation traces exist; Explorer file/folder create projection, authorization, and product acceptance are unassessed |
| WB011 | `crates/legion-app/src/language/translate.rs:8,188,448` | partial: committed rename translation trace exists; Explorer file/folder rename identity/tree behavior and product acceptance are unassessed |
| WB012 | `crates/legion-app/src/language/translate.rs:8,188,448` | partial: committed workspace-operation translation boundary exists; Explorer file/folder move path validation/tree behavior and product acceptance are unassessed |
| WB013 | `crates/legion-app/src/language/translate.rs:8,188,448` | partial: committed delete translation trace exists; Explorer file/folder delete projection, safeguards, and product acceptance are unassessed |

All scenario/configuration/protected/defect arrays for new rows remain empty at this inventory increment.
