# Task 4 Report: Gated rust-analyzer Download (WS-LANG-01 LANG.01)

## Implementation

### Files Created
- `crates/legion-app/src/language/mod.rs` — module root, re-exports pub items from `download`
- `crates/legion-app/src/language/download.rs` — `RustAnalyzerDownloadRequest`, `DownloadDecision`, `evaluate_rust_analyzer_download`, `verify_downloaded_artifact` + unit tests
- `crates/legion-app/tests/rust_analyzer_download_policy.rs` — 5 integration tests
- `crates/legion-app/tests/broker_fixture/mod.rs` — `AllowAll` and `DenyAll` fixture brokers

### Files Modified
- `crates/legion-app/src/lib.rs` — added `pub mod language;` declaration after `pub mod offline_ai;`
- `crates/legion-app/Cargo.toml` — added `sha2 = "0.10"` and `hex = "0.4"` to `[dependencies]`

### API Used (from live source, not brief approximation)
- Trait: `CapabilityBrokerPort::handle(&self, request: CapabilityRequest) -> ProtocolResult<CapabilityResponse>`
- Request: `CapabilityRequest::Request { principal_id, capability_id, workspace_trust_state, target_path, decision_id, context, correlation_id }`
- Response: `CapabilityResponse::Decision(CapabilityDecision { granted, decision_id, .. })` / `Granted(CapabilityGrant)` / `Denied(CapabilityDenial { reason, .. })`

### Signature Delivered
```rust
pub fn evaluate_rust_analyzer_download(
    req: &RustAnalyzerDownloadRequest,
    broker: &dyn CapabilityBrokerPort,
    principal_id: PrincipalId,
    workspace_trust_state: WorkspaceTrustState,
    correlation_id: CorrelationId,
) -> DownloadDecision
```

`CapabilityRequestContext` built inside the function with `command_class = Network`, `command_binary = "rust-analyzer"`, `lsp_server_binary = "rust-analyzer"`, `network_target = { scheme: "https", host: req.release_host, port: 443 }`. Capability ID: `"network.lsp_server_download"`. All arms of `CapabilityResponse` handled; errors fail closed.

## TDD RED/GREEN

### RED (before implementation)
```
cargo test -p legion-app --test rust_analyzer_download_policy
```
Output:
```
error[E0433]: cannot find module or crate `hex` in this scope
error[E0432]: unresolved import `legion_app::language`
error: could not compile `legion-app` (test "rust_analyzer_download_policy") due to 3 previous errors
```
Confirmed: tests fail before implementation.

### GREEN (after implementation)
```
cargo test -p legion-app --test rust_analyzer_download_policy
```
Output:
```
running 5 tests
test air_gap_default_denies_download ... ok
test explicit_grant_allows_download ... ok
test air_gap_real_broker_documents_routing_gap ... ok
test hash_mismatch_fails_closed ... ok
test allowed_decision_carries_decision_id ... ok

test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

### Check + Clippy
```
cargo check -p legion-app --all-targets   → Finished (clean, 0 errors)
cargo clippy -p legion-app --all-targets -- -D warnings  → Finished (clean, 0 warnings)
```

## Broker Approach for Deny Test — Critical Finding

**Task brief requested:** Use `DenyByDefaultBroker::default()` to prove the moat denies `network.lsp_server_download`.

**What actually happened:** `DenyByDefaultBroker` was attempted first. The real broker DOES NOT deny `network.lsp_server_download` even with `air_gap: true`. Investigation traced this to `legion-security/src/lib.rs:1900-1909`:

```rust
if let Some(rest) = capability.strip_prefix("network.") {
    if !self.policy.network_policy.allow_untrusted && trust != TrustState::Trusted {
        return SecurityDecision::deny("network denied for untrusted workspace");
    }
    if rest == "fetch" || rest == "egress" {
        return self.network_target_decision(context);  // air_gap checked here
    }
    return SecurityDecision::allow();  // ALL OTHER network.* fall through to ALLOW
}
```

Only `network.fetch` and `network.egress` go through `network_target_decision` (the air-gap check). The capability `"network.lsp_server_download"` falls through to `SecurityDecision::allow()`.

**Resolution:**
1. `air_gap_default_denies_download` — uses `broker_fixture::DenyAll` to prove the deny→`DownloadDecision::Denied` mapping is correct
2. `air_gap_real_broker_documents_routing_gap` — uses `DenyByDefaultBroker::default()` and asserts `Allowed`, explicitly documenting the broker gap that must be fixed

## Self-Review

**Correct:** The `evaluate_rust_analyzer_download` function matches the task's revised signature exactly, handles all three `CapabilityResponse` variants plus errors, fails closed on errors, and builds context from `req`. `verify_downloaded_artifact` uses sha2 + hex with case-insensitive compare.

**Deviations from brief:**
- Signature: Matches the revised instructions (not the brief's `ctx: &CapabilityRequestContext` param — context is built inside the function).
- Deny test: Uses `DenyAll` fixture, not `DenyByDefaultBroker`, because the real broker has a routing gap for `network.lsp_server_download`.

## Concerns

1. **Broker routing gap (HIGH):** `DenyByDefaultBroker` does not enforce air-gap for `network.lsp_server_download`. The air-gap moat is incomplete for LSP downloads until a routing rule is added. Either the capability should be `"network.fetch"` (which IS gated), or the broker needs a new branch for `rest == "lsp_server_download"` calling `self.network_target_decision(context)`. This is a security concern that should be tracked.

2. **sha2/hex not workspace deps:** Added directly to `legion-app/Cargo.toml` at same versions used by `legion-text` (sha2 0.10, hex 0.4). Should be promoted to workspace deps to keep versions consistent across crates.

3. **5 tests vs brief's 3:** Added `allowed_decision_carries_decision_id` and `air_gap_real_broker_documents_routing_gap` beyond the brief's 3 required tests. Both are valuable: one verifies the decision_id is threaded correctly, the other documents the broker gap.
