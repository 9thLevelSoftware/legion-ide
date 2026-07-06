//! Durable checkpoint store for workspace-level file-mutation rollback.
//!
//! Persists `DurableCheckpoint` blobs as individual JSON files under
//! `.legion/checkpoints/` and audit records under `.legion/audit/`.
//! When no `base_dir` is configured the store is purely in-memory,
//! which is the default for tests that do not call
//! `AppComposition::enable_checkpoint_persistence`.

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

use legion_protocol::{
    CanonicalPath, CheckpointAuditEvent, CheckpointAuditRecord, PrincipalId, ProposalId,
    TimestampMillis,
};
use serde::{Deserialize, Serialize};

use super::StorageError;

// ---------------------------------------------------------------------------
// Schema version
// ---------------------------------------------------------------------------

/// Schema version for durable checkpoint blobs.
pub const CHECKPOINT_SCHEMA_VERSION: u32 = 1;

// ---------------------------------------------------------------------------
// Data types
// ---------------------------------------------------------------------------

/// Kind of file mutation that a checkpoint target captures.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum CheckpointTargetKind {
    /// The proposal created this file; restoring deletes it.
    CreatedFile,
    /// The proposal deleted this file; restoring recreates it with `content_before`.
    DeletedFile,
    /// The proposal saved (overwrote) this file; restoring writes `content_before` back.
    SavedFile,
    /// The proposal renamed this file to the stored `path`; restoring moves it back.
    RenamedFile {
        /// Original canonical path before the rename.
        original_path: CanonicalPath,
    },
}

/// A single file that was mutated by the proposal and can be individually restored.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CheckpointTarget {
    /// Stable target identifier (UUID v4 string).
    pub target_id: String,
    /// Kind of mutation this target captures.
    pub kind: CheckpointTargetKind,
    /// Canonical path of the file after the proposal was applied.
    /// For `RenamedFile` this is the *destination* path.
    pub path: CanonicalPath,
    /// Pre-mutation file content, if applicable.
    /// - `None` for `CreatedFile` (nothing to restore to).
    /// - `Some(_)` for `DeletedFile`, `SavedFile`, and `RenamedFile`.
    pub content_before: Option<String>,
}

/// A durable, persistable checkpoint capturing the pre-mutation state for one
/// proposal apply.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DurableCheckpoint {
    /// Stable checkpoint identifier.
    pub checkpoint_id: String,
    /// Proposal that was applied and whose mutations this checkpoint covers.
    pub proposal_id: ProposalId,
    /// Principal that applied the proposal.
    pub principal: PrincipalId,
    /// Unix timestamp (milliseconds) when the checkpoint was created.
    pub created_at: TimestampMillis,
    /// Individual file targets covered by this checkpoint.
    pub targets: Vec<CheckpointTarget>,
    /// Whether the checkpoint is still available for restore.
    /// Set to `false` once a restore has been performed.
    pub available: bool,
    /// Schema version for forward compatibility.
    pub schema_version: u32,
}

/// Lightweight summary returned by `CheckpointStore::list_checkpoints`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DurableCheckpointSummary {
    /// Stable checkpoint identifier.
    pub checkpoint_id: String,
    /// Proposal that was applied.
    pub proposal_id: ProposalId,
    /// Principal that applied the proposal.
    pub principal: PrincipalId,
    /// Unix timestamp (milliseconds) when the checkpoint was created.
    pub created_at: TimestampMillis,
    /// Number of file targets in the checkpoint.
    pub target_count: usize,
    /// Whether the checkpoint is still available for restore.
    pub available: bool,
}

// ---------------------------------------------------------------------------
// CheckpointStore
// ---------------------------------------------------------------------------

/// File-backed (or in-memory when no path is configured) checkpoint store.
///
/// Checkpoints are stored as individual JSON files in
/// `<base_dir>/checkpoints/<id>.json`.  Audit records are stored in
/// `<base_dir>/audit/ckpt-<id>-<ts>.json`.
#[derive(Debug, Default)]
pub struct CheckpointStore {
    /// Optional workspace-local state directory root (`.legion/`).
    /// When `None` the store is in-memory only.
    base_dir: Option<PathBuf>,
    /// In-memory index sorted by `created_at` ascending.
    checkpoints: Vec<DurableCheckpoint>,
    /// In-memory audit record log.
    audits: Vec<CheckpointAuditRecord>,
}

