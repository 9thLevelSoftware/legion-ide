//! Export a trainer-ready dataset from the consent-gated training pipeline.
//!
//! P9.F4.T1/T2 landed the pipeline that turns `(audit, proposal)` traces into
//! metadata-only training candidates, re-checks consent at the adapter boundary,
//! and compares the resulting dataset against an archived Legion-Bench baseline.
//! What it did not have was an exit: nothing turned a
//! [`TrainingAdapterDataset`] into bytes a trainer can read.
//!
//! This module is that exit, and it is deliberately the *only* one. A QLoRA run
//! reads `train.jsonl`; if that file could be produced by any path that does not
//! run [`build_training_candidate_corpus`] and
//! [`build_training_adapter_dataset`], the stop condition for P9.F4.T1 ("no
//! non-consented trace in the training candidate set") would be true of the
//! corpus and false of the thing actually fed to the GPU.
//!
//! So the export re-derives consent a third time, from the corpus candidate
//! behind every emitted line, in [`assert_export_is_consented`]. Stage 2 already
//! re-checks what stage 1 filtered; this re-checks what stage 2 emitted. The
//! cost is a hash map and a loop, and the thing it buys is that the file the
//! trainer opens cannot contain a trace whose consent was denied, missing, or
//! lapsed, even if the intermediate artifacts on disk were hand-edited.
//!
//! The exported prompt is built from a fixed whitelist of metadata features
//! (payload kind, affected-file count, risk labels, privacy labels, raw-trace
//! flag). It is not a projection of the candidate struct: a struct grows fields,
//! and a serializer that renders "whatever is on the candidate" would silently
//! start rendering the next one. Adding a feature here has to be a deliberate
//! edit to [`render_instruction`].

use std::{
    collections::{BTreeMap, HashMap},
    fs,
    path::{Path, PathBuf},
};

use legion_observability::training::{
    TrainingAdapterDataset, TrainingAdapterExample, TrainingCandidate, TrainingCandidateCorpus,
    TrainingCandidateLabel, TrainingEvalBaseline, TrainingEvalComparison, TrainingTrace,
    build_training_adapter_dataset, build_training_candidate_corpus,
    build_training_eval_comparison,
};
use legion_protocol::{
    AssistedAiAuditRedactionState, AssistedAiConsentState, FileFingerprint, FileId,
    ProposalLifecycleState, ProposalPayloadKind, ProposalPrivacyLabel, ProposalRiskLabel,
};
use serde::{Deserialize, Serialize};

/// Default source traces: the checked-in P9.F4.T1 fixture batch.
pub const DEFAULT_TRACES_PATH: &str = "evals/training-candidates/source_traces.json";
/// Default archived Legion-Bench baseline the dataset is compared against.
pub const DEFAULT_BASELINE_PATH: &str = "evals/training-candidates/eval_baseline.json";
/// Default output directory for the exported trainer dataset.
pub const DEFAULT_EXPORT_OUTPUT_PATH: &str = "target/training-flywheel";
/// Default corpus identifier, matching the checked-in manifest.
pub const DEFAULT_CORPUS_ID: &str = "consented-accept-reject-v1";
/// Default seed for the deterministic trace expander.
pub const DEFAULT_EXPAND_SEED: u64 = 20_260_819;
/// Default holdout stride: every 4th candidate in corpus order is held out.
pub const DEFAULT_HOLDOUT_EVERY: usize = 4;

/// Prompt template version stamped into the export manifest.
///
/// Bumped whenever [`render_instruction`] changes shape, so an archived training
/// run can be told apart from one whose prompts were rendered differently.
pub const PROMPT_TEMPLATE_VERSION: &str = "legion-consented-decision-v1";

/// Split a candidate was assigned to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExportSplit {
    /// Used for adapter training.
    Train,
    /// Withheld from training and used only for evaluation.
    Holdout,
}

impl ExportSplit {
    /// Stable lowercase name used in serialized artifacts.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Train => "train",
            Self::Holdout => "holdout",
        }
    }
}

/// One trainer-ready line: an instruction, the label to predict, and its split.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExportedExample {
    /// Training-candidate identifier this line was derived from.
    pub example_id: String,
    /// Rendered instruction prompt.
    pub instruction: String,
    /// Target completion: `Accepted` or `Rejected`.
    pub output: String,
    /// Split assignment.
    pub split: ExportSplit,
}

/// Everything the export produced, ready to be written or asserted on.
#[derive(Debug, Clone)]
pub struct TrainingCorpusExport {
    /// The consent-filtered corpus.
    pub corpus: TrainingCandidateCorpus,
    /// The adapter dataset derived from the corpus.
    pub dataset: TrainingAdapterDataset,
    /// The comparison against the archived Legion-Bench baseline.
    pub comparison: TrainingEvalComparison,
    /// Exported lines, in corpus order.
    pub examples: Vec<ExportedExample>,
}

impl TrainingCorpusExport {
    /// Lines assigned to the training split.
    pub fn train(&self) -> impl Iterator<Item = &ExportedExample> {
        self.examples
            .iter()
            .filter(|example| example.split == ExportSplit::Train)
    }

    /// Lines assigned to the holdout split.
    pub fn holdout(&self) -> impl Iterator<Item = &ExportedExample> {
        self.examples
            .iter()
            .filter(|example| example.split == ExportSplit::Holdout)
    }
}

