//! Native synchronous delegated task execution loop.
//!
//! Implements the core agent loop: one model turn at a time, each tool call
//! audit-paired (request before dispatch, result after), all budgets bounded,
//! no async/tokio. Tool executors operate inside `worktree_root`; scope and
//! broker checks use workspace-mapped paths derived from `workspace_root`.

use std::path::{Path, PathBuf};

use legion_ai::redaction::redact_model_bound_output;
use legion_ai::tool_calls::{
    ToolCallingProvider, ToolCompletionRequest, ToolCompletionStopReason, ToolConversationTurn,
    ToolDefinition, ToolTurnBlock,
};
use legion_protocol::{
    AssistedAiEditProposalOutput, CapabilityId, CapabilityRequest, CapabilityRequestContext,
    CapabilityResponse, CorrelationId, DelegatedTaskLoopBudget, DelegatedTaskLoopStepKind,
    DelegatedTaskLoopStepRecord, DelegatedTaskScope, LegionToolCallFeedback,
    LegionToolCallFeedbackKind, LegionToolKind, PrincipalId, WorkspaceTrustState,
};
use uuid::Uuid;

use crate::AgentError;
use crate::scope::validate_delegated_task_tool_call;
use crate::worktree::{DelegatedTaskProposalGenerator, DelegatedTaskProposalInput};

// ─── Port traits ──────────────────────────────────────────────────────────────

/// External tool host for commands that run outside the process.
pub trait DelegatedToolHost {
    /// Run a terminal command through the sandboxed spawn.
    fn run_terminal_command(
        &self,
        command: &str,
        workdir: Option<&Path>,
        timeout_seconds: Option<u32>,
    ) -> Result<String, String>;

    /// Forward a call to an MCP tool.
    fn call_mcp_tool(
        &self,
        server_id: &str,
        tool_name: &str,
        arguments: &serde_json::Value,
    ) -> Result<String, String>;
}

/// Sink for audit records emitted by the loop.
pub trait DelegatedTaskAuditSink {
    /// Record a loop step.
    fn record_step(&mut self, step: DelegatedTaskLoopStepRecord);
}

/// Cancellation probe — polled before every model turn and every tool execution.
pub trait DelegatedTaskCancellationProbe {
    /// Returns true if the task has been cancelled.
    fn is_cancelled(&self) -> bool;
}

// ─── Loop config and result ────────────────────────────────────────────────────

/// Configuration for a single delegated task loop run.
pub struct DelegatedTaskLoopConfig {
    /// System prompt for the model.
    pub system_prompt: String,
    /// Initial user message (the task description).
    pub initial_message: String,
    /// Model identifier.
    pub model: String,
    /// Provider identifier.
    pub provider: String,
    /// Budget caps.
    pub budget: DelegatedTaskLoopBudget,
    /// Workspace root (for scope checks against workspace-relative paths).
    pub workspace_root: PathBuf,
    /// Worktree root (where tools operate — may differ from workspace_root).
    pub worktree_root: PathBuf,
    /// Scope for tool call validation.
    pub scope: DelegatedTaskScope,
    /// Explicitly forbidden paths (workspace-relative).
    pub forbidden_paths: Vec<String>,
}

/// Output returned by a single tool executor call.
pub struct ToolExecutionOutput {
    /// Text content to feed back to the model.
    pub content: String,
    /// Proposal built by `execute_edit_as_proposal`, if any.
    pub proposal: Option<AssistedAiEditProposalOutput>,
}

/// Outcome of a delegated task loop run.
#[derive(Debug, Clone)]
pub enum DelegatedTaskLoopResult {
    /// Model finished naturally (EndTurn).
    Completed {
        /// Final text from the model.
        final_message: String,
        /// Edit proposals surfaced by `edit-as-proposal` tool calls during the run.
        ///
        /// Proposals carry `ProposalId(0)` as a placeholder; the caller must
        /// reassign real IDs via the app-side proposal coordinator before
        /// registering them for human review.
        proposals: Vec<AssistedAiEditProposalOutput>,
    },
    /// Budget exhausted.
    BudgetExhausted {
        /// Human-readable reason label.
        reason: String,
    },
    /// Cancelled by the cancellation probe.
    Cancelled,
    /// Model hit max_tokens on every turn.
    MaxTokensExhausted,
    /// A non-retryable scope/policy denial terminated the loop.
    Blocked {
        /// Human-readable reason label.
        reason: String,
    },
}

// ─── Internal helpers ─────────────────────────────────────────────────────────

/// Parse a tool name string into a `LegionToolKind`.
fn parse_tool_kind(name: &str) -> Option<LegionToolKind> {
    match name {
        "read" => Some(LegionToolKind::Read),
        "grep" => Some(LegionToolKind::Grep),
        "glob" => Some(LegionToolKind::Glob),
        "outline" => Some(LegionToolKind::Outline),
        "edit-as-proposal" => Some(LegionToolKind::EditAsProposal),
        "terminal-command" => Some(LegionToolKind::TerminalCommand),
        "mcp-passthrough" => Some(LegionToolKind::McpPassthrough),
        _ => None,
    }
}

/// Build a `ToolDefinition` from a `LegionToolSchemaDefinition`.
fn tool_defs_from_registry() -> Vec<ToolDefinition> {
    legion_protocol::tools::tool_schema_definitions()
        .into_iter()
        .map(|def| ToolDefinition {
            name: def.tool_name,
            description: def.description_label,
            input_schema: def.input_schema,
        })
        .collect()
}

