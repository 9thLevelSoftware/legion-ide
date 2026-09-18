//! The live LSP read path: issue a request, drain the worker, ingest the result.
//!
//! Extracted verbatim from `lib.rs` (roadmap 1.6). Nothing here changed in the
//! move. The split exists because P2.F1.T4 adds four more read features to this
//! cluster — references, document symbols, inlay hints, code lenses — and those
//! belong in a reviewable diff rather than buried in a 38,000-line file.
//!
//! Every method is an inherent `AppComposition` method, so callers are
//! unaffected by the module existing.

use crate::*;
use uuid::Uuid;

#[cfg(test)]
#[path = "diagnostic_lifecycle_tests.rs"]
mod diagnostic_lifecycle_tests;
#[cfg(test)]
#[path = "document_sync_tests.rs"]
mod document_sync_tests;
#[cfg(test)]
#[path = "write_operation_tests.rs"]
mod write_operation_tests;

/// Metadata-only rename retained until its target document has entered the
/// bounded worker queue. No source text is held here.
#[derive(Clone)]
pub(crate) struct DeferredLspWrite {
    pub(crate) buffer_id: BufferId,
    pub(crate) snapshot_id: SnapshotId,
    pub(crate) uri: String,
    pub(crate) method: String,
    pub(crate) kind: crate::language::LspReadKind,
    pub(crate) params: serde_json::Value,
    pub(crate) operation_context: legion_protocol::LspOperationContext,
}

fn same_lsp_operation_context(
    expected: &legion_protocol::LspOperationContext,
    actual: &legion_protocol::LspOperationContext,
) -> bool {
    expected.request_id == actual.request_id
        && expected.workspace_id == actual.workspace_id
        && expected.file_id == actual.file_id
        && expected.buffer_id == actual.buffer_id
        && expected.snapshot_id == actual.snapshot_id
        && expected.buffer_version == actual.buffer_version
        && expected.language_id == actual.language_id
        && expected.correlation_id == actual.correlation_id
        && expected.causality_id == actual.causality_id
        && expected.timeout_ms == actual.timeout_ms
        && expected.cancellation_token == actual.cancellation_token
        && expected.content_hash == actual.content_hash
        && expected.privacy_scope == actual.privacy_scope
        && expected.schema_version == actual.schema_version
}

fn remove_matching_command_context(
    contexts: &mut std::collections::HashMap<String, crate::language::PendingLspCommandContext>,
    context: &legion_protocol::LspOperationContext,
) {
    let key = context.request_id.0.to_string();
    if contexts
        .get(&key)
        .is_some_and(|pending| same_lsp_operation_context(&pending.context, context))
    {
        contexts.remove(&key);
    }
}

impl AppComposition {
    pub(crate) fn lsp_operation_context(
        &mut self,
        buffer_id: BufferId,
        snapshot_id: SnapshotId,
        event_context: EventContext,
    ) -> Option<legion_protocol::LspOperationContext> {
        let workspace_id = self.active_documents.workspace_id()?;
        let metadata = self.active_documents.metadata_for_buffer(buffer_id)?;
        let buffer_version = self.editor.buffer_version(buffer_id).ok()?;
        Some(legion_protocol::LspOperationContext {
            request_id: legion_protocol::LspRequestId(Uuid::now_v7()),
            workspace_id,
            file_id: metadata.identity.file_id,
            buffer_id,
            snapshot_id,
            buffer_version,
            language_id: language_id_for_path(&metadata.identity.canonical_path),
            correlation_id: event_context.correlation_id,
            causality_id: event_context.causality_id,
            timeout_ms: 5_000,
            cancellation_token: legion_protocol::CancellationTokenId(Uuid::now_v7()),
            content_hash: None,
            privacy_scope: legion_protocol::SemanticPrivacyScope::Workspace,
            schema_version: 1,
        })
    }

    pub(crate) fn terminalize_pending_lsp_writes(
        &mut self,
        buffer_id: Option<BufferId>,
        status: LanguageToolingStatusKind,
        message: &str,
    ) {
        let operation_ids: Vec<String> = self
            .pending_lsp_writes
            .iter()
            .filter(|(_, pending)| buffer_id.is_none_or(|id| pending.buffer_id == id))
            .map(|(id, _)| id.clone())
            .collect();
        for operation_id in operation_ids {
            self.deferred_lsp_writes.remove(&operation_id);
            self.code_action_authority
                .remove_resolve_attempt(&operation_id);
            if let Some(pending) = self.pending_lsp_writes.remove(&operation_id) {
                self.pending_code_action_contexts.retain(|_, context| {
                    context.context.workspace_id != pending.workspace_id
                        || context.context.file_id != pending.file_id
                        || context.context.buffer_id != pending.buffer_id
                        || context.context.snapshot_id != pending.snapshot_id
                        || context.context.correlation_id != pending.event_context.correlation_id
                        || context.context.causality_id != pending.event_context.causality_id
                });
                let _ = self.language_tooling.upsert_write_operation(
                    &pending,
                    status,
                    message.to_string(),
                    None,
                );
            }
        }
    }

    /// Non-blocking LSP session drain. Call once per frame tick (mirrors
    /// `TerminalWorkflow::poll`). Returns `true` if session state changed,
    /// indicating the projection snapshot should be refreshed.
    ///
    /// PKT-LSP-B T1 / D4.
    /// Non-blocking LSP session drain — call once per frame tick (PKT-LSP-B T1).
    ///
    /// 1. Advances the startup lifecycle (Starting → Live or Failed) via `drain()`.
    /// 2. Drains completed worker results and dispatches them to the appropriate
    ///    ingest method (completions, hover, definition, diagnostics).
    ///
    /// Returns `true` if the lifecycle state changed (startup transition).
    pub fn drain_lsp_session(&mut self) -> bool {
        self.server_apply_edits.expire();
        let changed = self.lsp_session.drain();
        let became_fresh = changed
            && self.lsp_session.health_record().is_some_and(|health| {
                health.init_status == legion_protocol::LspResultStatus::Fresh
            });
        if became_fresh {
            self.reset_document_sync_for_new_session();
            let open_buffers = self.active_documents.open_tabs.clone();
            for buffer_id in open_buffers {
                self.notify_lsp_did_open(buffer_id);
            }
        } else {
            self.flush_document_sync_ledger();
        }
        self.drain_deferred_lsp_writes();
        // Drain any completed worker results (non-blocking).
        let results = self.lsp_session.try_drain_results();
        for result in results {
            use crate::language::LspWorkerResult;
            match result {
                LspWorkerResult::ApplyEditRequested {
                    request,
                    reply,
                    decision,
                } => {
                    self.ingest_server_apply_edit(request, reply, decision);
                }
                LspWorkerResult::ReadResult { outcome, tag } => {
                    self.ingest_lsp_worker_result(outcome, tag);
                }
                LspWorkerResult::DiagnosticBatch {
                    raw_params,
                    captured_buffer_id,
                } => {
                    self.ingest_lsp_diagnostic_batch(raw_params, captured_buffer_id);
                }
                LspWorkerResult::TransportDead { .. } => {
                    // Intercepted inside `LspSessionHandle::try_drain_results`
                    // (routed through the restart circuit breaker); it never
                    // reaches this dispatch.  Arm kept for exhaustiveness.
                }
            }
        }
        // `try_drain_results` applies transport-death transitions; cleanup
        // after it so pending writes are terminalized in the same frame.
        let session_unavailable = self
            .lsp_session
            .health_record()
            .is_none_or(|health| health.init_status != legion_protocol::LspResultStatus::Fresh)
            || self.lsp_session.failure_reason().is_some();
        if !self.pending_lsp_writes.is_empty() && session_unavailable {
            self.terminalize_pending_lsp_writes(
                None,
                LanguageToolingStatusKind::Failed,
                "language server became unavailable while an LSP write was pending",
            );
        }
        if session_unavailable {
            self.clear_code_actions();
            self.server_apply_edits.clear();
        }
        changed
    }

    /// Ingests a completed LSP read-request result from the worker thread.
    fn ingest_server_apply_edit(
        &mut self,
        request: legion_lsp::LspApplyWorkspaceEditRequest,
        reply: std::sync::mpsc::SyncSender<legion_lsp::LspApplyWorkspaceEditResponse>,
        decision: crate::language::ApplyEditDecision,
    ) {
        let had_request_context = request.context.is_some();
        let Some(context) = request
            .context
            .or_else(|| self.context_for_unsolicited_apply_edit(&request.params))
        else {
            let _ = reply.try_send(legion_lsp::LspApplyWorkspaceEditResponse {
                applied: false,
                failure_reason: Some(
                    "workspace/applyEdit has no open document matching the edit".to_string(),
                ),
            });
            return;
        };
        if had_request_context {
            let Some(expected_context) = self
                .pending_code_action_contexts
                .get(&context.request_id.0.to_string())
            else {
                let _ = reply.try_send(legion_lsp::LspApplyWorkspaceEditResponse {
                    applied: false,
                    failure_reason: Some(
                        "workspace/applyEdit context is not an active selected command".to_string(),
                    ),
                });
                return;
            };
            if !same_lsp_operation_context(&expected_context.context, &context)
                || self.active_documents.workspace_id() != Some(context.workspace_id)
                || self
                    .active_documents
                    .metadata_for_buffer(context.buffer_id)
                    .is_none()
                || self
                    .editor
                    .current_snapshot(context.buffer_id)
                    .ok()
                    .is_none_or(|snapshot| snapshot.snapshot_id != context.snapshot_id)
                || self
                    .editor
                    .buffer_version(context.buffer_id)
                    .ok()
                    .is_none_or(|version| version != context.buffer_version)
            {
                let _ = reply.try_send(legion_lsp::LspApplyWorkspaceEditResponse {
                    applied: false,
                    failure_reason: Some(
                        "workspace/applyEdit context is not an active selected command".to_string(),
                    ),
                });
                return;
            }
        }
        let Some(edit) = request.params.get("edit") else {
            let _ = reply.try_send(legion_lsp::LspApplyWorkspaceEditResponse {
                applied: false,
                failure_reason: Some("workspace/applyEdit edit payload is missing".to_string()),
            });
            return;
        };
        if crate::language::bounded_code_action_size(edit, 256 * 1024).is_err() {
            let _ = reply.try_send(legion_lsp::LspApplyWorkspaceEditResponse {
                applied: false,
                failure_reason: Some("workspace/applyEdit payload exceeds bound".to_string()),
            });
            return;
        }
        // The server's JSON-RPC id is preserved in the operation identity for
        // diagnostics, but it is not unique across repeated requests.  Add a
        // fresh internal id so two inbound requests can never alias one
        // proposal/reply entry.
        let operation_id = format!(
            "server-apply-edit:{}:{}:{}",
            context.request_id.0,
            request.json_rpc_id,
            Uuid::now_v7()
        );
        let pending_context = self
            .pending_code_action_contexts
            .get(&context.request_id.0.to_string())
            .cloned();
        let (operation_kind, proposal_kind, title) = pending_context
            .map(|metadata| {
                let proposal_kind = if metadata.operation_kind
                    == LanguageToolingOperationKind::OrganizeImportsProposal
                {
                    LanguageProposalKind::OrganizeImports
                } else {
                    LanguageProposalKind::CodeAction
                };
                let title = if matches!(proposal_kind, LanguageProposalKind::OrganizeImports) {
                    "Organize Imports"
                } else {
                    "Server workspace edit"
                };
                (metadata.operation_kind, proposal_kind, title.to_string())
            })
            .unwrap_or((
                LanguageToolingOperationKind::CodeActionProposal,
                LanguageProposalKind::CodeAction,
                "Server workspace edit".to_string(),
            ));
        let pending = crate::language::PendingLspWriteOperation {
            operation_id: operation_id.clone(),
            operation_kind,
            workspace_id: context.workspace_id,
            file_id: context.file_id,
            buffer_id: context.buffer_id,
            snapshot_id: context.snapshot_id,
            event_context: EventContext {
                correlation_id: context.correlation_id,
                causality_id: context.causality_id,
            },
        };
        self.ingest_lsp_write_side_result(
            context.buffer_id,
            LspWriteSideSpec {
                proposal_kind,
                source_kind: WorkspaceEditSourceKind::LspCodeAction,
                title,
                detail_tag: "language_tooling.server_apply_edit",
                detail_extra: Vec::new(),
                command: None,
            },
            edit,
            pending,
        );
        let proposal_id = self
            .language_tooling
            .projection
            .operations
            .iter()
            .find(|operation| operation.operation_id == operation_id)
            .and_then(|operation| operation.proposal_id);
        let Some(proposal_id) = proposal_id else {
            let _ = reply.try_send(legion_lsp::LspApplyWorkspaceEditResponse {
                applied: false,
                failure_reason: Some("workspace/applyEdit proposal was not created".to_string()),
            });
            return;
        };
        if !self
            .server_apply_edits
            .retain(proposal_id, reply.clone(), decision, request.deadline)
        {
            let _ = reply.try_send(legion_lsp::LspApplyWorkspaceEditResponse {
                applied: false,
                failure_reason: Some("too many pending workspace/applyEdit proposals".to_string()),
            });
        }
    }

