# S0-01h Navigation/search scope audit

Baseline: committed `f4eb546`. Implementation traces use `git show f4eb546:<path>`/`git grep f4eb546`; concurrent navigation-source edits in the worktree are excluded. This increment replaces only `COMP-SCOPE-FAMILY-04` with thirteen atomic `COMP-NAV-*` outcomes. The 243 other register rows remain field-identical and all product acceptance remains `unassessed`.

## Governing wording and deduplication

The approved family-04 wording is **“Navigation/search”** (`docs/superpowers/specs/2026-09-04-product-completion-design.md:54`). The current S1-05 deliverable is: **“Explorer create/rename/move/delete/open/reveal, quick file/symbol/definition/references navigation, literal/regex workspace search, replacement preview/apply/cancel and stale-result handling work on real trees while preserving ignores and write safeguards.”** (`docs/superpowers/plans/2026-09-04-manual-language-completion.md:198-216`). Its interfaces require `WorkspaceSearchReplaceCapability`, app proposal routes, canonical paths/file identities, and stale snapshot/conflict denial. Its native journey requires navigating a multi-file symbol, running regex replacement, inspecting a diff, cancelling one preview, applying another, triggering an external edit before apply, and verifying disk state with host reads.

The retained broad rows are canonical where exact: `COMP-P2-F4-T1-1` owns the broad Legion-repo search-speed outcome, `COMP-P2-F4-T2-1` owns the invariant that replace never mutates outside proposal review, and `COMP-P2-F4-T4-1` owns bounded large-fixture search. `COMP-WB-007` owns discoverable command-palette dispatch/open/reveal, not quick file/symbol result semantics. `COMP-P2-F1-T4-1` and `COMP-P2-F1-T6-1` remain canonical for broad LSP capability and call-hierarchy protocol outcomes; the new rows capture the narrower user navigation journeys and visible stale/empty/unavailable behavior that those broad rows do not prove. Existing Workbench CRUD rows remain unchanged.

No historical done status or evidence is treated as product acceptance. Search evidence from M8 is implementation context; current approved S1-05 and native workflow requirements remain open. The unavailable historical generator source listed by traceability is recorded there and not inferred.

## Canonical promise mapping

| Exact source promise | Normalized atomic outcome | Canonical mapping |
|---|---|---|
| Family-04 (`product-completion-design.md:54`) | Navigation/search family | NAV001-NAV013 plus exact retained P2.F4/P2.F1 broad rows |
| S1-05 deliverable (`manual-language-completion.md:198-216`) | file/symbol/definition/references navigation | NAV001-NAV006 |
| S1-05 deliverable (`manual-language-completion.md:198-216`) | literal/regex search, ignores, write safeguards | NAV007-NAV008 |
| S1-05 deliverable and native scenario | replacement preview/apply/cancel | NAV009-NAV011 |
| S1-05 stale/conflict interface and native scenario | stale snapshot, conflict, path denial, preserved work | NAV012 |
| S1-05 native `manual-refactor-search-review` scenario | real tree, host-observed effects and recovery evidence | NAV013 |
| Traceability P2.F4.T1 (`completion-traceability.md:94`) | real workspace search with streaming/cancellation | NAV007-NAV008; exact broad speed row retained |
| Traceability P2.F4.T2 (`completion-traceability.md:95`) | multi-file search/replace proposal | NAV009-NAV012; exact proposal-only invariant retained |
| Traceability P2.F4.T3 (`completion-traceability.md:96`) | fuzzy file finder, symbol finder, recent buffer switcher | NAV001, NAV002, NAV005 |
| Traceability P2.F4.T4 (`completion-traceability.md:97`) | 50K/100K fixture bounds | NAV008; exact bounded-search row retained |
| S1-05 files/tests (`manual-language-completion.md:203-216`) | project search, app proposal, desktop workflow and native test surfaces | NAV007-NAV013 |

## Source-by-source reconciliation

