# ADR-0040: Concurrent Edit Substrate

## Status

Accepted — ratified for Production Master Plan v0.1 M0 on 2026-06-10.

This ADR ratifies the Production Master Plan v0.1 §6 recommendation
verbatim (option (a), **operation/anchor layer now (stable position
IDs + version vectors over `legion-text`), full CRDT (Loro / yrs /
homegrown SumTree-style) deferred**) and records the resulting crate
boundary: the stable position anchor surface and the operation log
that diff overlays, decorations, diagnostics, AI proposals, and
concurrent agent edits attach to live in `legion-text` (the
`TextEdit` / `TextSnapshot` / `BufferVersion` / `SnapshotId`
substrate the existing rope layer already ships) and compose through
the existing `legion-collaboration` deterministic in-memory operation
log + replay runtime (the `CollaborationSessionRuntime` /
`CollaborationDocumentOperation` / `CollaborationVersionVector` /
`CollaborationOperationId` family) over the existing
`legion-collaboration` ↔ `legion-protocol` ↔ `legion-text` edge
chain. `legion-editor` consumes anchors and operation records through
its existing `legion-text` edge. `legion-ui` stays projection-only
and never owns anchor or operation-log state. The full CRDT decision
(Loro vs yrs vs homegrown SumTree-style over the anchor layer) is
explicitly deferred to WS-16.T1 (post-GA collaboration track), so the
M0 ratification does not force that future decision and does not
declare any CRDT runtime dependency today.

## Context

Legion needs positions that survive ordinary text changes so
diagnostics, decorations, AI diff overlays, proposal hunks, and
concurrent agent edits stay attached to the text they describe.
Without stable anchors, every feature that references a buffer
position — completion popups, hover popovers, go-to-def targets,
inline-edit selections, AI diff hunks, agent-edit-while-typing
overlays, diagnostic squiggles, code-folding regions, multi-cursor
selections, and the post-GA collaboration surface — drifts on every
keystroke and either silently misroutes or fights the user. The
mid-2026 editor market treats the anchor / position-id layer as
table-stakes: Zed exposes it through the `SumTree` Rope's stable
position IDs, Helix exposes it through the `Rope` slice-stable
positions, and Lapce exposes it through its own anchor table. The
master plan §2.2 calls this out as the substrate that has to land
before Delegate (M3), because the agent harness must be able to
attach its proposed edits to the same anchors the diagnostics, the
diff overlays, and the inline-edit UI attach to, and because
retrofitting anchors later is the classic editor-rewrite trigger —
adding anchors under an existing position-only contract requires
threading a position-id through every call site that holds a
position. The master plan §6 row 240 records the recommendation:
"**(a)**. Agent+human concurrent editing needs anchors at M3;
collaboration needs the CRDT only at post-GA. Retrofitting anchors
later is the classic editor-rewrite trigger — do the layer now,
cheaply."

The current Legion state is real and exercised by tests today:

- `legion-text` (`crates/legion-text/src/lib.rs`, 2019 lines) is the
  text-substrate crate over which the anchor / operation-log layer
  composes. The rope layer ships `TextPosition` (around line 35),
  `TextRange` (around line 65), `TextEdit` (around line 108,
  carrying `range: TextRange` + `replacement: String` + `replacement_utf16_len`),
  `TextSnapshotDescriptor` (around line 176, carrying
  `snapshot_id: SnapshotId`, `buffer_version: BufferVersion`,
  `content_hash: String`, `byte_len: usize`, `line_count: usize`,
  `memory_footprint_bytes: usize`, `retention_pin_reason`),
  `TextSnapshot` (around line 240, an immutable rope view that
  clones cheaply through `Arc<Rope>` and a `LineIndex` cache and is
  bounded by a 5 MiB full-cache budget), and `TextBuffer`
  (around line 938, the mutable rope owner with `apply_edit` /
  `try_apply_edit` at lines 1209 / 1214, `set_version` at line
  1102, `byte_offset` / `position` / `utf16_position` /
  `byte_offset_from_utf16` round-trips at lines 1126-1154, and the
  `LineIndex` rebuild on edit at line 1184). The 24 in-source
  contract tests at the bottom of `crates/legion-text/src/lib.rs`
  exercise the rope boundary — the `crlf_is_single_line_ending_for_lsp`,
  `property_edits_match_string_model_for_ascii`,
  `chunk_hashes_change_only_for_edited_chunk_when_boundaries_stay_stable`,
  `utf8_and_utf16_conversion_work_across_chunk_boundures`,
  `large_file_typical_keystroke_edit_smoke`,
  `huge_single_line_files_are_bounded_without_full_text_materialization`,
  `large_snapshot_line_slices_and_chunks_are_bounded_by_default`,
  `large_snapshot_can_materialize_save_payload_from_chunks`,
  `edits_can_shrink_below_and_grow_above_full_cache_budget`,
  `explicit_full_text_access_fails_for_uncached_snapshot_and_buffer`,
  `opening_larger_than_budget_uses_degraded_cache_free_mode`,
  `large_file_typical_keystroke_edit_smoke`,
  `position_roundtrip_multibyte_columns_are_bytes`,
  `buffer_insert_delete_replace`,
  `buffer_rejects_non_boundary_edits`,
  `content_hash_has_expected_prefix`,
  `snapshot_descriptor_has_required_metadata`,
  `snapshot_clone_is_cheap_and_immutable`,
  `text_position_display`, `text_range_empty`,
  `utf16_golden_surrogate_pairs`, `utf16_range_golden`,
  `line_slice_can_span_multiple_chunks`,
  `crlf_pair_is_not_split_when_near_chunk_boundary`,
  `visible_line_slices_return_exact_requested_range` tests. The
  M0 ratification does **not** change the `legion-text` boundary
  the WS-01.T6 anchor/operation layer composes through: the
  `legion-text` policy entry at `plans/dependency-policy.md` §1
  line 23-24 authorizes `legion-text` to depend on `legion-protocol`
  and nothing else, and the WS-01.T6 anchor layer is a
  protocol-mediated position-id + operation record over the
  existing `TextEdit` / `TextSnapshot` / `BufferVersion` /
  `SnapshotId` substrate (no new internal edge, no new external
  runtime dep). The M0 ratification explicitly forbids
  `legion-text` from declaring any CRDT runtime dependency
  (`loro`, `yrs`, `diamond-types`, `automerge`, etc.) at M0;
  the WS-16.T1 post-GA decision is the path that authorizes a
  CRDT runtime dep if the post-GA substrate needs it.

