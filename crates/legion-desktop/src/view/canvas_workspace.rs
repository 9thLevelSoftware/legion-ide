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
    /// Display title, as the tab bar shows it.
    pub title: String,
    /// The name the accessibility tree uses for this card and its controls.
    ///
    /// Usually the title, and something longer when the title is ambiguous:
    /// `tab_title` projects only the file name, so `src/index.ts` and
    /// `tests/index.ts` were both published as "Connect from index.ts". A
    /// screen-reader user could not tell which file a port belonged to, and
    /// connecting the wrong two cards was a matter of which one the tree
    /// happened to reach first.
    pub accessible_name: String,
    /// Whether the buffer has unsaved edits.
    pub dirty: bool,
    /// Excerpt lines to draw.
    pub lines: Vec<String>,
    /// Whether `lines` is shorter than the excerpt actually held.
    pub lines_truncated: bool,
    /// One-based first and last source line drawn, when the projection said.
    ///
    /// The excerpt is built from the buffer's saved scroll, so a file somebody
    /// has scrolled produces a section starting at line 100 rather than line 1.
    /// Discarding each line's `line_number` and calling the result "first 18
    /// lines" presented a mid-file excerpt as the opening of the file -- and
    /// where the scrolled tail was short enough not to be truncated, said
    /// nothing at all about the lines above it.
    pub line_span: Option<(u32, u32)>,
    /// Where the card sits in world space.
    pub position: egui::Pos2,
}

/// A saved card position, and whether a person chose it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SavedPosition {
    /// Where the card sits.
    pub position: egui::Pos2,
    /// Whether somebody dragged the card here.
    pub placed_by_person: bool,
}

/// Give every card a name the accessibility tree can tell apart.
///
/// Ambiguity is a property of the set, not of a card, so this runs once the
/// whole set is known. A title shared by two cards is qualified with enough of
/// the path to separate them; a title nothing else uses is left exactly as the
/// tab bar shows it, because a name that differs between two surfaces is its
/// own confusion.
fn disambiguate_names(mut nodes: Vec<CanvasNode>) -> Vec<CanvasNode> {
    let mut counts: std::collections::BTreeMap<&str, usize> = std::collections::BTreeMap::new();
    for node in &nodes {
        *counts.entry(node.title.as_str()).or_default() += 1;
    }
    let ambiguous: std::collections::BTreeSet<&str> = counts
        .into_iter()
        .filter(|(_, count)| *count > 1)
        .map(|(title, _)| title)
        .collect();
    // Names are decided before the borrow ends, then applied. The titles the
    // set borrows live in `nodes`, so nothing may be mutated while it is held.
    let names: Vec<String> = nodes
        .iter()
        .map(|node| {
            if ambiguous.contains(node.title.as_str()) {
                format!("{} ({})", node.title, node.path.0)
            } else {
                node.title.clone()
            }
        })
        .collect();
    drop(ambiguous);
    for (node, name) in nodes.iter_mut().zip(names) {
        node.accessible_name = name;
    }
    nodes
}

/// The world-space corner of a numbered grid slot, measured from `origin`.
///
/// The origin follows the viewport rather than being fixed at world zero.
/// Opening a file after panning away used to put its card near the origin --
/// off screen, on a surface with no minimap and no fit-to-content control, so
/// the canvas looked empty while the file was open. A new card belongs where
/// the person is looking.
fn slot_position_from(origin: egui::Pos2, slot: usize) -> egui::Pos2 {
    egui::pos2(
        origin.x + (slot % DEFAULT_COLUMNS) as f32 * DEFAULT_STRIDE,
        origin.y + (slot / DEFAULT_COLUMNS) as f32 * DEFAULT_STRIDE,
    )
}

