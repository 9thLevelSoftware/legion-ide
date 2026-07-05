//! Renderer-backed scope picker for delegated tasks.

use egui::Ui;
use legion_protocol::{CanonicalPath, DelegatedTaskScope, LegionToolKind};

/// Renderer-facing alias for the risk tolerance.
pub use legion_protocol::DelegatedTaskRiskTolerance as ScopeRiskTolerance;
/// Renderer-facing alias for the scope target kind.
pub use legion_protocol::DelegatedTaskScopeTargetKind as ScopeTargetKind;

/// Structured view model for the delegated-task scope picker.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DesktopScopePickerViewModel {
    /// Selected target kind.
    pub target_kind: ScopeTargetKind,
    /// Repository/workspace root label.
    pub workspace_root: String,
    /// Optional file or module anchor label.
    pub target_path: Option<String>,
    /// Selected risk tolerance.
    pub risk_tolerance: ScopeRiskTolerance,
    /// Tools visible as allowed in the picker.
    pub allowed_tools: Vec<LegionToolKind>,
    /// Explicitly forbidden path labels.
    pub forbidden_paths: Vec<String>,
    /// DTO schema version.
    pub schema_version: u16,
}

impl Default for DesktopScopePickerViewModel {
    fn default() -> Self {
        Self {
            target_kind: ScopeTargetKind::Repo,
            workspace_root: "workspace".to_string(),
            target_path: None,
            risk_tolerance: ScopeRiskTolerance::Balanced,
            allowed_tools: vec![
                LegionToolKind::Read,
                LegionToolKind::Grep,
                LegionToolKind::Glob,
                LegionToolKind::Outline,
            ],
            forbidden_paths: Vec::new(),
            schema_version: 1,
        }
    }
}

impl From<DelegatedTaskScope> for DesktopScopePickerViewModel {
    fn from(scope: DelegatedTaskScope) -> Self {
        Self {
            target_kind: scope.target_kind,
            workspace_root: scope.workspace_root.0,
            target_path: scope.target_path.map(|path| path.0),
            risk_tolerance: scope.risk_tolerance,
            allowed_tools: scope.allowed_tools,
            forbidden_paths: scope
                .forbidden_paths
                .into_iter()
                .map(|path| path.0)
                .collect(),
            schema_version: scope.schema_version,
        }
    }
}

impl From<DesktopScopePickerViewModel> for DelegatedTaskScope {
    fn from(model: DesktopScopePickerViewModel) -> Self {
        Self {
            target_kind: model.target_kind,
            workspace_root: CanonicalPath(model.workspace_root),
            target_path: model.target_path.map(CanonicalPath),
            risk_tolerance: model.risk_tolerance,
            allowed_tools: model.allowed_tools,
            forbidden_paths: model
                .forbidden_paths
                .into_iter()
                .map(CanonicalPath)
                .collect(),
            schema_version: model.schema_version,
        }
    }
}

impl DesktopScopePickerViewModel {
    /// Returns a user-facing summary row for the selected scope.
    pub fn summary_label(&self) -> String {
        let target = match self.target_kind {
            ScopeTargetKind::File => self.target_path.as_deref().unwrap_or("file"),
            ScopeTargetKind::Module => self.target_path.as_deref().unwrap_or("module"),
            ScopeTargetKind::Repo => self.workspace_root.as_str(),
        };
        format!(
            "{} · {} · {} tools",
            self.target_kind.label(),
            target,
            self.allowed_tools.len()
        )
    }
}

/// Renders a compact, projection-only scope picker card.
pub fn render_scope_picker(ui: &mut Ui, model: &DesktopScopePickerViewModel) {
    egui::Frame::group(ui.style()).show(ui, |ui| {
        ui.vertical(|ui| {
            ui.label("Scope picker");
            ui.horizontal_wrapped(|ui| {
                ui.label(format!("target={}", model.target_kind.label()));
                ui.separator();
                ui.label(format!("risk={}", model.risk_tolerance.label()));
                ui.separator();
                ui.label(format!("allowed tools={}", model.allowed_tools.len()));
                if let Some(target_path) = &model.target_path {
                    ui.separator();
                    ui.label(format!("anchor={target_path}"));
                }
            });
            ui.label(model.summary_label());
            if !model.forbidden_paths.is_empty() {
                ui.label(format!("forbidden={}", model.forbidden_paths.join(", ")));
            }
        });
    });
}
