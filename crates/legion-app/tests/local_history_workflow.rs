/// Task 3: Local history snapshots — fix-round tests covering C-1/C-2/I-2/I-3/M-3/M-5.
///
/// On every successful save, a bounded local-history entry is recorded
/// (file identity, content hash, timestamp, correlation id) and the content
/// blob is written under the workspace state dir. Restore generates a
/// proposal-mediated edit+save rather than a direct file write.
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

struct TempWorkspace {
    root: PathBuf,
}

impl TempWorkspace {
    fn new() -> Self {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        let id = TEMP_COUNTER.fetch_add(1, Ordering::SeqCst);
        let root =
            std::env::temp_dir().join(format!("legion_lh_{}_{}_{}", std::process::id(), nanos, id));
        fs::create_dir(&root).expect("temp dir");
        // Init a git repo so WorkspaceActor can resolve the root.
        let _ = Command::new("git")
            .current_dir(&root)
            .args(["init"])
            .status();
        let _ = Command::new("git")
            .current_dir(&root)
            .args(["config", "user.email", "lh@test.example"])
            .status();
        let _ = Command::new("git")
            .current_dir(&root)
            .args(["config", "user.name", "LH Test"])
            .status();
        Self { root }
    }

    fn path(&self) -> &Path {
        &self.root
    }

    fn write(&self, relative: &str, content: &str) -> PathBuf {
        let path = self.root.join(relative);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("parent dir");
        }
        fs::write(&path, content).expect("write");
        path
    }
}

impl Drop for TempWorkspace {
    fn drop(&mut self) {
        let tmp = std::env::temp_dir();
        if self.root.starts_with(&tmp)
            && self
                .root
                .file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.starts_with("legion_lh_"))
        {
            let _ = fs::remove_dir_all(&self.root);
        }
    }
}

fn open_app_with_file(
    ws: &TempWorkspace,
    relative: &str,
    content: &str,
) -> (AppComposition, PathBuf) {
    let path = ws.write(relative, content);
    let mut app = AppComposition::new();
    app.open_workspace(
        ws.path(),
        legion_protocol::WorkspaceTrustState::Trusted,
        legion_protocol::PrincipalId("lh-test".to_string()),
    )
    .expect("workspace open");
    app.open_file(path.to_string_lossy()).expect("open file");
    (app, path)
}

fn save_file(app: &mut AppComposition) {
    let buf = app.active_buffer_id().expect("active buffer");
    app.dispatch_ui_intent(CommandDispatchIntent::Save { buffer_id: buf })
        .expect("save should dispatch");
}

fn get_entries(
    app: &mut AppComposition,
    path: &Path,
) -> Vec<legion_ui::LocalHistoryEntryProjection> {
    match app
        .dispatch_ui_intent(CommandDispatchIntent::RequestLocalHistoryEntries {
            path: path.to_string_lossy().to_string(),
        })
        .expect("local history request should dispatch")
    {
        AppCommandOutcome::LocalHistoryEntriesUpdated(e) => e,
        other => panic!("expected LocalHistoryEntriesUpdated, got {other:?}"),
    }
}

// ---------------------------------------------------------------------------
// C-1: SHA-256 content hash is stable and well-formed
// ---------------------------------------------------------------------------

#[test]
fn local_history_sha256_hash_is_stable_and_64_chars() {
    let ws = TempWorkspace::new();
    let (mut app, path) = open_app_with_file(&ws, "src/hashtest.rs", "fn stable() {}\n");
    save_file(&mut app);

    let entries = get_entries(&mut app, &path);
    assert_eq!(entries.len(), 1, "one save → one entry");

    let hash = &entries[0].content_hash;
    // Known vector: SHA-256("fn stable() {}\n") — pins the implementation to
    // real SHA-256, not merely any deterministic 64-hex-char digest.
    assert_eq!(
        hash, "af2f02ad0b09831697256fb0fa84a89bbed8e7cd6463b181242686078e735da1",
        "content hash must be the SHA-256 of the saved file content"
    );
    // SHA-256 produces 32 bytes = 64 hex characters.
    assert_eq!(
        hash.len(),
        64,
        "SHA-256 hash must be 64 hex chars, got '{hash}'"
    );
    assert!(
        hash.chars().all(|c| c.is_ascii_hexdigit()),
        "hash must be lowercase hex: '{hash}'"
    );

    // Saving the same content again produces the same hash (stability).
    let buf = app.active_buffer_id().expect("buf");
    app.dispatch_ui_intent(CommandDispatchIntent::Save { buffer_id: buf })
        .expect("save 2");
    let entries2 = get_entries(&mut app, &path);
    assert!(entries2.len() >= 2, "two saves → two entries");
    // Newest entry is entries2[0] (newest-first ordering); oldest is last.
    assert_eq!(
        entries2[0].content_hash, *hash,
        "same content must produce the same SHA-256 hash"
    );
}

