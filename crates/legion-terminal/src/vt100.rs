//! VT100/xterm escape sequence interpreter and 2D cell grid model.
//!
//! Processes byte streams and maintains a terminal grid where each cell carries
//! a character and display attributes (foreground, background, bold, italic,
//! underline, inverse, etc.). Handles partial escape sequences across buffer
//! boundaries.

/// Terminal color model supporting default, 16 standard, 256 indexed, and 24-bit RGB.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Color {
    /// Terminal default color (foreground or background).
    Default,
    /// Indexed color: 0-15 standard, 16-231 cube, 232-255 grayscale.
    Indexed(u8),
    /// 24-bit true color.
    Rgb(u8, u8, u8),
}

impl Default for Color {
    fn default() -> Self {
        Color::Default
    }
}

/// Per-cell display attributes applied to rendered characters.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CellAttributes {
    /// Foreground color.
    pub fg: Color,
    /// Background color.
    pub bg: Color,
    /// Bold (increased intensity).
    pub bold: bool,
    /// Dim (decreased intensity).
    pub dim: bool,
    /// Italic style.
    pub italic: bool,
    /// Underline style.
    pub underline: bool,
    /// Strikethrough style.
    pub strikethrough: bool,
    /// Inverse (swap foreground/background).
    pub inverse: bool,
    /// Hidden (invisible text).
    pub hidden: bool,
}

impl Default for CellAttributes {
    fn default() -> Self {
        Self {
            fg: Color::Default,
            bg: Color::Default,
            bold: false,
            dim: false,
            italic: false,
            underline: false,
            strikethrough: false,
            inverse: false,
            hidden: false,
        }
    }
}

/// A single terminal cell containing a character and its display attributes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cell {
    /// The character displayed in this cell.
    pub ch: char,
    /// Display attributes for this cell.
    pub attrs: CellAttributes,
}

impl Default for Cell {
    fn default() -> Self {
        Self {
            ch: ' ',
            attrs: CellAttributes::default(),
        }
    }
}

/// Saved primary screen state when the alternate screen buffer is active.
#[derive(Debug, Clone)]
struct AltScreenState {
    grid: Vec<Vec<Cell>>,
    scrollback: Vec<Vec<Cell>>,
    cursor_row: usize,
    cursor_col: usize,
    cursor_visible: bool,
    current_attrs: CellAttributes,
}

/// Parser state machine for incremental escape sequence processing.
#[derive(Debug, Clone, PartialEq, Eq)]
enum ParserState {
    /// Normal character processing.
    Ground,
    /// ESC byte seen, awaiting next byte.
    Escape,
    /// CSI introducer seen (`ESC [`), collecting parameters.
    CsiParam {
        /// Accumulated parameter bytes (digits, semicolons, question marks).
        params: Vec<u8>,
        /// Whether this is a DEC private mode sequence (`?` prefix).
        private: bool,
    },
    /// OSC sequence seen (`ESC ]`), consuming until ST.
    OscString {
        /// Accumulated OSC content.
        content: Vec<u8>,
    },
}

impl Default for ParserState {
    fn default() -> Self {
        ParserState::Ground
    }
}

/// VT100/xterm terminal emulator with 2D cell grid and scrollback buffer.
///
/// Processes byte streams through `process()` and maintains the terminal state
/// including cursor position, cell attributes, scroll regions, and alternate
/// screen buffer.
pub struct TerminalEmulator {
    // Primary screen buffer
    cols: usize,
    rows: usize,
    grid: Vec<Vec<Cell>>,
    scrollback: Vec<Vec<Cell>>,
    scrollback_limit: usize,

    // Cursor state
    cursor_row: usize,
    cursor_col: usize,
    cursor_visible: bool,
    saved_cursor: Option<(usize, usize, CellAttributes)>,

    // Current attributes (applied to new characters)
    current_attrs: CellAttributes,

    // Scroll region (top, bottom) — 0-indexed, inclusive
    scroll_top: usize,
    scroll_bottom: usize,

    // DEC private modes
    alt_screen: Option<AltScreenState>,
    application_cursor_keys: bool,
    auto_wrap: bool,
    _origin_mode: bool,

    // Pending wrap: when a character is written at the last column, the cursor
    // stays there and the next printable character triggers a wrap + newline.
    pending_wrap: bool,

    // Parser state for handling partial sequences
    parser_state: ParserState,

    // Response buffer for device status reports (DSR)
    response_buf: Vec<u8>,
}

impl TerminalEmulator {
    /// Create a new terminal emulator with the given dimensions.
    ///
    /// Default scrollback limit is 1000 lines.
    pub fn new(cols: usize, rows: usize) -> Self {
        let cols = cols.max(1);
        let rows = rows.max(1);
        let grid = vec![vec![Cell::default(); cols]; rows];
        Self {
            cols,
            rows,
            grid,
            scrollback: Vec::new(),
            scrollback_limit: 1000,
            cursor_row: 0,
            cursor_col: 0,
            cursor_visible: true,
            saved_cursor: None,
            current_attrs: CellAttributes::default(),
            scroll_top: 0,
            scroll_bottom: rows - 1,
            alt_screen: None,
            application_cursor_keys: false,
            auto_wrap: true,
            _origin_mode: false,
            pending_wrap: false,
            parser_state: ParserState::Ground,
            response_buf: Vec::new(),
        }
    }

    /// Resize the terminal grid, reflowing content.
    pub fn resize(&mut self, cols: usize, rows: usize) {
        let cols = cols.max(1);
        let rows = rows.max(1);

        // Resize each existing row to the new column width
        for row in &mut self.grid {
            row.resize(cols, Cell::default());
        }

        // Add or remove rows
        if rows > self.rows {
            // Add empty rows at the bottom
            for _ in 0..(rows - self.rows) {
                self.grid.push(vec![Cell::default(); cols]);
            }
        } else if rows < self.rows {
            // Move excess rows to scrollback
            let excess = self.rows - rows;
            let to_remove = excess.min(self.grid.len());
            let removed: Vec<Vec<Cell>> = self.grid.drain(..to_remove).collect();
            for row in removed {
                self.push_scrollback(row);
            }
        }

        // Resize scrollback rows too
        for row in &mut self.scrollback {
            row.resize(cols, Cell::default());
        }

        self.cols = cols;
        self.rows = rows;
        self.scroll_top = 0;
        self.scroll_bottom = rows - 1;

        // Clamp cursor
        self.cursor_row = self.cursor_row.min(rows - 1);
        self.cursor_col = self.cursor_col.min(cols - 1);
        self.pending_wrap = false;
    }

    /// Read-only access to the terminal grid.
    pub fn grid(&self) -> &[Vec<Cell>] {
        &self.grid
    }

    /// Read-only access to the scrollback buffer.
    pub fn scrollback(&self) -> &[Vec<Cell>] {
        &self.scrollback
    }

    /// Current cursor position as (row, col).
    pub fn cursor_position(&self) -> (usize, usize) {
        (self.cursor_row, self.cursor_col)
    }

    /// Whether the cursor is visible.
    pub fn cursor_visible(&self) -> bool {
        self.cursor_visible
    }

    /// Whether the alternate screen buffer is active.
    pub fn is_alt_screen(&self) -> bool {
        self.alt_screen.is_some()
    }

    /// Whether application cursor key mode is active (DECCKM).
    pub fn application_cursor_keys(&self) -> bool {
        self.application_cursor_keys
    }

    /// Number of columns.
    pub fn cols(&self) -> usize {
        self.cols
    }

    /// Number of rows.
    pub fn rows(&self) -> usize {
        self.rows
    }

