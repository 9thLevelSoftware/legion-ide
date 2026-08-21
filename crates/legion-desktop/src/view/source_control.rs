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
    render_path_stage_controls(ui, snapshot, actions);
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
/// A merge already underway counts even with nothing staged: `git commit`
/// concludes it whether or not the index differs from `HEAD`, and resolving a
/// final conflict toward the current side produces exactly that empty index.
/// Cherry-pick and revert do *not* count -- git refuses an empty commit for
/// those -- so offering Commit on their behalf would be a button that only
/// errors.
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
    if files.iter().any(|file| status_is_committable(&file.status)) {
        return true;
    }
    // Nothing staged, but the repository may still owe a merge commit.
    //
    // Resolving the last conflict with **Use Current** can leave the index
    // identical to `HEAD`, and porcelain status then reports no entries at all
    // -- while `MERGE_HEAD` is still present and `git commit` would succeed and
    // conclude the merge. Deciding from changed files alone therefore removed
    // the panel's only Commit control in direct response to the panel's own
    // conflict action, stranding the merge with no way to finish it from here.
    //
    // Reached only after the unmerged veto above, so this cannot re-offer
    // Commit while conflicts remain.
    snapshot.git_projection.merge_awaiting_commit
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

/// Whether a whole-path stage control is safe to offer for this file.
///
/// "Has no hunk" is not sufficient, and each way it falls short stages
/// something the person did not choose:
///
/// * **A truncated hunk list.** Earlier files can consume the snapshot's hunk
///   allowance, leaving a text file with real hunks and no entry in the
///   projected vector. Staging the whole path then stages every hunk in it,
///   including the ones deliberately left unstaged. Numstat is the tell: it
///   comes from `git diff --numstat` and is not subject to the hunk budget, so
///   a file reporting changed lines has hunks whether or not they survived.
/// * **An untracked directory.** Porcelain projects `?? dir/` as one row, and
///   `git add dir/` stages everything inside it — many files, from one click,
///   on a row that named none of them.
/// * **A rename.** The projected path is porcelain's trailing field, which is
///   the *source* name. Unstaging with it leaves the destination staged as an
///   addition and restores only the deletion: a half-undone rename, which is
///   worse than no control at all.
///
/// Withholding is deliberate over guessing. The panel says why beneath the list.
fn path_control_is_safe(file: &legion_ui::GitFileProjection, hunks_truncated: bool) -> bool {
    if file.path.ends_with('/') {
        return false;
    }
    if file
        .status
        .chars()
        .next()
        .is_some_and(|index| index == 'R' || index == 'C')
    {
        return false;
    }
    if hunks_truncated && (file.inserted_lines > 0 || file.deleted_lines > 0) {
        return false;
    }
    true
}

