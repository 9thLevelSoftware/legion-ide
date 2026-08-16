//! Vim modal editing state machine.
//!
//! This module implements a pure state machine that parses keystrokes into
//! [`VimAction`] values.  It has **no** dependency on the editor engine, the
//! renderer, or the filesystem — callers translate the resolved action into a
//! [`CommandDispatchIntent`](super::ui::CommandDispatchIntent) for dispatch.

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

/// Which Vim editing mode the editor is in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum EditorInputMode {
    /// Command mode — motions, operators, and mode transitions.
    Normal,
    /// Text entry mode — keystrokes are passed through to the editor.
    Insert,
    /// Character-wise visual selection mode.
    Visual,
    /// Line-wise visual selection mode.
    VisualLine,
}

impl EditorInputMode {
    /// Human-readable status-bar label.
    pub fn label(self) -> &'static str {
        match self {
            Self::Normal => "NORMAL",
            Self::Insert => "INSERT",
            Self::Visual => "VISUAL",
            Self::VisualLine => "V-LINE",
        }
    }
}

/// Motions that move the cursor or define a text object range.
///
/// `Copy` because every variant is a discriminant or a `char`: motion
/// resolution repeats one for a count, and threading clones through that loop
/// would say something about ownership that is not true of the data.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VimMotionKind {
    /// `h` — left one character.
    Left,
    /// `l` — right one character.
    Right,
    /// `k` — up one line.
    Up,
    /// `j` — down one line.
    Down,
    /// `w` — forward to the start of the next word.
    WordForward,
    /// `b` — backward to the start of the current/previous word.
    WordBackward,
    /// `e` — forward to the end of the current/next word.
    WordEnd,
    /// `0` — to the first column of the line.
    LineStart,
    /// `$` — to the end of the line.
    LineEnd,
    /// `^` — to the first non-blank character of the line.
    FirstNonBlank,
    /// `gg` — to the first line of the file.
    FileStart,
    /// `G` — to the last line of the file.
    FileEnd,
    /// `f{char}` — forward to the next occurrence of `char` on the line.
    FindChar(char),
    /// `t{char}` — forward to just before the next occurrence of `char`.
    TillChar(char),
}

/// Operators that act on a motion-defined range.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VimOperatorKind {
    /// `d` — delete.
    Delete,
    /// `c` — change (delete and enter insert mode).
    Change,
    /// `y` — yank (copy).
    Yank,
}

/// Resolved action produced by the state machine after one or more keystrokes.
#[derive(Debug, Clone, PartialEq)]
pub enum VimAction {
    /// A cursor motion with an optional repeat count.
    Motion {
        /// The motion to execute.
        motion: VimMotionKind,
        /// Repeat count (minimum 1).
        count: usize,
    },
    /// An operator applied to a motion-defined range.
    OperatorMotion {
        /// The operator to apply.
        operator: VimOperatorKind,
        /// Repeat count (minimum 1).
        count: usize,
        /// The motion that defines the range.
        motion: VimMotionKind,
    },
    /// A line-wise operator (e.g. `dd`, `yy`, `cc`).
    LinewiseOperator {
        /// The operator to apply.
        operator: VimOperatorKind,
        /// Number of lines (minimum 1).
        count: usize,
    },
    /// Switch to a different editing mode.
    ChangeMode(EditorInputMode),
    /// `i` — enter insert mode before the cursor.
    InsertBefore,
    /// `a` — enter insert mode after the cursor.
    InsertAfter,
    /// `o` — open a new line below and enter insert mode.
    InsertLineBelow,
    /// `O` — open a new line above and enter insert mode.
    InsertLineAbove,
    /// `p` — put (paste) from the register.
    Put,
    /// `u` — undo.
    Undo,
    /// `Ctrl-R` — redo.
    Redo,
    /// `/` — begin forward search.
    SearchForward,
    /// `x` — delete the character under the cursor.
    DeleteChar,
    /// The key sequence is not yet complete (e.g. pending operator or `g`).
    Incomplete,
    /// The key sequence was not recognized.
    Unknown,
}

