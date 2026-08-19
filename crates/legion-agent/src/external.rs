//! External-agent containment: the governed lease an external agent runs
//! inside, the read/write requests it is allowed to make there, and the
//! conversion of everything it produced into Legion proposals and evidence.
//!
//! # Containment model
//!
//! An external agent never runs in the main workspace. It runs inside a
//! disposable, leased worktree ([`crate::DelegatedTaskSandboxOrchestrator`])
//! and reaches the filesystem only through [`ExternalAgentSession`], which
//! fails closed on every request it cannot prove is inside the lease.
//!
//! Nothing the agent writes is a workspace mutation. Its edits leave the lease
//! only as [`legion_protocol::WorkspaceProposal`]s and its logs leave only as
//! metadata-only evidence rows.

use std::path::{Path, PathBuf};

use legion_ai::redaction::scan_proposal_payload_for_secrets;
use legion_protocol::{
    AssistedAiContractError, CanonicalPath, CapabilityId, CausalityId, CorrelationId,
    DelegatedTaskRiskTolerance, DelegatedTaskScope, DelegatedTaskScopeTargetKind, FileFingerprint,
    LegionToolKind, PreviewSummary, PrincipalId, ProposalAffectedTarget, ProposalId,
    ProposalPayload, ProposalTargetCoverage, ProposalTargetCoverageKind, ProposalTargetKind,
    ProposalVersionPreconditions, RedactionHint, TimestampMillis, WorkspaceEditProposalPayload,
    WorkspaceEditSourceKind, WorkspaceFileOperation, WorkspaceId, WorkspaceProposal,
    WorkspaceTextEdit,
};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use super::AgentError;
use crate::scope::validate_delegated_task_tool_call;
use crate::worktree::{resolve_lease_relative_read, validate_not_main_workspace};

fn invalid_metadata(reason: impl Into<String>) -> AgentError {
    AgentError::InvalidMetadata(AssistedAiContractError::InvalidProposalMetadata {
        reason: reason.into(),
    })
}

fn preview_summary(payload: &WorkspaceEditProposalPayload) -> PreviewSummary {
    let mut details = vec![format!("source={:?}", payload.source)];
    details.push(format!("file_edits={}", payload.file_edits.len()));
    details.push(format!("file_operations={}", payload.file_operations.len()));
    details.push(format!(
        "target_coverage={:?}",
        payload.target_coverage.coverage_kind
    ));
    PreviewSummary {
        summary: payload.title.clone(),
        details,
    }
}

fn complete_preconditions(edit: &WorkspaceTextEdit) -> bool {
    edit.preconditions.file_version.is_some()
        && edit.preconditions.buffer_version.is_some()
        && edit.preconditions.snapshot_id.is_some()
        && edit.preconditions.generation.is_some()
        && edit.preconditions.file_content_version.is_some()
        && edit.preconditions.workspace_generation.is_some()
        && edit.preconditions.expected_fingerprint.is_some()
}