/// Options controlling a corpus export.
#[derive(Debug, Clone)]
pub struct ExportOptions {
    /// Corpus identifier stamped on the corpus artifact.
    pub corpus_id: String,
    /// When non-zero, expand the source batch to this many traces (see
    /// [`expand_traces`]). Zero uses the source batch verbatim.
    pub expand_to: usize,
    /// Seed for the deterministic expander.
    pub seed: u64,
    /// Every Nth candidate in corpus order goes to the holdout split.
    pub holdout_every: usize,
}

impl Default for ExportOptions {
    fn default() -> Self {
        Self {
            corpus_id: DEFAULT_CORPUS_ID.to_string(),
            expand_to: 0,
            seed: DEFAULT_EXPAND_SEED,
            holdout_every: DEFAULT_HOLDOUT_EVERY,
        }
    }
}

/// Render the instruction prompt for one adapter example.
///
/// The field list is a whitelist, not a projection of the example struct: see the
/// module docs. Every value rendered here is a bounded enum or a small integer,
/// so no free text from a proposal can reach the prompt even if a candidate
/// somehow carried some.
#[must_use]
pub fn render_instruction(example: &TrainingAdapterExample) -> String {
    let risk = join_labels(example.risk_labels.iter().map(risk_label_str));
    let privacy = join_labels(example.privacy_labels.iter().map(privacy_label_str));
    format!(
        "Legion proposal review.\n\
         Decide whether the reviewer accepted or rejected this proposal.\n\
         payload_kind: {}\n\
         affected_files: {}\n\
         risk_labels: {risk}\n\
         privacy_labels: {privacy}\n\
         carries_raw_trace: {}\n\
         decision:",
        payload_kind_str(example.payload_kind),
        example.affected_file_count,
        example.carries_raw_trace,
    )
}

fn join_labels<'a>(labels: impl Iterator<Item = &'a str>) -> String {
    let joined = labels.collect::<Vec<_>>().join(",");
    if joined.is_empty() {
        "none".to_string()
    } else {
        joined
    }
}

fn payload_kind_str(kind: ProposalPayloadKind) -> &'static str {
    match kind {
        ProposalPayloadKind::TextEdit => "TextEdit",
        ProposalPayloadKind::CreateFile => "CreateFile",
        ProposalPayloadKind::DeleteFile => "DeleteFile",
        ProposalPayloadKind::RenameFile => "RenameFile",
        ProposalPayloadKind::SaveFile => "SaveFile",
        ProposalPayloadKind::FormatFile => "FormatFile",
        ProposalPayloadKind::CodeAction => "CodeAction",
        ProposalPayloadKind::WorkspaceEdit => "WorkspaceEdit",
        ProposalPayloadKind::TerminalCommand => "TerminalCommand",
        ProposalPayloadKind::Batch => "Batch",
    }
}

fn risk_label_str(label: &ProposalRiskLabel) -> &'static str {
    match label {
        ProposalRiskLabel::Informational => "Informational",
        ProposalRiskLabel::Low => "Low",
        ProposalRiskLabel::Medium => "Medium",
        ProposalRiskLabel::High => "High",
        ProposalRiskLabel::Unknown => "Unknown",
    }
}

fn privacy_label_str(label: &ProposalPrivacyLabel) -> &'static str {
    match label {
        ProposalPrivacyLabel::PublicMetadata => "PublicMetadata",
        ProposalPrivacyLabel::WorkspaceMetadata => "WorkspaceMetadata",
        ProposalPrivacyLabel::RedactedSensitive => "RedactedSensitive",
        ProposalPrivacyLabel::ExternalEgressMetadata => "ExternalEgressMetadata",
        ProposalPrivacyLabel::Unknown => "Unknown",
    }
}

/// Target completion for a label.
#[must_use]
pub fn label_output(label: TrainingCandidateLabel) -> &'static str {
    match label {
        TrainingCandidateLabel::Accepted => "Accepted",
        TrainingCandidateLabel::Rejected => "Rejected",
    }
}

/// Consent states the pipeline treats as consented.
///
/// Duplicated from `legion-observability`'s private `is_consented` on purpose:
/// this check exists to catch a *disagreement* with the pipeline, and a check
/// that calls the thing it is auditing cannot catch one. If the pipeline ever
/// widens its notion of consent without this list widening too, the export
/// fails closed rather than shipping the new state to a trainer.
fn export_permits_consent(state: AssistedAiConsentState) -> bool {
    matches!(
        state,
        AssistedAiConsentState::Granted | AssistedAiConsentState::NotRequired
    )
}

/// Reasons an export is refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExportRefusal {
    /// An exported line has no corresponding adapter-dataset example.
    ExampleNotInDataset(String),
    /// An adapter-dataset example has no corresponding corpus candidate.
    ExampleNotInCorpus(String),
    /// A candidate behind an exported line is not consented.
    UnconsentedCandidate {
        /// Candidate identifier.
        candidate_id: String,
        /// Consent state that failed the check.
        consent_state: String,
    },
    /// A candidate behind an exported line is not metadata-only.
    NonMetadataOnlyCandidate {
        /// Candidate identifier.
        candidate_id: String,
        /// Why the candidate failed.
        reason: &'static str,
    },
    /// The corpus counters do not account for every source trace.
    CorpusAccountingMismatch {
        /// Traces offered to the corpus builder.
        source_trace_count: usize,
        /// Retained + dropped, which must equal the above.
        accounted: usize,
    },
    /// An exported line's label disagrees with its candidate's label.
    LabelMismatch {
        /// Candidate identifier.
        candidate_id: String,
        /// Label the exported line carries.
        exported: String,
        /// Label the candidate carries.
        candidate: String,
    },
}

