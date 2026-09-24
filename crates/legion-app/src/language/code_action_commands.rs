//! Bounded command payloads retained across an approved code-action edit.
//!
//! A mixed code action must keep its command separate from the workspace-edit
//! proposal.  The command is admitted here, but is only dispatched by the app
//! after the associated proposal has applied successfully.

use std::collections::HashMap;
use std::io;

use legion_protocol::{LanguageToolingOperationKind, LspOperationContext, ProposalId};

use super::code_actions::CodeActionIdentity;

pub(crate) const MAX_COMMAND_SIDECARS: usize = 32;
pub(crate) const MAX_COMMAND_ARGUMENT_BYTES: usize = 256 * 1024;
pub(crate) const MAX_COMMAND_ID_BYTES: usize = 256;

#[derive(Debug, Clone)]
pub(crate) struct PendingLspCommandContext {
    pub(crate) context: LspOperationContext,
    pub(crate) operation_kind: LanguageToolingOperationKind,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct CodeActionCommand {
    pub(crate) command_id: String,
    pub(crate) arguments: serde_json::Value,
    pub(crate) response_id: String,
    pub(crate) action_id: String,
    pub(crate) operation_kind: LanguageToolingOperationKind,
    pub(crate) identity: CodeActionIdentity,
}

#[derive(Debug, Default)]
pub(crate) struct CodeActionCommandSidecars {
    entries: HashMap<ProposalId, CodeActionCommand>,
    argument_bytes: usize,
}

impl CodeActionCommandSidecars {
    #[cfg(test)]
    pub(crate) fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub(crate) fn clear(&mut self) {
        self.entries.clear();
        self.argument_bytes = 0;
    }

    pub(crate) fn can_admit_arguments(&self, arguments: &serde_json::Value) -> bool {
        if self.entries.len() >= MAX_COMMAND_SIDECARS {
            return false;
        }
        let Ok(bytes) = bounded_json_size(arguments, MAX_COMMAND_ARGUMENT_BYTES) else {
            return false;
        };
        self.argument_bytes
            .checked_add(bytes)
            .is_some_and(|total| total <= MAX_COMMAND_ARGUMENT_BYTES)
    }

    pub(crate) fn insert(
        &mut self,
        proposal_id: ProposalId,
        command: CodeActionCommand,
    ) -> Result<(), &'static str> {
        if self.entries.contains_key(&proposal_id) {
            return Err("duplicate code-action proposal ID");
        }
        if self.entries.len() >= MAX_COMMAND_SIDECARS {
            return Err("code-action command sidecar limit exceeded");
        }
        let bytes = bounded_json_size(&command.arguments, MAX_COMMAND_ARGUMENT_BYTES)
            .map_err(|_| "code-action command arguments exceed the bounded payload budget")?;
        let next = self
            .argument_bytes
            .checked_add(bytes)
            .ok_or("code-action command sidecar size overflow")?;
        if next > MAX_COMMAND_ARGUMENT_BYTES {
            return Err("code-action command sidecar budget exceeded");
        }
        self.entries.insert(proposal_id, command);
        self.argument_bytes = next;
        Ok(())
    }

    pub(crate) fn take(&mut self, proposal_id: ProposalId) -> Option<CodeActionCommand> {
        let command = self.entries.remove(&proposal_id)?;
        let bytes = bounded_json_size(&command.arguments, MAX_COMMAND_ARGUMENT_BYTES).ok()?;
        self.argument_bytes = self.argument_bytes.saturating_sub(bytes);
        Some(command)
    }
}

/// Extract and validate the exact command and argument array from a raw LSP
/// code-action item.  LSP puts arguments on the command object; the fallback
/// accepts the legacy top-level shape used by older test servers.
pub(crate) fn extract_command(
    action: &serde_json::Value,
) -> Result<(String, serde_json::Value), &'static str> {
    let command = action
        .get("command")
        .ok_or("code-action command is missing")?;
    let command_id = command
        .as_str()
        .or_else(|| command.get("command").and_then(serde_json::Value::as_str))
        .ok_or("malformed code-action command")?;
    if command_id.is_empty() {
        return Err("empty code-action command");
    }
    if command_id.len() > MAX_COMMAND_ID_BYTES {
        return Err("code-action command ID exceeds the bounded payload budget");
    }
    let empty_arguments = serde_json::Value::Array(Vec::new());
    let arguments = if command.is_object() {
        command.get("arguments").unwrap_or(&empty_arguments)
    } else {
        action.get("arguments").unwrap_or(&empty_arguments)
    };
    if !arguments.is_array() {
        return Err("code-action command arguments must be an array");
    }
    // Enforce the bound while the server-owned value is still borrowed.  Only
    // after this check do we clone it into the retained sidecar.
    bounded_json_size(arguments, MAX_COMMAND_ARGUMENT_BYTES)
        .map_err(|_| "code-action command arguments exceed the bounded payload budget")?;
    Ok((command_id.to_string(), arguments.clone()))
}

