#[path = "tests/workspace_edit_conflict_tests.rs"]
mod workspace_edit_conflict_tests;

use super::*;
use legion_protocol::{WorkspaceEditAnnotationTarget, WorkspaceEditChangeAnnotation};
use std::fs;
use std::path::PathBuf;
#[cfg(feature = "ai")]
use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicUsize, Ordering},
};
use std::time::{SystemTime, UNIX_EPOCH};

#[test]
fn retryable_search_palette_rows_dispatch_the_existing_run_search_intent() {
    for status_kind in [
        SearchStatusKindProjection::NoResults,
        SearchStatusKindProjection::ValidationError,
        SearchStatusKindProjection::Error,
        SearchStatusKindProjection::Cancelled,
        SearchStatusKindProjection::DegradedLimited,
    ] {
        let mut app = AppComposition::new();
        app.palette.open = true;
        app.palette.mode = PaletteMode::Search;
        app.palette.query = "/needle".to_string();
        app.palette.scope = SearchScopeProjection::Workspace;
        app.search_projection = SearchProjection {
            query_id: Some("search:failed".to_string()),
            scope: SearchScopeProjection::Workspace,
            query_label: "needle".to_string(),
            status: SearchStatusProjection {
                kind: status_kind,
                message: "Retryable search state".to_string(),
            },
            results: Vec::new(),
            result_limit: 20,
            omitted_result_count: 0,
            omitted_file_count: 0,
            skipped_binary_count: 0,
            case_sensitive: false,
            whole_word: false,
            use_regex: false,
            diagnostics: Vec::new(),
            generated_at: TimestampMillis(1),
            schema_version: 1,
        };

        app.sync_search_palette_results();

        assert_eq!(app.palette.results.len(), 1, "{status_kind:?}");
        let retry = &app.palette.results[0];
        assert_eq!(retry.id, "search:retry", "{status_kind:?}");
        assert_eq!(retry.disabled_reason, None, "{status_kind:?}");
        assert_eq!(
            app.palette_result_intent(retry),
            Some(CommandDispatchIntent::RunSearch {
                scope: SearchScopeProjection::Workspace,
                query: "needle".to_string(),
                limit: 0,
                case_sensitive: None,
                whole_word: None,
                use_regex: None,
            }),
            "{status_kind:?}"
        );
    }
}

#[test]
fn workspace_edit_change_annotations_project_as_review_warnings() {
    let payload = ProposalPayload::WorkspaceEdit(WorkspaceEditProposalPayload {
        workspace_id: WorkspaceId(1),
        edit_id: uuid::Uuid::from_u128(1),
        title: "annotated edit".to_string(),
        source: WorkspaceEditSourceKind::LspRename,
        target_coverage: ProposalTargetCoverage {
            coverage_kind: ProposalTargetCoverageKind::Complete,
            targets: Vec::new(),
            omitted_target_count: 0,
            redaction_hints: Vec::new(),
        },
        file_edits: Vec::new(),
        file_operations: Vec::new(),
        change_annotations: vec![WorkspaceEditChangeAnnotation {
            id: "rename-default".to_string(),
            label: "Rename symbol".to_string(),
            description: Some("Update definition and references".to_string()),
            needs_confirmation: true,
            targets: vec![WorkspaceEditAnnotationTarget::TextEdit {
                file_edit_index: 0,
                edit_index: 0,
            }],
        }],
        required_capability: CapabilityId("fs.write".to_string()),
        diagnostics: Vec::new(),
        schema_version: 1,
    });

    let warnings = AppProposalCoordinator::preview_warnings(&payload);
    assert_eq!(warnings.len(), 1);
    assert_eq!(
        warnings[0].kind,
        ProposalPreviewWarningKind::ChangeAnnotation
    );
    assert_eq!(warnings[0].target_id.as_deref(), Some("rename-default"));
    assert!(warnings[0].message.contains("Confirmation required"));
    assert!(
        warnings[0]
            .message
            .contains("Update definition and references")
    );
}

#[test]
fn search_palette_does_not_restore_results_from_a_different_scope() {
    let mut app = AppComposition::new();
    app.search_projection = SearchProjection {
        query_id: Some("search:workspace".to_string()),
        scope: SearchScopeProjection::Workspace,
        query_label: "needle".to_string(),
        status: SearchStatusProjection {
            kind: SearchStatusKindProjection::NoResults,
            message: "No workspace matches".to_string(),
        },
        results: Vec::new(),
        result_limit: 20,
        omitted_result_count: 0,
        omitted_file_count: 0,
        skipped_binary_count: 0,
        case_sensitive: false,
        whole_word: false,
        use_regex: false,
        diagnostics: Vec::new(),
        generated_at: TimestampMillis(1),
        schema_version: 1,
    };

    let palette = app
        .open_palette(
            PaletteMode::Search,
            "needle".to_string(),
            SearchScopeProjection::ActiveFile,
        )
        .expect("active-file Search should open");

    assert_eq!(palette.scope, SearchScopeProjection::ActiveFile);
    assert_eq!(palette.results.len(), 1);
    assert_eq!(palette.results[0].id, "search:run");
    assert_eq!(
        palette.results[0].title,
        "Search active file for \"needle\""
    );
}

#[cfg(feature = "ai")]
#[test]
fn workflow_provider_boundary_denies_workspace_read_capabilities() {
    let broker = LegionWorkflowProposalOnlyCapabilityBroker;
    let decision_for = |capability: &str| {
        broker
            .handle(CapabilityRequest::Request {
                principal_id: PrincipalId("workflow-provider-test".to_string()),
                capability_id: CapabilityId(capability.to_string()),
                workspace_trust_state: WorkspaceTrustState::Trusted,
                target_path: None,
                decision_id: None,
                context: CapabilityRequestContext::default(),
                correlation_id: CorrelationId(1),
            })
            .expect("capability decision")
    };

    assert!(matches!(
        decision_for("delegate.tool.read"),
        CapabilityResponse::Decision(CapabilityDecision { granted: false, .. })
    ));
    assert!(matches!(
        decision_for("delegate.tool.grep"),
        CapabilityResponse::Decision(CapabilityDecision { granted: false, .. })
    ));
    assert!(matches!(
        decision_for("delegate.tool.edit-as-proposal"),
        CapabilityResponse::Decision(CapabilityDecision { granted: true, .. })
    ));

    let scope = AppComposition::new().legion_workflow_worker_scope();
    assert_eq!(
        scope.allowed_tools,
        vec![legion_protocol::LegionToolKind::EditAsProposal]
    );
}

#[test]
fn parse_terminal_keeps_unterminated_osc_bytes() {
    // OSC introducer with no BEL/ST terminator must not silently drop the
    // trailing bytes of the output.
    let payload = "before\x1b]7;file://localhost/home";
    let parsed = legion_terminal::osc::parse_terminal_shell_output(payload);
    assert_eq!(parsed.visible_output, "before\x1b]7;file://localhost/home");
    assert_eq!(parsed.cwd, None);
}

#[test]
fn parse_terminal_handles_terminated_osc() {
    let payload = "out\x1b]7;file:///home/user\x07tail";
    let parsed = legion_terminal::osc::parse_terminal_shell_output(payload);
    assert_eq!(parsed.visible_output, "outtail");
    assert_eq!(parsed.cwd.as_deref(), Some("/home/user"));
}

#[test]
fn osc7_cwd_decodes_windows_drive_and_percent() {
    assert_eq!(
        legion_terminal::osc::parse_terminal_shell_output(
            "\x1b]7;file:///C:/Users/My%20Project\x1b\\"
        )
        .cwd
        .as_deref(),
        Some("C:/Users/My Project")
    );
}

#[test]
fn osc7_cwd_handles_localhost_and_unc() {
    assert_eq!(
        legion_terminal::osc::parse_terminal_shell_output(
            "\x1b]7;file://localhost/home/user\x1b\\"
        )
        .cwd
        .as_deref(),
        Some("/home/user")
    );
    assert_eq!(
        legion_terminal::osc::parse_terminal_shell_output("\x1b]7;file://server/share/dir\x1b\\")
            .cwd
            .as_deref(),
        Some("//server/share/dir")
    );
}

fn unique_temp_dir(prefix: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time should be after epoch")
        .as_nanos();
    let path = std::env::temp_dir().join(format!("legion-{prefix}-{nanos}"));
    fs::create_dir_all(&path).expect("create temp root");
    path
}

fn save_proposal(proposal_id: ProposalId) -> WorkspaceProposal {
    let file = FileIdentity {
        file_id: FileId(1),
        workspace_id: WorkspaceId(1),
        canonical_path: CanonicalPath("C:/repo/file.txt".to_string()),
        content_version: FileContentVersion(1),
        content_hash: None,
    };
    let fingerprint = FileFingerprint {
        algorithm: "test".to_string(),
        value: "hash:test".to_string(),
    };
    WorkspaceProposal {
        proposal_id,
        principal: PrincipalId("trusted".to_string()),
        capability: CapabilityId("fs.write".to_string()),
        correlation_id: CorrelationId(1),
        payload: ProposalPayload::SaveFile(SaveFileProposal {
            file: file.clone(),
            buffer_id: BufferId(1),
            file_id: file.file_id,
            snapshot_id: legion_protocol::SnapshotId(1),
            buffer_version: legion_protocol::BufferVersion(1),
            file_content_version: FileContentVersion(1),
            workspace_generation: WorkspaceGeneration(1),
            expected_fingerprint: Some(fingerprint.clone()),
            save_intent: SaveIntent::Manual,
            conflict_policy: SaveConflictPolicy::RejectIfChanged,
            trust_decision: TrustDecisionContext {
                workspace_trust_state: WorkspaceTrustState::Trusted,
                decision_id: None,
                decided_at: Some(TimestampMillis(1)),
            },
            required_capability: CapabilityId("fs.write".to_string()),
            principal: PrincipalId("trusted".to_string()),
            correlation_id: CorrelationId(1),
            diagnostics: Vec::new(),
        }),
        preconditions: ProposalVersionPreconditions {
            file_version: Some(FileContentVersion(1)),
            buffer_version: Some(legion_protocol::BufferVersion(1)),
            snapshot_id: Some(legion_protocol::SnapshotId(1)),
            generation: Some(WorkspaceGeneration(1)),
            file_content_version: Some(FileContentVersion(1)),
            workspace_generation: Some(WorkspaceGeneration(1)),
            expected_fingerprint: Some(fingerprint),
            expected_file_length: None,
            expected_modified_at: None,
        },
        preview: PreviewSummary {
            summary: "test save".to_string(),
            details: Vec::new(),
        },
        expires_at: None,
        created_at: TimestampMillis(1),
    }
}

fn command(
    proposal_id: ProposalId,
    action: legion_protocol::ProposalLifecycleAction,
) -> ProposalLifecycleCommand {
    ProposalLifecycleCommand {
        proposal_id,
        action,
        principal: PrincipalId("trusted".to_string()),
        capability: CapabilityId("fs.write".to_string()),
        correlation_id: CorrelationId(1),
        causality_id: CausalityId(uuid::Uuid::now_v7()),
        reason: None,
        diagnostics: Vec::new(),
        requested_at: TimestampMillis(1),
        schema_version: 1,
    }
}

fn register_created(coordinator: &AppProposalCoordinator, proposal: &WorkspaceProposal) {
    coordinator
        .register_lifecycle_context(proposal.proposal_id, EventContext::new(CorrelationId(1)));
    assert!(matches!(
        coordinator.created_response(proposal),
        ProposalResponse::Created(_)
    ));
}

fn proposal_intent_route_context(
    proposal: Option<WorkspaceProposal>,
) -> AppProposalIntentRouteContext {
    AppProposalIntentRouteContext {
        proposal,
        principal: PrincipalId("trusted".to_string()),
        capability: CapabilityId("fs.write".to_string()),
        correlation_id: CorrelationId(99),
        causality_id: CausalityId(
            uuid::Uuid::parse_str("11111111-1111-1111-1111-111111111111").unwrap(),
        ),
        requested_at: TimestampMillis(123),
    }
}

