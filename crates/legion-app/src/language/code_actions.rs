//! Bounded two-step LSP code-action authority.
//!
//! The first step stores a response-scoped candidate table and projects only
//! metadata.  The second step consumes an opaque token after rechecking the
//! exact document identity captured by the request.  Translation and proposal
//! registration remain in the existing write-side pipeline; the drain owner
//! calls [`AppComposition::install_code_action_response`] when it receives a
//! response.

use std::collections::HashMap;
use std::io;

use crate::{AppComposition, language_id_for_path};
use legion_protocol::{
    BufferId, BufferVersion, FileContentVersion, FileFingerprint, LanguageCodeActionProjection,
    LspCodeActionCandidate, LspCodeActionPayload, ProtocolTextRange, SnapshotId, TextCoordinate,
    WorkspaceGeneration, WorkspaceId,
};
use uuid::Uuid;

const MAX_CANDIDATES: usize = 64;
const MAX_SERIALIZED_PAYLOAD_BYTES: usize = 256 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CodeActionIdentity {
    pub(crate) workspace_id: WorkspaceId,
    pub(crate) buffer_id: BufferId,
    pub(crate) snapshot_id: SnapshotId,
    pub(crate) buffer_version: BufferVersion,
    pub(crate) file_content_version: FileContentVersion,
    pub(crate) workspace_generation: WorkspaceGeneration,
    pub(crate) fingerprint: FileFingerprint,
}

#[derive(Debug, Clone)]
pub(crate) struct StoredCodeAction {
    pub(crate) response_id: String,
    pub(crate) action_id: String,
    pub(crate) identity: CodeActionIdentity,
    pub(crate) title: String,
    pub(crate) kind: Option<String>,
    pub(crate) is_preferred: bool,
    pub(crate) disabled_reason: Option<String>,
    pub(crate) payload: LspCodeActionPayload,
    pub(crate) raw_action: serde_json::Value,
    pub(crate) organize_imports: bool,
}

#[derive(Debug, Default)]
pub(crate) struct CodeActionAuthority {
    pending: Option<(String, CodeActionIdentity, ProtocolTextRange, bool)>,
    response_id: Option<String>,
    candidates: HashMap<String, StoredCodeAction>,
    order: Vec<String>,
    payload_bytes: usize,
    resolving: HashMap<(String, String), String>,
}

impl CodeActionAuthority {
    pub(crate) fn clear(&mut self) {
        self.pending = None;
        self.response_id = None;
        self.candidates.clear();
        self.order.clear();
        self.payload_bytes = 0;
        self.resolving.clear();
    }

    pub(crate) fn arm(
        &mut self,
        operation_id: String,
        identity: CodeActionIdentity,
        range: ProtocolTextRange,
        organize_imports: bool,
    ) {
        // A new request replaces both the pending request and any selectable
        // response.  This prevents a late selection from crossing requests.
        self.clear();
        self.pending = Some((operation_id, identity, range, organize_imports));
    }

    pub(crate) fn pending_operation(&self) -> Option<&str> {
        self.pending
            .as_ref()
            .map(|(operation_id, _, _, _)| operation_id.as_str())
    }

