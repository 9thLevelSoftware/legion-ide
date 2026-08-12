# 06-01 Result: GUI Phase 6 Governance

## Summary

Added a GUI Phase 6 policy note that keeps productization work limited to packaging, platform integration, accessibility smoke, session metadata safety, diagnostics, and CI/script parity. Added a dedicated GUI Phase 6 evidence gate in `xtask`, separate from the legacy collaboration Phase 6 gate, and created the scaffold acceptance document with explicit not-accepted status.

## Files Changed

- `plans/dependency-policy.md`
- `xtask/src/main.rs`
- `plans/evidence/gui-productization/phase-6-packaging-platform-accessibility.md`

## Verification

- Passed: `rg -q "GUI Phase 6" plans/dependency-policy.md`
- Passed: `rg -q "DEFAULT_GUI_PHASE6_EVIDENCE_PATH" xtask/src/main.rs`
- Passed: `rg -q "Phase 6 acceptance: Not accepted" plans/evidence/gui-productization/phase-6-packaging-platform-accessibility.md`
- Passed: `cargo run -p xtask -- check-deps`