- `legion-collaboration` (`crates/legion-collaboration/src/lib.rs`,
  1232 lines) is the deterministic, metadata-first collaboration
  operation log + replay runtime. It already ships the
  `CollaborationSessionRuntime` (around line 101, carrying
  `descriptor: CollaborationSessionDescriptor`, `participants:
  HashMap<CollaborationParticipantId, CollaborationParticipant>`,
  `operations: Vec<CollaborationDocumentOperation>`,
  `acknowledgements: Vec<CollaborationAcknowledgement>`, `causal_gaps:
  Vec<CollaborationCausalGap>`, `presence: HashMap<...>`,
  `participant_sequences: HashMap<...>`, `operation_ids: HashSet<CollaborationOperationId>`)
  with `submit_operation` (around line 242, with the
  duplicate/gap/stale/conflict fail-closed paths),
  `publish_presence` (around line 217), and the deterministic
  replay manifest. The `CollaborationRuntimeError` enum (around
  line 21, with `RuntimeDisabled`, `InvalidSession`,
  `InvalidParticipant`, `InvalidOperation`, `Conflict`,
  `InvalidSessionState` variants) is the failure surface. The 6
  in-source contract tests at the bottom of
  `crates/legion-collaboration/src/lib.rs` — the
  `default_runtime_config_is_fail_closed`,
  `duplicate_gap_and_conflict_fail_closed_without_clobbering_text`,
  `disconnect_reconnect_and_shutdown_states_are_fail_closed`,
  `delete_replace_and_undo_compensation_are_deterministic_metadata_operations`,
  `presence_and_replay_manifest_are_metadata_only`, and
  `concurrent_insert_converges_for_two_three_and_five_participants`
  tests — are the existing M0 contract surface. The
  `legion-collaboration` policy entry at
  `plans/dependency-policy.md` §1 lines 219-225 authorizes
  `legion-observability`, `legion-protocol`, `legion-security`,
  and `legion-storage`. The M0 ratification does **not** change
  the `legion-collaboration` boundary the WS-01.T6 anchor /
  operation-log layer composes through: the WS-01.T6 workstream
  adds the anchor / position-id record and the
  agent-edit-while-typing overlay to the existing operation log,
  not a new operation-log runtime, and the M0 ratification
  explicitly forbids `legion-collaboration` from declaring any
  CRDT runtime dependency (`loro`, `yrs`, `diamond-types`,
  `automerge`, etc.) at M0. The WS-16.T1 post-GA decision is the
  path that authorizes a CRDT runtime dep if the post-GA
  substrate needs it, and the same dependency-policy gate
  pattern that authorized the parser-boundary audit in
  `ADR-0033`, the LSP-boundary audit sketched in `ADR-0034`, the
  terminal-boundary sketch in `ADR-0035`, the search-boundary
  sketch in `ADR-0036`, the retrieval-boundary sketch in
  `ADR-0037`, the sandbox-boundary sketch in `ADR-0038`, and the
  agent-interop-boundary sketch in `ADR-0039` is the gate that
  authorizes the concurrent-edit boundary audit below.

- `legion-protocol` (`crates/legion-protocol/src/lib.rs`) is the
  protocol DTO layer that owns the collaboration DTO surface the
  anchor / operation-log layer composes through. The relevant
  DTOs are the `CollaborationOperationId` newtype (around line
  1382, a `u128` opaque id), the `CollaborationVersionVectorEntry`
  struct (around line 1511), the `CollaborationVersionVector`
  struct (around line 1520, carrying `entries: Vec<CollaborationVersionVectorEntry>`),
  the `CollaborationDocumentOperationKind` enum (around line 1527,
  with `Insert { text }` / `Delete` / `Replace { text }` /
  `CursorMove` / `SelectionUpdate` / `UndoCompensation` /
  `NoopAcknowledgement` / `ResyncRequest` variants),
  the `CollaborationOperationPreconditions` struct (around line
  1553, carrying `workspace_id: WorkspaceId`, `file_id: FileId`,
  `buffer_id: BufferId`, `snapshot_id: SnapshotId`,
  `buffer_version: BufferVersion`,
  `document_epoch: CollaborationDocumentEpoch`,
  `base_vector: CollaborationVersionVector`,
  `author_principal: PrincipalId`, `capability_decision:
  CapabilityDecision`, non-zero `correlation_id: CorrelationId`,
  non-nil `causality_id: CausalityId`, `redaction_hints: Vec<RedactionHint>`),
  and the `CollaborationDocumentOperation` struct (around line
  1594, carrying `session_id: CollaborationSessionId`,
  `operation_id: CollaborationOperationId`,
  `author_participant_id: CollaborationParticipantId`,
  `participant_sequence: u64`, `kind: CollaborationDocumentOperationKind`,
  `range: Option<TextRange>`, `preconditions: CollaborationOperationPreconditions`,
  `undo_group: Option<UndoGroup>`, `occurred_at: TimestampMillis`,
  `schema_version: u16`). The 124 in-source DTO contract tests
  across `crates/legion-protocol/src/lib.rs` (15 lib unittests +
  109 integration tests in `crates/legion-protocol/tests/dto_contracts.rs`)
  cover the DTO surface that the WS-01.T6 anchor / operation-log
  layer composes through, including the metadata-only invariant on
  the operation preconditions, the version-vector ordering, the
  correlation / causality id validation, the `schema_version`
  rejection, the snapshot / buffer version pair as the
  operation base, and the per-participant sequence gap detection.
  The M0 ratification does **not** change the DTO surface the
  WS-01.T6 anchor / operation-log layer composes through: the
  `legion-protocol` policy entry at `plans/dependency-policy.md` §1
  line 15 (the shared contracts boundary) is unchanged, and the
  M0 ratification does **not** add a new DTO for the
  `StablePositionId` / `PositionAnchor` family — the
  `TextEdit` / `TextSnapshot` / `BufferVersion` / `SnapshotId`
  family is the M0 boundary, and the WS-01.T6 workstream is the
  one that declares any new `StablePositionId` /
  `PositionAnchor` / `OperationLog` DTOs. The M0 ratification
  explicitly forbids `legion-protocol` from declaring any CRDT
  runtime dependency (`loro`, `yrs`, `diamond-types`, `automerge`,
  etc.) at M0; the WS-16.T1 post-GA decision is the path that
  authorizes a CRDT runtime dep if the post-GA substrate needs it.

