# Canvas and workbench additional scope (S0-01 extraction)

This is a source extraction for the Stage-0 completion register. It records
required outcomes and source references; it does not assert implementation or
acceptance. The approved completion contract is the scope authority.

## Source availability

- Available and inspected: `docs/ui/canvas-workspace-direction.md`; `plans/control-first-adaptive-ide-granular-implementation-plan-v0.1.md`; `plans/control-first-adaptive-ide-technical-design-v0.1.md`; `plans/foundational-core-ide-platform-roadmap-v0.1.md`; `plans/remaining-implementation-tasks-plan-v0.1.md`; `docs/superpowers/specs/2026-09-04-product-completion-design.md`; and `plans/kanban/legion-ga-backlog.toml`.
- The separately named `required22families` / `minimumlanguagebreadth` artifact was not found in the repository. The approved spec's §3 family table (lines 49–71) and minimum-language rule (§4, lines 82–90) are the available authority. The table visibly contains 21 rows; no missing 22nd family is inferred here.
- The Claude Design project files named by the canvas direction (§6) are external design-tool sources and were unavailable in the repository. The direction document's transcribed mock details are used; mock-only data is not treated as product fact.

## Existing legacy coverage (do not duplicate)

The following existing kanban rows already name overlapping outcomes. Register
additional atomic requirements beside them only where the outcome below is
more specific or absent; retain these IDs in `legacy_ids` when mapped.

- `P1.F2.T2` covers the broad shell regions; `P1.F2.T3` covers pointer-free Manual navigation; `P1.F2.T4` covers layout metadata persistence. These do not cover the additional workbench outcomes below (especially multiple windows, settings/profiles, command discoverability, or concrete tab/split recovery).
- `P6.F5.T1` covers the current arrangement slice: Canvas rail control, one text card per open file, drag, person-drawn connection, restart persistence, and separation of hand-drawn edges. Do not copy those clauses as new requirements. It does not cover the full-vision node kinds, groups, minimap, derived relationships, or editable workflows.
- `P1.F3.T2`, `P1.F3.T3`, `P1.F3.T5` cover ordinary editor painting, syntax/diagnostics, and Vim input. They are legacy mappings for shared editing behavior, not proof of canvas-card editing.
- `P2.F1.T4`/`T5`/`T6` cover LSP feature reachability, proposal-mediated writes, and call hierarchy; `P2.F4.T1`/`T2`/`T3` cover search/replacement/palette; `P2.F5.T1`–`T4` cover Git basics. Canvas navigation/edge provenance still needs distinct outcomes.

## Atomic canvas outcomes missing from the current Canvas acceptance text

Each item is a candidate `COMP-CANVAS-*` product requirement. Package mapping
is a suggested owner for the register, not an implementation claim.

### Arrangement, navigation, and shell

- **COMP-CANVAS-01 — world grid and transform:** Canvas provides a dot-grid world whose cell size and offset follow zoom/pan, with one world transform, zoom clamped to 0.5–1.6, and zoom centred on the viewport. Source: `docs/ui/canvas-workspace-direction.md` §2 “The canvas surface”. Suggested package: `S1-03C` after `S1-03B`.
- **COMP-CANVAS-02 — background/node gestures:** Background drag pans; node-header drag moves only that node; pointer movement remains correct at non-100% zoom; node positions are world coordinates. Source: same section; existing `P6.F5.T1` covers only basic drag, so map as an extension of that legacy ID. Package: `S1-03C`.
- **COMP-CANVAS-03 — coexisting surfaces:** Canvas is one activity-rail surface alongside Files, Search, Source Control, and Debug; those surfaces remain usable for finding content and placing it on Canvas. Source: §2 “Shell chrome” and note below it. Package: `S1-03`.
- **COMP-CANVAS-04 — graph scope context:** A fixed overlay identifies graph scope and explains arrangement/pan/header-drag gestures. Source: §2 “Screen-space overlays”. Package: `S1-03C`.
- **COMP-CANVAS-05 — mode/trust context:** A fixed overlay explains the active mode and what it means for user authority. Source: §2 “Screen-space overlays”. Package: `S3-01` (Assist/Delegate surfaces), with Manual shell ownership in `S1-03C`.
- **COMP-CANVAS-06 — canvas toolbar:** Fixed controls expose select/pan, add File, add Terminal, zoom out/in, and a clickable percent reset. Source: §2 “Screen-space overlays”. Package: `S1-03C`.
- **COMP-CANVAS-07 — navigator minimap:** Fixed bottom-right minimap shows a scaled rectangle for every node and the current viewport at a stable scale, and supports navigation to the represented world. Source: §2 “Screen-space overlays”. Package: `S1-03C`; requires a performance/accessibility outcome in `XQ-03`.
- **COMP-CANVAS-08 — status summary:** Canvas status exposes state, node/edge counts, pending approvals, test summary, provider, branch, and zoom. Source: §2 “Shell chrome”. Package: `S1-03C` (approval/provider values cross-map to `S3-01`).