fn validate_workspace_edit_conversion(
    proposal_id: ProposalId,
    principal: &PrincipalId,
    capability: &CapabilityId,
    correlation_id: CorrelationId,
    causality_id: CausalityId,
    payload: &WorkspaceEditProposalPayload,
    preconditions: &ProposalVersionPreconditions,
) -> Result<(), AgentError> {
    let _ = principal;
    if proposal_id.0 == 0 {
        return Err(invalid_metadata(
            "external proposal requires a non-zero proposal id",
        ));
    }
    if correlation_id.0 == 0 {
        return Err(AgentError::InvalidMetadata(
            AssistedAiContractError::ZeroCorrelationId,
        ));
    }
    if causality_id.0 == Uuid::nil() {
        return Err(AgentError::InvalidMetadata(
            AssistedAiContractError::NilCausalityId,
        ));
    }
    if payload.title.trim().is_empty() {
        return Err(invalid_metadata("external proposal requires a title"));
    }
    if payload.schema_version == 0 {
        return Err(invalid_metadata(
            "external proposal payload schema_version must be non-zero",
        ));
    }
    if payload.required_capability != *capability {
        return Err(invalid_metadata(
            "external proposal capability does not match payload capability",
        ));
    }
    if payload.target_coverage.coverage_kind != ProposalTargetCoverageKind::Complete
        || payload.target_coverage.omitted_target_count != 0
    {
        return Err(invalid_metadata(
            "external proposal requires complete target coverage without omissions",
        ));
    }
    if payload.file_edits.is_empty() && payload.file_operations.is_empty() {
        return Err(invalid_metadata(
            "external proposal requires at least one file edit or file operation",
        ));
    }
    if !payload.file_edits.is_empty()
        && payload
            .file_edits
            .iter()
            .any(|edit| !complete_preconditions(edit))
    {
        return Err(invalid_metadata(
            "external proposal file edits require version and fingerprint preconditions",
        ));
    }
    if preconditions.file_version.is_none()
        && preconditions.buffer_version.is_none()
        && preconditions.snapshot_id.is_none()
        && preconditions.generation.is_none()
        && preconditions.file_content_version.is_none()
        && preconditions.workspace_generation.is_none()
        && preconditions.expected_fingerprint.is_none()
        && !payload.file_edits.is_empty()
    {
        return Err(invalid_metadata(
            "external proposal requires proposal preconditions for file edits",
        ));
    }
    Ok(())
}

/// Input for converting an external edit into a proposal envelope.
#[derive(Debug, Clone)]
pub struct ExternalWorkspaceEditProposalInput {
    /// Stable proposal identifier.
    pub proposal_id: ProposalId,
    /// Principal responsible for the proposal.
    pub principal: PrincipalId,
    /// Capability required before mutation authority may apply the proposal.
    pub capability: CapabilityId,
    /// Audit correlation identifier.
    pub correlation_id: CorrelationId,
    /// Audit causality identifier.
    pub causality_id: CausalityId,
    /// Proposal-ready workspace edit payload.
    pub payload: WorkspaceEditProposalPayload,
    /// Version preconditions copied into the proposal envelope.
    pub preconditions: ProposalVersionPreconditions,
    /// Proposal expiration timestamp.
    pub expires_at: Option<TimestampMillis>,
    /// Proposal creation timestamp.
    pub created_at: TimestampMillis,
}

/// Convert an external edit into a proposal envelope without mutation.
pub fn external_workspace_edit_proposal(
    input: ExternalWorkspaceEditProposalInput,
) -> Result<WorkspaceProposal, AgentError> {
    validate_workspace_edit_conversion(
        input.proposal_id,
        &input.principal,
        &input.capability,
        input.correlation_id,
        input.causality_id,
        &input.payload,
        &input.preconditions,
    )?;

    let ExternalWorkspaceEditProposalInput {
        proposal_id,
        principal,
        capability,
        correlation_id,
        payload,
        preconditions,
        expires_at,
        created_at,
        ..
    } = input;

    let mut preview = preview_summary(&payload);
    let proposal_payload = ProposalPayload::WorkspaceEdit(payload);

    // Externally authored edit text reaches a reviewer's screen and then the
    // working tree. Scan it here, at the point the payload becomes a proposal,
    // rather than trusting the producer.
    //
    // Only counts are recorded, never the rule that fired.
    //
    // `contains_forbidden_phase8_payload` in `legion-protocol` rejects a proposal
    // whose rendering contains markers including `secret`, `token`, `password`,
    // and `api_key`. Ten of the eighteen `SecretRuleId::stable_id()` values
    // contain one of those markers (`github-token`, `stripe-secret-key`,
    // `high-entropy-token`, ...), so a rule-id-bearing annotation would make a
    // proposal unrepresentable depending on *which* credential was found — a
    // proposal that fails to serialize only sometimes is worse than one that
    // never names the rule.
    //
    // The remaining eight escape only because the marker list spells `api_key`
    // with an underscore while the rule ids use a hyphen. That is punctuation
    // luck, not a design property: normalising the separators would push all
    // eighteen into the rejected set, not free them. Do not read the eight as
    // headroom for naming rules here.
    //
    // The count tells the reviewer to look; the rule id lives in the audit log.
    let credential_sites = scan_proposal_payload_for_secrets(&proposal_payload);
    if !credential_sites.is_empty() {
        let finding_count: usize = credential_sites
            .iter()
            .map(|site| site.report.findings.len())
            .sum();
        preview.details.push(format!(
            "credential_scan_sites={} credential_scan_findings={finding_count}",
            credential_sites.len()
        ));
    }

    Ok(WorkspaceProposal {
        proposal_id,
        principal,
        capability,
        correlation_id,
        payload: proposal_payload,
        preconditions,
        preview,
        expires_at,
        created_at,
    })
}

