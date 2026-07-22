# Phase 4 D3 — Update channel + staging drill

**Date:** 2026-07-22  
**Status:** Design + local drill proven; **hosted staging feed** still open (D3.1)

## What already exists (standing gate)

| Piece | Location |
| --- | --- |
| Local drill | `cargo run -p xtask -- update-drill` → `target/update-drill/update_drill_report.toml` |
| Binary | `legion-app` bin `upd-drill` (name avoids Windows “update*” UAC heuristic) |
| Crypto | Ephemeral in-memory Ed25519 seed (never persisted) |
| Flow | Generate key → write v0.1/v0.2 manifests → sign → verify → stage → journal → rollback |
| Policy | ADR-0042 custom Zed-style updater; dry-run / unsigned-beta first-class |

This **is** the D3 “drill against staging” for **local** staging (temp dirs + ephemeral keys). It does **not** yet hit a public HTTPS feed.

## Hosted staging feed (D3.1 — open)

Target shape (no production secrets required for a throwaway staging key):

| Asset | Notes |
| --- | --- |
| `https://…/preview/release-manifest.v1.toml` | From `xtask release-manifest` / `--from-artifacts` |
| `https://…/preview/release-manifest.v1.toml.sig` | Detached Ed25519; omit when unsigned-beta |
| Artifact URLs in manifest | Point at GH Actions artifacts or draft GH Release assets from D1 |

### Operator env (reserved; not wired in product UI yet)

| Variable | Intent |
| --- | --- |
| `LEGION_UPDATE_FEED_URL` | Base URL of staging/preview manifest (HTTPS) |
| `LEGION_SIGNING_KEY` | Base64 32-byte seed for signing only (CI secret / local export) |

`upd-drill` remains **zero-egress** for the standing gate. Hosted fetch is a separate optional job (same posture as `legion-preview.yml` — non-merge-blocking).

## Acceptance for this packet (D3 design close)

- [x] Document local drill as current staging proof
- [x] Define hosted feed assets + env names for D3.1
- [x] Explicit: no private keys in git; unsigned-beta feed allowed (D2)
- [ ] D3.1: CI job downloads preview artifacts, publishes draft release + manifest, optional fetch smoke

## Non-goals

- Replacing standing-gate update-drill with network calls
- Stable-channel promotion without Phase 5 dogfood on installed preview

## Verification

```text
cargo run -p xtask -- update-drill
# expect exit 0 and target/update-drill/update_drill_report.toml
```

## References

- `plans/adrs/ADR-0042-auto-update-strategy-and-signed-manifest.md`
- `xtask/src/update_drill.rs`, `crates/legion-app/src/bin/update_drill.rs`
- D1 portable preview, D2 unsigned-beta retained
