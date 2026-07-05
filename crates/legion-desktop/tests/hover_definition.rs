//! TDD tests for PKT-LSP-B T7: hover tooltip + go-to-definition.
//!
//! Verifies the hover debounce state machine and go-to-definition navigation
//! through the desktop runtime without starting a real LSP server.

use std::{
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

use legion_desktop::{
    bridge::DesktopAction,
    workflow::{DesktopLaunchConfig, DesktopRuntime, DesktopWorkflowOutcome},
};
use legion_protocol::{BufferId, TextCoordinate};

static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

struct TempWorkspace {
    root: PathBuf,
}

impl TempWorkspace {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "legion-desktop-hover-def-{}-{}",
            std::process::id(),
            TEMP_COUNTER.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&root).expect("create temp workspace");
        Self { root }
    }

    fn path(&self) -> &Path {
        &self.root
    }

    fn write(&self, name: &str, content: &str) -> PathBuf {
        let path = self.root.join(name);
        fs::write(&path, content).expect("write workspace file");
        path
    }
}

impl Drop for TempWorkspace {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn open_runtime(root: &Path, file: &Path) -> DesktopRuntime {
    DesktopRuntime::open(DesktopLaunchConfig::new(
        root.to_path_buf(),
        Some(file.to_string_lossy().into_owned()),
    ))
    .expect("open desktop runtime")
}

fn position(line: u32, character: u32) -> TextCoordinate {
    TextCoordinate {
        line,
        character,
        byte_offset: None,
        utf16_offset: None,
    }
}

/// Inject a synthetic hover result directly via the test accessor.
fn inject_hover(runtime: &mut DesktopRuntime, label: &str) -> BufferId {
    let snapshot = runtime.projection_snapshot();
    let buffer_id = snapshot
        .active_buffer_projection
        .buffer_id
        .expect("active buffer must exist");

    let raw_response = serde_json::json!({
        "contents": {
            "kind": "markdown",
            "value": label
        },
        "range": {
            "start": { "line": 0, "character": 0 },
            "end": { "line": 0, "character": 3 }
        }
    });
    runtime
        .app_mut_for_test()
        .ingest_lsp_hover_response_for_buffer(buffer_id, &raw_response, None)
        .expect("inject hover");

    buffer_id
}

// ─── HoverDismiss ────────────────────────────────────────────────────────────

/// `HoverDismiss` with no hover data is a Noop and does not panic.
#[test]
fn hover_dismiss_with_no_hover_is_noop() {
    let ws = TempWorkspace::new();
    let file = ws.write("lib.rs", "fn x() {}\n");
    let mut runtime = open_runtime(ws.path(), &file);

    let outcome = runtime
        .handle_action(DesktopAction::HoverDismiss)
        .expect("dismiss");
    assert_eq!(outcome, DesktopWorkflowOutcome::Noop);
    assert!(!runtime.hover_tooltip_visible_for_test());
}

/// `HoverDismiss` closes the tooltip when it was open.
#[test]
fn hover_dismiss_closes_open_tooltip() {
    let ws = TempWorkspace::new();
    let file = ws.write("lib.rs", "fn foo() {}\n");
    let mut runtime = open_runtime(ws.path(), &file);

    inject_hover(&mut runtime, "fn foo");
    runtime.set_hover_tooltip_visible_for_test(true);
    assert!(runtime.hover_tooltip_visible_for_test());

    let outcome = runtime
        .handle_action(DesktopAction::HoverDismiss)
        .expect("dismiss");
    assert_eq!(outcome, DesktopWorkflowOutcome::Noop);
    assert!(
        !runtime.hover_tooltip_visible_for_test(),
        "tooltip must close after HoverDismiss"
    );
}

// ─── Hover auto-show ─────────────────────────────────────────────────────────

/// When hover data is injected and a refresh occurs, the tooltip becomes visible.
#[test]
fn hover_tooltip_shows_when_hover_data_arrives() {
    let ws = TempWorkspace::new();
    let file = ws.write("lib.rs", "fn bar() {}\n");
    let mut runtime = open_runtime(ws.path(), &file);

    assert!(!runtime.hover_tooltip_visible_for_test());

    // Inject hover and trigger a refresh (via HoverDismiss which calls refresh_projection).
    inject_hover(&mut runtime, "fn bar");
    // The tooltip auto-shows when hover data arrives and tooltip is not visible.
    // Dismiss will call refresh_projection which reads the new hover.
    // But dismiss will also clear hover_tooltip_visible... let's use a
    // side-effect-free action (CompletionDismiss) to trigger refresh_projection.
    runtime
        .handle_action(DesktopAction::CompletionDismiss)
        .expect("trigger refresh");
    assert!(
        runtime.hover_tooltip_visible_for_test(),
        "tooltip must auto-show when hover data is present and tooltip was not visible"
    );
}

// ─── Hover dismissed on tab switch ───────────────────────────────────────────

/// Switching tabs dismisses the hover tooltip.
#[test]
fn hover_tooltip_dismissed_on_tab_switch() {
    let ws = TempWorkspace::new();
    let file_a = ws.write("a.rs", "fn a() {}\n");
    let file_b = ws.write("b.rs", "fn b() {}\n");
    let mut runtime = open_runtime(ws.path(), &file_a);

    inject_hover(&mut runtime, "fn a");
    runtime.set_hover_tooltip_visible_for_test(true);
    assert!(runtime.hover_tooltip_visible_for_test());

    runtime
        .handle_action(DesktopAction::OpenPathText(
            file_b.to_string_lossy().into_owned(),
        ))
        .expect("open b.rs");

    let snapshot = runtime.projection_snapshot();
    let b_buffer_id = snapshot
        .daily_editing_projection
        .tabs
        .tabs
        .iter()
        .find(|tab| {
            tab.file_path
                .as_ref()
                .map(|p| p.0.contains("b.rs"))
                .unwrap_or(false)
        })
        .map(|tab| tab.buffer_id);

    if let Some(b_buf) = b_buffer_id {
        runtime
            .handle_action(DesktopAction::SwitchTab { buffer_id: b_buf })
            .expect("switch to b.rs");
        assert!(
            !runtime.hover_tooltip_visible_for_test(),
            "hover tooltip must be dismissed after tab switch"
        );
    }
}

// ─── GoToDefinition / NavigateToDefinition ───────────────────────────────────

/// `NavigateToDefinition` with no definitions returns Noop (guard condition).
#[test]
fn navigate_to_definition_with_no_definitions_is_noop() {
    let ws = TempWorkspace::new();
    let file = ws.write("lib.rs", "fn main() {}\n");
    let mut runtime = open_runtime(ws.path(), &file);

    let outcome = runtime
        .handle_action(DesktopAction::NavigateToDefinition { index: 0 })
        .expect("navigate noop");
    assert_eq!(outcome, DesktopWorkflowOutcome::Noop);
}

/// `GoToDefinition` action flags a pending navigation; the flag is observable.
#[test]
fn go_to_definition_action_fires_language_tooling_request() {
    let ws = TempWorkspace::new();
    let file = ws.write("lib.rs", "fn main() {}\n");
    let mut runtime = open_runtime(ws.path(), &file);

    // GoToDefinition dispatches through bridge → language tooling; even without
    // a live server, the outcome is LanguageToolingUpdated (pending operation).
    let outcome = runtime
        .handle_action(DesktopAction::GoToDefinition {
            position: position(0, 3),
        })
        .expect("go to definition action");
    assert_eq!(
        outcome,
        DesktopWorkflowOutcome::LanguageToolingUpdated,
        "GoToDefinition must produce LanguageToolingUpdated"
    );
}

/// `RequestHover` action dispatches through bridge even without a live server.
#[test]
fn request_hover_action_fires_language_tooling_request() {
    let ws = TempWorkspace::new();
    let file = ws.write("lib.rs", "fn main() {}\n");
    let mut runtime = open_runtime(ws.path(), &file);

    let outcome = runtime
        .handle_action(DesktopAction::RequestHover {
            position: position(0, 0),
        })
        .expect("request hover");
    assert_eq!(
        outcome,
        DesktopWorkflowOutcome::LanguageToolingUpdated,
        "RequestHover must produce LanguageToolingUpdated"
    );
}
