//! Phase 4 trust projections: what a reviewer reads about a run.
//!
//! Extracted from `lib.rs` because the chokepoint gate is right about it: this
//! is the region where a projection quietly disagreeing with the run it
//! describes has cost the most, and it is easier to see the four answers agree
//! when they are on one screen.

use super::*;

pub(crate) struct Phase4ContextAssemblyService;

impl Phase4ContextAssemblyService {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn assemble_context_manifest(
        context: &ActiveSaveContext,
        run_id: &legion_protocol::AgentRunId,
        provider_route_id: &str,
        snapshot_id: legion_protocol::SnapshotId,
        buffer_version: legion_protocol::BufferVersion,
        snapshot_hash: FileFingerprint,
        byte_len: u64,
        line_count: u32,
        generated_at: TimestampMillis,
        instruction_manifest_items: Vec<legion_protocol::ContextManifestItem>,
        // Whether the route this manifest describes carries the excerpt off the
        // machine. The manifest was written for the loopback fixture and said
        // so unconditionally, so a reviewer looking at an Anthropic proposal
        // read an accurate remote capability beside a manifest promising the
        // buffer never left.
        sends_the_buffer: bool,
    ) -> legion_protocol::ContextManifestProjection {
        let route_egress = if sends_the_buffer {
            legion_protocol::ContextManifestEgressStatus::ExternalEgressMetadata
        } else {
            legion_protocol::ContextManifestEgressStatus::LocalProvider
        };
        let route_privacy_scope = if sends_the_buffer {
            // File-scoped: the excerpt itself is what goes, and calling that
            // metadata is the claim this fix exists to remove.
            legion_protocol::SemanticPrivacyScope::File
        } else {
            legion_protocol::SemanticPrivacyScope::MetadataOnly
        };
        let route_risk = if sends_the_buffer {
            legion_protocol::ProposalRiskLabel::Medium
        } else {
            legion_protocol::ProposalRiskLabel::Low
        };
        let file_item = legion_protocol::ContextManifestItem {
            item_id: format!("phase4:{}:file", run_id.0),
            kind: legion_protocol::ContextManifestItemKind::File,
            inclusion: legion_protocol::ContextManifestInclusionState::Included,
            workspace_id: Some(context.workspace_id),
            file_id: Some(context.metadata.identity.file_id),
            buffer_id: Some(context.buffer_id),
            proposal_id: None,
            target_id: Some(context.metadata.identity.file_id.0.to_string()),
            path: Some(context.metadata.identity.canonical_path.clone()),
            ranges: Vec::new(),
            counts: context
                .metadata
                .file_length
                .map(|count| legion_protocol::ContextManifestItemCount {
                    label: "file_bytes".to_string(),
                    count: count.min(u32::MAX as u64) as u32,
                })
                .into_iter()
                .collect(),
            hashes: vec![context.metadata.fingerprint.clone()],
            privacy_scope: Some(legion_protocol::SemanticPrivacyScope::MetadataOnly),
            privacy_label: legion_protocol::ProposalPrivacyLabel::WorkspaceMetadata,
            risk_label: legion_protocol::ProposalRiskLabel::Low,
            egress: legion_protocol::ContextManifestEgressStatus::LocalOnly,
            freshness: Some(legion_protocol::ContextManifestFreshnessSummary {
                state: legion_protocol::SemanticFreshnessState::Fresh,
                freshness_key_present: true,
                snapshot_id: Some(snapshot_id),
                file_content_version: Some(context.metadata.file_content_version),
                workspace_generation: Some(context.metadata.workspace_generation),
                content_hash: Some(context.metadata.fingerprint.clone()),
                privacy_scope: Some(legion_protocol::SemanticPrivacyScope::MetadataOnly),
                observed_at: Some(generated_at),
                risk_label: legion_protocol::ProposalRiskLabel::Low,
                risk_reasons: Vec::new(),
                schema_version: 1,
            }),
            preconditions: None,
            labels: vec!["phase4.context.file_metadata".to_string()],
            redaction_hints: vec![legion_protocol::RedactionHint::MetadataOnly],
            schema_version: 1,
        };
        let buffer_item = legion_protocol::ContextManifestItem {
            item_id: format!("phase4:{}:buffer", run_id.0),
            kind: legion_protocol::ContextManifestItemKind::Buffer,
            inclusion: legion_protocol::ContextManifestInclusionState::Included,
            workspace_id: Some(context.workspace_id),
            file_id: Some(context.metadata.identity.file_id),
            buffer_id: Some(context.buffer_id),
            proposal_id: None,
            target_id: Some(context.buffer_id.0.to_string()),
            path: None,
            ranges: Vec::new(),
            counts: vec![
                legion_protocol::ContextManifestItemCount {
                    label: "snapshot_bytes".to_string(),
                    count: byte_len.min(u32::MAX as u64) as u32,
                },
                legion_protocol::ContextManifestItemCount {
                    label: "lines".to_string(),
                    count: line_count,
                },
            ],
            hashes: vec![snapshot_hash],
            privacy_scope: Some(legion_protocol::SemanticPrivacyScope::MetadataOnly),
            privacy_label: legion_protocol::ProposalPrivacyLabel::WorkspaceMetadata,
            risk_label: legion_protocol::ProposalRiskLabel::Low,
            egress: legion_protocol::ContextManifestEgressStatus::LocalOnly,
            freshness: Some(legion_protocol::ContextManifestFreshnessSummary {
                state: legion_protocol::SemanticFreshnessState::Fresh,
                freshness_key_present: true,
                snapshot_id: Some(snapshot_id),
                file_content_version: Some(context.metadata.file_content_version),
                workspace_generation: Some(context.metadata.workspace_generation),
                content_hash: None,
                privacy_scope: Some(legion_protocol::SemanticPrivacyScope::MetadataOnly),
                observed_at: Some(generated_at),
                risk_label: legion_protocol::ProposalRiskLabel::Low,
                risk_reasons: Vec::new(),
                schema_version: 1,
            }),
            preconditions: Some(legion_protocol::ContextManifestPreconditionSummary {
                file_content_version: Some(context.metadata.file_content_version),
                buffer_version: Some(buffer_version),
                snapshot_id: Some(snapshot_id),
                workspace_generation: Some(context.metadata.workspace_generation),
                expected_fingerprint: Some(context.metadata.fingerprint.clone()),
                expected_file_length: context.metadata.file_length,
                expected_modified_at: context.metadata.modified_at,
                core_preconditions_present: true,
                risk_label: legion_protocol::ProposalRiskLabel::Low,
                risk_reasons: Vec::new(),
                schema_version: 1,
            }),
            labels: vec!["phase4.context.buffer_descriptor".to_string()],
            redaction_hints: vec![legion_protocol::RedactionHint::MetadataOnly],
            schema_version: 1,
        };
        let route_item = Self::metadata_item(
            format!("phase4:{}:provider-route", run_id.0),
            legion_protocol::ContextManifestItemKind::ProviderRoute,
            context.workspace_id,
            provider_route_id,
            route_egress,
            vec![if sends_the_buffer {
                "phase4.provider.remote_egress".to_string()
            } else {
                "phase4.provider.local_loopback".to_string()
            }],
        );
        let agent_item = Self::metadata_item(
            format!("phase4:{}:agent-step", run_id.0),
            legion_protocol::ContextManifestItemKind::AgentStep,
            context.workspace_id,
            &run_id.0,
            legion_protocol::ContextManifestEgressStatus::LocalOnly,
            vec!["phase4.agent.proposal_only".to_string()],
        );
        let selection_item = Self::metadata_item(
            format!("phase4:{}:selection", run_id.0),
            legion_protocol::ContextManifestItemKind::UserSelection,
            context.workspace_id,
            "active-buffer",
            legion_protocol::ContextManifestEgressStatus::LocalOnly,
            vec!["phase4.selection.active_buffer".to_string()],
        );

        let permission = legion_protocol::ContextManifestPermissionSummary {
            kind: legion_protocol::ContextManifestPermissionKind::ModelProvider,
            capability: CapabilityId("ai.provider.invoke".to_string()),
            principal: Some(context.principal.clone()),
            decision_id: None,
            granted: false,
            privacy_scope: route_privacy_scope,
            egress: route_egress,
            risk_label: route_risk,
            redaction_hints: vec![legion_protocol::RedactionHint::MetadataOnly],
            schema_version: 1,
        };
        let manifest = legion_protocol::ContextManifestRecord {
            manifest_id: format!("phase4:manifest:{}", run_id.0),
            workspace_id: Some(context.workspace_id),
            proposal_id: None,
            purpose: legion_protocol::ContextManifestPurpose::ProviderRequest,
            workspace_trust_state: Some(context.trust.clone()),
            privacy_label: if sends_the_buffer {
                legion_protocol::ProposalPrivacyLabel::ExternalEgressMetadata
            } else {
                legion_protocol::ProposalPrivacyLabel::WorkspaceMetadata
            },
            risk_label: route_risk,
            egress: route_egress,
            items: vec![file_item, buffer_item]
                .into_iter()
                .chain(instruction_manifest_items)
                .chain(vec![selection_item, route_item, agent_item])
                .collect(),
            permissions: vec![permission],
            omitted_item_count: 0,
            stale_or_missing_metadata_risk_present: false,
            generated_at,
            redaction_hints: vec![legion_protocol::RedactionHint::MetadataOnly],
            schema_version: 1,
        };
        legion_protocol::ContextManifestProjection {
            manifest,
            selected_item_id: None,
            generated_at,
            redaction_hints: vec![legion_protocol::RedactionHint::MetadataOnly],
            schema_version: 1,
        }
    }

