//! User-journey view-model integration tests.
//!
//! These tests prove the desktop projection pipeline correctly maps
//! `ShellProjectionSnapshot` data through `DesktopProjectionViewModel::from_snapshot()`
//! into testable display models. The tests cover syntax highlights, diagnostic rows,
//! terminal rows, tab state, and git status — the full user-journey projection stages.
//!
//! Pattern: build `ShellProjectionSnapshot` inline, call `from_snapshot()`, assert on
//! the resulting view-model fields. No private `render_*` functions or `Color32` values
//! are accessed — only the public projection pipeline that feeds the renderer.

use legion_desktop::view::DesktopProjectionViewModel;
use legion_protocol::{
    BufferId, BufferVersion, ByteRange, CanonicalPath, EventSequence, FileFingerprint, FileId,
    LanguageProblemProjection, LineWrappingPolicy, ProtocolDiagnosticSeverity, ProtocolTextRange,
    RedactionHint, SnapshotId, TerminalOutputRowProjection, TerminalPanelProjection,
    TerminalPanelStatus, TerminalPanelStatusKind, TerminalScrollbackProjection,
    TerminalSearchProjection, TerminalSessionId, TextCoordinate, TimestampMillis, Utf16Position,
    Utf16Range, ViewportDimensions, ViewportLineSlice, ViewportLineTruncationState,
    ViewportProjection, ViewportProjectionMode, ViewportScroll, ViewportSemanticTokenKind,
    ViewportSemanticTokenOverlay, WorkspaceId,
};
use legion_ui::ui::{DailyEditingProjection, EditorTabProjection, EditorTabsProjection};
use legion_ui::{
    ActiveBufferProjection, ActiveBufferProjectionState, GitDiffStrategyProjection,
    GitFileProjection, Shell,
};

fn coord(line: u32, character: u32, byte_offset: u64) -> TextCoordinate {
    TextCoordinate {
        line,
        character,
        byte_offset: Some(byte_offset),
        utf16_offset: Some(byte_offset),
    }
}

fn range(start: u64, end: u64) -> ProtocolTextRange {
    ProtocolTextRange {
        start: coord(0, start as u32, start),
        end: coord(0, end as u32, end),
    }
}

fn fingerprint(value: &str) -> FileFingerprint {
    FileFingerprint {
        algorithm: "sha256".to_string(),
        value: value.to_string(),
    }
}

// ---------------------------------------------------------------------------
// Test 1: Syntax highlights map to correct token kinds
// ---------------------------------------------------------------------------

