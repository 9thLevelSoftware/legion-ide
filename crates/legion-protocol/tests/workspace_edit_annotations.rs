use legion_protocol::*;
use serde_json::json;
use uuid::Uuid;

fn empty_payload() -> WorkspaceEditProposalPayload {
    WorkspaceEditProposalPayload {
        workspace_id: WorkspaceId(7),
        edit_id: Uuid::from_u128(7),
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
        change_annotations: Vec::new(),
        required_capability: CapabilityId("fs.write".to_string()),
        diagnostics: Vec::new(),
        schema_version: 1,
    }
}

#[test]
fn workspace_edit_annotations_round_trip_unicode_confirmation_and_multiple_targets() {
    let annotation = WorkspaceEditChangeAnnotation {
        id: "default".to_string(),
        label: "Renommer привет 日本語".to_string(),
        description: Some("Review this cross-file change".to_string()),
        needs_confirmation: true,
        targets: vec![
            WorkspaceEditAnnotationTarget::TextEdit {
                file_edit_index: 0,
                edit_index: 1,
            },
            WorkspaceEditAnnotationTarget::TextEdit {
                file_edit_index: 1,
                edit_index: 0,
            },
            WorkspaceEditAnnotationTarget::FileOperation { operation_index: 0 },
        ],
    };

    let encoded = serde_json::to_value(&annotation).expect("annotation serializes");
    let decoded: WorkspaceEditChangeAnnotation =
        serde_json::from_value(encoded).expect("annotation deserializes");
    assert_eq!(decoded, annotation);
}

#[test]
fn workspace_edit_payload_omits_empty_annotations_and_accepts_legacy_json() {
    let payload = empty_payload();
    let encoded = serde_json::to_value(&payload).expect("payload serializes");
    assert!(encoded.get("change_annotations").is_none());

    let mut legacy = encoded;
    legacy
        .as_object_mut()
        .expect("payload object")
        .remove("change_annotations");
    let decoded: WorkspaceEditProposalPayload =
        serde_json::from_value(legacy).expect("legacy payload remains readable");
    assert!(decoded.change_annotations.is_empty());
}

#[test]
fn workspace_edit_annotation_wire_shape_is_explicit() {
    let value =
        serde_json::to_value(WorkspaceEditAnnotationTarget::FileOperation { operation_index: 3 })
            .expect("target serializes");
    assert_eq!(value, json!({"FileOperation": {"operation_index": 3}}));
}
