# Observability & Debug Review

Reviewed files:
- `crates/legion-observability/src/lib.rs`
- `crates/legion-observability/src/telemetry.rs`
- `crates/legion-observability/src/minidump.rs`
- `crates/legion-observability/src/training.rs`
- `crates/legion-debug/src/lib.rs`
- `crates/legion-debug/src/evidence.rs`

Validation note: `cargo check -p legion-observability -p legion-debug` completed successfully. Findings below are logic, privacy, redaction, and failure-mode issues found by full-file review.

## Summary

- Total findings: 13
- Critical: 1
- High: 6
- Medium: 5
- Low: 1

## `crates/legion-observability/src/lib.rs`

### Finding 1

- Category: bug
- Severity: high
- Line numbers: 713-735, 1631-1635, 1754-1757
- Description: `metadata_fingerprint` and `metadata_hash` build audit fingerprints with `std::collections::hash_map::DefaultHasher`. That hasher is not a stable, documented file/audit fingerprint algorithm and is not collision-resistant, yet the values are stored in `FileFingerprint` fields for provider, route, request, projection, preview, path, command, and policy metadata. Durable replay/audit comparisons can change across Rust implementations or collide far more easily than a real digest.
- Suggested fix direction: Replace `DefaultHasher` with an explicit stable digest such as SHA-256 or BLAKE3, set `FileFingerprint.algorithm` to the real digest algorithm plus any domain-separation label, and add golden tests proving hashes remain stable across runs.

### Finding 2

- Category: failure-point
- Severity: high
- Line numbers: 1799-1812, 1822-1830, 1832-1837
- Description: Metadata-only redaction preserves every top-level string whose key does not match a small denylist. Keys such as `summary`, `reason`, `message`, `diagnostics`, `path`, `command`, or `metadata_summary` can therefore retain raw source text, paths, prompts, terminal output, or secrets if a caller emits them under those names. Nested objects are replaced, but scalar strings are trusted without value inspection.
- Suggested fix direction: Make metadata-only redaction allowlist-based rather than denylist-based, hash or length-summarize free-form strings by default, and require typed builders/validators for any scalar string that is retained verbatim.

### Finding 3

- Category: failure-point
- Severity: medium
- Line numbers: 643, 1067, 1089, 1109, 1155, 1530, 1760-1767
- Description: Many public event helper functions return `EventEnvelope` directly but call `assert_core_ids`, which panics on a nil causality id, zero correlation id, or zero sequence. Invalid runtime metadata can therefore crash the caller instead of producing a recoverable `ObservabilityError`/protocol error.
- Suggested fix direction: Change public helpers that validate externally supplied identifiers to return `Result<EventEnvelope, ObservabilityError>`, reuse `validate_envelope`/protocol validation, and reserve panics for internal invariant violations only.

### Finding 4

- Category: bug
- Severity: medium
- Line numbers: 602-634
- Description: `proposal_audit_record` combines `proposal` data with lifecycle, timestamp, principal, capability, correlation, and causality from `transition`, but never verifies `transition.proposal_id == proposal.proposal_id`. A caller bug can create an audit record and checkpoint/rollback projection that attributes another proposal's transition to the current proposal.
- Suggested fix direction: Return a `Result`, reject mismatched proposal ids, and add tests covering cross-proposal transition misuse.

### Finding 5

- Category: failure-point
- Severity: low
- Line numbers: 568-583
- Description: `event_metadata_record` hardcodes `schema_version: 1` instead of copying `envelope.schema_version`. If the envelope schema is bumped, persisted metadata records will silently report the wrong schema version.
- Suggested fix direction: Copy the envelope schema version or introduce an explicit metadata-record schema field that is named separately from the source envelope schema.

## `crates/legion-observability/src/telemetry.rs`

### Finding 6

- Category: failure-point
- Severity: medium
- Line numbers: 122-178
- Description: `suggestion_metadata_summary` concatenates request ids, result ids, acceptance/dismissal ids, provider ids, model labels, health labels, and cost labels directly into one free-form string. The helper accepts most of these as strings from upstream DTOs and does not hash, bound, escape, or marker-scan them here, so a malformed provider label or id can inject raw text, newlines, or parser-confusing fields into hosted telemetry metadata.
- Suggested fix direction: Store structured metadata fields instead of a space-delimited string, hash or length-summarize arbitrary labels, bound field lengths, and validate provider/model labels against the same metadata-only raw-marker policy used by audit records.

