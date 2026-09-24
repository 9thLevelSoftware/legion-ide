//! Consent-gated conversion of acceptance/rejection metadata into eval/training candidates.
//!
//! The pipeline has three stages and every stage re-checks consent:
//!
//! 1. [`build_training_candidate_corpus`] turns raw `(audit, proposal)` traces into
//!    metadata-only [`TrainingCandidate`]s, dropping any trace that is not consented
//!    and any trace that has not reached an acceptance/rejection lifecycle state.
//! 2. [`build_training_adapter_dataset`] turns a corpus into adapter training input.
//! 3. [`build_training_eval_comparison`] compares that dataset against an archived
//!    Legion-Bench baseline.
//!
//! Stage 2 re-validates consent rather than trusting stage 1. That is deliberate: a
//! corpus is a serializable, checked-in artifact, so it can reach the adapter from a
//! file on disk that no live consent check ever produced. The only way to keep the
//! stop condition ("no non-consented trace in the training candidate set") true for
//! *that* path is to re-check at the boundary that actually feeds the trainer.

use legion_protocol::{
    AssistedAiAuditOutcomeCategory, AssistedAiAuditRecord, AssistedAiAuditRedactionState,
    AssistedAiConsentState, AssistedAiProviderInvocationState, CausalityId, CorrelationId,
    EventSequence, FileFingerprint, PermissionBudgetEvaluationDisposition, ProposalAuditRecord,
    ProposalId, ProposalLifecycleState, ProposalPayloadKind, ProposalPayloadSummary,
    ProposalPrivacyLabel, ProposalRiskLabel, RedactionHint, TimestampMillis, WorkspaceId,
    validate_assisted_ai_audit_record,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Schema version stamped on every artifact this module produces.
pub const TRAINING_PIPELINE_SCHEMA_VERSION: u16 = 1;

/// Failures raised by the consent-gated training pipeline.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum TrainingCandidateError {
    /// The assisted-AI audit record failed metadata-only protocol validation.
    #[error("assisted-AI audit record failed metadata-only validation")]
    InvalidAuditRecord,
    /// The audit record and the proposal audit record disagree on proposal identity.
    #[error("assisted-AI audit proposal identity does not match the proposal audit record")]
    MismatchedProposalIdentity,
    /// A candidate reached the adapter boundary without a consent grant.
    #[error(
        "training candidate `{candidate_id}` is not consented (consent state `{consent_state}`)"
    )]
    UnconsentedCandidate {
        /// Candidate that failed the check.
        candidate_id: String,
        /// Consent state recorded on the candidate.
        consent_state: String,
    },
    /// A candidate reached the adapter boundary carrying more than metadata.
    #[error("training candidate `{candidate_id}` is not metadata-only: {reason}")]
    NonMetadataOnlyCandidate {
        /// Candidate that failed the check.
        candidate_id: String,
        /// Which metadata-only invariant was violated.
        reason: &'static str,
    },
    /// Raw trace attachment was attempted without a live raw-trace opt-in row.
    #[error("raw trace attachment requires a live raw-trace opt-in row")]
    RawTraceOptInMissing,
    /// Raw trace attachment was attempted without boundary redaction enforcement.
    #[error("raw trace attachment requires boundary redaction enforcement")]
    RawTraceRedactionNotEnforced,
    /// A corpus could not be serialized to JSONL.
    #[error("training corpus serialization failed")]
    SerializationFailed,
}

/// Training label derived from consented proposal lifecycle metadata.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TrainingCandidateLabel {
    /// The trace corresponds to an accepted proposal.
    Accepted,
    /// The trace corresponds to a rejected proposal.
    Rejected,
}

impl TrainingCandidateLabel {
    fn as_str(self) -> &'static str {
        match self {
            Self::Accepted => "accepted",
            Self::Rejected => "rejected",
        }
    }
}

/// Metadata-only proof that a raw-trace opt-in row exists in the retention ledger.
///
/// This is the only key that unlocks raw payload retention on a training candidate.
/// It is minted by `legion-retention` from a live ledger row and never constructed
/// from caller-supplied booleans, so a caller cannot assert its own consent.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RawTraceOptInAttestation {
    /// Identifier of the opt-in row backing this attestation.
    pub row_id: String,
    /// Workspace the opt-in row covers.
    pub workspace_id: WorkspaceId,
    /// Metadata-only retention purpose label.
    pub purpose_label: String,
    /// Expiry of the backing opt-in row.
    pub expires_at: TimestampMillis,
    /// Whether the backing row additionally permits hosted export.
    pub export_allowed: bool,
    /// Whether the minting store enforces redaction before any raw bytes are sealed.
    pub redaction_enforced: bool,
    /// Attestation schema version.
    pub schema_version: u16,
}

/// Pointer from a training candidate to a raw trace held in the retention vault.
///
/// The candidate carries the pointer, never the bytes: a training corpus is copied,
/// shipped, and diffed, and none of those operations should move raw source.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RawTraceReference {
    /// Retention vault bundle holding the raw trace.
    pub bundle_id: String,
    /// Opt-in row that authorized the raw trace.
    pub opt_in_row_id: String,
    /// Whether the retention boundary enforced redaction before sealing.
    pub redaction_enforced: bool,
    /// Reference schema version.
    pub schema_version: u16,
}

/// One consented acceptance/rejection trace: an assisted-AI audit plus its proposal audit.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrainingTrace {
    /// Assisted-AI audit record carrying the consent disposition.
    pub audit: AssistedAiAuditRecord,
    /// Proposal audit record carrying the acceptance/rejection lifecycle state.
    pub proposal: ProposalAuditRecord,
}