impl std::fmt::Display for ExportRefusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ExampleNotInDataset(id) => write!(
                f,
                "exported example `{id}` is not in the adapter dataset; it did not pass the consent boundary"
            ),
            Self::ExampleNotInCorpus(id) => write!(
                f,
                "adapter example `{id}` has no corpus candidate; its consent posture cannot be re-derived"
            ),
            Self::UnconsentedCandidate {
                candidate_id,
                consent_state,
            } => write!(
                f,
                "candidate `{candidate_id}` reached the export with consent state `{consent_state}`"
            ),
            Self::NonMetadataOnlyCandidate {
                candidate_id,
                reason,
            } => write!(
                f,
                "candidate `{candidate_id}` reached the export but is not metadata-only: {reason}"
            ),
            Self::CorpusAccountingMismatch {
                source_trace_count,
                accounted,
            } => write!(
                f,
                "corpus counters account for {accounted} of {source_trace_count} source traces; a trace was neither retained nor dropped"
            ),
            Self::LabelMismatch {
                candidate_id,
                exported,
                candidate,
            } => write!(
                f,
                "exported label `{exported}` for `{candidate_id}` disagrees with the candidate label `{candidate}`"
            ),
        }
    }
}

/// Re-derive, from the corpus, that every exported line is consented and
/// metadata-only.
///
/// This runs after the dataset boundary, not instead of it. See the module docs
/// for why a third check is not redundant.
pub fn assert_export_is_consented(
    corpus: &TrainingCandidateCorpus,
    dataset: &TrainingAdapterDataset,
    examples: &[ExportedExample],
) -> Result<(), ExportRefusal> {
    let accounted = corpus.candidate_count
        + corpus.skipped_unconsented_count
        + corpus.skipped_non_terminal_count;
    if accounted != corpus.source_trace_count {
        return Err(ExportRefusal::CorpusAccountingMismatch {
            source_trace_count: corpus.source_trace_count,
            accounted,
        });
    }

    let candidates: HashMap<&str, &TrainingCandidate> = corpus
        .candidates
        .iter()
        .map(|candidate| (candidate.candidate_id.as_str(), candidate))
        .collect();
    let dataset_examples: HashMap<&str, &TrainingAdapterExample> = dataset
        .examples
        .iter()
        .map(|example| (example.example_id.as_str(), example))
        .collect();

    for example in examples {
        let Some(adapter_example) = dataset_examples.get(example.example_id.as_str()) else {
            return Err(ExportRefusal::ExampleNotInDataset(
                example.example_id.clone(),
            ));
        };
        let Some(candidate) = candidates.get(example.example_id.as_str()) else {
            return Err(ExportRefusal::ExampleNotInCorpus(
                example.example_id.clone(),
            ));
        };
        if !export_permits_consent(candidate.consent_state) {
            return Err(ExportRefusal::UnconsentedCandidate {
                candidate_id: candidate.candidate_id.clone(),
                consent_state: format!("{:?}", candidate.consent_state),
            });
        }
        if candidate.redaction_state != AssistedAiAuditRedactionState::MetadataOnly {
            return Err(ExportRefusal::NonMetadataOnlyCandidate {
                candidate_id: candidate.candidate_id.clone(),
                reason: "audit redaction state is not metadata-only",
            });
        }
        if candidate.raw_trace_reference.is_none()
            && candidate.proposal_payload_summary.title.is_some()
        {
            return Err(ExportRefusal::NonMetadataOnlyCandidate {
                candidate_id: candidate.candidate_id.clone(),
                reason: "payload title retained without a raw-trace opt-in row",
            });
        }
        if example.output != label_output(adapter_example.label) {
            return Err(ExportRefusal::LabelMismatch {
                candidate_id: candidate.candidate_id.clone(),
                exported: example.output.clone(),
                candidate: label_output(adapter_example.label).to_string(),
            });
        }
    }

    Ok(())
}

/// Build the export: corpus, dataset, baseline comparison, and trainer lines.
pub fn build_export(
    traces: &[TrainingTrace],
    baseline: &TrainingEvalBaseline,
    options: &ExportOptions,
) -> Result<TrainingCorpusExport, String> {
    let corpus = build_training_candidate_corpus(options.corpus_id.clone(), traces)
        .map_err(|err| format!("unable to build consented corpus: {err}"))?;
    let dataset = build_training_adapter_dataset(&corpus)
        .map_err(|err| format!("unable to build adapter dataset: {err}"))?;
    let comparison = build_training_eval_comparison(&dataset, baseline);

    let holdout_every = options.holdout_every.max(1);
    let examples = dataset
        .examples
        .iter()
        .enumerate()
        .map(|(index, example)| {
            let split = if holdout_every > 1 && index % holdout_every == holdout_every - 1 {
                ExportSplit::Holdout
            } else {
                ExportSplit::Train
            };
            ExportedExample {
                example_id: example.example_id.clone(),
                instruction: render_instruction(example),
                output: label_output(example.label).to_string(),
                split,
            }
        })
        .collect::<Vec<_>>();

    assert_export_is_consented(&corpus, &dataset, &examples)
        .map_err(|refusal| format!("consent re-check refused the export: {refusal}"))?;

    Ok(TrainingCorpusExport {
        corpus,
        dataset,
        comparison,
        examples,
    })
}

