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
    GitBlameLineProjection, GitHunkProjection, GitHunkStageProjection, GitRefreshState,
    PaletteMode, ShellProjectionSnapshot,
};

use super::components::soft_button;
use super::{bounded_join, theme, trim_middle};
use crate::bridge::DesktopAction;

pub(super) fn render_git_controls(
    ui: &mut egui::Ui,
    snapshot: &ShellProjectionSnapshot,
    actions: &mut Vec<DesktopAction>,
) {
    let refresh_label = match snapshot.git_projection.refresh_state {
        GitRefreshState::Idle => None,
        GitRefreshState::Refreshing => Some("Git refresh: Refreshing"),
        GitRefreshState::TimedOut => Some("Git refresh: TimedOut"),
        GitRefreshState::Failed => Some("Git refresh: Failed"),
        GitRefreshState::AuthRequired => Some("Git refresh: AuthRequired"),
    };
    if let Some(label) = refresh_label {
        let response = ui.label(theme::muted(label));
        ui.ctx().accesskit_node_builder(response.id, |node| {
            node.set_role(egui::accesskit::Role::Status);
            node.set_label(label);
        });
    }
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

/// Renderer-memory keys for how far each stage's window has been advanced.
const HUNK_STAGED_WINDOW: &str = "legion.source-control.window.hunks.staged";
/// See [`HUNK_STAGED_WINDOW`].
const HUNK_UNSTAGED_WINDOW: &str = "legion.source-control.window.hunks.unstaged";
/// See [`HUNK_STAGED_WINDOW`].
const PATH_STAGED_WINDOW: &str = "legion.source-control.window.paths.staged";
/// See [`HUNK_STAGED_WINDOW`].
const PATH_UNSTAGED_WINDOW: &str = "legion.source-control.window.paths.unstaged";

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
/// A page of a list that mixes staged and unstaged items.
pub(crate) struct StageWindow<'a, T> {
    /// The items to draw, in projection order.
    pub(crate) visible: Vec<&'a T>,
    /// Staged items this page does not show.
    pub(crate) staged_hidden: usize,
    /// Unstaged items this page does not show.
    pub(crate) unstaged_hidden: usize,
    /// How far a stage's offset advances when someone asks for the next page.
    pub(crate) staged_budget: usize,
    /// How far the unstaged offset advances.
    pub(crate) unstaged_budget: usize,
    /// The staged offset this page actually used, after normalization.
    pub(crate) staged_offset: usize,
    /// The unstaged offset this page actually used.
    pub(crate) unstaged_offset: usize,
}

