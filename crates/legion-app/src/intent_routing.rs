//! Command-intent routing: UI intents in, port-shaped requests out.
//!
//! Extracted verbatim from `lib.rs` (roadmap 1.1). Nothing here changed in the
//! move; the split exists so that Vim key routing and the Git surface — both
//! of which land in this code — can be reviewed as their own diffs instead of
//! as edits buried in a 38,000-line file.
//!
//! Everything is re-exported from the crate root, so no caller outside this
//! file needs to know the module exists.

use crate::*;

/// Service that maps UI intents into application command requests without invoking concrete adapters.
#[derive(Debug)]
pub struct CommandDispatcher;

/// App-owned metadata used to turn projection-only proposal UI intents into protocol requests.
#[derive(Debug, Clone)]
pub struct AppProposalIntentRouteContext {
    /// App-owned proposal for preview/apply intents when required.
    pub proposal: Option<WorkspaceProposal>,
    /// Principal selected by app/session policy, not by UI state.
    pub principal: PrincipalId,
    /// Capability selected by app/proposal policy, not by UI state.
    pub capability: CapabilityId,
    /// Non-zero app-routed correlation id.
    pub correlation_id: CorrelationId,
    /// App-routed causality id.
    pub causality_id: CausalityId,
    /// App-routed request timestamp.
    pub requested_at: TimestampMillis,
}

