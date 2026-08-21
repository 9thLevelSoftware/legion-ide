# Canvas workspace — UI direction

Date: 2026-08-17, updated 2026-08-20
Status: **Arrangement surface built and reachable. The rest of this document is
still direction, not commitment.** No ADR, no backlog task, no product-readiness
ledger row.
Source: Claude Design project `Application UI review and redesign`
(`b0839227-4e52-4aee-b1a2-8a56a825ff57`), file `Legion Canvas Workspace.dc.html`.

This document exists so a direction explored in a design tool survives outside
that tool. It records the concept, a spec precise enough to rebuild the layout
without the mock, and — the part worth more than the spec — what is actually
true in this codebase today that would have to change first.

## What is built, as of 2026-08-20

The owner directed a pivot to this direction, so section 3's "would have to be
true first" is no longer hypothetical for the first slice. What exists in
`crates/legion-desktop/src/view/canvas_workspace.rs`, reachable from a `Canvas`
control on the activity rail:

- Pan and zoom through `egui::Scene`, with `zoom_range` set explicitly because
  `Scene`'s own default caps at 1.0 and zooming in would otherwise do nothing.
- One card per open file, carrying that file's real text from
  `ExcerptSurfaceProjection`.
- Drag a card's header to move it. `Response::drag_delta` already divides by the
  layer's scaling inside a `Scene`, so — contrary to section 2's note, taken from
  the mock's JavaScript — no manual division by zoom is needed.
- Ports on each card, and a drag between them draws a connection.
- Positions and connections persist in `WorkspaceSessionRecord`, keyed by
  canonical path so an arrangement survives the restart that renumbers buffers.

**Edges are the person's, not the code's.** An edge here means "I say these are
related". Derived edges — imports, calls — remain absent for exactly the reason
section 3 gives: nothing in the tree can produce them yet. They are kept in a
separate type so derived ones can be added later without reinterpreting what
someone drew by hand.

Not built, and still as section 3 describes: syntax colouring inside cards,
terminal/test/proposal node kinds, group frames, the minimap, edge labels, and
every derived relationship.

Two things found while building it, both fixed here and neither specific to the
canvas:

- egui fills accessibility bounds from the widget rect in *ui* coordinates and
  applies no layer transform, so every node inside a `Scene` publishes its world
  position as its screen bounds. Any assistive technology would point at empty
  chrome. The canvas sets its own bounds through
  `Context::layer_transform_to_global`.
- `Context::pointer_interact_pos` answers in screen space regardless of the
  asking layer, so comparing it to scene-space geometry silently never matches.

It follows the posture of [`four-mode-prototype-fidelity.md`](four-mode-prototype-fidelity.md):
a visual direction retained as reference, explicit about which parts are product
truth and which are illustration. Every code fact below was verified against the
tree at the date above and is cited; the mock's data (a `legion-billing` crate,
its files, its tests) is illustration and does not exist.

---

## 1. The concept

Replace the tabbed editor with an **infinite pan/zoom 2D canvas**. Files are
cards placed in space and kept there. Edges between them show real relationships
— imports, trait boundaries, which file a pending proposal touches. Related
cards sit inside labelled group frames.

The bet: for holding a system in your head, spatial memory plus visible
structure beats a strip of tabs. In the owner's words — *"align files in a
workflow style and visualize architecture instead of making just another IDE
burying you in tabs and menus."*

The canvas is not only for files. The mock places terminals, test results, and
approval cards as nodes on the same surface, which makes the working set of a
task — the code, the test that proves it, the command that runs it, the
proposal awaiting review — one arrangement you build and return to, rather than
four panels you re-find.

---

## 2. Spec, as mocked

Enough detail to rebuild without opening the design project.

### Shell chrome

| Region | Height | Contents |
| --- | --- | --- |
| Top bar | 44px | Product mark, repo name, branch, centred four-way mode segmented control (Manual / Assist / Delegate / Workflows), `⌘K` keycap, contextual Pause action, window controls |
| Directive strip | 46px | Only in Delegate and Workflows. Accent edge, `DIRECTIVE`/`TASK` eyebrow, the directive sentence, a blinking `EXECUTING 64%` chip, task count, progress bar, "Add constraint"/"Steer task", destructive `Stop` |
| Activity rail | 46px | Five surfaces: Files, Search, Source control, **Canvas**, Debug. Active item carries a 2px accent left edge |
| Canvas | fill | See below |
| Status bar | 28px | State label with status dot, node/edge count, approvals pending, test summary, provider, branch, zoom percent |