// ---------------------------------------------------------------------------
// P6.F4.T2 — running an external agent inside a Legion-governed lease
// ---------------------------------------------------------------------------

/// Default lease-relative prefixes an external agent may never read.
///
/// `.git` inside a leased worktree is not a directory: it is a link file whose
/// contents name the *main* repository's git directory. Handing it to an
/// external agent discloses the location of the very tree the lease exists to
/// keep out of reach, and the object store it points at holds main-workspace
/// content the agent was not scoped to. The lease boundary itself does not
/// catch this — `.git` is genuinely inside the lease — so it is denied by name.
const DEFAULT_DENIED_READ_PREFIXES: &[&str] = &[".git"];

/// How the external agent reaches the filesystem.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExternalAgentFilesystemAccess {
    /// The agent holds no filesystem handle of its own. Every read is a request
    /// the host answers — the local adapter bridge shape ratified by ADR-0043.
    /// [`ExternalAgentSession::authorize_read`] is then the actual boundary,
    /// because there is no other way for the agent to obtain file content.
    HostBrokered,
    /// The agent process reads the filesystem itself, with real file
    /// descriptors. The session's decisions are advisory for such an agent:
    /// only the OS sandbox can contain it.
    DirectProcess {
        /// Whether the OS sandbox backend confines that process's reads to the
        /// assigned scope.
        ///
        /// Callers derive this from `legion_sandbox::os_read_enforcement`;
        /// `legion-agent` may not depend on `legion-sandbox`
        /// (`plans/dependency-policy.md`), so the answer is passed in rather
        /// than assumed. No backend reports `true` today.
        os_read_enforced: bool,
    },
}

/// The lease an external agent is confined to.
#[derive(Debug, Clone)]
pub struct ExternalAgentScope {
    lease_root: PathBuf,
    delegated_scope: DelegatedTaskScope,
    denied_read_prefixes: Vec<PathBuf>,
}

impl ExternalAgentScope {
    /// Builds the scope for one external agent run.
    ///
    /// `lease_root` is the disposable worktree the agent works inside;
    /// `main_workspace_root` is the tree it must never reach. The two are
    /// checked against each other with [`validate_not_main_workspace`], so a
    /// configuration that leases the main workspace (or any ancestor of it) is
    /// refused here rather than becoming an in-scope read of everything.
    pub fn new(
        lease_root: impl Into<PathBuf>,
        main_workspace_root: impl AsRef<Path>,
        allowed_tools: Vec<LegionToolKind>,
    ) -> Result<Self, AgentError> {
        let lease_root = lease_root.into();
        validate_not_main_workspace(&lease_root, main_workspace_root.as_ref())?;

        let denied_read_prefixes: Vec<PathBuf> = DEFAULT_DENIED_READ_PREFIXES
            .iter()
            .map(PathBuf::from)
            .collect();
        let forbidden_paths = denied_read_prefixes
            .iter()
            .map(|prefix| CanonicalPath(lease_root.join(prefix).to_string_lossy().to_string()))
            .collect();

        Ok(Self {
            delegated_scope: DelegatedTaskScope {
                target_kind: DelegatedTaskScopeTargetKind::Repo,
                // The lease *is* the agent's workspace. Naming the real
                // workspace root here would make every in-lease path look
                // out-of-scope and every main-workspace path look in-scope.
                workspace_root: CanonicalPath(lease_root.to_string_lossy().to_string()),
                target_path: None,
                risk_tolerance: DelegatedTaskRiskTolerance::Conservative,
                allowed_tools,
                forbidden_paths,
                schema_version: 1,
            },
            lease_root,
            denied_read_prefixes,
        })
    }

