use std::{
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

use devil_app::{AppCommandOutcome, AppComposition};
use devil_protocol::{PrincipalId, ProposalResponse, WorkspaceTrustState};
use devil_ui::{CommandDispatchIntent, SearchScopeProjection, SearchStatusKindProjection};

static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

struct TempWorkspace {
    root: PathBuf,
}

impl TempWorkspace {
    fn new() -> Self {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be after epoch")
            .as_nanos();
        let id = TEMP_COUNTER.fetch_add(1, Ordering::SeqCst);
        let root = std::env::temp_dir().join(format!(
            "devil_app_structural_search_{}_{}_{}",
            std::process::id(),
            nanos,
            id
        ));
        fs::create_dir(&root).expect("temp workspace should be created");
        Self { root }
    }

    fn path(&self) -> &Path {
        &self.root
    }

    fn write(&self, name: &str, content: &str) -> PathBuf {
        let path = self.root.join(name);
        fs::write(&path, content).expect("temp file should be written");
        path
    }
}

impl Drop for TempWorkspace {
    fn drop(&mut self) {
        let temp_root = std::env::temp_dir();
        let file_name = self.root.file_name().and_then(|name| name.to_str());
        if self.root.starts_with(&temp_root)
            && file_name.is_some_and(|name| name.starts_with("devil_app_structural_search_"))
        {
            let _ = fs::remove_dir_all(&self.root);
        }
    }
}

fn run_structural_search(
    app: &mut AppComposition,
    pattern: &str,
    rewrite: Option<&str>,
) -> devil_ui::StructuralSearchProjection {
    match app
        .dispatch_ui_intent(CommandDispatchIntent::RunStructuralSearch {
            scope: SearchScopeProjection::Workspace,
            pattern: pattern.to_string(),
            rewrite: rewrite.map(str::to_string),
            limit: 10,
        })
        .expect("structural search intent should dispatch")
    {
        AppCommandOutcome::StructuralSearchUpdated(projection) => projection,
        other => panic!("expected structural search outcome, got {other:?}"),
    }
}

#[test]
fn structural_search_previews_and_applies_open_workspace_rewrite() {
    let workspace = TempWorkspace::new();
    let first = workspace.write("one.rs", "pub fn alpha() {}\n");
    let second = workspace.write("two.rs", "pub fn beta() {}\n");
    let mut app = AppComposition::new();
    let opened_workspace = app
        .open_workspace(
            workspace.path(),
            WorkspaceTrustState::Trusted,
            PrincipalId("structural-search-test".to_string()),
        )
        .expect("workspace should reopen deterministically");
    let first_file = app.open_file(first.to_string_lossy()).expect("open first");
    let second_file = app
        .open_file(second.to_string_lossy())
        .expect("open second");
    let first_buffer = app
        .editor()
        .buffer_for_file(opened_workspace.workspace_id, first_file)
        .expect("first buffer");
    let second_buffer = app
        .editor()
        .buffer_for_file(opened_workspace.workspace_id, second_file)
        .expect("second buffer");

    let projection = run_structural_search(&mut app, "fn $NAME ( )", Some("fn renamed_$NAME ( )"));

    assert_eq!(
        projection.status.kind,
        SearchStatusKindProjection::Completed
    );
    assert_eq!(projection.matches.len(), 2);
    assert!(
        projection
            .matches
            .iter()
            .any(|row| row.file_id == first_file),
        "first open file id {first_file:?} missing from matches {:?}",
        projection.matches
    );
    assert!(
        projection
            .matches
            .iter()
            .any(|row| row.file_id == second_file),
        "second open file id {second_file:?} missing from matches {:?}",
        projection.matches
    );
    assert!(projection.matches.iter().any(|row| {
        row.captures
            .iter()
            .any(|capture| capture.name == "NAME" && capture.value == "alpha")
            && row.replacement_preview.as_deref() == Some("fn renamed_alpha ( )")
    }));
    assert!(projection.matches.iter().any(|row| {
        row.captures
            .iter()
            .any(|capture| capture.name == "NAME" && capture.value == "beta")
            && row.replacement_preview.as_deref() == Some("fn renamed_beta ( )")
    }));
    let proposal_id = projection
        .proposal_id
        .expect("rewrite search should create a proposal preview");
    assert_eq!(
        app.shell_projection_snapshot("structural search")
            .expect("snapshot")
            .structural_search_projection,
        projection
    );
    assert_eq!(
        fs::read_to_string(&first).expect("first disk"),
        "pub fn alpha() {}\n"
    );
    assert_eq!(
        fs::read_to_string(&second).expect("second disk"),
        "pub fn beta() {}\n"
    );

    match app
        .dispatch_ui_intent(CommandDispatchIntent::ApproveProposal { proposal_id })
        .expect("approve proposal")
    {
        AppCommandOutcome::ProposalLifecycleUpdated(ProposalResponse::Approved(_)) => {}
        other => panic!("expected approval, got {other:?}"),
    }
    match app
        .dispatch_ui_intent(CommandDispatchIntent::ApplyProposal { proposal_id })
        .expect("apply proposal")
    {
        AppCommandOutcome::ProposalLifecycleUpdated(ProposalResponse::Applied(_)) => {}
        other => panic!("expected apply, got {other:?}"),
    }

    assert_eq!(
        app.editor().text(first_buffer).expect("first buffer text"),
        "pub fn renamed_alpha ( ) {}\n"
    );
    assert_eq!(
        app.editor()
            .text(second_buffer)
            .expect("second buffer text"),
        "pub fn renamed_beta ( ) {}\n"
    );
    assert_eq!(
        fs::read_to_string(&first).expect("first disk after apply"),
        "pub fn alpha() {}\n"
    );
    assert_eq!(
        fs::read_to_string(&second).expect("second disk after apply"),
        "pub fn beta() {}\n"
    );
}
