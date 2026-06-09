# GUI Phase 6 performance and reliability evidence

## Status

status: passed

## Commands

- `cargo test -p legion-desktop --all-targets`: passed.
- `cargo run -p legion-desktop -- --smoke --workspace . --file Cargo.toml --duration-ms 250 --evidence plans/evidence/gui-productization/phase-6-platform-accessibility-smoke.md --session-state target/gui-phase6-session.json --diagnostics-export target/gui-phase6-diagnostics.md`: passed.

## Desktop Test Coverage

- Unit tests: passed.
- Projection rendering tests: passed.
- Daily editing workflow tests: passed.
- Save-all conflict tests: passed.
- Large-file guardrail tests: passed.
- Packaging tests: passed.
- Platform integration and smoke report tests: passed.
- Session restore and diagnostics export tests: passed.

## Smoke Timing Snapshot

- status: passed
- p50 input-to-paint: recorded in `phase-6-platform-accessibility-smoke.md`
- p95 input-to-paint: recorded in `phase-6-platform-accessibility-smoke.md`
- frame variance: recorded in `phase-6-platform-accessibility-smoke.md`
- errors: none

## Reliability Notes

- Session saves no longer write directly to the final path.
- Diagnostics and package manifests are metadata-only.
- The smoke report keeps adapter-path checks separate from OS-observed checks.
