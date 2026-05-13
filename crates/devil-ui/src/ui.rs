//! Basic text-editor UI primitives for a native shell spike.

use devil_editor::{EditorError, EditorSession, TextPosition, TextRange};

/// Render mode for the spike shell.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RenderMode {
    /// Basic read/write listing.
    Plain,
}

/// Minimal layout model used by the spike.
#[derive(Debug)]
pub struct Layout {
    /// Window title for the shell.
    pub title: String,
    /// Width of the frame.
    pub width: u16,
    /// Height of the frame.
    pub height: u16,
}

impl Layout {
    /// Construct a layout.
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            width: 80,
            height: 24,
        }
    }
}

/// Minimal IDE surface for spike verification.
#[derive(Debug)]
pub struct Shell {
    /// Top-level layout.
    pub layout: Layout,
    /// Backing editor session.
    pub editor: EditorSession,
    /// Current mode.
    pub mode: RenderMode,
}

impl Shell {
    /// Create a shell surface for a file.
    pub fn new(title: impl Into<String>, editor: EditorSession) -> Self {
        Self {
            layout: Layout::new(title),
            editor,
            mode: RenderMode::Plain,
        }
    }

    /// Render basic status and file content.
    pub fn render(&self) {
        print!("\x1b[2J\x1b[H");
        println!("{}", self.layout.title);
        println!(
            "Mode: {:?} | {}x{}",
            self.mode, self.layout.width, self.layout.height
        );
        println!("{}", "-".repeat(self.layout.width as usize));
        println!("{}", self.editor.text());
        println!("{}", "-".repeat(self.layout.width as usize));
        println!("Path: {}", self.editor.file_path());
        println!("Commands: :i text | :d start,end | :r start,end,text | :u | :r | :q");
    }

    /// Parse and apply a command in tiny interactive mode.
    pub fn handle_command(&mut self, input: &str) -> Result<bool, EditorError> {
        let trimmed = input.trim();
        if trimmed == ":q" {
            return Ok(false);
        }
        if trimmed == ":u" {
            let _ = self.editor.undo();
            return Ok(true);
        }
        if trimmed == ":redo" {
            let _ = self.editor.redo();
            return Ok(true);
        }

        if let Some(payload) = trimmed.strip_prefix(":i ") {
            let pos = TextPosition::new(0, 0);
            self.editor.insert_at(pos, payload)?;
            return Ok(true);
        }

        if let Some(payload) = trimmed.strip_prefix(":d ") {
            let mut split = payload.split(',');
            let start = split.next().unwrap_or("0").parse::<usize>().unwrap_or(0);
            let end = split.next().unwrap_or("0").parse::<usize>().unwrap_or(0);
            let start = parse_pos(&self.editor, start);
            let end = parse_pos(&self.editor, end);
            self.editor.delete_range(TextRange::new(start, end))?;
            return Ok(true);
        }

        if let Some(payload) = trimmed.strip_prefix(":r ") {
            let mut split = payload.splitn(3, ',');
            let start = split.next().unwrap_or("0").parse::<usize>().unwrap_or(0);
            let end = split.next().unwrap_or("0").parse::<usize>().unwrap_or(0);
            let replacement = split.next().unwrap_or("");
            let start = parse_pos(&self.editor, start);
            let end = parse_pos(&self.editor, end);
            self.editor
                .replace_range(TextRange::new(start, end), replacement)?;
            return Ok(true);
        }

        Ok(true)
    }
}

fn parse_pos(session: &EditorSession, byte_offset: usize) -> TextPosition {
    session
        .snapshot()
        .text()
        .as_bytes()
        .get(..byte_offset)
        .map(|prefix| {
            let line = prefix.iter().filter(|b| **b == b'\n').count();
            let column = prefix.iter().rev().take_while(|b| **b != b'\n').count();
            TextPosition::new(line, column)
        })
        .unwrap_or_else(|| TextPosition::new(0, 0))
}

#[cfg(test)]
mod tests {
    use super::*;
    use devil_editor::EditorSession;
    use devil_protocol::{FileId, ProjectId, ProjectInfo};

    #[test]
    fn shell_handles_commands() {
        let project = ProjectInfo {
            project_id: ProjectId(1),
            root_path: "r".into(),
            file_id: FileId(9),
            language_id: None,
        };
        let editor = EditorSession::open("a.md", project, "first");
        let mut shell = Shell::new("t", editor);
        shell
            .handle_command(":i \\n")
            .expect("insert command should succeed");
        assert!(!shell.editor.text().is_empty());
    }
}
