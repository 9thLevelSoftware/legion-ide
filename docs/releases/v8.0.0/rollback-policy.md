# Legion IDE v8.0.0 — Rollback Policy

> Companion to `release-checklist.md`, `migration-policy.md`, and
> `handoff.md` under this directory. This document governs the
> decision authority, branch naming, telemetry steps, and maximum
> rollback window for the v8.0.0 GA cut. The rollback surface is
> shaped by the existing `StorageMigrationRegistry::recover_from_backup`
> path, the `phase-8-update-rollback-incident.md` decision record, and
> the `xtask release-pipeline` release-descriptor model.

| Field | Value |
| --- | --- |
| Release | v8.0.0 |
| Authoritative source tree | `9thLevelSoftware/legion-ide` |
| Current HEAD at policy draft | `dc92004` (M6 acceptance gate) |
| Companion docs | [`release-checklist.md`](./release-checklist.md), [`migration-policy.md`](./migration-policy.md), [`handoff.md`](./handoff.md), [`stable-channel-approval.md`](./stable-channel-approval.md) |
| Maximum rollback window | 90 days from v8.0.0 GA |
| Last reviewed | 2026-06-22 |
| Review cadence | Re-validated on every rollback event and every M-gate transition |

## 1. Scope and goals

### 1.1 What "rollback" means for v8.0.0

Rollback for v8.0.0 means: **revert the user's installed build to a
known-good previous channel, recover on-disk save data from
`StorageBackupMarker` records the migration registry wrote during the
forward upgrade, and re-pin the release descriptor to the last
green pipeline output.** Rollback is operator-driven; the user never
runs the rollback commands directly.

### 1.2 Goals

- **Recoverable** — every install that booted v8.0.0 GA can be rolled
  back to v7.x without losing in-flight edits that were saved on the
  v8 sidecar.
- **Bounded blast radius** — a rollback never crosses into the
  raw-source retention vault boundary (the vault envelope keeps a
  distinct `schema_version` and is recovered on its own ledger).
- **Traceable** — every rollback writes a metadata-only incident
  record into the storage migration registry and a corresponding event
  into the telemetry spool (if the install is consented to telemetry).

### 1.3 Non-goals

- Patch-backporting v8 regressions into v7 (that is a v7.x release
  task, not a rollback).
- Auto-recovery of quarantine artifacts (`StorageMigrationApplyOutcome`
  with `recovery.status = "failed"`); the operator must explicitly
  resolve quarantine through `recover_from_backup`.
- Multi-host coordinated rollback; v8.0.0 collaboration and remote
  transports are post-GA (see `plans/product-readiness-ledger.md`
  PR-ENT-001 / PR-ENT-002).

## 2. Authoritative source citations

