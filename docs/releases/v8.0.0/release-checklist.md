# Legion IDE v8.0.0 — GA Release Checklist & Freeze Criteria

> **STATUS: FORWARD-LOOKING TEMPLATE — NOT A CURRENT RELEASE ARTIFACT.** No v8.0.0 release exists or is scheduled. The workspace version is 0.1.0 and the product is pre-beta; see `plans/product-readiness-ledger.md` for current status. CI references in this document (e.g. `.github/workflows/ci.yml`) describe a future pipeline that is not yet configured, and some referenced companion documents (e.g. `handoff.md`, `stable-channel-approval.md`) do not exist yet.

> Companion to [`migration-policy.md`](./migration-policy.md),
> [`rollback-policy.md`](./rollback-policy.md), [`handoff.md`](./handoff.md), and
> [`stable-channel-approval.md`](./stable-channel-approval.md) under this
> directory. This document is the **executable** GA checklist for the
> v8.0.0 cut: every item below is a checkbox with an explicit Owner and
> Status, sourced to a commit hash, doc section, ticket id, or command.
>
> No v8.0.0 build may be promoted to the stable channel until every box
> is `[x]` with an Owner signature and Status `Done`, the Go/No-Go gates
> in §9 are all `GO`, and `stable-channel-approval.md` is signed.

| Field | Value |
| --- | --- |
| Release | v8.0.0 (workspace version bump from `0.1.0` → `8.0.0`) |
| Authoritative source tree | `9thLevelSoftware/legion-ide` |
| Current HEAD at checklist draft | `dc92004` (M6 acceptance gate, `plans/evidence/production/M6/M6-milestone-acceptance.md`) |
| Tag trigger | `v8.0.0` (matches CI `release` job `if: startsWith(github.ref, 'refs/tags/v')`, `.github/workflows/ci.yml:166`) |
| Companion docs | [`migration-policy.md`](./migration-policy.md), [`rollback-policy.md`](./rollback-policy.md), [`handoff.md`](./handoff.md), [`stable-channel-approval.md`](./stable-channel-approval.md) |
| Supersedes | None (first GA cut with an explicit release checklist) |
| Last reviewed | 2026-06-22 |
| Review cadence | Re-validated on every GA-tag `v8.*.*` and on every M-gate transition |

## Owner convention

Each row names an Owner role. The decision-of-record approver for the
whole checklist is named in `stable-channel-approval.md` §3.

| Owner role | Source |
| --- | --- |
| `@release-manager` | `docs/releases/v8.0.0/migration-policy.md` §7 |
| `@storage-owner` | `docs/releases/v8.0.0/migration-policy.md` §7 |
| `@retention-owner` | `docs/releases/v8.0.0/migration-policy.md` §7 |
| `@telemetry-owner` | `docs/releases/v8.0.0/migration-policy.md` §7 |
| `@app-owner` | `docs/releases/v8.0.0/migration-policy.md` §7 |
| `@security-lead` | `docs/SECURITY.md`; `plans/evidence/security/P9-F2-T4-external-audit-gate.md` |
| `@privacy-officer` | `docs/SECURITY.md` |
| `@desktop-lead` | `plans/evidence/gui-productization/phase-8-final-gates.md` |
| `@platform-lead` | `plans/evidence/gui-productization/phase-8-platform-parity.md` |
| `@supply-chain-lead` | `deny.toml`; CI `EmbarkStudios/cargo-deny-action@v2` |
| `@docs-lead` | `docs/INDEX.md`; `docs/hygiene-allowlist.toml` |
| `@ga-approver` | `docs/releases/v8.0.0/stable-channel-approval.md` (approver of record) |

Status vocabulary: `Todo` → `In progress` → `Blocked` → `Done` → `Verified`.
A row cannot move to `Done` without an Owner signature and a source
citation; a row cannot move to `Verified` without passing the §9 gate.

---

## 1. Code-freeze trigger

Freeze fires when **all** of §1.1–§1.5 are `Done`. Once §1.6 is signed
no further changes land on `main` except as approved cherry-picks via
§1.7.

### 1.1 Freeze trigger criteria

- [ ] **Owner:** `@release-manager` — **Status:** `Todo`
  Cut branch `release/v8.0.0` from `main` at HEAD `dc92004` and tag it
  `v8.0.0-rc1`. CI `release` job triggers on `v*.*.*` tags
  (`.github/workflows/ci.yml:166,175`).
- [ ] **Owner:** `@release-manager` — **Status:** `Todo`
  All `M6` gates pass on the release branch:
  `cargo run -p xtask -- check-deps`, `cargo fmt --all --check`,
  `cargo check --workspace --all-targets`,
  `cargo test --workspace --all-targets --no-fail-fast`,
  `cargo clippy --workspace --all-targets -- -D warnings`
  (`plans/evidence/production/M6/M6-milestone-acceptance.md`).
- [ ] **Owner:** `@docs-lead` — **Status:** `Todo`
  `docs/releases/v8.0.0/{release-checklist,migration-policy,rollback-policy,handoff,stable-channel-approval}.md`
  all exist and link to each other (this document + the three siblings
  already on disk; `handoff.md` and `stable-channel-approval.md` are
  pending).
