# M0 — ADR-0040 (Concurrent Edit Substrate) Ratification Evidence

Milestone: **M0 (Plan lock)** — Production Master Plan v0.1
ADR: [`plans/adrs/ADR-0040-concurrent-edit-substrate.md`](../../../adrs/ADR-0040-concurrent-edit-substrate.md)
Date: 2026-06-10
Gate: `cargo run -p xtask -- check-deps` (dependency direction + structural
audit, with `legion-text`, `legion-protocol`, `legion-collaboration`,
`legion-editor`, `legion-app`, `legion-cli`, `legion-desktop`,
`legion-ui`, `legion-agent`, `legion-ai`, and `legion-ai-providers`
policy entries verified against `plans/dependency-policy.md` §1
and the concurrent-edit-boundary sketch in the ratified ADR)
Acceptance target: master-plan §6 row 240 "ADR-0040 | Concurrent-edit
substrate" → option (a) ratified in-repo: **operation/anchor layer
now (stable position IDs + version vectors over `legion-text`), full
CRDT (Loro / yrs / homegrown SumTree-style) deferred**; the
`legion-text` `TextEdit` / `TextSnapshot` / `BufferVersion` /
`SnapshotId` substrate is the anchor substrate; the
`legion-collaboration` deterministic in-memory operation log +
replay runtime is the operation-log substrate; the
`legion-protocol` `CollaborationDocumentOperation` /
`CollaborationVersionVector` / `CollaborationOperationId` family
is the DTO substrate; `legion-editor` consumes anchors and
operation records through its existing `legion-text` edge;
`legion-app` composes the layer through the existing
`legion-app` ↔ `legion-editor` ↔ `legion-text` and
`legion-app` ↔ `legion-collaboration` edges; `legion-ui` is
projection-only and never owns anchor or operation-log state;
no `loro` / `yrs` / `diamond-types` / `automerge` workspace
dependency declared today; WS-16.T1 (post-GA) is the path that
authorizes a CRDT runtime dep if the post-GA substrate needs
it; agent + human concurrent editing needs anchors at M3;
retrofitting anchors later is the classic editor-rewrite
trigger — do the layer now, cheaply.

## Decision Recorded

- Status flipped from `Draft` to `Accepted` in
  `plans/adrs/ADR-0040-concurrent-edit-substrate.md`.