### Full node inventory

- **COMP-CANVAS-09 — file card fidelity:** File nodes show line-numbered code, syntax colouring, diff additions/removals, proposal ownership/footer counts, active-agent accent, and active-work status. Source: §2 “Node kinds”, File node. Ordinary editor styling maps to `P1.F3.T2` but the card-specific outcome is new. Package: `S1-03C`.
- **COMP-CANVAS-10 — test nodes:** Test nodes show one row per test with status, name, duration, and running state. Source: §2 “Node kinds”, Test node. Package: `S1-03C`, cross-map `S1-06`/`P2.F3.T4` for actual test effects.
- **COMP-CANVAS-11 — terminal nodes:** Terminal nodes render shell output and a live cursor state. Source: §2 “Node kinds”, Terminal node. Package: `S1-03C`, cross-map `S1-06` for real execution.
- **COMP-CANVAS-12 — proposal cards:** Proposal nodes show approval eyebrow, id/title, owner/risk/file count, and reachable Approve, Review diff, and Reject controls with their stated semantics. Source: §2 “Node kinds”, Proposal card. Package: `S3-01`; proposal lifecycle maps to `P3.F1.*` and is not duplicated.
- **COMP-CANVAS-13 — Assist suggestion cards:** Assist nodes show suggested code, justification, Accept/Dismiss, and the Tab affordance. Source: §2 “Node kinds”, Assist suggestion card. Package: `S3-01`.
- **COMP-CANVAS-14 — common node frame/state:** Every node has header status/title/state, body, optional footer, and node states remain legible while moving/zooming. Source: §2 “Node kinds” introduction. Package: `S1-03C`.
- **COMP-CANVAS-15 — add/open/node lifecycle:** Users can add/open the promised File and Terminal nodes through the fixed controls and preserve node identity/position through the session. Source: §2 toolbar plus “Node positions are world coordinates and persist”. Package: `S1-03C`.

### Relationships, groups, and editing

