# ADR-0022: Remote Edge Workspace Agent

## Status

Accepted for Phase 7 deterministic edge workspace runtime harness.

## Context

Phase 7 adds edge-executed remote development after proposal-mediated mutation, semantic/LSP contracts, plugin isolation, and local collaboration are accepted. The implementation must not let remote code own local editor text, UI state, or durable workspace mutation.

## Decision

Activate `devil-remote` as a deterministic, metadata-first remote workspace runtime harness. The crate owns remote session lifecycle, transport-envelope validation, remote-side fixture filesystem metadata, bounded remote execution descriptors, reconnect state, offline-resume metadata, and metadata-only audit records.

`devil-remote` is default-off by configuration and can be activated only by an app-owned composition root. It depends only on protocol and utility crates allowed by dependency policy, and it has no dependency on app, UI, editor, project, collaboration, terminal, LSP, AI, plugin, tracker, memory, or semantic-index internals.

Durable local workspace writes remain outside `devil-remote`. Remote filesystem mutation requests require proposal IDs and explicit write preconditions before remote-side fixture state can change; local disk mutation still requires app/workspace proposal authority.

## Rejected Alternatives

- Raw network helper APIs: rejected because they bypass typed protocol envelopes, capability metadata, and audit correlation.
- Remote runtime owning local files or buffers: rejected because it violates editor/workspace authority boundaries.
- UI-owned remote session state: rejected because UI remains projection-only.

## Consequences

- Phase 7 acceptance covers deterministic local validation of remote lifecycle, filesystem, execution descriptors, reconnect/offline metadata, and audit paths.
- Production network transport, broad platform parity, operational hardening, and standalone local terminal/LSP crates remain future-gated unless separately accepted.