- [ ] **Owner:** `@supply-chain-lead` — **Status:** `Todo`
  `cargo deny check` is green on the release branch at HEAD `dc92004`
  with the documented warning-level duplicate-dependency baseline
  (`plans/evidence/gui-productization/phase-8-final-gates.md` §Required
  Commands; CI leg `.github/workflows/ci.yml:160`).
- [ ] **Owner:** `@release-manager` — **Status:** `Todo`
  PR-REL-001 readiness row status changes from `In progress` to
  `Product workflow validated` only after the §9 Go gate fires
  (`plans/product-readiness-ledger.md` row PR-REL-001).

### 1.2 Code-freeze window

- [ ] **Owner:** `@release-manager` — **Status:** `Todo`
  Declare a code freeze in `plans/phase-status-ledger.md` and on the
  kanban board (`t_486a9b50` thread); freeze blocks merges to `main`
  except for the §1.7 cherry-pick lane.
- [ ] **Owner:** `@release-manager` — **Status:** `Todo`
  Open a freeze-tracking issue linking this checklist and the
  migration / rollback siblings.

### 1.3 Tagging

- [ ] **Owner:** `@release-manager` — **Status:** `Todo`
  Tag the release commit as `v8.0.0` once §9 Go gate fires. The CI
  `release` job (`.github/workflows/ci.yml:165–196`) re-runs both
  channels (stable + preview) on every OS matrix leg.

### 1.4 Branch protection

- [ ] **Owner:** `@release-manager` — **Status:** `Todo`
  Branch protection on `main` requires CI green from
  `.github/workflows/ci.yml` `validate` job before merge.

### 1.5 Cherry-pick lane (§1.7)

- [ ] **Owner:** `@release-manager` — **Status:** `Todo`
  Only blocker fixes (Sev-1 regression, save-flow invariant, security
  CVE) may land post-freeze; each requires `@release-manager` +
  `@security-lead` dual sign-off and a paired cherry-pick PR against
  `main`.

### 1.6 Freeze sign-off

- [ ] **Owner:** `@ga-approver` — **Status:** `Todo`
  Freeze decision signed in `stable-channel-approval.md` §3 referencing
  this checklist, `migration-policy.md`, and `rollback-policy.md`.

---

## 2. Build-green criteria on all target platforms

Every platform matrix leg must end with `verify_report.toml`
`summary.failed == 0` per `xtask/src/release_pipeline.rs:468`.

### 2.1 Local repository gates (all OS)

- [ ] **Owner:** `@release-manager` — **Status:** `Todo`
  `cargo run -p xtask -- check-deps` → pass
  (`xtask/src/main.rs:32` policy path; `plans/evidence/phase-8/xtask-check-deps.txt`).
- [ ] **Owner:** `@docs-lead` — **Status:** `Todo`
  `cargo run -p xtask -- docs-hygiene` → pass (`docs/hygiene-allowlist.toml`).
- [ ] **Owner:** `@desktop-lead` — **Status:** `Todo`
  `cargo run -p xtask -- no-egui-textedit` → pass (`xtask/no-egui-textedit.toml`).
- [ ] **Owner:** `@release-manager` — **Status:** `Todo`
  `cargo run -p xtask -- release-pipeline --dry-run` → emits
  `target/release-pipeline/version_stamp.toml`
  (`xtask/src/release_pipeline.rs:18,335`).
- [ ] **Owner:** `@release-manager` — **Status:** `Todo`
  `cargo run -p xtask -- verify-release-pipeline` →
  `verify_report.toml` reports `summary.failed == 0`
  (`xtask/src/main.rs:757,811`; `xtask/src/release_pipeline.rs:242,468`).
- [ ] **Owner:** `@release-manager` — **Status:** `Todo`
  `cargo fmt --all --check` → pass.
- [ ] **Owner:** `@release-manager` — **Status:** `Todo`
  `cargo check --workspace --all-targets` → pass.
- [ ] **Owner:** `@release-manager` — **Status:** `Todo`
  `cargo test --workspace --all-targets` → pass.
- [ ] **Owner:** `@release-manager` — **Status:** `Todo`
  `cargo clippy --workspace --all-targets -- -D warnings` → pass.
- [ ] **Owner:** `@supply-chain-lead` — **Status:** `Todo`
  `cargo deny check` → pass (warning-level duplicate-dependency
  baseline acceptable per `deny.toml` and
  `plans/evidence/gui-productization/phase-8-final-gates.md`).

### 2.2 Phase-8 GUI evidence gates

- [ ] **Owner:** `@desktop-lead` — **Status:** `Todo`
  `cargo test -p legion-desktop --test plugin_management -- --nocapture` → pass
  (`plans/evidence/gui-productization/phase-8-final-gates.md`).
- [ ] **Owner:** `@desktop-lead` — **Status:** `Todo`
  `cargo test -p legion-desktop --test collaboration_gui -- --nocapture` → pass.