/// Metadata-only training candidate derived from a consented acceptance or rejection trace.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrainingCandidate {
    /// Stable training-candidate identifier.
    pub candidate_id: String,
    /// Source assisted-AI audit record identifier.
    pub audit_id: String,
    /// Proposal identifier tied to the acceptance/rejection outcome.
    pub proposal_id: ProposalId,
    /// Acceptance/rejection label.
    pub label: TrainingCandidateLabel,
    /// Consent posture for the trace.
    pub consent_state: AssistedAiConsentState,
    /// Proposal lifecycle state recorded for the trace.
    pub proposal_lifecycle_state: ProposalLifecycleState,
    /// Audit outcome category captured for the trace.
    pub outcome_category: AssistedAiAuditOutcomeCategory,
    /// Proposal payload summary for eval comparison.
    ///
    /// The free-text `title` is stripped unless a raw-trace opt-in attestation was
    /// supplied: a proposal title is model- or user-authored prose that can carry
    /// identifiers, snippets, and secrets, and nothing downstream of here needs it.
    pub proposal_payload_summary: ProposalPayloadSummary,
    /// Byte length of the payload title that was stripped, when one was present.
    pub payload_title_byte_count: Option<u64>,
    /// Number of files the proposal touched.
    pub affected_file_count: usize,
    /// Request-contract hash that anchors the trace.
    pub request_contract_hash: FileFingerprint,
    /// Route-decision hash that anchors the trace.
    pub route_decision_hash: FileFingerprint,
    /// Preview hash when the trace produced preview metadata.
    pub preview_hash: Option<FileFingerprint>,
    /// Correlation identifier for replay stitching.
    pub correlation_id: CorrelationId,
    /// Causality identifier for replay stitching.
    pub causality_id: CausalityId,
    /// Event sequence for deterministic ordering.
    pub event_sequence: EventSequence,
    /// Redaction hints preserved for the training artifact.
    pub redaction_hints: Vec<RedactionHint>,
    /// Audit redaction state.
    pub redaction_state: AssistedAiAuditRedactionState,
    /// Runtime invocation state; always metadata-only here.
    pub runtime_invocation_state: AssistedAiProviderInvocationState,
    /// Budget dispositions copied into the candidate for eval reproducibility.
    pub budget_dispositions: Vec<PermissionBudgetEvaluationDisposition>,
    /// Proposal risk labels preserved in the training artifact.
    pub risk_labels: Vec<ProposalRiskLabel>,
    /// Proposal privacy labels preserved in the training artifact.
    pub privacy_labels: Vec<ProposalPrivacyLabel>,
    /// Pointer to a retained raw trace, present only under a raw-trace opt-in row.
    pub raw_trace_reference: Option<RawTraceReference>,
    /// Schema version for the training candidate DTO.
    pub schema_version: u16,
}

/// Deterministic, consent-filtered corpus of training candidates.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrainingCandidateCorpus {
    /// Stable corpus identifier.
    pub corpus_id: String,
    /// Number of source traces offered to the builder.
    pub source_trace_count: usize,
    /// Number of candidates retained.
    pub candidate_count: usize,
    /// Number of accepted-label candidates.
    pub accepted_count: usize,
    /// Number of rejected-label candidates.
    pub rejected_count: usize,
    /// Traces dropped because consent was absent, denied, or needed renewal.
    pub skipped_unconsented_count: usize,
    /// Traces dropped because the proposal never reached acceptance or rejection.
    pub skipped_non_terminal_count: usize,
    /// Fingerprint over the retained candidates, in corpus order.
    pub corpus_fingerprint: String,
    /// Retained candidates, sorted by `candidate_id`.
    pub candidates: Vec<TrainingCandidate>,
    /// Corpus schema version.
    pub schema_version: u16,
}

/// One adapter training example derived from a re-validated training candidate.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrainingAdapterExample {
    /// Example identifier; equal to the source candidate identifier.
    pub example_id: String,
    /// Acceptance/rejection label the adapter trains against.
    pub label: TrainingCandidateLabel,
    /// Request-contract hash anchoring the example to a replayable request.
    pub request_contract_hash: FileFingerprint,
    /// Route-decision hash anchoring the example to a replayable route.
    pub route_decision_hash: FileFingerprint,
    /// Proposal payload kind used as a categorical feature.
    pub payload_kind: ProposalPayloadKind,
    /// Number of files the proposal touched, used as a scalar feature.
    pub affected_file_count: usize,
    /// Proposal risk labels used as categorical features.
    pub risk_labels: Vec<ProposalRiskLabel>,
    /// Proposal privacy labels used as categorical features.
    pub privacy_labels: Vec<ProposalPrivacyLabel>,
    /// Whether the example is backed by a retained raw trace.
    pub carries_raw_trace: bool,
    /// Example schema version.
    pub schema_version: u16,
}

/// Adapter training dataset bound to the corpus it was derived from.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrainingAdapterDataset {
    /// Fingerprint of the corpus this dataset was derived from.
    pub corpus_fingerprint: String,
    /// Fingerprint over the adapter examples, in dataset order.
    pub dataset_fingerprint: String,
    /// Number of accepted-label examples.
    pub accepted_count: usize,
    /// Number of rejected-label examples.
    pub rejected_count: usize,
    /// Adapter examples, in corpus order.
    pub examples: Vec<TrainingAdapterExample>,
    /// Dataset schema version.
    pub schema_version: u16,
}

/// Archived Legion-Bench baseline the adapter dataset is compared against.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrainingEvalBaseline {
    /// Baseline identifier, e.g. the bench suite name.
    pub baseline_id: String,
    /// Bench suite fingerprint recorded when the baseline was archived.
    pub suite_fingerprint: String,
    /// Baseline acceptance rate in basis points.
    ///
    /// Basis points rather than a float: this value is compared for equality across
    /// runs and machines, and binary floating point does not survive that reliably.
    pub accepted_rate_bp: u32,
    /// Baseline schema version.
    pub schema_version: u16,
}

/// Reproducible comparison of an adapter dataset against an archived baseline.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrainingEvalComparison {
    /// Baseline identifier.
    pub baseline_id: String,
    /// Bench suite fingerprint of the baseline.
    pub baseline_suite_fingerprint: String,
    /// Fingerprint of the corpus behind the dataset.
    pub corpus_fingerprint: String,
    /// Fingerprint of the adapter dataset.
    pub dataset_fingerprint: String,
    /// Baseline acceptance rate in basis points.
    pub baseline_accepted_rate_bp: u32,
    /// Dataset acceptance rate in basis points.
    pub dataset_accepted_rate_bp: u32,
    /// Dataset rate minus baseline rate, in basis points.
    pub delta_bp: i64,
    /// Whether the dataset acceptance rate fell below the baseline.
    pub regressed: bool,
    /// Comparison schema version.
    pub schema_version: u16,
}

/// Build a metadata-only training candidate from a consented audit/proposal pair.
///
/// Returns `Ok(None)` when the trace is not consented or does not correspond to an
/// acceptance/rejection lifecycle state.
pub fn consented_training_candidate_from_records(
    audit: &AssistedAiAuditRecord,
    proposal: &ProposalAuditRecord,
) -> Result<Option<TrainingCandidate>, TrainingCandidateError> {
    build_candidate(audit, proposal, None)
}

