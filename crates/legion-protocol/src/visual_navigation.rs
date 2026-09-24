//! Protocol DTOs for app-routed visual vertical navigation.

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::{BufferVersion, CaretAffinity, SnapshotId};

mod snapshot_id_hex {
    use serde::{Deserialize, Deserializer, Serializer};

    use crate::SnapshotId;

    pub fn serialize<S>(value: &SnapshotId, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&format!("{:032x}", value.0))
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<SnapshotId, D::Error>
    where
        D: Deserializer<'de>,
    {
        let encoded = String::deserialize(deserializer)?;
        let value = u128::from_str_radix(encoded.strip_prefix("0x").unwrap_or(&encoded), 16)
            .map_err(serde::de::Error::custom)?;
        Ok(SnapshotId(value))
    }
}

/// A UTF-8 byte position used by shaped visual navigation facts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct VisualNavigationPosition {
    /// Zero-based logical line.
    pub line: u32,
    /// UTF-8 byte column within the logical line.
    pub byte_column: u64,
}

/// A finite rendered X coordinate supplied by the renderer.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct VisualNavigationX {
    /// Row-local rendered X coordinate.
    pub value: f32,
}

impl PartialEq for VisualNavigationX {
    fn eq(&self, other: &Self) -> bool {
        self.value.to_bits() == other.value.to_bits()
    }
}

impl Eq for VisualNavigationX {}

/// A shaped caret stop on a visual row.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VisualNavigationStop {
    /// Valid UTF-8 source position represented by the stop.
    pub position: VisualNavigationPosition,
    /// Row-local rendered X coordinate.
    pub x: VisualNavigationX,
    /// Wrap-side affinity at the stop.
    pub affinity: CaretAffinity,
}

/// Renderer-shaped visual row facts.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VisualNavigationRow {
    /// Logical line containing this row.
    pub logical_line: u32,
    /// Optional zero-based visual-row index.
    pub row_index: Option<u32>,
    /// Optional visual-row count for the logical line.
    pub row_count: Option<u32>,
    /// Inclusive row start position.
    pub start: VisualNavigationPosition,
    /// Inclusive row end position.
    pub end: VisualNavigationPosition,
    /// Ordered caret stops on this row.
    pub stops: Vec<VisualNavigationStop>,
}

/// Shaped source row facts for one ordered caret.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VisualNavigationSourceRow {
    /// Source visual row.
    pub row: VisualNavigationRow,
    /// Source row-local rendered X.
    pub source_x: VisualNavigationX,
}

/// One ordered caret expected by a visual navigation request.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VisualNavigationCaret {
    /// Caret head.
    pub head: VisualNavigationPosition,
    /// Optional directed anchor.
    pub anchor: Option<VisualNavigationPosition>,
    /// Wrap-side affinity.
    pub affinity: CaretAffinity,
    /// Editor-owned preferred rendered X, when initialized.
    pub preferred_x: Option<VisualNavigationX>,
}

/// Vertical movement direction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum VisualNavigationDirection {
    /// Move to the preceding visual row.
    Up,
    /// Move to the following visual row.
    Down,
}

/// Opaque nonzero identity for shaped layout facts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct VisualNavigationLayoutId(pub u128);

impl Serialize for VisualNavigationLayoutId {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&format!("{:032x}", self.0))
    }
}

impl<'de> Deserialize<'de> for VisualNavigationLayoutId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let encoded = String::deserialize(deserializer)?;
        let value = u128::from_str_radix(encoded.strip_prefix("0x").unwrap_or(&encoded), 16)
            .map_err(serde::de::Error::custom)?;
        Ok(Self(value))
    }
}

/// Atomic app request for moving all ordered carets vertically.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VisualNavigationRequest {
    /// Snapshot expected by the renderer.
    #[serde(with = "snapshot_id_hex")]
    pub expected_snapshot_id: SnapshotId,
    /// Buffer version expected by the renderer.
    pub expected_buffer_version: BufferVersion,
    /// Exact ordered caret vector used to shape the facts.
    pub expected_carets: Vec<VisualNavigationCaret>,
    /// Nonzero shaped-layout identity.
    pub layout_id: VisualNavigationLayoutId,
    /// Requested direction.
    pub direction: VisualNavigationDirection,
    /// Whether to preserve or initialize directed anchors.
    pub extend: bool,
    /// Source facts aligned with expected carets.
    pub source_rows: Vec<VisualNavigationSourceRow>,
    /// Target facts aligned with expected carets.
    pub target_rows: Vec<VisualNavigationRow>,
}

/// Read-only visual navigation state projected by app authority.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VisualNavigationProjection {
    /// Current snapshot identity.
    #[serde(with = "snapshot_id_hex")]
    pub snapshot_id: SnapshotId,
    /// Current buffer version.
    pub buffer_version: BufferVersion,
    /// Total logical lines in the current snapshot.
    pub logical_line_count: u32,
    /// Exact ordered editor carets.
    pub carets: Vec<VisualNavigationCaret>,
}

/// A bounded logical-line fragment around a requested position.
///
/// All byte offsets are absolute snapshot offsets. Grapheme boundaries are
/// supplied by the text authority and are never reconstructed by the app or UI.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VisualNavigationWindow {
    /// Snapshot identity for the fragment.
    #[serde(with = "snapshot_id_hex")]
    pub snapshot_id: SnapshotId,
    /// Buffer version for the fragment.
    pub buffer_version: BufferVersion,
    /// Logical line containing the requested position.
    pub line: u32,
    /// Absolute logical-line start byte offset.
    pub line_start_byte: u64,
    /// Absolute primary-caret byte offset.
    pub caret_byte: u64,
    /// Inclusive fragment start byte offset.
    pub start_byte: u64,
    /// Exclusive fragment end byte offset.
    pub end_byte: u64,
    /// Exclusive logical-content end byte offset.
    pub logical_end_byte: u64,
    /// Whether the fragment includes the logical line start.
    pub complete_logical_start: bool,
    /// Whether the fragment includes the logical line end.
    pub complete_logical_end: bool,
    /// Absolute byte offsets of true extended-grapheme boundaries.
    pub grapheme_boundaries: Vec<u64>,
    /// Bounded UTF-8 text fragment.
    pub text: String,
}
