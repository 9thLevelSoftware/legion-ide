//! P2.F3.T4 — cargo test discovery + per-item exact run for the test explorer.

use std::{
    fs,
    sync::atomic::{AtomicU64, Ordering},
};

use legion_app::test_explorer::{
    parse_cargo_test_list, parse_cargo_test_summary, projection_from_items, validate_test_item_id,
};
use legion_app::{AppCommandOutcome, AppComposition};
use legion_protocol::{PrincipalId, TimestampMillis, WorkspaceTrustState};
use legion_ui::CommandDispatchIntent;

static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

fn create_fixture_crate() -> std::path::PathBuf {
    let root = std::env::temp_dir().join(format!(
        "legion-test-explorer-{}-{}",
        std::process::id(),
        TEMP_COUNTER.fetch_add(1, Ordering::SeqCst)
    ));
    fs::create_dir_all(root.join("src")).expect("create src");
    fs::write(
        root.join("Cargo.toml"),
        r#"[package]
name = "legion_test_explorer_fixture"
version = "0.1.0"
edition = "2021"
"#,
    )
    .expect("write Cargo.toml");
    fs::write(
        root.join("src/lib.rs"),
        "#[cfg(test)]\nmod tests {\n    #[test]\n    fn fixture_ok() {}\n}\n",
    )
    .expect("write lib");
    root
}

#[test]
fn test_explorer_refresh_requires_workspace() {
    let mut app = AppComposition::new();
    let err = app
        .dispatch_ui_intent(CommandDispatchIntent::RefreshTestExplorer)
        .expect_err("must fail closed without workspace");
    let _ = format!("{err}");
}

#[test]
fn test_explorer_parse_and_projection_are_metadata_only() {
    let items =
        parse_cargo_test_list("crate::alpha: test\ncrate::beta: bench\n\n2 tests, 1 benchmarks\n");
    assert_eq!(items.len(), 2);
    let projection = projection_from_items(items, Vec::new(), "ready", TimestampMillis(7));
    assert_eq!(projection.schema_version, 1);
    assert_eq!(projection.controller_label, "cargo-test");
    assert_eq!(projection.items[0].item_id, "crate::alpha");
    assert!(projection.diagnostics.is_empty());
}

#[test]
fn test_explorer_refresh_discovers_fixture_or_reports_honest_error() {
    let root = create_fixture_crate();
    let mut app = AppComposition::new();
    app.open_workspace(
        &root,
        WorkspaceTrustState::Trusted,
        PrincipalId("principal-test-explorer".to_string()),
    )
    .expect("open workspace");

    let outcome = app
        .dispatch_ui_intent(CommandDispatchIntent::RefreshTestExplorer)
        .expect("refresh should not panic");
    match outcome {
        AppCommandOutcome::TestExplorerUpdated(projection) => {
            assert_eq!(projection.controller_label, "cargo-test");
            assert!(!projection.status_label.is_empty());
            assert!(projection.items.len() <= 500);
            if projection.status_label == "ready" {
                assert!(
                    projection
                        .items
                        .iter()
                        .any(|item| item.item_id.contains("fixture_ok")
                            || item.label.contains("fixture_ok")),
                    "expected fixture_ok among {:?}",
                    projection
                        .items
                        .iter()
                        .map(|i| i.item_id.as_str())
                        .collect::<Vec<_>>()
                );
            }
            let snap = app
                .shell_projection_snapshot("test-explorer")
                .expect("snapshot");
            assert_eq!(
                snap.test_explorer_projection.status_label,
                projection.status_label
            );
        }
        other => panic!("unexpected outcome: {other:?}"),
    }

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn test_explorer_run_rejects_invalid_item_id() {
    let root = create_fixture_crate();
    let mut app = AppComposition::new();
    app.open_workspace(
        &root,
        WorkspaceTrustState::Trusted,
        PrincipalId("principal-test-explorer-run".to_string()),
    )
    .expect("open workspace");
    let err = app
        .dispatch_ui_intent(CommandDispatchIntent::RunTestExplorerItem {
            item_id: "evil;rm -rf".to_string(),
        })
        .expect_err("must reject unsafe item ids");
    let msg = format!("{err}");
    assert!(msg.contains("invalid test item id"), "msg={msg}");
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn test_explorer_run_fixture_records_last_run_and_verification_row() {
    assert!(validate_test_item_id("tests::fixture_ok").is_ok());
    let (p, f, _, ok) = parse_cargo_test_summary(
        "test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out\n",
    );
    assert!(ok && p == 1 && f == 0);

    let root = create_fixture_crate();
    let mut app = AppComposition::new();
    app.open_workspace(
        &root,
        WorkspaceTrustState::Trusted,
        PrincipalId("principal-test-explorer-run2".to_string()),
    )
    .expect("open workspace");

    // Discover first so items exist; run may still work without list if id is known.
    let _ = app
        .dispatch_ui_intent(CommandDispatchIntent::RefreshTestExplorer)
        .expect("refresh");

    let item_id = "tests::fixture_ok".to_string();
    let outcome = app
        .dispatch_ui_intent(CommandDispatchIntent::RunTestExplorerItem {
            item_id: item_id.clone(),
        })
        .expect("run should not panic");
    match outcome {
        AppCommandOutcome::TestExplorerUpdated(projection) => {
            assert_eq!(
                projection.last_run_item_id.as_deref(),
                Some(item_id.as_str())
            );
            assert!(projection.last_run_status.is_some());
            // When cargo is available, fixture_ok should pass.
            if projection.last_run_status.as_deref() == Some("passed") {
                assert_eq!(projection.last_run_exit_code, Some(0));
            }
            let snap = app
                .shell_projection_snapshot("test-explorer-run")
                .expect("snapshot");
            assert!(
                snap.verification_run_projection
                    .rows
                    .iter()
                    .any(|row| row.command_class_label == "cargo-test-exact"
                        && row.target_labels.iter().any(|t| t == &item_id)),
                "expected verification row for exact run, got {:?}",
                snap.verification_run_projection.rows
            );
        }
        other => panic!("unexpected outcome: {other:?}"),
    }
    let _ = fs::remove_dir_all(&root);
}
