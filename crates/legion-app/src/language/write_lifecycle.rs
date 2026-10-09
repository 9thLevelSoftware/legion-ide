//! App-owned admission and lifetime of write-producing language operations.
//!
//! A deferred request is part of its admitted operation, never an independent
//! entry that cancellation or session invalidation can leave behind. This
//! module owns metadata and queued parameters, not editor text or apply authority.

use std::collections::HashMap;

use legion_protocol::{BufferId, FileId, SnapshotId, WorkspaceId};

/// Bounded metadata retained while an accepted write-side LSP request is in flight.
#[derive(Debug, Clone)]
pub(crate) struct PendingLspWriteOperation {
    /// Opaque operation identifier reused in the projected terminal result.
    pub(crate) operation_id: String,
    /// Existing projection operation category.
    pub(crate) operation_kind: crate::LanguageToolingOperationKind,
    /// Workspace and file identity retained for terminal status projection.
    pub(crate) workspace_id: WorkspaceId,
    pub(crate) file_id: FileId,
    /// Buffer and snapshot the request was admitted against.
    pub(crate) buffer_id: BufferId,
    pub(crate) snapshot_id: SnapshotId,
    /// Event context reused when the resulting proposal is recorded.
    pub(crate) event_context: crate::EventContext,
}

/// Metadata-only rename retained until its target document has entered the
/// bounded worker queue. No source text is held here.
#[derive(Clone)]
pub(crate) struct DeferredLspWrite {
    pub(crate) buffer_id: BufferId,
    pub(crate) snapshot_id: SnapshotId,
    pub(crate) uri: String,
    pub(crate) method: String,
    pub(crate) kind: crate::language::LspReadKind,
    pub(crate) params: serde_json::Value,
    pub(crate) operation_context: legion_protocol::LspOperationContext,
}

const MAX_PENDING_WRITES: usize = 32;

struct WriteOperation {
    pending: PendingLspWriteOperation,
    deferred: Option<DeferredLspWrite>,
}

#[derive(Default)]
pub(crate) struct LspWriteLifecycle {
    operations: HashMap<String, WriteOperation>,
}

impl LspWriteLifecycle {
    pub(crate) fn has_capacity(&self) -> bool {
        self.operations.len() < MAX_PENDING_WRITES
    }

    pub(crate) fn can_defer(&self, buffer_id: BufferId) -> bool {
        self.has_capacity()
            && !self.operations.values().any(|operation| {
                operation.pending.buffer_id == buffer_id && operation.deferred.is_some()
            })
    }