/// Extract all text blocks from a response and join them.
fn extract_text_from_blocks(blocks: &[ToolTurnBlock]) -> String {
    blocks
        .iter()
        .filter_map(|b| {
            if let ToolTurnBlock::Text(t) = b {
                Some(t.as_str())
            } else {
                None
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Require a string field from a JSON input object.
fn require_string_field<'a>(
    input: &'a serde_json::Value,
    field: &str,
    tool: LegionToolKind,
) -> Result<&'a str, LegionToolCallFeedback> {
    input.get(field).and_then(|v| v.as_str()).ok_or_else(|| {
        LegionToolCallFeedback::new(
            tool,
            LegionToolCallFeedbackKind::InvalidArguments,
            format!("required field '{field}' is missing or not a string"),
            None,
        )
    })
}

/// Resolve a tool-supplied path against `worktree_root` so that relative paths
/// (like `"src/main.rs"`) become absolute before being passed to
/// `validate_containment`. Absolute paths are returned as-is.
fn resolve_tool_path(path_str: &str, worktree_root: &Path) -> PathBuf {
    let p = Path::new(path_str);
    if p.is_absolute() {
        p.to_path_buf()
    } else {
        worktree_root.join(path_str)
    }
}

/// Map a validated worktree-relative path back to a workspace-absolute path for
/// scope checking. `worktree_relative` is the path returned by
/// `validate_containment` (i.e. relative to worktree_root).
fn worktree_relative_to_workspace_path(worktree_relative: &Path, workspace_root: &Path) -> PathBuf {
    workspace_root.join(worktree_relative)
}

/// Invoke the capability broker for a single tool call.
///
/// Returns `Ok(())` if the broker grants the capability, or a
/// `LegionToolCallFeedback` with `PolicyDenied` if it denies or errors.
fn check_broker_capability(
    broker: &dyn legion_protocol::CapabilityBrokerPort,
    tool: LegionToolKind,
    loop_correlation_id: u64,
) -> Result<(), LegionToolCallFeedback> {
    let cap_id = format!("delegate.tool.{}", tool.tool_name());
    let request = CapabilityRequest::Request {
        principal_id: PrincipalId("agent.delegated".to_string()),
        capability_id: CapabilityId(cap_id.clone()),
        workspace_trust_state: WorkspaceTrustState::Trusted,
        target_path: None,
        decision_id: None,
        context: CapabilityRequestContext::default(),
        correlation_id: CorrelationId(loop_correlation_id),
    };

    match broker.handle(request) {
        Ok(CapabilityResponse::Decision(d)) if d.granted => Ok(()),
        Ok(CapabilityResponse::Decision(d)) => Err(LegionToolCallFeedback::new(
            tool,
            LegionToolCallFeedbackKind::PolicyDenied,
            d.reason
                .unwrap_or_else(|| format!("{cap_id} denied by capability broker")),
            None,
        )),
        Ok(_) => Err(LegionToolCallFeedback::new(
            tool,
            LegionToolCallFeedbackKind::PolicyDenied,
            format!("{cap_id}: unexpected broker response variant"),
            None,
        )),
        Err(e) => Err(LegionToolCallFeedback::new(
            tool,
            LegionToolCallFeedbackKind::PolicyDenied,
            format!("{cap_id}: broker error: {}", e.message),
            None,
        )),
    }
}

// ─── Per-tool executor functions ──────────────────────────────────────────────

fn execute_read(
    input: &serde_json::Value,
    worktree_root: &Path,
) -> Result<String, LegionToolCallFeedback> {
    let path_str = require_string_field(input, "path", LegionToolKind::Read)?;
    let start_line = input
        .get("start_line")
        .and_then(|v| v.as_u64())
        .map(|v| v as usize);
    let end_line = input
        .get("end_line")
        .and_then(|v| v.as_u64())
        .map(|v| v as usize);
    let max_bytes = input
        .get("max_bytes")
        .and_then(|v| v.as_u64())
        .map(|v| v as usize);

    let resolved = resolve_tool_path(path_str, worktree_root);
    crate::worktree::validate_containment(worktree_root, &resolved).map_err(|e| {
        LegionToolCallFeedback::new(
            LegionToolKind::Read,
            LegionToolCallFeedbackKind::ScopeDenied,
            format!("path containment check failed: {e}"),
            Some(path_str.to_string()),
        )
    })?;

    let abs_path = resolved;
    let content = std::fs::read_to_string(&abs_path).map_err(|e| {
        LegionToolCallFeedback::new(
            LegionToolKind::Read,
            LegionToolCallFeedbackKind::RuntimeFailure,
            format!("failed to read {path_str}: {e}"),
            Some(path_str.to_string()),
        )
    })?;

    // Apply line slicing
    let content = if start_line.is_some() || end_line.is_some() {
        let start = start_line.map(|l| l.saturating_sub(1)).unwrap_or(0);
        let lines: Vec<&str> = content.lines().collect();
        let end = end_line.map(|l| l.min(lines.len())).unwrap_or(lines.len());
        lines.get(start..end).unwrap_or_default().join("\n")
    } else {
        content
    };

    // Apply max_bytes cap
    let content = if let Some(cap) = max_bytes {
        if content.len() > cap {
            let mut end = cap;
            while end > 0 && !content.is_char_boundary(end) {
                end -= 1;
            }
            content[..end].to_string()
        } else {
            content
        }
    } else {
        content
    };

    Ok(content)
}

fn execute_grep(
    input: &serde_json::Value,
    worktree_root: &Path,
) -> Result<String, LegionToolCallFeedback> {
    let pattern = require_string_field(input, "pattern", LegionToolKind::Grep)?;
    let sub_path = input.get("path").and_then(|v| v.as_str());
    let file_glob = input.get("file_glob").and_then(|v| v.as_str());
    let limit = input
        .get("limit")
        .and_then(|v| v.as_u64())
        .map(|v| v as usize)
        .unwrap_or(100);

    let re = regex::Regex::new(pattern).map_err(|e| {
        LegionToolCallFeedback::new(
            LegionToolKind::Grep,
            LegionToolCallFeedbackKind::InvalidArguments,
            format!("invalid regex pattern: {e}"),
            None,
        )
    })?;

    let glob_matcher: Option<globset::GlobSet> = if let Some(g) = file_glob {
        let glob = globset::GlobBuilder::new(g)
            .literal_separator(false)
            .build()
            .map_err(|e| {
                LegionToolCallFeedback::new(
                    LegionToolKind::Grep,
                    LegionToolCallFeedbackKind::InvalidArguments,
                    format!("invalid file_glob: {e}"),
                    None,
                )
            })?;
        let set = globset::GlobSetBuilder::new()
            .add(glob)
            .build()
            .map_err(|e| {
                LegionToolCallFeedback::new(
                    LegionToolKind::Grep,
                    LegionToolCallFeedbackKind::InvalidArguments,
                    format!("failed to build glob set: {e}"),
                    None,
                )
            })?;
        Some(set)
    } else {
        None
    };

    let search_root = if let Some(p) = sub_path {
        let resolved = resolve_tool_path(p, worktree_root);
        crate::worktree::validate_containment(worktree_root, &resolved).map_err(|e| {
            LegionToolCallFeedback::new(
                LegionToolKind::Grep,
                LegionToolCallFeedbackKind::ScopeDenied,
                format!("path containment check failed: {e}"),
                Some(p.to_string()),
            )
        })?;
        resolved
    } else {
        worktree_root.to_path_buf()
    };

    let mut results = Vec::new();
    grep_walk(
        &search_root,
        &search_root,
        &re,
        &glob_matcher,
        &mut results,
        limit,
    );

    if results.is_empty() {
        return Ok("No matches found.".to_string());
    }

    Ok(results.join("\n"))
}

/// Recursive grep walker.
fn grep_walk(
    base: &Path,
    dir: &Path,
    re: &regex::Regex,
    glob_matcher: &Option<globset::GlobSet>,
    results: &mut Vec<String>,
    limit: usize,
) {
    if results.len() >= limit {
        return;
    }
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        if results.len() >= limit {
            return;
        }
        let path = entry.path();
        if path.is_dir() {
            // Skip hidden dirs
            let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if name.starts_with('.') {
                continue;
            }
            grep_walk(base, &path, re, glob_matcher, results, limit);
        } else if path.is_file() {
            // Apply file glob filter
            if let Some(matcher) = glob_matcher {
                let file_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
                if !matcher.is_match(file_name) {
                    continue;
                }
            }
            // Skip binary files (check first bytes)
            if looks_binary(&path) {
                continue;
            }
            if let Ok(content) = std::fs::read_to_string(&path) {
                let rel = path.strip_prefix(base).unwrap_or(&path);
                for (i, line) in content.lines().enumerate() {
                    if results.len() >= limit {
                        return;
                    }
                    if re.is_match(line) {
                        results.push(format!("{}:{}: {}", rel.to_string_lossy(), i + 1, line));
                    }
                }
            }
        }
    }
}

/// Heuristic binary file check — skip if the first 512 bytes contain a NUL.
fn looks_binary(path: &Path) -> bool {
    use std::io::Read;
    let Ok(mut f) = std::fs::File::open(path) else {
        return false;
    };
    let mut buf = [0u8; 512];
    let Ok(n) = f.read(&mut buf) else {
        return false;
    };
    buf[..n].contains(&0u8)
}

fn execute_glob(
    input: &serde_json::Value,
    worktree_root: &Path,
) -> Result<String, LegionToolCallFeedback> {
    let pattern = require_string_field(input, "pattern", LegionToolKind::Glob)?;
    let sub_path = input.get("path").and_then(|v| v.as_str());
    let limit = input
        .get("limit")
        .and_then(|v| v.as_u64())
        .map(|v| v as usize)
        .unwrap_or(100);

    let glob = globset::GlobBuilder::new(pattern)
        .literal_separator(false)
        .build()
        .map_err(|e| {
            LegionToolCallFeedback::new(
                LegionToolKind::Glob,
                LegionToolCallFeedbackKind::InvalidArguments,
                format!("invalid glob pattern: {e}"),
                None,
            )
        })?;
    let matcher = globset::GlobSetBuilder::new()
        .add(glob)
        .build()
        .map_err(|e| {
            LegionToolCallFeedback::new(
                LegionToolKind::Glob,
                LegionToolCallFeedbackKind::InvalidArguments,
                format!("failed to build glob set: {e}"),
                None,
            )
        })?;

    let search_root = if let Some(p) = sub_path {
        let resolved = resolve_tool_path(p, worktree_root);
        crate::worktree::validate_containment(worktree_root, &resolved).map_err(|e| {
            LegionToolCallFeedback::new(
                LegionToolKind::Glob,
                LegionToolCallFeedbackKind::ScopeDenied,
                format!("path containment check failed: {e}"),
                Some(p.to_string()),
            )
        })?;
        resolved
    } else {
        worktree_root.to_path_buf()
    };

    let mut results = Vec::new();
    glob_walk(&search_root, &search_root, &matcher, &mut results, limit);

    if results.is_empty() {
        return Ok("No matching files found.".to_string());
    }

    Ok(results.join("\n"))
}

/// Recursive glob walker.
fn glob_walk(
    base: &Path,
    dir: &Path,
    matcher: &globset::GlobSet,
    results: &mut Vec<String>,
    limit: usize,
) {
    if results.len() >= limit {
        return;
    }
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        if results.len() >= limit {
            return;
        }
        let path = entry.path();
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
        if path.is_dir() {
            if name.starts_with('.') {
                continue;
            }
            glob_walk(base, &path, matcher, results, limit);
        } else if path.is_file() {
            let rel = path.strip_prefix(base).unwrap_or(&path);
            if matcher.is_match(rel) || matcher.is_match(name) {
                results.push(rel.to_string_lossy().into_owned());
            }
        }
    }
}

