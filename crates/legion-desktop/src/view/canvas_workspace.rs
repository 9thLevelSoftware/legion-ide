//! The canvas workspace: files as cards in an infinite 2D space.
//!
//! A different bet from the tab strip. A tab bar shows you one file and the
//! names of the others; a canvas shows you several at once, where you put them,
//! with the relationships between them drawn. The claim is that spatial memory
//! and visible structure beat a list of names for holding a system in your head.
//!
//! ## What this module is, and is not
//!
//! It is the arrangement surface: pan, zoom, nodes you can drag, and edges you
//! draw yourself. It is **not** an architecture visualiser. An edge here means
//! "I say these are related" — a person's claim, saved as theirs.
//!
//! Derived edges (imports, calls) are deliberately absent rather than faked.
//! Nothing in the tree can currently produce them: no workspace-wide index
//! retains import targets, `callHierarchy` has request builders and response
//! projectors in `legion-lsp` with no callers outside their own contract tests,
//! and the one real dependency graph lives in `xtask`, which product crates are
//! forbidden to depend on. Drawing a guessed edge would make the surface's
//! central claim — that it shows you real structure — false on its first frame.
//! Keeping the person's edges in their own type means derived ones can join them
//! later without reinterpreting what someone drew by hand.
//!
//! ## Why the text is excerpt text
//!
//! Nodes render from `ExcerptSurfaceProjection`, which carries per-buffer text
//! for *every* open tab and is populated unconditionally on every snapshot.
//! `CodeCanvasPainter` — the full-fidelity painter, with semantic colouring and
//! decorations — takes no `buffer_id` and binds to `active_buffer_projection`
//! throughout, so it can paint exactly one buffer. Calling it N times a frame is
//! separately blocked by fixed egui ids (one `id_salt` for the code scroll area,
//! a global hover-position id) that would collide.
//!
//! So node text is real, and plainer than the active editor's: no syntax
//! colouring, no folds, no decorations. That is a stated limit of this
//! increment, not a placeholder — the alternative was inventing a second
//! rendering path for code, which is how two renderers drift.

use std::collections::BTreeMap;

use legion_protocol::CanonicalPath;
use legion_ui::ShellProjectionSnapshot;

use super::theme;
use crate::bridge::DesktopAction;

/// Node width in world units.
const NODE_WIDTH: f32 = 320.0;

/// Height of a node's draggable header.
const HEADER_HEIGHT: f32 = 28.0;

/// Line height for excerpt text inside a node.
const LINE_HEIGHT: f32 = 14.0;

/// Most lines drawn in one node.
///
/// The excerpt viewport is requested at a fixed size upstream, so this is a
/// display bound rather than a fetch bound: it keeps a long file from making one
/// node taller than the whole scene, and it is stated on the card when it bites.
const MAX_NODE_LINES: usize = 18;

/// Radius of the connection port on each side of a node.
const PORT_RADIUS: f32 = 6.0;

/// How far apart the default grid places nodes that have never been positioned.
const DEFAULT_STRIDE: f32 = 380.0;

/// Nodes per row in that default grid.
const DEFAULT_COLUMNS: usize = 3;

/// Zoom limits.
///
/// `Scene`'s own default `zoom_range` maxes at 1.0, which would make zooming in
/// past 100% silently do nothing, so it is always set explicitly.
const ZOOM_MIN: f32 = 0.25;
/// See [`ZOOM_MIN`].
const ZOOM_MAX: f32 = 2.5;

/// One card on the canvas.
pub(crate) struct CanvasNode {
    /// Canonical path, which is also the node's identity across sessions.
    pub path: CanonicalPath,
    /// Whether this card's position came from saved layout rather than a default.
    pub placed: bool,
    /// Buffer behind this node, when the projection named one.
    pub buffer_id: Option<legion_protocol::BufferId>,
    /// Display title.
    pub title: String,
    /// Whether the buffer has unsaved edits.
    pub dirty: bool,
    /// Excerpt lines to draw.
    pub lines: Vec<String>,
    /// Whether `lines` is shorter than the excerpt actually held.
    pub lines_truncated: bool,
    /// Where the card sits in world space.
    pub position: egui::Pos2,
}

/// The world-space corner of a numbered grid slot.
fn slot_position(slot: usize) -> egui::Pos2 {
    egui::pos2(
        (slot % DEFAULT_COLUMNS) as f32 * DEFAULT_STRIDE,
        (slot / DEFAULT_COLUMNS) as f32 * DEFAULT_STRIDE,
    )
}

/// The first grid slot at or after `from` that no *drawn* card is sitting on.
///
/// A slot is taken when a saved position falls inside its cell -- strictly
/// within one stride on both axes -- because a card placed there would be drawn
/// over the top of one already on screen, and the one underneath is unreachable
/// without moving the new one off it first.
///
/// A card that is not on screen blocks nothing: a closed file's position is
/// kept so reopening restores it, and treating it as occupied would push every
/// new card past a row of slots nothing is drawn in.
///
/// The search is bounded: each drawn card can block at most four cells, so a
/// free one always exists within `4 * rendered.len() + 1` of the start, and the
/// bound is a guard rather than a limit anybody can reach.
fn first_free_slot(from: usize, taken: &[egui::Pos2]) -> usize {
    let limit = from
        .saturating_add(taken.len().saturating_mul(4))
        .saturating_add(1);
    (from..=limit)
        .find(|slot| !overlaps(slot_position(*slot), taken))
        .unwrap_or(limit)
}

/// Whether a card at `candidate` would be drawn over one already placed.
///
/// Grid cells are one stride square, so a card strictly inside another's cell
/// on both axes covers it.
fn overlaps(candidate: egui::Pos2, taken: &[egui::Pos2]) -> bool {
    taken.iter().any(|position| {
        (position.x - candidate.x).abs() < DEFAULT_STRIDE
            && (position.y - candidate.y).abs() < DEFAULT_STRIDE
    })
}