- **COMP-CANVAS-16 — live edge geometry:** Edges are behind nodes, recompute from live positions, and support horizontal/vertical Bezier variants. Source: §2 “Edges and groups”. Package: `S1-03C`.
- **COMP-CANVAS-17 — edge semantics and labels:** Edge rendering distinguishes solid imports, dashed weaker relations, accent-dashed file→pending-proposal links, and midpoint labels such as `TRAIT BOUNDARY`. Source: §2 “Edges and groups”. Package: `S1-03C` after `S1-03B` provenance decision.
- **COMP-CANVAS-18 — visual groups:** Labelled group frames contain clusters and display their names/counts above the frame. Source: §2 “Edges and groups”. Package: `S1-03C`.
- **COMP-CANVAS-19 — derived imports:** Import relationships are resolved from workspace data and rendered with source/provenance, freshness, and invalidation behavior. Source: §1 concept; §3 “Where edges and groups would come from”; `S1-03B` explicitly requires authority/staleness design. Package: `S1-03B` design gate, then `S1-03C`.
- **COMP-CANVAS-20 — derived calls/references:** Call and reference relationships can be surfaced across files with the same provenance/freshness/invalidation contract. Source: §1 concept; §3 cross-file edge findings; `plans/remaining-implementation-tasks-plan-v0.1.md` R3 semantic records (symbols/references/imports/call edges). Package: `S1-03B` then `S1-03C`; semantic prerequisite may be `S2-03`.
- **COMP-CANVAS-21 — trait/type relationships:** Trait/type-boundary relationships have an authoritative source and are shown with explicit relation type rather than guessed structure. Source: §2 label example; §3 “There is no trait-boundary data”. Package: `S1-03B` then `S1-03C`.
- **COMP-CANVAS-22 — proposal/test/terminal relationships:** Links between files and pending proposals, tests, and terminal commands carry an explicit source relation and stale-result behavior. Source: §1 concept (proposal/terminal/test nodes) and §2 edge styles. Package: `S1-03B` then `S1-03C`, with `S3-01` for proposal state.
- **COMP-CANVAS-23 — hand-drawn versus derived identity:** Person-drawn edges remain a distinct relation type; derived edges never reinterpret or overwrite them. Source: §1 and “What is built” (hand-drawn edges), plus `P6.F5.T1` acceptance. Map the existing clause to `P6.F5.T1`; add only derived-edge separation/provenance extension. Package: `S1-03B`.
- **COMP-CANVAS-24 — editable canvas workflow:** Any approved editable card interaction must use the ordinary editor/app mutation and proposal/save routes, with real buffer mutation, navigation, save, and rejection recovery. Source: `docs/superpowers/plans/2026-09-04-manual-language-completion.md` S1-03B/S1-03C (interfaces and acceptance); approved spec §3 Canvas. Package: `S1-03C` after accepted contract; current ADR-0051 read-only behavior remains mapped to `S1-03`.
- **COMP-CANVAS-25 — read-only centre contract:** Until an amended ADR authorizes editing, Canvas cards remain read-only views and keyboard input while Canvas is central cannot mutate buffers. Source: manual plan S1-03 deliverable; `docs/ui/canvas-workspace-direction.md` status and ADR-0051 statement. Package: `S1-03` (preservation requirement, map `P6.F5.T1` where applicable).
- **COMP-CANVAS-26 — canvas recovery/accessibility:** Canvas controls, nodes, edges, minimap, zoom/pan, focus loss/cancel/restart, corrupt layout, and stale derived results have accessible operation and recoverable failure outcomes. Source: manual plan S1-03/S1-03C acceptance; canvas direction §3 accessibility/performance findings. Package: `S1-03C` with `XQ-02` and `XQ-03`.

## Workbench essentials beyond current acceptance text

These are atomic expansions of approved spec §3 “Workbench”, §3 “Text
editing”, §3 “Work preservation”, and manual plan S1-03/S1-04. Existing broad
`P1.F2.T2`/`T3`/`T4` rows are mapped where they already cover the clause.