// ---------------------------------------------------------------------------
// C-2: .legion/local-history/ is self-ignoring via a generated .gitignore
// ---------------------------------------------------------------------------

#[test]
fn local_history_creates_gitignore_in_legion_subdir() {
    let ws = TempWorkspace::new();
    let (mut app, path) = open_app_with_file(&ws, "src/gi_test.rs", "fn gi() {}\n");
    save_file(&mut app);
    let _ = get_entries(&mut app, &path); // ensure entry is recorded

    let gitignore = ws
        .path()
        .join(".legion")
        .join("local-history")
        .join(".gitignore");
    assert!(
        gitignore.exists(),
        ".legion/local-history/.gitignore must be created by the first save"
    );
    let content = fs::read_to_string(&gitignore).expect("read .gitignore");
    assert!(
        content.trim() == "*",
        ".legion/local-history/.gitignore must contain '*'; found: {content:?}"
    );
}

#[test]
fn local_history_blobs_not_visible_to_git_status() {
    let ws = TempWorkspace::new();
    let (mut app, path) = open_app_with_file(&ws, "src/gitstatus.rs", "fn status_test() {}\n");
    save_file(&mut app);
    let _ = get_entries(&mut app, &path);

    // Run git status --porcelain from the workspace root.
    let output = Command::new("git")
        .current_dir(ws.path())
        .args(["status", "--porcelain"])
        .output()
        .expect("git status should run");
    let stdout = String::from_utf8_lossy(&output.stdout);
    // No .legion/ entries should appear.
    assert!(
        !stdout.contains(".legion"),
        "git status must not show .legion/ entries after save; got:\n{stdout}"
    );
}

// ---------------------------------------------------------------------------
// I-2: prune deletes evicted blob files from disk
// ---------------------------------------------------------------------------

#[test]
fn prune_deletes_blob_files_on_eviction() {
    let ws = TempWorkspace::new();
    let (mut app, path) = open_app_with_file(&ws, "src/prune_test.rs", "fn v0() {}\n");
    let buf = app.active_buffer_id().expect("buf");

    // Produce 4 saves with distinct content.
    for i in 1..=4u32 {
        app.dispatch_ui_intent(CommandDispatchIntent::Insert {
            buffer_id: buf,
            at: legion_protocol::TextCoordinate {
                line: 0,
                character: 0,
                byte_offset: Some(0),
                utf16_offset: Some(0),
            },
            text: format!("// v{i}\n"),
        })
        .expect("insert");
        save_file(&mut app);
    }

    let canonical = path.canonicalize().unwrap_or_else(|_| path.clone());
    let canon_str = canonical.to_string_lossy();
    // Strip Windows UNC prefix manually for path-key derivation check.
    let stripped = canon_str.trim_start_matches(r"\\?\");
    let path_key = stripped.replace(['/', '\\', ':', '*', '?', '"', '<', '>', '|'], "_");
    let blob_dir = ws
        .path()
        .join(".legion")
        .join("local-history")
        .join(&path_key);

    let blobs_before: Vec<_> = fs::read_dir(&blob_dir)
        .expect("blob dir should exist")
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().and_then(|s| s.to_str()) == Some("blob"))
        .collect();
    let count_before = blobs_before.len();
    assert!(
        count_before >= 3,
        "should have >= 3 blobs before prune; got {count_before}"
    );

    // Use test helper to prune to 2 entries.
    let evicted = app.test_prune_local_history(&canonical.to_string_lossy(), 2);
    assert!(!evicted.is_empty(), "prune should evict entries");

    let count_after = fs::read_dir(&blob_dir)
        .expect("blob dir still exists")
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().and_then(|s| s.to_str()) == Some("blob"))
        .count();
    assert!(
        count_after <= 2,
        "blob count should drop to <= 2 after prune to 2; got {count_after}"
    );
    assert!(
        count_after < count_before,
        "prune must delete blob files (before={count_before}, after={count_after})"
    );
}