    /// Drain the response buffer (for device status reports).
    pub fn drain_response(&mut self) -> Vec<u8> {
        std::mem::take(&mut self.response_buf)
    }

    /// Process a byte stream, interpreting escape sequences and updating the grid.
    ///
    /// Handles partial escape sequences: if the buffer ends mid-sequence, state
    /// is saved and processing resumes on the next call.
    pub fn process(&mut self, data: &[u8]) {
        let mut i = 0;
        while i < data.len() {
            let byte = data[i];
            i += 1;

            match std::mem::take(&mut self.parser_state) {
                ParserState::Ground => {
                    self.process_ground(byte);
                }
                ParserState::Escape => {
                    self.process_escape(byte);
                }
                ParserState::CsiParam { mut params, private } => {
                    match byte {
                        // Parameter bytes: digits, semicolons, colons
                        b'0'..=b'9' | b';' | b':' => {
                            params.push(byte);
                            self.parser_state = ParserState::CsiParam { params, private };
                        }
                        // Private mode marker (if at start of params)
                        b'?' if params.is_empty() => {
                            self.parser_state = ParserState::CsiParam {
                                params,
                                private: true,
                            };
                        }
                        // Final byte: dispatch
                        b'@'..=b'~' => {
                            self.dispatch_csi(&params, private, byte);
                        }
                        // Intermediate bytes (space, !, ", #, etc.) — consume silently
                        b' '..=b'/' => {
                            // Intermediate bytes before final byte; consume but
                            // we only support the basic CSI sequences, so store
                            // and ignore for now
                            params.push(byte);
                            self.parser_state = ParserState::CsiParam { params, private };
                        }
                        _ => {
                            // Invalid: discard sequence and process byte as ground
                            self.process_ground(byte);
                        }
                    }
                }
                ParserState::OscString { mut content } => {
                    match byte {
                        // ST (String Terminator) as BEL
                        0x07 => {
                            // OSC complete; discard content (not handled)
                        }
                        // ESC might be start of ST (ESC \)
                        0x1b => {
                            // Peek at next byte
                            if i < data.len() && data[i] == b'\\' {
                                i += 1;
                                // OSC complete; discard content
                            } else {
                                // Treat as ESC starting a new sequence;
                                // discard the incomplete OSC
                                self.parser_state = ParserState::Escape;
                            }
                        }
                        _ => {
                            content.push(byte);
                            self.parser_state = ParserState::OscString { content };
                        }
                    }
                }
            }
        }
    }

    fn process_ground(&mut self, byte: u8) {
        match byte {
            // ESC
            0x1b => {
                self.parser_state = ParserState::Escape;
            }
            // LF, VT, FF — all treated as newline
            0x0a | 0x0b | 0x0c => {
                self.linefeed();
            }
            // CR
            0x0d => {
                self.cursor_col = 0;
                self.pending_wrap = false;
            }
            // TAB
            0x09 => {
                self.pending_wrap = false;
                // Advance to next tab stop (every 8 columns)
                let next_tab = ((self.cursor_col / 8) + 1) * 8;
                self.cursor_col = next_tab.min(self.cols - 1);
            }
            // BS (Backspace)
            0x08 => {
                self.pending_wrap = false;
                if self.cursor_col > 0 {
                    self.cursor_col -= 1;
                }
            }
            // BEL
            0x07 => {
                // Ignore
            }
            // NUL and other C0 controls: ignore
            0x00..=0x06 | 0x0e..=0x1a | 0x1c..=0x1f => {
                // Ignore
            }
            // Printable ASCII (and everything else treated as UTF-8 lead bytes)
            _ => {
                self.put_char(byte);
            }
        }
    }

    fn put_char(&mut self, first_byte: u8) {
        // Decode the character. For simplicity in this byte-oriented parser,
        // we handle ASCII directly and treat multi-byte UTF-8 as individual
        // replacement characters. A production parser would accumulate UTF-8
        // sequences, but the vast majority of terminal output is ASCII.
        let ch = if first_byte < 0x80 {
            first_byte as char
        } else {
            // Non-ASCII byte outside a complete UTF-8 sequence context.
            // Replace with Unicode replacement character.
            '\u{FFFD}'
        };

        if self.pending_wrap && self.auto_wrap {
            self.pending_wrap = false;
            self.cursor_col = 0;
            self.linefeed();
        }

        if self.cursor_row < self.rows && self.cursor_col < self.cols {
            self.grid[self.cursor_row][self.cursor_col] = Cell {
                ch,
                attrs: self.current_attrs.clone(),
            };
        }

        if self.cursor_col >= self.cols - 1 {
            // At last column: set pending wrap flag
            self.pending_wrap = true;
        } else {
            self.cursor_col += 1;
        }
    }

    fn process_escape(&mut self, byte: u8) {
        match byte {
            // CSI introducer
            b'[' => {
                self.parser_state = ParserState::CsiParam {
                    params: Vec::new(),
                    private: false,
                };
            }
            // OSC introducer
            b']' => {
                self.parser_state = ParserState::OscString {
                    content: Vec::new(),
                };
            }
            // Save cursor (DECSC)
            b'7' => {
                self.saved_cursor = Some((
                    self.cursor_row,
                    self.cursor_col,
                    self.current_attrs.clone(),
                ));
            }
            // Restore cursor (DECRC)
            b'8' => {
                if let Some((row, col, attrs)) = self.saved_cursor.clone() {
                    self.cursor_row = row.min(self.rows - 1);
                    self.cursor_col = col.min(self.cols - 1);
                    self.current_attrs = attrs;
                    self.pending_wrap = false;
                }
            }
            // Reverse index (RI) — move cursor up, scroll down if at top
            b'M' => {
                if self.cursor_row == self.scroll_top {
                    self.scroll_down_one();
                } else if self.cursor_row > 0 {
                    self.cursor_row -= 1;
                }
            }
            // Reset to initial state (RIS)
            b'c' => {
                let cols = self.cols;
                let rows = self.rows;
                *self = Self::new(cols, rows);
            }
            // ESC \ (ST) — String Terminator by itself; ignore
            b'\\' => {}
            // Anything else: discard the ESC sequence
            _ => {}
        }
    }

