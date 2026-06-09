# Phase 7 Remote Security Threat Model

## Status

Accepted for deterministic Phase 7 runtime harness.

## Threats And Controls

- Remote session activation in untrusted workspaces: denied by app composition and `RemoteDevelopmentPolicy`.
- Remote capability escalation: denied by default unless explicit `remote.*` policy flags are enabled.
- Remote write bypasses proposals: denied by `legion-remote` unless a proposal ID and full write guards are present.
- Stale remote filesystem state clobbers fixture state: stale fingerprint, generation, file version, and snapshot mismatch return explicit stale outcomes.
- Non-loopback egress in air-gap policy: denied by security tests.
- Raw source, process output, terminal transcript, transport payload, or secrets in audit: rejected by protocol/storage/observability validators.
- Local disk or editor mutation by remote runtime: prevented by dependency boundaries and app-owned composition tests.

## Residual Risks

- Production encrypted network transport is not activated by this deterministic harness and remains Phase 8 hardening.
- Real process isolation and PTY backend hardening remain Phase 8 hardening beyond descriptor validation.
