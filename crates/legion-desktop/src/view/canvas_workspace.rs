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
    // One card per path.
    //
    // Nothing upstream promises the excerpt sections are distinct by file, and
    // two sections for one path would stack two cards in the same slot: they
    // fight for the same default position, and every lookup by path -- including
    // the one that resolves a dropped connection -- silently picks whichever the
    // iteration reached first.
    let mut seen: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
    sections
        .iter()
        .enumerate()
        .filter_map(|(section_index, section)| {
            let path = section.file_path.clone()?;
            if !seen.insert(path.0.clone()) {
                return None;
            }
            let saved = positions.get(path.0.as_str()).copied();
            // The default slot comes from the section's own index, not from a
            // running count of unplaced cards. With a counter, moving one card
            // stopped incrementing it and every later unplaced card shifted a
            // slot to the left on the next frame -- so dragging the first of
            // three made the other two jump, possibly onto the card just moved.
            // A person's arrangement must not rearrange itself around them.
            let position = saved.unwrap_or_else(|| {
                egui::pos2(
                    (section_index % DEFAULT_COLUMNS) as f32 * DEFAULT_STRIDE,
                    (section_index / DEFAULT_COLUMNS) as f32 * DEFAULT_STRIDE,
                )
            });
            let available = section.lines.len();
            let lines: Vec<String> = section
                .lines
                .iter()
                .take(MAX_NODE_LINES)
                .map(|line| line.visible_text.clone())
                .collect();
            Some(CanvasNode {
                path,
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

/// Read the saved scene viewport, or the default view.
fn scene_rect(ctx: &egui::Context) -> egui::Rect {
    ctx.data_mut(|data| data.get_temp::<egui::Rect>(egui::Id::new(SCENE_RECT_ID)))
        .unwrap_or_else(|| {
            egui::Rect::from_min_size(egui::pos2(-40.0, -40.0), egui::vec2(1200.0, 800.0))
        })
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

    let mut rect = scene_rect(ui.ctx());
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

            for node in &nodes {
                render_node(ui, node, actions);
            }

            render_ports(ui, &nodes, &by_path, actions);
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

    // `drag_delta` is already divided by the layer's scaling inside a `Scene`,
    // so this is world units and needs no zoom correction of its own.
    let delta = header.drag_delta();
    if delta != egui::Vec2::ZERO {
        actions.push(DesktopAction::MoveCanvasNode {
            path: node.path.clone(),
            x: crate::bridge::WorldCoord::new(node.position.x + delta.x),
            y: crate::bridge::WorldCoord::new(node.position.y + delta.y),
            // Mid-drag: update the arrangement, do not write it to disk. This
            // fires on every pointer-movement frame, and persisting each one
            // rewrote, validated, `sync_all`ed and atomically replaced the
            // session file from the renderer thread -- dozens of filesystem
            // flushes during one drag, on the thread that has to keep drawing it.
            settled: false,
        });
    }
    if header.drag_stopped() {
        // The drag ended: this is the position worth keeping.
        actions.push(DesktopAction::MoveCanvasNode {
            path: node.path.clone(),
            x: crate::bridge::WorldCoord::new(node.position.x),
            y: crate::bridge::WorldCoord::new(node.position.y),
            settled: true,
        });
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

/// The connection ports, and the drag between them that makes an edge.
///
/// Drawn after every card so a port is never buried under a neighbouring node's
/// body, and interacted with after every card so the port wins the hit test over
/// the header it sits on.
fn render_ports(
    ui: &mut egui::Ui,
    nodes: &[CanvasNode],
    by_path: &BTreeMap<&str, &CanvasNode>,
    actions: &mut Vec<DesktopAction>,
) {
    let tokens = theme::tokens();
    let ctx = ui.ctx().clone();

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
    }

    // A drag that ended: connect if it ended over some node's input port.
    if ctx.input(|i| i.pointer.any_released()) {
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
                        actions.push(DesktopAction::ConnectCanvasNodes {
                            from_path: CanonicalPath(from.clone()),
                            to_path: node.path.clone(),
                        });
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

    /// A card with a saved position does not shift the cards after it.
    ///
    /// Default slots used to come from a running count of *unplaced* cards, so
    /// one card gaining a position stopped the counter and moved every later
    /// card a slot to the left -- possibly onto the one just placed.
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