// ---------------------------------------------------------------------------
// Internal parser state
// ---------------------------------------------------------------------------

/// Internal state tracking for multi-key sequences.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PendingSequence {
    /// Waiting for a motion after an operator key.
    Operator(VimOperatorKind),
    /// Waiting for the second `g` in `gg`.
    G,
    /// Waiting for the target character after `f`.
    FindChar,
    /// Waiting for the target character after `t`.
    TillChar,
}

// ---------------------------------------------------------------------------
// VimState
// ---------------------------------------------------------------------------

/// Vim modal editing state machine.
///
/// Feed keystrokes through [`process_key`](Self::process_key) and act on the
/// returned [`VimAction`].  The state machine is fully self-contained and does
/// not access any external resource.
pub struct VimState {
    mode: EditorInputMode,
    pending_count: Option<usize>,
    pending: Option<PendingSequence>,
    last_action: Option<VimAction>,
}

impl VimState {
    /// Create a new state machine starting in Normal mode.
    pub fn new() -> Self {
        Self {
            mode: EditorInputMode::Normal,
            pending_count: None,
            pending: None,
            last_action: None,
        }
    }

    /// Return the current editing mode.
    pub fn mode(&self) -> EditorInputMode {
        self.mode
    }

    /// Return a display-safe representation of any pending key sequence.
    pub fn pending_keys_display(&self) -> String {
        let mut s = String::new();
        if let Some(count) = self.pending_count {
            s.push_str(&count.to_string());
        }
        match self.pending {
            Some(PendingSequence::Operator(VimOperatorKind::Delete)) => s.push('d'),
            Some(PendingSequence::Operator(VimOperatorKind::Change)) => s.push('c'),
            Some(PendingSequence::Operator(VimOperatorKind::Yank)) => s.push('y'),
            Some(PendingSequence::G) => s.push('g'),
            Some(PendingSequence::FindChar) => s.push('f'),
            Some(PendingSequence::TillChar) => s.push('t'),
            None => {}
        }
        s
    }

    /// Clear all pending state without changing mode.
    pub fn reset(&mut self) {
        self.pending_count = None;
        self.pending = None;
    }

    /// Process a single keystroke and return the resolved action.
    ///
    /// `key` is the character produced by the key (case-sensitive).
    /// `ctrl` indicates modifier state.
    pub fn process_key(&mut self, key: char, ctrl: bool) -> VimAction {
        match self.mode {
            EditorInputMode::Normal | EditorInputMode::Visual | EditorInputMode::VisualLine => {
                self.process_normal_visual(key, ctrl)
            }
            EditorInputMode::Insert => self.process_insert(key),
        }
    }

    // -- Insert mode --------------------------------------------------------

    fn process_insert(&mut self, key: char) -> VimAction {
        if key == '\x1b' {
            // Escape
            self.mode = EditorInputMode::Normal;
            self.reset();
            return VimAction::ChangeMode(EditorInputMode::Normal);
        }
        VimAction::Unknown
    }

    // -- Normal / Visual mode -----------------------------------------------

