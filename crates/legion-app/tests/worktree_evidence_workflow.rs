/// Task 6: Worktree state evidence export.
///
/// ExportWorktreeEvidence produces a metadata-only TOML (staged/unstaged/
/// conflict counts, repo-relative paths) with NO file contents and NO diffs.
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

use legion_app::{AppCommandOutcome, AppComposition};
use legion_ui::CommandDispatchIntent;

static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

struct TempGitRepo {
    root: PathBuf,
}

fn git_available() -> bool {
    use std::sync::OnceLock;
    static AVAILABLE: OnceLock<bool> = OnceLock::new();
    *AVAILABLE.get_or_init(|| {
        Command::new("git")
            .arg("--version")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    })
}

impl TempGitRepo {
    fn new() -> Self {
        assert!(git_available(), "git required for evidence tests");
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        let id = TEMP_COUNTER.fetch_add(1, Ordering::SeqCst);
        let root =
            std::env::temp_dir().join(format!("legion_ev_{}_{}_{}", std::process::id(), nanos, id));
        fs::create_dir(&root).expect("temp dir");
        run_git(&root, ["init"]);
        run_git(&root, ["branch", "-M", "master"]);
        run_git(&root, ["config", "user.email", "ev@test.example"]);
        run_git(&root, ["config", "user.name", "Evidence Test"]);
        Self { root }
    }

    fn path(&self) -> &Path {
        &self.root
    }

    fn write(&self, relative: &str, content: &str) -> PathBuf {
        let path = self.root.join(relative);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("parent");
        }
        fs::write(&path, content).expect("write");
        path
    }
}

impl Drop for TempGitRepo {
    fn drop(&mut self) {
        let tmp = std::env::temp_dir();
        if self.root.starts_with(&tmp)
            && self
                .root
                .file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.starts_with("legion_ev_"))
        {
            let _ = fs::remove_dir_all(&self.root);
        }
    }
}

fn run_git<const N: usize>(root: &Path, args: [&str; N]) {
    let output = Command::new("git")
        .current_dir(root)
        .args(args)
        .output()
        .expect("git command");
    assert!(output.status.success(), "git {:?} failed", args);
}

fn setup_repo_with_changes(repo: &TempGitRepo) {
    repo.write("src/lib.rs", "pub fn hello() {}\n");
    repo.write("src/main.rs", "fn main() {}\n");
    run_git(repo.path(), ["add", "."]);
    run_git(repo.path(), ["commit", "-m", "initial"]);
    // Modify both files: one staged, one unstaged.
    repo.write("src/lib.rs", "pub fn hello() { println!(\"hi\"); }\n");
    run_git(repo.path(), ["add", "src/lib.rs"]);
    repo.write("src/main.rs", "fn main() { println!(\"hey\"); }\n");
}

#[test]
fn export_worktree_evidence_returns_path() {
    let repo = TempGitRepo::new();
    setup_repo_with_changes(&repo);

    let mut app = AppComposition::new();
    app.open_workspace(
        repo.path(),
        legion_protocol::WorkspaceTrustState::Trusted,
        legion_protocol::PrincipalId("ev-test".to_string()),
    )
    .expect("workspace open");
    app.dispatch_ui_intent(CommandDispatchIntent::RefreshGit)
        .expect("refresh git");

    let result = app
        .dispatch_ui_intent(CommandDispatchIntent::ExportWorktreeEvidence)
        .expect("export evidence should dispatch");

    let evidence_path = match result {
        AppCommandOutcome::WorktreeEvidenceExported(path) => path,
        other => panic!("expected WorktreeEvidenceExported, got {other:?}"),
    };

    assert!(
        !evidence_path.is_empty(),
        "evidence path should not be empty"
    );
    let path = std::path::Path::new(&evidence_path);
    assert!(
        path.exists(),
        "evidence file should exist at: {}",
        evidence_path
    );
}

#[test]
fn export_worktree_evidence_toml_has_no_file_contents() {
    let repo = TempGitRepo::new();
    setup_repo_with_changes(&repo);

    let mut app = AppComposition::new();
    app.open_workspace(
        repo.path(),
        legion_protocol::WorkspaceTrustState::Trusted,
        legion_protocol::PrincipalId("ev-test".to_string()),
    )
    .expect("workspace open");
    app.dispatch_ui_intent(CommandDispatchIntent::RefreshGit)
        .expect("refresh git");

    let evidence_path = match app
        .dispatch_ui_intent(CommandDispatchIntent::ExportWorktreeEvidence)
        .expect("export evidence")
    {
        AppCommandOutcome::WorktreeEvidenceExported(p) => p,
        other => panic!("{other:?}"),
    };

    let content = fs::read_to_string(&evidence_path).expect("evidence file should be readable");

    // Must be valid TOML (no parse error).
    let _: toml::Value = toml::from_str(&content)
        .unwrap_or_else(|e| panic!("evidence file must be valid TOML: {e}\ncontent:\n{content}"));

    // Hard constraint: MUST NOT contain actual file content (e.g. "pub fn hello" from src/lib.rs).
    assert!(
        !content.contains("pub fn hello"),
        "evidence must NOT contain file source content"
    );
    assert!(
        !content.contains("fn main"),
        "evidence must NOT contain file source content"
    );
    assert!(
        !content.contains("println"),
        "evidence must NOT contain diff lines"
    );

    // Must contain count information.
    assert!(
        content.contains("staged") || content.contains("unstaged"),
        "evidence should reference staged/unstaged counts"
    );
}

#[test]
fn export_worktree_evidence_written_to_legion_evidence_dir() {
    let repo = TempGitRepo::new();
    setup_repo_with_changes(&repo);

    let mut app = AppComposition::new();
    app.open_workspace(
        repo.path(),
        legion_protocol::WorkspaceTrustState::Trusted,
        legion_protocol::PrincipalId("ev-test".to_string()),
    )
    .expect("workspace open");
    app.dispatch_ui_intent(CommandDispatchIntent::RefreshGit)
        .expect("refresh git");

    let evidence_path_str = match app
        .dispatch_ui_intent(CommandDispatchIntent::ExportWorktreeEvidence)
        .expect("export evidence")
    {
        AppCommandOutcome::WorktreeEvidenceExported(p) => p,
        other => panic!("{other:?}"),
    };

    // Evidence must be under <workspace>/.legion/evidence/
    let evidence_path = std::path::Path::new(&evidence_path_str);
    let evidence_dir = repo.path().join(".legion").join("evidence");
    assert!(
        evidence_path.starts_with(&evidence_dir),
        "evidence file should be under .legion/evidence/; got: {}",
        evidence_path_str
    );
}
