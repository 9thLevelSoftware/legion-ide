//! Bounded, lossless annotation metadata for immutable workspace-edit proposals.

use std::collections::BTreeMap;

use legion_protocol::{WorkspaceEditAnnotationTarget, WorkspaceEditChangeAnnotation};
use serde_json::Value;

use super::TranslationError;

/// Compatibility pins for Pyright artifacts whose implicit `default`
/// annotation must be given an explicit review description.
///
/// Each row is `(server_id, release, sha256)`. Bumping Pyright is a new row
/// in this table, not an edit of a buried literal inside the matcher.
pub(crate) const PYRIGHT_PINNED_DEFAULT_ANNOTATION: &[(u64, &str, &str)] = &[(
    104,
    "1.1.400",
    "2ccba7af9c8b14bb81c8fa9bb558d8b5181b586ec4dfc448b78eb4209e7a429a",
)];

fn pinned_pyright_artifact(health: &legion_protocol::LspServerHealthRecord) -> bool {
    PYRIGHT_PINNED_DEFAULT_ANNOTATION
        .iter()
        .any(|(server_id, _, hash)| {
            health.server_id == legion_protocol::LanguageServerId(*server_id)
                && health.language_id.0 == "python"
                && health.artifact_hash.as_ref().is_some_and(|fingerprint| {
                    fingerprint.algorithm == "sha256" && fingerprint.value == *hash
                })
        })
}

/// Pyright 1.1.400 emits an implicit `default` reference without a definition.
/// Scope this compatibility rule to the verified artifact, retain the reference,
/// and require review of an explicitly client-generated explanation. Other
/// missing definitions still fail strict translation.
pub(crate) fn normalize_pyright_annotations<'a>(
    raw: &'a Value,
    health: Option<&legion_protocol::LspServerHealthRecord>,
) -> std::borrow::Cow<'a, Value> {
    let pinned_pyright = health.is_some_and(pinned_pyright_artifact);
    if !pinned_pyright || raw.get("changeAnnotations").is_some() {
        return std::borrow::Cow::Borrowed(raw);
    }
    let implicit_default = raw
        .get("documentChanges")
        .and_then(Value::as_array)
        .is_some_and(|changes| {
            changes.iter().any(|change| {
                change.get("annotationId").and_then(Value::as_str) == Some("default")
                    || change
                        .get("edits")
                        .and_then(Value::as_array)
                        .is_some_and(|edits| {
                            edits.iter().any(|edit| {
                                edit.get("annotationId").and_then(Value::as_str) == Some("default")
                            })
                        })
            })
        });
    if !implicit_default {
        return std::borrow::Cow::Borrowed(raw);
    }
    let mut normalized = raw.clone();
    normalized["changeAnnotations"] = serde_json::json!({
        "default": {
            "label": "Review Python changes",
            "description": "The verified Pyright server omitted the description for its default change annotation. Review every affected edit before approving.",
            "needsConfirmation": true
        }
    });
    std::borrow::Cow::Owned(normalized)
}

const MAX_ANNOTATIONS: usize = 256;
const MAX_TARGETS: usize = 4096;
const MAX_METADATA_BYTES: usize = 64 * 1024;

fn malformed(reason: &str) -> TranslationError {
    TranslationError::MalformedEdit {
        reason: format!("change annotation: {reason}"),
    }
}