    fn process_normal_visual(&mut self, key: char, ctrl: bool) -> VimAction {
        // Handle pending multi-key sequences first.
        if let Some(pending) = self.pending.take() {
            return self.resolve_pending(pending, key);
        }

        // Count prefix accumulation (digits 1-9 start a count, 0 is LineStart
        // unless we are already accumulating a count).
        if key.is_ascii_digit() {
            let digit = key as usize - '0' as usize;
            if digit != 0 || self.pending_count.is_some() {
                self.pending_count = Some(self.pending_count.unwrap_or(0) * 10 + digit);
                return VimAction::Incomplete;
            }
        }

        let count = self.take_count();

        // Ctrl-modified keys.
        if ctrl {
            return match key {
                'r' | 'R' => {
                    let action = VimAction::Redo;
                    self.last_action = Some(action.clone());
                    action
                }
                _ => {
                    // Not a recognized ctrl combo — unknown.
                    VimAction::Unknown
                }
            };
        }

        // Unmodified keys.
        match key {
            // -- Motions ----------------------------------------------------
            'h' => VimAction::Motion {
                motion: VimMotionKind::Left,
                count,
            },
            'j' => VimAction::Motion {
                motion: VimMotionKind::Down,
                count,
            },
            'k' => VimAction::Motion {
                motion: VimMotionKind::Up,
                count,
            },
            'l' => VimAction::Motion {
                motion: VimMotionKind::Right,
                count,
            },
            'w' => VimAction::Motion {
                motion: VimMotionKind::WordForward,
                count,
            },
            'b' => VimAction::Motion {
                motion: VimMotionKind::WordBackward,
                count,
            },
            'e' => VimAction::Motion {
                motion: VimMotionKind::WordEnd,
                count,
            },
            '0' => VimAction::Motion {
                motion: VimMotionKind::LineStart,
                count,
            },
            '$' => VimAction::Motion {
                motion: VimMotionKind::LineEnd,
                count,
            },
            '^' => VimAction::Motion {
                motion: VimMotionKind::FirstNonBlank,
                count,
            },
            'G' => VimAction::Motion {
                motion: VimMotionKind::FileEnd,
                count,
            },

            // `g` starts a multi-key sequence (gg).
            'g' => {
                self.pending_count = if count > 1 { Some(count) } else { None };
                self.pending = Some(PendingSequence::G);
                VimAction::Incomplete
            }

            // `f` / `t` — find/till character on the line.
            'f' => {
                self.pending_count = if count > 1 { Some(count) } else { None };
                self.pending = Some(PendingSequence::FindChar);
                VimAction::Incomplete
            }
            't' if self.mode == EditorInputMode::Normal || self.mode == EditorInputMode::Visual => {
                self.pending_count = if count > 1 { Some(count) } else { None };
                self.pending = Some(PendingSequence::TillChar);
                VimAction::Incomplete
            }

            // -- Operators --------------------------------------------------
            'd' => {
                self.pending_count = if count > 1 { Some(count) } else { None };
                self.pending = Some(PendingSequence::Operator(VimOperatorKind::Delete));
                VimAction::Incomplete
            }
            'c' => {
                self.pending_count = if count > 1 { Some(count) } else { None };
                self.pending = Some(PendingSequence::Operator(VimOperatorKind::Change));
                VimAction::Incomplete
            }
            'y' => {
                self.pending_count = if count > 1 { Some(count) } else { None };
                self.pending = Some(PendingSequence::Operator(VimOperatorKind::Yank));
                VimAction::Incomplete
            }

            // -- Mode changes -----------------------------------------------
            'i' => {
                self.mode = EditorInputMode::Insert;
                let action = VimAction::InsertBefore;
                self.last_action = Some(action.clone());
                action
            }
            'a' => {
                self.mode = EditorInputMode::Insert;
                let action = VimAction::InsertAfter;
                self.last_action = Some(action.clone());
                action
            }
            'o' => {
                self.mode = EditorInputMode::Insert;
                let action = VimAction::InsertLineBelow;
                self.last_action = Some(action.clone());
                action
            }
            'O' => {
                self.mode = EditorInputMode::Insert;
                let action = VimAction::InsertLineAbove;
                self.last_action = Some(action.clone());
                action
            }
            'v' => {
                if self.mode == EditorInputMode::Visual {
                    self.mode = EditorInputMode::Normal;
                    VimAction::ChangeMode(EditorInputMode::Normal)
                } else {
                    self.mode = EditorInputMode::Visual;
                    VimAction::ChangeMode(EditorInputMode::Visual)
                }
            }
            'V' => {
                if self.mode == EditorInputMode::VisualLine {
                    self.mode = EditorInputMode::Normal;
                    VimAction::ChangeMode(EditorInputMode::Normal)
                } else {
                    self.mode = EditorInputMode::VisualLine;
                    VimAction::ChangeMode(EditorInputMode::VisualLine)
                }
            }
            '\x1b' => {
                // Escape — always return to Normal.
                self.mode = EditorInputMode::Normal;
                self.reset();
                VimAction::ChangeMode(EditorInputMode::Normal)
            }

            // -- Single-key actions -----------------------------------------
            'x' => {
                let action = VimAction::DeleteChar;
                self.last_action = Some(action.clone());
                action
            }
            'p' => {
                let action = VimAction::Put;
                self.last_action = Some(action.clone());
                action
            }
            'u' => {
                let action = VimAction::Undo;
                self.last_action = Some(action.clone());
                action
            }
            '/' => VimAction::SearchForward,
            '.' => {
                if let Some(ref last) = self.last_action {
                    last.clone()
                } else {
                    VimAction::Unknown
                }
            }

            _ => VimAction::Unknown,
        }
    }

