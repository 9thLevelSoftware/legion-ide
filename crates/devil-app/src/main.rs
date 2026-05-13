//! Devil IDE desktop application entry point.

use std::env;
use std::io::{self, Write};

use anyhow::Result;

use devil_editor::EditorSession;
use devil_platform::{open_text_file, save_text_file, shell_title};
use devil_protocol::{FileId, ProjectId, ProjectInfo};
use devil_ui::Shell;

fn main() -> Result<()> {
    let path = env::args()
        .nth(1)
        .unwrap_or_else(|| "scratch.txt".to_string());
    let initial_text = match open_text_file(&path) {
        Ok(text) => text,
        Err(_) => String::new(),
    };

    let project_info = ProjectInfo {
        project_id: ProjectId(1),
        root_path: String::from("."),
        file_id: FileId(1),
        language_id: None,
    };
    let editor = EditorSession::open(&path, project_info, initial_text);
    let mut shell = Shell::new(shell_title(), editor);

    let mut input = String::new();

    loop {
        shell.render();
        print!("> ");
        io::stdout().flush()?;

        input.clear();
        if io::stdin().read_line(&mut input)? == 0 {
            break;
        }

        let command = input.trim_end();

        match command {
            ":q" => break,
            ":w" => {
                save_text_file(shell.editor.file_path(), shell.editor.text())?;
                println!("Saved {}", shell.editor.file_path());
            }
            other => {
                let keep_running = shell.handle_command(other)?;
                if !keep_running {
                    break;
                }
            }
        }
    }

    Ok(())
}
