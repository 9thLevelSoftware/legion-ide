//! Named MCP peer metadata; endpoint addresses and credential values are never retained here.

use crate::{McpServerId, McpTransportKind};
use serde::{Deserialize, Serialize};

/// Protocol revision pinned by the existing client/server substrate.
pub const MCP_PEER_PROTOCOL_VERSION: &str = "2025-11-25";

/// Legion's selected role in an MCP relationship.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum McpPeerRole {
    /// Legion consumes a peer's primitives.
    Client,
    /// Legion exposes only app-registered primitives over local stdio.
    Server,
}

/// Explicit authentication route; never an inference-provider credential fallback.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum McpPeerAuthentication {
    /// Deliberately unauthenticated.
    None,
    /// Peer-specific SecretStore reference, sent only to the selected HTTP endpoint.
    Bearer,
}

/// Supported retention contract for the bounded configuration slice.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum McpPeerPrivacy {
    /// Retain metadata; raw payload/trace retention is not activated here.
    MetadataOnly,
}

/// Named configuration metadata safe to retain without raw-trace consent.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct McpPeerMetadata {
    /// Stable peer identity.
    pub peer_id: McpServerId,
    /// Operator-supplied display-safe label.
    pub display_label: String,
    /// Selected Legion role.
    pub role: McpPeerRole,
    /// Pinned protocol revision; unsupported revisions fail closed.
    pub protocol_version: String,
    /// Selected transport family.
    pub transport: McpTransportKind,
    /// Selected authentication route.
    pub authentication: McpPeerAuthentication,
    /// Declared scopes reviewed with the credential; not a claim of remote enforcement.
    pub credential_scopes: Vec<String>,
    /// Explicit retention contract; credentials and raw exchanges are excluded.
    pub privacy: McpPeerPrivacy,
}

/// Observed local lifecycle; Ready is a protocol check, not live qualification.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum McpPeerHealth {
    /// Configuration validated without network/process activity.
    Configured,
    /// Selected peer answered negotiation and a health request.
    Ready,
    /// Activation or health request failed; runtime reuse is disabled.
    Unavailable,
    /// Local grant/runtime revoked; remote token invalidation is not implied.
    Revoked,
}

/// Observable credential-deletion outcome, independent of grant revocation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum McpPeerCredentialState {
    /// No credential deletion has been attempted.
    Unchanged,
    /// SecretStore reported deletion success.
    Deleted,
    /// SecretStore could not confirm deletion; runtime remains revoked.
    DeletionFailed,
}

/// Metadata-only app projection of one named peer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct McpPeerSnapshot {
    /// Reviewed configuration metadata.
    pub metadata: McpPeerMetadata,
    /// Monotonic app revision; binds activation/grants to the exact configuration.
    pub revision: u64,
    /// Last observed local health.
    pub health: McpPeerHealth,
    /// Whether the current revision has an explicit transport grant.
    pub transport_granted: bool,
    /// Secret deletion outcome.
    pub credential_state: McpPeerCredentialState,
}
