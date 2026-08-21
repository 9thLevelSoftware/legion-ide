//! Checklist row 8: does the Source Control panel show a repository's state,
//! and can a person act on it?
//!
//! Never exercised in a windowed session. The row reads "Git panel opens /
//! status rows", and both halves of it were wrong against a real repository:
//!
//! * **The rows were not there.** `GitProjection` is populated *only* by an
//!   explicit `RefreshGit` command. Nothing issued one on workspace open or on
//!   selecting the surface, so opening Source Control in a repository with three
//!   changed files rendered the empty state, "No source-control status". The
//!   remote verbs are gated on a projected branch label, so they were absent
//!   too: the whole panel was one Refresh button.
//!
//! * **There was nothing to act with.** `DesktopAction::StageGitHunk` and
//!   `UnstageGitHunk` reach `git apply --cached` through app authority, and no
//!   rendered control pushed either. There was no commit control at all. The
//!   panel offered Fetch, Pull, Push and Open PR — every verb that talks to a
//!   remote — and no way to put a line in the index or commit it.
//!
//! These tests drive the rendered UI and then ask **git**, not the projection,
//! whether the index changed. A panel that reports staging it did not do is the
//! same defect shape as a terminal that reports running a command it never sent.

use std::path::Path;
use std::process::Command;

mod common;
use common::{TempWorkspace, click_at, clickable_center, full_frame_input, rendered_text};

use legion_desktop::workflow::{DesktopEframeApp, DesktopLaunchConfig, DesktopRuntime};

