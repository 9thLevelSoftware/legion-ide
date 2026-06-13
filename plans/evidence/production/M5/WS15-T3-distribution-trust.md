# M5 — WS15.T3 Distribution & Trust Evidence

## Status

Accepted.

## Acceptance target

- Signed extension artifacts / checksum-manifest posture is enforced by fail-closed integrity checks.
- Permission review UI evidence is present for manifest capability rows.
- Tampered artifact rejection is covered by an automated test.

## What was verified

- `xtask/src/release_pipeline.rs`
  - `verify_descriptors()` now performs a fail-closed integrity comparison against the planned descriptor contents instead of trusting file presence alone.
  - Missing descriptors still fail closed, and on-disk tampering now returns `failed/tampered-descriptor`.
- `xtask/tests/release_pipeline.rs`
  - Added a regression test that appends bytes to a written descriptor and confirms verification rejects the tampered artifact.
- `crates/legion-desktop/src/view.rs`
  - The trust panel already renders context-manifest, permission-budget, approval-checklist, privacy, and rollback rows derived from projections.
- `crates/legion-desktop/tests/control_trust_view.rs`
  - The UI contract test already proves the permission-review surface is rendered from the manifest/projection data, including `context item`, `context permission`, `permission budget`, `permission evaluation`, `approval gate`, and `rollback target` rows.

## Verification commands

```bash
cargo test -p xtask --test release_pipeline -- --nocapture
cargo test -p legion-desktop --test control_trust_view -- --nocapture
cargo test -p legion-lsp --test registry_contract -- --nocapture
```

## Results

- `cargo test -p xtask --test release_pipeline -- --nocapture`
  - 14 tests passed, including `release_pipeline_verify_descriptors_rejects_tampered_descriptor_bytes`.
- `cargo test -p legion-desktop --test control_trust_view -- --nocapture`
  - 4 tests passed.
- `cargo test -p legion-lsp --test registry_contract -- --nocapture`
  - 4 tests passed.

## Findings

- Descriptor verification now rejects on-disk tampering instead of only checking file existence, which gives the plan a concrete fail-closed distribution-trust regression.
- The permission review UI evidence already exists in the desktop trust-view contract test, so the card is covered on both acceptance axes.
