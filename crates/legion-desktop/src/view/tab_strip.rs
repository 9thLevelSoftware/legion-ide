//! The editor tab strip.
//!
//! Moved out of `view.rs` unchanged in behaviour. The strip is a self-contained
//! surface — tabs, their close affordance, and drag-to-reorder — and `view.rs`
//! is a chokepoint file that the `extract-before-modify` gate keeps from
//! growing, so the region being worked on moves out rather than up.

use egui::{self};
use legion_protocol::BufferId;
use legion_ui::ShellProjectionSnapshot;

use crate::bridge::DesktopAction;
use crate::theme;

const TAB_DIRTY_GLYPH: &str = "\u{2022}";
const TAB_CLOSE_GLYPH: &str = "\u{00d7}";

/// Horizontal padding inside a tab.
const TAB_PADDING_X: f32 = 8.0;
/// Gap between a tab's label and its close affordance.
const TAB_LABEL_CLOSE_GAP: f32 = 2.0;
/// Side of a tab's close affordance — its *hit* area.
///
/// 28px is this project's minimum interactive target (`control_height.compact`,
/// enforced for every rendered control by the accessibility suite). The glyph
/// drawn inside it is much smaller; a close button that looks like a 14px `×`
/// but answers to a 28px square is easier to hit and no louder to look at.
const TAB_CLOSE_SIZE: f32 = 28.0;
/// Side of the rounded chip that lights up on hover, not the glyph inside it.
const TAB_CLOSE_HOVER_SIZE: f32 = 16.0;
/// Point size of the painted `×` / dirty glyph.
const TAB_CLOSE_GLYPH_FONT_SIZE: f32 = 12.0;
/// Height of a tab.
const TAB_HEIGHT: f32 = 28.0;

/// The close / unsaved-changes affordance drawn inside a tab.
///
/// A dirty tab shows a dot until you point at it, then offers the `×` — so the
/// unsaved marker and the close target occupy one slot instead of two, and the
/// tab does not change width when its contents are edited.
///
/// Takes an explicit rect and registers itself with `interact`, so the caller
/// controls when it enters the widget list. It has to be registered *after* the
/// tab whose rect encloses it, or egui hit-tests in the tab's favour and the
/// close button becomes decorative.
fn render_tab_close_affordance(
    ui: &mut egui::Ui,
    tab: &legion_ui::ui::EditorTabProjection,
    rect: egui::Rect,
) -> egui::Response {
    let response = ui.interact(
        rect,
        egui::Id::new(("legion_editor_tab_close", tab.buffer_id.0)),
        egui::Sense::click(),
    );
    // `interact` produces an untyped node; the accessibility suite checks
    // interactive targets by role, so this has to say what it is.
    ui.ctx().accesskit_node_builder(response.id, |node| {
        node.set_role(egui::accesskit::Role::Button);
    });
    let hovered = response.hovered();
    let glyph = if tab.dirty && !hovered {
        TAB_DIRTY_GLYPH
    } else {
        TAB_CLOSE_GLYPH
    };
    let color = if hovered {
        theme::tokens().text.primary
    } else {
        theme::tokens().text.muted
    };
    if hovered {
        // Only a small square lights up. Filling the whole 28px hit
        // area would put a block of highlight across half the tab.
        ui.painter().rect_filled(
            egui::Rect::from_center_size(rect.center(), egui::Vec2::splat(TAB_CLOSE_HOVER_SIZE)),
            egui::CornerRadius::same(3),
            theme::tokens().bg.hover,
        );
    }
    ui.painter().text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        glyph,
        egui::FontId::proportional(TAB_CLOSE_GLYPH_FONT_SIZE),
        color,
    );
    response
}

/// Persistent drag state for tab reorder, stored in `egui::Context::data_mut`.
#[derive(Clone, Default)]
struct TabDragState {
    /// Buffer id of the tab currently being dragged.
    dragging: Option<BufferId>,
    /// Original index of the dragged tab (at drag start).
    source_index: usize,
    /// Index of the tab currently under the pointer during a drag.
    drop_target: Option<usize>,
}

pub(super) fn adjusted_tab_drop_target(source_index: usize, target_index: usize) -> usize {
    if source_index < target_index {
        target_index.saturating_sub(1)
    } else {
        target_index
    }
}

