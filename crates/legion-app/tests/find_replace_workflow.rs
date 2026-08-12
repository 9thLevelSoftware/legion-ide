use std::sync::atomic::{AtomicU64, Ordering};

use legion_app::{AppCommandOutcome, AppComposition};
use legion_editor::{TextEdit, TextPosition};
use legion_protocol::{PrincipalId, WorkspaceTrustState};
use legion_ui::{CommandDispatchIntent, ShellLayoutProjection};

static TEMP_ROOT_COUNTER: AtomicU64 = AtomicU64::new(0);

struct TempWorkspace {
    root: std::path::PathBuf,
}

impl std::ops::Deref for TempWorkspace {
    type Target = std::path::Path;

    fn deref(&self) -> &std::path::Path {
        &self.root
    }
}

impl Drop for TempWorkspace {
    fn drop(&mut self) {
        let temp_root = std::env::temp_dir();
        let file_name = self.root.file_name().and_then(|name| name.to_str());
        if self.root.starts_with(&temp_root)
            && file_name.is_some_and(|name| name.starts_with("legion-find-replace-"))
        {
            let _ = std::fs::remove_dir_all(&self.root);
        }
    }
}

fn create_root() -> TempWorkspace {
    let root = std::env::temp_dir().join(format!(
        "legion-find-replace-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |value| value.as_millis() as u64)
            + TEMP_ROOT_COUNTER.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&root).expect("create temp root");
    TempWorkspace { root }
}

fn trusted_app(root: &std::path::Path) -> AppComposition {
    let mut app = AppComposition::new();
    app.open_workspace(
        root,
        WorkspaceTrustState::Trusted,
        PrincipalId("find-replace".to_string()),
    )
    .expect("open workspace");
    app
}

fn buffer_text(app: &mut AppComposition) -> String {
    app.active_buffer_projection(&ShellLayoutProjection::plain("find-replace"))
        .expect("active projection")
        .small_buffer_text()
        .expect("small buffer text")
        .to_string()
}

fn find_bar(app: &mut AppComposition) -> legion_ui::FindBarProjection {
    app.shell_projection_snapshot("find-replace")
        .expect("snapshot")
        .find_bar_projection
}

fn setup_with_content(content: &str) -> (TempWorkspace, AppComposition) {
    let root = create_root();
    let file = root.join("test.txt");
    std::fs::write(&file, content).expect("seed file");
    let mut app = trusted_app(&root);
    app.open_file(file.to_string_lossy()).expect("open file");
    (root, app)
}

#[test]
fn replace_one_substitutes_current_match() {
    let (_root, mut app) = setup_with_content("foo bar foo baz foo\n");

    app.dispatch_ui_intent(CommandDispatchIntent::ToggleFindBar)
        .expect("toggle find bar");
    app.dispatch_ui_intent(CommandDispatchIntent::SetFindQuery {
        query: "foo".into(),
    })
    .expect("set query");

    let fb = find_bar(&mut app);
    assert_eq!(fb.match_count, 3);

    app.dispatch_ui_intent(CommandDispatchIntent::ToggleFindReplace)
        .expect("toggle replace");
    app.dispatch_ui_intent(CommandDispatchIntent::SetFindReplaceText { text: "qux".into() })
        .expect("set replace text");

    app.dispatch_ui_intent(CommandDispatchIntent::ReplaceOne)
        .expect("replace one");

    let text = buffer_text(&mut app);
    assert_eq!(text, "qux bar foo baz foo\n");

    let fb = find_bar(&mut app);
    assert_eq!(
        fb.match_count, 2,
        "match count should drop by one after replace"
    );
}

#[test]
fn replace_one_refreshes_ranges_after_buffer_edit() {
    let (_root, mut app) = setup_with_content("foo bar foo\n");

    app.dispatch_ui_intent(CommandDispatchIntent::ToggleFindBar)
        .expect("toggle find bar");
    app.dispatch_ui_intent(CommandDispatchIntent::SetFindQuery {
        query: "foo".into(),
    })
    .expect("set query");
    app.dispatch_ui_intent(CommandDispatchIntent::ToggleFindReplace)
        .expect("toggle replace");
    app.dispatch_ui_intent(CommandDispatchIntent::SetFindReplaceText { text: "qux".into() })
        .expect("set replace text");

    app.edit_active_buffer(TextEdit::insert(TextPosition::new(0, 0), "prefix "))
        .expect("edit before replacing");
    app.dispatch_ui_intent(CommandDispatchIntent::ReplaceOne)
        .expect("replace one after edit");

    assert_eq!(buffer_text(&mut app), "prefix qux bar foo\n");
}

#[test]
fn replace_all_substitutes_every_match() {
    let (_root, mut app) = setup_with_content("aaa bbb aaa ccc aaa\n");

    app.dispatch_ui_intent(CommandDispatchIntent::ToggleFindBar)
        .expect("toggle find bar");
    app.dispatch_ui_intent(CommandDispatchIntent::SetFindQuery {
        query: "aaa".into(),
    })
    .expect("set query");
    app.dispatch_ui_intent(CommandDispatchIntent::ToggleFindReplace)
        .expect("toggle replace");
    app.dispatch_ui_intent(CommandDispatchIntent::SetFindReplaceText { text: "ZZ".into() })
        .expect("set replace text");

    let fb = find_bar(&mut app);
    assert_eq!(fb.match_count, 3);

    app.dispatch_ui_intent(CommandDispatchIntent::ReplaceAll)
        .expect("replace all");

    let text = buffer_text(&mut app);
    assert_eq!(text, "ZZ bbb ZZ ccc ZZ\n");

    let fb = find_bar(&mut app);
    assert_eq!(
        fb.match_count, 0,
        "no matches should remain after replace all"
    );
}

#[test]
fn replace_all_is_single_undo_group() {
    let (_root, mut app) = setup_with_content("x y x y x\n");

    app.dispatch_ui_intent(CommandDispatchIntent::ToggleFindBar)
        .expect("toggle find bar");
    app.dispatch_ui_intent(CommandDispatchIntent::SetFindQuery { query: "x".into() })
        .expect("set query");
    app.dispatch_ui_intent(CommandDispatchIntent::ToggleFindReplace)
        .expect("toggle replace");
    app.dispatch_ui_intent(CommandDispatchIntent::SetFindReplaceText { text: "W".into() })
        .expect("set replace text");

    app.dispatch_ui_intent(CommandDispatchIntent::ReplaceAll)
        .expect("replace all");

    let text = buffer_text(&mut app);
    assert_eq!(text, "W y W y W\n");

    let buffer_id = app.active_buffer_id().expect("active buffer");
    app.dispatch_ui_intent(CommandDispatchIntent::Undo { buffer_id })
        .expect("undo");

    let text = buffer_text(&mut app);
    assert_eq!(
        text, "x y x y x\n",
        "single undo should revert all replacements"
    );
}

#[test]
fn replace_one_no_match_is_noop() {
    let (_root, mut app) = setup_with_content("hello world\n");

    app.dispatch_ui_intent(CommandDispatchIntent::ToggleFindBar)
        .expect("toggle find bar");
    app.dispatch_ui_intent(CommandDispatchIntent::SetFindQuery {
        query: "missing".into(),
    })
    .expect("set query");
    app.dispatch_ui_intent(CommandDispatchIntent::ToggleFindReplace)
        .expect("toggle replace");
    app.dispatch_ui_intent(CommandDispatchIntent::SetFindReplaceText {
        text: "replacement".into(),
    })
    .expect("set replace text");

    let outcome = app
        .dispatch_ui_intent(CommandDispatchIntent::ReplaceOne)
        .expect("replace one");
    assert!(matches!(outcome, AppCommandOutcome::Noop));

    let text = buffer_text(&mut app);
    assert_eq!(
        text, "hello world\n",
        "buffer should be unchanged when no matches"
    );
}

#[test]
fn tab_switch_refreshes_find_matches() {
    let root = create_root();
    let first = root.join("first.txt");
    let second = root.join("second.txt");
    std::fs::write(&first, "alpha beta alpha\n").expect("seed first");
    std::fs::write(&second, "alpha gamma\n").expect("seed second");

    let mut app = trusted_app(&root);
    app.open_file(first.to_string_lossy()).expect("open first");
    let first_buffer = app.active_buffer_id().expect("first buffer");
    app.open_file(second.to_string_lossy())
        .expect("open second");

    app.dispatch_ui_intent(CommandDispatchIntent::ToggleFindBar)
        .expect("toggle find bar");
    app.dispatch_ui_intent(CommandDispatchIntent::SetFindQuery {
        query: "alpha".into(),
    })
    .expect("set query");

    let fb = find_bar(&mut app);
    assert_eq!(fb.match_count, 1, "second file has one 'alpha'");

    app.dispatch_ui_intent(CommandDispatchIntent::SwitchTab {
        buffer_id: first_buffer,
    })
    .expect("switch tab");

    let fb = find_bar(&mut app);
    assert_eq!(
        fb.match_count, 2,
        "first file has two 'alpha' matches after tab switch"
    );
}

#[test]
fn find_navigation_reveals_selected_match() {
    let (_root, mut app) = setup_with_content("needle\nfirst\nsecond\nneedle\n");

    app.dispatch_ui_intent(CommandDispatchIntent::ToggleFindBar)
        .expect("toggle find bar");
    app.dispatch_ui_intent(CommandDispatchIntent::SetFindQuery {
        query: "needle".into(),
    })
    .expect("set query");
    app.dispatch_ui_intent(CommandDispatchIntent::FindNext)
        .expect("find next");

    let snapshot = app
        .shell_projection_snapshot("find-replace")
        .expect("snapshot");
    assert_eq!(snapshot.find_bar_projection.current_match_index, 1);
    let viewport = snapshot
        .active_buffer_projection
        .viewport
        .expect("active viewport");
    assert_eq!(viewport.cursor.line, 3);
    assert_eq!(viewport.scroll.top_line, 3);
}
