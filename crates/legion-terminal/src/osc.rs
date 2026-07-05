//! OSC 7/133 shell metadata parsing for terminal runtime projections.

/// Structured terminal shell projection produced from OSC-marked output.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TerminalShellProjection {
    /// Visible output with OSC metadata stripped.
    pub visible_output: String,
    /// Latest OSC 7 cwd metadata, if present.
    pub cwd: Option<String>,
    /// Latest OSC 133 exit code metadata, if present.
    pub exit_code: Option<i32>,
    /// Latest OSC 133 boundary marker, if present.
    pub boundary: Option<TerminalShellBoundary>,
}

/// OSC 133 command boundary markers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerminalShellBoundary {
    /// Prompt start marker.
    PromptStart,
    /// Command start marker.
    CommandStart,
    /// Command output marker.
    CommandOutput,
    /// Command finished marker.
    CommandFinished,
}

/// Parse OSC 7/133 metadata out of raw shell output.
///
/// Shell-emitted OSC metadata is advisory UI metadata only. Security policy must
/// continue to use workspace/app authority, not shell-reported cwd values.
pub fn parse_terminal_shell_output(payload: &str) -> TerminalShellProjection {
    let mut visible_output = String::new();
    let mut cwd = None;
    let mut exit_code = None;
    let mut boundary = None;
    let bytes = payload.as_bytes();
    let mut cursor = 0;

    while cursor < bytes.len() {
        if bytes[cursor] == 0x1b && cursor + 1 < bytes.len() && bytes[cursor + 1] == b']' {
            let osc_start = cursor;
            cursor += 2;
            let seq_start = cursor;
            let mut terminated = false;
            while cursor < bytes.len() {
                if bytes[cursor] == 0x07 {
                    terminated = true;
                    break;
                }
                if bytes[cursor] == 0x1b && cursor + 1 < bytes.len() && bytes[cursor + 1] == b'\\' {
                    terminated = true;
                    break;
                }
                cursor += 1;
            }
            if !terminated {
                // Split/unterminated OSC sequences must not silently drop the
                // tail of visible terminal output. Keep bytes verbatim so a
                // later poll can be diagnosed rather than losing metadata.
                visible_output.push_str(&payload[osc_start..]);
                break;
            }
            let sequence = &payload[seq_start..cursor];
            if let Some(parsed_cwd) = terminal_shell_cwd_from_osc(sequence) {
                cwd = Some(parsed_cwd);
            }
            if let Some(parsed_boundary) = terminal_shell_boundary_from_osc(sequence) {
                boundary = Some(parsed_boundary);
            }
            if let Some(parsed_exit_code) = terminal_shell_exit_code_from_osc(sequence) {
                exit_code = Some(parsed_exit_code);
            }
            if cursor < bytes.len()
                && bytes[cursor] == 0x1b
                && cursor + 1 < bytes.len()
                && bytes[cursor + 1] == b'\\'
            {
                cursor += 2;
            } else if cursor < bytes.len() && bytes[cursor] == 0x07 {
                cursor += 1;
            }
            continue;
        }

        let Some(ch) = payload[cursor..].chars().next() else {
            break;
        };
        visible_output.push(ch);
        cursor += ch.len_utf8();
    }

    TerminalShellProjection {
        visible_output,
        cwd,
        exit_code,
        boundary,
    }
}

/// Split visible terminal output into display rows.
///
/// A PTY read chunk is a transport unit, not a display row: one chunk may
/// carry many lines, a shell's `\r`-redraw sequence for a single line, or
/// cursor-repositioning escapes standing in for line breaks (cmd.exe under
/// ConPTY). Projections that store one row per *chunk* silently hide
/// everything past a per-row payload cap — observed on macOS CI where a
/// single bash 3.2 chunk carried banner + prompt + command output + the
/// GP-1 exit marker, and the cap cut the marker (legion-smoke runs
/// 28741840232 / 28747873556).
///
/// Semantics (screen-visible content wins):
/// - `\n` ends a row; `\r\n` is one line ending.
/// - A bare `\r` is a redraw: it discards the drawn-over content of the
///   current row (progress bars, bash horizontal-scroll echo).
/// - CSI cursor-position sequences (`ESC[…H` / `ESC[…f`) end the current
///   row and are dropped; all other escape sequences pass through
///   untouched.
/// - Empty and whitespace-only rows are dropped.
pub fn split_visible_rows(visible_output: &str) -> Vec<String> {
    fn flush(rows: &mut Vec<String>, current: &mut String) {
        if current.trim().is_empty() {
            current.clear();
        } else {
            rows.push(std::mem::take(current));
        }
    }

    let mut rows = Vec::new();
    let mut current = String::new();
    let mut chars = visible_output.chars().peekable();
    while let Some(ch) = chars.next() {
        match ch {
            '\n' => flush(&mut rows, &mut current),
            '\r' => {
                if chars.peek() == Some(&'\n') {
                    chars.next();
                    flush(&mut rows, &mut current);
                } else {
                    // Redraw: the terminal would overwrite this row.
                    current.clear();
                }
            }
            '\u{1b}' if chars.peek() == Some(&'[') => {
                chars.next();
                let mut sequence = String::from("\u{1b}[");
                let mut final_byte = None;
                for c in chars.by_ref() {
                    sequence.push(c);
                    if !(c.is_ascii_digit() || matches!(c, ';' | ':' | '?')) {
                        final_byte = Some(c);
                        break;
                    }
                }
                match final_byte {
                    // Cursor reposition acts as a row boundary.
                    Some('H') | Some('f') => flush(&mut rows, &mut current),
                    // Any other CSI (colors, erase, cursor visibility)
                    // passes through, preserving prior payload behavior.
                    _ => current.push_str(&sequence),
                }
            }
            _ => current.push(ch),
        }
    }
    flush(&mut rows, &mut current);
    rows
}