    pub(crate) fn install(
        &mut self,
        operation_id: &str,
        actions: Vec<LspCodeActionCandidate>,
        raw_actions: Vec<serde_json::Value>,
    ) -> Option<String> {
        let (_, identity, _, organize_imports) = self.pending.as_ref()?;
        if operation_id != self.pending_operation()? {
            return None;
        }
        let identity = identity.clone();
        let response_id = format!("code-response:{}", Uuid::now_v7());
        let mut candidates = HashMap::new();
        let mut order = Vec::new();
        let mut payload_bytes = 0usize;
        for (ordinal, action) in actions.into_iter().take(MAX_CANDIDATES).enumerate() {
            if action.title.len() > 120 || action.kind.as_ref().is_some_and(|kind| kind.len() > 80)
            {
                continue;
            }
            let disabled_reason = match &action.payload {
                LspCodeActionPayload::Disabled { reason } if reason.len() > 160 => continue,
                LspCodeActionPayload::Disabled { reason } => Some(reason.clone()),
                _ => None,
            };
            let encoded_len =
                bounded_code_action_size(&action.payload, MAX_SERIALIZED_PAYLOAD_BYTES).ok()?;
            let raw_action_ref = raw_actions.get(ordinal);
            let raw_len = match raw_action_ref {
                Some(raw) => bounded_code_action_size(raw, MAX_SERIALIZED_PAYLOAD_BYTES).ok()?,
                None => 0,
            };
            let Some(encoded_len) = encoded_len.checked_add(raw_len) else {
                break;
            };
            let Some(next_bytes) = payload_bytes.checked_add(encoded_len) else {
                break;
            };
            if next_bytes > MAX_SERIALIZED_PAYLOAD_BYTES {
                break;
            }
            let action_id = format!("code-action:{response_id}:{ordinal}");
            candidates.insert(
                action_id.clone(),
                StoredCodeAction {
                    response_id: response_id.clone(),
                    action_id: action_id.clone(),
                    identity: identity.clone(),
                    title: action.title,
                    kind: action.kind,
                    is_preferred: action.is_preferred,
                    disabled_reason,
                    payload: action.payload,
                    raw_action: raw_action_ref.cloned().unwrap_or(serde_json::Value::Null),
                    organize_imports: *organize_imports,
                },
            );
            order.push(action_id);
            payload_bytes = next_bytes;
        }
        self.response_id = Some(response_id.clone());
        self.candidates = candidates;
        self.order = order;
        self.payload_bytes = payload_bytes;
        self.pending = None;
        Some(response_id)
    }

    pub(crate) fn select(
        &mut self,
        response_id: &str,
        action_id: &str,
        current: &CodeActionIdentity,
    ) -> Result<StoredCodeAction, &'static str> {
        if self.response_id.as_deref() != Some(response_id) {
            return Err("code action response is stale");
        }
        let candidate = self
            .candidates
            .get(action_id)
            .ok_or("code action token is unknown")?;
        if candidate.response_id != response_id || candidate.action_id != action_id {
            return Err("code action token identity mismatch");
        }
        if &candidate.identity != current {
            return Err("code action document identity is stale");
        }
        if matches!(candidate.payload, LspCodeActionPayload::Disabled { .. }) {
            return Err("code action is disabled");
        }
        Ok(candidate.clone())
    }

    pub(crate) fn current_response_id(&self) -> Option<&str> {
        self.response_id.as_deref()
    }

    pub(crate) fn candidate_buffer_id(
        &self,
        response_id: &str,
        action_id: &str,
    ) -> Option<BufferId> {
        (self.response_id.as_deref() == Some(response_id))
            .then(|| self.candidates.get(action_id))
            .flatten()
            .filter(|candidate| candidate.response_id == response_id)
            .map(|candidate| candidate.identity.buffer_id)
    }

    pub(crate) fn candidate_is_organize_imports(
        &self,
        response_id: &str,
        action_id: &str,
    ) -> Option<bool> {
        (self.response_id.as_deref() == Some(response_id))
            .then(|| self.candidates.get(action_id))
            .flatten()
            .filter(|candidate| candidate.response_id == response_id)
            .map(|candidate| candidate.organize_imports)
    }

    pub(crate) fn sole_organize_edit_candidate(&self) -> Option<(String, String)> {
        if self.candidates.len() != 1 {
            return None;
        }
        let candidate = self.candidates.values().next()?;
        if candidate.organize_imports
            && matches!(candidate.payload, LspCodeActionPayload::EditOnly { .. })
        {
            Some((candidate.response_id.clone(), candidate.action_id.clone()))
        } else {
            None
        }
    }

    pub(crate) fn consume(&mut self, response_id: &str, action_id: &str) {
        if self.response_id.as_deref() == Some(response_id) {
            self.candidates.remove(action_id);
            self.order.retain(|id| id != action_id);
        }
    }

    pub(crate) fn begin_resolve(
        &mut self,
        response_id: &str,
        action_id: &str,
        attempt_id: &str,
    ) -> bool {
        if self.resolving.len() >= 32 || self.response_id.as_deref() != Some(response_id) {
            return false;
        }
        if !self.candidates.contains_key(action_id) {
            return false;
        }
        if self
            .resolving
            .contains_key(&(response_id.to_string(), action_id.to_string()))
        {
            return false;
        }
        self.resolving.insert(
            (response_id.to_string(), action_id.to_string()),
            attempt_id.to_string(),
        );
        true
    }

    pub(crate) fn finish_resolve_if_matches(
        &mut self,
        response_id: &str,
        action_id: &str,
        attempt_id: &str,
    ) -> bool {
        let key = (response_id.to_string(), action_id.to_string());
        if self
            .resolving
            .get(&key)
            .is_some_and(|current| current == attempt_id)
        {
            self.resolving.remove(&key);
            return true;
        }
        false
    }

    pub(crate) fn remove_resolve_attempt(&mut self, attempt_id: &str) -> bool {
        let before = self.resolving.len();
        self.resolving.retain(|_, current| current != attempt_id);
        self.resolving.len() != before
    }

    pub(crate) fn projections(&self) -> Vec<LanguageCodeActionProjection> {
        self.order
            .iter()
            .filter_map(|id| self.candidates.get(id))
            .map(|candidate| {
                let (has_edit, has_command) = match &candidate.payload {
                    LspCodeActionPayload::CommandOnly { .. } => (false, true),
                    LspCodeActionPayload::EditOnly { .. } => (true, false),
                    LspCodeActionPayload::EditAndCommand { .. } => (true, true),
                    LspCodeActionPayload::Resolve { .. } => (false, false),
                    LspCodeActionPayload::Disabled { .. } => (false, false),
                };
                LanguageCodeActionProjection {
                    response_id: candidate.response_id.clone(),
                    action_id: candidate.action_id.clone(),
                    title: candidate.title.clone(),
                    kind: candidate.kind.clone(),
                    is_preferred: candidate.is_preferred,
                    disabled_reason: candidate.disabled_reason.clone(),
                    has_edit,
                    has_command,
                    buffer_id: Some(candidate.identity.buffer_id),
                    snapshot_id: Some(candidate.identity.snapshot_id),
                    schema_version: 1,
                }
            })
            .collect()
    }
}

