//! The command line: a typed string in, one dispatch intent out.
//!
//! Extracted from `ui.rs` as a pure move. `ui.rs` is a chokepoint file that
//! `xtask extract-before-modify` watches, and this is the region that grows
//! every time a feature gains a command — three separate workstreams added to
//! it in one day and together pushed the file past its slack.
//!
//! The `Shell` holds no authority. Everything here parses text and returns an
//! intent; nothing it produces has touched a buffer, a file or a remote.

use crate::ui::*;
// Tuple-struct constructors (`ProposalId`, `PluginId`, …) are values, not
// types, so the glob above does not bring them along.
use legion_protocol::*;

impl Shell {
    /// Parse a command and emit a typed dispatch intent without mutating editor or workspace state.
    pub fn handle_command(
        &mut self,
        input: &str,
    ) -> Result<Option<CommandDispatchIntent>, ShellCommandError> {
        let trimmed = input.trim();
        if trimmed == ":q" {
            return Ok(Some(self.push_intent(CommandDispatchIntent::Quit)));
        }
        if let Some(payload) = trimmed.strip_prefix(":mode") {
            return Ok(Some(self.push_intent(
                CommandDispatchIntent::SetProductMode {
                    mode: parse_dock_mode(payload.trim()),
                },
            )));
        }
        if trimmed == ":u" {
            let buffer_id = self.active_buffer_id()?;
            return Ok(Some(
                self.push_intent(CommandDispatchIntent::Undo { buffer_id }),
            ));
        }
        if trimmed == ":redo" {
            let buffer_id = self.active_buffer_id()?;
            return Ok(Some(
                self.push_intent(CommandDispatchIntent::Redo { buffer_id }),
            ));
        }
        if trimmed == ":w" {
            let buffer_id = self.active_buffer_id()?;
            return Ok(Some(
                self.push_intent(CommandDispatchIntent::Save { buffer_id }),
            ));
        }
        if trimmed == ":wa" {
            return Ok(Some(self.push_intent(CommandDispatchIntent::SaveAll)));
        }
        if let Some(payload) = trimmed.strip_prefix(":assist-predict") {
            let buffer_id = self.active_buffer_id()?;
            let position = self.command_position(payload.trim())?;
            return Ok(Some(self.push_intent(
                CommandDispatchIntent::RequestAssistInlinePrediction {
                    buffer_id,
                    position,
                },
            )));
        }
        if trimmed == ":tab" || trimmed == ":assist-accept" {
            let buffer_id = self.active_buffer_id()?;
            let prediction_id = self.active_assist_prediction_id();
            return Ok(Some(self.push_intent(
                CommandDispatchIntent::AcceptAssistInlinePrediction {
                    buffer_id,
                    prediction_id,
                },
            )));
        }
        if trimmed == ":assist-dismiss" {
            let buffer_id = self.active_buffer_id()?;
            let prediction_id = self.active_assist_prediction_id();
            return Ok(Some(self.push_intent(
                CommandDispatchIntent::DismissAssistInlinePrediction {
                    buffer_id,
                    prediction_id,
                },
            )));
        }
        if trimmed == ":assist-cancel" {
            let buffer_id = self.active_buffer_id()?;
            let prediction_id = self.active_assist_prediction_id();
            return Ok(Some(self.push_intent(
                CommandDispatchIntent::CancelAssistInlinePrediction {
                    buffer_id,
                    prediction_id,
                },
            )));
        }
        if let Some(buffer_id) = parse_buffer_id(trimmed.strip_prefix(":tab ")) {
            return Ok(Some(
                self.push_intent(CommandDispatchIntent::SwitchTab { buffer_id }),
            ));
        }
        if let Some(buffer_id) = parse_buffer_id(trimmed.strip_prefix(":close ")) {
            return Ok(Some(
                self.push_intent(CommandDispatchIntent::CloseTab { buffer_id }),
            ));
        }
        if let Some(item_id) = trimmed.strip_prefix(":context-manifest-select ") {
            let item_id = item_id.trim();
            if self
                .context_manifest_projection
                .manifest
                .items
                .iter()
                .any(|item| item.item_id == item_id)
            {
                self.context_manifest_projection.selected_item_id = Some(item_id.to_string());
                return Ok(None);
            }
            return Err(ShellCommandError::ContextManifestItemMissing);
        }
        if trimmed == ":context-manifest-clear" || trimmed == ":context-manifest-clear-selection" {
            self.context_manifest_projection.selected_item_id = None;
            return Ok(None);
        }
        if let Some(query) = trimmed.strip_prefix(":search ") {
            return Ok(Some(self.push_intent(CommandDispatchIntent::RunSearch {
                scope: SearchScopeProjection::ActiveFile,
                query: query.trim().to_string(),
                limit: 0,
                case_sensitive: None,
                whole_word: None,
                use_regex: None,
            })));
        }
        if let Some(query) = trimmed.strip_prefix(":search-workspace ") {
            return Ok(Some(self.push_intent(CommandDispatchIntent::RunSearch {
                scope: SearchScopeProjection::Workspace,
                query: query.trim().to_string(),
                limit: 0,
                case_sensitive: None,
                whole_word: None,
                use_regex: None,
            })));
        }
        if let Some(query_id) = trimmed.strip_prefix(":search-cancel ") {
            return Ok(Some(self.push_intent(
                CommandDispatchIntent::CancelSearch {
                    query_id: query_id.trim().to_string(),
                },
            )));
        }
        if let Some(payload) = trimmed.strip_prefix(":hover") {
            let buffer_id = self.active_buffer_id()?;
            let position = self.command_position(payload.trim())?;
            return Ok(Some(self.push_intent(
                CommandDispatchIntent::RequestHover {
                    buffer_id,
                    position,
                },
            )));
        }
        if let Some(payload) = trimmed.strip_prefix(":completion") {
            let buffer_id = self.active_buffer_id()?;
            let position = self.command_position(payload.trim())?;
            return Ok(Some(self.push_intent(
                CommandDispatchIntent::RequestCompletion {
                    buffer_id,
                    position,
                },
            )));
        }
        if let Some(payload) = trimmed.strip_prefix(":definition") {
            let buffer_id = self.active_buffer_id()?;
            let position = self.command_position(payload.trim())?;
            return Ok(Some(self.push_intent(
                CommandDispatchIntent::GoToDefinition {
                    buffer_id,
                    position,
                },
            )));
        }
        if let Some(payload) = trimmed.strip_prefix(":references") {
            let buffer_id = self.active_buffer_id()?;
            let position = self.command_position(payload.trim())?;
            return Ok(Some(self.push_intent(
                CommandDispatchIntent::FindReferences {
                    buffer_id,
                    position,
                },
            )));
        }
        if trimmed == ":outline" {
            let buffer_id = self.active_buffer_id()?;
            return Ok(Some(
                self.push_intent(CommandDispatchIntent::RefreshOutline { buffer_id }),
            ));
        }
        if trimmed == ":inlayhints" {
            let buffer_id = self.active_buffer_id()?;
            return Ok(Some(self.push_intent(
                CommandDispatchIntent::RefreshInlayHints { buffer_id },
            )));
        }
        if trimmed == ":codelens" {
            let buffer_id = self.active_buffer_id()?;
            return Ok(Some(self.push_intent(
                CommandDispatchIntent::RefreshCodeLenses { buffer_id },
            )));
        }
        if trimmed == ":format" {
            let buffer_id = self.active_buffer_id()?;
            return Ok(Some(self.push_intent(
                CommandDispatchIntent::RequestFormattingProposal { buffer_id },
            )));
        }
        if let Some(payload) = trimmed.strip_prefix(":rename ") {
            let buffer_id = self.active_buffer_id()?;
            let mut split = payload.splitn(2, ',');
            let first = split.next().unwrap_or_default().trim();
            let (position, new_name) = if let Some(name) = split.next() {
                let offset = first
                    .parse::<usize>()
                    .map_err(|_| ShellCommandError::InvalidPosition)?;
                (self.parse_pos(offset)?, name.trim())
            } else {
                (self.parse_pos(0)?, first)
            };
            return Ok(Some(self.push_intent(
                CommandDispatchIntent::RequestRenameProposal {
                    buffer_id,
                    position,
                    new_name: new_name.to_string(),
                },
            )));
        }
        if trimmed == ":organize-imports" {
            let buffer_id = self.active_buffer_id()?;
            return Ok(Some(self.push_intent(
                CommandDispatchIntent::RequestOrganizeImportsProposal { buffer_id },
            )));
        }
        if let Some(action_id) = trimmed.strip_prefix(":code-action ") {
            let buffer_id = self.active_buffer_id()?;
            return Ok(Some(self.push_intent(
                CommandDispatchIntent::RequestCodeActionProposal {
                    buffer_id,
                    action_id: action_id.trim().to_string(),
                },
            )));
        }
        if let Some(operation_id) = trimmed.strip_prefix(":language-cancel ") {
            return Ok(Some(self.push_intent(
                CommandDispatchIntent::CancelLanguageOperation {
                    operation_id: operation_id.trim().to_string(),
                },
            )));
        }
        if trimmed == ":git-refresh" {
            return Ok(Some(self.push_intent(CommandDispatchIntent::RefreshGit)));
        }
        if trimmed == ":test-refresh" || trimmed == ":tests-refresh" {
            return Ok(Some(
                self.push_intent(CommandDispatchIntent::RefreshTestExplorer),
            ));
        }
        if let Some(item_id) = trimmed.strip_prefix(":test-run ") {
            let item_id = item_id.trim();
            if !item_id.is_empty() {
                return Ok(Some(self.push_intent(
                    CommandDispatchIntent::RunTestExplorerItem {
                        item_id: item_id.to_string(),
                    },
                )));
            }
        }
        if let Some(parent) = trimmed.strip_prefix(":test-run-group ") {
            let parent = parent.trim();
            if !parent.is_empty() {
                return Ok(Some(self.push_intent(
                    CommandDispatchIntent::RunTestExplorerGroup {
                        parent_label: parent.to_string(),
                    },
                )));
            }
        }
        if let Some(session_id) = trimmed.strip_prefix(":test-attach-evidence ") {
            let session_id = session_id.trim();
            if !session_id.is_empty() {
                return Ok(Some(self.push_intent(
                    CommandDispatchIntent::AttachTestExplorerEvidence {
                        session_id: session_id.to_string(),
                    },
                )));
            }
        }
        if let Some(host) = trimmed.strip_prefix(":git-allow-remote ") {
            return Ok(Some(self.push_intent(
                CommandDispatchIntent::GrantGitRemoteHost {
                    host: host.trim().to_string(),
                },
            )));
        }
        if let Some(host) = trimmed.strip_prefix(":git-revoke-remote ") {
            return Ok(Some(self.push_intent(
                CommandDispatchIntent::RevokeGitRemoteHost {
                    host: host.trim().to_string(),
                },
            )));
        }
        if let Some(branch) = trimmed.strip_prefix(":git-switch-branch ") {
            return Ok(Some(self.push_intent(
                CommandDispatchIntent::SwitchGitBranch {
                    branch: branch.trim().to_string(),
                },
            )));
        }
        if let Some(branch) = trimmed.strip_prefix(":git-create-branch ") {
            return Ok(Some(self.push_intent(
                CommandDispatchIntent::CreateGitBranch {
                    branch: branch.trim().to_string(),
                },
            )));
        }
        if let Some(branch) = trimmed.strip_prefix(":git-delete-branch ") {
            return Ok(Some(self.push_intent(
                CommandDispatchIntent::DeleteGitBranch {
                    branch: branch.trim().to_string(),
                },
            )));
        }
        if let Some(message) = trimmed.strip_prefix(":git-stash ") {
            let message = message.trim();
            return Ok(Some(self.push_intent(
                CommandDispatchIntent::StashGitChanges {
                    message: (!message.is_empty()).then(|| message.to_string()),
                },
            )));
        }
        if trimmed == ":git-push" {
            return Ok(Some(self.push_intent(
                CommandDispatchIntent::PushGitRemote {
                    remote: "origin".to_string(),
                },
            )));
        }
        if trimmed == ":git-fetch" {
            return Ok(Some(self.push_intent(
                CommandDispatchIntent::FetchGitRemote {
                    remote: "origin".to_string(),
                },
            )));
        }
        if trimmed == ":git-pull" {
            return Ok(Some(self.push_intent(
                CommandDispatchIntent::PullGitRemote {
                    remote: "origin".to_string(),
                },
            )));
        }
        if trimmed == ":git-prune-worktrees" {
            return Ok(Some(
                self.push_intent(CommandDispatchIntent::PruneGitWorktrees),
            ));
        }
        if let Some(path) = trimmed.strip_prefix(":git-remove-worktree ") {
            return Ok(Some(self.push_intent(
                CommandDispatchIntent::RemoveGitWorktree {
                    path: path.trim().to_string(),
                },
            )));
        }
        if let Some(hunk_id) = trimmed.strip_prefix(":git-stage-hunk ") {
            return Ok(Some(self.push_intent(
                CommandDispatchIntent::StageGitHunk {
                    hunk_id: hunk_id.trim().to_string(),
                },
            )));
        }
        if let Some(hunk_id) = trimmed.strip_prefix(":git-unstage-hunk ") {
            return Ok(Some(self.push_intent(
                CommandDispatchIntent::UnstageGitHunk {
                    hunk_id: hunk_id.trim().to_string(),
                },
            )));
        }
        if let Some(path) = trimmed.strip_prefix(":git-accept-current-conflict ") {
            return Ok(Some(self.push_intent(
                CommandDispatchIntent::ResolveGitConflict {
                    path: path.trim().to_string(),
                    choice: GitConflictChoiceProjection::AcceptCurrent,
                },
            )));
        }
        if let Some(path) = trimmed.strip_prefix(":git-accept-incoming-conflict ") {
            return Ok(Some(self.push_intent(
                CommandDispatchIntent::ResolveGitConflict {
                    path: path.trim().to_string(),
                    choice: GitConflictChoiceProjection::AcceptIncoming,
                },
            )));
        }
        if trimmed == ":git-nav-next-hunk" {
            return Ok(Some(
                self.push_intent(CommandDispatchIntent::GitNavNextHunk),
            ));
        }
        if trimmed == ":git-nav-prev-hunk" {
            return Ok(Some(
                self.push_intent(CommandDispatchIntent::GitNavPrevHunk),
            ));
        }
        if trimmed == ":git-nav-next-file" {
            return Ok(Some(
                self.push_intent(CommandDispatchIntent::GitNavNextFile),
            ));
        }
        if trimmed == ":git-nav-prev-file" {
            return Ok(Some(
                self.push_intent(CommandDispatchIntent::GitNavPrevFile),
            ));
        }
        if let Some(rest) = trimmed.strip_prefix(":git-new-worktree ") {
            let parts: Vec<&str> = rest.trim().splitn(2, ' ').collect();
            if parts.len() == 2 {
                return Ok(Some(self.push_intent(
                    CommandDispatchIntent::CreateGitWorktree {
                        branch: parts[0].to_string(),
                        worktree_path: parts[1].to_string(),
                    },
                )));
            }
        }
        if let Some(path) = trimmed.strip_prefix(":git-local-history ") {
            return Ok(Some(self.push_intent(
                CommandDispatchIntent::RequestLocalHistoryEntries {
                    path: path.trim().to_string(),
                },
            )));
        }
        if let Some(rest) = trimmed.strip_prefix(":git-restore-history ") {
            let parts: Vec<&str> = rest.trim().splitn(2, ' ').collect();
            if parts.len() == 2 {
                return Ok(Some(self.push_intent(
                    CommandDispatchIntent::RestoreFromLocalHistory {
                        path: parts[0].to_string(),
                        entry_id: parts[1].to_string(),
                    },
                )));
            }
        }
        if trimmed == ":git-export-evidence" {
            return Ok(Some(
                self.push_intent(CommandDispatchIntent::ExportWorktreeEvidence),
            ));
        }
        if let Some(msg) = trimmed.strip_prefix(":git-validate-commit ") {
            return Ok(Some(self.push_intent(
                CommandDispatchIntent::ValidateGitCommitMessage {
                    message: msg.to_string(),
                },
            )));
        }
        if trimmed == ":debug-configs" {
            return Ok(Some(
                self.push_intent(CommandDispatchIntent::RefreshDebugConfigurations),
            ));
        }
        if let Some(configuration_id) = trimmed.strip_prefix(":debug-launch ") {
            return Ok(Some(self.push_intent(
                CommandDispatchIntent::LaunchDebugSession {
                    configuration_id: DebugConfigurationId(configuration_id.trim().to_string()),
                },
            )));
        }
        if let Some(payload) = trimmed.strip_prefix(":debug-breakpoint ") {
            let buffer_id = self.active_buffer_id()?;
            let mut parts = payload.splitn(4, ',');
            let line = parts
                .next()
                .and_then(|value| value.trim().parse::<u32>().ok())
                .unwrap_or(0);
            let condition = non_empty_string(parts.next().map(str::trim));
            let hit_condition = non_empty_string(parts.next().map(str::trim));
            let log_message = non_empty_string(parts.next().map(str::trim));
            return Ok(Some(self.push_intent(
                CommandDispatchIntent::ToggleDebugBreakpoint {
                    buffer_id,
                    line,
                    condition,
                    hit_condition,
                    log_message,
                },
            )));
        }
        if let Some(kind) = trimmed.strip_prefix(":debug-step ") {
            let session_id = self.active_debug_session_id()?;
            return Ok(Some(self.push_intent(CommandDispatchIntent::DebugStep {
                session_id,
                kind: parse_debug_step_kind(kind.trim()),
            })));
        }
        if let Some(payload) = trimmed.strip_prefix(":debug-run-to-cursor ") {
            let session_id = self.active_debug_session_id()?;
            let buffer_id = self.active_buffer_id()?;
            let position = self.command_position(payload.trim())?;
            return Ok(Some(self.push_intent(
                CommandDispatchIntent::DebugRunToCursor {
                    session_id,
                    buffer_id,
                    position,
                },
            )));
        }
        if let Some(expression_label) = trimmed.strip_prefix(":debug-eval ") {
            let session_id = self.active_debug_session_id()?;
            return Ok(Some(self.push_intent(
                CommandDispatchIntent::DebugEvaluateSelection {
                    session_id,
                    expression_label: expression_label.trim().to_string(),
                },
            )));
        }
        if let Some(expression_label) = trimmed.strip_prefix(":debug-watch ") {
            let session_id = self.active_debug_session_id()?;
            return Ok(Some(self.push_intent(
                CommandDispatchIntent::DebugAddWatch {
                    session_id,
                    expression_label: expression_label.trim().to_string(),
                },
            )));
        }
        if matches!(trimmed, ":debug-stop" | ":debug-disconnect" | ":debug-quit") {
            let session_id = self.active_debug_session_id()?;
            return Ok(Some(self.push_intent(
                CommandDispatchIntent::StopDebugSession { session_id },
            )));
        }
        if matches!(trimmed, ":debug-poll" | ":debug-poll-stop") {
            let session_id = self.active_debug_session_id()?;
            return Ok(Some(self.push_intent(
                CommandDispatchIntent::PollDebugSession { session_id },
            )));
        }
        if let Some(command_label) = trimmed.strip_prefix(":term-launch ") {
            return Ok(Some(self.push_intent(
                CommandDispatchIntent::TerminalLaunch {
                    command_label: command_label.trim().to_string(),
                    timeout_secs: None,
                },
            )));
        }
        if let Some(payload) = trimmed.strip_prefix(":term-input ") {
            let session_id = self.active_terminal_session_id()?;
            return Ok(Some(self.push_intent(
                CommandDispatchIntent::TerminalInput {
                    session_id,
                    payload: payload.to_string(),
                },
            )));
        }
        if let Some(payload) = trimmed.strip_prefix(":term-resize ") {
            let session_id = self.active_terminal_session_id()?;
            let mut split = payload.split_whitespace();
            let cols = split
                .next()
                .and_then(|value| value.parse::<u16>().ok())
                .unwrap_or(80);
            let rows = split
                .next()
                .and_then(|value| value.parse::<u16>().ok())
                .unwrap_or(24);
            return Ok(Some(self.push_intent(
                CommandDispatchIntent::TerminalResize {
                    session_id,
                    cols,
                    rows,
                },
            )));
        }
        if trimmed == ":term-kill" {
            let session_id = self.active_terminal_session_id()?;
            return Ok(Some(
                self.push_intent(CommandDispatchIntent::TerminalKill { session_id }),
            ));
        }
        if trimmed == ":term-close" {
            let session_id = self.active_terminal_session_id()?;
            return Ok(Some(
                self.push_intent(CommandDispatchIntent::TerminalClose { session_id }),
            ));
        }
        if trimmed == ":term-poll" {
            let session_id = self.active_terminal_session_id()?;
            return Ok(Some(self.push_intent(
                CommandDispatchIntent::TerminalOutputPoll { session_id },
            )));
        }
        if let Some(query) = trimmed.strip_prefix(":term-search ") {
            let session_id = self.active_terminal_session_id()?;
            return Ok(Some(self.push_intent(
                CommandDispatchIntent::TerminalSearch {
                    session_id,
                    query: query.trim().to_string(),
                },
            )));
        }

        if let Some(label) = trimmed.strip_prefix(":ai-start") {
            let instruction_label = label.trim();
            return Ok(Some(self.push_intent(CommandDispatchIntent::StartAiRun {
                instruction_label: if instruction_label.is_empty() {
                    "phase4.local_proposal".to_string()
                } else {
                    instruction_label.to_string()
                },
            })));
        }
        if let Some(label) = trimmed.strip_prefix(":ai-explain") {
            let instruction_label = label.trim();
            return Ok(Some(self.push_intent(
                CommandDispatchIntent::StartAiExplain {
                    instruction_label: if instruction_label.is_empty() {
                        "phase5.local_explain".to_string()
                    } else {
                        instruction_label.to_string()
                    },
                },
            )));
        }
        if let Some(label) = trimmed.strip_prefix(":ai-propose") {
            let instruction_label = label.trim();
            return Ok(Some(self.push_intent(
                CommandDispatchIntent::StartAiProposal {
                    instruction_label: if instruction_label.is_empty() {
                        "phase5.local_proposal".to_string()
                    } else {
                        instruction_label.to_string()
                    },
                    selection: None,
                },
            )));
        }
        if let Some(prompt) = trimmed.strip_prefix(":delegate-chat") {
            let prompt_label = prompt.trim();
            return Ok(Some(self.push_intent(
                CommandDispatchIntent::SendDelegateChat {
                    prompt_label: if prompt_label.is_empty() {
                        "delegate.context".to_string()
                    } else {
                        prompt_label.to_string()
                    },
                },
            )));
        }
        if let Some(payload) = trimmed.strip_prefix(":delegate-hunk ") {
            let mut split = payload.splitn(3, ' ');
            let proposal_id = split
                .next()
                .and_then(|value| value.parse::<u64>().ok())
                .map(ProposalId);
            let hunk_id = split.next().unwrap_or_default().trim();
            let disposition = parse_delegate_hunk_disposition(split.next().unwrap_or_default());
            if let (Some(proposal_id), Some(disposition)) = (proposal_id, disposition)
                && !hunk_id.is_empty()
            {
                return Ok(Some(self.push_intent(
                    CommandDispatchIntent::ReviewDelegateProposalHunk {
                        proposal_id,
                        hunk_id: hunk_id.to_string(),
                        disposition,
                    },
                )));
            }
        }
        if let Some(payload) = trimmed.strip_prefix(":delegate-permission ") {
            let mut split = payload.splitn(2, ' ');
            let request_id = split.next().unwrap_or_default().trim();
            let decision =
                parse_delegate_tool_permission_decision(split.next().unwrap_or_default());
            if !request_id.is_empty()
                && let Some(decision) = decision
            {
                return Ok(Some(self.push_intent(
                    CommandDispatchIntent::RecordDelegateToolPermission {
                        request_id: request_id.to_string(),
                        decision,
                    },
                )));
            }
        }
        if let Some(run_id) = trimmed.strip_prefix(":ai-cancel ") {
            return Ok(Some(self.push_intent(CommandDispatchIntent::CancelAiRun {
                run_id: AgentRunId(run_id.trim().to_string()),
            })));
        }
        if let Some(run_id) = trimmed.strip_prefix(":ai-replay ") {
            return Ok(Some(self.push_intent(CommandDispatchIntent::ReplayAiRun {
                run_id: AgentRunId(run_id.trim().to_string()),
            })));
        }
        if let Some(run_id) = trimmed.strip_prefix(":ai-inspect ") {
            return Ok(Some(self.push_intent(
                CommandDispatchIntent::InspectAiRun {
                    run_id: AgentRunId(run_id.trim().to_string()),
                },
            )));
        }

        if let Some(payload) = trimmed.strip_prefix(":plugin ") {
            let mut split = payload.splitn(3, ' ');
            let plugin_id = split
                .next()
                .and_then(|value| value.parse::<u64>().ok())
                .map(PluginId);
            let command_id = split.next().unwrap_or_default().trim();
            let metadata_label = split.next().unwrap_or(command_id).trim();
            if let Some(plugin_id) = plugin_id
                && plugin_id.0 != 0
                && !command_id.is_empty()
            {
                return Ok(Some(self.push_intent(
                    CommandDispatchIntent::InvokePluginCommand {
                        plugin_id,
                        command_id: command_id.to_string(),
                        metadata_label: if metadata_label.is_empty() {
                            command_id.to_string()
                        } else {
                            metadata_label.to_string()
                        },
                    },
                )));
            }
        }

        if let Some(session_id) =
            parse_collaboration_session_id(trimmed.strip_prefix(":collab-join "))
        {
            return Ok(Some(self.push_intent(
                CommandDispatchIntent::JoinCollaborationSession { session_id },
            )));
        }
        if let Some(session_id) =
            parse_collaboration_session_id(trimmed.strip_prefix(":collab-leave "))
        {
            return Ok(Some(self.push_intent(
                CommandDispatchIntent::LeaveCollaborationSession { session_id },
            )));
        }
        if let Some(payload) = trimmed.strip_prefix(":collab-presence ") {
            let mut split = payload.split_whitespace();
            let session_id = split
                .next()
                .and_then(|value| value.parse::<u128>().ok())
                .map(CollaborationSessionId);
            let participant_id = split
                .next()
                .and_then(|value| value.parse::<u128>().ok())
                .map(CollaborationParticipantId);
            if let (Some(session_id), Some(participant_id)) = (session_id, participant_id)
                && session_id.0 != 0
                && participant_id.0 != 0
            {
                return Ok(Some(self.push_intent(
                    CommandDispatchIntent::PublishCollaborationPresence {
                        session_id,
                        participant_id,
                    },
                )));
            }
        }

        if let Some(proposal_id) = parse_proposal_id(trimmed.strip_prefix(":proposal-preview ")) {
            return Ok(Some(self.push_intent(
                CommandDispatchIntent::PreviewProposal { proposal_id },
            )));
        }
        if let Some(proposal_id) = parse_proposal_id(trimmed.strip_prefix(":proposal-approve ")) {
            return Ok(Some(self.push_intent(
                CommandDispatchIntent::ApproveProposal { proposal_id },
            )));
        }
        if let Some(proposal_id) = parse_proposal_id(trimmed.strip_prefix(":proposal-reject ")) {
            return Ok(Some(self.push_intent(
                CommandDispatchIntent::RejectProposal {
                    proposal_id,
                    reason: ProposalRejectionReason::UserRejected,
                },
            )));
        }
        if let Some(proposal_id) = parse_proposal_id(trimmed.strip_prefix(":proposal-apply ")) {
            return Ok(Some(self.push_intent(
                CommandDispatchIntent::ApplyProposal { proposal_id },
            )));
        }
        if let Some(proposal_id) = parse_proposal_id(trimmed.strip_prefix(":proposal-rollback ")) {
            return Ok(Some(self.push_intent(
                CommandDispatchIntent::RollbackProposal {
                    proposal_id,
                    reason: ProposalRollbackReason::UserRequested,
                },
            )));
        }
        if let Some(proposal_id) = parse_proposal_id(trimmed.strip_prefix(":proposal-cancel ")) {
            return Ok(Some(self.push_intent(
                CommandDispatchIntent::CancelProposal {
                    proposal_id,
                    reason: ProposalCancellationReason::UserCancelled,
                },
            )));
        }
        if let Some(proposal_id) = parse_proposal_id(trimmed.strip_prefix(":proposal-details ")) {
            return Ok(Some(self.push_intent(
                CommandDispatchIntent::OpenProposalDetails { proposal_id },
            )));
        }
        if let Some(session_id) = parse_legion_session_id(trimmed.strip_prefix(":legion-inspect "))
        {
            return Ok(Some(self.push_intent(
                CommandDispatchIntent::InspectLegionWorkflowSession { session_id },
            )));
        }
        if let Some((session_id, proposal_id)) =
            parse_legion_session_proposal(trimmed.strip_prefix(":legion-proposal-preview "))
        {
            return Ok(Some(self.push_intent(
                CommandDispatchIntent::OpenLegionWorkflowProposalPreview {
                    session_id,
                    proposal_id,
                },
            )));
        }
        if let Some((session_id, proposal_id)) =
            parse_legion_session_proposal(trimmed.strip_prefix(":legion-proposal-details "))
        {
            return Ok(Some(self.push_intent(
                CommandDispatchIntent::OpenLegionWorkflowProposalDetails {
                    session_id,
                    proposal_id,
                },
            )));
        }
        if let Some((session_id, gate_id)) =
            parse_legion_session_label(trimmed.strip_prefix(":legion-verify "))
        {
            return Ok(Some(self.push_intent(
                CommandDispatchIntent::RequestLegionWorkflowVerification {
                    session_id,
                    gate_id: LegionWorkflowVerificationGateId(gate_id),
                },
            )));
        }
        if let Some((session_id, sign_off_id)) =
            parse_legion_session_label(trimmed.strip_prefix(":legion-signoff "))
        {
            return Ok(Some(self.push_intent(
                CommandDispatchIntent::RequestLegionWorkflowSignOff {
                    session_id,
                    sign_off_id: LegionWorkflowSignOffId(sign_off_id),
                },
            )));
        }
        if let Some((session_id, conflict_id)) =
            parse_legion_session_label(trimmed.strip_prefix(":legion-resolve "))
        {
            return Ok(Some(self.push_intent(
                CommandDispatchIntent::ResolveLegionWorkflowConflict {
                    session_id,
                    conflict_id: LegionWorkflowConflictId(conflict_id),
                },
            )));
        }
        if let Some(session_id) =
            parse_legion_session_id(trimmed.strip_prefix(":legion-readiness "))
        {
            return Ok(Some(self.push_intent(
                CommandDispatchIntent::RequestLegionWorkflowMergeReadiness { session_id },
            )));
        }
        if let Some((session_id, server_id, tool_name, decision)) =
            parse_legion_tool_permission(trimmed.strip_prefix(":legion-permission "))
        {
            return Ok(Some(self.push_intent(
                CommandDispatchIntent::RecordLegionWorkflowToolPermission {
                    session_id,
                    server_id,
                    tool_name,
                    decision,
                },
            )));
        }
        if let Some((session_id, reason_label)) =
            parse_legion_kill_switch(trimmed.strip_prefix(":legion-kill "))
        {
            return Ok(Some(self.push_intent(
                CommandDispatchIntent::TriggerLegionWorkflowKillSwitch {
                    session_id,
                    reason_label,
                },
            )));
        }

        if let Some(payload) = trimmed.strip_prefix(":i ") {
            let buffer_id = self.active_buffer_id()?;
            let pos = protocol_text_coordinate(0, 0, Some(0));
            return Ok(Some(self.push_intent(CommandDispatchIntent::Insert {
                buffer_id,
                at: pos,
                text: payload.to_string(),
            })));
        }

        if let Some(payload) = trimmed.strip_prefix(":d ") {
            let buffer_id = self.active_buffer_id()?;
            let mut split = payload.split(',');
            let start = split.next().unwrap_or("0").parse::<usize>().unwrap_or(0);
            let end = split.next().unwrap_or("0").parse::<usize>().unwrap_or(0);
            if start > end {
                return Err(ShellCommandError::InvalidRange);
            }
            let start = self.parse_pos(start)?;
            let end = self.parse_pos(end)?;
            return Ok(Some(self.push_intent(CommandDispatchIntent::Delete {
                buffer_id,
                range: ProtocolTextRange { start, end },
            })));
        }

        if let Some(payload) = trimmed.strip_prefix(":r ") {
            let buffer_id = self.active_buffer_id()?;
            let mut split = payload.splitn(3, ',');
            let start = split.next().unwrap_or("0").parse::<usize>().unwrap_or(0);
            let end = split.next().unwrap_or("0").parse::<usize>().unwrap_or(0);
            let replacement = split.next().unwrap_or("");
            if start > end {
                return Err(ShellCommandError::InvalidRange);
            }
            let start = self.parse_pos(start)?;
            let end = self.parse_pos(end)?;
            return Ok(Some(self.push_intent(CommandDispatchIntent::Replace {
                buffer_id,
                range: ProtocolTextRange { start, end },
                replacement: replacement.to_string(),
            })));
        }

        Ok(Some(self.push_intent(CommandDispatchIntent::Noop)))
    }
}