fn terminal_shell_cwd_from_osc(sequence: &str) -> Option<String> {
    let value = sequence.strip_prefix("7;")?;
    let value = value.strip_prefix("file://")?;

    // OSC 7 carries `file://HOST/PATH`. `file:///PATH` yields an empty host.
    let (host, raw_path) = value.split_once('/')?;
    let host = percent_decode(host);
    let host = if host.eq_ignore_ascii_case("localhost") {
        String::new()
    } else {
        host
    };

    // Percent-decode each path segment independently so encoded separators are
    // not misinterpreted as path boundaries.
    let decoded_path = raw_path
        .split('/')
        .map(percent_decode)
        .collect::<Vec<_>>()
        .join("/");

    // Windows drive-letter path: file:///C:/Users -> C:/Users
    if host.is_empty() && is_windows_drive_prefix(&decoded_path) {
        return Some(decoded_path);
    }

    // UNC path: file://server/share/... -> //server/share/...
    if !host.is_empty() {
        return Some(format!("//{host}/{}", decoded_path.trim_start_matches('/')));
    }

    Some(format!("/{}", decoded_path.trim_start_matches('/')))
}

/// Percent-decode a URL component, leaving invalid escapes untouched.
fn percent_decode(input: &str) -> String {
    let bytes = input.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' && index + 2 < bytes.len() {
            let high = (bytes[index + 1] as char).to_digit(16);
            let low = (bytes[index + 2] as char).to_digit(16);
            if let (Some(high), Some(low)) = (high, low) {
                decoded.push((high * 16 + low) as u8);
                index += 3;
                continue;
            }
        }
        decoded.push(bytes[index]);
        index += 1;
    }
    String::from_utf8_lossy(&decoded).into_owned()
}

/// Returns true when `path` begins with a Windows drive prefix such as `C:`.
fn is_windows_drive_prefix(path: &str) -> bool {
    let mut chars = path.chars();
    matches!(
        (chars.next(), chars.next()),
        (Some(letter), Some(':')) if letter.is_ascii_alphabetic()
    )
}

fn terminal_shell_exit_code_from_osc(sequence: &str) -> Option<i32> {
    let value = sequence.strip_prefix("133;")?;
    let (command, parameters) = value.split_once(';')?;
    if command != "D" {
        return None;
    }
    parameters.split(';').next()?.parse().ok()
}

fn terminal_shell_boundary_from_osc(sequence: &str) -> Option<TerminalShellBoundary> {
    let value = sequence.strip_prefix("133;")?;
    let marker = value
        .split_once(';')
        .map(|(marker, _)| marker)
        .unwrap_or(value);
    match marker {
        "A" => Some(TerminalShellBoundary::PromptStart),
        "B" => Some(TerminalShellBoundary::CommandStart),
        "C" => Some(TerminalShellBoundary::CommandOutput),
        "D" => Some(TerminalShellBoundary::CommandFinished),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::{TerminalShellBoundary, parse_terminal_shell_output, split_visible_rows};

    #[test]
    fn split_visible_rows_breaks_on_line_endings() {
        assert_eq!(
            split_visible_rows("alpha\r\nbeta\ngamma"),
            vec!["alpha", "beta", "gamma"]
        );
    }

    #[test]
    fn split_visible_rows_bare_carriage_return_is_a_redraw() {
        assert_eq!(
            split_visible_rows(
                "Compiling 1/10\rCompiling 5/10\rCompiling 10/10 done\r\nfinished\r\n"
            ),
            vec!["Compiling 10/10 done", "finished"]
        );
    }

    #[test]
    fn split_visible_rows_cursor_position_sequences_break_rows_and_are_dropped() {
        // cmd.exe under ConPTY repositions the cursor instead of emitting
        // newlines between logical lines.
        assert_eq!(
            split_visible_rows("test result: ok\u{1b}[9;1HSMOKE_EXIT:0\u{1b}[11;1Hprompt>"),
            vec!["test result: ok", "SMOKE_EXIT:0", "prompt>"]
        );
    }

    #[test]
    fn split_visible_rows_preserves_non_cursor_escape_sequences() {
        assert_eq!(
            split_visible_rows("\u{1b}[38;5;2mok\u{1b}[m\r\n"),
            vec!["\u{1b}[38;5;2mok\u{1b}[m"]
        );
    }

    #[test]
    fn split_visible_rows_keeps_trailing_prompt_without_newline() {
        assert_eq!(
            split_visible_rows("done\r\nbash-3.2$ "),
            vec!["done", "bash-3.2$ "]
        );
    }

    #[test]
    fn split_visible_rows_drops_blank_rows() {
        assert_eq!(
            split_visible_rows("\r\n\r\n  \r\nvalue\r\n\r\n"),
            vec!["value"]
        );
    }

    #[test]
    fn parse_terminal_shell_output_strips_osc_markers_and_tracks_metadata() {
        let parsed = parse_terminal_shell_output(
            "\u{1b}]7;file://localhost/tmp/workspace\u{1b}\\prompt\u{1b}]133;D;7\u{1b}\\",
        );

        assert_eq!(parsed.visible_output, "prompt");
        assert_eq!(parsed.cwd.as_deref(), Some("/tmp/workspace"));
        assert_eq!(parsed.exit_code, Some(7));
        assert_eq!(
            parsed.boundary,
            Some(TerminalShellBoundary::CommandFinished)
        );
    }
}