    /// Ingests a completed LSP read-request result from the worker thread.
    fn ingest_lsp_worker_result(
        &mut self,
        outcome: Result<crate::language::LspReadOutcome, crate::language::LanguageSessionError>,
        tag: crate::language::LspRequestTag,
    ) {
        use crate::language::LspReadKind;
        let is_write_operation = matches!(
            &tag.kind,
            LspReadKind::Rename { .. }
                | LspReadKind::Formatting
                | LspReadKind::CodeAction { .. }
                | LspReadKind::CodeActionResolve { .. }
                | LspReadKind::CodeActionExecuteCommand { .. }
        );
        let pending_write = if is_write_operation {
            let Some(operation_id) = tag.operation_id.as_ref() else {
                return;
            };
            let Some(pending) = self.pending_lsp_writes.remove(operation_id) else {
                return;
            };
            if let LspReadKind::CodeActionResolve {
                response_id,
                action_id,
            } = &tag.kind
                && !self.code_action_authority.finish_resolve_if_matches(
                    response_id,
                    action_id,
                    operation_id,
                )
            {
                let _ = self.language_tooling.upsert_write_operation(
                    &pending,
                    LanguageToolingStatusKind::Stale,
                    "code-action resolve response is no longer the live attempt".to_string(),
                    None,
                );
                return;
            }
            if pending.buffer_id != tag.buffer_id || pending.snapshot_id != tag.snapshot_id {
                if matches!(&tag.kind, LspReadKind::CodeActionExecuteCommand { .. })
                    && let Some(context) = tag.operation_context.as_ref()
                {
                    remove_matching_command_context(
                        &mut self.pending_code_action_contexts,
                        context,
                    );
                }
                let _ = self.language_tooling.upsert_write_operation(
                    &pending,
                    LanguageToolingStatusKind::Stale,
                    "LSP write response did not match its admitted buffer snapshot".to_string(),
                    None,
                );
                return;
            }
            Some(pending)
        } else {
            None
        };
        let lsp_outcome = match outcome {
            Ok(outcome) => outcome,
            Err(error) => {
                if matches!(&tag.kind, LspReadKind::CodeActionExecuteCommand { .. })
                    && let Some(context) = tag.operation_context.as_ref()
                {
                    remove_matching_command_context(
                        &mut self.pending_code_action_contexts,
                        context,
                    );
                }
                if let Some(pending) = pending_write.as_ref() {
                    let _ = self.language_tooling.upsert_write_operation(
                        pending,
                        LanguageToolingStatusKind::Failed,
                        format!("LSP write request failed: {error}"),
                        None,
                    );
                }
                return;
            }
        };
        self.ingest_lsp_worker_result_ok(lsp_outcome, tag, pending_write);
    }

    fn ingest_lsp_worker_result_ok(
        &mut self,
        lsp_outcome: crate::language::LspReadOutcome,
        tag: crate::language::LspRequestTag,
        pending_write: Option<crate::language::PendingLspWriteOperation>,
    ) {
        use crate::language::{LspReadKind, is_stale_response};
        // Stale-response gate: discard if snapshot moved on since the request.
        if let Ok(current_snapshot) = self.editor.current_snapshot(tag.buffer_id)
            && is_stale_response(lsp_outcome.issued_snapshot, current_snapshot.snapshot_id)
        {
            if matches!(&tag.kind, LspReadKind::CodeActionExecuteCommand { .. })
                && let Some(context) = tag.operation_context.as_ref()
            {
                remove_matching_command_context(&mut self.pending_code_action_contexts, context);
            }
            if let Some(pending) = pending_write.as_ref() {
                let _ = self.language_tooling.upsert_write_operation(
                    pending,
                    LanguageToolingStatusKind::Stale,
                    "LSP write response was stale for the current buffer snapshot".to_string(),
                    None,
                );
            }
            return; // session error; ignore
        }
        match tag.kind {
            LspReadKind::Completion => {
                let _ = self.ingest_lsp_completion_response_for_buffer(
                    tag.buffer_id,
                    &lsp_outcome.result,
                    None,
                );
            }
            LspReadKind::Hover => {
                let _ = self.ingest_lsp_hover_response_for_buffer(
                    tag.buffer_id,
                    &lsp_outcome.result,
                    None,
                );
            }
            LspReadKind::Definition => {
                let _ = self.ingest_lsp_definition_response_for_buffer(
                    tag.buffer_id,
                    &lsp_outcome.result,
                    None,
                );
            }
            LspReadKind::References => {
                let _ = self.ingest_lsp_references_response_for_buffer(
                    tag.buffer_id,
                    &lsp_outcome.result,
                    None,
                );
            }
            LspReadKind::Outline => {
                let _ = self.ingest_lsp_document_symbol_response_for_buffer(
                    tag.buffer_id,
                    &lsp_outcome.result,
                    None,
                );
            }
            LspReadKind::InlayHints => {
                let source = self.lsp_read_source_label();
                let _ = self.ingest_lsp_inlay_hint_response_for_buffer(
                    tag.buffer_id,
                    &lsp_outcome.result,
                    &source,
                    None,
                );
            }
            LspReadKind::CodeLens => {
                let source = self.lsp_read_source_label();
                let _ = self.ingest_lsp_code_lens_response_for_buffer(
                    tag.buffer_id,
                    &lsp_outcome.result,
                    &source,
                    None,
                );
            }
            LspReadKind::CallHierarchyPrepare => {
                // Step one of two. The prepare response resolves the caret to a
                // symbol; the direction the user asked for has been waiting in
                // `pending_call_hierarchy` since the request went out.
                //
                // Matched before it is taken. Taking first and checking after
                // meant a late response for an abandoned buffer consumed the
                // pending slot belonging to a newer request: ask for callers in
                // A, switch to B, ask for callees, and A's slow answer arrives,
                // takes B's slot, fails this check and returns — after which
                // B's own answer finds nothing pending and is discarded. The
                // user's most recent question then produces nothing, forever.
                if self
                    .pending_call_hierarchy
                    .as_ref()
                    .is_none_or(|pending| pending.buffer_id != tag.buffer_id)
                {
                    return;
                }
                let Some(pending) = self.pending_call_hierarchy.take() else {
                    return;
                };
                let items =
                    legion_lsp::project_prepare_call_hierarchy_response(&lsp_outcome.result)
                        .unwrap_or_default();
                let Some(item) = crate::language::first_item(&items) else {
                    // No symbol under the caret. Ordinary, not a failure, and
                    // there is nothing to follow up on.
                    return;
                };
                match pending.direction {
                    Some(legion_protocol::CallHierarchyDirection::Incoming) => {
                        self.issue_lsp_incoming_calls_request(tag.buffer_id, item);
                    }
                    Some(legion_protocol::CallHierarchyDirection::Outgoing) => {
                        self.issue_lsp_outgoing_calls_request(tag.buffer_id, item);
                    }
                    // Prepare-only: the caller wanted the symbol resolved and
                    // nothing more.
                    None => {}
                }
            }
            LspReadKind::IncomingCalls => {
                let _ = self.ingest_lsp_incoming_calls_response_for_buffer(
                    tag.buffer_id,
                    &lsp_outcome.result,
                    None,
                );
            }
            LspReadKind::OutgoingCalls => {
                let _ = self.ingest_lsp_outgoing_calls_response_for_buffer(
                    tag.buffer_id,
                    &lsp_outcome.result,
                    None,
                );
            }
            LspReadKind::Rename { new_name } => {
                let spec = LspWriteSideSpec {
                    proposal_kind: LanguageProposalKind::Rename,
                    source_kind: WorkspaceEditSourceKind::LspRename,
                    title: format!("Rename symbol to {}", bounded_label(&new_name, 64)),
                    detail_tag: "language_tooling.lsp_rename",
                    detail_extra: vec![format!("new_name={}", bounded_label(&new_name, 64))],
                    command: None,
                };
                let Some(pending) = pending_write else { return };
                self.ingest_lsp_write_side_result(
                    tag.buffer_id,
                    spec,
                    &lsp_outcome.result,
                    pending,
                );
            }
            LspReadKind::Formatting => {
                // A formatting response is a bare `TextEdit[]` for one
                // document. The proposal path speaks WorkspaceEdit, so the
                // edits are lifted into one — the same shape a rename arrives
                // in, so the same translation, preconditions and review apply.
                let Some(uri) = self.document_uri_for_buffer(tag.buffer_id) else {
                    if let Some(pending) = pending_write.as_ref() {
                        let _ = self.language_tooling.upsert_write_operation(
                            pending,
                            LanguageToolingStatusKind::Stale,
                            "formatting response arrived after the buffer closed".to_string(),
                            None,
                        );
                    }
                    return;
                };
                let edit = serde_json::json!({ "changes": { uri: lsp_outcome.result } });
                let spec = LspWriteSideSpec {
                    proposal_kind: LanguageProposalKind::Formatting,
                    source_kind: WorkspaceEditSourceKind::LspFormatting,
                    title: "Format document".to_string(),
                    detail_tag: "language_tooling.lsp_formatting",
                    detail_extra: Vec::new(),
                    command: None,
                };
                let Some(pending) = pending_write else { return };
                self.ingest_lsp_write_side_result(tag.buffer_id, spec, &edit, pending);
            }
            LspReadKind::CodeAction { organize_imports } => {
                if !organize_imports {
                    let Some(pending) = pending_write else { return };
                    self.ingest_lsp_code_actions_response(
                        tag.buffer_id,
                        &tag,
                        &lsp_outcome.result,
                        pending,
                        false,
                    );
                    return;
                }
                let Some(pending) = pending_write else { return };
                self.ingest_lsp_code_actions_response(
                    tag.buffer_id,
                    &tag,
                    &lsp_outcome.result,
                    pending,
                    true,
                );
                if let Some((response_id, action_id)) =
                    self.code_action_authority.sole_organize_edit_candidate()
                    && let Err(error) =
                        self.select_code_action_and_propose(&response_id, &action_id)
                    && let Some(buffer_id) = self
                        .code_action_authority
                        .candidate_buffer_id(&response_id, &action_id)
                    && let Some(input) = self.language_request_input_for_failure(buffer_id)
                {
                    let _ = self.language_tooling.record_proposal_failure(
                        &input,
                        LanguageProposalKind::OrganizeImports,
                        format!("organize imports selection refused: {error}"),
                    );
                }
            }
            LspReadKind::CodeActionResolve {
                response_id,
                action_id,
            } => {
                let Some(pending) = pending_write else { return };
                let Some(edit_value) = lsp_outcome.result.get("edit") else {
                    if lsp_outcome.result.get("command").is_some() {
                        let Some(workspace_id) = self.active_documents.workspace_id() else {
                            return;
                        };
                        if let Err(error) = self.issue_code_action_command(
                            tag.buffer_id,
                            &response_id,
                            &action_id,
                            &lsp_outcome.result,
                            tag.snapshot_id,
                            workspace_id,
                            pending.operation_kind,
                        ) {
                            let _ = self.language_tooling.upsert_write_operation(
                                &pending,
                                LanguageToolingStatusKind::Failed,
                                format!("resolved code-action command refused: {error}"),
                                None,
                            );
                            return;
                        }
                        self.code_action_authority.consume(&response_id, &action_id);
                        self.language_tooling.projection.code_action_candidates =
                            self.code_action_authority.projections();
                        return;
                    }
                    let _ = self.language_tooling.upsert_write_operation(
                        &pending,
                        LanguageToolingStatusKind::Failed,
                        "resolved code action did not contain a workspace edit".to_string(),
                        None,
                    );
                    return;
                };
                if crate::language::bounded_code_action_size(edit_value, 256 * 1024).is_err() {
                    let _ = self.language_tooling.upsert_write_operation(
                        &pending,
                        LanguageToolingStatusKind::Failed,
                        "resolved code-action edit exceeded the bounded payload budget".to_string(),
                        None,
                    );
                    return;
                }
                let edit = edit_value.clone();
                let command = if lsp_outcome.result.get("command").is_some() {
                    let Ok((command_id, arguments)) =
                        crate::language::extract_command(&lsp_outcome.result)
                    else {
                        let _ = self.language_tooling.upsert_write_operation(
                            &pending,
                            LanguageToolingStatusKind::Failed,
                            "resolved mixed code action command was malformed".to_string(),
                            None,
                        );
                        return;
                    };
                    if !self.lsp_session.supports_execute_command(&command_id)
                        || !self
                            .code_action_command_sidecars
                            .can_admit_arguments(&arguments)
                    {
                        let _ = self.language_tooling.upsert_write_operation(
                            &pending,
                            LanguageToolingStatusKind::Failed,
                            "resolved mixed code action command was not admitted".to_string(),
                            None,
                        );
                        return;
                    }
                    Some((command_id, arguments))
                } else {
                    None
                };
                let mut detail_extra = vec![
                    format!("response_id={response_id}"),
                    format!("action_id={action_id}"),
                ];
                if command.is_some() {
                    detail_extra.push("language_tooling.code_action_mixed_command".to_string());
                }
                let proposal_kind = if matches!(
                    pending.operation_kind,
                    LanguageToolingOperationKind::OrganizeImportsProposal
                ) {
                    LanguageProposalKind::OrganizeImports
                } else {
                    LanguageProposalKind::CodeAction
                };
                self.ingest_lsp_write_side_result(
                    tag.buffer_id,
                    LspWriteSideSpec {
                        proposal_kind,
                        source_kind: WorkspaceEditSourceKind::LspCodeAction,
                        title: "Resolved code action".to_string(),
                        detail_tag: "language_tooling.lsp_code_action_resolve",
                        detail_extra,
                        command,
                    },
                    &edit,
                    pending,
                );
                self.code_action_authority.consume(&response_id, &action_id);
                self.language_tooling.projection.code_action_candidates =
                    self.code_action_authority.projections();
            }
            LspReadKind::CodeActionExecuteCommand {
                response_id,
                action_id,
            } => {
                if let Some(context) = tag.operation_context.as_ref() {
                    self.pending_code_action_contexts
                        .remove(&context.request_id.0.to_string());
                }
                if let Some(pending) = pending_write {
                    let _ = self.language_tooling.upsert_write_operation(
                        &pending,
                        LanguageToolingStatusKind::Ready,
                        "selected code-action command completed".to_string(),
                        None,
                    );
                }
                self.code_action_authority.consume(&response_id, &action_id);
                self.language_tooling.projection.code_action_candidates =
                    self.code_action_authority.projections();
            }
        }
    }