- `legion-editor` (`crates/legion-editor/src/lib.rs`, 3379 lines)
  is the editor substrate that consumes the WS-01.T6 anchor /
  operation-log layer through its existing `legion-text` edge.
  The `legion-editor` policy entry at `plans/dependency-policy.md` §1
  lines 43-52 authorizes `legion-observability`, `legion-protocol`,
  and `legion-text` (and the MUST rules at lines 48-51 require
  `legion-protocol` and `legion-text` directly, and the MUST NOT
  rule at line 52 forbids the `legion-editor` ↔ `legion-project`
  edge). The M0 ratification does **not** change `legion-editor`'s
  allowed edges; the WS-01.T6 anchor / operation-log layer
  composes through the existing `legion-editor` ↔ `legion-text`
  edge plus the existing `legion-editor` ↔ `legion-protocol`
  edge, and the `legion-editor` MUST NOT `legion-collaboration`
  rule is unchanged. The M0 ratification explicitly forbids
  `legion-editor` from declaring any CRDT runtime dependency
  (`loro`, `yrs`, `diamond-types`, `automerge`, etc.) at M0, and
  the `xtask` policy audit enforces the boundary. The 36
  in-source contract tests across `crates/legion-editor/src/lib.rs`
  (21 lib unittests + 7 workspace vfs integration tests + 1
  additional integration test + 7 perf-harness tests with 3
  long-running perf workloads ignored) cover the editor
  boundary the WS-01.T6 workstream extends with anchor-aware
  transaction ordering.

- `legion-app` (`crates/legion-app/src/lib.rs`) is the GUI
  composition crate that composes the WS-01.T6 anchor /
  operation-log layer through its existing `legion-editor` /
  `legion-text` / `legion-collaboration` / `legion-protocol` /
  `legion-agent` / `legion-ai` / `legion-ai-providers` /
  `legion-security` composition edges. The `legion-app` policy
  entry at `plans/dependency-policy.md` §1 lines 86-105 authorizes
  the full app composition set (including `legion-collaboration`,
  `legion-editor`, `legion-text` through `legion-editor`,
  `legion-protocol`, `legion-agent`, `legion-ai`,
  `legion-ai-providers`, `legion-security`, `legion-platform`,
  `legion-observability`, etc.). The M0 ratification does
  **not** change `legion-app`'s allowed edges; the WS-01.T6
  anchor / operation-log layer composes through the existing
  `legion-app` ↔ `legion-editor` ↔ `legion-text` edge chain
  and the existing `legion-app` ↔ `legion-collaboration` edge
  (already authorized by the §1 line 90 entry), and the
  WS-12 / WS-13 workstreams that consume the anchor layer
  (agent-edit-while-typing, diff overlay anchoring, inline-edit
  selection) compose through the same `legion-app` composition
  path. The M0 ratification does **not** authorize a new
  `legion-app` ↔ CRDT-crate edge; the WS-16.T1 post-GA
  decision is the path that authorizes a CRDT runtime dep
  if the post-GA substrate needs it.

- `legion-ui` (`crates/legion-ui/`) is the projection-only UI
  shell. The `legion-ui` policy entry at
  `plans/dependency-policy.md` §1 lines 54-75 already forbids
  every renderer / editor / project / storage / app / agent /
  terminal / security / observability / platform edge and only
  allows `legion-protocol`. The M0 ratification does **not**
  extend `legion-ui`'s allowed edges: the anchor / operation-log
  projection surface is a new
  `AnchorProjection` / `OperationLogProjection` family on top of
  the existing `legion-ui` ↔ `legion-protocol` edge, emitted by
  `legion-app` and rendered by `legion-desktop`. `legion-ui`
  never owns anchor state, never owns operation-log state, and
  never owns mutation authority. The boundary sketch in this
  ADR reinforces this rule with a future
  `CONCURRENT_EDIT_BOUNDARY_POLICY_MARKERS` audit (no
  `legion-ui` may declare any `loro` / `yrs` /
  `diamond-types` / `automerge` runtime dependency), shaped
  like the existing `PARSER_BOUNDARY_POLICY_MARKERS` audit in
  `xtask/src/main.rs` (the `PARSER_BOUNDARY_POLICY_MARKERS`
  constant at line 446) and the future
  `SEARCH_BOUNDARY_POLICY_MARKERS` /
  `RETRIEVAL_BOUNDARY_POLICY_MARKERS` /
  `LSP_BOUNDARY_POLICY_MARKERS` /
  `TERMINAL_BOUNDARY_POLICY_MARKERS` /
  `SANDBOX_BOUNDARY_POLICY_MARKERS` /
  `AGENT_INTEROP_BOUNDARY_POLICY_MARKERS` sketches in
  ADR-0034 / 0035 / 0036 / 0037 / 0038 / 0039. The M0
  ratification does not require the concurrent-edit-boundary
  audit to land today; the audit is a phase-gate improvement
  that becomes useful the moment a workspace package actually
  declares one of the forbidden CRDT crates. Today, no
  package declares any of them, so the audit is a
  forward-compatibility gate, not a regression guard.

