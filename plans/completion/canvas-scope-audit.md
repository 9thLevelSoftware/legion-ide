# S0-01g Canvas scope audit

Baseline: committed `6750a7d` (`6750a7d`); implementation traces use `git show 6750a7d:<path>`/`git grep 6750a7d`. Concurrent navigation/app/UI edits in the worktree are excluded. This increment replaces only `COMP-SCOPE-FAMILY-03` with ten atomic `COMP-CAN-*` outcomes. The 234 other register rows remain field-identical and all product acceptance remains `unassessed`.

## Governing source wording and scope boundary

The approved family-03 wording is **“Canvas”** (`docs/superpowers/specs/2026-09-04-product-completion-design.md:53`). The S1-03C deliverable is: **“The complete Stage-1 canvas arrangement, navigation and editing outcomes assigned by S1-03B work through the accepted authority path, have accessible/discoverable controls, preserve current ADR-0051 behavior until the amendment takes effect, and have a native acceptance record per required configuration.”** (`docs/superpowers/plans/2026-09-04-manual-language-completion.md:155-159`). Its acceptance requires **“actual buffer mutation, saved file, navigation result or provenance-labeled edge state”** and each packaged scenario must verify **“real edit/save/restore through host file reads, keyboard and accessibility operation, stale derived-edge invalidation where applicable, focus loss/cancel/restart recovery, and canonical `EvidenceRun` JSON”** (`:169-174`).

The retained P6.F5 wording is preserved and mapped rather than duplicated: **“A Canvas control on the activity rail switches the centre; every open file is a card carrying its own text; a card can be dragged and two cards connected; the arrangement survives a restart.”** (`plans/kanban/legion-ga-backlog.toml:1915`, canonical `COMP-P6-F5-T1-1`), and **“Edges are the person's own claim, kept in their own type, so a derived import or call graph can be added later without reinterpreting them.”** (same source, canonical `COMP-P6-F5-T1-1-02`). New rows capture narrower full-vision promises those retained outcomes do not prove.

The direction document promises spatial behavior: an infinite pan/zoom surface, world-space node movement and persistence, file/test/terminal/proposal/Assist node kinds, typed visual edges and groups, graph scope, toolbar and navigator minimap (`docs/ui/canvas-workspace-direction.md:76-170`). It also describes a workflow-style working set with approval and execution cards (`:86-92,127-142`). These are current scope leads for the approved Canvas family; historical deferral or the absence of a current implementation does not remove them from the inventory. Editor text controls remain the `COMP-EDIT-*` family and are not duplicated here. ADR-0051's current read-only card boundary is recorded as an implementation/authority limit until an accepted S1-03B contract amends it.

The planned full-vision file `docs/superpowers/specs/2026-09-04-canvas-full-vision-spec.md` is unavailable in the committed baseline; it is recorded as unavailable rather than inferred. The S1-03B instructions requiring that file (`manual-language-completion.md:136-153`) remain an open design gate, while the approved family, direction, ADR, backlog and S1-03C deliverable still provide concrete inventory promises.

## Canonical promise mapping

