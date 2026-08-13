# Phase 4 D1 — Unsigned preview artifacts

**Date:** 2026-07-21  
**Status:** CI portable unsigned-beta (not MSI/DMG/deb installers yet)

## Delivered

| Item | Location |
| --- | --- |
| Preview build workflow | `.github/workflows/legion-preview.yml` (`workflow_dispatch` + weekly optional) |
| Package scripts | `scripts/package-preview.ps1`, `scripts/package-preview.sh` |
| Bundle layout | `target/preview/<os>-<arch>/` + archive + `UNSIGNED-BETA.toml` |
| Smoke | Archive exists; binary present; `legion-desktop --beta-smoke` when display/deps allow |

## Artifact shape (unsigned-beta)

```text
legion-desktop-preview-<os>-<arch>.zip|.tar.gz
  legion-desktop[.exe]
  UNSIGNED-BETA.toml
  package-manifest.txt
```

`UNSIGNED-BETA.toml` records channel=`preview`, `signer_status=unsigned-beta/no-os-code-signing`, git SHA, built-at, and explicit non-production language. **No private keys.**

## What this is not

- Not Authenticode / notarized / package-signed (D2)
- Not full cargo-dist MSI/DMG/deb matrix (follow-on; portable archive is the D1 MVP)
- Not a merge-blocking standing gate (independent workflow, same posture as `legion-smoke.yml`)

## Local

```text
# Windows
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/package-preview.ps1 -Release

# Unix
bash scripts/package-preview.sh --release
```

## Next (D2/D3)

- cargo-dist installer formats + OS signing secrets (external only)
- Hosted update manifest from preview feed