/// Deterministically expand a source batch to `target` traces.
///
/// Real telemetry is not in this repository and never will be: the consented
/// corpus is metadata-only by construction and the checked-in fixture is seven
/// traces. Seven traces cannot produce a train/holdout split with anything to
/// measure, so the expander mints a larger *fixture* batch with the same shape.
///
/// It is a fixture generator, not a simulator of user behaviour. The label is a
/// stated function of the metadata features plus seeded noise (see
/// [`synthetic_lifecycle`]), and the consent mix deliberately includes denied,
/// missing, and renewal-required traces so the consent filter has something to
/// drop on every run. Anything measured on this batch is a statement about the
/// pipeline, not about how reviewers behave.
#[must_use]
pub fn expand_traces(source: &[TrainingTrace], target: usize, seed: u64) -> Vec<TrainingTrace> {
    if source.is_empty() || target <= source.len() {
        return source.to_vec();
    }
    let template = &source[0];
    let mut out = source.to_vec();
    let mut rng = SplitMix64::new(seed);

    for ordinal in source.len()..target {
        let proposal_id = 100_000_u64 + ordinal as u64;
        let sequence = 1_000_u64 + ordinal as u64;

        let kind =
            SYNTHETIC_PAYLOAD_KINDS[(rng.next() % SYNTHETIC_PAYLOAD_KINDS.len() as u64) as usize];
        let risk =
            SYNTHETIC_RISK_LABELS[(rng.next() % SYNTHETIC_RISK_LABELS.len() as u64) as usize];
        let privacy =
            SYNTHETIC_PRIVACY_LABELS[(rng.next() % SYNTHETIC_PRIVACY_LABELS.len() as u64) as usize];
        let affected_file_count = 1 + (rng.next() % 6) as usize;
        let consent =
            SYNTHETIC_CONSENT_STATES[(rng.next() % SYNTHETIC_CONSENT_STATES.len() as u64) as usize];
        let noise_roll = rng.next() % 100;
        let non_terminal_roll = rng.next() % 100;

        let mut trace = template.clone();
        trace.audit.audit_id = format!("assist:audit:req-{proposal_id}:{sequence}");
        trace.audit.consent_disposition = Some(consent);
        trace.audit.proposal_id = Some(legion_protocol::ProposalId(proposal_id));
        trace.audit.correlation_id = legion_protocol::CorrelationId(proposal_id);
        trace.audit.event_sequence = legion_protocol::EventSequence(sequence);
        trace.audit.request_contract_id = format!("assist:req:{proposal_id}");
        trace.audit.request_contract_hash = FileFingerprint {
            algorithm: "hash".to_string(),
            value: format!("request-hash-{proposal_id}"),
        };
        trace.audit.route_decision_id = format!("assist:route:req-{proposal_id}");
        trace.audit.route_decision_hash = FileFingerprint {
            algorithm: "hash".to_string(),
            value: format!("route-hash-{proposal_id}"),
        };
        trace.audit.preview_id = Some(format!("assist:preview:{proposal_id}"));
        trace.audit.preview_hash = Some(FileFingerprint {
            algorithm: "hash".to_string(),
            value: format!("preview-hash-{proposal_id}"),
        });
        trace.audit.risk_labels = vec![risk];
        trace.audit.privacy_labels = vec![privacy];

        trace.proposal.proposal_id = legion_protocol::ProposalId(proposal_id);
        trace.proposal.correlation_id = legion_protocol::CorrelationId(proposal_id);
        trace.proposal.payload_summary.kind = kind;
        trace.proposal.payload_summary.affected_files = (0..affected_file_count)
            .map(|offset| FileId(u128::from(proposal_id) + offset as u128))
            .collect();
        trace.proposal.lifecycle_state = synthetic_lifecycle(
            kind,
            risk,
            affected_file_count,
            noise_roll,
            non_terminal_roll,
        );

        out.push(trace);
    }

    out
}

const SYNTHETIC_PAYLOAD_KINDS: [ProposalPayloadKind; 6] = [
    ProposalPayloadKind::TextEdit,
    ProposalPayloadKind::CreateFile,
    ProposalPayloadKind::DeleteFile,
    ProposalPayloadKind::CodeAction,
    ProposalPayloadKind::WorkspaceEdit,
    ProposalPayloadKind::TerminalCommand,
];

const SYNTHETIC_RISK_LABELS: [ProposalRiskLabel; 4] = [
    ProposalRiskLabel::Informational,
    ProposalRiskLabel::Low,
    ProposalRiskLabel::Medium,
    ProposalRiskLabel::High,
];

const SYNTHETIC_PRIVACY_LABELS: [ProposalPrivacyLabel; 3] = [
    ProposalPrivacyLabel::PublicMetadata,
    ProposalPrivacyLabel::WorkspaceMetadata,
    ProposalPrivacyLabel::ExternalEgressMetadata,
];

/// Consent mix for the expander. Two of five states are consented, so roughly
/// 60% of expanded traces are dropped by the consent filter and the drop
/// counter is never zero by accident.
const SYNTHETIC_CONSENT_STATES: [AssistedAiConsentState; 5] = [
    AssistedAiConsentState::Granted,
    AssistedAiConsentState::NotRequired,
    AssistedAiConsentState::Denied,
    AssistedAiConsentState::Missing,
    AssistedAiConsentState::RenewalRequired,
];

