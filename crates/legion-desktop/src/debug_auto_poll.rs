//! Pure helpers for B8 desktop auto-poll after non-blocking live continue.

use legion_ui::{DebugProjection, DebugStatusKindProjection};

/// Whether the desktop frame loop should dispatch [`crate::bridge::DesktopAction::PollDebugSession`].
///
/// True only for live adapter sessions that are still Running after B7 continue
/// (fixture continue remains synchronous and does not need a poll loop).
pub fn debug_needs_auto_poll(debug: &DebugProjection) -> bool {
    debug.live_adapter
        && debug.active_session_id.is_some()
        && debug.status.kind == DebugStatusKindProjection::Running
}

#[cfg(test)]
mod tests {
    use super::*;
    use legion_protocol::DebugSessionId;
    use legion_ui::DebugStatusProjection;

    fn projection(live: bool, running: bool, session: bool) -> DebugProjection {
        let mut debug = DebugProjection::empty();
        debug.live_adapter = live;
        if session {
            debug.active_session_id = Some(DebugSessionId("debug-1".to_string()));
        }
        debug.status = DebugStatusProjection {
            kind: if running {
                DebugStatusKindProjection::Running
            } else {
                DebugStatusKindProjection::Paused
            },
            message: "test".to_string(),
        };
        debug
    }

    #[test]
    fn auto_poll_only_when_live_running_with_session() {
        assert!(debug_needs_auto_poll(&projection(true, true, true)));
        assert!(!debug_needs_auto_poll(&projection(false, true, true)));
        assert!(!debug_needs_auto_poll(&projection(true, false, true)));
        assert!(!debug_needs_auto_poll(&projection(true, true, false)));
        assert!(!debug_needs_auto_poll(&DebugProjection::empty()));
    }
}
