//! Vim modal session state and app dispatch of editor-resolved text operations.
//!
//! The UI parser supplies pure intents. The editor resolves movements and operator
//! ranges from bounded snapshots in native UTF-8 coordinates. The app owns register
//! contents, mode changes, and ordinary edit/undo transactions.
//!
//! **The session.** [`VimSession`] holds whether modal editing is on and the
//! parser's state across keystrokes — `d` then `w` is one command, so the
//! parser has to remember the `d`. It is per-application rather than
//! per-buffer, matching Vim: the mode and the pending command follow the user,
//! not the file.
//!
//! [`character_to_byte_column`] and [`byte_to_character_column`] remain compatibility
//! helpers for put/open-line commands and tests. Motion/operator dispatch does not
//! read full text or convert columns in app. The two legacy coordinate types differ:
//!
//! * `legion_protocol::TextCoordinate::character` — a **character** offset.
//! * `legion_text::TextPosition::column` — a **UTF-8 byte** offset.
//!
//! Pure UI resolution remains available as a compatibility reference; native
//! resolution treats CRLF as a terminator and retains its exact bytes in registers.

use legion_editor::TextPosition;
use legion_ui::{EditorInputMode, VimState};

/// Translate parser vocabulary at the app boundary; the editor never depends on UI.
pub(crate) fn editor_motion(motion: legion_ui::VimMotionKind) -> legion_editor::vim::Motion {
    use legion_editor::vim::Motion;
    use legion_ui::VimMotionKind;
    match motion {
        VimMotionKind::Left => Motion::Left,
        VimMotionKind::Right => Motion::Right,
        VimMotionKind::Up => Motion::Up,
        VimMotionKind::Down => Motion::Down,
        VimMotionKind::WordForward => Motion::WordForward,
        VimMotionKind::WordBackward => Motion::WordBackward,
        VimMotionKind::WordEnd => Motion::WordEnd,
        VimMotionKind::LineStart => Motion::LineStart,
        VimMotionKind::LineEnd => Motion::LineEnd,
        VimMotionKind::FirstNonBlank => Motion::FirstNonBlank,
        VimMotionKind::FileStart => Motion::FileStart,
        VimMotionKind::FileEnd => Motion::FileEnd,
        VimMotionKind::FindChar(c) => Motion::FindChar(c),
        VimMotionKind::TillChar(c) => Motion::TillChar(c),
    }
}

