use super::source_control::git_relative_path;
use super::tab_strip::adjusted_tab_drop_target;
use super::*;
use legion_protocol::{
    CapabilityId, DelegatedTaskToolPermissionDecision, DelegatedTaskToolPermissionProfile,
    DelegatedTaskToolPermissionRequestInput, PermissionBudgetActionClass, RedactionHint,
    TerminalOutputRowProjection, TextCoordinate, delegated_task_tool_permission_request,
};
use legion_ui::{GitBlameLineProjection, GitHunkProjection, GitHunkStageProjection, Shell};

#[test]
fn rendered_reference_button_activates_exact_location_and_invalid_rows_do_not() {
    let context = egui::Context::default();
    context.enable_accesskit();
    let references = vec![
        LanguageLocationProjection {
            location_id: "valid".to_owned(),
            file_id: None,
            path: Some(CanonicalPath("src/lib.rs".to_owned())),
            range: Some(ProtocolTextRange {
                start: TextCoordinate {
                    line: 6,
                    character: 3,
                    byte_offset: None,
                    utf16_offset: None,
                },
                end: TextCoordinate {
                    line: 6,
                    character: 8,
                    byte_offset: None,
                    utf16_offset: None,
                },
            }),
            label: "valid reference".to_owned(),
            degraded: false,
            schema_version: 1,
        },
        LanguageLocationProjection {
            location_id: "invalid".to_owned(),
            file_id: None,
            path: None,
            range: None,
            label: "unavailable reference".to_owned(),
            degraded: true,
            schema_version: 1,
        },
    ];
    let mut actions = Vec::new();
    let first = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(800.0, 600.0),
        )),
        ..egui::RawInput::default()
    };
    let first_output = context.run_ui(first, |outer_ui| {
        egui::Area::new("reference_test_host".into()).show(outer_ui.ctx(), |ui| {
            render_reference_panel(ui, &references, &mut actions);
        });
    });
    let node = first_output
        .platform_output
        .accesskit_update
        .as_ref()
        .expect("reference accessibility tree")
        .nodes
        .iter()
        .find_map(|(id, node)| {
            (node.label() == Some("valid reference  src/lib.rs:7:4")
                && node.supports_action(egui::accesskit::Action::Click))
            .then_some(*id)
        })
        .expect("valid reference should be an accessible button");
    let click = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(800.0, 600.0),
        )),
        events: vec![egui::Event::AccessKitActionRequest(
            egui::accesskit::ActionRequest {
                action: egui::accesskit::Action::Click,
                target_tree: egui::accesskit::TreeId::ROOT,
                target_node: node,
                data: None,
            },
        )],
        ..egui::RawInput::default()
    };
    let _ = context.run_ui(click, |outer_ui| {
        egui::Area::new("reference_test_host".into()).show(outer_ui.ctx(), |ui| {
            render_reference_panel(ui, &references, &mut actions);
        });
    });
    assert!(actions.iter().any(|action| {
        matches!(
            action,
            DesktopAction::NavigateToReference { path, line, character }
                if path == "src/lib.rs" && *line == 6 && *character == 3
        )
    }));
    assert!(!actions.iter().any(|action| {
            matches!(action, DesktopAction::NavigateToReference { path, .. } if path == "<unavailable path>")
        }));
}

