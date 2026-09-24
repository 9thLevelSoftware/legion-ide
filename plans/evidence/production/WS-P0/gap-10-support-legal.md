# GAP-10 — Support / legal

**Date:** 2026-09-01  
**Wave:** 1 daily-driver safety  
**Tasks:** GAP-10.1, GAP-10.2, GAP-10.3

## What this is

1. Root `LICENSE` (proprietary, not OSI) and `docs/PRIVACY.md` (Manual zero-egress, opt-in AI, no phone-home), linked from `docs/INDEX.md` and `README.md`.
2. Help/About exports a metadata-only support bundle through `AppComposition` → `SupportBundleAssembler::build_metadata_bundle`. Palette commands `Help: About` and `Help: Export Support Bundle` exist without `--diagnostics-export`.
3. Native package layout copies `LICENSE`, `PRIVACY.md`, and `THIRD_PARTY_NOTICES.md` next to the executable / into packager files maps.

## What this is not

Not a windowed GAP-01 GUI journal. Not a signed installer. Not a promotion of ledger rows. Not a general-availability or production-ready claim.

Default destination is `.legion/support-bundle.md`. Crash summaries are read from `.legion/crash-reports`. Product consent keeps `raw_source_allowed: false`; Help/About never calls `build_raw_bundle`.

## Verification

```text
cargo test -p legion-app --test support_bundle
cargo test -p legion-desktop --test diagnostics_export
cargo test -p legion-desktop --test packaging
cargo test -p legion-desktop --test intent_bridge intent_bridge_routes_settings_actions
cargo run -p xtask -- docs-hygiene
cargo run -p xtask -- claim-audit
```

Primary files:

- `LICENSE`, `docs/PRIVACY.md`, `docs/INDEX.md`, `README.md`
- `crates/legion-app/src/diagnostics.rs`
- `crates/legion-desktop/src/view.rs`, `crates/legion-desktop/src/view/about.rs`
- `crates/legion-desktop/src/package.rs`, `scripts/package-native.sh`, `scripts/package-native.ps1`, `scripts/package-windows.ps1`, `packaging/Packager.toml`

Ledger row statuses are unchanged.