- `legion-editor` is the editor substrate. The M0 ratification
  does **not** extend `legion-editor`'s allowed edges (forbids
  any `loro` / `yrs` / `diamond-types` / `automerge` runtime
  dependency) and the `legion-editor` policy entry at
  `plans/dependency-policy.md` §1 lines 43-52 (the
  `MUST NOT depend on legion-project` rule plus the editor
  permission set) remains intact.

- `legion-protocol` is the protocol DTO layer. The M0
  ratification does **not** extend `legion-protocol`'s allowed
  edges and does **not** declare any CRDT runtime dep at M0.
  The `xtask` policy audit enforces the boundary.

- `legion-text` is the text substrate. The M0 ratification does
  **not** extend `legion-text`'s allowed edges (forbids any
  `loro` / `yrs` / `diamond-types` / `automerge` runtime
  dependency) and the `legion-text` policy entry at
  `plans/dependency-policy.md` §1 line 23-24 (allowing only
  `legion-protocol`) remains intact.

- `legion-collaboration` is the deterministic in-memory
  collaboration runtime. The M0 ratification does **not**
  extend `legion-collaboration`'s allowed edges (forbids any
  `loro` / `yrs` / `diamond-types` / `automerge` runtime
  dependency) and the `legion-collaboration` policy entry at
  `plans/dependency-policy.md` §1 lines 219-225 (allowing only
  `legion-observability`, `legion-protocol`, `legion-security`,
  `legion-storage`) remains intact.

The §2.2 invariants constrain the concurrent-edit layer:

- **App-composed and capability-gated** — the anchor /
  operation-log layer is composed through the existing
  `legion-app` ↔ `legion-editor` ↔ `legion-text` edge chain
  and the existing `legion-app` ↔ `legion-collaboration` edge.
  Anchor / operation-log application goes through the existing
  `legion-security` capability broker the same way every other
  editor mutation does, and the future agent-edit-while-typing
  surface (WS-12.T1 / WS-13.T4) composes the
  `CollaborationDocumentOperation` envelope through the same
  `delegated.allocate_sandbox` / `delegated.runtime.allocate`
  capability reservations the OS-sandbox tier in ADR-0038
  already wraps. The M0 ratification does **not** add a new
  capability name; the existing `terminal.launch` /
  `terminal.close` / `terminal.kill` /
  `delegated.runtime.allocate` / `sandbox.os.activate` /
  `sandbox.os.network.activate` / `sandbox.os.fs.activate`
  reservations (the Phase 8 production reservation set at
  `plans/dependency-policy.md` §1 line 247 plus the OS-sandbox
  extensions from ADR-0038) are the policy surface. External
  agents cannot bypass the broker; their proposed edits
  attach to anchors through the same proposal envelope so the
  proposal service, the capability broker, and the evidence
  ledger all see the external agent's edits identically to
  the native agent's edits.

- **Proposal-mediated mutation** — anchor / operation-log
  application applies edits to buffers or disk only through
  the accepted Phase 2 proposal routes (`ADR-0016`) and the
  AI-plane proposal flow. The anchor / operation-log layer
  returns the resolved `TextEdit` (range + replacement) and
  the proposal service previews, approves, applies, rejects,
  cancels, or rolls back; the anchor layer never applies
  edits to buffers or disk on its own. The agent-edit-while-
  typing overlay (WS-12) is the user-facing gate that shows
  the resolved anchor position and the proposed edit before
  any apply, and the conflict UX (WS-07.T2) is the
  failure-mode surface that surfaces three-way views and
  retry-with-rebase for text edits when the anchor's base
  snapshot / buffer version pair is stale. The
  `CollaborationDocumentOperation` envelope already carries
  the operation preconditions (the `base_vector`,
  `buffer_version`, `snapshot_id`, `buffer_id`, `file_id`,
  `workspace_id`, `document_epoch` fields) that the proposal
  service uses to detect stale / duplicate / gap / conflict
  before any apply, and the `CollaborationRuntimeError` enum's
  `Conflict` / `Stale` / `GapDetected` variants (asserted by
  the `duplicate_gap_and_conflict_fail_closed_without_clobbering_text`
  and `concurrent_insert_converges_for_two_three_and_five_participants`
  contract tests) are the failure surface.

- **Projection-only UI boundary** — `legion-ui` consumes
  anchor / operation-log projections (the future
  `AnchorProjection` / `OperationLogProjection` family that
  WS-01.T6 adds on top of the existing `legion-ui` ↔
  `legion-protocol` edge) and emits `CommandDispatchIntent`
  only. UI never owns anchor state, never owns operation-log
  state, and never owns mutation authority. The
  `legion-ui` policy entry at `plans/dependency-policy.md` §1
  lines 54-75 already forbids every renderer / editor /
  project / storage / app / agent / terminal / security /
  observability / platform edge, and the structural audit
  enforces it. The boundary sketch in this ADR reinforces
  this rule with a future
  `CONCURRENT_EDIT_BOUNDARY_POLICY_MARKERS` audit (no
  `legion-ui` may declare any `loro` / `yrs` /
  `diamond-types` / `automerge` runtime dependency), shaped
  like the existing `PARSER_BOUNDARY_POLICY_MARKERS` audit
  in `xtask/src/main.rs` and the
  `SEARCH_BOUNDARY_POLICY_MARKERS` /
  `RETRIEVAL_BOUNDARY_POLICY_MARKERS` /
  `LSP_BOUNDARY_POLICY_MARKERS` /
  `TERMINAL_BOUNDARY_POLICY_MARKERS` /
  `SANDBOX_BOUNDARY_POLICY_MARKERS` /
  `AGENT_INTEROP_BOUNDARY_POLICY_MARKERS` sketches in
  ADR-0034 / 0035 / 0036 / 0037 / 0038 / 0039.

