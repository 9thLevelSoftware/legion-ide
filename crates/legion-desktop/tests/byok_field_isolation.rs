//! Checklist row 3: typing an API key must not reach the editor buffer.
//!
//! Never exercised in a windowed session, and it is the row with teeth. The
//! 2026-08-17 journal records D2: `handle_keyboard` disabled editor input
//! whenever *any* widget held egui keyboard focus, because egui gives focus to
//! plain buttons. The fix narrowed that to `Context::text_edit_focused()`.
//!
//! That fix has a mirror-image failure mode nothing checks: if the editor were
//! to keep taking keystrokes while a text field has focus, every character of
//! an API key typed into the BYOK box would also be inserted into whatever file
//! is open — and then saved to disk, and then committed. A secret in the
//! keyring is the design; a secret in `main.rs` is a breach.
//!
//! These tests type a key-shaped string into the real rendered field and assert
//! it went exactly one place.

use std::path::Path;

mod common;
use common::{TempWorkspace, click_at, clickable_center, full_frame_input, rendered_text};

use legion_desktop::workflow::{DesktopEframeApp, DesktopLaunchConfig, DesktopRuntime};

/// A key-shaped string that is not a real credential.
const FAKE_KEY: &str = "sk-ant-notarealkey-0123456789";

fn open_app(root: &Path) -> DesktopEframeApp {
    let runtime = DesktopRuntime::open(DesktopLaunchConfig::new(root.to_path_buf(), None))
        .expect("desktop runtime should open workspace");
    DesktopEframeApp::new(runtime)
}

fn require(output: &egui::FullOutput, label: &str) -> egui::Pos2 {
    clickable_center(output, label)
        .unwrap_or_else(|| panic!("no clickable control labelled `{label}` in the rendered frame"))
}

/// Type `text` one character at a time, as a keyboard does.
fn type_text(app: &mut DesktopEframeApp, text: &str) -> egui::FullOutput {
    let mut output = app.run_headless_full_frame(full_frame_input(Vec::new()));
    for character in text.chars() {
        output = app.run_headless_full_frame(full_frame_input(vec![egui::Event::Text(
            character.to_string(),
        )]));
    }
    output
}

/// Open a file, reach Settings → AI Providers, and focus the BYOK field.
fn app_with_byok_focused(workspace: &TempWorkspace) -> DesktopEframeApp {
    let mut app = open_app(workspace.path());

    // Open a buffer first: the point of the test is that keystrokes do not
    // reach it, which cannot be observed if nothing is open.
    let primed = app.run_headless_full_frame(full_frame_input(Vec::new()));
    let file = require(&primed, "target.txt");
    let opened = click_at(&mut app, file);

    let settings = require(&opened, "Settings");
    let in_settings = click_at(&mut app, settings);
    let providers = require(&in_settings, "AI Providers");
    let on_providers = click_at(&mut app, providers);

    // The key box is the password field; egui exposes it without a label, so it
    // is reached by clicking the control next to the section's known text.
    let field = clickable_center(&on_providers, "Save Anthropic key")
        .expect("the AI Providers section must offer the BYOK form");
    // Click just above the Save button to land in the text field itself.
    let _ = click_at(&mut app, egui::pos2(field.x, field.y - 34.0));
    app
}

/// What the BYOK field currently holds.
///
/// Read from egui's temp store because the field is a password `TextEdit` and
/// does not surface as a `TextInput` in the accessibility tree — so there is no
/// way to confirm from the rendered output that a keystroke landed in it.
///
/// Without this, every test here would pass just as well if the click missed
/// the field entirely and the typing went nowhere. That is the difference
/// between "the key did not reach the buffer" and "nothing happened at all",
/// and only one of them is the property worth guarding.
fn byok_draft(app: &DesktopEframeApp) -> String {
    let draft_id = egui::Id::new("legion-byok-anthropic-draft");
    app.headless_egui_context()
        .data_mut(|data| data.get_temp::<String>(draft_id))
        .unwrap_or_default()
}

fn buffer_text(app: &DesktopEframeApp) -> String {
    app.runtime_snapshot()
        .active_buffer_projection
        .small_buffer_preview
        .unwrap_or_default()
}

#[test]
fn typing_an_api_key_never_reaches_the_open_buffer() {
    let workspace = TempWorkspace::new("legion_desktop_byok_isolation");
    workspace.write("target.txt", "original contents\n");
    let mut app = app_with_byok_focused(&workspace);

    let before = buffer_text(&app);
    let _ = type_text(&mut app, FAKE_KEY);
    let after = buffer_text(&app);

    // First prove the typing went somewhere, or the assertion below is vacuous.
    assert_eq!(
        byok_draft(&app),
        FAKE_KEY,
        "the key never reached the BYOK field, so this test proves nothing about where keystrokes go. The field is focused by clicking above the Save button; if that layout moved, fix the click rather than the assertion."
    );

    assert_eq!(
        before, after,
        "typing into the BYOK field changed the open buffer. Every character of \
         an API key would be inserted into the file, saved, and committed."
    );
    assert!(
        !after.contains("sk-ant"),
        "the buffer contains an API key prefix after typing into the key field"
    );
}

#[test]
fn a_typed_api_key_is_not_exposed_as_readable_text() {
    let workspace = TempWorkspace::new("legion_desktop_byok_isolation");
    workspace.write("target.txt", "original contents\n");
    let mut app = app_with_byok_focused(&workspace);

    let output = type_text(&mut app, FAKE_KEY);
    assert_eq!(
        byok_draft(&app),
        FAKE_KEY,
        "the key never reached the BYOK field, so there is nothing to leak and the assertion below would pass for the wrong reason"
    );

    // The field is a password field, so the accessibility tree must not carry
    // the secret in clear text. A screen reader reading it aloud, or a
    // diagnostics dump capturing it, is the same leak by a different route.
    let exposed: Vec<String> = rendered_text(&output)
        .into_iter()
        .filter(|text| text.contains(FAKE_KEY) || text.contains("sk-ant-notarealkey"))
        .collect();
    assert!(
        exposed.is_empty(),
        "the typed API key appears in the accessibility tree in clear text: {exposed:?}"
    );
}

#[test]
fn the_editor_still_accepts_typing_when_no_field_has_focus() {
    // The other half of D2, and the reason that fix could not simply be "ignore
    // keys whenever anything has focus": egui hands focus to plain buttons, and
    // the original guard left the editor permanently deaf after one click. A
    // test that only asserted isolation would pass on a completely dead editor.
    let workspace = TempWorkspace::new("legion_desktop_byok_isolation");
    workspace.write("target.txt", "original contents\n");
    let mut app = open_app(workspace.path());

    let primed = app.run_headless_full_frame(full_frame_input(Vec::new()));
    let file = require(&primed, "target.txt");
    let opened = click_at(&mut app, file);

    // Click into the editor body, well below the tab strip.
    let editor = egui::pos2(700.0, 400.0);
    let _ = click_at(&mut app, editor);
    let before = buffer_text(&app);
    let _ = type_text(&mut app, "XYZ");
    let after = buffer_text(&app);

    assert_ne!(
        before,
        after,
        "the editor accepted no typing with no text field focused, which is the \
         defect D2 produced: one click on a button and every later keystroke \
         was discarded, indefinitely, with nothing on screen to explain it. \
         Rendered frame had: {:?}",
        rendered_text(&opened).len()
    );
}
