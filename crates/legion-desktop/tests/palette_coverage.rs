//! Desktop palette coverage report.
//!
//! This regression locks the three fuzzy navigation surfaces and the command
//! palette catalog to the projection/dispatch paths the desktop adapter owns.

use std::{
    fs,
    path::{Path, PathBuf},
    sync::{
        Mutex, OnceLock,
        atomic::{AtomicU64, Ordering},
    },
    time::{SystemTime, UNIX_EPOCH},
};

use legion_app::{AppCommandOutcome, AppComposition};
use legion_desktop::{
    bridge::DesktopAction,
    workflow::{DesktopEframeApp, DesktopLaunchConfig, DesktopRuntime},
};
use legion_protocol::{PrincipalId, TextCoordinate, WorkspaceTrustState};
use legion_ui::{PaletteMode, PaletteResultKind, SearchScopeProjection};

static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);
static TEST_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

fn test_guard() -> std::sync::MutexGuard<'static, ()> {
    TEST_LOCK
        .get_or_init(|| Mutex::new(()))
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

struct TempWorkspace {
    root: PathBuf,
}

impl TempWorkspace {
    fn new(prefix: &str) -> Self {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be after epoch")
            .as_nanos();
        let id = TEMP_COUNTER.fetch_add(1, Ordering::SeqCst);
        let root =
            std::env::temp_dir().join(format!("{prefix}_{}_{}_{}", std::process::id(), nanos, id));
        fs::create_dir(&root).expect("temp workspace should be created");
        Self { root }
    }

    fn path(&self) -> &Path {
        &self.root
    }

    fn write(&self, name: &str, content: &str) -> PathBuf {
        let path = self.root.join(name);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("temp parent should be created");
        }
        fs::write(&path, content).expect("temp file should be written");
        path
    }
}

impl Drop for TempWorkspace {
    fn drop(&mut self) {
        let temp_root = std::env::temp_dir();
        let file_name = self.root.file_name().and_then(|name| name.to_str());
        if self.root.starts_with(&temp_root)
            && file_name.is_some_and(|name| name.starts_with("palette_coverage_"))
        {
            let _ = fs::remove_dir_all(&self.root);
        }
    }
}

fn open_runtime(root: &Path) -> DesktopRuntime {
    DesktopRuntime::open(DesktopLaunchConfig::new(root.to_path_buf(), None))
        .expect("desktop runtime should open workspace")
}

fn canonical_display(path: &Path) -> String {
    fs::canonicalize(path)
        .unwrap_or_else(|_| path.to_path_buf())
        .to_string_lossy()
        .into_owned()
}

fn command_key_input(key: egui::Key) -> egui::RawInput {
    egui::RawInput {
        focused: true,
        modifiers: egui::Modifiers {
            command: true,
            ..egui::Modifiers::default()
        },
        events: vec![egui::Event::Key {
            key,
            physical_key: Some(key),
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers {
                command: true,
                ..egui::Modifiers::default()
            },
        }],
        ..egui::RawInput::default()
    }
}