fn execute_outline(
    input: &serde_json::Value,
    worktree_root: &Path,
) -> Result<String, LegionToolCallFeedback> {
    let path_str = require_string_field(input, "path", LegionToolKind::Outline)?;
    let max_symbols = input
        .get("max_symbols")
        .and_then(|v| v.as_u64())
        .map(|v| v as usize)
        .unwrap_or(100);

    let resolved = resolve_tool_path(path_str, worktree_root);
    crate::worktree::validate_containment(worktree_root, &resolved).map_err(|e| {
        LegionToolCallFeedback::new(
            LegionToolKind::Outline,
            LegionToolCallFeedbackKind::ScopeDenied,
            format!("path containment check failed: {e}"),
            Some(path_str.to_string()),
        )
    })?;

    let abs_path = resolved;
    let content = std::fs::read_to_string(&abs_path).map_err(|e| {
        LegionToolCallFeedback::new(
            LegionToolKind::Outline,
            LegionToolCallFeedbackKind::RuntimeFailure,
            format!("failed to read {path_str}: {e}"),
            Some(path_str.to_string()),
        )
    })?;

    let ext = abs_path.extension().and_then(|e| e.to_str()).unwrap_or("");

    let mut symbols = Vec::new();

    match ext {
        "md" | "markdown" => {
            for (i, line) in content.lines().enumerate() {
                if symbols.len() >= max_symbols {
                    break;
                }
                if line.starts_with('#') {
                    symbols.push(format!("{}:{} {}", path_str, i + 1, line));
                }
            }
        }
        _ => {
            // Rust and generic language support: match declaration lines.
            let decl_patterns = [
                "pub fn ",
                "fn ",
                "pub struct ",
                "struct ",
                "pub enum ",
                "enum ",
                "pub trait ",
                "trait ",
                "pub impl ",
                "impl ",
                "pub mod ",
                "mod ",
                "pub type ",
                "type ",
                "pub const ",
                "const ",
            ];
            for (i, line) in content.lines().enumerate() {
                if symbols.len() >= max_symbols {
                    break;
                }
                let trimmed = line.trim();
                let is_decl = decl_patterns.iter().any(|p| trimmed.starts_with(p));
                if is_decl && !trimmed.starts_with("//") {
                    symbols.push(format!("{}:{} {}", path_str, i + 1, trimmed));
                }
            }
        }
    }

    if symbols.is_empty() {
        return Ok(format!("No symbols found in {path_str}."));
    }

    Ok(symbols.join("\n"))
}