Note the rail: in the mock **Canvas is one of five surfaces, not a replacement
for all of them.** The spatial view is where you work; Files and Search remain
as ways to find something and put it on the canvas.

### The canvas surface

- Dot-grid background. Grid cell size scales with zoom; grid offset tracks pan,
  so the grid appears fixed to the world rather than the screen.
- A single world transform: `translate(pan) scale(zoom)`, origin top-left.
- Zoom clamped to 0.5–1.6. Zoom about the viewport centre, not the origin.
- Drag the background to pan. Drag a node's **header** to move that node; the
  pointer delta is divided by zoom so the node tracks the cursor.
- Node positions are world coordinates and persist.

### Node kinds

All nodes share a frame: header strip (status dot, mono title, right-aligned
state tag), body, optional footer.

- **File node** — body is code: line numbers, syntax colouring, `+`/`−` diff
  signs, added lines tinted green with a left edge. Footer shows `+42 −7` and
  the owning proposal id. The node under active agent work gets a 2px accent top
  border and a blinking status dot.
- **Test node** — one row per test: status icon, test name, duration. A running
  test blinks.
- **Terminal node** — shell output with a blinking cursor block.
- **Proposal card** — `NEEDS YOUR APPROVAL` eyebrow, proposal id, title,
  `owner · risk · file count`, then `Approve` (primary) / `Review diff`
  (secondary) / `Reject` (destructive, right-aligned).
- **Assist suggestion card** — the Assist-mode occupant of the same slot: the
  suggested code with an accent left edge, a one-line justification
  ("Non-breaking · fits create_checkout · local context only"), `Accept` /
  `Dismiss`, and a `⇥ TAB` keycap.

### Edges and groups

- Bezier curves, horizontal and vertical variants, drawn behind nodes and
  recomputed from live node positions.
- Solid for imports; dashed for weaker relations; accent-dashed for the link
  from a file to its pending proposal card.
- An edge may carry a label chip on its midpoint — the mock shows
  `TRAIT BOUNDARY`.
- Groups are labelled rects behind node clusters: `BILLING CORE · 2 FILES`,
  `PROVIDERS · 1 FILE`, `VALIDATION · 1 FILE`. Purely visual; the label sits
  above the frame's top-left corner.

### Screen-space overlays

These do **not** pan or zoom:

- Top-left: graph scope chip — `◈ GRAPH · crates/legion-billing` plus a hint
  line ("arranged by module graph · drag canvas to pan · drag headers to move").
- Top-right: mode chip plus a one-sentence explanation of what the mode does to
  your authority ("Multi-agent plan executing — every merge gates on you.").
- Bottom-centre: toolbar — select/pan tool toggle, `+ File`, `+ Terminal`,
  zoom `−` / percent / `+`. The percent is a click target that resets the view.
- Bottom-right: **navigator minimap** — a scaled rect per node plus a viewport
  rectangle, at a fixed 1:9 scale in the mock.

### Palette — a conflict to resolve later

The mock uses a **neutral near-black** ramp: surfaces `#09090a` → `#0e0e0f` →
`#131315` → `#18181b`, borders `#232326`/`#2e2e33`/`#3a3a40`, text
`#ececec`/`#a3a3a8`/`#6e6e75`/`#4a4a50`, accent amber `#d0824b`, semantic green
`#5e9e77`, red `#c0554d`, blue `#8fb6d9`, cyan `#9fc4c9`.

The shipped theme is **blue-slate** (`crates/legion-desktop/src/theme.rs`):
canvas `#0b1219`, panel `#121e29`, raised `#1b2a38`, accent cyan `#55a8d7`,
amber `#cf8136`. The companion `Component Spec` in the same design project
assumes the blue-slate ramp and says explicitly to keep it.

These are two different products visually. Recorded, not resolved.

---

## 3. What would have to be true first

Verified against the tree. This section is the reason the document is worth
keeping — the mock is easy to redraw, these facts are not.

### The insertion point is small and singular

`ProjectionView::render_with_state` (`../../crates/legion-desktop/src/view.rs`)
is the entire shell; there is no separate `render_shell`. The central panel body
is three lines and contains the **only** call site of `render_code_canvas`.
Swapping a canvas in is a closure-body change at that one point.

Two cautions. `render_code_canvas` returns an `egui::Rect` stored as
`last_editor_rect`, which is load-bearing for several test suites
(`keyboard_nav`, `accessibility`, `projection_rendering`) and for
`layout_region_coverage`, which asserts panel tiling. And there is **no existing
notion of switching what the centre shows** — `center_surface_label` returns a
hard-coded `"editor"` and ignores its argument. `ActivitySurface` exists but
only drives the left rail, and is not persisted.