/// The space a card needs, before its contents are known.
///
/// The tallest a card can be, since the slot is chosen before the excerpt is
/// read. Reserving the maximum means a slot that is accepted can hold whatever
/// lands in it; reserving less means accepting slots that cannot.
///
/// Every term `node_height` adds, including the truncation footer and the
/// padding. Counting only the header and the lines left a card that had been
/// truncated needing 306 units in a slot judged on 280 -- so a slot with room
/// between those two numbers was accepted, and the footer and the last of the
/// body were persisted off screen.
fn slot_card_rect(position: egui::Pos2) -> egui::Rect {
    egui::Rect::from_min_size(position, egui::vec2(NODE_WIDTH, MAX_NODE_HEIGHT))
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
fn first_free_slot(
    origin: egui::Pos2,
    visible: Option<egui::Rect>,
    from: usize,
    taken: &[(egui::Pos2, bool)],
) -> usize {
    let limit = from
        .saturating_add(taken.len().saturating_mul(4))
        .saturating_add(1);
    (from..=limit)
        .find(|slot| {
            let candidate = slot_position_from(origin, *slot);
            // A fresh card avoids everything already drawn, however it got
            // there: opening a file on top of one somebody arranged is nobody's
            // intent.
            if overlaps(candidate, taken) {
                return false;
            }
            // And it has to be somewhere the person can see, at full height.
            //
            // Moving the origin into the viewport fixed where the *grid* starts
            // and not where a slot lands: with six cards already placed the next
            // slot is `origin.y + 760`, and in an ordinary 800-unit view that
            // leaves the header just inside the bottom edge with the card's
            // contents below it. Measuring the header alone would call that
            // visible, which is how the card ends up saved, real, and unreadable.
            visible.is_none_or(|view| view.contains_rect(slot_card_rect(candidate)))
        })
        // Nothing free and visible: fall back to the first free slot anywhere.
        // A card off screen is bad; no card at all is worse, and the person can
        // pan to it.
        .or_else(|| (from..=limit).find(|slot| !overlaps(slot_position_from(origin, *slot), taken)))
        .unwrap_or(limit)
}

/// Whether a card at `candidate` would be drawn over one already placed.
///
/// Grid cells are one stride square, so a card strictly inside another's cell
/// on both axes covers it.
fn overlaps(candidate: egui::Pos2, taken: &[(egui::Pos2, bool)]) -> bool {
    taken
        .iter()
        .any(|(position, _)| covers(*position, candidate))
}

/// Whether a card at `candidate` would cover one the *layout* placed.
///
/// Person placements are ignored here on purpose. Dragging one card onto
/// another is something people do deliberately, and moving the card underneath
/// -- which nobody touched -- would rearrange an arrangement somebody had just
/// made by hand. The overlap that *is* worth repairing is two automatic
/// positions, and the reopen case that used to mix the two is prevented at
/// source: a deliberately chosen place is reserved even while its file is
/// closed, so nothing is ever handed it.
fn overlaps_automatic(candidate: egui::Pos2, taken: &[(egui::Pos2, bool)]) -> bool {
    taken
        .iter()
        .any(|(position, by_person)| !by_person && covers(*position, candidate))
}

/// Whether two cards at these positions are drawn over one another.
fn covers(one: egui::Pos2, other: egui::Pos2) -> bool {
    (one.x - other.x).abs() < DEFAULT_STRIDE && (one.y - other.y).abs() < DEFAULT_STRIDE
}

/// The nodes a snapshot implies, positioned from saved layout where it exists.
///
/// A file with no saved position is laid out on a grid rather than at the
/// origin, because stacking every new node at one point looks like a single card
/// and hides the rest.
pub(crate) fn nodes_for_snapshot(
    snapshot: &ShellProjectionSnapshot,
    positions: &BTreeMap<String, SavedPosition>,
    origin: egui::Pos2,
    visible: Option<egui::Rect>,
) -> Vec<CanvasNode> {
    nodes_for_sections(
        &snapshot.excerpt_surface_projection.sections,
        positions,
        origin,
        visible,
    )
}

/// The cards a set of excerpt sections implies.
///
/// Split from the snapshot so the rules below can be tested against inputs the
/// live projection does not currently produce -- notably two sections naming one
/// file, which nothing upstream promises against and which no fixture can be
/// made to emit.
pub(crate) fn nodes_for_sections(
    sections: &[legion_ui::ui::ExcerptSurfaceSectionProjection],
    positions: &BTreeMap<String, SavedPosition>,
    origin: egui::Pos2,
    visible: Option<egui::Rect>,
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
    // Each entry records whether a person put the card there, because only two
    // *automatic* positions count as a collision worth repairing.
    //
    // A person dragging one card onto another is their arrangement, and neither
    // card may be moved for it: not the one being dragged, and certainly not the
    // stationary one, which nobody touched. Two default slots landing on each
    // other is the case nobody chose and nobody can see coming -- a closed file
    // keeping its slot while a new file is handed the same one.
    let mut taken: Vec<(egui::Pos2, bool)> = Vec::new();
    let mut resolved: Vec<Option<egui::Pos2>> = vec![None; drawn.len()];

    // A place somebody chose stays theirs, even while the file is closed.
    //
    // This is where the reopen collision comes from, and closing it here beats
    // repairing it later. Releasing *every* closed card's slot let a new file
    // take a position somebody had deliberately put a card in; reopening that
    // file then put two cards in one cell, and whichever rule fires next has to
    // move one of them -- either the person's card, or a card nobody touched.
    //
    // Only deliberate positions are held. A default slot is the layout's guess
    // and is released the moment its card is closed, which is what stops six
    // closed files pushing the next one off the bottom of the screen.
    for saved in positions.values().filter(|saved| saved.placed_by_person) {
        taken.push((saved.position, true));
    }

    let saved_for = |section: &legion_ui::ui::ExcerptSurfaceSectionProjection| {
        section
            .file_path
            .as_ref()
            .and_then(|path| positions.get(path.0.as_str()).copied())
    };
    for (index, section) in drawn.iter().enumerate() {
        let saved = saved_for(section);
        match saved {
            // Already reserved above; the card simply takes it.
            Some(saved) if saved.placed_by_person => {
                resolved[index] = Some(saved.position);
            }
            // A position the layout chose is kept only while nothing is drawn
            // over it -- including the person placements reserved above.
            //
            // Geometry, not equality: a closed card nudged to (10, 10) and a
            // new one at (0, 0) are 320-unit cards covering each other almost
            // entirely, and an equality test sees no collision there.
            Some(saved) if !overlaps_automatic(saved.position, &taken) => {
                taken.push((saved.position, false));
                resolved[index] = Some(saved.position);
            }
            _ => {}
        }
    }

    // Pass two: everything still unplaced takes the first slot nothing covers.
    //
    // Numbering starts after the cards that kept a saved position, which is
    // what stops an unplaced card sliding left when some *other* card moves.
    let mut next_slot = resolved.iter().filter(|slot| slot.is_some()).count();
    for slot in resolved.iter_mut() {
        if slot.is_some() {
            continue;
        }
        let free = first_free_slot(origin, visible, next_slot, &taken);
        next_slot = free + 1;
        let position = slot_position_from(origin, free);
        taken.push((position, false));
        *slot = Some(position);
    }

    let nodes: Vec<CanvasNode> = drawn
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
                .is_some_and(|saved| saved.position == position);
            let available = section.lines.len();
            let shown = &section.lines[..section.lines.len().min(MAX_NODE_LINES)];
            let lines: Vec<String> = shown.iter().map(|line| line.visible_text.clone()).collect();
            // `line_number` is zero-based in the projection and one-based
            // everywhere a person reads it, including the status bar two panels
            // over.
            let line_span = match (shown.first(), shown.last()) {
                (Some(first), Some(last)) => Some((first.line_number + 1, last.line_number + 1)),
                _ => None,
            };
            Some(CanvasNode {
                path,
                placed,
                buffer_id: section.buffer_id,
                accessible_name: String::new(),
                title: section.title.clone(),
                dirty: section.dirty,
                lines_truncated: available > lines.len(),
                line_span,
                lines,
                position,
            })
        })
        .collect();
    disambiguate_names(nodes)
}

/// The tallest a card can be: header, every line, the footer, and padding.
///
/// Defined next to `node_height` so the two cannot drift. A fit rectangle that
/// understates this accepts slots a card does not fit in.
const MAX_NODE_HEIGHT: f32 =
    HEADER_HEIGHT + MAX_NODE_LINES as f32 * LINE_HEIGHT + LINE_HEIGHT + 12.0;

/// What the card says about the lines it is not showing, if anything.
///
/// Two different omissions, and the old text acknowledged neither correctly.
/// An excerpt built from a scrolled buffer starts partway down the file, and an
/// excerpt longer than the card is cut off at the bottom; either can happen
/// without the other. "first 18 lines" claimed the first line was line 1, which
/// is exactly the fact the projection had already measured and this code threw
/// away.
fn excerpt_footer(node: &CanvasNode) -> Option<String> {
    let (first, last) = node.line_span?;
    if first <= 1 && !node.lines_truncated {
        // The whole excerpt, starting where the file does. Nothing to disclose,
        // and a card that says so anyway trains people to ignore the line.
        return None;
    }
    Some(if node.lines_truncated {
        format!("lines {first}-{last} of a longer excerpt")
    } else {
        format!("lines {first}-{last}")
    })
}