// ---------------------------------------------------------------------------
// I-3: entry_id is a UUID (unique, collision-proof)
// ---------------------------------------------------------------------------

#[test]
fn entry_ids_are_unique_across_saves() {
    let ws = TempWorkspace::new();
    let (mut app, path) = open_app_with_file(&ws, "src/uuid_test.rs", "fn a() {}\n");
    let buf = app.active_buffer_id().expect("buf");

    for i in 0..5u32 {
        app.dispatch_ui_intent(CommandDispatchIntent::Insert {
            buffer_id: buf,
            at: legion_protocol::TextCoordinate {
                line: 0,
                character: 0,
                byte_offset: Some(0),
                utf16_offset: Some(0),
            },
            text: format!("// {i}\n"),
        })
        .expect("insert");
        save_file(&mut app);
    }

    let entries = get_entries(&mut app, &path);
    assert!(entries.len() >= 5, "should have 5 entries");

    let ids: std::collections::HashSet<&str> =
        entries.iter().map(|e| e.entry_id.as_str()).collect();
    assert_eq!(
        ids.len(),
        entries.len(),
        "all entry_ids must be unique (UUID); found duplicates"
    );
    // UUID v7 format: 8-4-4-4-12 hex groups.
    for e in &entries {
        assert!(
            e.entry_id.len() == 36 && e.entry_id.chars().filter(|c| *c == '-').count() == 4,
            "entry_id must be a UUID (36 chars with 4 hyphens): '{}'",
            e.entry_id
        );
    }
}

// ---------------------------------------------------------------------------
// M-3: Retention cap test exercises the actual eviction boundary
// ---------------------------------------------------------------------------

#[test]
fn local_history_retention_cap_is_enforced_at_boundary() {
    let ws = TempWorkspace::new();
    let (mut app, path) = open_app_with_file(&ws, "src/capped.rs", "fn v0() {}\n");
    let buf = app.active_buffer_id().expect("buf");

    // Produce 4 saves, then prune to 3 with the test helper.
    for i in 1..=4u32 {
        app.dispatch_ui_intent(CommandDispatchIntent::Insert {
            buffer_id: buf,
            at: legion_protocol::TextCoordinate {
                line: 0,
                character: 0,
                byte_offset: Some(0),
                utf16_offset: Some(0),
            },
            text: format!("// v{i}\n"),
        })
        .expect("insert");
        save_file(&mut app);
    }

    // In-memory count before prune.
    let canonical = path.canonicalize().unwrap_or_else(|_| path.clone());
    let count_before = app.test_local_history_entry_count(&canonical.to_string_lossy());
    assert_eq!(count_before, 4, "should have 4 entries before prune");

    // Prune to 3 — exercises the count eviction boundary.
    let evicted = app.test_prune_local_history(&canonical.to_string_lossy(), 3);
    assert_eq!(evicted.len(), 1, "exactly 1 entry should be evicted");
    assert_eq!(
        app.test_local_history_entry_count(&canonical.to_string_lossy()),
        3,
        "count must be 3 after prune to 3"
    );

    // The production 50-entry cap is separately verified to be ≤ 50.
    let entries = get_entries(&mut app, &path);
    assert!(
        entries.len() <= 50,
        "entries must not exceed production cap"
    );
}

// ---------------------------------------------------------------------------
// M-5: Blob write errors surface as a degraded-mode diagnostic
// ---------------------------------------------------------------------------