### Multi-buffer text already exists, but constrained

This was the expected blocker and it is not one. `ExcerptSurfaceProjection`
carries per-buffer text for **every open tab**, populated unconditionally on
every snapshot in `../../crates/legion-app/src/lib.rs` — each section has
`buffer_id`, `file_path`, `dirty`, `snapshot_id`, `cursor`, and `lines` with
`visible_text`.

The constraints, precisely:

- The viewport is requested at a **hard-coded 800×384 px** — roughly 24 lines.
  A node showing more than that needs the dimensions to become a parameter.
- It carries **no semantic token overlays**, so excerpt text cannot be syntax
  coloured the way the active buffer is. The mock's file nodes are all
  syntax-coloured.
- Its scroll is whatever the app holds for that buffer. `DesktopAction::SetViewportScroll`
  already takes a `buffer_id`, so per-node scroll steering has a path.
- It is currently consumed only by `render_excerpt_surface`, which lists titles
  and line counts and never renders the line text.

So: a first canvas can render every open file with real text today. Full-fidelity
code — semantic colouring, folds, decorations — remains active-buffer-only.

### The code painter cannot paint a non-active buffer

`CodeCanvasPainter::paint_lines(ui, snapshot, model, actions)`
(`../../crates/legion-desktop/src/view/code_canvas_painter.rs`) takes **no
`buffer_id`**, and everything below it binds to
`snapshot.active_buffer_projection`. It does take a caller-supplied `Ui`, so
painting into an arbitrary rect via `ui.new_child(UiBuilder::new().max_rect(…))`
works — that is already how the code/minimap split is done.

Calling it N times per frame is blocked by fixed egui ids, all in `view.rs`:
a single `id_salt("legion_desktop_code_canvas_scroll")`, a galley-cache id that
ignores its `buffer_id` argument (the cache *key* is per-buffer, so correctness
holds but N buffers thrash one bounded LRU), and a global
`Id::new("lsp_last_hover_pos")`. It also emits git-hunk nav buttons and
breadcrumbs inline, so N calls produce N sets of those.

### egui 0.34.2 ships `Scene` — and nothing here uses it

`egui::Scene` (`containers/scene.rs`) does exactly this job: pan, zoom, a
`TSTransform` sublayer, and a public `register_pan_and_zoom` if a custom
container is wanted. A repo-wide search for `Scene`, `TSTransform`,
`set_transform_layer`, `with_clip_rect` or `set_clip_rect` returns **zero hits** —
the only zoom in the product is `ctx.set_zoom_factor`, which scales the whole
app.

Two sharp edges: `Scene`'s default `zoom_range` maxes at **1.0**, so zooming in
past 100% requires calling `.zoom_range()` explicitly; and `show` allocates all
remaining space in the parent `Ui`, so nothing drawn after it in the same
closure gets room. The caller must also own the `scene_rect: Rect` across frames.

### Node positions are adapter-local state, and persistence has a gap

Positions are renderer state, not app state — the same category as
`explorer_expansion` in `../../crates/legion-desktop/src/workflow.rs`. The
pattern to copy is `DockLayout`: a runtime field, a per-frame copy into
`DesktopProjectionViewState`, a DTO in `WorkspaceSessionRecord`, and a converter.
`SessionDockLayout` already round-trips an `f32` splitter fraction, so float
geometry through the session record is established.

`DesktopSessionStore` (`../../crates/legion-desktop/src/session.rs`) writes
crash-safely — temp file, `sync_all`, read-back validation, atomic replace — and
enforces a **metadata-only** policy. Node positions are metadata and fit; buffer
text does not.

**The gap:** `session_state` is set only by the `--session-state <path>` CLI flag
and has no default, so a normal `legion-desktop` launch persists nothing across
restarts. A canvas whose arrangement is its whole value cannot ship on that. This
would have to be fixed first, and it is worth fixing regardless.

Two related notes. The only geometry the repo persists today is normalised
fractions — `SessionLayoutSplit.ratio` and `SessionDockLayout`'s splitter
fraction; there is no x/y, per-node size, or pan/zoom state anywhere. And
`legion-storage` contains a complete, **entirely unused** `DockLayoutRepository`
(save/load/delete dock side layouts, including a splitter fraction) whose
`save_dock_side_layout` has zero callers outside its own definitions — worth
knowing before adding a parallel store.

### Nothing like this exists yet

There is no free-positioned draggable node anywhere in the codebase and **no
edge or link rendering of any kind**. The nearest prior art is the fleet board
and delegate task board, both column flow layouts, and three drag
implementations — tab reorder, minimap scrub, text selection — none of which
move a thing to an arbitrary point. Every `line_segment` call in the renderer is
a separator, a minimap bar, or an icon stroke.

