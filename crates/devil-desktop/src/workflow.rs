//! Desktop runtime workflow boundary.

use anyhow::Result;

/// Renderer-backed desktop runtime placeholder.
#[derive(Debug, Default)]
pub struct DesktopRuntime {
    quit_requested: bool,
}

impl DesktopRuntime {
    /// Creates an inert runtime placeholder.
    pub fn new() -> Self {
        Self {
            quit_requested: false,
        }
    }

    /// Returns whether the adapter has requested shutdown.
    pub fn quit_requested(&self) -> bool {
        self.quit_requested
    }
}

/// Run the desktop adapter from process arguments.
pub fn run_from_env() -> Result<()> {
    let _runtime = DesktopRuntime::new();
    Ok(())
}
