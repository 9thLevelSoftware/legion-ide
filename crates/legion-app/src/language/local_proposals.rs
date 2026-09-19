//! Proposal request routing. Only real language-server edits create proposals.
//!
//! A pending or unavailable server is represented by operation status, never
//! by a fabricated empty edit or a local token replacement labelled as rename.

use legion_protocol::{BufferId, LanguageToolingProjection, ProtocolTextRange, TextCoordinate};

use crate::{AppComposition, AppCompositionError, LanguageProposalKind};

impl AppComposition {
    /// Request a real organize-imports result, retaining explicit action
    /// selection when the server returns alternatives.
    pub(crate) fn run_organize_imports_proposal(
        &mut self,
        buffer_id: BufferId,
    ) -> Result<LanguageToolingProjection, AppCompositionError> {
        let issued = self
            .whole_document_utf16_range(buffer_id)
            .is_some_and(|range| {
                self.request_code_actions_scoped(
                    buffer_id,
                    ProtocolTextRange {
                        start: TextCoordinate {
                            line: range.start.line,
                            character: range.start.character,
                            byte_offset: None,
                            utf16_offset: None,
                        },
                        end: TextCoordinate {
                            line: range.end.line,
                            character: range.end.character,
                            byte_offset: None,
                            utf16_offset: None,
                        },
                    },
                    true,
                )
            });
        if issued {
            return Ok(self.language_tooling.projection());
        }
        self.record_language_proposal_unavailable(buffer_id, LanguageProposalKind::OrganizeImports)
    }

    pub(crate) fn record_language_proposal_unavailable(
        &mut self,
        buffer_id: BufferId,
        kind: LanguageProposalKind,
    ) -> Result<LanguageToolingProjection, AppCompositionError> {
        let event_context = self.next_event_context();
        let input = self.language_request_input(buffer_id, event_context)?;
        let message = match kind {
            LanguageProposalKind::Formatting => {
                "Formatting requires a configured formatter or a live language server with formatting support"
            }
            LanguageProposalKind::Rename => {
                "Semantic rename requires a live language server with rename support"
            }
            LanguageProposalKind::OrganizeImports => {
                "Organize imports requires a live language server with a supported organize action"
            }
            LanguageProposalKind::CodeAction => {
                "Select an action from the current language-server code-action results"
            }
        };
        Ok(self
            .language_tooling
            .record_proposal_failure(&input, kind, message.to_string()))
    }
}
