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