fn execute_edit_as_proposal(
    input: &serde_json::Value,
    worktree_root: &Path,
    loop_correlation_id: u64,
    causality_id: Uuid,
) -> Result<ToolExecutionOutput, LegionToolCallFeedback> {
    let path_str = require_string_field(input, "path", LegionToolKind::EditAsProposal)?;
    let replacement = require_string_field(input, "replacement", LegionToolKind::EditAsProposal)?;
    let proposal_title = input
        .get("proposal_title")
        .and_then(|v| v.as_str())
        .unwrap_or("Delegated task edit proposal");
    let proposal_reason = input
        .get("proposal_reason")
        .and_then(|v| v.as_str())
        .unwrap_or("Generated by delegated task loop");

    let resolved_edit_path = resolve_tool_path(path_str, worktree_root);
    crate::worktree::validate_containment(worktree_root, &resolved_edit_path).map_err(|e| {
        LegionToolCallFeedback::new(
            LegionToolKind::EditAsProposal,
            LegionToolCallFeedbackKind::ScopeDenied,
            format!("path containment check failed: {e}"),
            Some(path_str.to_string()),
        )
    })?;

    // Build the proposal using DelegatedTaskProposalGenerator — zero disk writes.
    let generator = DelegatedTaskProposalGenerator::new(worktree_root.to_path_buf());
    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64;

    let trust_ref = legion_protocol::AssistedAiTrustProjectionReference {
        reference_id: format!("agent.loop.{}", Uuid::new_v4()),
        kind: legion_protocol::AssistedAiTrustProjectionKind::ContextManifest,
        projection_hash: legion_protocol::FileFingerprint {
            algorithm: "none".to_string(),
            value: "0".to_string(),
        },
        schema_version: 1,
    };

    let proposal_input = DelegatedTaskProposalInput {
        target_path: &resolved_edit_path,
        modified_content: replacement,
        output_id: Uuid::new_v4().to_string(),
        request_id: loop_correlation_id.to_string(),
        provider_id: "agent.loop".to_string(),
        proposal_id: legion_protocol::ProposalId(0),
        principal: PrincipalId("agent.delegated".to_string()),
        capability: CapabilityId("delegate.tool.edit-as-proposal".to_string()),
        correlation_id: CorrelationId(loop_correlation_id),
        causality_id: legion_protocol::CausalityId(causality_id),
        created_at: legion_protocol::TimestampMillis(now_ms),
        context_manifest: trust_ref.clone(),
        approval_checklist: trust_ref,
    };

    let proposal = generator.generate_proposal(proposal_input).map_err(|e| {
        LegionToolCallFeedback::new(
            LegionToolKind::EditAsProposal,
            LegionToolCallFeedbackKind::RuntimeFailure,
            format!("proposal generation failed: {e}"),
            Some(path_str.to_string()),
        )
    })?;

    // Build a textual summary for the model — no disk write.
    let payload_variant = match &proposal.payload {
        legion_protocol::ProposalPayload::CreateFile(_) => "CreateFile",
        legion_protocol::ProposalPayload::TextEdit(_) => "TextEdit",
        legion_protocol::ProposalPayload::DeleteFile(_) => "DeleteFile",
        legion_protocol::ProposalPayload::RenameFile(_) => "RenameFile",
        legion_protocol::ProposalPayload::SaveFile(_) => "SaveFile",
        legion_protocol::ProposalPayload::FormatFile(_) => "FormatFile",
        legion_protocol::ProposalPayload::CodeAction(_) => "CodeAction",
        legion_protocol::ProposalPayload::WorkspaceEdit(_) => "WorkspaceEdit",
        legion_protocol::ProposalPayload::TerminalCommand(_) => "TerminalCommand",
        legion_protocol::ProposalPayload::Batch(_) => "Batch",
    };
    let content = format!(
        "Proposal created for {path_str}\nTitle: {proposal_title}\nReason: {proposal_reason}\n\
         Proposal ID: {:?}\nProposal kind: {}\nReplacement ({} bytes) staged for review.",
        proposal.proposal_id,
        payload_variant,
        replacement.len(),
    );
    Ok(ToolExecutionOutput {
        content,
        proposal: Some(proposal),
    })
}

