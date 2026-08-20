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

/// An untracked file gets a row with no stage control, and the panel says why.
///
/// Staging goes through `git apply --cached` on a projected hunk, and `git diff`
/// emits nothing for a file git has never seen: an untracked file projects with
/// no hunks and so gets no button. That is a real gap — there is no path-level
/// `git add` authority in the app layer to reach for — but a row sitting next to
/// rows that *do* have buttons, with no explanation, reads as the panel being
/// broken. This pins both halves: no control, and a stated reason.
#[test]
fn an_untracked_file_is_explained_rather_than_silently_unstageable() {
    let workspace = TempWorkspace::new("legion_desktop_source_control_untracked");
    init_repo(&workspace);
    workspace.write("brand-new.rs", "fn brand_new() {}\n");
    let root = workspace.path().to_path_buf();

    let mut app = open_app(&root);
    let panel = open_source_control(&mut app);
    let text = rendered_text(&panel).join("\n");

    assert!(
        text.contains("brand-new.rs"),
        "the panel does not show the untracked file at all.\n{text}"
    );
    assert!(
        app.runtime_snapshot()
            .git_projection
            .hunks
            .iter()
            .all(|hunk| hunk.path != "brand-new.rs"),
        "fixture assumption broken: an untracked file projected a hunk, so the \
         note this test guards would be wrong"
    );
    assert!(
        text.contains("1 untracked file: add with git before staging here"),
        "the panel shows an untracked file with no stage control and no reason. \
         A row that silently has no button next to rows that do reads as a \
         broken panel.\n{text}"
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
