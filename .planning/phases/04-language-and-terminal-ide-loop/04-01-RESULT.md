# Plan 04-01 Result: Governance And Projection Contract Rebaseline

Status: Complete

## Files Changed

- `plans/dependency-policy.md`
- `crates/devil-app/Cargo.toml`
- `Cargo.lock`
- `crates/devil-protocol/src/lib.rs`
- `crates/devil-protocol/tests/dto_contracts.rs`
- `crates/devil-ui/src/ui.rs`

## Implementation Summary

- Added GUI Phase 4 dependency-policy authorization for app composition edges to `devil-index` and `devil-terminal`.
- Added app crate dependencies and lockfile metadata for those approved internal edges.
- Added projection DTOs for language tooling and terminal panel state.
- Extended `ShellProjectionSnapshot` and `CommandDispatchIntent` with language/terminal projection and command surfaces while preserving projection-only UI ownership.

## Verification

- `rg -q "GUI Phase 4" plans/dependency-policy.md` passed.
- `rg -q "LanguageToolingProjection" crates/devil-ui/src/ui.rs` passed.
- `rg -q "TerminalPanelProjection" crates/devil-ui/src/ui.rs` passed.
- `cargo run -p xtask -- check-deps` passed.
- `cargo test -p devil-protocol --test dto_contracts language_terminal_projection -- --nocapture` passed.
- `cargo check -p devil-ui --all-targets` passed.

## Issues

- None.