/// One page of a two-stage list, budgeted per stage and offset within each.
///
/// Written once for both callers. The hunk controls and the path controls had
/// structurally identical copies of this 17 lines, 173 apart, differing only in
/// how they asked whether an item was staged -- two copies of a formula that the
/// next person to tighten it would have found only one of.
///
/// Budgeted per stage rather than as a prefix over the combined list, because
/// the projection appends every unstaged item before any staged one: a plain
/// `take` renders no Unstage control at all once the limit is filled with
/// unstaged items, forcing someone to stage unrelated changes before the item
/// they wanted to unstage is reachable. Either side's unused share goes to the
/// other, so a list of only one kind still fills the whole budget.
///
/// Offset, because a budget alone leaves the tail permanently out of reach.
/// Thirteen staged hunks and nothing unstaged showed the first twelve; unstaging
/// a visible one made it 12 staged and 1 unstaged, whose budgets show eleven
/// staged -- so the thirteenth stayed hidden, and restaging returned to the
/// start. No sequence of the controls on screen could reach it. The offset is
/// what the "show the rest" control moves, and it wraps, so every item is
/// reachable in a bounded number of clicks and the way back is the same control.
pub(crate) fn stage_window<'a, T>(
    items: &'a [T],
    limit: usize,
    staged_offset: usize,
    unstaged_offset: usize,
    is_staged: impl Fn(&T) -> bool,
) -> StageWindow<'a, T> {
    let staged_total = items.iter().filter(|item| is_staged(item)).count();
    let unstaged_total = items.len() - staged_total;
    let half = limit / 2;
    let staged_budget = half.max(limit.saturating_sub(unstaged_total));
    let unstaged_budget = limit.saturating_sub(staged_budget.min(staged_total));

    // An offset at or past the end shows the first page rather than an empty
    // one. A control that can leave the list blank is a control that can lose
    // the list.
    let staged_offset = if staged_offset >= staged_total {
        0
    } else {
        staged_offset
    };
    let unstaged_offset = if unstaged_offset >= unstaged_total {
        0
    } else {
        unstaged_offset
    };

    let mut staged_seen = 0usize;
    let mut unstaged_seen = 0usize;
    let mut staged_shown = 0usize;
    let mut unstaged_shown = 0usize;
    let mut visible: Vec<&'a T> = Vec::new();
    for item in items {
        if is_staged(item) {
            let index = staged_seen;
            staged_seen += 1;
            if index >= staged_offset && staged_shown < staged_budget {
                staged_shown += 1;
                visible.push(item);
            }
        } else {
            let index = unstaged_seen;
            unstaged_seen += 1;
            if index >= unstaged_offset && unstaged_shown < unstaged_budget {
                unstaged_shown += 1;
                visible.push(item);
            }
        }
    }

    StageWindow {
        visible,
        staged_hidden: staged_total - staged_shown,
        unstaged_hidden: unstaged_total - unstaged_shown,
        staged_budget,
        unstaged_budget,
        // Reported, not just applied. The wrap above is local to this call, so
        // a caller that advanced from the *stored* value kept adding a budget
        // to an offset the window had already discarded: page to the last
        // staged hunk and unstage it, and the stored 12 outlives the list it
        // indexed into, so every later click re-rendered the first page and the
        // final Unstage control could not be reached again.
        staged_offset,
        unstaged_offset,
    }
}

/// Read a stage's stored window offset out of renderer memory.
///
/// Adapter-local view state, in the same category as explorer expansion: which
/// page of a list somebody is looking at is not something the app decides, and
/// it does not belong in a session record.
fn window_offset(ui: &egui::Ui, key: &'static str) -> usize {
    ui.ctx()
        .data_mut(|data| data.get_temp::<usize>(egui::Id::new(key)))
        .unwrap_or(0)
}