- [ ] **Owner:** `@desktop-lead` — **Status:** `Todo`
  `cargo test -p legion-desktop --test remote_workspace_gui -- --nocapture` → pass.
- [ ] **Owner:** `@desktop-lead` — **Status:** `Todo`
  `cargo test -p legion-desktop --test delegated_task_command_center -- --nocapture` → pass.
- [ ] **Owner:** `@desktop-lead` — **Status:** `Todo`
  `cargo run -p legion-cli -- evidence check --phase gui-phase8` → pass.
- [ ] **Owner:** `@desktop-lead` — **Status:** `Todo`
  `cargo run -p legion-cli -- evidence check --phase phase8` → pass.

### 2.3 Platform matrix proof (Windows / macOS / Linux)

- [ ] **Owner:** `@platform-lead` — **Status:** `Todo`
  Latest `validate` matrix job (Ubuntu, macOS, Windows) on tag
  `v8.0.0-rc1` is green. Reference acceptance evidence:
  `plans/evidence/gui-productization/phase-8-final-gates.md`
  Platform Matrix Proof (GitHub Actions run `26590800830` for the
  prior baseline; the v8.0.0 cut must show a fresh run).
- [ ] **Owner:** `@platform-lead` — **Status:** `Todo`
  `scripts/gui-smoke.sh --phase-8 --dry-run` (Linux/macOS) passes on
  the release tag.
- [ ] **Owner:** `@platform-lead` — **Status:** `Todo`
  `powershell -NoProfile -ExecutionPolicy Bypass -File scripts/gui-smoke.ps1 -Phase8 -DryRun`
  passes on Windows release tag.
- [ ] **Owner:** `@platform-lead` — **Status:** `Todo`
  `scripts/package-windows.ps1 -DryRun` produces
  `target/gui-phase6-package/legion-desktop-package-manifest.txt`
  (`docs/OPERATOR_RUNBOOK.md` §GUI packaging).
- [ ] **Owner:** `@platform-lead` — **Status:** `Todo`
  `phase-8-platform-parity.md` is re-populated with current macOS /
  Linux runner output (currently trimmed per the file's first line;
  the v8.0.0 cut must restore it).

### 2.4 Tagged-release pipeline descriptor plan

- [ ] **Owner:** `@release-manager` — **Status:** `Todo`
  `cargo run -p xtask -- release-pipeline --channel stable --dry-run`
  passes on every CI matrix leg
  (`.github/workflows/ci.yml:165–196`).
- [ ] **Owner:** `@release-manager` — **Status:** `Todo`
  `cargo run -p xtask -- release-pipeline --channel preview --dry-run`
  passes on every CI matrix leg.
- [ ] **Owner:** `@release-manager` — **Status:** `Todo`
  Release pipeline config posture recorded:
  `xtask/release-pipeline.example.toml` reflects either an explicit
  `unsigned-beta` policy or the platform signing labels
  (`docs/OPERATOR_RUNBOOK.md` §Release signer references).

---

## 3. Asset-manifest verification

The release pipeline must produce a stable artifact set + manifest for
every platform. Source: `xtask/src/release_pipeline.rs`,
`docs/OPERATOR_RUNBOOK.md` §Expected artifacts.

### 3.1 Windows package

- [ ] **Owner:** `@release-manager` — **Status:** `Todo`
  `target/gui-phase6-package/legion-desktop.exe` exists with sha256
  recorded in the manifest.
- [ ] **Owner:** `@release-manager` — **Status:** `Todo`
  `target/gui-phase6-package/legion-desktop-package-manifest.txt` lists
  every packaged file with relative path, size, and sha256.

### 3.2 GUI diagnostics exports

- [ ] **Owner:** `@desktop-lead` — **Status:** `Todo`
  `target/gui-phase6-diagnostics.md`, `target/gui-phase7-diagnostics.md`,
  `target/gui-phase8-diagnostics.md` all exist and contain the
  `## Operational Health` section
  (`plans/evidence/gui-productization/phase-7-release-readiness.md`
  Diagnostics signoff row).
- [ ] **Owner:** `@desktop-lead` — **Status:** `Todo`
  `target/gui-phase8-session.json` matches the phase-8 smoke session
  shape and contains metadata only (no raw source, dirty buffer text,
  prompts, provider payloads, terminal output bodies, remote transport
  frames, secrets — `plans/evidence/gui-productization/phase-8-ga-release-runbook.md`
  §Update Path step 4).

### 3.3 Release pipeline descriptor plan

- [ ] **Owner:** `@release-manager` — **Status:** `Todo`
  `target/release-pipeline/version_stamp.toml` carries
  `schema_version`, `package_version`, `channel`, `rollout_policy`,
  `dist_tool`, `git_sha`, `built_at_utc`
  (`AGENTS.md` WS17.T1; `xtask/src/release_pipeline.rs:184`).
- [ ] **Owner:** `@release-manager` — **Status:** `Todo`
  `target/release-pipeline/verify_report.toml` reports
  `summary.failed == 0` with `verifier_status` per descriptor
  (`xtask/src/release_pipeline.rs:242`).

