//! Bounded, identity-fenced retention for raw LSP diagnostics.
//!
//! This cache is deliberately private to the application language layer.  It
//! keeps the original diagnostic objects so code-action requests can reuse
//! provider fields (`code`, `source`, and `data`) without projecting them into
//! a lossy UI representation.

use std::collections::{HashMap, VecDeque};

#[cfg(test)]
use legion_protocol::ProtocolTextRange;
use legion_protocol::{BufferId, BufferVersion, SnapshotId, Utf16Range};
use serde_json::Value;

const MAX_SERIALIZED_BYTES: usize = 256 * 1024;
const MAX_BUFFERS: usize = 32;
const MAX_DIAGNOSTICS_PER_BATCH: usize = 128;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct DiagnosticIdentity {
    pub(crate) buffer_id: BufferId,
    pub(crate) snapshot_id: SnapshotId,
    pub(crate) buffer_version: BufferVersion,
}

#[derive(Debug, Clone)]
struct CachedDiagnostic {
    value: Value,
    range: Utf16Range,
}

#[derive(Debug, Clone)]
struct CachedBatch {
    diagnostics: Vec<CachedDiagnostic>,
    serialized_bytes: usize,
}

#[derive(Debug, Default)]
pub(crate) struct CodeActionDiagnostics {
    entries: HashMap<DiagnosticIdentity, CachedBatch>,
    insertion_order: VecDeque<DiagnosticIdentity>,
    serialized_bytes: usize,
}

impl CodeActionDiagnostics {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    /// Replace the batch associated with `identity` from a publishDiagnostics
    /// params object. Any malformed, missing, or over-limit replacement first
    /// removes the prior batch, so stale diagnostics can never be returned.
    pub(crate) fn replace_from_params(&mut self, identity: DiagnosticIdentity, params: &Value) {
        // A buffer can only have one current diagnostic snapshot.  Retire all
        // older versions before admitting the replacement so stale versions
        // cannot consume the global retention budget.
        self.clear_buffer(identity.buffer_id);

        let Some(diagnostics) = params.get("diagnostics").and_then(Value::as_array) else {
            return;
        };
        // Check the count before touching/cloning any diagnostic value.
        if diagnostics.len() > MAX_DIAGNOSTICS_PER_BATCH {
            return;
        }
        let framing_bytes = 2 + diagnostics.len().saturating_sub(1);
        if diagnostics.is_empty() {
            return;
        }

        let mut parsed = Vec::with_capacity(diagnostics.len());
        let mut batch_bytes = 0usize;
        for diagnostic in diagnostics {
            let Some(range) = parse_range(diagnostic.get("range")) else {
                return;
            };
            let remaining = MAX_SERIALIZED_BYTES
                .saturating_sub(framing_bytes)
                .saturating_sub(batch_bytes);
            let Ok(serialized_bytes) =
                crate::language::bounded_code_action_size(diagnostic, remaining)
            else {
                return;
            };
            batch_bytes = batch_bytes.saturating_add(serialized_bytes);
            if batch_bytes > MAX_SERIALIZED_BYTES {
                return;
            }
            parsed.push((diagnostic, range, serialized_bytes));
        }

        let retained_batch_bytes = framing_bytes.saturating_add(batch_bytes);
        if self.serialized_bytes.saturating_add(retained_batch_bytes) > MAX_SERIALIZED_BYTES {
            return;
        }
        let cached = parsed
            .into_iter()
            .map(|(value, range, _serialized_bytes)| CachedDiagnostic {
                value: value.clone(),
                range,
            })
            .collect();
        self.serialized_bytes += retained_batch_bytes;
        self.entries.insert(
            identity,
            CachedBatch {
                diagnostics: cached,
                serialized_bytes: retained_batch_bytes,
            },
        );
        self.insertion_order.push_back(identity);
        self.enforce_buffer_limit();
    }

    /// Return cloned original diagnostic objects whose UTF-16 ranges intersect
    /// `query`, and only when all three identity fields match.
    pub(crate) fn query_utf16(
        &self,
        identity: DiagnosticIdentity,
        query: Utf16Range,
    ) -> Vec<Value> {
        if !range_is_valid(query) {
            return Vec::new();
        }
        self.entries
            .get(&identity)
            .into_iter()
            .flat_map(|batch| batch.diagnostics.iter())
            .filter(|diagnostic| ranges_intersect(diagnostic.range, query))
            .map(|diagnostic| diagnostic.value.clone())
            .collect()
    }

    /// Convenience query for tests and callers holding the protocol's
    /// line/character range. Byte offsets are intentionally ignored: LSP
    /// diagnostic ranges are UTF-16 coordinates.
    #[cfg(test)]
    pub(crate) fn query_protocol(
        &self,
        identity: DiagnosticIdentity,
        query: ProtocolTextRange,
    ) -> Vec<Value> {
        self.query_utf16(identity, protocol_to_utf16(query))
    }

