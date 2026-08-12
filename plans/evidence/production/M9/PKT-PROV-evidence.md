# PKT-PROV Evidence — M9 Provider Activation and Policy UX

Date: 2026-07-06
Branch: `m9/provider-activation`

## Task summary

PKT-PROV adds provider tiers, workspace consent, OS-keyring BYOK key entry,
capability matrix gating, and live/recorded smoke tests for Anthropic and Ollama.

---

## T1 — Provider tiers, workspace consent, activation gate

**Commit:** `0197c88`

### Types added

- `AssistedAiProviderTier` enum in `legion-protocol/src/lib.rs`:
  `LocalDefault | LocalLoopbackOptIn | ByokConsentRequired | HostedDenied`
- `AssistedAiWorkspaceConsent` enum in `legion-protocol/src/lib.rs`:
  `NotRequired | Pending | Granted { granted_at, principal } | Denied`
- `AssistedAiProviderActivationDenial` enum in `legion-ai-providers/src/lib.rs`:
  `ConsentRequired | CredentialRequired | HostedDenied | AirGapDenied`

### Functions added

- `provider_tier(class, provider_id) -> AssistedAiProviderTier` in `legion-ai-providers/src/lib.rs`
- `can_activate_provider(tier, consent, has_credential) -> Result<(), Denial>` in `legion-ai-providers/src/lib.rs`
- `provider_setup_rows() -> Vec<String>` in `legion-ai-providers/src/lib.rs`

### Orphaned modules fixed

- `capability.rs` was not declared in `legion-protocol/src/lib.rs`; added `pub mod capability;` + re-export.
- `secrets.rs` was not declared in `legion-storage/src/lib.rs`; added `pub mod secrets;` + re-exports.

### Tests (8 passing)

```
running 8 tests
test local_default_always_activatable ... ok
test loopback_opt_in_activatable_without_consent ... ok
test byok_requires_both_consent_and_credential ... ok
test byok_with_consent_but_no_credential_is_denied ... ok
test hosted_denied_never_activatable ... ok
test air_gap_denies_all_remote_providers ... ok
test provider_tier_maps_all_known_providers ... ok
test provider_setup_rows_show_tier_and_consent ... ok
```

---

## T2 — BYOK key entry through OS keyring only

**Commit:** `63fcb6b`

### Changes

- `DesktopAction::SetProviderApiKey { provider_id, api_key }` and
  `DesktopAction::DeleteProviderApiKey { provider_id }` added to
  `legion-desktop/src/bridge.rs`; bridge `translate()` maps them to `Noop`.
- `workflow.rs` handles both actions via `OsKeyringSecretStore` using
  `provider_secret_reference(provider_id, "api_key")`; `api_key` value
  is consumed (moved, dropped) after successful store — never retained.
- `keyring = { workspace = true }` added to `legion-storage/Cargo.toml`.
- `legion-ai-providers` and `legion-storage` added to `legion-desktop` deps.

### Security

- No API key value ever written to disk or config file.
- `InMemorySecretStore` used in all tests — CI never touches OS keyring.
- `set_key_never_writes_to_disk` test scans `$TMPDIR` for a sentinel value
  after a store call to prove no leakage.

### Tests (4 passing)

```
running 4 tests
test set_provider_api_key_stores_in_keyring ... ok
test delete_provider_api_key_removes_from_keyring ... ok
test set_key_never_writes_to_disk ... ok
test set_key_activates_provider ... ok
```

---

## T3 — Capability matrix gating at activation time

**Commit:** `a8577f2`

### Changes

- `gate_provider_capabilities(matrix, tier, consent, has_credential) -> AssistedAiCapabilityMatrix`
  added to `legion-ai-providers/src/capabilities.rs`.
- When `can_activate_provider` returns `Ok`: returns `matrix.clone()` (full capabilities).
- When denied: returns a zeroed matrix with `supports_streaming=false`,
  `supports_structured_output=false`, empty labels, `availability=Unavailable`;
  structural fields (`provider_id`, `provider_label`, `provider_class`,
  `context_length_*`, `cost_usage_label`, `redaction_hints`, `schema_version`)
  always preserved.

### Tests (4 new, 12 total passing)

```
running 12 tests
test gated_matrix_preserves_capabilities_when_activated ... ok
test gated_matrix_zeros_capabilities_when_denied ... ok
test gated_matrix_never_adds_capabilities ... ok
test capability_matrix_requires_provider_declaration ... ok
... (8 T1 tests also pass)
```

---

## T4 — Live and recorded smoke tests

**Commit:** `880e83a`

### Fixture

`evals/recorded/anthropic_smoke.json`:
- `completion_response`: `{type: "message", role: "assistant", content: [{type: "text", text: "Yes."}]}`
- `streaming_body`: SSE with `message_start → content_block_start → content_block_delta("Yes.") → content_block_stop → message_delta → message_stop`

### Tests (4 passing)

```
running 4 tests
test recorded_anthropic_completion_smoke ... ok
test recorded_anthropic_streaming_smoke ... ok
test live_anthropic_smoke ... ok   (prints "skip: ANTHROPIC_API_KEY not set" when absent)
test live_ollama_smoke ... ok      (prints "skip: Ollama not available at localhost:11434" when absent)
```

Live tests use `println!("skip: reason")` not `#[ignore]` — CI always sees pass.

---

## Full workspace test result

```
cargo test -p legion-ai-providers -p legion-desktop -p legion-storage
... all tests passed
```

No regressions. `manual_zero_egress` constraint: all new code paths are
either offline (InMemorySecretStore, RecordedAnthropicTransport) or gated
on explicit env-var/port checks that skip cleanly in CI.
