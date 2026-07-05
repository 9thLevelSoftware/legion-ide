use std::{
    fs,
    path::{Component, Path, PathBuf},
};

use serde::Deserialize;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NoEguiTextEditViolationKind {
    ForbiddenToken,
    UnreadableFile,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NoEguiTextEditViolation {
    pub path: PathBuf,
    pub line: usize,
    pub kind: NoEguiTextEditViolationKind,
    pub message: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct NoEguiTextEditConfig {
    pub scanned_paths: Vec<String>,
    pub allowlisted_paths: Vec<String>,
    pub forbidden_tokens: Vec<String>,
}

impl Default for NoEguiTextEditConfig {
    fn default() -> Self {
        Self {
            scanned_paths: vec![
                "crates/legion-desktop/src/view.rs".to_string(),
                "crates/legion-desktop/src/view/code_canvas_painter.rs".to_string(),
            ],
            allowlisted_paths: Vec::new(),
            forbidden_tokens: vec![
                "egui::TextEdit".to_string(),
                "eframe::egui::TextEdit".to_string(),
            ],
        }
    }
}

impl NoEguiTextEditConfig {
    pub fn from_file(path: &Path) -> Result<Self, String> {
        let text = fs::read_to_string(path).map_err(|err| {
            format!(
                "unable to read no-egui-textedit config `{}`: {err}",
                path.display()
            )
        })?;
        toml::from_str(&text).map_err(|err| {
            format!(
                "unable to parse no-egui-textedit config `{}`: {err}",
                path.display()
            )
        })
    }
}

pub fn run_no_egui_textedit(
    workspace_root: &Path,
    config: &NoEguiTextEditConfig,
) -> Result<(), Vec<NoEguiTextEditViolation>> {
    let mut violations = Vec::new();
    let rust_files = collect_rust_files(workspace_root);

    for path in rust_files {
        if !is_scanned(workspace_root, &path, config)
            || is_allowlisted(workspace_root, &path, config)
        {
            continue;
        }
        let text = match fs::read_to_string(&path) {
            Ok(text) => text,
            Err(err) => {
                // Fail closed: a scanned (non-allowlisted) file we cannot
                // read is a violation, not a silent skip, so the gate exits
                // non-zero instead of failing open.
                violations.push(NoEguiTextEditViolation {
                    path: path
                        .strip_prefix(workspace_root)
                        .unwrap_or(&path)
                        .to_path_buf(),
                    line: 0,
                    kind: NoEguiTextEditViolationKind::UnreadableFile,
                    message: format!("unable to read scanned file: {err}"),
                });
                continue;
            }
        };
        check_forbidden_tokens(workspace_root, &path, &text, config, &mut violations);
    }

    if violations.is_empty() {
        Ok(())
    } else {
        Err(violations)
    }
}

fn collect_rust_files(root: &Path) -> Vec<PathBuf> {
    if let Some(files) = git_rust_files(root) {
        let mut files: Vec<PathBuf> = files.into_iter().map(|rel| root.join(rel)).collect();
        files.sort();
        files.dedup();
        return files;
    }
    let mut files = Vec::new();
    collect_rust_files_recursive(root, root, &mut files);
    files.sort();
    files
}

fn git_rust_files(root: &Path) -> Option<Vec<String>> {
    let mut files = git_ls_files(root, &["ls-files", "-z", "--", "*.rs"])?;
    files.extend(git_ls_files(
        root,
        &[
            "ls-files",
            "-z",
            "--others",
            "--exclude-standard",
            "--",
            "*.rs",
        ],
    )?);
    files.retain(|rel| !should_skip_repo_relative_path(rel));
    files.sort();
    files.dedup();
    Some(files)
}

fn git_ls_files(root: &Path, args: &[&str]) -> Option<Vec<String>> {
    let output = std::process::Command::new("git")
        .arg("-C")
        .arg(root)
        .args(args)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let raw = String::from_utf8(output.stdout).ok()?;
    Some(
        raw.split('\0')
            .filter(|entry| !entry.is_empty())
            .map(|entry| entry.to_string())
            .collect(),
    )
}

fn should_skip_repo_relative_path(rel: &str) -> bool {
    rel == ".git"
        || rel == "target"
        || rel == ".almanac"
        || rel == ".hermes"
        || rel == ".omh"
        || rel == ".serena"
        || rel.starts_with(".git/")
        || rel.starts_with("target/")
        || rel.starts_with(".almanac/")
        || rel.starts_with(".hermes/")
        || rel.starts_with(".omh/")
        || rel.starts_with(".serena/")
}

fn collect_rust_files_recursive(root: &Path, current: &Path, files: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(current) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if should_skip_dir(root, &path) {
            continue;
        }
        if path.is_dir() {
            collect_rust_files_recursive(root, &path, files);
        } else if path.extension().is_some_and(|ext| ext == "rs") {
            files.push(path);
        }
    }
}

fn should_skip_dir(root: &Path, path: &Path) -> bool {
    if !path.is_dir() {
        return false;
    }
    let rel = path.strip_prefix(root).unwrap_or(path);
    matches!(
        rel.components().next(),
        Some(Component::Normal(name))
            if name == ".git"
                || name == "target"
                || name == ".almanac"
                || name == ".hermes"
                || name == ".omh"
                || name == ".serena"
    )
}

fn check_forbidden_tokens(
    root: &Path,
    file: &Path,
    text: &str,
    config: &NoEguiTextEditConfig,
    violations: &mut Vec<NoEguiTextEditViolation>,
) {
    for (line_index, line) in text.lines().enumerate() {
        let line_number = line_index + 1;
        for token in &config.forbidden_tokens {
            if contains_token(line, token) {
                violations.push(NoEguiTextEditViolation {
                    path: file.strip_prefix(root).unwrap_or(file).to_path_buf(),
                    line: line_number,
                    kind: NoEguiTextEditViolationKind::ForbiddenToken,
                    message: format!(
                        "forbidden renderer token `{token}` in code-canvas path; use the custom CodeCanvasPainter projection boundary instead of egui::TextEdit"
                    ),
                });
            }
        }
    }
}

fn contains_token(line: &str, token: &str) -> bool {
    let mut search_start = 0;
    while let Some(offset) = line[search_start..].find(token) {
        let start = search_start + offset;
        let end = start + token.len();
        let before_ok = line[..start]
            .chars()
            .next_back()
            .is_none_or(|ch| !is_token_char(ch));
        let after_ok = line[end..]
            .chars()
            .next()
            .is_none_or(|ch| !is_token_char(ch));
        if before_ok && after_ok {
            return true;
        }
        search_start = end;
    }
    false
}

fn is_token_char(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || ch == '_'
}

fn is_scanned(root: &Path, file: &Path, config: &NoEguiTextEditConfig) -> bool {
    let rel = repo_relative_path(file.strip_prefix(root).unwrap_or(file));
    config.scanned_paths.iter().any(|prefix| {
        let prefix = normalize_prefix(prefix);
        path_has_prefix(&rel, &prefix)
    })
}

fn is_allowlisted(root: &Path, file: &Path, config: &NoEguiTextEditConfig) -> bool {
    let rel = repo_relative_path(file.strip_prefix(root).unwrap_or(file));
    config.allowlisted_paths.iter().any(|prefix| {
        let prefix = normalize_prefix(prefix);
        path_has_prefix(&rel, &prefix)
    })
}

/// Match `rel` against `prefix` on path-segment boundaries so a configured
/// prefix `crates/foo` matches `crates/foo` and `crates/foo/bar.rs` but NOT
/// a sibling such as `crates/foo-bar.rs` that merely shares the byte prefix.
fn path_has_prefix(rel: &str, prefix: &str) -> bool {
    let prefix = prefix.trim_end_matches('/');
    if prefix.is_empty() {
        return false;
    }
    rel == prefix || rel.starts_with(&format!("{prefix}/"))
}

fn normalize_prefix(prefix: &str) -> String {
    prefix.trim().trim_start_matches("./").replace('\\', "/")
}

fn repo_relative_path(path: &Path) -> String {
    path.components()
        .map(|component| component.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/")
}