| Topic | Source | Symbol / Line |
| --- | --- | --- |
| Recovery primitive | `crates/legion-storage/src/lib.rs` | `StorageMigrationRegistry::recover_from_backup` (line 406); `StorageRepairRequest`, `StorageRecoveryOutcome`, `StorageBackupMarker` |
| Backup primitive | `crates/legion-storage/src/lib.rs` | `StorageMigrationRegistry::backup_file` (line 370); `STORAGE_CHECKSUM_ALGORITHM` (line 48) |
| Forward-only invariant | `crates/legion-storage/src/lib.rs` | `StorageMigrationRegistry::register` (line 315); rollback cannot reverse a forward step in place — only via `recover_from_backup` from a backup marker |
| Rollback decision record | `plans/evidence/gui-productization/phase-8-update-rollback-incident.md` | rollback triggers, incident checklist, privacy and safety notes |
| Recovery regression tests | `plans/evidence/phase-8/storage-migration-recovery-tests.txt` | `cargo test -p legion-storage migration_registry` (passed); dry-run, backup, recovery, checksum |
| Operational health surface | `plans/evidence/phase-8/operational-health-diagnostics.txt` | storage migration dry-run / backup / recovery outcome metadata; transport / terminal / telemetry / vault summaries |
| Hosted telemetry spool (rollback telemetry) | `crates/legion-telemetry/src/lib.rs` | `HostedTelemetrySpoolRecord`, `HostedTelemetryUploadOutcome` |
| Raw-source vault (boundary) | `crates/legion-retention/src/lib.rs` | `RawSourceRetentionPolicy`, `RawSourceRetentionConsentGrant`, `validate_raw_source_vault_envelope` |
| Release pipeline (descriptor re-pin) | `xtask/src/release_pipeline.rs` | `plan_release_pipeline`, `verify_descriptors`, `write_descriptors`, `VERSION_STAMP_FILE`, `VERIFY_REPORT_FILE` |
| Release pipeline config | `xtask/release-pipeline.example.toml` | signing posture, installer targets, updater manifest format |
| Phase-gate wiring | `xtask/src/main.rs` | line 72 release-readiness statement; line 282 / 323 / 410 phase-8 evidence hooks |
| Crash-rate telemetry | `plans/evidence/phase-8/` | operational health diagnostics (`operational-health-diagnostics.txt`), fault drills (`fault-drill-results.txt`), retention lifecycle (`raw-source-retention-lifecycle-tests.txt`) |
| PR-REL-001 readiness row | `plans/product-readiness-ledger.md` | current `In progress`; "Signed installers, auto-update/rollback, and crash-report controls are not yet validated." |

## 3. Rollback triggers

A rollback fires when **any** of the following is observed and
attributed to the v8.0.0 GA build. Triggers are evaluated in order;
the first hit halts promotion and opens an incident.

### 3.1 Sev-1 regression

A sev-1 regression is any build behaviour that:

- Causes silent data loss in user save data (any `SessionRecord`,
  `WorkspaceConfigSnapshot`, `DockLayoutStorageRecord`, plugin storage
  manifest, or raw-source vault envelope written by v8.0.0 that cannot
  be re-read on the same v8.0.0 build without operator intervention).
- Bypasses the proposal-mediated save flow
  (`crates/legion-app/src/lib.rs:3113` rejects generic save apply;
  the rollback fires if v8.0.0 emits a non-proposal save).
- Skips `StorageMigrationRegistry::dry_run` and writes a migration
  outcome without a recorded dry-run report.
- Bypasses `legion_protocol::validate_*` for any persisted schema.

### 3.2 Crash rate

The crash-free session rate over any rolling 24-hour window from
staging telemetry drops below the threshold published in
`release-checklist.md` §"Crash-free session threshold from staging".
The threshold currently mirrors the v8 staging baseline and is
re-baselined on every M-gate acceptance (`plans/evidence/production/M*`).

### 3.3 Data corruption

Any of:

- A `StorageBackupMarker` checksum returns a value that does not match
  `legion-storage-stable-sum-v1`
  (`crates/legion-storage/src/lib.rs:48`).
- A `StorageMigrationApplyOutcome` records `recovery.status = "failed"`
  for ≥ 0.5 % of installations in any rolling 24-hour window.
- A `RawSourceVaultEnvelope` round-trip through
  `validate_raw_source_vault_envelope` fails after a successful v8
  forward migration.
- Any `HostedTelemetrySpoolRecord` written under
  `HostedTelemetryConsentGrant` cannot be re-read on the same build.

### 3.4 Audit / privacy regressions

Any of:

- Diagnostics contain raw source, dirty buffer text, prompts, provider
  payloads, terminal output bodies, remote transport frames, secrets,
  or private keys (from `phase-8-update-rollback-incident.md`).
- A signed release claim cannot be matched to signer, checksum, and
  verification evidence (`release-pipeline.example.toml` signer
  status labels).
- The release pipeline dry-run produces a descriptor whose
  `signer_status` does not match the `xtask/release-pipeline.example.toml`
  posture for the current channel.

## 4. Decision authority

### 4.1 Primary rollback authority

The Release Manager (`@release-manager`) is the primary rollback
authority. The Release Manager signs the rollback decision in
`stable-channel-approval.md` §"Rollback decision log" within 60 minutes
of the trigger observation.

