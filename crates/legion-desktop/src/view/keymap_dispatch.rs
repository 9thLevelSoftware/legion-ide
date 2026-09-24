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
        "SearchWorkspace" => Some(DesktopAction::OpenPalette {
            mode: PaletteMode::Search,
            query: "/".to_string(),
            scope: SearchScopeProjection::Workspace,
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
        "RenameSymbol" => Some(DesktopAction::OpenPalette {
            mode: PaletteMode::Command,
            query: "language rename ".to_string(),
            scope: SearchScopeProjection::ActiveFile,
        }),
        "FormatDocument" => Some(DesktopAction::RequestFormattingProposal),
        "OrganizeImports" => Some(DesktopAction::RequestOrganizeImportsProposal),
        "StageFocusedGitHunk" => snapshot
            .git_projection
            .focused_hunk_id
            .as_deref()
            .and_then(|focused_id| {
                snapshot
                    .git_projection
                    .hunks
                    .iter()
                    .find(|hunk| hunk.hunk_id == focused_id)
            })
            .filter(|hunk| hunk.stage == legion_ui::GitHunkStageProjection::Unstaged)
            .map(|hunk| DesktopAction::StageGitHunk {
                hunk_id: hunk.hunk_id.clone(),
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
            // Match the modifiers carried by the key event itself. The frame
            // modifier state may describe a different event in the same
            // frame, and must not merge two observations into one shortcut.
            let event_matches = input.events.iter().any(|event| {
                let egui::Event::Key {
                    key: event_key,
                    pressed: true,
                    modifiers,
                    ..
                } = event
                else {
                    return false;
                };
                *event_key == key
                    // The keymap's `ctrl` flag represents the platform
                    // command modifier. `command` is true for Ctrl on
                    // Windows/Linux and Cmd on macOS; physical `ctrl` and
                    // `mac_cmd` remain irrelevant to this logical match.
                    && binding.combo.ctrl == modifiers.command
                    && binding.combo.shift == modifiers.shift
                    && binding.combo.alt == modifiers.alt
            });
            if !event_matches {
                continue;
            }
            let action = match binding.action_label.as_str() {
                "DebugStackPrevious" => debug_stack_navigation_action(ctx, snapshot, -1),
                "DebugStackNext" => debug_stack_navigation_action(ctx, snapshot, 1),
                _ => action_label_to_desktop_action(&binding.action_label, snapshot),
            };
            if let Some(action) = action
                && (editor_input_enabled || !action_is_editor_scoped(&action))
            {
                actions.push(action);
            }
        }
    });
}

fn debug_stack_navigation_action(
    ctx: &egui::Context,
    snapshot: &ShellProjectionSnapshot,
    direction: isize,
) -> Option<DesktopAction> {
    let frame_count = snapshot.debug_projection.stack_frames.len();
    let current =
        debug_selected_stack_frame_index(ctx, frame_count.min(DEBUG_STACK_FRAME_RENDER_LIMIT));
    let next = debug_stack_navigation_index(current, frame_count, direction)?;
    set_debug_selected_stack_frame_index(ctx, next);
    snapshot
        .debug_projection
        .stack_frames
        .get(next)
        .and_then(debug_frame_navigation_action)
}

fn debug_stack_navigation_index(
    current: usize,
    total_frame_count: usize,
    direction: isize,
) -> Option<usize> {
    let frame_count = total_frame_count.min(DEBUG_STACK_FRAME_RENDER_LIMIT);
    if frame_count == 0 {
        return None;
    }
    Some(
        (current.min(frame_count - 1) as isize + direction).clamp(0, frame_count as isize - 1)
            as usize,
    )
}

#[cfg(test)]
mod tests {
    use super::{
        action_label_to_desktop_action, debug_stack_navigation_index, dispatch_keybindings,
    };
    use crate::bridge::DesktopAction;
    use legion_ui::{
        GitHunkProjection, GitHunkStageProjection, PaletteMode, SearchScopeProjection, Shell,
    };

