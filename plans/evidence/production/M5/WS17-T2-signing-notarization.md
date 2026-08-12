# M5 — WS17.T2 Signing & Notarization Evidence

## Status

Accepted.

## Acceptance target

- The product-readiness ledger must carry an explicit unsigned-beta policy before any PR-REL-001 status flip.
- No private signing keys, certificates, notarization tokens, or API credentials are committed to the repository.
- Gatekeeper / SmartScreen-ready signed-installer validation remains a follow-on verification path, not a hidden assumption.

## What was verified

- `plans/legion-production-master-plan-v0.1.md`
  - WS17.T2 explicitly allows two acceptable end states: fresh-VM Gatekeeper/SmartScreen verification, or an explicitly recorded unsigned-beta policy before the readiness-ledger status flip.
- `plans/product-readiness-ledger.md`
  - PR-REL-001 remains `In progress`.
  - The new evidence note records that the current release posture is explicitly unsigned beta until signed installers and fresh-VM verification exist.
- Repository policy surfaces
  - `AGENTS.md` keeps signing credentials out of the tree and preserves the dry-run / no-production-signer posture until a real signing path exists.

## Policy record

- Current product distribution posture: unsigned beta.
- Readiness-ledger status: unchanged (`In progress`).
- Required future evidence for a readiness flip: signed installer artifacts plus a verification record on fresh VMs, or a later policy revision that supersedes unsigned-beta.
- Forbidden repository contents: private keys, cert material, notarization secrets, signing tokens, or any credential-bearing export.

## Verification notes

- This task is documentation/evidence only; no signing credentials or release artifacts were added.
- The evidence is intentionally narrow so later WS17.T2 follow-on work can replace the unsigned-beta policy with real signing and notarization proof without rewriting the broader readiness ledger.
