//! GAP-10.2: Help/About metadata-only support bundle through AppComposition.

use std::{
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

use legion_app::{AppCommandOutcome, AppComposition};
use legion_protocol::{PrincipalId, WorkspaceTrustState};
use legion_ui::CommandDispatchIntent;

static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

struct TempWorkspace {
    root: PathBuf,
}

impl TempWorkspace {
    fn new(prefix: &str) -> Self {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be after epoch")
            .as_nanos();
        let id = TEMP_COUNTER.fetch_add(1, Ordering::SeqCst);
        let root =
            std::env::temp_dir().join(format!("{prefix}_{}_{}_{}", std::process::id(), nanos, id));
        fs::create_dir(&root).expect("temp workspace should be created");
        Self { root }
    }

    fn path(&self) -> &Path {
        &self.root
    }
}

impl Drop for TempWorkspace {
    fn drop(&mut self) {
        let temp_root = std::env::temp_dir();
        let file_name = self.root.file_name().and_then(|name| name.to_str());
        if self.root.starts_with(&temp_root)
            && file_name.is_some_and(|name| name.starts_with("legion_support_bundle_"))
        {
            let _ = fs::remove_dir_all(&self.root);
        }
    }
}

fn open_app(root: &Path) -> AppComposition {
    let mut app = AppComposition::new();
    app.open_workspace(
        root,
        WorkspaceTrustState::Trusted,
        PrincipalId("support-bundle-test".to_string()),
    )
    .expect("workspace should open");
    app
}

#[test]
fn export_support_bundle_writes_metadata_only_markdown() {
    let workspace = TempWorkspace::new("legion_support_bundle");
    let crash_dir = workspace
        .path()
        .join(".legion")
        .join("crash-reports")
        .join("crash-aaa");
    fs::create_dir_all(&crash_dir).expect("crash dir");
    fs::write(
        crash_dir.join("summary.toml"),
        "crash_id = \"crash-aaa\"\ntimestamp = 1700000000\nos = \"windows\"\n",
    )
    .expect("summary");
    fs::write(crash_dir.join("panic.txt"), "SECRET_RAW_PANIC_BODY").expect("raw panic");

    let mut app = open_app(workspace.path());
    let outcome = app
        .dispatch_ui_intent(CommandDispatchIntent::ExportSupportBundle)
        .expect("export should dispatch");
    let path = match outcome {
        AppCommandOutcome::SupportBundleExported(path) => path,
        other => panic!("expected SupportBundleExported, got {other:?}"),
    };

    let dest = workspace.path().join(".legion").join("support-bundle.md");
    let written = Path::new(&path);
    assert!(
        written.ends_with(Path::new(".legion").join("support-bundle.md")),
        "export path should be .legion/support-bundle.md, got {path}"
    );
    assert!(dest.is_file(), "bundle should exist at {}", dest.display());
    let body = fs::read_to_string(&dest).expect("bundle should be readable");
    assert!(body.contains("metadata_only: true"));
    assert!(body.contains("crash_id: crash-aaa"));
    assert!(body.contains("product_mode: Manual"));
    assert!(body.contains("raw_source_allowed: false"));
    assert!(!body.contains("SECRET_RAW_PANIC_BODY"));
    assert!(!body.contains("panic.txt"));
    assert!(!body.contains("SECRET_DIRTY_BODY"));
}

#[test]
fn export_support_bundle_omits_dirty_editor_text() {
    let workspace = TempWorkspace::new("legion_support_bundle");
    let notes = workspace.path().join("notes.txt");
    fs::write(&notes, "clean").expect("notes");
    let mut app = open_app(workspace.path());
    app.open_file(notes.to_string_lossy()).expect("open notes");
    let buffer_id = app.active_buffer_id().expect("active buffer");
    app.dispatch_ui_intent(CommandDispatchIntent::Insert {
        buffer_id,
        at: legion_protocol::TextCoordinate {
            line: 0,
            character: 5,
            byte_offset: Some(5),
            utf16_offset: Some(5),
        },
        text: "SECRET_DIRTY_BODY".to_string(),
    })
    .expect("insert dirty text");

    let outcome = app
        .dispatch_ui_intent(CommandDispatchIntent::ExportSupportBundle)
        .expect("export should dispatch");
    let AppCommandOutcome::SupportBundleExported(path) = outcome else {
        panic!("expected SupportBundleExported");
    };
    let body = fs::read_to_string(path).expect("bundle");
    assert!(body.contains("metadata_only: true"));
    assert!(!body.contains("SECRET_DIRTY_BODY"));
    assert!(!body.contains("clean"));
}

#[test]
fn open_about_is_reachable() {
    let workspace = TempWorkspace::new("legion_support_bundle");
    let mut app = open_app(workspace.path());
    let outcome = app
        .dispatch_ui_intent(CommandDispatchIntent::OpenAbout)
        .expect("about should dispatch");
    assert!(matches!(outcome, AppCommandOutcome::AboutOpened));
}