pub(super) fn render_tab_strip(
    ui: &mut egui::Ui,
    snapshot: &ShellProjectionSnapshot,
    actions: &mut Vec<DesktopAction>,
) {
    let drag_state_id = egui::Id::new("tab_strip_drag_state");

    theme::pane_frame(theme::tokens().bg.panel).show(ui, |ui| {
        ui.set_height(34.0);
        let tabs = &snapshot.daily_editing_projection.tabs.tabs;
        if tabs.is_empty() {
            ui.horizontal(|ui| {
                ui.label(theme::muted("<no open tabs>"));
            });
            return;
        }

        // Wrap tabs in a horizontal scroll area.  Drag-to-scroll is disabled so
        // that pointer drag is reserved for tab reorder; users scroll with the
        // mouse wheel.
        egui::ScrollArea::horizontal()
            .auto_shrink([false, false])
            .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::VisibleWhenNeeded)
            .scroll_source(egui::scroll_area::ScrollSource {
                scroll_bar: true,
                drag: false,
                mouse_wheel: true,
            })
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    // Load current drag state and reset per-frame target.
                    let mut drag: TabDragState = ui
                        .ctx()
                        .data_mut(|d| d.get_temp(drag_state_id).unwrap_or_default());
                    drag.drop_target = None;

                    for (tab_index, tab) in tabs.iter().enumerate() {
                        // --- build tab label ---
                        let color = if tab.active {
                            theme::tokens().text.primary
                        } else {
                            theme::tokens().text.muted
                        };
                        let tab_fill = if tab.active {
                            theme::tokens().bg.code
                        } else {
                            theme::tokens().bg.panel
                        };
                        let tab_stroke = egui::Stroke::new(
                            1.0_f32,
                            if tab.active {
                                theme::tokens().border.default
                            } else {
                                theme::tokens().bg.panel
                            },
                        );

                        // The tab and its close affordance share one frame, so
                        // the `×` sits inside the tab rather than floating to
                        // its right — previously each was an independent
                        // `Button` laid out side by side and the layout's
                        // inter-widget spacing fell between them, which read as
                        // two unrelated controls.
                        //
                        // Registration order is the whole trick here. egui
                        // hit-tests in favour of the *last* widget registered
                        // at a point, and the tab's rect completely contains
                        // the close button's. So the tab is registered first
                        // and the close button after, or the tab silently eats
                        // every click aimed at the `×` and switches to the tab
                        // instead of closing it.
                        let title_galley = ui.painter().layout_no_wrap(
                            tab.title.clone(),
                            egui::TextStyle::Button.resolve(ui.style()),
                            color,
                        );
                        let tab_size = egui::vec2(
                            TAB_PADDING_X * 2.0
                                + title_galley.size().x
                                + TAB_LABEL_CLOSE_GAP
                                + TAB_CLOSE_SIZE,
                            TAB_HEIGHT,
                        );
                        let (tab_rect, tab_response) =
                            ui.allocate_exact_size(tab_size, egui::Sense::click_and_drag());
                        ui.painter().rect(
                            tab_rect,
                            egui::CornerRadius::same(6),
                            tab_fill,
                            tab_stroke,
                            egui::StrokeKind::Inside,
                        );
                        ui.painter().galley(
                            egui::pos2(
                                tab_rect.left() + TAB_PADDING_X,
                                tab_rect.center().y - title_galley.size().y / 2.0,
                            ),
                            title_galley,
                            color,
                        );
                        let close_rect = egui::Rect::from_center_size(
                            egui::pos2(
                                tab_rect.right() - TAB_PADDING_X - TAB_CLOSE_SIZE / 2.0,
                                tab_rect.center().y,
                            ),
                            egui::Vec2::splat(TAB_CLOSE_SIZE),
                        );
                        let close_response = Some(render_tab_close_affordance(ui, tab, close_rect));
                        ui.ctx().accesskit_node_builder(tab_response.id, |node| {
                            node.set_role(egui::accesskit::Role::Tab);
                            node.set_label(tab.title.as_str());
                            node.set_selected(tab.active);
                            if tab.dirty {
                                node.set_description("Unsaved changes");
                            } else {
                                node.clear_description();
                            }
                            if tab.active {
                                node.set_aria_current(egui::accesskit::AriaCurrent::True);
                            } else {
                                node.clear_aria_current();
                            }
                        });

                        let mut close_clicked = false;
                        if let Some(close_response) = close_response {
                            ui.ctx().accesskit_node_builder(close_response.id, |node| {
                                node.set_label(format!("Close {}", tab.title));
                            });
                            if close_response.clicked() {
                                close_clicked = true;
                                actions.push(DesktopAction::CloseTab {
                                    buffer_id: tab.buffer_id,
                                });
                            }
                        }

                        // Add a small gap between tabs.
                        ui.add_space(2.0);

                        // --- drag-to-reorder ---
                        if !close_clicked {
                            if tab_response.drag_started() {
                                drag.dragging = Some(tab.buffer_id);
                                drag.source_index = tab_index;
                            }

                            // While dragging, check if the cursor is over this
                            // tab to determine the drop target.  Uses
                            // `contains_pointer()` instead of `hovered()` because
                            // egui suppresses `hovered` on non-source widgets
                            // during a drag.
                            if let Some(dragging_id) = drag.dragging
                                && dragging_id != tab.buffer_id
                                && tab_response.contains_pointer()
                            {
                                // Treat the left and right halves of a tab as
                                // distinct insertion slots. The slot is
                                // measured before removing the source tab,
                                // then adjusted once at release.
                                let insert_after = tab_response
                                    .interact_pointer_pos()
                                    .is_some_and(|pos| pos.x >= tab_response.rect.center().x);
                                let target_index = if insert_after {
                                    tab_index.saturating_add(1)
                                } else {
                                    tab_index
                                };
                                drag.drop_target = Some(target_index);
                                let rect = tab_response.rect;
                                let indicator_x = if insert_after {
                                    rect.right()
                                } else {
                                    rect.left()
                                };
                                let painter = ui.painter();
                                painter.line_segment(
                                    [
                                        egui::pos2(indicator_x, rect.top()),
                                        egui::pos2(indicator_x, rect.bottom()),
                                    ],
                                    egui::Stroke::new(2.0_f32, theme::tokens().accent.blue),
                                );
                            }
                        }

                        // --- left-click to switch tab ---
                        if !close_clicked && tab_response.clicked() {
                            actions.push(DesktopAction::SwitchTab {
                                buffer_id: tab.buffer_id,
                            });
                        }

                        // --- context menu ---
                        tab_response.context_menu(|ui| {
                            if ui.button("Close").clicked() {
                                actions.push(DesktopAction::CloseTab {
                                    buffer_id: tab.buffer_id,
                                });
                                ui.close();
                            }
                            if ui.button("Close Others").clicked() {
                                for other in
                                    tabs.iter().filter(|other| other.buffer_id != tab.buffer_id)
                                {
                                    actions.push(DesktopAction::CloseTab {
                                        buffer_id: other.buffer_id,
                                    });
                                }
                                ui.close();
                            }
                            if ui.button("Close All").clicked() {
                                for other in tabs {
                                    actions.push(DesktopAction::CloseTab {
                                        buffer_id: other.buffer_id,
                                    });
                                }
                                ui.close();
                            }
                        });
                    }

                    // Handle pointer release: fire ReorderTab if dropped on
                    // a valid target, then clear drag state.
                    if ui.input(|i| i.pointer.any_released()) {
                        if let Some(dragging_id) = drag.dragging.take()
                            && let Some(target) = drag.drop_target.take()
                        {
                            // `target` is a pre-removal insertion slot.
                            // Removing a tab from the left shifts every later
                            // slot by one before insertion.
                            let adjusted_target =
                                adjusted_tab_drop_target(drag.source_index, target);
                            if adjusted_target != drag.source_index {
                                actions.push(DesktopAction::ReorderTab {
                                    buffer_id: dragging_id,
                                    new_index: adjusted_target,
                                });
                            }
                        }
                        drag.drop_target = None;
                    }

                    // Persist drag state.
                    ui.ctx().data_mut(|d| d.insert_temp(drag_state_id, drag));
                });
            });
    });
}
