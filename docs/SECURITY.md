# Legion Security Model and Disclosure Policy

This document describes the public-facing security posture of Legion IDE as it exists today: what the product is designed to protect, which boundaries are enforced in code, where the platform is intentionally weaker, and how to report vulnerabilities responsibly.

## Security principles

Legion is built around deny-by-default policy. The product is designed so that:

- UI layers do not own durable workspace state.
- AI and provider code do not mutate files directly.
- Plugins do not receive ambient host authority.
- Network and export paths are gated by mode, trust state, and capability policy.
- Raw traces are not retained by default.

When a policy check fails, the expected behavior is to deny the action rather than fall back to a broader permission.

## Mutation gating

Workspace mutation is proposal-mediated and authority-bound. In practice, that means Legion is intended to keep these paths separate:

- projection / display surfaces;
- proposal and review surfaces;
- workspace mutation surfaces;
- execution and transport surfaces.

The implementation currently enforces this through the workspace and security layers rather than trusting the UI, a provider callback, or a plugin to write directly. Writes are checked against trust state, path policy, version and fingerprint expectations, and fail-closed conflict handling.

What this means for users:

- a suggestion is not a write;
- a worker result is not a write;
- a plugin result is not a write;
- approval or policy gates must be satisfied before a durable mutation is applied.

## Sandbox guarantees and platform caveats

Legion’s sandbox story is policy-backed and OS-assisted where the platform supports it.  Each platform backend reports its actual enforcement honestly via `SandboxEnforcementReport`; no backend ever claims enforcement it cannot deliver.

### Per-platform enforcement matrix (M10, PKT-SANDBOX)

| Capability | Linux (Landlock) | macOS (Seatbelt) | Windows (Job Object) |
|---|---|---|---|
| Filesystem write isolation | **Enforced** — Landlock `AccessFs::WriteFile` | **Enforced** — SBPL `(deny default)` + selective `file-write*` | **Not enforced** — job object does not restrict paths |
| Filesystem read isolation | Partial (Landlock write, not read) | **Not enforced** — SBPL `(allow file-read* (subpath "/"))` grants unrestricted filesystem reads | **Not enforced** |
| Network egress isolation | **Enforced when bubblewrap is available** — `bwrap --unshare-net` for deny-all egress (empty allowlist); selective host allowlists not implemented on Linux (caveat reported). If `bwrap` is missing, `network_enforced=false` with honest caveat | **Enforced** — SBPL `(deny network*)` + per-host allows | **Not enforced** |
| Process kill on timeout | Yes — `child.kill()` via SIGKILL | Yes — `child.kill()` via SIGKILL | **Yes** — `KILL_ON_JOB_CLOSE` kills the entire process group |
| Backend identifier | `landlock-vN` or `landlock-vN+bwrap-unshare-net` | `seatbelt-sbpl` | `job-object-kill-on-close` |

**Windows note (WS-A-D Phase 3 C2 residual cut line):** The Windows implementation uses a Win32 Job Object with `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE`.  This enforces process lifetime (all child processes are killed when the job handle closes, including on timeout) but does **not** restrict filesystem or network access.  The `SandboxEnforcementReport` returned on Windows always has `filesystem_write_enforced: false` and `network_enforced: false` with caveat labels `windows-no-restricted-token`, `windows-no-filesystem-enforcement`, and `windows-no-network-enforcement`. Escape-probe tests assert the residual is **real** (outside-root writes succeed) and that the report never claims otherwise.

Upgrading the Windows tier to restricted tokens or AppContainer would require privileges (e.g. `SE_ASSIGNPRIMARYTOKEN_PRIVILEGE`) not available to normal user processes, or a larger AppContainer packaging redesign. That upgrade is **explicitly deferred** — see `plans/evidence/production/WS-A-D/phase-3-sandbox/C2-windows-fs-residual.md`. Until it lands, product UI and docs must not claim Windows FS or network isolation parity with Linux/macOS.

### Escape probe

`crates/legion-sandbox/src/bin/sandbox-escape-probe.rs` is a test binary used to validate sandbox enforcement in integration tests.  It accepts `write <path>` and `connect <addr>` subcommands, prints `WRITE_OK`/`WRITE_DENIED` or `CONNECT_OK`/`CONNECT_DENIED`, and is exercised by `crates/legion-sandbox/tests/escape_attempts.rs`.

Important caveat: Windows is not presented as equivalent to the strongest Linux/macOS tiers. The product keeps the same security model across platforms, but the Windows implementation is intentionally honest about its weaker enforcement surface.

### Product spawn path (WS-A-D Phase 3 C3)

Delegated `TerminalCommand` tool calls in product composition go through `legion_sandbox::spawn::spawn_sandboxed` (`AppDelegatedToolHost` in `legion-app`). Each successful spawn records a live `SandboxEnforcementReport` (`backend_used`, FS write/read flags, `network_enforced`, caveat labels). That report is:

- appended to the tool output as `sandbox live enforcement: …`
- stored on the delegate workflow and projected into `plan_only_disclaimers`
- surfaced on the desktop sandbox panel as `sandbox runtime: …` rows

Compile-time profile summaries on the panel describe *typical* OS capability; **the live report is authoritative** for what the last spawn actually enforced. Interactive terminal PTY is **not** wrapped by batch `spawn_sandboxed` (trust/capability gates). **Live non-fake DAP adapters** use `spawn_sandboxed_stdio` (C4): Linux Landlock (+ optional bwrap net), macOS Seatbelt, Windows job-object kill-on-close when assignable — with honest caveats; fake adapter CI path stays unsandboxed.