fn plugin_manifest(plugin_id: PluginId) -> PluginManifest {
    PluginManifest {
        plugin_id,
        name: "phase5.test".to_string(),
        version: "0.1.0".to_string(),
        schema_version: 1,
        min_abi_version: 1,
        max_abi_version: 1,
        module_hash: "sha256:phase5".to_string(),
        manifest_id: "manifest:phase5".to_string(),
        trust: legion_protocol::PluginTrustMetadata {
            source: legion_protocol::PluginTrustSource::ExplicitLocalAllow,
            decision: legion_protocol::PluginTrustDecision::ExplicitlyAllowed,
            reason: "test allow".to_string(),
        },
        signature: None,
        activation_events: vec![legion_protocol::PluginActivationEvent::OnCommand {
            command: "phase5.run".to_string(),
        }],
        contributions: vec![legion_protocol::PluginContribution::Command(
            legion_protocol::PluginCommandDescriptor {
                command_id: "phase5.run".to_string(),
                title: "Phase 5 Run".to_string(),
                required_capability: CapabilityId("plugin.command".to_string()),
            },
        )],
        requested_capabilities: vec![CapabilityId("plugin.command".to_string())],
        storage_namespace: legion_plugin::plugin_namespace(plugin_id, "state"),
        quotas: legion_protocol::PluginQuotaDeclaration {
            max_fuel: 1000,
            max_wall_time_ms: 50,
            max_memory_pages: 8,
            max_storage_bytes: 4096,
            max_host_calls: 4,
            max_events: 4,
            max_output_bytes: 128,
        },
    }
}

#[test]
fn instruction_prefix_bundle_collects_workspace_and_user_layers_in_order() {
    let workspace_root = unique_temp_dir("workspace");
    let workspace_legion_rules = workspace_root.join(".legion/rules");
    let user_root = unique_temp_dir("user-home");
    let user_legion_rules = user_root.join(".legion/rules");

    fs::create_dir_all(&workspace_legion_rules).expect("create workspace rules dir");
    fs::create_dir_all(&user_legion_rules).expect("create user rules dir");
    fs::write(
        workspace_root.join("AGENTS.md"),
        "workspace agent line 1\nworkspace agent line 2\n",
    )
    .expect("write workspace AGENTS.md");
    fs::write(
        workspace_legion_rules.join("b-rule.md"),
        "workspace rule b\n",
    )
    .expect("write workspace rule b");
    fs::write(
        workspace_legion_rules.join("a-rule.md"),
        "workspace rule a\n",
    )
    .expect("write workspace rule a");
    fs::write(user_legion_rules.join("user-rule.md"), "user rule\n").expect("write user rule");

    let bundle = instruction_prefix_bundle(
        WorkspaceId(11),
        TimestampMillis(1),
        Some(workspace_root.as_path()),
        Some(user_root.as_path()),
    );

    assert!(
        bundle
            .prompt_prefix
            .starts_with("source=workspace-agents\npath=")
    );
    assert!(bundle.prompt_prefix.contains("workspace agent line 1"));
    assert!(bundle.prompt_prefix.contains("workspace rule a"));
    assert!(bundle.prompt_prefix.contains("workspace rule b"));
    assert!(bundle.prompt_prefix.contains("user rule"));
    assert_eq!(bundle.manifest_items.len(), 4);
    assert_eq!(
        bundle.manifest_items[0]
            .path
            .as_ref()
            .map(|path| path.0.as_str()),
        Some(
            workspace_root
                .join("AGENTS.md")
                .to_str()
                .expect("workspace path text")
        )
    );
    assert!(
        bundle.manifest_items[0]
            .labels
            .iter()
            .any(|label| label == "phase4.context.instruction_source")
    );
    assert_eq!(
        bundle.manifest_items[3]
            .path
            .as_ref()
            .map(|path| path.0.as_str()),
        Some(
            user_legion_rules
                .join("user-rule.md")
                .to_str()
                .expect("user path text")
        )
    );

    let _ = fs::remove_dir_all(&workspace_root);
    let _ = fs::remove_dir_all(&user_root);
}

fn assert_transition_diagnostic(response: &ProposalResponse, expected_code: &str) {
    let diagnostics = match response {
        ProposalResponse::Created(transition)
        | ProposalResponse::Validated(transition)
        | ProposalResponse::Approved(transition)
        | ProposalResponse::Applied(transition) => &transition.diagnostics,
        ProposalResponse::Previewed { transition, .. } => &transition.diagnostics,
        ProposalResponse::Rejected { transition, .. }
        | ProposalResponse::Denied { transition, .. }
        | ProposalResponse::Failed { transition, .. }
        | ProposalResponse::RolledBack { transition, .. }
        | ProposalResponse::Stale { transition, .. }
        | ProposalResponse::Conflict { transition, .. }
        | ProposalResponse::Cancelled { transition, .. } => &transition.diagnostics,
    };

    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == expected_code),
        "expected diagnostic {expected_code}, got {diagnostics:?}"
    );
}

#[test]
fn rust_tree_sitter_overlay_pipeline_returns_keyword_function_and_string_tokens() {
    let text = "pub fn demo() {\n    let s = \"hi\";\n}\n";
    let line_slices = logical_lines_with_offsets(text)
        .into_iter()
        .map(|(line_number, start_byte, line)| ViewportLineSlice {
            line_number,
            visible_text: line.to_string(),
            byte_range: ByteRange {
                start: start_byte as u64,
                end: start_byte.saturating_add(line.len()) as u64,
            },
            utf16_range: legion_protocol::Utf16Range {
                start: legion_protocol::Utf16Position {
                    line: line_number,
                    character: 0,
                },
                end: legion_protocol::Utf16Position {
                    line: line_number,
                    character: line.encode_utf16().count() as u32,
                },
            },
            chunk_hash: FileFingerprint {
                algorithm: "test".to_string(),
                value: format!("line:{line_number}"),
            },
            truncation_state: legion_protocol::ViewportLineTruncationState::None,
        })
        .collect::<Vec<_>>();

    let overlays = tree_sitter_semantic_token_overlays_for_visible_lines(
        "/workspace/src/highlights.rs",
        &line_slices,
        Some(text),
    )
    .expect("rust full-text input should use tree-sitter overlays");
    let cache_key = tree_sitter_overlay_cache_key("/workspace/src/highlights.rs", text);
    assert!(
        tree_sitter_overlay_cache_guard().get(&cache_key).is_some(),
        "tree-sitter highlight captures should be cached by content hash"
    );
    assert_ne!(
        cache_key,
        tree_sitter_overlay_cache_key("/workspace/src/highlights.rs", &format!("{text} ")),
        "cache key should include length so hash collisions across lengths stay separated"
    );
    assert!(
        tree_sitter_semantic_token_overlays_for_visible_lines(
            "/workspace/src/highlights.txt",
            &line_slices,
            Some(text),
        )
        .is_none(),
        "non-Rust paths should skip tree-sitter overlays"
    );
    let cached_overlays = tree_sitter_semantic_token_overlays_for_visible_lines(
        "/workspace/src/highlights.rs",
        &line_slices,
        Some(text),
    )
    .expect("cached rust full-text input should use tree-sitter overlays");
    assert_eq!(overlays, cached_overlays);

    assert!(overlays.iter().any(|overlay| {
        overlay.line_number == 0
            && overlay.start_col == 0
            && overlay.end_col == 3
            && overlay.kind == ViewportSemanticTokenKind::Keyword
    }));
    assert!(overlays.iter().any(|overlay| {
        overlay.line_number == 0
            && overlay.start_col == 7
            && overlay.end_col == 11
            && overlay.kind == ViewportSemanticTokenKind::Function
    }));
    assert!(overlays.iter().any(|overlay| {
        overlay.line_number == 1 && overlay.kind == ViewportSemanticTokenKind::String
    }));
}

#[test]
fn tree_sitter_overlay_pipeline_splits_multiline_string_captures() {
    let text = "message = \"\"\"first\nsecond\nthird\"\"\"\n";
    let line_slices = logical_lines_with_offsets(text)
        .into_iter()
        .map(|(line_number, start_byte, line)| ViewportLineSlice {
            line_number,
            visible_text: line.to_string(),
            byte_range: ByteRange {
                start: start_byte as u64,
                end: start_byte.saturating_add(line.len()) as u64,
            },
            utf16_range: legion_protocol::Utf16Range {
                start: legion_protocol::Utf16Position {
                    line: line_number,
                    character: 0,
                },
                end: legion_protocol::Utf16Position {
                    line: line_number,
                    character: line.encode_utf16().count() as u32,
                },
            },
            chunk_hash: FileFingerprint {
                algorithm: "test".to_string(),
                value: format!("line:{line_number}"),
            },
            truncation_state: legion_protocol::ViewportLineTruncationState::None,
        })
        .collect::<Vec<_>>();

    let overlays = tree_sitter_semantic_token_overlays_for_visible_lines(
        "/workspace/src/multiline.py",
        &line_slices,
        Some(text),
    )
    .expect("Python full-text input should use tree-sitter overlays");

    for line_number in 0..3 {
        assert!(
            overlays.iter().any(|overlay| {
                overlay.line_number == line_number
                    && overlay.kind == ViewportSemanticTokenKind::String
                    && overlay.start_col < overlay.end_col
            }),
            "expected a string overlay on logical line {line_number}, got {overlays:?}"
        );
    }
}

#[test]
fn terminal_control_input_is_not_line_normalized() {
    assert_eq!(terminal_input_payload_to_send("echo ready"), "echo ready\n");
    assert_eq!(terminal_input_payload_to_send(""), "\n");
    assert_eq!(terminal_input_payload_to_send("\r"), "\r");
    assert_eq!(terminal_input_payload_to_send("\x03"), "\x03");
    assert_eq!(terminal_input_payload_to_send("\x1b[A"), "\x1b[A");
    assert_eq!(terminal_input_payload_to_send("\t"), "\t");
}

#[test]
fn tree_sitter_overlay_cache_evicts_oldest_inserted_entry() {
    let mut cache = TreeSitterOverlayCache::default();
    let first = tree_sitter_overlay_cache_key("/workspace/src/first.rs", "fn first() {}\n");
    cache.insert_if_absent(first.clone(), Vec::new());
    for index in 0..TREE_SITTER_OVERLAY_CACHE_MAX_ENTRIES {
        cache.insert_if_absent(
            tree_sitter_overlay_cache_key(
                &format!("/workspace/src/{index}.rs"),
                &format!("fn f_{index}() {{}}\n"),
            ),
            Vec::new(),
        );
    }

    assert!(
        cache.get(&first).is_none(),
        "oldest entry should be evicted"
    );
    assert_eq!(cache.entries.len(), TREE_SITTER_OVERLAY_CACHE_MAX_ENTRIES);
}

#[test]
fn audit_rollback_failure_diagnostics_are_preserved_on_failed_response() {
    let path = CanonicalPath("C:/repo/locked-file.txt".to_string());
    let diagnostic = ProtocolDiagnostic {
        code: "proposal.audit_rollback_workspace_failed".to_string(),
        message: "audit failure rollback did not restore workspace state: locked".to_string(),
        severity: ProtocolDiagnosticSeverity::Error,
        path: Some(path.clone()),
        range: None,
    };
    let mut response = ProposalResponse::Failed {
        transition: ProposalLifecycleTransition {
            proposal_id: ProposalId(99),
            lifecycle_state: ProposalLifecycleState::Failed,
            timestamp: TimestampMillis(1),
            principal: PrincipalId("trusted".to_string()),
            capability: CapabilityId("fs.write".to_string()),
            correlation_id: CorrelationId(1),
            causality_id: CausalityId(uuid::Uuid::now_v7()),
            diagnostics: vec![AppProposalCoordinator::diagnostic(
                "proposal.audit_storage_failed",
                "audit storage failed",
            )],
        },
        reason: ProposalFailureReason::StorageFailed,
    };

    AppComposition::append_response_diagnostics(&mut response, vec![diagnostic]);

    assert_transition_diagnostic(&response, "proposal.audit_storage_failed");
    assert_transition_diagnostic(&response, "proposal.audit_rollback_workspace_failed");
    let ProposalResponse::Failed { transition, .. } = response else {
        panic!("expected failed response");
    };
    let rollback_diagnostic = transition
        .diagnostics
        .iter()
        .find(|diagnostic| diagnostic.code == "proposal.audit_rollback_workspace_failed")
        .expect("rollback diagnostic");
    assert_eq!(rollback_diagnostic.path.as_ref(), Some(&path));
    assert!(rollback_diagnostic.message.contains("locked"));
}

