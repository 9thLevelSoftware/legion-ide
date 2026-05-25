# Phase 4 Cloud Provider Deferral Audit

Date: 2026-05-25

## Scope

Phase 4 accepts only deterministic local provider execution for tests and local/loopback provider routing after policy approval. Cloud providers remain disabled until a provider-specific ADR, dependency-policy update, allowlist policy, credential boundary, redaction tests, and air-gap tests are accepted.

## Evidence

- `devil-ai::ProviderRouter` refuses `HostedRemote` provider classes as `provider.remote_deferred` metadata.
- `devil-security` denies non-loopback provider invocation in air-gap mode.
- `devil-security` denies hosted telemetry, hosted embeddings, gateway capability ids, and unapproved outbound network access.
- `devil-security` denies remote provider invocation even when a remote host is allowlisted unless `allow_remote_provider` is explicitly enabled.
- `devil-ai-providers::OpenAiStub` remains a refusing stub and does not read credentials or invoke remote egress.
- No accepted Phase 4 provider code adds HTTP SDKs, keychain access, cloud credentials, or hosted gateway dependencies.

## Test evidence

- `cargo test -p devil-ai --all-targets`
- `cargo test -p devil-ai-providers --all-targets`
- `cargo test -p devil-security --all-targets`

## Acceptance

- [x] Hosted remote provider routes are refused as metadata.
- [x] Cloud provider stubs remain inactive.
- [x] Air-gap mode blocks hosted providers and non-loopback egress.
- [x] Credential material does not cross provider, protocol, observability, storage, tracker, or memory records.