/// Height of a node, given how many lines it draws.
fn node_height(node: &CanvasNode) -> f32 {
    let body = (node.lines.len() as f32) * LINE_HEIGHT;
    let footer = if excerpt_footer(node).is_some() {
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

/// The viewport a person last panned to, if they have panned at all.
fn saved_scene_rect(ctx: &egui::Context) -> Option<egui::Rect> {
    ctx.data_mut(|data| data.get_temp::<egui::Rect>(egui::Id::new(SCENE_RECT_ID)))
}

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
/// Shrink a view to something `Scene` can actually draw, keeping `anchor` in it.
///
/// `Scene` fits the rect it is handed by scaling, and the scale stops at
/// `ZOOM_MIN`. A rect wider than `panel / ZOOM_MIN` is therefore not honoured:
/// egui draws what the floor allows and the rest of the rectangle is off
/// screen. A view that promises to contain a card and then does not is worse
/// than one that never claimed to, because nothing on the canvas says which
/// direction the card went.
///
/// So a view that cannot be drawn is replaced by one that can: `preferred`
/// wide, positioned around `anchor` -- the card the caller most needs visible.
/// Everything else stays reachable by panning, which is what a canvas is for.
///
/// `preferred` rather than "as much as the floor allows", because the largest
/// drawable rectangle is the whole arrangement at 25%, where no card can be
/// read. The callers know better than this function does: an opening view wants
/// the default size, and a view being nudged to reach a new card wants to keep
/// whatever zoom the person was already working at.
fn within_zoom_floor(
    view: egui::Rect,
    anchor: egui::Rect,
    preferred: egui::Vec2,
    panel: egui::Vec2,
) -> egui::Rect {
    if panel.x <= 0.0 || panel.y <= 0.0 {
        return view;
    }
    let widest = panel / ZOOM_MIN;
    if view.width() <= widest.x && view.height() <= widest.y {
        return view;
    }
    let size = egui::vec2(preferred.x.min(widest.x), preferred.y.min(widest.y));
    // Centred on the anchor. Not clamped back inside `view`: `view` is the
    // rectangle that could not be drawn, and pinning a smaller window to its
    // edge is how the card ends up against the side of the screen rather than
    // in front of the person who just opened it.
    let min = anchor.center() - size / 2.0;
    let capped = egui::Rect::from_min_size(min, size);
    // A card larger than the drawable area cannot be contained by anything;
    // showing its top-left beats showing the space beside it.
    if capped.contains_rect(anchor) {
        capped
    } else {
        egui::Rect::from_min_size(anchor.min, size)
    }
}

/// Move `view` to reach cards placed this frame, keeping the zoom it has.
///
/// Preferring a visible slot is not always possible -- with the grid full
/// inside the current view, every free slot is outside it, and the search has
/// to put the card somewhere. Reaching it is what stops "somewhere" meaning
/// "nowhere you can see". Only newly placed cards count: chasing a card
/// somebody deliberately dragged far away would undo their pan every frame.
///
/// Moved rather than widened. Growing the rectangle is how `Scene` is asked to
/// zoom out, so opening one file just past a full grid shrank every card
/// already on screen -- and past `panel / ZOOM_MIN` the growth stops being
/// honoured at all: egui clamps the scale and shows a smaller window than the
/// rectangle it was given, leaving the new card outside the view widened for
/// it. The view keeps its size and travels the shortest distance that brings
/// the card inside, so the cards around it stay the size they were.
/// egui id under which a focused card asks the view to come to it.
///
/// Written inside the scene, where the focused card is drawn, and read outside
/// it, where the view for the next frame is decided. A card cannot move the
/// view it is being drawn into.
const FOCUS_REQUEST_ID: &str = "legion-canvas-focus-request";

/// Move `view` so that a card asking to be seen is inside it.
///
/// Focus can reach a card the view cannot: Tab walks every card in the
/// arrangement, and an arrangement larger than the zoom floor cannot be shown
/// at once however it is fitted. Without this, tabbing to an off-screen card
/// moved the keyboard somewhere invisible, and arrow keys then arranged a card
/// nobody could see.
fn view_following_focus(
    view: egui::Rect,
    focused: Option<egui::Rect>,
    panel: egui::Vec2,
) -> egui::Rect {
    let Some(focused) = focused else {
        return view;
    };
    if view.contains_rect(focused) {
        return view;
    }
    view_moved_to(view, focused, panel)
}

/// `view`, moved the shortest distance that puts `target` inside it.
fn view_moved_to(view: egui::Rect, target: egui::Rect, panel: egui::Vec2) -> egui::Rect {
    // A card too big for the view cannot be contained by moving; its top-left
    // is the part worth showing.
    if target.width() > view.width() || target.height() > view.height() {
        return within_zoom_floor(
            egui::Rect::from_min_size(target.min, view.size()),
            target,
            view.size(),
            panel,
        );
    }
    // Nothing on the axes that already reach, and just enough on the ones that
    // do not.
    let dx = (target.min.x - view.min.x).min(0.0) + (target.max.x - view.max.x).max(0.0);
    let dy = (target.min.y - view.min.y).min(0.0) + (target.max.y - view.max.y).max(0.0);
    within_zoom_floor(
        view.translate(egui::vec2(dx, dy)),
        target,
        view.size(),
        panel,
    )
}

fn view_reaching_new_cards(
    view: egui::Rect,
    nodes: &[CanvasNode],
    panel: egui::Vec2,
) -> egui::Rect {
    let Some(newest) = nodes
        .iter()
        .filter(|node| !node.placed)
        .map(|node| node_rect(node).expand(40.0))
        .next_back()
    else {
        return view;
    };
    if view.contains_rect(newest) {
        return view;
    }
    view_moved_to(view, newest, panel)
}

/// The widest view of `nodes` that `Scene` can actually draw.
///
/// What "fit all cards" means when the arrangement is larger than the zoom
/// floor allows. [`default_scene_rect`] deliberately falls back to a
/// *readable* view in that case, because an opening view zoomed to a quarter
/// scale is a wall of unreadable cards -- but somebody who presses a control
/// named for fitting everything is asking for the overview and should get as
/// much of it as exists.
///
/// When even the floor cannot hold the arrangement, this still cannot show it
/// all. That is why focus brings a card into view (see [`render_node`]): Tab
/// reaches every card whether or not any single view can contain them.
pub(crate) fn fit_scene_rect(nodes: &[CanvasNode], panel: egui::Vec2) -> egui::Rect {
    let fitted = default_scene_rect(nodes, panel);
    let bounds = nodes
        .iter()
        .map(|node| node_rect(node).expand(40.0))
        .reduce(|left, right| left.union(right));
    let (Some(bounds), true) = (bounds, panel.x > 0.0 && panel.y > 0.0) else {
        return fitted;
    };
    let widest = panel / ZOOM_MIN;
    if bounds.width() <= widest.x && bounds.height() <= widest.y {
        return bounds;
    }
    within_zoom_floor(bounds, bounds, widest, panel)
}

/// The view a canvas opens at when nothing has panned it yet.
pub(crate) fn default_scene_rect(nodes: &[CanvasNode], panel: egui::Vec2) -> egui::Rect {
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
    let fitted = egui::Rect::from_min_size(
        bounds.min,
        egui::vec2(
            bounds.width().max(fallback.width()),
            bounds.height().max(fallback.height()),
        ),
    );

    // And never wider than the zoom range can draw. Cards spread across ten
    // thousand units do not fit however carefully the rectangle is computed:
    // egui refuses to zoom out far enough and the canvas opens on empty space
    // between them. Showing the first card at a readable size is worth more
    // than a view that promises to contain everything and delivers nothing.
    let anchor = nodes.first().map_or(
        egui::Rect::from_min_size(fitted.min, fallback.size()),
        |node| node_rect(node).expand(MARGIN),
    );
    within_zoom_floor(fitted, anchor, fallback.size(), panel)
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
    positions: &BTreeMap<String, SavedPosition>,
    edges: &[(String, String)],
    actions: &mut Vec<DesktopAction>,
) -> egui::Rect {
    let outer = ui.available_rect_before_wrap();
    // Where a card with no saved position goes: the top-left of whatever is on
    // screen, not world zero. Read before the nodes are built, because the
    // saved viewport is what decides it.
    let saved_view = saved_scene_rect(ui.ctx());
    let origin = saved_view.map_or(egui::Pos2::ZERO, |view| view.min + egui::vec2(40.0, 40.0));
    let nodes = nodes_for_snapshot(snapshot, positions, origin, saved_view);

    if nodes.is_empty() {
        ui.label(theme::muted(
            "No files on the canvas. Open a file and it appears here as a card.",
        ));
        return outer;
    }

    let mut rect = saved_view.unwrap_or_else(|| default_scene_rect(&nodes, outer.size()));

    rect = view_reaching_new_cards(rect, &nodes, outer.size());
    // A card the keyboard reached last frame, if it is not already in view.
    // Taken rather than read: the view moves to it once, and panning away from
    // a card that still has focus must not be undone every frame.
    let focused = ui.ctx().data_mut(|data| {
        let id = egui::Id::new(FOCUS_REQUEST_ID);
        let requested: Option<egui::Rect> = data.get_temp(id);
        data.remove::<egui::Rect>(id);
        requested
    });
    rect = view_following_focus(rect, focused, outer.size());
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

    // The way back to a card that is no longer on screen.
    //
    // A canvas keeps the zoom somebody is working at, which means opening files
    // eventually puts one behind the edge of the view -- and on an infinite
    // surface a card off screen with nothing pointing at it is a card that is
    // gone. Panning finds it only if you know which direction to pan. This is
    // the fit-to-content control whose absence made "empty" and "broken" the
    // same picture; the arrangement is untouched, only the view moves.
    //
    // In its own foreground area rather than put into this `Ui`.
    //
    // `Scene` draws into a sublayer of its own that sits above anything placed
    // in the parent afterwards, so a button added here was published to the
    // accessibility tree, found by name, painted where it said it was -- and
    // never received the click, because the scene's own pan sense took every
    // press over the whole panel first. An overlay has to be an overlay.
    //
    // `outer` is what the shell stores as the central panel rect, so the
    // control is positioned rather than laid out: nothing about the region the
    // canvas reports may depend on whether this button exists.
    egui::Area::new(egui::Id::new("legion-canvas-fit"))
        .order(egui::Order::Foreground)
        .fixed_pos(outer.min + egui::vec2(12.0, 12.0))
        .show(&ctx, |ui| {
            if ui.button("Fit all cards").clicked() {
                let fitted = fit_scene_rect(&nodes, outer.size());
                ui.ctx()
                    .data_mut(|data| data.insert_temp(egui::Id::new(SCENE_RECT_ID), fitted));
            }
        });
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

/// How far one arrow-key press moves a focused card.
///
/// Small enough to place a card precisely, large enough that crossing the
/// canvas is not a career. Shift multiplies it by [`NUDGE_COARSE`].
const NUDGE_STEP: f32 = 24.0;

/// The multiplier Shift applies to [`NUDGE_STEP`].
const NUDGE_COARSE: f32 = 5.0;

/// Where a focused card is asked to go, if a key asked it to move at all.
///
/// Arranging cards was a pointer-only operation: every move came from
/// `dragged()` or `drag_stopped()`, so a canvas that publishes its cards and
/// ports as controls and invites a keyboard to reach them had nothing for that
/// keyboard to do once it arrived. Activation switched tabs and that was all.
fn nudged_position(ui: &egui::Ui, from: egui::Pos2) -> Option<egui::Pos2> {
    let (mut delta, coarse) = ui.input(|input| {
        let mut delta = egui::Vec2::ZERO;
        if input.key_pressed(egui::Key::ArrowLeft) {
            delta.x -= 1.0;
        }
        if input.key_pressed(egui::Key::ArrowRight) {
            delta.x += 1.0;
        }
        if input.key_pressed(egui::Key::ArrowUp) {
            delta.y -= 1.0;
        }
        if input.key_pressed(egui::Key::ArrowDown) {
            delta.y += 1.0;
        }
        (delta, input.modifiers.shift)
    });
    if delta == egui::Vec2::ZERO {
        return None;
    }
    delta *= NUDGE_STEP;
    if coarse {
        delta *= NUDGE_COARSE;
    }
    Some(from + delta)
}

/// Whether an arrow key finished being held this frame.
///
/// The end of a keyboard gesture, and the only frame of it worth persisting.
/// A held arrow repeats every frame; writing the arrangement on each repeat put
/// a validated, `sync_all`ed, atomically replaced session file between every
/// pair of frames, on the thread that has to keep drawing them.
fn nudge_settled(ui: &egui::Ui) -> bool {
    ui.input(|input| {
        [
            egui::Key::ArrowLeft,
            egui::Key::ArrowRight,
            egui::Key::ArrowUp,
            egui::Key::ArrowDown,
        ]
        .iter()
        .any(|key| input.key_released(*key))
    })
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

    // Which card the arrow keys will move.
    //
    // Tabbing to a card left it painted exactly like every other card, and
    // arrow keys then moved something the person could not identify. A
    // focusable control that looks unfocused is worse than an unfocusable one:
    // it invites the gesture and hides its target. Drawn around the whole card
    // rather than the header, because the whole card is what moves.
    if header.has_focus() {
        ui.painter().rect_stroke(
            rect.expand(2.0),
            8.0,
            egui::Stroke::new(2.0_f32, tokens.accent.orange),
            egui::StrokeKind::Outside,
        );
    }

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
    let unsaved_id = header_id.with("nudge-unsaved");
    // The same move, from a keyboard, with the same two halves a drag has.
    //
    // The arrow keys have to be taken deliberately: egui reads an unmodified
    // arrow as "move focus in that direction" (`memory/mod.rs`), so the first
    // nudge moved the card and then handed focus to whatever lay that way, and
    // the second arranged something else. A widget declares otherwise by
    // locking the filter, which is the mechanism egui provides for this and the
    // reason `TextEdit` can use arrows at all. Tab stays unlocked, because
    // leaving a card has to remain possible from the keyboard that arrived.
    //
    // The movement then streams and the *end of the gesture* persists. A held
    // arrow repeats every frame, and writing the arrangement on each repeat put
    // a validated, `sync_all`ed, atomically replaced session file between every
    // pair of frames -- on the thread drawing them. Releasing the key is the
    // release of the button.
    if header.has_focus() {
        // Where the keyboard is, in world space, for the next frame's view.
        ui.ctx()
            .data_mut(|data| data.insert_temp(egui::Id::new(FOCUS_REQUEST_ID), rect));
        ui.memory_mut(|memory| {
            memory.set_focus_lock_filter(
                header.id,
                egui::EventFilter {
                    tab: false,
                    horizontal_arrows: true,
                    vertical_arrows: true,
                    escape: false,
                },
            );
        });
        // One move per frame, settled or not, and never both.
        //
        // A tap short enough to press and release inside one frame produces
        // each -- and pushing them separately made the settled one land at
        // `node.position`, which is still the position *before* this frame's
        // nudge, because queued actions apply on the next frame. The card moved
        // and then a durable write put it back.
        let settled = nudge_settled(ui);
        let target = nudged_position(ui, node.position).or(settled.then_some(node.position));
        if let Some(target) = target {
            actions.push(DesktopAction::MoveCanvasNode {
                path: node.path.clone(),
                x: crate::bridge::WorldCoord::new(target.x),
                y: crate::bridge::WorldCoord::new(target.y),
                settled,
            });
            // Whether this card owes a durable write. Cleared by the write
            // itself, so the only state carried between frames is "a gesture
            // moved this card and nothing has saved it yet".
            ui.ctx()
                .data_mut(|data| data.insert_temp(unsaved_id, !settled));
        }
    } else if ui
        .ctx()
        .data_mut(|data| data.get_temp::<bool>(unsaved_id).unwrap_or(false))
    {
        // Focus left mid-gesture, so the release will never arrive here.
        //
        // Holding an arrow and then clicking something else takes focus away
        // while every move so far has been `settled: false` -- and the key-up
        // that would have persisted them lands on a card that no longer has the
        // keyboard. Closing the window then loses the arrangement. Losing focus
        // ends the gesture as surely as releasing the key does.
        actions.push(DesktopAction::MoveCanvasNode {
            path: node.path.clone(),
            x: crate::bridge::WorldCoord::new(node.position.x),
            y: crate::bridge::WorldCoord::new(node.position.y),
            settled: true,
        });
        ui.ctx()
            .data_mut(|data| data.insert_temp(unsaved_id, false));
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
        // The disambiguated name, like the ports and the body.
        //
        // The header was the one place still using the display title, so two
        // `index.ts` cards published "Card index.ts" twice -- on the control
        // that *selects* a card, which makes it the worst of the three to leave
        // ambiguous.
        builder.set_label(format!("Card {}", node.accessible_name));
        // A capability nobody is told about is a capability nobody uses, and
        // this one has no visible affordance at all: the card looks the same
        // focused as unfocused, and arranging it was a pointer gesture until
        // now.
        let arrangement = "Arrow keys move this card, Shift for larger steps";
        if node.dirty {
            builder.set_description(format!("Unsaved changes. {arrangement}"));
        } else {
            builder.set_description(arrangement);
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
            builder.set_label(format!("{} contents", node.accessible_name));
            builder.set_value(text.clone());
            set_bounds(builder, body_bounds);
        });
    }

    let painter = ui.painter();
    if let Some(footer) = excerpt_footer(node) {
        painter.text(
            egui::pos2(rect.left() + 8.0, y),
            egui::Align2::LEFT_TOP,
            &footer,
            egui::FontId::proportional(10.0),
            tokens.text.muted,
        );
        // Painted text is invisible to the accessibility tree, and this is the
        // only statement of which lines the card holds. A screen reader that
        // cannot reach it reads a mid-file excerpt as the whole file.
        let footer_rect = egui::Rect::from_min_size(
            egui::pos2(rect.left(), y),
            egui::vec2(NODE_WIDTH, LINE_HEIGHT),
        );
        let footer_id = ui.id().with(("canvas-footer", node.path.0.as_str()));
        let response = ui.interact(footer_rect, footer_id, egui::Sense::hover());
        let bounds = global_rect(ui, footer_rect);
        ui.ctx().accesskit_node_builder(response.id, |builder| {
            builder.set_role(egui::accesskit::Role::Label);
            builder.set_label(format!("{} shows {footer}", node.accessible_name));
            set_bounds(builder, bounds);
        });
    }
}

/// The titles a card is connected to, in one direction.
///
/// Titles rather than paths, because that is what the cards themselves are
/// named by, and a description naming something differently from the control
/// beside it is a description nobody can follow.
fn connection_titles(
    edges: &[(String, String)],
    by_path: &BTreeMap<&str, &CanvasNode>,
    path: &str,
    outgoing: bool,
) -> Vec<String> {
    edges
        .iter()
        .filter_map(|(from, to)| {
            let (own, other) = if outgoing { (from, to) } else { (to, from) };
            if own != path {
                return None;
            }
            Some(
                by_path
                    .get(other.as_str())
                    .map(|node| node.accessible_name.clone())
                    // An edge to a file that is no longer open is still real,
                    // and saying so is better than omitting it: the connection
                    // comes back when the file does.
                    .unwrap_or_else(|| format!("{other} (not open)")),
            )
        })
        .collect()
}

/// A description of a card's connections, or of their absence.
///
/// "No connections" rather than an empty description: silence is how a surface
/// that has nothing to say and one that is broken sound identical.
fn describe_connections(titles: &[String], verb: &str) -> String {
    if titles.is_empty() {
        "No connections".to_string()
    } else {
        format!("{verb} {}", titles.join(", "))
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

    // Escape gives up on a connection somebody started.
    //
    // Activating an output port arms a source and the only ways to clear it
    // were choosing a target or releasing a pointer -- neither of which a
    // keyboard user does. So a source armed and then thought better of stayed
    // armed across surface switches, and the next port activated, whenever that
    // happened, silently toggled an edge nobody was in the middle of drawing.
    //
    // Escape rather than a control, because that is what Escape means
    // everywhere else in this shell, and a half-drawn connection is exactly the
    // kind of thing it undoes.
    if ctx.input(|input| input.key_pressed(egui::Key::Escape)) {
        ctx.data_mut(|data| data.remove::<String>(egui::Id::new(PENDING_EDGE_ID)));
    }

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
        let outgoing = connection_titles(existing_edges, by_path, &node.path.0, true);
        ui.ctx().accesskit_node_builder(out.id, |builder| {
            builder.set_role(egui::accesskit::Role::Button);
            builder.set_label(format!("Connect from {}", node.accessible_name));
            // What this card is already connected to, on the control that
            // changes it. A painted curve says nothing to anyone reading the
            // tree, so activating a port was a step whose result could not be
            // checked -- and the gesture toggles, so "did that connect or
            // disconnect?" had no answer available.
            builder.set_description(describe_connections(&outgoing, "Connects to"));
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
        let incoming = connection_titles(existing_edges, by_path, &node.path.0, false);
        ui.ctx().accesskit_node_builder(input.id, |builder| {
            builder.set_role(egui::accesskit::Role::Button);
            builder.set_label(format!("Connect to {}", node.accessible_name));
            builder.set_description(describe_connections(&incoming, "Connected from"));
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

    /// A position somebody dragged a card to, which nothing may move.
    fn by_person(position: egui::Pos2) -> super::SavedPosition {
        super::SavedPosition {
            position,
            placed_by_person: true,
        }
    }

    /// A position the layout assigned, which collision repair may reassign.
    fn by_layout(position: egui::Pos2) -> super::SavedPosition {
        super::SavedPosition {
            position,
            placed_by_person: false,
        }
    }

    /// A section whose title is the file name, as `tab_title` projects it.
    ///
    /// The plain `section` helper titles a card with its whole path, which no
    /// projection does -- and a fixture that hands every card a distinct title
    /// cannot show two cards sharing one.
    fn section_titled_by_file_name(path: &str) -> ExcerptSurfaceSectionProjection {
        let mut section = section(path);
        section.title = path.rsplit('/').next().unwrap_or(path).to_string();
        section
    }

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

    /// A section whose lines start at `first_line` (zero-based), `count` long.
    fn scrolled_section(
        path: &str,
        first_line: u32,
        count: u32,
    ) -> ExcerptSurfaceSectionProjection {
        let mut projection = section(path);
        projection.lines = (0..count)
            .map(|offset| legion_ui::ui::ExcerptSurfaceLineProjection {
                line_number: first_line + offset,
                visible_text: format!("line {}", first_line + offset + 1),
                range: legion_protocol::Utf16Range {
                    start: legion_protocol::Utf16Position {
                        line: first_line + offset,
                        character: 0,
                    },
                    end: legion_protocol::Utf16Position {
                        line: first_line + offset,
                        character: 0,
                    },
                },
                truncation_state: legion_protocol::ViewportLineTruncationState::None,
            })
            .collect();
        projection
    }

    /// A card says which lines it is showing, not which lines it wishes it were.
    ///
    /// The excerpt is built from the buffer's saved scroll, so a file somebody
    /// has scrolled through produces a section starting partway down it. The
    /// card discarded every `line_number` and labelled the result "first 18
    /// lines", which presents a mid-file excerpt as the opening of the file --
    /// and where the tail was short enough not to be truncated, said nothing
    /// about the lines above it at all.
    #[test]
    fn a_card_names_the_lines_it_is_actually_showing() {
        // Scrolled and longer than the card: both omissions at once.
        let long = scrolled_section("scrolled.rs", 99, super::MAX_NODE_LINES as u32 + 20);
        let nodes = nodes_for_sections(&[long], &BTreeMap::new(), egui::Pos2::ZERO, None);
        let footer = super::excerpt_footer(&nodes[0])
            .expect("a truncated mid-file excerpt has something to disclose");
        assert!(
            footer.contains("100-") && footer.contains("longer excerpt"),
            "an excerpt starting at line 100 was labelled {footer:?}, which tells              the reader the card starts where the file does"
        );

        // Scrolled and short: nothing is cut off the bottom, and the old label
        // did not appear at all, so the lines above went unmentioned.
        let short = scrolled_section("tail.rs", 99, 4);
        let nodes = nodes_for_sections(&[short], &BTreeMap::new(), egui::Pos2::ZERO, None);
        assert_eq!(
            super::excerpt_footer(&nodes[0]).as_deref(),
            Some("lines 100-103"),
            "a short excerpt from the middle of a file must still say where it              starts; nothing else on the card does"
        );

        // From the top and complete: nothing omitted, so nothing claimed. A
        // card that discloses when there is nothing to disclose teaches people
        // to skip the line that matters.
        let whole = scrolled_section("whole.rs", 0, 4);
        let nodes = nodes_for_sections(&[whole], &BTreeMap::new(), egui::Pos2::ZERO, None);
        assert_eq!(
            super::excerpt_footer(&nodes[0]),
            None,
            "a complete excerpt from line 1 has nothing to disclose"
        );
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
        let nodes = nodes_for_sections(&sections, &BTreeMap::new(), egui::Pos2::ZERO, None);
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
        positions.insert("a.rs".to_string(), by_person(egui::pos2(10.0, 20.0)));

        let nodes = nodes_for_sections(&sections, &positions, egui::Pos2::ZERO, None);
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
        let first_pass = nodes_for_sections(&all, &BTreeMap::new(), egui::Pos2::ZERO, None);

        // What the renderer persists on that first frame.
        let saved: BTreeMap<String, super::SavedPosition> = first_pass
            .iter()
            .map(|node| (node.path.0.clone(), by_layout(node.position)))
            .collect();

        // The middle tab closes.
        let remaining = vec![section("a.rs"), section("c.rs")];
        let second_pass = nodes_for_sections(&remaining, &saved, egui::Pos2::ZERO, None);

        assert_eq!(
            second_pass[0].position, saved["a.rs"].position,
            "a.rs moved when b.rs was closed"
        );
        assert_eq!(
            second_pass[1].position, saved["c.rs"].position,
            "c.rs moved when b.rs was closed — the defect this numbering fixes"
        );
    }

    /// A card with a saved position does not shift the cards after it.
    ///
    /// Default slots used to come from a running count of *unplaced* cards, so
    /// one card gaining a position stopped the counter and moved every later
    /// card a slot to the left -- possibly onto the one just placed.
    #[test]
    fn a_place_somebody_chose_is_not_handed_to_another_file() {
        // The reopen collision, closed at source. A card somebody positioned is
        // closed; a new file is opened. If the new file were handed that place,
        // reopening the first would put two cards in one cell and whichever
        // rule fired next would have to move one of them -- either the person's
        // card, or a card nobody touched. Neither is acceptable, so the place
        // stays reserved while the file is closed.
        let mut positions = BTreeMap::new();
        positions.insert("closed.rs".to_string(), by_person(egui::pos2(0.0, 0.0)));

        let nodes = nodes_for_sections(&[section("fresh.rs")], &positions, egui::Pos2::ZERO, None);

        let fresh = nodes.first().expect("the new file must have a card");
        assert_ne!(
            fresh.position,
            egui::pos2(0.0, 0.0),
            "a new file was handed the place somebody had put a closed card in"
        );

        // And reopening it finds its place still free.
        let both = nodes_for_sections(
            &[section("fresh.rs"), section("closed.rs")],
            &positions,
            egui::Pos2::ZERO,
            None,
        );
        let reopened = both
            .iter()
            .find(|node| node.path.0 == "closed.rs")
            .expect("the reopened file must have a card");
        assert_eq!(
            reopened.position,
            egui::pos2(0.0, 0.0),
            "the reopened card must come back where it was left"
        );
        for other in both.iter().filter(|node| node.path.0 != "closed.rs") {
            assert!(
                (other.position.x - reopened.position.x).abs() >= DEFAULT_STRIDE
                    || (other.position.y - reopened.position.y).abs() >= DEFAULT_STRIDE,
                "{} is drawn over the reopened card at {:?}",
                other.path.0,
                reopened.position
            );
        }
    }

    #[test]
    fn a_card_dropped_on_another_moves_neither_of_them() {
        // Two cards a person put in the same place, on purpose. Repairing that
        // moves a card nobody asked to move -- and the earlier equality rule
        // moved whichever came second in tab order, so dragging card A onto
        // card B relocated *B*, which nobody had touched at all.
        let mut positions = BTreeMap::new();
        positions.insert("alpha.rs".to_string(), by_person(egui::pos2(120.0, 80.0)));
        positions.insert("beta.rs".to_string(), by_person(egui::pos2(120.0, 80.0)));

        let nodes = nodes_for_sections(
            &[section("alpha.rs"), section("beta.rs")],
            &positions,
            egui::Pos2::ZERO,
            None,
        );

        for node in &nodes {
            assert_eq!(
                node.position,
                egui::pos2(120.0, 80.0),
                "{} was moved out of an overlap a person created deliberately",
                node.path.0
            );
            assert!(
                node.placed,
                "a position a person chose must not be rewritten as a new default"
            );
        }
    }

    #[test]
    fn reused_default_slots_are_repaired_even_when_they_only_nearly_coincide() {
        // A closed card nudged slightly off its slot, and a new file that took
        // the slot. Neither position was chosen by a person, and the cards are
        // 320 units wide, so `(0, 0)` and `(10, 10)` cover each other almost
        // entirely -- while an equality test sees no collision at all.
        let mut positions = BTreeMap::new();
        positions.insert("closed.rs".to_string(), by_layout(egui::pos2(10.0, 10.0)));
        positions.insert("fresh.rs".to_string(), by_layout(egui::pos2(0.0, 0.0)));

        let nodes = nodes_for_sections(
            &[section("fresh.rs"), section("closed.rs")],
            &positions,
            egui::Pos2::ZERO,
            None,
        );

        assert_eq!(nodes.len(), 2, "both files must be drawn");
        let separation = (nodes[0].position - nodes[1].position).abs();
        assert!(
            separation.x >= DEFAULT_STRIDE || separation.y >= DEFAULT_STRIDE,
            "two automatically placed cards are still on top of each other at {:?} and {:?}",
            nodes[0].position,
            nodes[1].position
        );
    }

    #[test]
    fn the_slot_reservation_fits_the_tallest_card_there_can_be() {
        // The slot is chosen before the excerpt is read, so the reservation has
        // to cover the worst case. Counting only the header and the lines left
        // a truncated card needing 306 units judged on 280 -- and a slot with
        // room between those two numbers was accepted, putting the footer and
        // the last of the body off screen for good.
        let tallest = super::CanvasNode {
            path: CanonicalPath("tall.rs".to_string()),
            placed: false,
            buffer_id: None,
            title: "tall.rs".to_string(),
            accessible_name: "tall.rs".to_string(),
            dirty: false,
            lines_truncated: true,
            lines: vec!["x".to_string(); super::MAX_NODE_LINES],
            // A span, because the footer that makes this the tallest card is
            // only drawn when there is a range to name in it.
            line_span: Some((1, super::MAX_NODE_LINES as u32)),
            position: egui::Pos2::ZERO,
        };

        assert!(
            super::slot_card_rect(egui::Pos2::ZERO).height() >= super::node_height(&tallest),
            "a slot reserves {} units and the tallest card needs {}, so a slot with room \
             between the two is accepted and the card does not fit it",
            super::slot_card_rect(egui::Pos2::ZERO).height(),
            super::node_height(&tallest)
        );
    }

    #[test]
    fn a_new_card_prefers_a_slot_the_person_can_see() {
        // Three cards on the top row, a view with room for two rows. The next
        // free slot by number is 3, which is the second row and visible; the
        // rule matters when a *later* slot would be off screen, and the
        // renderer widens the view when no slot fits at all -- that half is
        // covered end to end by `every_open_file_has_a_card_on_screen`.
        let view = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(1200.0, 800.0));
        let mut positions = BTreeMap::new();
        let mut sections = Vec::new();
        for index in 0..3 {
            let name = format!("placed{index}.rs");
            positions.insert(
                name.clone(),
                by_layout(egui::pos2(index as f32 * DEFAULT_STRIDE, 0.0)),
            );
            sections.push(section(&name));
        }
        sections.push(section("fresh.rs"));

        let nodes = nodes_for_sections(&sections, &positions, egui::Pos2::ZERO, Some(view));

        let fresh = nodes
            .iter()
            .find(|node| node.path.0 == "fresh.rs")
            .expect("the newly opened file must have a card");
        assert!(
            view.contains_rect(super::slot_card_rect(fresh.position)),
            "the new card was placed at {:?}, outside the view {view:?}, while a slot inside \
             it was free",
            fresh.position
        );
    }

    #[test]
    fn cards_sharing_a_file_name_are_named_apart() {
        // `tab_title` projects only the file name, so two `index.ts` cards
        // published identical control names: "Connect from index.ts" twice,
        // with no way to tell which file a port belonged to and nothing to stop
        // a screen-reader user connecting the wrong two cards.
        let nodes = nodes_for_sections(
            &[
                section_titled_by_file_name("src/index.ts"),
                section_titled_by_file_name("tests/index.ts"),
            ],
            &BTreeMap::new(),
            egui::Pos2::ZERO,
            None,
        );

        assert_eq!(nodes.len(), 2, "both files must be drawn");
        assert_ne!(
            nodes[0].accessible_name, nodes[1].accessible_name,
            "two cards published the same name, so their ports are indistinguishable"
        );
        for node in &nodes {
            assert!(
                node.accessible_name.contains(&node.path.0),
                "an ambiguous name must carry enough path to resolve it, got {:?}",
                node.accessible_name
            );
        }
    }

    #[test]
    fn an_unambiguous_card_keeps_the_name_the_tab_bar_shows() {
        // The other direction: qualifying every card would make the canvas name
        // files differently from the tab bar, which is its own confusion.
        let nodes = nodes_for_sections(
            &[
                section_titled_by_file_name("src/main.rs"),
                section_titled_by_file_name("src/other.rs"),
            ],
            &BTreeMap::new(),
            egui::Pos2::ZERO,
            None,
        );

        for node in &nodes {
            assert_eq!(
                node.accessible_name, node.title,
                "a name nothing else uses must be left exactly as the tab bar shows it"
            );
        }
    }

    #[test]
    fn a_reopened_file_does_not_land_on_the_card_that_took_its_slot() {
        // The cost of not reserving slots for closed files. Close the card in
        // slot 0, open a new file into the vacancy, reopen the first: both are
        // saved at the same coordinates, and one is drawn underneath the other
        // where it cannot be reached without moving the one on top.
        let mut positions = BTreeMap::new();
        positions.insert("closed.rs".to_string(), by_layout(egui::pos2(0.0, 0.0)));
        positions.insert("fresh.rs".to_string(), by_layout(egui::pos2(0.0, 0.0)));

        let nodes = nodes_for_sections(
            &[section("fresh.rs"), section("closed.rs")],
            &positions,
            egui::Pos2::ZERO,
            None,
        );

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
                by_layout(egui::pos2(
                    (index % 3) as f32 * DEFAULT_STRIDE,
                    (index / 3) as f32 * DEFAULT_STRIDE,
                )),
            );
        }

        let nodes = nodes_for_sections(&[section("fresh.rs")], &positions, egui::Pos2::ZERO, None);

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
        positions.insert("far.rs".to_string(), by_person(egui::pos2(-4000.0, 2500.0)));
        positions.insert(
            "near.rs".to_string(),
            by_person(egui::pos2(-3600.0, 2500.0)),
        );

        let nodes = nodes_for_sections(
            &[section("far.rs"), section("near.rs")],
            &positions,
            egui::Pos2::ZERO,
            None,
        );
        let view = super::default_scene_rect(&nodes, egui::vec2(1200.0, 800.0));

        for node in &nodes {
            assert!(
                view.contains_rect(super::node_rect(node)),
                "{} sits outside the opening view {view:?}, so the canvas opens empty with no \
                 way to find it",
                node.path.0
            );
        }
    }

    /// Widening the view to reach a new card keeps it drawable.
    ///
    /// `Scene` honours the rect it is handed only down to `ZOOM_MIN`. Past
    /// that it clamps the scale and draws a smaller window into the rectangle,
    /// so a view grown to reach a card can be a view that no longer shows it --
    /// silently, with no minimap and no fit control to recover with. The
    /// oversize handling lived only in `default_scene_rect`, which this later
    /// expansion does not go through.
    #[test]
    fn reaching_a_new_card_never_asks_for_more_than_the_zoom_floor_can_draw() {
        // Enough saved cards that the next default slot is far below the view:
        // the grid wraps every few columns, so forty cards is several thousand
        // world units of rows.
        let panel = egui::vec2(1200.0, 800.0);
        let mut positions = BTreeMap::new();
        let mut sections = Vec::new();
        for index in 0..40 {
            let path = format!("file{index}.rs");
            positions.insert(
                path.clone(),
                by_person(super::slot_position_from(egui::Pos2::ZERO, index)),
            );
            sections.push(section(&path));
        }
        sections.push(section("newest.rs"));

        let view = egui::Rect::from_min_size(egui::Pos2::ZERO, panel);
        let nodes = nodes_for_sections(&sections, &positions, egui::Pos2::ZERO, Some(view));
        let newest = nodes
            .iter()
            .find(|node| node.path.0 == "newest.rs")
            .expect("the card just opened must be on the canvas");
        assert!(
            !newest.placed,
            "this test is about a card the canvas placed itself; with a saved              position it would never widen the view and the assertion below              would hold for the wrong reason"
        );

        let widened = super::view_reaching_new_cards(view, &nodes, panel);
        let drawable = panel / super::ZOOM_MIN;
        assert!(
            widened.width() <= drawable.x && widened.height() <= drawable.y,
            "the view grew to {widened:?}, past the {drawable:?} that `Scene`              can draw at the zoom floor -- egui clamps and shows less than this"
        );
        assert!(
            widened.contains_rect(super::node_rect(newest)),
            "the card the view was widened for sits outside it at {:?}, which is              the failure the widening exists to prevent",
            super::node_rect(newest)
        );
        // And at the size somebody was already working at. Filling the whole
        // drawable rectangle instead would satisfy both assertions above by
        // zooming out to a quarter scale, where the card is on screen and
        // cannot be read -- which is not what reaching it meant.
        assert_eq!(
            widened.size(),
            view.size(),
            "the view changed zoom to reach a new card; it should have moved"
        );
    }

    /// One card just past the edge does not shrink the ones already on screen.
    ///
    /// The ordinary case, and the one that reached a person first: a full grid
    /// and one more file opened. The union of the view and that card is still
    /// inside what `Scene` can draw, so a floor check alone lets it through --
    /// and `Scene` fits the larger rectangle by zooming out, so opening a file
    /// shrinks every card already visible. Nothing about the new card being
    /// slightly outside justifies rescaling the ones that were not.
    #[test]
    fn a_card_just_past_the_edge_moves_the_view_rather_than_zooming_it_out() {
        let panel = egui::vec2(1200.0, 800.0);
        let view = egui::Rect::from_min_size(egui::Pos2::ZERO, panel);
        let mut positions = BTreeMap::new();
        positions.insert("saved.rs".to_string(), by_person(egui::pos2(20.0, 20.0)));
        // Just below the bottom edge: the union is 1200x1100 or so, well inside
        // the 4800x3200 the zoom floor allows.
        let nodes = nodes_for_sections(
            &[section("saved.rs"), section("newest.rs")],
            &positions,
            egui::pos2(40.0, 900.0),
            Some(view),
        );
        let newest = nodes
            .iter()
            .find(|node| node.path.0 == "newest.rs")
            .expect("the newly opened file must be on the canvas");
        assert!(
            !view.contains_rect(super::node_rect(newest)),
            "this test needs a card outside the view; it is at {:?} inside {view:?}",
            super::node_rect(newest)
        );

        let reached = super::view_reaching_new_cards(view, &nodes, panel);
        assert_eq!(
            reached.size(),
            view.size(),
            "the view was resized to include a card just past its edge, which              makes every card already on screen smaller"
        );
        assert!(
            reached.contains_rect(super::node_rect(newest)),
            "the card at {:?} is still outside the view {reached:?}",
            super::node_rect(newest)
        );
        // And no further than it had to go: the card was below, so the view
        // moves down and not sideways.
        assert_eq!(
            reached.min.x, view.min.x,
            "the view moved along an axis that already reached the card"
        );
    }

    /// Fitting shows as much of the arrangement as can be drawn.
    ///
    /// The control is named for fitting everything, and it was calling the
    /// *opening* view -- which deliberately falls back to a small readable
    /// rectangle around the first card once the arrangement outgrows the zoom
    /// floor. Pressing "Fit all cards" therefore did not fit them, and on an
    /// arrangement that large it left blind panning as the way to find the
    /// rest.
    #[test]
    fn fitting_shows_as_much_of_the_arrangement_as_can_be_drawn() {
        let panel = egui::vec2(1200.0, 800.0);
        let mut positions = BTreeMap::new();
        let mut sections = Vec::new();
        // Deliberately wider than `panel / ZOOM_MIN` (4800 x 3200), so the
        // opening view takes its readable fallback and a fit must not.
        for (index, x) in [0.0_f32, 3000.0, 6000.0].iter().enumerate() {
            let path = format!("far{index}.rs");
            positions.insert(path.clone(), by_person(egui::pos2(*x, 0.0)));
            sections.push(section(&path));
        }
        let nodes = nodes_for_sections(&sections, &positions, egui::Pos2::ZERO, None);

        let opening = super::default_scene_rect(&nodes, panel);
        let fitted = super::fit_scene_rect(&nodes, panel);
        let drawable = panel / super::ZOOM_MIN;

        assert!(
            fitted.width() > opening.width(),
            "fitting showed no more than the opening view ({fitted:?} against              {opening:?}), so the control named for fitting everything did nothing"
        );
        assert!(
            fitted.width() <= drawable.x && fitted.height() <= drawable.y,
            "fitting asked for {fitted:?}, past the {drawable:?} `Scene` can draw --              egui clamps and shows less than this, so the fit would be a lie"
        );
        assert!(
            fitted.width() >= drawable.x - 1.0,
            "an arrangement wider than the floor allows should fill what the floor              allows; it used {fitted:?}"
        );

        // And an arrangement that does fit is shown whole, not merely widened.
        let mut near = BTreeMap::new();
        near.insert("a.rs".to_string(), by_person(egui::pos2(0.0, 0.0)));
        near.insert("b.rs".to_string(), by_person(egui::pos2(500.0, 0.0)));
        let nodes = nodes_for_sections(
            &[section("a.rs"), section("b.rs")],
            &near,
            egui::Pos2::ZERO,
            None,
        );
        let fitted = super::fit_scene_rect(&nodes, panel);
        for node in &nodes {
            assert!(
                fitted.contains_rect(super::node_rect(node)),
                "{} is outside the fitted view {fitted:?}, and the whole arrangement                  fits inside what can be drawn",
                node.path.0
            );
        }
    }

    #[test]
    fn the_opening_view_of_a_lone_card_is_not_a_single_card() {
        // The other direction. Fitting exactly to one card opens zoomed to fill
        // the screen with it, which is a different kind of disorienting.
        let nodes = nodes_for_sections(
            &[section("only.rs")],
            &BTreeMap::new(),
            egui::Pos2::ZERO,
            None,
        );
        let view = super::default_scene_rect(&nodes, egui::vec2(1200.0, 800.0));

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
        positions.insert(
            "alpha.rs".to_string(),
            by_person(egui::pos2(DEFAULT_STRIDE, 0.0)),
        );

        let nodes = nodes_for_sections(
            &[section("alpha.rs"), section("beta.rs")],
            &positions,
            egui::Pos2::ZERO,
            None,
        );

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
        positions.insert("alpha.rs".to_string(), by_layout(egui::pos2(0.0, 0.0)));
        positions.insert(
            "beta.rs".to_string(),
            by_layout(egui::pos2(DEFAULT_STRIDE, 0.0)),
        );

        let nodes = nodes_for_sections(
            &[section("alpha.rs"), section("beta.rs"), section("gamma.rs")],
            &positions,
            egui::Pos2::ZERO,
            None,
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

        let untouched = nodes_for_sections(&sections, &BTreeMap::new(), egui::Pos2::ZERO, None);
        let b_before = untouched[1].position;
        let c_before = untouched[2].position;

        let mut positions = BTreeMap::new();
        positions.insert("a.rs".to_string(), by_person(egui::pos2(-900.0, 900.0)));
        let after = nodes_for_sections(&sections, &positions, egui::Pos2::ZERO, None);

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
