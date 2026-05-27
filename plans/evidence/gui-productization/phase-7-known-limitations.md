# GUI Phase 7 Known Limitations

## Scope

GUI Phase 7 is a local IDE beta. It does not replace the accepted legacy remote-development Phase 7 evidence under `plans/evidence/phase-7/`, and it does not claim advanced GUI GA surfaces.

## Limitation Inventory

| Surface | Status | Notes |
|---|---|---|
| Remote production GUI: unsupported | unsupported | Legacy Phase 7 remote substrate evidence remains accepted, but the GUI beta does not expose production remote workspace management. |
| Collaboration GUI: unsupported | unsupported | Collaboration presence and operation substrate are not a Phase 7 local-beta GUI claim. |
| Plugin management GUI: unsupported | unsupported | Plugin marketplace, install, update, trust, and contribution management remain future-gated. |
| Hosted provider activation: unsupported | unsupported | Assisted AI remains local-first/default-deny; hosted provider activation is not accepted for this beta. |
| Signed installer: unsupported | unsupported | Phase 6 packaging produces a deterministic Windows package path and dry-run evidence, not a signed installer. |
| Cross-platform parity: unsupported | unsupported | Windows evidence exists; macOS/Linux parity is not accepted for GUI Phase 7. |
| Autonomous apply: unsupported | unsupported | AI and language edit outputs remain proposal-only and must not self-apply. |
| OS accessibility inspection | limited | Projection accessibility metadata is available; OS accessibility tree inspection remains not observed in the current smoke evidence. |
| Native PTY production hardening | limited | Terminal launch is policy-gated and denied by default in beta evidence; production native PTY hardening is not accepted here. |
| Real-repository write smoke | limited | Automated write smoke uses an isolated fixture under `target/`; real-repository manual writes should use an intentional scratch file only. |

## Required Markers

- Remote production GUI: unsupported
- Collaboration GUI: unsupported
- Plugin management GUI: unsupported
- Hosted provider activation: unsupported
- Signed installer: unsupported
- Cross-platform parity: unsupported
- Autonomous apply: unsupported

## Acceptance Boundary

The local beta can be accepted for deterministic open/browse/edit/search/save/language/terminal/proposal/diagnostics evidence without accepting remote production GUI, collaboration GUI, plugin management, hosted provider activation, autonomous apply, signed installer readiness, or platform parity.
