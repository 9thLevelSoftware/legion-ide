# ADR-0051: The Canvas Workspace as a Centre Surface

## Status

Accepted — 2026-08-21 for backlog card `P6.F5.T1`.

This ADR authorizes one thing: a spatial arrangement surface that can occupy the
centre region, alongside the editor. It does not authorize the wider canvas
direction recorded in `docs/ui/canvas-workspace-direction.md`, which remains
direction and not commitment.

## Context

### Why an ADR exists for a renderer change

`plans/dependency-policy.md` §4 requires an accepted ADR, a dependency-policy
entry, an active phase gate, contract tests and ownership tests before runtime
behaviour for a planned surface lands. Retiring ADR-0046 (the surface expansion
freeze) removed a moratorium; it did not remove this rule, and the two were
briefly conflated. The canvas was built and made reachable from the activity
rail without either artifact. This ADR and the policy entry supply them.

The rule is worth honouring here rather than waiving, because the canvas is
exactly the shape of thing it exists for: a new centre surface, reachable by
default, that draws file contents and accepts keyboard and pointer input.

### What the canvas is

An `egui::Scene` occupying the centre region, showing one card per open file.
A card carries the file's real text from `ExcerptSurfaceProjection`, a header
that can be dragged to move it, and two ports that can be connected to record a
relationship a person asserts between two files. Positions and connections
persist in `WorkspaceSessionRecord`.

### What it deliberately is not

- Not a second place a file can be open. A card is a view of a buffer the app
  already owns; clicking one focuses that buffer rather than creating anything.
- Not an editor. Cards are read-only. Editor keys do not reach a buffer while
  the canvas is showing — neither text input nor the keymap's editor-scoped
  entries — because an edit to a file that is not on screen is invisible until
  it is saved.
- Not a source of derived architecture. Every edge on the canvas is a claim a
  person made by drawing it. Nothing infers edges from imports, call graphs or
  the index, and no edge should be read as if something did.

## Decision

1. `CenterSurface` gains a `Canvas` variant, reachable from the activity rail.
2. The canvas is a renderer surface in `legion-desktop`. No new crate, no new
   dependency edge, no change to the crate graph.
3. Card positions and person-drawn connections are **adapter-local view state**,
   in the same category as explorer expansion. The app decides which buffers are
   open; the person decides where the cards sit. They are keyed by canonical
   path rather than `BufferId` so an arrangement survives a restart that
   renumbers buffers, and they travel to the runtime as `DesktopAction`s
   (`MoveCanvasNode`, `PlaceCanvasNodes`, `ConnectCanvasNodes`,
   `DisconnectCanvasNodes`) rather than being mutated in the renderer.
4. The editor remains the only surface that can mutate a buffer.

## Consequences

### What this permits

Building on the arrangement surface within `legion-desktop`, `legion-ui` and
`legion-app` — additional card kinds, richer edges, layout aids — under the
existing roadmap and readiness process, without a further ADR for each.

### What it still does not permit

- Derived edges from any index, LSP or Cargo metadata. Those need their own
  decision about provenance, staleness and what a wrong edge costs.
- Editing in a card.
- Any new crate or dependency edge.
- Any claim of product readiness. `PR-UI-001` is Manual mode's evidence; the
  canvas is not part of it and has no ledger row.

### Ownership and mutation rules, and how they are held

The rules above are asserted by tests rather than by this document:

| Rule | Test |
| --- | --- |
| Typing on the canvas cannot reach a buffer | `typing_on_the_canvas_never_reaches_the_open_buffer` |
| Editor keymap entries cannot reach a buffer | `undo_on_the_canvas_never_rewrites_the_buffer_behind_it` |
| Moving a card records an arrangement, not just pixels | `dragging_a_card_moves_it_and_the_runtime_records_where` |
| An arrangement outlives the process | `the_arrangement_survives_a_restart` |
| A file is one card, however the projection reports it | `a_file_gets_exactly_one_card` |
| Cards and ports are published as operable controls | `cards_and_ports_are_published_as_controls`, `connection_ports_answer_activation_and_not_only_dragging` |

All in `crates/legion-desktop/tests/canvas_workspace.rs`, driven through the
rendered UI and asserted against the runtime rather than the renderer.

### Risk accepted

The canvas draws N buffers' text per frame where the editor draws one, and
ADR-0048's budgets (keypress p50 <16 ms / p95 <32 ms, scroll p95 <32 ms) apply
to it as they do to every surface. The current slice caps the lines drawn per
card and clips each card's text to its own rectangle, which bounds the work but
does not measure it. No performance evidence is claimed here, and none should be
read into the surface being reachable.