| Inspected source | Relevant promise or limitation | Mapping |
|---|---|---|
| `docs/superpowers/specs/2026-09-04-product-completion-design.md:41-45,54,113-124` | Navigation/search is required Stage-1 daily-driver scope; current code/results govern implementation facts | NAV001-NAV013 |
| `docs/superpowers/plans/2026-09-04-manual-language-completion.md:198-216` | Exact S1-05 file navigation, search, replacement, stale/conflict and native journey | NAV001-NAV013 |
| `docs/superpowers/plans/2026-09-04-completion-traceability.md:7-18,28-34,94-97` | S1 routing, source-of-truth rule, P2.F4 search promises and additional source set | NAV001-NAV013; retained exact P2.F4 rows |
| `plans/legion-production-master-plan-v0.2.md:180-205,308-324,598-630` | app/project authority, large workspace/search scale, review/evidence/rollback boundaries | NAV007-NAV013 |
| `plans/legion-production-roadmap-v1.0.md:49-60` | Manual daily-driver sequencing; no exclusion of S1-05 navigation/search | context/limit; no additional row |
| `plans/product-readiness-ledger.md:49-52` | workflow validation and external-tool evidence remain separate from substrate | NAV013; acceptance unassessed |
| `plans/phase-status-ledger.md:13-16,39-42` | accepted substrate and gated product workflows; statuses do not overrule current scope | implementation context only |
| `plans/p0-installed-product-sequence-v0.1.md:96-101,120-127,137-145` | restart, installed/native and accessibility evidence requirements | NAV012-NAV013 |
| `plans/control-first-adaptive-ide-granular-implementation-plan-v0.1.md:155-185,207-223,361-379` | editor/workspace authority, proposals, rollback, stale/conflict, non-blocking search | NAV007-NAV012 |
| `plans/control-first-adaptive-ide-technical-design-v0.1.md:327-353,397-412,572-585` | projection-only UI, proposal authority and evidence ownership | NAV009-NAV013 |
| `plans/foundational-core-ide-platform-roadmap-v0.1.md:9-15,127-141` | trusted workspace/tree foundation and project authority | NAV001, NAV007, NAV012 |
| `plans/foundational-core-ide-platform-implementation-plan-v0.1.md:137-170` | shell/tree/search projection substrate | NAV001, NAV007-NAV008 |
| `plans/remaining-implementation-tasks-plan-v0.1.md:30-45,165-185` | accepted proposal/search substrate and path safeguards; future breadth remains gated | NAV007-NAV012 |
| `plans/kanban/legion-ga-backlog.toml:P2.F4` | exact search/replace/finder/large-fixture task acceptance and stop conditions | retained P2.F4 rows; NAV001-NAV013 |
| `plans/evidence/production/M8/WS-SEARCH-01-evidence.md:61-205,220-404` | option threading/cancel/stale/binary/history/50K evidence and proposal coverage/limits | NAV001, NAV005, NAV007-NAV012; historical evidence only |
| `crates/legion-project/src/lib.rs` | SearchPattern/options, ignores, bounded streaming, cancellation and diagnostics | NAV007-NAV008 |
| `crates/legion-app/src/search.rs` | search projection/cancel/stale and complete workspace-replace preview | NAV007-NAV012 |
| `crates/legion-desktop/src/search.rs` | search status, stale/degraded/error presentation | NAV008 |
| `crates/legion-app/src/lib.rs` | palette modes/results, definition/reference/local-history/proposal handlers | NAV001-NAV006, NAV011-NAV012 |
| `crates/legion-desktop/src/bridge.rs` and `crates/legion-app/src/intent_routing.rs` | definition/reference intent routing | NAV003-NAV004 |
| `crates/legion-lsp/src/features.rs` | definition/reference/document-symbol request/projection contracts | NAV002-NAV004 |
| `crates/legion-ui/src/ui.rs` | search projections, stale/error states, replacement intents, local-history projection | NAV005-NAV012 |
| `crates/legion-storage/src/lib.rs` | local-history metadata/blob persistence and trust checks | NAV006 |
| `crates/legion-project/tests/search_workspace.rs` and `crates/legion-app/tests/workspace_replace_proposal.rs` | named test surfaces for search/replacement | NAV013; tests do not equal native acceptance |

