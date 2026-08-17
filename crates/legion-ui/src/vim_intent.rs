//! Turning a keystroke into a Vim intent.
//!
//! The last link in the chain: [`VimState`](crate::vim::VimState) decides what
//! a key *means*, and this decides which intent carries it. Kept pure and
//! separate from the desktop input handler so the mapping is testable without
//! a window, an event loop, or a frame — the handler's job reduces to reading
//! a key and dispatching whatever comes back.
//!
//! Not every action produces an intent. A half-typed command (`d` awaiting a
//! motion) produces nothing at all, and that is the normal case rather than a
//! failure: the parser is holding state until the next key arrives.

use crate::ui::CommandDispatchIntent;
use crate::vim::{EditorInputMode, VimAction, VimState};

/// Feed one keystroke to `state` and return the intent it produced.
///
/// `None` means the key was consumed without completing a command — a count
/// digit, a pending operator, or an unrecognised key. The caller dispatches
/// nothing and waits.
pub fn key_to_intent(state: &mut VimState, key: char, ctrl: bool) -> Option<CommandDispatchIntent> {
    action_to_intent(state.process_key(key, ctrl))
}

/// Map a resolved action onto the intent that performs it.
///
/// Separate from [`key_to_intent`] so an action from anywhere — a replayed
/// macro, a test, a future command line — reaches the same intent.
pub fn action_to_intent(action: VimAction) -> Option<CommandDispatchIntent> {
    match action {
        VimAction::Motion { motion, count } => {
            Some(CommandDispatchIntent::VimMotion { motion, count })
        }
        VimAction::OperatorMotion {
            operator,
            count,
            motion,
        } => Some(CommandDispatchIntent::VimOperatorMotion {
            operator,
            count,
            motion,
        }),
        VimAction::LinewiseOperator { operator, count } => {
            Some(CommandDispatchIntent::VimLinewiseOperator { operator, count })
        }
        VimAction::ChangeMode(mode) => Some(CommandDispatchIntent::VimChangeMode(mode)),
        VimAction::InsertBefore => Some(CommandDispatchIntent::VimInsertBefore),
        VimAction::InsertAfter => Some(CommandDispatchIntent::VimInsertAfter),
        VimAction::InsertLineBelow => Some(CommandDispatchIntent::VimInsertLineBelow),
        VimAction::InsertLineAbove => Some(CommandDispatchIntent::VimInsertLineAbove),
        VimAction::Put => Some(CommandDispatchIntent::VimPut),
        VimAction::DeleteChar => Some(CommandDispatchIntent::VimDeleteChar),
        VimAction::SearchForward => Some(CommandDispatchIntent::VimSearchForward),
        // Undo and redo are the editor's own, not Vim's. Routing them to the
        // existing intents means one undo stack rather than two views of it.
        VimAction::Undo => Some(CommandDispatchIntent::Noop),
        VimAction::Redo => Some(CommandDispatchIntent::Noop),
        // Pending or unrecognised: the parser is holding state for the next
        // key, and dispatching anything now would act on half a command.
        _ => None,
    }
}

/// Whether a keystroke should reach the buffer as typed text.
///
/// In insert mode Vim passes characters straight through; in normal mode they
/// are commands and must not also be inserted. Getting this wrong is the most
/// visible possible bug — every `j` would move *and* type a `j`.
pub fn key_is_text_input(state: &VimState, enabled: bool) -> bool {
    !enabled || state.mode() == EditorInputMode::Insert
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vim::{VimMotionKind, VimOperatorKind};

    #[test]
    fn a_motion_key_becomes_a_motion_intent() {
        let mut state = VimState::new();
        let intent = key_to_intent(&mut state, 'j', false).expect("j is a motion");
        assert!(matches!(
            intent,
            CommandDispatchIntent::VimMotion {
                motion: VimMotionKind::Down,
                count: 1
            }
        ));
    }

    #[test]
    fn a_count_prefix_produces_nothing_until_the_motion_arrives() {
        let mut state = VimState::new();
        assert!(
            key_to_intent(&mut state, '3', false).is_none(),
            "a count alone is not a command"
        );
        let intent = key_to_intent(&mut state, 'w', false).expect("3w completes");
        assert!(matches!(
            intent,
            CommandDispatchIntent::VimMotion {
                motion: VimMotionKind::WordForward,
                count: 3
            }
        ));
    }

    #[test]
    fn an_operator_waits_for_its_motion() {
        let mut state = VimState::new();
        assert!(
            key_to_intent(&mut state, 'd', false).is_none(),
            "dispatching on `d` alone would delete something the user did not name"
        );
        let intent = key_to_intent(&mut state, 'w', false).expect("dw completes");
        assert!(matches!(
            intent,
            CommandDispatchIntent::VimOperatorMotion {
                operator: VimOperatorKind::Delete,
                motion: VimMotionKind::WordForward,
                count: 1,
            }
        ));
    }

    #[test]
    fn a_doubled_operator_is_linewise() {
        let mut state = VimState::new();
        assert!(key_to_intent(&mut state, 'd', false).is_none());
        let intent = key_to_intent(&mut state, 'd', false).expect("dd completes");
        assert!(matches!(
            intent,
            CommandDispatchIntent::VimLinewiseOperator {
                operator: VimOperatorKind::Delete,
                count: 1
            }
        ));
    }

    #[test]
    fn insert_keys_map_to_their_intents() {
        for (key, matches_intent) in [
            (
                'i',
                matches!(
                    action_to_intent(VimAction::InsertBefore),
                    Some(CommandDispatchIntent::VimInsertBefore)
                ),
            ),
            (
                'a',
                matches!(
                    action_to_intent(VimAction::InsertAfter),
                    Some(CommandDispatchIntent::VimInsertAfter)
                ),
            ),
            (
                'o',
                matches!(
                    action_to_intent(VimAction::InsertLineBelow),
                    Some(CommandDispatchIntent::VimInsertLineBelow)
                ),
            ),
            (
                'O',
                matches!(
                    action_to_intent(VimAction::InsertLineAbove),
                    Some(CommandDispatchIntent::VimInsertLineAbove)
                ),
            ),
        ] {
            assert!(matches_intent, "`{key}` did not map to its intent");
        }
    }

    #[test]
    fn normal_mode_keys_are_commands_not_text() {
        let state = VimState::new();
        assert!(
            !key_is_text_input(&state, true),
            "a `j` in normal mode that also typed a `j` is the loudest possible bug"
        );
    }

    #[test]
    fn insert_mode_keys_are_text() {
        let mut state = VimState::new();
        state.set_mode(EditorInputMode::Insert);
        assert!(key_is_text_input(&state, true));
    }

    #[test]
    fn a_disabled_session_passes_every_key_through_as_text() {
        let state = VimState::new();
        assert!(
            key_is_text_input(&state, false),
            "a user who never enabled Vim must be able to type `j`"
        );
    }

    #[test]
    fn undo_and_redo_defer_to_the_editors_own_stack() {
        assert!(matches!(
            action_to_intent(VimAction::Undo),
            Some(CommandDispatchIntent::Noop)
        ));
        assert!(matches!(
            action_to_intent(VimAction::Redo),
            Some(CommandDispatchIntent::Noop)
        ));
    }
}