| Exact source promise | Normalized atomic outcome | Canonical mapping |
|---|---|---|
| Family-03 Canvas (`product-completion-design.md:53`) | Full Canvas arrangement, navigation and editing family | CAN001-CAN010 plus retained P6.F5 rows |
| S1-03B inventory gate (`manual-language-completion.md:136-153`) | Every direction promise receives a finite requirement with authority/provenance/staleness/performance/accessibility ownership | CAN001-CAN010; unavailable full-vision spec recorded above |
| S1-03C deliverable (`manual-language-completion.md:155-174`) | Accepted arrangement, navigation/editing, accessible controls, native configuration evidence and recovery | CAN001-CAN010; retained P6.F5 rows for exact arrangement/edges |
| Direction surface (`canvas-workspace-direction.md:112-170`) | Pan/zoom, node movement, card kinds, edges/groups, overlays, toolbar/minimap | CAN001-CAN006; CAN004/CAN005 cover card kinds/groups/overlays |
| Direction workflow working set (`canvas-workspace-direction.md:76-92,127-142`) | Proposal, test, terminal and Assist cards with real action/status behavior | CAN004, CAN007 |
| Direction toolbar (`canvas-workspace-direction.md:158-170`) | Discoverable `+ File` and `+ Terminal` spatial node creation controls | CAN010 |
| Direction gap/edge authority (`canvas-workspace-direction.md:297-365`) | Derived edges need workspace-wide resolution, provenance and bounded performance | CAN008; current absence is a limit, not a scope omission |
| ADR-0051 arrangement (`ADR-0051:27-64`) | Existing file-card authority, person-owned placement/edges, editor-only buffer mutation | retained P6.F5 rows; CAN002, CAN003, CAN006 |
| P6.F5.T1 acceptance (`legion-ga-backlog.toml:1908-1918`) | Reachable Canvas, real open-file cards, drag/connect, restart persistence | exact mappings to `COMP-P6-F5-T1-1` and `COMP-P6-F5-T1-1-02`; no duplicate |
| S1-03C native gate (`manual-language-completion.md:173-174`) | Native external-effect, accessibility, stale/recovery and EvidenceRun records | CAN009 |

## Source-by-source reconciliation

| Inspected source | Relevant promise or limitation | Mapping |
|---|---|---|
| `docs/superpowers/specs/2026-09-04-product-completion-design.md:41-53,113-124` | Canvas is a required Stage-1 arrangement/navigation/editing family; each canvas requirement needs a journey and stage owner | CAN001-CAN010; retained P6.F5 rows |
| `docs/superpowers/plans/2026-09-04-manual-language-completion.md:136-174` | S1-03B full-vision inventory/design gate; S1-03C implementation, authority, controls, native acceptance and recovery | CAN001-CAN010 |
| `docs/ui/canvas-workspace-direction.md:1-14,76-170` | Spatial canvas, workflow-style working set, node kinds, pan/zoom, edges/groups, graph scope and minimap | CAN001-CAN007 |
| `docs/ui/canvas-workspace-direction.md:256-365` | Persistence gap, no current free-positioned/derived graph implementation, provenance and performance requirements | CAN002, CAN008, CAN009; implementation limits |
| `plans/adrs/ADR-0051-canvas-workspace-surface.md:27-100` | Current file-card arrangement, person-drawn edges, session persistence, editor-only mutation and no readiness claim | retained P6.F5 rows; CAN002, CAN003, CAN006; limits for CAN004/CAN008/CAN009 |
| `plans/kanban/legion-ga-backlog.toml:1904-1922` | P6.F5 reachable cards, drag/connect, restart and person-edge acceptance/stop condition | exact retained P6.F5 rows; no duplicate |
| `plans/legion-production-master-plan-v0.2.md:180-205,577-596` | Projection-only UI, app authority, workflow command-center execution, evidence, stop and merge readiness | CAN006, CAN007; source classification remains separate from Canvas acceptance |
| `plans/legion-production-roadmap-v1.0.md:49-60` | Manual daily-driver sequencing and later workflow breadth; no exclusion of approved Canvas family | context/limits; no additional atomic Canvas promise |
| `plans/product-readiness-ledger.md:49-52` | Renderer-backed accessibility/performance and cross-OS evidence bar | CAN009; acceptance remains unassessed |
| `plans/phase-status-ledger.md` | Historical phase status cannot overrule current approved scope | no new outcome; historical-status limit |
| `plans/p0-installed-product-sequence-v0.1.md:96-101,120-127` | Restart/windowed proof and native external-effect gates | CAN009 recovery/evidence mapping |
| `plans/control-first-adaptive-ide-granular-implementation-plan-v0.1.md:207-223,361-379` | UI emits intents/projections; semantic work must not block interactive editor/workflow paths | CAN006/CAN008 authority and performance limits |
| `plans/control-first-adaptive-ide-technical-design-v0.1.md:327-353,572-585` | UI projection boundaries, app/editor ownership, phase evidence and ownership tests | CAN006-CAN009 authority/evidence limits |
| `plans/foundational-core-ide-platform-roadmap-v0.1.md:167-178` | Existing Phase-5 tabs/groups/commands/session restore substrate | retained Workbench mappings; no duplicate Canvas outcome |
| `plans/foundational-core-ide-platform-implementation-plan-v0.1.md:137-170` | Shell projection and restore substrate | retained Workbench mappings; no duplicate Canvas outcome |
| `plans/remaining-implementation-tasks-plan-v0.1.md` | Current accepted slices and gated future expansions | no additional distinct Canvas promise |
| `docs/superpowers/specs/2026-09-04-canvas-full-vision-spec.md` | Unavailable in committed baseline | explicitly unavailable; not inferred |