- **Metadata-first observability** — every anchor / operation-
  log emission (the anchor attach, the operation submit, the
  version-vector gap detection, the duplicate detection, the
  stale detection, the conflict detection, the resync
  request, the replay manifest) emits a metadata-only
  observability record: anchor id, base snapshot id, base
  buffer version, session id, operation id, author
  participant id, kind, range start, range end,
  `CorrelationId` / `CausalityId` / `EventSequence`,
  capability decision, sandbox process id (when in
  Delegate/Workflow mode), exit reason, freshness. Raw
  proposed text is limited to the user's own UI session,
  raw argv and raw network payloads are never emitted, and
  the observability sinks that reject zero IDs apply to
  anchor / operation-log records the same way they apply to
  terminal / AI / tracker / retrieval / sandbox / agent-
  interop records. The
  `presence_and_replay_manifest_are_metadata_only` contract
  test in `crates/legion-collaboration/src/lib.rs` is the
  existing M0 contract surface that asserts the
  metadata-only invariant on the operation-log replay
  manifest.

- **Fail-closed policy** — the broker denies an anchor
  application that lacks a matching capability; the broker
  denies an operation submit that lacks a valid
  `CollaborationOperationPreconditions` (zero
  `correlation_id`, nil `causality_id`, missing snapshot
  id, missing buffer version, zero schema version,
  mismatched session / workspace / file / buffer id);
  the broker denies a stale operation whose base
  snapshot / buffer version pair is older than the
  current buffer head; the broker denies a duplicate
  operation whose `operation_id` is already in the
  `operation_ids` set; the broker denies a causal-gap
  operation and surfaces a `CollaborationCausalGap` to
  the user; the broker denies a `ResyncRequest` whose
  `observed_vector` cannot be satisfied; the
  `CollaborationRuntimeError` enum's `RuntimeDisabled`
  variant is the kill switch that the runtime config
  exposes for tests and the post-GA collaboration
  activation gate. The
  `duplicate_gap_and_conflict_fail_closed_without_clobbering_text`
  and `disconnect_reconnect_and_shutdown_states_are_fail_closed`
  contract tests in
  `crates/legion-collaboration/src/lib.rs` are the
  existing M0 contract surface that asserts the
  fail-closed invariant.

The plan compared three options: (a) **operation / anchor
layer now (stable position IDs + version vectors over
`legion-text`), full CRDT (Loro / yrs / homegrown SumTree-
style) deferred**, (b) **CRDT core now**, and (c) **anchor
layer with a non-CRDT operation log but a custom homegrown
SumTree-style data structure from day one**. Option (a) matches
how the 2026 editor market converged: Zed ships stable
position IDs in the rope layer and adds a CRDT only when
real-time collaboration turns on, Helix ships slice-stable
positions in the rope layer and does not ship a CRDT at
all, and Lapce ships a custom anchor table and does not
ship a CRDT until the collaboration track turns on.
Option (b) would bring Loro / yrs / diamond-types into the
workspace dependency graph at M0, with the WS-02 / WS-03 /
WS-12 workstreams all paying the CRDT cost for an
orchestration use case (agent-edit-while-typing, diff
overlays, diagnostics) that doesn't need it, and would
ship a CRDT before the agent harness, the diff overlay
substrate, and the inline-edit substrate are real and
exercised. Option (c) would burn the M3 / M4 budget on
homegrown data-structure work that the editor market
already validated as deferrable, and would couple the
post-GA CRDT decision to a from-scratch implementation
that the market has already iterated on (Loro,
diamond-types, yrs).

## Decision

Legion will implement stable position anchors and an
operation log over the existing `legion-text` snapshot
substrate **before** the post-GA CRDT decision, and the
post-GA CRDT decision (Loro vs yrs vs homegrown SumTree-
style) is explicitly deferred to WS-16.T1. The anchor
layer must keep diagnostics, decorations, diff overlays,
proposal hunks, agent-edit-while-typing overlays, and
inline-edit selections attached to the buffer across
ordinary edit sequences, and the operation log must
serialize concurrent edits to a deterministic order that
the conflict UX (WS-07.T2) can present as a three-way view
or a retry-with-rebase affordance. The anchor layer
composes through the existing `legion-text` / `legion-editor`
edge chain; the operation log composes through the existing
`legion-collaboration` deterministic in-memory runtime; the
DTOs that cross the protocol boundary reuse the existing
`CollaborationDocumentOperation` /
`CollaborationVersionVector` /
`CollaborationOperationId` family; the
`StablePositionId` / `PositionAnchor` / `OperationLog`
family is the new M3 / M0-substrate DTO set the WS-01.T6
workstream adds. The M0 ratification does **not** ship the
anchor layer; the M0 ratification ratifies the boundary,
the choice, the substrate, and the deferral.

- **Stable position anchors (WS-01.T6).** A `StablePositionId`
  is a content-derived anchor: the (line index, byte
  offset, content hash of the next chunk, predecessor
  `BufferVersion`) tuple that survives ordinary edits to
  the buffer. Anchors survive arbitrary edit sequences
  that do not delete the anchored line and are recovered
  (with a rebase / snap-to-line fallback) on edits that
  delete the anchored line. The anchor lives in
  `legion-text` (the policy entry at
  `plans/dependency-policy.md` §1 line 23-24 allows only
  `legion-protocol` and the new anchor is a position-id
  over the existing rope layer, so the anchor type
  composes through the existing `legion-text` ↔
  `legion-protocol` edge chain), and the M0 ratification
  ratifies the substrate choice; the WS-01.T6 workstream
  is the one that implements the anchor type, the
  property tests, and the agent-edit-while-typing
  integration test.

- **Operation log (WS-01.T6 + WS-12.T1).** A
  `CollaborationSessionRuntime` instance per buffer
  holds the in-memory operation log, the version
  vector, the duplicate-detection set, the gap-
  detection queue, the acknowledgement queue, and the
  presence projections. The operation log is the
  deterministic serialization substrate the agent
  harness and the inline-edit selection attach to,
  and the WS-12.T1 / WS-13.T4 workstreams that consume
  the operation log compose through the existing
  `legion-app` ↔ `legion-collaboration` edge (the
  `legion-collaboration` policy entry at
  `plans/dependency-policy.md` §1 line 90 already
  authorizes the edge). The M0 ratification ratifies
  the boundary; the WS-01.T6 workstream is the one
  that adds the agent-edit-while-typing integration
  test, the conflict-UX property test, and the
  inline-edit-while-typing integration test.

