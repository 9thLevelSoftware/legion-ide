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
status = "todo"
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
                    status: Some("todo".to_string()),
                    evidence: None,
                    external_unblock: None,
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

/// Swap the fixture's `status = "todo"` line for `block`, which may carry its
/// own status plus any of `evidence` / `external_unblock`.
fn with_status_block(block: &str) -> String {
    minimal_valid_backlog_toml().replace("status = \"todo\"", block)
}

const SAMPLE_EVIDENCE: &str = "plans/evidence/production/WS-P0/WS-P0-rebaseline-evidence.md";

#[test]
fn absent_status_is_rejected() {
    // Regression guard for the fail-open default: an omitted `status` used to
    // deserialize to `todo`, so 65 untriaged cards reported as not-started and
    // the gate stayed green. An omitted status is now a hard error.
    let toml_src = minimal_valid_backlog_toml().replace("status = \"todo\"\n", "");
    let dir = TempDir::new("status-absent");
    let path = dir.write("backlog.toml", &toml_src);
    let backlog = KanbanBacklog::from_file(&path).expect("backlog should parse");
    assert_eq!(backlog.epics[0].features[0].tasks[0].status, None);
    let err = validate_backlog(&backlog).expect_err("an omitted status must be rejected");
    match err {
        KanbanBacklogValidationError::MissingRequiredField { card_id, field } => {
            assert_eq!(card_id, "P0.F1.T1");
            assert_eq!(field, "status");
        }
        other => panic!("unexpected error variant: {other:?}"),
    }
}

#[test]
fn status_accepts_all_valid_values() {
    for status in ["todo", "in-progress", "done", "blocked"] {
        // Every status except `todo` asserts that work happened, so it must
        // carry evidence; `blocked` must additionally name its unblock. Supply
        // both so this test isolates the status vocabulary itself.
        let mut block = format!("status = \"{status}\"");
        if status != "todo" {
            block.push_str(&format!("\nevidence = \"{SAMPLE_EVIDENCE}\""));
        }
        if status == "blocked" {
            block.push_str("\nexternal_unblock = \"EXT-CERT-WIN\"");
        }
        let dir = TempDir::new(&format!("status-valid-{status}"));
        let path = dir.write("backlog.toml", &with_status_block(&block));
        let backlog = KanbanBacklog::from_file(&path).expect("backlog should parse");
        assert_eq!(
            backlog.epics[0].features[0].tasks[0].status.as_deref(),
            Some(status)
        );
        validate_backlog(&backlog)
            .unwrap_or_else(|err| panic!("status `{status}` should validate, got: {err}"));
    }
}

#[test]
fn status_rejects_invalid_value() {
    let dir = TempDir::new("status-invalid");
    let path = dir.write("backlog.toml", &with_status_block("status = \"shipped\""));
    let backlog = KanbanBacklog::from_file(&path).expect("backlog should parse");
    let err = validate_backlog(&backlog).expect_err("invalid status value should be rejected");
    match err {
        KanbanBacklogValidationError::InvalidStatus { card_id, status } => {
            assert_eq!(card_id, "P0.F1.T1");
            assert_eq!(status, "shipped");
        }
        other => panic!("unexpected error variant: {other:?}"),
    }
}

#[test]
fn statuses_asserting_work_require_evidence() {
    // `done`, `in-progress`, and `blocked` all claim someone looked at the
    // card, so all three must say what they looked at.
    for status in ["done", "in-progress", "blocked"] {
        let dir = TempDir::new(&format!("no-evidence-{status}"));
        let path = dir.write(
            "backlog.toml",
            &with_status_block(&format!("status = \"{status}\"")),
        );
        let backlog = KanbanBacklog::from_file(&path).expect("backlog should parse");
        let err = validate_backlog(&backlog)
            .expect_err(&format!("`{status}` without evidence must be rejected"));
        match err {
            KanbanBacklogValidationError::MissingEvidenceForStatus {
                card_id,
                status: reported,
            } => {
                assert_eq!(card_id, "P0.F1.T1");
                assert_eq!(reported, status);
            }
            other => panic!("unexpected error variant for `{status}`: {other:?}"),
        }
    }
}

#[test]
fn done_status_with_blank_evidence_is_rejected() {
    let dir = TempDir::new("done-blank-evidence");
    let path = dir.write(
        "backlog.toml",
        &with_status_block("status = \"done\"\nevidence = \"   \""),
    );
    let backlog = KanbanBacklog::from_file(&path).expect("backlog should parse");
    let err = validate_backlog(&backlog)
        .expect_err("a task marked done with blank evidence must be rejected");
    match err {
        KanbanBacklogValidationError::MissingEvidenceForStatus { card_id, status } => {
            assert_eq!(card_id, "P0.F1.T1");
            assert_eq!(status, "done");
        }
        other => panic!("unexpected error variant: {other:?}"),
    }
}

#[test]
fn done_status_with_evidence_is_accepted() {
    let dir = TempDir::new("done-with-evidence");
    let path = dir.write(
        "backlog.toml",
        &with_status_block(&format!(
            "status = \"done\"\nevidence = \"{SAMPLE_EVIDENCE}\""
        )),
    );
    let backlog = KanbanBacklog::from_file(&path).expect("backlog should parse");
    validate_backlog(&backlog).expect("done status with non-empty evidence should validate");
    assert_eq!(
        backlog.epics[0].features[0].tasks[0].evidence.as_deref(),
        Some(SAMPLE_EVIDENCE)
    );
}

#[test]
fn evidence_on_a_todo_card_is_allowed() {
    // Evidence is never *forbidden* — a not-yet-started card may still point at
    // partial findings without tripping validation.
    let dir = TempDir::new("evidence-on-todo");
    let path = dir.write(
        "backlog.toml",
        &with_status_block(&format!(
            "status = \"todo\"\nevidence = \"{SAMPLE_EVIDENCE}\""
        )),
    );
    let backlog = KanbanBacklog::from_file(&path).expect("backlog should parse");
    validate_backlog(&backlog).expect("evidence on a todo card should still validate");
}

#[test]
fn blocked_status_without_external_unblock_is_rejected() {
    let dir = TempDir::new("blocked-no-unblock");
    let path = dir.write(
        "backlog.toml",
        &with_status_block(&format!(
            "status = \"blocked\"\nevidence = \"{SAMPLE_EVIDENCE}\""
        )),
    );
    let backlog = KanbanBacklog::from_file(&path).expect("backlog should parse");
    let err = validate_backlog(&backlog)
        .expect_err("a blocked task must name the external gate holding it up");
    match err {
        KanbanBacklogValidationError::MissingExternalUnblock { card_id } => {
            assert_eq!(card_id, "P0.F1.T1");
        }
        other => panic!("unexpected error variant: {other:?}"),
    }
}

#[test]
fn unknown_external_unblock_is_rejected() {
    let dir = TempDir::new("unknown-unblock");
    let path = dir.write(
        "backlog.toml",
        &with_status_block("status = \"todo\"\nexternal_unblock = \"EXT-SOMEDAY\""),
    );
    let backlog = KanbanBacklog::from_file(&path).expect("backlog should parse");
    let err = validate_backlog(&backlog).expect_err("an unknown external unblock must be rejected");
    match err {
        KanbanBacklogValidationError::UnknownExternalUnblock {
            card_id,
            external_unblock,
        } => {
            assert_eq!(card_id, "P0.F1.T1");
            assert_eq!(external_unblock, "EXT-SOMEDAY");
        }
        other => panic!("unexpected error variant: {other:?}"),
    }
}