fn text_edit_proposal(proposal_id: ProposalId) -> WorkspaceProposal {
    WorkspaceProposal {
        proposal_id,
        principal: PrincipalId("trusted".to_string()),
        capability: CapabilityId("editor.write".to_string()),
        correlation_id: CorrelationId(1),
        payload: ProposalPayload::TextEdit(legion_protocol::TextEditProposal {
            file_id: FileId(1),
            edits: legion_protocol::EditBatch {
                edits: vec![legion_protocol::TextEdit {
                    range: legion_protocol::TextRange::new(
                        legion_protocol::TextOffset::byte(0),
                        legion_protocol::TextOffset::byte(0),
                    ),
                    replacement: "replacement".to_string(),
                }],
            },
        }),
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
        preview: PreviewSummary {
            summary: "test text edit".to_string(),
            details: Vec::new(),
        },
        expires_at: None,
        created_at: TimestampMillis(1),
    }
}

fn test_file(file_id: u128, path: &str) -> FileIdentity {
    FileIdentity {
        file_id: FileId(file_id),
        workspace_id: WorkspaceId(1),
        canonical_path: CanonicalPath(path.to_string()),
        content_version: FileContentVersion(1),
        content_hash: None,
    }
}

fn complete_file_preconditions() -> ProposalVersionPreconditions {
    ProposalVersionPreconditions {
        file_version: Some(FileContentVersion(1)),
        buffer_version: Some(legion_protocol::BufferVersion(1)),
        snapshot_id: Some(legion_protocol::SnapshotId(1)),
        generation: Some(WorkspaceGeneration(1)),
        file_content_version: Some(FileContentVersion(1)),
        workspace_generation: Some(WorkspaceGeneration(1)),
        expected_fingerprint: Some(FileFingerprint {
            algorithm: "test".to_string(),
            value: "hash:test".to_string(),
        }),
        expected_file_length: None,
        expected_modified_at: None,
    }
}

fn proposal_with(
    proposal_id: ProposalId,
    capability: &str,
    payload: ProposalPayload,
) -> WorkspaceProposal {
    WorkspaceProposal {
        proposal_id,
        principal: PrincipalId("trusted".to_string()),
        capability: CapabilityId(capability.to_string()),
        correlation_id: CorrelationId(1),
        payload,
        preconditions: complete_file_preconditions(),
        preview: PreviewSummary {
            summary: "test proposal".to_string(),
            details: Vec::new(),
        },
        expires_at: None,
        created_at: TimestampMillis(1),
    }
}

fn workspace_edit_payload() -> ProposalPayload {
    let path = CanonicalPath("C:/repo/workspace-created.rs".to_string());
    ProposalPayload::WorkspaceEdit(legion_protocol::WorkspaceEditProposalPayload {
        workspace_id: WorkspaceId(1),
        edit_id: uuid::Uuid::now_v7(),
        title: "workspace create".to_string(),
        source: legion_protocol::WorkspaceEditSourceKind::User,
        target_coverage: ProposalTargetCoverage {
            coverage_kind: ProposalTargetCoverageKind::Complete,
            targets: vec![AppProposalCoordinator::path_target(
                "workspace-create".to_string(),
                ProposalTargetKind::PathOnly,
                path.clone(),
                Vec::new(),
            )],
            omitted_target_count: 0,
            redaction_hints: vec![RedactionHint::MetadataOnly],
        },
        file_edits: Vec::new(),
        change_annotations: Vec::new(),
        file_operations: vec![legion_protocol::WorkspaceFileOperation::Create {
            path,
            initial_content_hash: None,
        }],
        required_capability: CapabilityId("fs.write".to_string()),
        diagnostics: Vec::new(),
        schema_version: 1,
    })
}

fn terminal_payload() -> ProposalPayload {
    ProposalPayload::TerminalCommand(legion_protocol::TerminalCommandProposal {
        session_id: Some(legion_protocol::TerminalSessionId(7)),
        command: "cargo test".to_string(),
        cwd: Some(CanonicalPath("C:/repo".to_string())),
        env: HashMap::new(),
    })
}

#[test]
fn proposal_coordinator_enforces_preview_after_validation() {
    let coordinator = AppProposalCoordinator::new(SharedEventSink::default());
    let proposal = save_proposal(ProposalId(1));
    register_created(&coordinator, &proposal);

    let preview = coordinator
        .handle(ProposalRequest::Preview(proposal.clone()))
        .expect("preview response");
    let ProposalResponse::Rejected { transition, reason } = preview else {
        panic!("preview before validation should reject");
    };
    assert_eq!(reason, ProposalRejectionReason::ValidationFailed);
    assert!(
        transition
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "proposal.invalid_lifecycle_transition")
    );

    let validation = coordinator
        .handle(ProposalRequest::Validate(proposal.clone()))
        .expect("validate response");
    assert!(matches!(validation, ProposalResponse::Validated(_)));
    let preview = coordinator
        .handle(ProposalRequest::Preview(proposal))
        .expect("preview response");
    assert!(matches!(preview, ProposalResponse::Previewed { .. }));
}

#[test]
fn command_dispatcher_maps_projection_only_proposal_intents_to_protocol_requests() {
    let proposal = save_proposal(ProposalId(42));
    let preview = CommandDispatcher::route_proposal_intent(
        CommandDispatchIntent::PreviewProposal {
            proposal_id: ProposalId(42),
        },
        proposal_intent_route_context(Some(proposal.clone())),
    )
    .expect("preview intent maps")
    .expect("preview request");
    assert!(
        matches!(preview, ProposalRequest::Preview(mapped) if mapped.proposal_id == ProposalId(42))
    );

    let approve = CommandDispatcher::route_proposal_intent(
        CommandDispatchIntent::ApproveProposal {
            proposal_id: ProposalId(42),
        },
        proposal_intent_route_context(None),
    )
    .expect("approve intent maps")
    .expect("approve request");
    let ProposalRequest::Approve(command) = approve else {
        panic!("expected approve request");
    };
    assert_eq!(command.proposal_id, ProposalId(42));
    assert_eq!(command.action, ProposalLifecycleAction::Approve);
    assert_eq!(command.principal, PrincipalId("trusted".to_string()));

    let reject = CommandDispatcher::route_proposal_intent(
        CommandDispatchIntent::RejectProposal {
            proposal_id: ProposalId(42),
            reason: ProposalRejectionReason::UserRejected,
        },
        proposal_intent_route_context(None),
    )
    .expect("reject intent maps")
    .expect("reject request");
    let ProposalRequest::Reject(command) = reject else {
        panic!("expected reject request");
    };
    assert!(matches!(
        command.reason,
        Some(ProposalLifecycleCommandReason::Rejection(
            ProposalRejectionReason::UserRejected
        ))
    ));

    let details = CommandDispatcher::route_proposal_intent(
        CommandDispatchIntent::OpenProposalDetails {
            proposal_id: ProposalId(42),
        },
        proposal_intent_route_context(Some(proposal)),
    )
    .expect("details intent maps");
    assert!(details.is_none());
}

#[test]
fn command_dispatcher_routes_manual_clipboard_input_intents_to_app_requests() {
    let active = AppCommandRouteContext {
        workspace_id: Some(WorkspaceId(1)),
        buffer_id: Some(BufferId(9)),
        file_id: Some(FileId(2)),
    };

    let copy = CommandDispatcher::route_intent(
        CommandDispatchIntent::ClipboardCopy {
            buffer_id: BufferId(9),
        },
        active,
        CorrelationId(1),
    )
    .expect("copy routes");
    assert_eq!(
        copy,
        AppCommandRequest::ClipboardCopy {
            buffer_id: BufferId(9)
        }
    );

    let cut = CommandDispatcher::route_intent(
        CommandDispatchIntent::ClipboardCut {
            buffer_id: BufferId(9),
        },
        active,
        CorrelationId(1),
    )
    .expect("cut routes");
    assert_eq!(
        cut,
        AppCommandRequest::ClipboardCut {
            buffer_id: BufferId(9)
        }
    );

    let select_all = CommandDispatcher::route_intent(
        CommandDispatchIntent::SelectAll {
            buffer_id: BufferId(9),
        },
        active,
        CorrelationId(1),
    )
    .expect("select-all routes");
    assert_eq!(
        select_all,
        AppCommandRequest::SelectAll {
            buffer_id: BufferId(9)
        }
    );
}

#[test]
fn plugin_command_intent_routes_through_app_owned_plugin_runtime() {
    let mut app = AppComposition::new();
    let plugin_id = app
        .load_plugin_manifest(plugin_manifest(PluginId(7)))
        .expect("plugin manifest loads");

    let outcome = app
        .dispatch_ui_intent(CommandDispatchIntent::InvokePluginCommand {
            plugin_id,
            command_id: "phase5.run".to_string(),
            metadata_label: "metadata-only".to_string(),
        })
        .expect("plugin command routes through app");

    match outcome {
        AppCommandOutcome::PluginCommandInvoked(response) => {
            assert!(matches!(
                response.as_ref(),
                PluginHostCallResponse::Accepted { metadata_label }
                    if metadata_label == "metadata-only"
            ));
        }
        other => panic!("unexpected plugin command outcome: {other:?}"),
    }
}

#[test]
fn command_dispatcher_routes_collaboration_intents_to_app_requests() {
    let active = AppCommandRouteContext {
        workspace_id: Some(WorkspaceId(1)),
        buffer_id: Some(BufferId(1)),
        file_id: Some(FileId(1)),
    };
    let join = CommandDispatcher::route_intent(
        CommandDispatchIntent::JoinCollaborationSession {
            session_id: CollaborationSessionId(7),
        },
        active,
        CorrelationId(1),
    )
    .expect("join routes");
    assert_eq!(
        join,
        AppCommandRequest::JoinCollaborationSession {
            session_id: CollaborationSessionId(7)
        }
    );

    let presence = CommandDispatcher::route_intent(
        CommandDispatchIntent::PublishCollaborationPresence {
            session_id: CollaborationSessionId(7),
            participant_id: CollaborationParticipantId(9),
        },
        active,
        CorrelationId(1),
    )
    .expect("presence routes");
    assert_eq!(
        presence,
        AppCommandRequest::PublishCollaborationPresence {
            session_id: CollaborationSessionId(7),
            participant_id: CollaborationParticipantId(9),
        }
    );
}