Nor is there a graph to draw. `legion-agent`'s `WorkflowDag` is, despite the
name, a linear chain — its edges are `nodes.windows(2)` with the label `"next"`.
The git "commit graph" stores `parent_count` as an integer rather than parent
ids, so no history DAG can be reconstructed from it.

### Where edges and groups would come from — the hard part

This is the largest gap by a wide margin, and it is not a rendering problem.

**No cross-file edge exists anywhere in running code.** The pieces look present
and are not connected:

- The lexical indexer detects import lines (`use`, `mod`, `extern crate`,
  `import`, `from`, `require(`) but stores each as a **content hash of the line**
  — the module path text is discarded, and the resulting graph record carries
  `target: None`. An import cannot be resolved to a target file from the index
  as built.
- Call and reference targets are resolved against a symbol table built from the
  **current document only**, so every edge's target `file_id` equals its source.
  The consequence is visible downstream: `repository_map_file_rank` guards on
  `source_file_id != target_file_id`, that guard never passes, the adjacency map
  is always empty, and its damped PageRank degenerates to a uniform score.
- There is **no workspace-wide index**. The single `SemanticIndex` in the running
  app is fed one active buffer at a time on a language read; `IndexingActor` has
  no callers outside `legion-index`'s own tests.
- Tree-sitter extracts **definitions only**, from the stock Rust `TAGS_QUERY` —
  `reference_ranges` is hard-coded empty. Twelve grammars are bundled but only
  Rust has a real structural worker; the rest fall back to a lexical parser.
- There is no trait-boundary data. `SemanticGraphRecordKind::TypeRelation` is
  emitted with `target: None`, and the Rust tags query yields
  `definition.interface` / `definition.class` labels but no impl→trait linkage.
  The mock's `TRAIT BOUNDARY` edge has no source today.
- There is **no grouping or cluster concept** on any node type — nothing carries
  a `group_id` or module membership, so `BILLING CORE · 2 FILES` has nothing to
  derive from.

**What genuinely works and could carry a first version:**

- LSP `references` and `definition` are wired end-to-end and **do** return real
  cross-file `(path, range)` pairs (capped at 250 and 100 results). They land in
  a single-buffer projection that is overwritten on every read rather than
  accumulated, but the data crossing files is real.
- **`callHierarchy` is the single largest ready-to-use asset.** Request builders
  and response projectors for `prepareCallHierarchy`, `incomingCalls` and
  `outgoingCalls` all exist in `legion-lsp` and are contract-tested — with no
  callers outside those tests. There is no `LspReadKind` variant, no issue
  method, no ingest, no projection field. The protocol layer is done; the app
  plumbing is not.
- `workspace/symbol` is in the same state: builder present, no caller.
- The real inter-crate dependency graph of the workspace exists — in
  `xtask`, via `cargo metadata`, feeding `check-deps` against
  `plans/dependency-policy.md`. It is **not reachable from the app process**:
  `xtask` is tooling and product crates are forbidden to depend on it.
  `legion-project`'s own Cargo parsing is a hand-rolled string scrape that reads
  neither `[workspace] members` nor `[dependencies]`.

**The DTO shape already exists.** `SystemGraphProjection` in `legion-protocol`
defines `SystemGraphNode` / `SystemGraphEdge` with the repo's conventions —
`omitted_node_count`/`omitted_edge_count` bounding, redaction hints, schema
version. It is the wire type to follow. But its only producer is a hard-coded
four-node star (workspace *contains* active-file / proposal-ledger / delegated-task-manager),
its only consumer prints a count, and it has no positions, sizes, or grouping
field. `SemanticGraphRecord` / `SemanticGraphEndpoint` — which do carry a real
`file_id` — are the data model underneath.

**Order of work implied by all this:** a workspace-wide index that retains
import targets, then cross-file edge resolution, then call-hierarchy plumbing —
*then* a canvas that draws the result. The rendering is the small half.

### Performance

ADR-0048 budgets — keypress p50 <16ms, p95 <32ms, scroll p95 <32ms — apply to
the canvas too. N live code nodes plus edge recomputation plus a minimap is a new
per-frame cost class, and `perf-harness --strict` would need a canvas scenario
before the surface could be called ready.

---

## 4. Conflicts with accepted decisions

- [**ADR-0046**](../../plans/adrs/ADR-0046-surface-expansion-freeze.md) —
  **resolved.** The freeze forbade surface expansion before Manual mode was
  daily-drivable, and this is plainly surface expansion. The owner retired the
  ADR on 2026-08-21 rather than amend it per surface; see its Retirement
  section. Retiring it approved nothing else: the three gates it froze are still
  deferred, now on their own lack of evidence.
