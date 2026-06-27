# Sandbox & Security Review

Reviewed files:
- `crates/legion-sandbox/src/lib.rs`
- `crates/legion-sandbox/src/landlock.rs`
- `crates/legion-sandbox/src/seatbelt.rs`
- `crates/legion-sandbox/src/windows.rs`
- `crates/legion-sandbox/src/network.rs`
- `crates/legion-security/src/lib.rs`
- `crates/legion-security/src/policy.rs`
- `crates/legion-security/src/risk.rs`

Validation note: `cargo check -p legion-security -p legion-sandbox` completed successfully. Findings below are security, policy, and failure-mode issues found by full-file review.

## Summary

- Total findings: 17
- Critical: 3
- High: 9
- Medium: 5
- Low: 0

## `crates/legion-sandbox/src/lib.rs`

### Finding 1

- Category: bug
- Severity: critical
- Line numbers: 290-315
- Description: `path_is_within_scope` is purely lexical and `normalize_path` collapses `..` without resolving symlinks or canonical paths. A symlink inside the workspace that points outside still passes `candidate.starts_with(scope)`, and relative paths can be normalized into an allowed-looking path without checking the actual filesystem target. This undermines the fail-closed write boundary.
- Suggested fix direction: Resolve the workspace root and candidate through a filesystem-aware canonicalization/openat-style policy before authorization, reject unresolved or symlink-escaping paths, and preserve fail-closed behavior for missing paths by validating the parent directory and final component separately.

### Finding 2

- Category: failure-point
- Severity: high
- Line numbers: 162-180, 260-272
- Description: `ActivatedSandbox::activate` cannot fail and always records an allowed activation event, even for an unsupported backend/platform pairing. The `SandboxError::UnsupportedBackend` and `DocumentedFallbackRequired` variants are not part of activation, so callers can believe a sandbox is active when no platform backend was actually enforced.
- Suggested fix direction: Make activation return `Result<ActivatedSandbox, SandboxError>`, validate backend/platform compatibility, and only emit an allowed activation event after the OS backend has actually been installed or an explicit weaker fallback has been accepted by policy.

## `crates/legion-sandbox/src/landlock.rs`

### Finding 3

- Category: stub
- Severity: high
- Line numbers: 15-24
- Description: `LandlockProfile::compile` only returns human-readable notes such as `bwrap --unshare-net` and `Landlock write rules deny paths outside workspace`. It does not construct Landlock rules, bubblewrap arguments, kernel ABI checks, or any enforceable profile data.
- Suggested fix direction: Replace note-only compilation with a structured profile containing actual bubblewrap arguments and Landlock rules, include host capability/kernel-version detection, and fail closed when required Linux enforcement primitives are unavailable.

## `crates/legion-sandbox/src/seatbelt.rs`

### Finding 4

- Category: stub
- Severity: high
- Line numbers: 15-24
- Description: `SeatbeltProfile::compile` emits generic strings, not a valid macOS Seatbelt profile. The rules do not interpolate or escape the workspace root or egress destinations, so the compiled result is not directly enforceable and cannot express the configured scope.
- Suggested fix direction: Generate a real Seatbelt profile DSL with safely quoted workspace paths and network rules, validate/compile it before use, and fail closed if the profile cannot be installed.

## `crates/legion-sandbox/src/windows.rs`

### Finding 5

- Category: failure-point
- Severity: high
- Line numbers: 21-48
- Description: On non-Windows hosts, `WindowsProfile::compile` returns `Ok(Self)` using `SandboxBackend::DocumentedFallback` instead of returning `SandboxError::DocumentedFallbackRequired` or `UnsupportedBackend`. Callers that treat `Ok` as an active sandbox will silently proceed with weaker guarantees.
- Suggested fix direction: Return an explicit error for unavailable Windows APIs unless the caller has selected and audited a documented fallback path. Require policy-level opt-in before constructing fallback profiles.

### Finding 6

- Category: bug
- Severity: medium
- Line numbers: 37-47
- Description: `documented_fallback` is always populated, including the `cfg!(windows)` path where `SandboxBackend::RestrictedToken` is selected. This makes successful Windows restricted-token profiles look like weaker fallback profiles and can cause incorrect audit or UI messaging.
- Suggested fix direction: Set `documented_fallback` only in the fallback branch, and add tests for both Windows and non-Windows compilation behavior.

## `crates/legion-sandbox/src/network.rs`

### Finding 7

- Category: failure-point
- Severity: medium
- Line numbers: 57-72
- Description: `allowlist_matches_target` treats a host-only allowlist entry as matching the same host with any explicit port. For example, allowing `localhost` also allows `localhost:1`, `localhost:2375`, or any other service on that host. That may be broader than an egress destination allowlist intends, especially for loopback services.
- Suggested fix direction: Make port matching explicit in policy: either require exact host:port entries for non-default ports, define safe default ports per scheme, or add a separate `allow_any_port` flag so broad host-level egress is intentional and auditable.

## `crates/legion-security/src/lib.rs`

### Finding 8