- **Crate boundary.** The concurrent-edit layer is
  split across `legion-text`, `legion-protocol`,
  `legion-collaboration`, `legion-editor`, and
  `legion-app` along the accepted policy entries in
  `plans/dependency-policy.md` §1. `legion-text`
  owns the rope layer, the `TextEdit` /
  `TextSnapshot` / `BufferVersion` / `SnapshotId`
  substrate, the new `StablePositionId` /
  `PositionAnchor` types, and the position
  round-trip helpers. `legion-text` may **not** take
  a CRDT runtime dependency (`loro`, `yrs`,
  `diamond-types`, `automerge`, etc.) at M0, and
  the `xtask` policy audit enforces the boundary.
  `legion-protocol` owns the
  `CollaborationDocumentOperation` /
  `CollaborationVersionVector` /
  `CollaborationOperationId` /
  `CollaborationOperationPreconditions` DTOs and
  the new `StablePositionId` / `PositionAnchor` /
  `OperationLog` DTO family the WS-01.T6 workstream
  adds. `legion-protocol` may **not** take a CRDT
  runtime dependency at M0. `legion-collaboration`
  owns the deterministic in-memory operation log
  runtime, the version-vector gap detection, the
  duplicate detection, the conflict detection, the
  acknowledgement queue, the presence projections,
  and the resync request flow. `legion-collaboration`
  may **not** take a CRDT runtime dependency at M0
  (the post-GA CRDT decision is the path that
  authorizes one). `legion-editor` consumes
  anchors and operation records through its
  existing `legion-text` edge; `legion-editor` may
  **not** declare a CRDT runtime dependency at M0
  and may **not** depend on `legion-collaboration`
  (the `legion-editor` policy entry at
  `plans/dependency-policy.md` §1 lines 43-52 only
  authorizes `legion-observability`, `legion-protocol`,
  and `legion-text`). `legion-app` composes the
  anchor / operation-log layer through the existing
  `legion-app` ↔ `legion-editor` ↔ `legion-text`
  edge chain and the existing `legion-app` ↔
  `legion-collaboration` edge. `legion-ui` consumes
  anchor / operation-log projections and emits
  `CommandDispatchIntent` only. `legion-desktop` is
  the renderer adapter and is unchanged. This
  boundary mirrors the parser-boundary audit in
  `ADR-0033`, the LSP-boundary sketch in `ADR-0034`,
  the terminal-boundary sketch in `ADR-0035`, the
  search-boundary sketch in `ADR-0036`, the
  retrieval-boundary sketch in `ADR-0037`, the
  sandbox-boundary sketch in `ADR-0038`, and the
  agent-interop-boundary sketch in `ADR-0039`, and
  is enforced by the same `cargo run -p xtask --
  check-deps` policy-text + package-dependency audit.

- **Post-GA CRDT decision (WS-16.T1).** The full CRDT
  decision (Loro vs yrs vs diamond-types vs
  homegrown SumTree-style) is explicitly deferred to
  WS-16.T1, the WS-16 post-GA track workstream that
  the master plan §6 row 240 records. The WS-16.T1
  workstream is the one that decides the CRDT
  substrate, ships the CRDT over the anchor layer,
  amends the `legion-text` / `legion-protocol` /
  `legion-collaboration` / `legion-editor` /
  `legion-app` policy entries to authorize the
  CRDT runtime dep, and adds the
  `CONCURRENT_EDIT_BOUNDARY_POLICY_MARKERS` audit to
  `xtask/src/main.rs`. The M0 ratification does
  **not** pre-decide the CRDT substrate and does
  **not** declare a CRDT runtime dep at M0; the
  M0 ratification ratifies the deferral, the
  substrate, and the audit sketch below.

- **Compatibility with `ADR-0018` (lsp runtime
  supervision) and `ADR-0034` (lsp client
  architecture).** The anchor / operation-log layer
  is independent of the LSP client substrate. The
  LSP `didChange` notification already maps to the
  existing `TextEdit` / `TextSnapshot` /
  `BufferVersion` / `SnapshotId` substrate, and the
  LSP `textDocument/formatting` /
  `textDocument/rename` /
  `workspace/executeCommand` responses already
  apply through the accepted Phase 2 proposal
  routes (`ADR-0016`). The anchor layer adds
  stable attachment for the LSP diagnostic
  range / completion range / hover range / goto-
  def target / signature-help range / references-
  range projections, but the M0 boundary is the
  existing `legion-text` / `legion-protocol` /
  `legion-editor` edge chain and the M0
  ratification does **not** require an LSP
  re-architecture. The WS-03.T2 / T3 workstreams
  are the path that composes the anchor layer
  into the LSP diagnostic / completion / hover
  / goto-def / signature-help / references
  projections.

- **Compatibility with `ADR-0020` (collaboration
  operation model) and `ADR-0021` (collaboration
  identity / permissions / retention).** The anchor
  / operation-log layer composes through the
  existing `legion-collaboration` runtime the
  Phase 6 evidence ratified (the
  `CollaborationSessionRuntime` +
  `CollaborationDocumentOperation` +
  `CollaborationVersionVector` +
  `CollaborationOperationId` family), and the
  collaboration identity / permissions / retention
  surface (`ADR-0021`) is the policy surface the
  WS-01.T6 anchor / operation-log layer inherits.
  The M0 ratification does **not** re-architect the
  Phase 6 collaboration runtime; the WS-01.T6
  workstream is the one that adds the
  `StablePositionId` / `PositionAnchor` /
  `OperationLog` DTO family and the anchor-aware
  transaction ordering, and the WS-16.T1 post-GA
  decision is the one that adds the CRDT over
  the existing Phase 6 substrate.

