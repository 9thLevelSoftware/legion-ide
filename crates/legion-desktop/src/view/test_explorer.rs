//! Test explorer tree rendering for the Tests surface.

use legion_ui::ShellProjectionSnapshot;

use super::components::soft_button;
use super::theme;
use crate::bridge::DesktopAction;

pub(super) fn render_test_explorer_tree(
    ui: &mut egui::Ui,
    snapshot: &ShellProjectionSnapshot,
    actions: &mut Vec<DesktopAction>,
) {
    let explorer = &snapshot.test_explorer_projection;
    if explorer.status_label == "discovering" || explorer.status_label == "running" {
        ui.label(theme::muted(format!("Tests {}…", explorer.status_label)));
    }
    if explorer.items.is_empty() {
        return;
    }

    ui.separator();
    ui.label(theme::label("Test tree"));
    egui::ScrollArea::vertical()
        .id_salt("legion_desktop_test_explorer_tree")
        .max_height(180.0)
        .auto_shrink([false, true])
        .show(ui, |ui| {
            for group in legion_ui::group_test_explorer_items_by_parent(&explorer.items) {
                let group_label = format!("{} ({})", group.parent_label, group.items.len());
                let group_path = group.parent_label.clone();
                let collapsing = egui::CollapsingHeader::new(theme::label(&group_label))
                    .id_salt(("legion_test_group", &group_path))
                    .default_open(true)
                    .show(ui, |ui| {
                        for item in group.items {
                            let response = ui.selectable_label(
                                false,
                                theme::label(format!("{}  [{}]", item.label, item.kind_label)),
                            );
                            ui.ctx().accesskit_node_builder(response.id, |node| {
                                node.set_role(egui::accesskit::Role::TreeItem);
                                node.set_label(item.label.as_str());
                            });
                            if response.clicked() {
                                actions.push(DesktopAction::RunTestExplorerItem {
                                    item_id: item.item_id.clone(),
                                });
                            }
                        }
                    });
                ui.ctx()
                    .accesskit_node_builder(collapsing.header_response.id, |node| {
                        node.set_role(egui::accesskit::Role::TreeItem);
                        node.set_label(group_label.as_str());
                    });
                if soft_button(ui, "Run group").clicked() {
                    actions.push(DesktopAction::RunTestExplorerGroup {
                        parent_label: group_path,
                    });
                }
            }
        });
}