    fn dispatch_for(
        event_modifiers: egui::Modifiers,
        frame_modifiers: egui::Modifiers,
        pressed: bool,
    ) -> Vec<DesktopAction> {
        dispatch_events(
            vec![egui::Event::Key {
                key: egui::Key::S,
                physical_key: Some(egui::Key::S),
                pressed,
                repeat: false,
                modifiers: event_modifiers,
            }],
            frame_modifiers,
        )
    }

    fn dispatch_events(
        events: Vec<egui::Event>,
        frame_modifiers: egui::Modifiers,
    ) -> Vec<DesktopAction> {
        dispatch_events_with_editor(events, frame_modifiers, true)
    }

    fn dispatch_events_with_editor(
        events: Vec<egui::Event>,
        frame_modifiers: egui::Modifiers,
        editor_input_enabled: bool,
    ) -> Vec<DesktopAction> {
        let ctx = egui::Context::default();
        ctx.begin_pass(egui::RawInput {
            events,
            modifiers: frame_modifiers,
            ..Default::default()
        });
        let mut actions = Vec::new();
        dispatch_keybindings(
            &ctx,
            &Shell::empty("keymap dispatch").projection_snapshot(),
            editor_input_enabled,
            &mut actions,
        );
        let _ = ctx.end_pass();
        actions
    }

    #[test]
    fn keymap_matches_event_modifiers_exactly_for_save_chords() {
        assert_eq!(
            dispatch_for(egui::Modifiers::COMMAND, egui::Modifiers::NONE, true),
            vec![DesktopAction::SaveActive]
        );
        assert!(dispatch_for(egui::Modifiers::NONE, egui::Modifiers::COMMAND, true).is_empty());
        assert_eq!(
            dispatch_for(
                egui::Modifiers::COMMAND | egui::Modifiers::SHIFT,
                egui::Modifiers::COMMAND,
                true,
            ),
            vec![DesktopAction::SaveAll]
        );
        assert_eq!(
            dispatch_for(
                egui::Modifiers::COMMAND,
                egui::Modifiers::COMMAND | egui::Modifiers::SHIFT,
                true,
            ),
            vec![DesktopAction::SaveActive]
        );
        assert!(dispatch_for(egui::Modifiers::COMMAND, egui::Modifiers::NONE, false).is_empty());
    }

    #[test]
    fn keymap_accepts_platform_command_variants_without_physical_ctrl_matching() {
        assert_eq!(
            dispatch_for(
                egui::Modifiers::CTRL | egui::Modifiers::COMMAND,
                egui::Modifiers::NONE,
                true
            ),
            vec![DesktopAction::SaveActive]
        );
        assert_eq!(
            dispatch_for(
                egui::Modifiers::MAC_CMD | egui::Modifiers::COMMAND,
                egui::Modifiers::NONE,
                true
            ),
            vec![DesktopAction::SaveActive]
        );
    }

    #[test]
    fn keymap_dispatches_distinct_event_chords_once_each() {
        let actions = dispatch_events(
            vec![
                egui::Event::Key {
                    key: egui::Key::S,
                    physical_key: Some(egui::Key::S),
                    pressed: true,
                    repeat: false,
                    modifiers: egui::Modifiers::COMMAND,
                },
                egui::Event::Key {
                    key: egui::Key::S,
                    physical_key: Some(egui::Key::S),
                    pressed: true,
                    repeat: false,
                    modifiers: egui::Modifiers::COMMAND | egui::Modifiers::SHIFT,
                },
            ],
            egui::Modifiers::NONE,
        );
        assert_eq!(
            actions,
            vec![DesktopAction::SaveActive, DesktopAction::SaveAll]
        );
    }

