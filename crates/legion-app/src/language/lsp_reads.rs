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
                let pending = self.pending_call_hierarchy.take();
                let items =
                    legion_lsp::project_prepare_call_hierarchy_response(&lsp_outcome.result)
                        .unwrap_or_default();
                let Some(pending) = pending else {
                    return;
                };
                if pending.buffer_id != tag.buffer_id {
                    // The caret moved to another buffer between the two
                    // requests. Answering with this buffer's symbol would put a
                    // list under a heading it does not belong to.
                    return;
                }
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
                };
                self.ingest_lsp_write_side_result(tag.buffer_id, spec, &lsp_outcome.result);
            }
            LspReadKind::Formatting => {
                // A formatting response is a bare `TextEdit[]` for one
                // document. The proposal path speaks WorkspaceEdit, so the
                // edits are lifted into one — the same shape a rename arrives
                // in, so the same translation, preconditions and review apply.
                let Some(uri) = self.document_uri_for_buffer(tag.buffer_id) else {
                    return;
                };
                let edit = serde_json::json!({ "changes": { uri: lsp_outcome.result } });
                let spec = LspWriteSideSpec {
                    proposal_kind: LanguageProposalKind::Formatting,
                    source_kind: WorkspaceEditSourceKind::LspFormatting,
                    title: "Format document".to_string(),
                    detail_tag: "language_tooling.lsp_formatting",
                    detail_extra: Vec::new(),
                };
                self.ingest_lsp_write_side_result(tag.buffer_id, spec, &edit);
            }
            LspReadKind::CodeAction { organize_imports } => {
                let (proposal_kind, source_kind, title, detail_tag) = if organize_imports {
                    (
                        LanguageProposalKind::OrganizeImports,
                        // Organize-imports *is* a code action; the source kind
                        // says where the edit came from, and it came from one.
                        WorkspaceEditSourceKind::LspCodeAction,
                        "Organize imports".to_string(),
                        "language_tooling.lsp_organize_imports",
                    )
                } else {
                    (
                        LanguageProposalKind::CodeAction,
                        WorkspaceEditSourceKind::LspCodeAction,
                        "Apply code action".to_string(),
                        "language_tooling.lsp_code_action",
                    )
                };
                let input = self.language_request_input_for_failure(tag.buffer_id);
                match first_code_action_edit(&lsp_outcome.result) {
                    Some(edit) => {
                        let spec = LspWriteSideSpec {
                            proposal_kind,
                            source_kind,
                            title,
                            detail_tag,
                            detail_extra: Vec::new(),
                        };
                        self.ingest_lsp_write_side_result(tag.buffer_id, spec, &edit);
                    }
                    None => {
                        // A code action carrying only a `command` needs
                        // `workspace/executeCommand` to run, which would let a
                        // server mutate the workspace outside the proposal
                        // pipeline. Refused, and said out loud, rather than
                        // silently producing nothing.
                        if let Some(input) = input {
                            let _ = self.language_tooling.record_proposal_failure(
                                &input,
                                proposal_kind,
                                "no code action with a workspace edit; command-only \
                                 actions are not executed"
                                    .to_string(),
                            );
                        }
                    }
                }
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
    fn ingest_lsp_write_side_result(
        &mut self,
        buffer_id: BufferId,
        spec: LspWriteSideSpec,
        raw: &serde_json::Value,
    ) {
        use crate::language::translate_workspace_edit;

        let LspWriteSideSpec {
            proposal_kind,
            source_kind,
            title,
            detail_tag,
            detail_extra,
        } = spec;

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

        let capability = CapabilityId("fs.write".to_string());

        // Build the production DocumentResolver from the current open-buffer state.
        let resolver = AppDocumentResolver::build(&self.active_documents, &self.editor);

        let workspace_edit = match translate_workspace_edit(
            raw,
            &resolver,
            workspace_id,
            source_kind,
            title.clone(),
            capability.clone(),
        ) {
            Ok(payload) => payload,
            Err(err) => {
                let _ = self.language_tooling.record_proposal_failure(
                    &input,
                    proposal_kind,
                    format!("{title}: LSP translation failed: {err}"),
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
                let _ = self.language_tooling.record_proposal_failure(
                    &input,
                    proposal_kind,
                    format!("{title}: proposal creation failed: {err:?}"),
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
                proposal_kind,
                format!("{title}: proposal coordinator rejected: {created:?}"),
            );
            return;
        }
        let validated = self
            .proposal_coordinator
            .handle(ProposalRequest::Validate(proposal.clone()));
        if !matches!(validated, Ok(ProposalResponse::Validated(_))) {
            let _ = self.language_tooling.record_proposal_failure(
                &input,
                proposal_kind,
                format!("{title}: proposal validation failed: {validated:?}"),
            );
            return;
        }
        let previewed = self
            .proposal_coordinator
            .handle(ProposalRequest::Preview(proposal.clone()));
        if !matches!(previewed, Ok(ProposalResponse::Previewed { .. })) {
            let _ = self.language_tooling.record_proposal_failure(
                &input,
                proposal_kind,
                format!("{title}: proposal preview failed: {previewed:?}"),
            );
            return;
        }
        let _ = self.language_tooling.record_proposal(
            &input,
            proposal_kind,
            proposal.proposal_id,
            None,
            format!("{title}: proposal generated"),
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
        self.lsp_session
            .set_live_with_result_sender_for_test(health)
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
        let tag = crate::language::LspRequestTag {
            buffer_id,
            kind,
            snapshot_id,
        };
        self.lsp_session.issue_request(method, params(&uri), tag)
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
        self.pending_call_hierarchy = issued.then(|| crate::language::PendingCallHierarchy {
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
        self.issue_lsp_prepare_call_hierarchy_request(buffer_id, position, direction);
        match direction {
            Some(legion_protocol::CallHierarchyDirection::Incoming) => {
                self.run_language_read(buffer_id, crate::LanguageReadKind::IncomingCalls, position)
            }
            Some(legion_protocol::CallHierarchyDirection::Outgoing) => {
                self.run_language_read(buffer_id, crate::LanguageReadKind::OutgoingCalls, position)
            }
            // Prepare-only asks no question a panel can answer, so there is
            // nothing to stamp and nothing to record.
            None => Ok(self.language_tooling_projection()),
        }
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

    /// A request input built only so a failure can be recorded against it.
    fn language_request_input_for_failure(
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
    pub fn issue_lsp_formatting_request(&mut self, buffer_id: BufferId) -> bool {
        self.issue_lsp_read(
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
        self.issue_lsp_read(
            buffer_id,
            "codeActionProvider",
            "textDocument/codeAction",
            crate::language::LspReadKind::CodeAction { organize_imports },
            |uri| {
                let mut context = serde_json::json!({ "diagnostics": [] });
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

/// What a write-side result should become once it has been translated.
///
/// The four write-side actions differ only in what they are called and which
/// proposal kind they are recorded as; the WorkspaceEdit → translate →
/// validate → preview path is identical, and duplicating it once per action is
/// how one of them quietly stops registering its proposal.
struct LspWriteSideSpec {
    proposal_kind: LanguageProposalKind,
    source_kind: WorkspaceEditSourceKind,
    title: String,
    detail_tag: &'static str,
    detail_extra: Vec<String>,
}

/// The first workspace edit offered by a code-action response.
///
/// A `CodeAction[]` may mix actions carrying an `edit` with actions carrying
/// only a `command`. Only the former can go through the proposal pipeline, so
/// only the former is taken; the rest are the caller's problem to report.
fn first_code_action_edit(response: &serde_json::Value) -> Option<serde_json::Value> {
    response
        .as_array()?
        .iter()
        .find_map(|action| action.get("edit").cloned())
}
