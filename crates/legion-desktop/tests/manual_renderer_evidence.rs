use legion_desktop::view::DesktopProjectionViewModel;
use legion_protocol::WorkbenchFontFallbackDiagnostic;
use legion_ui::{SettingsProjection, Shell};

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
