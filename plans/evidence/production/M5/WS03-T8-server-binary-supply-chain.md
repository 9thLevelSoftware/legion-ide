# M5 — WS03.T8 Server-Binary Supply Chain Evidence

## Status

Accepted.

## Acceptance target

- Checksummed download manifests.
- Offline / air-gap behavior denies downloads but keeps system-binary adapters usable.
- Version pinning is recorded per workspace in the manifest audit.

## What landed

- `crates/legion-lsp/src/lib.rs`
  - Added metadata-only binary-manifest audit types for workspace/language server adapters.
  - Added `LanguageServerAdapterRegistry::binary_manifest_for_workspace_language(...)`.
  - Air-gap mode now records denied download attempts instead of materializing downloaded-artifact entries.
  - Workspace-level version pin metadata is recorded for downloaded artifacts.
- `crates/legion-lsp/tests/registry_contract.rs`
  - Added coverage for:
    - air-gap denying the downloaded Python adapter while keeping system-binary adapters available,
    - checksum / policy metadata on downloaded artifact entries,
    - per-workspace version pin recording.

## Commands run

```bash
cargo test -p legion-lsp --test registry_contract
```

## Findings

- Tier-2 registry still resolves Rust, TypeScript, and Go from system binaries.
- Python remains modeled as a policy-gated downloaded artifact with checksum and policy gate metadata.
- Air-gap manifest audit records the denied download instead of attempting download resolution.
- Workspace pin metadata is emitted as `workspace/1` for the tier-2 manifest audit.