    /// Denies reads at or beneath an additional lease-relative prefix.
    pub fn with_denied_read_prefix(mut self, prefix: impl Into<PathBuf>) -> Self {
        let prefix = prefix.into();
        self.delegated_scope.forbidden_paths.push(CanonicalPath(
            self.lease_root.join(&prefix).to_string_lossy().to_string(),
        ));
        self.denied_read_prefixes.push(prefix);
        self
    }

    /// Returns the leased worktree root.
    pub fn lease_root(&self) -> &Path {
        &self.lease_root
    }

    /// Returns the delegated-task scope derived for this lease.
    pub fn delegated_scope(&self) -> &DelegatedTaskScope {
        &self.delegated_scope
    }
}

/// Kind of filesystem request an external agent made.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExternalAgentAccessKind {
    /// A request to read file content.
    Read,
    /// A request to change a file inside the lease.
    Write,
}

/// One audited external-agent filesystem decision.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExternalAgentAccessRecord {
    /// Whether the agent asked to read or to write.
    pub kind: ExternalAgentAccessKind,
    /// The path exactly as the agent spelled it.
    pub requested_path: String,
    /// Whether the request was granted.
    pub allowed: bool,
    /// Why it was granted or refused.
    pub reason: String,
}

/// A governed external-agent run.
///
/// Every filesystem request the agent makes passes through this type, and every
/// decision — granted or refused — leaves a row in [`Self::access_log`].
#[derive(Debug, Clone)]
pub struct ExternalAgentSession {
    scope: ExternalAgentScope,
    access: ExternalAgentFilesystemAccess,
    access_log: Vec<ExternalAgentAccessRecord>,
}

impl ExternalAgentSession {
    /// Begins a governed run, or refuses it.
    ///
    /// A [`ExternalAgentFilesystemAccess::DirectProcess`] agent whose reads the
    /// OS does not confine is refused outright. That is not conservatism: this
    /// type's decisions are only consulted for requests routed through it, and
    /// a process holding real file descriptors routes nothing. Admitting it
    /// would mean an external agent that can read outside its assigned scope.
    /// Since no sandbox backend confines reads today
    /// (`legion_sandbox::os_read_enforcement`), this refuses every direct-process
    /// agent on every platform, and will stop doing so only when a backend can
    /// truthfully report otherwise.
    pub fn begin(
        scope: ExternalAgentScope,
        access: ExternalAgentFilesystemAccess,
    ) -> Result<Self, AgentError> {
        if let ExternalAgentFilesystemAccess::DirectProcess {
            os_read_enforced: false,
        } = access
        {
            return Err(AgentError::ExternalAgentLaunchRefused {
                reason: "external agent holds direct filesystem access but the OS sandbox does \
                         not confine its reads to the assigned scope"
                    .to_string(),
            });
        }
        Ok(Self {
            scope,
            access,
            access_log: Vec::new(),
        })
    }

    /// Returns the lease the agent is confined to.
    pub fn scope(&self) -> &ExternalAgentScope {
        &self.scope
    }

    /// Returns how the agent reaches the filesystem.
    pub fn filesystem_access(&self) -> &ExternalAgentFilesystemAccess {
        &self.access
    }

    /// Returns every audited filesystem decision, in order.
    pub fn access_log(&self) -> &[ExternalAgentAccessRecord] {
        &self.access_log
    }