/// Run git in `root`, failing loudly with stderr rather than silently.
fn git(root: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .args(args)
        .current_dir(root)
        .output()
        .unwrap_or_else(|error| panic!("git {args:?} could not be run: {error}"));
    assert!(
        output.status.success(),
        "git {args:?} failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).to_string()
}

/// A repository with one commit, an identity, and a deterministic branch name.
///
/// `init.defaultBranch` differs between git versions and user configs, so the
/// branch is renamed rather than assumed; a test that hard-codes `master` fails
/// on a machine that defaults to `main` for reasons that have nothing to do with
/// the code under test.
fn init_repo(workspace: &TempWorkspace) {
    let root = workspace.path();
    git(root, &["init"]);
    git(root, &["config", "user.email", "dogfood@example.invalid"]);
    git(root, &["config", "user.name", "Dogfood Tester"]);
    git(root, &["config", "commit.gpgsign", "false"]);
    workspace.write("tracked.rs", "fn main() {}\n");
    git(root, &["add", "tracked.rs"]);
    git(root, &["commit", "-m", "initial"]);
    git(root, &["branch", "-M", "trunk"]);
}

fn open_app(root: &Path) -> DesktopEframeApp {
    let runtime = DesktopRuntime::open(DesktopLaunchConfig::new(root.to_path_buf(), None))
        .expect("desktop runtime should open workspace");
    DesktopEframeApp::new(runtime)
}

/// Click the Source Control rail control and return the settled frame.
fn open_source_control(app: &mut DesktopEframeApp) -> egui::FullOutput {
    let primed = app.run_headless_full_frame(full_frame_input(Vec::new()));
    let rail = clickable_center(&primed, "Source Control")
        .unwrap_or_else(|| panic!("the Source Control rail control must exist to reach the panel"));
    click_at(app, rail)
}

/// Paths git reports as staged, straight from the index.
fn staged_paths(root: &Path) -> Vec<String> {
    git(root, &["diff", "--cached", "--name-only"])
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(str::to_string)
        .collect()
}

/// The accessible label the panel gives the stage/unstage control for a hunk.
///
/// Built from the projection rather than guessed, so a test cannot pass by
/// clicking some *other* hunk's button that happens to sit at the same place.
fn hunk_control_label(app: &DesktopEframeApp, path: &str, staged: bool) -> String {
    let snapshot = app.runtime_snapshot();
    let hunk = snapshot
        .git_projection
        .hunks
        .iter()
        .find(|hunk| {
            hunk.path == path && (hunk.stage == legion_ui::GitHunkStageProjection::Staged) == staged
        })
        .unwrap_or_else(|| {
            panic!(
                "no {} hunk projected for `{path}`; projected hunks were {:?}",
                if staged { "staged" } else { "unstaged" },
                snapshot
                    .git_projection
                    .hunks
                    .iter()
                    .map(|hunk| (hunk.path.clone(), hunk.stage))
                    .collect::<Vec<_>>()
            )
        });
    let verb = if staged { "Unstage" } else { "Stage" };
    format!("{verb} {} {}", hunk.path, hunk.header)
}

/// Selecting Source Control must show the repository, not an empty state.
///
/// This is checklist row 8 itself. It asserts on the rendered text a person
/// reads, because the defect was invisible to every projection test: the
/// projection was correct whenever anyone asked it, and the panel never asked.
#[test]
fn opening_source_control_shows_the_repository_status() {
    let workspace = TempWorkspace::new("legion_desktop_source_control");
    init_repo(&workspace);
    workspace.write("tracked.rs", "fn main() { let answer = 42; }\n");
    workspace.write("added.rs", "fn added() {}\n");
    git(workspace.path(), &["add", "added.rs"]);
    workspace.write("untracked.rs", "fn untracked() {}\n");

    let mut app = open_app(workspace.path());
    let panel = open_source_control(&mut app);
    let text = rendered_text(&panel).join("\n");

    // Prove the click landed before asserting anything about content: a test
    // that would also pass with the pointer three pixels off the button is not
    // testing the panel.
    assert!(
        text.contains("SOURCE CONTROL"),
        "the Source Control surface did not open; rendered text was:\n{text}"
    );
    assert!(
        !text.contains("No source-control status"),
        "Source Control rendered its empty state for a repository with three \
         changed files. That is a panel telling the user a dirty repository is \
         clean.\n{text}"
    );

    for expected in ["tracked.rs", "added.rs", "untracked.rs", "trunk"] {
        assert!(
            text.contains(expected),
            "the Source Control panel never mentions `{expected}`, which git \
             reports as part of this repository's state.\n{text}"
        );
    }
}

/// The remote verbs are only rendered once a branch is projected.
///
/// They are gated on `branch_label.is_some()`, so before the refresh landed the
/// panel could not offer Fetch, Pull or Push at all — in a repository sitting on
/// a branch with a remote configured.
#[test]
fn opening_source_control_reveals_the_remote_verbs() {
    let workspace = TempWorkspace::new("legion_desktop_source_control_remote");
    init_repo(&workspace);
    git(
        workspace.path(),
        &[
            "remote",
            "add",
            "origin",
            "https://github.com/example/example.git",
        ],
    );

    let mut app = open_app(workspace.path());
    let panel = open_source_control(&mut app);

    let mut missing = Vec::new();
    for verb in ["Refresh Git", "Fetch", "Pull", "Push", "Open PR"] {
        if clickable_center(&panel, verb).is_none() {
            missing.push(verb);
        }
    }
    assert!(
        missing.is_empty(),
        "source-control verbs absent after opening the panel on a branch with a \
         remote: {missing:?}. They are gated on a projected branch label, so an \
         unrefreshed projection hides every one of them."
    );
}

/// Clicking Stage must move the hunk into git's index.
///
/// Asserted against `git diff --cached`, not against the projection: the
/// projection is rebuilt from git after the action, so a projection-only
/// assertion would be circular if the action were dropped and the refresh
/// happened to run anyway.
#[test]
fn staging_a_hunk_from_the_panel_reaches_the_index() {
    let workspace = TempWorkspace::new("legion_desktop_source_control_stage");
    init_repo(&workspace);
    workspace.write("tracked.rs", "fn main() { let answer = 42; }\n");
    let root = workspace.path().to_path_buf();

    let mut app = open_app(&root);
    let panel = open_source_control(&mut app);

    assert!(
        staged_paths(&root).is_empty(),
        "fixture is wrong: something was already staged before the click"
    );

    let label = hunk_control_label(&app, "tracked.rs", false);
    let control = clickable_center(&panel, &label).unwrap_or_else(|| {
        panic!(
            "the Source Control panel offers no `{label}` control. A panel that \
             shows a modified file and cannot stage it can push, but only what \
             some other tool staged."
        )
    });
    let after = click_at(&mut app, control);

    assert_eq!(
        staged_paths(&root),
        vec!["tracked.rs".to_string()],
        "clicking Stage did not put the hunk in git's index"
    );
    // And the panel has caught up: the same hunk is now offered for unstaging.
    let text = rendered_text(&after).join("\n");
    assert!(
        text.contains("Unstage tracked.rs"),
        "the panel still offers to stage a hunk it just staged.\n{text}"
    );
}

/// Clicking Unstage must take the hunk back out of the index.
#[test]
fn unstaging_a_hunk_from_the_panel_reaches_the_index() {
    let workspace = TempWorkspace::new("legion_desktop_source_control_unstage");
    init_repo(&workspace);
    workspace.write("tracked.rs", "fn main() { let answer = 42; }\n");
    let root = workspace.path().to_path_buf();
    git(&root, &["add", "tracked.rs"]);

    let mut app = open_app(&root);
    let panel = open_source_control(&mut app);

    assert_eq!(
        staged_paths(&root),
        vec!["tracked.rs".to_string()],
        "fixture is wrong: the hunk should start staged"
    );

    let label = hunk_control_label(&app, "tracked.rs", true);
    let control = clickable_center(&panel, &label)
        .unwrap_or_else(|| panic!("the Source Control panel offers no `{label}` control"));
    let _ = click_at(&mut app, control);

    assert!(
        staged_paths(&root).is_empty(),
        "clicking Unstage left the hunk in git's index: {:?}",
        staged_paths(&root)
    );
}

/// Commit is offered only when the index has something in it.
///
/// `commit_git_changes` fails on an empty index, so an always-live Commit button
/// is a button whose usual outcome is an error. This pins the gate in both
/// directions in one test, so it cannot pass by the control never appearing.
#[test]
fn commit_is_offered_exactly_when_something_is_staged() {
    let workspace = TempWorkspace::new("legion_desktop_source_control_commit_gate");
    init_repo(&workspace);
    workspace.write("tracked.rs", "fn main() { let answer = 42; }\n");
    let root = workspace.path().to_path_buf();

    let mut app = open_app(&root);
    let dirty_only = open_source_control(&mut app);
    assert!(
        clickable_center(&dirty_only, "Commit…").is_none(),
        "Commit was offered with nothing staged; committing an empty index fails"
    );

    git(&root, &["add", "tracked.rs"]);
    let mut app = open_app(&root);
    let staged = open_source_control(&mut app);
    assert!(
        clickable_center(&staged, "Commit…").is_some(),
        "Commit was not offered even though a hunk is staged"
    );
}

/// An untracked file can be staged from the panel.
///
/// This test used to assert the opposite half: that an untracked file had *no*
/// control and the panel explained why, because staging went through `git apply
/// --cached` on a projected hunk and `git diff` emits nothing for a file git has
/// never seen. The explanation was honest and useless — it told you to go and
/// use git.
///
/// Path-level staging removes the gap rather than describing it. The same
/// `git add -- <path>` reaches untracked files, modified binaries, mode-only
/// changes and pure renames, none of which produce a hunk. Kept as a test
/// because an affordance that was once missing is exactly the kind that rots
/// back.
#[test]
fn an_untracked_file_can_be_staged_from_the_panel() {
    let workspace = TempWorkspace::new("legion_desktop_source_control_untracked");
    init_repo(&workspace);
    workspace.write("brand-new.rs", "fn brand_new() {}\n");
    let root = workspace.path().to_path_buf();

    let mut app = open_app(&root);
    let panel = open_source_control(&mut app);

    assert!(
        rendered_text(&panel)
            .iter()
            .any(|text| text.contains("brand-new.rs")),
        "the panel does not show the untracked file at all"
    );
    // Non-vacuity: an untracked file must still project no hunk, or this is
    // exercising the ordinary hunk path rather than the gap it was written for.
    assert!(
        app.runtime_snapshot()
            .git_projection
            .hunks
            .iter()
            .all(|hunk| hunk.path != "brand-new.rs"),
        "fixture assumption broken: an untracked file projected a hunk"
    );

    let stage = clickable_center(&panel, "Stage brand-new.rs").unwrap_or_else(|| {
        panic!(
            "an untracked file must offer a Stage control; frame showed {:?}",
            rendered_text(&panel)
        )
    });
    let _ = click_at(&mut app, stage);

    assert!(
        staged_paths(&root)
            .iter()
            .any(|path| path.contains("brand-new.rs")),
        "clicking Stage did not add the untracked file to the index; staged: {:?}",
        staged_paths(&root)
    );
}

/// Clicking Commit and typing a message must produce a real commit.
///
/// End-to-end through the rendered UI: the panel hands the user to the palette
/// (which owns the text field and the `git-commit` operand parser), the message
/// is typed as key events, and `git log` — not the projection — is asked whether
/// a commit exists.
#[test]
fn committing_from_the_panel_creates_a_real_commit() {
    let workspace = TempWorkspace::new("legion_desktop_source_control_commit");
    init_repo(&workspace);
    workspace.write("tracked.rs", "fn main() { let answer = 42; }\n");
    let root = workspace.path().to_path_buf();
    git(&root, &["add", "tracked.rs"]);

    let mut app = open_app(&root);
    let panel = open_source_control(&mut app);
    let commit = clickable_center(&panel, "Commit…")
        .expect("the Source Control panel must offer a commit control");
    let _ = click_at(&mut app, commit);

    // Prove the click landed: the palette is open on the commit command with the
    // operand prefix already typed, so the user only supplies the message.
    let palette = app.runtime_snapshot().palette_projection;
    assert!(palette.open, "clicking Commit did not open the commit flow");
    assert_eq!(
        palette.query, ">git commit ",
        "the commit control opened the palette without the commit operand ready"
    );

    let message = "answer the question";
    app.run_headless_full_frame(full_frame_input(vec![egui::Event::Text(
        message.to_string(),
    )]));
    assert_eq!(
        app.runtime_snapshot().palette_projection.query,
        format!(">git commit {message}"),
        "the typed commit message never reached the palette query"
    );

    app.run_headless_full_frame(full_frame_input(vec![egui::Event::Key {
        key: egui::Key::Enter,
        physical_key: Some(egui::Key::Enter),
        pressed: true,
        repeat: false,
        modifiers: egui::Modifiers::default(),
    }]));
    app.run_headless_full_frame(full_frame_input(Vec::new()));

    assert_eq!(
        git(&root, &["log", "-1", "--pretty=%s"]).trim(),
        message,
        "no commit with the typed message exists; git log is the only witness \
         that matters here"
    );
    assert!(
        staged_paths(&root).is_empty(),
        "the commit left changes in the index: {:?}",
        staged_paths(&root)
    );
}

/// A staged change with no `@@` hunk must still offer Commit.
///
/// The gate used to count staged hunks. A staged binary modification, an
/// empty-file addition, a mode-only change and a pure rename all appear in
/// porcelain status with a staged index column and produce no hunk at all, so
/// the panel's only Commit control disappeared while `git commit` would have
/// succeeded. Binary is the cleanest of those to build deterministically.
#[test]
fn commit_is_offered_for_a_staged_change_that_has_no_hunks() {
    let workspace = TempWorkspace::new("legion_desktop_source_control_binary");
    init_repo(&workspace);
    let root = workspace.path();

    // A byte sequence git treats as binary: a NUL inside the first 8000 bytes.
    std::fs::write(root.join("blob.bin"), [0u8, 1, 2, 3, 0, 255, 7, 9])
        .expect("write a binary file");
    git(root, &["add", "blob.bin"]);

    let staged = git(root, &["diff", "--cached", "--stat"]);
    assert!(
        staged.contains("blob.bin"),
        "the fixture must actually stage the binary, got: {staged:?}"
    );
    let hunks = git(root, &["diff", "--cached", "-U0"]);
    assert!(
        !hunks.contains("@@"),
        "the fixture is only meaningful if git emits no hunk for it, got: {hunks:?}"
    );

    let mut app = open_app(root);
    let panel = open_source_control(&mut app);
    assert!(
        clickable_center(&panel, "Commit…").is_some(),
        "a staged binary produces no hunk, so a hunk-counting gate hides Commit \
         even though the commit would succeed. Panel showed: {:?}",
        rendered_text(&panel)
    );
}

/// Hunks past the control budget are reachable, not merely announced.
///
/// This used to assert the panel said "N more hunks not shown". It did say it,
/// and the sentence was true, and there was no way to see them: no sequence of
/// Stage and Unstage on the hunks that *were* drawn ever brought the rest into
/// view. A note naming what you cannot reach is a more honest version of the
/// same defect, not a fix — so the assertion is now that the panel offers a
/// route and that taking it shows hunks the first page did not.
#[test]
fn hunks_beyond_the_control_limit_are_reachable_rather_than_dropped() {
    let workspace = TempWorkspace::new("legion_desktop_source_control_overflow");
    init_repo(&workspace);
    let root = workspace.path();

    // Widely separated single-line edits so git cannot coalesce them: each
    // becomes its own hunk. 40 lines apart, 20 edits, against a 12-row budget.
    let mut original = String::new();
    for line in 0..1_000 {
        original.push_str(&format!("line {line}\n"));
    }
    std::fs::write(root.join("wide.txt"), &original).expect("seed a long file");
    git(root, &["add", "wide.txt"]);
    git(root, &["commit", "-m", "seed wide"]);

    let mut edited = String::new();
    for line in 0..1_000 {
        if line % 40 == 0 && line > 0 {
            edited.push_str(&format!("line {line} CHANGED\n"));
        } else {
            edited.push_str(&format!("line {line}\n"));
        }
    }
    std::fs::write(root.join("wide.txt"), &edited).expect("edit the long file");

    let mut app = open_app(root);
    let panel = open_source_control(&mut app);

    /// The hunk headers a frame is showing.
    fn hunk_headers(frame: &egui::FullOutput) -> std::collections::BTreeSet<String> {
        rendered_text(frame)
            .into_iter()
            .filter(|line| line.contains("@@"))
            .collect()
    }

    let first_page = hunk_headers(&panel);
    assert!(
        first_page.len() >= 2,
        "the fixture must produce a full page of hunks, got {first_page:?}"
    );

    let advance = rendered_text(&panel)
        .into_iter()
        .find(|line| line.starts_with("Show the other") && line.contains("hunks"))
        .expect(
            "with more hunks than the budget, the panel must offer a way to reach the rest; \
             without one they cannot be staged from this surface at all",
        );
    let control = clickable_center(&panel, &advance)
        .expect("the control naming the hidden hunks must be clickable");
    let advanced = click_at(&mut app, control);
    let second_page = hunk_headers(&advanced);

    let newly_visible: Vec<&String> = second_page.difference(&first_page).collect();
    assert!(
        !newly_visible.is_empty(),
        "advancing the window showed no hunk that was not already on screen, so the control \
         announces hunks it cannot actually reach. First page {first_page:?}, after {second_page:?}"
    );
}

/// An unresolved merge must not offer Commit.
///
/// Porcelain marks conflicts with pairs like `UU` and `AA`, whose index column
/// is neither a space nor `?` — so an index-column gate counts them as
/// committable while `git commit` refuses an unmerged index. The button's only
/// possible outcome would be an error, and for a binary or delete/modify
/// conflict nothing else on the panel hints at why.
#[test]
fn an_unresolved_merge_does_not_offer_commit() {
    let workspace = TempWorkspace::new("legion_desktop_source_control_conflict");
    init_repo(&workspace);
    let root = workspace.path();

    workspace.write("shared.txt", "base\n");
    git(root, &["add", "shared.txt"]);
    git(root, &["commit", "-m", "base"]);

    git(root, &["checkout", "-b", "theirs"]);
    workspace.write("shared.txt", "theirs\n");
    git(root, &["commit", "-am", "theirs"]);

    git(root, &["checkout", "trunk"]);
    workspace.write("shared.txt", "ours\n");
    git(root, &["commit", "-am", "ours"]);

    // A conflicting merge: `git merge` exits non-zero, so it cannot go through
    // the asserting `git` helper.
    let merged = std::process::Command::new("git")
        .args(["merge", "theirs"])
        .current_dir(root)
        .output()
        .expect("git merge should run");
    assert!(
        !merged.status.success(),
        "the fixture must actually conflict for this test to mean anything"
    );
    let status = git(root, &["status", "--porcelain"]);
    assert!(
        status.contains("UU") || status.contains("AA"),
        "the fixture must leave an unmerged porcelain status, got {status:?}"
    );

    let mut app = open_app(root);
    let panel = open_source_control(&mut app);
    assert!(
        clickable_center(&panel, "Commit…").is_none(),
        "an unmerged index cannot be committed, so offering Commit gives a button whose only outcome is an error. Panel showed: {:?}",
        rendered_text(&panel)
    );
}

/// Staged hunks stay reachable when unstaged hunks fill the control budget.
///
/// The projection appends every unstaged hunk before any staged one, so taking
/// a combined prefix rendered no Unstage control at all once twelve unstaged
/// hunks existed — forcing someone to stage unrelated changes before the hunk
/// they wanted to unstage became reachable.
#[test]
fn staged_hunks_remain_reachable_behind_many_unstaged_ones() {
    let workspace = TempWorkspace::new("legion_desktop_source_control_budget");
    init_repo(&workspace);
    let root = workspace.path();

    let mut seed = String::new();
    for line in 0..1_000 {
        seed.push_str(&format!("line {line}\n"));
    }
    std::fs::write(root.join("wide.txt"), &seed).expect("seed a long file");
    std::fs::write(root.join("staged.txt"), "one\n").expect("seed a second file");
    git(root, &["add", "wide.txt", "staged.txt"]);
    git(root, &["commit", "-m", "seed"]);

    // One staged hunk in its own file.
    std::fs::write(root.join("staged.txt"), "one changed\n").expect("edit the staged file");
    git(root, &["add", "staged.txt"]);

    // Far more unstaged hunks than the control budget, in another file.
    let mut edited = String::new();
    for line in 0..1_000 {
        if line % 40 == 0 && line > 0 {
            edited.push_str(&format!("line {line} CHANGED\n"));
        } else {
            edited.push_str(&format!("line {line}\n"));
        }
    }
    std::fs::write(root.join("wide.txt"), &edited).expect("edit the wide file");

    let mut app = open_app(root);
    let panel = open_source_control(&mut app);
    let text = rendered_text(&panel).join("\n");
    assert!(
        text.contains("Unstage staged.txt"),
        "the staged hunk must stay reachable behind the unstaged ones. Panel showed: {text}"
    );
}

/// A conflict blocks Commit even when something else is independently staged.
///
/// The first version of this guard tested each entry, so a repository mid-merge
/// that also held a staged file answered "yes, something is committable" and
/// offered Commit again — for exactly the case the guard was added to prevent.
/// `git commit` refuses while *any* path is unmerged, so the veto is on the
/// projection, not the entry.
#[test]
fn a_conflict_blocks_commit_even_with_another_file_staged() {
    let workspace = TempWorkspace::new("legion_desktop_source_control_conflict_mixed");
    init_repo(&workspace);
    let root = workspace.path();

    workspace.write("shared.txt", "base\n");
    workspace.write("other.txt", "base\n");
    git(root, &["add", "shared.txt", "other.txt"]);
    git(root, &["commit", "-m", "base"]);

    git(root, &["checkout", "-b", "theirs"]);
    workspace.write("shared.txt", "theirs\n");
    git(root, &["commit", "-am", "theirs"]);

    git(root, &["checkout", "trunk"]);
    workspace.write("shared.txt", "ours\n");
    git(root, &["commit", "-am", "ours"]);

    let merged = std::process::Command::new("git")
        .args(["merge", "theirs"])
        .current_dir(root)
        .output()
        .expect("git merge should run");
    assert!(!merged.status.success(), "the fixture must conflict");

    // An unrelated file, staged cleanly, alongside the conflict.
    workspace.write("other.txt", "independently staged\n");
    git(root, &["add", "other.txt"]);
    let status = git(root, &["status", "--porcelain"]);
    assert!(
        (status.contains("UU") || status.contains("AA")) && status.contains("M  other.txt"),
        "the fixture needs both an unmerged path and a cleanly staged one, got {status:?}"
    );

    let mut app = open_app(root);
    let panel = open_source_control(&mut app);
    assert!(
        clickable_center(&panel, "Commit…").is_none(),
        "git refuses every commit while any path is unmerged, so a cleanly staged file alongside a conflict must not re-enable Commit. Panel showed: {:?}",
        rendered_text(&panel)
    );
}

/// Resolving the last conflict must not take away the control that finishes it.
///
/// **Use Current** on the final conflict can leave the index byte-identical to
/// `HEAD` -- it does whenever the current side is the one that never changed.
/// Porcelain status then reports nothing at all, while `MERGE_HEAD` is still on
/// disk and `git commit` would succeed and conclude the merge.
///
/// A Commit gate reading changed files alone therefore withdrew the panel's only
/// Commit control in direct response to the panel's own conflict action, leaving
/// the repository mid-merge with no way to finish from that surface. Worse than
/// a missing feature: the panel walks you into the state and then removes the
/// exit.
///
/// The merge here is set up with git rather than through the conflict buttons
/// because the property under test is the gate, not the resolution controls, and
/// `git checkout --ours` is exactly what **Use Current** performs.
#[test]
fn commit_survives_a_merge_resolved_to_the_current_side() {
    let workspace = TempWorkspace::new("legion_desktop_source_control_empty_merge");
    init_repo(&workspace);
    let root = workspace.path().to_path_buf();

    workspace.write("shared.rs", "fn shared() { let value = 0; }\n");
    git(&root, &["add", "shared.rs"]);
    git(&root, &["commit", "-m", "base"]);
    let base = git(&root, &["rev-parse", "HEAD"]).trim().to_string();

    workspace.write("shared.rs", "fn shared() { let value = 1; }\n");
    git(&root, &["add", "shared.rs"]);
    git(&root, &["commit", "-m", "ours"]);

    git(&root, &["checkout", "-b", "theirs", &base]);
    workspace.write("shared.rs", "fn shared() { let value = 2; }\n");
    git(&root, &["add", "shared.rs"]);
    git(&root, &["commit", "-m", "theirs"]);
    git(&root, &["checkout", "-"]);

    // Expected to conflict, so this one is not asserted successful.
    let _ = Command::new("git")
        .args(["merge", "theirs"])
        .current_dir(&root)
        .output()
        .expect("merge should run");

    // While conflicts remain, Commit must stay withdrawn: `git commit` refuses
    // an unmerged index, so offering it would be a button that only errors.
    let mut app = open_app(&root);
    let conflicted = open_source_control(&mut app);
    assert!(
        clickable_center(&conflicted, "Commit…").is_none(),
        "Commit was offered with the index unmerged, where git refuses it; frame showed {:?}",
        rendered_text(&conflicted).len()
    );

    // **Use Current**: keep our side. Ours is what HEAD already holds, so the
    // index now matches HEAD and status goes quiet.
    git(&root, &["checkout", "--ours", "shared.rs"]);
    git(&root, &["add", "shared.rs"]);
    assert!(
        staged_paths(&root).is_empty(),
        "the fixture must leave an empty index, or it is not testing this case; staged: {:?}",
        staged_paths(&root)
    );

    let mut app = open_app(&root);
    let resolved = open_source_control(&mut app);
    let commit = clickable_center(&resolved, "Commit…").unwrap_or_else(|| {
        panic!(
            "the merge is unfinished and `git commit` would conclude it, but the panel offers \
             no Commit control -- the surface that created this state has no way out of it. \
             Frame showed {:?}",
            rendered_text(&resolved)
        )
    });

    // And it has to actually work, not merely be present.
    let _ = click_at(&mut app, commit);
    let message = "conclude the merge";
    app.run_headless_full_frame(full_frame_input(vec![egui::Event::Text(
        message.to_string(),
    )]));
    app.run_headless_full_frame(full_frame_input(vec![egui::Event::Key {
        key: egui::Key::Enter,
        physical_key: Some(egui::Key::Enter),
        pressed: true,
        repeat: false,
        modifiers: egui::Modifiers::default(),
    }]));
    app.run_headless_full_frame(full_frame_input(Vec::new()));

    assert!(
        !root.join(".git").join("MERGE_HEAD").exists(),
        "the merge is still unfinished after committing from the panel"
    );
}

/// A change with no textual hunk must still be stageable from the panel.
///
/// `git diff` emits no `@@` hunk for a modified binary, so hunk controls cannot
/// reach it. Before path-level staging, such a file appeared in the status list
/// with no control beside it and the panel's commit flow was unusable for it
/// without dropping to a terminal — a gap the panel previously *explained*
/// (untracked files got a sentence saying to use git first) rather than closed.
///
/// Driven end to end and checked against git: the index is the witness, not the
/// projection.
#[test]
fn a_binary_change_can_be_staged_and_committed_from_the_panel() {
    let workspace = TempWorkspace::new("legion_desktop_source_control_binary");
    init_repo(&workspace);
    let root = workspace.path().to_path_buf();

    // A file git treats as binary: NUL bytes, no trailing newline convention.
    let binary_path = root.join("blob.bin");
    std::fs::write(&binary_path, [0u8, 1, 2, 0, 3, 4, 0, 5]).expect("write binary fixture");
    git(&root, &["add", "blob.bin"]);
    git(&root, &["commit", "-m", "add binary"]);

    // Modify it, so it is a tracked change with no textual hunk.
    std::fs::write(&binary_path, [0u8, 9, 9, 0, 9, 9, 0, 9]).expect("modify binary fixture");

    let mut app = open_app(&root);
    let panel = open_source_control(&mut app);

    // Non-vacuity: the fixture really must produce no hunk, or this test is
    // exercising the ordinary hunk path and proves nothing about the gap.
    let snapshot = app.runtime_snapshot();
    assert!(
        !snapshot
            .git_projection
            .hunks
            .iter()
            .any(|hunk| hunk.path.contains("blob.bin")),
        "the binary fixture produced a textual hunk, so this test no longer covers the \
         hunkless case; hunks were {:?}",
        snapshot
            .git_projection
            .hunks
            .iter()
            .map(|hunk| &hunk.path)
            .collect::<Vec<_>>()
    );

    let stage = clickable_center(&panel, "Stage blob.bin").unwrap_or_else(|| {
        panic!(
            "a changed binary file must offer a Stage control; frame showed {:?}",
            rendered_text(&panel)
        )
    });
    let _ = click_at(&mut app, stage);

    // git is the witness.
    assert!(
        staged_paths(&root)
            .iter()
            .any(|path| path.contains("blob.bin")),
        "clicking Stage did not put the binary change in the index; staged: {:?}",
        staged_paths(&root)
    );

    // And it commits, which is the point of staging it.
    let after = app.run_headless_full_frame(full_frame_input(Vec::new()));
    assert!(
        clickable_center(&after, "Commit…").is_some(),
        "Commit was not offered after staging a binary change"
    );
}