pub(super) fn translate_annotations(
    raw: &Value,
) -> Result<Vec<WorkspaceEditChangeAnnotation>, TranslationError> {
    let mut annotations = BTreeMap::new();
    let mut metadata_bytes = 0usize;
    if let Some(map) = raw.get("changeAnnotations") {
        let map = map
            .as_object()
            .ok_or_else(|| malformed("map must be an object"))?;
        if map.len() > MAX_ANNOTATIONS {
            return Err(malformed("too many annotations"));
        }
        for (id, value) in map {
            let label = value
                .get("label")
                .and_then(Value::as_str)
                .ok_or_else(|| malformed("label must be a string"))?;
            let description = match value.get("description") {
                None => None,
                Some(value) => Some(
                    value
                        .as_str()
                        .ok_or_else(|| malformed("description must be a string"))?,
                ),
            };
            let needs_confirmation = match value.get("needsConfirmation") {
                None => false,
                Some(value) => value
                    .as_bool()
                    .ok_or_else(|| malformed("needsConfirmation must be boolean"))?,
            };
            if id.len() > 256
                || label.len() > 1024
                || description.is_some_and(|text| text.len() > 8192)
            {
                return Err(malformed("individual metadata limit exceeded"));
            }
            metadata_bytes = metadata_bytes
                .saturating_add(id.len() + label.len() + description.map_or(0, str::len));
            if metadata_bytes > MAX_METADATA_BYTES {
                return Err(malformed("aggregate metadata limit exceeded"));
            }
            annotations.insert(
                id.clone(),
                WorkspaceEditChangeAnnotation {
                    id: id.clone(),
                    label: label.to_owned(),
                    description: description.map(str::to_owned),
                    needs_confirmation,
                    targets: Vec::new(),
                },
            );
        }
    }
    let mut target_count = 0usize;
    let mut associate = |value: &Value, target| -> Result<(), TranslationError> {
        let Some(id) = value.get("annotationId") else {
            return Ok(());
        };
        let id = id
            .as_str()
            .ok_or_else(|| malformed("annotationId must be a string"))?;
        let annotation = annotations
            .get_mut(id)
            .ok_or_else(|| malformed("annotationId has no definition"))?;
        target_count += 1;
        if target_count > MAX_TARGETS {
            return Err(malformed("too many annotated targets"));
        }
        annotation.targets.push(target);
        Ok(())
    };
    if let Some(changes) = raw.get("documentChanges").and_then(Value::as_array) {
        let mut file_edit_index = 0u32;
        let mut operation_index = 0u32;
        for change in changes {
            if change.get("kind").and_then(Value::as_str).is_some() {
                associate(
                    change,
                    WorkspaceEditAnnotationTarget::FileOperation { operation_index },
                )?;
                operation_index = operation_index
                    .checked_add(1)
                    .ok_or_else(|| malformed("operation index overflow"))?;
            } else if let Some(edits) = change.get("edits").and_then(Value::as_array) {
                for (index, edit) in edits.iter().enumerate() {
                    let edit_index =
                        u32::try_from(index).map_err(|_| malformed("edit index overflow"))?;
                    associate(
                        edit,
                        WorkspaceEditAnnotationTarget::TextEdit {
                            file_edit_index,
                            edit_index,
                        },
                    )?;
                }
                file_edit_index = file_edit_index
                    .checked_add(1)
                    .ok_or_else(|| malformed("file index overflow"))?;
            }
        }
    } else if let Some(changes) = raw.get("changes").and_then(Value::as_object) {
        // Legacy TextEdit arrays cannot carry annotations in LSP. Never silently
        // discard an annotation reference supplied in the wrong representation.
        for edits in changes.values().filter_map(Value::as_array) {
            if edits.iter().any(|edit| edit.get("annotationId").is_some()) {
                return Err(malformed("legacy changes cannot carry annotationId"));
            }
        }
    }
    Ok(annotations.into_values().collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn pyright_compatibility_table_names_at_least_one_release() {
        assert!(
            !PYRIGHT_PINNED_DEFAULT_ANNOTATION.is_empty(),
            "the Pyright annotation pin table must name at least one release"
        );
        assert!(
            PYRIGHT_PINNED_DEFAULT_ANNOTATION
                .iter()
                .any(|(server_id, version, hash)| {
                    *server_id == 104 && !version.is_empty() && hash.len() == 64
                }),
            "the table must keep a named Pyright release with a 64-character SHA-256"
        );
    }

    #[test]
    fn only_pinned_pyright_implicit_default_gets_explicit_review_metadata() {
        let mut health = legion_protocol::LspServerHealthRecord {
            server_id: legion_protocol::LanguageServerId(104),
            language_id: legion_protocol::LanguageId("python".into()),
            binary_provenance: legion_protocol::LspServerBinaryProvenance::Downloaded,
            binary_path_hash: None,
            artifact_hash: Some(legion_protocol::FileFingerprint {
                algorithm: "sha256".into(),
                value: PYRIGHT_PINNED_DEFAULT_ANNOTATION[0].2.into(),
            }),
            version: None,
            init_status: legion_protocol::LspResultStatus::Fresh,
            capabilities: Vec::new(),
            diagnostics_latency_ms: None,
            restart_count: 0,
            download_decision_id: None,
            schema_version: 1,
        };
        let raw =
            json!({"documentChanges":[{"textDocument":{},"edits":[{"annotationId":"default"}]}]});
        assert!(translate_annotations(&raw).is_err());
        let normalized = normalize_pyright_annotations(&raw, Some(&health));
        let annotations = translate_annotations(&normalized).unwrap();
        assert!(annotations[0].needs_confirmation);
        assert!(annotations[0]
            .description
            .as_ref()
            .unwrap()
            .contains("omitted"));
        assert_eq!(normalized["documentChanges"], raw["documentChanges"]);
        let mut explicit = raw.clone();
        explicit["changeAnnotations"] =
            json!({"default":{"label":"Actual server label","needsConfirmation":true}});
        assert_eq!(
            *normalize_pyright_annotations(&explicit, Some(&health)),
            explicit
        );
        let mut dangling = raw.clone();
        dangling["documentChanges"][0]["edits"][0]["annotationId"] = json!("unknown");
        assert!(
            translate_annotations(&normalize_pyright_annotations(&dangling, Some(&health)))
                .is_err()
        );
        health.artifact_hash = None;
        assert!(
            translate_annotations(&normalize_pyright_annotations(&raw, Some(&health))).is_err()
        );
    }

    #[test]
    fn shared_annotation_preserves_metadata_and_mixed_target_order() {
        let raw = json!({
            "changeAnnotations": {"rename": {"label":"Rename λ", "description":"Includes strings", "needsConfirmation":true}},
            "documentChanges": [
                {"kind":"rename", "annotationId":"rename"},
                {"textDocument":{}, "edits":[{}, {"annotationId":"rename"}]},
                {"textDocument":{}, "edits":[{"annotationId":"rename"}]}
            ]
        });
        let annotations = translate_annotations(&raw).unwrap();
        assert_eq!(annotations.len(), 1);
        let annotation = &annotations[0];
        assert_eq!(annotation.label, "Rename λ");
        assert_eq!(annotation.description.as_deref(), Some("Includes strings"));
        assert!(annotation.needs_confirmation);
        assert_eq!(
            annotation.targets,
            vec![
                WorkspaceEditAnnotationTarget::FileOperation { operation_index: 0 },
                WorkspaceEditAnnotationTarget::TextEdit {
                    file_edit_index: 0,
                    edit_index: 1
                },
                WorkspaceEditAnnotationTarget::TextEdit {
                    file_edit_index: 1,
                    edit_index: 0
                },
            ]
        );
    }

    #[test]
    fn undefined_and_malformed_annotations_fail_closed() {
        for raw in [
            json!({"documentChanges":[{"textDocument":{}, "edits":[{"annotationId":"default"}]}]}),
            json!({"changeAnnotations":null}),
            json!({"changeAnnotations":{"x":{"label":"x", "needsConfirmation":"yes"}}}),
            json!({"changeAnnotations":{"x":{"label":"x", "description":null}}}),
            json!({"changeAnnotations":{"x":{"label":"x"}}, "changes":{"file:///x":[{"annotationId":"x"}]}}),
        ] {
            assert!(matches!(
                translate_annotations(&raw),
                Err(TranslationError::MalformedEdit { .. })
            ));
        }
    }

    #[test]
    fn oversized_metadata_is_rejected_without_truncating_review() {
        let raw = json!({"changeAnnotations":{"x":{"label":"a".repeat(1025)}}});
        assert!(translate_annotations(&raw).is_err());
    }
}