    // -- Pending sequence resolution ----------------------------------------

    fn resolve_pending(&mut self, pending: PendingSequence, key: char) -> VimAction {
        let count = self.take_count();

        match pending {
            PendingSequence::G => {
                if key == 'g' {
                    VimAction::Motion {
                        motion: VimMotionKind::FileStart,
                        count,
                    }
                } else {
                    VimAction::Unknown
                }
            }
            PendingSequence::FindChar => VimAction::Motion {
                motion: VimMotionKind::FindChar(key),
                count,
            },
            PendingSequence::TillChar => VimAction::Motion {
                motion: VimMotionKind::TillChar(key),
                count,
            },
            PendingSequence::Operator(op) => {
                // Line-doubled operator: dd, cc, yy.
                let doubled = matches!(
                    (op, key),
                    (VimOperatorKind::Delete, 'd')
                        | (VimOperatorKind::Change, 'c')
                        | (VimOperatorKind::Yank, 'y')
                );
                if doubled {
                    let action = VimAction::LinewiseOperator {
                        operator: op,
                        count,
                    };
                    if matches!(op, VimOperatorKind::Change) {
                        self.mode = EditorInputMode::Insert;
                    }
                    self.last_action = Some(action.clone());
                    action
                } else if let Some(motion) = Self::char_to_motion(key) {
                    let action = VimAction::OperatorMotion {
                        operator: op,
                        count,
                        motion,
                    };
                    if matches!(op, VimOperatorKind::Change) {
                        self.mode = EditorInputMode::Insert;
                    }
                    self.last_action = Some(action.clone());
                    action
                } else {
                    VimAction::Unknown
                }
            }
        }
    }

    // -- Helpers ------------------------------------------------------------

    fn take_count(&mut self) -> usize {
        self.pending_count.take().unwrap_or(1)
    }

    /// Map a character to its motion, if any.
    fn char_to_motion(key: char) -> Option<VimMotionKind> {
        match key {
            'h' => Some(VimMotionKind::Left),
            'j' => Some(VimMotionKind::Down),
            'k' => Some(VimMotionKind::Up),
            'l' => Some(VimMotionKind::Right),
            'w' => Some(VimMotionKind::WordForward),
            'b' => Some(VimMotionKind::WordBackward),
            'e' => Some(VimMotionKind::WordEnd),
            '0' => Some(VimMotionKind::LineStart),
            '$' => Some(VimMotionKind::LineEnd),
            '^' => Some(VimMotionKind::FirstNonBlank),
            'G' => Some(VimMotionKind::FileEnd),
            _ => None,
        }
    }
}

impl Default for VimState {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // -- Helpers ------------------------------------------------------------

    fn normal_state() -> VimState {
        VimState::new()
    }

    fn key(state: &mut VimState, ch: char) -> VimAction {
        state.process_key(ch, false)
    }

    fn ctrl_key(state: &mut VimState, ch: char) -> VimAction {
        state.process_key(ch, true)
    }

    fn esc(state: &mut VimState) -> VimAction {
        state.process_key('\x1b', false)
    }

    // -- Mode transitions ---------------------------------------------------