impl CommandDispatcher {
    /// Convert a UI command intent into a port-shaped application command request.
    pub fn route_intent(
        intent: CommandDispatchIntent,
        active: AppCommandRouteContext,
        correlation_id: CorrelationId,
    ) -> Result<AppCommandRequest, AppCompositionError> {
        match intent {
            CommandDispatchIntent::Noop => Ok(AppCommandRequest::Noop),
            CommandDispatchIntent::Quit => Ok(AppCommandRequest::Quit),
            CommandDispatchIntent::SetProductMode { mode } => {
                Ok(AppCommandRequest::SetProductMode {
                    mode: AppProductMode::from_dock_mode(mode),
                })
            }
            CommandDispatchIntent::Undo { buffer_id } => {
                Self::ensure_active_buffer(active.buffer_id, buffer_id)?;
                Ok(AppCommandRequest::Undo { buffer_id })
            }
            CommandDispatchIntent::Redo { buffer_id } => {
                Self::ensure_active_buffer(active.buffer_id, buffer_id)?;
                Ok(AppCommandRequest::Redo { buffer_id })
            }
            CommandDispatchIntent::Insert {
                buffer_id,
                at,
                text,
            } => Self::edit_request(
                active,
                buffer_id,
                TextEdit::insert(Self::editor_position(at), text),
                correlation_id,
            ),
            CommandDispatchIntent::Delete { buffer_id, range } => Self::edit_request(
                active,
                buffer_id,
                TextEdit::delete(Self::editor_range(range)),
                correlation_id,
            ),
            CommandDispatchIntent::Replace {
                buffer_id,
                range,
                replacement,
            } => Self::edit_request(
                active,
                buffer_id,
                TextEdit::new(Self::editor_range(range), replacement),
                correlation_id,
            ),
            CommandDispatchIntent::ClipboardCopy { buffer_id } => {
                Self::ensure_active_buffer(active.buffer_id, buffer_id)?;
                Ok(AppCommandRequest::ClipboardCopy { buffer_id })
            }
            CommandDispatchIntent::ClipboardCut { buffer_id } => {
                Self::ensure_active_buffer(active.buffer_id, buffer_id)?;
                Ok(AppCommandRequest::ClipboardCut { buffer_id })
            }
            CommandDispatchIntent::SelectAll { buffer_id } => {
                Self::ensure_active_buffer(active.buffer_id, buffer_id)?;
                Ok(AppCommandRequest::SelectAll { buffer_id })
            }
            CommandDispatchIntent::Save { buffer_id } => {
                Self::ensure_active_buffer(active.buffer_id, buffer_id)?;
                Ok(AppCommandRequest::Save { buffer_id })
            }
            CommandDispatchIntent::SwitchTab { buffer_id } => {
                Ok(AppCommandRequest::SwitchTab { buffer_id })
            }
            CommandDispatchIntent::CloseTab { buffer_id } => {
                Ok(AppCommandRequest::CloseTab { buffer_id })
            }
            CommandDispatchIntent::ReorderTab {
                buffer_id,
                new_index,
            } => Ok(AppCommandRequest::ReorderTab {
                buffer_id,
                new_index,
            }),
            CommandDispatchIntent::SaveAll => Ok(AppCommandRequest::SaveAll),
            CommandDispatchIntent::SetCursor { buffer_id, cursor } => {
                Ok(AppCommandRequest::SetCursor { buffer_id, cursor })
            }
            CommandDispatchIntent::SetSelection { buffer_id, range } => {
                Ok(AppCommandRequest::SetSelection { buffer_id, range })
            }
            CommandDispatchIntent::SetViewportScroll { buffer_id, scroll } => {
                Ok(AppCommandRequest::SetViewportScroll { buffer_id, scroll })
            }
            CommandDispatchIntent::OpenPalette { mode, query, scope } => {
                Ok(AppCommandRequest::OpenPalette { mode, query, scope })
            }
            CommandDispatchIntent::ClosePalette => Ok(AppCommandRequest::ClosePalette),
            CommandDispatchIntent::UpdatePaletteQuery { query } => {
                Ok(AppCommandRequest::UpdatePaletteQuery { query })
            }
            CommandDispatchIntent::MovePaletteSelection { delta } => {
                Ok(AppCommandRequest::MovePaletteSelection { delta })
            }
            CommandDispatchIntent::CompletePaletteSelection => {
                Ok(AppCommandRequest::CompletePaletteSelection)
            }
            CommandDispatchIntent::DispatchPaletteSelection => {
                Ok(AppCommandRequest::DispatchPaletteSelection)
            }
            CommandDispatchIntent::ConfirmPaletteSelection {
                token,
                command_id,
                operands,
            } => Ok(AppCommandRequest::ConfirmPaletteSelection {
                token,
                command_id,
                operands,
            }),
            CommandDispatchIntent::CancelPaletteConfirmation { token } => {
                Ok(AppCommandRequest::CancelPaletteConfirmation { token })
            }
            CommandDispatchIntent::OpenSettings => Ok(AppCommandRequest::OpenSettings),
            CommandDispatchIntent::SetThemePreference { preference } => {
                Ok(AppCommandRequest::SetThemePreference { preference })
            }
            CommandDispatchIntent::SetZoomPercent { zoom_percent } => {
                Ok(AppCommandRequest::SetZoomPercent { zoom_percent })
            }
            CommandDispatchIntent::SetEditorFontSize { font_size_pt } => {
                Ok(AppCommandRequest::SetEditorFontSize { font_size_pt })
            }
            CommandDispatchIntent::SetEditorFontFamily { family } => {
                Ok(AppCommandRequest::SetEditorFontFamily { family })
            }
            CommandDispatchIntent::SetToastVerbosity { verbosity } => {
                Ok(AppCommandRequest::SetToastVerbosity { verbosity })
            }
            CommandDispatchIntent::SetLineNumbersVisible { visible } => {
                Ok(AppCommandRequest::SetLineNumbersVisible { visible })
            }
            CommandDispatchIntent::SetCurrentLineHighlight { enabled } => {
                Ok(AppCommandRequest::SetCurrentLineHighlight { enabled })
            }
            CommandDispatchIntent::SetStickyHeadersVisible { visible } => {
                Ok(AppCommandRequest::SetStickyHeadersVisible { visible })
            }
            CommandDispatchIntent::SetCodeFoldingVisible { visible } => {
                Ok(AppCommandRequest::SetCodeFoldingVisible { visible })
            }
            CommandDispatchIntent::SetMinimapVisible { visible } => {
                Ok(AppCommandRequest::SetMinimapVisible { visible })
            }
            CommandDispatchIntent::SetWhitespaceGuidesVisible { visible } => {
                Ok(AppCommandRequest::SetWhitespaceGuidesVisible { visible })
            }
            CommandDispatchIntent::SetIndentGuidesVisible { visible } => {
                Ok(AppCommandRequest::SetIndentGuidesVisible { visible })
            }
            CommandDispatchIntent::SetSmoothScrollingEnabled { enabled } => {
                Ok(AppCommandRequest::SetSmoothScrollingEnabled { enabled })
            }
            CommandDispatchIntent::SetLineWrappingPolicy {
                policy,
                wrap_column,
            } => Ok(AppCommandRequest::SetLineWrappingPolicy {
                policy,
                wrap_column,
            }),
            CommandDispatchIntent::SetIndexedWorkspaceSearchEnabled { enabled } => {
                Ok(AppCommandRequest::SetIndexedWorkspaceSearchEnabled { enabled })
            }
            CommandDispatchIntent::SetNextEditPredictionEnabled { enabled } => {
                Ok(AppCommandRequest::SetNextEditPredictionEnabled { enabled })
            }
            CommandDispatchIntent::SetCrashReportsEnabled { enabled } => {
                Ok(AppCommandRequest::SetCrashReportsEnabled { enabled })
            }
            CommandDispatchIntent::ResetSettings => Ok(AppCommandRequest::ResetSettings),
            CommandDispatchIntent::RunSearch {
                scope,
                query,
                limit,
                case_sensitive,
                whole_word,
                use_regex,
            } => Ok(AppCommandRequest::RunSearch {
                query_id: format!("search:{}", correlation_id.0),
                scope,
                query,
                limit,
                case_sensitive,
                whole_word,
                use_regex,
            }),
            CommandDispatchIntent::RunStructuralSearch {
                scope,
                pattern,
                rewrite,
                limit,
            } => Ok(AppCommandRequest::RunStructuralSearch {
                query_id: format!("structural-search:{}", correlation_id.0),
                scope,
                pattern,
                rewrite,
                limit,
            }),
            CommandDispatchIntent::CancelSearch { query_id } => {
                Ok(AppCommandRequest::CancelSearch { query_id })
            }
            CommandDispatchIntent::RefreshGit => Ok(AppCommandRequest::RefreshGit),
            CommandDispatchIntent::StageGitHunk { hunk_id } => {
                Ok(AppCommandRequest::StageGitHunk { hunk_id })
            }
            CommandDispatchIntent::UnstageGitHunk { hunk_id } => {
                Ok(AppCommandRequest::UnstageGitHunk { hunk_id })
            }
            CommandDispatchIntent::ResolveGitConflict { path, choice } => {
                Ok(AppCommandRequest::ResolveGitConflict {
                    path,
                    choice: match choice {
                        GitConflictChoiceProjection::AcceptCurrent => {
                            GitConflictChoice::AcceptCurrent
                        }
                        GitConflictChoiceProjection::AcceptIncoming => {
                            GitConflictChoice::AcceptIncoming
                        }
                    },
                })
            }
            CommandDispatchIntent::CommitGitChanges { message } => {
                Ok(AppCommandRequest::CommitGitChanges { message })
            }
            CommandDispatchIntent::SwitchGitBranch { branch } => {
                Ok(AppCommandRequest::SwitchGitBranch { branch })
            }
            CommandDispatchIntent::CreateGitBranch { branch } => {
                Ok(AppCommandRequest::CreateGitBranch { branch })
            }
            CommandDispatchIntent::DeleteGitBranch { branch } => {
                Ok(AppCommandRequest::DeleteGitBranch { branch })
            }
            CommandDispatchIntent::StashGitChanges { message } => {
                Ok(AppCommandRequest::StashGitChanges { message })
            }
            CommandDispatchIntent::PushGitRemote { remote } => {
                Ok(AppCommandRequest::PushGitRemote { remote })
            }
            CommandDispatchIntent::FetchGitRemote { remote } => {
                Ok(AppCommandRequest::FetchGitRemote { remote })
            }
            CommandDispatchIntent::PullGitRemote { remote } => {
                Ok(AppCommandRequest::PullGitRemote { remote })
            }
            CommandDispatchIntent::GrantGitRemoteHost { host } => {
                Ok(AppCommandRequest::GrantGitRemoteHost { host })
            }
            CommandDispatchIntent::RevokeGitRemoteHost { host } => {
                Ok(AppCommandRequest::RevokeGitRemoteHost { host })
            }
            CommandDispatchIntent::PruneGitWorktrees => Ok(AppCommandRequest::PruneGitWorktrees),
            CommandDispatchIntent::RemoveGitWorktree { path } => {
                Ok(AppCommandRequest::RemoveGitWorktree { path })
            }
            CommandDispatchIntent::CreateGitWorktree {
                branch,
                worktree_path,
            } => Ok(AppCommandRequest::CreateGitWorktree {
                branch,
                worktree_path,
            }),
            CommandDispatchIntent::GitNavNextHunk => Ok(AppCommandRequest::GitNavNextHunk),
            CommandDispatchIntent::GitNavPrevHunk => Ok(AppCommandRequest::GitNavPrevHunk),
            CommandDispatchIntent::GitNavNextFile => Ok(AppCommandRequest::GitNavNextFile),
            CommandDispatchIntent::GitNavPrevFile => Ok(AppCommandRequest::GitNavPrevFile),
            CommandDispatchIntent::RequestLocalHistoryEntries { path } => {
                Ok(AppCommandRequest::RequestLocalHistoryEntries { path })
            }
            CommandDispatchIntent::RestoreFromLocalHistory { path, entry_id } => {
                Ok(AppCommandRequest::RestoreFromLocalHistory { path, entry_id })
            }
            CommandDispatchIntent::ExportWorktreeEvidence => {
                Ok(AppCommandRequest::ExportWorktreeEvidence)
            }
            CommandDispatchIntent::ValidateGitCommitMessage { message } => {
                Ok(AppCommandRequest::ValidateGitCommitMessage { message })
            }
            CommandDispatchIntent::RefreshDebugConfigurations => {
                Ok(AppCommandRequest::RefreshDebugConfigurations)
            }
            CommandDispatchIntent::RefreshTestExplorer => {
                Ok(AppCommandRequest::RefreshTestExplorer)
            }
            CommandDispatchIntent::RunTestExplorerItem { item_id } => {
                Ok(AppCommandRequest::RunTestExplorerItem { item_id })
            }
            CommandDispatchIntent::RunTestExplorerGroup { parent_label } => {
                Ok(AppCommandRequest::RunTestExplorerGroup { parent_label })
            }
            CommandDispatchIntent::AttachTestExplorerEvidence { session_id } => {
                Ok(AppCommandRequest::AttachTestExplorerEvidence { session_id })
            }
            CommandDispatchIntent::ToggleDebugBreakpoint {
                buffer_id,
                line,
                condition,
                hit_condition,
                log_message,
            } => {
                Self::ensure_active_buffer(active.buffer_id, buffer_id)?;
                Ok(AppCommandRequest::ToggleDebugBreakpoint {
                    buffer_id,
                    line,
                    condition,
                    hit_condition,
                    log_message,
                })
            }
            CommandDispatchIntent::LaunchDebugSession { configuration_id } => {
                Ok(AppCommandRequest::LaunchDebugSession { configuration_id })
            }
            CommandDispatchIntent::DebugStep { session_id, kind } => {
                Ok(AppCommandRequest::DebugStep { session_id, kind })
            }
            CommandDispatchIntent::StopDebugSession { session_id } => {
                Ok(AppCommandRequest::StopDebugSession { session_id })
            }
            CommandDispatchIntent::PollDebugSession { session_id } => {
                Ok(AppCommandRequest::PollDebugSession { session_id })
            }
            CommandDispatchIntent::DebugRunToCursor {
                session_id,
                buffer_id,
                position,
            } => {
                Self::ensure_active_buffer(active.buffer_id, buffer_id)?;
                Ok(AppCommandRequest::DebugRunToCursor {
                    session_id,
                    buffer_id,
                    position,
                })
            }
            CommandDispatchIntent::DebugEvaluateSelection {
                session_id,
                expression_label,
            } => Ok(AppCommandRequest::DebugEvaluateSelection {
                session_id,
                expression_label,
            }),
            CommandDispatchIntent::DebugAddWatch {
                session_id,
                expression_label,
            } => Ok(AppCommandRequest::DebugAddWatch {
                session_id,
                expression_label,
            }),
            CommandDispatchIntent::RequestHover {
                buffer_id,
                position,
            } => {
                Self::ensure_active_buffer(active.buffer_id, buffer_id)?;
                Ok(AppCommandRequest::RequestHover {
                    buffer_id,
                    position,
                })
            }
            CommandDispatchIntent::RequestCompletion {
                buffer_id,
                position,
            } => {
                Self::ensure_active_buffer(active.buffer_id, buffer_id)?;
                Ok(AppCommandRequest::RequestCompletion {
                    buffer_id,
                    position,
                })
            }
            CommandDispatchIntent::RequestAssistInlinePrediction {
                buffer_id,
                position,
            } => {
                Self::ensure_active_buffer(active.buffer_id, buffer_id)?;
                Ok(AppCommandRequest::RequestAssistInlinePrediction {
                    buffer_id,
                    position,
                })
            }
            CommandDispatchIntent::AcceptAssistInlinePrediction {
                buffer_id,
                prediction_id,
            } => {
                Self::ensure_active_buffer(active.buffer_id, buffer_id)?;
                Ok(AppCommandRequest::AcceptAssistInlinePrediction {
                    buffer_id,
                    prediction_id,
                })
            }
            CommandDispatchIntent::DismissAssistInlinePrediction {
                buffer_id,
                prediction_id,
            } => {
                Self::ensure_active_buffer(active.buffer_id, buffer_id)?;
                Ok(AppCommandRequest::DismissAssistInlinePrediction {
                    buffer_id,
                    prediction_id,
                })
            }
            CommandDispatchIntent::CancelAssistInlinePrediction {
                buffer_id,
                prediction_id,
            } => {
                Self::ensure_active_buffer(active.buffer_id, buffer_id)?;
                Ok(AppCommandRequest::CancelAssistInlinePrediction {
                    buffer_id,
                    prediction_id,
                })
            }
            CommandDispatchIntent::GoToDefinition {
                buffer_id,
                position,
            } => {
                Self::ensure_active_buffer(active.buffer_id, buffer_id)?;
                Ok(AppCommandRequest::GoToDefinition {
                    buffer_id,
                    position,
                })
            }
            CommandDispatchIntent::FindReferences {
                buffer_id,
                position,
            } => {
                Self::ensure_active_buffer(active.buffer_id, buffer_id)?;
                Ok(AppCommandRequest::FindReferences {
                    buffer_id,
                    position,
                })
            }
            CommandDispatchIntent::RefreshInlayHints { buffer_id } => {
                Self::ensure_active_buffer(active.buffer_id, buffer_id)?;
                Ok(AppCommandRequest::RefreshInlayHints { buffer_id })
            }
            CommandDispatchIntent::RefreshCodeLenses { buffer_id } => {
                Self::ensure_active_buffer(active.buffer_id, buffer_id)?;
                Ok(AppCommandRequest::RefreshCodeLenses { buffer_id })
            }
            CommandDispatchIntent::RefreshOutline { buffer_id } => {
                Self::ensure_active_buffer(active.buffer_id, buffer_id)?;
                Ok(AppCommandRequest::RefreshOutline { buffer_id })
            }
            CommandDispatchIntent::RequestFormattingProposal { buffer_id } => {
                Self::ensure_active_buffer(active.buffer_id, buffer_id)?;
                Ok(AppCommandRequest::RequestFormattingProposal { buffer_id })
            }
            CommandDispatchIntent::RequestRenameProposal {
                buffer_id,
                position,
                new_name,
            } => {
                Self::ensure_active_buffer(active.buffer_id, buffer_id)?;
                Ok(AppCommandRequest::RequestRenameProposal {
                    buffer_id,
                    position,
                    new_name,
                })
            }
            CommandDispatchIntent::RequestOrganizeImportsProposal { buffer_id } => {
                Self::ensure_active_buffer(active.buffer_id, buffer_id)?;
                Ok(AppCommandRequest::RequestOrganizeImportsProposal { buffer_id })
            }
            CommandDispatchIntent::RequestCodeActionProposal {
                buffer_id,
                action_id,
            } => {
                Self::ensure_active_buffer(active.buffer_id, buffer_id)?;
                Ok(AppCommandRequest::RequestCodeActionProposal {
                    buffer_id,
                    action_id,
                })
            }
            CommandDispatchIntent::ActivateLanguageCodeLens { buffer_id, lens_id } => {
                Self::ensure_active_buffer(active.buffer_id, buffer_id)?;
                Ok(AppCommandRequest::ActivateLanguageCodeLens { buffer_id, lens_id })
            }
            CommandDispatchIntent::CancelLanguageOperation { operation_id } => {
                Ok(AppCommandRequest::CancelLanguageOperation { operation_id })
            }
            CommandDispatchIntent::TerminalLaunch {
                command_label,
                timeout_secs,
            } => Ok(AppCommandRequest::TerminalLaunch {
                command_label,
                timeout_secs,
            }),
            CommandDispatchIntent::TerminalInput {
                session_id,
                payload,
            } => Ok(AppCommandRequest::TerminalInput {
                session_id,
                payload,
            }),
            CommandDispatchIntent::TerminalResize {
                session_id,
                cols,
                rows,
            } => Ok(AppCommandRequest::TerminalResize {
                session_id,
                cols,
                rows,
            }),
            CommandDispatchIntent::TerminalKill { session_id } => {
                Ok(AppCommandRequest::TerminalKill { session_id })
            }
            CommandDispatchIntent::TerminalClose { session_id } => {
                Ok(AppCommandRequest::TerminalClose { session_id })
            }
            CommandDispatchIntent::TerminalOutputPoll { session_id } => {
                Ok(AppCommandRequest::TerminalOutputPoll { session_id })
            }
            CommandDispatchIntent::TerminalSearch { session_id, query } => {
                Ok(AppCommandRequest::TerminalSearch { session_id, query })
            }
            CommandDispatchIntent::OpenPath { path } => Ok(AppCommandRequest::OpenPath { path }),
            CommandDispatchIntent::OpenPathAtPosition { path, position } => {
                Ok(AppCommandRequest::OpenPathAtPosition { path, position })
            }
            CommandDispatchIntent::RefreshExplorer => Ok(AppCommandRequest::RefreshExplorer),
            CommandDispatchIntent::RevealInExplorer { file_id } => {
                Ok(AppCommandRequest::RevealInExplorer { file_id })
            }
            CommandDispatchIntent::StartAiRun { instruction_label } => {
                Ok(AppCommandRequest::StartAiRun { instruction_label })
            }
            CommandDispatchIntent::StartAiExplain { instruction_label } => {
                Ok(AppCommandRequest::StartAiExplain { instruction_label })
            }
            CommandDispatchIntent::StartAiProposal {
                instruction_label, ..
            } => Ok(AppCommandRequest::StartAiProposal { instruction_label }),
            CommandDispatchIntent::SendDelegateChat { prompt_label } => {
                Ok(AppCommandRequest::SendDelegateChat { prompt_label })
            }
            CommandDispatchIntent::StartDelegatedTask {
                task_description,
                scope,
            } => Ok(AppCommandRequest::StartDelegatedTask {
                task_description,
                scope,
            }),
            CommandDispatchIntent::CancelDelegatedTask => {
                Ok(AppCommandRequest::CancelDelegatedTask)
            }
            CommandDispatchIntent::ReviewDelegateProposalHunk {
                proposal_id,
                hunk_id,
                disposition,
            } => Ok(AppCommandRequest::ReviewDelegateProposalHunk {
                proposal_id,
                hunk_id,
                disposition,
            }),
            CommandDispatchIntent::RecordDelegateToolPermission {
                request_id,
                decision,
            } => Ok(AppCommandRequest::RecordDelegateToolPermission {
                request_id,
                decision,
            }),
            CommandDispatchIntent::RecordLegionWorkflowToolPermission {
                session_id,
                server_id,
                tool_name,
                decision,
            } => Ok(AppCommandRequest::RecordLegionWorkflowToolPermission {
                session_id,
                server_id,
                tool_name,
                decision,
            }),
            CommandDispatchIntent::TriggerLegionWorkflowKillSwitch {
                session_id,
                reason_label,
            } => Ok(AppCommandRequest::TriggerLegionWorkflowKillSwitch {
                session_id,
                reason_label,
            }),
            CommandDispatchIntent::CancelAiRun { run_id } => {
                Ok(AppCommandRequest::CancelAiRun { run_id })
            }
            CommandDispatchIntent::ReplayAiRun { run_id } => {
                Ok(AppCommandRequest::ReplayAiRun { run_id })
            }
            CommandDispatchIntent::InspectAiRun { run_id } => {
                Ok(AppCommandRequest::InspectAiRun { run_id })
            }
            CommandDispatchIntent::InvokePluginCommand {
                plugin_id,
                command_id,
                metadata_label,
            } => Ok(AppCommandRequest::InvokePluginCommand {
                plugin_id,
                command_id,
                metadata_label,
            }),
            CommandDispatchIntent::JoinCollaborationSession { session_id } => {
                Ok(AppCommandRequest::JoinCollaborationSession { session_id })
            }
            CommandDispatchIntent::LeaveCollaborationSession { session_id } => {
                Ok(AppCommandRequest::LeaveCollaborationSession { session_id })
            }
            CommandDispatchIntent::PublishCollaborationPresence {
                session_id,
                participant_id,
            } => Ok(AppCommandRequest::PublishCollaborationPresence {
                session_id,
                participant_id,
            }),
            CommandDispatchIntent::LspStartSession => Ok(AppCommandRequest::LspStartSession),
            CommandDispatchIntent::LspRestartSession => Ok(AppCommandRequest::LspRestartSession),
            CommandDispatchIntent::PreviewProposal { .. }
            | CommandDispatchIntent::ApproveProposal { .. }
            | CommandDispatchIntent::RejectProposal { .. }
            | CommandDispatchIntent::ApplyProposal { .. }
            | CommandDispatchIntent::RollbackProposal { .. }
            | CommandDispatchIntent::CancelProposal { .. }
            | CommandDispatchIntent::OpenProposalDetails { .. }
            | CommandDispatchIntent::InspectLegionWorkflowSession { .. }
            | CommandDispatchIntent::OpenLegionWorkflowProposalPreview { .. }
            | CommandDispatchIntent::OpenLegionWorkflowProposalDetails { .. }
            | CommandDispatchIntent::RequestLegionWorkflowVerification { .. }
            | CommandDispatchIntent::RequestLegionWorkflowSignOff { .. }
            | CommandDispatchIntent::ResolveLegionWorkflowConflict { .. }
            | CommandDispatchIntent::RequestLegionWorkflowMergeReadiness { .. } => {
                Ok(AppCommandRequest::Noop)
            }
            // Multi-cursor intents need the buffer's text to clamp a new
            // cursor to a shorter line, which this router does not have. They
            // are handled in `AppComposition::dispatch_ui_intent`, and these
            // arms satisfy exhaustiveness.
            CommandDispatchIntent::AddCursorAbove { .. }
            | CommandDispatchIntent::AddCursorBelow { .. }
            | CommandDispatchIntent::ClearExtraCursors { .. } => Ok(AppCommandRequest::Noop),
            // Vim modal editing intents: VimState parser exists in legion-ui
            // but is not yet wired to the desktop keyboard handler. These arms
            // satisfy exhaustiveness until integration lands.
            CommandDispatchIntent::SetVimModeEnabled { .. }
            | CommandDispatchIntent::VimMotion { .. }
            | CommandDispatchIntent::VimOperatorMotion { .. }
            | CommandDispatchIntent::VimLinewiseOperator { .. }
            | CommandDispatchIntent::VimChangeMode { .. }
            | CommandDispatchIntent::VimInsertBefore
            | CommandDispatchIntent::VimInsertAfter
            | CommandDispatchIntent::VimInsertLineBelow
            | CommandDispatchIntent::VimInsertLineAbove
            | CommandDispatchIntent::VimPut
            | CommandDispatchIntent::VimSearchForward
            | CommandDispatchIntent::VimDeleteChar => Ok(AppCommandRequest::Noop),
            CommandDispatchIntent::PrepareCallHierarchy {
                buffer_id,
                position,
            } => {
                Self::ensure_active_buffer(active.buffer_id, buffer_id)?;
                Ok(AppCommandRequest::PrepareCallHierarchy {
                    buffer_id,
                    position,
                })
            }
            CommandDispatchIntent::ShowIncomingCalls {
                buffer_id,
                position,
            } => {
                Self::ensure_active_buffer(active.buffer_id, buffer_id)?;
                Ok(AppCommandRequest::ShowIncomingCalls {
                    buffer_id,
                    position,
                })
            }
            CommandDispatchIntent::ShowOutgoingCalls {
                buffer_id,
                position,
            } => {
                Self::ensure_active_buffer(active.buffer_id, buffer_id)?;
                Ok(AppCommandRequest::ShowOutgoingCalls {
                    buffer_id,
                    position,
                })
            }
            // Find/replace intents are handled by AppComposition::dispatch_ui_intent
            // before reaching this router; these arms satisfy exhaustiveness.
            CommandDispatchIntent::ToggleFindBar
            | CommandDispatchIntent::CloseFindBar
            | CommandDispatchIntent::SetFindQuery { .. }
            | CommandDispatchIntent::FindNext
            | CommandDispatchIntent::FindPrevious
            | CommandDispatchIntent::ToggleFindReplace
            | CommandDispatchIntent::SetFindReplaceText { .. }
            | CommandDispatchIntent::ReplaceOne
            | CommandDispatchIntent::ReplaceAll
            | CommandDispatchIntent::SetFindCaseSensitive { .. }
            | CommandDispatchIntent::SetFindWholeWord { .. }
            | CommandDispatchIntent::SetFindRegex { .. } => Ok(AppCommandRequest::Noop),
        }
    }

