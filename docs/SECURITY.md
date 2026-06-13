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

Legion’s sandbox story is policy-backed and OS-assisted where the platform supports it.

Supported enforcement direction:

- Linux: bubblewrap-style sandboxing for stronger filesystem and egress containment.
- macOS: Seatbelt-style sandboxing.
- Windows: restricted-token / AppContainer-style mitigations, with weaker guarantees that are explicitly documented.

Important caveat: Windows is not presented as equivalent to the strongest Linux/macOS tiers. The product keeps the same security model across platforms, but the Windows implementation is intentionally honest about its weaker enforcement surface.

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

## Implementation anchors

This document is reviewed against the current implementation in these areas:

- `crates/legion-security` — trust, path, egress, remote, cloud, telemetry, raw-source, and redaction policy.
- `crates/legion-plugin` — manifest validation, capability/quota checks, deny-by-default plugin host calls, and namespace isolation.
- `crates/legion-ai-providers` — provider metadata, including metadata-only redaction defaults and local/offline support labels.

If one of those layers changes, this document should be updated in the same change set.
