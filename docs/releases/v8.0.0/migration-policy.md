# Legion IDE v8.0.0 — Migration Policy

> **STATUS: FORWARD-LOOKING TEMPLATE — NOT A CURRENT RELEASE ARTIFACT.** No v8.0.0 release exists or is scheduled. The workspace version is 0.1.0 and the product is pre-beta; see `plans/product-readiness-ledger.md` for current status. CI references in this document (e.g. `.github/workflows/ci.yml`) describe a future pipeline that is not yet configured, and some referenced companion documents (e.g. `handoff.md`, `stable-channel-approval.md`) do not exist yet.

> Companion to `release-checklist.md` and `rollback-policy.md` under this
> directory. This document governs how the v8.0.0 GA cut moves user save
> data forward from v7.x and how the server-side data shape is staged
> during the cutover window. It is the operator-facing migration policy;
> implementation lives in `xtask`, `legion-storage`, `legion-retention`,
> `legion-telemetry`, and the `legion-desktop` installer descriptors.

| Field | Value |
| --- | --- |
| Release | v8.0.0 (workspace version bump from `0.1.0` → `8.0.0`) |
| Authoritative source tree | `9thLevelSoftware/legion-ide` |
| Current HEAD at policy draft | `dc92004` (M6 acceptance gate) |
| Companion docs | [`release-checklist.md`](./release-checklist.md), [`rollback-policy.md`](./rollback-policy.md), [`handoff.md`](./handoff.md), [`stable-channel-approval.md`](./stable-channel-approval.md) |
| Supersedes | None (first GA cut that ships an explicit migration policy) |
| Last reviewed | 2026-06-22 |
| Review cadence | Re-validated on every GA-tag `v8.*.*` and on every M-gate transition |

## 1. Scope and goals

### 1.1 What "migration" means for v8.0.0

Three independent migrations land together:

1. **User save-data forward compatibility** — existing v7.x workspace
   data (sessions, dock layouts, file metadata records, plugin storage,
   raw-source retention envelopes, trust decisions, and CLI telemetry
   spool files) opens without data loss in v8.0.0 and is upgraded in
   place by the v8 storage migration registry.
2. **Server-side data shape changes** — the protocol-level schema for
   `StorageSchemaManifest`, `StorageMigrationStep`, `StorageBackupMarker`,
   `StorageChecksum`, `StorageRecoveryOutcome`, `HostedTelemetrySpoolRecord`,
   `RawSourceRetentionConsentGrant`, and `RawSourceRetentionLease` is
   versioned (`schema_version = 1`) and validated through
   `legion_protocol::validate_*` before any on-disk artifact is touched.
3. **Dual-write window** — for the cutover period the v8 GA build
   produces both the v7 protocol bytes and the v8 protocol bytes for
   every protocol emission that is part of the migration surface
   (`session_record`, `workspace_config`, `dock_layout`, `raw_source_vault_envelope`,
   `hosted_telemetry_spool_record`).

### 1.2 Goals

- **Zero-data-loss** from any v7.x install that boots v8.0.0.
- **Single-shot in-place upgrade** for ≥ 99 % of installs; the remainder
  must hit a documented backfill branch, not silent corruption.
- **Forward-only migrations** — no schema can be downgraded by the
  migration registry; downgrade is a rollback concern covered in
  `rollback-policy.md`.
- **Metadata-only default** — every persisted record must remain
  metadata-only by default; raw-source bytes, prompts, and provider
  payloads stay encrypted under `legion-retention` and never appear in
  the migration dry-run output.

### 1.3 Non-goals

- Migration of third-party VS Code extensions (PR-VSC-002 remains
  deferred; see `plans/product-readiness-ledger.md`).
- Migration of marketplace / registry state from legacy `devil-app`
  builds (covered by the v7 → v8 handoff, not this policy).
- Migration of remote / collaboration server state (collaboration
  substrate remains local-deterministic in v8.0.0; remote transport
  is post-GA).

## 2. Authoritative source citations