    pub(crate) fn remove(&mut self, identity: DiagnosticIdentity) -> bool {
        let Some(entry) = self.entries.remove(&identity) else {
            return false;
        };
        self.serialized_bytes = self.serialized_bytes.saturating_sub(entry.serialized_bytes);
        self.insertion_order.retain(|item| *item != identity);
        true
    }

    pub(crate) fn clear_buffer(&mut self, buffer_id: BufferId) {
        let identities: Vec<_> = self
            .entries
            .keys()
            .filter(|identity| identity.buffer_id == buffer_id)
            .copied()
            .collect();
        for identity in identities {
            self.remove(identity);
        }
    }

    pub(crate) fn clear_session(&mut self) {
        self.entries.clear();
        self.insertion_order.clear();
        self.serialized_bytes = 0;
    }

    fn enforce_buffer_limit(&mut self) {
        while self
            .entries
            .keys()
            .map(|identity| identity.buffer_id)
            .collect::<std::collections::HashSet<_>>()
            .len()
            > MAX_BUFFERS
        {
            let Some(oldest_buffer) = self
                .insertion_order
                .iter()
                .map(|identity| identity.buffer_id)
                .find(|buffer_id| {
                    self.entries
                        .keys()
                        .any(|identity| identity.buffer_id == *buffer_id)
                })
            else {
                break;
            };
            self.clear_buffer(oldest_buffer);
        }
    }
}

fn parse_range(value: Option<&Value>) -> Option<Utf16Range> {
    let value = value?;
    let start = parse_position(value.get("start")?)?;
    let end = parse_position(value.get("end")?)?;
    if !range_is_valid(Utf16Range { start, end }) {
        return None;
    }
    Some(Utf16Range { start, end })
}

fn range_is_valid(range: Utf16Range) -> bool {
    position_key(range.start) <= position_key(range.end)
}

fn parse_position(value: &Value) -> Option<legion_protocol::Utf16Position> {
    Some(legion_protocol::Utf16Position {
        line: value.get("line")?.as_u64()?.try_into().ok()?,
        character: value.get("character")?.as_u64()?.try_into().ok()?,
    })
}

#[cfg(test)]
fn protocol_to_utf16(range: ProtocolTextRange) -> Utf16Range {
    Utf16Range {
        start: legion_protocol::Utf16Position {
            line: range.start.line,
            character: range.start.character,
        },
        end: legion_protocol::Utf16Position {
            line: range.end.line,
            character: range.end.character,
        },
    }
}

fn ranges_intersect(left: Utf16Range, right: Utf16Range) -> bool {
    let left_empty = left.start == left.end;
    let right_empty = right.start == right.end;
    if left_empty && right_empty {
        return left.start == right.start;
    }
    if left_empty {
        return position_key(right.start) <= position_key(left.start)
            && position_key(left.start) <= position_key(right.end);
    }
    if right_empty {
        return position_key(left.start) <= position_key(right.start)
            && position_key(right.start) <= position_key(left.end);
    }
    position_key(left.start) < position_key(right.end)
        && position_key(right.start) < position_key(left.end)
}

fn position_key(position: legion_protocol::Utf16Position) -> (u32, u32) {
    (position.line, position.character)
}

#[cfg(test)]
mod tests {
    use super::*;
    use legion_protocol::{TextCoordinate, Utf16Position};
    use serde_json::json;

    fn identity(n: u128) -> DiagnosticIdentity {
        DiagnosticIdentity {
            buffer_id: BufferId(n),
            snapshot_id: SnapshotId(n + 10),
            buffer_version: BufferVersion(n as u64 + 20),
        }
    }

    fn params(diagnostics: Value) -> Value {
        json!({"diagnostics": diagnostics})
    }

    fn diagnostic(start: u32, end: u32) -> Value {
        json!({
            "range": {"start": {"line": 1, "character": start}, "end": {"line": 1, "character": end}},
            "code": 42,
            "source": "rust-analyzer",
            "data": {"fix": "keep"},
            "message": "original"
        })
    }