/// Stage or unstage a whole path, for changes no hunk can express.
///
/// `git diff` emits no `@@` hunk for a file git has never seen, for a modified
/// binary, for a mode-only change, or for a pure rename. Every one of those
/// appears in status as a changed file, and none of them could be staged from
/// this panel — which left the commit control this PR added unusable for them
/// without dropping to a terminal.
///
/// This used to be a sentence explaining that untracked files must be added with
/// git first. The explanation was honest about the gap and did nothing about it;
/// `git add -- <path>` reaches all four cases, so the panel now offers the
/// action instead of apologising for its absence.
///
/// Only files with no hunk of their own get a control here. A file that *does*
/// have hunks is staged hunk by hunk above, and a whole-path button beside those
/// would silently stage changes the person had deliberately left out.
fn render_path_stage_controls(
    ui: &mut egui::Ui,
    snapshot: &ShellProjectionSnapshot,
    actions: &mut Vec<DesktopAction>,
) {
    let hunk_paths: std::collections::BTreeSet<&str> = snapshot
        .git_projection
        .hunks
        .iter()
        .map(|hunk| hunk.path.as_str())
        .collect();

    let truncated = snapshot.git_projection.hunks_truncated;
    let eligible: Vec<&legion_ui::GitFileProjection> = snapshot
        .git_projection
        .changed_files
        .iter()
        .filter(|file| !hunk_paths.contains(file.path.as_str()))
        // An unmerged path is not stageable by `git add` in any useful sense:
        // it would mark a conflict resolved that nobody resolved.
        .filter(|file| !status_pair(&file.status).is_some_and(|(x, y)| is_unmerged(x, y)))
        .collect();
    let (candidates, withheld): (Vec<_>, Vec<_>) = eligible
        .into_iter()
        .partition(|file| path_control_is_safe(file, truncated));

    if candidates.is_empty() && withheld.is_empty() {
        return;
    }
    let shown = candidates.len().min(GIT_HUNK_CONTROL_LIMIT);
    let hidden = candidates.len().saturating_sub(shown);
    let candidates = &candidates[..shown];

    super::components::section_header(ui, "Files", Some(theme::tokens().accent.cyan));
    for file in candidates {
        ui.horizontal_wrapped(|ui| {
            let staged = status_is_committable(&file.status);
            let verb = if staged { "Unstage" } else { "Stage" };
            let response = soft_button(ui, verb);
            // The visible label is the verb; the accessible label names the path
            // it acts on, for the same reason the hunk controls do it — a column
            // of buttons all called "Stage" is unusable with a screen reader and
            // untestable from the accessibility tree.
            ui.ctx().accesskit_node_builder(response.id, |node| {
                node.set_label(format!("{verb} {}", file.path));
            });
            if response.clicked() {
                actions.push(if staged {
                    DesktopAction::UnstageGitPath {
                        path: file.path.clone(),
                    }
                } else {
                    DesktopAction::StageGitPath {
                        path: file.path.clone(),
                    }
                });
            }
            ui.label(theme::muted(format!("{} {}", file.status, file.path)));
        });
    }
    // The hunk controls above state their own truncation; a silent cap here
    // would show twelve buttons and no sign that a thirteenth file exists.
    if hidden > 0 {
        ui.label(theme::muted(format!("{hidden} more file(s) not shown")));
    }
    if !withheld.is_empty() {
        // Named rather than omitted. A row that quietly has no button beside
        // rows that do reads as the panel being broken -- the same reason the
        // untracked note existed before there was an action to replace it.
        ui.label(theme::muted(format!(
            "{} file(s) need git directly: a directory, a rename, or a file whose \
             hunks exceeded this view's budget",
            withheld.len()
        )));
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

#[cfg(test)]
mod path_control_safety {
    use super::path_control_is_safe;
    use legion_ui::GitFileProjection;

    fn file(path: &str, status: &str, inserted: u32, deleted: u32) -> GitFileProjection {
        GitFileProjection {
            path: path.to_string(),
            status: status.to_string(),
            inserted_lines: inserted,
            deleted_lines: deleted,
            unstaged_hunk_count: 0,
            staged_hunk_count: 0,
            stageable: false,
            diff_strategy: legion_ui::GitDiffStrategyProjection::LineFallback,
            fallback_reason: None,
            conflict: false,
        }
    }

    /// The cases the control exists for stay offered.
    #[test]
    fn hunkless_files_keep_their_control() {
        // Untracked, binary modification, mode-only change: none produce a hunk
        // and none report changed lines, so none can be hiding staged content.
        for (path, status) in [("new.rs", "??"), ("blob.bin", " M"), ("run.sh", " M")] {
            assert!(
                path_control_is_safe(&file(path, status, 0, 0), false),
                "{path} is exactly the case this control was added for"
            );
        }
    }

    /// A truncated hunk list must not make a text file look hunkless.
    ///
    /// Earlier files can consume the snapshot's hunk allowance, leaving a file
    /// with real hunks and no entry in the projected vector. A whole-path Stage
    /// would then stage every hunk in it -- including the ones deliberately left
    /// unstaged, which is the one outcome hunk-level staging exists to prevent.
    ///
    /// Numstat is the tell: it comes from `git diff --numstat` and is not
    /// subject to the hunk budget.
    #[test]
    fn a_text_file_with_truncated_hunks_is_withheld() {
        let changed = file("big.rs", " M", 40, 12);
        assert!(
            !path_control_is_safe(&changed, true),
            "a file reporting changed lines has hunks, whether or not they survived truncation"
        );
        assert!(
            path_control_is_safe(&changed, false),
            "with nothing truncated the projection is complete and can be trusted"
        );
    }

    /// An untracked directory stages files the row never named.
    #[test]
    fn an_untracked_directory_is_withheld() {
        assert!(
            !path_control_is_safe(&file("build/", "??", 0, 0), false),
            "porcelain projects an untracked directory as one row, and `git add dir/` stages everything inside it"
        );
    }

    /// A rename carries the source path, so unstaging it half-undoes the rename.
    #[test]
    fn a_rename_is_withheld() {
        for status in ["R ", "RM", "C "] {
            assert!(
                !path_control_is_safe(&file("old.rs", status, 0, 0), false),
                "{status} projects the source name; unstaging with it leaves the destination staged as an addition"
            );
        }
    }
}