/// The nodes a snapshot implies, positioned from saved layout where it exists.
///
/// A file with no saved position is laid out on a grid rather than at the
/// origin, because stacking every new node at one point looks like a single card
/// and hides the rest.
pub(crate) fn nodes_for_snapshot(
    snapshot: &ShellProjectionSnapshot,
    positions: &BTreeMap<String, egui::Pos2>,
) -> Vec<CanvasNode> {
    nodes_for_sections(&snapshot.excerpt_surface_projection.sections, positions)
}

/// The cards a set of excerpt sections implies.
///
/// Split from the snapshot so the rules below can be tested against inputs the
/// live projection does not currently produce -- notably two sections naming one
/// file, which nothing upstream promises against and which no fixture can be
/// made to emit.
pub(crate) fn nodes_for_sections(
    sections: &[legion_ui::ui::ExcerptSurfaceSectionProjection],
    positions: &BTreeMap<String, egui::Pos2>,
) -> Vec<CanvasNode> {
    // Slots for cards that have never been placed are numbered after the cards
    // that have. Section index alone is not stable: the projection builds these
    // in `open_tabs` order, so closing or reordering a tab renumbers every card
    // after it and they all jump.
    //
    // The renderer persists each default the first time it draws it (see
    // `render_canvas_workspace`), so this numbering only has to be free of
    // collisions within a single frame -- by the next one, every card has a
    // saved position and none of them can move again.
    // Where to start looking, then step over whatever the saved cards cover.
    //
    // The count has to stay: it is what keeps an unplaced card still when some
    // *other* card is moved. Starting the search at zero instead let every
    // unplaced card slide left into the vacancy a moved card left behind, so
    // dragging one card rearranged the ones nobody had touched -- the defect
    // this counter was introduced to fix.
    //
    // The count alone is not an answer either, because it assumes a placed card
    // still sits in the slot it started in, which moving it is exactly what
    // stops being true. Move the only card onto slot 1 and the count still says
    // 1, so the next file opened is handed slot 1 as well and lands on top of
    // it. Count first, then skip what is actually occupied.
    // Counted over the cards being drawn, not over every position ever saved.
    //
    // Closing a file keeps its position -- deliberately, so reopening it puts
    // the card back where it was. But counting those made history occupy the
    // leading slots forever: with six closed files the next file opened started
    // at slot 6, `y = 760`, outside the initial viewport, and the canvas looked
    // empty while the file was open. Restoration and placement need different
    // views of the same map.
    // One card per path.
    //
    // Nothing upstream promises the excerpt sections are distinct by file, and
    // two sections for one path would stack two cards in the same slot: they
    // fight for the same default position, and every lookup by path -- including
    // the one that resolves a dropped connection -- silently picks whichever the
    // iteration reached first.
    let mut seen: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
    let drawn: Vec<&legion_ui::ui::ExcerptSurfaceSectionProjection> = sections
        .iter()
        .filter(|section| {
            section
                .file_path
                .as_ref()
                .is_some_and(|path| seen.insert(path.0.clone()))
        })
        .collect();

    // Pass one: saved positions claim their ground, first come first served.
    //
    // A saved position can collide now that closed files no longer reserve
    // their slots. Close a card in slot 0, open a new file into the vacancy,
    // reopen the first: both are saved at the same coordinates and one is drawn
    // underneath the other, unreachable without moving the one on top. Whoever
    // is later in tab order gives way, deterministically.
    let mut taken: Vec<egui::Pos2> = Vec::new();
    let mut resolved: Vec<Option<egui::Pos2>> = Vec::with_capacity(drawn.len());
    for section in &drawn {
        let saved = section
            .file_path
            .as_ref()
            .and_then(|path| positions.get(path.0.as_str()).copied());
        match saved {
            // Exactly equal, not merely overlapping. Dragging one card on top
            // of another is a thing a person may deliberately do, and a rule
            // that shuffled the card underneath would rearrange an arrangement
            // somebody had just made by hand. Two cards at the *same
            // coordinates* is the case nothing but a reused default slot
            // produces, and the case where one is perfectly hidden.
            Some(position) if !taken.contains(&position) => {
                taken.push(position);
                resolved.push(Some(position));
            }
            _ => resolved.push(None),
        }
    }

    // Pass two: everything still unplaced takes the first slot nothing covers.
    //
    // Numbering starts after the cards that kept a saved position, which is
    // what stops an unplaced card sliding left when some *other* card moves.
    let mut next_slot = taken.len();
    for slot in resolved.iter_mut() {
        if slot.is_some() {
            continue;
        }
        let free = first_free_slot(next_slot, &taken);
        next_slot = free + 1;
        let position = slot_position(free);
        taken.push(position);
        *slot = Some(position);
    }

    drawn
        .into_iter()
        .zip(resolved)
        .filter_map(|(section, position)| {
            let path = section.file_path.clone()?;
            let position = position?;
            // "Placed" means the position on screen is the one on record. A
            // card displaced out of a collision is not, so it is written down
            // like any other default -- otherwise the arrangement on disk would
            // keep describing two cards in one place.
            let placed = positions
                .get(path.0.as_str())
                .is_some_and(|saved| *saved == position);
            let available = section.lines.len();
            let lines: Vec<String> = section
                .lines
                .iter()
                .take(MAX_NODE_LINES)
                .map(|line| line.visible_text.clone())
                .collect();
            Some(CanvasNode {
                path,
                placed,
                buffer_id: section.buffer_id,
                title: section.title.clone(),
                dirty: section.dirty,
                lines_truncated: available > lines.len(),
                lines,
                position,
            })
        })
        .collect()
}

