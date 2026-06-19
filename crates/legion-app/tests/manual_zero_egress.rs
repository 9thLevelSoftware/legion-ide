use std::{
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

use legion_app::{AppCommandOutcome, AppComposition, AppProductMode, AppSaveOutcome};
use legion_protocol::{PrincipalId, TextCoordinate, WorkspaceTrustState};
use legion_ui::{CommandDispatchIntent, SearchScopeProjection, SearchStatusKindProjection};

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
            "legion_manual_zero_egress_{}_{}_{}",
            std::process::id(),
            nanos,
            id
        ));
        fs::create_dir(&root).expect("temp workspace should be created");
        fs::write(root.join("main.rs"), "fn main() {\n    let value = 1;\n}\n")
            .expect("source should be written");
        Self { root }
    }

    fn path(&self) -> &Path {
        &self.root
    }

    fn main_rs(&self) -> PathBuf {
        self.root.join("main.rs")
    }
}

impl Drop for TempWorkspace {
    fn drop(&mut self) {
        let temp_root = std::env::temp_dir();
        let file_name = self.root.file_name().and_then(|name| name.to_str());
        if self.root.starts_with(&temp_root)
            && file_name.is_some_and(|name| name.starts_with("legion_manual_zero_egress_"))
        {
            let _ = fs::remove_dir_all(&self.root);
        }
    }
}

fn coord(line: u32, character: u32, byte_offset: u64) -> TextCoordinate {
    TextCoordinate {
        line,
        character,
        byte_offset: Some(byte_offset),
        utf16_offset: Some(byte_offset),
    }
}

#[test]
fn manual_mode_open_edit_save_search_records_no_hosted_egress() {
    let workspace = TempWorkspace::new();
    let mut app = AppComposition::new();
    app.set_product_mode(AppProductMode::Manual);
    app.open_workspace(
        workspace.path(),
        WorkspaceTrustState::Trusted,
        PrincipalId("manual-smoke".to_string()),
    )
    .expect("workspace should open");
    app.open_file(workspace.main_rs().to_string_lossy())
        .expect("main.rs should open");
    let snapshot = app
        .shell_projection_snapshot("Manual")
        .expect("snapshot should project");
    let buffer_id = snapshot
        .active_buffer_projection
        .buffer_id
        .expect("active buffer should be projected");

    app.dispatch_ui_intent(CommandDispatchIntent::Insert {
        buffer_id,
        at: coord(1, 4, 16),
        text: "let local_only = true;\n    ".to_string(),
    })
    .expect("insert should route through app authority");

    let search = app
        .dispatch_ui_intent(CommandDispatchIntent::RunSearch {
            scope: SearchScopeProjection::ActiveFile,
            query: "local_only".to_string(),
            limit: 10,
        })
        .expect("active-file search should route through app authority");
    let AppCommandOutcome::SearchUpdated(search_projection) = search else {
        panic!("expected search projection, got {search:?}");
    };
    assert_eq!(
        search_projection.status.kind,
        SearchStatusKindProjection::Completed
    );
    assert_eq!(search_projection.results.len(), 1);

    let save = app
        .dispatch_ui_intent(CommandDispatchIntent::Save { buffer_id })
        .expect("save should route through app authority");
    assert!(matches!(
        save,
        AppCommandOutcome::Save(AppSaveOutcome::Saved(_))
    ));
    assert!(
        fs::read_to_string(workspace.main_rs())
            .expect("main.rs should be readable")
            .contains("let local_only = true;")
    );

    let snapshot = app
        .shell_projection_snapshot("Manual")
        .expect("snapshot should project after save");
    assert_eq!(app.product_mode(), AppProductMode::Manual);
    assert_eq!(snapshot.product_mode, legion_ui::DockMode::Manual);
    assert_eq!(snapshot.assisted_ai_projection.provider_count, 0);
    assert_eq!(snapshot.assisted_ai_projection.request_count, 0);
    assert_eq!(snapshot.assisted_ai_projection.preview_ready_count, 0);
    assert!(snapshot.assisted_ai_projection.providers.is_empty());
    assert!(snapshot.assisted_ai_projection.requests.is_empty());
    assert!(snapshot.assisted_ai_projection.proposal_previews.is_empty());
    assert!(
        snapshot
            .assist_inline_prediction_projection
            .active_prediction
            .is_none()
    );
    assert!(snapshot.assist_inline_prediction_projection.rows.is_empty());
    assert!(
        !snapshot
            .assist_inline_prediction_projection
            .request_in_flight
    );
    assert_eq!(snapshot.delegated_task_projection.plan_count, 0);
    assert!(snapshot.delegated_task_projection.plan_rows.is_empty());
    assert!(snapshot.delegated_task_projection.chat_messages.is_empty());
    assert!(
        snapshot
            .delegated_task_projection
            .context_citations
            .is_empty()
    );
    assert!(
        snapshot
            .status_messages
            .iter()
            .all(|status| !status.message.to_ascii_lowercase().contains("http"))
    );
}
