//! Capture and restore unsaved buffer bodies for hot-exit.
//!
//! The bodies themselves are persisted by `legion_storage::HotExitStore`, not
//! by session JSON. This module only reads and writes editor buffers.

use crate::*;
use legion_storage::HotExitSnapshot;

impl AppComposition {
    /// Capture dirty buffer bodies for a crash-safe sidecar store.
    pub fn capture_hot_exit_snapshots(&self) -> Result<Vec<HotExitSnapshot>, AppCompositionError> {
        let mut snapshots = Vec::new();
        for buffer_id in &self.active_documents.open_tabs {
            if !self.editor.is_dirty(*buffer_id)? {
                continue;
            }
            let Some(metadata) = self.active_documents.metadata_for_buffer(*buffer_id) else {
                continue;
            };
            let body = self.editor.text(*buffer_id)?;
            let buffer_version = self.editor.buffer_metadata(*buffer_id)?.buffer_version.0;
            snapshots.push(HotExitSnapshot {
                path: metadata.identity.canonical_path.0.clone(),
                buffer_version,
                body: body.to_string(),
            });
        }
        Ok(snapshots)
    }

    /// Re-apply sidecar bodies onto already-opened buffers. Does not write disk.
    ///
    /// Returns how many buffers received a restoring edit.
    pub fn restore_hot_exit_snapshots(
        &mut self,
        snapshots: &[HotExitSnapshot],
    ) -> Result<usize, AppCompositionError> {
        let mut restored = 0;
        for snapshot in snapshots {
            let Some(buffer_id) =
                self.active_documents
                    .open_tabs
                    .iter()
                    .copied()
                    .find(|buffer_id| {
                        self.active_documents
                            .metadata_for_buffer(*buffer_id)
                            .is_some_and(|metadata| {
                                metadata.identity.canonical_path.0 == snapshot.path
                            })
                    })
            else {
                continue;
            };
            let current = self.editor.text(buffer_id)?;
            if current == snapshot.body {
                continue;
            }
            let end = end_position(current);
            let correlation_id = self.correlation_generator.next();
            self.apply_edit_to_buffer_with_correlation(
                buffer_id,
                TextEdit::new(
                    EditorTextRange::new(TextPosition::zero(), end),
                    snapshot.body.clone(),
                ),
                correlation_id,
            )?;
            restored += 1;
        }
        Ok(restored)
    }
}
