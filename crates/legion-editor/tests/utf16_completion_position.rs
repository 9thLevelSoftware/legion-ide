//! A UTF-16 completion position must address the character it names.
//!
//! LSP positions are UTF-16, so `TextOffset::utf16` is the encoding a real language-server
//! client sends; `TextOffset::byte` is the internal convenience. Until 2026-08-17 the two
//! disagreed at the start of every line but the first, because
//! `EditorEngine::byte_offset_from_absolute_utf16` walked lines subtracting lengths and
//! clamped an offset landing on a line ending back to that line's content end before
//! considering the next line — so a residual of zero was unreachable and a line start
//! resolved to the end of the previous line.
//!
//! The resolved byte offset is not returned by `completion`, but it is observable:
//! completions are filtered by the identifier prefix ending at the resolved offset. At the
//! start of a line the prefix is empty and everything in scan range is offered; at the end
//! of the previous line the prefix is that line's trailing identifier and only it matches.
//! That difference is what these tests read.

use legion_editor::EditorEngine;
use legion_protocol::{CompletionRequest, CorrelationId, FileId, TextOffset, WorkspaceId};

/// Ask for completions at `position`, returning the item labels.
fn labels_at(
    engine: &EditorEngine,
    snapshot: legion_protocol::SnapshotId,
    position: TextOffset,
) -> Vec<String> {
    engine
        .completion(CompletionRequest {
            workspace_id: WorkspaceId(1),
            file_id: FileId(2),
            snapshot_id: snapshot,
            position,
            correlation_id: CorrelationId(1),
        })
        .expect("completion request")
        .items
        .into_iter()
        .map(|item| item.label)
        .collect()
}

/// Open `text` and return the engine plus its snapshot id.
fn open(text: &str) -> (EditorEngine, legion_protocol::SnapshotId) {
    let mut engine = EditorEngine::new();
    let buffer = engine
        .open_buffer(WorkspaceId(1), FileId(2), "probe.rs", text.to_string())
        .expect("open buffer");
    let snapshot = engine
        .buffer_metadata(buffer)
        .expect("buffer metadata")
        .snapshot_id;
    (engine, snapshot)
}

#[test]
fn utf16_and_byte_encodings_agree_on_a_line_start() {
    // All ASCII, so byte offset and UTF-16 offset are the same number and any
    // disagreement between the encodings is the resolver's, not the text's.
    let text = "alpha_one\nbravo_two\n";
    let (engine, snapshot) = open(text);

    // "alpha_one\n" is ten bytes, so offset 10 is the start of the second line.
    let start_of_second_line = 10u64;
    assert_eq!(
        labels_at(&engine, snapshot, TextOffset::utf16(start_of_second_line)),
        labels_at(&engine, snapshot, TextOffset::byte(start_of_second_line)),
        "the two encodings of the same position must resolve identically"
    );
}

#[test]
fn a_utf16_position_at_a_line_start_has_no_identifier_prefix() {
    let text = "alpha_one\nbravo_two\n";
    let (engine, snapshot) = open(text);

    // At the start of the second line there is no prefix, so both identifiers in scan
    // range are offered. Resolving to the end of the first line instead would filter the
    // list down to `alpha_one`, which is the bug this pins.
    let labels = labels_at(&engine, snapshot, TextOffset::utf16(10));
    assert!(
        labels.iter().any(|label| label == "bravo_two"),
        "a line start must not inherit the previous line's identifier as a prefix; got {labels:?}"
    );
}

#[test]
fn a_utf16_position_inside_a_line_still_filters_by_its_prefix() {
    // The clamping behaviour that was correct is still correct: a position in the middle
    // of an identifier filters by the prefix ending there.
    let text = "alpha_one\nalpha_two\nbravo\n";
    let (engine, snapshot) = open(text);

    // Offset 5 is inside "alpha_one", after "alpha".
    let labels = labels_at(&engine, snapshot, TextOffset::utf16(5));
    assert!(
        labels.iter().all(|label| label.starts_with("alpha")),
        "a position inside an identifier must filter by its prefix; got {labels:?}"
    );
}

#[test]
fn a_utf16_position_past_the_end_of_the_buffer_is_rejected() {
    let text = "alpha\n";
    let (engine, snapshot) = open(text);

    let error = engine
        .completion(CompletionRequest {
            workspace_id: WorkspaceId(1),
            file_id: FileId(2),
            snapshot_id: snapshot,
            position: TextOffset::utf16(999),
            correlation_id: CorrelationId(1),
        })
        .expect_err("an offset past the end must be refused, not clamped");
    assert!(
        format!("{error:?}").contains("utf16 offset outside buffer"),
        "unexpected error for an out-of-range offset: {error:?}"
    );
}

#[test]
fn a_utf16_position_addresses_characters_not_bytes() {
    // Three-byte characters: byte offsets and UTF-16 offsets diverge, which is the whole
    // reason the UTF-16 encoding exists. "日本語\n" is 9 bytes but 4 UTF-16 units, so the
    // second line starts at UTF-16 offset 4 and byte offset 10.
    let text = "日本語\nbravo_two\n";
    let (engine, snapshot) = open(text);

    assert_eq!(
        labels_at(&engine, snapshot, TextOffset::utf16(4)),
        labels_at(&engine, snapshot, TextOffset::byte(10)),
        "the same position expressed in each encoding must resolve identically"
    );
}