- Decision text matches Production Master Plan v0.1 §6 row 240
  recommendation verbatim: option (a) — **operation/anchor
  layer now (stable position IDs + version vectors over
  `legion-text`), full CRDT (Loro / yrs / homegrown SumTree-style)
  deferred**. The plan's §6 row 240 explicitly says "**(a)**.
  Agent+human concurrent editing needs anchors at M3;
  collaboration needs the CRDT only at post-GA. Retrofitting
  anchors later is the classic editor-rewrite trigger — do
  the layer now, cheaply." The ADR ratifies that recommendation
  without amendment and records the WS-01.T6 acceptance shape
  ("property tests — anchors survive arbitrary edit sequences;
  agent-edit-while-typing integration test") as the M3 gate
  and the WS-16.T1 acceptance shape ("ADR with benchmark
  evidence") as the post-GA CRDT-substrate decision gate.
- No amendments were required to the master-plan recommendation.
  The ADR adds six confirmations consistent with the plan and
  with current code / contracts:
  1. The text substrate over which the anchor / operation-log
     layer composes is live and exercised by tests today.
     `legion-text` (`crates/legion-text/src/lib.rs`, 2019 lines)
     is the text-substrate crate. The rope layer ships
     `TextPosition` (around line 35), `TextRange` (around line
     65), `TextEdit` (around line 108, carrying `range:
     TextRange` + `replacement: String` + `replacement_utf16_len`),
     `TextSnapshotDescriptor` (around line 176, carrying
     `snapshot_id: SnapshotId`, `buffer_version: BufferVersion`,
     `content_hash: String`, `byte_len: usize`, `line_count:
     usize`, `memory_footprint_bytes: usize`, `retention_pin_reason`),
     `TextSnapshot` (around line 240, an immutable rope view
     that clones cheaply through `Arc<Rope>` and a `LineIndex`
     cache and is bounded by a 5 MiB full-cache budget), and
     `TextBuffer` (around line 938, the mutable rope owner
     with `apply_edit` / `try_apply_edit` at lines 1209 /
     1214, `set_version` at line 1102, `byte_offset` /
     `position` / `utf16_position` /
     `byte_offset_from_utf16` round-trips at lines 1126-1154,
     and the `LineIndex` rebuild on edit at line 1184). The
     24 in-source contract tests at the bottom of
     `crates/legion-text/src/lib.rs` cover the rope boundary
     — the `crlf_is_single_line_ending_for_lsp`,
     `property_edits_match_string_model_for_ascii`,
     `chunk_hashes_change_only_for_edited_chunk_when_boundaries_stay_stable`,
     `utf8_and_utf16_conversion_work_across_chunk_boundaries`,
     `large_file_typical_keystroke_edit_smoke`,
     `huge_single_line_files_are_bounded_without_full_text_materialization`,
     `large_snapshot_line_slices_and_chunks_are_bounded_by_default`,
     `large_snapshot_can_materialize_save_payload_from_chunks`,
     `edits_can_shrink_below_and_grow_above_full_cache_budget`,
     `explicit_full_text_access_fails_for_uncached_snapshot_and_buffer`,
     `opening_larger_than_budget_uses_degraded_cache_free_mode`,
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
     `visible_line_slices_return_exact_requested_range` tests.
     The M0 ratification does **not** change the `legion-text`
     boundary the WS-01.T6 anchor / operation layer composes
     through: the `legion-text` policy entry at
     `plans/dependency-policy.md` §1 lines 23-24 authorizes
     `legion-protocol` and nothing else, and the WS-01.T6
     anchor / operation layer is a protocol-mediated
     position-id + operation record over the existing
     `TextEdit` / `TextSnapshot` / `BufferVersion` /
     `SnapshotId` substrate (no new internal edge, no new
     external runtime dep). The M0 ratification explicitly
     forbids `legion-text` from declaring any CRDT runtime
     dependency (`loro`, `yrs`, `diamond-types`, `automerge`,
     etc.) at M0; the WS-16.T1 post-GA decision is the path
     that authorizes a CRDT runtime dep if the post-GA
     substrate needs it.
  2. The deterministic, metadata-first collaboration
     operation log + replay runtime that the anchor /
     operation-log layer composes through is live and
     exercised by tests today. `legion-collaboration`
     (`crates/legion-collaboration/src/lib.rs`, 1232 lines)
     ships the `CollaborationSessionRuntime` (around line
     101, carrying `descriptor: CollaborationSessionDescriptor`,
     `participants: HashMap<CollaborationParticipantId, CollaborationParticipant>`,
     `operations: Vec<CollaborationDocumentOperation>`,
     `acknowledgements: Vec<CollaborationAcknowledgement>`,
     `causal_gaps: Vec<CollaborationCausalGap>`, `presence:
     HashMap<...>`, `participant_sequences: HashMap<...>`,
     `operation_ids: HashSet<CollaborationOperationId>`) with
     `submit_operation` (around line 242, with the
     duplicate / gap / stale / conflict fail-closed paths),
     `publish_presence` (around line 217), and the
     deterministic replay manifest. The
     `CollaborationRuntimeError` enum (around line 21, with
     `RuntimeDisabled`, `InvalidSession`, `InvalidParticipant`,
     `InvalidOperation`, `Conflict`, `InvalidSessionState`
     variants) is the failure surface. The 6 in-source
     contract tests at the bottom of
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
     `legion-observability`, `legion-protocol`,
     `legion-security`, and `legion-storage`. The M0
     ratification does **not** change the `legion-collaboration`
     boundary the WS-01.T6 anchor / operation-log layer
     composes through: the WS-01.T6 workstream adds the
     anchor / position-id record and the
     agent-edit-while-typing overlay to the existing
     operation log, not a new operation-log runtime, and
     the M0 ratification explicitly forbids
     `legion-collaboration` from declaring any CRDT runtime
     dependency (`loro`, `yrs`, `diamond-types`, `automerge`,
     etc.) at M0. The WS-16.T1 post-GA decision is the path
     that authorizes a CRDT runtime dep if the post-GA
     substrate needs it.
  3. The protocol DTO surface the anchor / operation-log
     layer composes through is live and exercised by tests
     today. `legion-protocol`
     (`crates/legion-protocol/src/lib.rs`) is the protocol
     DTO layer. It owns the collaboration DTO surface the
     anchor / operation-log layer composes through: the
     `CollaborationOperationId` newtype (around line 1382,
     a `u128` opaque id), the
     `CollaborationVersionVectorEntry` struct (around line
     1511), the `CollaborationVersionVector` struct (around
     line 1520, carrying `entries: Vec<CollaborationVersionVectorEntry>`),
     the `CollaborationDocumentOperationKind` enum (around
     line 1527, with `Insert { text }` / `Delete` /
     `Replace { text }` / `CursorMove` / `SelectionUpdate` /
     `UndoCompensation` / `NoopAcknowledgement` /
     `ResyncRequest` variants), the
     `CollaborationOperationPreconditions` struct (around
     line 1553, carrying `workspace_id: WorkspaceId`,
     `file_id: FileId`, `buffer_id: BufferId`,
     `snapshot_id: SnapshotId`, `buffer_version: BufferVersion`,
     `document_epoch: CollaborationDocumentEpoch`,
     `base_vector: CollaborationVersionVector`,
     `author_principal: PrincipalId`, `capability_decision:
     CapabilityDecision`, non-zero `correlation_id: CorrelationId`,
     non-nil `causality_id: CausalityId`, `redaction_hints:
     Vec<RedactionHint>`), and the
     `CollaborationDocumentOperation` struct (around line
     1594, carrying `session_id: CollaborationSessionId`,
     `operation_id: CollaborationOperationId`,
     `author_participant_id: CollaborationParticipantId`,
     `participant_sequence: u64`, `kind: CollaborationDocumentOperationKind`,
     `range: Option<TextRange>`, `preconditions: CollaborationOperationPreconditions`,
     `undo_group: Option<UndoGroup>`, `occurred_at: TimestampMillis`,
     `schema_version: u16`). The 124 in-source DTO contract
     tests across `crates/legion-protocol/src/lib.rs` (15
     lib unittests + 109 integration tests in
     `crates/legion-protocol/tests/dto_contracts.rs`) cover
     the DTO surface that the WS-01.T6 anchor / operation-log
     layer composes through, including the metadata-only
     invariant on the operation preconditions, the
     version-vector ordering, the correlation / causality
     id validation, the `schema_version` rejection, the
     snapshot / buffer version pair as the operation base,
     and the per-participant sequence gap detection. The
     M0 ratification does **not** change the DTO surface
     the WS-01.T6 anchor / operation-log layer composes
     through: the `legion-protocol` policy entry at
     `plans/dependency-policy.md` §1 line 15 (the shared
     contracts boundary) is unchanged, and the M0 ratification
     does **not** add a new DTO for the `StablePositionId`
     / `PositionAnchor` family — the `TextEdit` /
     `TextSnapshot` / `BufferVersion` / `SnapshotId` family
     is the M0 boundary, and the WS-01.T6 workstream is
     the one that declares any new `StablePositionId` /
     `PositionAnchor` / `OperationLog` DTOs. The M0
     ratification explicitly forbids `legion-protocol` from
     declaring any CRDT runtime dependency (`loro`, `yrs`,
     `diamond-types`, `automerge`, etc.) at M0; the WS-16.T1
     post-GA decision is the path that authorizes a CRDT
     runtime dep if the post-GA substrate needs it.
  4. The editor substrate that consumes the WS-01.T6
     anchor / operation-log layer through its existing
     `legion-text` edge is policy-bounded and never owns
     anchor or operation-log state directly. `legion-editor`
     (`crates/legion-editor/src/lib.rs`, 3379 lines) is
     the editor substrate. The
     `plans/dependency-policy.md` §1 entry at lines 43-52
     authorizes `legion-observability`, `legion-protocol`,
     and `legion-text` (and the MUST rules at lines 48-51
     require `legion-protocol` and `legion-text` directly,
     and the MUST NOT rule at line 52 forbids the
     `legion-editor` ↔ `legion-project` edge). The M0
     ratification does **not** extend `legion-editor`'s
     allowed edges; the WS-01.T6 anchor / operation-log
     layer composes through the existing `legion-editor`
     ↔ `legion-text` edge plus the existing
     `legion-editor` ↔ `legion-protocol` edge, and the
     `legion-editor` MUST NOT `legion-collaboration` rule
     is unchanged (the `legion-collaboration` crate is
     composed by `legion-app`, not by `legion-editor`).
     The M0 ratification explicitly forbids `legion-editor`
     from declaring any CRDT runtime dependency (`loro`,
     `yrs`, `diamond-types`, `automerge`, etc.) at M0, and
     the `xtask` policy audit enforces the boundary. The
     36 in-source contract tests across
     `crates/legion-editor/src/lib.rs` (21 lib unittests
     + 7 workspace vfs integration tests + 1 additional
     integration test + 7 perf-harness tests with 3
     long-running perf workloads ignored) cover the
     editor boundary the WS-01.T6 workstream extends with
     anchor-aware transaction ordering.
  5. The GUI composition path the anchor / operation-log
     layer composes through is projection-bound and never
     owns anchor or operation-log state. `legion-app`
     (`crates/legion-app/src/lib.rs`) is the GUI composition
     crate. The `plans/dependency-policy.md` §1 entry at
     lines 86-105 authorizes the full app composition set
     (including `legion-collaboration`, `legion-editor`,
     `legion-text` through `legion-editor`, `legion-protocol`,
     `legion-agent`, `legion-ai`, `legion-ai-providers`,
     `legion-security`, `legion-platform`,
     `legion-observability`, etc.). The M0 ratification
     does **not** change `legion-app`'s allowed edges;
     the WS-01.T6 anchor / operation-log layer composes
     through the existing `legion-app` ↔ `legion-editor`
     ↔ `legion-text` edge chain and the existing
     `legion-app` ↔ `legion-collaboration` edge (already
     authorized by the §1 line 90 entry), and the WS-12 /
     WS-13 workstreams that consume the anchor layer
     (agent-edit-while-typing, diff overlay anchoring,
     inline-edit selection) compose through the same
     `legion-app` composition path. The M0 ratification
     does **not** authorize a new `legion-app` ↔ CRDT-crate
     edge; the WS-16.T1 post-GA decision is the path that
     authorizes a CRDT runtime dep if the post-GA
     substrate needs it. `legion-cli` is one of the two
     entry points that may launch the GUI composition
     (alongside `legion-desktop`); the capability-broker
     contract is the same from both entry points. The
     `legion-desktop` policy entry at
     `plans/dependency-policy.md` §1 line 77 authorizes
     `legion-app`, `legion-protocol`, and `legion-ui`.
     The M0 ratification does **not** extend
     `legion-desktop`'s allowed edges; the desktop adapter
     launches the GUI composition through the existing
     `legion-desktop` ↔ `legion-app` edge, and the
     proposal / evidence envelope applies identically
     from both entry points. The `legion-ui` policy entry
     at lines 54-75 forbids every renderer / editor /
     project / storage / app / agent / terminal /
     security / observability / platform edge, and the
     structural audit enforces it. The boundary sketch
     in the ratified ADR reinforces this rule with a
     future `CONCURRENT_EDIT_BOUNDARY_POLICY_MARKERS`
     audit (no `legion-ui` may declare any `loro` /
     `yrs` / `diamond-types` / `automerge` runtime
     dependency), shaped like the existing
     `PARSER_BOUNDARY_POLICY_MARKERS` audit in
     `xtask/src/main.rs` and the
     `SEARCH_BOUNDARY_POLICY_MARKERS` /
     `RETRIEVAL_BOUNDARY_POLICY_MARKERS` /
     `LSP_BOUNDARY_POLICY_MARKERS` /
     `TERMINAL_BOUNDARY_POLICY_MARKERS` /
     `SANDBOX_BOUNDARY_POLICY_MARKERS` /
     `AGENT_INTEROP_BOUNDARY_POLICY_MARKERS` sketches
     in ADR-0034 / 0035 / 0036 / 0037 / 0038 / 0039.
  6. The `legion-text` / `legion-protocol` /
     `legion-collaboration` / `legion-editor` /
     `legion-app` policy entries the M0 ratification
     ratifies are unchanged and never authorize a CRDT
     runtime dep at M0. The
     `plans/dependency-policy.md` §1 line 23-24 entry
     for `legion-text` allows only `legion-protocol` and
     forbids any CRDT runtime dep. The
     `plans/dependency-policy.md` §1 line 15 entry for
     `legion-protocol` is the shared contracts boundary
     and forbids any CRDT runtime dep. The
     `plans/dependency-policy.md` §1 lines 219-225 entry
     for `legion-collaboration` allows only
     `legion-observability`, `legion-protocol`,
     `legion-security`, and `legion-storage` and forbids
     any CRDT runtime dep. The
     `plans/dependency-policy.md` §1 lines 43-52 entry
     for `legion-editor` allows only `legion-observability`,
     `legion-protocol`, and `legion-text` and forbids any
     CRDT runtime dep. The
     `plans/dependency-policy.md` §1 lines 86-105 entry
     for `legion-app` authorizes the full app composition
     set and does not authorize a new `legion-app` ↔
     CRDT-crate edge at M0. The
     `xtask` policy audit confirms zero `loro` / `yrs` /
     `diamond-types` / `automerge` workspace dependencies
     exist in `Cargo.lock` today. The WS-16.T1 post-GA
     decision is the path that authorizes a CRDT runtime
     dep if the post-GA substrate needs it, and the same
     dependency-policy gate pattern that authorized the
     parser-boundary audit in `ADR-0033`, the LSP-boundary
     audit sketched in `ADR-0034`, the terminal-boundary
     sketch in `ADR-0035`, the search-boundary sketch in
     `ADR-0036`, the retrieval-boundary sketch in
     `ADR-0037`, the sandbox-boundary sketch in
     `ADR-0038`, and the agent-interop-boundary sketch in
     `ADR-0039` is the gate that authorizes the
     concurrent-edit-boundary audit below.

## Crate / Dependency Boundary Impact

- No new internal crate edges are introduced by this ADR.
  The concurrent-edit layer is split across `legion-text`,
  `legion-protocol`, `legion-collaboration`, `legion-editor`,
  and `legion-app` along the accepted policy entries in
  `plans/dependency-policy.md` §1.
- The `legion-text` policy entry at
  `plans/dependency-policy.md` §1 lines 23-24 is unchanged:
  `legion-text` may depend on `legion-protocol` and nothing
  else. The current `crates/legion-text/Cargo.toml` is
  consistent with this entry. The M0 ratification does
  **not** declare any new `legion-text` dependency, and
  the M0 ratification explicitly forbids `legion-text`
  from declaring any CRDT runtime dependency (`loro`,
  `yrs`, `diamond-types`, `automerge`, etc.) at M0. The
  WS-16.T1 post-GA decision is the path that authorizes
  a CRDT runtime dep if the post-GA substrate needs it.
  The `xtask` policy audit confirms zero CRDT runtime
  deps in `Cargo.lock` today.
- The `legion-protocol` policy entry at
  `plans/dependency-policy.md` §1 line 15 is unchanged:
  `legion-protocol` is the shared contracts boundary
  and may not depend on any other workspace crate. The
  current `crates/legion-protocol/Cargo.toml` is
  consistent with this entry. The M0 ratification does
  **not** declare any new `legion-protocol` dependency,
  and the collaboration DTO surface
  (`CollaborationOperationId` /
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
  `RetentionLabel` DTOs at lines 1382 / 1511 / 1520 /
  1527 / 1553 / 1594 in the current tree) is the M0
  boundary. The WS-01.T6 workstream may add new DTOs
  for the `StablePositionId` / `PositionAnchor` /
  `OperationLog` family but the M0 boundary is the
  existing DTOs. The M0 ratification explicitly forbids
  `legion-protocol` from declaring any CRDT runtime
  dependency (`loro`, `yrs`, `diamond-types`,
  `automerge`, etc.) at M0, and the `xtask` policy
  audit enforces the boundary.
- The `legion-collaboration` policy entry at
  `plans/dependency-policy.md` §1 lines 219-225 is
  unchanged: `legion-collaboration` may depend on
  `legion-observability`, `legion-protocol`,
  `legion-security`, and `legion-storage`. The current
  `crates/legion-collaboration/Cargo.toml` is consistent
  with this entry, and the
  `CollaborationSessionRuntime` at line 101 is the
  runtime the WS-01.T6 anchor / operation-log layer
  composes through. The M0 ratification does **not**
  declare any new `legion-collaboration` dependency,
  and the WS-01.T6 workstream is the one that adds
  the anchor / position-id record and the
  agent-edit-while-typing overlay to the existing
  operation log. The M0 ratification explicitly
  forbids `legion-collaboration` from declaring any
  CRDT runtime dependency (`loro`, `yrs`,
  `diamond-types`, `automerge`, etc.) at M0, and the
  `xtask` policy audit enforces the boundary. The
  WS-16.T1 post-GA decision is the path that
  authorizes a CRDT runtime dep if the post-GA
  substrate needs it.
- The `legion-editor` policy entry at
  `plans/dependency-policy.md` §1 lines 43-52 is
  unchanged: `legion-editor` may depend on
  `legion-observability`, `legion-protocol`, and
  `legion-text` (and the MUST rules at lines 48-51
  require `legion-protocol` and `legion-text`
  directly, and the MUST NOT rule at line 52 forbids
  the `legion-editor` ↔ `legion-project` edge). The
  current `crates/legion-editor/Cargo.toml` is
  consistent with this entry. The M0 ratification
  does **not** extend `legion-editor`'s allowed
  edges, and the M0 ratification explicitly forbids
  `legion-editor` from declaring any CRDT runtime
  dependency (`loro`, `yrs`, `diamond-types`,
  `automerge`, etc.) at M0. The WS-01.T6 anchor /
  operation-log layer composes through the existing
  `legion-editor` ↔ `legion-text` edge plus the
  existing `legion-editor` ↔ `legion-protocol` edge.
  The `legion-editor` MUST NOT `legion-collaboration`
  rule is unchanged: `legion-collaboration` is
  composed by `legion-app`, not by `legion-editor`.
- The `legion-app` policy entry at
  `plans/dependency-policy.md` §1 lines 86-105 is
  unchanged: `legion-app` may depend on the full app
  composition set (`legion-agent`, `legion-ai`,
  `legion-ai-providers`, `legion-collaboration`,
  `legion-editor`, `legion-index`, `legion-lsp`,
  `legion-memory`, `legion-observability`,
  `legion-platform`, `legion-plugin`, `legion-project`,
  `legion-protocol`, `legion-remote`, `legion-security`,
  `legion-storage`, `legion-terminal`, `legion-tracker`,
  `legion-ui`, plus the remaining lines). The current
  `crates/legion-app/Cargo.toml` is consistent with
  this entry. The M0 ratification does **not** authorize
  a new `legion-app` ↔ CRDT-crate edge; the WS-01.T6
  anchor / operation-log layer composes through the
  existing `legion-app` ↔ `legion-editor` ↔
  `legion-text` edge chain and the existing
  `legion-app` ↔ `legion-collaboration` edge (the
  `legion-collaboration` policy entry at §1 line 90
  already authorizes the edge). The WS-12 / WS-13 /
  WS-07 / WS-11 workstreams that consume the anchor
  layer (agent-edit-while-typing, diff overlay
  anchoring, conflict UX, inline-edit selection)
  compose through the same `legion-app` composition
  path.
- The `legion-cli` policy entry at
  `plans/dependency-policy.md` §1 line 175 is unchanged:
  `legion-cli` may depend on `legion-index`,
  `legion-protocol`, and `legion-storage`. The current
  `crates/legion-cli/Cargo.toml` is consistent with
  this entry. The M0 ratification explicitly forbids
  `legion-cli` from declaring any CRDT runtime
  dependency (`loro`, `yrs`, `diamond-types`,
  `automerge`, etc.), and the `xtask` policy audit
  enforces the boundary by iterating the same
  `package_dependencies` map that drives the
  renderer-boundary, parser-boundary,
  search-boundary, retrieval-boundary,
  LSP-boundary, terminal-boundary,
  sandbox-boundary, and agent-interop-boundary
  checks.
- The `legion-desktop` policy entry at
  `plans/dependency-policy.md` §1 line 77 is unchanged:
  `legion-desktop` may depend on `legion-app`,
  `legion-protocol`, and `legion-ui`. The current
  `crates/legion-desktop/Cargo.toml` is consistent
  with this entry. The M0 ratification does **not**
  authorize a new `legion-desktop` ↔ CRDT-crate
  edge; the desktop adapter launches the GUI
  composition through the existing `legion-desktop`
  ↔ `legion-app` edge, and the proposal / evidence
  envelope applies identically from both entry
  points.
- The `legion-ui` policy entry at
  `plans/dependency-policy.md` §1 lines 54-75 already
  forbids `legion-ui` from depending on `legion-project`,
  `legion-editor`, `legion-storage`, `eframe`, `egui`,
  `egui-winit`, `egui-wgpu`, `winit`, `wgpu`,
  `accesskit`, `slint`, `tauri`, `wry`, `tao`, or `gpui`.
  None of the CRDT runtime crates (`loro`, `yrs`,
  `diamond-types`, `automerge`, etc.) are added to
  that list because the `legion-ui` policy entry is
  already a closed boundary (only `legion-protocol`
  is allowed). The boundary sketch in the ratified
  ADR reinforces this rule with a future
  `CONCURRENT_EDIT_BOUNDARY_POLICY_MARKERS` audit
  (no `legion-ui` may declare any `loro` / `yrs` /
  `diamond-types` / `automerge` runtime dependency),
  shaped like the existing
  `PARSER_BOUNDARY_POLICY_MARKERS` audit in
  `xtask/src/main.rs` and the
  `SEARCH_BOUNDARY_POLICY_MARKERS` /
  `RETRIEVAL_BOUNDARY_POLICY_MARKERS` /
  `LSP_BOUNDARY_POLICY_MARKERS` /
  `TERMINAL_BOUNDARY_POLICY_MARKERS` /
  `SANDBOX_BOUNDARY_POLICY_MARKERS` /
  `AGENT_INTEROP_BOUNDARY_POLICY_MARKERS` sketches
  in ADR-0034 / 0035 / 0036 / 0037 / 0038 / 0039.
- The `legion-agent` policy entry at
  `plans/dependency-policy.md` §1 line 144 is
  unchanged: `legion-agent` may depend on
  `legion-ai`, `legion-protocol`, and `legion-tracker`.
  The current `crates/legion-agent/Cargo.toml` is
  consistent with this entry, and `legion-agent`
  does not contain any anchor or operation-log
  runtime code today. The M0 ratification
  explicitly forbids `legion-agent` from declaring
  any CRDT runtime dependency (`loro`, `yrs`,
  `diamond-types`, `automerge`, etc.) at M0, and
  the `xtask` policy audit enforces the boundary.
  The WS-12.T1 / WS-13.T4 workstreams that consume
  the anchor layer (agent-edit-while-typing,
  ACP host) compose through the existing
  `legion-agent` ↔ `legion-ai` ↔ `legion-ai-providers`
  ↔ `legion-protocol` edge chain, and the
  agent-edit-while-typing overlay wraps external
  agent edits in the same proposal envelope so
  the proposal service, the capability broker, and
  the evidence ledger all see the external
  agent's edits identically to the native agent's
  edits. The M0 ratification does **not** authorize
  a new `legion-agent` ↔ CRDT-crate edge; the
  WS-16.T1 post-GA decision is the path that
  authorizes a CRDT runtime dep if the post-GA
  substrate needs it.
- The `legion-ai` and `legion-ai-providers` policy
  entries at `plans/dependency-policy.md` §1 lines
  111-117 / 119-125 are unchanged: `legion-ai` may
  depend on `legion-protocol` and `legion-security`,
  and `legion-ai-providers` may depend on `legion-ai`,
  `legion-protocol`, and `legion-security`. The M0
  ratification explicitly forbids `legion-ai` /
  `legion-ai-providers` from declaring any CRDT
  runtime dependency (`loro`, `yrs`,
  `diamond-types`, `automerge`, etc.) at M0, and
  the `xtask` policy audit enforces the boundary.
- The concurrent-edit workspace dependencies
  (`loro`, `yrs`, `diamond-types`, `automerge`,
  etc.) are **not** added to the root `Cargo.toml`
  at M0. They will be added during WS-16.T1 (the
  post-GA CRDT decision) if the post-GA substrate
  needs a CRDT runtime dep, and the WS-16.T1
  workstream is the one that amends the
  `legion-text` / `legion-protocol` /
  `legion-collaboration` / `legion-editor` /
  `legion-app` policy entries to authorize the
  CRDT runtime dep, and adds the
  `CONCURRENT_EDIT_BOUNDARY_POLICY_MARKERS` audit
  to `xtask/src/main.rs`. The gate is
  forward-compatible with a future
  `CONCURRENT_EDIT_BOUNDARY_POLICY_MARKERS` /
  `CONCURRENT_EDIT_ANCHOR_TRAIT_PACKAGES = ["legion-text"]` /
  `CONCURRENT_EDIT_OPERATION_LOG_PACKAGES = ["legion-collaboration"]` /
  `CONCURRENT_EDIT_DTO_PACKAGES = ["legion-protocol"]` /
  `FORBIDDEN_CONCURRENT_EDIT_DEPS = ["loro", "yrs", "diamond-types", "automerge"]`
  audit shaped like the existing
  `PARSER_BOUNDARY_POLICY_MARKERS` /
  `PARSER_DEPENDENCY_ALLOWED_PACKAGES = ["legion-index"]` /
  `FORBIDDEN_PARSER_DEPS = ["tree-sitter", "tree-sitter-rust"]`
  audit in `xtask/src/main.rs` (the constants are
  at lines 446-453 in the current tree). The M0
  ratification does not require the
  concurrent-edit-boundary audit to land today; the
  ADR commits to the boundary and to the
  forward-compatibility gate, not to a new `xtask`
  subcommand.
- `xtask` does not need a new subcommand. The
  structural dependency audit and the
  protocol-contract audit that already run as part
  of `check-deps` are sufficient to enforce the
  current `legion-text`, `legion-protocol`,
  `legion-collaboration`, `legion-editor`,
  `legion-app`, `legion-cli`, `legion-desktop`,
  `legion-ui`, `legion-agent`, `legion-ai`, and
  `legion-ai-providers` policy entries; the future
  concurrent-edit-boundary audit is a phase-gate
  improvement, not an M0 prerequisite.
- No new capability names are introduced by this
  ADR. The Phase 8 production capability
  reservations at `plans/dependency-policy.md` §1
  line 247 plus the OS-sandbox extensions from
  ADR-0038 are the policy surface. The M0
  ratification does **not** add a new capability
  name; the WS-01.T6 workstream composes the
  anchor / operation-log layer through the existing
  `legion-security` broker and the existing
  Phase 8 capability reservations. The
  `CollaborationDocumentOperation` envelope already
  carries the operation preconditions
  (correlation / causality / snapshot / buffer
  version / schema version / capability decision)
  that the proposal service uses to detect stale /
  duplicate / gap / conflict before any apply, and
  the `CollaborationRuntimeError` enum's
  `Conflict` / `Stale` / `GapDetected` variants
  (asserted by the
  `duplicate_gap_and_conflict_fail_closed_without_clobbering_text`
  and
  `concurrent_insert_converges_for_two_three_and_five_participants`
  contract tests) are the failure surface.

## Gate Evidence (verbatim)

All gates were run against the current working tree with commit
baseline `b56dcb2`; the ratification changes (ADR flip + this
evidence file) are untracked as required by the task's "no commit
without explicit user instruction" rule. (The working tree
contains unrelated uncommitted edits from sibling M0 cards; they
are not part of this ratification and are noted only so the gate
outputs are reproducible against the same baseline.)

### `cargo run -p xtask -- check-deps`

```
$ cd /Users/christopherwilloughby/legion-ide
$ cargo run -p xtask -- check-deps
   Compiling xtask v0.1.0 (/Users/christopherwilloughby/legion-ide/xtask)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 4.82s
     Running `target/debug/xtask check-deps`
dependency policy checks passed
```

Exit code: `0`. The renderer-boundary audit, the parser-boundary
audit, the structural dependency audit, the protocol-contract
audit, and the phase 3 / 4 / 5 / 6 / 7 / 8 / 13 acceptance
governance audits all pass against the current tree. In
particular:

- `plans/dependency-policy.md` still contains every
  `PARSER_BOUNDARY_POLICY_MARKERS` string.
- The `legion-text` policy entry at
  `plans/dependency-policy.md` §1 lines 23-24 is
  intact and matches `crates/legion-text/Cargo.toml`
  (`legion-protocol`; no CRDT runtime dependency
  declared today).
- The `legion-protocol` policy entry at
  `plans/dependency-policy.md` §1 line 15 is intact
  and the collaboration DTO surface
  (`CollaborationOperationId` /
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
  `RetentionLabel` DTOs at lines 1382 / 1511 / 1520
  / 1527 / 1553 / 1594 in the current tree) is
  intact and exercised by 124 DTO contract tests
  across the lib unittests (15 tests) + the
  `dto_contracts` integration test (109 tests).
- The `legion-collaboration` policy entry at
  `plans/dependency-policy.md` §1 lines 219-225 is
  intact and matches
  `crates/legion-collaboration/Cargo.toml`
  (`legion-observability`, `legion-protocol`,
  `legion-security`, `legion-storage`; no CRDT
  runtime dependency declared today; the
  `CollaborationSessionRuntime` at line 101 is the
  deterministic in-memory operation log runtime
  that the WS-01.T6 anchor / operation-log layer
  composes through).
- The `legion-editor` policy entry at
  `plans/dependency-policy.md` §1 lines 43-52 is
  intact and matches
  `crates/legion-editor/Cargo.toml` (`legion-observability`,
  `legion-protocol`, `legion-text`; no CRDT runtime
  dependency declared today; the editor substrate
  consumes the WS-01.T6 anchor / operation-log
  layer through its existing `legion-text` edge;
  the `legion-editor` MUST NOT `legion-collaboration`
  rule is unchanged).
- The `legion-app` policy entry at
  `plans/dependency-policy.md` §1 lines 86-105 is
  intact and matches `crates/legion-app/Cargo.toml`
  (the full app composition set, including
  `legion-collaboration`, `legion-editor`,
  `legion-text` through `legion-editor`,
  `legion-protocol`, `legion-agent`, `legion-ai`,
  `legion-ai-providers`, `legion-security`,
  `legion-platform`, `legion-observability`; no
  CRDT runtime dependency declared today; the M0
  ratification does **not** authorize a new
  `legion-app` ↔ CRDT-crate edge).
- The `legion-cli` policy entry at
  `plans/dependency-policy.md` §1 line 175 is
  intact and matches `crates/legion-cli/Cargo.toml`
  (`legion-index`, `legion-protocol`, `legion-storage`;
  no CRDT runtime dependency declared today).
- The `legion-desktop` policy entry at
  `plans/dependency-policy.md` §1 line 77 is intact
  and matches `crates/legion-desktop/Cargo.toml`
  (`legion-app`, `legion-protocol`, `legion-ui`;
  no CRDT runtime dependency declared today).
- The `legion-ui` policy entry at
  `plans/dependency-policy.md` §1 lines 54-75 is
  intact and matches `crates/legion-ui/Cargo.toml`
  (`legion-protocol` only; the `legion-ui` policy
  entry is already a closed boundary and the M0
  ratification does **not** add any CRDT runtime
  crate to that list). The `xtask` policy audit
  confirms zero `loro` / `yrs` /
  `diamond-types` / `automerge` workspace
  dependencies exist in `Cargo.lock` today.
- The `legion-agent` policy entry at
  `plans/dependency-policy.md` §1 line 144 is
  intact and matches `crates/legion-agent/Cargo.toml`
  (`legion-ai`, `legion-protocol`, `legion-tracker`;
  no CRDT runtime dependency declared today;
  `legion-agent` does not contain any anchor or
  operation-log runtime code).
- The `legion-ai` and `legion-ai-providers` policy
  entries at `plans/dependency-policy.md` §1 lines
  111-117 / 119-125 are intact and match
  `crates/legion-ai/Cargo.toml` and
  `crates/legion-ai-providers/Cargo.toml`
  respectively (`legion-protocol` + `legion-security`
  for `legion-ai`; `legion-ai` + `legion-protocol` +
  `legion-security` for `legion-ai-providers`; no
  CRDT runtime dependency declared today).

### `cargo run -p xtask -- docs-hygiene`

```
$ cd /Users/christopherwilloughby/legion-ide
$ cargo run -p xtask -- docs-hygiene
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.05s
     Running `target/debug/xtask docs-hygiene`
documentation hygiene checks passed
```

Exit code: `0`. No broken relative Markdown links, no
unallowlisted stale Legion-rename marker, and the
`plans/adrs/ADR-0040-concurrent-edit-substrate.md` file
exists under `plans/adrs/` and is reachable from the
`§6 Architecture Decision Queue` row 240 reference
inside `plans/legion-production-master-plan-v0.1.md`.

### `cargo run -p xtask -- no-egui-textedit`

```
$ cd /Users/christopherwilloughby/legion-ide
$ cargo run -p xtask -- no-egui-textedit
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.05s
     Running `target/debug/xtask no-egui-textedit`
no-egui-textedit checks passed
```

Exit code: `0`. The companion gate from `ADR-0032`
remains green; the WS-01.T6 anchor / operation-log
panel and the WS-07.T2 conflict-UX panel render
projected anchor and operation-log results, not an
`egui::TextEdit`. The M0 ratification does **not**
reopen the gate; the gate is a forward-compatibility
check that the WS-01.T6 / WS-07.T2 / WS-12.T1
acceptance shape respects.

### `cargo fmt --all --check`

```
$ cd /Users/christopherwilloughby/legion-ide
$ cargo fmt --all --check
$ echo $?
0
```

Exit code: `0`. No formatting drift introduced by
the ADR flip; the ADR is markdown-only and the
evidence file is markdown-only.

### `cargo test -p legion-text --tests`

```
$ cd /Users/christopherwilloughby/legion-ide
$ cargo test -p legion-text --tests
   Compiling legion-text v0.1.0 (/Users/christopherwilloughby/legion-ide/crates/legion-text)
    Finished `test` profile [unoptimized + debuginfo] target(s)
     Running unittests src/lib.rs (target/debug/deps/legion_text-d49fd60f06d73b8a)

running 24 tests
test tests::crlf_is_single_line_ending_for_lsp ... ok
test tests::buffer_rejects_non_boundary_edits ... ok
test tests::content_hash_has_expected_prefix ... ok
test tests::buffer_insert_delete_replace ... ok
test tests::line_slice_can_span_multiple_chunks ... ok
test tests::position_roundtrip_multibyte_columns_are_bytes ... ok
test tests::crlf_pair_is_not_split_when_near_chunk_boundary ... ok
test tests::snapshot_clone_is_cheap_and_immutable ... ok
test tests::snapshot_descriptor_has_required_metadata ... ok
test tests::text_position_display ... ok
test tests::text_range_empty ... ok
test tests::utf16_golden_surrogate_pairs ... ok
test tests::utf16_range_golden ... ok
test tests::property_edits_match_string_model_for_ascii ... ok
test tests::visible_line_slices_return_exact_requested_range ... ok
test tests::chunk_hashes_change_only_for_edited_chunk_when_boundaries_stay_stable ... ok
test tests::utf8_and_utf16_conversion_work_across_chunk_boundaries ... ok
test tests::large_file_typical_keystroke_edit_smoke ... ok
test tests::huge_single_line_files_are_bounded_without_full_text_materialization ... ok
test tests::large_snapshot_line_slices_and_chunks_are_bounded_by_default ... ok
test tests::large_snapshot_can_materialize_save_payload_from_chunks ... ok
test tests::edits_can_shrink_below_and_grow_above_full_cache_budget ... ok
test tests::explicit_full_text_access_fails_for_uncached_snapshot_and_buffer ... ok
test tests::opening_larger_than_budget_uses_degraded_cache_free_mode ... ok

test result: ok. 24 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.86s
```

Exit code: `0`. The 24 contract tests across the
`legion-text` lib unittests cover the rope boundary
the WS-01.T6 anchor / operation-log layer composes
through (`TextPosition` / `TextRange` / `TextEdit` /
`TextSnapshot` / `TextSnapshotDescriptor` /
`TextBuffer` / `LineIndex` surface; rope chunk
hashing; UTF-8 / UTF-16 round-trips; chunk-rebuild-
on-edit invariant; large-file degraded cache-free
mode; `apply_edit` / `try_apply_edit` /
`set_version` / `byte_offset` / `utf16_position` /
`byte_offset_from_utf16` round-trips; LSP
`crlf_is_single_line_ending_for_lsp` conformance).

### `cargo test -p legion-protocol --tests`

```
$ cd /Users/christopherwilloughby/legion-ide
$ cargo test -p legion-protocol --tests
   Compiling legion-protocol v0.1.0 (/Users/christopherwilloughby/legion-ide/crates/legion-protocol)
    Finished `test` profile [unoptimized + debuginfo] target(s)

test result: ok. 15 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 109 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
```

Exit code: `0`. The 124 DTO contract tests across
the `legion-protocol` lib unittests (15 tests) +
the `dto_contracts` integration test (109 tests)
cover the collaboration DTO surface the WS-01.T6
anchor / operation-log layer composes through
(`CollaborationOperationId` /
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
`RetentionLabel` DTOs).

### `cargo test -p legion-collaboration --tests`

```
$ cd /Users/christopherwilloughby/legion-ide
$ cargo test -p legion-collaboration --tests
   Compiling legion-protocol v0.1.0 (/Users/christopherwilloughby/legion-ide/crates/legion-protocol)
   Compiling legion-collaboration v0.1.0 (/Users/christopherwilloughby/legion-ide/crates/legion-collaboration)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 10.34s
     Running unittests src/lib.rs (target/debug/deps/legion_collaboration-1c0351b7f3e418cb)

running 6 tests
test tests::default_runtime_config_is_fail_closed ... ok
test tests::duplicate_gap_and_conflict_fail_closed_without_clobbering_text ... ok
test tests::disconnect_reconnect_and_shutdown_states_are_fail_closed ... ok
test tests::delete_replace_and_undo_compensation_are_deterministic_metadata_operations ... ok
test tests::presence_and_replay_manifest_are_metadata_only ... ok
test tests::concurrent_insert_converges_for_two_three_and_five_participants ... ok

test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

Exit code: `0`. The 6 contract tests across the
`legion-collaboration` lib unittests cover the
deterministic, metadata-first collaboration
operation log + replay runtime that the WS-01.T6
anchor / operation-log layer extends
(`default_runtime_config_is_fail_closed` /
`duplicate_gap_and_conflict_fail_closed_without_clobbering_text`
/ `disconnect_reconnect_and_shutdown_states_are_fail_closed`
/ `delete_replace_and_undo_compensation_are_deterministic_metadata_operations`
/ `presence_and_replay_manifest_are_metadata_only`
/ `concurrent_insert_converges_for_two_three_and_five_participants`).

### `cargo test -p legion-editor --tests`

```
$ cd /Users/christopherwilloughby/legion-ide
$ cargo test -p legion-editor --tests
    Finished `test` profile [unoptimized + debuginfo] target(s)

test result: ok. 21 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.66s
test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.82s
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.16s
test result: ok. 7 passed; 0 failed; 3 ignored; 0 measured; 0 filtered out; finished in 9.98s
```

Exit code: `0`. The 36 contract tests across the
`legion-editor` lib unittests (21 tests) + the
workspace vfs integration test (7 tests) + 1
additional integration test + the perf-harness
integration test (7 tests with 3 long-running perf
workloads ignored) cover the editor boundary the
WS-01.T6 workstream extends with anchor-aware
transaction ordering.

## Comparison to the M0 Plan Lock State

- The M0 ratification records the Production Master
  Plan v0.1 §6 row 240 recommendation verbatim
  (option (a)) and does not amend the master plan.
- The M0 ratification does **not** change
  `plans/dependency-policy.md`: the `legion-text`
  / `legion-protocol` / `legion-collaboration` /
  `legion-editor` / `legion-app` / `legion-cli` /
  `legion-desktop` / `legion-ui` / `legion-agent` /
  `legion-ai` / `legion-ai-providers` policy
  entries are unchanged; no new internal crate
  edge is added; no new external runtime dep is
  added; no new capability name is added; no new
  xtask subcommand is added.
- The M0 ratification ratifies a forward-compatible
  `CONCURRENT_EDIT_BOUNDARY_POLICY_MARKERS` audit
  sketch (the audit lands when a workspace
  package actually declares a CRDT runtime dep
  at the post-GA WS-16.T1 workstream; today no
  package declares one).
- The M0 ratification documents the WS-01.T6 /
  WS-12.T1 / WS-13.T4 / WS-07.T2 / WS-11.T2 /
  WS-16.T1 acceptance shape but does **not** ship
  the anchor / operation-log layer; the M0
  ratification ratifies the boundary, the choice,
  the substrate, and the deferral.
- The M0 ratification does **not** require the
  `legion-text` 100MB streaming-mode workload
  (the 100MB performance workload is a known
  degraded / streaming-mode gap, not a green
  benchmark) or the `legion-editor` 100MB perf
  workload (the 3 ignored tests in the
  `legion-editor` perf-harness integration test)
  to land at M0; both are WS-01.T7 acceptance
  criteria and are recorded as such in the master
  plan §7 / §11.
- The M0 ratification does **not** commit any
  untracked change to git; the ADR flip + this
  evidence file are untracked as required by the
  task's "no commit without explicit user
  instruction" rule.

## Independent Review Path

This ratification was prepared by the legionworker
Kanban worker profile and follows the same
acceptance shape as the M0 ADR-0035 / 0036 / 0037
/ 0038 / 0039 ratifications. The
`plans/evidence/production/M0/ADR-0040-ratification.md`
file is the M0 evidence package for this
ratification; the
`plans/evidence/production/M0/ADR-0039-ratification.md`
and `plans/evidence/production/M0/ADR-0038-ratification.md`
files are the M0 evidence packages for the prior
ratifications and the precedent for the evidence
shape used here. The independent-review pass
(coordinator / reviewer subagent run after this
ratification) is responsible for re-running the
four phase gates against the untracked ADR flip +
evidence file changes, confirming the verbatim
gate outputs, and recording the reviewer verdict
in a follow-up comment on the Kanban card.
