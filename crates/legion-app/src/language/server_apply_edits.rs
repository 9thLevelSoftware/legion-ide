//! Bounded reply authority for server-originated `workspace/applyEdit`.

use std::collections::HashMap;
use std::sync::mpsc::SyncSender;
use std::time::{Duration, Instant};

use legion_lsp::LspApplyWorkspaceEditResponse;
use legion_protocol::ProposalId;

#[derive(Debug, Default)]
pub(crate) struct ServerApplyEditAuthority {
    replies: HashMap<
        ProposalId,
        (
            SyncSender<LspApplyWorkspaceEditResponse>,
            super::ApplyEditDecision,
            Instant,
        ),
    >,
}

impl ServerApplyEditAuthority {
    pub(crate) fn retain(
        &mut self,
        proposal_id: ProposalId,
        reply: SyncSender<LspApplyWorkspaceEditResponse>,
        decision: super::ApplyEditDecision,
        deadline: Option<Instant>,
    ) -> bool {
        if self.replies.len() >= 8 || self.replies.contains_key(&proposal_id) {
            return false;
        }
        // Honor an already-expired request deadline so an inline claim cannot
        // revive it. Live or missing deadlines get the 120s review budget
        // rather than the short LSP request timeout.
        let now = Instant::now();
        let deadline = match deadline {
            Some(deadline) if deadline <= now => deadline,
            _ => now + Duration::from_secs(120),
        };
        self.replies
            .insert(proposal_id, (reply, decision, deadline));
        true
    }

    pub(crate) fn finish(&mut self, proposal_id: ProposalId, applied: bool, reason: String) {
        if let Some((reply, decision, _)) = self.replies.remove(&proposal_id) {
            let result = if applied {
                super::ApplyEditDecisionResult::applied()
            } else {
                super::ApplyEditDecisionResult::rejected(reason.clone())
            };
            if let Ok(claim) = decision.claim(Instant::now() + Duration::from_secs(1)) {
                let _ = claim.finish(result);
            }
            let _ = reply.try_send(LspApplyWorkspaceEditResponse {
                applied,
                failure_reason: (!applied).then_some(reason),
            });
        }
    }

    pub(crate) fn is_live(&self, proposal_id: ProposalId) -> bool {
        self.replies.contains_key(&proposal_id)
    }

    pub(crate) fn claim(
        &mut self,
        proposal_id: ProposalId,
    ) -> Option<(
        super::ApplyEditClaim,
        SyncSender<LspApplyWorkspaceEditResponse>,
    )> {
        let (reply, decision, deadline) = self.replies.remove(&proposal_id)?;
        if deadline <= Instant::now() {
            let _ = decision.on_deadline();
            let _ = reply.try_send(LspApplyWorkspaceEditResponse {
                applied: false,
                failure_reason: Some("workspace/applyEdit proposal expired".to_string()),
            });
            return None;
        }
        let claim = match decision.claim(deadline) {
            Ok(claim) => claim,
            Err(_) => {
                let _ = decision.on_deadline();
                let _ = reply.try_send(LspApplyWorkspaceEditResponse {
                    applied: false,
                    failure_reason: Some("workspace/applyEdit proposal expired".to_string()),
                });
                return None;
            }
        };
        Some((claim, reply))
    }

    pub(crate) fn finish_claimed(
        claim: super::ApplyEditClaim,
        reply: SyncSender<LspApplyWorkspaceEditResponse>,
        applied: bool,
        reason: String,
    ) {
        let result = if applied {
            super::ApplyEditDecisionResult::applied()
        } else {
            super::ApplyEditDecisionResult::rejected(reason.clone())
        };
        if !claim.finish(result) {
            return;
        }
        let _ = reply.try_send(LspApplyWorkspaceEditResponse {
            applied,
            failure_reason: (!applied).then_some(reason),
        });
    }

    pub(crate) fn clear(&mut self) {
        for (_, (reply, decision, _)) in self.replies.drain() {
            let _ = decision.on_deadline();
            let _ = reply.try_send(LspApplyWorkspaceEditResponse {
                applied: false,
                failure_reason: Some("applyEdit authority was cleared".to_string()),
            });
        }
    }

    /// Expires server-originated proposals before they can be applied by a
    /// later UI event. The proposal remains visible for diagnosis, but the
    /// application guard rejects it once this authority has expired it.
    pub(crate) fn expire(&mut self) {
        let now = Instant::now();
        let expired: Vec<ProposalId> = self
            .replies
            .iter()
            .filter_map(|(proposal_id, (_, _, deadline))| {
                (*deadline <= now).then_some(*proposal_id)
            })
            .collect();
        for proposal_id in expired {
            if let Some((reply, decision, _)) = self.replies.remove(&proposal_id) {
                let _ = decision.on_deadline();
                let _ = reply.try_send(LspApplyWorkspaceEditResponse {
                    applied: false,
                    failure_reason: Some("workspace/applyEdit proposal expired".to_string()),
                });
            }
        }
    }

    #[cfg(test)]
    pub(crate) fn expire_pending_for_test(&mut self, proposal_id: ProposalId) {
        if let Some((_, _, deadline)) = self.replies.get_mut(&proposal_id) {
            *deadline = Instant::now() - Duration::from_secs(1);
        }
        self.expire();
    }
}