### 4.2 Backup approver chain

If the Release Manager is unavailable within the 60-minute window,
authority passes in order:

1. Storage lead (`@storage-owner`)
2. App composition lead (`@app-owner`)
3. OTR lead (`@otr-lead`)
4. The Approver of Record (`@dasbl`) per `stable-channel-approval.md`

The pass is recorded in the rollback decision log with a timestamp and
the reason the primary was bypassed.

### 4.3 Required sign-offs

A rollback decision log entry must be co-signed by:

- The Storage lead (data-recovery feasibility).
- The Release Manager (channel re-pin authorization).
- The Approver of Record (`@dasbl`) for any rollback that crosses the
  14-day window from v8.0.0 GA, or any rollback that affects a
  collaborative or remote-workflow install.

## 5. Hotfix branch naming

### 5.1 Naming convention

Hotfix branches for v8.0.0 follow the convention:

```
hotfix/v8.0.0-<scope>-<short-token>
```

Where:

- `<scope>` is one of `rollback`, `migration`, `security`, `telemetry`,
  `save`, `storage`, `release`.
- `<short-token>` is a kebab-case, ≤ 32-character summary, e.g.
  `migration-session-record-shape`.

### 5.2 Branch lineage

- The hotfix branch is cut from the v8.0.0 GA tag (`v8.0.0`) and
  rebased onto the latest `main` only when the GA branch is no longer
  fast-forward safe.
- Hotfix branches land through the standard PR flow with two required
  reviews: the subsystem owner and the Approver of Record (`@dasbl`).
- The hotfix branch name is referenced verbatim in the rollback
  decision log and in `handoff.md` under the rollback section.

### 5.3 Release artifact for the hotfix

The hotfix build is re-tagged as `v8.0.1` (or `v8.0.1-hotfix.N` if
multiple hotfixes land within the rollback window). The release
pipeline re-runs `cargo run -p xtask -- release-pipeline --dry-run
--channel stable` and `cargo run -p xtask -- verify-release-pipeline`
against the hotfix branch; the resulting `verify_report.toml`
`summary.failed` must be `0` before the hotfix is promoted to the
stable channel.

## 6. Telemetry rollback steps

Rollback instrumentation is metadata-only and never records raw
buffer text, prompts, provider payloads, secrets, or private keys
(`AGENTS.md` line 14). The rollback sequence is:

1. **Trigger capture** — at the moment the rollback decision is signed,
   the v8.0.0 build emits a `HostedTelemetrySpoolRecord` keyed on
   `rollback_decision` with the rollback reason, the affected
   subsystem (`migration`, `save`, `storage`, `telemetry`, etc.),
   the affected installer target, the correlation / causality ids,
   the `EventSequence`, and the rollback authority chain
   (excluding raw names; only role labels are recorded).
2. **Pre-rollback snapshot** — the storage migration registry
   invokes `backup_file` against every subsystem touched by the
   rollback, writing one `StorageBackupMarker` per subsystem to
   `<workspace>/.legion/storage/backups/`. Each marker carries
   `correlation_id`, `causality_id`, and `event_sequence` so the
   pre-rollback state is fully reconstructable.
3. **Channel re-pin** — the release pipeline re-runs against the
   last green channel; the resulting `version_stamp.toml` and
   `verify_report.toml` are archived under
   `plans/evidence/release/<rollback-id>/`.
4. **Recovery invocation** — for each affected subsystem, the v7.x
   build re-reads the matching `StorageBackupMarker` and calls
   `recover_from_backup` with an explicit `StorageRepairRequest`.
   Recovery is logged with a `StorageRecoveryOutcome` carrying the
   correlation / causality ids.
5. **Post-rollback snapshot** — the v7.x build runs
   `StorageMigrationRegistry::dry_run` against the recovered
   artifacts to confirm the v7 active schema version. The resulting
   `StorageMigrationDryRunReport` is archived with the rollback
   evidence bundle.
