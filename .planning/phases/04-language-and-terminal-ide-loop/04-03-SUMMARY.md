# Plan 04-03 Summary

The app terminal workflow is policy-gated and denied by default. The deterministic fixture path verifies lifecycle projection and no editor/disk mutation.

## Verification

- `cargo test -p devil-app --test terminal_workflow -- --nocapture`
- `cargo test -p devil-terminal --all-targets`
- `cargo test -p devil-security --all-targets`
