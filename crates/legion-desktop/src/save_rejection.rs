//! Turning a refused save into a sentence a person can act on.
//!
//! The desktop shell used to report a refused save as `format!("Save rejected:
//! {response:?}")`. On the one path a person actually reaches — editing a file
//! that something else has since rewritten on disk — that produced roughly
//! fifteen hundred characters of Rust `Debug` output in the status area:
//! lifecycle transition, correlation and causality ids, capability name, both
//! sets of version preconditions, two file fingerprints with their hashes, and
//! the absolute path in its `\\?\` extended-length form.
//!
//! None of that tells the person what happened, and all of it is internal
//! structure the product should not be publishing. The refusal itself is
//! correct — the file on disk is left alone and the edits stay in the buffer —
//! so the only thing wrong was that it could not be read.
//!
//! ## What the message has to carry
//!
//! Three things, and no more: which file, why the save did not happen, and what
//! is true now. The last one matters most and is the part a `Debug` dump buries
//! — someone who has just been told "rejected" needs to know their typing is
//! still there before anything else.
//!
//! The wording deliberately keeps the condition word (`stale`, `conflict`,
//! `denied`) in the text. `view::save_rejection_status_marker` classifies these
//! rows by substring, and dropping the word would silently reclassify every
//! stale save as a generic rejection.

use legion_app::AppSaveOutcome;
use legion_protocol::{
    FileConflictState, ProposalDenialReason, ProposalFailureReason, ProposalLifecycleTransition,
    ProposalResponse, ProposalStaleContext, ProposalStaleReason,
};

/// The file a refusal is about, in the short form the rest of the shell uses.
///
/// Falls back to "the file" rather than to a path: an empty spot in a sentence
/// reads as a bug, and the extended-length prefix Windows canonical paths carry
/// is worse than saying nothing.
fn subject(diagnostics: &[legion_protocol::ProtocolDiagnostic]) -> String {
    diagnostics
        .iter()
        .find_map(|diagnostic| diagnostic.path.as_ref())
        .map(|path| {
            let shown = crate::path_display::display_path(path.0.as_str());
            shown
                .rsplit(['/', '\\'])
                .next()
                .unwrap_or(shown.as_ref())
                .to_string()
        })
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| "the file".to_string())
}

/// Why the version check failed, in the terms the person can see for themselves.
fn stale_cause(reason: ProposalStaleReason) -> &'static str {
    match reason {
        // The three that mean, in practice, "someone else wrote this file".
        ProposalStaleReason::FileContentVersionMismatch
        | ProposalStaleReason::FingerprintMismatch
        | ProposalStaleReason::ModifiedTimestampMismatch => "changed on disk since it was opened",
        ProposalStaleReason::FileLengthMismatch => "changed size on disk since it was opened",
        // These are internal drift rather than something the person did, so the
        // wording does not accuse the disk of a change that may not have happened.
        ProposalStaleReason::BufferVersionMismatch
        | ProposalStaleReason::SnapshotMismatch
        | ProposalStaleReason::WorkspaceGenerationMismatch => {
            "moved on from the version this save was prepared against"
        }
    }
}

fn denial_cause(reason: ProposalDenialReason) -> &'static str {
    match reason {
        ProposalDenialReason::CapabilityDenied => "this build is not allowed to write files",
        ProposalDenialReason::WorkspaceUntrusted => "the workspace is not trusted",
        ProposalDenialReason::PrincipalUnauthorized => "this session is not authorized to write",
        ProposalDenialReason::PolicyDenied => "policy does not allow writing here",
    }
}

fn failure_cause(reason: ProposalFailureReason) -> &'static str {
    match reason {
        ProposalFailureReason::ApplyFailed => "the write did not complete",
        ProposalFailureReason::RollbackFailed => "the write failed and could not be undone",
        _ => "the write could not be recorded",
    }
}

/// The reassurance that belongs on every refusal: nothing was lost.
const EDITS_INTACT: &str = "Your edits are unsaved and still in the editor.";

fn stale_message(transition: &ProposalLifecycleTransition, stale: &ProposalStaleContext) -> String {
    format!(
        "Save rejected: {} {} (stale). {EDITS_INTACT} The file on disk was left as it is.",
        subject(&transition.diagnostics),
        stale_cause(stale.reason),
    )
}

