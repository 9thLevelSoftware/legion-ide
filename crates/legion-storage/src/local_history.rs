//! Local history metadata store.
//!
//! Keeps a bounded in-memory index of local file history snapshots.
//! The authoritative content blobs live on disk in the workspace state
//! directory (`.legion/local-history/<path_key>/<content_hash>.blob`);
//! this module persists only the metadata (identity, hash, timestamp,
//! correlation id) so audit records stay metadata-only.

use std::{
    collections::HashMap,
    fs::{self, File, OpenOptions},
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};

use super::StorageError;

/// Schema version for local history records and the on-disk manifest.
pub const LOCAL_HISTORY_SCHEMA_VERSION: u32 = 1;

/// Directory name under `.legion/` for blobs and the metadata manifest.
pub const LOCAL_HISTORY_DIR_NAME: &str = "local-history";

/// SHA-256 hex digest length (`{:x}` of 32 bytes).
pub const CONTENT_HASH_HEX_LEN: usize = 64;

const MANIFEST_FILE: &str = "manifest.json";
const LOCK_FILE: &str = "manifest.lock";

/// True when `hash` is a 64-character lowercase SHA-256 hex digest.
#[must_use]
pub fn is_sha256_content_hash(hash: &str) -> bool {
    hash.len() == CONTENT_HASH_HEX_LEN
        && hash.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
}

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

    /// Resolve a blob path after validating the hash and rejecting symlink
    /// or path-escape attempts. The returned path is contained under `dir`.
    pub fn trusted_blob_path(
        dir: &Path,
        canonical_path: &str,
        content_hash: &str,
    ) -> Result<PathBuf, StorageError> {
        if !is_sha256_content_hash(content_hash) {
            return Err(StorageError::Failed {
                message: format!("local-history content hash is not SHA-256 hex: {content_hash}"),
            });
        }
        let key = Self::path_key(canonical_path);
        if key.is_empty() || key == "." || key == ".." {
            return Err(StorageError::Failed {
                message: "local-history path key is not a single safe component".to_string(),
            });
        }
        reject_history_symlink(dir)?;
        let file_dir = dir.join(&key);
        reject_history_symlink(&file_dir)?;
        let blob = file_dir.join(format!("{content_hash}.blob"));
        reject_history_symlink(&blob)?;
        if !blob.starts_with(dir) {
            return Err(StorageError::Failed {
                message: format!(
                    "local-history blob escaped history directory: {}",
                    blob.display()
                ),
            });
        }
        if blob.exists() {
            let canonical_dir = fs::canonicalize(dir).map_err(|err| StorageError::Failed {
                message: format!("canonicalize local-history dir failed: {err}"),
            })?;
            let canonical_blob = fs::canonicalize(&blob).map_err(|err| StorageError::Failed {
                message: format!("canonicalize local-history blob failed: {err}"),
            })?;
            if !canonical_blob.starts_with(&canonical_dir) {
                return Err(StorageError::Failed {
                    message: format!(
                        "local-history blob resolved outside history directory: {}",
                        canonical_blob.display()
                    ),
                });
            }
        }
        Ok(blob)
    }

    /// Load metadata from `dir/manifest.json`.
    ///
    /// Missing directory or missing manifest is an empty store. Records whose
    /// blob files are absent, whose content hash is not SHA-256 hex, or whose
    /// blob/path-key is a symlink or escapes `dir` are dropped so the
    /// projection does not offer unrestorable or untrusted entries. A symlink
    /// at `dir` or its `.legion` parent is rejected.
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
            let Ok(blob) =
                Self::trusted_blob_path(dir, &record.canonical_path, &record.content_hash)
            else {
                continue;
            };
            let Ok(metadata) = fs::symlink_metadata(&blob) else {
                continue;
            };
            if !metadata.is_file() {
                continue;
            }
            store.push_record(record);
        }
        Ok(store)
    }

    /// Write `manifest.json` under `dir`.
    ///
    /// Takes an inter-process lock, reloads the on-disk index, and unions
    /// this store's records so a concurrent Legion process cannot replace
    /// the other's metadata. Each per-path list is serialized in insertion
    /// order (oldest first), not by wall-clock timestamp. Empty store still
    /// writes a manifest so a later load does not resurrect a stale file.
    ///
    /// One retry is applied on I/O failure; the caller should treat a
    /// remaining error as "this snapshot was not committed".
    pub fn persist(&mut self, dir: &Path) -> Result<(), StorageError> {
        match self.persist_once(dir) {
            Ok(()) => Ok(()),
            Err(_) => self.persist_once(dir),
        }
    }

    fn persist_once(&mut self, dir: &Path) -> Result<(), StorageError> {
        reject_history_symlink(dir)?;
        if let Some(parent) = dir.parent() {
            reject_history_symlink(parent)?;
        }
        if !dir.exists() {
            fs::create_dir_all(dir).map_err(|err| StorageError::Failed {
                message: format!("create local-history dir failed: {err}"),
            })?;
        }
        let _lock = acquire_manifest_lock(dir)?;
        let disk = Self::load(dir)?;
        let mut merged = disk;
        for record in self.records_in_stable_order() {
            if merged.find_entry_by_id(&record.entry_id).is_none() {
                merged.push_record(record.clone());
            }
        }
        merged.write_manifest_unlocked(dir)?;
        *self = merged;
        Ok(())
    }

    fn write_manifest_unlocked(&self, dir: &Path) -> Result<(), StorageError> {
        let encoded = serde_json::to_vec_pretty(&LocalHistoryManifest {
            schema_version: LOCAL_HISTORY_SCHEMA_VERSION,
            records: self.records_in_stable_order(),
        })
        .map_err(|err| StorageError::Failed {
            message: format!("serialize local-history manifest failed: {err}"),
        })?;
        crate::fs_atomic::write_atomically_with_symlink_guard(
            &dir.join(MANIFEST_FILE),
            "local-history",
            &encoded,
            "local-history",
        )
    }

    fn records_in_stable_order(&self) -> Vec<LocalHistoryRecord> {
        let mut paths: Vec<&String> = self.records.keys().collect();
        paths.sort();
        let mut records = Vec::new();
        for path in paths {
            if let Some(list) = self.records.get(path) {
                records.extend(list.iter().cloned());
            }
        }
        records
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct LocalHistoryManifest {
    schema_version: u32,
    records: Vec<LocalHistoryRecord>,
}

fn reject_history_symlink(path: &Path) -> Result<(), StorageError> {
    crate::fs_atomic::reject_symlink(path, "local-history")
}

fn acquire_manifest_lock(dir: &Path) -> Result<File, StorageError> {
    let lock_path = dir.join(LOCK_FILE);
    reject_history_symlink(&lock_path)?;
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(&lock_path)
        .map_err(|err| StorageError::Failed {
            message: format!("open local-history lock failed: {err}"),
        })?;
    file.lock().map_err(|err| StorageError::Failed {
        message: format!("lock local-history manifest failed: {err}"),
    })?;
    Ok(file)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn hash_n(n: u32) -> String {
        format!("{n:064x}")
    }

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

    fn unique_dir(label: &str) -> PathBuf {
        let root = std::env::temp_dir().join(format!(
            "legion-lh-{label}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("time")
                .as_nanos()
        ));
        let dir = root.join(LOCAL_HISTORY_DIR_NAME);
        fs::create_dir_all(&dir).expect("dir");
        dir
    }

    fn write_blob(dir: &Path, canonical_path: &str, hash: &str, body: &[u8]) {
        let blob_dir = dir.join(LocalHistoryMetadataStore::path_key(canonical_path));
        fs::create_dir_all(&blob_dir).expect("blob dir");
        fs::write(blob_dir.join(format!("{hash}.blob")), body).expect("blob");
    }

    #[test]
    fn push_and_retrieve_records() {
        let mut store = LocalHistoryMetadataStore::new();
        store.push_record(make_record("e1", "src/a.rs", &hash_n(1), 100));
        store.push_record(make_record("e2", "src/a.rs", &hash_n(2), 200));

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
            store.push_record(make_record(&format!("e{i}"), "src/b.rs", &hash_n(i), 50));
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
            store.push_record(make_record(&format!("e{i}"), "src/c.rs", &hash_n(i), 100));
        }
        let evicted = store.prune("src/c.rs", 5, u64::MAX);
        // Five oldest entries evicted.
        assert_eq!(evicted.len(), 5);
        assert!(evicted.contains(&hash_n(0)));
        assert_eq!(store.entry_count("src/c.rs"), 5);
        // Oldest entries should have been removed.
        let records = store.records_for_file("src/c.rs", 10);
        assert_eq!(records[0].entry_id, "e9"); // newest
    }

    #[test]
    fn prune_enforces_size_cap() {
        let mut store = LocalHistoryMetadataStore::new();
        for i in 0..5u32 {
            store.push_record(make_record(&format!("e{i}"), "src/d.rs", &hash_n(i), 200));
        }
        // Cap at 400 bytes: should keep 2 newest.
        let evicted = store.prune("src/d.rs", 100, 400);
        assert_eq!(evicted.len(), 3);
        assert_eq!(store.entry_count("src/d.rs"), 2);
    }

    #[test]
    fn prune_returns_evicted_content_hashes() {
        let mut store = LocalHistoryMetadataStore::new();
        store.push_record(make_record("e1", "src/g.rs", &hash_n(1), 50));
        store.push_record(make_record("e2", "src/g.rs", &hash_n(2), 50));
        store.push_record(make_record("e3", "src/g.rs", &hash_n(3), 50));

        // Cap to 1 entry; oldest two should be evicted.
        let evicted = store.prune("src/g.rs", 1, u64::MAX);
        assert_eq!(evicted.len(), 2);
        assert!(evicted.contains(&hash_n(1)));
        assert!(evicted.contains(&hash_n(2)));
        assert!(!evicted.contains(&hash_n(3)));
        assert_eq!(store.entry_count("src/g.rs"), 1);
    }

    #[test]
    fn find_entry_by_id_across_files() {
        let mut store = LocalHistoryMetadataStore::new();
        store.push_record(make_record("target-id", "src/e.rs", &hash_n(10), 50));
        store.push_record(make_record("other-id", "src/f.rs", &hash_n(11), 50));

        let found = store.find_entry_by_id("target-id");
        assert!(found.is_some());
        assert_eq!(found.unwrap().canonical_path, "src/e.rs");

        assert!(store.find_entry_by_id("missing").is_none());
    }

    #[test]
    fn persist_round_trips_records_with_blobs() {
        let dir = unique_dir("meta");
        let mut store = LocalHistoryMetadataStore::new();
        store.push_record(make_record("e1", "src/a.rs", &hash_n(1), 100));
        write_blob(&dir, "src/a.rs", &hash_n(1), b"fn a() {}\n");
        store.persist(&dir).expect("persist");
        let loaded = LocalHistoryMetadataStore::load(&dir).expect("load");
        let records = loaded.records_for_file("src/a.rs", 10);
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].entry_id, "e1");
        assert_eq!(records[0].content_hash, hash_n(1));
        let _ = fs::remove_dir_all(dir.parent().expect("root"));
    }

    #[test]
    fn load_drops_records_whose_blobs_are_missing() {
        let dir = unique_dir("orphan");
        let mut store = LocalHistoryMetadataStore::new();
        store.push_record(make_record("keep", "src/a.rs", &hash_n(1), 10));
        store.push_record(make_record("gone", "src/a.rs", &hash_n(2), 10));
        write_blob(&dir, "src/a.rs", &hash_n(1), b"keep\n");
        store.persist(&dir).expect("persist");
        let loaded = LocalHistoryMetadataStore::load(&dir).expect("load");
        let records = loaded.records_for_file("src/a.rs", 10);
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].entry_id, "keep");
        let _ = fs::remove_dir_all(dir.parent().expect("root"));
    }

    #[test]
    fn persist_preserves_insertion_order_when_clock_moves_backward() {
        let dir = unique_dir("order");
        let mut first = make_record("older", "src/a.rs", &hash_n(1), 10);
        first.timestamp_ms = 2_000;
        let mut second = make_record("newer", "src/a.rs", &hash_n(2), 10);
        second.timestamp_ms = 1_000;
        let mut store = LocalHistoryMetadataStore::new();
        store.push_record(first);
        store.push_record(second);
        write_blob(&dir, "src/a.rs", &hash_n(1), b"one\n");
        write_blob(&dir, "src/a.rs", &hash_n(2), b"two\n");
        store.persist(&dir).expect("persist");
        let loaded = LocalHistoryMetadataStore::load(&dir).expect("load");
        let records = loaded.records_for_file("src/a.rs", 10);
        assert_eq!(records.len(), 2);
        assert_eq!(records[0].entry_id, "newer");
        assert_eq!(records[1].entry_id, "older");
        let _ = fs::remove_dir_all(dir.parent().expect("root"));
    }

    #[test]
    fn persist_merges_concurrent_in_memory_snapshots() {
        let dir = unique_dir("merge");
        let mut first = LocalHistoryMetadataStore::new();
        first.push_record(make_record("from-a", "src/a.rs", &hash_n(1), 10));
        write_blob(&dir, "src/a.rs", &hash_n(1), b"a\n");
        first.persist(&dir).expect("persist a");

        let mut second = LocalHistoryMetadataStore::new();
        second.push_record(make_record("from-b", "src/a.rs", &hash_n(2), 10));
        write_blob(&dir, "src/a.rs", &hash_n(2), b"b\n");
        second.persist(&dir).expect("persist b");

        let loaded = LocalHistoryMetadataStore::load(&dir).expect("load");
        let records = loaded.records_for_file("src/a.rs", 10);
        let ids: Vec<&str> = records.iter().map(|r| r.entry_id.as_str()).collect();
        assert!(
            ids.contains(&"from-a"),
            "disk records must survive: {ids:?}"
        );
        assert!(
            ids.contains(&"from-b"),
            "new records must be added: {ids:?}"
        );
        let _ = fs::remove_dir_all(dir.parent().expect("root"));
    }

    #[test]
    fn load_rejects_non_hex_content_hash() {
        let dir = unique_dir("bad-hash");
        let escape = dir.parent().expect("root").join("escaped.blob");
        fs::write(&escape, b"stolen\n").expect("escape target");
        let manifest = serde_json::json!({
            "schema_version": 1,
            "records": [{
                "entry_id": "evil",
                "file_id_str": "file-1",
                "canonical_path": "src/a.rs",
                "content_hash": "../../escaped",
                "timestamp_ms": 1,
                "correlation_id_str": "corr-1",
                "size_bytes": 7,
                "schema_version": 1
            }]
        });
        fs::write(dir.join(MANIFEST_FILE), manifest.to_string()).expect("manifest");
        let loaded = LocalHistoryMetadataStore::load(&dir).expect("load");
        assert!(loaded.find_entry_by_id("evil").is_none());
        let _ = fs::remove_dir_all(dir.parent().expect("root"));
    }

    #[cfg(unix)]
    #[test]
    fn load_rejects_symlinked_blob() {
        let dir = unique_dir("sym-blob");
        let mut store = LocalHistoryMetadataStore::new();
        store.push_record(make_record("e1", "src/a.rs", &hash_n(1), 10));
        write_blob(&dir, "src/a.rs", &hash_n(1), b"ok\n");
        store.persist(&dir).expect("persist");
        let blob = dir
            .join(LocalHistoryMetadataStore::path_key("src/a.rs"))
            .join(format!("{}.blob", hash_n(1)));
        fs::remove_file(&blob).expect("remove blob");
        let outside = dir.parent().expect("root").join("outside.txt");
        fs::write(&outside, b"external\n").expect("outside");
        std::os::unix::fs::symlink(&outside, &blob).expect("symlink blob");
        let loaded = LocalHistoryMetadataStore::load(&dir).expect("load");
        assert!(
            loaded.find_entry_by_id("e1").is_none(),
            "symlinked blob must not be admitted"
        );
        let _ = fs::remove_dir_all(dir.parent().expect("root"));
    }
}
