# Installed preview dogfood checklist (Phase 5 residual)

Use after packaging a **local** unsigned-beta preview artifact. This is not a
signed installer path and must not be claimed as production-ready installability.

## Package (local)

```text
# Windows
pwsh scripts/package-preview.ps1

# Unix
bash scripts/package-preview.sh
```

Confirm output includes `UNSIGNED-BETA.toml` with `production = false` and a
portable zip/tar.gz for the host OS.

## Run

Extract the archive and launch the desktop binary from the package layout
(see smoke steps in `package-preview.*` / CI `legion-preview.yml`).

## Checklist

| # | Action | Pass? | Notes |
|---|--------|-------|-------|
| 1 | Binary starts without unsigned-beta false “production” claim | | Read `UNSIGNED-BETA.toml` |
| 2 | Open a workspace (this repo or fixture) | | |
| 3 | Manual edit + save | | |
| 4 | Terminal launch | | |
| 5 | Debug: F9 BP, F5 launch (if configs), Continue, Stop | | B14–B17 keys |
| 6 | Sandbox panel shows honest OS caveats | | |
| 7 | No crash on quit | | |

## Journal

```text
plans/evidence/dogfood/YYYY-MM-DD-installed-preview-journal.md
```

## Product-readiness impact

- **PR-REL-001** remains **In progress** until signed installers + fresh-VM smoke.
- This checklist only exercises **unsigned-beta portable** layout dogfood.
