//! Guard an external input batch against an independently observed foreground
//! window. A lost target is blocked, never repaired by redirecting the batch.
use std::time::Duration;

pub const ATTENDED_FOREGROUND_TIMEOUT: Duration = Duration::from_secs(60);

#[derive(Debug, PartialEq, Eq)]
pub enum ForegroundObservation {
    Target,
    Other,
    WindowExited,
}

#[derive(Debug, PartialEq, Eq)]
pub enum AwaitForegroundOutcome {
    Ready,
    WindowExited,
    TimedOut,
}

/// Read-only waiting at the OS/clock boundary. Never activates a window or sends
/// input. Injected observations/time let headless checks exercise the deadline.
pub fn await_user_foreground(
    mut observe: impl FnMut() -> ForegroundObservation,
    mut elapsed: impl FnMut() -> Duration,
    mut pause: impl FnMut(),
) -> AwaitForegroundOutcome {
    loop {
        if elapsed() >= ATTENDED_FOREGROUND_TIMEOUT {
            return AwaitForegroundOutcome::TimedOut;
        }
        match observe() {
            ForegroundObservation::Target => return AwaitForegroundOutcome::Ready,
            ForegroundObservation::WindowExited => return AwaitForegroundOutcome::WindowExited,
            ForegroundObservation::Other => pause(),
        }
    }
}

pub fn guarded_batch<T: Eq>(
    expected: T,
    foreground: T,
    send: impl FnOnce() -> Result<(), String>,
) -> Result<(), String> {
    if foreground != expected {
        return Err("product lost foreground focus; input batch aborted".into());
    }
    send()
}
