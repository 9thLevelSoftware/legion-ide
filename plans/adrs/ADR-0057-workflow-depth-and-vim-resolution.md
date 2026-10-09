# ADR-0057: Concentrate Git and LSP lifecycle state and Vim text resolution

## Status

Accepted design, 2026-10-08. Implementation verification is recorded in
`../evidence/production/module-depth-2026-10-08.md`; this does not promote a
product-readiness ledger row.

## Context

Git worker scheduling required the app caller to maintain a second in-flight
flag, generations, and pending mutation state. LSP write admission kept pending
and deferred requests in separate maps, requiring paired cleanup. Vim motion
callers cloned whole editor text and translated character coordinates, while
operator resolution parsed the document twice.

## Decision

- An app-internal `GitWorkflow` owns scheduling, coalescing, stale-result
  rejection, queued mutations, and disconnection outcomes. Existing app policy
  checks and project Git execution remain authoritative. The existing injected
  snapshot runner remains the deterministic test adapter.
- An app-internal `LspWriteLifecycle` owns bounded admission and the lifetime of
  pending/deferred write operations. Submission reserves capacity before calling
  the worker; refused submissions release it. Completion, cancellation, and
  document/session invalidation remove the queued request with the operation.
  Code-action admission returns its operation identity directly. Document sync,
  editor identity checks, proposal translation, and apply authority stay with
  their existing owners under ADR-0018 and ADR-0034.
- `legion-editor` resolves Vim text motions and operator ranges against immutable
  snapshots and returns byte-based editor coordinates. Reads use bounded chunks;
  register extraction copies only the requested range. UI parsing remains pure,
  and app dispatch retains register, mode, and transaction effects. Existing UI
  resolver functions remain compatible and share their parsed representation
  internally. No editor dependency on UI or project is introduced.

These are interfaces for existing workflows, not new provider, network, or
workspace-mutation capabilities. No new crate or internal dependency edge is
introduced. The 5 MiB full-text cache budget and proposal-mediated saves remain
unchanged. Scalar-character Vim semantics are preserved; this decision does not
claim a new grapheme-motion contract or complete large-file modal editing.

Native motion coordinates exclude CRLF terminators. Append (`a`) advances to
the next scalar boundary, including the content-end insertion position, rather
than using normal-mode movement clamping. This fixes insertion before the final
character and is covered with typed insertion on LF and CRLF lines.

## Verification gate

Before considering this implementation locally verified, run:

- Git workflow and remote-policy integration tests, including coalescing, queued
  operation ordering, and disconnected-worker outcomes.
- App language tests, including bounded admission, document invalidation,
  cancellation followed by late response, restart, and stale-edit handling.
- Vim editor/UI tests and app modal-editing integration tests, including native
  coordinate parity and documents above the full-text cache budget.
- Formatting, affected-crate Clippy, dependency-policy, and documentation-hygiene
  checks.

Keep cross-platform CI, windowed GUI evidence, and measured performance distinct
from these local contracts. Preserve existing tests of observable behavior;
update tests tied to the old collection ownership to use the new interface.

## Consequences

Callers no longer reproduce Git scheduling or paired LSP collection cleanup.
Editor-owned Vim resolution keeps text access and coordinate semantics local.
The remaining full-text Vim paths and broader LSP document-sync ownership can
be addressed independently. Tracker durability remains separate work; a second
storage adapter is not introduced by this refactor.