fn execute_terminal_command(
    input: &serde_json::Value,
    worktree_root: &Path,
    tool_host: &dyn DelegatedToolHost,
) -> Result<String, LegionToolCallFeedback> {
    let command = require_string_field(input, "command", LegionToolKind::TerminalCommand)?;
    let workdir = input.get("workdir").and_then(|v| v.as_str());
    let timeout_seconds = input
        .get("timeout_seconds")
        .and_then(|v| v.as_u64())
        .map(|v| v as u32);

    let workdir_path: Option<PathBuf> = if let Some(w) = workdir {
        let resolved_w = resolve_tool_path(w, worktree_root);
        crate::worktree::validate_containment(worktree_root, &resolved_w).map_err(|e| {
            LegionToolCallFeedback::new(
                LegionToolKind::TerminalCommand,
                LegionToolCallFeedbackKind::ScopeDenied,
                format!("workdir containment check failed: {e}"),
                Some(w.to_string()),
            )
        })?;
        Some(resolved_w)
    } else {
        None
    };

    tool_host
        .run_terminal_command(command, workdir_path.as_deref(), timeout_seconds)
        .map_err(|e| {
            LegionToolCallFeedback::new(
                LegionToolKind::TerminalCommand,
                LegionToolCallFeedbackKind::RuntimeFailure,
                format!("terminal command failed: {e}"),
                None,
            )
        })
}

fn execute_mcp_passthrough(
    input: &serde_json::Value,
    tool_host: &dyn DelegatedToolHost,
) -> Result<String, LegionToolCallFeedback> {
    let server_id = require_string_field(input, "server_id", LegionToolKind::McpPassthrough)?;
    let tool_name = require_string_field(input, "tool_name", LegionToolKind::McpPassthrough)?;
    let arguments = input.get("arguments").ok_or_else(|| {
        LegionToolCallFeedback::new(
            LegionToolKind::McpPassthrough,
            LegionToolCallFeedbackKind::InvalidArguments,
            "required field 'arguments' is missing".to_string(),
            None,
        )
    })?;

    tool_host
        .call_mcp_tool(server_id, tool_name, arguments)
        .map_err(|e| {
            LegionToolCallFeedback::new(
                LegionToolKind::McpPassthrough,
                LegionToolCallFeedbackKind::RuntimeFailure,
                format!("MCP tool call failed: {e}"),
                None,
            )
        })
}

// ─── Full validation + execution pipeline ────────────────────────────────────