Legion should be treated as a trusted desktop application with sandboxed execution lanes, not as a formal operating-system boundary or a replacement for host hardening.

## Egress policy

Legion’s network posture is deny-by-default and mode-aware.

At a high level:

- Manual mode forbids AI, cloud, hosted telemetry, and any network-capable AI action.
- Assisted and delegated flows only use network paths that satisfy policy, privacy, and trust gates.
- Air-gap policy denies non-loopback network access.
- Hosted provider invocation is denied in air-gap policy.
- Collaboration and remote-development transports are denied when they would use non-loopback egress in air-gap policy.
- Plugin network host calls are denied by default.

Local loopback services may still be allowed when the policy explicitly permits them. The intent is to support local-only models and local services without silently widening egress to the public network.

## Secret handling and retention

Legion defaults to metadata-only retention.

The implementation is designed so that raw prompts, responses, command output, diffs, and trace payloads are not stored or exported unless the relevant consent and policy path is explicitly enabled.

The redaction layer is conservative and scans for common secret markers, including examples such as:

- PEM / private-material delimiters;
- AWS secret access keys;
- OpenAI-style API key markers;
- generic `api_key=` assignments;
- Slack bot token prefixes;
- `sk-` provider key prefixes.

If a payload contains a sensitive marker, it is expected to be redacted before retention or export. If the policy path does not allow retention/export, the payload should be denied rather than silently stored.

## Plugin isolation

Plugins are isolated by design:

- the runtime exposes protocol DTOs rather than ambient host objects;
- manifests are validated before activation;
- capability and quota metadata are checked before invocation;
- unknown capabilities are denied;
- namespaces are required;
- ambient host authority is denied;
- plugin network access is denied by default.

Plugin state is namespaced, and the runtime tracks metadata without keeping WASM memory or host objects alive. That keeps plugin execution bounded and prevents plugins from becoming a back door into the main workspace or host environment.

## What this does not promise

Legion does not promise:

- cryptographic proof of sandbox isolation;
- immunity from a compromised host operating system;
- protection against a malicious kernel, hypervisor, or physical attacker;
- perfect detection of every possible secret format;
- equivalent sandbox strength on every operating system.

The security model is a layered policy system, not a claim of absolute containment.

## Responsible disclosure

If you believe you have found a vulnerability, report it privately first.

Please include:

- the Legion version or commit range;
- operating system and platform tier;
- mode / trust state / policy settings used when the issue appeared;
- concise reproduction steps;
- the expected behavior versus actual behavior;
- sanitized evidence that does not reveal secrets or raw private data.

Please do not:

- publish exploit details publicly before the issue is acknowledged and triaged;
- include secrets, tokens, raw prompts, or raw traces in the report;
- attempt destructive testing against real user data or real third-party services.

After triage, the maintainers can coordinate a fix and a disclosure timeline. If a private reporting channel is available in your deployment, use that channel; otherwise use the project’s normal private maintainer path and request a confidential follow-up.

## Terminal policy (M8)

The terminal runtime is gated behind workspace trust and capability policy:

- Untrusted workspaces are denied at the product gate — unconditionally, before any capability broker evaluation. `enable_terminal_runtime_for_tests()` cannot override this for untrusted callers.
- Trusted workspaces in Manual mode auto-enable the terminal on the first explicit launch intent.
- The `LEGION_SECRET*` and `LEGION_TOKEN*` environment variable prefixes are on the hard deny-list and are stripped before any PTY spawn, regardless of trust state or `passthrough_env` setting. The effective env configuration (passthrough enabled/disabled + deny-prefix count) is recorded in the launch audit record as metadata only.
- When `passthrough_env=false`, the child process receives a minimal platform-safe baseline (Windows: `SystemRoot`, `SystemDrive`, `PATH`, `TEMP`, `TMP`, `COMSPEC`, `USERPROFILE`, `HOMEDRIVE`, `HOMEPATH`, `windir`; Unix: `PATH`, `HOME`, `TERM`, `USER`, `SHELL`, `LOGNAME`) — no other parent variables are forwarded. The deny-list is still applied on top of this baseline. This prevents the shell from crashing while ensuring parent variables outside the baseline are not inadvertently inherited.
- No raw command output, shell command lines, or process arguments are written to audit records. Redaction stays.
- Shell binary taxonomy: only classified shell binaries (`cmd`, `powershell`, `pwsh`, `bash`, `sh`, `zsh`) are permitted; unrecognized commands are denied by `DenyByDefaultBroker`.

## Implementation anchors

This document is reviewed against the current implementation in these areas:

- `crates/legion-security` — trust, path, egress, remote, cloud, telemetry, raw-source, and redaction policy; terminal command taxonomy.
- `crates/legion-app/src/terminal_policy.rs` — shell selection, env allow/deny policy, scrollback limit, failure kind enum.
- `crates/legion-plugin` — manifest validation, capability/quota checks, deny-by-default plugin host calls, and namespace isolation.
- `crates/legion-ai-providers` — provider metadata, including metadata-only redaction defaults and local/offline support labels.
- `crates/legion-sandbox/src/spawn.rs` — `spawn_sandboxed`, `SandboxSpawnSpec`, `SandboxEnforcementReport`, `SandboxedCommandOutput`, SBPL profile generator, per-platform enforcement backends.

If one of those layers changes, this document should be updated in the same change set.