#[test]
fn code_action_accesskit_selects_exact_second_candidate_and_disables_invalid() {
    let context = egui::Context::default();
    context.enable_accesskit();
    let candidates = vec![
        LanguageCodeActionProjection {
            response_id: "resp-1".to_owned(),
            action_id: "first".to_owned(),
            title: "First".to_owned(),
            kind: Some("quickfix".to_owned()),
            is_preferred: false,
            disabled_reason: Some("not applicable".to_owned()),
            has_edit: true,
            has_command: false,
            buffer_id: None,
            snapshot_id: None,
            schema_version: 1,
        },
        LanguageCodeActionProjection {
            response_id: "resp-1".to_owned(),
            action_id: "second".to_owned(),
            title: "Second".to_owned(),
            kind: Some("quickfix".to_owned()),
            is_preferred: true,
            disabled_reason: None,
            has_edit: false,
            has_command: true,
            buffer_id: None,
            snapshot_id: None,
            schema_version: 1,
        },
    ];
    let raw = |events| egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(800.0, 600.0),
        )),
        events,
        ..egui::RawInput::default()
    };
    let mut actions = Vec::new();
    let first = context.run_ui(raw(Vec::new()), |ui| {
        render_code_action_panel(ui, &candidates, &mut actions);
    });
    let update = first
        .platform_output
        .accesskit_update
        .as_ref()
        .expect("code action accessibility tree");
    let node = update
        .nodes
        .iter()
        .find_map(|(id, node)| {
            (node.label() == Some("Second (quickfix)")
                && node.supports_action(egui::accesskit::Action::Click))
            .then_some(*id)
        })
        .expect("enabled second action should be accessible");
    let disabled_node = update
        .nodes
        .iter()
        .find_map(|(id, node)| {
            (node.label() == Some("First (quickfix) — disabled: not applicable"))
                .then_some((*id, node))
        })
        .expect("disabled action should remain visible to assistive technology");
    assert!(disabled_node.1.is_disabled());
    let disabled_click = raw(vec![egui::Event::AccessKitActionRequest(
        egui::accesskit::ActionRequest {
            action: egui::accesskit::Action::Click,
            target_tree: egui::accesskit::TreeId::ROOT,
            target_node: disabled_node.0,
            data: None,
        },
    )]);
    let _ = context.run_ui(disabled_click, |ui| {
        render_code_action_panel(ui, &candidates, &mut actions);
    });
    assert!(!actions.iter().any(|action| {
        matches!(action, DesktopAction::SelectCodeAction { action_id, .. } if action_id == "first")
    }));
    let click = raw(vec![egui::Event::AccessKitActionRequest(
        egui::accesskit::ActionRequest {
            action: egui::accesskit::Action::Click,
            target_tree: egui::accesskit::TreeId::ROOT,
            target_node: node,
            data: None,
        },
    )]);
    let _ = context.run_ui(click, |ui| {
        render_code_action_panel(ui, &candidates, &mut actions);
    });
    assert!(actions.iter().any(|action| {
        matches!(
            action,
            DesktopAction::SelectCodeAction { response_id, action_id }
                if response_id == "resp-1" && action_id == "second"
        )
    }));
    assert!(!actions.iter().any(|action| {
        matches!(action, DesktopAction::SelectCodeAction { action_id, .. } if action_id == "first")
    }));
}

#[test]
fn typescript_toolchain_draft_is_bounded_and_projection_restores_it() {
    let mut long_path = "x".repeat(TYPESCRIPT_TOOLCHAIN_PATH_MAX_CHARS + 20);
    bound_typescript_toolchain_path(&mut long_path);
    assert_eq!(
        long_path.chars().count(),
        TYPESCRIPT_TOOLCHAIN_PATH_MAX_CHARS
    );

    let projection = TypeScriptToolchainProjection {
        settings: Some(legion_protocol::TypeScriptToolchainSettings {
            server_archive: CanonicalPath("server.tgz".to_string()),
            compiler_archive: CanonicalPath("compiler.tgz".to_string()),
            node_executable: CanonicalPath("node".to_string()),
        }),
        status: LanguageToolchainConfigurationStatus::Draft,
    };
    let mut view = ProjectionView::new();
    view.sync_typescript_toolchain_draft(None, &projection);
    assert_eq!(view.typescript_toolchain_draft.server_archive, "server.tgz");
    assert_eq!(
        view.typescript_toolchain_draft.compiler_archive,
        "compiler.tgz"
    );
    assert_eq!(view.typescript_toolchain_draft.node_executable, "node");
    view.typescript_toolchain_draft.server_archive = "typed-draft".to_string();
    view.sync_typescript_toolchain_draft(None, &projection);
    assert_eq!(
        view.typescript_toolchain_draft.server_archive,
        "typed-draft"
    );

    let mut snapshot = legion_ui::Shell::empty("typescript").projection_snapshot();
    snapshot.active_buffer_projection.file_path = Some(CanonicalPath("src/main.ts".to_string()));
    assert!(active_file_supports_typescript(&snapshot));
    snapshot.active_buffer_projection.file_path = Some(CanonicalPath("src/main.rs".to_string()));
    assert!(!active_file_supports_typescript(&snapshot));
}

