//! Integration tests proving that `DesktopRuntime::open` wires a real
//! `FilePaletteUsageRepository` at the workspace-local `.legion/` state
//! directory, and that frequency boosts accumulated during one runtime
//! session survive a full drop-and-reopen cycle.

use std::{
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

use legion_desktop::{
    bridge::DesktopAction,
    workflow::{DesktopLaunchConfig, DesktopRuntime},
};
use legion_ui::{PaletteMode, SearchScopeProjection};

static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

struct TempWorkspace {
    root: PathBuf,
}

impl TempWorkspace {
    fn new(label: &str) -> Self {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be after epoch")
            .as_nanos();
        let id = TEMP_COUNTER.fetch_add(1, Ordering::SeqCst);
        let root =
            std::env::temp_dir().join(format!("legion_palette_persist_{label}_{}_{}", nanos, id));
        fs::create_dir(&root).expect("temp workspace should be created");
        Self { root }
    }

    fn path(&self) -> &Path {
        &self.root
    }

    fn write(&self, name: &str, content: &str) {
        fs::write(self.root.join(name), content).expect("temp file should be written");
    }
}

impl Drop for TempWorkspace {
    fn drop(&mut self) {
        if self.root.starts_with(std::env::temp_dir()) {
            let _ = fs::remove_dir_all(&self.root);
        }
    }
}

fn open_runtime(root: &Path) -> DesktopRuntime {
    DesktopRuntime::open(DesktopLaunchConfig::new(root.to_path_buf(), None))
        .expect("desktop runtime should open workspace")
}

/// Record usage of a palette command and dispatch it.
/// Opens the palette, finds the target command, moves selection to it,
/// and dispatches — which records usage and runs the command.
/// Returns the position of the target command in the result list.
fn dispatch_command_usage(runtime: &mut DesktopRuntime, query: &str, command_id: &str) -> usize {
    // Open the palette in command mode with the given query.
    runtime
        .handle_action(DesktopAction::OpenPalette {
            mode: PaletteMode::Command,
            query: query.to_string(),
            scope: SearchScopeProjection::Workspace,
        })
        .expect("open palette should succeed");

    // Find the target command position.
    let snapshot = runtime.projection_snapshot();
    let pos = snapshot
        .palette_projection
        .results
        .iter()
        .position(|r| r.id == command_id)
        .unwrap_or_else(|| panic!("{command_id} should appear in palette results for '{query}'"));

    // Move selection to the target position (palette starts at index 0).
    if pos > 0 {
        runtime
            .handle_action(DesktopAction::MovePaletteSelection { delta: pos as i32 })
            .expect("move palette selection should succeed");
    }

    // Dispatch: records usage + runs the command.
    runtime
        .handle_action(DesktopAction::DispatchPaletteSelection)
        .expect("dispatch palette selection should succeed");

    pos
}

/// Prove that `DesktopRuntime::open` wires a `FilePaletteUsageRepository` at
/// `.legion/palette_usage.json` and that a frequency boost accumulated in
/// session 1 survives drop-and-reopen into session 2.
///
/// The test mirrors the logic of the unit test
/// `palette_usage_frequency_bonus_lifts_heavily_used_command` in legion-app,
/// but exercises the PRODUCT PATH (DesktopRuntime) rather than calling
/// internal app state directly.
///
/// If `DesktopRuntime::open` does NOT call `set_palette_usage_repository()`,
/// the ranking in session 2 will revert to the alphabetical default
/// ("Refresh Explorer" < "Refresh Git") and the assertion will fail.
#[test]
fn palette_usage_persists_ranking_boost_across_desktop_runtime_restart() {
    let workspace = TempWorkspace::new("ranking");
    workspace.write("seed.rs", "fn main() {}\n");

    let palette_usage_path = workspace.path().join(".legion").join("palette_usage.json");

    // ---- Session 1: record 20 usages of "command:refresh-git" ----
    {
        let mut runtime = open_runtime(workspace.path());

        // Verify baseline: with no usage history "Refresh Explorer" ranks
        // above "Refresh Git" due to alphabetical tiebreaking ("E" < "G").
        runtime
            .handle_action(DesktopAction::OpenPalette {
                mode: PaletteMode::Command,
                query: "refresh".to_string(),
                scope: SearchScopeProjection::Workspace,
            })
            .expect("open palette");
        let baseline = runtime.projection_snapshot();
        let git_pos_base = baseline
            .palette_projection
            .results
            .iter()
            .position(|r| r.id == "command:refresh-git")
            .expect("refresh-git should appear in baseline results");
        let explorer_pos_base = baseline
            .palette_projection
            .results
            .iter()
            .position(|r| r.id == "command:refresh-explorer")
            .expect("refresh-explorer should appear in baseline results");
        assert!(
            explorer_pos_base < git_pos_base,
            "baseline: Refresh Explorer (pos {explorer_pos_base}) should rank before \
             Refresh Git (pos {git_pos_base}) before any usage is recorded"
        );
        runtime
            .handle_action(DesktopAction::ClosePalette)
            .expect("close palette");

        // Record 20 usages — each dispatch flushes the usage file to disk
        // via FilePaletteUsageRepository::record_usage.
        for _ in 0..20 {
            dispatch_command_usage(&mut runtime, "refresh", "command:refresh-git");
        }
    }
    // runtime dropped; .legion/palette_usage.json was written on each dispatch.

    // Verify the file was created by the product wiring.
    assert!(
        palette_usage_path.exists(),
        "DesktopRuntime::open must have wired FilePaletteUsageRepository; \
         .legion/palette_usage.json was not created"
    );

    // ---- Session 2: fresh runtime; ranking boost must survive ----
    {
        let mut runtime = open_runtime(workspace.path());

        // Open palette with same "refresh" query.
        runtime
            .handle_action(DesktopAction::OpenPalette {
                mode: PaletteMode::Command,
                query: "refresh".to_string(),
                scope: SearchScopeProjection::Workspace,
            })
            .expect("open palette in session 2");

        let snapshot = runtime.projection_snapshot();
        let git_pos_boosted = snapshot
            .palette_projection
            .results
            .iter()
            .position(|r| r.id == "command:refresh-git")
            .expect("refresh-git should appear in session-2 results");
        let explorer_pos_boosted = snapshot
            .palette_projection
            .results
            .iter()
            .position(|r| r.id == "command:refresh-explorer")
            .expect("refresh-explorer should appear in session-2 results");

        assert!(
            git_pos_boosted < explorer_pos_boosted,
            "after 20 usages persisted to disk and reloaded, Refresh Git (pos {git_pos_boosted}) \
             should rank above Refresh Explorer (pos {explorer_pos_boosted})"
        );
    }
}