    fn dispatch_csi(&mut self, params: &[u8], private: bool, final_byte: u8) {
        let parsed = parse_csi_params(params);

        if private {
            self.dispatch_dec_private(&parsed, final_byte);
            return;
        }

        match final_byte {
            // CUU — Cursor Up
            b'A' => {
                let n = parsed.first().copied().unwrap_or(1).max(1) as usize;
                self.cursor_row = self.cursor_row.saturating_sub(n);
                self.pending_wrap = false;
            }
            // CUB — Cursor Down (note: CUB is actually Cursor Back; B is Cursor Down)
            b'B' => {
                let n = parsed.first().copied().unwrap_or(1).max(1) as usize;
                self.cursor_row = (self.cursor_row + n).min(self.rows - 1);
                self.pending_wrap = false;
            }
            // CUF — Cursor Forward
            b'C' => {
                let n = parsed.first().copied().unwrap_or(1).max(1) as usize;
                self.cursor_col = (self.cursor_col + n).min(self.cols - 1);
                self.pending_wrap = false;
            }
            // CUB — Cursor Back
            b'D' => {
                let n = parsed.first().copied().unwrap_or(1).max(1) as usize;
                self.cursor_col = self.cursor_col.saturating_sub(n);
                self.pending_wrap = false;
            }
            // CHA — Cursor Horizontal Absolute (column)
            b'G' => {
                let n = parsed.first().copied().unwrap_or(1).max(1) as usize;
                self.cursor_col = (n - 1).min(self.cols - 1);
                self.pending_wrap = false;
            }
            // CUP / HVP — Cursor Position
            b'H' | b'f' => {
                let row = parsed.first().copied().unwrap_or(1).max(1) as usize;
                let col = parsed.get(1).copied().unwrap_or(1).max(1) as usize;
                self.cursor_row = (row - 1).min(self.rows - 1);
                self.cursor_col = (col - 1).min(self.cols - 1);
                self.pending_wrap = false;
            }
            // ED — Erase in Display
            b'J' => {
                let mode = parsed.first().copied().unwrap_or(0);
                self.erase_in_display(mode);
            }
            // EL — Erase in Line
            b'K' => {
                let mode = parsed.first().copied().unwrap_or(0);
                self.erase_in_line(mode);
            }
            // IL — Insert Lines
            b'L' => {
                let n = parsed.first().copied().unwrap_or(1).max(1) as usize;
                self.insert_lines(n);
            }
            // DL — Delete Lines
            b'M' => {
                let n = parsed.first().copied().unwrap_or(1).max(1) as usize;
                self.delete_lines(n);
            }
            // DCH — Delete Characters
            b'P' => {
                let n = parsed.first().copied().unwrap_or(1).max(1) as usize;
                self.delete_chars(n);
            }
            // SU — Scroll Up
            b'S' => {
                let n = parsed.first().copied().unwrap_or(1).max(1) as usize;
                for _ in 0..n {
                    self.scroll_up_one();
                }
            }
            // SD — Scroll Down
            b'T' => {
                let n = parsed.first().copied().unwrap_or(1).max(1) as usize;
                for _ in 0..n {
                    self.scroll_down_one();
                }
            }
            // ICH — Insert Characters
            b'@' => {
                let n = parsed.first().copied().unwrap_or(1).max(1) as usize;
                self.insert_chars(n);
            }
            // VPA — Vertical Position Absolute (row)
            b'd' => {
                let n = parsed.first().copied().unwrap_or(1).max(1) as usize;
                self.cursor_row = (n - 1).min(self.rows - 1);
                self.pending_wrap = false;
            }
            // SGR — Select Graphic Rendition
            b'm' => {
                self.process_sgr(&parsed);
            }
            // DSR — Device Status Report
            b'n' => {
                let mode = parsed.first().copied().unwrap_or(0);
                if mode == 6 {
                    // Report cursor position
                    let response = format!(
                        "\x1b[{};{}R",
                        self.cursor_row + 1,
                        self.cursor_col + 1
                    );
                    self.response_buf.extend_from_slice(response.as_bytes());
                }
            }
            // DECSTBM — Set Scrolling Region
            b'r' => {
                let top = parsed.first().copied().unwrap_or(1).max(1) as usize;
                let bottom = parsed
                    .get(1)
                    .copied()
                    .unwrap_or(self.rows as u32)
                    .max(1) as usize;
                if top < bottom && top >= 1 && bottom <= self.rows {
                    self.scroll_top = top - 1;
                    self.scroll_bottom = bottom - 1;
                    // Home cursor after setting scroll region
                    self.cursor_row = 0;
                    self.cursor_col = 0;
                    self.pending_wrap = false;
                }
            }
            // DECSC — Save Cursor Position (CSI s)
            b's' => {
                self.saved_cursor = Some((
                    self.cursor_row,
                    self.cursor_col,
                    self.current_attrs.clone(),
                ));
            }
            // DECRC — Restore Cursor Position (CSI u)
            b'u' => {
                if let Some((row, col, attrs)) = self.saved_cursor.clone() {
                    self.cursor_row = row.min(self.rows - 1);
                    self.cursor_col = col.min(self.cols - 1);
                    self.current_attrs = attrs;
                    self.pending_wrap = false;
                }
            }
            // ECH — Erase Characters
            b'X' => {
                let n = parsed.first().copied().unwrap_or(1).max(1) as usize;
                let row = self.cursor_row;
                let start = self.cursor_col;
                let end = (start + n).min(self.cols);
                for col in start..end {
                    self.grid[row][col] = Cell::default();
                }
            }
            // Unknown final byte: silently discard
            _ => {}
        }
    }

    fn dispatch_dec_private(&mut self, params: &[u32], final_byte: u8) {
        let set = final_byte == b'h';
        // Only handle set (h) and reset (l)
        if final_byte != b'h' && final_byte != b'l' {
            return;
        }

        for &param in params {
            match param {
                // DECCKM — Application Cursor Keys
                1 => {
                    self.application_cursor_keys = set;
                }
                // DECAWM — Auto-wrap Mode
                7 => {
                    self.auto_wrap = set;
                }
                // Cursor blink — ignore
                12 => {}
                // DECTCEM — Cursor Visibility
                25 => {
                    self.cursor_visible = set;
                }
                // Alt screen buffer (1049: save cursor + switch + clear)
                1049 => {
                    if set {
                        self.enter_alt_screen();
                    } else {
                        self.exit_alt_screen();
                    }
                }
                // Unknown private mode: ignore
                _ => {}
            }
        }
    }

    fn process_sgr(&mut self, params: &[u32]) {
        if params.is_empty() {
            // No params = reset
            self.current_attrs = CellAttributes::default();
            return;
        }

        let mut i = 0;
        while i < params.len() {
            match params[i] {
                0 => self.current_attrs = CellAttributes::default(),
                1 => self.current_attrs.bold = true,
                2 => self.current_attrs.dim = true,
                3 => self.current_attrs.italic = true,
                4 => self.current_attrs.underline = true,
                7 => self.current_attrs.inverse = true,
                8 => self.current_attrs.hidden = true,
                9 => self.current_attrs.strikethrough = true,
                22 => {
                    self.current_attrs.bold = false;
                    self.current_attrs.dim = false;
                }
                23 => self.current_attrs.italic = false,
                24 => self.current_attrs.underline = false,
                27 => self.current_attrs.inverse = false,
                28 => self.current_attrs.hidden = false,
                29 => self.current_attrs.strikethrough = false,
                // Standard foreground: 30-37
                30..=37 => {
                    self.current_attrs.fg = Color::Indexed((params[i] - 30) as u8);
                }
                // Extended foreground: 38;5;N or 38;2;R;G;B
                38 => {
                    i += 1;
                    if i < params.len() {
                        match params[i] {
                            5 => {
                                i += 1;
                                if i < params.len() {
                                    self.current_attrs.fg =
                                        Color::Indexed(params[i] as u8);
                                }
                            }
                            2 => {
                                if i + 3 < params.len() {
                                    let r = params[i + 1] as u8;
                                    let g = params[i + 2] as u8;
                                    let b = params[i + 3] as u8;
                                    self.current_attrs.fg = Color::Rgb(r, g, b);
                                    i += 3;
                                }
                            }
                            _ => {}
                        }
                    }
                }
                // Default foreground
                39 => self.current_attrs.fg = Color::Default,
                // Standard background: 40-47
                40..=47 => {
                    self.current_attrs.bg = Color::Indexed((params[i] - 40) as u8);
                }
                // Extended background: 48;5;N or 48;2;R;G;B
                48 => {
                    i += 1;
                    if i < params.len() {
                        match params[i] {
                            5 => {
                                i += 1;
                                if i < params.len() {
                                    self.current_attrs.bg =
                                        Color::Indexed(params[i] as u8);
                                }
                            }
                            2 => {
                                if i + 3 < params.len() {
                                    let r = params[i + 1] as u8;
                                    let g = params[i + 2] as u8;
                                    let b = params[i + 3] as u8;
                                    self.current_attrs.bg = Color::Rgb(r, g, b);
                                    i += 3;
                                }
                            }
                            _ => {}
                        }
                    }
                }
                // Default background
                49 => self.current_attrs.bg = Color::Default,
                // Bright foreground: 90-97
                90..=97 => {
                    self.current_attrs.fg = Color::Indexed((params[i] - 90 + 8) as u8);
                }
                // Bright background: 100-107
                100..=107 => {
                    self.current_attrs.bg = Color::Indexed((params[i] - 100 + 8) as u8);
                }
                // Unknown SGR: ignore
                _ => {}
            }
            i += 1;
        }
    }

