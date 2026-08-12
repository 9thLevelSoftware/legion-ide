# Phase 4 D2 — Signing path **or** unsigned-beta retained

**Date:** 2026-07-22  
**Status:** **Unsigned-beta retained** (explicit product decision)

## Decision

Until OS code-signing secrets exist **outside** the repository and are wired into CI:

1. **Do not** claim Gatekeeper / SmartScreen / package-signature readiness.
2. **Retain** `signer_status = unsigned-beta/no-os-code-signing` (portable preview) and  
   `unsigned-beta/no-signer-configured` / `dry-run/no-production-signer` (release-pipeline descriptors) as **first-class** outcomes.
3. **Do not** commit private keys, P12/PFX, notary passwords, or API tokens.

This matches M5 WS17.T2 and D0 secret inventory. Real Authenticode / Apple notarization / deb-sig is **blocked on external secret provisioning**, not on missing design.

## What “signed path later” requires (external)

| Secret | Consumer | Not in git |
| --- | --- | --- |
| Windows Authenticode cert + unlock | MSI/exe signing step | ✓ |
| Apple Developer ID + notary profile | codesign + notarytool | ✓ |
| Optional Linux package key | deb/rpm | ✓ |
| `LEGION_SIGNING_KEY` (Ed25519 seed) | **Update manifest** only (ADR-0042) — may exist in CI without OS signing | ✓ |

## Current distribution surfaces

| Surface | Signing today | Evidence |
| --- | --- | --- |
| `legion-preview.yml` portable zip/tar.gz | Unsigned-beta TOML in bundle | D1 |
| `xtask release-pipeline --dry-run` | `dry-run/no-production-signer` | WS17.T1 |
| `xtask release-pipeline --from-artifacts` without signer | `unsigned-beta/no-signer-configured` | WS17.T2 tests |
| cargo-dist MSI/DMG/deb | **Not built in CI yet** | Deferred to D2.1 when secrets + dist config land |

## Product language (required)

- Preview archives and dry-run descriptors must keep `production = false` / unsigned status strings.
- USER_GUIDE / OPERATOR_RUNBOOK already state no production signing credentials in-repo; D2 does not change that.

## Acceptance for D2 (this packet)

- [x] Explicit choose **unsigned-beta retained** over incomplete/fake signing
- [x] Cross-link M5 WS17.T2 + D0/D1
- [x] Checklist row for D2 closed without inventing secrets
- [ ] D2.1 (future): real OS signing jobs when secrets are provisioned in CI

## Non-goals

- Adding placeholder “signed” flags
- Storing any private material in the repo
- Promoting preview → stable without dogfood on installed preview (D4 / Phase 5)

## References

- `plans/evidence/production/M5/WS17-T2-signing-notarization.md`
- `plans/evidence/production/WS-A-D/phase-4-release/D0-packaging-design.md`
- `plans/evidence/production/WS-A-D/phase-4-release/D1-unsigned-preview-artifacts.md`
- `xtask/release-pipeline.example.toml`
