//! Local history metadata store.
//!
//! Keeps a bounded in-memory index of local file history snapshots.
//! The authoritative content blobs live on disk in the workspace state
//! directory (`.legion/local-history/<file_id>/<content_hash>.blob`);
//! this module persists only the metadata (identity, hash, timestamp,
//! correlation id) so audit records stay metadata-only.

use std::collections::HashMap;

/// Schema version for local history records.
pub const LOCAL_HISTORY_SCHEMA_VERSION: u32 = 1;

/// Metadata record for one local file history snapshot.
///
/// The actual file content is stored as a content-addressed blob on disk;
/// only identity metadata is held here.
#[derive(Debug, Clone)]
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

/// In-memory local history metadata store with bounded retention.
///
/// Records are keyed by canonical file path and ordered by insertion time
/// (oldest at index 0, newest at the tail). The store is intentionally
/// in-memory only; persistence of history metadata across sessions is deferred
/// to a future migration step (see GIT.09 deferred-features note).
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
    pub fn prune(&mut self, canonical_path: &str, max_count: usize, max_size_bytes: u64) {
        let Some(list) = self.records.get_mut(canonical_path) else {
            return;
        };
        // Count cap: remove from front (oldest).
        while list.len() > max_count {
            list.remove(0);
        }
        // Size cap: remove from front until under budget.
        let mut total: u64 = list.iter().map(|r| r.size_bytes).sum();
        while total > max_size_bytes && !list.is_empty() {
            total = total.saturating_sub(list[0].size_bytes);
            list.remove(0);
        }
    }

    /// Return the number of recorded entries for the given path.
    pub fn entry_count(&self, canonical_path: &str) -> usize {
        self.records
            .get(canonical_path)
            .map(|v| v.len())
            .unwrap_or(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
        store.prune("src/c.rs", 5, u64::MAX);
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
        store.prune("src/d.rs", 100, 400);
        assert_eq!(store.entry_count("src/d.rs"), 2);
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
}
