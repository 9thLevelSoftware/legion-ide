# T2 slice — Delegate UI path + keyring credential load

**Date:** 2026-07-21  

## Changes

| Item | Detail |
| --- | --- |
| **Delegate primary button** | Emits `StartDelegatedTask` with a repo-scoped default `DelegatedTaskScope` (not deterministic `StartAiProposal`) |
| **Scope helper** | `desktop_default_delegated_scope` derives workspace root from open file / explorer / Cargo.toml / `.git` |
| **Anthropic credentials** | `anthropic_client_with_keyring_fallback` uses env first, then OS keyring `legion-ai-providers` / `anthropic:ANTHROPIC_API_KEY` (matches desktop `SetProviderApiKey` write path) |

## Explicitly still open (later slices)

- ~~Assist/inline default still uses `deterministic-local` for proposal generation~~ → closed in `T2-assist-real-provider.md` (live Anthropic when credentials exist; fixture offline)
- Full BYOK provider picker UI still incomplete
- Real DAP adapter (Tier 3)
- Streaming UI for model responses

## Verification

```text
cargo check -p legion-app --lib
cargo check -p legion-desktop --lib
cargo test -p legion-desktop --test input_conformance
cargo run -p xtask -- docs-hygiene
cargo run -p xtask -- claim-audit
```

All passed this session.