    fn metadata_item(
        item_id: String,
        kind: legion_protocol::ContextManifestItemKind,
        workspace_id: WorkspaceId,
        target_id: &str,
        egress: legion_protocol::ContextManifestEgressStatus,
        labels: Vec<String>,
    ) -> legion_protocol::ContextManifestItem {
        legion_protocol::ContextManifestItem {
            item_id,
            kind,
            inclusion: legion_protocol::ContextManifestInclusionState::Included,
            workspace_id: Some(workspace_id),
            file_id: None,
            buffer_id: None,
            proposal_id: None,
            target_id: Some(target_id.to_string()),
            path: None,
            ranges: Vec::new(),
            counts: Vec::new(),
            hashes: Vec::new(),
            privacy_scope: Some(legion_protocol::SemanticPrivacyScope::MetadataOnly),
            privacy_label: legion_protocol::ProposalPrivacyLabel::WorkspaceMetadata,
            risk_label: legion_protocol::ProposalRiskLabel::Low,
            egress,
            freshness: None,
            preconditions: None,
            labels,
            redaction_hints: vec![legion_protocol::RedactionHint::MetadataOnly],
            schema_version: 1,
        }
    }
}

/// Whether a route of this class carries the buffer excerpt off the machine.
///
/// One predicate, because it is asked by the capability, by the context
/// manifest, by the privacy inspector and by the permission budget -- and four
/// copies is four chances for a projection to disagree with the run it
/// describes, which is the defect this whole area keeps producing.
///
/// `Unknown` counts as remote deliberately. A class nothing recognises is
/// exactly where guessing "local and free" is the expensive way to be wrong.
pub(crate) fn provider_class_sends_the_buffer(
    provider_class: legion_protocol::AssistedAiProviderClass,
) -> bool {
    matches!(
        provider_class,
        legion_protocol::AssistedAiProviderClass::ByokRemote
            | legion_protocol::AssistedAiProviderClass::HostedRemote
            | legion_protocol::AssistedAiProviderClass::Gateway
            | legion_protocol::AssistedAiProviderClass::Unknown
    )
}

