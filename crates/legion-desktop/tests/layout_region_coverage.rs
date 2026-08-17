//! Coverage gate for the workbench layout regions (P1.F2.T2).
//!
//! The dock/panel acceptance is "every layout region has a projection and an
//! integration test". Every region already had both, but only individually:
//! nothing enumerated the regions, so "every" was an unverified claim and a
//! region that lost its projection — or arrived with no coverage at all —
//! would fail no test.
//!
//! These tests close that hole. They match exhaustively over
//! [`LayoutRegion`], so a new region cannot be added without giving it a
//! projection source and naming the test that covers it.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use legion_desktop::view::DesktopProjectionViewModel;
use legion_protocol::{
    CanonicalPath, EventSequence, FileId, LanguageOutlineSymbolProjection,
    LanguageProblemProjection, ProtocolDiagnosticSeverity, RedactionHint,
    TerminalOutputRowProjection, TerminalSessionId, WorkspaceId,
};
use legion_ui::{
    ExplorerNodeProjection, LayoutRegion, Shell, ShellProjectionSnapshot,
    TestExplorerItemProjection,
};

/// Build a snapshot in which every layout region has something to project.
///
/// It starts from the empty shell rather than a hand-written literal so that a
/// new `ShellProjectionSnapshot` field cannot silently break this file, and so
/// the empty-shell negative case below is the exact same shape minus content.
fn populated_snapshot() -> ShellProjectionSnapshot {
    let mut snapshot = Shell::empty("Legion workspace").projection_snapshot();

    // Status bar: the bar reads the active buffer, so an identified,
    // file-backed buffer is what makes it non-empty.
    snapshot.active_buffer_projection.workspace_id = Some(WorkspaceId(1));
    snapshot.active_buffer_projection.buffer_id = Some(legion_protocol::BufferId(7));
    snapshot.active_buffer_projection.file_id = Some(FileId(3));
    snapshot.active_buffer_projection.file_path = Some(CanonicalPath("src/region.rs".to_string()));

    // File tree.
    snapshot.explorer_projection.nodes = vec![ExplorerNodeProjection {
        file_id: FileId(3),
        canonical_path: CanonicalPath("src/region.rs".to_string()),
        name: "region.rs".to_string(),
        children: Vec::new(),
        is_directory: false,
    }];

    // Editor tabs.
    snapshot.daily_editing_projection.tabs.tabs = vec![legion_ui::ui::EditorTabProjection {
        buffer_id: legion_protocol::BufferId(7),
        file_id: Some(FileId(3)),
        file_path: Some(CanonicalPath("src/region.rs".to_string())),
        title: "region.rs".to_string(),
        active: true,
        dirty: false,
        pinned: false,
        preview: false,
    }];
    snapshot.daily_editing_projection.tabs.active_buffer_id = Some(legion_protocol::BufferId(7));

    // Terminal panel.
    snapshot.terminal_panel_projection.output_rows = vec![TerminalOutputRowProjection {
        session_id: TerminalSessionId(1),
        sequence: EventSequence(1),
        redacted_payload: "cargo test".to_string(),
        byte_count: 10,
        is_stderr: false,
        truncated: false,
        redaction: RedactionHint::MetadataOnly,
        schema_version: 1,
    }];

    // Tests panel.
    snapshot.test_explorer_projection.status_label = "ready".to_string();
    snapshot.test_explorer_projection.items = vec![TestExplorerItemProjection {
        item_id: "legion_ui::projection::tests::layout_region_ids".to_string(),
        label: "layout_region_ids".to_string(),
        kind_label: "test".to_string(),
        parent_label: Some("legion_ui::projection::tests".to_string()),
        run_command_label: None,
    }];
    snapshot.test_explorer_projection.last_run_item_id =
        Some("legion_ui::projection::tests::layout_region_ids".to_string());
    snapshot.test_explorer_projection.last_run_status = Some("passed".to_string());

    // Problems panel.
    snapshot.language_tooling_projection.problems = vec![LanguageProblemProjection {
        file_id: Some(FileId(3)),
        path: Some(CanonicalPath("src/region.rs".to_string())),
        range: None,
        severity: ProtocolDiagnosticSeverity::Warning,
        code_label: Some("unused_variable".to_string()),
        message: "unused variable `region`".to_string(),
        source_label: Some("rust-analyzer".to_string()),
        redaction_hints: vec![RedactionHint::MetadataOnly],
        schema_version: 1,
    }];

    // Symbols panel.
    snapshot.language_tooling_projection.outline = vec![LanguageOutlineSymbolProjection {
        symbol_id: "outline:region".to_string(),
        label: "region".to_string(),
        kind_label: "function".to_string(),
        range: None,
        depth: 0,
        children_omitted: false,
        schema_version: 1,
    }];

    snapshot
}