/// Height of a node, given how many lines it draws.
fn node_height(node: &CanvasNode) -> f32 {
    let body = (node.lines.len() as f32) * LINE_HEIGHT;
    let footer = if node.lines_truncated {
        LINE_HEIGHT
    } else {
        0.0
    };
    HEADER_HEIGHT + body + footer + 12.0
}

/// The rect a node occupies in world space.
fn node_rect(node: &CanvasNode) -> egui::Rect {
    egui::Rect::from_min_size(node.position, egui::vec2(NODE_WIDTH, node_height(node)))
}

/// Where an edge leaves a node, and where it arrives.
fn output_port(node: &CanvasNode) -> egui::Pos2 {
    let rect = node_rect(node);
    egui::pos2(rect.right(), rect.top() + HEADER_HEIGHT / 2.0)
}

/// See [`output_port`].
fn input_port(node: &CanvasNode) -> egui::Pos2 {
    let rect = node_rect(node);
    egui::pos2(rect.left(), rect.top() + HEADER_HEIGHT / 2.0)
}

/// Draw a connection as a horizontal bezier between two ports.
fn paint_edge(painter: &egui::Painter, from: egui::Pos2, to: egui::Pos2, color: egui::Color32) {
    // Control points pushed out horizontally by a fraction of the span, so a
    // short hop curves gently and a long one does not loop back on itself.
    let span = ((to.x - from.x).abs() * 0.5).clamp(24.0, 160.0);
    let curve = egui::epaint::CubicBezierShape::from_points_stroke(
        [
            from,
            egui::pos2(from.x + span, from.y),
            egui::pos2(to.x - span, to.y),
            to,
        ],
        false,
        egui::Color32::TRANSPARENT,
        egui::Stroke::new(1.5_f32, color),
    );
    painter.add(curve);
}

/// egui id under which the scene's world-space viewport is kept.
///
/// Pan and zoom are renderer state with no app meaning, so they live in egui's
/// temp store rather than in the projection. Named as a constant because the
/// tests read it: there is no other way to observe that a pan happened.
pub(crate) const SCENE_RECT_ID: &str = "legion-canvas-scene-rect";

/// egui id under which an in-progress connection is kept.
pub(crate) const PENDING_EDGE_ID: &str = "legion-canvas-pending-edge";

/// Read the saved scene viewport, or a first view that contains `nodes`.
///
/// Pan and zoom live in egui's temp store and do not survive a restart, while
/// card positions do -- so a fixed opening rectangle showed an empty canvas to
/// anybody who had arranged their cards outside it, which panning an infinite
/// surface makes ordinary. There is no minimap and no fit-to-content control to
/// recover with, so "empty" is indistinguishable from "broken".
///
/// The opening view is derived from the cards instead: their bounding box, with
/// a margin, and never smaller than the default so a single card does not open
/// zoomed to fill the screen.
fn scene_rect(ctx: &egui::Context, nodes: &[CanvasNode]) -> egui::Rect {
    if let Some(saved) =
        ctx.data_mut(|data| data.get_temp::<egui::Rect>(egui::Id::new(SCENE_RECT_ID)))
    {
        return saved;
    }
    default_scene_rect(nodes)
}

/// The view a canvas opens at when nothing has panned it yet.
pub(crate) fn default_scene_rect(nodes: &[CanvasNode]) -> egui::Rect {
    const MARGIN: f32 = 40.0;
    let fallback =
        egui::Rect::from_min_size(egui::pos2(-MARGIN, -MARGIN), egui::vec2(1200.0, 800.0));
    let mut bounds: Option<egui::Rect> = None;
    for node in nodes {
        let rect = node_rect(node);
        bounds = Some(match bounds {
            Some(current) => current.union(rect),
            None => rect,
        });
    }
    let Some(bounds) = bounds else {
        return fallback;
    };
    let bounds = bounds.expand(MARGIN);
    // Never smaller than the default. A lone card would otherwise open filling
    // the screen, which is a different kind of disorienting from an empty one.
    egui::Rect::from_min_size(
        bounds.min,
        egui::vec2(
            bounds.width().max(fallback.width()),
            bounds.height().max(fallback.height()),
        ),
    )
}