/// The stated labelling rule for expanded fixture traces.
///
/// Rejected when the change is destructive (`DeleteFile`, `TerminalCommand`),
/// high risk, or medium risk touching four or more files; accepted otherwise.
/// `noise_roll < 8` flips the label so the task is not perfectly separable and a
/// model cannot reach 100% by memorising the rule. `non_terminal_roll < 6`
/// leaves the proposal in `Previewed`, which the pipeline drops as non-terminal.
fn synthetic_lifecycle(
    kind: ProposalPayloadKind,
    risk: ProposalRiskLabel,
    affected_file_count: usize,
    noise_roll: u64,
    non_terminal_roll: u64,
) -> ProposalLifecycleState {
    if non_terminal_roll < 6 {
        return ProposalLifecycleState::Previewed;
    }
    let destructive = matches!(
        kind,
        ProposalPayloadKind::DeleteFile | ProposalPayloadKind::TerminalCommand
    );
    let mut rejected = destructive
        || risk == ProposalRiskLabel::High
        || (risk == ProposalRiskLabel::Medium && affected_file_count >= 4);
    if noise_roll < 8 {
        rejected = !rejected;
    }
    if rejected {
        ProposalLifecycleState::Rejected
    } else {
        ProposalLifecycleState::Approved
    }
}

/// SplitMix64. Chosen for the same reason the pipeline chose FNV-1a: the job is
/// reproducibility across machines, not cryptographic quality, and it needs no
/// dependency the workspace does not already have.
struct SplitMix64 {
    state: u64,
}

impl SplitMix64 {
    fn new(seed: u64) -> Self {
        Self { state: seed }
    }

    fn next(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }
}

/// Serialize exported lines of one split to JSONL.
pub fn serialize_split_jsonl(
    export: &TrainingCorpusExport,
    split: ExportSplit,
) -> Result<String, String> {
    let mut out = String::new();
    for example in export
        .examples
        .iter()
        .filter(|example| example.split == split)
    {
        let line = serde_json::to_string(&serde_json::json!({
            "example_id": example.example_id,
            "instruction": example.instruction,
            "output": example.output,
            "split": example.split.as_str(),
        }))
        .map_err(|err| format!("unable to serialize exported example: {err}"))?;
        out.push_str(&line);
        out.push('\n');
    }
    Ok(out)
}

/// Manifest describing an export, archived next to the trained adapter.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExportManifest {
    /// Manifest schema version.
    pub schema_version: u16,
    /// Prompt template version used to render instructions.
    pub prompt_template_version: String,
    /// Corpus identifier.
    pub corpus_id: String,
    /// Traces offered to the corpus builder.
    pub source_trace_count: usize,
    /// Candidates retained after consent filtering.
    pub candidate_count: usize,
    /// Traces dropped for want of consent.
    pub skipped_unconsented_count: usize,
    /// Traces dropped for never reaching a terminal lifecycle state.
    pub skipped_non_terminal_count: usize,
    /// Accepted-label candidates.
    pub accepted_count: usize,
    /// Rejected-label candidates.
    pub rejected_count: usize,
    /// Corpus fingerprint.
    pub corpus_fingerprint: String,
    /// Adapter dataset fingerprint.
    pub dataset_fingerprint: String,
    /// Lines in the training split.
    pub train_count: usize,
    /// Lines in the holdout split.
    pub holdout_count: usize,
    /// Holdout stride used for the split.
    pub holdout_every: usize,
    /// Expander seed, when the source batch was expanded.
    pub expand_seed: u64,
    /// Target size the source batch was expanded to; 0 when unexpanded.
    pub expand_to: usize,
    /// Consent states observed among retained candidates, with counts.
    pub retained_consent_states: BTreeMap<String, usize>,
    /// Comparison against the archived Legion-Bench baseline.
    pub comparison: TrainingEvalComparison,
}

/// Build the manifest for an export.
#[must_use]
pub fn build_manifest(export: &TrainingCorpusExport, options: &ExportOptions) -> ExportManifest {
    let mut retained_consent_states = BTreeMap::new();
    for candidate in &export.corpus.candidates {
        *retained_consent_states
            .entry(format!("{:?}", candidate.consent_state))
            .or_insert(0) += 1;
    }

    ExportManifest {
        schema_version: 1,
        prompt_template_version: PROMPT_TEMPLATE_VERSION.to_string(),
        corpus_id: export.corpus.corpus_id.clone(),
        source_trace_count: export.corpus.source_trace_count,
        candidate_count: export.corpus.candidate_count,
        skipped_unconsented_count: export.corpus.skipped_unconsented_count,
        skipped_non_terminal_count: export.corpus.skipped_non_terminal_count,
        accepted_count: export.corpus.accepted_count,
        rejected_count: export.corpus.rejected_count,
        corpus_fingerprint: export.corpus.corpus_fingerprint.clone(),
        dataset_fingerprint: export.dataset.dataset_fingerprint.clone(),
        train_count: export.train().count(),
        holdout_count: export.holdout().count(),
        holdout_every: options.holdout_every.max(1),
        expand_seed: options.seed,
        expand_to: options.expand_to,
        retained_consent_states,
        comparison: export.comparison.clone(),
    }
}

/// Read a source trace batch from JSON.
pub fn read_traces(path: &Path) -> Result<Vec<TrainingTrace>, String> {
    let text = fs::read_to_string(path)
        .map_err(|err| format!("unable to read source traces `{}`: {err}", path.display()))?;
    serde_json::from_str(&text)
        .map_err(|err| format!("unable to parse source traces `{}`: {err}", path.display()))
}