    fn linefeed(&mut self) {
        self.pending_wrap = false;
        self.cursor_col = 0;
        if self.cursor_row == self.scroll_bottom {
            self.scroll_up_one();
        } else if self.cursor_row < self.rows - 1 {
            self.cursor_row += 1;
        }
    }

    fn scroll_up_one(&mut self) {
        if self.scroll_top >= self.grid.len() || self.scroll_bottom >= self.grid.len() {
            return;
        }
        let removed = self.grid.remove(self.scroll_top);
        // Only push to scrollback when the scroll region is the full screen
        if self.scroll_top == 0 {
            self.push_scrollback(removed);
        }
        self.grid
            .insert(self.scroll_bottom, vec![Cell::default(); self.cols]);
    }

    fn scroll_down_one(&mut self) {
        if self.scroll_top >= self.grid.len() || self.scroll_bottom >= self.grid.len() {
            return;
        }
        self.grid.remove(self.scroll_bottom);
        self.grid
            .insert(self.scroll_top, vec![Cell::default(); self.cols]);
    }

    fn push_scrollback(&mut self, row: Vec<Cell>) {
        self.scrollback.push(row);
        while self.scrollback.len() > self.scrollback_limit {
            self.scrollback.remove(0);
        }
    }

    fn erase_in_display(&mut self, mode: u32) {
        match mode {
            // Erase below (from cursor to end)
            0 => {
                // Current line from cursor to end
                let row = self.cursor_row;
                for col in self.cursor_col..self.cols {
                    self.grid[row][col] = Cell::default();
                }
                // All lines below
                for r in (row + 1)..self.rows {
                    for col in 0..self.cols {
                        self.grid[r][col] = Cell::default();
                    }
                }
            }
            // Erase above (from start to cursor)
            1 => {
                // All lines above
                for r in 0..self.cursor_row {
                    for col in 0..self.cols {
                        self.grid[r][col] = Cell::default();
                    }
                }
                // Current line from start to cursor (inclusive)
                for col in 0..=self.cursor_col.min(self.cols - 1) {
                    self.grid[self.cursor_row][col] = Cell::default();
                }
            }
            // Erase all
            2 => {
                for r in 0..self.rows {
                    for col in 0..self.cols {
                        self.grid[r][col] = Cell::default();
                    }
                }
            }
            // Erase all + scrollback
            3 => {
                for r in 0..self.rows {
                    for col in 0..self.cols {
                        self.grid[r][col] = Cell::default();
                    }
                }
                self.scrollback.clear();
            }
            _ => {}
        }
    }

    fn erase_in_line(&mut self, mode: u32) {
        let row = self.cursor_row;
        match mode {
            // Erase right (from cursor to end of line)
            0 => {
                for col in self.cursor_col..self.cols {
                    self.grid[row][col] = Cell::default();
                }
            }
            // Erase left (from start to cursor)
            1 => {
                for col in 0..=self.cursor_col.min(self.cols - 1) {
                    self.grid[row][col] = Cell::default();
                }
            }
            // Erase entire line
            2 => {
                for col in 0..self.cols {
                    self.grid[row][col] = Cell::default();
                }
            }
            _ => {}
        }
    }

    fn insert_lines(&mut self, n: usize) {
        if self.cursor_row < self.scroll_top || self.cursor_row > self.scroll_bottom {
            return;
        }
        let n = n.min(self.scroll_bottom - self.cursor_row + 1);
        for _ in 0..n {
            if self.scroll_bottom < self.grid.len() {
                self.grid.remove(self.scroll_bottom);
            }
            self.grid
                .insert(self.cursor_row, vec![Cell::default(); self.cols]);
        }
    }

    fn delete_lines(&mut self, n: usize) {
        if self.cursor_row < self.scroll_top || self.cursor_row > self.scroll_bottom {
            return;
        }
        let n = n.min(self.scroll_bottom - self.cursor_row + 1);
        for _ in 0..n {
            self.grid.remove(self.cursor_row);
            self.grid
                .insert(self.scroll_bottom, vec![Cell::default(); self.cols]);
        }
    }

    fn insert_chars(&mut self, n: usize) {
        let row = self.cursor_row;
        let col = self.cursor_col;
        let n = n.min(self.cols - col);
        // Shift characters right
        for _ in 0..n {
            self.grid[row].pop();
            self.grid[row].insert(col, Cell::default());
        }
        // Ensure row stays at correct width
        self.grid[row].resize(self.cols, Cell::default());
    }

    fn delete_chars(&mut self, n: usize) {
        let row = self.cursor_row;
        let col = self.cursor_col;
        let n = n.min(self.cols - col);
        // Remove characters at cursor, fill with blanks at end
        for _ in 0..n {
            if col < self.grid[row].len() {
                self.grid[row].remove(col);
            }
        }
        // Pad to full width
        self.grid[row].resize(self.cols, Cell::default());
    }

    fn enter_alt_screen(&mut self) {
        let saved = AltScreenState {
            grid: self.grid.clone(),
            scrollback: self.scrollback.clone(),
            cursor_row: self.cursor_row,
            cursor_col: self.cursor_col,
            cursor_visible: self.cursor_visible,
            current_attrs: self.current_attrs.clone(),
        };
        self.alt_screen = Some(saved);
        // Clear screen and home cursor on alt screen
        self.grid = vec![vec![Cell::default(); self.cols]; self.rows];
        self.scrollback = Vec::new();
        self.cursor_row = 0;
        self.cursor_col = 0;
        self.pending_wrap = false;
    }

    fn exit_alt_screen(&mut self) {
        if let Some(saved) = self.alt_screen.take() {
            self.grid = saved.grid;
            self.scrollback = saved.scrollback;
            self.cursor_row = saved.cursor_row.min(self.rows - 1);
            self.cursor_col = saved.cursor_col.min(self.cols - 1);
            self.cursor_visible = saved.cursor_visible;
            self.current_attrs = saved.current_attrs;
            self.pending_wrap = false;
        }
    }
}