/// Draw the canvas, and return the rect it occupied.
///
/// The return value matters beyond this module: the shell stores the central
/// panel's rect as `last_editor_rect`, which several suites and the panel-tiling
/// gate assert against. A canvas that returned nothing would fail them for a
/// reason unrelated to the canvas.
pub(crate) fn render_canvas_workspace(
    ui: &mut egui::Ui,
    snapshot: &ShellProjectionSnapshot,
    positions: &BTreeMap<String, egui::Pos2>,
    edges: &[(String, String)],
    actions: &mut Vec<DesktopAction>,
) -> egui::Rect {
    let outer = ui.available_rect_before_wrap();
    let nodes = nodes_for_snapshot(snapshot, positions);

    if nodes.is_empty() {
        ui.label(theme::muted(
            "No files on the canvas. Open a file and it appears here as a card.",
        ));
        return outer;
    }

    let mut rect = scene_rect(ui.ctx(), &nodes);
    let ctx = ui.ctx().clone();

    egui::Scene::new()
        // Always explicit: `Scene`'s own default maxes at 1.0, so zooming in
        // past 100% would silently do nothing.
        .zoom_range(ZOOM_MIN..=ZOOM_MAX)
        .show(ui, &mut rect, |ui| {
            let by_path: BTreeMap<&str, &CanvasNode> = nodes
                .iter()
                .map(|node| (node.path.0.as_str(), node))
                .collect();

            // Edges first, so cards sit on top of their own connections rather
            // than being crossed by them.
            let painter = ui.painter();
            for (from, to) in edges {
                let (Some(from_node), Some(to_node)) =
                    (by_path.get(from.as_str()), by_path.get(to.as_str()))
                else {
                    // An edge naming a file that is no longer open is kept in
                    // state -- reopening the file should restore the
                    // connection -- but has nothing to draw between.
                    continue;
                };
                paint_edge(
                    painter,
                    output_port(from_node),
                    input_port(to_node),
                    theme::tokens().accent.orange,
                );
            }

            // A connection being dragged right now, from its port to the cursor.
            let pending: Option<String> =
                ctx.data_mut(|data| data.get_temp::<String>(egui::Id::new(PENDING_EDGE_ID)));
            if let Some(pending_path) = pending.as_deref()
                && let Some(source) = by_path.get(pending_path)
                && let Some(cursor) = pointer_in_scene(ui)
            {
                paint_edge(
                    ui.painter(),
                    output_port(source),
                    cursor,
                    theme::tokens().accent.cyan,
                );
            }

            // A default slot is written down the first time it is used, so the
            // card keeps it when the tab list changes underneath. Until it is
            // saved, its position depends on which slots are free, and that
            // changes as cards move.
            //
            // All of them in one action. A settled `MoveCanvasNode` each meant
            // the first canvas frame ran one validate, one `sync_all`, one
            // atomic replace and one projection rebuild per open file, on the
            // renderer thread, before anything appeared.
            let placements: Vec<crate::bridge::CanvasPlacement> = nodes
                .iter()
                .filter(|node| !node.placed)
                .map(|node| crate::bridge::CanvasPlacement {
                    path: node.path.clone(),
                    x: crate::bridge::WorldCoord::new(node.position.x),
                    y: crate::bridge::WorldCoord::new(node.position.y),
                })
                .collect();
            if !placements.is_empty() {
                actions.push(DesktopAction::PlaceCanvasNodes { placements });
            }

            for node in &nodes {
                render_node(ui, node, actions);
            }

            render_ports(ui, &nodes, &by_path, edges, actions);
        });

    ctx.data_mut(|data| data.insert_temp(egui::Id::new(SCENE_RECT_ID), rect));
    outer
}

/// The pointer, in scene coordinates.
///
/// `Context::pointer_interact_pos` answers in screen space — it is a context
/// query and knows nothing about which layer is asking. Everything inside the
/// scene (node rects, ports, edge endpoints) is in world space, so comparing the
/// two directly is a category error that silently never matches: connections
/// were dropped and the in-flight edge drew itself to the wrong place.
fn pointer_in_scene(ui: &egui::Ui) -> Option<egui::Pos2> {
    let pointer = ui.ctx().pointer_interact_pos()?;
    Some(
        ui.ctx()
            .layer_transform_from_global(ui.layer_id())
            .map_or(pointer, |from_global| from_global * pointer),
    )
}

/// A scene-space rect in screen space.
///
/// egui fills accesskit bounds from the widget rect in *ui* coordinates and
/// applies no layer transform (`Response::fill_accesskit_node_common`). Inside a
/// `Scene` the ui is the transformed sublayer, so every node would be published
/// at its world position — a card at world origin reports bounds near `(0, 0)`
/// however the scene is panned or zoomed.
///
/// That is wrong for anything reading the tree to point at something: an
/// assistive technology would put its focus rectangle over empty chrome, and a
/// test clicking a reported centre would miss the card entirely. Both were true
/// before this existed.
///
/// Read **outside** the `accesskit_node_builder` closure, deliberately. That
/// closure runs with the context locked, and asking the context for a layer
/// transform from inside it deadlocks — a ten-second debug panic in every test
/// at once, which is how this was found.
fn global_rect(ui: &egui::Ui, rect: egui::Rect) -> egui::Rect {
    ui.ctx()
        .layer_transform_to_global(ui.layer_id())
        .map_or(rect, |transform| transform * rect)
}

/// Publish a precomputed screen-space rect as a node's bounds.
fn set_bounds(builder: &mut egui::accesskit::Node, rect: egui::Rect) {
    builder.set_bounds(egui::accesskit::Rect {
        x0: rect.min.x.into(),
        y0: rect.min.y.into(),
        x1: rect.max.x.into(),
        y1: rect.max.y.into(),
    });
}