/// Build a training candidate that points at a retained raw trace.
///
/// The attestation must be live (unexpired at `now`) and must come from a store that
/// enforces redaction before sealing. Without both, this refuses rather than falling
/// back to the metadata-only candidate: a caller that asked for the raw path and
/// silently got the metadata path would not know its opt-in had lapsed.
pub fn consented_training_candidate_with_raw_trace(
    audit: &AssistedAiAuditRecord,
    proposal: &ProposalAuditRecord,
    attestation: &RawTraceOptInAttestation,
    bundle_id: &str,
    now: TimestampMillis,
) -> Result<Option<TrainingCandidate>, TrainingCandidateError> {
    if attestation.row_id.trim().is_empty()
        || attestation.schema_version == 0
        || bundle_id.trim().is_empty()
        || attestation.expires_at.0 <= now.0
    {
        return Err(TrainingCandidateError::RawTraceOptInMissing);
    }
    if !attestation.redaction_enforced {
        return Err(TrainingCandidateError::RawTraceRedactionNotEnforced);
    }
    build_candidate(
        audit,
        proposal,
        Some(RawTraceReference {
            bundle_id: bundle_id.to_string(),
            opt_in_row_id: attestation.row_id.clone(),
            redaction_enforced: true,
            schema_version: TRAINING_PIPELINE_SCHEMA_VERSION,
        }),
    )
}

/// Convert a batch of traces into a deterministic, consent-filtered corpus.
///
/// Non-consented and non-terminal traces are counted and dropped rather than erroring:
/// a real batch arrives mixed, and the caller needs the consented remainder. Structural
/// contradictions (an audit pointing at a different proposal than the proposal record)
/// still fail the whole batch, because those indicate a broken trace writer.
pub fn build_training_candidate_corpus(
    corpus_id: impl Into<String>,
    traces: &[TrainingTrace],
) -> Result<TrainingCandidateCorpus, TrainingCandidateError> {
    let mut candidates = Vec::new();
    let mut skipped_unconsented_count = 0;
    let mut skipped_non_terminal_count = 0;

    for trace in traces {
        if !is_consented(trace.audit.consent_disposition) {
            skipped_unconsented_count += 1;
            continue;
        }
        match consented_training_candidate_from_records(&trace.audit, &trace.proposal)? {
            Some(candidate) => candidates.push(candidate),
            None => skipped_non_terminal_count += 1,
        }
    }

    candidates.sort_by(|left, right| left.candidate_id.cmp(&right.candidate_id));

    let accepted_count = candidates
        .iter()
        .filter(|candidate| candidate.label == TrainingCandidateLabel::Accepted)
        .count();
    let rejected_count = candidates.len() - accepted_count;
    let corpus_fingerprint = fingerprint_candidates(&candidates);

    Ok(TrainingCandidateCorpus {
        corpus_id: corpus_id.into(),
        source_trace_count: traces.len(),
        candidate_count: candidates.len(),
        accepted_count,
        rejected_count,
        skipped_unconsented_count,
        skipped_non_terminal_count,
        corpus_fingerprint,
        candidates,
        schema_version: TRAINING_PIPELINE_SCHEMA_VERSION,
    })
}

/// Serialize a corpus to JSONL, one candidate per line.
pub fn serialize_corpus_jsonl(
    corpus: &TrainingCandidateCorpus,
) -> Result<String, TrainingCandidateError> {
    let mut out = String::new();
    for candidate in &corpus.candidates {
        let line = serde_json::to_string(candidate)
            .map_err(|_| TrainingCandidateError::SerializationFailed)?;
        out.push_str(&line);
        out.push('\n');
    }
    Ok(out)
}

/// Convert a corpus into adapter training input, re-checking consent on every candidate.
///
/// This is the last gate before bytes reach a trainer, so it does not trust the corpus
/// it was handed. A corpus is a file; files get hand-edited, merged, and restored from
/// backups, and none of those paths run the stage-1 consent filter.
pub fn build_training_adapter_dataset(
    corpus: &TrainingCandidateCorpus,
) -> Result<TrainingAdapterDataset, TrainingCandidateError> {
    let mut examples = Vec::with_capacity(corpus.candidates.len());
    for candidate in &corpus.candidates {
        assert_candidate_is_trainable(candidate)?;
        examples.push(TrainingAdapterExample {
            example_id: candidate.candidate_id.clone(),
            label: candidate.label,
            request_contract_hash: candidate.request_contract_hash.clone(),
            route_decision_hash: candidate.route_decision_hash.clone(),
            payload_kind: candidate.proposal_payload_summary.kind,
            affected_file_count: candidate.affected_file_count,
            risk_labels: candidate.risk_labels.clone(),
            privacy_labels: candidate.privacy_labels.clone(),
            carries_raw_trace: candidate.raw_trace_reference.is_some(),
            schema_version: TRAINING_PIPELINE_SCHEMA_VERSION,
        });
    }

    let accepted_count = examples
        .iter()
        .filter(|example| example.label == TrainingCandidateLabel::Accepted)
        .count();
    let rejected_count = examples.len() - accepted_count;
    let dataset_fingerprint = fingerprint_examples(&examples);

    Ok(TrainingAdapterDataset {
        corpus_fingerprint: corpus.corpus_fingerprint.clone(),
        dataset_fingerprint,
        accepted_count,
        rejected_count,
        examples,
        schema_version: TRAINING_PIPELINE_SCHEMA_VERSION,
    })
}

/// Compare an adapter dataset's acceptance rate against an archived bench baseline.
#[must_use]
pub fn build_training_eval_comparison(
    dataset: &TrainingAdapterDataset,
    baseline: &TrainingEvalBaseline,
) -> TrainingEvalComparison {
    let total = dataset.accepted_count + dataset.rejected_count;
    // An empty dataset reports a 0 bp acceptance rate, which registers as a regression
    // against any non-zero baseline. That is the intended reading: a corpus that lost
    // all its candidates is a problem, not a neutral result.
    let dataset_accepted_rate_bp = (dataset.accepted_count * 10_000)
        .checked_div(total)
        .and_then(|rate| u32::try_from(rate).ok())
        .unwrap_or(0);
    let delta_bp = i64::from(dataset_accepted_rate_bp) - i64::from(baseline.accepted_rate_bp);

    TrainingEvalComparison {
        baseline_id: baseline.baseline_id.clone(),
        baseline_suite_fingerprint: baseline.suite_fingerprint.clone(),
        corpus_fingerprint: dataset.corpus_fingerprint.clone(),
        dataset_fingerprint: dataset.dataset_fingerprint.clone(),
        baseline_accepted_rate_bp: baseline.accepted_rate_bp,
        dataset_accepted_rate_bp,
        delta_bp,
        regressed: delta_bp < 0,
        schema_version: TRAINING_PIPELINE_SCHEMA_VERSION,
    }
}