pub(crate) fn bounded_code_action_size<T: serde::Serialize>(
    payload: &T,
    limit: usize,
) -> Result<usize, &'static str> {
    struct Counter {
        used: usize,
        limit: usize,
    }
    impl io::Write for Counter {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            let next = self
                .used
                .checked_add(bytes.len())
                .ok_or_else(|| io::Error::other("payload size overflow"))?;
            if next > self.limit {
                return Err(io::Error::other(
                    "payload exceeds bounded code-action budget",
                ));
            }
            self.used = next;
            Ok(bytes.len())
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }
    let mut writer = Counter { used: 0, limit };
    serde_json::to_writer(&mut writer, payload).map_err(|_| "payload exceeds bound")?;
    Ok(writer.used)
}

impl AppComposition {
    pub(crate) fn select_code_action_candidate(
        &mut self,
        response_id: &str,
        action_id: &str,
        buffer_id: BufferId,
    ) -> Result<StoredCodeAction, &'static str> {
        let workspace_id = self
            .active_documents
            .workspace_id()
            .ok_or("code action workspace is unavailable")?;
        let metadata = self
            .active_documents
            .metadata_for_buffer(buffer_id)
            .ok_or("code action buffer is unavailable")?;
        let snapshot = self
            .editor
            .current_snapshot(buffer_id)
            .map_err(|_| "code action snapshot is unavailable")?;
        let buffer_version = self
            .editor
            .buffer_version(buffer_id)
            .map_err(|_| "code action buffer version is unavailable")?;
        let current = CodeActionIdentity {
            workspace_id,
            buffer_id,
            snapshot_id: snapshot.snapshot_id,
            buffer_version,
            file_content_version: metadata.file_content_version,
            workspace_generation: metadata.workspace_generation,
            fingerprint: metadata.fingerprint.clone(),
        };
        self.code_action_authority
            .select(response_id, action_id, &current)
    }

    pub(crate) fn request_code_actions(
        &mut self,
        buffer_id: BufferId,
        range: ProtocolTextRange,
    ) -> bool {
        self.request_code_actions_scoped(buffer_id, range, false)
    }

    pub(crate) fn active_code_action_range(
        &self,
        buffer_id: BufferId,
    ) -> Option<ProtocolTextRange> {
        let cursor = self.editor.primary_cursor(buffer_id).ok()?;
        let text = self.editor.text(buffer_id).ok()?.to_string();
        let line_start = text
            .split_inclusive('\n')
            .take(cursor.line)
            .map(str::len)
            .sum::<usize>();
        let byte_offset = line_start.saturating_add(cursor.column);
        let character = text
            .get(line_start..byte_offset)
            .map(|line| line.chars().count() as u32)
            .unwrap_or(0);
        let position = TextCoordinate {
            line: cursor.line as u32,
            character,
            byte_offset: Some(byte_offset as u64),
            utf16_offset: None,
        };
        Some(ProtocolTextRange {
            start: position,
            end: position,
        })
    }

    pub(crate) fn request_code_actions_scoped(
        &mut self,
        buffer_id: BufferId,
        range: ProtocolTextRange,
        organize_imports: bool,
    ) -> bool {
        let Some(workspace_id) = self.active_documents.workspace_id() else {
            return false;
        };
        let Some(metadata) = self
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
        let Ok(buffer_version) = self.editor.buffer_version(buffer_id) else {
            return false;
        };
        let wire_range = legion_protocol::Utf16Range {
            start: legion_protocol::Utf16Position {
                line: range.start.line,
                character: range.start.character,
            },
            end: legion_protocol::Utf16Position {
                line: range.end.line,
                character: range.end.character,
            },
        };
        if organize_imports
            && matches!(
                language_id_for_path(&metadata.identity.canonical_path)
                    .0
                    .as_str(),
                "typescript" | "typescriptreact" | "javascript" | "javascriptreact"
            )
            && self
                .lsp_session
                .supports_execute_command("_typescript.organizeImports")
        {
            let response_id = format!("app-organize:{}", uuid::Uuid::now_v7());
            let action_id = format!("app-organize:{}", uuid::Uuid::now_v7());
            let file = crate::uri_to_canonical_path(&crate::canonical_path_to_uri(
                &metadata.identity.canonical_path.0,
            ));
            let raw_action = serde_json::json!({
                "command": {
                    "command": "_typescript.organizeImports",
                    "arguments": [
                        file,
                        { "mode": "All", "skipDestructiveCodeActions": false }
                    ]
                }
            });
            return self
                .issue_code_action_command(
                    buffer_id,
                    &response_id,
                    &action_id,
                    &raw_action,
                    snapshot_id,
                    workspace_id,
                    legion_protocol::LanguageToolingOperationKind::OrganizeImportsProposal,
                )
                .is_ok();
        }
        let old_operations: std::collections::HashSet<String> =
            self.pending_lsp_writes.keys().cloned().collect();
        if !self.issue_lsp_code_action_request(buffer_id, wire_range, organize_imports) {
            return false;
        }
        let Some(operation_id) = self
            .pending_lsp_writes
            .keys()
            .find(|operation_id| !old_operations.contains(*operation_id))
            .cloned()
        else {
            return false;
        };
        self.code_action_authority.arm(
            operation_id,
            CodeActionIdentity {
                workspace_id,
                buffer_id,
                snapshot_id,
                buffer_version,
                file_content_version: metadata.file_content_version,
                workspace_generation: metadata.workspace_generation,
                fingerprint: metadata.fingerprint,
            },
            range,
            organize_imports,
        );
        true
    }

    pub(crate) fn install_code_action_response(
        &mut self,
        operation_id: &str,
        actions: Vec<LspCodeActionCandidate>,
        raw_actions: Vec<serde_json::Value>,
    ) -> Option<String> {
        let response_id = self
            .code_action_authority
            .install(operation_id, actions, raw_actions)?;
        self.language_tooling.projection.code_action_candidates =
            self.code_action_authority.projections();
        Some(response_id)
    }

    pub(crate) fn clear_code_actions(&mut self) {
        self.code_action_authority.clear();
        self.code_action_command_sidecars.clear();
        self.pending_code_action_contexts.clear();
        self.language_tooling
            .projection
            .code_action_candidates
            .clear();
    }
}