/// One card: header you can drag, and the file's text under it.
fn render_node(ui: &mut egui::Ui, node: &CanvasNode, actions: &mut Vec<DesktopAction>) {
    let rect = node_rect(node);
    let tokens = theme::tokens();
    let painter = ui.painter();

    painter.rect_filled(rect, 6.0, tokens.bg.card);
    painter.rect_stroke(
        rect,
        6.0,
        egui::Stroke::new(1.0_f32, tokens.border.subtle),
        egui::StrokeKind::Inside,
    );

    let header_rect = egui::Rect::from_min_size(rect.min, egui::vec2(rect.width(), HEADER_HEIGHT));
    painter.rect_filled(header_rect, 6.0, tokens.bg.toolbar);

    let header_id = ui.id().with(("canvas-node", node.path.0.as_str()));
    let header = ui.interact(header_rect, header_id, egui::Sense::click_and_drag());
    let header_bounds = global_rect(ui, header_rect);

    // The card follows the pointer, rather than accumulating deltas.
    //
    // `drag_delta` cannot answer on the frame that matters: egui 0.34.2 returns
    // `Vec2::ZERO` from it unless `dragged()` is true, and the frame a drag
    // stops is documented as one where the widget "will not be found in
    // `dragged`" (`interaction.rs`). So a release that carries movement -- the
    // end of any fast flick -- reported no delta at all, and a settled position
    // built from `node.position + delta` was the previous frame's position
    // however the arithmetic was arranged. That was the defect the delta was
    // added to fix, still present after adding it.
    //
    // `interact_pointer_pos` is populated on precisely that frame: the response
    // sets it when `drag_stopped()` holds, already mapped out of global space
    // into this layer, which inside a `Scene` is world space. Recording where in
    // the card the pointer took hold lets every frame -- including the last --
    // place the card from the pointer alone.
    let grab_id = header_id.with("grab-offset");
    // Captured when the button goes down, not when the drag starts.
    //
    // egui does not call it a drag until the pointer has moved: `drag_started`
    // is reported on the frame that carries the movement, by which point the
    // pointer is already somewhere else. Taking the offset there measured the
    // grab from the position the card was being dragged *to*, which cancels out
    // exactly -- the card computed its own position as its own position and sat
    // still for the whole gesture.
    //
    // `interact_pointer_pos` is answered a frame earlier than that, on the press
    // itself, because the button is down on this widget. That is the frame the
    // hand actually took hold.
    let stored: Option<egui::Vec2> = ui.ctx().data_mut(|data| data.get_temp(grab_id));
    let grab_offset = match (stored, header.interact_pointer_pos()) {
        (Some(offset), _) => Some(offset),
        (None, Some(pointer)) if header.is_pointer_button_down_on() => {
            let offset = pointer - node.position;
            ui.ctx().data_mut(|data| data.insert_temp(grab_id, offset));
            Some(offset)
        }
        _ => None,
    };
    let settled_position = match (grab_offset, header.interact_pointer_pos()) {
        (Some(offset), Some(pointer)) => pointer - offset,
        _ => node.position,
    };
    if header.dragged() && settled_position != node.position {
        actions.push(DesktopAction::MoveCanvasNode {
            path: node.path.clone(),
            x: crate::bridge::WorldCoord::new(settled_position.x),
            y: crate::bridge::WorldCoord::new(settled_position.y),
            // Mid-drag: update the arrangement, do not write it to disk. This
            // fires on every pointer-movement frame, and persisting each one
            // rewrote, validated, `sync_all`ed and atomically replaced the
            // session file from the renderer thread -- dozens of filesystem
            // flushes during one drag, on the thread that has to keep drawing it.
            settled: false,
        });
    }
    if header.drag_stopped() {
        // The drag ended: this is the position worth keeping, including the
        // movement that arrived on this same frame, which is why it comes from
        // the pointer rather than from a delta that is zero here by definition.
        actions.push(DesktopAction::MoveCanvasNode {
            path: node.path.clone(),
            x: crate::bridge::WorldCoord::new(settled_position.x),
            y: crate::bridge::WorldCoord::new(settled_position.y),
            settled: true,
        });
    }
    if !header.is_pointer_button_down_on() {
        ui.ctx().data_mut(|data| data.remove::<egui::Vec2>(grab_id));
    }
    if header.clicked()
        && let Some(buffer_id) = node.buffer_id
    {
        // Clicking a card focuses that buffer, so the canvas and the editor
        // agree about what is active rather than being two separate places a
        // file can be "open".
        actions.push(DesktopAction::SwitchTab { buffer_id });
    }

    let title = if node.dirty {
        format!("{} *", node.title)
    } else {
        node.title.clone()
    };
    // "Card alpha.rs", not "alpha.rs". The same file already appears in the
    // accessibility tree as an explorer row and as an editor tab, so a bare
    // filename here names three different things -- ambiguous to a screen
    // reader, and to anything else reading the tree by name.
    ui.ctx().accesskit_node_builder(header.id, |builder| {
        // Without a role these publish as plain text: "Card alpha.rs" reads like
        // a heading and "Connect from alpha.rs" like a section title, so nothing
        // tells a screen reader they can be pressed or dragged. A module that
        // went to the trouble of transforming bounds and hoisting body text
        // should not then ship half a tree.
        builder.set_role(egui::accesskit::Role::Button);
        builder.set_label(format!("Card {title}"));
        if node.dirty {
            builder.set_description("Unsaved changes");
        }
        set_bounds(builder, header_bounds);
    });

    let painter = ui.painter();
    painter.text(
        header_rect.left_center() + egui::vec2(8.0, 0.0),
        egui::Align2::LEFT_CENTER,
        &title,
        egui::FontId::proportional(12.0),
        tokens.text.primary,
    );

    let body_top = rect.top() + HEADER_HEIGHT + 6.0;
    // Clipped to the card. The excerpt viewport upstream is requested at 800
    // units against a 320-unit card, so a line wider than the card is ordinary
    // rather than exotic -- and the scene-wide painter drew the whole of it,
    // straight across whatever cards and connections lay to the right.
    let body_clip = egui::Rect::from_min_max(
        egui::pos2(rect.left(), body_top),
        egui::pos2(rect.right(), rect.bottom()),
    );
    let painter = painter.with_clip_rect(painter.clip_rect().intersect(body_clip));
    let mut y = body_top;
    for line in &node.lines {
        painter.text(
            egui::pos2(rect.left() + 8.0, y),
            egui::Align2::LEFT_TOP,
            line,
            egui::FontId::monospace(11.0),
            tokens.text.secondary,
        );
        y += LINE_HEIGHT;
    }

    // Painted glyphs are invisible to the accessibility tree, so a card's code
    // would be on screen and nowhere a screen reader -- or any tool reading the
    // tree -- could find it. The body gets a node carrying the same text.
    let body_rect = egui::Rect::from_min_max(
        egui::pos2(rect.left(), body_top),
        egui::pos2(rect.right(), y.max(body_top)),
    );
    if !node.lines.is_empty() {
        let body_id = ui.id().with(("canvas-body", node.path.0.as_str()));
        let body = ui.interact(body_rect, body_id, egui::Sense::hover());
        let body_bounds = global_rect(ui, body_rect);
        let text = node.lines.join("\n");
        ui.ctx().accesskit_node_builder(body.id, |builder| {
            // Not a button: this is a region of text with a name and a value.
            builder.set_role(egui::accesskit::Role::Label);
            builder.set_label(format!("{} contents", node.title));
            builder.set_value(text.clone());
            set_bounds(builder, body_bounds);
        });
    }

    let painter = ui.painter();
    if node.lines_truncated {
        painter.text(
            egui::pos2(rect.left() + 8.0, y),
            egui::Align2::LEFT_TOP,
            format!("first {MAX_NODE_LINES} lines"),
            egui::FontId::proportional(10.0),
            tokens.text.muted,
        );
    }
}

