//! Smoke tests for clipboard operations (copy, cut, paste).
//!
//! These tests verify that clipboard actions flow through the desktop bridge to
//! the correct `CommandDispatchIntent` variants:
//!
//! * `ClipboardPaste { text, at }` → `CommandDispatchIntent::Insert` (same path
//!   as `InsertText` and `ImeCommit`).
//! * `ClipboardCopy` → `CommandDispatchIntent::ClipboardCopy`.
//! * `ClipboardCut`  → `CommandDispatchIntent::ClipboardCut`.
//!
//! The bridge-level tests use `DesktopRuntime::handle_action` directly, which is
//! the same entry point the eframe app delegates to.  The headless tests push
//! synthetic egui events through `DesktopEframeApp::run_headless_input` so the
//! keyboard handler runs the production code path.

use std::{
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    sync::{Mutex, OnceLock},
    time::{SystemTime, UNIX_EPOCH},
};

use legion_desktop::{
    bridge::DesktopAction,
    workflow::{DesktopEframeApp, DesktopLaunchConfig, DesktopRuntime, DesktopWorkflowOutcome},
};
use legion_protocol::{BufferId, ProtocolTextRange, TextCoordinate};

static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);
static TEST_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

fn clipboard_smoke_test_guard() -> std::sync::MutexGuard<'static, ()> {
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
            "legion_desktop_clipboard_smoke_{}_{}_{}",
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
            && file_name.is_some_and(|name| name.starts_with("legion_desktop_clipboard_smoke_"))
        {
            let _ = fs::remove_dir_all(&self.root);
        }
    }
}

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

fn open_runtime_with_file(root: &Path, file: &Path) -> DesktopRuntime {
    DesktopRuntime::open(DesktopLaunchConfig::new(
        root.to_path_buf(),
        Some(file.to_string_lossy().into_owned()),
    ))
    .expect("desktop runtime should open workspace and file")
}

fn open_runtime(root: &Path) -> DesktopRuntime {
    DesktopRuntime::open(DesktopLaunchConfig::new(root.to_path_buf(), None))
        .expect("desktop runtime should open workspace")
}

