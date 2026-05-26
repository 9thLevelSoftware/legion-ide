# Phase 7 Future Surface Deferral Audit

## Status

Accepted.

## Deferred Surfaces

- Hosted telemetry remains inactive.
- Hosted AI/cloud providers remain outside Phase 7 remote transport scope.
- Raw source, raw terminal transcript, raw process output, and raw transport payload retention remain rejected by default.
- Standalone local terminal runtime remains inactive.
- Standalone `devil-terminal`, `devil-lsp`, and production remote transport crates remain future-gated outside the accepted deterministic Phase 7 harness.
- Production operations, migrations, privacy metrics, enterprise policy profiles, and broad platform parity remain Phase 8 hardening.

## Accepted Surface

- `devil-remote` is active only for deterministic edge workspace harness behavior, protocol DTO validation, policy-gated descriptors, reconnect/offline metadata, and metadata-only audit.
