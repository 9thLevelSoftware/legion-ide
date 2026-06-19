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

#[test]
fn font_fallback_diagnostics_are_projected_without_raw_font_paths() {
    let mut snapshot = Shell::empty("Font").projection_snapshot();
    snapshot.settings_projection = SettingsProjection {
        editor_font_family: "JetBrains Mono".to_string(),
        font_fallback_diagnostics: (0..9).map(diagnostic).collect(),
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
    assert!(
        model
            .settings
            .font_fallback_rows
            .iter()
            .all(|row| !row.contains("\\Windows\\Fonts") && !row.contains("/usr/share/fonts"))
    );
}