    /// Convert a projection-only proposal UI intent into a protocol proposal request.
    pub fn route_proposal_intent(
        intent: CommandDispatchIntent,
        context: AppProposalIntentRouteContext,
    ) -> Result<Option<ProposalRequest>, AppCompositionError> {
        match intent {
            CommandDispatchIntent::PreviewProposal { proposal_id } => {
                let proposal = Self::owned_proposal_for_intent(proposal_id, context.proposal)?;
                Ok(Some(ProposalRequest::Preview(proposal)))
            }
            CommandDispatchIntent::ApplyProposal { proposal_id } => {
                let proposal = Self::owned_proposal_for_intent(proposal_id, context.proposal)?;
                Ok(Some(ProposalRequest::Apply(proposal)))
            }
            CommandDispatchIntent::ApproveProposal { proposal_id } => Ok(Some(
                ProposalRequest::Approve(Self::proposal_lifecycle_command(
                    proposal_id,
                    ProposalLifecycleAction::Approve,
                    None,
                    context,
                )),
            )),
            CommandDispatchIntent::RejectProposal {
                proposal_id,
                reason,
            } => Ok(Some(ProposalRequest::Reject(
                Self::proposal_lifecycle_command(
                    proposal_id,
                    ProposalLifecycleAction::Reject,
                    Some(ProposalLifecycleCommandReason::Rejection(reason)),
                    context,
                ),
            ))),
            CommandDispatchIntent::RollbackProposal {
                proposal_id,
                reason,
            } => Ok(Some(ProposalRequest::Rollback(
                Self::proposal_lifecycle_command(
                    proposal_id,
                    ProposalLifecycleAction::Rollback,
                    Some(ProposalLifecycleCommandReason::Rollback(reason)),
                    context,
                ),
            ))),
            CommandDispatchIntent::CancelProposal {
                proposal_id,
                reason,
            } => Ok(Some(ProposalRequest::Cancel(
                Self::proposal_lifecycle_command(
                    proposal_id,
                    ProposalLifecycleAction::Cancel,
                    Some(ProposalLifecycleCommandReason::Cancellation(reason)),
                    context,
                ),
            ))),
            CommandDispatchIntent::OpenProposalDetails { .. } => Ok(None),
            _ => Ok(None),
        }
    }