fn tab_buffers(runtime: &DesktopRuntime) -> Vec<BufferId> {
    runtime
        .projection_snapshot()
        .daily_editing_projection
        .tabs
        .tabs
        .iter()
        .map(|tab| tab.buffer_id)
        .collect()
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

fn paste_events_input(text: &str) -> egui::RawInput {
    egui::RawInput {
        focused: true,
        events: vec![egui::Event::Paste(text.to_string())],
        ..egui::RawInput::default()
    }
}

fn copy_events_input() -> egui::RawInput {
    egui::RawInput {
        focused: true,
        events: vec![egui::Event::Copy],
        ..egui::RawInput::default()
    }
}

// ---------------------------------------------------------------------------
// Bridge-level tests (DesktopRuntime::handle_action)
// ---------------------------------------------------------------------------

/// Clipboard paste inserts text at a projected coordinate.
///
/// `ClipboardPaste` maps to `CommandDispatchIntent::Insert`, the same path as
/// `InsertText` and `ImeCommit`.  The expected outcome is `Edited`.
#[test]
fn clipboard_paste_inserts_text_through_bridge() {
    let workspace = TempWorkspace::new();
    let file = workspace.write("paste_bridge.txt", "hello");
    let mut runtime = open_runtime_with_file(workspace.path(), &file);

    let outcome = runtime
        .handle_action(DesktopAction::ClipboardPaste {
            text: " world".to_string(),
            at: coord(0, 5, 5),
        })
        .expect("ClipboardPaste should route through app authority");

    assert_eq!(
        outcome,
        DesktopWorkflowOutcome::Edited,
        "ClipboardPaste should produce Edited outcome"
    );

    let snapshot = runtime.projection_snapshot();
    assert!(snapshot.active_buffer_projection.dirty);
    assert_eq!(
        snapshot.active_buffer_projection.small_buffer_text(),
        Some("hello world"),
        "pasted text should appear in the buffer"
    );
}

/// Clipboard copy produces `ClipboardUpdated` metadata.
///
/// `ClipboardCopy` maps to `CommandDispatchIntent::ClipboardCopy`.  The adapter
/// never exposes the copied text in outcomes; it returns metadata (buffer id,
/// byte length, line count).
#[test]
fn clipboard_copy_produces_metadata_outcome() {
    let workspace = TempWorkspace::new();
    let file = workspace.write("copy_bridge.txt", "select me");
    let mut runtime = open_runtime_with_file(workspace.path(), &file);
    let buffer_id = tab_buffers(&runtime)[0];

    // Select the word "select" (bytes 0..6).
    runtime
        .handle_action(DesktopAction::SetSelection {
            buffer_id: Some(buffer_id),
            range: range(0, 6),
        })
        .expect("selection should be set");

    let outcome = runtime
        .handle_action(DesktopAction::ClipboardCopy)
        .expect("ClipboardCopy should route through app authority");

    match outcome {
        DesktopWorkflowOutcome::ClipboardUpdated {
            buffer_id: out_buf,
            byte_len,
            line_count,
            cut,
        } => {
            assert_eq!(out_buf, buffer_id);
            assert_eq!(byte_len, 6, "copied region should be 6 bytes");
            assert_eq!(line_count, 1, "single-line selection");
            assert!(!cut, "copy should not set the cut flag");
        }
        other => panic!("expected ClipboardUpdated, got {other:?}"),
    }

    // Copy must not dirty the buffer.
    assert!(
        !runtime.projection_snapshot().active_buffer_projection.dirty,
        "copy should not mutate the buffer"
    );
}

/// Clipboard cut produces `ClipboardUpdated` with `cut: true` and dirties the
/// buffer.
#[test]
fn clipboard_cut_produces_metadata_and_dirties_buffer() {
    let workspace = TempWorkspace::new();
    let file = workspace.write("cut_bridge.txt", "cut me");
    let mut runtime = open_runtime_with_file(workspace.path(), &file);
    let buffer_id = tab_buffers(&runtime)[0];

    // Select "cut" (bytes 0..3).
    runtime
        .handle_action(DesktopAction::SetSelection {
            buffer_id: Some(buffer_id),
            range: range(0, 3),
        })
        .expect("selection should be set");

    let outcome = runtime
        .handle_action(DesktopAction::ClipboardCut)
        .expect("ClipboardCut should route through app authority");

    match outcome {
        DesktopWorkflowOutcome::ClipboardUpdated {
            buffer_id: out_buf,
            cut,
            ..
        } => {
            assert_eq!(out_buf, buffer_id);
            assert!(cut, "cut should set the cut flag");
        }
        other => panic!("expected ClipboardUpdated, got {other:?}"),
    }

    assert!(
        runtime.projection_snapshot().active_buffer_projection.dirty,
        "cut should dirty the buffer (text was removed)"
    );
}

/// Clipboard paste and `InsertText` produce identical outcomes.
///
/// Both map to `CommandDispatchIntent::Insert`.  This test demonstrates
/// behavioural equivalence by inserting the same text at the same position
/// through each action on separate buffers.
#[test]
fn clipboard_paste_and_insert_text_share_insert_path() {
    let workspace = TempWorkspace::new();
    let file_paste = workspace.write("paste_equiv.txt", "base");
    let file_insert = workspace.write("insert_equiv.txt", "base");
    let at = coord(0, 4, 4);

    let mut runtime_paste = open_runtime_with_file(workspace.path(), &file_paste);
    let outcome_paste = runtime_paste
        .handle_action(DesktopAction::ClipboardPaste {
            text: "!".to_string(),
            at,
        })
        .expect("paste should succeed");

    let mut runtime_insert = open_runtime_with_file(workspace.path(), &file_insert);
    let outcome_insert = runtime_insert
        .handle_action(DesktopAction::InsertText {
            text: "!".to_string(),
            at,
        })
        .expect("insert should succeed");

    assert_eq!(
        outcome_paste, outcome_insert,
        "ClipboardPaste and InsertText must produce identical Edited outcomes"
    );
}

// ---------------------------------------------------------------------------
// Headless harness tests (egui event stream)
// ---------------------------------------------------------------------------

/// Headless paste event marks the buffer dirty.
///
/// Pushes an `egui::Event::Paste` through `run_headless_input` and verifies the
/// buffer transitions to dirty, confirming the event reached the editor through
/// the production keyboard handler.
#[test]
fn headless_paste_event_marks_buffer_dirty() {
    let _guard = clipboard_smoke_test_guard();
    let workspace = TempWorkspace::new();
    workspace.write("paste_headless.txt", "original");
    let runtime = open_runtime(workspace.path());
    let mut app = DesktopEframeApp::new(runtime);

    open_file_via_palette(&mut app, "paste_headless.txt");
    assert!(!app.runtime_snapshot().active_buffer_projection.dirty);

    let _ = app.run_headless_input(paste_events_input("pasted"));

    assert!(
        app.runtime_snapshot().active_buffer_projection.dirty,
        "pasting through the headless harness should mark the buffer dirty"
    );
}

/// Headless copy event does not dirty the buffer.
///
/// Copy is a read-only operation at the buffer level.  The buffer must remain
/// clean after a copy even when text is selected.
#[test]
fn headless_copy_event_does_not_dirty_buffer() {
    let _guard = clipboard_smoke_test_guard();
    let workspace = TempWorkspace::new();
    let _file = workspace.write("copy_headless.txt", "read only");
    let runtime = open_runtime(workspace.path());
    let mut app = DesktopEframeApp::new(runtime);

    open_file_via_palette(&mut app, "copy_headless.txt");
    assert!(!app.runtime_snapshot().active_buffer_projection.dirty);

    let _ = app.run_headless_input(copy_events_input());

    assert!(
        !app.runtime_snapshot().active_buffer_projection.dirty,
        "copy must not dirty the buffer"
    );
}

/// Empty paste text is a no-op.
///
/// The workflow code guards against empty paste text
/// (`egui::Event::Paste(text) if !text.is_empty()`).  Verify the guard.
#[test]
fn headless_empty_paste_is_noop() {
    let _guard = clipboard_smoke_test_guard();
    let workspace = TempWorkspace::new();
    workspace.write("empty_paste.txt", "untouched");
    let runtime = open_runtime(workspace.path());
    let mut app = DesktopEframeApp::new(runtime);

    open_file_via_palette(&mut app, "empty_paste.txt");
    assert!(!app.runtime_snapshot().active_buffer_projection.dirty);

    let _ = app.run_headless_input(paste_events_input(""));

    assert!(
        !app.runtime_snapshot().active_buffer_projection.dirty,
        "pasting empty text must not dirty the buffer"
    );
}
