use std::sync::atomic::{AtomicU64, Ordering};

use legion_app::AppComposition;
use legion_protocol::{
    BufferId, BufferVersion, CaretAffinity, PrincipalId, SnapshotId, TextCoordinate,
    VisualNavigationDirection, VisualNavigationLayoutId, VisualNavigationPosition,
    VisualNavigationRequest, VisualNavigationRow, VisualNavigationSourceRow, VisualNavigationStop,
    VisualNavigationX, WorkspaceTrustState,
};
use legion_ui::CommandDispatchIntent;

static TEMP_ROOT_COUNTER: AtomicU64 = AtomicU64::new(0);

struct TempRoot(std::path::PathBuf);

impl Drop for TempRoot {
    fn drop(&mut self) {
        let temp = std::env::temp_dir();
        if self.0.starts_with(temp)
            && self
                .0
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with("legion-vertical-navigation-"))
        {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
}

fn app_with_file() -> (TempRoot, AppComposition, legion_protocol::BufferId) {
    app_with_content("a\nb\nc\n", "vertical")
}

fn app_with_content(
    content: &str,
    name: &str,
) -> (TempRoot, AppComposition, legion_protocol::BufferId) {
    let root = std::env::temp_dir().join(format!(
        "legion-vertical-navigation-{}-{}",
        std::process::id(),
        TEMP_ROOT_COUNTER.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir(&root).expect("create temporary workspace");
    let path = root.join(format!("{name}.txt"));
    std::fs::write(&path, content).expect("seed temporary file");
    let mut app = AppComposition::new();
    app.open_workspace(
        &root,
        WorkspaceTrustState::Trusted,
        PrincipalId("vertical-navigation-test".to_string()),
    )
    .expect("open workspace");
    app.open_file(path.to_string_lossy()).expect("open file");
    let buffer_id = app.active_buffer_id().expect("active buffer");
    (TempRoot(root), app, buffer_id)
}

fn app_with_large_line() -> (TempRoot, AppComposition, legion_protocol::BufferId) {
    let root = std::env::temp_dir().join(format!(
        "legion-vertical-navigation-large-{}-{}",
        std::process::id(),
        TEMP_ROOT_COUNTER.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir(&root).expect("create large temporary workspace");
    let path = root.join("large.txt");
    std::fs::write(&path, "x".repeat(160 * 1024)).expect("seed large line");
    let mut app = AppComposition::new();
    app.open_workspace(
        &root,
        WorkspaceTrustState::Trusted,
        PrincipalId("vertical-navigation-large-test".to_string()),
    )
    .expect("open large workspace");
    app.open_file(path.to_string_lossy())
        .expect("open large file");
    let buffer_id = app.active_buffer_id().expect("large active buffer");
    (TempRoot(root), app, buffer_id)
}

fn pos(line: u32, byte_column: u64) -> VisualNavigationPosition {
    VisualNavigationPosition { line, byte_column }
}

fn row(line: u32) -> VisualNavigationRow {
    VisualNavigationRow {
        logical_line: line,
        row_index: Some(0),
        row_count: Some(1),
        start: pos(line, 0),
        end: pos(line, 1),
        stops: vec![
            VisualNavigationStop {
                position: pos(line, 0),
                x: VisualNavigationX { value: 0.0 },
                affinity: CaretAffinity::Upstream,
            },
            VisualNavigationStop {
                position: pos(line, 1),
                x: VisualNavigationX { value: 8.0 },
                affinity: CaretAffinity::Upstream,
            },
        ],
    }
}

fn request(app: &AppComposition, buffer_id: legion_protocol::BufferId) -> VisualNavigationRequest {
    let projection = app
        .visual_navigation_projection(buffer_id)
        .expect("projection");
    let carets = projection.carets.clone();
    VisualNavigationRequest {
        expected_snapshot_id: projection.snapshot_id,
        expected_buffer_version: projection.buffer_version,
        expected_carets: carets.clone(),
        layout_id: VisualNavigationLayoutId(1),
        direction: VisualNavigationDirection::Down,
        extend: false,
        source_rows: carets
            .iter()
            .map(|caret| VisualNavigationSourceRow {
                row: row(caret.head.line),
                source_x: VisualNavigationX { value: 0.0 },
            })
            .collect(),
        target_rows: carets
            .iter()
            .map(|caret| row(caret.head.line + 1))
            .collect(),
    }
}

#[test]
fn visual_navigation_projection_preserves_exact_caret_freshness() {
    let (_root, app, buffer_id) = app_with_file();
    let projection = app
        .visual_navigation_projection(buffer_id)
        .expect("visual navigation projection");
    assert_ne!(projection.snapshot_id, SnapshotId(0));
    assert_eq!(projection.carets.len(), 1);
    assert_eq!(projection.carets[0].head, pos(0, 0));
    assert_eq!(projection.logical_line_count, 4);
}

#[test]
fn visual_navigation_projection_reports_empty_document_line_count() {
    let (_root, app, buffer_id) = app_with_content("", "empty");
    let projection = app
        .visual_navigation_projection(buffer_id)
        .expect("visual navigation projection");
    assert_eq!(projection.logical_line_count, 1);
}

#[test]
fn visual_navigation_projection_reports_exact_trailing_newline_count() {
    let (_root, app, buffer_id) = app_with_content("a\nb\n", "trailing");
    let projection = app
        .visual_navigation_projection(buffer_id)
        .expect("visual navigation projection");
    assert_eq!(projection.logical_line_count, 3);
}

#[test]
fn invalid_visual_navigation_x_is_rejected_before_editor_mutation() {
    let (_root, mut app, buffer_id) = app_with_file();
    let before = app
        .visual_navigation_projection(buffer_id)
        .expect("before projection");
    let mut invalid = request(&app, buffer_id);
    invalid.source_rows[0].source_x.value = f32::NAN;
    let result = app.dispatch_ui_intent(CommandDispatchIntent::MoveVertically {
        buffer_id,
        request: invalid,
    });
    assert!(result.is_err());
    assert_eq!(
        app.visual_navigation_projection(buffer_id)
            .expect("after projection"),
        before
    );
}

#[test]
fn zero_layout_and_inactive_buffer_are_rejected_atomically() {
    let (_root, mut app, buffer_id) = app_with_file();
    let before = app
        .visual_navigation_projection(buffer_id)
        .expect("before projection");
    let mut invalid = request(&app, buffer_id);
    invalid.layout_id = VisualNavigationLayoutId(0);
    assert!(
        app.dispatch_ui_intent(CommandDispatchIntent::MoveVertically {
            buffer_id,
            request: invalid,
        })
        .is_err()
    );
    assert_eq!(
        app.visual_navigation_projection(buffer_id)
            .expect("after zero-layout projection"),
        before
    );

    let inactive = BufferId(u128::MAX);
    let inactive_request = request(&app, buffer_id);
    assert!(
        app.dispatch_ui_intent(CommandDispatchIntent::MoveVertically {
            buffer_id: inactive,
            request: inactive_request,
        })
        .is_err()
    );
}

#[test]
fn move_vertically_routes_two_ordered_carets_and_projects_preferred_x() {
    let (_root, mut app, buffer_id) = app_with_file();
    app.dispatch_ui_intent(CommandDispatchIntent::AddCursorBelow { buffer_id })
        .expect("add second caret");
    let before = app
        .visual_navigation_projection(buffer_id)
        .expect("before projection");
    assert_eq!(before.carets.len(), 2);
    let movement = request(&app, buffer_id);
    let result = app.dispatch_ui_intent(CommandDispatchIntent::MoveVertically {
        buffer_id,
        request: movement,
    });
    assert!(
        result.is_ok(),
        "valid vertical request must route: {result:?}"
    );
    let after = app
        .visual_navigation_projection(buffer_id)
        .expect("after projection");
    assert_eq!(after.carets.len(), 2);
    assert_eq!(after.carets[0].head.line, 1);
    assert_eq!(after.carets[1].head.line, 2);
    assert!(after.carets.iter().all(|caret| caret.preferred_x.is_some()));
}

#[test]
fn stale_snapshot_and_expected_carets_fail_without_state_change() {
    let (_root, mut app, buffer_id) = app_with_file();
    let stale = request(&app, buffer_id);
    app.dispatch_ui_intent(CommandDispatchIntent::Insert {
        buffer_id,
        at: TextCoordinate {
            line: 0,
            character: 0,
            byte_offset: None,
            utf16_offset: None,
        },
        text: "x".to_string(),
    })
    .expect("change buffer version");
    let before = app
        .visual_navigation_projection(buffer_id)
        .expect("current projection");
    assert!(
        app.dispatch_ui_intent(CommandDispatchIntent::MoveVertically {
            buffer_id,
            request: stale,
        })
        .is_err()
    );
    assert_eq!(
        app.visual_navigation_projection(buffer_id)
            .expect("projection after stale request"),
        before
    );

    let mut wrong_carets = request(&app, buffer_id);
    wrong_carets.expected_carets[0].head.byte_column = 1;
    let before_vector = app
        .visual_navigation_projection(buffer_id)
        .expect("before wrong vector");
    assert!(
        app.dispatch_ui_intent(CommandDispatchIntent::MoveVertically {
            buffer_id,
            request: wrong_carets,
        })
        .is_err()
    );
    assert_eq!(
        app.visual_navigation_projection(buffer_id)
            .expect("after wrong vector"),
        before_vector
    );
}

#[test]
fn stale_buffer_version_with_current_snapshot_is_rejected_atomically() {
    let (_root, mut app, buffer_id) = app_with_file();
    let mut stale_version = request(&app, buffer_id);
    let before = app
        .visual_navigation_projection(buffer_id)
        .expect("before version mismatch");
    let before_text = app.buffer_text_for_input(buffer_id).expect("before text");
    stale_version.expected_buffer_version = BufferVersion(before.buffer_version.0 + 1);
    assert!(
        app.dispatch_ui_intent(CommandDispatchIntent::MoveVertically {
            buffer_id,
            request: stale_version,
        })
        .is_err()
    );
    assert_eq!(
        app.visual_navigation_projection(buffer_id)
            .expect("after version mismatch"),
        before
    );
    assert_eq!(
        app.buffer_text_for_input(buffer_id).expect("after text"),
        before_text
    );
}

#[test]
fn invalid_late_target_and_coordinate_fail_without_partial_mutation() {
    let (_root, mut app, buffer_id) = app_with_file();
    app.dispatch_ui_intent(CommandDispatchIntent::AddCursorBelow { buffer_id })
        .expect("add second caret");
    let before = app
        .visual_navigation_projection(buffer_id)
        .expect("before invalid request");
    let mut invalid_target = request(&app, buffer_id);
    invalid_target.target_rows[1].stops.clear();
    assert!(
        app.dispatch_ui_intent(CommandDispatchIntent::MoveVertically {
            buffer_id,
            request: invalid_target,
        })
        .is_err()
    );
    assert_eq!(
        app.visual_navigation_projection(buffer_id)
            .expect("after invalid target"),
        before
    );

    let mut invalid_coordinate = request(&app, buffer_id);
    invalid_coordinate.source_rows[1].source_x.value = f32::INFINITY;
    assert!(
        app.dispatch_ui_intent(CommandDispatchIntent::MoveVertically {
            buffer_id,
            request: invalid_coordinate,
        })
        .is_err()
    );
    assert_eq!(
        app.visual_navigation_projection(buffer_id)
            .expect("after invalid coordinate"),
        before
    );

    let mut out_of_range = request(&app, buffer_id);
    out_of_range.source_rows[1].row.start.byte_column = u64::MAX;
    assert!(
        app.dispatch_ui_intent(CommandDispatchIntent::MoveVertically {
            buffer_id,
            request: out_of_range,
        })
        .is_err()
    );
    assert_eq!(
        app.visual_navigation_projection(buffer_id)
            .expect("after out-of-range coordinate"),
        before
    );
}

#[test]
fn inactive_real_second_buffer_is_rejected() {
    let (root, mut app, first_buffer) = app_with_file();
    let second_path = root.0.join("second.txt");
    std::fs::write(&second_path, "second\n").expect("seed second file");
    app.open_file(second_path.to_string_lossy())
        .expect("open second file");
    let second_buffer = app.active_buffer_id().expect("second active buffer");
    let request_for_first = request(&app, first_buffer);
    assert_ne!(first_buffer, second_buffer);
    assert!(
        app.dispatch_ui_intent(CommandDispatchIntent::MoveVertically {
            buffer_id: first_buffer,
            request: request_for_first,
        })
        .is_err()
    );
}

#[test]
fn app_window_projection_preserves_absolute_origin_and_grapheme_boundaries() {
    let (_root, app, buffer_id) = app_with_file();
    let window = app
        .visual_navigation_window(buffer_id, pos(1, 0), 96)
        .expect("bounded app window");
    assert_eq!(window.line, 1);
    assert_eq!(window.line_start_byte, 2);
    assert_eq!(window.caret_byte, 2);
    assert_eq!(window.start_byte, 2);
    assert!(window.end_byte >= window.caret_byte);
    assert!(window.grapheme_boundaries.contains(&window.caret_byte));
    assert_eq!(window.text, "b");
}

#[test]
fn app_window_projection_rejects_out_of_range_position_and_budget() {
    let (_root, app, buffer_id) = app_with_file();
    assert!(
        app.visual_navigation_window(buffer_id, pos(99, 0), 96)
            .is_err()
    );
    assert!(
        app.visual_navigation_window(buffer_id, pos(0, 0), 96 * 1024 + 1)
            .is_err()
    );
    assert!(
        app.visual_navigation_window(BufferId(u128::MAX), pos(0, 0), 96)
            .is_err()
    );
}

#[test]
fn app_window_projection_bounds_mid_large_line_without_full_materialization() {
    let (_root, app, buffer_id) = app_with_large_line();
    let window = app
        .visual_navigation_window(buffer_id, pos(0, 80 * 1024), 96)
        .expect("mid-line bounded window");
    assert_eq!(window.caret_byte, 80 * 1024);
    assert!(window.text.len() <= 96);
    assert!(window.start_byte < window.caret_byte);
    assert!(window.caret_byte < window.end_byte);
    assert!(!window.complete_logical_start);
    assert!(!window.complete_logical_end);
}