/// The action drawing `from` to `to` implies, given the edges that already exist.
///
/// Repeating a connection removes it. `DisconnectCanvasNodes` existed with no
/// gesture that could emit it, so an edge drawn by accident was permanent --
/// the state had an undo and the surface did not.
pub(crate) fn edge_action(
    from: &str,
    to: &CanonicalPath,
    existing_edges: &[(String, String)],
) -> DesktopAction {
    let already = existing_edges
        .iter()
        .any(|(edge_from, edge_to)| edge_from == from && edge_to == &to.0);
    let from_path = CanonicalPath(from.to_string());
    let to_path = to.clone();
    if already {
        DesktopAction::DisconnectCanvasNodes { from_path, to_path }
    } else {
        DesktopAction::ConnectCanvasNodes { from_path, to_path }
    }
}

/// The connection ports, and the gestures between them that make an edge.
///
/// Drawn after every card so a port is never buried under a neighbouring node's
/// body, and interacted with after every card so the port wins the hit test over
/// the header it sits on.
fn render_ports(
    ui: &mut egui::Ui,
    nodes: &[CanvasNode],
    by_path: &BTreeMap<&str, &CanvasNode>,
    existing_edges: &[(String, String)],
    actions: &mut Vec<DesktopAction>,
) {
    let tokens = theme::tokens();
    let ctx = ui.ctx().clone();
    // Which port, if any, was activated this frame rather than dragged.
    let mut activated_source: Option<String> = None;
    let mut activated_target: Option<CanonicalPath> = None;

    for node in nodes {
        let out_pos = output_port(node);
        let out_rect = egui::Rect::from_center_size(out_pos, egui::Vec2::splat(PORT_RADIUS * 2.0));
        let out_id = ui.id().with(("canvas-out", node.path.0.as_str()));
        let out = ui.interact(out_rect, out_id, egui::Sense::click_and_drag());
        let out_bounds = global_rect(ui, out_rect);
        ui.painter()
            .circle_filled(out_pos, PORT_RADIUS, tokens.accent.orange);
        ui.ctx().accesskit_node_builder(out.id, |builder| {
            builder.set_role(egui::accesskit::Role::Button);
            builder.set_label(format!("Connect from {}", node.title));
            set_bounds(builder, out_bounds);
        });

        if out.drag_started() {
            ctx.data_mut(|data| {
                data.insert_temp(egui::Id::new(PENDING_EDGE_ID), node.path.0.clone())
            });
        }
        if out.clicked() {
            activated_source = Some(node.path.0.clone());
        }

        let in_pos = input_port(node);
        let in_rect = egui::Rect::from_center_size(in_pos, egui::Vec2::splat(PORT_RADIUS * 2.0));
        let in_id = ui.id().with(("canvas-in", node.path.0.as_str()));
        let input = ui.interact(in_rect, in_id, egui::Sense::click_and_drag());
        let in_bounds = global_rect(ui, in_rect);
        ui.painter().circle_stroke(
            in_pos,
            PORT_RADIUS,
            egui::Stroke::new(1.5_f32, tokens.accent.cyan),
        );
        ui.ctx().accesskit_node_builder(input.id, |builder| {
            builder.set_role(egui::accesskit::Role::Button);
            builder.set_label(format!("Connect to {}", node.title));
            set_bounds(builder, in_bounds);
        });
        if input.clicked() {
            activated_target = Some(node.path.clone());
        }
    }

    // Activation, not dragging alone.
    //
    // Both ports publish as `Button`, and a button is answered with Space, Enter
    // or an AccessKit click -- all of which set `clicked()` and none of which set
    // `drag_started()`. Every connection gesture ran through drag start and
    // pointer release, so the two controls a screen reader can find and press
    // did nothing whatsoever when pressed. Publishing a control that cannot be
    // operated is worse than not publishing it: it is a promise the surface does
    // not keep.
    //
    // The activation flow is the same edge in two steps -- choose the source,
    // then choose the target -- and the rubber band already drawn to the cursor
    // shows that the first step took. Both steps go through `edge_action`, so an
    // edge made by keyboard toggles exactly like one made by pointer.
    let mut consumed_activation = false;
    if let Some(to) = activated_target {
        let pending: Option<String> =
            ctx.data_mut(|data| data.get_temp::<String>(egui::Id::new(PENDING_EDGE_ID)));
        if let Some(from) = pending
            && from != to.0
            && by_path.contains_key(from.as_str())
        {
            actions.push(edge_action(&from, &to, existing_edges));
            ctx.data_mut(|data| data.remove::<String>(egui::Id::new(PENDING_EDGE_ID)));
            consumed_activation = true;
        }
    }
    if !consumed_activation && let Some(from) = activated_source {
        ctx.data_mut(|data| data.insert_temp(egui::Id::new(PENDING_EDGE_ID), from));
        consumed_activation = true;
    }

    // A drag that ended: connect if it ended over some node's input port.
    //
    // Skipped on a frame an activation answered. A click *is* a pointer release,
    // so this block would otherwise run in the same frame that a click on an
    // output port armed the source and clear it again before anyone could reach
    // the second step -- leaving the keyboard flow looking like it did nothing,
    // which is the defect being fixed.
    if !consumed_activation && ctx.input(|i| i.pointer.any_released()) {
        let pending: Option<String> =
            ctx.data_mut(|data| data.get_temp::<String>(egui::Id::new(PENDING_EDGE_ID)));
        if let Some(from) = pending {
            if let Some(cursor) = pointer_in_scene(ui) {
                for node in nodes {
                    let target = input_port(node);
                    // A little forgiving: the port is small and the pointer is
                    // being released, which is when a hand is least precise.
                    if (target - cursor).length() <= PORT_RADIUS * 3.0
                        && node.path.0 != from
                        && by_path.contains_key(from.as_str())
                    {
                        actions.push(edge_action(&from, &node.path, existing_edges));
                        break;
                    }
                }
            }
            ctx.data_mut(|data| data.remove::<String>(egui::Id::new(PENDING_EDGE_ID)));
        }
    }
}