/// Read an archived Legion-Bench baseline from JSON.
pub fn read_baseline(path: &Path) -> Result<TrainingEvalBaseline, String> {
    let text = fs::read_to_string(path)
        .map_err(|err| format!("unable to read eval baseline `{}`: {err}", path.display()))?;
    serde_json::from_str(&text)
        .map_err(|err| format!("unable to parse eval baseline `{}`: {err}", path.display()))
}

/// Write `train.jsonl`, `holdout.jsonl`, and `export_manifest.json`.
pub fn write_export(
    out_dir: &Path,
    export: &TrainingCorpusExport,
    manifest: &ExportManifest,
) -> Result<Vec<PathBuf>, String> {
    fs::create_dir_all(out_dir).map_err(|err| {
        format!(
            "unable to create export output dir `{}`: {err}",
            out_dir.display()
        )
    })?;

    let mut written = Vec::new();
    for (name, split) in [
        ("train.jsonl", ExportSplit::Train),
        ("holdout.jsonl", ExportSplit::Holdout),
    ] {
        let path = out_dir.join(name);
        fs::write(&path, serialize_split_jsonl(export, split)?)
            .map_err(|err| format!("unable to write `{}`: {err}", path.display()))?;
        written.push(path);
    }

    let manifest_path = out_dir.join("export_manifest.json");
    let text = serde_json::to_string_pretty(manifest)
        .map_err(|err| format!("unable to serialize export manifest: {err}"))?;
    fs::write(&manifest_path, format!("{text}\n"))
        .map_err(|err| format!("unable to write `{}`: {err}", manifest_path.display()))?;
    written.push(manifest_path);

    Ok(written)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn baseline() -> TrainingEvalBaseline {
        TrainingEvalBaseline {
            baseline_id: "legion-bench-v0".to_string(),
            suite_fingerprint: "bench-suite-v1:bd2aa3a7d84d9485".to_string(),
            accepted_rate_bp: 6666,
            schema_version: 1,
        }
    }

    /// The consented states, written out as literals.
    ///
    /// A test that asserts "every retained candidate is consented" by calling
    /// [`export_permits_consent`] proves nothing: widen that function and the
    /// assertion widens with it. Mutating the function to accept
    /// `RenewalRequired` was caught only by an incidental count assertion in
    /// one other test, which is not a check, it is luck. Every consent
    /// assertion below compares against this list instead.
    fn assert_state_is_consented(state: AssistedAiConsentState, context: &str) {
        assert!(
            matches!(
                state,
                AssistedAiConsentState::Granted | AssistedAiConsentState::NotRequired
            ),
            "{context}: consent state {state:?} is not Granted or NotRequired"
        );
    }

    /// The states the pipeline must never retain, written out as literals for
    /// the same reason.
    const UNCONSENTED_STATES: [AssistedAiConsentState; 3] = [
        AssistedAiConsentState::Denied,
        AssistedAiConsentState::Missing,
        AssistedAiConsentState::RenewalRequired,
    ];

    fn checked_in_traces() -> Vec<TrainingTrace> {
        let text = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../evals/training-candidates/source_traces.json"
        ));
        serde_json::from_str(text).expect("checked-in source traces parse")
    }

    #[test]
    fn checked_in_batch_exports_only_consented_candidates() {
        let traces = checked_in_traces();
        let export =
            build_export(&traces, &baseline(), &ExportOptions::default()).expect("export builds");

        assert_eq!(export.corpus.source_trace_count, 7);
        assert_eq!(export.corpus.candidate_count, 3);
        assert_eq!(export.corpus.skipped_unconsented_count, 3);
        assert_eq!(export.corpus.skipped_non_terminal_count, 1);
        assert_eq!(export.examples.len(), 3);
        for candidate in &export.corpus.candidates {
            assert_state_is_consented(candidate.consent_state, &candidate.candidate_id);
        }
    }

    /// Pins the export's consent list against the pipeline's, in the direction
    /// that can actually leak.
    ///
    /// `build_training_candidate_corpus` filters first, so widening
    /// [`export_permits_consent`] alone changes nothing observable end to end —
    /// the second list can only ever be narrower in effect. What it defends
    /// against is the pipeline widening: a corpus artifact that already carries
    /// a non-consented candidate, which is what a hand-edited or
    /// future-pipeline corpus looks like from here. So this test builds that
    /// corpus directly, one state at a time, rather than hoping a batch
    /// produces one.
    #[test]
    fn every_unconsented_state_is_refused_at_the_export_boundary() {
        let traces = checked_in_traces();
        let corpus = build_training_candidate_corpus(DEFAULT_CORPUS_ID, &traces)
            .expect("corpus builds from fixture");
        let dataset = build_training_adapter_dataset(&corpus).expect("dataset builds");
        let examples = dataset
            .examples
            .iter()
            .map(|example| ExportedExample {
                example_id: example.example_id.clone(),
                instruction: render_instruction(example),
                output: label_output(example.label).to_string(),
                split: ExportSplit::Train,
            })
            .collect::<Vec<_>>();

        for state in UNCONSENTED_STATES {
            let mut tampered = corpus.clone();
            tampered.candidates[0].consent_state = state;
            let refusal = assert_export_is_consented(&tampered, &dataset, &examples)
                .expect_err("a non-consented candidate must refuse the export");
            assert!(
                matches!(
                    refusal,
                    ExportRefusal::UnconsentedCandidate { ref consent_state, .. }
                        if consent_state == &format!("{state:?}")
                ),
                "`{state:?}` produced the wrong refusal: {refusal:?}"
            );
        }
    }

    #[test]
    fn unconsented_traces_never_reach_the_exported_lines() {
        // Every unconsented state, paired with a terminal lifecycle state, so the
        // only thing that can drop them is the consent filter.
        let traces = checked_in_traces();
        let unconsented_audit_ids = traces
            .iter()
            .filter(|trace| {
                trace
                    .audit
                    .consent_disposition
                    .is_some_and(|state| UNCONSENTED_STATES.contains(&state))
            })
            .map(|trace| trace.audit.audit_id.clone())
            .collect::<Vec<_>>();
        assert_eq!(
            unconsented_audit_ids.len(),
            3,
            "fixture must carry unconsented traces for this test to mean anything"
        );

        let export =
            build_export(&traces, &baseline(), &ExportOptions::default()).expect("export builds");
        let exported = serialize_split_jsonl(&export, ExportSplit::Train)
            .expect("train serializes")
            + &serialize_split_jsonl(&export, ExportSplit::Holdout).expect("holdout serializes");

        for audit_id in unconsented_audit_ids {
            assert!(
                !exported.contains(&audit_id),
                "unconsented audit `{audit_id}` appears in the exported trainer dataset"
            );
        }
    }

    #[test]
    fn a_hand_edited_corpus_cannot_smuggle_an_unconsented_line_past_the_export() {
        // The scenario the third check exists for: a corpus artifact edited on
        // disk so a denied trace looks retained. Stage 2 refuses it, and if stage
        // 2 were ever relaxed, `assert_export_is_consented` still refuses.
        let traces = checked_in_traces();
        let mut corpus = build_training_candidate_corpus(DEFAULT_CORPUS_ID, &traces)
            .expect("corpus builds from fixture");
        let dataset = build_training_adapter_dataset(&corpus).expect("dataset builds");
        let examples = dataset
            .examples
            .iter()
            .map(|example| ExportedExample {
                example_id: example.example_id.clone(),
                instruction: render_instruction(example),
                output: label_output(example.label).to_string(),
                split: ExportSplit::Train,
            })
            .collect::<Vec<_>>();
        assert!(assert_export_is_consented(&corpus, &dataset, &examples).is_ok());

        corpus.candidates[0].consent_state = AssistedAiConsentState::Denied;
        let refusal = assert_export_is_consented(&corpus, &dataset, &examples)
            .expect_err("a denied candidate must refuse the export");
        assert!(
            matches!(refusal, ExportRefusal::UnconsentedCandidate { .. }),
            "expected an unconsented-candidate refusal, got {refusal:?}"
        );
    }

    #[test]
    fn a_retained_payload_title_refuses_the_export() {
        let traces = checked_in_traces();
        let mut corpus = build_training_candidate_corpus(DEFAULT_CORPUS_ID, &traces)
            .expect("corpus builds from fixture");
        let dataset = build_training_adapter_dataset(&corpus).expect("dataset builds");
        let examples = dataset
            .examples
            .iter()
            .map(|example| ExportedExample {
                example_id: example.example_id.clone(),
                instruction: render_instruction(example),
                output: label_output(example.label).to_string(),
                split: ExportSplit::Train,
            })
            .collect::<Vec<_>>();

        corpus.candidates[0].proposal_payload_summary.title =
            Some("Fix acceptance edge case in ledger reconciliation".to_string());
        let refusal = assert_export_is_consented(&corpus, &dataset, &examples)
            .expect_err("a retained title must refuse the export");
        assert!(
            matches!(refusal, ExportRefusal::NonMetadataOnlyCandidate { .. }),
            "expected a metadata-only refusal, got {refusal:?}"
        );
    }

    #[test]
    fn a_line_with_no_dataset_example_refuses_the_export() {
        let traces = checked_in_traces();
        let corpus = build_training_candidate_corpus(DEFAULT_CORPUS_ID, &traces)
            .expect("corpus builds from fixture");
        let dataset = build_training_adapter_dataset(&corpus).expect("dataset builds");
        let examples = vec![ExportedExample {
            example_id: "training-candidate:assist:audit:req-9001:91:accepted".to_string(),
            instruction: "smuggled".to_string(),
            output: "Accepted".to_string(),
            split: ExportSplit::Train,
        }];

        let refusal = assert_export_is_consented(&corpus, &dataset, &examples)
            .expect_err("a line with no dataset example must refuse the export");
        assert!(
            matches!(refusal, ExportRefusal::ExampleNotInDataset(_)),
            "expected a not-in-dataset refusal, got {refusal:?}"
        );
    }

    #[test]
    fn a_flipped_label_refuses_the_export() {
        let traces = checked_in_traces();
        let corpus = build_training_candidate_corpus(DEFAULT_CORPUS_ID, &traces)
            .expect("corpus builds from fixture");
        let dataset = build_training_adapter_dataset(&corpus).expect("dataset builds");
        let mut examples = dataset
            .examples
            .iter()
            .map(|example| ExportedExample {
                example_id: example.example_id.clone(),
                instruction: render_instruction(example),
                output: label_output(example.label).to_string(),
                split: ExportSplit::Train,
            })
            .collect::<Vec<_>>();
        examples[0].output = if examples[0].output == "Accepted" {
            "Rejected".to_string()
        } else {
            "Accepted".to_string()
        };

        let refusal = assert_export_is_consented(&corpus, &dataset, &examples)
            .expect_err("a flipped label must refuse the export");
        assert!(
            matches!(refusal, ExportRefusal::LabelMismatch { .. }),
            "expected a label-mismatch refusal, got {refusal:?}"
        );
    }

    #[test]
    fn corpus_accounting_must_cover_every_source_trace() {
        let traces = checked_in_traces();
        let mut corpus = build_training_candidate_corpus(DEFAULT_CORPUS_ID, &traces)
            .expect("corpus builds from fixture");
        let dataset = build_training_adapter_dataset(&corpus).expect("dataset builds");

        corpus.skipped_unconsented_count -= 1;
        let refusal = assert_export_is_consented(&corpus, &dataset, &[])
            .expect_err("an unaccounted source trace must refuse the export");
        assert!(
            matches!(refusal, ExportRefusal::CorpusAccountingMismatch { .. }),
            "expected an accounting refusal, got {refusal:?}"
        );
    }

    #[test]
    fn the_expander_is_deterministic_and_drops_unconsented_traces() {
        let traces = checked_in_traces();
        let first = expand_traces(&traces, 400, DEFAULT_EXPAND_SEED);
        let second = expand_traces(&traces, 400, DEFAULT_EXPAND_SEED);
        assert_eq!(first.len(), 400);
        assert_eq!(
            serde_json::to_string(&first).expect("first serializes"),
            serde_json::to_string(&second).expect("second serializes"),
            "the expander must be reproducible from its seed"
        );

        let export = build_export(&first, &baseline(), &ExportOptions::default())
            .expect("expanded export builds");
        assert!(
            export.corpus.skipped_unconsented_count > 0,
            "the expanded batch must exercise the consent filter"
        );
        assert!(
            export.corpus.skipped_non_terminal_count > 0,
            "the expanded batch must exercise the non-terminal filter"
        );
        for candidate in &export.corpus.candidates {
            assert_state_is_consented(candidate.consent_state, &candidate.candidate_id);
        }
    }

    #[test]
    fn a_different_seed_produces_a_different_batch() {
        let traces = checked_in_traces();
        let first = expand_traces(&traces, 200, DEFAULT_EXPAND_SEED);
        let second = expand_traces(&traces, 200, DEFAULT_EXPAND_SEED + 1);
        assert_ne!(
            serde_json::to_string(&first).expect("first serializes"),
            serde_json::to_string(&second).expect("second serializes"),
            "a different seed must produce a different batch"
        );
    }

    #[test]
    fn the_rendered_instruction_carries_no_free_text() {
        let traces = checked_in_traces();
        let export = build_export(
            &expand_traces(&traces, 200, DEFAULT_EXPAND_SEED),
            &baseline(),
            &ExportOptions::default(),
        )
        .expect("export builds");
        // The fixture proposals all carry this title; the metadata-only corpus
        // strips it, and the prompt renderer must not reintroduce it.
        const FIXTURE_TITLE: &str = "Fix acceptance edge case in ledger reconciliation";
        for example in &export.examples {
            assert!(
                !example.instruction.contains(FIXTURE_TITLE),
                "prompt leaked the stripped proposal title: {}",
                example.instruction
            );
            assert!(
                example.instruction.ends_with("decision:"),
                "prompt must end at the decision cue: {}",
                example.instruction
            );
        }
    }

    #[test]
    fn the_split_is_deterministic_and_disjoint() {
        let traces = expand_traces(&checked_in_traces(), 400, DEFAULT_EXPAND_SEED);
        let export =
            build_export(&traces, &baseline(), &ExportOptions::default()).expect("export builds");
        let train_ids = export
            .train()
            .map(|example| example.example_id.clone())
            .collect::<Vec<_>>();
        let holdout_ids = export
            .holdout()
            .map(|example| example.example_id.clone())
            .collect::<Vec<_>>();

        assert!(!train_ids.is_empty());
        assert!(!holdout_ids.is_empty());
        assert_eq!(train_ids.len() + holdout_ids.len(), export.examples.len());
        for id in &holdout_ids {
            assert!(
                !train_ids.contains(id),
                "`{id}` is in both splits; the holdout would be memorised, not measured"
            );
        }
    }

    #[test]
    fn the_manifest_records_the_comparison_and_the_drop_counts() {
        let traces = expand_traces(&checked_in_traces(), 400, DEFAULT_EXPAND_SEED);
        let options = ExportOptions {
            expand_to: 400,
            ..ExportOptions::default()
        };
        let export = build_export(&traces, &baseline(), &options).expect("export builds");
        let manifest = build_manifest(&export, &options);

        assert_eq!(manifest.prompt_template_version, PROMPT_TEMPLATE_VERSION);
        assert_eq!(manifest.source_trace_count, 400);
        assert_eq!(
            manifest.candidate_count
                + manifest.skipped_unconsented_count
                + manifest.skipped_non_terminal_count,
            400
        );
        assert_eq!(manifest.comparison.baseline_id, "legion-bench-v0");
        assert_eq!(manifest.comparison.baseline_accepted_rate_bp, 6666);
        assert!(
            manifest
                .retained_consent_states
                .keys()
                .all(|state| state == "Granted" || state == "NotRequired"),
            "the manifest recorded a non-consented state among retained candidates: {:?}",
            manifest.retained_consent_states
        );
    }
}
