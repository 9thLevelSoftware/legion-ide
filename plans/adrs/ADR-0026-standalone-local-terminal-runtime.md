# ADR-0026: Standalone Local Terminal Runtime

Status: Accepted for production implementation direction; Phase 8 GA acceptance deferred

## Context

Phase 7 validates remote process and PTY descriptors only. Phase 8 must add standalone local terminal runtime behavior without letting UI or terminal code own editor/workspace mutation authority.

## Decision

Implement a production terminal runtime behind explicit policy and feature activation. Keep the deterministic metadata-only `devil-terminal` fixture backend for conformance tests.

Production terminal runtime must be app-composed, trusted-workspace and capability-gated, bounded, redacted, metadata-audited, cleanup-safe, and unable to mutate workspace/editor/disk directly. Terminal-originated mutation candidates must become proposals. `devil-ui` may render terminal projections and emit terminal intents only.

## Required Implementation Gates

- Define terminal session lifecycle states, deterministic fixture backend, native PTY backend criteria, input/resize/close/kill semantics, cwd/env/shell policy, output chunking, truncation, timeout, kill tree cleanup, orphan detection, and diagnostics.
- Implement Windows ConPTY before claiming Windows GA and Unix PTY before claiming Unix-like parity.
- Persist metadata-only terminal summaries by default; never persist raw command bodies, transcripts, process output, secrets, or full environment values.
- Provide protocol contracts, runtime tests, platform PTY evidence, privacy/redaction tests, ownership tests, cleanup/orphan drills, and Phase 8 evidence before GA acceptance.

## Acceptance Reservation

This ADR accepts the production implementation direction only. Phase 8 GA remains blocked until native PTY behavior, policy enforcement, cleanup safety, platform evidence, and release evidence are archived.
