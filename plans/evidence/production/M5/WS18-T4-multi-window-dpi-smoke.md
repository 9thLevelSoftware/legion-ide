# M5 — WS18.T4 Multi-window / Multi-monitor / DPI Smoke Evidence

## Status

Blocked on runner hardware: the current macOS runner exposes only one active display, so a real multi-monitor/per-monitor-DPI smoke cannot be performed here.

## Acceptance target

- Validate the recorded gap for per-monitor DPI, window restore, and detachable panels (if kept in scope).
- Produce multi-monitor smoke evidence if the environment supports it.

## Evidence gathered on this runner

### Display inventory

- `system_profiler SPDisplaysDataType`
  - Passed.
  - Output reports a single online display:
    - `Resolution: 1920 x 1080 (1080p FHD - Full High Definition)`
    - `UI Looks like: 1920 x 1080 @ 60.00Hz`
    - `Main Display: Yes`
    - `Mirror: Off`

### Desktop smoke / projection evidence

- `cargo test -p legion-desktop --test platform_smoke -- --nocapture`
  - Passed: 6 tests.
  - Confirms the desktop smoke harness still writes the evidence file and keeps the adapter/projection surface intact.

- `cargo test -p legion-desktop --test platform_integration -- --nocapture`
  - Passed: 2 tests.
  - Confirms the platform smoke snapshot records the projected adapter status, and the high-DPI field can represent an observed scale when such an observation exists.

### Session restore / window state evidence

- `cargo test -p legion-desktop --test session_restore -- --nocapture`
  - Passed: 6 tests.
  - Confirms session state persists and restores tabs, focus/layout/explorer state, and dock layout metadata.
  - This covers the window/session-restore portion of the WS18.T4 gap at the projection layer.

## Current conclusion

- Window/session restore is covered by passing tests.
- The code path can represent high-DPI observations, but this runner does not expose a non-1x or multi-monitor setup to smoke them.
- No detachable-panel product smoke was executed because the task scope keeps it "if kept in scope" and there is no multi-display environment here to validate the full multi-window story.

## Blocker

A real multi-monitor smoke requires a runner with at least two active displays or a virtual-display setup that exposes per-monitor scaling changes. This host currently has one 1920x1080 display only, so the acceptance target cannot be fully satisfied here.
