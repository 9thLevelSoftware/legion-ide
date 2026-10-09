//! Shared traversal policy for the delegated grep and glob tools.

use std::path::Path;

use super::{DelegatedTaskLoopConfig, worktree_relative_to_workspace_path};

/// Visit regular files depth-first, omitting hidden directories, forbidden
/// entries, and links. The caller retains matching and per-file result limits;
/// traversal stops before further filesystem work once the result limit is met.
pub(super) fn walk_files(
    root: &Path,
    config: &DelegatedTaskLoopConfig,
    results: &mut Vec<String>,
    limit: usize,
    mut visit: impl FnMut(&Path, &Path, &mut Vec<String>) -> std::io::Result<()>,
) -> std::io::Result<()> {
    walk_directory(root, root, config, results, limit, &mut visit)
}

fn walk_directory<F>(
    base: &Path,
    dir: &Path,
    config: &DelegatedTaskLoopConfig,
    results: &mut Vec<String>,
    limit: usize,
    visit: &mut F,
) -> std::io::Result<()>
where
    F: FnMut(&Path, &Path, &mut Vec<String>) -> std::io::Result<()>,
{
    if results.len() >= limit {
        return Ok(());
    }
    let entries = std::fs::read_dir(dir)?;
    for entry in entries {
        if results.len() >= limit {
            return Ok(());
        }
        let entry = entry?;
        let path = entry.path();
        if worktree_path_is_forbidden(config, &path) {
            continue;
        }
        // DirEntry classification does not follow symlinks or junctions. Do not
        // replace this with Path::is_dir/is_file, which follow their targets.
        let file_type = entry.file_type()?;
        if file_type.is_dir() {
            let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if name.starts_with('.') {
                continue;
            }
            walk_directory(base, &path, config, results, limit, visit)?;
        } else if file_type.is_file() {
            let relative = path.strip_prefix(base).unwrap_or(&path);
            visit(&path, relative, results)?;
        }
    }
    Ok(())
}

/// Apply both forbidden-path sources to every entry, mapped back to workspace
/// paths. Search roots have already been resolved by the containment gate.
fn worktree_path_is_forbidden(config: &DelegatedTaskLoopConfig, path: &Path) -> bool {
    let Ok(relative) = path.strip_prefix(&config.worktree_root) else {
        return true;
    };
    let candidate = worktree_relative_to_workspace_path(relative, &config.workspace_root);
    let canonical = legion_protocol::CanonicalPath(candidate.to_string_lossy().into_owned());

    config.scope.forbids_path(&canonical)
        || config.forbidden_paths.iter().any(|forbidden| {
            let forbidden = Path::new(forbidden);
            let forbidden = if forbidden.is_absolute() {
                forbidden.to_path_buf()
            } else {
                config.workspace_root.join(forbidden)
            };
            candidate == forbidden || candidate.starts_with(forbidden)
        })
}
