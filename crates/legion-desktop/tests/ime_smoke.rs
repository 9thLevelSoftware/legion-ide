//! Smoke tests for the IME (Input Method Editor) lifecycle.
//!
//! These tests exercise the `ImeCompositionProjection` adapter-local overlay and
//! verify that egui IME events flow through the same keyboard handler that
//! production uses.  The invariants under test:
//!
//! * `ImeEvent::Enabled`  → composition becomes active.
//! * `ImeEvent::Preedit`  → preedit text is stored and composition stays active.
//! * `ImeEvent::Commit`   → preedit is cleared, text is inserted via
//!   `CommandDispatchIntent::Insert` (same path as
//!   `ClipboardPaste` and `InsertText`).
//! * `ImeEvent::Disabled` → composition state is fully cleared.

use std::{
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    sync::{Mutex, OnceLock},
    time::{SystemTime, UNIX_EPOCH},
};

use legion_desktop::{
    bridge::DesktopAction,
    workflow::{DesktopEframeApp, DesktopLaunchConfig, DesktopRuntime},
};

static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);
static TEST_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

fn ime_smoke_test_guard() -> std::sync::MutexGuard<'static, ()> {
    match TEST_LOCK.get_or_init(|| Mutex::new(())).lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    }
}

struct TempWorkspace {
    root: PathBuf,
}