- Category: bug
- Severity: critical
- Line numbers: 133-148, 183-191
- Description: `NormalizedPolicyPath::starts_with` only checks prefix equality when the policy root has a prefix. The default roots are `./`, which parse to no prefix and no segments, so they match essentially every absolute or relative path that is not blocked earlier. With the default policy, paths outside the workspace such as `/tmp/file` can be allowed because the empty relative root matches all candidates.
- Suggested fix direction: Require candidate and root prefixes to match exactly, reject empty relative roots unless they have been resolved against a trusted workspace root, and make the default policy carry an explicit canonical workspace root rather than `./`.

### Finding 9

- Category: bug
- Severity: critical
- Line numbers: 1852-1889
- Description: Filesystem capability handling only applies `PathPolicy` to `fs.write` when `target_path` is present. If `fs.write` omits `target_path`, the code returns `Allow`; non-write `fs.*` capabilities also return `Allow` without checking readable roots, blocked roots, or trust-sensitive path boundaries. A malformed or under-specified request can therefore bypass path policy entirely.
- Suggested fix direction: Require target path metadata for every filesystem capability that touches disk, apply `PathPolicy` to read/list/write paths, and deny requests that omit path context unless the capability is explicitly pathless and safe.

### Finding 10

- Category: bug
- Severity: high
- Line numbers: 1402-1411
- Description: `remote.fs.read` and `remote.fs.write` are allowed whenever remote runtime sessions and filesystem access are enabled; only write size is checked. The remote filesystem path is not required and `PathPolicy` is not consulted, so enabling remote filesystem support bypasses local readable/writable root restrictions.
- Suggested fix direction: Require canonical remote target path metadata, apply the same read/write root and blocked-root checks used for local filesystem capabilities, and deny missing path context.

### Finding 11

- Category: failure-point
- Severity: high
- Line numbers: 383-414, 1913-1920, 1948-1958
- Description: `CommandTaxonomy` classifies `git` as `Read` based only on the first token. Mutating or networked commands such as `git push`, `git clean`, or `git checkout` are therefore treated as read operations in `cmd.*` checks, and terminal launch only denies `CommandClass::Network`, not shells or mutating tools, once terminal runtime is enabled.
- Suggested fix direction: Classify commands by verb/subcommand and argument shape, treat unknown subcommands as deny-by-default for untrusted workspaces, and explicitly gate shell and mutating command launches behind stronger policy checks.

### Finding 12

- Category: bug
- Severity: high
- Line numbers: 457-475, 1925-1933
- Description: `LspLaunchPolicy` defines `allowed_binaries` and `deny_network_refresh`, but the `lsp.*` decision path ignores both fields and allows any LSP capability in trusted workspaces. A malicious workspace could request an arbitrary language server command despite the policy surface suggesting a binary allowlist.
- Suggested fix direction: Require command metadata for `lsp.launch`, compare the resolved binary against `allowed_binaries`, reject network-refresh commands when configured, and deny unknown `lsp.*` capabilities by default.

### Finding 13

- Category: bug
- Severity: high
- Line numbers: 2012-2054
- Description: `CapabilityBrokerPort::handle` passes `CapabilityRequest::Grant` and `CapabilityRequest::Deny` through directly without checking namespace, policy, trust state, principal, or whether the grant was previously authorized. A caller that can submit a `Grant` request can receive a granted response without going through `DenyByDefaultBroker::decide`.
- Suggested fix direction: Treat external broker input as requests only, or validate grant/deny records against a signed/known decision source. Do not return arbitrary grants from untrusted request payloads.

### Finding 14

- Category: failure-point
- Severity: medium
- Line numbers: 2012-2048
- Description: `handle` clones the broker for every request and increments the clone's counter. When callers omit `decision_id`, repeated `handle` calls on the same broker can reuse the same generated `CapabilityDecisionId`, weakening audit correlation and deduplication.
- Suggested fix direction: Store the counter behind interior mutability or require caller-provided decision IDs at the protocol boundary. Ensure generated IDs are monotonic for the actual broker instance.

## `crates/legion-security/src/policy.rs`

### Finding 15

- Category: bug
- Severity: high
- Line numbers: 69-76
- Description: `ProposalAutoApprovalPolicy::allows_rule_ids` returns true when `enabled` is true and `rule_ids` is empty because `.all(...)` on an empty iterator is true. This can allow auto-approval without any deterministic rule evidence.
- Suggested fix direction: Require `!rule_ids.is_empty()` before accepting the set, and consider requiring every rule ID to be known, unique, and tied to the current risk assessment.

## `crates/legion-security/src/risk.rs`

### Finding 16

- Category: failure-point
- Severity: medium
- Line numbers: 114-130
- Description: `is_dependency_or_lockfile` catches several lockfiles but misses common dependency manifests such as `Cargo.toml` and `package.json`. Dependency changes in Rust or npm projects can therefore be classified as low risk by this rule.
- Suggested fix direction: Add all supported ecosystem manifests and lockfiles, including `Cargo.toml`, `package.json`, `deno.json`, `bun.lockb`, and other project package descriptors used by Legion-supported workspaces.

### Finding 17

- Category: failure-point
- Severity: medium
- Line numbers: 218-248, 395-404
- Description: When `workspace_root` is absent, the path-scope rule emits an allow finding with an informational label. If no other rule denies, the aggregate risk remains `Low`, even though path containment could not be evaluated. Missing scope metadata should not be treated as low-risk evidence.
- Suggested fix direction: Make absent workspace root a deny or at least a non-low/manual-review finding for auto-approval purposes, and require scope metadata before classifying proposals as low risk.
