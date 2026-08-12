# 06-03 Result: Platform and Accessibility Smoke

## Summary

Added metadata-only platform smoke projection for menu, shortcut, adapter path, theme, high-DPI, focus traversal, and accessibility tree coverage. Wired smoke evidence to use the shared desktop native-window options and to record projection accessibility status separately from OS accessibility observation.

## Files Changed

- `crates/devil-desktop/src/platform.rs`
- `crates/devil-desktop/src/lib.rs`
- `crates/devil-desktop/src/smoke.rs`
- `crates/devil-desktop/src/workflow.rs`
- `crates/devil-desktop/tests/platform_integration.rs`
- `crates/devil-desktop/tests/platform_smoke.rs`

## Verification

- Passed: `cargo test -p devil-desktop --test platform_integration -- --nocapture`
- Passed: `cargo test -p devil-desktop --test platform_smoke -- --nocapture`
