//! Checklist row 2: edit a file, save it, and survive an external overwrite.
//!
//! The row was never driven through the rendered UI. `save_all_conflict.rs`
//! covers the same authority through `runtime.handle_action`, which cannot see
//! whether a person can reach any of it: whether typing marks the tab, whether
//! the published Ctrl/Cmd+S binding actually dispatches, or what the shell puts
//! on screen when a save is refused.
//!
//! It could not see the last one in particular. The refusal path rendered
//! `format!("Save rejected: {response:?}")` — about fifteen hundred characters
//! of lifecycle ids, version preconditions, fingerprint hashes and the
//! extended-length Windows path — and a projection test asserting the *outcome*
//! is right passes over that without noticing.
//!
//! The safety property is the one that matters most here and it did hold: a
//! save that would overwrite someone else's write is refused, the file on disk
//! is left alone, and the edits stay in the buffer. So these tests assert the
//! refusal *and* that it is legible, because a correct refusal nobody can read
//! is a correct refusal that gets clicked past.

use std::path::Path;

mod common;
use common::{
    TempWorkspace, click_at, clickable_center, full_frame_input, node_description, rendered_text,
};

use legion_desktop::workflow::{DesktopEframeApp, DesktopLaunchConfig, DesktopRuntime};

/// What the tab announces while a buffer has unsaved changes.
const UNSAVED: &str = "Unsaved changes";

fn open_app(root: &Path) -> DesktopEframeApp {
    let runtime = DesktopRuntime::open(DesktopLaunchConfig::new(root.to_path_buf(), None))
        .expect("desktop runtime should open workspace");
    DesktopEframeApp::new(runtime)
}

/// The published save chord, as a keyboard sends it.
fn save_chord() -> egui::Event {
    egui::Event::Key {
        key: egui::Key::S,
        physical_key: Some(egui::Key::S),
        pressed: true,
        repeat: false,
        // `Modifiers::COMMAND` is Ctrl on Windows and Linux, Cmd on macOS,
        // which is what `default_keymap()`'s `ctrl` flag means.
        modifiers: egui::Modifiers::COMMAND,
    }
}

/// Open `target.txt` by clicking it, put the caret in the editor, and type.
///
/// Returns the app with the buffer dirty. The typing is proved to have landed
/// before the caller asserts anything about saving, because a mistargeted click
/// would otherwise leave every later assertion passing for the wrong reason.
fn app_with_unsaved_edit(workspace: &TempWorkspace, original: &str) -> DesktopEframeApp {
    let mut app = open_app(workspace.path());

    let primed = app.run_headless_full_frame(full_frame_input(Vec::new()));
    let file = clickable_center(&primed, "target.txt")
        .expect("the explorer must offer target.txt as a clickable row");
    let _ = click_at(&mut app, file);

    // Click into the editor body, well below the tab strip.
    let _ = click_at(&mut app, egui::pos2(700.0, 400.0));
    for character in "XYZ".chars() {
        let _ = app.run_headless_full_frame(full_frame_input(vec![egui::Event::Text(
            character.to_string(),
        )]));
    }

    // Compared as `Option`, not through `unwrap_or_default()`. A degraded
    // projection carries no `small_buffer_preview` at all, and defaulting that
    // to `""` would satisfy this `assert_ne!` without a single keystroke having
    // landed -- turning the one guard that makes the rest of the test mean
    // something into a guard that cannot fail.
    assert_eq!(
        app.runtime_snapshot()
            .active_buffer_projection
            .small_buffer_preview
            .as_deref()
            .map(|preview| preview != original),
        Some(true),
        "the typing never reached the buffer (or the buffer has no readable preview), so \
         nothing below tests saving. The editor is focused by clicking its body; if that \
         layout moved, fix the click rather than the assertion."
    );
    app
}

