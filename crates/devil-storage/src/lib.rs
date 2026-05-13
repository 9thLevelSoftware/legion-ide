//! Storage service interfaces for workspace metadata, trust decisions, file metadata cache, and sessions.

#![warn(missing_docs)]

use std::collections::HashMap;

use devil_protocol::{CanonicalPath, CorrelationId, FileId, SnapshotId, WorkspaceId, WorkspaceTrustState};
use devil_security::TrustState;
use thiserror::Error;

#[derive(Debug, Clone)]
/// Lightweight record for persisted workspace configuration snapshots.
pub struct WorkspaceConfigRecord {
    /// Serialized configuration payload.
    pub serialized: String,
    /// Current snapshot identifier for this configuration.
    pub snapshot_id: SnapshotId,
}

#[derive(Debug, Clone)]
/// Stored trust decision metadata for a workspace principal.
pub struct TrustDecisionRecord {
    /// Last known trust state.
    pub trust_state: WorkspaceTrustState,
    /// Correlation tracking this decision.
    pub correlation_id: CorrelationId,
}

#[derive(Debug, Clone)]
/// Cached file metadata used by shallow-discovery reconciliation.
pub struct FileMetadataRecord {
    /// Fingerprint hash or digest string.
    pub fingerprint: String,
    /// Stable workspace-local file identifier.
    pub file_id: FileId,
}

#[derive(Debug, Clone)]
/// Session metadata persisted for recovery and restore.
pub struct SessionRecord {
    /// Owning workspace identifier.
    pub workspace_id: WorkspaceId,
    /// Canonical workspace root path.
    pub workspace_path: CanonicalPath,
    /// Persisted trust state.
    pub trust_state: WorkspaceTrustState,
}

#[derive(Debug, Error)]
/// Storage error conditions.
pub enum StorageError {
    /// Requested record was not found.
    #[error("not found: {key}")]
    NotFound {
        /// Lookup key used for this lookup.
        key: String,
    },
    /// Low-level failure.
    #[error("storage operation failed: {message}")]
    Failed {
        /// Detailed failure text.
        message: String,
    },
}

type StorageResult<T> = Result<T, StorageError>;

/// Persistent workspace config persistence API.
pub trait WorkspaceConfigRepository {
    /// Store workspace configuration data.
    fn save(&mut self, workspace_id: WorkspaceId, config: WorkspaceConfigRecord) -> StorageResult<()>;
    /// Load workspace configuration data.
    fn load(&self, workspace_id: WorkspaceId) -> StorageResult<WorkspaceConfigRecord>;
    /// Remove workspace configuration data.
    fn remove(&mut self, workspace_id: WorkspaceId) -> StorageResult<()>;
}

/// Persistent trust decision API.
pub trait WorkspaceTrustRepository {
    /// Persist trust decision for principal in workspace.
    fn persist(
        &mut self,
        workspace_id: WorkspaceId,
        principal_id: &str,
        decision: TrustDecisionRecord,
    ) -> StorageResult<()>;
    /// Resolve trust decision for principal/workspace pair.
    fn resolve(
        &self,
        workspace_id: WorkspaceId,
        principal_id: &str,
    ) -> StorageResult<TrustDecisionRecord>;
}

/// File metadata cache API.
pub trait FileMetadataCache {
    /// Save fingerprint metadata for a path.
    fn put_fingerprint(
        &mut self,
        workspace_id: WorkspaceId,
        canonical_path: &str,
        metadata: FileMetadataRecord,
    ) -> StorageResult<()>;
    /// Load fingerprint metadata for a path.
    fn get_fingerprint(
        &self,
        workspace_id: WorkspaceId,
        canonical_path: &str,
    ) -> StorageResult<FileMetadataRecord>;
    /// Clear cache for workspace.
    fn clear_workspace(&mut self, workspace_id: WorkspaceId) -> StorageResult<()>;
}

/// Session persistence API.
pub trait WorkspaceSessionRepository {
    /// Persist session metadata.
    fn save_session(
        &mut self,
        session_id: &str,
        session: SessionRecord,
    ) -> StorageResult<()>;
    /// Restore session metadata.
    fn load_session(&self, session_id: &str) -> StorageResult<SessionRecord>;
    /// Delete session metadata.
    fn delete_session(&mut self, session_id: &str) -> StorageResult<()>;
}

