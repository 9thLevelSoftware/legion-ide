# P9.F4.T1 / P9.F4.T2 — Consented training corpus and raw-trace opt-in controls

Date: 2026-08-19
Backlog rows: `P9.F4.T1`, `P9.F4.T2`
Readiness rows referenced: `PR-AI-002` (T1), `PR-ENT-002` (T2)

## Starting condition

`crates/legion-observability/src/training.rs` existed but was **never declared as a
module** in `crates/legion-observability/src/lib.rs`. It did not compile as part of the
crate and none of its tests ran under `cargo test -p legion-observability`. The
acceptance/rejection conversion helper and its fixture were effectively dead code.

`crates/legion-retention/src/training.rs` compiled, but the only consent artifact it
knew about was a `RawSourceRetentionConsentGrant` **passed in by the caller**. A caller
could construct its own grant and capture against it, so there was no stored record that
a user had opted in to raw-trace retention.

## Consent model implemented

Two layers, each of which fails closed on its own.

### Layer 1 — metadata-only training pipeline (`legion-observability::training`)

Three stages, each re-checking consent:

1. `build_training_candidate_corpus` filters `(audit, proposal)` traces. Only
   `AssistedAiConsentState::Granted` and `NotRequired` pass; `Denied`, `Missing`,
   `RenewalRequired`, and an absent disposition are counted and dropped. Only
   `Approved`/`Rejected` lifecycle states produce a label.
2. `build_training_adapter_dataset` re-validates every candidate before it becomes
   adapter input: consent state, `redaction_state == MetadataOnly`,
   `runtime_invocation_state == NotEncoded`, no payload title without a raw-trace
   reference, and no raw-trace reference without recorded redaction enforcement. This
   stage exists because a corpus is a *file*: it can reach the trainer from disk without
   ever having passed stage 1.
3. `build_training_eval_comparison` compares the dataset's acceptance rate (basis
   points, integer, so the value is stable across machines) against the archived
   Legion-Bench baseline in `evals/training-candidates/eval_baseline.json`.

Redaction at the boundary: the free-text `ProposalPayloadSummary.title` is stripped from
every candidate unless a raw-trace opt-in attestation was supplied. Only the title's byte
length survives. The checked-in corpus therefore contains no proposal prose.

### Layer 2 — raw-trace opt-in ledger (`legion-retention::training`)

`RawTraceOptInRow` is the stored opt-in. It binds principal, workspace, purpose, path
scope, and an expiry, and it carries a separate `export_allowed` flag.
`RawTraceOptInLedger` holds the rows; `revoke` removes a row outright.

- `capture_raw_trace_under_opt_in` looks the row up (it is never supplied), refuses when
  there is no row / the row lapsed / the purpose or workspace does not match, scans the
  payload for credentials before anything is sealed, and then **derives** the consent
  grant from the row.
- `export_raw_trace_under_opt_in` derives `raw_source_consent_verified` from the row's
  `export_allowed` flag rather than from a caller-supplied boolean. Retention consent is
  not export consent, and revoking the row blocks export of already-retained bundles.
- `delete_raw_trace_under_opt_in` is deliberately **not** gated on a live row: an expired
  or revoked consent must not become a reason to keep bytes the user asked to remove.
- `RawTraceOptInLedger::attest` mints the `RawTraceOptInAttestation` that unlocks the
  raw path in layer 1. With no live row there is no attestation.

`legion-retention` now depends on `legion-observability` so the attestation type is
single-sourced. `plans/dependency-policy.md` already permits that edge; `check-deps`
passes.

## Artifacts

`evals/training-candidates/`:

- `source_traces.json` — 7 fixture traces: 3 consented terminal, 3 unconsented
  (`Denied`, `Missing`, `RenewalRequired`), 1 consented but non-terminal (`Previewed`).
- `consented_accept_reject.jsonl` — the regenerated corpus, 3 candidates
  (2 accepted / 1 rejected), no titles, no raw-trace references.
- `eval_baseline.json` — `legion-bench-v0`, suite fingerprint
  `bench-suite-v1:bd2aa3a7d84d9485`, `accepted_rate_bp = 6666` (the first archived
  corpus acceptance rate).