fn is_consented(consent: Option<AssistedAiConsentState>) -> bool {
    matches!(
        consent,
        Some(AssistedAiConsentState::Granted) | Some(AssistedAiConsentState::NotRequired)
    )
}

fn build_candidate(
    audit: &AssistedAiAuditRecord,
    proposal: &ProposalAuditRecord,
    raw_trace_reference: Option<RawTraceReference>,
) -> Result<Option<TrainingCandidate>, TrainingCandidateError> {
    let Some(consent_state) = audit.consent_disposition else {
        return Ok(None);
    };
    if !is_consented(Some(consent_state)) {
        return Ok(None);
    }

    validate_assisted_ai_audit_record(audit)
        .map_err(|_| TrainingCandidateError::InvalidAuditRecord)?;

    let label = match proposal.lifecycle_state {
        ProposalLifecycleState::Approved => TrainingCandidateLabel::Accepted,
        ProposalLifecycleState::Rejected => TrainingCandidateLabel::Rejected,
        _ => return Ok(None),
    };

    let Some(proposal_id) = audit.proposal_id else {
        return Ok(None);
    };
    if proposal_id != proposal.proposal_id {
        return Err(TrainingCandidateError::MismatchedProposalIdentity);
    }

    let keeps_raw_payload = raw_trace_reference.is_some();
    let payload_title_byte_count = proposal
        .payload_summary
        .title
        .as_ref()
        .map(|title| title.len() as u64);
    let proposal_payload_summary = if keeps_raw_payload {
        proposal.payload_summary.clone()
    } else {
        ProposalPayloadSummary {
            title: None,
            ..proposal.payload_summary.clone()
        }
    };

    Ok(Some(TrainingCandidate {
        candidate_id: format!("training-candidate:{}:{}", audit.audit_id, label.as_str()),
        audit_id: audit.audit_id.clone(),
        proposal_id,
        label,
        consent_state,
        proposal_lifecycle_state: proposal.lifecycle_state,
        outcome_category: audit.outcome_category,
        affected_file_count: proposal.payload_summary.affected_files.len(),
        proposal_payload_summary,
        payload_title_byte_count,
        request_contract_hash: audit.request_contract_hash.clone(),
        route_decision_hash: audit.route_decision_hash.clone(),
        preview_hash: audit.preview_hash.clone(),
        correlation_id: audit.correlation_id,
        causality_id: audit.causality_id,
        event_sequence: audit.event_sequence,
        redaction_hints: audit.redaction_hints.clone(),
        redaction_state: audit.redaction_state,
        runtime_invocation_state: audit.runtime_invocation_state,
        budget_dispositions: audit.budget_dispositions.clone(),
        risk_labels: audit.risk_labels.clone(),
        privacy_labels: audit.privacy_labels.clone(),
        raw_trace_reference,
        schema_version: audit.schema_version,
    }))
}

fn assert_candidate_is_trainable(
    candidate: &TrainingCandidate,
) -> Result<(), TrainingCandidateError> {
    if !is_consented(Some(candidate.consent_state)) {
        return Err(TrainingCandidateError::UnconsentedCandidate {
            candidate_id: candidate.candidate_id.clone(),
            consent_state: format!("{:?}", candidate.consent_state),
        });
    }
    if candidate.redaction_state != AssistedAiAuditRedactionState::MetadataOnly {
        return Err(TrainingCandidateError::NonMetadataOnlyCandidate {
            candidate_id: candidate.candidate_id.clone(),
            reason: "audit redaction state is not metadata-only",
        });
    }
    if candidate.runtime_invocation_state != AssistedAiProviderInvocationState::NotEncoded {
        return Err(TrainingCandidateError::NonMetadataOnlyCandidate {
            candidate_id: candidate.candidate_id.clone(),
            reason: "provider invocation state is encoded",
        });
    }
    // A raw payload title is only allowed alongside a raw-trace reference. Without one,
    // a title in the corpus means the redaction step was bypassed or reverted by hand.
    if candidate.raw_trace_reference.is_none() && candidate.proposal_payload_summary.title.is_some()
    {
        return Err(TrainingCandidateError::NonMetadataOnlyCandidate {
            candidate_id: candidate.candidate_id.clone(),
            reason: "payload title retained without a raw-trace opt-in row",
        });
    }
    if let Some(reference) = &candidate.raw_trace_reference
        && !reference.redaction_enforced
    {
        return Err(TrainingCandidateError::NonMetadataOnlyCandidate {
            candidate_id: candidate.candidate_id.clone(),
            reason: "raw trace reference does not record redaction enforcement",
        });
    }
    Ok(())
}

/// FNV-1a 64. Chosen because the fingerprint's job is reproducibility (same input,
/// same hex string, on any machine) rather than resistance to a chosen-collision
/// attack, and because it needs no dependency this crate does not already have.
fn fnv1a(seed: u64, bytes: &[u8]) -> u64 {
    let mut hash = seed;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100_0000_01b3);
    }
    hash
}

const FNV_OFFSET_BASIS: u64 = 0xcbf2_9ce4_8422_2325;

fn fingerprint_candidates(candidates: &[TrainingCandidate]) -> String {
    let mut hash = FNV_OFFSET_BASIS;
    for candidate in candidates {
        hash = fnv1a(
            hash,
            format!(
                "{}|{}|{:?}|{:?}|{}|{}|{:?}|{}|{}",
                candidate.candidate_id,
                candidate.label.as_str(),
                candidate.consent_state,
                candidate.proposal_lifecycle_state,
                candidate.request_contract_hash.value,
                candidate.route_decision_hash.value,
                candidate.redaction_state,
                candidate.affected_file_count,
                candidate.raw_trace_reference.is_some(),
            )
            .as_bytes(),
        );
    }
    format!("training-corpus-v1:{hash:016x}")
}