impl crate::AppComposition {
    /// Handle text-dependent Vim intents with editor-owned bounded resolution.
    pub(crate) fn dispatch_vim_text_intent(
        &mut self,
        intent: &legion_ui::CommandDispatchIntent,
        event_context: &crate::EventContext,
    ) -> Result<Option<crate::AppCommandOutcome>, crate::AppCompositionError> {
        use crate::AppCommandOutcome;
        use legion_ui::CommandDispatchIntent;

        if !matches!(
            intent,
            CommandDispatchIntent::VimMotion { .. }
                | CommandDispatchIntent::VimOperatorMotion { .. }
                | CommandDispatchIntent::VimLinewiseOperator { .. }
                | CommandDispatchIntent::VimDeleteChar
                | CommandDispatchIntent::VimInsertBefore
                | CommandDispatchIntent::VimInsertAfter
        ) {
            return Ok(None);
        }
        if !self.vim.enabled {
            return Ok(Some(AppCommandOutcome::Noop));
        }
        let Some(buffer_id) = self.active_documents.active_buffer_id else {
            return Ok(Some(AppCommandOutcome::Noop));
        };
        let position = self.editor.primary_cursor(buffer_id)?;
        let (range, operator) = match intent {
            CommandDispatchIntent::VimMotion { motion, count } => {
                let to = self.editor.resolve_vim_motion(
                    buffer_id,
                    position,
                    editor_motion(*motion),
                    *count,
                )?;
                self.editor
                    .set_cursors(buffer_id, vec![legion_editor::Cursor { position: to }])?;
                return Ok(Some(AppCommandOutcome::CursorSet(buffer_id)));
            }
            CommandDispatchIntent::VimOperatorMotion {
                operator,
                count,
                motion,
            } => (
                self.editor.resolve_vim_operator_range(
                    buffer_id,
                    position,
                    editor_motion(*motion),
                    *count,
                )?,
                *operator,
            ),
            CommandDispatchIntent::VimLinewiseOperator { operator, count } => (
                Some(
                    self.editor
                        .resolve_vim_linewise_range(buffer_id, position, *count)?,
                ),
                *operator,
            ),
            CommandDispatchIntent::VimDeleteChar => (
                self.editor
                    .resolve_vim_character_range(buffer_id, position)?,
                legion_ui::VimOperatorKind::Delete,
            ),
            CommandDispatchIntent::VimInsertBefore | CommandDispatchIntent::VimInsertAfter => {
                if matches!(intent, CommandDispatchIntent::VimInsertAfter) {
                    let to = self
                        .editor
                        .resolve_vim_character_range(buffer_id, position)?
                        .map_or(position, |range| range.range.end);
                    self.editor
                        .set_cursors(buffer_id, vec![legion_editor::Cursor { position: to }])?;
                }
                self.vim.state.set_mode(EditorInputMode::Insert);
                return Ok(Some(AppCommandOutcome::VimModeChanged(
                    self.vim.display_mode(),
                )));
            }
            _ => unreachable!("text intent filtered above"),
        };
        let Some(range) = range else {
            return Ok(Some(AppCommandOutcome::Noop));
        };
        self.vim.register = Some(VimRegister {
            text: self.editor.vim_range_text(buffer_id, range.range)?,
            linewise: range.linewise,
        });
        if matches!(operator, legion_ui::VimOperatorKind::Yank) {
            return Ok(Some(AppCommandOutcome::Noop));
        }
        let edit = legion_editor::TextEdit::new(range.range, String::new());
        let outcome = self.apply_vim_edit(buffer_id, edit, event_context)?;
        if matches!(operator, legion_ui::VimOperatorKind::Change) {
            self.vim.state.set_mode(EditorInputMode::Insert);
        }
        Ok(Some(outcome))
    }
}

/// Application-wide Vim modal editing state.
#[derive(Debug, Default)]
pub struct VimSession {
    /// Whether modal editing is active. Off by default: Vim is opt-in, and a
    /// user who has not asked for it must keep ordinary insert behaviour.
    pub enabled: bool,
    /// Key-parser state, carried across keystrokes so multi-key commands work.
    pub state: VimState,
    /// The unnamed register: what `y` and `d` last took, and what `p` puts.
    ///
    /// Vim's delete is a cut, not a discard, so `dd` then `p` moves a line —
    /// treating delete as a discard would make the most common way to move
    /// text silently lose it.
    pub register: Option<VimRegister>,
}

/// Text held in the unnamed register.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VimRegister {
    /// The text itself.
    pub text: String,
    /// Whether it was taken line-wise.
    ///
    /// `p` puts a line-wise register on a new line below and a char-wise one
    /// after the cursor. Losing this makes `dd`/`p` splice a whole line into
    /// the middle of another.
    pub linewise: bool,
}

impl VimSession {
    /// The mode to display, or `None` when modal editing is off.
    ///
    /// `None` rather than `Insert` so the status bar can distinguish "not
    /// using Vim" from "using Vim, in insert mode" — they look identical while
    /// typing and mean very different things about what the next `d` will do.
    pub fn display_mode(&self) -> Option<EditorInputMode> {
        self.enabled.then(|| self.state.mode())
    }

    /// Turn modal editing on or off, discarding any half-typed command.
    ///
    /// Resetting matters on both edges: a pending `d` left over from before
    /// the toggle would silently consume the next motion the user typed for
    /// some other purpose.
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
        self.state.reset();
    }
}

/// Byte column for a character offset on `line` of `text`.
///
/// Saturates at the line's length so a cursor past the end lands at the end
/// rather than failing — motions clamp before this is called, and a hard error
/// here would turn a harmless overshoot into a lost keystroke.
pub fn character_to_byte_column(text: &str, line: usize, character: usize) -> usize {
    let Some(line_text) = text.split('\n').nth(line) else {
        return 0;
    };
    line_text
        .char_indices()
        .nth(character)
        .map(|(offset, _)| offset)
        .unwrap_or(line_text.len())
}