    fn ingest_lsp_code_actions_response(
        &mut self,
        buffer_id: BufferId,
        tag: &crate::language::LspRequestTag,
        response: &serde_json::Value,
        pending: crate::language::PendingLspWriteOperation,
        organize_imports: bool,
    ) {
        let Some(workspace_id) = self.active_documents.workspace_id() else {
            return;
        };
        let resolver = AppDocumentResolver::build(&self.active_documents, &self.editor);
        let Some(actions) = response.as_array() else {
            let _ = self.language_tooling.upsert_write_operation(
                &pending,
                LanguageToolingStatusKind::Failed,
                "code-action response was not an array".to_string(),
                None,
            );
            return;
        };
        let mut candidates = Vec::new();
        let mut raw_actions = Vec::new();
        let mut response_payload_bytes = 0usize;
        for action in actions.iter().take(64) {
            let Ok(action_bytes) = crate::language::bounded_code_action_size(action, 256 * 1024)
            else {
                continue;
            };
            let Some(next_bytes) = response_payload_bytes.checked_add(action_bytes) else {
                break;
            };
            if next_bytes > 256 * 1024 {
                break;
            }
            let Some(metadata) = legion_lsp::project_code_action_response(
                &serde_json::Value::Array(vec![action.clone()]),
                1,
            )
            .into_iter()
            .next()
            .or_else(|| {
                let title = action.get("title")?.as_str()?.to_string();
                let reason = action.get("disabled")?.get("reason")?.as_str()?.to_string();
                Some(legion_lsp::LspCodeActionMetadata {
                    action_id: "disabled".to_string(),
                    title,
                    kind: action
                        .get("kind")
                        .and_then(|kind| kind.as_str())
                        .map(str::to_string),
                    is_preferred: action
                        .get("isPreferred")
                        .and_then(|preferred| preferred.as_bool())
                        .unwrap_or(false),
                    has_edit: false,
                    has_command: false,
                    disabled_reason: Some(reason),
                    schema_version: 1,
                })
            }) else {
                continue;
            };
            if organize_imports
                && metadata.kind.as_deref().is_some_and(|kind| {
                    kind != "source.organizeImports" && !kind.starts_with("source.organizeImports.")
                })
            {
                continue;
            }
            let payload = if let Some(reason) = metadata.disabled_reason.clone() {
                legion_protocol::LspCodeActionPayload::Disabled { reason }
            } else if let Some(edit) = action.get("edit") {
                let Ok(workspace_edit) = crate::language::translate_workspace_edit(
                    edit,
                    &resolver,
                    workspace_id,
                    WorkspaceEditSourceKind::LspCodeAction,
                    metadata.title.clone(),
                    CapabilityId("fs.write".to_string()),
                ) else {
                    continue;
                };
                match action.get("command") {
                    Some(command) if command.is_string() => {
                        legion_protocol::LspCodeActionPayload::EditAndCommand {
                            workspace_edit,
                            command: legion_protocol::LspCommandDescriptor {
                                command_id: command.as_str().unwrap_or_default().to_string(),
                                title: metadata.title.clone(),
                                argument_hints: Vec::new(),
                            },
                        }
                    }
                    Some(command) if command.is_object() => {
                        let Some(command_id) = command.get("command").and_then(|v| v.as_str())
                        else {
                            continue;
                        };
                        legion_protocol::LspCodeActionPayload::EditAndCommand {
                            workspace_edit,
                            command: legion_protocol::LspCommandDescriptor {
                                command_id: command_id.to_string(),
                                title: metadata.title.clone(),
                                argument_hints: Vec::new(),
                            },
                        }
                    }
                    _ => legion_protocol::LspCodeActionPayload::EditOnly { workspace_edit },
                }
            } else if let Some(data) = action.get("data") {
                legion_protocol::LspCodeActionPayload::Resolve { data: data.clone() }
            } else if let Some(command) = action.get("command") {
                let command_id = if let Some(command_id) = command.as_str() {
                    command_id.to_string()
                } else {
                    command
                        .get("command")
                        .and_then(|value| value.as_str())
                        .unwrap_or_default()
                        .to_string()
                };
                if command_id.is_empty() {
                    continue;
                }
                legion_protocol::LspCodeActionPayload::CommandOnly {
                    command: legion_protocol::LspCommandDescriptor {
                        command_id,
                        title: metadata.title.clone(),
                        argument_hints: Vec::new(),
                    },
                }
            } else {
                continue;
            };
            candidates.push(legion_protocol::LspCodeActionCandidate {
                title: metadata.title,
                kind: if organize_imports && metadata.kind.is_none() {
                    Some("source.organizeImports".to_string())
                } else {
                    metadata.kind
                },
                is_preferred: metadata.is_preferred,
                payload,
                diagnostics: Vec::new(),
            });
            raw_actions.push(action.clone());
            response_payload_bytes = next_bytes;
        }
        let Some(operation_id) = tag.operation_id.as_deref() else {
            return;
        };
        if self
            .install_code_action_response(operation_id, candidates, raw_actions)
            .is_some()
        {
            let _ = self.language_tooling.upsert_write_operation(
                &pending,
                LanguageToolingStatusKind::Ready,
                "code-action candidates ready".to_string(),
                None,
            );
        } else {
            let _ = self.language_tooling.upsert_write_operation(
                &pending,
                LanguageToolingStatusKind::Failed,
                "code-action response did not match its pending request".to_string(),
                None,
            );
        }
        let _ = buffer_id;
    }

