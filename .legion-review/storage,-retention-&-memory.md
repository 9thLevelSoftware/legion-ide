# Storage, Retention & Memory Review

Scope reviewed:
- `crates/legion-storage/src/lib.rs`
- `crates/legion-storage/src/plan.rs`
- `crates/legion-storage/src/secrets.rs`
- `crates/legion-retention/src/lib.rs`
- `crates/legion-retention/src/training.rs`
- `crates/legion-memory/src/lib.rs`

Verification performed:
- `cargo check -p legion-storage -p legion-retention -p legion-memory` completed successfully.

## crates/legion-storage/src/lib.rs

### Finding 1
- Category: bug
- Severity: high
- Line numbers: 501-542, 745-775, 1121-1152
- Description: File-backed protocol records for workspace config, file metadata, session records, and trust records are not persisted across reopen. `StorageRepositoryRequest::SaveWorkspaceConfig`, `SaveFileMetadata`, `SaveSessionRecord`, and `SaveTrustRecord` write only to `protocol_workspace_configs`, `protocol_file_metadata`, `protocol_sessions`, and `protocol_trust`, but `PersistedState` has no fields for those maps and `From<PersistedState>` reinitializes them to empty. A `FileBackedStorage` flush/reopen therefore silently loses these protocol records while returning successful save responses before restart.
- Suggested fix direction: Add the missing protocol maps to `PersistedState` with `#[serde(default)]`, include them in `From<&InMemoryStorage>`, restore them in `From<PersistedState>`, and add a reopen regression test for all four protocol request variants.

### Finding 2
- Category: failure-point
- Severity: high
- Line numbers: 675-684
- Description: `InMemoryStorageRepositoryPort::record_event` emits the event envelope before it verifies that redacted event metadata was durably stored, and it evaluates both operations before returning. If `emit` succeeds but `SaveEventMetadata` fails, observers can receive an event that has no audit metadata. If storage succeeds but `emit` fails, the caller receives an error even though the record was persisted. This creates inconsistent event/audit state on partial failure.
- Suggested fix direction: Define a single ordering with fail-closed semantics, preferably validate and persist metadata before external emission, and return/record partial-failure state explicitly if both actions cannot be made atomic.

### Finding 3
- Category: failure-point
- Severity: medium
- Line numbers: 391-394, 422-427, 494-499
- Description: Migration backup verification uses `stable_storage_sum`, a wrapping additive checksum plus length, as the integrity value. This is trivially collision-prone: byte changes can preserve the same sum and length, allowing a corrupted or swapped backup to pass `recover_from_backup` checksum verification.
- Suggested fix direction: Replace the stable sum with a cryptographic digest such as SHA-256, set the checksum algorithm accordingly, and keep backward compatibility only through an explicit migration/legacy path.

### Finding 4
- Category: failure-point
- Severity: medium
- Line numbers: 789-798
- Description: Corrupt file quarantine ignores the result of `fs::rename`. `FileBackedStorage::open` can return `StorageError::Corrupt` with a quarantine path even if the rename failed because of permissions, a cross-device move, or another filesystem error. That leaves the corrupt primary file in place while reporting that it was quarantined.
- Suggested fix direction: Propagate rename failures or include them in the error, and only report a quarantine path after the corrupt file has actually been moved or copied-and-removed successfully.

## crates/legion-storage/src/plan.rs

No findings.

## crates/legion-storage/src/secrets.rs

### Finding 5
- Category: bug
- Severity: medium
- Line numbers: 93-115
- Description: `InMemorySecretStore` uses `Mutex::lock().expect("secret store lock")` in `store`, `load`, and `delete`. A poisoned lock will panic even though the `SecretStore` trait returns `Result<..., SecretStoreError>`, making test or injected stores able to unwind callers instead of returning a recoverable store error.
- Suggested fix direction: Map poisoned-lock errors into `SecretStoreError::KeyringFailure` or add a dedicated in-memory lock error variant so all trait implementations preserve the non-panicking contract.

### Finding 6
- Category: bug
- Severity: medium
- Line numbers: 118-123
- Description: `provider_secret_reference` builds the keyring account as `format!("{provider_id}:{secret_name}")` without escaping or length-prefixing either component. Inputs containing `:` can collide, for example `(provider_id="a:b", secret_name="c")` and `(provider_id="a", secret_name="b:c")` both map to `a:b:c`, which can overwrite or expose the wrong provider secret.
- Suggested fix direction: Encode the tuple unambiguously, e.g. length-prefix each component, percent/base64-url encode separators, or store a structured hash with collision-resistant domain separation.

### Finding 7
- Category: failure-point
- Severity: medium
- Line numbers: 67-74, 132-134
- Description: Missing keyring entries are detected by lowercasing `error.to_string()` and searching for `"not found"`. This is brittle across keyring versions, platforms, and localized/backend-specific messages, and can misclassify real backend failures as missing secrets or missing secrets as hard failures.
- Suggested fix direction: Match the concrete `keyring::Error` variant for missing credentials if available, or centralize backend-specific classification with tests for macOS, Windows, and Linux keyring backends.

## crates/legion-retention/src/lib.rs