/// Draw the control that moves a stage's window, when anything is behind it.
fn render_window_advance(
    ui: &mut egui::Ui,
    key: &'static str,
    hidden: usize,
    budget: usize,
    offset: usize,
    noun: &str,
) {
    if hidden == 0 || budget == 0 {
        return;
    }
    let label = format!("Show the other {hidden} {noun}");
    if ui.button(&label).clicked() {
        ui.ctx()
            .data_mut(|data| data.insert_temp(egui::Id::new(key), offset + budget));
    }
}

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
    let window = stage_window(
        hunks,
        GIT_HUNK_CONTROL_LIMIT,
        window_offset(ui, HUNK_STAGED_WINDOW),
        window_offset(ui, HUNK_UNSTAGED_WINDOW),
        |hunk| hunk.stage == GitHunkStageProjection::Staged,
    );
    for hunk in &window.visible {
        // A dirty submodule has nothing to stage from here.
        //
        // Git synthesises a hunk for a gitlink whose worktree changed while its
        // recorded commit did not, and `git apply --cached` accepts it,
        // succeeds, and changes nothing -- so the row reappeared identically on
        // the next refresh and the control was a button that reported success
        // for doing nothing. The row stays, because the change is real and
        // hiding it would be its own lie; what it carries is the reason and the
        // next step instead of an action that cannot work.
        if hunk.submodule_dirty_only {
            ui.horizontal_wrapped(|ui| {
                ui.label(theme::muted(format!(
                    "{} · submodule has uncommitted changes; commit them inside the submodule \
                     before it can be staged here",
                    hunk.path
                )));
            });
            continue;
        }
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
    // The controls that move the window, so a hunk off this page is a click
    // away rather than out of reach. Without them the tail of a long staged list
    // could not be got to at all: no sequence of Stage and Unstage on the hunks
    // that *were* shown ever brought the last one into view.
    render_window_advance(
        ui,
        HUNK_STAGED_WINDOW,
        window.staged_hidden,
        window.staged_budget,
        window.staged_offset,
        "staged hunks",
    );
    render_window_advance(
        ui,
        HUNK_UNSTAGED_WINDOW,
        window.unstaged_hidden,
        window.unstaged_budget,
        window.unstaged_offset,
        "unstaged hunks",
    );
    // The count is stated only when it is knowable. The projection caps what it
    // collects, so a panel subtracting what it drew from what it received would
    // report "116 more" for a repository with sixteen thousand -- a precise
    // number that is precisely wrong. When the projection says it truncated,
    // the panel says there are more without pretending to know how many.
    if snapshot.git_projection.hunks_truncated {
        ui.label(theme::muted(
            "More hunks than can be listed; refine the change set to review the rest",
        ));
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
    let (mut candidates, withheld): (Vec<_>, Vec<_>) = eligible
        .into_iter()
        .partition(|file| path_control_is_safe(file, truncated));
    // Unstaged first, so the window moves as work is done.
    //
    // A staged file keeps its row (it now offers Unstage), so a plain
    // first-twelve prefix was permanent: with thirteen untracked files, staging
    // the visible twelve left the thirteenth hidden forever and the only way to
    // reach it was git directly. Ordering by what still needs staging means each
    // Stage click frees a slot for the next file that does.
    //
    // The budget is split rather than a plain prefix, for the same reason the
    // hunk controls above split theirs: ordering alone fixes only one direction.
    // With more than twelve *staged* hunkless files and nothing unstaged, a
    // prefix over the sorted list would show twelve Unstage controls and hide
    // the rest with no way to reach them -- the mirror image of the defect, and
    // just as permanent.
    candidates.sort_by_key(|file| status_is_committable(&file.status));

    if candidates.is_empty() && withheld.is_empty() {
        return;
    }
    let window = stage_window(
        &candidates,
        GIT_HUNK_CONTROL_LIMIT,
        window_offset(ui, PATH_STAGED_WINDOW),
        window_offset(ui, PATH_UNSTAGED_WINDOW),
        |file| status_is_committable(&file.status),
    );
    let hidden = window.staged_hidden + window.unstaged_hidden;

    // Only when something is under it. With every eligible file withheld -- a
    // few renames, an untracked directory -- a bare "Files" header above the
    // explanation reads as a section that failed to load.
    if !window.visible.is_empty() {
        super::components::section_header(ui, "Files", Some(theme::tokens().accent.cyan));
    }
    for file in window.visible.iter().copied() {
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
    // And a way to get to them. Saying a thirteenth file exists while offering
    // no route to it is a more honest version of the same defect, not a fix:
    // with thirteen staged files and nothing unstaged, unstaging a visible one
    // only rebalanced the budgets and the thirteenth stayed hidden, so the note
    // named a file the panel could never show.
    render_window_advance(
        ui,
        PATH_STAGED_WINDOW,
        window.staged_hidden,
        window.staged_budget,
        window.staged_offset,
        "staged files",
    );
    render_window_advance(
        ui,
        PATH_UNSTAGED_WINDOW,
        window.unstaged_hidden,
        window.unstaged_budget,
        window.unstaged_offset,
        "unstaged files",
    );
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
        || git.refresh_state != GitRefreshState::Idle
    {
        rows.push(format!(
            "git: refresh={:?} branch={} head={} changes={} hunks={} conflicts={} worktrees={}",
            git.refresh_state,
            git.branch_label.as_deref().unwrap_or("<none>"),
            git.head_short.as_deref().unwrap_or("<none>"),
            git.changed_files.len(),
            git.hunks.len(),
            git.conflicts.len(),
            git.worktrees.len()
        ));
    }
    let mut grouped_files = std::collections::BTreeMap::<String, Vec<_>>::new();
    for file in git.changed_files.iter().take(16) {
        let group = file
            .path
            .rsplit_once('/')
            .map(|(directory, _)| directory.to_string())
            .unwrap_or_else(|| "<root>".to_string());
        grouped_files.entry(group).or_default().push(file);
    }
    for (group, files) in grouped_files {
        rows.push(format!("git group {group}"));
        rows.extend(files.into_iter().map(|file| {
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
    }
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

#[cfg(test)]
mod stage_window_rules {
    use super::{GIT_HUNK_CONTROL_LIMIT, stage_window};

    /// `(id, staged)` — the least a windowed item has to be.
    fn items(staged: usize, unstaged: usize) -> Vec<(usize, bool)> {
        // Unstaged first, the order the projection produces and the order that
        // made a plain prefix hide every Unstage control.
        (0..unstaged)
            .map(|index| (index, false))
            .chain((0..staged).map(|index| (index + unstaged, true)))
            .collect()
    }

    /// Advancing the window reaches every item, one page at a time.
    ///
    /// The property the cap alone could not provide. Thirteen staged items and
    /// nothing unstaged showed the first twelve, and no sequence of the controls
    /// on screen brought the thirteenth into view: unstaging a visible one made
    /// it twelve staged and one unstaged, whose budgets show eleven staged, so
    /// the last one stayed hidden and restaging returned to the start.
    #[test]
    fn every_item_is_reachable_by_advancing_the_window() {
        for (staged, unstaged) in [(13, 0), (0, 13), (13, 13), (25, 1), (1, 25)] {
            let items = items(staged, unstaged);
            let mut seen: std::collections::BTreeSet<usize> = std::collections::BTreeSet::new();
            let mut staged_offset = 0usize;
            let mut unstaged_offset = 0usize;

            // Bounded: if the window does not cover the list in this many
            // advances it is not converging, and looping forever would hide
            // that as a hang rather than report it as a failure.
            for _ in 0..(items.len() + 2) {
                let window = stage_window(
                    &items,
                    GIT_HUNK_CONTROL_LIMIT,
                    staged_offset,
                    unstaged_offset,
                    |item: &(usize, bool)| item.1,
                );
                for item in &window.visible {
                    seen.insert(item.0);
                }
                if window.staged_hidden == 0 && window.unstaged_hidden == 0 {
                    break;
                }
                if window.staged_hidden > 0 {
                    staged_offset += window.staged_budget;
                }
                if window.unstaged_hidden > 0 {
                    unstaged_offset += window.unstaged_budget;
                }
            }

            let missing: Vec<usize> = items
                .iter()
                .map(|item| item.0)
                .filter(|id| !seen.contains(id))
                .collect();
            assert!(
                missing.is_empty(),
                "with {staged} staged and {unstaged} unstaged, advancing the window never \
                 showed {missing:?} — those items have controls that cannot be reached at all"
            );
        }
    }

    /// A page never exceeds the limit, however far it has been advanced.
    #[test]
    fn a_page_never_exceeds_the_control_limit() {
        for (staged, unstaged) in [(13, 0), (0, 13), (13, 13), (40, 40)] {
            let items = items(staged, unstaged);
            for offset in 0..items.len() {
                let window = stage_window(
                    &items,
                    GIT_HUNK_CONTROL_LIMIT,
                    offset,
                    offset,
                    |item: &(usize, bool)| item.1,
                );
                assert!(
                    window.visible.len() <= GIT_HUNK_CONTROL_LIMIT,
                    "{} controls drawn at offset {offset} with {staged}/{unstaged}, over a \
                     limit of {GIT_HUNK_CONTROL_LIMIT}",
                    window.visible.len()
                );
            }
        }
    }

    /// Both kinds get controls when both exist.
    ///
    /// The reason the budget is split per stage rather than taken as a prefix:
    /// the projection lists every unstaged item first, so a prefix renders no
    /// Unstage control at all once the limit fills with unstaged ones.
    #[test]
    fn each_stage_gets_controls_when_both_are_present() {
        let items = items(5, 20);
        let window = stage_window(
            &items,
            GIT_HUNK_CONTROL_LIMIT,
            0,
            0,
            |item: &(usize, bool)| item.1,
        );
        assert!(
            window.visible.iter().any(|item| item.1),
            "twenty unstaged items filled the budget and the five staged ones got no controls"
        );
        assert!(
            window.visible.iter().any(|item| !item.1),
            "the unstaged items got no controls"
        );
    }

    /// The page reports the offset it actually used, not the one it was handed.
    ///
    /// The wrap is local to the call, so a caller advancing from the *stored*
    /// value kept adding a budget to an offset the window had already
    /// discarded. The 13-staged flow does exactly that: page to the last hunk
    /// and unstage it, and the stored 12 outlives the list it indexed into --
    /// every later click re-renders the first page and the final Unstage
    /// control cannot be reached again.
    #[test]
    fn the_page_reports_the_offset_it_used() {
        let items = items(12, 0);
        let window = stage_window(
            &items,
            GIT_HUNK_CONTROL_LIMIT,
            12,
            0,
            |item: &(usize, bool)| item.1,
        );

        assert_eq!(
            window.staged_offset, 0,
            "an offset at or past the end wrapped to the first page, so that is the offset \
             the next advance must count from"
        );

        // And the other half, without which reporting a constant zero passes:
        // an offset the window actually used must come back unchanged, or every
        // advance would restart from the first page.
        // Built inline: the local binding above shadows the `items` helper.
        let deeper: Vec<(usize, bool)> = (0..25).map(|index| (index, true)).collect();
        let paged = stage_window(
            &deeper,
            GIT_HUNK_CONTROL_LIMIT,
            12,
            0,
            |item: &(usize, bool)| item.1,
        );
        assert_eq!(
            paged.staged_offset, 12,
            "an in-range offset was not reported back, so advancing from it would count from \
             somewhere the page never was"
        );
        assert!(
            paged.visible.iter().all(|item| item.0 >= 12),
            "the reported offset does not describe the page that was actually built"
        );

        // Advancing from what the page reports reaches the tail; advancing from
        // a stored offset that had already wrapped would ask for the first page
        // again, forever.
        //
        // This runs against the 25-item list, not the 12-item one. Against
        // twelve, `0 + 12` wraps back to zero and the page is non-empty either
        // way -- a wrap-to-zero assertion wearing the name of a different
        // property, which is the case the first assertion above already covers.
        let tail = stage_window(
            &deeper,
            GIT_HUNK_CONTROL_LIMIT,
            paged.staged_offset + paged.staged_budget,
            0,
            |item: &(usize, bool)| item.1,
        );
        assert!(
            tail.visible.iter().any(|item| item.0 >= 24),
            "advancing twice never reached the last of twenty-five items, so the tail of a              long list is still unreachable; page showed {:?}",
            tail.visible.iter().map(|item| item.0).collect::<Vec<_>>()
        );
    }

    /// An offset past the end shows the first page, not an empty one.
    #[test]
    fn an_offset_past_the_end_returns_to_the_start() {
        let items = items(3, 0);
        let window = stage_window(
            &items,
            GIT_HUNK_CONTROL_LIMIT,
            99,
            99,
            |item: &(usize, bool)| item.1,
        );
        assert_eq!(
            window.visible.len(),
            3,
            "advancing past the end emptied the list, so the control that moves the window \
             can lose the window"
        );
    }
}

#[cfg(test)]
mod grouped_rows {
    use super::git_rows;
    use legion_ui::{GitDiffStrategyProjection, GitFileProjection, Shell};

    #[test]
    fn git_rows_group_changed_files_without_dropping_file_details() {
        let mut snapshot = Shell::empty("grouped git rows").projection_snapshot();
        let file = |path: &str| GitFileProjection {
            path: path.to_string(),
            status: " M".to_string(),
            inserted_lines: 1,
            deleted_lines: 0,
            unstaged_hunk_count: 1,
            staged_hunk_count: 0,
            stageable: true,
            diff_strategy: GitDiffStrategyProjection::Syntactic,
            fallback_reason: None,
            conflict: false,
        };
        snapshot.git_projection.changed_files = vec![
            file("src/lib.rs"),
            file("src/bin/main.rs"),
            file("README.md"),
        ];

        let rows = git_rows(&snapshot);
        assert!(rows.iter().any(|row| row == "git group src"));
        assert!(rows.iter().any(|row| row == "git group src/bin"));
        assert!(rows.iter().any(|row| row == "git group <root>"));
        for path in ["src/lib.rs", "src/bin/main.rs", "README.md"] {
            assert!(
                rows.iter().any(|row| row.contains(path)),
                "grouped Git rows dropped {path}: {rows:?}"
            );
        }
    }
}