    #[test]
    fn keymap_rejects_unrelated_keys_and_modifiers() {
        let unrelated = dispatch_events(
            vec![egui::Event::Key {
                key: egui::Key::A,
                physical_key: Some(egui::Key::A),
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers::COMMAND,
            }],
            egui::Modifiers::NONE,
        );
        let extra_modifier = dispatch_for(
            egui::Modifiers::COMMAND | egui::Modifiers::ALT,
            egui::Modifiers::NONE,
            true,
        );
        assert!(unrelated.is_empty());
        assert!(extra_modifier.is_empty());
    }

    #[test]
    fn keymap_preserves_repeat_and_at_most_once_per_binding() {
        let actions = dispatch_events(
            vec![
                egui::Event::Key {
                    key: egui::Key::S,
                    physical_key: Some(egui::Key::S),
                    pressed: true,
                    repeat: true,
                    modifiers: egui::Modifiers::COMMAND,
                },
                egui::Event::Key {
                    key: egui::Key::S,
                    physical_key: Some(egui::Key::S),
                    pressed: true,
                    repeat: true,
                    modifiers: egui::Modifiers::COMMAND,
                },
            ],
            egui::Modifiers::NONE,
        );
        assert_eq!(actions, vec![DesktopAction::SaveActive]);
    }

    #[test]
    fn keymap_keeps_editor_guard_while_allowing_global_save() {
        let undo = dispatch_events_with_editor(
            vec![egui::Event::Key {
                key: egui::Key::Z,
                physical_key: Some(egui::Key::Z),
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers::COMMAND,
            }],
            egui::Modifiers::NONE,
            false,
        );
        let save = dispatch_events_with_editor(
            vec![egui::Event::Key {
                key: egui::Key::S,
                physical_key: Some(egui::Key::S),
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers::COMMAND,
            }],
            egui::Modifiers::NONE,
            false,
        );
        assert!(undo.is_empty());
        assert_eq!(save, vec![DesktopAction::SaveActive]);
    }

    #[test]
    fn debug_stack_navigation_stays_within_rendered_frame_limit() {
        assert_eq!(debug_stack_navigation_index(31, 40, 1), Some(31));
        assert_eq!(debug_stack_navigation_index(40, 40, -1), Some(30));
        assert_eq!(debug_stack_navigation_index(0, 0, 1), None);
    }

    #[test]
    fn search_workspace_keybinding_opens_workspace_search_palette() {
        let snapshot = Shell::empty("workspace search").projection_snapshot();
        assert!(matches!(
            action_label_to_desktop_action("SearchWorkspace", &snapshot),
            Some(DesktopAction::OpenPalette {
                mode: PaletteMode::Search,
                query,
                scope: SearchScopeProjection::Workspace,
            }) if query == "/"
        ));
    }

    #[test]
    fn go_to_definition_keybinding_uses_the_projected_cursor() {
        let snapshot = Shell::empty("definition").projection_snapshot();
        assert!(matches!(
            action_label_to_desktop_action("GoToDefinition", &snapshot),
            Some(DesktopAction::GoToDefinition { .. })
        ));
    }

    #[test]
    fn stage_focused_hunk_keybinding_requires_an_unstaged_focus() {
        let mut snapshot = Shell::empty("stage hunk").projection_snapshot();
        assert_eq!(
            action_label_to_desktop_action("StageFocusedGitHunk", &snapshot),
            None,
            "no focused hunk must not invent a stage action"
        );
        snapshot.git_projection.focused_hunk_id = Some("h1".to_string());
        snapshot.git_projection.hunks.push(GitHunkProjection {
            hunk_id: "h1".to_string(),
            path: "src/lib.rs".to_string(),
            stage: GitHunkStageProjection::Unstaged,
            header: "@@ -1,1 +1,1 @@".to_string(),
            old_start: 1,
            old_lines: 1,
            new_start: 1,
            new_lines: 1,
            added_lines: 1,
            deleted_lines: 1,
            submodule_dirty_only: false,
            context: None,
        });
        assert_eq!(
            action_label_to_desktop_action("StageFocusedGitHunk", &snapshot),
            Some(DesktopAction::StageGitHunk {
                hunk_id: "h1".to_string(),
            })
        );
    }
}