fn fingerprint_examples(examples: &[TrainingAdapterExample]) -> String {
    let mut hash = FNV_OFFSET_BASIS;
    for example in examples {
        hash = fnv1a(
            hash,
            format!(
                "{}|{}|{}|{}|{:?}|{}|{}",
                example.example_id,
                example.label.as_str(),
                example.request_contract_hash.value,
                example.route_decision_hash.value,
                example.payload_kind,
                example.affected_file_count,
                example.carries_raw_trace,
            )
            .as_bytes(),
        );
    }
    format!("training-adapter-v1:{hash:016x}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use legion_protocol::{
        AssistedAiAuditPrivacyDisposition, FileId, PermissionBudgetEvaluationDisposition,
    };
    use uuid::Uuid;

    const SOURCE_TRACES: &str = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../evals/training-candidates/source_traces.json"
    ));
    const CHECKED_IN_CORPUS: &str = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../evals/training-candidates/consented_accept_reject.jsonl"
    ));
    const CHECKED_IN_MANIFEST: &str = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../evals/training-candidates/corpus_manifest.json"
    ));
    const CHECKED_IN_BASELINE: &str = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../evals/training-candidates/eval_baseline.json"
    ));

    /// The corpus id the checked-in manifest was generated under. Changing it changes
    /// nothing about the fingerprints, but keeps the regenerated artifacts identical.
    const CORPUS_ID: &str = "consented-accept-reject-v1";

    /// The proposal title carried by every fixture trace. The metadata-only corpus must
    /// never contain it.
    const FIXTURE_TITLE: &str = "Fix acceptance edge case in ledger reconciliation";

    #[derive(Debug, serde::Deserialize)]
    struct ManifestFixture {
        corpus_id: String,
        source_trace_count: usize,
        candidate_count: usize,
        accepted_count: usize,
        rejected_count: usize,
        skipped_unconsented_count: usize,
        skipped_non_terminal_count: usize,
        corpus_fingerprint: String,
        dataset_fingerprint: String,
        comparison: TrainingEvalComparison,
    }

    fn source_traces() -> Vec<TrainingTrace> {
        serde_json::from_str(SOURCE_TRACES).expect("source trace fixture must parse")
    }

    fn baseline() -> TrainingEvalBaseline {
        serde_json::from_str(CHECKED_IN_BASELINE).expect("baseline fixture must parse")
    }

    fn manifest() -> ManifestFixture {
        serde_json::from_str(CHECKED_IN_MANIFEST).expect("manifest fixture must parse")
    }

    /// Working-copy line endings are CRLF on Windows checkouts; compare content, not
    /// the platform's newline convention.
    fn normalized_lines(text: &str) -> Vec<String> {
        text.lines()
            .map(|line| line.trim_end_matches('\r').to_string())
            .filter(|line| !line.is_empty())
            .collect()
    }

    fn audit_record(consent: AssistedAiConsentState) -> AssistedAiAuditRecord {
        AssistedAiAuditRecord {
            audit_id: "assist:audit:req-1:77".to_string(),
            provider_capability_id: "provider:local-redacted".to_string(),
            provider_capability_hash: FileFingerprint {
                algorithm: "hash".to_string(),
                value: "provider-hash".to_string(),
            },
            route_decision_id: "assist:route:req-1".to_string(),
            route_decision_hash: FileFingerprint {
                algorithm: "hash".to_string(),
                value: "route-hash".to_string(),
            },
            consent_disposition: Some(consent),
            budget_dispositions: vec![PermissionBudgetEvaluationDisposition::Allowed],
            privacy_disposition: AssistedAiAuditPrivacyDisposition::Allowed,
            request_contract_id: "assist:req:1".to_string(),
            request_contract_hash: FileFingerprint {
                algorithm: "hash".to_string(),
                value: "request-hash".to_string(),
            },
            projection_id: Some("assisted-ai:p6-3".to_string()),
            projection_hash: Some(FileFingerprint {
                algorithm: "hash".to_string(),
                value: "projection-hash".to_string(),
            }),
            preview_id: Some("assist:preview:701".to_string()),
            preview_hash: Some(FileFingerprint {
                algorithm: "hash".to_string(),
                value: "preview-hash".to_string(),
            }),
            proposal_id: Some(ProposalId(701)),
            outcome_category: AssistedAiAuditOutcomeCategory::ProposalPreviewReady,
            refusal_error_category: None,
            correlation_id: CorrelationId(901),
            causality_id: CausalityId(
                Uuid::parse_str("11111111-1111-1111-1111-111111111111").unwrap(),
            ),
            event_sequence: EventSequence(77),
            risk_labels: vec![ProposalRiskLabel::Medium],
            privacy_labels: vec![ProposalPrivacyLabel::WorkspaceMetadata],
            redaction_state: AssistedAiAuditRedactionState::MetadataOnly,
            runtime_invocation_state: AssistedAiProviderInvocationState::NotEncoded,
            runtime_activation_labels: vec![
                "provider.invocation.not_encoded".to_string(),
                "network.not_encoded".to_string(),
                "tool.disabled".to_string(),
                "agent.disabled".to_string(),
                "terminal.disabled".to_string(),
            ],
            redaction_hints: vec![RedactionHint::MetadataOnly],
            schema_version: 1,
        }
    }

    fn proposal_record(state: ProposalLifecycleState) -> ProposalAuditRecord {
        ProposalAuditRecord {
            proposal_id: ProposalId(701),
            lifecycle_state: state,
            timestamp: legion_protocol::TimestampMillis(1_717_171_717),
            principal: legion_protocol::PrincipalId("principal-1".to_string()),
            capability: legion_protocol::CapabilityId("cap:proposal".to_string()),
            correlation_id: CorrelationId(901),
            causality_id: CausalityId(
                Uuid::parse_str("22222222-2222-2222-2222-222222222222").unwrap(),
            ),
            payload_summary: ProposalPayloadSummary {
                kind: ProposalPayloadKind::TextEdit,
                affected_files: vec![FileId(11), FileId(12)],
                title: Some(FIXTURE_TITLE.to_string()),
                byte_count: Some(144),
            },
            checkpoint_rollback_projection: None,
            risk_rule_ids: vec!["risk.rule.accepted".to_string()],
            diagnostics: vec![],
            redaction_hints: vec![RedactionHint::MetadataOnly],
            schema_version: 1,
        }
    }

    /// The seven-trace fixture batch that `evals/training-candidates/` is generated
    /// from: three consented terminal traces, three unconsented traces, and one
    /// consented trace that never reached a terminal lifecycle state.
    fn fixture_traces() -> Vec<TrainingTrace> {
        let spec: &[(
            &str,
            u64,
            u64,
            AssistedAiConsentState,
            ProposalLifecycleState,
        )] = &[
            (
                "assist:audit:req-1001:11",
                1001,
                11,
                AssistedAiConsentState::Granted,
                ProposalLifecycleState::Approved,
            ),
            (
                "assist:audit:req-1002:12",
                1002,
                12,
                AssistedAiConsentState::Granted,
                ProposalLifecycleState::Rejected,
            ),
            (
                "assist:audit:req-1003:13",
                1003,
                13,
                AssistedAiConsentState::NotRequired,
                ProposalLifecycleState::Approved,
            ),
            (
                "assist:audit:req-1004:14",
                1004,
                14,
                AssistedAiConsentState::Granted,
                ProposalLifecycleState::Previewed,
            ),
            (
                "assist:audit:req-9001:91",
                9001,
                91,
                AssistedAiConsentState::Denied,
                ProposalLifecycleState::Approved,
            ),
            (
                "assist:audit:req-9002:92",
                9002,
                92,
                AssistedAiConsentState::Missing,
                ProposalLifecycleState::Rejected,
            ),
            (
                "assist:audit:req-9003:93",
                9003,
                93,
                AssistedAiConsentState::RenewalRequired,
                ProposalLifecycleState::Approved,
            ),
        ];

        spec.iter()
            .map(|(audit_id, proposal, sequence, consent, lifecycle)| {
                let mut audit = audit_record(*consent);
                audit.audit_id = (*audit_id).to_string();
                audit.proposal_id = Some(ProposalId(*proposal));
                audit.correlation_id = CorrelationId(*proposal);
                audit.event_sequence = EventSequence(*sequence);
                audit.request_contract_id = format!("assist:req:{proposal}");
                audit.request_contract_hash = FileFingerprint {
                    algorithm: "hash".to_string(),
                    value: format!("request-hash-{proposal}"),
                };
                audit.route_decision_id = format!("assist:route:req-{proposal}");
                audit.route_decision_hash = FileFingerprint {
                    algorithm: "hash".to_string(),
                    value: format!("route-hash-{proposal}"),
                };
                audit.preview_id = Some(format!("assist:preview:{proposal}"));
                audit.preview_hash = Some(FileFingerprint {
                    algorithm: "hash".to_string(),
                    value: format!("preview-hash-{proposal}"),
                });

                let mut record = proposal_record(*lifecycle);
                record.proposal_id = ProposalId(*proposal);
                record.correlation_id = CorrelationId(*proposal);

                TrainingTrace {
                    audit,
                    proposal: record,
                }
            })
            .collect()
    }

    /// Regenerates the checked-in `evals/training-candidates/` artifacts.
    ///
    /// Ignored by default because it writes into the repository. Run it after an
    /// intentional pipeline change, then review the diff:
    /// `cargo test -p legion-observability regenerate_training_candidate_fixtures -- --ignored`
    #[test]
    #[ignore = "writes repository fixtures; run explicitly after an intentional pipeline change"]
    fn regenerate_training_candidate_fixtures() {
        use std::path::Path;

        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../evals/training-candidates");
        let traces = fixture_traces();
        let corpus = build_training_candidate_corpus(CORPUS_ID, &traces).expect("corpus builds");
        let dataset = build_training_adapter_dataset(&corpus).expect("dataset builds");
        let comparison = build_training_eval_comparison(&dataset, &baseline());

        std::fs::write(
            dir.join("source_traces.json"),
            serde_json::to_string_pretty(&traces).expect("traces serialize"),
        )
        .expect("write source traces");
        std::fs::write(
            dir.join("consented_accept_reject.jsonl"),
            serialize_corpus_jsonl(&corpus).expect("corpus serializes"),
        )
        .expect("write corpus");
        std::fs::write(
            dir.join("corpus_manifest.json"),
            serde_json::to_string_pretty(&serde_json::json!({
                "corpus_id": corpus.corpus_id,
                "source_trace_count": corpus.source_trace_count,
                "candidate_count": corpus.candidate_count,
                "accepted_count": corpus.accepted_count,
                "rejected_count": corpus.rejected_count,
                "skipped_unconsented_count": corpus.skipped_unconsented_count,
                "skipped_non_terminal_count": corpus.skipped_non_terminal_count,
                "corpus_fingerprint": corpus.corpus_fingerprint,
                "dataset_fingerprint": dataset.dataset_fingerprint,
                "comparison": comparison,
            }))
            .expect("manifest serializes"),
        )
        .expect("write manifest");
    }

    fn live_attestation() -> RawTraceOptInAttestation {
        RawTraceOptInAttestation {
            row_id: "raw-trace-opt-in:ws-1:replay".to_string(),
            workspace_id: WorkspaceId(1),
            purpose_label: "Replay".to_string(),
            expires_at: TimestampMillis(900_000),
            export_allowed: false,
            redaction_enforced: true,
            schema_version: 1,
        }
    }

    // -----------------------------------------------------------------------
    // Stage 1: consent filtering
    // -----------------------------------------------------------------------

    #[test]
    fn consented_approved_trace_becomes_training_candidate() {
        let candidate = consented_training_candidate_from_records(
            &audit_record(AssistedAiConsentState::Granted),
            &proposal_record(ProposalLifecycleState::Approved),
        )
        .expect("candidate conversion")
        .expect("consented approved trace should be retained");

        assert_eq!(
            candidate.candidate_id,
            "training-candidate:assist:audit:req-1:77:accepted"
        );
        assert_eq!(candidate.label, TrainingCandidateLabel::Accepted);
        assert_eq!(candidate.consent_state, AssistedAiConsentState::Granted);
        assert_eq!(candidate.affected_file_count, 2);
    }

    /// NEGATIVE: every non-granting consent posture must produce no candidate at all.
    #[test]
    fn unconsented_traces_are_not_converted() {
        for consent in [
            AssistedAiConsentState::Denied,
            AssistedAiConsentState::Missing,
            AssistedAiConsentState::RenewalRequired,
        ] {
            let candidate = consented_training_candidate_from_records(
                &audit_record(consent),
                &proposal_record(ProposalLifecycleState::Approved),
            )
            .expect("candidate conversion");
            assert!(
                candidate.is_none(),
                "consent state {consent:?} must not produce a training candidate"
            );
        }

        let mut absent = audit_record(AssistedAiConsentState::Granted);
        absent.consent_disposition = None;
        assert!(
            consented_training_candidate_from_records(
                &absent,
                &proposal_record(ProposalLifecycleState::Approved),
            )
            .expect("candidate conversion")
            .is_none(),
            "an audit record with no consent disposition must not produce a candidate"
        );
    }

    /// NEGATIVE: the free-text proposal title must not survive into a metadata-only
    /// candidate, and must not appear anywhere in its serialized form.
    #[test]
    fn proposal_title_is_redacted_without_a_raw_trace_opt_in() {
        let candidate = consented_training_candidate_from_records(
            &audit_record(AssistedAiConsentState::Granted),
            &proposal_record(ProposalLifecycleState::Approved),
        )
        .expect("candidate conversion")
        .expect("consented approved trace should be retained");

        assert_eq!(candidate.proposal_payload_summary.title, None);
        assert_eq!(
            candidate.payload_title_byte_count,
            Some(FIXTURE_TITLE.len() as u64)
        );
        assert!(candidate.raw_trace_reference.is_none());

        let encoded = serde_json::to_string(&candidate).expect("candidate serializes");
        assert!(
            !encoded.contains(FIXTURE_TITLE),
            "redacted candidate must not carry the proposal title: {encoded}"
        );
    }

    #[test]
    fn non_metadata_only_audit_traces_are_rejected_before_training_candidate_creation() {
        let mut audit = audit_record(AssistedAiConsentState::Granted);
        audit.redaction_state = AssistedAiAuditRedactionState::FullyRedacted;
        audit.runtime_invocation_state = AssistedAiProviderInvocationState::Completed;

        let err = consented_training_candidate_from_records(
            &audit,
            &proposal_record(ProposalLifecycleState::Approved),
        )
        .expect_err("fully redacted traces must be rejected at the boundary");

        assert_eq!(err, TrainingCandidateError::InvalidAuditRecord);
    }

    // -----------------------------------------------------------------------
    // Raw trace opt-in boundary
    // -----------------------------------------------------------------------

    #[test]
    fn live_opt_in_attestation_retains_the_raw_payload_and_records_the_reference() {
        let candidate = consented_training_candidate_with_raw_trace(
            &audit_record(AssistedAiConsentState::Granted),
            &proposal_record(ProposalLifecycleState::Approved),
            &live_attestation(),
            "bundle:1:901",
            TimestampMillis(1_000),
        )
        .expect("candidate conversion")
        .expect("consented approved trace should be retained");

        assert_eq!(
            candidate.proposal_payload_summary.title.as_deref(),
            Some(FIXTURE_TITLE)
        );
        let reference = candidate
            .raw_trace_reference
            .expect("raw trace reference must be recorded");
        assert_eq!(reference.bundle_id, "bundle:1:901");
        assert_eq!(reference.opt_in_row_id, "raw-trace-opt-in:ws-1:replay");
        assert!(reference.redaction_enforced);
    }

    /// NEGATIVE: an expired opt-in row is not an opt-in row.
    #[test]
    fn expired_opt_in_attestation_refuses_raw_trace_attachment() {
        let err = consented_training_candidate_with_raw_trace(
            &audit_record(AssistedAiConsentState::Granted),
            &proposal_record(ProposalLifecycleState::Approved),
            &live_attestation(),
            "bundle:1:901",
            TimestampMillis(900_001),
        )
        .expect_err("an expired opt-in row must refuse raw trace attachment");

        assert_eq!(err, TrainingCandidateError::RawTraceOptInMissing);
    }

    /// NEGATIVE: an attestation from a store that does not redact is refused.
    #[test]
    fn attestation_without_redaction_enforcement_refuses_raw_trace_attachment() {
        let mut attestation = live_attestation();
        attestation.redaction_enforced = false;

        let err = consented_training_candidate_with_raw_trace(
            &audit_record(AssistedAiConsentState::Granted),
            &proposal_record(ProposalLifecycleState::Approved),
            &attestation,
            "bundle:1:901",
            TimestampMillis(1_000),
        )
        .expect_err("an unredacted store must refuse raw trace attachment");

        assert_eq!(err, TrainingCandidateError::RawTraceRedactionNotEnforced);
    }

    // -----------------------------------------------------------------------
    // Stage 2: adapter boundary re-validation
    // -----------------------------------------------------------------------

    fn corpus_from_fixture() -> TrainingCandidateCorpus {
        build_training_candidate_corpus(CORPUS_ID, &source_traces()).expect("corpus builds")
    }

    /// NEGATIVE: a corpus file that has been hand-edited to carry a denied-consent
    /// candidate must be refused by the adapter, not silently trained on.
    #[test]
    fn adapter_refuses_a_corpus_carrying_an_unconsented_candidate() {
        let mut corpus = corpus_from_fixture();
        corpus.candidates[0].consent_state = AssistedAiConsentState::Denied;

        let err = build_training_adapter_dataset(&corpus)
            .expect_err("an unconsented candidate must not reach the adapter");

        assert!(
            matches!(err, TrainingCandidateError::UnconsentedCandidate { ref consent_state, .. } if consent_state == "Denied"),
            "unexpected error: {err:?}"
        );
    }

    /// NEGATIVE: a corpus carrying a non-metadata-only candidate must be refused.
    #[test]
    fn adapter_refuses_a_corpus_carrying_a_non_metadata_only_candidate() {
        let mut corpus = corpus_from_fixture();
        corpus.candidates[0].runtime_invocation_state =
            AssistedAiProviderInvocationState::Completed;

        let err = build_training_adapter_dataset(&corpus)
            .expect_err("an encoded provider invocation must not reach the adapter");

        assert!(
            matches!(
                err,
                TrainingCandidateError::NonMetadataOnlyCandidate {
                    reason: "provider invocation state is encoded",
                    ..
                }
            ),
            "unexpected error: {err:?}"
        );
    }

    /// NEGATIVE: re-adding a payload title by hand, without a raw-trace reference,
    /// must be refused at the adapter.
    #[test]
    fn adapter_refuses_a_payload_title_reinstated_without_an_opt_in_row() {
        let mut corpus = corpus_from_fixture();
        corpus.candidates[0].proposal_payload_summary.title = Some(FIXTURE_TITLE.to_string());

        let err = build_training_adapter_dataset(&corpus)
            .expect_err("a title without an opt-in row must not reach the adapter");

        assert!(
            matches!(
                err,
                TrainingCandidateError::NonMetadataOnlyCandidate {
                    reason: "payload title retained without a raw-trace opt-in row",
                    ..
                }
            ),
            "unexpected error: {err:?}"
        );
    }

    /// NEGATIVE: a raw-trace reference that does not record redaction enforcement is
    /// refused, so a reference cannot be forged by editing the corpus file.
    #[test]
    fn adapter_refuses_a_raw_trace_reference_without_redaction_enforcement() {
        let mut corpus = corpus_from_fixture();
        corpus.candidates[0].raw_trace_reference = Some(RawTraceReference {
            bundle_id: "bundle:1:901".to_string(),
            opt_in_row_id: "forged".to_string(),
            redaction_enforced: false,
            schema_version: 1,
        });

        let err = build_training_adapter_dataset(&corpus)
            .expect_err("an unredacted raw trace reference must not reach the adapter");

        assert!(
            matches!(
                err,
                TrainingCandidateError::NonMetadataOnlyCandidate {
                    reason: "raw trace reference does not record redaction enforcement",
                    ..
                }
            ),
            "unexpected error: {err:?}"
        );
    }

    // -----------------------------------------------------------------------
    // End-to-end reproducibility
    // -----------------------------------------------------------------------

    /// NEGATIVE: the non-consented fixture traces must be absent from the corpus by
    /// audit id, not merely absent by count.
    #[test]
    fn unconsented_fixture_traces_never_reach_the_corpus() {
        let traces = source_traces();
        let unconsented: Vec<String> = traces
            .iter()
            .filter(|trace| !is_consented(trace.audit.consent_disposition))
            .map(|trace| trace.audit.audit_id.clone())
            .collect();
        assert!(
            !unconsented.is_empty(),
            "the fixture must contain unconsented traces or this test proves nothing"
        );

        let corpus = build_training_candidate_corpus(CORPUS_ID, &traces).expect("corpus builds");
        let encoded = serialize_corpus_jsonl(&corpus).expect("corpus serializes");

        for audit_id in &unconsented {
            assert!(
                !corpus
                    .candidates
                    .iter()
                    .any(|candidate| &candidate.audit_id == audit_id),
                "unconsented audit `{audit_id}` reached the corpus"
            );
            assert!(
                !encoded.contains(audit_id),
                "unconsented audit `{audit_id}` reached the serialized corpus"
            );
        }
        assert_eq!(corpus.skipped_unconsented_count, unconsented.len());
    }

    #[test]
    fn corpus_and_dataset_fingerprints_are_deterministic() {
        let first = corpus_from_fixture();
        let second = corpus_from_fixture();
        assert_eq!(first.corpus_fingerprint, second.corpus_fingerprint);

        let first_dataset = build_training_adapter_dataset(&first).expect("dataset builds");
        let second_dataset = build_training_adapter_dataset(&second).expect("dataset builds");
        assert_eq!(
            first_dataset.dataset_fingerprint,
            second_dataset.dataset_fingerprint
        );
    }

    /// The end-to-end contract: the checked-in corpus, its fingerprints, and its bench
    /// comparison must all be exactly what the pipeline regenerates from the checked-in
    /// source traces. If any stage changes behaviour, this fails.
    #[test]
    fn checked_in_corpus_reproduces_from_the_source_traces() {
        let manifest = manifest();
        let corpus = build_training_candidate_corpus(&manifest.corpus_id, &source_traces())
            .expect("corpus builds");

        assert_eq!(corpus.corpus_id, CORPUS_ID);
        assert_eq!(corpus.source_trace_count, manifest.source_trace_count);
        assert_eq!(corpus.candidate_count, manifest.candidate_count);
        assert_eq!(corpus.accepted_count, manifest.accepted_count);
        assert_eq!(corpus.rejected_count, manifest.rejected_count);
        assert_eq!(
            corpus.skipped_unconsented_count,
            manifest.skipped_unconsented_count
        );
        assert_eq!(
            corpus.skipped_non_terminal_count,
            manifest.skipped_non_terminal_count
        );
        assert_eq!(corpus.corpus_fingerprint, manifest.corpus_fingerprint);

        let regenerated = serialize_corpus_jsonl(&corpus).expect("corpus serializes");
        assert_eq!(
            normalized_lines(&regenerated),
            normalized_lines(CHECKED_IN_CORPUS),
            "checked-in corpus JSONL does not match the regenerated corpus"
        );

        let dataset = build_training_adapter_dataset(&corpus).expect("dataset builds");
        assert_eq!(dataset.dataset_fingerprint, manifest.dataset_fingerprint);
        assert_eq!(dataset.corpus_fingerprint, manifest.corpus_fingerprint);

        let comparison = build_training_eval_comparison(&dataset, &baseline());
        assert_eq!(comparison, manifest.comparison);
    }

    /// The comparison must be able to report a regression, not just agree with itself:
    /// dropping a consented accepted candidate moves the rate below the baseline.
    #[test]
    fn eval_comparison_reports_a_regression_when_acceptance_falls() {
        let mut corpus = corpus_from_fixture();
        let accepted = corpus
            .candidates
            .iter()
            .position(|candidate| candidate.label == TrainingCandidateLabel::Accepted)
            .expect("fixture corpus must contain an accepted candidate");
        corpus.candidates.remove(accepted);

        let dataset = build_training_adapter_dataset(&corpus).expect("dataset builds");
        let comparison = build_training_eval_comparison(&dataset, &baseline());

        assert!(comparison.dataset_accepted_rate_bp < comparison.baseline_accepted_rate_bp);
        assert!(comparison.delta_bp < 0);
        assert!(comparison.regressed);
    }

    /// NEGATIVE: the checked-in corpus must be metadata-only — no raw trace references
    /// and no payload titles — so the artifact in the repo is itself safe to copy.
    #[test]
    fn checked_in_corpus_is_metadata_only() {
        let mut lines = 0;
        for line in normalized_lines(CHECKED_IN_CORPUS) {
            let candidate: TrainingCandidate =
                serde_json::from_str(&line).expect("checked-in candidate parses");
            assert!(candidate.raw_trace_reference.is_none());
            assert_eq!(candidate.proposal_payload_summary.title, None);
            assert_eq!(
                candidate.redaction_state,
                AssistedAiAuditRedactionState::MetadataOnly
            );
            assert!(!line.contains(FIXTURE_TITLE));
            lines += 1;
        }
        assert!(lines > 0, "the checked-in corpus must not be empty");
    }
}