/// The provider capability a proposal is reviewed against.
///
/// Built from the backend that was actually resolved, not from the shape of the
/// deterministic one. Hard-coding `deterministic-local`, `local.free` and
/// `metadata-only` described an Anthropic run that had uploaded workspace text
/// as a free, offline, air-gap-safe local run -- and this projection is what a
/// reviewer reads to decide whether to accept an edit, so it was the one place
/// the truth mattered most and the one place it was invented.
pub(crate) fn phase4_provider_capability(
    provider_class: legion_protocol::AssistedAiProviderClass,
    routed_provider_id: &str,
    refusal: Option<legion_protocol::AssistedAiRefusalMetadata>,
) -> legion_protocol::AssistedAiProviderCapability {
    // Egress is a property of the provider *class*, which is typed, and not of
    // its name, which is a string that happens to correlate today.
    //
    // Deriving `remote` from `routed_provider_id == "anthropic"` was the same
    // drift this function exists to stop, one column over: a second BYOK route
    // or any `HostedRemote` would be described to the reviewer as free,
    // air-gap-safe and metadata-only while it shipped their buffer excerpt over
    // the wire. `product_ai_route_fields` already decides the class; asking the
    // name again is a second place for the same fact to live.
    //
    // `Unknown` is treated as remote deliberately. A class nothing recognises is
    // exactly the case where guessing "local and free" is the expensive way to
    // be wrong.
    let remote = provider_class_sends_the_buffer(provider_class);
    let live = routed_provider_id != DETERMINISTIC_LOCAL_PROVIDER_ID;
    let provider_id = routed_provider_id.to_string();
    // The label names the class, so a provider nobody has taught this function
    // about is still described by what it is rather than mislabelled.
    let provider_label = match provider_class {
        legion_protocol::AssistedAiProviderClass::Local => {
            format!("{routed_provider_id} (local)")
        }
        legion_protocol::AssistedAiProviderClass::LocalLoopback => {
            format!("{routed_provider_id} (local loopback)")
        }
        legion_protocol::AssistedAiProviderClass::ByokRemote => {
            format!("{routed_provider_id} (BYOK remote)")
        }
        legion_protocol::AssistedAiProviderClass::HostedRemote => {
            format!("{routed_provider_id} (hosted remote)")
        }
        legion_protocol::AssistedAiProviderClass::Gateway => {
            format!("{routed_provider_id} (gateway)")
        }
        legion_protocol::AssistedAiProviderClass::Unknown => {
            format!("{routed_provider_id} (unrecognised provider class)")
        }
    };
    let supported = legion_protocol::AssistedAiSupportLabel::Supported;
    let unsupported = legion_protocol::AssistedAiSupportLabel::Unsupported;
    legion_protocol::AssistedAiProviderCapability {
        provider_id,
        provider_label,
        provider_class,
        supported_operations: vec![
            legion_protocol::AssistedAiOperationClass::Explain,
            legion_protocol::AssistedAiOperationClass::ProposeEdit,
        ],
        model_capability_labels: vec![if live {
            "live".to_string()
        } else {
            "deterministic".to_string()
        }],
        tool_capability_labels: Vec::new(),
        context_window_label: "small".to_string(),
        // A remote call is not free and a reviewer should not be told it is.
        cost_budget_label: if remote {
            "remote.metered".to_string()
        } else {
            "local.free".to_string()
        },
        risk_budget_label: if remote {
            "elevated".to_string()
        } else {
            "low".to_string()
        },
        // Metadata-only is a promise about what leaves the machine. It is true
        // of the deterministic and loopback routes and false of a remote one,
        // which sends the excerpt itself.
        privacy_retention_label: if remote {
            "provider-retained".to_string()
        } else {
            "metadata-only".to_string()
        },
        byok_support: if remote { supported } else { unsupported },
        local_execution_support: if remote { unsupported } else { supported },
        offline_support: if remote { unsupported } else { supported },
        air_gap_support: if remote { unsupported } else { supported },
        redaction_requirements: vec![if remote {
            "prompt-and-metadata".to_string()
        } else {
            "metadata-only".to_string()
        }],
        consent_requirements: vec!["proposal-review".to_string()],
        availability: if refusal.is_some() {
            legion_protocol::AssistedAiProviderAvailabilityState::Refused
        } else {
            legion_protocol::AssistedAiProviderAvailabilityState::Available
        },
        refusal,
        redaction_hints: vec![RedactionHint::MetadataOnly],
        schema_version: 1,
    }
}

