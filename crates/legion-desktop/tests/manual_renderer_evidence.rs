use legion_desktop::view::DesktopProjectionViewModel;
use legion_protocol::{
    BufferId, CanonicalPath, FileId, WorkbenchFontFallbackDiagnostic, WorkspaceId,
};
use legion_ui::{ActiveBufferProjection, SettingsProjection, Shell};

fn diagnostic(index: usize) -> WorkbenchFontFallbackDiagnostic {
    WorkbenchFontFallbackDiagnostic {
        requested_family_label: "JetBrains Mono".to_string(),
        resolved_family_label: "legion-cjk-fallback".to_string(),
        coverage_label: format!("cjk-{index}"),
        fallback_found: true,
        message: "CJK fallback loaded from host font catalog".to_string(),
        schema_version: 1,
    }
}

fn path_diagnostic() -> WorkbenchFontFallbackDiagnostic {
    WorkbenchFontFallbackDiagnostic {
        requested_family_label: "C:\\Windows\\Fonts\\malgun.ttf\n".to_string(),
        resolved_family_label: "/usr/share/fonts/noto/NotoSansCJK.ttc".to_string(),
        coverage_label: "/System/Library/Fonts/PingFang.ttc\u{0000}".to_string(),
        fallback_found: true,
        message: "host catalog contained a font path".to_string(),
        schema_version: 1,
    }
}

#[test]
fn font_fallback_diagnostics_are_projected_without_raw_font_paths() {
    let mut snapshot = Shell::empty("Font").projection_snapshot();
    let mut font_fallback_diagnostics = vec![path_diagnostic()];
    font_fallback_diagnostics.extend((0..8).map(diagnostic));
    snapshot.settings_projection = SettingsProjection {
        editor_font_family: "JetBrains Mono".to_string(),
        font_fallback_diagnostics,
        ..SettingsProjection::default()
    };

    let model = DesktopProjectionViewModel::from_snapshot(&snapshot);

    assert_eq!(model.settings.editor_font_family, "JetBrains Mono");
    assert_eq!(model.settings.font_fallback_rows.len(), 8);
    assert!(
        model
            .settings
            .font_fallback_rows
            .iter()
            .any(|row| row.contains("coverage=cjk-0") && row.contains("found=true"))
    );
    assert!(model.settings.font_fallback_rows.iter().all(|row| {
        !row.contains("\\Windows\\Fonts")
            && !row.contains("/usr/share/fonts")
            && !row.contains("/System/Library/Fonts")
            && !row.contains('\n')
            && !row.contains('\0')
    }));
}

#[test]
fn line_wrapping_policy_keeps_viewport_math_stable() {
    let mut snapshot = Shell::empty("Wrap").projection_snapshot();
    snapshot.settings_projection.editor.line_wrapping_policy =
        legion_protocol::LineWrappingPolicy::FixedColumn;
    snapshot.settings_projection.editor.wrap_column = Some(80);

    let model = DesktopProjectionViewModel::from_snapshot(&snapshot);

    assert_eq!(
        model.settings.line_wrapping_policy,
        legion_protocol::LineWrappingPolicy::FixedColumn
    );
    assert_eq!(model.settings.wrap_column, Some(80));
    assert_eq!(model.settings.wrapping_row, "wrapping: fixed_column 80");
    assert!(
        model
            .viewport_metadata_rows
            .iter()
            .all(|row| !row.contains("visual_line"))
    );
}

#[test]
fn deterministic_renderer_evidence_covers_core_editor_states() {
    let empty_snapshot = Shell::empty("Evidence").projection_snapshot();
    let empty_model = DesktopProjectionViewModel::from_snapshot(&empty_snapshot);

    let empty_evidence = empty_model.deterministic_editor_evidence();

    assert!(empty_evidence.iter().any(|row| row == "title=Evidence"));
    assert!(
        empty_evidence
            .iter()
            .any(|row| row.starts_with("editor_status="))
    );
    assert!(
        empty_evidence
            .iter()
            .any(|row| row.starts_with("viewport=") || row == "flag=no_active_buffer")
    );
    assert!(
        empty_evidence
            .iter()
            .all(|row| !row.contains("raw_source="))
    );

    for path in [
        "C:\\Users\\ada\\secret\\evidence.rs",
        "/home/ada/secret/evidence.rs",
        "/System/Volumes/Data/Users/ada/secret/evidence.rs",
    ] {
        let mut active_snapshot = Shell::empty("Evidence Active").projection_snapshot();
        active_snapshot.active_buffer_projection = ActiveBufferProjection {
            workspace_id: Some(WorkspaceId(1)),
            buffer_id: Some(BufferId(2)),
            file_id: Some(FileId(3)),
            file_path: Some(CanonicalPath(path.to_string())),
            viewport: None,
            degraded: false,
            small_buffer_preview: Some(
                "let super_secret = 42;\nprintln!(\"hidden payload\");".to_string(),
            ),
            dirty: false,
        };
        let active_model = DesktopProjectionViewModel::from_snapshot(&active_snapshot);

        let active_evidence = active_model.deterministic_editor_evidence();

        assert!(
            active_evidence
                .iter()
                .any(|row| row == "code_line=1 len=22 truncation=None")
        );
        assert!(active_evidence.iter().all(|row| {
            !row.contains("super_secret")
                && !row.contains("hidden payload")
                && !row.contains("raw_source=")
                && !row.contains("C:\\Users")
                && !row.contains("/home/ada")
                && !row.contains("/System/Volumes")
                && !row.contains("secret")
        }));
    }
}
