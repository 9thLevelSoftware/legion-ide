//! The published keymap: what each entry means, and who owns it.
//!
//! Split out of `view.rs` rather than grown there. The three pieces belong
//! together because they answer one question in sequence -- which action a
//! binding names, whether the surface on screen owns that action, and whether
//! to dispatch it -- and the gap between the second and third is where an
//! editor shortcut reached a buffer nobody was looking at.

use super::*;

/// Map a keybinding action label to a `DesktopAction`, if applicable.
///
/// Context-dependent actions like GoToDefinition are resolved from the current
/// projection here so the default keymap remains the single source of truth.
pub(crate) fn action_label_to_desktop_action(
    label: &str,
    snapshot: &ShellProjectionSnapshot,
) -> Option<DesktopAction> {
    match label {
        "SaveActive" => Some(DesktopAction::SaveActive),
        "SaveAll" => Some(DesktopAction::SaveAll),
        // Preserve the existing Ctrl/Cmd+F search-palette behavior while
        // routing it through the published keymap entry. The in-editor find
        // bar remains available through its explicit UI action.
        "ToggleFindBar" => Some(DesktopAction::OpenPalette {
            mode: PaletteMode::Search,
            query: "/".to_string(),
            scope: SearchScopeProjection::ActiveFile,
        }),
        "ToggleFindReplace" => Some(DesktopAction::ToggleFindReplace),
        "FindNext" => Some(DesktopAction::FindNext),
        "FindPrevious" => Some(DesktopAction::FindPrevious),
        "Undo" => Some(DesktopAction::Undo),
        "Redo" => Some(DesktopAction::Redo),
        "AddCursorAbove" => Some(DesktopAction::AddCursorAbove { buffer_id: None }),
        "AddCursorBelow" => Some(DesktopAction::AddCursorBelow { buffer_id: None }),
        "GoToDefinition" => Some(DesktopAction::GoToDefinition {
            position: projected_cursor(snapshot),
        }),
        "GoToLine" => Some(DesktopAction::OpenPalette {
            mode: PaletteMode::Command,
            query: "Go to line".to_string(),
            scope: SearchScopeProjection::ActiveFile,
        }),
        "OpenPalette" => Some(DesktopAction::OpenPalette {
            mode: PaletteMode::File,
            query: String::new(),
            scope: SearchScopeProjection::ActiveFile,
        }),
        "OpenCommandPalette" => Some(DesktopAction::OpenPalette {
            mode: PaletteMode::Command,
            query: String::new(),
            scope: SearchScopeProjection::ActiveFile,
        }),
        "CloseTab" => active_buffer_for_keybinding(snapshot)
            .map(|buffer_id| DesktopAction::CloseTab { buffer_id }),
        "NextTab" => adjacent_tab_for_keybinding(snapshot, 1)
            .map(|buffer_id| DesktopAction::SwitchTab { buffer_id }),
        "PrevTab" => adjacent_tab_for_keybinding(snapshot, -1)
            .map(|buffer_id| DesktopAction::SwitchTab { buffer_id }),
        "ProblemNext" => Some(DesktopAction::ProblemNext),
        "ProblemPrev" => Some(DesktopAction::ProblemPrev),
        "DebugStart" => {
            if let Some(session_id) = snapshot.debug_projection.active_session_id.clone() {
                Some(DesktopAction::DebugStep {
                    session_id,
                    kind: legion_ui::DebugStepKindProjection::Continue,
                })
            } else if let Some(configuration_id) = snapshot
                .debug_projection
                .configurations
                .first()
                .map(|config| config.configuration_id.clone())
            {
                Some(DesktopAction::LaunchDebugSession { configuration_id })
            } else {
                Some(DesktopAction::RefreshExplorer)
            }
        }
        "DebugStop" => snapshot
            .debug_projection
            .active_session_id
            .clone()
            .map(|_| DesktopAction::StopDebugSession),
        "ToggleBreakpoint" => {
            active_buffer_for_keybinding(snapshot).map(|_| DesktopAction::ToggleDebugBreakpoint {
                line: projected_cursor(snapshot).line,
                condition: None,
                hit_condition: None,
                log_message: None,
            })
        }
        "DebugStepOver" => snapshot
            .debug_projection
            .active_session_id
            .clone()
            .map(|session_id| DesktopAction::DebugStep {
                session_id,
                kind: legion_ui::DebugStepKindProjection::Over,
            }),
        "DebugStepInto" => snapshot
            .debug_projection
            .active_session_id
            .clone()
            .map(|session_id| DesktopAction::DebugStep {
                session_id,
                kind: legion_ui::DebugStepKindProjection::Into,
            }),
        "DebugStepOut" => snapshot
            .debug_projection
            .active_session_id
            .clone()
            .map(|session_id| DesktopAction::DebugStep {
                session_id,
                kind: legion_ui::DebugStepKindProjection::Out,
            }),
        _ => None,
    }
}