#[test]
fn shared_collaboration_route_wraps_existing_safe_targets_only() {
    let editor_target = ProposalAffectedTarget {
        target_id: "editor".to_string(),
        kind: ProposalTargetKind::OpenBuffer,
        workspace_id: Some(WorkspaceId(1)),
        file_id: Some(FileId(1)),
        buffer_id: Some(BufferId(1)),
        path: None,
        terminal_session_id: None,
        plugin_id: None,
        remote_authority: None,
        collaboration_session_id: None,
        byte_ranges: Vec::new(),
        redaction_hints: vec![RedactionHint::MetadataOnly],
    };
    let collaboration_target = ProposalAffectedTarget {
        target_id: "collaboration".to_string(),
        kind: ProposalTargetKind::CollaborationSession,
        workspace_id: Some(WorkspaceId(1)),
        file_id: Some(FileId(1)),
        buffer_id: Some(BufferId(1)),
        path: None,
        terminal_session_id: None,
        plugin_id: None,
        remote_authority: None,
        collaboration_session_id: Some("7".to_string()),
        byte_ranges: Vec::new(),
        redaction_hints: vec![RedactionHint::MetadataOnly],
    };
    let shared = ProposalTargetCoverage {
        coverage_kind: ProposalTargetCoverageKind::Complete,
        targets: vec![editor_target, collaboration_target.clone()],
        omitted_target_count: 0,
        redaction_hints: vec![RedactionHint::MetadataOnly],
    };
    assert_eq!(
        ProposalExecutionRoute::for_payload(&text_edit_proposal(ProposalId(70)).payload, &shared),
        ProposalExecutionRoute::SharedCollaboration
    );

    let pure_collaboration = ProposalTargetCoverage {
        coverage_kind: ProposalTargetCoverageKind::Complete,
        targets: vec![collaboration_target],
        omitted_target_count: 0,
        redaction_hints: vec![RedactionHint::MetadataOnly],
    };
    assert_eq!(
        ProposalExecutionRoute::for_payload(
            &text_edit_proposal(ProposalId(71)).payload,
            &pure_collaboration
        ),
        ProposalExecutionRoute::Unsupported
    );
}

#[test]
fn command_dispatcher_rejects_apply_intent_without_app_owned_matching_proposal() {
    let missing = CommandDispatcher::route_proposal_intent(
        CommandDispatchIntent::ApplyProposal {
            proposal_id: ProposalId(42),
        },
        proposal_intent_route_context(None),
    );
    assert!(matches!(
        missing,
        Err(AppCompositionError::ProposalIntentMissingProposal)
    ));

    let mismatch = CommandDispatcher::route_proposal_intent(
        CommandDispatchIntent::ApplyProposal {
            proposal_id: ProposalId(42),
        },
        proposal_intent_route_context(Some(save_proposal(ProposalId(7)))),
    );
    assert!(matches!(
        mismatch,
        Err(AppCompositionError::ProposalIntentMismatch {
            target: ProposalId(42),
            active: Some(ProposalId(7))
        })
    ));
}

#[test]
fn proposal_coordinator_allows_created_validated_previewed_approved_applied_path() {
    let coordinator = AppProposalCoordinator::new(SharedEventSink::default());
    let proposal = save_proposal(ProposalId(10));
    register_created(&coordinator, &proposal);

    assert!(matches!(
        coordinator.handle(ProposalRequest::Validate(proposal.clone())),
        Ok(ProposalResponse::Validated(_))
    ));
    assert!(matches!(
        coordinator.handle(ProposalRequest::Preview(proposal.clone())),
        Ok(ProposalResponse::Previewed { .. })
    ));
    assert!(matches!(
        coordinator.handle(ProposalRequest::Approve(command(
            proposal.proposal_id,
            legion_protocol::ProposalLifecycleAction::Approve,
        ))),
        Ok(ProposalResponse::Approved(_))
    ));

    let transition = coordinator
        .record_transition(&proposal, ProposalLifecycleState::Applied, "apply")
        .expect("approved proposal can apply");
    assert_eq!(transition.lifecycle_state, ProposalLifecycleState::Applied);
    assert_eq!(
        coordinator.current_lifecycle_state(proposal.proposal_id),
        Some(ProposalLifecycleState::Applied)
    );
}

#[test]
fn proposal_coordinator_exports_and_recovers_lifecycle_snapshot() {
    let coordinator = AppProposalCoordinator::new(SharedEventSink::default());
    let proposal = save_proposal(ProposalId(20));
    register_created(&coordinator, &proposal);
    assert!(matches!(
        coordinator.handle(ProposalRequest::Validate(proposal.clone())),
        Ok(ProposalResponse::Validated(_))
    ));
    assert!(matches!(
        coordinator.handle(ProposalRequest::Preview(proposal.clone())),
        Ok(ProposalResponse::Previewed { .. })
    ));

    let snapshot = coordinator.proposal_lifecycle_recovery_snapshot();
    assert_eq!(snapshot.records.len(), 1);
    assert!(snapshot.generated_at.0 > 0);

    let recovered = AppProposalCoordinator::new(SharedEventSink::default());
    recovered.recover_lifecycle_from_snapshot(snapshot);

    assert_eq!(
        recovered.current_lifecycle_state(proposal.proposal_id),
        Some(ProposalLifecycleState::Previewed)
    );
    assert!(recovered.has_lifecycle_context(proposal.proposal_id));
    assert_eq!(
        recovered
            .proposal(proposal.proposal_id)
            .map(|proposal| proposal.proposal_id),
        Some(proposal.proposal_id)
    );

    let ledger = recovered.proposal_ledger_projection(TimestampMillis(99));
    assert_eq!(ledger.rows.len(), 1);
    assert_eq!(ledger.selected_proposal_id, Some(proposal.proposal_id));
    assert_eq!(
        ledger.rows[0].lifecycle.state,
        ProposalLifecycleState::Previewed
    );
    assert_eq!(ledger.rows[0].updated_at, TimestampMillis(99));
    assert!(
        ledger.rows[0]
            .redaction_hints
            .contains(&RedactionHint::MetadataOnly)
    );
}

#[cfg(feature = "ai")]
fn delegated_output_from(
    proposal: WorkspaceProposal,
    suffix: &str,
) -> legion_protocol::AssistedAiEditProposalOutput {
    legion_protocol::AssistedAiEditProposalOutput {
        output_id: format!("delegated-output-{suffix}"),
        request_id: format!("delegated-request-{suffix}"),
        provider_id: "provider:test".to_string(),
        proposal_id: ProposalId(0),
        principal: proposal.principal,
        capability: proposal.capability,
        correlation_id: proposal.correlation_id,
        causality_id: CausalityId(uuid::Uuid::now_v7()),
        payload: proposal.payload,
        preconditions: proposal.preconditions,
        preview: proposal.preview,
        expires_at: proposal.expires_at,
        created_at: proposal.created_at,
        context_manifest: trust_reference(
            "delegated-context-test",
            legion_protocol::AssistedAiTrustProjectionKind::ContextManifest,
        ),
        approval_checklist: trust_reference(
            "delegated-approval-test",
            legion_protocol::AssistedAiTrustProjectionKind::ProposalApprovalChecklist,
        ),
        redaction_hints: vec![RedactionHint::MetadataOnly],
        schema_version: 1,
    }
}

#[cfg(feature = "ai")]
#[derive(Clone)]
struct FailSecondAtomicBatchSink {
    recorder: legion_observability::InMemoryEventSink,
    fail_second: Arc<AtomicBool>,
    legacy_emit_calls: Arc<AtomicUsize>,
    batch_emit_calls: Arc<AtomicUsize>,
}

#[cfg(feature = "ai")]
impl EventSinkPort for FailSecondAtomicBatchSink {
    fn emit(&self, request: EventSinkRequest) -> ProtocolResult<()> {
        self.legacy_emit_calls.fetch_add(1, Ordering::SeqCst);
        self.recorder.emit(request)
    }

    fn emit_batch(&self, requests: Vec<EventSinkRequest>) -> ProtocolResult<()> {
        self.batch_emit_calls.fetch_add(1, Ordering::SeqCst);
        for (index, request) in requests.iter().enumerate() {
            legion_observability::validate_envelope(
                &request.envelope,
                legion_observability::EventSinkConfig::default(),
            )
            .map_err(|error| ProtocolError {
                code: "test_sink_validation_failed".to_string(),
                message: error.to_string(),
            })?;
            if index == 1 && self.fail_second.load(Ordering::SeqCst) {
                return Err(ProtocolError {
                    code: "test_sink_validation_failed".to_string(),
                    message: "injected validation failure at second batch item".to_string(),
                });
            }
        }
        self.recorder.emit_batch(requests)
    }
}

#[cfg(feature = "ai")]
#[test]
fn delegated_proposal_preflight_failure_keeps_ledger_and_storage_unchanged() {
    let event_sink = legion_observability::InMemoryEventSink::new();
    let mut app = AppComposition::with_event_sink(SharedEventSink::new(event_sink.clone()));
    let mut rejected = delegated_output_from(save_proposal(ProposalId(101)), "second");
    rejected.correlation_id = CorrelationId(0);

    let error = app
        .register_delegated_task_proposals(vec![
            delegated_output_from(save_proposal(ProposalId(100)), "first"),
            rejected,
        ])
        .expect_err("invalid second proposal rejects the staged batch");
    assert!(matches!(error, AppCompositionError::AiRuntime(_)));
    assert_eq!(
        app.delegate_workflow.runtime_activation,
        DelegatedTaskRuntimeActivationState::Failed
    );
    assert!(
        app.proposal_coordinator
            .proposal_ledger_projection(TimestampMillis(99))
            .rows
            .is_empty()
    );
    assert!(event_sink.events().expect("event snapshot").is_empty());
    assert!(
        app.storage
            .pending_proposal_observation_batches()
            .expect("pending batches")
            .is_empty()
    );
    for proposal_id in [ProposalId(1), ProposalId(2)] {
        assert!(matches!(
            app.storage
                .handle(StorageRepositoryRequest::ReadProposalAuditRecord(
                    proposal_id
                ))
                .expect("read audit record"),
            StorageRepositoryResponse::ProposalAuditRecord(None)
        ));
    }

    let registered = app
        .register_delegated_task_proposals(vec![delegated_output_from(
            save_proposal(ProposalId(102)),
            "retry",
        )])
        .expect("valid retry after staged rollback");
    assert_eq!(registered[0].proposal_id, ProposalId(1));
}

#[cfg(feature = "ai")]
#[test]
fn delegated_proposal_storage_failure_keeps_ledger_and_ids_unchanged() {
    let event_sink = legion_observability::InMemoryEventSink::new();
    let mut app = AppComposition::with_event_sink(SharedEventSink::new(event_sink.clone()));
    app.storage
        .fail_proposal_observation_batch_at_item_for_test(1);
    let error = app
        .register_delegated_task_proposals(vec![
            delegated_output_from(save_proposal(ProposalId(100)), "first"),
            delegated_output_from(save_proposal(ProposalId(101)), "second"),
        ])
        .expect_err("the injected second-item storage failure rejects the full batch");

    assert!(matches!(
        error,
        AppCompositionError::Protocol(ProtocolError { code, .. }) if code == "storage_failed"
    ));
    assert_eq!(
        app.delegate_workflow.runtime_activation,
        DelegatedTaskRuntimeActivationState::Failed
    );
    let ledger = app
        .proposal_coordinator
        .proposal_ledger_projection(TimestampMillis(99));
    assert!(ledger.rows.is_empty());
    assert!(
        event_sink.events().expect("event snapshot").is_empty(),
        "storage rejection must not emit any Created event"
    );
    assert!(app.proposal_coordinator.proposal(ProposalId(1)).is_none());
    assert!(
        app.storage
            .pending_proposal_observation_batches()
            .expect("pending batches")
            .is_empty()
    );
    for proposal_id in [ProposalId(1), ProposalId(2)] {
        assert!(matches!(
            app.storage
                .handle(StorageRepositoryRequest::ReadProposalAuditRecord(
                    proposal_id
                ))
                .expect("read audit record"),
            StorageRepositoryResponse::ProposalAuditRecord(None)
        ));
    }
    let storage_debug = app
        .storage
        .with_storage(|storage| format!("{storage:?}"))
        .expect("storage snapshot");
    assert!(storage_debug.contains("protocol_event_metadata: {}"));
    assert!(storage_debug.contains("protocol_proposal_audit: {}"));
    assert!(storage_debug.contains("protocol_proposal_observation_outbox: {}"));

    let registered = app
        .register_delegated_task_proposals(vec![delegated_output_from(
            save_proposal(ProposalId(102)),
            "retry",
        )])
        .expect("a valid retry should register after rollback");
    assert_eq!(registered[0].proposal_id, ProposalId(1));
}

