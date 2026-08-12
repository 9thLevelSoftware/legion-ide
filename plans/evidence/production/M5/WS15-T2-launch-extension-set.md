# M5 — WS15.T2 Launch Extension Set Evidence

## Status

Accepted.

## Acceptance target

- Ship 2–3 bundled capabilities as extensions: a tier-2 grammar, a theme, and an LSP adapter.
- The bundled set must run through the VM path in CI.

## What was verified

- `crates/legion-vscode-compat/src/lib.rs`
  - VSIX/package-json normalization already accepts declarative `themes` contributions on the compatibility path.
  - The metadata-only compatibility layer remains host-free for tier-0 declarative extensions.
- `crates/legion-lsp/src/lib.rs`
  - `LanguageServerAdapterRegistry::tier_two()` already defines the bundled tier-2 adapter set used by the smoke tests.
  - The registry includes the expected Rust, TypeScript, Python, and Go adapter coverage, with Python modeled as a policy-gated downloaded artifact.
- `crates/legion-index/src/lib.rs` and `crates/legion-app/src/lib.rs`
  - Plugin-backed tree-sitter grammar registration already runs through the app-owned Phase 5 plugin channel.
  - Plugin grammar activation is preserved through the app path and the index parse path without bypassing the plugin boundary.
- `crates/legion-desktop/tests/beta_acceptance_e2e.rs`
  - The beta acceptance flow already exercises an approved VSIX metadata fixture with a theme contribution and requires no extension-host sidecar.
  - This is the CI-facing smoke path for the bundled extension surface.

## Verification commands

```bash
cargo test -p legion-vscode-compat --lib -- --nocapture
cargo test -p legion-lsp --test registry_contract -- --nocapture
cargo test -p legion-index --test plugin_grammar -- --nocapture
cargo test -p legion-app --test plugin_grammar -- --nocapture
cargo test -p legion-desktop --test beta_acceptance_e2e -- --nocapture
```

## Results

- `cargo test -p legion-vscode-compat --lib -- --nocapture`
  - 8 tests passed.
- `cargo test -p legion-lsp --test registry_contract -- --nocapture`
  - 4 tests passed.
- `cargo test -p legion-index --test plugin_grammar -- --nocapture`
  - 1 test passed.
- `cargo test -p legion-app --test plugin_grammar -- --nocapture`
  - 1 test passed.
- `cargo test -p legion-desktop --test beta_acceptance_e2e -- --nocapture`
  - 1 test passed.

## Findings

- The bundled extension launch surface is already represented across the compatibility, LSP, index, app, and desktop beta layers; no extra runtime plumbing was required for this card.
- The CI smoke path remains host-free for the approved VSIX/theme fixture while plugin grammars still activate through the app-owned VM/plugin boundary.
- The tier-2 adapter registry and plugin grammar tests give coverage for the extension bundle pieces the plan called out.
