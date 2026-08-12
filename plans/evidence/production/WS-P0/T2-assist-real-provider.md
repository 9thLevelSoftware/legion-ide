# T2 slice — Assist / inline real provider when credentials exist

**Date:** 2026-07-21

## Changes

| Item | Detail |
| --- | --- |
| **Credential resolve** | `resolve_anthropic_api_key` checks env (`ANTHROPIC_API_KEY`, `LEGION_ANTHROPIC_API_KEY`, `DEVIL_ANTHROPIC_API_KEY`) then OS keyring `provider_secret_reference("anthropic", "ANTHROPIC_API_KEY")` |
| **Assist edit proposals** | `resolve_assisted_edit_proposal_text` calls Anthropic Messages when a key is present; otherwise keeps deterministic `/* phase4 local AI proposal */` fixture (offline/CI) |
| **Inline ghost text** | `try_live_anthropic_inline_prediction` maps a short chat completion into `InlinePredictionResult` (Anthropic has no `predict_inline` adapter); `invoke_inline_prediction_provider` prefers live then falls back to `deterministic-local` |
| **Keyring client** | `anthropic_client_with_keyring_fallback` always builds the client from the resolved key so BYOK matches desktop `SetProviderApiKey` |
| **Docs honesty** | `docs/USER_GUIDE.md` Assist section documents credential-gated live path vs offline fixture |

## Explicitly still open

- Full multi-provider picker / Ollama-first local default still incomplete
- Streaming UI for model responses
- Real DAP adapter (Tier 3)
- Live network smoke is **not** part of standing CI (no key in gates)

## Verification

```text
cargo check -p legion-app --lib
cargo test -p legion-app --test assist_inline_prediction_workflow
cargo test -p legion-app --test control_trust_surfaces
cargo test -p legion-app --test delegated_task_integration
```

All passed this session (offline deterministic path; no `ANTHROPIC_API_KEY` in environment).