#[cfg(feature = "ai")]
#[test]
fn delegated_proposal_sink_failure_schedules_production_retry() {
    let recorder = legion_observability::InMemoryEventSink::new();
    let fail_second = Arc::new(AtomicBool::new(true));
    let legacy_emit_calls = Arc::new(AtomicUsize::new(0));
    let batch_emit_calls = Arc::new(AtomicUsize::new(0));
    let sink = FailSecondAtomicBatchSink {
        recorder: recorder.clone(),
        fail_second: Arc::clone(&fail_second),
        legacy_emit_calls: Arc::clone(&legacy_emit_calls),
        batch_emit_calls: Arc::clone(&batch_emit_calls),
    };
    let mut app = AppComposition::with_event_sink(SharedEventSink::new(sink));

    let registered = app
        .register_delegated_task_proposals(vec![
            delegated_output_from(save_proposal(ProposalId(100)), "first"),
            delegated_output_from(save_proposal(ProposalId(101)), "second"),
        ])
        .expect("sink failure must retain registration and schedule delivery retry");

    assert_eq!(
        registered
            .iter()
            .map(|proposal| proposal.proposal_id)
            .collect::<Vec<_>>(),
        vec![ProposalId(1), ProposalId(2)]
    );
    let ledger = app
        .proposal_coordinator
        .proposal_ledger_projection(TimestampMillis(99));
    assert_eq!(ledger.rows.len(), 2);
    assert_eq!(
        ledger
            .rows
            .iter()
            .map(|row| row.proposal_id)
            .collect::<Vec<_>>(),
        vec![ProposalId(1), ProposalId(2)]
    );
    assert!(recorder.events().expect("event snapshot").is_empty());
    assert_eq!(legacy_emit_calls.load(Ordering::SeqCst), 0);
    assert_eq!(batch_emit_calls.load(Ordering::SeqCst), 1);

    let pending = app
        .storage
        .pending_proposal_observation_batches()
        .expect("pending batch");
    assert_eq!(pending.len(), 1);
    assert!(pending[0].batch.batch_id.starts_with("dpr3-"));
    assert_eq!(pending[0].batch.batch_id.len(), "dpr3-".len() + 64);
    assert_eq!(pending[0].batch.event_metadata.len(), 2);
    assert_eq!(pending[0].batch.proposal_audits.len(), 2);
    assert_eq!(
        pending[0].batch.schema_version,
        PROPOSAL_OBSERVATION_BATCH_SCHEMA_VERSION
    );
    for ((event, metadata), audit) in pending[0]
        .batch
        .events
        .iter()
        .zip(&pending[0].batch.event_metadata)
        .zip(&pending[0].batch.proposal_audits)
    {
        assert_eq!(event.event_id, metadata.event_id);
        assert_eq!(
            event.payload["proposal_id"].as_u64(),
            Some(audit.proposal_id.0)
        );
        assert_eq!(event.correlation_id, audit.correlation_id);
        assert_eq!(event.causality_id, audit.causality_id);
        assert_eq!(event.occurred_at, audit.timestamp);
        assert_eq!(audit.lifecycle_state, ProposalLifecycleState::Created);
        assert!(matches!(
            app.storage
                .handle(StorageRepositoryRequest::ReadProposalAuditRecord(
                    audit.proposal_id
                ))
                .expect("read audit record"),
            StorageRepositoryResponse::ProposalAuditRecord(Some(stored))
                if stored.lifecycle_state == ProposalLifecycleState::Created
        ));
    }
    for metadata in &pending[0].batch.event_metadata {
        assert!(matches!(
            app.storage
                .handle(StorageRepositoryRequest::ReadEventMetadata(
                    metadata.event_id
                ))
                .expect("read event metadata"),
            StorageRepositoryResponse::EventMetadata(Some(_))
        ));
    }

    let still_pending = app
        .retry_pending_proposal_observations()
        .expect("failed retry report");
    assert_eq!(still_pending.delivered_count, 0);
    assert_eq!(still_pending.pending_count, 1);
    assert_eq!(still_pending.attempts.len(), 1);
    assert_eq!(
        still_pending.attempts[0].delivery_state,
        legion_storage::ProposalObservationDeliveryState::Pending
    );
    assert_eq!(
        still_pending.attempts[0].error_code.as_deref(),
        Some("test_sink_validation_failed")
    );
    assert_eq!(
        still_pending.attempts[0].error_kind,
        Some(legion_storage::ProposalObservationRetryErrorKind::Transient)
    );
    assert_eq!(recorder.events().expect("event snapshot").len(), 0);

    fail_second.store(false, Ordering::SeqCst);
    assert!(
        app.poll_product_ai_stream(),
        "production polling must service the scheduled observation retry"
    );
    assert_eq!(recorder.events().expect("event snapshot").len(), 2);
    assert!(
        app.storage
            .pending_proposal_observation_batches()
            .expect("pending batches after retry")
            .is_empty()
    );
    assert_eq!(legacy_emit_calls.load(Ordering::SeqCst), 0);
    assert_eq!(batch_emit_calls.load(Ordering::SeqCst), 3);

    let empty = app
        .retry_pending_proposal_observations()
        .expect("idempotent empty retry");
    assert_eq!(empty.delivered_count, 0);
    assert_eq!(empty.pending_count, 0);
    assert!(empty.attempts.is_empty());
    assert_eq!(recorder.events().expect("event snapshot").len(), 2);
    assert_eq!(legacy_emit_calls.load(Ordering::SeqCst), 0);
    assert_eq!(batch_emit_calls.load(Ordering::SeqCst), 3);
}

#[cfg(feature = "ai")]
#[test]
fn delegated_registration_key_canonicalizes_hash_map_insertion_order() {
    let mut first = delegated_output_from(save_proposal(ProposalId(100)), "canonical");
    let mut first_env = HashMap::new();
    first_env.insert("B_KEY".to_string(), "two".to_string());
    first_env.insert("A_KEY".to_string(), "one".to_string());
    first.payload = ProposalPayload::TerminalCommand(legion_protocol::TerminalCommandProposal {
        session_id: None,
        command: "cargo test".to_string(),
        cwd: Some(CanonicalPath("C:/repo".to_string())),
        env: first_env,
    });

    let mut second = first.clone();
    let mut second_env = HashMap::new();
    second_env.insert("A_KEY".to_string(), "one".to_string());
    second_env.insert("B_KEY".to_string(), "two".to_string());
    if let ProposalPayload::TerminalCommand(command) = &mut second.payload {
        command.env = second_env;
    } else {
        panic!("test payload must remain a terminal command");
    }

    assert_eq!(
        AppComposition::delegated_registration_keys(&[first]).expect("first canonical key"),
        AppComposition::delegated_registration_keys(&[second]).expect("second canonical key")
    );
}

#[cfg(feature = "ai")]
#[test]
fn durable_proposal_observation_rejects_near_terminal_identity_floors() {
    let app_sink = legion_observability::InMemoryEventSink::new();
    let mut app = AppComposition::with_event_sink(SharedEventSink::new(app_sink));
    let proposal_id = ProposalId(MAX_DURABLE_PROPOSAL_OBSERVATION_FLOOR);
    let proposal = save_proposal(proposal_id);
    let causality_id = CausalityId(uuid::Uuid::now_v7());
    let transition = ProposalLifecycleTransition {
        proposal_id,
        lifecycle_state: ProposalLifecycleState::Created,
        timestamp: TimestampMillis(1),
        principal: proposal.principal.clone(),
        capability: proposal.capability.clone(),
        correlation_id: proposal.correlation_id,
        causality_id,
        diagnostics: Vec::new(),
    };
    let event = proposal_created_event_with_transition(
        &proposal,
        &transition,
        EventSequence(MAX_DURABLE_PROPOSAL_OBSERVATION_FLOOR),
    )
    .expect("near-limit event remains structurally valid");
    let batch = ProposalObservationBatch {
        batch_id: "near-terminal-floor".to_string(),
        event_metadata: vec![event_metadata_record(&event)],
        proposal_audits: vec![
            proposal_audit_record(&proposal, &transition).expect("near-limit audit"),
        ],
        events: vec![event],
        schema_version: PROPOSAL_OBSERVATION_BATCH_SCHEMA_VERSION,
    };
    app.storage
        .store_proposal_observation_batch(batch)
        .expect("store near-limit untrusted durable record");

    let error = app
        .reserve_durable_proposal_observation_identities()
        .expect_err("near-terminal durable floors must fail closed");
    assert_eq!(error.code, "proposal_observation_identity_exhausted");
    assert_eq!(app.proposal_coordinator.next_proposal_id.get(), 0);
    assert_eq!(app.proposal_coordinator.next_event_sequence.get(), 0);
}

#[test]
fn proposal_persistence_late_enable_rejects_live_identity_overlap() {
    let workspace_root = unique_temp_dir("proposal-persistence-late-enable");
    let mut app = AppComposition::new();
    let live = save_proposal(ProposalId(1));
    register_created(&app.proposal_coordinator, &live);

    let error = app
        .enable_proposal_audit_persistence(&workspace_root)
        .expect_err("persistence enable after live proposals must fail closed");
    assert!(matches!(
        error,
        AppCompositionError::Protocol(ProtocolError { code, .. })
            if code == "proposal_observation_publication_conflict"
    ));
    assert!(
        !workspace_root.join(".legion").exists(),
        "rejected late enable must not bind or create the durability root"
    );

    fs::remove_dir_all(&workspace_root).expect("remove late-enable test workspace");
}

#[cfg(feature = "ai")]
#[test]
fn proposal_observation_retry_skips_orphan_without_blocking_published_batch() {
    let recorder = legion_observability::InMemoryEventSink::new();
    let fail_second = Arc::new(AtomicBool::new(true));
    let sink = FailSecondAtomicBatchSink {
        recorder: recorder.clone(),
        fail_second: Arc::clone(&fail_second),
        legacy_emit_calls: Arc::new(AtomicUsize::new(0)),
        batch_emit_calls: Arc::new(AtomicUsize::new(0)),
    };
    let mut app = AppComposition::with_event_sink(SharedEventSink::new(sink));
    let published = app
        .register_delegated_task_proposals(vec![
            delegated_output_from(save_proposal(ProposalId(100)), "published-first"),
            delegated_output_from(save_proposal(ProposalId(101)), "published-second"),
        ])
        .expect("published batch remains Pending with a scheduled retry");
    let first_published = app
        .proposal_coordinator
        .proposal(published[0].proposal_id)
        .expect("published proposal");
    assert!(matches!(
        app.proposal_coordinator
            .handle(ProposalRequest::Validate(first_published)),
        Ok(ProposalResponse::Validated(_))
    ));

    let orphan_proposal = save_proposal(ProposalId(900));
    let orphan_transition = ProposalLifecycleTransition {
        proposal_id: orphan_proposal.proposal_id,
        lifecycle_state: ProposalLifecycleState::Created,
        timestamp: TimestampMillis(900),
        principal: orphan_proposal.principal.clone(),
        capability: orphan_proposal.capability.clone(),
        correlation_id: orphan_proposal.correlation_id,
        causality_id: CausalityId(uuid::Uuid::now_v7()),
        diagnostics: Vec::new(),
    };
    let orphan_event = proposal_created_event_with_transition(
        &orphan_proposal,
        &orphan_transition,
        EventSequence(900),
    )
    .expect("orphan event");
    app.storage
        .store_proposal_observation_batch(ProposalObservationBatch {
            batch_id: "aaa-orphan".to_string(),
            event_metadata: vec![event_metadata_record(&orphan_event)],
            proposal_audits: vec![
                proposal_audit_record(&orphan_proposal, &orphan_transition).expect("orphan audit"),
            ],
            events: vec![orphan_event],
            schema_version: PROPOSAL_OBSERVATION_BATCH_SCHEMA_VERSION,
        })
        .expect("store unassociated orphan");

    fail_second.store(false, Ordering::SeqCst);
    let report = app
        .retry_pending_proposal_observations()
        .expect("retry all pending without head-of-line blocking");
    assert_eq!(report.attempts.len(), 2);
    assert_eq!(report.delivered_count, 1);
    assert_eq!(report.pending_count, 1);
    assert_eq!(report.attempts[0].batch_id, "aaa-orphan");
    assert_eq!(
        report.attempts[0].error_code.as_deref(),
        Some("proposal_observation_publication_missing")
    );
    assert_eq!(
        report.attempts[0].error_kind,
        Some(legion_storage::ProposalObservationRetryErrorKind::Permanent)
    );
    assert_eq!(
        report.attempts[1].delivery_state,
        legion_storage::ProposalObservationDeliveryState::Delivered
    );
    assert_eq!(recorder.events().expect("published events").len(), 2);
    let remaining = app
        .storage
        .pending_proposal_observation_batches()
        .expect("remaining orphan");
    assert_eq!(remaining.len(), 1);
    assert_eq!(remaining[0].batch.batch_id, "aaa-orphan");
}