/// Validate and execute a single tool call.
///
/// Returns `Ok(ToolExecutionOutput)` on success or `Err(feedback)` on rejection.
#[allow(clippy::too_many_arguments)]
fn validate_and_execute(
    config: &DelegatedTaskLoopConfig,
    tool_name: &str,
    input: &serde_json::Value,
    broker: &dyn legion_protocol::CapabilityBrokerPort,
    tool_host: &dyn DelegatedToolHost,
    loop_correlation_id: u64,
    causality_id: Uuid,
) -> Result<ToolExecutionOutput, LegionToolCallFeedback> {
    // Step 1: parse tool kind
    let tool = parse_tool_kind(tool_name).ok_or_else(|| {
        LegionToolCallFeedback::new(
            LegionToolKind::Read, // placeholder for unknown
            LegionToolCallFeedbackKind::UnknownTool,
            format!("unknown tool: {tool_name}"),
            None,
        )
    })?;

    // Step 2: schema validation — check required fields
    for field in tool.required_fields() {
        if input.get(*field).is_none() {
            return Err(LegionToolCallFeedback::new(
                tool,
                LegionToolCallFeedbackKind::InvalidArguments,
                format!("required field '{field}' is missing"),
                None,
            ));
        }
    }

    // Step 3: validate containment + scope for path-bearing tools.
    // Relative paths are resolved against worktree_root before containment check.
    // Containment failures are ScopeDenied (non-retryable) — the model cannot fix
    // a path that escapes the sandbox by adjusting arguments.
    let path_opt: Option<PathBuf> = match tool {
        LegionToolKind::Read | LegionToolKind::Outline | LegionToolKind::EditAsProposal => {
            let path_str = input.get("path").and_then(|v| v.as_str()).ok_or_else(|| {
                LegionToolCallFeedback::new(
                    tool,
                    LegionToolCallFeedbackKind::InvalidArguments,
                    "path field missing".to_string(),
                    None,
                )
            })?;
            let resolved = resolve_tool_path(path_str, &config.worktree_root);
            let relative = crate::worktree::validate_containment(&config.worktree_root, &resolved)
                .map_err(|e| {
                    LegionToolCallFeedback::new(
                        tool,
                        LegionToolCallFeedbackKind::ScopeDenied,
                        format!("containment check failed: {e}"),
                        Some(path_str.to_string()),
                    )
                })?;
            Some(worktree_relative_to_workspace_path(
                &relative,
                &config.workspace_root,
            ))
        }
        LegionToolKind::Grep | LegionToolKind::Glob => {
            if let Some(path_str) = input.get("path").and_then(|v| v.as_str()) {
                let resolved = resolve_tool_path(path_str, &config.worktree_root);
                let relative =
                    crate::worktree::validate_containment(&config.worktree_root, &resolved)
                        .map_err(|e| {
                            LegionToolCallFeedback::new(
                                tool,
                                LegionToolCallFeedbackKind::ScopeDenied,
                                format!("containment check failed: {e}"),
                                Some(path_str.to_string()),
                            )
                        })?;
                Some(worktree_relative_to_workspace_path(
                    &relative,
                    &config.workspace_root,
                ))
            } else {
                None
            }
        }
        LegionToolKind::TerminalCommand | LegionToolKind::McpPassthrough => None,
    };

    // Step 4: scope validation
    validate_delegated_task_tool_call(&config.scope, tool, path_opt.as_deref()).map_err(|e| {
        crate::scope::tool_call_feedback_for_scope_denial(&e).unwrap_or_else(|| {
            LegionToolCallFeedback::new(
                tool,
                LegionToolCallFeedbackKind::ScopeDenied,
                format!("{e}"),
                None,
            )
        })
    })?;

    // Step 5: broker capability check
    check_broker_capability(broker, tool, loop_correlation_id)?;

    // Execute tool — non-proposal tools wrap their String output in ToolExecutionOutput.
    match tool {
        LegionToolKind::Read => execute_read(input, &config.worktree_root)
            .map(|content| ToolExecutionOutput { content, proposal: None }),
        LegionToolKind::Grep => execute_grep(input, &config.worktree_root)
            .map(|content| ToolExecutionOutput { content, proposal: None }),
        LegionToolKind::Glob => execute_glob(input, &config.worktree_root)
            .map(|content| ToolExecutionOutput { content, proposal: None }),
        LegionToolKind::Outline => execute_outline(input, &config.worktree_root)
            .map(|content| ToolExecutionOutput { content, proposal: None }),
        LegionToolKind::EditAsProposal => execute_edit_as_proposal(
            input,
            &config.worktree_root,
            loop_correlation_id,
            causality_id,
        ),
        LegionToolKind::TerminalCommand => {
            execute_terminal_command(input, &config.worktree_root, tool_host)
                .map(|content| ToolExecutionOutput { content, proposal: None })
        }
        LegionToolKind::McpPassthrough => execute_mcp_passthrough(input, tool_host)
            .map(|content| ToolExecutionOutput { content, proposal: None }),
    }
}

// ─── The main loop ─────────────────────────────────────────────────────────────

