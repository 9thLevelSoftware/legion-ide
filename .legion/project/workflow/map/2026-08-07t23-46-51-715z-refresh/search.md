# Codebase Search Index

Use `legion map --query <text>` to search this deterministic index.

## .github/ISSUE_TEMPLATE/bug_report.md

.github/ISSUE_TEMPLATE/bug_report.md has 46 lines; headings: Summary, Environment, Reproduction steps; first content: ---
Symbols: none

## .github/workflows/legion-bench.yml

.github/workflows/legion-bench.yml has 46 lines; headings: Recorded mode does NOT open fixture repos or run agents. Task scores are, synthetic budget arithmetic (scoring_mode=synthetic_budget_arithmetic in the, report). This workflow proves report shape + verifier integrity, not product; first content: name: Legion Bench Recorded
Symbols: none

## .github/workflows/legion-gates.yml

.github/workflows/legion-gates.yml has 142 lines; headings: Standing gate set on every push to main and every pull request, across the, three supported OSes. Local gates remain the primary verification source, until this workflow has a proven stable history (see AGENTS.md); a red run; first content: name: Legion Gates
Symbols: none

## .github/workflows/legion-preview.yml

.github/workflows/legion-preview.yml has 126 lines; headings: WS-A-D Phase 4 D1: build portable unsigned-beta preview bundles of, legion-desktop on the 3-OS matrix. Independent of standing gates — failures, do not block PR merges unless added to branch protection later.; first content: name: Legion Preview
Symbols: none

## .github/workflows/legion-smoke.yml

.github/workflows/legion-smoke.yml has 304 lines; headings: GP-1/2/3/4 golden-path headless smoke: open fixture repo → edit Rust → diagnostics →, search → terminal cargo test → git workflow → evidence artifact (GP-1);, delegate mode scope/sandbox/kill/review flows (GP-2); full delegate task loop; first content: name: Legion Smoke
Symbols: none

## AGENTS.md

AGENTS.md has 19 lines; headings: AGENTS.md; first content: This file provides guidance to agents when working with code in this repository.
Symbols: none

## audit-reports/2026-07-13-release-readiness-codebase-map-and-gaps.md

audit-reports/2026-07-13-release-readiness-codebase-map-and-gaps.md has 253 lines; headings: Legion IDE — Full Codebase Map & Release-Readiness Gap Audit, 1. Headline verdict, Completeness map at a glance; first content: **Date:** 2026-07-13
Symbols: none

## audit-reports/external-security-audit-2026-06-13.md

audit-reports/external-security-audit-2026-06-13.md has 77 lines; headings: Audit Report: External Security Audit + Pen Test, Audit Metadata, Verdict; first content: > Internal security audit and pen-test summary for WS20.T4. This report records the current security gate results for th
Symbols: none

## audit-reports/manual-ui-e2e-audit-2026-06-02.md

audit-reports/manual-ui-e2e-audit-2026-06-02.md has 221 lines; headings: Audit Report: Manual Mode, Projection UI, and Deterministic IDE Flows, Audit Metadata, Feature Status Table; first content: > Historical pre-Legion-rename evidence. This file may contain `devil-*` crate names, old paths, or old commands. Do not
Symbols: none

## Cargo.toml

Cargo.toml has 124 lines; headings: PKT-UPDATER: Ed25519 verify for auto-updater client (ADR-0042)., BSD-3-Clause + MIT/Apache; same crate family already present via xtask., Internal crates (version managed by workspace); first content: [workspace]
Symbols: none

## config/workers.example.yaml

config/workers.example.yaml has 17 lines; first content: workers:
Symbols: none

## CONTRIBUTING.md

