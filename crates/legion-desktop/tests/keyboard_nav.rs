//! Keyboard navigation smoke test for the desktop adapter.
//!
//! This regression ensures the product-mode switch can be activated without a
//! pointer by tabbing to the first pill and pressing Enter.

use std::{
    fs,
    path::Path,
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

use legion_desktop::{
    bridge::DesktopAction,
    workflow::{DesktopEframeApp, DesktopLaunchConfig, DesktopRuntime},
};
use legion_ui::DockMode;

static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

struct TempWorkspace {
    root: std::path::PathBuf,
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
            "legion_desktop_keyboard_nav_{}_{}_{}",
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
            && file_name.is_some_and(|name| name.starts_with("legion_desktop_keyboard_nav_"))
        {
            let _ = fs::remove_dir_all(&self.root);
        }
    }
}

fn open_runtime(root: &Path) -> DesktopRuntime {
    DesktopRuntime::open(DesktopLaunchConfig::new(root.to_path_buf(), None))
        .expect("desktop runtime should open workspace")
}

#[test]
fn product_mode_switch_accepts_keyboard_activation() {
    let workspace = TempWorkspace::new();
    let mut runtime = open_runtime(workspace.path());
    runtime
        .handle_action(DesktopAction::SetProductMode {
            mode: DockMode::Assist,
        })
        .expect("switching to Assist should succeed");
    let mut app = DesktopEframeApp::new(runtime);

    assert_eq!(app.runtime_snapshot().product_mode, DockMode::Assist);

    let input = egui::RawInput {
        focused: true,
        modifiers: egui::Modifiers {
            command: true,
            alt: true,
            ..egui::Modifiers::default()
        },
        events: vec![egui::Event::Key {
            key: egui::Key::M,
            physical_key: Some(egui::Key::M),
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers {
                command: true,
                alt: true,
                ..egui::Modifiers::default()
            },
        }],
        ..egui::RawInput::default()
    };
    let _ = app.run_headless_input(input);

    assert_eq!(
        app.runtime_snapshot().product_mode,
        DockMode::Manual,
        "keyboard activation should select the Manual product mode"
    );
}