- **COMP-WB-01 — file/folder lifecycle:** Create, open, close, rename, delete, and reveal files/folders through ordinary controls, with path-policy and failure feedback. Source: approved spec §3 Workbench; manual plan S1-05. Package: `S1-05`.
- **COMP-WB-02 — tab lifecycle:** Open, switch, reorder, and close tabs; closing dirty work produces an actionable recovery decision. Source: manual plan S1-03 and S1-05; map existing shell projection to `P1.F2.T2`, add dirty-close acceptance under `S1-03`. Package: `S1-03`.
- **COMP-WB-03 — split lifecycle:** Create, focus, resize, and close splits; focus transfer is visible and keyboard-operable. Source: manual plan S1-03 and native scenario `manual-workbench-layout-restart`. Package: `S1-03`; broad shell map `P1.F2.T2`.
- **COMP-WB-04 — multiple windows:** Open/use more than one window and preserve each window's workspace/layout identity across restart. Source: manual plan S1-03 deliverable and acceptance. Package: `S1-03`.
- **COMP-WB-05 — layout persistence as rendered behavior:** Restart restores the layout the user sees, not only a serialized record; corrupt layout falls back to recoverable defaults with a diagnostic. Source: manual plan S1-03 acceptance; kanban `P1.F2.T4` notes explicitly distinguish record round-trip from rendered restore. Package: `S1-03`, map `P1.F2.T4` as legacy defect context.
- **COMP-WB-06 — settings and profiles:** Settings/profiles are discoverable, persist safely, and expose the active profile/configuration without leaking restricted context into Manual mode. Source: approved spec §3 Workbench; granular plan P5 trust/configuration direction; Manual-mode legacy `P1.F2.T1` maps only to silence. Package: `S1-04` plus `S3-01` for trust settings.
- **COMP-WB-07 — keymaps:** Keymaps are configurable or explicitly published, conflict-safe, and cover all required workbench/canvas controls. Source: approved spec §3 Workbench and manual plan S1-04. Package: `S1-04`; map `P1.F2.T3` for existing keyboard reachability.
- **COMP-WB-08 — discoverable commands:** Every user-facing command has a reachable command id/palette/menu/keybinding route and a visible unavailable reason where blocked. Source: approved spec §3 Workbench; current kanban `P2.F4.T3` covers palette reachability and maps as duplicate for that subset. Package: `S1-04`.
- **COMP-WB-09 — focus and accessibility order:** Explorer, tabs, splits, editor, Canvas, terminal, diagnostics, search, source control, and dialogs have deterministic focus order, keyboard operation, and accessible names/bounds. Source: approved spec §3 Platform quality; manual plan S1-03/S1-08. Package: `S1-08` and `XQ-02`; map `P1.F2.T3` only for existing Manual surfaces.
- **COMP-WB-10 — editing semantics:** Selection, Unicode, undo/redo, multi-cursor, indentation, wrapping, clipboard, IME, Vim, large-file behavior, and predictable input are all accepted through the product input route. Source: approved spec §3 Text editing; manual plan S1-02/S1-04/S1-08. Existing Vim/multicursor rows map (`P1.F3.T2`/`T5`); package: `S1-04`.
- **COMP-WB-11 — ordinary save/recovery:** Open/edit/save through the packaged application preserves dirty text on denial/conflict, handles external edits, checkpoints, restart/restore, storage failure, and safe migration. Source: approved spec §3 Work preservation; manual plan S1-05/S1-07/S1-08. Existing save/Git rows map only to their clauses. Package: `S1-05` and `S1-07`.
- **COMP-WB-12 — workload behavior:** Workbench/canvas retains input, scroll, startup, resource, and sustained-use budgets on declared configurations, including canvas edge/minimap cost. Source: approved spec §3 Platform quality; canvas direction §3 Performance; manual plan S1-08; `XQ-03`. Package: `XQ-03`.

## Minimum language breadth required by the approved scope

The approved spec §3 lines 82–90 requires named versions and representative
repositories for Rust binaries/libraries/multi-crate workspaces, TypeScript and
JavaScript browser and Node applications, and Python applications/packages in
isolated environments. For **each** group, the atomic outcomes are:

- package/environment discovery and setup;
- language-server provisioning, discovery, lifecycle, restart/failure handling;
- completion, diagnostics, hover, definition/references, symbols and navigation;
- rename, formatting, code actions and refactoring as reviewable workspace edits;
- multi-file navigation/refactoring;
- build/task execution and test discovery, targeted/group runs, passing/failing results;
- real breakpoints, stepping, variables/evaluation/debug console and termination;
- tool failure and restart recovery;
- a native packaged journey that reproduces a defect, navigates/refactors, debugs, fixes, tests, and commits.

Sources: approved spec §3 “Language tooling” and §4 minimum project breadth;
manual plan S2-01 through S2-06; roadmap Phase 8; granular plan P3/P4
semantic/LSP sections. Suggested package: `S2-01` through `S2-06`; map
already-specific kanban rows `P2.F1.*`, `P2.F3.*`, and related rows rather than
creating duplicate requirements.

## Other approved-scope promises that must remain represented

The following are additional atomic register prompts from approved spec §3
lines 54–71. They are included so S0-01 does not silently narrow scope while
extracting Canvas/Workbench. Use the owning package named where the current
plans already provide one; otherwise create a linked Stage 3–5 package.

