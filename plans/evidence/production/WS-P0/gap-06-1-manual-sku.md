# GAP-06.1 — Unsigned Manual SKU package channel

**Date:** 2026-09-02  
**Wave:** 3 trust chain  
**Task:** GAP-06.1 (unsigned packaging; not signed)

## What this is

A separate native-package SKU that builds

`cargo build -p legion-desktop --no-default-features --features offline`

and labels that SKU **Manual** in the package manifest, `RELEASE-METADATA.toml`,
and Help/About.

`legion-ai-providers` is optional and enabled only by the `ai` feature. The
Manual SKU does not declare provider HTTP stacks as a product feature.

Default `ai` desktop remains a different SKU (`sku: default`).

`signer_status` stays `unsigned-beta/no-os-code-signing`. This is not GAP-02.2.

## How to build

```text
scripts/package-native.sh --version 0.0.1 --format deb --sku manual --dry-run
scripts/package-native.ps1 -Version 0.0.1 -Format wix -Sku manual -DryRun
```

Windows package plans: `WindowsPackageConfig::with_sku(PackageSku::Manual)`.

## What this is not

- Not Authenticode / Developer ID / minisign (GAP-02.2)
- Not GAP-06.2 OS-level packet-capture no-egress
- Not a ledger promotion of PR-REL-001 or PR-AI-001
- Not a claim that invitation-only preview is signed

Ledger row statuses are unchanged.