    #[test]
    fn normal_to_insert_via_i() {
        let mut s = normal_state();
        let a = key(&mut s, 'i');
        assert_eq!(a, VimAction::InsertBefore);
        assert_eq!(s.mode(), EditorInputMode::Insert);
    }

    #[test]
    fn normal_to_insert_via_a() {
        let mut s = normal_state();
        let a = key(&mut s, 'a');
        assert_eq!(a, VimAction::InsertAfter);
        assert_eq!(s.mode(), EditorInputMode::Insert);
    }

    #[test]
    fn normal_to_insert_via_o() {
        let mut s = normal_state();
        let a = key(&mut s, 'o');
        assert_eq!(a, VimAction::InsertLineBelow);
        assert_eq!(s.mode(), EditorInputMode::Insert);
    }

    #[test]
    fn normal_to_insert_via_shift_o() {
        let mut s = normal_state();
        let a = key(&mut s, 'O');
        assert_eq!(a, VimAction::InsertLineAbove);
        assert_eq!(s.mode(), EditorInputMode::Insert);
    }

    #[test]
    fn insert_to_normal_via_esc() {
        let mut s = normal_state();
        key(&mut s, 'i');
        assert_eq!(s.mode(), EditorInputMode::Insert);
        let a = esc(&mut s);
        assert_eq!(a, VimAction::ChangeMode(EditorInputMode::Normal));
        assert_eq!(s.mode(), EditorInputMode::Normal);
    }

    #[test]
    fn normal_to_visual_via_v() {
        let mut s = normal_state();
        let a = key(&mut s, 'v');
        assert_eq!(a, VimAction::ChangeMode(EditorInputMode::Visual));
        assert_eq!(s.mode(), EditorInputMode::Visual);
    }

    #[test]
    fn visual_to_normal_via_v() {
        let mut s = normal_state();
        key(&mut s, 'v');
        let a = key(&mut s, 'v');
        assert_eq!(a, VimAction::ChangeMode(EditorInputMode::Normal));
        assert_eq!(s.mode(), EditorInputMode::Normal);
    }

    #[test]
    fn normal_to_visual_line_via_shift_v() {
        let mut s = normal_state();
        let a = key(&mut s, 'V');
        assert_eq!(a, VimAction::ChangeMode(EditorInputMode::VisualLine));
        assert_eq!(s.mode(), EditorInputMode::VisualLine);
    }

    #[test]
    fn visual_line_to_normal_via_shift_v() {
        let mut s = normal_state();
        key(&mut s, 'V');
        let a = key(&mut s, 'V');
        assert_eq!(a, VimAction::ChangeMode(EditorInputMode::Normal));
        assert_eq!(s.mode(), EditorInputMode::Normal);
    }

    #[test]
    fn visual_to_normal_via_esc() {
        let mut s = normal_state();
        key(&mut s, 'v');
        let a = esc(&mut s);
        assert_eq!(a, VimAction::ChangeMode(EditorInputMode::Normal));
        assert_eq!(s.mode(), EditorInputMode::Normal);
    }

    // -- Basic motions ------------------------------------------------------

    #[test]
    fn motion_h() {
        let mut s = normal_state();
        assert_eq!(
            key(&mut s, 'h'),
            VimAction::Motion {
                motion: VimMotionKind::Left,
                count: 1
            }
        );
    }

    #[test]
    fn motion_j() {
        let mut s = normal_state();
        assert_eq!(
            key(&mut s, 'j'),
            VimAction::Motion {
                motion: VimMotionKind::Down,
                count: 1
            }
        );
    }

    #[test]
    fn motion_k() {
        let mut s = normal_state();
        assert_eq!(
            key(&mut s, 'k'),
            VimAction::Motion {
                motion: VimMotionKind::Up,
                count: 1
            }
        );
    }

    #[test]
    fn motion_l() {
        let mut s = normal_state();
        assert_eq!(
            key(&mut s, 'l'),
            VimAction::Motion {
                motion: VimMotionKind::Right,
                count: 1
            }
        );
    }