#[test]
fn fuzzy_file_symbol_and_recent_buffer_surfaces_are_reachable() {
    let _guard = test_guard();
    let workspace = TempWorkspace::new("palette_coverage");
    let first = workspace.write("src/alpha_widget.rs", "fn alpha_widget() {}\n");
    let second = workspace.write("src/beta_widget.rs", "fn beta_widget() {}\n");
    let symbol_file = workspace.write("src/lib.rs", "fn alpha_symbol() {}\nfn beta_symbol() {}\n");

    let mut app = DesktopEframeApp::new(open_runtime(workspace.path()));
    let snapshot = app.runtime_snapshot();
    assert!(!snapshot.palette_projection.open);

    let _ = app.run_headless_input(command_key_input(egui::Key::O));
    let snapshot = app.runtime_snapshot();
    assert!(
        snapshot.palette_projection.open,
        "Cmd+O should open the file palette"
    );
    assert_eq!(snapshot.palette_projection.mode, PaletteMode::File);

    let mut runtime = open_runtime(workspace.path());
    runtime
        .handle_action(DesktopAction::OpenPathText(
            symbol_file.to_string_lossy().to_string(),
        ))
        .expect("open symbol file");
    runtime
        .handle_action(DesktopAction::OpenPalette {
            mode: PaletteMode::Symbol,
            query: "alpha_symbol".to_string(),
            scope: SearchScopeProjection::Workspace,
        })
        .expect("symbol palette should open");
    let snapshot = runtime.projection_snapshot();
    assert!(snapshot.palette_projection.open);
    assert_eq!(snapshot.palette_projection.mode, PaletteMode::Symbol);
    assert_eq!(
        snapshot.palette_projection.results[0].kind,
        PaletteResultKind::Symbol
    );
    let expected_symbol_path = canonical_display(&symbol_file);
    assert_eq!(
        snapshot.palette_projection.results[0].path.as_deref(),
        Some(expected_symbol_path.as_str())
    );
    assert!(snapshot.palette_projection.results[0].position.is_some());

    runtime
        .handle_action(DesktopAction::OpenPathText(
            first.to_string_lossy().to_string(),
        ))
        .expect("open first file");
    let first_buffer = runtime
        .projection_snapshot()
        .active_buffer_projection
        .buffer_id
        .expect("first buffer");
    runtime
        .handle_action(DesktopAction::OpenPathText(
            second.to_string_lossy().to_string(),
        ))
        .expect("open second file");
    let second_buffer = runtime
        .projection_snapshot()
        .active_buffer_projection
        .buffer_id
        .expect("second buffer");
    runtime
        .handle_action(DesktopAction::SwitchTab {
            buffer_id: first_buffer,
        })
        .expect("switch back to first file");

    runtime
        .handle_action(DesktopAction::OpenPalette {
            mode: PaletteMode::RecentBuffers,
            query: String::new(),
            scope: SearchScopeProjection::Workspace,
        })
        .expect("recent buffer palette should open");
    let snapshot = runtime.projection_snapshot();
    assert!(snapshot.palette_projection.open);
    assert_eq!(snapshot.palette_projection.mode, PaletteMode::RecentBuffers);
    assert_eq!(
        snapshot.palette_projection.results[0].kind,
        PaletteResultKind::RecentBuffers
    );
    let expected_first_path = canonical_display(&first);
    assert_eq!(
        snapshot.palette_projection.results[0].path.as_deref(),
        Some(expected_first_path.as_str())
    );
    assert_eq!(
        snapshot.palette_projection.results[0].buffer_id,
        Some(first_buffer)
    );
    assert!(
        snapshot
            .palette_projection
            .results
            .iter()
            .any(|result| result.buffer_id == Some(second_buffer))
    );

    let outcome = runtime
        .handle_action(DesktopAction::DispatchPaletteSelection)
        .expect("recent buffer selection should dispatch");
    assert!(
        matches!(outcome, legion_desktop::workflow::DesktopWorkflowOutcome::TabSwitched(buffer) if buffer == first_buffer)
    );
}

