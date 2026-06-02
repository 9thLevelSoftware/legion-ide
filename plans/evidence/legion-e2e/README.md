# Legion E2E Evidence Directory

This directory stores raw command outputs for the consolidated Legion E2E implementation plan.

Required final gates:

- `cargo run -p xtask -- check-deps`
- `cargo fmt --all --check`
- `cargo check --workspace --all-targets`
- `cargo test --workspace --all-targets`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo deny check`

Record command, working directory, exit code, and raw output for every phase gate.

Complete current final-gate capture: `20260602T091320_final_gates.txt`.
The evidence redacts absolute workspace paths as `<workspace>` to keep committed logs machine-neutral.
