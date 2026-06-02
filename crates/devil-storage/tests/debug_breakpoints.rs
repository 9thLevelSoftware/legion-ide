use devil_protocol::{
    CanonicalPath, CausalityId, CorrelationId, DebugBreakpointId, DebugBreakpointRecord,
    EventSequence, ProtocolTextRange, RedactionHint, StorageRepositoryPort,
    StorageRepositoryRequest, StorageRepositoryResponse, TextCoordinate, WorkspaceId,
};
use devil_storage::InMemoryStorageRepositoryPort;
use uuid::Uuid;

fn range() -> ProtocolTextRange {
    ProtocolTextRange {
        start: TextCoordinate {
            line: 4,
            character: 0,
            byte_offset: Some(48),
            utf16_offset: Some(48),
        },
        end: TextCoordinate {
            line: 4,
            character: 0,
            byte_offset: Some(48),
            utf16_offset: Some(48),
        },
    }
}

#[test]
fn debug_breakpoints_persist_by_workspace_independent_of_sessions() {
    let repo = InMemoryStorageRepositoryPort::new();
    let record = DebugBreakpointRecord {
        breakpoint_id: DebugBreakpointId("bp-1".to_string()),
        workspace_id: WorkspaceId(11),
        session_id: None,
        path: CanonicalPath("C:/repo/src/main.rs".to_string()),
        range: range(),
        enabled: true,
        condition: Some("count > 2".to_string()),
        hit_condition: Some("3".to_string()),
        log_message: Some("count changed".to_string()),
        verified: false,
        message: Some("pending adapter verification".to_string()),
        correlation_id: CorrelationId(900),
        causality_id: CausalityId(Uuid::parse_str("aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa").unwrap()),
        sequence: EventSequence(1),
        schema_version: 1,
    };

    let saved = repo
        .handle(StorageRepositoryRequest::SaveDebugBreakpointRecord(
            record.clone(),
        ))
        .expect("breakpoint should save");
    assert!(matches!(saved, StorageRepositoryResponse::Saved { .. }));

    let loaded = repo
        .handle(StorageRepositoryRequest::ReadDebugBreakpointRecords {
            workspace_id: WorkspaceId(11),
        })
        .expect("breakpoints should load");
    match loaded {
        StorageRepositoryResponse::DebugBreakpointRecords(records) => {
            assert_eq!(records, vec![record.clone()]);
            assert_eq!(records[0].session_id, None);
            assert_eq!(
                records[0].redaction_hints(),
                vec![RedactionHint::MetadataOnly]
            );
        }
        other => panic!("expected debug breakpoint records, got {other:?}"),
    }

    let deleted = repo
        .handle(StorageRepositoryRequest::DeleteDebugBreakpointRecord {
            workspace_id: WorkspaceId(11),
            breakpoint_id: record.breakpoint_id.clone(),
        })
        .expect("breakpoint delete should be idempotent");
    assert!(matches!(deleted, StorageRepositoryResponse::Saved { .. }));

    let loaded_after_delete = repo
        .handle(StorageRepositoryRequest::ReadDebugBreakpointRecords {
            workspace_id: WorkspaceId(11),
        })
        .expect("breakpoints should load after delete");
    assert!(matches!(
        loaded_after_delete,
        StorageRepositoryResponse::DebugBreakpointRecords(records) if records.is_empty()
    ));

    let deleted_again = repo
        .handle(StorageRepositoryRequest::DeleteDebugBreakpointRecord {
            workspace_id: WorkspaceId(11),
            breakpoint_id: record.breakpoint_id,
        })
        .expect("missing breakpoint delete should still succeed");
    assert!(matches!(
        deleted_again,
        StorageRepositoryResponse::Saved { .. }
    ));
}