#[test]
fn syntax_highlights_map_to_correct_token_kinds() {
    let mut snapshot = Shell::empty("HighlightJourney").projection_snapshot();
    snapshot.active_buffer_projection = ActiveBufferProjection {
        state: ActiveBufferProjectionState::Full,
        workspace_id: Some(WorkspaceId(1)),
        buffer_id: Some(BufferId(10)),
        file_id: Some(FileId(5)),
        file_path: Some(CanonicalPath("src/main.rs".to_string())),
        viewport: Some(ViewportProjection {
            workspace_id: WorkspaceId(1),
            buffer_id: BufferId(10),
            file_id: Some(FileId(5)),
            snapshot_id: SnapshotId(1),
            buffer_version: BufferVersion(1),
            visible_range: range(0, 40),
            selections: Vec::new(),
            cursor: coord(0, 0, 0),
            cursors: vec![coord(0, 0, 0)],
            scroll: ViewportScroll {
                top_line: 0,
                left_column: 0,
            },
            dimensions: ViewportDimensions {
                width_px: 800,
                height_px: 600,
            },
            line_wrapping_policy: LineWrappingPolicy::Off,
            wrap_column: None,
            mode: ViewportProjectionMode::Normal,
            line_slices: vec![
                ViewportLineSlice {
                    line_number: 0,
                    visible_text: "fn main() {".to_string(),
                    byte_range: ByteRange::new(0, 11),
                    utf16_range: Utf16Range {
                        start: Utf16Position {
                            line: 0,
                            character: 0,
                        },
                        end: Utf16Position {
                            line: 0,
                            character: 11,
                        },
                    },
                    chunk_hash: fingerprint("chunk-0"),
                    truncation_state: ViewportLineTruncationState::None,
                },
                ViewportLineSlice {
                    line_number: 1,
                    visible_text: "    let x = 42;".to_string(),
                    byte_range: ByteRange::new(12, 27),
                    utf16_range: Utf16Range {
                        start: Utf16Position {
                            line: 1,
                            character: 0,
                        },
                        end: Utf16Position {
                            line: 1,
                            character: 15,
                        },
                    },
                    chunk_hash: fingerprint("chunk-1"),
                    truncation_state: ViewportLineTruncationState::None,
                },
                ViewportLineSlice {
                    line_number: 2,
                    visible_text: "}".to_string(),
                    byte_range: ByteRange::new(28, 29),
                    utf16_range: Utf16Range {
                        start: Utf16Position {
                            line: 2,
                            character: 0,
                        },
                        end: Utf16Position {
                            line: 2,
                            character: 1,
                        },
                    },
                    chunk_hash: fingerprint("chunk-2"),
                    truncation_state: ViewportLineTruncationState::None,
                },
            ],
            line_metrics: Vec::new(),
            decoration_spans: Vec::new(),
            fold_ranges: Vec::new(),
            semantic_token_overlays: vec![
                ViewportSemanticTokenOverlay {
                    line_number: 0,
                    start_col: 0,
                    end_col: 2,
                    kind: ViewportSemanticTokenKind::Keyword,
                },
                ViewportSemanticTokenOverlay {
                    line_number: 0,
                    start_col: 3,
                    end_col: 7,
                    kind: ViewportSemanticTokenKind::Function,
                },
                ViewportSemanticTokenOverlay {
                    line_number: 1,
                    start_col: 4,
                    end_col: 7,
                    kind: ViewportSemanticTokenKind::Keyword,
                },
                ViewportSemanticTokenOverlay {
                    line_number: 1,
                    start_col: 12,
                    end_col: 14,
                    kind: ViewportSemanticTokenKind::Number,
                },
            ],
            large_file_status: None,
            schema_version: 2,
        }),
        degraded: false,
        small_buffer_preview: None,
        dirty: false,
    };

    let model = DesktopProjectionViewModel::from_snapshot(&snapshot);

    // Three visible lines should produce three code-line view models.
    assert_eq!(model.active_buffer_code_lines.len(), 3);

    // Line 0: "fn main() {" — has Keyword at 0..2 and Function at 3..7.
    assert_eq!(model.active_buffer_code_lines[0].number, 1);
    assert_eq!(model.active_buffer_code_lines[0].text, "fn main() {");
    assert!(
        model.active_buffer_code_lines[0]
            .highlights
            .iter()
            .any(|span| {
                span.start_col == 0
                    && span.end_col == 2
                    && span.kind == ViewportSemanticTokenKind::Keyword
            }),
        "Expected Keyword span at 0..2 on line 0"
    );
    assert!(
        model.active_buffer_code_lines[0]
            .highlights
            .iter()
            .any(|span| {
                span.start_col == 3
                    && span.end_col == 7
                    && span.kind == ViewportSemanticTokenKind::Function
            }),
        "Expected Function span at 3..7 on line 0"
    );

    // Line 1: "    let x = 42;" — has Keyword at 4..7 and Number at 12..14.
    assert_eq!(model.active_buffer_code_lines[1].number, 2);
    assert_eq!(model.active_buffer_code_lines[1].text, "    let x = 42;");
    assert!(
        model.active_buffer_code_lines[1]
            .highlights
            .iter()
            .any(|span| {
                span.start_col == 4
                    && span.end_col == 7
                    && span.kind == ViewportSemanticTokenKind::Keyword
            }),
        "Expected Keyword span at 4..7 on line 1"
    );
    assert!(
        model.active_buffer_code_lines[1]
            .highlights
            .iter()
            .any(|span| {
                span.start_col == 12
                    && span.end_col == 14
                    && span.kind == ViewportSemanticTokenKind::Number
            }),
        "Expected Number span at 12..14 on line 1"
    );

    // Line 2: "}" — no highlights expected.
    assert_eq!(model.active_buffer_code_lines[2].number, 3);
    assert_eq!(model.active_buffer_code_lines[2].text, "}");
    assert!(
        model.active_buffer_code_lines[2].highlights.is_empty(),
        "Expected no highlight spans on closing brace line"
    );

    // Verify the active_buffer_lines text representation also contains the source.
    assert!(
        model
            .active_buffer_lines
            .iter()
            .any(|row| row.contains("fn main")),
        "active_buffer_lines should contain the source text"
    );
}