/// The integration test that covers each region.
///
/// This is the "and an integration test" half of the acceptance, made
/// machine-checkable: the match is exhaustive, so a new region must name its
/// covering test, and [`every_layout_region_names_an_integration_test_that_exists`]
/// proves the named test is real rather than aspirational.
fn covering_integration_test(region: LayoutRegion) -> &'static str {
    match region {
        LayoutRegion::TopBar => {
            "projection_rendering_desktop_top_bar_uses_three_non_overlapping_regions"
        }
        LayoutRegion::StatusBar => "layout_region_status_bar_projects_active_file_metadata",
        LayoutRegion::Dock => "projection_rendering_uses_mode_filtered_dock_registry",
        LayoutRegion::FileTree => "projection_rendering_marks_expanded_and_collapsed_explorer_rows",
        LayoutRegion::EditorTabs => {
            "projection_rendering_editor_tabs_expose_tab_state_and_named_close_buttons"
        }
        LayoutRegion::TerminalPanel => {
            "terminal_panel_render_model_exposes_grid_status_and_scrollback"
        }
        LayoutRegion::TestsPanel => {
            "layout_region_tests_panel_projects_discovered_items_and_run_status"
        }
        LayoutRegion::ProblemsPanel => "diagnostic_problems_appear_in_language_rows",
        LayoutRegion::SymbolsPanel => {
            "projection_rendering_symbols_setup_and_settings_use_plain_copy_while_diagnostics_keeps_raw_rows"
        }
    }
}

/// Rows this region contributes to the desktop view model, when it has a field.
///
/// [`LayoutRegion::SymbolsPanel`] returns `None`: the outline is painted
/// straight from the snapshot by the renderer and never lands in
/// [`DesktopProjectionViewModel`], so its proof is the rendered-frame test
/// named above rather than a row assertion here. Returning `None` keeps that
/// exception explicit instead of hiding it behind an empty vector.
fn view_model_rows(
    region: LayoutRegion,
    model: &DesktopProjectionViewModel,
) -> Option<Vec<&String>> {
    let rows: Vec<&String> = match region {
        LayoutRegion::TopBar => model.top_bar_rows.iter().collect(),
        LayoutRegion::StatusBar => model.status_bar.path.iter().collect(),
        LayoutRegion::Dock => model
            .dock_rows
            .iter()
            .chain(model.dock_panel_rows.iter())
            .collect(),
        LayoutRegion::FileTree => model.explorer_rows.iter().collect(),
        LayoutRegion::EditorTabs => model.tab_rows.iter().collect(),
        LayoutRegion::TerminalPanel => model.terminal_rows.iter().collect(),
        LayoutRegion::TestsPanel => model.test_rows.iter().collect(),
        LayoutRegion::ProblemsPanel => model.language_rows.iter().collect(),
        LayoutRegion::SymbolsPanel => return None,
    };
    Some(rows)
}

/// Every `.rs` file under this crate's `tests/` directory, including `common/`.
///
/// Resolved from `CARGO_MANIFEST_DIR` rather than a literal path so the test
/// depends on the crate layout, not on the checkout location or the host OS.
fn desktop_test_sources() -> Vec<PathBuf> {
    fn collect(dir: &Path, out: &mut Vec<PathBuf>) {
        let Ok(entries) = fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                collect(&path, out);
            } else if path.extension().is_some_and(|ext| ext == "rs") {
                out.push(path);
            }
        }
    }

    let mut sources = Vec::new();
    collect(
        &Path::new(env!("CARGO_MANIFEST_DIR")).join("tests"),
        &mut sources,
    );
    sources
}

#[test]
fn every_layout_region_projects_content_in_a_populated_snapshot() {
    let snapshot = populated_snapshot();

    for region in LayoutRegion::ALL {
        assert!(
            region.projected_item_count(&snapshot) > 0,
            "{} ({}) projected nothing from a fully populated snapshot",
            region.as_str(),
            region.projection_source()
        );
    }
}