    // -- Word motions -------------------------------------------------------

    #[test]
    fn motion_w() {
        let mut s = normal_state();
        assert_eq!(
            key(&mut s, 'w'),
            VimAction::Motion {
                motion: VimMotionKind::WordForward,
                count: 1
            }
        );
    }

    #[test]
    fn motion_b() {
        let mut s = normal_state();
        assert_eq!(
            key(&mut s, 'b'),
            VimAction::Motion {
                motion: VimMotionKind::WordBackward,
                count: 1
            }
        );
    }

    #[test]
    fn motion_e() {
        let mut s = normal_state();
        assert_eq!(
            key(&mut s, 'e'),
            VimAction::Motion {
                motion: VimMotionKind::WordEnd,
                count: 1
            }
        );
    }

    // -- Line motions -------------------------------------------------------

    #[test]
    fn motion_zero_line_start() {
        let mut s = normal_state();
        assert_eq!(
            key(&mut s, '0'),
            VimAction::Motion {
                motion: VimMotionKind::LineStart,
                count: 1
            }
        );
    }

    #[test]
    fn motion_dollar_line_end() {
        let mut s = normal_state();
        assert_eq!(
            key(&mut s, '$'),
            VimAction::Motion {
                motion: VimMotionKind::LineEnd,
                count: 1
            }
        );
    }

    #[test]
    fn motion_caret_first_nonblank() {
        let mut s = normal_state();
        assert_eq!(
            key(&mut s, '^'),
            VimAction::Motion {
                motion: VimMotionKind::FirstNonBlank,
                count: 1
            }
        );
    }

    // -- File motions -------------------------------------------------------

    #[test]
    fn motion_gg_file_start() {
        let mut s = normal_state();
        let a1 = key(&mut s, 'g');
        assert_eq!(a1, VimAction::Incomplete);
        let a2 = key(&mut s, 'g');
        assert_eq!(
            a2,
            VimAction::Motion {
                motion: VimMotionKind::FileStart,
                count: 1
            }
        );
    }

    #[test]
    fn motion_shift_g_file_end() {
        let mut s = normal_state();
        assert_eq!(
            key(&mut s, 'G'),
            VimAction::Motion {
                motion: VimMotionKind::FileEnd,
                count: 1
            }
        );
    }

    // -- Find/Till motions --------------------------------------------------

    #[test]
    fn motion_f_char() {
        let mut s = normal_state();
        let a1 = key(&mut s, 'f');
        assert_eq!(a1, VimAction::Incomplete);
        let a2 = key(&mut s, 'x');
        assert_eq!(
            a2,
            VimAction::Motion {
                motion: VimMotionKind::FindChar('x'),
                count: 1
            }
        );
    }

    #[test]
    fn motion_t_char() {
        let mut s = normal_state();
        let a1 = key(&mut s, 't');
        assert_eq!(a1, VimAction::Incomplete);
        let a2 = key(&mut s, 'x');
        assert_eq!(
            a2,
            VimAction::Motion {
                motion: VimMotionKind::TillChar('x'),
                count: 1
            }
        );
    }

    // -- Operator-motion composition ----------------------------------------

    #[test]
    fn operator_dw_delete_word() {
        let mut s = normal_state();
        let a1 = key(&mut s, 'd');
        assert_eq!(a1, VimAction::Incomplete);
        let a2 = key(&mut s, 'w');
        assert_eq!(
            a2,
            VimAction::OperatorMotion {
                operator: VimOperatorKind::Delete,
                count: 1,
                motion: VimMotionKind::WordForward
            }
        );
        assert_eq!(s.mode(), EditorInputMode::Normal);
    }

    #[test]
    fn operator_cw_change_word() {
        let mut s = normal_state();
        key(&mut s, 'c');
        let a = key(&mut s, 'w');
        assert_eq!(
            a,
            VimAction::OperatorMotion {
                operator: VimOperatorKind::Change,
                count: 1,
                motion: VimMotionKind::WordForward
            }
        );
        // `c` enters insert mode after deleting.
        assert_eq!(s.mode(), EditorInputMode::Insert);
    }