// ---------------------------------------------------------------------------
// Test 2: Diagnostic problems appear in language_rows
// ---------------------------------------------------------------------------

#[test]
fn diagnostic_problems_appear_in_language_rows() {
    let mut snapshot = Shell::empty("DiagnosticJourney").projection_snapshot();
    snapshot.language_tooling_projection.problems = vec![
        LanguageProblemProjection {
            file_id: Some(FileId(3)),
            path: Some(CanonicalPath("src/lib.rs".to_string())),
            range: Some(range(10, 20)),
            severity: ProtocolDiagnosticSeverity::Error,
            code_label: Some("E0308".to_string()),
            message: "mismatched types".to_string(),
            source_label: Some("rustc".to_string()),
            redaction_hints: vec![RedactionHint::MetadataOnly],
            schema_version: 1,
        },
        LanguageProblemProjection {
            file_id: Some(FileId(4)),
            path: Some(CanonicalPath("src/main.rs".to_string())),
            range: None,
            severity: ProtocolDiagnosticSeverity::Warning,
            code_label: None,
            message: "unused variable".to_string(),
            source_label: None,
            redaction_hints: vec![RedactionHint::MetadataOnly],
            schema_version: 1,
        },
    ];

    let model = DesktopProjectionViewModel::from_snapshot(&snapshot);

    // The language_rows should contain the diagnostic problem text.
    assert!(
        model
            .language_rows
            .iter()
            .any(|row| row.contains("mismatched types")),
        "language_rows should contain the first problem message"
    );
    assert!(
        model
            .language_rows
            .iter()
            .any(|row| row.contains("unused variable")),
        "language_rows should contain the second problem message"
    );
    // Verify severity is surfaced.
    assert!(
        model
            .language_rows
            .iter()
            .any(|row| row.contains("Error") && row.contains("E0308")),
        "language_rows should contain severity and code for Error diagnostic"
    );
    assert!(
        model
            .language_rows
            .iter()
            .any(|row| row.contains("Warning")),
        "language_rows should contain Warning severity"
    );
    // Verify file path context appears.
    assert!(
        model
            .language_rows
            .iter()
            .any(|row| row.contains("src/lib.rs")),
        "language_rows should contain the file path for problems"
    );
}

// ---------------------------------------------------------------------------
// Test 3: Terminal session content appears in terminal_rows
// ---------------------------------------------------------------------------