### 3.4 Asset signing posture

- [ ] **Owner:** `@release-manager` — **Status:** `Todo`
  Either (a) explicit `unsigned-beta` posture recorded in
  `xtask/release-pipeline.example.toml`, or (b) signer reference
  resolved through one of `env` / `keyring` / `kms` / `ci-secret`
  (`docs/OPERATOR_RUNBOOK.md` §Release signer references).
- [ ] **Owner:** `@release-manager` — **Status:** `Todo`
  No private keys, certificates, tokens, or notarization credentials
  in the repo (`docs/OPERATOR_RUNBOOK.md` §Operational notes; prior
  scan archive `git log --grep 'signer' --name-only` clean).

---

## 4. Localization parity

Legion does not ship localized UI strings in v8.0.0; parity here means
**the absence of any accidental locale-locked path that would silently
fail on non-en-US systems** plus the public docs supporting every
supported locale marker.

### 4.1 Locale parity absence-of-regression

- [ ] **Owner:** `@desktop-lead` — **Status:** `Todo`
  Grep `crates/legion-ui/`, `crates/legion-desktop/` for any
  hard-coded `en-US`, `LANG=C`, or non-UTF-8 default; none expected
  (`docs/SECURITY.md` §Egress policy is locale-agnostic).
- [ ] **Owner:** `@desktop-lead` — **Status:** `Todo`
  `cargo test -p legion-ui` and `cargo test -p legion-desktop` cover
  any locale-sensitive string formatting paths; test count at HEAD
  `dc92004` matches `plans/product-readiness-ledger.md` row PR-UI-001.

### 4.2 Documentation locale parity

- [ ] **Owner:** `@docs-lead` — **Status:** `Todo`
  `docs/INDEX.md` lists the same set of canonical documents in every
  `docs/releases/v*` series. No partial / draft / out-of-date doc is
  linked from the index (`docs/INDEX.md` §How to use this index).
- [ ] **Owner:** `@docs-lead` — **Status:** `Todo`
  `docs/hygiene-allowlist.toml` is reviewed; only narrow historical
  entries remain
  (`docs/OPERATOR_RUNBOOK.md` §Local verification gates).

### 4.3 Locale marker in release notes

- [ ] **Owner:** `@release-manager` — **Status:** `Todo`
  v8.0.0 release notes in `docs/releases/v8.0.0/handoff.md` explicitly
  state "UI strings: English only; other locales follow in v8.x".

---

## 5. Save-data migration dry-run

Source of truth: [`migration-policy.md`](./migration-policy.md) §3.

### 5.1 Migration registry dry-run

- [ ] **Owner:** `@storage-owner` — **Status:** `Todo`
  `cargo test -p legion-storage migration_registry` passes (3 matching
  tests at `crates/legion-storage/src/lib.rs:2381,2436,2469`;
  evidence baseline
  `plans/evidence/phase-8/storage-migration-recovery-tests.txt`).
- [ ] **Owner:** `@storage-owner` — **Status:** `Todo`
  `StorageMigrationRegistry::register` (line 315) rejects
  `from_schema_version == 0`, `to <= from`, and empty ids; verified by
  the dry-run report test.
- [ ] **Owner:** `@storage-owner` — **Status:** `Todo`
  `StorageMigrationRegistry::dry_run` produces a
  `StorageMigrationDryRunReport` whose `compatible` field is `true`
  for every registered subsystem
  (`crates/legion-storage/src/lib.rs:333`; validated by
  `legion_protocol::validate_storage_migration_dry_run_report`).

### 5.2 Backup / recovery path

- [ ] **Owner:** `@storage-owner` — **Status:** `Todo`
  `StorageMigrationRegistry::backup_file` (line 370) writes a
  `StorageBackupMarker` whose `StorageChecksum.algorithm ==
  "legion-storage-stable-sum-v1"` (`STORAGE_CHECKSUM_ALGORITHM` at
  line 48); mismatch fails closed.
- [ ] **Owner:** `@storage-owner` — **Status:** `Todo`
  `StorageMigrationRegistry::recover_from_backup` (line 406) returns
  a `StorageRecoveryOutcome` validated by
  `legion_protocol::validate_storage_recovery_outcome`.
- [ ] **Owner:** `@storage-owner` — **Status:** `Todo`
  End-to-end dry-run → backup → apply → recover flow exercises every
  subsystem in
  [`migration-policy.md`](./migration-policy.md) §3.2
  (`legion.session`, `legion.workspace_config`, `legion.dock_layout`,
  `legion.trust_decision`, `legion.file_metadata`,
  `legion.plugin_storage`, `legion.raw_source_vault`,
  `legion.telemetry_spool`).

### 5.3 Dual-write window

- [ ] **Owner:** `@storage-owner` — **Status:** `Todo`
  Dual-write of v7 + v8 bytes per
  [`migration-policy.md`](./migration-policy.md) §4.2 is verified on a
  fixture workspace (no live user data); v7 sidecar failure does not
  block v8 write and vice versa.
