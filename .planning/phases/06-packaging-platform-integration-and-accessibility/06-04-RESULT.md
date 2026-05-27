# 06-04 Result: Session Safety and Diagnostics

## Summary

Changed desktop session persistence to validate and sync a same-directory temp file before publishing through platform replace semantics. Added metadata-only diagnostics export wiring and tests that verify diagnostics contain counts and platform smoke labels without editor text, false default adapter failures, or raw source markers.

## Files Changed

- `crates/devil-desktop/src/session.rs`
- `crates/devil-desktop/src/diagnostics.rs`
- `crates/devil-desktop/src/lib.rs`
- `crates/devil-desktop/src/workflow.rs`
- `crates/devil-desktop/tests/session_restore.rs`
- `crates/devil-desktop/tests/diagnostics_export.rs`

## Verification

- Passed: `cargo test -p devil-desktop --test session_restore -- --nocapture`
- Passed: `cargo test -p devil-desktop --test diagnostics_export -- --nocapture`