- `corpus_manifest.json` — corpus fingerprint `training-corpus-v1:19bf58cc1855d8c0`,
  dataset fingerprint `training-adapter-v1:aa466e80c17f557a`, and the archived
  comparison (`delta_bp = 0`, `regressed = false`).

Regeneration is an explicitly ignored test:
`cargo test -p legion-observability regenerate_training_candidate_fixtures -- --ignored`.

Baseline scope note: the recorded Legion-Bench report labels its own scoring
`synthetic_budget_arithmetic`, so the suite fingerprint here anchors *which* suite
version the corpus was compared against. It is not a measurement of model quality.

## Verification commands run

| Command | Result |
| --- | --- |
| `cargo test -p legion-observability` | 63 passed / 0 failed / 1 ignored; 8 passed; 0 doc-tests |
| `cargo test -p legion-retention` | 46 passed / 0 failed; 6 passed; 0 doc-tests |
| `cargo run -p xtask -- legion-bench --mode recorded` | `total=20 passed=20 failed=0 regressed=0 fingerprint=bench-suite-v1:bd2aa3a7d84d9485` |
| `cargo clippy --workspace --all-targets -j 6 -- -D warnings` | clean |
| `cargo fmt --all` | applied |
| `cargo run -p xtask -- check-deps` | `dependency policy checks passed` |
| `cargo run -p xtask -- docs-hygiene` | `documentation hygiene checks passed` |
| `cargo run -p xtask -- claim-audit` | `claim audit passed` |
| `cargo run -p xtask -- extract-before-modify` | `no chokepoint file grew past its slack` |
| `cargo run -p xtask -- verify-kanban-backlog` | `kanban backlog ok: 162 task(s)` |
| `cargo run -p xtask -- verify-readiness-consistency` | `readiness consistency ok: 162 backlog task(s)` |

## Negative tests

Consent (T1):

- `unconsented_traces_are_not_converted` — `Denied`, `Missing`, `RenewalRequired`, and an
  absent disposition each produce no candidate.
- `unconsented_fixture_traces_never_reach_the_corpus` — the unconsented fixture audit ids
  appear neither in the corpus nor in its serialized JSONL.
- `adapter_refuses_a_corpus_carrying_an_unconsented_candidate` — a hand-edited corpus with
  a `Denied` candidate is refused at the adapter.
- `adapter_refuses_a_corpus_carrying_a_non_metadata_only_candidate` — an encoded provider
  invocation is refused.
- `adapter_refuses_a_payload_title_reinstated_without_an_opt_in_row` — a title added back
  by hand is refused.
- `adapter_refuses_a_raw_trace_reference_without_redaction_enforcement` — a forged raw
  trace reference is refused.

Redaction (T1/T2):

- `proposal_title_is_redacted_without_a_raw_trace_opt_in` — the title is absent from both
  the candidate and its serialized form.
- `checked_in_corpus_is_metadata_only` — every checked-in line has a null title and no
  raw-trace reference.
- `expired_opt_in_attestation_refuses_raw_trace_attachment` and
  `attestation_without_redaction_enforcement_refuses_raw_trace_attachment`.

Raw-trace storage (T2):

- `capture_without_an_opt_in_row_is_denied_and_stores_nothing`
- `capture_under_an_expired_opt_in_row_is_denied_and_stores_nothing`
- `capture_after_revocation_is_denied_and_stores_nothing`
- `capture_outside_the_opt_in_purpose_is_denied`
- `capture_outside_the_opt_in_path_scope_is_denied`
- `capture_carrying_a_credential_is_denied_and_stores_nothing`
- `capture_carrying_a_credential_is_denied_even_when_the_vault_would_accept_it` — the gate
  refuses with the vault's own `deny_capture_on_detected_secrets` switched off.
- `attestation_is_refused_without_a_live_opt_in_row`
- `opt_in_rows_must_expire`, `opt_in_rows_must_carry_a_path_scope`

Each of these asserts the vault directory holds zero `.vault` files after the refusal, so
"denied" means nothing was written, not merely that an error was returned.

Export controls (T2):