#[test]
fn rendered_typescript_configure_button_emits_action_on_accesskit_click() {
    let context = egui::Context::default();
    context.enable_accesskit();
    let mut snapshot = legion_ui::Shell::empty("typescript").projection_snapshot();
    snapshot.active_buffer_projection.file_path = Some(CanonicalPath("src/main.ts".to_string()));
    snapshot
        .language_tooling_projection
        .typescript_toolchain
        .settings = Some(legion_protocol::TypeScriptToolchainSettings {
        server_archive: CanonicalPath("server.tgz".to_string()),
        compiler_archive: CanonicalPath("compiler.tgz".to_string()),
        node_executable: CanonicalPath("node".to_string()),
    });
    let mut view = ProjectionView::new();
    view.utility_surface = Some(UtilitySurface::Settings);
    view.settings_section = SettingsSection::LanguageTools;
    let raw = |events| egui::RawInput {
        focused: true,
        screen_rect: Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(1_440.0, 900.0),
        )),
        events,
        ..egui::RawInput::default()
    };
    let mut first_actions = None;
    let first = context.run_ui(raw(Vec::new()), |ui| {
        first_actions = Some(view.render(ui, &snapshot).actions);
    });
    assert!(first_actions.expect("first render").is_empty());
    let node = first
        .platform_output
        .accesskit_update
        .as_ref()
        .expect("accessibility tree")
        .nodes
        .iter()
        .find_map(|(id, node)| {
            (node.label() == Some("Configure toolchain")
                && node.supports_action(egui::accesskit::Action::Click))
            .then_some(*id)
        })
        .expect("configure button should be accessible");
    let click = vec![egui::Event::AccessKitActionRequest(
        egui::accesskit::ActionRequest {
            action: egui::accesskit::Action::Click,
            target_tree: egui::accesskit::TreeId::ROOT,
            target_node: node,
            data: None,
        },
    )];
    let mut clicked_actions = None;
    let _ = context.run_ui(raw(click), |ui| {
        clicked_actions = Some(view.render(ui, &snapshot).actions);
    });
    assert_eq!(
        clicked_actions.expect("clicked render"),
        vec![DesktopAction::ConfigureTypeScriptToolchain {
            server_archive: "server.tgz".to_string(),
            compiler_archive: "compiler.tgz".to_string(),
            node_executable: "node".to_string(),
        }]
    );
}

#[test]
fn provider_permission_uses_plain_ai_copy() {
    assert_eq!(
        workflow_permission_action_label(PermissionBudgetActionClass::InvokeProvider),
        "Uses an AI provider"
    );
}

#[test]
fn tab_drop_target_accounts_for_source_removal() {
    // Before B: no-op for A; after B: [B, A, C].
    assert_eq!(adjusted_tab_drop_target(0, 1), 0);
    assert_eq!(adjusted_tab_drop_target(0, 2), 1);
    // Before C: no-op for B; after C: [A, C, B].
    assert_eq!(adjusted_tab_drop_target(1, 2), 1);
    assert_eq!(adjusted_tab_drop_target(1, 3), 2);
    assert_eq!(adjusted_tab_drop_target(2, 0), 0);
    assert_eq!(adjusted_tab_drop_target(1, 1), 1);
}

#[test]
fn tab_drop_target_can_insert_after_the_last_tab() {
    // A right-half drop on C in [A, B, C] is the pre-removal slot 3;
    // after removing B, the app inserts it at index 2.
    assert_eq!(adjusted_tab_drop_target(1, 3), 2);
}

#[test]
fn find_match_byte_columns_convert_to_display_columns() {
    assert_eq!(byte_column_to_display_column("éfoo", 0), 0);
    assert_eq!(byte_column_to_display_column("éfoo", 2), 1);
    assert_eq!(byte_column_to_display_column("éfoo", 5), 4);
}

#[test]
fn find_bar_keybinding_routes_to_search_palette_action() {
    let snapshot = Shell::empty("Keybinding test").projection_snapshot();
    assert!(matches!(
        action_label_to_desktop_action("ToggleFindBar", &snapshot),
        Some(DesktopAction::OpenPalette {
            mode: PaletteMode::Search,
            query,
            scope: SearchScopeProjection::ActiveFile,
        }) if query == "/"
    ));
}

#[test]
fn format_and_organize_imports_keybindings_dispatch_proposals() {
    let snapshot = Shell::empty("Format keybinding test").projection_snapshot();
    assert_eq!(
        action_label_to_desktop_action("FormatDocument", &snapshot),
        Some(DesktopAction::RequestFormattingProposal)
    );
    assert_eq!(
        action_label_to_desktop_action("OrganizeImports", &snapshot),
        Some(DesktopAction::RequestOrganizeImportsProposal)
    );
}

