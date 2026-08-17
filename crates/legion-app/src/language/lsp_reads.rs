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

impl AppComposition {
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
        let changed = self.lsp_session.drain();
        // Drain any completed worker results (non-blocking).
        let results = self.lsp_session.try_drain_results();
        for result in results {
            use crate::language::LspWorkerResult;
            match result {
                LspWorkerResult::ReadResult { outcome, tag } => {
                    self.ingest_lsp_worker_result(outcome, tag);
                }
                LspWorkerResult::DiagnosticBatch { raw_params } => {
                    self.ingest_lsp_diagnostic_batch(raw_params);
                }
                LspWorkerResult::TransportDead { .. } => {
                    // Intercepted inside `LspSessionHandle::try_drain_results`
                    // (routed through the restart circuit breaker); it never
                    // reaches this dispatch.  Arm kept for exhaustiveness.
                }
            }
        }
        changed
    }

    /// Ingests a completed LSP read-request result from the worker thread.
    fn ingest_lsp_worker_result(
        &mut self,
        outcome: Result<crate::language::LspReadOutcome, crate::language::LanguageSessionError>,
        tag: crate::language::LspRequestTag,
    ) {
        use crate::language::{LspReadKind, is_stale_response};
        let Ok(lsp_outcome) = outcome else {
            return; // session error; ignore
        };
        // Stale-response gate: discard if snapshot moved on since the request.
        if let Ok(current_snapshot) = self.editor.current_snapshot(tag.buffer_id)
            && is_stale_response(lsp_outcome.issued_snapshot, current_snapshot.snapshot_id)
        {
            return;
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
            LspReadKind::Rename { new_name } => {
                self.ingest_lsp_rename_result(tag.buffer_id, new_name, &lsp_outcome.result);
            }
        }
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
    fn ingest_lsp_rename_result(
        &mut self,
        buffer_id: BufferId,
        new_name: String,
        raw: &serde_json::Value,
    ) {
        use crate::language::translate_workspace_edit;

        let Some(workspace_id) = self.active_documents.workspace_id() else {
            return;
        };
        let Some(meta) = self
            .active_documents
            .metadata_for_buffer(buffer_id)
            .cloned()
        else {
            return;
        };
        let principal = self
            .active_documents
            .active_principal_id
            .clone()
            .unwrap_or_else(|| PrincipalId("system".to_string()));

        let event_context = self.next_event_context();
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
        };

        let title = format!("Rename symbol to {}", bounded_label(&new_name, 64));
        let capability = CapabilityId("fs.write".to_string());

        // Build the production DocumentResolver from the current open-buffer state.
        let resolver = AppDocumentResolver::build(&self.active_documents, &self.editor);

        let workspace_edit = match translate_workspace_edit(
            raw,
            &resolver,
            workspace_id,
            WorkspaceEditSourceKind::LspRename,
            title.clone(),
            capability.clone(),
        ) {
            Ok(payload) => payload,
            Err(err) => {
                let _ = self.language_tooling.record_proposal_failure(
                    &input,
                    LanguageProposalKind::Rename,
                    format!("LSP rename translation failed: {err}"),
                );
                return;
            }
        };

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
                    details: vec![
                        "language_tooling.lsp_rename".to_string(),
                        format!("new_name={}", bounded_label(&new_name, 64)),
                    ],
                },
                expires_at: None,
                created_at: TimestampMillis::now(),
                diagnostics: Vec::new(),
                schema_version: 1,
            },
        ) {
            Ok(p) => p,
            Err(err) => {
                let _ = self.language_tooling.record_proposal_failure(
                    &input,
                    LanguageProposalKind::Rename,
                    format!("rename proposal creation failed: {err:?}"),
                );
                return;
            }
        };

        self.proposal_coordinator
            .register_lifecycle_context(proposal.proposal_id, input.event_context);
        let created = self.proposal_coordinator.created_response(&proposal);
        if !matches!(created, ProposalResponse::Created(_)) {
            let _ = self.language_tooling.record_proposal_failure(
                &input,
                LanguageProposalKind::Rename,
                format!("rename proposal coordinator rejected: {created:?}"),
            );
            return;
        }
        let validated = self
            .proposal_coordinator
            .handle(ProposalRequest::Validate(proposal.clone()));
        if !matches!(validated, Ok(ProposalResponse::Validated(_))) {
            let _ = self.language_tooling.record_proposal_failure(
                &input,
                LanguageProposalKind::Rename,
                format!("rename proposal validation failed: {validated:?}"),
            );
            return;
        }
        let previewed = self
            .proposal_coordinator
            .handle(ProposalRequest::Preview(proposal.clone()));
        if !matches!(previewed, Ok(ProposalResponse::Previewed { .. })) {
            let _ = self.language_tooling.record_proposal_failure(
                &input,
                LanguageProposalKind::Rename,
                format!("rename proposal preview failed: {previewed:?}"),
            );
            return;
        }
        let _ = self.language_tooling.record_proposal(
            &input,
            LanguageProposalKind::Rename,
            proposal.proposal_id,
            None,
            format!(
                "Rename proposal generated ({})",
                bounded_label(&new_name, 64)
            ),
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

        self.apply_workspace_proposal(proposal)
    }

    /// Ingests a raw `publishDiagnostics` notification batch from the worker.
    fn ingest_lsp_diagnostic_batch(&mut self, raw_params: serde_json::Value) {
        // Extract URI from params to look up the buffer.
        let Some(uri) = raw_params.get("uri").and_then(|v| v.as_str()) else {
            return;
        };
        // Convert file URI to a canonical path for lookup.
        let canonical_path = uri_to_canonical_path(uri);
        // Find the buffer_id for this URI.
        let Some(buffer_id) = self.active_documents.buffer_id_for_path(&canonical_path) else {
            return; // Not an open buffer; ignore (uri-filtered).
        };
        // Project + ingest through redaction layer.
        let _ = self.ingest_lsp_publish_diagnostics_for_buffer(buffer_id, &raw_params, true, None);
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
        let uri = canonical_path_to_uri(&meta.identity.canonical_path.0);
        let params = serde_json::json!({
            "textDocument": { "uri": uri },
            "position": { "line": position.line, "character": position.character }
        });
        let tag = crate::language::LspRequestTag {
            buffer_id,
            kind: crate::language::LspReadKind::Completion,
            snapshot_id: snapshot.snapshot_id,
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
        let uri = canonical_path_to_uri(&meta.identity.canonical_path.0);
        let params = serde_json::json!({
            "textDocument": { "uri": uri },
            "position": { "line": position.line, "character": position.character }
        });
        let tag = crate::language::LspRequestTag {
            buffer_id,
            kind: crate::language::LspReadKind::Hover,
            snapshot_id: snapshot.snapshot_id,
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
        let uri = canonical_path_to_uri(&meta.identity.canonical_path.0);
        let params = serde_json::json!({
            "textDocument": { "uri": uri },
            "position": { "line": position.line, "character": position.character }
        });
        let tag = crate::language::LspRequestTag {
            buffer_id,
            kind: crate::language::LspReadKind::Definition,
            snapshot_id: snapshot.snapshot_id,
        };
        self.lsp_session
            .issue_request("textDocument/definition", params, tag)
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
        self.issue_lsp_rename_request_inner(buffer_id, position, new_name)
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
        let uri = canonical_path_to_uri(&meta.identity.canonical_path.0);
        let params = serde_json::json!({
            "textDocument": { "uri": uri },
            "position": { "line": position.line, "character": position.character },
            "newName": new_name,
        });
        let tag = crate::language::LspRequestTag {
            buffer_id,
            kind: crate::language::LspReadKind::Rename { new_name },
            snapshot_id: snapshot.snapshot_id,
        };
        self.lsp_session
            .issue_request("textDocument/rename", params, tag)
    }
}
