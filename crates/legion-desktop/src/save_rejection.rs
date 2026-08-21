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

/// A diagnostic message with embedded absolute paths reduced to file names.
///
/// The diagnostics this module now prefers are written where the failure
/// happened, and a filesystem failure carries `PlatformError::to_string()` —
/// which embeds the canonical path, `\\?\` prefix and all. Copying one verbatim
/// into a status line puts the internal path disclosure back that this formatter
/// exists to remove.
///
/// The cause survives; only the path shrinks. "failed to replace
/// `\\?\C:\work\src\main.rs`: access denied" becomes "failed to replace
/// `main.rs`: access denied" — still actionable, and still about a file the
/// person can see.
fn redact_paths(message: &str) -> String {
    // Quoted runs first, then bare tokens.
    //
    // Splitting on whitespace alone treated `"C:\work\my project\main.rs"` as
    // three tokens, so only the fragment holding the last separator shrank and
    // `C:\work\my` stayed on screen -- a redaction that reported success while
    // leaving most of the path visible. A quoted run is one path however many
    // spaces it contains, which is why the quoting is there.
    let mut out = String::with_capacity(message.len());
    let mut rest = message;
    while let Some(open) = rest.find('"') {
        out.push_str(&redact_bare_tokens(&rest[..open]));
        let after = &rest[open + 1..];
        match after.find('"') {
            Some(close) => {
                out.push('"');
                out.push_str(&shorten_path(&after[..close]));
                out.push('"');
                rest = &after[close + 1..];
            }
            // An unbalanced quote: treat the remainder as ordinary text rather
            // than swallowing it.
            None => {
                out.push('"');
                rest = after;
            }
        }
    }
    out.push_str(&redact_bare_tokens(rest));
    out
}

/// Shrink whitespace-separated tokens that look like paths.
fn redact_bare_tokens(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut last = 0usize;
    for (index, character) in text.char_indices() {
        if character.is_whitespace() {
            out.push_str(&shorten_token(&text[last..index]));
            out.push(character);
            last = index + character.len_utf8();
        }
    }
    out.push_str(&shorten_token(&text[last..]));
    out
}

/// One token, with any surrounding punctuation preserved.
fn shorten_token(token: &str) -> String {
    let trimmed = token.trim_matches(|c: char| "'`(),.:;".contains(c));
    if trimmed.is_empty() || (!trimmed.contains('/') && !trimmed.contains('\\')) {
        return token.to_string();
    }
    let shortened = shorten_path(trimmed);
    if shortened == trimmed {
        token.to_string()
    } else {
        token.replace(trimmed, &shortened)
    }
}

/// The last segment of a path, or the input when it is not one.
fn shorten_path(path: &str) -> String {
    if !path.contains('/') && !path.contains('\\') {
        return path.to_string();
    }
    path.rsplit(['/', '\\'])
        .find(|segment| !segment.is_empty())
        .unwrap_or(path)
        .to_string()
}

/// What the app itself said went wrong, when it said something specific.
///
/// Every refusal carries `ProtocolDiagnostic`s written where the failure
/// happened, and they know things this module cannot: which configured limit a
/// write exceeded, that a file disappeared rather than changed. The first
/// version of this formatter threw them away and substituted prose derived from
/// the typed reason alone, which collapses distinct causes into one sentence —
/// a 512 KiB size-limit denial and a genuinely disabled build both arrive as
/// `CapabilityDenied`, and both read as "this build is not allowed to write
/// files".
///
/// So the typed reason picks the *shape* of the sentence and the diagnostic
/// fills in the cause. Trimmed and bounded, because it is written for an
/// operator rather than for a status line.
fn diagnostic_detail(diagnostics: &[legion_protocol::ProtocolDiagnostic]) -> Option<String> {
    const MAX_DETAIL: usize = 200;
    diagnostics
        .iter()
        .find(|diagnostic| {
            diagnostic.severity == legion_protocol::ProtocolDiagnosticSeverity::Error
                && !diagnostic.message.trim().is_empty()
        })
        .map(|diagnostic| {
            let message = redact_paths(diagnostic.message.trim());
            if message.chars().count() > MAX_DETAIL {
                let clipped: String = message.chars().take(MAX_DETAIL).collect();
                format!("{clipped}…")
            } else {
                message
            }
        })
}