| Topic | Source | Symbol / Line |
| --- | --- | --- |
| Migration registry | `crates/legion-storage/src/lib.rs` | `StorageMigrationRegistry` (line 291), `StorageMigrationStep`, `StorageSchemaManifest`, `StorageMigrationDryRunReport` |
| Backup + recovery | `crates/legion-storage/src/lib.rs` | `backup_file` (line 370), `recover_from_backup` (line 406), `StorageBackupMarker`, `STORAGE_CHECKSUM_ALGORITHM` (line 48) |
| Forward-only invariant | `crates/legion-storage/src/lib.rs` | `StorageMigrationRegistry::register` (line 315): rejects `from_schema_version == 0`, `to <= from`, empty ids |
| Save flow | `crates/legion-app/src/lib.rs` | `SaveWorkflowService::save_active_buffer` (line 10430), `WorkspaceActor::save_file_with_proposal` (line 10525) |
| Protocol validators | `crates/legion-protocol/src/lib.rs` | `validate_storage_schema_manifest`, `validate_storage_migration_dry_run_report`, `validate_storage_backup_marker`, `validate_storage_recovery_outcome`, `validate_storage_repair_request` |
| Raw-source retention | `crates/legion-retention/src/lib.rs` | `RawSourceRetentionPolicy`, `RawSourceRetentionConsentGrant`, `RawSourceRetentionLease`, `validate_raw_source_vault_envelope` |
| Hosted telemetry consent | `crates/legion-telemetry/src/lib.rs` | `HostedTelemetryConsentGrant`, `HostedTelemetryEndpointDescriptor`, `HostedTelemetryExportBatch` |
| Release pipeline | `xtask/src/release_pipeline.rs` | `ReleasePipelineConfig`, `InstallerDescriptor`, `VersionStamp`, `VerificationReport` |
| Release pipeline config example | `xtask/release-pipeline.example.toml` | signing posture, installer targets, manifest format |
| xtask gates | `xtask/src/main.rs` | line 72: "Release readiness: Security, privacy, operations, rollback, canary, incident, and supply-chain signoff complete."; lines 282, 410, 323: phase-8 evidence wiring |
| Phase-8 evidence (current) | `plans/evidence/phase-8/storage-migration-recovery-tests.txt` | `cargo test -p legion-storage migration_registry` passes |
| Rollback decision record | `plans/evidence/gui-productization/phase-8-update-rollback-incident.md` | rollback triggers and incident checklist |
| M6 acceptance gate | `plans/evidence/production/M6/M6-milestone-acceptance.md` | HEAD `dc92004`, all required gates pass |
| PR-REL-001 readiness | `plans/product-readiness-ledger.md` | "Packaging, licensing, release" row; status `In progress` |
| Phase status | `plans/phase-status-ledger.md` | Phase 8 substrate accepted; GUI productization evidence owned by the post-substrate track |

## 3. User save-data forward compatibility (v7.x → v8.0.0)

### 3.1 Forward compatibility contract

A v7.x workspace boots into v8.0.0 **without** an interactive migration
wizard **if and only if** the on-disk schema satisfies the v8 registry's
`from_schema_version == v7_active_schema_version` precondition. If the
precondition fails, the v8 build falls through to the backfill branch
in §5.

### 3.2 Storage subsystems covered

The migration registry must register an explicit forward-only step for
every subsystem that touches user save data. The v8.0.0 registry
includes steps for:

- `subsystem_id = "legion.session"` (`SessionRecord`,
  `WorkspaceSessionRecord` in `legion-storage`)
- `subsystem_id = "legion.workspace_config"` (`WorkspaceConfigRecord`,
  `WorkspaceConfigSnapshot` in `legion-protocol`)
- `subsystem_id = "legion.dock_layout"` (`DockLayoutStorageRecord`)
- `subsystem_id = "legion.trust_decision"` (`TrustDecisionRecord`,
  `TrustRecord`)