    #[test]
    fn preserves_original_fields_and_fences_identity() {
        let mut cache = CodeActionDiagnostics::new();
        let key = identity(1);
        cache.replace_from_params(key, &params(json!([diagnostic(2, 5)])));
        let result = cache.query_utf16(
            key,
            Utf16Range {
                start: Utf16Position {
                    line: 1,
                    character: 3,
                },
                end: Utf16Position {
                    line: 1,
                    character: 4,
                },
            },
        );
        assert_eq!(result[0]["code"], 42);
        assert_eq!(result[0]["source"], "rust-analyzer");
        assert_eq!(result[0]["data"]["fix"], "keep");
        assert!(
            cache
                .query_utf16(
                    identity(2),
                    Utf16Range {
                        start: Utf16Position {
                            line: 1,
                            character: 3
                        },
                        end: Utf16Position {
                            line: 1,
                            character: 4
                        }
                    }
                )
                .is_empty()
        );
    }

    #[test]
    fn malformed_replacement_clears_stale_batch() {
        let mut cache = CodeActionDiagnostics::new();
        let key = identity(1);
        cache.replace_from_params(key, &params(json!([diagnostic(0, 1)])));
        cache.replace_from_params(key, &json!({"diagnostics": [{"message": "missing range"}]}));
        assert!(
            cache
                .query_utf16(
                    key,
                    Utf16Range {
                        start: Utf16Position {
                            line: 1,
                            character: 0
                        },
                        end: Utf16Position {
                            line: 1,
                            character: 1
                        }
                    }
                )
                .is_empty()
        );
    }

    #[test]
    fn query_uses_exact_half_open_intersection() {
        let mut cache = CodeActionDiagnostics::new();
        let key = identity(1);
        cache.replace_from_params(key, &params(json!([diagnostic(2, 5)])));
        let boundary = ProtocolTextRange {
            start: TextCoordinate {
                line: 1,
                character: 5,
                byte_offset: None,
                utf16_offset: None,
            },
            end: TextCoordinate {
                line: 1,
                character: 6,
                byte_offset: None,
                utf16_offset: None,
            },
        };
        assert!(cache.query_protocol(key, boundary).is_empty());
    }

    #[test]
    fn over_limit_batch_clears_previous_entry() {
        let mut cache = CodeActionDiagnostics::new();
        let key = identity(1);
        cache.replace_from_params(key, &params(json!([diagnostic(0, 1)])));
        let too_many: Vec<_> = (0..=MAX_DIAGNOSTICS_PER_BATCH)
            .map(|_| diagnostic(0, 1))
            .collect();
        cache.replace_from_params(key, &params(Value::Array(too_many)));
        assert!(
            cache
                .query_utf16(
                    key,
                    Utf16Range {
                        start: Utf16Position {
                            line: 1,
                            character: 0
                        },
                        end: Utf16Position {
                            line: 1,
                            character: 1
                        }
                    }
                )
                .is_empty()
        );
    }

    #[test]
    fn oversized_single_diagnostic_clears_previous_entry() {
        let mut cache = CodeActionDiagnostics::new();
        let key = identity(1);
        cache.replace_from_params(key, &params(json!([diagnostic(0, 1)])));
        let oversized = json!({
            "range": {"start": {"line": 1, "character": 0}, "end": {"line": 1, "character": 1}},
            "message": "x".repeat(MAX_SERIALIZED_BYTES + 1)
        });
        cache.replace_from_params(key, &params(json!([oversized])));
        assert!(
            cache
                .query_utf16(
                    key,
                    Utf16Range {
                        start: Utf16Position {
                            line: 1,
                            character: 0
                        },
                        end: Utf16Position {
                            line: 1,
                            character: 1
                        },
                    },
                )
                .is_empty()
        );
    }

    #[test]
    fn array_framing_counts_against_retention_budget() {
        let mut cache = CodeActionDiagnostics::new();
        let key = identity(1);
        let mut low = 0usize;
        let mut high = MAX_SERIALIZED_BYTES;
        while low < high {
            let candidate = (low + high).div_ceil(2);
            let value = json!({
                "range": {"start": {"line": 1, "character": 0}, "end": {"line": 1, "character": 1}},
                "message": "x".repeat(candidate)
            });
            if serde_json::to_vec(&value).expect("serializable").len() <= MAX_SERIALIZED_BYTES {
                low = candidate;
            } else {
                high = candidate - 1;
            }
        }
        let value = json!({
            "range": {"start": {"line": 1, "character": 0}, "end": {"line": 1, "character": 1}},
            "message": "x".repeat(low)
        });
        let object_bytes = serde_json::to_vec(&value).expect("serializable").len();
        assert!(object_bytes <= MAX_SERIALIZED_BYTES);
        assert!(object_bytes + 2 > MAX_SERIALIZED_BYTES);
        cache.replace_from_params(key, &params(json!([value])));
        assert!(
            cache
                .query_utf16(
                    key,
                    Utf16Range {
                        start: Utf16Position {
                            line: 1,
                            character: 0
                        },
                        end: Utf16Position {
                            line: 1,
                            character: 1
                        },
                    },
                )
                .is_empty()
        );
    }
}