#[cfg(test)]
mod canvas_layout_rules {
    use super::{DEFAULT_STRIDE, nodes_for_sections};
    use legion_protocol::CanonicalPath;
    use legion_ui::ui::ExcerptSurfaceSectionProjection;
    use std::collections::BTreeMap;

    fn section(path: &str) -> ExcerptSurfaceSectionProjection {
        ExcerptSurfaceSectionProjection {
            excerpt_id: format!("excerpt:{path}"),
            workspace_id: None,
            buffer_id: None,
            file_id: None,
            file_path: Some(CanonicalPath(path.to_string())),
            title: path.to_string(),
            dirty: false,
            editable: true,
            snapshot_id: None,
            cursor: None,
            lines: Vec::new(),
        }
    }

    /// Two sections naming one file produce one card.
    ///
    /// Nothing upstream promises the sections are distinct by path. Two cards
    /// for one file stack in the same slot and every lookup by path -- including
    /// the one resolving a dropped connection -- silently picks whichever the
    /// iteration reached first.
    #[test]
    fn one_file_is_one_card_even_if_the_projection_repeats_it() {
        let sections = vec![section("a.rs"), section("a.rs"), section("b.rs")];
        let nodes = nodes_for_sections(&sections, &BTreeMap::new());
        assert_eq!(nodes.len(), 2, "a repeated path produced a duplicate card");
        assert_eq!(nodes[0].path.0, "a.rs");
        assert_eq!(nodes[1].path.0, "b.rs");
    }

    /// A card is marked unplaced exactly when its slot is a default.
    ///
    /// The renderer persists an unplaced card's slot the first time it draws it,
    /// which is what makes the slot stable when the tab list later changes. That
    /// only works if `placed` tells the truth, so it is asserted directly rather
    /// than inferred from a position.
    #[test]
    fn a_default_slot_is_reported_as_unplaced() {
        let sections = vec![section("a.rs"), section("b.rs")];
        let mut positions = BTreeMap::new();
        positions.insert("a.rs".to_string(), egui::pos2(10.0, 20.0));

        let nodes = nodes_for_sections(&sections, &positions);
        assert!(nodes[0].placed, "a.rs has a saved position");
        assert!(
            !nodes[1].placed,
            "b.rs is on a default slot and must say so"
        );
    }

    /// Closing an earlier tab must not renumber the cards after it.
    ///
    /// Slots used to come from the section index, and the projection builds
    /// sections in `open_tabs` order — so closing a tab shifted every later card
    /// one slot left. Numbering from the count of *placed* cards instead means a
    /// card that has been written down keeps its slot, and the renderer writes
    /// each default down on first sight.
    #[test]
    fn removing_a_tab_does_not_move_the_cards_that_remain() {
        let all = vec![section("a.rs"), section("b.rs"), section("c.rs")];
        let first_pass = nodes_for_sections(&all, &BTreeMap::new());

        // What the renderer persists on that first frame.
        let saved: BTreeMap<String, egui::Pos2> = first_pass
            .iter()
            .map(|node| (node.path.0.clone(), node.position))
            .collect();

        // The middle tab closes.
        let remaining = vec![section("a.rs"), section("c.rs")];
        let second_pass = nodes_for_sections(&remaining, &saved);

        assert_eq!(
            second_pass[0].position, saved["a.rs"],
            "a.rs moved when b.rs was closed"
        );
        assert_eq!(
            second_pass[1].position, saved["c.rs"],
            "c.rs moved when b.rs was closed — the defect this numbering fixes"
        );
    }

    /// A card with a saved position does not shift the cards after it.
    ///
    /// Default slots used to come from a running count of *unplaced* cards, so
    /// one card gaining a position stopped the counter and moved every later
    /// card a slot to the left -- possibly onto the one just placed.
    #[test]
    fn a_reopened_file_does_not_land_on_the_card_that_took_its_slot() {
        // The cost of not reserving slots for closed files. Close the card in
        // slot 0, open a new file into the vacancy, reopen the first: both are
        // saved at the same coordinates, and one is drawn underneath the other
        // where it cannot be reached without moving the one on top.
        let mut positions = BTreeMap::new();
        positions.insert("closed.rs".to_string(), egui::pos2(0.0, 0.0));
        positions.insert("fresh.rs".to_string(), egui::pos2(0.0, 0.0));

        let nodes = nodes_for_sections(&[section("fresh.rs"), section("closed.rs")], &positions);

        assert_eq!(nodes.len(), 2, "both files must be drawn");
        assert_ne!(
            nodes[0].position, nodes[1].position,
            "two saved cards were drawn at the same coordinates, so one of them is invisible \
             and unreachable"
        );
        // Whoever is later in tab order gives way, and the displacement is
        // recorded rather than recomputed every frame -- otherwise the saved
        // arrangement would go on describing two cards in one place.
        assert_eq!(
            nodes[0].position,
            egui::pos2(0.0, 0.0),
            "the first card in tab order keeps the position it was saved at"
        );
        assert!(
            !nodes[1].placed,
            "a displaced card must be written down at where it is actually drawn"
        );
    }