/// The short display name of a path.
fn file_name(path: &legion_protocol::CanonicalPath) -> String {
    let shown = crate::path_display::display_path(path.0.as_str());
    let name = shown.rsplit(['/', '\\']).next().unwrap_or(shown.as_ref());
    if name.is_empty() {
        "the file".to_string()
    } else {
        name.to_string()
    }
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

/// A failure, which may or may not have left new bytes on disk.
///
/// `ApplyFailed` is returned both when the write never happened and when
/// `write_text_file_atomic` succeeded but the metadata read or fingerprint that
/// follows it did not. In the second case the new bytes *are* on disk, so the
/// categorical "was not written, your edits are unsaved" is exactly the lie this
/// module was written to remove — reintroduced one variant over.
///
/// Neither this formatter nor the response can tell the two apart, so the
/// message says what is certainly true and points at the one place that can
/// settle it, rather than guessing and sounding confident.
fn failed_message(
    transition: &ProposalLifecycleTransition,
    reason: ProposalFailureReason,
) -> String {
    let name = subject(&transition.diagnostics);
    let cause = diagnostic_detail(&transition.diagnostics)
        .unwrap_or_else(|| failure_cause(reason).to_string());
    match reason {
        // Not "partly written": `write_text_file_atomic` writes and syncs a
        // complete temporary file before replacing the target, and the
        // non-atomic fallback is disabled, so the file is either the whole old
        // version or the whole new one. Saying it may be torn describes a state
        // this writer cannot produce, and sends someone looking for damage that
        // is not there. Which of the two it is, is the part they do need to
        // check, because `ApplyFailed` covers failures on both sides of the
        // replacement.
        ProposalFailureReason::ApplyFailed | ProposalFailureReason::RollbackFailed => format!(
            "Save failed for {name}: {cause}. Your edits are still in the editor. The file on \
             disk is either the previous version or the newly written one, not a partial \
             write — check which before retyping."
        ),
        _ => format!("Save failed: {name} was not written because {cause}. {EDITS_INTACT}"),
    }
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
pub fn save_outcome_message(outcome: &AppSaveOutcome) -> Option<SaveReport> {
    match outcome {
        AppSaveOutcome::Saved(_) => None,
        AppSaveOutcome::CommittedThenAuditFailed { path, response, .. } => Some(SaveReport {
            // The file changed, so this is a warning about the audit trail and
            // not a report of work lost.
            reached_disk: true,
            message: save_committed_audit_failure_message(path, response),
        }),
        AppSaveOutcome::Rejected(response) => Some(SaveReport {
            reached_disk: false,
            message: save_rejection_message(response),
        }),
    }
}

/// What to tell the person, and whether the bytes got there.
///
/// A named field rather than a bare `bool` in a tuple. The first version
/// returned `(bool, String)`, the caller destructured it as `(_refused,
/// message)`, and the flag was silently dropped -- an accurate sentence in
/// front of the person and a false outcome behind them. `_refused` reads like
/// something safely ignorable; `reached_disk` does not.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SaveReport {
    /// Whether the write actually landed on disk.
    pub reached_disk: bool,
    /// The sentence to show.
    pub message: String,
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
pub fn save_committed_audit_failure_message(
    path: &legion_protocol::CanonicalPath,
    response: &ProposalResponse,
) -> String {
    // The path is passed in, not recovered from the response.
    // `audit_storage_failed_response` builds its diagnostic with `path: None`,
    // so every attempt to read the filename back out of it yields "the file" —
    // and the unit test that fabricated a path hid exactly that.
    let name = file_name(path);
    let detail = match response {
        ProposalResponse::Failed { transition, .. }
        | ProposalResponse::Denied { transition, .. }
        | ProposalResponse::Rejected { transition, .. }
        | ProposalResponse::Stale { transition, .. } => diagnostic_detail(&transition.diagnostics),
        _ => None,
    };
    let because = match detail {
        Some(detail) => format!(" ({detail})"),
        None => String::new(),
    };
    format!(
        "Saved {name}, but recording the save failed{because}. The file on disk is correct and \
         your edits are safe; the audit trail for this save is incomplete."
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
            // The diagnostic distinguishes a size-limit refusal from a build
            // that cannot write at all; the typed reason calls both
            // `CapabilityDenied`.
            diagnostic_detail(&transition.diagnostics)
                .unwrap_or_else(|| denial_cause(*reason).to_string()),
        ),
        ProposalResponse::Failed { transition, reason } => failed_message(transition, *reason),
        ProposalResponse::Rejected { transition, .. } => {
            match diagnostic_detail(&transition.diagnostics) {
                Some(detail) => format!(
                    "Save rejected: {} was not written because {detail}. {EDITS_INTACT}",
                    subject(&transition.diagnostics),
                ),
                None => format!(
                    "Save rejected: {} was not written. {EDITS_INTACT}",
                    subject(&transition.diagnostics),
                ),
            }
        }
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
    // "a conflict with the copy on disk" is wrong when the conflict is that
    // there *is* no copy: a file deleted outside the editor arrives here with a
    // diagnostic reading "file disappeared from disk before save". Saying it
    // conflicts with something that does not exist leaves someone unable to tell
    // whether to recreate the file or reconcile it.
    let detail = diagnostic_detail(&conflict.diagnostics)
        .or_else(|| diagnostic_detail(&transition.diagnostics));
    match detail {
        // The word "conflict" stays in the sentence. This module already
        // documents that `view::save_rejection_status_marker` classifies these
        // rows by substring, and that dropping the condition word silently
        // reclassifies them -- and then this branch dropped it, so every real
        // conflict (which always carries a diagnostic) was filed as a generic
        // rejection. Named before the detail, so it still reads as prose.
        Some(detail) => {
            format!("Save rejected: {name} has a conflict — {detail}. {EDITS_INTACT}")
        }
        None => {
            format!("Save rejected: {name} has a conflict with the copy on disk. {EDITS_INTACT}")
        }
    }
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

    /// A transition shaped like a real post-commit audit failure.
    ///
    /// `path: None`, exactly as `audit_storage_failed_response` builds it.
    fn transition_without_path() -> ProposalLifecycleTransition {
        ProposalLifecycleTransition {
            proposal_id: ProposalId(7),
            lifecycle_state: ProposalLifecycleState::Failed,
            timestamp: TimestampMillis(0),
            principal: legion_protocol::PrincipalId("desktop".to_string()),
            capability: legion_protocol::CapabilityId("fs.write".to_string()),
            correlation_id: CorrelationId(1),
            causality_id: CausalityId(uuid::Uuid::nil()),
            diagnostics: vec![ProtocolDiagnostic {
                code: "proposal.audit_storage_failed".to_string(),
                message: "proposal success blocked because audit storage failed: io_error"
                    .to_string(),
                severity: ProtocolDiagnosticSeverity::Error,
                path: None,
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
        // Built the way production builds it: `audit_storage_failed_response`
        // sets `path: None`, so nothing in the response names the file. An
        // earlier version of this test fabricated a response *with* a path,
        // which made the formatter look like it could name the file when in
        // production it always said "the file". The path is now passed in.
        let response = ProposalResponse::Failed {
            transition: transition_without_path(),
            reason: ProposalFailureReason::StorageFailed,
        };
        let path = CanonicalPath(r"\\?\C:\work\src\main.rs".to_string());
        let message = save_committed_audit_failure_message(&path, &response);

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

        let committed = super::save_outcome_message(&AppSaveOutcome::CommittedThenAuditFailed {
            save,
            path: CanonicalPath("main.rs".to_string()),
            response: Box::new(response.clone()),
        })
        .expect("a committed write with a failed audit has something to say");
        assert!(
            committed.reached_disk,
            "a write that reached disk must say so; the desktop outcome follows this flag"
        );
        assert!(
            committed.message.starts_with("Saved "),
            "the committed outcome got the rejection wording: {}",
            committed.message
        );

        let rejected = super::save_outcome_message(&AppSaveOutcome::Rejected(Box::new(response)))
            .expect("a rejection has something to say");
        assert!(!rejected.reached_disk, "a rejected save wrote nothing");
        assert!(
            rejected.message.contains("was not written"),
            "the rejected outcome got the committed wording: {}",
            rejected.message
        );
    }

    /// A diagnostic's cause survives; its embedded absolute path does not.
    ///
    /// Preferring the app's own diagnostic is what made these messages specific,
    /// and it is also what let a canonical path back in: a filesystem failure
    /// carries `PlatformError::to_string()`, which embeds the full path with its
    /// `\?\` prefix. The formatter exists partly to keep that off the screen.
    #[test]
    fn a_diagnostic_keeps_its_cause_and_loses_its_path() {
        let redacted = super::redact_paths(
            r"failed to replace \\?\C:\work\src\main.rs: access denied (os error 5)",
        );

        assert!(
            redacted.contains("access denied"),
            "the actionable cause must survive: {redacted}"
        );
        assert!(
            redacted.contains("main.rs"),
            "the file must still be identifiable: {redacted}"
        );
        for leak in [r"\\?\", r"C:\work", r"src\main.rs"] {
            assert!(
                !redacted.contains(leak),
                "the embedded path is still disclosed ({leak:?}): {redacted}"
            );
        }

        // A message with no path is left exactly as written.
        let plain = "write of 900000 bytes exceeds the 524288 byte limit";
        assert_eq!(super::redact_paths(plain), plain);
    }

    /// A quoted path with spaces is redacted as one path.
    ///
    /// Splitting on whitespace treated `"C:\\work\\my project\\main.rs"` as three
    /// tokens, so only the fragment holding the last separator shrank and
    /// `C:\\work\\my` stayed on screen — a redaction that reported success
    /// while leaving most of the path visible. A quoted run is one path however
    /// many spaces it contains, which is why the quoting is there.
    #[test]
    fn a_quoted_path_with_spaces_is_redacted_whole() {
        let redacted =
            super::redact_paths(r#"failed to replace "C:\work\my project\main.rs": access denied"#);

        assert!(
            redacted.contains("access denied"),
            "the cause must survive: {redacted}"
        );
        assert!(
            redacted.contains("main.rs"),
            "the file must stay identifiable: {redacted}"
        );
        for leak in [r"C:\work", "my project"] {
            assert!(
                !redacted.contains(leak),
                "part of the quoted path is still on screen ({leak:?}): {redacted}"
            );
        }
        // The exact result, because "no leak" is satisfied by mangling too.
        // A whitespace-split redaction leaves "my main.rs" -- two fragments
        // of two different segments, naming no file that exists and reading
        // as corruption rather than as redaction.
        assert_eq!(
            redacted, r#"failed to replace "main.rs": access denied"#,
            "a quoted path must reduce to exactly its file name"
        );
    }

    /// The redaction is reached by the message, not only by its own test.
    ///
    /// Testing `redact_paths` directly proves the function works and nothing
    /// about whether anything calls it: removing the call from
    /// `diagnostic_detail` left every other test green. This goes through the
    /// public message, which is the thing that actually reaches a screen.
    #[test]
    fn a_failure_message_carries_no_absolute_path() {
        let mut transition = transition_naming("main.rs");
        transition.lifecycle_state = ProposalLifecycleState::Failed;
        transition.diagnostics = vec![ProtocolDiagnostic {
            code: "workspace.write_failed".to_string(),
            message: r"failed to replace \\?\C:\work\src\main.rs: access denied".to_string(),
            severity: ProtocolDiagnosticSeverity::Error,
            path: Some(CanonicalPath("main.rs".to_string())),
            range: None,
        }];
        let message = save_rejection_message(&ProposalResponse::Failed {
            transition,
            reason: ProposalFailureReason::ApplyFailed,
        });

        assert!(
            message.contains("access denied"),
            "the actionable cause must survive redaction: {message}"
        );
        for leak in [r"\\?\", r"C:\work"] {
            assert!(
                !message.contains(leak),
                "an internal path reached the status line ({leak:?}): {message}"
            );
        }
    }

    /// A size-limit denial must not read as a disabled build.
    ///
    /// `WorkspaceActor::save_file_with_proposal` maps the broker's detailed
    /// size-limit refusal to `CapabilityDenied`, which is the same typed reason
    /// a genuinely write-disabled build produces. Deriving the sentence from the
    /// reason alone therefore tells someone that saving is switched off, when in
    /// fact one file was too large — a normal policy rejection made impossible
    /// to diagnose.
    #[test]
    fn a_denial_says_which_policy_refused_it() {
        let mut transition = transition_naming("main.rs");
        transition.lifecycle_state = ProposalLifecycleState::Denied;
        transition.diagnostics = vec![ProtocolDiagnostic {
            code: "capability.write_size".to_string(),
            message: "write of 900000 bytes exceeds the 524288 byte limit".to_string(),
            severity: ProtocolDiagnosticSeverity::Error,
            path: Some(CanonicalPath("main.rs".to_string())),
            range: None,
        }];
        let message = save_rejection_message(&ProposalResponse::Denied {
            transition,
            reason: legion_protocol::ProposalDenialReason::CapabilityDenied,
        });

        assert!(
            message.contains("524288") || message.contains("exceeds"),
            "the denial does not say which limit refused it: {message}"
        );
        assert!(
            !message.contains("not allowed to write files"),
            "a size-limit refusal is being reported as a disabled build: {message}"
        );
    }

    /// A deleted file is described as deleted, not as conflicting with a copy.
    #[test]
    fn a_conflict_over_a_deleted_file_says_it_disappeared() {
        let mut transition = transition_naming("main.rs");
        transition.lifecycle_state = ProposalLifecycleState::Conflict;
        // `conflict_save_response` puts the same diagnostic on both the conflict
        // state and the transition, so either carries it in production. This
        // populates the conflict state, which is the one the formatter reads
        // first.
        let diagnostic = ProtocolDiagnostic {
            code: "workspace.file_missing".to_string(),
            message: "file disappeared from disk before save".to_string(),
            severity: ProtocolDiagnosticSeverity::Error,
            path: Some(CanonicalPath("main.rs".to_string())),
            range: None,
        };
        let identity = legion_protocol::FileIdentity {
            file_id: legion_protocol::FileId(0),
            workspace_id: legion_protocol::WorkspaceId(0),
            canonical_path: CanonicalPath("main.rs".to_string()),
            content_version: legion_protocol::FileContentVersion(1),
            content_hash: None,
        };
        let message = save_rejection_message(&ProposalResponse::Conflict {
            transition,
            conflict: legion_protocol::FileConflictState {
                state: legion_protocol::FileConflictLifecycleState::ConflictDirty,
                context: legion_protocol::FileConflictContext {
                    workspace_id: legion_protocol::WorkspaceId(0),
                    file_identity: identity,
                    buffer_version: legion_protocol::BufferVersion(1),
                    file_content_version: legion_protocol::FileContentVersion(1),
                    snapshot_id: legion_protocol::SnapshotId(1),
                    disk_fingerprint: None,
                    expected_fingerprint: None,
                    reason: legion_protocol::FileConflictReason::FileDeletedOnDisk,
                    diagnostics: vec![diagnostic.clone()],
                },
                diagnostics: vec![diagnostic],
                schema_version: 1,
            },
        });

        assert!(
            message.contains("disappeared"),
            "the deletion-specific cause was discarded: {message}"
        );
        assert!(
            !message.contains("copy on disk"),
            "the message claims a conflict with a copy that no longer exists: {message}"
        );
        // `view::save_rejection_status_marker` reads these rows by substring, so
        // the condition word is part of the contract, not decoration.
        assert!(
            message.to_ascii_lowercase().contains("conflict"),
            "the condition word must survive for status classification: {message}"
        );
    }

    /// An apply failure must not swear the file is unchanged.
    ///
    /// `ApplyFailed` covers both "the write never happened" and "the write
    /// landed and the metadata read after it did not". Nothing in the response
    /// distinguishes them, so a categorical "was not written" is the same lie
    /// this module exists to remove, one variant over.
    #[test]
    fn an_apply_failure_does_not_claim_the_file_is_unchanged() {
        let mut transition = transition_naming("main.rs");
        transition.lifecycle_state = ProposalLifecycleState::Failed;
        let message = save_rejection_message(&ProposalResponse::Failed {
            transition,
            reason: ProposalFailureReason::ApplyFailed,
        });

        assert!(
            !message.contains("was not written"),
            "an apply failure can happen after the bytes reach disk, so it must not claim \
             otherwise: {message}"
        );
        assert!(
            message.contains("check which"),
            "the message should point at the one place that can settle it: {message}"
        );
        assert!(
            !message.contains("partly written") || message.contains("not a partial"),
            "atomic writes are all-or-nothing; describing a torn file sends someone looking \
             for damage that cannot exist: {message}"
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