pub(crate) fn phase4_permission_budget_projection(
    context_manifest: &legion_protocol::ContextManifestProjection,
    run_id: &legion_protocol::AgentRunId,
    generated_at: TimestampMillis,
    sends_the_buffer: bool,
) -> legion_protocol::PermissionBudgetProjection {
    let budget = legion_protocol::PermissionBudgetContract {
        budget_id: format!("phase4:budget:{}", run_id.0),
        action_class: legion_protocol::PermissionBudgetActionClass::InvokeProvider,
        capability: Some(CapabilityId("ai.provider.invoke".to_string())),
        state: legion_protocol::PermissionBudgetState::Allowed,
        privacy_scope: if sends_the_buffer {
            legion_protocol::SemanticPrivacyScope::File
        } else {
            legion_protocol::SemanticPrivacyScope::MetadataOnly
        },
        usage: legion_protocol::PermissionBudgetUsageSummary {
            unit_label: "calls".to_string(),
            used: 0,
            ceiling: Some(1),
            remaining: Some(1),
            attempted: 0,
            redaction_hints: vec![RedactionHint::MetadataOnly],
            schema_version: 1,
        },
        reset_policy_label: legion_protocol::PermissionBudgetResetPolicyLabel::Session,
        // Consent is "not required" only because a metadata-only local call
        // asks nothing of anybody. Sending the excerpt to a remote provider
        // does, and the proposal review is where that consent is given.
        consent_requirement_label: if sends_the_buffer {
            legion_protocol::PermissionBudgetConsentRequirementLabel::Required
        } else {
            legion_protocol::PermissionBudgetConsentRequirementLabel::NotRequired
        },
        risk_label: if sends_the_buffer {
            legion_protocol::ProposalRiskLabel::Medium
        } else {
            legion_protocol::ProposalRiskLabel::Low
        },
        reasons: vec![if sends_the_buffer {
            "phase4.remote_provider.budget_allowed".to_string()
        } else {
            "phase4.local_provider.budget_allowed".to_string()
        }],
        redaction_hints: vec![RedactionHint::MetadataOnly],
        schema_version: 1,
    };
    let action = legion_protocol::permission_budget_action_from_permission_summary(
        &context_manifest.manifest.permissions[0],
        format!("phase4:budget-action:{}", run_id.0),
        legion_protocol::PermissionBudgetActionClass::InvokeProvider,
        context_manifest.manifest.workspace_id,
        context_manifest.manifest.proposal_id,
        1,
    );
    let evaluation = legion_protocol::evaluate_permission_budget(
        &budget,
        action,
        format!("phase4:budget-eval:{}", run_id.0),
        1,
    );
    legion_protocol::permission_budget_projection_from_contracts(
        format!("phase4:permission-budget:{}", run_id.0),
        vec![budget],
        vec![evaluation],
        generated_at,
        1,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The manifest and the budget describe the route that will run.
    ///
    /// Both were built before the backend was resolved and hard-coded the
    /// loopback fixture's answers: local egress, metadata-only scope, low risk,
    /// no consent required, `phase4.local_provider.budget_allowed`. The
    /// capability beside them was corrected first, so a reviewer of an
    /// Anthropic proposal saw an accurate remote capability next to three
    /// projections promising the buffer never left the machine.
    #[test]
    fn the_trust_projections_follow_the_route_that_will_run() {
        let root = std::env::temp_dir().join(format!(
            "legion-phase4-trust-egress-{}",
            uuid::Uuid::now_v7()
        ));
        std::fs::create_dir_all(&root).expect("create temp root");
        std::fs::write(
            root.join("lib.rs"),
            "pub fn marker() -> u32 {
    42
}
",
        )
        .expect("fixture file should be written");
        let mut app = AppComposition::new();
        app.open_workspace(
            &root,
            WorkspaceTrustState::Trusted,
            PrincipalId("trust-egress".to_string()),
        )
        .expect("workspace should open");
        app.open_file("lib.rs").expect("fixture file should open");

        let context = app
            .active_documents
            .require_active_save_context()
            .expect("an open file has a save context");
        let run_id = legion_protocol::AgentRunId("phase4-run-trust".to_string());
        let snapshot = app
            .editor
            .current_snapshot(context.buffer_id)
            .expect("the open buffer has a snapshot")
            .clone();

        let manifest_for = |sends_the_buffer: bool| {
            Phase4ContextAssemblyService::assemble_context_manifest(
                &context,
                &run_id,
                "phase4-route-trust",
                snapshot.snapshot_id,
                snapshot.buffer_version,
                FileFingerprint {
                    algorithm: "legion-text-snapshot".to_string(),
                    value: snapshot.content_hash.clone(),
                },
                snapshot.byte_len as u64,
                snapshot.line_count.min(u32::MAX as usize) as u32,
                TimestampMillis::now(),
                Vec::new(),
                sends_the_buffer,
            )
        };

        let local = manifest_for(false);
        let remote = manifest_for(true);

        assert_eq!(
            local.manifest.egress,
            legion_protocol::ContextManifestEgressStatus::LocalProvider,
            "a loopback route really is local and must keep saying so"
        );
        let local_permission = &local.manifest.permissions[0];
        assert_eq!(
            local_permission.privacy_scope,
            legion_protocol::SemanticPrivacyScope::MetadataOnly,
            "a loopback route really is metadata-only and must keep saying so"
        );
        // The exact value, not merely "not the local one". Excluding a single
        // wrong answer leaves every other wrong answer passing -- a regression
        // to `LocalOnly` would have satisfied all three of these as `assert_ne!`
        // and reported a remote run as never having touched the network at all.
        assert_eq!(
            remote.manifest.egress,
            legion_protocol::ContextManifestEgressStatus::ExternalEgressMetadata,
            "a route that uploads the excerpt must say external egress happened"
        );
        let remote_permission = &remote.manifest.permissions[0];
        assert_eq!(
            remote_permission.privacy_scope,
            legion_protocol::SemanticPrivacyScope::File,
            "the excerpt itself is file-scoped, and calling it metadata-only is the claim this fix removes"
        );
        assert_eq!(
            remote_permission.egress,
            legion_protocol::ContextManifestEgressStatus::ExternalEgressMetadata,
            "the model-provider permission must report the egress the route performs"
        );
        assert_eq!(
            remote_permission.risk_label,
            legion_protocol::ProposalRiskLabel::Medium,
            "a run that uploads the buffer is not the same risk as one that does not"
        );

        // The inspector is derived from the manifest, so correcting the
        // manifest is what corrects it -- assert that rather than assume it.
        let remote_inspector = legion_protocol::privacy_inspector_from_context_manifest_projection(
            &remote,
            "phase4:privacy:trust".to_string(),
            TimestampMillis::now(),
            1,
        );
        let local_inspector = legion_protocol::privacy_inspector_from_context_manifest_projection(
            &local,
            "phase4:privacy:trust".to_string(),
            TimestampMillis::now(),
            1,
        );
        assert_ne!(
            format!("{remote_inspector:?}"),
            format!("{local_inspector:?}"),
            "the privacy inspector reads the same for a remote route as for a local one"
        );

        let remote_budget =
            phase4_permission_budget_projection(&remote, &run_id, TimestampMillis::now(), true);
        assert_eq!(
            remote_budget.budgets[0].consent_requirement_label,
            legion_protocol::PermissionBudgetConsentRequirementLabel::Required,
            "uploading the excerpt to a metered provider needs consent, and the budget said none was required"
        );
        assert!(
            remote_budget.budgets[0]
                .reasons
                .iter()
                .all(|reason| reason != "phase4.local_provider.budget_allowed"),
            "the remote budget is justified by a local-provider reason: {:?}",
            remote_budget.budgets[0].reasons
        );

        let local_budget =
            phase4_permission_budget_projection(&local, &run_id, TimestampMillis::now(), false);
        assert_eq!(
            local_budget.budgets[0].consent_requirement_label,
            legion_protocol::PermissionBudgetConsentRequirementLabel::NotRequired,
            "a metadata-only local call asks nothing of anybody and must not start demanding consent"
        );
    }
}
