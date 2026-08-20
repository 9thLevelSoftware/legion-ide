//! Source-control panel rendering for the desktop adapter.
//!
//! Extracted from `view.rs` as a pure move so the Source Control surface can be
//! changed without growing the shell's single largest file. Everything here was
//! already written; nothing in this commit changes behaviour.
//!
//! The module covers three things that all read the same `GitProjection`: the
//! panel's controls, the status rows the sidebar lists, and the helpers the code
//! canvas uses to draw hunk markers and inline blame in the gutter.

use legion_protocol::TextCoordinate;
use legion_ui::{GitBlameLineProjection, GitHunkProjection, ShellProjectionSnapshot};

use super::components::soft_button;
use super::{bounded_join, trim_middle};
use crate::bridge::DesktopAction;

pub(super) fn render_git_controls(
    ui: &mut egui::Ui,
    snapshot: &ShellProjectionSnapshot,
    actions: &mut Vec<DesktopAction>,
) {
    ui.horizontal_wrapped(|ui| {
        if soft_button(ui, "Refresh Git").clicked() {
            actions.push(DesktopAction::RefreshGit);
        }
        if snapshot.git_projection.branch_label.is_some() {
            // Remote verbs. Each dispatch is policy-gated in the app layer and
            // records a verdict row, so a refusal appears in the panel body
            // rather than failing silently.
            if soft_button(ui, "Fetch").clicked() {
                actions.push(DesktopAction::FetchGitRemote);
            }
            if soft_button(ui, "Pull").clicked() {
                actions.push(DesktopAction::PullGitRemote);
            }
            if soft_button(ui, "Push").clicked() {
                actions.push(DesktopAction::PushGitRemote);
            }
            if soft_button(ui, "Open PR").clicked() {
                actions.push(DesktopAction::OpenGitPullRequestUrl);
            }
        }
        // Offer the grant only while a host-naming denial is the standing
        // verdict, so consent is asked for at the moment it is meaningful.
        if let Some(host) = snapshot
            .git_projection
            .remote_policy_audit
            .iter()
            .rev()
            .find(|row| !row.allowed && row.host.is_some())
            .and_then(|row| row.host.as_deref())
            && soft_button(ui, &format!("Allow {host}")).clicked()
        {
            actions.push(DesktopAction::GrantDeniedGitRemoteHost);
        }
    });
    if let Some(conflict) = snapshot.git_projection.conflicts.first() {
        ui.horizontal_wrapped(|ui| {
            if soft_button(ui, "Use Current").clicked() {
                actions.push(DesktopAction::AcceptGitConflictCurrent {
                    path: conflict.path.clone(),
                });
            }
            if soft_button(ui, "Use Incoming").clicked() {
                actions.push(DesktopAction::AcceptGitConflictIncoming {
                    path: conflict.path.clone(),
                });
            }
        });
    }
}

pub(super) fn git_relative_path(
    root_label: Option<&str>,
    file_path: Option<&str>,
) -> Option<String> {
    let root = root_label?;
    let file_path = file_path?;
    let remainder = file_path.strip_prefix(root)?;
    // Require a component boundary so root `/repo` does not match `/repo2/...`:
    // the remainder must be empty, begin with a separator, or the root itself
    // must already end with a separator.
    let boundary_ok = remainder.is_empty()
        || remainder.starts_with('/')
        || remainder.starts_with('\\')
        || root.ends_with('/')
        || root.ends_with('\\');
    if !boundary_ok {
        return None;
    }
    let relative = remainder.trim_start_matches(['/', '\\']).to_string();
    (!relative.is_empty()).then_some(relative)
}

pub(super) fn active_git_relative_path(snapshot: &ShellProjectionSnapshot) -> Option<String> {
    git_relative_path(
        snapshot.git_projection.root_label.as_deref(),
        snapshot
            .active_buffer_projection
            .file_path
            .as_ref()
            .map(|path| path.0.as_str()),
    )
}

pub(super) fn git_hunk_marker_for_line(
    relative_path: Option<&str>,
    hunks: &[GitHunkProjection],
    line_number: u32,
) -> Option<&'static str> {
    let relative_path = relative_path?;
    hunks
        .iter()
        .filter(|hunk| hunk.path == relative_path)
        .filter(|hunk| {
            line_number >= hunk.new_start && line_number < hunk.new_start + hunk.new_lines
        })
        .map(
            |hunk| match (hunk.added_lines > 0, hunk.deleted_lines > 0) {
                (true, true) => "~",
                (true, false) => "+",
                (false, true) => "-",
                (false, false) => "•",
            },
        )
        .next()
}

pub(super) fn git_inline_blame_label(
    relative_path: Option<&str>,
    blame_lines: &[GitBlameLineProjection],
    line_number: u32,
) -> Option<String> {
    let relative_path = relative_path?;
    blame_lines
        .iter()
        .find(|line| line.path == relative_path && line.line_number == line_number)
        .map(|line| {
            format!(
                "{} {} {}",
                line.commit_short,
                trim_middle(&line.author, 20),
                trim_middle(&line.summary, 36)
            )
        })
}