#[cfg(feature = "ai")]
#[test]
fn delegated_replay_rejects_durable_lifecycle_advanced_beyond_created() {
    let workspace_root = unique_temp_dir("delegated-observation-advanced");
    let recorder = legion_observability::InMemoryEventSink::new();
    let outputs = vec![delegated_output_from(
        save_proposal(ProposalId(100)),
        "advanced",
    )];
    {
        let mut interrupted =
            AppComposition::with_event_sink(SharedEventSink::new(recorder.clone()));
        interrupted
            .enable_proposal_audit_persistence(&workspace_root)
            .expect("enable durable proposal observations");
        interrupted.interrupt_after_proposal_observation_store = true;
        interrupted
            .register_delegated_task_proposals(outputs.clone())
            .expect_err("inject post-commit interruption");
        let pending = interrupted
            .storage
            .pending_proposal_observation_batches()
            .expect("pending record");
        let mut advanced = pending[0].batch.proposal_audits[0].clone();
        advanced.lifecycle_state = ProposalLifecycleState::Applied;
        advanced.timestamp = TimestampMillis(advanced.timestamp.0.saturating_add(1));
        interrupted
            .storage
            .handle(StorageRepositoryRequest::SaveProposalAuditRecord(advanced))
            .expect("persist advanced lifecycle audit");
    }

    let mut recovered = AppComposition::with_event_sink(SharedEventSink::new(recorder.clone()));
    recovered
        .enable_proposal_audit_persistence(&workspace_root)
        .expect("reopen advanced durable state");
    let error = recovered
        .register_delegated_task_proposals(outputs)
        .expect_err("advanced durable lifecycle must not regress to Created");
    assert!(matches!(
        error,
        AppCompositionError::Protocol(ProtocolError { code, .. })
            if code == "proposal_observation_replay_lifecycle_advanced"
    ));
    assert_eq!(
        recovered.delegate_workflow.runtime_activation,
        DelegatedTaskRuntimeActivationState::Failed
    );
    assert!(
        recovered
            .proposal_coordinator
            .proposal_ledger_projection(TimestampMillis(99))
            .rows
            .is_empty()
    );
    assert!(
        recorder
            .events()
            .expect("no regressed Created event")
            .is_empty()
    );
    assert_eq!(
        recovered
            .storage
            .pending_proposal_observation_batches()
            .expect("advanced record stays pending")
            .len(),
        1
    );

    fs::remove_dir_all(&workspace_root).expect("remove advanced replay workspace");
}

#[cfg(feature = "ai")]
#[test]
fn delegated_registration_allocates_above_generic_persisted_audit_floor() {
    let workspace_root = unique_temp_dir("delegated-generic-audit-floor");
    let historical_id = ProposalId(7);
    {
        let mut first = AppComposition::new();
        first
            .enable_proposal_audit_persistence(&workspace_root)
            .expect("enable generic proposal audit persistence");
        let historical = save_proposal(historical_id);
        let transition = ProposalLifecycleTransition {
            proposal_id: historical_id,
            lifecycle_state: ProposalLifecycleState::Created,
            timestamp: TimestampMillis(7),
            principal: historical.principal.clone(),
            capability: historical.capability.clone(),
            correlation_id: historical.correlation_id,
            causality_id: CausalityId(uuid::Uuid::now_v7()),
            diagnostics: Vec::new(),
        };
        let audit = proposal_audit_record(&historical, &transition)
            .expect("build historical generic audit");
        first
            .storage
            .handle(StorageRepositoryRequest::SaveProposalAuditRecord(audit))
            .expect("persist generic proposal audit without an outbox batch");
        assert!(
            first
                .storage
                .proposal_observation_batches()
                .expect("no first-process outbox")
                .is_empty()
        );
    }

    let recorder = legion_observability::InMemoryEventSink::new();
    let mut recovered = AppComposition::with_event_sink(SharedEventSink::new(recorder.clone()));
    recovered
        .enable_proposal_audit_persistence(&workspace_root)
        .expect("reload generic proposal audit floor");
    assert_eq!(
        recovered
            .storage
            .max_proposal_audit_id()
            .expect("read generic proposal audit high-watermark"),
        Some(historical_id)
    );
    assert_eq!(
        recovered.proposal_coordinator.next_proposal_id.get(),
        historical_id.0
    );

    let registered = recovered
        .register_delegated_task_proposals(vec![delegated_output_from(
            save_proposal(ProposalId(100)),
            "after-generic-audit",
        )])
        .expect("delegated registration allocates above historical audit id");
    assert_eq!(registered[0].proposal_id, ProposalId(8));
    assert_eq!(recorder.events().expect("created event").len(), 1);

    fs::remove_dir_all(&workspace_root).expect("remove generic audit floor workspace");
}

#[cfg(feature = "ai")]
#[test]
fn delegated_proposal_replays_exact_durable_registration_after_interruption() {
    let workspace_root = unique_temp_dir("delegated-observation-replay");
    let recorder = legion_observability::InMemoryEventSink::new();
    let outputs = vec![
        delegated_output_from(save_proposal(ProposalId(100)), "first"),
        delegated_output_from(save_proposal(ProposalId(101)), "second"),
    ];

    let durable_batch = {
        let mut interrupted =
            AppComposition::with_event_sink(SharedEventSink::new(recorder.clone()));
        interrupted
            .enable_proposal_audit_persistence(&workspace_root)
            .expect("enable durable proposal observations");
        interrupted.interrupt_after_proposal_observation_store = true;
        let error = interrupted
            .register_delegated_task_proposals(outputs.clone())
            .expect_err("inject post-commit interruption");
        assert!(matches!(
            error,
            AppCompositionError::Protocol(ProtocolError { code, .. })
                if code == "proposal_observation_post_commit_interrupted"
        ));
        assert_eq!(
            interrupted.delegate_workflow.runtime_activation,
            DelegatedTaskRuntimeActivationState::Failed
        );
        assert!(
            interrupted
                .proposal_coordinator
                .proposal_ledger_projection(TimestampMillis(99))
                .rows
                .is_empty()
        );
        let orphan = interrupted
            .retry_pending_proposal_observations()
            .expect("report unpublished durable batch");
        assert_eq!(orphan.delivered_count, 0);
        assert_eq!(orphan.pending_count, 1);
        assert_eq!(
            orphan.attempts[0].error_code.as_deref(),
            Some("proposal_observation_publication_missing")
        );
        assert!(recorder.events().expect("no orphan events").is_empty());
        interrupted
            .storage
            .pending_proposal_observation_batches()
            .expect("durable pending batch")
            .into_iter()
            .next()
            .expect("one pending batch")
            .batch
    };

    let mut recovered = AppComposition::with_event_sink(SharedEventSink::new(recorder.clone()));
    recovered
        .enable_proposal_audit_persistence(&workspace_root)
        .expect("reopen durable proposal observations");
    let orphan = recovered
        .retry_pending_proposal_observations()
        .expect("restart must still refuse orphan delivery");
    assert_eq!(orphan.pending_count, 1);
    assert_eq!(
        orphan.attempts[0].error_code.as_deref(),
        Some("proposal_observation_publication_missing")
    );
    assert!(
        recorder
            .events()
            .expect("no restart orphan events")
            .is_empty()
    );

    let mut divergent = outputs.clone();
    let ProposalPayload::SaveFile(save) = &mut divergent[0].payload else {
        panic!("test payload must remain a save proposal");
    };
    save.file.canonical_path.0 = "C:/repo/file.txu".to_string();
    let error = recovered
        .register_delegated_task_proposals(divergent)
        .expect_err("same logical identities with changed payload must fail closed");
    assert!(matches!(
        error,
        AppCompositionError::Protocol(ProtocolError { code, .. })
            if code == "proposal_observation_replay_mismatch"
    ));
    assert!(
        recovered
            .proposal_coordinator
            .proposal_ledger_projection(TimestampMillis(99))
            .rows
            .is_empty()
    );

    let replayed = recovered
        .register_delegated_task_proposals(outputs.clone())
        .expect("exact caller-assisted replay");
    assert_eq!(
        replayed
            .iter()
            .map(|proposal| proposal.proposal_id)
            .collect::<Vec<_>>(),
        vec![ProposalId(1), ProposalId(2)]
    );
    assert_eq!(
        recovered
            .proposal_coordinator
            .proposal_ledger_projection(TimestampMillis(99))
            .rows
            .len(),
        2
    );
    let restored = recovered
        .storage
        .proposal_observation_batches()
        .expect("restored outbox record");
    assert_eq!(restored.len(), 1);
    assert_eq!(
        restored[0].delivery_state,
        legion_storage::ProposalObservationDeliveryState::Delivered
    );
    assert_eq!(restored[0].batch.batch_id, durable_batch.batch_id);
    assert_eq!(
        restored[0]
            .batch
            .events
            .iter()
            .map(|event| (event.event_id, event.occurred_at, event.sequence))
            .collect::<Vec<_>>(),
        durable_batch
            .events
            .iter()
            .map(|event| (event.event_id, event.occurred_at, event.sequence))
            .collect::<Vec<_>>()
    );
    assert_eq!(
        serde_json::to_value(&restored[0].batch.proposal_audits)
            .expect("serialize restored audits"),
        serde_json::to_value(&durable_batch.proposal_audits).expect("serialize original audits")
    );
    assert_eq!(
        recorder.events().expect("one atomic replay delivery").len(),
        2
    );

    let next = recovered
        .register_delegated_task_proposals(vec![delegated_output_from(
            save_proposal(ProposalId(200)),
            "after-restart",
        )])
        .expect("new registration allocates above durable floors");
    assert_eq!(next[0].proposal_id, ProposalId(3));
    let records = recovered
        .storage
        .proposal_observation_batches()
        .expect("all observation records");
    assert_eq!(records.len(), 2);
    let newest_sequence = records
        .iter()
        .flat_map(|record| &record.batch.events)
        .map(|event| event.sequence.0)
        .max()
        .expect("event sequence");
    assert!(newest_sequence > durable_batch.events[1].sequence.0);

    fs::remove_dir_all(&workspace_root).expect("remove replay test workspace");
}