#[test]
fn problem_keybindings_route_to_navigation_actions() {
    let snapshot = Shell::empty("Problem keybinding test").projection_snapshot();
    assert_eq!(
        action_label_to_desktop_action("ProblemNext", &snapshot),
        Some(DesktopAction::ProblemNext)
    );
    assert_eq!(
        action_label_to_desktop_action("ProblemPrev", &snapshot),
        Some(DesktopAction::ProblemPrev)
    );
    assert_eq!(key_label_to_egui("F8"), Some(egui::Key::F8));

    let bindings = legion_ui::ui::default_keymap();
    assert!(bindings.iter().any(|binding| {
        binding.combo.key == "F8" && !binding.combo.shift && binding.action_label == "ProblemNext"
    }));
    assert!(bindings.iter().any(|binding| {
        binding.combo.key == "F8" && binding.combo.shift && binding.action_label == "ProblemPrev"
    }));
}

#[test]
fn automate_permission_session_is_parsed_from_request_labels() {
    let request = delegated_task_tool_permission_request(DelegatedTaskToolPermissionRequestInput {
        request_id: "automate:permission:beta".to_string(),
        profile: DelegatedTaskToolPermissionProfile::Write,
        action_class: PermissionBudgetActionClass::InvokeLocalTool,
        capability: Some(CapabilityId("mcp.tool.call".to_string())),
        target_id: Some("mcp-tool:mcp:test|write_file".to_string()),
        decision: DelegatedTaskToolPermissionDecision::Confirm,
        labels: vec![
            "automate.permission.mcp_tool_call".to_string(),
            "legion.session:session:legion:beta".to_string(),
        ],
        schema_version: 1,
    });

    let session_id = parse_automate_permission_session(&request)
        .expect("request should carry its owning workflow session");

    assert_eq!(session_id.0, "session:legion:beta");
}

#[test]
fn code_line_fingerprint_is_stable_for_identical_input() {
    let line = DesktopCodeLineViewModel {
        number: 1,
        text: "fn main() {}".to_string(),
        highlights: Vec::new(),
        truncation_state: ViewportLineTruncationState::None,
        byte_range: ByteRange::new(0, 12),
        utf16_range: Utf16Range {
            start: legion_protocol::Utf16Position {
                line: 0,
                character: 0,
            },
            end: legion_protocol::Utf16Position {
                line: 0,
                character: 12,
            },
        },
        line_start_byte_offset: Some(0),
        logical_end_byte: Some(12),
        line_start_utf16_offset: None,
    };

    assert_eq!(
        code_line_content_fingerprint(&line),
        code_line_content_fingerprint(&line)
    );
}

#[test]
fn small_buffer_geometry_preserves_multiline_byte_and_utf16_origins() {
    let rows = small_buffer_code_lines("é\r\n🙂\n");
    assert_eq!(rows.len(), 3);
    assert_eq!(rows[0].text, "é");
    assert_eq!(rows[0].byte_range, ByteRange::new(0, 2));
    assert_eq!(rows[0].line_start_utf16_offset, Some(0));
    assert_eq!(rows[1].text, "🙂");
    // The helper preserves raw source offsets: `é` is two UTF-8 bytes and
    // CRLF is two more bytes, while UTF-16 counts the scalar plus CRLF.
    assert_eq!(rows[1].byte_range, ByteRange::new(4, 8));
    assert_eq!(rows[1].line_start_byte_offset, Some(4));
    assert_eq!(rows[1].line_start_utf16_offset, Some(3));
    assert!(rows[2].text.is_empty());
    assert_eq!(rows[2].line_start_byte_offset, Some(9));
    assert_eq!(rows[2].line_start_utf16_offset, Some(6));
}