#[test]
fn every_content_backed_layout_region_projects_nothing_for_an_empty_shell() {
    // The negative half. Without it the positive assertion above would pass on
    // a counter that is non-zero regardless of what the snapshot contains.
    let snapshot = Shell::empty("").projection_snapshot();

    for region in LayoutRegion::ALL {
        let count = region.projected_item_count(&snapshot);
        if region.is_content_backed() {
            assert_eq!(
                count,
                0,
                "{} projected {count} items from an empty shell",
                region.as_str()
            );
        } else {
            assert!(
                count > 0,
                "{} is persistent chrome and must still project",
                region.as_str()
            );
        }
    }
}

#[test]
fn every_layout_region_with_a_view_model_field_projects_rows() {
    let populated = DesktopProjectionViewModel::from_snapshot(&populated_snapshot());
    let empty = DesktopProjectionViewModel::from_snapshot(&Shell::empty("").projection_snapshot());

    for region in LayoutRegion::ALL {
        let Some(rows) = view_model_rows(region, &populated) else {
            continue;
        };
        assert!(
            !rows.is_empty(),
            "{} reached the desktop view model with no rows",
            region.as_str()
        );

        // Negative case: a content-backed region must not carry the populated
        // shell's rows into an empty one, otherwise the assertion above proves
        // only that some constant string exists. Rows need not vanish — an
        // empty panel legitimately draws its own empty-state row, such as the
        // explorer's `<empty explorer>` — but none of the populated content may
        // survive.
        if region.is_content_backed() {
            let empty_rows = view_model_rows(region, &empty)
                .expect("regions with a view-model field keep it for every snapshot");
            let leaked: Vec<_> = empty_rows.iter().filter(|row| rows.contains(row)).collect();
            assert!(
                leaked.is_empty(),
                "{} carried populated rows into an empty shell: {leaked:?}",
                region.as_str()
            );
        }
    }
}

#[test]
fn every_layout_region_names_an_integration_test_that_exists() {
    let sources: Vec<String> = desktop_test_sources()
        .iter()
        .filter_map(|path| fs::read_to_string(path).ok())
        .collect();
    assert!(
        !sources.is_empty(),
        "no desktop test sources were found; the coverage check would pass vacuously"
    );

    let mut named = BTreeSet::new();
    for region in LayoutRegion::ALL {
        let test_name = covering_integration_test(region);
        assert!(
            sources
                .iter()
                .any(|source| source.contains(&format!("fn {test_name}("))),
            "{} names covering test `{test_name}`, which does not exist",
            region.as_str()
        );
        named.insert(test_name);
    }

    // A single test claimed as the cover for several regions would let one
    // region's coverage stand in for another's.
    assert_eq!(
        named.len(),
        LayoutRegion::ALL.len(),
        "each region must name its own covering test"
    );
}

#[test]
fn layout_region_status_bar_projects_active_file_metadata() {
    let model = DesktopProjectionViewModel::from_snapshot(&populated_snapshot());

    assert_eq!(model.status_bar.product_mode, "Manual");
    assert_eq!(model.status_bar.path.as_deref(), Some("src/region.rs"));
    assert_eq!(model.status_bar.encoding.as_deref(), Some("UTF-8"));
    assert_eq!(model.status_bar.language.as_deref(), Some("rust"));
    assert_eq!(model.status_bar.buffer_id, Some(7));
    assert_eq!(model.status_bar.file_id, Some(3));
    assert_eq!(model.status_bar.workspace_id, Some(1));

    // Negative case: with no active buffer the bar must report nothing rather
    // than keep the last file's metadata on screen.
    let empty = DesktopProjectionViewModel::from_snapshot(&Shell::empty("").projection_snapshot());
    assert_eq!(empty.status_bar.path, None);
    assert_eq!(empty.status_bar.encoding, None);
    assert_eq!(empty.status_bar.language, None);
    assert_eq!(empty.status_bar.buffer_id, None);
    assert_eq!(empty.status_bar.cursor, None);
}

#[test]
fn layout_region_tests_panel_projects_discovered_items_and_run_status() {
    let model = DesktopProjectionViewModel::from_snapshot(&populated_snapshot());

    assert!(
        model
            .test_rows
            .iter()
            .any(|row| row.contains("layout_region_ids")),
        "tests panel must project the discovered item label; rows={:?}",
        model.test_rows
    );
    assert!(
        model.test_rows.iter().any(|row| row.contains("passed")),
        "tests panel must project the last run status; rows={:?}",
        model.test_rows
    );

    // Negative case: an empty test explorer must not project discovery rows.
    let empty = DesktopProjectionViewModel::from_snapshot(&Shell::empty("").projection_snapshot());
    assert!(
        empty.test_rows.is_empty(),
        "tests panel projected rows with nothing discovered: {:?}",
        empty.test_rows
    );
}