    #[test]
    fn positions_kept_for_closed_files_do_not_push_new_cards_off_screen() {
        // Closing a file keeps its position on purpose, so reopening puts the
        // card back where it was. Counting those made history occupy the
        // leading slots forever: six closed files put the next file opened at
        // slot 6 -- `y = 760`, outside the initial viewport -- and the canvas
        // looked empty while the file was open.
        let mut positions = BTreeMap::new();
        for index in 0..6 {
            positions.insert(
                format!("closed{index}.rs"),
                egui::pos2(
                    (index % 3) as f32 * DEFAULT_STRIDE,
                    (index / 3) as f32 * DEFAULT_STRIDE,
                ),
            );
        }

        let nodes = nodes_for_sections(&[section("fresh.rs")], &positions);

        let fresh = nodes
            .first()
            .expect("the newly opened file must have a card");
        assert_eq!(
            fresh.position,
            egui::pos2(0.0, 0.0),
            "nothing is on screen, so the first slot is free; a card placed past six \
             remembered-but-closed files is drawn where nobody is looking"
        );
    }

    #[test]
    fn the_opening_view_contains_every_saved_card() {
        // Card positions survive a restart; pan and zoom live in egui's temp
        // store and do not. A fixed opening rectangle therefore showed an empty
        // canvas to anybody who had arranged their cards outside it -- and with
        // no minimap and no fit-to-content control, "empty" cannot be told from
        // "broken".
        let mut positions = BTreeMap::new();
        positions.insert("far.rs".to_string(), egui::pos2(-4000.0, 2500.0));
        positions.insert("near.rs".to_string(), egui::pos2(-3600.0, 2500.0));

        let nodes = nodes_for_sections(&[section("far.rs"), section("near.rs")], &positions);
        let view = super::default_scene_rect(&nodes);

        for node in &nodes {
            assert!(
                view.contains_rect(super::node_rect(node)),
                "{} sits outside the opening view {view:?}, so the canvas opens empty with no \
                 way to find it",
                node.path.0
            );
        }
    }

    #[test]
    fn the_opening_view_of_a_lone_card_is_not_a_single_card() {
        // The other direction. Fitting exactly to one card opens zoomed to fill
        // the screen with it, which is a different kind of disorienting.
        let nodes = nodes_for_sections(&[section("only.rs")], &BTreeMap::new());
        let view = super::default_scene_rect(&nodes);

        assert!(
            view.width() >= 1200.0 && view.height() >= 800.0,
            "a single card opened a view of {view:?}, tighter than the default"
        );
        assert!(
            view.contains_rect(super::node_rect(&nodes[0])),
            "the card must still be inside the view it widened to"
        );
    }

    #[test]
    fn a_new_card_never_lands_on_one_already_placed() {
        // Numbering the next slot by how many cards are saved assumes a placed
        // card still sits in the slot it started in -- which moving it is
        // precisely what stops being true. Move the only card to slot 1 and the
        // count still says 1, so the next file opened is handed slot 1 as well
        // and lands on top of it. The card underneath cannot be reached without
        // first moving the one covering it.
        let mut positions = BTreeMap::new();
        positions.insert("alpha.rs".to_string(), egui::pos2(DEFAULT_STRIDE, 0.0));

        let nodes = nodes_for_sections(&[section("alpha.rs"), section("beta.rs")], &positions);

        let alpha = nodes
            .iter()
            .find(|node| node.path.0 == "alpha.rs")
            .expect("the saved card must still be drawn");
        let beta = nodes
            .iter()
            .find(|node| node.path.0 == "beta.rs")
            .expect("the new card must be drawn");
        assert_ne!(
            alpha.position, beta.position,
            "a newly opened file was placed exactly on top of a card already there"
        );
        assert_eq!(
            beta.position,
            egui::pos2(2.0 * DEFAULT_STRIDE, 0.0),
            "one card is saved, so the search starts at slot 1 -- and slot 1 is where that \
             card was moved to, so the new card belongs on the next slot after it"
        );
    }

    #[test]
    fn a_new_card_skips_every_slot_a_saved_card_covers() {
        // Two saved cards sitting on slots 0 and 1: the next card belongs on 2,
        // and the count agrees only by coincidence here. What it must not do is
        // stop at the first free-looking number without checking the second.
        let mut positions = BTreeMap::new();
        positions.insert("alpha.rs".to_string(), egui::pos2(0.0, 0.0));
        positions.insert("beta.rs".to_string(), egui::pos2(DEFAULT_STRIDE, 0.0));

        let nodes = nodes_for_sections(
            &[section("alpha.rs"), section("beta.rs"), section("gamma.rs")],
            &positions,
        );

        let gamma = nodes
            .iter()
            .find(|node| node.path.0 == "gamma.rs")
            .expect("the new card must be drawn");
        for placed in [egui::pos2(0.0, 0.0), egui::pos2(DEFAULT_STRIDE, 0.0)] {
            assert_ne!(
                gamma.position, placed,
                "the new card was placed on a slot a saved card is sitting on"
            );
        }
    }

    #[test]
    fn a_saved_position_does_not_reshuffle_the_unplaced_cards() {
        let sections = vec![section("a.rs"), section("b.rs"), section("c.rs")];

        let untouched = nodes_for_sections(&sections, &BTreeMap::new());
        let b_before = untouched[1].position;
        let c_before = untouched[2].position;

        let mut positions = BTreeMap::new();
        positions.insert("a.rs".to_string(), egui::pos2(-900.0, 900.0));
        let after = nodes_for_sections(&sections, &positions);

        assert_eq!(
            after[1].position, b_before,
            "placing a.rs moved b.rs, which nobody touched"
        );
        assert_eq!(
            after[2].position, c_before,
            "placing a.rs moved c.rs, which nobody touched"
        );
        assert_eq!(
            after[0].position,
            egui::pos2(-900.0, 900.0),
            "the saved position must win over the default slot"
        );
        assert!(
            (b_before.x - DEFAULT_STRIDE).abs() < f32::EPSILON,
            "the fixture assumes b.rs starts in the second column"
        );
    }
}