#[test]
fn proposal_coordinator_builds_metadata_only_ledger_projection() {
    let coordinator = AppProposalCoordinator::new(SharedEventSink::default());
    let save = save_proposal(ProposalId(21));
    let terminal = proposal_with(ProposalId(22), "terminal.spawn", terminal_payload());
    register_created(&coordinator, &save);
    register_created(&coordinator, &terminal);

    let ledger = coordinator.proposal_ledger_projection(TimestampMillis(123));
    assert_eq!(ledger.rows.len(), 2);
    assert_eq!(ledger.selected_proposal_id, Some(ProposalId(22)));
    assert!(
        ledger
            .redaction_hints
            .contains(&RedactionHint::MetadataOnly)
    );

    let save_row = ledger
        .rows
        .iter()
        .find(|row| row.proposal_id == save.proposal_id)
        .expect("save row");
    assert_eq!(
        save_row.payload_kind,
        legion_protocol::ProposalPayloadKind::SaveFile
    );
    assert_eq!(save_row.workspace_id, Some(WorkspaceId(1)));
    assert!(save_row.diff_summary.full_source_redacted);
    assert_eq!(
        save_row.privacy_label,
        legion_protocol::ProposalPrivacyLabel::WorkspaceMetadata
    );

    let terminal_row = ledger
        .rows
        .iter()
        .find(|row| row.proposal_id == terminal.proposal_id)
        .expect("terminal row");
    assert_eq!(
        terminal_row.risk_label,
        legion_protocol::ProposalRiskLabel::High
    );
    assert_eq!(
        terminal_row.rollback,
        legion_protocol::ProposalRollbackAvailability::Unavailable
    );
    assert_eq!(
        terminal_row.diff_summary.kind,
        legion_protocol::ProposalDiffSummaryKind::TerminalMetadata
    );
}

#[test]
fn proposal_coordinator_allows_validated_denied_path() {
    let coordinator = AppProposalCoordinator::new(SharedEventSink::default());
    let proposal = save_proposal(ProposalId(11));
    register_created(&coordinator, &proposal);

    assert!(matches!(
        coordinator.handle(ProposalRequest::Validate(proposal.clone())),
        Ok(ProposalResponse::Validated(_))
    ));
    let transition = coordinator
        .record_transition_with_diagnostics(
            &proposal,
            ProposalLifecycleState::Denied,
            "validate",
            vec![AppProposalCoordinator::diagnostic(
                "proposal.validation_denied",
                "test validation denial",
            )],
        )
        .expect("validated proposal can deny");
    assert_eq!(transition.lifecycle_state, ProposalLifecycleState::Denied);
    assert!(
        transition
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "proposal.validation_denied")
    );
}

#[test]
fn proposal_coordinator_allows_approved_stale_conflict_and_failed_paths() {
    for (proposal_id, terminal_state) in [
        (ProposalId(12), ProposalLifecycleState::Stale),
        (ProposalId(13), ProposalLifecycleState::Conflict),
        (ProposalId(14), ProposalLifecycleState::Failed),
    ] {
        let coordinator = AppProposalCoordinator::new(SharedEventSink::default());
        let proposal = save_proposal(proposal_id);
        register_created(&coordinator, &proposal);
        assert!(matches!(
            coordinator.handle(ProposalRequest::Validate(proposal.clone())),
            Ok(ProposalResponse::Validated(_))
        ));
        assert!(matches!(
            coordinator.handle(ProposalRequest::Preview(proposal.clone())),
            Ok(ProposalResponse::Previewed { .. })
        ));
        assert!(matches!(
            coordinator.handle(ProposalRequest::Approve(command(
                proposal.proposal_id,
                legion_protocol::ProposalLifecycleAction::Approve,
            ))),
            Ok(ProposalResponse::Approved(_))
        ));

        let transition = coordinator
            .record_transition_with_diagnostics(
                &proposal,
                terminal_state,
                "apply",
                vec![AppProposalCoordinator::diagnostic(
                    "proposal.apply_terminal",
                    format!("test {terminal_state:?} terminal transition"),
                )],
            )
            .expect("approved proposal can enter terminal apply state");
        assert_eq!(transition.lifecycle_state, terminal_state);
        assert!(
            transition
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code == "proposal.apply_terminal")
        );
    }
}

#[test]
fn proposal_coordinator_rejects_created_to_applied_without_state_mutation() {
    let coordinator = AppProposalCoordinator::new(SharedEventSink::default());
    let proposal = save_proposal(ProposalId(15));
    register_created(&coordinator, &proposal);

    let response = coordinator
        .record_transition(&proposal, ProposalLifecycleState::Applied, "apply")
        .expect_err("created proposal cannot apply directly");
    assert_transition_diagnostic(&response, "proposal.invalid_lifecycle_transition");
    assert_eq!(
        coordinator.current_lifecycle_state(proposal.proposal_id),
        Some(ProposalLifecycleState::Created)
    );
}

#[test]
fn proposal_coordinator_rejects_expired_lifecycle_before_validation() {
    let coordinator = AppProposalCoordinator::new(SharedEventSink::default());
    let mut proposal = save_proposal(ProposalId(16));
    proposal.expires_at = Some(TimestampMillis(1));
    register_created(&coordinator, &proposal);

    let response = coordinator
        .handle(ProposalRequest::Validate(proposal.clone()))
        .expect("expired validate response");
    let ProposalResponse::Rejected { reason, .. } = &response else {
        panic!("expired proposal should reject, got {response:?}");
    };
    assert_eq!(*reason, ProposalRejectionReason::Expired);
    assert_transition_diagnostic(&response, "proposal.expired");
    assert_eq!(
        coordinator.current_lifecycle_state(proposal.proposal_id),
        Some(ProposalLifecycleState::Rejected)
    );
}

#[test]
fn proposal_coordinator_rejects_zero_correlation_or_nil_causality_context() {
    let coordinator = AppProposalCoordinator::new(SharedEventSink::default());
    let mut proposal = save_proposal(ProposalId(17));
    proposal.correlation_id = CorrelationId(0);
    coordinator.register_lifecycle_context(
        proposal.proposal_id,
        EventContext {
            correlation_id: CorrelationId(0),
            causality_id: CausalityId(uuid::Uuid::nil()),
        },
    );

    let response = coordinator.created_response(&proposal);
    assert_transition_diagnostic(&response, "proposal.invalid_lifecycle_context");
    assert_transition_diagnostic(&response, "proposal.zero_correlation_id");
    assert_transition_diagnostic(&response, "proposal.lifecycle_context_nil_causality_id");
    assert_eq!(
        coordinator.current_lifecycle_state(proposal.proposal_id),
        None
    );

    let coordinator = AppProposalCoordinator::new(SharedEventSink::default());
    let proposal = save_proposal(ProposalId(18));
    register_created(&coordinator, &proposal);
    assert!(matches!(
        coordinator.handle(ProposalRequest::Validate(proposal.clone())),
        Ok(ProposalResponse::Validated(_))
    ));
    assert!(matches!(
        coordinator.handle(ProposalRequest::Preview(proposal.clone())),
        Ok(ProposalResponse::Previewed { .. })
    ));
    let mut approve = command(
        proposal.proposal_id,
        legion_protocol::ProposalLifecycleAction::Approve,
    );
    approve.correlation_id = CorrelationId(0);
    approve.causality_id = CausalityId(uuid::Uuid::nil());

    let response = coordinator
        .handle(ProposalRequest::Approve(approve))
        .expect("invalid command context response");
    assert_transition_diagnostic(&response, "proposal.command_zero_correlation_id");
    assert_transition_diagnostic(&response, "proposal.command_nil_causality_id");
    assert_eq!(
        coordinator.current_lifecycle_state(proposal.proposal_id),
        Some(ProposalLifecycleState::Previewed)
    );
}

#[test]
fn proposal_coordinator_rejects_command_without_lifecycle_context() {
    let coordinator = AppProposalCoordinator::new(SharedEventSink::default());
    let response = coordinator
        .handle(ProposalRequest::Approve(command(
            ProposalId(99),
            legion_protocol::ProposalLifecycleAction::Approve,
        )))
        .expect("approve response");

    let ProposalResponse::Rejected { transition, reason } = response else {
        panic!("unknown lifecycle command should reject");
    };
    assert_eq!(reason, ProposalRejectionReason::ValidationFailed);
    assert!(
        transition
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "proposal.missing_lifecycle_context")
    );
}

#[test]
fn proposal_coordinator_denies_registered_text_edit_missing_preconditions() {
    let coordinator = AppProposalCoordinator::new(SharedEventSink::default());
    let proposal = text_edit_proposal(ProposalId(3));
    coordinator
        .register_lifecycle_context(proposal.proposal_id, EventContext::new(CorrelationId(1)));
    assert!(matches!(
        coordinator.created_response(&proposal),
        ProposalResponse::Created(_)
    ));

    let response = coordinator
        .handle(ProposalRequest::Validate(proposal))
        .expect("validate response");
    let ProposalResponse::Denied { transition, reason } = response else {
        panic!("registered text edit with missing preconditions should deny");
    };
    assert_eq!(reason, ProposalDenialReason::PolicyDenied);
    assert!(transition.diagnostics.iter().any(|diagnostic| {
        diagnostic.code == "proposal.missing_buffer_precondition"
            || diagnostic.code == "proposal.missing_file_precondition"
    }));
}

#[test]
fn proposal_coordinator_rejects_stateless_generic_save_apply() {
    let coordinator = AppProposalCoordinator::new(SharedEventSink::default());
    let proposal = save_proposal(ProposalId(2));
    coordinator
        .register_lifecycle_context(proposal.proposal_id, EventContext::new(CorrelationId(1)));
    assert!(matches!(
        coordinator.created_response(&proposal),
        ProposalResponse::Created(_)
    ));
    assert!(matches!(
        coordinator.handle(ProposalRequest::Validate(proposal.clone())),
        Ok(ProposalResponse::Validated(_))
    ));
    assert!(matches!(
        coordinator.handle(ProposalRequest::Preview(proposal.clone())),
        Ok(ProposalResponse::Previewed { .. })
    ));

    let response = coordinator
        .handle(ProposalRequest::Apply(proposal))
        .expect("apply response");
    let ProposalResponse::Rejected { transition, reason } = response else {
        panic!("stateless coordinator save apply should remain denied");
    };
    assert_eq!(reason, ProposalRejectionReason::Unsupported);
    assert!(transition.diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("use AppComposition::save_active_buffer")
    }));
}

