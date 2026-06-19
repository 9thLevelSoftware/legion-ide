use std::{
    fs,
    path::{Component, Path, PathBuf},
};

use serde::Deserialize;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DocsHygieneViolationKind {
    BrokenRelativeLink,
    StaleDevilReference,
    StaleProductionPlanReference,
}

const LATEST_PRODUCTION_MASTER_PLAN: &str = "legion-production-master-plan-v0.2.md";
const PRODUCTION_PLAN_ENTRYPOINTS: [&str; 2] = ["README.md", "docs/INDEX.md"];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocsHygieneViolation {
    pub path: PathBuf,
    pub line: usize,
    pub kind: DocsHygieneViolationKind,
    pub message: String,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct DocsHygieneConfig {
    pub allowlisted_paths: Vec<String>,
}

impl DocsHygieneConfig {
    pub fn from_file(path: &Path) -> Result<Self, String> {
        let text = fs::read_to_string(path).map_err(|err| {
            format!(
                "unable to read docs hygiene allowlist `{}`: {err}",
                path.display()
            )
        })?;
        toml::from_str(&text).map_err(|err| {
            format!(
                "unable to parse docs hygiene allowlist `{}`: {err}",
                path.display()
            )
        })
    }
}

pub fn normalize_relative_target(raw: &str) -> Option<String> {
    let trimmed = raw.trim().trim_matches('<').trim_matches('>');
    if trimmed.is_empty() || trimmed.starts_with('#') || looks_external(trimmed) {
        return None;
    }

    let without_anchor = trimmed.split('#').next().unwrap_or(trimmed);
    let without_line = strip_line_suffix(without_anchor);
    if without_line.is_empty() {
        None
    } else {
        Some(without_line.to_string())
    }
}

fn looks_external(target: &str) -> bool {
    target.contains("://")
        || target.starts_with("mailto:")
        || target.starts_with("tel:")
        || target.starts_with("data:")
}

fn strip_line_suffix(target: &str) -> &str {
    let Some((prefix, suffix)) = target.rsplit_once(':') else {
        return target;
    };
    let is_line_suffix = suffix
        .split('-')
        .all(|part| !part.is_empty() && part.chars().all(|ch| ch.is_ascii_digit()));
    if is_line_suffix && prefix.contains('.') {
        prefix
    } else {
        target
    }
}

pub fn run_docs_hygiene(
    workspace_root: &Path,
    config: &DocsHygieneConfig,
) -> Result<(), Vec<DocsHygieneViolation>> {
    let mut violations = Vec::new();
    let markdown_files = collect_markdown_files(workspace_root);

    for path in markdown_files {
        if is_allowlisted(workspace_root, &path, config) {
            continue;
        }
        let Ok(text) = fs::read_to_string(&path) else {
            continue;
        };
        check_markdown_links(workspace_root, &path, &text, &mut violations);
        check_stale_devil_references(workspace_root, &path, &text, &mut violations);
        check_current_production_plan_reference(workspace_root, &path, &text, &mut violations);
    }

    if violations.is_empty() {
        Ok(())
    } else {
        Err(violations)
    }
}

fn collect_markdown_files(root: &Path) -> Vec<PathBuf> {
    if let Some(files) = git_markdown_files(root) {
        let mut files: Vec<PathBuf> = files.into_iter().map(|rel| root.join(rel)).collect();
        files.sort();
        files.dedup();
        return files;
    }
    let mut files = Vec::new();
    collect_markdown_files_recursive(root, root, &mut files);
    files.sort();
    files
}

fn git_markdown_files(root: &Path) -> Option<Vec<String>> {
    let mut files = git_ls_files(root, &["ls-files", "-z", "--", "*.md"])?;
    files.extend(git_ls_files(
        root,
        &[
            "ls-files",
            "-z",
            "--others",
            "--exclude-standard",
            "--",
            "*.md",
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
    let files = raw
        .split('\0')
        .filter(|entry| !entry.is_empty())
        .map(|entry| entry.to_string())
        .collect::<Vec<_>>();
    // Returned paths are repo-relative; callers re-root them against `root`.
    Some(files)
}

fn should_skip_repo_relative_path(rel: &str) -> bool {
    rel == ".git"
        || rel == "target"
        || rel == ".almanac"
        || rel == ".hermes"
        || rel == ".serena"
        || rel.starts_with(".git/")
        || rel.starts_with("target/")
        || rel.starts_with(".almanac/")
        || rel.starts_with(".hermes/")
        || rel.starts_with(".serena/")
}

fn collect_markdown_files_recursive(root: &Path, current: &Path, files: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(current) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if should_skip_dir(root, &path) {
            continue;
        }
        if path.is_dir() {
            collect_markdown_files_recursive(root, &path, files);
        } else if path.extension().is_some_and(|ext| ext == "md") {
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
                || name == ".serena"
    )
}

fn check_markdown_links(
    root: &Path,
    file: &Path,
    text: &str,
    violations: &mut Vec<DocsHygieneViolation>,
) {
    for (line_index, line) in text.lines().enumerate() {
        let line_number = line_index + 1;
        for raw_target in markdown_link_targets(line) {
            let Some(normalized) = normalize_relative_target(&raw_target) else {
                continue;
            };
            if normalized.starts_with('/') {
                continue;
            }
            let file_relative = file.parent().unwrap_or(root).join(&normalized);
            let root_relative = root.join(&normalized);
            if !file_relative.exists() && !root_relative.exists() {
                violations.push(DocsHygieneViolation {
                    path: file.strip_prefix(root).unwrap_or(file).to_path_buf(),
                    line: line_number,
                    kind: DocsHygieneViolationKind::BrokenRelativeLink,
                    message: format!(
                        "broken relative Markdown link `{raw_target}` (normalized `{normalized}`)"
                    ),
                });
            }
        }
    }
}

fn markdown_link_targets(line: &str) -> Vec<String> {
    let mut targets = Vec::new();
    let bytes = line.as_bytes();
    let mut index = 0;
    while index + 1 < bytes.len() {
        if bytes[index] == b']' && bytes[index + 1] == b'(' {
            let start = index + 2;
            if let Some(end_offset) = line[start..].find(')') {
                let raw = &line[start..start + end_offset];
                if let Some(target) = raw.split_whitespace().next() {
                    targets.push(target.trim_matches('<').trim_matches('>').to_string());
                }
                index = start + end_offset + 1;
                continue;
            }
        }
        index += 1;
    }
    targets
}

fn is_allowlisted(root: &Path, file: &Path, config: &DocsHygieneConfig) -> bool {
    let rel = repo_relative_path(file.strip_prefix(root).unwrap_or(file));
    config.allowlisted_paths.iter().any(|prefix| {
        let prefix = normalize_allowlist_prefix(prefix);
        !prefix.is_empty() && rel.starts_with(&prefix)
    })
}

fn repo_relative_path(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

fn normalize_allowlist_prefix(prefix: &str) -> String {
    prefix.trim().replace('\\', "/")
}

fn check_stale_devil_references(
    root: &Path,
    file: &Path,
    text: &str,
    violations: &mut Vec<DocsHygieneViolation>,
) {
    for (line_index, line) in text.lines().enumerate() {
        if let Some(token) = first_devil_token(line) {
            violations.push(DocsHygieneViolation {
                path: file.strip_prefix(root).unwrap_or(file).to_path_buf(),
                line: line_index + 1,
                kind: DocsHygieneViolationKind::StaleDevilReference,
                message: format!("unallowlisted stale Legion rename marker `{token}`"),
            });
        }
    }
}

fn check_current_production_plan_reference(
    root: &Path,
    file: &Path,
    text: &str,
    violations: &mut Vec<DocsHygieneViolation>,
) {
    let relative_path = repo_relative_path(file.strip_prefix(root).unwrap_or(file));
    if !PRODUCTION_PLAN_ENTRYPOINTS.contains(&relative_path.as_str()) {
        return;
    }
    if text.contains(LATEST_PRODUCTION_MASTER_PLAN) {
        return;
    }

    violations.push(DocsHygieneViolation {
        path: PathBuf::from(relative_path),
        line: 1,
        kind: DocsHygieneViolationKind::StaleProductionPlanReference,
        message: format!(
            "current documentation entrypoint must reference latest production master plan `{LATEST_PRODUCTION_MASTER_PLAN}`"
        ),
    });
}

fn first_devil_token(line: &str) -> Option<&str> {
    for marker in ["devil-", "devil_", "Devil IDE"] {
        let Some(start) = line.find(marker) else {
            continue;
        };
        // For "Devil IDE" the marker itself is already a complete multi-word token
        // (it terminates at the next non-space/word boundary we don't want to extend).
        // For "devil-" / "devil_" we extend to capture the full identifier token
        // (e.g. "devil-app", "devil_x") by walking forward over word characters,
        // dashes, and underscores until a non-continuation character is found.
        if marker.contains(' ') {
            return Some(&line[start..start + marker.len()]);
        }
        let mut end = start + marker.len();
        let bytes = line.as_bytes();
        while end < bytes.len() {
            let ch = bytes[end];
            if ch.is_ascii_alphanumeric() || ch == b'-' || ch == b'_' {
                end += 1;
            } else {
                break;
            }
        }
        return Some(&line[start..end]);
    }
    None
}