#[test]
fn typing_marks_the_tab_and_the_save_chord_clears_it() {
    let original = "original contents\n";
    let workspace = TempWorkspace::new("legion_desktop_save_row_2");
    workspace.write("target.txt", original);
    let mut app = app_with_unsaved_edit(&workspace, original);

    let dirty = app.run_headless_full_frame(full_frame_input(Vec::new()));
    assert_eq!(
        node_description(&dirty, "target.txt").as_deref(),
        Some(UNSAVED),
        "an edited buffer must say so on its tab; a dirty file that looks clean is how work \
         gets closed away. Frame showed {:?}",
        rendered_text(&dirty)
    );

    let _ = app.run_headless_full_frame(full_frame_input(vec![save_chord()]));
    let saved = app.run_headless_full_frame(full_frame_input(Vec::new()));

    assert_eq!(
        node_description(&saved, "target.txt"),
        None,
        "the tab still claims unsaved changes after a successful save"
    );
    assert!(
        !app.runtime_snapshot().active_buffer_projection.dirty,
        "the projection still reports the buffer dirty after a successful save"
    );

    let on_disk = std::fs::read_to_string(workspace.path().join("target.txt"))
        .expect("the file must still be readable");
    assert_ne!(
        on_disk, original,
        "Ctrl/Cmd+S left the file on disk unchanged. The binding is published in \
         `default_keymap()` and routed through `dispatch_keybindings`, so a save that changes \
         nothing means the chord never reached the dispatcher."
    );
    assert!(
        on_disk.contains("XYZ"),
        "the saved file does not contain what was typed, got {on_disk:?}"
    );
}

#[test]
fn a_save_that_would_overwrite_someone_elses_write_is_refused_and_says_so_in_words() {
    let original = "original contents\n";
    let external = "external edit\n";
    let workspace = TempWorkspace::new("legion_desktop_save_row_2_conflict");
    workspace.write("target.txt", original);
    let mut app = app_with_unsaved_edit(&workspace, original);

    // Something else rewrites the file while this buffer is dirty.
    std::fs::write(workspace.path().join("target.txt"), external)
        .expect("the external overwrite must succeed");

    let _ = app.run_headless_full_frame(full_frame_input(vec![save_chord()]));
    let refused = app.run_headless_full_frame(full_frame_input(Vec::new()));

    // The safety property first: nothing was lost on either side.
    assert_eq!(
        std::fs::read_to_string(workspace.path().join("target.txt"))
            .expect("the file must still be readable"),
        external,
        "the save overwrote a change made outside the editor"
    );
    assert!(
        app.runtime_snapshot().active_buffer_projection.dirty,
        "the buffer was marked clean by a save that did not happen, so the edits would be \
         discarded on close with no prompt"
    );
    assert_eq!(
        node_description(&refused, "target.txt").as_deref(),
        Some(UNSAVED),
        "the tab must keep announcing unsaved changes after a refused save"
    );

    // Then that a person can tell what happened. The shell renders a status row
    // as severity heading plus body, so the two halves arrive as separate nodes.
    let texts = rendered_text(&refused);
    assert!(
        texts.iter().any(|text| text.contains("Save rejected")),
        "nothing on screen said the save was rejected; frame showed {texts:?}"
    );
    let notice = texts
        .iter()
        .find(|text| text.contains("still in the editor"))
        .unwrap_or_else(|| {
            panic!(
                "a refusal has to say the edits survived -- that is the first thing someone \
                 needs to know, and the part a structure dump buries. Frame showed {texts:?}"
            )
        });
    assert!(
        notice.contains("target.txt"),
        "the refusal does not name the file it is about: {notice:?}"
    );
    assert!(
        notice.contains("changed on disk"),
        "the refusal does not say why the save did not happen: {notice:?}"
    );

    // The message must be prose, not a `Debug` rendering of the response. Each
    // of these markers appeared in the fifteen-hundred-character blob this
    // replaced, and each is internal structure the product should not publish.
    for leak in [
        "ProposalId(",
        "CorrelationId",
        "CausalityId",
        "ProposalLifecycleTransition",
        "ProposalStaleContext",
        "FileFingerprint",
        "PrincipalId",
        "CapabilityId",
        r"\\?\",
    ] {
        assert!(
            !notice.contains(leak),
            "the refusal notice is leaking internal structure ({leak:?}): {notice:?}"
        );
    }
}