/// What the shell says about a save outcome, and how loudly.
///
/// The choice between these wordings is the whole point of this module, so it
/// lives in one function rather than in two match arms. Split across arms, an
/// edit that routes the committed case through the rejection wording -- the
/// exact mistake this module exists to prevent -- type-checks, runs, and is
/// caught by nothing, because both functions take a `&ProposalResponse` and
/// return a `String`.
///
/// `None` for a clean save: there is nothing to explain.
pub fn save_outcome_message(outcome: &AppSaveOutcome) -> Option<(bool, String)> {
    match outcome {
        AppSaveOutcome::Saved(_) => None,
        // `false` = not a refusal. The file changed, so this is a warning about
        // the audit trail, not a report of work lost.
        AppSaveOutcome::CommittedThenAuditFailed { response, .. } => {
            Some((false, save_committed_audit_failure_message(response)))
        }
        AppSaveOutcome::Rejected(response) => Some((true, save_rejection_message(response))),
    }
}

/// A save whose bytes reached disk but whose record of it did not.
///
/// Separate from `save_rejection_message` because every sentence that function
/// produces would be false here. The write committed and the buffer has already
/// been reconciled with disk, so "was not written" and "your edits are unsaved"
/// are both wrong -- and wrong in the direction that costs work, since someone
/// told their save failed will retype it or close the file believing it
/// unchanged.
///
/// What is actually true is narrower and stranger: the file is correct, and the
/// audit trail covering it is not. That is worth saying rather than hiding,
/// because in a product whose premise is a trustworthy audit trail, a gap in it
/// is the thing an operator needs to know about.
pub fn save_committed_audit_failure_message(response: &ProposalResponse) -> String {
    let name = match response {
        ProposalResponse::Failed { transition, .. }
        | ProposalResponse::Denied { transition, .. }
        | ProposalResponse::Rejected { transition, .. }
        | ProposalResponse::Stale { transition, .. } => subject(&transition.diagnostics),
        _ => "the file".to_string(),
    };
    format!(
        "Saved {name}, but recording the save failed. The file on disk is correct and your \
         edits are safe; the audit trail for this save is incomplete."
    )
}

/// A refused save, as a sentence.
pub fn save_rejection_message(response: &ProposalResponse) -> String {
    match response {
        ProposalResponse::Stale { transition, stale } => stale_message(transition, stale),
        ProposalResponse::Conflict {
            transition,
            conflict,
        } => conflict_message(transition, conflict),
        ProposalResponse::Denied { transition, reason } => format!(
            "Save denied: {} was not written because {}. {EDITS_INTACT}",
            subject(&transition.diagnostics),
            denial_cause(*reason),
        ),
        ProposalResponse::Failed { transition, reason } => format!(
            "Save failed: {} was not written because {}. {EDITS_INTACT}",
            subject(&transition.diagnostics),
            failure_cause(*reason),
        ),
        ProposalResponse::Rejected { transition, .. } => format!(
            "Save rejected: {} was not written. {EDITS_INTACT}",
            subject(&transition.diagnostics),
        ),
        // Every remaining variant is a success or a lifecycle step that should
        // never reach a save-rejection path. Naming the variant is enough to
        // find it, and is still one line rather than a structure dump.
        other => format!(
            "Save rejected: unexpected {} outcome. {EDITS_INTACT}",
            variant_name(other),
        ),
    }
}

fn conflict_message(
    transition: &ProposalLifecycleTransition,
    conflict: &FileConflictState,
) -> String {
    // The conflict state carries its own diagnostics, which are closer to the
    // cause than the transition's.
    let name = if conflict.diagnostics.is_empty() {
        subject(&transition.diagnostics)
    } else {
        subject(&conflict.diagnostics)
    };
    format!("Save rejected: {name} has a conflict with the copy on disk. {EDITS_INTACT}")
}