    #[test]
    fn operator_yw_yank_word() {
        let mut s = normal_state();
        key(&mut s, 'y');
        let a = key(&mut s, 'w');
        assert_eq!(
            a,
            VimAction::OperatorMotion {
                operator: VimOperatorKind::Yank,
                count: 1,
                motion: VimMotionKind::WordForward
            }
        );
        assert_eq!(s.mode(), EditorInputMode::Normal);
    }

    #[test]
    fn operator_d_dollar() {
        let mut s = normal_state();
        key(&mut s, 'd');
        let a = key(&mut s, '$');
        assert_eq!(
            a,
            VimAction::OperatorMotion {
                operator: VimOperatorKind::Delete,
                count: 1,
                motion: VimMotionKind::LineEnd
            }
        );
    }

    // -- Line-doubled operators ---------------------------------------------

    #[test]
    fn operator_dd_delete_line() {
        let mut s = normal_state();
        key(&mut s, 'd');
        let a = key(&mut s, 'd');
        assert_eq!(
            a,
            VimAction::LinewiseOperator {
                operator: VimOperatorKind::Delete,
                count: 1
            }
        );
    }

    #[test]
    fn operator_yy_yank_line() {
        let mut s = normal_state();
        key(&mut s, 'y');
        let a = key(&mut s, 'y');
        assert_eq!(
            a,
            VimAction::LinewiseOperator {
                operator: VimOperatorKind::Yank,
                count: 1
            }
        );
    }

    #[test]
    fn operator_cc_change_line() {
        let mut s = normal_state();
        key(&mut s, 'c');
        let a = key(&mut s, 'c');
        assert_eq!(
            a,
            VimAction::LinewiseOperator {
                operator: VimOperatorKind::Change,
                count: 1
            }
        );
        assert_eq!(s.mode(), EditorInputMode::Insert);
    }

    // -- Count prefix -------------------------------------------------------

    #[test]
    fn count_3dw() {
        let mut s = normal_state();
        assert_eq!(key(&mut s, '3'), VimAction::Incomplete);
        key(&mut s, 'd');
        let a = key(&mut s, 'w');
        assert_eq!(
            a,
            VimAction::OperatorMotion {
                operator: VimOperatorKind::Delete,
                count: 3,
                motion: VimMotionKind::WordForward
            }
        );
    }

    #[test]
    fn count_5j() {
        let mut s = normal_state();
        key(&mut s, '5');
        let a = key(&mut s, 'j');
        assert_eq!(
            a,
            VimAction::Motion {
                motion: VimMotionKind::Down,
                count: 5
            }
        );
    }

    #[test]
    fn count_3dd() {
        let mut s = normal_state();
        key(&mut s, '3');
        key(&mut s, 'd');
        let a = key(&mut s, 'd');
        assert_eq!(
            a,
            VimAction::LinewiseOperator {
                operator: VimOperatorKind::Delete,
                count: 3
            }
        );
    }

    #[test]
    fn count_12j() {
        let mut s = normal_state();
        key(&mut s, '1');
        key(&mut s, '2');
        let a = key(&mut s, 'j');
        assert_eq!(
            a,
            VimAction::Motion {
                motion: VimMotionKind::Down,
                count: 12
            }
        );
    }

    #[test]
    fn zero_is_line_start_not_count() {
        let mut s = normal_state();
        let a = key(&mut s, '0');
        assert_eq!(
            a,
            VimAction::Motion {
                motion: VimMotionKind::LineStart,
                count: 1
            }
        );
    }

    #[test]
    fn count_10_zero_extends_count() {
        let mut s = normal_state();
        key(&mut s, '1');
        key(&mut s, '0'); // this is a digit in a count context
        let a = key(&mut s, 'j');
        assert_eq!(
            a,
            VimAction::Motion {
                motion: VimMotionKind::Down,
                count: 10
            }
        );
    }

