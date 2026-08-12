# Plan 04-01 Summary

Governance and projection contracts are rebaselined for GUI Phase 4. The app can now legally compose `devil-index` and `devil-terminal`, and protocol/UI expose language and terminal projections without moving product-state ownership into `devil-ui`.

## Verification

- `cargo run -p xtask -- check-deps`
- `cargo test -p devil-protocol --test dto_contracts language_terminal_projection -- --nocapture`
- `cargo check -p devil-ui --all-targets`