fn variant_name(response: &ProposalResponse) -> &'static str {
    match response {
        ProposalResponse::Created(_) => "created",
        ProposalResponse::Validated(_) => "validated",
        ProposalResponse::Previewed { .. } => "previewed",
        ProposalResponse::Approved(_) => "approved",
        ProposalResponse::Rejected { .. } => "rejected",
        ProposalResponse::Applied(_) => "applied",
        ProposalResponse::Denied { .. } => "denied",
        ProposalResponse::Failed { .. } => "failed",
        ProposalResponse::RolledBack { .. } => "rolled-back",
        ProposalResponse::Stale { .. } => "stale",
        ProposalResponse::Conflict { .. } => "conflict",
        ProposalResponse::Cancelled { .. } => "cancelled",
    }
}

#[cfg(test)]
mod save_message_tests {
    use super::{save_committed_audit_failure_message, save_rejection_message};
    use legion_protocol::{
        CanonicalPath, CausalityId, CorrelationId, ProposalFailureReason, ProposalId,
        ProposalLifecycleState, ProposalLifecycleTransition, ProposalResponse,
        ProposalStaleContext, ProposalStaleReason, ProposalVersionPreconditions,
        ProtocolDiagnostic, ProtocolDiagnosticSeverity, TimestampMillis,
    };

    /// Structure names that appeared in the `{response:?}` dump this replaced.
    ///
    /// Each is internal shape the product should not publish, and the path
    /// marker is the Windows extended-length prefix, which is not something to
    /// show anyone even when the rest of the sentence is fine.
    const LEAKS: [&str; 8] = [
        "ProposalId(",
        "CorrelationId",
        "CausalityId",
        "ProposalLifecycleTransition",
        "ProposalStaleContext",
        "FileFingerprint",
        "PrincipalId",
        r"\\?\",
    ];

    fn transition_naming(path: &str) -> ProposalLifecycleTransition {
        ProposalLifecycleTransition {
            proposal_id: ProposalId(7),
            lifecycle_state: ProposalLifecycleState::Stale,
            timestamp: TimestampMillis(0),
            principal: legion_protocol::PrincipalId("desktop".to_string()),
            capability: legion_protocol::CapabilityId("fs.write".to_string()),
            correlation_id: CorrelationId(1),
            causality_id: CausalityId(uuid::Uuid::nil()),
            diagnostics: vec![ProtocolDiagnostic {
                code: "proposal.stale".to_string(),
                message: "file content version changed before save".to_string(),
                severity: ProtocolDiagnosticSeverity::Error,
                path: Some(CanonicalPath(path.to_string())),
                range: None,
            }],
        }
    }

    fn stale_response(path: &str) -> ProposalResponse {
        ProposalResponse::Stale {
            transition: transition_naming(path),
            stale: ProposalStaleContext {
                reason: ProposalStaleReason::FileContentVersionMismatch,
                expected: ProposalVersionPreconditions {
                    file_version: None,
                    buffer_version: None,
                    snapshot_id: None,
                    generation: None,
                    file_content_version: None,
                    workspace_generation: None,
                    expected_fingerprint: None,
                    expected_file_length: None,
                    expected_modified_at: None,
                },
                actual: None,
            },
        }
    }

    #[test]
    fn a_refused_save_names_the_file_the_cause_and_the_fate_of_the_edits() {
        let message = save_rejection_message(&stale_response(r"\\?\C:\work\src\main.rs"));

        assert!(message.contains("main.rs"), "no file named: {message}");
        assert!(
            message.contains("changed on disk"),
            "no cause given: {message}"
        );
        assert!(
            message.contains("still in the editor"),
            "does not say the edits survived: {message}"
        );
        // `view::save_rejection_status_marker` classifies these rows by
        // substring, so losing the condition word silently reclassifies every
        // stale save as a generic rejection.
        assert!(
            message.to_ascii_lowercase().contains("stale"),
            "the condition word must survive for status classification: {message}"
        );
        for leak in LEAKS {
            assert!(!message.contains(leak), "leaked {leak:?}: {message}");
        }
    }