CONTRIBUTING.md has 64 lines; headings: Contributing to Legion IDE, Scope of this guide, Start here; first content: > **Proprietary codebase.** The source in this repository is proprietary software. All rights reserved. The workspace `p
Symbols: none

## crates/legion-agent/Cargo.toml

crates/legion-agent/Cargo.toml has 25 lines; first content: [package]
Symbols: none

## crates/legion-agent/src/agent_loop.rs

crates/legion-agent/src/agent_loop.rs has 1447 lines; symbols: abs_path, arguments, b, bound, candidate, canonical; first content: use std::path::{Path, PathBuf};
Symbols: abs_path, arguments, b, bound, candidate, canonical, cap_id, causality_id_str, causality_uuid, command, config, content, correlation_id_str, correlation_id_u64, correlation_id_uuid, decl_patterns, dir, end, end_line, ext, feedback_content, file_glob, file_name, final_message, forbidden, generator, glob, glob_matcher, grep, is_decl, limit, lines, matcher, max_bytes, max_symbols, mut, name, now_ms, Ok, p, path, path_opt, path_str, pattern, payload_variant, proposal, proposal_input, proposal_reason, proposal_title, re

## crates/legion-agent/src/budget.rs

crates/legion-agent/src/budget.rs has 182 lines; symbols: budget, mut, usage; first content: use legion_protocol::{
Symbols: budget, mut, usage

## crates/legion-agent/src/comm.rs

crates/legion-agent/src/comm.rs has 128 lines; symbols: line, parsed, rest, tag; first content: pub enum AgentCommTag {
Symbols: line, parsed, rest, tag

## crates/legion-agent/src/coordinator.rs

crates/legion-agent/src/coordinator.rs has 193 lines; symbols: dependency_edges, mut, predecessor, predecessor_index, session, successor; first content: use std::collections::HashMap;
Symbols: dependency_edges, mut, predecessor, predecessor_index, session, successor, worker_assignments, worker_ids_by_task

## crates/legion-agent/src/dag.rs

crates/legion-agent/src/dag.rs has 168 lines; symbols: dag, edges, mut; first content: use legion_protocol::{EditablePlanArtifact, EditablePlanSectionKind, TimestampMillis};
Symbols: dag, edges, mut

## crates/legion-agent/src/evidence.rs

crates/legion-agent/src/evidence.rs has 470 lines; symbols: artifact, audit, debug_output, digest, error, evidence; first content: use super::*;
Symbols: artifact, audit, debug_output, digest, error, evidence, mut, payload_hash, summary, summary_text, test_output, test_run, worker_id

## crates/legion-agent/src/external.rs

crates/legion-agent/src/external.rs has 306 lines; symbols: _, error, ExternalWorkspaceEditProposalInput, input, mut, proposal; first content: use legion_protocol::{
Symbols: _, error, ExternalWorkspaceEditProposalInput, input, mut, proposal

## crates/legion-agent/src/lib.rs

crates/legion-agent/src/lib.rs has 1839 lines; symbols: allowed_files, base, blocked_worker_ids, colon_prefix, command_output_refs, completed_worker_ids; first content: pub mod agent_loop;
Symbols: allowed_files, base, blocked_worker_ids, colon_prefix, command_output_refs, completed_worker_ids, conflicts, context_snippet_refs, coordinator, cost_budget_cents, create, denied, error, escaping_file, evidence, evidence_id, expected_id, first, first_meta, forbidden_files, forbidden_imports, full_file_refs, generator, locality_preference, manifest, metadata, missing, modified, modified_content, mut, objective_summary_hash, output, output_contract, packet, packet_id, path, permission, policy, prefix, proposal, ready, ready_ids, replayed, result, result_id, route_a, route_b, route_health, route_id, route_ref

## crates/legion-agent/src/merge_readiness.rs

crates/legion-agent/src/merge_readiness.rs has 76 lines; symbols: mut; first content: use super::*;
Symbols: mut

## crates/legion-agent/src/plan.rs

crates/legion-agent/src/plan.rs has 214 lines; symbols: directive, mut, plan, sections, spec, task_graph; first content: use legion_protocol::{
Symbols: directive, mut, plan, sections, spec, task_graph

## crates/legion-agent/src/scheduler.rs

crates/legion-agent/src/scheduler.rs has 104 lines; symbols: lane, lane_ids, mut, worker_lookup; first content: use crate::AgentError;
Symbols: lane, lane_ids, mut, worker_lookup

## crates/legion-agent/src/scope.rs

crates/legion-agent/src/scope.rs has 73 lines; symbols: candidate; first content: use std::path::Path;
Symbols: candidate

## crates/legion-agent/src/state.rs

crates/legion-agent/src/state.rs has 187 lines; symbols: mut, transition; first content: use thiserror::Error;
Symbols: mut, transition

## crates/legion-agent/src/tools.rs

crates/legion-agent/src/tools.rs has 13 lines; first content: pub use legion_protocol::tools::{
Symbols: none

## crates/legion-agent/src/worktree.rs

crates/legion-agent/src/worktree.rs has 1203 lines; symbols: _, acquired_lease, base_absolute, base_state, base_stripped, canonical_sandbox; first content: use std::path::{Path, PathBuf};
Symbols: _, acquired_lease, base_absolute, base_state, base_stripped, canonical_sandbox, canonical_workspace, clean_lexical, clean_path, clean_stripped, content_hash, create_payload, delegated_runtime_capability, delegated_tasks_root, destination_path, entry, file_name, file_type, FNV_OFFSET_BASIS, FNV_PRIME, lease_path, message, metadata, modified_at, msg, mut, name, non_absent_kinds, orchestrator, output, p_str, path, path_absolute, permission, preview_summary, publish_result, relative, result, sandbox_action, sandbox_path, sandbox_root, Some, source_path, source_root, strip_unc, target_relative, tmp, workspace_root, write_profile

## crates/legion-agent/tests/agent_loop_integration.rs

crates/legion-agent/tests/agent_loop_integration.rs has 995 lines; symbols: bad_input, cancel, cap_id, config, dir, executed_count; first content: use std::path::Path;
Symbols: bad_input, cancel, cap_id, config, dir, executed_count, exhausted, first_targets_a, has_result, indices, mut, n, outside_dir, outside_path, paired_result, proposal, provider, rejected_count, req, req_cids, req_count, request_causality_ids, res_cids, res_count, result, root, second_targets_b, seqs, Some, targets_main_rs, tool_host

## crates/legion-agent/tests/comm.rs

crates/legion-agent/tests/comm.rs has 39 lines; symbols: parsed; first content: use legion_agent::comm::{AgentCommTag, parse_agent_comm_line};
Symbols: parsed

## crates/legion-agent/tests/containment_canonicalization.rs

crates/legion-agent/tests/containment_canonicalization.rs has 135 lines; symbols: _, alias, alias_parent, dir, gone, link; first content: use std::fs;
Symbols: _, alias, alias_parent, dir, gone, link, outside, real_root, relative, result, sandbox

## crates/legion-agent/tests/coordinator.rs

crates/legion-agent/tests/coordinator.rs has 142 lines; symbols: dag, edge, error, graph, mut, plan; first content: use legion_agent::coordinator::{
Symbols: dag, edge, error, graph, mut, plan, session

## crates/legion-agent/tests/dag.rs

crates/legion-agent/tests/dag.rs has 55 lines; symbols: dag, mut; first content: use legion_agent::dag::workflow_dag_from_approved_plan;
Symbols: dag, mut

## crates/legion-agent/tests/merge_readiness.rs

crates/legion-agent/tests/merge_readiness.rs has 155 lines; symbols: coordinator, readiness, report, row; first content: use legion_agent::LegionWorkflowCoordinator;
Symbols: coordinator, readiness, report, row

## crates/legion-agent/tests/openai_tool_loop_cross_check.rs

crates/legion-agent/tests/openai_tool_loop_cross_check.rs has 268 lines; symbols: cap_id, config, dir, has_result, mut, provider; first content: use std::collections::VecDeque;
Symbols: cap_id, config, dir, has_result, mut, provider, request_cids, result, root, transport

## crates/legion-agent/tests/plan_artifact.rs

crates/legion-agent/tests/plan_artifact.rs has 205 lines; symbols: audit_row, current_plan, directive, plan, previous_plan, revision; first content: use legion_agent::plan::editable_plan_from_workflow_artifacts;
Symbols: audit_row, current_plan, directive, plan, previous_plan, revision, spec, task_graph

## crates/legion-agent/tests/sandbox_reaping.rs

crates/legion-agent/tests/sandbox_reaping.rs has 141 lines; symbols: _, holder, lease_path, mut, probe_result, removed; first content: use legion_agent::reap_orphaned_sandboxes;
Symbols: _, holder, lease_path, mut, probe_result, removed, root

## crates/legion-agent/tests/scheduler.rs

crates/legion-agent/tests/scheduler.rs has 119 lines; symbols: lane_ids, lanes; first content: use legion_agent::scheduler::parallel_worker_lanes;
Symbols: lane_ids, lanes

## crates/legion-agent/tests/scope_enforcement.rs

crates/legion-agent/tests/scope_enforcement.rs has 69 lines; symbols: err, feedback, scope; first content: use legion_agent::{
Symbols: err, feedback, scope

## crates/legion-agent/tests/tools_schema.rs

crates/legion-agent/tests/tools_schema.rs has 66 lines; symbols: expected, registry, required, tool; first content: use legion_agent::tools::native_tool_registry;
Symbols: expected, registry, required, tool

## crates/legion-agent/tests/worktree_sandbox.rs

crates/legion-agent/tests/worktree_sandbox.rs has 274 lines; symbols: _, delegated_tasks_root, lease_path, missing_source_root, mut, nanos; first content: use legion_agent::{DelegatedTaskSandboxOrchestrator, reap_orphaned_sandboxes};
Symbols: _, delegated_tasks_root, lease_path, missing_source_root, mut, nanos, path, permission, probe, result, sandbox_path, sandbox_root, source_root, unique

## crates/legion-ai-providers/Cargo.toml

crates/legion-ai-providers/Cargo.toml has 16 lines; first content: [package]
Symbols: none

## crates/legion-ai-providers/src/bin/mcp_stdio_fixture.rs

crates/legion-ai-providers/src/bin/mcp_stdio_fixture.rs has 116 lines; symbols: line, method, mode, mut, prompts, request; first content: use std::io::{self, BufRead, Write};
Symbols: line, method, mode, mut, prompts, request, resources, response, result, spec, stdin, stdout, tools

## crates/legion-ai-providers/src/capabilities.rs

crates/legion-ai-providers/src/capabilities.rs has 113 lines; symbols: matrix; first content: use legion_protocol::{
Symbols: matrix

## crates/legion-ai-providers/src/lib.rs

crates/legion-ai-providers/src/lib.rs has 5721 lines; symbols: _, allow, allow_for_other_capability, allow_for_other_tool, answer_fingerprint, answer_label; first content: pub mod capabilities;
Symbols: _, allow, allow_for_other_capability, allow_for_other_tool, answer_fingerprint, answer_label, anthropic, ANTHROPIC_API_VERSION, anthropic_calls, anthropic_response, ANTHROPIC_STRUCTURED_OUTPUTS_BETA, anthropic_transport, api_calls, api_key, api_key_client, api_key_transport, arguments, arguments_str, array, assistant_msg, assistant_turn, available, base_url, batch_job_type, batch_request, BatchJobRequest, bearer_token, beta, body, calls, candidates, capabilities, choice, client, completion, confirm, content, credential, data, deadline, deltas, deterministic, DIMENSIONS, discovered, embeddings, err, error, existing, expected_id, expected_target_id

## crates/legion-ai-providers/tests/mcp_ga_conformance.rs

crates/legion-ai-providers/tests/mcp_ga_conformance.rs has 596 lines; symbols: base_transport, body, client, client_a, client_b, content_length; first content: use std::io::{ErrorKind, Read, Write};
Symbols: base_transport, body, client, client_a, client_b, content_length, deadline, endpoint, expected_target, handle, header_end, header_text, HTTP_FIXTURE_DEADLINE, HTTP_FIXTURE_STREAM_TIMEOUT, labels, list_prompts, list_resources, list_tools, listener, lower, method, mut, payload, permission, pid_a, pid_b, prompt, read, registry, reloaded, request, requests, requests_thread, resource, response, result, server_id, server_id_name, tool, transport

## crates/legion-ai-providers/tests/prompt_stability.rs

crates/legion-ai-providers/tests/prompt_stability.rs has 126 lines; symbols: first, first_value, json, mut, request, second; first content: use std::hash::{Hash, Hasher};
Symbols: first, first_value, json, mut, request, second, second_value

## crates/legion-ai-providers/tests/provider_activation.rs

crates/legion-ai-providers/tests/provider_activation.rs has 383 lines; symbols: byok_result, cases, consent, copilot_row, denied, empty_matrix; first content: use legion_ai_providers::{
Symbols: byok_result, cases, consent, copilot_row, denied, empty_matrix, gated, granted, hosted_result, local_row, matrix, ollama_row, result, rows, tier

## crates/legion-ai-providers/tests/provider_smoke.rs

crates/legion-ai-providers/tests/provider_smoke.rs has 229 lines; symbols: available, base_url, client, deltas, events, fixture; first content: use std::path::PathBuf;
Symbols: available, base_url, client, deltas, events, fixture, json_response, model, provider, request, response, sse_body, text, transport

## crates/legion-ai-providers/tests/smoke.rs

crates/legion-ai-providers/tests/smoke.rs has 97 lines; symbols: completion, embedding_input, embeddings, fixture, hosted, local; first content: use std::{fs, path::PathBuf};
Symbols: completion, embedding_input, embeddings, fixture, hosted, local, model, prompt, provider, provider_id, request, response, text, tokens

## crates/legion-ai/Cargo.toml

crates/legion-ai/Cargo.toml has 19 lines; first content: [package]
Symbols: none

## crates/legion-ai/src/classifier.rs

crates/legion-ai/src/classifier.rs has 65 lines; first content: use legion_protocol::ProposalRiskLabel;
Symbols: none

## crates/legion-ai/src/lib.rs

crates/legion-ai/src/lib.rs has 1338 lines; symbols: base, broker, byte_len, capabilities, capability_response, chat_error; first content: pub mod classifier;
Symbols: base, broker, byte_len, capabilities, capability_response, chat_error, completion, decoded, error, ghost_text, id, insert_range, json, line, line_count, mut, policy, prompt, provider, provider_metadata, refusal, registry, request, response, result, router, Some, text

## crates/legion-ai/src/manifest.rs

crates/legion-ai/src/manifest.rs has 447 lines; symbols: all_items, assembly, FNV_OFFSET, FNV_PRIME, manifest_id, mut; first content: use legion_protocol::{
Symbols: all_items, assembly, FNV_OFFSET, FNV_PRIME, manifest_id, mut, omitted_item_count, stale_or_missing_metadata_risk_present

## crates/legion-ai/src/redaction.rs

crates/legion-ai/src/redaction.rs has 124 lines; symbols: bound, limit, mut, payload, REDACTED, result; first content: use std::sync::OnceLock;
Symbols: bound, limit, mut, payload, REDACTED, result, scan, specs, truncated

## crates/legion-ai/src/streaming.rs

crates/legion-ai/src/streaming.rs has 182 lines; symbols: flush_code, flush_text, language, mut, segments, trimmed; first content: pub enum MarkdownStreamSegment {
Symbols: flush_code, flush_text, language, mut, segments, trimmed

## crates/legion-ai/src/telemetry.rs

crates/legion-ai/src/telemetry.rs has 286 lines; symbols: detail_level, metadata, policy, record, result; first content: use legion_observability::telemetry::{
Symbols: detail_level, metadata, policy, record, result

## crates/legion-ai/src/tool_calls.rs

crates/legion-ai/src/tool_calls.rs has 441 lines; symbols: cursor, err, expect, found, msg, provider; first content: use serde::{Deserialize, Serialize};
Symbols: cursor, err, expect, found, msg, provider, resp1, resp2, resp3, ToolTurnBlock, turn

## crates/legion-ai/tests/advisory_classifier.rs

crates/legion-ai/tests/advisory_classifier.rs has 32 lines; symbols: classifier, recommendation; first content: use legion_ai::classifier::{AdvisoryRiskClassifier, RiskClassifierRecommendation};
Symbols: classifier, recommendation

## crates/legion-ai/tests/context_manifest.rs

crates/legion-ai/tests/context_manifest.rs has 321 lines; symbols: assembly, items, items1, items2, meta, paths; first content: use legion_ai::{
Symbols: assembly, items, items1, items2, meta, paths, record, record1, record2, sources, sources1, sources2

## crates/legion-ai/tests/egress_equality.rs

crates/legion-ai/tests/egress_equality.rs has 494 lines; symbols: actual_bytes, actual_text, assembly, assembly_a, bytes_a, bytes_b; first content: use legion_ai::{
Symbols: actual_bytes, actual_text, assembly, assembly_a, bytes_a, bytes_b, bytes_c, egress, egress_items_a, egress_items_b, egress_items_c, egress_text, expected_bytes, expected_egress, expected_kinds, items, manifest, mut, paths, record, record_a, record_b, record_c, sources_b

## crates/legion-ai/tests/redaction.rs

crates/legion-ai/tests/redaction.rs has 46 lines; symbols: output, redacted; first content: use legion_ai::redaction::redact_model_bound_output;
Symbols: output, redacted

## crates/legion-ai/tests/streaming.rs

crates/legion-ai/tests/streaming.rs has 44 lines; symbols: segments; first content: use legion_ai::streaming::{MarkdownStreamSegment, split_markdown_stream};
Symbols: segments

## crates/legion-ai/tests/telemetry.rs

crates/legion-ai/tests/telemetry.rs has 168 lines; symbols: blocked, coord, envelope, metadata, policy_no_consent, policy_with_consent; first content: use legion_ai::telemetry::{
Symbols: blocked, coord, envelope, metadata, policy_no_consent, policy_with_consent, record, result

## crates/legion-app/Cargo.toml

crates/legion-app/Cargo.toml has 98 lines; headings: offline: no hosted provider calls at runtime.  We still compile legion-ai, (pure Rust, no network deps) so that the deterministic-local inline, prediction provider is available.  The policy layer prevents actual remote; first content: [package]
Symbols: none

## crates/legion-app/src/bin/golden_path_1.rs

crates/legion-app/src/bin/golden_path_1.rs has 1659 lines; symbols: _, _x, any_failed, append_edit, args, at_rest_text; first content: use std::{
Symbols: _, _x, any_failed, append_edit, args, at_rest_text, bat_content, bat_path, binary_path, buffer_id, buffered, clear_projection, clear_raw, cmd, command, committed, config, cs_lower_count, cs_lower_projection, cs_lower_query, cs_lower_result, cwd, d, days, deadline, detail, discovery, doe, doy, drive, dst_path, entry, era, error_edit, error_last_col, error_last_line, error_lines, error_projection, error_raw, ev_path, exit_ok, expected_hash, finished_utc, first_result, fix_edit, forward, ft, git_projection, h, hit

## crates/legion-app/src/bin/golden_path_2.rs

crates/legion-app/src/bin/golden_path_2.rs has 1310 lines; symbols: _, accept_outcome, accept_projection, accepted_text, active, any_failed; first content: use std::{
Symbols: _, accept_outcome, accept_projection, accepted_text, active, any_failed, apply_resp, args, broker, buffer_id, canonical, checkpoint_id, checkpoints, current_gen, cwd, d, days, detail, doe, doy, dst_path, entry, era, ev_path, file_id, file_items, finished_utc, first_result, ft, h, legion_sha, local_request, local_response, m, main_rs, main_rs_path, main_rs_str, manifest, metadata, mon, mp, mut, nanos, now, opened, original_text, out_path, outcome, output, overall_status

## crates/legion-app/src/bin/golden_path_3.rs

crates/legion-app/src/bin/golden_path_3.rs has 1246 lines; symbols: _, any_failed, apply_resp, args, checkpoint_id, checkpoints; first content: use std::{
Symbols: _, any_failed, apply_resp, args, checkpoint_id, checkpoints, current_gen, cwd, d, days, detail, doe, doy, dst_path, entry, era, ev_path, files, finished_utc, first_result, flag, ft, h, hunk_id, legion_sha, m, main_fingerprint, mon, mp, mut, name, name_str, nanos, now, out_path, outcome, output, overall_status, path, post_fingerprint, preview_resp, proposal, proposal_id, provider, reap_root, register_resp, rejected, rem, removed, requests

## crates/legion-app/src/bin/golden_path_4.rs

crates/legion-app/src/bin/golden_path_4.rs has 1873 lines; symbols: _, args, barrier, blocked, bundle, cancellation_flag; first content: use std::{
Symbols: _, args, barrier, blocked, bundle, cancellation_flag, cancelled_at, cites_evidence, conflict_id, cwd, d, days, dependent, dependent_dispatch, detail, dispatch_log, doe, doy, dst_path, duration_ms, entry, era, err, ev_path, exhausted, finished, finished_utc, first, ft, h, independent_plan_id, latest, left, left_pass, legion_sha, log, m, mon, mp, mut, nanos, now, out_path, outcome, output, overall_status, paused, plan, plan_id, proposal_count

## crates/legion-app/src/bin/update_drill.rs

crates/legion-app/src/bin/update_drill.rs has 1022 lines; symbols: _, any_failed, applied_journal, args, artifact, artifact_bytes; first content: use std::{
Symbols: _, any_failed, applied_journal, args, artifact, artifact_bytes, artifact_bytes_v2, bad_dir, check, d, days, detail, doe, double_rolled, doy, e, era, finished_utc, git_sha, h, journal_path, m, manifest, manifest_toml, manifest_toml_bytes, manifest_v2, mon, mp, mut, nanos, ok, output, overall_status, pid, policy, rem, report_path, report_result, report_tmp, result, s, s1_end, s1_start, s10_end, s10_start, s11_end, s11_start, s2, s2_dir, s2_end

## crates/legion-app/src/diagnostics.rs

crates/legion-app/src/diagnostics.rs has 225 lines; symbols: assembler, bundle, content, crash_dir, dir, line; first content: use std::path::PathBuf;
Symbols: assembler, bundle, content, crash_dir, dir, line, mut, prefix, raw, result, rows, summary

## crates/legion-app/src/first_run.rs

crates/legion-app/src/first_run.rs has 60 lines; symbols: consent, mut; first content: use legion_protocol::WorkbenchTelemetryConsent;
Symbols: consent, mut

## crates/legion-app/src/language/app_lsp.rs

crates/legion-app/src/language/app_lsp.rs has 1417 lines; symbols: _, changed, command, config, delay_ms, earliest_retry_ms; first content: use std::{
Symbols: _, changed, command, config, delay_ms, earliest_retry_ms, guard, handle, health, identity, language_id, launch_policy, lines, LspSessionState, mut, nanos, normalized, NOTIFICATION_POLL_INTERVAL, now, now_ms, Ok, outcome, posture, proj, raw_line, reader, reason, redacted, remaining, request, resolved_discovery, result, result_tx, results, ring, root, root_path, root_uri, sentinel, server_id, status, STDERR_LINE_MAX_LEN, stderr_ring, STDERR_RING_CAPACITY, supervisor, truncated, worker, workspace_id

## crates/legion-app/src/language/download.rs

crates/legion-app/src/language/download.rs has 156 lines; symbols: context, expected, mut; first content: use legion_protocol::{
Symbols: context, expected, mut

## crates/legion-app/src/language/mod.rs

crates/legion-app/src/language/mod.rs has 94 lines; first content: mod download;
Symbols: none

## crates/legion-app/src/language/proposal.rs

crates/legion-app/src/language/proposal.rs has 64 lines; first content: use legion_protocol::{
Symbols: none

## crates/legion-app/src/language/redaction.rs

crates/legion-app/src/language/redaction.rs has 346 lines; symbols: at_word_start, chars, is_home_path, is_unc_path, is_unix_path, is_windows_path; first content: pub struct StderrSummary {
Symbols: at_word_start, chars, is_home_path, is_unc_path, is_unix_path, is_windows_path, line, mut, out, sentinel, upper

## crates/legion-app/src/language/session.rs

crates/legion-app/src/language/session.rs has 634 lines; symbols: _, attempt, buffered_clean, buffered_error, caps, ctx; first content: use std::collections::VecDeque;
Symbols: _, attempt, buffered_clean, buffered_error, caps, ctx, deadline, expected_hash, has_buffered, health, LINE_MAX_LEN, mut, Ok, options, outcome, params, provenance, reader, redacted, response, ring, RING_CAPACITY, session, Some, supported, truncated

## crates/legion-app/src/language/translate.rs

crates/legion-app/src/language/translate.rs has 1309 lines; symbols: after_alpha, after_beta, alpha_path, alpha_text, alpha_uri, before_alpha; first content: use legion_protocol::{
Symbols: after_alpha, after_beta, alpha_path, alpha_text, alpha_uri, before_alpha, before_beta, beta_path, beta_text, beta_uri, canonical, changes, changes_obj, current, dest_path, dir, doc, edit, edit_a, edit_b, edits_array, edits_json, end_byte, end_char, end_line, err, expected_fingerprint_a, expected_fingerprint_b, file, file_a_uri, file_b_uri, kind, lsp_version, mut, native, new_text, new_uri, obj, old_uri, op, path, payload, preconditions, range, raw, remaining, resolver, rest, rest_bytes, snap

## crates/legion-app/src/main.rs

crates/legion-app/src/main.rs has 79 lines; symbols: explicit_path, file_id, mut, path, root; first content: use std::env;
Symbols: explicit_path, file_id, mut, path, root

## crates/legion-app/src/offline_ai.rs

crates/legion-app/src/offline_ai.rs has 2460 lines; symbols: _, acquired_lease, allowed_files, base_absolute, base_stripped, blocked_worker_ids; first content: use legion_protocol::{
Symbols: _, acquired_lease, allowed_files, base_absolute, base_stripped, blocked_worker_ids, broker, capability_ok, clean_path, clean_stripped, command_output_refs, completed_worker_ids, conflicts, context_snippet_refs, cost_budget_cents, current_dir, delegated_tasks_root, edges, entry, error, evidence, expected_id, file_name, forbidden_files, full_file_refs, holder, lease_path, locality_preference, message, metadata, mut, name, non_absent_kinds, objective_summary_hash, output, output_contract, packet, packet_id, path, path_absolute, path_string, permission, policy, predecessor, predecessor_index, probe, publish_result, refusal, registry, removed

## crates/legion-app/src/proposal.rs

crates/legion-app/src/proposal.rs has 553 lines; symbols: accepted, accepted_target_ids, batch_item_drop, batch_item_keep, coverage, filtered; first content: use std::collections::HashMap;
Symbols: accepted, accepted_target_ids, batch_item_drop, batch_item_keep, coverage, filtered, filtered_items, hunk_id, hunks, mut, payload, policy, previous, proposal, ProposalPayload, retained_item_ids, retained_target_ids, risk_rule_ids, section, Some, target_drop, target_keep, targets

## crates/legion-app/src/terminal_policy.rs

crates/legion-app/src/terminal_policy.rs has 333 lines; symbols: deny_prefixes, env, env2, is_denied, keys, ku; first content: pub const SCROLLBACK_DEFAULT_MAX_ROWS: usize = 5_000;
Symbols: deny_prefixes, env, env2, is_denied, keys, ku, PLATFORM_BASELINE_KEYS, policy, TEST_NON_BASELINE_KEY

## crates/legion-app/src/test_explorer.rs

crates/legion-app/src/test_explorer.rs has 709 lines; symbols: _, base, cmd_l, completed_at, duration_ms, exit_code; first content: use std::path::Path;
Symbols: _, base, cmd_l, completed_at, duration_ms, exit_code, failed, id, is_runnable, items, kind, kind_l, label, lenses, line, MAX_PARSE_STDOUT_BYTES, mut, name, omitted, out, output, parent_label, passed, projection, result, run_id, skipped, Some, started, state, status, status_label, stdout, summary, title_l, tokens, truncated

## crates/legion-app/src/updater.rs

crates/legion-app/src/updater.rs has 600 lines; symbols: actual, bytes, dst, existing, file, journal; first content: use std::{
Symbols: actual, bytes, dst, existing, file, journal, key_bytes, major, manifest, manifest_bytes, manifest_path, manifest_str, minor, parts, patch, prev, previous_version, rolled, sig, sig_bytes, sig_path, signer_status, src, src_path, staged_dir, text, tmp, toml_text, triple, vk, wrapper

## crates/legion-app/tests/app_lsp_composition.rs

crates/legion-app/tests/app_lsp_composition.rs has 770 lines; symbols: add_params, after_hash, before_hash, buffer_id, capabilities, changed; first content: use std::time::Duration;
Symbols: add_params, after_hash, before_hash, buffer_id, capabilities, changed, clear_params, content, deadline, deadline2, deadline3, dir, handle, hash, health, health_records, len, lsp_count, lsp_problems, mut, Ok, ops, p1, p2, params, path, pos, problem, projection, reason, root, snap, snap1, snap2, snap2_lsp_count, snapshot, Some, src_file, uri

## crates/legion-app/tests/apply_activation.rs

crates/legion-app/tests/apply_activation.rs has 838 lines; symbols: _, apply_response, audit_state, canonical, contract, dest_path; first content: use std::path::{Path, PathBuf};
Symbols: _, apply_response, audit_state, canonical, contract, dest_path, fingerprint, journal, mut, node, opened, plan, policy, proposal, response, root, source_path, target, target_id, target_path, validate_response

## crates/legion-app/tests/assist_inline_prediction_workflow.rs

crates/legion-app/tests/assist_inline_prediction_workflow.rs has 319 lines; symbols: _, accepted, active, assist_projection, buffer_id, buffer_version; first content: use std::sync::atomic::{AtomicU64, Ordering};
Symbols: _, accepted, active, assist_projection, buffer_id, buffer_version, current, error, manual_projection, mut, prediction_id, projected, projection, root, snapshot_id, stale, stale_row, target, text, tx_log, undo

## crates/legion-app/tests/broker_fixture/mod.rs

crates/legion-app/tests/broker_fixture/mod.rs has 28 lines; symbols: capability_id; first content: use legion_protocol::{
Symbols: capability_id

## crates/legion-app/tests/checkpoint_restore.rs

crates/legion-app/tests/checkpoint_restore.rs has 726 lines; symbols: _, audit_dir, audit_files, audits_after, audits_after_apply, audits_after_restore; first content: use std::sync::atomic::{AtomicU64, Ordering};
Symbols: _, audit_dir, audit_files, audits_after, audits_after_apply, audits_after_restore, audits_before, buffer_id, checkpoint, checkpoint_dir, checkpoint_id, checkpoints, checkpoints_after, ckpt, ckpt_id, ckpt_id_1001, current_gen, error, events, file_id, file1, file2, file3, file4, fingerprint, middle, middle_id, mut, node, opened, outside, proposal, proposal_id, response, result, rollback, root, row, shell, summary, target, target_path, unrelated, updated

## crates/legion-app/tests/commit_validation_workflow.rs

crates/legion-app/tests/commit_validation_workflow.rs has 387 lines; symbols: _, file_name, id, log, log_output, msg; first content: use std::{
Symbols: _, file_name, id, log, log_output, msg, mut, nanos, outcome, path, projection, repo, result, root, tmp

## crates/legion-app/tests/control_trust_surfaces.rs

crates/legion-app/tests/control_trust_surfaces.rs has 1000 lines; symbols: _, applied, applied_buffer_id, applied_file_id, applied_node, applied_response; first content: use std::{
Symbols: _, applied, applied_buffer_id, applied_file_id, applied_node, applied_response, applied_snapshot, applied_target, apply, approve, before_disk, before_editor, buffer_id, cancel, conflict, conflict_buffer_id, conflict_file_id, conflict_fingerprint, conflict_node, conflict_response, conflict_snapshot, conflict_target, details, error, failed, failed_buffer_id, failed_file_id, failed_node, failed_response, failed_snapshot, failed_target, file_id, fingerprint, first, mut, node, opened, outcome, path, preview, proposal_id, reject, reject_buffer_id, reject_file_id, reject_node, reject_response, reject_snapshot, reject_target, response, rollback_response

## crates/legion-app/tests/daily_editing_contracts.rs

crates/legion-app/tests/daily_editing_contracts.rs has 602 lines; symbols: _, cases, clean, clean_buffer, clean_item, close_clean; first content: use std::sync::atomic::{AtomicU64, Ordering};
Symbols: _, cases, clean, clean_buffer, clean_item, close_clean, close_dirty, conflicted, conflicted_buffer, dirty, dirty_body, dirty_buffer, file_name, first, first_buffer, markdown_buffer, markdown_path, markdown_projection, markdown_viewport, memory_snapshot_json, metadata, mut, outcome, path, projected, projection, record, rejected_item, restored, root, rust_buffer, rust_path, rust_projection, rust_slashes, rust_source, rust_viewport, second, second_buffer, serialized_shape, snapshot, target, Target, temp_root, toml_hash, toml_path, toml_source, toml_viewport, viewport

## crates/legion-app/tests/daily_editing_search.rs

crates/legion-app/tests/daily_editing_search.rs has 350 lines; symbols: _, case_target, case_workspace, file_name, first, git_init; first content: use std::{
Symbols: _, case_target, case_workspace, file_name, first, git_init, id, included, mut, nanos, nocase_projection, oversized, path, projection, regex_projection, regex_target, regex_workspace, root, second, snapshot, target, temp_root, word_projection, word_target, word_workspace, workspace

## crates/legion-app/tests/debug_workflow.rs

crates/legion-app/tests/debug_workflow.rs has 616 lines; symbols: buffer_id, configs, configuration_id, continued, deadline, first_buffer; first content: use std::{
Symbols: buffer_id, configs, configuration_id, continued, deadline, first_buffer, first_config_id, first_root, first_source, mut, original_text, polled, projection, root, second_root, second_source, session_id, source, stepped, stopped

## crates/legion-app/tests/delegated_task_integration.rs

crates/legion-app/tests/delegated_task_integration.rs has 925 lines; symbols: _, accepted, accepted_review, boundary_input, citation, content; first content: use std::{fs, path::PathBuf};
Symbols: _, accepted, accepted_review, boundary_input, citation, content, err, error, file_name, host, hunk_id, mut, outcome, paired, plan_id, proposal, proposal_id, provider, reap_root, rejected, rejected_review, rejected_steps, removed, request_id, request_steps, result_steps, review, root, scope, snapshot, Target, targets_hello, temp_root, tmp, workspace_root

## crates/legion-app/tests/git_nav_workflow.rs

crates/legion-app/tests/git_nav_workflow.rs has 274 lines; symbols: _, file_name, first, first_file, focused, id; first content: use std::{
Symbols: _, file_name, first, first_file, focused, id, last_hunk_id, mut, nanos, new_file, p1, p2, path, projection, repo, root, second, snapshot, src_a, src_b, status, temp_root, unique_files

## crates/legion-app/tests/git_workflow.rs

crates/legion-app/tests/git_workflow.rs has 968 lines; symbols: _, active_text, after_edit, after_text, cached, committed; first content: use std::{
Symbols: _, active_text, after_edit, after_text, cached, committed, conflict, content, disk, disk_after_save, err, file_name, hunk_id, id, mut, nanos, output, path, projection, repo, repo_root, resolved, root, save_outcome, snapshot, source, staged, staged_hunk_id, status, subdir, temp_root, unmerged, unstaged

## crates/legion-app/tests/hostile_eval_integration.rs

crates/legion-app/tests/hostile_eval_integration.rs has 338 lines; symbols: _, file_name, hostile_content, mut, outcome, provider; headings: Real Content\nThis is legitimate documentation.\n",; first content: use std::{fs, path::PathBuf};
Symbols: _, file_name, hostile_content, mut, outcome, provider, read_requests, rejected, results, root, scope, Target, temp_root

## crates/legion-app/tests/language_edit_proposal_routing.rs

crates/legion-app/tests/language_edit_proposal_routing.rs has 81 lines; symbols: input, payload, proposal; first content: use legion_app::language::workspace_edit_to_proposal_input;
Symbols: input, payload, proposal

## crates/legion-app/tests/language_log_redaction.rs

crates/legion-app/tests/language_log_redaction.rs has 11 lines; symbols: raw, summary; first content: use legion_app::language::redact_lsp_stderr;
Symbols: raw, summary

## crates/legion-app/tests/language_restart_policy.rs

crates/legion-app/tests/language_restart_policy.rs has 147 lines; symbols: backoff, backoff2, config, mock, mut, policy; first content: use legion_app::language::{
Symbols: backoff, backoff2, config, mock, mut, policy

## crates/legion-app/tests/language_stale_snapshot.rs

crates/legion-app/tests/language_stale_snapshot.rs has 27 lines; first content: use legion_app::language::is_stale_response;
Symbols: none

## crates/legion-app/tests/language_terminal_integration.rs

crates/legion-app/tests/language_terminal_integration.rs has 398 lines; symbols: _, _untrusted_src, buffer_id, cancellation, code_lens_payload, deadline; first content: use std::{
Symbols: _, _untrusted_src, buffer_id, cancellation, code_lens_payload, deadline, denied, dispatch_terminal, first, first_projection, language, launched, lens_id, mut, original_disk_text, original_editor_text, outcome, path, projection, proposal, proposal_id, root, second, second_buffer, second_projection, session_id, shell, source, terminal, untrusted_workspace, workspace

## crates/legion-app/tests/language_tooling_workflow.rs

crates/legion-app/tests/language_tooling_workflow.rs has 669 lines; symbols: action_id, beta_offset, buffer_id, code_action, code_lens_payload, completion; first content: use std::sync::atomic::{AtomicU64, Ordering};
Symbols: action_id, beta_offset, buffer_id, code_action, code_lens_payload, completion, completion_payload, diagnostics, fallback, formatting, hover, hover_payload, inlay_payload, locations_payload, lsp_problem, mut, original_text, outcome, outline_payload, payload, projection, proposal, proposal_id, quick_fix, request, request_id, root, row, seeded, shell, snapshot, source, target

## crates/legion-app/tests/legion_workflow_integration.rs

crates/legion-app/tests/legion_workflow_integration.rs has 2651 lines; symbols: _, allowed_scope, barrier, bundle, cancellation_flag, cancelled_at; first content: use std::collections::HashMap;
Symbols: _, allowed_scope, barrier, bundle, cancellation_flag, cancelled_at, child_plan_id, client, conflict_id, dependent_dispatch, dependent_plan_id, dispatch_log, err, error, evidence, file_name, finished, first, first_metadata, first_routes, forbidden_scope, id, independent_plan_id, lane_barrier_timeout, left_pass, left_plan_id, live_projection, main, main_id, mut, nanos, opened, other, other_id, outcome, packet, plan_id, pre_authorized, projection, ready, registry, repeated, report, request, resolver, result, right_pass, right_plan_id, root, root_plan_id

## crates/legion-app/tests/legion_workflow_plan_lifecycle.rs

crates/legion-app/tests/legion_workflow_plan_lifecycle.rs has 240 lines; symbols: approved, dag, error, latest_before, mut, path; first content: use legion_agent::coordinator::LegionWorkflowSessionBuilderConfig;
Symbols: approved, dag, error, latest_before, mut, path, plan, rejected, reloaded, revision, revisions, session, temp

## crates/legion-app/tests/live_dap_prebuild.rs

crates/legion-app/tests/live_dap_prebuild.rs has 74 lines; symbols: args, bin, note, root; first content: use std::{
Symbols: args, bin, note, root

## crates/legion-app/tests/local_history_workflow.rs

crates/legion-app/tests/local_history_workflow.rs has 690 lines; symbols: _, _guard, alias, blob_dir, blobs_before, blocker_path; first content: use std::{
Symbols: _, _guard, alias, blob_dir, blobs_before, blocker_path, buf, canon_str, canonical, content, count_after, count_before, entries, entries2, entry_id, escape_target, evicted, git_proj, gitignore, hash, hashes, history_base, history_dir, id, ids, junc_ok, junc_status, link, mut, nanos, output, path, path_key, real_path, restore_result, root, save_result, stdout, stripped, sym_file, tmp, write_err, ws

## crates/legion-app/tests/lsp_mock/mod.rs

crates/legion-app/tests/lsp_mock/mod.rs has 135 lines; symbols: candidate, command, exe, mut, name, path; first content: use std::path::PathBuf;
Symbols: candidate, command, exe, mut, name, path, profile_dir

## crates/legion-app/tests/manual_zero_egress.rs

crates/legion-app/tests/manual_zero_egress.rs has 432 lines; symbols: _, AppCommandOutcome, buffer_id, file_name, id, mut; first content: use std::{
Symbols: _, AppCommandOutcome, buffer_id, file_name, id, mut, nanos, root, save, search, snapshot, temp_root, workspace

## crates/legion-app/tests/palette.rs

crates/legion-app/tests/palette.rs has 622 lines; symbols: _, _second_buffer, allowlisted, buffer_id, case_titles, cases; first content: use std::{
Symbols: _, _second_buffer, allowlisted, buffer_id, case_titles, cases, catalog_titles, coverage_percent, covered, file_name, first, first_buffer, id, initial_buffer_id, mut, nanos, Ok, outcome, palette, path, projected, root, search, second, Some, source, stale_cases, structural, target, temp_root, uncovered, viewport, workspace

## crates/legion-app/tests/plugin_grammar.rs

crates/legion-app/tests/plugin_grammar.rs has 122 lines; symbols: language_id, mut, outcome, parser, plugin_id; first content: use legion_app::AppComposition;
Symbols: language_id, mut, outcome, parser, plugin_id

## crates/legion-app/tests/proposal_fixture/mod.rs

crates/legion-app/tests/proposal_fixture/mod.rs has 187 lines; first content: use legion_protocol::{
Symbols: none

## crates/legion-app/tests/proposal_review_surface.rs

crates/legion-app/tests/proposal_review_surface.rs has 726 lines; symbols: accepted_hunk_ids, accepted_ids, all_accept, changed_target_ids, contents, file_ids; first content: use std::collections::{HashMap, HashSet};
Symbols: accepted_hunk_ids, accepted_ids, all_accept, changed_target_ids, contents, file_ids, filtered, ids, item, items, mut, new_text, old_text, panel, partial_accept, pid, proposal, ProposalPayload, result, retained_ids, section, surface, target, target_ids, targets, total_changed, undone

## crates/legion-app/tests/rust_analyzer_doc_sync.rs

crates/legion-app/tests/rust_analyzer_doc_sync.rs has 227 lines; symbols: config, diags, err, file_b, MOCK_DIAG_URI, mock_path; first content: use std::time::Duration;
Symbols: config, diags, err, file_b, MOCK_DIAG_URI, mock_path, mut, policy

## crates/legion-app/tests/rust_analyzer_download_policy.rs

crates/legion-app/tests/rust_analyzer_download_policy.rs has 115 lines; symbols: broker, good_hash, mut, result; first content: use legion_app::language::{
Symbols: broker, good_hash, mut, result

## crates/legion-app/tests/rust_analyzer_read_requests.rs

crates/legion-app/tests/rust_analyzer_read_requests.rs has 210 lines; symbols: advanced, config, err, issued, mock_path, mut; first content: use legion_app::language::{
Symbols: advanced, config, err, issued, mock_path, mut, outcome, params, policy, request_snapshot

## crates/legion-app/tests/rust_analyzer_session_handshake.rs

crates/legion-app/tests/rust_analyzer_session_handshake.rs has 179 lines; symbols: comp_cap, config, def_cap, health, hover_cap, mock_path; first content: use legion_app::language::{RustAnalyzerDiscovery, RustAnalyzerLaunchConfig, RustAnalyzerSession};
Symbols: comp_cap, config, def_cap, health, hover_cap, mock_path, mut

## crates/legion-app/tests/rust_analyzer_workflow.rs

crates/legion-app/tests/rust_analyzer_workflow.rs has 613 lines; symbols: _, backoff, backoff2, buffer_id, command, completion_count; first content: use std::fs;
Symbols: _, backoff, backoff2, buffer_id, command, completion_count, completion_deadline, completion_outcome, completion_params, completion_position, config, d, deadline, definition_outcome, definition_params, diags, fixture_dir, formatting_outcome, formatting_params, forward, has_changes, health, health_in_snap, hover_outcome, hover_params, insert_pos, lib_path, lib_rs, lib_rs_uri, lib_src, mut, policy, problem_count, references_outcome, references_params, rename_outcome, rename_params, root_uri, s, snap, Some, version

## crates/legion-app/tests/settings.rs

crates/legion-app/tests/settings.rs has 245 lines; symbols: _, file_name, id, mut, nanos, palette; first content: use std::{
Symbols: _, file_name, id, mut, nanos, palette, root, settings, snapshot, temp_root, workspace

## crates/legion-app/tests/structural_search_workflow.rs

crates/legion-app/tests/structural_search_workflow.rs has 190 lines; symbols: _, file_name, first, first_buffer, first_file, id; first content: use std::{
Symbols: _, file_name, first, first_buffer, first_file, id, mut, nanos, opened_workspace, path, projection, proposal_id, root, second, second_buffer, second_file, temp_root, workspace

## crates/legion-app/tests/terminal_workflow.rs

crates/legion-app/tests/terminal_workflow.rs has 889 lines; symbols: _, buffer_id, deadline, denied, expect_finish_markers, expected_session_id; first content: use std::sync::atomic::{AtomicU64, Ordering};
Symbols: _, buffer_id, deadline, denied, expect_finish_markers, expected_session_id, file_name, final_projection, has_finish, has_pascal, has_ready, kill_projection, killed, kinds, label, launched, mut, original_text, p, prefix, projection, projection2, ready_index, records, resized, root, root2, search_deadline, second, session_id, start_index, target, Target, temp_root, TERMINAL_POLL_DEADLINE, unique, untrusted_root, user_t2

## crates/legion-app/tests/test_explorer_workflow.rs

crates/legion-app/tests/test_explorer_workflow.rs has 473 lines; symbols: _, artifacts, bundle, err, item_id, items; first content: use std::{
Symbols: _, artifacts, bundle, err, item_id, items, marker, msg, mut, outcome, projection, records, root, session, session_id, snap, worker

## crates/legion-app/tests/two_ra_stress.rs

crates/legion-app/tests/two_ra_stress.rs has 370 lines; symbols: _, at_rest, buffered, churn, command, config; first content: use std::fs;
Symbols: _, at_rest, buffered, churn, command, config, cycles, deadline, DiscoveredBinary, discovery, entry, error_text, ERROR_TEXT_TEMPLATE, fixed_text, FIXED_TEXT_TEMPLATE, fixture, identity, initial, lower_drive, lower_pct, main_at_rest, main_uri, mut, nanos, normalized, pct, posture, ring, root_uri, scratchpad_path, scratchpad_uri, solo, stats, status, to, ty, workspace

## crates/legion-app/tests/upd_tests.rs

crates/legion-app/tests/upd_tests.rs has 556 lines; symbols: _, applied, artifact, artifact_bytes, check, dir; first content: use std::{
Symbols: _, applied, artifact, artifact_bytes, check, dir, double_rolled, journal_path, manifest, manifest_bytes, manifest_toml, mut, nanos, now, pid, policy, result, rolled, secs, seed, sig, sk, source, staged, tid, ts, updater, vk, wrong_bytes

## crates/legion-app/tests/workspace_vfs_integration.rs

crates/legion-app/tests/workspace_vfs_integration.rs has 5408 lines; symbols: _, accepted, accepted_operation, active, after_save_fingerprint, all_or_nothing; first content: use std::{
Symbols: _, accepted, accepted_operation, active, after_save_fingerprint, all_or_nothing, app, AppCommandOutcome, apply, AppSaveOutcome, atomic_contract, atomic_target, audit, audit_response, batch, best_effort_contract, best_effort_rollback, best_effort_target, buffer_id, conflict, conflict_delete, conflict_node, conflict_response, conflict_target, contract, coverage, create, create_path, create_response, create_target, current_snapshot, cycle_plan, debug, delete, delete_node, delete_path, delete_response, delete_target, denied, denied_operation, dependent, descriptor, destination, dirty, dirty_buffer, dry_run_batch, dry_run_contract, dry_run_target, duplicate_plan, duplicate_targets

## crates/legion-app/tests/worktree_creation_workflow.rs

crates/legion-app/tests/worktree_creation_workflow.rs has 538 lines; symbols: _, _guard, alias, alias_repo, fake_root, id; first content: use std::{
Symbols: _, _guard, alias, alias_repo, fake_root, id, junc_ok, junc_status, mut, nanos, output, outside_path, palette, parent, path, repo, repo_name, result, root, tmp, worktree_path, worktree_target

## crates/legion-app/tests/worktree_evidence_workflow.rs

crates/legion-app/tests/worktree_evidence_workflow.rs has 265 lines; symbols: _, content, error, escape_target, evidence_dir, evidence_path; first content: use std::{
Symbols: _, content, error, escape_target, evidence_dir, evidence_path, evidence_path_str, id, mut, nanos, output, path, repo, result, root, tmp

## crates/legion-cli/Cargo.toml

crates/legion-cli/Cargo.toml has 15 lines; first content: [package]
Symbols: none

## crates/legion-cli/src/main.rs

crates/legion-cli/src/main.rs has 1633 lines; symbols: _, after_is_boundary, args, artifacts, before_is_boundary, body; headings: Acceptance Status, Required Artifacts, Required Commands; first content: use std::{fs, path::PathBuf};
Symbols: _, after_is_boundary, args, artifacts, before_is_boundary, body, checklist, child_path, ci, commands, end, evidence, GUI_PHASE6_EVIDENCE_PATH, GUI_PHASE6_REQUIRED_ARTIFACTS, GUI_PHASE6_REQUIRED_COMMAND_MARKERS, GUI_PHASE7_EVIDENCE_PATH, GUI_PHASE7_REQUIRED_ARTIFACTS, GUI_PHASE7_REQUIRED_COMMAND_MARKERS, GUI_PHASE7_REQUIRED_LIMITATION_MARKERS, GUI_PHASE8_EVIDENCE_PATH, GUI_PHASE8_REQUIRED_ARTIFACTS, GUI_PHASE8_REQUIRED_COMMAND_MARKERS, GUI_PHASE8_REQUIRED_PLATFORM_MARKERS, GUI_PHASE8_REQUIRED_SURFACE_MARKERS, GUI_PHASE8_STALE_UNSUPPORTED_MARKERS, historical, id, ledger, lib_path, limitations, manifest_path, mut, nanos, normalized, Ok, path, PHASE_GATE_COMMANDS, PHASE0_EVIDENCE_FILES, phase3, PHASE3_REQUIRED_ARTIFACTS, PHASE8_ACCEPTED_ARTIFACT_STALE_MARKERS, PHASE8_ACCEPTED_REQUIRED_MARKERS, PHASE8_NOT_ACCEPTED_ALLOWED_MARKERS, PHASE8_PLATFORM_MATRIX_ARTIFACT, PHASE8_PLATFORM_MATRIX_REQUIRED_MARKERS, PHASE8_RELEASE_READINESS_ARTIFACT, PHASE8_RELEASE_SIGNOFF_REQUIRED_MARKERS, PHASE8_REQUIRED_ARTIFACTS, PHASE8_STALE_DEFERRED_MARKERS, platforms

## crates/legion-collaboration/Cargo.toml

crates/legion-collaboration/Cargo.toml has 12 lines; first content: [package]
Symbols: none

## crates/legion-collaboration/src/lib.rs

crates/legion-collaboration/src/lib.rs has 1757 lines; symbols: accepted, acknowledgement, audit, binding, conflict, delete; first content: use std::collections::{HashMap, HashSet};
Symbols: accepted, acknowledgement, audit, binding, conflict, delete, delta, duplicate, end, expected_sequence, gap, initial, initial_text, insert, manifest, mut, op, operations, other_end, other_start, outcome, participant, participant_id, participants, point, presence_rejected, previous_text, projection, range, ready_index, rejected, replacement, result, start, value

## crates/legion-debug/Cargo.toml

crates/legion-debug/Cargo.toml has 19 lines; first content: [package]
Symbols: none

## crates/legion-debug/src/adapter_resolve.rs

crates/legion-debug/src/adapter_resolve.rs has 256 lines; symbols: adapter_type, bare, mode, mut, names, path; first content: use std::env;
Symbols: adapter_type, bare, mode, mut, names, path, path_var, t

## crates/legion-debug/src/bin/fake_dap_adapter.rs

crates/legion-debug/src/bin/fake_dap_adapter.rs has 362 lines; symbols: _, arguments, body, breakpoints, command, length; first content: use std::io::{self, BufRead, BufReader, Write};
Symbols: _, arguments, body, breakpoints, command, length, line, lines, msg_type, mut, n, request_seq, seq, source, stdin

## crates/legion-debug/src/dap.rs

crates/legion-debug/src/dap.rs has 357 lines; symbols: adapter_type, audit, breakpoints, first_path, first_range, label; first content: use std::{collections::HashMap, sync::Mutex};
Symbols: adapter_type, audit, breakpoints, first_path, first_range, label, mut, sequence, session_id, sessions

## crates/legion-debug/src/evidence.rs

crates/legion-debug/src/evidence.rs has 250 lines; symbols: audit, error, evidence, lowered, passed, summary; first content: use legion_protocol::{
Symbols: audit, error, evidence, lowered, passed, summary, summary_text

## crates/legion-debug/src/framing.rs

crates/legion-debug/src/framing.rs has 375 lines; symbols: decoded, ev, ev_json, frame, header, header_end; first content: use serde::{Deserialize, Serialize};
Symbols: decoded, ev, ev_json, frame, header, header_end, header_text, json, length, msg, mut, n, payload, payload_end, payload_start, resp, resp_json

## crates/legion-debug/src/lib.rs

crates/legion-debug/src/lib.rs has 33 lines; first content: pub mod adapter_resolve;
Symbols: none

## crates/legion-debug/src/live_session.rs

crates/legion-debug/src/live_session.rs has 617 lines; symbols: _, breakpoints, deadline, list, msg, mut; first content: use std::io::{BufReader, Write};
Symbols: _, breakpoints, deadline, list, msg, mut, p, program, remaining, req, result, scopes, seq, stack, stack_frames, stdin, stdout, target, vars

## crates/legion-debug/src/state.rs

crates/legion-debug/src/state.rs has 35 lines; first content: use legion_protocol::DebugSessionState;
Symbols: none

## crates/legion-debug/tests/dap_runtime.rs

crates/legion-debug/tests/dap_runtime.rs has 49 lines; symbols: denied, outcome, runtime; first content: use legion_debug::{DapClientConfig, DapClientRuntime, DapLifecycleState};
Symbols: denied, outcome, runtime

## crates/legion-debug/tests/live_dap_handshake.rs

crates/legion-debug/tests/live_dap_handshake.rs has 85 lines; symbols: bps, cont, mut, outcome, stepped, stop; first content: use std::time::Duration;
Symbols: bps, cont, mut, outcome, stepped, stop

## crates/legion-debug/tests/system_adapter_dogfood.rs

crates/legion-debug/tests/system_adapter_dogfood.rs has 108 lines; symbols: _, mut, outcome, require, Some; first content: use std::time::Duration;
Symbols: _, mut, outcome, require, Some

## crates/legion-debug/tests/system_adapter_launch_step_dogfood.rs

crates/legion-debug/tests/system_adapter_launch_step_dogfood.rs has 195 lines; symbols: _, bin, cwd, mut, program, require; first content: use std::{
Symbols: _, bin, cwd, mut, program, require, root, Some, source, source_path, status, stop

## crates/legion-desktop/Cargo.toml

crates/legion-desktop/Cargo.toml has 43 lines; headings: Enable `test-helpers` seams on AppComposition (e.g. `set_lsp_health_for_test`), so integration tests in tests/ can reach them without a real LSP server.; first content: [package]
Symbols: none

## crates/legion-desktop/src/beta.rs

crates/legion-desktop/src/beta.rs has 959 lines; symbols: _, active_file_search_status, beta_root, browse_status, command, diagnostics_export_label; first content: use std::{
Symbols: _, active_file_search_status, beta_root, browse_status, command, diagnostics_export_label, diagnostics_export_written, edit, EDIT_TEXT, errors, escaped, file_name, final_snapshot, FIXTURE_CARGO_TOML, FIXTURE_MAIN_RS, FIXTURE_README, is_simple, language, language_status, launch_config, mut, node_count, outcome, parent, prepared, preview, projection, proposal_id, proposal_mode, refresh, report, requested, retry_deadline, save, saved_text, snapshot, Some, src_dir, status, target_root, terminal, unsupported, workspace_root, workspace_search_status

## crates/legion-desktop/src/bridge.rs

crates/legion-desktop/src/bridge.rs has 3105 lines; symbols: authority_label, base_branch, command_id, instruction_label, label, needle; first content: use std::fmt;
Symbols: authority_label, base_branch, command_id, instruction_label, label, needle, path, plan_id, projection_id, request_id, run_index, Some, tabs, Target

## crates/legion-desktop/src/cut_lines.rs

crates/legion-desktop/src/cut_lines.rs has 62 lines; first content: pub fn plugin_registered_status(plugin_id: u64) -> String {
Symbols: none

## crates/legion-desktop/src/debug_auto_poll.rs

crates/legion-desktop/src/debug_auto_poll.rs has 47 lines; symbols: mut; first content: use legion_ui::{DebugProjection, DebugStatusKindProjection};
Symbols: mut

## crates/legion-desktop/src/diagnostics.rs

crates/legion-desktop/src/diagnostics.rs has 117 lines; symbols: markdown, path; first content: use std::{
Symbols: markdown, path

## crates/legion-desktop/src/harness.rs

crates/legion-desktop/src/harness.rs has 126 lines; symbols: _, id, nanos, path, root, runtime; first content: use std::fs;
Symbols: _, id, nanos, path, root, runtime, temp_root

## crates/legion-desktop/src/health.rs

crates/legion-desktop/src/health.rs has 380 lines; symbols: assisted, language, ledger, legion, mut, NOT_OBSERVED; first content: use legion_ui::ShellProjectionSnapshot;
Symbols: assisted, language, ledger, legion, mut, NOT_OBSERVED, search, tabs, terminal

## crates/legion-desktop/src/lib.rs

crates/legion-desktop/src/lib.rs has 28 lines; first content: pub mod beta;
Symbols: none

## crates/legion-desktop/src/main.rs

crates/legion-desktop/src/main.rs has 8 lines; first content: use anyhow::Result;
Symbols: none

## crates/legion-desktop/src/manual_perf.rs

crates/legion-desktop/src/manual_perf.rs has 372 lines; symbols: _, buffer_id, cursor, index, initial_file, input; first content: use std::{
Symbols: _, buffer_id, cursor, index, initial_file, input, keypress_p50_budget_micros, keypress_p95_budget_micros, message, mut, passed, rank, renderer, report, scroll_p95_budget_micros, snapshot, started_at, status

## crates/legion-desktop/src/metrics.rs

crates/legion-desktop/src/metrics.rs has 178 lines; symbols: average_frame_ms, delta, duration, index, MAX_RETAINED_SAMPLES, mut; first content: use std::collections::VecDeque;
Symbols: average_frame_ms, delta, duration, index, MAX_RETAINED_SAMPLES, mut, rank, Some

## crates/legion-desktop/src/package.rs

crates/legion-desktop/src/package.rs has 207 lines; symbols: cargo_command, executable_destination, executable_source, manifest_path, mut, target_triple; first content: use std::{
Symbols: cargo_command, executable_destination, executable_source, manifest_path, mut, target_triple

## crates/legion-desktop/src/platform.rs

crates/legion-desktop/src/platform.rs has 315 lines; symbols: accessibility_nodes, ADAPTER_PATH_PASSED, at, bridge, label, mut; first content: use legion_protocol::TextCoordinate;
Symbols: accessibility_nodes, ADAPTER_PATH_PASSED, at, bridge, label, mut, node_count, NOT_OBSERVED

## crates/legion-desktop/src/search.rs

crates/legion-desktop/src/search.rs has 130 lines; symbols: header, mut, path, query, result_rows, scope; first content: use legion_ui::{SearchProjection, SearchScopeProjection, SearchStatusKindProjection};
Symbols: header, mut, path, query, result_rows, scope, stale_tag, truncated

## crates/legion-desktop/src/session.rs

crates/legion-desktop/src/session.rs has 264 lines; symbols: _, existing, file_name, json, MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH; first content: use std::{
Symbols: _, existing, file_name, json, MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH, mut, new, nonce, ok, path, RAW_SOURCE_MARKERS, record, Some, temp_path, written

## crates/legion-desktop/src/smoke.rs

crates/legion-desktop/src/smoke.rs has 610 lines; symbols: _, adapter_checks, command, duration, errors, escaped; first content: use std::{
Symbols: _, adapter_checks, command, duration, errors, escaped, is_simple, mut, native, native_options, NOT_OBSERVED, now, observations, observations_for_app, platform_snapshot, recorder, recorder_for_app, report, report_config, runtime, smoke_title, snapshot, started_at, status, timing

## crates/legion-desktop/src/theme.rs

crates/legion-desktop/src/theme.rs has 585 lines; symbols: dark, fallback_found, fallback_probe, fn, label, light; first content: use egui::{
Symbols: dark, fallback_found, fallback_probe, fn, label, light, mut, normalized

## crates/legion-desktop/src/view.rs

crates/legion-desktop/src/view.rs has 8547 lines; symbols: accent, accepts, actionable, active, active_buffer_id, active_git_relative_path; first content: mod assistant_rail;
Symbols: accent, accepts, actionable, active, active_buffer_id, active_git_relative_path, anchor, assisted, attempts, autonomy_scale_rows, base, bg, blame_lines, blink_on, board_height, body, bottom_height, boundary_ok, budget, budget_evaluation_count, buffer, bytes, cache_id, capabilities, channel, char_count, char_width, chars, checklist, child_ids, CODE_LINE_GALLEY_CACHE_LIMIT, col, color, column, command_palette_overlay, command_palette_rows, COMMAND_PALETTE_VISIBLE_RESULT_ROWS, commands, completions, confirm, context_manifest, coordinate, count, current_cursor, current_line_number, cursor, cursor_rect, debug, delegated, different_buffer

## crates/legion-desktop/src/view/agent_comm.rs

crates/legion-desktop/src/view/agent_comm.rs has 117 lines; symbols: rows, tag_color; first content: use egui::Color32;
Symbols: rows, tag_color

## crates/legion-desktop/src/view/assistant_rail.rs

crates/legion-desktop/src/view/assistant_rail.rs has 449 lines; symbols: accumulated, available, block, blocks, bound_proposal_id, button; first content: use legion_ai::streaming::{MarkdownStreamSegment, split_markdown_stream};
Symbols: accumulated, available, block, blocks, bound_proposal_id, button, code_blocks, complete, id, label, mut, partial, response, rows, segments

## crates/legion-desktop/src/view/code_canvas_painter.rs

crates/legion-desktop/src/view/code_canvas_painter.rs has 53 lines; symbols: painter; first content: use legion_ui::ShellProjectionSnapshot;
Symbols: painter

## crates/legion-desktop/src/view/fleet_board.rs

crates/legion-desktop/src/view/fleet_board.rs has 135 lines; symbols: board_height, color, columns; first content: use egui::Color32;
Symbols: board_height, color, columns

## crates/legion-desktop/src/view/fleet_card.rs

crates/legion-desktop/src/view/fleet_card.rs has 85 lines; symbols: cards; first content: use legion_protocol::ProposalRiskLabel;
Symbols: cards

## crates/legion-desktop/src/view/ghost_text.rs

crates/legion-desktop/src/view/ghost_text.rs has 97 lines; symbols: ghost_text; first content: use legion_protocol::{
Symbols: ghost_text

## crates/legion-desktop/src/view/inline_edit.rs

crates/legion-desktop/src/view/inline_edit.rs has 600 lines; symbols: accepted_hunks, accumulated, applied_hunk_count, audit_record, checkpoint, checkpoint_id; first content: use std::collections::HashMap;
Symbols: accepted_hunks, accumulated, applied_hunk_count, audit_record, checkpoint, checkpoint_id, complete, edits, end, header_and_original, hunk, hunk_id, is_trailing, mut, original_text, part, parts, proposal_id, replacement_text, SEP, sep_pos, start, targets, undo_group_id

## crates/legion-desktop/src/view/interactive_fields.rs

crates/legion-desktop/src/view/interactive_fields.rs has 117 lines; symbols: draft_id, key, mut, response, selected, submit; first content: use crate::bridge::{DesktopAction, SensitiveString};
Symbols: draft_id, key, mut, response, selected, submit

## crates/legion-desktop/src/view/manifest_panel.rs

crates/legion-desktop/src/view/manifest_panel.rs has 160 lines; symbols: can_exclude, egress_marker, excluded_count, included_count, is_selected, mandatory_count; first content: use legion_protocol::{
Symbols: can_exclude, egress_marker, excluded_count, included_count, is_selected, mandatory_count, manifest, mut, selected_item_id, Some

## crates/legion-desktop/src/view/plan_editor.rs

crates/legion-desktop/src/view/plan_editor.rs has 307 lines; symbols: artifact, body, model, sections, summary_label; first content: use legion_protocol::{
Symbols: artifact, body, model, sections, summary_label

## crates/legion-desktop/src/view/proposal_review.rs

crates/legion-desktop/src/view/proposal_review.rs has 687 lines; symbols: checkpoint_projection, checkpoint_proposal_id, entry, fallback, file, file_label; first content: use legion_app::proposal_risk_rule_ids_from_coverage;
Symbols: checkpoint_projection, checkpoint_proposal_id, entry, fallback, file, file_label, files, mut, review, verification_summary_count

## crates/legion-desktop/src/view/risk_strip.rs

crates/legion-desktop/src/view/risk_strip.rs has 116 lines; symbols: evidence, findings_summary, label, level_label, mut, paused; first content: use legion_protocol::ProposalRiskLabel;
Symbols: evidence, findings_summary, label, level_label, mut, paused, reason, reasons, requires_human_approval

## crates/legion-desktop/src/view/sandbox_panel.rs

crates/legion-desktop/src/view/sandbox_panel.rs has 452 lines; symbols: activation, all, all_output, backends, label, lease_held; first content: use legion_protocol::DelegatedTaskRuntimeActivationState;
Symbols: activation, all, all_output, backends, label, lease_held, mut, panel_rows, profile, rows, scope, snapshot, state, summary

## crates/legion-desktop/src/view/scope_picker.rs

crates/legion-desktop/src/view/scope_picker.rs has 125 lines; symbols: target; first content: use egui::Ui;
Symbols: target

## crates/legion-desktop/src/view/terminal_panel.rs

crates/legion-desktop/src/view/terminal_panel.rs has 64 lines; first content: use legion_protocol::{EventSequence, TerminalPanelProjection};
Symbols: none

## crates/legion-desktop/src/view/worker_panel.rs

crates/legion-desktop/src/view/worker_panel.rs has 446 lines; symbols: matching_verification_failed, model, mut, recovery_actions, snapshot; first content: use legion_protocol::{
Symbols: matching_verification_failed, model, mut, recovery_actions, snapshot

## crates/legion-desktop/src/workflow.rs

crates/legion-desktop/src/workflow.rs has 5035 lines; symbols: _, accepted_hunk_ids, accepted_target_ids, active, active_index, alt; first content: use std::process::Command;
Symbols: _, accepted_hunk_ids, accepted_target_ids, active, active_index, alt, arg_text, at, at_ime, at_paste, at_text, backspace, beta, bridge_output, buffer_id, byte, bytes, character, command, command_a, COMMAND_PALETTE_VISIBLE_RESULT_ROWS, completions, composition_id, config, count, ctx, cursor, custom_toolkit, delete, delta, detail, details, diagnostics, dirty_tab_count, dock_mode, editor_input_enabled, end, enter, events, file, file_name, flat_hunks, full_output, health, id, ime_composition_active, index, initial_file, input, intent

## crates/legion-desktop/tests/accessibility.rs

crates/legion-desktop/tests/accessibility.rs has 297 lines; symbols: _, decoded, encoded, file_name, id, model; first content: use std::{
Symbols: _, decoded, encoded, file_name, id, model, mut, nanos, profile, raw_input, roles, root, runtime, smoke, status_node, temp_root, workspace

## crates/legion-desktop/tests/agent_comm.rs

crates/legion-desktop/tests/agent_comm.rs has 15 lines; symbols: rows; first content: use legion_desktop::view::agent_comm::agent_comm_rows;
Symbols: rows

## crates/legion-desktop/tests/assist_inline_prediction_workflow.rs

crates/legion-desktop/tests/assist_inline_prediction_workflow.rs has 100 lines; symbols: _, manual, mut, root, source; first content: use std::{
Symbols: _, manual, mut, root, source

## crates/legion-desktop/tests/assistant_rail.rs

crates/legion-desktop/tests/assistant_rail.rs has 235 lines; symbols: blocks, bridge, capabilities, caps, commands, explain; first content: use legion_desktop::bridge::{DesktopAction, DesktopBridgeOutput, DesktopCommandBridge};
Symbols: blocks, bridge, capabilities, caps, commands, explain, fix, ids, result, rows, selection, snapshot, view_models

## crates/legion-desktop/tests/beta_acceptance_e2e.rs

crates/legion-desktop/tests/beta_acceptance_e2e.rs has 928 lines; symbols: _, autonomous_apply_commands, beta_workspace, causality_id, command_capability, config_id; first content: use std::{
Symbols: _, autonomous_apply_commands, beta_workspace, causality_id, command_capability, config_id, context_manifest, debug_model, diff, evidence, evidence_text, host_session, id, install_model, installed_plugin_id, joined, manifest, mut, name, nanos, Ok, path, paths, plan_id, presence, projected_lifecycle_states, proposal_id, proposal_row, proposal_snapshot, proposal_start, report, saved_main, session_id, snapshot, step, target, target_root, test_run, verification_rows, verification_snapshot, vsix, workspace_root

## crates/legion-desktop/tests/beta_workflow.rs

crates/legion-desktop/tests/beta_workflow.rs has 193 lines; symbols: _, beta_workspace, config, diagnostics_directory, error, evidence; first content: use std::{
Symbols: _, beta_workspace, config, diagnostics_directory, error, evidence, evidence_text, id, name, nanos, Ok, outside_target, path, paths, report, saved_main, target_root

## crates/legion-desktop/tests/breakpoint_hit.rs

crates/legion-desktop/tests/breakpoint_hit.rs has 434 lines; symbols: body, bound, breakpoints_response, command, configuration_id, continued; first content: use std::{
Symbols: body, bound, breakpoints_response, command, configuration_id, continued, decoded, event, framed, initialize, launch, line, model, mut, production, production_frame, program, protocol_frame, protocol_frame_name, protocol_variable, protocol_variable_name, response, response_framed, root, scopes, session_id, set_breakpoints, snapshot, source, stack_response, stack_trace, threads, variables, variables_response

## crates/legion-desktop/tests/collaboration_gui.rs

crates/legion-desktop/tests/collaboration_gui.rs has 578 lines; symbols: _, bridge, buffer_path, disabled, file_name, gui; first content: use std::{
Symbols: _, bridge, buffer_path, disabled, file_name, gui, id, joined, model, mut, nanos, path, presence, presence_envelope, presence_outcome, root, runtime, shared, snapshot, target, temp_root, workspace

## crates/legion-desktop/tests/common/mod.rs

crates/legion-desktop/tests/common/mod.rs has 207 lines; symbols: after_idx, after_ok, b, before_ok, bytes, hay; first content: pub fn strip_comments_and_strings(source: &str) -> String {
Symbols: after_idx, after_ok, b, before_ok, bytes, hay, mut, needle, next, stripped

## crates/legion-desktop/tests/completion_popup.rs

crates/legion-desktop/tests/completion_popup.rs has 311 lines; symbols: _, b_buffer_id, buffer_id, file, file_a, file_b; first content: use std::{
Symbols: _, b_buffer_id, buffer_id, file, file_a, file_b, items, mut, outcome, path, raw_response, root, snapshot, text, ws

## crates/legion-desktop/tests/control_trust_bridge.rs

crates/legion-desktop/tests/control_trust_bridge.rs has 349 lines; symbols: _, bridge, bridge_source, forbidden, manifest_dir, outcome; first content: use std::{
Symbols: _, bridge, bridge_source, forbidden, manifest_dir, outcome, partial_run_id, path, root, runtime, snapshot, target, view_source, workspace

## crates/legion-desktop/tests/control_trust_view.rs

crates/legion-desktop/tests/control_trust_view.rs has 348 lines; symbols: _, budget_row, mode, model, mut, outcome; first content: use std::{
Symbols: _, budget_row, mode, model, mut, outcome, path, privacy_row, root, snapshot, state, target, trusted_model, trusted_snapshot, untrusted_model, untrusted_snapshot, workspace

## crates/legion-desktop/tests/daily_editing_controls.rs

crates/legion-desktop/tests/daily_editing_controls.rs has 299 lines; symbols: _, buffer_id, buffers, file_id, file_name, first; first content: use std::{
Symbols: _, buffer_id, buffers, file_id, file_name, first, first_buffer, id, mut, nanos, outcome, path, root, scroll, second, second_buffer, snapshot, target, temp_root, workspace

## crates/legion-desktop/tests/debug_keyboard.rs

crates/legion-desktop/tests/debug_keyboard.rs has 272 lines; symbols: _, after, after_f5, after_step, before, config_id; first content: use std::{
Symbols: _, after, after_f5, after_step, before, config_id, deadline, debug, mut, root, source, status, toggled

## crates/legion-desktop/tests/debug_workflow.rs

crates/legion-desktop/tests/debug_workflow.rs has 479 lines; symbols: body, breakpoints_response, command, config_id, continue_request, decoded; first content: use std::{
Symbols: body, breakpoints_response, command, config_id, continue_request, decoded, error_initialize, evaluate, evaluate_response, event, failed_launch, failed_match, framed, initialize, launch, model, mut, production, production_frame, program, protocol_frame_name, response, response_framed, root, session_id, set_breakpoints, snapshot, source, stack_response, stack_trace, step, step_match, success, threads

## crates/legion-desktop/tests/delegated_task_command_center.rs

crates/legion-desktop/tests/delegated_task_command_center.rs has 778 lines; symbols: _, bridge, file_name, hunk, hunk_id, id; first content: use std::{
Symbols: _, bridge, file_name, hunk, hunk_id, id, inspected, model, mut, nanos, path, plan_id, preview, root, runtime, snapshot, target, temp_root, workspace

## crates/legion-desktop/tests/desktop_workflow.rs

crates/legion-desktop/tests/desktop_workflow.rs has 204 lines; symbols: _, after, before, edit, edited, file_name; first content: use std::{
Symbols: _, after, before, edit, edited, file_name, id, model, mut, nanos, outcome, path, rejected, root, save, target, temp_root, workspace

## crates/legion-desktop/tests/diagnostics_export.rs

crates/legion-desktop/tests/diagnostics_export.rs has 160 lines; symbols: _, config, diagnostics_path, file, id, initial; first content: use std::{
Symbols: _, config, diagnostics_path, file, id, initial, mut, nanos, path, root, session_path, temp_root, updated, workspace

## crates/legion-desktop/tests/diagnostics_harness.rs

crates/legion-desktop/tests/diagnostics_harness.rs has 137 lines; symbols: _, buffer_id, cleared, diagnostics, file, file_id; first content: use std::{
Symbols: _, buffer_id, cleared, diagnostics, file, file_id, id, mut, nanos, path, root, snapshot, temp_root, workspace

## crates/legion-desktop/tests/ghost_text.rs

crates/legion-desktop/tests/ghost_text.rs has 298 lines; symbols: bridge, dismissed, mut, overlay, provider_ids, registry; first content: use legion_ai_providers::{DETERMINISTIC_LOCAL_PROVIDER_ID, make_inline_prediction_registry};
Symbols: bridge, dismissed, mut, overlay, provider_ids, registry, request_id, result, snapshot

## crates/legion-desktop/tests/git_workflow.rs

crates/legion-desktop/tests/git_workflow.rs has 413 lines; symbols: _, bridge, cached, content, file_name, hunk_id; first content: use std::{
Symbols: _, bridge, cached, content, file_name, hunk_id, id, model, mut, nanos, output, path, pushed, remote_root, repo, root, snapshot, source, temp_root, unmerged

## crates/legion-desktop/tests/headless_input.rs

crates/legion-desktop/tests/headless_input.rs has 362 lines; symbols: _, _guard, file_name, file_path, id, mut; first content: use std::{
Symbols: _, _guard, file_name, file_path, id, mut, nanos, path, root, runtime, saved, snapshot, temp_root, workspace

## crates/legion-desktop/tests/hover_definition.rs

crates/legion-desktop/tests/hover_definition.rs has 257 lines; symbols: _, b_buffer_id, buffer_id, file, file_a, file_b; first content: use std::{
Symbols: _, b_buffer_id, buffer_id, file, file_a, file_b, mut, outcome, path, raw_response, root, snapshot, ws

## crates/legion-desktop/tests/inline_edit.rs

crates/legion-desktop/tests/inline_edit.rs has 692 lines; symbols: actions, apply_result, audit_record, bridge, caus, changed_buffer_version; first content: use legion_desktop::bridge::{DesktopAction, DesktopBridgeOutput, DesktopCommandBridge};
Symbols: actions, apply_result, audit_record, bridge, caus, changed_buffer_version, chunks, complete, corr, current_buffer_version, current_snapshot_id, hunks, instruction, is_fresh, loaded, mut, output, overlay, partial, proposal, proposal_id, proposal_result, result, snapshot, two_hunks

## crates/legion-desktop/tests/input_conformance.rs

crates/legion-desktop/tests/input_conformance.rs has 421 lines; symbols: _, actions, backspace_range, file, file_name, id; first content: use std::{
Symbols: _, actions, backspace_range, file, file_name, id, keyboard_actions, mut, nanos, path, root, runtime, selected, snapshot, suppressed, temp_root, text_actions, workspace

## crates/legion-desktop/tests/intent_bridge.rs

crates/legion-desktop/tests/intent_bridge.rs has 1077 lines; symbols: actions, at, bridge, configuration_id, cursor, insert_at; first content: use std::path::PathBuf;
Symbols: actions, at, bridge, configuration_id, cursor, insert_at, model, mut, position, result, scroll, session_id, snapshot, source

## crates/legion-desktop/tests/keyboard_nav.rs

crates/legion-desktop/tests/keyboard_nav.rs has 745 lines; symbols: _, accepted, accepted_after_dismiss, buffer_id, content, file; first content: use std::{
Symbols: _, accepted, accepted_after_dismiss, buffer_id, content, file, file_name, id, input, items, mut, nanos, outcome, params, pid, proposal, raw_input, root, runtime, src_file, targets, temp_root, uri, workspace

## crates/legion-desktop/tests/language_health_view.rs

crates/legion-desktop/tests/language_health_view.rs has 252 lines; symbols: allowed, base, cases, dir, health, model; first content: use legion_desktop::view::DesktopProjectionViewModel;
Symbols: allowed, base, cases, dir, health, model, mut, p, record, refused, row, snapshot

## crates/legion-desktop/tests/language_terminal_view.rs

crates/legion-desktop/tests/language_terminal_view.rs has 318 lines; symbols: bridge, model, mut; first content: use legion_desktop::{
Symbols: bridge, model, mut

## crates/legion-desktop/tests/language_terminal_workflow.rs

crates/legion-desktop/tests/language_terminal_workflow.rs has 250 lines; symbols: _, bridge, bridge_source, empty, forbidden, language_model; first content: use std::{
Symbols: _, bridge, bridge_source, empty, forbidden, language_model, manifest_dir, model, mut, path, root, source, status_row, terminal_model, view_source, workspace

## crates/legion-desktop/tests/large_file_guardrails.rs

crates/legion-desktop/tests/large_file_guardrails.rs has 171 lines; symbols: _, id, large, model, mut, nanos; first content: use std::{
Symbols: _, id, large, model, mut, nanos, path, root, runtime, snapshot, temp_root, viewport, workspace

## crates/legion-desktop/tests/legion_workflow_command_center.rs

crates/legion-desktop/tests/legion_workflow_command_center.rs has 755 lines; symbols: blocked, blockers, bridge, card, cards, columns; first content: use legion_agent::comm::{AgentCommTag, format_agent_comm_line};
Symbols: blocked, blockers, bridge, card, cards, columns, health, missing_session, model, mut, parsed, parsed_tags, rows, server_id, session_id, snapshot, source, states, template, tool_name, view_models

## crates/legion-desktop/tests/live_continue_auto_poll.rs

crates/legion-desktop/tests/live_continue_auto_poll.rs has 230 lines; symbols: _, config_id, continued, deadline, debug, final_debug; first content: use std::{
Symbols: _, config_id, continued, deadline, debug, final_debug, launched, model, mut, root, session_id, source, status, stopped

## crates/legion-desktop/tests/manifest_panel.rs

crates/legion-desktop/tests/manifest_panel.rs has 252 lines; symbols: leaves_row, manifest, mut, result, rows, snapshot; first content: use legion_desktop::view::{preview_rows, toggle_manifest_item_inclusion};
Symbols: leaves_row, manifest, mut, result, rows, snapshot, toggled, toggled_back

## crates/legion-desktop/tests/manual_input_conformance.rs

crates/legion-desktop/tests/manual_input_conformance.rs has 277 lines; symbols: _, buffer_id, cursor, file_name, id, mut; first content: use std::{
Symbols: _, buffer_id, cursor, file_name, id, mut, nanos, path, root, snapshot, snapshot_after_cut, target, temp_root, workspace

## crates/legion-desktop/tests/manual_perf.rs

crates/legion-desktop/tests/manual_perf.rs has 159 lines; symbols: _, beta_error, config, contents, file_name, id; first content: use std::{
Symbols: _, beta_error, config, contents, file_name, id, manual_perf, nanos, path, prefix, report, root, smoke_error, target, temp_root, workspace

## crates/legion-desktop/tests/manual_renderer_evidence.rs

crates/legion-desktop/tests/manual_renderer_evidence.rs has 160 lines; symbols: active_evidence, active_model, empty_evidence, empty_model, empty_snapshot, model; first content: use legion_desktop::view::DesktopProjectionViewModel;
Symbols: active_evidence, active_model, empty_evidence, empty_model, empty_snapshot, model, mut

## crates/legion-desktop/tests/operational_health.rs

crates/legion-desktop/tests/operational_health.rs has 147 lines; symbols: _, file, health, id, joined, model; first content: use std::{
Symbols: _, file, health, id, joined, model, mut, nanos, path, rendered_health, root, rows, snapshot, temp_root, workspace

## crates/legion-desktop/tests/packaging.rs

crates/legion-desktop/tests/packaging.rs has 112 lines; symbols: _, config, manifest, output, plan, root; first content: use std::{fs, path::PathBuf};
Symbols: _, config, manifest, output, plan, root

## crates/legion-desktop/tests/palette_coverage.rs

crates/legion-desktop/tests/palette_coverage.rs has 424 lines; symbols: _, _guard, cases, coverage_percent, expected_first_path, expected_symbol_path; first content: use std::{
Symbols: _, _guard, cases, coverage_percent, expected_first_path, expected_symbol_path, file_name, first, first_buffer, id, initial_buffer_id, mut, nanos, outcome, palette, path, root, second, second_buffer, snapshot, source, symbol_file, temp_root, workspace

## crates/legion-desktop/tests/palette_persistence.rs

crates/legion-desktop/tests/palette_persistence.rs has 201 lines; symbols: _, baseline, explorer_pos_base, explorer_pos_boosted, git_pos_base, git_pos_boosted; first content: use std::{
Symbols: _, baseline, explorer_pos_base, explorer_pos_boosted, git_pos_base, git_pos_boosted, id, mut, nanos, palette_usage_path, pos, root, snapshot, workspace

## crates/legion-desktop/tests/plan_editor.rs

crates/legion-desktop/tests/plan_editor.rs has 130 lines; symbols: artifact, bridge, edited_sections, model, mut, output; first content: use legion_desktop::bridge::{
Symbols: artifact, bridge, edited_sections, model, mut, output, snapshot

## crates/legion-desktop/tests/platform_integration.rs

crates/legion-desktop/tests/platform_integration.rs has 70 lines; symbols: labels, mut, platform; first content: use legion_desktop::platform::{
Symbols: labels, mut, platform

## crates/legion-desktop/tests/platform_smoke.rs

crates/legion-desktop/tests/platform_smoke.rs has 272 lines; symbols: _, at, bridge, config, contents, empty; first content: use std::{
Symbols: _, at, bridge, config, contents, empty, evidence, id, input_at, markdown, mut, nanos, report, root, smoke, source, start, summary, temp_root, workspace

## crates/legion-desktop/tests/plugin_management.rs

crates/legion-desktop/tests/plugin_management.rs has 261 lines; symbols: _, bridge, denied, file_name, id, invoked; first content: use std::{
Symbols: _, bridge, denied, file_name, id, invoked, model, mut, nanos, root, runtime, snapshot, temp_root, workspace

## crates/legion-desktop/tests/projection_rendering.rs

crates/legion-desktop/tests/projection_rendering.rs has 1595 lines; symbols: all_model, anchor, assisted_model, clamped, collapsed, coordinate; first content: use std::collections::BTreeSet;
Symbols: all_model, anchor, assisted_model, clamped, collapsed, coordinate, degraded_model, delegated, dismissed_id, empty, empty_model, end, fallback, full_line, initial, line, lines, manual, model, mut, old_cursor, populated, range, rows, snapshot, source, streaming_model, word

## crates/legion-desktop/tests/provider_key_entry.rs

crates/legion-desktop/tests/provider_key_entry.rs has 120 lines; symbols: after, before, granted, has_credential, leaked, loaded; first content: use legion_ai_providers::{ANTHROPIC_PROVIDER_ID, can_activate_provider, provider_tier};
Symbols: after, before, granted, has_credential, leaked, loaded, mut, Ok, path, reference, sentinel, store, tier, workspace_root

## crates/legion-desktop/tests/remote_workspace_gui.rs

crates/legion-desktop/tests/remote_workspace_gui.rs has 616 lines; symbols: _, base_descriptor, bridge, connected, connected_snapshot, disabled; first content: use std::{
Symbols: _, base_descriptor, bridge, connected, connected_snapshot, disabled, file_name, id, lsp_outcome, model, mut, nanos, offline_outcome, offline_snapshot, path, pty_outcome, reconnect_outcome, reconnect_snapshot, root, runtime, session_id, snapshot, target, temp_root, unmediated_write, workspace

## crates/legion-desktop/tests/risk_strip.rs

crates/legion-desktop/tests/risk_strip.rs has 213 lines; symbols: ask_vm, assessment, auto_vm, deny_row, findings, low; first content: use legion_desktop::view::{risk_strip_rows, risk_strip_view_model};
Symbols: ask_vm, assessment, auto_vm, deny_row, findings, low, path_finding, rows, vm

## crates/legion-desktop/tests/sandbox_panel.rs

crates/legion-desktop/tests/sandbox_panel.rs has 139 lines; symbols: all, model, mut, snapshot; first content: use legion_desktop::view::DesktopProjectionViewModel;
Symbols: all, model, mut, snapshot

## crates/legion-desktop/tests/save_all_conflict.rs

crates/legion-desktop/tests/save_all_conflict.rs has 406 lines; symbols: _, alpha, beta, beta_buffer, buffer_id, clean; first content: use std::{
Symbols: _, alpha, beta, beta_buffer, buffer_id, clean, conflicted, file_name, id, model, mut, nanos, path, root, save_all_outcome, snapshot, target, temp_root, workflow_source, workspace

## crates/legion-desktop/tests/scope_picker.rs

crates/legion-desktop/tests/scope_picker.rs has 31 lines; symbols: model, round_trip, scope; first content: use legion_desktop::view::{DesktopScopePickerViewModel, ScopeRiskTolerance, ScopeTargetKind};
Symbols: model, round_trip, scope

## crates/legion-desktop/tests/search_workflow.rs

crates/legion-desktop/tests/search_workflow.rs has 647 lines; symbols: _, file_name, first, header_all, header_ci, header_cs; first content: use std::{
Symbols: _, file_name, first, header_all, header_ci, header_cs, header_rx, header_ww, id, insensitive_count, literal_count, model, mut, nanos, partial_count, path, projection, query_id, regex_count, regex_snapshot, regex_status, root, second, sensitive_count, snapshot, structural, target, temp_root, whole_word_count, workspace

## crates/legion-desktop/tests/session_restore.rs

crates/legion-desktop/tests/session_restore.rs has 441 lines; symbols: _, delegate_layout, error, explorer_path, first, id; first content: use std::{
Symbols: _, delegate_layout, error, explorer_path, first, id, json, leftovers, loaded, mut, nanos, outcome, path, restored, restored_delegate_layout, root, saved, second, session_state, settings, snapshot, temp_root, workspace

## crates/legion-desktop/tests/terminal_panel.rs

crates/legion-desktop/tests/terminal_panel.rs has 100 lines; symbols: all_kinds, has_pascal, label, model, mut, text; first content: use legion_desktop::view::terminal_panel::TerminalPanelRenderModel;
Symbols: all_kinds, has_pascal, label, model, mut, text

## crates/legion-editor/Cargo.toml

crates/legion-editor/Cargo.toml has 17 lines; first content: [package]
Symbols: none

## crates/legion-editor/src/diff.rs

crates/legion-editor/src/diff.rs has 475 lines; symbols: anchor, changed_positions, chunk, chunks, CONTEXT_LINES, deleted; first content: use legion_protocol::{
Symbols: anchor, changed_positions, chunk, chunks, CONTEXT_LINES, deleted, h, hunks, inserted, last_idx, lines, m, mut, n, new, new_count, new_end, new_lines, new_start, old, old_count, old_lines, old_start, ops, row_width, section, section_id, slice, text, title

## crates/legion-editor/src/lib.rs

crates/legion-editor/src/lib.rs has 3633 lines; symbols: _, a, acknowledgement, approx_visible_lines, b, before; first content: pub mod diff;
Symbols: _, a, acknowledgement, approx_visible_lines, b, before, buffer, buffer_id, byte_offset, chunk, chunks, completion, completion_offset, COMPLETION_SCAN_WINDOW_BYTES, consumers, continues_after_window, correlation_id, current_after, current_before, current_snapshot, current_snapshot_id, cursor, DEFAULT_RETENTION_BUDGET_BYTES, DEFAULT_RETENTION_BUDGET_SNAPSHOTS, DEFAULT_SNAPSHOT_LEASE_TTL_MILLIS, DEFAULT_TRANSACTION_EVENT_QUEUE_CAPACITY, delta, descriptor, descriptors, detection, drained, dropped_before_drain, dto, editor, EditorApplyTransactionRequest, edits, empty, end, end_line, error, event, events, file_path, final_end, final_start, half_window, initial_text, items, kind, label

## crates/legion-editor/tests/atomicity_and_retention.rs

crates/legion-editor/tests/atomicity_and_retention.rs has 373 lines; symbols: after, before, buffer, changed, chunk, current_snapshot_id; first content: use legion_editor::{
Symbols: after, before, buffer, changed, chunk, current_snapshot_id, descriptor, dirty, expected_after, line_before, line_body, log_len, mut, pending, pending_snapshot_id, pinned, redo_len, redone, result, save, text, tx, undo_len, undone, version

## crates/legion-editor/tests/large_file_scale.rs

crates/legion-editor/tests/large_file_scale.rs has 339 lines; symbols: binary_content, buffer, current, err, err_msg, expected_after_edit; first content: use legion_editor::{BufferMode, EditorEngine, EditorThresholds, TextEdit, TextPosition};
Symbols: binary_content, buffer, current, err, err_msg, expected_after_edit, INSERT, lease, line, lines_needed, mut, ok, original, original_buffer_version, original_snapshot_id, projection, result, save, save_after_edit, save_after_redo, save_after_undo, stale_result, status, text, THRESHOLD

## crates/legion-editor/tests/large_file_streaming.rs

crates/legion-editor/tests/large_file_streaming.rs has 75 lines; symbols: buffer, LARGE_FILE_BYTES, LARGE_TEXT_LINE, mut, scroll_line, text; first content: use legion_editor::{EditorEngine, EditorError};
Symbols: buffer, LARGE_FILE_BYTES, LARGE_TEXT_LINE, mut, scroll_line, text, viewport

## crates/legion-editor/tests/performance_suite.rs

crates/legion-editor/tests/performance_suite.rs has 822 lines; symbols: _, at, baseline_buffer, baseline_p50, baseline_p95, baseline_p99; first content: use std::time::{Duration, Instant};
Symbols: _, at, baseline_buffer, baseline_p50, baseline_p95, baseline_p99, buffer, chunks, CI_LARGE_FILE_BYTES, collaboration_buffer, collaboration_p50, collaboration_p95, collaboration_p99, consumer_p50, consumer_p95, consumer_p99, consumer_summary, delete, descriptor, drained, idx, LARGE_TEXT_LINE, lease, leases, mut, open_elapsed, open_start, p50, p95, p95_overhead, p99_overhead, payload_bytes, pin_after_ack, pin_after_save, pin_before_save, post_version, redo_start, redo_total, SAMPLES, save, simulated_large_thresholds, size, start, status, text, undo_start, undo_total, viewport, viewport_elapsed, viewport_start

## crates/legion-index/Cargo.toml

crates/legion-index/Cargo.toml has 19 lines; first content: [package]
Symbols: none

## crates/legion-index/src/fuzzy.rs

crates/legion-index/src/fuzzy.rs has 322 lines; symbols: boundary, camel, candidate_lower, candidate_lower_str, candidate_raw, consecutive; first content: pub struct FuzzyScore {
Symbols: boundary, camel, candidate_lower, candidate_lower_str, candidate_raw, consecutive, contains, curr_raw, directory, exact, filename, filename_start_idx, mid, mid_path, midword, mut, prefix, prev_raw, query, query_chars, query_lower_str, result, scattered, seg_start

## crates/legion-index/src/lib.rs

crates/legion-index/src/lib.rs has 6707 lines; symbols: ack, action, active_index, after, base, base_adjust; first content: pub mod fuzzy;
Symbols: ack, action, active_index, after, base, base_adjust, before, best_index, bit_len, byte_end, byte_range, byte_ranges, byte_start, bytes, cache_key, cancellation_reason, candidate, candidate_set, canonical_path, capture, capture_end, capture_name, capture_names, capture_start, ch, chars, chunk_fingerprint, chunk_id, chunk_refs, chunk_text, chunks, citation, citation_id, code, content_hash, contributions, current, current_version, damping, declaration_ranges, declarations_in_segment, degraded_reasons, deleted, denominator, dependency_bonus, descriptor, diagnostics, digest, document_version, edits

## crates/legion-index/tests/index_workflows.rs

crates/legion-index/tests/index_workflows.rs has 3327 lines; symbols: accepted, alpha_hash, ast_has_recall, auth, auth_source, auth_v1; first content: use legion_index::{
Symbols: accepted, alpha_hash, ast_has_recall, auth, auth_source, auth_v1, auth_v2, background_doc, base, base_doc, base_request, build_symbols, captures, cases, chunks, content_hash, contract_debug, debug, decision, deferred_launch, delta, descriptor, descriptor_document, descriptor_index, descriptor_request, discovery_request, discovery_snapshot, document, edit_doc, file_index, first, first_chunk_hash, first_report, fixed_has_recall, fixture, fixture_repo, fixtures, fresh_plan, fresh_report, grammar_one, grammar_two, grammar_version, haystack, hybrid_response, hybrid_top, identity, imported, indexer, initial_audit, initial_issue

## crates/legion-index/tests/plugin_grammar.rs

crates/legion-index/tests/plugin_grammar.rs has 107 lines; symbols: _guard, language_id, loaded, outcome, parser; first content: use legion_index::{
Symbols: _guard, language_id, loaded, outcome, parser

## crates/legion-lsp/Cargo.toml

crates/legion-lsp/Cargo.toml has 17 lines; first content: [package]
Symbols: none

## crates/legion-lsp/src/bin/mock_lsp_server.rs

crates/legion-lsp/src/bin/mock_lsp_server.rs has 504 lines; symbols: _, body, code, diagnostics, envelope, frame; first content: use std::io::{self, BufRead, Read, Write};
Symbols: _, body, code, diagnostics, envelope, frame, HEADER_SEPARATOR, header_str, id, length, MAX_FRAME_PAYLOAD_BYTES, method, mut, new_name, Ok, params, payload, progress, read, register, response, result_is_null, stdin, stdout, unknown, uri

## crates/legion-lsp/src/diagnostics.rs

crates/legion-lsp/src/diagnostics.rs has 155 lines; symbols: a, b, c, end, mut, range; first content: use legion_protocol::{FileFingerprint, ProtocolDiagnosticSeverity};
Symbols: a, b, c, end, mut, range, result, start

## crates/legion-lsp/src/features.rs

crates/legion-lsp/src/features.rs has 457 lines; symbols: changes, diags, edit, edits, mut, new_text; first content: use serde_json::{Value, json};
Symbols: changes, diags, edit, edits, mut, new_text, proposal, range, request

## crates/legion-lsp/src/lib.rs

crates/legion-lsp/src/lib.rs has 3489 lines; symbols: _, adapters, authority, binary_name, bytes, cancelled; first content: pub mod diagnostics;
Symbols: _, adapters, authority, binary_name, bytes, cancelled, candidate, child, code, code_label, command, command_label, content_changes, contents, correlated, data, data_kind, data_label, deadline, decoded, degraded, detail, diagnostics, display_name, drive, elapsed_ms, end, entries, envelope, error, event, events, exe, frame, handle, header, header_end, header_str, insert_text, insert_text_raw, joined, json_rpc_id, kind, kind_label, label, language_id, last_failure_hash, length, location, locations

## crates/legion-lsp/tests/common/mod.rs

crates/legion-lsp/tests/common/mod.rs has 112 lines; symbols: mut; first content: use std::path::PathBuf;
Symbols: mut

## crates/legion-lsp/tests/discovery_contract.rs

crates/legion-lsp/tests/discovery_contract.rs has 33 lines; symbols: d; first content: use std::path::PathBuf;
Symbols: d

## crates/legion-lsp/tests/document_sync_contract.rs

crates/legion-lsp/tests/document_sync_contract.rs has 166 lines; symbols: crab_end, crab_start, did_change, did_open, document, fallback; first content: use legion_lsp::{
Symbols: crab_end, crab_start, did_change, did_open, document, fallback, line_index, params, payload, problem, projected, text, utf16_range

## crates/legion-lsp/tests/lifecycle_contract.rs

crates/legion-lsp/tests/lifecycle_contract.rs has 392 lines; symbols: cancelled, correlated, correlated_first, correlated_second, decoded, envelope; first content: use legion_lsp::{
Symbols: cancelled, correlated, correlated_first, correlated_second, decoded, envelope, events, first, first_response, frame, lower_case_frame, mut, pending, second, second_response, third, timeout

## crates/legion-lsp/tests/pump_contract.rs

crates/legion-lsp/tests/pump_contract.rs has 89 lines; symbols: deadline, deadline_ms, elapsed, mut, outcome, start; first content: use std::time::{Duration, Instant};
Symbols: deadline, deadline_ms, elapsed, mut, outcome, start

## crates/legion-lsp/tests/read_side_contract.rs

crates/legion-lsp/tests/read_side_contract.rs has 594 lines; symbols: completions, degraded, hints, hover, lenses, locations; first content: use legion_lsp::{
Symbols: completions, degraded, hints, hover, lenses, locations, long_query, outline, params, position, query, request, response

## crates/legion-lsp/tests/registry_contract.rs

crates/legion-lsp/tests/registry_contract.rs has 135 lines; symbols: entry, expected_rust_command, go, process, python, python_adapter; first content: use legion_lsp::{LanguageServerAdapterRegistry, LspServerBinaryManifest, LspServerBinarySource};
Symbols: entry, expected_rust_command, go, process, python, python_adapter, registry, rust, typescript, workspace_id

## crates/legion-lsp/tests/rust_analyzer_launch.rs

crates/legion-lsp/tests/rust_analyzer_launch.rs has 30 lines; symbols: registry, rust_configs, workspace_id; first content: use legion_lsp::LanguageServerAdapterRegistry;
Symbols: registry, rust_configs, workspace_id

## crates/legion-lsp/tests/rust_analyzer_smoke.rs

crates/legion-lsp/tests/rust_analyzer_smoke.rs has 277 lines; symbols: _, command, d, deadline, fixture_dir, forward; first content: use std::fs;
Symbols: _, command, d, deadline, fixture_dir, forward, init_params, init_response, lib_rs, lib_rs_text, lib_rs_uri, mut, outcome, root_uri, s, Some, supervisor_config, version

## crates/legion-lsp/tests/stdio_transport_contract.rs

crates/legion-lsp/tests/stdio_transport_contract.rs has 1075 lines; symbols: _, _hints, _lenses, _locations, budget_ms, call_pos; first content: use std::path::PathBuf;
Symbols: _, _hints, _lenses, _locations, budget_ms, call_pos, cancelled, completion, completion_pos, completion_rows, completions, declaration, declarations, definition, definitions, diagnostics, document, echoed, elapsed, err, exit_status, first, folding_ranges, frames_before, hover, hover_pos, implementation, mut, notification, outcome, outline, pending, position, position_after, position_of, progress, ra, references, registered_hash, response, second, semantic_tokens, signature_help, signatures, source, start, started, stats, temp_root, terminal

## crates/legion-lsp/tests/uri_fingerprint_normalization.rs

crates/legion-lsp/tests/uri_fingerprint_normalization.rs has 64 lines; symbols: upper; first content: use legion_lsp::lsp_diagnostic_uri_fingerprint as fingerprint;
Symbols: upper

## crates/legion-lsp/tests/write_side_contract.rs

crates/legion-lsp/tests/write_side_contract.rs has 163 lines; symbols: diagnostics, options, organize_imports, params, position, prepare; first content: use legion_lsp::{
Symbols: diagnostics, options, organize_imports, params, position, prepare, range, range_request, rename, request

## crates/legion-memory/Cargo.toml

crates/legion-memory/Cargo.toml has 17 lines; first content: [package]
Symbols: none

## crates/legion-memory/src/lib.rs

crates/legion-memory/src/lib.rs has 1329 lines; symbols: before, byte_offset, candidate, candidate_id, error, export_id; first content: use legion_protocol::{
Symbols: before, byte_offset, candidate, candidate_id, error, export_id, local_count, lower, manifest, mut, passed_verification_count, proposed, provider_count, record, restored_snapshot, service, session, signed_off_count, simple_markers, snapshot_json, SUPPORTED_SNAPSHOT_SCHEMA_VERSION, trace, workflow_candidate

## crates/legion-observability/Cargo.toml

crates/legion-observability/Cargo.toml has 14 lines; first content: [package]
Symbols: none

## crates/legion-observability/src/crash_capture.rs

crates/legion-observability/src/crash_capture.rs has 286 lines; symbols: _, audit, backtrace, bundle_dir, components, consent; first content: use std::path::{Path, PathBuf};
Symbols: _, audit, backtrace, bundle_dir, components, consent, crash_dir, crash_id, crate_name, location, message, msg, panic_txt, rest, result, safe_message, sanitized, summary_toml, timestamp

## crates/legion-observability/src/export.rs

crates/legion-observability/src/export.rs has 234 lines; symbols: bundle, crash_dir, crash_id, dir, entry, f; first content: use std::path::PathBuf;
Symbols: bundle, crash_dir, crash_id, dir, entry, f, has_raw, metadata_only, mut, p, raw_paths, result, summary_path

## crates/legion-observability/src/lib.rs

crates/legion-observability/src/lib.rs has 2900 lines; symbols: _, applied, applied_transition, audit, audit_event, base; first content: use std::{
Symbols: _, applied, applied_transition, audit, audit_event, base, bit_len, causality, ch, context, created, created_causality, denial, descriptor, digest, digest_input, envelope, err, escape, event, events, failed, failed_transition, fallback, K, lower, maj, manifest, metadata, METADATA_DIGEST_ALGORITHM, mut, names, overflow, path, payload, policy, previewed, previewed_transition, privacy_disposition, proposal, provider, reason, record, recovery, refusal_error_category, rejected, rejected_transition, replay_event, retention, rolled_back

## crates/legion-observability/src/minidump.rs

crates/legion-observability/src/minidump.rs has 174 lines; symbols: consent, envelope, event, mut, report, symbol_upload_state; first content: use legion_protocol::{
Symbols: consent, envelope, event, mut, report, symbol_upload_state, symbolicated

## crates/legion-observability/src/telemetry.rs

crates/legion-observability/src/telemetry.rs has 352 lines; symbols: choice, consent, envelope, event, metadata_summary, mut; first content: use crate::{
Symbols: choice, consent, envelope, event, metadata_summary, mut, record, request_id

## crates/legion-observability/src/training.rs

crates/legion-observability/src/training.rs has 327 lines; symbols: candidate, candidates, err, fixture, label, mut; first content: use crate::ObservabilityError;
Symbols: candidate, candidates, err, fixture, label, mut, Some

## crates/legion-observability/tests/crash_capture_tests.rs

crates/legion-observability/tests/crash_capture_tests.rs has 355 lines; symbols: _, _guard, bundle, bundle_dir, config, crash_dir; first content: use std::path::PathBuf;
Symbols: _, _guard, bundle, bundle_dir, config, crash_dir, crash_id, dir, dirs, lower, panic_txt, result, summary

## crates/legion-platform/Cargo.toml

crates/legion-platform/Cargo.toml has 29 lines; first content: [package]
Symbols: none

## crates/legion-platform/src/lib.rs

crates/legion-platform/src/lib.rs has 2846 lines; symbols: _, absolute, attribute_list, base, block, block_str; first content: use std::{
Symbols: _, absolute, attribute_list, base, block, block_str, bytes, candidate, child, child_env, chunk, conpty, content, current, current_dir, current_dir_ptr, cwd, deadline, dir, directory, elapsed, entries, entry, env_block, env_service, error, events, existing, exit_status, expected, expected_dir, fake, fd, file, fingerprint, flags, fs_service, handle, hash, id, key, keys, leftovers, link, list, message, metadata, MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH, mut

## crates/legion-platform/src/windows.rs

crates/legion-platform/src/windows.rs has 63 lines; first content: pub struct WindowsConptyParityContract {
Symbols: none

## crates/legion-platform/tests/windows_conpty_contract.rs

crates/legion-platform/tests/windows_conpty_contract.rs has 28 lines; symbols: contract; first content: use legion_platform::windows::{WindowsConptyParityContract, windows_conpty_parity_contract};
Symbols: contract

## crates/legion-plugin/Cargo.toml

crates/legion-plugin/Cargo.toml has 20 lines; first content: [package]
Symbols: none

## crates/legion-plugin/src/host.rs

crates/legion-plugin/src/host.rs has 389 lines; symbols: bytes, config, decision, engine, func, import_module; first content: use std::{
Symbols: bytes, config, decision, engine, func, import_module, import_name, instance, linker, loaded, module, mut, plugin_id, result, Some

## crates/legion-plugin/src/lib.rs

crates/legion-plugin/src/lib.rs has 645 lines; symbols: decision, error, mut, next_output_bytes, plugin, plugin_id; first content: pub mod host;
Symbols: decision, error, mut, next_output_bytes, plugin, plugin_id, port, registered, response, Some

## crates/legion-plugin/src/manifest.rs

crates/legion-plugin/src/manifest.rs has 148 lines; symbols: plugin_id, reason, rows; first content: use legion_protocol::{CapabilityId, PluginContribution, PluginManifest};
Symbols: plugin_id, reason, rows

## crates/legion-plugin/src/registry.rs

crates/legion-plugin/src/registry.rs has 209 lines; symbols: error, manifest, mut, plugin_id, registry, removed; first content: use std::collections::HashMap;
Symbols: error, manifest, mut, plugin_id, registry, removed, updated

## crates/legion-plugin/src/wit_bindings.rs

crates/legion-plugin/src/wit_bindings.rs has 2 lines; first content: wit_bindgen::generate!();
Symbols: none

## crates/legion-plugin/tests/hostile.rs

crates/legion-plugin/tests/hostile.rs has 171 lines; symbols: audit, error, mut, plugin_id, source, unique; first content: use std::{
Symbols: audit, error, mut, plugin_id, source, unique, wasm

## crates/legion-plugin/tests/quotas.rs

crates/legion-plugin/tests/quotas.rs has 215 lines; symbols: audit, error, mut, plugin_id, unique, value; first content: use std::{
Symbols: audit, error, mut, plugin_id, unique, value, wasm, wasm_path, wat

## crates/legion-plugin/tests/tampered.rs

crates/legion-plugin/tests/tampered.rs has 97 lines; symbols: error, manifest, mut; first content: use legion_plugin::{WasmPluginHost, registry::SignedExtensionRegistry};
Symbols: error, manifest, mut

## crates/legion-plugin/tests/wit_abi.rs

crates/legion-plugin/tests/wit_abi.rs has 25 lines; symbols: grammars, lsp, themes, wit_dir; first content: use std::{fs, path::PathBuf};
Symbols: grammars, lsp, themes, wit_dir

## crates/legion-project/Cargo.toml

crates/legion-project/Cargo.toml has 21 lines; first content: [package]
Symbols: none

## crates/legion-project/src/lib.rs

crates/legion-project/src/lib.rs has 9045 lines; symbols: _, a, absolute, absolute_path, active_relative, actor; first content: use std::collections::{HashMap, HashSet, VecDeque};
Symbols: _, a, absolute, absolute_path, active_relative, actor, actual, actual_context, actual_fingerprint, actual_identity, actual_metadata, actual_protocol_fingerprint, added, added_lines, allowed, allowed_parent, applied, args, b, base, batch, batch_size, binaries, binary_path, blame_lines, block_start, blocked, boolean, borrowed_args, branch, branch_label, byte_end, byte_len, byte_start, candidate, candidate_parts, candidate_paths, canonical, canonical_destination, canonical_name, canonical_path, canonical_repo, canonical_root, canonical_source, canonical_target, CC_PREFIXES, change_kind, changed_files, chars, checkpoint_path

## crates/legion-project/tests/debug_locator.rs

crates/legion-project/tests/debug_locator.rs has 108 lines; symbols: _, configs, file_name, id, package_bin, path; first content: use std::{
Symbols: _, configs, file_name, id, package_bin, path, project, root, temp_root

## crates/legion-project/tests/git_workflow.rs

crates/legion-project/tests/git_workflow.rs has 1134 lines; symbols: _, after_stage, cached, cached_after_unstage, cli, conflicted; first content: use std::{
Symbols: _, after_stage, cached, cached_after_unstage, cli, conflicted, content, content_incoming, contents, crlf_content, current, current_path, current_resolved, current_unchanged, diff3_content, err, expected, file_name, first_hunk, gix, head, hook_path, hooks, id, incoming, incoming_path, incoming_resolved, literal, marker_content, marker_len, merge, multi_resolved, mut, nanos, options, original, orphan, output, outside, path, prunable_snapshot, repo, repo_root, resolved, resolved2, root, sep, snapshot, source_file, source_path

## crates/legion-project/tests/harness_tools.rs

crates/legion-project/tests/harness_tools.rs has 158 lines; symbols: _, actor, grep_report, matches, mut, opened; first content: use std::{
Symbols: _, actor, grep_report, matches, mut, opened, outline, paths, policy, query, root, search_report, text

## crates/legion-project/tests/path_boundary.rs

crates/legion-project/tests/path_boundary.rs has 632 lines; symbols: _, actor, candidate, changed, deleted, delta; first content: use std::path::{Path, PathBuf};
Symbols: _, actor, candidate, changed, deleted, delta, denied_snapshot, diagnostic, err, escape, events, flipped, link_path, long_prefixed, mut, opened, opened_file, outside, outside_path, policy, resolve_err, response, root, root_text, snapshot, target, Target, text, write_attempts, write_err

## crates/legion-project/tests/search_cancellation.rs

crates/legion-project/tests/search_cancellation.rs has 279 lines; symbols: _, actor, content, file_count, first_file, mut; first content: use std::{
Symbols: _, actor, content, file_count, first_file, mut, opened, policy, report, report2, root, workspace_id

## crates/legion-project/tests/search_workspace.rs

crates/legion-project/tests/search_workspace.rs has 265 lines; symbols: _, _warmup, actor, body, deadline, file; first content: use std::{
Symbols: _, _warmup, actor, body, deadline, file, first, indexed, indexed_elapsed, indexed_query, indexed_start, live, live_elapsed, live_query, live_start, mut, needle, opened, pattern, policy, query, report, result, root, second

## crates/legion-project/tests/watcher_burst.rs

crates/legion-project/tests/watcher_burst.rs has 165 lines; symbols: _, actor, events, file, mut, opened; first content: use std::{
Symbols: _, actor, events, file, mut, opened, policy, root

## crates/legion-project/tests/watcher_recovery.rs

crates/legion-project/tests/watcher_recovery.rs has 157 lines; symbols: _, actor, deadline, events, file, first; first content: use std::path::{Path, PathBuf};
Symbols: _, actor, deadline, events, file, first, mut, names, opened, recovered, root, sink

## crates/legion-project/tests/workspace_scale.rs

crates/legion-project/tests/workspace_scale.rs has 126 lines; symbols: _, actor, dir, elapsed, file, mut; first content: use std::{
Symbols: _, actor, dir, elapsed, file, mut, opened, policy, root, t0

## crates/legion-protocol/Cargo.toml

crates/legion-protocol/Cargo.toml has 13 lines; first content: [package]
Symbols: none

## crates/legion-protocol/src/capability.rs

crates/legion-protocol/src/capability.rs has 51 lines; first content: use serde::{Deserialize, Serialize};
Symbols: none

## crates/legion-protocol/src/delegate_loop.rs

crates/legion-protocol/src/delegate_loop.rs has 143 lines; symbols: budget, decoded, json, kinds, step; first content: use serde::{Deserialize, Serialize};
Symbols: budget, decoded, json, kinds, step

## crates/legion-protocol/src/manifest.rs

crates/legion-protocol/src/manifest.rs has 128 lines; symbols: Self; first content: use serde::{Deserialize, Serialize};
Symbols: Self

## crates/legion-protocol/src/plan.rs

crates/legion-protocol/src/plan.rs has 489 lines; symbols: artifact, changed_section_count, diff_summary, index, mut, section_diffs; first content: use serde::{Deserialize, Serialize};
Symbols: artifact, changed_section_count, diff_summary, index, mut, section_diffs, summary_label

## crates/legion-protocol/src/release_manifest.rs

crates/legion-protocol/src/release_manifest.rs has 144 lines; first content: use serde::{Deserialize, Serialize};
Symbols: none

## crates/legion-protocol/src/risk.rs

crates/legion-protocol/src/risk.rs has 189 lines; first content: use serde::{Deserialize, Serialize};
Symbols: none

## crates/legion-protocol/src/scope.rs

crates/legion-protocol/src/scope.rs has 111 lines; symbols: candidate, forbidden, Some, workspace_root; first content: use serde::{Deserialize, Serialize};
Symbols: candidate, forbidden, Some, workspace_root

## crates/legion-protocol/src/tools.rs

crates/legion-protocol/src/tools.rs has 527 lines; symbols: decoded, denied, expected_required, feedback, inv, json; first content: use serde::{Deserialize, Serialize};
Symbols: decoded, denied, expected_required, feedback, inv, json, mut, properties, read, registry, rejected, required, schema, success

## crates/legion-protocol/tests/context_manifest.rs

crates/legion-protocol/tests/context_manifest.rs has 136 lines; symbols: assembly, json, json_text, record; first content: use legion_protocol::*;
Symbols: assembly, json, json_text, record

## crates/legion-protocol/tests/dto_contracts.rs

crates/legion-protocol/tests/dto_contracts.rs has 10455 lines; symbols: accept, accessibility, action, active, agent_manifest, agent_src; first content: use legion_protocol::*;
Symbols: accept, accessibility, action, active, agent_manifest, agent_src, allow, allowed, always, api_call, api_call_roundtrip, api_call_value, apply, approval, artifact_projection, assisted, attempt, audit, audit_ref, backup, batch, binding, blocked, blocker_codes, boundary, breakpoint, breakpoint_roundtrip, budget, budgets, bundle, cancel, cancel_request, capability, cases, causality_uuid, checklist, checksum, close, code_action_request, code_action_response, collaboration_audit, command_projection, commands, config, confirm, conflict, consent, contract, contract_tests_required, crash

## crates/legion-protocol/tests/lsp_server_health_record.rs

crates/legion-protocol/tests/lsp_server_health_record.rs has 48 lines; symbols: back, json, record; first content: use legion_protocol::{
Symbols: back, json, record

## crates/legion-protocol/tests/manual_mode_silence.rs

crates/legion-protocol/tests/manual_mode_silence.rs has 87 lines; symbols: MANUAL_FORBIDDEN_SURFACES; first content: use legion_protocol::{ProductMode, ProductRuntimeSurface};
Symbols: MANUAL_FORBIDDEN_SURFACES

## crates/legion-protocol/tests/mode_taxonomy.rs

crates/legion-protocol/tests/mode_taxonomy.rs has 123 lines; symbols: label, modes, variants; first content: use legion_protocol::{CANONICAL_PRODUCT_MODES, ProductMode, ProductRuntimeSurface};
Symbols: label, modes, variants

## crates/legion-protocol/tests/plan_artifact.rs

crates/legion-protocol/tests/plan_artifact.rs has 122 lines; symbols: artifact, audit_row, current, previous, revision; first content: use legion_protocol::{
Symbols: artifact, audit_row, current, previous, revision

## crates/legion-protocol/tests/scope_contracts.rs

crates/legion-protocol/tests/scope_contracts.rs has 29 lines; symbols: json, round_trip, scope; first content: use legion_protocol::{
Symbols: json, round_trip, scope

## crates/legion-remote-transport/Cargo.toml

crates/legion-remote-transport/Cargo.toml has 17 lines; first content: [package]
Symbols: none

## crates/legion-remote-transport/src/lib.rs

crates/legion-remote-transport/src/lib.rs has 1991 lines; symbols: _, accept, addrs, ahead, attempt, audit; first content: use std::collections::{HashSet, VecDeque};
Symbols: _, accept, addrs, ahead, attempt, audit, bytes, carrier, cert_path, certs, chain, checkpoint, client_config, closed, closed_addr, config, configured_digest, connector, deadline, diagnostic, digest, duplicate, endpoint, err, expired, flow, health, HEX, intermediate, key, key_path, leaf, leaf_digest, listener, manifest, matching, mismatched, mut, negotiated_alpn, open_addr, oversized, port, remaining, root, runtime, server_identity, server_name, Some, stale, stream

## crates/legion-remote/Cargo.toml

crates/legion-remote/Cargo.toml has 16 lines; first content: [package]
Symbols: none

## crates/legion-remote/src/lib.rs

crates/legion-remote/src/lib.rs has 2748 lines; symbols: accepted, audit, auth_value, body, cancelled, checkpoint; first content: use std::collections::{HashMap, HashSet};
Symbols: accepted, audit, auth_value, body, cancelled, checkpoint, client, content, content_version, denied, devcontainer, entry, error, events, evidence, feature_count, fingerprint, header_value, identity_value, image_label, inactive, known, lsp, manifest, mount_count, mut, name_label, object, operation, path, plan, process, proposal, proposal_id, pty, reason, remote_user_label, replacement, request, response, runtime, seed, semantic, session, snapshot, snapshot_id, Some, stale, status, task_id

## crates/legion-remote/tests/cloud_lane_http_transport.rs

crates/legion-remote/tests/cloud_lane_http_transport.rs has 516 lines; symbols: _, base_url, config, content_length, debug, err; first content: use std::io::{Read, Write};
Symbols: _, base_url, config, content_length, debug, err, events, evidence, header_end, header_text, listener, lower, mut, port, proposal, read, request, response, status, transport

## crates/legion-retention/Cargo.toml

crates/legion-retention/Cargo.toml has 19 lines; first content: [package]
Symbols: none

## crates/legion-retention/src/lib.rs

crates/legion-retention/src/lib.rs has 2717 lines; symbols: _, aad, ack, algorithm_id, audit, body; first content: use std::collections::HashMap;
Symbols: _, aad, ack, algorithm_id, audit, body, bundle_id, bundle_ids, bundle_path, CHACHA20_POLY1305_ALGORITHM_ID, CHACHA20_POLY1305_KEY_LEN, CHACHA20_POLY1305_NONCE_LEN, CHACHA20_POLY1305_TAG_LEN, cipher, ciphertext_and_tag, current_key_reference, decrypted, descriptor, digest, encoded, encrypted, encrypted_bundle, encrypted_fingerprint, envelope, expires_at, files, first, first_bytes, first_envelope, HEADER_LEN, HEX, high, index, index_path, index_text, key, key_provider, key_reference, kms, last, lease, linkage, loaded, low, mut, new_aad, new_envelope, new_key, new_key_reference, new_provider

## crates/legion-retention/src/privacy.rs

crates/legion-retention/src/privacy.rs has 112 lines; symbols: confirmed, rest, tombstone; first content: use legion_protocol::{
Symbols: confirmed, rest, tombstone

## crates/legion-retention/src/training.rs

crates/legion-retention/src/training.rs has 118 lines; symbols: err, linkage, tombstone; first content: use legion_protocol::{
Symbols: err, linkage, tombstone

## crates/legion-retention/tests/privacy_deletion.rs

crates/legion-retention/tests/privacy_deletion.rs has 277 lines; symbols: descriptor, exposure_id, handle, inspector_id, missing, mut; first content: use legion_protocol::{
Symbols: descriptor, exposure_id, handle, inspector_id, missing, mut, result, tombstone

## crates/legion-sandbox/Cargo.toml

crates/legion-sandbox/Cargo.toml has 30 lines; first content: [package]
Symbols: none

## crates/legion-sandbox/src/bin/sandbox-escape-probe.rs

crates/legion-sandbox/src/bin/sandbox-escape-probe.rs has 51 lines; symbols: _, addr, args, path; first content: use std::io::Write;
Symbols: _, addr, args, path

## crates/legion-sandbox/src/landlock.rs

crates/legion-sandbox/src/landlock.rs has 28 lines; first content: use crate::{SandboxBackend, SandboxProfile, SandboxScope};
Symbols: none

## crates/legion-sandbox/src/lib.rs

crates/legion-sandbox/src/lib.rs has 428 lines; symbols: action, candidate, decision, mut, normalized, path; first content: use std::{
Symbols: action, candidate, decision, mut, normalized, path, scope

## crates/legion-sandbox/src/network.rs

crates/legion-sandbox/src/network.rs has 146 lines; symbols: action, allowed, authority, canonical, decision, host; first content: use crate::{
Symbols: action, allowed, authority, canonical, decision, host, mut, rest, scope, Some, target

## crates/legion-sandbox/src/seatbelt.rs

crates/legion-sandbox/src/seatbelt.rs has 28 lines; first content: use crate::{SandboxBackend, SandboxProfile, SandboxScope};
Symbols: none

## crates/legion-sandbox/src/spawn_stdio.rs

crates/legion-sandbox/src/spawn_stdio.rs has 423 lines; symbols: _, access, backend, bwrap, child, deny_all_network; first content: use std::collections::BTreeSet;
Symbols: _, access, backend, bwrap, child, deny_all_network, mut, network_enforced, path_fd, pid, result, sandbox_exec, sbpl, stdin, stdout, TRUSTED_BWRAP_PATHS, writable_root, write_access, write_access_child, WRITE_POLICY_ABI

## crates/legion-sandbox/src/spawn.rs

crates/legion-sandbox/src/spawn.rs has 820 lines; symbols: _, abi, abi_version, app_name_ptr, app_wide, backend; first content: use std::collections::BTreeSet;
Symbols: _, abi, abi_version, app_name_ptr, app_wide, backend, bwrap, deadline, deny_all_network, exit_code, h, handle, job, job_handle, mut, network_enforced, path_fd, process_handle, result, resume_result, rights, sa, sandbox_exec, sbpl, spawn_result, stderr, stderr_read, stderr_thread, stderr_usize, stdout, stdout_read, stdout_thread, stdout_usize, thread_handle, timed_out, timeout_ms, wait_result, WAIT_TIMEOUT_VAL, WIN_INFINITE, working_dir_wide, writable_root, write_access, write_access_child, WRITE_POLICY_ABI

## crates/legion-sandbox/src/windows.rs

crates/legion-sandbox/src/windows.rs has 56 lines; first content: use crate::{SandboxBackend, SandboxError, SandboxProfile, SandboxScope};
Symbols: none

## crates/legion-sandbox/tests/compile_profiles.rs

crates/legion-sandbox/tests/compile_profiles.rs has 66 lines; symbols: profile; first content: use legion_sandbox::{
Symbols: profile

## crates/legion-sandbox/tests/escape_attempts.rs

crates/legion-sandbox/tests/escape_attempts.rs has 339 lines; symbols: dir, mut, output, outside, p, probe; first content: mod windows_tests {
Symbols: dir, mut, output, outside, p, probe, profile, result, spec, stdout, target_file, writable

## crates/legion-sandbox/tests/stdio_spawn.rs

crates/legion-sandbox/tests/stdio_spawn.rs has 71 lines; symbols: _, mut, proc, root, spec, status; first content: use std::collections::BTreeSet;
Symbols: _, mut, proc, root, spec, status

## crates/legion-security/Cargo.toml

crates/legion-security/Cargo.toml has 16 lines; first content: [package]
Symbols: none

## crates/legion-security/src/lib.rs

crates/legion-security/src/lib.rs has 4198 lines; symbols: _, absolute, access, air_gap_denied, allow, allowed; first content: use std::{
Symbols: _, absolute, access, air_gap_denied, allow, allowed, apply, broker, budget_exceeded, bundle, byte_offset, capability, capture, class, decision, decision_id, decoded, default_denied, delete, denied, denied_unknown, deny, diff, effective_max, ext, forbidden_scope, fs_read, fs_write, fs_write_no_path, generated_decision_id, grant, host, hosted, hosted_export, hosted_provider, json, listen, log, loopback, lower, MARKERS, missing, missing_command, missing_consent, missing_context, missing_visibility, mut, network_tool, no_path, operation

## crates/legion-security/src/policy.rs

crates/legion-security/src/policy.rs has 253 lines; symbols: level_str, mut, policy, rule_ids; first content: use std::collections::HashMap;
Symbols: level_str, mut, policy, rule_ids

## crates/legion-security/src/risk.rs

crates/legion-security/src/risk.rs has 360 lines; symbols: aggregate_risk_label, deletion_ratio, escapes_scope, findings, mut, name; first content: pub enum RiskLevel {
Symbols: aggregate_risk_label, deletion_ratio, escapes_scope, findings, mut, name, normalized, normalized_root, Some, touched

## crates/legion-security/tests/advisory_classifier_wiring.rs

crates/legion-security/tests/advisory_classifier_wiring.rs has 120 lines; symbols: assessment, assessment2, path_finding; first content: use legion_protocol::ProposalRiskLabel;
Symbols: assessment, assessment2, path_finding

## crates/legion-security/tests/graduated_approval.rs

crates/legion-security/tests/graduated_approval.rs has 223 lines; symbols: assessment, engine, gate, level, meta, metadata; first content: use legion_protocol::ProposalRiskLabel;
Symbols: assessment, engine, gate, level, meta, metadata, mut, path_finding

## crates/legion-security/tests/org_policy_bundle.rs

crates/legion-security/tests/org_policy_bundle.rs has 232 lines; symbols: bundle, cloud_submit_ok, cloud_submit_over_budget, principal, remote_provider_context, retention_context; first content: use legion_protocol::risk::RiskRuleId;
Symbols: bundle, cloud_submit_ok, cloud_submit_over_budget, principal, remote_provider_context, retention_context, telemetry_context

## crates/legion-security/tests/path_policy_windows.rs

crates/legion-security/tests/path_policy_windows.rs has 56 lines; symbols: policy; first content: use legion_security::{PathAccess, PathPolicy};
Symbols: policy

## crates/legion-security/tests/proposal_apply_gate.rs

crates/legion-security/tests/proposal_apply_gate.rs has 47 lines; symbols: gate; first content: use legion_protocol::ProposalRiskLabel;
Symbols: gate

## crates/legion-security/tests/proposal_auto_approval_policy.rs

crates/legion-security/tests/proposal_auto_approval_policy.rs has 33 lines; symbols: enabled, missing_rule, policy; first content: use legion_protocol::risk::RiskRuleId;
Symbols: enabled, missing_rule, policy

## crates/legion-security/tests/risk_rules.rs

crates/legion-security/tests/risk_rules.rs has 240 lines; symbols: allow_assessment, allow_finding, assessment, cases, deny_assessment, deny_finding; first content: use legion_protocol::ProposalRiskLabel;
Symbols: allow_assessment, allow_finding, assessment, cases, deny_assessment, deny_finding, engine

## crates/legion-security/tests/secrets.rs

crates/legion-security/tests/secrets.rs has 59 lines; symbols: report; first content: use legion_security::{RedactionPayloadKind, scan_payload_for_sensitive_markers};
Symbols: report

## crates/legion-storage/Cargo.toml

crates/legion-storage/Cargo.toml has 19 lines; first content: [package]
Symbols: none

## crates/legion-storage/src/checkpoint.rs

crates/legion-storage/src/checkpoint.rs has 508 lines; symbols: _, all, audit_dir, base_dir, body, by_proposal; first content: use std::fs::{self, OpenOptions};
Symbols: _, all, audit_dir, base_dir, body, by_proposal, cp, event_tag, filename, list, loaded, mut, Ok, other, parent, path, record, Some, store2, suffix, temp, tmp, write_result

## crates/legion-storage/src/lib.rs

crates/legion-storage/src/lib.rs has 5064 lines; symbols: _, access, assist_right, assisted, audit, backup; first content: pub mod plan;
Symbols: _, access, assist_right, assisted, audit, backup, backup_dir, backup_id, backup_path, base, base_dir, before, bit_len, body, breakpoint_id, bytes, ch, collaboration, computed, config, count, counts, delegated, denied, digest, durable_audit, emitted, entry, err, Error, escape, event, excess, file_id, file_name, file_state, first, freshness_key, get, had_existing, id, invalid, invalid_mode, invalid_side, invalid_splitter, K, key, layouts, LEGACY_STORAGE_CHECKSUM_ALGORITHM, loaded

## crates/legion-storage/src/local_history.rs

crates/legion-storage/src/local_history.rs has 239 lines; symbols: all, evicted, found, mut, records, Some; first content: use std::collections::HashMap;
Symbols: all, evicted, found, mut, records, Some, start

## crates/legion-storage/src/plan.rs

crates/legion-storage/src/plan.rs has 215 lines; symbols: audit_row, current, first, mut, plan_id, previous; first content: use std::collections::HashMap;
Symbols: audit_row, current, first, mut, plan_id, previous, revision_id, second

## crates/legion-storage/src/secrets.rs

crates/legion-storage/src/secrets.rs has 239 lines; symbols: debug, entry, reference, store; first content: use std::collections::HashMap;
Symbols: debug, entry, reference, store

## crates/legion-storage/tests/debug_breakpoints.rs

crates/legion-storage/tests/debug_breakpoints.rs has 100 lines; symbols: deleted, deleted_again, loaded, loaded_after_delete, record, repo; first content: use legion_protocol::{
Symbols: deleted, deleted_again, loaded, loaded_after_delete, record, repo, saved

## crates/legion-storage/tests/plan_revisions.rs

crates/legion-storage/tests/plan_revisions.rs has 205 lines; symbols: _, audit_row, body, current, duplicate_revision, err; first content: use legion_protocol::{
Symbols: _, audit_row, body, current, duplicate_revision, err, error, mut, path, persisted, previous, quarantined, reloaded, revision, revision_one, revision_two, revision_two_plan

## crates/legion-telemetry/Cargo.toml

crates/legion-telemetry/Cargo.toml has 16 lines; first content: [package]
Symbols: none

## crates/legion-telemetry/src/lib.rs

crates/legion-telemetry/src/lib.rs has 1328 lines; symbols: _, accepted, batch, before, body, client; first content: use std::collections::{HashSet, VecDeque};
Symbols: _, accepted, batch, before, body, client, count, date, day, days, decoded, delay, doe, doy, endpoint, era, file_name, first, first_batch, first_sequence, hour, invalid, last, last_sequence, max_records, millis, minute, month, month_prime, MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH, mut, now, ok, outcome, parent, parts, path, pending_ids, records, removed, response, result, second, second_batch, seconds, short_year, spool, state, status

## crates/legion-terminal/Cargo.toml

crates/legion-terminal/Cargo.toml has 20 lines; first content: [package]
Symbols: none

## crates/legion-terminal/src/conpty.rs

crates/legion-terminal/src/conpty.rs has 46 lines; symbols: platform; first content: pub struct ConptyParityContract {
Symbols: platform

## crates/legion-terminal/src/grid.rs

crates/legion-terminal/src/grid.rs has 115 lines; symbols: mut, rows; first content: use legion_protocol::{
Symbols: mut, rows

## crates/legion-terminal/src/lib.rs

crates/legion-terminal/src/lib.rs has 2143 lines; symbols: _, adapter_type, audit, aws, backend, bearer; first content: pub mod conpty;
Symbols: _, adapter_type, audit, aws, backend, bearer, breakpoints, byte_count, c, calls, causality_id, chunk, clean, cleaned, close, close_session_id, correlation_id, ctx, deadline, denied, effective_limit, env, eq_end, event_sequence, exit, extend, first, first_path, first_range, gh, header_len, ident_lower, idle_timeout, input, interrupt, kill, kill_session_id, killed, killed_session_id, label, launch, launched, limit, line_end, lower, lower_header, metadata, mixed_header, mode, mut

## crates/legion-terminal/src/osc.rs

crates/legion-terminal/src/osc.rs has 330 lines; symbols: bytes, decoded_path, high, host, low, marker; first content: pub struct TerminalShellProjection {
Symbols: bytes, decoded_path, high, host, low, marker, mut, osc_start, parsed, seq_start, sequence, Some, value

## crates/legion-terminal/src/session.rs

crates/legion-terminal/src/session.rs has 30 lines; first content: use crate::osc::{TerminalShellBoundary, TerminalShellProjection};
Symbols: none

## crates/legion-terminal/tests/conpty_parity.rs

crates/legion-terminal/tests/conpty_parity.rs has 23 lines; symbols: contract; first content: use legion_terminal::conpty::{ConptyParityContract, conpty_parity_contract};
Symbols: contract

## crates/legion-terminal/tests/dap_adapter_fixture.rs

crates/legion-terminal/tests/dap_adapter_fixture.rs has 140 lines; symbols: breakpoint, launched, runtime, stepped; first content: use legion_protocol::{
Symbols: breakpoint, launched, runtime, stepped

## crates/legion-terminal/tests/dap_client_state_machine.rs

crates/legion-terminal/tests/dap_client_state_machine.rs has 293 lines; symbols: attach, capabilities, continue_request, continue_response, fixture, initialize; first content: use legion_protocol::{
Symbols: attach, capabilities, continue_request, continue_response, fixture, initialize, initialize_response, launch, launch_response, launched, matched_initialize, mut, scopes, stack_trace, stepped, stray_event, threads, variables

## crates/legion-terminal/tests/osc_tracking.rs

crates/legion-terminal/tests/osc_tracking.rs has 57 lines; symbols: cwd_projection, exit_projection, local, mut, parsed, payload; first content: use legion_terminal::osc::{TerminalShellBoundary, parse_terminal_shell_output};
Symbols: cwd_projection, exit_projection, local, mut, parsed, payload, unc, windows

## crates/legion-terminal/tests/platform_shell_smoke.rs

crates/legion-terminal/tests/platform_shell_smoke.rs has 576 lines; symbols: baseline, BASELINE_KEYS, CONTROL_KEY, CONTROL_VAL, CUSTOM_KEY, CUSTOM_VAL; first content: use legion_platform::NativePtyService;
Symbols: baseline, BASELINE_KEYS, CONTROL_KEY, CONTROL_VAL, CUSTOM_KEY, CUSTOM_VAL, deadline, filtered_env, mut, outcome, output, pwsh_available, request, result, runtime, SECRET_KEY, SECRET_VAL, service, session, which, zsh_available

## crates/legion-terminal/tests/terminal_grid.rs

crates/legion-terminal/tests/terminal_grid.rs has 85 lines; symbols: grid, mut; first content: use legion_protocol::{
Symbols: grid, mut

## crates/legion-text/Cargo.toml

crates/legion-text/Cargo.toml has 18 lines; first content: [package]
Symbols: none

## crates/legion-text/src/binary.rs

crates/legion-text/src/binary.rs has 185 lines; symbols: data, elf_start, mut, png_start, utf8, window; first content: use memchr::memchr;
Symbols: data, elf_start, mut, png_start, utf8, window

## crates/legion-text/src/lib.rs

crates/legion-text/src/lib.rs has 2391 lines; symbols: _, after, before, big, boundary, buf; first content: use std::cmp::Ordering;
Symbols: _, after, before, big, boundary, buf, byte, byte_delta, byte_offset, bytes, candidate, char_idx, chunk, chunks, clamped, clone, column, column_end, context, context_offset, cr_offset, crab_count, crab_index, DEFAULT_CHUNK_BOUNDARY_WINDOW_BYTES, DEFAULT_CHUNK_FORCE_MAX_BYTES, DEFAULT_CHUNK_TARGET_BYTES, DEFAULT_LEAF_TARGET_BYTES, DEFAULT_LINE_SLICE_MAX_BYTES, descriptor, edit_chunk_index, edit_line_index, edit_start, end, end_char, end_line, err, first, first_chunk, first_chunk_text, force_max, full_text_cache, idx, len, line, line_a, line_b, line_c, line_count, line_index, lines

## crates/legion-text/tests/large_scale_100mb.rs

crates/legion-text/tests/large_scale_100mb.rs has 252 lines; symbols: buf, byte_len, ceiling, chunks, doc_len, elapsed; first content: use legion_protocol::BufferVersion;
Symbols: buf, byte_len, ceiling, chunks, doc_len, elapsed, footprint, insert_offset, last, line_count, line_len, LINE_PATTERN, mid, mut, original_len, slices, snapshot, t0, TARGET_BYTES, text, text_len, threshold_ms, total_lines, viewport_end, viewport_start

## crates/legion-tracker/Cargo.toml

crates/legion-tracker/Cargo.toml has 15 lines; first content: [package]
Symbols: none

## crates/legion-tracker/src/lib.rs

crates/legion-tracker/src/lib.rs has 522 lines; symbols: audit, by_proposal, by_run, conflicts, error, mut; first content: use legion_protocol::{
Symbols: audit, by_proposal, by_run, conflicts, error, mut, record, run_id, session_id, worker_id

## crates/legion-ui/Cargo.toml

crates/legion-ui/Cargo.toml has 16 lines; first content: [package]
Symbols: none

## crates/legion-ui/src/lib.rs

crates/legion-ui/src/lib.rs has 39 lines; first content: pub mod projection;
Symbols: none

## crates/legion-ui/src/projection.rs

crates/legion-ui/src/projection.rs has 676 lines; symbols: card, cards, columns, files_label, kind, kinds; first content: use legion_protocol::{
Symbols: card, cards, columns, files_label, kind, kinds, label, labels, matching_rows, mut, projected_row, projection, represented_targets, summary_label, total_targets, unlinked_cards, verification_projection

## crates/legion-ui/src/ui.rs

crates/legion-ui/src/ui.rs has 8102 lines; symbols: absolute, active, actual, all_stack, all_visible, allowed; first content: use legion_protocol::{
Symbols: absolute, active, actual, all_stack, all_visible, allowed, assist, assistant, assisted, automate, before, body, budgets, buffer_id, capabilities, character, checklist, code, cols, command_count, command_id, commands, condition, debug, decision, delegated, descriptor, diagnostics, dismissed, dismissed_stack, disposition, enabled_count, end, error, errors_only_stack, excluded_count, expected, first, FORBIDDEN_MANUAL_PANEL_IDS, FORBIDDEN_MANUAL_SURFACES, graph, groups, has_forbidden_capability, hit_condition, hunk_id, inspect, inspector, instruction_label, intent, item_id

## crates/legion-ui/tests/assist_inline_prediction.rs

crates/legion-ui/tests/assist_inline_prediction.rs has 140 lines; symbols: before_commands, mut, projection, row; first content: use legion_protocol::{
Symbols: before_commands, mut, projection, row

## crates/legion-ui/tests/debug_projection.rs

crates/legion-ui/tests/debug_projection.rs has 212 lines; symbols: before_commands, debug_projection, mut, session_id, source_path; first content: use legion_protocol::{
Symbols: before_commands, debug_projection, mut, session_id, source_path

## crates/legion-ui/tests/legion_workflow_board_projection.rs

crates/legion-ui/tests/legion_workflow_board_projection.rs has 119 lines; symbols: columns, kinds, projection; first content: use legion_protocol::{
Symbols: columns, kinds, projection

## crates/legion-vscode-compat/Cargo.toml

crates/legion-vscode-compat/Cargo.toml has 15 lines; first content: [package]
Symbols: none

## crates/legion-vscode-compat/src/lib.rs

crates/legion-vscode-compat/src/lib.rs has 988 lines; symbols: _redaction_marker, activation_events, bad_controls, contributions, diagnostics, display_name; first content: use std::collections::BTreeSet;
Symbols: _redaction_marker, activation_events, bad_controls, contributions, diagnostics, display_name, download_url, engine_vscode, entrypoint_floor, error, extension_kinds, has_executable_entrypoint, host_session, loaded, manifest, missing, mut, name, namespace, process_label, publisher, requested_capabilities, required_tier, resolved, runtime, session, Some, status, version

## crates/legion-vscode-compat/tests/compat_report.rs

crates/legion-vscode-compat/tests/compat_report.rs has 134 lines; symbols: loaded, resolved; first content: use legion_protocol::{
Symbols: loaded, resolved

## deny.toml

deny.toml has 161 lines; headings: Milestone-0 baseline policy for cargo-deny., This baseline enforces dependency governance used by CI gate checks., RUSTSEC-2026-0194 (quick-xml 0.39.4, quadratic-time attribute-duplicate check); first content: [advisories]
Symbols: none

## docs/ARCHITECTURE_AUTHORITY_BOUNDARIES.md

docs/ARCHITECTURE_AUTHORITY_BOUNDARIES.md has 134 lines; headings: Legion Architecture Authority Boundaries, UI and desktop, App composition; first content: Legion is built around strict ownership boundaries. These boundaries must be preserved when implementing the consolidate
Symbols: none

## docs/hygiene-allowlist.toml

docs/hygiene-allowlist.toml has 29 lines; headings: Paths in this file are repo-relative prefixes or exact file paths., They are intentionally limited to historical/generated documentation surfaces., New active documentation should use Legion naming and valid relative paths.; first content: allowlisted_paths = [
Symbols: none

## docs/INDEX.md

docs/INDEX.md has 55 lines; headings: Legion IDE — Documentation Index, Audience map, Canonical documents; first content: This index is the canonical entry point for the Legion IDE documentation set under `docs/`. Use it as a starting point w
Symbols: none

## docs/KEYBOARD_REFERENCE.md

docs/KEYBOARD_REFERENCE.md has 56 lines; headings: Legion Keyboard Reference, Projected shortcut labels, SCM diff review navigation; first content: This page lists the shortcut labels currently projected by Legion.
Symbols: none

## docs/LEGION_PIVOT.md

docs/LEGION_PIVOT.md has 45 lines; headings: Legion Pivot, Naming strategy, Product promise; first content: Legion IDE is the user-facing product direction for this repository.
Symbols: none

## docs/LEGION_RENAME.md

docs/LEGION_RENAME.md has 41 lines; headings: Legion Rename, Crate mapping, Commands; first content: The repository now uses the canonical Legion namespace for active code, packages, scripts, and docs.
Symbols: none

## docs/MODES.md

docs/MODES.md has 79 lines; headings: Legion Product Modes, Manual, Assist; first content: Legion has four primary modes. Mode policy is a product contract, not a visual preference.
Symbols: none

## docs/OPERATOR_RUNBOOK.md

docs/OPERATOR_RUNBOOK.md has 253 lines; headings: Legion Operator Runbook, Local verification gates, Golden-path smoke promotion criteria (Tier 0); first content: This runbook is the operational companion to `plans/legion-e2e/00_CONSOLIDATED_E2E_IMPLEMENTATION_PLAN.md`.
Symbols: none

## docs/releases/v8.0.0/migration-policy.md

docs/releases/v8.0.0/migration-policy.md has 371 lines; headings: Legion IDE v8.0.0 — Migration Policy, 1. Scope and goals, 1.1 What "migration" means for v8.0.0; first content: > **STATUS: FORWARD-LOOKING TEMPLATE — NOT A CURRENT RELEASE ARTIFACT.** No v8.0.0 release exists or is scheduled. The w
Symbols: none

## docs/releases/v8.0.0/release-checklist.md

docs/releases/v8.0.0/release-checklist.md has 713 lines; headings: Legion IDE v8.0.0 — GA Release Checklist & Freeze Criteria, Owner convention, 1. Code-freeze trigger; first content: > **STATUS: FORWARD-LOOKING TEMPLATE — NOT A CURRENT RELEASE ARTIFACT.** No v8.0.0 release exists or is scheduled. The w
Symbols: none

## docs/releases/v8.0.0/rollback-policy.md

docs/releases/v8.0.0/rollback-policy.md has 335 lines; headings: Legion IDE v8.0.0 — Rollback Policy, 1. Scope and goals, 1.1 What "rollback" means for v8.0.0; first content: > **STATUS: FORWARD-LOOKING TEMPLATE — NOT A CURRENT RELEASE ARTIFACT.** No v8.0.0 release exists or is scheduled. The w
Symbols: none

## docs/SECURITY.md

docs/SECURITY.md has 172 lines; headings: Legion Security Model and Disclosure Policy, Security principles, Mutation gating; first content: This document describes the public-facing security posture of Legion IDE as it exists today: what the product is designe
Symbols: none

## docs/superpowers/plans/2026-06-19-ws-manual-01-editor-feel-rendering-input.md

docs/superpowers/plans/2026-06-19-ws-manual-01-editor-feel-rendering-input.md has 1699 lines; symbols: _, budgets, buffer, buffer_id, column, cursor; headings: WS-MANUAL-01 Editor Feel, Rendering, and Input Implementation Plan, Current Branch Facts to Preserve, Files to Create; first content: > **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:
Symbols: _, budgets, buffer, buffer_id, column, cursor, evidence, fallback_found, idx, keypress_p50, keypress_p95, label, large, launch, MANUAL_RENDERER_KEYPRESS_P50_BUDGET_MILLIS, MANUAL_RENDERER_KEYPRESS_P95_BUDGET_MILLIS, MANUAL_RENDERER_SAMPLE_COUNT, MANUAL_RENDERER_SCROLL_P95_BUDGET_MILLIS, measurement, model, mut, passed, projection, report, report_path, root, runtime, save, scroll_p95, scroll_started, seq, snapshot, Some, started, status, text, workspace

## docs/superpowers/plans/2026-06-19-ws-manual-02-large-files-workspace-scale.md

docs/superpowers/plans/2026-06-19-ws-manual-02-large-files-workspace-scale.md has 1350 lines; symbols: after_edit_save, after_redo_save, after_undo_save, banner_text, binary_content, BINARY_DETECTION_WINDOW_BYTES; headings: WS-MANUAL-02 Large Files and Workspace Scale Implementation Plan, Current Codebase Facts to Preserve, Files to Create; first content: > **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:
Symbols: after_edit_save, after_redo_save, after_undo_save, banner_text, binary_content, BINARY_DETECTION_WINDOW_BYTES, buf, buffer_id, bytes_per_line, ceiling, chunk, chunks, creation_elapsed, data, detection, edit_offset, elapsed, elf_header, err, err_msg, expected_len, gen_elapsed, lease, line, LINE_CONTENT, line_count, line_with_newline, lines_needed, memory, MEMORY_CEILING_DEFAULT_BUDGET_BYTES, MEMORY_CEILING_FIXTURE_BYTES, mid_line, mid_offset, mut, ONE_HUNDRED_MB, original, png_header, post_snapshot_id, pre_snapshot_id, projection, result, save_dto, scan_len, slices, snap_elapsed, snapshot, start, status, text, threshold

## docs/superpowers/plans/2026-06-19-ws-p0-rebaseline-ledgers-plan-hygiene.md

docs/superpowers/plans/2026-06-19-ws-p0-rebaseline-ledgers-plan-hygiene.md has 559 lines; symbols: LATEST_PRODUCTION_MASTER_PLAN, PRODUCTION_PLAN_ENTRYPOINTS, repo, result, violations; headings: WS-P0 Rebaseline, Ledgers, and Plan Hygiene Implementation Plan, Current Branch Facts to Preserve, Files to Edit; first content: > **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:
Symbols: LATEST_PRODUCTION_MASTER_PLAN, PRODUCTION_PLAN_ENTRYPOINTS, repo, result, violations

## docs/superpowers/plans/2026-06-21-ws-lang-01-rust-lsp-product-workflow.md

docs/superpowers/plans/2026-06-21-ws-lang-01-rust-lsp-product-workflow.md has 1666 lines; symbols: _, attempt, back, before, broker, candidate; headings: WS-LANG-01 Rust LSP Product Workflow Implementation Plan, Global Constraints, Existing Substrate to Reuse (do not rebuild); first content: > **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:
Symbols: _, attempt, back, before, broker, candidate, config, d, deadline, diags, envelope, exe, health, input, invocations, json, mut, outcome, output, p, params, policy, proposal, provenance, provenance_label, raw, record, request, response, result, server_edit, session, Some, status, status_label, summary, text, upper, version, workspace_edit

## docs/superpowers/plans/2026-07-02-legion-production-shippable-program.md

docs/superpowers/plans/2026-07-02-legion-production-shippable-program.md has 541 lines; symbols: _, cells, entry, FORBIDDEN_PHRASES, gate_cell, ledger; headings: Legion IDE Production-Shippable Program Plan, Global Constraints, Phase 0 — Truth Repair and Gate Restoration (active now); first content: > **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:
Symbols: _, cells, entry, FORBIDDEN_PHRASES, gate_cell, ledger, lower, mut, name, NEGATION_MARKERS, path, removed, root, rows, Some, violations, worktree_removed

## docs/superpowers/specs/2026-06-21-ws-lang-01-rust-lsp-product-workflow-design.md

docs/superpowers/specs/2026-06-21-ws-lang-01-rust-lsp-product-workflow-design.md has 234 lines; headings: WS-LANG-01 Rust LSP Product Workflow — Design, 1. Problem and Goal, 2. Shaping Decisions (locked); first content: - Status: Approved for implementation planning
Symbols: none

## docs/TROUBLESHOOTING.md

docs/TROUBLESHOOTING.md has 164 lines; headings: Legion Troubleshooting and Diagnostics, Fast triage checklist, Common support artifacts; first content: Use this page when a smoke test, packaging run, release gate, or projected workflow fails.
Symbols: none

## docs/USER_GUIDE.md

docs/USER_GUIDE.md has 172 lines; headings: Legion User Guide, Start here, Core product paths; first content: This guide is the end-user entry point for the current Legion product paths.
Symbols: none

## ENGINEERING_AUDIT.html

ENGINEERING_AUDIT.html has 108 lines; first content: <!doctype html><html><head><meta charset='utf-8'>
Symbols: none

## ENGINEERING_AUDIT.yaml

ENGINEERING_AUDIT.yaml has 530 lines; first content: schema_version: 1
Symbols: none

## ENGINEERING_PLAN.html

ENGINEERING_PLAN.html has 171 lines; first content: <!doctype html><html><head><meta charset='utf-8'>
Symbols: none

## ENGINEERING_PLAN.yaml

ENGINEERING_PLAN.yaml has 383 lines; first content: schema_version: 1
Symbols: none

## ENGINEERING_STATUS.md

ENGINEERING_STATUS.md has 76 lines; headings: Engineering Audit Status — Legion IDE, Audit Result, Counts; first content: > **Historical snapshot.** This file is a point-in-time engineering audit status record (Date 2026-06-03 UTC, Branch `ma
Symbols: none