    fn owned_proposal_for_intent(
        proposal_id: ProposalId,
        proposal: Option<WorkspaceProposal>,
    ) -> Result<WorkspaceProposal, AppCompositionError> {
        let proposal = proposal.ok_or(AppCompositionError::ProposalIntentMissingProposal)?;
        if proposal.proposal_id == proposal_id {
            Ok(proposal)
        } else {
            Err(AppCompositionError::ProposalIntentMismatch {
                target: proposal_id,
                active: Some(proposal.proposal_id),
            })
        }
    }

    fn proposal_lifecycle_command(
        proposal_id: ProposalId,
        action: ProposalLifecycleAction,
        reason: Option<ProposalLifecycleCommandReason>,
        context: AppProposalIntentRouteContext,
    ) -> ProposalLifecycleCommand {
        ProposalLifecycleCommand {
            proposal_id,
            action,
            principal: context.principal,
            capability: context.capability,
            correlation_id: context.correlation_id,
            causality_id: context.causality_id,
            reason,
            diagnostics: Vec::new(),
            requested_at: context.requested_at,
            schema_version: 1,
        }
    }

    fn edit_request(
        active: AppCommandRouteContext,
        buffer_id: BufferId,
        edit: TextEdit,
        _correlation_id: CorrelationId,
    ) -> Result<AppCommandRequest, AppCompositionError> {
        Self::ensure_active_buffer(active.buffer_id, buffer_id)?;
        let _ = active
            .workspace_id
            .ok_or(AppCompositionError::WorkspaceNotOpen)?;
        let _ = active
            .file_id
            .ok_or(AppCompositionError::ActiveFileMissing)?;

        Ok(AppCommandRequest::ApplyEdit { buffer_id, edit })
    }

    pub(crate) fn editor_position(position: TextCoordinate) -> TextPosition {
        TextPosition::new(position.line as usize, position.character as usize)
    }

    pub(crate) fn editor_range(range: legion_protocol::ProtocolTextRange) -> EditorTextRange {
        EditorTextRange::new(
            Self::editor_position(range.start),
            Self::editor_position(range.end),
        )
    }

    fn ensure_active_buffer(
        active: Option<BufferId>,
        target: BufferId,
    ) -> Result<(), AppCompositionError> {
        if active == Some(target) {
            Ok(())
        } else {
            Err(AppCompositionError::BufferMismatch { target, active })
        }
    }
}

/// Minimal active-document context used by command routing tests and dispatcher calls.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AppCommandRouteContext {
    /// Active workspace identifier when a workspace is open.
    pub workspace_id: Option<WorkspaceId>,
    /// Active buffer identifier.
    pub buffer_id: Option<BufferId>,
    /// Active file identifier.
    pub file_id: Option<FileId>,
}