    /// Authorizes a read and returns the lease-relative path to serve.
    ///
    /// Two independent guards run, in this order:
    ///
    /// 1. [`resolve_lease_relative_read`] resolves the request against the
    ///    lease root — not the host process's working directory — collapses
    ///    `..`, and follows symlinks on both sides before comparing. This is
    ///    the boundary guard, and it is the only guard that catches a traversal
    ///    request or an in-lease symlink aimed outside.
    /// 2. [`validate_delegated_task_tool_call`] applies the scope's tool
    ///    allowlist and forbidden-path list to the resolved location.
    ///
    /// Guard 2's own containment check (`target_is_within_scope`) is
    /// structurally satisfied by then, because guard 1 already returned a path
    /// under the lease root. It is left in place as defense in depth, but it is
    /// deliberately not the thing being relied on: it compares path components
    /// lexically, so `<lease>/../../etc/passwd` passes it, and a symlink
    /// spelled inside the lease passes it too. Guard 2's live contributions
    /// here are the tool allowlist and the forbidden-path list.
    pub fn authorize_read(&mut self, requested: &Path) -> Result<PathBuf, AgentError> {
        self.authorize(
            ExternalAgentAccessKind::Read,
            LegionToolKind::Read,
            requested,
        )
    }

    /// Authorizes a change to a file inside the lease and returns the
    /// lease-relative path.
    ///
    /// This grants nothing in the main workspace. An authorized write lands in
    /// the disposable lease; reaching the workspace requires a reviewed
    /// proposal (see [`external_edits_to_proposals`] and the admission gate in
    /// `legion_app::proposal`).
    pub fn authorize_write(&mut self, requested: &Path) -> Result<PathBuf, AgentError> {
        self.authorize(
            ExternalAgentAccessKind::Write,
            LegionToolKind::EditAsProposal,
            requested,
        )
    }

    fn authorize(
        &mut self,
        kind: ExternalAgentAccessKind,
        tool: LegionToolKind,
        requested: &Path,
    ) -> Result<PathBuf, AgentError> {
        let spelled = requested.to_string_lossy().to_string();

        let relative = match resolve_lease_relative_read(&self.scope.lease_root, requested) {
            Ok(relative) => relative,
            Err(error) => {
                return Err(self.refuse(kind, spelled, error.to_string()));
            }
        };

        let resolved = self.scope.lease_root.join(&relative);
        if let Err(error) =
            validate_delegated_task_tool_call(&self.scope.delegated_scope, tool, Some(&resolved))
        {
            return Err(self.refuse(kind, spelled, error.to_string()));
        }

        self.access_log.push(ExternalAgentAccessRecord {
            kind,
            requested_path: spelled,
            allowed: true,
            reason: "request resolves inside the assigned lease".to_string(),
        });
        Ok(relative)
    }

    fn refuse(
        &mut self,
        kind: ExternalAgentAccessKind,
        requested_path: String,
        reason: String,
    ) -> AgentError {
        self.access_log.push(ExternalAgentAccessRecord {
            kind,
            requested_path: requested_path.clone(),
            allowed: false,
            reason: reason.clone(),
        });
        AgentError::ExternalAgentAccessDenied {
            requested_path,
            reason,
        }
    }
}

// ---------------------------------------------------------------------------
// P6.F4.T3 — external edits become proposals, external logs become evidence
// ---------------------------------------------------------------------------

/// A file the external agent changed inside its lease.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExternalWorktreeEdit {
    /// Path as the agent named it, relative to the lease root.
    pub lease_relative_path: PathBuf,
    /// Full post-edit content the agent produced.
    pub content: String,
}

/// Envelope metadata shared by every proposal in one external batch.
#[derive(Debug, Clone)]
pub struct ExternalEditBatchInput {
    /// Workspace the proposals target once reviewed.
    pub workspace_id: WorkspaceId,
    /// Principal responsible for the batch.
    pub principal: PrincipalId,
    /// Capability required before mutation authority may apply any of them.
    pub capability: CapabilityId,
    /// Audit correlation identifier.
    pub correlation_id: CorrelationId,
    /// Audit causality identifier.
    pub causality_id: CausalityId,
    /// Proposal id assigned to the first edit; later edits take successive ids.
    pub first_proposal_id: ProposalId,
    /// Creation timestamp assigned by the caller.
    pub created_at: TimestampMillis,
}