/// Character offset for a byte column on `line` of `text`.
///
/// A byte column landing *inside* a multi-byte character resolves to that
/// character rather than the one after it. Rounding down never moves a cursor
/// past where the byte offset pointed, so a conversion cannot silently
/// overshoot; rounding up can, and an interior offset is exactly what a
/// byte-based editor produces when it splits a character.
pub fn byte_to_character_column(text: &str, line: usize, byte_column: usize) -> usize {
    let Some(line_text) = text.split('\n').nth(line) else {
        return 0;
    };
    let mut characters = 0;
    for (offset, character) in line_text.char_indices() {
        if byte_column < offset + character.len_utf8() {
            return characters;
        }
        characters += 1;
    }
    characters
}

/// Read a byte-based editor position as a character-based line and column.
pub fn position_to_character_column(text: &str, position: TextPosition) -> (usize, usize) {
    (
        position.line,
        byte_to_character_column(text, position.line, position.column),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    const ASCII: &str = "fn main() {\n    let x = 1;\n}\n";
    const UNICODE: &str = "let café = \"naïve\";\nlet x = 1;\n";

    #[test]
    fn ascii_columns_convert_identically() {
        for column in 0..11 {
            assert_eq!(character_to_byte_column(ASCII, 0, column), column);
            assert_eq!(byte_to_character_column(ASCII, 0, column), column);
        }
    }

    #[test]
    fn a_multi_byte_character_shifts_every_column_after_it() {
        // `café` — the é is two bytes, so character 8 is byte 9.
        assert_eq!(character_to_byte_column(UNICODE, 0, 4), 4, "before the é");
        assert_eq!(
            character_to_byte_column(UNICODE, 0, 8),
            9,
            "one byte of drift has accumulated by the space after café"
        );
        assert_eq!(byte_to_character_column(UNICODE, 0, 9), 8);
    }

    #[test]
    fn the_conversions_round_trip() {
        let line_text = UNICODE.split('\n').next().unwrap();
        for character in 0..line_text.chars().count() {
            let byte = character_to_byte_column(UNICODE, 0, character);
            assert_eq!(
                byte_to_character_column(UNICODE, 0, byte),
                character,
                "character {character} did not survive the round trip"
            );
        }
    }

    #[test]
    fn a_column_past_the_end_lands_at_the_end() {
        let line_len = UNICODE.split('\n').next().unwrap().len();
        assert_eq!(character_to_byte_column(UNICODE, 0, 999), line_len);
    }

    #[test]
    fn a_line_past_the_end_is_column_zero() {
        assert_eq!(character_to_byte_column(ASCII, 99, 5), 0);
        assert_eq!(byte_to_character_column(ASCII, 99, 5), 0);
    }

    #[test]
    fn a_byte_offset_inside_a_character_counts_it_as_reached() {
        // Byte 8 is the é's second byte, so the position is *in* the é at
        // character 7. Rounding up to 8 would place a cursor past where the
        // offset pointed.
        assert_eq!(byte_to_character_column(UNICODE, 0, 8), 7);
    }

    #[test]
    fn a_session_is_off_and_silent_by_default() {
        let session = VimSession::default();
        assert!(!session.enabled);
        assert_eq!(
            session.display_mode(),
            None,
            "a status bar must be able to tell 'not using Vim' from 'in insert mode'"
        );
    }

    #[test]
    fn toggling_discards_a_half_typed_command() {
        let mut session = VimSession::default();
        session.set_enabled(true);
        session.state.process_key('d', false);
        assert!(
            !session.state.pending_keys_display().is_empty(),
            "precondition: a `d` is pending"
        );

        session.set_enabled(false);
        assert!(
            session.state.pending_keys_display().is_empty(),
            "a leftover operator would silently consume the next motion typed"
        );
    }

    #[test]
    fn an_enabled_session_reports_its_mode() {
        let mut session = VimSession::default();
        session.set_enabled(true);
        assert_eq!(session.display_mode(), Some(EditorInputMode::Normal));
    }
}