/// Parse CSI parameter bytes into a list of u32 values.
/// Semicolons separate parameters; empty/missing parameters default to 0.
fn parse_csi_params(params: &[u8]) -> Vec<u32> {
    if params.is_empty() {
        return Vec::new();
    }

    // Filter out any non-digit, non-semicolon bytes (intermediate bytes like space)
    let filtered: Vec<u8> = params
        .iter()
        .copied()
        .filter(|&b| b.is_ascii_digit() || b == b';')
        .collect();

    if filtered.is_empty() {
        return Vec::new();
    }

    filtered
        .split(|&b| b == b';')
        .map(|part| {
            let mut val: u32 = 0;
            for &b in part {
                if b.is_ascii_digit() {
                    val = val.saturating_mul(10).saturating_add((b - b'0') as u32);
                }
            }
            val
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    // ---- Helper ----

    /// Extract text from a single row (0-indexed), trimmed.
    fn row_text(emu: &TerminalEmulator, row: usize) -> String {
        emu.grid()[row]
            .iter()
            .map(|cell| cell.ch)
            .collect::<String>()
            .trim_end()
            .to_string()
    }

    // ---- Task 1: Cell, CellAttributes, Color types ----

    #[test]
    fn color_default_is_default() {
        assert_eq!(Color::default(), Color::Default);
    }

    #[test]
    fn cell_attributes_default_has_no_styles() {
        let attrs = CellAttributes::default();
        assert_eq!(attrs.fg, Color::Default);
        assert_eq!(attrs.bg, Color::Default);
        assert!(!attrs.bold);
        assert!(!attrs.dim);
        assert!(!attrs.italic);
        assert!(!attrs.underline);
        assert!(!attrs.strikethrough);
        assert!(!attrs.inverse);
        assert!(!attrs.hidden);
    }

    #[test]
    fn cell_default_is_space_with_default_attrs() {
        let cell = Cell::default();
        assert_eq!(cell.ch, ' ');
        assert_eq!(cell.attrs, CellAttributes::default());
    }

    #[test]
    fn cell_clone_and_eq() {
        let cell = Cell {
            ch: 'A',
            attrs: CellAttributes {
                fg: Color::Indexed(1),
                bold: true,
                ..CellAttributes::default()
            },
        };
        let cloned = cell.clone();
        assert_eq!(cell, cloned);
    }

    // ---- Task 2: TerminalEmulator construction and accessors ----

    #[test]
    fn new_creates_grid_with_correct_dimensions() {
        let emu = TerminalEmulator::new(80, 24);
        assert_eq!(emu.grid().len(), 24);
        assert_eq!(emu.grid()[0].len(), 80);
        assert_eq!(emu.cols(), 80);
        assert_eq!(emu.rows(), 24);
    }

    #[test]
    fn new_cursor_at_origin_and_visible() {
        let emu = TerminalEmulator::new(80, 24);
        assert_eq!(emu.cursor_position(), (0, 0));
        assert!(emu.cursor_visible());
        assert!(!emu.is_alt_screen());
    }

    #[test]
    fn new_minimum_size_is_1x1() {
        let emu = TerminalEmulator::new(0, 0);
        assert_eq!(emu.cols(), 1);
        assert_eq!(emu.rows(), 1);
    }

    #[test]
    fn resize_grows_grid() {
        let mut emu = TerminalEmulator::new(10, 5);
        emu.process(b"hello");
        emu.resize(20, 10);
        assert_eq!(emu.cols(), 20);
        assert_eq!(emu.rows(), 10);
        assert_eq!(emu.grid().len(), 10);
        assert_eq!(emu.grid()[0].len(), 20);
        // Content should be preserved
        assert_eq!(row_text(&emu, 0), "hello");
    }

    #[test]
    fn resize_shrinks_grid_moves_to_scrollback() {
        let mut emu = TerminalEmulator::new(10, 5);
        // Write text on first line
        emu.process(b"line1");
        emu.process(b"\nline2");
        emu.process(b"\nline3");
        emu.resize(10, 2);
        assert_eq!(emu.rows(), 2);
        assert_eq!(emu.grid().len(), 2);
        // Some lines should have moved to scrollback
        assert!(!emu.scrollback().is_empty());
    }

    // ---- Task 3: Basic text output and cursor positioning ----

    #[test]
    fn basic_text_output() {
        let mut emu = TerminalEmulator::new(80, 24);
        emu.process(b"Hello, World!");
        assert_eq!(row_text(&emu, 0), "Hello, World!");
        assert_eq!(emu.cursor_position(), (0, 13));
    }

    #[test]
    fn newline_moves_cursor_down() {
        let mut emu = TerminalEmulator::new(80, 24);
        emu.process(b"line1\nline2");
        assert_eq!(row_text(&emu, 0), "line1");
        assert_eq!(row_text(&emu, 1), "line2");
        assert_eq!(emu.cursor_position(), (1, 5));
    }

    #[test]
    fn carriage_return_moves_to_column_zero() {
        let mut emu = TerminalEmulator::new(80, 24);
        emu.process(b"hello\rworld");
        // "world" overwrites "hello" starting from column 0
        assert_eq!(row_text(&emu, 0), "world");
    }

    #[test]
    fn crlf_combination() {
        let mut emu = TerminalEmulator::new(80, 24);
        emu.process(b"line1\r\nline2");
        assert_eq!(row_text(&emu, 0), "line1");
        assert_eq!(row_text(&emu, 1), "line2");
    }

    #[test]
    fn tab_advances_to_next_tab_stop() {
        let mut emu = TerminalEmulator::new(80, 24);
        emu.process(b"a\tb");
        assert_eq!(emu.cursor_position().1, 9); // tab to col 8, then 'b' at 8, cursor at 9
        // Check 'a' at col 0 and 'b' at col 8
        assert_eq!(emu.grid()[0][0].ch, 'a');
        assert_eq!(emu.grid()[0][8].ch, 'b');
    }

    #[test]
    fn backspace_moves_cursor_left() {
        let mut emu = TerminalEmulator::new(80, 24);
        emu.process(b"abc\x08");
        assert_eq!(emu.cursor_position(), (0, 2));
        // Backspace at column 0 stays at 0
        let mut emu2 = TerminalEmulator::new(80, 24);
        emu2.process(b"\x08");
        assert_eq!(emu2.cursor_position(), (0, 0));
    }

    #[test]
    fn line_wrap_at_end_of_row() {
        let mut emu = TerminalEmulator::new(5, 3);
        emu.process(b"abcdefgh");
        // 'abcde' on row 0, 'fgh' on row 1
        assert_eq!(row_text(&emu, 0), "abcde");
        assert_eq!(row_text(&emu, 1), "fgh");
    }

    // ---- CSI cursor movement ----

    #[test]
    fn csi_cursor_up() {
        let mut emu = TerminalEmulator::new(80, 24);
        emu.process(b"\x1b[5;10H"); // Move to row 5, col 10
        emu.process(b"\x1b[2A"); // Up 2
        assert_eq!(emu.cursor_position(), (2, 9));
    }

    #[test]
    fn csi_cursor_down() {
        let mut emu = TerminalEmulator::new(80, 24);
        emu.process(b"\x1b[2B"); // Down 2
        assert_eq!(emu.cursor_position(), (2, 0));
    }

    #[test]
    fn csi_cursor_forward() {
        let mut emu = TerminalEmulator::new(80, 24);
        emu.process(b"\x1b[5C"); // Forward 5
        assert_eq!(emu.cursor_position(), (0, 5));
    }

    #[test]
    fn csi_cursor_back() {
        let mut emu = TerminalEmulator::new(80, 24);
        emu.process(b"\x1b[10;10H"); // row 10, col 10
        emu.process(b"\x1b[3D"); // Back 3
        assert_eq!(emu.cursor_position(), (9, 6));
    }

    #[test]
    fn csi_cursor_position() {
        let mut emu = TerminalEmulator::new(80, 24);
        emu.process(b"\x1b[10;20H");
        assert_eq!(emu.cursor_position(), (9, 19)); // 1-based to 0-based

        // Home position (no params)
        emu.process(b"\x1b[H");
        assert_eq!(emu.cursor_position(), (0, 0));
    }

    #[test]
    fn csi_cursor_column() {
        let mut emu = TerminalEmulator::new(80, 24);
        emu.process(b"\x1b[15G");
        assert_eq!(emu.cursor_position(), (0, 14));
    }

    #[test]
    fn csi_cursor_row() {
        let mut emu = TerminalEmulator::new(80, 24);
        emu.process(b"\x1b[5d");
        assert_eq!(emu.cursor_position(), (4, 0));
    }

    // ---- SGR color attributes ----

    #[test]
    fn sgr_standard_foreground_red() {
        let mut emu = TerminalEmulator::new(80, 24);
        emu.process(b"\x1b[31mhello\x1b[0m");
        let cell = &emu.grid()[0][0];
        assert_eq!(cell.ch, 'h');
        assert_eq!(cell.attrs.fg, Color::Indexed(1)); // red = index 1
        // After reset
        let cell_after = &emu.grid()[0][5];
        assert_eq!(cell_after.attrs.fg, Color::Default);
    }

    #[test]
    fn sgr_256_color_foreground() {
        let mut emu = TerminalEmulator::new(80, 24);
        emu.process(b"\x1b[38;5;196mX");
        assert_eq!(emu.grid()[0][0].attrs.fg, Color::Indexed(196));
    }

    #[test]
    fn sgr_rgb_foreground() {
        let mut emu = TerminalEmulator::new(80, 24);
        emu.process(b"\x1b[38;2;255;128;0mX");
        assert_eq!(emu.grid()[0][0].attrs.fg, Color::Rgb(255, 128, 0));
    }

    #[test]
    fn sgr_standard_background() {
        let mut emu = TerminalEmulator::new(80, 24);
        emu.process(b"\x1b[42mX"); // green background
        assert_eq!(emu.grid()[0][0].attrs.bg, Color::Indexed(2));
    }

    #[test]
    fn sgr_256_color_background() {
        let mut emu = TerminalEmulator::new(80, 24);
        emu.process(b"\x1b[48;5;100mX");
        assert_eq!(emu.grid()[0][0].attrs.bg, Color::Indexed(100));
    }

    #[test]
    fn sgr_rgb_background() {
        let mut emu = TerminalEmulator::new(80, 24);
        emu.process(b"\x1b[48;2;10;20;30mX");
        assert_eq!(emu.grid()[0][0].attrs.bg, Color::Rgb(10, 20, 30));
    }

    #[test]
    fn sgr_bright_foreground() {
        let mut emu = TerminalEmulator::new(80, 24);
        emu.process(b"\x1b[91mX"); // bright red
        assert_eq!(emu.grid()[0][0].attrs.fg, Color::Indexed(9));
    }

    #[test]
    fn sgr_bright_background() {
        let mut emu = TerminalEmulator::new(80, 24);
        emu.process(b"\x1b[101mX"); // bright red bg
        assert_eq!(emu.grid()[0][0].attrs.bg, Color::Indexed(9));
    }

    #[test]
    fn sgr_bold_italic_underline() {
        let mut emu = TerminalEmulator::new(80, 24);
        emu.process(b"\x1b[1;3;4mX");
        let attrs = &emu.grid()[0][0].attrs;
        assert!(attrs.bold);
        assert!(attrs.italic);
        assert!(attrs.underline);
    }

    #[test]
    fn sgr_reset_individual_attributes() {
        let mut emu = TerminalEmulator::new(80, 24);
        emu.process(b"\x1b[1;3;4;7;9mA"); // Set all
        emu.process(b"\x1b[22;23;24;27;29mB"); // Reset individually

        let a_attrs = &emu.grid()[0][0].attrs;
        assert!(a_attrs.bold);
        assert!(a_attrs.italic);
        assert!(a_attrs.underline);
        assert!(a_attrs.inverse);
        assert!(a_attrs.strikethrough);

        let b_attrs = &emu.grid()[0][1].attrs;
        assert!(!b_attrs.bold);
        assert!(!b_attrs.italic);
        assert!(!b_attrs.underline);
        assert!(!b_attrs.inverse);
        assert!(!b_attrs.strikethrough);
    }

    #[test]
    fn sgr_default_foreground_and_background() {
        let mut emu = TerminalEmulator::new(80, 24);
        emu.process(b"\x1b[31;42mA\x1b[39;49mB");
        assert_eq!(emu.grid()[0][0].attrs.fg, Color::Indexed(1));
        assert_eq!(emu.grid()[0][0].attrs.bg, Color::Indexed(2));
        assert_eq!(emu.grid()[0][1].attrs.fg, Color::Default);
        assert_eq!(emu.grid()[0][1].attrs.bg, Color::Default);
    }

    // ---- CSI erase commands ----

    #[test]
    fn erase_in_display_clear_all_and_home() {
        let mut emu = TerminalEmulator::new(80, 24);
        emu.process(b"hello world");
        emu.process(b"\x1b[2J\x1b[H");
        // Screen should be cleared and cursor at home
        assert_eq!(row_text(&emu, 0), "");
        assert_eq!(emu.cursor_position(), (0, 0));
    }

    #[test]
    fn erase_in_display_below() {
        let mut emu = TerminalEmulator::new(10, 5);
        emu.process(b"aaaaaaaaaa");
        emu.process(b"\nbbbbbbbbbb");
        emu.process(b"\ncccccccccc");
        emu.process(b"\x1b[2;3H"); // Row 2, Col 3
        emu.process(b"\x1b[0J"); // Erase below
        assert_eq!(row_text(&emu, 0), "aaaaaaaaaa");
        // Row 1 should be partially erased (from col 2 onward)
        assert_eq!(
            emu.grid()[1][0..2].iter().map(|c| c.ch).collect::<String>(),
            "bb"
        );
        // Row 2 should be cleared
        assert_eq!(row_text(&emu, 2), "");
    }

    #[test]
    fn erase_in_line_right() {
        let mut emu = TerminalEmulator::new(10, 5);
        emu.process(b"abcdefghij");
        emu.process(b"\x1b[1;4H"); // Row 1, Col 4
        emu.process(b"\x1b[0K"); // Erase right
        assert_eq!(row_text(&emu, 0), "abc");
    }

    #[test]
    fn erase_in_line_left() {
        let mut emu = TerminalEmulator::new(10, 5);
        emu.process(b"abcdefghij");
        emu.process(b"\x1b[1;4H"); // Row 1, Col 4
        emu.process(b"\x1b[1K"); // Erase left (inclusive)
        // Columns 0-3 should be erased
        assert_eq!(emu.grid()[0][0].ch, ' ');
        assert_eq!(emu.grid()[0][3].ch, ' ');
        assert_eq!(emu.grid()[0][4].ch, 'e');
    }

    #[test]
    fn erase_in_line_all() {
        let mut emu = TerminalEmulator::new(10, 5);
        emu.process(b"abcdefghij");
        emu.process(b"\x1b[2K");
        assert_eq!(row_text(&emu, 0), "");
    }

    #[test]
    fn erase_scrollback() {
        let mut emu = TerminalEmulator::new(5, 3);
        // Fill enough lines to generate scrollback
        emu.process(b"line1\nline2\nline3\nline4\nline5");
        assert!(!emu.scrollback().is_empty());
        emu.process(b"\x1b[3J");
        assert!(emu.scrollback().is_empty());
    }

    // ---- Scroll region behavior ----

    #[test]
    fn scroll_region_limits_scrolling() {
        let mut emu = TerminalEmulator::new(10, 5);
        emu.process(b"line0\nline1\nline2\nline3\nline4");
        // Set scroll region to rows 2-4 (1-indexed)
        emu.process(b"\x1b[2;4r");
        // Move cursor to bottom of scroll region and add a line
        emu.process(b"\x1b[4;1H"); // Row 4 (1-indexed), the bottom of region
        emu.process(b"\nnew");
        // Row 0 (outside region) should be unchanged
        assert_eq!(row_text(&emu, 0), "line0");
        // Row 4 (outside region, 0-indexed) should be unchanged
        assert_eq!(row_text(&emu, 4), "line4");
    }

    #[test]
    fn set_scroll_region_homes_cursor() {
        let mut emu = TerminalEmulator::new(80, 24);
        emu.process(b"\x1b[10;10H"); // Move cursor
        emu.process(b"\x1b[5;20r"); // Set scroll region
        assert_eq!(emu.cursor_position(), (0, 0));
    }

    // ---- Insert/Delete lines and characters ----

    #[test]
    fn insert_lines() {
        let mut emu = TerminalEmulator::new(10, 5);
        emu.process(b"line0\nline1\nline2\nline3\nline4");
        emu.process(b"\x1b[2;1H"); // Row 2 (1-indexed)
        emu.process(b"\x1b[1L"); // Insert 1 line
        assert_eq!(row_text(&emu, 0), "line0");
        assert_eq!(row_text(&emu, 1), ""); // Inserted blank
        assert_eq!(row_text(&emu, 2), "line1");
    }

    #[test]
    fn delete_lines() {
        let mut emu = TerminalEmulator::new(10, 5);
        emu.process(b"line0\nline1\nline2\nline3\nline4");
        emu.process(b"\x1b[2;1H"); // Row 2 (1-indexed)
        emu.process(b"\x1b[1M"); // Delete 1 line
        assert_eq!(row_text(&emu, 0), "line0");
        assert_eq!(row_text(&emu, 1), "line2");
        assert_eq!(row_text(&emu, 2), "line3");
    }

    #[test]
    fn insert_characters() {
        let mut emu = TerminalEmulator::new(10, 5);
        emu.process(b"abcdefghij");
        emu.process(b"\x1b[1;4H"); // Col 4 (1-indexed)
        emu.process(b"\x1b[2@"); // Insert 2 chars
        // 'abc' stays, 2 blanks inserted, rest shifts right (some falls off)
        assert_eq!(emu.grid()[0][0].ch, 'a');
        assert_eq!(emu.grid()[0][1].ch, 'b');
        assert_eq!(emu.grid()[0][2].ch, 'c');
        assert_eq!(emu.grid()[0][3].ch, ' ');
        assert_eq!(emu.grid()[0][4].ch, ' ');
        assert_eq!(emu.grid()[0][5].ch, 'd');
    }

    #[test]
    fn delete_characters() {
        let mut emu = TerminalEmulator::new(10, 5);
        emu.process(b"abcdefghij");
        emu.process(b"\x1b[1;3H"); // Col 3 (1-indexed)
        emu.process(b"\x1b[2P"); // Delete 2 chars
        assert_eq!(emu.grid()[0][0].ch, 'a');
        assert_eq!(emu.grid()[0][1].ch, 'b');
        assert_eq!(emu.grid()[0][2].ch, 'e');
        assert_eq!(emu.grid()[0][3].ch, 'f');
    }

    // ---- Alt screen switch and restore ----

    #[test]
    fn alt_screen_preserves_primary() {
        let mut emu = TerminalEmulator::new(10, 5);
        emu.process(b"primary");
        assert_eq!(row_text(&emu, 0), "primary");

        // Enter alt screen
        emu.process(b"\x1b[?1049h");
        assert!(emu.is_alt_screen());
        assert_eq!(row_text(&emu, 0), ""); // Alt screen is clear
        assert_eq!(emu.cursor_position(), (0, 0));

        // Write on alt screen
        emu.process(b"alternate");

        // Exit alt screen
        emu.process(b"\x1b[?1049l");
        assert!(!emu.is_alt_screen());
        assert_eq!(row_text(&emu, 0), "primary"); // Primary content restored
    }

    #[test]
    fn alt_screen_exit_without_enter_is_noop() {
        let mut emu = TerminalEmulator::new(10, 5);
        emu.process(b"content");
        emu.process(b"\x1b[?1049l"); // Exit without enter
        assert_eq!(row_text(&emu, 0), "content"); // Content unchanged
    }

    // ---- DEC private modes ----

    #[test]
    fn cursor_visibility_mode() {
        let mut emu = TerminalEmulator::new(80, 24);
        assert!(emu.cursor_visible());
        emu.process(b"\x1b[?25l"); // Hide cursor
        assert!(!emu.cursor_visible());
        emu.process(b"\x1b[?25h"); // Show cursor
        assert!(emu.cursor_visible());
    }

    #[test]
    fn auto_wrap_mode() {
        let mut emu = TerminalEmulator::new(5, 3);
        // With auto-wrap on (default)
        emu.process(b"abcdefgh");
        assert_eq!(row_text(&emu, 0), "abcde");
        assert_eq!(row_text(&emu, 1), "fgh");

        // Disable auto-wrap
        let mut emu2 = TerminalEmulator::new(5, 3);
        emu2.process(b"\x1b[?7l"); // Disable
        emu2.process(b"abcdefgh");
        // Without wrap, characters overwrite the last column
        assert_eq!(row_text(&emu2, 0), "abcdh");
        assert_eq!(row_text(&emu2, 1), "");
    }

    // ---- Cursor save/restore ----

    #[test]
    fn cursor_save_restore_csi() {
        let mut emu = TerminalEmulator::new(80, 24);
        emu.process(b"\x1b[5;10H"); // Move
        emu.process(b"\x1b[s"); // Save
        emu.process(b"\x1b[1;1H"); // Home
        emu.process(b"\x1b[u"); // Restore
        assert_eq!(emu.cursor_position(), (4, 9));
    }

    #[test]
    fn cursor_save_restore_dec() {
        let mut emu = TerminalEmulator::new(80, 24);
        emu.process(b"\x1b[5;10H"); // Move
        emu.process(b"\x1b7"); // DECSC
        emu.process(b"\x1b[1;1H"); // Home
        emu.process(b"\x1b8"); // DECRC
        assert_eq!(emu.cursor_position(), (4, 9));
    }

    // ---- Scroll commands ----

    #[test]
    fn scroll_up_command() {
        let mut emu = TerminalEmulator::new(10, 3);
        emu.process(b"line0\nline1\nline2");
        emu.process(b"\x1b[1S"); // Scroll up 1
        assert_eq!(row_text(&emu, 0), "line1");
        assert_eq!(row_text(&emu, 1), "line2");
        assert_eq!(row_text(&emu, 2), "");
        // line0 should be in scrollback
        assert_eq!(emu.scrollback().len(), 1);
    }

    #[test]
    fn scroll_down_command() {
        let mut emu = TerminalEmulator::new(10, 3);
        emu.process(b"line0\nline1\nline2");
        emu.process(b"\x1b[1T"); // Scroll down 1
        assert_eq!(row_text(&emu, 0), "");
        assert_eq!(row_text(&emu, 1), "line0");
        assert_eq!(row_text(&emu, 2), "line1");
    }

    // ---- Scrollback ----

    #[test]
    fn scrollback_accumulates_on_scroll() {
        let mut emu = TerminalEmulator::new(10, 3);
        for i in 0..10 {
            emu.process(format!("line{i}\n").as_bytes());
        }
        assert!(!emu.scrollback().is_empty());
    }

    #[test]
    fn scrollback_limit_is_enforced() {
        let mut emu = TerminalEmulator::new(10, 2);
        emu.scrollback_limit = 5;
        for i in 0..20 {
            emu.process(format!("l{i}\n").as_bytes());
        }
        assert!(emu.scrollback().len() <= 5);
    }

    // ---- Device Status Report ----

    #[test]
    fn device_status_report_cursor_position() {
        let mut emu = TerminalEmulator::new(80, 24);
        emu.process(b"\x1b[5;10H");
        emu.process(b"\x1b[6n"); // Request cursor position
        let response = emu.drain_response();
        assert_eq!(response, b"\x1b[5;10R");
    }

    // ---- Partial sequence handling across buffer boundaries ----

    #[test]
    fn partial_escape_sequence_across_buffers() {
        let mut emu = TerminalEmulator::new(80, 24);
        // Split "\x1b[31m" across two process calls
        emu.process(b"\x1b");
        emu.process(b"[31mhello");
        assert_eq!(emu.grid()[0][0].attrs.fg, Color::Indexed(1));
        assert_eq!(row_text(&emu, 0), "hello");
    }

    #[test]
    fn partial_csi_params_across_buffers() {
        let mut emu = TerminalEmulator::new(80, 24);
        // Split "\x1b[10;20H" across multiple calls
        emu.process(b"\x1b[10");
        emu.process(b";20H");
        assert_eq!(emu.cursor_position(), (9, 19));
    }

    #[test]
    fn partial_sgr_rgb_across_buffers() {
        let mut emu = TerminalEmulator::new(80, 24);
        // Split RGB color sequence
        emu.process(b"\x1b[38;2;");
        emu.process(b"100;200;50m");
        emu.process(b"X");
        assert_eq!(emu.grid()[0][0].attrs.fg, Color::Rgb(100, 200, 50));
    }

    // ---- Success criteria tests ----

    #[test]
    fn success_criterion_new_80x24() {
        let emu = TerminalEmulator::new(80, 24);
        assert_eq!(emu.cols(), 80);
        assert_eq!(emu.rows(), 24);
        assert_eq!(emu.grid().len(), 24);
        assert_eq!(emu.grid()[0].len(), 80);
    }

    #[test]
    fn success_criterion_red_foreground() {
        let mut emu = TerminalEmulator::new(80, 24);
        emu.process(b"\x1b[31mhello\x1b[0m");
        for i in 0..5 {
            assert_eq!(emu.grid()[0][i].attrs.fg, Color::Indexed(1));
        }
        assert_eq!(emu.grid()[0][0].ch, 'h');
        assert_eq!(emu.grid()[0][1].ch, 'e');
        assert_eq!(emu.grid()[0][2].ch, 'l');
        assert_eq!(emu.grid()[0][3].ch, 'l');
        assert_eq!(emu.grid()[0][4].ch, 'o');
    }

    #[test]
    fn success_criterion_clear_screen_and_home() {
        let mut emu = TerminalEmulator::new(80, 24);
        emu.process(b"some content here");
        emu.process(b"\x1b[2J\x1b[H");
        assert_eq!(emu.cursor_position(), (0, 0));
        for col in 0..80 {
            assert_eq!(emu.grid()[0][col].ch, ' ');
        }
    }

    #[test]
    fn success_criterion_alt_screen_preserves_primary() {
        let mut emu = TerminalEmulator::new(80, 24);
        emu.process(b"primary content");
        emu.process(b"\x1b[?1049h"); // Enter alt
        assert!(emu.is_alt_screen());
        emu.process(b"alt content");
        emu.process(b"\x1b[?1049l"); // Exit alt
        assert!(!emu.is_alt_screen());
        assert_eq!(row_text(&emu, 0), "primary content");
    }

    #[test]
    fn success_criterion_partial_sequences() {
        let mut emu = TerminalEmulator::new(80, 24);
        emu.process(b"\x1b[38;2;25");
        emu.process(b"5;0;128mtest");
        assert_eq!(emu.grid()[0][0].attrs.fg, Color::Rgb(255, 0, 128));
        assert_eq!(row_text(&emu, 0), "test");
    }

    // ---- Edge cases ----

    #[test]
    fn empty_input() {
        let mut emu = TerminalEmulator::new(80, 24);
        emu.process(b"");
        assert_eq!(emu.cursor_position(), (0, 0));
    }

    #[test]
    fn unknown_csi_sequence_is_discarded() {
        let mut emu = TerminalEmulator::new(80, 24);
        emu.process(b"\x1b[999zHello");
        assert_eq!(row_text(&emu, 0), "Hello");
    }

    #[test]
    fn unknown_escape_sequence_is_discarded() {
        let mut emu = TerminalEmulator::new(80, 24);
        emu.process(b"\x1b?Hello");
        assert_eq!(row_text(&emu, 0), "Hello");
    }

    #[test]
    fn cursor_stays_in_bounds() {
        let mut emu = TerminalEmulator::new(5, 3);
        emu.process(b"\x1b[100;100H");
        assert_eq!(emu.cursor_position(), (2, 4));
        emu.process(b"\x1b[100A"); // Up 100
        assert_eq!(emu.cursor_position(), (0, 4));
    }

    #[test]
    fn sgr_combined_with_color() {
        let mut emu = TerminalEmulator::new(80, 24);
        emu.process(b"\x1b[1;31;42mX");
        let attrs = &emu.grid()[0][0].attrs;
        assert!(attrs.bold);
        assert_eq!(attrs.fg, Color::Indexed(1)); // red
        assert_eq!(attrs.bg, Color::Indexed(2)); // green
    }

    #[test]
    fn osc_sequence_consumed_silently() {
        let mut emu = TerminalEmulator::new(80, 24);
        emu.process(b"\x1b]0;window title\x07Hello");
        assert_eq!(row_text(&emu, 0), "Hello");
    }

    #[test]
    fn osc_with_st_consumed_silently() {
        let mut emu = TerminalEmulator::new(80, 24);
        emu.process(b"\x1b]0;title\x1b\\Hello");
        assert_eq!(row_text(&emu, 0), "Hello");
    }

    #[test]
    fn reverse_index_at_top_scrolls_down() {
        let mut emu = TerminalEmulator::new(10, 3);
        emu.process(b"line0\nline1\nline2");
        emu.process(b"\x1b[H"); // Home
        emu.process(b"\x1bM"); // Reverse index
        assert_eq!(row_text(&emu, 0), "");
        assert_eq!(row_text(&emu, 1), "line0");
        assert_eq!(row_text(&emu, 2), "line1");
    }

    #[test]
    fn erase_characters_command() {
        let mut emu = TerminalEmulator::new(10, 3);
        emu.process(b"abcdefghij");
        emu.process(b"\x1b[1;3H"); // Col 3
        emu.process(b"\x1b[3X"); // Erase 3 chars
        assert_eq!(emu.grid()[0][0].ch, 'a');
        assert_eq!(emu.grid()[0][1].ch, 'b');
        assert_eq!(emu.grid()[0][2].ch, ' ');
        assert_eq!(emu.grid()[0][3].ch, ' ');
        assert_eq!(emu.grid()[0][4].ch, ' ');
        assert_eq!(emu.grid()[0][5].ch, 'f');
    }

    #[test]
    fn scrollback_not_added_for_scroll_region() {
        let mut emu = TerminalEmulator::new(10, 5);
        emu.process(b"line0\nline1\nline2\nline3\nline4");
        emu.process(b"\x1b[2;4r"); // Scroll region rows 2-4
        let sb_before = emu.scrollback().len();
        emu.process(b"\x1b[4;1H\n"); // Scroll within region
        // Scrollback should not grow for region-internal scrolls
        assert_eq!(emu.scrollback().len(), sb_before);
    }
}
