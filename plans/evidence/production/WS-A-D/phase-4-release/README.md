# Phase 4 — WS17 release evidence

**Current posture:**
- WS17.T1 dry-run descriptors only (`AGENTS.md`)
- WS17.T2 unsigned-beta policy: `plans/evidence/production/M5/WS17-T2-signing-notarization.md`
- No private signing material in-repo
- **D1:** portable unsigned preview archives via `legion-preview.yml` (not merge-blocking)
- **D2:** unsigned-beta **retained** for OS installers until secrets exist
- **D3:** local `update-drill` staging proof; hosted feed deferred (D3.1)
- **D4:** readiness close note — 3-OS preview CI success recorded; PR-REL-001 not flipped

## Packets

| File | Role |
| --- | --- |
| `D0-packaging-design.md` | Preview channel design: cargo-dist, artifact matrix, secrets, CI shape |
| `D1-unsigned-preview-artifacts.md` | Portable zip/tar.gz unsigned-beta + package scripts + CI |
| `D2-unsigned-beta-retained.md` | Explicit retain unsigned-beta until OS signing secrets exist |
| `D3-update-channel-staging.md` | Local update-drill as staging proof; hosted feed = D3.1 |
| `D4-readiness-close.md` | Campaign Phase 4 close + 3-OS preview proof + ledger honesty |
