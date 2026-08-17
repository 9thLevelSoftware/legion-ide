//! P2.F1.T4: the LSP read features are reachable from the editor's command line.
//!
//! Every read feature the task names has to be something a person sitting in
//! the editor can ask for. Hover, completion and definition already were — they
//! ride the keyboard path. References and outline were too. Inlay hints and
//! code lenses were not: the projection could hold them and the app could
//! ingest them, but nothing in the shell could ask.
//!
//! These tests are about the asking, not the answering. The Shell holds no
//! authority; it emits an intent and stops.

use legion_protocol::{BufferId, FileId, TextCoordinate, WorkspaceId};
use legion_ui::{CommandDispatchIntent, Shell};

/// A shell with one buffer open, which is all any of these commands need.
fn shell_with_active_buffer() -> Shell {
    let mut snapshot = Shell::empty("lsp-reads").projection_snapshot();
    snapshot.active_buffer_projection.workspace_id = Some(WorkspaceId(1));
    snapshot.active_buffer_projection.buffer_id = Some(BufferId(7));
    snapshot.active_buffer_projection.file_id = Some(FileId(11));
    snapshot.active_buffer_projection.small_buffer_preview = Some("fn main() {}".to_string());
    Shell::new(snapshot)
}

#[test]
fn inlay_hints_can_be_asked_for_from_the_command_line() {
    let mut shell = shell_with_active_buffer();
    assert_eq!(
        shell
            .handle_command(":inlayhints")
            .expect("inlay-hint refresh should parse"),
        Some(CommandDispatchIntent::RefreshInlayHints {
            buffer_id: BufferId(7)
        })
    );
}

#[test]
fn code_lenses_can_be_asked_for_from_the_command_line() {
    let mut shell = shell_with_active_buffer();
    assert_eq!(
        shell
            .handle_command(":codelens")
            .expect("code-lens refresh should parse"),
        Some(CommandDispatchIntent::RefreshCodeLenses {
            buffer_id: BufferId(7)
        })
    );
}

/// The two read features that were already reachable stay reachable.
///
/// Worth asserting alongside the new pair: P2.F1.T4 changed what `:references`
/// and `:outline` do underneath — they now also ask the language server — and
/// the command surface must not have moved while that happened.
#[test]
fn references_and_outline_remain_reachable_unchanged() {
    let mut shell = shell_with_active_buffer();
    assert_eq!(
        shell
            .handle_command(":outline")
            .expect("outline refresh should parse"),
        Some(CommandDispatchIntent::RefreshOutline {
            buffer_id: BufferId(7)
        })
    );
    assert_eq!(
        shell
            .handle_command(":references 0")
            .expect("references lookup should parse"),
        Some(CommandDispatchIntent::FindReferences {
            buffer_id: BufferId(7),
            position: TextCoordinate {
                line: 0,
                character: 0,
                byte_offset: Some(0),
                utf16_offset: None,
            },
        })
    );
}

/// A command that names no buffer cannot be routed, and says so instead of
/// guessing at one.
#[test]
fn the_new_commands_refuse_when_no_buffer_is_open() {
    let mut shell = Shell::empty("lsp-reads");
    assert!(
        shell.handle_command(":inlayhints").is_err(),
        "inlay hints with no active buffer must not invent a target"
    );
    assert!(
        shell.handle_command(":codelens").is_err(),
        "code lenses with no active buffer must not invent a target"
    );
}