impl CheckpointStore {
    /// Create a new in-memory-only checkpoint store (no disk persistence).
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a checkpoint store backed by the given workspace-local directory.
    ///
    /// Existing checkpoints are loaded from disk; any file that fails to parse
    /// is silently skipped so a single corrupt file does not block the store.
    pub fn with_base_dir(base_dir: impl AsRef<Path>) -> Self {
        let base_dir = base_dir.as_ref().to_path_buf();
        let mut store = Self {
            base_dir: Some(base_dir.clone()),
            checkpoints: Vec::new(),
            audits: Vec::new(),
        };
        store.load_from_disk();
        store
    }

    // -----------------------------------------------------------------------
    // Checkpoint CRUD
    // -----------------------------------------------------------------------

    /// Persist a checkpoint to the store (and to disk when a base directory is
    /// configured).
    ///
    /// If a checkpoint with the same `checkpoint_id` already exists in memory
    /// it is replaced (update semantics).
    pub fn save_checkpoint(&mut self, checkpoint: DurableCheckpoint) -> Result<(), StorageError> {
        if let Some(path) = self.checkpoint_path(&checkpoint.checkpoint_id) {
            let body =
                serde_json::to_string_pretty(&checkpoint).map_err(|err| StorageError::Failed {
                    message: format!("serialize checkpoint failed: {err}"),
                })?;
            write_atomically(&path, body.as_bytes())?;
        }
        self.checkpoints
            .retain(|c| c.checkpoint_id != checkpoint.checkpoint_id);
        self.checkpoints.push(checkpoint);
        self.checkpoints.sort_by_key(|c| c.created_at.0);
        Ok(())
    }

    /// Load a single checkpoint by identifier.
    ///
    /// Returns `None` when no checkpoint with that id is held in the store.
    pub fn load_checkpoint(
        &self,
        checkpoint_id: &str,
    ) -> Result<Option<DurableCheckpoint>, StorageError> {
        Ok(self
            .checkpoints
            .iter()
            .find(|c| c.checkpoint_id == checkpoint_id)
            .cloned())
    }

    /// List all checkpoints as lightweight summaries, sorted newest-first.
    pub fn list_checkpoints(&self) -> Vec<DurableCheckpointSummary> {
        self.checkpoints
            .iter()
            .rev()
            .map(|c| DurableCheckpointSummary {
                checkpoint_id: c.checkpoint_id.clone(),
                proposal_id: c.proposal_id,
                principal: c.principal.clone(),
                created_at: c.created_at,
                target_count: c.targets.len(),
                available: c.available,
            })
            .collect()
    }

    /// Delete a checkpoint by identifier.
    ///
    /// Silently succeeds when the checkpoint does not exist.
    pub fn delete_checkpoint(&mut self, checkpoint_id: &str) -> Result<(), StorageError> {
        if let Some(path) = self.checkpoint_path(checkpoint_id) {
            let _ = fs::remove_file(path);
        }
        self.checkpoints
            .retain(|c| c.checkpoint_id != checkpoint_id);
        Ok(())
    }

    /// Mark a checkpoint as unavailable (consumed by a restore).
    ///
    /// The checkpoint remains in the store but `available` becomes `false`.
    pub fn mark_unavailable(&mut self, checkpoint_id: &str) {
        // Resolve the path before the mutable borrow.
        let path = self.checkpoint_path(checkpoint_id);
        if let Some(cp) = self
            .checkpoints
            .iter_mut()
            .find(|c| c.checkpoint_id == checkpoint_id)
        {
            cp.available = false;
            // Persist the update.
            if let Some(ref p) = path
                && let Ok(body) = serde_json::to_string_pretty(cp)
            {
                let _ = write_atomically(p, body.as_bytes());
            }
        }
    }

    // -----------------------------------------------------------------------
    // Audit
    // -----------------------------------------------------------------------

