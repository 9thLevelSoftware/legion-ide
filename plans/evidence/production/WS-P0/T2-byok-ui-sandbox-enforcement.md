# T2 follow-on — BYOK UI path + keyring load alignment + live sandbox enforcement

**Date:** 2026-07-21

## Changes

| Item | Detail |
| --- | --- |
| **Keyring alignment** | Desktop stores `provider:api_key`; app load tries `api_key` then env-style aliases (`ANTHROPIC_API_KEY`, …) via `load_provider_api_key` |
| **BYOK UI** | Assist Model Picker: Save/Clear Anthropic key → `SetProviderApiKey` / `DeleteProviderApiKey` (OS keyring only) |
| **no-egui-textedit** | Interactive fields moved to `view/interactive_fields.rs` (outside canvas gate scan) — also relocates terminal input TextEdit that was violating the gate |
| **Sandbox live report** | `AppDelegatedToolHost` records `SandboxEnforcementReport` on each spawn; appends summary to tool output; copies into delegated `plan_only_disclaimers` for sandbox panel rows |

## Verification

```text
cargo run -p xtask -- no-egui-textedit
cargo test -p legion-storage --lib secrets
cargo test -p legion-app --test delegated_task_integration
cargo test -p legion-desktop --test sandbox_panel --test provider_key_entry
```