#[test]
fn active_viewport_wrap_config_shapes_rows_even_when_settings_differ() {
    let text = "one two three four five six";
    let model = DesktopCodeLineViewModel {
        number: 1,
        text: text.to_string(),
        highlights: Vec::new(),
        truncation_state: ViewportLineTruncationState::None,
        byte_range: ByteRange::new(0, text.len() as u64),
        utf16_range: Utf16Range {
            start: legion_protocol::Utf16Position {
                line: 0,
                character: 0,
            },
            end: legion_protocol::Utf16Position {
                line: 0,
                character: text.encode_utf16().count() as u32,
            },
        },
        line_start_byte_offset: Some(0),
        logical_end_byte: Some(text.len() as u64),
        line_start_utf16_offset: Some(0),
    };
    let settings_width = active_code_line_wrap_config(LineWrappingPolicy::Off, None, None, 48.0).2;
    let (_, _, viewport_width) = active_code_line_wrap_config(
        LineWrappingPolicy::Off,
        None,
        Some((LineWrappingPolicy::Viewport, None)),
        48.0,
    );
    assert!(settings_width.is_infinite());
    assert_eq!(viewport_width, 48.0);
    let context = egui::Context::default();
    let mut row_count = 0;
    let _ = context.run_ui(egui::RawInput::default(), |ui| {
        row_count = shape_code_line_galley(ui, &model, viewport_width)
            .rows
            .len();
    });
    assert!(
        row_count > 1,
        "active viewport width must drive actual wrapping"
    );
}

#[test]
fn shell_geometry_compact_top_bar_keeps_modes_and_command_palette_without_bar_overflow() {
    let desktop = ShellGeometry::for_available_size(1440.0, 900.0);
    let compact = ShellGeometry::for_available_size(960.0, 720.0);

    let desktop_top_bar = top_bar_composition(desktop);
    assert_eq!(desktop_top_bar.density, TopBarDensity::Desktop);
    assert!(desktop_top_bar.shows_workspace_context);
    assert!(desktop_top_bar.shows_mode_switch);
    assert!(desktop_top_bar.shows_command_palette);

    let compact_top_bar = top_bar_composition(compact);
    assert_eq!(compact_top_bar.density, TopBarDensity::Compact);
    assert!(compact_top_bar.shows_mode_switch);
    assert!(compact_top_bar.shows_command_palette);
    assert!(!compact_top_bar.shows_workspace_context);
    assert_eq!(compact.top_bar_content_height(), 30.0);
    assert_eq!(compact.status_bar_content_height(), 22.0);
}

#[test]
fn code_line_fingerprint_changes_with_highlight_kind() {
    let mut keyword = DesktopCodeLineViewModel {
        number: 1,
        text: "fn main() {}".to_string(),
        highlights: vec![DesktopCodeHighlightSpan {
            start_col: 0,
            end_col: 2,
            kind: ViewportSemanticTokenKind::Keyword,
        }],
        truncation_state: ViewportLineTruncationState::None,
        byte_range: ByteRange::new(0, 12),
        utf16_range: Utf16Range {
            start: legion_protocol::Utf16Position {
                line: 0,
                character: 0,
            },
            end: legion_protocol::Utf16Position {
                line: 0,
                character: 12,
            },
        },
        line_start_byte_offset: Some(0),
        logical_end_byte: Some(12),
        line_start_utf16_offset: None,
    };
    let keyword_hash = code_line_content_fingerprint(&keyword);
    keyword.highlights[0].kind = ViewportSemanticTokenKind::Function;

    assert_ne!(keyword_hash, code_line_content_fingerprint(&keyword));
}

#[test]
fn code_line_width_bucket_quantizes_to_four_pixels() {
    assert_eq!(code_line_width_bucket(100.0), 25);
    assert_eq!(code_line_width_bucket(103.9), 25);
    assert_eq!(code_line_width_bucket(104.0), 26);
    assert_eq!(code_line_width_bucket(-1.0), 0);
}

#[test]
fn code_line_galley_cache_discards_entries_from_prior_render_pass() {
    let mut cache = RenderPassCache::<u8, u8>::default();
    cache.prepare_for_pass(41);
    cache.insert_bounded(1, 7, CODE_LINE_GALLEY_CACHE_LIMIT);
    assert_eq!(cache.get(&1), Some(&7));

    cache.prepare_for_pass(42);

    assert_eq!(cache.get(&1), None);
    assert_eq!(cache.entries.len(), 0);
}

#[test]
fn code_line_galley_cache_never_exceeds_its_entry_limit() {
    let mut cache = RenderPassCache::<usize, usize>::default();
    cache.prepare_for_pass(1);
    for key in 0..=CODE_LINE_GALLEY_CACHE_LIMIT {
        cache.insert_bounded(key, key, CODE_LINE_GALLEY_CACHE_LIMIT);
    }

    assert!(cache.entries.len() <= CODE_LINE_GALLEY_CACHE_LIMIT);
}