## evals/__init__.py

evals/__init__.py has 2 lines; first content: """Legion Phase 8 evaluation helpers."""
Symbols: none

## evals/legion-bench/hostile/exfiltration.toml

evals/legion-bench/hostile/exfiltration.toml has 8 lines; first content: id = "hostile-exfiltration"
Symbols: none

## evals/legion-bench/hostile/hostile-file.toml

evals/legion-bench/hostile/hostile-file.toml has 8 lines; first content: id = "hostile-hostile-file"
Symbols: none

## evals/legion-bench/hostile/prompt-injection.toml

evals/legion-bench/hostile/prompt-injection.toml has 8 lines; first content: id = "hostile-prompt-injection"
Symbols: none

## evals/legion-bench/hostile/tool-output.toml

evals/legion-bench/hostile/tool-output.toml has 8 lines; first content: id = "hostile-tool-output"
Symbols: none

## evals/README.md

evals/README.md has 34 lines; headings: Legion Evaluation Harness, Dry-run (CI-safe), Offline fixture mode (no network); first content: `evals/run_eval.py` records the Phase 8 evaluation contract and can run in multiple modes:
Symbols: none

## evals/recorded/anthropic_smoke.json

evals/recorded/anthropic_smoke.json has 23 lines; first content: {
Symbols: none

## evals/recorded/provider_smoke_fixture.json

evals/recorded/provider_smoke_fixture.json has 15 lines; first content: {
Symbols: none

## evals/run_eval.py

evals/run_eval.py has 360 lines; symbols: _call_endpoint, _load_jsonl, _rate, _redact_endpoint, _run_endpoint, _run_offline; headings: Lightweight heuristics for real endpoint mode; first content: """Legion specialist evaluation harness with optional offline and endpoint modes."""
Symbols: _call_endpoint, _load_jsonl, _rate, _redact_endpoint, _run_endpoint, _run_offline, _run_reviewer_fixture, _subset_rate, main

## evals/test_run_eval.py

evals/test_run_eval.py has 57 lines; symbols: ReviewerFixtureEvalTest, test_reviewer_fixture_cli_writes_output, test_reviewer_fixture_flags_seeded_bug; first content: from __future__ import annotations
Symbols: ReviewerFixtureEvalTest, test_reviewer_fixture_cli_writes_output, test_reviewer_fixture_flags_seeded_bug

## evals/ws10_t4_hybrid.json

evals/ws10_t4_hybrid.json has 74 lines; first content: {
Symbols: none

## fixtures/gp1-rust/Cargo.toml

fixtures/gp1-rust/Cargo.toml has 5 lines; first content: [package]
Symbols: none

## fixtures/gp1-rust/README.md

fixtures/gp1-rust/README.md has 30 lines; headings: GP-1 Smoke Fixture, What this fixture is, Contents; first content: This directory is a **smoke test fixture** used exclusively by the Legion IDE
Symbols: none

## fixtures/gp1-rust/src/main.rs

fixtures/gp1-rust/src/main.rs has 21 lines; first content: mod scratchpad;
Symbols: none

## fixtures/gp1-rust/src/scratchpad.rs

fixtures/gp1-rust/src/scratchpad.rs has 10 lines; first content: pub fn scratchpad() {}
Symbols: none

## HERMESGOAL-GAP-ANALYSIS.md

HERMESGOAL-GAP-ANALYSIS.md has 131 lines; headings: HERMESGOAL Gap Analysis — Built vs Deferred vs Ignored, 1. Headline verdict, 2. Milestone-by-milestone status; first content: Date: 2026-07-01
Symbols: none

## HERMESGOAL.md

HERMESGOAL.md has 1093 lines; headings: 0. Repository and canonical source order, 1. Product end state to build toward, 1.1 Manual excellence; first content: /goal
Symbols: none

## mockups/ATTRIBUTIONS.md

mockups/ATTRIBUTIONS.md has 4 lines; first content: This Figma Make file includes components from [shadcn/ui](https://ui.shadcn.com/) used under [MIT license](https://githu
Symbols: none

## mockups/default_shadcn_theme.css

mockups/default_shadcn_theme.css has 121 lines; first content: /* KEEP_IN_SYNC(fullscreen/resources/figmake/shadcn/globals.css) */
Symbols: none

## mockups/design.md

mockups/design.md has 1256 lines; headings: Legion IDE Design System, 1. Product Identity, Core Promise; first content: _A starting style guide for a fast, native-feeling, AI-native development environment._
Symbols: none

## mockups/guidelines/components.md

mockups/guidelines/components.md has 21 lines; first content: This file documents per-component usage patterns specific to this kit.
Symbols: none

## mockups/guidelines/Guidelines.md

mockups/guidelines/Guidelines.md has 62 lines; headings: General guidelines, Design system guidelines, Button; first content: **Add your own guidelines here**
Symbols: none

## mockups/guidelines/setup.md

mockups/guidelines/setup.md has 1 lines; first content: **Add your own guidelines here**
Symbols: none

## mockups/guidelines/styles.md

mockups/guidelines/styles.md has 14 lines; first content: **Add your own guidelines here**
Symbols: none

## mockups/guidelines/text.txt

mockups/guidelines/text.txt has 2 lines
Symbols: none

## mockups/guidelines/tokens.md

mockups/guidelines/tokens.md has 24 lines; first content: This file documents per-token usage patterns specific to this kit.
Symbols: none

## mockups/index.html

mockups/index.html has 13 lines; first content: <!doctype html>
Symbols: none

## mockups/package.json

mockups/package.json has 59 lines; first content: {
Symbols: none

## mockups/pnpm-workspace.yaml

mockups/pnpm-workspace.yaml has 6 lines; first content: packages:
Symbols: none

## mockups/postcss.config.mjs

mockups/postcss.config.mjs has 16 lines; first content: /**
Symbols: none

## mockups/src/app/App.tsx

mockups/src/app/App.tsx has 75 lines; symbols: manual; first content: import { useState } from "react";
Symbols: manual

## mockups/src/app/components/BottomConsole.tsx

mockups/src/app/components/BottomConsole.tsx has 576 lines; symbols: active, AGENT_LOGS, AI_SUGGESTIONS, assisted, BottomConsole, COMM; first content: import {
Symbols: active, AGENT_LOGS, AI_SUGGESTIONS, assisted, BottomConsole, COMM, copilot, delegated, effectiveTab, fleet, FLEET_COMM, FLEET_TERMINAL, FleetBottomConsole, manual, manualDiagnostics, REASONING, TABS, TABS_ASSISTED, TABS_COPILOT, TABS_DELEGATED, TABS_FULL, TABS_MANUAL, TERMINAL, TERMINAL_COPILOT, TERMINAL_MANUAL

## mockups/src/app/components/CodeCanvas.tsx

mockups/src/app/components/CodeCanvas.tsx has 1037 lines; symbols: active, activeTab, bg, CODE, CODE_AI, CODE_ASSISTED; first content: import { X, Circle, ChevronRight, ChevronDown, Sparkles, Check, CornerDownLeft, Wand2, Lightbulb, TestTube2, FileText as
Symbols: active, activeTab, bg, CODE, CODE_AI, CODE_ASSISTED, CODE_MANUAL, CodeCanvas, COLUMNS, CopilotCanvas, DELEGATED_DIFF, DelegatedCanvas, DIFF_FILES, done, EDITOR_LINES, FLEET_COLUMNS, FleetCanvas, Line, manual, mark, markColor, PLAN_STEPS, TABS, TABS_AI, TABS_ASSISTED, TABS_MANUAL

## mockups/src/app/components/LeftSidebar.tsx

mockups/src/app/components/LeftSidebar.tsx has 571 lines; symbols: agents, AGENTS, assisted, copilot, delegated, FileRow; first content: import {
Symbols: agents, AGENTS, assisted, copilot, delegated, FileRow, fleet, fleetTeams, LeftSidebar, manual, MANUAL_SERVICES, ManualToolchainPanel, s, Status, STATUS, TOOL_HEALTH_COLOR, WORKSPACE_PACKAGES

## mockups/src/app/components/ProductModeSwitch.tsx

mockups/src/app/components/ProductModeSwitch.tsx has 203 lines; symbols: cancelPending, confirmPending, handleLevelClick, Icon, isActive, isHovered; first content: import { useState, useRef, useEffect } from "react";
Symbols: cancelPending, confirmPending, handleLevelClick, Icon, isActive, isHovered, isPending, LEVELS, ProductModeSwitch

## mockups/src/app/components/RightInspector.tsx

mockups/src/app/components/RightInspector.tsx has 1112 lines; symbols: ACCENT, ACTIONS, active, ACTIVITY, APPROVALS, APPROVALS_DEL; first content: import {
Symbols: ACCENT, ACTIONS, active, ACTIVITY, APPROVALS, APPROVALS_DEL, APPROVALS_FLEET, APPROVALS_PAIR, AssistedPanel, color, DECISIONS, DECISIONS_DEL, DECISIONS_FLEET, DelegationConsole, diagnostics, done, featuredTools, FEEDBACK, FleetConsole, ManualContextInspector, PairSessionPanel, PLAN, RECENT, RightInspector, tabs

## mockups/src/app/components/TopBar.tsx

mockups/src/app/components/TopBar.tsx has 195 lines; symbols: LEVEL_STATUS, manualToolsHealthy, ResourceChip, status, TopBar; first content: import { ProductModeSwitch } from "./ProductModeSwitch";
Symbols: LEVEL_STATUS, manualToolsHealthy, ResourceChip, status, TopBar

## mockups/src/app/manualModeProjection.ts

mockups/src/app/manualModeProjection.ts has 227 lines; symbols: MANUAL_COMMAND_TARGETS, MANUAL_TOOLCHAIN, MANUAL_TRUST_BOUNDARY, ManualCommandTarget, ManualDiagnostic, ManualProviderKind; first content: export type ManualToolHealth = "running" | "ready" | "idle" | "healthy" | "degraded";
Symbols: MANUAL_COMMAND_TARGETS, MANUAL_TOOLCHAIN, MANUAL_TRUST_BOUNDARY, ManualCommandTarget, ManualDiagnostic, ManualProviderKind, ManualToolHealth, ManualToolState

## mockups/src/imports/design.md

mockups/src/imports/design.md has 1256 lines; headings: Legion IDE Design System, 1. Product Identity, Core Promise; first content: _A starting style guide for a fast, native-feeling, AI-native development environment._
Symbols: none

## mockups/src/main.tsx

mockups/src/main.tsx has 12 lines; first content: import React from "react";
Symbols: none

## mockups/src/styles/fonts.css

mockups/src/styles/fonts.css has 3 lines; first content: @import url("https://fonts.googleapis.com/css2?family=Inter:wght@300;400;500;600;700&display=swap");
Symbols: none

## mockups/src/styles/globals.css

mockups/src/styles/globals.css has 1 lines
Symbols: none

## mockups/src/styles/index.css

mockups/src/styles/index.css has 3 lines; first content: @import './fonts.css';
Symbols: none

## mockups/src/styles/tailwind.css

mockups/src/styles/tailwind.css has 5 lines; first content: @import 'tailwindcss' source(none);
Symbols: none

## mockups/src/styles/theme.css

mockups/src/styles/theme.css has 182 lines; first content: @custom-variant dark (&:is(.dark *));
Symbols: none

## mockups/src/test.txt

mockups/src/test.txt has 2 lines
Symbols: none

## mockups/vite.config.ts

mockups/vite.config.ts has 23 lines; first content: import { defineConfig } from 'vite'
Symbols: none

## plans/adrs/ADR-0001-rust-workspace.md

plans/adrs/ADR-0001-rust-workspace.md has 17 lines; headings: ADR-0001: Adopt Rust 2024 Multi-Crate Workspace and Proprietary Distribution Model, Status, Context; first content: Accepted
Symbols: none

## plans/adrs/ADR-0002-ui-editor-rendering.md

plans/adrs/ADR-0002-ui-editor-rendering.md has 50 lines; headings: ADR-0002: Select Primary UI/Editor Rendering Architecture, Status, Context; first content: Accepted with reservations — Spike 1A validated projection-only shell behavior; renderer-backed p50/p95 input-to-paint, 
Symbols: none

## plans/adrs/ADR-0003-editor-core-text-model.md

plans/adrs/ADR-0003-editor-core-text-model.md has 17 lines; headings: ADR-0003: Define Editor Core Text Buffer, Rope, Transaction, and Snapshot Model, Status, Context; first content: Accepted
Symbols: none

## plans/adrs/ADR-0004-async-runtime-actor-model.md

plans/adrs/ADR-0004-async-runtime-actor-model.md has 17 lines; headings: ADR-0004: Select Async Runtime and Subsystem Actor Model, Status, Context; first content: Accepted
Symbols: none

## plans/adrs/ADR-0005-storage-backends.md

plans/adrs/ADR-0005-storage-backends.md has 20 lines; headings: ADR-0005: Select Local Metadata, Lexical Index, and Vector Store Backends, Status, Context; first content: Accepted with reservations — SQLite/Tantivy metadata baseline accepted; vector-store selection and durable semantic/trac
Symbols: none

## plans/adrs/ADR-0006-ai-provider-abstraction.md

plans/adrs/ADR-0006-ai-provider-abstraction.md has 25 lines; headings: ADR-0006: Define AI Provider Abstraction and BYOK Credential Boundaries, Status, Context; first content: Accepted
Symbols: none

## plans/adrs/ADR-0007-mode-policy-engine.md

plans/adrs/ADR-0007-mode-policy-engine.md has 17 lines; headings: ADR-0007: Define Mode Policy Engine and Action Broker Capability Model, Status, Context; first content: Accepted
Symbols: none

## plans/adrs/ADR-0008-tracker-schema.md

plans/adrs/ADR-0008-tracker-schema.md has 17 lines; headings: ADR-0008: Define Local Tracker Schema and Event Retention Policy, Status, Context; first content: Accepted
Symbols: none

## plans/adrs/ADR-0009-memory-consent.md

plans/adrs/ADR-0009-memory-consent.md has 17 lines; headings: ADR-0009: Define Memory Consent, Storage, Retention, and Retrieval Policy, Status, Context; first content: Accepted
Symbols: none

## plans/adrs/ADR-0010-air-gap-mode.md

plans/adrs/ADR-0010-air-gap-mode.md has 17 lines; headings: ADR-0010: Define Air-Gap Mode and Outbound Network Enforcement Model, Status, Context; first content: Accepted
Symbols: none

## plans/adrs/ADR-0015-streaming-text-viewport.md

plans/adrs/ADR-0015-streaming-text-viewport.md has 118 lines; headings: ADR-0015: Streaming Text, Chunked Snapshots, and Viewport Projection, Status, Context; first content: Accepted for Phase 1 Workstream 0.
Symbols: none

## plans/adrs/ADR-0016-generalized-proposal-service.md

plans/adrs/ADR-0016-generalized-proposal-service.md has 77 lines; headings: ADR-0016: Generalized Proposal Service, Status, Context; first content: Accepted for Phase 2 protocol and app-orchestration workstreams.
Symbols: none

## plans/adrs/ADR-0017-semantic-fabric-indexing.md

plans/adrs/ADR-0017-semantic-fabric-indexing.md has 114 lines; headings: ADR-0017: Semantic Fabric Indexing, Status, Context; first content: Accepted. Phase 3 implementation evidence satisfies this ADR in [`predictive-semantic-fabric.md`](../evidence/phase-3/pr
Symbols: none

## plans/adrs/ADR-0018-lsp-runtime-supervision.md

plans/adrs/ADR-0018-lsp-runtime-supervision.md has 94 lines; headings: ADR-0018: LSP Runtime Supervision, Status, Context; first content: Accepted. Phase 3 implementation evidence satisfies this ADR in [`predictive-semantic-fabric.md`](../evidence/phase-3/pr
Symbols: none

## plans/adrs/ADR-0019-wasm-plugin-runtime.md

plans/adrs/ADR-0019-wasm-plugin-runtime.md has 34 lines; headings: ADR-0019: WASM Plugin Runtime Boundary, Context, Decision; first content: Status: Accepted — with an open supply-chain debt against the Wasmtime clause below
Symbols: none

## plans/adrs/ADR-0020-collaboration-operation-model.md

plans/adrs/ADR-0020-collaboration-operation-model.md has 28 lines; headings: ADR-0020: Collaboration Operation Model, Status, Context; first content: Accepted for Phase 6 local operation-log collaboration runtime.
Symbols: none

## plans/adrs/ADR-0021-collaboration-identity-permissions-retention.md

plans/adrs/ADR-0021-collaboration-identity-permissions-retention.md has 28 lines; headings: ADR-0021: Collaboration Identity, Permissions, and Retention, Status, Context; first content: Accepted for Phase 6 collaboration identity, policy, and metadata-retention boundaries.
Symbols: none

## plans/adrs/ADR-0022-remote-edge-workspace-agent.md

plans/adrs/ADR-0022-remote-edge-workspace-agent.md has 29 lines; headings: ADR-0022: Remote Edge Workspace Agent, Status, Context; first content: Accepted for Phase 7 deterministic edge workspace runtime harness.
Symbols: none

## plans/adrs/ADR-0023-remote-transport-security.md

plans/adrs/ADR-0023-remote-transport-security.md has 29 lines; headings: ADR-0023: Remote Transport And Security Policy, Status, Context; first content: Accepted for Phase 7 deterministic transport, policy, and metadata-only audit validation.
Symbols: none

## plans/adrs/ADR-0024-remote-execution-boundary.md

plans/adrs/ADR-0024-remote-execution-boundary.md has 27 lines; headings: ADR-0024: Remote Execution Boundary, Status, Context; first content: Accepted for Phase 7 bounded remote execution descriptors.
Symbols: none

## plans/adrs/ADR-0025-production-remote-network-transport.md

plans/adrs/ADR-0025-production-remote-network-transport.md has 28 lines; headings: ADR-0025: Production Remote Network Transport, Context, Decision; first content: Status: Accepted for production implementation direction; Phase 8 GA acceptance deferred
Symbols: none

## plans/adrs/ADR-0026-standalone-local-terminal-runtime.md

plans/adrs/ADR-0026-standalone-local-terminal-runtime.md has 25 lines; headings: ADR-0026: Standalone Local Terminal Runtime, Context, Decision; first content: Status: Accepted for production implementation direction; Phase 8 GA acceptance deferred
Symbols: none

## plans/adrs/ADR-0027-hosted-telemetry-and-egress.md

plans/adrs/ADR-0027-hosted-telemetry-and-egress.md has 25 lines; headings: ADR-0027: Hosted Telemetry And Egress, Context, Decision; first content: Status: Accepted for production implementation direction; Phase 8 GA acceptance deferred
Symbols: none

## plans/adrs/ADR-0028-raw-source-retention.md

plans/adrs/ADR-0028-raw-source-retention.md has 25 lines; headings: ADR-0028: Raw-Source Retention, Context, Decision; first content: Status: Accepted for production implementation direction; Phase 8 GA acceptance deferred
Symbols: none

## plans/adrs/ADR-0029-phase-8-operational-hardening.md

plans/adrs/ADR-0029-phase-8-operational-hardening.md has 25 lines; headings: ADR-0029: Phase 8 Operational Hardening, Context, Decision; first content: Status: Accepted for production implementation direction; Phase 8 GA acceptance deferred
Symbols: none

## plans/adrs/ADR-0030-desktop-adapter-boundary.md

plans/adrs/ADR-0030-desktop-adapter-boundary.md has 58 lines; headings: ADR-0030: Desktop Adapter Boundary, Status, Context; first content: Accepted.
Symbols: none

## plans/adrs/ADR-0031-legion-workflow-orchestration.md

plans/adrs/ADR-0031-legion-workflow-orchestration.md has 51 lines; headings: ADR 0031: Legion Workflow Orchestration, Status, Context; first content: Accepted
Symbols: none

## plans/adrs/ADR-0032-editor-render-path.md

plans/adrs/ADR-0032-editor-render-path.md has 41 lines; headings: ADR-0032: Editor Render Path, Status, Context; first content: Accepted — ratified for Production Master Plan v0.1 M0 on 2026-06-10.
Symbols: none

## plans/adrs/ADR-0033-syntax-parse-engine.md

plans/adrs/ADR-0033-syntax-parse-engine.md has 85 lines; headings: ADR-0033: Syntax and Parse Engine, Status, Context; first content: Accepted — ratified for Production Master Plan v0.1 M0 on 2026-06-10.
Symbols: none

## plans/adrs/ADR-0034-lsp-client-architecture.md

plans/adrs/ADR-0034-lsp-client-architecture.md has 193 lines; headings: ADR-0034: LSP Client Architecture, Status, Context; first content: Accepted — ratified for Production Master Plan v0.1 M0 on 2026-06-10.
Symbols: none

## plans/adrs/ADR-0035-terminal-stack.md

plans/adrs/ADR-0035-terminal-stack.md has 264 lines; headings: ADR-0035: Terminal Stack, Status, Context; first content: Accepted — ratified for Production Master Plan v0.1 M0 on 2026-06-10.
Symbols: none

## plans/adrs/ADR-0036-search-and-index-stack.md

plans/adrs/ADR-0036-search-and-index-stack.md has 327 lines; headings: ADR-0036: Search and Index Stack, Status, Context; first content: Accepted — ratified for Production Master Plan v0.1 M0 on 2026-06-10.
Symbols: none

## plans/adrs/ADR-0037-semantic-retrieval.md

plans/adrs/ADR-0037-semantic-retrieval.md has 439 lines; headings: ADR-0037: Semantic Retrieval, Status, Context; first content: Accepted — ratified for Production Master Plan v0.1 M0 on 2026-06-10.
Symbols: none

## plans/adrs/ADR-0038-os-sandbox-layer.md

plans/adrs/ADR-0038-os-sandbox-layer.md has 1896 lines; headings: ADR-0038: OS Sandbox Layer, Status, Context; first content: Accepted — ratified for Production Master Plan v0.1 M0 on 2026-06-10.
Symbols: none

## plans/adrs/ADR-0039-agent-interop.md

plans/adrs/ADR-0039-agent-interop.md has 1907 lines; symbols: at; headings: ADR-0039: Agent Interop, Status, Context; first content: Accepted — ratified for Production Master Plan v0.1 M0 on 2026-06-10.
Symbols: at

## plans/adrs/ADR-0040-concurrent-edit-substrate.md

plans/adrs/ADR-0040-concurrent-edit-substrate.md has 938 lines; symbols: target; headings: ADR-0040: Concurrent Edit Substrate, Status, Context; first content: Accepted — ratified for Production Master Plan v0.1 M0 on 2026-06-10.
Symbols: target

## plans/adrs/ADR-0041-crdt-adoption.md

plans/adrs/ADR-0041-crdt-adoption.md has 64 lines; headings: ADR-0041: CRDT Adoption for the Anchor Layer, Status, Context; first content: Accepted — post-GA collaboration decision for WS16.T1 on 2026-06-13.
Symbols: none

## plans/adrs/ADR-0042-auto-update-strategy-and-signed-manifest.md

plans/adrs/ADR-0042-auto-update-strategy-and-signed-manifest.md has 62 lines; headings: ADR-0042: Auto-Update Strategy and Signed Manifest Format, Status, Context; first content: Accepted — WS17.T3 design decision for the release/rollback surface.
Symbols: none

## plans/adrs/ADR-0043-acp-host-local-adapter-bridge.md

plans/adrs/ADR-0043-acp-host-local-adapter-bridge.md has 32 lines; headings: ADR-0043: ACP Host Scope Is the Local Adapter Bridge, Status, Context; first content: Accepted — WS13.T4 scope clarification.
Symbols: none

## plans/adrs/ADR-0044-dap-client-architecture.md

plans/adrs/ADR-0044-dap-client-architecture.md has 158 lines; headings: ADR-0044: DAP Client Architecture (Real Adapter Path), Status, Context; first content: **Accepted** — WS-A-D Phase 2 B0–B3 (2026-07-21).
Symbols: none

## plans/adrs/ADR-0045-collaboration-operation-layer.md

plans/adrs/ADR-0045-collaboration-operation-layer.md has 39 lines; headings: ADR-0045: Collaboration Operation Layer Sits on the Accepted Collaboration Substrate, Status, Context; first content: Accepted — ratified for P9.F3.T1.
Symbols: none

## plans/architecture-charter-v0.1.md

plans/architecture-charter-v0.1.md has 1016 lines; headings: Legion IDE Architecture Charter v0.1, 0. Executive Position, 1. Product-Level Architectural Principles; first content: Status: Draft for founding engineering review
Symbols: none

## plans/architecture-freeze-v0.1.md

plans/architecture-freeze-v0.1.md has 35 lines; headings: Architecture Freeze: Legion IDE Spike 1A Prerequisites v0.1, Status, Scope; first content: Accepted
Symbols: none

## plans/architecture-review-2026-ide-roadmap-v0.1.md

plans/architecture-review-2026-ide-roadmap-v0.1.md has 438 lines; headings: Legion IDE 2026 Architecture and Functional Review Roadmap v0.1, 1. Executive verdict, 2. Evidence basis and stale-document correction; first content: Status: Strategic review artifact
Symbols: none

## plans/architecture-review-full-codebase-v0.1.md

plans/architecture-review-full-codebase-v0.1.md has 316 lines; headings: Legion IDE - Full Codebase Architectural Review v0.1, Scope and evidence basis, Executive assessment; first content: Status: **REVIEW COMPLETE — REQUIRED REFACTORING IDENTIFIED**
Symbols: none

## plans/architecture-review-phases-5-6-v0.1.md

plans/architecture-review-phases-5-6-v0.1.md has 137 lines; headings: Legion IDE - Architecture Review for Phases 5-6 v0.1, Review scope, Executive outcome; first content: Status: **HOLD FOR REQUIRED CHANGES**
Symbols: none

## plans/architecture-review-v0.1.md

plans/architecture-review-v0.1.md has 80 lines; headings: Legion IDE - Founding Architecture Review v0.1, Outcome, Required user-specified corrective action implemented; first content: Status: **PASS WITH CHANGES**
Symbols: none

## plans/control-first-adaptive-ide-granular-implementation-plan-v0.1.md

plans/control-first-adaptive-ide-granular-implementation-plan-v0.1.md has 634 lines; headings: Control-First Adaptive IDE Granular Implementation Plan v0.1, 1. Planning thesis, 2. Critical path and blockers; first content: Status: Execution planning artifact
Symbols: none

## plans/control-first-adaptive-ide-technical-design-v0.1.md

plans/control-first-adaptive-ide-technical-design-v0.1.md has 617 lines; headings: Control-First Adaptive IDE Technical Design v0.1, 1. Executive conclusion, 2. Research findings compared with current system reality; first content: Status: Draft for architecture review
Symbols: none

## plans/dependency-policy.md

plans/dependency-policy.md has 844 lines; headings: Dependency Policy for Legion IDE v0.1, Scope, Rules; first content: This document defines the required internal crate dependency direction and runtime-surface activation gates used by `car
Symbols: none

## plans/desktop-adapter-boundary-v0.1.md

plans/desktop-adapter-boundary-v0.1.md has 90 lines; headings: Desktop Adapter Boundary v0.1, Scope, Startup Flow; first content: `legion-desktop` is the planned renderer-backed desktop adapter for Phase 2. It renders current app/UI projections in a 
Symbols: none

## plans/dogfood/legion-on-legion-weekly-journal-template.md

plans/dogfood/legion-on-legion-weekly-journal-template.md has 62 lines; headings: Legion-on-Legion Weekly Dogfood Journal, Instructions, Template; first content: Use this template for weekly dogfood runs where Legion is used to develop itself.
Symbols: none

## plans/evidence/accessibility/gp-1-manual-walkthrough.md

plans/evidence/accessibility/gp-1-manual-walkthrough.md has 36 lines; headings: GP-1 Manual Screen-Reader Walkthrough, Status, Transcript; first content: - Walkthrough transcript: captured.
Symbols: none

## plans/evidence/accessibility/gp-2-assist-walkthrough.md

plans/evidence/accessibility/gp-2-assist-walkthrough.md has 43 lines; headings: GP-2 Assist Screen-Reader Walkthrough, Status, Transcript; first content: - Walkthrough transcript: captured.
Symbols: none

## plans/evidence/accessibility/gp-3-delegate-walkthrough.md

plans/evidence/accessibility/gp-3-delegate-walkthrough.md has 85 lines; headings: GP-3 Delegate Screen-Reader Walkthrough, Status, Transcript; first content: **Updated:** 2026-07-07 (M10 PKT-GP3 — reflects M10 delegate surface)
Symbols: none

## plans/evidence/accessibility/README.md

plans/evidence/accessibility/README.md has 29 lines; headings: AccessKit Product Pass and GP Screen-Reader Walkthroughs, Status, Purpose; first content: - Product-level accessibility evidence: passed.
Symbols: none

## plans/evidence/dogfood/2026-07-21-dogfood-journal.md

plans/evidence/dogfood/2026-07-21-dogfood-journal.md has 73 lines; headings: Dogfood Journal — 2026-07-21, Session, Workflow Attempted; first content: - **Branch:** main
Symbols: none

## plans/evidence/dogfood/2026-07-21-phase1-floor-journal.md

plans/evidence/dogfood/2026-07-21-phase1-floor-journal.md has 71 lines; headings: Dogfood Journal — 2026-07-21 (WS-A-D Phase 1 floor verification), Session, Workflow Attempted; first content: - **Branch:** `phase-1/dogfood-session-2026-07-21` (on top of `docs/ws-a-d-campaign-charter` / main tip including #66)
Symbols: none

## plans/evidence/dogfood/2026-07-22-dap-b10-headless-journal.md

plans/evidence/dogfood/2026-07-22-dap-b10-headless-journal.md has 55 lines; headings: Dogfood Journal — 2026-07-22 (DAP B10 headless continue auto-poll), Session, Workflow Attempted; first content: - **Branch:** `main` (post `#84` B10)
Symbols: none

## plans/evidence/dogfood/2026-07-22-preview-artifact-journal.md

plans/evidence/dogfood/2026-07-22-preview-artifact-journal.md has 54 lines; headings: Dogfood Journal — 2026-07-22 (preview artifact / CI), Session, Workflow Attempted; first content: - **Branch:** main
Symbols: none

## plans/evidence/dogfood/INSTALLED-PREVIEW-CHECKLIST.md

plans/evidence/dogfood/INSTALLED-PREVIEW-CHECKLIST.md has 46 lines; headings: Installed preview dogfood checklist (Phase 5 residual), Package (local), Windows; first content: Use after packaging a **local** unsigned-beta preview artifact. This is not a
Symbols: none

## plans/evidence/dogfood/INTERACTIVE-GUI-CHECKLIST.md

plans/evidence/dogfood/INTERACTIVE-GUI-CHECKLIST.md has 74 lines; headings: Interactive GUI dogfood checklist (Phase 1 + Phase 2 DAP), Setup, fake adapter (CI-grade contract, no system LLDB required); first content: Use this for a **human-driven eframe** session. Automated substitutes exist
Symbols: none

## plans/evidence/dogfood/README.md

plans/evidence/dogfood/README.md has 39 lines; headings: Dogfood journal evidence, Journals (index), Naming; first content: Weekly Legion-on-Legion dogfood journals live here.
Symbols: none

## plans/evidence/gui-productization/gui-headless-input-evidence.md

plans/evidence/gui-productization/gui-headless-input-evidence.md has 89 lines; headings: GUI Headless Input Evidence, Purpose, Verification; first content: Date: 2026-06-14T09:10:56Z
Symbols: none

## plans/evidence/gui-productization/gui-productization-baseline.md

plans/evidence/gui-productization/gui-productization-baseline.md has 67 lines; headings: GUI Productization Baseline, Sources Read, Current Product Shape; first content: Date: 2026-05-26
Symbols: none

## plans/evidence/gui-productization/phase-1-renderer-readiness.md

plans/evidence/gui-productization/phase-1-renderer-readiness.md has 58 lines; headings: Phase 1 Renderer Readiness, Phase 1 readiness: Accepted, Artifact Inventory; first content: Date: 2026-05-26
Symbols: none

## plans/evidence/gui-productization/phase-13-final-gates.md

plans/evidence/gui-productization/phase-13-final-gates.md has 81 lines; headings: Phase 13 Final Gates, Status, Required Commands; first content: - Phase 13 final gates: passed for the current local checkout on 2026-05-28.
Symbols: none

## plans/evidence/gui-productization/phase-13-governance.md

plans/evidence/gui-productization/phase-13-governance.md has 29 lines; headings: Phase 13 Governance Evidence, Scope, Accepted Runtime Boundary; first content: Defines the accepted activation boundary for Phase 13 Legion Workflow Orchestration.
Symbols: none

## plans/evidence/gui-productization/phase-13-legion-workflow-orchestration.md

plans/evidence/gui-productization/phase-13-legion-workflow-orchestration.md has 164 lines; headings: Phase 13 Legion Workflow Orchestration Evidence, Acceptance Status, Scope; first content: - Phase 13 acceptance: Accepted
Symbols: none

## plans/evidence/gui-productization/phase-13-runbook.md

plans/evidence/gui-productization/phase-13-runbook.md has 137 lines; headings: Phase 13 Legion Workflow Runbook, Purpose, Operating Markers; first content: This runbook describes how to operate, review, and recover the Phase 13 Legion Workflow orchestration surface.
Symbols: none

## plans/evidence/gui-productization/phase-2-renderer-foundation.md

plans/evidence/gui-productization/phase-2-renderer-foundation.md has 113 lines; headings: Phase 2 Renderer Foundation, Phase 2 renderer foundation: Accepted, Artifact Inventory; first content: Decision date: 2026-05-26
Symbols: none

## plans/evidence/gui-productization/phase-2-renderer-smoke.md

plans/evidence/gui-productization/phase-2-renderer-smoke.md has 40 lines; headings: Phase 2 Renderer Smoke Evidence, Status, Command; first content: status: passed
Symbols: none

## plans/evidence/gui-productization/phase-3-daily-editing-mvp.md

plans/evidence/gui-productization/phase-3-daily-editing-mvp.md has 111 lines; headings: Phase 3 Daily Editing MVP, Acceptance Status, Artifact Inventory; first content: Phase 3 daily editing MVP: Accepted
Symbols: none

## plans/evidence/gui-productization/phase-3-session-and-large-file.md

plans/evidence/gui-productization/phase-3-session-and-large-file.md has 49 lines; headings: Phase 3 Session Restore And Large-File Guardrails Evidence, Scope, Commands; first content: Plan 03-05 adds desktop metadata-only session persistence/restore and verifies that large-file GUI rendering/search rema
Symbols: none

## plans/evidence/gui-productization/phase-4-language-terminal-ide-loop.md

plans/evidence/gui-productization/phase-4-language-terminal-ide-loop.md has 51 lines; headings: Phase 4 Language And Terminal IDE Loop, Delivered, Acceptance Mapping; first content: Acceptance status: Accepted
Symbols: none

## plans/evidence/gui-productization/phase-4-language-terminal-safety.md

plans/evidence/gui-productization/phase-4-language-terminal-safety.md has 72 lines; headings: Phase 4 Language And Terminal Safety Evidence, Scope, Boundary Proof; first content: Acceptance status: Accepted
Symbols: none

## plans/evidence/gui-productization/phase-5-control-trust-assisted-ai.md

plans/evidence/gui-productization/phase-5-control-trust-assisted-ai.md has 85 lines; headings: GUI Phase 5 Control, Trust, And Assisted AI Evidence, Acceptance Status, Scope; first content: - Phase 5 acceptance: Accepted.
Symbols: none

## plans/evidence/gui-productization/phase-5-control-trust-safety.md

plans/evidence/gui-productization/phase-5-control-trust-safety.md has 86 lines; headings: GUI Phase 5 Control Trust Safety Evidence, Scope, Proposal Lifecycle Visibility; first content: Status: Safety evidence complete; not final acceptance.
Symbols: none

## plans/evidence/gui-productization/phase-6-ci-parity-plan.md

plans/evidence/gui-productization/phase-6-ci-parity-plan.md has 25 lines; headings: GUI Phase 6 CI parity plan, Scope, CI Checks; first content: CI now carries the same non-interactive GUI Phase 6 checks that can run reliably without a visible desktop session.
Symbols: none

## plans/evidence/gui-productization/phase-6-input-conformance.md

plans/evidence/gui-productization/phase-6-input-conformance.md has 56 lines; headings: GUI Phase 6 input conformance evidence, Status, Commands; first content: status: passed
Symbols: none

## plans/evidence/gui-productization/phase-6-package-runbook.md

plans/evidence/gui-productization/phase-6-package-runbook.md has 48 lines; headings: GUI Phase 6 package runbook, Scope, Dry Run; first content: This runbook covers the deterministic Windows desktop packaging path for `legion-desktop`. It packages the existing exec
Symbols: none

## plans/evidence/gui-productization/phase-6-packaging-platform-accessibility.md

plans/evidence/gui-productization/phase-6-packaging-platform-accessibility.md has 66 lines; headings: GUI Phase 6 packaging, platform, and accessibility evidence, Acceptance Status, Scope; first content: - Phase 6 acceptance: Accepted.
Symbols: none

## plans/evidence/gui-productization/phase-6-packaging-smoke.md

plans/evidence/gui-productization/phase-6-packaging-smoke.md has 26 lines; headings: GUI Phase 6 packaging smoke evidence, Status, Commands; first content: status: passed
Symbols: none

## plans/evidence/gui-productization/phase-6-performance-reliability.md

plans/evidence/gui-productization/phase-6-performance-reliability.md has 36 lines; headings: GUI Phase 6 performance and reliability evidence, Status, Commands; first content: status: passed
Symbols: none

## plans/evidence/gui-productization/phase-6-platform-accessibility-smoke.md

plans/evidence/gui-productization/phase-6-platform-accessibility-smoke.md has 58 lines; headings: Renderer Smoke Evidence, Status, Command; first content: status: passed
Symbols: none

## plans/evidence/gui-productization/phase-6-session-diagnostics-safety.md

plans/evidence/gui-productization/phase-6-session-diagnostics-safety.md has 32 lines; headings: GUI Phase 6 session and diagnostics safety evidence, Status, Commands; first content: status: passed
Symbols: none

## plans/evidence/gui-productization/phase-6-workflow-smoke.md

plans/evidence/gui-productization/phase-6-workflow-smoke.md has 24 lines; headings: GUI Phase 6 workflow smoke evidence, Status, Commands; first content: status: passed with one local shell limitation
Symbols: none

## plans/evidence/gui-productization/phase-7-known-limitations.md

plans/evidence/gui-productization/phase-7-known-limitations.md has 35 lines; headings: GUI Phase 7 Known Limitations, Scope, Limitation Inventory; first content: GUI Phase 7 is a local IDE beta. It does not replace the accepted legacy remote-development Phase 7 evidence under `plan
Symbols: none

## plans/evidence/gui-productization/phase-7-launch-runbook.md

plans/evidence/gui-productization/phase-7-launch-runbook.md has 86 lines; headings: GUI Phase 7 Local IDE Beta Launch Runbook, Scope, Launch Commands; first content: This runbook covers the GUI Phase 7 local IDE beta. It is for local Rust repository workflows only: open, browse, edit/s
Symbols: none

## plans/evidence/gui-productization/phase-7-local-ide-beta.md

plans/evidence/gui-productization/phase-7-local-ide-beta.md has 69 lines; headings: GUI Phase 7 local IDE beta evidence, Acceptance Status, Scope; first content: - Phase 7 acceptance: Accepted.
Symbols: none

## plans/evidence/gui-productization/phase-7-local-workflow-smoke.md

plans/evidence/gui-productization/phase-7-local-workflow-smoke.md has 46 lines; headings: GUI Phase 7 Local Workflow Smoke, Status, Command; first content: status: passed
Symbols: none

## plans/evidence/gui-productization/phase-7-manual-beta-evidence.md

plans/evidence/gui-productization/phase-7-manual-beta-evidence.md has 25 lines; headings: Manual beta evidence, Scope, Captured Run Notes; first content: This file records real-repository beta launch notes separately from the automated fixture smoke. Automated write smoke u
Symbols: none

## plans/evidence/gui-productization/phase-7-operational-health-diagnostics.md

plans/evidence/gui-productization/phase-7-operational-health-diagnostics.md has 54 lines; headings: GUI Phase 7 Operational Health And Diagnostics Evidence, Status, Diagnostics Export; first content: status: passed
Symbols: none

## plans/evidence/gui-productization/phase-7-release-readiness.md

plans/evidence/gui-productization/phase-7-release-readiness.md has 38 lines; headings: GUI Phase 7 Release Readiness, Status, Readiness Checklist; first content: status: accepted; Plan 07-05 ran final gates and the main Phase 7 evidence now says `Phase 7 acceptance: Accepted.`
Symbols: none

## plans/evidence/gui-productization/phase-8-advanced-platform-gui-ga.md

plans/evidence/gui-productization/phase-8-advanced-platform-gui-ga.md has 83 lines; headings: GUI Phase 8 advanced platform GUI GA evidence, Acceptance Status, Scope; first content: - Phase 8 acceptance: Accepted.
Symbols: none

## plans/evidence/gui-productization/phase-8-advanced-surface-smoke.md

plans/evidence/gui-productization/phase-8-advanced-surface-smoke.md has 55 lines; headings: GUI Phase 8 advanced surface smoke evidence, Status, Smoke Coverage; first content: - Advanced surface smoke: scripted markers added.
Symbols: none

## plans/evidence/gui-productization/phase-8-collaboration-gui.md

plans/evidence/gui-productization/phase-8-collaboration-gui.md has 53 lines; headings: GUI Phase 8 collaboration GUI evidence, Status, Scope; first content: - Collaboration GUI: supported.
Symbols: none

## plans/evidence/gui-productization/phase-8-delegated-task-command-center.md

plans/evidence/gui-productization/phase-8-delegated-task-command-center.md has 57 lines; headings: GUI Phase 8 delegated task command-center evidence, Status, Scope; first content: - Delegated task command center: approval-gated.
Symbols: none

## plans/evidence/gui-productization/phase-8-final-gates.md

plans/evidence/gui-productization/phase-8-final-gates.md has 44 lines; headings: GUI Phase 8 final gates, Status, Required Commands; first content: - Phase 8 final gates: passed for the current local checkout on 2026-05-28.
Symbols: none

## plans/evidence/gui-productization/phase-8-ga-release-runbook.md

plans/evidence/gui-productization/phase-8-ga-release-runbook.md has 98 lines; headings: GUI Phase 8 GA release runbook, Status, Source Evidence; first content: - GA release readiness: not accepted.
Symbols: none

## plans/evidence/gui-productization/phase-8-platform-parity.md

plans/evidence/gui-productization/phase-8-platform-parity.md has 70 lines; headings: GUI Phase 8 platform parity evidence, Status, Windows Evidence; first content: - Platform parity: Windows - evidenced locally on 2026-05-27 and by GitHub Actions run `26590800830` on 2026-05-28.
Symbols: none

## plans/evidence/gui-productization/phase-8-plugin-management.md

plans/evidence/gui-productization/phase-8-plugin-management.md has 53 lines; headings: GUI Phase 8 plugin management evidence, Status, Scope; first content: - Plugin management GUI: supported.
Symbols: none

## plans/evidence/gui-productization/phase-8-remote-workspace-gui.md

plans/evidence/gui-productization/phase-8-remote-workspace-gui.md has 56 lines; headings: GUI Phase 8 remote workspace GUI evidence, Status, Scope; first content: - Remote workspace GUI: supported.
Symbols: none

## plans/evidence/gui-productization/phase-8-update-rollback-incident.md

plans/evidence/gui-productization/phase-8-update-rollback-incident.md has 57 lines; headings: GUI Phase 8 update, rollback, and incident drill evidence, Status, Drill Scope; first content: - Update drill: documented from script and CI marker checks.
Symbols: none

## plans/evidence/gui-productization/renderer-decision-matrix.md

plans/evidence/gui-productization/renderer-decision-matrix.md has 80 lines; headings: Renderer Decision Matrix, Source Set, Required Criteria; first content: Date: 2026-05-26
Symbols: none

## plans/evidence/legion-e2e/2026-06-03_beta_acceptance_e2e.txt

plans/evidence/legion-e2e/2026-06-03_beta_acceptance_e2e.txt has 18 lines; first content: COMMAND: cargo test -p legion-desktop --test beta_acceptance_e2e -- --nocapture
Symbols: none

## plans/evidence/legion-e2e/2026-06-03_cargo_deny_local.txt

plans/evidence/legion-e2e/2026-06-03_cargo_deny_local.txt has 7 lines; first content: error: no such command: `deny`
Symbols: none

## plans/evidence/legion-e2e/2026-06-03_cloud_lane_http_transport_gates.txt

plans/evidence/legion-e2e/2026-06-03_cloud_lane_http_transport_gates.txt has 91 lines; first content: COMMAND: cargo test -p legion-remote --test cloud_lane_http_transport -- --nocapture
Symbols: none

## plans/evidence/legion-e2e/2026-06-03_cloud_transport_contract.md

plans/evidence/legion-e2e/2026-06-03_cloud_transport_contract.md has 76 lines; headings: Cloud Lane HTTP JSON Transport Contract — 2026-06-03, Scope, Configuration; first content: This document captures the exact DTOs, endpoint paths, headers, and policy checks implemented for the production HTTP JS
Symbols: none

## plans/evidence/legion-e2e/2026-06-03_cloud_transport_post_clippy_fix.txt

plans/evidence/legion-e2e/2026-06-03_cloud_transport_post_clippy_fix.txt has 22 lines; first content: COMMAND: cargo test -p legion-remote --test cloud_lane_http_transport -- --nocapture
Symbols: none

## plans/evidence/legion-e2e/2026-06-03_final_clippy_rerun.txt

plans/evidence/legion-e2e/2026-06-03_final_clippy_rerun.txt has 29 lines; first content: COMMAND: cargo fmt --all --check
Symbols: none

## plans/evidence/legion-e2e/2026-06-03_final_gates.txt

plans/evidence/legion-e2e/2026-06-03_final_gates.txt has 1510 lines; first content: COMMAND: cargo fmt --all --check
Symbols: none

## plans/evidence/legion-e2e/2026-06-03_python_model_fixture_gates.txt

plans/evidence/legion-e2e/2026-06-03_python_model_fixture_gates.txt has 214 lines; first content: COMMAND: Phase 8 Python/model dry-run and fixture-smoke gates
Symbols: none

## plans/evidence/legion-e2e/20260602T004113_workspace_gates.txt

plans/evidence/legion-e2e/20260602T004113_workspace_gates.txt has 1351 lines; headings: Legion E2E gate evidence 20260602T004113, cargo fmt --all --check, cargo run -p xtask -- check-deps; first content: Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.06s
Symbols: none

## plans/evidence/legion-e2e/20260602T091320_final_gates.txt

plans/evidence/legion-e2e/20260602T091320_final_gates.txt has 5081 lines; headings: Legion E2E final gate evidence 20260602T091320, cargo run -p xtask -- check-deps, cargo fmt --all --check; first content: Working directory: <workspace>
Symbols: none

## plans/evidence/legion-e2e/20260602T182617_rebaseline_product_surface_gates.txt

plans/evidence/legion-e2e/20260602T182617_rebaseline_product_surface_gates.txt has 5131 lines; headings: Legion E2E rebaseline/product-surface gate evidence 20260602T182617, git status --short, git log --oneline -3; first content: Scope: latest origin/main rebaseline, PR #10/#11/#12 presence checks, Windows git CRLF regression fix, Legion visible br
Symbols: none

## plans/evidence/legion-e2e/20260602T184509_dock_mode_shell_gates.txt

plans/evidence/legion-e2e/20260602T184509_dock_mode_shell_gates.txt has 5137 lines; headings: Legion E2E dock/mode-shell gate evidence 20260602T184509, git status --short, git log --oneline -3; first content: Scope: typed PanelCapability contract, shared runtime-surface command visibility, migration from right-console naming to
Symbols: none

## plans/evidence/legion-e2e/20260602T190023_assist_llama_cpp_provider_gates.txt

plans/evidence/legion-e2e/20260602T190023_assist_llama_cpp_provider_gates.txt has 126 lines; headings: Legion E2E Evidence - Assist llama.cpp provider route, Scope, Source References; first content: Timestamp: 2026-06-02T19:00:23-04:00
Symbols: none

## plans/evidence/legion-e2e/20260602T191139_editor_completion_gates.txt

plans/evidence/legion-e2e/20260602T191139_editor_completion_gates.txt has 150 lines; headings: Legion E2E Evidence - Manual editor-port completion, Scope, Source References; first content: Timestamp: 2026-06-02T19:11:39-04:00
Symbols: none

## plans/evidence/legion-e2e/20260602T204859_phase8_trace_model_flywheel_gates.txt

plans/evidence/legion-e2e/20260602T204859_phase8_trace_model_flywheel_gates.txt has 210 lines; headings: Legion E2E Evidence - Phase 8 trace/model-flywheel dry-run gates, Scope, Focused Rust Verification; first content: Timestamp: 2026-06-02T20:48:59-04:00
Symbols: none

## plans/evidence/legion-e2e/README.md

plans/evidence/legion-e2e/README.md has 23 lines; headings: Legion E2E Evidence Directory; first content: This directory stores raw command outputs for the consolidated Legion E2E implementation plan.
Symbols: none

## plans/evidence/mutation-route-inventory.md

plans/evidence/mutation-route-inventory.md has 43 lines; headings: Mutation Route Inventory, Inventory, M9 PKT-APPLY activation state (2026-07-05); first content: Date: 2026-07-05 (updated PKT-APPLY)
Symbols: none

## plans/evidence/p0-governance-mutation-path-audit.md

plans/evidence/p0-governance-mutation-path-audit.md has 41 lines; headings: P0 Governance Mutation-Path Audit, Control-first invariants audited, Permitted mutation paths; first content: Status: P0.1/P0.2 implementation evidence
Symbols: none

## plans/evidence/p2-1-viewport-non-regression-harness.md

plans/evidence/p2-1-viewport-non-regression-harness.md has 35 lines; headings: P2.1 viewport non-regression harness evidence, Coverage added or strengthened, Validation run; first content: Date: 2026-05-21
Symbols: none

## plans/evidence/p2-2-snapshot-lease-consumer-contract.md

plans/evidence/p2-2-snapshot-lease-consumer-contract.md has 41 lines; headings: P2.2 Snapshot Lease Consumer Contract Evidence, Scope, Contract coverage; first content: This note records the P2.2 integration slice for snapshot lease consumer contracts from the granular control-first imple
Symbols: none

## plans/evidence/perf-harness-fixtures/100k-file-search.toml

plans/evidence/perf-harness-fixtures/100k-file-search.toml has 11 lines; first content: name = "m1.fixture_search_100k"
Symbols: none

## plans/evidence/perf-harness-fixtures/50k-file-search.toml

plans/evidence/perf-harness-fixtures/50k-file-search.toml has 11 lines; first content: name = "m1.fixture_search_50k"
Symbols: none

## plans/evidence/perf-harness-fixtures/README.md

plans/evidence/perf-harness-fixtures/README.md has 9 lines; headings: Perf harness fixture benchmarks; first content: These manifests back the `xtask perf-harness` large-fixture search benchmarks.
Symbols: none

## plans/evidence/perf-harness-trend/README.md

plans/evidence/perf-harness-trend/README.md has 7 lines; headings: Perf harness trend archive; first content: This directory stores archived `xtask perf-harness` reports, grouped by host OS.
Symbols: none

## plans/evidence/phase-0/cargo-check-workspace-all-targets.txt

plans/evidence/phase-0/cargo-check-workspace-all-targets.txt has 2 lines; first content: Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.09s
Symbols: none

## plans/evidence/phase-0/cargo-clippy-workspace-all-targets.txt

plans/evidence/phase-0/cargo-clippy-workspace-all-targets.txt has 2 lines; first content: Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.13s
Symbols: none

## plans/evidence/phase-0/cargo-test-workspace-all-targets.txt

plans/evidence/phase-0/cargo-test-workspace-all-targets.txt has 254 lines; first content: Finished `test` profile [unoptimized + debuginfo] target(s) in 0.08s
Symbols: none

## plans/evidence/phase-0/check-deps.txt

plans/evidence/phase-0/check-deps.txt has 4 lines; first content: Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.12s
Symbols: none

## plans/evidence/phase-0/editor-performance-suite.txt

plans/evidence/phase-0/editor-performance-suite.txt has 47 lines; first content: Phase 6 editor performance-suite evidence / reservation
Symbols: none

## plans/evidence/phase-0/fmt-check.txt

plans/evidence/phase-0/fmt-check.txt has 1 lines
Symbols: none

## plans/evidence/phase-0/native-shell-proof-summary.md

plans/evidence/phase-0/native-shell-proof-summary.md has 40 lines; headings: Phase 0 Native Shell Proof Summary, Evidence basis, Native shell and editor-path measurements; first content: Status: Accepted with reservations
Symbols: none

## plans/evidence/phase-0/platform-boundary-api-map.md

plans/evidence/phase-0/platform-boundary-api-map.md has 53 lines; headings: Phase 0 Platform Boundary API Map, Ownership rule, Public API ownership map; first content: Status: Accepted
Symbols: none

## plans/evidence/phase-0/text-index-stress-baseline.md

plans/evidence/phase-0/text-index-stress-baseline.md has 46 lines; headings: Phase 0 Text and Index Stress Baseline, Evidence artifacts, Non-ignored performance baseline; first content: Status: Accepted with reservations
Symbols: none

## plans/evidence/phase-1/editor-text-substrate.md

plans/evidence/phase-1/editor-text-substrate.md has 117 lines; headings: Phase 1 Editor Text Substrate Evidence, Scope, Degraded-mode threshold and bounded viewport workload; first content: Date: 2026-05-15
Symbols: none

## plans/evidence/phase-2/proposal-mutation-substrate.md

plans/evidence/phase-2/proposal-mutation-substrate.md has 255 lines; headings: Phase 2 Proposal Mutation Substrate Evidence, Scope, Acceptance status; first content: Date: 2026-05-24
Symbols: none

## plans/evidence/phase-3/cargo-check-workspace-all-targets.txt

plans/evidence/phase-3/cargo-check-workspace-all-targets.txt has 7 lines; first content: Command: cargo check --workspace --all-targets
Symbols: none

## plans/evidence/phase-3/cargo-clippy-workspace-all-targets.txt

plans/evidence/phase-3/cargo-clippy-workspace-all-targets.txt has 7 lines; first content: Command: cargo clippy --workspace --all-targets -- -D warnings
Symbols: none

## plans/evidence/phase-3/cargo-fmt-check.txt

plans/evidence/phase-3/cargo-fmt-check.txt has 6 lines; first content: Command: cargo fmt --all --check
Symbols: none

## plans/evidence/phase-3/cargo-test-workspace-all-targets.txt

plans/evidence/phase-3/cargo-test-workspace-all-targets.txt has 9 lines; first content: Command: cargo test --workspace --all-targets
Symbols: none

## plans/evidence/phase-3/check-deps.txt

plans/evidence/phase-3/check-deps.txt has 9 lines; first content: Command: cargo run -p xtask -- check-deps
Symbols: none

## plans/evidence/phase-3/editor-semantic-latency.txt

plans/evidence/phase-3/editor-semantic-latency.txt has 18 lines; first content: Command: cargo test -p devil-editor --test performance_suite -- --list
Symbols: none

## plans/evidence/phase-3/index-dependency-boundary.txt

plans/evidence/phase-3/index-dependency-boundary.txt has 15 lines; first content: Command: cargo run -p xtask -- check-deps
Symbols: none

## plans/evidence/phase-3/legion-index-tests.txt

plans/evidence/phase-3/legion-index-tests.txt has 17 lines; first content: Command: cargo test -p devil-index --all-targets
Symbols: none

## plans/evidence/phase-3/legion-protocol-dto-contracts.txt

plans/evidence/phase-3/legion-protocol-dto-contracts.txt has 14 lines; first content: Command: cargo test -p devil-protocol --test dto_contracts
Symbols: none

## plans/evidence/phase-3/lexical-symbol-map-tests.txt

plans/evidence/phase-3/lexical-symbol-map-tests.txt has 19 lines; first content: Command: cargo test -p devil-index --all-targets
Symbols: none

## plans/evidence/phase-3/lsp-supervision-tests.txt

plans/evidence/phase-3/lsp-supervision-tests.txt has 27 lines; first content: Commands:
Symbols: none

## plans/evidence/phase-3/normalized-graph-contract-tests.txt

plans/evidence/phase-3/normalized-graph-contract-tests.txt has 25 lines; first content: Command: cargo test -p devil-index --all-targets
Symbols: none

## plans/evidence/phase-3/predictive-semantic-fabric.md

plans/evidence/phase-3/predictive-semantic-fabric.md has 121 lines; headings: Phase 3 Predictive Semantic Fabric Evidence, Scope, Acceptance status; first content: Date: 2026-05-24
Symbols: none

## plans/evidence/phase-3/privacy-redaction-audit.md

plans/evidence/phase-3/privacy-redaction-audit.md has 20 lines; headings: Privacy Redaction Audit; first content: Date: 2026-05-24
Symbols: none

## plans/evidence/phase-3/proposal-routing-regression.txt

plans/evidence/phase-3/proposal-routing-regression.txt has 25 lines; first content: Commands:
Symbols: none

## plans/evidence/phase-3/repository-discovery-ignore-fingerprint.md

plans/evidence/phase-3/repository-discovery-ignore-fingerprint.md has 14 lines; headings: Repository Discovery, Ignore, And Fingerprint Evidence; first content: Date: 2026-05-24
Symbols: none

## plans/evidence/phase-3/save-conflict-regression.txt

plans/evidence/phase-3/save-conflict-regression.txt has 12 lines; first content: Command: cargo test -p devil-app --test workspace_vfs_integration workspace_vfs_integration_external_overwrite_between_o
Symbols: none

## plans/evidence/phase-3/semantic-fabric-architecture-map.md

plans/evidence/phase-3/semantic-fabric-architecture-map.md has 40 lines; headings: Semantic Fabric Architecture Map; first content: Date: 2026-05-24
Symbols: none

## plans/evidence/phase-3/semantic-query-api-tests.txt

plans/evidence/phase-3/semantic-query-api-tests.txt has 27 lines; first content: Command: cargo test -p devil-index --all-targets
Symbols: none

## plans/evidence/phase-3/tree-sitter-cache-tests.txt

plans/evidence/phase-3/tree-sitter-cache-tests.txt has 17 lines; first content: Command: cargo test -p devil-index --all-targets
Symbols: none

## plans/evidence/phase-3/vector-deferral-audit.md

plans/evidence/phase-3/vector-deferral-audit.md has 13 lines; headings: Vector Deferral Audit; first content: Date: 2026-05-24
Symbols: none

## plans/evidence/phase-4/agent-state-machine-tests.txt

plans/evidence/phase-4/agent-state-machine-tests.txt has 16 lines; first content: Command: cargo test -p devil-agent --all-targets
Symbols: none

## plans/evidence/phase-4/agentic-ai-architecture-map.md

plans/evidence/phase-4/agentic-ai-architecture-map.md has 133 lines; headings: Phase 4 Native Agentic AI Execution Context Evidence, Scope, Acceptance status; first content: Date: 2026-05-25
Symbols: none

## plans/evidence/phase-4/air-gap-provider-egress-tests.txt

plans/evidence/phase-4/air-gap-provider-egress-tests.txt has 38 lines; first content: Command: cargo test -p devil-security --all-targets
Symbols: none

## plans/evidence/phase-4/cargo-check-workspace-all-targets.txt

plans/evidence/phase-4/cargo-check-workspace-all-targets.txt has 7 lines; first content: Command: cargo check --workspace --all-targets
Symbols: none

## plans/evidence/phase-4/cargo-clippy-workspace-all-targets.txt

plans/evidence/phase-4/cargo-clippy-workspace-all-targets.txt has 7 lines; first content: Command: cargo clippy --workspace --all-targets -- -D warnings
Symbols: none

## plans/evidence/phase-4/cargo-fmt-check.txt

plans/evidence/phase-4/cargo-fmt-check.txt has 3 lines; first content: Command: cargo fmt --all --check
Symbols: none

## plans/evidence/phase-4/cargo-test-workspace-all-targets.txt

plans/evidence/phase-4/cargo-test-workspace-all-targets.txt has 528 lines; first content: Command: cargo test --workspace --all-targets
Symbols: none

## plans/evidence/phase-4/cloud-provider-deferral-audit.md

plans/evidence/phase-4/cloud-provider-deferral-audit.md has 30 lines; headings: Phase 4 Cloud Provider Deferral Audit, Scope, Evidence; first content: Date: 2026-05-25
Symbols: none

## plans/evidence/phase-4/dependency-boundary.txt

plans/evidence/phase-4/dependency-boundary.txt has 6 lines; first content: Command: cargo run -p xtask -- check-deps
Symbols: none

## plans/evidence/phase-4/local-provider-adapter-tests.txt

plans/evidence/phase-4/local-provider-adapter-tests.txt has 14 lines; first content: Command: cargo test -p devil-ai-providers --all-targets
Symbols: none

## plans/evidence/phase-4/memory-retention-consent-tests.txt

plans/evidence/phase-4/memory-retention-consent-tests.txt has 14 lines; first content: Command: cargo test -p devil-memory --all-targets
Symbols: none

## plans/evidence/phase-4/observability-redaction-audit.md

plans/evidence/phase-4/observability-redaction-audit.md has 32 lines; headings: Phase 4 Observability Redaction Audit, Scope, Evidence; first content: Date: 2026-05-25
Symbols: none

## plans/evidence/phase-4/privacy-inspector-context-manifest-tests.txt

plans/evidence/phase-4/privacy-inspector-context-manifest-tests.txt has 35 lines; first content: Phase 4 privacy inspector and context manifest evidence
Symbols: none

## plans/evidence/phase-4/proposal-routing-regression.txt

plans/evidence/phase-4/proposal-routing-regression.txt has 26 lines; first content: Phase 4 proposal routing regression evidence
Symbols: none

## plans/evidence/phase-4/provider-router-contract-tests.txt

plans/evidence/phase-4/provider-router-contract-tests.txt has 16 lines; first content: Command: cargo test -p devil-ai --all-targets
Symbols: none

## plans/evidence/phase-4/tracker-run-ledger-tests.txt

plans/evidence/phase-4/tracker-run-ledger-tests.txt has 26 lines; first content: Phase 4 tracker ledger evidence
Symbols: none

## plans/evidence/phase-4/vector-deferral-audit.md

plans/evidence/phase-4/vector-deferral-audit.md has 29 lines; headings: Phase 4 Vector Deferral Audit, Scope, Evidence; first content: Date: 2026-05-25
Symbols: none

## plans/evidence/phase-5/cargo-check-workspace-all-targets.txt

plans/evidence/phase-5/cargo-check-workspace-all-targets.txt has 7 lines; first content: Command: cargo check --workspace --all-targets
Symbols: none

## plans/evidence/phase-5/cargo-clippy-workspace-all-targets.txt

plans/evidence/phase-5/cargo-clippy-workspace-all-targets.txt has 7 lines; first content: Command: cargo clippy --workspace --all-targets -- -D warnings
Symbols: none

## plans/evidence/phase-5/cargo-fmt-check.txt

plans/evidence/phase-5/cargo-fmt-check.txt has 6 lines; first content: Command: cargo fmt --all --check
Symbols: none

## plans/evidence/phase-5/cargo-test-workspace-all-targets.txt

plans/evidence/phase-5/cargo-test-workspace-all-targets.txt has 9 lines; first content: Command: cargo test --workspace --all-targets
Symbols: none

## plans/evidence/phase-5/dependency-boundary.txt

plans/evidence/phase-5/dependency-boundary.txt has 14 lines; first content: Command: cargo run -p xtask -- check-deps
Symbols: none

## plans/evidence/phase-5/future-surface-deferral-audit.md

plans/evidence/phase-5/future-surface-deferral-audit.md has 22 lines; headings: Phase 5 Future Surface Deferral Audit; first content: Phase 5 activates only the isolated plugin runtime boundary. The following remain inactive and require separate ADRs, de
Symbols: none

## plans/evidence/phase-5/host-call-capability-tests.txt

plans/evidence/phase-5/host-call-capability-tests.txt has 12 lines; first content: Commands:
Symbols: none

## plans/evidence/phase-5/manifest-golden-tests.txt

plans/evidence/phase-5/manifest-golden-tests.txt has 8 lines; first content: Command: cargo test -p devil-protocol --test dto_contracts
Symbols: none

## plans/evidence/phase-5/plugin-architecture-map.md

plans/evidence/phase-5/plugin-architecture-map.md has 61 lines; headings: Phase 5 Plugin Architecture Map, Acceptance Status, Architecture Map; first content: - Phase 5 acceptance: Accepted.
Symbols: none

## plans/evidence/phase-5/plugin-crash-isolation-tests.txt

plans/evidence/phase-5/plugin-crash-isolation-tests.txt has 14 lines; first content: Command: cargo test -p devil-plugin --all-targets
Symbols: none

## plans/evidence/phase-5/plugin-observability-redaction-audit.md

plans/evidence/phase-5/plugin-observability-redaction-audit.md has 13 lines; headings: Phase 5 Plugin Observability Redaction Audit; first content: Plugin audit uses `EventEnvelope` with metadata-only redaction and non-zero schema, correlation id, causality id, and ev
Symbols: none

## plans/evidence/phase-5/plugin-proposal-routing-tests.txt

plans/evidence/phase-5/plugin-proposal-routing-tests.txt has 16 lines; first content: Commands:
Symbols: none

## plans/evidence/phase-5/plugin-storage-quota-tests.txt

plans/evidence/phase-5/plugin-storage-quota-tests.txt has 13 lines; first content: Command: cargo test -p devil-storage --all-targets
Symbols: none

## plans/evidence/phase-5/sandbox-denial-tests.txt

plans/evidence/phase-5/sandbox-denial-tests.txt has 15 lines; first content: Commands:
Symbols: none

## plans/evidence/phase-5/wasm-abi-contract-tests.txt

plans/evidence/phase-5/wasm-abi-contract-tests.txt has 8 lines; first content: Command: cargo test -p devil-protocol --test dto_contracts
Symbols: none

## plans/evidence/phase-6/cargo-check-workspace-all-targets.txt

plans/evidence/phase-6/cargo-check-workspace-all-targets.txt has 7 lines; first content: Command: `cargo check --workspace --all-targets`
Symbols: none

## plans/evidence/phase-6/cargo-clippy-workspace-all-targets.txt

plans/evidence/phase-6/cargo-clippy-workspace-all-targets.txt has 7 lines; first content: Command: `cargo clippy --workspace --all-targets -- -D warnings`
Symbols: none

## plans/evidence/phase-6/cargo-deny-check.txt

plans/evidence/phase-6/cargo-deny-check.txt has 22 lines; first content: Command: `cargo deny check`
Symbols: none

## plans/evidence/phase-6/cargo-fmt-check.txt

plans/evidence/phase-6/cargo-fmt-check.txt has 6 lines; first content: Command: `cargo fmt --all --check`
Symbols: none

## plans/evidence/phase-6/cargo-test-workspace-all-targets.txt

plans/evidence/phase-6/cargo-test-workspace-all-targets.txt has 14 lines; first content: Command: `cargo test --workspace --all-targets`
Symbols: none

## plans/evidence/phase-6/collaboration-architecture-map.md

plans/evidence/phase-6/collaboration-architecture-map.md has 90 lines; headings: Phase 6 Collaboration Architecture Map, Acceptance Status, Runtime Surface Status; first content: - Phase 6 acceptance: Accepted.
Symbols: none

## plans/evidence/phase-6/collaboration-convergence-tests.txt

plans/evidence/phase-6/collaboration-convergence-tests.txt has 13 lines; first content: Command: `cargo test -p devil-collaboration --all-targets`
Symbols: none

## plans/evidence/phase-6/collaboration-security-capability-tests.txt

plans/evidence/phase-6/collaboration-security-capability-tests.txt has 14 lines; first content: Command: `cargo test -p devil-security`
Symbols: none

## plans/evidence/phase-6/dependency-boundary.txt

plans/evidence/phase-6/dependency-boundary.txt has 15 lines; first content: Command: `cargo run -p xtask -- check-deps`
Symbols: none

## plans/evidence/phase-6/dirty-buffer-conflict-tests.txt

plans/evidence/phase-6/dirty-buffer-conflict-tests.txt has 13 lines; first content: Command: `cargo test --workspace --all-targets`
Symbols: none

## plans/evidence/phase-6/disconnect-reconnect-replay-tests.txt

plans/evidence/phase-6/disconnect-reconnect-replay-tests.txt has 15 lines; first content: Command: `cargo test -p devil-collaboration --all-targets`
Symbols: none

## plans/evidence/phase-6/future-surface-deferral-audit.md

plans/evidence/phase-6/future-surface-deferral-audit.md has 23 lines; headings: Phase 6 Future Surface Deferral Audit, Status, Deferred Surfaces; first content: Phase 6 collaboration activates only the local operation-log collaboration runtime, protocol DTOs, metadata audit/replay
Symbols: none

## plans/evidence/phase-6/performance-budget-tests.txt

plans/evidence/phase-6/performance-budget-tests.txt has 28 lines; first content: Commands:
Symbols: none

## plans/evidence/phase-6/presence-ui-projection-tests.txt

plans/evidence/phase-6/presence-ui-projection-tests.txt has 14 lines; first content: Commands:
Symbols: none

## plans/evidence/phase-6/protocol-dto-contract-tests.txt

plans/evidence/phase-6/protocol-dto-contract-tests.txt has 15 lines; first content: Command: `cargo test -p devil-protocol --test dto_contracts`
Symbols: none

## plans/evidence/phase-6/shared-proposal-approval-tests.txt

plans/evidence/phase-6/shared-proposal-approval-tests.txt has 16 lines; first content: Commands:
Symbols: none

## plans/evidence/phase-6/storage-observability-redaction-audit.md

plans/evidence/phase-6/storage-observability-redaction-audit.md has 24 lines; headings: Phase 6 Storage And Observability Redaction Audit, Commands, Result; first content: - `cargo test -p legion-storage`
Symbols: none

## plans/evidence/phase-6/undo-semantics-tests.txt

plans/evidence/phase-6/undo-semantics-tests.txt has 12 lines; first content: Command: `cargo test -p devil-collaboration --all-targets`
Symbols: none

## plans/evidence/phase-7/cargo-check-workspace-all-targets.txt

plans/evidence/phase-7/cargo-check-workspace-all-targets.txt has 8 lines; first content: Command: cargo check --workspace --all-targets
Symbols: none

## plans/evidence/phase-7/cargo-clippy-workspace-all-targets.txt

plans/evidence/phase-7/cargo-clippy-workspace-all-targets.txt has 8 lines; first content: Command: cargo clippy --workspace --all-targets -- -D warnings
Symbols: none

## plans/evidence/phase-7/cargo-deny-check.txt

plans/evidence/phase-7/cargo-deny-check.txt has 9 lines; first content: Command: cargo deny check
Symbols: none

## plans/evidence/phase-7/cargo-fmt-check.txt

plans/evidence/phase-7/cargo-fmt-check.txt has 7 lines; first content: Command: cargo fmt --all --check
Symbols: none

## plans/evidence/phase-7/cargo-test-workspace-all-targets.txt

plans/evidence/phase-7/cargo-test-workspace-all-targets.txt has 10 lines; first content: Command: cargo test --workspace --all-targets
Symbols: none

## plans/evidence/phase-7/collaboration-remote-integration-tests.txt

plans/evidence/phase-7/collaboration-remote-integration-tests.txt has 13 lines; first content: Phase 7 collaboration and remote integration evidence
Symbols: none

## plans/evidence/phase-7/dependency-boundary.txt

plans/evidence/phase-7/dependency-boundary.txt has 15 lines; first content: Phase 7 dependency-boundary evidence
Symbols: none

## plans/evidence/phase-7/future-surface-deferral-audit.md

plans/evidence/phase-7/future-surface-deferral-audit.md has 19 lines; headings: Phase 7 Future Surface Deferral Audit, Status, Deferred Surfaces; first content: Accepted.
Symbols: none

## plans/evidence/phase-7/latency-reconnect-offline-resume-tests.txt

plans/evidence/phase-7/latency-reconnect-offline-resume-tests.txt has 13 lines; first content: Phase 7 latency, reconnect, and offline resume evidence
Symbols: none

## plans/evidence/phase-7/performance-budget-tests.txt

plans/evidence/phase-7/performance-budget-tests.txt has 16 lines; first content: Phase 7 performance budget evidence
Symbols: none

## plans/evidence/phase-7/protocol-dto-contract-tests.txt

plans/evidence/phase-7/protocol-dto-contract-tests.txt has 16 lines; first content: Phase 7 protocol DTO contract evidence
Symbols: none

## plans/evidence/phase-7/remote-agent-lifecycle-tests.txt

plans/evidence/phase-7/remote-agent-lifecycle-tests.txt has 19 lines; first content: Phase 7 remote agent lifecycle evidence
Symbols: none

## plans/evidence/phase-7/remote-architecture-map.md

plans/evidence/phase-7/remote-architecture-map.md has 82 lines; headings: Phase 7 Remote Development Architecture Map, Acceptance Status, Runtime Surface Status; first content: - Phase 7 acceptance: Accepted.
Symbols: none

## plans/evidence/phase-7/remote-filesystem-proposal-tests.txt

plans/evidence/phase-7/remote-filesystem-proposal-tests.txt has 13 lines; first content: Phase 7 remote filesystem proposal evidence
Symbols: none

## plans/evidence/phase-7/remote-lsp-policy-tests.txt

plans/evidence/phase-7/remote-lsp-policy-tests.txt has 11 lines; first content: Phase 7 remote LSP policy evidence
Symbols: none

## plans/evidence/phase-7/remote-process-terminal-policy-tests.txt

plans/evidence/phase-7/remote-process-terminal-policy-tests.txt has 12 lines; first content: Phase 7 remote process and terminal policy evidence
Symbols: none

## plans/evidence/phase-7/remote-security-threat-model.md

plans/evidence/phase-7/remote-security-threat-model.md has 21 lines; headings: Phase 7 Remote Security Threat Model, Status, Threats And Controls; first content: Accepted for deterministic Phase 7 runtime harness.
Symbols: none

## plans/evidence/phase-7/remote-semantic-index-query-tests.txt

plans/evidence/phase-7/remote-semantic-index-query-tests.txt has 11 lines; first content: Phase 7 remote semantic query evidence
Symbols: none

## plans/evidence/phase-7/remote-stale-conflict-tests.txt

plans/evidence/phase-7/remote-stale-conflict-tests.txt has 13 lines; first content: Phase 7 remote stale/conflict evidence
Symbols: none

## plans/evidence/phase-7/storage-observability-redaction-audit.md

plans/evidence/phase-7/storage-observability-redaction-audit.md has 19 lines; headings: Phase 7 Storage And Observability Redaction Audit, Status, Commands; first content: Accepted.
Symbols: none

## plans/evidence/phase-7/transport-security-tests.txt

plans/evidence/phase-7/transport-security-tests.txt has 19 lines; first content: Phase 7 transport and security test evidence
Symbols: none

## plans/evidence/phase-7/ws07-t1-apply-activation-audit.md

plans/evidence/phase-7/ws07-t1-apply-activation-audit.md has 36 lines; headings: WS07.T1 Apply-activation audit, Summary, Activation ADR checklist; first content: Date: 2026-06-11
Symbols: none

## plans/evidence/phase-7/xtask-check-deps.txt

plans/evidence/phase-7/xtask-check-deps.txt has 8 lines; first content: Command: cargo run -p xtask -- check-deps
Symbols: none

## plans/evidence/phase-8/cargo-check-workspace-all-targets.txt

plans/evidence/phase-8/cargo-check-workspace-all-targets.txt has 3 lines; first content: Command: cargo check --workspace --all-targets
Symbols: none

## plans/evidence/phase-8/cargo-clippy-workspace-all-targets.txt

plans/evidence/phase-8/cargo-clippy-workspace-all-targets.txt has 3 lines; first content: Command: cargo clippy --workspace --all-targets -- -D warnings
Symbols: none

## plans/evidence/phase-8/cargo-deny-check.txt

plans/evidence/phase-8/cargo-deny-check.txt has 13 lines; first content: Command: cargo deny check
Symbols: none

## plans/evidence/phase-8/cargo-fmt-check.txt

plans/evidence/phase-8/cargo-fmt-check.txt has 3 lines; first content: Command: cargo fmt --all --check
Symbols: none

## plans/evidence/phase-8/cargo-test-workspace-all-targets.txt

plans/evidence/phase-8/cargo-test-workspace-all-targets.txt has 3 lines; first content: Command: cargo test --workspace --all-targets
Symbols: none

## plans/evidence/phase-8/dependency-boundary.txt

plans/evidence/phase-8/dependency-boundary.txt has 13 lines; first content: Command: cargo run -p xtask -- check-deps
Symbols: none

## plans/evidence/phase-8/enterprise-policy-profile-ci.txt

plans/evidence/phase-8/enterprise-policy-profile-ci.txt has 12 lines; first content: Commands:
Symbols: none

## plans/evidence/phase-8/fault-drill-results.txt

plans/evidence/phase-8/fault-drill-results.txt has 10 lines; first content: Command: cargo test --workspace --all-targets
Symbols: none

## plans/evidence/phase-8/hosted-telemetry-consent-policy-tests.txt

plans/evidence/phase-8/hosted-telemetry-consent-policy-tests.txt has 12 lines; first content: Commands:
Symbols: none

## plans/evidence/phase-8/hosted-telemetry-failure-mode-tests.txt

plans/evidence/phase-8/hosted-telemetry-failure-mode-tests.txt has 13 lines; first content: Command: cargo test -p devil-telemetry --all-targets
Symbols: none

## plans/evidence/phase-8/metadata-replay-drills.txt

plans/evidence/phase-8/metadata-replay-drills.txt has 9 lines; first content: Command: cargo test --workspace --all-targets
Symbols: none

## plans/evidence/phase-8/operational-health-diagnostics.txt

plans/evidence/phase-8/operational-health-diagnostics.txt has 13 lines; first content: Commands:
Symbols: none

## plans/evidence/phase-8/performance-budget-tests.txt

plans/evidence/phase-8/performance-budget-tests.txt has 10 lines; first content: Command: cargo test --workspace --all-targets
Symbols: none

## plans/evidence/phase-8/phase-8-architecture-map.md

plans/evidence/phase-8/phase-8-architecture-map.md has 86 lines; headings: Phase 8 Architecture Map, Acceptance Status, Scope; first content: - Phase 8 acceptance: Accepted.
Symbols: none

## plans/evidence/phase-8/phase-8-threat-model.md

plans/evidence/phase-8/phase-8-threat-model.md has 54 lines; headings: Phase 8 Threat Model, Scope, Assets; first content: Status: initial implementation-gate evidence; not GA acceptance evidence.
Symbols: none

## plans/evidence/phase-8/platform-matrix-evidence.txt

plans/evidence/phase-8/platform-matrix-evidence.txt has 29 lines; first content: Status: final platform matrix evidence archived for Phase 8 GA acceptance.
Symbols: none

## plans/evidence/phase-8/privacy-redaction-classifier-audit.md

plans/evidence/phase-8/privacy-redaction-classifier-audit.md has 15 lines; headings: Phase 8 Privacy Redaction Classifier Audit; first content: Status: implementation evidence
Symbols: none

## plans/evidence/phase-8/protocol-dto-contract-tests.txt

plans/evidence/phase-8/protocol-dto-contract-tests.txt has 15 lines; first content: Command: cargo test -p devil-protocol --test dto_contracts phase8
Symbols: none

## plans/evidence/phase-8/raw-source-retention-lifecycle-tests.txt

plans/evidence/phase-8/raw-source-retention-lifecycle-tests.txt has 20 lines; first content: Command: cargo test -p devil-retention --all-targets
Symbols: none

## plans/evidence/phase-8/raw-source-retention-policy-tests.txt

plans/evidence/phase-8/raw-source-retention-policy-tests.txt has 12 lines; first content: Command: cargo test -p devil-retention --all-targets
Symbols: none

## plans/evidence/phase-8/release-readiness-review.md

plans/evidence/phase-8/release-readiness-review.md has 43 lines; headings: Phase 8 Release Readiness Review; first content: Status: implementation evidence, platform matrix evidence, and final GA signoff are archived for Phase 8 acceptance.
Symbols: none

## plans/evidence/phase-8/remote-agent-packaging-tests.txt

plans/evidence/phase-8/remote-agent-packaging-tests.txt has 8 lines; first content: Command: cargo test -p devil-remote-transport --all-targets
Symbols: none

## plans/evidence/phase-8/remote-production-transport-security-tests.txt

plans/evidence/phase-8/remote-production-transport-security-tests.txt has 20 lines; first content: Commands:
Symbols: none

## plans/evidence/phase-8/storage-migration-recovery-tests.txt

plans/evidence/phase-8/storage-migration-recovery-tests.txt has 10 lines; first content: Command: cargo test -p devil-storage migration_registry
Symbols: none

## plans/evidence/phase-8/terminal-pty-platform-tests.txt

plans/evidence/phase-8/terminal-pty-platform-tests.txt has 19 lines; first content: Commands:
Symbols: none

## plans/evidence/phase-8/terminal-runtime-policy-tests.txt

plans/evidence/phase-8/terminal-runtime-policy-tests.txt has 16 lines; first content: Commands:
Symbols: none

## plans/evidence/phase-8/xtask-check-deps.txt

plans/evidence/phase-8/xtask-check-deps.txt has 6 lines; first content: Command: cargo run -p xtask -- check-deps
Symbols: none

## plans/evidence/platform-parity/P8-F5-T3-platform-parity-report.md

plans/evidence/platform-parity/P8-F5-T3-platform-parity-report.md has 60 lines; headings: P8.F5.T3 Platform Parity Report, Status, macOS parity record; first content: Date: 2026-06-14
Symbols: none

## plans/evidence/production/M0/ADR-0032-ratification.md

plans/evidence/production/M0/ADR-0032-ratification.md has 85 lines; headings: M0 — ADR-0032 (Editor Render Path) Ratification Evidence, Re-verification (post docs-hygiene fix), Decision Recorded; first content: Milestone: **M0 (Plan lock)** — Production Master Plan v0.1
Symbols: none

## plans/evidence/production/M0/ADR-0033-ratification.md

plans/evidence/production/M0/ADR-0033-ratification.md has 159 lines; headings: M0 — ADR-0033 (Syntax/Parse Engine) Ratification Evidence, Decision Recorded, Crate / Dependency Boundary Impact; first content: Milestone: **M0 (Plan lock)** — Production Master Plan v0.1
Symbols: none

## plans/evidence/production/M0/ADR-0034-ratification.md

plans/evidence/production/M0/ADR-0034-ratification.md has 271 lines; headings: M0 — ADR-0034 (LSP Client Architecture) Ratification Evidence, Decision Recorded, Crate / Dependency Boundary Impact; first content: Milestone: **M0 (Plan lock)** — Production Master Plan v0.1
Symbols: none

## plans/evidence/production/M0/ADR-0035-ratification.md

plans/evidence/production/M0/ADR-0035-ratification.md has 336 lines; headings: M0 — ADR-0035 (Terminal Stack) Ratification Evidence, Decision Recorded, Crate / Dependency Boundary Impact; first content: Milestone: **M0 (Plan lock)** — Production Master Plan v0.1
Symbols: none

## plans/evidence/production/M0/ADR-0036-ratification.md

plans/evidence/production/M0/ADR-0036-ratification.md has 403 lines; headings: M0 — ADR-0036 (Search & Index Stack) Ratification Evidence, Decision Recorded, Crate / Dependency Boundary Impact; first content: Milestone: **M0 (Plan lock)** — Production Master Plan v0.1
Symbols: none

## plans/evidence/production/M0/ADR-0037-ratification.md

plans/evidence/production/M0/ADR-0037-ratification.md has 740 lines; headings: M0 — ADR-0037 (Semantic Retrieval) Ratification Evidence, Decision Recorded, Crate / Dependency Boundary Impact; first content: Milestone: **M0 (Plan lock)** — Production Master Plan v0.1
Symbols: none

## plans/evidence/production/M0/ADR-0038-ratification.md

plans/evidence/production/M0/ADR-0038-ratification.md has 962 lines; headings: M0 — ADR-0038 (OS Sandbox Layer) Ratification Evidence, Decision Recorded, Crate / Dependency Boundary Impact; first content: Milestone: **M0 (Plan lock)** — Production Master Plan v0.1
Symbols: none

## plans/evidence/production/M0/ADR-0039-ratification.md

plans/evidence/production/M0/ADR-0039-ratification.md has 798 lines; headings: M0 — ADR-0039 (Agent Interop) Ratification Evidence, Decision Recorded, Crate / Dependency Boundary Impact; first content: Milestone: **M0 (Plan lock)** — Production Master Plan v0.1
Symbols: none

## plans/evidence/production/M0/ADR-0040-ratification.md

plans/evidence/production/M0/ADR-0040-ratification.md has 1023 lines; headings: M0 — ADR-0040 (Concurrent Edit Substrate) Ratification Evidence, Decision Recorded, Crate / Dependency Boundary Impact; first content: Milestone: **M0 (Plan lock)** — Production Master Plan v0.1
Symbols: none

## plans/evidence/production/M0/M0-evidence-bundle.md

plans/evidence/production/M0/M0-evidence-bundle.md has 109 lines; headings: M0 Evidence Bundle, Executive summary, Evidence inventory; first content: Date: 2026-06-13
Symbols: none

## plans/evidence/production/M0/M0-milestone-acceptance.md

plans/evidence/production/M0/M0-milestone-acceptance.md has 66 lines; headings: M0 Milestone Acceptance Gate, Decision, Predecessor Kanban Status; first content: Date: 2026-06-11T04:26:15Z
Symbols: none

## plans/evidence/production/M0/no-egui-textedit-tests.txt

plans/evidence/production/M0/no-egui-textedit-tests.txt has 19 lines; first content: Command: cargo test -p xtask --test no_egui_textedit
Symbols: none

## plans/evidence/production/M0/no-egui-textedit.txt

plans/evidence/production/M0/no-egui-textedit.txt has 14 lines; first content: Command: cargo run -p xtask -- no-egui-textedit
Symbols: none

## plans/evidence/production/M0/WS17-T1-release-pipeline.md

plans/evidence/production/M0/WS17-T1-release-pipeline.md has 128 lines; headings: M0 — WS17.T1 (Release Pipeline) Bootstrap Evidence, Acceptance target, What landed in this card; first content: Milestone: **M0 (Plan lock)** — Production Master Plan v0.1
Symbols: none

## plans/evidence/production/M0/WS18-T1-perf-harness.md

plans/evidence/production/M0/WS18-T1-perf-harness.md has 164 lines; headings: M0 — WS18.T1 (Performance Harness) Skeleton Evidence, Acceptance target, What landed in this card; first content: Milestone: **M0 (Plan lock)** — Production Master Plan v0.1
Symbols: none

## plans/evidence/production/M1/M1-milestone-acceptance.md

plans/evidence/production/M1/M1-milestone-acceptance.md has 57 lines; headings: M1 Milestone Acceptance Gate, Decision, Predecessor Kanban Status; first content: Date: 2026-06-12T00:07:02Z
Symbols: none

## plans/evidence/production/M10/PKT-0-evidence.md

plans/evidence/production/M10/PKT-0-evidence.md has 79 lines; headings: PKT-0 Evidence — Orphan Sweep + Honesty Fixes, Deliverables Completed, D1 — legion-agent orphan modules declared; first content: **Milestone:** M10
Symbols: none

## plans/evidence/production/M10/PKT-CLOSE-evidence.md

plans/evidence/production/M10/PKT-CLOSE-evidence.md has 110 lines; headings: PKT-CLOSE Evidence — M10 Closeout, Summary, Deliverables; first content: Branch: `m12/m10-closeout`
Symbols: none

## plans/evidence/production/M10/PKT-EVAL-evidence.md

plans/evidence/production/M10/PKT-EVAL-evidence.md has 126 lines; headings: PKT-EVAL Evidence: Adversarial Evals Against the Native Loop, Summary, Hostile Eval Scenario Coverage; first content: **Branch:** `m10/adversarial-evals`
Symbols: none

## plans/evidence/production/M10/PKT-GP3-evidence.md

plans/evidence/production/M10/PKT-GP3-evidence.md has 113 lines; headings: PKT-GP3 Evidence — GP-3 Delegate Mode Golden Path, Overview, Deliverables; first content: **Milestone:** M10
Symbols: none

## plans/evidence/production/M10/PKT-LOOP-evidence.md

plans/evidence/production/M10/PKT-LOOP-evidence.md has 104 lines; headings: PKT-LOOP: Native Delegated Task Execution Loop — Evidence, Deliverables, Commits; first content: **Packet:** PKT-LOOP (M10, agent loop)
Symbols: none

## plans/evidence/production/M10/PKT-MODELIO-evidence.md

plans/evidence/production/M10/PKT-MODELIO-evidence.md has 56 lines; headings: PKT-MODELIO Evidence: Tool-Calling Model I/O, Deliverables Completed, D1 — ToolCallingProvider trait + DTOs; first content: **Branch:** `m10/tool-model-io`
Symbols: none

## plans/evidence/production/M10/PKT-PROPOSAL-SURFACE-evidence.md

plans/evidence/production/M10/PKT-PROPOSAL-SURFACE-evidence.md has 76 lines; headings: PKT-PROPOSAL-SURFACE — Evidence, What was implemented, 1. `ToolExecutionOutput` return type + agent_loop.rs plumbing; first content: **Campaign:** M10
Symbols: none

## plans/evidence/production/M10/PKT-SANDBOX-evidence.md

plans/evidence/production/M10/PKT-SANDBOX-evidence.md has 105 lines; headings: PKT-SANDBOX Evidence: OS Sandbox Enforcement, Deliverables Completed, D1 — `spawn.rs` DTOs + extended `SandboxError`; first content: **Branch:** `m10/os-sandbox`
Symbols: none

## plans/evidence/production/M10/PKT-START-evidence.md

plans/evidence/production/M10/PKT-START-evidence.md has 90 lines; headings: PKT-START: Delegate Start Wiring — Evidence, Deliverables, Commits; first content: **Packet:** PKT-START (M10, delegate start)
Symbols: none

## plans/evidence/production/M10/PKT-WORKER-evidence.md

plans/evidence/production/M10/PKT-WORKER-evidence.md has 94 lines; symbols: evidence, panel_vm; headings: PKT-WORKER Evidence, D1 — Worker panel module wired into Delegate dock, D2 — SharedCancellationFlag and cancel_delegated_task; first content: **Branch:** `m10/worker-panel`
Symbols: evidence, panel_vm

## plans/evidence/production/M10/PKT-WORKTREE-evidence.md

plans/evidence/production/M10/PKT-WORKTREE-evidence.md has 74 lines; headings: PKT-WORKTREE Evidence, D1 — Isolation mode reporting (GitWorktree vs DirectoryCopy), D2 — Workspace-root-derived sandbox paths; first content: **Branch:** `m10/worktree-scope`
Symbols: none

## plans/evidence/production/M11/PKT-CONSOLE-evidence.md

plans/evidence/production/M11/PKT-CONSOLE-evidence.md has 81 lines; headings: M11 PKT-CONSOLE Evidence, Scope, RED Evidence; first content: Date: 2026-07-08
Symbols: none

## plans/evidence/production/M11/PKT-GP4-evidence.md

plans/evidence/production/M11/PKT-GP4-evidence.md has 61 lines; headings: M11 PKT-GP4 Evidence, Scope, Verification; first content: Date: 2026-07-08
Symbols: none

## plans/evidence/production/M11/PKT-LANES-evidence.md

plans/evidence/production/M11/PKT-LANES-evidence.md has 80 lines; headings: M11 PKT-LANES Evidence, Scope, RED Evidence; first content: Date: 2026-07-07
Symbols: none

## plans/evidence/production/M11/PKT-OPEN-evidence.md

plans/evidence/production/M11/PKT-OPEN-evidence.md has 200 lines; headings: PKT-OPEN Evidence — M11 Opener, Summary, Deliverables; first content: Branch: `main`
Symbols: none

## plans/evidence/production/M11/PKT-PLAN-evidence.md

plans/evidence/production/M11/PKT-PLAN-evidence.md has 164 lines; headings: PKT-PLAN Evidence - M11 Plan Artifact, Summary, Changed Files; first content: Branch: `m11/plan-artifact`
Symbols: none

## plans/evidence/production/M11/PKT-WORKERS-evidence.md

plans/evidence/production/M11/PKT-WORKERS-evidence.md has 122 lines; headings: PKT-WORKERS Evidence - M11 Real Workflow Workers, Summary, Changed Files; first content: Branch: `m11/real-workers`
Symbols: none

## plans/evidence/production/M12/PKT-CRASH-evidence.md

plans/evidence/production/M12/PKT-CRASH-evidence.md has 162 lines; headings: PKT-CRASH Evidence — Consent-Gated Local Crash Capture, What Was Implemented, 1. Consent-Gated Panic Hook (`crates/legion-observability/src/crash_capture.rs`); first content: **Branch:** `m12/crash-capture`
Symbols: none

## plans/evidence/production/M12/PKT-OPENAI-TOOLS-evidence.md

plans/evidence/production/M12/PKT-OPENAI-TOOLS-evidence.md has 83 lines; headings: PKT-OPENAI-TOOLS Evidence, What was implemented, Wire format mapping; first content: **Campaign:** M12
Symbols: none

## plans/evidence/production/M12/PKT-SIGN-evidence.md

plans/evidence/production/M12/PKT-SIGN-evidence.md has 122 lines; headings: PKT-SIGN Evidence — Real Release Signing Infrastructure (M12), Scope, Security invariants enforced; first content: Branch: `m12/release-signing`
Symbols: none

## plans/evidence/production/M12/PKT-UPDATER-evidence.md

plans/evidence/production/M12/PKT-UPDATER-evidence.md has 118 lines; headings: PKT-UPDATER Evidence — M12 Campaign, What was delivered, 1. Updater client module (`crates/legion-app/src/updater.rs`); first content: **Packet**: PKT-UPDATER (P8.F2 — Auto-update and rollback client)
Symbols: none

## plans/evidence/production/M2/M2-milestone-acceptance.md

plans/evidence/production/M2/M2-milestone-acceptance.md has 57 lines; headings: M2 Milestone Acceptance Gate, Decision, Predecessor Kanban Status; first content: Date: 2026-06-12T17:22:12Z
Symbols: none

## plans/evidence/production/M3/M3-milestone-acceptance.md

plans/evidence/production/M3/M3-milestone-acceptance.md has 38 lines; headings: M3 Milestone Acceptance Gate, Decision, Scope of this acceptance; first content: Date: 2026-06-14T20:47:02Z
Symbols: none

## plans/evidence/production/M3/WS09-T6-mcp-client-ga.md

plans/evidence/production/M3/WS09-T6-mcp-client-ga.md has 31 lines; headings: WS09.T6 MCP Client GA Evidence, Decision, Changes; first content: Date: 2026-06-12
Symbols: none

## plans/evidence/production/M3/WS13-T5-workflow-review-replay.md

plans/evidence/production/M3/WS13-T5-workflow-review-replay.md has 28 lines; headings: WS13.T5 Workflow Review / Replay Evidence, Verdict, Verified behavior; first content: Date: 2026-06-12
Symbols: none

## plans/evidence/production/M3/WS14-T5-privacy-inspector-productization.md

plans/evidence/production/M3/WS14-T5-privacy-inspector-productization.md has 35 lines; headings: WS14.T5 Privacy Inspector Productization Evidence, Verdict, Changes made in this run; first content: Date: 2026-06-12
Symbols: none

## plans/evidence/production/M3/WS14-T6-ai-review-agent-second-opinion.md

plans/evidence/production/M3/WS14-T6-ai-review-agent-second-opinion.md has 32 lines; headings: WS14.T6 AI Review Agent Second-Opinion Evidence, Verdict, Changes made in this run; first content: Date: 2026-06-13
Symbols: none

## plans/evidence/production/M3/WS19-T1-legion-bench-v0.md

plans/evidence/production/M3/WS19-T1-legion-bench-v0.md has 47 lines; headings: WS19.T1 Legion-Bench v0 Evidence, Verdict, What landed in this card; first content: Date: 2026-06-12
Symbols: none

## plans/evidence/production/M4/M4-milestone-acceptance.md

plans/evidence/production/M4/M4-milestone-acceptance.md has 58 lines; headings: M4 Milestone Acceptance Gate, Decision, Predecessor Kanban Status; first content: Date: 2026-06-13T01:45:33Z
Symbols: none

## plans/evidence/production/M5/M5-milestone-acceptance.md

plans/evidence/production/M5/M5-milestone-acceptance.md has 57 lines; headings: M5 Milestone Acceptance Gate, Decision, Predecessor Kanban Status; first content: Date: 2026-06-13T16:10:57Z
Symbols: none

## plans/evidence/production/M5/WS03-T8-server-binary-supply-chain.md

plans/evidence/production/M5/WS03-T8-server-binary-supply-chain.md has 38 lines; headings: M5 — WS03.T8 Server-Binary Supply Chain Evidence, Status, Acceptance target; first content: Accepted.
Symbols: none

## plans/evidence/production/M5/WS05-T6-pty-production-hardening.md

plans/evidence/production/M5/WS05-T6-pty-production-hardening.md has 46 lines; headings: M5 — WS05.T6 PTY Production Hardening Evidence, Status, Acceptance target; first content: Accepted.
Symbols: none

## plans/evidence/production/M5/WS15-T2-launch-extension-set.md

plans/evidence/production/M5/WS15-T2-launch-extension-set.md has 55 lines; headings: M5 — WS15.T2 Launch Extension Set Evidence, Status, Acceptance target; first content: Accepted.
Symbols: none

## plans/evidence/production/M5/WS15-T3-distribution-trust.md

plans/evidence/production/M5/WS15-T3-distribution-trust.md has 46 lines; headings: M5 — WS15.T3 Distribution & Trust Evidence, Status, Acceptance target; first content: Accepted.
Symbols: none

## plans/evidence/production/M5/WS17-T2-signing-notarization.md

plans/evidence/production/M5/WS17-T2-signing-notarization.md has 34 lines; headings: M5 — WS17.T2 Signing & Notarization Evidence, Status, Acceptance target; first content: Accepted.
Symbols: none

## plans/evidence/production/M5/WS17-T3-auto-update-rollback.md

plans/evidence/production/M5/WS17-T3-auto-update-rollback.md has 59 lines; headings: M5 — WS17.T3 Auto-Update + Rollback Evidence, Status, Acceptance target; first content: Accepted for the current repository-local WS17.T3 scaffold: the release pipeline dry-run/verification surface is stable 
Symbols: none

## plans/evidence/production/M5/WS17-T6-docs-support-surface.md

plans/evidence/production/M5/WS17-T6-docs-support-surface.md has 41 lines; headings: M5 — WS17.T6 Docs & Support Surface Evidence, Status, Acceptance target; first content: Verified.
Symbols: none

## plans/evidence/production/M5/WS18-T2-accesskit-product-pass.md

plans/evidence/production/M5/WS18-T2-accesskit-product-pass.md has 66 lines; headings: M5 — WS18.T2 AccessKit Product Pass Evidence, Status, Acceptance target; first content: Verified for OS accessibility-tree inspection evidence.
Symbols: none

## plans/evidence/production/M5/WS18-T3-platform-parity-matrix.md

plans/evidence/production/M5/WS18-T3-platform-parity-matrix.md has 62 lines; headings: M5 — WS18.T3 Platform Parity Matrix Evidence, Status, Acceptance target; first content: Verified for the current Legion tree using fresh macOS-local regression runs plus the archived Linux/Windows/macOS CI ma
Symbols: none

## plans/evidence/production/M5/WS18-T4-multi-window-dpi-smoke.md

plans/evidence/production/M5/WS18-T4-multi-window-dpi-smoke.md has 50 lines; headings: M5 — WS18.T4 Multi-window / Multi-monitor / DPI Smoke Evidence, Status, Acceptance target; first content: Blocked on runner hardware: the current macOS runner exposes only one active display, so a real multi-monitor/per-monito
Symbols: none

## plans/evidence/production/M6/M6-milestone-acceptance.md

plans/evidence/production/M6/M6-milestone-acceptance.md has 45 lines; headings: M6 Milestone Acceptance Gate, Decision, Predecessor Kanban Status; first content: Date: 2026-06-13T16:??:??Z
Symbols: none

## plans/evidence/production/M6/WS15-T4-agent-capability-marketplace-position.md

plans/evidence/production/M6/WS15-T4-agent-capability-marketplace-position.md has 62 lines; headings: M6 — WS15.T4 Agent-Capability Marketplace Position Evidence, Status, Acceptance target; first content: Accepted.
Symbols: none

## plans/evidence/production/M6/WS16-T1-crdt-adoption.md

plans/evidence/production/M6/WS16-T1-crdt-adoption.md has 57 lines; headings: M6 — WS16.T1 CRDT Adoption Evidence, Status, Acceptance target; first content: Accepted.
Symbols: none

## plans/evidence/production/M6/WS16-T2-remote-transport-activation.md

plans/evidence/production/M6/WS16-T2-remote-transport-activation.md has 60 lines; headings: M6 — WS16.T2 Remote Transport Activation Evidence, Status, Acceptance target; first content: Accepted.
Symbols: none

## plans/evidence/production/M6/WS16-T3-cloud-lane-productization.md

plans/evidence/production/M6/WS16-T3-cloud-lane-productization.md has 54 lines; headings: M6 — WS16.T3 Cloud Lane Productization Evidence, Status, Acceptance target; first content: Accepted.
Symbols: none

## plans/evidence/production/M6/WS19-T3-external-benchmark-posture.md

plans/evidence/production/M6/WS19-T3-external-benchmark-posture.md has 57 lines; headings: M6 — WS19.T3 External Benchmark Posture Evidence, Status, Acceptance target; first content: Accepted.
Symbols: none

## plans/evidence/production/M6/WS19-T4-telemetry-to-flywheel-consented.md

plans/evidence/production/M6/WS19-T4-telemetry-to-flywheel-consented.md has 84 lines; headings: M6 — WS19.T4 Telemetry-to-Flywheel (Consented) Evidence, Status, Acceptance target; first content: Accepted.
Symbols: none

## plans/evidence/production/M8/PKT-S3-WEDGE-R3-evidence.md

plans/evidence/production/M8/PKT-S3-WEDGE-R3-evidence.md has 192 lines; headings: PKT-S3-WEDGE-R3 — GP-1 s3 rust-analyzer wedge, round 3, Verdict (read this first), Problem statement; first content: Status: **both root causes identified, fixed, and reproduced red→green**
Symbols: none

## plans/evidence/production/M8/PKT-SMOKE-MACOS-evidence.md

plans/evidence/production/M8/PKT-SMOKE-MACOS-evidence.md has 63 lines; headings: PKT-SMOKE-MACOS — GP-1 s5 failure on macOS CI: root cause and fix, Failure, Diagnosis (instrumented dispatch, run [28747873556](https://github.com/9thLevelSoftware/legion-ide/actions/runs/28747873556)); first content: - Date: 2026-07-05
Symbols: none

## plans/evidence/production/M8/WS-GIT-01-evidence.md

plans/evidence/production/M8/WS-GIT-01-evidence.md has 82 lines; headings: WS-GIT-01 Evidence — PKT-GIT M8 Milestone, Scope, Tasks Completed; first content: **Branch:** `m8/git-residual`
Symbols: none

## plans/evidence/production/M8/WS-LANG-01-product-ui-evidence.md

plans/evidence/production/M8/WS-LANG-01-product-ui-evidence.md has 278 lines; headings: WS-LANG-01 Product UI Evidence — PKT-LSP-B (M8), Summary, Verification Table; first content: **Branch:** m8/lsp-read-ui
Symbols: none

## plans/evidence/production/M8/WS-LANG-01-write-side-evidence.md

plans/evidence/production/M8/WS-LANG-01-write-side-evidence.md has 164 lines; headings: WS-LANG-01 Write-Side Evidence — PKT-LSP-C (M8), Summary, Task 1: Lazy session start; first content: **Branch:** m8/lsp-write-side
Symbols: none

## plans/evidence/production/M8/WS-SEARCH-01-evidence.md

plans/evidence/production/M8/WS-SEARCH-01-evidence.md has 219 lines; headings: M8 — WS-SEARCH-01 Search Polish Evidence, Status, Acceptance targets; first content: Accepted (all review rounds complete — all findings addressed with named passing tests).
Symbols: none

## plans/evidence/production/M8/WS-TERM-01-evidence.md

plans/evidence/production/M8/WS-TERM-01-evidence.md has 337 lines; headings: M8 — WS-TERM-01 Terminal Runtime Productization Evidence, Status, Acceptance targets; first content: Done.
Symbols: none

## plans/evidence/production/M9/PKT-0-residuals-evidence.md

plans/evidence/production/M9/PKT-0-residuals-evidence.md has 104 lines; headings: PKT-0 Residuals Evidence (M9), Task 1: Product RA session watcher=client initialization, Task 2: Perf-harness build-failure heuristic; first content: Branch: `m9/residuals`
Symbols: none

## plans/evidence/production/M9/PKT-APPLY-evidence.md

plans/evidence/production/M9/PKT-APPLY-evidence.md has 114 lines; headings: PKT-APPLY Evidence — M9 Apply Activation, Task summary, What was implemented; first content: Date: 2026-07-05
Symbols: none

## plans/evidence/production/M9/PKT-CKPT-evidence.md

plans/evidence/production/M9/PKT-CKPT-evidence.md has 81 lines; headings: PKT-CKPT Evidence — M9 Checkpoints and Rollback UX, Task Coverage, Test Results; first content: Branch: `m9/checkpoints`
Symbols: none

## plans/evidence/production/M9/PKT-CTX-evidence.md

plans/evidence/production/M9/PKT-CTX-evidence.md has 101 lines; headings: PKT-CTX Evidence — Context Manifest and Privacy Inspector, Task Commits, T1 — assemble_context_manifest_from_sources + 7 collector functions; first content: Branch: `m9/context-manifest`
Symbols: none

## plans/evidence/production/M9/PKT-DIFF-evidence.md

plans/evidence/production/M9/PKT-DIFF-evidence.md has 145 lines; headings: PKT-DIFF Evidence — Multi-file Proposal Review Surface, Status: DONE, Verification Results; first content: Branch: `m9/review-surface`
Symbols: none

## plans/evidence/production/M9/PKT-GP2-evidence.md

plans/evidence/production/M9/PKT-GP2-evidence.md has 73 lines; headings: PKT-GP2: GP-2 Golden Path Smoke Harness — Evidence, Deliverables, GP-2 Steps; first content: **Packet:** PKT-GP2 (Wave 5, M9 milestone closer)
Symbols: none

## plans/evidence/production/M9/PKT-INLINE-evidence.md

plans/evidence/production/M9/PKT-INLINE-evidence.md has 81 lines; headings: PKT-INLINE Evidence — P4.F4 Inline Edit Loop, Commits, Task Coverage; first content: Branch: `m9/inline-edit`
Symbols: none

## plans/evidence/production/M9/PKT-PROV-evidence.md

plans/evidence/production/M9/PKT-PROV-evidence.md has 149 lines; headings: PKT-PROV Evidence — M9 Provider Activation and Policy UX, Task summary, T1 — Provider tiers, workspace consent, activation gate; first content: Date: 2026-07-06
Symbols: none

## plans/evidence/production/M9/PKT-RAIL-evidence.md

plans/evidence/production/M9/PKT-RAIL-evidence.md has 99 lines; headings: PKT-RAIL Evidence — Ghost Text and Assistant Rail, Task summary, T1 — Ghost text overlay view model (e5b05e2); first content: Date: 2026-07-06
Symbols: none

## plans/evidence/production/M9/PKT-RISK-evidence.md

plans/evidence/production/M9/PKT-RISK-evidence.md has 205 lines; headings: PKT-RISK Evidence — Graduated Approvals and Risk Gates, Task Coverage Table, T1 — Risk Rule Coverage Matrix; first content: **Branch:** `m9/risk-gates`
Symbols: none

## plans/evidence/production/W0-truth-reconciliation/W0-7-wasmtime-adr-debt.md

plans/evidence/production/W0-truth-reconciliation/W0-7-wasmtime-adr-debt.md has 59 lines; headings: W0.7 — Wasmtime supply-chain ADR debt (P7 entry blocker), Finding, Why this is the P7 entry blocker; first content: Date: 2026-08-05
Symbols: none

## plans/evidence/production/W0-truth-reconciliation/W0-closure.md

plans/evidence/production/W0-truth-reconciliation/W0-closure.md has 122 lines; headings: Wave 0 — Truth reconciliation closure, Why this wave existed, Changes; first content: Date: 2026-08-05
Symbols: none

## plans/evidence/production/WS-A-D/campaign-charter.md

plans/evidence/production/WS-A-D/campaign-charter.md has 95 lines; headings: WS-A-D campaign charter — Dogfood → DAP → Sandbox → Release, Purpose, Sequencing; first content: **Opened:** 2026-07-21
Symbols: none

## plans/evidence/production/WS-A-D/campaign-closeout-2026-07-22.md

plans/evidence/production/WS-A-D/campaign-closeout-2026-07-22.md has 67 lines; headings: WS-A-D campaign closeout, Outcome, Delivered (on `main`); first content: **Closed:** 2026-07-22
Symbols: none

## plans/evidence/production/WS-A-D/phase-1-dogfood/automated-floor-run-2026-07-21.txt

plans/evidence/production/WS-A-D/phase-1-dogfood/automated-floor-run-2026-07-21.txt has 168 lines; first content: === control_trust_surfaces ===
Symbols: none

## plans/evidence/production/WS-A-D/phase-1-dogfood/phase-1-closeout.md

plans/evidence/production/WS-A-D/phase-1-dogfood/phase-1-closeout.md has 41 lines; headings: Phase 1 dogfood — interim closeout (2026-07-21), Status, Journals; first content: **In progress** toward full Phase 1 DoD (≥3 journals). Automated floor verification + one floor fix landed this session.
Symbols: none

## plans/evidence/production/WS-A-D/phase-1-dogfood/README.md

plans/evidence/production/WS-A-D/phase-1-dogfood/README.md has 16 lines; headings: Phase 1 — Dogfood evidence, This folder, Journals (to date); first content: Session journals live in `plans/evidence/dogfood/` (see template in `plans/dogfood/legion-on-legion-weekly-journal-templ
Symbols: none

## plans/evidence/production/WS-A-D/phase-2-dap/B0-adr-0044-proposal.md

plans/evidence/production/WS-A-D/phase-2-dap/B0-adr-0044-proposal.md has 28 lines; headings: Phase 2 B0 — ADR-0044 proposal evidence, Delivered, Explicitly not in B0; first content: **Date:** 2026-07-21
Symbols: none

## plans/evidence/production/WS-A-D/phase-2-dap/B1-framing-fake-adapter.md

plans/evidence/production/WS-A-D/phase-2-dap/B1-framing-fake-adapter.md has 30 lines; headings: Phase 2 B1 — DAP framing + fake adapter, Delivered, Explicitly not in B1; first content: **Date:** 2026-07-21
Symbols: none

## plans/evidence/production/WS-A-D/phase-2-dap/B10-headless-continue-auto-poll.md

plans/evidence/production/WS-A-D/phase-2-dap/B10-headless-continue-auto-poll.md has 33 lines; headings: Phase 2 B10 — Headless continue → auto-poll dogfood, Problem, Delivered; first content: **Date:** 2026-07-22
Symbols: none

## plans/evidence/production/WS-A-D/phase-2-dap/B11-debug-controls-honesty.md

plans/evidence/production/WS-A-D/phase-2-dap/B11-debug-controls-honesty.md has 39 lines; headings: Phase 2 B11 — Debug toolbar + residual honesty refresh, Problem, Delivered; first content: **Date:** 2026-07-22
Symbols: none

## plans/evidence/production/WS-A-D/phase-2-dap/B12-live-prebuild-cargo.md

plans/evidence/production/WS-A-D/phase-2-dap/B12-live-prebuild-cargo.md has 34 lines; headings: Phase 2 B12 — Live DAP cargo prebuild, Problem, Delivered; first content: **Date:** 2026-07-23
Symbols: none

## plans/evidence/production/WS-A-D/phase-2-dap/B13-system-launch-step-dogfood.md

plans/evidence/production/WS-A-D/phase-2-dap/B13-system-launch-step-dogfood.md has 41 lines; headings: Phase 2 B13 — System adapter launch + step dogfood, Problem, Delivered; first content: **Date:** 2026-07-23
Symbols: none

## plans/evidence/production/WS-A-D/phase-2-dap/B14-debug-keyboard-gui-checklist.md

plans/evidence/production/WS-A-D/phase-2-dap/B14-debug-keyboard-gui-checklist.md has 25 lines; headings: Phase 2 B14 — Debug keyboard + GUI dogfood checklist, Problem, Delivered; first content: **Date:** 2026-07-23
Symbols: none

## plans/evidence/production/WS-A-D/phase-2-dap/B15-f9-toggle-breakpoint.md

plans/evidence/production/WS-A-D/phase-2-dap/B15-f9-toggle-breakpoint.md has 17 lines; headings: Phase 2 B15 — F9 toggle breakpoint, Delivered, Residual; first content: **Date:** 2026-07-23
Symbols: none

## plans/evidence/production/WS-A-D/phase-2-dap/B16-stop-on-entry-default.md

plans/evidence/production/WS-A-D/phase-2-dap/B16-stop-on-entry-default.md has 15 lines; headings: Phase 2 B16 — Cargo debug configs default stop_on_entry, Delivered, Residual; first content: **Date:** 2026-07-23
Symbols: none

## plans/evidence/production/WS-A-D/phase-2-dap/B17-smart-f5-launch.md

plans/evidence/production/WS-A-D/phase-2-dap/B17-smart-f5-launch.md has 18 lines; headings: Phase 2 B17 — Smart F5 launch + preview dogfood checklist, Delivered, Residual; first content: **Date:** 2026-07-23
Symbols: none

## plans/evidence/production/WS-A-D/phase-2-dap/B18-residual-honesty-b17.md

plans/evidence/production/WS-A-D/phase-2-dap/B18-residual-honesty-b17.md has 18 lines; headings: Phase 2 B18 — Residual honesty refresh (through B17), Delivered, No false ready flips; first content: **Date:** 2026-07-23
Symbols: none

## plans/evidence/production/WS-A-D/phase-2-dap/B2-breakpoints-stack-step.md

plans/evidence/production/WS-A-D/phase-2-dap/B2-breakpoints-stack-step.md has 25 lines; headings: Phase 2 B2 — Breakpoints, stack, step (fake adapter), Delivered, Explicitly not in B2; first content: **Date:** 2026-07-21
Symbols: none

## plans/evidence/production/WS-A-D/phase-2-dap/B3-resolution-trust-dual-mode.md

plans/evidence/production/WS-A-D/phase-2-dap/B3-resolution-trust-dual-mode.md has 46 lines; headings: Phase 2 B3 — Adapter resolution, trust, dual-mode honesty, Delivered, Env (operators); first content: **Date:** 2026-07-21
Symbols: none

## plans/evidence/production/WS-A-D/phase-2-dap/B4-microsoft-dap-codec.md

plans/evidence/production/WS-A-D/phase-2-dap/B4-microsoft-dap-codec.md has 45 lines; headings: Phase 2 B4 — Microsoft DAP wire codec, Delivered, Wire shape; first content: **Date:** 2026-07-22
Symbols: none

## plans/evidence/production/WS-A-D/phase-2-dap/B5-persistent-live-session.md

plans/evidence/production/WS-A-D/phase-2-dap/B5-persistent-live-session.md has 46 lines; headings: Phase 2 B5 — Persistent live DAP session, Problem, Delivered; first content: **Date:** 2026-07-22
Symbols: none

## plans/evidence/production/WS-A-D/phase-2-dap/B6-continue-stop.md

plans/evidence/production/WS-A-D/phase-2-dap/B6-continue-stop.md has 35 lines; headings: Phase 2 B6 — Continue-until-stop + disconnect, Delivered, Commands; first content: **Date:** 2026-07-22
Symbols: none

## plans/evidence/production/WS-A-D/phase-2-dap/B7-nonblocking-continue-poll.md

plans/evidence/production/WS-A-D/phase-2-dap/B7-nonblocking-continue-poll.md has 39 lines; headings: Phase 2 B7 — Non-blocking continue + poll, Problem, Delivered; first content: **Date:** 2026-07-22
Symbols: none

## plans/evidence/production/WS-A-D/phase-2-dap/B8-desktop-auto-poll.md

plans/evidence/production/WS-A-D/phase-2-dap/B8-desktop-auto-poll.md has 30 lines; headings: Phase 2 B8 — Desktop auto-poll after non-blocking continue, Problem, Delivered; first content: **Date:** 2026-07-22
Symbols: none

## plans/evidence/production/WS-A-D/phase-2-dap/B9-system-adapter-dogfood.md

plans/evidence/production/WS-A-D/phase-2-dap/B9-system-adapter-dogfood.md has 58 lines; headings: Phase 2 B9 — System adapter dogfood (lldb-dap / CodeLLDB), Problem, Delivered; first content: **Date:** 2026-07-22
Symbols: none

## plans/evidence/production/WS-A-D/phase-2-dap/README.md

plans/evidence/production/WS-A-D/phase-2-dap/README.md has 44 lines; headings: Phase 2 — Real DAP evidence, Packets (B0–B11), Residual; first content: **Current cut line:** Microsoft DAP wire + fake-adapter CI green; persistent live
Symbols: none

## plans/evidence/production/WS-A-D/phase-3-sandbox/C0-threat-model-stub.md

plans/evidence/production/WS-A-D/phase-3-sandbox/C0-threat-model-stub.md has 33 lines; headings: Phase 3 C0 — Sandbox threat model stub, Current enforcement (source of truth), Non-goals; first content: **Date:** 2026-07-21
Symbols: none

## plans/evidence/production/WS-A-D/phase-3-sandbox/C1-linux-network-isolation.md

plans/evidence/production/WS-A-D/phase-3-sandbox/C1-linux-network-isolation.md has 29 lines; headings: Phase 3 C1 — Linux network isolation, Decision, Code; first content: **Date:** 2026-07-21
Symbols: none

## plans/evidence/production/WS-A-D/phase-3-sandbox/C2-windows-fs-residual.md

plans/evidence/production/WS-A-D/phase-3-sandbox/C2-windows-fs-residual.md has 54 lines; headings: Phase 3 C2 — Windows FS residual cut line, Decision, Why not “enforce now”; first content: **Date:** 2026-07-21
Symbols: none

## plans/evidence/production/WS-A-D/phase-3-sandbox/C3-product-spawn-integration.md

plans/evidence/production/WS-A-D/phase-3-sandbox/C3-product-spawn-integration.md has 48 lines; headings: Phase 3 C3 — Product spawn integration, Decision, Honesty updates this slice; first content: **Date:** 2026-07-22
Symbols: none

## plans/evidence/production/WS-A-D/phase-3-sandbox/C4-dap-stdio-sandbox.md

plans/evidence/production/WS-A-D/phase-3-sandbox/C4-dap-stdio-sandbox.md has 36 lines; headings: Phase 3 C4 — DAP adapter sandboxed stdio spawn, Problem, Delivered; first content: **Date:** 2026-07-23
Symbols: none

## plans/evidence/production/WS-A-D/phase-3-sandbox/README.md

plans/evidence/production/WS-A-D/phase-3-sandbox/README.md has 14 lines; headings: Phase 3 — Sandbox isolation evidence; first content: **Matrix:** `docs/SECURITY.md` (§ Sandbox guarantees and platform caveats).
Symbols: none

## plans/evidence/production/WS-A-D/phase-4-release/D0-packaging-design.md

plans/evidence/production/WS-A-D/phase-4-release/D0-packaging-design.md has 85 lines; headings: Phase 4 D0 — Packaging design (preview channel), Goals (MVP), Dist tool; first content: **Date:** 2026-07-21
Symbols: none

## plans/evidence/production/WS-A-D/phase-4-release/D1-unsigned-preview-artifacts.md

plans/evidence/production/WS-A-D/phase-4-release/D1-unsigned-preview-artifacts.md has 46 lines; headings: Phase 4 D1 — Unsigned preview artifacts, Delivered, Artifact shape (unsigned-beta); first content: **Date:** 2026-07-21
Symbols: none

## plans/evidence/production/WS-A-D/phase-4-release/D2-unsigned-beta-retained.md

plans/evidence/production/WS-A-D/phase-4-release/D2-unsigned-beta-retained.md has 59 lines; headings: Phase 4 D2 — Signing path **or** unsigned-beta retained, Decision, What “signed path later” requires (external); first content: **Date:** 2026-07-22
Symbols: none

## plans/evidence/production/WS-A-D/phase-4-release/D3-update-channel-staging.md

plans/evidence/production/WS-A-D/phase-4-release/D3-update-channel-staging.md has 61 lines; headings: Phase 4 D3 — Update channel + staging drill, What already exists (standing gate), Hosted staging feed (D3.1 — open); first content: **Date:** 2026-07-22
Symbols: none

## plans/evidence/production/WS-A-D/phase-4-release/D4-readiness-close.md

plans/evidence/production/WS-A-D/phase-4-release/D4-readiness-close.md has 58 lines; headings: Phase 4 D4 — Readiness close (ledger note), What D4 accepts as “close” for this campaign, Explicitly **not** claimed; first content: **Date:** 2026-07-22
Symbols: none

## plans/evidence/production/WS-A-D/phase-4-release/README.md

plans/evidence/production/WS-A-D/phase-4-release/README.md has 21 lines; headings: Phase 4 — WS17 release evidence, Packets; first content: **Current posture:**
Symbols: none

## plans/evidence/production/WS-A-D/phase-gate-checklist.md

plans/evidence/production/WS-A-D/phase-gate-checklist.md has 85 lines; headings: WS-A-D phase gate checklist, Phase 0 — Scaffolding, Phase 1 — Dogfood (A); first content: Use before starting the next phase. Standing gates remain required for every code merge.
Symbols: none

## plans/evidence/production/WS-LANG-01/P2-F3-T4-test-explorer-discovery.md

plans/evidence/production/WS-LANG-01/P2-F3-T4-test-explorer-discovery.md has 35 lines; headings: P2.F3.T4 — Test explorer cargo discovery (thin slice), Scope delivered, Explicit non-claims (original slice); first content: **Date:** 2026-07-23
Symbols: none

## plans/evidence/production/WS-LANG-01/P2-F3-T4b-test-explorer-run.md

plans/evidence/production/WS-LANG-01/P2-F3-T4b-test-explorer-run.md has 30 lines; headings: P2.F3.T4b — Test explorer per-item exact run, Scope delivered, Explicit non-claims; first content: **Date:** 2026-07-23
Symbols: none

## plans/evidence/production/WS-LANG-01/P2-F3-T4c-lsp-runnable-preference.md

plans/evidence/production/WS-LANG-01/P2-F3-T4c-lsp-runnable-preference.md has 27 lines; headings: P2.F3.T4c — Prefer LSP runnable code lenses for test explorer, Scope delivered, Explicit non-claims; first content: **Date:** 2026-07-23
Symbols: none

## plans/evidence/production/WS-LANG-01/P2-F3-T4d-test-explorer-tree.md

plans/evidence/production/WS-LANG-01/P2-F3-T4d-test-explorer-tree.md has 27 lines; headings: P2.F3.T4d — Test explorer module-path tree grouping, Scope delivered, Explicit non-claims; first content: **Date:** 2026-07-23
Symbols: none

## plans/evidence/production/WS-LANG-01/P2-F3-T5-test-evidence-and-run-group.md

plans/evidence/production/WS-LANG-01/P2-F3-T5-test-evidence-and-run-group.md has 30 lines; headings: P2.F3.T5 — Test explorer → agent evidence + run-group, Scope delivered, Explicit non-claims; first content: **Date:** 2026-07-23
Symbols: none

## plans/evidence/production/WS-LANG-01/P2-F3-T5b-workflow-evidence-attach.md

plans/evidence/production/WS-LANG-01/P2-F3-T5b-workflow-evidence-attach.md has 26 lines; headings: P2.F3.T5b — Attach test-explorer evidence into workflow export, Scope delivered, Explicit non-claims; first content: **Date:** 2026-07-23
Symbols: none

## plans/evidence/production/WS-LANG-01/WS-LANG-01-evidence.md

plans/evidence/production/WS-LANG-01/WS-LANG-01-evidence.md has 92 lines; headings: WS-LANG-01 Rust LSP Substrate Evidence, Workstream status, Product gate; first content: - Status: Complete (single-OS local validation; 3-OS hosted CI deferred — see LANG.12 note)
Symbols: none

## plans/evidence/production/WS-MANUAL-01/editor-latency-budgets.md

plans/evidence/production/WS-MANUAL-01/editor-latency-budgets.md has 25 lines; headings: WS-MANUAL-01 Editor Latency Budgets, Budget Table, Enforcement Rules; first content: Date: 2026-06-19
Symbols: none

## plans/evidence/production/WS-MANUAL-01/manual-mode-zero-egress.md

plans/evidence/production/WS-MANUAL-01/manual-mode-zero-egress.md has 24 lines; headings: Manual Mode Zero-Egress Smoke, Contract, Verification Command; first content: Date: 2026-06-19
Symbols: none

## plans/evidence/production/WS-MANUAL-01/WS-MANUAL-01-evidence.md

plans/evidence/production/WS-MANUAL-01/WS-MANUAL-01-evidence.md has 81 lines; headings: WS-MANUAL-01 Evidence, Branch State, Workstream Coverage; first content: Date: 2026-06-19
Symbols: none

## plans/evidence/production/WS-MANUAL-02/reference-workspaces.md

plans/evidence/production/WS-MANUAL-02/reference-workspaces.md has 60 lines; headings: WS-MANUAL-02 Reference Workspaces, Purpose, Reference workspaces; first content: Define the reference workspaces against which all WS-MANUAL-02 scale tasks are measured.
Symbols: none

## plans/evidence/production/WS-MANUAL-02/WS-MANUAL-02-evidence.md

plans/evidence/production/WS-MANUAL-02/WS-MANUAL-02-evidence.md has 27 lines; headings: WS-MANUAL-02 Large Files and Workspace Scale Evidence, Workstream status, Product gate; first content: - Status: Complete
Symbols: none

## plans/evidence/production/WS-P0/campaign-closeout-2026-07-21.md

plans/evidence/production/WS-P0/campaign-closeout-2026-07-21.md has 39 lines; headings: WS-P0 product-wiring campaign close-out, Delivered slices (evidence), Explicitly deferred (cut lines remain); first content: **Date:** 2026-07-21
Symbols: none

## plans/evidence/production/WS-P0/dirty-worktree-caveat-audit.md

plans/evidence/production/WS-P0/dirty-worktree-caveat-audit.md has 32 lines; headings: WS-P0 Dirty-Worktree Caveat Audit, Decision, Caveat Matrix; first content: Date: 2026-06-19
Symbols: none

## plans/evidence/production/WS-P0/phase-0-gate-baseline.md

plans/evidence/production/WS-P0/phase-0-gate-baseline.md has 230 lines; headings: Phase 0 Gate Baseline — Clean Full-Gate Run with Evidence, Environment substitution (per controller amendment 1), Real failures found and fixed (per Step 3 decision rules); first content: Date: 2026-07-02
Symbols: none

## plans/evidence/production/WS-P0/phase-0-truth-repair-closure.md

plans/evidence/production/WS-P0/phase-0-truth-repair-closure.md has 197 lines; headings: Phase 0 Truth Repair — Closure Evidence, Step 1: Full standing gate set — commands, results, real fixes, Real finding 1: `cargo fmt --all --check` drift on branch-touched files; first content: Date: 2026-07-02
Symbols: none

## plans/evidence/production/WS-P0/T0-A-ledger-claim-repair.md

plans/evidence/production/WS-P0/T0-A-ledger-claim-repair.md has 45 lines; headings: T0-A — Product-readiness ledger and public-doc claim repair, Intent, Changes; first content: **Date:** 2026-07-21
Symbols: none

## plans/evidence/production/WS-P0/T0-B-honest-simulated-ui.md

plans/evidence/production/WS-P0/T0-B-honest-simulated-ui.md has 41 lines; headings: T0-B — Honest UI for simulated / deferred surfaces, Intent, String inventory (before → after); first content: **Date:** 2026-07-21
Symbols: none

## plans/evidence/production/WS-P0/T0-D-smoke-promotion-criteria.md

plans/evidence/production/WS-P0/T0-D-smoke-promotion-criteria.md has 25 lines; headings: T0-D — Golden-path smoke promotion criteria, Decision, Promotion criteria (all required); first content: **Date:** 2026-07-21
Symbols: none

## plans/evidence/production/WS-P0/T0-E-synthetic-gate-honesty.md

plans/evidence/production/WS-P0/T0-E-synthetic-gate-honesty.md has 36 lines; headings: T0-E — Synthetic gate honesty, Intent, Changes; first content: **Date:** 2026-07-21
Symbols: none

## plans/evidence/production/WS-P0/T1-A1-A2-editor-keys-clipboard.md

plans/evidence/production/WS-P0/T1-A1-A2-editor-keys-clipboard.md has 44 lines; headings: T1 — A1/A2 Editor keys + OS clipboard, Changes, A1 — Buffer mutation keys; first content: **Date:** 2026-07-21
Symbols: none

## plans/evidence/production/WS-P0/T1-A8-A10-A11-storage-watcher-terminal.md

plans/evidence/production/WS-P0/T1-A8-A10-A11-storage-watcher-terminal.md has 42 lines; headings: T1 — A8 / A10 / A11 storage, watcher, terminal, A10 — Durable product state, A11 — Recursive watcher poll; first content: **Date:** 2026-07-21
Symbols: none

## plans/evidence/production/WS-P0/T2-assist-real-provider.md

plans/evidence/production/WS-P0/T2-assist-real-provider.md has 32 lines; headings: T2 slice — Assist / inline real provider when credentials exist, Changes, Explicitly still open; first content: **Date:** 2026-07-21
Symbols: none

## plans/evidence/production/WS-P0/T2-byok-ui-sandbox-enforcement.md

plans/evidence/production/WS-P0/T2-byok-ui-sandbox-enforcement.md has 22 lines; headings: T2 follow-on — BYOK UI path + keyring load alignment + live sandbox enforcement, Changes, Verification; first content: **Date:** 2026-07-21
Symbols: none

## plans/evidence/production/WS-P0/T2-delegate-chat-lsp-uri.md

plans/evidence/production/WS-P0/T2-delegate-chat-lsp-uri.md has 20 lines; headings: T2/T3 follow-on — Delegate chat body + LSP location URI paths + router honesty, Changes, Verification; first content: **Date:** 2026-07-21
Symbols: none

## plans/evidence/production/WS-P0/T2-delegate-ui-keyring.md

plans/evidence/production/WS-P0/T2-delegate-ui-keyring.md has 31 lines; headings: T2 slice — Delegate UI path + keyring credential load, Changes, Explicitly still open (later slices); first content: **Date:** 2026-07-21
Symbols: none

## plans/evidence/production/WS-P0/T2-local-first-provider-preference.md

plans/evidence/production/WS-P0/T2-local-first-provider-preference.md has 25 lines; headings: T2 follow-on — Local-first provider preference (Ollama → Anthropic → fixture), Changes, Verification; first content: **Date:** 2026-07-21
Symbols: none

## plans/evidence/production/WS-P0/T2-product-ai-streaming.md

plans/evidence/production/WS-P0/T2-product-ai-streaming.md has 29 lines; headings: T2 follow-on — Product AI streaming (Anthropic SSE → rail projection), Changes, Honest limits; first content: **Date:** 2026-07-21
Symbols: none

## plans/evidence/production/WS-P0/T2-progressive-sse-live-stream.md

plans/evidence/production/WS-P0/T2-progressive-sse-live-stream.md has 31 lines; headings: T2 follow-on — Progressive Anthropic SSE + live stream sink, Changes, Honest limits; first content: **Date:** 2026-07-21
Symbols: none

## plans/evidence/production/WS-P0/T3-dap-honest-cut-line.md

plans/evidence/production/WS-P0/T3-dap-honest-cut-line.md has 35 lines; headings: T3 slice — DAP honest cut line (simulated fixture), Decision, Changes; first content: **Date:** 2026-07-21
Symbols: none

## plans/evidence/production/WS-P0/WS-P0-rebaseline-evidence-refresh.md

plans/evidence/production/WS-P0/WS-P0-rebaseline-evidence-refresh.md has 60 lines; headings: WS-P0 Rebaseline Evidence — Refresh, Changes Made, Gate Verification; first content: Date: 2026-07-01
Symbols: none

## plans/evidence/production/WS-P0/WS-P0-rebaseline-evidence.md

plans/evidence/production/WS-P0/WS-P0-rebaseline-evidence.md has 45 lines; headings: WS-P0 Rebaseline Evidence, Branch State, Completed Tasks; first content: Date: 2026-06-19
Symbols: none

## plans/evidence/release/P8-F1-T3-fresh-vm-gatekeeper-smartscreen-install-smoke.md

plans/evidence/release/P8-F1-T3-fresh-vm-gatekeeper-smartscreen-install-smoke.md has 28 lines; headings: P8.F1.T3 — Fresh-VM Gatekeeper/SmartScreen/Install Smoke Evidence, Status, Scope; first content: Archived checkpoint.
Symbols: none

## plans/evidence/remote/P9-F3-T2-reconnect-offline-evidence.md

plans/evidence/remote/P9-F3-T2-reconnect-offline-evidence.md has 28 lines; headings: P9.F3.T2 remote transport reconnect/offline evidence, Scope, Evidence recorded; first content: Date: 2026-06-15
Symbols: none

## plans/evidence/security/P9-F2-T4-external-audit-gate.md

plans/evidence/security/P9-F2-T4-external-audit-gate.md has 22 lines; headings: P9.F2.T4 external audit / pen-test gate evidence, Scope, Evidence recorded; first content: Date: 2026-06-15
Symbols: none

## plans/evidence/training-flywheel/P9-F4-T3-consented-corpus-legion-bench-comparison.md

plans/evidence/training-flywheel/P9-F4-T3-consented-corpus-legion-bench-comparison.md has 35 lines; headings: P9.F4.T3 — Consented Corpus Legion-Bench Comparison, Verdict, Evidence archived; first content: Date: 2026-06-15
Symbols: none

## plans/foundational-core-ide-platform-implementation-plan-v0.1.md

plans/foundational-core-ide-platform-implementation-plan-v0.1.md has 330 lines; headings: Foundational Core IDE Platform Implementation Plan v0.1, Objective, Context Reviewed; first content: > Historical/superseded planning artifact. Do not use this file as current implementation direction without checking `RE
Symbols: none

## plans/foundational-core-ide-platform-roadmap-v0.1.md

plans/foundational-core-ide-platform-roadmap-v0.1.md has 344 lines; headings: Foundational Core IDE Platform Roadmap v0.1, Purpose, Baseline Assumptions; first content: This roadmap is the execution blueprint for shipping a standalone, deterministic, low-latency core IDE that remains usef
Symbols: none

## plans/ide-core-architecture-spec-v0.1.md

plans/ide-core-architecture-spec-v0.1.md has 1025 lines; headings: Legion IDE Core Architecture and Design Specification v0.1, 1. Source Inputs Reviewed, 2. Executive Architecture Position; first content: Status: Draft for architecture review
Symbols: none

## plans/implementation-plan.md

plans/implementation-plan.md has 553 lines; headings: Legion IDE 2026 Implementation Plan, Executive Summary, Strategic Program Timeline; first content: The architecture review in [`plans/architecture-review-2026-ide-roadmap-v0.1.md`](plans/architecture-review-2026-ide-roa
Symbols: none

## plans/kanban/legion-ga-backlog.toml

plans/kanban/legion-ga-backlog.toml has 2165 lines; headings: Legion IDE — machine-readable Kanban backlog (P0–P9), Generated from .hermes/plans/2026-06-13_173122-legion-current-to-ga-kanban-plan.md, (commit-era source of truth: 2026-06-13).; first content: [meta]
Symbols: none

## plans/legion-e2e/00_CONSOLIDATED_E2E_IMPLEMENTATION_PLAN.md

plans/legion-e2e/00_CONSOLIDATED_E2E_IMPLEMENTATION_PLAN.md has 650 lines; headings: Legion E2E Design, Development, and Implementation Plan, Source: `00_INDEX.md`, Source: `01_FRONTEND_APP_ARCHITECTURE_PLAN.md`; first content: > **Historical / supporting material.** This is a pre-Legion-rename consolidated E2E plan and is preserved for traceabil
Symbols: none

## plans/legion-e2e/source-package/00_INDEX.md

plans/legion-e2e/source-package/00_INDEX.md has 64 lines; headings: Legion IDE planning package; first content: Generated: 2026-06-01 16:24:53 EDT
Symbols: none

## plans/legion-e2e/source-package/01_FRONTEND_APP_ARCHITECTURE_PLAN.md

plans/legion-e2e/source-package/01_FRONTEND_APP_ARCHITECTURE_PLAN.md has 836 lines; headings: 01 — Legion IDE Front-End App Architecture Plan, 0. Executive summary, 1. Current known repo state; first content: Generated: 2026-06-01 16:24:53 EDT
Symbols: none

## plans/legion-e2e/source-package/02_BACKEND_APP_ARCHITECTURE_PLAN.md

plans/legion-e2e/source-package/02_BACKEND_APP_ARCHITECTURE_PLAN.md has 932 lines; headings: 02 — Legion IDE Back-End / Local Runtime Architecture Plan, 0. Executive summary, 1. Existing crate responsibilities and target responsibilities; first content: Generated: 2026-06-01 16:24:53 EDT
Symbols: none

## plans/legion-e2e/source-package/03_CLOUD_OFFERING_ARCHITECTURE_PLAN.md

plans/legion-e2e/source-package/03_CLOUD_OFFERING_ARCHITECTURE_PLAN.md has 949 lines; headings: 03 — Legion Cloud Offering Architecture and Provider Plan, 0. Executive summary, 1. Verified provider research summary; first content: Generated: 2026-06-01 16:24:53 EDT
Symbols: none

## plans/legion-e2e/source-package/04_PRODUCT_IMPLEMENTATION_ROADMAP.md

plans/legion-e2e/source-package/04_PRODUCT_IMPLEMENTATION_ROADMAP.md has 767 lines; headings: 04 — Legion Product Design, Development, and Implementation Roadmap, 0. Executive summary, 1. What has already been built / found; first content: Generated: 2026-06-01 16:24:53 EDT
Symbols: none

## plans/legion-e2e/source-package/05_MODEL_ACQUISITION_AND_TRAINING_PLAN.md

plans/legion-e2e/source-package/05_MODEL_ACQUISITION_AND_TRAINING_PLAN.md has 938 lines; headings: 05 — Legion Model Acquisition, Training, Evaluation, and Serving Plan, 0. Executive summary, 1. Verified model metadata; first content: Generated: 2026-06-01 16:24:53 EDT
Symbols: none

## plans/legion-production-master-plan-v0.1.md

plans/legion-production-master-plan-v0.1.md has 640 lines; headings: Legion IDE — Production Master Plan v0.1, 1. Executive Summary, 2. Method and Evidence Base; first content: > Historical status: this v0.1 plan was superseded by `plans/legion-production-master-plan-v0.2.md` on 2026-06-19. It is
Symbols: none

## plans/legion-production-master-plan-v0.2.md

plans/legion-production-master-plan-v0.2.md has 1093 lines; headings: Legion IDE - Production Master Plan v0.2, 1. Executive Verdict, 2. Evidence Basis; first content: - Status: Draft for review
Symbols: none

## plans/milestone-0-feasibility-proofs.md

plans/milestone-0-feasibility-proofs.md has 87 lines; headings: Milestone 0: Spike 1A Feasibility Proofs, Status, Purpose; first content: Accepted
Symbols: none

## plans/phase-status-ledger.md

plans/phase-status-ledger.md has 112 lines; headings: Legion IDE Phase Status Ledger, Phase summary, ADR status reconciliation; first content: Prepared: 2026-05-24
Symbols: none

## plans/product-readiness-ledger.md

plans/product-readiness-ledger.md has 70 lines; headings: Legion IDE Product Readiness Ledger, Product Target, Gate Rules; first content: This ledger is the product-readiness track for Legion as an enterprise AI-native IDE with required VS Code extension com
Symbols: none

## plans/proposal-execution-lsp-runtime-gating-plan-v0.1.md

plans/proposal-execution-lsp-runtime-gating-plan-v0.1.md has 234 lines; headings: Proposal Execution and LSP Runtime Gating Plan v0.1, Status, Scope and non-goals; first content: > Historical/superseded planning artifact. This proposal/LSP handoff is superseded by accepted Phase 2 and Phase 3 evide
Symbols: none

## plans/remaining-implementation-tasks-plan-v0.1.md

plans/remaining-implementation-tasks-plan-v0.1.md has 421 lines; headings: Legion IDE Remaining Implementation Tasks Plan v0.1, 1. Reviewed documentation baseline, 2. Current implementation status; first content: Status: Planning handoff
Symbols: none

## plans/semantic-index-boundary-remediation-plan-v0.1.md

plans/semantic-index-boundary-remediation-plan-v0.1.md has 160 lines; headings: Semantic Index Boundary Remediation Plan v0.1, Scope and constraints, Current-state evidence; first content: > Historical/superseded planning artifact. This plan is superseded by accepted Phase 3 evidence in `plans/evidence/phase
Symbols: none

## plans/SPIKE-000-platform-boundary-proof.md

plans/SPIKE-000-platform-boundary-proof.md has 66 lines; headings: SPIKE-000: Platform Boundary Proof, Status, Objective; first content: Accepted
Symbols: none

## plans/SPIKE-001A-native-shell-proof.md

plans/SPIKE-001A-native-shell-proof.md has 48 lines; headings: SPIKE-001A: Native UI Editor Latency Proof, Status, Objective; first content: Accepted with reservations
Symbols: none

## plans/spikes/SPIKE-001A-result.md

plans/spikes/SPIKE-001A-result.md has 63 lines; headings: SPIKE-001A Result — Native Shell + Text Model, Scope, Acceptance commands; first content: Status: Accepted
Symbols: none

## plans/spikes/SPIKE-0037-vector-store-result.md

plans/spikes/SPIKE-0037-vector-store-result.md has 76 lines; headings: SPIKE-0037: Vector Store Choice for ADR-0037, Context, Spike constraints; first content: - Status: Draft for M0 ratification
Symbols: none

## plans/spikes/SPIKE-WS08-T6-jj-lib-result.md

plans/spikes/SPIKE-WS08-T6-jj-lib-result.md has 64 lines; headings: SPIKE-WS08-T6: jj-lib exploration result, Question, Evidence reviewed; first content: Status: Complete
Symbols: none

## pyproject.toml

pyproject.toml has 22 lines; headings: Minimal Python metadata for Legion training/eval harnesses., This project is primarily a Rust workspace; Python entrypoints are standalone, scripts with lazy imports for heavy dependencies.; first content: [project]
Symbols: none

## README.md

README.md has 116 lines; headings: Legion IDE, Current Status, Architecture at a Glance; first content: > **License notice:** This codebase is proprietary software. All rights reserved. The source in this repository is provi
Symbols: none

## REVIEW.md

REVIEW.md has 194 lines; headings: PR Review Guidelines, Goals, Required review passes; first content: This file is the source of truth for how pull requests are reviewed in this repository. It exists so every reviewer appl
Symbols: none

## scripts/models/__init__.py

scripts/models/__init__.py has 2 lines; first content: """Legion model workflow helpers."""
Symbols: none

## scripts/models/local_worker_launcher.py

scripts/models/local_worker_launcher.py has 114 lines; symbols: main, parse_workers_config, validate_worker; first content: """Dry-run and launch helper for local Legion worker endpoints."""
Symbols: main, parse_workers_config, validate_worker

## scripts/models/model_manifest.py

scripts/models/model_manifest.py has 73 lines; symbols: main, ModelSpec, roster_as_dicts; first content: """Phase 8 model roster used by dry-run helpers."""
Symbols: main, ModelSpec, roster_as_dicts

## training/__init__.py

training/__init__.py has 2 lines; first content: """Legion Phase 8 training helpers."""
Symbols: none

## training/convert_to_gguf.py

training/convert_to_gguf.py has 200 lines; symbols: _build_manifest, _run_fixture_smoke, _run_real, _validate_tool, _write_manifest, main; headings: Simulate the conversion command list (no shell interpolation), Build explicit arg lists — no shell interpolation; first content: """Conversion harness for trained Legion adapters with optional real and fixture paths."""
Symbols: _build_manifest, _run_fixture_smoke, _run_real, _validate_tool, _write_manifest, main

## training/qlora_train.py

training/qlora_train.py has 251 lines; symbols: _build_training_plan, _import_training_deps, _load_jsonl, _run_fixture_smoke, _validate_dataset, _write_manifest; headings: Minimal real code skeleton: validate dataset loading and dep versions, Real mode: validate deps and build a manifest. Do not launch long training, unless operator explicitly provides positive max_steps.; first content: """QLoRA training entrypoint for Legion specialist models with optional real paths."""
Symbols: _build_training_plan, _import_training_deps, _load_jsonl, _run_fixture_smoke, _validate_dataset, _write_manifest, main

## training/README.md

training/README.md has 40 lines; headings: Legion Training Harness, Dry-run commands, Fixture smoke tests (CI-safe, no heavy deps); first content: Phase 8 keeps training opt-in and consent-gated. The checked-in Python entrypoints validate and print operator plans wit
Symbols: none

## xtask/Cargo.toml

xtask/Cargo.toml has 27 lines; headings: PKT-SIGN: Ed25519 signing infrastructure (ADR-0042)., BSD-3-Clause + MIT/Apache; adds curve25519-dalek, ed25519, signature, fiat-crypto, (all new single-version crates with no deny.toml conflicts).; first content: [package]
Symbols: none

## xtask/legion-policy.example.toml

xtask/legion-policy.example.toml has 237 lines; headings: Example Legion signed policy bundle for a restrictive enterprise profile., It keeps automation locked to a narrow allowlist, enforces a mode ceiling, and, leaves every export/retention surface disabled unless explicitly budgeted below.; first content: schema_version = 1
Symbols: none

## xtask/no-egui-textedit.toml

xtask/no-egui-textedit.toml has 10 lines; headings: M0 / WS01.T1 guardrail: the code canvas must remain a custom painter,, not an egui::TextEdit-backed widget. Palette/search text inputs outside the, code-canvas render path are intentionally out of scope for this gate.; first content: scanned_paths = [
Symbols: none

## xtask/release-pipeline.example.toml

xtask/release-pipeline.example.toml has 100 lines; headings: Legion desktop release pipeline (WS17.T1 dry-run scaffold + PKT-SIGN signing paths), channel: "stable"  -> version = <workspace.package.version>, rollout_policy = "full", channel: "preview" -> version = <workspace.package.version>-preview, rollout_policy = "staged"; first content: package_name = "legion-desktop"
Symbols: none

## xtask/src/claim_audit.rs

xtask/src/claim_audit.rs has 177 lines; symbols: cells, end, following, FORBIDDEN_PHRASES, gate_cell, leading_ok; first content: const FORBIDDEN_PHRASES: [&str; 4] = [
Symbols: cells, end, following, FORBIDDEN_PHRASES, gate_cell, leading_ok, lookbehind_start, lower, mut, NEGATION_FOLLOWUPS, NEGATION_LOOKBEHIND_CHARS, NEGATION_MARKERS, phrase_end, phrase_start, preceding, requires_leading_boundary, Some, start, trailing_ok

## xtask/src/docs_hygiene.rs

xtask/src/docs_hygiene.rs has 535 lines; symbols: ADR_DIR, adr_files, after_hashes, bytes, ch, digits; first content: use std::{
Symbols: ADR_DIR, adr_files, after_hashes, bytes, ch, digits, EVIDENCE_DIR, files, hashes, is_line_suffix, label, LATEST_PRODUCTION_MASTER_PLAN, line_number, markdown_files, mut, Ok, output, path, prefix, PRODUCTION_PLAN_ENTRYPOINTS, raw, rel, rel_path, relative_path, resolved, rest, Some, STALE_MODE_TAXONOMY_LABELS, start, text, trimmed, trimmed_start, without_anchor, without_line

## xtask/src/golden_path_2.rs

xtask/src/golden_path_2.rs has 92 lines; symbols: fixture_dir, mut, status; first content: use std::{path::Path, process};
Symbols: fixture_dir, mut, status

## xtask/src/golden_path_3.rs

xtask/src/golden_path_3.rs has 97 lines; symbols: fixture_dir, mut, status; first content: use std::{path::Path, process};
Symbols: fixture_dir, mut, status

## xtask/src/golden_path_4.rs

xtask/src/golden_path_4.rs has 96 lines; symbols: fixture_dir, mut, status; first content: use std::{path::Path, process};
Symbols: fixture_dir, mut, status

## xtask/src/golden_path.rs

xtask/src/golden_path.rs has 88 lines; symbols: fixture_dir, mut, status; first content: use std::{path::Path, process};
Symbols: fixture_dir, mut, status

## xtask/src/kanban_backlog.rs

xtask/src/kanban_backlog.rs has 513 lines; symbols: backlog, c, err, has_evidence, mut, present; first content: use std::{
Symbols: backlog, c, err, has_evidence, mut, present, REQUIRED_TASK_FIELDS, status, STATUSES_REQUIRING_EVIDENCE, text, VALID_EXTERNAL_UNBLOCKS, VALID_STATUS_VALUES

## xtask/src/legion_bench.rs

xtask/src/legion_bench.rs has 704 lines; symbols: BENCH_SCHEMA_VERSION, budget, cost_cents, d, days, DEFAULT_LIVE_PROFILE; first content: use std::{
Symbols: BENCH_SCHEMA_VERSION, budget, cost_cents, d, days, DEFAULT_LIVE_PROFILE, DEFAULT_RECORDING_PROFILE, DEFAULT_SUITE_NAME, diff_files, doe, doy, era, fixture_repo, fixture_repos, hostile_tasks, hour, kinds, m, minute, mp, mut, notes, now, ordinal, passed, path, provider_profile, recomputed, results, score, second, secs, secs_of_day, slack, status, suite, suite_fingerprint, summary, tests_gate, tests_passed, text, turns, y, yoe, z

## xtask/src/lib.rs

xtask/src/lib.rs has 15 lines; first content: pub mod claim_audit;
Symbols: none

## xtask/src/main.rs

xtask/src/main.rs has 5512 lines; symbols: after_is_boundary, after_start, all_validated, allowed_packages, allowlist_path, args; headings: Acceptance status (inside a code fence)\n\, Acceptance status\n\, Next\n\; first content: use std::{
Symbols: after_is_boundary, after_start, all_validated, allowed_packages, allowlist_path, args, artifacts, artifacts_dir, artifacts_dir_buf, backlog, before_is_boundary, blocked_context, budgets, channel, channel_parsed, checklist_marker, code, commands, config, config_path, content, DEFAULT_BENCH_OUTPUT_PATH, DEFAULT_CLAIM_AUDIT_LEDGER_PATH, DEFAULT_DOCS_HYGIENE_ALLOWLIST_PATH, DEFAULT_GUI_PHASE5_EVIDENCE_PATH, DEFAULT_GUI_PHASE6_EVIDENCE_PATH, DEFAULT_GUI_PHASE7_EVIDENCE_PATH, DEFAULT_GUI_PHASE8_EVIDENCE_PATH, DEFAULT_NO_EGUI_TEXTEDIT_CONFIG_PATH, DEFAULT_PERF_HARNESS_OUTPUT_PATH, DEFAULT_PHASE13_EVIDENCE_PATH, DEFAULT_PHASE13_FINAL_GATES_PATH, DEFAULT_PHASE13_RUNBOOK_PATH, DEFAULT_PHASE3_EVIDENCE_PATH, DEFAULT_PHASE4_EVIDENCE_PATH, DEFAULT_PHASE5_EVIDENCE_PATH, DEFAULT_PHASE6_EVIDENCE_PATH, DEFAULT_PHASE7_EVIDENCE_PATH, DEFAULT_PHASE8_EVIDENCE_PATH, DEFAULT_POLICY_PATH, DEFAULT_PROTOCOL_PATH, DEFAULT_RELEASE_PIPELINE_CONFIG_PATH, DEFAULT_RELEASE_PIPELINE_OUTPUT_PATH, DEFAULT_UI_MANIFEST_PATH, dependencies, deps, disclaimer, docs_dir, end, evidence

## xtask/src/no_egui_textedit.rs

xtask/src/no_egui_textedit.rs has 290 lines; symbols: after_ok, before_ok, end, line_number, mut, Ok; first content: use std::{
Symbols: after_ok, before_ok, end, line_number, mut, Ok, output, path, prefix, raw, rel, rust_files, start, text

## xtask/src/perf_harness.rs

xtask/src/perf_harness.rs has 1092 lines; symbols: _, actor, actor_fs, budget, budget_micros, buf; first content: use std::{
Symbols: _, actor, actor_fs, budget, budget_micros, buf, cancel_elapsed, cancel_pattern, cancel_start, cancel_us, ceiling, content, content_hash, d, days, doe, doy, era, file_count, fixture_created, fixture_lines, fixture_root, footprint, hit_count, hour, idx, key, line, line_count, LINE_GALLEY_DEFAULT_BUDGET_MILLIS, LINE_GALLEY_FIXTURE_LINES, LINE_GALLEY_VISIBLE_ROWS, line_index, m, make_query, MANUAL_RENDERER_KEYPRESS_P50_BUDGET_MILLIS, MANUAL_RENDERER_KEYPRESS_P95_BUDGET_MILLIS, MANUAL_RENDERER_SAMPLE_COUNT, MANUAL_RENDERER_SCENARIO, MANUAL_RENDERER_SCROLL_P95_BUDGET_MILLIS, measurement, measurements, MEMORY_CEILING_DEFAULT_BUDGET_BYTES, MEMORY_CEILING_FIXTURE_BYTES, message, minute, mp, mut, needle, now

## xtask/src/readiness_consistency.rs

xtask/src/readiness_consistency.rs has 486 lines; symbols: backlog, bytes, CLAUSE_SEPARATORS, connector, context, CONTEXT_CHARS; first content: use std::{collections::BTreeMap, fs, path::Path};
Symbols: backlog, bytes, CLAUSE_SEPARATORS, connector, context, CONTEXT_CHARS, coordinated, DELIVERED_MARKERS, end, feature_start, filler, found, ids, index, ledger, ledger_text, lines, mentions, mut, OPEN_MARKERS, siblings, Some, start, statuses, task_start, violation, violations, window_end, window_start

## xtask/src/release_pipeline.rs

xtask/src/release_pipeline.rs has 836 lines; symbols: actual_text, artifact_file, artifacts, bytes, d, days; first content: use std::{
Symbols: actual_text, artifact_file, artifacts, bytes, d, days, descriptor_path, dir, doe, doy, DRY_RUN_VERIFIER_MESSAGE, DRY_RUN_VERIFIER_STATUS, era, expected_text, hash, hour, m, manifest, manifest_bytes, manifest_path, manifest_toml, minute, mp, mut, now, output, parsed, path, report, report_path, report_text, second, secs, secs_of_day, sha, sha256, sig_bytes, sig_path, signer_reference, signer_status, signer_status_for_mode, stamp_path, stamp_text, status, stem, text, version, version_stamp, workspace_version, written_stamp

## xtask/src/signing.rs

xtask/src/signing.rs has 300 lines; symbols: entry, key_bytes, len, secret, seed_arr, seed_bytes; first content: use std::fmt;
Symbols: entry, key_bytes, len, secret, seed_arr, seed_bytes, sig, sig_bytes, signature, signer, value, var_name, vk

## xtask/src/update_drill.rs

xtask/src/update_drill.rs has 82 lines; symbols: cargo_args, code, status; first content: use std::{path::Path, process};
Symbols: cargo_args, code, status

## xtask/tests/claim_audit.rs

xtask/tests/claim_audit.rs has 141 lines; symbols: ledger, line, rows, violations; first content: use xtask::claim_audit::{ClaimViolation, audit_text};
Symbols: ledger, line, rows, violations

## xtask/tests/docs_hygiene.rs

xtask/tests/docs_hygiene.rs has 447 lines; symbols: _, add, config, init, path, repo; first content: use std::{
Symbols: _, add, config, init, path, repo, result, root, stamp, violations

## xtask/tests/kanban_backlog.rs

xtask/tests/kanban_backlog.rs has 455 lines; symbols: _, backlog, combined, dir, err, extra_task; first content: use std::{
Symbols: _, backlog, combined, dir, err, extra_task, ids, msg, mut, path, result, root, SAMPLE_EVIDENCE, stamp, toml_src

## xtask/tests/legion_bench.rs

xtask/tests/legion_bench.rs has 169 lines; symbols: bug_fix, err, live, multi_file, mut, nanos; first content: use xtask::legion_bench::{
Symbols: bug_fix, err, live, multi_file, mut, nanos, path, recorded, refactor, report, root, round_trip, seq, suite, temp_dir, test_add

## xtask/tests/manifest_sign.rs

xtask/tests/manifest_sign.rs has 464 lines; symbols: config, data, encoded, err, manifest, manifest_data; first content: use xtask::signing::{
Symbols: config, data, encoded, err, manifest, manifest_data, mut, original, parsed, result, seed, seed_a, seed_b, short_seed, sig, sig_a, signer, signer_a, signer_b, tampered, toml_str, var_name, vk, vk_b

## xtask/tests/no_egui_textedit.rs

xtask/tests/no_egui_textedit.rs has 135 lines; symbols: _, config, path, repo, result, root; first content: use std::{
Symbols: _, config, path, repo, result, root, stamp, violations

## xtask/tests/perf_harness.rs

xtask/tests/perf_harness.rs has 597 lines; symbols: _, budgets, cases, left, left_total, legacy; first content: use std::{
Symbols: _, budgets, cases, left, left_total, legacy, measurement, mut, nanos, original, out_dir, parsed, path, ratio, report, right, right_total, root, round_trip, seq, serialized, sha, skeleton, skeletons, temp, text

## xtask/tests/release_pipeline.rs

xtask/tests/release_pipeline.rs has 519 lines; symbols: _, artifacts_dir, config, config_path, err, error; first content: use std::{
Symbols: _, artifacts_dir, config, config_path, err, error, first, first_contents, head, installer, mut, out_dir, output, pid, plan, preview, repo, report, root, run, second, second_contents, seq, sha, stable, stamp, stamp_text, tampered_entry, tampered_path, written