/// Whether an action's whole effect happens inside the editor view.
///
/// These mutate the active buffer or move a cursor through it, and none of it
/// is visible from another centre surface. The canvas gate on text input closed
/// one route to that and left this one open: the keymap dispatcher runs before
/// any editor-specific handling, so Ctrl/Cmd+Z went on rewriting a file that
/// was not on screen. An invisible edit is the worst shape a keyboard defect
/// can take -- nothing looks wrong until the file is saved.
///
/// Saving is deliberately not in this set. It writes what is already there
/// rather than changing it, the dirty marker is visible from any surface, and
/// wanting to save while looking at the canvas is an ordinary thing to want.
pub(crate) fn action_is_editor_scoped(action: &DesktopAction) -> bool {
    matches!(
        action,
        DesktopAction::Undo
            | DesktopAction::Redo
            | DesktopAction::AddCursorAbove { .. }
            | DesktopAction::AddCursorBelow { .. }
            | DesktopAction::FindNext
            | DesktopAction::FindPrevious
            | DesktopAction::ToggleFindReplace
            // F12 and F9. Both are published keymap entries, both act on the
            // buffer's cursor line, and both were missing from the first
            // version of this list -- which mattered more than the omission
            // itself, because ADR-0051 and the dependency-policy entry promise
            // that no editor input reaches a buffer while the canvas is
            // showing. That is a claim about the whole set, and a set with two
            // holes in it makes the document wrong rather than merely
            // incomplete. A breakpoint landing on a file nobody is looking at
            // fails exactly the way `GoToDefinition` does: silently.
            | DesktopAction::GoToDefinition { .. }
            | DesktopAction::ToggleDebugBreakpoint { .. }
    )
}

/// Central keyboard dispatch from `default_keymap()`.
///
/// Reads the keymap bindings and checks each combo against egui's current
/// input.  Matched actions are pushed to `actions`.  This runs BEFORE existing
/// hardcoded key checks so the keymap takes precedence for non-context-dependent
/// actions.
pub(crate) fn dispatch_keybindings(
    ctx: &egui::Context,
    snapshot: &ShellProjectionSnapshot,
    editor_input_enabled: bool,
    actions: &mut Vec<DesktopAction>,
) {
    let bindings = legion_ui::ui::default_keymap();
    ctx.input(|input| {
        for binding in &bindings {
            let Some(key) = key_label_to_egui(&binding.combo.key) else {
                continue;
            };
            if !input.key_pressed(key) {
                continue;
            }
            // The keymap's `ctrl` flag represents the platform command
            // modifier. `egui::Modifiers::command` maps to Ctrl on Windows/
            // Linux and Cmd on macOS, while `ctrl` is only the physical Ctrl
            // key and would make the default map fail for Cmd-based input.
            if binding.combo.ctrl != input.modifiers.command {
                continue;
            }
            if binding.combo.shift != input.modifiers.shift {
                continue;
            }
            if binding.combo.alt != input.modifiers.alt {
                continue;
            }
            if let Some(action) = action_label_to_desktop_action(&binding.action_label, snapshot)
                && (editor_input_enabled || !action_is_editor_scoped(&action))
            {
                actions.push(action);
            }
        }
    });
}
