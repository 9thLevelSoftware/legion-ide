# GUI Phase 6 session and diagnostics safety evidence

## Status

status: passed

## Commands

- `cargo test -p devil-desktop --test session_restore -- --nocapture`: passed, 5 tests.
- `cargo test -p devil-desktop --test diagnostics_export -- --nocapture`: passed, 2 tests.
- `cargo run -p devil-desktop -- --smoke --workspace . --file Cargo.toml --duration-ms 250 --evidence plans/evidence/gui-productization/phase-6-platform-accessibility-smoke.md --session-state target/gui-phase6-session.json --diagnostics-export target/gui-phase6-diagnostics.md`: passed.

## Session Persistence

- Session JSON is validated before publish.
- Session saves write through a same-directory temp file and atomically replace the final file, leaving no temp/backup intermediates after publish.
- Raw-source markers are rejected before save and before load.
- Dirty editor text is not serialized into session records.

## Diagnostics Export

- Diagnostics export path: `target/gui-phase6-diagnostics.md`
- Observed open tabs: `1`
- Observed dirty tabs: `0`
- Observed platform projection accessibility nodes: `5`
- Diagnostics include metadata counts and smoke labels only.
- Diagnostics do not include editor text, bounded previews, source bodies, secrets, or status-message bodies.

## Residual Risk

- The short smoke run does not produce a session JSON file by itself because no mutating desktop action occurs during smoke. Session persistence is verified by action-driven session tests.
