//! DAP lifecycle state model.

use legion_protocol::DebugSessionState;

/// Internal DAP client lifecycle state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DapLifecycleState {
    /// Runtime is configured but no adapter session has started.
    Configured,
    /// Adapter process/session launch has been requested.
    Launching,
    /// DAP initialize handshake completed.
    Initialized,
    /// Launch/configuration completed and execution is paused.
    Paused,
    /// Adapter/session exited.
    Exited,
    /// Adapter/session failed.
    Failed,
}

impl DapLifecycleState {
    /// Convert to the public protocol session state.
    pub fn as_debug_session_state(self) -> DebugSessionState {
        match self {
            Self::Configured => DebugSessionState::Configured,
            Self::Launching => DebugSessionState::Launching,
            Self::Initialized => DebugSessionState::Running,
            Self::Paused => DebugSessionState::Paused,
            Self::Exited => DebugSessionState::Exited,
            Self::Failed => DebugSessionState::Failed,
        }
    }
}
