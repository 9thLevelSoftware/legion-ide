# Release Procurement & Key Escrow (P8.F1.T5)

Status: **open — purchases are owner actions.** This document is the checklist
and the standing key-inventory policy. Update the Status column as each item
lands; several backlog cards are `blocked` on the `EXT-*` ids below.

## Procurement checklist

| Item | Unblocks | Est. cost | Lead time | Where | Status |
| --- | --- | --- | --- | --- | --- |
| Apple Developer Program (organization or individual) | `EXT-CERT-MAC` — Developer ID signing + notarization (Phase 5); fresh-VM Gatekeeper evidence (P8.F1.T3) | $99/yr | Days (identity verification) | developer.apple.com/programs | ☑ **acquired 2026-08-17** (owner-confirmed; certificates not yet issued or wired into CI) |
| Azure Trusted Signing account + identity validation | `EXT-CERT-WIN` — Authenticode signing for MSI via signtool dlib; SmartScreen reputation accrual starts at first signed release | ~$9.99/mo (Basic) | 1–3 weeks (identity validation) | portal.azure.com → Trusted Signing | ☐ |
| Linux signing keypair (minisign/Ed25519) | `EXT-CERT-LIN` — detached signatures for DEB/AppImage + signed SHA256SUMS | $0 (generate locally) | Immediate | `minisign -G` (store per escrow policy below) | ☐ |
| Update-feed domain + Cloudflare R2 bucket | `EXT-FEED` — HTTPS host for per-channel `release-manifest.v1.toml` + `.sig` (ADR-0042; feed topology ADR to follow in Phase 5) | ~$10/yr domain; R2 ≈ $0 at beta scale | Days | Cloudflare dashboard | ☐ |
| Cloud Mac access (e.g. MacStadium / AWS EC2 Mac / rented Mac mini) | `EXT-VM` — fresh-VM Gatekeeper evidence (P8.F1.T3), macOS notarization drills | ~$50–130/mo (cancel after evidence) | Days | vendor of choice | ☐ |
| Hosted provider API keys (Anthropic BYOK at minimum) | `EXT-LIVEKEY` — live-model adversarial evals (PR-AI-002 deferred item), Phase 3 hostile-eval runs | usage-billed | Immediate | console.anthropic.com | ☐ |

Not purchased deliberately: no GPU CI runners yet (local-model perf evidence
runs on the owner's hardware until Phase 4 sizes the need), no external pen
test yet (`EXT-PENTEST` is Phase 7 scope).

## Key inventory

| Key / credential | Purpose | Lives where | Backup |
| --- | --- | --- | --- |
| Ed25519 release-manifest signing key (ADR-0042) | Signs `release-manifest.v1.toml`; public key pinned in the `legion-app` updater | CI secret (never in tree; `xtask` signer config is reference-only) | Offline copy required — see escrow policy |
| Azure Trusted Signing access (service principal / API creds) | MSI Authenticode at release time | Azure + GitHub Actions secret | Azure account recovery + break-glass owner creds |
| Apple Developer ID certificates + App Store Connect API key | codesign/notarytool | Apple account + GitHub Actions secret | Apple account recovery; cert re-issue possible |
| minisign secret key (Linux artifacts) | DEB/AppImage detached sigs | CI secret | Offline copy required |
| Provider API keys (BYOK) | Live evals only — never shipped, never default | CI secret (evals), OS keyring locally | Revocable/reissuable; no escrow needed |

## Escrow policy

1. **Offline backup** of the Ed25519 manifest key and the minisign secret key:
   two copies on separate offline media, stored apart from the development
   machine. Loss of the manifest key strands every installed client on its
   pinned public key — this is the single worst credential loss in the system.
2. **Rotation path**: the ADR-0042 manifest schema carries a signer reference;
   a `manifest key v2` rotation is executed by shipping an update (signed with
   v1) whose binary pins both v1 and v2, then switching the feed to v2
   signatures. A break-glass rotation drill is added to `update-drill` in
   Phase 5 (roadmap step 5.9).
3. **No signing credential is ever committed**, echoed in `AGENTS.md` and
   enforced by the existing release-pipeline posture (`dry-run/no-production-signer`
   until credentials exist in CI).
4. **Bus factor**: one owner today. Each account above must have recovery
   contact + 2FA backup codes stored with the offline media.

## Windows signing — the open decision (`EXT-CERT-WIN`)

Recorded 2026-08-17. Apple is acquired; Windows is not, and the row above names
Azure Trusted Signing as though it were settled. It is the cheapest option, not
the only one, and it has an eligibility gate worth checking *before* budgeting
around it.

**Nothing is blocked by this today.** Phases 0-4 proceed without any Windows
certificate; the release pipeline already runs `dry-run/no-production-signer`
and the product ships an unsigned-beta channel by design. The only cost of
deciding late is that SmartScreen reputation accrues from the first *signed*
release, so a later start means a longer warning period for early users.

| Option | Rough cost | CI story | The catch |
| --- | --- | --- | --- |
| Azure Trusted Signing | ~$10/mo | Good — signtool dlib, service principal in Actions | Identity validation has had an eligibility bar (organizations needing several years of verifiable legal existence; individual validation offered separately). **Verify current terms before planning around it** — this is the item most likely to disqualify a solo developer, and it is the one thing on this page nobody has checked. |
| OV certificate from a commercial CA with cloud signing (SSL.com eSigner, DigiCert KeyLocker, Certum, …) | Low hundreds/yr | Good — the cloud-HSM services exist precisely for CI | Since the 2023 CA/Browser Forum change, code-signing private keys must live on certified hardware. A plain PFX file in a secret is no longer issuable, so a hardware token without a cloud option cannot sign in GitHub Actions. |
| EV certificate | Higher hundreds/yr | Same as above, cloud-HSM required | Historically the only route to *immediate* SmartScreen reputation rather than accrual. If the warning-on-first-download experience matters commercially, this is what buys it away. |
| Stay unsigned on Windows | $0 | n/a | Already the documented posture. Honest, and it keeps the beta shipping — but `PR-REL-001` cannot reach product-ready, and every Windows user sees a SmartScreen warning indefinitely. |

Costs and eligibility rules above are as understood at time of writing and are
the kind of thing vendors change; treat them as a starting point for a check,
not as quotes. The decision belongs in the Phase 5 signing ADR alongside the
feed-topology decision (ADR-0051), and until it is made `EXT-CERT-WIN` stays
open.

**What Apple being acquired unblocks, concretely:** nothing yet, because
certificates still have to be issued (Developer ID Application + Developer ID
Installer), an App Store Connect API key created for `notarytool`, and both
stored per the escrow policy below and wired as Actions secrets. Those are
Phase 5 steps 5.3 and 5.1. `P8.F1.T3` (fresh-VM Gatekeeper evidence) also still
needs `EXT-VM` — a Mac to run it on — which is a separate line item.

## Backlog linkage

- `P8.F1.T5` (this checklist) — in progress until every row above is checked.
- `P8.F1.T3` (fresh-VM evidence) — `blocked` / `EXT-VM`.
- Phase 5 signing pipeline work starts only after `EXT-CERT-WIN` + `EXT-CERT-MAC` land; everything else in Phases 0–4 proceeds without them.
- `EXT-CERT-MAC` is acquired (2026-08-17); certificate issuance, the App Store Connect API key, and CI wiring remain.
- `EXT-CERT-WIN` is undecided, not merely unpurchased — see the Windows signing section above. It blocks no current phase.