#[test]
fn terminal_session_content_appears_in_terminal_rows() {
    let mut snapshot = Shell::empty("TerminalJourney").projection_snapshot();
    snapshot.terminal_panel_projection = TerminalPanelProjection {
        workspace_id: Some(WorkspaceId(1)),
        active_session_id: Some(TerminalSessionId(42)),
        runtime_state: None,
        status: TerminalPanelStatus {
            kind: TerminalPanelStatusKind::Running,
            message: "Terminal session active".to_string(),
        },
        policy: None,
        output_rows: vec![
            TerminalOutputRowProjection {
                session_id: TerminalSessionId(42),
                sequence: EventSequence(1),
                redacted_payload: "cargo build --release".to_string(),
                byte_count: 21,
                is_stderr: false,
                truncated: false,
                redaction: RedactionHint::MetadataOnly,
                schema_version: 1,
            },
            TerminalOutputRowProjection {
                session_id: TerminalSessionId(42),
                sequence: EventSequence(2),
                redacted_payload: "Compiling legion v0.1.0".to_string(),
                byte_count: 23,
                is_stderr: false,
                truncated: false,
                redaction: RedactionHint::MetadataOnly,
                schema_version: 1,
            },
        ],
        scrollback: TerminalScrollbackProjection {
            visible_row_count: 2,
            omitted_row_count: 0,
            byte_limit: 65536,
            truncated: false,
            schema_version: 1,
        },
        search: TerminalSearchProjection {
            query_label: None,
            match_count: 0,
            active_match_index: None,
            truncated: false,
            schema_version: 1,
        },
        last_error: None,
        last_denial: None,
        cell_grid: None,
        cell_scrollback: None,
        cursor_row: None,
        cursor_col: None,
        cursor_visible: None,
        generated_at: TimestampMillis(100),
        redaction_hints: vec![RedactionHint::MetadataOnly],
        schema_version: 1,
    };

    let model = DesktopProjectionViewModel::from_snapshot(&snapshot);

    // Terminal rows should contain the terminal status summary.
    assert!(
        model
            .terminal_rows
            .iter()
            .any(|row| row.contains("Running")),
        "terminal_rows should contain the Running status kind"
    );
    // Terminal rows should contain the output payloads.
    assert!(
        model
            .terminal_rows
            .iter()
            .any(|row| row.contains("cargo build --release")),
        "terminal_rows should contain the first output payload"
    );
    assert!(
        model
            .terminal_rows
            .iter()
            .any(|row| row.contains("Compiling legion v0.1.0")),
        "terminal_rows should contain the second output payload"
    );
    // Verify session ID is surfaced in the summary row.
    assert!(
        model
            .terminal_rows
            .iter()
            .any(|row| row.contains("42")),
        "terminal_rows should reference the session ID"
    );
}

// ---------------------------------------------------------------------------
// Test 4: Tab strip reflects active/dirty/clean state
// ---------------------------------------------------------------------------

#[test]
fn tab_strip_reflects_active_dirty_clean_state() {
    let mut snapshot = Shell::empty("TabJourney").projection_snapshot();
    snapshot.daily_editing_projection = DailyEditingProjection {
        tabs: EditorTabsProjection {
            tabs: vec![
                EditorTabProjection {
                    buffer_id: BufferId(1),
                    file_id: Some(FileId(10)),
                    file_path: Some(CanonicalPath("src/main.rs".to_string())),
                    title: "main.rs".to_string(),
                    active: true,
                    dirty: true,
                    pinned: false,
                    preview: false,
                },
                EditorTabProjection {
                    buffer_id: BufferId(2),
                    file_id: Some(FileId(11)),
                    file_path: Some(CanonicalPath("src/lib.rs".to_string())),
                    title: "lib.rs".to_string(),
                    active: false,
                    dirty: false,
                    pinned: true,
                    preview: false,
                },
                EditorTabProjection {
                    buffer_id: BufferId(3),
                    file_id: Some(FileId(12)),
                    file_path: Some(CanonicalPath("Cargo.toml".to_string())),
                    title: "Cargo.toml".to_string(),
                    active: false,
                    dirty: false,
                    pinned: false,
                    preview: true,
                },
            ],
            active_buffer_id: Some(BufferId(1)),
        },
        close_dirty_prompt: None,
        viewport_states: Vec::new(),
        session_record: None,
    };

    let model = DesktopProjectionViewModel::from_snapshot(&snapshot);

    // Verify the tab strip has three entries.
    assert_eq!(
        model.tab_rows.len(),
        3,
        "tab_rows should have one entry per tab"
    );

    // Active dirty tab: "*" prefix and "+" dirty marker.
    assert!(
        model.tab_rows[0].contains("*") && model.tab_rows[0].contains("main.rs +"),
        "First tab should be active (*) and dirty (+): got {:?}",
        model.tab_rows[0]
    );

    // Inactive clean pinned tab: space prefix and "pinned" marker.
    assert!(
        model.tab_rows[1].starts_with(" ") && model.tab_rows[1].contains("pinned"),
        "Second tab should be inactive and pinned: got {:?}",
        model.tab_rows[1]
    );
    assert!(
        !model.tab_rows[1].contains("+"),
        "Second tab should not be dirty: got {:?}",
        model.tab_rows[1]
    );

    // Inactive clean preview tab: "preview" marker.
    assert!(
        model.tab_rows[2].contains("preview"),
        "Third tab should be preview: got {:?}",
        model.tab_rows[2]
    );
    assert!(
        !model.tab_rows[2].contains("*"),
        "Third tab should not be active: got {:?}",
        model.tab_rows[2]
    );
}

