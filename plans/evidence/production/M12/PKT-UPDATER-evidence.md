# PKT-UPDATER Evidence — M12 Campaign

**Packet**: PKT-UPDATER (P8.F2 — Auto-update and rollback client)
**Campaign**: M12 release readiness
**Branch**: `m12/updater`
**Date**: 2026-07-07
**Status**: Complete

---

## What was delivered

### 1. Updater client module (`crates/legion-app/src/updater.rs`)

- `ManifestSource` trait with `fetch_manifest() -> (Vec<u8>, Option<Vec<u8>>)` signature.
- `LocalDirManifestSource`: reads `release-manifest.v1.toml` + optional `.sig` from a local directory. **HTTP source explicitly deferred** — no update server exists; documented non-stub deferral per ADR-0042 D5.
- `UpdatePolicy`: `current_version`, `current_channel`, `allow_unsigned_beta`.
- `UpdateCheck` enum: `Available { manifest, signer_status }` or `NoUpdate`.
- `StagedUpdate`: holds staged artifact directory + signer status.
- `UpdateJournal`: TOML-serializable journal struct (`[journal]` table).
- `UpdateError`: covers all failure modes.
- `Updater` (stateless): `check_for_update`, `stage_update`, `apply_update`, `rollback`.
- `compare_versions` / `version_is_newer`: hand-rolled numeric triple comparison with `-preview` suffix ordering.
- `verify_ed25519_signature`: duplicated from `xtask/src/signing.rs` (pure ~25-line function; no xtask dep).
- Windows-safe file writes (copy-then-rename, never in-place).

**Security invariants enforced:**
- Signature verification runs BEFORE TOML parsing (fail-closed).
- Unsigned manifests rejected unless `allow_unsigned_beta` is true.
- Key material never logged.

### 2. Tests (`crates/legion-app/tests/upd_tests.rs`)

TDD red→green: version-compare tests were pinned before implementation.

Tests cover:
- Version compare: basic minor/patch bump, major wins, preview < release, preview vs preview, equal.
- `verify_ed25519_signature`: tampered manifest bytes rejected before parse.
- `check_for_update` with bad signature: `SignatureInvalid`.
- Unsigned manifest with `allow_unsigned_beta: false`: `UnsignedNotAllowed`.
- Unsigned manifest with `allow_unsigned_beta: true`: proceeds with `signer_status = "unsigned-beta"`.
- Channel mismatch (preview manifest + stable policy): `ChannelMismatch`.
- Hash mismatch during staging: `HashMismatch`.
- Full pipeline (check→stage→apply): journal `current=0.2.0 previous=0.1.0`.
- Rollback: journal toggled to `current=0.1.0 previous=0.2.0`.
- Double rollback: idempotent toggle back to `current=0.2.0 previous=0.1.0`.
- Downgrade: `check_for_update` returns `NoUpdate`.
- Signed happy path: `signer_status = "signed/ed25519"` in journal.

### 3. Update drill binary (`crates/legion-app/src/bin/update_drill.rs`)

11 deterministic steps (s1–s11) + report write:

| Step | Description | Expected outcome |
| --- | --- | --- |
| s1 | Generate ephemeral Ed25519 keypair (time-based seed, in-memory) | 32-byte verifying key |
| s2 | Fabricate v0.2.0 artifact + manifest + sign with ephemeral key | Manifest file + `.sig` written |
| s3 | `check_for_update` v0.1.0 → v0.2.0 (signed) | `Available`, `signer_status=signed/ed25519` |
| s4 | `stage_update` → SHA-256 verified, artifact copied | `StagedUpdate` returned |
| s5 | `apply_update` → journal written | `current=0.2.0`, `previous=0.1.0` |
| s6 | `rollback` (toggle 1) | `current=0.1.0`, `previous=0.2.0` |
| s7 | Double rollback (idempotent toggle) | Back to `current=0.2.0`, `previous=0.1.0` |
| s8 | Negative: corrupted signature | `SignatureInvalid` |
| s9 | Negative: wrong artifact bytes | `HashMismatch` |
| s10 | Negative: preview manifest + stable policy | `ChannelMismatch` |
| s11 | Negative: downgrade (v0.2.0 → v0.1.0 manifest) | `NoUpdate` |

Report: `target/update-drill/update_drill_report.toml`

**Binary swap / restart: explicitly out of scope** (ADR-0042 D5). The drill validates the journal and artifact-verification pipeline only.

### 4. Xtask subprocess subcommand (`xtask/src/update_drill.rs`, `xtask/src/main.rs`)

`cargo run -p xtask -- update-drill` spawns `update_drill` binary as subprocess (subprocess model — xtask cannot depend on legion-app).

**19th standing gate**: registered in AGENTS.md, README.md, `plans/legion-production-master-plan-v0.2.md`, and `docs/OPERATOR_RUNBOOK.md`.

### 5. CI job (`.github/workflows/legion-smoke.yml`)

Job `update-drill` on 3-OS matrix (ubuntu/windows/macos), dispatched + weekly, independent (not a PR merge blocker). Uploads `update-drill-report-${{ matrix.os }}` artifact.

### 6. Dependency changes

- `ed25519-dalek = "2"` and `base64 = "0.22"` added to `[workspace.dependencies]`.
- `xtask/Cargo.toml` updated to use workspace references.
- `crates/legion-app/Cargo.toml` gains `ed25519-dalek = { workspace = true }` and `toml = { workspace = true }` as real (non-dev) dependencies.

---

## Kanban closure

| Task | v0.1 wording | v0.2 closure |
| --- | --- | --- |
| P8.F2.T1 | Strategy and manifest format documented in ADR | Done — ADR-0042 ratified in PKT-SIGN; `UpdaterConfig` reflected in pipeline config. |
| P8.F2.T2 | Each channel produces its own descriptor with no cross-channel leakage | Done — `UpdatePolicy::current_channel` + channel mismatch stop-condition implemented and tested. |
| P8.F2.T3 | "GP-5 update and rollback works" | Done — superseded by v0.2 numbering. The v0.1 "GP-5" reference mapped to the update-drill 19th gate. Installer-swap/restart remain explicitly deferred (ADR-0042 D5). |

---

## PR-REL-001 partial update

Auto-update/rollback is now **deterministically validated** via the update-drill gate (journal pipeline, artifact hash verification, rollback toggle, four negative cases). The installer-swap/process-restart path is **caveat-labeled as explicitly deferred** per ADR-0042 D5. Full PR-REL-001 promotion to "substrate validated" requires signed installer evidence and crash-report controls.

---

## Global constraints satisfied

- [x] TDD red→green (version-compare tests pinned first)
- [x] Conventional Commits
- [x] `manual_zero_egress` unaffected (updater module has no network I/O)
- [x] `verify-kanban-backlog` passes after kanban edits
- [x] Binary swap/installer/restart out of scope (documented)
- [x] No HTTP ManifestSource (explicit deferral, not a stub)
- [x] Sig verification before parsing (fail-closed)
- [x] Key material never logged or persisted
- [x] Windows: copy+rename for file ops
- [x] No stubs or fixture-only product paths