- `subsystem_id = "legion.file_metadata"` (`FileMetadataRecord`)
- `subsystem_id = "legion.plugin_storage"` (plugin host manifest, app-owned)
- `subsystem_id = "legion.raw_source_vault"` (`RawSourceRetentionLease`
  envelope via `legion-retention`)
- `subsystem_id = "legion.telemetry_spool"` (`HostedTelemetrySpoolRecord`
  via `legion-telemetry`)

Each step declares:

- `migration_id`: stable string, e.g. `"v7-to-v8-session-record-shape"`
- `subsystem_id`: matches one of the strings above
- `from_schema_version`: the v7-active schema version
- `to_schema_version`: the v8-active schema version (target)
- `schema_version`: the registry schema version (`1` for v8.0.0)

The registry rejects empty `migration_id` / `subsystem_id` and any
non-monotonic `from < to` pair at registration time
(`crates/legion-storage/src/lib.rs:315`), so the registry cannot ship
with a step that does not advance the schema forward.

### 3.3 In-place upgrade flow

For each subsystem the v8 build executes the following sequence, which
is the same shape `StorageMigrationRegistry::dry_run` /
`backup_file` / `recover_from_backup` model on disk today:

1. **Detect** — read the existing artifact and parse the
   `StorageSchemaManifest`. If the manifest is missing the parser falls
   back to a v7 default-shape manifest pinned in the registry for that
   subsystem.
2. **Dry-run** — produce a `StorageMigrationDryRunReport` with
   `correlation_id`, `causality_id`, `event_sequence`, and `metadata_summary`.
   The dry-run report is validated by
   `validate_storage_migration_dry_run_report` before the user is shown
   any prompt or any mutation is staged.
3. **Backup** — write a `StorageBackupMarker` to
   `<workspace>/.legion/storage/backups/backup-<cid>-<cuid>.json`. The
   marker carries a `StorageChecksum` whose `algorithm` is the registry
   algorithm constant `legion-storage-stable-sum-v1`; an algorithm
   mismatch fails closed.
4. **Apply** — apply the migration step and atomically rename the new
   artifact into place. Non-atomic fallbacks are intentionally disabled
   (`AGENTS.md` line 11).
5. **Verify** — re-read the artifact and re-validate against the v8
   schema. If validation fails, the v8 build triggers the
   `recover_from_backup` path with the freshly written
   `StorageBackupMarker` and an explicit `StorageRepairRequest`.

### 3.4 Save semantics

The save flow stays proposal-mediated and unchanged for v8.0.0:

- `AppComposition::save_active_buffer` (`crates/legion-app/src/lib.rs:10430`)
  → `SaveWorkflowService` → `WorkspaceActor::save_file_with_proposal`
  (`crates/legion-app/src/lib.rs:10525`).
- Stale / conflict / denial returns `Ok(AppSaveOutcome::Rejected(_))`
  while preserving dirty editor text.
- Workspace saves still require the expected fingerprint, file content
  version, workspace generation, buffer version, snapshot id, and
  non-zero correlation / causality ids.

The migration policy does **not** alter this surface. v7.x sessions
that were opened mid-edit boot into v8.0.0 with the dirty buffer text
preserved in memory and the migration registry applies only to the
persisted record set.

## 4. Server-side data shape changes

### 4.1 Versioned schema contract

Every persisted schema in v8.0.0 carries an explicit `schema_version`
field. `legion_protocol::validate_*` enforces:

- `schema_version > 0`.
- Required fields are non-empty (string ids, correlation ids,
  causality ids).
- `RedactionHint::MetadataOnly` is the default for every migration
  report.

If any validator fails, the v8 build rejects the artifact and routes
through the backfill branch in §5. The on-disk validator never silently
coerces a schema; corrupt records go to quarantine, not to live state.

### 4.2 Dual-write window

The dual-write window covers the period from v8.0.0 GA through the
v8.0.0 + 90 day cutover. During this window the v8 build produces both
the v7 protocol bytes and the v8 protocol bytes for the following
records:

- `SessionRecord` (`workspace_id`, `workspace_path`, `trust_state`)
- `WorkspaceConfigSnapshot`
- `DockLayoutStorageRecord` (`workspace_id`, `mode`, `side`,
  `pinned_default_panel_id`, `custom_toolkit_panel_ids`)
- `RawSourceVaultEnvelope` (metadata-only; raw-source bytes stay under
  the encrypted vault and are not dual-written)
- `HostedTelemetrySpoolRecord` (only when `HostedTelemetryConsentGrant`
  is in scope; consent is verified per `validate_hosted_telemetry_consent_grant`)

Dual-write happens in the storage port adapter
(`crates/legion-storage/src/lib.rs` `save_session`, `save_workspace_config`,
etc.). The v7 bytes are written to a sidecar file
(`<artifact>.v7.json`) and never block on the v8 write; if the v8 write
fails the v7 sidecar stays valid and the operator can roll back to v7
without losing recent edits.

### 4.3 Dual-write exit criteria

Dual-write is removed once **all** of the following are true:

- ≥ 90 days have elapsed since v8.0.0 GA.
- Crash-free session rate from staging telemetry is ≥ 99.5 % across the
  most recent 14-day window (see `rollback-policy.md` §3 for the
  comparable threshold).
- The migration registry has logged zero `recover_from_backup` outcomes
  attributable to dual-write divergence in the most recent 30-day
  window.
- A signed v8.1.0 release artifact exists with `signer_status` ≠
  `unsigned-beta/no-production-signer` or with an explicit
  `unsigned_beta_reason` recorded in `xtask/release-pipeline.example.toml`.

The exit is recorded as a Phase 8 follow-on ledger update and a new
v8.x release checklist; this migration policy is not edited to remove
the dual-write section.

## 5. Backfill plan

### 5.1 When backfill triggers

Backfill is the branch the migration registry falls through to when
the on-disk artifact cannot be detected, parsed, or forward-migrated
in place. The v8.0.0 build triggers backfill in any of the following
cases:

- `StorageSchemaManifest` parse fails (`active_schema_version` missing
  or non-positive).
- A registered `StorageMigrationStep` cannot be found that matches
  `subsystem_id` + `from_schema_version` + `to_schema_version`.
- The dry-run report's `compatible` field is `false`
  (`target_schema_version < active_schema_version`).
- The `StorageBackupMarker` checksum does not match
  `legion-storage-stable-sum-v1` after a backup write.
- The `validate_storage_recovery_outcome` validator rejects a recovery
  payload.

### 5.2 Backfill flow

1. **Quarantine** — copy the artifact to
   `<workspace>/.legion/storage/quarantine/<artifact>-<cid>.json` and
   record a `StorageBackupMarker` with the quarantine path. The artifact
   is removed from the live directory.
2. **Backfill** — read the legacy parser for the matching
   `subsystem_id` and re-emit the v8 schema. The legacy parser must be
   a registered v7-shape parser kept in the v8 registry exactly for this
   case; it is **not** a runtime fallback for normal upgrades.
3. **Validate** — pass the backfilled artifact through the v8
   `validate_*` suite. Failures are returned as
   `StorageMigrationApplyOutcome` with `recovery.status = "failed"`
   and a non-zero `error_code`.
4. **User-visible message** — the v8 build surfaces a single
   non-modal banner that points at the affected subsystem and the
   quarantine file path. The banner never reads the artifact body; the
   user can copy the path and the error code without seeing raw
   content.
5. **Recovery handoff** — the user is asked to confirm
   `recover_from_backup` if a `StorageBackupMarker` is available.
   Recovery is opt-in; the v8 build never auto-recovers from a
   quarantine marker.

### 5.3 Backfill observability

Every backfill invocation writes a metadata-only telemetry event keyed
on `subsystem_id`, `from_schema_version`, `to_schema_version`,
`correlation_id`, `causality_id`. Backfill telemetry is metadata-only
and never includes artifact body bytes, prompts, or provider payloads
(`AGENTS.md` line 14: observability sinks default to metadata-only
redaction and reject zero `CorrelationId`, nil `CausalityId`, or zero
`EventSequence`).