fn bounded_json_size<T: serde::Serialize>(value: &T, limit: usize) -> Result<usize, io::Error> {
    struct Counter {
        used: usize,
        limit: usize,
    }
    impl io::Write for Counter {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            let next = self
                .used
                .checked_add(bytes.len())
                .ok_or_else(|| io::Error::other("JSON size overflow"))?;
            if next > self.limit {
                return Err(io::Error::other("JSON payload exceeds bound"));
            }
            self.used = next;
            Ok(bytes.len())
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    let mut writer = Counter { used: 0, limit };
    serde_json::to_writer(&mut writer, value)
        .map_err(|error| io::Error::other(error.to_string()))?;
    Ok(writer.used)
}

#[cfg(test)]
mod tests {
    use super::*;
    use legion_protocol::{
        BufferId, BufferVersion, FileContentVersion, FileFingerprint, SnapshotId,
        WorkspaceGeneration, WorkspaceId,
    };

    fn identity() -> CodeActionIdentity {
        CodeActionIdentity {
            workspace_id: WorkspaceId(1),
            buffer_id: BufferId(2),
            snapshot_id: SnapshotId(3),
            buffer_version: BufferVersion(4),
            file_content_version: FileContentVersion(5),
            workspace_generation: WorkspaceGeneration(6),
            fingerprint: FileFingerprint {
                algorithm: "test".to_string(),
                value: "fingerprint".to_string(),
            },
        }
    }

    fn sidecar() -> CodeActionCommand {
        CodeActionCommand {
            command_id: "server.fix".to_string(),
            arguments: serde_json::json!(["exact", {"line": 2}]),
            response_id: "response".to_string(),
            action_id: "action".to_string(),
            operation_kind: LanguageToolingOperationKind::CodeActionProposal,
            identity: identity(),
        }
    }

    #[test]
    fn extracts_object_command_and_exact_arguments() {
        let raw = serde_json::json!({
            "command": {"command": "server.fix", "arguments": ["exact", {"line": 2}]}
        });
        assert_eq!(
            extract_command(&raw).unwrap(),
            (
                "server.fix".to_string(),
                serde_json::json!(["exact", {"line": 2}])
            )
        );
    }

    #[test]
    fn nested_command_does_not_read_unrelated_root_arguments() {
        let raw = serde_json::json!({
            "command": {"command": "server.fix"},
            "arguments": ["must-not-be-used"]
        });
        assert_eq!(extract_command(&raw).unwrap().1, serde_json::json!([]));
    }

    #[test]
    fn plain_command_uses_root_arguments() {
        let raw = serde_json::json!({
            "command": "server.fix",
            "arguments": ["exact"]
        });
        assert_eq!(
            extract_command(&raw).unwrap().1,
            serde_json::json!(["exact"])
        );
    }

    #[test]
    fn rejects_non_array_arguments_and_duplicate_proposals() {
        let malformed = serde_json::json!({"command": {"command": "server.fix", "arguments": {}}});
        assert_eq!(
            extract_command(&malformed),
            Err("code-action command arguments must be an array")
        );

        let mut store = CodeActionCommandSidecars::default();
        store.insert(ProposalId(1), sidecar()).unwrap();
        assert_eq!(
            store.insert(ProposalId(1), sidecar()),
            Err("duplicate code-action proposal ID")
        );
        assert_eq!(
            store.take(ProposalId(1)).unwrap().arguments,
            serde_json::json!(["exact", {"line": 2}])
        );
        assert!(store.is_empty());
    }

    #[test]
    fn rejects_arguments_over_cumulative_budget() {
        let mut store = CodeActionCommandSidecars::default();
        let oversized = serde_json::Value::Array(vec![serde_json::Value::String(
            "x".repeat(MAX_COMMAND_ARGUMENT_BYTES),
        )]);
        let mut command = sidecar();
        command.arguments = oversized;
        assert_eq!(
            store.insert(ProposalId(1), command),
            Err("code-action command arguments exceed the bounded payload budget")
        );
        assert!(store.is_empty());
    }
}
