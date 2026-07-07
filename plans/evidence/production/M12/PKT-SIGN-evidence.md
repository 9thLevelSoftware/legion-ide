# PKT-SIGN Evidence — Real Release Signing Infrastructure (M12)

Branch: `m12/release-signing`
Date: 2026-07-07

## Scope

PKT-SIGN implements real Ed25519 release signing infrastructure with an unsigned-beta fallback
for the Legion IDE auto-updater (ADR-0042). It covers:

- `ReleaseManifestV1` DTO in `legion-protocol` (forward-compatible, `#[non_exhaustive]`)
- `xtask/src/signing.rs`: Ed25519 signer via `ed25519-dalek` v2, resolver chain
  (env / ci-secret / keyring / kms), `verify_ed25519_signature()`, key zeroization
- `xtask/src/release_pipeline.rs`: `--from-artifacts` mode, three explicit pipeline modes
  (dry-run / from-artifacts / rejected), signer resolution integration, sha256 computation
- `xtask/src/main.rs`: `release-manifest` subcommand, `--from-artifacts` flag
- `xtask/release-pipeline.example.toml`: updated with `[signing]` and `[updater]` sections
- `docs/OPERATOR_RUNBOOK.md`: Ed25519 key format, manifest commands, unsigned-beta policy
- Kanban: P8.F1.T1, T2, T4 → done; T3 stays todo (fresh-VM smoke evidence)

## Security invariants enforced

- Key material is NEVER committed, logged, or written to disk
- Key bytes are wrapped in `zeroize::Zeroizing<_>` and zeroized after `SigningKey` construction
- Only signer references (env var names, keyring service names) are stored in the repo
- Test keys are ephemeral, generated in-test via deterministic seed, never persisted

## Test results

### cargo test -p xtask (full xtask suite)

```
running 17 tests (manifest_sign)
test keypair_roundtrip ... ok
test tampered_manifest_fails_verification ... ok
test tampered_signature_fails_verification ... ok
test tampered_artifact_hash_fails_verification ... ok
test wrong_verifying_key_fails_verification ... ok
test unsigned_beta_status_when_env_signer_unavailable ... ok
test env_resolver_roundtrip ... ok
test ci_secret_resolver_is_same_as_env ... ok
test kms_resolver_returns_honest_unavailable ... ok
test unknown_source_returns_unavailable ... ok
test keyring_resolver_visible_skip_if_unavailable ... ok
test empty_source_returns_unavailable_not_configured ... ok
test env_resolver_rejects_wrong_length_seed ... ok
test release_manifest_v1_validates_well_formed ... ok
test release_manifest_v1_rejects_empty_artifacts ... ok
test release_manifest_v1_rejects_empty_artifact_sha256 ... ok
test release_manifest_v1_toml_roundtrip ... ok

running 17 tests (release_pipeline)
test release_pipeline_plan_is_deterministic_for_same_inputs ... ok
test release_pipeline_descriptors_use_dry_run_signer_and_pending_sha256 ... ok
test release_pipeline_preview_channel_changes_version_label_only ... ok
test release_pipeline_write_descriptors_is_idempotent ... ok
test release_pipeline_write_descriptors_rejects_file_name_collision ... ok
test release_pipeline_rejects_when_neither_mode_given ... ok
test release_pipeline_from_artifacts_mode_with_absent_files_uses_unsigned_beta ... ok
test release_pipeline_stable_rollout_policy_is_full ... ok
test release_pipeline_preview_rollout_policy_is_staged ... ok
test release_pipeline_dry_run_version_stamp_matches_descriptor ... ok
test release_pipeline_verify_descriptors_passes_on_matching_descriptors ... ok
test release_pipeline_verify_descriptors_fails_on_missing_descriptor ... ok
test release_pipeline_verify_descriptors_fails_on_mismatched_version_stamp ... ok
test release_pipeline_verify_descriptors_fails_on_mismatched_signer_status ... ok
test release_pipeline_stable_descriptor_contains_channel_and_rollout_policy ... ok
test release_pipeline_write_version_stamp_is_idempotent ... ok
test verify_report_written_after_verify_descriptors ... ok

test result: ok. 34 passed; 0 failed
```

### cargo deny check

```
Checked 1 crate, no issues found.
(ed25519-dalek 2 and all transitive deps are single-version additions;
 no new multiple-versions skip entries required)
```

### cargo run -p xtask -- verify-kanban-backlog

Passes after P8.F1.T1, T2, T4 marked done with this evidence file.

## Pre-existing issues (not introduced by PKT-SIGN)

`cargo run -p xtask -- check-deps` reports two pre-existing violations:

- `legion-agent` depends on `legion-debug` (not in allowed policy set)
- `legion-app` depends on `legion-sandbox` (not in allowed policy set)

These violations were confirmed present before this branch via `git stash` + re-run. They are
not introduced by PKT-SIGN and are tracked separately.

## Files changed

### New files
- `crates/legion-protocol/src/release_manifest.rs` — `ReleaseManifestV1` + `ReleaseArtifact` DTOs
- `xtask/src/signing.rs` — Ed25519 signing module with resolver chain
- `xtask/tests/manifest_sign.rs` — 17 TDD tests for signing, verification, and resolvers

### Modified files
- `crates/legion-protocol/src/lib.rs` — added `pub mod release_manifest`
- `xtask/Cargo.toml` — added `ed25519-dalek`, `base64`, `zeroize`, `keyring`, `sha2`, `hex`
- `xtask/src/lib.rs` — added `pub mod signing`
- `xtask/src/main.rs` — `--from-artifacts` flag, `release-manifest` subcommand
- `xtask/src/release_pipeline.rs` — complete rewrite for three-mode pipeline
- `xtask/tests/release_pipeline.rs` — updated for new API signatures and new mode tests
- `xtask/release-pipeline.example.toml` — added `[signing]` and `[updater]` sections
- `docs/OPERATOR_RUNBOOK.md` — Ed25519 key format, manifest commands, unsigned-beta policy
- `plans/kanban/legion-ga-backlog.toml` — P8.F1.T1, T2, T4 → done

## Kanban tasks closed

| Task | Title | Status |
|------|-------|--------|
| P8.F1.T1 | Define non-committed signer config: env/keyring/KMS/CI secret reference | done |
| P8.F1.T2 | Add real signing path (Ed25519 + unsigned-beta fallback) | done |
| P8.F1.T4 | Preserve unsigned-beta policy if shipping before credentials exist | done |
| P8.F1.T3 | Add fresh-VM Gatekeeper/SmartScreen/install smoke evidence | todo (deferred) |