- [ ] **Owner:** `@storage-owner` — **Status:** `Todo`
  Dual-write exit criteria from [`migration-policy.md`](./migration-policy.md)
  §4.3 are pinned as a follow-on v8.x checklist; the v8.0.0 cut does
  **not** attempt to exit dual-write at GA.

### 5.4 Save-flow invariant preserved

- [ ] **Owner:** `@app-owner` — **Status:** `Todo`
  `AppComposition::save_active_buffer` (`crates/legion-app/src/lib.rs:18122`)
  → `SaveWorkflowService::save_active_buffer` (line 10430) →
  `WorkspaceActor::save_file_with_proposal` (line 10525) chain is
  exercised by the migration dry-run; dirty editor text is preserved
  on stale / conflict / denial (`AGENTS.md` lines 10–11;
  `plans/evidence/gui-productization/phase-8-update-rollback-incident.md`).

---

## 6. Telemetry disabled in shipping build

Default posture: metadata-only, opt-in. Anything beyond metadata-only
must be explicitly consented. Source:
`docs/SECURITY.md` §Egress policy and §Secret handling and retention.

### 6.1 Default posture

- [ ] **Owner:** `@telemetry-owner` — **Status:** `Todo`
  `HostedTelemetryConsentGrant` (`crates/legion-telemetry/src/lib.rs`)
  is the only path that enables hosted telemetry; without an in-scope
  grant the v8.0.0 build cannot emit a `HostedTelemetrySpoolRecord`
  (`crates/legion-protocol/src/lib.rs` validator suite).
- [ ] **Owner:** `@telemetry-owner` — **Status:** `Todo`
  Local observability sinks (`crates/legion-observability`) default to
  metadata-only redaction; zero `CorrelationId`, nil `CausalityId`,
  zero `EventSequence` are rejected (`AGENTS.md` line 14).
- [ ] **Owner:** `@privacy-officer` — **Status:** `Todo`
  `docs/SECURITY.md` §Egress policy is updated to explicitly name the
  v8.0.0 default deny-by-default posture and the consent grant flow.

### 6.2 Air-gap policy

- [ ] **Owner:** `@security-lead` — **Status:** `Todo`
  Manual mode forbids AI, cloud, hosted telemetry, and any
  network-capable AI action (`docs/MODES.md` §Manual).
- [ ] **Owner:** `@security-lead` — **Status:** `Todo`
  Air-gap policy denies non-loopback network access and hosted
  provider invocation (`docs/SECURITY.md` §Egress policy); verified by
  sandbox + egress tests in
  `crates/legion-security/src/lib.rs`.

### 6.3 Redaction layer

- [ ] **Owner:** `@retention-owner` — **Status:** `Todo`
  Redaction layer scans for PEM, AWS keys, OpenAI-style key markers,
  `api_key=` assignments (`docs/SECURITY.md` §Secret handling and
  retention); secret-bearing marker scan at HEAD `41d9b56`
  (`git log --grep 'secret'`).
- [ ] **Owner:** `@retention-owner` — **Status:** `Todo`
  Raw-source retention stays encrypted under
  `RawSourceRetentionPolicy` / `RawSourceRetentionLease`
  (`crates/legion-retention/src/lib.rs`); migration dry-run never
  reads or exports raw bytes (migration-policy.md §1.2).

### 6.4 Release-time verification

- [ ] **Owner:** `@telemetry-owner` — **Status:** `Todo`
  `target/gui-phase8-diagnostics.md` contains no raw prompt text,
  generated diffs, source text, dirty buffer text, provider payloads,
  terminal output bodies, remote transport frames, or secrets
  (`plans/evidence/gui-productization/phase-8-ga-release-runbook.md`
  §Update Path step 4;
  `plans/evidence/gui-productization/phase-8-update-rollback-incident.md`
  §Privacy And Safety Notes).

---

## 7. Crash-free session threshold from staging

The GA cutover requires a **staging crash-free session rate ≥ 99.5 %
across the most recent 14-day window**, mirroring the dual-write exit
criterion in [`migration-policy.md`](./migration-policy.md) §4.3 and
[`rollback-policy.md`](./rollback-policy.md) §3.4.

### 7.1 Threshold definition

- [ ] **Owner:** `@release-manager` — **Status:** `Todo`
  Crash-free session rate definition: sessions without a non-recovered
  `AppCompositionError::Panic`, `AppCompositionError::WorkerExit`, or
  `WorkspaceActor::crash_marker` event over total sessions, normalized
  per user.
- [ ] **Owner:** `@telemetry-owner` — **Status:** `Todo`
  Sampling window: 14 contiguous days ending no earlier than 7 days
  before GA tag.

### 7.2 Staging instrumentation

- [ ] **Owner:** `@telemetry-owner` — **Status:** `Todo`
  Staging build has `HostedTelemetryConsentGrant` issued to the
  staging scope so the count is observable; production scope grant is
  **not** required to be present at GA.
- [ ] **Owner:** `@telemetry-owner` — **Status:** `Todo`
  Crash counter excludes air-gap installs (no egress → no count),
  excluded from the denominator (`docs/SECURITY.md` §Egress policy).