### Finding 8
- Category: bug
- Severity: high
- Line numbers: 428-440, 590-597, 1188-1217
- Description: Raw-source retention leases store `expires_at` as `TimestampMillis(self.policy.ttl_ms)`, treating a TTL duration as an absolute timestamp. With a normal TTL such as 60,000 ms, every new bundle appears to expire at 1970-01-01T00:01:00Z and will be purged immediately by real wall-clock timestamps. The code also ignores the consent grant expiry when computing lease expiry.
- Suggested fix direction: Compute expiry as `min(now + policy.ttl_ms, grant.expires_at)` using a clock supplied to the capture path for testability, and update purge tests to use real absolute expiry timestamps rather than TTL literals.

### Finding 9
- Category: bug
- Severity: high
- Line numbers: 568-573, 626-630, 1219-1241
- Description: `RawSourceCaptureRequest::max_bytes` is validated only as metadata and is not enforced against the actual packed file payload. `capture_bundle` rejects only empty payloads or payloads over `config.max_bundle_bytes`; `pack_files` appends all file bytes without checking the request's byte budget. A caller can request a small consent-bound `max_bytes` but capture much more data as long as it fits the vault configuration.
- Suggested fix direction: Track cumulative packed/raw bytes in `pack_files` and reject when they exceed `request.max_bytes` before encryption. Consider also ensuring the final encrypted size is bounded by both the request budget and the vault configuration or documenting why tag/header overhead is excluded.

### Finding 10
- Category: failure-point
- Severity: high
- Line numbers: 1248-1253
- Description: The file-backed vault index is written with plain `fs::write`. A process crash, disk-full condition, or interrupted write can leave `index.json` truncated or partially written, making all retained bundle metadata unavailable on next open despite encrypted payload files still existing.
- Suggested fix direction: Use the same atomic temp-file, fsync, rename, and parent-directory sync pattern used by `legion-storage`, and add recovery/quarantine behavior for a corrupt index.

### Finding 11
- Category: failure-point
- Severity: medium
- Line numbers: 631-649
- Description: `capture_bundle` writes encrypted bundle bytes before inserting metadata and flushing the vault index. If `flush_index` fails after the ciphertext write, the vault returns an error but leaves an orphan `.vault` file that is not referenced by the index. Later capture attempts with the same logical bundle can overwrite or strand encrypted data with no descriptor/tombstone path.
- Suggested fix direction: Write ciphertext to a temporary file, stage index metadata, atomically commit both in a defined order, and roll back the temp/ciphertext file when index persistence fails.

### Finding 12
- Category: failure-point
- Severity: medium
- Line numbers: 1155-1185, 1203-1214
- Description: `delete_bundle` returns an I/O error if the ciphertext file is already missing and does not remove index metadata or record the tombstone. `purge_expired` calls `delete_bundle` with `?`, so one missing ciphertext can abort the whole purge and leave stale descriptors permanently blocking cleanup.
- Suggested fix direction: Treat missing ciphertext as a recoverable/quarantine deletion path: record a tombstone or recovery report, remove the stale descriptor metadata, continue purge for other bundles, and surface a warning/report rather than failing the entire cleanup.

## crates/legion-retention/src/training.rs

No findings.

## crates/legion-memory/src/lib.rs

### Finding 13
- Category: failure-point
- Severity: medium
- Line numbers: 157-168, 243-260
- Description: `MemoryService::from_snapshot` does not validate `MemoryServiceSnapshot.schema_version`. Snapshots with schema version 0 or a future unsupported schema are accepted as long as the contained records validate, which can silently restore incompatible or malformed persisted state.
- Suggested fix direction: Reject zero schema versions and explicitly gate future versions, or add a migration path before constructing `MemoryService`.

### Finding 14
- Category: bug
- Severity: medium
- Line numbers: 38-55, 467-482
- Description: `validate_memory_candidate` does not explicitly require `MemoryCandidateRecord.candidate_id` to be non-empty. It builds a Phase 4 audit id as `memory:{candidate_id}`, and the protocol audit-string validator rejects raw markers but not empty logical ids. Empty candidate ids can therefore be proposed/retained, making deletion and snapshot identity ambiguous.
- Suggested fix direction: Add a direct `candidate.candidate_id.trim().is_empty()` check and tests for empty candidate ids before constructing the audit wrapper.

### Finding 15
- Category: bug
- Severity: medium
- Line numbers: 299-306, 331-341, 366-373, 309-315, 344-350, 376-382
- Description: The three retain paths append records without checking for duplicate `candidate_id` or `trace_id`, while delete operations remove every matching id. Duplicate retained records can accumulate across repeated review/restore flows, produce duplicate export entries, and make deletion counts/auditability ambiguous.
- Suggested fix direction: Enforce uniqueness on retain (reject or replace existing ids), or store records in keyed maps and make delete semantics explicit.

### Finding 16
- Category: failure-point
- Severity: low
- Line numbers: 643-700
- Description: `LegionWorkflowOutcomeCandidate::from_session_metadata` hard-codes `event_sequence: EventSequence(13)` for every generated candidate. Multiple candidates from different sessions or repeated snapshots can share the same synthetic sequence, reducing audit ordering value and potentially colliding with downstream event-sequence assumptions.
- Suggested fix direction: Derive the event sequence from session metadata, accept it as a constructor parameter, or allocate it from the caller's event sequencing source.