    /// A write that reached disk must never be described as one that did not.
    ///
    /// This is the case where the two wrong answers are not symmetrical.
    /// Telling someone a failed save succeeded costs them a retry; telling them
    /// a successful save failed invites retyping over content already on disk,
    /// or abandoning a file believing it unchanged. The `committed` branch of
    /// `save_buffer` marks the buffer clean and binds it to the new disk state,
    /// so every claim the rejection wording makes is false there.
    #[test]
    fn a_committed_save_whose_audit_failed_is_not_described_as_unwritten() {
        let response = ProposalResponse::Failed {
            transition: transition_naming(r"\\?\C:\work\src\main.rs"),
            reason: ProposalFailureReason::StorageFailed,
        };
        let message = save_committed_audit_failure_message(&response);

        assert!(message.contains("main.rs"), "no file named: {message}");
        assert!(
            message.starts_with("Saved "),
            "a committed write has to lead with the fact that it saved: {message}"
        );
        for wrong in ["was not written", "unsaved", "rejected", "Save rejected"] {
            assert!(
                !message.contains(wrong),
                "claims {wrong:?} about a write that reached disk: {message}"
            );
        }
        assert!(
            message.contains("audit trail"),
            "the audit gap is the part an operator needs, and it is missing: {message}"
        );
        for leak in LEAKS {
            assert!(!message.contains(leak), "leaked {leak:?}: {message}");
        }
    }

    /// The mapping, not the wordings.
    ///
    /// The two message functions have the same signature and both return prose,
    /// so routing the committed outcome through the rejection wording compiles
    /// and runs. Testing them separately does not catch that; testing the
    /// mapping does. This is the assertion the first version of this work was
    /// missing -- swapping the call in `save_outcome_message` type-checks, and
    /// nothing else in the suite notices.
    #[test]
    fn each_save_outcome_gets_the_wording_that_is_true_of_it() {
        use legion_app::{AppSaveOutcome, PublicSaveRequestDto};
        use legion_protocol::{
            BufferId, BufferVersion, CorrelationId, FileId, SnapshotId, TimestampMillis,
            WorkspaceId,
        };

        let save = PublicSaveRequestDto {
            request_id: uuid::Uuid::nil(),
            workspace_id: WorkspaceId(0),
            buffer_id: BufferId(1),
            file_id: FileId(0),
            snapshot_id: SnapshotId(1),
            buffer_version: BufferVersion(1),
            content_hash: "hash".to_string(),
            payload_byte_len: 0,
            text: String::new(),
            requested_at: TimestampMillis(0),
            correlation_id: CorrelationId(1),
        };
        let response = ProposalResponse::Failed {
            transition: transition_naming("main.rs"),
            reason: ProposalFailureReason::StorageFailed,
        };

        assert_eq!(
            super::save_outcome_message(&AppSaveOutcome::Saved(save.clone())),
            None,
            "a clean save has nothing to explain"
        );

        let (refused, committed) =
            super::save_outcome_message(&AppSaveOutcome::CommittedThenAuditFailed {
                save,
                response: Box::new(response.clone()),
            })
            .expect("a committed write with a failed audit has something to say");
        assert!(
            !refused,
            "a write that reached disk is not a refusal, and severity follows this flag"
        );
        assert!(
            committed.starts_with("Saved "),
            "the committed outcome got the rejection wording: {committed}"
        );

        let (refused, rejected) =
            super::save_outcome_message(&AppSaveOutcome::Rejected(Box::new(response)))
                .expect("a rejection has something to say");
        assert!(refused, "a rejected save is a refusal");
        assert!(
            rejected.contains("was not written"),
            "the rejected outcome got the committed wording: {rejected}"
        );
    }

    /// The rejection wording, applied to the committed case, would be wrong.
    ///
    /// Stated as a test so the two functions cannot quietly converge: if
    /// someone routes the committed outcome back through
    /// `save_rejection_message`, this is what they will have said.
    #[test]
    fn the_rejection_wording_would_be_false_for_a_committed_write() {
        let response = ProposalResponse::Failed {
            transition: transition_naming("main.rs"),
            reason: ProposalFailureReason::StorageFailed,
        };
        let as_rejection = save_rejection_message(&response);
        assert!(
            as_rejection.contains("was not written") || as_rejection.contains("unsaved"),
            "this test exists to pin why the committed case needs its own message; if the \
             rejection wording no longer claims the file was unwritten, revisit both: \
             {as_rejection}"
        );
    }
}