### 7.3 Threshold gate

- [ ] **Owner:** `@release-manager` — **Status:** `Todo`
  Staging crash-free session rate ≥ **99.5 %** across the 14-day
  window preceding v8.0.0-rc1.
- [ ] **Owner:** `@release-manager` — **Status:** `Todo`
  Staging rate is archived at
  `plans/evidence/release/v8.0.0-staging-crash-rate.json` with
  `window_start_utc`, `window_end_utc`, `crash_free_session_rate`,
  `total_sessions`, `crashed_sessions`.

### 7.4 Rollback coupling

- [ ] **Owner:** `@release-manager` — **Status:** `Todo`
  If post-GA staging-equivalent production crash-free rate drops below
  99.5 % in any 7-day rolling window, a §9 No-Go fires and the rollback
  in [`rollback-policy.md`](./rollback-policy.md) §3 activates
  (`docs/OPERATOR_RUNBOOK.md` §Subagent execution pattern step 7).

---

## 8. Explicit Go/No-Go gates with owners

Eight gates. **Every gate must be `GO`** to mark the checklist
complete. Each gate is binary (no "GO with reservations" — reservations
become a separate blocking checklist row).

### 8.1 Gate G1 — Repository health (build-green)

- **Owner:** `@release-manager`
- **Required:** All §2 boxes `Done`; §2.3 matrix run is green on tag
  `v8.0.0-rc1`.
- **Decision:** `[ ] GO` / `[ ] NO-GO`
- **Source citations:** §2.1–§2.4 above;
  `.github/workflows/ci.yml` `validate` and `release` jobs;
  `plans/evidence/gui-productization/phase-8-final-gates.md`.

### 8.2 Gate G2 — Migration dry-run

- **Owner:** `@storage-owner`
- **Required:** §5.1–§5.4 boxes `Done`; storage migration dry-run
  report is compatible for every registered subsystem.
- **Decision:** `[ ] GO` / `[ ] NO-GO`
- **Source citations:** `crates/legion-storage/src/lib.rs:291,315,370,406`;
  [`migration-policy.md`](./migration-policy.md) §3.3;
  `plans/evidence/phase-8/storage-migration-recovery-tests.txt`.

### 8.3 Gate G3 — Telemetry / privacy posture

- **Owner:** `@telemetry-owner` + `@privacy-officer`
- **Required:** §6 boxes `Done`; staging crash-rate telemetry is
  observable only via the staging consent scope
  (`docs/SECURITY.md` §Egress policy).
- **Decision:** `[ ] GO` / `[ ] NO-GO`
- **Source citations:** `docs/SECURITY.md` §Egress policy and
  §Secret handling and retention;
  `crates/legion-telemetry/src/lib.rs`;
  `crates/legion-retention/src/lib.rs`.

### 8.4 Gate G4 — Crash-free session threshold

- **Owner:** `@release-manager` + `@telemetry-owner`
- **Required:** §7.3 box `Done` (staging crash-free ≥ 99.5 %);
  archived evidence at
  `plans/evidence/release/v8.0.0-staging-crash-rate.json`.
- **Decision:** `[ ] GO` / `[ ] NO-GO`
- **Source citations:** §7 above; [`migration-policy.md`](./migration-policy.md) §4.3;
  [`rollback-policy.md`](./rollback-policy.md) §3.4.

### 8.5 Gate G5 — Rollback readiness

- **Owner:** `@release-manager`
- **Required:** [`rollback-policy.md`](./rollback-policy.md) is signed
  off; rollback dry-run on the staging fixture recovers from
  `StorageBackupMarker` cleanly (`recover_from_backup` at line 406).
- **Decision:** `[ ] GO` / `[ ] NO-GO`
- **Source citations:** [`rollback-policy.md`](./rollback-policy.md) §3;
  `crates/legion-storage/src/lib.rs:406`;
  `plans/evidence/gui-productization/phase-8-update-rollback-incident.md`.

### 8.6 Gate G6 — Security / supply-chain

- **Owner:** `@security-lead` + `@supply-chain-lead`
- **Required:** `cargo deny check` green; external audit gate
  (`plans/evidence/security/P9-F2-T4-external-audit-gate.md`) signed;
  no new Sev-1 regressions since M6.
- **Decision:** `[ ] GO` / `[ ] NO-GO`
- **Source citations:** `deny.toml`;
  `audit-reports/external-security-audit-2026-06-13.md`;
  `plans/evidence/security/P9-F2-T4-external-audit-gate.md`.

### 8.7 Gate G7 — Documentation & release index

- **Owner:** `@docs-lead`
- **Required:** `docs/INDEX.md` links this checklist and the v8.0.0
  release folder; `docs/hygiene-allowlist.toml` reviewed;
  `cargo run -p xtask -- docs-hygiene` green.
- **Decision:** `[ ] GO` / `[ ] NO-GO`
- **Source citations:** `docs/INDEX.md`; `docs/OPERATOR_RUNBOOK.md`.

### 8.8 Gate G8 — Stable-channel approval

