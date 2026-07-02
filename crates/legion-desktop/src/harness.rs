//! Headless test harness for the desktop adapter.
//!
//! This module provides the documented seam between the headless egui/eframe
//! test path and the production desktop adapter. It exists so that tests can
//! exercise the real `DesktopEframeApp` / `egui::Context` input path without
//! requiring a real `winit` window, while making it explicit that:
//!
//! * `legion-ui` stays projection-only.
//! * `legion-desktop` is a renderer/adapter; it may own adapter-local view
//!   state but never workspace state.
//! * `legion-app` owns app authority and is the only writer of the buffer
//!   state that the projection reflects.
//!
//! # Usage
//!
//! ```no_run
//! use legion_desktop::harness::HeadlessTestWorkspace;
//!
//! let workspace = HeadlessTestWorkspace::new("my_test");
//! workspace.write_file("hello.txt", "world");
//! let mut app = workspace.open_app();
//! // Feed synthetic input through app.run_headless_input(...)
//! // Assert on app.runtime_snapshot() projections
//! ```

use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::workflow::{DesktopEframeApp, DesktopLaunchConfig, DesktopRuntime};
use legion_ui::ShellProjectionSnapshot;

static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

/// A temporary workspace for headless desktop testing.
///
/// Creates a temp directory, writes test files into it, and provides
/// a `DesktopRuntime` that opens the workspace. The workspace is
/// automatically cleaned up on drop.
pub struct HeadlessTestWorkspace {
    root: PathBuf,
    label: String,
}

impl HeadlessTestWorkspace {
    /// Create a new temporary workspace with the given label.
    ///
    /// The label is used in the temp directory name for debuggability.
    pub fn new(label: &str) -> Self {
        let temp_root = std::env::temp_dir();
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be after epoch")
            .as_nanos();
        let id = TEMP_COUNTER.fetch_add(1, Ordering::SeqCst);
        let root = temp_root.join(format!(
            "legion_harness_{}_{}_{}_{}",
            label,
            std::process::id(),
            nanos,
            id
        ));
        fs::create_dir_all(&root).expect("temp workspace should be created");
        Self {
            root,
            label: label.to_string(),
        }
    }

    /// Return the root path of the temporary workspace.
    pub fn path(&self) -> &std::path::Path {
        &self.root
    }

    /// Write a file into the temporary workspace and return its path.
    pub fn write_file(&self, name: &str, content: &str) -> PathBuf {
        let path = self.root.join(name);
        fs::write(&path, content).expect("temp file should be written");
        path
    }

    /// Open a `DesktopRuntime` for this workspace without an initial file.
    pub fn open_runtime(&self) -> DesktopRuntime {
        DesktopRuntime::open(DesktopLaunchConfig::new(self.root.clone(), None))
            .expect("desktop runtime should open workspace")
    }

    /// Open a `DesktopRuntime` for this workspace with an initial file.
    pub fn open_runtime_with_file(&self, file: &PathBuf) -> DesktopRuntime {
        DesktopRuntime::open(DesktopLaunchConfig::new(
            self.root.clone(),
            Some(file.to_string_lossy().into_owned()),
        ))
        .expect("desktop runtime should open workspace and file")
    }

    /// Open a `DesktopEframeApp` for headless input testing.
    pub fn open_app(&self) -> DesktopEframeApp {
        let runtime = self.open_runtime();
        DesktopEframeApp::new(runtime)
    }

    /// Get the current projection snapshot from a runtime.
    pub fn snapshot(runtime: &DesktopRuntime) -> ShellProjectionSnapshot {
        runtime.projection_snapshot()
    }
}

impl Drop for HeadlessTestWorkspace {
    fn drop(&mut self) {
        if self.root.exists() {
            let _ = fs::remove_dir_all(&self.root);
        }
    }
}

impl std::fmt::Debug for HeadlessTestWorkspace {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HeadlessTestWorkspace")
            .field("label", &self.label)
            .field("root", &self.root)
            .finish()
    }
}