    pub(crate) fn issue_code_action_command_sidecar(
        &mut self,
        sidecar: crate::language::CodeActionCommand,
    ) -> Result<(), String> {
        let buffer_id = sidecar.identity.buffer_id;
        let _metadata = self
            .active_documents
            .metadata_for_buffer(buffer_id)
            .ok_or_else(|| "code action buffer is no longer open".to_string())?;
        let snapshot = self
            .editor
            .current_snapshot(buffer_id)
            .map_err(|_| "code action snapshot is unavailable".to_string())?;
        if self.active_documents.workspace_id() != Some(sidecar.identity.workspace_id) {
            return Err("code action command identity is stale after edit".to_string());
        }
        let raw = serde_json::json!({
            "command": {"command": sidecar.command_id.clone(), "arguments": sidecar.arguments.clone()}
        });
        self.issue_code_action_command(
            buffer_id,
            &sidecar.response_id,
            &sidecar.action_id,
            &raw,
            snapshot.snapshot_id,
            sidecar.identity.workspace_id,
            sidecar.operation_kind,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn issue_code_action_command(
        &mut self,
        buffer_id: BufferId,
        response_id: &str,
        action_id: &str,
        raw_action: &serde_json::Value,
        snapshot_id: SnapshotId,
        workspace_id: WorkspaceId,
        operation_kind: LanguageToolingOperationKind,
    ) -> Result<(), String> {
        if self.pending_lsp_writes.len() >= 32 {
            return Err("code-action command operation limit reached".to_string());
        }
        let (command_id, arguments) =
            crate::language::extract_command(raw_action).map_err(str::to_string)?;
        if !self.lsp_session.supports_execute_command(&command_id) {
            return Err("language server did not advertise this command ID".to_string());
        }
        let params = serde_json::json!({"command": command_id, "arguments": arguments});
        if !self.lsp_workspace_sync_ready() {
            let uri = self
                .document_uri_for_buffer(buffer_id)
                .ok_or_else(|| "code action buffer is unavailable".to_string())?;
            let event_context = self.next_event_context();
            let accepted = self.defer_lsp_write_request(
                buffer_id,
                snapshot_id,
                uri,
                "workspace/executeCommand",
                crate::language::LspReadKind::CodeActionExecuteCommand {
                    response_id: response_id.to_string(),
                    action_id: action_id.to_string(),
                },
                params,
                operation_kind,
                event_context,
            );
            return accepted
                .then_some(())
                .ok_or_else(|| "code-action command deferred admission failed".to_string());
        }
        let event_context = self.next_event_context();
        let operation_id = format!("code-action-command:{response_id}:{action_id}");
        let Some(operation_context) =
            self.lsp_operation_context(buffer_id, snapshot_id, event_context)
        else {
            return Err("code action command context is unavailable".to_string());
        };
        let command_request_id = operation_context.request_id.0.to_string();
        let tag = crate::language::LspRequestTag {
            buffer_id,
            kind: crate::language::LspReadKind::CodeActionExecuteCommand {
                response_id: response_id.to_string(),
                action_id: action_id.to_string(),
            },
            snapshot_id,
            operation_id: Some(operation_id.clone()),
            operation_context: Some(operation_context.clone()),
        };
        if !self
            .lsp_session
            .issue_request("workspace/executeCommand", params.clone(), tag)
        {
            let uri = self
                .document_uri_for_buffer(buffer_id)
                .ok_or_else(|| "code action buffer is unavailable".to_string())?;
            if !self.defer_lsp_write_request(
                buffer_id,
                snapshot_id,
                uri,
                "workspace/executeCommand",
                crate::language::LspReadKind::CodeActionExecuteCommand {
                    response_id: response_id.to_string(),
                    action_id: action_id.to_string(),
                },
                params,
                operation_kind,
                event_context,
            ) {
                return Err("code-action command request was not accepted".to_string());
            }
            return Ok(());
        }
        self.pending_code_action_contexts.insert(
            command_request_id,
            crate::language::PendingLspCommandContext {
                context: operation_context,
                operation_kind,
            },
        );
        let file_id = self
            .active_documents
            .metadata_for_buffer(buffer_id)
            .map(|metadata| metadata.identity.file_id)
            .ok_or_else(|| "code action buffer is unavailable".to_string())?;
        self.pending_lsp_writes.insert(
            operation_id.clone(),
            crate::language::PendingLspWriteOperation {
                operation_id: operation_id.clone(),
                operation_kind,
                workspace_id,
                file_id,
                buffer_id,
                snapshot_id,
                event_context,
            },
        );
        if let Some(pending) = self.pending_lsp_writes.get(&operation_id) {
            let _ = self.language_tooling.upsert_write_operation(
                pending,
                LanguageToolingStatusKind::Running,
                "code-action command request accepted".to_string(),
                None,
            );
        }
        Ok(())
    }

    pub(crate) fn select_code_action_and_propose(
        &mut self,
        response_id: &str,
        action_id: &str,
    ) -> Result<(), String> {
        let buffer_id = self
            .code_action_authority
            .candidate_buffer_id(response_id, action_id)
            .ok_or_else(|| "code action buffer is unavailable".to_string())?;
        let selected = self
            .select_code_action_candidate(response_id, action_id, buffer_id)
            .map_err(str::to_string)?;
        let selected_proposal_kind = if selected.organize_imports {
            LanguageProposalKind::OrganizeImports
        } else {
            LanguageProposalKind::CodeAction
        };
        let selected_operation_kind = if matches!(
            selected_proposal_kind,
            LanguageProposalKind::OrganizeImports
        ) {
            LanguageToolingOperationKind::OrganizeImportsProposal
        } else {
            LanguageToolingOperationKind::CodeActionProposal
        };
        if selected.raw_action.get("edit").is_none()
            && selected.raw_action.get("data").is_none()
            && selected.raw_action.get("command").is_some()
        {
            self.issue_code_action_command(
                buffer_id,
                response_id,
                action_id,
                &selected.raw_action,
                selected.identity.snapshot_id,
                selected.identity.workspace_id,
                selected_operation_kind,
            )?;
            self.code_action_authority.consume(response_id, action_id);
            self.language_tooling.projection.code_action_candidates =
                self.code_action_authority.projections();
            return Ok(());
        }
        if selected.raw_action.get("edit").is_none() && selected.raw_action.get("data").is_some() {
            if self.pending_lsp_writes.len() >= 32 {
                return Err("code-action resolve operation limit reached".to_string());
            }
            if !self.lsp_server_supports_capability("codeActionResolveProvider")
                && !self.lsp_server_supports_capability("codeActionProvider.resolveProvider")
            {
                return Err("language server does not advertise code-action resolve".to_string());
            }
            let snapshot_id = selected.identity.snapshot_id;
            let event_context = self.next_event_context();
            let operation_id = format!("code-action-resolve:{}", uuid::Uuid::now_v7());
            if !self
                .code_action_authority
                .begin_resolve(response_id, action_id, &operation_id)
            {
                return Err("code-action resolve is already in flight".to_string());
            }
            let Some(operation_context) =
                self.lsp_operation_context(buffer_id, snapshot_id, event_context)
            else {
                self.code_action_authority
                    .remove_resolve_attempt(&operation_id);
                return Err("code action resolve context is unavailable".to_string());
            };
            let tag = crate::language::LspRequestTag {
                buffer_id,
                kind: crate::language::LspReadKind::CodeActionResolve {
                    response_id: response_id.to_string(),
                    action_id: action_id.to_string(),
                },
                snapshot_id,
                operation_id: Some(operation_id.clone()),
                operation_context: Some(operation_context),
            };
            if !self.lsp_workspace_sync_ready() {
                let uri = self
                    .document_uri_for_buffer(buffer_id)
                    .ok_or_else(|| "code action buffer is unavailable".to_string())?;
                if !self.defer_lsp_write_request_with_id(
                    operation_id.clone(),
                    buffer_id,
                    snapshot_id,
                    uri,
                    "codeAction/resolve",
                    crate::language::LspReadKind::CodeActionResolve {
                        response_id: response_id.to_string(),
                        action_id: action_id.to_string(),
                    },
                    selected.raw_action.clone(),
                    selected_operation_kind,
                    event_context,
                ) {
                    self.code_action_authority
                        .remove_resolve_attempt(&operation_id);
                    return Err("code action resolve deferred admission failed".to_string());
                }
                return Ok(());
            }
            if !self.lsp_session.issue_request(
                "codeAction/resolve",
                selected.raw_action.clone(),
                tag,
            ) {
                let Some(uri) = self.document_uri_for_buffer(buffer_id) else {
                    self.code_action_authority
                        .remove_resolve_attempt(&operation_id);
                    return Err("code action buffer is unavailable".to_string());
                };
                if !self.defer_lsp_write_request_with_id(
                    operation_id.clone(),
                    buffer_id,
                    snapshot_id,
                    uri,
                    "codeAction/resolve",
                    crate::language::LspReadKind::CodeActionResolve {
                        response_id: response_id.to_string(),
                        action_id: action_id.to_string(),
                    },
                    selected.raw_action.clone(),
                    selected_operation_kind,
                    event_context,
                ) {
                    self.code_action_authority
                        .remove_resolve_attempt(&operation_id);
                    return Err("code action resolve request was not accepted".to_string());
                }
                return Ok(());
            }
            self.pending_lsp_writes.insert(
                operation_id.clone(),
                crate::language::PendingLspWriteOperation {
                    operation_id: operation_id.clone(),
                    operation_kind: selected_operation_kind,
                    workspace_id: selected.identity.workspace_id,
                    file_id: self
                        .active_documents
                        .metadata_for_buffer(buffer_id)
                        .map(|metadata| metadata.identity.file_id)
                        .ok_or_else(|| "code action buffer is unavailable".to_string())?,
                    buffer_id,
                    snapshot_id,
                    event_context,
                },
            );
            if let Some(pending) = self.pending_lsp_writes.get(&operation_id) {
                let _ = self.language_tooling.upsert_write_operation(
                    pending,
                    LanguageToolingStatusKind::Running,
                    "code-action resolve request accepted".to_string(),
                    None,
                );
            }
            return Ok(());
        }
        let Some(raw_edit) = selected.raw_action.get("edit").cloned() else {
            let input = self
                .language_request_input_for_failure(buffer_id)
                .ok_or_else(|| "code action request context is unavailable".to_string())?;
            let _ = self.language_tooling.record_proposal_failure(
                &input,
                selected_proposal_kind,
                "command-only code actions require an explicit execute-command authority"
                    .to_string(),
            );
            return Err("command-only code actions are unsupported".to_string());
        };
        let command = if selected.raw_action.get("command").is_some() {
            let (command_id, arguments) =
                crate::language::extract_command(&selected.raw_action).map_err(str::to_string)?;
            if !self.lsp_session.supports_execute_command(&command_id) {
                return Err("language server did not advertise this command ID".to_string());
            }
            if !self
                .code_action_command_sidecars
                .can_admit_arguments(&arguments)
            {
                return Err("code-action command sidecar budget is exhausted".to_string());
            }
            Some((command_id, arguments))
        } else {
            None
        };
        let Some(metadata) = self
            .active_documents
            .metadata_for_buffer(buffer_id)
            .cloned()
        else {
            return Err("code action buffer is unavailable".to_string());
        };
        let operation_id = format!("code-action-selection:{response_id}:{action_id}");
        let pending = crate::language::PendingLspWriteOperation {
            operation_id,
            operation_kind: selected_operation_kind,
            workspace_id: selected.identity.workspace_id,
            file_id: metadata.identity.file_id,
            buffer_id,
            snapshot_id: selected.identity.snapshot_id,
            event_context: self.next_event_context(),
        };
        let mut detail_extra = vec![
            format!("response_id={response_id}"),
            format!("action_id={action_id}"),
        ];
        if command.is_some() {
            detail_extra.push("language_tooling.code_action_mixed_command".to_string());
        }
        self.ingest_lsp_write_side_result(
            buffer_id,
            LspWriteSideSpec {
                proposal_kind: selected_proposal_kind,
                source_kind: WorkspaceEditSourceKind::LspCodeAction,
                title: selected.title,
                detail_tag: "language_tooling.lsp_code_action_selection",
                detail_extra,
                command,
            },
            &raw_edit,
            pending,
        );
        self.code_action_authority.consume(response_id, action_id);
        self.language_tooling.projection.code_action_candidates =
            self.code_action_authority.projections();
        Ok(())
    }

    /// Ingests a completed LSP `textDocument/rename` result from the worker
    /// thread (PKT-LSP-C I-2).
    ///
    /// Translates the raw `WorkspaceEdit` JSON via [`translate_workspace_edit`]
    /// using an [`AppDocumentResolver`] backed by the current active-document
    /// state.  On success, creates and registers a proposal through the
    /// coordinator and records it in the language-tooling projection.
    ///
    /// The resulting proposal enters the `Previewed` state. Call
    /// [`approve_and_apply_rename_proposal`] to transition it through
    /// `Approved` → `Applied` (PKT-APPLY Task 2c).
    ///
    /// `pub(crate)` because the external-formatter route in
    /// `crate::language::external_formatter` lifts a whole-document
    /// replacement into the same `{"changes": {uri: [TextEdit]}}` shape and
    /// hands it here. That is the point: one translation, one preconditions
    /// check, one proposal lifecycle, one `workspace/applyEdit` arbitration.
    pub(crate) fn ingest_lsp_write_side_result(
        &mut self,
        buffer_id: BufferId,
        spec: LspWriteSideSpec,
        raw: &serde_json::Value,
        pending: crate::language::PendingLspWriteOperation,
    ) {
        use crate::language::translate_workspace_edit;

        let LspWriteSideSpec {
            proposal_kind,
            source_kind,
            title,
            detail_tag,
            detail_extra,
            command,
        } = spec;

        let Some(workspace_id) = self.active_documents.workspace_id() else {
            let _ = self.language_tooling.upsert_write_operation(
                &pending,
                LanguageToolingStatusKind::Unavailable,
                format!("{title}: workspace is no longer active"),
                None,
            );
            return;
        };
        let Some(meta) = self
            .active_documents
            .metadata_for_buffer(buffer_id)
            .cloned()
        else {
            let _ = self.language_tooling.upsert_write_operation(
                &pending,
                LanguageToolingStatusKind::Stale,
                format!("{title}: buffer is no longer open"),
                None,
            );
            return;
        };
        let principal = self
            .active_documents
            .active_principal_id
            .clone()
            .unwrap_or_else(|| PrincipalId("system".to_string()));

        let event_context = pending.event_context;
        let text = self
            .editor
            .text(buffer_id)
            .ok()
            .map(|t| t.to_string())
            .unwrap_or_default();
        let snapshot_id = self
            .editor
            .current_snapshot(buffer_id)
            .ok()
            .map(|s| s.snapshot_id)
            .unwrap_or(legion_protocol::SnapshotId(0));
        let buffer_version = self
            .editor
            .buffer_version(buffer_id)
            .ok()
            .unwrap_or(BufferVersion(0));

        let input = LanguageRequestInput {
            workspace_id,
            buffer_id,
            metadata: meta.clone(),
            principal,
            text,
            snapshot_id,
            buffer_version,
            event_context,
            open_files: self.active_documents.open_file_ids(),
        };

        let capability = CapabilityId("fs.write".to_string());

        // Build the production DocumentResolver from the current open-buffer state.
        let resolver = AppDocumentResolver::build(&self.active_documents, &self.editor);

        let health = self.lsp_session.health_record();
        let raw = crate::language::translate::normalize_pyright_annotations(raw, health.as_ref());
        let workspace_edit = match translate_workspace_edit(
            &raw,
            &resolver,
            workspace_id,
            source_kind,
            title.clone(),
            capability.clone(),
        ) {
            Ok(payload) => payload,
            Err(err) => {
                let _ = self
                    .language_tooling
                    .record_proposal_failure_with_operation_id(
                        &input,
                        proposal_kind,
                        format!("{title}: LSP translation failed: {err}"),
                        Some(pending.operation_id.clone()),
                    );
                return;
            }
        };

        if let Some((_, arguments)) = command.as_ref()
            && !self
                .code_action_command_sidecars
                .can_admit_arguments(arguments)
        {
            let _ = self.language_tooling.upsert_write_operation(
                &pending,
                LanguageToolingStatusKind::Failed,
                format!("{title}: command sidecar budget is exhausted"),
                None,
            );
            return;
        }

        let preconditions = ProposalVersionPreconditions {
            file_version: Some(meta.file_content_version),
            buffer_version: Some(input.buffer_version),
            snapshot_id: Some(input.snapshot_id),
            generation: Some(meta.workspace_generation),
            file_content_version: Some(meta.file_content_version),
            workspace_generation: Some(meta.workspace_generation),
            expected_fingerprint: Some(meta.fingerprint.clone()),
            expected_file_length: meta.file_length,
            expected_modified_at: meta.modified_at,
        };

        let proposal_id = self.proposal_coordinator.next_id();
        let request = LspRequestCorrelation {
            request_id: legion_protocol::LspRequestId(uuid::Uuid::now_v7()),
            server_id: legion_protocol::LanguageServerId(1),
            workspace_id: input.workspace_id,
            file_id: Some(meta.identity.file_id),
            snapshot_id: Some(input.snapshot_id),
            buffer_version: Some(input.buffer_version),
            correlation_id: input.event_context.correlation_id,
            causality_id: input.event_context.causality_id,
            cancellation_token: Some(CancellationTokenId(uuid::Uuid::now_v7())),
            privacy_scope: SemanticPrivacyScope::Workspace,
            issued_at: TimestampMillis::now(),
            schema_version: 1,
        };

        let proposal = match legion_protocol::convert_lsp_edit_to_workspace_proposal(
            LspEditProposalConversionInput {
                proposal_id,
                principal: input.principal.clone(),
                capability,
                request,
                workspace_edit,
                preconditions,
                lifecycle_state: ProposalLifecycleState::Created,
                privacy_label: legion_protocol::ProposalPrivacyLabel::WorkspaceMetadata,
                preview: PreviewSummary {
                    summary: title.clone(),
                    details: {
                        let mut details = vec![detail_tag.to_string()];
                        details.extend(detail_extra.iter().cloned());
                        details
                    },
                },
                expires_at: None,
                created_at: TimestampMillis::now(),
                diagnostics: Vec::new(),
                schema_version: 1,
            },
        ) {
            Ok(p) => p,
            Err(err) => {
                let _ = self
                    .language_tooling
                    .record_proposal_failure_with_operation_id(
                        &input,
                        proposal_kind,
                        format!("{title}: proposal creation failed: {err:?}"),
                        Some(pending.operation_id.clone()),
                    );
                return;
            }
        };

        self.proposal_coordinator
            .register_lifecycle_context(proposal.proposal_id, input.event_context);
        let created = self.proposal_coordinator.created_response(&proposal);
        if !matches!(created, ProposalResponse::Created(_)) {
            let _ = self
                .language_tooling
                .record_proposal_failure_with_operation_id(
                    &input,
                    proposal_kind,
                    format!("{title}: proposal coordinator rejected: {created:?}"),
                    Some(pending.operation_id.clone()),
                );
            return;
        }
        let validated = self
            .proposal_coordinator
            .handle(ProposalRequest::Validate(proposal.clone()));
        if !matches!(validated, Ok(ProposalResponse::Validated(_))) {
            let _ = self
                .language_tooling
                .record_proposal_failure_with_operation_id(
                    &input,
                    proposal_kind,
                    format!("{title}: proposal validation failed: {validated:?}"),
                    Some(pending.operation_id.clone()),
                );
            return;
        }
        let previewed = self
            .proposal_coordinator
            .handle(ProposalRequest::Preview(proposal.clone()));
        if !matches!(previewed, Ok(ProposalResponse::Previewed { .. })) {
            let _ = self
                .language_tooling
                .record_proposal_failure_with_operation_id(
                    &input,
                    proposal_kind,
                    format!("{title}: proposal preview failed: {previewed:?}"),
                    Some(pending.operation_id.clone()),
                );
            return;
        }
        if let Some((command_id, arguments)) = command {
            let sidecar = crate::language::CodeActionCommand {
                command_id,
                arguments,
                response_id: detail_extra
                    .iter()
                    .find_map(|detail| detail.strip_prefix("response_id=").map(str::to_string))
                    .unwrap_or_default(),
                action_id: detail_extra
                    .iter()
                    .find_map(|detail| detail.strip_prefix("action_id=").map(str::to_string))
                    .unwrap_or_default(),
                operation_kind: pending.operation_kind,
                identity: crate::language::CodeActionIdentity {
                    workspace_id: input.workspace_id,
                    buffer_id,
                    snapshot_id: input.snapshot_id,
                    buffer_version: input.buffer_version,
                    file_content_version: meta.file_content_version,
                    workspace_generation: meta.workspace_generation,
                    fingerprint: meta.fingerprint.clone(),
                },
            };
            if self
                .code_action_command_sidecars
                .insert(proposal.proposal_id, sidecar)
                .is_err()
            {
                let _ = self
                    .language_tooling
                    .record_proposal_failure_with_operation_id(
                        &input,
                        proposal_kind,
                        format!("{title}: command sidecar admission failed"),
                        Some(pending.operation_id.clone()),
                    );
                return;
            }
        }
        let _ = self.language_tooling.record_proposal(
            &input,
            proposal_kind,
            proposal.proposal_id,
            None,
            format!("{title}: proposal generated"),
            Some(pending.operation_id),
        );
    }

    /// Approve and apply a rename proposal that is in the `Previewed` or
    /// `Approved` state (PKT-APPLY Task 2c).
    ///
    /// This is the user-triggered Approve→Apply path for LSP rename proposals.
    /// It transitions the proposal to `Approved` (recording explicit human
    /// approval), then dispatches it to `apply_workspace_proposal`.
    ///
    /// Returns `Err` if the proposal is not found or is not in a state that
    /// accepts approval, or if the apply fails at the composition level.
    pub fn approve_and_apply_rename_proposal(
        &mut self,
        proposal_id: ProposalId,
    ) -> Result<ProposalResponse, AppCompositionError> {
        let proposal = self
            .proposal_coordinator
            .proposal(proposal_id)
            .ok_or_else(|| {
                AppCompositionError::Protocol(ProtocolError {
                    code: "proposal.not_found".to_string(),
                    message: format!("rename proposal {proposal_id:?} not found in coordinator"),
                })
            })?;

        // Only approve if currently in Previewed state; if already Approved, skip.
        if matches!(
            self.proposal_coordinator
                .current_lifecycle_state(proposal_id),
            Some(ProposalLifecycleState::Previewed)
        ) {
            let approve_command = ProposalLifecycleCommand {
                proposal_id,
                action: ProposalLifecycleAction::Approve,
                principal: proposal.principal.clone(),
                capability: proposal.capability.clone(),
                correlation_id: proposal.correlation_id,
                causality_id: CausalityId(uuid::Uuid::now_v7()),
                reason: None,
                diagnostics: vec![],
                requested_at: TimestampMillis(0),
                schema_version: 1,
            };
            let approved =
                self.handle_lifecycle_command_request(ProposalRequest::Approve(approve_command))?;
            if !matches!(approved, ProposalResponse::Approved(_)) {
                return Ok(approved);
            }
        }

        // Re-fetch after approval state change.
        let proposal = self
            .proposal_coordinator
            .proposal(proposal_id)
            .ok_or_else(|| {
                AppCompositionError::Protocol(ProtocolError {
                    code: "proposal.not_found_after_approve".to_string(),
                    message: format!("rename proposal {proposal_id:?} not found after approval"),
                })
            })?;

        self.handle_proposal_request(ProposalRequest::Apply(proposal))
    }

    /// Ingests a raw `publishDiagnostics` notification batch from the worker.
    fn ingest_lsp_diagnostic_batch(
        &mut self,
        raw_params: serde_json::Value,
        captured_buffer_id: Option<BufferId>,
    ) {
        // Extract URI from params to look up the buffer.
        let Some(uri) = raw_params.get("uri").and_then(|v| v.as_str()) else {
            return;
        };
        // Match against the authoritative URI derived from each open buffer.
        // This normalizes Windows verbatim prefixes and percent encoding while
        // avoiding filesystem probing or arbitrary path rewriting.
        let Some(normalized_uri) = crate::normalize_lsp_document_uri(uri) else {
            return;
        };
        let Some(buffer_id) = self
            .active_documents
            .open_tabs
            .iter()
            .copied()
            .find(|buffer_id| {
                self.active_documents
                    .metadata_for_buffer(*buffer_id)
                    .is_some_and(|metadata| {
                        crate::lsp_file_uris_refer_to_same_document(
                            &normalized_uri,
                            &crate::canonical_path_to_uri(&metadata.identity.canonical_path.0),
                        )
                    })
            })
        else {
            return; // Not an open buffer; ignore (uri-filtered).
        };
        if captured_buffer_id != Some(buffer_id) {
            return;
        }
        let Ok(current_version) = self.editor.buffer_version(buffer_id) else {
            return;
        };
        if !Self::diagnostic_version_is_current(&raw_params, current_version.0 as i64) {
            return;
        }
        let Ok(snapshot) = self.editor.current_snapshot(buffer_id) else {
            return;
        };
        let snapshot_id = snapshot.snapshot_id;
        self.code_action_diagnostics.replace_from_params(
            crate::language::DiagnosticIdentity {
                buffer_id,
                snapshot_id,
                buffer_version: current_version,
            },
            &raw_params,
        );
        // Project + ingest through redaction layer.
        let _ = self.ingest_lsp_publish_diagnostics_for_buffer(buffer_id, &raw_params, true, None);
    }

    fn diagnostic_version_is_current(raw_params: &serde_json::Value, current: i64) -> bool {
        raw_params
            .get("version")
            .and_then(serde_json::Value::as_i64)
            .is_none_or(|reported| reported == current)
    }

    /// Returns true when the live LSP server advertises support for `capability`.
    ///
    /// If the server has not yet advertised capabilities (empty list, e.g. during
    /// startup or when the session is idle/refused), returns `false` so callers
    /// silently skip the request rather than firing into a dead session.
    ///
    /// An empty capability list from a live session is treated as *not supported*
    /// (fail-closed) rather than "assume all" — callers must wait until capabilities
    /// are populated by a successful `initialize` handshake.
    fn lsp_server_supports_capability(&self, capability: &str) -> bool {
        let Some(record) = self.lsp_session.health_record() else {
            return false; // No session at all.
        };
        if record.capabilities.is_empty() {
            // No capability list published yet (e.g. startup refused, or initialize
            // has not been called). Fail-closed: do not fire the request.
            return false;
        }
        record
            .capabilities
            .iter()
            .any(|c| c.capability == capability && c.supported)
    }

    /// Explains why a rename cannot be admitted without conflating a capable
    /// server with a document that is still waiting in the bounded sync queue.
    ///
    /// Startup can report `Fresh` while the cap-one worker is still delivering
    /// `didOpen` for another open buffer.  A command issued during that window
    /// is a truthful pending-sync state, not a missing server capability.
    pub(crate) fn lsp_rename_unavailable_message(&self, buffer_id: BufferId) -> &'static str {
        if self.lsp_server_supports_capability("renameProvider")
            && self.lsp_document_sync_pending(buffer_id)
        {
            "rename waiting for document synchronization"
        } else {
            "rename unavailable until a live capable language server is ready"
        }
    }

    fn lsp_document_sync_pending(&self, buffer_id: BufferId) -> bool {
        let Some(meta) = self.active_documents.metadata_for_buffer(buffer_id) else {
            return false;
        };
        let uri = canonical_path_to_uri(&meta.identity.canonical_path.0);
        self.document_sync_ledger.get(&uri).is_some_and(|entries| {
            entries.iter().rev().any(|entry| {
                matches!(
                    entry,
                    DesiredDocumentSync::Open {
                        buffer_id: entry_buffer,
                        sent: false,
                        ..
                    } if *entry_buffer == buffer_id
                )
            })
        })
    }

    /// Issues a non-blocking LSP completion request on the worker thread.
    ///
    /// Returns `false` if the session is not Live, or if the server did not
    /// advertise `completionProvider` in the initialize response (silent skip).
    pub fn issue_lsp_completion_request(
        &mut self,
        buffer_id: BufferId,
        position: TextCoordinate,
    ) -> bool {
        if !self.lsp_server_supports_capability("completionProvider") {
            return false;
        }
        let Some(meta) = self
            .active_documents
            .metadata_for_buffer(buffer_id)
            .cloned()
        else {
            return false;
        };
        let Ok(snapshot) = self.editor.current_snapshot(buffer_id) else {
            return false;
        };
        let snapshot_id = snapshot.snapshot_id;
        if !self.lsp_document_sync_ready(buffer_id) {
            return false;
        }
        let uri = canonical_path_to_uri(&meta.identity.canonical_path.0);
        let params = serde_json::json!({
            "textDocument": { "uri": uri },
            "position": { "line": position.line, "character": position.character }
        });
        let event_context = self.next_event_context();
        let Some(operation_context) =
            self.lsp_operation_context(buffer_id, snapshot_id, event_context)
        else {
            return false;
        };
        let tag = crate::language::LspRequestTag {
            buffer_id,
            kind: crate::language::LspReadKind::Completion,
            snapshot_id,
            operation_id: None,
            operation_context: Some(operation_context),
        };
        self.lsp_session
            .issue_request("textDocument/completion", params, tag)
    }

    /// Issues a non-blocking LSP hover request on the worker thread.
    ///
    /// Returns `false` if the session is not Live, or if the server did not
    /// advertise `hoverProvider` in the initialize response (silent skip).
    pub fn issue_lsp_hover_request(
        &mut self,
        buffer_id: BufferId,
        position: TextCoordinate,
    ) -> bool {
        if !self.lsp_server_supports_capability("hoverProvider") {
            return false;
        }
        let Some(meta) = self
            .active_documents
            .metadata_for_buffer(buffer_id)
            .cloned()
        else {
            return false;
        };
        let Ok(snapshot) = self.editor.current_snapshot(buffer_id) else {
            return false;
        };
        let snapshot_id = snapshot.snapshot_id;
        if !self.lsp_document_sync_ready(buffer_id) {
            return false;
        }
        let uri = canonical_path_to_uri(&meta.identity.canonical_path.0);
        let event_context = self.next_event_context();
        let Some(operation_context) =
            self.lsp_operation_context(buffer_id, snapshot_id, event_context)
        else {
            return false;
        };
        let params = serde_json::json!({
            "textDocument": { "uri": uri },
            "position": { "line": position.line, "character": position.character }
        });
        let tag = crate::language::LspRequestTag {
            buffer_id,
            kind: crate::language::LspReadKind::Hover,
            snapshot_id,
            operation_id: None,
            operation_context: Some(operation_context),
        };
        self.lsp_session
            .issue_request("textDocument/hover", params, tag)
    }

    /// Issues a non-blocking LSP go-to-definition request on the worker thread.
    ///
    /// Returns `false` if the session is not Live, or if the server did not
    /// advertise `definitionProvider` in the initialize response (silent skip).
    pub fn issue_lsp_definition_request(
        &mut self,
        buffer_id: BufferId,
        position: TextCoordinate,
    ) -> bool {
        if !self.lsp_server_supports_capability("definitionProvider") {
            return false;
        }
        let Some(meta) = self
            .active_documents
            .metadata_for_buffer(buffer_id)
            .cloned()
        else {
            return false;
        };
        let Ok(snapshot) = self.editor.current_snapshot(buffer_id) else {
            return false;
        };
        let snapshot_id = snapshot.snapshot_id;
        if !self.lsp_document_sync_ready(buffer_id) {
            return false;
        }
        let uri = canonical_path_to_uri(&meta.identity.canonical_path.0);
        let event_context = self.next_event_context();
        let Some(operation_context) =
            self.lsp_operation_context(buffer_id, snapshot_id, event_context)
        else {
            return false;
        };
        let params = serde_json::json!({
            "textDocument": { "uri": uri },
            "position": { "line": position.line, "character": position.character }
        });
        let tag = crate::language::LspRequestTag {
            buffer_id,
            kind: crate::language::LspReadKind::Definition,
            snapshot_id,
            operation_id: None,
            operation_context: Some(operation_context),
        };
        self.lsp_session
            .issue_request("textDocument/definition", params, tag)
    }

    /// Names the server a hint or lens came from, for display beside it.
    ///
    /// Inlay hints and code lenses are the two read results that get attributed
    /// in the UI — a hint sitting inside your code should say who put it there.
    /// Falls back to `"lsp"` before `initialize` has answered, rather than
    /// inventing a server name.
    pub(crate) fn lsp_read_source_label(&self) -> String {
        self.lsp_session
            .health_record()
            .map(|record| record.language_id.0)
            .filter(|label| !label.is_empty())
            .unwrap_or_else(|| "lsp".to_string())
    }

    /// Shared preamble for the position-free read requests.
    ///
    /// Returns the document URI and the snapshot the request is being issued
    /// against, or `None` if the buffer has no metadata or no readable
    /// snapshot. Every read request needs both and none of them can proceed
    /// without them, so the four callers below say it once.
    fn lsp_read_target(
        &mut self,
        buffer_id: BufferId,
    ) -> Option<(String, legion_protocol::SnapshotId)> {
        let meta = self
            .active_documents
            .metadata_for_buffer(buffer_id)
            .cloned()?;
        let snapshot = self.editor.current_snapshot(buffer_id).ok()?;
        Some((
            canonical_path_to_uri(&meta.identity.canonical_path.0),
            snapshot.snapshot_id,
        ))
    }

    /// The UTF-16 range covering the whole of a buffer.
    ///
    /// Inlay hints are requested per-range, and the editor does not yet plumb
    /// its visible line range down to this layer. Asking for the whole document
    /// is correct but not cheap on a large file; narrowing it to the viewport
    /// is a refinement this signature already allows, and the reason
    /// `issue_lsp_inlay_hint_request` takes a range rather than computing one.
    pub(crate) fn whole_document_utf16_range(
        &mut self,
        buffer_id: BufferId,
    ) -> Option<legion_protocol::Utf16Range> {
        let snapshot = self.editor.current_snapshot(buffer_id).ok()?;
        Some(legion_protocol::Utf16Range {
            start: legion_protocol::Utf16Position {
                line: 0,
                character: 0,
            },
            // One line past the last, at character zero. The snapshot
            // descriptor carries `line_count` but not the text, and reading the
            // whole document back just to find the final column would be an
            // absurd cost for a range every server clamps anyway. LSP positions
            // past the end of a document are defined to clamp to it.
            end: legion_protocol::Utf16Position {
                line: u32::try_from(snapshot.line_count).unwrap_or(u32::MAX),
                character: 0,
            },
        })
    }

    /// Test-only: the document URI a request for this buffer would carry.
    ///
    /// Tests that hand the app a `WorkspaceEdit` must key it the way the
    /// resolver does. Guessing the URI from a temp path is how a test ends up
    /// asserting against its own URI-building bug instead of the app's.
    #[cfg(any(test, feature = "test-helpers"))]
    pub fn document_uri_for_buffer_for_test(&self, buffer_id: BufferId) -> Option<String> {
        self.active_documents
            .metadata_for_buffer(buffer_id)
            .map(|meta| canonical_path_to_uri(&meta.identity.canonical_path.0))
    }

    /// Test-only: the snapshot id a request issued now would carry.
    #[cfg(any(test, feature = "test-helpers"))]
    pub fn current_snapshot_id_for_test(
        &mut self,
        buffer_id: BufferId,
    ) -> Option<legion_protocol::SnapshotId> {
        self.editor
            .current_snapshot(buffer_id)
            .ok()
            .map(|snapshot| snapshot.snapshot_id)
    }

    /// Test-only: put the session Live and hand back the worker's result
    /// channel, so a test can inject a read result and watch `drain_lsp_session`
    /// route it.
    ///
    /// This is the only way to exercise the drain-side routing without a real
    /// language server: the routing decides which of seven ingest methods a
    /// result reaches, and getting it wrong silently drops a feature rather
    /// than failing anything.
    #[cfg(any(test, feature = "test-helpers"))]
    pub fn inject_lsp_result_sender_for_test(
        &mut self,
        health: legion_protocol::LspServerHealthRecord,
    ) -> std::sync::mpsc::SyncSender<crate::language::LspWorkerResult> {
        let sender = self
            .lsp_session
            .set_live_with_result_sender_for_test(health);
        self.mark_injected_lsp_documents_ready_for_test();
        sender
    }

    /// Treat an injected Live session like a real Fresh handshake: every open
    /// buffer must have its desired `didOpen` marked sent before reads or
    /// `didChange` can go out.
    #[cfg(any(test, feature = "test-helpers"))]
    pub(crate) fn mark_injected_lsp_documents_ready_for_test(&mut self) {
        self.reset_document_sync_for_new_session();
        let open_buffers = self.active_documents.open_tabs.clone();
        for buffer_id in open_buffers {
            self.notify_lsp_did_open(buffer_id);
        }
    }

    /// Test-only: whether the injected or live session has sent `didOpen` for
    /// `buffer_id` at the current buffer version.
    #[cfg(any(test, feature = "test-helpers"))]
    pub fn lsp_document_sync_ready_for_test(&self, buffer_id: BufferId) -> bool {
        self.lsp_document_sync_ready(buffer_id)
    }

    /// Test-only: the label that would be attached to hints and lenses now.
    #[cfg(any(test, feature = "test-helpers"))]
    pub fn lsp_read_source_label_for_test(&self) -> String {
        self.lsp_read_source_label()
    }

    /// Test-only: the range an inlay-hint refresh would ask for.
    #[cfg(any(test, feature = "test-helpers"))]
    pub fn whole_document_utf16_range_for_test(
        &mut self,
        buffer_id: BufferId,
    ) -> Option<legion_protocol::Utf16Range> {
        self.whole_document_utf16_range(buffer_id)
    }

    /// Capability gate, target lookup, tag, send — the five lines every read
    /// request shares.
    ///
    /// Written once because six copies of a capability check is how one of them
    /// ends up checking the wrong capability, or none at all: the copies are
    /// similar enough that a wrong one reads as right. Each public method is now
    /// its capability, its method name, its kind and its params.
    fn issue_lsp_read(
        &mut self,
        buffer_id: BufferId,
        capability: &str,
        method: &str,
        kind: crate::language::LspReadKind,
        params: impl FnOnce(&str) -> serde_json::Value,
    ) -> bool {
        if !self.lsp_server_supports_capability(capability) {
            return false;
        }
        let Some((uri, snapshot_id)) = self.lsp_read_target(buffer_id) else {
            return false;
        };
        if !self.lsp_document_sync_ready(buffer_id) {
            return false;
        }
        let event_context = self.next_event_context();
        let Some(operation_context) =
            self.lsp_operation_context(buffer_id, snapshot_id, event_context)
        else {
            return false;
        };
        let tag = crate::language::LspRequestTag {
            buffer_id,
            kind: kind.clone(),
            snapshot_id,
            operation_id: None,
            operation_context: Some(operation_context),
        };
        self.lsp_session.issue_request(method, params(&uri), tag)
    }

    /// Admits one bounded write-side request and records only its correlation metadata.
    fn issue_lsp_write_read(
        &mut self,
        buffer_id: BufferId,
        capability: &str,
        method: &str,
        kind: crate::language::LspReadKind,
        params: impl FnOnce(&str) -> serde_json::Value,
        event_context: crate::EventContext,
    ) -> bool {
        if self.pending_lsp_writes.len() >= 32 || !self.lsp_server_supports_capability(capability) {
            return false;
        }
        let Some((uri, snapshot_id)) = self.lsp_read_target(buffer_id) else {
            return false;
        };
        let document_sync_ready =
            self.lsp_document_sync_ready(buffer_id) && self.lsp_workspace_sync_ready();
        let Some(workspace_id) = self.active_documents.workspace_id() else {
            return false;
        };
        let Some(file_id) = self
            .active_documents
            .metadata_for_buffer(buffer_id)
            .map(|metadata| metadata.identity.file_id)
        else {
            return false;
        };
        let operation_kind = match &kind {
            crate::language::LspReadKind::Formatting => {
                LanguageToolingOperationKind::FormattingProposal
            }
            crate::language::LspReadKind::CodeAction {
                organize_imports: true,
            } => LanguageToolingOperationKind::OrganizeImportsProposal,
            _ => LanguageToolingOperationKind::CodeActionProposal,
        };
        let operation_id = Uuid::now_v7().to_string();
        let Some(operation_context) =
            self.lsp_operation_context(buffer_id, snapshot_id, event_context)
        else {
            return false;
        };
        let request_params = params(&uri);
        if !document_sync_ready {
            return self.defer_lsp_write_request(
                buffer_id,
                snapshot_id,
                uri,
                method,
                kind,
                request_params,
                operation_kind,
                event_context,
            );
        }
        let tag = crate::language::LspRequestTag {
            buffer_id,
            kind: kind.clone(),
            snapshot_id,
            operation_id: Some(operation_id.clone()),
            operation_context: Some(operation_context),
        };
        if !self
            .lsp_session
            .issue_request(method, request_params.clone(), tag)
        {
            return self.defer_lsp_write_request(
                buffer_id,
                snapshot_id,
                uri,
                method,
                kind,
                request_params,
                operation_kind,
                event_context,
            );
        }
        self.pending_lsp_writes.insert(
            operation_id.clone(),
            crate::language::PendingLspWriteOperation {
                operation_id: operation_id.clone(),
                operation_kind,
                workspace_id,
                file_id,
                buffer_id,
                snapshot_id,
                event_context,
            },
        );
        let _ = self.language_tooling.upsert_write_operation(
            self.pending_lsp_writes
                .get(&operation_id)
                .expect("inserted write operation"),
            LanguageToolingStatusKind::Running,
            "LSP write request accepted".to_string(),
            None,
        );
        true
    }

    /// Issues a non-blocking LSP references request on the worker thread.
    ///
    /// Returns `false` if the session is not Live, or if the server did not
    /// advertise `referencesProvider` in the initialize response (silent skip).
    ///
    /// `include_declaration` follows the LSP parameter of the same name: the
    /// declaration site itself is usually wanted in a "find all references"
    /// result and usually not wanted when the caller is about to rename.
    pub fn issue_lsp_references_request(
        &mut self,
        buffer_id: BufferId,
        position: TextCoordinate,
        include_declaration: bool,
    ) -> bool {
        self.issue_lsp_read(
            buffer_id,
            "referencesProvider",
            "textDocument/references",
            crate::language::LspReadKind::References,
            |uri| {
                serde_json::json!({
                    "textDocument": { "uri": uri },
                    "position": { "line": position.line, "character": position.character },
                    "context": { "includeDeclaration": include_declaration }
                })
            },
        )
    }

    /// Issues `textDocument/prepareCallHierarchy` for the caret position.
    ///
    /// Step one of two: the response resolves a position to a symbol item, and
    /// only then can callers or callees be asked for. `direction` is stashed in
    /// `pending_call_hierarchy` so the drain knows which follow-up to issue —
    /// `None` means the caller wanted only the symbol resolved.
    ///
    /// Returns `false` if the session is not Live or the server did not
    /// advertise `callHierarchyProvider`.
    pub fn issue_lsp_prepare_call_hierarchy_request(
        &mut self,
        buffer_id: BufferId,
        position: TextCoordinate,
        direction: Option<legion_protocol::CallHierarchyDirection>,
    ) -> bool {
        let issued = self.issue_lsp_read(
            buffer_id,
            "callHierarchyProvider",
            "textDocument/prepareCallHierarchy",
            crate::language::LspReadKind::CallHierarchyPrepare,
            |uri| crate::language::prepare_params(uri, position),
        );
        // Only remembered when the request actually went out. Setting it
        // regardless would leave a direction waiting for a response that will
        // never arrive, and the next prepare would answer the wrong question.
        self.pending_call_hierarchy = issued.then_some(crate::language::PendingCallHierarchy {
            buffer_id,
            direction,
        });
        issued
    }

    /// Issues `callHierarchy/incomingCalls` for a prepared item.
    pub fn issue_lsp_incoming_calls_request(
        &mut self,
        buffer_id: BufferId,
        item: &legion_protocol::LspCallHierarchyItem,
    ) -> bool {
        let params = crate::language::call_params(item);
        self.issue_lsp_read(
            buffer_id,
            "callHierarchyProvider",
            "callHierarchy/incomingCalls",
            crate::language::LspReadKind::IncomingCalls,
            move |_uri| params,
        )
    }

    /// Issues `callHierarchy/outgoingCalls` for a prepared item.
    pub fn issue_lsp_outgoing_calls_request(
        &mut self,
        buffer_id: BufferId,
        item: &legion_protocol::LspCallHierarchyItem,
    ) -> bool {
        let params = crate::language::call_params(item);
        self.issue_lsp_read(
            buffer_id,
            "callHierarchyProvider",
            "callHierarchy/outgoingCalls",
            crate::language::LspReadKind::OutgoingCalls,
            move |_uri| params,
        )
    }

    /// The command outcome for a call-hierarchy request.
    ///
    /// Here rather than inline in the dispatch match: `lib.rs` is a chokepoint
    /// with a growth budget, and three multi-line arms is precisely the shape
    /// of feature work the rule says belongs in a module.
    pub fn call_hierarchy_outcome(
        &mut self,
        buffer_id: BufferId,
        position: TextCoordinate,
        direction: Option<legion_protocol::CallHierarchyDirection>,
    ) -> Result<crate::AppCommandOutcome, crate::AppCompositionError> {
        Ok(crate::AppCommandOutcome::language_tooling(
            self.run_call_hierarchy(buffer_id, position, direction)?,
        ))
    }

    /// Ask the server about the symbol under the caret.
    ///
    /// Returns the projection as it stands now. The rows are not in it yet and
    /// cannot be: the answer takes two round trips to the server and arrives on
    /// a later frame through `drain`. What this does synchronously is stamp the
    /// projection with this buffer's identity and record the operation, so the
    /// panel has a row to update when the answer lands rather than one
    /// appearing from nowhere — the same reason the inlay-hint path runs its
    /// index leg knowing the index has nothing to say.
    pub fn run_call_hierarchy(
        &mut self,
        buffer_id: BufferId,
        position: TextCoordinate,
        direction: Option<legion_protocol::CallHierarchyDirection>,
    ) -> Result<legion_protocol::LanguageToolingProjection, crate::AppCompositionError> {
        // The return value decides whether a question was actually asked.
        // Discarding it left `call_hierarchy_awaiting` set by the index leg
        // below even when nothing went out — no live session, or a server that
        // never advertised `callHierarchyProvider` — so the panel sat on
        // "asking…" forever for a question the product had silently declined to
        // ask. That is the inversion of the misreading the flag exists to
        // prevent: instead of an empty answer looking like a conclusion, a
        // conclusion that will never come looks like an answer in flight.
        //
        // The read leg still runs when nothing was issued. Pressing the key is
        // a thing that happened and the operation record says so; what must not
        // survive is the claim that an answer is on its way.
        let issued = self.issue_lsp_prepare_call_hierarchy_request(buffer_id, position, direction);
        let projection = match direction {
            Some(legion_protocol::CallHierarchyDirection::Incoming) => {
                self.run_language_read(buffer_id, crate::LanguageReadKind::IncomingCalls, position)
            }
            Some(legion_protocol::CallHierarchyDirection::Outgoing) => {
                self.run_language_read(buffer_id, crate::LanguageReadKind::OutgoingCalls, position)
            }
            // Prepare-only asks no question a panel can answer, so there is
            // nothing to stamp and nothing to record.
            None => Ok(self.language_tooling_projection()),
        }?;
        if issued {
            return Ok(projection);
        }
        self.language_tooling.cancel_call_hierarchy_wait();
        Ok(self.language_tooling_projection())
    }

    /// Shared tail of both call-hierarchy ingests.
    ///
    /// Both directions produce the same row type and differ only in which
    /// question was asked, so the direction is the one thing that has to be
    /// carried separately.
    fn ingest_call_hierarchy_rows(
        &mut self,
        buffer_id: BufferId,
        rows: Vec<legion_protocol::LanguageLocationProjection>,
        direction: legion_protocol::CallHierarchyDirection,
        request_id: Option<legion_protocol::LspRequestId>,
    ) -> Result<legion_protocol::LanguageToolingProjection, crate::AppCompositionError> {
        let count = rows.len();
        let event_context = self.next_event_context();
        let input = self.language_request_input(buffer_id, event_context)?;
        Ok(self.language_tooling.ingest_lsp_read_projection(
            &input,
            crate::LspReadProjectionIngest {
                kind: match direction {
                    legion_protocol::CallHierarchyDirection::Incoming => {
                        crate::LanguageReadKind::IncomingCalls
                    }
                    legion_protocol::CallHierarchyDirection::Outgoing => {
                        crate::LanguageReadKind::OutgoingCalls
                    }
                },
                call_hierarchy: rows,
                hover: None,
                completions: Vec::new(),
                locations: Vec::new(),
                outline: Vec::new(),
                inlay_hints: Vec::new(),
                code_lenses: Vec::new(),
                request_id,
                message: crate::language::status_message(direction, count),
            },
        ))
    }

    /// Ingest a `callHierarchy/incomingCalls` response into the projection.
    pub fn ingest_lsp_incoming_calls_response_for_buffer(
        &mut self,
        buffer_id: BufferId,
        response: &serde_json::Value,
        request_id: Option<legion_protocol::LspRequestId>,
    ) -> Result<legion_protocol::LanguageToolingProjection, crate::AppCompositionError> {
        let calls = legion_lsp::project_incoming_calls_response(response);
        let rows = crate::language::rows_from_incoming(&calls);
        self.ingest_call_hierarchy_rows(
            buffer_id,
            rows,
            legion_protocol::CallHierarchyDirection::Incoming,
            request_id,
        )
    }

    /// Ingest a `callHierarchy/outgoingCalls` response into the projection.
    pub fn ingest_lsp_outgoing_calls_response_for_buffer(
        &mut self,
        buffer_id: BufferId,
        response: &serde_json::Value,
        request_id: Option<legion_protocol::LspRequestId>,
    ) -> Result<legion_protocol::LanguageToolingProjection, crate::AppCompositionError> {
        let calls = legion_lsp::project_outgoing_calls_response(response);
        let rows = crate::language::rows_from_outgoing(&calls);
        self.ingest_call_hierarchy_rows(
            buffer_id,
            rows,
            legion_protocol::CallHierarchyDirection::Outgoing,
            request_id,
        )
    }

    /// Issues a non-blocking LSP document-symbol request on the worker thread.
    ///
    /// Returns `false` if the session is not Live, or if the server did not
    /// advertise `documentSymbolProvider` in the initialize response.
    pub fn issue_lsp_document_symbol_request(&mut self, buffer_id: BufferId) -> bool {
        self.issue_lsp_read(
            buffer_id,
            "documentSymbolProvider",
            "textDocument/documentSymbol",
            crate::language::LspReadKind::Outline,
            |uri| serde_json::json!({ "textDocument": { "uri": uri } }),
        )
    }

    /// Issues a non-blocking LSP inlay-hint request for a range.
    ///
    /// Returns `false` if the session is not Live, or if the server did not
    /// advertise `inlayHintProvider` in the initialize response.
    ///
    /// The range is the caller's, and should be the visible viewport rather
    /// than the whole document: inlay hints are computed per-range by the
    /// server precisely so that a large file does not have to pay for hints
    /// nobody can see.
    pub fn issue_lsp_inlay_hint_request(
        &mut self,
        buffer_id: BufferId,
        range: legion_protocol::Utf16Range,
    ) -> bool {
        self.issue_lsp_read(
            buffer_id,
            "inlayHintProvider",
            "textDocument/inlayHint",
            crate::language::LspReadKind::InlayHints,
            |uri| {
                serde_json::json!({
                    "textDocument": { "uri": uri },
                    "range": {
                        "start": { "line": range.start.line, "character": range.start.character },
                        "end": { "line": range.end.line, "character": range.end.character }
                    }
                })
            },
        )
    }

    /// Issues a non-blocking LSP code-lens request on the worker thread.
    ///
    /// Returns `false` if the session is not Live, or if the server did not
    /// advertise `codeLensProvider` in the initialize response.
    ///
    /// This is also how runnables reach the editor. rust-analyzer publishes
    /// "Run"/"Debug" as code lenses carrying a `rust-analyzer.runSingle`
    /// command, which is a capability it actually advertises — unlike its
    /// `experimental/runnables` request, which is not in the standard
    /// capability set and so is out of scope by this task's stop condition.
    pub fn issue_lsp_code_lens_request(&mut self, buffer_id: BufferId) -> bool {
        self.issue_lsp_read(
            buffer_id,
            "codeLensProvider",
            "textDocument/codeLens",
            crate::language::LspReadKind::CodeLens,
            |uri| serde_json::json!({ "textDocument": { "uri": uri } }),
        )
    }

    /// The document URI for a buffer, or `None` if it has no metadata.
    fn document_uri_for_buffer(&self, buffer_id: BufferId) -> Option<String> {
        self.active_documents
            .metadata_for_buffer(buffer_id)
            .map(|meta| canonical_path_to_uri(&meta.identity.canonical_path.0))
    }

    fn context_for_unsolicited_apply_edit(
        &mut self,
        params: &serde_json::Value,
    ) -> Option<legion_protocol::LspOperationContext> {
        if self.active_documents.active_workspace_trust
            != Some(legion_protocol::WorkspaceTrustState::Trusted)
        {
            return None;
        }
        let edit = params.get("edit")?;
        let buffer_id = self.buffer_matching_apply_edit(edit)?;
        let snapshot_id = self.editor.current_snapshot(buffer_id).ok()?.snapshot_id;
        let event_context = self.next_event_context();
        self.lsp_operation_context(buffer_id, snapshot_id, event_context)
    }

    fn buffer_matching_apply_edit(&self, edit: &serde_json::Value) -> Option<BufferId> {
        let mut uris = Vec::new();
        if let Some(changes) = edit.get("changes").and_then(serde_json::Value::as_object) {
            uris.extend(changes.keys().cloned());
        }
        if let Some(document_changes) = edit
            .get("documentChanges")
            .and_then(serde_json::Value::as_array)
        {
            for change in document_changes {
                if let Some(uri) = change
                    .get("textDocument")
                    .and_then(|document| document.get("uri"))
                    .and_then(serde_json::Value::as_str)
                {
                    uris.push(uri.to_string());
                }
            }
        }
        for uri in uris {
            let Some(normalized) = crate::normalize_lsp_document_uri(&uri) else {
                continue;
            };
            if let Some(buffer_id) = self.active_documents.open_tabs.iter().copied().find(|id| {
                self.document_uri_for_buffer(*id)
                    .is_some_and(|document| document == normalized)
            }) {
                return Some(buffer_id);
            }
        }
        None
    }

    /// A request input built only so a failure can be recorded against it.
    pub(crate) fn language_request_input_for_failure(
        &mut self,
        buffer_id: BufferId,
    ) -> Option<LanguageRequestInput> {
        let event_context = self.next_event_context();
        self.language_request_input(buffer_id, event_context).ok()
    }

    /// Issues a non-blocking LSP formatting request on the worker thread.
    ///
    /// Returns `false` if the session is not Live, or if the server did not
    /// advertise `documentFormattingProvider` in the initialize response.
    ///
    /// The result becomes a reviewable proposal like every other write-side
    /// action; nothing here writes.
    ///
    /// # Precedence
    ///
    /// A buffer whose language has an explicitly configured external formatter
    /// takes that route instead, because the operator selected that tool
    /// deliberately and a language server's own formatting would silently
    /// override the choice. `route_external_python_formatting` answers `false`
    /// for every other buffer — a non-Python file, or a Python file with no
    /// configured formatter — and the language-server request below then runs
    /// exactly as it did before this route existed.
    pub fn issue_lsp_formatting_request(&mut self, buffer_id: BufferId) -> bool {
        if self.route_external_python_formatting(buffer_id) {
            return true;
        }
        let event_context = self.next_event_context();
        self.issue_lsp_write_read(
            buffer_id,
            "documentFormattingProvider",
            "textDocument/formatting",
            crate::language::LspReadKind::Formatting,
            |uri| {
                serde_json::json!({
                    "textDocument": { "uri": uri },
                    "options": { "tabSize": 4, "insertSpaces": true }
                })
            },
            event_context,
        )
    }

    /// Issues a non-blocking LSP code-action request on the worker thread.
    ///
    /// `organize_imports` narrows the request to `source.organizeImports`,
    /// which is how LSP spells that action — it is a code action with a kind,
    /// not a request of its own.
    ///
    /// Returns `false` if the session is not Live, or if the server did not
    /// advertise `codeActionProvider` in the initialize response.
    pub fn issue_lsp_code_action_request(
        &mut self,
        buffer_id: BufferId,
        range: legion_protocol::Utf16Range,
        organize_imports: bool,
    ) -> bool {
        let event_context = self.next_event_context();
        let Ok(snapshot) = self.editor.current_snapshot(buffer_id) else {
            return false;
        };
        let diagnostics = self.code_action_diagnostics.query_utf16(
            crate::language::DiagnosticIdentity {
                buffer_id,
                snapshot_id: snapshot.snapshot_id,
                buffer_version: snapshot.buffer_version,
            },
            range,
        );
        self.issue_lsp_write_read(
            buffer_id,
            "codeActionProvider",
            "textDocument/codeAction",
            crate::language::LspReadKind::CodeAction { organize_imports },
            |uri| {
                let mut context = serde_json::json!({ "diagnostics": diagnostics });
                if organize_imports {
                    context["only"] = serde_json::json!(["source.organizeImports"]);
                }
                serde_json::json!({
                    "textDocument": { "uri": uri },
                    "range": {
                        "start": { "line": range.start.line, "character": range.start.character },
                        "end": { "line": range.end.line, "character": range.end.character }
                    },
                    "context": context
                })
            },
            event_context,
        )
    }

    /// Issues a non-blocking LSP rename request on the worker thread
    /// (PKT-LSP-C I-2).
    ///
    /// Sends `textDocument/rename` through the live session worker.  The result
    /// arrives via [`LspWorkerResult::ReadResult`] with
    /// [`LspReadKind::Rename { new_name }`] and is ingested by
    /// [`ingest_lsp_rename_result`].
    ///
    /// Returns `false` if the session is not Live, or if the server did not
    /// advertise `renameProvider` in the initialize response (silent skip).
    ///
    /// The resulting proposal enters the `Previewed` state. Call
    /// [`approve_and_apply_rename_proposal`] to transition it through
    /// `Approved` → `Applied` (PKT-APPLY Task 2c).
    pub fn issue_lsp_rename_request(
        &mut self,
        buffer_id: BufferId,
        position: TextCoordinate,
        new_name: String,
    ) -> bool {
        if !self.lsp_server_supports_capability("renameProvider") {
            return false;
        }
        if self.issue_lsp_rename_request_inner(buffer_id, position, new_name.clone()) {
            return true;
        }
        self.defer_lsp_rename_request(buffer_id, position, new_name)
    }

    /// Test-only: issues a rename request bypassing the `renameProvider`
    /// capability gate.  Needed for mock servers that do not advertise
    /// `renameProvider` in their `initialize` response.
    #[cfg(any(test, feature = "test-helpers"))]
    pub fn issue_lsp_rename_request_for_test(
        &mut self,
        buffer_id: BufferId,
        position: TextCoordinate,
        new_name: String,
    ) -> bool {
        self.issue_lsp_rename_request_inner(buffer_id, position, new_name)
    }

    /// Inner rename request sender — shared by the gated and ungated paths.
    pub(crate) fn issue_lsp_rename_request_inner(
        &mut self,
        buffer_id: BufferId,
        position: TextCoordinate,
        new_name: String,
    ) -> bool {
        if self.pending_lsp_writes.len() >= 32 {
            return false;
        }
        let Some(meta) = self
            .active_documents
            .metadata_for_buffer(buffer_id)
            .cloned()
        else {
            return false;
        };
        let Some(workspace_id) = self.active_documents.workspace_id() else {
            return false;
        };
        let Ok(snapshot) = self.editor.current_snapshot(buffer_id) else {
            return false;
        };
        let snapshot_id = snapshot.snapshot_id;
        if !self.lsp_document_sync_ready(buffer_id) || !self.lsp_workspace_sync_ready() {
            return false;
        }
        let uri = canonical_path_to_uri(&meta.identity.canonical_path.0);
        let params = serde_json::json!({
            "textDocument": { "uri": uri },
            "position": { "line": position.line, "character": position.character },
            "newName": new_name,
        });
        let event_context = self.next_event_context();
        let operation_id = Uuid::now_v7().to_string();
        let Some(operation_context) =
            self.lsp_operation_context(buffer_id, snapshot_id, event_context)
        else {
            return false;
        };
        let tag = crate::language::LspRequestTag {
            buffer_id,
            kind: crate::language::LspReadKind::Rename { new_name },
            snapshot_id,
            operation_id: Some(operation_id.clone()),
            operation_context: Some(operation_context),
        };
        if !self
            .lsp_session
            .issue_request("textDocument/rename", params, tag)
        {
            return false;
        }
        self.pending_lsp_writes.insert(
            operation_id.clone(),
            crate::language::PendingLspWriteOperation {
                operation_id: operation_id.clone(),
                operation_kind: LanguageToolingOperationKind::RenameProposal,
                workspace_id,
                file_id: meta.identity.file_id,
                buffer_id,
                snapshot_id,
                event_context,
            },
        );
        let _ = self.language_tooling.upsert_write_operation(
            self.pending_lsp_writes
                .get(&operation_id)
                .expect("inserted rename operation"),
            LanguageToolingStatusKind::Running,
            "LSP rename request accepted".to_string(),
            None,
        );
        true
    }

    fn defer_lsp_rename_request(
        &mut self,
        buffer_id: BufferId,
        position: TextCoordinate,
        new_name: String,
    ) -> bool {
        if new_name.is_empty()
            || new_name.len() > 4_096
            || self.pending_lsp_writes.len() >= 32
            || self
                .deferred_lsp_writes
                .values()
                .any(|deferred| deferred.buffer_id == buffer_id)
            || !self
                .lsp_session
                .health_record()
                .is_some_and(|health| health.init_status == legion_protocol::LspResultStatus::Fresh)
        {
            return false;
        }
        let Ok(snapshot) = self.editor.current_snapshot(buffer_id) else {
            return false;
        };
        let snapshot_id = snapshot.snapshot_id;
        let Some(meta) = self
            .active_documents
            .metadata_for_buffer(buffer_id)
            .cloned()
        else {
            return false;
        };
        let uri = canonical_path_to_uri(&meta.identity.canonical_path.0);
        let event_context = self.next_event_context();
        self.defer_lsp_write_request(
            buffer_id,
            snapshot_id,
            uri.clone(),
            "textDocument/rename",
            crate::language::LspReadKind::Rename {
                new_name: new_name.clone(),
            },
            serde_json::json!({
                "textDocument": { "uri": uri },
                "position": { "line": position.line, "character": position.character },
                "newName": new_name,
            }),
            LanguageToolingOperationKind::RenameProposal,
            event_context,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn defer_lsp_write_request(
        &mut self,
        buffer_id: BufferId,
        snapshot_id: SnapshotId,
        uri: String,
        method: &str,
        kind: crate::language::LspReadKind,
        params: serde_json::Value,
        operation_kind: LanguageToolingOperationKind,
        event_context: crate::EventContext,
    ) -> bool {
        self.defer_lsp_write_request_with_id(
            Uuid::now_v7().to_string(),
            buffer_id,
            snapshot_id,
            uri,
            method,
            kind,
            params,
            operation_kind,
            event_context,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn defer_lsp_write_request_with_id(
        &mut self,
        operation_id: String,
        buffer_id: BufferId,
        snapshot_id: SnapshotId,
        uri: String,
        method: &str,
        kind: crate::language::LspReadKind,
        params: serde_json::Value,
        operation_kind: LanguageToolingOperationKind,
        event_context: crate::EventContext,
    ) -> bool {
        if self.pending_lsp_writes.len() >= 32
            || self
                .deferred_lsp_writes
                .values()
                .any(|deferred| deferred.buffer_id == buffer_id)
            || !self
                .lsp_session
                .health_record()
                .is_some_and(|health| health.init_status == legion_protocol::LspResultStatus::Fresh)
        {
            return false;
        }
        let Some(meta) = self
            .active_documents
            .metadata_for_buffer(buffer_id)
            .cloned()
        else {
            return false;
        };
        let Some(workspace_id) = self.active_documents.workspace_id() else {
            return false;
        };
        let Some(operation_context) =
            self.lsp_operation_context(buffer_id, snapshot_id, event_context)
        else {
            return false;
        };
        let pending = crate::language::PendingLspWriteOperation {
            operation_id: operation_id.clone(),
            operation_kind,
            workspace_id,
            file_id: meta.identity.file_id,
            buffer_id,
            snapshot_id,
            event_context,
        };
        self.pending_lsp_writes
            .insert(operation_id.clone(), pending.clone());
        self.deferred_lsp_writes.insert(
            operation_id,
            DeferredLspWrite {
                buffer_id,
                snapshot_id,
                uri,
                method: method.to_string(),
                kind,
                params,
                operation_context,
            },
        );
        let _ = self.language_tooling.upsert_write_operation(
            &pending,
            LanguageToolingStatusKind::Running,
            "LSP write request waiting for document synchronization".to_string(),
            None,
        );
        true
    }

    fn drain_deferred_lsp_writes(&mut self) {
        let deferred_ids: Vec<String> = self.deferred_lsp_writes.keys().cloned().collect();
        for operation_id in deferred_ids {
            let Some(deferred) = self.deferred_lsp_writes.get(&operation_id).cloned() else {
                continue;
            };
            let Some(pending) = self.pending_lsp_writes.get(&operation_id).cloned() else {
                self.deferred_lsp_writes.remove(&operation_id);
                let _ = self
                    .code_action_authority
                    .remove_resolve_attempt(&operation_id);
                continue;
            };
            let current_snapshot = self.editor.current_snapshot(deferred.buffer_id).ok();
            let current_file = self
                .active_documents
                .metadata_for_buffer(deferred.buffer_id)
                .map(|metadata| metadata.identity.file_id);
            let current_uri = self
                .active_documents
                .metadata_for_buffer(deferred.buffer_id)
                .map(|metadata| canonical_path_to_uri(&metadata.identity.canonical_path.0));
            let current_workspace = self.active_documents.workspace_id();
            if current_snapshot
                .as_ref()
                .is_none_or(|snapshot| snapshot.snapshot_id != deferred.snapshot_id)
                || current_file != Some(pending.file_id)
                || current_uri.as_deref() != Some(deferred.uri.as_str())
                || current_workspace != Some(pending.workspace_id)
            {
                self.deferred_lsp_writes.remove(&operation_id);
                self.pending_lsp_writes.remove(&operation_id);
                let _ = self
                    .code_action_authority
                    .remove_resolve_attempt(&operation_id);
                let _ = self.language_tooling.upsert_write_operation(
                    &pending,
                    LanguageToolingStatusKind::Stale,
                    "deferred LSP write became stale before document synchronization".to_string(),
                    None,
                );
                continue;
            }
            if !self.lsp_workspace_sync_ready() || !self.lsp_document_sync_ready(deferred.buffer_id)
            {
                continue;
            }
            let tag = crate::language::LspRequestTag {
                buffer_id: deferred.buffer_id,
                kind: deferred.kind.clone(),
                snapshot_id: deferred.snapshot_id,
                operation_id: Some(operation_id.clone()),
                operation_context: Some(deferred.operation_context.clone()),
            };
            if self
                .lsp_session
                .issue_request(&deferred.method, deferred.params, tag)
            {
                self.deferred_lsp_writes.remove(&operation_id);
                if matches!(
                    deferred.kind,
                    crate::language::LspReadKind::CodeActionExecuteCommand { .. }
                ) {
                    self.pending_code_action_contexts.insert(
                        deferred.operation_context.request_id.0.to_string(),
                        crate::language::PendingLspCommandContext {
                            context: deferred.operation_context.clone(),
                            operation_kind: pending.operation_kind,
                        },
                    );
                }
                let _ = self.language_tooling.upsert_write_operation(
                    &pending,
                    LanguageToolingStatusKind::Running,
                    "LSP write request accepted after document synchronization".to_string(),
                    None,
                );
            }
        }
    }
}

/// What a write-side result should become once it has been translated.
///
/// The four write-side actions differ only in what they are called and which
/// proposal kind they are recorded as; the WorkspaceEdit → translate →
/// validate → preview path is identical, and duplicating it once per action is
/// how one of them quietly stops registering its proposal.
pub(crate) struct LspWriteSideSpec {
    pub(crate) proposal_kind: LanguageProposalKind,
    pub(crate) source_kind: WorkspaceEditSourceKind,
    pub(crate) title: String,
    pub(crate) detail_tag: &'static str,
    pub(crate) detail_extra: Vec<String>,
    pub(crate) command: Option<(String, serde_json::Value)>,
}

#[cfg(test)]
mod diagnostics_tests {
    #[test]
    fn versionless_push_is_ordered_and_version_mismatch_is_stale() {
        assert!(super::AppComposition::diagnostic_version_is_current(
            &serde_json::json!({"uri": "file:///tmp/a.ts"}),
            4
        ));
        assert!(super::AppComposition::diagnostic_version_is_current(
            &serde_json::json!({"version": 4}),
            4
        ));
        assert!(!super::AppComposition::diagnostic_version_is_current(
            &serde_json::json!({"version": 3}),
            4
        ));
    }
}
