//! Devil IDE desktop application entry point.

use std::env;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use anyhow::Result;

use devil_editor::EditorSession;
use devil_platform::{
    FileSystemService, NativeFileSystem, PathNormalizationService, PlatformError, shell_title,
};
use devil_protocol::{FileId, ProjectId, ProjectInfo};
use devil_ui::Shell;

#[derive(Debug)]
struct WorkspaceVfsComposition {
    root: PathBuf,
    fs: NativeFileSystem,
}

impl WorkspaceVfsComposition {
    fn new(root: impl Into<PathBuf>) -> Self {
        Self {
            root: root.into(),
            fs: NativeFileSystem,
        }
    }

    fn normalize_path_or_fallback(&self, path: &str) -> PathBuf {
        self.fs
            .normalize_path(Path::new(path))
            .unwrap_or_else(|_| Path::new(path).to_path_buf())
    }

    fn resolve_project_for_file(&self, path: &Path) -> ProjectInfo {
        let _ = path;

        ProjectInfo {
            project_id: ProjectId(1),
            root_path: self.root.to_string_lossy().into_owned(),
            file_id: FileId(1),
            language_id: None,
        }
    }

    fn read_file_text(&self, path: &Path) -> Result<String, PlatformError> {
        self.fs.read_text_file(path)
    }

    fn write_file_text(&self, path: &Path, text: &str) -> Result<(), PlatformError> {
        self.fs.write_text_file(path, text)
    }
}

fn main() -> Result<()> {
    let path = env::args()
        .nth(1)
        .unwrap_or_else(|| "scratch.txt".to_string());

    let workspace_root = env::current_dir()?;
    let vfs = WorkspaceVfsComposition::new(workspace_root);
    let normalized_path = vfs.normalize_path_or_fallback(&path);
    let initial_text = vfs
        .read_file_text(&normalized_path)
        .unwrap_or_else(|_| String::new());

    let project_info = vfs.resolve_project_for_file(&normalized_path);
    let editor = EditorSession::open(
        normalized_path.to_string_lossy(),
        project_info,
        initial_text,
    );
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
                let save_request = shell.editor.request_save()?;
                vfs.write_file_text(Path::new(shell.editor.file_path()), &save_request.text)?;
                println!(
                    "Saved {} (snapshot={}, hash={})",
                    shell.editor.file_path(),
                    save_request.snapshot_id.0,
                    save_request.content_hash
                );
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn workspace_vfs_roundtrips_file_content() {
        let workspace_root = std::env::temp_dir();
        let vfs = WorkspaceVfsComposition::new(&workspace_root);
        let target = workspace_root.join(format!("devil-app-roundtrip-{}.txt", std::process::id()));

        let initial = "alpha\n";
        let _ = std::fs::write(&target, initial);

        let loaded = vfs
            .read_file_text(&target)
            .expect("composition should read via platform fs service");
        assert_eq!(loaded, initial);

        let updated = "beta\n";
        vfs.write_file_text(&target, updated)
            .expect("composition should write via platform fs service");

        let on_disk =
            std::fs::read_to_string(&target).expect("writing via native fs should persist");
        assert_eq!(on_disk, updated);

        let _ = std::fs::remove_file(&target);
    }
}