- [**ADR-0048**](../../plans/adrs/ADR-0048-renderer-strategy.md) — stay on egui,
  with `CodeCanvasPainter` as the escape hatch. Compatible in principle; the
  painter is the mechanism a canvas would use. The perf budgets bind.
- [**`mockups/design.md`**](../../mockups/design.md) §7.3 describes "Main Canvas"
  as a mode-switched central *region*, not a spatial map, and §18's component
  inventory has no node, edge, or viewport concepts. design.md would need
  amending, not extending.
- [**The production roadmap**](../../plans/legion-production-roadmap-v1.0.md) —
  Phase 1 is "the boring, excellent, zero-AI native IDE first", and the plan's
  own anti-scope rules argue against pulling a spatial workspace forward. On its
  dependencies this reads as Phase 6 or later.

---

## 5. The companion shell audit

The same design project produced `UX Audit & Plan.dc.html` (12 findings) and
`Component Spec.dc.html` (an egui control vocabulary). Those target the
**current** shell and are independent of the canvas — worth having either way.

Its core diagnosis matches what dogfooding found on 2026-08-17: *"everything
renders at the same visual weight… there is no affordance vocabulary, so the
user cannot tell what is readable, clickable, or configurable."*

| # | Finding | Sev | Status 2026-08-17 |
| --- | --- | --- | --- |
| F1 | No affordance vocabulary — labels, buttons and chips look identical | Critical | **Partly fixed.** Resting controls now carry a border, so a control reads as a control; the five-class vocabulary is not built |
| F2 | Right rail mixes six unrelated concerns in one scroll | Critical | Open |
| F3 | Internal projection strings leak into product UI | Critical | **Partly fixed.** Terminal panel humanised and the settings-echo strip above the editor removed; the workflow-activity and agent-stream strings the audit also cites are untouched |
| F4 | Duplicated content — "Assist" appears 4×, trust copy twice | High | Open |
| F5 | First-run onboarding is a wall of text, permanently parked | High | Open |
| F6 | Section headers are accent-coloured text that reads as links | High | Open |
| F7 | Empty states are literal placeholders (`<no active buffer>`) | High | Open |
| F8 | The mode switch — the hero control — is visually mute | High | Open |
| F9 | Confirmation modal is policy prose, not a decision | Medium | Open |
| F10 | Dead or ambiguous controls with no disabled-with-reason state | Medium | Open, and worse than reported — the status bar is entirely non-interactive and computes `flags`, `encoding`, `line_ending`, `language` and `connection` every frame while rendering none of them |
| F11 | No elevation model — panels, cards and inputs share near-identical fills | Medium | Open |
| F12 | Status bar is wasted; typography has no rhythm | Medium | Open |

The control vocabulary it proposes — primary / secondary / ghost / destructive /
disabled-with-reason buttons; selectable chip, status pill, add-context chip,
static keycap; a section-header pattern; and the elevation ladder
`code < surface < panel < card < hover < active` — maps entirely onto tokens
that already exist in `theme.rs`, and belongs beside the existing helpers in
`../../crates/legion-desktop/src/view/components.rs`. None of it depends on the
canvas.

**What the audit could not see.** It was screenshot-and-source based, so it found
no interaction defects. The same day's windowed session found eight, four of them
blocking — clicking a file in the explorer opened nothing, clicking a rail button
stopped typing permanently, the unsaved-changes prompt rendered below the window
edge. See
[the dogfood journal](../../plans/evidence/dogfood/2026-08-17-interactive-gui-journal.md).
A static audit and a driven session find disjoint sets of problems; neither
substitutes for the other.

---

## 6. Source material

Design project `b0839227-4e52-4aee-b1a2-8a56a825ff57`
(`Application UI review and redesign`), reachable through the Claude Design MCP:

| File | What it is |
| --- | --- |
| `Legion Canvas Workspace.dc.html` | The canvas mock — interactive, with pan/zoom/drag logic |
| `UX Audit & Plan.dc.html` | The 12-finding audit and its five-phase plan |
| `Component Spec.dc.html` | Five control classes and the elevation ladder, in `theme.rs` tokens |
| `Redesign - Legion Shell.dc.html` | A non-canvas redesign of the current shell |
| `Legion Command Center v2.dc.html`, `Reimagined - Legion Command Center.dc.html` | Earlier directions |
| `support.js` | The design tool's own React runtime. Nothing in it ports to Rust |
