//! Debug call-stack and variable inspector for the Run and Debug panel.

use legion_ui::ShellProjectionSnapshot;

use super::theme;
use crate::bridge::DesktopAction;

pub(super) const DEBUG_STACK_FRAME_RENDER_LIMIT: usize = 32;

pub(super) fn render_debug_inspector(
    ui: &mut egui::Ui,
    snapshot: &ShellProjectionSnapshot,
    actions: &mut Vec<DesktopAction>,
) {
    let debug = &snapshot.debug_projection;
    if debug.active_session_id.is_none()
        || (debug.stack_frames.is_empty() && debug.variables.is_empty())
    {
        return;
    }

    ui.separator();
    egui::CollapsingHeader::new("Call stack")
        .default_open(true)
        .show(ui, |ui| {
            ui.ctx().accesskit_node_builder(ui.unique_id(), |node| {
                node.set_role(egui::accesskit::Role::List);
                node.set_label("Debug call stack");
            });
            if debug.stack_frames.is_empty() {
                ui.label(theme::muted("No stack frames"));
            } else {
                let selected_index = debug_selected_stack_frame_index(
                    ui.ctx(),
                    debug.stack_frames.len().min(DEBUG_STACK_FRAME_RENDER_LIMIT),
                );
                for (index, frame) in debug
                    .stack_frames
                    .iter()
                    .take(DEBUG_STACK_FRAME_RENDER_LIMIT)
                    .enumerate()
                {
                    let response = ui.selectable_label(
                        index == selected_index,
                        debug_stack_frame_label(frame, index),
                    );
                    if response.clicked() {
                        set_debug_selected_stack_frame_index(ui.ctx(), index);
                        if let Some(action) = debug_frame_navigation_action(frame) {
                            actions.push(action);
                        }
                    }
                    ui.ctx().accesskit_node_builder(response.id, |node| {
                        node.set_role(egui::accesskit::Role::ListItem);
                        node.set_label(debug_stack_frame_label(frame, index));
                        node.set_selected(index == selected_index);
                    });
                }
            }
        });

    egui::CollapsingHeader::new("Variables")
        .default_open(true)
        .show(ui, |ui| {
            ui.ctx().accesskit_node_builder(ui.unique_id(), |node| {
                node.set_role(egui::accesskit::Role::Tree);
                node.set_label("Debug variables");
            });
            if debug.variables.is_empty() {
                ui.label(theme::muted("No variables"));
            } else {
                for variable in debug.variables.iter().take(64) {
                    let label = debug_variable_label(variable);
                    let response = ui.label(label.clone());
                    ui.ctx().accesskit_node_builder(response.id, |node| {
                        node.set_role(egui::accesskit::Role::TreeItem);
                        node.set_label(label);
                        if let Some(expanded) = debug_variable_accesskit_expanded(variable) {
                            node.set_expanded(expanded);
                        }
                    });
                }
            }
        });
}

fn debug_stack_selection_id() -> egui::Id {
    egui::Id::new("legion.debug.selected-stack-frame")
}

pub(super) fn debug_selected_stack_frame_index(ctx: &egui::Context, frame_count: usize) -> usize {
    if frame_count == 0 {
        return 0;
    }
    ctx.data(|data| {
        data.get_temp::<usize>(debug_stack_selection_id())
            .unwrap_or_default()
            .min(frame_count - 1)
    })
}

pub(super) fn set_debug_selected_stack_frame_index(ctx: &egui::Context, index: usize) {
    ctx.data_mut(|data| data.insert_temp(debug_stack_selection_id(), index));
}

pub(super) fn debug_frame_navigation_action(
    frame: &legion_ui::DebugStackFrameProjection,
) -> Option<DesktopAction> {
    Some(DesktopAction::NavigateToProblem {
        path: frame.path.as_ref()?.0.clone(),
        line: frame.line?.saturating_sub(1),
    })
}

fn debug_stack_frame_label(frame: &legion_ui::DebugStackFrameProjection, index: usize) -> String {
    let path = frame
        .path
        .as_ref()
        .map(|path| path.0.as_str())
        .unwrap_or("<unknown>");
    let line = frame
        .line
        .map(|line| line.to_string())
        .unwrap_or_else(|| "<unknown>".to_string());
    format!("{} · {} · {}:{}", index + 1, frame.name, path, line)
}

fn debug_variable_label(variable: &legion_ui::DebugVariableProjection) -> String {
    format!(
        "{} = {}{}",
        variable.name,
        variable.value_label,
        variable
            .type_label
            .as_deref()
            .map(|type_label| format!(" · {type_label}"))
            .unwrap_or_default()
    )
}

fn debug_variable_accesskit_expanded(
    variable: &legion_ui::DebugVariableProjection,
) -> Option<bool> {
    (!variable.has_children).then_some(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use legion_protocol::CanonicalPath;
    use legion_ui::{DebugStackFrameProjection, DebugVariableProjection};

    #[test]
    fn debug_inspector_labels_and_navigation_preserve_projection_metadata() {
        let frame = DebugStackFrameProjection {
            session_id: legion_protocol::DebugSessionId("debug:1".into()),
            frame_id: 7,
            name: "main".into(),
            path: Some(CanonicalPath("src/main.rs".into())),
            line: Some(12),
        };
        assert_eq!(
            debug_stack_frame_label(&frame, 0),
            "1 · main · src/main.rs:12"
        );
        assert_eq!(
            debug_frame_navigation_action(&frame),
            Some(DesktopAction::NavigateToProblem {
                path: "src/main.rs".into(),
                line: 11,
            })
        );

        let variable = DebugVariableProjection {
            session_id: legion_protocol::DebugSessionId("debug:1".into()),
            name: "count".into(),
            value_label: "3".into(),
            type_label: Some("i32".into()),
            has_children: false,
        };
        assert_eq!(debug_variable_label(&variable), "count = 3 · i32");
        assert_eq!(debug_variable_accesskit_expanded(&variable), Some(false));

        let expandable_variable = DebugVariableProjection {
            has_children: true,
            ..variable
        };
        assert_eq!(
            debug_variable_accesskit_expanded(&expandable_variable),
            None
        );
    }
}
