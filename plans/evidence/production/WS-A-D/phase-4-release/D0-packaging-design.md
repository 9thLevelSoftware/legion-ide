# Phase 4 D0 — Packaging design (preview channel)

**Date:** 2026-07-21  
**Status:** Design decision record (no production secrets, no signed artifacts yet)

## Goals (MVP)

1. Produce **installable preview** artifacts for Windows / macOS / Linux (one primary arch each).
2. Keep **unsigned-beta** as a first-class outcome until OS code signing secrets exist.
3. Feed the existing **Ed25519 update-manifest** path (ADR-0042) from a non-secret staging location.
4. Never commit private keys, certs, or notarization tokens.

## Dist tool

| Option | Decision |
| --- | --- |
| **cargo-dist** | **Primary** — already named in `xtask/release-pipeline.example.toml` `dist_tool` and per-target `build_command` strings |
| Custom `scripts/package-*.ps1` | Keep as Windows dry-run / fallback evidence path (`package-windows.ps1`) |

D1 will either execute those `build_command`s from CI or teach `xtask release-pipeline` a non-dry-run execute mode that shells out to cargo-dist.

## Artifact matrix (preview MVP)

| Platform | Target triple | Artifact | Notes |
| --- | --- | --- | --- |
| Windows | `x86_64-pc-windows-msvc` | `.msi` or portable `.zip` of `legion-desktop.exe` | MSI preferred for install smoke; zip acceptable for first CI |
| macOS | `aarch64-apple-darwin` (primary), `x86_64-apple-darwin` secondary | `.dmg` | Notarization deferred to D2 |
| Linux | `x86_64-unknown-linux-gnu` | `.deb` and/or `.tar.gz` | deb for apt-style smoke; tarball always |

Secondary arches (win-arm64, linux-aarch64) are post-MVP.

## Channels

| Channel | Version stamp | Rollout |
| --- | --- | --- |
| `preview` | `<package_version>-preview` | staged |
| `stable` | `<package_version>` | full (after preview dogfood) |

`xtask release-pipeline --channel preview` remains the planning entry point.

## Secret inventory (external only)

| Secret | Used for | Storage |
| --- | --- | --- |
| `LEGION_SIGNING_KEY` | Ed25519 **update manifest** seed (base64 32-byte) | CI secret / env (already referenced in example TOML) |
| Windows Authenticode cert + password | OS code signing | CI secret / HSM (D2) |
| Apple Developer ID + notary credentials | codesign + notarize | CI secret / ASC (D2) |
| Optional Linux package signing key | deb/rpm sigs | CI secret (D2 optional) |

**Forbidden in git:** raw keys, P12/PFX, notary passwords, API tokens.

## CI shape (D1)

1. Workflow: `workflow_dispatch` + tag `preview-v*` (not merge-blocking initially).
2. Matrix: ubuntu / windows / macos runners build host-native preview artifact.
3. Upload artifacts to GitHub Actions artifacts or a draft GitHub Release.
4. Run `xtask release-pipeline --dry-run` then later `--from-artifacts` for manifest planning.
5. Smoke job: download artifact, install or extract, launch `--help` or beta-smoke headless if available.

## Update feed (D3 sketch)

- Host `release-manifest.v1.toml` + `.sig` on HTTPS (GitHub Releases or static bucket).
- Staging uses ephemeral or CI-generated keys; production uses protected `LEGION_SIGNING_KEY`.
- `xtask update-drill` already exercises rollback with ephemeral keys — wire staging URL in D3.

## Acceptance for D0

- [x] This design recorded under `phase-4-release/`
- [x] Artifact matrix + secret inventory + cargo-dist primary tool named
- [x] Linked from phase-gate checklist; D1 portable path landed in parallel

## Non-goals (D0)

- Implementing cargo-dist config in-repo (D1)
- Real Authenticode/notarization (D2)
- Stable channel promotion (after preview dogfood)

## References

- `xtask/release-pipeline.example.toml`
- `plans/adrs/ADR-0042-auto-update-strategy-and-signed-manifest.md`
- `plans/evidence/production/M5/WS17-T2-signing-notarization.md`
- `crates/legion-desktop/src/package.rs` (explicit non-installer dry-run today)
- `scripts/package-windows.ps1`
