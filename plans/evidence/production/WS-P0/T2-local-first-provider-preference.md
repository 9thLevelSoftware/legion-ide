# T2 follow-on — Local-first provider preference (Ollama → Anthropic → fixture)

**Date:** 2026-07-21

> **Superseded in part (2026-08-22).** The `Auto` arm described below no longer
> falls through to Anthropic when BYOK credentials are present; it routes Ollama
> then the deterministic fixture, and a remote provider has to be selected
> explicitly. This record is kept as written for what was done on the date above.

## Changes

| Item | Detail |
| --- | --- |
| **`ProductAiProviderPreference`** | `Auto` / `Ollama` / `Anthropic` / `Deterministic` on `AppComposition`; default from `LEGION_AI_PROVIDER` |
| **`complete_product_chat`** | Shared product completion edge for Assist proposals, Delegate chat, and inline ghost text |
| **Ollama** | Tried when preferred is Auto/Ollama **and** a 150ms TCP loopback probe succeeds (no 10s HTTP hang in CI) |
| **Anthropic** | Tried when preferred is Auto/Anthropic **and** env/keyring BYOK resolves |
| **Desktop UI** | Model Picker route buttons + existing BYOK key form; `SetPreferredAiProvider` action |
| **Docs** | `docs/USER_GUIDE.md` Assist section updated |

## Verification

```text
cargo check -p legion-app --lib
cargo check -p legion-desktop --lib
cargo test -p legion-app --test assist_inline_prediction_workflow --test delegated_task_integration --test control_trust_surfaces
cargo run -p xtask -- no-egui-textedit
cargo run -p xtask -- docs-hygiene
```