#[derive(Debug, Default)]
/// Test-oriented, in-memory storage implementation.
pub struct InMemoryStorage {
    workspace_configs: HashMap<WorkspaceId, WorkspaceConfigRecord>,
    trust: HashMap<(WorkspaceId, String), TrustDecisionRecord>,
    metadata: HashMap<(WorkspaceId, String), FileMetadataRecord>,
    sessions: HashMap<String, SessionRecord>,
}

impl InMemoryStorage {
    /// Construct a new in-memory store.
    pub fn new() -> Self {
        Self::default()
    }
}

impl WorkspaceConfigRepository for InMemoryStorage {
    fn save(&mut self, workspace_id: WorkspaceId, config: WorkspaceConfigRecord) -> StorageResult<()> {
        self.workspace_configs.insert(workspace_id, config);
        Ok(())
    }

    fn load(&self, workspace_id: WorkspaceId) -> StorageResult<WorkspaceConfigRecord> {
        self.workspace_configs
            .get(&workspace_id)
            .cloned()
            .ok_or_else(|| StorageError::NotFound {
                key: format!("workspace_config:{workspace_id:?}"),
            })
    }

    fn remove(&mut self, workspace_id: WorkspaceId) -> StorageResult<()> {
        self.workspace_configs
            .remove(&workspace_id)
            .map(|_| ())
            .ok_or_else(|| StorageError::NotFound {
                key: format!("workspace_config:{workspace_id:?}"),
            })
    }
}

impl WorkspaceTrustRepository for InMemoryStorage {
    fn persist(
        &mut self,
        workspace_id: WorkspaceId,
        principal_id: &str,
        decision: TrustDecisionRecord,
    ) -> StorageResult<()> {
        self.trust
            .insert((workspace_id, principal_id.to_string()), decision);
        Ok(())
    }

    fn resolve(
        &self,
        workspace_id: WorkspaceId,
        principal_id: &str,
    ) -> StorageResult<TrustDecisionRecord> {
        self.trust
            .get(&(workspace_id, principal_id.to_string()))
            .cloned()
            .ok_or_else(|| StorageError::NotFound {
                key: format!("workspace_trust:{workspace_id:?}:{principal_id}"),
            })
    }
}

impl FileMetadataCache for InMemoryStorage {
    fn put_fingerprint(
        &mut self,
        workspace_id: WorkspaceId,
        canonical_path: &str,
        metadata: FileMetadataRecord,
    ) -> StorageResult<()> {
        self.metadata
            .insert((workspace_id, canonical_path.to_string()), metadata);
        Ok(())
    }

    fn get_fingerprint(
        &self,
        workspace_id: WorkspaceId,
        canonical_path: &str,
    ) -> StorageResult<FileMetadataRecord> {
        self.metadata
            .get(&(workspace_id, canonical_path.to_string()))
            .cloned()
            .ok_or_else(|| StorageError::NotFound {
                key: format!("file_metadata:{workspace_id:?}:{canonical_path}"),
            })
    }

    fn clear_workspace(&mut self, workspace_id: WorkspaceId) -> StorageResult<()> {
        let before = self.metadata.len();
        self.metadata.retain(|(id, _), _| *id != workspace_id);

        if self.metadata.len() == before {
            return Err(StorageError::NotFound {
                key: format!("file_metadata:{workspace_id:?}"),
            });
        }

        Ok(())
    }
}

impl WorkspaceSessionRepository for InMemoryStorage {
    fn save_session(&mut self, session_id: &str, session: SessionRecord) -> StorageResult<()> {
        self.sessions.insert(session_id.to_string(), session);
        Ok(())
    }

    fn load_session(&self, session_id: &str) -> StorageResult<SessionRecord> {
        self.sessions
            .get(session_id)
            .cloned()
            .ok_or_else(|| StorageError::NotFound {
                key: format!("session:{session_id}"),
            })
    }