6. **Verification** — `cargo test -p legion-storage migration_*`
   passes; `cargo run -p xtask -- check-deps` passes; the
   `plans/evidence/phase-8/storage-migration-recovery-tests.txt`
   markers are re-emitted as part of the rollback evidence bundle.

## 7. Maximum rollback window

### 7.1 Window length

The maximum rollback window for any v8.0.0 GA install is **90 days
from the v8.0.0 GA cutover date** declared in `migration-policy.md`
§6.1. After the 90-day window:

- The dual-write sidecar format is removed (§4.3 of the migration
  policy).
- The `StorageBackupMarker` set written by the migration registry is
  retained for an additional 90 days, then pruned by the storage
  retention sweep.
- Any user that has not rolled back by the window end must upgrade to
  v8.1.0 or later to continue receiving security patches.

### 7.2 Window extension

The Approver of Record (`@dasbl`) may extend the rollback window by
one additional 30-day period under all of the following conditions:

- A sev-1 regression is in flight and a hotfix is staged.
- The hotfix cannot land within the original 90-day window without
  crossing the dual-write removal date.
- The extension is recorded in `stable-channel-approval.md` and
  cross-linked from `release-checklist.md` with the new
  rollback-expiration timestamp.

Only one extension is permitted per GA cut. A second sev-1 regression
during the extension escalates to a v8.1.0 cutover rather than a
further window extension.

### 7.3 Post-window commitments

After the rollback window closes, the v8.0.0 build continues to ship
security patches via `v8.0.x` until the v8 EOL date declared in
`migration-policy.md` §6.1 (GA + 270 days). The migration policy's
v8 EOL path does not require an active rollback window because the
forward-only invariant makes downgrades unsafe after dual-write is
removed; users past the rollback window must move forward.

## 8. Decision owners

| Decision | Owner | Backup |
| --- | --- | --- |
| Rollback trigger evaluation | Release manager | OTR lead |
| Channel re-pin authorization | Release manager | Approver of Record (`@dasbl`) |
| Data-recovery feasibility (subsystem coverage) | Storage lead | Retention lead |
| Save-flow regressions | App composition lead | Editor lead |
| Telemetry-spool regressions | Telemetry lead | Privacy officer |
| Hotfix branch lineage | Release manager | Storage lead |
| Rollback window extension | Approver of Record (`@dasbl`) | OTR lead |
| Incident response record | Release manager | Privacy officer |

## 9. Approval sign-off

This policy is binding on the v8.0.0 GA cut. No v8.0.0 build may be
promoted to the stable channel without a signed approval block in
`stable-channel-approval.md` that explicitly references this document
and `migration-policy.md`.

```
APPROVAL — Legion IDE v8.0.0 Rollback Policy

Rollback policy review (Approver):
  Name:  __________________________
  Role:  __________________________
  Date:  __________________________ (YYYY-MM-DD)

Storage lead (data-recovery feasibility):
  Name:  __________________________
  Date:  __________________________ (YYYY-MM-DD)

Release manager (channel re-pin authority):
  Name:  __________________________
  Date:  __________________________ (YYYY-MM-DD)

Approver of Record (window extension / escalation):
  Name:  __________________________
  Date:  __________________________ (YYYY-MM-DD)

Sign-off references:
  - docs/releases/v8.0.0/release-checklist.md     (cross-link)
  - docs/releases/v8.0.0/migration-policy.md      (cross-link)
  - docs/releases/v8.0.0/handoff.md               (cross-link)
  - docs/releases/v8.0.0/stable-channel-approval.md   (cross-link)
  - plans/evidence/gui-productization/phase-8-update-rollback-incident.md  (current triggers)
  - plans/evidence/phase-8/storage-migration-recovery-tests.txt          (recovery regression)
  - plans/product-readiness-ledger.md             (PR-REL-001 row)

Decision:
  [ ] Approved — proceed with v8.0.0 cutover on the scheduled date
  [ ] Approved with conditions — see comment block
  [ ] Blocked — see comment block
```
