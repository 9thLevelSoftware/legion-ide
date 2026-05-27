# Phase 4 Wave Checklist

## Status

- Plan 04-01: Complete
- Plan 04-02: Complete
- Plan 04-03: Complete
- Plan 04-04: Complete
- Plan 04-05: Complete
- Plan 04-06: Complete

## Boundary Checklist

- `devil-ui` remains projection-only.
- App owns language and terminal workflow state.
- Language edit-producing actions create proposal previews before mutation.
- Terminal launch is denied by default and denied for untrusted workspaces.
- Desktop renders and routes projections without editor/workspace/terminal ownership.

## Verification Checklist

- `cargo run -p xtask -- check-deps`: passed
- `cargo fmt --all --check`: passed
- `cargo check --workspace --all-targets`: passed
- `cargo test --workspace --all-targets`: passed
- `cargo clippy --workspace --all-targets -- -D warnings`: passed
- `cargo deny check`: passed with duplicate-version warnings under warning-level policy
- Plan 04-05 named integration test targets: passed after review-cycle coverage fix

## Map Freshness

Phase 4 live implementation used current source inspection. The Phase 4 context noted stale map data for some crate surfaces, so final acceptance is based on live source and command evidence rather than stale map claims.
