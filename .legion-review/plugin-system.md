# Plugin System Review

Reviewed files:
- `crates/legion-plugin/src/lib.rs`
- `crates/legion-plugin/src/host.rs`
- `crates/legion-plugin/src/manifest.rs`
- `crates/legion-plugin/src/registry.rs`
- `crates/legion-plugin/src/wit_bindings.rs`

Verification run:
- `cargo test -p legion-plugin --lib` passed: 10 passed, 0 failed.
- `cargo test -p legion-plugin` passed: 21 passed, 0 failed.
- `cargo clippy -p legion-plugin --all-targets -- -D warnings` passed.

Summary: 10 findings total: 1 critical, 6 high, 3 medium, 0 low.

## `crates/legion-plugin/src/lib.rs`

### Finding 1
- Category: bug
- Severity: high
- Line numbers: 171-176, 205-208
- Description: `dispatch_host_call` tracks `output_bytes_used`, but quota enforcement only compares the current `metadata_label.len()` to `max_output_bytes`. Multiple accepted host calls with labels under the per-call limit can cumulatively exceed the declared bounded-output quota, and the tracked total is never used to deny the next call.
- Suggested fix direction: Compute `next_output_bytes = output_bytes_used + metadata_label.len()` with checked/saturating arithmetic and deny when the cumulative total would exceed `manifest.quotas.max_output_bytes`. Reset the counter at the correct invocation boundary if the quota is intended to be per invocation.

### Finding 2
- Category: failure-point
- Severity: medium
- Line numbers: 84-87, 164-168, 205-209
- Description: `host_calls_used` is documented as usage during the active invocation, but this metadata-only host never establishes or resets an invocation boundary. The counter is initialized at load and then accumulates for the life of the loaded plugin, so a plugin can be denied later even though the manifest quota is declared as `max_host_calls` per invocation.
- Suggested fix direction: Model invocation start/end explicitly and reset per-invocation counters there, or rename/redefine the quota as lifetime quota and update protocol docs/tests accordingly.

### Finding 3
- Category: failure-point
- Severity: medium
- Line numbers: 263-271
- Description: `PluginRuntimePort::handle` accepts `CommandDescriptor` and `Contribution` requests directly and returns successful registration responses without verifying that a trusted manifest was loaded, that the contribution belongs to the loaded plugin, or that any required capability was declared/granted. This bypasses the manifest validation path used for host calls.
- Suggested fix direction: Require command/contribution registration to be derived from a loaded `PluginManifest`, or validate the request against the loaded plugin identity and declared capabilities before returning `CommandRegistered` / `ContributionRegistered`.

## `crates/legion-plugin/src/host.rs`

### Finding 4
- Category: failure-point
- Severity: critical
- Line numbers: 84-87, 223-232
- Description: The Wasmtime host does not enforce the manifest's `max_fuel`, `max_wall_time_ms`, or `max_memory_pages` quotas. The engine is created with the default `Config`, the store is created without fuel/epoch interruption or a resource limiter, and calls are executed synchronously with no timeout. A malicious fixture with an actual infinite loop or unbounded memory growth can consume CPU or memory despite declaring strict quotas.
- Suggested fix direction: Enable fuel consumption and/or epoch interruption in `Config`, add the manifest fuel budget before each call, install a `ResourceLimiter`/`StoreLimits` that caps memory/table growth from `max_memory_pages`, and enforce wall-clock timeout/cancellation around `func.call`.

### Finding 5
- Category: bug
- Severity: high
- Line numbers: 136-160, 223-231
- Description: Import validation allows exactly `env::host_log`, but `invoke` creates an empty `Linker` and never defines `env.host_log`. A module importing the only allowed host function successfully passes `load_fixture` but then traps during instantiation because the allowed import is missing. That makes the host interface internally inconsistent and records a crash instead of an audited host-call acceptance/denial.
- Suggested fix direction: Either reject all imports at load time if host calls are intentionally unavailable, or define `env::host_log` with `linker.func_wrap` and make the callback perform broker checks, quota accounting, and audit recording.

### Finding 6
- Category: bug
- Severity: high
- Line numbers: 181-188, 197-210, 263-265
- Description: Host-call quota enforcement is tied to successful guest invocations rather than actual host calls. `used_host_calls` is checked before instantiation and incremented once after an invocation returns successfully, so the quota does not correspond to calls to a host interface such as `host_log`. If host functions are added, a guest could make multiple host calls within one invocation while consuming only one quota unit.
- Suggested fix direction: Move `max_host_calls` accounting into each host-function callback, deny/trap when the per-invocation call count exceeds the quota, and reset that counter for each invocation.

### Finding 7
- Category: bug
- Severity: medium
- Line numbers: 226-231, 284-299, 333-347
- Description: Traps during instantiation or exported-function lookup call `finish_trap`, which only appends an audit entry and returns an error. The stored plugin state remains `Running`; `plugin_state` later infers `Crashed` by scanning the audit log, but the underlying state machine is inconsistent and subsequent internal logic still sees a running plugin.
- Suggested fix direction: On every `finish_trap` path, mutate the loaded plugin state to `PluginRuntimeState::Crashed` before returning. Avoid deriving state from audit history as a substitute for updating the state machine.

## `crates/legion-plugin/src/manifest.rs`

### Finding 8
- Category: failure-point
- Severity: medium
- Line numbers: 9-24, 31-69
- Description: Permission review rows are emitted per requested capability and `permission_reason_for_capability` uses `find_map`, so only the first matching contribution is surfaced. A manifest with many commands, formatters, LSP entries, scanners, or AI context providers sharing one capability can show a single benign-looking reason while hiding other uses behind the same capability. Several contribution variants also fall back to generic capability text, reducing install-review clarity.
- Suggested fix direction: Build review rows per contribution/capability pair or aggregate all matching contribution names for each capability. Add explicit coverage for every `PluginContribution` variant and tests for multiple contributions sharing the same capability.

## `crates/legion-plugin/src/registry.rs`

### Finding 9
- Category: failure-point
- Severity: high
- Line numbers: 41-63, 81-95
- Description: `SignedExtensionRegistry` validates only signature presence and trust decision; it never calls `validate_plugin_manifest`. As a result, a signed/trusted manifest with an invalid plugin id, empty module hash, bad ABI range, mismatched storage namespace, or missing grammar capability can be installed into the registry even though the runtime would reject it later.
- Suggested fix direction: Reuse `validate_plugin_manifest` with the current host ABI in `validate_installable` and map protocol validation errors into registry errors before inserting or updating registry state.

### Finding 10
- Category: bug
- Severity: high
- Line numbers: 81-94
- Description: The registry treats `signature.is_some()` plus a trusted self-reported `manifest.trust.decision` as sufficient proof that an artifact is signed and trusted. It does not verify that signer/algorithm/digest fields are non-empty, that the digest matches the manifest/module, or that the signature chains to an approved signer. A forged manifest can populate `signature` metadata and `ExplicitlyAllowed` trust fields and pass registry validation.
- Suggested fix direction: Validate signature metadata fields at minimum, and preferably verify a detached signature over canonical manifest/module bytes against an allowlisted signer before accepting `Trusted`/`ExplicitlyAllowed`. Treat trust metadata as verification output, not as input authority from the manifest itself.

## `crates/legion-plugin/src/wit_bindings.rs`

No direct findings in this file. The bare `wit_bindgen::generate!();` compiles successfully, but the runtime integration issues are covered above in `host.rs`, especially the allowed-but-unimplemented host import path.