    fn delete_session(&mut self, session_id: &str) -> StorageResult<()> {
        self.sessions
            .remove(session_id)
            .map(|_| ())
            .ok_or_else(|| StorageError::NotFound {
                key: format!("session:{session_id}"),
            })
    }
}

/// Convert security trust model to protocol trust model.
pub fn security_trust_to_protocol(state: TrustState) -> WorkspaceTrustState {
    match state {
        TrustState::Trusted => WorkspaceTrustState::Trusted,
        TrustState::Untrusted => WorkspaceTrustState::Untrusted,
        TrustState::Unknown => WorkspaceTrustState::Unknown,
    }
}

/// Convert protocol trust model to security trust model.
pub fn protocol_trust_to_security(state: WorkspaceTrustState) -> TrustState {
    match state {
        WorkspaceTrustState::Trusted => TrustState::Trusted,
        WorkspaceTrustState::Untrusted => TrustState::Untrusted,
        WorkspaceTrustState::Unknown => TrustState::Unknown,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn in_memory_storage_roundtrip_config() {
        let mut storage = InMemoryStorage::new();
        let id = WorkspaceId(10);
        let record = WorkspaceConfigRecord {
            serialized: r#"{"name":"demo"}"#.to_string(),
            snapshot_id: SnapshotId(99),
        };

        storage.save(id, record.clone()).expect("save workspace config");
        let loaded = storage.load(id).expect("load workspace config");
        assert_eq!(loaded.serialized, record.serialized);
        assert_eq!(loaded.snapshot_id, record.snapshot_id);
    }

    #[test]
    fn in_memory_storage_roundtrip_trust() {
        let mut storage = InMemoryStorage::new();
        let record = TrustDecisionRecord {
            trust_state: WorkspaceTrustState::Trusted,
            correlation_id: CorrelationId(3),
        };

        storage
            .persist(WorkspaceId(20), "principal", record.clone())
            .expect("persist trust decision");
        let loaded = storage
            .resolve(WorkspaceId(20), "principal")
            .expect("load trust decision");
        assert_eq!(
            loaded.trust_state as u8,
            record.trust_state as u8,
            "stored and loaded trust state must match"
        );
    }

    #[test]
    fn in_memory_storage_roundtrip_file_metadata() {
        let mut storage = InMemoryStorage::new();
        let rec = FileMetadataRecord {
            fingerprint: "abc123".to_string(),
            file_id: FileId(5),
        };

        storage
            .put_fingerprint(WorkspaceId(30), "/tmp/a.txt", rec.clone())
            .expect("store file metadata");
        let loaded = storage
            .get_fingerprint(WorkspaceId(30), "/tmp/a.txt")
            .expect("load file metadata");
        assert_eq!(loaded.fingerprint, rec.fingerprint);

        storage
            .clear_workspace(WorkspaceId(30))
            .expect("clear workspace");
        assert!(storage.get_fingerprint(WorkspaceId(30), "/tmp/a.txt").is_err());
    }

    #[test]
    fn in_memory_storage_roundtrip_session() {
        let mut storage = InMemoryStorage::new();
        let rec = SessionRecord {
            workspace_id: WorkspaceId(40),
            workspace_path: CanonicalPath("/tmp/ws".to_string()),
            trust_state: WorkspaceTrustState::Trusted,
        };

        storage
            .save_session("session-1", rec.clone())
            .expect("save session");
        let loaded = storage.load_session("session-1").expect("load session");
        assert_eq!(loaded.workspace_id, rec.workspace_id);

        storage
            .delete_session("session-1")
            .expect("delete session");
        assert!(storage.load_session("session-1").is_err());
    }

    #[test]
    fn trust_conversion_roundtrips() {
        let security_from_protocol = protocol_trust_to_security(WorkspaceTrustState::Trusted);
        let protocol_from_security = security_trust_to_protocol(TrustState::Untrusted);
        let protocol_from_security_roundtrip = protocol_from_security.clone();

        assert!(matches!(security_from_protocol, TrustState::Trusted));
        assert!(matches!(
            protocol_trust_to_security(protocol_from_security_roundtrip),
            TrustState::Untrusted
        ));
        assert!(matches!(protocol_from_security, WorkspaceTrustState::Untrusted));
    }
}
