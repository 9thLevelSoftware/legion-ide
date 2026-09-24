# GAP-02.1 — EXT-CERT issues filed (certs not in the org store)

**Date:** 2026-09-02  
**Wave:** 3 trust chain  
**Task:** GAP-02.1 (not closed)

## What this is

The QUAL.11 queue now has one issue per `EXT-CERT-*` id. Filing them is the
sequence's "Issue `EXT-CERT-WIN/MAC/LIN`" step. It is not certificate
procurement and not GAP-02.2.

| Id | Issue | Status |
| --- | --- | --- |
| `EXT-CERT-MAC` | [#211](https://github.com/9thLevelSoftware/legion-ide/issues/211) | Apple Developer Program acquired 2026-08-17; Developer ID certs and `notarytool` API key not issued into CI |
| `EXT-CERT-LIN` | [#212](https://github.com/9thLevelSoftware/legion-ide/issues/212) | minisign keypair not generated; $0 operator action |
| `EXT-CERT-WIN` | [#213](https://github.com/9thLevelSoftware/legion-ide/issues/213) | Authenticode path undecided (Azure Trusted Signing eligibility still unchecked) |

Checklist: [`plans/release/procurement-and-key-escrow.md`](../../../release/procurement-and-key-escrow.md)

## What this is not

- Not certs in the org secret store (GAP-02.1 acceptance still open)
- Not GAP-02.2 (`signer_status` remains `unsigned-beta/no-signer-configured`)
- Not GAP-02.3 fresh-VM Gatekeeper / SmartScreen / Linux trust journals
- Not a ledger promotion of PR-REL-001
- No private keys, `.p12` files, or API tokens are in this change

Escrow checklist is **not** complete. Close each issue only when the named
credential exists outside the repo and the close-out evidence path in that
issue is real.

Ledger row statuses are unchanged.