/// Stable content fingerprint binding a reviewed proposal to the exact bytes
/// the external agent produced.
///
/// The admission gate re-derives this from the edit it is about to admit and
/// compares it with the hash carried by the reviewed proposal, so content
/// swapped in after review does not match what a human approved.
pub fn external_edit_content_fingerprint(content: &str) -> FileFingerprint {
    FileFingerprint {
        algorithm: "sha256".to_string(),
        value: format!("{:x}", Sha256::digest(content.as_bytes())),
    }
}

/// Renders a lease-relative path as a portable, forward-slash canonical path.
fn canonical_relative_path(relative: &Path) -> Result<String, AgentError> {
    let rendered = relative
        .components()
        .map(|component| {
            component
                .as_os_str()
                .to_str()
                .ok_or_else(|| AgentError::ExternalEditBatchRejected {
                    reason: "external edit path is not valid UTF-8".to_string(),
                })
        })
        .collect::<Result<Vec<_>, _>>()?
        .join("/");
    if rendered.is_empty() {
        return Err(AgentError::ExternalEditBatchRejected {
            reason: "external edit path is empty".to_string(),
        });
    }
    Ok(rendered)
}

/// Converts every external edit into a Legion proposal — or none of them.
///
/// This is the only route out of a lease. It is all-or-nothing on purpose: a
/// partial result would be a batch in which some of the agent's edits carry a
/// reviewable proposal and the rest are simply unaccounted for, which is
/// exactly the state the acceptance criterion forbids. Any rejection aborts the
/// whole conversion and returns no proposals at all.
///
/// Each edit is re-authorized through the session first, so a path the agent
/// smuggled into the edit list without ever asking to write is refused here
/// too, and the refusal is audited.
///
/// Two edits naming the same path are rejected: they would produce two
/// proposals for one file, and whichever applied second would silently discard
/// the reviewed content of the first.
pub fn external_edits_to_proposals(
    session: &mut ExternalAgentSession,
    edits: &[ExternalWorktreeEdit],
    input: &ExternalEditBatchInput,
) -> Result<Vec<WorkspaceProposal>, AgentError> {
    let mut seen: Vec<String> = Vec::with_capacity(edits.len());
    let mut proposals = Vec::with_capacity(edits.len());

    for (index, edit) in edits.iter().enumerate() {
        let relative = session.authorize_write(&edit.lease_relative_path)?;
        let path = canonical_relative_path(&relative)?;
        if seen.contains(&path) {
            return Err(AgentError::ExternalEditBatchRejected {
                reason: format!("external edit batch names {path} more than once"),
            });
        }
        seen.push(path.clone());

        let proposal_id = ProposalId(input.first_proposal_id.0.saturating_add(index as u64));
        let payload = WorkspaceEditProposalPayload {
            workspace_id: input.workspace_id,
            edit_id: Uuid::now_v7(),
            title: format!("External agent edit: {path}"),
            source: WorkspaceEditSourceKind::AiAssisted,
            target_coverage: ProposalTargetCoverage {
                coverage_kind: ProposalTargetCoverageKind::Complete,
                targets: vec![ProposalAffectedTarget {
                    target_id: format!("external-edit:{path}"),
                    kind: ProposalTargetKind::PathOnly,
                    workspace_id: Some(input.workspace_id),
                    file_id: None,
                    buffer_id: None,
                    path: Some(CanonicalPath(path.clone())),
                    terminal_session_id: None,
                    plugin_id: None,
                    remote_authority: None,
                    collaboration_session_id: None,
                    byte_ranges: vec![],
                    redaction_hints: vec![RedactionHint::MetadataOnly],
                }],
                omitted_target_count: 0,
                redaction_hints: vec![RedactionHint::MetadataOnly],
            },
            file_edits: vec![],
            file_operations: vec![WorkspaceFileOperation::Create {
                path: CanonicalPath(path.clone()),
                initial_content_hash: Some(external_edit_content_fingerprint(&edit.content)),
            }],
            required_capability: input.capability.clone(),
            diagnostics: vec![],
            schema_version: 1,
        };

        proposals.push(external_workspace_edit_proposal(
            ExternalWorkspaceEditProposalInput {
                proposal_id,
                principal: input.principal.clone(),
                capability: input.capability.clone(),
                correlation_id: input.correlation_id,
                causality_id: input.causality_id,
                payload,
                preconditions: ProposalVersionPreconditions {
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
                expires_at: None,
                created_at: input.created_at,
            },
        )?);
    }

    Ok(proposals)
}

#[cfg(test)]
mod tests {
    use super::*;
    use legion_protocol::{
        CanonicalPath, ProposalAffectedTarget, ProposalTargetCoverage, ProposalTargetKind,
        RedactionHint, WorkspaceEditSourceKind, WorkspaceFileOperation,
    };

    fn proposal_target(path: &str) -> ProposalAffectedTarget {
        ProposalAffectedTarget {
            target_id: format!("target:{path}"),
            kind: ProposalTargetKind::PathOnly,
            workspace_id: None,
            file_id: None,
            buffer_id: None,
            path: Some(CanonicalPath(path.to_string())),
            terminal_session_id: None,
            plugin_id: None,
            remote_authority: None,
            collaboration_session_id: None,
            byte_ranges: vec![],
            redaction_hints: vec![RedactionHint::MetadataOnly],
        }
    }

    #[test]
    fn external_workspace_edit_proposal_builds_preview_without_mutation() {
        let input = ExternalWorkspaceEditProposalInput {
            proposal_id: ProposalId(42),
            principal: PrincipalId("principal:external".to_string()),
            capability: CapabilityId("fs.write".to_string()),
            correlation_id: CorrelationId(17),
            causality_id: CausalityId(Uuid::now_v7()),
            payload: WorkspaceEditProposalPayload {
                workspace_id: legion_protocol::WorkspaceId(9),
                edit_id: Uuid::now_v7(),
                title: "Apply external workspace change".to_string(),
                source: WorkspaceEditSourceKind::User,
                target_coverage: ProposalTargetCoverage {
                    coverage_kind: ProposalTargetCoverageKind::Complete,
                    targets: vec![proposal_target("src/external.rs")],
                    omitted_target_count: 0,
                    redaction_hints: vec![RedactionHint::MetadataOnly],
                },
                file_edits: vec![],
                file_operations: vec![WorkspaceFileOperation::Create {
                    path: CanonicalPath("src/external.rs".to_string()),
                    initial_content_hash: None,
                }],
                required_capability: CapabilityId("fs.write".to_string()),
                diagnostics: vec![],
                schema_version: 1,
            },
            preconditions: ProposalVersionPreconditions {
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
            expires_at: None,
            created_at: TimestampMillis(99),
        };

        let proposal = external_workspace_edit_proposal(input).expect("proposal envelope");

        assert_eq!(proposal.proposal_id, ProposalId(42));
        assert_eq!(proposal.correlation_id, CorrelationId(17));
        assert_eq!(proposal.preview.summary, "Apply external workspace change");
        assert_eq!(proposal.preview.details[0], "source=User");
        assert!(matches!(
            proposal.payload,
            ProposalPayload::WorkspaceEdit(_)
        ));
    }

    /// Re-exported rather than re-implemented: four crates grew their own
    /// copy of this generator, each with a different seed, so none was
    /// authoritative. See `legion_security::synthetic_credentials`.
    use legion_security::synthetic_credentials::synthetic_access_key_id;

    #[test]
    fn external_workspace_edit_proposal_flags_credentials_in_preview_details() {
        let input = ExternalWorkspaceEditProposalInput {
            proposal_id: ProposalId(44),
            principal: PrincipalId("principal:external".to_string()),
            capability: CapabilityId("fs.write".to_string()),
            correlation_id: CorrelationId(18),
            causality_id: CausalityId(Uuid::now_v7()),
            payload: WorkspaceEditProposalPayload {
                workspace_id: legion_protocol::WorkspaceId(9),
                edit_id: Uuid::now_v7(),
                title: format!("Rotate {} in deploy config", synthetic_access_key_id()),
                source: WorkspaceEditSourceKind::AiAssisted,
                target_coverage: ProposalTargetCoverage {
                    coverage_kind: ProposalTargetCoverageKind::Complete,
                    targets: vec![proposal_target("deploy/config.toml")],
                    omitted_target_count: 0,
                    redaction_hints: vec![RedactionHint::MetadataOnly],
                },
                file_edits: vec![],
                file_operations: vec![WorkspaceFileOperation::Create {
                    path: CanonicalPath("deploy/config.toml".to_string()),
                    initial_content_hash: None,
                }],
                required_capability: CapabilityId("fs.write".to_string()),
                diagnostics: vec![],
                schema_version: 1,
            },
            preconditions: ProposalVersionPreconditions {
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
            expires_at: None,
            created_at: TimestampMillis(99),
        };

        let proposal = external_workspace_edit_proposal(input).expect("proposal envelope");

        assert!(
            proposal
                .preview
                .details
                .iter()
                .any(|detail| detail.starts_with("credential_scan_sites=")),
            "a credential in externally authored proposal content must surface to the reviewer"
        );
    }

    #[test]
    fn external_workspace_edit_proposal_leaves_clean_previews_unannotated() {
        let input = ExternalWorkspaceEditProposalInput {
            proposal_id: ProposalId(45),
            principal: PrincipalId("principal:external".to_string()),
            capability: CapabilityId("fs.write".to_string()),
            correlation_id: CorrelationId(19),
            causality_id: CausalityId(Uuid::now_v7()),
            payload: WorkspaceEditProposalPayload {
                workspace_id: legion_protocol::WorkspaceId(9),
                edit_id: Uuid::now_v7(),
                title: "Apply external workspace change".to_string(),
                source: WorkspaceEditSourceKind::User,
                target_coverage: ProposalTargetCoverage {
                    coverage_kind: ProposalTargetCoverageKind::Complete,
                    targets: vec![proposal_target("src/external.rs")],
                    omitted_target_count: 0,
                    redaction_hints: vec![RedactionHint::MetadataOnly],
                },
                file_edits: vec![],
                file_operations: vec![WorkspaceFileOperation::Create {
                    path: CanonicalPath("src/external.rs".to_string()),
                    initial_content_hash: None,
                }],
                required_capability: CapabilityId("fs.write".to_string()),
                diagnostics: vec![],
                schema_version: 1,
            },
            preconditions: ProposalVersionPreconditions {
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
            expires_at: None,
            created_at: TimestampMillis(99),
        };

        let proposal = external_workspace_edit_proposal(input).expect("proposal envelope");

        assert!(
            !proposal
                .preview
                .details
                .iter()
                .any(|detail| detail.starts_with("credential_scan_sites=")),
            "clean proposal previews must not carry a scan annotation"
        );
    }

    #[test]
    fn external_workspace_edit_proposal_rejects_missing_payload() {
        let input = ExternalWorkspaceEditProposalInput {
            proposal_id: ProposalId(43),
            principal: PrincipalId("principal:external".to_string()),
            capability: CapabilityId("fs.write".to_string()),
            correlation_id: CorrelationId(17),
            causality_id: CausalityId(Uuid::now_v7()),
            payload: WorkspaceEditProposalPayload {
                workspace_id: legion_protocol::WorkspaceId(9),
                edit_id: Uuid::now_v7(),
                title: "Empty proposal".to_string(),
                source: WorkspaceEditSourceKind::User,
                target_coverage: ProposalTargetCoverage {
                    coverage_kind: ProposalTargetCoverageKind::Complete,
                    targets: vec![proposal_target("src/external.rs")],
                    omitted_target_count: 0,
                    redaction_hints: vec![RedactionHint::MetadataOnly],
                },
                file_edits: vec![],
                file_operations: vec![],
                required_capability: CapabilityId("fs.write".to_string()),
                diagnostics: vec![],
                schema_version: 1,
            },
            preconditions: ProposalVersionPreconditions {
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
            expires_at: None,
            created_at: TimestampMillis(99),
        };

        let error = external_workspace_edit_proposal(input).expect_err("invalid proposal");
        assert!(
            error
                .to_string()
                .contains("requires at least one file edit or file operation")
        );
    }
}