- `export_is_refused_when_the_opt_in_row_does_not_allow_export`
- `export_is_refused_after_the_opt_in_row_is_revoked`
- `hosted_export_linkage_requires_verified_raw_source_consent`

Deletion handles (T2):

- `deletion_handle_removes_the_ciphertext_and_the_descriptor` — the `.vault` file count
  drops to zero, the descriptor read returns `BundleMissing`, and the sealed bytes are
  unreadable.
- `deletion_still_works_after_the_opt_in_row_is_revoked`

## Mutation proofs

Each mutation was applied, the tests were run, the mutation was reverted with
`git checkout --`, and `git status --short` was confirmed empty afterwards.

| # | Mutation | Result |
| --- | --- | --- |
| M1 | `build_candidate`: `if !is_consented(...)` → `if false` | 1 failed — `unconsented_traces_are_not_converted`: "consent state Denied must not produce a training candidate" |
| M1b | M1 plus the corpus-level pre-filter → `if false` | 5 failed — `unconsented_traces_are_not_converted`, `unconsented_fixture_traces_never_reach_the_corpus` ("unconsented audit `assist:audit:req-9001:91` reached the corpus"), `checked_in_corpus_reproduces_from_the_source_traces` (candidate count 6 vs 3), plus two failures whose message shows the adapter's own re-check firing: `UnconsentedCandidate { consent_state: "Denied" }` |
| M2 | `assert_candidate_is_trainable` returns `Ok(())` immediately | 4 failed — all four adapter-refusal tests returned a built `TrainingAdapterDataset` instead of an error |
| M3 | `build_candidate`: always keep the payload title | 4 failed — `proposal_title_is_redacted_without_a_raw_trace_opt_in` (`Some("Fix acceptance edge case in ledger reconciliation")` vs `None`), `checked_in_corpus_reproduces_from_the_source_traces` (JSONL mismatch on `title`), and two more via `NonMetadataOnlyCandidate { reason: "payload title retained without a raw-trace opt-in row" }` |
| M4 | `capture_raw_trace_under_opt_in`: fall back to a caller-derived grant when the ledger has no row (the pre-existing hole) | 4 failed — `capture_without_an_opt_in_row_...`, `capture_under_an_expired_opt_in_row_...`, `capture_after_revocation_...`, `capture_outside_the_opt_in_purpose_...`; each returned a lease and a sealed 96-byte bundle, i.e. exactly the stop-condition violation |
| M5 | Remove the gate's credential scan | 1 failed — `capture_carrying_a_credential_is_denied_even_when_the_vault_would_accept_it`: a 126-byte bundle was sealed |
| M6 | Remove the `export_allowed` check | 1 failed — `export_is_refused_when_the_opt_in_row_does_not_allow_export` returned `HostedRetentionExportLinkage { raw_source_consent_verified: true }` |
| M7 | `FileBackedRawSourceVault::delete_bundle`: skip `fs::remove_file` | 2 failed — both deletion tests: "deletion must remove the sealed ciphertext from disk" (1 file vs 0) |

M5 initially killed nothing, because the vault's own `deny_capture_on_detected_secrets`
default masked the gate's scan. That gap was closed by adding
`capture_carrying_a_credential_is_denied_even_when_the_vault_would_accept_it`, which opens
the vault with that flag off; M5 was then re-run and did fail. The masking is recorded
here rather than quietly fixed, because it is the exact shape of a test that would have
passed whether or not the feature worked.

## Stop conditions

- T1 — "Stop if a non-consented trace can land in the training candidate set." Enforced at
  the corpus filter and re-enforced at the adapter boundary; M1/M1b/M2 each break one of
  those and produce failing tests.
- T2 — "Stop if a raw trace can be stored without an opt-in row." Enforced by
  `capture_raw_trace_under_opt_in`, which derives the grant from a looked-up row; M4
  breaks it and the tests fail with a sealed bundle in hand.

## Not done

- No UI surface. The ledger, corpus, and export controls are library-level; wiring an
  opt-in toggle into the privacy inspector is not in either task's `files`.
- The opt-in ledger is in-memory. Durable persistence of opt-in rows is a separate
  concern from the enforcement contract these tasks specify.
- No adapter training run happens here; that is `P9.F4.T3`, which consumes this corpus.
