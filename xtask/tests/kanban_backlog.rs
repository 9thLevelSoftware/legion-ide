use std::{
    fs,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

use xtask::kanban_backlog::{
    BacklogCard, BacklogEpic, BacklogFeature, KanbanBacklog, KanbanBacklogValidationError,
    validate_backlog,
};

struct TempDir {
    root: PathBuf,
}

impl TempDir {
    fn new(name: &str) -> Self {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock before epoch")
            .as_nanos();
        let root = std::env::temp_dir().join(format!("legion-kanban-backlog-{name}-{stamp}"));
        fs::create_dir_all(&root).expect("create temp dir");
        Self { root }
    }

    fn write(&self, rel: &str, text: &str) -> PathBuf {
        let path = self.root.join(rel);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("create parent");
        }
        fs::write(&path, text).expect("write fixture file");
        path
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn minimal_valid_backlog_toml() -> &'static str {
    r#"
[meta]
plan = ".hermes/plans/2026-06-13_173122-legion-current-to-ga-kanban-plan.md"
milestone = "M0"

[[epics]]
id = "P0"
title = "Truth, taxonomy, and Kanban import"
milestone = "M0"
readiness_rows = ["all"]

[[epics.features]]
id = "P0.F1"
title = "Canonical mode taxonomy"

[[epics.features.tasks]]
id = "P0.F1.T1"
title = "Decide canonical v1 modes"
mode = "Manual"
readiness_row = "PR-UI-001"
files = ["docs/MODES.md"]
dependencies = []
verification = ["cargo test -p legion-protocol"]
acceptance = ["One mode table exists"]
stop_condition = "Manual mode policy still forbids AI"
"#
}

#[test]
fn parse_minimal_valid_backlog_succeeds() {
    let dir = TempDir::new("minimal-valid");
    let path = dir.write("backlog.toml", minimal_valid_backlog_toml());
    let backlog = KanbanBacklog::from_file(&path).expect("minimal valid backlog should parse");
    assert_eq!(
        backlog.meta.plan,
        ".hermes/plans/2026-06-13_173122-legion-current-to-ga-kanban-plan.md"
    );
    assert_eq!(backlog.meta.milestone, "M0");
    assert_eq!(backlog.epics.len(), 1);
    assert_eq!(backlog.epics[0].id, "P0");
    assert_eq!(backlog.epics[0].features.len(), 1);
    assert_eq!(backlog.epics[0].features[0].tasks.len(), 1);
    assert_eq!(backlog.epics[0].features[0].tasks[0].id, "P0.F1.T1");
}

#[test]
fn validate_passes_for_minimal_backlog() {
    let dir = TempDir::new("validate-minimal");
    let path = dir.write("backlog.toml", minimal_valid_backlog_toml());
    let backlog = KanbanBacklog::from_file(&path).expect("minimal valid backlog should parse");
    let result = validate_backlog(&backlog);
    assert!(
        result.is_ok(),
        "expected minimal backlog to validate, got: {:?}",
        result.err()
    );
}

#[test]
fn validate_rejects_missing_required_task_field() {
    // Drop the `acceptance` field from the only task.
    let toml_src =
        minimal_valid_backlog_toml().replace("acceptance = [\"One mode table exists\"]\n", "");
    let dir = TempDir::new("missing-acceptance");
    let path = dir.write("backlog.toml", &toml_src);
    let backlog = KanbanBacklog::from_file(&path)
        .expect("backlog with missing optional-shaped field should still parse");
    let err = validate_backlog(&backlog)
        .expect_err("validation should fail when a required task field is missing");
    match err {
        KanbanBacklogValidationError::MissingRequiredField { card_id, field } => {
            assert_eq!(card_id, "P0.F1.T1");
            assert_eq!(field, "acceptance");
        }
        other => panic!("unexpected error variant: {other:?}"),
    }
}

#[test]
fn validate_rejects_omitted_dependencies_field() {
    // Drop the `dependencies` field entirely. An omitted field must be
    // treated as a missing required field, not silently defaulted to empty.
    let toml_src = minimal_valid_backlog_toml().replace("dependencies = []\n", "");
    let dir = TempDir::new("omitted-dependencies");
    let path = dir.write("backlog.toml", &toml_src);
    let backlog = KanbanBacklog::from_file(&path)
        .expect("backlog with omitted dependencies should still parse");
    let err = validate_backlog(&backlog)
        .expect_err("validation should fail when the dependencies field is omitted");
    match err {
        KanbanBacklogValidationError::MissingRequiredField { card_id, field } => {
            assert_eq!(card_id, "P0.F1.T1");
            assert_eq!(field, "dependencies");
        }
        other => panic!("unexpected error variant: {other:?}"),
    }
}

#[test]
fn validate_accepts_present_empty_dependencies() {
    // An explicit empty list is present and therefore valid.
    let dir = TempDir::new("empty-dependencies");
    let path = dir.write("backlog.toml", minimal_valid_backlog_toml());
    let backlog = KanbanBacklog::from_file(&path).expect("backlog should parse");
    validate_backlog(&backlog).expect("present empty dependencies list should validate");
}

#[test]
fn validate_rejects_unknown_dependency() {
    // Add a dependency to a card id that does not exist anywhere in the backlog.
    let toml_src = minimal_valid_backlog_toml()
        .replace("dependencies = []", "dependencies = [\"P9.F99.T99\"]");
    let dir = TempDir::new("unknown-dep");
    let path = dir.write("backlog.toml", &toml_src);
    let backlog = KanbanBacklog::from_file(&path).expect("backlog should still parse");
    let err =
        validate_backlog(&backlog).expect_err("validation should fail when dependency is unknown");
    match err {
        KanbanBacklogValidationError::UnknownDependency {
            card_id,
            dependency,
        } => {
            assert_eq!(card_id, "P0.F1.T1");
            assert_eq!(dependency, "P9.F99.T99");
        }
        other => panic!("unexpected error variant: {other:?}"),
    }
}

#[test]
fn validate_rejects_duplicate_card_ids() {
    // Append a second task with the same id inside the same feature.
    let extra_task = r#"
[[epics.features.tasks]]
id = "P0.F1.T1"
title = "Duplicate id, should be rejected"
mode = "Manual"
readiness_row = "PR-UI-001"
files = []
dependencies = []
verification = []
acceptance = []
stop_condition = ""
"#;
    let combined = format!("{}{}", minimal_valid_backlog_toml(), extra_task);
    let dir = TempDir::new("duplicate-ids");
    let path = dir.write("backlog.toml", &combined);
    let backlog = KanbanBacklog::from_file(&path).expect("backlog should parse");
    let err = validate_backlog(&backlog).expect_err("validation should fail on duplicate ids");
    match err {
        KanbanBacklogValidationError::DuplicateId { id } => {
            assert_eq!(id, "P0.F1.T1");
        }
        other => panic!("unexpected error variant: {other:?}"),
    }
}

#[test]
fn collect_all_ids_returns_feature_and_task_ids() {
    // This is a light helper test to ensure we can index every card in the
    // backlog by id, which is what dependency validation needs.
    let backlog = KanbanBacklog {
        meta: xtask::kanban_backlog::BacklogMeta {
            plan: "p".to_string(),
            milestone: "M0".to_string(),
        },
        epics: vec![BacklogEpic {
            id: "P0".to_string(),
            title: "Truth".to_string(),
            milestone: "M0".to_string(),
            readiness_rows: vec!["all".to_string()],
            features: vec![BacklogFeature {
                id: "P0.F1".to_string(),
                title: "Mode taxonomy".to_string(),
                tasks: vec![BacklogCard {
                    id: "P0.F1.T1".to_string(),
                    title: "Decide modes".to_string(),
                    mode: "Manual".to_string(),
                    readiness_row: "PR-UI-001".to_string(),
                    files: vec![],
                    dependencies: Some(vec![]),
                    verification: vec![],
                    acceptance: vec![],
                    stop_condition: "n/a".to_string(),
                }],
            }],
        }],
    };

    let ids = backlog.collect_all_ids();
    // Epic, feature, and task ids are all indexed so any of them can be
    // referenced as a dependency in another card.
    assert!(ids.contains("P0"));
    assert!(ids.contains("P0.F1"));
    assert!(ids.contains("P0.F1.T1"));
    assert_eq!(ids.len(), 3);
}

#[test]
fn from_file_reports_read_error() {
    let dir = TempDir::new("missing-file");
    let path = dir.root.join("does-not-exist.toml");
    let err = KanbanBacklog::from_file(&path).expect_err("loading a missing file must fail");
    let msg = err.to_string();
    assert!(
        msg.contains("unable to read kanban backlog"),
        "unexpected error message: {msg}"
    );
}

#[test]
fn from_file_reports_parse_error() {
    let dir = TempDir::new("parse-error");
    let path = dir.write("backlog.toml", "this is = = not valid toml ===\n");
    let err = KanbanBacklog::from_file(&path).expect_err("malformed toml must fail to parse");
    let msg = err.to_string();
    assert!(
        msg.contains("unable to parse kanban backlog"),
        "unexpected error message: {msg}"
    );
}