#[test]
fn proposal_coordinator_discovers_targets_for_every_payload_variant() {
    let file = test_file(10, "C:/repo/file.rs");
    let rename_destination = CanonicalPath("C:/repo/renamed.rs".to_string());
    let cases = vec![
        (
            ProposalPayload::TextEdit(legion_protocol::TextEditProposal {
                file_id: FileId(10),
                edits: legion_protocol::EditBatch {
                    edits: vec![legion_protocol::TextEdit {
                        range: legion_protocol::TextRange::new(
                            legion_protocol::TextOffset::byte(0),
                            legion_protocol::TextOffset::byte(4),
                        ),
                        replacement: "edit".to_string(),
                    }],
                },
            }),
            vec![ProposalTargetKind::OpenBuffer],
        ),
        (
            ProposalPayload::CreateFile(legion_protocol::CreateFileProposal {
                path: CanonicalPath("C:/repo/new.rs".to_string()),
                initial_content: None,
            }),
            vec![ProposalTargetKind::PathOnly],
        ),
        (
            ProposalPayload::DeleteFile(legion_protocol::DeleteFileProposal { file: file.clone() }),
            vec![ProposalTargetKind::ClosedFile],
        ),
        (
            ProposalPayload::RenameFile(legion_protocol::RenameFileProposal {
                file: file.clone(),
                destination: rename_destination,
            }),
            vec![ProposalTargetKind::ClosedFile, ProposalTargetKind::PathOnly],
        ),
        (
            save_proposal(ProposalId(40)).payload,
            vec![ProposalTargetKind::OpenBuffer],
        ),
        (
            ProposalPayload::FormatFile(legion_protocol::FormatFileProposal {
                file: file.clone(),
                snapshot_id: legion_protocol::SnapshotId(1),
                options: HashMap::new(),
            }),
            vec![ProposalTargetKind::ClosedFile],
        ),
        (
            ProposalPayload::CodeAction(legion_protocol::CodeActionProposal {
                file: file.clone(),
                title: "fix".to_string(),
                edits: vec![legion_protocol::TextEdit {
                    range: legion_protocol::TextRange::new(
                        legion_protocol::TextOffset::byte(1),
                        legion_protocol::TextOffset::byte(2),
                    ),
                    replacement: "x".to_string(),
                }],
            }),
            vec![ProposalTargetKind::ClosedFile],
        ),
        (workspace_edit_payload(), vec![ProposalTargetKind::PathOnly]),
        (
            terminal_payload(),
            vec![ProposalTargetKind::TerminalSession],
        ),
        (
            ProposalPayload::Batch(BatchProposalPayload {
                batch_id: uuid::Uuid::now_v7(),
                atomicity: ProposalBatchAtomicity::OrderedNonAtomic,
                rollback_policy: ProposalBatchRollbackPolicy::NotSupported,
                target_coverage: ProposalTargetCoverage {
                    coverage_kind: ProposalTargetCoverageKind::Complete,
                    targets: Vec::new(),
                    omitted_target_count: 0,
                    redaction_hints: Vec::new(),
                },
                items: vec![ProposalBatchItem {
                    order: 0,
                    item_id: "create".to_string(),
                    payload: Box::new(ProposalPayload::CreateFile(
                        legion_protocol::CreateFileProposal {
                            path: CanonicalPath("C:/repo/batch.rs".to_string()),
                            initial_content: None,
                        },
                    )),
                    target_ids: Vec::new(),
                    required_capability: CapabilityId("fs.write".to_string()),
                    rollback_step_ids: Vec::new(),
                }],
                dependency_edges: Vec::new(),
                rollback_steps: Vec::new(),
                partial_failures: Vec::new(),
                preview_warnings: Vec::new(),
                schema_version: 1,
            }),
            vec![ProposalTargetKind::PathOnly],
        ),
    ];

    for (payload, expected_kinds) in cases {
        let coverage = AppProposalCoordinator::affected_target_coverage_for_payload(&payload);
        let actual_kinds = coverage
            .targets
            .iter()
            .map(|target| target.kind)
            .collect::<Vec<_>>();
        assert_eq!(coverage.coverage_kind, ProposalTargetCoverageKind::Complete);
        assert_eq!(coverage.omitted_target_count, 0);
        assert_eq!(actual_kinds, expected_kinds, "payload {payload:?}");
    }
}

#[test]
fn proposal_coordinator_denies_duplicate_ambiguous_and_unsupported_targets() {
    let coordinator = AppProposalCoordinator::new(SharedEventSink::default());
    let file = test_file(20, "C:/repo/dup.rs");
    let mut proposal = proposal_with(
        ProposalId(41),
        "fs.write",
        ProposalPayload::WorkspaceEdit(legion_protocol::WorkspaceEditProposalPayload {
            workspace_id: WorkspaceId(1),
            edit_id: uuid::Uuid::now_v7(),
            title: "duplicate targets".to_string(),
            source: legion_protocol::WorkspaceEditSourceKind::User,
            target_coverage: ProposalTargetCoverage {
                coverage_kind: ProposalTargetCoverageKind::Complete,
                targets: vec![
                    AppProposalCoordinator::file_identity_target(
                        "dup".to_string(),
                        ProposalTargetKind::ClosedFile,
                        &file,
                        None,
                        Vec::new(),
                    ),
                    AppProposalCoordinator::file_identity_target(
                        "dup".to_string(),
                        ProposalTargetKind::ClosedFile,
                        &file,
                        None,
                        Vec::new(),
                    ),
                ],
                omitted_target_count: 0,
                redaction_hints: Vec::new(),
            },
            file_edits: vec![legion_protocol::WorkspaceTextEdit {
                file,
                buffer_id: None,
                edits: legion_protocol::EditBatch { edits: Vec::new() },
                preconditions: complete_file_preconditions(),
            }],
            change_annotations: Vec::new(),
            file_operations: Vec::new(),
            required_capability: CapabilityId("fs.write".to_string()),
            diagnostics: Vec::new(),
            schema_version: 1,
        }),
    );
    register_created(&coordinator, &proposal);

    let response = coordinator
        .handle(ProposalRequest::Validate(proposal.clone()))
        .expect("validate duplicate targets");
    let ProposalResponse::Denied { transition, reason } = response else {
        panic!("duplicate targets should deny, got {response:?}");
    };
    assert_eq!(reason, ProposalDenialReason::PolicyDenied);
    assert!(
        transition
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "proposal.duplicate_target")
    );

    proposal.proposal_id = ProposalId(42);
    let ProposalPayload::WorkspaceEdit(payload) = &mut proposal.payload else {
        panic!("expected workspace-edit payload");
    };
    payload.target_coverage.targets = vec![ProposalAffectedTarget {
        target_id: "ambiguous".to_string(),
        kind: ProposalTargetKind::Plugin,
        workspace_id: Some(WorkspaceId(1)),
        file_id: Some(FileId(20)),
        buffer_id: None,
        path: None,
        terminal_session_id: None,
        plugin_id: Some(legion_protocol::PluginId(7)),
        remote_authority: None,
        collaboration_session_id: None,
        byte_ranges: Vec::new(),
        redaction_hints: Vec::new(),
    }];
    let coordinator = AppProposalCoordinator::new(SharedEventSink::default());
    register_created(&coordinator, &proposal);
    let response = coordinator
        .handle(ProposalRequest::Validate(proposal))
        .expect("validate ambiguous target");
    assert_transition_diagnostic(&response, "proposal.ambiguous_target");
    assert_transition_diagnostic(&response, "proposal.unsupported_target_kind");
}

#[test]
fn proposal_coordinator_denies_nested_batch_duplicates_and_unsupported_items() {
    let coordinator = AppProposalCoordinator::new(SharedEventSink::default());
    let create_path = CanonicalPath("C:/repo/batch-create.rs".to_string());
    let duplicate_target = AppProposalCoordinator::path_target(
        "target-create".to_string(),
        ProposalTargetKind::PathOnly,
        create_path.clone(),
        Vec::new(),
    );
    let proposal = proposal_with(
        ProposalId(43),
        "fs.write",
        ProposalPayload::Batch(BatchProposalPayload {
            batch_id: uuid::Uuid::now_v7(),
            atomicity: ProposalBatchAtomicity::OrderedNonAtomic,
            rollback_policy: ProposalBatchRollbackPolicy::NotSupported,
            target_coverage: ProposalTargetCoverage {
                coverage_kind: ProposalTargetCoverageKind::Complete,
                targets: vec![duplicate_target.clone(), duplicate_target],
                omitted_target_count: 0,
                redaction_hints: Vec::new(),
            },
            items: vec![
                ProposalBatchItem {
                    order: 0,
                    item_id: "create".to_string(),
                    payload: Box::new(ProposalPayload::CreateFile(
                        legion_protocol::CreateFileProposal {
                            path: create_path,
                            initial_content: None,
                        },
                    )),
                    target_ids: vec!["target-create".to_string(), "target-create".to_string()],
                    required_capability: CapabilityId("fs.write".to_string()),
                    rollback_step_ids: Vec::new(),
                },
                ProposalBatchItem {
                    order: 1,
                    item_id: "terminal".to_string(),
                    payload: Box::new(terminal_payload()),
                    target_ids: vec!["target-missing".to_string()],
                    required_capability: CapabilityId("terminal.execute".to_string()),
                    rollback_step_ids: Vec::new(),
                },
            ],
            dependency_edges: Vec::new(),
            rollback_steps: Vec::new(),
            partial_failures: Vec::new(),
            preview_warnings: Vec::new(),
            schema_version: 1,
        }),
    );
    register_created(&coordinator, &proposal);

    let response = coordinator
        .handle(ProposalRequest::Validate(proposal))
        .expect("validate batch");
    let ProposalResponse::Denied { transition, reason } = response else {
        panic!("invalid batch should deny, got {response:?}");
    };
    assert_eq!(reason, ProposalDenialReason::PolicyDenied);
    for expected in [
        "proposal.duplicate_target",
        "proposal.duplicate_batch_item_target",
        "proposal.unknown_batch_target",
        "proposal.unsupported_batch_item_route",
    ] {
        assert!(
            transition
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code == expected),
            "missing {expected}: {:?}",
            transition.diagnostics
        );
    }
}

#[test]
fn paths_equivalent_matches_real_file_with_alternate_separators() {
    let dir = std::env::temp_dir().join(format!(
        "legion_app_paths_equivalent_{}",
        std::process::id()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    let file_path = dir.join("test.txt");
    std::fs::write(&file_path, "hello").unwrap();

    let canonical = file_path.to_string_lossy();
    let forward = canonical.replace('\\', "/");
    let backward = canonical.replace('/', "\\");

    // On the current platform, at least one alternate form should be
    // equivalent to the canonical form via Path comparison or canonicalize.
    assert!(
        AppComposition::paths_equivalent(&canonical, &forward)
            || AppComposition::paths_equivalent(&canonical, &backward),
        "paths_equivalent should match a real file with alternate separators"
    );

    std::fs::remove_dir_all(&dir).unwrap();
}

/// SEARCH.06: frequency bonus lifts heavily-used palette commands.
///
/// "Preferences: Theme Dark" and "Preferences: Theme Light" score
/// identically for query "preferences theme". The alphabetical
/// tiebreaker puts Dark first. After recording 20 usages for Light, its
/// +100 frequency bonus lifts it within the canonical View group.
#[test]
fn palette_usage_frequency_bonus_lifts_heavily_used_command() {
    let workspace_id = WorkspaceId(42);
    let mut app = AppComposition::new();
    // Give the composition a workspace so `workspace_id()` returns `Some`.
    app.active_documents.opened_workspace = Some(WorkspaceOpened {
        workspace_id,
        root_id: legion_protocol::WorkspaceRootId(1),
        generation: WorkspaceGeneration(1),
        snapshot_id: legion_protocol::SnapshotId(0),
        correlation_id: CorrelationId(0),
    });

    let baseline = app.palette_command_results("preferences theme");
    let light_pos_base = baseline
        .iter()
        .position(|r| r.id == "command:preferences-theme-light")
        .expect("Theme Light should match 'preferences theme'");
    let dark_pos_base = baseline
        .iter()
        .position(|r| r.id == "command:preferences-theme-dark")
        .expect("Theme Dark should match 'preferences theme'");
    assert!(
        dark_pos_base <= light_pos_base,
        "Theme Dark should rank at least as high as Theme Light without a frequency boost"
    );

    // Record 20 usages for Theme Light -> +100 frequency bonus.
    for _ in 0..20 {
        app.palette_usage
            .record_usage(workspace_id, "command:preferences-theme-light");
    }

    let boosted = app.palette_command_results("preferences theme");
    let light_pos_boosted = boosted
        .iter()
        .position(|r| r.id == "command:preferences-theme-light")
        .expect("Theme Light should still match after boost");
    let dark_pos_boosted = boosted
        .iter()
        .position(|r| r.id == "command:preferences-theme-dark")
        .expect("Theme Dark should still match after boost");

    assert!(
        light_pos_boosted < dark_pos_boosted,
        "Theme Light (20 usages, +100 frequency bonus) must outrank Theme Dark within View"
    );
}

#[test]
fn cloud_lane_endpoint_parses_bracketed_ipv6_without_port() {
    let target =
        parse_cloud_lane_endpoint("https://[::1]").expect("bracketed IPv6 defaults to HTTPS");

    assert_eq!(target.scheme, "https");
    assert_eq!(target.host, "[::1]");
    assert_eq!(target.port, Some(443));
}

#[test]
fn cloud_lane_endpoint_parses_bracketed_ipv6_with_port() {
    let target =
        parse_cloud_lane_endpoint("https://[::1]:9443/path").expect("bracketed IPv6 with port");

    assert_eq!(target.scheme, "https");
    assert_eq!(target.host, "[::1]");
    assert_eq!(target.port, Some(9443));
}
