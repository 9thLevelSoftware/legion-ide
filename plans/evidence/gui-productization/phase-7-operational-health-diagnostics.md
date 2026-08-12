# GUI Phase 7 Operational Health And Diagnostics Evidence

## Status

status: passed

## Diagnostics Export

- Export path: `target/gui-phase7-diagnostics.md`
- Section observed: `## Operational Health`
- Runtime workspace: isolated beta workspace under `target/gui-phase7-beta-workspace`
- Open tabs: `1`
- Dirty tabs: `0`
- Search status: `Completed`
- Language status: `Cancelled`
- Terminal status: `Denied`
- Terminal denial label: `denied`
- Proposal rows: `2`
- Selected proposal: `2`
- Assisted-AI requests: `1`
- Assisted-AI preview-ready count: `1`
- Session state configured: `true`
- Diagnostics export configured: `true`

## Unsupported Surfaces

unsupported_surfaces:
- Remote production GUI: unsupported
- Collaboration GUI: unsupported
- Plugin management GUI: unsupported
- Hosted provider activation: unsupported
- Signed installer: unsupported
- Cross-platform parity: unsupported
- Autonomous apply: unsupported

## Commands

- `cargo test -p legion-desktop --test operational_health -- --nocapture`: passed, 2 tests.
- `cargo test -p legion-desktop --test diagnostics_export -- --nocapture`: passed, 2 tests.
- `cargo run -p legion-desktop -- --beta-smoke --workspace . --beta-workspace target/gui-phase7-beta-workspace --evidence plans/evidence/gui-productization/phase-7-local-workflow-smoke.md --session-state target/gui-phase7-session.json --diagnostics-export target/gui-phase7-diagnostics.md`: passed.
- `rg -q "Operational Health" target/gui-phase7-diagnostics.md`: passed.
- `rg -q "unsupported_surfaces" plans/evidence/gui-productization/phase-7-operational-health-diagnostics.md`: passed.

## Redaction Checks

- Operational health records labels, counts, booleans, ids, and status categories only.
- Diagnostics do not include dirty editor text, source bodies, terminal payloads, provider payloads, prompts, or status-message bodies.
- Targeted regression tests used `SECRET_PHASE7_DIRTY_BODY`, `SECRET_PHASE7_TERMINAL_PAYLOAD`, and `SECRET_PHASE7_PROMPT` markers and asserted they were absent from health rows and diagnostics.

## Residual Risk

- Native OS accessibility and high-DPI state remain not directly observed in this beta smoke run.
- Cross-platform parity and signed installer readiness remain explicitly unsupported for Phase 7 local beta.