// ---------------------------------------------------------------------------
// Test 5: Git status appears in git_rows
// ---------------------------------------------------------------------------

#[test]
fn git_status_appears_in_git_rows() {
    let mut snapshot = Shell::empty("GitJourney").projection_snapshot();
    snapshot.git_projection.root_label = Some("/workspace".to_string());
    snapshot.git_projection.branch_label = Some("feature/user-journey".to_string());
    snapshot.git_projection.head_short = Some("abc1234".to_string());
    snapshot.git_projection.changed_files = vec![
        GitFileProjection {
            path: "src/main.rs".to_string(),
            status: "M ".to_string(),
            inserted_lines: 5,
            deleted_lines: 2,
            unstaged_hunk_count: 1,
            staged_hunk_count: 0,
            stageable: true,
            diff_strategy: GitDiffStrategyProjection::Syntactic,
            fallback_reason: None,
            conflict: false,
        },
        GitFileProjection {
            path: "src/new_file.rs".to_string(),
            status: "??".to_string(),
            inserted_lines: 30,
            deleted_lines: 0,
            unstaged_hunk_count: 0,
            staged_hunk_count: 0,
            stageable: false,
            diff_strategy: GitDiffStrategyProjection::LineFallback,
            fallback_reason: None,
            conflict: false,
        },
    ];

    let model = DesktopProjectionViewModel::from_snapshot(&snapshot);

    // Git rows should contain the branch and HEAD summary.
    assert!(
        model
            .git_rows
            .iter()
            .any(|row| row.contains("feature/user-journey")),
        "git_rows should contain the branch label"
    );
    assert!(
        model
            .git_rows
            .iter()
            .any(|row| row.contains("abc1234")),
        "git_rows should contain the HEAD short hash"
    );
    // Git rows should contain file change entries.
    assert!(
        model
            .git_rows
            .iter()
            .any(|row| row.contains("src/main.rs")),
        "git_rows should contain the modified file path"
    );
    assert!(
        model
            .git_rows
            .iter()
            .any(|row| row.contains("src/new_file.rs")),
        "git_rows should contain the untracked file path"
    );
    // Verify changed file count is in the summary row.
    assert!(
        model
            .git_rows
            .iter()
            .any(|row| row.contains("changes=2")),
        "git_rows summary should report 2 changes"
    );
}

// ---------------------------------------------------------------------------
// Test 6: Empty shell produces no panic
// ---------------------------------------------------------------------------

#[test]
fn empty_shell_produces_no_panic() {
    let snapshot = Shell::empty("Manual").projection_snapshot();
    let model = DesktopProjectionViewModel::from_snapshot(&snapshot);

    // The model should be constructable without panics. Basic sanity checks:
    assert_eq!(model.layout_title, "Manual");
    assert!(
        model.active_buffer_code_lines.is_empty(),
        "Empty shell should have no code lines"
    );
    assert!(
        model.tab_rows.iter().any(|row| row.contains("<no open tabs>")),
        "Empty shell should show no-tabs placeholder"
    );
}