- **Compatibility with the air-gap story.** The
  anchor / operation-log layer is local-only at
  M0 (no network sync, no remote transport, no
  hosted provider call). The M0 ratification does
  **not** enable the post-GA collaboration track
  (`ADR-0020` / `ADR-0021` / `ADR-0025`), the
  remote-development track (`ADR-0022` /
  `ADR-0023` / `ADR-0024`), or the production
  remote network transport (`ADR-0025`). The
  in-memory operation log is local-only and
  deterministic, and the WS-16.T1 post-GA decision
  is the one that decides how the post-GA CRDT
  substrate composes with the post-GA remote /
  collaboration tracks.

- **Crate boundary audit (future gate).** A
  future
  `CONCURRENT_EDIT_BOUNDARY_POLICY_MARKERS` /
  `CONCURRENT_EDIT_ANCHOR_TRAIT_PACKAGES = ["legion-text"]` /
  `CONCURRENT_EDIT_OPERATION_LOG_PACKAGES = ["legion-collaboration"]` /
  `CONCURRENT_EDIT_DTO_PACKAGES = ["legion-protocol"]` /
  `FORBIDDEN_CONCURRENT_EDIT_DEPS = ["loro", "yrs", "diamond-types", "automerge"]`
  audit is sketched in this decision for the
  next phase gate, shaped like the existing
  `PARSER_BOUNDARY_POLICY_MARKERS` /
  `PARSER_DEPENDENCY_ALLOWED_PACKAGES = ["legion-index"]` /
  `FORBIDDEN_PARSER_DEPS = ["tree-sitter", "tree-sitter-rust"]`
  audit in `xtask/src/main.rs` (the constants are
  at lines 446-453 in the current tree) and the
  future `SEARCH_BOUNDARY_POLICY_MARKERS` /
  `RETRIEVAL_BOUNDARY_POLICY_MARKERS` /
  `LSP_BOUNDARY_POLICY_MARKERS` /
  `TERMINAL_BOUNDARY_POLICY_MARKERS` /
  `SANDBOX_BOUNDARY_POLICY_MARKERS` /
  `AGENT_INTEROP_BOUNDARY_POLICY_MARKERS`
  sketches in ADR-0034 / 0035 / 0036 / 0037 / 0038
  / 0039. The M0 ratification does not require
  the concurrent-edit-boundary audit to land
  today; the audit is a phase-gate improvement
  that becomes useful the moment a workspace
  package actually declares one of the forbidden
  CRDT crates. Today, no package declares any
  of them, so the audit is a forward-compatibility
  gate, not a regression guard.

## Consequences

- **Positive:** the M0 ratification ratifies a
  working substrate (the `legion-text` rope layer
  with `TextEdit` / `TextSnapshot` /
  `BufferVersion` / `SnapshotId`) plus a working
  in-memory operation log runtime
  (`legion-collaboration`'s
  `CollaborationSessionRuntime` with the
  duplicate / gap / stale / conflict / resync
  fail-closed surface) plus a real DTO surface
  (`legion-protocol`'s
  `CollaborationDocumentOperation` /
  `CollaborationVersionVector` /
  `CollaborationOperationId` family) the WS-01.T6
  anchor / operation-log layer composes through.
  The agent-edit-while-typing overlay (WS-12),
  the diff overlay anchoring (WS-11.T2), the
  inline-edit selection (WS-11.T2), the conflict
  UX (WS-07.T2), and the post-GA collaboration
  track (WS-16) have a real starting point in
  `crates/legion-text/src/lib.rs` (2019 lines, 24
  contract tests), `crates/legion-collaboration/src/lib.rs`
  (1232 lines, 6 contract tests), and
  `crates/legion-protocol/src/lib.rs` (the
  collaboration DTO family plus 124 DTO contract
  tests). The `legion-app` composition edges to
  `legion-editor` / `legion-text` /
  `legion-collaboration` are already policy-
  allowed; the existing `legion-app` AI projection
  surface (the AI-plane proposal flow) and the
  existing `legion-app` agent surface (the
  Phase 4 metadata-only agent state machine) are
  the starting point for the anchor-aware
  proposal composition that WS-12.T1 will add.

- **Positive:** the M0 ratification ratifies the
  boundary that prevents the silent-drift problem
  (anchors survive ordinary edits, anchor
  deletion falls back to a snap-to-line or
  rebase affordance, and the operation log
  detects duplicates / gaps / conflicts /
  stale-state conditions before any apply). The
  WS-01.T6 property test (anchors survive
  arbitrary edit sequences) and the WS-12.T1
  agent-edit-while-typing integration test are
  the future gates that catch anchor / operation-
  log regressions.

- **Positive:** air-gap mode is preserved. The
  anchor / operation-log layer is local-only and
  never reaches the network. The post-GA CRDT
  decision (WS-16.T1) is the path that decides
  how the CRDT substrate composes with the
  post-GA remote / collaboration tracks, and the
  post-GA remote / collaboration tracks are
  themselves gated by `ADR-0025` (production
  remote network transport) and the WS-16
  post-GA track.

- **Positive:** the WS-01.T6 workstream (anchor /
  operation layer), the WS-12.T1 workstream
  (agent-edit-while-typing), the WS-12.T6
  workstream (cancellation reap), the WS-13.T4
  workstream (ACP host), the WS-07.T2 workstream
  (conflict UX), the WS-11.T2 workstream (inline
  edit), and the WS-16.T1 workstream (post-GA
  CRDT decision) have a real substrate and a real
  set of downstream workstreams. The M0
  ratification ratifies the substrate, the
  boundary, the choice, and the deferral.

- **Negative:** introducing Loro / yrs /
  diamond-types / automerge / SumTree-style is
  a WS-16.T1 (post-GA) decision, not an M0
  prerequisite. The M0 ratification ratifies
  the boundary, the choice, the substrate, and
  the deferral; the WS-16.T1 workstream is the
  one that decides the CRDT substrate, declares
  the CRDT runtime dep, amends the
  `legion-text` / `legion-protocol` /
  `legion-collaboration` / `legion-editor` /
  `legion-app` policy entries, and ships the
  post-GA CRDT over the existing M0 anchor
  layer.