#[test]
fn code_line_galley_cache_key_changes_on_content_width_buffer_or_snapshot() {
    let line = DesktopCodeLineViewModel {
        number: 7,
        text: "let value = 1;".to_string(),
        highlights: Vec::new(),
        truncation_state: ViewportLineTruncationState::None,
        byte_range: ByteRange::new(0, 14),
        utf16_range: Utf16Range {
            start: legion_protocol::Utf16Position {
                line: 0,
                character: 0,
            },
            end: legion_protocol::Utf16Position {
                line: 0,
                character: 14,
            },
        },
        line_start_byte_offset: Some(0),
        logical_end_byte: Some(14),
        line_start_utf16_offset: None,
    };
    let snapshot_id = Some(legion_protocol::SnapshotId(11));
    let base = code_line_galley_cache_key(
        Some(legion_protocol::BufferId(1)),
        snapshot_id,
        &line,
        100.0,
    );
    let same = code_line_galley_cache_key(
        Some(legion_protocol::BufferId(1)),
        snapshot_id,
        &line,
        103.0,
    );
    let different_width = code_line_galley_cache_key(
        Some(legion_protocol::BufferId(1)),
        snapshot_id,
        &line,
        104.0,
    );
    let different_buffer = code_line_galley_cache_key(
        Some(legion_protocol::BufferId(2)),
        snapshot_id,
        &line,
        100.0,
    );
    let different_snapshot = code_line_galley_cache_key(
        Some(legion_protocol::BufferId(1)),
        Some(legion_protocol::SnapshotId(12)),
        &line,
        100.0,
    );
    let mut changed_line = line.clone();
    changed_line.text.push_str(" // changed");
    let different_content = code_line_galley_cache_key(
        Some(legion_protocol::BufferId(1)),
        snapshot_id,
        &changed_line,
        100.0,
    );

    assert_eq!(base, same);
    assert_ne!(base, different_width);
    assert_ne!(base, different_buffer);
    assert_ne!(base, different_snapshot);
    assert_ne!(base, different_content);
}

#[test]
fn code_line_cache_id_is_shared_across_buffers() {
    assert_eq!(
        code_line_galley_cache_id(legion_protocol::BufferId(1)),
        code_line_galley_cache_id(legion_protocol::BufferId(2))
    );
}

#[test]
fn code_line_galley_cache_has_one_total_bound_across_buffers() {
    let mut cache = RenderPassCache::<CodeLineGalleyCacheKey, u8>::default();
    cache.prepare_for_pass(1);
    for buffer in 0..(CODE_LINE_GALLEY_CACHE_LIMIT * 2) {
        cache.insert_bounded(
            CodeLineGalleyCacheKey {
                buffer_id: buffer as u128,
                snapshot_id: 1,
                content_fingerprint: buffer as u64,
                font_size_bucket: 12,
                width_bucket: 800,
            },
            0,
            CODE_LINE_GALLEY_CACHE_LIMIT,
        );
    }

    assert!(cache.entries.len() <= CODE_LINE_GALLEY_CACHE_LIMIT);
}

#[test]
fn code_line_truncation_marker_reflects_slice_state() {
    assert_eq!(
        code_line_truncation_marker(ViewportLineTruncationState::None),
        " "
    );
    assert_eq!(
        code_line_truncation_marker(ViewportLineTruncationState::Leading),
        "↤"
    );
    assert_eq!(
        code_line_truncation_marker(ViewportLineTruncationState::Trailing),
        "↦"
    );
    assert_eq!(
        code_line_truncation_marker(ViewportLineTruncationState::Both),
        "↔"
    );
}

