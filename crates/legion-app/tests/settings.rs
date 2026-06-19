use std::{
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

use legion_app::{AppCommandOutcome, AppComposition};
use legion_protocol::{PrincipalId, WorkspaceTrustState};
use legion_ui::{
    CommandDispatchIntent, PaletteMode, PaletteResultKind, SearchScopeProjection,
    SettingsProjection, ThemePreferenceProjection, ToastVerbosityProjection,
};

static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

struct TempWorkspace {
    root: PathBuf,
}

impl TempWorkspace {
    fn new() -> Self {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be after epoch")
            .as_nanos();
        let id = TEMP_COUNTER.fetch_add(1, Ordering::SeqCst);
        let root = std::env::temp_dir().join(format!(
            "legion_app_settings_{}_{}_{}",
            std::process::id(),
            nanos,
            id
        ));
        fs::create_dir(&root).expect("temp workspace should be created");
        Self { root }
    }

    fn path(&self) -> &Path {
        &self.root
    }
}

impl Drop for TempWorkspace {
    fn drop(&mut self) {
        let temp_root = std::env::temp_dir();
        let file_name = self.root.file_name().and_then(|name| name.to_str());
        if self.root.starts_with(&temp_root)
            && file_name.is_some_and(|name| name.starts_with("legion_app_settings_"))
        {
            let _ = fs::remove_dir_all(&self.root);
        }
    }
}

fn open_app() -> (TempWorkspace, AppComposition) {
    let workspace = TempWorkspace::new();
    let mut app = AppComposition::new();
    app.open_workspace(
        workspace.path(),
        WorkspaceTrustState::Trusted,
        PrincipalId("settings-test".to_string()),
    )
    .expect("workspace should open");
    (workspace, app)
}

fn settings_from_outcome(outcome: AppCommandOutcome) -> legion_ui::SettingsProjection {
    match outcome {
        AppCommandOutcome::SettingsUpdated(settings) => settings,
        other => panic!("expected settings update, got {other:?}"),
    }
}

#[test]
fn settings_intents_update_projection_and_clamp_numeric_values() {
    let (_workspace, mut app) = open_app();

    let settings = settings_from_outcome(
        app.dispatch_ui_intent(CommandDispatchIntent::SetThemePreference {
            preference: ThemePreferenceProjection::Light,
        })
        .expect("theme preference should update"),
    );
    assert_eq!(settings.theme_preference, ThemePreferenceProjection::Light);

    let settings = settings_from_outcome(
        app.dispatch_ui_intent(CommandDispatchIntent::SetZoomPercent { zoom_percent: 999 })
            .expect("zoom should update"),
    );
    assert_eq!(settings.zoom_percent, SettingsProjection::MAX_ZOOM_PERCENT);

    let settings = settings_from_outcome(
        app.dispatch_ui_intent(CommandDispatchIntent::SetEditorFontSize { font_size_pt: 1 })
            .expect("editor font should update"),
    );
    assert_eq!(
        settings.editor_font_size_pt,
        SettingsProjection::MIN_EDITOR_FONT_SIZE_PT
    );

    let settings = settings_from_outcome(
        app.dispatch_ui_intent(CommandDispatchIntent::SetEditorFontFamily {
            family: "  JetBrains Mono<script>\n".to_string(),
        })
        .expect("editor font family should update"),
    );
    assert_eq!(settings.editor_font_family, "JetBrains Monoscript");

    let settings = settings_from_outcome(
        app.dispatch_ui_intent(CommandDispatchIntent::SetToastVerbosity {
            verbosity: ToastVerbosityProjection::All,
        })
        .expect("toast verbosity should update"),
    );
    assert_eq!(settings.toast_verbosity, ToastVerbosityProjection::All);

    let settings = settings_from_outcome(
        app.dispatch_ui_intent(CommandDispatchIntent::SetLineNumbersVisible { visible: false })
            .expect("line number setting should update"),
    );
    assert!(!settings.editor.line_numbers_visible);

    let settings = settings_from_outcome(
        app.dispatch_ui_intent(CommandDispatchIntent::SetCurrentLineHighlight { enabled: false })
            .expect("current line setting should update"),
    );
    assert!(!settings.editor.current_line_highlight);

    let settings = settings_from_outcome(
        app.dispatch_ui_intent(CommandDispatchIntent::SetStickyHeadersVisible { visible: false })
            .expect("sticky header setting should update"),
    );
    assert!(!settings.editor.sticky_headers_visible);

    let settings = settings_from_outcome(
        app.dispatch_ui_intent(CommandDispatchIntent::SetCodeFoldingVisible { visible: false })
            .expect("code folding setting should update"),
    );
    assert!(!settings.editor.code_folding_visible);

    let settings = settings_from_outcome(
        app.dispatch_ui_intent(CommandDispatchIntent::SetMinimapVisible { visible: true })
            .expect("minimap setting should update"),
    );
    assert!(settings.editor.minimap_visible);

    let settings = settings_from_outcome(
        app.dispatch_ui_intent(CommandDispatchIntent::SetWhitespaceGuidesVisible { visible: true })
            .expect("whitespace guides setting should update"),
    );
    assert!(settings.editor.whitespace_guides_visible);

    let settings = settings_from_outcome(
        app.dispatch_ui_intent(CommandDispatchIntent::SetIndentGuidesVisible { visible: true })
            .expect("indent guides setting should update"),
    );
    assert!(settings.editor.indent_guides_visible);

    let settings = settings_from_outcome(
        app.dispatch_ui_intent(CommandDispatchIntent::SetSmoothScrollingEnabled { enabled: false })
            .expect("smooth scrolling setting should update"),
    );
    assert!(!settings.editor.smooth_scrolling_enabled);

    let settings = settings_from_outcome(
        app.dispatch_ui_intent(CommandDispatchIntent::SetIndexedWorkspaceSearchEnabled {
            enabled: true,
        })
        .expect("indexed search setting should update"),
    );
    assert!(settings.indexed_workspace_search_enabled);

    let settings = settings_from_outcome(
        app.dispatch_ui_intent(CommandDispatchIntent::SetNextEditPredictionEnabled {
            enabled: true,
        })
        .expect("next-edit prediction setting should update"),
    );
    assert!(settings.next_edit_prediction_enabled);

    let settings = settings_from_outcome(
        app.dispatch_ui_intent(CommandDispatchIntent::SetCrashReportsEnabled { enabled: true })
            .expect("crash reports setting should update"),
    );
    assert!(settings.telemetry.crash_reports_enabled);
    assert_eq!(settings.telemetry.consent_label, "crash-reports");

    let snapshot = app
        .shell_projection_snapshot("settings")
        .expect("projection should build");
    assert_eq!(snapshot.settings_projection, settings);
}

#[test]
fn command_palette_dispatches_preferences_commands() {
    let (_workspace, mut app) = open_app();
    app.dispatch_ui_intent(CommandDispatchIntent::OpenPalette {
        mode: PaletteMode::Command,
        query: ">Preferences: Theme Light".to_string(),
        scope: SearchScopeProjection::ActiveFile,
    })
    .expect("command palette should open");

    let palette = app
        .shell_projection_snapshot("settings")
        .expect("projection should build")
        .palette_projection;
    assert!(palette.open);
    assert_eq!(palette.mode, PaletteMode::Command);
    assert_eq!(palette.results[0].kind, PaletteResultKind::Command);
    assert_eq!(palette.results[0].title, "Preferences: Theme Light");

    let settings = settings_from_outcome(
        app.dispatch_ui_intent(CommandDispatchIntent::DispatchPaletteSelection)
            .expect("palette selection should dispatch"),
    );
    assert_eq!(settings.theme_preference, ThemePreferenceProjection::Light);
}