pub(super) fn git_previous_hunk_cursor(
    relative_path: Option<&str>,
    hunks: &[GitHunkProjection],
    current_line: u32,
) -> Option<TextCoordinate> {
    let relative_path = relative_path?;
    hunks
        .iter()
        .filter(|hunk| hunk.path == relative_path && hunk.new_start < current_line)
        .max_by_key(|hunk| hunk.new_start)
        .map(|hunk| TextCoordinate {
            line: hunk.new_start.saturating_sub(1),
            character: 0,
            byte_offset: None,
            utf16_offset: None,
        })
}

pub(super) fn git_next_hunk_cursor(
    relative_path: Option<&str>,
    hunks: &[GitHunkProjection],
    current_line: u32,
) -> Option<TextCoordinate> {
    let relative_path = relative_path?;
    hunks
        .iter()
        .filter(|hunk| hunk.path == relative_path && hunk.new_start > current_line)
        .min_by_key(|hunk| hunk.new_start)
        .map(|hunk| TextCoordinate {
            line: hunk.new_start.saturating_sub(1),
            character: 0,
            byte_offset: None,
            utf16_offset: None,
        })
}

pub(super) fn git_rows(snapshot: &ShellProjectionSnapshot) -> Vec<String> {
    let git = &snapshot.git_projection;
    let mut rows = Vec::new();
    if git.root_label.is_some()
        || !git.changed_files.is_empty()
        || !git.hunks.is_empty()
        || !git.blame_lines.is_empty()
        || !git.commits.is_empty()
        || !git.conflicts.is_empty()
        || !git.worktrees.is_empty()
        || !git.diagnostics.is_empty()
    {
        rows.push(format!(
            "git: branch={} head={} changes={} hunks={} conflicts={} worktrees={}",
            git.branch_label.as_deref().unwrap_or("<none>"),
            git.head_short.as_deref().unwrap_or("<none>"),
            git.changed_files.len(),
            git.hunks.len(),
            git.conflicts.len(),
            git.worktrees.len()
        ));
    }
    rows.extend(git.changed_files.iter().take(16).map(|file| {
        format!(
            "git file {} status={} diff={:?} +{} -{} hunks={}/{} conflict={}",
            file.path,
            file.status,
            file.diff_strategy,
            file.inserted_lines,
            file.deleted_lines,
            file.staged_hunk_count,
            file.unstaged_hunk_count,
            file.conflict
        )
    }));
    rows.extend(git.hunks.iter().take(20).map(|hunk| {
        format!(
            "git hunk {} {} stage={:?} +{} -{} {}",
            hunk.hunk_id, hunk.path, hunk.stage, hunk.added_lines, hunk.deleted_lines, hunk.header
        )
    }));
    rows.extend(git.blame_lines.iter().take(12).map(|line| {
        format!(
            "git blame {}:{} {} {} {}",
            line.path, line.line_number, line.commit_short, line.author, line.summary
        )
    }));
    rows.extend(git.commits.iter().take(12).map(|commit| {
        format!(
            "git commit {} parents={} refs={} {}",
            commit.short_hash,
            commit.parent_count,
            bounded_join(&commit.refs),
            commit.summary
        )
    }));
    rows.extend(git.conflicts.iter().take(8).map(|conflict| {
        format!(
            "git conflict {} markers={} actions={}",
            conflict.path,
            conflict.marker_count,
            bounded_join(&conflict.actions)
        )
    }));
    rows.extend(git.worktrees.iter().take(12).map(|worktree| {
        format!(
            "git worktree {} branch={} head={} kind={:?} prunable={}",
            worktree.path,
            worktree.branch_label.as_deref().unwrap_or("<detached>"),
            worktree.head_short.as_deref().unwrap_or("<none>"),
            worktree.kind,
            worktree.prunable
        )
    }));
    rows.extend(
        git.diagnostics
            .iter()
            .take(8)
            .map(|diagnostic| format!("git diagnostic {diagnostic}")),
    );
    // Network/auth policy verdicts (P2.F5.T4). Newest last, so the tail of the
    // list is the decision for the operation the user just attempted. Denied
    // rows are prefixed distinctly so a refusal cannot be mistaken for success.
    rows.extend(
        git.remote_policy_audit
            .iter()
            .rev()
            .take(4)
            .rev()
            .map(|row| {
                let verdict = if row.allowed { "allowed" } else { "DENIED" };
                format!("git policy {verdict}: {}", row.detail)
            }),
    );
    // Commit validation errors (hard blockers) — shown near the commit action.
    rows.extend(
        git.commit_validation_errors
            .iter()
            .take(4)
            .map(|err| format!("git commit-error: {err}")),
    );
    // Advisory commit validation warnings — shown near the commit action.
    rows.extend(
        git.commit_validation_warnings
            .iter()
            .take(4)
            .map(|warn| format!("git commit-warning: {warn}")),
    );
    rows
}