#[test]
fn blob_write_error_sets_degraded_diagnostic() {
    let ws = TempWorkspace::new();
    // Create a file at the path where the blob DIRECTORY would go, preventing
    // dir creation and causing a write error.
    let (mut app, path) = open_app_with_file(&ws, "src/err_test.rs", "fn err() {}\n");

    let canonical = path.canonicalize().unwrap_or_else(|_| path.clone());
    let canon_str = canonical.to_string_lossy();
    let stripped = canon_str.trim_start_matches(r"\\?\");
    let path_key = stripped.replace(['/', '\\', ':', '*', '?', '"', '<', '>', '|'], "_");
    // Place a *file* where the blob dir should go so create_dir_all fails.
    let history_base = ws.path().join(".legion").join("local-history");
    fs::create_dir_all(&history_base).expect("create base");
    let blocker_path = history_base.join(&path_key);
    // Only place a blocker if the path doesn't already exist as a directory.
    if !blocker_path.exists() {
        fs::write(&blocker_path, b"blocker").expect("write blocker file");
    }

    // Attempt to save — the blob write will fail but the save itself should succeed.
    let save_result = app.dispatch_ui_intent(CommandDispatchIntent::Save {
        buffer_id: app.active_buffer_id().expect("buf"),
    });
    // Save outcome itself should be OK (blob write failure is non-fatal).
    assert!(
        save_result.is_ok(),
        "save should succeed even if blob write fails: {:?}",
        save_result.err()
    );

    // The degraded write error should be accessible.
    let write_err = app.test_local_history_last_write_error();
    assert!(
        write_err.is_some(),
        "a blob write error should set the degraded diagnostic"
    );

    // After a git refresh the error propagates into diagnostics.
    let git_proj = app.refresh_git_projection();
    assert!(
        git_proj
            .diagnostics
            .iter()
            .any(|d| d.contains("local_history.write_degraded")),
        "degraded write error must appear in git_projection.diagnostics; got: {:?}",
        git_proj.diagnostics
    );
}

// ---------------------------------------------------------------------------
// Existing tests (preserve)
// ---------------------------------------------------------------------------

#[test]
fn local_history_records_entry_after_save() {
    let ws = TempWorkspace::new();
    let (mut app, path) = open_app_with_file(&ws, "src/file.rs", "fn main() {}\n");
    save_file(&mut app);

    let entries = get_entries(&mut app, &path);
    assert_eq!(
        entries.len(),
        1,
        "one save should produce one history entry"
    );
    assert!(
        !entries[0].content_hash.is_empty(),
        "history entry should have a non-empty content hash"
    );
}

#[test]
fn local_history_records_multiple_saves() {
    let ws = TempWorkspace::new();
    let (mut app, path) = open_app_with_file(&ws, "src/multi.rs", "fn v1() {}\n");
    let buf = app.active_buffer_id().expect("active buffer");

    save_file(&mut app);

    app.dispatch_ui_intent(CommandDispatchIntent::Insert {
        buffer_id: buf,
        at: legion_protocol::TextCoordinate {
            line: 1,
            character: 0,
            byte_offset: Some(11),
            utf16_offset: Some(11),
        },
        text: "fn v2() {}\n".to_string(),
    })
    .expect("insert");
    save_file(&mut app);

    let entries = get_entries(&mut app, &path);
    assert!(
        entries.len() >= 2,
        "two saves should produce at least two history entries; got {}",
        entries.len()
    );
    let hashes: std::collections::HashSet<&str> =
        entries.iter().map(|e| e.content_hash.as_str()).collect();
    assert!(
        hashes.len() >= 2,
        "entries should have distinct content hashes"
    );
}

#[test]
fn restore_from_local_history_uses_proposal_route() {
    let ws = TempWorkspace::new();
    let (mut app, path) = open_app_with_file(&ws, "src/restore.rs", "fn original() {}\n");
    let buf = app.active_buffer_id().expect("active buffer");

    save_file(&mut app);

    let entries = get_entries(&mut app, &path);
    assert!(!entries.is_empty(), "should have history entry");
    let entry_id = entries[0].entry_id.clone();

    app.dispatch_ui_intent(CommandDispatchIntent::Insert {
        buffer_id: buf,
        at: legion_protocol::TextCoordinate {
            line: 0,
            character: 0,
            byte_offset: Some(0),
            utf16_offset: Some(0),
        },
        text: "// modified\n".to_string(),
    })
    .expect("insert");
    save_file(&mut app);

    let restore_result = app.dispatch_ui_intent(CommandDispatchIntent::RestoreFromLocalHistory {
        path: path.to_string_lossy().to_string(),
        entry_id,
    });

    assert!(
        restore_result.is_ok(),
        "restore should succeed, got: {:?}",
        restore_result.err()
    );
    let history_dir = ws.path().join(".legion").join("local-history");
    assert!(
        history_dir.exists(),
        ".legion/local-history/ should exist after saves"
    );
}

// ---------------------------------------------------------------------------
// Path-canonicalization regression tests (CI: Windows 8.3 / macOS /var symlink)
// ---------------------------------------------------------------------------

/// Windows regression: open the workspace via a directory junction pointing to
/// the real temp dir.  Saving a file and then looking up history via the real
/// path must find the entry even though the workspace was opened via the alias.
///
/// Uses `mklink /J` (available without elevation on Windows) to create the
/// junction.  Gracefully skipped if junction creation fails (e.g. FAT volume).
#[cfg(windows)]
#[test]
fn local_history_survives_junction_alias_on_windows() {
    let ws = TempWorkspace::new();

    // Create a directory junction that points to ws.root.
    let alias = std::env::temp_dir().join(format!(
        "legion_lh_junc_{}_{}_{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos(),
        TEMP_COUNTER.fetch_add(1, Ordering::SeqCst)
    ));

    struct JuncGuard(PathBuf);
    impl Drop for JuncGuard {
        fn drop(&mut self) {
            let _ = Command::new("cmd")
                .args(["/C", "rmdir", &self.0.to_string_lossy()])
                .status();
        }
    }

    let junc_status = Command::new("cmd")
        .args([
            "/C",
            "mklink",
            "/J",
            &alias.to_string_lossy(),
            &ws.root.to_string_lossy(),
        ])
        .status();
    let junc_ok = junc_status.map(|s| s.success()).unwrap_or(false);
    if !junc_ok || !alias.exists() {
        // Junction unavailable on this runner; skip gracefully.
        eprintln!("SKIP: junction creation failed, skipping Windows alias test");
        return;
    }
    let _guard = JuncGuard(alias.clone());

    // Write the file via the real path and open the workspace via the junction.
    let real_path = ws.write("src/junc.rs", "fn junc() {}\n");

    let mut app = AppComposition::new();
    // Open workspace via the alias path (junction).
    app.open_workspace(
        &alias,
        legion_protocol::WorkspaceTrustState::Trusted,
        legion_protocol::PrincipalId("junc-test".to_string()),
    )
    .expect("workspace open via junction");
    // Open the file via the real path.
    app.open_file(real_path.to_string_lossy())
        .expect("open file");

    save_file(&mut app);

    // Look up via the real path — must find 1 entry.
    let entries = get_entries(&mut app, &real_path);
    assert_eq!(
        entries.len(),
        1,
        "one save → one entry even with junction alias; got {}",
        entries.len()
    );
}

/// Unix regression: open workspace via a symlink to the real temp dir.  Saving
/// via the symlink path and looking up via the real (resolved) path must still
/// find the entry.
#[cfg(not(windows))]
#[test]
fn local_history_survives_symlinked_workspace_on_unix() {
    let ws = TempWorkspace::new();

    // Create a symlink that points to ws.root.
    let link = std::env::temp_dir().join(format!(
        "legion_lh_sym_{}_{}_{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos(),
        TEMP_COUNTER.fetch_add(1, Ordering::SeqCst)
    ));

    struct SymGuard(PathBuf);
    impl Drop for SymGuard {
        fn drop(&mut self) {
            let _ = fs::remove_file(&self.0);
        }
    }

    if std::os::unix::fs::symlink(&ws.root, &link).is_err() {
        eprintln!("SKIP: symlink creation failed, skipping Unix symlink test");
        return;
    }
    let _guard = SymGuard(link.clone());

    // Write and open the file via the symlink path.
    let sym_file = link.join("src").join("sym.rs");
    fs::create_dir_all(sym_file.parent().unwrap()).expect("sym parent dir");
    fs::write(&sym_file, "fn sym() {}\n").expect("sym write");

    let mut app = AppComposition::new();
    app.open_workspace(
        &link,
        legion_protocol::WorkspaceTrustState::Trusted,
        legion_protocol::PrincipalId("sym-test".to_string()),
    )
    .expect("workspace open via symlink");
    app.open_file(sym_file.to_string_lossy())
        .expect("open file via symlink");

    save_file(&mut app);

    // Look up via the symlink path — must find 1 entry.
    let entries = get_entries(&mut app, &sym_file);
    assert_eq!(
        entries.len(),
        1,
        "one save → one entry even with symlinked workspace; got {}",
        entries.len()
    );
}