#[test]
fn git_code_canvas_projects_gutter_markers_inline_blame_and_hunk_navigation() {
    let relative_path = Some("src/lib.rs");
    let hunks = vec![GitHunkProjection {
        hunk_id: "git-hunk:1".to_string(),
        path: "src/lib.rs".to_string(),
        stage: GitHunkStageProjection::Unstaged,
        header: "@@ -1,3 +1,4 @@".to_string(),
        old_start: 1,
        old_lines: 3,
        new_start: 2,
        new_lines: 2,
        added_lines: 1,
        deleted_lines: 1,
        submodule_dirty_only: false,
        context: Some("main".to_string()),
    }];
    let blame_lines = vec![GitBlameLineProjection {
        path: "src/lib.rs".to_string(),
        line_number: 2,
        commit_short: "abc1234".to_string(),
        author: "Ada Lovelace".to_string(),
        summary: "refine gutter diff".to_string(),
        line_preview: "let value = 1;".to_string(),
    }];

    assert_eq!(
        git_relative_path(Some("/repo"), Some("/repo/src/lib.rs")),
        Some("src/lib.rs".to_string())
    );
    assert_eq!(git_hunk_marker_for_line(relative_path, &hunks, 1), None);
    assert_eq!(
        git_hunk_marker_for_line(relative_path, &hunks, 2),
        Some("~")
    );
    assert_eq!(
        git_inline_blame_label(relative_path, &blame_lines, 2),
        Some("abc1234 Ada Lovelace refine gutter diff".to_string())
    );
    assert_eq!(
        git_previous_hunk_cursor(relative_path, &hunks, 3),
        Some(TextCoordinate {
            line: 1,
            character: 0,
            byte_offset: None,
            utf16_offset: None,
        })
    );
    assert_eq!(
        git_next_hunk_cursor(relative_path, &hunks, 1),
        Some(TextCoordinate {
            line: 1,
            character: 0,
            byte_offset: None,
            utf16_offset: None,
        })
    );
}

#[test]
fn stage_focused_git_hunk_routes_only_an_unstaged_focus() {
    let mut snapshot = Shell::empty("focused hunk").projection_snapshot();
    snapshot.git_projection.focused_hunk_id = Some("git-hunk:focused".to_string());
    snapshot.git_projection.hunks = vec![GitHunkProjection {
        hunk_id: "git-hunk:focused".to_string(),
        path: "src/lib.rs".to_string(),
        stage: GitHunkStageProjection::Unstaged,
        header: "@@ -1 +1 @@".to_string(),
        old_start: 1,
        old_lines: 1,
        new_start: 1,
        new_lines: 1,
        added_lines: 1,
        deleted_lines: 1,
        submodule_dirty_only: false,
        context: None,
    }];

    assert_eq!(
        action_label_to_desktop_action("StageFocusedGitHunk", &snapshot),
        Some(DesktopAction::StageGitHunk {
            hunk_id: "git-hunk:focused".to_string(),
        })
    );

    snapshot.git_projection.hunks[0].stage = GitHunkStageProjection::Staged;
    assert_eq!(
        action_label_to_desktop_action("StageFocusedGitHunk", &snapshot),
        None
    );
}

#[test]
fn terminal_text_segments_split_urls_and_trailing_text() {
    let segments =
        terminal_text_segments("open https://example.com/docs?ref=legion, then keep going");
    assert_eq!(
        segments,
        vec![
            TerminalTextSegment::Text("open ".to_string()),
            TerminalTextSegment::Url("https://example.com/docs?ref=legion".to_string()),
            TerminalTextSegment::Text(", then keep going".to_string()),
        ]
    );
}

#[test]
fn terminal_output_row_badges_reflect_projection_flags() {
    let row = TerminalOutputRowProjection {
        session_id: legion_protocol::TerminalSessionId(9),
        sequence: legion_protocol::EventSequence(3),
        redacted_payload: "warning: truncated".to_string(),
        byte_count: 42,
        is_stderr: true,
        truncated: true,
        redaction: RedactionHint::MetadataOnly,
        schema_version: 1,
    };

    assert_eq!(
        terminal_output_row_badges(&row),
        vec![
            "stderr".to_string(),
            "truncated".to_string(),
            "redacted=metadata-only".to_string(),
            "42 bytes".to_string(),
        ]
    );
}

#[test]
fn terminal_output_row_badges_reflect_shell_command_markers() {
    let row = TerminalOutputRowProjection {
        session_id: legion_protocol::TerminalSessionId(11),
        sequence: legion_protocol::EventSequence(7),
        redacted_payload: "command block finished • exit=0 • duration=15ms • cwd=/tmp/workspace"
            .to_string(),
        byte_count: 0,
        is_stderr: false,
        truncated: false,
        redaction: RedactionHint::MetadataOnly,
        schema_version: 1,
    };

    assert_eq!(
        terminal_output_row_badges(&row),
        vec![
            "command-finished".to_string(),
            "exit=0".to_string(),
            "duration=15ms".to_string(),
            "cwd=/tmp/workspace".to_string(),
        ]
    );
}
