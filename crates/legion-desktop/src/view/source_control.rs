//! Source-control panel rendering for the desktop adapter.
//!
//! Extracted from `view.rs` so the Source Control surface can be changed
//! without growing the shell's single largest file.
//!
//! The module covers three things that all read the same `GitProjection`: the
//! panel's controls, the status rows the sidebar lists, and the helpers the code
//! canvas uses to draw hunk markers and inline blame in the gutter.

use legion_protocol::TextCoordinate;
use legion_ui::{
    GitBlameLineProjection, GitHunkProjection, GitHunkStageProjection, PaletteMode,
    ShellProjectionSnapshot,
};

use super::components::soft_button;
use super::{bounded_join, theme, trim_middle};
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
        // Commit is offered only while something is staged. `commit_git_changes`
        // fails on an empty index, so a Commit button that is always live is a
        // button whose usual outcome is an error toast.
        //
        // It routes through the palette because a commit needs a message and
        // this renderer owns no text state: the palette already has the input
        // field, the `git-commit` command, and the operand parser, so the button
        // hands the user to the flow that exists rather than inventing a second
        // one.
        if index_has_staged_changes(snapshot) && soft_button(ui, "Commit…").clicked() {
            actions.push(DesktopAction::OpenPalette {
                mode: PaletteMode::Command,
                query: COMMIT_PALETTE_QUERY.to_string(),
                scope: snapshot.search_projection.scope,
            });
        }
    });
    render_git_hunk_controls(ui, snapshot, actions);
    render_untracked_note(ui, snapshot);
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

/// The palette query that opens the commit flow with its operand ready.
///
/// `>` selects command mode and `git commit ` is the prefix
/// `parse_palette_command_operands` strips before treating the rest as the
/// message, so the user lands with the cursor where the message goes.
const COMMIT_PALETTE_QUERY: &str = ">git commit ";

/// How many hunks get their own stage/unstage control.
///
/// The sidebar is a column, not a diff viewer; past this the panel is a wall of
/// buttons. The overflow is stated rather than silently dropped.
const GIT_HUNK_CONTROL_LIMIT: usize = 12;

/// Whether the index holds anything to commit, read from porcelain status.
///
/// Deliberately not "is there a staged hunk". A staged binary modification, an
/// empty-file addition, a mode-only change and a pure rename all appear in
/// `changed_files` with a staged index column and produce no `@@` hunk at all,
/// so a hunk-counting gate hides the panel's only Commit control while
/// `git commit` would succeed. The same happens when unstaged hunks exhaust the
/// projection's hunk limit before the staged ones are collected.
///
/// Porcelain status is two columns, `XY`: `X` is the index and `Y` the working
/// tree. Anything in `X` other than a space is staged; `?` is the untracked
/// marker (`??`), which is not.
///
/// Unmerged entries are excluded even though their index column qualifies.
/// `DD`, `AU`, `UD`, `UA`, `DU`, `AA` and `UU` all mean an unresolved merge, and
/// `git commit` refuses an unmerged index -- so counting them as committable
/// offers a button whose only outcome is an error. That includes conflicts with
/// no textual markers, such as binary or delete/modify, where nothing else on
/// the panel would hint at why the commit failed.
fn index_has_staged_changes(snapshot: &ShellProjectionSnapshot) -> bool {
    let files = &snapshot.git_projection.changed_files;
    // One unmerged path vetoes the whole commit, so this cannot be a per-entry
    // test. `git commit` refuses while *any* entry is unmerged, and a repository
    // mid-merge can easily also hold an independently staged file -- which made
    // the per-entry version offer Commit again for exactly the case it was
    // added to prevent.
    if files
        .iter()
        .any(|file| status_pair(&file.status).is_some_and(|(x, y)| is_unmerged(x, y)))
    {
        return false;
    }
    files.iter().any(|file| status_is_committable(&file.status))
}

/// The two porcelain columns, when the status is well formed.
fn status_pair(status: &str) -> Option<(char, char)> {
    let mut columns = status.chars();
    match (columns.next(), columns.next()) {
        (Some(index), Some(worktree)) => Some((index, worktree)),
        _ => None,
    }
}

/// Whether a porcelain status pair represents something `git commit` can commit.
fn status_is_committable(status: &str) -> bool {
    let Some((index, worktree)) = status_pair(status) else {
        return false;
    };
    if is_unmerged(index, worktree) {
        return false;
    }
    index != ' ' && index != '?'
}

/// The seven porcelain pairs that mean an unresolved merge.
fn is_unmerged(index: char, worktree: char) -> bool {
    matches!(
        (index, worktree),
        ('D', 'D') | ('A', 'U') | ('U', 'D') | ('U', 'A') | ('D', 'U') | ('A', 'A') | ('U', 'U')
    )
}

