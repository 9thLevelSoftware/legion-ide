//! Local history metadata store.
//!
//! Keeps a bounded in-memory index of local file history snapshots.
//! The authoritative content blobs live on disk in the workspace state
//! directory (`.legion/local-history/<path_key>/<content_hash>.blob`);
//! this module persists only the metadata (identity, hash, timestamp,
//! correlation id) so audit records stay metadata-only.

use std::{
    collections::HashMap,
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};

use super::StorageError;

/// Schema version for local history records and the on-disk manifest.
pub const LOCAL_HISTORY_SCHEMA_VERSION: u32 = 1;

/// Directory name under `.legion/` for blobs and the metadata manifest.
pub const LOCAL_HISTORY_DIR_NAME: &str = "local-history";

const MANIFEST_FILE: &str = "manifest.json";

/// Metadata record for one local file history snapshot.
///
/// The actual file content is stored as a content-addressed blob on disk;
/// only identity metadata is held here.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LocalHistoryRecord {
    /// Stable entry identifier (UUID v4 string).
    pub entry_id: String,
    /// Workspace-local file identifier string (from `FileId`).
    pub file_id_str: String,
    /// Canonical file path (workspace-relative or absolute).
    pub canonical_path: String,
    /// SHA-256 content hash hex string (from editor save request).
    pub content_hash: String,
    /// Unix timestamp in milliseconds at snapshot time.
    pub timestamp_ms: u64,
    /// Correlation identifier string for audit cross-referencing.
    pub correlation_id_str: String,
    /// Content size in bytes.
    pub size_bytes: u64,
    /// Schema version for forward compatibility.
    pub schema_version: u32,
}

/// Local history metadata store with bounded retention.
///
/// Records are keyed by canonical file path and ordered by insertion time
/// (oldest at index 0, newest at the tail). The in-memory index is the
/// working copy; [`Self::persist`] / [`Self::load`] round-trip it through
/// `.legion/local-history/manifest.json` so a restart can restore entries.
#[derive(Debug, Default)]
pub struct LocalHistoryMetadataStore {
    /// Maps `canonical_path` → time-ordered records (oldest first).
    records: HashMap<String, Vec<LocalHistoryRecord>>,
}

impl LocalHistoryMetadataStore {
    /// Create a new empty store.
    pub fn new() -> Self {
        Self::default()
    }

    /// Push a new record for the file identified by its canonical path.
    /// The record is appended at the tail (newest).
    pub fn push_record(&mut self, record: LocalHistoryRecord) {
        self.records
            .entry(record.canonical_path.clone())
            .or_default()
            .push(record);
    }

    /// Return the most recent records for the given canonical path, up to
    /// `limit` entries, ordered newest-first.
    pub fn records_for_file(&self, canonical_path: &str, limit: usize) -> Vec<&LocalHistoryRecord> {
        let all = match self.records.get(canonical_path) {
            Some(list) => list,
            None => return Vec::new(),
        };
        let start = all.len().saturating_sub(limit);
        // Reverse so caller sees newest first.
        all[start..].iter().rev().collect()
    }

    /// Find a single record by its `entry_id` across all files.
    pub fn find_entry_by_id(&self, entry_id: &str) -> Option<&LocalHistoryRecord> {
        self.records
            .values()
            .flat_map(|v| v.iter())
            .find(|r| r.entry_id == entry_id)
    }

    /// Prune records for the given canonical path to enforce retention limits.
    ///
    /// Removes the oldest entries until both:
    /// - the count is at most `max_count`, and
    /// - the total `size_bytes` is at most `max_size_bytes`.
    ///
    /// Returns the `content_hash` values of every evicted record so the caller
    /// can delete the corresponding on-disk blob files.
    pub fn prune(
        &mut self,
        canonical_path: &str,
        max_count: usize,
        max_size_bytes: u64,
    ) -> Vec<String> {
        let Some(list) = self.records.get_mut(canonical_path) else {
            return Vec::new();
        };
        let mut evicted_hashes = Vec::new();
        // Count cap: remove from front (oldest).
        while list.len() > max_count {
            evicted_hashes.push(list.remove(0).content_hash);
        }
        // Size cap: remove from front until under budget.
        let mut total: u64 = list.iter().map(|r| r.size_bytes).sum();
        while total > max_size_bytes && !list.is_empty() {
            total = total.saturating_sub(list[0].size_bytes);
            evicted_hashes.push(list.remove(0).content_hash);
        }
        evicted_hashes
    }

