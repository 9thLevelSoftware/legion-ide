//! Machine-readable Kanban backlog model + validation.
//!
//! The backlog file is TOML so it stays small and uses the workspace
//! `toml` dependency that is already vendored by `xtask`. The schema is
//! flat enough to round-trip through plain `serde` derives; dependency
//! references are validated by walking every epic/feature/task and
//! looking the id up in the global id set.

use std::{
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
};

use serde::Deserialize;

const REQUIRED_TASK_FIELDS: &[&str] = &[
    "id",
    "title",
    "mode",
    "readiness_row",
    "files",
    "dependencies",
    "verification",
    "acceptance",
    "stop_condition",
];

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct BacklogMeta {
    pub plan: String,
    pub milestone: String,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct BacklogCard {
    pub id: String,
    pub title: String,
    pub mode: String,
    pub readiness_row: String,
    #[serde(default)]
    pub files: Vec<String>,
    #[serde(default)]
    pub dependencies: Vec<String>,
    #[serde(default)]
    pub verification: Vec<String>,
    #[serde(default)]
    pub acceptance: Vec<String>,
    #[serde(default)]
    pub stop_condition: String,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct BacklogFeature {
    pub id: String,
    pub title: String,
    #[serde(default)]
    pub tasks: Vec<BacklogCard>,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct BacklogEpic {
    pub id: String,
    pub title: String,
    pub milestone: String,
    #[serde(default)]
    pub readiness_rows: Vec<String>,
    #[serde(default)]
    pub features: Vec<BacklogFeature>,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct KanbanBacklog {
    pub meta: BacklogMeta,
    #[serde(default)]
    pub epics: Vec<BacklogEpic>,
}

impl KanbanBacklog {
    pub fn from_file(path: &Path) -> Result<Self, String> {
        let text = fs::read_to_string(path)
            .map_err(|err| format!("unable to read kanban backlog `{}`: {err}", path.display()))?;
        toml::from_str(&text)
            .map_err(|err| format!("unable to parse kanban backlog `{}`: {err}", path.display()))
    }

    /// Return every epic, feature, and task id in the backlog in a stable
    /// order. Used by dependency validation.
    pub fn collect_all_ids(&self) -> BTreeSet<String> {
        let mut ids = BTreeSet::new();
        for epic in &self.epics {
            ids.insert(epic.id.clone());
            for feature in &epic.features {
                ids.insert(feature.id.clone());
                for task in &feature.tasks {
                    ids.insert(task.id.clone());
                }
            }
        }
        ids
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KanbanBacklogValidationError {
    DuplicateId {
        id: String,
    },
    MissingRequiredField {
        card_id: String,
        field: &'static str,
    },
    UnknownDependency {
        card_id: String,
        dependency: String,
    },
}

impl std::fmt::Display for KanbanBacklogValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            KanbanBacklogValidationError::DuplicateId { id } => {
                write!(f, "duplicate card id `{id}` in backlog")
            }
            KanbanBacklogValidationError::MissingRequiredField { card_id, field } => {
                write!(f, "card `{card_id}` is missing required field `{field}`")
            }
            KanbanBacklogValidationError::UnknownDependency {
                card_id,
                dependency,
            } => write!(
                f,
                "card `{card_id}` declares unknown dependency `{dependency}`"
            ),
        }
    }
}

impl std::error::Error for KanbanBacklogValidationError {}

/// Validate the parsed backlog. Errors are returned on the first violation;
/// callers that need a full report can iterate card-by-card instead.
pub fn validate_backlog(backlog: &KanbanBacklog) -> Result<(), KanbanBacklogValidationError> {
    let mut seen: BTreeSet<String> = BTreeSet::new();

    // Index all ids first so we can check duplicates globally and have a
    // complete set for dependency lookups.
    let mut all_ids: BTreeSet<String> = BTreeSet::new();
    for epic in &backlog.epics {
        check_unique(epic.id.clone(), &mut seen)?;
        all_ids.insert(epic.id.clone());
        for feature in &epic.features {
            check_unique(feature.id.clone(), &mut seen)?;
            all_ids.insert(feature.id.clone());
            for task in &feature.tasks {
                check_unique(task.id.clone(), &mut seen)?;
                check_required_fields(task)?;
                all_ids.insert(task.id.clone());
            }
        }
    }

    // Now check every task's dependencies resolve to a real id.
    for epic in &backlog.epics {
        for feature in &epic.features {
            for task in &feature.tasks {
                for dep in &task.dependencies {
                    if !all_ids.contains(dep) {
                        return Err(KanbanBacklogValidationError::UnknownDependency {
                            card_id: task.id.clone(),
                            dependency: dep.clone(),
                        });
                    }
                }
            }
        }
    }

    Ok(())
}

fn check_unique(
    id: String,
    seen: &mut BTreeSet<String>,
) -> Result<(), KanbanBacklogValidationError> {
    if !seen.insert(id.clone()) {
        return Err(KanbanBacklogValidationError::DuplicateId { id });
    }
    Ok(())
}

fn check_required_fields(task: &BacklogCard) -> Result<(), KanbanBacklogValidationError> {
    for field in REQUIRED_TASK_FIELDS {
        let present = match *field {
            "id" => !task.id.trim().is_empty(),
            "title" => !task.title.trim().is_empty(),
            "mode" => !task.mode.trim().is_empty(),
            "readiness_row" => !task.readiness_row.trim().is_empty(),
            "files" => !task.files.is_empty(),
            "dependencies" => true, // Vec may be empty; presence of the field is sufficient.
            "verification" => !task.verification.is_empty(),
            "acceptance" => !task.acceptance.is_empty(),
            "stop_condition" => !task.stop_condition.trim().is_empty(),
            _ => false,
        };
        if !present {
            return Err(KanbanBacklogValidationError::MissingRequiredField {
                card_id: task.id.clone(),
                field,
            });
        }
    }
    Ok(())
}

/// Summary of a single validation pass, suitable for printing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KanbanBacklogSummary {
    pub epics: usize,
    pub features: usize,
    pub tasks: usize,
}

pub fn summarize(backlog: &KanbanBacklog) -> KanbanBacklogSummary {
    let mut features = 0;
    let mut tasks = 0;
    for epic in &backlog.epics {
        features += epic.features.len();
        for feature in &epic.features {
            tasks += feature.tasks.len();
        }
    }
    KanbanBacklogSummary {
        epics: backlog.epics.len(),
        features,
        tasks,
    }
}

/// Default path for the GA Kanban backlog file, repo-relative.
pub const DEFAULT_BACKLOG_PATH: &str = "plans/kanban/legion-ga-backlog.toml";

/// Validate the backlog file at `path`. Returns the summary on success.
pub fn run_verify_kanban_backlog(path: &Path) -> Result<KanbanBacklogSummary, String> {
    let backlog = KanbanBacklog::from_file(path)?;
    validate_backlog(&backlog).map_err(|err| err.to_string())?;
    Ok(summarize(&backlog))
}

/// Convenience for callers that want a strongly-typed path.
pub fn backlog_file_path(workspace_root: &Path) -> PathBuf {
    workspace_root.join(DEFAULT_BACKLOG_PATH)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn card(id: &str) -> BacklogCard {
        BacklogCard {
            id: id.to_string(),
            title: "T".to_string(),
            mode: "Manual".to_string(),
            readiness_row: "PR-UI-001".to_string(),
            files: vec!["docs/MODES.md".to_string()],
            dependencies: vec![],
            verification: vec!["cargo test".to_string()],
            acceptance: vec!["done".to_string()],
            stop_condition: "stop".to_string(),
        }
    }

    #[test]
    fn required_field_missing_title_is_reported() {
        let mut c = card("P0.F1.T1");
        c.title = "   ".to_string();
        let err = check_required_fields(&c).expect_err("empty title should fail");
        assert!(matches!(
            err,
            KanbanBacklogValidationError::MissingRequiredField { field: "title", .. }
        ));
    }

    #[test]
    fn required_field_empty_files_is_reported() {
        let mut c = card("P0.F1.T1");
        c.files.clear();
        let err = check_required_fields(&c).expect_err("empty files should fail");
        assert!(matches!(
            err,
            KanbanBacklogValidationError::MissingRequiredField { field: "files", .. }
        ));
    }
}