    /// Append a checkpoint audit record to the store.
    ///
    /// When a base directory is configured the record is also written to
    /// `.legion/audit/`.
    pub fn save_audit_record(&mut self, record: CheckpointAuditRecord) -> Result<(), StorageError> {
        if let Some(dir) = &self.base_dir {
            let audit_dir = dir.join("audit");
            fs::create_dir_all(&audit_dir).map_err(|err| StorageError::Failed {
                message: format!("create audit directory failed: {err}"),
            })?;
            let event_tag = match record.event {
                CheckpointAuditEvent::Created => "created",
                CheckpointAuditEvent::Restored => "restored",
                CheckpointAuditEvent::Deleted => "deleted",
            };
            let filename = format!(
                "ckpt-{}-{}-{}.json",
                record.checkpoint_id, record.timestamp.0, event_tag
            );
            let path = audit_dir.join(filename);
            let body =
                serde_json::to_string_pretty(&record).map_err(|err| StorageError::Failed {
                    message: format!("serialize audit record failed: {err}"),
                })?;
            write_atomically(&path, body.as_bytes())?;
        }
        self.audits.push(record);
        Ok(())
    }

    /// Query audit records, optionally filtered by proposal identifier.
    ///
    /// Returns all records when `proposal_id` is `None`.
    pub fn query_checkpoint_audit(
        &self,
        proposal_id: Option<ProposalId>,
    ) -> Vec<CheckpointAuditRecord> {
        match proposal_id {
            None => self.audits.clone(),
            Some(pid) => self
                .audits
                .iter()
                .filter(|a| a.proposal_id == pid)
                .cloned()
                .collect(),
        }
    }

    // -----------------------------------------------------------------------
    // Private helpers
    // -----------------------------------------------------------------------

    fn checkpoint_dir(&self) -> Option<PathBuf> {
        self.base_dir.as_ref().map(|d| d.join("checkpoints"))
    }

    fn checkpoint_path(&self, checkpoint_id: &str) -> Option<PathBuf> {
        self.checkpoint_dir()
            .map(|d| d.join(format!("{checkpoint_id}.json")))
    }

    /// Load all valid checkpoint JSON blobs from `<base_dir>/checkpoints/`.
    fn load_from_disk(&mut self) {
        let Some(dir) = self.checkpoint_dir() else {
            return;
        };
        let Ok(entries) = fs::read_dir(&dir) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("json") {
                continue;
            }
            if let Ok(bytes) = fs::read(&path)
                && let Ok(cp) = serde_json::from_slice::<DurableCheckpoint>(&bytes)
            {
                self.checkpoints.push(cp);
            }
        }
        self.checkpoints.sort_by_key(|c| c.created_at.0);