Editor text entry, selection and buffer mutation promises were inspected only to enforce the boundary: they map to existing `COMP-EDIT-*` rows and ADR-0051 says Canvas cards are read-only. They do not become spatial Canvas requirements.

## Committed implementation traces and limits

| ID | `6750a7d` source evidence | Classification / limit |
|---|---|---|
| CAN001 | `crates/legion-desktop/src/view/canvas_workspace.rs` `render_canvas_workspace`, Scene constants; `crates/legion-desktop/tests/canvas_workspace.rs` pan/focus tests | implemented source path for pan/zoom/focus-follow behavior; full native/product acceptance unassessed |
| CAN002 | `crates/legion-desktop/src/bridge.rs:143-177` `MoveCanvasNode`/`PlaceCanvasNodes`; `crates/legion-desktop/src/workflow.rs:953-1010`; rendered keyboard/drag tests | implemented source path for movement and arrangement actions; native acceptance unassessed |
| CAN003 | `crates/legion-desktop/src/bridge.rs:177-190`; `crates/legion-desktop/src/workflow.rs:1012-1030`; connection accessibility tests | implemented person-edge source path; derived-edge semantics and native acceptance unassessed |
| CAN004 | `crates/legion-desktop/src/view/canvas_workspace.rs` `CanvasNode` file-card projection; `crates/legion-desktop/src/view.rs` workflow canvas projections | partial: file cards exist, promised test/terminal/proposal/Assist Canvas node kinds and actions are not evidenced |
| CAN005 | `crates/legion-desktop/src/view/canvas_workspace.rs` current renderer; direction overlay/group/minimap promise | absent: no committed group/minimap/graph-scope implementation path |
| CAN006 | `crates/legion-desktop/src/view/canvas_workspace.rs` `accessible_name`, `active`, card activation; canvas tests | partial: accessible card/port projection and active-buffer navigation traces exist; complete keyboard/assistive workflow acceptance unassessed |
| CAN007 | `crates/legion-desktop/src/view.rs` `render_fleet_canvas`/`render_delegated_canvas`; `crates/legion-agent/src/{dag.rs,coordinator.rs}` workflow authority | partial: workflow authority/projections exist outside Canvas; Canvas node action integration is not evidenced |
| CAN008 | `crates/legion-desktop/src/view/canvas_workspace.rs` explicit derived-edge absence; `plans/adrs/ADR-0051-canvas-workspace-surface.md:43-45,71-72` | absent: no committed accepted derived-edge producer/provenance/invalidation path; no generic no-op substitutes the requirement |
| CAN009 | `crates/legion-desktop/tests/canvas_workspace.rs` rendered arrangement/accessibility tests; `xtask/tests/native_product_acceptance.rs` native harness exists | partial: source tests/harness exist, but Canvas native per-configuration EvidenceRun and full recovery/performance records are not evidenced |
| CAN010 | `crates/legion-desktop/src/view/canvas_workspace.rs` current file-card construction; `crates/legion-desktop/src/view.rs` existing workflow surface projections | partial: committed file cards and separate workflow projections exist, but promised +File/+Terminal spatial creation routing and recovery are not evidenced |

Classification is based on concrete committed symbols or an explicit limit; file existence alone is not implementation proof. Product/native acceptance remains `unassessed` for every new row. Scenario/configuration/protected/defect arrays are intentionally empty at this inventory increment.