- **Owner:** `@ga-approver`
- **Required:** `stable-channel-approval.md` is signed and references
  this checklist, [`migration-policy.md`](./migration-policy.md), and
  [`rollback-policy.md`](./rollback-policy.md); approver of record
  named per `stable-channel-approval.md` §3.
- **Decision:** `[ ] GO` / `[ ] NO-GO`
- **Source citations:** [`stable-channel-approval.md`](./stable-channel-approval.md) §3.

---

## 9. Final Go/No-Go

The v8.0.0 cutover fires on 2026-07-15 (target,
[`migration-policy.md`](./migration-policy.md) §6.1) only when **all
eight §8 gates are `GO`** and the approval block in
`stable-channel-approval.md` is signed.

### 9.1 Composite decision

- **Composite Owner:** `@ga-approver`
- **Required:** G1 ∩ G2 ∩ G3 ∩ G4 ∩ G5 ∩ G6 ∩ G7 ∩ G8 = `GO`.
- **Decision:** `[ ] GO` / `[ ] NO-GO`
- **Effective date target:** 2026-07-15
  ([`migration-policy.md`](./migration-policy.md) §6.1)

### 9.2 NO-GO handling

If any gate is `NO-GO`:

1. `@release-manager` opens an incident tied to the failing gate.
2. `@release-manager` halts v8.0.0 promotion and removes the candidate
   from the promoted channel.
3. The failing gate's owner produces a remediation plan with target
   re-eval date; that date is appended to
   `stable-channel-approval.md` as a follow-on entry.
4. Rollback to v7.x follows [`rollback-policy.md`](./rollback-policy.md)
   if the v8 build was already promoted; otherwise the rc tag is
   deleted.
5. Re-run this checklist from §1 before any renewed GA claim.

### 9.3 Cutover log

- [ ] **Owner:** `@release-manager` — **Status:** `Todo`
  On `GO`, append a row to `plans/phase-status-ledger.md` recording
  `v8.0.0 GA accepted at HEAD <sha> on YYYY-MM-DD` with link to the
  signed `stable-channel-approval.md`.
- [ ] **Owner:** `@release-manager` — **Status:** `Todo`
  On `GO`, update `plans/product-readiness-ledger.md` row PR-REL-001
  status to `Product workflow validated` with a pointer to this
  checklist, the migration / rollback siblings, and the staging
  crash-rate evidence.

---

## 10. Post-GA follow-ons (informational, not gates)

These items are tracked outside the §8 gates; they inform the v8.0.x
and v8.1.0 follow-on checklists but do not block the v8.0.0 cut.

- [ ] **Owner:** `@desktop-lead` — **Status:** `Todo`
  Fresh-VM Gatekeeper / SmartScreen install smoke
  (`plans/evidence/release/P8-F1-T3-fresh-vm-gatekeeper-smartscreen-install-smoke.md`)
  is re-run against the v8.0.0 tag and archived.
- [ ] **Owner:** `@security-lead` — **Status:** `Todo`
  External audit follow-up against the v8.0.0 signed installer / build
  artifact is queued; gate note updated in
  `plans/evidence/security/P9-F2-T4-external-audit-gate.md`.
- [ ] **Owner:** `@storage-owner` — **Status:** `Todo`
  Dual-write exit criteria (`migration-policy.md` §4.3) tracked as a
  v8.1.0 checklist row; do not remove the dual-write section from
  `migration-policy.md` until §4.3 is fully satisfied.
- [ ] **Owner:** `@release-manager` — **Status:** `Todo`
  VS Code extension-host sidecar, webviews, notebooks, custom editors,
  extension storage, and marketplace execution (PR-VSC-002) remain
  deferred with explicit cut line (`plans/product-readiness-ledger.md`
  row PR-VSC-002).
- [ ] **Owner:** `@release-manager` — **Status:** `Todo`
  PR-ENT-001 remote development UX and PR-ENT-002 collaboration/admin
  controls remain deferred with explicit cut line
  (`plans/product-readiness-ledger.md`).

---

## Appendix A — Source citation map