/// Run the native delegated task execution loop.
///
/// The loop is synchronous — no async or tokio. It processes one model turn
/// at a time, dispatching tool calls sequentially, with a paired audit step
/// (ToolCallRequest before dispatch, ToolCallResult after) sharing a
/// `causality_id` for each tool invocation.
pub fn run_delegated_task_loop(
    config: &DelegatedTaskLoopConfig,
    provider: &dyn ToolCallingProvider,
    tool_host: &dyn DelegatedToolHost,
    audit_sink: &mut dyn DelegatedTaskAuditSink,
    cancellation: &dyn DelegatedTaskCancellationProbe,
    broker: &dyn legion_protocol::CapabilityBrokerPort,
) -> Result<DelegatedTaskLoopResult, AgentError> {
    let run_id = Uuid::new_v4().to_string();
    let correlation_id_uuid = Uuid::new_v4();
    let correlation_id_str = correlation_id_uuid.to_string();
    // Derive a u64 correlation id for the broker calls from the UUID's first 8 bytes.
    let correlation_id_u64 = {
        let b = correlation_id_uuid.as_bytes();
        u64::from_le_bytes([b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7]])
    };

    let tool_defs = tool_defs_from_registry();

    // Initialize conversation with the user's task message.
    let mut turns: Vec<ToolConversationTurn> = vec![ToolConversationTurn {
        role: "user".to_string(),
        blocks: vec![ToolTurnBlock::Text(config.initial_message.clone())],
    }];

    let mut model_turns: u32 = 0;
    let mut tool_calls: u32 = 0;
    let mut consecutive_retries: u32 = 0;
    let mut total_output_bytes: u64 = 0;
    let mut event_seq: u32 = 0;
    let mut step_index: u32 = 0;
    // Proposals accumulate across all model turns; only `Completed` returns them.
    // Blocked/BudgetExhausted/Cancelled discard partial proposals.
    let mut accumulated_proposals: Vec<AssistedAiEditProposalOutput> = Vec::new();

    let start_time = if config.budget.wall_clock_limit_ms > 0 {
        Some(std::time::Instant::now())
    } else {
        None
    };

    loop {
        // Poll cancellation before each model turn.
        if cancellation.is_cancelled() {
            return Ok(DelegatedTaskLoopResult::Cancelled);
        }

        // Check wall clock budget.
        if let Some(start) = start_time
            && start.elapsed()
                >= std::time::Duration::from_millis(config.budget.wall_clock_limit_ms)
        {
            event_seq += 1;
            step_index += 1;
            audit_sink.record_step(DelegatedTaskLoopStepRecord {
                run_id: run_id.clone(),
                step_index,
                kind: DelegatedTaskLoopStepKind::BudgetExhausted,
                correlation_id: correlation_id_str.clone(),
                causality_id: correlation_id_str.clone(),
                event_sequence: event_seq,
                tool_name: None,
                allowed: Some(false),
                reason: Some("wall_clock_limit_ms".to_string()),
            });
            return Ok(DelegatedTaskLoopResult::BudgetExhausted {
                reason: "wall clock limit exceeded".to_string(),
            });
        }

        // Check model turn budget.
        if model_turns >= config.budget.max_model_turns {
            event_seq += 1;
            step_index += 1;
            audit_sink.record_step(DelegatedTaskLoopStepRecord {
                run_id: run_id.clone(),
                step_index,
                kind: DelegatedTaskLoopStepKind::BudgetExhausted,
                correlation_id: correlation_id_str.clone(),
                causality_id: correlation_id_str.clone(),
                event_sequence: event_seq,
                tool_name: None,
                allowed: Some(false),
                reason: Some("max_model_turns".to_string()),
            });
            return Ok(DelegatedTaskLoopResult::BudgetExhausted {
                reason: "max_model_turns exceeded".to_string(),
            });
        }

        // Call the provider.
        let request = ToolCompletionRequest {
            provider: config.provider.clone(),
            model: config.model.clone(),
            system: config.system_prompt.clone(),
            turns: turns.clone(),
            tools: tool_defs.clone(),
            max_tokens: 4096,
        };

        let response = provider.complete_with_tools(request).map_err(|e| {
            AgentError::InvalidMetadata(
                legion_protocol::AssistedAiContractError::InvalidProposalMetadata {
                    reason: format!("provider error: {e}"),
                },
            )
        })?;
        model_turns += 1;
        event_seq += 1;
        step_index += 1;

        // Emit ModelResponse step.
        audit_sink.record_step(DelegatedTaskLoopStepRecord {
            run_id: run_id.clone(),
            step_index,
            kind: DelegatedTaskLoopStepKind::ModelResponse,
            correlation_id: correlation_id_str.clone(),
            causality_id: correlation_id_str.clone(),
            event_sequence: event_seq,
            tool_name: None,
            allowed: None,
            reason: None,
        });

        match response.stop_reason {
            ToolCompletionStopReason::EndTurn => {
                let final_message = extract_text_from_blocks(&response.blocks);
                return Ok(DelegatedTaskLoopResult::Completed {
                    final_message,
                    proposals: accumulated_proposals,
                });
            }
            ToolCompletionStopReason::MaxTokens => {
                return Ok(DelegatedTaskLoopResult::MaxTokensExhausted);
            }
            ToolCompletionStopReason::ToolUse => {
                // Collect tool results for this model turn.
                let mut tool_result_blocks: Vec<ToolTurnBlock> = Vec::new();
                let mut blocked: Option<String> = None;

                for block in &response.blocks {
                    let ToolTurnBlock::ToolUse { id, name, input } = block else {
                        continue;
                    };

                    // Poll cancellation per tool call.
                    if cancellation.is_cancelled() {
                        return Ok(DelegatedTaskLoopResult::Cancelled);
                    }

                    // Check tool call budget.
                    if tool_calls >= config.budget.max_tool_calls {
                        event_seq += 1;
                        step_index += 1;
                        audit_sink.record_step(DelegatedTaskLoopStepRecord {
                            run_id: run_id.clone(),
                            step_index,
                            kind: DelegatedTaskLoopStepKind::BudgetExhausted,
                            correlation_id: correlation_id_str.clone(),
                            causality_id: correlation_id_str.clone(),
                            event_sequence: event_seq,
                            tool_name: Some(name.clone()),
                            allowed: Some(false),
                            reason: Some("max_tool_calls".to_string()),
                        });
                        return Ok(DelegatedTaskLoopResult::BudgetExhausted {
                            reason: "max_tool_calls exceeded".to_string(),
                        });
                    }

                    // Assign causality_id for this request+result pair.
                    let causality_uuid = Uuid::new_v4();
                    let causality_id_str = causality_uuid.to_string();
                    event_seq += 1;
                    step_index += 1;

                    // Emit ToolCallRequest BEFORE dispatch.
                    audit_sink.record_step(DelegatedTaskLoopStepRecord {
                        run_id: run_id.clone(),
                        step_index,
                        kind: DelegatedTaskLoopStepKind::ToolCallRequest,
                        correlation_id: correlation_id_str.clone(),
                        causality_id: causality_id_str.clone(),
                        event_sequence: event_seq,
                        tool_name: Some(name.clone()),
                        allowed: None,
                        reason: None,
                    });

                    // Validate + execute the tool.
                    match validate_and_execute(
                        config,
                        name,
                        input,
                        broker,
                        tool_host,
                        correlation_id_u64,
                        causality_uuid,
                    ) {
                        Ok(ToolExecutionOutput {
                            content: raw_output,
                            proposal,
                        }) => {
                            // Accumulate any proposal produced by edit-as-proposal.
                            if let Some(p) = proposal {
                                accumulated_proposals.push(p);
                            }
                            // Apply redaction and per-call byte cap.
                            let bound = redact_model_bound_output(
                                &raw_output,
                                config.budget.max_tool_output_bytes as usize,
                            );
                            total_output_bytes =
                                total_output_bytes.saturating_add(bound.byte_count);

                            // Check total output budget.
                            if total_output_bytes > config.budget.max_total_tool_output_bytes {
                                // Pair the ToolCallRequest: emit ToolCallResult before BudgetExhausted.
                                // The tool executed successfully; the cumulative output ceiling was
                                // hit after the fact.
                                event_seq += 1;
                                step_index += 1;
                                audit_sink.record_step(DelegatedTaskLoopStepRecord {
                                    run_id: run_id.clone(),
                                    step_index,
                                    kind: DelegatedTaskLoopStepKind::ToolCallResult,
                                    correlation_id: correlation_id_str.clone(),
                                    causality_id: causality_id_str.clone(),
                                    event_sequence: event_seq,
                                    tool_name: Some(name.clone()),
                                    allowed: Some(true),
                                    reason: Some(
                                        "output produced; max_total_tool_output_bytes budget exhausted"
                                            .to_string(),
                                    ),
                                });
                                // BudgetExhausted is a loop-level event — use a fresh causality_id.
                                event_seq += 1;
                                step_index += 1;
                                audit_sink.record_step(DelegatedTaskLoopStepRecord {
                                    run_id: run_id.clone(),
                                    step_index,
                                    kind: DelegatedTaskLoopStepKind::BudgetExhausted,
                                    correlation_id: correlation_id_str.clone(),
                                    causality_id: Uuid::new_v4().to_string(),
                                    event_sequence: event_seq,
                                    tool_name: Some(name.clone()),
                                    allowed: Some(false),
                                    reason: Some("max_total_tool_output_bytes".to_string()),
                                });
                                return Ok(DelegatedTaskLoopResult::BudgetExhausted {
                                    reason: "max_total_tool_output_bytes exceeded".to_string(),
                                });
                            }

                            consecutive_retries = 0;
                            tool_calls += 1;
                            event_seq += 1;
                            step_index += 1;

                            // Emit ToolCallResult AFTER dispatch.
                            audit_sink.record_step(DelegatedTaskLoopStepRecord {
                                run_id: run_id.clone(),
                                step_index,
                                kind: DelegatedTaskLoopStepKind::ToolCallResult,
                                correlation_id: correlation_id_str.clone(),
                                causality_id: causality_id_str.clone(),
                                event_sequence: event_seq,
                                tool_name: Some(name.clone()),
                                allowed: Some(true),
                                reason: None,
                            });

                            tool_result_blocks.push(ToolTurnBlock::ToolResult {
                                tool_use_id: id.clone(),
                                content: bound.redacted_text,
                                is_error: false,
                            });
                        }
                        Err(feedback) => {
                            let retryable = feedback.retryable;
                            let reason = feedback.detail_label.clone();
                            let tool_name_str = feedback.tool.tool_name().to_string();

                            event_seq += 1;
                            step_index += 1;

                            // Emit ToolCallRejected.
                            audit_sink.record_step(DelegatedTaskLoopStepRecord {
                                run_id: run_id.clone(),
                                step_index,
                                kind: DelegatedTaskLoopStepKind::ToolCallRejected,
                                correlation_id: correlation_id_str.clone(),
                                causality_id: causality_id_str.clone(),
                                event_sequence: event_seq,
                                tool_name: Some(tool_name_str),
                                allowed: Some(false),
                                reason: Some(reason.clone()),
                            });

                            if retryable {
                                consecutive_retries += 1;
                                if consecutive_retries > config.budget.max_consecutive_retries {
                                    event_seq += 1;
                                    step_index += 1;
                                    audit_sink.record_step(DelegatedTaskLoopStepRecord {
                                        run_id: run_id.clone(),
                                        step_index,
                                        kind: DelegatedTaskLoopStepKind::BudgetExhausted,
                                        correlation_id: correlation_id_str.clone(),
                                        causality_id: correlation_id_str.clone(),
                                        event_sequence: event_seq,
                                        tool_name: None,
                                        allowed: Some(false),
                                        reason: Some("max_consecutive_retries".to_string()),
                                    });
                                    return Ok(DelegatedTaskLoopResult::BudgetExhausted {
                                        reason: format!(
                                            "max_consecutive_retries exceeded: {}",
                                            reason
                                        ),
                                    });
                                }
                                // Feed error back to model as a ToolResult.
                                let feedback_content = serde_json::to_string(&feedback)
                                    .unwrap_or_else(|_| reason.clone());
                                tool_result_blocks.push(ToolTurnBlock::ToolResult {
                                    tool_use_id: id.clone(),
                                    content: feedback_content,
                                    is_error: true,
                                });
                            } else {
                                // Non-retryable: terminate loop.
                                blocked = Some(reason);
                                break;
                            }
                        }
                    }
                }

                if let Some(reason) = blocked {
                    return Ok(DelegatedTaskLoopResult::Blocked { reason });
                }

                // Append the assistant's turn to conversation history.
                turns.push(ToolConversationTurn {
                    role: "assistant".to_string(),
                    blocks: response.blocks,
                });

                // Append all tool results as a user turn.
                if !tool_result_blocks.is_empty() {
                    turns.push(ToolConversationTurn {
                        role: "user".to_string(),
                        blocks: tool_result_blocks,
                    });
                }
                // Loop continues to next model turn.
            }
        }
    }
}