    // -- Insert mode pass-through -------------------------------------------

    #[test]
    fn insert_mode_pass_through() {
        let mut s = normal_state();
        key(&mut s, 'i');
        assert_eq!(s.mode(), EditorInputMode::Insert);

        // Regular keys in insert mode return Unknown (pass-through).
        assert_eq!(key(&mut s, 'a'), VimAction::Unknown);
        assert_eq!(key(&mut s, 'x'), VimAction::Unknown);
        assert_eq!(key(&mut s, '1'), VimAction::Unknown);
    }

    // -- Undo / Redo --------------------------------------------------------

    #[test]
    fn undo() {
        let mut s = normal_state();
        assert_eq!(key(&mut s, 'u'), VimAction::Undo);
    }

    #[test]
    fn redo_ctrl_r() {
        let mut s = normal_state();
        assert_eq!(ctrl_key(&mut s, 'r'), VimAction::Redo);
    }

    // -- Single-key actions -------------------------------------------------

    #[test]
    fn put() {
        let mut s = normal_state();
        assert_eq!(key(&mut s, 'p'), VimAction::Put);
    }

    #[test]
    fn search_forward() {
        let mut s = normal_state();
        assert_eq!(key(&mut s, '/'), VimAction::SearchForward);
    }

    #[test]
    fn delete_char_x() {
        let mut s = normal_state();
        assert_eq!(key(&mut s, 'x'), VimAction::DeleteChar);
    }

    // -- Repeat (.) ---------------------------------------------------------

    #[test]
    fn repeat_replays_last_action() {
        let mut s = normal_state();
        key(&mut s, 'x'); // records DeleteChar
        let a = key(&mut s, '.');
        assert_eq!(a, VimAction::DeleteChar);
    }

    #[test]
    fn repeat_with_no_prior_action() {
        let mut s = normal_state();
        let a = key(&mut s, '.');
        assert_eq!(a, VimAction::Unknown);
    }

    // -- Visual mode motions ------------------------------------------------

    #[test]
    fn visual_mode_motions() {
        let mut s = normal_state();
        key(&mut s, 'v');
        assert_eq!(s.mode(), EditorInputMode::Visual);
        assert_eq!(
            key(&mut s, 'w'),
            VimAction::Motion {
                motion: VimMotionKind::WordForward,
                count: 1
            }
        );
        assert_eq!(
            key(&mut s, 'j'),
            VimAction::Motion {
                motion: VimMotionKind::Down,
                count: 1
            }
        );
    }

    // -- Pending key display ------------------------------------------------

    #[test]
    fn pending_display_operator() {
        let mut s = normal_state();
        key(&mut s, 'd');
        assert_eq!(s.pending_keys_display(), "d");
    }

    #[test]
    fn pending_display_count_operator() {
        let mut s = normal_state();
        key(&mut s, '3');
        key(&mut s, 'd');
        assert_eq!(s.pending_keys_display(), "3d");
    }

    #[test]
    fn pending_display_g() {
        let mut s = normal_state();
        key(&mut s, 'g');
        assert_eq!(s.pending_keys_display(), "g");
    }

    #[test]
    fn pending_display_empty_after_complete() {
        let mut s = normal_state();
        key(&mut s, 'j');
        assert_eq!(s.pending_keys_display(), "");
    }

    // -- Unknown key in operator context ------------------------------------

    #[test]
    fn unknown_motion_after_operator() {
        let mut s = normal_state();
        key(&mut s, 'd');
        let a = key(&mut s, 'z'); // not a motion
        assert_eq!(a, VimAction::Unknown);
        // pending is cleared
        assert_eq!(s.pending_keys_display(), "");
    }

    // -- Unknown key after g ------------------------------------------------

    #[test]
    fn unknown_after_g() {
        let mut s = normal_state();
        key(&mut s, 'g');
        let a = key(&mut s, 'z'); // not 'g'
        assert_eq!(a, VimAction::Unknown);
    }
}