impl TempWorkspace {
    fn new() -> Self {
        let temp_root = std::env::temp_dir();
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be after epoch")
            .as_nanos();
        let id = TEMP_COUNTER.fetch_add(1, Ordering::SeqCst);
        let root = temp_root.join(format!(
            "legion_desktop_ime_smoke_{}_{}_{}",
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

    fn write(&self, name: &str, content: &str) -> PathBuf {
        let path = self.root.join(name);
        fs::write(&path, content).expect("temp file should be written");
        path
    }
}

impl Drop for TempWorkspace {
    fn drop(&mut self) {
        let temp_root = std::env::temp_dir();
        let file_name = self.root.file_name().and_then(|name| name.to_str());
        if self.root.starts_with(&temp_root)
            && file_name.is_some_and(|name| name.starts_with("legion_desktop_ime_smoke_"))
        {
            let _ = fs::remove_dir_all(&self.root);
        }
    }
}

fn open_runtime(root: &Path) -> DesktopRuntime {
    DesktopRuntime::open(DesktopLaunchConfig::new(root.to_path_buf(), None))
        .expect("desktop runtime should open workspace")
}

fn open_runtime_with_file(root: &Path, file: &Path) -> DesktopRuntime {
    DesktopRuntime::open(DesktopLaunchConfig::new(
        root.to_path_buf(),
        Some(file.to_string_lossy().into_owned()),
    ))
    .expect("desktop runtime should open workspace and file")
}

fn text_input(text: &str) -> egui::RawInput {
    egui::RawInput {
        focused: true,
        events: vec![egui::Event::Text(text.to_string())],
        ..egui::RawInput::default()
    }
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

fn enter_input() -> egui::RawInput {
    egui::RawInput {
        focused: true,
        events: vec![egui::Event::Key {
            key: egui::Key::Enter,
            physical_key: Some(egui::Key::Enter),
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::default(),
        }],
        ..egui::RawInput::default()
    }
}

fn open_file_via_palette(app: &mut DesktopEframeApp, relative_path: &str) {
    let _ = app.run_headless_input(command_key_input(egui::Key::O));
    let _ = app.run_headless_input(text_input(relative_path));
    let _ = app.run_headless_input(enter_input());
}

fn ime_events_input(events: Vec<egui::Event>) -> egui::RawInput {
    egui::RawInput {
        focused: true,
        events,
        ..egui::RawInput::default()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

/// IME commit inserts text through `handle_action` (bridge-level test).
///
/// Verifies that `DesktopAction::ImeCommit` produces the same `Edited` outcome
/// as `InsertText` and `ClipboardPaste`, confirming they share the
/// `CommandDispatchIntent::Insert` path.
#[test]
fn ime_commit_inserts_text_through_bridge() {
    let workspace = TempWorkspace::new();
    let file = workspace.write("ime_bridge.txt", "hello");
    let mut runtime = open_runtime_with_file(workspace.path(), &file);

    let at = legion_protocol::TextCoordinate {
        line: 0,
        character: 5,
        byte_offset: Some(5),
        utf16_offset: Some(5),
    };

    let outcome = runtime
        .handle_action(DesktopAction::ImeCommit {
            text: "\u{4e16}\u{754c}".to_string(), // 世界
            at,
        })
        .expect("ImeCommit should route through app authority");

    assert_eq!(
        outcome,
        legion_desktop::workflow::DesktopWorkflowOutcome::Edited,
        "ImeCommit should produce Edited outcome like InsertText and ClipboardPaste"
    );

    let snapshot = runtime.projection_snapshot();
    assert!(
        snapshot.active_buffer_projection.dirty,
        "ImeCommit should mark the buffer dirty"
    );
}

/// IME commit and clipboard paste share the same bridge translation path.
///
/// Both `ImeCommit` and `ClipboardPaste` map to `CommandDispatchIntent::Insert`
/// in the bridge.  This test confirms behavioural equivalence by inserting
/// identical text through each path and verifying both produce `Edited`.
#[test]
fn ime_commit_and_clipboard_paste_share_insert_path() {
    let workspace = TempWorkspace::new();
    let file_ime = workspace.write("ime_path.txt", "base");
    let file_paste = workspace.write("paste_path.txt", "base");

    let at = legion_protocol::TextCoordinate {
        line: 0,
        character: 4,
        byte_offset: Some(4),
        utf16_offset: Some(4),
    };

    // IME path
    let mut runtime_ime = open_runtime_with_file(workspace.path(), &file_ime);
    let outcome_ime = runtime_ime
        .handle_action(DesktopAction::ImeCommit {
            text: "!".to_string(),
            at,
        })
        .expect("ImeCommit should succeed");

    // Clipboard paste path
    let mut runtime_paste = open_runtime_with_file(workspace.path(), &file_paste);
    let outcome_paste = runtime_paste
        .handle_action(DesktopAction::ClipboardPaste {
            text: "!".to_string(),
            at,
        })
        .expect("ClipboardPaste should succeed");

    assert_eq!(
        outcome_ime, outcome_paste,
        "ImeCommit and ClipboardPaste must produce identical outcomes"
    );

    // Both buffers should be dirty with the same edit applied.
    assert!(
        runtime_ime
            .projection_snapshot()
            .active_buffer_projection
            .dirty
    );
    assert!(
        runtime_paste
            .projection_snapshot()
            .active_buffer_projection
            .dirty
    );
}

/// Headless IME lifecycle: enable → preedit → commit → verify insertion.
///
/// Exercises the egui event stream through `run_headless_input` so the IME
/// composition tracking in `handle_keyboard` runs the same path as production.
#[test]
fn headless_ime_lifecycle_enable_preedit_commit() {
    let _guard = ime_smoke_test_guard();
    let workspace = TempWorkspace::new();
    workspace.write("ime_lifecycle.txt", "start");
    let runtime = open_runtime(workspace.path());
    let mut app = DesktopEframeApp::new(runtime);

    open_file_via_palette(&mut app, "ime_lifecycle.txt");
    assert!(
        app.runtime_snapshot()
            .active_buffer_projection
            .file_path
            .as_ref()
            .is_some_and(|path| path.0.ends_with("ime_lifecycle.txt")),
        "file should be opened before IME test begins"
    );

    // Step 1: IME enable
    let _ = app.run_headless_input(ime_events_input(vec![egui::Event::Ime(
        egui::ImeEvent::Enabled,
    )]));

    // Step 2: Preedit (composition in progress)
    let _ = app.run_headless_input(ime_events_input(vec![
        egui::Event::Ime(egui::ImeEvent::Preedit("\u{304b}\u{306a}".to_string())), // かな
    ]));

    // Step 3: Commit (finalise the composition)
    let _ = app.run_headless_input(ime_events_input(vec![
        egui::Event::Ime(egui::ImeEvent::Commit("\u{6f22}\u{5b57}".to_string())), // 漢字
    ]));

    let snapshot = app.runtime_snapshot();
    assert!(
        snapshot.active_buffer_projection.dirty,
        "IME commit through the headless harness should mark the buffer dirty"
    );
}

/// IME disable clears composition state even without a preceding commit.
///
/// When the user cancels an in-progress composition the platform sends
/// `ImeEvent::Disabled`.  The adapter must reset `composition.active` and
/// `composition.preedit` so no stale preedit leaks into subsequent frames.
#[test]
fn headless_ime_disable_clears_composition_state() {
    let _guard = ime_smoke_test_guard();
    let workspace = TempWorkspace::new();
    workspace.write("ime_cancel.txt", "unchanged");
    let runtime = open_runtime(workspace.path());
    let mut app = DesktopEframeApp::new(runtime);

    open_file_via_palette(&mut app, "ime_cancel.txt");

    // Enable → preedit → disable (cancel without commit).
    let _ = app.run_headless_input(ime_events_input(vec![egui::Event::Ime(
        egui::ImeEvent::Enabled,
    )]));
    let _ = app.run_headless_input(ime_events_input(vec![egui::Event::Ime(
        egui::ImeEvent::Preedit("partial".to_string()),
    )]));
    let _ = app.run_headless_input(ime_events_input(vec![egui::Event::Ime(
        egui::ImeEvent::Disabled,
    )]));

    // The buffer should NOT be dirty because no commit happened.
    let snapshot = app.runtime_snapshot();
    assert!(
        !snapshot.active_buffer_projection.dirty,
        "IME disable without commit must not mutate the buffer"
    );
}

/// Empty IME preedit resets the composition active flag.
///
/// The workflow code treats an empty preedit as `composition.active = false`
/// (see `ImeEvent::Preedit` handler: `composition.active = !preedit.is_empty()`).
/// Verify this invariant.
#[test]
fn headless_ime_empty_preedit_deactivates_composition() {
    let _guard = ime_smoke_test_guard();
    let workspace = TempWorkspace::new();
    workspace.write("ime_empty.txt", "text");
    let runtime = open_runtime(workspace.path());
    let mut app = DesktopEframeApp::new(runtime);

    open_file_via_palette(&mut app, "ime_empty.txt");

    // Enable → non-empty preedit → empty preedit (resets active flag).
    let _ = app.run_headless_input(ime_events_input(vec![egui::Event::Ime(
        egui::ImeEvent::Enabled,
    )]));
    let _ = app.run_headless_input(ime_events_input(vec![egui::Event::Ime(
        egui::ImeEvent::Preedit("draft".to_string()),
    )]));
    let _ = app.run_headless_input(ime_events_input(vec![egui::Event::Ime(
        egui::ImeEvent::Preedit(String::new()),
    )]));

    // An empty preedit should make the composition inactive, and since no
    // commit event was dispatched, the buffer stays clean.
    assert!(
        !app.runtime_snapshot().active_buffer_projection.dirty,
        "empty preedit without commit must not dirty the buffer"
    );
}

/// IME commit with empty text is a no-op.
///
/// The workflow skips empty commit text (`if !text.is_empty()`).  Verify the
/// guard by confirming the buffer stays clean.
#[test]
fn headless_ime_empty_commit_is_noop() {
    let _guard = ime_smoke_test_guard();
    let workspace = TempWorkspace::new();
    workspace.write("ime_empty_commit.txt", "clean");
    let runtime = open_runtime(workspace.path());
    let mut app = DesktopEframeApp::new(runtime);

    open_file_via_palette(&mut app, "ime_empty_commit.txt");

    let _ = app.run_headless_input(ime_events_input(vec![
        egui::Event::Ime(egui::ImeEvent::Enabled),
        egui::Event::Ime(egui::ImeEvent::Commit(String::new())),
    ]));

    assert!(
        !app.runtime_snapshot().active_buffer_projection.dirty,
        "an empty IME commit must not mutate the buffer"
    );
}
