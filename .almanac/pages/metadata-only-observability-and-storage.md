---
title: Metadata-Only Observability And Storage
summary: "Observability and storage default to metadata-only redaction and reject structurally invalid audit records instead of silently persisting unsafe payloads."
topics: [observability, decisions, architecture, stack]
sources:
  - id: observability-lib
    type: file
    path: crates/legion-observability/src/lib.rs
    note: Defines redacting sinks, metadata defaults, and validation tests for event envelopes.
  - id: storage-lib
    type: file
    path: crates/legion-storage/src/lib.rs
    note: Defines storage ports, metadata-only persistence, and validation tests.
  - id: save-tests
    type: file
    path: crates/legion-app/tests/workspace_vfs_integration.rs
    note: Verifies event ordering and non-zero core ids in save workflows.
  - id: beta-harness
    type: file
    path: crates/legion-desktop/src/beta.rs
    note: Shows metadata-only evidence generation for GUI beta workflows.
status: active
verified: 2026-06-08
---
The default retention posture is metadata-only. `[[crates/legion-observability/src/lib.rs]]` builds event sinks and envelope builders around `RedactionHint::MetadataOnly`, redacts payloads by default, and rejects invalid envelopes such as nil causality ids, zero correlation ids, and zero event sequences in tests [@observability-lib].

## What gets stored

`[[crates/legion-storage/src/lib.rs]]` persists protocol-validated metadata records for proposals, audits, semantic metadata, breakpoints, sessions, remote state, collaboration state, plugin state, tracker records, retention audits, and telemetry spool records [@storage-lib]. The storage layer is explicit about staying metadata-only for these persisted records, and the tests assert that persisted state does not quietly become raw-payload storage [@storage-lib].

## Why this matters to feature work

Save workflows, assisted AI, delegated tasks, remote sessions, workflow orchestration, and GUI beta evidence all rely on audit/event trails. The repository's rule is that these trails are durable and queryable without retaining raw source, prompts, terminal payloads, or provider payloads by default [@observability-lib] [@beta-harness].

The save integration tests check this indirectly by asserting event ordering and non-zero core ids during proposal-mediated save paths [@save-tests]. The desktop beta harness does it directly by writing markdown evidence that records paths, counts, statuses, and labels while explicitly excluding raw source and prompt payloads [@beta-harness].

## Consequence

If a new feature needs durable storage, it has to justify both schema shape and privacy shape. "We already log events" is not enough. The expected pattern is protocol validation plus metadata-only persistence first, with raw retention treated as a separately gated surface. Related pages: [[phase-gates-and-evidence]], [[assisted-ai-and-delegated-tasks]], [[bounded-runtime-surfaces]].
