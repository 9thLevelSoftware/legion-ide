# Phase 4 D4 — Readiness close (ledger note)

**Date:** 2026-07-22  
**Status:** Closed with **honest residual** (no PR-REL-001 flip to product-ready)

## What D4 accepts as “close” for this campaign

Phase 4 definition of done (charter): *preview installers on 3 OS families; signing **or** explicit unsigned-beta; update drill against staging*.

| Requirement | Evidence | Met? |
| --- | --- | --- |
| Preview artifacts 3 OS | `legion-preview.yml` matrix; successful run [29887799213](https://github.com/9thLevelSoftware/legion-ide/actions/runs/29887799213) (ubuntu/windows/macos **success**, head `da4815d`) | **Yes** — portable zip/tar.gz unsigned-beta (not MSI/DMG/deb) |
| Signing **or** unsigned-beta | D2 retained unsigned-beta; no private keys in repo | **Yes** (unsigned-beta path) |
| Update drill staging | Standing gate `update-drill` + D3 local staging design | **Yes** (local); hosted feed = D3.1 open |

## Explicitly **not** claimed

- Fresh-VM Gatekeeper / SmartScreen validation of signed installers
- Authenticode / Apple notarization / package signatures
- Hosted HTTPS update feed (D3.1)
- Daily-driver install from MSI/DMG/deb
- PR-REL-001 status flip to product-ready

## Ledger impact (PR-REL-001)

**Status remains `In progress`.**  
WS-A-D D0–D4 **strengthens** installability evidence (hosted 3-OS portable preview CI + unsigned-beta policy + local update drill) without satisfying signed-installer / fresh-VM acceptance.

## Operator how-to (preview artifacts)

1. Dispatch **Legion Preview** workflow (`workflow_dispatch`) on `main`.
2. Download `legion-desktop-preview-<os>` artifacts.
3. Extract zip/tar.gz; read `UNSIGNED-BETA.toml` (`production = false`).
4. Run `legion-desktop` / `legion-desktop.exe` (GUI / `--beta-smoke` as available).

Local:

```text
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/package-preview.ps1 -Release
# or
bash scripts/package-preview.sh --release
```

## Residual open for release track

| ID | Item |
| --- | --- |
| D2.1 | OS code signing when secrets exist outside git |
| D3.1 | Hosted preview update feed + optional fetch smoke |
| D4.1 | Fresh-VM install smoke of signed (or policy-accepted unsigned) installers |
| Phase 5 | Dogfood on **installed** preview build (interactive) |

## References

- D0 design, D1 portable CI, D2 unsigned-beta, D3 update drill
- M5 WS17-T2, M12 PKT-SIGN / PKT-UPDATER
- Run: https://github.com/9thLevelSoftware/legion-ide/actions/runs/29887799213