## Committed implementation traces and limits

| ID | `f4eb546` source evidence | Classification / limit |
|---|---|---|
| NAV001 | `crates/legion-app/src/lib.rs:13137-13142,17051-17059` `PaletteMode::File`/`palette_file_results`; project discovery | implemented source route; real-tree/native acceptance unassessed |
| NAV002 | `crates/legion-app/src/lib.rs:13140,17056-17058`; `crates/legion-lsp/src/features.rs:87-100` | partial: symbol mode and document-symbol contract exist; complete workspace symbol navigation/product acceptance unassessed |
| NAV003 | `crates/legion-desktop/src/bridge.rs:953-965,2582-2590`; `crates/legion-app/src/lib.rs:18847-18858` | partial: definition intent/handler route exists; live server result/navigation and stale/unavailable UX unassessed |
| NAV004 | `crates/legion-desktop/src/bridge.rs:958-965,2590-2598`; `crates/legion-lsp/src/features.rs:16-18` | partial: references route/contract exists; complete visible result navigation and stale/unavailable UX unassessed |
| NAV005 | `crates/legion-app/src/lib.rs:13141,17058`; `plans/evidence/production/M8/WS-SEARCH-01-evidence.md:120-153` | implemented source route/persistence trace; native recent-buffer workflow acceptance unassessed |
| NAV006 | `crates/legion-app/src/lib.rs:9702-9713,18769-18783,25597-25893`; `crates/legion-storage/src/lib.rs` local-history store | implemented source route with proposal restore/trust checks; product recovery/native acceptance unassessed |
| NAV007 | `crates/legion-project/src/lib.rs:96-288,5362-5575`; `crates/legion-app/src/search.rs`; `crates/legion-ui/src/ui.rs:1208-1299` | implemented search source/projection; real-tree acceptance and large-workspace product gate unassessed |
| NAV008 | `crates/legion-project/src/lib.rs:275-288,5401-5575`; `crates/legion-app/src/search.rs`; `crates/legion-desktop/src/search.rs:20-50` | partial: cancellation, diagnostics, stale/degraded states and bounds are traced; native timing/error/recovery evidence unassessed |
| NAV009 | `crates/legion-app/src/search.rs` `propose_workspace_replace`; `plans/evidence/production/M8/WS-SEARCH-01-evidence.md:220-316` | implemented preview/proposal construction trace; complete renderer review and native acceptance unassessed |
| NAV010 | `crates/legion-ui/src/ui.rs:1595-1612,3853-3864`; proposal lifecycle routes in `crates/legion-app/src/lib.rs` | partial: preview/cancel intents and lifecycle substrate exist; no native no-mutation cancellation record |
| NAV011 | `crates/legion-app/src/search.rs`; `crates/legion-app/src/lib.rs` workspace-edit apply; `plans/control-first-adaptive-ide-granular-implementation-plan-v0.1.md:174-185` | partial: proposal/precondition/rollback source paths exist; external host effect and renderer acceptance unassessed |
| NAV012 | `crates/legion-app/src/search.rs` complete-target/live-buffer guards; `crates/legion-project/src/lib.rs` path policy/search identity | partial: fail-closed source guards exist; external edit timing, conflict UX and native recovery unassessed |
| NAV013 | named project/app test surfaces plus S1-05 native scenario | partial: test harness/source surfaces exist; no current packaged external-oracle EvidenceRun proving the full journey |

All product acceptance is `unassessed`; no classification is inferred from historical `done` labels or file existence. New scenario/configuration/protected/defect arrays are empty at this inventory increment.