        // Load audit records similarly.
        if let Some(audit_dir) = self.base_dir.as_ref().map(|d| d.join("audit"))
            && let Ok(entries) = fs::read_dir(&audit_dir)
        {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().and_then(|e| e.to_str()) != Some("json") {
                    continue;
                }
                if let Ok(bytes) = fs::read(&path)
                    && let Ok(record) = serde_json::from_slice::<CheckpointAuditRecord>(&bytes)
                {
                    self.audits.push(record);
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Atomic write helper (mirrors the pattern used in FileBackedStorage)
// ---------------------------------------------------------------------------

fn write_atomically(dest: &Path, body: &[u8]) -> Result<(), StorageError> {
    let parent = dest.parent().unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent).map_err(|err| StorageError::Failed {
        message: format!("create checkpoint directory failed: {err}"),
    })?;

    let suffix = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let temp = parent.join(format!(".ckpt-tmp-{}-{}.tmp", std::process::id(), suffix));

    let write_result = (|| -> Result<(), StorageError> {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp)
            .map_err(|err| StorageError::Failed {
                message: format!("create checkpoint temp file failed: {err}"),
            })?;
        file.write_all(body).map_err(|err| StorageError::Failed {
            message: format!("write checkpoint temp file failed: {err}"),
        })?;
        file.flush().map_err(|err| StorageError::Failed {
            message: format!("flush checkpoint temp file failed: {err}"),
        })?;
        drop(file);
        fs::rename(&temp, dest).map_err(|err| StorageError::Failed {
            message: format!("rename checkpoint temp file failed: {err}"),
        })
    })();

    if write_result.is_err() {
        let _ = fs::remove_file(&temp);
    }
    write_result
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use legion_protocol::{CheckpointAuditEvent, TimestampMillis};

    fn make_checkpoint(id: &str, proposal_id: u64, ts: u64) -> DurableCheckpoint {
        DurableCheckpoint {
            checkpoint_id: id.to_string(),
            proposal_id: ProposalId(proposal_id),
            principal: PrincipalId("test-principal".to_string()),
            created_at: TimestampMillis(ts),
            targets: vec![CheckpointTarget {
                target_id: format!("target-{id}"),
                kind: CheckpointTargetKind::SavedFile,
                path: CanonicalPath(format!("/tmp/{id}.txt")),
                content_before: Some("before".to_string()),
            }],
            available: true,
            schema_version: CHECKPOINT_SCHEMA_VERSION,
        }
    }

    #[test]
    fn save_load_roundtrip() {
        let mut store = CheckpointStore::new();
        let cp = make_checkpoint("ckpt-1", 1, 1000);
        store.save_checkpoint(cp.clone()).expect("save");
        let loaded = store.load_checkpoint("ckpt-1").expect("load");
        assert_eq!(loaded, Some(cp));
    }

    #[test]
    fn list_ordering_newest_first() {
        let mut store = CheckpointStore::new();
        store
            .save_checkpoint(make_checkpoint("ckpt-a", 1, 1000))
            .unwrap();
        store
            .save_checkpoint(make_checkpoint("ckpt-b", 2, 2000))
            .unwrap();
        store
            .save_checkpoint(make_checkpoint("ckpt-c", 3, 3000))
            .unwrap();
        let list = store.list_checkpoints();
        assert_eq!(list.len(), 3);
        assert_eq!(list[0].checkpoint_id, "ckpt-c");
        assert_eq!(list[1].checkpoint_id, "ckpt-b");
        assert_eq!(list[2].checkpoint_id, "ckpt-a");
    }

    #[test]
    fn delete_removes_checkpoint() {
        let mut store = CheckpointStore::new();
        store
            .save_checkpoint(make_checkpoint("ckpt-x", 1, 1000))
            .unwrap();
        assert_eq!(store.list_checkpoints().len(), 1);
        store.delete_checkpoint("ckpt-x").unwrap();
        assert_eq!(store.list_checkpoints().len(), 0);
        assert_eq!(store.load_checkpoint("ckpt-x").unwrap(), None);
    }

    #[test]
    fn save_load_roundtrip_with_disk() {
        let tmp = std::env::temp_dir().join(format!(
            "legion-checkpoint-store-test-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |d| d.as_millis() as u64)
        ));
        {
            let mut store = CheckpointStore::with_base_dir(&tmp);
            store
                .save_checkpoint(make_checkpoint("ckpt-disk-1", 10, 5000))
                .expect("save to disk");
        }
        // Re-open from disk.
        let store2 = CheckpointStore::with_base_dir(&tmp);
        let loaded = store2.load_checkpoint("ckpt-disk-1").unwrap();
        assert!(loaded.is_some());
        assert_eq!(loaded.unwrap().proposal_id, ProposalId(10));
        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn audit_save_and_query() {
        let mut store = CheckpointStore::new();
        let record = CheckpointAuditRecord {
            checkpoint_id: "ckpt-audit-1".to_string(),
            event: CheckpointAuditEvent::Created,
            proposal_id: ProposalId(42),
            target_paths: vec![CanonicalPath("/tmp/file.txt".to_string())],
            timestamp: TimestampMillis(9000),
            schema_version: 1,
        };
        store.save_audit_record(record.clone()).unwrap();
        let all = store.query_checkpoint_audit(None);
        assert_eq!(all.len(), 1);
        let by_proposal = store.query_checkpoint_audit(Some(ProposalId(42)));
        assert_eq!(by_proposal.len(), 1);
        let other = store.query_checkpoint_audit(Some(ProposalId(99)));
        assert!(other.is_empty());
    }

    #[test]
    fn mark_unavailable() {
        let mut store = CheckpointStore::new();
        store
            .save_checkpoint(make_checkpoint("ckpt-avail", 1, 1000))
            .unwrap();
        assert!(
            store
                .load_checkpoint("ckpt-avail")
                .unwrap()
                .unwrap()
                .available
        );
        store.mark_unavailable("ckpt-avail");
        assert!(
            !store
                .load_checkpoint("ckpt-avail")
                .unwrap()
                .unwrap()
                .available
        );
    }
}
