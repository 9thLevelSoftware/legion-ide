//! In-process completion/hover scheduling from accepted editor interactions.

use std::time::{Duration, Instant};

use legion_editor::TextPosition;
use legion_protocol::{BufferId, BufferVersion, SnapshotId, TextCoordinate, WorkspaceId};
use legion_ui::CommandDispatchIntent;

use crate::{AppCommandOutcome, AppComposition, AppCompositionError};

/// Kind of language interaction whose settle window elapsed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LspDebounceKind {
    /// Completion after an accepted text edit.
    Completion,
    /// Hover after an accepted caret movement.
    Hover,
}

/// A due interaction, exposed for deterministic interface tests.
#[derive(Debug, Clone)]
pub struct LspDebounceEvent {
    /// Authoritative buffer identity.
    pub buffer_id: BufferId,
    /// Authoritative post-action caret coordinate.
    pub position: TextCoordinate,
    /// Request kind.
    pub kind: LspDebounceKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct InteractionContext {
    workspace_id: WorkspaceId,
    buffer_id: BufferId,
    snapshot_id: SnapshotId,
    buffer_version: BufferVersion,
    cursor: TextPosition,
}

struct PendingInteraction {
    armed_at: Instant,
    context: InteractionContext,
    position: TextCoordinate,
}

#[derive(Default)]
pub(crate) struct LspInteraction {
    completion: Option<PendingInteraction>,
    hover: Option<PendingInteraction>,
}

impl LspInteraction {
    pub(crate) fn invalidate(&mut self) {
        self.completion = None;
        self.hover = None;
    }

    fn take_due(
        &mut self,
        now: Instant,
        current: Option<InteractionContext>,
    ) -> Vec<LspDebounceEvent> {
        let mut due = Vec::with_capacity(2);
        for (pending, kind, delay) in [
            (&mut self.completion, LspDebounceKind::Completion, 50),
            (&mut self.hover, LspDebounceKind::Hover, 200),
        ] {
            if pending.as_ref().is_some_and(|p| Some(p.context) != current) {
                *pending = None;
            }
            if pending.as_ref().is_some_and(|p| {
                now.saturating_duration_since(p.armed_at) >= Duration::from_millis(delay)
            }) {
                let p = pending.take().expect("due interaction exists");
                due.push(LspDebounceEvent {
                    buffer_id: p.context.buffer_id,
                    position: p.position,
                    kind,
                });
            }
        }
        due
    }
}

impl AppComposition {
    /// Route accepted UI interactions through app/editor authority and schedule language reads.
    pub fn dispatch_ui_intent(
        &mut self,
        intent: CommandDispatchIntent,
    ) -> Result<AppCommandOutcome, AppCompositionError> {
        self.dispatch_interaction(intent, None)
    }

    /// Dispatch with a deterministic interaction clock, without changing transport behavior.
    #[cfg(any(test, feature = "test-helpers"))]
    pub fn dispatch_ui_intent_at_for_test(
        &mut self,
        intent: CommandDispatchIntent,
        now: Instant,
    ) -> Result<AppCommandOutcome, AppCompositionError> {
        self.dispatch_interaction(intent, Some(now))
    }

    fn dispatch_interaction(
        &mut self,
        intent: CommandDispatchIntent,
        now: Option<Instant>,
    ) -> Result<AppCommandOutcome, AppCompositionError> {
        // Paste and IME already enter as Insert; no adapter-specific action types belong here.
        let completion_edit = matches!(
            intent,
            CommandDispatchIntent::Insert { .. }
                | CommandDispatchIntent::Delete { .. }
                | CommandDispatchIntent::ReplaceDirectedCarets { .. }
                | CommandDispatchIntent::DeleteDirectedCarets { .. }
        );
        let outcome = self.dispatch_ui_intent_inner(intent)?;
        let trigger = match &outcome {
            AppCommandOutcome::Edited(edit) if completion_edit => {
                Some((LspDebounceKind::Completion, edit.buffer_id))
            }
            AppCommandOutcome::CursorSet(buffer_id)
            | AppCommandOutcome::SelectionSet(buffer_id) => {
                Some((LspDebounceKind::Hover, *buffer_id))
            }
            _ => None,
        };
        if let Some((kind, buffer_id)) = trigger
            && let Some(context) = self.lsp_interaction_context()
            && buffer_id == context.buffer_id
            && let Ok(viewport) =
                self.editor
                    .viewport_projection(legion_protocol::EditorViewportRequest {
                        buffer_id: context.buffer_id,
                        scroll: legion_protocol::ViewportScroll {
                            top_line: context.cursor.line as u32,
                            left_column: 0,
                        },
                        dimensions: legion_protocol::ViewportDimensions {
                            width_px: 1,
                            height_px: 1,
                        },
                    })
        {
            // This bounded editor projection supplies offsets without materializing whole text.
            let pending = Some(PendingInteraction {
                armed_at: now.unwrap_or_else(Instant::now),
                context,
                position: viewport.cursor,
            });
            match kind {
                LspDebounceKind::Completion => self.lsp_interaction.completion = pending,
                LspDebounceKind::Hover => self.lsp_interaction.hover = pending,
            }
        }
        Ok(outcome)
    }

    fn lsp_interaction_context(&self) -> Option<InteractionContext> {
        let workspace_id = self.active_documents.workspace_id()?;
        let buffer_id = self.active_documents.active_buffer_id?;
        let snapshot = self.editor.current_snapshot(buffer_id).ok()?;
        Some(InteractionContext {
            workspace_id,
            buffer_id,
            snapshot_id: snapshot.snapshot_id,
            buffer_version: snapshot.buffer_version,
            cursor: self.editor.primary_cursor(buffer_id).ok()?,
        })
    }

    /// Advance language interactions using the adapter's frame clock.
    /// Unavailable language servers remain non-fatal; existing request policy/transport owns reads.
    pub fn tick_lsp_interactions(&mut self, now: Instant) {
        let current = self.lsp_interaction_context();
        for event in self.lsp_interaction.take_due(now, current) {
            let intent = match event.kind {
                LspDebounceKind::Completion => CommandDispatchIntent::RequestCompletion {
                    buffer_id: event.buffer_id,
                    position: event.position,
                },
                LspDebounceKind::Hover => CommandDispatchIntent::RequestHover {
                    buffer_id: event.buffer_id,
                    position: event.position,
                },
            };
            let _ = self.dispatch_ui_intent(intent);
        }
    }

    /// Consume due interactions without transport dispatch for deterministic interface tests.
    #[cfg(any(test, feature = "test-helpers"))]
    pub fn tick_lsp_debounces(&mut self, now: Instant) -> Vec<LspDebounceEvent> {
        let current = self.lsp_interaction_context();
        self.lsp_interaction.take_due(now, current)
    }

    /// Cancel a pending completion when the user dismisses or accepts the popup.
    pub fn disarm_lsp_completion_debounce(&mut self) {
        self.lsp_interaction.completion = None;
    }

    /// Cancel a pending hover when the user dismisses the tooltip.
    pub fn disarm_lsp_hover_debounce(&mut self) {
        self.lsp_interaction.hover = None;
    }
}