    pub(crate) fn admit(
        &mut self,
        pending: PendingLspWriteOperation,
        deferred: Option<DeferredLspWrite>,
    ) -> Result<(), &'static str> {
        if !self.has_capacity() {
            return Err("language operation limit reached");
        }
        if self.operations.contains_key(&pending.operation_id) {
            return Err("language operation already admitted");
        }
        if let Some(request) = &deferred {
            if !self.can_defer(pending.buffer_id) {
                return Err("buffer already has a deferred language operation");
            }
            if request.buffer_id != pending.buffer_id
                || request.snapshot_id != pending.snapshot_id
                || request.operation_context.workspace_id != pending.workspace_id
                || request.operation_context.file_id != pending.file_id
                || request.operation_context.buffer_id != pending.buffer_id
                || request.operation_context.snapshot_id != pending.snapshot_id
            {
                return Err("deferred language operation identity mismatch");
            }
        }
        self.operations.insert(
            pending.operation_id.clone(),
            WriteOperation { pending, deferred },
        );
        Ok(())
    }

    pub(crate) fn pending(&self, operation_id: &str) -> Option<&PendingLspWriteOperation> {
        self.operations
            .get(operation_id)
            .map(|operation| &operation.pending)
    }

    /// Reserve lifecycle capacity before calling the transport. A refused
    /// submission releases the reservation, permitting a deferred admission.
    pub(crate) fn submit(
        &mut self,
        pending: PendingLspWriteOperation,
        send: impl FnOnce() -> bool,
    ) -> Result<bool, &'static str> {
        let operation_id = pending.operation_id.clone();
        self.admit(pending, None)?;
        let accepted = send();
        if !accepted {
            self.finish(&operation_id);
        }
        Ok(accepted)
    }

    /// Completion, cancellation and stale rejection all release the queued request.
    pub(crate) fn finish(&mut self, operation_id: &str) -> Option<PendingLspWriteOperation> {
        self.operations
            .remove(operation_id)
            .map(|operation| operation.pending)
    }

    /// Invalidate one document, or every operation on session/workspace loss.
    pub(crate) fn invalidate(
        &mut self,
        buffer_id: Option<BufferId>,
    ) -> Vec<PendingLspWriteOperation> {
        let ids: Vec<_> = self
            .operations
            .iter()
            .filter(|(_, operation)| buffer_id.is_none_or(|id| operation.pending.buffer_id == id))
            .map(|(id, _)| id.clone())
            .collect();
        ids.into_iter().filter_map(|id| self.finish(&id)).collect()
    }

    /// Owned snapshots let the app inspect current editor identity and submit
    /// to the bounded worker without exposing mutable lifecycle internals.
    pub(crate) fn deferred(&self) -> Vec<(PendingLspWriteOperation, DeferredLspWrite)> {
        self.operations
            .values()
            .filter_map(|operation| {
                operation
                    .deferred
                    .as_ref()
                    .map(|request| (operation.pending.clone(), request.clone()))
            })
            .collect()
    }

    pub(crate) fn dispatched(&mut self, operation_id: &str) {
        if let Some(operation) = self.operations.get_mut(operation_id) {
            operation.deferred = None;
        }
    }

    #[cfg(test)]
    pub(crate) fn len(&self) -> usize {
        self.operations.len()
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.operations.is_empty()
    }

    #[cfg(test)]
    pub(crate) fn pending_operations(&self) -> impl Iterator<Item = &PendingLspWriteOperation> {
        self.operations.values().map(|operation| &operation.pending)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use legion_protocol::{CausalityId, CorrelationId};

    fn operation(id: usize, buffer: u64) -> PendingLspWriteOperation {
        PendingLspWriteOperation {
            operation_id: id.to_string(),
            operation_kind: crate::LanguageToolingOperationKind::RenameProposal,
            workspace_id: WorkspaceId(1),
            file_id: FileId(buffer.into()),
            buffer_id: BufferId(buffer.into()),
            snapshot_id: SnapshotId(1),
            event_context: crate::EventContext {
                correlation_id: CorrelationId(1),
                causality_id: CausalityId(uuid::Uuid::now_v7()),
            },
        }
    }

    fn deferred(pending: &PendingLspWriteOperation) -> DeferredLspWrite {
        let mut context = super::super::operation_context_for_snapshot(pending.snapshot_id);
        context.workspace_id = pending.workspace_id;
        context.file_id = pending.file_id;
        context.buffer_id = pending.buffer_id;
        DeferredLspWrite {
            buffer_id: pending.buffer_id,
            snapshot_id: pending.snapshot_id,
            uri: format!("file:///{}.rs", pending.buffer_id.0),
            method: "textDocument/rename".to_string(),
            kind: super::super::LspReadKind::Rename {
                new_name: "renamed".to_string(),
            },
            params: serde_json::json!({}),
            operation_context: context,
        }
    }

    #[test]
    fn admission_refusal_never_submits_and_transport_refusal_releases_capacity() {
        let mut lifecycle = LspWriteLifecycle::default();
        for id in 0..MAX_PENDING_WRITES {
            assert_eq!(
                lifecycle.submit(operation(id, id as u64), || true),
                Ok(true)
            );
        }
        assert!(
            lifecycle
                .submit(operation(99, 99), || panic!("must not send over capacity"))
                .is_err()
        );
        lifecycle.finish("0").expect("release slot");
        assert_eq!(lifecycle.submit(operation(99, 99), || false), Ok(false));
        assert!(lifecycle.has_capacity());
        assert!(lifecycle.pending("99").is_none());
        assert!(
            lifecycle
                .submit(operation(1, 1), || panic!("must not resend duplicate"))
                .is_err()
        );
    }

    #[test]
    fn document_invalidation_removes_queued_work_and_preserves_other_documents() {
        let mut lifecycle = LspWriteLifecycle::default();
        for id in 1..=2 {
            let pending = operation(id, id as u64);
            lifecycle
                .admit(pending.clone(), Some(deferred(&pending)))
                .unwrap();
        }
        let cancelled = lifecycle.invalidate(Some(BufferId(1)));
        assert_eq!(cancelled.len(), 1);
        assert_eq!(cancelled[0].operation_id, "1");
        assert!(
            lifecycle.finish("1").is_none(),
            "late completion cannot revive an operation"
        );
        let waiting = lifecycle.deferred();
        assert_eq!(waiting.len(), 1);
        assert_eq!(waiting[0].0.operation_id, "2");
        lifecycle.dispatched("2");
        assert!(lifecycle.deferred().is_empty());
        assert!(
            lifecycle.pending("2").is_some(),
            "dispatch must keep completion authority"
        );
        assert_eq!(lifecycle.invalidate(None).len(), 1);
        assert!(lifecycle.is_empty());
    }

    #[test]
    fn deferred_admission_rejects_mismatched_identity_and_duplicate_document() {
        let mut lifecycle = LspWriteLifecycle::default();
        let first = operation(1, 1);
        let mut request = deferred(&first);
        request.operation_context.snapshot_id = SnapshotId(2);
        assert!(lifecycle.admit(first.clone(), Some(request)).is_err());
        assert!(lifecycle.is_empty());
        lifecycle
            .admit(first.clone(), Some(deferred(&first)))
            .unwrap();
        let second = operation(2, 1);
        assert!(
            lifecycle
                .admit(second.clone(), Some(deferred(&second)))
                .is_err()
        );
        assert_eq!(lifecycle.deferred().len(), 1);
        lifecycle.finish("1").unwrap();
        assert!(
            lifecycle
                .admit(second.clone(), Some(deferred(&second)))
                .is_ok()
        );
    }
}