/// Per-hunk stage and unstage controls.
///
/// ## Why these had to exist at all
///
/// `DesktopAction::StageGitHunk` and `UnstageGitHunk` have been wired from the
/// bridge through `AppCommandRequest` to `git apply --cached` for as long as the
/// panel has existed, and nothing in the renderer ever pushed either one. The
/// Source Control surface offered Fetch, Pull, Push and Open PR — every verb
/// that talks to a *remote* — and no way to put a single line into the index or
/// to commit it. Push was the only write the panel could perform, and it could
/// only ever push what some other tool had staged.
///
/// The comment on the commit-validation rows in [`git_rows`] said those errors
/// are "shown near the commit action". There was no commit action.
///
/// ## Why one hunk per click, and no "Stage All"
///
/// A hunk's identity is `hash(stage:path:header:index)`, and `header` carries
/// the line numbers of the diff it came from. Staging one hunk rewrites the
/// index, so every remaining hunk in that file is re-derived against new line
/// numbers and gets a *new* id. A "Stage All" that queued N actions from one
/// frame's projection would stage the first and fail the rest with
/// `git_hunk_missing`. Staging a hunk at a time is not a lesser affordance here;
/// it is the only one the identity scheme supports.
fn render_git_hunk_controls(
    ui: &mut egui::Ui,
    snapshot: &ShellProjectionSnapshot,
    actions: &mut Vec<DesktopAction>,
) {
    let hunks = &snapshot.git_projection.hunks;
    if hunks.is_empty() {
        return;
    }
    // Budgeted per stage, not as a combined prefix. The projection appends
    // every unstaged hunk before any staged one, so a plain `take` over the
    // combined list renders no Unstage control at all once twelve unstaged
    // hunks exist -- forcing someone to stage unrelated changes before the hunk
    // they wanted to unstage becomes reachable.
    let half = GIT_HUNK_CONTROL_LIMIT / 2;
    let staged_total = hunks
        .iter()
        .filter(|hunk| hunk.stage == GitHunkStageProjection::Staged)
        .count();
    let unstaged_total = hunks.len() - staged_total;
    // An unused half is given back, so a repository with only one kind of hunk
    // still fills the whole budget.
    let staged_budget = half.max(GIT_HUNK_CONTROL_LIMIT.saturating_sub(unstaged_total));
    let unstaged_budget = GIT_HUNK_CONTROL_LIMIT.saturating_sub(staged_budget.min(staged_total));
    let mut staged_shown = 0usize;
    let mut unstaged_shown = 0usize;
    let visible: Vec<_> = hunks
        .iter()
        .filter(|hunk| match hunk.stage {
            GitHunkStageProjection::Staged => {
                staged_shown += 1;
                staged_shown <= staged_budget
            }
            GitHunkStageProjection::Unstaged => {
                unstaged_shown += 1;
                unstaged_shown <= unstaged_budget
            }
        })
        .collect();
    for hunk in visible {
        ui.horizontal_wrapped(|ui| {
            let verb = match hunk.stage {
                GitHunkStageProjection::Unstaged => "Stage",
                GitHunkStageProjection::Staged => "Unstage",
            };
            let response = soft_button(ui, verb);
            // The visible label is the verb; the accessible label names the hunk
            // it acts on. Twelve buttons all called "Stage" are unusable with a
            // screen reader and untestable from the accessibility tree, because
            // nothing distinguishes the one that was clicked from the eleven
            // that were not.
            let accessible = format!("{verb} {} {}", hunk.path, hunk.header);
            ui.ctx().accesskit_node_builder(response.id, |node| {
                node.set_label(accessible.clone());
            });
            if response.clicked() {
                actions.push(match hunk.stage {
                    GitHunkStageProjection::Unstaged => DesktopAction::StageGitHunk {
                        hunk_id: hunk.hunk_id.clone(),
                    },
                    GitHunkStageProjection::Staged => DesktopAction::UnstageGitHunk {
                        hunk_id: hunk.hunk_id.clone(),
                    },
                });
            }
            ui.label(theme::muted(format!(
                "{} +{} -{}",
                hunk.path, hunk.added_lines, hunk.deleted_lines
            )));
        });
    }
    // The count is stated only when it is knowable. The projection caps what it
    // collects, so a panel subtracting what it drew from what it received would
    // report "116 more" for a repository with sixteen thousand -- a precise
    // number that is precisely wrong. When the projection says it truncated,
    // the panel says there are more without pretending to know how many.
    let shown = staged_shown.min(staged_budget) + unstaged_shown.min(unstaged_budget);
    if snapshot.git_projection.hunks_truncated {
        ui.label(theme::muted(
            "More hunks than can be listed; refine the change set to review the rest",
        ));
    } else if hunks.len() > shown {
        ui.label(theme::muted(format!(
            "{} more hunks not shown",
            hunks.len() - shown
        )));
    }
}

/// Say why untracked files have no stage control, instead of leaving a row that
/// silently has no button next to rows that do.
///
/// Staging goes through `git apply --cached` on a projected hunk, and `git diff`
/// emits no hunks for a file git has never seen — so an untracked file projects
/// with `stageable: false` and cannot be staged from here. Adding one to the
/// index needs a path-level `git add`, which is authority the app layer does not
/// have; it is a gap, not a rendering bug, and the panel should say so rather
/// than look broken.
fn render_untracked_note(ui: &mut egui::Ui, snapshot: &ShellProjectionSnapshot) {
    let untracked = snapshot
        .git_projection
        .changed_files
        .iter()
        .filter(|file| file.status.trim() == "??")
        .count();
    if untracked == 0 {
        return;
    }
    let noun = if untracked == 1 { "file" } else { "files" };
    ui.label(theme::muted(format!(
        "{untracked} untracked {noun}: add with git before staging here"
    )));
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