- **Negative:** anchor deletion is a UX
  problem. The M0 ratification ratifies the
  substrate (anchors survive ordinary edits)
  but does not ratify the snap-to-line or
  rebase affordance for the rare case where
  the anchored content is deleted. The
  WS-01.T6 workstream is the one that ships
  the affordance, and the conflict UX
  (WS-07.T2) is the user-facing surface that
  surfaces the three-way view and the
  retry-with-rebase affordance.

- **Mitigation:** the `xtask` policy audit that
  already runs as part of `cargo run -p xtask
  -- check-deps` is the M0 test surface. The
  future `CONCURRENT_EDIT_BOUNDARY_POLICY_MARKERS`
  audit is the structural guard that prevents
  any workspace package from declaring a CRDT
  runtime dep at M0, and the WS-16.T1 decision
  is the gate that authorizes a CRDT runtime
  dep if the post-GA substrate needs it. The
  WS-01.T6 acceptance criteria (anchors
  survive arbitrary edit sequences; the
  agent-edit-while-typing integration test;
  the conflict-UX property test; the
  inline-edit-while-typing integration test)
  are the WS-01.T6 / WS-12.T1 / WS-07.T2
  acceptance shape.

## Verification

- `cargo run -p xtask -- check-deps` (dependency
  direction + structural audit, with the
  `legion-text`, `legion-protocol`,
  `legion-collaboration`, `legion-editor`,
  `legion-app`, `legion-cli`, `legion-desktop`,
  `legion-ui`, and `legion-ai` /
  `legion-ai-providers` / `legion-agent` policy
  entries verified against
  `plans/dependency-policy.md` §1 and the
  concurrent-edit-boundary sketch above)
- `cargo run -p xtask -- docs-hygiene` (broken
  relative Markdown links and the unallowlisted
  stale Legion-rename marker)
- `cargo run -p xtask -- no-egui-textedit`
  (companion gate, unchanged from `ADR-0032`;
  the anchor / operation-log / conflict-UX
  panel renders projected anchor and
  operation-log results, not an
  `egui::TextEdit`)
- `cargo fmt --all --check`
- `cargo test -p legion-text --tests` (the
  rope layer over which the WS-01.T6 anchor
  layer composes, 24 contract tests across
  the lib unittests covering the
  `TextPosition` / `TextRange` / `TextEdit` /
  `TextSnapshot` / `TextSnapshotDescriptor` /
  `TextBuffer` surface, the rope chunk
  hashing, the UTF-8 / UTF-16 round-trips,
  the chunk-rebuild-on-edit invariant, the
  large-file degraded cache-free mode, the
  `LineIndex` line-ending width, the
  `apply_edit` / `try_apply_edit` /
  `set_version` / `byte_offset` /
  `utf16_position` /
  `byte_offset_from_utf16` round-trips, and
  the LSP `crlf_is_single_line_ending_for_lsp`
  conformance test)
- `cargo test -p legion-protocol --tests` (the
  protocol DTO layer that owns the
  collaboration DTO surface, 124 contract
  tests across 15 lib unittests + 109
  integration tests in
  `crates/legion-protocol/tests/dto_contracts.rs`,
  covering the
  `CollaborationOperationId` /
  `CollaborationVersionVector` /
  `CollaborationVersionVectorEntry` /
  `CollaborationDocumentOperationKind` /
  `CollaborationOperationPreconditions` /
  `CollaborationDocumentOperation` /
  `CollaborationAcknowledgement` /
  `CollaborationAcknowledgementStatus` /
  `CollaborationCausalGap` /
  `CollaborationSessionDescriptor` /
  `CollaborationSessionState` /
  `CollaborationSessionId` /
  `CollaborationTransportEnvelope` /
  `CollaborationTransportPayload` /
  `CollaborationReplayManifest` /
  `CollaborationDocumentBinding` /
  `CollaborationParticipant` /
  `CollaborationParticipantId` /
  `CollaborationPermission` /
  `CollaborationPresenceProjection` /
  `CollaborationDocumentEpoch` /
  `RedactionHint` /
  `RetentionLabel` DTOs that the WS-01.T6
  anchor / operation-log layer composes
  through)
- `cargo test -p legion-collaboration --tests`
  (the deterministic, metadata-first
  collaboration operation log + replay
  runtime, 6 contract tests across the lib
  unittests covering the
  `default_runtime_config_is_fail_closed`,
  `duplicate_gap_and_conflict_fail_closed_without_clobbering_text`,
  `disconnect_reconnect_and_shutdown_states_are_fail_closed`,
  `delete_replace_and_undo_compensation_are_deterministic_metadata_operations`,
  `presence_and_replay_manifest_are_metadata_only`,
  and
  `concurrent_insert_converges_for_two_three_and_five_participants`
  tests that the WS-01.T6 anchor /
  operation-log layer extends)
- `cargo test -p legion-editor --tests` (the
  editor substrate that consumes the
  WS-01.T6 anchor / operation-log layer
  through its existing `legion-text` edge,
  36 contract tests across 21 lib unittests
  + 7 workspace vfs integration tests + 1
  additional integration test + 7
  perf-harness tests with 3 long-running
  perf workloads ignored, covering the
  editor boundary the WS-01.T6 workstream
  extends with anchor-aware transaction
  ordering)
- WS-01 evidence under
  `plans/evidence/production/m3/` once the
  WS-01.T6 anchor / operation layer lands
  with dependency-policy updates and
  contract tests; WS-12 evidence under
  `plans/evidence/production/m3/` once the
  agent-edit-while-typing overlay (WS-12.T1)
  lands with the proposal / capability
  broker / evidence ledger chain; WS-16
  evidence under
  `plans/evidence/production/m6/` once the
  post-GA CRDT decision (WS-16.T1) lands
  with dependency-policy updates and
  contract tests. M0 ratification does not
  require any of these WS-01 / WS-12 / WS-16
  evidence packages today; the M0 evidence
  package for this ratification is
  `plans/evidence/production/M0/ADR-0040-ratification.md`.