| Topic | Source | Symbol / Line |
| --- | --- | --- |
| Migration registry | `crates/legion-storage/src/lib.rs` | `StorageMigrationRegistry` (line 291), `StorageMigrationStep`, `StorageSchemaManifest`, `StorageMigrationDryRunReport` |
| Backup / recovery | `crates/legion-storage/src/lib.rs` | `backup_file` (line 370), `recover_from_backup` (line 406), `StorageBackupMarker`; `STORAGE_CHECKSUM_ALGORITHM` (line 48) |
| Forward-only invariant | `crates/legion-storage/src/lib.rs` | `StorageMigrationRegistry::register` (line 315) |
| Save flow | `crates/legion-app/src/lib.rs` | `SaveWorkflowService::save_active_buffer` (line 10430), `WorkspaceActor::save_file_with_proposal` (line 10525), `AppComposition::save_active_buffer` (line 18122) |
| Protocol validators | `crates/legion-protocol/src/lib.rs` | `validate_storage_schema_manifest`, `validate_storage_migration_dry_run_report`, `validate_storage_backup_marker`, `validate_storage_recovery_outcome`, `validate_storage_repair_request` |
| Raw-source retention | `crates/legion-retention/src/lib.rs` | `RawSourceRetentionPolicy`, `RawSourceRetentionConsentGrant`, `RawSourceRetentionLease`, `validate_raw_source_vault_envelope` |
| Hosted telemetry consent | `crates/legion-telemetry/src/lib.rs` | `HostedTelemetryConsentGrant`, `HostedTelemetryEndpointDescriptor`, `HostedTelemetryExportBatch` |
| Release pipeline | `xtask/src/release_pipeline.rs` | `VERSION_STAMP_FILE` (line 18), `VERIFY_REPORT_FILE` (line 19), `VersionStamp` (line 184), `InstallerDescriptor` (line 196), `VerificationReport` (line 242) |
| Release pipeline config | `xtask/release-pipeline.example.toml` | signing posture, installer targets, manifest format |
| xtask gates | `xtask/src/main.rs` | `DEFAULT_RELEASE_PIPELINE_CONFIG_PATH` (line 32), `DEFAULT_RELEASE_PIPELINE_OUTPUT_PATH` (line 33); verify logic at lines 757, 811 |
| Phase-8 evidence (current) | `plans/evidence/phase-8/storage-migration-recovery-tests.txt` | `cargo test -p legion-storage migration_registry` passes |
| Phase-8 GUI evidence | `plans/evidence/gui-productization/phase-8-final-gates.md` | matrix proof GitHub Actions run `26590800830` (2026-05-28) |
| Rollback decision record | `plans/evidence/gui-productization/phase-8-update-rollback-incident.md` | rollback triggers and incident checklist |
| M6 acceptance gate | `plans/evidence/production/M6/M6-milestone-acceptance.md` | HEAD `dc92004`, all required gates pass |
| PR-REL-001 readiness row | `plans/product-readiness-ledger.md` | `Packaging, licensing, release` row; current `In progress` |
| Phase status | `plans/phase-status-ledger.md` | Phase 8 substrate accepted; GUI productization evidence owned by post-substrate track |
| Operator runbook | `docs/OPERATOR_RUNBOOK.md` | §Local verification gates; §Release signer references; §Expected artifacts |
| Security posture | `docs/SECURITY.md` | §Egress policy; §Secret handling and retention |
| Mode policy | `docs/MODES.md` | §Manual mode forbids AI / cloud / hosted telemetry |
| External audit | `audit-reports/external-security-audit-2026-06-13.md`; `plans/evidence/security/P9-F2-T4-external-audit-gate.md` | Gate note archive |
| Tag-triggered release job | `.github/workflows/ci.yml` | lines 165–196 (matrix: ubuntu-latest, macos-latest, windows-latest; `if: startsWith(github.ref, 'refs/tags/v')`) |
| Migration companion | `docs/releases/v8.0.0/migration-policy.md` | §3 forward compatibility; §4 dual-write window; §6 cutover date; §7 decision owners |
| Rollback companion | `docs/releases/v8.0.0/rollback-policy.md` | §3 rollback triggers; §3.4 crash-rate threshold |
| Stable-channel approval | `docs/releases/v8.0.0/stable-channel-approval.md` | §3 approver of record |
| Handoff notes | `docs/releases/v8.0.0/handoff.md` | v7 → v8 handoff narrative |

## Appendix B — Acceptance block

```
APPROVAL — Legion IDE v8.0.0 GA Release Checklist

Checklist review (Release manager):
  Name:  __________________________
  Role:  __________________________
  Date:  __________________________ (YYYY-MM-DD)

Gate sweep (GA approver):
  G1 Repository health            [ ] GO  [ ] NO-GO
  G2 Migration dry-run            [ ] GO  [ ] NO-GO
  G3 Telemetry / privacy          [ ] GO  [ ] NO-GO
  G4 Crash-free session ≥ 99.5 %  [ ] GO  [ ] NO-GO
  G5 Rollback readiness           [ ] GO  [ ] NO-GO
  G6 Security / supply-chain      [ ] GO  [ ] NO-GO
  G7 Docs / release index         [ ] GO  [ ] NO-GO
  G8 Stable-channel approval      [ ] GO  [ ] NO-GO

Composite decision:                  [ ] GO  [ ] NO-GO

Sign-off references:
  - docs/releases/v8.0.0/release-checklist.md  (this document)
  - docs/releases/v8.0.0/migration-policy.md
  - docs/releases/v8.0.0/rollback-policy.md
  - docs/releases/v8.0.0/handoff.md
  - docs/releases/v8.0.0/stable-channel-approval.md
  - plans/evidence/release/v8.0.0-staging-crash-rate.json
  - plans/evidence/production/M6/M6-milestone-acceptance.md
  - plans/product-readiness-ledger.md (PR-REL-001 row)

Decision:
  [ ] Approved — proceed with v8.0.0 cutover on the scheduled date
  [ ] Approved with conditions — see comment block
  [ ] Blocked — see comment block
```