- **Navigation/search:** file and symbol navigation; literal and regex search; workspace replacement; ignore rules; cancellation; stale-result handling. Source: approved spec §3 Navigation/search. Existing duplicate mappings: `P2.F4.T1`–`T3`; missing stale/ignore/recovery details package `S1-05`.
- **Build/test/debug:** build/task execution; discovery; targeted/group runs; real adapters; breakpoints; stepping; variables; evaluation; debug console; termination. Source: approved spec §3 Build/test/debug; package `S2-04`/`S2-05`; map `P2.F3.*` for already-specific rows.
- **Terminal:** real shell process execution; interactive programs; control keys; resize; scrollback/output; working directory; process cleanup. Source: approved spec §3 Terminal; manual plan S1-06; map `P2.F2.*` where present.
- **Source control/history:** status/diffs/staging/commits/branches/conflict resolution/remote verbs/review integration/local history. Source: approved spec §3 Source control; manual S1-07; map `P2.F5.*` for existing clauses.
- **Provider/model setup:** local/hosted configuration; credentials; managed local runtime; model download/integrity; hardware fit; useful first-run setup. Source: approved spec §3 Provider/model setup; package `S3-01`/`S3-02`.
- **Context/retrieval:** index freshness; symbols; memory; provenance; token/context budgets; privacy boundaries; inspectable selection. Source: approved spec §3 Context/retrieval; granular P3/P4 and manual plan S3 context work.
- **Assist:** ghost text/completion; explanations; inline and multi-file changes; progressive feedback; review/apply/reject; cancellation. Source: approved spec §3 Assist; package `S3-01`/`S3-02`.
- **Delegate:** scoped tasks; isolated execution; real tool loop; verification; reviewable changes; resource limits; restart recovery. Source: approved spec §3 Delegate; package `S3-03`/`S4-01`; map `P6.F4.*` for existing external-agent clauses.
- **Multi-agent workflows:** editable plans; dependency execution; worker coordination; conflicts; fleet controls; budgets; interruption; replay. Source: approved spec §3 Multi-agent workflows; package `S4-01`.
- **External interoperability:** required MCP/ACP roles and real interoperability with named supported implementations. Source: approved spec §3 External interoperability; package `S4-02`.
- **Trust/proposals:** lifecycle, approvals, policy, audit, cancellation, rollback, secret handling, visible egress. Source: approved spec §3 Trust/proposals; granular P1/P5/P6 and kanban P3 rows; map existing specific rows, retain gaps as atomic requirements.
- **Extensions:** WASM/runtime and required VS Code webview, notebook, custom-editor, storage capabilities; install/update/disable/remove; permissions; isolation; failure handling; distribution. Source: approved spec §3 Extensions; remaining plan R5; package `S5-01`/`S5-02`; `P7.F2.T4` is metadata-only compatibility and is not a duplicate of execution capabilities.
- **Remote development:** SSH/container environments; remote files/tools/terminal/debugging; reconnect; offline transitions; workspace identity. Source: approved spec §3 Remote development; remaining plan R7; package `S5-03`.
- **Collaboration/enterprise:** shared state/proposals/review; reconciliation; administration; identity; policy distribution; audit/export; retention. Source: approved spec §3 Collaboration/enterprise; remaining plan R6/R8; package `S5-04`.
- **Training/telemetry:** explicit opt-in capture; redaction; export; deletion; retention; feedback mechanisms; Manual privacy intact. Source: approved spec §3 Training/telemetry; remaining plan R8; package `S5-05`.
- **Platform quality:** Windows/macOS/Linux; accessibility; focus; DPI; window restore; responsiveness; startup; resource bounds; sustained use. Source: approved spec §3 Platform quality; manual S1-08; cross-cutting package `XQ-02`/`XQ-03`/`XQ-04`.

## Package and authority note

The approved master plan says S0-01 must assign every required row to a linked
package and must inventory full canvas direction without treating historical
titles as current truth (`docs/superpowers/plans/2026-09-04-full-product-completion.md`
S0-01). The manual plan specifically assigns core Canvas arrangement/navigation/
editing to Stage 1, AI/workflow Canvas integrations to Stages 3–4, and team
integrations to Stage 5 (`docs/superpowers/specs/2026-09-04-product-completion-design.md`
§3 lines 112–124). Derived-edge/editable-card scope therefore remains gated by
the S1-03B design decision before S1-03C implementation rows are created.