#[test]
fn command_palette_coverage_report_resolves_catalog_commands() {
    let _guard = test_guard();
    let workspace = TempWorkspace::new("palette_coverage_commands");
    let source = workspace.write("src/main.rs", "fn main() {}\n");
    let mut app = AppComposition::new();
    app.open_workspace(
        workspace.path(),
        WorkspaceTrustState::Trusted,
        PrincipalId("palette-coverage".to_string()),
    )
    .expect("workspace should open");
    app.dispatch_ui_intent(legion_ui::CommandDispatchIntent::OpenPalette {
        mode: PaletteMode::File,
        query: "main".to_string(),
        scope: SearchScopeProjection::ActiveFile,
    })
    .expect("file finder should open");
    let palette = app
        .shell_projection_snapshot("palette")
        .expect("projection should build")
        .palette_projection;
    assert_eq!(palette.results[0].kind, PaletteResultKind::File);
    assert!(palette.results[0].title.contains("main.rs"));
    let outcome = app
        .dispatch_ui_intent(legion_ui::CommandDispatchIntent::DispatchPaletteSelection)
        .expect("file selection should dispatch");
    assert!(matches!(outcome, AppCommandOutcome::Opened(_)));

    assert!(
        app.active_buffer_id().is_some(),
        "opening a file via the fuzzy finder should activate a buffer"
    );

    enum ExpectedOutcome {
        Save,
        SaveAll,
        TabClosed,
        ExplorerRefreshed,
        GitUpdated,
        PaletteClosed,
        SettingsUpdated,
    }

    struct CommandCase {
        query: &'static str,
        expected_title: &'static str,
        expected_outcome: ExpectedOutcome,
        dirty_before_save: bool,
    }

    let cases = [
        CommandCase {
            query: ">save all",
            expected_title: "Save All",
            expected_outcome: ExpectedOutcome::SaveAll,
            dirty_before_save: false,
        },
        CommandCase {
            query: ">save active buffer",
            expected_title: "Save Active Buffer",
            expected_outcome: ExpectedOutcome::Save,
            dirty_before_save: true,
        },
        CommandCase {
            query: ">close active tab",
            expected_title: "Close Active Tab",
            expected_outcome: ExpectedOutcome::TabClosed,
            dirty_before_save: false,
        },
        CommandCase {
            query: ">reveal active file",
            expected_title: "Reveal Active File in Explorer",
            expected_outcome: ExpectedOutcome::ExplorerRefreshed,
            dirty_before_save: false,
        },
        CommandCase {
            query: ">refresh explorer",
            expected_title: "Refresh Explorer",
            expected_outcome: ExpectedOutcome::ExplorerRefreshed,
            dirty_before_save: false,
        },
        CommandCase {
            query: ">refresh git",
            expected_title: "Refresh Git",
            expected_outcome: ExpectedOutcome::GitUpdated,
            dirty_before_save: false,
        },
        CommandCase {
            query: ">close command palette",
            expected_title: "Close Command Palette",
            expected_outcome: ExpectedOutcome::PaletteClosed,
            dirty_before_save: false,
        },
        CommandCase {
            query: ">preferences open settings",
            expected_title: "Preferences: Open Settings",
            expected_outcome: ExpectedOutcome::SettingsUpdated,
            dirty_before_save: false,
        },
        CommandCase {
            query: ">preferences theme dark",
            expected_title: "Preferences: Theme Dark",
            expected_outcome: ExpectedOutcome::SettingsUpdated,
            dirty_before_save: false,
        },
        CommandCase {
            query: ">preferences theme light",
            expected_title: "Preferences: Theme Light",
            expected_outcome: ExpectedOutcome::SettingsUpdated,
            dirty_before_save: false,
        },
        CommandCase {
            query: ">preferences theme system",
            expected_title: "Preferences: Theme System",
            expected_outcome: ExpectedOutcome::SettingsUpdated,
            dirty_before_save: false,
        },
        CommandCase {
            query: ">preferences reset zoom",
            expected_title: "Preferences: Reset Zoom",
            expected_outcome: ExpectedOutcome::SettingsUpdated,
            dirty_before_save: false,
        },
        CommandCase {
            query: ">preferences reset settings",
            expected_title: "Preferences: Reset Settings",
            expected_outcome: ExpectedOutcome::SettingsUpdated,
            dirty_before_save: false,
        },
    ];

    let mut resolved_cases = 0_usize;
    for case in cases {
        if app.active_buffer_id().is_none() {
            app.open_file(source.to_string_lossy())
                .expect("reopen active buffer");
        }
        let initial_buffer_id = app.active_buffer_id().expect("active buffer");
        if case.dirty_before_save {
            app.dispatch_ui_intent(legion_ui::CommandDispatchIntent::Insert {
                buffer_id: initial_buffer_id,
                at: TextCoordinate {
                    line: 0,
                    character: 0,
                    byte_offset: None,
                    utf16_offset: None,
                },
                text: "// dirty\n".to_string(),
            })
            .expect("dirty insert should dispatch");
        }

        app.dispatch_ui_intent(legion_ui::CommandDispatchIntent::OpenPalette {
            mode: PaletteMode::Command,
            query: case.query.to_string(),
            scope: SearchScopeProjection::ActiveFile,
        })
        .expect("command palette should open");

        let palette = app
            .shell_projection_snapshot("palette")
            .expect("projection should build")
            .palette_projection;

        assert_eq!(palette.results[0].title, case.expected_title);
        assert_eq!(palette.results[0].kind, PaletteResultKind::Command);

        let outcome = app
            .dispatch_ui_intent(legion_ui::CommandDispatchIntent::DispatchPaletteSelection)
            .expect("command selection should dispatch");

        match case.expected_outcome {
            ExpectedOutcome::Save => assert!(matches!(outcome, AppCommandOutcome::Save(_))),
            ExpectedOutcome::SaveAll => assert!(matches!(outcome, AppCommandOutcome::SaveAll(_))),
            ExpectedOutcome::TabClosed => {
                assert!(matches!(outcome, AppCommandOutcome::TabClose(_)))
            }
            ExpectedOutcome::ExplorerRefreshed => {
                assert!(matches!(outcome, AppCommandOutcome::ExplorerRefreshed(_)))
            }
            ExpectedOutcome::GitUpdated => {
                assert!(matches!(outcome, AppCommandOutcome::GitUpdated(_)))
            }
            ExpectedOutcome::PaletteClosed => match outcome {
                AppCommandOutcome::PaletteUpdated(projection) => assert!(!projection.open),
                other => panic!("expected palette update, got {other:?}"),
            },
            ExpectedOutcome::SettingsUpdated => {
                assert!(matches!(outcome, AppCommandOutcome::SettingsUpdated(_)))
            }
        }

        resolved_cases += 1;
    }

    let coverage_percent = (resolved_cases as f32 / 13.0) * 100.0;
    assert!(
        coverage_percent >= 95.0,
        "command coverage report: {resolved_cases}/13 commands resolved ({coverage_percent:.1}%)"
    );
}