## `crates/legion-observability/src/minidump.rs`

### Finding 7

- Category: bug
- Severity: high
- Line numbers: 26-31, 58-60
- Description: Crash symbol upload gating only checks `consent.crash_reports_enabled`. It ignores the master `consent.enabled` flag, so an inconsistent or stale consent record with telemetry disabled but crash reports enabled can still mark symbol upload as queued and include the upload target.
- Suggested fix direction: Gate symbolication/upload on a single validated consent predicate such as `consent.enabled && consent.crash_reports_enabled`, and add tests for inconsistent consent combinations.

### Finding 8

- Category: failure-point
- Severity: high
- Line numbers: 39-50, 58-60
- Description: The crash summary envelope stores `report.crash_id`, `report.signal`, `report.metadata_summary`, and, when consented, `symbol_upload_target` verbatim in a metadata-only payload. Crash summaries and upload targets can contain paths, bucket names, query strings, or raw crash annotations, and this helper does not validate or hash them before persistence.
- Suggested fix direction: Validate crash-report strings for metadata-only safety, hash or classify upload targets instead of storing raw endpoints, and record only bounded crash-summary labels/counts unless raw retention has been explicitly authorized elsewhere.

### Finding 9

- Category: bug
- Severity: medium
- Line numbers: 26-31, 47-49
- Description: `symbolicated` is set to `consent.crash_reports_enabled` rather than to whether the minidump was actually symbolicated. With crash reporting enabled but no upload target, the event reports `symbolicated=true` and `symbol_upload_state=skipped`, which is contradictory audit metadata.
- Suggested fix direction: Track separate states for consent, upload queued, upload completed, and symbolication completed; only set `symbolicated=true` after successful symbolication metadata is available.

## `crates/legion-observability/src/training.rs`

### Finding 10

- Category: failure-point
- Severity: high
- Line numbers: 87, 102-128
- Description: `consented_training_candidate_from_records` validates the assisted-AI audit record but does not validate the `ProposalAuditRecord` before copying `proposal.payload_summary`, lifecycle state, and other proposal-derived fields into a training artifact. A malformed proposal audit with `RedactionHint::None`, schema version zero, raw titles, paths, or diagnostics can therefore enter the training-candidate dataset.
- Suggested fix direction: Add and call a proposal-audit validation/redaction routine before candidate creation, reject non-metadata-only proposal fields, and add tests with raw path/title/diagnostic payloads.

### Finding 11

- Category: failure-point
- Severity: medium
- Line numbers: 77-85
- Description: The training candidate gate treats `AssistedAiConsentState::NotRequired` the same as explicit `Granted` consent. If training export or retention policy requires affirmative user consent, traces that only skipped consent because it was considered unnecessary can be retained for training.
- Suggested fix direction: Split local/non-export retention from training retention and require an explicit training consent grant unless a policy object proves `NotRequired` is acceptable for this exact artifact.

### Finding 12

- Category: bug
- Severity: medium
- Line numbers: 89-92
- Description: Only `ProposalLifecycleState::Approved` is labeled as `Accepted`; `Applied` traces return `Ok(None)`. If downstream audit records are captured after a proposal is actually applied, successfully accepted/applied examples will be silently dropped from the training set, biasing the dataset toward pre-apply approvals.
- Suggested fix direction: Define the intended acceptance lifecycle boundary explicitly and include `Applied` when the training label means successful acceptance/application, or document and test that only pre-apply approval events are eligible.

## `crates/legion-debug/src/lib.rs`

No findings in this re-export-only file.

## `crates/legion-debug/src/evidence.rs`

### Finding 13

- Category: bug
- Severity: critical
- Line numbers: 13-17, 91, 99, 127, 138
- Description: `fingerprint` labels every `FileFingerprint` as `sha256` but stores the input string directly instead of a SHA-256 digest. `debug_adapter_audit_evidence` and `test_run_summary_evidence` therefore put raw summary text into `summary_hash` and `log_hashes`, and the algorithm label is false. This both leaks metadata/raw evidence into fields intended to be hashes and breaks any verifier that expects a real SHA-256 value.
- Suggested fix direction: Compute a real SHA-256 digest over the domain-separated summary/log text, store only the hex digest in `value`, and update tests so they assert the raw summary is absent and the digest format is valid.