## 6. Cutover date and gating

### 6.1 Cutover date

- **v8.0.0 GA cutover**: 2026-07-15 (target). The cutover fires only
  if the release checklist is fully green and
  `stable-channel-approval.md` is signed (see `release-checklist.md`
  Go/No-Go gates and `stable-channel-approval.md` §3 for the approval
  form).
- **Dual-write end**: 2026-10-13 (GA + 90 days), conditional on §4.3.
- **v8.0.x final**: 2027-01-13 (GA + 180 days).
- **v8 EOL notice**: 2027-04-13 (GA + 270 days).

### 6.2 Gating

The cutover is gated by:

- All items in `release-checklist.md` reach `Status = Done` with an
  Owner signature.
- All `cargo test -p legion-storage migration_*` regression tests
  pass (`plans/evidence/phase-8/storage-migration-recovery-tests.txt`
  is the current evidence baseline).
- `cargo run -p xtask -- release-pipeline --dry-run --channel stable`
  produces `target/release-pipeline/version_stamp.toml` and a
  `verify_report.toml` whose `summary.failed == 0`.
- `cargo run -p xtask -- check-deps` passes
  (`plans/evidence/phase-8/xtask-check-deps.txt`).
- `cargo run -p xtask -- docs-hygiene` passes.
- `cargo fmt --all --check`, `cargo check --workspace --all-targets`,
  `cargo clippy --workspace --all-targets -- -D warnings` all pass.
- M-gate acceptance for the cutover milestone is signed
  (`plans/evidence/production/M*/M*-milestone-acceptance.md`).

## 7. Decision owners

| Decision | Owner | Backup |
| --- | --- | --- |
| Migration registry shape (subsystems, schema versions) | Storage lead (`@storage-owner`) | Retention lead |
| Save-flow invariants (`SaveWorkflowService`, `save_file_with_proposal`) | App composition lead (`@app-owner`) | Editor lead |
| Dual-write window and exit criteria | Release manager (`@release-manager`) | GA approver (see `stable-channel-approval.md`) |
| Raw-source retention migration | Retention lead (`@retention-owner`) | Security lead |
| Telemetry migration (consent, spool, export) | Telemetry lead (`@telemetry-owner`) | Privacy officer |
| Cutover date and rollback gating | Release manager | GA approver |
| Approver of record for the migration policy | `@dasbl` (per `stable-channel-approval.md`) | OTR lead |

## 8. Approval sign-off

This policy is binding on the v8.0.0 GA cut. No v8.0.0 build may be
promoted to the stable channel without a signed approval block in
`stable-channel-approval.md` that explicitly references this document
and `rollback-policy.md`.

```
APPROVAL — Legion IDE v8.0.0 Migration Policy

Migration policy review (Approver):
  Name:  __________________________
  Role:  __________________________
  Date:  __________________________ (YYYY-MM-DD)

Backup approver (Storage lead):
  Name:  __________________________
  Date:  __________________________ (YYYY-MM-DD)

Cutover approver (Release manager):
  Name:  __________________________
  Date:  __________________________ (YYYY-MM-DD)

Sign-off references:
  - docs/releases/v8.0.0/release-checklist.md  (cross-link)
  - docs/releases/v8.0.0/rollback-policy.md    (cross-link)
  - docs/releases/v8.0.0/handoff.md            (cross-link)
  - docs/releases/v8.0.0/stable-channel-approval.md  (cross-link)
  - plans/product-readiness-ledger.md          (PR-REL-001 row)
  - plans/evidence/production/M6/M6-milestone-acceptance.md  (current gate)

Decision:
  [ ] Approved — proceed with v8.0.0 cutover on the scheduled date
  [ ] Approved with conditions — see comment block
  [ ] Blocked — see comment block
```
