//! Structured delegated-task scope DTOs.

use serde::{Deserialize, Serialize};
use std::path::Path;

use crate::{CanonicalPath, LegionToolKind};

/// Scope selector for delegated work.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DelegatedTaskScopeTargetKind {
    /// Scope a single file.
    File,
    /// Scope a module or directory subtree.
    Module,
    /// Scope the whole repository/workspace.
    Repo,
}

impl DelegatedTaskScopeTargetKind {
    /// Stable display label for the target kind.
    pub const fn label(self) -> &'static str {
        match self {
            Self::File => "file",
            Self::Module => "module",
            Self::Repo => "repo",
        }
    }
}

/// Risk tolerance for delegated work.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DelegatedTaskRiskTolerance {
    /// Prefer the smallest possible blast radius.
    Conservative,
    /// Prefer bounded progress with a normal approval footprint.
    Balanced,
    /// Prefer broader reach when the work is well understood.
    Aggressive,
}

impl DelegatedTaskRiskTolerance {
    /// Stable display label for the risk tolerance.
    pub const fn label(self) -> &'static str {
        match self {
            Self::Conservative => "conservative",
            Self::Balanced => "balanced",
            Self::Aggressive => "aggressive",
        }
    }
}

/// Structured delegated-task scope selection.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DelegatedTaskScope {
    /// Selected scope target kind.
    pub target_kind: DelegatedTaskScopeTargetKind,
    /// Repository/workspace root for the task.
    pub workspace_root: CanonicalPath,
    /// File or module anchor path when the target kind is not repo-scoped.
    pub target_path: Option<CanonicalPath>,
    /// Risk tolerance chosen by the user.
    pub risk_tolerance: DelegatedTaskRiskTolerance,
    /// Tools allowed within this scope.
    pub allowed_tools: Vec<LegionToolKind>,
    /// Explicitly forbidden paths.
    pub forbidden_paths: Vec<CanonicalPath>,
    /// Scope DTO schema version.
    pub schema_version: u16,
}

impl DelegatedTaskScope {
    /// Returns true when the tool is allowed by this scope.
    pub fn allows_tool(&self, tool: LegionToolKind) -> bool {
        self.allowed_tools.contains(&tool)
    }

    /// Returns true when the path is blocked by an explicit forbidden-path entry.
    pub fn forbids_path(&self, path: &CanonicalPath) -> bool {
        let candidate = Path::new(path.0.as_str());
        self.forbidden_paths.iter().any(|forbidden| {
            let forbidden = Path::new(forbidden.0.as_str());
            candidate == forbidden || candidate.starts_with(forbidden)
        })
    }

    /// Returns true when a tool target remains within the selected scope.
    pub fn target_is_within_scope(&self, target_path: Option<&Path>) -> bool {
        let Some(target_path) = target_path else {
            return self.target_kind == DelegatedTaskScopeTargetKind::Repo;
        };

        let workspace_root = Path::new(self.workspace_root.0.as_str());
        if !target_path.starts_with(workspace_root) {
            return false;
        }

        match self.target_kind {
            DelegatedTaskScopeTargetKind::Repo => true,
            DelegatedTaskScopeTargetKind::Module => {
                self.target_path.as_ref().is_some_and(|target_root| {
                    target_path.starts_with(Path::new(target_root.0.as_str()))
                })
            }
            DelegatedTaskScopeTargetKind::File => self
                .target_path
                .as_ref()
                .is_some_and(|target_file| target_path == Path::new(target_file.0.as_str())),
        }
    }
}