    /// Return the number of recorded entries for the given path.
    pub fn entry_count(&self, canonical_path: &str) -> usize {
        self.records
            .get(canonical_path)
            .map(|v| v.len())
            .unwrap_or(0)
    }

    /// Directory that holds blobs and `manifest.json` for a workspace root.
    #[must_use]
    pub fn dir_for_workspace(workspace_root: &Path) -> PathBuf {
        workspace_root.join(".legion").join(LOCAL_HISTORY_DIR_NAME)
    }

    /// Sanitized on-disk directory name for one file's blobs.
    #[must_use]
    pub fn path_key(canonical_path: &str) -> String {
        let stripped = canonical_path
            .strip_prefix(r"\\?\")
            .unwrap_or(canonical_path);
        stripped.replace(['/', '\\', ':', '*', '?', '"', '<', '>', '|'], "_")
    }

    /// Load metadata from `dir/manifest.json`.
    ///
    /// Missing directory or missing manifest is an empty store. Records whose
    /// blob files are absent are dropped so the projection does not offer
    /// unrestorable entries. A symlink at `dir` or its `.legion` parent is
    /// rejected.
    pub fn load(dir: &Path) -> Result<Self, StorageError> {
        reject_history_symlink(dir)?;
        if let Some(parent) = dir.parent() {
            reject_history_symlink(parent)?;
        }
        let manifest_path = dir.join(MANIFEST_FILE);
        if !manifest_path.is_file() {
            return Ok(Self::new());
        }
        reject_history_symlink(&manifest_path)?;
        let encoded = fs::read(&manifest_path).map_err(|err| StorageError::Failed {
            message: format!("read local-history manifest failed: {err}"),
        })?;
        let manifest: LocalHistoryManifest =
            serde_json::from_slice(&encoded).map_err(|err| StorageError::Failed {
                message: format!("parse local-history manifest failed: {err}"),
            })?;
        if manifest.schema_version > LOCAL_HISTORY_SCHEMA_VERSION {
            return Err(StorageError::Failed {
                message: format!(
                    "local-history manifest schema {} is newer than {}",
                    manifest.schema_version, LOCAL_HISTORY_SCHEMA_VERSION
                ),
            });
        }
        let mut store = Self::new();
        for record in manifest.records {
            if record.schema_version == 0 {
                continue;
            }
            let blob = dir
                .join(Self::path_key(&record.canonical_path))
                .join(format!("{}.blob", record.content_hash));
            if !blob.is_file() {
                continue;
            }
            store.push_record(record);
        }
        Ok(store)
    }

    /// Write `manifest.json` under `dir`. Empty store still writes a manifest
    /// so a later load does not resurrect a stale file from a previous save.
    pub fn persist(&self, dir: &Path) -> Result<(), StorageError> {
        reject_history_symlink(dir)?;
        if let Some(parent) = dir.parent() {
            reject_history_symlink(parent)?;
        }
        if !dir.exists() {
            fs::create_dir_all(dir).map_err(|err| StorageError::Failed {
                message: format!("create local-history dir failed: {err}"),
            })?;
        }
        let mut records = Vec::new();
        for list in self.records.values() {
            records.extend(list.iter().cloned());
        }
        records.sort_by(|a, b| {
            a.canonical_path
                .cmp(&b.canonical_path)
                .then(a.timestamp_ms.cmp(&b.timestamp_ms))
                .then(a.entry_id.cmp(&b.entry_id))
        });
        let encoded = serde_json::to_vec_pretty(&LocalHistoryManifest {
            schema_version: LOCAL_HISTORY_SCHEMA_VERSION,
            records,
        })
        .map_err(|err| StorageError::Failed {
            message: format!("serialize local-history manifest failed: {err}"),
        })?;
        write_atomically(&dir.join(MANIFEST_FILE), &encoded)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct LocalHistoryManifest {
    schema_version: u32,
    records: Vec<LocalHistoryRecord>,
}

fn is_symlink(path: &Path) -> bool {
    fs::symlink_metadata(path)
        .map(|metadata| metadata.file_type().is_symlink())
        .unwrap_or(false)
}

fn reject_history_symlink(path: &Path) -> Result<(), StorageError> {
    if is_symlink(path) {
        Err(StorageError::Failed {
            message: format!(
                "local-history path is a symlink and was rejected: {}",
                path.display()
            ),
        })
    } else {
        Ok(())
    }
}

fn write_atomically(dest: &Path, body: &[u8]) -> Result<(), StorageError> {
    reject_history_symlink(dest)?;
    let parent = dest.parent().unwrap_or_else(|| Path::new("."));
    reject_history_symlink(parent)?;
    fs::create_dir_all(parent).map_err(|err| StorageError::Failed {
        message: format!("create local-history parent failed: {err}"),
    })?;
    let temp = parent.join(format!(
        ".local-history-{}-{}.tmp",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    ));
    let write_result = (|| -> Result<(), StorageError> {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp)
            .map_err(|err| StorageError::Failed {
                message: format!("create local-history temp failed: {err}"),
            })?;
        file.write_all(body).map_err(|err| StorageError::Failed {
            message: format!("write local-history temp failed: {err}"),
        })?;
        file.sync_all().map_err(|err| StorageError::Failed {
            message: format!("sync local-history temp failed: {err}"),
        })?;
        drop(file);
        atomic_replace(&temp, dest).map_err(|err| StorageError::Failed {
            message: format!("publish local-history file failed: {err}"),
        })
    })();
    if write_result.is_err() {
        let _ = fs::remove_file(&temp);
    }
    write_result
}

#[cfg(windows)]
fn atomic_replace(temp: &Path, target: &Path) -> std::io::Result<()> {
    use std::os::windows::ffi::OsStrExt;

    const MOVEFILE_REPLACE_EXISTING: u32 = 0x0000_0001;
    const MOVEFILE_WRITE_THROUGH: u32 = 0x0000_0008;

    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn MoveFileExW(existing: *const u16, new_name: *const u16, flags: u32) -> i32;
    }

    fn wide(path: &Path) -> Vec<u16> {
        path.as_os_str().encode_wide().chain(Some(0)).collect()
    }

    let ok = unsafe {
        MoveFileExW(
            wide(temp).as_ptr(),
            wide(target).as_ptr(),
            MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
        )
    };
    if ok == 0 {
        Err(std::io::Error::last_os_error())
    } else {
        Ok(())
    }
}

#[cfg(not(windows))]
fn atomic_replace(temp: &Path, target: &Path) -> std::io::Result<()> {
    fs::rename(temp, target)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn make_record(entry_id: &str, path: &str, hash: &str, size: u64) -> LocalHistoryRecord {
        LocalHistoryRecord {
            entry_id: entry_id.to_string(),
            file_id_str: "file-1".to_string(),
            canonical_path: path.to_string(),
            content_hash: hash.to_string(),
            timestamp_ms: 1_000_000,
            correlation_id_str: "corr-1".to_string(),
            size_bytes: size,
            schema_version: LOCAL_HISTORY_SCHEMA_VERSION,
        }
    }

    #[test]
    fn push_and_retrieve_records() {
        let mut store = LocalHistoryMetadataStore::new();
        store.push_record(make_record("e1", "src/a.rs", "hash1", 100));
        store.push_record(make_record("e2", "src/a.rs", "hash2", 200));

        let records = store.records_for_file("src/a.rs", 10);
        assert_eq!(records.len(), 2);
        // Newest first.
        assert_eq!(records[0].entry_id, "e2");
        assert_eq!(records[1].entry_id, "e1");
    }

    #[test]
    fn records_for_file_limits_results() {
        let mut store = LocalHistoryMetadataStore::new();
        for i in 0..10u32 {
            store.push_record(make_record(
                &format!("e{i}"),
                "src/b.rs",
                &format!("h{i}"),
                50,
            ));
        }
        let records = store.records_for_file("src/b.rs", 3);
        assert_eq!(records.len(), 3);
        // Newest first.
        assert_eq!(records[0].entry_id, "e9");
    }

    #[test]
    fn prune_enforces_count_cap() {
        let mut store = LocalHistoryMetadataStore::new();
        for i in 0..10u32 {
            store.push_record(make_record(
                &format!("e{i}"),
                "src/c.rs",
                &format!("h{i}"),
                100,
            ));
        }
        let evicted = store.prune("src/c.rs", 5, u64::MAX);
        // Five oldest entries evicted.
        assert_eq!(evicted.len(), 5);
        assert!(evicted.contains(&"h0".to_string()));
        assert_eq!(store.entry_count("src/c.rs"), 5);
        // Oldest entries should have been removed.
        let records = store.records_for_file("src/c.rs", 10);
        assert_eq!(records[0].entry_id, "e9"); // newest
    }

    #[test]
    fn prune_enforces_size_cap() {
        let mut store = LocalHistoryMetadataStore::new();
        for i in 0..5u32 {
            store.push_record(make_record(
                &format!("e{i}"),
                "src/d.rs",
                &format!("h{i}"),
                200,
            ));
        }
        // Cap at 400 bytes: should keep 2 newest.
        let evicted = store.prune("src/d.rs", 100, 400);
        assert_eq!(evicted.len(), 3);
        assert_eq!(store.entry_count("src/d.rs"), 2);
    }

    #[test]
    fn prune_returns_evicted_content_hashes() {
        let mut store = LocalHistoryMetadataStore::new();
        store.push_record(make_record("e1", "src/g.rs", "old-hash-1", 50));
        store.push_record(make_record("e2", "src/g.rs", "old-hash-2", 50));
        store.push_record(make_record("e3", "src/g.rs", "keep-hash", 50));

        // Cap to 1 entry; oldest two should be evicted.
        let evicted = store.prune("src/g.rs", 1, u64::MAX);
        assert_eq!(evicted.len(), 2);
        assert!(evicted.contains(&"old-hash-1".to_string()));
        assert!(evicted.contains(&"old-hash-2".to_string()));
        assert!(!evicted.contains(&"keep-hash".to_string()));
        assert_eq!(store.entry_count("src/g.rs"), 1);
    }

    #[test]
    fn find_entry_by_id_across_files() {
        let mut store = LocalHistoryMetadataStore::new();
        store.push_record(make_record("target-id", "src/e.rs", "hashX", 50));
        store.push_record(make_record("other-id", "src/f.rs", "hashY", 50));

        let found = store.find_entry_by_id("target-id");
        assert!(found.is_some());
        assert_eq!(found.unwrap().canonical_path, "src/e.rs");

        assert!(store.find_entry_by_id("missing").is_none());
    }

    #[test]
    fn persist_round_trips_records_with_blobs() {
        let root = std::env::temp_dir().join(format!(
            "legion-lh-meta-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("time")
                .as_nanos()
        ));
        let dir = root.join("local-history");
        fs::create_dir_all(&dir).expect("dir");
        let mut store = LocalHistoryMetadataStore::new();
        store.push_record(make_record("e1", "src/a.rs", "hash1", 100));
        let blob_dir = dir.join(LocalHistoryMetadataStore::path_key("src/a.rs"));
        fs::create_dir_all(&blob_dir).expect("blob dir");
        fs::write(blob_dir.join("hash1.blob"), b"fn a() {}\n").expect("blob");
        store.persist(&dir).expect("persist");
        let loaded = LocalHistoryMetadataStore::load(&dir).expect("load");
        let records = loaded.records_for_file("src/a.rs", 10);
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].entry_id, "e1");
        assert_eq!(records[0].content_hash, "hash1");
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn load_drops_records_whose_blobs_are_missing() {
        let root = std::env::temp_dir().join(format!(
            "legion-lh-orphan-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("time")
                .as_nanos()
        ));
        let dir = root.join("local-history");
        fs::create_dir_all(&dir).expect("dir");
        let mut store = LocalHistoryMetadataStore::new();
        store.push_record(make_record("keep", "src/a.rs", "keep-hash", 10));
        store.push_record(make_record("gone", "src/a.rs", "gone-hash", 10));
        let blob_dir = dir.join(LocalHistoryMetadataStore::path_key("src/a.rs"));
        fs::create_dir_all(&blob_dir).expect("blob dir");
        fs::write(blob_dir.join("keep-hash.blob"), b"keep\n").expect("blob");
        store.persist(&dir).expect("persist");
        let loaded = LocalHistoryMetadataStore::load(&dir).expect("load");
        let records = loaded.records_for_file("src/a.rs", 10);
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].entry_id, "keep");
        let _ = fs::remove_dir_all(root);
    }
}
